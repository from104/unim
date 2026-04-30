//! TSF 프로필 등록/해제 + COM InProcServer32 레지스트리 등록

use windows::core::*;
use windows::Win32::Foundation::*;
use windows::Win32::System::Registry::*;
use windows::Win32::UI::TextServices::*;

use crate::globals::*;

fn get_dll_path() -> Result<String> {
    let hmodule = crate::dll_instance();
    let mut buf = [0u16; 260];
    let len =
        unsafe { windows::Win32::System::LibraryLoader::GetModuleFileNameW(hmodule, &mut buf) };
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
            0,
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

        let category_mgr: ITfCategoryMgr = windows::Win32::System::Com::CoCreateInstance(
            &CLSID_TF_CategoryMgr,
            None,
            windows::Win32::System::Com::CLSCTX_INPROC_SERVER,
        )?;

        for cat in &[
            GUID_TFCAT_TIP_KEYBOARD,
            GUID_TFCAT_DISPLAYATTRIBUTEPROVIDER,
            GUID_TFCAT_TIPCAP_UIELEMENTENABLED,
            GUID_TFCAT_TIPCAP_IMMERSIVESUPPORT,
            GUID_TFCAT_TIPCAP_SYSTRAYSUPPORT,
        ] {
            category_mgr.RegisterCategory(&UNIM_CLSID, cat, &UNIM_CLSID)?;
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
            for cat in &[
                GUID_TFCAT_TIP_KEYBOARD,
                GUID_TFCAT_DISPLAYATTRIBUTEPROVIDER,
                GUID_TFCAT_TIPCAP_UIELEMENTENABLED,
                GUID_TFCAT_TIPCAP_IMMERSIVESUPPORT,
                GUID_TFCAT_TIPCAP_SYSTRAYSUPPORT,
            ] {
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
