//! input.rs — VK + key-state → KeyCode/ModifierState bridge (owner: core)
//!
//! Two responsibilities (DESIGN.md §5.1):
//!  1. [`get_modifier_state`] — lifted from `unim-tsf/src/key_handler.rs:20`, but
//!     reading the **caller-supplied `lpbKeyState` array** (256 bytes) that IMM32
//!     hands to `ImeProcessKey`/`ImeToAsciiEx` instead of live `GetKeyState`. The
//!     IMM32 contract guarantees this snapshot is the keyboard state for the key
//!     being processed, which is exactly what we want (and is thread/order-safe).
//!  2. [`should_consume`] — a PURE should-consume probe mirroring
//!     `key_handler::test_key_down`, WITHOUT mutating engine state. `ImeProcessKey`
//!     returns this; `press_key` is NEVER called here.

use unim::config::{Config, InputCategory};
use unim::input_engine::AtfToggleKind;
use unim::keycode::{KeyCode, ModifierState};

use crate::ime_state::ImeContext;

/// Read the current modifier state from an IMM32 key-state snapshot.
///
/// `key_state` is the `lpbKeyState` pointer passed to `ImeProcessKey` /
/// `ImeToAsciiEx`: a 256-byte array indexed by virtual-key code, where bit 7 is
/// "down" and bit 0 is "toggled". This is the IMM32 analogue of the live
/// `GetKeyState` reads in `key_handler::get_modifier_state` (lifted verbatim in
/// spirit; the bit semantics are identical to `GetKeyState`'s return sign/LSB).
///
/// # Safety
/// `key_state` must point to a readable 256-byte array (the IMM32 contract
/// guarantees this for the documented callbacks). A null pointer yields an
/// all-false [`ModifierState`].
pub fn get_modifier_state(key_state: *const u8) -> ModifierState {
    unsafe { unim_windows_common::modifier::modifier_state_from_key_array(key_state) }
}

/// PURE should-consume probe — does this key get eaten by `ImeToAsciiEx`?
///
/// Mirrors `key_handler::test_key_down` (the TSF OnTestKeyDown path) against the
/// engine WITHOUT mutating it: no `press_key`, no commit/preedit drain. The
/// engine is borrowed `&` only (read-only `input_category`/`is_composing`).
///
/// Decision order (identical to TSF, minus popup which IMM32 wires later):
///  - modifier-only key → false
///  - Ctrl+Shift+Space (manual AutoTypeFix) → true
///  - 한/영 (VK_HANGUL) → true
///  - ATF 토글 핫키 (수정자 정확 일치 — 조합 포함) → true
///  - Ctrl/Alt/Super combo → false (system shortcut)
///  - Hanja / F9 → true
///  - Korean mode + character key → true
///  - English mode + character key + AutoTypeFix forward → true
///  - composing + Back/Space/Enter/Tab/Esc/arrows → true
///  - else false
///
/// ORDER MIRRORS `key_handler::test_key_down`: configured 한/영 toggle keys are
/// resolved first (so a modifier toggle key like RightAlt bypasses the
/// `is_modifier()` guard), then the shortcut filters, then the per-mode rules.
///
/// 한/영 토글키 처리 (RightAlt 포함): 엔진 `InputEngine::press_key` 가 설정된
/// `toggle_keys` 를 is_modifier 가드 *앞에서* 처리하도록 고쳐졌으므로(RightAlt 가
/// 자기 Alt 비트 때문에 단축키로 분류돼 토글이 죽던 버그 수정), 프로브도 동일하게
/// `engine.is_toggle_key()` 로 판정해 토글키를 소비한다. 소비 여부와 실제 토글
/// 동작을 정확히 일치시켜야 RightAlt 가 "먹히기만 하고 토글은 안 되는" 어긋남이
/// 생기지 않는다.
pub fn should_consume(ctx: &ImeContext, cfg: &Config, vkey: u32, key_state: *const u8) -> bool {
    let keycode = KeyCode::from_win32_vk(vkey as u16);
    let modifiers = get_modifier_state(key_state);
    let engine = &ctx.engine;

    // 설정된 한/영 전환키 여부 (엔진과 동일 판정). is_modifier 가드보다 먼저 구한다.
    let is_toggle = engine.is_toggle_key(keycode);

    // 수정자 키만 누른 경우 통과 — 단, 토글키(RightAlt 등)는 아래에서 소비 판정.
    if keycode.is_modifier() && !is_toggle {
        return false;
    }

    // Ctrl+Shift+Space: 수동 AutoTypeFix 소비
    if modifiers.control && modifiers.shift && keycode == KeyCode::Space {
        return true;
    }

    // 한/영 전환 키 — RightAlt 같은 수정자 토글키 포함, 단축키 조합(Ctrl/Super,
    // 또는 토글키가 수정자가 아닌데 Alt)일 때는 제외. 엔진 press_key 의 토글 판정과
    // 정확히 일치시킨다.
    if is_toggle {
        let self_is_modifier = keycode.is_modifier();
        let shortcut_combo = modifiers.control
            || modifiers.super_key
            || (modifiers.alt && !self_is_modifier);
        if !shortcut_combo {
            return true;
        }
    }

    // AutoTypeFix 토글 단축키 — press_key 의 atf_hotkey_kind 와 동일한 **수정자
    // 정확-일치** 판정(test_key_down 과 동일)이라 조합 표기(기본 `Shift+F8` 등)가
    // Linux 와 그대로 동작한다. 소비하지 않으면 앱이 ImeToAsciiEx 를 호출하지 않아
    // press_key 매칭 자체가 불가능하다. 정확-일치라 조합의 맨 base 키·미설정 조합은
    // 소비되지 않고, 목록이 비면 항상 false → 무회귀. 아래 Ctrl/Alt/Super 통과보다
    // 앞서야 조합 핫키가 살아남는다.
    if engine.is_atf_hotkey(keycode, modifiers) {
        return true;
    }

    // Ctrl/Alt/Super 조합은 통과 (단축키)
    if modifiers.control || modifiers.alt || modifiers.super_key {
        return false;
    }

    // 한자/F9 키
    if keycode == KeyCode::Hanja || keycode == KeyCode::F9 {
        return true;
    }

    // 한국어 모드: 문자 키 소비
    if engine.input_category() == InputCategory::Korean && keycode.is_character_key() {
        return true;
    }

    // 영문 모드 + ATF 순방향 ON: 문자 키를 소비해 IME 경로로 커밋한다.
    // (test_key_down 과 동일 — IMM32 앱에서 순방향 ATF 가 키를 관찰하려면 필요.)
    // 비밀번호/PIN 필드(content_purpose 감지 시)에서는 소비하지 않는다 — IMM32 에는
    // ATF 버퍼가 없어 소비 이득이 0 인데, 비번 문자를 WM_CHAR 대신 IME 결과 문자열
    // 경로로 우회시키는 호환성 리스크만 남기 때문. 통과시키면 앱이 문자를 직접 받는다.
    if engine.input_category() == InputCategory::English
        && keycode.is_character_key()
        && cfg.engine.auto_typefix.enabled
        && cfg.engine.auto_typefix.forward
        && !engine.content_purpose().should_block_hangul()
    {
        return true;
    }

    // 조합 중: Backspace, Space, Enter, Tab, Esc, 화살표 소비
    if engine.is_composing() {
        matches!(
            keycode,
            KeyCode::Backspace
                | KeyCode::Space
                | KeyCode::Enter
                | KeyCode::Tab
                | KeyCode::Escape
                | KeyCode::Left
                | KeyCode::Right
                | KeyCode::Up
                | KeyCode::Down
        )
    } else {
        false
    }
}

