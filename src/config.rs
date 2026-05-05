//! 설정 모듈
//!
//! UNIM 입력기의 설정을 관리합니다.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fs;
use std::path::PathBuf;
use std::time::SystemTime;

/// 입력 카테고리 (한국어/영어)
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default, Serialize, Deserialize)]
#[repr(C)]
pub enum InputCategory {
    /// 한국어 (Korean) 입력 모드
    Korean,
    /// 영어 (English) 입력 모드
    #[default]
    English,
}

/// 입력 모드 공유 방식
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default, Serialize, Deserialize)]
#[repr(C)]
pub enum ModeSharingMode {
    /// 모든 앱/창이 동일한 한/영 상태를 공유 (기본)
    #[default]
    Global,
    /// 각 앱이 독립적인 한/영 상태를 유지 (window_id의 ':' 앞부분으로 앱 식별)
    PerApp,
}

impl ModeSharingMode {
    /// 표시용 레이블을 반환합니다.
    pub fn display_name(&self) -> &'static str {
        match self {
            ModeSharingMode::Global => "전역 공유",
            ModeSharingMode::PerApp => "앱별 독립",
        }
    }

    /// 사용 가능한 모든 모드를 반환합니다.
    pub fn all() -> &'static [ModeSharingMode] {
        &[ModeSharingMode::Global, ModeSharingMode::PerApp]
    }
}

/// 한국어 키보드 레이아웃 식별자 — 자판 프로필 이름을 담는 문자열 래퍼.
///
/// 레거시 시절 enum(`Dubeolsik` / `Sebeolsik390` / ...)이었던 필드를 Phase 8에서
/// 문자열로 통합. 내장 4종은 `ko_2bulstd`·`ko_3bul390`·`ko_3bul391`·`ko_3bul_noshift`,
/// 사용자 정의는 `~/.config/unim/layouts/<name>.json` 파일의 `name` 필드.
///
/// YAML 역직렬화 시 `normalize_korean_layout_name`을 거쳐 레거시 enum 값
/// (`Dubeolsik` 등)과 별칭(`2bul` 등)을 모두 수용한다. 이는 *config 값 정규화*이며,
/// *프로필 JSON 스키마*와는 무관하다 (v0 layout profile JSON은 0.2.0+ 거부 — 별도
/// `LoadError::UnsupportedSchema`).
pub type KoreanLayout = String;

/// 내장 한국어 자판의 정식 이름(레지스트리 키).
pub const KOREAN_LAYOUT_DUBEOLSIK: &str = "ko_2bulstd";
pub const KOREAN_LAYOUT_SEBEOLSIK_390: &str = "ko_3bul390";
pub const KOREAN_LAYOUT_SEBEOLSIK_391: &str = "ko_3bul391";
pub const KOREAN_LAYOUT_SEBEOLSIK_NOSHIFT: &str = "ko_3bul_noshift";

/// 내장 한국어 자판 4종(이관 시점 기준). 표시용·CLI 나열 등에 사용.
///
/// 0.2.0+: 쿼티형 세벌식(`ko_3bul_qwerty`)은 빌트인에서 제거되어 연구 자료로만
/// 보존된다. 사용자가 직접 사용하려면 `docs/references/keymaps/ko_3bul_qwerty.json`
/// 을 `~/.config/unim/layouts/`에 복사.
pub const KOREAN_LAYOUT_BUILTINS: &[&str] = &[
    KOREAN_LAYOUT_DUBEOLSIK,
    KOREAN_LAYOUT_SEBEOLSIK_390,
    KOREAN_LAYOUT_SEBEOLSIK_391,
    KOREAN_LAYOUT_SEBEOLSIK_NOSHIFT,
];

/// 레거시 enum 이름·별칭을 내장 정식 이름으로 승격한다.
///
/// 예: `"Dubeolsik"`·`"2bul"`·`"2bulstd"` → `"ko_2bulstd"`. 이미 정식 이름이거나
/// 매칭 대상이 없으면 입력을 그대로 반환(사용자 프로필 이름 허용).
pub fn normalize_korean_layout_name(raw: &str) -> String {
    match raw {
        "Dubeolsik" | "2bul" | "2bulstd" => KOREAN_LAYOUT_DUBEOLSIK.to_string(),
        "Sebeolsik390" | "390" | "3bul390" => KOREAN_LAYOUT_SEBEOLSIK_390.to_string(),
        "Sebeolsik391" | "391" | "3bul391" => KOREAN_LAYOUT_SEBEOLSIK_391.to_string(),
        "SebeolsikNoShift" | "noshift" | "3bul_noshift" => {
            KOREAN_LAYOUT_SEBEOLSIK_NOSHIFT.to_string()
        }
        other => other.to_string(),
    }
}

/// 세벌식 계열 자판인지 판정. `ko_3bul*` 또는 `3bul*` 접두 또는 명시 리스트 매칭.
pub fn is_sebeolsik_layout(name: &str) -> bool {
    matches!(
        name,
        "Sebeolsik390"
            | "Sebeolsik391"
            | "SebeolsikNoShift"
            | "390"
            | "391"
            | "noshift"
            | "ko_anmatae"
            | "anmatae"
            | "anmatae_2003"
    ) || name.starts_with("ko_3bul")
        || name.starts_with("3bul")
}

/// 내장 자판의 한국어 표시 레이블.
///
/// 반환값이 `""`이면 "내장이 아님"(= 사용자 프로필). GUI는 프로필 metadata의
/// `display_name`을 resolve 해야 한다.
pub fn korean_layout_display_name(name: &str) -> &'static str {
    match normalize_korean_layout_name(name).as_str() {
        KOREAN_LAYOUT_DUBEOLSIK => "두벌식 표준",
        KOREAN_LAYOUT_SEBEOLSIK_390 => "세벌식 390",
        KOREAN_LAYOUT_SEBEOLSIK_391 => "세벌식 최종",
        KOREAN_LAYOUT_SEBEOLSIK_NOSHIFT => "세벌식 순아래",
        _ => "",
    }
}

/// 영어 키보드 레이아웃 식별자 — 자판 프로필 이름을 담는 문자열 래퍼.
///
/// 레거시 시절 enum(`Qwerty` / `Dvorak` / ...)이었던 필드를 Phase 9에서
/// 문자열로 통합. 내장 5종은 `qwerty`·`dvorak`·`colemak`·`colemak_dh`·`workman`.
///
/// YAML 역직렬화 시 `EnglishConfigCompat`을 거쳐 레거시 enum 값(`Qwerty` 등)을
/// 소문자 내장 이름으로 승격한다. 타입 호환을 위해 String 별칭으로 노출.
pub type EnglishLayout = String;

/// 내장 영어 자판의 정식 이름.
pub const ENGLISH_LAYOUT_QWERTY: &str = "qwerty";
pub const ENGLISH_LAYOUT_DVORAK: &str = "dvorak";
pub const ENGLISH_LAYOUT_COLEMAK: &str = "colemak";
pub const ENGLISH_LAYOUT_COLEMAK_DH: &str = "colemak_dh";
pub const ENGLISH_LAYOUT_WORKMAN: &str = "workman";

/// 내장 영어 자판 5종.
pub const ENGLISH_LAYOUT_BUILTINS: &[&str] = &[
    ENGLISH_LAYOUT_QWERTY,
    ENGLISH_LAYOUT_DVORAK,
    ENGLISH_LAYOUT_COLEMAK,
    ENGLISH_LAYOUT_COLEMAK_DH,
    ENGLISH_LAYOUT_WORKMAN,
];

/// 레거시 enum 이름을 소문자 내장 이름으로 승격한다. 알 수 없는 값은 pass-through.
pub fn normalize_english_layout_name(raw: &str) -> String {
    match raw {
        "Qwerty" => ENGLISH_LAYOUT_QWERTY.to_string(),
        "Dvorak" => ENGLISH_LAYOUT_DVORAK.to_string(),
        "Colemak" => ENGLISH_LAYOUT_COLEMAK.to_string(),
        "ColemakDh" | "colemak-dh" => ENGLISH_LAYOUT_COLEMAK_DH.to_string(),
        "Workman" => ENGLISH_LAYOUT_WORKMAN.to_string(),
        other => other.to_string(),
    }
}

/// 내장 영어 자판의 keymap JSON 파일명. 내장이 아니면 입력을 그대로 반환
/// (사용자 프로필 이름이 곧 파일명 stem인 경우를 위해).
pub fn english_layout_keymap_name(name: &str) -> String {
    let normalized = normalize_english_layout_name(name);
    match normalized.as_str() {
        ENGLISH_LAYOUT_QWERTY => "en_qwerty".to_string(),
        ENGLISH_LAYOUT_DVORAK => "en_dvorak".to_string(),
        ENGLISH_LAYOUT_COLEMAK => "en_colemak".to_string(),
        ENGLISH_LAYOUT_COLEMAK_DH => "en_colemak_dh".to_string(),
        ENGLISH_LAYOUT_WORKMAN => "en_workman".to_string(),
        _ => normalized,
    }
}

/// 내장 영어 자판의 표시 레이블. 사용자 프로필이면 빈 문자열 — GUI는
/// 프로필 metadata의 display_name을 resolve 해야 한다.
pub fn english_layout_display_name(name: &str) -> &'static str {
    match normalize_english_layout_name(name).as_str() {
        ENGLISH_LAYOUT_QWERTY => "QWERTY",
        ENGLISH_LAYOUT_DVORAK => "Dvorak",
        ENGLISH_LAYOUT_COLEMAK => "Colemak",
        ENGLISH_LAYOUT_COLEMAK_DH => "Colemak-DH",
        ENGLISH_LAYOUT_WORKMAN => "Workman",
        _ => "",
    }
}

