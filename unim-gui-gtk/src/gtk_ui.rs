//! GTK4/libadwaita — 설정 다이얼로그 전용 진입점.
//!
//! popup·트레이는 unim-popup-service로 이관됐다. 이 파일은 `run_settings_only`만 노출.

use libadwaita as adw;
use libadwaita::prelude::*;

use crate::settings_dialog;

/// `unim-gui-gtk` 또는 `unim-gui-gtk --settings` 모드: 설정 다이얼로그만 표시.
pub fn run_settings_only() {
    let app = adw::Application::builder()
        .application_id("io.github.from104.unim.settings")
        .build();

    app.connect_activate(|app| {
        // 시스템 테마 자동 추종 (라이트/다크) — portal 기반
        adw::StyleManager::default().set_color_scheme(adw::ColorScheme::Default);
        settings_dialog::show_settings_dialog(app);
    });

    app.run_with_args::<String>(&[]);
}
