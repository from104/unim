//! UNIM popup service — standalone GTK4 popup 전용 프로세스.
//!
//! 책임: daemon DBus signal → popup ViewModel → GTK4 popup window.
//! 트레이는 `unim-indicator`, 설정 GUI는 `unim-settings`가 담당. 본 service는 popup만 다룬다.
//! X11/Wayland 환경 자동 검출 후 적절한 backend 사용.

#![allow(dead_code)]

mod backend;
mod dbus_server;
mod gtk_ui;
mod popup;
mod single_instance;

use std::sync::mpsc;
use std::sync::{Arc, Mutex, RwLock};
use std::thread;

use unim::unim_log;
use unim_gui_common::dbus_client;
use unim_gui_common::types::{GuiAction, IndicatorState};

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

    // 상태 — popup 처리에만 사용 (TrayController 없음)
    let state = Arc::new(RwLock::new(IndicatorState::default()));

    // popup channel — DBus watcher → GTK 메인 루프
    let (popup_tx, popup_rx) = mpsc::channel::<GuiAction>();
    let popup_rx = Arc::new(Mutex::new(popup_rx));

    // dbus_action 채널은 트레이용이지만 popup-service에서는 사용 안 함 (idle)
    let (_dbus_action_tx, dbus_action_rx) = tokio::sync::mpsc::channel::<GuiAction>(1);

    // 더미 tray_update 채널 (controller가 None이므로 어떤 알림도 발행 안 됨)
    let (tray_update_tx, _tray_update_rx) = std::sync::mpsc::channel::<()>();

    // DBus 시그널 감시 스레드 — controller=None으로 트레이 미시작
    let dbus_state = state.clone();
    let dbus_popup_tx = popup_tx.clone();
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
                    if let Err(e) = proxy.register_frontend("popup-service").await {
                        unim_log!("INDICATOR", "[RegisterFrontend] 실패: {}", e);
                    } else {
                        unim_log!("INDICATOR", "[RegisterFrontend] popup-service 등록됨");
                    }
                }

                // org.atit.unim.Popup interface 노출 — Phase 3 method forward 구현.
                // 외부 frontend(GNOME ext) 가 popup 전반을 본 서비스에 의존하도록 단일 SoT.
                let popup_server = dbus_server::PopupServer::new(conn.clone());
                match conn
                    .object_server()
                    .at("/org/atit/unim/popup", popup_server)
                    .await
                {
                    Ok(_) => unim_log!("INDICATOR", "[Popup] /org/atit/unim/popup 등록됨"),
                    Err(e) => unim_log!("INDICATOR", "[Popup] object 등록 실패: {}", e),
                }
                match conn
                    .request_name("org.atit.unim.PopupService")
                    .await
                {
                    Ok(_) => unim_log!("INDICATOR", "[Popup] bus name org.atit.unim.PopupService 획득"),
                    Err(e) => unim_log!("INDICATOR", "[Popup] bus name 획득 실패: {}", e),
                }

                // Phase 2: daemon InputContext popup signal → PopupServer signal 재발행.
                // 별도 task 로 분리하여 watch_dbus_signals(GTK popup window 트리거) 와 병렬 처리.
                let conn_for_forward = conn.clone();
                tokio::spawn(async move {
                    dbus_server::forward_daemon_popup_signals(conn_for_forward).await;
                });
            }

            dbus_client::watch_dbus_signals(
                dbus_state,
                tray_update_tx,
                dbus_popup_tx,
                dbus_action_rx,
                None, // 트레이는 unim-indicator가 담당
            )
            .await;
        });
    });

    // GTK4/libadwaita 앱 시작 (메인 스레드)
    gtk_ui::run_gtk_app(state, popup_rx);
}
