//! 대상③ 커서 주변 단어(HANJA_WORD_SPEC v1.6 §2.4b·§2.8) 엔진 테스트 — 실제 사전.
//!
//! 기본 헬퍼는 능력 `REPLACE | SELECTION_RELIABLE`(0x3)과 신선한 surrounding 으로 시작한다.
//! 기대 순서는 리터럴이다(사전·정렬 규칙 결함이 동어반복으로 가려지지 않게).

use super::surrounding_word::{classify_sel_keysym, hint_key_log_name, SelFlagCause, SelKeyEffect};
use super::test_helpers::create_test_engine;
use super::{
    HanjaSource, InputEngine, InputResult, PopupAction, SURROUNDING_CAP_POINTER_HINTS,
    SURROUNDING_CAP_REPLACE, SURROUNDING_CAP_SELECTION_RELIABLE, SURROUNDING_HINT_KEY,
    SURROUNDING_HINT_POINTER_PLAIN, SURROUNDING_HINT_POINTER_SELECT,
    SURROUNDING_HINT_PRIMARY_CHANGED, SURROUNDING_HINT_SELECTION_UNKNOWN,
};
use crate::config::{Config, ContentPurpose, HanjaOutputFormat, InputCategory};
use crate::keycode::{KeyCode, ModifierState};

const CAPS_RS: u32 = SURROUNDING_CAP_REPLACE | SURROUNDING_CAP_SELECTION_RELIABLE;
const CAPS_RP: u32 = SURROUNDING_CAP_REPLACE | SURROUNDING_CAP_POINTER_HINTS;

fn m() -> ModifierState {
    ModifierState::default()
}

fn shift() -> ModifierState {
    ModifierState { shift: true, ..Default::default() }
}

fn ctrl() -> ModifierState {
    ModifierState { control: true, ..Default::default() }
}

fn eng_caps(caps: u32, text: &str, cursor: u32) -> InputEngine {
    let mut e = create_test_engine();
    e.set_surrounding_word_caps(caps);
    e.set_surrounding_text(text.to_string(), cursor, cursor);
    e
}

fn eng(text: &str, cursor: u32) -> InputEngine {
    eng_caps(CAPS_RS, text, cursor)
}

fn key(e: &mut InputEngine, k: KeyCode, mods: ModifierState) -> InputResult {
    e.press_key(k, mods, &Config::default())
}

fn f9(e: &mut InputEngine) -> InputResult {
    key(e, KeyCode::F9, m())
}

/// F9 를 반복해 대상③ 후보의 `(a, b)` 열을 모은다(소진되면 멈춘다).
fn seq_caps(caps: u32, text: &str, cursor: u32) -> Vec<(u32, u32)> {
    let mut e = eng_caps(caps, text, cursor);
    f9(&mut e);
    let mut out = vec![];
    for _ in 0..24 {
        match e.surrounding_word_span() {
            Some(s) => out.push(s),
            None => break,
        }
        f9(&mut e);
    }
    out
}

fn seq(text: &str, cursor: u32) -> Vec<(u32, u32)> {
    seq_caps(CAPS_RS, text, cursor)
}

fn is_sw(e: &InputEngine) -> bool {
    e.is_hanja_mode() && e.hanja_source() == HanjaSource::SurroundingWord
}

// =============================================================================
// 후보
// =============================================================================

#[test]
fn sw_order_end() {
    assert_eq!(seq("국어교육", 4), vec![(4, 0), (2, 0), (1, 0)]);
}

#[test]
fn sw_order_mid() {
    assert_eq!(
        seq("국어교육", 2),
        vec![(2, 2), (2, 0), (0, 2), (1, 1), (1, 0), (0, 1)]
    );
}

/// 규칙(§2.4b.2·3) 그대로의 순서: 어교(0/2, 커서에서 시작)가 국어(1/1, 가로지름)보다 앞선다.
#[test]
fn sw_order_after_first_rule_order() {
    assert_eq!(
        seq("국어교육", 1),
        vec![(1, 3), (0, 2), (1, 1), (1, 0), (0, 1)]
    );
}

#[test]
fn sw_order_daehan() {
    assert_eq!(
        seq("대한민국", 2),
        vec![(2, 2), (2, 0), (0, 2), (1, 1), (1, 0), (0, 1)]
    );
}

#[test]
fn sw_order_ascii_prefix() {
    assert_eq!(seq("abc국어", 3), vec![(0, 2), (0, 1)]);
}

