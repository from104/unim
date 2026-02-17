pub mod char;
pub mod composer;
pub mod composer_with_2bul;
pub mod composer_with_3bul;
pub mod hanja;
pub mod input_context;
pub mod jamo;
pub mod special_chars;

// Re-export commonly used items for easier access
pub use char::{HangulChar, HangulCharExt, HangulError};
pub use composer::HangulComposer;
pub use composer_with_2bul::HangulComposer2Bul;
pub use composer_with_3bul::HangulComposer3Bul;
pub use hanja::{HanjaDictionary, HanjaEntry};
pub use input_context::{ComposerType, HangulInputContext};
pub use jamo::{Cho, JamoEnum, Jong, Jung};
pub use special_chars::SpecialCharEntry;

// --- Additional Aliases for Examples/Compatibility ---
pub use crate::hangul::jamo::Cho as Chosung;
pub use crate::hangul::jamo::Jong as Jongsung;
pub use crate::hangul::jamo::Jung as Jungsung;