/// 내장 영어 자판의 상단 행(2nd row) 앞 9개 키 레이블 — 특수문자 팝업 헤더용.
/// 내장이 아니면 빈 문자열.
pub fn english_layout_top_row_labels(name: &str) -> &'static str {
    match normalize_english_layout_name(name).as_str() {
        ENGLISH_LAYOUT_QWERTY => "QWERTYUIO",
        ENGLISH_LAYOUT_DVORAK => "',.PYFGCR",
        ENGLISH_LAYOUT_COLEMAK => "QWFPGJLUY",
        ENGLISH_LAYOUT_COLEMAK_DH => "QWFPBJLUY",
        ENGLISH_LAYOUT_WORKMAN => "QDRWBJFUP",
        _ => "",
    }
}

/// 내장 영어 자판의 홈 행(3rd row) 앞 9개 키 레이블 — 이모지 카테고리 단축키 표시용.
/// 물리 키 위치 (qwerty 의 ASDFGHJKL) 는 `KeyCode::A..L` 로 고정 매핑되며, 본 함수는
/// 사용자에게 보일 레이블만 레이아웃에 따라 변환한다.
/// 내장이 아니면 빈 문자열.
pub fn english_layout_home_row_labels(name: &str) -> &'static str {
    match normalize_english_layout_name(name).as_str() {
        ENGLISH_LAYOUT_QWERTY => "ASDFGHJKL",
        ENGLISH_LAYOUT_DVORAK => "AOEUIDHTN",
        ENGLISH_LAYOUT_COLEMAK => "ARSTDHNEI",
        ENGLISH_LAYOUT_COLEMAK_DH => "ARSTGMNEI",
        ENGLISH_LAYOUT_WORKMAN => "ASHTGYNEO",
        _ => "",
    }
}

/// 입력 필드의 목적 (Content Type Hint)
///
/// 프론트엔드가 감지한 입력 필드의 용도를 나타냅니다.
/// Password/Pin 필드에서는 한글 모드를 자동으로 차단합니다.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default, Serialize, Deserialize)]
#[repr(u32)]
pub enum ContentPurpose {
    /// 일반 텍스트 입력
    #[default]
    Normal = 0,
    /// 비밀번호 입력 (한글 차단)
    Password = 1,
    /// PIN 입력 (한글 차단)
    Pin = 2,
    /// 이메일 주소 입력
    Email = 3,
    /// 숫자 입력
    Number = 4,
    /// URL 입력
    Url = 5,
    /// 터미널 입력
    Terminal = 6,
}

impl ContentPurpose {
    /// u32 값에서 변환합니다.
    pub fn from_u32(value: u32) -> Self {
        match value {
            0 => ContentPurpose::Normal,
            1 => ContentPurpose::Password,
            2 => ContentPurpose::Pin,
            3 => ContentPurpose::Email,
            4 => ContentPurpose::Number,
            5 => ContentPurpose::Url,
            6 => ContentPurpose::Terminal,
            _ => ContentPurpose::Normal,
        }
    }

    /// 한글 입력을 차단해야 하는 목적인지 확인합니다.
    pub fn should_block_hangul(&self) -> bool {
        matches!(self, ContentPurpose::Password | ContentPurpose::Pin)
    }
}

/// AutoTypeFix 값 범위 상수
pub const AUTO_TYPEFIX_KOR_THRESHOLD_MIN: u8 = 2;
pub const AUTO_TYPEFIX_KOR_THRESHOLD_MAX: u8 = 6;
pub const AUTO_TYPEFIX_ENG_MIN_LENGTH_MIN: u8 = 3;
pub const AUTO_TYPEFIX_ENG_MIN_LENGTH_MAX: u8 = 8;
pub const AUTO_TYPEFIX_TIME_WINDOW_MIN: u32 = 500;
pub const AUTO_TYPEFIX_TIME_WINDOW_MAX: u32 = 5000;
pub const AUTO_TYPEFIX_TENTATIVE_EXPIRY_MIN: u16 = 1;
pub const AUTO_TYPEFIX_TENTATIVE_EXPIRY_MAX: u16 = 12;
pub const AUTO_TYPEFIX_OBSERVATION_TIMEOUT_MIN: u8 = 5;
pub const AUTO_TYPEFIX_OBSERVATION_TIMEOUT_MAX: u8 = 15;

fn default_auto_typefix_enabled() -> bool {
    true
}
fn default_auto_typefix_time_window_ms() -> u32 {
    5000
}
fn default_auto_typefix_forward_time_window_ms() -> u32 {
    default_auto_typefix_time_window_ms()
}
fn default_auto_typefix_reverse_time_window_ms() -> u32 {
    default_auto_typefix_time_window_ms()
}
fn default_auto_typefix_kor_syllable_threshold() -> u8 {
    2
}
fn default_auto_typefix_eng_word_min_length() -> u8 {
    5
}
fn default_auto_typefix_forward() -> bool {
    true
}
fn default_auto_typefix_reverse() -> bool {
    true
}
fn default_auto_typefix_skip_on_english_word() -> bool {
    true
}
fn default_auto_typefix_skip_on_complete_syllable() -> bool {
    true
}
fn default_auto_typefix_rollback_detection() -> bool {
    true
}
fn default_auto_typefix_tentative_expiry_hours() -> u16 {
    4
}
fn default_auto_typefix_observation_timeout_secs() -> u8 {
    10
}
fn default_auto_typefix_user_dict_enabled() -> bool {
    true
}

/// 자동 오타 교정 (AutoTypeFix) 설정
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct AutoTypeFixConfig {
    /// 활성화 여부
    #[serde(default = "default_auto_typefix_enabled")]
    pub enabled: bool,
    /// 순방향 (영→한) 시간 윈도우 (ms) — 이 시간 내의 키스트로크만 검사 (500~5000)
    #[serde(default = "default_auto_typefix_forward_time_window_ms")]
    pub forward_time_window_ms: u32,
    /// 역방향 (한→영) 시간 윈도우 (ms) — 이 시간 내의 키스트로크만 검사 (500~5000)
    #[serde(default = "default_auto_typefix_reverse_time_window_ms")]
    pub reverse_time_window_ms: u32,
    /// [DEPRECATED] 구(舊) 통합 시간 윈도우. 후처리 단계에서 forward/reverse 두 필드에
    /// 주입되고 제거된다. 직렬화에서는 `skip_serializing_if`로 숨겨 신규 config 파일에는
    /// 남지 않는다. 역호환 용도로만 유지.
    #[serde(default, skip_serializing)]
    pub time_window_ms: Option<u32>,
    /// 순방향 (영→한) 트리거: 한글 완성 음절 수 (2~6)
    #[serde(default = "default_auto_typefix_kor_syllable_threshold")]
    pub kor_syllable_threshold: u8,
    /// 역방향 (한→영) 트리거: 영문 단어 최소 길이 (3~8)
    #[serde(default = "default_auto_typefix_eng_word_min_length")]
    pub eng_word_min_length: u8,
    /// 순방향 (영→한 교정) 활성화
    #[serde(default = "default_auto_typefix_forward")]
    pub forward: bool,
    /// 역방향 (한→영 교정) 활성화
    #[serde(default = "default_auto_typefix_reverse")]
    pub reverse: bool,
    /// 순방향 트리거 시 영단어 매칭(사전 hit)이면 억제 (기본 true, 기존 동작 유지)
    #[serde(default = "default_auto_typefix_skip_on_english_word")]
    pub skip_on_english_word: bool,
    /// 역방향 트리거 시 버퍼의 한글이 모두 완성 음절이면 억제 (기본 true, 기존 동작 유지)
    #[serde(default = "default_auto_typefix_skip_on_complete_syllable")]
    pub skip_on_complete_syllable: bool,
    /// 재트리거 기반 학습형 억제. 동일 입력이 관찰 창 내에 재차 트리거되면
    /// 오탐 후보로 간주해 해당 순간에도 교정을 억제하고 blacklist에 기록한다. (기본 true)
    #[serde(default = "default_auto_typefix_rollback_detection")]
    pub rollback_detection: bool,
    /// 임시 억제 단어 만료 기간 (시간) — 이 기간 내 수동 확정 안 되면 inactive로 전환 (1~12, 기본 4)
    #[serde(default = "default_auto_typefix_tentative_expiry_hours")]
    pub tentative_expiry_hours: u16,
    /// 재트리거 관찰 창 (초) — 첫 교정 후 이 시간 내에 동일 입력이 재트리거되면 오탐으로 판정 (5~15, 기본 10)
    #[serde(default = "default_auto_typefix_observation_timeout_secs")]
    pub observation_timeout_secs: u8,
    /// 역방향 사용자 사전(`~/.config/unim/typefix-userdict.yaml`) 사용 여부.
    /// 활성 시 사전 등록 단어는 `eng_word_min_length` 및 내장 영어 사전 검사를 우회하여 즉시 교정.
    #[serde(default = "default_auto_typefix_user_dict_enabled")]
    pub user_dict_enabled: bool,
}

impl Default for AutoTypeFixConfig {
    fn default() -> Self {
        Self {
            enabled: default_auto_typefix_enabled(),
            forward_time_window_ms: default_auto_typefix_forward_time_window_ms(),
            reverse_time_window_ms: default_auto_typefix_reverse_time_window_ms(),
            time_window_ms: None,
            kor_syllable_threshold: default_auto_typefix_kor_syllable_threshold(),
            eng_word_min_length: default_auto_typefix_eng_word_min_length(),
            forward: default_auto_typefix_forward(),
            reverse: default_auto_typefix_reverse(),
            skip_on_english_word: default_auto_typefix_skip_on_english_word(),
            skip_on_complete_syllable: default_auto_typefix_skip_on_complete_syllable(),
            rollback_detection: default_auto_typefix_rollback_detection(),
            tentative_expiry_hours: default_auto_typefix_tentative_expiry_hours(),
            observation_timeout_secs: default_auto_typefix_observation_timeout_secs(),
            user_dict_enabled: default_auto_typefix_user_dict_enabled(),
        }
    }
}

