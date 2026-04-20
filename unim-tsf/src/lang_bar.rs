//! 언어 바 아이템 — 한/영 모드 표시 버튼
//!
//! 향후 ITfLangBarItemButton 구현을 위한 플레이스홀더.
//! 현재 TSF 하네스에서는 기본 키 이벤트/조합에 집중합니다.

/// 언어 바 상태 (향후 ITfLangBarItemButton 구현 시 사용)
#[allow(dead_code)]
pub struct LangBarState {
    pub is_korean: bool,
}

#[allow(dead_code)]
impl LangBarState {
    pub fn new() -> Self {
        Self { is_korean: true }
    }

    pub fn set_mode(&mut self, is_korean: bool) {
        self.is_korean = is_korean;
    }
}
