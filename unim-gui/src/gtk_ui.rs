//! GTK4/libadwaita — 트레이 동반 GTK Application + 설정 다이얼로그.
//!
//! `run_gui` — 트레이 모드: GTK 메인 루프를 유지하면서 `OpenSettings` 액션만
//! 다이얼로그로 띄운다. popup(한자/특수문자/이모지) 처리는 unim-popup-service가 담당.
//! `run_settings_only` — `--settings` 단발 모드.

use std::sync::mpsc::Receiver;
use std::sync::{Arc, Mutex, RwLock};
use std::time::Duration;

use gtk4::glib;
use gtk4::prelude::*;
use libadwaita as adw;

use unim::unim_log;
use unim_gui_common::types::{GuiAction, IndicatorState};

use crate::settings_dialog;

/// 트레이 동반 GTK App — popup은 미처리. OpenSettings만 다이얼로그로.
pub fn run_gui(_state: Arc<RwLock<IndicatorState>>, popup_rx: Arc<Mutex<Receiver<GuiAction>>>) {
    let app = adw::Application::builder()
        .application_id("io.github.from104.unim.gui")
        .flags(gtk4::gio::ApplicationFlags::NON_UNIQUE)
        .build();

    app.connect_activate(move |app| {
        adw::StyleManager::default().set_color_scheme(adw::ColorScheme::Default);

        let popup_rx_clone = popup_rx.clone();
        let app_clone = app.clone();

        glib::timeout_add_local(Duration::from_millis(100), move || {
            if let Ok(rx) = popup_rx_clone.lock() {
                while let Ok(action) = rx.try_recv() {
                    if let GuiAction::OpenSettings = action {
                        settings_dialog::show_settings_dialog(&app_clone);
                    }
                    // 그 외 액션은 트레이/DBus watcher가 별도 처리.
                }
            }
            glib::ControlFlow::Continue
        });

        let _ = app.hold(); // 윈도우 없어도 종료 안 됨 (트레이 백그라운드)
        unim_log!("INDICATOR", "[GUI] GTK Application activated (tray-host mode)");
    });

    app.run_with_args::<String>(&[]);
}

/// `unim-gui --settings` 모드: 다이얼로그만 표시 후 종료.
pub fn run_settings_only() {
    let app = adw::Application::builder()
        .application_id("io.github.from104.unim.settings")
        .build();

    app.connect_activate(|app| {
        adw::StyleManager::default().set_color_scheme(adw::ColorScheme::Default);
        settings_dialog::show_settings_dialog(app);
    });

    app.run_with_args::<String>(&[]);
}
