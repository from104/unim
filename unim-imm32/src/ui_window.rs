/// ui_window.rs — UI window class registration + UIWndProc body (owner: ui)
///
/// The #[no_mangle] extern "system" fn UIWndProc symbol lives in lib.rs and is a
/// one-line forwarder to `ui_wndproc_impl` here. This keeps the export table in
/// one file (lib.rs owns all exports) while ui owns the behaviour.
///
/// Candidate / hanja popup window is a stub (TODO(unim-imm32)).

use windows::Win32::Foundation::{HINSTANCE, HWND, LPARAM, WPARAM};
use windows::Win32::UI::WindowsAndMessaging::{
    DefWindowProcW, RegisterClassExW, WNDCLASSEXW, WNDPROC, CS_IME,
    WM_IME_NOTIFY, WM_IME_STARTCOMPOSITION, WM_IME_ENDCOMPOSITION, WM_IME_COMPOSITION,
};
use windows::Win32::UI::Input::Ime::{
    IMN_OPENCANDIDATE, IMN_CLOSECANDIDATE, IMN_CHANGECANDIDATE,
    IMN_SETCOMPOSITIONWINDOW, IMN_SETCANDIDATEPOS, IMN_GUIDELINE,
    IMN_SETCOMPOSITIONFONT, IMN_SETCONVERSIONMODE, IMN_SETSENTENCEMODE,
    IMN_SETOPENSTATUS,
};
use windows::core::{PCWSTR, Result};
use core::sync::atomic::{AtomicBool, Ordering};

use crate::globals::UNIM_UI_CLASS_NAME;

// ---------------------------------------------------------------------------
// Class registration (idempotent)
// ---------------------------------------------------------------------------

/// Track whether the class has been registered in this process already.
static CLASS_REGISTERED: AtomicBool = AtomicBool::new(false);

/// Register the IME UI window class named `UNIM_UI_CLASS_NAME`.
///
/// Must be called before any UI window is created — typically from `ImeInquire`
/// or the first `ImeSelect`. Safe to call multiple times; subsequent calls are
/// no-ops once the class exists.
///
/// Style: `CS_IME`.  WndProc: the exported `UIWndProc` symbol in lib.rs.
pub fn ensure_ui_class_registered(hinst: HINSTANCE) -> Result<()> {
    // Fast path: already registered in this process.
    if CLASS_REGISTERED.load(Ordering::Acquire) {
        return Ok(());
    }

    // Build a NUL-terminated UTF-16 class name from the compile-time &str.
    let class_name_wide: Vec<u16> = UNIM_UI_CLASS_NAME
        .encode_utf16()
        .chain(core::iter::once(0u16))
        .collect();

    // SAFETY: UIWndProc is the #[no_mangle] extern "system" symbol exported by lib.rs.
    // It forwards directly to ui_wndproc_impl below. Declared with LRESULT return to
    // match the WNDPROC type alias (LRESULT is repr(transparent) over isize, so the
    // ABI is identical to lib.rs's `-> isize` definition). The fn item is coerced to a
    // WNDPROC fn pointer via an explicit binding.
    extern "system" {
        fn UIWndProc(
            hwnd: HWND,
            msg: u32,
            wparam: WPARAM,
            lparam: LPARAM,
        ) -> windows::Win32::Foundation::LRESULT;
    }
    let wndproc: WNDPROC = Some(UIWndProc);

    let wc = WNDCLASSEXW {
        cbSize: core::mem::size_of::<WNDCLASSEXW>() as u32,
        style: CS_IME,
        lpfnWndProc: wndproc,
        cbClsExtra: 0,
        cbWndExtra: 0,
        hInstance: hinst,
        hIcon: windows::Win32::UI::WindowsAndMessaging::HICON::default(),
        hCursor: windows::Win32::UI::WindowsAndMessaging::HCURSOR::default(),
        hbrBackground: windows::Win32::Graphics::Gdi::HBRUSH::default(),
        lpszMenuName: PCWSTR::null(),
        lpszClassName: PCWSTR(class_name_wide.as_ptr()),
        hIconSm: windows::Win32::UI::WindowsAndMessaging::HICON::default(),
    };

    // SAFETY: all fields are valid; class_name_wide is alive for the duration of the call.
    let atom = unsafe { RegisterClassExW(&wc) };
    if atom == 0 {
        // Class may already be registered by a previous DLL load in this process.
        // Check the last error; ERROR_CLASS_ALREADY_EXISTS (1410) is not fatal.
        let err = unsafe { windows::Win32::Foundation::GetLastError() };
        const ERROR_CLASS_ALREADY_EXISTS: u32 = 1410;
        if err.0 != ERROR_CLASS_ALREADY_EXISTS {
            return Err(windows::core::Error::from(err));
        }
    }

    CLASS_REGISTERED.store(true, Ordering::Release);
    Ok(())
}

