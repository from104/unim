//! 입력 엔진 모듈
//!
//! 실시간 키 입력을 처리하고 한국어 조합을 관리하는 핵심 엔진입니다.

use crate::config::{Config, EnglishLayout, InputCategory, KoreanLayout};
use crate::korean::input_context::{ComposerType, KoreanInputContext};
use crate::korean::jamo::JamoEnum;
use crate::keycode::{KeyCode, ModifierState};
use std::collections::HashMap;

/// 디버그 로깅 매크로 (UNIM_DEVELOP=1 환경변수로 활성화)
macro_rules! unim_debug {
    ($($arg:tt)*) => {
        if std::env::var("UNIM_DEVELOP").map(|v| v == "1").unwrap_or(false) {
            eprintln!("[UNIM-ENGINE] {}", format!($($arg)*));
        }
    };
}

/// 입력 처리 결과
///
/// 키 입력 처리 후의 상태 변화를 나타냅니다.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[repr(C)]
pub struct InputResult {
    /// 키 입력이 소비되었는지 여부
    /// true면 애플리케이션으로 전달하지 않음
    pub consumed: bool,
    /// preedit 문자열이 변경되었는지 여부
    pub preedit_changed: bool,
    /// commit 문자열이 변경되었는지 여부  
    pub commit_changed: bool,
}

impl InputResult {
    /// 키가 소비되지 않은 결과
    pub fn not_consumed() -> Self {
        Self {
            consumed: false,
            preedit_changed: false,
            commit_changed: false,
        }
    }

    /// 키가 소비된 결과
    pub fn consumed() -> Self {
        Self {
            consumed: true,
            preedit_changed: false,
            commit_changed: false,
        }
    }

    /// preedit 변경된 결과
    pub fn preedit_updated() -> Self {
        Self {
            consumed: true,
            preedit_changed: true,
            commit_changed: false,
        }
    }

    /// commit 발생한 결과
    pub fn committed() -> Self {
        Self {
            consumed: true,
            preedit_changed: true,
            commit_changed: true,
        }
    }

    /// commit 발생 후 키 통과 (Enter, Tab 등 특수키용)
    /// commit은 발생하지만 키는 애플리케이션으로 전달됨
    pub fn committed_passthrough() -> Self {
        Self {
            consumed: false,
            preedit_changed: true,
            commit_changed: true,
        }
    }
}

/// 입력 엔진
///
/// 키 입력을 받아 한국어 조합을 처리하고 preedit/commit 문자열을 관리합니다.
pub struct InputEngine {
    /// 현재 입력 카테고리 (한국어/영어)
    input_category: InputCategory,
    /// 한국어 입력 컨텍스트
    korean_context: KoreanInputContext,
    /// commit 버퍼
    commit_buffer: String,
    /// preedit 버퍼 (캐시)
    preedit_cache: String,
    /// 키보드 맵 (한국어) - 영어 키 -> 한국어 자모 매핑
    keyboard_map: Option<HashMap<char, JamoEnum>>,
    /// 설정 캐시
    korean_layout: KoreanLayout,
}

impl InputEngine {
    /// 새로운 InputEngine을 생성합니다.
    ///
    /// # Arguments
    ///
    /// * `config` - 엔진 설정
    pub fn new(config: &Config) -> Self {
        let composer_type = if config.engine.korean.layout.is_sebeolsik() {
            ComposerType::ThreeBul
        } else {
            ComposerType::TwoBul
        };

        let keyboard_map = Self::create_keyboard_map(&config.engine.korean.layout);

        Self {
            input_category: config.engine.default_category,
            korean_context: KoreanInputContext::new(composer_type),
            commit_buffer: String::new(),
            preedit_cache: String::new(),
            keyboard_map: Some(keyboard_map),
            korean_layout: config.engine.korean.layout,
        }
    }

    /// 기본 설정으로 새로운 InputEngine을 생성합니다.
    pub fn default() -> Self {
        Self::new(&Config::default())
    }

    /// 키보드 맵을 생성합니다.
    fn create_keyboard_map(layout: &KoreanLayout) -> HashMap<char, JamoEnum> {
        let en_json = crate::keystroke::get_keymap_json("en_qwerty");
        let ko_json = crate::keystroke::get_keymap_json(layout.name());
        let is_three_bul = layout.is_sebeolsik();
        crate::keystroke::KeyboardMap::create_keyboard_map_from_str(en_json, ko_json, is_three_bul)
    }

