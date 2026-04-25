//! UNIM 데몬 프로세스
//!
//! DBus 기반 입력 서비스를 제공하고 프론트엔드 모듈들을 관리합니다.

// jemalloc: glibc ptmalloc의 멀티 스레드 arena 증식(64MB 단위 mmap 누적)을 회피해
// 장시간 실행 시 RSS가 수 GB까지 치솟는 문제를 방지. Linux/glibc 타겟에서만 활성화.
#[cfg(all(target_os = "linux", not(target_env = "musl")))]
#[global_allocator]
static GLOBAL: tikv_jemallocator::Jemalloc = tikv_jemallocator::Jemalloc;

use clap::Parser;
use std::fs;
use std::io::{Read, Write};
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;
use unim::unim_log;

use tokio::sync::mpsc;
use zbus::Connection;

use unim_dbus::engine_worker::spawn_engine_worker;
use unim_dbus::service::InputMethodService;
use unim_dbus::{BUS_NAME, INPUT_METHOD_PATH};

mod migration;

/// UNIM 입력기 데몬
#[derive(Parser, Debug)]
#[command(name = "unim-daemon", version, about = "UNIM 입력기 데몬")]
struct Args {
    /// 데몬화 없이 포그라운드에서 실행
    #[arg(short = 'n', long)]
    no_daemon: bool,

    /// 기존 데몬을 강제 종료하고 교체 실행
    #[arg(short = 'r', long)]
    replace: bool,

    /// 기존 데몬 실행 여부 확인 (실행 중이면 exit 0, 아니면 exit 1)
    #[arg(long)]
    check: bool,
}
/// 프론트엔드 모듈 종류
#[derive(Clone, Copy, Debug)]
enum Module {
    Xim,
    Wayland,
}

impl Module {
    fn binary_name(&self) -> &'static str {
        match self {
            Module::Xim => "unim-xim",
            Module::Wayland => "unim-wayland",
        }
    }

    fn resolve_path(&self) -> PathBuf {
        let name = self.binary_name();

        // 1. 환경 변수 (UNIM_XIM_PATH 등)
        let env_var = format!("UNIM_{}_PATH", name.replace("-", "_").to_uppercase());
        if let Ok(path) = std::env::var(&env_var) {
            let p = PathBuf::from(&path);
            if p.exists() {
                return p;
            }
        }

        // 2. 현재 실행 파일 기준 탐색
        if let Ok(exe_path) = std::env::current_exe() {
            if let Some(exe_dir) = exe_path.parent() {
                // 같은 디렉토리 (주로 libexec)
                let sibling_path = exe_dir.join(name);
                if sibling_path.exists() {
                    return sibling_path;
                }
            }
        }

        // 3. 빌드 디렉토리 (개발용)
        if std::env::var("UNIM_DEVELOP").is_ok() {
            if let Ok(cwd) = std::env::current_dir() {
                let dev_paths = [
                    cwd.join("target/release").join(name),
                    cwd.join("target/debug").join(name),
                    cwd.join("../target/release").join(name),
                    cwd.join("../target/debug").join(name),
                ];
                for p in dev_paths {
                    if p.exists() {
                        return p;
                    }
                }
            }
        }

        // 4. 시스템 표준 경로
        let system_paths = [
            PathBuf::from("/usr/local/libexec").join(name),
            PathBuf::from("/usr/libexec").join(name),
            PathBuf::from("/usr/local/bin").join(name),
            PathBuf::from("/usr/bin").join(name),
        ];
        for p in system_paths {
            if p.exists() {
                return p;
            }
        }

        // 5. PATH
        match which::which(name) {
            Ok(p) => p,
            Err(_) => PathBuf::from(name),
        }
    }

    fn description(&self) -> &'static str {
        match self {
            Module::Xim => "XIM 서버",
            Module::Wayland => "Wayland IM",
        }
    }
}

