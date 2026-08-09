use windows::Win32::UI::Input::Ime::{HIMC, HIMCC};
use windows::Win32::Foundation::HWND;
use windows::core::{BOOL, PCWSTR};

// Manual `imm32.dll` FFI (REQUIRED — windows 0.62 omits these IME-side exports).
// imm32 is pulled in by this `#[link]` attribute alone; build.rs does NOT also
// emit `rustc-link-lib=imm32` (that was redundant).
//
// A few decls (ImmGetIMCCLockCount/ImmDestroyIMCC and the dev-only ImmInstallIMEW/
// ImmGetContext/ImmReleaseContext) are retained for completeness but not yet
// called from every build; allow dead_code rather than dropping the contract.
#[allow(dead_code)]
#[link(name = "imm32")]
extern "system" {
    // INPUTCONTEXT lock (returns *mut INPUTCONTEXT)
    pub fn ImmLockIMC(himc: HIMC) -> *mut core::ffi::c_void;
    pub fn ImmUnlockIMC(himc: HIMC) -> BOOL;
    pub fn ImmGetIMCCSize(imcc: HIMCC) -> u32;
    pub fn ImmGetIMCCLockCount(imcc: HIMCC) -> u32;
    pub fn ImmCreateIMCC(size: u32) -> HIMCC;
    pub fn ImmDestroyIMCC(imcc: HIMCC) -> HIMCC;
    pub fn ImmReSizeIMCC(imcc: HIMCC, size: u32) -> HIMCC;
    pub fn ImmLockIMCC(imcc: HIMCC) -> *mut core::ffi::c_void;
    pub fn ImmUnlockIMCC(imcc: HIMCC) -> BOOL;
    pub fn ImmGenerateMessage(himc: HIMC) -> BOOL;
    pub fn ImmInstallIMEW(lpszIMEFileName: PCWSTR, lpszLayoutText: PCWSTR) -> isize; // returns HKL
    pub fn ImmGetContext(hwnd: HWND) -> HIMC;
    pub fn ImmReleaseContext(hwnd: HWND, himc: HIMC) -> BOOL;
}

/// Identity constants

/// KLID for the static keyboard-layout registration. E0xx0412, 0412 = ko-KR.
/// MUST stay identical to installer/wix/unim.wxs.
pub const UNIM_IMM32_KLID: &str = "E0200412";
/// ko-KR LangID. Retained for completeness/dev-registration parity (not yet read
/// at runtime — KLID encodes the LangID for the MSI registry rows).
#[allow(dead_code)]
pub const UNIM_IMM32_LANGID: u16 = 0x0412; // ko-KR
pub const UNIM_IMM32_IME_FILE: &str = "unim_imm32.ime";
pub const UNIM_IME_NAME: &str = "UNIM Korean IME (IMM32)";
/// "Layout File" = 한국어 base keyboard scancode DLL.
/// MS 표준 한국어 레이아웃(KLID 00000412)이 쓰는 것과 동일. 과거 KBDA1.DLL
/// (아랍 자판) 오설정 → 잘못된 스캔코드 매핑. 한국어는 반드시 KBDKOR.DLL.
pub const UNIM_IMM32_LAYOUT_FILE: &str = "KBDKOR.DLL";
/// "Layout Id" — langid(0412)와 충돌하지 않는 free 4-hex 식별자.
pub const UNIM_IMM32_LAYOUT_ID: &str = "00d2";
/// IMM32 레이아웃의 사용자 표시명 폴백(평문 REG_SZ "Layout Text").
/// 인디렉트 리소스 로드 실패 시 OS 가 이 평문을 쓴다. WiX 의 Layout Text 와 동일.
pub const UNIM_LAYOUT_TEXT: &str = "UNIM Korean (IMM32)";
/// IMM32 "Layout Display Name" 인디렉트 문자열(정석).
///
/// SHLoadIndirectString 규약: `@<module>,-<id>` → `LoadString(module, id)`.
/// 음수 절댓값 = STRINGTABLE 리소스 id. `unim_imm32.rc` 의 id 1 → `,-1`.
/// `%SystemRoot%` 를 쓰므로 레지스트리에는 REG_EXPAND_SZ 로 기록한다(MS 한국어
/// 레이아웃과 동일 관례). 표시 문자열 자체는 `unim_imm32.rc` 의 id 1 = UNIM_LAYOUT_TEXT.
/// WiX `installer/wix/unim.wxs`(x64·x86) 의 Layout Display Name 과 반드시 동일해야 함.
pub const UNIM_LAYOUT_DISPLAY_NAME_INDIRECT: &str =
    "@%SystemRoot%\\System32\\unim_imm32.ime,-1";
/// UI window class registered by ui_window.rs and reported from ImeInquire.
pub const UNIM_UI_CLASS_NAME: &str = "UnimImm32UiClass";
/// Per-HIMC private-data size advertised in IMEINFO.dwPrivateDataSize (§4).
pub const UNIM_PRIVATE_DATA_SIZE: u32 = 4; // we key by HIMC in a global map; keep minimal but non-zero.

/// IMEINFO flag values (taken from mozc 2.28.4880 ImeInquire, adapted)
use windows::Win32::UI::Input::Ime::*;
// fdwProperty:
pub const UNIM_FDW_PROPERTY: u32 =
      IME_PROP_UNICODE          // strings are UTF-16
    | IME_PROP_AT_CARET         // composition drawn at caret
    | IME_PROP_KBD_CHAR_FIRST
    | IME_PROP_CANDLIST_START_FROM_1
    | IME_PROP_END_UNLOAD
    | IME_PROP_NEED_ALTKEY;     // (drop IME_PROP_ACCEPT_WIDE_VKEY unless needed)
// fdwConversionCaps: native+full-shape Korean. Start minimal:
pub const UNIM_FDW_CONVERSION_CAPS: u32 = IME_CMODE_NATIVE.0; // (+ IME_CMODE_FULLSHAPE later)
// fdwSentenceCaps:
pub const UNIM_FDW_SENTENCE_CAPS: u32 = 0; // IME_SMODE_NONE
// fdwUICaps:
pub const UNIM_FDW_UI_CAPS: u32 = UI_CAP_2700; // 0x2700-capable UI (no ROT90/SOFTKBD)
// fdwSCSCaps: we build the comp string ourselves; advertise comp + reconversion seed.
pub const UNIM_FDW_SCS_CAPS: u32 = SCS_CAP_COMPSTR | SCS_CAP_SETRECONVERTSTRING;
// fdwSelectCaps:
pub const UNIM_FDW_SELECT_CAPS: u32 = 0; // SELECT_CAP_CONVERSION not advertised yet
