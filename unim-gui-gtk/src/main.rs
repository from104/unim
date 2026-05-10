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

use unim::unim_log;

use unim_gui_common::dbus_client;
use unim_gui_common::tray::TrayController;
use unim_gui_common::types::{GuiAction, IndicatorState, SETTINGS_TX};
// unim-dbus proxy (RegisterFrontend + fetch_active_frontends)

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

    // 초기 tray spawn: DBus 연결 전 초기값으로 시작하되,
    // watch_active_frontends가 gnome-shell 감지 시 stop() 호출함.
    // 시작 직후 gnome-shell 여부는 DBus watcher 루프 내 fetch로 판단.
    // TODO: 데몬 재시작 시 RegisterFrontend 재호출 필요 (향후 구현)

    // 트레이 업데이트 루프 (컨트롤러 공유)
    TrayController::run_update_loop(controller.clone(), tray_update_rx);

    // DBus 시그널 감시 스레드 (ksni와 완전 분리)
    let dbus_state = state.clone();
    let dbus_popup_tx = popup_tx.clone();
    let dbus_controller = controller.clone();

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

        rt.block_on(async {
            // 데몬 연결 후 초기 활성 프런트엔드 조회 → 트레이 초기 spawn 결정
            let connection = zbus::Connection::session().await;
            if let Ok(ref conn) = connection {
                // 자기 자신 등록 (best-effort)
                if let Ok(proxy) = unim_dbus::client::InputMethodProxy::new(conn).await {
                    if let Err(e) = proxy.register_frontend("unim-gui-gtk").await {
                        unim_log!("INDICATOR", "[RegisterFrontend] 등록 실패 (무시): {}", e);
                    } else {
                        unim_log!("INDICATOR", "[RegisterFrontend] unim-gui-gtk 등록됨");
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
                    // 프록시 실패 시 일단 트레이 시작
                    dbus_controller.spawn_start();
                }
            } else {
                // 연결 실패 시 일단 트레이 시작 (watch_dbus_signals가 재연결 처리)
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
