//! 설정 모듈
//!
//! UNIM 입력기의 설정을 관리합니다.

use serde::{Deserialize, Serialize};
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
        &[
            ModeSharingMode::Global,
            ModeSharingMode::PerApp,
        ]
    }
}

/// 한국어 키보드 레이아웃
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default, Serialize, Deserialize)]
#[repr(u32)]
pub enum KoreanLayout {
    /// 두벌식 표준
    #[default]
    Dubeolsik = 0,
    /// 세벌식 390
    Sebeolsik390 = 1,
    /// 세벌식 최종
    Sebeolsik391 = 2,
    /// 세벌식 순아래 (No-Shift)
    SebeolsikNoShift = 3,
}

impl KoreanLayout {
    /// 레이아웃 이름을 반환합니다.
    pub fn name(&self) -> &'static str {
        match self {
            KoreanLayout::Dubeolsik => "2bul",
            KoreanLayout::Sebeolsik390 => "3bul390",
            KoreanLayout::Sebeolsik391 => "3bul391",
            KoreanLayout::SebeolsikNoShift => "3bul_noshift",
        }
    }

    /// 세벌식 레이아웃인지 확인합니다.
    pub fn is_sebeolsik(&self) -> bool {
        matches!(
            self,
            KoreanLayout::Sebeolsik390
                | KoreanLayout::Sebeolsik391
                | KoreanLayout::SebeolsikNoShift
        )
    }

    /// 표시용 레이블을 반환합니다.
    pub fn display_name(&self) -> &'static str {
        match self {
            KoreanLayout::Dubeolsik => "두벌식 표준",
            KoreanLayout::Sebeolsik390 => "세벌식 390",
            KoreanLayout::Sebeolsik391 => "세벌식 최종",
            KoreanLayout::SebeolsikNoShift => "세벌식 순아래",
        }
    }

    /// 사용 가능한 모든 레이아웃을 반환합니다.
    pub fn all() -> &'static [KoreanLayout] {
        &[
            KoreanLayout::Dubeolsik,
            KoreanLayout::Sebeolsik390,
            KoreanLayout::Sebeolsik391,
            KoreanLayout::SebeolsikNoShift,
        ]
    }
}

/// 영어 키보드 레이아웃
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
#[repr(u32)]
pub enum EnglishLayout {
    /// QWERTY
    #[default]
    Qwerty = 0,
    /// Dvorak
    Dvorak = 1,
    /// Colemak
    Colemak = 2,
    /// Colemak-DH (Colemak의 인체공학적 개선 버전)
    ColemakDh = 3,
    /// Workman
    Workman = 4,
}

impl EnglishLayout {
    /// 레이아웃 이름을 반환합니다.
    pub fn name(&self) -> &'static str {
        match self {
            EnglishLayout::Qwerty => "qwerty",
            EnglishLayout::Dvorak => "dvorak",
            EnglishLayout::Colemak => "colemak",
            EnglishLayout::ColemakDh => "colemak_dh",
            EnglishLayout::Workman => "workman",
        }
    }

    /// Keymap JSON 파일명을 반환합니다.
    ///
    /// # Returns
    ///
    /// 해당 레이아웃의 keymap JSON 파일 식별자 (예: "en_qwerty", "en_dvorak")
    pub fn keymap_name(&self) -> &'static str {
        match self {
            EnglishLayout::Qwerty => "en_qwerty",
            EnglishLayout::Dvorak => "en_dvorak",
            EnglishLayout::Colemak => "en_colemak",
            EnglishLayout::ColemakDh => "en_colemak_dh",
            EnglishLayout::Workman => "en_workman",
        }
    }

    /// 표시용 레이블을 반환합니다.
    pub fn display_name(&self) -> &'static str {
        match self {
            EnglishLayout::Qwerty => "QWERTY",
            EnglishLayout::Dvorak => "Dvorak",
            EnglishLayout::Colemak => "Colemak",
            EnglishLayout::ColemakDh => "Colemak-DH",
            EnglishLayout::Workman => "Workman",
        }
    }

    /// 사용 가능한 모든 레이아웃을 반환합니다.
    pub fn all() -> &'static [EnglishLayout] {
        &[
            EnglishLayout::Qwerty,
            EnglishLayout::Dvorak,
            EnglishLayout::Colemak,
            EnglishLayout::ColemakDh,
            EnglishLayout::Workman,
        ]
    }

    /// 상단 행(2nd row)의 앞 9개 키 레이블을 반환합니다.
    ///
    /// 특수문자 팝업의 열 헤더 및 키 매핑에 사용됩니다.
    pub fn top_row_labels(&self) -> &'static str {
        match self {
            EnglishLayout::Qwerty => "QWERTYUIO",
            EnglishLayout::Dvorak => "',.PYFGCR", // 드보락 상단 행 앞 9개 (논리 문자 기준)
            EnglishLayout::Colemak => "QWFPGJLUY",
            EnglishLayout::ColemakDh => "QWFPBJLUY",
            EnglishLayout::Workman => "QDRWBJFUP",
        }
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

