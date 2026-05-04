//! 팝업 뷰 모델
//!
//! PopupState로부터 렌더링에 필요한 모든 데이터를 구조화하여 제공합니다.
//! 프런트엔드는 이 뷰 모델만으로 화면을 그릴 수 있습니다.

use super::popup_keys::PopupKind;
use super::popup_state::PopupState;

/// 개별 셀 데이터
#[derive(Debug, Clone)]
pub struct CellData {
    /// 표시할 텍스트 (한자 또는 특수문자)
    pub text: String,
    /// 뜻풀이 (한자 전용, 특수문자는 None)
    pub meaning: Option<String>,
    /// 현재 선택된 셀인지
    pub is_selected: bool,
    /// 선택된 열과 같은 열인지 (특수문자/한자 expanded 열 헤더 강조용)
    pub is_col_highlight: bool,
    /// 선택된 행과 같은 행인지 (특수문자/한자 expanded 행 번호 강조용)
    pub is_row_highlight: bool,
}

/// 팝업 뷰 모델 — 렌더링에 필요한 모든 데이터
#[derive(Debug, Clone)]
pub struct PopupViewModel {
    /// 팝업 종류
    pub kind: PopupKind,
    /// 대상 문자열
    pub target: String,
    /// 그리드 데이터 [row][col], None이면 빈 셀
    pub cells: Vec<Vec<Option<CellData>>>,
    /// 열 헤더 (특수문자: top_row 문자들, 한자: 빈 벡터)
    pub col_headers: Vec<String>,
    /// 행 헤더 (특수문자: "1"-"9", 한자: "1."-"9.")
    pub row_headers: Vec<String>,
    /// 선택 행
    pub sel_row: usize,
    /// 선택 열
    pub sel_col: usize,
    /// 현재 페이지 (0-based)
    pub current_page: usize,
    /// 전체 페이지 수
    pub total_pages: usize,
    /// 푸터 텍스트
    pub footer_text: String,
}

impl PopupState {
    /// 현재 상태에서 뷰 모델 생성
    pub fn view_model(&self) -> PopupViewModel {
        match self.kind() {
            PopupKind::SpecialChar => self.special_view_model(),
            PopupKind::Hanja => self.hanja_view_model(),
            PopupKind::Emoji => self.emoji_view_model(),
        }
    }

    /// 이모지 팝업 뷰 모델 — SpecialChar 와 동일 9×9 그리드, 푸터에 카테고리 라벨 추가.
    fn emoji_view_model(&self) -> PopupViewModel {
        // 셀 채우기는 SpecialChar 와 동일.
        let top_row_chars: Vec<char> = self.top_row().chars().collect();
        let col_headers: Vec<String> = (0..self.cols())
            .map(|c| {
                if c < top_row_chars.len() {
                    top_row_chars[c].to_string()
                } else {
                    format!("{}", c + 1)
                }
            })
            .collect();
        let row_headers: Vec<String> = (1..=self.rows()).map(|r| format!("{}", r)).collect();

        let mut cells = Vec::with_capacity(self.rows());
        for r in 0..self.rows() {
            let mut row = Vec::with_capacity(self.cols());
            for c in 0..self.cols() {
                if let Some(text) = self.cell_text(r, c) {
                    row.push(Some(CellData {
                        text: text.to_string(),
                        meaning: None,
                        is_selected: r == self.sel_row() && c == self.sel_col(),
                        is_col_highlight: c == self.sel_col(),
                        is_row_highlight: r == self.sel_row(),
                    }));
                } else {
                    row.push(None);
                }
            }
            cells.push(row);
        }

        // 푸터: 카테고리 라벨 + 페이지 인디케이터.
        let cat_label = self
            .emoji_categories()
            .get(self.emoji_cat_index())
            .map(|c| c.label_ko.as_str())
            .unwrap_or("");
        let footer_text = format!(
            "[{}]  {} / {}",
            cat_label,
            self.current_page() + 1,
            self.total_pages()
        );

        PopupViewModel {
            kind: PopupKind::Emoji,
            target: self.target().to_string(),
            cells,
            col_headers,
            row_headers,
            sel_row: self.sel_row(),
            sel_col: self.sel_col(),
            current_page: self.current_page(),
            total_pages: self.total_pages(),
            footer_text,
        }
    }

