//! 설치 마법사(unim-tsf-settings `--first-run`)용 감지/액션 헬퍼.
//!
//! unim-tsf(cdylib)와 unim-tsf-settings(exe)가 둘 다 쓸 수 있도록 여기(rlib)에 둔다.
//! TSF GUID 는 `unim-tsf/src/globals.rs` 가 `installer/wix/gen-guids.sh` 의 파싱 원본
//! (single source of truth)이라 그쪽에 리터럴로 남겨야 하므로, 마법사 exe 가 쓸 수
//! 있게 여기에 **동일 값을 복제**한다. 값이 절대 바뀌지 않는 상수라 드리프트 위험은
//! 낮지만, globals.rs 를 바꾸면 여기도 반드시 맞춰야 한다(단위테스트로 자기점검).
//!
//! set_as_default() 는 unim-tsf/register.rs 에도 동등 구현이 있다(TSF DLL 자체용,
//! globals.rs 소비). 여기 것은 마법사 exe 전용 — 의도적 중복(unim-tsf 무접촉·회귀 0).

use windows::core::*;
use windows::Win32::Foundation::*;
use windows::Win32::System::Com::*;
use windows::Win32::System::Registry::*;
use windows::Win32::UI::Input::KeyboardAndMouse::{GetKeyboardLayoutList, HKL};
use windows::Win32::UI::Shell::ShellExecuteW;
use windows::Win32::UI::TextServices::*;
use windows::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL;

// ── UNIM TSF GUID (globals.rs 와 동일해야 함 — 아래 test 로 자기점검) ──
const UNIM_CLSID: GUID = GUID::from_u128(0xA1B2C3D4_E5F6_7890_ABCD_EF1234567890);
const UNIM_PROFILE_GUID: GUID = GUID::from_u128(0xB2C3D4E5_F6A7_8901_BCDE_F12345678901);
const UNIM_LANGID_KOREAN: u16 = 0x0412;

// ── 기본 입력기(default profile) 감지/설정 — 사용자 세션 전용 ──
//
// SetDefaultLanguageProfile / ActivateProfile 은 HKCU·현재 세션에 작용하므로 반드시
// 로그인한 사용자 프로세스(= 마법사 exe)에서 호출해야 한다. per-machine MSI 의
// SYSTEM 컨텍스트(deferred CA)에서는 무효.

/// UNIM 이 현재 한국어(0x0412)의 **기본 입력 프로필인지** 조회한다.
///
/// `ITfInputProcessorProfiles::GetDefaultLanguageProfile` 의 (CLSID, profile) 이
/// UNIM 과 일치하면 true. COM 실패·조회 실패 시 **보수적으로 false**(체크박스 노출 →
/// 사용자가 직접 판단). GetDefaultLanguageProfile 이 HKCU Assemblies Default 와 항상
/// 일치하는지는 device-QA 로 확인(TSF 문서 §GetDefault 참조).
pub fn is_default_ime() -> bool {
    unsafe {
        let _ = CoInitializeEx(None, COINIT_APARTMENTTHREADED);
        let profiles: ITfInputProcessorProfiles = match CoCreateInstance(
            &CLSID_TF_InputProcessorProfiles,
            None,
            CLSCTX_INPROC_SERVER,
        ) {
            Ok(p) => p,
            Err(_) => return false,
        };
        let mut clsid = GUID::zeroed();
        let mut profile = GUID::zeroed();
        match profiles.GetDefaultLanguageProfile(
            UNIM_LANGID_KOREAN,
            &GUID_TFCAT_TIP_KEYBOARD,
            &mut clsid,
            &mut profile,
        ) {
            Ok(()) => clsid == UNIM_CLSID && profile == UNIM_PROFILE_GUID,
            Err(_) => false,
        }
    }
}

