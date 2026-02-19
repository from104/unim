//! 특수문자 그리드 팝업 모듈
//!
//! XIM special_window.rs 패턴을 Wayland + tiny-skia로 재구현합니다.
//! 9×9 그리드로 특수문자를 표시합니다.

use cosmic_text::FontSystem;
use cosmic_text::SwashCache;
use tiny_skia::Pixmap;

use crate::popup::{
    draw_rect, draw_text, sel_bg_color, HEADER_COLOR, PAGE_COLOR, SEL_TEXT_COLOR, TEXT_COLOR,
};

/// 그리드 상수
const MAX_ROWS: usize = 9;
const MAX_COLS: usize = 9;
const PAGE_SIZE: usize = MAX_ROWS * MAX_COLS; // 81

/// 특수문자 팝업에서 키 처리 결과
pub enum SpecialAction {
    /// 인덱스로 문자 선택 (전체 인덱스)
    Select(usize),
    /// 취소 (Esc)
    Cancel,
    /// 다음 페이지
    NextPage,
    /// 이전 페이지
    PrevPage,
    /// 내부 처리됨 (리드로우 필요)
    Consumed,
    /// 처리하지 않음 → 팝업 닫기
    None,
}

/// 특수문자 그리드 팝업 상태
pub struct SpecialPopup {
    /// 전체 특수문자 배열
    characters: Vec<String>,
    /// 대상 초성
    target: String,
    /// 상단 행 레이블 ("QWERTYUIO")
    top_row: String,
    /// 현재 페이지 (0-based)
    current_page: usize,
    /// 총 페이지 수
    total_pages: usize,
    /// 현재 페이지 행/열 수
    rows: usize,
    cols: usize,
    /// 선택 커서
    sel_row: usize,
    sel_col: usize,
    /// 셀 크기
    cell_w: f32,
    cell_h: f32,
}

impl SpecialPopup {
    pub fn new() -> Self {
        Self {
            characters: Vec::new(),
            target: String::new(),
            top_row: String::new(),
            current_page: 0,
            total_pages: 1,
            rows: 0,
            cols: 0,
            sel_row: 0,
            sel_col: 0,
            cell_w: 36.0,
            cell_h: 28.0,
        }
    }

    /// 후보가 있는지
    #[allow(dead_code)]
    pub fn has_characters(&self) -> bool {
        !self.characters.is_empty()
    }

    /// 특수문자 설정
    pub fn set_characters(&mut self, target: &str, characters: Vec<String>, top_row: &str) {
        self.target = target.to_string();
        self.characters = characters;
        self.top_row = top_row.to_string();
        self.current_page = 0;
        self.sel_row = 0;
        self.sel_col = 0;
        self.total_pages = if self.characters.is_empty() {
            1
        } else {
            (self.characters.len() + PAGE_SIZE - 1) / PAGE_SIZE
        };
        self.update_page_layout();
    }

    /// 전체 인덱스로 문자 가져오기
    #[allow(dead_code)]
    pub fn get_character(&self, index: usize) -> Option<&str> {
        self.characters.get(index).map(|s| s.as_str())
    }

