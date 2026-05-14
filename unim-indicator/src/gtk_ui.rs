//! GTK4 메인 루프 호스트 — 트레이 백그라운드를 살아있게 한다.
//!
//! 트레이 메뉴 "설정" 클릭 시 `GuiAction::OpenSettings` 가 수신되며,
//! 별도 `unim-settings` 프로세스를 spawn 한다. (한 책임, 한 프로세스 원칙)

use std::sync::mpsc::Receiver;
use std::time::Duration;

use gtk4::glib;
use gtk4::prelude::*;
use libadwaita as adw;

use unim::unim_log;
use unim_gui_common::types::GuiAction;

/// 트레이 host — GTK 메인 루프 유지 + OpenSettings → unim-settings spawn.
pub fn run_tray_host(popup_rx: Receiver<GuiAction>) {
    let app = adw::Application::builder()
        .application_id("io.github.from104.unim.indicator")
        .flags(gtk4::gio::ApplicationFlags::NON_UNIQUE)
        .build();

    app.connect_activate(move |app| {
        adw::StyleManager::default().set_color_scheme(adw::ColorScheme::Default);

        let _ = app.hold(); // 윈도우 없어도 종료 안 됨 (트레이 백그라운드)
        unim_log!("INDICATOR", "[GUI] GTK Application activated (tray-host mode)");
    });

    // popup_rx 는 한 번만 활성화 — Receiver 를 timeout 클로저로 이동.
    let rx_cell: std::cell::RefCell<Option<Receiver<GuiAction>>> =
        std::cell::RefCell::new(Some(popup_rx));
    glib::timeout_add_local(Duration::from_millis(100), move || {
        if let Some(rx) = rx_cell.borrow().as_ref() {
            while let Ok(action) = rx.try_recv() {
                if let GuiAction::OpenSettings = action {
                    spawn_settings();
                }
                // 그 외 액션은 트레이/DBus watcher가 별도 처리.
            }
        }
        glib::ControlFlow::Continue
    });

    app.run_with_args::<String>(&[]);
}

fn spawn_settings() {
    match std::process::Command::new("unim-settings").spawn() {
        Ok(_) => unim_log!("INDICATOR", "unim-settings spawn"),
        Err(e) => unim_log!("INDICATOR", "unim-settings spawn 실패: {}", e),
    }
}
