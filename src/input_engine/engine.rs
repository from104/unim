//! `InputEngine` 구조체 정의 + 생성·게터·세터·레이아웃 변경.
//!
//! press 계열 hot path는 `press_key.rs`, 한자/특수문자는 `candidates.rs`,
//! 팝업 디스패치는 `popup_dispatch.rs`, content_purpose/surrounding은 `surrounding.rs`로
//! 분산 `impl InputEngine` 블록으로 구현된다.

use super::build_korean_context;
use super::types::{InputResult, PopupAction};
use crate::config::{Config, ContentPurpose, EnglishLayout, InputCategory, KoreanLayout};
use crate::hangul::input_context::{ComposerType, HangulInputContext};
use crate::hangul::jamo::JamoEnum;
use crate::keycode::{KeyCode, ModifierState};
use crate::keystroke::EnglishKeymap;
use crate::popup::PopupState;
use std::collections::HashMap;

/// 입력 엔진
///
/// 키 입력을 받아 한국어 조합을 처리하고 preedit/commit 문자열을 관리합니다.
pub struct InputEngine {
    /// 현재 입력 카테고리 (한국어/영어)
    pub(super) input_category: InputCategory,
    /// 한국어 입력 컨텍스트
    pub(super) korean_context: HangulInputContext,
    /// commit 버퍼
    pub(super) commit_buffer: String,
    /// preedit 버퍼 (캐시)
    pub(super) preedit_cache: String,
    /// 키보드 맵 (한국어) - 영어 키 -> 한국어 자모 매핑
    pub(super) keyboard_map: Option<HashMap<char, JamoEnum>>,
    /// 영어 키보드 레이아웃 맵 (JSON 기반 동적 로드)
    pub(super) english_keymap: EnglishKeymap,
    /// 영어 키보드 레이아웃 설정
    pub(super) english_layout: EnglishLayout,
    /// 한국어 키보드 레이아웃 설정 캐시
    pub(super) korean_layout: KoreanLayout,
    /// 한자 사전 (Arc로 래핑하여 공유)
    pub(super) hanja_dict: std::sync::Arc<crate::hanja::HanjaDictionary>,
    /// 한자 즐겨찾기 저장소 (영구 저장)
    pub(super) hanja_bookmarks: crate::hanja::HanjaBookmarkStore,
    /// 현재 한자 후보 목록
    pub(super) hanja_candidates: Vec<crate::hanja::HanjaEntry>,
    /// 한자 선택 모드 활성화 여부
    pub(super) hanja_mode: bool,
    /// 한자 변환 대상 문자열 (preedit 또는 마지막 음절)
    pub(super) hanja_target: String,
    /// 특수문자 모드 활성화 여부
    pub(super) special_char_mode: bool,
    /// 현재 특수문자 후보 목록
    pub(super) special_char_candidates: Vec<char>,
    /// 특수문자 변환 대상 초성
    pub(super) special_char_target: String,
    /// 한/영 전환 키 목록 (설정 기반)
    pub(super) toggle_keys: Vec<KeyCode>,
    /// 자동 영문 전환 활성화 여부 (설정 캐시)
    pub(super) auto_english_enabled: bool,
    /// 자동 영문 전환 트리거 (파싱된 `(KeyCode, Shift 조건)` 캐시)
    ///
    /// - `(code, None)`: shift 무관 매칭 (Escape 등 제어 키)
    /// - `(code, Some(true))`: shift 필수 (문자 키의 shift 문자. 예: `ShiftSemicolon` → `:`)
    /// - `(code, Some(false))`: shift 없어야 함 (기본 문자 키. 예: `Slash` → `/`)
    pub(super) auto_english_triggers: Vec<(KeyCode, Option<bool>)>,
    /// 이모지 팝업 트리거 (modifier, keycode) 쌍 목록
    pub(super) emoji_triggers: Vec<(ModifierState, KeyCode)>,
    /// 이모지 팝업 기능 활성 여부
    pub(super) emoji_popup_enabled: bool,
    /// 통합 팝업 상태 (한자/특수문자 공용)
    pub(super) popup_state: Option<PopupState>,
    /// 처리 대기 팝업 액션
    pub(super) popup_pending_action: Option<PopupAction>,
    /// 영어 레이아웃 top_row_labels 캐시 (특수문자 팝업용)
    pub(super) top_row_labels: String,
    /// 현재 입력 필드의 목적 (비밀번호 등)
    pub(super) content_purpose: ContentPurpose,
    /// Surrounding text (커서 주변 텍스트)
    pub(super) surrounding_text: String,
    /// Surrounding text 커서 위치 (문자 단위)
    pub(super) surrounding_cursor: u32,
    /// Surrounding text 앵커 위치 (문자 단위)
    pub(super) surrounding_anchor: u32,
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
        let composer_type = if crate::config::is_sebeolsik_layout(&config.engine.korean.layout) {
            ComposerType::ThreeBul
        } else {
            ComposerType::TwoBul
        };