impl AutoTypeFixConfig {
    /// 값 범위를 허용 범위로 강제 보정합니다.
    ///
    /// CLI/GUI 양쪽에서 중복 범위 검증을 피하기 위해 config 레벨에서 한 번에 clamp.
    pub fn clamp_ranges(&mut self) {
        self.kor_syllable_threshold = self.kor_syllable_threshold.clamp(
            AUTO_TYPEFIX_KOR_THRESHOLD_MIN,
            AUTO_TYPEFIX_KOR_THRESHOLD_MAX,
        );
        self.eng_word_min_length = self.eng_word_min_length.clamp(
            AUTO_TYPEFIX_ENG_MIN_LENGTH_MIN,
            AUTO_TYPEFIX_ENG_MIN_LENGTH_MAX,
        );
        // 구(舊) 통합 필드가 남아있다면 신규 두 필드 중 기본값인 쪽에 주입.
        // (역호환: 구 yaml은 forward/reverse를 직접 쓰지 않았으므로 둘 다 기본값이다.)
        if let Some(legacy) = self.time_window_ms.take() {
            let fwd_default = default_auto_typefix_forward_time_window_ms();
            let rev_default = default_auto_typefix_reverse_time_window_ms();
            if self.forward_time_window_ms == fwd_default {
                self.forward_time_window_ms = legacy;
            }
            if self.reverse_time_window_ms == rev_default {
                self.reverse_time_window_ms = legacy;
            }
        }
        self.forward_time_window_ms = self
            .forward_time_window_ms
            .clamp(AUTO_TYPEFIX_TIME_WINDOW_MIN, AUTO_TYPEFIX_TIME_WINDOW_MAX);
        self.reverse_time_window_ms = self
            .reverse_time_window_ms
            .clamp(AUTO_TYPEFIX_TIME_WINDOW_MIN, AUTO_TYPEFIX_TIME_WINDOW_MAX);
        self.tentative_expiry_hours = self.tentative_expiry_hours.clamp(
            AUTO_TYPEFIX_TENTATIVE_EXPIRY_MIN,
            AUTO_TYPEFIX_TENTATIVE_EXPIRY_MAX,
        );
        self.observation_timeout_secs = self.observation_timeout_secs.clamp(
            AUTO_TYPEFIX_OBSERVATION_TIMEOUT_MIN,
            AUTO_TYPEFIX_OBSERVATION_TIMEOUT_MAX,
        );
    }
}

fn default_auto_english_enabled() -> bool {
    false
}
fn default_auto_english_trigger_keys() -> Vec<String> {
    vec!["key:Escape".to_string(), "char:/".to_string()]
}

/// 자동 영문 모드 전환 설정
///
/// 특정 키(기본: `key:Escape`, `char:/`)를 한글 모드에서 입력하면
/// 조합을 커밋하고 영문 모드로 영구 전환한다. vi/vim 명령 모드 진입,
/// CLI 도구의 `/` prefix 같은 워크플로우를 편리하게 만든다.
///
/// 트리거 키 표기 문법(접두사 기반):
/// - `"key:<KeyCode>"`  — Functional 매칭. KeyCode 이름 직접 비교.
///   예: `"key:Escape"`, `"key:Tab"`, `"key:F1"`, `"key:ShiftSemicolon"` (Shift 조합).
/// - `"char:<문자>"`    — Character 매칭. 키맵을 거쳐 산출된 char 와 비교.
///   비-QWERTY 한국어 레이아웃(예: 세벌식390) 에서도 산출 문자가 같으면 발동.
///   예: `"char:/"`, `"char:,"`, `"char:?"`. shift 무관 — `'/'` 와 `'?'` 를
///   구분하려면 둘 다 등록.
/// - 무접두사(legacy)   — 종전 표기 유지. `"Escape"` / `"Slash"` / `"ShiftSemicolon"`
///   등은 자동으로 Functional 로 흡수되어 100% 호환.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct AutoEnglishConfig {
    /// 활성화 여부 (기본 false, opt-in)
    #[serde(default = "default_auto_english_enabled")]
    pub enabled: bool,
    /// 자동 영문 전환을 유발하는 트리거 키 표기 목록.
    /// 표기 문법은 위 docstring 참조 (`key:` / `char:` / 무접두사 legacy).
    #[serde(default = "default_auto_english_trigger_keys")]
    pub trigger_keys: Vec<String>,
}

impl Default for AutoEnglishConfig {
    fn default() -> Self {
        Self {
            enabled: default_auto_english_enabled(),
            trigger_keys: default_auto_english_trigger_keys(),
        }
    }
}

/// 한국어 엔진 설정
///
/// Phase 8에서 `layout` 필드가 enum에서 문자열로 통합됨. 이전의 `custom_layout`
/// 우회 필드는 제거되었고, 레거시 YAML(`layout: Dubeolsik` / `custom_layout: X`)은
/// `Deserialize` 구현에서 자동 흡수된다.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default, from = "KoreanConfigCompat")]
pub struct KoreanConfig {
    /// 한국어 자판 프로필 이름(레지스트리 키). 기본값 `ko_2bulstd`.
    pub layout: KoreanLayout,
    /// 활성 규칙 세트 이름 목록 (자판 프로필 v1 — `docs/plans/LAYOUT_PROFILE_V1.md` §3.5).
    ///
    /// engine 측 `LayoutProfile.active_rule_sets`(Option<Vec<String>>) 의미와 일치:
    /// - `None` = 미설정. 프로필 기본값 사용(각 `rule_sets.<name>.active` 그대로).
    /// - `Some(list)` = 명시적 활성 목록. 목록에 있는 이름만 active, 나머지 강제 off.
    /// - `Some(vec![])` = 사용자가 모든 규칙 세트를 끔(All OFF). 프로필 기본값으로
    ///   되돌리지 **않으며** 이 의도가 그대로 유지된다.
    ///
    /// `None`은 `serde(skip_serializing_if = "Option::is_none")`로 YAML에서 필드 자체가
    /// 누락된다(기본 상태에서 config.yaml이 깔끔). `Some(vec![])`은 `[]`로 명시 저장.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub active_rule_sets: Option<Vec<String>>,

    /// 키맵(자판 프로필)별 `active_rule_sets` 스냅샷 캐시 — 사용자 편의용.
    ///
    /// GTK 설정에서 자판을 전환할 때 이전 자판의 활성 룰셋 ON/OFF 상태를 잃지 않도록
    /// 키맵 이름을 키로 매핑해 보존한다. 기본값은 빈 맵 (캐시 미사용 → 프로필 기본값).
    ///
    /// - **Key**: `effective_layout_name()` (정규화된 자판 이름, 예: `ko_2bulstd`).
    /// - **Value**: 그 자판에서 사용자가 마지막으로 설정한 active rule_sets 이름 목록.
    ///   빈 vec(`[]`)도 유효한 의도("모두 OFF") — `active_rule_sets: Some(vec![])` 의미와 동일.
    ///
    /// daemon은 본 캐시를 읽지 **않는다** — 활성 자판의 효과는 `active_rule_sets` 만 결정.
    /// GUI가 자판 전환 시 본 캐시에서 새 자판의 값을 꺼내 `active_rule_sets`로 복원한다.
    ///
    /// `BTreeMap`은 직렬화 순서 안정성 (자판명 알파벳 순서). 빈 맵일 때 YAML에서 누락.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub layout_rule_sets: BTreeMap<String, Vec<String>>,

    /// 모아치기 — 양방향 자모 결합 사용자 재정의.
    ///
    /// `None` = 자판 프로필 기본값 사용(`supports_moachigi=true` 자판의 `bidirectional_combine`).
    /// `Some(true/false)` = 사용자가 프로필 기본값을 재정의.
    /// `supports_moachigi=false` 자판에서는 무시된다.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bidirectional_combine: Option<bool>,

    /// 모아치기 — 동시 입력 시간 (밀리초).
    ///
    /// `None` = 자판 프로필 기본값 사용(`chord_window_ms`).
    /// `Some(0)` = 동시 입력 OFF. `Some(N)` = N ms 이내에 들어온 키를 한 음절로 모아 처리.
    /// `supports_moachigi=false` 자판에서는 무시된다.
    /// `bidirectional_combine=false`이면 chord도 무시된다 (옵션 2 종속).
    /// InputEngine 레벨 ChordBuffer로 구현됨 (idle flush + force_flush 패턴).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub chord_window_ms: Option<u16>,
}

impl Default for KoreanConfig {
    fn default() -> Self {
        Self {
            layout: KOREAN_LAYOUT_DUBEOLSIK.to_string(),
            active_rule_sets: None,
            layout_rule_sets: BTreeMap::new(),
            bidirectional_combine: None,
            chord_window_ms: None,
        }
    }
}

impl KoreanConfig {
    /// 실제 로드할 프로필 이름을 반환. 단순히 `layout`의 clone이며 정식 이름으로
    /// 정규화(`normalize_korean_layout_name`)를 거친다. 레지스트리의 별칭 처리도
    /// 있어 두 단계 모두 안전하다.
    pub fn effective_layout_name(&self) -> String {
        normalize_korean_layout_name(&self.layout)
    }

