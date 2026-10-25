//! 한자 단어 대상③ — 커서 주변 단어 (HANJA_WORD_SPEC v1.6 §2.4b·§2.8).
//!
//! 프런트가 보고한 surrounding(커서 앞뒤 문서)에서 커서가 걸친 한글 구간의 사전 일치
//! 후보를 만들고, 한자키 재타로 순환한다. 문서를 건드리는 사건(확정·통과 키·포인터)이
//! 마지막 보고 뒤에 있었는지(낡음)와 앱에 선택이 있을 수 있는지(거부 플래그)를 이
//! 모듈이 추적한다. 로그에는 사용자 텍스트 평문을 남기지 않는다(길이·수치·이유 코드만).

use super::engine::InputEngine;
use super::types::{
    InputResult, SurroundingWordCarry, HANJA_MAX_KEY_CHARS, SURROUNDING_CAP_POINTER_HINTS,
    SURROUNDING_CAP_REPLACE, SURROUNDING_CAP_SELECTION_RELIABLE, SURROUNDING_DEFER_WAIT_MS,
    SURROUNDING_HINT_FIELD_FOCUS, SURROUNDING_HINT_KEY, SURROUNDING_HINT_POINTER_PLAIN, SURROUNDING_HINT_POINTER_SELECT,
    SURROUNDING_HINT_PRIMARY_CHANGED, SURROUNDING_HINT_SELECTION_UNKNOWN,
};
use crate::hangul::char::HangulCharExt;
use crate::keycode::{KeyCode, ModifierState};
use crate::unim_log;

/// 선택 거부 플래그를 올린 원인(로그용).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) enum SelFlagCause {
    #[default]
    None,
    ShiftNav,
    CtrlA,
    PointerSelect,
    PrimaryChanged,
    FocusIn,
    SelUnknown,
    /// push 프런트가 보고한 필드 포커스(`SURROUNDING_HINT_FIELD_FOCUS`).
    FieldFocus,
}

impl SelFlagCause {
    pub(super) fn as_u8(self) -> u8 {
        match self {
            Self::None => 0,
            Self::ShiftNav => 1,
            Self::CtrlA => 2,
            Self::PointerSelect => 3,
            Self::PrimaryChanged => 4,
            Self::FocusIn => 5,
            Self::SelUnknown => 6,
            Self::FieldFocus => 7,
        }
    }

    pub(super) fn from_u8(v: u8) -> Self {
        match v {
            1 => Self::ShiftNav,
            2 => Self::CtrlA,
            3 => Self::PointerSelect,
            4 => Self::PrimaryChanged,
            5 => Self::FocusIn,
            6 => Self::SelUnknown,
            7 => Self::FieldFocus,
            _ => Self::None,
        }
    }

    /// 로그용 이름(수치·코드만).
    pub(super) fn name(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::ShiftNav => "shift_nav",
            Self::CtrlA => "ctrl_a",
            Self::PointerSelect => "pointer_select",
            Self::PrimaryChanged => "primary_changed",
            Self::FocusIn => "focus_in",
            Self::SelUnknown => "sel_unknown",
            Self::FieldFocus => "field_focus",
        }
    }
}

/// 키가 선택 거부 플래그에 주는 효과.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum SelKeyEffect {
    Set(SelFlagCause),
    Clear,
    Keep,
}

/// 마지막 surrounding 보고의 기준점(R5, push 전용). 시각은 후보(`sw_move_at`)에만 쓴다.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct SwReport {
    pub(super) cursor: u32,
    /// 이 보고 뒤 키·KEY 힌트·커밋이 있었는가.
    pub(super) keys_since: bool,
}