/// 필요한 모듈을 감지합니다.
fn detect_required_modules() -> Vec<Module> {
    let mut modules = Vec::new();

    // X11 감지 (DISPLAY 환경 변수)
    if std::env::var("DISPLAY").is_ok() {
        modules.push(Module::Xim);
        unim_log!("DAEMON", "X11 환경 감지 - XIM 서버 추가");
    }

    // Wayland 감지 (WAYLAND_DISPLAY 환경 변수)
    if std::env::var("WAYLAND_DISPLAY").is_ok() {
        modules.push(Module::Wayland);
        unim_log!("DAEMON", "Wayland 환경 감지 - Wayland IM 추가");
    }

    modules
}

/// GNOME+Wayland 환경에서 Flatpak 앱의 IM 환경변수를 설정합니다.
///
/// Flatpak 샌드박스 안에는 UNIM IM 모듈이 없으므로,
/// QT_IM_MODULE/GTK_IM_MODULE을 비워서 Wayland text-input-v3
/// 기본 경로(GNOME extension)를 사용하도록 합니다.
fn configure_flatpak_im_env() {
    // GNOME+Wayland 조건 확인
    let is_wayland = std::env::var("WAYLAND_DISPLAY").is_ok();
    let is_gnome = std::env::var("XDG_CURRENT_DESKTOP")
        .map(|d| d.contains("GNOME"))
        .unwrap_or(false);

    if !is_wayland || !is_gnome {
        return;
    }

    // flatpak 명령어 존재 확인
    let flatpak_exists = Command::new("which")
        .arg("flatpak")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false);

    if !flatpak_exists {
        return;
    }

    unim_log!(
        "DAEMON",
        "[Flatpak] GNOME+Wayland 감지 — Flatpak IM 환경변수 설정 시작"
    );

    // flatpak override --user --env=QT_IM_MODULE= --env=GTK_IM_MODULE=
    match Command::new("flatpak")
        .args([
            "override", "--user",
            "--env=QT_IM_MODULE=",
            "--env=GTK_IM_MODULE=",
        ])
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .output()
    {
        Ok(output) if output.status.success() => {
            unim_log!(
                "DAEMON",
                "[Flatpak] IM 환경변수 override 완료 (QT_IM_MODULE=, GTK_IM_MODULE= → text-input-v3 경로 사용)"
            );
        }
        Ok(output) => {
            let stderr = String::from_utf8_lossy(&output.stderr);
            unim_log!(
                "DAEMON",
                "[Flatpak] override 실패 (비치명적): {}",
                stderr.trim()
            );
        }
        Err(e) => {
            unim_log!(
                "DAEMON",
                "[Flatpak] override 실행 오류 (비치명적): {}",
                e
            );
        }
    }
}

/// 모듈 프로세스를 시작합니다.
fn start_module(module: &Module) -> Option<(String, Child)> {
    let path = module.resolve_path();
    let name = module.binary_name();

    match Command::new(&path)
        .stdin(Stdio::null())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .spawn()
    {
        Ok(child) => {
            unim_log!("DAEMON", "{} ({:?}) 시작됨", module.description(), path);
            Some((name.to_string(), child))
        }
        Err(err) => {
            unim_log!(
                "DAEMON",
                "{} ({:?}) 시작 실패: {}",
                module.description(),
                path,
                err
            );
            None
        }
    }
}

