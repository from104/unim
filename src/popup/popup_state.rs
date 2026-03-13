//! 팝업 상태 및 키 처리 로직
//!
//! 한자/특수문자 팝업의 모든 상태 관리와 키보드/마우스 입력 처리를 담당합니다.

/// 팝업 종류
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PopupKind {
    /// 한자 후보 팝업 (단일 열, 9개/페이지)
    Hanja,
    /// 특수문자 팝업 (9×9 그리드, 81개/페이지)
    SpecialChar,
}

/// 툴킷 중립 키 열거형
///
/// 각 프런트엔드가 자체 키코드(GDK, Qt, X11 keysym 등)를
/// 이 열거형으로 변환하여 PopupState에 전달합니다.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PopupKey {
    /// 숫자 1-9 (1-based)
    Number(u8),
    /// QWERTYUIO 물리 위치 (0-based: Q=0, W=1, ..., O=8)
    Letter(u8),
    Up,
    Down,
    Left,
    Right,
    Enter,
    Escape,
    Tab,
    ShiftTab,
    PageUp,
    PageDown,
    Space,
    Backspace,
    /// 모디파이어 키 (Shift, Ctrl, Alt 등) — 무시
    Modifier,
    /// 알 수 없는 키 — 팝업 닫고 통과
    Other,
}

/// 키 처리 결과
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PopupKeyResult {
    /// 선택 확정 (전체 인덱스, 0-based)
    Select(usize),
    /// 취소 (Escape)
    Cancel,
    /// 상태 변경됨 → 재렌더링 필요
    Updated,
    /// 키 소비됨, 화면 변경 없음
    Consumed,
    /// 미처리 → 팝업 닫고 키 통과
    NotHandled,
}

/// 특수문자 그리드 상수
const MAX_ROWS: usize = 9;
const MAX_COLS: usize = 9;
const SPECIAL_PAGE_SIZE: usize = MAX_ROWS * MAX_COLS; // 81

/// 한자 페이지 크기
const HANJA_PAGE_SIZE: usize = 9;

/// 팝업 상태 (단일 진실 소스)
#[derive(Debug, Clone)]
pub struct PopupState {
    kind: PopupKind,
    /// 전체 문자 목록 (한자 또는 특수문자)
    items: Vec<String>,
    /// 한자 뜻 (한자 팝업에서만 사용, 특수문자는 빈 벡터)
    meanings: Vec<String>,
    /// 대상 글자 (초성 또는 한글)
    target: String,
    /// 상단 행 레이블 (특수문자: "QWERTYUIO" 등)
    top_row: String,
    /// 현재 페이지 (0-based)
    current_page: usize,
    /// 전체 페이지 수
    total_pages: usize,
    /// 선택 행 (특수문자: 0..rows-1, 한자: 0..page_items-1)
    sel_row: usize,
    /// 선택 열 (한자는 항상 0)
    sel_col: usize,
    /// 현재 페이지 행 수
    rows: usize,
    /// 현재 페이지 열 수
    cols: usize,
    /// 페이지당 최대 항목 수
    page_size: usize,
}

impl PopupState {
    /// 한자 팝업 생성
    ///
    /// # Arguments
    /// * `target` - 변환 대상 한글 문자열
    /// * `candidates` - (한자, 뜻) 쌍의 벡터
    pub fn new_hanja(target: &str, candidates: Vec<(String, String)>) -> Self {
        let total = candidates.len();
        let items: Vec<String> = candidates.iter().map(|(h, _)| h.clone()).collect();
        let meanings: Vec<String> = candidates.iter().map(|(_, m)| m.clone()).collect();
        let total_pages = if total == 0 {
            1
        } else {
            (total + HANJA_PAGE_SIZE - 1) / HANJA_PAGE_SIZE
        };
        let page_items = total.min(HANJA_PAGE_SIZE);

        Self {
            kind: PopupKind::Hanja,
            items,
            meanings,
            target: target.to_string(),
            top_row: String::new(),
            current_page: 0,
            total_pages,
            sel_row: 0,
            sel_col: 0,
            rows: page_items,
            cols: 1,
            page_size: HANJA_PAGE_SIZE,
        }
    }

