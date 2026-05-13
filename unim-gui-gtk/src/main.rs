//! UNIM 설정 GUI (GTK4/libadwaita)
//!
//! popup/트레이는 `unim-popup-service`로 이관됐다. 이 바이너리는 **설정 다이얼로그
//! 전용**이다. 트레이 메뉴 "설정" 또는 `unim-gui-gtk --settings` 호출 시 표시된다.

mod gtk_ui;
mod settings_dialog;

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

    unim_log!("INDICATOR", "UNIM settings GUI 시작");

    // --settings 인자 유무와 관계없이 항상 설정 다이얼로그만 표시한다.
    // (이전에는 인자 없으면 popup·트레이 통합 모드였지만 popup-service로 이관됨.)
    gtk_ui::run_settings_only();
}
