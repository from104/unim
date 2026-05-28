//! TSF 프로필 등록/해제 + COM InProcServer32 레지스트리 등록
//!
//! 카테고리 등록은 SampleIME 표준 8종을 따른다.
//! GUID/패턴 출처: docs/dev/windows/TSF_RESEARCH_REDESIGN.md (SampleIME 분석)

use windows::core::*;
use windows::Win32::Foundation::*;
use windows::Win32::System::Registry::*;
use windows::Win32::UI::Input::KeyboardAndMouse::HKL;
use windows::Win32::UI::TextServices::*;

use crate::globals::*;

/// SampleIME 표준 8종 — researcher 보고서 docs/dev/windows/TSF_RESEARCH_REDESIGN.md 참조.
///
/// 누락 시 결함:
/// - COMLESS 없음 → conhost/wezterm 등 콘솔에서 IME 활성화 자체가 안 됨 (결함 5)
/// - INPUTMODECOMPARTMENT 없음 → Windows 설정 "옵션" 버튼 미노출 (결함 3)
/// - SECUREMODE 없음 → UAC/비밀번호 필드 호환성 (결함 1 일부)
///
/// register_server / unregister_server 양쪽이 동일 const 를 사용해 드리프트 차단.
const TSF_CATEGORIES: &[GUID] = &[
    GUID_TFCAT_TIP_KEYBOARD,
    GUID_TFCAT_DISPLAYATTRIBUTEPROVIDER,
    GUID_TFCAT_TIPCAP_UIELEMENTENABLED,
    GUID_TFCAT_TIPCAP_SECUREMODE,
    GUID_TFCAT_TIPCAP_COMLESS,
    GUID_TFCAT_TIPCAP_INPUTMODECOMPARTMENT,
    GUID_TFCAT_TIPCAP_IMMERSIVESUPPORT,
    GUID_TFCAT_TIPCAP_SYSTRAYSUPPORT,
];

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
        register_com_server()?;

        let profiles: ITfInputProcessorProfiles = windows::Win32::System::Com::CoCreateInstance(
            &CLSID_TF_InputProcessorProfiles,
            None,
            windows::Win32::System::Com::CLSCTX_INPROC_SERVER,
        )?;

        profiles.Register(&UNIM_CLSID)?;

        let display_name: Vec<u16> = UNIM_IME_NAME
            .encode_utf16()
            .chain(std::iter::once(0))
            .collect();
        profiles.AddLanguageProfile(
            &UNIM_CLSID,
            UNIM_LANGID_KOREAN,
            &UNIM_PROFILE_GUID,
            &display_name,
            &[],
            0,
        )?;

        // ── LanguageProfile 키에 6개 값 직접 기록 ──
        // AddLanguageProfile 이 묵시로 일부를 만들지만, MSI 의 static 키 와
        // 일치시키고 Windows 설정 페이지가 이름/아이콘을 제대로 표시하도록 보강.
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

        let category_mgr: ITfCategoryMgr = windows::Win32::System::Com::CoCreateInstance(
            &CLSID_TF_CategoryMgr,
            None,
            windows::Win32::System::Com::CLSCTX_INPROC_SERVER,
        )?;

        // SampleIME 표준 8종 — 위 TSF_CATEGORIES 정의 참조.
        for cat in TSF_CATEGORIES {
            category_mgr.RegisterCategory(&UNIM_CLSID, cat, &UNIM_CLSID)?;
        }
    }
    Ok(())
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
        if let Ok(category_mgr) = windows::Win32::System::Com::CoCreateInstance::<_, ITfCategoryMgr>(
            &CLSID_TF_CategoryMgr,
            None,
            windows::Win32::System::Com::CLSCTX_INPROC_SERVER,
        ) {
            // SampleIME 표준 8종 — 위 TSF_CATEGORIES 정의 참조 (등록/해제 일치).
            for cat in TSF_CATEGORIES {
                let _ = category_mgr.UnregisterCategory(&UNIM_CLSID, cat, &UNIM_CLSID);
            }
        }

        if let Ok(profiles) =
            windows::Win32::System::Com::CoCreateInstance::<_, ITfInputProcessorProfiles>(
                &CLSID_TF_InputProcessorProfiles,
                None,
                windows::Win32::System::Com::CLSCTX_INPROC_SERVER,
            )
        {
            let _ = profiles.Unregister(&UNIM_CLSID);
        }

        unregister_com_server()?;
    }
    Ok(())
}
