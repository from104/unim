//! UNIM 타자 연습 — 임의 한글 자판 위에서 WPM/정확도/키 히트맵을 측정.
//!
//! `unim-keymap-studio`(보기·편집)와 함께 `unim-keymap-common`의 위젯·레지스트리를
//! 공유한다. 본 바이너리는 사이드바와 단일 연습 페이지만 책임.

mod active_layout;
mod app;
mod corpus;
mod daemon_check;
mod keyboard_view;
mod practice_engine;
mod practice_page;
mod segmented_progress;
mod triangle_spinner;

use gtk4::prelude::*;
use libadwaita as adw;

rust_i18n::i18n!("locales", fallback = "en");

fn main() {
    unim_keymap_common::init_locale();
    rust_i18n::set_locale(unim_keymap_common::detect_locale());

    let application = adw::Application::builder()
        .application_id("io.github.from104.unim.TypingPractice")
        .build();
    application.connect_activate(app::build_window);
    application.run();
}
