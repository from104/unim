//! 팝업 키/클릭 입력 처리 및 키 enum 정의
//!
//! `PopupKey`, `PopupKind`, `PopupKeyResult` 열거형과
//! 한자/특수문자 팝업의 키 처리 로직을 담당합니다.
//! 상태 자체(`PopupState`)와 레이아웃 계산은 형제 모듈을 참고하세요.

use super::popup_state::PopupState;

/// 팝업 종류
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PopupKind {
    /// 한자 후보 팝업 (단일 열, 9개/페이지)
    Hanja,
    /// 특수문자 팝업 (9×9 그리드, 81개/페이지)
    SpecialChar,
    /// 이모지 팝업 (9×9 그리드 + 좌측 9 카테고리 탭).
    ///
    /// PR #1: SpecialChar 와 동일 페이지 모델을 차용하되, Tab/ShiftTab 은
    /// "다음/이전 카테고리" 로 의미 재해석된다 (페이지 전환은 PageDown/PageUp).
    Emoji,
}

/// 툴킷 중립 키 열거형
///
/// 각 프런트엔드가 자체 키코드(GDK, Qt, X11 keysym 등)를
/// 이 열거형으로 변환하여 PopupState에 전달합니다.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PopupKey {
    /// 숫자 1-9 (1-based)
    Number(u8),
    /// QWERTYUIO 물리 위치 (0-based: Q=0, W=1, ..., O=8) — 한자 expanded·특수문자 열 점프
    Letter(u8),
    /// ASDFGHJKL 물리 위치 (0-based: A=0, S=1, ..., L=8) — 이모지 카테고리 단축키
    CatLetter(u8),
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
    /// Home — 첫 페이지의 첫 셀로 점프
    Home,
    /// End — 마지막 페이지의 마지막 데이터 셀로 점프
    End,
    Space,
    Backspace,
    /// '.' (Period) — 한자 팝업 확장/축소 토글
    Period,
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
    /// 즐겨찾기 토글 요청 (전체 인덱스, 한자 팝업 전용)
    ToggleBookmark(usize),
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
pub(super) const MAX_ROWS: usize = 9;
pub(super) const MAX_COLS: usize = 9;
pub(super) const SPECIAL_PAGE_SIZE: usize = MAX_ROWS * MAX_COLS; // 81

/// 이모지 페이지 크기 (9×9, SpecialChar 와 동일).
pub(super) const EMOJI_PAGE_SIZE: usize = MAX_ROWS * MAX_COLS; // 81

/// 한자 페이지 크기
pub(super) const HANJA_PAGE_SIZE: usize = 9;

impl PopupState {
    /// 키 처리
    pub fn handle_key(&mut self, key: PopupKey) -> PopupKeyResult {
        match self.kind {
            PopupKind::SpecialChar => self.handle_special_key(key),
            PopupKind::Hanja => self.handle_hanja_key(key),
            PopupKind::Emoji => self.handle_emoji_key(key),
        }
    }

    /// Home: 첫 페이지의 첫 셀(0,0)로 점프 — 3개 popup 공통 로직.
    fn jump_to_first(&mut self) {
        self.current_page = 0;
        self.update_page_layout();
        self.sel_row = 0;
        self.sel_col = 0;
    }

    /// End: 마지막 페이지의 마지막 데이터 셀로 점프 — 3개 popup 공통 로직.
    /// compact 한자(cols=1)는 sel_col=0 유지하고 마지막 행으로, 그리드는 column-major
    /// 채움 순서대로 마지막 셀의 (row, col) 계산.
    fn jump_to_last(&mut self) {
        if self.total_pages == 0 {
            self.sel_row = 0;
            self.sel_col = 0;
            return;
        }
        self.current_page = self.total_pages - 1;
        self.update_page_layout();
        let count = self.page_item_count();
        if count == 0 {
            self.sel_row = 0;
            self.sel_col = 0;
            return;
        }
        let rows = self.rows.max(1);
        // compact 한자(cols=1)는 단일 열 — 마지막 행으로.
        // 그리드(expanded hanja, special, emoji)는 column-major 채움이므로
        // last_idx = count-1, col = idx/rows, row = idx%rows.
        let last = count - 1;
        if self.kind == PopupKind::Hanja && !self.hanja_expanded {
            self.sel_col = 0;
            self.sel_row = last;
        } else {
            self.sel_col = last / rows;
            self.sel_row = last % rows;
        }
    }