/// 한국어 엔진 설정
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct KoreanConfig {
    /// 한국어 키보드 레이아웃
    pub layout: KoreanLayout,
}

/// 영어 엔진 설정
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct EnglishConfig {
    /// 영어 키보드 레이아웃
    pub layout: EnglishLayout,
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

/// 팝업 표시 방식
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub enum PopupMode {
    /// 독립 프로세스 팝업 (unim-gui-gtk에서 표시)
    #[default]
    Standalone,
    /// 프론트엔드 내장 팝업 (기존 방식)
    Embedded,
}

impl PopupMode {
    pub fn name(&self) -> &str {
        match self {
            PopupMode::Standalone => "Standalone",
            PopupMode::Embedded => "Embedded",
        }
    }
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
    /// 팝업 표시 방식 (Standalone: GUI 통합, Embedded: 프론트엔드 내장)
    pub popup_mode: PopupMode,
    /// 자동 오타 교정 (AutoTypeFix) 설정
    pub auto_typefix: AutoTypeFixConfig,
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
            popup_mode: PopupMode::default(),
            auto_typefix: AutoTypeFixConfig::default(),
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
        assert_eq!(config.engine.korean.layout, KoreanLayout::Dubeolsik);
        assert_eq!(config.engine.english.layout, EnglishLayout::Qwerty);
    }

    #[test]
    fn test_korean_layout() {
        assert_eq!(KoreanLayout::Dubeolsik.name(), "2bul");
        assert!(!KoreanLayout::Dubeolsik.is_sebeolsik());
        assert!(KoreanLayout::Sebeolsik390.is_sebeolsik());
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
        assert_eq!(
            deserialized.engine.mode_sharing,
            config.engine.mode_sharing
        );
    }

    #[test]
    fn test_config_save_and_load() {
        let config = Config::default();
        let dir = std::env::temp_dir().join("unim_test_config");
        let path = dir.join("test_config.yaml");

        config.save_to_path(&path).unwrap();
        let loaded = Config::load_from_path(&path).unwrap();

        assert_eq!(
            loaded.engine.korean.layout,
            config.engine.korean.layout
        );
        assert_eq!(
            loaded.engine.toggle_keys,
            config.engine.toggle_keys
        );

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
    fn test_korean_layout_all() {
        let all = KoreanLayout::all();
        assert_eq!(all.len(), 4);
        assert!(all.contains(&KoreanLayout::Dubeolsik));
        assert!(all.contains(&KoreanLayout::SebeolsikNoShift));
    }

    #[test]
    fn test_english_layout_all() {
        let all = EnglishLayout::all();
        assert_eq!(all.len(), 5);
    }

    #[test]
    fn test_english_layout_keymap_name() {
        assert_eq!(EnglishLayout::Dvorak.keymap_name(), "en_dvorak");
        assert_eq!(EnglishLayout::Colemak.keymap_name(), "en_colemak");
        assert_eq!(EnglishLayout::ColemakDh.keymap_name(), "en_colemak_dh");
        assert_eq!(EnglishLayout::Workman.keymap_name(), "en_workman");
    }

    #[test]
    fn test_english_layout_top_row_labels() {
        assert_eq!(EnglishLayout::Qwerty.top_row_labels(), "QWERTYUIO");
        assert_eq!(EnglishLayout::Dvorak.top_row_labels(), "',.PYFGCR");
        assert_eq!(EnglishLayout::Colemak.top_row_labels(), "QWFPGJLUY");
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
        config.engine.korean.layout = KoreanLayout::Sebeolsik390;
        config.engine.english.layout = EnglishLayout::Dvorak;
        config.engine.mode_sharing = ModeSharingMode::PerApp;

        let yaml = serde_yaml::to_string(&config).unwrap();
        let loaded: Config = serde_yaml::from_str(&yaml).unwrap();

        assert_eq!(loaded.engine.korean.layout, KoreanLayout::Sebeolsik390);
        assert_eq!(loaded.engine.english.layout, EnglishLayout::Dvorak);
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
        assert_eq!(c.observation_timeout_secs, AUTO_TYPEFIX_OBSERVATION_TIMEOUT_MAX);

        let mut c = AutoTypeFixConfig {
            observation_timeout_secs: 0,
            ..Default::default()
        };
        c.clamp_ranges();
        assert_eq!(c.observation_timeout_secs, AUTO_TYPEFIX_OBSERVATION_TIMEOUT_MIN);
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
