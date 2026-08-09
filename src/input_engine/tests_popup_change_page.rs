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

/// 회귀: 즐겨찾기 등록 후 다시 해제하면 후보가 자연(빈도) 위치로 돌아가야 한다.
///
/// 기존 버그 — `sort_by_key(|e| !is_bookmarked(...))` 가 stable sort 를 쓰는데
/// 모든 후보의 키가 동일(true)해지는 해제 직후에는 입력 순서를 유지해버려,
/// 방금 해제된 한자가 0번 자리에 그대로 머물렀다. dict.search 로 자연순서를
/// 재로드해 정렬 기반을 리셋하면 자연 위치로 복구된다.
///
/// 환경 yaml 에 다른 즐겨찾기가 잔존할 수 있으므로 절대 인덱스(0번 promote)를
/// 단정하지 않는다. 진짜 invariant 만 검증: **ON→OFF 토글 후, 후보 한자는 dict
/// 자연순서 (i.e. dict.search 의 결과 순서) 의 자기 위치로 돌아가야 한다.**
#[test]
fn unbookmark_restores_candidate_to_natural_dict_position() {
    let Some(mut engine) = enter_hanja_mode_for_test() else {
        return;
    };
    let _ = engine.take_popup_action();

    // 환경에 잔존하는 즐겨찾기를 모두 OFF 로 클리어한 자연순서를 baseline 으로 잡는다.
    // 클리어 자체가 토글 호출 + reorder 를 유발하므로 단순화하기 위해, baseline 은
    // "현재 hanja_candidates 에서 즐겨찾기 ON 인 항목들을 제거한 나머지의 순서" 로
    // 정의한다. 이 순서가 곧 "dict 자연 순서 (즐겨찾기 없는 항목들만)" 이다.
    let initial: Vec<(String, String)> = engine.get_hanja_candidates();
    let initial_flags: Vec<bool> = engine.hanja_bookmark_states();
    let natural: Vec<String> = initial
        .iter()
        .zip(initial_flags.iter())
        .filter(|(_, &b)| !b)
        .map(|(p, _)| p.0.clone())
        .collect();
    if natural.len() < 5 {
        eprintln!("환경 사전 미즐겨찾기 후보 < 5 — 회귀 검증 skip");
        return;
    }
    // 자연순서의 5번째 항목 (충분히 뒤쪽이라 promote↔demote 변화가 의미있음).
    let target_hanja = natural[4].clone();
    let target_idx = initial
        .iter()
        .position(|(h, _)| h == &target_hanja)
        .expect("target_hanja 는 initial 에 존재");

    // (1) 토글 ON. 반환은 (new_idx, new_state, prev_state).
    let (_idx_on, on_state, prev_state) = engine.toggle_hanja_bookmark(target_idx).unwrap();
    let _ = engine.take_popup_action();
    assert!(!prev_state, "테스트 전제: target 이 OFF 상태였어야 함");
    assert!(on_state, "ON 으로 전환됨");

    // (2) 토글 OFF: target 한자를 새 위치에서 찾아 다시 OFF.
    let after_on: Vec<(String, String)> = engine.get_hanja_candidates();
    let on_idx = after_on
        .iter()
        .position(|(h, _)| h == &target_hanja)
        .expect("ON 후에도 target 한자는 존재");
    let (idx_off, off_state, _) = engine.toggle_hanja_bookmark(on_idx).unwrap();
    let _ = engine.take_popup_action();
    assert!(!off_state, "OFF 로 전환됐어야 함");

    // 핵심 검증: OFF 후 target 의 위치는 "자연순서에서 그 한자가 차지하던 자리".
    // 즉, 미즐겨찾기 항목들만 모은 자연순서 (`natural`) 의 5번째(인덱스 4) 자리.
    // 즐겨찾기 ON 인 항목들이 위에 있을 수 있으므로, 절대 인덱스는 그만큼 밀린다.
    let after_off: Vec<(String, String)> = engine.get_hanja_candidates();
    let off_flags: Vec<bool> = engine.hanja_bookmark_states();
    let leading_bookmarks = off_flags.iter().take_while(|&&b| b).count();
    let expected_idx = leading_bookmarks + 4;
    assert_eq!(
        idx_off, expected_idx,
        "해제 후 자연 위치({})로 복귀해야 함 — got {} ({}). after_off[idx_off]='{:?}'",
        expected_idx,
        idx_off,
        target_hanja,
        after_off.get(idx_off)
    );
    assert_eq!(
        after_off[idx_off].0, target_hanja,
        "해제 후 그 위치의 한자는 target 그 자체"
    );
}

