# unim-imm32 — Authoritative Interface Spec (DESIGN.md)

> Status: CONTRACT. Downstream implementers follow this verbatim. No signature is
> "TBD". When in doubt, the signatures and offsets here win.
>
> Crate: `unim-imm32` — a Windows **IMM32 IME** (`.ime` DLL) that reuses the existing
> UNIM Korean engine (`unim` crate) so legacy/IMM32-only apps (KakaoTalk, 아래아한글)
> that opt out of TSF still get Korean input.

---

## 0. Verified ground facts (do not relitigate)

1. **Engine reuse** — depend on workspace `unim` via PATH dep `unim = { path = ".." }`.
   - `InputEngine::new(config: &Config) -> InputEngine` (engine.rs:106)
   - `engine.press_key(keycode: KeyCode, modifier: ModifierState, config: &Config) -> InputResult` (press_key.rs:50; signature takes `modifier` BY VALUE, and `&Config`)
   - `InputResult { consumed, preedit_changed, commit_changed, hanja_candidates_available, special_char_candidates_available }` (types.rs:144) — all `pub bool`.
   - `engine.commit_str() -> &str` (286), `engine.preedit_str() -> &str` (291), `engine.clear_commit()` (296), `engine.clear_preedit()` (301), `engine.reset()` (312), `engine.is_composing() -> bool` (397), `engine.input_category() -> InputCategory` (261).
   - `KeyCode::from_win32_vk(vk: u16) -> KeyCode` (conversion.rs:366); `KeyCode::is_modifier()` (mod.rs:146), `KeyCode::is_character_key()` (mod.rs:141).
   - `ModifierState { shift, control, alt, super_key, caps_lock, num_lock }` all `pub bool` (modifiers.rs:8); `ModifierState::from_win32_modifiers(modifiers: u32) -> ModifierState` (modifiers.rs:57).
   - `Config::load_from_default_path() -> Config` (config.rs:834).
   - Layout (두벌식/세벌식) and 자동 한영 전환 live INSIDE the engine via `Config`. IMM32 feeds every key through `press_key`; ZERO layout code here.

2. **windows-rs 0.62 reality (VERIFIED against tag 0.62.0 metadata):**
   - Feature `Win32_UI_Input_Ime` EXISTS and gates the IME **structs/handles/consts**:
     `IMEINFO, COMPOSITIONSTRING, TRANSMSGLIST, TRANSMSG, CANDIDATELIST, INPUTCONTEXT, REGISTERWORDW, IMEMENUITEMINFOW, HIMC, HIMCC`, and consts
     `IME_PROP_UNICODE, IME_PROP_AT_CARET, UI_CAP_2700, SCS_CAP_COMPSTR, SCS_CAP_SETRECONVERTSTRING, GCS_COMPSTR, GCS_RESULTSTR, ATTR_INPUT, ATTR_TARGET_CONVERTED, CPS_COMPLETE, NI_COMPOSITIONSTR`.
   - **CRITICAL:** windows 0.62 binds **ZERO `Imm*` functions** in that module (the IME-side `imm32.dll` exports are not in the metadata: there is no `ImmLockIMC`, `ImmCreateIMCC`, `ImmGenerateMessage`, `ImmInstallIMEW`, etc.). We MUST declare them ourselves as manual `#[link(name = "imm32")] extern "system"` FFI in `globals.rs` (see §3.1). This is the single most load-bearing scaffold decision.
   - `HKL` lives in `Win32_UI_Input_KeyboardAndMouse` (NOT WindowsAndMessaging, NOT Foundation).
   - `WM_IME_STARTCOMPOSITION / WM_IME_COMPOSITION / WM_IME_ENDCOMPOSITION / WM_IME_NOTIFY` are in `Win32_UI_WindowsAndMessaging`.

3. **Cargo cannot emit `.ime`** — cdylib emits `unim_imm32.dll`; build scripts copy/rename to `unim_imm32.ime`. The `.ime` resolves relative to System32, so MSI installs it to System32 (x64) / SysWOW64 (x86).

4. **Workspace** sets `panic = "abort"` for release+dev (Cargo.toml:64-68). REQUIRED for C-ABI cdylib. DO NOT re-add or override.

5. **Reference flow** — `unim-tsf/src/key_handler.rs::get_modifier_state()` (lines 20-38) is pure Win32 (`GetKeyState`) and is LIFTED VERBATIM into `input.rs`. `test_key_down` (41-131) is the should-consume probe model.

6. **IMM32 contract references** — mozc tag `2.28.4880.102` `src/win32/ime/*` (last revision before mozc deleted its IMM32 IME) + MS docs. The `.def` export set, `ImeInquire` IMEINFO fields, `ImeToAsciiEx` TRANSMSGLIST return-count protocol, and `CompositionString` offset layout below are taken directly from those sources (see research_citations).

---