    /// 특수문자 팝업 생성
    ///
    /// # Arguments
    /// * `target` - 대상 초성
    /// * `characters` - 특수문자 벡터
    /// * `top_row` - 상단 키 레이블 (예: "QWERTYUIO")
    pub fn new_special(target: &str, characters: Vec<String>, top_row: &str) -> Self {
        let total = characters.len();
        let total_pages = if total == 0 {
            1
        } else {
            (total + SPECIAL_PAGE_SIZE - 1) / SPECIAL_PAGE_SIZE
        };

        let mut state = Self {
            kind: PopupKind::SpecialChar,
            items: characters,
            meanings: Vec::new(),
            target: target.to_string(),
            top_row: top_row.to_string(),
            current_page: 0,
            total_pages,
            sel_row: 0,
            sel_col: 0,
            rows: 0,
            cols: 0,
            page_size: SPECIAL_PAGE_SIZE,
        };
        state.update_page_layout();
        state
    }

    /// 현재 페이지 레이아웃 재계산 (특수문자 전용)
    fn update_page_layout(&mut self) {
        match self.kind {
            PopupKind::SpecialChar => {
                let page_chars = self.page_item_count();
                self.cols = if page_chars == 0 {
                    1
                } else {
                    ((page_chars + MAX_ROWS - 1) / MAX_ROWS)
                        .min(MAX_COLS)
                        .max(1)
                };
                self.rows = if page_chars == 0 {
                    1
                } else {
                    ((page_chars + self.cols - 1) / self.cols)
                        .min(MAX_ROWS)
                        .max(1)
                };
            }
            PopupKind::Hanja => {
                let page_items = self.page_item_count();
                self.rows = page_items;
                self.cols = 1;
            }
        }
    }

    /// 현재 페이지의 항목 수
    fn page_item_count(&self) -> usize {
        let page_start = self.current_page * self.page_size;
        if page_start >= self.items.len() {
            0
        } else {
            (self.items.len() - page_start).min(self.page_size)
        }
    }

    /// 특수문자 그리드에서 (row, col) → 전체 인덱스 (열 우선 채움)
    fn special_global_index(&self, row: usize, col: usize) -> Option<usize> {
        let page_start = self.current_page * self.page_size;
        let idx = col * self.rows + row;
        let global = page_start + idx;
        if global < self.items.len() {
            Some(global)
        } else {
            None
        }
    }

    /// 한자 목록에서 페이지 내 인덱스 → 전체 인덱스
    fn hanja_global_index(&self, page_index: usize) -> Option<usize> {
        let global = self.current_page * self.page_size + page_index;
        if global < self.items.len() {
            Some(global)
        } else {
            None
        }
    }

    /// 특수문자 그리드에서 해당 셀에 문자가 있는지 확인
    fn cell_exists(&self, row: usize, col: usize) -> bool {
        self.special_global_index(row, col).is_some()
    }

    /// 현재 선택된 항목의 전체 인덱스
    pub fn selected_global_index(&self) -> Option<usize> {
        match self.kind {
            PopupKind::SpecialChar => self.special_global_index(self.sel_row, self.sel_col),
            PopupKind::Hanja => self.hanja_global_index(self.sel_row),
        }
    }

    /// 키 처리
    pub fn handle_key(&mut self, key: PopupKey) -> PopupKeyResult {
        match self.kind {
            PopupKind::SpecialChar => self.handle_special_key(key),
            PopupKind::Hanja => self.handle_hanja_key(key),
        }
    }

