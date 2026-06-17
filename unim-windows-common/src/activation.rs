//! IMM32 IME 사용자 활성화 (per-user HKL 등록 + 즉시 로드).
//!
//! 진단 보고서 `docs/dev/windows/imm32-diagnosis-report.md` §2-A(BLOCKER) 대응.
//!
//! KakaoTalk(32bit, TSF opt-out)·한컴 등 레거시 앱은 UNIM `.ime` 가 **활성 HKL**
//! 로 로드돼야만 IMM32 폴백으로 입력을 받을 수 있다. 그러나 설치 코드(WiX·.ime
//! SelfRegCost) 어디에도 `HKCU\Keyboard Layout\Preload` 기록/`LoadKeyboardLayoutW`
//! 호출이 없어, KLID `E0200412` 가 어떤 세션에서도 HKL 이 된 적이 없다.
//!
//! 이 모듈은 로그인 유저 컨텍스트로 상주하는 `unim-popup-win.exe` 기동 시
//! [`ensure_imm32_active`] 를 호출해:
//!  1. `Preload` 를 스캔해 KLID 가 이미 등록됐는지(영속) 확인,
//!  2. 없으면 다음 빈 정수 인덱스에 REG_SZ 로 기록(다음 로그인부터 자동 로드/전환기 노출),
//!  3. `LoadKeyboardLayoutW` 로 현재 세션에 즉시 반영한다.
//!
//! idempotent — 매 기동마다 호출해도 무해(이미 있으면 기록 스킵).

use crate::debug::dbg_log;
use windows::core::PCWSTR;
use windows::Win32::Foundation::MAX_PATH;
use windows::Win32::System::Registry::{
    RegCloseKey, RegOpenKeyExW, RegQueryValueExW, RegSetValueExW, HKEY, HKEY_CURRENT_USER,
    KEY_READ, KEY_WRITE, REG_SAM_FLAGS, REG_SZ, REG_VALUE_TYPE,
};
use windows::Win32::UI::Input::KeyboardAndMouse::{
    LoadKeyboardLayoutW, ACTIVATE_KEYBOARD_LAYOUT_FLAGS, KLF_ACTIVATE, KLF_SUBSTITUTE_OK,
};

const LOG_COMPONENT: &str = "unim-activation";
const LOG_FILE: &str = "unim-activation.log";

/// UNIM IMM32 IME 의 KLID.
///
/// `unim-imm32/src/globals.rs` 의 `UNIM_IMM32_KLID` 와 **반드시 동일**해야 한다
/// (unim-imm32 는 cdylib 라 직접 의존 불가 — 리터럴로 동기화). 대소문자는
/// `Preload` 스캔에서 무시하지만, 신규 기록은 표준 대문자 8-hex 로 쓴다.
const UNIM_IMM32_KLID: &str = "E0200412";

/// `HKCU\Keyboard Layout\Preload` 하위 경로.
const PRELOAD_SUBKEY: &str = "Keyboard Layout\\Preload";

fn log(msg: &str) {
    dbg_log(LOG_COMPONENT, LOG_FILE, msg, false);
}

/// 빈 `HKEY`(null 핸들). windows 0.62 의 `HKEY` 는 `Default` 를 파생하지 않아
/// 수동으로 만든다.
fn null_hkey() -> HKEY {
    HKEY(std::ptr::null_mut())
}

/// UNIM IMM32 IME(KLID `E0200412`)를 사용자 활성 입력기로 보장한다.
///
/// - 영속: `HKCU\Keyboard Layout\Preload` 에 KLID 가 없으면 추가(다음 로그인부터
///   Win+Space 전환기 노출·자동 로드).
/// - 즉시: 이번 세션에 `LoadKeyboardLayoutW` 로 HKL 을 로드.
///
/// 실패는 모두 로깅만 하고 무시한다(팝업 렌더러 기동을 막지 않는다).
pub fn ensure_imm32_active() {
    let already = match preload_contains_klid() {
        Ok(found) => found,
        Err(e) => {
            // Preload 조회 실패(예: 키 부재) — 신규로 간주하고 기록 시도.
            log(&format!(
                "preload scan failed ({e}) — treating as not-present, will attempt write"
            ));
            false
        }
    };

    let newly_added = if already {
        log(&format!("preload already contains {UNIM_IMM32_KLID} — skip write"));
        false
    } else {
        match add_klid_to_preload() {
            Ok(idx) => {
                log(&format!(
                    "preload: wrote {UNIM_IMM32_KLID} at index \"{idx}\" (persistent)"
                ));
                true
            }
            Err(e) => {
                log(&format!("preload: write failed ({e})"));
                false
            }
        }
    };

    // 즉시 세션 반영.
    //
    // 플래그 정책(진단 보고서 §5 권장 + 사용자 TSF 선택 비강탈 요구):
    //  - 신규 추가(newly_added) 시에만 KLF_ACTIVATE → 설치 직후 사용자가 즉시
    //    전환기/입력으로 확인 가능. KLF_SUBSTITUTE_OK 는 Substitutes 키 매핑이
    //    있을 때 대체 HKL 을 따르게 해 MS Korean 과의 공존을 깨지 않는다.
    //  - 이미 등록돼 있던(매 기동) 경우엔 KLF_ACTIVATE 를 빼고 로드만 보장 →
    //    사용자가 그 순간 쓰던 TSF/다른 입력기를 매 부팅마다 강탈하지 않는다.
    //    (플래그 0 = HKL 캐시에 로드하되 포그라운드 활성화는 하지 않음.)
    //  - KLF_SETFORPROCESS/KLF_REORDER 는 쓰지 않는다(전역 사용자 선택을
    //    프로세스 한정으로 가두거나 순서를 흔들 수 있어 부적절).
    //
    // ACTIVATE_KEYBOARD_LAYOUT_FLAGS 는 windows 0.62 에서 BitOr 를 구현하지 않아
    // `.0` 비트를 직접 OR 해서 조립한다.
    let flags = if newly_added {
        ACTIVATE_KEYBOARD_LAYOUT_FLAGS(KLF_ACTIVATE.0 | KLF_SUBSTITUTE_OK.0)
    } else {
        ACTIVATE_KEYBOARD_LAYOUT_FLAGS(0)
    };

    let klid_w = to_wide(UNIM_IMM32_KLID);
    // LoadKeyboardLayoutW 는 invalid HKL 이면 Err 를 돌려준다(windows 0.62).
    let hkl = unsafe { LoadKeyboardLayoutW(PCWSTR(klid_w.as_ptr()), flags) };
    match hkl {
        Ok(h) => log(&format!(
            "LoadKeyboardLayoutW({UNIM_IMM32_KLID}, flags=0x{:08X}) -> HKL {:?} (newly_added={newly_added})",
            flags.0, h.0
        )),
        Err(e) => log(&format!(
            "LoadKeyboardLayoutW({UNIM_IMM32_KLID}, flags=0x{:08X}) failed: {e}",
            flags.0
        )),
    }
}

