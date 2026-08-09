//! 앱 전역 상태 — AppState 와 편집 버퍼 EditorState.

pub mod app_state;
pub mod editor_state;

pub use app_state::{AppState, SharedAppState};
pub use editor_state::EditorState;
// ComboKind 는 Phase D 부터 외부에서 참조한다 — 현재는 모듈 내부 전용.
#[allow(unused_imports)]
pub use editor_state::ComboKind;
