# UNIM TSF — Unified Input Fix Plan

Status: implementation-ready synthesis of 3 investigated + verifier-reviewed bugs, re-checked against live source this session.
Target crate: `unim-tsf` (in-proc TSF COM cdylib). windows-rs **0.62.2**. Target triple `x86_64-pc-windows-msvc`.
Scope: BUG 1 (reversed Hangul), BUG 2 (no composition in console hosts), BUG 3 (no tray icon).

> IMPORTANT — current-tree reality check
> Most of the originally-proposed fixes were already partially applied in the working tree. This plan uses the **verifier-refined edits** confirmed against live source, NOT the stale originals. Where an original edit would fail to compile (duplicate field) or leak a COM ref (inline `ManuallyDrop` without `drop`), it is discarded in favor of the in-tree helper / corrected idiom.

---

## 0. Confirmed ground truth (read from live source this session)

`unim-tsf/src/composition.rs` (447 lines) — EXACT current state:

| Element | Line(s) | State (verified) |
|---|---|---|
| `unsafe fn move_caret_to_end(context: &ITfContext, ec: u32, range: &ITfRange) -> Result<()>` | 15–29 | EXISTS. `range.Clone()` → `Collapse(TF_ANCHOR_END)` → `TF_SELECTION{ range: ManuallyDrop::new(Some(end)), style: TF_SELECTIONSTYLE{ ase: TF_AE_END, fInterimChar: BOOL(0) } }` → `SetSelection(ec, slice::from_ref(&sel))` → `ManuallyDrop::drop(&mut sel.range)`. Ref-balanced SampleIME pattern. |
| `CompositionManager` struct fields | 32–35 | ONLY `composition`, `composition_slot`. (No `pending_text_or_arg` — the original BUG-2 edit referenced a non-existent field.) |
| `start_composition` | 52–77 | `let _ = context.RequestEditSession(...)` at **L70** discards the HRESULT. |
| `StartCompositionEditSession::DoEditSession` | 190–206 | hard `?` on `InsertTextAtSelection` (**L195**) + `StartComposition` (**L199**). ALREADY calls `move_caret_to_end` (**L201**). No GetSelection fallback. |
| `UpdateCompositionEditSession::DoEditSession` | 217–228 | calls `move_caret_to_end` (L224). OK. |
| `EndCompositionEditSession` struct | 232–237 | ALREADY has `context` field (**L234**). |
| `EndCompositionEditSession::DoEditSession` | 239–253 | ALREADY calls `move_caret_to_end` (**L247**) inside `if let Some(ref text)`; cancel path `text=None` only `EndComposition` (L249). OK. |
| `ReplaceSurroundingEditSession::DoEditSession` | 274–319 | `InsertTextAtSelection` (**L278–279**) + `StartComposition` (**L312**); already calls `move_caret_to_end` (L304/L313). |
| `InsertTextEditSession` struct | 323–327 | has `context` field (**L325**). |
| `InsertTextEditSession::DoEditSession` | 329–339 | ends at `range.SetText(ec, 0, &wide)?;` (**L335**) — **NO caret advance. ← BUG 1, the one open gap.** |
| `ReadSelectionEditSession` GetSelection idiom | 363–377 | `let mut sel = TF_SELECTION::default(); let mut fetched: u32 = 0; self.context.GetSelection(ec, TF_DEFAULT_SELECTION, std::slice::from_mut(&mut sel), &mut fetched)?;` then `sel.range.as_ref()` (L374). **Proven-compiling GetSelection fallback idiom in this crate.** |
| Imports | 3–8 | `use std::mem::ManuallyDrop;`, `use windows::core::*;`, `use windows::core::BOOL;`, `use windows::Win32::UI::TextServices::*;`. NOTE: `E_FAIL` is NOT imported here — BUG-2 EDIT A must add it. |

