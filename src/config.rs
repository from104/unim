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
    #[default]
    Korean,
    /// 영어 (English) 입력 모드
    English,
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
}

impl KoreanLayout {
    /// 레이아웃 이름을 반환합니다.
    pub fn name(&self) -> &'static str {
        match self {
            KoreanLayout::Dubeolsik => "2bul",
            KoreanLayout::Sebeolsik390 => "3bul390",
            KoreanLayout::Sebeolsik391 => "3bul391",
        }
    }

    /// 세벌식 레이아웃인지 확인합니다.
    pub fn is_sebeolsik(&self) -> bool {
        matches!(
            self,
            KoreanLayout::Sebeolsik390 | KoreanLayout::Sebeolsik391
        )
    }
}

/// 영어 키보드 레이아웃
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default, Serialize, Deserialize)]
#[repr(u32)]
pub enum EnglishLayout {
    /// QWERTY
    #[default]
    Qwerty = 0,
    /// Dvorak
    Dvorak = 1,
}

impl EnglishLayout {
    /// 레이아웃 이름을 반환합니다.
    pub fn name(&self) -> &'static str {
        match self {
            EnglishLayout::Qwerty => "qwerty",
            EnglishLayout::Dvorak => "dvorak",
        }
    }
}

/// 자동 전환 설정
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct AutoSwitchConfig {
    /// 자동 전환 활성화 여부
    pub enabled: bool,
    /// 감지 임계값 (0.0 ~ 1.0)
    pub threshold: f32,
    /// 전환 시 알림 표시 여부
    pub show_notification: bool,
}

/// 한국어 엔진 설정
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct KoreanConfig {
    /// 한국어 키보드 레이아웃
    pub layout: KoreanLayout,
    /// 조합 중 문자 표시 (Johab 형식)
    pub preedit_johab: bool,
    /// 단어 단위 커밋
    pub word_commit: bool,
}

impl Default for KoreanConfig {
    fn default() -> Self {
        Self {
            layout: KoreanLayout::default(),
            preedit_johab: false,
            word_commit: false,
        }
    }
}

/// 영어 엔진 설정
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct EnglishConfig {
    /// 영어 키보드 레이아웃
    pub layout: EnglishLayout,
    /// 다이렉트 입력 선호
    pub preferred_direct: bool,
}

impl Default for EnglishConfig {
    fn default() -> Self {
        Self {
            layout: EnglishLayout::default(),
            preferred_direct: true,
        }
    }
}

/// 엔진 설정
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct EngineConfig {
    /// 기본 입력 카테고리
    pub default_category: InputCategory,
    /// 한국어 설정
    pub korean: KoreanConfig,
    /// 영어 설정
    pub english: EnglishConfig,
    /// 자동 전환 설정
    pub auto_switch: AutoSwitchConfig,
}

impl Default for EngineConfig {
    fn default() -> Self {
        Self {
            default_category: InputCategory::default(),
            korean: KoreanConfig::default(),
            english: EnglishConfig::default(),
            auto_switch: AutoSwitchConfig::default(),
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
        eprintln!("[UNIM]   unim-config");
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
        assert_eq!(config.engine.default_category, InputCategory::Korean);
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
}