## 1. Module map

Every file under `unim-imm32/`, with OWNER ∈ {scaffold, core, register, ui, packaging}.
**Partition invariant:** no two implement-phase owners edit the same file.

| File | Owner | Purpose |
|---|---|---|
| `unim-imm32/Cargo.toml` | scaffold | crate manifest: cdylib, `unim` path dep, windows/windows-core 0.62 verified feature list, `version.workspace = true`. |
| `unim-imm32/build.rs` | scaffold | emit `cargo:rustc-link-arg=/DEF:unim_imm32.def` (undecorated stdcall exports, esp. x86). |
| `unim-imm32/unim_imm32.def` | scaffold | module-definition file listing all 17 exports + `DllMain` undecorated. |
| `unim-imm32/src/globals.rs` | scaffold | KLID, UI class name, IMEINFO flag constants, GUIDs, **and the manual `imm32.dll` `Imm*` FFI declarations** (§3.1). |
| `unim-imm32/src/lib.rs` | core | `DllMain` + all 17 `#[no_mangle] extern "system"` exports; thin dispatchers into the other core modules. |
| `unim-imm32/src/ime_state.rs` | core | per-HIMC engine state (`ImeContext`), global registry, `Config` storage, thread-safety (§4). |
| `unim-imm32/src/composition.rs` | core | `COMPOSITIONSTRING` IMCC build + offsets + transmsg generation (§5). |
| `unim-imm32/src/input.rs` | core | `get_modifier_state` (lifted), should-consume probe, key→engine feed; bridges `ImeProcessKey`/`ImeToAsciiEx`. |
| `unim-imm32/src/register.rs` | register | dev `register_ime()` / `unregister_ime()` via `ImmInstallIMEW` + registry; `dbg_log`. |
| `unim-imm32/src/ui_window.rs` | ui | `UIWndProc`, UI window class registration helper, IMN_* handling (candidate window = stub). |
| `scripts/build-msi.bat` | packaging | add `cargo build -p unim-imm32`; copy `unim_imm32.dll` → `unim_imm32.ime`. (NOT under unim-imm32/) |
| `scripts/cargo-msvc.bat` | packaging | no change required (forwards args); listed for completeness. |
| `.github/workflows/windows-msi.yml` | packaging | add `-p unim-imm32` build + artifact verify of `unim_imm32.ime`. |
| `installer/wix/unim.wxs` | packaging | KLID `E0200412` keyboard-layout registry rows + `.ime` File component into System32. |
| **root `Cargo.toml`** | scaffold | add `"unim-imm32"` to `[workspace].members`. |

`core` owns `lib.rs + composition.rs + ime_state.rs + input.rs`. `scaffold` owns `Cargo.toml + build.rs + unim_imm32.def + globals.rs` (+ root member). `register` owns `register.rs`. `ui` owns `ui_window.rs`. `packaging` owns the four files NOT under `unim-imm32/`.

---

## 2. Exports (the contract — `def_exports`)

All exports are `#[no_mangle] pub unsafe extern "system"` and listed in `unim_imm32.def`
(undecorated). Types use windows-0.62: `HIMC, HIMCC, HWND, HKL, HINSTANCE, WPARAM,
LPARAM, BOOL, IMEINFO, TRANSMSGLIST, CANDIDATELIST, REGISTERWORDW, IMEMENUITEMINFOW,
STYLEBUFW, RECONVERTSTRING` and raw pointers/`windows::core::PCWSTR/PWSTR`.

The classic IMM32 typedefs use `LPCTSTR/LPTSTR`; since `IME_PROP_UNICODE` is set, the
**Unicode (W)** variants are in force. Use `PCWSTR`/`PWSTR`/`*mut u16` accordingly.