`unim-tsf/src/lang_bar.rs`:
- `GetIcon` stub at **L213–215**: `fn GetIcon(&self) -> Result<HICON> { Ok(HICON::default()) }` → null handle → blank tray. CONFIRMED.
- `dwStyle = TF_LBI_STYLE_BTN_BUTTON | TF_LBI_STYLE_SHOWNINTRAY` at L122 — correct, NOT the bug.
- Imports: `windows::core::*` (L10), `windows::Win32::Foundation::*` (L11), `windows::Win32::Graphics::Gdi::HBITMAP` (L12), `windows::Win32::UI::TextServices::*` (L13), `windows::Win32::UI::WindowsAndMessaging::{GetForegroundWindow, HICON}` (**L14**). → `HINSTANCE` already in scope (Foundation::\*), `PCWSTR` already in scope (core::\*). Only `LoadImageW`/`IMAGE_ICON`/`LR_DEFAULTCOLOR` need adding to L14.
- `OnUpdate(TF_LBI_STATUS | TF_LBI_ICON | TF_LBI_TEXT)` on mode change (L58) → sink re-queries GetIcon, so the icon refreshes on 한/영 toggle once GetIcon returns a real handle.

`unim-tsf/src/lib.rs`:
- **`pub fn dll_instance() -> HMODULE` (L49–51)** — CONFIRMED returns `HMODULE`, so `HINSTANCE(crate::dll_instance().0)` in BUG-3 GetIcon is correct (all are `*mut c_void` newtypes).
- `E_FAIL` imported from `Win32::Foundation` at lib.rs:38 (but composition.rs does not re-import it — see BUG-2 EDIT A).

`unim-tsf/src/key_handler.rs` (250–339): commit dispatch **L286–291**:
```rust
if !commit.is_empty() {
    if was_composing || comp_mgr.is_active() {
        comp_mgr.end_composition_with_text(context, tid, &commit); // PATH 2 — already fixed
    } else {
        comp_mgr.insert_text(context, tid, &commit);               // PATH 1 — BUG 1 open
    }
}
```
Preedit dispatch L299–311 confirms: empty → `end_composition`; active → `update_composition`; else → `start_composition`.

`unim-tsf/src/text_service.rs`: `OnKeyDown` returns `Ok(BOOL::from(eaten))`, never `?`-propagates composition errors (per prior verified read). Activation wiring (ActivateEx/AddItem/AdviseKeyEventSink/AdviseSink) correct.

`Cargo.toml`: crate-type cdylib; `Win32_UI_WindowsAndMessaging` feature present; **no `[build-dependencies]`, no embed-resource/winres, no build.rs, no .rc/.ico** in crate.

**One fact NOT confirmable this session (PowerShell denied / search tooling limited):** whether a `unim_log!` macro exists in `unim-tsf`. A grep for `unim_log` in composition.rs/lib.rs/key_handler.rs returned NOTHING. **Treat BUG-2 logging as: macro likely does NOT exist in unim-tsf — use `windows::Win32::System::Diagnostics::Debug::OutputDebugStringW` (viewable in DebugView/WinDbg) or whatever the crate's existing log entry point is. Do NOT assume `unim_log!`.**

---

# PART 1 — READY TO IMPLEMENT NOW (high confidence)

## BUG 1 — Reversed / stacked Hangul in Notepad

### Confirmed root cause
`ITfRange::SetText` writes text but does NOT advance the document selection (caret). The next non-composing commit re-queries the same unmoved caret via `InsertTextAtSelection(TF_IAS_QUERYONLY, &[])` and writes at the same offset → syllables stack/reverse (`안녕` → `녕안`). The engine is NOT at fault (`engine.rs:291` `preedit_str()` returns the cumulative cache; `press_key.rs:738-739` rebuilds the full preedit each keystroke; no `.rev()`/`insert(0,…)` anywhere). The in-tree doc comment on `move_caret_to_end` (composition.rs:10-14) states this mechanism verbatim.

**Most of this bug is already fixed.** Start (L201), Update (L224), End (L247), and ReplaceSurrounding (L304/L313) sessions all already call `move_caret_to_end`. The verifier rejected the original PATH-2 edits (they would re-add the `context` field that already exists at L234 → duplicate-field compile error) and rejected the original inline `TF_SELECTION` snippet (omits `ManuallyDrop::drop` → COM ref leak).

