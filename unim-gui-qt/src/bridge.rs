//! cxx-qt 브릿지: Rust DBus ↔ Qt QML
//!
//! DBus 시그널(ShowHanjaPopup 등)을 수신하여 Qt 시그널로 변환합니다.
//! QML UI가 이 시그널을 구독하여 팝업을 표시합니다.

use core::pin::Pin;
use cxx_qt::Threading;
use cxx_qt_lib::QString;
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

        /// 한자 팝업 표시 (candidates_json: [["한","뜻"],…])
        #[qsignal]
        fn show_hanja_popup(
            self: Pin<&mut Self>,
            target: QString,
            candidates_json: QString,
            cursor_x: i32,
            cursor_y: i32,
            cursor_width: i32,
            cursor_height: i32,
        );

        /// 특수문자 팝업 표시 (characters_json: ["★","☆",…])
        #[qsignal]
        fn show_special_popup(
            self: Pin<&mut Self>,
            target: QString,
            characters_json: QString,
            top_row: QString,
            cursor_x: i32,
            cursor_y: i32,
            cursor_width: i32,
            cursor_height: i32,
        );

        /// 팝업 숨김
        #[qsignal]
        fn hide_popup(self: Pin<&mut Self>);

        /// 입력 모드 변경
        #[qsignal]
        fn mode_changed(self: Pin<&mut Self>, is_korean: bool);

        // ─── 인보커블 ───

        /// 한자 선택
        #[qinvokable]
        fn select_hanja(self: Pin<&mut Self>, index: u32);

        /// 한자 취소
        #[qinvokable]
        fn cancel_hanja(self: Pin<&mut Self>);

        /// 특수문자 선택
        #[qinvokable]
        fn select_special_char(self: Pin<&mut Self>, index: u32);

        /// 특수문자 취소
        #[qinvokable]
        fn cancel_special_char(self: Pin<&mut Self>);
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

impl Default for UnimBridgeRust {
    fn default() -> Self {
        Self {
            is_korean: false,
            connected: false,
        }
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
            rt.block_on(dbus_client::watch_dbus_signals(
                dbus_state,
                tray_update_tx,
                popup_tx,
            ));
        });

        // GuiAction 수신 → Qt 시그널 발행
        let qt_thread = self.qt_thread();
        std::thread::spawn(move || {
            loop {
                match popup_rx.recv() {
                    Ok(action) => {
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
                            GuiAction::ShowHanjaPopup {
                                target,
                                candidates,
                                cursor_x,
                                cursor_y,
                                cursor_width,
                                cursor_height,
                            } => {
                                // Vec<(String,String)> → JSON 배열
                                let json = serde_json::to_string(&candidates)
                                    .unwrap_or_else(|_| "[]".into());
                                qt.queue(move |mut bridge| {
                                    bridge.as_mut().show_hanja_popup(
                                        QString::from(&target),
                                        QString::from(&json),
                                        cursor_x,
                                        cursor_y,
                                        cursor_width,
                                        cursor_height,
                                    );
                                })
                                .ok();
                            }
                            GuiAction::ShowSpecialPopup {
                                target,
                                characters,
                                top_row,
                                cursor_x,
                                cursor_y,
                                cursor_width,
                                cursor_height,
                            } => {
                                let json = serde_json::to_string(&characters)
                                    .unwrap_or_else(|_| "[]".into());
                                qt.queue(move |mut bridge| {
                                    bridge.as_mut().show_special_popup(
                                        QString::from(&target),
                                        QString::from(&json),
                                        QString::from(&top_row),
                                        cursor_x,
                                        cursor_y,
                                        cursor_width,
                                        cursor_height,
                                    );
                                })
                                .ok();
                            }
                            GuiAction::HidePopup => {
                                qt.queue(move |mut bridge| {
                                    bridge.as_mut().hide_popup();
                                })
                                .ok();
                            }
                            GuiAction::ShowModePopup | GuiAction::OpenSettings => {}
                        }
                    }
                    Err(_) => break,
                }
            }
        });
    }
}

impl qobject::UnimBridge {
    /// 한자 선택
    pub fn select_hanja(self: Pin<&mut Self>, index: u32) {
        dbus_client::call_context_method("select_hanja", Some(index));
    }

    /// 한자 취소
    pub fn cancel_hanja(self: Pin<&mut Self>) {
        dbus_client::call_context_method("cancel_hanja", None);
    }

    /// 특수문자 선택
    pub fn select_special_char(self: Pin<&mut Self>, index: u32) {
        dbus_client::call_context_method("select_special_char", Some(index));
    }

    /// 특수문자 취소
    pub fn cancel_special_char(self: Pin<&mut Self>) {
        dbus_client::call_context_method("cancel_special_char", None);
    }
}