/// DBus 서비스를 시작합니다.
async fn start_dbus_service(
    engine_tx: mpsc::Sender<unim_dbus::service::EngineRequest>,
) -> zbus::Result<Connection> {
    let config = unim::config::Config::load_from_default_path();

    // 세션 버스에 먼저 연결
    let connection = Connection::session().await?;

    // DBus 프록시를 통해 이름 요청 (교체 허용 플래그 사용)
    use zbus::fdo::{DBusProxy, RequestNameFlags, RequestNameReply};
    let dbus_proxy = DBusProxy::new(&connection).await?;
    let flags = RequestNameFlags::ReplaceExisting | RequestNameFlags::AllowReplacement;

    let mut reply = dbus_proxy
        .request_name(BUS_NAME.try_into().unwrap(), flags.into())
        .await?;

    if reply == RequestNameReply::Exists {
        unim_log!(
            "DAEMON",
            "DBus 이름 '{}'이(가) 이미 사용 중입니다. 기존 프로세스 종료를 시도합니다.",
            BUS_NAME
        );
        let pid_file = get_pid_file_path();
        kill_existing_daemon(&pid_file);

        // 프로세스가 종료될 때까지 잠시 대기 후 재시도
        tokio::time::sleep(Duration::from_millis(500)).await;

        reply = dbus_proxy
            .request_name(BUS_NAME.try_into().unwrap(), flags.into())
            .await?;
    }

    match reply {
        RequestNameReply::PrimaryOwner => {
            unim_log!("DAEMON", "[DBus] 버스 이름 등록 성공: {}", BUS_NAME)
        }
        RequestNameReply::AlreadyOwner => {
            unim_log!("DAEMON", "[DBus] 이미 버스 이름의 소유자입니다")
        }
        RequestNameReply::InQueue => {
            unim_log!("DAEMON", "[DBus] 버스 이름 대기열에 추가되었습니다")
        }
        RequestNameReply::Exists => {
            unim_log!(
                "DAEMON",
                "[DBus] 버스 이름 등록 실패: 이미 다른 프로세스가 소유 중입니다."
            );
            return Err(zbus::Error::Failure("Name already taken".to_string()));
        }
    }

    // 서비스 객체 등록
    let service = InputMethodService::new(config, engine_tx.clone(), connection.clone());
    connection
        .object_server()
        .at(INPUT_METHOD_PATH, service)
        .await?;
    unim_log!("DAEMON", "[DBus] 서비스 등록: {}", INPUT_METHOD_PATH);

    // IBus 호환 레이어 시작 (Flatpak/Snap 앱 지원)
    match unim_dbus::ibus_compat::start_ibus_compat(engine_tx, &connection).await {
        Ok(()) => {
            unim_log!("DAEMON", "[IBus Compat] IBus 호환 레이어 활성화됨");
        }
        Err(e) => {
            unim_log!(
                "DAEMON",
                "[IBus Compat] 시작 실패 (비치명적, 네이티브 모듈은 정상 동작): {}",
                e
            );
        }
    }

    Ok(connection)
}

/// 기존에 실행 중인 unim-daemon 프로세스를 찾아 종료합니다.
fn kill_existing_daemon(pid_file: &PathBuf) {
    let my_pid = std::process::id();

    // 1. PID 파일에서 읽은 PID 먼저 시도
    if let Some(pid) = read_pid_file(pid_file) {
        if pid != my_pid && is_process_running(pid) {
            unim_log!(
                "DAEMON",
                "PID 파일에서 기존 데몬 감지 (PID: {}), 종료 시도",
                pid
            );
            let _ = Command::new("kill").arg(pid.to_string()).status();
            std::thread::sleep(Duration::from_millis(200));
        }
    }

    // 2. pgrep으로 모든 unim-daemon 프로세스 PID 조회 (폴백)
    let output = Command::new("pgrep").arg("-f").arg("unim-daemon").output();

    if let Ok(out) = output {
        let pids_str = String::from_utf8_lossy(&out.stdout);
        for pid_line in pids_str.lines() {
            if let Ok(pid) = pid_line.trim().parse::<u32>() {
                // 자기 자신은 제외
                if pid != my_pid {
                    unim_log!(
                        "DAEMON",
                        "기존 unim-daemon 프로세스 종료 시도 (PID: {})",
                        pid
                    );
                    // SIGTERM 전송
                    let _ = Command::new("kill").arg(pid.to_string()).status();
                }
            }
        }
    }
}