/// 대상③ 판정 결과(`surrounding_word_judge`). `settled` = R6 이 FocusIn·FieldFocus 원인
/// 플래그를 풀어 준 판정(호출부가 플래그를 실제로 내린다).
pub(super) enum SwJudge {
    Ok { cands: Vec<SwCand>, settled: bool },
    Reject { reason: &'static str, run: u32, settled: bool },
    /// R7(`stale`)·R8(`settle`) — 호스트가 `deadline_ms` 에 다시 판정한다.
    Defer { deadline_ms: u64, reason: &'static str },
}

/// 보류 중인 한자키를 다시 판정하게 한 호스트 사건(`InputEngine::poll_deferred_hanja`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DeferTrigger {
    /// `SetSurroundingText`(F2)가 엔진에 반영된 직후.
    Report,
    /// 호스트 타이머가 `deferred_hanja_deadline()` 에 도달했다.
    Deadline,
}

/// 대상③ 후보 한 건 — 사전 키와 커서 기준 앞 a자·뒤 b자.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct SwCand {
    pub(super) key: String,
    pub(super) a: u32,
    pub(super) b: u32,
}

fn is_nav_key(k: KeyCode) -> bool {
    matches!(
        k,
        KeyCode::Left
            | KeyCode::Right
            | KeyCode::Up
            | KeyCode::Down
            | KeyCode::Home
            | KeyCode::End
            | KeyCode::PageUp
            | KeyCode::PageDown
    )
}

/// 키 하나가 선택 거부 플래그에 주는 효과(순수 함수, I7: 표에 없는 키는 `Keep`).
pub(super) fn classify_sel_key(keycode: KeyCode, m: ModifierState) -> SelKeyEffect {
    if m.alt || m.super_key {
        return SelKeyEffect::Keep;
    }
    if is_nav_key(keycode) {
        return if m.shift {
            SelKeyEffect::Set(SelFlagCause::ShiftNav)
        } else {
            SelKeyEffect::Clear
        };
    }
    match keycode {
        KeyCode::Enter | KeyCode::Tab | KeyCode::Backspace | KeyCode::Delete => SelKeyEffect::Clear,
        KeyCode::A if m.control && !m.shift => SelKeyEffect::Set(SelFlagCause::CtrlA),
        KeyCode::X | KeyCode::V | KeyCode::Z | KeyCode::Y if m.control => SelKeyEffect::Clear,
        _ => SelKeyEffect::Keep,
    }
}

/// KEY 힌트의 X11 keysym → 엔진 키코드(탐색·편집·a/x/v/z/y 만).
fn keysym_to_keycode(keyval: u32) -> Option<KeyCode> {
    Some(match keyval {
        0xff51 | 0xff96 => KeyCode::Left,
        0xff52 | 0xff97 => KeyCode::Up,
        0xff53 | 0xff98 => KeyCode::Right,
        0xff54 | 0xff99 => KeyCode::Down,
        0xff50 | 0xff95 => KeyCode::Home,
        0xff57 | 0xff9c => KeyCode::End,
        0xff55 | 0xff9a => KeyCode::PageUp,
        0xff56 | 0xff9b => KeyCode::PageDown,
        0xff0d | 0xff8d => KeyCode::Enter,
        0xff09 | 0xfe20 => KeyCode::Tab,
        0xff08 => KeyCode::Backspace,
        0xffff | 0xff9f => KeyCode::Delete,
        0x61 | 0x41 => KeyCode::A,
        0x78 | 0x58 => KeyCode::X,
        0x76 | 0x56 => KeyCode::V,
        0x7a | 0x5a => KeyCode::Z,
        0x79 | 0x59 => KeyCode::Y,
        _ => return None,
    })
}

/// KEY 힌트 분류 — `(수정자 단독 키 여부, 효과)`. 상태는 X11/Clutter 마스크.
///
/// 수정자 없이 눌린 인쇄 가능 키(글자 입력)는 선택을 대체하므로 `Clear` 다.
pub(super) fn classify_sel_keysym(keyval: u32, state: u32) -> (bool, SelKeyEffect) {
    if (0xffe1..=0xffee).contains(&keyval) {
        return (true, SelKeyEffect::Keep);
    }
    let m = ModifierState::from_x11_mask(state);
    if let Some(k) = keysym_to_keycode(keyval) {
        let e = classify_sel_key(k, m);
        // 수정자 없는 a/x/v/z/y 는 글자 입력이다.
        let printable_plain = matches!(k, KeyCode::A | KeyCode::X | KeyCode::V | KeyCode::Z | KeyCode::Y)
            && !m.control
            && !m.alt
            && !m.super_key;
        return (false, if printable_plain { SelKeyEffect::Clear } else { e });
    }
    let printable = (0x20..=0x7e).contains(&keyval) || (0xa0..=0xff).contains(&keyval) || keyval >= 0x0100_0000;
    if printable && !m.control && !m.alt && !m.super_key {
        return (false, SelKeyEffect::Clear);
    }
    (false, SelKeyEffect::Keep)
}

