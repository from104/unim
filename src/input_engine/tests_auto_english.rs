//! 자동 영문 전환(auto_english) 트리거 동작 + `parse_trigger_key` 단위 테스트.
//!
//! - Functional 트리거: Escape/Slash/ShiftSemicolon 등 KeyCode 기반
//! - Character 트리거: `char:/` 등 산출 문자 기반 (레이아웃 무관)
//! - 회귀 핵심: ko_3bul390 + `char:/` + Shift+G → '/' commit + 영문 전환

use super::test_helpers::create_test_engine;
use super::types::AutoEnglishTrigger;
use super::InputEngine;
use crate::config::{Config, ContentPurpose, InputCategory};
use crate::keycode::{KeyCode, ModifierState};

fn make_engine_with_auto_english(trigger_keys: Vec<&str>) -> (InputEngine, Config) {
    let mut config = Config::default();
    config.engine.auto_english.enabled = true;
    config.engine.auto_english.trigger_keys =
        trigger_keys.into_iter().map(|s| s.to_string()).collect();
    let engine = InputEngine::new(&config);
    (engine, config)
}

/// 한국어 레이아웃을 명시한 자동 영문 엔진 헬퍼 (회귀 테스트용).
fn make_engine_with_layout_and_triggers(
    korean_layout: &str,
    trigger_keys: Vec<&str>,
) -> (InputEngine, Config) {
    let mut config = Config::default();
    config.engine.korean.layout = korean_layout.into();
    config.engine.auto_english.enabled = true;
    config.engine.auto_english.trigger_keys =
        trigger_keys.into_iter().map(|s| s.to_string()).collect();
    let engine = InputEngine::new(&config);
    (engine, config)
}

/// 기본값(비활성)에서는 Escape가 기존 §3.6 동작만 수행한다.
#[test]
fn test_auto_english_disabled_preserves_escape_passthrough() {
    let mut engine = create_test_engine();
    let config = Config::default();
    let modifier = ModifierState::default();

    engine.set_input_category(InputCategory::Korean);
    engine.press_key(KeyCode::R, modifier, &config); // ㄱ
    engine.press_key(KeyCode::K, modifier, &config); // 가

    let result = engine.press_key(KeyCode::Escape, modifier, &config);
    assert!(result.commit_changed, "조합은 커밋되어야 함");
    assert!(!result.consumed, "ESC 자체는 passthrough");
    assert_eq!(engine.commit_str(), "가");
    // auto_english 비활성이므로 모드는 한글 유지
    assert_eq!(engine.input_category(), InputCategory::Korean);
}

/// 조합 중 Escape → 커밋 + 영문 전환 + passthrough
#[test]
fn test_auto_english_escape_switches_to_english() {
    let (mut engine, config) = make_engine_with_auto_english(vec!["Escape"]);
    let modifier = ModifierState::default();

    engine.set_input_category(InputCategory::Korean);
    engine.press_key(KeyCode::R, modifier, &config); // ㄱ
    engine.press_key(KeyCode::K, modifier, &config); // 가

    let result = engine.press_key(KeyCode::Escape, modifier, &config);
    assert!(result.commit_changed, "조합이 커밋되어야 함");
    assert!(!result.consumed, "ESC 키는 앱에 passthrough (vi 호환)");
    assert_eq!(engine.commit_str(), "가");
    assert_eq!(engine.input_category(), InputCategory::English);
}

/// 조합 중 '/' → 커밋 + 영문 전환 + '/' commit
#[test]
fn test_auto_english_slash_commits_slash() {
    let (mut engine, config) = make_engine_with_auto_english(vec!["Slash"]);
    let modifier = ModifierState::default();

    engine.set_input_category(InputCategory::Korean);
    engine.press_key(KeyCode::R, modifier, &config); // ㄱ
    engine.press_key(KeyCode::K, modifier, &config); // 가

    let result = engine.press_key(KeyCode::Slash, modifier, &config);
    assert!(result.consumed, "'/'는 IME가 소비하여 commit");
    assert!(result.commit_changed);
    assert_eq!(engine.commit_str(), "가/");
    assert_eq!(engine.input_category(), InputCategory::English);
}