/// UNIM 을 한국어(0x0412)의 기본 입력 프로필로 지정하고 현재 세션에서 활성화한다.
/// (unim-tsf/register.rs::set_as_default 와 동일 로직 — 마법사 exe 전용 복제.)
pub fn set_as_default() -> Result<()> {
    unsafe {
        let _ = CoInitializeEx(None, COINIT_APARTMENTTHREADED);
        let profiles: ITfInputProcessorProfiles =
            CoCreateInstance(&CLSID_TF_InputProcessorProfiles, None, CLSCTX_INPROC_SERVER)?;
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

// ── 한국어 입력(언어팩) 설치 감지 + 설정 딥링크 ──

/// 사용자에게 한국어 입력이 준비돼 있는지(언어팩/입력 프로필) 휴리스틱 감지.
///
/// 판정: HKCU `Control Panel\International\User Profile` 아래 `ko-KR` 서브키가 있으면
/// 사용자가 한국어를 추가한 것으로 본다. 보조로 로드된 키보드 레이아웃 목록에 langid
/// 0x0412 가 있는지도 본다. 둘 중 하나면 true.
///
/// ⚠ 휴리스틱 — UNIM 자체가 0x0412 TIP 라 레이아웃 목록에 0x0412 를 넣을 수 있어
/// (false-positive 여지), 레지스트리 User Profile 을 1차 신호로 둔다. 최종은 device-QA.
pub fn is_korean_language_installed() -> bool {
    if user_profile_has_korean() {
        return true;
    }
    unsafe {
        let mut list = [HKL::default(); 64];
        let n = GetKeyboardLayoutList(Some(&mut list));
        if n <= 0 {
            return false;
        }
        let n = (n as usize).min(list.len());
        list[..n]
            .iter()
            .any(|hkl| (hkl.0 as usize & 0xFFFF) as u16 == UNIM_LANGID_KOREAN)
    }
}

/// HKCU `Control Panel\International\User Profile` 에 `ko-KR` 서브키 존재 여부.
fn user_profile_has_korean() -> bool {
    unsafe {
        let subkey: HSTRING = "Control Panel\\International\\User Profile\\ko-KR".into();
        let mut hkey = HKEY::default();
        if RegOpenKeyExW(HKEY_CURRENT_USER, &subkey, Some(0), KEY_READ, &mut hkey).is_ok() {
            let _ = RegCloseKey(hkey);
            return true;
        }
        false
    }
}

/// Windows "언어 및 지역" 설정을 연다(한국어 언어팩 추가 안내용 딥링크).
/// 실패해도 무해(마법사가 텍스트 안내를 함께 제공).
pub fn open_language_settings() {
    unsafe {
        let file: HSTRING = "ms-settings:regionlanguage".into();
        let op: HSTRING = "open".into();
        let _ = ShellExecuteW(
            None,
            PCWSTR(op.as_ptr()),
            PCWSTR(file.as_ptr()),
            PCWSTR::null(),
            PCWSTR::null(),
            SW_SHOWNORMAL,
        );
    }
}

// ── 마법사 상태 (HKCU\Software\atit.org\UNIM) ──
//
// "업데이트 시 새 항목만" 판정용. 마법사가 마지막으로 완료된 버전을 REG_SZ 로 기록하고,
// 각 마법사 항목의 introduced_version 과 비교해 seen < introduced 인 항목만 보여준다.

const PREF_SUBKEY: &str = "Software\\atit.org\\UNIM";
const WIZARD_SEEN_VALUE: &str = "WizardSeenVersion";
const DEFAULT_ON_STARTUP_VALUE: &str = "DefaultOnStartup";

/// 마법사가 마지막으로 완료된 버전("0.3.60" 등). 미기록이면 None(= 최초 실행).
pub fn get_wizard_seen_version() -> Option<String> {
    read_hkcu_string(WIZARD_SEEN_VALUE)
}

/// 마법사 완료 버전을 기록한다.
pub fn set_wizard_seen_version(version: &str) -> Result<()> {
    write_hkcu_string(WIZARD_SEEN_VALUE, version)
}

/// "시작 시 기본 입력기로 설정" 선호값(REG_DWORD). unim-tsf/register.rs 와 동일 키.
pub fn get_default_on_startup() -> bool {
    unsafe {
        let subkey: HSTRING = PREF_SUBKEY.into();
        let value: HSTRING = DEFAULT_ON_STARTUP_VALUE.into();
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

/// "시작 시 기본 입력기로 설정" 선호값을 기록한다.
pub fn set_default_on_startup(enabled: bool) -> Result<()> {
    unsafe {
        let subkey: HSTRING = PREF_SUBKEY.into();
        let value: HSTRING = DEFAULT_ON_STARTUP_VALUE.into();
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

fn read_hkcu_string(name: &str) -> Option<String> {
    unsafe {
        let subkey: HSTRING = PREF_SUBKEY.into();
        let value: HSTRING = name.into();
        let mut hkey = HKEY::default();
        if RegOpenKeyExW(HKEY_CURRENT_USER, &subkey, Some(0), KEY_READ, &mut hkey).is_err() {
            return None;
        }
        // 크기 조회 → 버퍼 → 읽기.
        let mut size: u32 = 0;
        let probe = RegQueryValueExW(
            hkey,
            PCWSTR(value.as_ptr()),
            None,
            None,
            None,
            Some(&mut size),
        );
        if probe.is_err() || size == 0 {
            let _ = RegCloseKey(hkey);
            return None;
        }
        let mut buf = vec![0u16; (size as usize / 2) + 1];
        let mut got = size;
        let res = RegQueryValueExW(
            hkey,
            PCWSTR(value.as_ptr()),
            None,
            None,
            Some(buf.as_mut_ptr() as *mut u8),
            Some(&mut got),
        );
        let _ = RegCloseKey(hkey);
        if res.is_err() {
            return None;
        }
        // 널 종단 제거.
        let end = buf.iter().position(|&c| c == 0).unwrap_or(buf.len());
        Some(String::from_utf16_lossy(&buf[..end]))
    }
}

fn write_hkcu_string(name: &str, val: &str) -> Result<()> {
    unsafe {
        let subkey: HSTRING = PREF_SUBKEY.into();
        let value: HSTRING = name.into();
        let mut hkey = HKEY::default();
        if RegCreateKeyW(HKEY_CURRENT_USER, &subkey, &mut hkey).is_err() {
            return Err(E_FAIL.into());
        }
        let mut wide: Vec<u16> = val.encode_utf16().collect();
        wide.push(0);
        let bytes = std::slice::from_raw_parts(wide.as_ptr() as *const u8, wide.len() * 2);
        let err = RegSetValueExW(hkey, PCWSTR(value.as_ptr()), Some(0), REG_SZ, Some(bytes));
        let _ = RegCloseKey(hkey);
        if err.is_err() {
            return Err(E_FAIL.into());
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// globals.rs 의 GUID/LANGID 와 이 복제본이 일치하는지 자기점검.
    /// (globals.rs 는 gen-guids.sh 파싱 원본이라 리터럴을 옮길 수 없어 복제됨.)
    #[test]
    fn guids_match_globals() {
        assert_eq!(
            UNIM_CLSID,
            GUID::from_u128(0xA1B2C3D4_E5F6_7890_ABCD_EF1234567890)
        );
        assert_eq!(
            UNIM_PROFILE_GUID,
            GUID::from_u128(0xB2C3D4E5_F6A7_8901_BCDE_F12345678901)
        );
        assert_eq!(UNIM_LANGID_KOREAN, 0x0412);
    }
}