/// Result of feeding one key through the engine (the commit/preedit drain).
///
/// This is the IMM32 analogue of the `key_handler.rs:307-407` flow: call
/// `press_key`, then drain `commit_str` (clearing it) and read `preedit_str`.
/// `composition.rs` turns this pair into the COMPOSITIONSTRING + transmsgs.
pub struct KeyFeed {
    /// Whether the engine consumed the key (drives `ImeToAsciiEx` semantics /
    /// fallthrough decisions).
    pub consumed: bool,
    /// Newly committed (result) text this keystroke, already drained from the
    /// engine. Empty when nothing was committed.
    pub commit: String,
    /// Current preedit (composition) text after the keystroke.
    pub preedit: String,
    /// Whether the preedit changed (engine flag, passed through for callers).
    pub preedit_changed: bool,
    /// Whether commit changed (engine flag).
    pub commit_changed: bool,
}

/// Feed one key into the engine and drain its output.
///
/// Mirrors the non-fallback branch of `handle_key_down`:
///   `press_key` → if `commit_changed` take+clear `commit_str` → read `preedit_str`.
/// MUTATES the engine — call only from `ImeToAsciiEx`, never from the probe.
pub fn feed_key(ctx: &mut ImeContext, cfg: &mut Config, vkey: u32, key_state: *const u8) -> KeyFeed {
    let keycode = KeyCode::from_win32_vk(vkey as u16);
    let modifiers = get_modifier_state(key_state);

    let result = ctx.engine.press_key(keycode, modifiers, cfg);

    // ── ATF 토글 핫키 드레인 (press_key 매칭분 소비) ──
    // press_key 가 매칭한 ATF 토글을 여기서 소비해 config 플래그를 제자리 반전하고
    // 영속화한다. enabled/forward/reverse 는 press_key/should_consume 이 매 키 config
    // 에서 직접 읽으므로(엔진 캐시 아님) in-memory 반전만으로 다음 키부터 즉시 효력이
    // 난다. save_to_default_path 는 영속화 + 타 앱 전파용.
    //
    // TSF(text_service.rs)와 달리 IMM32 에는 config mtime 폴링·langbar·비프 인프라가
    // 없으므로 mtime 스냅샷 갱신과 차등 비프는 이식하지 않는다(자기 저장을 재감지하는
    // 폴링 자체가 없어 스퓨리어스 엔진 재생성 문제도 발생하지 않는다). 저장 실패는
    // IMM32 로깅 관례대로 로그만 남기고 삼킨다.
    if let Some(kind) = ctx.engine.take_atf_toggle() {
        let now_on = {
            let atf = &mut cfg.engine.auto_typefix;
            let flag = match kind {
                AtfToggleKind::Enabled => &mut atf.enabled,
                AtfToggleKind::Forward => &mut atf.forward,
                AtfToggleKind::Reverse => &mut atf.reverse,
            };
            *flag = !*flag;
            *flag
        };
        if let Err(e) = cfg.save_to_default_path() {
            crate::register::dbg_log(&format!("ATF 토글 config 저장 실패: {e:?}"));
        }
        crate::register::dbg_log(&format!("feed_key: ATF 토글 핫키 {kind:?} → on={now_on}"));
    }

    let commit = if result.commit_changed {
        let c = ctx.engine.commit_str().to_string();
        ctx.engine.clear_commit();
        c
    } else {
        String::new()
    };
    let preedit = ctx.engine.preedit_str().to_string();

    KeyFeed {
        consumed: result.consumed,
        commit,
        preedit,
        preedit_changed: result.preedit_changed,
        commit_changed: result.commit_changed,
    }
}