**One real gap remains:** `InsertTextEditSession::DoEditSession` (the non-composing commit dispatched from `key_handler.rs:290`) never advances the caret.

### Exact edit — THE ONLY change needed for BUG 1

File: `C:\Users\USER\Desktop\work\unim\unim-tsf\src\composition.rs`
Function: `InsertTextEditSession_Impl::DoEditSession` (lines 329–339)

OLD:
```rust
impl ITfEditSession_Impl for InsertTextEditSession_Impl {
    fn DoEditSession(&self, ec: u32) -> Result<()> {
        unsafe {
            let wide: Vec<u16> = self.text.encode_utf16().collect();
            let insert: ITfInsertAtSelection = self.context.cast()?;
            let range = insert.InsertTextAtSelection(ec, TF_IAS_QUERYONLY, &[])?;
            range.SetText(ec, 0, &wide)?;
        }
        Ok(())
    }
}
```

NEW:
```rust
impl ITfEditSession_Impl for InsertTextEditSession_Impl {
    fn DoEditSession(&self, ec: u32) -> Result<()> {
        unsafe {
            let wide: Vec<u16> = self.text.encode_utf16().collect();
            let insert: ITfInsertAtSelection = self.context.cast()?;
            let range = insert.InsertTextAtSelection(ec, TF_IAS_QUERYONLY, &[])?;
            range.SetText(ec, 0, &wide)?;
            // caret 을 삽입 텍스트 끝으로 이동 (비조합 commit 거꾸로 입력 방지)
            let _ = move_caret_to_end(&self.context, ec, &range);
        }
        Ok(())
    }
}
```

Why this and not the original proposal:
- `move_caret_to_end` (composition.rs:15-29) already exists, is `unsafe` (call sits inside the existing `unsafe` block), already handles the `ManuallyDrop<Option<ITfRange>>` lifetime correctly (Clone → Collapse(END) → SetSelection → `ManuallyDrop::drop`), and is the exact pattern the four other sessions use.
- `self.context` is already a field on `InsertTextEditSession` (L325) — no struct change, no call-site change.
- `let _ =` swallows errors, matching every other call site, so a SetSelection failure in a quirky client cannot break the commit.

DISCARD (do NOT apply):
- Original PATH 2a/2b/2c (add `context` to `EndCompositionEditSession`) — already in tree (L234/L247; call sites L97, L113). Re-applying 2a = duplicate-field compile error.
- Original inline `let sel = TF_SELECTION { range: ManuallyDrop::new(Some(range.clone())), … }; self.context.SetSelection(ec, &[sel])?;` — omits `ManuallyDrop::drop(&mut sel.range)` → unbalanced COM ref. Use the helper.

### windows-rs 0.62.2 API notes (BUG 1)
- `ITfContext::SetSelection(&self, ec: u32, pselection: &[TF_SELECTION]) -> Result<()>` — UNSAFE, slice arg. (Inside `move_caret_to_end`.)
- `TF_SELECTION { range: core::mem::ManuallyDrop<Option<ITfRange>>, style: TF_SELECTIONSTYLE }`. The helper builds it with `ManuallyDrop::new(Some(end))` and `ManuallyDrop::drop`s after the call (SetSelection AddRefs internally).
- `TF_SELECTIONSTYLE { ase: TfActiveSelEnd, fInterimChar: windows_core::BOOL }`, no bitfield. `TF_AE_END = TfActiveSelEnd(2)` (in-tree choice). `TF_ANCHOR_END` + `ITfRange::Clone`/`Collapse` all already compile.
- Signatures confirmed valid **by construction**: the helper is already compiled in a crate the task states builds, registers, processes keystrokes.

### Risk / regression (BUG 1)
- Low. Mirrors what Start/Update/End/ReplaceSurrounding already do successfully.
- Rich TSF clients (WordPad/Edge/Word) already advance the caret on SetText; an extra `SetSelection` to the same collapsed END anchor is idempotent — no double-advance.
- Cancel paths untouched (Esc / mid-composition Backspace go through `text=None` at L249).

