//! UNIM 데몬 프로세스
//!
//! DBus 기반 입력 서비스를 제공하고 프론트엔드 모듈들을 관리합니다.

use log::{error, info};
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use tokio::sync::mpsc;
use zbus::Connection;

use unim_dbus::engine_worker::spawn_engine_worker;
use unim_dbus::service::InputMethodService;
use unim_dbus::{BUS_NAME, INPUT_METHOD_PATH};

/// 프론트엔드 모듈 종류
#[derive(Clone, Copy, Debug)]
enum Module {
    Xim,
    Wayland,
}

impl Module {
    fn process_name(&self) -> &'static str {
        match self {
            Module::Xim => "/usr/libexec/unim-xim",
            Module::Wayland => "/usr/libexec/unim-wayland",
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
        info!("X11 환경 감지 - XIM 서버 추가");
    }

    // Wayland 감지 (WAYLAND_DISPLAY 환경 변수)
    if std::env::var("WAYLAND_DISPLAY").is_ok() {
        modules.push(Module::Wayland);
        info!("Wayland 환경 감지 - Wayland IM 추가");
    }

    modules
}

/// 모듈 프로세스를 시작합니다.
fn start_module(module: &Module) -> Option<(String, Child)> {
    let name = module.process_name();

    match Command::new(name)
        .stdin(Stdio::null())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .spawn()
    {
        Ok(child) => {
            info!("{} ({}) 시작됨", module.description(), name);
            Some((name.to_string(), child))
        }
        Err(err) => {
            error!("{} ({}) 시작 실패: {}", module.description(), name, err);
            None
        }
    }
}

/// DBus 서비스를 시작합니다.
async fn start_dbus_service(engine_tx: mpsc::Sender<unim_dbus::service::EngineRequest>) -> zbus::Result<Connection> {
    let config = unim::config::Config::load_from_default_path();
    
    // 세션 버스에 먼저 연결
    let connection = Connection::session().await?;
    
    // 버스 이름 등록
    connection.request_name(BUS_NAME).await?;
    info!("[DBus] 버스 이름 등록: {}", BUS_NAME);
    
    // DBus 서비스 생성 (Connection 전달)
    let service = InputMethodService::new(config, engine_tx, connection.clone());
    
    // 서비스 객체 등록
    connection.object_server().at(INPUT_METHOD_PATH, service).await?;
    info!("[DBus] 서비스 등록: {}", INPUT_METHOD_PATH);
    
    Ok(connection)
}

#[tokio::main]
async fn main() {
    // 로거 초기화
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    info!("UNIM 데몬 시작...");

    // 설정 로드
    let config = unim::config::Config::load_from_default_path();
    info!("설정 로드 완료");

    // 데몬화 옵션 확인
    let no_daemon = std::env::args().any(|a| a == "--no-daemon" || a == "-n");

    if !no_daemon {
        // PID 파일 경로
        let run_dir = dirs::runtime_dir().unwrap_or_else(|| std::path::PathBuf::from("/tmp"));
        let pid_file = run_dir.join("unim.pid");

        match daemonize::Daemonize::new()
            .pid_file(&pid_file)
            .working_directory("/tmp")
            .start()
        {
            Ok(_) => info!("데몬화 성공"),
            Err(err) => {
                error!("데몬화 실패: {}", err);
                std::process::exit(1);
            }
        }
    }

    // 종료 플래그
    let running = Arc::new(AtomicBool::new(true));
    let r = running.clone();

    // 엔진 워커 시작
    let engine_tx = spawn_engine_worker(config);
    info!("엔진 워커 시작됨");

    // DBus 서비스 시작
    let _connection = match start_dbus_service(engine_tx).await {
        Ok(conn) => {
            info!("[DBus] 서비스 시작 성공");
            conn
        }
        Err(err) => {
            error!("[DBus] 서비스 시작 실패: {}", err);
            std::process::exit(1);
        }
    };

    // 필요한 모듈 감지 및 시작
    let modules = detect_required_modules();

    if modules.is_empty() {
        info!("감지된 디스플레이 서버가 없습니다 (DBus 서비스만 실행)");
    }

    let mut processes: Vec<(String, Child)> = modules
        .iter()
        .filter_map(|module| start_module(module))
        .collect();

    info!("{}개 프론트엔드 실행 중, DBus 서비스 활성", processes.len());

    // Ctrl+C 핸들러 (tokio)
    let shutdown_signal = async move {
        tokio::signal::ctrl_c().await.ok();
        info!("종료 시그널 수신");
        r.store(false, Ordering::SeqCst);
    };

    // 프로세스 모니터링 태스크
    let monitor_task = async {
        loop {
            if !running.load(Ordering::SeqCst) {
                break;
            }
            
            // 프로세스 상태 확인
            processes.retain_mut(|(name, process)| {
                match process.try_wait() {
                    Ok(Some(status)) => {
                        info!("{} 종료: {}", name, status);
                        false
                    }
                    Ok(None) => true,
                    Err(err) => {
                        error!("{} 상태 확인 오류: {}", name, err);
                        false
                    }
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
        info!("{} 종료 중...", name);
        if let Err(err) = process.kill() {
            error!("{} 종료 실패: {}", name, err);
        }
    }

    info!("UNIM 데몬 종료");
}