### Full (real behavior)
```rust
// --- lifecycle / inquiry ---
#[no_mangle] pub unsafe extern "system"
fn ImeInquire(lpIMEInfo: *mut IMEINFO, lpszWndClass: PWSTR, dwSystemInfoFlags: u32) -> BOOL;
// fills *lpIMEInfo (§3.3) and writes UI class name (§3.2) into lpszWndClass (caller buffer).

#[no_mangle] pub unsafe extern "system"
fn ImeDestroy(uReserved: u32) -> BOOL;        // process unload cleanup; return TRUE when uReserved==0.

#[no_mangle] pub unsafe extern "system"
fn ImeEscape(hIMC: HIMC, uSubFunc: u32, lpData: *mut core::ffi::c_void) -> isize; // LRESULT
// handle IME_ESC_IME_NAME -> write UNIM_IME_NAME wide into lpData (>=64 wchars), return TRUE(1);
// default return 0 (FALSE).

#[no_mangle] pub unsafe extern "system"
fn ImeConfigure(hKL: HKL, hWnd: HWND, dwMode: u32, lpData: *mut core::ffi::c_void) -> BOOL;
// open settings (defer: launch unim settings app or return FALSE honestly). Return BOOL.

// --- context binding ---
#[no_mangle] pub unsafe extern "system"
fn ImeSelect(hIMC: HIMC, fSelect: BOOL) -> BOOL;
// fSelect==TRUE: create/bind ImeContext for hIMC (ime_state::on_select), init private data,
//   init COMPOSITIONSTRING IMCC. fSelect==FALSE: engine.reset() + drop binding. Return TRUE.

#[no_mangle] pub unsafe extern "system"
fn ImeSetActiveContext(hIMC: HIMC, fActivate: BOOL) -> BOOL;
// focus change. fActivate==FALSE -> ime_state::on_deactivate (flush/hide UI). Return TRUE.

// --- key path (the crux) ---
#[no_mangle] pub unsafe extern "system"
fn ImeProcessKey(hIMC: HIMC, vKey: u32, lKeyData: LPARAM, lpbKeyState: *const u8) -> BOOL;
// PURE should-consume PROBE. MUST NOT mutate committed engine state. Returns TRUE iff this
// key will be consumed by ImeToAsciiEx. Mirrors input::should_consume (§5.1).

#[no_mangle] pub unsafe extern "system"
fn ImeToAsciiEx(uVKey: u32, uScanCode: u32, lpbKeyState: *const u8,
                lpTransBuf: *mut TRANSMSGLIST, fuState: u32, hIMC: HIMC) -> u32;
// THE feed point. Calls engine.press_key, drains commit/preedit into COMPOSITIONSTRING,
// returns the NUMBER OF GENERATED MESSAGES written to lpTransBuf (§5). Return 0 = no msgs.

#[no_mangle] pub unsafe extern "system"
fn NotifyIME(hIMC: HIMC, dwAction: u32, dwIndex: u32, dwValue: u32) -> BOOL;
// handle NI_COMPOSITIONSTR (CPS_COMPLETE/CPS_CANCEL/CPS_CONVERT), NI_CONTEXTUPDATED,
// NI_OPENCANDIDATE/NI_CLOSECANDIDATE minimally; default TRUE. CPS_CANCEL -> engine.reset().

#[no_mangle] pub unsafe extern "system"
fn ImeSetCompositionString(hIMC: HIMC, dwIndex: u32, lpComp: *const core::ffi::c_void,
                           dwCompLen: u32, lpRead: *const core::ffi::c_void, dwReadLen: u32) -> BOOL;
// SCS_SETSTR (reconversion seed) — wire later; for now return FALSE honestly. // TODO(unim-imm32)

// --- UI ---
#[no_mangle] pub unsafe extern "system"
fn UIWndProc(hWnd: HWND, uMsg: u32, wParam: WPARAM, lParam: LPARAM) -> isize; // LRESULT
// owned by ui_window.rs (re-exported through lib.rs). Handles WM_IME_NOTIFY/IMN_*; DefWindowProcW fallback.
```

### Honest stubs (return 0/FALSE/empty so the IME still loads; NO faked behavior)
```rust
#[no_mangle] pub unsafe extern "system"
fn ImeRegisterWord(lpszReading: PCWSTR, dwStyle: u32, lpszString: PCWSTR) -> BOOL;     // -> FALSE
#[no_mangle] pub unsafe extern "system"
fn ImeUnregisterWord(lpszReading: PCWSTR, dwStyle: u32, lpszString: PCWSTR) -> BOOL;   // -> FALSE
#[no_mangle] pub unsafe extern "system"
fn ImeGetRegisterWordStyle(nItem: u32, lpStyleBuf: *mut STYLEBUFW) -> u32;             // -> 0
#[no_mangle] pub unsafe extern "system"
fn ImeEnumRegisterWord(lpfnEnumProc: *const core::ffi::c_void, lpszReading: PCWSTR,
                       dwStyle: u32, lpszString: PCWSTR, lpData: *mut core::ffi::c_void) -> u32; // -> 0
#[no_mangle] pub unsafe extern "system"
fn ImeGetImeMenuItems(hIMC: HIMC, dwFlags: u32, dwType: u32,
                      lpImeParentMenu: *mut IMEMENUITEMINFOW,
                      lpImeMenu: *mut IMEMENUITEMINFOW, dwSize: u32) -> u32;            // -> 0
#[no_mangle] pub unsafe extern "system"
fn ImeConversionList(hIMC: HIMC, lpSrc: PCWSTR, lpDst: *mut CANDIDATELIST,
                     dwBufLen: u32, uFlag: u32) -> u32;                                // -> 0
```

### `DllMain`
```rust
#[no_mangle] pub unsafe extern "system"
fn DllMain(hinst: HINSTANCE, reason: u32, _reserved: *mut core::ffi::c_void) -> BOOL;
// DLL_PROCESS_ATTACH: store hinst into globals (OnceLock), DisableThreadLibraryCalls. Return TRUE.
```