    /// 페이지 레이아웃 계산
    fn update_page_layout(&mut self) {
        let page_start = self.current_page * PAGE_SIZE;
        let page_chars = if page_start >= self.characters.len() {
            0
        } else {
            (self.characters.len() - page_start).min(PAGE_SIZE)
        };

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

    /// 선택된 전체 인덱스
    fn selected_global_index(&self) -> Option<usize> {
        let page_start = self.current_page * PAGE_SIZE;
        let idx = self.sel_col * self.rows + self.sel_row;
        let page_chars = if page_start >= self.characters.len() {
            0
        } else {
            (self.characters.len() - page_start).min(PAGE_SIZE)
        };
        if idx < page_chars {
            Some(page_start + idx)
        } else {
            None
        }
    }

    /// 특정 셀의 문자
    fn char_at(&self, row: usize, col: usize) -> Option<&str> {
        let page_start = self.current_page * PAGE_SIZE;
        let idx = col * self.rows + row;
        let global_idx = page_start + idx;
        if global_idx < self.characters.len() {
            Some(&self.characters[global_idx])
        } else {
            None
        }
    }

    /// 팝업 크기 계산
    pub fn popup_size(&self) -> (u32, u32) {
        let header_col_w: f32 = 30.0;
        let header_row_h = self.cell_h;
        let footer_h: f32 = 24.0;
        let w = (header_col_w + (self.cols as f32) * self.cell_w + 10.0) as u32;
        let h = (header_row_h + (self.rows as f32) * self.cell_h + footer_h + 10.0) as u32;
        (w, h)
    }

    /// 렌더링
    pub fn render(
        &self,
        pixmap: &mut Pixmap,
        font_system: &mut FontSystem,
        swash_cache: &mut SwashCache,
    ) {
        let header_col_w: f32 = 30.0;
        let header_row_h = self.cell_h;
        let font_size: f32 = 14.0;
        let top_row_chars: Vec<char> = self.top_row.chars().collect();

        // 1. 열 헤더
        for c in 0..self.cols {
            let label = if c < top_row_chars.len() {
                top_row_chars[c].to_string()
            } else {
                format!("{}", c + 1)
            };
            let color = if c == self.sel_col {
                SEL_TEXT_COLOR
            } else {
                HEADER_COLOR
            };
            let x = header_col_w + (c as f32) * self.cell_w + self.cell_w / 2.0 - 4.0;
            let y = header_row_h - self.cell_h + 2.0;
            draw_text(
                pixmap,
                font_system,
                swash_cache,
                x,
                y,
                self.cell_w,
                &label,
                font_size,
                color,
            );
        }

        // 2. 행 헤더
        for r in 0..self.rows {
            let label = format!("{}", r + 1);
            let color = if r == self.sel_row {
                SEL_TEXT_COLOR
            } else {
                HEADER_COLOR
            };
            let y = header_row_h + (r as f32) * self.cell_h + 2.0;
            draw_text(
                pixmap,
                font_system,
                swash_cache,
                6.0,
                y,
                24.0,
                &label,
                font_size,
                color,
            );
        }

        // 3. 그리드 셀
        for c in 0..self.cols {
            for r in 0..self.rows {
                let cx = header_col_w + (c as f32) * self.cell_w;
                let cy = header_row_h + (r as f32) * self.cell_h;
                let is_selected = r == self.sel_row && c == self.sel_col;

                if let Some(ch) = self.char_at(r, c) {
                    if is_selected {
                        draw_rect(pixmap, cx, cy, self.cell_w, self.cell_h, sel_bg_color());
                    }
                    let color = if is_selected {
                        SEL_TEXT_COLOR
                    } else {
                        TEXT_COLOR
                    };
                    let tx = cx + self.cell_w / 2.0 - 6.0;
                    let ty = cy + 2.0;
                    draw_text(
                        pixmap,
                        font_system,
                        swash_cache,
                        tx,
                        ty,
                        self.cell_w,
                        ch,
                        font_size,
                        color,
                    );
                }
            }
        }

        // 4. 푸터
        let footer_y = header_row_h + (self.rows as f32) * self.cell_h + 4.0;
        let footer = format!(
            "[{}]  {} / {}",
            self.target,
            self.current_page + 1,
            self.total_pages
        );
        draw_text(
            pixmap,
            font_system,
            swash_cache,
            6.0,
            footer_y,
            300.0,
            &footer,
            12.0,
            PAGE_COLOR,
        );
    }

    /// 키 처리
    pub fn handle_key(&mut self, keysym: u32) -> SpecialAction {
        match keysym {
            // Escape
            0xff1b => SpecialAction::Cancel,

            // 열 점프 (q,w,e,r,t,y,u,i,o)
            0x71 | 0x77 | 0x65 | 0x72 | 0x74 | 0x79 | 0x75 | 0x69 | 0x6f => {
                let keys: &[u32] = &[0x71, 0x77, 0x65, 0x72, 0x74, 0x79, 0x75, 0x69, 0x6f];
                if let Some(col_idx) = keys.iter().position(|&k| k == keysym) {
                    if col_idx < self.cols {
                        self.sel_col = col_idx;
                        if self.char_at(self.sel_row, self.sel_col).is_none() {
                            for r in (0..self.rows).rev() {
                                if self.char_at(r, self.sel_col).is_some() {
                                    self.sel_row = r;
                                    break;
                                }
                            }
                        }
                    }
                }
                SpecialAction::Consumed
            }

            // 숫자 1-9 → 행 선택
            0x31..=0x39 => {
                let row_idx = (keysym - 0x31) as usize;
                if row_idx < self.rows && self.char_at(row_idx, self.sel_col).is_some() {
                    self.sel_row = row_idx;
                    if let Some(idx) = self.selected_global_index() {
                        return SpecialAction::Select(idx);
                    }
                }
                SpecialAction::Consumed
            }

            // Enter
            0xff0d => {
                if let Some(idx) = self.selected_global_index() {
                    SpecialAction::Select(idx)
                } else {
                    SpecialAction::Consumed
                }
            }

            // Up
            0xff52 => {
                if self.sel_row > 0 {
                    self.sel_row -= 1;
                } else {
                    self.sel_row = self.rows - 1;
                }
                while self.sel_row > 0 && self.char_at(self.sel_row, self.sel_col).is_none() {
                    self.sel_row -= 1;
                }
                SpecialAction::Consumed
            }
            // Down
            0xff54 => {
                if self.sel_row + 1 < self.rows {
                    self.sel_row += 1;
                } else {
                    self.sel_row = 0;
                }
                if self.char_at(self.sel_row, self.sel_col).is_none() {
                    self.sel_row = 0;
                }
                SpecialAction::Consumed
            }
            // Left
            0xff51 => {
                if self.sel_col > 0 {
                    self.sel_col -= 1;
                } else {
                    self.sel_col = self.cols - 1;
                }
                if self.char_at(self.sel_row, self.sel_col).is_none() {
                    self.sel_row = 0;
                }
                SpecialAction::Consumed
            }
            // Right
            0xff53 => {
                if self.sel_col + 1 < self.cols {
                    self.sel_col += 1;
                } else {
                    self.sel_col = 0;
                }
                if self.char_at(self.sel_row, self.sel_col).is_none() {
                    self.sel_row = 0;
                }
                SpecialAction::Consumed
            }

            // Tab → 다음 페이지
            0xff09 => {
                if self.total_pages > 1 {
                    self.current_page = (self.current_page + 1) % self.total_pages;
                    self.update_page_layout();
                    self.sel_row = 0;
                    self.sel_col = 0;
                }
                SpecialAction::NextPage
            }
            // Shift+Tab (ISO_Left_Tab)
            0xfe20 => {
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
                SpecialAction::PrevPage
            }

            // 모디파이어 키 무시
            0xffe1..=0xffee | 0xff7f | 0xff14 => SpecialAction::Consumed,

            _ => SpecialAction::None,
        }
    }
}