#[test]
fn sw_cursor0() {
    let mut e = eng("국어교육", 0);
    assert_eq!(e.surrounding_word_resolve().unwrap_err().0, "cursor0");
    f9(&mut e);
    assert!(!e.is_hanja_mode());
    assert!(e.is_emoji_popup_active());
}

#[test]
fn sw_trailing_space() {
    let e = eng("국어교육 ", 5);
    assert_eq!(e.surrounding_word_resolve().unwrap_err().0, "norun");
}

/// 구간은 공백·자모·한자·이모지·개행을 넘지 않는다.
#[test]
fn sw_run_boundaries() {
    for sep in [" ", "ㄱ", "國", "😀", "\n"] {
        // 구분자 앞(커서 2): 오른쪽이 구분자 → 모든 후보의 b == 0.
        let text = format!("국어{sep}교육");
        let left = seq(&text, 2);
        assert!(!left.is_empty(), "sep={sep:?}");
        assert!(left.iter().all(|&(a, b)| b == 0 && a <= 2), "sep={sep:?}: {left:?}");
        // 구분자 뒤(커서 3): 왼쪽이 구분자 → 모든 후보의 a == 0.
        let right = seq(&text, 3);
        assert!(
            right.iter().all(|&(a, b)| a == 0 && b <= 2),
            "sep={sep:?}: {right:?}"
        );
    }
}

#[test]
fn sw_cap_18() {
    let text = "국".repeat(20);
    let e = eng(&text, 10);
    let cands = e.surrounding_word_resolve().unwrap();
    assert!(!cands.is_empty());
    assert!(cands.iter().all(|c| c.key.chars().count() <= 18));
}

#[test]
fn sw_surrogate_offset() {
    // "😀" 는 1문자(UTF-16 2단위) — 오프셋은 문자 기준이다.
    let mut e = eng("😀국어", 3);
    f9(&mut e);
    assert!(is_sw(&e));
    assert_eq!(e.get_hanja_target(), "국어");
    assert_eq!(e.surrounding_word_span(), Some((2, 0)));
}

#[test]
fn sw_nomatch() {
    let mut e = eng("뷁", 1);
    assert_eq!(e.surrounding_word_resolve().unwrap_err().0, "nomatch");
    f9(&mut e);
    assert!(!e.is_hanja_mode());
    assert!(e.is_emoji_popup_active());
}

// =============================================================================
// 재타·소진
// =============================================================================

#[test]
fn sw_repress_next_updates_bookmark_key() {
    let mut e = eng("국어교육", 4);
    f9(&mut e);
    assert_eq!(e.get_hanja_target(), "국어교육");
    let r = f9(&mut e);
    assert_eq!(r, InputResult::hanja_candidates());
    assert_eq!(e.get_hanja_target(), "교육");
    assert_eq!(e.surrounding_word_span(), Some((2, 0)));
    assert!(matches!(
        e.take_popup_action(),
        Some(PopupAction::ShowHanja { .. })
    ));
}

#[test]
fn sw_exhaust_push_opens_emoji_no_commit() {
    let mut e = eng("국어교육", 4);
    for _ in 0..3 {
        f9(&mut e);
        assert!(is_sw(&e));
    }
    let r = f9(&mut e);
    assert_eq!(r, InputResult::consumed());
    assert!(!e.is_hanja_mode());
    assert!(e.is_emoji_popup_active());
    assert_eq!(e.commit_str(), "", "재커밋 없음");
    assert!(e.take_hanja_replacement().is_none());
    assert!(matches!(
        e.take_popup_action(),
        Some(PopupAction::ShowEmoji { .. })
    ));
}

#[test]
fn sw_exhaust_pull_then_processkey_emoji() {
    let mut e = eng("국어교육", 4);
    assert_eq!(e.start_hanja_conversion(), InputResult::hanja_candidates());
    assert_eq!(e.start_hanja_conversion(), InputResult::hanja_candidates());
    assert_eq!(e.start_hanja_conversion(), InputResult::hanja_candidates());
    // 소진 — 팝업이 닫히고 후보 없음.
    assert_eq!(e.start_hanja_conversion(), InputResult::consumed());
    assert!(!e.is_hanja_mode());
    assert!(!e.is_emoji_popup_active());
    // 같은 키의 나머지 pull 호출은 대상③ 을 다시 열지 않는다.
    assert_eq!(e.start_hanja_conversion(), InputResult::consumed());
    assert!(!e.is_hanja_mode());
    // 폴백 ProcessKey → 이모지.
    f9(&mut e);
    assert!(e.is_emoji_popup_active());
}

