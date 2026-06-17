//! IMM32 키보드 레이아웃 등록/해제 + 개발용 헬퍼.
//!
//! 1차 설치 경로는 MSI(`installer/wix/unim.wxs`)의 static 레지스트리 블록이다.
//! 본 모듈은 `cargo`-driven 개발 루프용 in-crate 헬퍼를 제공한다.
//!
//! ## 등록 방식
//! IMM32 IME는 TSF CTF/COM과 달리 키보드 레이아웃 레지스트리 키 하나로 등록된다.
//!   `HKLM\SYSTEM\CurrentControlSet\Control\Keyboard Layouts\<KLID>`
//! KLID = `E0200412` (E0xx = IME 레이아웃, 0412 = ko-KR). 이 값은
//! `globals.rs::UNIM_IMM32_KLID` 및 `installer/wix/unim.wxs` 와 동일해야 한다.
//!
//! `ImmInstallIMEW`는 레이아웃을 동적 KLID로 추가하며 반환값이 불투명(HKL)이라
//! 고정 KLID를 보장하지 않는다. 따라서 고정 KLID 행은 직접 `RegCreateKeyW`로
//! 기록하고, `ImmInstallIMEW`는 그 뒤 OS 내부 캐시 갱신 용도로 호출한다.
//!
//! ## 해제
//! `UnloadKeyboardLayout`으로 현재 세션에서 제거 후 KLID 키를 삭제한다.

use windows::Win32::Foundation::*;
use windows::Win32::System::Registry::*;
use windows::Win32::UI::Input::KeyboardAndMouse::*;
use windows::core::*;

use unim_windows_common::registry::set_reg_value;

use crate::globals::{
    UNIM_IMM32_IME_FILE, UNIM_IMM32_KLID, UNIM_IMM32_LAYOUT_FILE, UNIM_IMM32_LAYOUT_ID,
    UNIM_LAYOUT_TEXT,
};

// ── 내부 헬퍼 ──────────────────────────────────────────────────────────────

/// 이 DLL(.ime)의 파일 시스템 경로를 반환한다.
/// `DllMain`에서 저장한 HINSTANCE(isize)를 통해 `GetModuleFileNameW`로 읽는다.
fn get_dll_path() -> Result<String> {
    let raw = crate::ime_state::hinst();
    let hmodule = windows::Win32::Foundation::HMODULE(raw as *mut core::ffi::c_void);
    unim_windows_common::registry::get_module_path(hmodule)
}

// ── 공개 API ───────────────────────────────────────────────────────────────

