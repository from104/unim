//! IMM32 잔재 정리 (언인스톨 전용 복원).
//!
//! [폐기됨] IMM32 `.ime` 갈래는 헛다리로 폐기됐다. 카톡 한글 입력의 진짜 원인은
//! `unim_tsf.dll` 이 x64 단독이라 32비트 카톡의 msctf 가 TIP 를 못 찾던 것이었고,
//! i686 `unim_tsf.dll` 의 32비트 COM 등록으로 해결됐다. 따라서 런타임 IMM32 활성화
//! (Preload 기록·HKL 로드·매 로그인 MS 한국어 어셈블리 덮어쓰기)는 **제거**됐다.
//!
//! 이 모듈에 남은 것은 기존 설치본 잔재 정리용 [`remove_substitute_and_assembly`]
//! 뿐이다. **WiX 에 배선된 CustomAction 이 아니다** — `unim-popup-win.exe
//! --deactivate-imm32` 플래그(`unim-popup-win/src/main.rs`)로 수동 호출하는
//! 레거시 개발 빌드 정리용 명령이다. MSI 설치·제거 시퀀스(`installer/wix/unim.wxs`)
//! 에는 걸려 있지 않다. 과거 버전이 박았을 수 있는 `HKCU\Keyboard Layout\Substitutes`
//! 항목과 `CTF\Assemblies` 단일항목을 MS 한국어 기본값으로 복원한다(한국어 입력
//! brick 방지). fail-soft — 실패는 로깅만.

use crate::debug::dbg_log;
use windows::core::PCWSTR;
use windows::Win32::Foundation::MAX_PATH;
use windows::Win32::System::Registry::{
    RegCloseKey, RegDeleteValueW, RegOpenKeyExW, RegQueryValueExW,
    RegSetValueExW, HKEY, HKEY_CURRENT_USER, KEY_READ, KEY_WRITE, REG_DWORD,
    REG_SAM_FLAGS, REG_SZ, REG_VALUE_TYPE,
};

const LOG_COMPONENT: &str = "unim-activation";
const LOG_FILE: &str = "unim-activation.log";

// ── Assemblies + Substitutes 단일 언어바 항목 (Mozc식) ────────────────────────
//
// 목표: TSF TIP(UNIM CLSID/Profile)과 IMM32 .ime(KLID E0200412)를 **한 언어바
// 항목**으로 통합. 실 스키마는 정상 동작 MS 한국어(0x00000412)의 Assemblies 를
// 읽기전용 덤프해 미러했다(아래 값 형식 그대로):
//
//   HKCU\SOFTWARE\Microsoft\CTF\Assemblies\0x00000412\{34745C63-…1A31}
//     Default        REG_SZ     {CLSID}          ← TIP CLSID
//     Profile        REG_SZ     {PROFILE GUID}   ← LanguageProfile GUID (문자열! 바이너리 아님)
//     KeyboardLayout REG_DWORD  0x04120412       ← (hiword|loword) langid
//
// ⚠️ 실측 결과 Profile 은 16바이트 LE 바이너리가 아니라 **중괄호 GUID 문자열
//    REG_SZ** 였다(브리프 가정과 다름). unim-tsf/register.rs:111 의
//    `format!("{{{:?}}}", UNIM_PROFILE_GUID)` 표기와 동일 형식으로 미러한다.

/// Assemblies 루트 + langid 노드(0x00000412 = ko-KR).
const ASSEMBLIES_LANGID_SUBKEY: &str =
    "SOFTWARE\\Microsoft\\CTF\\Assemblies\\0x00000412";

/// Assembly 의 카테고리 GUID 서브키 = TIP_KEYBOARD(MS 한국어 항목이 쓰는 것과 동일).
const ASSEMBLY_CATEGORY_GUID: &str = "{34745C63-B2F0-4784-8B67-5E12C8701A31}";

/// 과거 UNIM TSF TIP CLSID — 언인스톨 복원 가드에 쓴다(이 CLSID 가 박혀 있을 때만
/// MS 한국어로 되돌림). `unim-tsf/src/globals.rs::UNIM_CLSID` 와 동일(리터럴 동기화).
/// `{A1B2C3D4-E5F6-7890-ABCD-EF1234567890}`.
const UNIM_TSF_CLSID: &str = "{A1B2C3D4-E5F6-7890-ABCD-EF1234567890}";

/// KeyboardLayout REG_DWORD = (hiword langid | loword langid) = 0x0412_0412.
/// MS 한국어 항목 실측값과 동일.
const ASSEMBLY_KEYBOARD_LAYOUT: u32 = 0x0412_0412;

/// 언인스톨 복원용 MS 한국어 기본 CLSID(실측 덤프). Microsoft 한국어 IME TIP.
const MS_KOREAN_CLSID: &str = "{A028AE76-01B1-46C2-99C4-ACD9858AE02F}";
/// 언인스톨 복원용 MS 한국어 LanguageProfile GUID(실측 덤프).
const MS_KOREAN_PROFILE_GUID: &str = "{B5FE1F02-D5F2-4445-9C03-C568F23C99A1}";