/// `Preload` 의 값들("1","2",…)을 스캔해 KLID 가 이미 있으면 true(대소문자 무시).
fn preload_contains_klid() -> windows::core::Result<bool> {
    let hkey = open_preload(KEY_READ)?;
    let result = (|| -> windows::core::Result<bool> {
        // 인덱스 "1","2",… 를 순차 조회. 첫 결측에서 멈추지 않고 작은 갭은 건너뛴다.
        // (Preload 인덱스는 통상 1부터 연속이나 방어적으로 약간의 갭 허용.)
        let mut missing_streak = 0u32;
        let mut idx = 1u32;
        while missing_streak < 4 {
            match query_string_value(hkey, &idx.to_string()) {
                Some(val) => {
                    missing_streak = 0;
                    if val.eq_ignore_ascii_case(UNIM_IMM32_KLID) {
                        return Ok(true);
                    }
                }
                None => missing_streak += 1,
            }
            idx += 1;
        }
        Ok(false)
    })();
    unsafe {
        let _ = RegCloseKey(hkey);
    }
    result
}

/// 다음 빈 정수 인덱스를 찾아 KLID 를 REG_SZ 로 기록. 기록한 인덱스 문자열을 반환.
fn add_klid_to_preload() -> windows::core::Result<String> {
    let hkey = open_preload(REG_SAM_FLAGS(KEY_READ.0 | KEY_WRITE.0))?;
    let result = (|| -> windows::core::Result<String> {
        // 첫 비어있는 인덱스를 찾는다.
        let mut idx = 1u32;
        while query_string_value(hkey, &idx.to_string()).is_some() {
            idx += 1;
        }
        let name = idx.to_string();
        let name_w = to_wide(&name);
        let value_w = to_wide(UNIM_IMM32_KLID);
        let bytes = unsafe {
            std::slice::from_raw_parts(value_w.as_ptr() as *const u8, value_w.len() * 2)
        };
        // RegSetValueExW 반환은 WIN32_ERROR — `.ok()` 로 Result 변환.
        unsafe {
            RegSetValueExW(
                hkey,
                PCWSTR(name_w.as_ptr()),
                Some(0),
                REG_SZ,
                Some(bytes),
            )
        }
        .ok()?;
        Ok(name)
    })();
    unsafe {
        let _ = RegCloseKey(hkey);
    }
    result
}

/// `HKCU\Keyboard Layout\Preload` 를 주어진 액세스로 연다.
fn open_preload(access: REG_SAM_FLAGS) -> windows::core::Result<HKEY> {
    let sub_w = to_wide(PRELOAD_SUBKEY);
    let mut hkey = null_hkey();
    // RegOpenKeyExW 반환은 WIN32_ERROR — `.ok()` 로 Result 변환.
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

#[cfg(test)]
mod tests {
    use super::*;

    /// KLID 리터럴이 unim-imm32 globals 와 동일한 값으로 고정돼 있는지(드리프트 방지).
    #[test]
    fn klid_literal_is_expected() {
        assert_eq!(UNIM_IMM32_KLID, "E0200412");
    }

    /// 대소문자 무시 비교가 Preload 스캔 의도대로 동작하는지.
    #[test]
    fn klid_case_insensitive_match() {
        assert!("e0200412".eq_ignore_ascii_case(UNIM_IMM32_KLID));
        assert!("E0200412".eq_ignore_ascii_case(UNIM_IMM32_KLID));
        assert!(!"00000412".eq_ignore_ascii_case(UNIM_IMM32_KLID));
    }

    /// to_wide 는 NUL 종단된다.
    #[test]
    fn to_wide_nul_terminated() {
        let w = to_wide("AB");
        assert_eq!(w, vec![0x41, 0x42, 0x00]);
    }

    /// 신규 추가 플래그가 ACTIVATE|SUBSTITUTE_OK 비트를 갖는지(즉시 활성 정책).
    #[test]
    fn newly_added_flags_activate() {
        let flags = ACTIVATE_KEYBOARD_LAYOUT_FLAGS(KLF_ACTIVATE.0 | KLF_SUBSTITUTE_OK.0);
        assert_ne!(flags.0 & KLF_ACTIVATE.0, 0);
        assert_ne!(flags.0 & KLF_SUBSTITUTE_OK.0, 0);
    }

    /// null_hkey 는 null 핸들이다.
    #[test]
    fn null_hkey_is_null() {
        assert!(null_hkey().0.is_null());
    }
}
