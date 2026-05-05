//! 순수 한글 처리 모듈
//!
//! 한글 자모(`jamo`), 음절(`char`), 조합기(`composer`, `composer_with_2bul`,
//! `composer_with_3bul`), 입력 컨텍스트(`input_context`)를 묶는다.
//!
//! 한자(`crate::hanja`), 이모지(`crate::emoji`), 초성 특수문자
//! (`crate::special_chars`)는 이제 별도 최상위 모듈로 분리되었다.

pub mod char;
pub mod composer;
pub mod composer_with_2bul;
pub mod composer_with_3bul;
pub mod composer_with_3bul_moachigi;
pub mod input_context;
pub mod jamo;

// Re-export commonly used items for easier access
pub use char::{HangulChar, HangulCharExt, HangulError};
pub use composer::{HangulComposer, Region};
pub use composer_with_2bul::HangulComposer2Bul;
pub use composer_with_3bul::HangulComposer3Bul;
pub use composer_with_3bul_moachigi::HangulComposer3BulMoachigi;
pub use input_context::{ComposerType, HangulInputContext};
pub use jamo::{Cho, JamoEnum, Jong, Jung};

// --- Additional Aliases for Examples/Compatibility ---
pub use crate::hangul::jamo::Cho as Chosung;
pub use crate::hangul::jamo::Jong as Jongsung;
pub use crate::hangul::jamo::Jung as Jungsung;