/// Shift+Semicolon → ':' commit + 영문 전환
#[test]
fn test_auto_english_shift_semicolon_commits_colon() {
    let (mut engine, config) = make_engine_with_auto_english(vec!["ShiftSemicolon"]);
    let modifier = ModifierState {
        shift: true,
        ..Default::default()
    };

    engine.set_input_category(InputCategory::Korean);
    let no_shift = ModifierState::default();
    engine.press_key(KeyCode::R, no_shift, &config); // ㄱ
    engine.press_key(KeyCode::K, no_shift, &config); // 가

    let result = engine.press_key(KeyCode::Semicolon, modifier, &config);
    assert!(result.consumed);
    assert!(result.commit_changed);
    assert_eq!(engine.commit_str(), "가:");
    assert_eq!(engine.input_category(), InputCategory::English);
}

/// 영문 모드에서는 자동 영문 트리거가 no-op. 기존 동작만 적용된다.
#[test]
fn test_auto_english_noop_in_english_mode() {
    let (mut engine, config) =
        make_engine_with_auto_english(vec!["Escape", "Slash", "ShiftSemicolon"]);
    let modifier = ModifierState::default();

    // 기본은 영문 모드
    assert_eq!(engine.input_category(), InputCategory::English);

    // ESC: 영문 모드의 process_english_key에서는 문자가 없어 not_consumed
    let result = engine.press_key(KeyCode::Escape, modifier, &config);
    assert!(!result.consumed);
    assert_eq!(engine.input_category(), InputCategory::English);
}

/// 커스텀 트리거 키: "Period"만 설정하면 '.'만 영문 전환을 트리거한다.
#[test]
fn test_auto_english_custom_keys() {
    let (mut engine, config) = make_engine_with_auto_english(vec!["Period"]);
    let modifier = ModifierState::default();

    engine.set_input_category(InputCategory::Korean);
    engine.press_key(KeyCode::R, modifier, &config); // ㄱ
    engine.press_key(KeyCode::K, modifier, &config); // 가

    // ESC는 트리거가 아니므로 기존 §3.6 동작 + 모드 유지
    let result = engine.press_key(KeyCode::Escape, modifier, &config);
    assert!(result.commit_changed);
    assert_eq!(engine.input_category(), InputCategory::Korean);

    // '.'는 트리거 → 영문 전환
    engine.set_input_category(InputCategory::Korean);
    engine.clear_commit();
    engine.press_key(KeyCode::R, modifier, &config);
    engine.press_key(KeyCode::K, modifier, &config);
    let result = engine.press_key(KeyCode::Period, modifier, &config);
    assert!(result.consumed);
    assert!(result.commit_changed);
    let committed = engine.commit_str();
    assert!(committed.ends_with('.'), "committed='{}'", committed);
    assert_eq!(engine.input_category(), InputCategory::English);
}

/// `"Slash"`만 지정하면 Shift+Slash('?')는 트리거가 아니다.
#[test]
fn test_auto_english_shift_slash_does_not_trigger() {
    let (mut engine, config) = make_engine_with_auto_english(vec!["Slash"]);
    let shift = ModifierState {
        shift: true,
        ..Default::default()
    };

    engine.set_input_category(InputCategory::Korean);

    // Shift+Slash = '?' → 트리거가 아니므로 한글 모드 유지
    let _ = engine.press_key(KeyCode::Slash, shift, &config);
    assert_eq!(engine.input_category(), InputCategory::Korean);
}

/// 비밀번호 필드는 이미 영문 강제 전환이므로 자동 영문 전환 훅이 도달해도 영향 없음.
#[test]
fn test_auto_english_password_field_unchanged() {
    let (mut engine, config) = make_engine_with_auto_english(vec!["Escape"]);
    let modifier = ModifierState::default();

    engine.set_input_category(InputCategory::Korean);
    engine.set_content_purpose(ContentPurpose::Password);

    // 비밀번호 필드에서 한글 키 입력 → press_key 상단 가드가 영문으로 전환
    engine.press_key(KeyCode::A, modifier, &config);
    assert_eq!(engine.input_category(), InputCategory::English);
}

// === Character 카테고리 (key:/char: 이원화) 테스트 ===

