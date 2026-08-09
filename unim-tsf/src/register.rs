//! TSF 프로필 등록/해제 + COM InProcServer32 레지스트리 등록.
//!
//! 카테고리 / LanguageProfile / TIP root 키는 `installer/wix/unim.wxs` 의 static
//! block 이 작성한다 (SampleIME 표준 8종 — researcher 보고서
//! `docs/dev/windows/TSF_RESEARCH_REDESIGN.md` 참조). 본 모듈은 `HKCR\CLSID\{CLSID}`
//! (+ `InProcServer32`) 와 LanguageProfile 6개 값만 직접 기록한다.
//!
//! 과거 `ITfInputProcessorProfiles::AddLanguageProfile` 및
//! `ITfCategoryMgr::RegisterCategory` 호출은 wxs 가 이미 동일 키를 박은 상태에서
//! `msctf.dll` 내부 (offset 0x97e5a) 에서 `0xC0000005` NULL deref 를 일으켜 제거함
//! (재진입/state 충돌 추정).

use windows::core::*;
use windows::Win32::Foundation::*;
use windows::Win32::System::Registry::*;
use windows::Win32::UI::Input::KeyboardAndMouse::HKL;
use windows::Win32::UI::TextServices::*;

use crate::globals::*;

use unim_windows_common::registry::{set_reg_value, set_reg_dword};

fn get_dll_path() -> Result<String> {
    unim_windows_common::registry::get_module_path(crate::dll_instance())
}

/// 설정 GUI(`unim-settings.exe`)를 DLL 과 같은 디렉터리에서 실행한다.
///
/// 트레이/랭귀지바 우클릭 메뉴의 "설정" 및 Windows 언어 옵션(ITfFnConfigure)에서
/// 호출한다. 별도 프로세스(Slint GUI)로 띄우므로 TSF STA 스레드를 막지 않는다.
/// 성공 시 `true`. exe 부재(개발 빌드 등)·spawn 실패 시 `false` → 호출자는
/// 옛 내장 Win32 다이얼로그로 폴백한다.
pub fn launch_settings_app() -> bool {
    let dll_path = match get_dll_path() {
        Ok(p) => p,
        Err(_) => return false,
    };
    let exe = match std::path::Path::new(&dll_path).parent() {
        Some(dir) => dir.join("unim-settings.exe"),
        None => return false,
    };
    if !exe.exists() {
        dbg_log(&format!(
            "launch_settings_app: exe not found ({})",
            exe.display()
        ));
        return false;
    }
    match std::process::Command::new(&exe).spawn() {
        Ok(_) => {
            dbg_log("launch_settings_app: settings GUI spawned");
            true
        }
        Err(e) => {
            dbg_log(&format!("launch_settings_app: spawn failed: {e}"));
            false
        }
    }
}


unsafe fn register_com_server() -> Result<()> {
    let dll_path = get_dll_path()?;
    let clsid_str = format!("{{{:?}}}", UNIM_CLSID);

    let key_path: HSTRING = format!("CLSID\\{}", clsid_str).into();
    let mut hkey = HKEY::default();
    let err = RegCreateKeyW(HKEY_CLASSES_ROOT, &key_path, &mut hkey);
    if err.is_err() {
        return Err(E_FAIL.into());
    }
    set_reg_value(hkey, None, UNIM_IME_NAME)?;
    let _ = RegCloseKey(hkey);

    let inproc_path: HSTRING = format!("CLSID\\{}\\InProcServer32", clsid_str).into();
    let mut hkey_inproc = HKEY::default();
    let err = RegCreateKeyW(HKEY_CLASSES_ROOT, &inproc_path, &mut hkey_inproc);
    if err.is_err() {
        return Err(E_FAIL.into());
    }
    set_reg_value(hkey_inproc, None, &dll_path)?;
    let threading_model: HSTRING = "ThreadingModel".into();
    set_reg_value(hkey_inproc, Some(&threading_model), "Apartment")?;
    let _ = RegCloseKey(hkey_inproc);

    Ok(())
}

