//! composition.rs — engine output → IMCC COMPOSITIONSTRING + transmsg (owner: core)
//!
//! The crux of the IME (DESIGN.md §5). Mirrors mozc `ime_composition_string.cc`
//! + `ime_message_queue.cc`. We keep a fixed-capacity [`UnimCompositionString`]
//! in the context's `hCompStr` IMCC and, on each keystroke, populate it then emit
//! the matching `WM_IME_*` transmsg sequence into the caller's `TRANSMSGLIST`,
//! returning the count (the standard WDK/mozc `ImeToAsciiEx` protocol).
//!
//! Single clause/segment only — the UNIM engine has no multi-clause conversion.
//! Unicode (`IME_PROP_UNICODE`) so all strings are UTF-16.

use core::mem::{offset_of, size_of};

use windows::Win32::UI::Input::Ime::{
    COMPOSITIONSTRING, INPUTCONTEXT, TRANSMSG, TRANSMSGLIST,
    ATTR_INPUT,
    GCS_COMPSTR, GCS_COMPATTR, GCS_COMPCLAUSE, GCS_CURSORPOS, GCS_DELTASTART,
    GCS_RESULTSTR, GCS_RESULTCLAUSE,
    HIMC,
};
use windows::Win32::UI::WindowsAndMessaging::{
    WM_IME_STARTCOMPOSITION, WM_IME_COMPOSITION, WM_IME_ENDCOMPOSITION,
};
use windows::Win32::Foundation::{WPARAM, LPARAM};

// IMCC accessors — provided by the scaffold's manual imm32.dll FFI (globals.rs §3.1).
use crate::globals::{
    ImmLockIMC, ImmUnlockIMC, ImmGetIMCCSize, ImmCreateIMCC, ImmReSizeIMCC,
    ImmLockIMCC, ImmUnlockIMCC,
};

/// wchars; matches IMM32 practical composition cap (mozc uses the same bound).
const MAX_COMPOSITION_LENGTH: usize = 500;

/// Fixed-capacity COMPOSITIONSTRING backing store, exactly like mozc's
/// `CompositionString`. Lives inside the `hCompStr` IMCC. `#[repr(C)]` so the
/// `dw*Offset` fields (computed via `offset_of!`) are byte-accurate on BOTH x86
/// and x64 — IMM32 reads the strings by adding these offsets to the IMCC base.
#[repr(C)]
pub struct UnimCompositionString {
    pub info: COMPOSITIONSTRING, // header with all dw*Offset/dw*Len
    pub comp_reading_attr: [u8; MAX_COMPOSITION_LENGTH],
    pub comp_attr: [u8; MAX_COMPOSITION_LENGTH], // GCS_COMPATTR: one ATTR_* byte / wchar
    pub comp_clause: [u32; 2],                   // {0, comp_len}
    pub comp_reading_clause: [u32; 2],
    pub comp_reading_str: [u16; MAX_COMPOSITION_LENGTH],
    pub comp_str: [u16; MAX_COMPOSITION_LENGTH], // GCS_COMPSTR: preedit
    pub result_reading_clause: [u32; 2],
    pub result_clause: [u32; 2], // {0, result_len}
    pub result_reading_str: [u16; MAX_COMPOSITION_LENGTH],
    pub result_str: [u16; MAX_COMPOSITION_LENGTH], // GCS_RESULTSTR: committed text
}

impl UnimCompositionString {
    /// Zero the whole struct, set `dwSize`, and wire every `dw*Offset` to the
    /// byte offset of its backing field. All `dw*Len` start at 0.
    ///
    /// # Safety
    /// `self` must point to a `size_of::<UnimCompositionString>()`-byte region
    /// (guaranteed by [`with_comp_string`] before this is called).
    pub unsafe fn initialize(&mut self) {
        // Zero everything (offsets/lens/strings).
        core::ptr::write_bytes(self as *mut Self as *mut u8, 0, size_of::<Self>());

        self.info.dwSize = size_of::<Self>() as u32;

        self.info.dwCompStrOffset = offset_of!(Self, comp_str) as u32;
        self.info.dwCompAttrOffset = offset_of!(Self, comp_attr) as u32;
        self.info.dwCompClauseOffset = offset_of!(Self, comp_clause) as u32;
        self.info.dwCompReadStrOffset = offset_of!(Self, comp_reading_str) as u32;
        self.info.dwCompReadAttrOffset = offset_of!(Self, comp_reading_attr) as u32;
        self.info.dwCompReadClauseOffset = offset_of!(Self, comp_reading_clause) as u32;

        self.info.dwResultStrOffset = offset_of!(Self, result_str) as u32;
        self.info.dwResultClauseOffset = offset_of!(Self, result_clause) as u32;
        self.info.dwResultReadStrOffset = offset_of!(Self, result_reading_str) as u32;
        self.info.dwResultReadClauseOffset = offset_of!(Self, result_reading_clause) as u32;
    }

