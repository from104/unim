//! UNIM 키맵 스튜디오 — 한글 자판 보기·편집 전용 GTK4 앱.
//!
//! `unim-typing-practice`와 함께 `unim-keymap-common`의 ProfileSidebar /
//! KeyboardWidget / SerializableProfile을 재사용한다. 본 바이너리는 사이드바와
//! 보기·편집 두 탭만 책임.

mod app;
mod editor_state;
mod tab_edit;
mod tab_view;

use gtk4::prelude::*;
use libadwaita as adw;

rust_i18n::i18n!("locales", fallback = "en");

fn main() {
    // 두 i18n backend(공유 lib + 본 바이너리) 모두에 동일한 로케일을 적용.
    unim_keymap_common::init_locale();
    rust_i18n::set_locale(unim_keymap_common::detect_locale());

    let application = adw::Application::builder()
        .application_id("io.github.from104.unim.KeymapStudio")
        .build();
    application.connect_activate(app::build_window);
    application.run();
}
