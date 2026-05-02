//! 이모지 팝업 트리거 (parse_emoji_trigger + Super+Period 등) 테스트.

use super::test_helpers::create_test_engine;
use super::{InputEngine, PopupAction};
use crate::config::Config;
use crate::keycode::{KeyCode, ModifierState};

#[test]
fn test_emoji_trigger_parse_super_period() {
    let parsed = InputEngine::parse_emoji_trigger("Super+Period");
    assert!(parsed.is_some());
    let (modifier, keycode) = parsed.unwrap();
    assert!(modifier.super_key);
    assert!(!modifier.control);
    assert!(!modifier.alt);
    assert!(!modifier.shift);
    assert_eq!(keycode, KeyCode::Period);
}

#[test]
fn test_emoji_trigger_parse_aliases() {
    assert!(
        InputEngine::parse_emoji_trigger("Meta+Period")
            .unwrap()
            .0
            .super_key
    );
    assert!(
        InputEngine::parse_emoji_trigger("Win+Period")
            .unwrap()
            .0
            .super_key
    );
    assert!(
        InputEngine::parse_emoji_trigger("Ctrl+Semicolon")
            .unwrap()
            .0
            .control
    );
}

#[test]
fn test_emoji_trigger_parse_multi_modifier() {
    let parsed = InputEngine::parse_emoji_trigger("Control+Shift+E");
    assert!(parsed.is_some());
    let (modifier, keycode) = parsed.unwrap();
    assert!(modifier.control);
    assert!(modifier.shift);
    assert_eq!(keycode, KeyCode::E);
}

#[test]
fn test_emoji_trigger_parse_invalid() {
    assert!(InputEngine::parse_emoji_trigger("Super+Bogus").is_none());
    assert!(InputEngine::parse_emoji_trigger("Super").is_none());
    assert!(InputEngine::parse_emoji_trigger("").is_none());
}

#[test]
fn test_emoji_trigger_fires_popup_action() {
    let engine = create_test_engine();
    assert!(engine.emoji_popup_enabled);
    assert!(!engine.emoji_triggers.is_empty());

    let mut engine = engine;
    let config = Config::default();
    let modifier = ModifierState {
        super_key: true,
        ..Default::default()
    };
    let result = engine.press_key(KeyCode::Period, modifier, &config);
    assert!(result.consumed);
    let action = engine.take_popup_action();
    assert!(matches!(action, Some(PopupAction::ShowEmoji { .. })));
}

#[test]
fn test_emoji_trigger_only_super_period_matches() {
    let mut engine = create_test_engine();
    let config = Config::default();

    // Period 단독은 트리거 아님
    engine.press_key(KeyCode::Period, ModifierState::default(), &config);
    assert!(engine.take_popup_action().is_none());

    // Control+Period 도 아님 (설정된 트리거가 Super+Period 뿐)
    let ctrl = ModifierState {
        control: true,
        ..Default::default()
    };
    engine.press_key(KeyCode::Period, ctrl, &config);
    assert!(engine.take_popup_action().is_none());

    // Super+Comma 도 아님
    let sup = ModifierState {
        super_key: true,
        ..Default::default()
    };
    engine.press_key(KeyCode::Comma, sup, &config);
    assert!(engine.take_popup_action().is_none());
}

#[test]
fn test_emoji_trigger_disabled_when_config_off() {
    let mut config = Config::default();
    config.engine.emoji_popup.enabled = false;
    let mut engine = InputEngine::new(&config);

    let modifier = ModifierState {
        super_key: true,
        ..Default::default()
    };
    let result = engine.press_key(KeyCode::Period, modifier, &config);
    assert!(engine.take_popup_action().is_none());
    // 비활성화 시에는 기존 단축키 경로로 흘러가 소비되지 않음
    assert!(!result.consumed);
}

#[test]
fn test_emoji_trigger_flushes_composing() {
    let mut engine = create_test_engine();
    engine.set_input_category(crate::config::InputCategory::Korean);
    let config = Config::default();

    // ㄱ 조합 시작
    engine.press_key(KeyCode::R, ModifierState::default(), &config);
    assert!(engine.korean_context.is_composing());

    // Super+. 누르면 조합이 커밋되고 이모지 팝업 액션이 큐잉됨
    let sup = ModifierState {
        super_key: true,
        ..Default::default()
    };
    let result = engine.press_key(KeyCode::Period, sup, &config);
    assert!(result.consumed);
    assert!(result.commit_changed);
    assert!(matches!(
        engine.take_popup_action(),
        Some(PopupAction::ShowEmoji { .. })
    ));
}

/// 이슈 #1 검증: emoji 팝업 진입 시 Recent 가 비었으면 두 번째 탭(cat_index=1)
/// 으로 시작하고, 비어있지 않으면 Recent 탭(cat_index=0) 으로 시작한다.
///
/// `load_recent()` 가 환경(`~/.config/unim/emoji-recent.yaml`)에 의존하므로
/// 본 테스트는 두 시나리오를 contract 형태로 검증한다 — 즉 popup_state 의
/// cat_index 가 0 이면 recent_emojis 비어있지 않아야 하고, cat_index=1 이면
/// recent_emojis 비어있고 새 카테고리의 items 가 비어있지 않아야 한다.
#[test]
fn test_emoji_popup_skips_empty_recent_tab() {
    let mut engine = create_test_engine();
    let config = Config::default();
    let modifier = ModifierState {
        super_key: true,
        ..Default::default()
    };
    let result = engine.press_key(KeyCode::Period, modifier, &config);
    assert!(result.consumed);
    let action = engine.take_popup_action();
    assert!(matches!(action, Some(PopupAction::ShowEmoji { .. })));

    let state = engine.popup_state().expect("emoji popup_state present");
    let cat_index = state.emoji_cat_index();
    let cats = state.emoji_categories();
    assert!(cats.len() >= 2, "minimum: Recent + 1 정적 카테고리");

    if state.emoji_recent().is_empty() {
        // Recent 비어있으면 두 번째 탭으로 자동 진입.
        assert_eq!(
            cat_index, 1,
            "빈 recent → cat_index=1 (두 번째 탭)으로 시작해야 함"
        );
        // 두 번째 탭은 정적 카테고리 (SmileysPeople 등)이므로 풀이 비어있지 않다.
        assert!(
            state.emoji_items().len() > 0,
            "두 번째 카테고리는 items 가 비어있지 않아야 함"
        );
    } else {
        // Recent 가 있으면 첫 탭(0) 으로 시작.
        assert_eq!(
            cat_index, 0,
            "비어있지 않은 recent → cat_index=0 (Recent 탭) 유지"
        );
    }
}

#[test]
fn test_emoji_custom_trigger_from_config() {
    let mut config = Config::default();
    config.engine.emoji_popup.trigger_keys = vec!["Control+Shift+E".to_string()];
    let mut engine = InputEngine::new(&config);

    // 기본 Super+Period는 이제 트리거 아님
    let sup = ModifierState {
        super_key: true,
        ..Default::default()
    };
    engine.press_key(KeyCode::Period, sup, &config);
    assert!(engine.take_popup_action().is_none());

    // Control+Shift+E가 트리거
    let ctrl_shift = ModifierState {
        control: true,
        shift: true,
        ..Default::default()
    };
    engine.press_key(KeyCode::E, ctrl_shift, &config);
    assert!(matches!(
        engine.take_popup_action(),
        Some(PopupAction::ShowEmoji { .. })
    ));
}
