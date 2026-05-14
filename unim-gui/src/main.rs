//! UNIM GUI — 트레이 인디케이터 + 설정 다이얼로그 (GTK4/libadwaita).
//!
//! popup(한자/특수문자/이모지) 표시는 `unim-popup-service`가 담당하고,
//! 이 바이너리는 시스템 트레이와 설정 GUI를 책임진다.
//!
//! 모드:
//!   - 기본: 트레이 + DBus watcher + GTK 메인 루프 (트레이 메뉴 "설정" → 다이얼로그)
//!   - `--settings`: 다이얼로그만 표시 후 종료

mod gtk_ui;
mod settings_dialog;

use std::sync::mpsc;
use std::sync::{Arc, Mutex, RwLock};
use std::thread;

use unim::unim_log;
use unim_gui_common::dbus_client;
use unim_gui_common::tray::TrayController;
use unim_gui_common::types::{GuiAction, IndicatorState, SETTINGS_TX};

rust_i18n::i18n!("locales", fallback = "en");

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
    unim_gui_common::init_locale();
    rust_i18n::set_locale(detect_locale());

    // --settings: 설정 다이얼로그만 표시 후 종료
    let args: Vec<String> = std::env::args().collect();
    if args.iter().any(|a| a == "--settings") {
        unim_log!("INDICATOR", "UNIM settings 다이얼로그 시작");
        gtk_ui::run_settings_only();
        return;
    }

    unim_log!("INDICATOR", "UNIM GUI 시작 (tray + settings 대기)");

    let state = Arc::new(RwLock::new(IndicatorState::default()));

    // GuiAction 채널 — 트레이 메뉴 "설정" → GTK 이벤트 루프 전달
    let (popup_tx, popup_rx) = mpsc::channel::<GuiAction>();
    let popup_rx = Arc::new(Mutex::new(popup_rx));
    if let Ok(mut tx) = SETTINGS_TX.lock() {
        *tx = Some(popup_tx.clone());
    }

    // tokio mpsc — 트레이 → DBus watcher (SetGlobalMode 등)
    let (dbus_action_tx, dbus_action_rx) = tokio::sync::mpsc::channel::<GuiAction>(16);

    let controller = Arc::new(TrayController::new(
        state.clone(),
        popup_tx.clone(),
        dbus_action_tx,
    ));

    let (tray_update_tx, tray_update_rx) = std::sync::mpsc::channel::<()>();
    TrayController::run_update_loop(controller.clone(), tray_update_rx);

    let dbus_state = state.clone();
    let dbus_popup_tx = popup_tx.clone();
    let dbus_controller = controller.clone();
    thread::spawn(move || {
        let rt = match tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
        {
            Ok(rt) => rt,
            Err(e) => {
                unim_log!("INDICATOR", "tokio 런타임 실패: {}", e);
                return;
            }
        };
        rt.block_on(async {
            let connection = zbus::Connection::session().await;
            if let Ok(ref conn) = connection {
                if let Ok(proxy) = unim_dbus::client::InputMethodProxy::new(conn).await {
                    if let Err(e) = proxy.register_frontend("unim-gui").await {
                        unim_log!("INDICATOR", "[RegisterFrontend] 실패: {}", e);
                    } else {
                        unim_log!("INDICATOR", "[RegisterFrontend] unim-gui 등록됨");
                    }
                    let frontends = dbus_client::fetch_active_frontends(conn).await;
                    let has_gnome = frontends.iter().any(|n| n == "gnome-shell");
                    unim_log!(
                        "INDICATOR",
                        "[ActiveFrontends] 초기값: {:?}, gnome-shell={}",
                        frontends,
                        has_gnome
                    );
                    if !has_gnome {
                        dbus_controller.spawn_start();
                    }
                } else {
                    dbus_controller.spawn_start();
                }
            } else {
                dbus_controller.spawn_start();
            }

            dbus_client::watch_dbus_signals(
                dbus_state,
                tray_update_tx,
                dbus_popup_tx,
                dbus_action_rx,
                Some(dbus_controller),
            )
            .await;
        });
    });

    gtk_ui::run_gui(state, popup_rx);
}