`def_exports` (the literal `.def` `EXPORTS` list, undecorated):
`ImeInquire, ImeConfigure, ImeDestroy, ImeEscape, ImeSelect, ImeSetActiveContext,
ImeProcessKey, ImeToAsciiEx, NotifyIME, ImeSetCompositionString, UIWndProc,
ImeRegisterWord, ImeUnregisterWord, ImeGetRegisterWordStyle, ImeEnumRegisterWord,
ImeGetImeMenuItems, ImeConversionList`. (`DllMain` is exported by the linker
automatically; do NOT list it in the `.def` EXPORTS — list only the 17 IME entrypoints.)

---

## 3. `globals.rs` — full contents (owner: scaffold)

### 3.1 Manual `imm32.dll` FFI (REQUIRED — windows 0.62 omits these)
```rust
use windows::Win32::UI::Input::Ime::{HIMC, HIMCC};
use windows::Win32::Foundation::{BOOL, HWND, LPARAM, WPARAM};
use windows::core::PCWSTR;

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
```
> Use `*mut INPUTCONTEXT` (cast the `c_void`) and `*mut COMPOSITIONSTRING` after locking.
> `INPUTCONTEXT` and `COMPOSITIONSTRING` types come from `windows::Win32::UI::Input::Ime`.

### 3.2 Identity constants
```rust
/// KLID for the static keyboard-layout registration. E0xx0412, 0412 = ko-KR.
/// MUST stay identical to installer/wix/unim.wxs.
pub const UNIM_IMM32_KLID: &str = "E0200412";
pub const UNIM_IMM32_LANGID: u16 = 0x0412;           // ko-KR
pub const UNIM_IMM32_IME_FILE: &str = "unim_imm32.ime";
pub const UNIM_IME_NAME: &str = "UNIM Korean IME (IMM32)";
pub const UNIM_LAYOUT_TEXT: &str = "UNIM Korean (IMM32)";
/// UI window class registered by ui_window.rs and reported from ImeInquire.
pub const UNIM_UI_CLASS_NAME: &str = "UnimImm32UiClass";
/// Per-HIMC private-data size advertised in IMEINFO.dwPrivateDataSize (§4).
pub const UNIM_PRIVATE_DATA_SIZE: u32 = 4; // we key by HIMC in a global map; keep minimal but non-zero.
```
> GUIDs: an IMM32 keyboard-layout IME does NOT use COM CLSIDs. No new GUIDs are
> required. (TSF GUIDs in `unim-tsf/src/globals.rs` are unrelated; do not reuse.)

### 3.3 IMEINFO flag values (taken from mozc 2.28.4880 ImeInquire, adapted)
```rust
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
pub const UNIM_FDW_CONVERSION_CAPS: u32 = IME_CMODE_NATIVE; // (+ IME_CMODE_FULLSHAPE later)
// fdwSentenceCaps:
pub const UNIM_FDW_SENTENCE_CAPS: u32 = 0; // IME_SMODE_NONE
// fdwUICaps:
pub const UNIM_FDW_UI_CAPS: u32 = UI_CAP_2700; // 0x2700-capable UI (no ROT90/SOFTKBD)
// fdwSCSCaps: we build the comp string ourselves; advertise comp + reconversion seed.
pub const UNIM_FDW_SCS_CAPS: u32 = SCS_CAP_COMPSTR | SCS_CAP_SETRECONVERTSTRING;
// fdwSelectCaps:
pub const UNIM_FDW_SELECT_CAPS: u32 = 0; // SELECT_CAP_CONVERSION not advertised yet
```
`ImeInquire` sets, in order: `ZeroMemory(lpIMEInfo)`,
`(*p).dwPrivateDataSize = UNIM_PRIVATE_DATA_SIZE`,
`(*p).fdwProperty = UNIM_FDW_PROPERTY`,
`(*p).fdwConversionCaps = UNIM_FDW_CONVERSION_CAPS`,
`(*p).fdwSentenceCaps = UNIM_FDW_SENTENCE_CAPS`,
`(*p).fdwUICaps = UNIM_FDW_UI_CAPS`,
`(*p).fdwSCSCaps = UNIM_FDW_SCS_CAPS`,
`(*p).fdwSelectCaps = UNIM_FDW_SELECT_CAPS`,
then copy `UNIM_UI_CLASS_NAME` (UTF-16, NUL-terminated) into `lpszWndClass`,
then return `TRUE`.

---

## 4. state_model — per-HIMC engine state (owner: core, `ime_state.rs`)

The engine is NOT `Sync`, and IMM32 callbacks for a given HIMC arrive on the owning
UI thread, but different HIMCs (windows/threads) can interleave. **Model: a
process-global map keyed by HIMC raw value, each entry behind its own `Mutex`.**

