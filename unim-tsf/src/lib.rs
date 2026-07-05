//! UNIM TSF (Text Services Framework) IME DLL
//!
//! Windows 시스템 전역 한국어 입력기 COM 서버.
//! regsvr32로 등록하면 모든 앱에서 한글 입력 가능.

pub mod globals;

#[cfg(windows)]
mod app_tiers;
#[cfg(windows)]
mod auto_typefix;
#[cfg(windows)]
mod class_factory;
#[cfg(windows)]
mod fn_configure;
#[cfg(windows)]
mod compartment;
#[cfg(windows)]
mod composition;
#[cfg(windows)]
mod crash;
#[cfg(windows)]
mod display_attr;
#[cfg(windows)]
mod input_scope;
#[cfg(windows)]
mod key_handler;
#[cfg(windows)]
mod lang_bar;
#[cfg(windows)]
mod popup_ipc;
#[cfg(windows)]
mod preedit_window;
#[cfg(windows)]
pub mod register;
#[cfg(windows)]
pub mod settings_dialog;
#[cfg(windows)]
mod sandbox_paths;
#[cfg(windows)]
mod synth_input;
#[cfg(windows)]
mod text_service;
#[cfg(windows)]
mod ui_element;

#[cfg(windows)]
use std::sync::atomic::{AtomicUsize, Ordering};
#[cfg(windows)]
use windows::core::{IUnknown, Interface, GUID, HRESULT};
#[cfg(windows)]
use windows::core::BOOL;
#[cfg(windows)]
use windows::Win32::Foundation::{E_FAIL, E_INVALIDARG, HMODULE, S_FALSE, S_OK};
#[cfg(windows)]
const CLASS_E_CLASSNOTAVAILABLE: HRESULT = HRESULT(0x80040111_u32 as i32);

#[cfg(windows)]
static DLL_INSTANCE: std::sync::Mutex<usize> = std::sync::Mutex::new(0);

#[cfg(windows)]
static DLL_REF_COUNT: AtomicUsize = AtomicUsize::new(0);

#[cfg(windows)]
pub fn dll_instance() -> HMODULE {
    HMODULE((*DLL_INSTANCE.lock().unwrap()) as *mut core::ffi::c_void)
}

#[cfg(windows)]
pub fn dll_add_ref() {
    DLL_REF_COUNT.fetch_add(1, Ordering::SeqCst);
}

#[cfg(windows)]
pub fn dll_release() {
    DLL_REF_COUNT.fetch_sub(1, Ordering::SeqCst);
}

#[cfg(windows)]
#[no_mangle]
pub unsafe extern "system" fn DllMain(
    hinst_dll: HMODULE,
    fdw_reason: u32,
    _lpv_reserved: *mut core::ffi::c_void,
) -> BOOL {
    if fdw_reason == 1 {
        // DLL_PROCESS_ATTACH
        *DLL_INSTANCE.lock().unwrap() = hinst_dll.0 as usize;
        // I9 — 크래시 리포팅: panic=abort 하에서 abort 직전에 호출되는 panic hook 을
        // 1회 설치한다. 이로써 COM 경계 너머 패닉이 호스트 앱을 조용히 종료시키기
        // 전에 %APPDATA%\unim\crash\ 에 크래시 마커를 남겨 장애를 UNIM 에 귀속·가시화한다.
        // set_hook 은 경량(RwLock+Box 할당)이라 로더 락 하에서 안전하며, 실제 파일 I/O 는
        // 패닉 시점에만 실행된다. (crate::crash 모듈 문서 참조)
        crash::install_panic_hook();
    }
    BOOL(1)
}

#[cfg(windows)]
#[no_mangle]
pub unsafe extern "system" fn DllGetClassObject(
    rclsid: *const GUID,
    riid: *const GUID,
    ppv: *mut *mut core::ffi::c_void,
) -> HRESULT {
    if ppv.is_null() {
        return E_INVALIDARG;
    }
    *ppv = std::ptr::null_mut();

    if *rclsid != globals::UNIM_CLSID {
        return CLASS_E_CLASSNOTAVAILABLE;
    }

    let factory = class_factory::UnimClassFactory::new();
    let unknown: IUnknown = factory.into();
    unknown.query(&*riid, ppv).into()
}

#[cfg(windows)]
#[no_mangle]
pub extern "system" fn DllCanUnloadNow() -> HRESULT {
    if DLL_REF_COUNT.load(Ordering::SeqCst) == 0 {
        S_OK
    } else {
        S_FALSE
    }
}

#[cfg(windows)]
#[no_mangle]
pub unsafe extern "system" fn DllRegisterServer() -> HRESULT {
    match register::register_server() {
        Ok(()) => S_OK,
        Err(_) => E_FAIL,
    }
}

#[cfg(windows)]
#[no_mangle]
pub unsafe extern "system" fn DllUnregisterServer() -> HRESULT {
    match register::unregister_server() {
        Ok(()) => S_OK,
        Err(_) => E_FAIL,
    }
}
