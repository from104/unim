//! UNIM 키맵 스튜디오 — 한·영 자판 보기·편집 GTK4 앱.
//!
//! `unim-typing-practice` 의 6세대 UI 디자인을 그대로 차용 — 헤더 좌측에
//! 3중 깊이(언어/출처/자판명) 자판 드롭다운, 우측에 [도움말 ?][UNIM 설정 ⚙]
//! [메뉴 ☰], 본문은 ViewSwitcher 4탭(기본/자판/조합/확장).
//!
//! "조합"·"확장" 탭은 한글 자판일 때만 표시. 빌트인 자판은 자유롭게 만져볼
//! 수 있지만 'Save' 는 disable, 'Save As' 로만 사용자 폴더에 보관 가능.

mod app;
mod dialogs;
mod header;
mod helpers;
mod state;
mod tabs;

use gtk4::prelude::*;
use libadwaita as adw;

rust_i18n::i18n!("locales", fallback = "en");

fn main() {
    // 공유 lib · 본 바이너리의 i18n backend 양쪽에 동일 로케일 적용.
    unim_keymap_common::init_locale();
    rust_i18n::set_locale(unim_keymap_common::detect_locale());

    let application = adw::Application::builder()
        .application_id("io.github.from104.unim.KeymapStudio")
        .build();
    application.connect_activate(app::build_window);
    application.run();
}
