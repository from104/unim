//! `InputEngine` 구조체 정의 + 생성·게터·세터·레이아웃 변경.
//!
//! press 계열 hot path는 `press_key.rs`, 한자/특수문자는 `candidates.rs`,
//! 팝업 디스패치는 `popup_dispatch.rs`, content_purpose/surrounding은 `surrounding.rs`로
//! 분산 `impl InputEngine` 블록으로 구현된다.

use super::build_korean_context;
use super::chord_buffer::ChordBuffer;
use super::types::{AtfHotkey, AtfToggleKind, AutoEnglishTrigger, InputResult, PopupAction};
use crate::config::{CommitUnit, Config, ContentPurpose, EnglishLayout, InputCategory, KoreanLayout};
use crate::hangul::input_context::{ComposerType, HangulInputContext};
use crate::hangul::jamo::JamoEnum;
use crate::keycode::{KeyCode, ModifierState};
use crate::keystroke::EnglishKeymap;
use crate::popup::PopupState;
use once_cell::sync::Lazy;
use std::collections::HashMap;
use std::sync::Mutex;

/// "무효 키 집합이 바뀔 때만 로그" 게이트의 메모 — 필드 라벨별 마지막 서명(B2).
///
/// `parse_switch_keys`/`parse_auto_english_triggers`/`parse_atf_hotkeys` 는 config
/// 적용 시점마다(`InputEngine::new`, `set_switch_keys`, `set_atf_hotkeys`) 다시 불리고,
/// `engine_worker` 의 리로드 루프는 열려 있는 모든 컨텍스트를 순회하며 이들을 재호출한다.
/// 무조건 로깅하면 (컨텍스트 수 × 무효 키 수) 만큼 같은 줄이 쌓이므로(word_gate 선례와
/// 같은 문제), 필드별 무효 집합이 실제로 바뀐 때만 로그를 남긴다. 프로세스 전역 상태이며
/// 로깅 모듈이 아니라 이 파일에 국한한다 — 이 세 파서만 쓰는 전용 게이트.
///
/// 알려진 한계: 개발 로그 파일(`UNIM_DEVELOP`)은 프로세스 × **날짜**로 갈리는데 메모는
/// 프로세스 수명 전체를 덮으므로, 데몬이 자정을 넘기면 새 날짜 파일에는 직전 경고가
/// 없을 수 있다(설정이 다시 바뀌면 재발화). 진단 로그 한정이라 수용한다.
static LOG_STATE_MEMO: Lazy<Mutex<HashMap<&'static str, String>>> =
    Lazy::new(|| Mutex::new(HashMap::new()));

/// `field` 그룹의 무효 키 서명(`signature`)이 직전에 로그로 남긴 것과 **다를 때만** `true`.
///
/// `signature` 가 비면(무효 키 0건) 메모에서 지우고 `false` — 문제가 해소된 뒤 같은
/// 필드에서 다시 무효 키가 생기면 한 번 더 알린다. lock 오염 시에도 로깅을 죽이지
/// 않는다(`into_inner`).
fn log_state_changed(field: &'static str, signature: &str) -> bool {
    let mut memo = LOG_STATE_MEMO.lock().unwrap_or_else(|e| e.into_inner());
    if signature.is_empty() {
        memo.remove(field);
        return false;
    }
    if memo.get(field).map(String::as_str) == Some(signature) {
        return false;
    }
    memo.insert(field, signature.to_string());
    true
}

/// 고정키(Sticky Keys) 래치 잔류 마스킹 대상 수정자 종류.
///
/// 수정자 토글키(RightAlt 등)로 한/영 전환이 성사된 직후, Sticky Keys 가 켜져
/// 있으면 그 토글키 눌림이 해당 수정자 비트를 **다음 한 키에** 래치시킨다. 전환
/// 직후 첫 문자키가 이 잔류 비트 때문에 단축키(예: Alt+키)로 오분류되어 유실될
/// 수 있으므로, 토글 성사 시 토글키가 대표하는 수정자 종류를 기억해 두었다가
/// 다음 키 1건에서 그 비트만 지운다.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum LatchModifier {
    Alt,
    Control,
    Super,
    Shift,
}

impl LatchModifier {
    /// 수정자 KeyCode 가 대표하는 래치 수정자 종류. 비-수정자 키는 `None`.
    pub(super) fn of_keycode(keycode: KeyCode) -> Option<Self> {
        match keycode {
            KeyCode::LeftAlt | KeyCode::RightAlt => Some(Self::Alt),
            KeyCode::LeftControl | KeyCode::RightControl => Some(Self::Control),
            KeyCode::LeftSuper | KeyCode::RightSuper => Some(Self::Super),
            KeyCode::LeftShift | KeyCode::RightShift => Some(Self::Shift),
            _ => None,
        }
    }

