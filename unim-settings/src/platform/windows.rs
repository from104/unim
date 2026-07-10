//! Windows 플랫폼 백엔드 — main.rs 에서 **verbatim 이동**.
//!
//! Win32 API 로 UI 언어 판정·단일 인스턴스 가드를 구현하고, 설치 마법사의 기본
//! 입력기/언어팩 감지·설정을 `unim_windows_common::ime` 래퍼(`wizard_*`)로 노출한다.
//! 로직은 이동 전과 바이트 동일(이식 규율: Windows 분기 diff 0). 마지막 확인은
//! GHA windows-msi.yml(MSVC) — skia 가 windows-gnu 프리빌트 부재라 로컬 cross 불가.

use unim::config::Config;
use unim_windows_common::ime;

/// OS 기본 UI 언어가 한국어(LANG_KOREAN=0x12)인지 판정.
/// GetUserDefaultUILanguage 는 하위 10비트가 primary language id.
pub fn ui_language_is_korean() -> bool {
    extern "system" {
        fn GetUserDefaultUILanguage() -> u16;
    }
    // SAFETY: 인자 없는 순수 조회 Win32 API.
    let langid = unsafe { GetUserDefaultUILanguage() };
    (langid & 0x3ff) == 0x12
}

/// 단일 인스턴스 가드 (Windows). 이미 설정 창이 떠 있으면 그 창을 전면화하고
/// `false` 를 돌려 호출자가 즉시 종료하도록 한다. 첫 인스턴스면 `true`.
///
/// 명명 뮤텍스(`Local\` = 세션 로컬)로 중복 실행을 감지하고, 기존 창은 제목으로
/// `FindWindowW` 해서 최소화 상태면 복원 후 `SetForegroundWindow` 로 끌어올린다.
/// 창 제목은 실행 중 인스턴스와 동일 규칙(OS UI 언어)으로 계산한다.
/// 첫 인스턴스가 만든 뮤텍스 핸들은 프로세스 종료 시 OS 가 정리하므로 닫지 않는다
/// (원시 포인터라 스코프 이탈만으로는 커널 핸들이 닫히지 않는다).
pub fn acquire_singleton_or_foreground() -> bool {
    use std::ffi::c_void;
    extern "system" {
        fn CreateMutexW(attrs: *const c_void, owner: i32, name: *const u16) -> *mut c_void;
        fn GetLastError() -> u32;
        fn FindWindowW(class: *const u16, window: *const u16) -> *mut c_void;
        fn SetForegroundWindow(hwnd: *mut c_void) -> i32;
        fn ShowWindow(hwnd: *mut c_void, cmd: i32) -> i32;
        fn IsIconic(hwnd: *mut c_void) -> i32;
    }
    const ERROR_ALREADY_EXISTS: u32 = 183;
    const SW_RESTORE: i32 = 9;

    fn wide(s: &str) -> Vec<u16> {
        s.encode_utf16().chain(std::iter::once(0)).collect()
    }

    let name = wide("Local\\unim-settings-singleton");
    // SAFETY: 명명 커널 오브젝트 생성. 인자는 널종단 UTF-16 이름 하나뿐.
    let already_running = unsafe {
        let _h = CreateMutexW(std::ptr::null(), 0, name.as_ptr());
        GetLastError() == ERROR_ALREADY_EXISTS
    };
    if !already_running {
        return true;
    }

    // 이미 실행 중 → 기존 창 전면화. 제목은 실행 중 인스턴스와 동일 규칙으로 계산.
    let title = wide(if ui_language_is_korean() {
        "UNIM 설정"
    } else {
        "UNIM Settings"
    });
    // SAFETY: 조회/포커스 이동 Win32 호출. HWND 는 널 검사 후에만 사용.
    unsafe {
        let hwnd = FindWindowW(std::ptr::null(), title.as_ptr());
        if !hwnd.is_null() {
            if IsIconic(hwnd) != 0 {
                ShowWindow(hwnd, SW_RESTORE);
            }
            SetForegroundWindow(hwnd);
        }
    }
    false
}

/// 저장 후 데몬 통지 — Windows 는 no-op. TSF DLL 이 config.yaml mtime 폴링으로
/// 스스로 reload 하므로(text_service.rs::maybe_reload_config) 별도 IPC 가 없다.
/// Linux 백엔드와 함수 표면을 맞추기 위한 대칭 no-op.
pub fn notify_config_saved(_cfg: &Config, _label: &str) {}

// ── 설치 마법사: 기본 입력기/언어팩 감지·설정 (unim-windows-common::ime 래퍼) ──
// 감지/설정 헬퍼는 windows 전용 크레이트(#![cfg(windows)])에 있으므로 이 모듈은
// windows 빌드에서만 컴파일된다. 비Windows 는 linux/fallback 백엔드가 대응한다.
pub fn wizard_is_default_ime() -> bool {
    ime::is_default_ime()
}
pub fn wizard_set_as_default() {
    let _ = ime::set_as_default();
}
pub fn wizard_set_default_on_startup(v: bool) {
    let _ = ime::set_default_on_startup(v);
}
pub fn wizard_is_korean_language_installed() -> bool {
    ime::is_korean_language_installed()
}
pub fn wizard_open_language_settings() {
    ime::open_language_settings();
}
pub fn wizard_seen_version() -> Option<String> {
    ime::get_wizard_seen_version()
}
pub fn set_wizard_seen_version(v: &str) {
    let _ = ime::set_wizard_seen_version(v);
}