    /// 마우스 클릭 처리
    ///
    /// # Arguments
    /// * `row` - 클릭한 행 (0-based)
    /// * `col` - 클릭한 열 (0-based, 한자는 항상 0)
    pub fn handle_click(&mut self, row: usize, col: usize) -> PopupKeyResult {
        match self.kind {
            PopupKind::SpecialChar | PopupKind::Emoji => {
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
                if row < self.rows && col < self.cols && self.hanja_cell_exists(row, col) {
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

            PopupKey::Home => {
                self.jump_to_first();
                PopupKeyResult::Updated
            }
            PopupKey::End => {
                self.jump_to_last();
                PopupKeyResult::Updated
            }

            PopupKey::Period => PopupKeyResult::Consumed,
            PopupKey::Modifier => PopupKeyResult::Consumed,
            // CatLetter 는 이모지 전용 키 — 특수문자 모드에서는 무관, 키 소비.
            PopupKey::CatLetter(_) => PopupKeyResult::Consumed,
            PopupKey::Other | PopupKey::Backspace => PopupKeyResult::NotHandled,
        }
    }

    // --- 한자 키 처리 ---

    fn handle_hanja_key(&mut self, key: PopupKey) -> PopupKeyResult {
        match key {
            PopupKey::Escape => PopupKeyResult::Cancel,

            PopupKey::Period => {
                self.toggle_hanja_expanded();
                PopupKeyResult::Updated
            }

            PopupKey::Enter => {
                if let Some(global) = self.selected_global_index() {
                    return PopupKeyResult::Select(global);
                }
                PopupKeyResult::Consumed
            }

            PopupKey::Space => {
                // 현재 선택 항목의 즐겨찾기 토글 (compact / expanded 공통)
                if let Some(global) = self.selected_global_index() {
                    return PopupKeyResult::ToggleBookmark(global);
                }
                PopupKeyResult::Consumed
            }

            PopupKey::Number(n) => {
                let row_idx = (n - 1) as usize;
                if self.hanja_expanded {
                    if row_idx < self.rows && self.hanja_cell_exists(row_idx, self.sel_col) {
                        self.sel_row = row_idx;
                        if let Some(global) = self.selected_global_index() {
                            return PopupKeyResult::Select(global);
                        }
                    }
                } else if let Some(global) = self.hanja_global_index(row_idx) {
                    return PopupKeyResult::Select(global);
                }
                PopupKeyResult::Consumed
            }

            PopupKey::Down => {
                let count = if self.hanja_expanded {
                    self.rows
                } else {
                    self.page_item_count()
                };
                if count > 0 {
                    self.sel_row = (self.sel_row + 1) % count;
                    if self.hanja_expanded && !self.hanja_cell_exists(self.sel_row, self.sel_col) {
                        self.sel_row = 0;
                    }
                }
                PopupKeyResult::Updated
            }

            PopupKey::Up => {
                let count = if self.hanja_expanded {
                    self.rows
                } else {
                    self.page_item_count()
                };
                if count > 0 {
                    if self.sel_row == 0 {
                        self.sel_row = count - 1;
                    } else {
                        self.sel_row -= 1;
                    }
                    if self.hanja_expanded {
                        while self.sel_row > 0
                            && !self.hanja_cell_exists(self.sel_row, self.sel_col)
                        {
                            self.sel_row -= 1;
                        }
                    }
                }
                PopupKeyResult::Updated
            }

            PopupKey::Right => {
                if self.hanja_expanded {
                    if self.cols > 0 {
                        self.sel_col = (self.sel_col + 1) % self.cols;
                        if !self.hanja_cell_exists(self.sel_row, self.sel_col) {
                            self.sel_row = 0;
                        }
                    }
                } else if self.total_pages > 1 {
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

            PopupKey::Left => {
                if self.hanja_expanded {
                    if self.cols > 0 {
                        self.sel_col = if self.sel_col == 0 {
                            self.cols - 1
                        } else {
                            self.sel_col - 1
                        };
                        if !self.hanja_cell_exists(self.sel_row, self.sel_col) {
                            self.sel_row = 0;
                        }
                    }
                } else if self.total_pages > 1 {
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

            PopupKey::Tab | PopupKey::PageDown => {
                if self.total_pages > 1 {
                    if self.current_page + 1 < self.total_pages {
                        self.current_page += 1;
                    } else {
                        self.current_page = 0;
                    }
                    self.update_page_layout();
                    self.sel_row = 0;
                    self.sel_col = 0;
                }
                PopupKeyResult::Updated
            }

            PopupKey::Backspace | PopupKey::ShiftTab | PopupKey::PageUp => {
                if self.total_pages > 1 {
                    if self.current_page > 0 {
                        self.current_page -= 1;
                    } else {
                        self.current_page = self.total_pages - 1;
                    }
                    self.update_page_layout();
                    self.sel_row = 0;
                    self.sel_col = 0;
                }
                PopupKeyResult::Updated
            }

            PopupKey::Home => {
                self.jump_to_first();
                PopupKeyResult::Updated
            }
            PopupKey::End => {
                self.jump_to_last();
                PopupKeyResult::Updated
            }

            PopupKey::Modifier => PopupKeyResult::Consumed,

            // expanded(9x9)에서만 special과 동일한 열 점프 동작.
            // compact(1열)는 NotHandled로 남겨 회귀 방지.
            PopupKey::Letter(col_idx) => {
                if self.hanja_expanded {
                    let col = col_idx as usize;
                    if col < self.cols {
                        self.sel_col = col;
                        if !self.hanja_cell_exists(self.sel_row, self.sel_col) {
                            for r in (0..self.rows).rev() {
                                if self.hanja_cell_exists(r, self.sel_col) {
                                    self.sel_row = r;
                                    break;
                                }
                            }
                        }
                    }
                    PopupKeyResult::Updated
                } else {
                    PopupKeyResult::NotHandled
                }
            }
            // CatLetter 는 이모지 전용 키 — 한자 모드에서는 무관, 키 소비.
            PopupKey::CatLetter(_) => PopupKeyResult::Consumed,
            PopupKey::Other => PopupKeyResult::NotHandled,
        }
    }

    // --- 이모지 키 처리 (PR #1 emoji overhaul) ---
    //
    // SpecialChar 와 같은 9×9 페이지 모델을 차용하되, 다음 차이가 있다:
    //
    // - Tab / ShiftTab → "다음/이전 카테고리" 로 의미 재해석. 페이지 전환은
    //   PageUp / PageDown 만 사용한다 (사용자 결정 #8 / 매핑서 X.2.e).
    // - Space → 페이지 전환 효과 제거 (이모지 commit 없음, 무동작 → Consumed).
    // - 그 외 키 매핑은 `handle_special_key` 와 동일하다.
    //
    // 카테고리 자체의 emoji pool 교체는 엔진(`InputEngine::popup_dispatch`)이
    // 담당한다. 본 함수는 cat_index 만 갱신하고 `PopupKeyResult::Updated` 를
    // 반환한다 — 엔진 측에서 `state.cat_index` 변화를 감지해 새 카테고리의
    // emoji 슬라이스로 `replace_for_category` 를 호출한다.
    fn handle_emoji_key(&mut self, key: PopupKey) -> PopupKeyResult {
        match key {
            PopupKey::Escape => PopupKeyResult::Cancel,

            PopupKey::Letter(col_idx) => {
                let col = col_idx as usize;
                if col < self.cols {
                    self.sel_col = col;
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
                if self.rows == 0 {
                    return PopupKeyResult::Consumed;
                }
                if self.sel_row > 0 {
                    self.sel_row -= 1;
                } else {
                    self.sel_row = self.rows - 1;
                }
                while self.sel_row > 0 && !self.cell_exists(self.sel_row, self.sel_col) {
                    self.sel_row -= 1;
                }
                PopupKeyResult::Updated
            }

            PopupKey::Down => {
                if self.rows == 0 {
                    return PopupKeyResult::Consumed;
                }
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
                if self.cols == 0 {
                    return PopupKeyResult::Consumed;
                }
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
                if self.cols == 0 {
                    return PopupKeyResult::Consumed;
                }
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

            // CatLetter: 홈 행(ASDFGHJKL 위치) 단축키로 9 카테고리 직접 점프.
            // cat_idx 가 categories 범위 밖이면 무동작 (Consumed).
            PopupKey::CatLetter(cat_idx) => {
                let cat_total = self.categories.len();
                let target = cat_idx as usize;
                if target < cat_total && target != self.cat_index {
                    self.cat_index = target;
                    self.current_page = 0;
                    self.sel_row = 0;
                    self.sel_col = 0;
                    PopupKeyResult::Updated
                } else {
                    PopupKeyResult::Consumed
                }
            }

            // Tab / ShiftTab: 카테고리 순환 (cat_index 만 갱신, emoji pool 은 엔진이 교체).
            PopupKey::Tab => {
                let cat_total = self.categories.len().max(1);
                self.cat_index = (self.cat_index + 1) % cat_total;
                self.current_page = 0;
                self.sel_row = 0;
                self.sel_col = 0;
                PopupKeyResult::Updated
            }
            PopupKey::ShiftTab => {
                let cat_total = self.categories.len().max(1);
                self.cat_index = if self.cat_index == 0 {
                    cat_total - 1
                } else {
                    self.cat_index - 1
                };
                self.current_page = 0;
                self.sel_row = 0;
                self.sel_col = 0;
                PopupKeyResult::Updated
            }

            // PageDown / PageUp: 같은 카테고리 내 페이지 전환.
            PopupKey::PageDown => {
                if self.total_pages > 1 {
                    self.current_page = (self.current_page + 1) % self.total_pages;
                    self.update_page_layout();
                    self.sel_row = 0;
                    self.sel_col = 0;
                }
                PopupKeyResult::Updated
            }
            PopupKey::PageUp => {
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

            PopupKey::Home => {
                self.jump_to_first();
                PopupKeyResult::Updated
            }
            PopupKey::End => {
                self.jump_to_last();
                PopupKeyResult::Updated
            }

            PopupKey::Space => PopupKeyResult::Consumed,
            PopupKey::Period => PopupKeyResult::Consumed,
            PopupKey::Modifier => PopupKeyResult::Consumed,
            PopupKey::Backspace => PopupKeyResult::Consumed,
            PopupKey::Other => PopupKeyResult::NotHandled,
        }
    }
}