    /// 마우스 클릭 처리
    ///
    /// # Arguments
    /// * `row` - 클릭한 행 (0-based)
    /// * `col` - 클릭한 열 (0-based, 한자는 항상 0)
    pub fn handle_click(&mut self, row: usize, col: usize) -> PopupKeyResult {
        match self.kind {
            PopupKind::SpecialChar => {
                if row < self.rows && col < self.cols && self.cell_exists(row, col) {
                    self.sel_row = row;
                    self.sel_col = col;
                    if let Some(idx) = self.selected_global_index() {
                        PopupKeyResult::Select(idx)
                    } else {
                        PopupKeyResult::Consumed
                    }
                } else {
                    PopupKeyResult::Consumed
                }
            }
            PopupKind::Hanja => {
                let page_items = self.page_item_count();
                if row < page_items {
                    self.sel_row = row;
                    if let Some(idx) = self.selected_global_index() {
                        PopupKeyResult::Select(idx)
                    } else {
                        PopupKeyResult::Consumed
                    }
                } else {
                    PopupKeyResult::Consumed
                }
            }
        }
    }

    // --- 특수문자 키 처리 ---

    fn handle_special_key(&mut self, key: PopupKey) -> PopupKeyResult {
        match key {
            PopupKey::Escape => PopupKeyResult::Cancel,

            PopupKey::Letter(col_idx) => {
                let col = col_idx as usize;
                if col < self.cols {
                    self.sel_col = col;
                    // 현재 행이 유효하지 않으면 마지막 유효 행으로 이동
                    if !self.cell_exists(self.sel_row, self.sel_col) {
                        for r in (0..self.rows).rev() {
                            if self.cell_exists(r, self.sel_col) {
                                self.sel_row = r;
                                break;
                            }
                        }
                    }
                }
                PopupKeyResult::Updated
            }

            PopupKey::Number(n) => {
                let row_idx = (n - 1) as usize;
                if row_idx < self.rows && self.cell_exists(row_idx, self.sel_col) {
                    self.sel_row = row_idx;
                    if let Some(idx) = self.selected_global_index() {
                        return PopupKeyResult::Select(idx);
                    }
                }
                PopupKeyResult::Consumed
            }

            PopupKey::Enter => {
                if let Some(idx) = self.selected_global_index() {
                    PopupKeyResult::Select(idx)
                } else {
                    PopupKeyResult::Consumed
                }
            }

            PopupKey::Up => {
                if self.sel_row > 0 {
                    self.sel_row -= 1;
                } else {
                    self.sel_row = self.rows - 1;
                }
                // 유효한 셀로 조정
                while self.sel_row > 0 && !self.cell_exists(self.sel_row, self.sel_col) {
                    self.sel_row -= 1;
                }
                PopupKeyResult::Updated
            }

            PopupKey::Down => {
                if self.sel_row + 1 < self.rows {
                    self.sel_row += 1;
                } else {
                    self.sel_row = 0;
                }
                if !self.cell_exists(self.sel_row, self.sel_col) {
                    self.sel_row = 0;
                }
                PopupKeyResult::Updated
            }

            PopupKey::Left => {
                if self.sel_col > 0 {
                    self.sel_col -= 1;
                } else {
                    self.sel_col = self.cols - 1;
                }
                if !self.cell_exists(self.sel_row, self.sel_col) {
                    self.sel_row = 0;
                }
                PopupKeyResult::Updated
            }

            PopupKey::Right => {
                if self.sel_col + 1 < self.cols {
                    self.sel_col += 1;
                } else {
                    self.sel_col = 0;
                }
                if !self.cell_exists(self.sel_row, self.sel_col) {
                    self.sel_row = 0;
                }
                PopupKeyResult::Updated
            }

            PopupKey::Tab | PopupKey::PageDown | PopupKey::Space => {
                if self.total_pages > 1 {
                    self.current_page = (self.current_page + 1) % self.total_pages;
                    self.update_page_layout();
                    self.sel_row = 0;
                    self.sel_col = 0;
                }
                PopupKeyResult::Updated
            }

            PopupKey::ShiftTab | PopupKey::PageUp => {
                if self.total_pages > 1 {
                    self.current_page = if self.current_page > 0 {
                        self.current_page - 1
                    } else {
                        self.total_pages - 1
                    };
                    self.update_page_layout();
                    self.sel_row = 0;
                    self.sel_col = 0;
                }
                PopupKeyResult::Updated
            }

            PopupKey::Modifier => PopupKeyResult::Consumed,
            PopupKey::Other | PopupKey::Backspace => PopupKeyResult::NotHandled,
        }
    }

