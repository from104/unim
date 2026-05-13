//! UNIM popup service — standalone process.
//!
//! 책임: daemon DBus signal → popup ViewModel → GTK4 popup window + 트레이.
//! X11/Wayland 환경 자동 검출 후 적절한 backend 사용.

#![allow(dead_code)]

mod backend;
mod gtk_ui;
mod popup;
mod single_instance;

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
    // 단일 인스턴스 검증 — 다른 인스턴스가 lock을 잡고 있으면 즉시 종료.
    let _lock = match single_instance::acquire() {
        Some(f) => f,
        None => {
            eprintln!("[unim-popup-service] 이미 다른 인스턴스가 실행 중입니다.");
            return;
        }
    };

    unim_gui_common::init_locale();
    rust_i18n::set_locale(detect_locale());

    let backend_kind = backend::detect();
    unim_log!(
        "INDICATOR",
        "UNIM popup-service 시작 — backend={:?}",
        backend_kind
    );

    // 상태 초기화
    let state = Arc::new(RwLock::new(IndicatorState::default()));

    // 채널들
    let (popup_tx, popup_rx) = mpsc::channel::<GuiAction>();
    let popup_rx = Arc::new(Mutex::new(popup_rx));

    // 트레이 메뉴 "설정" 에서 open_settings() 호출 시 GTK 이벤트 루프로 전달
    if let Ok(mut tx) = SETTINGS_TX.lock() {
        *tx = Some(popup_tx.clone());
    }

    // 트레이 → DBus watcher: SetGlobalMode 액션 채널 (tokio mpsc, async 수신)
    let (dbus_action_tx, dbus_action_rx) = tokio::sync::mpsc::channel::<GuiAction>(16);

    // TrayController 생성 (Arc로 dbus watcher와 공유)
    let controller = Arc::new(TrayController::new(
        state.clone(),
        popup_tx.clone(),
        dbus_action_tx,
    ));

    // 트레이 업데이트 채널 (DBus -> TrayController)
    let (tray_update_tx, tray_update_rx) = std::sync::mpsc::channel::<()>();
    TrayController::run_update_loop(controller.clone(), tray_update_rx);

    // DBus 시그널 감시 스레드 (ksni와 완전 분리)
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
                unim_log!("INDICATOR", "tokio 런타임 생성 실패: {}", e);
                return;
            }
        };

        rt.block_on(async {
            let connection = zbus::Connection::session().await;
            if let Ok(ref conn) = connection {
                // popup-service 자기 자신 등록
                if let Ok(proxy) = unim_dbus::client::InputMethodProxy::new(conn).await {
                    if let Err(e) = proxy.register_frontend("popup-service").await {
                        unim_log!("INDICATOR", "[RegisterFrontend] 등록 실패 (무시): {}", e);
                    } else {
                        unim_log!("INDICATOR", "[RegisterFrontend] popup-service 등록됨");
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
                dbus_controller,
            )
            .await;
        });
    });

    // GTK4/libadwaita 앱 시작 (메인 스레드)
    gtk_ui::run_gtk_app(state, popup_rx);
}