#[test]
fn sw_exhausted_cleared_by_next_key_and_surrounding() {
    let exhaust = |e: &mut InputEngine| {
        for _ in 0..4 {
            e.start_hanja_conversion();
        }
        assert!(e.surrounding_word_exhausted);
    };
    let mut e = eng("국어교육", 4);
    exhaust(&mut e);
    key(&mut e, KeyCode::Right, m());
    assert!(!e.surrounding_word_exhausted, "다음 키 처리에서 내려간다");

    let mut e = eng("국어교육", 4);
    exhaust(&mut e);
    e.set_surrounding_text("국어교육".to_string(), 4, 4);
    assert!(!e.surrounding_word_exhausted, "새 surrounding 보고로 내려간다");
    e.start_hanja_conversion();
    assert!(is_sw(&e), "다시 대상③ 을 연다");
}

/// 1음절 후보("국")도 일반 한자 후보 목록을 낸다.
#[test]
fn sw_single_syllable_restores_expand() {
    let mut e = eng("국어교육", 4);
    for _ in 0..3 {
        f9(&mut e);
    }
    assert_eq!(e.get_hanja_target(), "육");
    assert!(e.get_hanja_candidates().len() > 1);
}

// =============================================================================
// 확정·취소
// =============================================================================

#[test]
fn sw_confirm_keyboard_payload() {
    let mut e = eng("국어교육", 4);
    f9(&mut e);
    let r = key(&mut e, KeyCode::Num1, m());
    assert_eq!(r, InputResult::consumed(), "I5: consumed");
    let rep = e.take_hanja_replacement().expect("대상③ 페이로드");
    assert_eq!(rep.delete_chars, 4);
    assert_eq!(rep.delete_after_chars, 0);
    assert_eq!(rep.preedit_chars, 0);
    assert_eq!(rep.text, "國語敎育");
    assert_eq!(rep.source, HanjaSource::SurroundingWord);
    assert_eq!(rep.expect_text, "국어교육");
    assert_eq!(e.commit_str(), "", "commit_buffer 비어 있음");
    assert!(!e.is_hanja_mode());
    assert!(matches!(e.take_popup_action(), Some(PopupAction::HidePopup)));
}

#[test]
fn sw_confirm_keyboard_mid_cursor_payload() {
    let mut e = eng("국어교육", 2);
    f9(&mut e);
    f9(&mut e); // 대한 계열 말고 두 번째 후보 (2,0) = 국어
    assert_eq!(e.surrounding_word_span(), Some((2, 0)));
    key(&mut e, KeyCode::Num1, m());
    let rep = e.take_hanja_replacement().unwrap();
    assert_eq!((rep.delete_chars, rep.delete_after_chars), (2, 0));
    assert_eq!(rep.expect_text, "국어");
}

#[test]
fn sw_confirm_select_hanja_payload() {
    let mut e = eng("국어교육", 2);
    f9(&mut e);
    assert_eq!(e.surrounding_word_span(), Some((2, 2)));
    let text = e.select_hanja(0).expect("선택");
    assert_eq!(text, "國語敎育");
    let rep = e.take_hanja_replacement().unwrap();
    assert_eq!((rep.delete_chars, rep.delete_after_chars), (2, 2));
    assert_eq!(rep.source, HanjaSource::SurroundingWord);
    assert_eq!(rep.expect_text, "국어교육");
    assert_eq!(rep.text, text);
}

/// 마우스 확정(`select_hanja` = DBus SelectHanja 경로)도 키보드 확정과 같이 최근 확정 음절
/// 버퍼를 비운다(§2.4b.5) — 비우지 않으면 다음 한자키가 낡은 접두로 대상①을 시작한다.
#[test]
fn sw_mouse_confirm_clears_recent_syllables() {
    let mut e = eng("국어교육", 2);
    f9(&mut e);
    assert!(is_sw(&e));
    // 확정 직전에 버퍼가 차 있었다고 가정(전제: 비어 있지 않음).
    for c in "국어".chars() {
        e.recent_push_char(c);
    }
    assert!(!e.recent_syllables().is_empty(), "전제: 버퍼가 채워져 있어야 한다");
    let text = e.select_hanja(0).expect("선택");
    assert_eq!(text, "國語敎育");
    assert_eq!(e.recent_syllables(), "", "마우스 확정 후 버퍼 clear");
    assert!(e.surrounding_stale(), "확정은 문서를 바꾸므로 낡음");
    assert!(!e.is_hanja_mode(), "팝업 종료");
}

