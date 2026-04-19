//! 입력 엔진 모듈
//!
//! 실시간 키 입력을 처리하고 한국어 조합을 관리하는 핵심 엔진입니다.

use crate::config::{Config, ContentPurpose, EnglishLayout, InputCategory, KoreanLayout};
use crate::hangul::input_context::{ComposerType, HangulInputContext};
use crate::hangul::jamo::JamoEnum;
use crate::keycode::{KeyCode, ModifierState};
use crate::keystroke::EnglishKeymap;
use crate::popup::{PopupKey, PopupKeyResult, PopupState};
use crate::unim_log;
use std::collections::HashMap;

/// 팝업 동작 (ProcessKeyEvent 후 발생)
#[derive(Debug, Clone)]
pub enum PopupAction {
    /// 한자 팝업 표시
    ShowHanja {
        target: String,
        candidates: Vec<(String, String)>,
    },
    /// 특수문자 팝업 표시
    ShowSpecial {
        target: String,
        characters: Vec<String>,
        top_row: String,
    },
    /// 팝업 숨김
    HidePopup,
    /// 페이지/선택 변경 (UI 업데이트용)
    PopupNavigate {
        page: usize,
        total_pages: usize,
        selected: usize,
        rows: usize,
        cols: usize,
        sel_row: usize,
        sel_col: usize,
    },
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
    /// 한자 후보가 사용 가능한지 여부
    pub hanja_candidates_available: bool,
    /// 특수문자 후보가 사용 가능한지 여부
    pub special_char_candidates_available: bool,
}

impl InputResult {
    /// 키가 소비되지 않은 결과
    pub fn not_consumed() -> Self {
        Self {
            consumed: false,
            preedit_changed: false,
            commit_changed: false,
            hanja_candidates_available: false,
            special_char_candidates_available: false,
        }
    }

    /// 키가 소비된 결과
    pub fn consumed() -> Self {
        Self {
            consumed: true,
            preedit_changed: false,
            commit_changed: false,
            hanja_candidates_available: false,
            special_char_candidates_available: false,
        }
    }

    /// preedit 변경된 결과
    pub fn preedit_updated() -> Self {
        Self {
            consumed: true,
            preedit_changed: true,
            commit_changed: false,
            hanja_candidates_available: false,
            special_char_candidates_available: false,
        }
    }

    /// commit 발생한 결과
    pub fn committed() -> Self {
        Self {
            consumed: true,
            preedit_changed: true,
            commit_changed: true,
            hanja_candidates_available: false,
            special_char_candidates_available: false,
        }
    }

    /// commit 발생 후 키 통과 (Enter, Tab 등 특수키용)
    /// commit은 발생하지만 키는 애플리케이션으로 전달됨
    pub fn committed_passthrough() -> Self {
        Self {
            consumed: false,
            preedit_changed: true,
            commit_changed: true,
            hanja_candidates_available: false,
            special_char_candidates_available: false,
        }
    }

    /// 한자 후보 사용 가능 (preedit 유지 — 팝업 중 조합 문자 표시)
    pub fn hanja_candidates() -> Self {
        Self {
            consumed: true,
            preedit_changed: true,
            commit_changed: false,
            hanja_candidates_available: true,
            special_char_candidates_available: false,
        }
    }

