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
//! - `open_help()` — 오프라인 매뉴얼 HTML 을 기본 브라우저로 연다
//!   (Linux = `xdg-open` + `$(DATADIR)/unim/help`, Windows = `ShellExecuteW` +
//!   모듈 디렉터리 `help\`. 언어는 각 백엔드가 `ui_language_is_korean()` 으로 결정)
//! - `wizard_is_default_ime() -> bool`
//! - `wizard_set_as_default() -> DefaultImeOutcome` — 기본 입력기 지정 결과(3치, 아래 참조)
//! - `wizard_set_default_on_startup(v: bool)`
//! - `wizard_is_korean_language_installed() -> bool`
//! - `wizard_open_language_settings()`
//! - `wizard_seen_version() -> Option<String>`
//! - `set_wizard_seen_version(v: &str)`
//! - `is_gnome_wayland_session() -> bool` — GNOME Shell(Wayland) 세션 감지(BLOCKER-2)
//! - `detect_conflicting_ime() -> bool` — ibus/fcitx 공존 감지(GAP-first-06)
//! - `log_wizard_render_failure(msg: &str)` — 마법사 창 생성 실패를 로그·통지로 기록(GAP-first-05)

/// 기본 입력기 지정 결과 — 세 백엔드(windows/linux/fallback) 공통 표면.
///
/// `Success`/`Failed` 2치가 아니라 3치인 이유(BLOCKER-1, GAP-first-run-lifecycle-01):
/// Fedora 등 im-config 자체가 없는 배포판에서는 `~/.xinputrc` 를 직접 써도 그 파일을
/// 실제로 읽어 적용하는 im-config 인프라(`/usr/share/im-config/xinputrc.common` +
/// `/etc/X11/Xsession.d/70im-config_launch`)가 없어 "성공처럼 보이는 무효과" 가 된다.
/// 이 경우를 재시도가 유의미한 실행 실패(`Failed`)와 분리해 `ManualSetupRequired` 로
/// 두고, 호출부(wizard.rs)가 seen 기록을 보류하고 별도 안내 카드를 띄우게 한다.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DefaultImeOutcome {
    /// 지정 성공(자동 지정 또는 폴백 기록 성공).
    Success,
    /// 지정 시도가 실패했으나 재시도 가능(수단 자체는 존재 — 예: im-config 설치돼
    /// 있으나 이번 실행만 실패).
    Failed,
    /// 이 환경엔 자동 지정 수단이 아예 없음 — 폴백을 시도하지 않았고 수동 설정 안내가 필요.
    ///
    /// Windows·제3 플랫폼 백엔드는 이 variant 를 **생성하지 않는다**(레지스트리 API 는
    /// 항상 존재하므로 im-config 부재에 해당하는 개념이 없다). 그러나 `wizard.rs` 의
    /// match 는 전 플랫폼 공통 코드라 variant 자체는 유지해야 한다.
    #[cfg_attr(not(target_os = "linux"), allow(dead_code))]
    ManualSetupRequired,
}

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
    // 도움말 진입점 — 제3 플랫폼에는 동봉 매뉴얼 설치 경로 계약이 없으므로 no-op.
    // (UI 의 [도움말] 버튼은 렌더되지만 아무 동작도 하지 않는다. 빌드 유지가 목적.)
    pub fn open_help() {}
    pub fn wizard_is_default_ime() -> bool {
        true
    }
    pub fn wizard_set_as_default() -> super::DefaultImeOutcome {
        super::DefaultImeOutcome::Success
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
    // GNOME/ibus/fcitx 는 Linux 전용 개념이라 제3 플랫폼에선 항상 무해(false).
    pub fn is_gnome_wayland_session() -> bool {
        false
    }
    pub fn gnome_extension_needs_enable() -> bool {
        false
    }
    // 호출부가 Linux 전용 cfg 라 여기서도 미사용 — 표면 대칭 유지 목적.
    #[allow(dead_code)]
    pub fn detect_conflicting_ime() -> bool {
        false
    }
    pub fn log_wizard_render_failure(_msg: &str) {}
}
#[cfg(not(any(windows, target_os = "linux")))]
pub use fallback::*;