/// PID 파일 경로 반환
fn get_pid_file_path() -> PathBuf {
    let run_dir = dirs::runtime_dir().unwrap_or_else(|| PathBuf::from("/tmp"));
    run_dir.join("unim-daemon.pid")
}

/// PID 파일에서 PID 읽기
fn read_pid_file(path: &PathBuf) -> Option<u32> {
    let mut file = fs::File::open(path).ok()?;
    let mut contents = String::new();
    file.read_to_string(&mut contents).ok()?;
    contents.trim().parse().ok()
}

/// PID 파일 쓰기
fn write_pid_file(path: &PathBuf) -> std::io::Result<()> {
    let mut file = fs::File::create(path)?;
    write!(file, "{}", std::process::id())?;
    Ok(())
}

/// PID 파일 삭제
fn remove_pid_file(path: &PathBuf) {
    if path.exists() {
        let _ = fs::remove_file(path);
    }
}

/// 프로세스가 실행 중인지 확인 (kill -0 사용)
fn is_process_running(pid: u32) -> bool {
    // /proc/{pid} 존재 여부로 확인 (Linux 전용)
    PathBuf::from(format!("/proc/{}", pid)).exists()
}

/// 기존 데몬 실행 여부 확인
fn check_existing_daemon(pid_file: &PathBuf) -> Option<u32> {
    if let Some(pid) = read_pid_file(pid_file) {
        if is_process_running(pid) {
            return Some(pid);
        } else {
            // stale PID 파일 정리
            unim_log!("DAEMON", "Stale PID 파일 발견 (PID: {}), 삭제합니다", pid);
            remove_pid_file(pid_file);
        }
    }
    None
}

