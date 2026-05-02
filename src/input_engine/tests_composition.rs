//! 한글 조합·BS·Space·Enter·Modifier·Layout 변경 등 키 dispatch 단위 테스트.
//!
//! `tests_scenarios.rs`(통합 시나리오)와 분리된 좁은 단위 테스트들.

use super::test_helpers::create_test_engine;
use super::InputEngine;
use crate::config::{Config, InputCategory};
use crate::keycode::{KeyCode, ModifierState};

#[test]
fn test_korean_basic_input() {
    let mut engine = create_test_engine();
    let config = Config::default();
    let modifier = ModifierState::default();

    // 한글 모드로 전환
    engine.set_input_category(InputCategory::Korean);

    // ㄱ 입력
    let result = engine.press_key(KeyCode::R, modifier, &config);
    assert!(result.consumed);
    assert!(result.preedit_changed);
    assert_eq!(engine.preedit_str(), "ㄱ");

    // ㅏ 입력 → 가
    let result = engine.press_key(KeyCode::K, modifier, &config);
    assert!(result.consumed);
    assert_eq!(engine.preedit_str(), "가");
}

#[test]
fn test_korean_syllable_commit() {
    let mut engine = create_test_engine();
    let config = Config::default();
    let modifier = ModifierState::default();

    engine.set_input_category(InputCategory::Korean);

    // 가 입력 (ㄱ + ㅏ)
    engine.press_key(KeyCode::R, modifier, &config);
    engine.press_key(KeyCode::K, modifier, &config);

    // ㄴ 입력 → 2벌식: 종성으로 추가되어 '간'
    let result = engine.press_key(KeyCode::S, modifier, &config);
    assert!(result.consumed);
    assert!(!result.commit_changed);
    assert_eq!(engine.preedit_str(), "간");

    // ㅏ 입력 → 도깨비불: '가' 커밋 + '나' preedit
    let result = engine.press_key(KeyCode::K, modifier, &config);
    assert!(result.consumed);
    assert!(result.commit_changed);
    assert_eq!(engine.commit_str(), "가");
    assert_eq!(engine.preedit_str(), "나");
}

// === Modifier 키 테스트 ===

#[test]
fn test_modifier_key_not_consumed() {
    let mut engine = create_test_engine();
    let config = Config::default();
    let modifier = ModifierState::default();

    // Shift만 누르면 무시
    let result = engine.press_key(KeyCode::LeftShift, modifier, &config);
    assert!(!result.consumed);
}

#[test]
fn test_ctrl_flushes_preedit() {
    let mut engine = create_test_engine();
    let config = Config::default();
    let modifier = ModifierState::default();

    engine.set_input_category(InputCategory::Korean);
    engine.press_key(KeyCode::R, modifier, &config); // ㄱ
    engine.press_key(KeyCode::K, modifier, &config); // 가

    // Ctrl+C → 조합 커밋
    let ctrl_modifier = ModifierState {
        control: true,
        ..Default::default()
    };
    let result = engine.press_key(KeyCode::C, ctrl_modifier, &config);
    assert!(result.commit_changed);
    assert_eq!(engine.commit_str(), "가");
}

// === Space 처리 테스트 ===

#[test]
fn test_korean_space_commits() {
    let mut engine = create_test_engine();
    let config = Config::default();
    let modifier = ModifierState::default();

    engine.set_input_category(InputCategory::Korean);
    engine.press_key(KeyCode::R, modifier, &config); // ㄱ
    engine.press_key(KeyCode::K, modifier, &config); // 가

    let result = engine.press_key(KeyCode::Space, modifier, &config);
    assert!(result.consumed);
    assert!(result.commit_changed);
    // "가" + " "
    assert!(engine.commit_str().contains("가"));
}

// === Enter/Tab/Escape → committed_passthrough 테스트 ===

