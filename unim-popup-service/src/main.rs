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

    // popup_dbus.rs 의 fetch_bookmark_states_async / cancel 등 fire-and-forget
    // 헬퍼들이 응답을 흘려 보낼 채널을 SETTINGS_TX 전역에 등록한다. 미등록 시
    // 헬퍼 첫 줄 `let Some(tx) = tx_opt else { return };` 에서 silent fail →
    // 한자 popup 첫 렌더 북마크 스타일 누락의 근본 원인 (관측 #2040 후속).
    {
        use unim_gui_common::types::SETTINGS_TX;
        if let Ok(mut slot) = SETTINGS_TX.lock() {
            *slot = Some(popup_tx.clone());
        }
    }

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
                // ──────────────────────────────────────────────────────────
                // 1) PopupService 인프라 *먼저* 등록 — daemon 응답에 의존 안 함.
                //
                // 과거 흐름은 register_frontend 를 가장 먼저 await 했는데, autostart
                // 환경에서 daemon NoReply 또는 응답 지연 시 popup_server/bus name/
                // forward task 가 줄줄이 stuck → popup 전체 미동작. 시작 순서를
                // 뒤집어 PopupService 가 즉시 노출되도록 보장한다.
                // ──────────────────────────────────────────────────────────

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

                // daemon InputContext popup signal → PopupServer signal 재발행.
                // 별도 task — watch_dbus_signals (GTK 트리거) 와 병렬 동작.
                let conn_for_forward = conn.clone();
                tokio::spawn(async move {
                    dbus_server::forward_daemon_popup_signals(conn_for_forward).await;
                });

                // ──────────────────────────────────────────────────────────
                // 2) register_frontend 는 fire-and-forget — 응답 안 와도 무관.
                //
                // 단순 best-effort 통지일 뿐이고, PopupService 본업(signal forward
                // + popup window 트리거) 은 이미 위에서 등록 완료. daemon 미준비
                // /응답 지연 시에도 PopupService 가 stuck 하지 않게 분리.
                // ──────────────────────────────────────────────────────────
                let conn_for_reg = conn.clone();
                tokio::spawn(async move {
                    match unim_dbus::client::InputMethodProxy::new(&conn_for_reg).await {
                        Ok(proxy) => match proxy.register_frontend("popup-service").await {
                            Ok(_) => unim_log!("INDICATOR", "[RegisterFrontend] popup-service 등록됨"),
                            Err(e) => unim_log!("INDICATOR", "[RegisterFrontend] 실패: {}", e),
                        },
                        Err(e) => unim_log!("INDICATOR", "[RegisterFrontend] proxy 실패: {}", e),
                    }
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