/// QWERTY 한국어 + `char:/` + KeyCode::Slash → 영문 전환 + '/' commit
#[test]
fn test_auto_english_character_qwerty_slash() {
    let (mut engine, config) =
        make_engine_with_layout_and_triggers("ko_2bulstd", vec!["char:/"]);
    let modifier = ModifierState::default();

    engine.set_input_category(InputCategory::Korean);
    engine.press_key(KeyCode::R, modifier, &config); // ㄱ
    engine.press_key(KeyCode::K, modifier, &config); // 가

    let result = engine.press_key(KeyCode::Slash, modifier, &config);
    assert!(result.consumed);
    assert!(result.commit_changed);
    assert_eq!(engine.commit_str(), "가/");
    assert_eq!(engine.input_category(), InputCategory::English);
}

/// **회귀 방지 핵심**: 세벌식390 + `char:/` + KeyCode::G + Shift → '/' commit + 영문 전환
#[test]
fn test_auto_english_character_3bul390_slash() {
    let (mut engine, config) =
        make_engine_with_layout_and_triggers("ko_3bul390", vec!["char:/"]);
    let shift = ModifierState {
        shift: true,
        ..Default::default()
    };

    engine.set_input_category(InputCategory::Korean);

    // 세벌식390 에서 Shift+G = '/' 산출. 한국어 모드에서 트리거 발동해야 함.
    let result = engine.press_key(KeyCode::G, shift, &config);
    assert!(result.consumed, "Shift+G='/'는 IME가 소비하여 commit");
    assert!(result.commit_changed);
    assert!(
        engine.commit_str().ends_with('/'),
        "committed='{}'",
        engine.commit_str()
    );
    assert_eq!(engine.input_category(), InputCategory::English);
}

/// 세벌식390 + `char:/` + KeyCode::G (no shift, lower='ㅡ') → 트리거 발동 안 함
#[test]
fn test_auto_english_character_3bul390_no_unwanted_trigger() {
    let (mut engine, config) =
        make_engine_with_layout_and_triggers("ko_3bul390", vec!["char:/"]);
    let modifier = ModifierState::default();

    engine.set_input_category(InputCategory::Korean);

    // shift 없이 G → 'ㅡ' 산출. 트리거 발동 X, 한글 모드 유지.
    let _ = engine.press_key(KeyCode::G, modifier, &config);
    assert_eq!(engine.input_category(), InputCategory::Korean);
}

/// 세벌식390 + `key:Escape` (Functional) → 레이아웃 무관 동작
#[test]
fn test_auto_english_functional_unaffected_by_layout() {
    let (mut engine, config) =
        make_engine_with_layout_and_triggers("ko_3bul390", vec!["key:Escape"]);
    let modifier = ModifierState::default();

    engine.set_input_category(InputCategory::Korean);
    engine.press_key(KeyCode::K, modifier, &config); // 세벌식390 'k' = 'ㄱ' (lower 3rd[7])

    let result = engine.press_key(KeyCode::Escape, modifier, &config);
    assert!(result.commit_changed, "조합이 커밋되어야 함");
    assert!(!result.consumed, "ESC 는 passthrough");
    assert_eq!(engine.input_category(), InputCategory::English);
}

/// 기본값(`key:Escape`, `char:/`) + 세벌식390 + Shift+G → 발동 (기본값 해방 검증)
#[test]
fn test_auto_english_default_works_on_3bul390() {
    let mut config = Config::default();
    config.engine.korean.layout = "ko_3bul390".into();
    config.engine.auto_english.enabled = true;
    // trigger_keys 는 기본값 그대로 — 명시 변경 없음
    let mut engine = InputEngine::new(&config);

    let shift = ModifierState {
        shift: true,
        ..Default::default()
    };
    engine.set_input_category(InputCategory::Korean);

    let result = engine.press_key(KeyCode::G, shift, &config);
    assert!(result.consumed, "기본값 char:/ 가 세벌식390 의 Shift+G 를 잡아야 함");
    assert!(result.commit_changed);
    assert!(
        engine.commit_str().ends_with('/'),
        "committed='{}'",
        engine.commit_str()
    );
    assert_eq!(engine.input_category(), InputCategory::English);
}