/// 개발용: `.ime`를 설치하고 IMM32 키보드 레이아웃 레지스트리 행을 기록한다.
///
/// 1차 설치 경로는 MSI다. 본 함수는 `cargo`-driven 개발 루프에서 `regsvr32`
/// 없이 IME를 등록하기 위한 헬퍼다.
///
/// ### 수행 순서
/// 1. `HKLM\SYSTEM\CurrentControlSet\Control\Keyboard Layouts\E0200412` 키를
///    생성하고 5개 값을 기록한다.
/// 2. 보조적으로 `ImmInstallIMEW`를 호출해 OS 내부 레이아웃 캐시를 갱신한다
///    (실패해도 오류로 처리하지 않음 — 레지스트리 행이 권위(authority)다).
pub fn register_ime() -> windows::core::Result<()> {
    let dll_path = get_dll_path()?;
    dbg_log(&format!("register_ime: dll_path={}", dll_path));

    // ── 1. 고정 KLID 레지스트리 행 기록 ─────────────────────────────────
    //   HKLM\SYSTEM\CurrentControlSet\Control\Keyboard Layouts\E0200412
    let klid_key_path: HSTRING = format!(
        "SYSTEM\\CurrentControlSet\\Control\\Keyboard Layouts\\{}",
        UNIM_IMM32_KLID
    )
    .into();

    let mut hkey = HKEY::default();
    unsafe {
        let err = RegCreateKeyW(HKEY_LOCAL_MACHINE, &klid_key_path, &mut hkey);
        if err.is_err() {
            dbg_log(&format!(
                "register_ime: RegCreateKeyW failed for {}",
                UNIM_IMM32_KLID
            ));
            return Err(E_FAIL.into());
        }
    }

    // "Ime File" = "unim_imm32.ime"
    let name_ime_file: HSTRING = "Ime File".into();
    set_reg_value(hkey, Some(&name_ime_file), UNIM_IMM32_IME_FILE)?;

    // "Layout File" = "KBDKOR.DLL"  (Korean base keyboard scancode DLL,
    //   MS 표준 한국어 레이아웃 00000412 와 동일. 과거 KBDA1.DLL=아랍 자판 오기였음.)
    let name_layout_file: HSTRING = "Layout File".into();
    set_reg_value(hkey, Some(&name_layout_file), UNIM_IMM32_LAYOUT_FILE)?;

    // "Layout Text" = "UNIM Korean (IMM32)"
    let name_layout_text: HSTRING = "Layout Text".into();
    set_reg_value(hkey, Some(&name_layout_text), UNIM_LAYOUT_TEXT)?;

    // "Layout Display Name" — 평문 REG_SZ 로 기록.
    //   과거에는 인디렉트 문자열(@...unim_imm32.ime,-1000)을 썼으나 .ime 에 해당
    //   STRINGTABLE 리소스가 없어 SHLoadIndirectString 이 빈 문자열을 반환 → 표시명이
    //   비어 보이는 결함이 있었다. SHLoadIndirectString 은 '@' 없는 평문은 그대로
    //   반환하므로, 평문 UNIM_LAYOUT_TEXT 로 기록해 표시명을 보장한다.
    let name_display: HSTRING = "Layout Display Name".into();
    set_reg_value(hkey, Some(&name_display), UNIM_LAYOUT_TEXT)?;

    // "Layout Id" — langid(0412)와 충돌하지 않는 free 4-hex 식별자.
    let name_layout_id: HSTRING = "Layout Id".into();
    set_reg_value(hkey, Some(&name_layout_id), UNIM_IMM32_LAYOUT_ID)?;

    unsafe {
        let _ = RegCloseKey(hkey);
    }

    dbg_log(&format!(
        "register_ime: wrote KLID key HKLM\\...\\{}",
        UNIM_IMM32_KLID
    ));

    // ── 2. ImmInstallIMEW — OS 내부 캐시 갱신 (보조, 실패 무시) ─────────
    //   ImmInstallIMEW는 동적 KLID를 반환하므로 권위 있는 KLID로 사용하지 않는다.
    //   단, 호출 자체가 mmctf/win32k 내부 레이아웃 목록을 갱신하는 부작용이 있어
    //   세션 재시작 없이 IME가 보이도록 돕는다.
    unsafe {
        let ime_file_wide: Vec<u16> = UNIM_IMM32_IME_FILE
            .encode_utf16()
            .chain(std::iter::once(0))
            .collect();
        let layout_text_wide: Vec<u16> = UNIM_LAYOUT_TEXT
            .encode_utf16()
            .chain(std::iter::once(0))
            .collect();
        let hkl = crate::globals::ImmInstallIMEW(
            PCWSTR(ime_file_wide.as_ptr()),
            PCWSTR(layout_text_wide.as_ptr()),
        );
        if hkl == 0 {
            dbg_log("register_ime: ImmInstallIMEW returned null (non-fatal; registry rows are authority)");
        } else {
            dbg_log(&format!("register_ime: ImmInstallIMEW -> HKL 0x{:08X}", hkl));
        }
    }

    dbg_log("register_ime: done");
    Ok(())
}

