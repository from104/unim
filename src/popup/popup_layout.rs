//! 팝업 레이아웃 계산 및 단순 접근자
//!
//! 페이지·행·열·인덱스 변환과 셀 존재 검사 등 상태에 의존하는
//! 파생값들을 계산합니다. 키 처리(`popup_keys`)와 핵심 상태
//! (`popup_state`)에서 호출됩니다.

use super::popup_keys::{PopupKind, HANJA_PAGE_SIZE, MAX_COLS, MAX_ROWS, SPECIAL_PAGE_SIZE};
use super::popup_state::PopupState;

impl PopupState {
    /// 현재 페이지 레이아웃 재계산
    pub(super) fn update_page_layout(&mut self) {
        match self.kind {
            PopupKind::SpecialChar => {
                let page_chars = self.page_item_count();
                self.cols = if page_chars == 0 {
                    1
                } else {
                    ((page_chars + MAX_ROWS - 1) / MAX_ROWS).clamp(1, MAX_COLS)
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
                if self.hanja_expanded {
                    let page_chars = self.page_item_count();
                    self.cols = if page_chars == 0 {
                        1
                    } else {
                        ((page_chars + MAX_ROWS - 1) / MAX_ROWS).clamp(1, MAX_COLS)
                    };
                    self.rows = if page_chars == 0 {
                        1
                    } else {
                        ((page_chars + self.cols - 1) / self.cols)
                            .min(MAX_ROWS)
                            .max(1)
                    };
                } else {
                    let page_items = self.page_item_count();
                    self.rows = page_items;
                    self.cols = 1;
                }
            }
        }
    }

    /// 한자 팝업 확장 모드 토글 (current_page/선택 초기화).
    pub fn toggle_hanja_expanded(&mut self) {
        if self.kind != PopupKind::Hanja {
            return;
        }
        self.hanja_expanded = !self.hanja_expanded;
        self.page_size = if self.hanja_expanded {
            SPECIAL_PAGE_SIZE
        } else {
            HANJA_PAGE_SIZE
        };
        let total = self.items.len();
        self.total_pages = if total == 0 {
            1
        } else {
            total.div_ceil(self.page_size)
        };
        self.current_page = 0;
        self.sel_row = 0;
        self.sel_col = 0;
        self.update_page_layout();
    }

    /// 한자 팝업 확장 모드 여부.
    pub fn is_hanja_expanded(&self) -> bool {
        self.hanja_expanded
    }

    /// 현재 페이지의 항목 수
    pub(super) fn page_item_count(&self) -> usize {
        let page_start = self.current_page * self.page_size;
        if page_start >= self.items.len() {
            0
        } else {
            (self.items.len() - page_start).min(self.page_size)
        }
    }

    /// 특수문자 그리드에서 (row, col) → 전체 인덱스 (열 우선 채움)
    pub(super) fn special_global_index(&self, row: usize, col: usize) -> Option<usize> {
        let page_start = self.current_page * self.page_size;
        let idx = col * self.rows + row;
        let global = page_start + idx;
        if global < self.items.len() {
            Some(global)
        } else {
            None
        }
    }

    /// 한자 목록에서 페이지 내 인덱스 → 전체 인덱스 (compact 모드 전용)
    pub(super) fn hanja_global_index(&self, page_index: usize) -> Option<usize> {
        let global = self.current_page * self.page_size + page_index;
        if global < self.items.len() {
            Some(global)
        } else {
            None
        }
    }

    /// 한자 그리드 (row, col) → 전체 인덱스. compact 모드에서는 col을 무시한다.
    pub(super) fn hanja_global_index_rc(&self, row: usize, col: usize) -> Option<usize> {
        let page_start = self.current_page * self.page_size;
        let page_offset = if self.hanja_expanded {
            col * self.rows + row
        } else {
            row
        };
        let global = page_start + page_offset;
        if global < self.items.len() {
            Some(global)
        } else {
            None
        }
    }

    /// 한자 그리드에서 해당 셀에 항목이 있는지 확인
    pub(super) fn hanja_cell_exists(&self, row: usize, col: usize) -> bool {
        self.hanja_global_index_rc(row, col).is_some()
    }

    /// 특수문자 그리드에서 해당 셀에 문자가 있는지 확인
    pub(super) fn cell_exists(&self, row: usize, col: usize) -> bool {
        self.special_global_index(row, col).is_some()
    }

    /// 현재 선택된 항목의 전체 인덱스
    pub fn selected_global_index(&self) -> Option<usize> {
        match self.kind {
            PopupKind::SpecialChar => self.special_global_index(self.sel_row, self.sel_col),
            PopupKind::Hanja => self.hanja_global_index_rc(self.sel_row, self.sel_col),
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

    /// 전체 인덱스로 즐겨찾기 상태 조회 (한자 전용; 특수문자는 항상 false).
    pub fn is_bookmarked(&self, index: usize) -> bool {
        self.bookmarks.get(index).copied().unwrap_or(false)
    }

    /// 한자 팝업의 즐겨찾기 플래그를 items 순서대로 설정합니다.
    ///
    /// 길이가 맞지 않으면 items 길이에 맞춰 잘라내거나 false로 채웁니다.
    pub fn set_bookmark_flags(&mut self, flags: Vec<bool>) {
        if self.kind != PopupKind::Hanja {
            return;
        }
        self.bookmarks = flags;
        self.bookmarks.resize(self.items.len(), false);
    }

    /// 특정 인덱스의 즐겨찾기 플래그를 설정합니다 (한자 전용).
    pub fn set_bookmark(&mut self, index: usize, bookmarked: bool) {
        if self.kind != PopupKind::Hanja {
            return;
        }
        if let Some(slot) = self.bookmarks.get_mut(index) {
            *slot = bookmarked;
        }
    }

    /// 한자 후보 목록 일괄 교체 (즐겨찾기 토글 후 재정렬용).
    ///
    /// 페이지 수만 다시 계산하고, 현재 페이지/선택 위치는 유지한다.
    /// 호출자는 보통 직후 [`set_selected_global`]을 호출해 새 정렬에서의
    /// 토글된 한자 위치로 커서를 이동시킨다.
    pub fn replace_hanja_items(
        &mut self,
        items: Vec<String>,
        meanings: Vec<String>,
        bookmarks: Vec<bool>,
    ) {
        if self.kind != PopupKind::Hanja {
            return;
        }
        let total = items.len();
        self.items = items;
        self.meanings = meanings;
        self.bookmarks = bookmarks;
        self.bookmarks.resize(total, false);
        self.meanings.resize(total, String::new());
        self.total_pages = if total == 0 {
            1
        } else {
            total.div_ceil(self.page_size)
        };
        if self.current_page >= self.total_pages {
            self.current_page = self.total_pages.saturating_sub(1);
        }
        self.update_page_layout();
    }

    /// 전체 인덱스로 커서 위치를 강제 설정한다.
    ///
    /// 한자 즐겨찾기 토글 후 재정렬된 새 위치로 점프할 때 사용.
    /// 페이지·sel_row·sel_col을 한 번에 갱신한다.
    /// 한자 expanded 모드는 col 우선 인덱싱(idx = col*rows + row)을 따른다.
    pub fn set_selected_global(&mut self, global_index: usize) {
        if global_index >= self.items.len() {
            return;
        }
        let new_page = global_index / self.page_size;
        let page_offset = global_index % self.page_size;
        if new_page != self.current_page {
            self.current_page = new_page.min(self.total_pages.saturating_sub(1));
            self.update_page_layout();
        }
        match self.kind {
            PopupKind::Hanja => {
                if self.hanja_expanded {
                    // col 우선 인덱싱 (rows = MAX_ROWS = 9 보통)
                    let rows = self.rows.max(1);
                    self.sel_col = page_offset / rows;
                    self.sel_row = page_offset % rows;
                } else {
                    self.sel_row = page_offset;
                    self.sel_col = 0;
                }
            }
            PopupKind::SpecialChar => {
                let rows = self.rows.max(1);
                self.sel_col = page_offset / rows;
                self.sel_row = page_offset % rows;
            }
        }
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
                let global = self.hanja_global_index_rc(row, col)?;
                self.items.get(global).map(|s| s.as_str())
            }
        }
    }

    /// 엔진의 PopupNavigate 시그널에서 받은 상태로 갱신
    ///
    /// 엔진 위임 모델에서 팝업 UI가 엔진의 상태를 반영할 때 사용.
    /// page/sel_row/sel_col만 설정하고 레이아웃을 재계산한다.
    pub fn set_navigate_state(&mut self, page: usize, sel_row: usize, sel_col: usize) {
        if page < self.total_pages {
            self.current_page = page;
        }
        self.update_page_layout();
        self.sel_row = sel_row.min(self.rows.saturating_sub(1));
        self.sel_col = sel_col.min(self.cols.saturating_sub(1));
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
