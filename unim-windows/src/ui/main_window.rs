//! 메인 텍스트 입력 창 UI

/// 텍스트 입력 영역의 상태
pub struct TextArea {
    /// 확정된 텍스트 (committed)
    pub committed_text: String,
    /// 현재 조합 중인 텍스트 (preedit)
    pub preedit: String,
    /// 커서 위치 (committed_text 내 문자 인덱스)
    pub cursor_pos: usize,
    /// 선택 시작 위치 (None이면 선택 없음)
    pub selection_start: Option<usize>,
}

impl Default for TextArea {
    fn default() -> Self {
        Self {
            committed_text: String::new(),
            preedit: String::new(),
            cursor_pos: 0,
            selection_start: None,
        }
    }
}

impl TextArea {
    /// 커밋된 텍스트를 커서 위치에 삽입합니다.
    pub fn insert_commit(&mut self, text: &str) {
        // 선택 영역이 있으면 먼저 삭제
        self.delete_selection();

        let byte_pos = self.char_to_byte_pos(self.cursor_pos);
        self.committed_text.insert_str(byte_pos, text);
        self.cursor_pos += text.chars().count();
    }

    /// preedit 텍스트를 업데이트합니다.
    pub fn update_preedit(&mut self, preedit: &str) {
        self.preedit = preedit.to_string();
    }

    /// 백스페이스 처리 (엔진이 소비하지 않은 경우)
    pub fn handle_backspace(&mut self) {
        if self.delete_selection() {
            return;
        }
        if self.cursor_pos > 0 {
            self.cursor_pos -= 1;
            let byte_pos = self.char_to_byte_pos(self.cursor_pos);
            let next_byte = self.char_to_byte_pos(self.cursor_pos + 1);
            self.committed_text.drain(byte_pos..next_byte);
        }
    }

    /// Delete 키 처리
    pub fn handle_delete(&mut self) {
        if self.delete_selection() {
            return;
        }
        let char_count = self.committed_text.chars().count();
        if self.cursor_pos < char_count {
            let byte_pos = self.char_to_byte_pos(self.cursor_pos);
            let next_byte = self.char_to_byte_pos(self.cursor_pos + 1);
            self.committed_text.drain(byte_pos..next_byte);
        }
    }

    /// 선택 영역을 삭제합니다. 선택이 있었으면 true 반환.
    fn delete_selection(&mut self) -> bool {
        if let Some(sel_start) = self.selection_start.take() {
            let start = sel_start.min(self.cursor_pos);
            let end = sel_start.max(self.cursor_pos);
            let start_byte = self.char_to_byte_pos(start);
            let end_byte = self.char_to_byte_pos(end);
            self.committed_text.drain(start_byte..end_byte);
            self.cursor_pos = start;
            true
        } else {
            false
        }
    }

    /// 선택된 텍스트를 반환합니다.
    pub fn selected_text(&self) -> Option<String> {
        self.selection_start.map(|sel_start| {
            let start = sel_start.min(self.cursor_pos);
            let end = sel_start.max(self.cursor_pos);
            self.committed_text
                .chars()
                .skip(start)
                .take(end - start)
                .collect()
        })
    }

    /// 선택 영역을 새 텍스트로 교체합니다 (수동 한영타 변환용).
    pub fn replace_selection(&mut self, replacement: &str) {
        if self.selection_start.is_some() {
            self.delete_selection();
            self.insert_commit(replacement);
        }
    }

    /// 문자 인덱스 → 바이트 위치 변환
    pub fn char_to_byte_pos(&self, char_pos: usize) -> usize {
        self.committed_text
            .char_indices()
            .nth(char_pos)
            .map(|(i, _)| i)
            .unwrap_or(self.committed_text.len())
    }

    /// 전체 표시 텍스트 (committed + preedit) 를 egui UI에 렌더링합니다.
    pub fn show(&mut self, ui: &mut egui::Ui) {
        let available_width = ui.available_width();
        let frame = egui::Frame::new()
            .inner_margin(egui::Margin::same(8))
            .stroke(egui::Stroke::new(
                1.0,
                egui::Color32::from_rgb(180, 180, 180),
            ))
            .corner_radius(4);

        frame.show(ui, |ui| {
            ui.set_min_height(200.0);
            ui.set_min_width(available_width - 16.0);

            // 커밋된 텍스트 + preedit 합성 표시
            let before_cursor: String = self.committed_text.chars().take(self.cursor_pos).collect();
            let after_cursor: String = self.committed_text.chars().skip(self.cursor_pos).collect();

            let mut job = egui::text::LayoutJob::default();

            // 커서 앞 텍스트
            if !before_cursor.is_empty() {
                job.append(
                    &before_cursor,
                    0.0,
                    egui::TextFormat {
                        font_id: egui::FontId::proportional(16.0),
                        color: ui.visuals().text_color(),
                        ..Default::default()
                    },
                );
            }

            // preedit (조합 중 문자) — 밑줄 + 파란색
            if !self.preedit.is_empty() {
                job.append(
                    &self.preedit,
                    0.0,
                    egui::TextFormat {
                        font_id: egui::FontId::proportional(16.0),
                        color: egui::Color32::from_rgb(0, 100, 200),
                        underline: egui::Stroke::new(2.0, egui::Color32::from_rgb(0, 100, 200)),
                        ..Default::default()
                    },
                );
            }

            // 커서 뒤 텍스트
            if !after_cursor.is_empty() {
                job.append(
                    &after_cursor,
                    0.0,
                    egui::TextFormat {
                        font_id: egui::FontId::proportional(16.0),
                        color: ui.visuals().text_color(),
                        ..Default::default()
                    },
                );
            }

            // 빈 상태일 때 placeholder
            if before_cursor.is_empty() && self.preedit.is_empty() && after_cursor.is_empty() {
                job.append(
                    "여기에 입력하세요... (한/영 전환: RightAlt 또는 한/영 키)",
                    0.0,
                    egui::TextFormat {
                        font_id: egui::FontId::proportional(14.0),
                        color: egui::Color32::from_rgb(160, 160, 160),
                        ..Default::default()
                    },
                );
            }

            job.wrap = egui::text::TextWrapping {
                max_width: available_width - 32.0,
                ..Default::default()
            };

            ui.label(job);
        });
    }
}
