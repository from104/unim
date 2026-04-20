//! TSF 프로필 및 COM 등록/해제

use windows::Win32::UI::TextServices::*;
use windows::core::*;

#[allow(unused_imports)]
use crate::globals::*;

/// TSF 입력 프로세서 등록
pub fn register_server() -> Result<()> {
    unsafe {
        // 1. TSF 입력 프로세서 프로필 등록
        let profiles: ITfInputProcessorProfiles =
            windows::Win32::System::Com::CoCreateInstance(
                &CLSID_TF_InputProcessorProfiles,
                None,
                windows::Win32::System::Com::CLSCTX_INPROC_SERVER,
            )?;

        profiles.Register(&UNIM_CLSID)?;

        let display_name: Vec<u16> = UNIM_IME_NAME.encode_utf16().chain(std::iter::once(0)).collect();
        profiles.AddLanguageProfile(
            &UNIM_CLSID,
            UNIM_LANGID_KOREAN,
            &UNIM_PROFILE_GUID,
            &display_name,
            &[],  // icon file
            0,    // icon index
        )?;

        // 2. TSF 카테고리 등록
        let category_mgr: ITfCategoryMgr =
            windows::Win32::System::Com::CoCreateInstance(
                &CLSID_TF_CategoryMgr,
                None,
                windows::Win32::System::Com::CLSCTX_INPROC_SERVER,
            )?;

        let categories = [
            GUID_TFCAT_TIP_KEYBOARD,
            GUID_TFCAT_DISPLAYATTRIBUTEPROVIDER,
            GUID_TFCAT_TIPCAP_UIELEMENTENABLED,
            GUID_TFCAT_TIPCAP_IMMERSIVESUPPORT,
            GUID_TFCAT_TIPCAP_SYSTRAYSUPPORT,
        ];

        for cat in &categories {
            let _ = category_mgr.RegisterCategory(&UNIM_CLSID, cat, &UNIM_CLSID);
        }
    }
    Ok(())
}

/// TSF 입력 프로세서 등록 해제
pub fn unregister_server() -> Result<()> {
    unsafe {
        // 1. TSF 카테고리 해제
        let category_mgr: ITfCategoryMgr =
            windows::Win32::System::Com::CoCreateInstance(
                &CLSID_TF_CategoryMgr,
                None,
                windows::Win32::System::Com::CLSCTX_INPROC_SERVER,
            )?;

        let categories = [
            GUID_TFCAT_TIP_KEYBOARD,
            GUID_TFCAT_DISPLAYATTRIBUTEPROVIDER,
            GUID_TFCAT_TIPCAP_UIELEMENTENABLED,
            GUID_TFCAT_TIPCAP_IMMERSIVESUPPORT,
            GUID_TFCAT_TIPCAP_SYSTRAYSUPPORT,
        ];

        for cat in &categories {
            let _ = category_mgr.UnregisterCategory(&UNIM_CLSID, cat, &UNIM_CLSID);
        }

        // 2. TSF 프로필 해제
        let profiles: ITfInputProcessorProfiles =
            windows::Win32::System::Com::CoCreateInstance(
                &CLSID_TF_InputProcessorProfiles,
                None,
                windows::Win32::System::Com::CLSCTX_INPROC_SERVER,
            )?;

        let _ = profiles.Unregister(&UNIM_CLSID);
    }
    Ok(())
}