    /// `modifier` 에서 이 종류의 비트를 지운다 (고정키 래치 잔류 제거).
    pub(super) fn clear_from(self, modifier: &mut crate::keycode::ModifierState) {
        match self {
            Self::Alt => modifier.alt = false,
            Self::Control => modifier.control = false,
            Self::Super => modifier.super_key = false,
            Self::Shift => modifier.shift = false,
        }
    }
}

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
    /// AutoTypeFix 전체(`enabled`) 토글 단축키 (설정 기반 — 기본 `Shift+F8`).
    pub(super) atf_hotkeys_enabled: Vec<AtfHotkey>,
    /// AutoTypeFix 순방향(영→한) 교정 토글 단축키 (설정 기반, 옵트인 — 기본 빈 목록).
    pub(super) atf_hotkeys_forward: Vec<AtfHotkey>,
    /// AutoTypeFix 역방향(한→영) 교정 토글 단축키 (설정 기반, 옵트인 — 기본 빈 목록).
    pub(super) atf_hotkeys_reverse: Vec<AtfHotkey>,
    /// `press_key` 에서 매칭된 ATF 토글을 호스트가 드레인하기 전까지 보관.
    ///
    /// `InputResult`(`repr(C)` ABI)를 건드리지 않는 out-of-band 채널
    /// (`popup_pending_action` 선례). 호스트가 `take_atf_toggle()` 로 소비한다.
    pub(super) pending_atf_toggle: Option<AtfToggleKind>,
    /// 마지막으로 매칭된 ATF 핫키(키코드, 시각) — 오토리핏 디바운스용.
    ///
    /// 키 홀드 시 프런트가 자동반복 키다운을 흘려보내면 press_key 가 연속 호출돼
    /// 토글이 수십 번 뒤집힌다. 동일 키코드가 `ATF_HOTKEY_DEBOUNCE` 이내에 다시
    /// 매칭되면 **소비는 유지하되(홀드 중 키가 앱으로 새면 안 됨) 토글은 생략**하고,
    /// 시각을 갱신해 홀드가 지속되는 내내 rolling window 로 차단한다. 다른 키코드가
    /// 오거나 창을 넘겨 눌러야 다음 토글이 성립한다. `None` = 직전 매칭 없음.
    pub(super) last_atf_hotkey: Option<(KeyCode, std::time::Instant)>,
    /// 자동 영문 전환 활성화 여부 (설정 캐시)
    pub(super) auto_english_enabled: bool,
    /// 자동 영문 전환 트리거 (파싱된 카테고리별 캐시)
    ///
    /// 두 카테고리:
    /// - `Functional { code, shift, ctrl, alt, super_key }`: KeyCode + modifier
    ///   비교 (Escape/Tab/F*/Shift 명시 문자, 그리고 Ctrl/Alt/Super 조합).
    /// - `Character(ch)`: keymap 산출 char 비교 (비-QWERTY 한국어 레이아웃 안전).
    ///
    /// 표기 문법: `key:Escape` / `char:/` / `key:Ctrl+B` (접두사). `key:` 뒤에는
    /// `(ctrl|alt|super|shift +)* <KeyName>` 조합을 쓸 수 있다(대소문자·순서 무관,
    /// base KeyName 은 대소문자 구분). 무접두사는 legacy 호환으로 `Functional`
    /// (modifier 전부 false)로 흡수한다.
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
    /// 조합 확정 단위 (음절/단어/스마트) — `config.engine.korean.commit_unit` 캐시.
    ///
    /// `Word` 면 `korean_context.set_accumulate_word(true)` 로 단어 누적을 켠다.
    /// `set_word_mode` 토글·`rebuild`/`set_layout` 재구성 경로에서 일관되게 반영된다.
    pub(super) commit_unit: CommitUnit,
    /// 단어 모드 대상 앱 목록(정확일치) — `config.engine.korean.word_mode_apps` 캐시.
    ///
    /// `Smart` 게이트가 `is_word_mode_app` 로 조회한다(Windows TSF·Linux 데몬 공통).
    /// `new`/`rebuild_korean_context` 재구성 경로에서 config 와 동기화된다. 기본값은
    /// 플랫폼 분기 — Windows=`["winword.exe"]`, 리눅스=빈 목록(wmux 는 양 플랫폼 모두
    /// 역방향 음절모드 synth 라우팅 동작 확인 후 제외).
    pub(super) word_mode_apps: Vec<String>,
    /// 비밀번호/PIN 필드 진입 시 저장한 직전 입력 카테고리 (자동 복구용).
    ///
    /// `set_content_purpose` 상태머신이 password/PIN 진입 시 현재 카테고리를
    /// 저장하고 영문으로 강제 전환하며, 필드를 벗어나면(비-비밀번호 목적) 저장값을
    /// 복구한 뒤 클리어한다. `None` = 저장 없음(평소 경로 — 바이트 동일 무회귀).
    /// 멱등(이중 저장/복구 방지).
    pub(super) saved_category: Option<InputCategory>,
    /// 고정키(Sticky Keys) 래치 잔류 무시 — 수정자 토글키(RightAlt 등)로 한/영
    /// 전환이 성사된 직후 **다음 키 1건**에서 마스킹할 수정자 종류.
    ///
    /// 지체장애 사용자가 고정키를 쓰면 RightAlt 눌림이 Alt 비트를 다음 키에
    /// 래치시켜, 전환 직후 첫 문자키가 단축키(Alt+키)로 오분류돼 유실될 수 있다.
    /// 토글 성사 시(그리고 토글키가 수정자일 때) 그 수정자 종류를 저장하고, 다음
    /// `press_key` 진입에서 해당 비트를 지운 뒤 소비(`take`)한다. RightAlt 토글
    /// 자체는 그대로 유지된다. `None` = 마스킹 없음(평소 경로 — 바이트 동일 무회귀).
    pub(super) sticky_toggle_mask: Option<LatchModifier>,
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

        let commit_unit = config.engine.korean.commit_unit;
        let word_mode_apps = config.engine.korean.word_mode_apps.clone();

        let mut engine = Self {
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
            toggle_keys: Self::parse_switch_keys(&config.engine.toggle_keys, "한/영 전환키"),
            hanja_keys: Self::parse_switch_keys(&config.engine.hanja_keys, "한자 키"),
            atf_hotkeys_enabled: Self::parse_atf_hotkeys(
                &config.engine.auto_typefix.toggle_enabled_keys,
                "ATF 토글 단축키(전체)",
            ),
            atf_hotkeys_forward: Self::parse_atf_hotkeys(
                &config.engine.auto_typefix.toggle_forward_keys,
                "ATF 토글 단축키(정방향)",
            ),
            atf_hotkeys_reverse: Self::parse_atf_hotkeys(
                &config.engine.auto_typefix.toggle_reverse_keys,
                "ATF 토글 단축키(역방향)",
            ),
            pending_atf_toggle: None,
            last_atf_hotkey: None,
            auto_english_enabled: config.engine.auto_english.enabled,
            auto_english_triggers: Self::parse_auto_english_triggers(
                &config.engine.auto_english.trigger_keys,
            ),
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
            commit_unit,
            word_mode_apps,
            saved_category: None,
            sticky_toggle_mask: None,
        };

        // Word 모드면 단어 누적을 초기 주입 (Smart/Syllable = 누적 off, 기존 동작).
        if commit_unit == CommitUnit::Word {
            engine.korean_context.set_accumulate_word(true);
        }

        engine
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

    /// ATF 핫키 표기 목록을 `AtfHotkey` 벡터로 파싱한다 (파싱 실패분은 경고 로그 후 제외).
    ///
    /// ATF 토글 핫키 3목록 파싱을 생성자와 `set_atf_hotkeys` 재적용에서 공유한다.
    /// 표기 문법(`[수정자+]* <KeyName>`)과 정확-일치 규약, 수정자 키 base 배제 사유는
    /// `InputEngine::parse_atf_hotkey` rustdoc 참조. config 적용 시점에만 불리는
    /// cold path 라 실패 표기를 데몬 로그에 남긴다 — 오타로 핫키가 조용히 죽는 것을
    /// 사용자가 로그로 확인할 수 있게 한다.
    ///
    /// `field` 는 로그용 라벨이자 `log_state_changed`(B2) 의 dedupe 메모 키를 겸한다 —
    /// ATF 3목록(전체/정방향/역방향)은 서로 다른 라벨을 써야 한다(메모 키가 같으면
    /// 서로의 무효 집합 변화를 가려 경고가 누락된다). 리로드 루프는 열려 있는 모든
    /// 컨텍스트를 순회하므로, 무효 집합이 직전과 동일하면 로그를 생략해 (컨텍스트 수 ×
    /// 무효 키 수) 스팸을 필드당 1줄로 줄인다 — `engine_worker` word_gate flip 로깅과
    /// 같은 결.
    pub(super) fn parse_atf_hotkeys(names: &[String], field: &'static str) -> Vec<AtfHotkey> {
        let mut invalid: Vec<&str> = Vec::new();
        let parsed: Vec<AtfHotkey> = names
            .iter()
            .filter_map(|name| {
                let p = Self::parse_atf_hotkey(name);
                if p.is_none() {
                    invalid.push(name.as_str());
                }
                p
            })
            .collect();
        let joined = invalid.join(", ");
        if log_state_changed(field, &joined) {
            crate::unim_log!(
                "ENGINE",
                "{} 파싱 실패 — 무시됨: {} (문법: [Ctrl+][Alt+][Super+][Shift+]키이름, 예: Shift+F8)",
                field,
                joined
            );
        }
        parsed
    }

    /// 한/영 전환키·한자키 이름 목록을 `KeyCode` 벡터로 파싱한다 (실패분은 경고 로그 후 제외).
    ///
    /// 한/영 전환키·한자키 목록 파싱을 생성자와 `set_switch_keys` 재적용에서 공유한다.
    /// 표기 규약(조합 표기 미지원, 수정자 키 자체 허용)은 `InputEngine::parse_switch_key`
    /// rustdoc 참조. config 적용 시점에만 불리는 cold path 라 `parse_atf_hotkeys` 와 같은
    /// 형식으로 실패 표기를 데몬 로그에 남긴다 — 오타로 키가 조용히 죽는 것을 사용자가
    /// 로그로 확인할 수 있게 한다.
    ///
    /// `field` 는 로그용 필드 이름(`"한/영 전환키"` / `"한자 키"`)이자 `log_state_changed`
    /// (B2) 의 dedupe 메모 키를 겸한다 — 리로드는 열려 있는 모든 컨텍스트를 순회하고
    /// 설정앱은 LineEdit 편집마다 저장하므로, 무효 집합이 직전과 동일하면 로그를 생략해
    /// (컨텍스트 수 × 무효 키 수) 스팸을 필드당 1줄로 줄인다.
    pub(super) fn parse_switch_keys(names: &[String], field: &'static str) -> Vec<KeyCode> {
        let mut invalid: Vec<&str> = Vec::new();
        let parsed: Vec<KeyCode> = names
            .iter()
            .filter_map(|name| {
                let p = Self::parse_switch_key(name);
                if p.is_none() {
                    invalid.push(name.as_str());
                }
                p
            })
            .collect();
        let joined = invalid.join(", ");
        if log_state_changed(field, &joined) {
            crate::unim_log!(
                "ENGINE",
                "{} 파싱 실패 — 무시됨: {} (조합 표기 미지원, 키 이름 정확 일치: 예 Korean, RightAlt, Hanja, F9)",
                field,
                joined
            );
        }
        parsed
    }

    /// 자동 영문 전환 트리거 표기 목록을 파싱한다 (실패분은 경고 로그 후 제외).
    ///
    /// 문법(`key:` / `char:` / legacy 무접두사)은 `InputEngine::parse_trigger_key`
    /// rustdoc 참조. `parse_atf_hotkeys`·`parse_switch_keys` 와 같은 cold path 규약으로
    /// 실패 표기를 데몬 로그에 남긴다. 목록이 하나뿐이라 dedupe 메모 키(B2)는 고정
    /// 리터럴 `"자동 영문 트리거"`.
    pub(super) fn parse_auto_english_triggers(names: &[String]) -> Vec<AutoEnglishTrigger> {
        const FIELD: &str = "자동 영문 트리거";
        let mut invalid: Vec<&str> = Vec::new();
        let parsed: Vec<AutoEnglishTrigger> = names
            .iter()
            .filter_map(|name| {
                let p = Self::parse_trigger_key(name);
                if p.is_none() {
                    invalid.push(name.as_str());
                }
                p
            })
            .collect();
        let joined = invalid.join(", ");
        if log_state_changed(FIELD, &joined) {
            crate::unim_log!(
                "ENGINE",
                "{} 파싱 실패 — 무시됨: {} (문법: key:<키> / char:<문자>, 예: key:Escape, char:/, key:Ctrl+B)",
                FIELD,
                joined
            );
        }
        parsed
    }
}

