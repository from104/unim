//! ime_state.rs — per-HIMC engine state (owner: core)
//!
//! Model (DESIGN.md §4): a process-global map keyed by the raw `HIMC` pointer
//! value, each entry holding one [`ImeContext`] (which owns an [`InputEngine`]).
//! The whole registry sits behind a single `Mutex`, which makes it `Send + Sync`
//! without any `unsafe impl`. IMM32 callbacks for a given HIMC arrive on that
//! HIMC's owning UI thread, but different HIMCs (windows/threads) can interleave,
//! so the map authority lives here — NOT in the per-context private data block
//! (which has no stable pointer across re-locks).

use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};

use windows::Win32::UI::Input::Ime::HIMC;

use unim::config::Config;
use unim::input_engine::InputEngine;

/// One IMM32 input context's engine state.
///
/// The [`InputEngine`] is the single source of truth for preedit/commit; future
/// candidate/UI bookkeeping can be added here without touching the engine.
pub struct ImeContext {
    pub engine: InputEngine,
    /// Authoritative HOST composition lifecycle flag: `true` once we have emitted
    /// `WM_IME_STARTCOMPOSITION` to the app and not yet balanced it with
    /// `WM_IME_ENDCOMPOSITION`. This is DISTINCT from `engine.is_composing()`
    /// (which tracks the engine's own preedit). The host flag must drive
    /// START/END so the pair stays balanced even after a dropped/zero-message
    /// keystroke (DESIGN.md §5.5).
    pub comp_open: bool,
}

impl ImeContext {
    fn new(config: &Config) -> Self {
        ImeContext {
            engine: InputEngine::new(config),
            comp_open: false,
        }
    }
}

/// Process-global registry. Key = `HIMC.0 as usize` (HIMC is a typed pointer handle).
pub struct ImeRegistry {
    contexts: HashMap<usize, ImeContext>,
}

impl ImeRegistry {
    fn new() -> Self {
        ImeRegistry {
            contexts: HashMap::new(),
        }
    }
}

/// `Send`/`Sync` wrapper around the registry `Mutex`.
///
/// DEVIATION FROM DESIGN.md §4: the spec asserted that `Mutex<ImeRegistry>`
/// alone makes the registry `Send + Sync` with "no `unsafe impl` needed". That
/// is FALSE for this engine — [`InputEngine`] embeds `Box<dyn HangulComposer>`
/// (no `+ Send`), so `InputEngine: !Send`, hence `Mutex<ImeRegistry>: !Sync`,
/// hence it cannot be a `static`. A documented, minimal `unsafe impl Sync` is
/// required.
///
/// SAFETY: this is sound under the IMM32 threading contract. IMM32 callbacks for
/// a given HIMC always arrive on that HIMC's owning UI thread, and every access
/// goes through [`with_context`]/`on_*`, which take the registry `Mutex` and
/// hold it for the whole (non-reentrant, non-blocking) closure. No `ImeContext`
/// is ever borrowed across threads or across a lock release; the `Mutex`
/// serialises all map access. The engine therefore never actually crosses a
/// thread boundary while in use — we only need to convince the type system the
/// container may live in a `static`.
struct SyncRegistry(Mutex<ImeRegistry>);
// `OnceLock<T>: Sync` requires `T: Send + Sync`; provide both (same safety
// argument as above — the engine never crosses a thread boundary while in use).
unsafe impl Sync for SyncRegistry {}
unsafe impl Send for SyncRegistry {}

static REGISTRY: OnceLock<SyncRegistry> = OnceLock::new();
/// Config is immutable after load; share one copy. (Reload on demand later.)
static CONFIG: OnceLock<Config> = OnceLock::new();
/// `DllMain` stores the module handle here.
static HINST: OnceLock<isize> = OnceLock::new();

#[inline]
fn registry() -> &'static Mutex<ImeRegistry> {
    &REGISTRY
        .get_or_init(|| SyncRegistry(Mutex::new(ImeRegistry::new())))
        .0
}

/// Turn a typed HIMC handle into the map key.
#[inline]
fn key(himc: HIMC) -> usize {
    himc.0 as usize
}

// ---------------------------------------------------------------------------
// public API used by lib.rs
// ---------------------------------------------------------------------------

/// Lazily-loaded, process-wide [`Config`]. Loaded from the default path on first
/// access. Layout (두벌식/세벌식) and 자동 한영 전환 live inside this Config and are
/// consumed entirely by the engine — IMM32 needs zero layout code.
pub fn config() -> &'static Config {
    CONFIG.get_or_init(Config::load_from_default_path)
}

/// Store the module handle (called from `DllMain` on `DLL_PROCESS_ATTACH`).
pub fn set_hinst(h: isize) {
    // First writer wins; later loads of the same DLL keep the original handle.
    let _ = HINST.set(h);
}

/// Module handle captured at `DLL_PROCESS_ATTACH` (0 if not yet set).
pub fn hinst() -> isize {
    HINST.get().copied().unwrap_or(0)
}

/// `ImeSelect(TRUE)`: create + bind a fresh [`ImeContext`] for this HIMC.
///
/// Idempotent: if the HIMC is already bound, the existing engine is reset so the
/// context starts clean (mirrors a re-select). Binding happens ONLY here.
pub fn on_select(himc: HIMC) {
    if himc.is_invalid() {
        return;
    }
    let cfg = config();
    let mut reg = match registry().lock() {
        Ok(g) => g,
        Err(p) => p.into_inner(),
    };
    match reg.contexts.get_mut(&key(himc)) {
        Some(ctx) => {
            ctx.engine.reset();
            ctx.comp_open = false;
        }
        None => {
            reg.contexts.insert(key(himc), ImeContext::new(cfg));
        }
    }
}

/// `ImeSelect(FALSE)` / `ImeDestroy`: reset the engine then drop the binding.
pub fn on_unselect(himc: HIMC) {
    let mut reg = match registry().lock() {
        Ok(g) => g,
        Err(p) => p.into_inner(),
    };
    if let Some(mut ctx) = reg.contexts.remove(&key(himc)) {
        ctx.engine.reset();
        ctx.comp_open = false;
    }
}

/// `ImeSetActiveContext(FALSE)`: flush/hide UI but keep the binding so the
/// composition can resume on re-focus. We conservatively reset the engine's
/// transient composition so no stale preedit lingers across focus loss.
pub fn on_deactivate(himc: HIMC) {
    let mut reg = match registry().lock() {
        Ok(g) => g,
        Err(p) => p.into_inner(),
    };
    if let Some(ctx) = reg.contexts.get_mut(&key(himc)) {
        ctx.engine.reset();
        ctx.comp_open = false;
    }
}

/// Run a closure with exclusive access to this HIMC's engine.
///
/// Returns `None` if the HIMC isn't bound (the caller should then return
/// `FALSE`/`0`). The registry `Mutex` is held for the whole closure, so `f` must
/// stay non-blocking and must NOT re-enter [`with_context`].
pub fn with_context<R>(himc: HIMC, f: impl FnOnce(&mut ImeContext) -> R) -> Option<R> {
    let mut reg = match registry().lock() {
        Ok(g) => g,
        Err(p) => p.into_inner(),
    };
    reg.contexts.get_mut(&key(himc)).map(f)
}
