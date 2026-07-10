//! GTK4 메인 루프 호스트 — 트레이 백그라운드를 살아있게 한다.
//!
//! 트레이 메뉴 "설정" 클릭 시 `GuiAction::OpenSettings` 가 수신되며,
//! 별도 `unim-settings-gtk` 프로세스를 spawn 한다. (한 책임, 한 프로세스 원칙)

use std::sync::mpsc::Receiver;
use std::time::Duration;

use gtk4::glib;
use gtk4::prelude::*;
use libadwaita as adw;

use unim::unim_log;
use unim_gui_common::types::GuiAction;

/// 트레이 host — GTK 메인 루프 유지 + OpenSettings → unim-settings-gtk spawn.
pub fn run_tray_host(popup_rx: Receiver<GuiAction>) {
    let app = adw::Application::builder()
        .application_id("io.github.from104.unim.Indicator")
        .flags(gtk4::gio::ApplicationFlags::NON_UNIQUE)
        .build();

    app.connect_activate(move |app| {
        adw::StyleManager::default().set_color_scheme(adw::ColorScheme::Default);

        // gio::Application::hold() 는 ApplicationHoldGuard(RAII) 반환.
        // guard 가 drop 되면 즉시 release → hold count 0 → 1초 idle 후 GTK quit.
        // 트레이 호스트는 process lifetime 동안 영구 hold 가 필요하므로 forget.
        std::mem::forget(app.hold());
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
    match std::process::Command::new("unim-settings-gtk").spawn() {
        Ok(_) => unim_log!("INDICATOR", "unim-settings-gtk spawn"),
        Err(e) => unim_log!("INDICATOR", "unim-settings-gtk spawn 실패: {}", e),
    }
}
