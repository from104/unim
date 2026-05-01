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
mod popup_renderer;
mod popup_surface;
mod repeat;
mod state;

use std::os::fd::{AsFd, AsRawFd};

use unim::config::{Config as UnimConfig, PopupMode};
use unim::unim_log;
use wayland_client::protocol::wl_compositor::WlCompositor;
use wayland_client::protocol::wl_shm::WlShm;
use wayland_client::{globals::registry_queue_init, Connection};
use wayland_protocols_misc::zwp_input_method_v2::client::zwp_input_method_manager_v2::ZwpInputMethodManagerV2;
use wayland_protocols_misc::zwp_virtual_keyboard_v1::client::zwp_virtual_keyboard_manager_v1::ZwpVirtualKeyboardManagerV1;

use dbus_client::DbusClient;
use state::AppState;

const TOKEN_WAYLAND: mio::Token = mio::Token(0);
const TOKEN_TIMER: mio::Token = mio::Token(1);

fn main() {
    unim_log!("WAYLAND", "UNIM Wayland 입력 방식 시작...");

    // 팝업 이벤트 채널
    let (popup_tx, popup_rx) = std::sync::mpsc::channel::<dbus_client::PopupEvent>();

    // DBus 클라이언트 시작
    let (_dbus_client, dbus_tx) = DbusClient::new(popup_tx);
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
    app.qh = Some(qh.clone());

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

    // wl_compositor (팝업 서피스용)
    match globals.bind::<WlCompositor, _, _>(&qh, 4..=6, ()) {
        Ok(comp) => {
            unim_log!("WAYLAND", "wl_compositor 바인딩 성공");
            app.compositor = Some(comp);
        }
        Err(err) => {
            unim_log!("WAYLAND", "wl_compositor 바인딩 실패: {}", err);
        }
    }

    // wl_shm (팝업 SHM 버퍼용)
    match globals.bind::<WlShm, _, _>(&qh, 1..=1, ()) {
        Ok(shm) => {
            unim_log!("WAYLAND", "wl_shm 바인딩 성공");
            app.shm = Some(shm);
        }
        Err(err) => {
            unim_log!("WAYLAND", "wl_shm 바인딩 실패: {}", err);
        }
    }

    // 프로토콜 오브젝트 셋업 (input_method + grab + virtual_keyboard)
    if !app.setup(&qh) {
        unim_log!("WAYLAND", "입력 방식 셋업 실패");
        std::process::exit(1);
    }

    unim_log!("WAYLAND", "이벤트 루프 시작...");

    // mio Poll 기반 이벤트 루프
    let mut poll = match mio::Poll::new() {
        Ok(p) => p,
        Err(err) => {
            unim_log!("WAYLAND", "epoll 생성 실패: {}", err);
            std::process::exit(1);
        }
    };
    let mut events = mio::Events::with_capacity(8);

    // Wayland fd 등록
    if let Err(err) = poll.registry().register(
        &mut mio::unix::SourceFd(&conn.as_fd().as_raw_fd()),
        TOKEN_WAYLAND,
        mio::Interest::READABLE | mio::Interest::WRITABLE,
    ) {
        unim_log!("WAYLAND", "Wayland fd 등록 실패: {}", err);
        std::process::exit(1);
    }

    // 타이머 fd 등록
    if let Err(err) =
        poll.registry()
            .register(&mut app.repeat_timer, TOKEN_TIMER, mio::Interest::READABLE)
    {
        unim_log!("WAYLAND", "타이머 fd 등록 실패: {}", err);
        std::process::exit(1);
    }

    // 이벤트 루프
    loop {
        // 쓰기 버퍼 플러시
        if let Err(err) = conn.flush() {
            if let wayland_client::backend::WaylandError::Io(ref io_err) = err {
                if io_err.kind() != std::io::ErrorKind::WouldBlock {
                    unim_log!("WAYLAND", "연결 플러시 오류: {}", err);
                    break;
                }
            } else {
                unim_log!("WAYLAND", "연결 플러시 오류: {}", err);
                break;
            }
        }

        // 이벤트 대기
        if let Err(err) = poll.poll(&mut events, None) {
            if err.kind() == std::io::ErrorKind::Interrupted {
                continue;
            }
            unim_log!("WAYLAND", "poll 오류: {}", err);
            break;
        }

        for event in &events {
            match event.token() {
                TOKEN_WAYLAND => {
                    // Wayland 이벤트 읽기
                    if let Some(guard) = conn.prepare_read() {
                        if let Err(err) = guard.read() {
                            if let wayland_client::backend::WaylandError::Io(ref io_err) = err {
                                if io_err.kind() != std::io::ErrorKind::WouldBlock {
                                    unim_log!("WAYLAND", "읽기 오류: {}", err);
                                    break;
                                }
                            } else {
                                unim_log!("WAYLAND", "읽기 오류: {}", err);
                                break;
                            }
                        }
                    }

                    // 대기 중인 이벤트 디스패치
                    if let Err(err) = event_queue.dispatch_pending(&mut app) {
                        unim_log!("WAYLAND", "이벤트 디스패치 오류: {}", err);
                        break;
                    }
                }
                TOKEN_TIMER => {
                    // 키 반복 타이머 만료
                    app.handle_repeat_timer();
                }
                _ => {}
            }
        }

        // 팝업 이벤트 처리 (non-blocking)
        while let Ok(popup_event) = popup_rx.try_recv() {
            match popup_event {
                dbus_client::PopupEvent::ShowHanja {
                    target,
                    candidates,
                    top_row,
                } => {
                    if UnimConfig::load_from_default_path().engine.popup_mode
                        != PopupMode::Standalone
                    {
                        if let (Some(ref shm), Some(ref qh)) = (&app.shm, &app.qh) {
                            app.popup_surface
                                .show_hanja(shm, qh, &target, candidates, &top_row);
                        }
                    }
                }
                dbus_client::PopupEvent::ShowSpecial {
                    target,
                    characters,
                    top_row,
                } => {
                    if UnimConfig::load_from_default_path().engine.popup_mode
                        != PopupMode::Standalone
                    {
                        if let (Some(ref shm), Some(ref qh)) = (&app.shm, &app.qh) {
                            app.popup_surface
                                .show_special(shm, qh, &target, characters, &top_row);
                        }
                    }
                }
                dbus_client::PopupEvent::Hide => {
                    app.popup_surface.hide();
                }
                dbus_client::PopupEvent::AutoTypeFix {
                    delete_chars,
                    commit_text,
                    preedit_text,
                } => {
                    unim_log!(
                        "WAYLAND",
                        "AutoTypeFix 적용: delete={}, commit='{}', preedit='{}'",
                        delete_chars,
                        commit_text,
                        preedit_text
                    );
                    app.apply_auto_typefix(delete_chars, &commit_text, &preedit_text);
                }
            }
        }

        if app.should_exit {
            unim_log!("WAYLAND", "종료 플래그 감지");
            break;
        }
    }

    unim_log!("WAYLAND", "UNIM Wayland 입력 방식 종료");
}