// =====================================================================
// Hanja 키 dual-purpose: preedit 있으면 한자, idle 이면 이모지 트리거
// =====================================================================

/// preedit 있을 때 Hanja → 기존대로 한자 모드 진입 (회귀).
#[test]
fn hanja_key_with_preedit_starts_hanja_conversion() {
    let mut engine = create_test_engine();
    engine.set_input_category(InputCategory::Korean);
    let config = Config::default();
    let m = ModifierState::default();
    engine.press_key(KeyCode::R, m, &config); // ㄱ
    engine.press_key(KeyCode::K, m, &config); // ㅏ
    let r = engine.press_key(KeyCode::Hanja, m, &config);
    if !r.hanja_candidates_available {
        eprintln!("환경 사전 없음 — skip");
        return;
    }
    assert!(engine.is_hanja_mode(), "Hanja 키 → 한자 모드 진입");
}

/// idle 상태(preedit 없음) → 이모지 팝업 진입 (항상 ON).
#[test]
fn hanja_key_idle_with_emoji_enabled_starts_emoji_popup() {
    let mut engine = create_test_engine();
    engine.set_input_category(InputCategory::Korean);
    let config = Config::default();
    let m = ModifierState::default();
    let _r = engine.press_key(KeyCode::Hanja, m, &config);
    // 이모지 팝업 활성: popup_state.kind() == Emoji
    let kind = engine.popup_state().map(|s| s.kind());
    assert_eq!(
        kind,
        Some(crate::popup::PopupKind::Emoji),
        "idle Hanja + emoji_enabled → 이모지 팝업"
    );
    assert!(!engine.is_hanja_mode(), "한자 모드는 진입하지 않음");
}

/// `hanja_keys` config 의 두 번째 키(F9)도 한자 모드 트리거 — 이전엔 dead 였음.
#[test]
fn hanja_keys_config_f9_also_triggers_hanja_mode() {
    let mut engine = create_test_engine();
    engine.set_input_category(InputCategory::Korean);
    let config = Config::default();
    let m = ModifierState::default();
    engine.press_key(KeyCode::R, m, &config); // ㄱ
    engine.press_key(KeyCode::K, m, &config); // ㅏ
    let r = engine.press_key(KeyCode::F9, m, &config);
    if !r.hanja_candidates_available {
        eprintln!("환경 사전 없음 — skip");
        return;
    }
    assert!(engine.is_hanja_mode(), "F9 도 hanja_keys config 매칭 시 한자 모드");
}

/// 회귀 방지: English 모드 idle 상태에서도 Hanja 키가 이모지 팝업을 트리거해야 함.
/// 종전엔 dispatch 가 process_korean_key 안에 있어 영문 모드 첫 Hanja 키가
/// not_consumed 로 떨어져 무시됐다 (터미널 부팅 직후 사용자 보고 회귀).
#[test]
fn hanja_key_idle_in_english_mode_starts_emoji_popup() {
    let mut engine = create_test_engine();
    engine.set_input_category(InputCategory::English);
    let config = Config::default();
    let m = ModifierState::default();
    let r = engine.press_key(KeyCode::Hanja, m, &config);
    assert!(r.consumed, "Hanja 키는 영문 모드라도 consumed");
    let kind = engine.popup_state().map(|s| s.kind());
    assert_eq!(
        kind,
        Some(crate::popup::PopupKind::Emoji),
        "English 모드 idle Hanja → 이모지 팝업"
    );
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
