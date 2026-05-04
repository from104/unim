//! UNIM XIM (X Input Method) 프론트엔드
//!
//! X11 환경에서 한국어 입력을 제공하는 XIM 서버입니다.
//! DBus를 통해 unim-daemon과 통신합니다.

mod dbus_client;
mod handler;
mod pe_window;

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use unim::unim_log;
use x11rb::connection::Connection;
use x11rb::protocol::Event;
use xim::x11rb::{HasConnection, X11rbServer};
use xim::XimConnections;

use dbus_client::DbusClient;

/// 디버그 모드 플래그 (UNIM_DEVELOP=1 시 활성화)
pub static DEBUG_ENABLED: AtomicBool = AtomicBool::new(false);

/// UNIM_DEVELOP 환경변수 체크 및 디버그 모드 초기화
fn init_debug_mode() -> bool {
    if let Ok(val) = std::env::var("UNIM_DEVELOP") {
        if val == "1" {
            DEBUG_ENABLED.store(true, Ordering::Relaxed);
            return true;
        }
    }
    false
}

fn main() {
    // 디버그 모드 체크
    let debug_mode = init_debug_mode();

    // 로거 초기화 (디버그 모드 시 debug 레벨, 아니면 info 레벨)
    let default_level = if debug_mode { "debug" } else { "info" };
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or(default_level))
        .init();

    // 디버그 모드 활성화 메시지 (GTK4와 동일한 형식)
    if debug_mode {
        println!("[UNIM-XIM] 디버그 모드 활성화 (UNIM_DEVELOP=1)");
    }

    unim_log!("XIM", "UNIM XIM 서버 시작...");

    // 설정 로드
    let config = unim::config::Config::load_from_default_path();
    unim_log!("XIM", "설정 로드 완료");

    // 팝업 시그널 수신 채널
    let (popup_tx, popup_rx) = std::sync::mpsc::channel::<dbus_client::PopupEvent>();

    // DBus 클라이언트 시작
    let (_dbus_client, dbus_tx) = DbusClient::new(popup_tx);
    unim_log!("XIM", "DBus 클라이언트 시작됨");

    // X11 연결
    let (conn, screen_num) = match x11rb::rust_connection::RustConnection::connect(None) {
        Ok(result) => result,
        Err(err) => {
            unim_log!("XIM", "X11 연결 실패: {}", err);
            std::process::exit(1);
        }
    };

    unim_log!("XIM", "X11 연결 성공 (screen: {})", screen_num);

    // 핸들러 생성 (DBus 채널 전달)
    let mut unim_handler = match handler::UnimHandler::new(screen_num, config, dbus_tx) {
        Ok(handler) => handler,
        Err(err) => {
            unim_log!("XIM", "XIM 핸들러 생성 실패: {}", err);
            std::process::exit(1);
        }
    };

    // XIM 서버 초기화
    let mut server = match X11rbServer::init(conn, screen_num, "unim", xim::ALL_LOCALES) {
        Ok(server) => server,
        Err(err) => {
            unim_log!("XIM", "XIM 서버 초기화 실패: {:?}", err);
            std::process::exit(1);
        }
    };

    unim_log!("XIM", "XIM 서버 초기화 완료 (name: unim)");

    let mut connections = XimConnections::new();

    unim_log!("XIM", "이벤트 루프 시작...");

    // 종료 신호 핸들러 설정
    let running = Arc::new(AtomicBool::new(true));
    let r = running.clone();
    ctrlc::set_handler(move || {
        r.store(false, Ordering::SeqCst);
    })
    .expect("Error setting Ctrl-C handler");

    // 이벤트 루프 (논블로킹 체크를 위해 poll_for_event 고려 가능하나,
    // 여기선 일단 wait_for_event를 유지하되 루프 플래그 확인)
    while running.load(Ordering::SeqCst) {
        // x11rb의 wait_for_event는 블로킹이므로,
        // 실제로는 신호가 와도 다음 이벤트까지 대기할 수 있음.
        // 하지만 종료 시퀀스를 타는 것이 중요.
        // DBus 팝업 시그널 처리 (논블로킹)
        while let Ok(popup_event) = popup_rx.try_recv() {
            if let Err(err) = unim_handler.handle_popup_event(popup_event, &mut server) {
                unim_log!("XIM", "팝업 이벤트 처리 오류: {:?}", err);
            }
            server.conn().flush().ok();
        }

        let event = match server.conn().poll_for_event() {
            Ok(Some(event)) => event,
            Ok(None) => {
                std::thread::sleep(std::time::Duration::from_millis(10));
                continue;
            }
            Err(err) => {
                unim_log!("XIM", "이벤트 대기 오류: {}", err);
                break;
            }
        };

        match server.filter_event(&event, &mut connections, &mut unim_handler) {
            Ok(true) => {
                // 이벤트가 필터링됨 (XIM에서 처리)
                server.conn().flush().ok();
                // AutoTypeFix: BS ForwardEvent 전송 후 교정 텍스트 커밋
                // (GTK3 g_idle_add 패턴 — 현재 이벤트 처리 완료 후 commit)
                unim_handler.process_deferred_autofix(&mut server);
            }
            Ok(false) => {
                // XIM에서 처리하지 않은 이벤트
                match event {
                    Event::Expose(e) => {
                        if let Err(err) = unim_handler.expose(e.window, server.conn()) {
                            unim_log!("XIM", "Expose 처리 오류: {:?}", err);
                        }
                        server.conn().flush().ok();
                    }
                    Event::ConfigureNotify(e) => {
                        unim_handler.configure_notify(&e);
                        server.conn().flush().ok();
                    }
                    Event::ButtonPress(_e) => {
                        // Standalone 모드: 팝업 클릭은 unim-gui-gtk가 처리
                        server.conn().flush().ok();
                    }
                    Event::DestroyNotify(_) | Event::UnmapNotify(_) | Event::MappingNotify(_) => {
                        // 무시
                    }
                    _ => {
                        unim_log!("XIM", "처리되지 않은 이벤트: {:?}", event);
                    }
                }
            }
            Err(err) => {
                unim_log!("XIM", "XIM 이벤트 처리 오류: {:?}", err);
            }
        }
    }

    unim_log!("XIM", "UNIM XIM 서버 종료");
}