unsafe fn unregister_com_server() -> Result<()> {
    let clsid_str = format!("{{{:?}}}", UNIM_CLSID);
    let inproc_path: HSTRING = format!("CLSID\\{}\\InProcServer32", clsid_str).into();
    let _ = RegDeleteKeyW(HKEY_CLASSES_ROOT, &inproc_path);
    let key_path: HSTRING = format!("CLSID\\{}", clsid_str).into();
    let _ = RegDeleteKeyW(HKEY_CLASSES_ROOT, &key_path);
    Ok(())
}

pub fn register_server() -> Result<()> {
    unsafe {
        // (1) HKCR\CLSID\{CLSID} (+ InProcServer32) — COM 서버 등록.
        //     MSI wxs static 블록도 동일 키를 박지만, 외부 regsvr32 시나리오와
        //     drift 방지 위해 양쪽 유지.
        register_com_server()?;

        // (2) LanguageProfile 6개 값 직접 기록.
        //     예전엔 ITfInputProcessorProfiles::AddLanguageProfile 을 호출했으나,
        //     wxs 가 이미 동일 LP 키를 박은 뒤 재호출하면 msctf.dll 0x97e5a
        //     에서 0xC0000005 NULL deref. 직접 RegSetValueExW 로 우회.
        let dll_path = get_dll_path()?;
        let clsid_str = format!("{{{:?}}}", UNIM_CLSID);
        let profile_str = format!("{{{:?}}}", UNIM_PROFILE_GUID);
        let lp_path: HSTRING = format!(
            "SOFTWARE\\Microsoft\\CTF\\TIP\\{}\\LanguageProfile\\0x{:08X}\\{}",
            clsid_str, UNIM_LANGID_KOREAN, profile_str
        )
        .into();
        let mut hkey_lp = HKEY::default();
        if RegCreateKeyW(HKEY_LOCAL_MACHINE, &lp_path, &mut hkey_lp).is_ok() {
            let name_enable: HSTRING = "Enable".into();
            let name_substitute: HSTRING = "SubstituteLayout".into();
            let name_icon_file: HSTRING = "IconFile".into();
            let name_icon_index: HSTRING = "IconIndex".into();
            let name_display: HSTRING = "Display".into();
            let name_description: HSTRING = "Description".into();
            let _ = set_reg_dword(hkey_lp, &name_enable, 1);
            let _ = set_reg_dword(hkey_lp, &name_substitute, u32::from(UNIM_LANGID_KOREAN));
            let _ = set_reg_value(hkey_lp, Some(&name_icon_file), &dll_path);
            let _ = set_reg_dword(hkey_lp, &name_icon_index, 0);
            let _ = set_reg_value(hkey_lp, Some(&name_display), UNIM_IME_NAME);
            let _ = set_reg_value(hkey_lp, Some(&name_description), UNIM_IME_NAME);
            let _ = RegCloseKey(hkey_lp);
        }

        // (3) Category 8종 + TIP root + profiles.Register 는 모두 wxs 가 박는다.
        //     ITfCategoryMgr::RegisterCategory / ITfInputProcessorProfiles::Register
        //     를 여기서 호출하면 msctf.dll 와 충돌하므로 호출하지 않는다.
        //     (수정 이력: docs/dev/windows/TSF_RESEARCH_REDESIGN.md 결함 1 참조)
    }
    Ok(())
}

// ── 진단 로그 런타임 게이트 (릴리스 기본 OFF, 환경변수 opt-in) ──
//
// 과거에는 `UNIM_DEBUG_LOG: bool = true` 컴파일 상수라 릴리스에서도 상시 ON 이었다.
// 그 결과 매 키의 vk·preedit·commit 평문이 `%TEMP%\unim-tsf.log` 에 무회전 누적되어
// 사실상 키로거가 됐고, 키당 동기 파일 IO 로 입력 지연을 유발했다.
//
// 이제 두 단계 게이트로 나눈다 (프로세스 최초 1회 env 읽어 캐시, hot-path 원자 read).
//   • UNIM_DEBUG_LOG   : 구조적 진단 이벤트 파일 기록을 켠다 (opt-in). 미설정=완전 OFF.
//   • UNIM_DEBUG_CONTENT: 위에 더해 민감 콘텐츠(vk 원값·preedit/commit/꼬리 텍스트)
//                         까지 기록한다. 미설정 시엔 구조적 이벤트만 남는다.
// "0"/빈 문자열은 OFF 로 본다. 로깅 비활성이면 format!·파일 open 이전에 조기 반환하여
// hot-path 비용을 0 으로 만든다 (dbg_log 시그니처는 유지).