    /// 한국어 자판 전환을 캐시 보존/복원과 함께 수행한다.
    ///
    /// **이 함수는 GUI 토글이 만드는 사용자 의도를 자판 전환 직후에도 유지하는
    /// 데에 핵심**이다 — 이전 자판의 `active_rule_sets`(Some)을 캐시에 저장하고,
    /// 새 자판의 캐시된 값(있으면)을 복원한다.
    ///
    /// - **이전 자판**: `active_rule_sets == Some(...)`이면 그 자판 이름으로
    ///   `layout_rule_sets`에 보존. `None` (프로필 기본값)은 캐시하지 않는다 —
    ///   "기본값 사용" 의도가 의미상 캐시 hit과 구별되어야 하기 때문.
    /// - **새 자판**: `layout_rule_sets[new_layout]`이 존재하면 그 값으로
    ///   `active_rule_sets` 복원, 없으면 `None`(프로필 기본값).
    ///
    /// 호출자가 새 프로필의 유효한 rule_set 이름 슬라이스를 알고 있으면
    /// `valid_rule_set_names`에 넘기면 stale 이름을 자동 정리한다 (예: 사용자가
    /// 캐시 후 프로필 JSON을 편집해 일부 rule_set을 제거한 경우). `None`이면 필터링
    /// 생략 — 다음 GUI/CLI 조작 때 정리된다.
    ///
    /// 같은 자판으로의 "전환"은 의미가 없으므로 무시한다 (캐시 변동 없음).
    pub fn switch_layout(&mut self, new_layout: &str, valid_rule_set_names: Option<&[String]>) {
        let new_norm = normalize_korean_layout_name(new_layout);
        let old_norm = self.effective_layout_name();
        if old_norm == new_norm {
            return;
        }

        // 1. 이전 자판의 명시 상태(Some) 보존. None은 캐시하지 않음.
        if let Some(active) = self.active_rule_sets.as_ref() {
            self.layout_rule_sets.insert(old_norm, active.clone());
        }

        // 2. 자판 변경.
        self.layout = new_norm.clone();

        // 3. 새 자판의 캐시 복원 (없으면 None → 프로필 기본값 사용).
        self.active_rule_sets = self.layout_rule_sets.get(&new_norm).cloned();

        // 4. stale rule_set 이름 정리 (호출자가 valid 집합 제공한 경우).
        if let (Some(list), Some(valid)) = (self.active_rule_sets.as_mut(), valid_rule_set_names) {
            list.retain(|n| valid.iter().any(|v| v == n));
        }
    }

    /// 현재 자판의 `active_rule_sets` 값을 캐시(`layout_rule_sets`)에 동기화한다.
    /// 사용자 토글 또는 CLI `set` 직후에 호출.
    ///
    /// - `Some(list)` → `layout_rule_sets[current_layout] = list` (덮어쓰기).
    /// - `None` (프로필 기본값으로 재설정 의도) → 해당 자판의 캐시 entry 제거.
    ///   (캐시가 남아있으면 다음 자판 전환 시 stale Some으로 복원되어 사용자 의도와
    ///   어긋난다.)
    pub fn cache_active_rule_sets(&mut self) {
        let layout = self.effective_layout_name();
        match self.active_rule_sets.as_ref() {
            Some(list) => {
                self.layout_rule_sets.insert(layout, list.clone());
            }
            None => {
                self.layout_rule_sets.remove(&layout);
            }
        }
    }
}

/// 역직렬화 호환 레이어.
///
/// 아래 3가지 형태를 모두 `KoreanConfig`로 승격한다:
/// - 현재 포맷 (`layout: "ko_2bulstd"`, `active_rule_sets: [...]`)
/// - 레거시 Phase 4b (`layout: "Dubeolsik"`, `custom_layout: "X"`)
/// - 레거시 Phase 4a (`layout: "Dubeolsik"`)
/// - 레거시 pre-Phase 4 (`layout: Dubeolsik` as bare YAML — serde yaml도 String으로 읽음)
#[derive(Deserialize)]
#[serde(default)]
struct KoreanConfigCompat {
    layout: String,
    preedit_johab: bool,
    word_commit: bool,
    /// `serde(default)` + `Option<Vec<String>>` — YAML에 키 자체가 없으면 `None`,
    /// 있으면 `Some(...)` (빈 list `[]` 포함, 이는 "모두 OFF" 의도).
    #[serde(default)]
    active_rule_sets: Option<Vec<String>>,
    /// 키맵별 active_rule_sets 캐시 (GTK GUI 사용자 편의용).
    #[serde(default)]
    layout_rule_sets: BTreeMap<String, Vec<String>>,
    /// 레거시 override 필드 — Some이면 `layout`을 덮어씀. 본 통합 이후 제거됨.
    #[serde(default)]
    custom_layout: Option<String>,
    /// 모아치기 — 양방향 자모 결합 사용자 재정의.
    #[serde(default)]
    bidirectional_combine: Option<bool>,
    /// 모아치기 — 동시 입력 시간 (ms). 0=OFF. N ms 이내 키를 한 음절로 묶음 처리.
    #[serde(default)]
    chord_window_ms: Option<u16>,
}

impl Default for KoreanConfigCompat {
    fn default() -> Self {
        Self {
            layout: KOREAN_LAYOUT_DUBEOLSIK.to_string(),
            preedit_johab: false,
            word_commit: false,
            active_rule_sets: None,
            layout_rule_sets: BTreeMap::new(),
            custom_layout: None,
            bidirectional_combine: None,
            chord_window_ms: None,
        }
    }
}

impl From<KoreanConfigCompat> for KoreanConfig {
    fn from(c: KoreanConfigCompat) -> Self {
        let layout = match c.custom_layout {
            Some(ref s) if !s.is_empty() => s.clone(),
            _ => c.layout,
        };
        Self {
            layout: normalize_korean_layout_name(&layout),
            active_rule_sets: c.active_rule_sets,
            layout_rule_sets: c.layout_rule_sets,
            bidirectional_combine: c.bidirectional_combine,
            chord_window_ms: c.chord_window_ms,
        }
    }
}

/// 영어 엔진 설정
///
/// Phase 9에서 `layout` 필드가 enum → String. 레거시 YAML(`layout: Qwerty`)은
/// `EnglishConfigCompat`을 거쳐 자동 정규화.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default, from = "EnglishConfigCompat")]
pub struct EnglishConfig {
    /// 영어 키보드 레이아웃 프로필 이름(레지스트리 키). 기본값 `qwerty`.
    pub layout: EnglishLayout,
    /// 다이렉트 입력 선호
    pub preferred_direct: bool,
}

impl Default for EnglishConfig {
    fn default() -> Self {
        Self {
            layout: ENGLISH_LAYOUT_QWERTY.to_string(),
            preferred_direct: true,
        }
    }
}

/// 역직렬화 호환 레이어 — 레거시 enum 이름(`Qwerty` 등)을 소문자 내장 이름으로 승격.
#[derive(Deserialize)]
#[serde(default)]
struct EnglishConfigCompat {
    layout: String,
    preferred_direct: bool,
}

impl Default for EnglishConfigCompat {
    fn default() -> Self {
        Self {
            layout: ENGLISH_LAYOUT_QWERTY.to_string(),
            preferred_direct: true,
        }
    }
}

impl From<EnglishConfigCompat> for EnglishConfig {
    fn from(c: EnglishConfigCompat) -> Self {
        Self {
            layout: normalize_english_layout_name(&c.layout),
            preferred_direct: c.preferred_direct,
        }
    }
}

/// 앱별 기본 모드 규칙
///
/// 특정 앱이 포커스를 받을 때 자동으로 설정할 입력 모드입니다.
/// 예: 터미널은 항상 영문, 한글 에디터는 항상 한글
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AppRule {
    /// 앱 패턴 (window_id 또는 client_name에 대한 부분 문자열 매칭)
    pub app_pattern: String,
    /// 해당 앱의 기본 입력 카테고리
    pub default_category: InputCategory,
}

/// 엔진 설정
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct EngineConfig {
    /// 기본 입력 카테고리
    pub default_category: InputCategory,
    /// 입력 모드 공유 방식
    pub mode_sharing: ModeSharingMode,
    /// 한국어 설정
    pub korean: KoreanConfig,
    /// 영어 설정
    pub english: EnglishConfig,
    /// 한/영 전환 키 목록 (KeyCode 이름)
    pub toggle_keys: Vec<String>,
    /// 한자/특수문자 키 목록 (KeyCode 이름)
    pub hanja_keys: Vec<String>,
    /// 앱별 기본 모드 규칙
    pub app_rules: Vec<AppRule>,
    /// 자동 오타 교정 (AutoTypeFix) 설정
    pub auto_typefix: AutoTypeFixConfig,
    /// 특정 키 입력 시 자동으로 영문 모드로 전환하는 설정
    pub auto_english: AutoEnglishConfig,
    /// 이모지 팝업 설정 (Super+. 단축키)
    pub emoji_popup: EmojiPopupConfig,
}

/// 이모지 팝업 설정
///
/// 한자 키가 idle (preedit/조합 비어있음) 상태일 때 이모지 팝업을 트리거한다.
/// 별도 단축키 설정은 제공하지 않는다 — 한자 키 단일 진입점.
/// `enabled=false` 면 idle 한자 키도 silent no-op (한자 변환은 영향 없음).
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct EmojiPopupConfig {
    /// 기능 활성화 여부
    pub enabled: bool,
}

impl Default for EmojiPopupConfig {
    fn default() -> Self {
        Self { enabled: true }
    }
}

