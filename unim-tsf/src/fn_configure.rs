//! ITfFunctionProvider + ITfFnConfigure 구현
//!
//! TSF 프레임워크는 TIP CLSID 로 등록된 ITfFunctionProvider 를 조회한다.
//! GetFunction(GUID_NULL, IID_ITfFnConfigure) → UnimFnConfigure 반환.
//! Windows "설정 → 한국어 → 키보드 옵션" 의 [속성] 버튼이 이 경로를 호출한다.
//!
//! ITfFunctionProvider 는 UnimTextService 의 #[implement] 목록에 추가하는 방식과
//! 별도 객체 방식 모두 가능하다. 여기서는 별도 객체로 구현해 text_service 에서
//! cast() 로 노출한다.

use windows::core::*;
use windows::Win32::Foundation::*;
use windows::Win32::UI::TextServices::*;

use crate::globals;

// ── UnimFnConfigure (ITfFnConfigure + ITfFunction) ───────────────────────────

#[implement(ITfFnConfigure, ITfFunction)]
pub struct UnimFnConfigure;

impl UnimFnConfigure {
    pub fn new() -> Self {
        Self
    }
}

/// ITfFunction: GetDisplayName
impl ITfFunction_Impl for UnimFnConfigure_Impl {
    fn GetDisplayName(&self) -> Result<BSTR> {
        Ok(BSTR::from("UNIM 설정"))
    }
}

/// ITfFnConfigure: Show — 설정 다이얼로그를 모달로 연다.
impl ITfFnConfigure_Impl for UnimFnConfigure_Impl {
    fn Show(
        &self,
        hwndparent: HWND,
        _langid: u16,
        _rguidprofile: *const GUID,
    ) -> Result<()> {
        // 새 Slint 설정 GUI 실행. 실패 시에만 옛 내장 다이얼로그로 폴백.
        if !crate::register::launch_settings_app() {
            crate::settings_dialog::show_settings_dialog(hwndparent);
        }
        Ok(())
    }
}

// ── UnimFunctionProvider (ITfFunctionProvider) ────────────────────────────────

#[implement(ITfFunctionProvider)]
pub struct UnimFunctionProvider;

impl UnimFunctionProvider {
    pub fn new() -> Self {
        Self
    }
}

impl ITfFunctionProvider_Impl for UnimFunctionProvider_Impl {
    /// GetType: 이 provider 가 어느 TIP 에 속하는지 CLSID 반환.
    fn GetType(&self) -> Result<GUID> {
        Ok(globals::UNIM_CLSID)
    }

    /// GetDescription: UI 표시용 문자열.
    fn GetDescription(&self) -> Result<BSTR> {
        Ok(BSTR::from("UNIM Korean IME Function Provider"))
    }

    /// GetFunction: rguid 무시, riid == IID_ITfFnConfigure → UnimFnConfigure 반환.
    /// 그 외 riid 는 E_NOINTERFACE.
    fn GetFunction(
        &self,
        _rguid: *const GUID,
        riid: *const GUID,
    ) -> Result<IUnknown> {
        unsafe {
            if riid.is_null() {
                return Err(E_INVALIDARG.into());
            }
            if *riid == ITfFnConfigure::IID {
                let obj = UnimFnConfigure::new();
                let fn_cfg: ITfFnConfigure = obj.into();
                // ITfFnConfigure: ITfFunction: IUnknown — cast to IUnknown
                let unk: IUnknown = fn_cfg.cast()?;
                Ok(unk)
            } else {
                Err(windows_core::HRESULT(0x80004002_u32 as i32).into()) // E_NOINTERFACE
            }
        }
    }
}