// ==========================================
// 입력 카테고리 / commit·preedit / 레이아웃 변경
// ==========================================

impl InputEngine {
    pub fn input_category(&self) -> InputCategory {
        self.input_category
    }

    /// 주어진 키코드가 설정된 한/영 전환키인지 여부.
    ///
    /// 프런트엔드의 소비 판정(TSF `test_key_down`, IMM32 `should_consume`)이
    /// 엔진의 `press_key` 와 동일하게 토글키(RightAlt 같은 수정자 토글키 포함)를
    /// 소비하도록 노출한다. 프런트엔드가 토글키를 소비하지 않으면 `press_key`
    /// 자체가 호출되지 않아 토글이 죽는다.
    pub fn is_toggle_key(&self, keycode: KeyCode) -> bool {
        self.toggle_keys.contains(&keycode)
    }

    /// 주어진 키코드+수정자 조합이 설정된 AutoTypeFix 토글 단축키(전체/순방향/역방향
    /// 중 하나)와 **정확 일치**하는지.
    ///
    /// `is_toggle_key` 와 동일한 목적 — 프런트엔드의 소비 판정(TSF `OnTestKeyDown`,
    /// IMM32 `should_consume`)을 엔진의 `press_key` 와 정렬시킨다. 프런트엔드가 핫키를
    /// 소비하지 않으면 `press_key` 자체가 호출되지 않아 핫키가 죽는다.
    ///
    /// 판정은 `atf_hotkey_kind` 와 동일한 **수정자 정확 일치**다(`AtfHotkey` 규약):
    /// 조합 표기(`Shift+F8`)는 수정자가 함께 눌린 때만, bare 표기(`F10`)는 수정자가
    /// 전부 떼어진 때만 참이다. 그래서 조합 핫키의 맨 base 키(예 맨 `F8`, 한자키 맨
    /// `F9`)는 소비되지 않아 원래 기능이 보존되고, 반대로 조합 자체는 test 단계에서
    /// 소비돼 press_key 까지 도달한다 — **Linux(GNOME divert·DBus 경로)와 Windows
    /// TSF/IMM32 가 동일하게 동작한다.** 호출부는 press/test 시점의 실제 수정자
    /// 상태(TSF `get_modifier_state()`, IMM32 `lpbKeyState`)를 넘겨야 한다.
    pub fn is_atf_hotkey(&self, keycode: KeyCode, modifier: ModifierState) -> bool {
        self.atf_hotkey_kind(keycode, modifier).is_some()
    }