impl Default for EngineConfig {
    fn default() -> Self {
        Self {
            default_category: InputCategory::default(),
            mode_sharing: ModeSharingMode::default(),
            korean: KoreanConfig::default(),
            english: EnglishConfig::default(),
            toggle_keys: vec!["Korean".to_string(), "RightAlt".to_string()],
            hanja_keys: vec!["Hanja".to_string(), "F9".to_string()],
            app_rules: Vec::new(),
            auto_typefix: AutoTypeFixConfig::default(),
            auto_english: AutoEnglishConfig::default(),
            emoji_popup: EmojiPopupConfig::default(),
        }
    }
}

/// UNIM 전체 설정
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    /// 엔진 설정
    pub engine: EngineConfig,
    /// 마지막 로드 시점의 파일 수정 시간 (직렬화 제외)
    #[serde(skip)]
    pub last_modified: Option<SystemTime>,
    /// 마지막으로 파일 시스템을 확인한 시간 (Throttling용)
    #[serde(skip)]
    pub last_checked: Option<SystemTime>,
}

impl Config {
    /// 새로운 기본 설정을 생성합니다.
    pub fn new() -> Self {
        Self::default()
    }

    /// 기본 설정 파일 경로를 반환합니다.
    ///
    /// - Linux: `~/.config/unim/config.yaml`
    /// - macOS: `~/Library/Application Support/unim/config.yaml`
    /// - Windows: `%APPDATA%\unim\config.yaml`
    pub fn default_config_path() -> Option<PathBuf> {
        dirs::config_dir().map(|p| p.join("unim").join("config.yaml"))
    }

    /// 기본 경로에서 설정을 로드합니다.
    ///
    /// 설정 파일이 없거나 파싱 실패 시:
    /// - 기본 설정을 생성하고 파일로 저장을 시도합니다.
    /// - 저장 실패 시 (퍼미션 등) 로그로 해결 방법을 안내합니다.
    pub fn load_from_default_path() -> Self {
        let Some(path) = Self::default_config_path() else {
            eprintln!("[UNIM] 설정 디렉터리 경로를 찾을 수 없습니다.");
            return Self::default();
        };

        match Self::load_from_path(&path) {
            Ok(config) => config,
            Err(e) => {
                // 오류 원인에 따라 복구 시도
                Self::handle_load_error(&path, e)
            }
        }
    }

    /// 설정 로드 오류를 처리하고 복구를 시도합니다.
    fn handle_load_error(path: &PathBuf, error: ConfigError) -> Self {
        let mut default_config = Self::default();

        match &error {
            ConfigError::IoError(msg) => {
                if msg.contains("No such file") || msg.contains("찾을 수 없") {
                    // 파일이 없음 - 새로 생성
                    eprintln!(
                        "[UNIM] 설정 파일이 없습니다. 기본 설정을 생성합니다: {:?}",
                        path
                    );
                } else if msg.contains("Permission denied") || msg.contains("권한") {
                    // 퍼미션 문제
                    Self::log_permission_error(path);
                    return default_config;
                } else {
                    eprintln!(
                        "[UNIM] 설정 파일 읽기 오류: {}. 기본 설정을 사용합니다.",
                        msg
                    );
                }
            }
            ConfigError::ParseError(msg) => {
                // 파싱 오류 - 기본 설정으로 덮어쓰기
                eprintln!(
                    "[UNIM] 설정 파일 형식 오류: {}. 기본 설정으로 복구합니다.",
                    msg
                );
            }
            _ => {
                eprintln!(
                    "[UNIM] 설정 로드 오류: {:?}. 기본 설정을 사용합니다.",
                    error
                );
            }
        }

        // 기본 설정 저장 시도
        match default_config.save_to_path(path) {
            Ok(_) => {
                eprintln!("[UNIM] 기본 설정 파일을 생성했습니다: {:?}", path);
                // 저장 후 mtime 갱신
                default_config.last_modified = Self::get_config_mtime(path);
            }
            Err(save_err) => {
                if let ConfigError::IoError(msg) = &save_err {
                    if msg.contains("Permission denied") || msg.contains("권한") {
                        Self::log_permission_error(path);
                    } else {
                        eprintln!("[UNIM] 설정 파일 저장 실패: {}", msg);
                    }
                }
            }
        }

        default_config
    }

    /// 퍼미션 오류 시 해결 방법을 로그로 안내합니다.
    fn log_permission_error(path: &PathBuf) {
        eprintln!("[UNIM] ⚠️ 설정 파일 접근 권한 오류: {:?}", path);
        eprintln!("[UNIM] 다음 명령어로 해결할 수 있습니다:");
        if let Some(parent) = path.parent() {
            eprintln!("[UNIM]   mkdir -p {:?} && chmod 755 {:?}", parent, parent);
        }
        eprintln!("[UNIM]   touch {:?} && chmod 644 {:?}", path, path);
        eprintln!("[UNIM] 또는 관리자 권한으로 설정 도구를 실행하세요:");
        eprintln!("[UNIM]   unim-cli config");
    }

    /// 지정된 경로에서 설정을 로드합니다.
    ///
    /// # Arguments
    ///
    /// * `path` - 설정 파일 경로
    ///
    /// # Returns
    ///
    /// 로드된 설정 또는 오류
    pub fn load_from_path(path: &PathBuf) -> Result<Self, ConfigError> {
        let metadata = fs::metadata(path).map_err(|e| ConfigError::IoError(e.to_string()))?;
        let mtime = metadata.modified().ok();

        let content = fs::read_to_string(path).map_err(|e| ConfigError::IoError(e.to_string()))?;
        let mut config: Self =
            serde_yaml::from_str(&content).map_err(|e| ConfigError::ParseError(e.to_string()))?;
        // 구(舊) time_window_ms 필드 역호환: forward/reverse에 주입 + 범위 clamp.
        config.engine.auto_typefix.clamp_ranges();
        config.last_modified = mtime;
        config.last_checked = Some(SystemTime::now());
        Ok(config)
    }

    /// 설정을 기본 경로에 저장합니다.
    pub fn save_to_default_path(&self) -> Result<(), ConfigError> {
        let path = Self::default_config_path()
            .ok_or_else(|| ConfigError::IoError("설정 디렉터리를 찾을 수 없습니다.".to_string()))?;
        self.save_to_path(&path)
    }

    /// 설정을 지정된 경로에 저장합니다.
    ///
    /// # Arguments
    ///
    /// * `path` - 저장할 경로
    pub fn save_to_path(&self, path: &PathBuf) -> Result<(), ConfigError> {
        // 디렉터리 생성
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|e| ConfigError::IoError(e.to_string()))?;
        }

        let content =
            serde_yaml::to_string(self).map_err(|e| ConfigError::SerializeError(e.to_string()))?;
        fs::write(path, content).map_err(|e| ConfigError::IoError(e.to_string()))
    }

    /// 설정 파일이 변경되어 다시 로드가 필요한지 확인합니다.
    ///
    /// # Returns
    ///
    /// 재로드가 필요하면 true
    pub fn needs_reload(&self) -> bool {
        let Some(path) = Self::default_config_path() else {
            return false;
        };

        let Some(current_mtime) = Self::get_config_mtime(&path) else {
            return false;
        };

        match self.last_modified {
            Some(saved_mtime) => current_mtime > saved_mtime,
            None => true, // mtime이 없으면 항상 reload
        }
    }

    /// 설정 파일이 변경되었으면 다시 로드합니다. (2초 throttling 적용)
    ///
    /// 리로드 실패 시 현재 설정을 유지하고 로그를 출력합니다.
    ///
    /// # Returns
    ///
    /// 재로드 성공 시 true
    pub fn reload_if_changed(&mut self) -> bool {
        let now = SystemTime::now();

        // 2초 이내에 이미 확인했다면 건너뜀 (성능 최적화)
        if let Some(last) = self.last_checked {
            if let Ok(duration) = now.duration_since(last) {
                if duration.as_secs() < 2 {
                    return false;
                }
            }
        }

        self.last_checked = Some(now);

        if !self.needs_reload() {
            return false;
        }

        let Some(path) = Self::default_config_path() else {
            return false;
        };

        match Self::load_from_path(&path) {
            Ok(new_config) => {
                let last_checked = self.last_checked;
                *self = new_config;
                self.last_checked = last_checked;
                true
            }
            Err(e) => {
                // 리로드 실패 시 현재 설정 유지
                eprintln!("[UNIM] 설정 리로드 실패: {:?}. 현재 설정을 유지합니다.", e);
                false
            }
        }
    }

    /// 설정 파일 존재 및 유효성을 보장합니다.
    ///
    /// 파일이 없거나 유효하지 않으면 기본 설정으로 생성합니다.
    ///
    /// # Returns
    ///
    /// 설정 파일 경로 (성공 시)
    pub fn ensure_config_file() -> Option<PathBuf> {
        let path = Self::default_config_path()?;

        // 파일 유효성 확인
        if Self::load_from_path(&path).is_err() {
            // 기본 설정 저장
            let default_config = Self::default();
            if default_config.save_to_path(&path).is_ok() {
                eprintln!("[UNIM] 설정 파일을 초기화했습니다: {:?}", path);
            }
        }

        Some(path)
    }

    /// 파일의 수정 시간을 가져옵니다.
    fn get_config_mtime(path: &PathBuf) -> Option<SystemTime> {
        fs::metadata(path).ok()?.modified().ok()
    }
}

/// 설정 관련 오류
#[derive(Clone, Debug)]
pub enum ConfigError {
    /// IO 오류
    IoError(String),
    /// 파싱 오류
    ParseError(String),
    /// 직렬화 오류
    SerializeError(String),
}

