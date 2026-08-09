//! IBus 주소 파일 관리
//!
//! IBus 클라이언트(`im-ibus.so`)는 `~/.config/ibus/bus/` 디렉토리의 주소 파일을 읽어
//! ibus-daemon에 연결한다. UNIM은 session bus 주소를 이 파일에 기록하여
//! `im-ibus.so`가 session bus를 통해 UNIM의 IBus 호환 서비스에 연결하게 한다.
//!
//! Fcitx5도 동일한 방식을 사용한다.

use std::fs;
use std::io;
use std::path::PathBuf;
use unim::unim_log;

/// IBus 주소 파일 경로 생성
///
/// 형식: `~/.config/ibus/bus/<machine-id>-<hostname>-<display>`
pub fn ibus_address_file_path() -> io::Result<PathBuf> {
    let config_dir = dirs_config_dir()?;
    let dir = config_dir.join("ibus").join("bus");

    let machine_id = read_machine_id()?;
    let hostname = get_hostname();
    let display = get_display_id();

    let filename = format!("{}-{}-{}", machine_id, hostname, display);
    Ok(dir.join(filename))
}

/// IBus 주소 파일 작성
///
/// Session bus 주소를 IBus 형식으로 기록한다.
/// `im-ibus.so`가 이 파일을 읽어 session bus에 연결하게 된다.
pub fn write_address_file() -> io::Result<PathBuf> {
    let path = ibus_address_file_path()?;

    // 디렉토리 생성
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }

    let bus_address = get_session_bus_address()?;
    let pid = std::process::id();

    let content = format!(
        "# This file is created by unim-daemon (IBus compat), please do not modify it.\n\
         # Created by UNIM IBus compatibility layer\n\
         IBUS_ADDRESS={}\n\
         IBUS_DAEMON_PID={}\n",
        bus_address, pid
    );

    fs::write(&path, &content)?;

    unim_log!("DAEMON", "[IBus Compat] 주소 파일 생성: {}", path.display());

    Ok(path)
}

/// 주소 파일 내용에서 `IBUS_DAEMON_PID` 값을 파싱한다.
fn parse_address_file_pid(content: &str) -> Option<u32> {
    content.lines().find_map(|line| {
        line.strip_prefix("IBUS_DAEMON_PID=")
            .and_then(|v| v.trim().parse().ok())
    })
}

/// IBus 주소 파일 삭제 — 자기 자신이 쓴 파일일 때만 지운다.
///
/// SIGTERM 종료 경로가 `--replace`/D-Bus 재활성화 시에도 실행되게 되면서,
/// `kill_existing_daemon` 이 구 인스턴스에 SIGTERM 을 보낸 뒤 200~500ms 만
/// 대기하는 사이 새 인스턴스가 먼저 [`write_address_file`] 로 자신의 주소 파일을
/// 써버릴 수 있다. 이 상태에서 구 인스턴스가 조건 없이 파일을 지우면 방금 시작된
/// 새 인스턴스를 ibus 호환 클라이언트가 찾지 못하게 된다. [`remove_own_pid_file`]
/// (unim-daemon/src/main.rs) 과 동일하게, 파일에 기록된 `IBUS_DAEMON_PID` 가
/// 자신의 PID 와 일치할 때만 삭제한다.
pub fn remove_address_file() {
    match ibus_address_file_path() {
        Ok(path) => {
            let content = match fs::read_to_string(&path) {
                Ok(c) => c,
                Err(e) if e.kind() == io::ErrorKind::NotFound => return,
                Err(e) => {
                    unim_log!(
                        "DAEMON",
                        "[IBus Compat] 주소 파일 읽기 실패: {} - {}",
                        path.display(),
                        e
                    );
                    return;
                }
            };

            match parse_address_file_pid(&content) {
                Some(pid) if pid == std::process::id() => {
                    if let Err(e) = fs::remove_file(&path) {
                        unim_log!(
                            "DAEMON",
                            "[IBus Compat] 주소 파일 삭제 실패: {} - {}",
                            path.display(),
                            e
                        );
                    } else {
                        unim_log!("DAEMON", "[IBus Compat] 주소 파일 삭제: {}", path.display());
                    }
                }
                Some(other_pid) => {
                    // 다른(새) 인스턴스가 이미 자신의 주소로 덮어썼다 — 건드리지 않는다.
                    unim_log!(
                        "DAEMON",
                        "[IBus Compat] 주소 파일이 다른 PID({}) 소유 — 삭제 건너뜀: {}",
                        other_pid,
                        path.display()
                    );
                }
                None => {
                    // PID 를 파싱할 수 없음 — 우리가 쓴 흔적을 확인할 수 없으므로
                    // best-effort 로 정리(구 버전 포맷 등 예외적인 경우 대비).
                    if let Err(e) = fs::remove_file(&path) {
                        unim_log!(
                            "DAEMON",
                            "[IBus Compat] 주소 파일 삭제 실패: {} - {}",
                            path.display(),
                            e
                        );
                    } else {
                        unim_log!("DAEMON", "[IBus Compat] 주소 파일 삭제: {}", path.display());
                    }
                }
            }
        }
        Err(e) => {
            unim_log!("DAEMON", "[IBus Compat] 주소 파일 경로 확인 실패: {}", e);
        }
    }
}