/// 개발용: 현재 세션에서 키보드 레이아웃을 언로드하고 KLID 레지스트리 키를 삭제한다.
///
/// `UnloadKeyboardLayout`은 현재 세션에 로드된 레이아웃 인스턴스를 언로드한다.
/// KLID 키 삭제는 다음 세션부터 레이아웃이 보이지 않도록 한다.
pub fn unregister_ime() -> windows::core::Result<()> {
    dbg_log("unregister_ime: start");

    // ── 1. 현재 세션에서 언로드 ─────────────────────────────────────────
    //   KLID 문자열 -> HKL 는 LoadKeyboardLayoutW("E0200412", KLF_SUBSTITUTE_OK)로
    //   얻는다. 실패해도 레지스트리 삭제는 계속한다.
    unsafe {
        let klid_wide: Vec<u16> = UNIM_IMM32_KLID
            .encode_utf16()
            .chain(std::iter::once(0))
            .collect();
        // KLF_SUBSTITUTE_OK = 0x00000002
        // windows-0.62: LoadKeyboardLayoutW returns Result<HKL>; unwrap before use.
        match LoadKeyboardLayoutW(PCWSTR(klid_wide.as_ptr()), KLF_SUBSTITUTE_OK) {
            Ok(hkl) if !hkl.is_invalid() => {
                // windows-0.62: UnloadKeyboardLayout returns Result<()>, not BOOL.
                match UnloadKeyboardLayout(hkl) {
                    Ok(()) => dbg_log(&format!(
                        "unregister_ime: UnloadKeyboardLayout(0x{:p}) ok",
                        hkl.0
                    )),
                    Err(_) => {
                        dbg_log("unregister_ime: UnloadKeyboardLayout failed (non-fatal)")
                    }
                }
            }
            _ => {
                dbg_log("unregister_ime: LoadKeyboardLayoutW returned null/err (layout not loaded or already removed)");
            }
        }
    }

    // ── 2. KLID 키 삭제 ─────────────────────────────────────────────────
    let klid_key_path: HSTRING = format!(
        "SYSTEM\\CurrentControlSet\\Control\\Keyboard Layouts\\{}",
        UNIM_IMM32_KLID
    )
    .into();

    unsafe {
        // RegDeleteKeyW는 하위 키가 없을 때만 성공한다. KLID 키는 값만 있고
        // 하위 키가 없으므로 직접 삭제 가능하다.
        let err = RegDeleteKeyW(HKEY_LOCAL_MACHINE, &klid_key_path);
        if err.is_err() {
            dbg_log(&format!(
                "unregister_ime: RegDeleteKeyW failed for {} (may already be absent)",
                UNIM_IMM32_KLID
            ));
            // 키가 이미 없으면 오류로 취급하지 않는다.
        } else {
            dbg_log(&format!(
                "unregister_ime: deleted HKLM\\...\\{}",
                UNIM_IMM32_KLID
            ));
        }
    }

    dbg_log("unregister_ime: done");
    Ok(())
}

// ── 진단 로그 ──────────────────────────────────────────────────────────────

/// 진단 로그 ON/OFF 컴파일 상수.
/// 릴리스 빌드에서는 항상 `false`(no-op): 모든 호스트 프로세스에서 키 입력마다
/// `%TEMP%\unim-imm32.log` 에 기록되는 perf/PII 문제를 막는다(reviewer major).
/// 디버그 빌드에서만 켜진다.
pub(crate) const UNIM_DEBUG_LOG: bool = cfg!(debug_assertions);

/// 진단 로그 한 줄을 `OutputDebugStringW`와 `%TEMP%\unim-imm32.log`에 기록한다.
///
/// `UNIM_DEBUG_LOG = false`이면 완전 no-op. 실패해도 무시(크래시 없음).
pub fn dbg_log(msg: &str) {
    if !UNIM_DEBUG_LOG {
        return;
    }
    unim_windows_common::debug::dbg_log("unim-imm32", "unim-imm32.log", msg, true);
}