fn env_flag_on(name: &str) -> bool {
    match std::env::var_os(name) {
        Some(v) => !v.is_empty() && v != "0",
        None => false,
    }
}

/// 구조 로그 게이트 (UNIM_DEBUG_LOG). 프로세스 최초 호출 시 env 를 읽어 캐시한다.
pub(crate) fn logging_enabled() -> bool {
    use std::sync::OnceLock;
    static LOG_ON: OnceLock<bool> = OnceLock::new();
    *LOG_ON.get_or_init(|| env_flag_on("UNIM_DEBUG_LOG"))
}

/// 콘텐츠 로그 게이트 (UNIM_DEBUG_LOG && UNIM_DEBUG_CONTENT). 캐시된다.
/// 콘텐츠는 구조 로그가 켜져 있을 때에만 의미가 있으므로 두 게이트의 AND 이다.
pub(crate) fn content_enabled() -> bool {
    use std::sync::OnceLock;
    static CONTENT_ON: OnceLock<bool> = OnceLock::new();
    *CONTENT_ON.get_or_init(|| logging_enabled() && env_flag_on("UNIM_DEBUG_CONTENT"))
}

/// 진단 로그 한 줄을 `%TEMP%\unim-tsf.log` 에 append 한다.
///
/// UNIM_DEBUG_LOG 미설정 시 format!·파일 open 이전에 조기 반환(비용 0). 호출자가 넘긴
/// 문자열은 그대로 기록되므로, 민감 콘텐츠를 담는 호출은 반드시 `dbg_log_ev!` 매크로로
/// content 게이트를 통과시켜야 한다. compartment / lang_bar 등 구조 진단에서 공용.
/// 실패해도 무시(크래시 없음).
pub(crate) fn dbg_log(msg: &str) {
    if !logging_enabled() {
        return;
    }
    unim_windows_common::debug::dbg_log("unim-tsf", "unim-tsf.log", msg, false);
}

/// 구조 이벤트는 로그 ON 시 항상 남기고, 민감 콘텐츠(vk 원값·타이핑 텍스트)는
/// UNIM_DEBUG_CONTENT=1 일 때만 덧붙이는 로깅 매크로.
///
/// - `dbg_log_ev!("구조 문자열")` : 구조 라인만 (`dbg_log` 와 동일).
/// - `dbg_log_ev!("구조 폴백", "콘텐츠 포함 {} 포맷", 값…)` : content 게이트 ON 이면
///   콘텐츠 포맷을, OFF 면 구조 폴백 문자열을 남긴다. content 게이트가 OFF 이면
///   format! 자체를 수행하지 않으므로 콘텐츠가 메모리에도 구성되지 않는다.
macro_rules! dbg_log_ev {
    ($plain:expr $(,)?) => {
        $crate::register::dbg_log($plain)
    };
    ($plain:expr, $fmt:literal $(, $arg:expr)* $(,)?) => {{
        // 로그 자체가 OFF 면 $plain·$fmt 어느 것도 format! 하지 않아 hot-path 비용 0.
        if $crate::register::logging_enabled() {
            if $crate::register::content_enabled() {
                $crate::register::dbg_log(&format!($fmt $(, $arg)*));
            } else {
                $crate::register::dbg_log($plain);
            }
        }
    }};
}
pub(crate) use dbg_log_ev;

// ── 기본 입력기(default profile) 설정 — 사용자 컨텍스트 전용 ──
//
// SetDefaultLanguageProfile / ActivateProfile 은 HKCU 와 현재 세션에 작용하므로
// 반드시 로그인한 사용자 프로세스(unim-windows.exe)에서 호출해야 한다.
// per-machine MSI 의 DllRegisterServer(SYSTEM 컨텍스트)에서 부르면 효과가 없다.