---

## BUG 3 — No tray icon (Track A: embed icon + real GetIcon)

### Confirmed root cause
Two compounding defects (the `SHOWNINTRAY` flag is correct — NOT the cause):
1. **GetIcon returns a null HICON** (lang_bar.rs:213-215 stub `Ok(HICON::default())`). A SHOWNINTRAY lang-bar button with a null icon renders nothing.
2. **The DLL embeds no icon resource** — no `.ico`/`.rc`/build.rs/embed-resource in the crate. So `LanguageProfile IconFile=[#unim_tsf.dll] / IconIndex=0` (register.rs:131-138, unim.wxs:92-93) point at a non-existent resource → the OS "language options" list icon is blank too.

Track A embeds one icon group and makes GetIcon load it. Because the registry `IconFile/IconIndex` already exist, this single fix lights up the OS input-method list, language-options UI, docked language bar, and legacy tray simultaneously.

> Win11 caveat (set expectations): even a correct embedded icon may not reliably surface the **floating** `TF_LBI_STYLE_SHOWNINTRAY` glyph — that legacy tray is largely hidden in the new Win11 taskbar. Track A definitively fixes the OS list / language-options / docked bar. The floating tray glyph is what Track B (Part 3) closes.

### Exact edits (Track A)

**EDIT 3.1 — `unim-tsf/src/lang_bar.rs` imports (line 14).**
OLD:
```rust
use windows::Win32::UI::WindowsAndMessaging::{GetForegroundWindow, HICON};
```
NEW:
```rust
use windows::Win32::UI::WindowsAndMessaging::{GetForegroundWindow, HICON, LoadImageW, IMAGE_ICON, LR_DEFAULTCOLOR};
```
Do **NOT** add separate `use` lines for `HINSTANCE` (already via `Foundation::*` L11) or `PCWSTR` (already via `windows::core::*` L10) — duplicates trigger an unused/duplicate-import warning, and this crate is warning-0 intolerant.

**EDIT 3.2 — `unim-tsf/src/lang_bar.rs` GetIcon body (lines 213–215).**
OLD:
```rust
    fn GetIcon(&self) -> Result<HICON> {
        Ok(HICON::default())
    }
```
NEW:
```rust
    fn GetIcon(&self) -> Result<HICON> {
        unsafe {
            // dll_instance() 는 HMODULE 반환 (lib.rs:49) — HINSTANCE 로 변환.
            // HMODULE/HINSTANCE/HANDLE/HICON 모두 *mut c_void newtype 라 .0 변환 유효.
            let hinst = HINSTANCE(crate::dll_instance().0);
            let handle = LoadImageW(
                Some(hinst),
                PCWSTR(1 as *const u16), // 리소스 id 1 (첫 ICON 그룹). windows-rs 엔 MAKEINTRESOURCEW 없음.
                IMAGE_ICON,
                16, 16,
                LR_DEFAULTCOLOR,         // LR_SHARED 금지: TSF 가 핸들 소유 후 DestroyIcons (SampleIME 패턴)
            )?;
            Ok(HICON(handle.0))
        }
    }
```
CONFIRMED this session: `dll_instance()` returns `HMODULE` (lib.rs:49), so the `HINSTANCE(...)` wrap is correct (do NOT drop it).

**EDIT 3.3 — NEW FILE `unim-tsf/build.rs`:**
```rust
fn main() {
    #[cfg(windows)]
    {
        embed_resource::compile("unim-tsf.rc", embed_resource::NONE);
    }
}
```
`cfg(windows)`-gated so the Linux workspace build stays warning-0 and never invokes a Windows resource compiler.

**EDIT 3.4 — `unim-tsf/Cargo.toml`, append:**
```toml
[target.'cfg(windows)'.build-dependencies]
embed-resource = "2"
```
`Win32_UI_WindowsAndMessaging` already enabled, so `LoadImageW`/`IMAGE_ICON`/`LR_DEFAULTCOLOR` need no feature change.

