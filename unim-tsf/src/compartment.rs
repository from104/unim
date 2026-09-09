//! TSF compartment 동기화 — 한/영 상태를 OS 입력 표시기에 반영.
//!
//! UNIM 이 한/영 모드를 바꿀 때 thread-manager 스코프 compartment 2개를 갱신해
//! Windows 입력 표시기(트레이/IME 인디케이터)가 UNIM 의 상태를 알게 한다. 이를
//! 갱신하지 않으면 이전 IME 의 상태 표시기가 잔상으로 남는다.
//!
//! - `GUID_COMPARTMENT_KEYBOARD_OPENCLOSE`           : VT_I4 BOOL  (한글=1, 영문=0)
//! - `GUID_COMPARTMENT_KEYBOARD_INPUTMODE_CONVERSION`: VT_I4 DWORD (한글=NATIVE, 영문=ALPHANUMERIC)
//!
//! SampleIME `Compartment.cpp` 의 `_SetCompartmentBOOL`/`_SetCompartmentDWORD`
//! 시퀀스를 그대로 따른다: thread_mgr → ITfCompartmentMgr → GetCompartment(GUID)
//! → SetValue(tid, &VARIANT{vt=VT_I4, lVal=value}).
//!
//! 실패해도 입력 기능 자체에는 영향이 없으므로 결과를 무시(`let _ =`)하고
//! `dbg_log` 로 HRESULT 만 남긴다. (과거 추측 패치가 전 앱 크래시 회귀를 낸 적
//! 있으므로 절대 panic/early-return 하지 않는다.)

use std::sync::atomic::{AtomicU8, Ordering};

use windows::core::*;
use windows::Win32::System::Variant::{VariantClear, VARIANT, VT_I4};
use windows::Win32::UI::TextServices::{
    ITfCompartmentMgr, ITfContext, ITfThreadMgr, GUID_COMPARTMENT_EMPTYCONTEXT,
    GUID_COMPARTMENT_KEYBOARD_DISABLED, GUID_COMPARTMENT_KEYBOARD_INPUTMODE_CONVERSION,
    GUID_COMPARTMENT_KEYBOARD_OPENCLOSE,
};

use crate::register::dbg_log;

/// TF_CONVERSIONMODE_NATIVE — 한글(네이티브) 변환 모드.
const TF_CONVERSIONMODE_NATIVE: i32 = 0x0001;
/// TF_CONVERSIONMODE_ALPHANUMERIC — 영문 모드.
const TF_CONVERSIONMODE_ALPHANUMERIC: i32 = 0x0000;

/// VT_I4 VARIANT 를 생성한다 (windows-rs 0.62.2 의 중첩 anonymous union 접근).
///
/// 레이아웃: VARIANT.Anonymous(VARIANT_0).Anonymous(ManuallyDrop<VARIANT_0_0>)
///           .{ vt: VARENUM, Anonymous(VARIANT_0_0_0).lVal: i32 }
fn make_i4_variant(value: i32) -> VARIANT {
    let mut var = VARIANT::default();
    unsafe {
        let v00 = &mut *var.Anonymous.Anonymous;
        v00.vt = VT_I4;
        v00.Anonymous.lVal = value;
    }
    var
}

/// 한 compartment 의 VT_I4 값을 set 한다. 실패는 dbg_log 후 무시.
fn set_i4_compartment(
    comp_mgr: &ITfCompartmentMgr,
    tid: u32,
    guid: &GUID,
    value: i32,
    label: &str,
) {
    unsafe {
        match comp_mgr.GetCompartment(guid) {
            Ok(comp) => {
                let var = make_i4_variant(value);
                match comp.SetValue(tid, &var) {
                    Ok(()) => dbg_log(&format!("{} set={} hr=S_OK", label, value)),
                    Err(e) => {
                        dbg_log(&format!("{} set={} hr=0x{:08X}", label, value, e.code().0))
                    }
                }
            }
            Err(e) => dbg_log(&format!(
                "{} GetCompartment FAILED hr=0x{:08X}",
                label,
                e.code().0
            )),
        }
    }
}

/// 한/영 상태를 thread-manager compartment 2개에 반영한다.
///
/// - `thread_mgr`: ActivateEx 에서 받은 ITfThreadMgr.
/// - `tid`: ActivateEx 에서 받은 client id.
/// - `is_korean`: true=한글(open/NATIVE), false=영문(close/ALPHANUMERIC).
///
/// thread_mgr 캐스팅 실패 시 조용히 skip (dbg_log 만).
pub fn sync_keyboard_mode(thread_mgr: &ITfThreadMgr, tid: u32, is_korean: bool) {
    let comp_mgr = match thread_mgr.cast::<ITfCompartmentMgr>() {
        Ok(m) => m,
        Err(e) => {
            dbg_log(&format!(
                "sync_keyboard_mode: cast ITfCompartmentMgr FAILED hr=0x{:08X}",
                e.code().0
            ));
            return;
        }
    };

    let open_close: i32 = i32::from(is_korean);
    let conversion: i32 = if is_korean {
        TF_CONVERSIONMODE_NATIVE
    } else {
        TF_CONVERSIONMODE_ALPHANUMERIC
    };

    set_i4_compartment(
        &comp_mgr,
        tid,
        &GUID_COMPARTMENT_KEYBOARD_OPENCLOSE,
        open_close,
        "OPENCLOSE",
    );
    set_i4_compartment(
        &comp_mgr,
        tid,
        &GUID_COMPARTMENT_KEYBOARD_INPUTMODE_CONVERSION,
        conversion,
        "CONVERSION",
    );
}