    // --- 한자 키 처리 ---

    fn handle_hanja_key(&mut self, key: PopupKey) -> PopupKeyResult {
        match key {
            PopupKey::Escape => PopupKeyResult::Cancel,

            PopupKey::Number(n) => {
                let idx = (n - 1) as usize;
                let page_items = self.page_item_count();
                if idx < page_items {
                    if let Some(global) = self.hanja_global_index(idx) {
                        return PopupKeyResult::Select(global);
                    }
                }
                PopupKeyResult::Consumed
            }

            PopupKey::Enter => {
                let page_items = self.page_item_count();
                if page_items > 0 && self.sel_row < page_items {
                    if let Some(global) = self.hanja_global_index(self.sel_row) {
                        return PopupKeyResult::Select(global);
                    }
                }
                PopupKeyResult::Consumed
            }

            PopupKey::Down => {
                let count = self.page_item_count();
                if count > 0 {
                    self.sel_row = (self.sel_row + 1) % count;
                }
                PopupKeyResult::Updated
            }

            PopupKey::Up => {
                let count = self.page_item_count();
                if count > 0 {
                    if self.sel_row == 0 {
                        self.sel_row = count - 1;
                    } else {
                        self.sel_row -= 1;
                    }
                }
                PopupKeyResult::Updated
            }

            PopupKey::Right | PopupKey::Space | PopupKey::Tab | PopupKey::PageDown => {
                if self.total_pages > 1 {
                    if self.current_page + 1 < self.total_pages {
                        self.current_page += 1;
                    } else {
                        self.current_page = 0;
                    }
                    self.update_page_layout();
                    self.sel_row = 0;
                }
                PopupKeyResult::Updated
            }

            PopupKey::Left | PopupKey::Backspace | PopupKey::ShiftTab | PopupKey::PageUp => {
                if self.total_pages > 1 {
                    if self.current_page > 0 {
                        self.current_page -= 1;
                    } else {
                        self.current_page = self.total_pages - 1;
                    }
                    self.update_page_layout();
                    self.sel_row = 0;
                }
                PopupKeyResult::Updated
            }

            PopupKey::Modifier => PopupKeyResult::Consumed,
            PopupKey::Letter(_) | PopupKey::Other => PopupKeyResult::NotHandled,
        }
    }

    // --- 접근자 ---

    /// 팝업 종류
    pub fn kind(&self) -> PopupKind {
        self.kind
    }

    /// 대상 문자열
    pub fn target(&self) -> &str {
        &self.target
    }

    /// 상단 행 레이블
    pub fn top_row(&self) -> &str {
        &self.top_row
    }

    /// 현재 페이지 (0-based)
    pub fn current_page(&self) -> usize {
        self.current_page
    }

    /// 전체 페이지 수
    pub fn total_pages(&self) -> usize {
        self.total_pages
    }

    /// 선택 행
    pub fn sel_row(&self) -> usize {
        self.sel_row
    }

    /// 선택 열
    pub fn sel_col(&self) -> usize {
        self.sel_col
    }

    /// 현재 페이지 행 수
    pub fn rows(&self) -> usize {
        self.rows
    }

    /// 현재 페이지 열 수
    pub fn cols(&self) -> usize {
        self.cols
    }

    /// 전체 항목 수
    pub fn total_items(&self) -> usize {
        self.items.len()
    }

    /// 전체 인덱스로 항목 텍스트 가져오기
    pub fn get_item(&self, index: usize) -> Option<&str> {
        self.items.get(index).map(|s| s.as_str())
    }

    /// 전체 인덱스로 뜻 가져오기 (한자 전용)
    pub fn get_meaning(&self, index: usize) -> Option<&str> {
        self.meanings.get(index).map(|s| s.as_str())
    }