**EDIT 3.5 — NEW FILE `unim-tsf/unim-tsf.rc`:**
```rc
1 ICON "assets/unim.ico"
```
Resource id `1` must match `PCWSTR(1 as *const u16)` in GetIcon. The FIRST icon group is positional index 0, so `register.rs`/`unim.wxs` `IconIndex=0` stays consistent (IconIndex is a 0-based index into RT_GROUP_ICON resources, independent of the id).

**EDIT 3.6 — REQUIRED ASSET (blocking, not polish): commit `unim-tsf/assets/unim.ico`** (multi-size, at least 16×16 and 32×32). Without it, EDIT 3.5 fails resource compilation and `cargo build -p unim-tsf` breaks.

**EDIT 3.7 — OPTIONAL POLISH `installer/wix/unim.wxs` (lines 233–235):** once `assets/unim.ico` exists, uncomment the `ARPPRODUCTICON`/`<Icon SourceFile=..\assets\unim.ico>` block so Add/Remove Programs shows the icon. `IconFile=[#unim_tsf.dll]/IconIndex=0` (L92-93) needs no change. Do this after the `.ico` lands.

### windows-rs 0.62.2 API notes (BUG 3)
- Trait method `fn GetIcon(&self) -> windows_core::Result<HICON>` — stub already matches; only the body changes.
- `LoadImageW<P1>(hinst: Option<HINSTANCE>, name: P1, r#type: GDI_IMAGE_TYPE, cx: i32, cy: i32, fuload: IMAGE_FLAGS) -> Result<HANDLE> where P1: Param<PCWSTR>`. The generic is on NAME; `hinst` is a concrete `Option<HINSTANCE>` → `Some(HINSTANCE(...))`. `PCWSTR: Param<PCWSTR>`.
- `IMAGE_ICON = GDI_IMAGE_TYPE(1)`; `LR_DEFAULTCOLOR = IMAGE_FLAGS(0)`; `LR_SHARED = IMAGE_FLAGS(32768)` — omit LR_SHARED (TSF DestroyIcons the handle each call; LR_SHARED would change ownership → double-free risk).
- Returns `HANDLE` → convert `HICON(handle.0)`. No `MAKEINTRESOURCEW` macro → `PCWSTR(1 as *const u16)`.
- Do **NOT** cache the HICON — load a fresh handle each call (SampleIME semantics).

### Risk / regression (BUG 3)
- None to the input path (key handling, composition, AddItem/RemoveItem untouched). GetIcon only feeds lang-bar/tray rendering.
- One hard failure mode: the `.rc` references `assets/unim.ico` → `cargo build -p unim-tsf` fails at resource compile until the `.ico` is committed (EDIT 3.6).
- Watch for an unused-import warning if the full original `use` list is applied blindly — EDIT 3.1 adds exactly three symbols.
- `build.rs`/embed-resource is `cfg(windows)`-gated → Linux `cargo build --workspace` warning-0 invariant preserved.

---

# PART 2 — NEEDS MORE INVESTIGATION (medium confidence; code path confirmed, exact failing call not yet pinned)

## BUG 2 — No Hangul composition in wezterm / console hosts (conhost, Windows Terminal)

### Confirmed root cause (direction + mechanism verified; needs a runtime log to pin the exact failing call)
The composition START path silently fails in console/CUAS hosts AND the `RequestEditSession` HRESULT is discarded, so a console failure produces zero visible output:
1. `OnKeyDown` → `handle_key_down` → `CompositionManager::start_composition` (composition.rs:52).
2. `start_composition` runs `StartCompositionEditSession` via `let _ = context.RequestEditSession(..., TF_ES_READWRITE | TF_ES_SYNC)` (**L70, HRESULT discarded**), then reads `composition_slot` (L73-76).
3. `StartCompositionEditSession::DoEditSession` (L190-206) uses hard `?` on `InsertTextAtSelection` (L195) and `StartComposition` (L199). In console hosts these often return `TF_E_DISCONNECTED` (0x80040054) / `E_NOTIMPL` / `E_FAIL`, or the host refuses the sync RW lock (`TF_E_NOLOCK`).
4. On failure `composition_slot` stays `None` → `self.composition` stays `None`. All later `update/end` calls are gated on `if let Some(ref composition)` (L79-124) → permanent no-op → console shows nothing. Notepad differs because its store grants the sync lock and supports InsertTextAtSelection/StartComposition.