impl std::fmt::Display for ConfigError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ConfigError::IoError(msg) => write!(f, "IO 오류: {}", msg),
            ConfigError::ParseError(msg) => write!(f, "파싱 오류: {}", msg),
            ConfigError::SerializeError(msg) => write!(f, "직렬화 오류: {}", msg),
        }
    }
}

impl std::error::Error for ConfigError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = Config::default();
        assert_eq!(config.engine.default_category, InputCategory::English);
        assert_eq!(config.engine.korean.layout, KOREAN_LAYOUT_DUBEOLSIK);
        assert_eq!(config.engine.english.layout, "qwerty");
    }

    #[test]
    fn normalize_korean_layout_name_handles_legacy_and_aliases() {
        // 레거시 enum 이름
        assert_eq!(normalize_korean_layout_name("Dubeolsik"), "ko_2bulstd");
        assert_eq!(normalize_korean_layout_name("Sebeolsik390"), "ko_3bul390");
        // 축약 별칭
        assert_eq!(normalize_korean_layout_name("2bul"), "ko_2bulstd");
        assert_eq!(normalize_korean_layout_name("390"), "ko_3bul390");
        assert_eq!(
            normalize_korean_layout_name("3bul_noshift"),
            "ko_3bul_noshift"
        );
        // 사용자 프로필은 pass-through (쿼티형 세벌식도 0.2.0+에서는 사용자 프로필 경로)
        assert_eq!(
            normalize_korean_layout_name("ko_3bul_qwerty"),
            "ko_3bul_qwerty"
        );
        assert_eq!(normalize_korean_layout_name("my_custom"), "my_custom");
    }

    #[test]
    fn is_sebeolsik_layout_classifies_correctly() {
        assert!(!is_sebeolsik_layout("ko_2bulstd"));
        assert!(!is_sebeolsik_layout("Dubeolsik"));
        assert!(is_sebeolsik_layout("ko_3bul390"));
        assert!(is_sebeolsik_layout("Sebeolsik391"));
        assert!(is_sebeolsik_layout("3bul_noshift"));
        // ko_3bul_qwerty는 빌트인 아니지만 prefix 매칭으로 세벌식 계열 분류됨 (사용자 프로필)
        assert!(is_sebeolsik_layout("ko_3bul_qwerty"));
        // 안마태 자판 — ko_anmatae / anmatae / anmatae_2003 모두 세벌식으로 분류
        assert!(is_sebeolsik_layout("ko_anmatae"));
        assert!(is_sebeolsik_layout("anmatae"));
        assert!(is_sebeolsik_layout("anmatae_2003"));
    }

    #[test]
    fn effective_layout_name_defaults_to_builtin() {
        let kc = KoreanConfig::default();
        assert_eq!(kc.effective_layout_name(), "ko_2bulstd");
    }

    #[test]
    fn effective_layout_name_normalizes_legacy_value() {
        let mut kc = KoreanConfig::default();
        kc.layout = "Dubeolsik".to_string();
        assert_eq!(kc.effective_layout_name(), "ko_2bulstd");
    }

    #[test]
    fn effective_layout_name_accepts_user_profile() {
        let mut kc = KoreanConfig::default();
        kc.layout = "my_custom_layout".to_string();
        assert_eq!(kc.effective_layout_name(), "my_custom_layout");
    }

    #[test]
    fn korean_config_serde_roundtrips_rule_sets() {
        let mut kc = KoreanConfig::default();
        kc.layout = "ko_3bul390".to_string();
        kc.active_rule_sets = Some(vec!["set_a".to_string(), "set_b".to_string()]);
        let yaml = serde_yaml::to_string(&kc).unwrap();
        let back: KoreanConfig = serde_yaml::from_str(&yaml).unwrap();
        assert_eq!(back.layout, "ko_3bul390");
        assert_eq!(
            back.active_rule_sets,
            Some(vec!["set_a".to_string(), "set_b".to_string()])
        );
    }

    /// `Some(vec![])` = "사용자가 모든 규칙 세트를 끔" 의도가 round-trip에서 보존되어야 한다.
    /// (이전 Vec<String> 시절에는 빈 vec → 프로필 기본값 fallback이라 의도가 손실되던 버그.)
    #[test]
    fn korean_config_serde_roundtrips_explicit_all_off() {
        let mut kc = KoreanConfig::default();
        kc.active_rule_sets = Some(Vec::new());
        let yaml = serde_yaml::to_string(&kc).unwrap();
        let back: KoreanConfig = serde_yaml::from_str(&yaml).unwrap();
        assert_eq!(
            back.active_rule_sets,
            Some(Vec::<String>::new()),
            "Some(vec![])는 명시적 All OFF 의도, round-trip 보존 필수"
        );
    }

    /// `None`은 직렬화 시 YAML 필드 자체가 누락된다(`skip_serializing_if`).
    #[test]
    fn korean_config_none_skips_field_in_yaml() {
        let kc = KoreanConfig::default();
        assert!(kc.active_rule_sets.is_none());
        let yaml = serde_yaml::to_string(&kc).unwrap();
        assert!(
            !yaml.contains("active_rule_sets"),
            "None일 때 active_rule_sets 키가 직렬화 결과에 없어야 함: {yaml}"
        );
        let back: KoreanConfig = serde_yaml::from_str(&yaml).unwrap();
        assert_eq!(back.active_rule_sets, None);
    }

    /// 레거시 YAML(필드 부재) → `None` (프로필 기본값 사용).
    #[test]
    fn korean_config_legacy_missing_field_is_none() {
        let yaml = r#"
layout: ko_2bulstd
preedit_johab: false
word_commit: false
"#;
        let kc: KoreanConfig = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(kc.active_rule_sets, None);
    }

    // ─────────────────────────────────────────────
    // layout_rule_sets 캐시 동작 — switch_layout / cache_active_rule_sets
    // ─────────────────────────────────────────────

    #[test]
    fn switch_layout_caches_old_and_restores_new() {
        // A에서 토글 후 B로 전환 → A의 상태가 캐시에 보존되어야 한다.
        let mut kc = KoreanConfig::default();
        kc.layout = "ko_2bulstd".to_string();
        kc.active_rule_sets = Some(vec!["a_set".to_string()]);
        kc.switch_layout("ko_3bul390", None);

        assert_eq!(kc.layout, "ko_3bul390");
        // 새 자판은 캐시 없으므로 None (프로필 기본값 사용).
        assert_eq!(kc.active_rule_sets, None);
        // 이전 자판 상태는 캐시에 보존.
        assert_eq!(
            kc.layout_rule_sets.get("ko_2bulstd"),
            Some(&vec!["a_set".to_string()])
        );
    }

    #[test]
    fn switch_layout_restores_cached_state_on_return() {
        // A → B → A 왕복 시 A의 상태가 복원되어야 한다.
        let mut kc = KoreanConfig::default();
        kc.layout = "ko_2bulstd".to_string();
        kc.active_rule_sets = Some(vec!["a_set".to_string()]);
        kc.switch_layout("ko_3bul390", None);
        // B에서 다른 상태로 토글.
        kc.active_rule_sets = Some(vec!["b_set".to_string()]);
        kc.cache_active_rule_sets();
        // 다시 A로.
        kc.switch_layout("ko_2bulstd", None);

        assert_eq!(kc.layout, "ko_2bulstd");
        assert_eq!(
            kc.active_rule_sets,
            Some(vec!["a_set".to_string()]),
            "A로 돌아왔을 때 A의 활성 룰셋 상태가 복원되어야 함"
        );
        // B의 캐시도 보존되어야 함 (다음에 B로 갈 때 사용).
        assert_eq!(
            kc.layout_rule_sets.get("ko_3bul390"),
            Some(&vec!["b_set".to_string()])
        );
    }

    #[test]
    fn switch_layout_preserves_explicit_all_off() {
        // Some(vec![]) (명시 All OFF) 의도가 캐시 → 복원 시 보존되어야 한다.
        let mut kc = KoreanConfig::default();
        kc.layout = "ko_2bulstd".to_string();
        kc.active_rule_sets = Some(Vec::new()); // 명시 All OFF
        kc.switch_layout("ko_3bul390", None);
        kc.switch_layout("ko_2bulstd", None);

        assert_eq!(
            kc.active_rule_sets,
            Some(Vec::<String>::new()),
            "명시 All OFF 의도가 자판 왕복 후에도 보존"
        );
    }

    #[test]
    fn switch_layout_does_not_cache_none() {
        // None(프로필 기본값 사용)은 캐시하지 않는다 — 캐시되면 다음에 None을 못 만듦.
        let mut kc = KoreanConfig::default();
        kc.layout = "ko_2bulstd".to_string();
        kc.active_rule_sets = None;
        kc.switch_layout("ko_3bul390", None);

        assert!(
            kc.layout_rule_sets.is_empty(),
            "None은 캐시되지 않아야 함 (의미상 '캐시 없음 = 프로필 기본')"
        );
    }

    #[test]
    fn switch_layout_same_name_is_noop() {
        let mut kc = KoreanConfig::default();
        kc.layout = "ko_2bulstd".to_string();
        kc.active_rule_sets = Some(vec!["x".to_string()]);
        let before = kc.layout_rule_sets.clone();
        kc.switch_layout("ko_2bulstd", None);
        assert_eq!(kc.layout_rule_sets, before);
        assert_eq!(kc.active_rule_sets, Some(vec!["x".to_string()]));
    }

    #[test]
    fn switch_layout_filters_stale_rule_set_names() {
        // 캐시된 이름 중 새 프로필에 없는 것은 정리.
        let mut kc = KoreanConfig::default();
        kc.layout = "ko_2bulstd".to_string();
        kc.layout_rule_sets.insert(
            "ko_3bul390".to_string(),
            vec!["valid".to_string(), "stale".to_string()],
        );
        let valid_names = vec!["valid".to_string(), "other".to_string()];
        kc.switch_layout("ko_3bul390", Some(&valid_names));
        assert_eq!(kc.active_rule_sets, Some(vec!["valid".to_string()]));
    }

    #[test]
    fn cache_active_rule_sets_writes_some_removes_none() {
        let mut kc = KoreanConfig::default();
        kc.layout = "ko_2bulstd".to_string();
        kc.active_rule_sets = Some(vec!["x".to_string()]);
        kc.cache_active_rule_sets();
        assert_eq!(
            kc.layout_rule_sets.get("ko_2bulstd"),
            Some(&vec!["x".to_string()])
        );
        // None으로 되돌리면 캐시 entry 제거.
        kc.active_rule_sets = None;
        kc.cache_active_rule_sets();
        assert!(!kc.layout_rule_sets.contains_key("ko_2bulstd"));
    }

    #[test]
    fn korean_config_serde_roundtrips_layout_rule_sets() {
        let mut kc = KoreanConfig::default();
        kc.layout = "ko_3bul390".to_string();
        kc.layout_rule_sets
            .insert("ko_2bulstd".to_string(), vec!["a".to_string()]);
        kc.layout_rule_sets
            .insert("ko_3bul390".to_string(), vec!["b".to_string(), "c".to_string()]);
        let yaml = serde_yaml::to_string(&kc).unwrap();
        let back: KoreanConfig = serde_yaml::from_str(&yaml).unwrap();
        assert_eq!(back.layout_rule_sets.len(), 2);
        assert_eq!(
            back.layout_rule_sets.get("ko_2bulstd"),
            Some(&vec!["a".to_string()])
        );
    }

    #[test]
    fn korean_config_empty_layout_rule_sets_skips_field_in_yaml() {
        let kc = KoreanConfig::default();
        assert!(kc.layout_rule_sets.is_empty());
        let yaml = serde_yaml::to_string(&kc).unwrap();
        assert!(
            !yaml.contains("layout_rule_sets"),
            "빈 BTreeMap일 때 YAML에 layout_rule_sets 키 없어야 함: {yaml}"
        );
    }

    /// 명시적 빈 list (`active_rule_sets: []`)는 `Some(vec![])`로 로드되어
    /// "모두 OFF" 의도로 해석된다.
    #[test]
    fn korean_config_explicit_empty_list_is_some_empty() {
        let yaml = r#"
layout: ko_2bulstd
preedit_johab: false
word_commit: false
active_rule_sets: []
"#;
        let kc: KoreanConfig = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(kc.active_rule_sets, Some(Vec::<String>::new()));
    }

    #[test]
    fn korean_config_legacy_yaml_enum_variant_normalized() {
        // Phase 4 이전 포맷 (`layout: Dubeolsik`)도 로드 가능 — 자동 정식 이름 승격.
        let yaml = r#"
layout: Dubeolsik
preedit_johab: false
word_commit: false
"#;
        let kc: KoreanConfig = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(kc.layout, "ko_2bulstd");
        assert_eq!(kc.active_rule_sets, None);
        assert_eq!(kc.effective_layout_name(), "ko_2bulstd");
    }

    #[test]
    fn korean_config_legacy_custom_layout_overrides_layout() {
        // Phase 4b 포맷 (`layout: Dubeolsik, custom_layout: "my_x"`) 흡수.
        let yaml = r#"
layout: Dubeolsik
custom_layout: my_custom
preedit_johab: false
word_commit: false
"#;
        let kc: KoreanConfig = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(
            kc.layout, "my_custom",
            "custom_layout이 Some이면 layout을 덮어씀"
        );
    }

    #[test]
    fn test_input_category() {
        let korean = InputCategory::Korean;
        let english = InputCategory::English;
        assert_ne!(korean, english);
    }

    // === Config 직렬화/역직렬화 테스트 ===

    #[test]
    fn test_config_serialize_deserialize() {
        let config = Config::default();
        let yaml = serde_yaml::to_string(&config).unwrap();
        let deserialized: Config = serde_yaml::from_str(&yaml).unwrap();
        assert_eq!(
            deserialized.engine.default_category,
            config.engine.default_category
        );
        assert_eq!(
            deserialized.engine.korean.layout,
            config.engine.korean.layout
        );
        assert_eq!(
            deserialized.engine.english.layout,
            config.engine.english.layout
        );
        assert_eq!(deserialized.engine.mode_sharing, config.engine.mode_sharing);
    }

    #[test]
    fn test_config_save_and_load() {
        let config = Config::default();
        let dir = std::env::temp_dir().join("unim_test_config");
        let path = dir.join("test_config.yaml");

        config.save_to_path(&path).unwrap();
        let loaded = Config::load_from_path(&path).unwrap();

        assert_eq!(loaded.engine.korean.layout, config.engine.korean.layout);
        assert_eq!(loaded.engine.toggle_keys, config.engine.toggle_keys);

        // 정리
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_config_load_nonexistent() {
        let path = PathBuf::from("/tmp/unim_nonexistent_config_12345.yaml");
        let result = Config::load_from_path(&path);
        assert!(result.is_err());
    }

    #[test]
    fn test_config_load_invalid_yaml() {
        let dir = std::env::temp_dir().join("unim_test_invalid");
        let path = dir.join("bad_config.yaml");
        fs::create_dir_all(&dir).unwrap();
        fs::write(&path, "{{{{invalid yaml").unwrap();

        let result = Config::load_from_path(&path);
        assert!(result.is_err());

        let _ = std::fs::remove_dir_all(&dir);
    }

    // === Config needs_reload 테스트 ===

    #[test]
    fn test_config_needs_reload_no_mtime() {
        let config = Config {
            last_modified: None,
            ..Config::default()
        };
        // mtime이 None이면 reload 필요 (파일이 존재할 경우)
        // 파일 경로가 없으면 false 반환
        let _ = config.needs_reload();
    }

    // === 레이아웃 enum 테스트 ===

    #[test]
    fn test_korean_layout_builtins() {
        let all = KOREAN_LAYOUT_BUILTINS;
        assert_eq!(all.len(), 4);
        assert!(all.contains(&"ko_2bulstd"));
        assert!(all.contains(&"ko_3bul390"));
        assert!(all.contains(&"ko_3bul391"));
        assert!(all.contains(&"ko_3bul_noshift"));
        // 0.2.0+: ko_3bul_qwerty는 빌트인 아님 (연구 자료로만 보존)
        assert!(!all.contains(&"ko_3bul_qwerty"));
    }

    #[test]
    fn test_english_layout_builtins() {
        let all = ENGLISH_LAYOUT_BUILTINS;
        assert_eq!(all.len(), 5);
        assert!(all.contains(&"qwerty"));
        assert!(all.contains(&"colemak_dh"));
    }

    #[test]
    fn test_english_layout_keymap_name() {
        assert_eq!(english_layout_keymap_name("dvorak"), "en_dvorak");
        assert_eq!(english_layout_keymap_name("colemak"), "en_colemak");
        assert_eq!(english_layout_keymap_name("colemak_dh"), "en_colemak_dh");
        assert_eq!(english_layout_keymap_name("workman"), "en_workman");
        // 레거시 enum 이름 → 소문자 승격
        assert_eq!(english_layout_keymap_name("Dvorak"), "en_dvorak");
    }

    #[test]
    fn test_english_layout_top_row_labels() {
        assert_eq!(english_layout_top_row_labels("qwerty"), "QWERTYUIO");
        assert_eq!(english_layout_top_row_labels("dvorak"), "',.PYFGCR");
        assert_eq!(english_layout_top_row_labels("colemak"), "QWFPGJLUY");
    }

    #[test]
    fn normalize_english_layout_name_handles_legacy() {
        assert_eq!(normalize_english_layout_name("Qwerty"), "qwerty");
        assert_eq!(normalize_english_layout_name("ColemakDh"), "colemak_dh");
        assert_eq!(normalize_english_layout_name("colemak-dh"), "colemak_dh");
        assert_eq!(normalize_english_layout_name("workman"), "workman");
        // 사용자 프로필 pass-through
        assert_eq!(normalize_english_layout_name("my_dvorak"), "my_dvorak");
    }

    #[test]
    fn english_config_legacy_yaml_enum_variant_normalized() {
        // 레거시 `layout: Qwerty` 포맷 로드 → 정식 소문자 이름으로 승격.
        let yaml = r#"
layout: Qwerty
preferred_direct: true
"#;
        let ec: EnglishConfig = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(ec.layout, "qwerty");

        let yaml2 = r#"
layout: ColemakDh
preferred_direct: false
"#;
        let ec2: EnglishConfig = serde_yaml::from_str(yaml2).unwrap();
        assert_eq!(ec2.layout, "colemak_dh");
    }

    #[test]
    fn test_mode_sharing_all() {
        let all = ModeSharingMode::all();
        assert_eq!(all.len(), 2);
    }

    #[test]
    fn test_mode_sharing_display_name() {
        assert!(!ModeSharingMode::Global.display_name().is_empty());
        assert!(!ModeSharingMode::PerApp.display_name().is_empty());
    }

    // === EngineConfig 기본값 테스트 ===

    #[test]
    fn test_engine_config_defaults() {
        let config = EngineConfig::default();
        assert_eq!(config.toggle_keys, vec!["Korean", "RightAlt"]);
        assert_eq!(config.hanja_keys, vec!["Hanja", "F9"]);
        assert_eq!(config.mode_sharing, ModeSharingMode::Global);
    }

    // === Custom config 직렬화 테스트 ===

    #[test]
    fn test_config_custom_values() {
        let mut config = Config::default();
        config.engine.korean.layout = "ko_3bul390".to_string();
        config.engine.english.layout = "dvorak".to_string();
        config.engine.mode_sharing = ModeSharingMode::PerApp;

        let yaml = serde_yaml::to_string(&config).unwrap();
        let loaded: Config = serde_yaml::from_str(&yaml).unwrap();

        assert_eq!(loaded.engine.korean.layout, "ko_3bul390");
        assert_eq!(loaded.engine.english.layout, "dvorak");
        assert_eq!(loaded.engine.mode_sharing, ModeSharingMode::PerApp);
    }

    // === AutoTypeFix 기본값/범위 테스트 ===

    #[test]
    fn test_auto_typefix_defaults() {
        let c = AutoTypeFixConfig::default();
        assert!(c.enabled);
        assert_eq!(c.forward_time_window_ms, 5000);
        assert_eq!(c.reverse_time_window_ms, 5000);
        assert_eq!(c.kor_syllable_threshold, 2);
        assert_eq!(c.eng_word_min_length, 5);
        assert!(c.forward);
        assert!(c.reverse);
        assert!(c.skip_on_english_word);
        assert!(c.skip_on_complete_syllable);
        assert!(c.rollback_detection);
        assert_eq!(c.tentative_expiry_hours, 4);
        assert_eq!(c.observation_timeout_secs, 10);
    }

    #[test]
    fn test_auto_typefix_clamp_expiry() {
        let mut c = AutoTypeFixConfig {
            tentative_expiry_hours: 500,
            ..Default::default()
        };
        c.clamp_ranges();
        assert_eq!(c.tentative_expiry_hours, AUTO_TYPEFIX_TENTATIVE_EXPIRY_MAX);

        let mut c = AutoTypeFixConfig {
            tentative_expiry_hours: 0,
            ..Default::default()
        };
        c.clamp_ranges();
        assert_eq!(c.tentative_expiry_hours, AUTO_TYPEFIX_TENTATIVE_EXPIRY_MIN);

        let mut c = AutoTypeFixConfig {
            observation_timeout_secs: 99,
            ..Default::default()
        };
        c.clamp_ranges();
        assert_eq!(
            c.observation_timeout_secs,
            AUTO_TYPEFIX_OBSERVATION_TIMEOUT_MAX
        );

        let mut c = AutoTypeFixConfig {
            observation_timeout_secs: 0,
            ..Default::default()
        };
        c.clamp_ranges();
        assert_eq!(
            c.observation_timeout_secs,
            AUTO_TYPEFIX_OBSERVATION_TIMEOUT_MIN
        );
    }

    #[test]
    fn test_auto_typefix_clamp() {
        let mut c = AutoTypeFixConfig {
            kor_syllable_threshold: 10,
            eng_word_min_length: 1,
            forward_time_window_ms: 100,
            reverse_time_window_ms: 100,
            ..Default::default()
        };
        c.clamp_ranges();
        assert_eq!(c.kor_syllable_threshold, AUTO_TYPEFIX_KOR_THRESHOLD_MAX);
        assert_eq!(c.eng_word_min_length, AUTO_TYPEFIX_ENG_MIN_LENGTH_MIN);
        assert_eq!(c.forward_time_window_ms, AUTO_TYPEFIX_TIME_WINDOW_MIN);
        assert_eq!(c.reverse_time_window_ms, AUTO_TYPEFIX_TIME_WINDOW_MIN);

        let mut c2 = AutoTypeFixConfig {
            kor_syllable_threshold: 0,
            eng_word_min_length: 99,
            forward_time_window_ms: 99999,
            reverse_time_window_ms: 99999,
            ..Default::default()
        };
        c2.clamp_ranges();
        assert_eq!(c2.kor_syllable_threshold, AUTO_TYPEFIX_KOR_THRESHOLD_MIN);
        assert_eq!(c2.eng_word_min_length, AUTO_TYPEFIX_ENG_MIN_LENGTH_MAX);
        assert_eq!(c2.forward_time_window_ms, AUTO_TYPEFIX_TIME_WINDOW_MAX);
        assert_eq!(c2.reverse_time_window_ms, AUTO_TYPEFIX_TIME_WINDOW_MAX);
    }

    /// 구(旧) config.yaml (신규 필드 없음) 파싱 역호환성 검증.
    #[test]
    fn test_legacy_yaml_backcompat_autotypefix() {
        let legacy = r#"
engine:
  auto_typefix:
    enabled: true
    time_window_ms: 3000
    kor_syllable_threshold: 2
    eng_word_min_length: 5
    forward: true
    reverse: true
"#;
        let mut cfg: Config = serde_yaml::from_str(legacy).expect("legacy yaml must parse");
        // 구 필드가 forward/reverse 두 신 필드로 주입되도록 clamp_ranges 호출.
        cfg.engine.auto_typefix.clamp_ranges();
        // 누락 필드는 serde default로 채워져야 함.
        assert!(cfg.engine.auto_typefix.skip_on_english_word);
        assert!(cfg.engine.auto_typefix.skip_on_complete_syllable);
        // 구 time_window_ms 값이 forward/reverse 양쪽에 주입되어야 함.
        assert_eq!(cfg.engine.auto_typefix.forward_time_window_ms, 3000);
        assert_eq!(cfg.engine.auto_typefix.reverse_time_window_ms, 3000);
        assert!(cfg.engine.auto_typefix.time_window_ms.is_none());
    }

    /// 새 필드(forward/reverse)만 있는 yaml 파싱.
    #[test]
    fn test_new_yaml_separate_time_windows() {
        let yaml = r#"
engine:
  auto_typefix:
    forward_time_window_ms: 2500
    reverse_time_window_ms: 4000
"#;
        let mut cfg: Config = serde_yaml::from_str(yaml).unwrap();
        cfg.engine.auto_typefix.clamp_ranges();
        assert_eq!(cfg.engine.auto_typefix.forward_time_window_ms, 2500);
        assert_eq!(cfg.engine.auto_typefix.reverse_time_window_ms, 4000);
    }

    /// 구 time_window_ms + 신 forward_time_window_ms 혼재 시 신 필드 우선.
    #[test]
    fn test_mixed_yaml_new_field_wins() {
        let yaml = r#"
engine:
  auto_typefix:
    time_window_ms: 1500
    forward_time_window_ms: 3500
"#;
        let mut cfg: Config = serde_yaml::from_str(yaml).unwrap();
        cfg.engine.auto_typefix.clamp_ranges();
        assert_eq!(cfg.engine.auto_typefix.forward_time_window_ms, 3500);
        // reverse는 신 필드 미지정 → 기본값이었으므로 legacy 값(1500) 주입.
        assert_eq!(cfg.engine.auto_typefix.reverse_time_window_ms, 1500);
    }

    #[test]
    fn test_empty_yaml_full_defaults() {
        // 완전히 빈 YAML에서도 모든 기본값이 채워져야 함.
        let cfg: Config = serde_yaml::from_str("{}").unwrap();
        assert!(cfg.engine.auto_typefix.skip_on_english_word);
        assert!(cfg.engine.auto_typefix.skip_on_complete_syllable);
    }

    /// 제거된 필드(auto_switch, manual_shortcuts)가 포함된 구 yaml도
    /// 파싱 실패 없이 무시되어야 한다 (serde_yaml 기본 동작: unknown field ignore).
    #[test]
    fn test_legacy_yaml_removed_fields_ignored() {
        let yaml = "engine:\n  auto_switch:\n    enabled: true\n    threshold: 0.7\n  manual_shortcuts:\n    forward: ['<Super>k']\n    reverse: ['<Shift><Super>k']\n";
        let _: crate::config::Config =
            serde_yaml::from_str(yaml).expect("legacy yaml must still parse");
    }

    // === AutoEnglish 설정 테스트 ===

    #[test]
    fn test_auto_english_defaults() {
        let c = AutoEnglishConfig::default();
        assert!(!c.enabled, "기본은 비활성 (opt-in)");
        assert_eq!(
            c.trigger_keys,
            vec!["key:Escape", "char:/"],
            "기본 트리거 키는 key:Escape / char:/ (비-QWERTY 한국어 안전)"
        );
    }

    #[test]
    fn test_auto_english_serde_roundtrip() {
        let mut cfg = Config::default();
        cfg.engine.auto_english.enabled = true;
        cfg.engine.auto_english.trigger_keys = vec!["Escape".into(), "Period".into()];

        let yaml = serde_yaml::to_string(&cfg).unwrap();
        let loaded: Config = serde_yaml::from_str(&yaml).unwrap();
        assert!(loaded.engine.auto_english.enabled);
        assert_eq!(
            loaded.engine.auto_english.trigger_keys,
            vec!["Escape", "Period"]
        );
    }

    /// 구 YAML에 `auto_english` 섹션이 없어도 기본값으로 채워져야 한다.
    #[test]
    fn test_auto_english_serde_backcompat() {
        let legacy = r#"
engine:
  auto_typefix:
    enabled: true
"#;
        let cfg: Config = serde_yaml::from_str(legacy).expect("legacy yaml must parse");
        assert!(!cfg.engine.auto_english.enabled);
        assert_eq!(
            cfg.engine.auto_english.trigger_keys,
            vec!["key:Escape", "char:/"]
        );
    }

    // === ConfigError Display 테스트 ===

    #[test]
    fn test_config_error_display() {
        let e = ConfigError::IoError("test".to_string());
        assert!(e.to_string().contains("test"));

        let e = ConfigError::ParseError("bad".to_string());
        assert!(e.to_string().contains("bad"));

        let e = ConfigError::SerializeError("err".to_string());
        assert!(e.to_string().contains("err"));
    }
}
