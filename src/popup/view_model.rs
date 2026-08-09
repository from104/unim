//! 팝업 뷰 모델
//!
//! PopupState로부터 렌더링에 필요한 모든 데이터를 구조화하여 제공합니다.
//! 프런트엔드는 이 뷰 모델만으로 화면을 그릴 수 있습니다.

use super::popup_keys::{PopupKind, MAX_COLS};
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
    /// 즐겨찾기 (한자 popup 전용, 그 외엔 항상 false)
    pub is_bookmarked: bool,
}

/// 팝업 뷰 모델 — 렌더링에 필요한 모든 데이터.
///
/// frontend (GNOME extension / unim-gui-gtk) 가 본 모델을 그대로 consume 하여
/// 헤더·푸터·셀·탭·확장 아이콘 등을 즉시 렌더할 수 있다. 서식 문자열·visibility
/// 조건 등 표시 로직은 모두 daemon (engine) 측에 SoT 로 위치 — 두 frontend 가
/// 동일한 결과를 보장한다.
#[derive(Debug, Clone)]
pub struct PopupViewModel {
    /// 팝업 종류
    pub kind: PopupKind,
    /// 대상 문자열 (한자: 음절, 특수문자: 초성, 이모지: 카테고리 id)
    pub target: String,
    /// 헤더 텍스트 (예: "「한」 → 한자", "「ㄱ」 → 특수문자", "이모지").
    /// frontend 는 이 문자열을 그대로 헤더 라벨에 적용.
    pub header_text: String,
    /// 그리드 데이터 [row][col], None이면 빈 셀
    pub cells: Vec<Vec<Option<CellData>>>,
    /// 열 헤더 텍스트 (특수문자/이모지: top_row 문자들, 한자 compact: 빈 벡터)
    pub col_headers: Vec<String>,
    /// 열 헤더 활성 플래그 (sel_col 와 같은 열만 true) — col_headers 와 같은 길이
    pub col_header_active: Vec<bool>,
    /// 행 헤더 텍스트 (특수문자/이모지: "1"-"9", 한자: "1."-"9.")
    pub row_headers: Vec<String>,
    /// 행 헤더 활성 플래그 (sel_row 와 같은 행만 true) — row_headers 와 같은 길이
    pub row_header_active: Vec<bool>,
    /// 선택 행
    pub sel_row: usize,
    /// 선택 열
    pub sel_col: usize,
    /// 현재 페이지 (0-based)
    pub current_page: usize,
    /// 전체 페이지 수
    pub total_pages: usize,
    /// 푸터 텍스트 (예: "1/3", "[ㄱ] 1/3", "[Smileys] 1/3").
    /// `show_footer` 가 false 면 이 문자열은 사용하지 않음.
    pub footer_text: String,
    /// 푸터 가시성 — 단일 페이지면 false (◀/▶ 버튼도 함께 hide).
    pub show_footer: bool,
    /// 한자 expand 토글 아이콘 가시성 (한자 popup 만 true, 그 외 false)
    pub expand_visible: bool,
    /// 한자 expand 토글 아이콘 텍스트 ("⊞" compact / "⊟" expanded).
    /// `expand_visible=false` 면 무의미.
    pub expand_text: String,
    /// 이모지 좌측 9 카테고리 탭 라벨 (단축키 prefix 포함, 예: "스마일 (a)").
    /// 이모지 popup 만 비어있지 않음.
    pub tab_labels: Vec<String>,
    /// 활성 탭 인덱스 (이모지 popup 만 의미 있음)
    pub active_tab_index: usize,
}

impl PopupState {
    /// 현재 상태에서 뷰 모델 생성.
    ///
    /// `home_row` 는 이모지 popup 카테고리 단축키 표시용 (active 영문 키맵의
    /// 홈 행 9 문자, 예: "ASDFGHJKL"). 한자/특수문자에선 무시됨. 짧으면
    /// 부족한 카테고리는 단축키 표시 생략.
    pub fn view_model(&self, home_row: &str) -> PopupViewModel {
        match self.kind() {
            PopupKind::SpecialChar => self.special_view_model(),
            PopupKind::Hanja => self.hanja_view_model(),
            PopupKind::Emoji => self.emoji_view_model(home_row),
        }
    }

