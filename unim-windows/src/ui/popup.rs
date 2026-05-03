//! 한자/특수문자 팝업 UI

use unim::input_engine::PopupAction;

/// 마우스 페이지 이동 클릭 결과 (Phase 9). app.rs 가 받아 engine.popup_change_page 호출.
#[derive(Debug, Clone, Copy)]
pub enum PopupClickAction {
    PrevPage,
    NextPage,
}

/// 팝업 표시 상태
pub struct PopupState {
    pub visible: bool,
    pub title: String,
    pub candidates: Vec<String>,
    pub descriptions: Vec<String>,
    pub page: usize,
    pub total_pages: usize,
    pub selected: usize,
    pub rows: usize,
    pub cols: usize,
    /// 한자 팝업이면 true (★ 해제 flash 적용용). title prefix 보다 명시적인 표시. Phase 9.
    pub is_hanja: bool,
    /// ★ 해제 flash 종료 시각 (Phase 9). 이 시각 이전이면 selected 셀에 yellow 배경.
    pub flash_until: Option<std::time::Instant>,
}

impl Default for PopupState {
    fn default() -> Self {
        Self {
            visible: false,
            title: String::new(),
            candidates: Vec::new(),
            descriptions: Vec::new(),
            page: 0,
            total_pages: 1,
            selected: 0,
            rows: 3,
            cols: 3,
            is_hanja: false,
            flash_until: None,
        }
    }
}

impl PopupState {
    /// PopupAction에 따라 팝업 상태를 업데이트합니다.
    pub fn handle_action(&mut self, action: PopupAction) {
        match action {
            PopupAction::ShowHanja {
                target, candidates, ..
            } => {
                self.visible = true;
                self.title = format!("한자: {}", target);
                self.candidates = candidates.iter().map(|(c, _)| c.clone()).collect();
                self.descriptions = candidates.iter().map(|(_, d)| d.clone()).collect();
                self.selected = 0;
                self.page = 0;
                self.is_hanja = true;
                self.flash_until = None;
            }
            PopupAction::ShowSpecial {
                target, characters, ..
            } => {
                self.visible = true;
                self.title = format!("특수문자: {}", target);
                self.candidates = characters;
                self.descriptions = Vec::new();
                self.selected = 0;
                self.page = 0;
                self.is_hanja = false;
                self.flash_until = None;
            }
            PopupAction::HidePopup => {
                self.visible = false;
                self.candidates.clear();
                self.descriptions.clear();
                self.is_hanja = false;
                self.flash_until = None;
            }
            PopupAction::PopupNavigate {
                page,
                total_pages,
                selected,
                rows,
                cols,
                ..
            } => {
                self.page = page;
                self.total_pages = total_pages;
                self.selected = selected;
                self.rows = rows;
                self.cols = cols;
            }
            // 한자 즐겨찾기 토글: 시각 갱신 대상 아니나 was_bookmarked 페이로드를 보고
            // un-bookmark (★ 해제) 시 cursor 셀 140ms yellow flash 시작 (Phase 9).
            PopupAction::HanjaBookmarkChanged { .. } => {}
            PopupAction::HanjaCandidatesReordered {
                page,
                sel_row,
                sel_col,
                bookmarked,
                ..
            } => {
                // Phase 9: ★ 해제 (was→false) 시 cursor 셀 flash. enum payload 의 `bookmarked`
                // 가 false 이면 OFF 결과 → flash. (XIM/GTK/Qt와 동일 정책 — 단순화하여
                // was_bookmarked 비교 대신 bookmarked==false 로 분기.)
                self.page = page as usize;
                self.selected = (page as usize) * (self.rows * self.cols.max(1))
                    + (sel_row as usize) * self.cols.max(1)
                    + sel_col as usize;
                if !bookmarked {
                    self.flash_until = Some(
                        std::time::Instant::now() + std::time::Duration::from_millis(140),
                    );
                }
            }
            // 페이지 점프 (마우스 ◀/▶ 또는 키보드 PageDown/PageUp) — page_index 만 업데이트.
            // selected/sel_row/sel_col 은 후속 PopupNavigate 시그널이 갱신한다 (XIM/GTK 동일).
            PopupAction::PageJump { page_index } => {
                self.page = page_index as usize;
            }
            // 이모지 팝업은 Windows standalone egui UI에서 미지원 — 무시
            PopupAction::ShowEmoji { .. } => {}
        }
    }

    /// 팝업 UI를 그립니다. 마우스 ◀/▶ 클릭 시 [`PopupClickAction`] 반환 (Phase 9).
    pub fn show(&mut self, ctx: &egui::Context) -> Option<PopupClickAction> {
        if !self.visible || self.candidates.is_empty() {
            return None;
        }

        let mut click_action: Option<PopupClickAction> = None;
        // flash 활성 시 다음 프레임에서 자연스럽게 deadline 검사 — egui repaint 트리거.
        let flash_active = self
            .flash_until
            .map(|t| std::time::Instant::now() < t)
            .unwrap_or(false);
        if flash_active {
            ctx.request_repaint_after(std::time::Duration::from_millis(140));
        }

        let multi_page = self.total_pages > 1;

        egui::Window::new(&self.title)
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, egui::vec2(0.0, 0.0))
            .show(ctx, |ui| {
                let cols = self.cols.max(1);
                egui::Grid::new("popup_grid")
                    .min_col_width(40.0)
                    .spacing(egui::vec2(4.0, 4.0))
                    .show(ui, |ui| {
                        let start = self.page * (self.rows * cols);
                        let end = (start + self.rows * cols).min(self.candidates.len());
                        for (i, idx) in (start..end).enumerate() {
                            let is_selected = idx == self.selected;
                            let text = &self.candidates[idx];
                            // Phase 9: hanja popup 의 selected 셀에 flash 활성화 시 yellow 배경.
                            let label = if is_selected && self.is_hanja && flash_active {
                                egui::RichText::new(text)
                                    .strong()
                                    .background_color(egui::Color32::from_rgb(
                                        0xf9, 0xe2, 0xaf,
                                    ))
                                    .color(egui::Color32::BLACK)
                            } else if is_selected {
                                egui::RichText::new(text)
                                    .strong()
                                    .background_color(egui::Color32::from_rgb(0, 120, 215))
                                    .color(egui::Color32::WHITE)
                            } else {
                                egui::RichText::new(text)
                            };
                            ui.label(label);
                            if (i + 1) % cols == 0 {
                                ui.end_row();
                            }
                        }
                    });

                ui.separator();
                ui.horizontal(|ui| {
                    // Phase 9: 마우스 ◀/▶ 페이지 버튼 (단일 페이지면 hide).
                    if multi_page {
                        if ui.button("◀").clicked() {
                            click_action = Some(PopupClickAction::PrevPage);
                        }
                    }
                    ui.label(format!(
                        "{}/{}",
                        self.page + 1,
                        self.total_pages.max(1)
                    ));
                    if multi_page {
                        if ui.button("▶").clicked() {
                            click_action = Some(PopupClickAction::NextPage);
                        }
                    }
                    ui.separator();
                    ui.label("←→ 이동  Enter 선택  Esc 취소");
                });
            });

        click_action
    }
}
