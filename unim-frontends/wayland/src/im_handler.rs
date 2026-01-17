//! 입력 방식 핸들러 모듈
//!
//! Wayland input-method-v2 이벤트를 처리하고 UNIM 엔진과 연동합니다.

use unim::config::{Config, InputCategory};
use unim::input_engine::InputEngine;
use unim::keycode::{KeyCode, ModifierState};
use wayland_protocols_misc::zwp_input_method_v2::client::zwp_input_method_v2::ZwpInputMethodV2;

/// 입력 방식 핸들러
pub struct InputMethodHandler {
    engine: InputEngine,
    config: Config,
    active: bool,
    serial: u32,
    surrounding_text: String,
    surrounding_cursor: u32,
    surrounding_anchor: u32,
    pending_commit: Option<String>,
    pending_preedit: Option<String>,
}

impl InputMethodHandler {
    pub fn new(config: &Config) -> Self {
        Self {
            engine: InputEngine::new(config),
            config: config.clone(),
            active: false,
            serial: 0,
            surrounding_text: String::new(),
            surrounding_cursor: 0,
            surrounding_anchor: 0,
            pending_commit: None,
            pending_preedit: None,
        }
    }

    /// 입력 방식 활성화
    pub fn activate(&mut self) {
        self.active = true;
        self.engine.reset();
        log::info!("입력 방식 활성화됨");
    }

    /// 입력 방식 비활성화
    pub fn deactivate(&mut self, im: &ZwpInputMethodV2) {
        if self.active {
            // 조합 중이면 커밋
            if self.engine.is_composing() {
                self.engine.clear_preedit();
                let commit = self.engine.commit_str();
                if !commit.is_empty() {
                    im.commit_string(commit.to_string());
                    self.engine.clear_commit();
                }
                // preedit 지우기
                im.set_preedit_string(String::new(), 0, 0);
            }
            self.engine.reset();
        }
        self.active = false;
        log::info!("입력 방식 비활성화됨");
    }

    /// 주변 텍스트 설정
    pub fn set_surrounding_text(&mut self, text: &str, cursor: u32, anchor: u32) {
        self.surrounding_text = text.to_string();
        self.surrounding_cursor = cursor;
        self.surrounding_anchor = anchor;
    }

    /// Done 이벤트 처리 (상태 커밋)
    pub fn done(&mut self, im: &ZwpInputMethodV2) {
        self.serial = self.serial.wrapping_add(1);

        // 대기 중인 커밋이 있으면 전송
        if let Some(commit) = self.pending_commit.take() {
            im.commit_string(commit);
        }

        // 대기 중인 preedit이 있으면 전송
        if let Some(preedit) = self.pending_preedit.take() {
            let cursor = preedit.len() as i32;
            im.set_preedit_string(preedit, cursor, cursor);
        }

        im.commit(self.serial);
    }

    /// 키 입력 처리
    pub fn handle_key(
        &mut self,
        im: &ZwpInputMethodV2,
        keycode: u32,
        state: u32,
        modifiers: u32,
    ) -> bool {
        if !self.active || state != 1 {
            // 키 릴리즈는 무시
            return false;
        }

        // evdev 키코드를 KeyCode로 변환
        let key = KeyCode::from_evdev_keycode(keycode as u16);

        // 수정자 상태 변환
        let modifier = ModifierState {
            shift: (modifiers & 1) != 0,
            control: (modifiers & 4) != 0,
            alt: (modifiers & 8) != 0,
            super_key: (modifiers & 64) != 0,
            caps_lock: (modifiers & 2) != 0,
            num_lock: false,
        };

        // 입력 처리
        let result = self.engine.press_key(key, modifier, &self.config);

        if result.consumed {
            // 커밋 문자열이 있으면 전송
            if result.commit_changed {
                let commit = self.engine.commit_str().to_string();
                if !commit.is_empty() {
                    self.pending_commit = Some(commit);
                    self.engine.clear_commit();
                }
            }

            // Preedit 변경이 있으면 업데이트
            if result.preedit_changed {
                let preedit = self.engine.preedit_str().to_string();
                self.pending_preedit = Some(preedit);
            }

            true // consumed
        } else {
            false // forward to application
        }
    }

    /// 현재 입력 카테고리 반환
    pub fn input_category(&self) -> InputCategory {
        self.engine.input_category()
    }

    /// 입력 카테고리 설정
    pub fn set_input_category(&mut self, category: InputCategory) {
        self.engine.set_input_category(category);
    }
}