    /// 이모지 팝업 뷰 모델 — 9×9 그리드 (rows 고정 정책), 좌측 9 카테고리 탭, 푸터에 카테고리 라벨.
    fn emoji_view_model(&self, home_row: &str) -> PopupViewModel {
        let top_row_chars: Vec<char> = self.top_row().chars().collect();
        // col_headers 는 항상 MAX_COLS(9) 개 — page item 수가 9 미만이어도 popup 폭
        // 고정을 위해 가로축 레이블 9개 전부 출력. cells 의 실제 col 수(self.cols())
        // 는 그대로 — 키 처리·인덱싱 정합 보존.
        let col_headers: Vec<String> = (0..MAX_COLS)
            .map(|c| {
                if c < top_row_chars.len() {
                    top_row_chars[c].to_string()
                } else {
                    format!("{}", c + 1)
                }
            })
            .collect();
        let col_header_active: Vec<bool> =
            (0..MAX_COLS).map(|c| c == self.sel_col()).collect();
        let row_headers: Vec<String> = (1..=self.rows()).map(|r| format!("{}", r)).collect();
        let row_header_active: Vec<bool> = (0..self.rows()).map(|r| r == self.sel_row()).collect();

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
                        is_bookmarked: false,
                    }));
                } else {
                    row.push(None);
                }
            }
            cells.push(row);
        }

        // 카테고리 탭 라벨: "{label_ko} ({key})" — 단축키 prefix.
        // home_row 문자가 부족하면 단축키 표시 생략 (key 없음).
        let home_row_chars: Vec<char> = home_row.chars().collect();
        let categories = self.emoji_categories();
        let tab_labels: Vec<String> = categories
            .iter()
            .enumerate()
            .map(|(i, cat)| {
                if let Some(key) = home_row_chars.get(i) {
                    format!("{} ({})", cat.label_ko, key)
                } else {
                    cat.label_ko.clone()
                }
            })
            .collect();

        let cat_label = categories
            .get(self.emoji_cat_index())
            .map(|c| c.label_ko.as_str())
            .unwrap_or("");
        let header_text = format!("「{}」 → 이모지", cat_label);

        let show_footer = self.total_pages() > 1;
        let footer_text = if show_footer {
            format!(
                "[{}]  {} / {}",
                cat_label,
                self.current_page() + 1,
                self.total_pages()
            )
        } else {
            String::new()
        };

        PopupViewModel {
            kind: PopupKind::Emoji,
            target: self.target().to_string(),
            header_text,
            cells,
            col_headers,
            col_header_active,
            row_headers,
            row_header_active,
            sel_row: self.sel_row(),
            sel_col: self.sel_col(),
            current_page: self.current_page(),
            total_pages: self.total_pages(),
            footer_text,
            show_footer,
            expand_visible: false,
            expand_text: String::new(),
            tab_labels,
            active_tab_index: self.emoji_cat_index(),
        }
    }

    fn special_view_model(&self) -> PopupViewModel {
        let top_row_chars: Vec<char> = self.top_row().chars().collect();

        // col_headers 항상 9개 (popup 폭 고정). cells 의 실제 col 수는 self.cols() 유지.
        let col_headers: Vec<String> = (0..MAX_COLS)
            .map(|c| {
                if c < top_row_chars.len() {
                    top_row_chars[c].to_string()
                } else {
                    format!("{}", c + 1)
                }
            })
            .collect();
        let col_header_active: Vec<bool> =
            (0..MAX_COLS).map(|c| c == self.sel_col()).collect();

        let row_headers: Vec<String> = (1..=self.rows()).map(|r| format!("{}", r)).collect();
        let row_header_active: Vec<bool> = (0..self.rows()).map(|r| r == self.sel_row()).collect();

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
                        is_bookmarked: false,
                    }));
                } else {
                    row.push(None);
                }
            }
            cells.push(row);
        }

        let header_text = format!("「{}」 → 특수문자", self.target());
        let show_footer = self.total_pages() > 1;
        let footer_text = if show_footer {
            format!(
                "[{}]  {} / {}",
                self.target(),
                self.current_page() + 1,
                self.total_pages()
            )
        } else {
            String::new()
        };

        PopupViewModel {
            kind: PopupKind::SpecialChar,
            target: self.target().to_string(),
            header_text,
            cells,
            col_headers,
            col_header_active,
            row_headers,
            row_header_active,
            sel_row: self.sel_row(),
            sel_col: self.sel_col(),
            current_page: self.current_page(),
            total_pages: self.total_pages(),
            footer_text,
            show_footer,
            expand_visible: false,
            expand_text: String::new(),
            tab_labels: Vec::new(),
            active_tab_index: 0,
        }
    }

    fn hanja_view_model(&self) -> PopupViewModel {
        let expand_text = if self.is_hanja_expanded() {
            "⊟".to_string()
        } else {
            "⊞".to_string()
        };

        if self.is_hanja_expanded() {
            // expanded 9×9 그리드 — special 과 거의 동일하되 각 셀에 bookmarked 플래그.
            let top_row_chars: Vec<char> = self.top_row().chars().collect();
            // col_headers 항상 9개 (popup 폭 고정). cells col 수는 self.cols() 유지.
            let col_headers: Vec<String> = (0..MAX_COLS)
                .map(|c| {
                    if c < top_row_chars.len() {
                        top_row_chars[c].to_string()
                    } else {
                        format!("{}", c + 1)
                    }
                })
                .collect();
            let col_header_active: Vec<bool> =
                (0..MAX_COLS).map(|c| c == self.sel_col()).collect();
            let row_headers: Vec<String> = (1..=self.rows()).map(|r| format!("{}", r)).collect();
            let row_header_active: Vec<bool> =
                (0..self.rows()).map(|r| r == self.sel_row()).collect();

            let mut cells = Vec::with_capacity(self.rows());
            for r in 0..self.rows() {
                let mut row = Vec::with_capacity(self.cols());
                for c in 0..self.cols() {
                    if let Some(global) = self.hanja_global_index_rc(r, c) {
                        let hanja = self.get_item(global).unwrap_or("").to_string();
                        let meaning = self.get_meaning(global).unwrap_or("").to_string();
                        row.push(Some(CellData {
                            text: hanja,
                            meaning: if meaning.is_empty() { None } else { Some(meaning) },
                            is_selected: r == self.sel_row() && c == self.sel_col(),
                            is_col_highlight: c == self.sel_col(),
                            is_row_highlight: r == self.sel_row(),
                            is_bookmarked: self.is_bookmarked(global),
                        }));
                    } else {
                        row.push(None);
                    }
                }
                cells.push(row);
            }

            // 확장 모드 헤더는 선택된 한자의 뜻을 표시 — `「target」 → {hanja}  {meaning}`.
            // 빈 셀에 커서가 있을 경우(81 미만 페이지) 기본 헤더로 폴백.
            let header_text = if let Some(global) = self.hanja_global_index_rc(self.sel_row(), self.sel_col()) {
                let hanja = self.get_item(global).unwrap_or("");
                let meaning = self.get_meaning(global).unwrap_or("");
                if meaning.is_empty() {
                    format!("「{}」 → {}", self.target(), hanja)
                } else {
                    format!("「{}」 → {}  {}", self.target(), hanja, meaning)
                }
            } else {
                format!("「{}」 → 한자", self.target())
            };

            // 페이지 네비 푸터는 항상 가시 — popup 크기 안정 (한자 popup 정책).
            let show_footer = true;
            let footer_text = format!("{}/{}", self.current_page() + 1, self.total_pages());

            PopupViewModel {
                kind: PopupKind::Hanja,
                target: self.target().to_string(),
                header_text,
                cells,
                col_headers,
                col_header_active,
                row_headers,
                row_header_active,
                sel_row: self.sel_row(),
                sel_col: self.sel_col(),
                current_page: self.current_page(),
                total_pages: self.total_pages(),
                footer_text,
                show_footer,
                expand_visible: true,
                expand_text,
                tab_labels: Vec::new(),
                active_tab_index: 0,
            }
        } else {
            // compact 1×N 리스트
            let items = self.hanja_page_items();
            let row_headers: Vec<String> = (1..=items.len()).map(|r| format!("{}.", r)).collect();
            let row_header_active: Vec<bool> =
                (0..items.len()).map(|i| i == self.sel_row()).collect();

            let mut cells = Vec::with_capacity(items.len());
            for (i, (hanja, meaning)) in items.iter().enumerate() {
                let global = self.current_page() * super::popup_keys::HANJA_PAGE_SIZE + i;
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
                    is_bookmarked: self.is_bookmarked(global),
                };
                cells.push(vec![Some(cell)]);
            }

            // 페이지 네비 푸터는 항상 가시 — popup 크기 안정 (한자 popup 정책).
            let show_footer = true;
            let footer_text = format!("{}/{}", self.current_page() + 1, self.total_pages());

            // compact 모드 헤더는 기본형 — 각 행에 hanja+meaning 인라인 표시되므로 헤더에 별도 표기 불필요.
            let header_text = format!("「{}」 → 한자", self.target());

            PopupViewModel {
                kind: PopupKind::Hanja,
                target: self.target().to_string(),
                header_text,
                cells,
                col_headers: Vec::new(),
                col_header_active: Vec::new(),
                row_headers,
                row_header_active,
                sel_row: self.sel_row(),
                sel_col: 0,
                current_page: self.current_page(),
                total_pages: self.total_pages(),
                footer_text,
                show_footer,
                expand_visible: true,
                expand_text,
                tab_labels: Vec::new(),
                active_tab_index: 0,
            }
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
        let vm = state.view_model("");
        assert_eq!(vm.kind, PopupKind::SpecialChar);
        assert_eq!(vm.target, "ㄱ");
        assert_eq!(vm.col_headers.len(), 9); // popup 폭 고정 정책 — 항상 9
        assert_eq!(vm.row_headers.len(), 9); // 9행 (고정)
        assert_eq!(vm.col_headers[0], "Q");
        assert_eq!(vm.col_headers[1], "W");
        assert_eq!(vm.col_headers[2], "E");
        assert_eq!(vm.col_headers[8], "O"); // top_row(QWERTYUIO) 의 9번째
        assert_eq!(vm.row_headers[0], "1");
        assert_eq!(vm.cells.len(), 9); // 9행 (고정)
        assert!(vm.cells[0][0].is_some());
        assert_eq!(vm.cells[0][0].as_ref().unwrap().text, "S0");
        assert!(vm.cells[0][0].as_ref().unwrap().is_selected); // (0,0) 선택
        // col=2 row=2 이상은 빈 셀 (S20 없음 → idx 2*9+2=20 >= 20)
        assert!(vm.cells[2][2].is_none());
        // 단일 페이지: show_footer=false, footer_text=""
        assert!(!vm.show_footer);
        assert_eq!(vm.footer_text, "");
        // 헤더는 daemon 측 SoT — "「ㄱ」 → 특수문자"
        assert_eq!(vm.header_text, "「ㄱ」 → 특수문자");
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
        let vm = state.view_model("");
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
        let vm = state.view_model("");
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
        let vm = state.view_model("");
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
        let vm = state.view_model("");
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
        let vm = state.view_model("");
        assert!(vm.cells[0][0].as_ref().unwrap().meaning.is_none());
    }

    #[test]
    fn hanja_view_model_footer() {
        // 20 개 후보 → compact 9/page → 3 pages → footer "1/3" + show_footer=true.
        // 헤더는 별도 필드 "「한」 → 한자".
        let candidates: Vec<(String, String)> = (0..20)
            .map(|i| (format!("漢{}", i), format!("뜻{}", i)))
            .collect();
        let state = PopupState::new_hanja("한", candidates);
        let vm = state.view_model("");
        assert!(vm.show_footer);
        assert_eq!(vm.footer_text, "1/3");
        assert_eq!(vm.header_text, "「한」 → 한자");
        // compact 모드: expand 아이콘은 ⊞ (확장 가능), 가시
        assert!(vm.expand_visible);
        assert_eq!(vm.expand_text, "⊞");
    }

    #[test]
    fn hanja_view_model_expanded_footer() {
        // 100 개 후보 → expanded 81/page → 2 pages.
        let candidates: Vec<(String, String)> = (0..100)
            .map(|i| (format!("漢{}", i), format!("뜻{}", i)))
            .collect();
        let mut state = PopupState::new_hanja("한", candidates);
        state.handle_key(super::super::PopupKey::Period); // expanded
        let vm = state.view_model("");
        assert!(vm.show_footer);
        assert_eq!(vm.footer_text, "1/2");
        assert_eq!(vm.expand_text, "⊟"); // expanded → 축소 아이콘
    }

    #[test]
    fn hanja_view_model_footer_always_shown_single_page() {
        // 5 개 후보 (compact 9/page → 1 page) — 단일 페이지여도 footer 가시 + "1/1".
        // popup 크기 안정 정책 (한자 popup 한정).
        let candidates: Vec<(String, String)> = (0..5)
            .map(|i| (format!("漢{}", i), format!("뜻{}", i)))
            .collect();
        let state = PopupState::new_hanja("한", candidates);
        let vm = state.view_model("");
        assert!(vm.show_footer, "단일 페이지여도 푸터 가시");
        assert_eq!(vm.footer_text, "1/1");
    }

    #[test]
    fn hanja_view_model_expanded_header_with_meaning() {
        // expanded 모드 헤더는 선택된 한자의 뜻을 포함 — `「target」 → {hanja}  {meaning}`.
        let candidates: Vec<(String, String)> = vec![
            ("韓".to_string(), "나라 한".to_string()),
            ("漢".to_string(), "한나라 한".to_string()),
        ];
        let mut state = PopupState::new_hanja("한", candidates);
        state.handle_key(super::super::PopupKey::Period); // expanded
        let vm = state.view_model("");
        // 초기 sel = (0,0) → 첫 번째 후보 "韓" 선택
        assert_eq!(vm.header_text, "「한」 → 韓  나라 한");
    }

    #[test]
    fn hanja_view_model_expanded_header_meaning_changes_on_selection() {
        let candidates: Vec<(String, String)> = vec![
            ("韓".to_string(), "나라 한".to_string()),
            ("漢".to_string(), "한나라 한".to_string()),
        ];
        let mut state = PopupState::new_hanja("한", candidates);
        state.handle_key(super::super::PopupKey::Period); // expanded
        // expanded 9×9 column-major: idx 0=(0,0) 韓, idx 1=(1,0) 漢
        state.handle_key(super::super::PopupKey::Down); // (1, 0) → 漢
        let vm = state.view_model("");
        assert_eq!(vm.header_text, "「한」 → 漢  한나라 한");
    }

    #[test]
    fn hanja_view_model_expanded_header_no_meaning() {
        // 의미 비어있을 때: `「target」 → {hanja}` (meaning 없이).
        let candidates: Vec<(String, String)> = vec![("韓".to_string(), "".to_string())];
        let mut state = PopupState::new_hanja("한", candidates);
        state.handle_key(super::super::PopupKey::Period);
        let vm = state.view_model("");
        assert_eq!(vm.header_text, "「한」 → 韓");
    }
}
