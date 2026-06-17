//! unim-imm32 — UNIM Korean IME as a Windows IMM32 `.ime` DLL (owner: core).
//!
//! This file owns the **export table**: `DllMain` + all 17 `#[no_mangle] extern
//! "system"` IMM32 entry points (listed undecorated in `unim_imm32.def`). Each
//! export is a thin dispatcher into the core modules:
//!   - [`ime_state`]   — per-HIMC engine state (§4)
//!   - [`input`]       — VK→KeyCode, modifier state, should-consume probe (§5.1)
//!   - [`composition`] — COMPOSITIONSTRING IMCC build + transmsg emit (§5)
//!   - [`ui_window`]   — UI window class + `UIWndProc` body (owner: ui)
//!   - [`register`]    — dev install/uninstall + `dbg_log` (owner: register)
//!   - [`globals`]     — identity consts, IMEINFO flags, imm32 FFI (owner: scaffold)
//!
//! Strings are UTF-16 (`IME_PROP_UNICODE`). Builds for BOTH x86_64 and i686:
//! all `WPARAM/LPARAM` payloads use `usize`/`isize` and IMCC offsets are computed
//! with `offset_of!`, so bitness is handled structurally.

#![cfg(windows)]

mod composition;
mod globals;
mod ime_state;
mod input;
// `register` mirrors the MSI install path for `cargo`-driven dev loops (DESIGN.md
// §8). The MSI's static registry block is the AUTHORITATIVE install path. For a
// real `cargo`-driven dev loop (no MSI), we ALSO expose the standard
// `DllRegisterServer`/`DllUnregisterServer` exports below so `regsvr32
// unim_imm32.ime` can write/remove the KLID rows. This is a DEV convenience, not
// COM self-registration: IMM32 IMEs are registry-registered (the exports just
// call `register::register_ime`/`unregister_ime`, which write the same
// `Keyboard Layouts\E0200412` rows the MSI installs). DESIGN.md §10 "NO
// SelfRegCost" still holds for the MSI — it does NOT invoke these; only manual
// `regsvr32` does. Exporting these also removes the dead-code status of the
// `register` module (it is now reachable from the export table).
mod register;
mod ui_window;

use core::ffi::c_void;
use core::mem::size_of;

use windows::core::{BOOL, HRESULT, PWSTR};
use windows::Win32::Foundation::{FALSE, HINSTANCE, HMODULE, HWND, LPARAM, TRUE, WPARAM};
use windows::Win32::System::LibraryLoader::DisableThreadLibraryCalls;
use windows::Win32::UI::Input::Ime::{
    CANDIDATELIST, IMEINFO, IMEMENUITEMINFOW, REGISTERWORDW, STYLEBUFW, TRANSMSGLIST,
    CPS_CANCEL, CPS_COMPLETE, IME_ESC_IME_NAME, NI_COMPOSITIONSTR,
    HIMC,
};
use windows::Win32::UI::Input::KeyboardAndMouse::HKL;

use globals::*;

// ===========================================================================
// DllMain
// ===========================================================================

const DLL_PROCESS_ATTACH: u32 = 1;

/// DLL entry point. On `DLL_PROCESS_ATTACH` we capture the module handle and
/// disable per-thread notifications (we have no thread-local state).
#[no_mangle]
pub unsafe extern "system" fn DllMain(
    hinst: HMODULE,
    reason: u32,
    _reserved: *mut c_void,
) -> BOOL {
    if reason == DLL_PROCESS_ATTACH {
        ime_state::set_hinst(hinst.0 as isize);
        let _ = DisableThreadLibraryCalls(hinst);
    }
    TRUE
}

// ===========================================================================
// Lifecycle / inquiry
// ===========================================================================