    /// 특수문자 그리드에서 (row, col) 위치의 텍스트
    pub fn cell_text(&self, row: usize, col: usize) -> Option<&str> {
        match self.kind {
            PopupKind::SpecialChar => {
                let global = self.special_global_index(row, col)?;
                self.items.get(global).map(|s| s.as_str())
            }
            PopupKind::Hanja => {
                if col != 0 {
                    return None;
                }
                let global = self.hanja_global_index(row)?;
                self.items.get(global).map(|s| s.as_str())
            }
        }
    }

    /// 한자 페이지 항목 (한자, 뜻) 슬라이스
    pub fn hanja_page_items(&self) -> Vec<(&str, &str)> {
        let start = self.current_page * self.page_size;
        let end = (start + self.page_size).min(self.items.len());
        if start >= self.items.len() {
            Vec::new()
        } else {
            self.items[start..end]
                .iter()
                .zip(self.meanings[start..end].iter())
                .map(|(h, m)| (h.as_str(), m.as_str()))
                .collect()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // --- 특수문자 팝업 테스트 ---

    fn make_special(n: usize) -> PopupState {
        let chars: Vec<String> = (0..n).map(|i| format!("C{}", i)).collect();
        PopupState::new_special("ㄱ", chars, "QWERTYUIO")
    }

    #[test]
    fn special_page_layout_full() {
        let state = make_special(81);
        assert_eq!(state.rows, 9);
        assert_eq!(state.cols, 9);
        assert_eq!(state.total_pages, 1);
    }

    #[test]
    fn special_page_layout_partial() {
        // 20 chars → cols = ceil(20/9) = 3, rows = ceil(20/3) = 7
        let state = make_special(20);
        assert_eq!(state.cols, 3);
        assert_eq!(state.rows, 7);
        assert_eq!(state.total_pages, 1);
    }

    #[test]
    fn special_page_layout_multi_page() {
        // 100 chars → 2 pages
        let state = make_special(100);
        assert_eq!(state.total_pages, 2);
        assert_eq!(state.rows, 9);
        assert_eq!(state.cols, 9);
    }

    #[test]
    fn special_column_first_index() {
        // 열 우선 채움 검증: (row=0, col=0)=0, (row=1, col=0)=1, ..., (row=0, col=1)=rows
        let state = make_special(81);
        assert_eq!(state.special_global_index(0, 0), Some(0));
        assert_eq!(state.special_global_index(1, 0), Some(1));
        assert_eq!(state.special_global_index(8, 0), Some(8));
        assert_eq!(state.special_global_index(0, 1), Some(9));
        assert_eq!(state.special_global_index(8, 8), Some(80));
    }

    #[test]
    fn special_number_key_selects() {
        let mut state = make_special(81);
        // 숫자 1 → row 0, 즉시 선택
        let result = state.handle_key(PopupKey::Number(1));
        assert_eq!(result, PopupKeyResult::Select(0));
    }

    #[test]
    fn special_number_key_in_column() {
        let mut state = make_special(81);
        // 열 2로 이동 후 숫자 3 → (row=2, col=2) = 2*9+2 = 20
        state.handle_key(PopupKey::Right);
        state.handle_key(PopupKey::Right);
        let result = state.handle_key(PopupKey::Number(3));
        assert_eq!(result, PopupKeyResult::Select(20));
    }

    #[test]
    fn special_letter_jump_column() {
        let mut state = make_special(81);
        // 'E' = Letter(2) → col 2
        state.handle_key(PopupKey::Letter(2));
        assert_eq!(state.sel_col, 2);
        assert_eq!(state.sel_row, 0);
    }

    #[test]
    fn special_letter_invalid_cell_fallback() {
        // 20 chars: cols=3, rows=7. col 2의 마지막 유효 행은?
        // col2: indices 14..19 → 6개 (rows 0-5)
        let mut state = make_special(20);
        state.sel_row = 6; // row 6
        state.handle_key(PopupKey::Letter(2)); // col 2
        // col 2에서 row 6은 idx=2*7+6=20, 범위 밖 → 마지막 유효 행으로
        assert!(state.cell_exists(state.sel_row, state.sel_col));
    }

    #[test]
    fn special_arrow_up_wrap() {
        let mut state = make_special(81);
        assert_eq!(state.sel_row, 0);
        state.handle_key(PopupKey::Up);
        assert_eq!(state.sel_row, 8); // 래핑
    }

    #[test]
    fn special_arrow_down_wrap() {
        let mut state = make_special(81);
        state.sel_row = 8;
        state.handle_key(PopupKey::Down);
        assert_eq!(state.sel_row, 0); // 래핑
    }

    #[test]
    fn special_arrow_left_wrap() {
        let mut state = make_special(81);
        assert_eq!(state.sel_col, 0);
        state.handle_key(PopupKey::Left);
        assert_eq!(state.sel_col, 8); // 래핑
    }

    #[test]
    fn special_arrow_right_wrap() {
        let mut state = make_special(81);
        state.sel_col = 8;
        state.handle_key(PopupKey::Right);
        assert_eq!(state.sel_col, 0); // 래핑
    }

    #[test]
    fn special_arrow_down_invalid_cell_reset() {
        // 20 chars: cols=3, rows=7, col 2 has 6 items (row 0-5)
        let mut state = make_special(20);
        state.sel_col = 2;
        state.sel_row = 5; // 마지막 유효 행
        state.handle_key(PopupKey::Down);
        // row 6은 유효하지 않으므로 row 0으로 리셋
        assert_eq!(state.sel_row, 0);
    }

    #[test]
    fn special_tab_next_page() {
        let mut state = make_special(100);
        assert_eq!(state.current_page, 0);
        state.handle_key(PopupKey::Tab);
        assert_eq!(state.current_page, 1);
        assert_eq!(state.sel_row, 0);
        assert_eq!(state.sel_col, 0);
    }

    #[test]
    fn special_tab_wrap_page() {
        let mut state = make_special(100);
        state.current_page = 1; // 마지막 페이지
        state.handle_key(PopupKey::Tab);
        assert_eq!(state.current_page, 0); // 래핑
    }

    #[test]
    fn special_shift_tab_prev_page() {
        let mut state = make_special(100);
        state.current_page = 1;
        state.update_page_layout();
        state.handle_key(PopupKey::ShiftTab);
        assert_eq!(state.current_page, 0);
    }

    #[test]
    fn special_shift_tab_wrap_page() {
        let mut state = make_special(100);
        assert_eq!(state.current_page, 0);
        state.handle_key(PopupKey::ShiftTab);
        assert_eq!(state.current_page, 1); // 래핑
    }

    #[test]
    fn special_enter_select() {
        let mut state = make_special(81);
        state.sel_row = 3;
        state.sel_col = 2;
        let result = state.handle_key(PopupKey::Enter);
        // (row=3, col=2) → 2*9+3 = 21
        assert_eq!(result, PopupKeyResult::Select(21));
    }

    #[test]
    fn special_escape_cancel() {
        let mut state = make_special(81);
        let result = state.handle_key(PopupKey::Escape);
        assert_eq!(result, PopupKeyResult::Cancel);
    }

    #[test]
    fn special_other_key_not_handled() {
        let mut state = make_special(81);
        let result = state.handle_key(PopupKey::Other);
        assert_eq!(result, PopupKeyResult::NotHandled);
    }

    #[test]
    fn special_modifier_consumed() {
        let mut state = make_special(81);
        let result = state.handle_key(PopupKey::Modifier);
        assert_eq!(result, PopupKeyResult::Consumed);
    }

    #[test]
    fn special_click_select() {
        let mut state = make_special(81);
        let result = state.handle_click(2, 3);
        // (row=2, col=3) → 3*9+2 = 29
        assert_eq!(result, PopupKeyResult::Select(29));
    }

    #[test]
    fn special_click_invalid() {
        let mut state = make_special(20);
        // (row=8, col=8) → 유효하지 않음
        let result = state.handle_click(8, 8);
        assert_eq!(result, PopupKeyResult::Consumed);
    }

    #[test]
    fn special_page_layout_second_page() {
        // 100 chars: page 0 = 81, page 1 = 19
        let mut state = make_special(100);
        state.current_page = 1;
        state.update_page_layout();
        // 19 chars: cols = ceil(19/9) = 3, rows = ceil(19/3) = 7
        assert_eq!(state.cols, 3);
        assert_eq!(state.rows, 7);
    }

    #[test]
    fn special_space_next_page() {
        let mut state = make_special(100);
        state.handle_key(PopupKey::Space);
        assert_eq!(state.current_page, 1);
    }

    #[test]
    fn special_empty() {
        let state = make_special(0);
        assert_eq!(state.total_pages, 1);
        assert_eq!(state.rows, 1);
        assert_eq!(state.cols, 1);
    }

    // --- 한자 팝업 테스트 ---

    fn make_hanja(n: usize) -> PopupState {
        let candidates: Vec<(String, String)> = (0..n)
            .map(|i| (format!("漢{}", i), format!("뜻{}", i)))
            .collect();
        PopupState::new_hanja("한", candidates)
    }

    #[test]
    fn hanja_basic_layout() {
        let state = make_hanja(20);
        assert_eq!(state.total_pages, 3); // ceil(20/9)
        assert_eq!(state.rows, 9); // 첫 페이지 9개
        assert_eq!(state.cols, 1);
    }

    #[test]
    fn hanja_number_select() {
        let mut state = make_hanja(20);
        let result = state.handle_key(PopupKey::Number(3));
        assert_eq!(result, PopupKeyResult::Select(2)); // 0-based
    }

    #[test]
    fn hanja_number_out_of_range() {
        let mut state = make_hanja(3);
        let result = state.handle_key(PopupKey::Number(5));
        assert_eq!(result, PopupKeyResult::Consumed);
    }

    #[test]
    fn hanja_arrow_down_wrap() {
        let mut state = make_hanja(9);
        state.sel_row = 8;
        state.handle_key(PopupKey::Down);
        assert_eq!(state.sel_row, 0);
    }

    #[test]
    fn hanja_arrow_up_wrap() {
        let mut state = make_hanja(9);
        assert_eq!(state.sel_row, 0);
        state.handle_key(PopupKey::Up);
        assert_eq!(state.sel_row, 8);
    }

    #[test]
    fn hanja_right_next_page() {
        let mut state = make_hanja(20);
        state.handle_key(PopupKey::Right);
        assert_eq!(state.current_page, 1);
        assert_eq!(state.sel_row, 0);
    }

    #[test]
    fn hanja_right_wrap_page() {
        let mut state = make_hanja(20);
        state.current_page = 2; // 마지막 페이지 (3페이지 중)
        state.update_page_layout();
        state.handle_key(PopupKey::Right);
        assert_eq!(state.current_page, 0); // 래핑
    }

    #[test]
    fn hanja_left_prev_page() {
        let mut state = make_hanja(20);
        state.current_page = 1;
        state.update_page_layout();
        state.handle_key(PopupKey::Left);
        assert_eq!(state.current_page, 0);
    }

    #[test]
    fn hanja_left_wrap_page() {
        let mut state = make_hanja(20);
        state.handle_key(PopupKey::Left);
        assert_eq!(state.current_page, 2); // 래핑
    }

    #[test]
    fn hanja_space_next_page() {
        let mut state = make_hanja(20);
        state.handle_key(PopupKey::Space);
        assert_eq!(state.current_page, 1);
    }

    #[test]
    fn hanja_backspace_prev_page() {
        let mut state = make_hanja(20);
        state.current_page = 1;
        state.update_page_layout();
        state.handle_key(PopupKey::Backspace);
        assert_eq!(state.current_page, 0);
    }

    #[test]
    fn hanja_enter_select() {
        let mut state = make_hanja(9);
        state.sel_row = 4;
        let result = state.handle_key(PopupKey::Enter);
        assert_eq!(result, PopupKeyResult::Select(4));
    }

    #[test]
    fn hanja_escape_cancel() {
        let mut state = make_hanja(9);
        let result = state.handle_key(PopupKey::Escape);
        assert_eq!(result, PopupKeyResult::Cancel);
    }

    #[test]
    fn hanja_letter_not_handled() {
        let mut state = make_hanja(9);
        let result = state.handle_key(PopupKey::Letter(0));
        assert_eq!(result, PopupKeyResult::NotHandled);
    }

    #[test]
    fn hanja_click_select() {
        let mut state = make_hanja(9);
        let result = state.handle_click(3, 0);
        assert_eq!(result, PopupKeyResult::Select(3));
    }

    #[test]
    fn hanja_page_items() {
        let state = make_hanja(20);
        let items = state.hanja_page_items();
        assert_eq!(items.len(), 9);
        assert_eq!(items[0].0, "漢0");
        assert_eq!(items[0].1, "뜻0");
    }

    #[test]
    fn hanja_last_page_items() {
        let mut state = make_hanja(20);
        state.current_page = 2;
        state.update_page_layout();
        let items = state.hanja_page_items();
        assert_eq!(items.len(), 2); // 20 - 18 = 2
    }

    #[test]
    fn hanja_empty() {
        let state = make_hanja(0);
        assert_eq!(state.total_pages, 1);
        assert_eq!(state.rows, 0);
    }

    #[test]
    fn hanja_second_page_number_select() {
        let mut state = make_hanja(20);
        state.handle_key(PopupKey::Right); // page 1
        let result = state.handle_key(PopupKey::Number(1));
        assert_eq!(result, PopupKeyResult::Select(9)); // page 1, idx 0
    }

    // --- 경계 조건 테스트 ---

    #[test]
    fn special_single_char() {
        let state = make_special(1);
        assert_eq!(state.rows, 1);
        assert_eq!(state.cols, 1);
        assert_eq!(state.total_pages, 1);
    }

    #[test]
    fn special_exactly_one_page() {
        let state = make_special(81);
        assert_eq!(state.total_pages, 1);
    }

    #[test]
    fn special_one_over_page() {
        let state = make_special(82);
        assert_eq!(state.total_pages, 2);
    }

    #[test]
    fn hanja_exactly_one_page() {
        let state = make_hanja(9);
        assert_eq!(state.total_pages, 1);
    }

    #[test]
    fn hanja_one_over_page() {
        let state = make_hanja(10);
        assert_eq!(state.total_pages, 2);
    }

    #[test]
    fn special_page_switch_resets_selection() {
        let mut state = make_special(100);
        state.sel_row = 5;
        state.sel_col = 3;
        state.handle_key(PopupKey::Tab);
        assert_eq!(state.sel_row, 0);
        assert_eq!(state.sel_col, 0);
    }

    #[test]
    fn hanja_page_switch_resets_selection() {
        let mut state = make_hanja(20);
        state.sel_row = 5;
        state.handle_key(PopupKey::Right);
        assert_eq!(state.sel_row, 0);
    }

    #[test]
    fn special_get_item() {
        let state = make_special(10);
        assert_eq!(state.get_item(0), Some("C0"));
        assert_eq!(state.get_item(9), Some("C9"));
        assert_eq!(state.get_item(10), None);
    }

    #[test]
    fn hanja_get_meaning() {
        let state = make_hanja(5);
        assert_eq!(state.get_meaning(0), Some("뜻0"));
        assert_eq!(state.get_meaning(4), Some("뜻4"));
        assert_eq!(state.get_meaning(5), None);
    }

    #[test]
    fn special_cell_text() {
        let state = make_special(81);
        assert_eq!(state.cell_text(0, 0), Some("C0"));
        assert_eq!(state.cell_text(0, 1), Some("C9"));  // 열 우선
        assert_eq!(state.cell_text(8, 8), Some("C80"));
    }

    #[test]
    fn special_single_page_no_tab_effect() {
        let mut state = make_special(10);
        state.handle_key(PopupKey::Tab);
        assert_eq!(state.current_page, 0); // 단일 페이지면 변경 없음
    }
}