```rust
use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};
use windows::Win32::UI::Input::Ime::HIMC;
use unim::config::Config;
use unim::input_engine::InputEngine;

/// One IMM32 input context's engine state.
pub struct ImeContext {
    pub engine: InputEngine,
    // future: candidate/UI bookkeeping. Keep engine the single source of truth.
}

/// Process-global registry. Key = HIMC.0 as usize (HIMC is a typed handle).
pub struct ImeRegistry {
    contexts: HashMap<usize, ImeContext>,
}

static REGISTRY: OnceLock<Mutex<ImeRegistry>> = OnceLock::new();
/// Config is immutable after load; share one copy. Reload via ImeSetActiveContext later.
static CONFIG: OnceLock<Config> = OnceLock::new();
/// DllMain stores the module handle here.
static HINST: OnceLock<isize> = OnceLock::new();

// --- public API used by lib.rs ---
pub fn config() -> &'static Config;                       // lazy: Config::load_from_default_path()
pub fn set_hinst(h: isize);
pub fn hinst() -> isize;

/// ImeSelect(TRUE): create+bind a fresh ImeContext for this HIMC (idempotent).
pub fn on_select(himc: HIMC);
/// ImeSelect(FALSE) / ImeDestroy: engine.reset() then drop the binding.
pub fn on_unselect(himc: HIMC);
/// ImeSetActiveContext(FALSE): flush/hide; keep binding. (TRUE: no-op or rebind.)
pub fn on_deactivate(himc: HIMC);

/// Run a closure with exclusive access to this HIMC's engine.
/// Returns None if HIMC isn't bound (caller returns FALSE/0). Auto-creates on access
/// is NOT done here — binding only happens in on_select.
pub fn with_context<R>(himc: HIMC, f: impl FnOnce(&mut ImeContext) -> R) -> Option<R>;
```

Thread-safety contract:
- `with_context` locks the registry `Mutex` only long enough to look up the entry,
  then holds it for the closure (engines are short-lived per call; no re-entrancy
  into `with_context` inside `f`). Keep `f` non-blocking.
- `InputEngine` never crosses threads while borrowed; the `Mutex<ImeRegistry>`
  makes the whole registry `Send + Sync`. `unsafe impl` is NOT needed.
- `dwPrivateDataSize = UNIM_PRIVATE_DATA_SIZE` (non-zero) so IMM32 allocates the
  private block per context; we don't store the engine there (no stable pointer
  across re-locks) — the HIMC-keyed map is the authority.

---

## 5. composition_strategy — engine output → IMCC COMPOSITIONSTRING + transmsg (owner: core, `composition.rs`)

This is the crux. It mirrors mozc `ime_composition_string.cc` + `ime_message_queue.cc`.

### 5.1 `ImeProcessKey` probe (in `input.rs`, called from lib.rs)
```rust
pub fn get_modifier_state(key_state: *const u8) -> ModifierState; // lifted from key_handler.rs:20
pub fn should_consume(ctx: &ImeContext, cfg: &Config, vkey: u32, key_state: *const u8) -> bool;
```
`should_consume` replicates `key_handler::test_key_down` against the engine WITHOUT
mutating it: modifier-only → false; 한/영 (VK_HANGUL/right-Alt) → true; Hanja/F9 →
true; Korean mode + character key → true; composing + Back/Space/Enter/Tab/Esc/arrows
→ true; else false. `ImeProcessKey` returns this as BOOL. **No `press_key` here.**

### 5.2 `ImeToAsciiEx` — the feed + emit
Steps (in `lib.rs` calling `input.rs` + `composition.rs`):
1. `ime_state::with_context(hIMC, |ctx| { ... }).unwrap_or(0)`.
2. `let kc = KeyCode::from_win32_vk(uVKey as u16); let m = get_modifier_state(lpbKeyState);`
3. `let r = ctx.engine.press_key(kc, m, cfg);`
4. Drain: if `r.commit_changed` → `result = ctx.engine.commit_str().to_string(); ctx.engine.clear_commit();` else empty. `preedit = ctx.engine.preedit_str().to_string();`
5. `composition::build_and_emit(hIMC, lpTransBuf, &result, &preedit, prev_was_composing)` → returns message count.
6. `ImeToAsciiEx` returns that count.

### 5.3 COMPOSITIONSTRING IMCC buffer layout
We use a fixed-capacity struct stored in the context's `hCompStr` IMCC, exactly like
mozc's `CompositionString`. Single clause/segment (engine has no multi-clause).