#[test]
fn sw_confirm_formats_3() {
    let cases = [
        (HanjaOutputFormat::Hanja, "國語敎育"),
        (HanjaOutputFormat::HangulHanja, "국어교육(國語敎育)"),
        (HanjaOutputFormat::HanjaHangul, "國語敎育(국어교육)"),
    ];
    for (format, expected) in cases {
        let mut config = Config::default();
        config.engine.korean.hanja_output_format = format;
        let mut e = InputEngine::new(&config);
        e.set_surrounding_word_caps(CAPS_RS);
        e.set_surrounding_text("국어교육".to_string(), 4, 4);
        e.press_key(KeyCode::F9, m(), &config);
        e.press_key(KeyCode::Num1, m(), &config);
        let rep = e.take_hanja_replacement().unwrap();
        assert_eq!(rep.text, expected, "{format:?}");
        assert_eq!(rep.expect_text, "국어교육", "expect 는 항상 한글 원문");
    }
}

#[test]
fn sw_confirm_sets_stale_clears_flag_and_recent() {
    let mut e = eng("국어교육", 4);
    e.surrounding_hint(SURROUNDING_HINT_PRIMARY_CHANGED, 0, 0);
    assert!(e.selection_possible());
    assert!(!e.surrounding_stale());
    f9(&mut e);
    assert!(is_sw(&e), "SELECTION_RELIABLE 은 플래그를 무시");
    key(&mut e, KeyCode::Num1, m());
    assert!(e.surrounding_stale());
    assert!(!e.selection_possible());
    assert_eq!(e.recent_syllables(), "");
}

#[test]
fn sw_escape_no_commit() {
    let mut e = eng("국어교육", 4);
    f9(&mut e);
    key(&mut e, KeyCode::Escape, m());
    assert!(!e.is_hanja_mode());
    assert_eq!(e.commit_str(), "");
    assert!(e.take_hanja_replacement().is_none());
    assert!(!e.surrounding_stale(), "문서 불변 — 낡지 않는다");
}

#[test]
fn sw_cancel_hanja_no_commit() {
    let mut e = eng("국어교육", 4);
    f9(&mut e);
    assert_eq!(e.hanja_cancel_text(), "");
    e.cancel_hanja();
    assert!(!e.is_hanja_mode());
    assert_eq!(e.surrounding_word_span(), None);
    assert_eq!(e.commit_str(), "");
}

#[test]
fn sw_ctrl_hanja_in_popup_reprocesses() {
    let mut e = eng("국어교육", 4);
    f9(&mut e);
    key(&mut e, KeyCode::F9, ctrl());
    // Ctrl+한자키는 순환이 아니다(미지원 키 재처리) — 두 번째 후보로 가지 않는다.
    assert_ne!(e.get_hanja_target(), "교육");
    assert_eq!(e.commit_str(), "");
}

// =============================================================================
// 우선순위
// =============================================================================

#[test]
fn sw_composing_uses_target1() {
    let config = Config::default();
    let mut e = create_test_engine();
    e.set_input_category(InputCategory::Korean);
    e.set_surrounding_word_caps(CAPS_RS);
    // "국" 조합 중
    for k in [KeyCode::R, KeyCode::N, KeyCode::R] {
        e.press_key(k, m(), &config);
    }
    e.set_surrounding_text("국어교육".to_string(), 4, 4);
    e.press_key(KeyCode::F9, m(), &config);
    assert!(e.is_hanja_mode());
    assert_ne!(e.hanja_source(), HanjaSource::SurroundingWord);
}

#[test]
fn sw_fresh_matching_selection_target2() {
    let mut e = eng("대한민국", 0);
    e.set_surrounding_text("대한민국".to_string(), 4, 0);
    f9(&mut e);
    assert!(e.is_hanja_mode());
    assert_eq!(e.hanja_source(), HanjaSource::Selection);
}

#[test]
fn sw_fresh_nomatch_selection_emoji_not_target3() {
    let mut e = eng("국어뷁", 0);
    e.set_surrounding_text("국어뷁".to_string(), 3, 2);
    f9(&mut e);
    assert!(!e.is_hanja_mode());
    assert!(e.is_emoji_popup_active());
}

#[test]
fn sw_stale_selection_emoji() {
    let mut e = eng("대한민국", 0);
    e.set_surrounding_text("대한민국".to_string(), 4, 0);
    e.surrounding_hint(SURROUNDING_HINT_POINTER_PLAIN, 0, 0);
    assert!(e.surrounding_stale());
    f9(&mut e);
    assert!(!e.is_hanja_mode(), "낡은 선택 스냅샷은 대상② 로 쓰지 않는다");
    assert!(e.is_emoji_popup_active());
}

// =============================================================================
// 신선도
// =============================================================================