    fn special_view_model(&self) -> PopupViewModel {
        let top_row_chars: Vec<char> = self.top_row().chars().collect();

        let col_headers: Vec<String> = (0..self.cols())
            .map(|c| {
                if c < top_row_chars.len() {
                    top_row_chars[c].to_string()
                } else {
                    format!("{}", c + 1)
                }
            })
            .collect();

        let row_headers: Vec<String> = (1..=self.rows()).map(|r| format!("{}", r)).collect();

        let mut cells = Vec::with_capacity(self.rows());
        for r in 0..self.rows() {
            let mut row = Vec::with_capacity(self.cols());
            for c in 0..self.cols() {
                if let Some(text) = self.cell_text(r, c) {
                    row.push(Some(CellData {
                        text: text.to_string(),
                        meaning: None,
                        is_selected: r == self.sel_row() && c == self.sel_col(),
                        is_col_highlight: c == self.sel_col(),
                        is_row_highlight: r == self.sel_row(),
                    }));
                } else {
                    row.push(None);
                }
            }
            cells.push(row);
        }

        let footer_text = format!(
            "[{}]  {} / {}",
            self.target(),
            self.current_page() + 1,
            self.total_pages()
        );

        PopupViewModel {
            kind: PopupKind::SpecialChar,
            target: self.target().to_string(),
            cells,
            col_headers,
            row_headers,
            sel_row: self.sel_row(),
            sel_col: self.sel_col(),
            current_page: self.current_page(),
            total_pages: self.total_pages(),
            footer_text,
        }
    }