    /// Write the preedit (composition) string + attrs + clause + cursor.
    pub fn set_preedit(&mut self, preedit_utf16: &[u16]) {
        let len = preedit_utf16.len().min(MAX_COMPOSITION_LENGTH);

        self.comp_str[..len].copy_from_slice(&preedit_utf16[..len]);
        self.info.dwCompStrLen = len as u32;

        // Whole preedit is being input (no converted/target segment for plain hangul).
        for b in self.comp_attr[..len].iter_mut() {
            *b = ATTR_INPUT as u8;
        }
        self.info.dwCompAttrLen = len as u32;

        // Single clause spanning the whole preedit: {0, len}.
        self.comp_clause = [0, len as u32];
        self.info.dwCompClauseLen = (2 * size_of::<u32>()) as u32; // 8

        self.info.dwCursorPos = len as u32; // caret at end of preedit
        self.info.dwDeltaStart = 0;
    }

    /// Write the result (committed) string + clause. (Excel-2003 quirk: result
    /// needs clause info — mozc b/2959222.)
    pub fn set_result(&mut self, result_utf16: &[u16]) {
        let len = result_utf16.len().min(MAX_COMPOSITION_LENGTH);

        self.result_str[..len].copy_from_slice(&result_utf16[..len]);
        self.info.dwResultStrLen = len as u32;

        self.result_clause = [0, len as u32];
        self.info.dwResultClauseLen = (2 * size_of::<u32>()) as u32; // 8
    }

    /// Clear the composition (preedit) portion only; leaves result intact.
    pub fn clear_preedit(&mut self) {
        for b in self.comp_str.iter_mut() {
            *b = 0;
        }
        for b in self.comp_attr.iter_mut() {
            *b = 0;
        }
        self.comp_clause = [0, 0];
        self.info.dwCompStrLen = 0;
        self.info.dwCompAttrLen = 0;
        self.info.dwCompClauseLen = 0;
        self.info.dwCursorPos = 0;
        self.info.dwDeltaStart = 0;
    }

    /// Clear the result portion (after it has been delivered via GCS_RESULTSTR).
    pub fn clear_result(&mut self) {
        for b in self.result_str.iter_mut() {
            *b = 0;
        }
        self.result_clause = [0, 0];
        self.info.dwResultStrLen = 0;
        self.info.dwResultClauseLen = 0;
    }
}

/// Lock the INPUTCONTEXT, ensure `hCompStr` is sized to [`UnimCompositionString`],
/// run `f` over `&mut UnimCompositionString`, then unlock everything.
///
/// Returns `None` if the HIMC can't be locked or the IMCC can't be created.
///
/// # Safety
/// `himc` must be a valid IMM32 input context handle (from an IMM32 callback).
unsafe fn with_comp_string<R>(himc: HIMC, f: impl FnOnce(&mut UnimCompositionString) -> R) -> Option<R> {
    let need = size_of::<UnimCompositionString>() as u32;

    let ic = ImmLockIMC(himc) as *mut INPUTCONTEXT;
    if ic.is_null() {
        return None;
    }

    // Ensure hCompStr exists and is large enough.
    let mut fresh = false;
    if (*ic).hCompStr.is_invalid() {
        (*ic).hCompStr = ImmCreateIMCC(need);
        fresh = true;
    } else if ImmGetIMCCSize((*ic).hCompStr) < need {
        (*ic).hCompStr = ImmReSizeIMCC((*ic).hCompStr, need);
        fresh = true;
    }
    if (*ic).hCompStr.is_invalid() {
        let _ = ImmUnlockIMC(himc);
        return None;
    }

    let p = ImmLockIMCC((*ic).hCompStr) as *mut UnimCompositionString;
    if p.is_null() {
        let _ = ImmUnlockIMC(himc);
        return None;
    }

    // A just-created/resized IMCC needs its offset header (re)initialised. A
    // resize keeps the old bytes but our offsets are fixed, so re-init is safe
    // and idempotent w.r.t. the offset table.
    if fresh {
        (*p).initialize();
    }

    let r = f(&mut *p);

    let _ = ImmUnlockIMCC((*ic).hCompStr);
    let _ = ImmUnlockIMC(himc);
    Some(r)
}