// ---------------------------------------------------------------------------
// WndProc body
// ---------------------------------------------------------------------------

/// Body of the IME UI window procedure.
///
/// lib.rs re-exports this as the `#[no_mangle] extern "system" fn UIWndProc`.
///
/// IMM32 sends `WM_IME_NOTIFY` with `wParam` = an `IMN_*` notification code.
/// We handle the subset needed to suppress candidate / composition UI cleanly;
/// everything else falls through to `DefWindowProcW`.
///
/// Candidate / hanja popup is a stub — the hook points are present but the
/// actual child window is not created yet (TODO(unim-imm32)).
pub fn ui_wndproc_impl(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> isize {
    match msg {
        // ------------------------------------------------------------------
        // WM_IME_NOTIFY — dispatched per IMN_* sub-code in wParam
        // ------------------------------------------------------------------
        WM_IME_NOTIFY => {
            let imn = wparam.0 as u32;
            match imn {
                // Candidate list opened: show candidate window (stub).
                IMN_OPENCANDIDATE => {
                    // TODO(unim-imm32): create / show candidate popup child window.
                    0
                }

                // Candidate list closed: hide candidate window (stub).
                IMN_CLOSECANDIDATE => {
                    // TODO(unim-imm32): hide / destroy candidate popup child window.
                    0
                }

                // Candidate list contents changed: repaint (stub).
                IMN_CHANGECANDIDATE => {
                    // TODO(unim-imm32): refresh candidate popup contents.
                    0
                }

                // Composition window position changed.
                IMN_SETCOMPOSITIONWINDOW => {
                    // TODO(unim-imm32): reposition any overlay composition window.
                    0
                }

                // Candidate window position hint changed.
                IMN_SETCANDIDATEPOS => {
                    // TODO(unim-imm32): reposition candidate popup to follow caret.
                    0
                }

                // IME guideline / error notification — no-op for now.
                IMN_GUIDELINE => 0,

                // Composition font changed.
                IMN_SETCOMPOSITIONFONT => {
                    // TODO(unim-imm32): update font used by any overlay window.
                    0
                }

                // Conversion / sentence / open-status mode changed — no visible UI change needed.
                IMN_SETCONVERSIONMODE | IMN_SETSENTENCEMODE | IMN_SETOPENSTATUS => 0,

                // All other IMN_* values: pass through.
                _ => unsafe { DefWindowProcW(hwnd, msg, wparam, lparam).0 },
            }
        }

        // ------------------------------------------------------------------
        // Composition lifecycle — at the UI-window level these are no-ops:
        // apps draw the composition string at caret (IME_PROP_AT_CARET).
        // ------------------------------------------------------------------
        WM_IME_STARTCOMPOSITION => 0,
        WM_IME_ENDCOMPOSITION => 0,
        WM_IME_COMPOSITION => 0,

        // ------------------------------------------------------------------
        // All other messages — pass to the default window procedure.
        // ------------------------------------------------------------------
        _ => unsafe { DefWindowProcW(hwnd, msg, wparam, lparam).0 },
    }
}
