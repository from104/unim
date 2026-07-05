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

use windows::core::*;
use windows::Win32::System::Variant::{VARIANT, VT_I4};
use windows::Win32::UI::TextServices::{
    ITfCompartmentMgr, ITfThreadMgr, GUID_COMPARTMENT_KEYBOARD_INPUTMODE_CONVERSION,
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
