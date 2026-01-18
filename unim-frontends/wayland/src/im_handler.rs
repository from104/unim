//! 입력 방식 핸들러 모듈
//!
//! Wayland input-method-v2 이벤트를 처리하고 UNIM 엔진과 연동합니다.

use unim::config::Config;
use unim::input_engine::InputEngine;
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

    /// 설정을 업데이트합니다.
    pub fn update_config(&mut self, config: &Config) {
        self.config = config.clone();
        self.engine.set_hangul_layout(config.engine.hangul.layout);
        self.engine.set_latin_layout(config.engine.latin.layout);
        log::debug!("핸들러 설정 업데이트 완료");
    }

    /// 입력 방식 활성화
    pub fn activate(&mut self) {
        self.active = true;
        self.engine.reset();
        log::info!("입력 방식 활성화됨");
    }

    /// 입력 방식 비활성화
    pub fn deactivate(&mut self, _im: &ZwpInputMethodV2) {
        if self.active {
            // 조합 중이면 커밋
            if self.engine.is_composing() {
                self.engine.clear_preedit();
                let commit = self.engine.commit_str();
                if !commit.is_empty() {
                    _im.commit_string(commit.to_string());
                    self.engine.clear_commit();
                }
                // preedit 지우기
                _im.set_preedit_string(String::new(), 0, 0);
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
    pub fn done(&mut self, im: &ZwpInputMethodV2, config: &mut unim::config::Config) {
        // 설정 파일 변경 체크 (mtime 기반, 매우 가벼움)
        if config.reload_if_changed() {
            log::debug!("설정 파일 변경 감지, 리로드 완료");
            self.update_config(config);
        }

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
}
