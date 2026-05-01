//! UNIM Windows 앱 (egui App 트레이트 구현)

use unim::config::Config;
use unim::input_engine::InputEngine;

use crate::input_handler;
use crate::ui::main_window::TextArea;
use crate::ui::popup::PopupState;

/// UNIM Windows 앱 상태
pub struct UnimApp {
    config: Config,
    engine: InputEngine,
    text_area: TextArea,
    popup: PopupState,
    /// 자동 오타 교정으로 삭제할 문자 수와 교체 문자열
    pending_typefix: Option<(u32, String)>,
}

impl UnimApp {
    pub fn new(_cc: &eframe::CreationContext<'_>) -> Self {
        let config = Config::load_from_default_path();
        let engine = InputEngine::new(&config);
        Self {
            config,
            engine,
            text_area: TextArea::default(),
            popup: PopupState::default(),
            pending_typefix: None,
        }
    }

    /// 키 이벤트를 처리합니다.
    fn handle_key_event(&mut self, ctx: &egui::Context) {
        let events: Vec<egui::Event> = ctx.input(|i| i.events.clone());

        for event in &events {
            if let egui::Event::Key {
                key,
                pressed: true,
                modifiers,
                ..
            } = event
            {
                // Ctrl+Shift+Space: 수동 한영타 변환
                if modifiers.ctrl && modifiers.shift && *key == egui::Key::Space {
                    self.handle_manual_typefix();
                    continue;
                }

                // Ctrl+A: 전체 선택
                if modifiers.ctrl && *key == egui::Key::A {
                    let char_count = self.text_area.committed_text.chars().count();
                    if char_count > 0 {
                        self.text_area.selection_start = Some(0);
                        self.text_area.cursor_pos = char_count;
                    }
                    continue;
                }

                // Ctrl+C: 복사
                if modifiers.ctrl && *key == egui::Key::C {
                    if let Some(text) = self.text_area.selected_text() {
                        if let Ok(mut clipboard) = arboard::Clipboard::new() {
                            let _ = clipboard.set_text(&text);
                        }
                    }
                    continue;
                }

                // Ctrl+V: 붙여넣기
                if modifiers.ctrl && *key == egui::Key::V {
                    if let Ok(mut clipboard) = arboard::Clipboard::new() {
                        if let Ok(text) = clipboard.get_text() {
                            self.text_area.insert_commit(&text);
                        }
                    }
                    continue;
                }

                let result =
                    input_handler::process_key(&mut self.engine, &self.config, key, modifiers);

                // 팝업 액션 처리
                if let Some(action) = result.popup_action {
                    self.popup.handle_action(action);
                }

                if result.consumed {
                    // 엔진이 키를 소비함
                    if let Some(ref commit) = result.commit {
                        self.text_area.update_preedit("");
                        self.text_area.insert_commit(commit);
                    }
                    if let Some(ref preedit) = result.preedit {
                        self.text_area.update_preedit(preedit);
                    }
                } else {
                    // 엔진이 소비하지 않은 키 → 직접 처리
                    // commit이 함께 올 수 있음 (Enter, Tab 등)
                    if let Some(ref commit) = result.commit {
                        self.text_area.update_preedit("");
                        self.text_area.insert_commit(commit);
                    }
                    match key {
                        egui::Key::Backspace if !modifiers.ctrl => {
                            self.text_area.handle_backspace();
                        }
                        egui::Key::Delete => {
                            self.text_area.handle_delete();
                        }
                        egui::Key::Enter => {
                            self.text_area.insert_commit("\n");
                        }
                        egui::Key::ArrowLeft => {
                            if self.text_area.cursor_pos > 0 {
                                self.text_area.cursor_pos -= 1;
                            }
                            self.text_area.selection_start = None;
                        }
                        egui::Key::ArrowRight => {
                            let len = self.text_area.committed_text.chars().count();
                            if self.text_area.cursor_pos < len {
                                self.text_area.cursor_pos += 1;
                            }
                            self.text_area.selection_start = None;
                        }
                        egui::Key::Home => {
                            self.text_area.cursor_pos = 0;
                            self.text_area.selection_start = None;
                        }
                        egui::Key::End => {
                            self.text_area.cursor_pos =
                                self.text_area.committed_text.chars().count();
                            self.text_area.selection_start = None;
                        }
                        _ => {}
                    }
                }

                // 자동 오타 교정 처리
                self.check_auto_typefix();
            }
        }
    }

    /// 자동 한영타 오타 교정을 확인하고 적용합니다.
    fn check_auto_typefix(&mut self) {
        // AutoTypeFix는 엔진의 press_key 내부에서 처리되고
        // commit/preedit으로 결과가 반환됨.
        // pending_typefix가 있으면 적용
        if let Some((delete_count, replacement)) = self.pending_typefix.take() {
            // 커서 위치에서 delete_count만큼 뒤로 삭제 후 replacement 삽입
            let delete = delete_count as usize;
            if self.text_area.cursor_pos >= delete {
                self.text_area.cursor_pos -= delete;
                let start_byte = self.text_area.char_to_byte_pos(self.text_area.cursor_pos);
                let end_byte = self
                    .text_area
                    .char_to_byte_pos(self.text_area.cursor_pos + delete);
                self.text_area.committed_text.drain(start_byte..end_byte);
            }
            self.text_area.insert_commit(&replacement);
        }
    }

