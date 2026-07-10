//! 플랫폼 백엔드 — `cfg` 로 정확히 하나의 구현만 컴파일된다.
//!
//! "트레이트 없는 트레이트": 세 백엔드(windows / linux / fallback)가 **동일한
//! 함수 표면**을 노출한다. 시그니처가 어긋나면 호출부(main.rs·wizard.rs)에서
//! 컴파일 에러로 즉시 드러난다. 실질 지원 대상은 windows·linux 두 플랫폼이며,
//! 그 외(macOS 등 제3 플랫폼)는 **워크스페이스 빌드 유지용 no-op fallback**을 쓴다.
//! (linux 백엔드는 후속 단계에서 `cfg(target_os = "linux")` 전용 의존을 끌어오므로,
//!  제3 플랫폼이 linux.rs 를 컴파일하지 않도록 축을 3분한다.)
//!
//! 함수 계약:
//! - `ui_language_is_korean() -> bool`
//! - `acquire_singleton_or_foreground() -> bool`
//! - `notify_config_saved(cfg: &Config, label: &str)` — 저장 후 데몬 통지
//!   (Linux = DBus fire-and-forget, Windows·fallback = no-op)
//! - `wizard_is_default_ime() -> bool`
//! - `wizard_set_as_default() -> bool` — 기본 입력기 지정 성공 여부
//! - `wizard_set_default_on_startup(v: bool)`
//! - `wizard_is_korean_language_installed() -> bool`
//! - `wizard_open_language_settings()`
//! - `wizard_seen_version() -> Option<String>`
//! - `set_wizard_seen_version(v: &str)`

#[cfg(windows)]
mod windows;
#[cfg(windows)]
pub use windows::*;

#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "linux")]
pub use linux::*;

// windows·linux 이외(macOS 등) — 빌드 유지를 위한 no-op stub. linux.rs 의 현행
// 폴백과 동일한 표면·의미(모두 무해한 기본값)를 유지한다.
#[cfg(not(any(windows, target_os = "linux")))]
mod fallback {
    pub fn ui_language_is_korean() -> bool {
        false
    }
    pub fn acquire_singleton_or_foreground() -> bool {
        true
    }
    // main.rs `persist_config` 가 저장 성공 시 무조건 호출하므로, 이 표면이 없으면
    // 제3 플랫폼 빌드가 컴파일되지 않는다. windows.rs·linux.rs 와 동일 no-op 대칭.
    pub fn notify_config_saved(_cfg: &unim::config::Config, _label: &str) {}
    pub fn wizard_is_default_ime() -> bool {
        true
    }
    pub fn wizard_set_as_default() -> bool {
        true
    }
    pub fn wizard_set_default_on_startup(_v: bool) {}
    pub fn wizard_is_korean_language_installed() -> bool {
        true
    }
    pub fn wizard_open_language_settings() {}
    pub fn wizard_seen_version() -> Option<String> {
        None
    }
    pub fn set_wizard_seen_version(_v: &str) {}
}
#[cfg(not(any(windows, target_os = "linux")))]
pub use fallback::*;
