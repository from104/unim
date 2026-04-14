//! UNIM Qt6 GUI
//!
//! cxx-qt 기반 순수 Cargo 빌드 Qt6 GUI 앱.
//! DBus 시그널을 수신하여 한자/특수문자 팝업을 표시하고,
//! ksni 기반 시스템 트레이 아이콘을 제공합니다.

#![allow(dead_code)]

pub mod bridge;

use std::sync::{Arc, RwLock};
use std::thread;
use std::time::Duration;

use cxx_qt_lib::{QGuiApplication, QQmlApplicationEngine, QUrl};
use ksni::blocking::TrayMethods;
use unim::unim_log;

use unim_gui_common::tray::UnimTray;
use unim_gui_common::types::{GuiAction, IndicatorState, SETTINGS_TX};

fn main() {
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

    // ksni 트레이 시작 (별도 스레드)
    let tray_state = state.clone();
    let (_tray_update_tx, tray_update_rx) = std::sync::mpsc::channel::<()>();

    // 더미 popup_tx (트레이에서 사용)
    let (popup_tx, _popup_rx) = std::sync::mpsc::channel::<GuiAction>();

    thread::spawn(move || {
        let tray = UnimTray {
            state: tray_state,
            popup_tx,
        };
        match tray.spawn() {
            Ok(handle) => {
                unim_log!("INDICATOR", "시스템 트레이 시작됨 (Qt6)");
                loop {
                    match tray_update_rx.recv_timeout(Duration::from_millis(100)) {
                        Ok(()) => {
                            handle.update(|_| {});
                        }
                        Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {}
                        Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => break,
                    }
                }
            }
            Err(e) => {
                unim_log!("INDICATOR", "시스템 트레이 시작 실패: {}", e);
            }
        }
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