Activation is ruled out (ActivateEx/AdviseKeyEventSink/AdviseSink correct; COMLESS category `{364215D9-75BC-11D7-A6EF-00065B84435C}` present at unim.wxs:145-149).

### Why this is PART 2 (not blind-apply)
- The original proposed edits do NOT compile / introduce a new bug, and one alternative root cause is not excluded:
  - Original EDIT #2 references `self.pending_text_or_arg` — **no such field** (CONFIRMED: `CompositionManager` = `{composition, composition_slot}` only, L32-35).
  - Original EDIT #2 degrade would re-insert the growing preedit every keystroke → duplicated `ㄱ가각`.
  - Original fallback `sel[0].range.clone().ok_or(E_FAIL)?` mis-derefs `ManuallyDrop<Option<ITfRange>>` → must be `sel.range.as_ref().ok_or(E_FAIL)?.clone()` (matches in-tree idiom at L374).
  - **Alternative cause not excluded:** some wezterm builds are IMM32-only, so `OnKeyDown` may never fire there. **Confirm via runtime log BEFORE trusting the edit-session fix.**

### Step 0 (DO FIRST) — diagnostic to pin the failing call
Add diagnostic logging via **`OutputDebugStringW`** (a `unim_log!` macro does NOT appear to exist in unim-tsf — grep returned nothing; use OutputDebugStringW viewable in DebugView/WinDbg, or the crate's actual log entry point if one exists). Log:
- In `start_composition` after L70: the `RequestEditSession` HRESULT (EDIT B captures it).
- In `StartCompositionEditSession::DoEditSession`: the `InsertTextAtSelection` Result (L195) and `StartComposition` Result (L199).
- Type a Hangul key in wezterm:
  - If a failure/no-lock HRESULT or `TF_E_DISCONNECTED`/`E_NOTIMPL` appears and `composition_slot` ends `None` → root cause confirmed → apply EDITs A/B/C.
  - If **no** OnKeyDown/edit-session log appears at all → host is IMM32-only → different fix (IMM32 support) → re-scope; do NOT apply A/C.

### Refined edits (apply only after Step 0 confirms the TSF path)

All in `C:\Users\USER\Desktop\work\unim\unim-tsf\src\composition.rs`. Reuse the file's proven GetSelection idiom (L363-377).

**EDIT A — `StartCompositionEditSession::DoEditSession` (L190-206): GetSelection fallback.**
Replace L194-195:
OLD:
```rust
            let insert: ITfInsertAtSelection = self.context.cast()?;
            let range = insert.InsertTextAtSelection(ec, TF_IAS_QUERYONLY, &[])?;
```
NEW:
```rust
            let range = match self.context.cast::<ITfInsertAtSelection>()
                .and_then(|ins| ins.InsertTextAtSelection(ec, TF_IAS_QUERYONLY, &[]))
            {
                Ok(r) => r,
                Err(_) => {
                    // console/CUAS host lacking InsertTextAtSelection: 현재 selection 에서 range 도출
                    let mut sel = TF_SELECTION::default();
                    let mut fetched: u32 = 0;
                    self.context.GetSelection(ec, TF_DEFAULT_SELECTION, std::slice::from_mut(&mut sel), &mut fetched)?;
                    if fetched == 0 { return Err(E_FAIL.into()); }
                    sel.range.as_ref().ok_or(E_FAIL)?.clone()
                }
            };
```
Keep L196 `SetText`, L198-199 `StartComposition`, L201 `move_caret_to_end` as-is. **`E_FAIL` is NOT imported in composition.rs** — add `use windows::Win32::Foundation::E_FAIL;` to the import block (it is available; lib.rs:38 imports it from the same path).

**EDIT B — `start_composition` (L52-77): capture the HRESULT (both outer Err AND inner failure hr).**
OLD (L69-71):
```rust
        unsafe {
            let _ = context.RequestEditSession(tid, &session_intf, TF_ES_READWRITE | TF_ES_SYNC);
        }
```
NEW:
```rust
        let hr = unsafe { context.RequestEditSession(tid, &session_intf, TF_ES_READWRITE | TF_ES_SYNC) };
        match hr {
            Err(_e) => { /* log via OutputDebugStringW: RequestEditSession COM error */ }
            Ok(session_hr) if session_hr.is_err() => { /* log: edit session failure hr (console no-lock?) */ }
            Ok(_) => {}
        }
```
CRITICAL: `RequestEditSession` returns `Result<HRESULT>` — the **inner** HRESULT (`phrsession`) carries the SYNC session result. A console lock-refusal can appear as `Ok(failure_hr)`, which an Err-only check misses. Leave the L73-76 slot read as-is. Do **NOT** add a preedit-mirroring degrade here. (Replace the comment placeholders with real `OutputDebugStringW` calls.)

**EDIT C — `ReplaceSurroundingEditSession::DoEditSession` (L274-319): same fallback.**
Replace L278-279 (`InsertTextAtSelection(ec, TF_IAS_QUERYONLY, &[])?`) with the SAME `match` block as EDIT A. NOTE: a GetSelection-derived range equals the caret only when the selection is empty; the existing L283 `Collapse(TF_ANCHOR_START)` normalizes for the delete branch, but for the `delete_chars==0` + commit path consider an explicit `Collapse(TF_ANCHOR_END)` if the fallback range was a non-empty selection. Test AutoTypeFix in console after the change.

**EDIT D — `text_service.rs`: NONE.** `OnKeyDown` already returns `Ok(BOOL)` without aborting. The existing commit path (`key_handler.rs:286-291`) already routes the final commit through `insert_text` when `comp_mgr.is_active()` is false — so the console degrade is **automatic**; no extra degrade code needed (this is what avoids the `ㄱ가각` double-insert). Do NOT add `GetActiveFlags` console detection (out of scope).

### windows-rs 0.62.2 API notes (BUG 2)
- `ITfContext::RequestEditSession(&self, tid: u32, pes: Ref<ITfEditSession>, dwflags: TF_CONTEXT_EDIT_CONTEXT_FLAGS) -> Result<HRESULT>`. Handle BOTH `Err(outer)` and `Ok(hr) where hr.is_err()`.
- `ITfContextComposition::StartComposition(ec, Ref<ITfRange>, Ref<ITfCompositionSink>) -> Result<ITfComposition>` — `&range`/`&self.comp_sink` coerce to `Ref`.
- `ITfContext::GetSelection(ec, TF_DEFAULT_SELECTION, &mut [TF_SELECTION], &mut u32)` — proven compiling at composition.rs:367-370.
- `TF_SELECTION.range` is `ManuallyDrop<Option<ITfRange>>` → `sel.range.as_ref().ok_or(E_FAIL)?.clone()` (never `sel[0].range.clone()`).
- Pre-existing latent note (out of scope): `ReadSelectionEditSession` L374-377 does `sel.range.as_ref().clone()` leaving the `ManuallyDrop` original undropped (minor COM ref leak). The fallback follows the same idiom for consistency; a future cleanup could `ManuallyDrop::into_inner`.

### Risk / regression (BUG 2)
- GetSelection fallback (A/C) is SAFE for Notepad — only triggers when `InsertTextAtSelection` errors, which Notepad does not; happy path unchanged.
- Do NOT add the original commit-degrade (would cause `ㄱ가각`). The existing `insert_text` route already covers the degrade.
- ReplaceSurrounding + non-empty selection: collapse before delete or gate to empty-selection; test ATF in console.
- `ManuallyDrop` misuse would be UB — follow the `as_ref().clone()` idiom exactly.

---

# PART 3 — LARGER FOLLOW-UP (out of immediate scope, route to PM)

## BUG 3 Track B — Standalone tray indicator for Win11
- New crate `unim-tray/` — a standalone Win32 `.exe` using `Shell_NotifyIcon` for a guaranteed-visible Win11 tray indicator (the floating `SHOWNINTRAY` glyph is unreliable on the new Win11 taskbar).
- Autostart at logon (HKCU `Run`); sync `is_korean` state from the existing `HKCU\Software\atit.org\UNIM` registry key (state-sync IPC with the in-proc DLL).
- License: Weasel ships `WeaselServer.exe` for exactly this purpose but is GPLv3 — its out-of-proc Named-Pipe model is **algorithm-only reuse, do NOT copy code**.
- Substantial: new crate + logon autostart + IPC. Schedule via PM after Track A user feedback confirms the Win11 floating-glyph gap matters.

---

# APPLY ORDER, BUILD, VERIFY, ROLLBACK

## Recommended apply order
1. **BUG 1** (composition.rs `InsertTextEditSession` one-liner) — highest confidence, smallest blast radius, fixes the most visible defect. Apply + build + Notepad-verify FIRST in isolation.
2. **BUG 3 Track A** (icon embed + GetIcon) — independent of the input path; needs the `.ico` asset committed first. Apply + build + verify the OS language-options list icon.
3. **BUG 2** — apply the Step-0 diagnostic and confirm wezterm uses TSF (not IMM32) BEFORE the A/B/C edits. Highest uncertainty; do last, gated on the log.

Apply BUG 1 and BUG 3 as **separate commits** so each reverts independently. Do not bundle BUG 2 until Step 0 confirms the path.

## Build (user)
```
scripts\build-msi.bat
```
Then clean-reinstall:
```
scripts\unim-clean-reinstall.ps1
```
(or `regsvr32` the rebuilt `unim-tsf.dll` for a quick DLL-only swap). Gate: workspace builds **warning-0**; `cargo test --workspace` all-pass; Linux `cargo build --workspace` stays warning-0 (BUG 3 `build.rs` is `cfg(windows)`-gated).

## User verification
**BUG 1 (Notepad):** switch to UNIM, type a multi-syllable word (e.g. `d k s` `s u d` → 안녕). PASS = left-to-right, caret advances per syllable. Also commit while NOT composing (digit/space between syllables) to exercise PATH 1. Regression: repeat in WordPad + Edge address bar (no double advance); Esc + mid-composition Backspace cancel cleanly.

**BUG 3 Track A (icon):** after committing `assets/unim.ico` and rebuilding — Settings → Time & Language → Language & region → Korean → Language options → Keyboards: the UNIM entry shows the embedded icon. Switch to UNIM: docked lang-bar button shows the icon. Inspect DLL with Resource Hacker (Icon Group at id 1). If the **floating** Win11 tray glyph is still absent, that is expected → Track B.

**BUG 2 (console):** after Step 0 diagnostic — launch wezterm, switch to UNIM, type a Hangul key. Read DebugView output to confirm which call fails (RequestEditSession lock vs InsertTextAtSelection vs StartComposition). After A/B/C: wezterm should compose (type `gks` → 한; ordering already fixed by BUG 1). Repeat in conhost + Windows Terminal. Regression: Notepad still composes; ATF still works in Notepad and console. If NO OnKeyDown log appeared in wezterm → IMM32-only → re-scope.

## Rollback path
- Each bug is an isolated, additive change. Revert the corresponding commit:
  - BUG 1: revert the one added line in `InsertTextEditSession::DoEditSession` → returns to current (mostly-fixed) state.
  - BUG 3: revert lang_bar.rs imports + GetIcon body, delete `build.rs`/`unim-tsf.rc`/Cargo.toml `[build-dependencies]` block, remove `assets/unim.ico`. (The registry `IconFile/IconIndex` values are harmless when the resource is absent — same as today.)
  - BUG 2: revert composition.rs EDITs A/B/C; the diagnostic logging can stay (harmless) or be removed.
- No registry/MSI schema change required for BUG 1 or BUG 2, so a DLL-only `regsvr32` swap is a safe fast rollback for those.
