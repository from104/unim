//! UNIM XIM (X Input Method) 프론트엔드
//!
//! X11 환경에서 한글 입력을 제공하는 XIM 서버입니다.

mod handler;

use log::{error, info};
use x11rb::connection::Connection;
use x11rb::protocol::Event;
use xim::x11rb::{HasConnection, X11rbServer};
use xim::XimConnections;

fn main() {
    // 로거 초기화
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    info!("UNIM XIM 서버 시작...");

    // 설정 로드
    let config = unim::config::Config::load_from_default_path();
    info!("설정 로드 완료");

    // X11 연결
    let (conn, screen_num) = match x11rb::rust_connection::RustConnection::connect(None) {
        Ok(result) => result,
        Err(err) => {
            error!("X11 연결 실패: {}", err);
            std::process::exit(1);
        }
    };

    info!("X11 연결 성공 (screen: {})", screen_num);

    // 핸들러 생성
    let mut unim_handler = handler::UnimHandler::new(screen_num, config);

    // XIM 서버 초기화
    let mut server = match X11rbServer::init(conn, screen_num, "unim", xim::ALL_LOCALES) {
        Ok(server) => server,
        Err(err) => {
            error!("XIM 서버 초기화 실패: {:?}", err);
            std::process::exit(1);
        }
    };

    info!("XIM 서버 초기화 완료 (name: unim)");

    let mut connections = XimConnections::new();

    info!("이벤트 루프 시작...");

    // 이벤트 루프
    loop {
        let event = match server.conn().wait_for_event() {
            Ok(event) => event,
            Err(err) => {
                error!("이벤트 대기 오류: {}", err);
                break;
            }
        };

        match server.filter_event(&event, &mut connections, &mut unim_handler) {
            Ok(true) => {
                // 이벤트가 필터링됨 (XIM에서 처리)
            }
            Ok(false) => {
                // XIM에서 처리하지 않은 이벤트
                match event {
                    Event::Expose(e) => {
                        if let Err(err) = unim_handler.expose(e.window, server.conn()) {
                            error!("Expose 처리 오류: {:?}", err);
                        }
                        server.conn().flush().ok();
                    }
                    Event::ConfigureNotify(e) => {
                        unim_handler.configure_notify(&e);
                        server.conn().flush().ok();
                    }
                    Event::DestroyNotify(_) | Event::UnmapNotify(_) | Event::MappingNotify(_) => {
                        // 무시
                    }
                    _ => {
                        log::trace!("처리되지 않은 이벤트: {:?}", event);
                    }
                }
            }
            Err(err) => {
                error!("XIM 이벤트 처리 오류: {:?}", err);
            }
        }
    }

    info!("UNIM XIM 서버 종료");
}
