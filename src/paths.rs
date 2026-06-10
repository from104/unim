//! 기본 디렉터리 해석 (UWP/MSIX 샌드박스 오버라이드 지원).
//!
//! Windows packaged 앱(UWP AppContainer) 내부에서는 `dirs::config_dir()` 등이
//! 패키지 전용 경로로 리다이렉트되어, 사용자의 실제 `%APPDATA%\unim` 을 절대
//! 읽지 못한다. Windows TSF DLL(unim-tsf)이 `SHGetKnownFolderPath` +
//! `KF_FLAG_NO_PACKAGE_REDIRECTION` 로 *리다이렉트되지 않은* 실제 경로를 알아내
//! `UNIM_CONFIG_DIR` / `UNIM_DATA_DIR` / `UNIM_CACHE_DIR` 환경변수로 내보내면,
//! 아래 헬퍼를 거치는 모든 config 로더가 올바른 비-리다이렉트 디렉터리를 쓴다.
//!
//! 비-패키지 프로세스(메모장·wezterm·CLI 등)에서는 환경변수가 없거나 실제
//! 경로와 동일하므로 `dirs::*` 와 완전히 같게 동작한다.

use std::path::PathBuf;

fn env_dir(key: &str) -> Option<PathBuf> {
    match std::env::var(key) {
        Ok(v) if !v.is_empty() => Some(PathBuf::from(v)),
        _ => None,
    }
}

/// `UNIM_CONFIG_DIR` 오버라이드가 있으면 그것을, 없으면 `dirs::config_dir()`.
pub fn config_dir() -> Option<PathBuf> {
    env_dir("UNIM_CONFIG_DIR").or_else(dirs::config_dir)
}

/// `UNIM_DATA_DIR` 오버라이드가 있으면 그것을, 없으면 `dirs::data_dir()`.
pub fn data_dir() -> Option<PathBuf> {
    env_dir("UNIM_DATA_DIR").or_else(dirs::data_dir)
}

/// `UNIM_CACHE_DIR` 오버라이드가 있으면 그것을, 없으면 `dirs::cache_dir()`.
pub fn cache_dir() -> Option<PathBuf> {
    env_dir("UNIM_CACHE_DIR").or_else(dirs::cache_dir)
}