/// Fill `IMEINFO` and report the UI window class name. The first thing CUAS calls.
#[no_mangle]
pub unsafe extern "system" fn ImeInquire(
    lp_ime_info: *mut IMEINFO,
    lpsz_wnd_class: PWSTR,
    _dw_system_info_flags: u32,
) -> BOOL {
    if lp_ime_info.is_null() {
        return FALSE;
    }

    // Zero then fill, in the order documented in §3.3.
    core::ptr::write_bytes(lp_ime_info as *mut u8, 0, size_of::<IMEINFO>());
    let info = &mut *lp_ime_info;
    info.dwPrivateDataSize = UNIM_PRIVATE_DATA_SIZE;
    info.fdwProperty = UNIM_FDW_PROPERTY;
    info.fdwConversionCaps = UNIM_FDW_CONVERSION_CAPS;
    info.fdwSentenceCaps = UNIM_FDW_SENTENCE_CAPS;
    info.fdwUICaps = UNIM_FDW_UI_CAPS;
    info.fdwSCSCaps = UNIM_FDW_SCS_CAPS;
    info.fdwSelectCaps = UNIM_FDW_SELECT_CAPS;

    // Copy the UI class name (UTF-16, NUL-terminated) into the caller buffer.
    // The buffer is sized for a class name (IMM32 guarantees >= the class name
    // capacity); UNIM_UI_CLASS_NAME is short and ASCII.
    if !lpsz_wnd_class.is_null() {
        let dst = lpsz_wnd_class.0;
        let mut i = 0isize;
        for u in UNIM_UI_CLASS_NAME.encode_utf16() {
            *dst.offset(i) = u;
            i += 1;
        }
        *dst.offset(i) = 0; // NUL terminator
    }

    // Make sure the UI class is registered (idempotent).
    let hinst = HINSTANCE(ime_state::hinst() as *mut c_void);
    let _ = ui_window::ensure_ui_class_registered(hinst);

    TRUE
}

/// Process unload cleanup. Return TRUE when `uReserved == 0`.
#[no_mangle]
pub unsafe extern "system" fn ImeDestroy(u_reserved: u32) -> BOOL {
    if u_reserved == 0 {
        TRUE
    } else {
        FALSE
    }
}

/// IME escape function. Handle `IME_ESC_IME_NAME` (write the IME display name);
/// everything else returns 0 (FALSE) honestly.
#[no_mangle]
pub unsafe extern "system" fn ImeEscape(
    _h_imc: HIMC,
    u_sub_func: u32,
    lp_data: *mut c_void,
) -> isize {
    if u_sub_func == IME_ESC_IME_NAME.0 && !lp_data.is_null() {
        // Caller buffer is >= 64 wchars (IMM32 contract for IME_ESC_IME_NAME).
        let dst = lp_data as *mut u16;
        let mut i = 0isize;
        for u in UNIM_IME_NAME.encode_utf16().take(63) {
            *dst.offset(i) = u;
            i += 1;
        }
        *dst.offset(i) = 0;
        return TRUE.0 as isize;
    }
    0
}

/// Open IME settings. Deferred — return FALSE honestly rather than fake success.
#[no_mangle]
pub unsafe extern "system" fn ImeConfigure(
    _h_kl: HKL,
    _h_wnd: HWND,
    _dw_mode: u32,
    _lp_data: *mut c_void,
) -> BOOL {
    // TODO(unim-imm32): launch the UNIM settings app.
    FALSE
}

// ===========================================================================
// Context binding
// ===========================================================================

/// Create/bind (fSelect=TRUE) or reset+drop (fSelect=FALSE) the per-HIMC engine.
#[no_mangle]
pub unsafe extern "system" fn ImeSelect(h_imc: HIMC, f_select: BOOL) -> BOOL {
    if f_select.0 != 0 {
        // Ensure the UI class exists before any composition starts.
        let hinst = HINSTANCE(ime_state::hinst() as *mut c_void);
        let _ = ui_window::ensure_ui_class_registered(hinst);
        ime_state::on_select(h_imc);
    } else {
        ime_state::on_unselect(h_imc);
    }
    TRUE
}

/// Focus change. On deactivate, flush/hide; keep the binding.
#[no_mangle]
pub unsafe extern "system" fn ImeSetActiveContext(h_imc: HIMC, f_activate: BOOL) -> BOOL {
    if f_activate.0 == 0 {
        ime_state::on_deactivate(h_imc);
    }
    TRUE
}

// ===========================================================================
// Key path (the crux)
// ===========================================================================

/// PURE should-consume probe. MUST NOT mutate committed engine state. Returns
/// TRUE iff this key will be consumed by `ImeToAsciiEx`.
#[no_mangle]
pub unsafe extern "system" fn ImeProcessKey(
    h_imc: HIMC,
    v_key: u32,
    _l_key_data: LPARAM,
    lpb_key_state: *const u8,
) -> BOOL {
    let cfg = ime_state::config();
    let consume = ime_state::with_context(h_imc, |ctx| {
        input::should_consume(ctx, cfg, v_key, lpb_key_state)
    })
    .unwrap_or(false);

    if consume {
        TRUE
    } else {
        FALSE
    }
}

