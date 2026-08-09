//! 헬퍼 모음.
//!
//! - `name_validator` : 이름 충돌·정규식 검사 (Phase B)
//! - `jamo_catalog`   : Cho/Jung/Jong 19/21/28 카탈로그 (Phase C/D)
//!
//! LocalizedText ko/en 추출은 `state::editor_state::localized_lang` 에 인라인.

pub mod jamo_catalog;
pub mod name_validator;
