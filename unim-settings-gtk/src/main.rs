//! UNIM Settings — 설정 다이얼로그 전용 프로세스.
//!
//! 한 책임, 한 프로세스 원칙:
//!   - GTK4/libadwaita 설정 다이얼로그를 띄우고 종료한다.
//!   - 트레이는 `unim-indicator`, popup은 `unim-popup-service` 별도 프로세스.

mod settings_dialog;

use gtk4::prelude::*;
use libadwaita as adw;

use unim::unim_log;

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
    unim_gui_common::init_locale();
    rust_i18n::set_locale(detect_locale());

    unim_log!("SETTINGS", "UNIM settings 다이얼로그 시작");

    let app = adw::Application::builder()
        .application_id("io.github.from104.unim.Settings")
        .build();

    app.connect_activate(|app| {
        adw::StyleManager::default().set_color_scheme(adw::ColorScheme::Default);
        settings_dialog::show_settings_dialog(app);
    });

    app.run_with_args::<String>(&[]);
}