    /// 특수문자 후보 사용 가능 (preedit 유지 — 팝업 중 조합 문자 표시)
    pub fn special_char_candidates() -> Self {
        Self {
            consumed: true,
            preedit_changed: true,
            commit_changed: false,
            hanja_candidates_available: false,
            special_char_candidates_available: true,
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
    korean_context: HangulInputContext,
    /// commit 버퍼
    commit_buffer: String,
    /// preedit 버퍼 (캐시)
    preedit_cache: String,
    /// 키보드 맵 (한국어) - 영어 키 -> 한국어 자모 매핑
    keyboard_map: Option<HashMap<char, JamoEnum>>,
    /// 영어 키보드 레이아웃 맵 (JSON 기반 동적 로드)
    english_keymap: EnglishKeymap,
    /// 영어 키보드 레이아웃 설정
    english_layout: EnglishLayout,
    /// 한국어 키보드 레이아웃 설정 캐시
    korean_layout: KoreanLayout,
    /// 한자 사전 (Arc로 래핑하여 공유)
    hanja_dict: std::sync::Arc<crate::hangul::HanjaDictionary>,
    /// 현재 한자 후보 목록
    hanja_candidates: Vec<crate::hangul::HanjaEntry>,
    /// 한자 선택 모드 활성화 여부
    hanja_mode: bool,
    /// 한자 변환 대상 문자열 (preedit 또는 마지막 음절)
    hanja_target: String,
    /// 특수문자 모드 활성화 여부
    special_char_mode: bool,
    /// 현재 특수문자 후보 목록
    special_char_candidates: Vec<char>,
    /// 특수문자 변환 대상 초성
    special_char_target: String,
    /// 한/영 전환 키 목록 (설정 기반)
    toggle_keys: Vec<KeyCode>,
    /// 통합 팝업 상태 (한자/특수문자 공용)
    popup_state: Option<PopupState>,
    /// 처리 대기 팝업 액션
    popup_pending_action: Option<PopupAction>,
    /// 영어 레이아웃 top_row_labels 캐시 (특수문자 팝업용)
    top_row_labels: String,
    /// 현재 입력 필드의 목적 (비밀번호 등)
    content_purpose: ContentPurpose,
    /// Surrounding text (커서 주변 텍스트)
    surrounding_text: String,
    /// Surrounding text 커서 위치 (문자 단위)
    surrounding_cursor: u32,
    /// Surrounding text 앵커 위치 (문자 단위)
    surrounding_anchor: u32,
}

impl Default for InputEngine {
    fn default() -> Self {
        Self::new(&Config::default())
    }
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

        let keyboard_map =
            Self::create_keyboard_map(&config.engine.korean.layout, &config.engine.english.layout);

        let english_keymap = Self::create_english_keymap(&config.engine.english.layout);

        // 한자 사전 초기화 (한 번만 로드하여 Arc로 공유)
        let hanja_dict = std::sync::Arc::new(crate::hangul::HanjaDictionary::new());

        Self {
            input_category: config.engine.default_category,
            korean_context: HangulInputContext::new(composer_type),
            commit_buffer: String::new(),
            preedit_cache: String::new(),
            keyboard_map: Some(keyboard_map),
            english_keymap,
            korean_layout: config.engine.korean.layout,
            english_layout: config.engine.english.layout,
            hanja_dict,
            hanja_candidates: Vec::new(),
            hanja_mode: false,
            hanja_target: String::new(),
            special_char_mode: false,
            special_char_candidates: Vec::new(),
            special_char_target: String::new(),
            toggle_keys: config
                .engine
                .toggle_keys
                .iter()
                .map(|name| KeyCode::from_name(name))
                .filter(|k| *k != KeyCode::Unknown)
                .collect(),
            popup_state: None,
            popup_pending_action: None,
            top_row_labels: config.engine.english.layout.top_row_labels().to_string(),
            content_purpose: ContentPurpose::Normal,
            surrounding_text: String::new(),
            surrounding_cursor: 0,
            surrounding_anchor: 0,
        }
    }


    /// 키보드 맵을 생성합니다.
    ///
    /// # Arguments
    ///
    /// * `korean_layout` - 한국어 키보드 레이아웃
    /// * `english_layout` - 영어 키보드 레이아웃
    fn create_keyboard_map(
        korean_layout: &KoreanLayout,
        english_layout: &EnglishLayout,
    ) -> HashMap<char, JamoEnum> {
        let en_json = crate::keystroke::get_keymap_json(english_layout.keymap_name());
        let ko_json = crate::keystroke::get_keymap_json(korean_layout.name());
        let is_three_bul = korean_layout.is_sebeolsik();
        crate::keystroke::KeyboardMap::create_keyboard_map_from_str(en_json, ko_json, is_three_bul)
    }

    /// 영어 키맵을 생성합니다.
    ///
    /// # Arguments
    ///
    /// * `layout` - 영어 키보드 레이아웃
    fn create_english_keymap(layout: &EnglishLayout) -> EnglishKeymap {
        let json = crate::keystroke::get_keymap_json(layout.keymap_name());
        EnglishKeymap::from_json(json)
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

        // 한자/특수문자 팝업 활성 상태에서 키 인터셉트
        if self.hanja_mode || self.special_char_mode {
            return self.process_popup_key(keycode, modifier, _config);
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

        // 한/영 전환 처리 (설정 기반)
        if self.toggle_keys.contains(&keycode) {
            // 비밀번호 필드에서는 한/영 전환 차단
            if self.content_purpose.should_block_hangul()
                && self.input_category == InputCategory::English
            {
                unim_log!(
                    "ENGINE",
                    "한/영 전환 차단: content_purpose={:?}",
                    self.content_purpose
                );
                return InputResult::consumed();
            }

            // 조합 중이면 먼저 커밋
            let was_composing = self.korean_context.is_composing();
            self.toggle_input_category();

            // 조합 중이었으면 commit이 발생했으므로 committed() 반환
            if was_composing {
                return InputResult::committed();
            }
            return InputResult::consumed();
        }

        // 비밀번호/PIN 필드에서는 한글 모드를 영어로 강제 전환
        if self.content_purpose.should_block_hangul()
            && self.input_category == InputCategory::Korean
        {
            unim_log!(
                "ENGINE",
                "비밀번호 필드 감지: 영문 모드로 강제 전환"
            );
            self.set_input_category(InputCategory::English);
        }

        // 입력 카테고리에 따른 처리
        match self.input_category {
            InputCategory::Korean => self.process_korean_key(keycode, modifier),
            InputCategory::English => self.process_english_key(keycode, modifier),
        }
    }

    /// 한국어 키 입력을 처리합니다.
    fn process_korean_key(&mut self, keycode: KeyCode, modifier: ModifierState) -> InputResult {
        unim_log!(
            "ENGINE",
            "process_korean_key: keycode={:?}, shift={}, caps={}",
            keycode,
            modifier.shift,
            modifier.caps_lock
        );

        // Hanja 키 처리 - 한자 변환 모드 시작
        if keycode == KeyCode::Hanja {
            return self.start_hanja_conversion();
        }

        // Backspace 처리
        if keycode == KeyCode::Backspace {
            if self.korean_context.is_composing() {
                self.korean_context.backspace();
                self.update_preedit_cache();
                unim_log!("ENGINE", "Backspace -> preedit='{}'", self.preedit_cache);
                return InputResult::preedit_updated();
            }
            return InputResult::not_consumed();
        }

        // Enter 처리 - 조합 커밋 후 키 통과
        if keycode == KeyCode::Enter {
            if self.korean_context.is_composing() {
                self.flush_preedit();
                unim_log!("ENGINE", "Enter -> 조합 커밋 후 키 통과");
                return InputResult::committed_passthrough();
            }
            return InputResult::not_consumed();
        }

        // Tab 처리 - 조합 커밋 후 키 통과
        if keycode == KeyCode::Tab {
            if self.korean_context.is_composing() {
                self.flush_preedit();
                unim_log!("ENGINE", "Tab -> 조합 커밋 후 키 통과");
                return InputResult::committed_passthrough();
            }
            return InputResult::not_consumed();
        }

        // Escape 처리 - 조합 커밋 후 키 통과
        if keycode == KeyCode::Escape {
            if self.korean_context.is_composing() {
                self.flush_preedit();
                unim_log!("ENGINE", "Escape -> 조합 커밋 후 키 통과");
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
        // JSON 키맵 기반으로 레이아웃에 따른 문자 변환
        let ch = self.english_keymap.get_char(keycode, modifier.shift);

        if let Some(c) = ch {
            unim_log!("ENGINE", "문자 키: '{}'", c);

            // 키보드 맵에서 자모 찾기
            if let Some(ref keyboard_map) = self.keyboard_map {
                if let Some(jamo) = keyboard_map.get(&c) {
                    unim_log!("ENGINE", "자모 매핑: {:?}", jamo);

                    // Special 자모는 비-한국어 문자이므로 별도 처리
                    if let JamoEnum::Special(special_char) = jamo {
                        let ch_to_commit = *special_char; // 먼저 복사
                        unim_log!("ENGINE", "Special 자모 처리: '{}'", ch_to_commit);
                        // 조합 중이면 먼저 커밋
                        self.flush_preedit();
                        // Special 문자를 commit_buffer에 추가
                        self.commit_buffer.push(ch_to_commit);
                        return InputResult::committed();
                    }

                    // 일반 자모 입력
                    self.korean_context.process_jamo(*jamo);

                    // committed 문자가 있으면 commit_buffer에 추가
                    let committed = self.korean_context.get_committed();
                    unim_log!(
                        "ENGINE",
                        "context.committed='{}', context.preedit='{}'",
                        committed,
                        self.korean_context.get_preedit()
                    );

                    if !committed.is_empty() {
                        self.commit_buffer.push_str(committed);
                        // committed 문자열만 비우기 (preedit은 유지)
                        self.korean_context.clear_committed();
                        unim_log!(
                            "ENGINE",
                            "commit_buffer에 추가 후: '{}'",
                            self.commit_buffer
                        );
                    }

                    self.update_preedit_cache();
                    unim_log!(
                        "ENGINE",
                        "preedit_cache 업데이트 후: '{}'",
                        self.preedit_cache
                    );

                    if !self.commit_buffer.is_empty() {
                        unim_log!("ENGINE", "-> InputResult::committed()");
                        return InputResult::committed();
                    }
                    unim_log!("ENGINE", "-> InputResult::preedit_updated()");
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
            unim_log!("ENGINE", "비문자키 -> 조합 커밋 후 키 통과");
            return InputResult::committed_passthrough();
        }

        InputResult::not_consumed()
    }

    /// 영어 키 입력을 처리합니다.
    fn process_english_key(&mut self, keycode: KeyCode, modifier: ModifierState) -> InputResult {
        // Space는 영문 키맵에 매핑돼 있지 않으므로 여기서 먼저 커밋한다.
        // 한국어 모드는 process_korean_key에서 동일하게 처리한다.
        //
        // 이전에는 Space를 not_consumed로 반환했는데, 영문 알파벳은 consumed=true/
        // commit='x' 경로를 타고 Space만 consumed=false가 돼서 GTK IM 모듈의 상태
        // 전환이 꼬여 gedit 등에서 공백이 간헐적으로 drop되는 회귀가 있었다.
        // GNOME 경로는 이미 Korean 모드에서 Space를 commit=' '로 받고 있었으므로,
        // 영문 모드에서도 같은 전략으로 통일한다.
        if keycode == KeyCode::Space {
            self.commit_buffer.push(' ');
            return InputResult::committed();
        }

        // JSON 키맵 기반으로 레이아웃에 따른 문자 변환
        // CapsLock은 알파벳 문자에만 적용 (숫자/기호는 Shift만)

        // 먼저 lower case 문자를 확인하여 알파벳인지 판단
        // 이렇게 하면 Dvorak/Colemak 등 커스텀 키맵에서도 정상 동작
        let lower_char = self.english_keymap.get_char(keycode, false);
        let is_alpha_output = lower_char.map(|c| c.is_ascii_alphabetic()).unwrap_or(false);

        let shifted = if is_alpha_output {
            // 알파벳 출력: Shift XOR CapsLock (둘 다 켜면 소문자)
            modifier.shift ^ modifier.caps_lock
        } else {
            // 숫자/기호 출력: Shift만 적용, CapsLock 무시
            modifier.shift
        };
        let ch = self.english_keymap.get_char(keycode, shifted);

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
                hanja_candidates_available: false,
                special_char_candidates_available: false,
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
            self.keyboard_map = Some(Self::create_keyboard_map(&layout, &self.english_layout));

            // 컨텍스트 업데이트
            let composer_type = if layout.is_sebeolsik() {
                ComposerType::ThreeBul
            } else {
                ComposerType::TwoBul
            };
            self.korean_context = HangulInputContext::new(composer_type);
        }
    }

    /// 영어 레이아웃을 설정합니다.
    ///
    /// 레이아웃이 변경되면 키보드 맵과 영어 키맵을 재생성합니다.
    pub fn set_english_layout(&mut self, layout: EnglishLayout) {
        if self.english_layout != layout {
            self.flush_preedit();
            self.english_layout = layout;

            // 한국어 키보드 맵 재생성 (영어 레이아웃과 연동)
            self.keyboard_map = Some(Self::create_keyboard_map(&self.korean_layout, &layout));

            // 영어 키맵 재생성
            self.english_keymap = Self::create_english_keymap(&layout);
        }
    }

    // =========================================
    // 한자 변환 관련 메서드
    // =========================================

    /// 한자 변환 모드를 시작합니다.
    ///
    /// 현재 preedit 또는 마지막 음절에서 한자 후보를 검색합니다.
    pub fn start_hanja_conversion(&mut self) -> InputResult {
        // 이미 한자 모드이면 무시
        if self.hanja_mode {
            return InputResult::consumed();
        }

        // 변환 대상 결정: preedit 우선, 없으면 마지막 커밋 음절
        let target = if !self.preedit_cache.is_empty() {
            // preedit의 마지막 음절
            self.preedit_cache.chars().last().map(|c| c.to_string())
        } else {
            // 커밋 버퍼의 마지막 음절 (이미 입력된 경우)
            None
        };

        if let Some(target_syllable) = target {
            let candidates = self.hanja_dict.search(&target_syllable);
            if !candidates.is_empty() {
                unim_log!(
                    "ENGINE",
                    "한자 후보 발견: '{}' -> {} 개",
                    target_syllable,
                    candidates.len()
                );
                self.hanja_target = target_syllable.clone();
                let hanja_pairs = candidates
                    .iter()
                    .map(|e| (e.hanja.clone(), e.meaning.clone()))
                    .collect::<Vec<_>>();
                self.hanja_candidates = candidates;
                self.hanja_mode = true;
                self.popup_state =
                    Some(PopupState::new_hanja(&target_syllable, hanja_pairs.clone()));
                // 팝업 액션 설정
                self.popup_pending_action = Some(PopupAction::ShowHanja {
                    target: target_syllable,
                    candidates: hanja_pairs,
                });
                return InputResult::hanja_candidates();
            }

            // 한자 후보 없음 → 초성이면 특수문자 검색 시도
            let ch = target_syllable.chars().next().unwrap_or('\0');
            if let Some(entry) = crate::hangul::special_chars::search_by_choseong(ch) {
                unim_log!(
                    "ENGINE",
                    "특수문자 후보 발견: '{}' ({}) -> {} 개",
                    ch,
                    entry.category,
                    entry.characters.len()
                );
                self.special_char_target = target_syllable;
                self.special_char_candidates = entry.characters.to_vec();
                self.special_char_mode = true;
                let chars: Vec<String> = self
                    .special_char_candidates
                    .iter()
                    .map(|c| c.to_string())
                    .collect();
                self.popup_state = Some(PopupState::new_special(
                    &self.special_char_target,
                    chars.clone(),
                    &self.top_row_labels,
                ));
                // 팝업 액션 설정
                self.popup_pending_action = Some(PopupAction::ShowSpecial {
                    target: self.special_char_target.clone(),
                    characters: chars,
                    top_row: self.top_row_labels.clone(),
                });
                return InputResult::special_char_candidates();
            }
        }

        unim_log!("ENGINE", "한자/특수문자 후보 없음");
        InputResult::consumed()
    }

    /// 현재 한자 모드 상태를 반환합니다.
    pub fn is_hanja_mode(&self) -> bool {
        self.hanja_mode
    }

    /// 현재 한자 후보 목록을 반환합니다.
    ///
    /// 각 항목은 (한자, 뜻풀이) 튜플입니다.
    pub fn get_hanja_candidates(&self) -> Vec<(String, String)> {
        self.hanja_candidates
            .iter()
            .map(|entry| (entry.hanja.clone(), entry.meaning.clone()))
            .collect()
    }

    /// 한자 변환 대상 문자열을 반환합니다.
    pub fn get_hanja_target(&self) -> &str {
        &self.hanja_target
    }

    /// 한자를 선택합니다.
    ///
    /// # 인자
    ///
    /// * `index` - 선택할 한자의 인덱스 (0부터 시작)
    ///
    /// # 반환
    ///
    /// 선택된 한자 문자열. 유효하지 않은 인덱스면 None.
    pub fn select_hanja(&mut self, index: usize) -> Option<String> {
        if !self.hanja_mode || index >= self.hanja_candidates.len() {
            return None;
        }

        let selected = &self.hanja_candidates[index];
        let hanja = selected.hanja.clone();

        unim_log!("ENGINE", "한자 선택: [{}] '{}'", index, hanja);

        // preedit에서 마지막 음절을 제거
        if !self.preedit_cache.is_empty() {
            self.korean_context.clear();
            self.preedit_cache.clear();
            // DBus 응답으로 한자를 반환하므로 commit_buffer에 추가하지 않음
            // (추가 시 다음 키 입력에 묻어나와 이중 커밋 발생)
        }

        self.cancel_hanja();
        Some(hanja)
    }

    /// 한자 모드를 취소합니다.
    pub fn cancel_hanja(&mut self) {
        self.hanja_mode = false;
        self.hanja_candidates.clear();
        self.hanja_target.clear();
        self.popup_state = None;

        // preedit도 클리어 (한자 선택 후 원래 한글이 남지 않도록)
        self.korean_context.clear();
        self.preedit_cache.clear();
    }

    // =========================================
    // 특수문자 변환 관련 메서드
    // =========================================

    /// 현재 특수문자 모드 상태를 반환합니다.
    pub fn is_special_char_mode(&self) -> bool {
        self.special_char_mode
    }

    /// 현재 특수문자 후보 목록을 반환합니다.
    pub fn get_special_char_candidates(&self) -> &[char] {
        &self.special_char_candidates
    }

    /// 특수문자 변환 대상 문자열(초성)을 반환합니다.
    pub fn get_special_char_target(&self) -> &str {
        &self.special_char_target
    }

    /// 특수문자를 선택합니다.
    ///
    /// # 인자
    ///
    /// * `index` - 선택할 특수문자의 인덱스 (0부터 시작)
    ///
    /// # 반환
    ///
    /// 선택된 특수문자. 유효하지 않은 인덱스면 None.
    pub fn select_special_char(&mut self, index: usize) -> Option<char> {
        if !self.special_char_mode || index >= self.special_char_candidates.len() {
            return None;
        }

        let selected = self.special_char_candidates[index];
        unim_log!("ENGINE", "특수문자 선택: [{}] '{}'", index, selected);

        // preedit(초성)을 제거
        self.korean_context.clear();
        self.preedit_cache.clear();
        // DBus 응답으로 특수문자를 반환하므로 commit_buffer에 추가하지 않음
        // (추가 시 다음 키 입력에 묻어나와 이중 커밋 발생)

        self.cancel_special_char();
        Some(selected)
    }

    /// 특수문자 모드를 취소합니다.
    pub fn cancel_special_char(&mut self) {
        self.special_char_mode = false;
        self.special_char_candidates.clear();
        self.special_char_target.clear();
        self.popup_state = None;

        // preedit도 클리어
        self.korean_context.clear();
        self.preedit_cache.clear();
    }

    // =========================================
    // 팝업 키 핸들링
    // =========================================

    /// 처리 대기 중인 팝업 액션을 꺼냅니다.
    pub fn take_popup_action(&mut self) -> Option<PopupAction> {
        self.popup_pending_action.take()
    }

    /// KeyCode를 PopupKey로 변환합니다.
    fn keycode_to_popup_key(keycode: KeyCode) -> PopupKey {
        match keycode {
            KeyCode::Num1 => PopupKey::Number(1),
            KeyCode::Num2 => PopupKey::Number(2),
            KeyCode::Num3 => PopupKey::Number(3),
            KeyCode::Num4 => PopupKey::Number(4),
            KeyCode::Num5 => PopupKey::Number(5),
            KeyCode::Num6 => PopupKey::Number(6),
            KeyCode::Num7 => PopupKey::Number(7),
            KeyCode::Num8 => PopupKey::Number(8),
            KeyCode::Num9 => PopupKey::Number(9),
            KeyCode::Enter => PopupKey::Enter,
            KeyCode::Escape => PopupKey::Escape,
            KeyCode::Up => PopupKey::Up,
            KeyCode::Down => PopupKey::Down,
            KeyCode::Left => PopupKey::Left,
            KeyCode::Right => PopupKey::Right,
            KeyCode::PageUp => PopupKey::PageUp,
            KeyCode::PageDown => PopupKey::PageDown,
            KeyCode::Tab => PopupKey::Tab,
            KeyCode::Space => PopupKey::Space,
            KeyCode::Backspace => PopupKey::Backspace,
            // 특수문자 팝업 열 점프: 물리 키 위치 기준 (레이아웃 무관)
            KeyCode::Q => PopupKey::Letter(0),
            KeyCode::W => PopupKey::Letter(1),
            KeyCode::E => PopupKey::Letter(2),
            KeyCode::R => PopupKey::Letter(3),
            KeyCode::T => PopupKey::Letter(4),
            KeyCode::Y => PopupKey::Letter(5),
            KeyCode::U => PopupKey::Letter(6),
            KeyCode::I => PopupKey::Letter(7),
            KeyCode::O => PopupKey::Letter(8),
            _ => PopupKey::Other,
        }
    }

    /// 팝업(한자/특수문자) 활성 상태에서 키를 처리합니다.
    fn process_popup_key(
        &mut self,
        keycode: KeyCode,
        _modifier: ModifierState,
        config: &Config,
    ) -> InputResult {
        let popup_key = Self::keycode_to_popup_key(keycode);

        let result = if let Some(ref mut state) = self.popup_state {
            state.handle_key(popup_key)
        } else {
            PopupKeyResult::NotHandled
        };

        match result {
            PopupKeyResult::Select(abs_index) => self.popup_select(abs_index),

            PopupKeyResult::Cancel => {
                self.popup_cancel();
                InputResult::committed()
            }

            PopupKeyResult::Updated => {
                // PopupState 내부 상태가 변경됨 → PopupNavigate 액션 발행
                if let Some(ref state) = self.popup_state {
                    self.popup_pending_action = Some(PopupAction::PopupNavigate {
                        page: state.current_page(),
                        total_pages: state.total_pages(),
                        selected: state.sel_row(),
                        rows: state.rows(),
                        cols: state.cols(),
                        sel_row: state.sel_row(),
                        sel_col: state.sel_col(),
                    });
                }
                // preedit_updated: 트리거 문자를 preedit으로 유지
                InputResult::preedit_updated()
            }

            // preedit_updated: 트리거 문자를 preedit으로 유지
            PopupKeyResult::Consumed => InputResult::preedit_updated(),

            PopupKeyResult::NotHandled => {
                unim_log!("ENGINE", "팝업 미지원 키 {:?} → 팝업 닫고 재처리", keycode);
                self.popup_cancel();
                // 키를 다시 처리 (재귀 방지: popup 모드가 이미 해제됨)
                self.press_key(keycode, _modifier, config)
            }
        }
    }

    /// 팝업에서 항목 선택 처리
    fn popup_select(&mut self, abs_index: usize) -> InputResult {
        if self.hanja_mode {
            if let Some(hanja) = self.select_hanja(abs_index) {
                unim_log!("ENGINE", "팝업 한자 선택: [{}] '{}'", abs_index, hanja);
                self.commit_buffer.push_str(&hanja);
                self.popup_pending_action = Some(PopupAction::HidePopup);
                return InputResult::committed();
            }
        } else if self.special_char_mode {
            if let Some(ch) = self.select_special_char(abs_index) {
                unim_log!("ENGINE", "팝업 특수문자 선택: [{}] '{}'", abs_index, ch);
                self.commit_buffer.push(ch);
                self.popup_pending_action = Some(PopupAction::HidePopup);
                return InputResult::committed();
            }
        }
        InputResult::consumed()
    }

    /// 팝업 취소 처리 — 원래 한글/초성을 그대로 커밋
    fn popup_cancel(&mut self) {
        if self.hanja_mode {
            if !self.hanja_target.is_empty() {
                self.commit_buffer.push_str(&self.hanja_target);
            }
            self.cancel_hanja();
        } else if self.special_char_mode {
            if !self.special_char_target.is_empty() {
                self.commit_buffer.push_str(&self.special_char_target);
            }
            self.cancel_special_char();
        }
        self.popup_pending_action = Some(PopupAction::HidePopup);
    }

    /// 현재 팝업 상태에 대한 참조를 반환합니다.
    pub fn popup_state(&self) -> Option<&PopupState> {
        self.popup_state.as_ref()
    }

    /// 현재 팝업 상태에 대한 가변 참조를 반환합니다.
    pub fn popup_state_mut(&mut self) -> Option<&mut PopupState> {
        self.popup_state.as_mut()
    }

    // =========================================
    // Content Type / Surrounding Text
    // =========================================

    /// 입력 필드의 목적을 설정합니다.
    ///
    /// 비밀번호/PIN 필드에서는 한글 모드가 자동으로 차단됩니다.
    pub fn set_content_purpose(&mut self, purpose: ContentPurpose) {
        if self.content_purpose != purpose {
            unim_log!(
                "ENGINE",
                "content_purpose 변경: {:?} -> {:?}",
                self.content_purpose,
                purpose
            );
            self.content_purpose = purpose;

            // 비밀번호 필드로 전환 시 한글 모드면 즉시 영문 전환
            if purpose.should_block_hangul() && self.input_category == InputCategory::Korean {
                self.flush_preedit();
                self.input_category = InputCategory::English;
                self.update_status_file();
            }
        }
    }

    /// 현재 content purpose를 반환합니다.
    pub fn content_purpose(&self) -> ContentPurpose {
        self.content_purpose
    }

    /// Surrounding text를 설정합니다.
    pub fn set_surrounding_text(&mut self, text: String, cursor_pos: u32, anchor_pos: u32) {
        self.surrounding_text = text;
        self.surrounding_cursor = cursor_pos;
        self.surrounding_anchor = anchor_pos;
    }

    /// Surrounding text를 반환합니다.
    pub fn surrounding_text(&self) -> (&str, u32, u32) {
        (&self.surrounding_text, self.surrounding_cursor, self.surrounding_anchor)
    }

    // =========================================
    // Smart Backspace (자모 단위 삭제)
    // =========================================

    /// Smart Backspace: 커밋된 한글 글자를 자모 단위로 삭제합니다.
    ///
    /// 조합 중이 아닌 상태에서 백스페이스를 누르면, surrounding text의 커서 앞
    /// 한글 글자를 분해하여 마지막 자모를 제거하고 재조합합니다.
    ///
    /// # Returns
    /// * `Some((1, replacement))` - 1글자 삭제 후 대체 텍스트
    /// * `None` - surrounding text가 없거나 한글이 아님
    pub fn smart_backspace(&self) -> Option<(u32, String)> {
        if self.surrounding_text.is_empty() || self.surrounding_cursor == 0 {
            return None;
        }

        // 커서 앞의 마지막 문자를 가져옴
        let text_before: String = self
            .surrounding_text
            .chars()
            .take(self.surrounding_cursor as usize)
            .collect();

        let last_char = text_before.chars().last()?;

        // 한글 음절인지 확인 (U+AC00 ~ U+D7A3)
        if !('\u{AC00}'..='\u{D7A3}').contains(&last_char) {
            return None;
        }

        // 음절을 분해
        use crate::hangul::char::HangulChar;
        let hchar = HangulChar::from_syllable(last_char).ok()?;

        let cho = hchar.get_cho();
        let jung = hchar.get_jung();
        let jong = hchar.get_jong();

        use crate::hangul::jamo::Jong;

        // 종성이 있으면 종성을 제거
        if let Some(j) = jong {
            if j != Jong::E {
                // 종성을 제거하고 초+중으로 재조합
                let new_char = HangulChar::from_jamo_sequences(
                    cho.unwrap() as i32,
                    jung.unwrap() as i32,
                    Jong::E as i32,
                );
                return Some((1, new_char.to_string()));
            }
        }

        // 중성이 있으면 중성을 제거 → 초성만 남음
        if jung.is_some() {
            if let Some(c) = cho {
                // 초성만으로 HangulChar 생성
                let result = HangulChar::from_jamo_sequences(
                    c as i32,
                    -1, // 중성 없음
                    -1, // 종성 없음
                );
                return Some((1, result.to_string()));
            }
        }

        // 초성만 있으면 글자 전체 삭제 (빈 문자열 반환)
        Some((1, String::new()))
    }

    // =========================================
    // TypeFix (한/영 오타 변환)
    // =========================================

    /// TypeFix 변환을 수행합니다.
    ///
    /// 선택된 텍스트(cursor != anchor)만 변환합니다.
    /// 선택 영역이 없으면 변환하지 않습니다.
    ///
    /// 변환 후 결과 언어에 맞게 입력 모드를 자동 전환합니다.
    ///
    /// # Arguments
    /// * `direction` - 0: 자동 감지, 1: 영→한, 2: 한→영
    ///
    /// # Returns
    /// * `Some((delete_chars, replacement))` - 변환 성공
    /// * `None` - 선택 영역이 없거나 surrounding text가 없음
    pub fn typefix_convert(&mut self, direction: u32) -> Option<(u32, String)> {
        if self.surrounding_text.is_empty() {
            return None;
        }

        let chars: Vec<char> = self.surrounding_text.chars().collect();
        let cursor = self.surrounding_cursor as usize;
        let anchor = self.surrounding_anchor as usize;

        // 선택 영역이 없으면 변환하지 않음
        if cursor == anchor {
            return None;
        }

        let start = cursor.min(anchor);
        let end = cursor.max(anchor);
        let word: String = chars[start..end.min(chars.len())].iter().collect();
        let delete_chars = word.chars().count() as u32;

        if word.is_empty() {
            return None;
        }

        let korean_layout = self.korean_layout;
        let english_layout = self.english_layout;

        // 자동 감지 + 변환
        let (replacement, target_mode) = match direction {
            1 => {
                // 영→한 강제
                let converted = crate::typefix::eng_to_kor(&word, korean_layout, english_layout);
                if converted != word {
                    (Some(converted), Some(InputCategory::Korean))
                } else {
                    (None, None)
                }
            }
            2 => {
                // 한→영 강제
                let converted = crate::typefix::kor_to_eng(&word, korean_layout, english_layout);
                if converted != word {
                    (Some(converted), Some(InputCategory::English))
                } else {
                    (None, None)
                }
            }
            _ => {
                // 자동 감지: 한글이면 한→영, 영문이면 영→한
                if crate::typefix::is_korean_text(&word) {
                    let converted =
                        crate::typefix::kor_to_eng(&word, korean_layout, english_layout);
                    if converted != word {
                        (Some(converted), Some(InputCategory::English))
                    } else {
                        (None, None)
                    }
                } else {
                    let converted =
                        crate::typefix::eng_to_kor(&word, korean_layout, english_layout);
                    if converted != word {
                        (Some(converted), Some(InputCategory::Korean))
                    } else {
                        (None, None)
                    }
                }
            }
        };

        if let Some(ref repl) = replacement {
            // 변환 결과 언어로 입력 모드 자동 전환
            if let Some(mode) = target_mode {
                if self.input_category != mode {
                    unim_log!(
                        "ENGINE",
                        "TypeFix 모드 전환: {:?} → {:?}",
                        self.input_category,
                        mode
                    );
                    self.input_category = mode;
                    self.update_status_file();
                }
            }
            Some((delete_chars, repl.clone()))
        } else {
            None
        }
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
        assert_eq!(engine.input_category(), InputCategory::English);
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
        assert_eq!(engine.input_category(), InputCategory::English);

        let config = Config::default();
        let modifier = ModifierState::default();

        engine.press_key(KeyCode::Korean, modifier, &config);
        assert_eq!(engine.input_category(), InputCategory::Korean);

        engine.press_key(KeyCode::Korean, modifier, &config);
        assert_eq!(engine.input_category(), InputCategory::English);
    }

    #[test]
    fn test_reset() {
        let mut engine = create_test_engine();
        engine.commit_buffer.push_str("test");

        engine.reset();
        assert!(engine.commit_str().is_empty());
        assert!(engine.preedit_str().is_empty());
    }

    // === 한국어 모드 기본 키 입력 테스트 ===

    #[test]
    fn test_korean_basic_input() {
        let mut engine = create_test_engine();
        let config = Config::default();
        let modifier = ModifierState::default();

        // 한글 모드로 전환
        engine.set_input_category(InputCategory::Korean);

        // ㄱ 입력
        let result = engine.press_key(KeyCode::R, modifier, &config);
        assert!(result.consumed);
        assert!(result.preedit_changed);
        assert_eq!(engine.preedit_str(), "ㄱ");

        // ㅏ 입력 → 가
        let result = engine.press_key(KeyCode::K, modifier, &config);
        assert!(result.consumed);
        assert_eq!(engine.preedit_str(), "가");
    }

    #[test]
    fn test_korean_syllable_commit() {
        let mut engine = create_test_engine();
        let config = Config::default();
        let modifier = ModifierState::default();

        engine.set_input_category(InputCategory::Korean);

        // 가 입력 (ㄱ + ㅏ)
        engine.press_key(KeyCode::R, modifier, &config);
        engine.press_key(KeyCode::K, modifier, &config);

        // ㄴ 입력 → 2벌식: 종성으로 추가되어 '간'
        let result = engine.press_key(KeyCode::S, modifier, &config);
        assert!(result.consumed);
        assert!(!result.commit_changed);
        assert_eq!(engine.preedit_str(), "간");

        // ㅏ 입력 → 도깨비불: '가' 커밋 + '나' preedit
        let result = engine.press_key(KeyCode::K, modifier, &config);
        assert!(result.consumed);
        assert!(result.commit_changed);
        assert_eq!(engine.commit_str(), "가");
        assert_eq!(engine.preedit_str(), "나");
    }

    // === Modifier 키 테스트 ===

    #[test]
    fn test_modifier_key_not_consumed() {
        let mut engine = create_test_engine();
        let config = Config::default();
        let modifier = ModifierState::default();

        // Shift만 누르면 무시
        let result = engine.press_key(KeyCode::LeftShift, modifier, &config);
        assert!(!result.consumed);
    }

    #[test]
    fn test_ctrl_flushes_preedit() {
        let mut engine = create_test_engine();
        let config = Config::default();
        let modifier = ModifierState::default();

        engine.set_input_category(InputCategory::Korean);
        engine.press_key(KeyCode::R, modifier, &config); // ㄱ
        engine.press_key(KeyCode::K, modifier, &config); // 가

        // Ctrl+C → 조합 커밋
        let ctrl_modifier = ModifierState {
            control: true,
            ..Default::default()
        };
        let result = engine.press_key(KeyCode::C, ctrl_modifier, &config);
        assert!(result.commit_changed);
        assert_eq!(engine.commit_str(), "가");
    }

    // === Space 처리 테스트 ===

    #[test]
    fn test_korean_space_commits() {
        let mut engine = create_test_engine();
        let config = Config::default();
        let modifier = ModifierState::default();

        engine.set_input_category(InputCategory::Korean);
        engine.press_key(KeyCode::R, modifier, &config); // ㄱ
        engine.press_key(KeyCode::K, modifier, &config); // 가

        let result = engine.press_key(KeyCode::Space, modifier, &config);
        assert!(result.consumed);
        assert!(result.commit_changed);
        // "가" + " "
        assert!(engine.commit_str().contains("가"));
    }

    // === Enter/Tab/Escape → committed_passthrough 테스트 ===

    #[test]
    fn test_enter_commits_passthrough() {
        let mut engine = create_test_engine();
        let config = Config::default();
        let modifier = ModifierState::default();

        engine.set_input_category(InputCategory::Korean);
        engine.press_key(KeyCode::R, modifier, &config); // ㄱ

        let result = engine.press_key(KeyCode::Enter, modifier, &config);
        assert!(!result.consumed); // passthrough
        assert!(result.commit_changed);
    }

    #[test]
    fn test_enter_not_composing_passthrough() {
        let mut engine = create_test_engine();
        let config = Config::default();
        let modifier = ModifierState::default();

        engine.set_input_category(InputCategory::Korean);
        let result = engine.press_key(KeyCode::Enter, modifier, &config);
        assert!(!result.consumed);
        assert!(!result.commit_changed);
    }

    // === Backspace 테스트 ===

    #[test]
    fn test_korean_backspace() {
        let mut engine = create_test_engine();
        let config = Config::default();
        let modifier = ModifierState::default();

        engine.set_input_category(InputCategory::Korean);
        engine.press_key(KeyCode::R, modifier, &config); // ㄱ
        engine.press_key(KeyCode::K, modifier, &config); // 가

        let result = engine.press_key(KeyCode::Backspace, modifier, &config);
        assert!(result.consumed);
        assert!(result.preedit_changed);
        assert_eq!(engine.preedit_str(), "ㄱ");
    }

    #[test]
    fn test_backspace_not_composing() {
        let mut engine = create_test_engine();
        let config = Config::default();
        let modifier = ModifierState::default();

        engine.set_input_category(InputCategory::Korean);
        let result = engine.press_key(KeyCode::Backspace, modifier, &config);
        assert!(!result.consumed); // 앱으로 전달
    }

    // === 도깨비불 through engine 테스트 ===

    #[test]
    fn test_engine_dokkaebi() {
        let mut engine = create_test_engine();
        let config = Config::default();
        let modifier = ModifierState::default();

        engine.set_input_category(InputCategory::Korean);
        // ㄱㅏㄱ → 각
        engine.press_key(KeyCode::R, modifier, &config);
        engine.press_key(KeyCode::K, modifier, &config);
        engine.press_key(KeyCode::R, modifier, &config);
        assert_eq!(engine.preedit_str(), "각");

        // ㅏ → 도깨비불 → 가 + 가
        engine.press_key(KeyCode::K, modifier, &config);
        assert_eq!(engine.commit_str(), "가");
        assert_eq!(engine.preedit_str(), "가");
    }

    // === 영어 모드 Shift 테스트 ===

    #[test]
    fn test_english_shift_uppercase() {
        let mut engine = create_test_engine();
        let config = Config::default();

        engine.set_input_category(InputCategory::English);
        let shift_modifier = ModifierState {
            shift: true,
            ..Default::default()
        };

        let result = engine.press_key(KeyCode::A, shift_modifier, &config);
        assert!(result.consumed);
        assert_eq!(engine.commit_str(), "A");
    }

    // === 레이아웃 변경 테스트 ===

    #[test]
    fn test_set_korean_layout() {
        let mut config = Config::default();
        config.engine.korean.layout = KoreanLayout::Sebeolsik390;

        let engine = InputEngine::new(&config);
        assert_eq!(engine.korean_layout, KoreanLayout::Sebeolsik390);
    }

    #[test]
    fn test_set_english_layout_dvorak() {
        let mut config = Config::default();
        config.engine.english.layout = EnglishLayout::Dvorak;

        let engine = InputEngine::new(&config);
        assert_eq!(engine.english_layout, EnglishLayout::Dvorak);
    }

    // === 한/영 전환 중 조합 커밋 테스트 ===

    #[test]
    fn test_toggle_while_composing() {
        let mut engine = create_test_engine();
        let config = Config::default();
        let modifier = ModifierState::default();

        engine.set_input_category(InputCategory::Korean);
        engine.press_key(KeyCode::R, modifier, &config); // ㄱ
        engine.press_key(KeyCode::K, modifier, &config); // 가

        // 한/영 전환 → 조합 커밋
        let result = engine.press_key(KeyCode::Korean, modifier, &config);
        assert!(result.consumed);
        assert!(result.commit_changed);
        assert_eq!(engine.commit_str(), "가");
        assert_eq!(engine.input_category(), InputCategory::English);
    }

    // =========================================
    // 통합 테스트 시나리오
    // =========================================

    /// 헬퍼: 키 시퀀스를 입력하고 최종 결과를 수집
    fn type_keys(engine: &mut InputEngine, keys: &[KeyCode], config: &Config) -> (String, String) {
        let modifier = ModifierState::default();
        let mut total_commit = String::new();
        for &key in keys {
            let result = engine.press_key(key, modifier, config);
            if result.commit_changed {
                total_commit.push_str(engine.commit_str());
                engine.clear_commit();
            }
        }
        (total_commit, engine.preedit_str().to_string())
    }

    #[test]
    fn test_scenario_hangul_sentence() {
        // "안녕하세요" 입력 시나리오
        let mut engine = create_test_engine();
        let config = Config::default();
        engine.set_input_category(InputCategory::Korean);

        let keys = [
            KeyCode::D, KeyCode::K,   // 아 → ㅏ
            KeyCode::S, KeyCode::S,   // 안 → ㄴ+ㄴ (도깨비불)
            KeyCode::U, KeyCode::D,   // 녕
            KeyCode::G, KeyCode::K,   // 하
            KeyCode::T, KeyCode::P,   // 세
            KeyCode::D, KeyCode::Y,   // 요
        ];

        let (commit, preedit) = type_keys(&mut engine, &keys, &config);
        // 최종: 커밋된 텍스트 + 남은 preedit
        let full = format!("{}{}", commit, preedit);
        assert!(full.contains("안녕"), "Expected '안녕' in '{}'", full);
    }

    #[test]
    fn test_scenario_mixed_korean_english() {
        // 한글 입력 → 한/영 전환 → 영문 입력 → 한/영 전환 → 한글 입력
        let mut engine = create_test_engine();
        let config = Config::default();
        let modifier = ModifierState::default();

        // 1. 한글 모드: "가" 입력
        engine.set_input_category(InputCategory::Korean);
        engine.press_key(KeyCode::R, modifier, &config); // ㄱ
        engine.press_key(KeyCode::K, modifier, &config); // 가
        assert_eq!(engine.preedit_str(), "가");

        // 2. 한/영 전환
        let result = engine.press_key(KeyCode::Korean, modifier, &config);
        assert!(result.commit_changed);
        let commit1 = engine.commit_str().to_string();
        engine.clear_commit();
        assert_eq!(commit1, "가");
        assert_eq!(engine.input_category(), InputCategory::English);

        // 3. 영문 입력 "ab"
        engine.press_key(KeyCode::A, modifier, &config);
        let a = engine.commit_str().to_string();
        engine.clear_commit();
        assert_eq!(a, "a");

        engine.press_key(KeyCode::B, modifier, &config);
        let b = engine.commit_str().to_string();
        engine.clear_commit();
        assert_eq!(b, "b");

        // 4. 한/영 전환 → 한글
        engine.press_key(KeyCode::Korean, modifier, &config);
        assert_eq!(engine.input_category(), InputCategory::Korean);

        // 5. 한글 "나"
        engine.press_key(KeyCode::S, modifier, &config); // ㄴ
        engine.press_key(KeyCode::K, modifier, &config); // 나
        assert_eq!(engine.preedit_str(), "나");
    }

    #[test]
    fn test_scenario_backspace_during_composition() {
        // 조합 중 백스페이스: "각" → Backspace → "가" → Backspace → "ㄱ" → Backspace → 빈칸
        let mut engine = create_test_engine();
        let config = Config::default();
        let modifier = ModifierState::default();

        engine.set_input_category(InputCategory::Korean);

        // ㄱ+ㅏ+ㄱ = 각
        engine.press_key(KeyCode::R, modifier, &config);
        engine.press_key(KeyCode::K, modifier, &config);
        engine.press_key(KeyCode::R, modifier, &config);
        assert_eq!(engine.preedit_str(), "각");

        // Backspace → 가
        engine.press_key(KeyCode::Backspace, modifier, &config);
        assert_eq!(engine.preedit_str(), "가");

        // Backspace → ㄱ
        engine.press_key(KeyCode::Backspace, modifier, &config);
        assert_eq!(engine.preedit_str(), "ㄱ");

        // Backspace → 빈칸
        engine.press_key(KeyCode::Backspace, modifier, &config);
        assert_eq!(engine.preedit_str(), "");
        assert!(!engine.is_composing());
    }

    #[test]
    fn test_scenario_content_purpose_password() {
        // 비밀번호 필드에서 한글 차단
        let mut engine = create_test_engine();
        let config = Config::default();
        let modifier = ModifierState::default();

        // 한글 모드로 전환
        engine.set_input_category(InputCategory::Korean);
        assert_eq!(engine.input_category(), InputCategory::Korean);

        // 비밀번호 목적 설정 → 자동 영문 전환
        engine.set_content_purpose(ContentPurpose::Password);
        assert_eq!(engine.input_category(), InputCategory::English);

        // 한/영 전환 시도 → 차단
        let result = engine.press_key(KeyCode::Korean, modifier, &config);
        assert!(result.consumed);
        assert_eq!(engine.input_category(), InputCategory::English);

        // 영문 입력은 정상 동작
        engine.press_key(KeyCode::A, modifier, &config);
        assert_eq!(engine.commit_str(), "a");
    }

    #[test]
    fn test_scenario_content_purpose_normal_after_password() {
        // 비밀번호 → Normal 전환 시 한글 모드 복구 가능
        let mut engine = create_test_engine();
        let config = Config::default();
        let modifier = ModifierState::default();

        engine.set_input_category(InputCategory::Korean);
        engine.set_content_purpose(ContentPurpose::Password);
        assert_eq!(engine.input_category(), InputCategory::English);

        // Normal로 복원
        engine.set_content_purpose(ContentPurpose::Normal);

        // 이제 한/영 전환 가능
        engine.press_key(KeyCode::Korean, modifier, &config);
        assert_eq!(engine.input_category(), InputCategory::Korean);
    }

    #[test]
    fn test_scenario_typefix_with_selection() {
        // TypeFix: 선택된 텍스트 영→한 변환 + 모드 자동 전환
        let mut engine = create_test_engine();
        engine.set_input_category(InputCategory::English);

        // "gksrmf" 전체 선택 (cursor=6, anchor=0)
        engine.set_surrounding_text("gksrmf".to_string(), 6, 0);

        // TypeFix 자동 감지 (영문 → 한글)
        let result = engine.typefix_convert(0);
        assert!(result.is_some());
        let (delete_count, replacement) = result.unwrap();
        assert_eq!(delete_count, 6);
        assert_eq!(replacement, "한글");
        assert_eq!(engine.input_category(), InputCategory::Korean);
    }

    #[test]
    fn test_scenario_typefix_no_selection_returns_none() {
        // TypeFix: 선택 없으면 None 반환
        let mut engine = create_test_engine();
        engine.set_surrounding_text("gksrmf".to_string(), 6, 6);
        assert!(engine.typefix_convert(0).is_none());
    }

    #[test]
    fn test_scenario_typefix_kor_to_eng() {
        // TypeFix: 한글 → 영문 강제 변환 (선택 필수)
        let mut engine = create_test_engine();
        engine.set_input_category(InputCategory::Korean);

        // "한글" 전체 선택 (cursor=2, anchor=0)
        engine.set_surrounding_text("한글".to_string(), 2, 0);

        let result = engine.typefix_convert(2);
        assert!(result.is_some());
        let (delete_count, replacement) = result.unwrap();
        assert_eq!(delete_count, 2);
        assert_eq!(replacement, "gksrmf");
        assert_eq!(engine.input_category(), InputCategory::English);
    }

    #[test]
    fn test_scenario_typefix_selection() {
        // 선택 영역 TypeFix: cursor != anchor일 때 선택 영역 변환
        let mut engine = create_test_engine();
        engine.set_input_category(InputCategory::English);

        // "hello gksrmf world" 에서 "gksrmf"가 선택됨 (cursor=12, anchor=6)
        engine.set_surrounding_text("hello gksrmf world".to_string(), 12, 6);

        let result = engine.typefix_convert(0);
        assert!(result.is_some());
        let (delete_count, replacement) = result.unwrap();
        assert_eq!(delete_count, 6); // "gksrmf" 6글자 삭제
        assert_eq!(replacement, "한글");
        assert_eq!(engine.input_category(), InputCategory::Korean);
    }

    #[test]
    fn test_scenario_typefix_auto_detect() {
        // 자동 감지: 한글 자모 선택 → 영문으로 변환
        let mut engine = create_test_engine();
        engine.set_input_category(InputCategory::Korean);

        // "ㅗ디ㅣㅐ" 전체 선택 (cursor=4, anchor=0)
        engine.set_surrounding_text("ㅗ디ㅣㅐ".to_string(), 4, 0);

        let result = engine.typefix_convert(0);
        assert!(result.is_some());
        let (_delete_count, replacement) = result.unwrap();
        assert!(!replacement.is_empty());
        assert_eq!(engine.input_category(), InputCategory::English);
    }

    #[test]
    fn test_scenario_smart_backspace() {
        // Smart Backspace: 커밋된 "한" → "하" → "ㅎ" → 삭제
        let mut engine = create_test_engine();

        // "한" 글자 뒤에 커서
        engine.set_surrounding_text("한".to_string(), 1, 1);
        let result = engine.smart_backspace();
        assert!(result.is_some());
        let (del, repl) = result.unwrap();
        assert_eq!(del, 1);
        assert_eq!(repl, "하"); // 종성 ㄴ 제거 → 하

        // "하" 글자 뒤에 커서
        engine.set_surrounding_text("하".to_string(), 1, 1);
        let result = engine.smart_backspace();
        assert!(result.is_some());
        let (del, repl) = result.unwrap();
        assert_eq!(del, 1);
        assert_eq!(repl, "ㅎ"); // 중성 ㅏ 제거 → ㅎ

        // "ㅎ" 글자 → 한글 음절이 아니므로 None
        engine.set_surrounding_text("ㅎ".to_string(), 1, 1);
        let result = engine.smart_backspace();
        assert!(result.is_none()); // 자모는 음절이 아님
    }

    #[test]
    fn test_scenario_hanja_conversion() {
        // 한자 변환 시나리오: "가" → 한자 후보 표시 → 선택
        let mut engine = create_test_engine();
        let config = Config::default();
        let modifier = ModifierState::default();

        engine.set_input_category(InputCategory::Korean);

        // "가" 입력
        engine.press_key(KeyCode::R, modifier, &config); // ㄱ
        engine.press_key(KeyCode::K, modifier, &config); // 가
        assert_eq!(engine.preedit_str(), "가");

        // 한자 변환 시작
        let result = engine.start_hanja_conversion();
        assert!(result.hanja_candidates_available);
        assert!(engine.is_hanja_mode());

        // 후보 목록 확인
        let candidates = engine.get_hanja_candidates();
        assert!(!candidates.is_empty());

        // 첫 번째 한자 선택
        let selected = engine.select_hanja(0);
        assert!(selected.is_some());
        assert!(!engine.is_hanja_mode()); // 모드 해제
    }

    #[test]
    fn test_scenario_special_char_fallback() {
        // 특수문자 fallback: 초성 "ㄱ" → 한자 후보 없음 → 특수문자 후보
        let mut engine = create_test_engine();
        let config = Config::default();
        let modifier = ModifierState::default();

        engine.set_input_category(InputCategory::Korean);

        // "ㄱ" 입력 (초성만)
        engine.press_key(KeyCode::R, modifier, &config);
        assert_eq!(engine.preedit_str(), "ㄱ");

        // 한자 변환 시작 → 한자 없음 → 특수문자 fallback
        let result = engine.start_hanja_conversion();
        // ㄱ에 대한 한자가 없으면 특수문자 모드로 전환
        if result.special_char_candidates_available {
            assert!(engine.is_special_char_mode());
            let candidates = engine.get_special_char_candidates();
            assert!(!candidates.is_empty());
        }
    }

    #[test]
    fn test_scenario_double_consonant() {
        // 쌍자음 입력: ㄲ (Shift+ㄱ)
        let mut engine = create_test_engine();
        let config = Config::default();
        let shift = ModifierState {
            shift: true,
            ..Default::default()
        };
        let modifier = ModifierState::default();

        engine.set_input_category(InputCategory::Korean);

        // Shift+R = ㄲ
        engine.press_key(KeyCode::R, shift, &config);
        assert_eq!(engine.preedit_str(), "ㄲ");

        // ㅏ → 까
        engine.press_key(KeyCode::K, modifier, &config);
        assert_eq!(engine.preedit_str(), "까");
    }

    #[test]
    fn test_scenario_space_after_composition() {
        // 조합 후 스페이스: "가" + Space → "가 " 커밋
        let mut engine = create_test_engine();
        let config = Config::default();
        let modifier = ModifierState::default();

        engine.set_input_category(InputCategory::Korean);

        engine.press_key(KeyCode::R, modifier, &config);
        engine.press_key(KeyCode::K, modifier, &config);
        assert_eq!(engine.preedit_str(), "가");

        let result = engine.press_key(KeyCode::Space, modifier, &config);
        assert!(result.consumed);
        assert!(result.commit_changed);
        assert_eq!(engine.commit_str(), "가 ");
        assert_eq!(engine.preedit_str(), "");
    }

    #[test]
    fn test_scenario_number_in_korean_mode() {
        // 한글 모드에서 숫자: 조합 커밋 후 숫자 커밋
        let mut engine = create_test_engine();
        let config = Config::default();
        let modifier = ModifierState::default();

        engine.set_input_category(InputCategory::Korean);

        engine.press_key(KeyCode::R, modifier, &config); // ㄱ
        engine.press_key(KeyCode::K, modifier, &config); // 가

        // 숫자 1 → 조합 "가" 커밋 + "1" 커밋
        let result = engine.press_key(KeyCode::Num1, modifier, &config);
        assert!(result.commit_changed);
        let committed = engine.commit_str().to_string();
        assert!(committed.contains("가"), "committed: '{}'", committed);
    }

    #[test]
    fn test_scenario_caps_lock_korean() {
        // 한글 모드에서 CapsLock → 영향 없음 (쌍자음은 Shift로만)
        let mut engine = create_test_engine();
        let config = Config::default();
        let caps = ModifierState {
            caps_lock: true,
            ..Default::default()
        };

        engine.set_input_category(InputCategory::Korean);

        // CapsLock 상태에서 R → ㄱ (CapsLock 무시)
        engine.press_key(KeyCode::R, caps, &config);
        assert_eq!(engine.preedit_str(), "ㄱ");
    }

    #[test]
    fn test_scenario_caps_lock_english() {
        // 영어 모드에서 CapsLock → 대문자
        let mut engine = create_test_engine();
        let config = Config::default();
        let caps = ModifierState {
            caps_lock: true,
            ..Default::default()
        };

        engine.set_input_category(InputCategory::English);

        engine.press_key(KeyCode::A, caps, &config);
        assert_eq!(engine.commit_str(), "A");
    }
}
