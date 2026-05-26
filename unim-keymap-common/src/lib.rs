//! UNIM 키맵 도구 공통 위젯·상태·직렬화 헬퍼.
//!
//! `unim-keymap-studio` (보기·편집)와 `unim-typing-practice` (타자 연습)가 함께
//! 쓰는 GTK4 위젯·`ProfileRegistry` 래퍼·JSON 저장 헬퍼를 제공한다.
//!
//! GUI 라이브러리이므로 `unim-gui-common`(트레이/DBus)에는 의존하지 않고,
//! 동일한 `init_locale()` 헬퍼만 다시 노출한다 — 두 바이너리에서 한 줄로
//! 로케일을 맞출 수 있게.

rust_i18n::i18n!("locales", fallback = "en");

pub mod keyboard_view;
pub mod keyboard_widget;
pub mod locale;
pub mod serializable;
pub mod state;

pub use keyboard_view::{qwerty_position, KeyboardView};
pub use locale::{detect_locale, init_locale};
pub use serializable::{layout_user_path, save_profile_json, SaveError};
pub use state::SharedRegistry;