        let keyboard_map =
            Self::create_keyboard_map(&config.engine.korean.layout, &config.engine.english.layout);

        let english_keymap = Self::create_english_keymap(&config.engine.english.layout);

        // 한자 사전 초기화 (한 번만 로드하여 Arc로 공유)
        let hanja_dict = std::sync::Arc::new(crate::hanja::HanjaDictionary::new());

        Self {
            input_category: config.engine.default_category,
            korean_context: build_korean_context(config, composer_type),
            commit_buffer: String::new(),
            preedit_cache: String::new(),
            keyboard_map: Some(keyboard_map),
            english_keymap,
            korean_layout: config.engine.korean.layout.clone(),
            english_layout: config.engine.english.layout.clone(),
            hanja_dict,
            hanja_bookmarks: crate::hanja::HanjaBookmarkStore::load_default(),
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
            auto_english_enabled: config.engine.auto_english.enabled,
            auto_english_triggers: config
                .engine
                .auto_english
                .trigger_keys
                .iter()
                .filter_map(|n| Self::parse_trigger_key(n))
                .collect(),
            emoji_triggers: config
                .engine
                .emoji_popup
                .trigger_keys
                .iter()
                .filter_map(|s| Self::parse_emoji_trigger(s))
                .collect(),
            emoji_popup_enabled: config.engine.emoji_popup.enabled,
            popup_state: None,
            popup_pending_action: None,
            top_row_labels: crate::config::english_layout_top_row_labels(
                &config.engine.english.layout,
            )
            .to_string(),
            content_purpose: ContentPurpose::Normal,
            surrounding_text: String::new(),
            surrounding_cursor: 0,
            surrounding_anchor: 0,
        }
    }

    /// 이모지 트리거 문자열을 파싱합니다.
    ///
    /// 형식: "Super+Period", "Control+Shift+E" 처럼 `+`로 구분된 토큰.
    /// 각 토큰은 modifier 이름(Super/Control/Alt/Shift) 또는 KeyCode 이름.
    /// 유효하지 않거나 KeyCode가 없는 경우 None을 반환합니다.
    pub(super) fn parse_emoji_trigger(spec: &str) -> Option<(ModifierState, KeyCode)> {
        let mut modifier = ModifierState::new();
        let mut keycode: Option<KeyCode> = None;
        for token in spec.split('+') {
            match token.trim() {
                "" => continue,
                "Super" | "Meta" | "Win" => modifier.super_key = true,
                "Control" | "Ctrl" => modifier.control = true,
                "Alt" => modifier.alt = true,
                "Shift" => modifier.shift = true,
                other => {
                    let kc = KeyCode::from_name(other);
                    if kc == KeyCode::Unknown {
                        return None;
                    }
                    keycode = Some(kc);
                }
            }
        }
        keycode.map(|k| (modifier, k))
    }

    /// 현재 키 입력이 이모지 팝업 트리거와 일치하는지 확인합니다.
    pub(super) fn matches_emoji_trigger(&self, keycode: KeyCode, modifier: ModifierState) -> bool {
        if !self.emoji_popup_enabled || self.emoji_triggers.is_empty() {
            return false;
        }
        self.emoji_triggers.iter().any(|(m, k)| {
            *k == keycode
                && m.shift == modifier.shift
                && m.control == modifier.control
                && m.alt == modifier.alt
                && m.super_key == modifier.super_key
        })
    }

    /// 키보드 맵을 생성합니다.
    ///
    /// # Arguments
    ///
    /// * `korean_layout` - 한국어 키보드 레이아웃
    /// * `english_layout` - 영어 키보드 레이아웃
    pub(super) fn create_keyboard_map(
        korean_layout: &KoreanLayout,
        english_layout: &EnglishLayout,
    ) -> HashMap<char, JamoEnum> {
        let en_keymap = crate::config::english_layout_keymap_name(english_layout);
        let en_json = crate::keystroke::get_keymap_json(&en_keymap);
        let ko_json = crate::keystroke::get_keymap_json(korean_layout);
        let is_three_bul = crate::config::is_sebeolsik_layout(korean_layout);
        crate::keystroke::KeyboardMap::create_keyboard_map_from_str(en_json, ko_json, is_three_bul)
    }

    /// 영어 키맵을 생성합니다.
    ///
    /// # Arguments
    ///
    /// * `layout` - 영어 키보드 레이아웃 프로필 이름
    pub(super) fn create_english_keymap(layout: &EnglishLayout) -> EnglishKeymap {
        let keymap_file = crate::config::english_layout_keymap_name(layout);
        let json = crate::keystroke::get_keymap_json(&keymap_file);
        EnglishKeymap::from_json(json)
    }
}