```rust
const MAX_COMPOSITION_LENGTH: usize = 500; // wchars; matches IMM32 practical cap

#[repr(C)]
pub struct UnimCompositionString {
    pub info: COMPOSITIONSTRING,                 // header with all dw*Offset/dw*Len
    pub comp_reading_attr: [u8; MAX_COMPOSITION_LENGTH],
    pub comp_attr:        [u8; MAX_COMPOSITION_LENGTH],   // GCS_COMPATTR: one ATTR_* byte per wchar
    pub comp_clause:      [u32; 2],                       // {0, comp_len}
    pub comp_reading_clause:[u32; 2],
    pub comp_reading_str: [u16; MAX_COMPOSITION_LENGTH],
    pub comp_str:         [u16; MAX_COMPOSITION_LENGTH],  // GCS_COMPSTR: preedit (no NUL counted)
    pub result_reading_clause:[u32; 2],
    pub result_clause:    [u32; 2],                       // {0, result_len}
    pub result_reading_str:[u16; MAX_COMPOSITION_LENGTH],
    pub result_str:       [u16; MAX_COMPOSITION_LENGTH],  // GCS_RESULTSTR: committed text
}
```
`initialize(&mut self)`:
- `ZeroMemory(self)`, `info.dwSize = size_of::<UnimCompositionString>() as u32`.
- Set every `info.dw*Offset = offset_of!(UnimCompositionString, <field>) as u32`
  (use `core::mem::offset_of!`): `dwCompStrOffset`, `dwCompAttrOffset`,
  `dwCompClauseOffset`, `dwCompReadStrOffset`, `dwCompReadAttrOffset`,
  `dwCompReadClauseOffset`, `dwResultStrOffset`, `dwResultClauseOffset`,
  `dwResultReadStrOffset`, `dwResultReadClauseOffset`.
- All `dw*Len = 0` initially.

`set_preedit(&mut self, preedit_utf16: &[u16])`:
- Copy into `comp_str`; `info.dwCompStrLen = len`.
- Fill `comp_attr[0..len] = ATTR_INPUT` (whole string is being composed; use
  `ATTR_TARGET_CONVERTED` only for a converted/selected segment — N/A for plain
  hangul preedit). `info.dwCompAttrLen = len`.
- `comp_clause = [0, len]; info.dwCompClauseLen = 2 * size_of::<u32>() = 8`.
- `info.dwCursorPos = len` (caret at end of preedit).
- `info.dwDeltaStart = 0`.

`set_result(&mut self, result_utf16: &[u16])`:
- Copy into `result_str`; `info.dwResultStrLen = len`.
- `result_clause = [0, len]; info.dwResultClauseLen = 8` (Excel-2003 quirk: result
  needs clause info — mozc b/2959222).

`clear_preedit(&mut self)`: zero comp_* lens, `dwCursorPos = 0`.

### 5.4 IMCC access helper (composition.rs)
```rust
/// Lock the INPUTCONTEXT, ensure hCompStr is sized to UnimCompositionString,
/// run f over a &mut UnimCompositionString, unlock.
fn with_comp_string<R>(himc: HIMC, f: impl FnOnce(&mut UnimCompositionString) -> R) -> Option<R>;
//   ic = ImmLockIMC(himc) as *mut INPUTCONTEXT;
//   if (*ic).hCompStr == 0 { (*ic).hCompStr = ImmCreateIMCC(size); }
//   else if ImmGetIMCCSize((*ic).hCompStr) < size { (*ic).hCompStr = ImmReSizeIMCC(...); }
//   p = ImmLockIMCC((*ic).hCompStr) as *mut UnimCompositionString;
//   (if just created/resized: (*p).initialize())
//   r = f(&mut *p);
//   ImmUnlockIMCC((*ic).hCompStr); ImmUnlockIMC(himc); Some(r)
```

### 5.5 Message generation — TRANSMSGLIST (chosen approach)
**Decision: write messages into `lpTransBuf` (TRANSMSGLIST) and RETURN THE COUNT** —
the standard WDK/mozc `ImeToAsciiEx` path. (`ImmGenerateMessage` is used only OUTSIDE
`ImeToAsciiEx`, e.g. from `NotifyIME`, where there is no transbuf.)

`TRANSMSGLIST { uMsgCount: u32, TransMsg: [TRANSMSG; 1] }` — `TransMsg` is a
flexible array (capacity is `uMsgCount`, conventionally ~256). Write sequentially
through `(*lpTransBuf).TransMsg.as_mut_ptr().add(i)`; cap at `uMsgCount`. If more
messages than capacity are needed, the overflow goes into the context message buffer
(`dwNumMsgBuf`) via `ImmGenerateMessage` — but for single-clause hangul we never
exceed ~3, so capacity overflow is a non-issue (document the cap-check anyway).

`build_and_emit(himc, transbuf, result, preedit, prev_composing) -> u32`:
Compute the transition, then emit in THIS ORDER (each is one TRANSMSG = `{message, wParam, lParam}`):

