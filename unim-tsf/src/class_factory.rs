//! IClassFactory 구현

use windows::core::*;
use windows::Win32::Foundation::*;
use windows::Win32::System::Com::*;

use crate::text_service::UnimTextService;

#[implement(IClassFactory)]
pub struct UnimClassFactory;

impl UnimClassFactory {
    pub fn new() -> Self {
        Self
    }
}

impl IClassFactory_Impl for UnimClassFactory_Impl {
    fn CreateInstance(
        &self,
        punkouter: Option<&IUnknown>,
        riid: *const GUID,
        ppvobject: *mut *mut core::ffi::c_void,
    ) -> Result<()> {
        unsafe {
            if ppvobject.is_null() {
                return Err(E_INVALIDARG.into());
            }
            *ppvobject = std::ptr::null_mut();

            if punkouter.is_some() {
                return Err(CLASS_E_NOAGGREGATION.into());
            }

            let service = UnimTextService::new();
            let unknown: IUnknown = service.into();
            let hr = unknown.query(&*riid, ppvobject);
            hr.ok()
        }
    }

    fn LockServer(&self, flock: BOOL) -> Result<()> {
        if flock.as_bool() {
            crate::dll_add_ref();
        } else {
            crate::dll_release();
        }
        Ok(())
    }
}
