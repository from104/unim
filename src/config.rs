//! 설정 모듈
//!
//! UNIM 입력기의 설정을 관리합니다.

use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

/// 입력 카테고리 (한글/영문)
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default, Serialize, Deserialize)]
#[repr(C)]
pub enum InputCategory {
    /// 한글 입력 모드
    #[default]
    Hangul,
    /// 영문 (라틴) 입력 모드
    Latin,
}

/// 한글 키보드 레이아웃
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default, Serialize, Deserialize)]
#[repr(u32)]
pub enum HangulLayout {
    /// 두벌식 표준
    #[default]
    Dubeolsik = 0,
    /// 세벌식 390
    Sebeolsik390 = 1,
    /// 세벌식 최종
    Sebeolsik391 = 2,
}

impl HangulLayout {
    /// 레이아웃 이름을 반환합니다.
    pub fn name(&self) -> &'static str {
        match self {
            HangulLayout::Dubeolsik => "2bul",
            HangulLayout::Sebeolsik390 => "3bul390",
            HangulLayout::Sebeolsik391 => "3bul391",
        }
    }

    /// 세벌식 레이아웃인지 확인합니다.
    pub fn is_sebeolsik(&self) -> bool {
        matches!(self, HangulLayout::Sebeolsik390 | HangulLayout::Sebeolsik391)
    }
}

/// 영문 키보드 레이아웃
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default, Serialize, Deserialize)]
#[repr(u32)]
pub enum LatinLayout {
    /// QWERTY
    #[default]
    Qwerty = 0,
    /// Dvorak
    Dvorak = 1,
}

impl LatinLayout {
    /// 레이아웃 이름을 반환합니다.
    pub fn name(&self) -> &'static str {
        match self {
            LatinLayout::Qwerty => "qwerty",
            LatinLayout::Dvorak => "dvorak",
        }
    }
}

/// 자동 전환 설정
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct AutoSwitchConfig {
    /// 자동 전환 활성화 여부
    pub enabled: bool,
    /// 감지 임계값 (0.0 ~ 1.0)
    pub threshold: f32,
    /// 전환 시 알림 표시 여부
    pub show_notification: bool,
}

/// 한글 엔진 설정
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct HangulConfig {
    /// 한글 키보드 레이아웃
    pub layout: HangulLayout,
    /// 조합 중 문자 표시 (Johab 형식)
    pub preedit_johab: bool,
    /// 단어 단위 커밋
    pub word_commit: bool,
}

impl Default for HangulConfig {
    fn default() -> Self {
        Self {
            layout: HangulLayout::default(),
            preedit_johab: false,
            word_commit: false,
        }
    }
}

/// 영문 엔진 설정
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct LatinConfig {
    /// 영문 키보드 레이아웃
    pub layout: LatinLayout,
    /// 다이렉트 입력 선호
    pub preferred_direct: bool,
}

impl Default for LatinConfig {
    fn default() -> Self {
        Self {
            layout: LatinLayout::default(),
            preferred_direct: true,
        }
    }
}

/// 엔진 설정
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct EngineConfig {
    /// 기본 입력 카테고리
    pub default_category: InputCategory,
    /// 한글 설정
    pub hangul: HangulConfig,
    /// 영문 설정
    pub latin: LatinConfig,
    /// 자동 전환 설정
    pub auto_switch: AutoSwitchConfig,
}

impl Default for EngineConfig {
    fn default() -> Self {
        Self {
            default_category: InputCategory::default(),
            hangul: HangulConfig::default(),
            latin: LatinConfig::default(),
            auto_switch: AutoSwitchConfig::default(),
        }
    }
}

/// UNIM 전체 설정
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Config {
    /// 엔진 설정
    pub engine: EngineConfig,
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
    /// 설정 파일이 없거나 파싱 실패 시 기본값을 반환합니다.
    pub fn load_from_default_path() -> Self {
        Self::default_config_path()
            .and_then(|path| Self::load_from_path(&path).ok())
            .unwrap_or_default()
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
        let content = fs::read_to_string(path).map_err(|e| ConfigError::IoError(e.to_string()))?;
        serde_yaml::from_str(&content).map_err(|e| ConfigError::ParseError(e.to_string()))
    }

    /// 설정을 기본 경로에 저장합니다.
    pub fn save_to_default_path(&self) -> Result<(), ConfigError> {
        let path = Self::default_config_path().ok_or_else(|| {
            ConfigError::IoError("설정 디렉터리를 찾을 수 없습니다.".to_string())
        })?;
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
        assert_eq!(config.engine.default_category, InputCategory::Hangul);
        assert_eq!(config.engine.hangul.layout, HangulLayout::Dubeolsik);
        assert_eq!(config.engine.latin.layout, LatinLayout::Qwerty);
    }

    #[test]
    fn test_hangul_layout() {
        assert_eq!(HangulLayout::Dubeolsik.name(), "2bul");
        assert!(!HangulLayout::Dubeolsik.is_sebeolsik());
        assert!(HangulLayout::Sebeolsik390.is_sebeolsik());
    }

    #[test]
    fn test_input_category() {
        let hangul = InputCategory::Hangul;
        let latin = InputCategory::Latin;
        assert_ne!(hangul, latin);
    }
}
