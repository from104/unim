//! `popup_change_page` 와 `toggle_hanja_bookmark` 반환값 동작 테스트.
//!
//! Phase 1 (mouse-paginate + flash) 의 엔진 측 회귀 방지:
//! - 마우스 ◀/▶ 페이지 이동의 wrap-around 동작
//! - cursor (sel_row, sel_col) 보존
//! - 단일 페이지에서 no-op
//! - 즐겨찾기 토글 반환값에 직전 상태(was_bookmarked) 포함
//!
//! 한자 사전(`HanjaDictionary`)이 환경 yaml 에 의존하므로, 본 테스트들은
//! 직접 `PopupState` 를 조립해 popup_state 만 가진 InputEngine 을 만들고
//! `popup_change_page` 를 검증한다 (실제 한자 후보 fetch 경로는 별도 시나리오 테스트).

use super::test_helpers::create_test_engine;
use super::{PageDirection, PopupAction};
use crate::config::{Config, InputCategory};
use crate::keycode::{KeyCode, ModifierState};
use crate::popup::PopupState;

/// 테스트용 — popup_state 에 직접 한자 PopupState 를 주입.
///
/// 한자 사전 fetch 를 우회해 페이지 동작만 검증한다. `n_candidates` 후보를 만들고
/// compact 모드 (페이지 9개씩) 로 시작.
fn make_engine_with_hanja_popup(n_candidates: usize) -> super::InputEngine {
    let mut engine = create_test_engine();
    let candidates: Vec<(String, String)> = (0..n_candidates)
        .map(|i| (format!("漢{}", i), format!("뜻{}", i)))
        .collect();
    let popup = PopupState::new_hanja("한", candidates);
    engine.popup_state = Some(popup);
    engine.hanja_mode = true;
    engine.hanja_target = "한".to_string();
    engine
}

#[test]
fn popup_change_page_next_advances_one_page() {
    // 27개 → compact 페이지 3개. Next → page 1.
    let mut engine = make_engine_with_hanja_popup(27);
    let r = engine.popup_change_page(PageDirection::Next);
    assert_eq!(r, Some(1));
    let state = engine.popup_state().unwrap();
    assert_eq!(state.current_page(), 1);
}

#[test]
fn popup_change_page_next_wraps_at_last() {
    // 27개 → 3 페이지. 마지막 페이지에서 Next → page 0 으로 wrap.
    let mut engine = make_engine_with_hanja_popup(27);
    // page 0 → 1 → 2 → wrap to 0
    engine.popup_change_page(PageDirection::Next);
    engine.popup_change_page(PageDirection::Next);
    let r = engine.popup_change_page(PageDirection::Next);
    assert_eq!(r, Some(0));
    assert_eq!(engine.popup_state().unwrap().current_page(), 0);
}

#[test]
fn popup_change_page_prev_wraps_at_first() {
    // 첫 페이지에서 Prev → 마지막 페이지 (wrap-around).
    let mut engine = make_engine_with_hanja_popup(27);
    let r = engine.popup_change_page(PageDirection::Prev);
    assert_eq!(r, Some(2));
    assert_eq!(engine.popup_state().unwrap().current_page(), 2);
}

#[test]
fn popup_change_page_returns_none_when_single_page() {
    // 5개 → 1 페이지. Next/Prev 모두 no-op (None).
    let mut engine = make_engine_with_hanja_popup(5);
    assert_eq!(engine.popup_change_page(PageDirection::Next), None);
    assert_eq!(engine.popup_change_page(PageDirection::Prev), None);
    assert_eq!(engine.popup_state().unwrap().current_page(), 0);
}

#[test]
fn popup_change_page_returns_none_when_no_popup() {
    let mut engine = create_test_engine();
    assert!(engine.popup_state().is_none());
    assert_eq!(engine.popup_change_page(PageDirection::Next), None);
    assert_eq!(engine.popup_change_page(PageDirection::Prev), None);
}

#[test]
fn popup_change_page_preserves_cursor_row_col_compact() {
    // compact 모드(1열). sel_row 가 페이지 이동 후에도 유지되어야 한다.
    let mut engine = make_engine_with_hanja_popup(27);
    if let Some(state) = engine.popup_state_mut() {
        state.set_navigate_state(0, 5, 0);
    }
    assert_eq!(engine.popup_state().unwrap().sel_row(), 5);

    engine.popup_change_page(PageDirection::Next);
    let state = engine.popup_state().unwrap();
    assert_eq!(state.current_page(), 1);
    // 두 번째 페이지도 9개 → row 5 유지.
    assert_eq!(state.sel_row(), 5);
    assert_eq!(state.sel_col(), 0);
}