    /// 키 코드를 처리합니다.
    ///
    /// # Arguments
    ///
    /// * `hardware_code` - 하드웨어 키코드 (evdev)
    /// * `modifier` - 수정자 키 상태
    /// * `config` - 현재 설정
    ///
    /// # Returns
    ///
    /// 입력 처리 결과
    pub fn press_key_code(
        &mut self,
        hardware_code: u16,
        modifier: ModifierState,
        config: &Config,
    ) -> InputResult {
        let keycode = KeyCode::from_evdev_keycode(hardware_code);
        self.press_key(keycode, modifier, config)
    }

    /// KeyCode를 처리합니다.
    ///
    /// # Arguments
    ///
    /// * `keycode` - 변환된 키코드
    /// * `modifier` - 수정자 키 상태
    /// * `config` - 현재 설정
    ///
    /// # Returns
    ///
    /// 입력 처리 결과
    pub fn press_key(
        &mut self,
        keycode: KeyCode,
        modifier: ModifierState,
        _config: &Config,
    ) -> InputResult {
        // 수정자 키만 누른 경우 무시
        if keycode.is_modifier() {
            return InputResult::not_consumed();
        }

        // Control/Alt가 눌린 경우 (단축키) 무시
        if modifier.control || modifier.alt || modifier.super_key {
            // 조합 중이면 먼저 커밋
            if self.korean_context.is_composing() {
                self.flush_preedit();
                return InputResult::committed();
            }
            return InputResult::not_consumed();
        }

        // 한/영 전환 처리
        if keycode == KeyCode::Korean || keycode == KeyCode::RightAlt {
            // 조합 중이면 먼저 커밋
            let was_composing = self.korean_context.is_composing();
            self.toggle_input_category();

            // 조합 중이었으면 commit이 발생했으므로 committed() 반환
            if was_composing {
                return InputResult::committed();
            }
            return InputResult::consumed();
        }

        // 입력 카테고리에 따른 처리
        match self.input_category {
            InputCategory::Korean => self.process_korean_key(keycode, modifier),
            InputCategory::English => self.process_english_key(keycode, modifier),
        }
    }