    /// 자동 영문 전환 **조합** 트리거(`key:Ctrl+B` 등)의 판정 + 축소 반응.
    ///
    /// TSF `OnTestKeyDown` 처럼 실 처리(`press_key`) 전에, 그것도 Word 류 호스트에선
    /// **같은 키에 대해 투기적으로 여러 번** 불릴 수 있는 경로용 헬퍼다. `press_key()`
    /// 의 단축키 가드(조합 트리거 매칭 시 `flush_preedit()` 로 preedit 를
    /// `commit_buffer` 에 밀어 넣고 `InputResult::committed_passthrough` 를 반환하는
    /// 실 경로)와 달리, 이 헬퍼는 **입력 카테고리 전환만** 하고 `commit_buffer`/preedit
    /// 에는 손대지 않는다 — 투기적으로 쌓인 `commit_buffer` 를 draining 하는 호출자가
    /// 없으므로, 여기서 `flush_preedit()`(≒ `set_input_category`)를 그대로 타면 조합
    /// 중이던 텍스트가 아무도 못 읽는 채 유실된다.
    ///
    /// 멱등: 이미 목표 카테고리(English)면 판정만 하고 아무 것도 쓰지 않으므로, 동일
    /// 입력으로 연속 호출해도 두 번째 호출부터는 완전한 no-op 이다(필드 쓰기 0회).
    /// 실제 preedit flush/commit 텍스트 반영은 `press_key()` 실 경로가 담당한다 —
    /// 이 헬퍼가 이미 카테고리를 옮겨 놨더라도 `match_auto_english_trigger` 가 Korean
    /// 전용이라 재매칭하지 않고, 대신 기존 "조합 중 단축키는 먼저 커밋" 일반 경로로
    /// 자연히 흡수된다.
    ///
    /// Ctrl/Alt/Super 중 하나도 안 눌렸으면(조합이 아니면) 즉시 `false`.
    pub fn try_auto_english_combo(&mut self, keycode: KeyCode, modifier: ModifierState) -> bool {
        if !(modifier.control || modifier.alt || modifier.super_key) {
            return false;
        }
        let matched = self.match_auto_english_trigger(keycode, modifier).is_some();
        if matched && self.input_category != InputCategory::English {
            self.input_category = InputCategory::English;
            self.update_status_file();
        }
        matched
    }

    /// ATF 토글 핫키 매칭 — 매칭된 대상 플래그를 돌려준다(비매칭 시 `None`).
    ///
    /// keycode 와 **네 수정자 전부**를 정확 일치로 비교한다(`AtfHotkey` 규약): 표기에
    /// 등장한 수정자는 눌려 있어야 하고, 등장하지 않은 수정자는 눌리면 안 된다.
    /// 덕분에 설정에 없는 조합은 매칭 실패로 통과해 앱 단축키가 보호된다.
    ///
    /// 한 조합이 여러 목록에 중복 지정되면 전체(`Enabled`) → 순방향 → 역방향 순으로
    /// 우선한다.
    pub(super) fn atf_hotkey_kind(
        &self,
        keycode: KeyCode,
        modifier: ModifierState,
    ) -> Option<AtfToggleKind> {
        let matches = |list: &[AtfHotkey]| {
            list.iter().any(|h| {
                h.code == keycode
                    && h.ctrl == modifier.control
                    && h.alt == modifier.alt
                    && h.super_key == modifier.super_key
                    && h.shift == modifier.shift
            })
        };

        if matches(&self.atf_hotkeys_enabled) {
            Some(AtfToggleKind::Enabled)
        } else if matches(&self.atf_hotkeys_forward) {
            Some(AtfToggleKind::Forward)
        } else if matches(&self.atf_hotkeys_reverse) {
            Some(AtfToggleKind::Reverse)
        } else {
            None
        }
    }

    /// `press_key` 에서 매칭된 ATF 토글을 드레인한다(1회성 — `take`).
    ///
    /// 호스트(Linux `engine_worker` / Windows `text_service`)가 `press_key` 직후
    /// 호출해 `config.engine.auto_typefix.{enabled,forward,reverse}` 반전·persist·통지를
    /// 수행한다. `InputResult`(`repr(C)` ABI)를 건드리지 않는 out-of-band 드레인 채널
    /// (`popup_pending_action` 선례). 대기 중 토글이 없으면 `None`.
    pub fn take_atf_toggle(&mut self) -> Option<AtfToggleKind> {
        self.pending_atf_toggle.take()
    }

    /// ATF 토글 핫키 3목록을 config 에서 재파싱해 반영한다 (hot-reload 재적용).
    ///
    /// GUI/CLI 로 핫키를 편집한 뒤 `engine_worker` 의 reload 루프가 살아있는 컨텍스트에
    /// 즉시 반영하도록 호출한다(toggle_keys/hanja_keys 의 live-reload 갭과 달리 ATF
    /// 핫키는 재적용을 보장한다). 파싱 실패(`Unknown`)한 이름은 제외한다.
    pub fn set_atf_hotkeys(&mut self, config: &Config) {
        self.atf_hotkeys_enabled = Self::parse_atf_hotkeys(
            &config.engine.auto_typefix.toggle_enabled_keys,
            "ATF 토글 단축키(전체)",
        );
        self.atf_hotkeys_forward = Self::parse_atf_hotkeys(
            &config.engine.auto_typefix.toggle_forward_keys,
            "ATF 토글 단축키(정방향)",
        );
        self.atf_hotkeys_reverse = Self::parse_atf_hotkeys(
            &config.engine.auto_typefix.toggle_reverse_keys,
            "ATF 토글 단축키(역방향)",
        );
    }

    /// 한/영 전환키(`toggle_keys`)·한자키(`hanja_keys`) 캐시를 config 에서 재파싱해
    /// 반영한다 (hot-reload 재적용).
    ///
    /// GUI/CLI 로 전환·한자 키를 편집한 뒤 `engine_worker` 의 reload 루프가 살아있는
    /// 컨텍스트에 재로그인 없이 즉시 반영하도록 호출한다(종전의 live-reload 갭 해소 —
    /// `set_atf_hotkeys` 와 동일 패턴, `::new` 의 파싱 로직 재사용). 파싱 실패
    /// (`Unknown`)한 이름은 제외한다.
    pub fn set_switch_keys(&mut self, config: &Config) {
        self.toggle_keys = Self::parse_switch_keys(&config.engine.toggle_keys, "한/영 전환키");
        self.hanja_keys = Self::parse_switch_keys(&config.engine.hanja_keys, "한자 키");
    }