/// KEY 힌트 로그용 이름 — 탐색·편집 키만 이름을 내고 나머지는 글자를 숨긴다(§2.8).
pub(super) fn hint_key_log_name(keyval: u32) -> &'static str {
    match keyval {
        0xff51 | 0xff96 => "left",
        0xff52 | 0xff97 => "up",
        0xff53 | 0xff98 => "right",
        0xff54 | 0xff99 => "down",
        0xff50 | 0xff95 => "home",
        0xff57 | 0xff9c => "end",
        0xff55 | 0xff9a => "pgup",
        0xff56 | 0xff9b => "pgdn",
        0xff0d | 0xff8d => "enter",
        0xff09 | 0xfe20 => "tab",
        0xff08 => "backspace",
        0xffff | 0xff9f => "delete",
        _ => "char",
    }
}

impl InputEngine {
    // =========================================
    // 능력·사건 보고
    // =========================================

    /// 클라이언트 능력 비트를 설정한다(`SURROUNDING_CAP_*`). 0(기본)이면 v1.5 와 같다.
    pub fn set_surrounding_word_caps(&mut self, caps: u32) {
        self.surrounding_caps = caps;
    }

    /// 현재 능력 비트.
    pub fn surrounding_word_caps(&self) -> u32 {
        self.surrounding_caps
    }

    /// 프런트 종류 꼬리표를 설정한다(대상③ 판정 로그 `client=` 전용 — 동작 무관).
    /// 값: `gtk3|gtk4|qt5|qt6|gnome|tsf|xim|wayland|ibus|unknown`.
    pub fn set_client_tag(&mut self, tag: &'static str) {
        self.client_tag = tag;
    }

