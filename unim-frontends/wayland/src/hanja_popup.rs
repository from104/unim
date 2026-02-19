//! 한자 후보 팝업 모듈
//!
//! XIM hanja_window.rs 패턴을 Wayland + tiny-skia로 재구현합니다.
//! 9개씩 페이지로 나누어 "번호. 漢 뜻" 형태로 표시합니다.

use cosmic_text::FontSystem;
use cosmic_text::SwashCache;
use tiny_skia::Pixmap;

use crate::popup::{draw_rect, draw_text, sel_bg_color, PAGE_COLOR, SEL_TEXT_COLOR, TEXT_COLOR};

/// 페이지당 표시할 후보 수
pub const PAGE_SIZE: usize = 9;

/// 한자 팝업에서 키 처리 결과
pub enum HanjaAction {
    /// 후보 선택 (전체 인덱스)
    Select(u32),
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

/// 한자 후보 팝업 상태
pub struct HanjaPopup {
    /// 후보 목록 (한자, 뜻)
    candidates: Vec<(String, String)>,
    /// 대상 문자열
    target: String,
    /// 현재 페이지 (0-based)
    current_page: usize,
    /// 선택된 인덱스 (페이지 내)
    selected_index: usize,
}

impl HanjaPopup {
    pub fn new() -> Self {
        Self {
            candidates: Vec::new(),
            target: String::new(),
            current_page: 0,
            selected_index: 0,
        }
    }

    /// 후보 설정
    pub fn set_candidates(&mut self, target: &str, candidates: Vec<(String, String)>) {
        self.target = target.to_string();
        self.candidates = candidates;
        self.current_page = 0;
        self.selected_index = 0;
    }

    /// 후보가 있는지
    #[allow(dead_code)]
    pub fn has_candidates(&self) -> bool {
        !self.candidates.is_empty()
    }

    /// 총 페이지 수
    fn total_pages(&self) -> usize {
        if self.candidates.is_empty() {
            1
        } else {
            (self.candidates.len() + PAGE_SIZE - 1) / PAGE_SIZE
        }
    }

    /// 현재 페이지 항목
    fn page_items(&self) -> &[(String, String)] {
        let start = self.current_page * PAGE_SIZE;
        let end = (start + PAGE_SIZE).min(self.candidates.len());
        if start >= self.candidates.len() {
            &[]
        } else {
            &self.candidates[start..end]
        }
    }

    /// 전체 인덱스로 한자 가져오기
    #[allow(dead_code)]
    pub fn get_candidate(&self, index: usize) -> Option<&str> {
        self.candidates.get(index).map(|(h, _)| h.as_str())
    }

    /// 팝업 크기 계산 (width, height)
    pub fn popup_size(&self) -> (u32, u32) {
        let items = self.page_items().len().max(1);
        let line_h: u32 = 28;
        let height = (items as u32 + 1) * line_h + 12; // +1 for page info + padding
        (320, height)
    }

    /// 렌더링
    pub fn render(
        &self,
        pixmap: &mut Pixmap,
        font_system: &mut FontSystem,
        swash_cache: &mut SwashCache,
    ) {
        let items = self.page_items();
        let line_h: f32 = 28.0;
        let padding: f32 = 6.0;
        let font_size: f32 = 15.0;

        for (i, (hanja, meaning)) in items.iter().enumerate() {
            let y = padding + (i as f32) * line_h;

            // 선택 배경
            if i == self.selected_index {
                draw_rect(
                    pixmap,
                    0.0,
                    y,
                    pixmap.width() as f32,
                    line_h,
                    sel_bg_color(),
                );
            }

            let text = format!("{}. {} {}", i + 1, hanja, meaning);
            let color = if i == self.selected_index {
                SEL_TEXT_COLOR
            } else {
                TEXT_COLOR
            };

            draw_text(
                pixmap,
                font_system,
                swash_cache,
                padding,
                y + 2.0,
                300.0,
                &text,
                font_size,
                color,
            );
        }

        // 페이지 정보
        let page_y = padding + (items.len() as f32) * line_h + 4.0;
        let page_text = format!(
            "[{}]  {} / {}",
            self.target,
            self.current_page + 1,
            self.total_pages()
        );
        draw_text(
            pixmap,
            font_system,
            swash_cache,
            padding,
            page_y,
            300.0,
            &page_text,
            12.0,
            PAGE_COLOR,
        );
    }

    /// 키 처리
    pub fn handle_key(&mut self, keysym: u32) -> HanjaAction {
        match keysym {
            // 숫자 1-9
            0x31..=0x39 => {
                let idx = (keysym - 0x31) as usize;
                if idx < self.page_items().len() {
                    let global_idx = self.current_page * PAGE_SIZE + idx;
                    HanjaAction::Select(global_idx as u32)
                } else {
                    HanjaAction::None
                }
            }
            // Escape
            0xff1b => HanjaAction::Cancel,
            // Right / Space → 다음 페이지
            0xff53 | 0x20 => {
                if self.total_pages() > 1 {
                    self.current_page = (self.current_page + 1) % self.total_pages();
                    self.selected_index = 0;
                }
                HanjaAction::NextPage
            }
            // Left / BackSpace → 이전 페이지
            0xff51 | 0xff08 => {
                if self.total_pages() > 1 {
                    self.current_page = if self.current_page > 0 {
                        self.current_page - 1
                    } else {
                        self.total_pages() - 1
                    };
                    self.selected_index = 0;
                }
                HanjaAction::PrevPage
            }
            // Down
            0xff54 => {
                let count = self.page_items().len();
                if count > 0 {
                    self.selected_index = (self.selected_index + 1) % count;
                }
                HanjaAction::Consumed
            }
            // Up
            0xff52 => {
                let count = self.page_items().len();
                if count > 0 {
                    self.selected_index = if self.selected_index == 0 {
                        count - 1
                    } else {
                        self.selected_index - 1
                    };
                }
                HanjaAction::Consumed
            }
            // Enter
            0xff0d => {
                let count = self.page_items().len();
                if count > 0 && self.selected_index < count {
                    let global_idx = self.current_page * PAGE_SIZE + self.selected_index;
                    HanjaAction::Select(global_idx as u32)
                } else {
                    HanjaAction::Consumed
                }
            }
            // 모디파이어 키 무시
            0xffe1..=0xffee | 0xff7f | 0xff14 => HanjaAction::Consumed,
            _ => HanjaAction::None,
        }
    }
}