    /// 고정키(Sticky Keys) 래치 잔류 마스킹 **미리보기(비소비)**.
    ///
    /// 수정자 토글키(RightAlt 등)로 전환이 성사된 직후 예약된 마스크가 있으면
    /// 주어진 `modifier` 에서 그 수정자 비트를 지운 복사본을 돌려준다(원본 상태
    /// 불변 — `take` 하지 않음). 프런트엔드 소비 판정(TSF `OnTestKeyDown`)이
    /// 전환 직후 첫 키의 래치 잔류 비트를 무시하고 그 키를 IME 로 소비하도록
    /// 게이트를 `press_key` 와 정렬시키는 용도다. 실제 비트 제거·마스크 소비는
    /// `press_key`(OnKeyDown 경로)가 한 번만 수행한다. 마스크가 없으면(평소 경로)
    /// 입력 그대로 반환 — 바이트 동일 무회귀.
    pub fn peek_sticky_masked_modifiers(
        &self,
        modifier: crate::keycode::ModifierState,
    ) -> crate::keycode::ModifierState {
        let mut m = modifier;
        if let Some(latch) = self.sticky_toggle_mask {
            latch.clear_from(&mut m);
        }
        m
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

    /// 단어별 preedit 모드(단어 단위 누적)를 켜거나 끕니다.
    ///
    /// 켜기/끄기 전에 진행 중 조합을 `flush_preedit` 으로 비운 뒤
    /// `korean_context.set_accumulate_word(on)` 를 호출한다. 이 setter 는
    /// `commit_unit` 캐시 자체를 바꾸지는 않으며(런타임 토글), config 동기화는
    /// `rebuild_korean_context` 경로가 담당한다.
    pub fn set_word_mode(&mut self, on: bool) {
        self.flush_preedit();
        self.korean_context.set_accumulate_word(on);
    }

    /// 단어별 preedit 모드(단어 누적)가 현재 켜져 있는지 반환합니다.
    ///
    /// `commit_unit` 캐시가 아니라 `korean_context` 의 실제 누적 상태(런타임 권위)를
    /// 반환한다 — `set_word_mode` 런타임 토글이 반영된다.
    pub fn is_word_mode(&self) -> bool {
        self.korean_context.is_accumulate_word()
    }

    /// 현재 조합 확정 단위(`CommitUnit`)를 반환합니다.
    ///
    /// `config.engine.korean.commit_unit` 의 캐시값으로 `new`/`rebuild_korean_context`
    /// /`set_korean_layout` 재구성 경로에서 config 와 동기화된다. 런타임 누적 상태를
    /// 반환하는 `is_word_mode` 와 달리 **설정상의 의도**를 나타낸다 — TSF 게이트가
    /// 앱별 단어모드 desired 판정(Syllable→off / Word→on / Smart→앱판정)에 사용한다.
    pub fn commit_unit(&self) -> CommitUnit {
        self.commit_unit
    }

    /// 프로세스 실행 파일명이 단어 모드 대상 앱 목록(정확일치)에 속하는지 판정한다.
    ///
    /// `config.engine.korean.word_mode_apps` 캐시를 정확일치(`==`)로 조회한다. 기본값은
    /// 플랫폼 분기 — Windows=`["winword.exe"]`, 리눅스=빈 목록(wmux 는 양 플랫폼 모두
    /// 역방향 음절모드 synth 라우팅 동작 확인 후 제외) — 대소문자 구분·부분일치 없음. `Smart` 확정 단위
    /// 게이트(OnKeyDown 매-키 재적용 + OnSetFocus)가 앱별 단어 모드 desired 판정에 쓴다.
    /// 앱 호환 목록이 코드 릴리스와 분리되어 config.yaml/CLI 로 확장 가능하다.
    pub fn is_word_mode_app(&self, name: &str) -> bool {
        self.word_mode_apps.iter().any(|app| app == name)
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
    ///
    /// 조합(`korean_context`)·commit/preedit 버퍼·chord 버퍼에 더해 팝업(한자·특수문자
    /// ·이모지) 라우팅 상태까지 초기화한다. 리셋 직후 이어지는 조합(예: AutoTypeFix
    /// replay)이 `press_key` 의 팝업 인터셉트 분기(`hanja_mode || special_char_mode ||
    /// is_emoji_popup_active`)로 오라우팅되지 않게 하기 위함이며, `InputEngine::new`
    /// 가 기본값으로 두는 런타임 조합/팝업 상태를 **사전 재로딩 없이**(6.76MB 한자사전
    /// 재파싱 + 북마크 디스크IO 회피) 동일하게 비운다.
    ///
    /// 보존(초기화하지 않음):
    /// - config 파생 캐시(키맵·`key_meta`·영문 키맵·한자사전 Arc·레이아웃·toggle/hanja
    ///   키·auto_english·라벨) — 재생성 없이 그대로 유효(동일 config 기준).
    /// - `input_category` — 호출부가 필요 시 별도 `set_input_category` 로 지정.
    /// - `content_purpose`/`surrounding_*`/`saved_category` — 앱·포커스 컨텍스트(조합
    ///   상태 아님). 리셋이 필드 목적을 잊으면 안 되므로 보존.
    /// - `accumulate_word`(단어 누적 플래그) — `korean_context.clear()` 가 버퍼만 비우고
    ///   플래그는 유지. config 기반 재동기화가 필요한 호출부는 리셋 후 `set_word_mode`
    ///   로 명시 지정한다(예: `auto_typefix`).
    pub fn reset(&mut self) {
        self.korean_context.clear();
        self.commit_buffer.clear();
        self.preedit_cache.clear();
        self.chord_buffer.clear();

        // 팝업/한자/특수문자 모드 라우팅 상태 초기화 (new() 기본값과 동일).
        // 이들이 남아 있으면 press_key 가 조합 대신 팝업 디스패치로 흘러간다.
        self.hanja_mode = false;
        self.hanja_candidates.clear();
        self.hanja_target.clear();
        self.special_char_mode = false;
        self.special_char_candidates.clear();
        self.special_char_target.clear();
        self.popup_state = None;
        self.popup_pending_action = None;

        // 고정키 래치 마스킹 잔류도 비운다 — 조합 리셋 시점의 미소비 마스크가
        // 이후 무관한 첫 키의 수정자를 잘못 지우지 않게 한다.
        self.sticky_toggle_mask = None;
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
    ///
    /// preview 가 이미 inject 된 상태(Case A 진행 중) 면 `apply_chord_entries`
    /// 를 다시 호출하지 않고 preedit 만 `flush_preedit` 으로 commit. preview 가
    /// 없는 경우(Case B/C 또는 1키 풀어쓰기) 는 종전대로 entries 를 적용.
    pub fn chord_idle_flush_commit(&mut self) -> Option<String> {
        let preview_was_injected = self.chord_buffer.was_preview_injected();
        let entries = self.chord_buffer.force_flush()?;
        if !preview_was_injected {
            self.apply_chord_entries(entries);
        }
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
    ///
    /// preview 가 이미 inject 된 상태이면 buffer 만 비우고 preedit 은 그대로 유지
    /// (apply_chord_entries 재호출 시 동일 음절이 commit 으로 흘러가는 중복을 차단).
    pub fn chord_idle_flush_pending(&mut self) -> (Option<String>, String) {
        let preview_was_injected = self.chord_buffer.was_preview_injected();
        let Some(entries) = self.chord_buffer.force_flush() else {
            return (None, self.preedit_cache.clone());
        };
        if !preview_was_injected {
            self.apply_chord_entries(entries);
        } else {
            // preview 가 이미 preedit 을 들고 있음. 캐시만 동기화.
            self.update_preedit_cache();
        }
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
            // 새 컨텍스트는 accumulate_word 가 off 로 시작 → commit_unit 캐시와 일관성 유지.
            if self.commit_unit == CommitUnit::Word {
                self.korean_context.set_accumulate_word(true);
            }
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

        // commit_unit 도 config 에서 재동기화 (hot-reload 경로).
        self.commit_unit = config.engine.korean.commit_unit;
        // 단어 모드 앱 목록도 재동기화 (config.yaml/CLI 편집 → maybe_reload_config 반영).
        self.word_mode_apps = config.engine.korean.word_mode_apps.clone();

        // v1 builder 경로로 컨텍스트 재구성 (active_rule_sets override 포함)
        self.korean_context = build_korean_context(config, composer_type);
        // 새 컨텍스트는 accumulate_word 가 off 로 시작 → commit_unit 캐시와 일관성 유지.
        if self.commit_unit == CommitUnit::Word {
            self.korean_context.set_accumulate_word(true);
        }
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

    // B2: 무효 키 로그 dedupe 게이트(`log_state_changed`) 단위 테스트.
    // 정적 메모(`LOG_STATE_MEMO`)를 프로세스 전체 테스트가 공유하므로, 병렬 실행 간
    // 간섭을 피하려면 테스트마다 고유 필드 키를 쓴다.

    #[test]
    fn log_state_changed_suppresses_identical_signature() {
        assert!(log_state_changed("test-b2-a", "X"));
        assert!(!log_state_changed("test-b2-a", "X"));
        assert!(log_state_changed("test-b2-a", "X, Y"));
    }

    #[test]
    fn log_state_changed_clears_on_empty() {
        assert!(log_state_changed("test-b2-b", "X"));
        assert!(!log_state_changed("test-b2-b", ""));
        // 문제가 해소된 뒤 재발하면 다시 알려야 한다.
        assert!(log_state_changed("test-b2-b", "X"));
    }

    // ─────────────────────────────────────────────
    // 단어별 preedit (commit_unit / set_word_mode) — Phase 2 코어 배선
    // ─────────────────────────────────────────────

    use crate::keycode::ModifierState;

    /// dubeolsik 기본 자판에서 "가나"를 입력하는 키 시퀀스(ㄱ ㅏ ㄴ ㅏ).
    fn type_ganan(engine: &mut InputEngine, config: &Config) {
        let m = ModifierState::default();
        engine.press_key(KeyCode::R, m, config); // ㄱ
        engine.press_key(KeyCode::K, m, config); // 가
        engine.press_key(KeyCode::S, m, config); // ㄴ (도깨비불: 가 → 누적/commit)
        engine.press_key(KeyCode::K, m, config); // 나
    }

    /// commit_unit=Syllable 은 음절 단위 — preedit 은 현재 음절만, 직전 음절은 commit.
    /// 무회귀 검증. (기본값은 Smart 로 바뀌었으나 Syllable 경로 자체는 불변.)
    #[test]
    fn syllable_mode_no_word_accumulation() {
        let mut config = Config::default();
        config.engine.korean.commit_unit = CommitUnit::Syllable;
        assert_eq!(config.engine.korean.commit_unit, CommitUnit::Syllable);
        let mut engine = InputEngine::new(&config);
        engine.set_input_category(InputCategory::Korean);
        type_ganan(&mut engine, &config);
        assert_eq!(engine.preedit_str(), "나", "syllable 모드 preedit 은 현재 음절만");
        assert!(
            engine.commit_str().contains('가'),
            "직전 음절 '가'는 commit 으로 흘러야 함: {}",
            engine.commit_str()
        );
    }

    /// commit_unit=Word 로 생성하면 new() 가 accumulate_word 를 초기 주입 →
    /// set_word_mode 호출 없이도 preedit 이 단어 전체로 누적된다.
    #[test]
    fn new_with_word_commit_unit_accumulates() {
        let mut config = Config::default();
        config.engine.korean.commit_unit = CommitUnit::Word;
        let mut engine = InputEngine::new(&config);
        engine.set_input_category(InputCategory::Korean);
        type_ganan(&mut engine, &config);
        assert_eq!(
            engine.preedit_str(),
            "가나",
            "Word 모드 preedit 은 누적된 단어 전체"
        );
    }

    /// Smart/Syllable 은 누적 off (new 초기 주입 안 함).
    #[test]
    fn new_with_smart_commit_unit_does_not_accumulate() {
        let mut config = Config::default();
        config.engine.korean.commit_unit = CommitUnit::Smart;
        let mut engine = InputEngine::new(&config);
        engine.set_input_category(InputCategory::Korean);
        type_ganan(&mut engine, &config);
        assert_eq!(engine.preedit_str(), "나", "Smart 는 Phase2 에서 누적 off");
    }

    /// is_word_mode_app 기본값은 플랫폼 분기 — Windows=winword.exe(정확일치), 리눅스=빈
    /// 목록(어떤 앱도 불일치). wmux 는 양 플랫폼 모두 역방향 음절모드 synth 라우팅 동작
    /// 확인 후 제외됨 → 기본 syllable. 대소문자 구분·부분일치 없음.
    #[test]
    fn is_word_mode_app_default_matches_winword_only() {
        let engine = InputEngine::new(&Config::default());
        #[cfg(target_os = "windows")]
        assert!(engine.is_word_mode_app("winword.exe"));
        // 리눅스 기본값은 빈 목록 → winword.exe 도 불일치.
        #[cfg(not(target_os = "windows"))]
        assert!(!engine.is_word_mode_app("winword.exe"));
        // wmux 는 기본 화이트리스트에서 제외(역방향 syllable synth 라우팅으로 동작).
        assert!(!engine.is_word_mode_app("wmux.exe"));
        // 정확일치 — 대문자/부분일치/비대상 앱은 불일치(무회귀).
        assert!(!engine.is_word_mode_app("WINWORD.EXE"));
        assert!(!engine.is_word_mode_app("winword"));
        assert!(!engine.is_word_mode_app("chrome.exe"));
        assert!(!engine.is_word_mode_app("wezterm-gui.exe"));
        assert!(!engine.is_word_mode_app(""));
    }

    /// config 목록을 확장하면 새 앱도 단어 모드 대상이 되고, 목록에서 빠지면 제외된다.
    #[test]
    fn is_word_mode_app_respects_config_override() {
        let mut config = Config::default();
        config.engine.korean.word_mode_apps =
            vec!["kakaotalk.exe".to_string(), "winword.exe".to_string()];
        let engine = InputEngine::new(&config);
        assert!(engine.is_word_mode_app("kakaotalk.exe"), "확장된 앱은 대상");
        assert!(engine.is_word_mode_app("winword.exe"));
        assert!(
            !engine.is_word_mode_app("wmux.exe"),
            "목록에서 빠진 앱은 제외"
        );
    }

    /// 빈 목록이면 어떤 앱도 단어 모드 대상이 아니다(Smart 게이트 항상 syllable).
    #[test]
    fn is_word_mode_app_empty_list_matches_nothing() {
        let mut config = Config::default();
        config.engine.korean.word_mode_apps = Vec::new();
        let engine = InputEngine::new(&config);
        assert!(!engine.is_word_mode_app("winword.exe"));
        assert!(!engine.is_word_mode_app("wmux.exe"));
    }

    /// set_word_mode(true) 가 런타임에 단어 누적을 켠다.
    #[test]
    fn set_word_mode_on_enables_accumulation() {
        let config = Config::default();
        let mut engine = InputEngine::new(&config);
        engine.set_input_category(InputCategory::Korean);
        engine.set_word_mode(true);
        type_ganan(&mut engine, &config);
        assert_eq!(engine.preedit_str(), "가나");
    }

    /// is_word_mode() 는 commit_unit 캐시가 아니라 런타임 누적 상태를 반환한다
    /// (Phase 3a Word 게이트가 매 포커스 토글하는 권위 = korean_context.accumulate_word).
    #[test]
    fn is_word_mode_reflects_runtime_toggle() {
        let config = Config::default(); // 기본 Smart — new() 가 누적을 초기 주입하지 않음
        let mut engine = InputEngine::new(&config);
        assert!(!engine.is_word_mode(), "기본(Smart)은 누적 off → 단어 모드 off");
        engine.set_word_mode(true);
        assert!(engine.is_word_mode(), "set_word_mode(true) 후 on");
        engine.set_word_mode(false);
        assert!(!engine.is_word_mode(), "set_word_mode(false) 후 off");
    }

    /// set_word_mode(false) 는 끄기 전 진행 중 조합을 flush 한다 (잔여 단어 손실 없음).
    #[test]
    fn set_word_mode_off_flushes_accumulated_word() {
        let config = Config::default();
        let mut engine = InputEngine::new(&config);
        engine.set_input_category(InputCategory::Korean);
        engine.set_word_mode(true);
        type_ganan(&mut engine, &config);
        engine.clear_commit();
        engine.set_word_mode(false);
        assert_eq!(
            engine.commit_str(),
            "가나",
            "끄기 전 누적 단어가 commit 으로 flush 되어야 함"
        );
        assert!(engine.preedit_str().is_empty(), "flush 후 preedit 비어야 함");
    }

    /// rebuild_korean_context 가 commit_unit 을 config 에서 재동기화하고
    /// 새 컨텍스트에 accumulate_word 를 일관되게 반영한다.
    #[test]
    fn rebuild_reapplies_word_accumulation() {
        let config = Config::default();
        let mut engine = InputEngine::new(&config);
        engine.set_input_category(InputCategory::Korean);

        let mut word_config = Config::default();
        word_config.engine.korean.commit_unit = CommitUnit::Word;
        engine.rebuild_korean_context(&word_config);
        assert_eq!(engine.commit_unit, CommitUnit::Word);

        type_ganan(&mut engine, &word_config);
        assert_eq!(engine.preedit_str(), "가나", "rebuild 후 단어 누적 일관");
    }

    /// set_korean_layout 재구성 경로도 기존 commit_unit 캐시를 유지·반영한다.
    /// (3bul ↔ 2bul 전환을 거쳐도 누적 모드가 살아남는지 검증 — 키맵 비의존.)
    #[test]
    fn set_layout_preserves_word_accumulation() {
        let mut config = Config::default();
        config.engine.korean.commit_unit = CommitUnit::Word;
        let mut engine = InputEngine::new(&config);
        engine.set_input_category(InputCategory::Korean);

        // 자판 전환(2bul→3bul→2bul) 후에도 commit_unit 캐시·누적 상태 유지.
        engine.set_korean_layout("ko_3bul390".to_string());
        assert_eq!(engine.commit_unit, CommitUnit::Word);
        engine.set_korean_layout("ko_2bulstd".to_string());
        assert_eq!(engine.commit_unit, CommitUnit::Word);

        // 알려진 dubeolsik 키맵으로 "가나" 입력 → 누적 preedit 확인.
        type_ganan(&mut engine, &config);
        assert_eq!(
            engine.preedit_str(),
            "가나",
            "자판 전환 후에도 단어 누적이 유지되어야 함"
        );
    }

    // ─────────────────────────────────────────────
    // I3(ATF perf): reset() 확장 — new(config) 재생성 대체의 정확성 근거
    // ─────────────────────────────────────────────

    /// reset() 이 팝업/한자/특수문자 라우팅 상태를 비운다. 이 상태가 잔류하면
    /// 리셋 직후의 press_key 가 조합 대신 팝업 디스패치(process_popup_key)로
    /// 오라우팅된다 — ATF replay 가 새 조합을 만들 수 없게 된다.
    #[test]
    fn reset_clears_popup_and_hanja_routing_state() {
        let config = Config::default();
        let mut engine = InputEngine::new(&config);

        // 팝업/한자/특수문자 라우팅 상태를 인위적으로 활성화.
        engine.hanja_mode = true;
        engine.hanja_target.push('한');
        engine.special_char_mode = true;
        engine.special_char_candidates.push('★');
        engine.special_char_target.push('ㅁ');

        engine.reset();

        assert!(!engine.hanja_mode, "reset 후 hanja_mode off");
        assert!(engine.hanja_target.is_empty(), "reset 후 hanja_target 비움");
        assert!(!engine.special_char_mode, "reset 후 special_char_mode off");
        assert!(
            engine.special_char_candidates.is_empty(),
            "reset 후 special_char_candidates 비움"
        );
        assert!(
            engine.special_char_target.is_empty(),
            "reset 후 special_char_target 비움"
        );
        assert!(engine.popup_state.is_none(), "reset 후 popup_state None");

        // 라우팅 검증: hanja_mode 가 살아 있었다면 press_key 가 process_popup_key
        // 로 흘러 'a' 를 커밋하지 못한다. reset 이 비웠으므로 영문 정상 커밋.
        engine.set_input_category(InputCategory::English);
        let m = ModifierState::default();
        let r = engine.press_key(KeyCode::A, m, &config);
        assert!(r.consumed, "영문 키는 소비되어야");
        assert_eq!(engine.commit_str(), "a", "팝업 오라우팅 없이 정상 커밋");
    }

    /// reset() 이 진행 중 조합과 word_buffer(단어 누적)를 모두 비운다. ATF replay
    /// 가 이전 조합을 끌고 오지 않게 하는 근거. accumulate_word 플래그는 보존되어
    /// (Word 모드 유지) 이어지는 타이핑이 stale 누적 없이 새 단어로 누적된다.
    #[test]
    fn reset_clears_composition_and_word_buffer() {
        let mut config = Config::default();
        config.engine.korean.commit_unit = CommitUnit::Word;
        let mut engine = InputEngine::new(&config);
        engine.set_input_category(InputCategory::Korean);

        type_ganan(&mut engine, &config);
        assert_eq!(engine.preedit_str(), "가나", "리셋 전 단어 누적 확인");

        engine.reset();
        assert!(engine.preedit_str().is_empty(), "reset 후 preedit 비움");
        assert!(engine.commit_str().is_empty(), "reset 후 commit 비움");
        assert!(!engine.is_composing(), "reset 후 비조합");

        // accumulate_word 플래그는 보존(Word 모드 유지) → 재타이핑은 새 단어만.
        assert!(engine.is_word_mode(), "reset 은 accumulate_word 플래그 보존");
        type_ganan(&mut engine, &config);
        assert_eq!(
            engine.preedit_str(),
            "가나",
            "reset 후 재타이핑은 stale 누적 없이 새 단어"
        );
    }

    // ─────────────────────────────────────────────
    // 고정키(Sticky Keys) + 수정자 토글(RightAlt) 충돌 방어
    // ─────────────────────────────────────────────

    /// 고정키가 켜진 환경 재현: RightAlt 로 한/영 전환 성사 직후, OS 가 Alt 비트를
    /// 다음 키에 래치시킨다. 방어가 없으면 전환 직후 첫 자모키가 단축키(Alt+키)로
    /// 오분류돼 유실된다. 방어가 있으면 그 잔류 Alt 비트를 지우고 첫 자모를 정상 처리.
    #[test]
    fn sticky_keys_masks_latched_alt_on_first_jamo_after_rightalt_toggle() {
        let config = Config::default();
        let mut engine = InputEngine::new(&config);
        engine.set_input_category(InputCategory::English);

        // RightAlt 눌림 — 자신이 Alt 비트를 세운 채 들어와도 토글은 성사(self_is_modifier).
        let alt_down = ModifierState {
            alt: true,
            ..Default::default()
        };
        let r = engine.press_key(KeyCode::RightAlt, alt_down, &config);
        assert!(r.consumed, "RightAlt 토글은 소비되어야");
        assert_eq!(engine.input_category(), InputCategory::Korean, "한글로 전환");
        // 토글 성사 → 다음 키 1건 Alt 마스크 예약.
        assert_eq!(
            engine.sticky_toggle_mask,
            Some(LatchModifier::Alt),
            "수정자 토글키 성사 후 Alt 마스크 예약"
        );

        // 고정키 래치로 Alt 가 남은 채 첫 자모키 R 입력.
        let r2 = engine.press_key(KeyCode::R, alt_down, &config);
        assert!(r2.consumed, "첫 자모키는 소비되어야(유실 금지)");
        assert_eq!(engine.preedit_str(), "ㄱ", "래치 Alt 무시 후 첫 자모 정상 조합");
        assert!(
            engine.sticky_toggle_mask.is_none(),
            "마스크는 일회성 소비되어 None"
        );
    }

    /// 일회성: 마스크는 딱 한 키만 방어한다. 첫 키가 소비하면 그 다음 키의 래치
    /// Alt 는 다시 단축키로 취급된다(마스크가 무한 지속되지 않음을 보장).
    #[test]
    fn sticky_mask_is_consumed_after_single_key() {
        let config = Config::default();
        let mut engine = InputEngine::new(&config);
        engine.set_input_category(InputCategory::English);

        let alt_down = ModifierState {
            alt: true,
            ..Default::default()
        };
        engine.press_key(KeyCode::RightAlt, alt_down, &config); // 전환 + 마스크 예약
        engine.press_key(KeyCode::R, alt_down, &config); // 마스크 소비 → ㄱ
        assert_eq!(engine.preedit_str(), "ㄱ");

        // 다음 키도 Alt 래치가 남아 있다고 가정 → 마스크는 이미 없으므로 단축키 처리.
        // 조합(ㄱ) 중이므로 flush 후 committed, 자모는 추가되지 않는다.
        let r3 = engine.press_key(KeyCode::K, alt_down, &config);
        assert!(r3.commit_changed, "두 번째 Alt 키는 단축키 → 조합 flush/commit");
        assert!(
            !engine.preedit_str().contains('ㅏ') && !engine.preedit_str().contains('가'),
            "두 번째 키는 자모로 처리되지 않아야(마스크 일회성): preedit='{}'",
            engine.preedit_str()
        );
    }

    /// 비-수정자 토글키(Korean/한영)는 고정키 래치가 없으므로 마스크를 예약하지 않는다.
    /// 이후 진짜 Alt+키 단축키는 그대로 단축키로 유지(무회귀).
    #[test]
    fn non_modifier_toggle_key_does_not_reserve_mask() {
        let config = Config::default();
        let mut engine = InputEngine::new(&config);
        engine.set_input_category(InputCategory::English);

        let m = ModifierState::default();
        let r = engine.press_key(KeyCode::Korean, m, &config);
        assert!(r.consumed, "Korean 토글 소비");
        assert_eq!(engine.input_category(), InputCategory::Korean);
        assert!(
            engine.sticky_toggle_mask.is_none(),
            "비-수정자 토글키는 마스크 예약 안 함"
        );

        // 진짜 Alt+키 단축키는 마스킹 없이 그대로 통과(not_consumed).
        let alt_down = ModifierState {
            alt: true,
            ..Default::default()
        };
        let r2 = engine.press_key(KeyCode::R, alt_down, &config);
        assert!(!r2.consumed, "마스크 없으면 Alt+키는 단축키(not_consumed)");
        assert!(engine.preedit_str().is_empty(), "자모 조합 없음");
    }

    /// 무회귀: 선행 토글 없이 들어온 진짜 Alt+키 단축키는 마스크가 없어(None)
    /// 종전과 동일하게 단축키로 처리된다. 정상 경로 modifier 비변경.
    #[test]
    fn plain_alt_shortcut_without_prior_toggle_unaffected() {
        let config = Config::default();
        let mut engine = InputEngine::new(&config);
        engine.set_input_category(InputCategory::Korean);
        assert!(engine.sticky_toggle_mask.is_none());

        let alt_down = ModifierState {
            alt: true,
            ..Default::default()
        };
        let r = engine.press_key(KeyCode::R, alt_down, &config);
        assert!(!r.consumed, "선행 토글 없는 Alt+키는 단축키 그대로");
        assert!(engine.preedit_str().is_empty());
        assert!(engine.commit_str().is_empty());
    }
}