/// Convert a Rust `&str` to a NUL-free UTF-16 vector (no terminator — IMM32
/// counts by `dw*Len`, not NUL).
fn to_utf16(s: &str) -> Vec<u16> {
    s.encode_utf16().collect()
}

/// One TRANSMSG = `{message, wParam, lParam}`.
struct Msg {
    message: u32,
    wparam: usize,
    lparam: isize,
}

/// Write the message list into `lpTransBuf` (TRANSMSGLIST), capped at its
/// advertised `uMsgCount`, and return the number written.
///
/// # Safety
/// `transbuf` must be a valid `*mut TRANSMSGLIST` whose `TransMsg` flexible array
/// has at least `uMsgCount` entries (the IMM32 caller guarantees this).
unsafe fn write_transmsgs(transbuf: *mut TRANSMSGLIST, msgs: &[Msg]) -> u32 {
    if transbuf.is_null() {
        return 0;
    }
    let cap = (*transbuf).uMsgCount as usize;
    let base = (*transbuf).TransMsg.as_mut_ptr();
    let n = msgs.len().min(cap);
    for (i, m) in msgs.iter().take(n).enumerate() {
        let slot = base.add(i);
        (*slot) = TRANSMSG {
            message: m.message,
            wParam: WPARAM(m.wparam),
            lParam: LPARAM(m.lparam),
        };
    }
    n as u32
}

/// Composition flag set carried by an updating `WM_IME_COMPOSITION` for the
/// preedit (comp) portion.
#[inline]
fn comp_flags() -> isize {
    (GCS_COMPSTR.0
        | GCS_COMPATTR.0
        | GCS_COMPCLAUSE.0
        | GCS_CURSORPOS.0
        | GCS_DELTASTART.0) as isize
}

/// Flag set for the result (committed) portion.
#[inline]
fn result_flags() -> isize {
    (GCS_RESULTSTR.0 | GCS_RESULTCLAUSE.0) as isize
}

