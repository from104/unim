//! 한자/특수문자 팝업 UI

use unim::input_engine::PopupAction;

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
            }
            PopupAction::HidePopup => {
                self.visible = false;
                self.candidates.clear();
                self.descriptions.clear();
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
            // 한자 즐겨찾기 토글은 standalone egui UI에서 시각적 갱신 대상이 아님 — 무시
            PopupAction::HanjaBookmarkChanged { .. } => {}
            // 한자 후보 재정렬도 Windows standalone egui UI에서 시각적 동기화 대상이 아님
            PopupAction::HanjaCandidatesReordered { .. } => {}
            // 페이지 점프 (마우스 ◀/▶) 는 Windows standalone egui UI 에서 시각 동기화 대상 아님
            PopupAction::PageJump { .. } => {}
            // 이모지 팝업은 Windows standalone egui UI에서 미지원 — 무시
            PopupAction::ShowEmoji { .. } => {}
        }
    }

    /// 팝업 UI를 그립니다.
    pub fn show(&self, ctx: &egui::Context) {
        if !self.visible || self.candidates.is_empty() {
            return;
        }

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
                            let label = if is_selected {
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
                    ui.label(format!(
                        "페이지 {}/{}",
                        self.page + 1,
                        self.total_pages.max(1)
                    ));
                    ui.label("←→ 이동  Enter 선택  Esc 취소");
                });
            });
    }
}