/// UNIM 을 한국어(0x0412)의 기본 입력 프로필로 지정하고 현재 세션에서 활성화한다.
pub fn set_as_default() -> Result<()> {
    unsafe {
        // 이미 COM 초기화돼 있으면 S_FALSE / RPC_E_CHANGED_MODE 가 오지만 무시한다.
        let _ = windows::Win32::System::Com::CoInitializeEx(
            None,
            windows::Win32::System::Com::COINIT_APARTMENTTHREADED,
        );

        let profiles: ITfInputProcessorProfiles = windows::Win32::System::Com::CoCreateInstance(
            &CLSID_TF_InputProcessorProfiles,
            None,
            windows::Win32::System::Com::CLSCTX_INPROC_SERVER,
        )?;

        profiles.SetDefaultLanguageProfile(UNIM_LANGID_KOREAN, &UNIM_CLSID, &UNIM_PROFILE_GUID)?;

        let mgr: ITfInputProcessorProfileMgr = profiles.cast()?;
        mgr.ActivateProfile(
            TF_PROFILETYPE_INPUTPROCESSOR,
            UNIM_LANGID_KOREAN,
            &UNIM_CLSID,
            &UNIM_PROFILE_GUID,
            HKL::default(),
            TF_IPPMF_ENABLEPROFILE | TF_IPPMF_FORSESSION,
        )?;
    }
    Ok(())
}

// ── "시작 시 기본 입력기로 설정" 선호값 (HKCU\Software\atit.org\UNIM) ──

const PREF_SUBKEY: &str = "Software\\atit.org\\UNIM";
const PREF_VALUE_NAME: &str = "DefaultOnStartup";

/// 선호값을 읽는다. 키가 없거나 0이면 false.
pub fn get_default_on_startup() -> bool {
    unsafe {
        let subkey: HSTRING = PREF_SUBKEY.into();
        let value: HSTRING = PREF_VALUE_NAME.into();
        let mut hkey = HKEY::default();
        if RegOpenKeyExW(HKEY_CURRENT_USER, &subkey, Some(0), KEY_READ, &mut hkey).is_err() {
            return false;
        }
        let mut data: u32 = 0;
        let mut size = std::mem::size_of::<u32>() as u32;
        let res = RegQueryValueExW(
            hkey,
            PCWSTR(value.as_ptr()),
            None,
            None,
            Some(&mut data as *mut u32 as *mut u8),
            Some(&mut size),
        );
        let _ = RegCloseKey(hkey);
        res.is_ok() && data != 0
    }
}

/// 선호값을 기록한다 (REG_DWORD).
pub fn set_default_on_startup(enabled: bool) -> Result<()> {
    unsafe {
        let subkey: HSTRING = PREF_SUBKEY.into();
        let value: HSTRING = PREF_VALUE_NAME.into();
        let mut hkey = HKEY::default();
        if RegCreateKeyW(HKEY_CURRENT_USER, &subkey, &mut hkey).is_err() {
            return Err(E_FAIL.into());
        }
        let data: u32 = u32::from(enabled);
        let bytes = data.to_ne_bytes();
        let err = RegSetValueExW(hkey, PCWSTR(value.as_ptr()), Some(0), REG_DWORD, Some(&bytes));
        let _ = RegCloseKey(hkey);
        if err.is_err() {
            return Err(E_FAIL.into());
        }
    }
    Ok(())
}

pub fn unregister_server() -> Result<()> {
    unsafe {
        // wxs `ForceDeleteOnUninstall="yes"` 가 LanguageProfile / Category /
        // TIP root 키를 모두 제거하므로 ITfCategoryMgr::UnregisterCategory /
        // ITfInputProcessorProfiles::Unregister 는 호출하지 않는다 (register
        // 측과 대칭, msctf 충돌 방지).
        unregister_com_server()?;
    }
    Ok(())
}