/// QWERTY + `char:?` + Shift+Slash → '?' commit + 영문 전환
#[test]
fn test_auto_english_character_question_mark() {
    let (mut engine, config) = make_engine_with_auto_english(vec!["char:?"]);
    let shift = ModifierState {
        shift: true,
        ..Default::default()
    };

    engine.set_input_category(InputCategory::Korean);

    let result = engine.press_key(KeyCode::Slash, shift, &config);
    assert!(result.consumed);
    assert!(result.commit_changed);
    assert!(
        engine.commit_str().ends_with('?'),
        "committed='{}'",
        engine.commit_str()
    );
    assert_eq!(engine.input_category(), InputCategory::English);
}

/// parse_trigger_key — legacy 무접두사 호환 (Functional 로 흡수)
#[test]
fn test_parse_trigger_key_legacy_compat() {
    // 제어 키: shift 무관
    assert_eq!(
        InputEngine::parse_trigger_key("Escape"),
        Some(AutoEnglishTrigger::Functional {
            code: KeyCode::Escape,
            shift: None
        })
    );
    assert_eq!(
        InputEngine::parse_trigger_key("Tab"),
        Some(AutoEnglishTrigger::Functional {
            code: KeyCode::Tab,
            shift: None
        })
    );
    // 문자 키: shift 없어야 함
    assert_eq!(
        InputEngine::parse_trigger_key("Slash"),
        Some(AutoEnglishTrigger::Functional {
            code: KeyCode::Slash,
            shift: Some(false)
        })
    );
    assert_eq!(
        InputEngine::parse_trigger_key("Semicolon"),
        Some(AutoEnglishTrigger::Functional {
            code: KeyCode::Semicolon,
            shift: Some(false)
        })
    );
    // Shift 조합: shift 필수
    assert_eq!(
        InputEngine::parse_trigger_key("ShiftSemicolon"),
        Some(AutoEnglishTrigger::Functional {
            code: KeyCode::Semicolon,
            shift: Some(true)
        })
    );
    assert_eq!(
        InputEngine::parse_trigger_key("ShiftSlash"),
        Some(AutoEnglishTrigger::Functional {
            code: KeyCode::Slash,
            shift: Some(true)
        })
    );
    // 알 수 없는 이름
    assert_eq!(InputEngine::parse_trigger_key("Nonsense"), None);
    assert_eq!(InputEngine::parse_trigger_key("ShiftNonsense"), None);
}

/// parse_trigger_key — `key:` 접두사 (Functional 명시)
#[test]
fn test_parse_trigger_key_functional_prefix() {
    assert_eq!(
        InputEngine::parse_trigger_key("key:Escape"),
        Some(AutoEnglishTrigger::Functional {
            code: KeyCode::Escape,
            shift: None
        })
    );
    assert_eq!(
        InputEngine::parse_trigger_key("key:Tab"),
        Some(AutoEnglishTrigger::Functional {
            code: KeyCode::Tab,
            shift: None
        })
    );
    assert_eq!(
        InputEngine::parse_trigger_key("key:F1"),
        Some(AutoEnglishTrigger::Functional {
            code: KeyCode::F1,
            shift: None
        })
    );
    assert_eq!(
        InputEngine::parse_trigger_key("key:ShiftSemicolon"),
        Some(AutoEnglishTrigger::Functional {
            code: KeyCode::Semicolon,
            shift: Some(true)
        })
    );
    // 알 수 없는 이름은 None (접두사가 있어도)
    assert_eq!(InputEngine::parse_trigger_key("key:Nonsense"), None);
}

/// parse_trigger_key — `char:` 접두사 (Character)
#[test]
fn test_parse_trigger_key_character_prefix() {
    assert_eq!(
        InputEngine::parse_trigger_key("char:/"),
        Some(AutoEnglishTrigger::Character('/'))
    );
    assert_eq!(
        InputEngine::parse_trigger_key("char:,"),
        Some(AutoEnglishTrigger::Character(','))
    );
    assert_eq!(
        InputEngine::parse_trigger_key("char:?"),
        Some(AutoEnglishTrigger::Character('?'))
    );
    assert_eq!(
        InputEngine::parse_trigger_key("char::"),
        Some(AutoEnglishTrigger::Character(':'))
    );
    // 빈 char: 는 None
    assert_eq!(InputEngine::parse_trigger_key("char:"), None);
}
