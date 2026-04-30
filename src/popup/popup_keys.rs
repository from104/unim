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

/// 한자 페이지 크기
pub(super) const HANJA_PAGE_SIZE: usize = 9;

impl PopupState {
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

            PopupKey::Period => PopupKeyResult::Consumed,
            PopupKey::Modifier => PopupKeyResult::Consumed,
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
            PopupKey::Other => PopupKeyResult::NotHandled,
        }
    }
}