| Situation | Messages (in order) |
|---|---|
| start (no comp before, preedit now non-empty) | `WM_IME_STARTCOMPOSITION(0,0)`, then `WM_IME_COMPOSITION(0, GCS_COMPSTR\|GCS_COMPATTR\|GCS_COMPCLAUSE\|GCS_CURSORPOS\|GCS_DELTASTART)` |
| update (already composing, preedit changed, no commit) | `WM_IME_COMPOSITION(0, <comp flags>)` |
| commit only, no new preedit (e.g. Enter/space final) | `WM_IME_COMPOSITION(lastChar, GCS_RESULTSTR\|GCS_RESULTCLAUSE)`, then `WM_IME_ENDCOMPOSITION(0,0)` |
| commit + new preedit (syllable rollover) | `WM_IME_COMPOSITION(0, GCS_RESULTSTR\|GCS_RESULTCLAUSE\|GCS_COMPSTR\|GCS_COMPATTR\|GCS_COMPCLAUSE\|GCS_CURSORPOS)` (single message carries BOTH; do NOT end+restart — same rationale as TSF `commit_and_restart`) |
| preedit cleared, no commit (cancel) | `WM_IME_COMPOSITION(0,0)`, then `WM_IME_ENDCOMPOSITION(0,0)` |

Before emitting, populate the IMCC via §5.4/§5.3 (`set_result` and/or `set_preedit`
or `clear_preedit`) so the flags in `WM_IME_COMPOSITION.lParam` match what's in the
buffer. `wParam` of `WM_IME_COMPOSITION` is the last result char for legacy apps that
read `wParam` (use 0 when no result, or the final UTF-16 unit of `result`).
Return the number of TRANSMSG entries written.

> Caret position: rely on `IME_PROP_AT_CARET`; the UI window positions the candidate
> window later. No `IMECHARPOSITION` handling in this scope.

---

## 6. Cargo.toml — exact contents (owner: scaffold)