#[tokio::main]
async fn main() {
    // 로거 초기화
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    // CLI 인수 파싱
    let args = Args::parse();

    // PID 파일 경로
    let pid_file = get_pid_file_path();

    // --check 모드: 기존 데몬 실행 여부만 확인
    if args.check {
        if let Some(pid) = check_existing_daemon(&pid_file) {
            unim_log!("DAEMON", "unim-daemon이 실행 중입니다 (PID: {})", pid);
            std::process::exit(0);
        } else {
            unim_log!("DAEMON", "unim-daemon이 실행 중이지 않습니다");
            std::process::exit(1);
        }
    }

    unim_log!("DAEMON", "UNIM 데몬 시작...");

    // 기존 데몬 확인
    if let Some(existing_pid) = check_existing_daemon(&pid_file) {
        if args.replace {
            unim_log!(
                "DAEMON",
                "기존 데몬 (PID: {}) 강제 종료 후 교체합니다",
                existing_pid
            );
            kill_existing_daemon(&pid_file);
            // 프로세스 종료 대기
            std::thread::sleep(Duration::from_millis(500));
        } else {
            unim_log!("DAEMON",
                "unim-daemon이 이미 실행 중입니다 (PID: {}). --replace 옵션으로 강제 교체할 수 있습니다.",
                existing_pid
            );
            std::process::exit(1);
        }
    }

    // GSettings → config.yaml 1회성 마이그레이션 (v2)
    // 가드 파일(.migrated-v2) 기반으로 첫 기동 시에만 동작.
    // DBus/엔진 초기화 전에 실행해 이관된 값이 엔진에 바로 반영되도록 한다.
    migration::migrate_v2();

    // 설정 로드
    let config = unim::config::Config::load_from_default_path();
    unim_log!("DAEMON", "설정 로드 완료");

    // 데몬화
    if !args.no_daemon {
        match daemonize::Daemonize::new()
            .pid_file(&pid_file)
            .working_directory("/tmp")
            .start()
        {
            Ok(_) => unim_log!("DAEMON", "데몬화 성공"),
            Err(err) => {
                unim_log!("DAEMON", "데몬화 실패: {}", err);
                std::process::exit(1);
            }
        }
    } else {
        // 포그라운드 모드에서는 수동으로 PID 파일 작성
        if let Err(err) = write_pid_file(&pid_file) {
            unim_log!("DAEMON", "PID 파일 작성 실패: {}", err);
        }
    }

    // 종료 플래그
    let running = Arc::new(AtomicBool::new(true));
    let r = running.clone();

    // 종료 시 PID 파일 정리를 위한 클론
    let pid_file_cleanup = pid_file.clone();

    // 엔진 워커 시작
    let engine_tx = spawn_engine_worker(config);
    unim_log!("DAEMON", "엔진 워커 시작됨");

    // DBus 서비스 시작
    let _connection = match start_dbus_service(engine_tx).await {
        Ok(conn) => {
            unim_log!("DAEMON", "[DBus] 서비스 시작 성공");
            conn
        }
        Err(err) => {
            unim_log!("DAEMON", "[DBus] 서비스 시작 실패: {}", err);
            remove_pid_file(&pid_file);
            std::process::exit(1);
        }
    };

    // 메모리 회수 주기 태스크: C 라이브러리(zbus 등) glibc malloc 영역의 여유 청크를
    // OS에 반환한다. jemalloc은 background thread로 자체 회수하므로 여기서는 glibc
    // 영역만 담당. 60초 간격은 장시간 실행 데몬에 충분히 자주다.
    tokio::spawn(async {
        let mut ticker = tokio::time::interval(Duration::from_secs(60));
        ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
        ticker.tick().await; // 첫 tick 즉시 반환 소비
        loop {
            ticker.tick().await;
            #[cfg(all(target_os = "linux", not(target_env = "musl")))]
            // malloc_trim은 glibc 전용 — 다른 libc는 no-op 심볼이 없을 수 있으니 cfg로 제한.
            unsafe {
                libc::malloc_trim(0);
            }
        }
    });

    // Flatpak 앱 IM 환경변수 설정 (GNOME+Wayland 전용)
    configure_flatpak_im_env();

    // 필요한 모듈 감지 및 시작
    let modules = detect_required_modules();

    if modules.is_empty() {
        unim_log!(
            "DAEMON",
            "감지된 디스플레이 서버가 없습니다 (DBus 서비스만 실행)"
        );
    }

    let mut processes: Vec<(String, Child)> = modules
        .iter()
        .filter_map(|module| start_module(module))
        .collect();

    unim_log!(
        "DAEMON",
        "{}개 프론트엔드 실행 중, DBus 서비스 활성",
        processes.len()
    );

    // Ctrl+C 핸들러 (tokio)
    let shutdown_signal = async move {
        tokio::signal::ctrl_c().await.ok();
        unim_log!("DAEMON", "종료 시그널 수신");
        r.store(false, Ordering::SeqCst);
    };

    // 프로세스 모니터링 태스크
    let monitor_task = async {
        loop {
            if !running.load(Ordering::SeqCst) {
                break;
            }

            // 프로세스 상태 확인
            processes.retain_mut(|(name, process)| match process.try_wait() {
                Ok(Some(status)) => {
                    unim_log!("DAEMON", "{} 종료: {}", name, status);
                    false
                }
                Ok(None) => true,
                Err(err) => {
                    unim_log!("DAEMON", "{} 상태 확인 오류: {}", name, err);
                    false
                }
            });

            tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
        }
    };

    // 동시 실행: 종료 시그널 또는 모니터링
    tokio::select! {
        _ = shutdown_signal => {}
        _ = monitor_task => {}
    }

    // 정리 - 남은 프로세스 종료
    for (name, mut process) in processes {
        unim_log!("DAEMON", "{} 종료 중...", name);
        if let Err(err) = process.kill() {
            unim_log!("DAEMON", "{} 종료 실패: {}", name, err);
        }
    }

    // IBus 호환 레이어 정리 (주소 파일 삭제)
    unim_dbus::ibus_compat::stop_ibus_compat();

    // PID 파일 정리
    remove_pid_file(&pid_file_cleanup);

    unim_log!("DAEMON", "UNIM 데몬 종료");
}