    /// 현재 프런트 종류 꼬리표.
    pub fn client_tag(&self) -> &'static str {
        self.client_tag
    }

    fn set_sel_flag(&mut self, cause: SelFlagCause) {
        self.selection_possible = true;
        self.selection_flag_cause = cause;
    }

    pub(super) fn clear_sel_flag(&mut self) {
        self.selection_possible = false;
        self.selection_flag_cause = SelFlagCause::None;
    }

    fn apply_sel_effect(&mut self, e: SelKeyEffect) {
        match e {
            SelKeyEffect::Set(c) => self.set_sel_flag(c),
            SelKeyEffect::Clear => self.clear_sel_flag(),
            SelKeyEffect::Keep => {}
        }
    }

    /// 프런트가 엔진 밖에서 관측한 사건을 보고한다(`SurroundingHint`).
    pub fn surrounding_hint(&mut self, kind: u32, keyval: u32, state: u32) {
        match kind {
            SURROUNDING_HINT_KEY => {
                let (is_mod, effect) = classify_sel_keysym(keyval, state);
                if is_mod {
                    return; // 수정자 단독 키는 문서를 바꾸지 않는다
                }
                self.surrounding_stale = true;
                self.sw_cancel_defer("hint_key");
                self.sw_note_key_event();
                self.apply_sel_effect(effect);
                unim_log!(
                    "ENGINE",
                    "SurroundingHint: kind={} key={}",
                    kind,
                    hint_key_log_name(keyval)
                );
            }
            SURROUNDING_HINT_POINTER_PLAIN => {
                self.surrounding_stale = true;
                self.clear_sel_flag();
                // R10 — 다른 위치를 눌렀다: 보류한 한자키는 옛 위치 기준이라 조용히 취소한다.
                self.sw_cancel_defer("pointer");
                unim_log!("ENGINE", "SurroundingHint: kind={} (pointer_plain)", kind);
            }
            SURROUNDING_HINT_POINTER_SELECT => {
                self.surrounding_stale = true;
                self.sw_cancel_defer("pointer");
                self.sw_note_select_signal();
                self.set_sel_flag(SelFlagCause::PointerSelect);
                unim_log!("ENGINE", "SurroundingHint: kind={} (pointer_select)", kind);
            }
            SURROUNDING_HINT_PRIMARY_CHANGED => {
                self.sw_note_select_signal();
                self.set_sel_flag(SelFlagCause::PrimaryChanged);
                unim_log!("ENGINE", "SurroundingHint: kind={} (primary_changed)", kind);
            }
            SURROUNDING_HINT_FIELD_FOCUS => {
                if self.sw_push() {
                    // R4: FocusIn 과 같은 표시. 모드 복원·ATF 리셋은 호스트의 FocusIn 몫이라 하지 않는다.
                    self.sw_focus_mark(SelFlagCause::FieldFocus, "field_focus");
                    unim_log!("ENGINE", "SurroundingHint: kind={} (field_focus)", kind);
                } else {
                    unim_log!("ENGINE", "SurroundingHint: kind={} (field_focus) — push 아님, 무시", kind);
                }
            }
            SURROUNDING_HINT_SELECTION_UNKNOWN => {
                self.selection_unknown_once = true;
                unim_log!("ENGINE", "SurroundingHint: kind={} (selection_unknown)", kind);
            }
            _ => {
                unim_log!("ENGINE", "SurroundingHint: 알 수 없는 kind={} — 무시", kind);
            }
        }
    }

    /// FocusIn — 이전 앱의 상태를 믿지 않는다(낡음·선택 가능 표시, 소진·불명 해제).
    pub fn surrounding_word_focus_in(&mut self) {
        self.sw_focus_mark(SelFlagCause::FocusIn, "focus_in");
    }

    /// FocusIn(창)·FIELD_FOCUS(필드) 공통 표시. push 상태(기준점·후보·보류)도 함께 비운다 —
    /// 포커스 뒤 첫 보고(캐시 재송신 포함)는 기준점으로만 쓰이고 "커서 이동"으로 세지 않는다.
    fn sw_focus_mark(&mut self, cause: SelFlagCause, why: &'static str) {
        self.surrounding_stale = true;
        self.set_sel_flag(cause);
        self.surrounding_word_exhausted = false;
        self.selection_unknown_once = false;
        self.sw_last_report = None;
        self.sw_move_at = None;
        self.sw_focus_at = Some(self.sw_now_ms);
        self.sw_cancel_defer(why);
    }

    // =========================================
    // push 컨텍스트 상태기계 (R5~R10)
    // =========================================

    /// 호스트의 단조 ms 를 설정한다. push 컨텍스트의 사건(보고·힌트·한자키)을 엔진에 넣기
    /// **직전에** 호출한다. 코어는 시계를 읽지 않는다(결정적). S 비트가 있는 컨텍스트는 쓰지 않는다.
    pub fn set_now_ms(&mut self, now_ms: u64) {
        self.sw_now_ms = now_ms;
    }

    /// push 컨텍스트 = 선택을 신뢰할 수 없고(`S` 없음) 포인터 힌트를 받는다(`P`).
    pub(super) fn sw_push(&self) -> bool {
        self.surrounding_caps & SURROUNDING_CAP_SELECTION_RELIABLE == 0
            && self.surrounding_caps & SURROUNDING_CAP_POINTER_HINTS != 0
    }

    /// 판정을 미룬 한자키의 기한(ms). 없으면 `None`. 호스트는 이 시각이나 다음 surrounding
    /// 보고 도착 때 `resume_deferred_hanja` 를 부른다.
    pub fn deferred_hanja_deadline(&self) -> Option<u64> {
        self.sw_pending_defer
    }

    /// 마지막 대상③ 판정 이유 코드(`ok`·`stale`·`defer`·…). 진단·적합성 시험용.
    pub fn surrounding_word_last_reason(&self) -> &'static str {
        self.sw_last_reason
    }

    /// 보류 중인 한자키를 조용히 취소한다(호스트가 엔진을 재생성하기 전 등). 보류가 없으면 무동작.
    pub fn cancel_deferred_hanja(&mut self) {
        self.sw_cancel_defer("host");
    }

    pub(super) fn sw_cancel_defer(&mut self, why: &'static str) {
        if self.sw_pending_defer.take().is_some() {
            self.sw_last_reason = "defer_cancel";
            unim_log!(
                "ENGINE",
                "대상③: reason=defer_cancel client={} why={}",
                self.client_tag,
                why
            );
        }
    }

    /// 키·KEY 힌트·커밋 — 마지막 보고 뒤 문서가 키로 바뀌었다(R5: 이동 후보를 무효화).
    pub(super) fn sw_note_key_event(&mut self) {
        if let Some(r) = self.sw_last_report.as_mut() {
            r.keys_since = true;
        }
        self.sw_move_at = None;
    }

    fn sw_note_select_signal(&mut self) {
        if self.sw_push() {
            self.sw_last_select_ms = Some(self.sw_now_ms);
        }
    }

    /// R5 — push 컨텍스트에서 surrounding 보고를 받았다.
    pub(super) fn sw_note_report(&mut self, cursor: u32) {
        if !self.sw_push() {
            return;
        }
        if let Some(prev) = self.sw_last_report {
            if !prev.keys_since && prev.cursor != cursor {
                // 포커스 직후 SETTLE 안의 이동은 앱이 스스로 옮긴 것(select-on-focus·선택 복원 등)일 수
                // 있어 클릭 후보로 세지 않는다(기준점만 갱신) — 선택이 남은 채 한자로 풀리는 것을 막는다.
                let near_focus = self
                    .sw_focus_at
                    .is_some_and(|f| self.sw_now_ms.saturating_sub(f) < super::types::SURROUNDING_SETTLE_MS);
                if !near_focus {
                    self.sw_move_at = Some(self.sw_now_ms);
                }
            }
        }
        self.sw_last_report = Some(SwReport { cursor, keys_since: false });
    }

    /// 비밀번호 진입 등으로 push 상태 전체를 버린다.
    pub(super) fn sw_drop_push_state(&mut self) {
        self.sw_last_report = None;
        self.sw_move_at = None;
    }

    /// R6 — `now − at ≥ SETTLE` 이고 `[at−SETTLE, now]` 에 select 신호가 없는가.
    fn sw_settle_ok(&self, at: u64) -> bool {
        let now = self.sw_now_ms;
        if now.saturating_sub(at) < super::types::SURROUNDING_SETTLE_MS {
            return false;
        }
        match self.sw_last_select_ms {
            Some(sig) => sig.saturating_add(super::types::SURROUNDING_SETTLE_MS) < at,
            None => true,
        }
    }

    /// R9 — 보류했던 한자키의 재판정 시도. `None` 이면 **계속 대기**(`deferred_hanja_deadline()` 이
    /// 갱신됐을 수 있다), `Some(r)` 이면 판정이 끝났다(`resume_deferred_hanja` 와 같은 결과 규약).
    ///
    /// - 보류가 없으면 `Some(not_consumed)`(무동작).
    /// - `Deadline`: 기한이 됐으므로 곧장 재판정한다(두 번째 Defer 없음).
    /// - `Report`: 기한 전이라도 새 보고가 낡음을 풀었다. 그런데 R8(안정 대기)이 아직 남아 있으면
    ///   일찍 재판정하지 않고(그러면 `selpossible` 이모지가 된다) 안정 시각까지 기한을 늦춰 한 번 더
    ///   기다린다. 단 **한자키 시점부터 총 대기가 `SURROUNDING_DEFER_TOTAL_MAX_MS`(SETTLE+WAIT=450ms)를
    ///   넘지 않게** 기한을 자른다. 기한이 이미 지났으면 곧장 재판정한다.
    pub fn poll_deferred_hanja(&mut self, now_ms: u64, trigger: DeferTrigger) -> Option<InputResult> {
        let Some(deadline) = self.sw_pending_defer else {
            return Some(InputResult::not_consumed());
        };
        let now = match trigger {
            DeferTrigger::Deadline => now_ms.max(deadline),
            DeferTrigger::Report => now_ms,
        };
        self.sw_now_ms = now;
        if trigger == DeferTrigger::Report && now < deadline {
            if let Some(next) = self.sw_peek_defer_deadline() {
                let cap = self
                    .sw_defer_start
                    .saturating_add(super::types::SURROUNDING_DEFER_TOTAL_MAX_MS);
                let next = next.min(cap);
                if next > now {
                    if next != deadline {
                        unim_log!(
                            "ENGINE",
                            "대상③: reason=defer_extend client={} wait_ms={} total_ms={}",
                            self.client_tag,
                            next - now,
                            next.saturating_sub(self.sw_defer_start)
                        );
                    }
                    self.sw_pending_defer = Some(next);
                    return None;
                }
            }
        }
        Some(self.resume_deferred_hanja(now))
    }

    /// 지금 상태로 판정하면 Defer 가 날지 미리 본다(상태 불변). 앱 선택(대상②)·소진은 Defer 가 없다.
    fn sw_peek_defer_deadline(&self) -> Option<u64> {
        if self.has_selection() || self.surrounding_word_exhausted {
            return None;
        }
        match self.surrounding_word_judge(true) {
            SwJudge::Defer { deadline_ms, .. } => Some(deadline_ms),
            _ => None,
        }
    }

    /// R9 — 보류했던 한자키를 다시 판정한다(Defer 는 한 번만). 팝업/이모지는 `take_popup_action`
    /// 으로 평소처럼 드레인한다.
    ///
    /// 반환: 보류가 없으면 `not_consumed()`(무동작). 한자 팝업이 열리면 `hanja_candidates()`,
    /// 거절이면 지금의 이모지 폴백을 열고 `consumed()`.
    pub fn resume_deferred_hanja(&mut self, now_ms: u64) -> InputResult {
        if self.sw_pending_defer.take().is_none() {
            return InputResult::not_consumed();
        }
        self.sw_now_ms = now_ms;
        self.sw_no_defer = true;
        let r = self.start_hanja_conversion();
        self.sw_no_defer = false;
        if self.hanja_mode {
            return r;
        }
        self.start_emoji_popup();
        InputResult::consumed()
    }

    /// 엔진 재생성(Reset·FocusOut)을 건너 옮길 신선도·거부 상태.
    pub fn surrounding_word_carry(&self) -> SurroundingWordCarry {
        SurroundingWordCarry {
            caps: self.surrounding_caps,
            client_tag: self.client_tag,
            stale: self.surrounding_stale,
            selection_possible: self.selection_possible,
            flag_cause: self.selection_flag_cause.as_u8(),
            exhausted: self.surrounding_word_exhausted,
            move_at: self.sw_move_at,
            last_select_ms: self.sw_last_select_ms,
            now_ms: self.sw_now_ms,
        }
    }

    /// `surrounding_word_carry` 로 받아 둔 상태를 되돌린다.
    pub fn restore_surrounding_word_carry(&mut self, c: SurroundingWordCarry) {
        self.surrounding_caps = c.caps;
        if !c.client_tag.is_empty() {
            self.client_tag = c.client_tag;
        }
        self.surrounding_stale = c.stale;
        self.selection_possible = c.selection_possible;
        self.selection_flag_cause = SelFlagCause::from_u8(c.flag_cause);
        self.surrounding_word_exhausted = c.exhausted;
        // 기준점(`sw_last_report`)·보류(`sw_pending_defer`)는 이월하지 않는다 — Reset 은 둘 다 비운다.
        self.sw_move_at = c.move_at;
        self.sw_last_select_ms = c.last_select_ms;
        self.sw_now_ms = c.now_ms;
    }

    /// 문서가 바뀌었음을 알린다(낡음만 — 거부 플래그는 그대로).
    pub fn mark_surrounding_stale(&mut self) {
        self.surrounding_stale = true;
        self.sw_note_key_event();
    }

    /// 확정이 앱 문서에 텍스트를 넣었음을 알린다(낡음 + 거부 플래그 해제 — 선택은 대체됨).
    pub fn note_surrounding_commit(&mut self) {
        self.surrounding_stale = true;
        self.sw_note_key_event();
        self.clear_sel_flag();
    }

    /// 마지막 surrounding 보고 뒤 낡았는가(진단·테스트용).
    pub fn surrounding_stale(&self) -> bool {
        self.surrounding_stale
    }

    /// 앱에 선택이 있을 수 있다고 표시돼 있는가(진단·테스트용).
    pub fn selection_possible(&self) -> bool {
        self.selection_possible
    }

    /// 현재 대상③ 팝업의 `(앞 a자, 뒤 b자)`. 대상③ 팝업이 아니면 `None`.
    pub fn surrounding_word_span(&self) -> Option<(u32, u32)> {
        (self.hanja_mode && self.hanja_source == super::types::HanjaSource::SurroundingWord)
            .then_some((self.hanja_sw_before, self.hanja_sw_after))
    }

    // =========================================
    // press_key 후처리
    // =========================================

    /// `press_key` 래퍼의 후처리 — 이번 키가 문서·선택 상태에 준 영향을 반영한다(§2.4b.4).
    ///
    /// 순서: 커밋 규칙 → 통과 규칙 → 키 분류. 한자키가 아닌 키는 "선택 불명" 표시를 내린다.
    pub(super) fn surrounding_track_after_key(
        &mut self,
        keycode: KeyCode,
        modifier: ModifierState,
        r: &super::types::InputResult,
        before_len: usize,
        was_popup: bool,
    ) {
        if self.commit_buffer.len() > before_len || self.pending_hanja_replacement.is_some() {
            self.note_surrounding_commit();
        }
        if !r.consumed && !keycode.is_modifier() {
            self.surrounding_stale = true;
        }
        let is_hanja = self.hanja_keys.contains(&keycode);
        if (!was_popup || !r.consumed) && !is_hanja {
            let effect = classify_sel_key(keycode, modifier);
            self.apply_sel_effect(effect);
        }
        if !is_hanja {
            self.selection_unknown_once = false;
            if !keycode.is_modifier() {
                self.sw_note_key_event();
            }
        }
    }

    // =========================================
    // 대상 결정
    // =========================================

    /// 대상③ 후보를 만든다(Defer 없음 — 시험·진단용). 거절이면 `(이유 코드, 한글 구간 길이)`.
    #[cfg(test)]
    pub(super) fn surrounding_word_resolve(&self) -> Result<Vec<SwCand>, (&'static str, u32)> {
        match self.surrounding_word_judge(false) {
            SwJudge::Ok { cands, .. } => Ok(cands),
            SwJudge::Reject { reason, run, .. } => Err((reason, run)),
            SwJudge::Defer { reason, .. } => Err((reason, 0)),
        }
    }

    /// 대상③ 판정.
    ///
    /// 검사 순서: sensitive → nocap → nosur → selection → stale(→R7) → selpossible(R6 먼저,
    /// →R8) → cursor0 → norun → nomatch. 후보 정렬은 길이 내림차순 → 등급(커서에서 끝:0,
    /// 커서에서 시작:1, 가로지름:2) → 시작 위치 오름차순.
    ///
    /// `allow_defer` 이고 push 컨텍스트일 때만 R7·R8 이 `Defer` 를 낸다. S 비트가 있는
    /// 컨텍스트는 R6~R8 을 전혀 타지 않아 종전과 같다.
    pub(super) fn surrounding_word_judge(&self, allow_defer: bool) -> SwJudge {
        let reject = |reason: &'static str, run: u32| SwJudge::Reject { reason, run, settled: false };
        if self.content_purpose.should_block_hangul() {
            return reject("sensitive", 0);
        }
        let caps = self.surrounding_caps;
        if caps & SURROUNDING_CAP_REPLACE == 0
            || caps & (SURROUNDING_CAP_SELECTION_RELIABLE | SURROUNDING_CAP_POINTER_HINTS) == 0
        {
            return reject("nocap", 0);
        }
        let push = self.sw_push();
        let chars: Vec<char> = self.surrounding_text.chars().collect();
        let n = chars.len();
        let cursor = self.surrounding_cursor as usize;
        let anchor = self.surrounding_anchor as usize;
        if n == 0 || cursor > n || anchor > n {
            return reject("nosur", 0);
        }
        if cursor != anchor {
            return reject("selection", 0);
        }
        if self.surrounding_stale {
            if push && allow_defer {
                // R7
                return SwJudge::Defer {
                    deadline_ms: self.sw_now_ms.saturating_add(SURROUNDING_DEFER_WAIT_MS),
                    reason: "stale",
                };
            }
            return reject("stale", 0);
        }
        let mut settled = false;
        let mut flag_on = caps & SURROUNDING_CAP_SELECTION_RELIABLE == 0 && self.selection_possible;
        if push
            && flag_on
            && !self.selection_unknown_once
            && matches!(
                self.selection_flag_cause,
                SelFlagCause::FocusIn | SelFlagCause::FieldFocus
            )
        {
            if let Some(at) = self.sw_move_at {
                if self.sw_settle_ok(at) {
                    // R6 — 포커스 원인 플래그만 푼다. 실제 선택 신호로 선 플래그는 원인이 달라 풀리지 않는다.
                    flag_on = false;
                    settled = true;
                } else if allow_defer
                    && self.sw_now_ms.saturating_sub(at) < super::types::SURROUNDING_SETTLE_MS
                {
                    // R8 — 안정될 때까지(`at + SETTLE`) 미룬다.
                    return SwJudge::Defer {
                        deadline_ms: at.saturating_add(super::types::SURROUNDING_SETTLE_MS),
                        reason: "settle",
                    };
                }
            }
        }
        if self.selection_unknown_once || flag_on {
            return reject("selpossible", 0);
        }
        let c = cursor;
        if c == 0 {
            return SwJudge::Reject { reason: "cursor0", run: 0, settled };
        }
        let mut s = c;
        while s > 0 && chars[s - 1].is_hangul_syllable() {
            s -= 1;
        }
        let mut e = c;
        while e < n && chars[e].is_hangul_syllable() {
            e += 1;
        }
        let run = (e - s) as u32;
        if s == e {
            return SwJudge::Reject { reason: "norun", run: 0, settled };
        }
        // (길이, 등급, 시작, 후보)
        let mut found: Vec<(usize, u8, usize, SwCand)> = Vec::new();
        for i in s.max(c.saturating_sub(HANJA_MAX_KEY_CHARS))..=c {
            let jmin = c.max(i + 1);
            let jmax = e.min(i + HANJA_MAX_KEY_CHARS);
            for j in jmin..=jmax {
                let key: String = chars[i..j].iter().collect();
                if !self.hanja_dict.contains(&key) {
                    continue;
                }
                let grade = if j == c {
                    0
                } else if i == c {
                    1
                } else {
                    2
                };
                found.push((
                    j - i,
                    grade,
                    i,
                    SwCand { key, a: (c - i) as u32, b: (j - c) as u32 },
                ));
            }
        }
        if found.is_empty() {
            return SwJudge::Reject { reason: "nomatch", run, settled };
        }
        found.sort_by(|x, y| y.0.cmp(&x.0).then(x.1.cmp(&y.1)).then(x.2.cmp(&y.2)));
        SwJudge::Ok { cands: found.into_iter().map(|t| t.3).collect(), settled }
    }
}