```toml
[package]
name = "unim-imm32"
version.workspace = true
edition = "2021"
description = "UNIM Korean IME - Windows IMM32 (.ime) DLL for legacy/IMM32 apps"
license.workspace = true
repository.workspace = true
authors.workspace = true

[lib]
# IMM32 keyboard-layout IME. cdylib -> unim_imm32.dll; build scripts copy to unim_imm32.ime.
crate-type = ["cdylib"]

[dependencies]
unim = { path = ".." }

[target.'cfg(windows)'.dependencies]
windows-core = "0.62"
windows = { version = "0.62", features = [
    "Win32_Foundation",
    "Win32_System_LibraryLoader",
    "Win32_System_SystemServices",
    "Win32_System_Registry",
    "Win32_Globalization",
    "Win32_UI_Input_Ime",                # IMEINFO/COMPOSITIONSTRING/TRANSMSGLIST/CANDIDATELIST/HIMC/HIMCC + IME consts
    "Win32_UI_Input_KeyboardAndMouse",   # GetKeyState, VK_*, HKL
    "Win32_UI_WindowsAndMessaging",      # WM_IME_*, RegisterClass, DefWindowProc, HWND msgs
    "Win32_Graphics_Gdi",                # UI window paint (candidate window scaffold)
] }
```
> Do NOT add a `[profile.*]` block — workspace `panic="abort"` already applies.
> `Win32_UI_TextServices` is NOT needed (that's TSF). `Win32_System_Com` NOT needed.
> The `Imm*` functions are provided by the manual `#[link(name="imm32")]` block in
> `globals.rs` (§3.1), NOT by a windows feature — there is no feature that binds them.

---

## 7. build.rs plan (owner: scaffold)

```rust
fn main() {
    // Pass the module-definition file so x86 (i686) emits UNDECORATED stdcall exports
    // (ImeInquire, not _ImeInquire@12). Harmless on x64.
    println!("cargo:rustc-link-arg=/DEF:unim_imm32.def");
    println!("cargo:rerun-if-changed=unim_imm32.def");
    // imm32.dll is linked via #[link(name="imm32")] in globals.rs; no extra link-lib needed,
    // but make it explicit for clarity on both bitnesses:
    println!("cargo:rustc-link-lib=dylib=imm32");
}
```
Notes:
- `.def` path is relative to the crate root (where `cargo` runs the build); MSVC
  `link.exe` accepts `/DEF:`. On x64 the names are already undecorated, but the `.def`
  keeps both bitnesses identical and authoritative.
- **.dll → .ime rename happens in build scripts, NOT build.rs** — `scripts/build-msi.bat`
  copies `target/<triple>/release/unim_imm32.dll` to `unim_imm32.ime` (packaging owner).
- `unim_imm32.def` body:
  ```
  LIBRARY unim_imm32
  EXPORTS
      ImeInquire
      ImeConfigure
      ImeDestroy
      ImeEscape
      ImeSelect
      ImeSetActiveContext
      ImeProcessKey
      ImeToAsciiEx
      NotifyIME
      ImeSetCompositionString
      UIWndProc
      ImeRegisterWord
      ImeUnregisterWord
      ImeGetRegisterWordStyle
      ImeEnumRegisterWord
      ImeGetImeMenuItems
      ImeConversionList
  ```

---

## 8. register.rs (owner: register)

```rust
/// Dev-only: install the .ime and write the KLID keyboard-layout rows.
/// Primary install path is the MSI; this mirrors it for `cargo`-driven dev loops.
pub fn register_ime() -> windows::core::Result<()>;
//   1. ImmInstallIMEW(L"unim_imm32.ime", L"UNIM Korean (IMM32)") -> HKL (allocates an E0xx0412 KLID)
//   2. OR write static rows under
//      HKLM\SYSTEM\CurrentControlSet\Control\Keyboard Layouts\E0200412:
//      "Ime File"=REG_SZ "unim_imm32.ime"
//      "Layout File"=REG_SZ "KBDA1.DLL"        (Korean) [or "kbdus.dll"]
//      "Layout Text"=REG_SZ UNIM_LAYOUT_TEXT
//      "Layout Display Name"=REG_SZ "@%SystemRoot%\\System32\\unim_imm32.ime,-1000" (or plain text)
//      "Layout Id"=REG_SZ "00d2"               (any free 4-hex id)
pub fn unregister_ime() -> windows::core::Result<()>;  // UnloadKeyboardLayout + delete the KLID key
pub fn dbg_log(msg: &str);                              // OutputDebugStringW + optional %TEMP% file
```
> Keep KLID `E0200412` consistent with `installer/wix/unim.wxs` (packaging) and
> `globals.rs::UNIM_IMM32_KLID`.

---

## 9. ui_window.rs (owner: ui)

```rust
/// Register the UI window class named UNIM_UI_CLASS_NAME (idempotent). Called from
/// ImeInquire-time or first ImeSelect. Style: CS_IME; wndproc = UIWndProc.
pub fn ensure_ui_class_registered(hinst: HINSTANCE) -> windows::core::Result<()>;

/// The IME UI window procedure. Re-exported as the #[no_mangle] UIWndProc by lib.rs
/// (lib.rs defines the extern entry that simply forwards here), OR exported directly
/// from this module — lib.rs owns the #[no_mangle] symbol, ui owns the body via a
/// plain `pub fn ui_wndproc_impl(hwnd, msg, wparam, lparam) -> LRESULT`.
pub fn ui_wndproc_impl(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> isize;
//   WM_IME_NOTIFY: match wparam (IMN_OPENCANDIDATE/IMN_CLOSECANDIDATE/IMN_SETCOMPOSITIONWINDOW...)
//     -> stub: hide/show candidate window (candidate window = None for now).
//   WM_IME_STARTCOMPOSITION/ENDCOMPOSITION/COMPOSITION at UI level: no-op (apps draw at caret).
//   default: DefWindowProcW.
// TODO(unim-imm32): hanja/emoji candidate popup (engine.hanja_candidates_available) — leave hook.
```
> **Ownership split for `UIWndProc`:** the `#[no_mangle] extern "system" fn UIWndProc`
> lives in `lib.rs` (core) and is a one-line forwarder to `ui_window::ui_wndproc_impl`
> (ui). This keeps the export table in one file while ui owns the behavior. No file is
> co-edited.

---

## 10. Packaging deltas (owner: packaging — listed for completeness, NOT under unim-imm32/)

- `scripts/build-msi.bat`: add `cargo build --release --target x86_64-pc-windows-msvc -p unim-imm32`,
  then `copy /Y target\x86_64-pc-windows-msvc\release\unim_imm32.dll <stage>\unim_imm32.ime`.
- `.github/workflows/windows-msi.yml`: add `-p unim-imm32` to the build; artifact-verify `unim_imm32.ime`.
- `installer/wix/unim.wxs`: `<File>` for `unim_imm32.ime` into a System32 component
  (`Directory` = `SystemFolder`, `Win64="yes"`); RegistryKey rows under
  `HKLM\SYSTEM\CurrentControlSet\Control\Keyboard Layouts\E0200412` (Ime File / Layout
  File=KBDA1.DLL / Layout Text / Layout Display Name / Layout Id) per §8. NO `SelfRegCost`
  (IMM32 IMEs are registry-registered, not self-registered like the TSF COM DLL).

---

## 11. Out of scope (stub with `// TODO(unim-imm32):` + honest return)

Hanja/emoji candidate popup UI, reconversion (`ImeSetCompositionString` SCS_SETSTR,
`SCS_CAP_SETRECONVERTSTRING` consumer side), word registration
(`ImeRegisterWord`/`ImeEnumRegisterWord`/`ImeGetRegisterWordStyle`), IME menu
(`ImeGetImeMenuItems`), conversion list (`ImeConversionList`). Leave the engine hooks
(`hanja_candidates_available`, `special_char_candidates_available`) read but unwired.
