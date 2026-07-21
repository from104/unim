//! 오프라인 사용자 매뉴얼(HTML) 경로 해석 + 기본 브라우저로 열기 (Windows).
//!
//! 설치본은 매뉴얼을 **실행 모듈 디렉터리의 `help\` 하위**에 동봉한다. 탐색 순서는
//! `unim-tsf/src/popup_ipc.rs::find_renderer_exe()` 와 동일한 계약을 따른다:
//! ① 실행 모듈 디렉터리 → ② HKLM `SOFTWARE\atit.org\UNIM\InstallDir`(MSI 가 기록).
//!
//! Linux 쪽 대응 구현은 `unim-settings/src/platform/linux.rs` 에 별도로 둔다 —
//! 기준점이 DLL/EXE 모듈 경로가 아니라 `$(DATADIR)` 이라 공통화 이득이 없다.

use windows::core::{HSTRING, PCWSTR};
use windows::Win32::Foundation::HMODULE;
use windows::Win32::System::Registry::{
    RegGetValueW, HKEY_LOCAL_MACHINE, RRF_RT_REG_SZ,
};
use windows::Win32::UI::Shell::ShellExecuteW;
use windows::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL;

use crate::registry::get_module_path;

/// 매뉴얼 파일명 — Makefile·MSI·도움말 생성기와 공유하는 고정 계약.
pub const HELP_FILE_KO: &str = "unim-help-ko.html";
pub const HELP_FILE_EN: &str = "unim-help-en.html";

/// UI 언어에 대응하는 매뉴얼 파일명.
pub fn help_file_name(korean: bool) -> &'static str {
    if korean {
        HELP_FILE_KO
    } else {
        HELP_FILE_EN
    }
}

/// 실행 모듈(EXE 호스트면 그 EXE)의 디렉터리.
/// `HMODULE::default()`(NULL) → `GetModuleFileNameW` 가 프로세스 실행 파일 경로를 준다.
fn module_dir() -> Option<std::path::PathBuf> {
    let path = get_module_path(HMODULE::default()).ok()?;
    std::path::Path::new(&path)
        .parent()
        .map(|p| p.to_path_buf())
}

/// HKLM\SOFTWARE\atit.org\UNIM\InstallDir (MSI 가 기록). popup_ipc 와 동일 키.
fn hklm_install_dir() -> Option<std::path::PathBuf> {
    // SAFETY: 읽기 전용 레지스트리 조회. 버퍼 크기를 바이트 단위로 정확히 넘긴다.
    unsafe {
        let subkey: HSTRING = "SOFTWARE\\atit.org\\UNIM".into();
        let value: HSTRING = "InstallDir".into();
        let mut buf = [0u16; 520];
        let mut size: u32 = (buf.len() * std::mem::size_of::<u16>()) as u32;
        let rc = RegGetValueW(
            HKEY_LOCAL_MACHINE,
            PCWSTR(subkey.as_ptr()),
            PCWSTR(value.as_ptr()),
            RRF_RT_REG_SZ,
            None,
            Some(buf.as_mut_ptr() as *mut core::ffi::c_void),
            Some(&mut size),
        );
        if rc.is_err() {
            return None;
        }
        // size 는 NUL 포함 바이트 수 → u16 길이로 환산 후 NUL 제거.
        let nchars = (size as usize / std::mem::size_of::<u16>()).saturating_sub(1);
        Some(std::path::PathBuf::from(String::from_utf16_lossy(
            &buf[..nchars],
        )))
    }
}

/// 매뉴얼 HTML 의 실제 경로. 후보를 순회해 **파일이 존재하는 첫 항목**을 채택하고,
/// 어디에도 없으면 `None`(호출부가 사용자에게 알린다).
pub fn find_help_file(korean: bool) -> Option<std::path::PathBuf> {
    let name = help_file_name(korean);
    for base in [module_dir(), hklm_install_dir()].into_iter().flatten() {
        let cand = base.join("help").join(name);
        if cand.is_file() {
            return Some(cand);
        }
    }
    None
}

/// 매뉴얼을 기본 브라우저로 연다. 파일을 못 찾거나 셸 실행이 실패하면 `false`
/// (호출부가 사용자에게 안내한다 — 조용한 무시 금지).
///
/// `lpFile` 은 단일 인자라 `C:\Program Files\...` 같은 공백 경로를 **따옴표 없이**
/// 그대로 넘겨야 한다(따옴표를 붙이면 경로의 일부로 해석돼 실패).
///
/// 언어는 보통 [`ui_language_is_korean`] 으로 정한다 — 호출자마다 다른 판정을 쓰면
/// 같은 PC 에서 언어바와 설정앱이 서로 다른 언어의 매뉴얼을 여는 사고가 난다.
pub fn open_help(korean: bool) -> bool {
    let Some(path) = find_help_file(korean) else {
        return false;
    };
    // SAFETY: 널이 아닌 UTF-16 문자열 포인터만 전달하는 셸 호출.
    let ok = unsafe {
        let file: HSTRING = path.as_os_str().into();
        let op: HSTRING = "open".into();
        let hinst = ShellExecuteW(
            None,
            PCWSTR(op.as_ptr()),
            PCWSTR(file.as_ptr()),
            PCWSTR::null(),
            PCWSTR::null(),
            SW_SHOWNORMAL,
        );
        // ShellExecuteW 는 성공 시 32 초과의 값을 돌려준다(레거시 계약).
        hinst.0 as usize > 32
    };
    ok
}

/// OS UI 언어가 한국어인지 — `GetUserDefaultUILanguage()` 하위 10비트(primary
/// language ID)가 `0x12`(LANG_KOREAN)인지로 판정한다.
///
/// 언어바(`unim-tsf`)와 설정앱(`unim-settings`)이 **같은 판정**을 쓰도록 여기 한
/// 곳에 둔다. 갈라지면 같은 PC 에서 앱마다 다른 언어의 매뉴얼이 열린다.
/// windows-rs 의 `Win32_Globalization` 피처를 새로 켜지 않으려고 `extern` 선언을
/// 쓴다(인자 없는 순수 조회라 바인딩 이득이 없다).
pub fn ui_language_is_korean() -> bool {
    extern "system" {
        fn GetUserDefaultUILanguage() -> u16;
    }
    // SAFETY: 인자 없는 순수 조회 Win32 API.
    let langid = unsafe { GetUserDefaultUILanguage() };
    (langid & 0x3ff) == 0x12
}

#[cfg(test)]
mod tests {
    /// 파일명 계약이 언어별로 고정돼 있는지 — Makefile/MSI 와 문자열을 공유한다.
    #[test]
    fn help_file_name_matches_contract() {
        assert_eq!(super::help_file_name(true), "unim-help-ko.html");
        assert_eq!(super::help_file_name(false), "unim-help-en.html");
    }
}
