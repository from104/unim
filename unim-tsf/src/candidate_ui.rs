//! 한자/특수문자 후보 창 — ITfCandidateListUIElement
//!
//! 향후 하이브리드 후보 창 구현을 위한 플레이스홀더.
//! Phase 4-7에서 ITfCandidateListUIElement + 자체 HWND 팝업 구현 예정.

/// 후보 창 상태 (향후 구현 시 사용)
#[allow(dead_code)]
pub struct CandidateUiState {
    pub visible: bool,
    pub candidates: Vec<String>,
    pub selected: usize,
}

#[allow(dead_code)]
impl CandidateUiState {
    pub fn new() -> Self {
        Self {
            visible: false,
            candidates: Vec::new(),
            selected: 0,
        }
    }
}