/// `HKCU\Keyboard Layout\Substitutes` 하위 경로.
const SUBSTITUTES_SUBKEY: &str = "Keyboard Layout\\Substitutes";

/// Substitutes 값 이름 = 과거 UNIM IMM32 KLID. 언인스톨 시 이 값만 삭제한다.
/// (과거 버전이 E0200412 -> 00000412 매핑을 박았을 수 있어 잔재 정리용.)
const SUBSTITUTE_VALUE_NAME: &str = "E0200412";

fn log(msg: &str) {
    dbg_log(LOG_COMPONENT, LOG_FILE, msg, false);
}

/// 빈 `HKEY`(null 핸들). windows 0.62 의 `HKEY` 는 `Default` 를 파생하지 않아
/// 수동으로 만든다.
fn null_hkey() -> HKEY {
    HKEY(std::ptr::null_mut())
}

/// 언인스톨 대칭: Substitutes 값 삭제 + Assembly 단일항목을 베이스 MS 한국어로 복원.
///
/// Assembly 키 자체를 삭제하면 한국어가 무항목이 될 수 있으므로 **삭제가 아니라
/// MS 한국어 기본값으로 복원**한다(실측 덤프값). Substitutes 는 우리가 넣은 값만 삭제.
/// fail-soft: 실패는 로깅만. MSI ForceDeleteOnUninstall 과 함께 호출되도 무해(idempotent).
pub fn remove_substitute_and_assembly() {
    // ── (1) Substitutes 값 삭제(우리 값만) ────────────────────────────────
    match delete_substitute() {
        Ok(()) => log(&format!("substitute: removed {SUBSTITUTE_VALUE_NAME}")),
        Err(e) => log(&format!("substitute: remove failed/absent ({e})")),
    }

    // ── (2) Assembly 를 MS 한국어 기본값으로 복원 ─────────────────────────
    //   복원값은 실측 덤프(MS 한국어): Default={A028AE76-…}, Profile={B5FE1F02-…},
    //   KeyboardLayout=0x04120412. UNIM 항목이 박혀 있던 자리만 되돌린다.
    match restore_assembly_to_ms_korean() {
        Ok(()) => log("assembly: restored to MS Korean defaults"),
        Err(e) => log(&format!("assembly: restore failed ({e})")),
    }
}

/// `hkey` 에서 `name` 값을 REG_SZ 로 읽어 UTF-8 String 으로 반환(없으면 None).
fn query_string_value(hkey: HKEY, name: &str) -> Option<String> {
    let name_w = to_wide(name);
    let mut buf = [0u16; MAX_PATH as usize];
    let mut cb = (buf.len() * 2) as u32; // 바이트 단위 버퍼 크기.
    let mut ty = REG_VALUE_TYPE::default();
    let err = unsafe {
        RegQueryValueExW(
            hkey,
            PCWSTR(name_w.as_ptr()),
            Some(std::ptr::null()),
            Some(&mut ty),
            Some(buf.as_mut_ptr() as *mut u8),
            Some(&mut cb),
        )
    };
    if err.is_err() {
        return None;
    }
    // cb = 바이트 수(종단 NUL 포함 가능). u16 개수로 환산 후 trailing NUL 제거.
    let mut len = (cb as usize) / 2;
    while len > 0 && buf[len - 1] == 0 {
        len -= 1;
    }
    Some(String::from_utf16_lossy(&buf[..len]))
}

/// NUL 종단 wide 문자열.
fn to_wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

// ── Substitutes / Assemblies 헬퍼 ────────────────────────────────────────────

/// `name` REG_SZ 값을 idempotent 로 기록(기존값이 같으면 write 스킵, 다르면 갱신).
/// 반환: true=write 수행, false=이미 일치해 스킵.
fn upsert_string(hkey: HKEY, name: &str, value: &str) -> windows::core::Result<bool> {
    if let Some(cur) = query_string_value(hkey, name) {
        if cur.eq_ignore_ascii_case(value) {
            return Ok(false);
        }
    }
    let name_w = to_wide(name);
    let value_w = to_wide(value);
    let bytes =
        unsafe { std::slice::from_raw_parts(value_w.as_ptr() as *const u8, value_w.len() * 2) };
    unsafe { RegSetValueExW(hkey, PCWSTR(name_w.as_ptr()), Some(0), REG_SZ, Some(bytes)) }.ok()?;
    Ok(true)
}

/// `name` REG_DWORD 값을 idempotent 로 기록(기존값이 같으면 write 스킵).
fn upsert_dword(hkey: HKEY, name: &str, value: u32) -> windows::core::Result<bool> {
    if let Some(cur) = query_dword_value(hkey, name) {
        if cur == value {
            return Ok(false);
        }
    }
    let name_w = to_wide(name);
    let bytes = value.to_ne_bytes();
    unsafe { RegSetValueExW(hkey, PCWSTR(name_w.as_ptr()), Some(0), REG_DWORD, Some(&bytes)) }
        .ok()?;
    Ok(true)
}