// ==========================================
// 입력 카테고리 / commit·preedit / 레이아웃 변경
// ==========================================

impl InputEngine {
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
    ///
    /// `InputEngine::new`의 초기화 경로와 동일하게 v1 builder
    /// (`build_korean_context`)를 거쳐 `HangulInputContext`를 만든다.
    /// 단독 setter 경로에서는 `Config`를 받지 않으므로 `active_rule_sets`는
    /// `None`으로 두어 프로필이 정의한 기본값을 사용한다. `active_rule_sets`까지
    /// 함께 갱신하려면 `rebuild_korean_context(&Config)` 사용.
    pub fn set_korean_layout(&mut self, layout: KoreanLayout) {
        if self.korean_layout != layout {
            self.flush_preedit();

            // 키보드 맵 업데이트
            self.keyboard_map = Some(Self::create_keyboard_map(&layout, &self.english_layout));

            // 컨텍스트 업데이트 — v1 builder 경로로 통일
            let composer_type = if crate::config::is_sebeolsik_layout(&layout) {
                ComposerType::ThreeBul
            } else {
                ComposerType::TwoBul
            };
            let mut snapshot = Config::default();
            snapshot.engine.korean.layout = layout.clone();
            snapshot.engine.korean.active_rule_sets = None;
            self.korean_context = build_korean_context(&snapshot, composer_type);
            self.korean_layout = layout;
        }
    }

    /// Config 전체로부터 한국어 컨텍스트를 재구성합니다.
    ///
    /// `set_korean_layout`이 layout 변경만 처리하는 반면, 본 메서드는
    /// `active_rule_sets`/`combinations` 등 v1 프로필 관련 필드 변경 시에도
    /// `build_korean_context` 빌더로 컨텍스트를 다시 만들어 즉시 반영한다.
    /// hot-reload 경로(`engine_worker`)에서 `reload_if_changed` 직후 호출.
    ///
    /// 다른 상태(`hanja_dict`/`hanja_bookmarks`/`input_category` 등)는 보존.
    pub fn rebuild_korean_context(&mut self, config: &Config) {
        self.flush_preedit();

        let new_layout = config.engine.korean.layout.clone();
        let composer_type = if crate::config::is_sebeolsik_layout(&new_layout) {
            ComposerType::ThreeBul
        } else {
            ComposerType::TwoBul
        };

        // 키맵 갱신 (layout 동일이어도 영어 레이아웃과 짝이 맞도록 안전하게 재생성)
        self.keyboard_map = Some(Self::create_keyboard_map(&new_layout, &self.english_layout));

        // v1 builder 경로로 컨텍스트 재구성 (active_rule_sets override 포함)
        self.korean_context = build_korean_context(config, composer_type);
        self.korean_layout = new_layout;
    }

    /// 영어 레이아웃을 설정합니다.
    ///
    /// 레이아웃이 변경되면 키보드 맵과 영어 키맵을 재생성합니다.
    pub fn set_english_layout(&mut self, layout: EnglishLayout) {
        if self.english_layout != layout {
            self.flush_preedit();

            // 한국어 키보드 맵 재생성 (영어 레이아웃과 연동)
            self.keyboard_map = Some(Self::create_keyboard_map(&self.korean_layout, &layout));

            // 영어 키맵 재생성
            self.english_keymap = Self::create_english_keymap(&layout);
            self.english_layout = layout;
        }
    }
}
