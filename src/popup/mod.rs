//! 팝업 상태 관리 모듈
//!
//! 한자/특수문자/이모지 팝업의 공통 로직을 통합하여 단일 진실 소스(Single Source
//! of Truth)를 제공합니다. 프런트엔드(GTK, Qt, XIM, Wayland, GNOME Shell)는 이
//! 모듈의 PopupState 를 사용하여 키 처리와 상태 관리를 수행하고, 렌더링만 각자
//! 담당합니다.

pub(crate) mod popup_keys;
pub(crate) mod popup_layout;
pub(crate) mod popup_state;
pub(crate) mod view_model;

pub use popup_keys::{PopupKey, PopupKeyResult, PopupKind};
pub use popup_state::{EmojiCatMeta, PopupState};
pub use view_model::{CellData, PopupViewModel};