/// `/etc/machine-id` 읽기
fn read_machine_id() -> io::Result<String> {
    let content = fs::read_to_string("/etc/machine-id")
        .or_else(|_| fs::read_to_string("/var/lib/dbus/machine-id"))?;
    Ok(content.trim().to_string())
}

/// 호스트명 가져오기 (ibus-daemon 호환: "unix" 고정)
///
/// ibus-daemon은 실제 호스트명 대신 "unix"을 사용한다.
/// `/usr/libexec/ibus-daemon` 소스 참조: `ibus_get_local_machine_id()`
fn get_hostname() -> String {
    "unix".to_string()
}

/// 디스플레이 ID 추출 (ibus-daemon 호환)
///
/// ibus-daemon은 다음 순서로 디스플레이를 결정한다:
/// 1. `WAYLAND_DISPLAY` → 그대로 사용 (예: "wayland-0")
/// 2. `DISPLAY` → `:0.0` → `0` (screen 번호 제거)
/// 3. 폴백: "0"
fn get_display_id() -> String {
    // Wayland: WAYLAND_DISPLAY=wayland-0 → "wayland-0" (전체 사용)
    if let Ok(wayland) = std::env::var("WAYLAND_DISPLAY") {
        if !wayland.is_empty() {
            return wayland;
        }
    }
    // X11: DISPLAY=:0 or :0.0 → "0"
    if let Ok(display) = std::env::var("DISPLAY") {
        let stripped = display.trim_start_matches(':');
        if let Some(dot) = stripped.find('.') {
            return stripped[..dot].to_string();
        }
        return stripped.to_string();
    }
    "0".to_string()
}

/// Session bus 주소 가져오기
fn get_session_bus_address() -> io::Result<String> {
    std::env::var("DBUS_SESSION_BUS_ADDRESS").map_err(|_| {
        io::Error::new(
            io::ErrorKind::NotFound,
            "DBUS_SESSION_BUS_ADDRESS 환경변수 없음",
        )
    })
}

/// `~/.config` 디렉토리 경로
fn dirs_config_dir() -> io::Result<PathBuf> {
    if let Ok(xdg) = std::env::var("XDG_CONFIG_HOME") {
        return Ok(PathBuf::from(xdg));
    }
    if let Ok(home) = std::env::var("HOME") {
        return Ok(PathBuf::from(home).join(".config"));
    }
    Err(io::Error::new(
        io::ErrorKind::NotFound,
        "HOME 환경변수 없음",
    ))
}