#[test]
fn popup_change_page_preserves_cursor_row_col_expanded() {
    // expanded(9x9) 모드. sel_row=3, sel_col=4 위치를 페이지 이동에서 유지.
    // 162개 후보 → expanded 페이지 2개.
    let mut engine = make_engine_with_hanja_popup(162);
    if let Some(state) = engine.popup_state_mut() {
        state.toggle_hanja_expanded(); // expanded 진입
        state.set_navigate_state(0, 3, 4);
    }
    assert_eq!(engine.popup_state().unwrap().current_page(), 0);
    assert_eq!(engine.popup_state().unwrap().sel_row(), 3);
    assert_eq!(engine.popup_state().unwrap().sel_col(), 4);

    engine.popup_change_page(PageDirection::Next);
    let state = engine.popup_state().unwrap();
    assert_eq!(state.current_page(), 1);
    assert_eq!(state.sel_row(), 3);
    assert_eq!(state.sel_col(), 4);
}

#[test]
fn popup_change_page_emits_page_jump_action() {
    // popup_change_page 는 PopupAction::PageJump 를 pending 으로 설정해야 한다.
    let mut engine = make_engine_with_hanja_popup(27);
    engine.popup_change_page(PageDirection::Next);
    let action = engine.take_popup_action();
    match action {
        Some(PopupAction::PageJump { page_index }) => {
            assert_eq!(page_index, 1);
        }
        other => panic!("Expected PageJump, got {:?}", other),
    }
}

// =====================================================================
// toggle_hanja_bookmark 반환값에 was_bookmarked 포함 검증
// =====================================================================

/// 헬퍼 — 한자 모드 진입까지 수행. 사전이 없으면 None 반환.
///
/// 환경별 한자 즐겨찾기 yaml 의 영향을 배제하기 위해, 본 테스트들은 토글의
/// **invariant** (was != new, action 페이로드 일관성) 만 검증한다 — 절대 상태
/// (true/false) 가 아니라 ON/OFF 전환 여부만 본다.
fn enter_hanja_mode_for_test() -> Option<super::InputEngine> {
    let mut engine = create_test_engine();
    engine.set_input_category(InputCategory::Korean);
    let config = Config::default();
    let modifier = ModifierState::default();
    engine.press_key(KeyCode::R, modifier, &config);
    engine.press_key(KeyCode::K, modifier, &config);
    let r = engine.start_hanja_conversion();
    if !r.hanja_candidates_available {
        eprintln!("환경에 한자 사전 없음 — 본 테스트 skip");
        return None;
    }
    Some(engine)
}

#[test]
fn toggle_hanja_bookmark_returns_was_bookmarked_field_inverted_from_new_state() {
    // was != new 가 항상 성립해야 한다 (toggle 의 정의).
    let Some(mut engine) = enter_hanja_mode_for_test() else {
        return;
    };
    let result = engine.toggle_hanja_bookmark(0);
    assert!(result.is_some());
    let (_idx, new_state, was_state) = result.unwrap();
    assert_ne!(was_state, new_state, "토글 후 was != new");
}

#[test]
fn toggle_hanja_bookmark_double_toggle_returns_to_original() {
    // 두 번 토글하면 원래 상태로 돌아온다.
    let Some(mut engine) = enter_hanja_mode_for_test() else {
        return;
    };
    let (idx_after_first, state_after_first, _initial_was) =
        engine.toggle_hanja_bookmark(0).unwrap();
    let _ = engine.take_popup_action();

    // 2차 토글 — 같은 한자가 재정렬된 새 위치(idx_after_first)에 있음.
    let (_idx2, state_after_second, was_before_second) =
        engine.toggle_hanja_bookmark(idx_after_first).unwrap();
    // 두 번째 토글의 직전 상태 = 첫 번째 토글의 결과 상태.
    assert_eq!(was_before_second, state_after_first);
    // 새 상태는 그 반대.
    assert_ne!(state_after_second, state_after_first);
}

#[test]
fn toggle_hanja_bookmark_action_carries_was_bookmarked_field() {
    // PopupAction::HanjaCandidatesReordered 페이로드에 was_bookmarked 가 들어있고
    // bookmarked != was_bookmarked 인지 검증 (toggle invariant).
    let Some(mut engine) = enter_hanja_mode_for_test() else {
        return;
    };
    // ShowHanja 비우기.
    let _ = engine.take_popup_action();
    engine.toggle_hanja_bookmark(0);
    let action = engine.take_popup_action();
    match action {
        Some(PopupAction::HanjaCandidatesReordered {
            bookmarked,
            was_bookmarked,
            ..
        }) => {
            assert_ne!(
                was_bookmarked, bookmarked,
                "토글 후 페이로드의 was != new"
            );
        }
        other => panic!("Expected HanjaCandidatesReordered, got {:?}", other),
    }
}
