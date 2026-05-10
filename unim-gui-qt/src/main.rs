//! UNIM Qt6 GUI
//!
//! cxx-qt 기반 순수 Cargo 빌드 Qt6 GUI 앱.
//! DBus 시그널을 수신하여 한자/특수문자 팝업을 표시하고,
//! ksni 기반 시스템 트레이 아이콘을 제공합니다.

#![allow(dead_code)]

pub mod bridge;

use std::sync::{Arc, RwLock};
use std::thread;

use cxx_qt_lib::{QGuiApplication, QQmlApplicationEngine, QUrl};
use unim::unim_log;

use unim_gui_common::dbus_client;
use unim_gui_common::tray::TrayController;
use unim_gui_common::types::{GuiAction, IndicatorState, SETTINGS_TX};

// rust-i18n 매크로 — locales 디렉토리의 ko.yml/en.yml 사용
rust_i18n::i18n!("locales", fallback = "en");

/// 시스템 LANG/LC_ALL 환경변수에서 로케일 결정
fn init_locale() {
    let lang = std::env::var("LC_ALL")
        .ok()
        .filter(|s| !s.is_empty())
        .or_else(|| std::env::var("LC_MESSAGES").ok().filter(|s| !s.is_empty()))
        .or_else(|| std::env::var("LANG").ok())
        .unwrap_or_default();
    let locale = if lang.starts_with("ko") { "ko" } else { "en" };
    rust_i18n::set_locale(locale);
}

fn main() {
    init_locale();
    unim_log!("INDICATOR", "UNIM Qt6 GUI 시작...");

    // 상태 초기화
    let state = Arc::new(RwLock::new(IndicatorState::default()));

    // 설정 요청 채널 (트레이 메뉴 "설정" → unim-gui-gtk --settings 서브프로세스로 리다이렉트)
    // Qt GUI는 독자 설정 UI를 유지하지 않고 GTK 설정 앱을 기동한다.
    let (settings_tx, settings_rx) = std::sync::mpsc::channel::<GuiAction>();
    if let Ok(mut tx) = SETTINGS_TX.lock() {
        *tx = Some(settings_tx);
    }

    thread::spawn(move || {
        while let Ok(action) = settings_rx.recv() {
            if matches!(action, GuiAction::OpenSettings) {
                match std::process::Command::new("unim-gui-gtk")
                    .arg("--settings")
                    .spawn()
                {
                    Ok(_child) => {
                        unim_log!("INDICATOR", "unim-gui-gtk --settings 기동");
                    }
                    Err(e) => {
                        unim_log!("INDICATOR", "unim-gui-gtk 실행 실패: {}", e);
                    }
                }
            }
        }
    });

    // 더미 popup_tx (Qt는 트레이에서만 사용)
    let (popup_tx, _popup_rx) = std::sync::mpsc::channel::<GuiAction>();

    // 트레이 → DBus watcher: SetGlobalMode 액션 채널
    let (dbus_action_tx, dbus_action_rx) = tokio::sync::mpsc::channel::<GuiAction>(16);

    // TrayController (Arc로 dbus watcher와 공유)
    let controller = Arc::new(TrayController::new(
        state.clone(),
        popup_tx,
        dbus_action_tx,
    ));

    // 트레이 업데이트 채널
    let (_tray_update_tx, tray_update_rx) = std::sync::mpsc::channel::<()>();
    TrayController::run_update_loop(controller.clone(), tray_update_rx);

    // Qt tray 전용 DBus 태스크 (handle_dbus_actions) + ActiveFrontendsChanged watcher
    let qt_controller = controller.clone();
    thread::spawn(move || {
        let rt = match tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
        {
            Ok(rt) => rt,
            Err(e) => {
                unim_log!("INDICATOR", "Qt tray dbus 런타임 생성 실패: {}", e);
                return;
            }
        };
        rt.block_on(async {
            // DBus 연결
            let connection = match zbus::Connection::session().await {
                Ok(c) => c,
                Err(e) => {
                    unim_log!("INDICATOR", "[Qt] DBus 연결 실패: {}", e);
                    // 연결 실패 시 일단 트레이 시작
                    qt_controller.start();
                    dbus_client::handle_dbus_actions(dbus_action_rx).await;
                    return;
                }
            };

            // 자기 자신 등록 (best-effort)
            if let Ok(proxy) = unim_dbus::client::InputMethodProxy::new(&connection).await {
                if let Err(e) = proxy.register_frontend("unim-gui-qt").await {
                    unim_log!("INDICATOR", "[Qt][RegisterFrontend] 등록 실패 (무시): {}", e);
                } else {
                    unim_log!("INDICATOR", "[Qt][RegisterFrontend] unim-gui-qt 등록됨");
                }
                // 초기 활성 프런트엔드 조회
                let frontends = dbus_client::fetch_active_frontends(&connection).await;
                let has_gnome = frontends.iter().any(|n| n == "gnome-shell");
                unim_log!(
                    "INDICATOR",
                    "[Qt][ActiveFrontends] 초기값: {:?}, gnome-shell={}",
                    frontends,
                    has_gnome
                );
                if !has_gnome {
                    qt_controller.start();
                }
            } else {
                qt_controller.start();
            }

            // ActiveFrontendsChanged 감시 (별도 task)
            let af_controller = qt_controller.clone();
            let af_conn = connection.clone();
            tokio::spawn(async move {
                dbus_client::watch_active_frontends(af_conn, af_controller).await;
            });

            // SetGlobalMode 액션 처리
            dbus_client::handle_dbus_actions(dbus_action_rx).await;
        });
    });

    // Qt 앱 생성
    let mut app = QGuiApplication::new();
    let mut engine = QQmlApplicationEngine::new();

    // QML 로드
    if let Some(engine) = engine.as_mut() {
        engine.load(&QUrl::from(
            "qrc:/qt/qml/io/github/from104/unim/qt/qml/main.qml",
        ));
    }

    // Qt 이벤트 루프 시작
    if let Some(app) = app.as_mut() {
        app.exec();
    }
}
