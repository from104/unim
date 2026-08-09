//! toolkit-free popup 데이터 모델
//!
//! `PopupModel`은 `PopupRender` 이벤트를 받아 상태를 보관하는 thin wrapper.
//! GTK 측에서 Phase 1에서는 직접 사용하지 않아도 OK.
//! Phase 2에서 Qt popup이 이 모델을 사용한다.

// ─────────────────────────────────────────────────────────────
// PopupKind
// ─────────────────────────────────────────────────────────────

/// popup 종류
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PopupKind {
    Hanja = 0,
    Special = 1,
    Emoji = 2,
}

impl PopupKind {
    pub fn from_u32(v: u32) -> Option<Self> {
        match v {
            0 => Some(Self::Hanja),
            1 => Some(Self::Special),
            2 => Some(Self::Emoji),
            _ => None,
        }
    }
}

// ─────────────────────────────────────────────────────────────
// PopupCellFlags
// ─────────────────────────────────────────────────────────────

/// 셀 플래그 비트마스크 — GuiAction::PopupRender cells.flags 와 일치해야 한다.
pub mod cell_flags {
    /// 데이터 있음
    pub const HAS_DATA: u32 = 0x01;
    /// 선택된 셀
    pub const SELECTED: u32 = 0x02;
    /// 열 강조
    pub const COL_HL: u32 = 0x04;
    /// 행 강조
    pub const ROW_HL: u32 = 0x08;
    /// 북마크
    pub const BOOKMARKED: u32 = 0x10;
}

// ─────────────────────────────────────────────────────────────
// PopupModel
// ─────────────────────────────────────────────────────────────

/// popup 페이지·선택·grid 데이터를 toolkit-free로 보관하는 모델.
///
/// `apply_render`로 `PopupRender` 이벤트 데이터를 그대로 받아 저장한다.
/// `cell(row, col)`로 특정 셀 데이터를 조회할 수 있다.
#[derive(Debug, Clone, Default)]
pub struct PopupModel {
    pub kind: u32,
    pub target: String,
    pub header_text: String,
    pub footer_text: String,
    pub show_footer: bool,
    pub rows: u32,
    pub cols: u32,
    pub sel_row: u32,
    pub sel_col: u32,
    pub current_page: u32,
    pub total_pages: u32,
    /// column-major 평면: `cells[col * rows + row] = (text, meaning, flags)`
    pub cells: Vec<(String, String, u32)>,
    pub col_headers: Vec<(String, bool)>,
    pub row_headers: Vec<(String, bool)>,
    pub expand_visible: bool,
    pub expand_text: String,
    pub tab_labels: Vec<String>,
    pub active_tab_index: u32,
}

impl PopupModel {
    pub fn new() -> Self {
        Self::default()
    }

    /// PopupRender 이벤트 데이터를 받아 모델 전체를 갱신한다.
    #[allow(clippy::too_many_arguments)]
    pub fn apply_render(
        &mut self,
        kind: u32,
        target: String,
        header_text: String,
        footer_text: String,
        show_footer: bool,
        rows: u32,
        cols: u32,
        sel_row: u32,
        sel_col: u32,
        current_page: u32,
        total_pages: u32,
        cells: Vec<(String, String, u32)>,
        col_headers: Vec<(String, bool)>,
        row_headers: Vec<(String, bool)>,
        expand_visible: bool,
        expand_text: String,
        tab_labels: Vec<String>,
        active_tab_index: u32,
    ) {
        self.kind = kind;
        self.target = target;
        self.header_text = header_text;
        self.footer_text = footer_text;
        self.show_footer = show_footer;
        self.rows = rows;
        self.cols = cols;
        self.sel_row = sel_row;
        self.sel_col = sel_col;
        self.current_page = current_page;
        self.total_pages = total_pages;
        self.cells = cells;
        self.col_headers = col_headers;
        self.row_headers = row_headers;
        self.expand_visible = expand_visible;
        self.expand_text = expand_text;
        self.tab_labels = tab_labels;
        self.active_tab_index = active_tab_index;
    }

    /// PopupNavigate 이벤트로 페이지·선택 위치만 갱신.
    pub fn apply_navigate(
        &mut self,
        page: i32,
        total_pages: i32,
        sel_row: i32,
        sel_col: i32,
    ) {
        if page >= 0 {
            self.current_page = page as u32;
        }
        if total_pages >= 0 {
            self.total_pages = total_pages as u32;
        }
        if sel_row >= 0 {
            self.sel_row = sel_row as u32;
        }
        if sel_col >= 0 {
            self.sel_col = sel_col as u32;
        }
    }

    /// 특정 셀 데이터 조회 (column-major). 범위 밖이면 `None`.
    pub fn cell(&self, row: u32, col: u32) -> Option<&(String, String, u32)> {
        if row >= self.rows || col >= self.cols {
            return None;
        }
        let idx = (col * self.rows + row) as usize;
        self.cells.get(idx)
    }
}

// ─────────────────────────────────────────────────────────────
// 단위 테스트
// ─────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn make_model_3x3() -> PopupModel {
        let mut m = PopupModel::new();
        // 3 rows × 3 cols, column-major
        // col0: (A,_,has_data), (B,_,has_data), (C,_,has_data)
        // col1: (D,_,selected), (E,_,has_data), (F,_,has_data)
        // col2: (G,_,has_data), (H,_,has_data), (I,_,bookmarked)
        let cells: Vec<(String, String, u32)> = vec![
            ("A".into(), "".into(), cell_flags::HAS_DATA),
            ("B".into(), "".into(), cell_flags::HAS_DATA),
            ("C".into(), "".into(), cell_flags::HAS_DATA),
            ("D".into(), "".into(), cell_flags::HAS_DATA | cell_flags::SELECTED),
            ("E".into(), "".into(), cell_flags::HAS_DATA),
            ("F".into(), "".into(), cell_flags::HAS_DATA),
            ("G".into(), "".into(), cell_flags::HAS_DATA),
            ("H".into(), "".into(), cell_flags::HAS_DATA),
            ("I".into(), "".into(), cell_flags::HAS_DATA | cell_flags::BOOKMARKED),
        ];
        m.apply_render(
            0, "test".into(), "header".into(), "footer".into(), true,
            3, 3, 0, 1, 0, 1, cells,
            vec![], vec![], false, "".into(), vec![], 0,
        );
        m
    }

    #[test]
    fn cell_lookup_correct() {
        let m = make_model_3x3();
        // (row=0, col=0) → index 0 → "A"
        assert_eq!(m.cell(0, 0).map(|c| c.0.as_str()), Some("A"));
        // (row=0, col=1) → index 3 → "D" (selected)
        let d = m.cell(0, 1).unwrap();
        assert_eq!(d.0, "D");
        assert!(d.2 & cell_flags::SELECTED != 0);
        // (row=2, col=2) → index 8 → "I" (bookmarked)
        let i = m.cell(2, 2).unwrap();
        assert_eq!(i.0, "I");
        assert!(i.2 & cell_flags::BOOKMARKED != 0);
    }

    #[test]
    fn cell_out_of_bounds_returns_none() {
        let m = make_model_3x3();
        assert!(m.cell(3, 0).is_none());
        assert!(m.cell(0, 3).is_none());
    }

    #[test]
    fn apply_navigate_updates_page_and_sel() {
        let mut m = make_model_3x3();
        m.apply_navigate(2, 5, 1, 2);
        assert_eq!(m.current_page, 2);
        assert_eq!(m.total_pages, 5);
        assert_eq!(m.sel_row, 1);
        assert_eq!(m.sel_col, 2);
    }
}