#[test]
fn sw_stale_after_commit() {
    let mut e = eng("국어교육", 4);
    e.set_input_category(InputCategory::English);
    key(&mut e, KeyCode::A, m());
    assert_eq!(e.commit_str(), "a");
    assert!(e.surrounding_stale());
    assert_eq!(e.surrounding_word_resolve().unwrap_err().0, "stale");
    f9(&mut e);
    assert!(e.is_emoji_popup_active());
}

#[test]
fn sw_stale_after_passthrough_key() {
    let mut e = eng("국어교육", 4);
    let r = key(&mut e, KeyCode::Right, m());
    assert!(!r.consumed);
    assert!(e.surrounding_stale());
    f9(&mut e);
    assert!(!e.is_hanja_mode());
}

#[test]
fn sw_stale_after_hint_key_and_pointer() {
    let mut e = eng("국어교육", 4);
    e.surrounding_hint(SURROUNDING_HINT_KEY, 0xff51, 0);
    assert!(e.surrounding_stale());
    e.set_surrounding_text("국어교육".to_string(), 4, 4);
    assert!(!e.surrounding_stale());
    e.surrounding_hint(SURROUNDING_HINT_POINTER_PLAIN, 0, 0);
    assert!(e.surrounding_stale());
    e.set_surrounding_text("국어교육".to_string(), 4, 4);
    e.surrounding_hint(SURROUNDING_HINT_POINTER_SELECT, 0, 0);
    assert!(e.surrounding_stale());
}

#[test]
fn sw_primary_changed_not_stale() {
    let mut e = eng("국어교육", 4);
    e.surrounding_hint(SURROUNDING_HINT_PRIMARY_CHANGED, 0, 0);
    assert!(!e.surrounding_stale());
    assert!(e.selection_possible());
}

#[test]
fn sw_modifier_only_not_stale() {
    let mut e = eng("국어교육", 4);
    key(&mut e, KeyCode::LeftShift, shift());
    assert!(!e.surrounding_stale());
    e.surrounding_hint(SURROUNDING_HINT_KEY, 0xffe1, 1); // Shift_L
    assert!(!e.surrounding_stale());
    e.surrounding_hint(SURROUNDING_HINT_KEY, 0xffe3, 4); // Control_L
    assert!(!e.surrounding_stale());
}

#[test]
fn sw_fresh_after_set_surrounding() {
    let mut e = eng("국어교육", 4);
    key(&mut e, KeyCode::Right, m());
    assert!(e.surrounding_stale());
    e.set_surrounding_text("국어교육".to_string(), 4, 4);
    assert!(!e.surrounding_stale());
    f9(&mut e);
    assert!(is_sw(&e));
}

/// 비밀번호 필드에서도 수신 사실로 신선해진다(차단 여부 무관, 값은 비워진다).
#[test]
fn sw_set_surrounding_fresh_even_when_blocked() {
    let mut e = eng("국어교육", 4);
    e.set_content_purpose(ContentPurpose::Password);
    e.mark_surrounding_stale();
    e.set_surrounding_text("secret".to_string(), 6, 6);
    assert!(!e.surrounding_stale());
    assert_eq!(e.surrounding_text().0, "");
}

#[test]
fn sw_carry_over_recreation() {
    let mut a = eng("국어교육", 4);
    a.surrounding_hint(SURROUNDING_HINT_POINTER_SELECT, 0, 0);
    a.surrounding_hint(SURROUNDING_HINT_SELECTION_UNKNOWN, 0, 0);
    let carry = a.surrounding_word_carry();
    let mut b = create_test_engine();
    assert_eq!(b.surrounding_word_caps(), 0);
    b.restore_surrounding_word_carry(carry);
    assert_eq!(b.surrounding_word_caps(), CAPS_RS);
    assert!(b.surrounding_stale());
    assert!(b.selection_possible());
    assert_eq!(b.selection_flag_cause, SelFlagCause::PointerSelect);
    // reset() 도 보존한다.
    b.reset();
    assert_eq!(b.surrounding_word_caps(), CAPS_RS);
    assert!(b.surrounding_stale());
    assert!(b.selection_possible());
}

#[test]
fn sw_focus_in_resets() {
    let mut e = eng("국어교육", 4);
    e.surrounding_hint(SURROUNDING_HINT_SELECTION_UNKNOWN, 0, 0);
    e.start_hanja_conversion();
    e.cancel_hanja();
    e.surrounding_word_focus_in();
    assert!(e.surrounding_stale());
    assert!(e.selection_possible());
    assert_eq!(e.selection_flag_cause, SelFlagCause::FocusIn);
    assert!(!e.selection_unknown_once);
    assert!(!e.surrounding_word_exhausted);
    assert_eq!(e.surrounding_word_caps(), CAPS_RS, "능력은 유지");
}

