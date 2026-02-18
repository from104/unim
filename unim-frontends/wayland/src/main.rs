//! UNIM Wayland 입력 방식 프론트엔드
//!
//! Wayland 환경에서 한국어 입력을 제공합니다.
//! input-method-unstable-v2 프로토콜을 사용합니다.
//! DBus를 통해 unim-daemon과 통신합니다.
//!
//! 지원 컴포지터: KDE(KWin), Sway, Weston, Hyprland 등
//! (zwp_input_method_manager_v2 지원 필요)

mod dbus_client;
mod keymap;
mod state;

use unim::unim_log;
use wayland_client::{globals::registry_queue_init, Connection};
use wayland_protocols_misc::zwp_input_method_v2::client::zwp_input_method_manager_v2::ZwpInputMethodManagerV2;
use wayland_protocols_misc::zwp_virtual_keyboard_v1::client::zwp_virtual_keyboard_manager_v1::ZwpVirtualKeyboardManagerV1;

use dbus_client::DbusClient;
use state::AppState;

fn main() {
    unim_log!("WAYLAND", "UNIM Wayland 입력 방식 시작...");

    // DBus 클라이언트 시작
    let (_dbus_client, dbus_tx) = DbusClient::new();
    unim_log!("WAYLAND", "DBus 클라이언트 시작됨");

    // Wayland 연결
    let conn = match Connection::connect_to_env() {
        Ok(conn) => conn,
        Err(err) => {
            unim_log!("WAYLAND", "Wayland 연결 실패: {}", err);
            std::process::exit(1);
        }
    };
    unim_log!("WAYLAND", "Wayland 연결 성공");

    // 이벤트 큐 및 레지스트리 초기화
    let (globals, mut event_queue) = match registry_queue_init::<AppState>(&conn) {
        Ok(result) => result,
        Err(err) => {
            unim_log!("WAYLAND", "레지스트리 초기화 실패: {}", err);
            std::process::exit(1);
        }
    };

    let qh = event_queue.handle();
    let mut app = AppState::new(dbus_tx);

    // 글로벌 바인딩
    // wl_seat
    match globals.bind::<wayland_client::protocol::wl_seat::WlSeat, _, _>(&qh, 1..=9, ()) {
        Ok(seat) => {
            unim_log!("WAYLAND", "wl_seat 바인딩 성공");
            app.seat = Some(seat);
        }
        Err(err) => {
            unim_log!("WAYLAND", "wl_seat 바인딩 실패: {}", err);
            std::process::exit(1);
        }
    }

    // zwp_input_method_manager_v2
    match globals.bind::<ZwpInputMethodManagerV2, _, _>(&qh, 1..=1, ()) {
        Ok(mgr) => {
            unim_log!("WAYLAND", "zwp_input_method_manager_v2 바인딩 성공");
            app.im_manager = Some(mgr);
        }
        Err(err) => {
            unim_log!(
                "WAYLAND",
                "zwp_input_method_manager_v2 바인딩 실패: {}",
                err
            );
            unim_log!(
                "WAYLAND",
                "컴포지터가 input-method-v2 프로토콜을 지원하지 않습니다"
            );
            std::process::exit(1);
        }
    }

    // zwp_virtual_keyboard_manager_v1 (옵션 - 없어도 동작은 함)
    match globals.bind::<ZwpVirtualKeyboardManagerV1, _, _>(&qh, 1..=1, ()) {
        Ok(mgr) => {
            unim_log!("WAYLAND", "zwp_virtual_keyboard_manager_v1 바인딩 성공");
            app.vk_manager = Some(mgr);
        }
        Err(err) => {
            unim_log!(
                "WAYLAND",
                "zwp_virtual_keyboard_manager_v1 바인딩 실패: {} (키 바이패스 불가)",
                err
            );
        }
    }

    // 프로토콜 오브젝트 셋업 (input_method + grab + virtual_keyboard)
    if !app.setup(&qh) {
        unim_log!("WAYLAND", "입력 방식 셋업 실패");
        std::process::exit(1);
    }

    unim_log!("WAYLAND", "이벤트 루프 시작...");

    // 이벤트 루프
    loop {
        match event_queue.blocking_dispatch(&mut app) {
            Ok(_) => {}
            Err(err) => {
                unim_log!("WAYLAND", "이벤트 디스패치 오류: {}", err);
                break;
            }
        }

        if app.should_exit {
            unim_log!("WAYLAND", "종료 플래그 감지");
            break;
        }
    }

    unim_log!("WAYLAND", "UNIM Wayland 입력 방식 종료");
}