/// Build the COMPOSITIONSTRING into the HIMC's IMCC and emit the matching
/// transmsg sequence into `transbuf`. Returns `(count, comp_open)` where `count`
/// is the number of TRANSMSG entries written and `comp_open` is the NEW
/// authoritative host-composition state after this emit (see [`ImeContext`]).
///
/// Transition table (DESIGN.md §5.5):
///
/// | situation | messages |
/// |---|---|
/// | start (no comp, preedit now non-empty) | START, COMPOSITION(comp) |
/// | update (composing, preedit changed, no commit) | COMPOSITION(comp) |
/// | commit only, no new preedit | COMPOSITION(result, wParam=lastChar), END |
/// | commit + new preedit (rollover) | COMPOSITION(result|comp) — single msg |
/// | preedit cleared, no commit (cancel) | COMPOSITION(0,0), END |
///
/// `comp_open` (IN) = is a HOST composition currently open (did we emit START
/// without a matching END). This is the AUTHORITATIVE flag tracked per-HIMC in
/// [`crate::ime_state::ImeContext`], NOT `engine.is_composing()` — the two can
/// desync after any zero-message keystroke. Driving START/END off this flag
/// keeps the pair balanced (reviewer: composition.rs host-desync).
///
/// # Safety
/// `himc` valid IMM32 handle; `transbuf` valid `*mut TRANSMSGLIST` (or null).
pub unsafe fn build_and_emit(
    himc: HIMC,
    transbuf: *mut TRANSMSGLIST,
    result: &str,
    preedit: &str,
    comp_open: bool,
) -> (u32, bool) {
    let result_u16 = to_utf16(result);
    let preedit_u16 = to_utf16(preedit);

    let has_result = !result_u16.is_empty();
    let has_preedit = !preedit_u16.is_empty();

    // wParam for legacy apps that read the last result char from WM_IME_COMPOSITION.
    let last_result_char: usize = result_u16.last().copied().unwrap_or(0) as usize;

    // Populate the IMCC so the flags we set in the messages match the buffer.
    let populated = with_comp_string(himc, |cs| {
        if has_result {
            cs.set_result(&result_u16);
        } else {
            cs.clear_result();
        }
        if has_preedit {
            cs.set_preedit(&preedit_u16);
        } else {
            cs.clear_preedit();
        }
    });
    if populated.is_none() {
        // Could not access the comp string — emit nothing rather than lie, and
        // leave the host-composition flag unchanged (no transitions happened).
        return (0, comp_open);
    }

    let mut msgs: Vec<Msg> = Vec::with_capacity(3);
    // New host-composition state; mutated as we push START/END below.
    let mut now_open = comp_open;

    match (has_result, has_preedit) {
        // commit + new preedit (syllable rollover): ONE message carries both,
        // do NOT end+restart (same rationale as TSF commit_and_restart).
        (true, true) => {
            if !now_open {
                msgs.push(Msg { message: WM_IME_STARTCOMPOSITION, wparam: 0, lparam: 0 });
                now_open = true;
            }
            msgs.push(Msg {
                message: WM_IME_COMPOSITION,
                wparam: last_result_char,
                lparam: result_flags() | comp_flags(),
            });
            // Composition remains open (new preedit is live).
        }

        // commit only, no new preedit: deliver result then end composition.
        (true, false) => {
            if !now_open {
                // A commit with no open host composition (e.g. English char
                // passthrough via the engine). Wrap it START..RESULT..END so the
                // host inserts it.
                msgs.push(Msg { message: WM_IME_STARTCOMPOSITION, wparam: 0, lparam: 0 });
            }
            msgs.push(Msg {
                message: WM_IME_COMPOSITION,
                wparam: last_result_char,
                lparam: result_flags(),
            });
            msgs.push(Msg { message: WM_IME_ENDCOMPOSITION, wparam: 0, lparam: 0 });
            now_open = false;
        }

        // preedit present, no commit.
        (false, true) => {
            if now_open {
                // update
                msgs.push(Msg {
                    message: WM_IME_COMPOSITION,
                    wparam: 0,
                    lparam: comp_flags(),
                });
            } else {
                // start
                msgs.push(Msg { message: WM_IME_STARTCOMPOSITION, wparam: 0, lparam: 0 });
                msgs.push(Msg {
                    message: WM_IME_COMPOSITION,
                    wparam: 0,
                    lparam: comp_flags(),
                });
                now_open = true;
            }
        }

        // nothing to commit, nothing to compose.
        (false, false) => {
            if now_open {
                // preedit cleared (cancel): empty composition then end.
                msgs.push(Msg { message: WM_IME_COMPOSITION, wparam: 0, lparam: 0 });
                msgs.push(Msg { message: WM_IME_ENDCOMPOSITION, wparam: 0, lparam: 0 });
                now_open = false;
            }
            // else: no state change → no messages.
        }
    }

    let n = write_transmsgs(transbuf, &msgs);
    (n, now_open)
}