// =============================================================================
// 거부 플래그
// =============================================================================

fn flagged() -> InputEngine {
    let mut e = eng_caps(CAPS_RP, "국어교육", 4);
    e.surrounding_hint(SURROUNDING_HINT_PRIMARY_CHANGED, 0, 0);
    assert!(e.selection_possible());
    e
}

#[test]
fn sw_flag_set_shift_nav() {
    let mut e = eng_caps(CAPS_RP, "국어교육", 4);
    key(&mut e, KeyCode::Right, shift());
    assert!(e.selection_possible());
    assert_eq!(e.selection_flag_cause, SelFlagCause::ShiftNav);
}

#[test]
fn sw_flag_set_ctrl_shift_nav() {
    let mut e = eng_caps(CAPS_RP, "국어교육", 4);
    key(&mut e, KeyCode::End, ModifierState { shift: true, control: true, ..Default::default() });
    assert!(e.selection_possible());
}

#[test]
fn sw_flag_set_ctrl_a() {
    let mut e = eng_caps(CAPS_RP, "국어교육", 4);
    key(&mut e, KeyCode::A, ctrl());
    assert!(e.selection_possible());
    assert_eq!(e.selection_flag_cause, SelFlagCause::CtrlA);
}

#[test]
fn sw_flag_set_pointer_select() {
    let mut e = eng_caps(CAPS_RP, "국어교육", 4);
    e.surrounding_hint(SURROUNDING_HINT_POINTER_SELECT, 0, 0);
    assert!(e.selection_possible());
    assert_eq!(e.selection_flag_cause, SelFlagCause::PointerSelect);
}

#[test]
fn sw_flag_set_primary() {
    let e = flagged();
    assert_eq!(e.selection_flag_cause, SelFlagCause::PrimaryChanged);
}

#[test]
fn sw_flag_set_focus_in() {
    let mut e = eng_caps(CAPS_RP, "국어교육", 4);
    e.surrounding_word_focus_in();
    assert!(e.selection_possible());
}

#[test]
fn sw_flag_clear_plain_nav() {
    let mut e = flagged();
    key(&mut e, KeyCode::Left, m());
    assert!(!e.selection_possible());
}

#[test]
fn sw_flag_clear_ctrl_nav() {
    let mut e = flagged();
    key(&mut e, KeyCode::Left, ctrl());
    assert!(!e.selection_possible());
}

#[test]
fn sw_flag_clear_text_commit() {
    let mut e = flagged();
    e.set_input_category(InputCategory::English);
    key(&mut e, KeyCode::B, m());
    assert_eq!(e.commit_str(), "b");
    assert!(!e.selection_possible());
}

#[test]
fn sw_flag_clear_enter_tab_bs_del() {
    for k in [KeyCode::Enter, KeyCode::Tab, KeyCode::Backspace, KeyCode::Delete] {
        let mut e = flagged();
        key(&mut e, k, m());
        assert!(!e.selection_possible(), "{k:?}");
    }
}

#[test]
fn sw_flag_clear_ctrl_xvzy() {
    for k in [KeyCode::X, KeyCode::V, KeyCode::Z, KeyCode::Y] {
        let mut e = flagged();
        key(&mut e, k, ctrl());
        assert!(!e.selection_possible(), "{k:?}");
    }
    // Ctrl+C 는 표에 없다 — 바꾸지 않는다(I7).
    let mut e = flagged();
    key(&mut e, KeyCode::C, ctrl());
    assert!(e.selection_possible());
}

#[test]
fn sw_flag_clear_popup_confirm() {
    let mut e = eng_caps(CAPS_RS, "국어교육", 4);
    e.surrounding_hint(SURROUNDING_HINT_PRIMARY_CHANGED, 0, 0);
    f9(&mut e);
    key(&mut e, KeyCode::Num1, m());
    assert!(!e.selection_possible());
}

#[test]
fn sw_flag_clear_pointer_plain() {
    let mut e = flagged();
    e.surrounding_hint(SURROUNDING_HINT_POINTER_PLAIN, 0, 0);
    assert!(!e.selection_possible());
}