/// THE feed point. Calls `engine.press_key`, drains commit/preedit into the
/// COMPOSITIONSTRING, and returns the number of generated transmsgs.
#[no_mangle]
pub unsafe extern "system" fn ImeToAsciiEx(
    u_v_key: u32,
    _u_scan_code: u32,
    lpb_key_state: *const u8,
    lp_trans_buf: *mut TRANSMSGLIST,
    _fu_state: u32,
    h_imc: HIMC,
) -> u32 {
    let cfg = ime_state::config();

    ime_state::with_context(h_imc, |ctx| {
        let feed = input::feed_key(ctx, cfg, u_v_key, lpb_key_state);

        // Nothing happened at all → no messages (key falls through to the app).
        // NOTE: if the host already has a composition open (comp_open) we must
        // still let build_and_emit run so an empty-preedit/empty-commit result
        // tears it down with a balanced END — but feed_key only reaches here with
        // changes; the no-change branch leaves comp_open untouched.
        if !feed.consumed && !feed.commit_changed && !feed.preedit_changed {
            return 0u32;
        }

        // Drive START/END off the AUTHORITATIVE per-HIMC host-composition flag,
        // not engine.is_composing() (which can desync after a zero-message
        // keystroke). build_and_emit returns the updated flag.
        let (count, comp_open) = composition::build_and_emit(
            h_imc,
            lp_trans_buf,
            &feed.commit,
            &feed.preedit,
            ctx.comp_open,
        );
        ctx.comp_open = comp_open;
        count
    })
    .unwrap_or(0)
}

/// IME notifications. CPS_CANCEL resets the engine; others default to TRUE.
#[no_mangle]
pub unsafe extern "system" fn NotifyIME(
    h_imc: HIMC,
    dw_action: u32,
    dw_index: u32,
    _dw_value: u32,
) -> BOOL {
    if dw_action == NI_COMPOSITIONSTR.0 {
        match dw_index {
            // CPS_CANCEL: discard the in-flight composition.
            x if x == CPS_CANCEL.0 => {
                let was_open = ime_state::with_context(h_imc, |ctx| {
                    ctx.engine.reset();
                    let open = ctx.comp_open;
                    ctx.comp_open = false;
                    open
                })
                .unwrap_or(false);
                if was_open {
                    composition::clear_composition(h_imc);
                    // Deliver the cleared state to the app (no transbuf here).
                    let _ = globals::ImmGenerateMessage(h_imc);
                }
            }
            // CPS_COMPLETE: host asks to FINALIZE the current composition without
            // a key event. Drain the engine — clear_preedit() flushes the in-flight
            // preedit into the commit buffer (engine.rs:301) — read the committed
            // text, deliver it as GCS_RESULTSTR + WM_IME_ENDCOMPOSITION, then
            // generate the queued messages. Without this the active preedit is
            // abandoned (left in the engine/IMCC) and text is lost on focus-driven
            // finalize (reviewer minor: CPS_COMPLETE).
            x if x == CPS_COMPLETE.0 => {
                let result = ime_state::with_context(h_imc, |ctx| {
                    let was_open = ctx.comp_open;
                    ctx.engine.clear_preedit(); // flush preedit -> commit buffer
                    let committed = ctx.engine.commit_str().to_string();
                    ctx.engine.clear_commit();
                    ctx.comp_open = false;
                    (was_open, committed)
                });
                if let Some((was_open, committed)) = result {
                    if was_open || !committed.is_empty() {
                        composition::commit_composition(h_imc, &committed);
                        let _ = globals::ImmGenerateMessage(h_imc);
                    }
                }
            }
            // CPS_CONVERT / CPS_REVERT: not supported (no conversion model).
            _ => {}
        }
    }
    // NI_CONTEXTUPDATED / NI_OPENCANDIDATE / NI_CLOSECANDIDATE: accept silently.
    TRUE
}

/// SCS_SETSTR reconversion seed. Deferred — return FALSE honestly.
#[no_mangle]
pub unsafe extern "system" fn ImeSetCompositionString(
    _h_imc: HIMC,
    _dw_index: u32,
    _lp_comp: *const c_void,
    _dw_comp_len: u32,
    _lp_read: *const c_void,
    _dw_read_len: u32,
) -> BOOL {
    // TODO(unim-imm32): SCS_SETSTR reconversion (SCS_CAP_SETRECONVERTSTRING consumer).
    FALSE
}

// ===========================================================================
// UI — #[no_mangle] export lives here; behaviour owned by ui_window.rs
// ===========================================================================

/// The IME UI window procedure. One-line forwarder into the ui-owned body.
#[no_mangle]
pub unsafe extern "system" fn UIWndProc(
    h_wnd: HWND,
    u_msg: u32,
    w_param: WPARAM,
    l_param: LPARAM,
) -> isize {
    ui_window::ui_wndproc_impl(h_wnd, u_msg, w_param, l_param)
}

// ===========================================================================
// Honest stubs (return 0/FALSE so the IME still loads; NO faked behaviour)
// ===========================================================================