    fn hanja_view_model(&self) -> PopupViewModel {
        let items = self.hanja_page_items();

        let row_headers: Vec<String> = (1..=items.len()).map(|r| format!("{}.", r)).collect();

        let mut cells = Vec::with_capacity(items.len());
        for (i, (hanja, meaning)) in items.iter().enumerate() {
            let cell = CellData {
                text: hanja.to_string(),
                meaning: if meaning.is_empty() {
                    None
                } else {
                    Some(meaning.to_string())
                },
                is_selected: i == self.sel_row(),
                is_col_highlight: false,
                is_row_highlight: i == self.sel_row(),
            };
            cells.push(vec![Some(cell)]);
        }

        let footer_text = format!(
            "「{}」 → 한자  {}/{}",
            self.target(),
            self.current_page() + 1,
            self.total_pages()
        );

        PopupViewModel {
            kind: PopupKind::Hanja,
            target: self.target().to_string(),
            cells,
            col_headers: Vec::new(),
            row_headers,
            sel_row: self.sel_row(),
            sel_col: 0,
            current_page: self.current_page(),
            total_pages: self.total_pages(),
            footer_text,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn special_view_model_basic() {
        // 20 chars → cols=3, rows=9 (rows 고정 정책). col=0 chars 0..8,
        // col=1 chars 9..17, col=2 chars 18..19. 빈 셀은 None.
        let state = PopupState::new_special(
            "ㄱ",
            (0..20).map(|i| format!("S{}", i)).collect(),
            "QWERTYUIO",
        );
        let vm = state.view_model();
        assert_eq!(vm.kind, PopupKind::SpecialChar);
        assert_eq!(vm.target, "ㄱ");
        assert_eq!(vm.col_headers.len(), 3); // 3열
        assert_eq!(vm.row_headers.len(), 9); // 9행 (고정)
        assert_eq!(vm.col_headers[0], "Q");
        assert_eq!(vm.col_headers[1], "W");
        assert_eq!(vm.col_headers[2], "E");
        assert_eq!(vm.row_headers[0], "1");
        assert_eq!(vm.cells.len(), 9); // 9행 (고정)
        assert!(vm.cells[0][0].is_some());
        assert_eq!(vm.cells[0][0].as_ref().unwrap().text, "S0");
        assert!(vm.cells[0][0].as_ref().unwrap().is_selected); // (0,0) 선택
        // col=2 row=2 이상은 빈 셀 (S20 없음 → idx 2*9+2=20 >= 20)
        assert!(vm.cells[2][2].is_none());
        assert_eq!(vm.footer_text, "[ㄱ]  1 / 1");
    }

    #[test]
    fn special_view_model_selection() {
        let mut state = PopupState::new_special(
            "ㄱ",
            (0..81).map(|i| format!("S{}", i)).collect(),
            "QWERTYUIO",
        );
        state.handle_key(super::super::PopupKey::Right); // col 1
        state.handle_key(super::super::PopupKey::Down); // row 1
        let vm = state.view_model();
        assert!(vm.cells[1][1].as_ref().unwrap().is_selected);
        assert!(!vm.cells[0][0].as_ref().unwrap().is_selected);
        // col 1은 highlight
        assert!(vm.cells[0][1].as_ref().unwrap().is_col_highlight);
        assert!(!vm.cells[0][0].as_ref().unwrap().is_col_highlight);
        // row 1도 highlight (sel_row와 같은 행의 모든 셀)
        assert!(vm.cells[1][0].as_ref().unwrap().is_row_highlight);
        assert!(vm.cells[1][2].as_ref().unwrap().is_row_highlight);
        assert!(!vm.cells[0][0].as_ref().unwrap().is_row_highlight);
        assert!(!vm.cells[2][1].as_ref().unwrap().is_row_highlight);
    }

    #[test]
    fn special_view_model_row_highlight_initial() {
        // 초기 상태(sel_row=0, sel_col=0): 0행 전체가 row_highlight, 0열 전체가 col_highlight
        let state = PopupState::new_special(
            "ㄱ",
            (0..81).map(|i| format!("S{}", i)).collect(),
            "QWERTYUIO",
        );
        let vm = state.view_model();
        assert!(vm.cells[0][0].as_ref().unwrap().is_row_highlight);
        assert!(vm.cells[0][1].as_ref().unwrap().is_row_highlight);
        assert!(!vm.cells[1][0].as_ref().unwrap().is_row_highlight);
    }

    #[test]
    fn special_view_model_empty_cells() {
        // 20 chars: 3 cols × 7 rows, col 2 has only 6 items
        let state = PopupState::new_special(
            "ㄱ",
            (0..20).map(|i| format!("S{}", i)).collect(),
            "QWERTYUIO",
        );
        let vm = state.view_model();
        // col 2, row 6 → idx = 2*7+6 = 20 → out of range
        assert!(vm.cells[6][2].is_none());
    }

    #[test]
    fn hanja_view_model_basic() {
        let candidates: Vec<(String, String)> = vec![
            ("韓".to_string(), "나라 한".to_string()),
            ("漢".to_string(), "한나라 한".to_string()),
        ];
        let state = PopupState::new_hanja("한", candidates);
        let vm = state.view_model();
        assert_eq!(vm.kind, PopupKind::Hanja);
        assert_eq!(vm.target, "한");
        assert!(vm.col_headers.is_empty());
        assert_eq!(vm.row_headers, vec!["1.", "2."]);
        assert_eq!(vm.cells.len(), 2);
        assert_eq!(vm.cells[0][0].as_ref().unwrap().text, "韓");
        assert_eq!(
            vm.cells[0][0].as_ref().unwrap().meaning,
            Some("나라 한".to_string())
        );
        assert!(vm.cells[0][0].as_ref().unwrap().is_selected);
        assert!(!vm.cells[1][0].as_ref().unwrap().is_selected);
        // row highlight: 선택 행만 (compact 1열에서는 is_selected와 일치하지만 의미상 분리)
        assert!(vm.cells[0][0].as_ref().unwrap().is_row_highlight);
        assert!(!vm.cells[1][0].as_ref().unwrap().is_row_highlight);
        // hanja compact는 1열이라 col_highlight는 항상 false
        assert!(!vm.cells[0][0].as_ref().unwrap().is_col_highlight);
    }

    #[test]
    fn hanja_view_model_empty_meaning() {
        let candidates = vec![("韓".to_string(), "".to_string())];
        let state = PopupState::new_hanja("한", candidates);
        let vm = state.view_model();
        assert!(vm.cells[0][0].as_ref().unwrap().meaning.is_none());
    }

    #[test]
    fn hanja_view_model_footer() {
        let candidates: Vec<(String, String)> = (0..20)
            .map(|i| (format!("漢{}", i), format!("뜻{}", i)))
            .collect();
        let state = PopupState::new_hanja("한", candidates);
        let vm = state.view_model();
        assert_eq!(vm.footer_text, "「한」 → 한자  1/3");
    }
}