/// Append `msgs` into the INPUTCONTEXT's message buffer (`hMsgBuf`) and bump
/// `dwNumMsgBuf`. Used OUTSIDE `ImeToAsciiEx` (e.g. `NotifyIME`) where there is
/// no `TRANSMSGLIST` — the caller then dispatches them via `ImmGenerateMessage`.
///
/// `ImmGenerateMessage` only dispatches messages that already sit in `hMsgBuf`,
/// so clearing the comp string alone is not enough: we must enqueue the
/// `WM_IME_*` records here first (DESIGN.md §5.5).
///
/// # Safety
/// `himc` must be a valid IMM32 input context handle.
unsafe fn enqueue_messages(himc: HIMC, msgs: &[Msg]) -> bool {
    if msgs.is_empty() {
        return true;
    }

    let ic = ImmLockIMC(himc) as *mut INPUTCONTEXT;
    if ic.is_null() {
        return false;
    }

    let old_count = (*ic).dwNumMsgBuf as usize;
    let new_count = old_count + msgs.len();
    let need = (new_count * size_of::<TRANSMSG>()) as u32;

    // Ensure hMsgBuf exists and is large enough to hold old + new records.
    if (*ic).hMsgBuf.is_invalid() {
        (*ic).hMsgBuf = ImmCreateIMCC(need);
    } else if ImmGetIMCCSize((*ic).hMsgBuf) < need {
        (*ic).hMsgBuf = ImmReSizeIMCC((*ic).hMsgBuf, need);
    }
    if (*ic).hMsgBuf.is_invalid() {
        let _ = ImmUnlockIMC(himc);
        return false;
    }

    let base = ImmLockIMCC((*ic).hMsgBuf) as *mut TRANSMSG;
    if base.is_null() {
        let _ = ImmUnlockIMC(himc);
        return false;
    }

    for (i, m) in msgs.iter().enumerate() {
        *base.add(old_count + i) = TRANSMSG {
            message: m.message,
            wParam: WPARAM(m.wparam),
            lParam: LPARAM(m.lparam),
        };
    }
    (*ic).dwNumMsgBuf = new_count as u32;

    let _ = ImmUnlockIMCC((*ic).hMsgBuf);
    let _ = ImmUnlockIMC(himc);
    true
}

/// Finalize the current composition by committing `result` to the host: write
/// `result` into the IMCC result slot, clear the preedit, and enqueue
/// `WM_IME_COMPOSITION(GCS_RESULTSTR|GCS_RESULTCLAUSE)` + `WM_IME_ENDCOMPOSITION`
/// into the context message buffer. Used by `NotifyIME(CPS_COMPLETE)` where there
/// is no `TRANSMSGLIST`; the caller drives delivery via `ImmGenerateMessage`
/// AFTER this returns (DESIGN.md §5.5 commit-only row).
///
/// If `result` is empty there is nothing to commit; we still enqueue the
/// cancel/end pair so an open host composition is torn down cleanly.
///
/// # Safety
/// `himc` valid IMM32 handle.
pub unsafe fn commit_composition(himc: HIMC, result: &str) {
    let result_u16 = to_utf16(result);
    let has_result = !result_u16.is_empty();
    let last_result_char: usize = result_u16.last().copied().unwrap_or(0) as usize;

    let _ = with_comp_string(himc, |cs| {
        if has_result {
            cs.set_result(&result_u16);
        } else {
            cs.clear_result();
        }
        cs.clear_preedit();
    });

    let mut msgs: Vec<Msg> = Vec::with_capacity(2);
    if has_result {
        msgs.push(Msg {
            message: WM_IME_COMPOSITION,
            wparam: last_result_char,
            lparam: result_flags(),
        });
    } else {
        msgs.push(Msg { message: WM_IME_COMPOSITION, wparam: 0, lparam: 0 });
    }
    msgs.push(Msg { message: WM_IME_ENDCOMPOSITION, wparam: 0, lparam: 0 });

    let _ = enqueue_messages(himc, &msgs);
}

/// Force-terminate any active composition: empty the comp string and enqueue
/// `WM_IME_COMPOSITION(0,0)` + `WM_IME_ENDCOMPOSITION(0,0)` into the context
/// message buffer (DESIGN.md §5.5 cancel row). Used by `NotifyIME` (CPS_CANCEL)
/// where there is no transbuf — the caller drives delivery via
/// `ImmGenerateMessage` AFTER this returns.
///
/// # Safety
/// `himc` valid IMM32 handle.
pub unsafe fn clear_composition(himc: HIMC) {
    let _ = with_comp_string(himc, |cs| {
        cs.clear_preedit();
        cs.clear_result();
    });

    // Enqueue the cancel pair so ImmGenerateMessage actually tears down the
    // host's composition (otherwise it would dispatch nothing and the preedit
    // stays visually stuck in the target app).
    let msgs = [
        Msg { message: WM_IME_COMPOSITION, wparam: 0, lparam: 0 },
        Msg { message: WM_IME_ENDCOMPOSITION, wparam: 0, lparam: 0 },
    ];
    let _ = enqueue_messages(himc, &msgs);
}