#[test]
fn sw_flag_unchanged_modifier_hanja_escape() {
    let mut e = flagged();
    key(&mut e, KeyCode::LeftShift, shift());
    assert!(e.selection_possible(), "수정자 단독");
    key(&mut e, KeyCode::Escape, m());
    assert!(e.selection_possible(), "표에 없는 키");
    key(&mut e, KeyCode::Right, ModifierState { alt: true, ..Default::default() });
    assert!(e.selection_possible(), "Alt+탐색");
    e.set_surrounding_text("국어교육".to_string(), 4, 4);
    f9(&mut e); // 한자키는 플래그를 바꾸지 않는다 → 이모지(POINTER_HINTS 만)
    assert!(e.selection_possible());
    assert!(e.is_emoji_popup_active());
}

#[test]
fn sw_reliable_ignores_flag() {
    let mut e = eng_caps(CAPS_RS, "국어교육", 4);
    key(&mut e, KeyCode::Right, shift());
    assert!(e.selection_possible());
    e.set_surrounding_text("국어교육".to_string(), 4, 4);
    f9(&mut e);
    assert!(is_sw(&e));
}

#[test]
fn sw_selection_unknown_one_key_only() {
    let mut e = eng("국어교육", 4);
    e.surrounding_hint(SURROUNDING_HINT_SELECTION_UNKNOWN, 0, 0);
    f9(&mut e);
    assert!(!e.is_hanja_mode());
    assert!(e.is_emoji_popup_active());
    key(&mut e, KeyCode::Escape, m()); // 한자키가 아닌 키 → 표시 해제
    assert!(!e.selection_unknown_once);
    f9(&mut e);
    assert!(is_sw(&e));
}

#[test]
fn sw_selection_unknown_survives_pull_triple_call() {
    let mut e = eng("국어교육", 4);
    e.surrounding_hint(SURROUNDING_HINT_SELECTION_UNKNOWN, 0, 0);
    // GetHanjaCandidates → (GetSpecialCharCandidates) → ProcessKey(F9)
    assert_eq!(e.start_hanja_conversion(), InputResult::consumed());
    assert!(e.selection_unknown_once);
    assert_eq!(e.start_hanja_conversion(), InputResult::consumed());
    f9(&mut e);
    assert!(e.is_emoji_popup_active());
    assert!(e.selection_unknown_once, "한자키는 표시를 내리지 않는다");
    // 새 surrounding 보고가 내린다.
    key(&mut e, KeyCode::Escape, m());
    e.set_surrounding_text("국어교육".to_string(), 4, 4);
    f9(&mut e);
    assert!(is_sw(&e));
}

#[test]
fn sw_late_primary_after_plain_nav_emoji() {
    let mut e = eng_caps(CAPS_RP, "국어교육", 4);
    key(&mut e, KeyCode::Right, m());
    e.set_surrounding_text("국어교육".to_string(), 4, 4);
    assert!(!e.selection_possible());
    e.surrounding_hint(SURROUNDING_HINT_PRIMARY_CHANGED, 0, 0); // 늦게 도착
    f9(&mut e);
    assert!(!e.is_hanja_mode());
    assert!(e.is_emoji_popup_active());
}

// =============================================================================
// 능력
// =============================================================================

#[test]
fn sw_no_caps_byte_identical_v15() {
    // 능력 미선언 → 대상③ 없음(이모지), 대상② 는 낡아도 종전대로 사용.
    let mut e = create_test_engine();
    e.set_surrounding_text("국어교육".to_string(), 4, 4);
    f9(&mut e);
    assert!(!e.is_hanja_mode());
    assert!(e.is_emoji_popup_active());

    let mut e = create_test_engine();
    e.set_surrounding_text("대한민국".to_string(), 4, 0);
    e.mark_surrounding_stale();
    f9(&mut e);
    assert_eq!(e.hanja_source(), HanjaSource::Selection);
}

#[test]
fn sw_caps_r_only_off() {
    let mut e = eng_caps(SURROUNDING_CAP_REPLACE, "국어교육", 4);
    f9(&mut e);
    assert!(!e.is_hanja_mode());
    assert_eq!(e.surrounding_word_resolve().unwrap_err().0, "nocap");
    // 선택 신뢰/포인터 힌트만 있고 REPLACE 가 없어도 꺼진다.
    let e = eng_caps(SURROUNDING_CAP_SELECTION_RELIABLE | SURROUNDING_CAP_POINTER_HINTS, "국어교육", 4);
    assert_eq!(e.surrounding_word_resolve().unwrap_err().0, "nocap");
}

#[test]
fn sw_caps_rs_on() {
    let mut e = eng_caps(CAPS_RS, "국어교육", 4);
    f9(&mut e);
    assert!(is_sw(&e));
}

#[test]
fn sw_caps_rp_on() {
    let mut e = eng_caps(CAPS_RP, "국어교육", 4);
    f9(&mut e);
    assert!(is_sw(&e));
}

