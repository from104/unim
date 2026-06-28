//! TSF InputScope 기반 비밀번호 필드 감지.
//!
//! 포커스 문서의 selection range 에 부여된 `GUID_PROP_INPUTSCOPE` 프로퍼티를
//! 읽어 `ITfInputScope::GetInputScopes` 에 비밀번호/PIN 계열 스코프가 있는지
//! 확인한다. ThreadMgr `OnSetFocus` 에서 호출하며, 어떤 실패도 패닉 금지
//! (panic=abort) 이고 조회 실패/미지정 시 `false`(=normal) 로 graceful 처리한다.
//!
//! 읽기 전용 동기 edit session(`TF_ES_READ | TF_ES_SYNC`)으로 락 안에서 조회하며
//! 문서를 절대 변형하지 않는다(read-only). 시스템콜이므로 엔진 락 밖에서 호출한다.

use std::mem::ManuallyDrop;
use std::sync::{Arc, Mutex};

use windows::core::*;
use windows::Win32::System::Com::CoTaskMemFree;
use windows::Win32::System::Variant::{VariantClear, VARIANT, VT_UNKNOWN};
use windows::Win32::UI::TextServices::*;

/// 포커스 문서가 비밀번호/PIN 계열 입력 필드인지 InputScope 로 판정한다.
///
/// `pdimfocus` 가 없거나(비-TSF 창), 컨텍스트/락/프로퍼티 조회가 실패하면
/// `false`(normal 취급) 를 반환한다.
pub fn focus_is_password(pdimfocus: Option<&ITfDocumentMgr>, tid: u32) -> bool {
    let Some(docmgr) = pdimfocus else {
        return false;
    };
    // 최상위 컨텍스트(GetTop) 우선, 실패 시 GetBase 폴백.
    let context = unsafe {
        match docmgr.GetTop() {
            Ok(c) => c,
            Err(_) => match docmgr.GetBase() {
                Ok(c) => c,
                Err(_) => return false,
            },
        }
    };

    let result = Arc::new(Mutex::new(false));
    let session: ITfEditSession = InputScopeQuerySession {
        context: context.clone(),
        result: result.clone(),
    }
    .into();

    // 읽기 전용 동기 락. TF_ES_SYNC 이므로 DoEditSession 이 호출 내에서 동기 실행된다.
    // 락 거부(TF_E_*)/세션 HRESULT 실패 시 slot 은 false 로 남아 graceful(normal).
    let hr = unsafe { context.RequestEditSession(tid, &session, TF_ES_READ | TF_ES_SYNC) };
    if let Err(e) = hr {
        crate::register::dbg_log(&format!(
            "input_scope: RequestEditSession 거부 hr={e:?} → normal 취급"
        ));
        return false;
    }

    // 락 결과를 bool 로 복사한 뒤 반환. (MutexGuard 임시값을 tail 식에 두면 locals 보다
    // 늦게 드롭돼 `result` 보다 오래 살아 빌림 오류가 나므로 let 으로 즉시 해제한다.)
    let is_password = *result.lock().unwrap();
    is_password
}

/// 읽기 전용 InputScope 조회 edit session (문서 미변형).
#[implement(ITfEditSession)]
struct InputScopeQuerySession {
    context: ITfContext,
    /// 비밀번호/PIN 계열 InputScope 발견 시 `true` 로 기록.
    result: Arc<Mutex<bool>>,
}

impl ITfEditSession_Impl for InputScopeQuerySession_Impl {
    fn DoEditSession(&self, ec: u32) -> Result<()> {
        unsafe {
            // 1. 현재 selection range 획득 (읽기 전용).
            let mut sel = [TF_SELECTION::default(); 1];
            let mut fetched: u32 = 0;
            let got =
                self.context
                    .GetSelection(ec, TF_DEFAULT_SELECTION, &mut sel, &mut fetched);
            if got.is_err() || fetched == 0 {
                return Ok(());
            }
            let Some(range) = ManuallyDrop::take(&mut sel[0].range) else {
                return Ok(());
            };

            // 2. GUID_PROP_INPUTSCOPE 프로퍼티 값(VT_UNKNOWN = ITfInputScope) 조회.
            let Ok(prop) = self.context.GetProperty(&GUID_PROP_INPUTSCOPE) else {
                return Ok(());
            };
            let Ok(mut var) = prop.GetValue(ec, &range) else {
                return Ok(());
            };

            // 3. ITfInputScope::GetInputScopes 에 비밀번호/PIN 계열이 있는지 검사.
            let is_pw = variant_has_password_scope(&var);
            // 받은 VARIANT 소유권 해제 (windows 0.62 VARIANT 는 Drop 미구현 → 수동 해제).
            let _ = VariantClear(&mut var);

            if is_pw {
                *self.result.lock().unwrap() = true;
            }
        }
        Ok(())
    }
}

/// VARIANT(VT_UNKNOWN = ITfInputScope) 에 비밀번호/PIN 계열 InputScope 가 있는지.
///
/// VT_UNKNOWN 이 아니거나 QueryInterface/GetInputScopes 실패 시 `false`.
unsafe fn variant_has_password_scope(var: &VARIANT) -> bool {
    let inner = &*var.Anonymous.Anonymous; // VARIANT_0_0
    if inner.vt != VT_UNKNOWN {
        return false;
    }
    let Some(unk) = inner.Anonymous.punkVal.as_ref() else {
        return false;
    };
    let Ok(input_scope) = unk.cast::<ITfInputScope>() else {
        return false;
    };

    let mut scopes_ptr: *mut InputScope = std::ptr::null_mut();
    let mut count: u32 = 0;
    if input_scope
        .GetInputScopes(&mut scopes_ptr, &mut count)
        .is_err()
    {
        return false;
    }
    if scopes_ptr.is_null() || count == 0 {
        return false;
    }
    let slice = std::slice::from_raw_parts(scopes_ptr, count as usize);
    let found = slice.iter().any(|s| is_password_like_scope(*s));
    // GetInputScopes 가 CoTaskMemAlloc 으로 할당한 배열 해제(메모리 누수 방지).
    CoTaskMemFree(Some(scopes_ptr as *const core::ffi::c_void));
    found
}

/// 한글 입력을 차단해야 하는 비밀번호/PIN 계열 InputScope 인지.
fn is_password_like_scope(scope: InputScope) -> bool {
    scope == IS_PASSWORD
        || scope == IS_NUMERIC_PASSWORD
        || scope == IS_NUMERIC_PIN
        || scope == IS_ALPHANUMERIC_PIN
        || scope == IS_ALPHANUMERIC_PIN_SET
}
