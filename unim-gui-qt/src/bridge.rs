//! cxx-qt 브릿지: Rust DBus ↔ Qt QML
//!
//! DBus 시그널을 수신하여 Qt 시그널로 변환합니다.
//! 팝업은 Qt immodule이 직접 처리하며, 이 모듈은 모드 변경만 전달합니다.

use core::pin::Pin;
use cxx_qt::Threading;
use cxx_qt_lib::QString;
use rust_i18n::t;
use std::sync::{Arc, RwLock};

#[cxx_qt::bridge]
pub mod qobject {
    unsafe extern "C++" {
        include!("cxx-qt-lib/qstring.h");
        type QString = cxx_qt_lib::QString;
    }

    // 백그라운드 스레드에서 Qt 시그널 발행 허용
    impl cxx_qt::Threading for UnimBridge {}

    extern "RustQt" {
        /// UNIM DBus 브릿지 QObject
        #[qobject]
        #[qml_element]
        #[qproperty(bool, is_korean)]
        #[qproperty(bool, connected)]
        #[namespace = "unim"]
        type UnimBridge = super::UnimBridgeRust;

        // ─── 시그널 ───

        /// 입력 모드 변경
        #[qsignal]
        fn mode_changed(self: Pin<&mut Self>, is_korean: bool);

        // ─── i18n 헬퍼 (QML에서 호출) ───

        /// 모드 상태 라벨 (한국어 입력 중 / Typing English)
        #[qinvokable]
        fn mode_status_label(self: &UnimBridge, is_korean: bool) -> QString;

        /// 앱 윈도우 타이틀
        #[qinvokable]
        fn window_title(self: &UnimBridge) -> QString;
    }

    impl cxx_qt::Initialize for UnimBridge {}
}

use unim_gui_common::dbus_client;
use unim_gui_common::types::{GuiAction, IndicatorState};

/// Rust 측 QObject 구조체
pub struct UnimBridgeRust {
    is_korean: bool,
    connected: bool,
}

#[allow(clippy::derivable_impls)]
impl Default for UnimBridgeRust {
    fn default() -> Self {
        Self {
            is_korean: false,
            connected: false,
        }
    }
}

impl qobject::UnimBridge {
    /// 모드 상태 라벨 (한국어 입력 중 / Typing English)
    pub fn mode_status_label(&self, is_korean: bool) -> QString {
        let s = if is_korean {
            t!("modepopup_status_korean")
        } else {
            t!("modepopup_status_english")
        };
        QString::from(&s.to_string())
    }

    /// 앱 윈도우 타이틀
    pub fn window_title(&self) -> QString {
        QString::from(&t!("qt_window_title").to_string())
    }
}

impl cxx_qt::Constructor<()> for qobject::UnimBridge {
    type NewArguments = ();
    type BaseArguments = ();
    type InitializeArguments = ();

    fn route_arguments(
        _args: (),
    ) -> (
        Self::NewArguments,
        Self::BaseArguments,
        Self::InitializeArguments,
    ) {
        ((), (), ())
    }

    fn new((): ()) -> UnimBridgeRust {
        UnimBridgeRust::default()
    }

    /// QObject 초기화 시 DBus 연결 시작
    fn initialize(self: Pin<&mut Self>, _arguments: Self::InitializeArguments) {
        let state = Arc::new(RwLock::new(IndicatorState::default()));
        let (popup_tx, popup_rx) = std::sync::mpsc::channel::<GuiAction>();
        let (tray_update_tx, _tray_update_rx) = std::sync::mpsc::channel::<()>();

        // DBus 시그널 감시 (백그라운드 스레드)
        let dbus_state = state.clone();
        std::thread::spawn(move || {
            let rt = match tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
            {
                Ok(rt) => rt,
                Err(_) => return,
            };
            // bridge.rs 의 DBus 스레드는 시그널 수신 전용.
            // tray SetGlobalMode 는 main.rs 의 handle_dbus_actions 태스크가 처리하므로
            // 여기서는 항상 비어있는 더미 채널을 전달한다.
            let (_dummy_tx, dummy_rx) = tokio::sync::mpsc::channel::<unim_gui_common::types::GuiAction>(1);
            rt.block_on(dbus_client::watch_dbus_signals(
                dbus_state,
                tray_update_tx,
                popup_tx,
                dummy_rx,
            ));
        });

        // GuiAction 수신 → Qt 시그널 발행
        let qt_thread = self.qt_thread();
        std::thread::spawn(move || {
            while let Ok(action) = popup_rx.recv() {
                let qt = qt_thread.clone();
                match action {
                    GuiAction::UpdateCategory(category) => {
                        let is_korean = category == unim::status::InputCategory::Korean;
                        qt.queue(move |mut bridge| {
                            bridge.as_mut().set_is_korean(is_korean);
                            bridge.as_mut().set_connected(true);
                            bridge.as_mut().mode_changed(is_korean);
                        })
                        .ok();
                    }
                    GuiAction::ShowModePopup | GuiAction::OpenSettings | GuiAction::SetGlobalMode(_) => {}
                    // 팝업 액션은 unim-gui-gtk에서만 처리 (Qt GUI에서는 무시)
                    GuiAction::ShowHanjaPopup { .. }
                    | GuiAction::ShowSpecialPopup { .. }
                    | GuiAction::ShowEmojiPopup { .. }
                    | GuiAction::HidePopup
                    | GuiAction::PopupNavigate { .. }
                    | GuiAction::PopupRender { .. }
                    | GuiAction::HanjaBookmarkChanged { .. }
                    | GuiAction::HanjaBookmarkStatesFetched { .. }
                    | GuiAction::HanjaCandidatesReordered { .. } => {}
                }
            }
        });
    }
}