#[test]
fn sw_caps_p_only_target2_stale_gate() {
    // 포인터 힌트만(대상③ 불가) — 대상② 의 신선도 게이트는 켜진다(caps != 0).
    let mut e = eng_caps(SURROUNDING_CAP_POINTER_HINTS, "대한민국", 0);
    e.set_surrounding_text("대한민국".to_string(), 4, 0);
    e.mark_surrounding_stale();
    f9(&mut e);
    assert!(!e.is_hanja_mode());
    assert!(e.is_emoji_popup_active());

    let mut e = eng_caps(SURROUNDING_CAP_POINTER_HINTS, "대한민국", 0);
    e.set_surrounding_text("대한민국".to_string(), 4, 0);
    f9(&mut e);
    assert_eq!(e.hanja_source(), HanjaSource::Selection);
}

// =============================================================================
// 비밀번호·로그
// =============================================================================

#[test]
fn sw_password_no_target3() {
    let mut e = eng("국어교육", 4);
    e.set_content_purpose(ContentPurpose::Password);
    assert_eq!(e.surrounding_word_resolve().unwrap_err().0, "sensitive");
    f9(&mut e);
    assert!(!e.is_hanja_mode());
    // 비밀번호 진입으로 열려 있던 대상③ 팝업은 재커밋 없이 닫힌다.
    let mut e = eng("국어교육", 4);
    f9(&mut e);
    assert!(is_sw(&e));
    e.set_content_purpose(ContentPurpose::Password);
    assert!(!e.is_hanja_mode());
    assert_eq!(e.surrounding_word_span(), None);
    assert_eq!(e.commit_str(), "");
}

#[test]
fn sw_hint_keysym_classification() {
    let cases: &[(u32, u32, bool, SelKeyEffect)] = &[
        (0xffe1, 0, true, SelKeyEffect::Keep),
        (0xffee, 0, true, SelKeyEffect::Keep),
        (0xff53, 0, false, SelKeyEffect::Clear), // Right
        (0xff53, 1, false, SelKeyEffect::Set(SelFlagCause::ShiftNav)),
        (0xff53, 1 | 4, false, SelKeyEffect::Set(SelFlagCause::ShiftNav)),
        (0xff53, 4, false, SelKeyEffect::Clear), // Ctrl+Right
        (0xff53, 8, false, SelKeyEffect::Keep),  // Alt+Right
        (0xff98, 1, false, SelKeyEffect::Set(SelFlagCause::ShiftNav)), // KP_Right
        (0xff50, 1, false, SelKeyEffect::Set(SelFlagCause::ShiftNav)), // Home
        (0xff56, 0, false, SelKeyEffect::Clear), // Page_Down
        (0xff0d, 0, false, SelKeyEffect::Clear), // Return
        (0xff09, 0, false, SelKeyEffect::Clear), // Tab
        (0xfe20, 1, false, SelKeyEffect::Clear), // ISO_Left_Tab
        (0xff08, 0, false, SelKeyEffect::Clear), // BackSpace
        (0xffff, 0, false, SelKeyEffect::Clear), // Delete
        (0x61, 4, false, SelKeyEffect::Set(SelFlagCause::CtrlA)),
        (0x41, 4, false, SelKeyEffect::Set(SelFlagCause::CtrlA)),
        (0x78, 4, false, SelKeyEffect::Clear), // Ctrl+x
        (0x56, 4, false, SelKeyEffect::Clear), // Ctrl+V
        (0x7a, 4, false, SelKeyEffect::Clear),
        (0x79, 4, false, SelKeyEffect::Clear),
        (0x63, 4, false, SelKeyEffect::Keep), // Ctrl+c
        (0x61, 0, false, SelKeyEffect::Clear), // 글자 입력은 선택을 대체
        (0xff1b, 0, false, SelKeyEffect::Keep), // Escape
    ];
    for &(keyval, state, is_mod, effect) in cases {
        assert_eq!(
            classify_sel_keysym(keyval, state),
            (is_mod, effect),
            "keyval=0x{keyval:x} state=0x{state:x}"
        );
    }
}

#[test]
fn sw_hint_log_name_masks_chars() {
    assert_eq!(hint_key_log_name(0xff51), "left");
    assert_eq!(hint_key_log_name(0xff50), "home");
    assert_eq!(hint_key_log_name(0xff0d), "enter");
    assert_eq!(hint_key_log_name(0xff08), "backspace");
    assert_eq!(hint_key_log_name(0x61), "char");
    assert_eq!(hint_key_log_name(0xac00), "char");
    assert_eq!(hint_key_log_name(0x0100_c8fd), "char");
}