    /// 수동 한영타 변환을 수행합니다 (Ctrl+Shift+Space).
    fn handle_manual_typefix(&mut self) {
        if self.text_area.selected_text().is_some() {
            // surrounding text 설정
            let text = self.text_area.committed_text.clone();
            let cursor = self.text_area.cursor_pos as u32;
            let anchor = self
                .text_area
                .selection_start
                .unwrap_or(self.text_area.cursor_pos) as u32;
            self.engine.set_surrounding_text(text, cursor, anchor);

            // 자동 감지 방향으로 변환
            if let Some((_offset_from_cursor, _delete_count, replacement)) =
                self.engine.typefix_convert(0)
            {
                self.text_area.replace_selection(&replacement);
            }
        }
    }
}

impl eframe::App for UnimApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // 키 이벤트 처리
        self.handle_key_event(ctx);

        // 상단 메뉴 바
        egui::TopBottomPanel::top("menu_bar").show(ctx, |ui| {
            egui::menu::bar(ui, |ui| {
                ui.menu_button("파일", |ui| {
                    if ui.button("텍스트 복사 (Ctrl+C)").clicked() {
                        if let Ok(mut clipboard) = arboard::Clipboard::new() {
                            let _ = clipboard.set_text(&self.text_area.committed_text);
                        }
                        ui.close_menu();
                    }
                    if ui.button("붙여넣기 (Ctrl+V)").clicked() {
                        if let Ok(mut clipboard) = arboard::Clipboard::new() {
                            if let Ok(text) = clipboard.get_text() {
                                self.text_area.insert_commit(&text);
                            }
                        }
                        ui.close_menu();
                    }
                    ui.separator();
                    if ui.button("지우기").clicked() {
                        self.text_area.committed_text.clear();
                        self.text_area.preedit.clear();
                        self.text_area.cursor_pos = 0;
                        self.text_area.selection_start = None;
                        self.engine.reset();
                        ui.close_menu();
                    }
                    ui.separator();
                    if ui.button("종료").clicked() {
                        ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                    }
                });
                ui.menu_button("변환", |ui| {
                    if ui.button("한영타 자동 변환 (Ctrl+Shift+Space)").clicked() {
                        self.handle_manual_typefix();
                        ui.close_menu();
                    }
                    if ui.button("영→한 변환").clicked() {
                        self.do_typefix(1);
                        ui.close_menu();
                    }
                    if ui.button("한→영 변환").clicked() {
                        self.do_typefix(2);
                        ui.close_menu();
                    }
                });
                ui.menu_button("설정", |ui| {
                    ui.label("한국어 자판:");
                    let layouts = [
                        ("두벌식", unim::config::KOREAN_LAYOUT_DUBEOLSIK),
                        ("세벌식 390", unim::config::KOREAN_LAYOUT_SEBEOLSIK_390),
                        ("세벌식 391", unim::config::KOREAN_LAYOUT_SEBEOLSIK_391),
                        (
                            "세벌식 순아래",
                            unim::config::KOREAN_LAYOUT_SEBEOLSIK_NOSHIFT,
                        ),
                    ];
                    for (name, layout) in layouts {
                        let selected = self.config.engine.korean.layout == layout;
                        if ui.selectable_label(selected, name).clicked() {
                            self.config.engine.korean.layout = layout.to_string();
                            self.engine = InputEngine::new(&self.config);
                        }
                    }
                    ui.separator();
                    ui.label("영문 자판:");
                    let eng_layouts = [
                        ("QWERTY", unim::config::ENGLISH_LAYOUT_QWERTY),
                        ("Dvorak", unim::config::ENGLISH_LAYOUT_DVORAK),
                        ("Colemak", unim::config::ENGLISH_LAYOUT_COLEMAK),
                        ("Colemak-DH", unim::config::ENGLISH_LAYOUT_COLEMAK_DH),
                        ("Workman", unim::config::ENGLISH_LAYOUT_WORKMAN),
                    ];
                    for (name, layout) in eng_layouts {
                        let selected = self.config.engine.english.layout == layout;
                        if ui.selectable_label(selected, name).clicked() {
                            self.config.engine.english.layout = layout.to_string();
                            self.engine = InputEngine::new(&self.config);
                        }
                    }
                });
            });
        });

        // 하단 상태 바
        egui::TopBottomPanel::bottom("status_bar")
            .exact_height(28.0)
            .show(ctx, |ui| {
                crate::ui::status_bar::show(ui, self.engine.input_category());
            });

        // 메인 영역
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.heading("UNIM 한글 입력기");
            ui.add_space(8.0);
            self.text_area.show(ui);
            ui.add_space(8.0);

            // 도움말
            ui.collapsing("단축키 안내", |ui| {
                ui.label("• 한/영 전환: RightAlt 또는 한/영 키");
                ui.label("• 한자 변환: F9 또는 한자 키");
                ui.label("• 수동 한영타 변환: 텍스트 선택 후 Ctrl+Shift+Space");
                ui.label("• 전체 선택: Ctrl+A");
                ui.label("• 복사: Ctrl+C");
                ui.label("• 붙여넣기: Ctrl+V");
            });
        });

        // 팝업
        self.popup.show(ctx);
    }
}

impl UnimApp {
    /// 지정 방향으로 수동 한영타 변환
    fn do_typefix(&mut self, direction: u32) {
        if self.text_area.selected_text().is_some() {
            let text = self.text_area.committed_text.clone();
            let cursor = self.text_area.cursor_pos as u32;
            let anchor = self
                .text_area
                .selection_start
                .unwrap_or(self.text_area.cursor_pos) as u32;
            self.engine.set_surrounding_text(text, cursor, anchor);

            if let Some((_offset_from_cursor, _delete_count, replacement)) =
                self.engine.typefix_convert(direction)
            {
                self.text_area.replace_selection(&replacement);
            }
        }
    }
}
