//! UNIM GUI 통합 프로세스
//!
//! 시스템 트레이 아이콘, 한자/특수문자 팝업, 설정 다이얼로그를 통합 제공합니다.
//! GTK4/libadwaita 기반이며, DBus 시그널을 구독하여 상태를 실시간 반영합니다.

mod emoji_popup;
mod gtk_ui;
mod hanja_popup;
mod popup_positioning;
mod settings_dialog;
mod special_popup;

use std::sync::mpsc;
use std::sync::{Arc, Mutex, RwLock};
use std::thread;
use std::time::Duration;

use ksni::blocking::TrayMethods;
use unim::unim_log;

use unim_gui_common::dbus_client;
use unim_gui_common::tray::UnimTray;
use unim_gui_common::types::{GuiAction, IndicatorState, SETTINGS_TX};

// 설정 다이얼로그·팝업 등 GUI 전용 문자열에 대한 i18n.
// `locales/{ko,en}.yml` 파일이 컴파일 시점에 임베드된다.
rust_i18n::i18n!("locales", fallback = "en");

/// LANG/LC_ALL 환경 변수 기반으로 ko/en을 결정한다 (대소문자 무시).
fn detect_locale() -> &'static str {
    let lang = std::env::var("LANG")
        .or_else(|_| std::env::var("LC_ALL"))
        .unwrap_or_default();
    if lang.to_ascii_lowercase().starts_with("ko") {
        "ko"
    } else {
        "en"
    }
}

fn main() {
    // gtk 크레이트와 common 크레이트는 서로 다른 i18n 인스턴스(static BACKEND)를
    // 가지므로 양쪽 모두에 동일한 로케일을 설정해야 한다.
    unim_gui_common::init_locale();
    rust_i18n::set_locale(detect_locale());

    unim_log!("INDICATOR", "UNIM GUI 시작...");

    // --settings 인자 확인: 설정 다이얼로그만 표시
    let args: Vec<String> = std::env::args().collect();
    if args.iter().any(|a| a == "--settings") {
        gtk_ui::run_settings_only();
        return;
    }

    // 상태 초기화
    let state = Arc::new(RwLock::new(IndicatorState::default()));

    // 채널들
    let (popup_tx, popup_rx) = mpsc::channel::<GuiAction>();
    let popup_rx = Arc::new(Mutex::new(popup_rx));

    // 트레이 메뉴 "설정" 에서 open_settings() 호출 시 GTK 이벤트 루프로 전달
    if let Ok(mut tx) = SETTINGS_TX.lock() {
        *tx = Some(popup_tx.clone());
    }

    // DBus 시그널 감시 스레드 (ksni와 완전 분리)
    let dbus_state = state.clone();
    let dbus_popup_tx = popup_tx.clone();
    // 트레이 업데이트 요청 채널 (DBus -> ksni)
    let (tray_update_tx, tray_update_rx) = std::sync::mpsc::channel::<()>();
    // 트레이 → DBus watcher: SetGlobalMode 액션 채널 (tokio mpsc, async 수신)
    let (dbus_action_tx, dbus_action_rx) = tokio::sync::mpsc::channel::<GuiAction>(16);

    thread::spawn(move || {
        // 별도의 tokio 런타임 생성 (독립 스레드)
        let rt = match tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
        {
            Ok(rt) => rt,
            Err(e) => {
                unim_log!("INDICATOR", "tokio 런타임 생성 실패: {}", e);
                return;
            }
        };

        // handle을 전달하지 않고, 채널만 사용
        rt.block_on(dbus_client::watch_dbus_signals(
            dbus_state,
            tray_update_tx,
            dbus_popup_tx,
            dbus_action_rx,
        ));
    });

    // ksni 트레이 시작 (별도 스레드)
    let tray_state = state.clone();

    thread::spawn(move || {
        let tray = UnimTray {
            state: tray_state,
            popup_tx: popup_tx.clone(),
            dbus_action_tx,
        };
        match tray.spawn() {
            Ok(handle) => {
                unim_log!("INDICATOR", "시스템 트레이 시작됨");
                // 트레이 업데이트 요청 대기 및 처리
                loop {
                    // 100ms 타임아웃으로 채널 폴링
                    match tray_update_rx.recv_timeout(Duration::from_millis(100)) {
                        Ok(()) => {
                            // 트레이 아이콘 업데이트
                            handle.update(|_| {});
                        }
                        Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {
                            // 타임아웃 - 계속 대기
                        }
                        Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => {
                            // 채널 닫힘 - 종료
                            break;
                        }
                    }
                }
            }
            Err(e) => {
                unim_log!("INDICATOR", "시스템 트레이 시작 실패: {}", e);
            }
        }
    });

    // GTK4/libadwaita 앱 시작 (메인 스레드)
    gtk_ui::run_gtk_app(state, popup_rx);
}
