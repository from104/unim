//! 한자 입력 모듈
//!
//! 한자 사전(`hanja.txt`) 로딩·검색과 사용자 즐겨찾기 저장을 담당한다.

pub mod bookmark;
pub mod dict;

pub use bookmark::HanjaBookmarkStore;
pub use dict::{HanjaDictionary, HanjaEntry};
