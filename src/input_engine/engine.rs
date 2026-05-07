//! `InputEngine` 구조체 정의 + 생성·게터·세터·레이아웃 변경.
//!
//! press 계열 hot path는 `press_key.rs`, 한자/특수문자는 `candidates.rs`,
//! 팝업 디스패치는 `popup_dispatch.rs`, content_purpose/surrounding은 `surrounding.rs`로
//! 분산 `impl InputEngine` 블록으로 구현된다.

use super::build_korean_context;
use super::chord_buffer::ChordBuffer;
use super::types::{AutoEnglishTrigger, InputResult, PopupAction};
use crate::config::{Config, ContentPurpose, EnglishLayout, InputCategory, KoreanLayout};
use crate::hangul::input_context::{ComposerType, HangulInputContext};
use crate::hangul::jamo::JamoEnum;
use crate::keycode::KeyCode;
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
    /// 키별 메타데이터 (schema v2 `key_meta`) — context_alt 분기 등에 사용.
    /// 한국어 레이아웃 기준으로 빌드되며, 누락 시 빈 맵.
    pub(super) key_meta_map: HashMap<char, crate::keystroke::profile::KeyMeta>,
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
    /// 한자 변환 트리거 키 목록 (설정 기반 — 기본 `["Hanja", "F9"]`).
    /// preedit 비었을 때는 dual-purpose 로 emoji 팝업 트리거로 동작.
    pub(super) hanja_keys: Vec<KeyCode>,
    /// 자동 영문 전환 활성화 여부 (설정 캐시)
    pub(super) auto_english_enabled: bool,
    /// 자동 영문 전환 트리거 (파싱된 카테고리별 캐시)
    ///
    /// 두 카테고리:
    /// - `Functional { code, shift }`: KeyCode 비교 (Escape/Tab/F*/Shift 명시 문자)
    /// - `Character(ch)`: keymap 산출 char 비교 (비-QWERTY 한국어 레이아웃 안전)
    ///
    /// 표기 문법: `key:Escape` / `char:/` (접두사). 무접두사는 legacy 호환으로
    /// `Functional` 로 흡수한다.
    pub(super) auto_english_triggers: Vec<AutoEnglishTrigger>,
    /// 통합 팝업 상태 (한자/특수문자 공용)
    pub(super) popup_state: Option<PopupState>,
    /// 처리 대기 팝업 액션
    pub(super) popup_pending_action: Option<PopupAction>,
    /// 영어 레이아웃 top_row_labels 캐시 (특수문자 팝업용)
    pub(super) top_row_labels: String,
    /// 영어 레이아웃 home_row_labels 캐시 (이모지 카테고리 단축키 표시용)
    pub(super) home_row_labels: String,
    /// chord 윈도우 버퍼 — 안마태 모아치기 동시 입력 처리.
    /// `chord_window_ms == 0` 이면 비활성 (즉시 처리, 회귀 0).
    pub(super) chord_buffer: ChordBuffer,
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
        let key_meta_map = Self::create_key_meta_map(&config.engine.korean.layout);

        let english_keymap = Self::create_english_keymap(&config.engine.english.layout);

        // 한자 사전 초기화 (한 번만 로드하여 Arc로 공유)
        let hanja_dict = std::sync::Arc::new(crate::hanja::HanjaDictionary::new());

        Self {
            input_category: config.engine.default_category,
            korean_context: build_korean_context(config, composer_type),
            commit_buffer: String::new(),
            preedit_cache: String::new(),
            keyboard_map: Some(keyboard_map),
            key_meta_map,
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
            hanja_keys: config
                .engine
                .hanja_keys
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
            popup_state: None,
            popup_pending_action: None,
            top_row_labels: crate::config::english_layout_top_row_labels(
                &config.engine.english.layout,
            )
            .to_string(),
            home_row_labels: crate::config::english_layout_home_row_labels(
                &config.engine.english.layout,
            )
            .to_string(),
            chord_buffer: ChordBuffer::new(Self::compute_chord_window_ms(config)),
            content_purpose: ContentPurpose::Normal,
            surrounding_text: String::new(),
            surrounding_cursor: 0,
            surrounding_anchor: 0,
        }
    }

    /// Config에서 유효 chord_window_ms를 계산.
    ///
    /// Phase 5a~: chord_window_ms와 bidirectional_combine은 독립 게이트.
    /// - `moachigi.is_none()` (supports_moachigi=false 자판) → 0 (강제 OFF, 자판 capability 게이트)
    /// - 사용자 config `chord_window_ms` = None → 0 (OPT-IN 디폴트 OFF)
    /// - 사용자 config `chord_window_ms` = Some(0) → 0 (명시적 OFF)
    /// - 사용자 config `chord_window_ms` = Some(N) → N 반환
    ///
    /// `bidirectional_combine`은 composer retry 및 chord_compose permutation 게이트이며
    /// chord 타이밍 윈도우(ChordBuffer) 활성화와는 독립적으로 동작한다.
    pub(crate) fn compute_chord_window_ms(config: &Config) -> u16 {
        use crate::keystroke::profile::{resolve_inherits, ProfileRegistry};

        let name = config.engine.korean.effective_layout_name();
        let registry = ProfileRegistry::new();

        // 프로필 찾기 실패 → 0 (안전 폴백)
        let Some(raw) = registry.find_raw(&name) else {
            return 0;
        };
        let profile = match resolve_inherits(&raw, &registry) {
            Ok(p) => p,
            Err(_) => return 0,
        };

        // supports_moachigi=false → chord 강제 OFF (자판 capability 게이트)
        if profile.moachigi.is_none() {
            return 0;
        }

        // chord_window_ms: 사용자 설정만. None = 0 (OFF).
        // bidirectional_combine과 독립 — 두 옵션은 별도 게이트.
        config.engine.korean.chord_window_ms.unwrap_or(0)
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

    /// 한국어 레이아웃의 schema v2 `key_meta`를 char 기반 runtime 맵으로 빌드합니다.
    /// 누락 또는 v1 자판이면 빈 맵.
    pub(super) fn create_key_meta_map(
        korean_layout: &KoreanLayout,
    ) -> HashMap<char, crate::keystroke::profile::KeyMeta> {
        let ko_json = crate::keystroke::get_keymap_json(korean_layout);
        match crate::keystroke::profile::parse_profile_str(ko_json) {
            Ok(profile) => crate::keystroke::profile::build_key_meta_char_map(&profile),
            Err(_) => HashMap::new(),
        }
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

    /// 활성 영문 키맵의 홈 행 9 문자 (이모지 카테고리 단축키 표시용).
    pub fn home_row_labels(&self) -> &str {
        &self.home_row_labels
    }

    /// 활성 영문 키맵의 상단 행 9 문자 (특수문자/이모지 컬럼 헤더용).
    pub fn top_row_labels(&self) -> &str {
        &self.top_row_labels
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
        self.chord_buffer.clear();
    }

    // =========================================================================
    // chord idle flush 공개 API (engine_worker/unim-dbus 접근용)
    // =========================================================================

    /// chord 진행 중 여부 + idle flush 타이머 정보.
    ///
    /// 반환값: `Some((epoch, window_ms))` — chord 버퍼에 자모가 대기 중.
    /// `None` — chord 비활성 또는 버퍼 비어있음.
    pub fn chord_pending_info(&self) -> Option<(u64, u16)> {
        if self.chord_buffer.has_pending() {
            Some((
                self.chord_buffer.current_epoch(),
                self.chord_buffer.window_ms_pub(),
            ))
        } else {
            None
        }
    }

    /// idle flush 타이머 epoch 유효성 검증.
    ///
    /// 타이머 발화 시 epoch 가 현재 chord_buffer epoch 와 일치하고
    /// 버퍼가 비어있지 않으면 `true`.
    pub fn chord_epoch_valid(&self, epoch: u64) -> bool {
        self.chord_buffer.is_idle_epoch_valid(epoch)
    }

    /// chord 강제 종결: FocusOut/Reset 경로에서 호출 (포커스 빠짐 → 음절 강제 commit).
    ///
    /// 반환값: commit 된 텍스트 (`None` 이면 대기 없음 또는 결과 없음).
    /// 내부적으로 `force_flush` + `apply_chord_entries` + `flush_preedit` 를 순서대로 호출.
    pub fn chord_idle_flush_commit(&mut self) -> Option<String> {
        let entries = self.chord_buffer.force_flush()?;
        self.apply_chord_entries(entries);
        self.flush_preedit();
        let text = self.commit_buffer.clone();
        self.commit_buffer.clear();
        if text.is_empty() { None } else { Some(text) }
    }

    /// chord idle 만료 flush: idle timer 경로 전용 (preedit 유지).
    ///
    /// 반환값: `(commit_opt, preedit)` —
    /// - `commit_opt` 는 비자모/composer 가 종결한 음절만 commit (Case C 또는 sequential
    ///   push 도중 syllable 완성 시).
    /// - `preedit` 은 composer 가 들고 있는 진행 중 음절 (풀어쓰기/모아쓰기 결과).
    ///
    /// 사용자 명세: chord_window 만료 = preedit 갱신만, 자모 강제 commit 금지.
    /// 후속 타건이 일반 결합 규칙으로 결합 시도하다 실패하면 그때 commit.
    pub fn chord_idle_flush_pending(&mut self) -> (Option<String>, String) {
        let Some(entries) = self.chord_buffer.force_flush() else {
            return (None, self.preedit_cache.clone());
        };
        self.apply_chord_entries(entries);
        let commit = if self.commit_buffer.is_empty() {
            None
        } else {
            Some(std::mem::take(&mut self.commit_buffer))
        };
        (commit, self.preedit_cache.clone())
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
            self.key_meta_map = Self::create_key_meta_map(&layout);

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
            // chord_buffer: 레이아웃 변경 시 윈도우 재계산 (새 자판이 supports_moachigi=false면 0으로)
            let new_window = Self::compute_chord_window_ms(&snapshot);
            self.chord_buffer.clear();
            self.chord_buffer.set_window_ms(new_window);
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
        self.key_meta_map = Self::create_key_meta_map(&new_layout);

        // v1 builder 경로로 컨텍스트 재구성 (active_rule_sets override 포함)
        self.korean_context = build_korean_context(config, composer_type);
        // chord_buffer: config 갱신 시 윈도우 재계산
        let new_window = Self::compute_chord_window_ms(config);
        self.chord_buffer.clear();
        self.chord_buffer.set_window_ms(new_window);
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

#[cfg(test)]
mod tests {
    use super::*;

    /// chord_window_ms 와 bidirectional_combine 이 독립 게이트임을 검증.
    ///
    /// - chord_window_ms=Some(60), bidirectional_combine=None → 60 반환 (bidir 없어도 chord 동작)
    /// - chord_window_ms=None, bidirectional_combine=Some(true) → 0 반환 (chord_window 없으면 OFF)
    /// - chord_window_ms=Some(80), bidirectional_combine=Some(false) → 80 반환 (bidir과 무관)
    #[test]
    fn compute_chord_window_independent_of_bidirectional() {
        let mut config = Config::default();
        // 모아치기 지원 자판으로 설정
        config.engine.korean.layout = "ko_anmatae".to_string();

        // 케이스 1: chord_window_ms=Some(60), bidirectional_combine=None → 60
        config.engine.korean.chord_window_ms = Some(60);
        config.engine.korean.bidirectional_combine = None;
        assert_eq!(
            InputEngine::compute_chord_window_ms(&config),
            60,
            "chord_window_ms=Some(60), bidir=None should return 60"
        );

        // 케이스 2: chord_window_ms=None, bidirectional_combine=Some(true) → 0
        config.engine.korean.chord_window_ms = None;
        config.engine.korean.bidirectional_combine = Some(true);
        assert_eq!(
            InputEngine::compute_chord_window_ms(&config),
            0,
            "chord_window_ms=None should return 0 regardless of bidir"
        );

        // 케이스 3: chord_window_ms=Some(80), bidirectional_combine=Some(false) → 80
        config.engine.korean.chord_window_ms = Some(80);
        config.engine.korean.bidirectional_combine = Some(false);
        assert_eq!(
            InputEngine::compute_chord_window_ms(&config),
            80,
            "chord_window_ms=Some(80), bidir=Some(false) should return 80 (independent)"
        );
    }
}