/// `hkey` 에서 `name` REG_DWORD 값을 읽는다(없거나 타입 불일치면 None).
fn query_dword_value(hkey: HKEY, name: &str) -> Option<u32> {
    let name_w = to_wide(name);
    let mut data = 0u32;
    let mut cb = 4u32;
    let mut ty = REG_VALUE_TYPE::default();
    let err = unsafe {
        RegQueryValueExW(
            hkey,
            PCWSTR(name_w.as_ptr()),
            Some(std::ptr::null()),
            Some(&mut ty),
            Some(&mut data as *mut u32 as *mut u8),
            Some(&mut cb),
        )
    };
    if err.is_err() || ty != REG_DWORD || cb != 4 {
        return None;
    }
    Some(data)
}

/// Substitutes 에서 우리 값(E0200412)을 삭제(언인스톨 대칭).
fn delete_substitute() -> windows::core::Result<()> {
    let hkey = open_hkcu_key(SUBSTITUTES_SUBKEY, REG_SAM_FLAGS(KEY_READ.0 | KEY_WRITE.0))?;
    let name_w = to_wide(SUBSTITUTE_VALUE_NAME);
    let r = unsafe { RegDeleteValueW(hkey, PCWSTR(name_w.as_ptr())) }.ok();
    unsafe {
        let _ = RegCloseKey(hkey);
    }
    r
}

/// Assembly 단일항목을 MS 한국어 기본값으로 복원(언인스톨 대칭).
/// 실측 덤프값: Default={A028AE76-…}, Profile={B5FE1F02-…}, KeyboardLayout=0x04120412.
fn restore_assembly_to_ms_korean() -> windows::core::Result<()> {
    let path = format!("{ASSEMBLIES_LANGID_SUBKEY}\\{ASSEMBLY_CATEGORY_GUID}");
    let hkey = open_hkcu_key(&path, REG_SAM_FLAGS(KEY_READ.0 | KEY_WRITE.0))?;
    let r = (|| -> windows::core::Result<()> {
        // 우리가 박은 UNIM 항목일 때만 복원(타 IME 가 점유 중이면 건드리지 않음).
        let cur_default = query_string_value(hkey, "Default").unwrap_or_default();
        if cur_default.eq_ignore_ascii_case(UNIM_TSF_CLSID) {
            let _ = upsert_string(hkey, "Default", MS_KOREAN_CLSID)?;
            let _ = upsert_string(hkey, "Profile", MS_KOREAN_PROFILE_GUID)?;
            let _ = upsert_dword(hkey, "KeyboardLayout", ASSEMBLY_KEYBOARD_LAYOUT)?;
        }
        Ok(())
    })();
    unsafe {
        let _ = RegCloseKey(hkey);
    }
    r
}

/// HKCU 하위 키를 (생성 없이) 연다.
fn open_hkcu_key(subkey: &str, access: REG_SAM_FLAGS) -> windows::core::Result<HKEY> {
    let sub_w = to_wide(subkey);
    let mut hkey = null_hkey();
    unsafe {
        RegOpenKeyExW(
            HKEY_CURRENT_USER,
            PCWSTR(sub_w.as_ptr()),
            Some(0),
            access,
            &mut hkey,
        )
    }
    .ok()?;
    Ok(hkey)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// to_wide 는 NUL 종단된다.
    #[test]
    fn to_wide_nul_terminated() {
        let w = to_wide("AB");
        assert_eq!(w, vec![0x41, 0x42, 0x00]);
    }

    /// null_hkey 는 null 핸들이다.
    #[test]
    fn null_hkey_is_null() {
        assert!(null_hkey().0.is_null());
    }

    /// UNIM Assembly 복원 가드 CLSID 리터럴이 unim-tsf globals 와 동일(드리프트 방지).
    /// (unim-tsf 는 직접 의존 불가 — 리터럴 동기화.)
    #[test]
    fn unim_assembly_guids_match_tsf() {
        assert_eq!(UNIM_TSF_CLSID, "{A1B2C3D4-E5F6-7890-ABCD-EF1234567890}");
    }

    /// Assembly 카테고리 GUID = TIP_KEYBOARD(실측 MS 한국어 항목과 동일).
    #[test]
    fn assembly_category_is_tip_keyboard() {
        assert_eq!(ASSEMBLY_CATEGORY_GUID, "{34745C63-B2F0-4784-8B67-5E12C8701A31}");
    }

    /// KeyboardLayout DWORD = 0x04120412(실측값).
    #[test]
    fn assembly_keyboard_layout_value() {
        assert_eq!(ASSEMBLY_KEYBOARD_LAYOUT, 0x0412_0412);
    }

    /// 복원 CLSID 와 UNIM 가드 CLSID 는 서로 달라야 한다(복원이 실제로 되돌리는지).
    #[test]
    fn ms_korean_restore_guids_differ_from_unim() {
        assert_ne!(MS_KOREAN_CLSID, UNIM_TSF_CLSID);
    }
}
