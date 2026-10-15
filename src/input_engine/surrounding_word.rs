//! 한자 단어 대상③ — 커서 주변 단어 (HANJA_WORD_SPEC v1.6 §2.4b·§2.8).
//!
//! 프런트가 보고한 surrounding(커서 앞뒤 문서)에서 커서가 걸친 한글 구간의 사전 일치
//! 후보를 만들고, 한자키 재타로 순환한다. 문서를 건드리는 사건(확정·통과 키·포인터)이
//! 마지막 보고 뒤에 있었는지(낡음)와 앱에 선택이 있을 수 있는지(거부 플래그)를 이
//! 모듈이 추적한다. 로그에는 사용자 텍스트 평문을 남기지 않는다(길이·수치·이유 코드만).

use super::engine::InputEngine;
use super::types::{
    SurroundingWordCarry, HANJA_MAX_KEY_CHARS, SURROUNDING_CAP_POINTER_HINTS,
    SURROUNDING_CAP_REPLACE, SURROUNDING_CAP_SELECTION_RELIABLE, SURROUNDING_HINT_KEY,
    SURROUNDING_HINT_POINTER_PLAIN, SURROUNDING_HINT_POINTER_SELECT,
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

    fn set_sel_flag(&mut self, cause: SelFlagCause) {
        self.selection_possible = true;
        self.selection_flag_cause = cause;
    }

    fn clear_sel_flag(&mut self) {
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
                unim_log!("ENGINE", "SurroundingHint: kind={} (pointer_plain)", kind);
            }
            SURROUNDING_HINT_POINTER_SELECT => {
                self.surrounding_stale = true;
                self.set_sel_flag(SelFlagCause::PointerSelect);
                unim_log!("ENGINE", "SurroundingHint: kind={} (pointer_select)", kind);
            }
            SURROUNDING_HINT_PRIMARY_CHANGED => {
                self.set_sel_flag(SelFlagCause::PrimaryChanged);
                unim_log!("ENGINE", "SurroundingHint: kind={} (primary_changed)", kind);
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
        self.surrounding_stale = true;
        self.set_sel_flag(SelFlagCause::FocusIn);
        self.surrounding_word_exhausted = false;
        self.selection_unknown_once = false;
    }

    /// 엔진 재생성(Reset·FocusOut)을 건너 옮길 신선도·거부 상태.
    pub fn surrounding_word_carry(&self) -> SurroundingWordCarry {
        SurroundingWordCarry {
            caps: self.surrounding_caps,
            stale: self.surrounding_stale,
            selection_possible: self.selection_possible,
            flag_cause: self.selection_flag_cause.as_u8(),
            exhausted: self.surrounding_word_exhausted,
        }
    }

    /// `surrounding_word_carry` 로 받아 둔 상태를 되돌린다.
    pub fn restore_surrounding_word_carry(&mut self, c: SurroundingWordCarry) {
        self.surrounding_caps = c.caps;
        self.surrounding_stale = c.stale;
        self.selection_possible = c.selection_possible;
        self.selection_flag_cause = SelFlagCause::from_u8(c.flag_cause);
        self.surrounding_word_exhausted = c.exhausted;
    }

    /// 문서가 바뀌었음을 알린다(낡음만 — 거부 플래그는 그대로).
    pub fn mark_surrounding_stale(&mut self) {
        self.surrounding_stale = true;
    }

    /// 확정이 앱 문서에 텍스트를 넣었음을 알린다(낡음 + 거부 플래그 해제 — 선택은 대체됨).
    pub fn note_surrounding_commit(&mut self) {
        self.surrounding_stale = true;
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
        }
    }

    // =========================================
    // 대상 결정
    // =========================================

    /// 대상③ 후보를 만든다. 거절이면 `(이유 코드, 한글 구간 길이)`.
    ///
    /// 검사 순서: sensitive → nocap → nosur → selection → stale → selpossible → cursor0 →
    /// norun → nomatch. 후보 정렬은 길이 내림차순 → 등급(커서에서 끝:0, 커서에서 시작:1,
    /// 가로지름:2) → 시작 위치 오름차순.
    pub(super) fn surrounding_word_resolve(&self) -> Result<Vec<SwCand>, (&'static str, u32)> {
        if self.content_purpose.should_block_hangul() {
            return Err(("sensitive", 0));
        }
        let caps = self.surrounding_caps;
        if caps & SURROUNDING_CAP_REPLACE == 0
            || caps & (SURROUNDING_CAP_SELECTION_RELIABLE | SURROUNDING_CAP_POINTER_HINTS) == 0
        {
            return Err(("nocap", 0));
        }
        let chars: Vec<char> = self.surrounding_text.chars().collect();
        let n = chars.len();
        let cursor = self.surrounding_cursor as usize;
        let anchor = self.surrounding_anchor as usize;
        if n == 0 || cursor > n || anchor > n {
            return Err(("nosur", 0));
        }
        if cursor != anchor {
            return Err(("selection", 0));
        }
        if self.surrounding_stale {
            return Err(("stale", 0));
        }
        if self.selection_unknown_once
            || (caps & SURROUNDING_CAP_SELECTION_RELIABLE == 0 && self.selection_possible)
        {
            return Err(("selpossible", 0));
        }
        let c = cursor;
        if c == 0 {
            return Err(("cursor0", 0));
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
            return Err(("norun", 0));
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
            return Err(("nomatch", run));
        }
        found.sort_by(|x, y| y.0.cmp(&x.0).then(x.1.cmp(&y.1)).then(x.2.cmp(&y.2)));
        Ok(found.into_iter().map(|t| t.3).collect())
    }
}