#[test]
fn test_enter_commits_passthrough() {
    let mut engine = create_test_engine();
    let config = Config::default();
    let modifier = ModifierState::default();

    engine.set_input_category(InputCategory::Korean);
    engine.press_key(KeyCode::R, modifier, &config); // ㄱ

    let result = engine.press_key(KeyCode::Enter, modifier, &config);
    assert!(!result.consumed); // passthrough
    assert!(result.commit_changed);
}

#[test]
fn test_enter_not_composing_passthrough() {
    let mut engine = create_test_engine();
    let config = Config::default();
    let modifier = ModifierState::default();

    engine.set_input_category(InputCategory::Korean);
    let result = engine.press_key(KeyCode::Enter, modifier, &config);
    assert!(!result.consumed);
    assert!(!result.commit_changed);
}

// === Backspace 테스트 ===

#[test]
fn test_korean_backspace() {
    let mut engine = create_test_engine();
    let config = Config::default();
    let modifier = ModifierState::default();

    engine.set_input_category(InputCategory::Korean);
    engine.press_key(KeyCode::R, modifier, &config); // ㄱ
    engine.press_key(KeyCode::K, modifier, &config); // 가

    let result = engine.press_key(KeyCode::Backspace, modifier, &config);
    assert!(result.consumed);
    assert!(result.preedit_changed);
    assert_eq!(engine.preedit_str(), "ㄱ");
}

#[test]
fn test_backspace_not_composing() {
    let mut engine = create_test_engine();
    let config = Config::default();
    let modifier = ModifierState::default();

    engine.set_input_category(InputCategory::Korean);
    let result = engine.press_key(KeyCode::Backspace, modifier, &config);
    assert!(!result.consumed); // 앱으로 전달
}

// === 도깨비불 through engine 테스트 ===

#[test]
fn test_engine_dokkaebi() {
    let mut engine = create_test_engine();
    let config = Config::default();
    let modifier = ModifierState::default();

    engine.set_input_category(InputCategory::Korean);
    // ㄱㅏㄱ → 각
    engine.press_key(KeyCode::R, modifier, &config);
    engine.press_key(KeyCode::K, modifier, &config);
    engine.press_key(KeyCode::R, modifier, &config);
    assert_eq!(engine.preedit_str(), "각");

    // ㅏ → 도깨비불 → 가 + 가
    engine.press_key(KeyCode::K, modifier, &config);
    assert_eq!(engine.commit_str(), "가");
    assert_eq!(engine.preedit_str(), "가");
}

// === 영어 모드 Shift 테스트 ===

#[test]
fn test_english_shift_uppercase() {
    let mut engine = create_test_engine();
    let config = Config::default();

    engine.set_input_category(InputCategory::English);
    let shift_modifier = ModifierState {
        shift: true,
        ..Default::default()
    };

    let result = engine.press_key(KeyCode::A, shift_modifier, &config);
    assert!(result.consumed);
    assert_eq!(engine.commit_str(), "A");
}

// === 레이아웃 변경 테스트 ===

#[test]
fn test_set_korean_layout() {
    let mut config = Config::default();
    config.engine.korean.layout = "ko_3bul390".to_string();

    let engine = InputEngine::new(&config);
    assert_eq!(engine.korean_layout, "ko_3bul390");
}

#[test]
fn test_set_english_layout_dvorak() {
    let mut config = Config::default();
    config.engine.english.layout = "dvorak".to_string();

    let engine = InputEngine::new(&config);
    assert_eq!(engine.english_layout, "dvorak");
}

// === 한/영 전환 중 조합 커밋 테스트 ===

#[test]
fn test_toggle_while_composing() {
    let mut engine = create_test_engine();
    let config = Config::default();
    let modifier = ModifierState::default();

    engine.set_input_category(InputCategory::Korean);
    engine.press_key(KeyCode::R, modifier, &config); // ㄱ
    engine.press_key(KeyCode::K, modifier, &config); // 가

    // 한/영 전환 → 조합 커밋
    let result = engine.press_key(KeyCode::Korean, modifier, &config);
    assert!(result.consumed);
    assert!(result.commit_changed);
    assert_eq!(engine.commit_str(), "가");
    assert_eq!(engine.input_category(), InputCategory::English);
}
