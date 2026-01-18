//! UNIM 데몬 프로세스
//!
//! 프론트엔드 모듈들을 관리하고 백그라운드에서 실행합니다.

use log::{error, info};
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

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

fn main() {
    // 로거 초기화
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    info!("UNIM 데몬 시작...");

    // 설정 로드
    let _config = unim::config::Config::load_from_default_path();
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

    // 종료 시그널 핸들러
    let running = Arc::new(AtomicBool::new(true));
    let r = running.clone();

    ctrlc::set_handler(move || {
        info!("종료 시그널 수신");
        r.store(false, Ordering::SeqCst);
    })
    .expect("시그널 핸들러 설정 실패");

    // 필요한 모듈 감지 및 시작
    let modules = detect_required_modules();

    if modules.is_empty() {
        error!("감지된 디스플레이 서버가 없습니다");
        std::process::exit(1);
    }

    let mut processes: Vec<(String, Child)> = modules
        .iter()
        .filter_map(|module| start_module(module))
        .collect();

    if processes.is_empty() {
        error!("시작된 프론트엔드가 없습니다");
        std::process::exit(1);
    }

    info!("{}개 프론트엔드 실행 중", processes.len());

    // 메인 루프 - 프로세스 모니터링
    while running.load(Ordering::SeqCst) && !processes.is_empty() {
        processes.retain_mut(|(name, process)| {
            match process.try_wait() {
                Ok(Some(status)) => {
                    info!("{} 종료: {}", name, status);
                    false // 목록에서 제거
                }
                Ok(None) => true, // 계속 실행 중
                Err(err) => {
                    error!("{} 상태 확인 오류: {}", name, err);
                    false
                }
            }
        });

        std::thread::sleep(std::time::Duration::from_secs(1));
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