#[no_mangle]
pub unsafe extern "system" fn ImeRegisterWord(
    _lpsz_reading: windows::core::PCWSTR,
    _dw_style: u32,
    _lpsz_string: windows::core::PCWSTR,
) -> BOOL {
    // TODO(unim-imm32): user word registration.
    FALSE
}

#[no_mangle]
pub unsafe extern "system" fn ImeUnregisterWord(
    _lpsz_reading: windows::core::PCWSTR,
    _dw_style: u32,
    _lpsz_string: windows::core::PCWSTR,
) -> BOOL {
    // TODO(unim-imm32): user word unregistration.
    FALSE
}

#[no_mangle]
pub unsafe extern "system" fn ImeGetRegisterWordStyle(
    _n_item: u32,
    _lp_style_buf: *mut STYLEBUFW,
) -> u32 {
    // TODO(unim-imm32): register-word styles.
    0
}

#[no_mangle]
pub unsafe extern "system" fn ImeEnumRegisterWord(
    _lpfn_enum_proc: *const c_void,
    _lpsz_reading: windows::core::PCWSTR,
    _dw_style: u32,
    _lpsz_string: windows::core::PCWSTR,
    _lp_data: *mut c_void,
) -> u32 {
    // TODO(unim-imm32): enumerate registered words.
    0
}

#[no_mangle]
pub unsafe extern "system" fn ImeGetImeMenuItems(
    _h_imc: HIMC,
    _dw_flags: u32,
    _dw_type: u32,
    _lp_ime_parent_menu: *mut IMEMENUITEMINFOW,
    _lp_ime_menu: *mut IMEMENUITEMINFOW,
    _dw_size: u32,
) -> u32 {
    // TODO(unim-imm32): IME menu items.
    0
}

#[no_mangle]
pub unsafe extern "system" fn ImeConversionList(
    _h_imc: HIMC,
    _lp_src: windows::core::PCWSTR,
    _lp_dst: *mut CANDIDATELIST,
    _dw_buf_len: u32,
    _u_flag: u32,
) -> u32 {
    // TODO(unim-imm32): conversion candidate list.
    0
}

// Keep the less-common IMM32 typedefs referenced so the windows feature set is
// validated at compile time even before reconversion/word-registration land.
#[allow(dead_code)]
fn _type_anchor(_w: REGISTERWORDW) {}

// ===========================================================================
// Dev registration exports — `regsvr32 unim_imm32.ime` calls these.
//
// IMM32 IMEs are registry-registered (not COM self-registered); the MSI is the
// authoritative install path (DESIGN.md §8/§10). These exports exist ONLY so a
// `cargo`-driven dev loop can install/uninstall without building an MSI:
//   regsvr32 unim_imm32.ime      -> DllRegisterServer   -> register::register_ime()
//   regsvr32 /u unim_imm32.ime   -> DllUnregisterServer -> register::unregister_ime()
// Both write/remove the same `Keyboard Layouts\E0200412` rows the MSI installs.
// They are NOT listed for the MSI to call (NO SelfRegCost stays true).
// ===========================================================================

/// `S_OK` (0). The IME loaded fine; registration succeeded.
const S_OK: HRESULT = HRESULT(0);
/// `E_FAIL` (0x8000_4005). Generic registration failure.
const E_FAIL: HRESULT = HRESULT(0x8000_4005u32 as i32);

/// Standard COM-shaped registration entry point. `regsvr32 unim_imm32.ime`
/// invokes this; it forwards to [`register::register_ime`] (writes the KLID
/// keyboard-layout registry rows). Maps `Ok` → `S_OK`, `Err` → the error's
/// `HRESULT` (or `E_FAIL`). Never panics across the C ABI.
#[no_mangle]
pub unsafe extern "system" fn DllRegisterServer() -> HRESULT {
    match register::register_ime() {
        Ok(()) => S_OK,
        Err(e) => {
            let hr = e.code();
            if hr.is_ok() {
                E_FAIL
            } else {
                hr
            }
        }
    }
}

/// Standard COM-shaped unregistration entry point. `regsvr32 /u unim_imm32.ime`
/// invokes this; it forwards to [`register::unregister_ime`] (unloads the layout
/// and deletes the KLID key). Maps `Ok` → `S_OK`, `Err` → the error's `HRESULT`
/// (or `E_FAIL`). Never panics across the C ABI.
#[no_mangle]
pub unsafe extern "system" fn DllUnregisterServer() -> HRESULT {
    match register::unregister_ime() {
        Ok(()) => S_OK,
        Err(e) => {
            let hr = e.code();
            if hr.is_ok() {
                E_FAIL
            } else {
                hr
            }
        }
    }
}