// ── 입력 비활성 컨텍스트 판정 (keyboard-disabled) ───────────────────────────
//
// TSF 규약상 호스트는 "지금 이 문서는 입력을 받지 않는다" 를 **컨텍스트 스코프**
// compartment 로 알린다.
//
// - `GUID_COMPARTMENT_KEYBOARD_DISABLED` : VT_I4, 1 = 이 컨텍스트는 키보드 입력 비활성
// - `GUID_COMPARTMENT_EMPTYCONTEXT`      : VT_I4, 1 = 편집 대상이 없는 빈 컨텍스트
//
// Chromium 은 `ui/base/ime/win/tsf_bridge.cc` 의 `InitializeDisabledContext` 에서
// TEXT_INPUT_TYPE_NONE(편집 요소 미포커스) · PASSWORD 일 때 포커스 문서의 컨텍스트에
// 두 값을 1 로 세팅한다. 입력기가 이를 무시하고 문자키를 소비하면 브라우저 단축키
// (스페이스 스크롤 · `/` 검색 · j/k 이동)가 전부 죽는다. weasel(rime) 의
// `WeaselTSF::_IsKeyboardDisabled()` 와 동일한 판정을 구현한다.
//
// 조회 실패(cast/GetCompartment/GetValue HRESULT 에러)는 **활성(false)** 으로 본다 —
// 종전 동작을 유지해 무회귀. 컨텍스트 자체가 없으면(포커스 문서 없음) 비활성(true).

/// 마지막으로 로그에 남긴 상태 (0=미기록, 1=활성, 2=비활성).
/// 매 키 호출되므로 상태가 바뀔 때만 dbg_log 한다.
static LAST_LOGGED_DISABLED: AtomicU8 = AtomicU8::new(0);

/// compartment 하나의 VT_I4 값이 0 이 아닌지 읽는다. 실패는 모두 `false`.
fn read_i4_flag_nonzero(comp_mgr: &ITfCompartmentMgr, guid: &GUID) -> bool {
    unsafe {
        let Ok(comp) = comp_mgr.GetCompartment(guid) else {
            return false;
        };
        // 미설정 compartment 는 S_OK + VT_EMPTY 로 오므로 vt 검사가 곧 게이트다.
        let Ok(mut var) = comp.GetValue() else {
            return false;
        };
        // VariantClear 가 &mut 를 요구하므로 값만 복사해 borrow 를 먼저 끝낸다.
        let (vt, lval) = {
            let inner = &*var.Anonymous.Anonymous;
            (inner.vt, inner.Anonymous.lVal)
        };
        // VT_I4 는 힙 자원이 없지만, 호스트가 다른 vt 를 넣었을 때를 대비해 해제한다.
        let _ = VariantClear(&mut var);
        vt == VT_I4 && lval != 0
    }
}

/// 이 컨텍스트가 키보드 입력 비활성 상태인지 판정한다.
///
/// - `context` 가 `None` → `true` (포커스 문서/컨텍스트 없음 = 입력 대상 없음)
/// - `KEYBOARD_DISABLED` 또는 `EMPTYCONTEXT` 중 하나라도 VT_I4 비-0 → `true`
/// - 그 외(조회 실패 포함) → `false` (활성, 종전 동작)
pub fn context_keyboard_disabled(context: Option<&ITfContext>) -> bool {
    let disabled = match context {
        None => true,
        Some(ctx) => match ctx.cast::<ITfCompartmentMgr>() {
            Ok(comp_mgr) => {
                read_i4_flag_nonzero(&comp_mgr, &GUID_COMPARTMENT_KEYBOARD_DISABLED)
                    || read_i4_flag_nonzero(&comp_mgr, &GUID_COMPARTMENT_EMPTYCONTEXT)
            }
            Err(_) => false,
        },
    };

    let tag = if disabled { 2u8 } else { 1u8 };
    if LAST_LOGGED_DISABLED.swap(tag, Ordering::Relaxed) != tag {
        dbg_log(if disabled {
            "context_keyboard_disabled: DISABLED (키 통과 모드)"
        } else {
            "context_keyboard_disabled: ENABLED (정상 입력)"
        });
    }
    disabled
}