    /// 한국어 키 입력을 처리합니다.
    fn process_korean_key(&mut self, keycode: KeyCode, modifier: ModifierState) -> InputResult {
        unim_debug!(
            "process_korean_key: keycode={:?}, shift={}, caps={}",
            keycode,
            modifier.shift,
            modifier.caps_lock
        );

        // Backspace 처리
        if keycode == KeyCode::Backspace {
            if self.korean_context.is_composing() {
                self.korean_context.backspace();
                self.update_preedit_cache();
                unim_debug!("Backspace -> preedit='{}'", self.preedit_cache);
                return InputResult::preedit_updated();
            }
            return InputResult::not_consumed();
        }

        // Enter 처리 - 조합 커밋 후 키 통과
        if keycode == KeyCode::Enter {
            if self.korean_context.is_composing() {
                self.flush_preedit();
                unim_debug!("Enter -> 조합 커밋 후 키 통과");
                return InputResult::committed_passthrough();
            }
            return InputResult::not_consumed();
        }

        // Tab 처리 - 조합 커밋 후 키 통과
        if keycode == KeyCode::Tab {
            if self.korean_context.is_composing() {
                self.flush_preedit();
                unim_debug!("Tab -> 조합 커밋 후 키 통과");
                return InputResult::committed_passthrough();
            }
            return InputResult::not_consumed();
        }

        // Escape 처리 - 조합 커밋 후 키 통과
        if keycode == KeyCode::Escape {
            if self.korean_context.is_composing() {
                self.flush_preedit();
                unim_debug!("Escape -> 조합 커밋 후 키 통과");
                return InputResult::committed_passthrough();
            }
            return InputResult::not_consumed();
        }

        // Space 처리
        if keycode == KeyCode::Space {
            if self.korean_context.is_composing() {
                self.flush_preedit();
            }
            self.commit_buffer.push(' ');
            return InputResult::committed();
        }

        // 문자 키 처리
        // 한국어 모드에서는 CapsLock을 무시하고 Shift만 적용 (쌍자음 입력용)
        let ch = if modifier.shift {
            keycode.to_shifted_char()
        } else {
            keycode.to_char()
        };

        if let Some(c) = ch {
            unim_debug!("문자 키: '{}'", c);

            // 키보드 맵에서 자모 찾기
            if let Some(ref keyboard_map) = self.keyboard_map {
                if let Some(jamo) = keyboard_map.get(&c) {
                    unim_debug!("자모 매핑: {:?}", jamo);

                    // Special 자모는 비-한국어 문자이므로 별도 처리
                    if let JamoEnum::Special(special_char) = jamo {
                        let ch_to_commit = *special_char; // 먼저 복사
                        unim_debug!("Special 자모 처리: '{}'", ch_to_commit);
                        // 조합 중이면 먼저 커밋
                        self.flush_preedit();
                        // Special 문자를 commit_buffer에 추가
                        self.commit_buffer.push(ch_to_commit);
                        return InputResult::committed();
                    }

                    // 일반 자모 입력
                    self.korean_context.process_jamo(jamo.clone());

                    // committed 문자가 있으면 commit_buffer에 추가
                    let committed = self.korean_context.get_committed();
                    unim_debug!(
                        "context.committed='{}', context.preedit='{}'",
                        committed,
                        self.korean_context.get_preedit()
                    );

                    if !committed.is_empty() {
                        self.commit_buffer.push_str(committed);
                        // committed 문자열만 비우기 (preedit은 유지)
                        self.korean_context.clear_committed();
                        unim_debug!("commit_buffer에 추가 후: '{}'", self.commit_buffer);
                    }

                    self.update_preedit_cache();
                    unim_debug!("preedit_cache 업데이트 후: '{}'", self.preedit_cache);

                    if !self.commit_buffer.is_empty() {
                        unim_debug!("-> InputResult::committed()");
                        return InputResult::committed();
                    }
                    unim_debug!("-> InputResult::preedit_updated()");
                    return InputResult::preedit_updated();
                }
            }

            // 자모가 아닌 문자 (기호 등)
            self.flush_preedit();
            self.commit_buffer.push(c);
            return InputResult::committed();
        }

        // 조합 중에 문자가 아닌 키(화살표, F키 등) 입력 시 커밋 후 키 통과
        if self.korean_context.is_composing() {
            self.flush_preedit();
            unim_debug!("비문자키 -> 조합 커밋 후 키 통과");
            return InputResult::committed_passthrough();
        }

        InputResult::not_consumed()
    }

    /// 영어 키 입력을 처리합니다.
    fn process_english_key(&mut self, keycode: KeyCode, modifier: ModifierState) -> InputResult {
        let ch = if modifier.shift || modifier.caps_lock {
            keycode.to_shifted_char()
        } else {
            keycode.to_char()
        };

        if let Some(c) = ch {
            self.commit_buffer.push(c);
            return InputResult::committed();
        }

        InputResult::not_consumed()
    }

    /// preedit 캐시를 업데이트합니다.
    fn update_preedit_cache(&mut self) {
        self.preedit_cache = self.korean_context.get_preedit().to_string();
    }

    /// preedit을 commit_buffer로 플러시합니다.
    fn flush_preedit(&mut self) {
        if self.korean_context.is_composing() {
            self.korean_context.commit();
            let committed = self.korean_context.get_committed();
            self.commit_buffer.push_str(committed);
            self.korean_context.clear();
            self.preedit_cache.clear();
        }
    }

    /// 입력 카테고리를 토글합니다.
    fn toggle_input_category(&mut self) {
        // 조합 중이면 먼저 플러시
        self.flush_preedit();

        self.input_category = match self.input_category {
            InputCategory::Korean => InputCategory::English,
            InputCategory::English => InputCategory::Korean,
        };

        // 상태 파일 업데이트
        self.update_status_file();
    }

    /// 현재 입력 카테고리를 반환합니다.
    pub fn input_category(&self) -> InputCategory {
        self.input_category
    }

    /// 입력 카테고리를 설정합니다.
    pub fn set_input_category(&mut self, category: InputCategory) {
        if self.input_category != category {
            self.flush_preedit();
            self.input_category = category;
            // 상태 파일 업데이트
            self.update_status_file();
        }
    }

