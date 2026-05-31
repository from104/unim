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

fn get_dll_path() -> Result<String> {
    let hmodule = crate::dll_instance();
    let mut buf = [0u16; 260];
    let len =
        unsafe {
            windows::Win32::System::LibraryLoader::GetModuleFileNameW(Some(hmodule), &mut buf)
        };
    if len == 0 {
        return Err(E_FAIL.into());
    }
    Ok(String::from_utf16_lossy(&buf[..len as usize]))
}

fn set_reg_value(hkey: HKEY, name: Option<&HSTRING>, value: &str) -> Result<()> {
    let wide: Vec<u16> = value.encode_utf16().chain(std::iter::once(0)).collect();
    unsafe {
        let err = RegSetValueExW(
            hkey,
            name.map(|h| PCWSTR(h.as_ptr())).unwrap_or(PCWSTR::null()),
            Some(0),
            REG_SZ,
            Some(std::slice::from_raw_parts(
                wide.as_ptr() as *const u8,
                wide.len() * 2,
            )),
        );
        if err.is_err() {
            return Err(E_FAIL.into());
        }
    }
    Ok(())
}

/// REG_DWORD 헬퍼 — LanguageProfile 의 Enable / SubstituteLayout / IconIndex 등 정수값 기록.
fn set_reg_dword(hkey: HKEY, name: &HSTRING, value: u32) -> Result<()> {
    let bytes = value.to_ne_bytes();
    unsafe {
        let err = RegSetValueExW(
            hkey,
            PCWSTR(name.as_ptr()),
            Some(0),
            REG_DWORD,
            Some(&bytes),
        );
        if err.is_err() {
            return Err(E_FAIL.into());
        }
    }
    Ok(())
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

/// 진단 로그 ON/OFF 컴파일 상수. `true`(=1) 면 `%TEMP%\unim-tsf.log` 에 남기고,
/// `false`(=0) 면 완전 no-op (릴리스 기본값은 false 권장).
///
/// 리눅스 프런트엔드의 unim_log 처럼 파일에 무조건 남기되, 이 상수로 토글한다.
/// DebugView 등 외부 도구 불필요 — 로그 파일을 직접 열어 본다.
pub(crate) const UNIM_DEBUG_LOG: bool = true;

/// 진단 로그 한 줄을 `%TEMP%\unim-tsf.log` 에 append 한다 (UNIM_DEBUG_LOG=true 일 때).
///
/// compartment / lang_bar / notify_tray 디버깅에서 공용. 실패해도 무시(크래시 없음).
pub(crate) fn dbg_log(msg: &str) {
    if !UNIM_DEBUG_LOG {
        return;
    }
    use std::io::Write;
    let path = std::env::temp_dir().join("unim-tsf.log");
    if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(path) {
        let _ = writeln!(f, "[unim-tsf {}] {}", process_tag(), msg);
    }
}

/// 로그 식별용 짧은 태그 (PID). 여러 앱에 DLL 이 로드되므로 구분에 쓴다.
fn process_tag() -> u32 {
    std::process::id()
}

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