    /// 상태 파일을 업데이트합니다.
    fn update_status_file(&self) {
        let status_category = match self.input_category {
            InputCategory::Korean => crate::status::InputCategory::Korean,
            InputCategory::English => crate::status::InputCategory::English,
        };
        // 오류 발생 시 무시 (로깅은 하지 않음 - 성능을 위해)
        let _ = crate::status::set_status(status_category);
    }

    /// commit 문자열을 반환합니다.
    pub fn commit_str(&self) -> &str {
        &self.commit_buffer
    }

    /// preedit 문자열을 반환합니다.
    pub fn preedit_str(&self) -> &str {
        &self.preedit_cache
    }

    /// commit 버퍼를 비웁니다.
    pub fn clear_commit(&mut self) {
        self.commit_buffer.clear();
    }

    /// preedit을 비웁니다 (commit으로 플러시).
    pub fn clear_preedit(&mut self) {
        self.flush_preedit();
    }

    /// preedit을 제거합니다 (commit 없이).
    pub fn remove_preedit(&mut self) {
        self.korean_context.clear();
        self.preedit_cache.clear();
    }

    /// 엔진 상태를 리셋합니다.
    pub fn reset(&mut self) {
        self.korean_context.clear();
        self.commit_buffer.clear();
        self.preedit_cache.clear();
    }

    /// 조합 중인지 확인합니다.
    pub fn is_composing(&self) -> bool {
        self.korean_context.is_composing()
    }

    /// ready 상태 확인 (프론트엔드 호환용)
    pub fn check_ready(&self) -> bool {
        !self.commit_buffer.is_empty() || !self.preedit_cache.is_empty()
    }

    /// ready 상태 종료 (프론트엔드 호환용)
    pub fn end_ready(&mut self) -> InputResult {
        if self.check_ready() {
            InputResult {
                consumed: true,
                preedit_changed: !self.preedit_cache.is_empty(),
                commit_changed: !self.commit_buffer.is_empty(),
            }
        } else {
            InputResult::not_consumed()
        }
    }

    /// 한국어 레이아웃을 설정합니다.
    pub fn set_korean_layout(&mut self, layout: KoreanLayout) {
        if self.korean_layout != layout {
            self.flush_preedit();
            self.korean_layout = layout;

            // 키보드 맵 업데이트
            self.keyboard_map = Some(Self::create_keyboard_map(&layout));

            // 컨텍스트 업데이트
            let composer_type = if layout.is_sebeolsik() {
                ComposerType::ThreeBul
            } else {
                ComposerType::TwoBul
            };
            self.korean_context = KoreanInputContext::new(composer_type);
        }
    }

    /// 영어 레이아웃을 설정합니다.
    pub fn set_english_layout(&mut self, _layout: EnglishLayout) {
        // 현재 영어 레이아웃은 키보드 맵에 직접 영향을 주지 않고 (항상 QWERTY로 가정하거나 OS 레벨 처리)
        // 엔진 수준에서는 자리 매김만 기록합니다.
        // 향후 Dvorak 등을 위해 키보드 맵을 재생성하도록 확장 가능합니다.
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_engine() -> InputEngine {
        let config = Config::default();
        InputEngine::new(&config)
    }

    #[test]
    fn test_engine_creation() {
        let engine = create_test_engine();
        assert_eq!(engine.input_category(), InputCategory::Korean);
        assert!(!engine.is_composing());
    }

    #[test]
    fn test_english_input() {
        let mut engine = create_test_engine();
        engine.set_input_category(InputCategory::English);

        let config = Config::default();
        let modifier = ModifierState::default();

        let result = engine.press_key(KeyCode::A, modifier, &config);
        assert!(result.consumed);
        assert!(result.commit_changed);
        assert_eq!(engine.commit_str(), "a");
    }

    #[test]
    fn test_input_category_toggle() {
        let mut engine = create_test_engine();
        assert_eq!(engine.input_category(), InputCategory::Korean);

        let config = Config::default();
        let modifier = ModifierState::default();

        engine.press_key(KeyCode::Korean, modifier, &config);
        assert_eq!(engine.input_category(), InputCategory::English);

        engine.press_key(KeyCode::Korean, modifier, &config);
        assert_eq!(engine.input_category(), InputCategory::Korean);
    }

    #[test]
    fn test_reset() {
        let mut engine = create_test_engine();
        engine.commit_buffer.push_str("test");

        engine.reset();
        assert!(engine.commit_str().is_empty());
        assert!(engine.preedit_str().is_empty());
    }
}
