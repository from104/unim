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

/// **회귀 방지**: 세벌식390 + `slash_context_alt` 활성 + `char:/` + 빈 preedit + Slash 키
/// → context_alt fallback `/` 가 산출되므로 트리거 발동 + 영문 전환 + '/' commit.
#[test]
fn test_auto_english_3bul390_slash_key_empty_preedit_triggers() {
    let (mut engine, config) =
        make_engine_with_layout_and_triggers("ko_3bul390", vec!["char:/"]);
    let modifier = ModifierState::default();

    engine.set_input_category(InputCategory::Korean);

    // 빈 preedit + Slash → slash_context_alt 의 fallback '/' 가 발동.
    let result = engine.press_key(KeyCode::Slash, modifier, &config);
    assert!(result.consumed, "fallback '/' 산출 → 트리거 소비");
    assert!(result.commit_changed);
    assert!(
        engine.commit_str().ends_with('/'),
        "committed='{}'",
        engine.commit_str()
    );
    assert_eq!(engine.input_category(), InputCategory::English);
}

/// **회귀 방지**: 세벌식390 + `slash_context_alt` 활성 + `char:/` + 초성-only(ㄱ) + Slash 키
/// → context_alt 조건 충족 → ㅗ 자모 경로 → 트리거 미발동 → "고" 합성 + 한글 모드 유지.
#[test]
fn test_auto_english_3bul390_slash_key_choseong_only_no_trigger() {
    let (mut engine, config) =
        make_engine_with_layout_and_triggers("ko_3bul390", vec!["char:/"]);
    let modifier = ModifierState::default();

    engine.set_input_category(InputCategory::Korean);
    // 세벌식390 'k' 키 → ㄱ (초성-only)
    engine.press_key(KeyCode::K, modifier, &config);

    let result = engine.press_key(KeyCode::Slash, modifier, &config);
    // 자모(ㅗ) 합성 경로 → preedit 업데이트, commit 비어있음, 한글 모드 유지.
    assert!(result.consumed);
    assert!(
        engine.commit_str().is_empty(),
        "commit='{}'",
        engine.commit_str()
    );
    assert_eq!(engine.preedit_str(), "고");
    assert_eq!(engine.input_category(), InputCategory::Korean);
}

// === Functional 트리거 + slash_context_alt 가드 회귀 (UNIM-TSF-AUTO-ENGLISH-SLASH-CONTEXT-ALT-CONFLICT) ===
//
// legacy 무접두사 `trigger_keys: [Slash]` 는 `Functional { Slash, Some(false) }` 로 파싱된다.
// Functional 분기에 `produced_char` 가드가 없으면 세벌식390 초성-only 컨텍스트에서 '/' 키가
// slash_context_alt(ㅗ 자모 경로) 보다 먼저 트리거를 선점해 '되' 가 'ㄷ/d' 로 깨졌다.

/// **회귀 핵심**: 세벌식390 + Functional `Slash` 트리거 + 'u','/','d' → '되'.
/// 초성-only(ㄷ) 컨텍스트에서 '/' 키는 Functional Slash 트리거를 양보하고 ㅗ 자모 경로를 타야 한다.
#[test]
fn test_auto_english_functional_slash_3bul390_does_not_preempt_doe() {
    let (mut engine, config) =
        make_engine_with_layout_and_triggers("ko_3bul390", vec!["Slash"]);
    let modifier = ModifierState::default();

    engine.set_input_category(InputCategory::Korean);

    // 세벌식390: u → ㄷ(초성), / → ㅗ(slash_context_alt, ㅗ+ㅣ=ㅚ), d → ㅣ(중성)
    engine.press_key(KeyCode::U, modifier, &config);
    let slash_result = engine.press_key(KeyCode::Slash, modifier, &config);
    // '/' 는 자모(ㅗ) 경로 → IME 소비, commit 없음, 한글 모드 유지.
    assert!(slash_result.consumed, "'/'는 자모 경로로 소비되어야 함");
    assert!(
        engine.commit_str().is_empty(),
        "Functional Slash 트리거가 선점하면 안 됨, commit='{}'",
        engine.commit_str()
    );
    assert_eq!(
        engine.input_category(),
        InputCategory::Korean,
        "자모 경로이므로 한글 모드 유지"
    );

    engine.press_key(KeyCode::D, modifier, &config);

    assert_eq!(engine.preedit_str(), "되");
    assert!(
        engine.commit_str().is_empty(),
        "조합 중 commit 없어야 함, commit='{}'",
        engine.commit_str()
    );
    assert_eq!(engine.input_category(), InputCategory::Korean);
}

/// 세벌식390 + Functional `Slash` 트리거 + 빈 preedit + '/' → fallback '/' 산출 →
/// Functional 트리거 정상 발동(영문 전환 + '/' commit). 가드가 평문 컨텍스트를 막지 않음을 검증.
#[test]
fn test_auto_english_functional_slash_3bul390_empty_preedit_triggers() {
    let (mut engine, config) =
        make_engine_with_layout_and_triggers("ko_3bul390", vec!["Slash"]);
    let modifier = ModifierState::default();

    engine.set_input_category(InputCategory::Korean);

    // 빈 preedit → slash_context_alt 조건 불충족 → fallback '/' 산출 → 트리거 발동.
    let result = engine.press_key(KeyCode::Slash, modifier, &config);
    assert!(result.consumed, "fallback '/' 산출 → 트리거 소비");
    assert!(result.commit_changed);
    assert!(
        engine.commit_str().ends_with('/'),
        "committed='{}'",
        engine.commit_str()
    );
    assert_eq!(engine.input_category(), InputCategory::English);
}

/// 평문(QWERTY) + Functional `Slash` 트리거 + '/' → 영문 전환 + '/' commit.
/// 평문 자판에는 context_alt 가 없으므로 '/' 는 항상 char 를 산출 → 가드 통과 정상 발동.
#[test]
fn test_auto_english_functional_slash_qwerty_triggers() {
    let (mut engine, config) =
        make_engine_with_layout_and_triggers("ko_2bulstd", vec!["Slash"]);
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

/// 비문자 제어키(Escape) Functional 트리거는 `is_character_key()=false` 라 가드를 우회 →
/// 세벌식390 조합 중에도 정상 발동. produced_char.is_some() 만 요구하면 깨지는 케이스 보호.
#[test]
fn test_auto_english_functional_escape_bypasses_char_guard() {
    let (mut engine, config) =
        make_engine_with_layout_and_triggers("ko_3bul390", vec!["Escape"]);
    let modifier = ModifierState::default();

    engine.set_input_category(InputCategory::Korean);
    engine.press_key(KeyCode::U, modifier, &config); // ㄷ(초성)

    // Escape 는 english_keymap.get_char 가 None → produced_char=None 이지만
    // is_character_key()=false 라 가드 우회 → 트리거 정상 발동.
    let result = engine.press_key(KeyCode::Escape, modifier, &config);
    assert!(result.commit_changed, "조합이 커밋되어야 함");
    assert!(!result.consumed, "Escape 는 passthrough");
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

/// 비조합(legacy·`key:` 단일·`char:`) 트리거의 기대 Functional 리터럴 헬퍼.
/// modifier(ctrl/alt/super)는 전부 false — "이 트리거는 조합이 아님" 을 명시한다.
fn func_plain(code: KeyCode, shift: Option<bool>) -> AutoEnglishTrigger {
    AutoEnglishTrigger::Functional {
        code,
        shift,
        ctrl: false,
        alt: false,
        super_key: false,
    }
}

/// parse_trigger_key — legacy 무접두사 호환 (Functional 로 흡수, modifier 전부 false)
#[test]
fn test_parse_trigger_key_legacy_compat() {
    // 제어 키: shift 무관
    assert_eq!(
        InputEngine::parse_trigger_key("Escape"),
        Some(func_plain(KeyCode::Escape, None))
    );
    assert_eq!(
        InputEngine::parse_trigger_key("Tab"),
        Some(func_plain(KeyCode::Tab, None))
    );
    // 문자 키: shift 없어야 함
    assert_eq!(
        InputEngine::parse_trigger_key("Slash"),
        Some(func_plain(KeyCode::Slash, Some(false)))
    );
    assert_eq!(
        InputEngine::parse_trigger_key("Semicolon"),
        Some(func_plain(KeyCode::Semicolon, Some(false)))
    );
    // Shift 조합: shift 필수
    assert_eq!(
        InputEngine::parse_trigger_key("ShiftSemicolon"),
        Some(func_plain(KeyCode::Semicolon, Some(true)))
    );
    assert_eq!(
        InputEngine::parse_trigger_key("ShiftSlash"),
        Some(func_plain(KeyCode::Slash, Some(true)))
    );
    // 알 수 없는 이름
    assert_eq!(InputEngine::parse_trigger_key("Nonsense"), None);
    assert_eq!(InputEngine::parse_trigger_key("ShiftNonsense"), None);
    // legacy 문법에는 '+' 조합이 없다 — 무접두사 "Ctrl+B" 는 KeyCode 이름이 아니므로 None.
    assert_eq!(InputEngine::parse_trigger_key("Ctrl+B"), None);
}

/// parse_trigger_key — `key:` 접두사 단일 표기 (modifier 전부 false, 종전 파싱 보존)
#[test]
fn test_parse_trigger_key_functional_prefix() {
    assert_eq!(
        InputEngine::parse_trigger_key("key:Escape"),
        Some(func_plain(KeyCode::Escape, None))
    );
    assert_eq!(
        InputEngine::parse_trigger_key("key:Tab"),
        Some(func_plain(KeyCode::Tab, None))
    );
    assert_eq!(
        InputEngine::parse_trigger_key("key:F1"),
        Some(func_plain(KeyCode::F1, None))
    );
    assert_eq!(
        InputEngine::parse_trigger_key("key:ShiftSemicolon"),
        Some(func_plain(KeyCode::Semicolon, Some(true)))
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

// === Ctrl/Alt/Super 조합 트리거 (key:Ctrl+B 류) 파서 ===

/// parse_trigger_key — `key:` modifier 조합 파싱. 기대값은 표기 문자열과 독립적으로 단언.
#[test]
fn test_parse_trigger_key_modifier_combo() {
    // key:Ctrl+B → Functional{B, Some(false)(문자키), ctrl}
    assert_eq!(
        InputEngine::parse_trigger_key("key:Ctrl+B"),
        Some(AutoEnglishTrigger::Functional {
            code: KeyCode::B,
            shift: Some(false),
            ctrl: true,
            alt: false,
            super_key: false,
        })
    );
    // key:Alt+F1 → Functional{F1, None(제어키), alt}
    assert_eq!(
        InputEngine::parse_trigger_key("key:Alt+F1"),
        Some(AutoEnglishTrigger::Functional {
            code: KeyCode::F1,
            shift: None,
            ctrl: false,
            alt: true,
            super_key: false,
        })
    );
    // key:Super+Space → Functional{Space, Some(false), super}
    assert_eq!(
        InputEngine::parse_trigger_key("key:Super+Space"),
        Some(AutoEnglishTrigger::Functional {
            code: KeyCode::Space,
            shift: Some(false),
            ctrl: false,
            alt: false,
            super_key: true,
        })
    );
    // key:Ctrl+Shift+B → Functional{B, Some(true)(shift 강제), ctrl}
    assert_eq!(
        InputEngine::parse_trigger_key("key:Ctrl+Shift+B"),
        Some(AutoEnglishTrigger::Functional {
            code: KeyCode::B,
            shift: Some(true),
            ctrl: true,
            alt: false,
            super_key: false,
        })
    );
}

/// parse_trigger_key — modifier 관용: 대소문자·순서·별칭·공백·중복.
#[test]
fn test_parse_trigger_key_modifier_lenient() {
    let ctrl_b = AutoEnglishTrigger::Functional {
        code: KeyCode::B,
        shift: Some(false),
        ctrl: true,
        alt: false,
        super_key: false,
    };
    // modifier 대소문자 무관 (base 'B' 는 대문자 유지).
    assert_eq!(InputEngine::parse_trigger_key("key:ctrl+B"), Some(ctrl_b));
    assert_eq!(InputEngine::parse_trigger_key("key:CTRL+B"), Some(ctrl_b));
    // 별칭: control == ctrl.
    assert_eq!(InputEngine::parse_trigger_key("key:control+B"), Some(ctrl_b));
    // 토큰 공백 trim.
    assert_eq!(InputEngine::parse_trigger_key("key: Ctrl + B"), Some(ctrl_b));
    // 중복 토큰 멱등.
    assert_eq!(InputEngine::parse_trigger_key("key:Ctrl+Ctrl+B"), Some(ctrl_b));

    // super 별칭: win / meta / super 모두 super_key.
    let super_b = AutoEnglishTrigger::Functional {
        code: KeyCode::B,
        shift: Some(false),
        ctrl: false,
        alt: false,
        super_key: true,
    };
    assert_eq!(InputEngine::parse_trigger_key("key:super+B"), Some(super_b));
    assert_eq!(InputEngine::parse_trigger_key("key:win+B"), Some(super_b));
    assert_eq!(InputEngine::parse_trigger_key("key:meta+B"), Some(super_b));

    // 순서 무관: Shift+Ctrl+B == Ctrl+Shift+B (둘 다 명시 기대값과 일치).
    let ctrl_shift_b = AutoEnglishTrigger::Functional {
        code: KeyCode::B,
        shift: Some(true),
        ctrl: true,
        alt: false,
        super_key: false,
    };
    assert_eq!(
        InputEngine::parse_trigger_key("key:Shift+Ctrl+B"),
        Some(ctrl_shift_b)
    );
    assert_eq!(
        InputEngine::parse_trigger_key("key:Ctrl+Shift+B"),
        Some(ctrl_shift_b)
    );

    // base 는 대소문자 구분 — 소문자 base 'b' 는 KeyCode 미지 → None (관용 비대칭 방지).
    assert_eq!(InputEngine::parse_trigger_key("key:CTRL+b"), None);
}

/// parse_trigger_key — 비정상 조합은 현행 정책대로 침묵 무시(None).
#[test]
fn test_parse_trigger_key_modifier_invalid() {
    assert_eq!(InputEngine::parse_trigger_key("key:Foo+B"), None); // 미지 modifier
    assert_eq!(InputEngine::parse_trigger_key("key:Ctrl+Nonsense"), None); // 미지 base
    assert_eq!(InputEngine::parse_trigger_key("key:Ctrl+"), None); // 빈 base
    assert_eq!(InputEngine::parse_trigger_key("key:Ctrl"), None); // 'Ctrl' 자체는 from_name 미지
}

/// parse_trigger_key — 단일 base(비조합)는 trim 하지 않아 내부 공백 표기가 종전대로
/// 침묵 무시된다(하위호환). 공백 관용은 `'+'` 조합 표기 전용.
#[test]
fn test_parse_trigger_key_single_base_no_trim() {
    let escape = AutoEnglishTrigger::Functional {
        code: KeyCode::Escape,
        shift: None,
        ctrl: false,
        alt: false,
        super_key: false,
    };
    // 정상 단일 표기는 종전대로 파싱.
    assert_eq!(InputEngine::parse_trigger_key("key:Escape"), Some(escape));
    // 내부/후행 공백은 종전(피처 이전)과 동일하게 None — trim 되살아나지 않음.
    assert_eq!(InputEngine::parse_trigger_key("key: Escape"), None);
    assert_eq!(InputEngine::parse_trigger_key("key:Escape "), None);
    // 반면 '+' 조합 표기는 토큰 공백을 trim 한다(관용 — 별도 정책).
    assert_eq!(
        InputEngine::parse_trigger_key("key: Ctrl + Escape"),
        Some(AutoEnglishTrigger::Functional {
            code: KeyCode::Escape,
            shift: None,
            ctrl: true,
            alt: false,
            super_key: false,
        })
    );
}

// === Ctrl/Alt/Super 조합 트리거 발동/정확일치/하위호환 (엔진 press flow) ===

/// idle 한글 모드 + `key:Ctrl+B` → 영문 전환 + 키 passthrough + 'b' commit 없음.
#[test]
fn test_auto_english_ctrl_b_idle_passthrough() {
    let (mut engine, config) = make_engine_with_auto_english(vec!["key:Ctrl+B"]);
    engine.set_input_category(InputCategory::Korean);

    let ctrl = ModifierState {
        control: true,
        ..Default::default()
    };
    let result = engine.press_key(KeyCode::B, ctrl, &config);
    assert!(!result.consumed, "Ctrl+B 는 앱으로 통과해야 함 (tmux prefix)");
    assert!(!result.commit_changed, "idle 이므로 commit 없음");
    assert!(
        engine.commit_str().is_empty(),
        "'b' 가 commit 되면 안 됨: '{}'",
        engine.commit_str()
    );
    assert_eq!(engine.input_category(), InputCategory::English);
}

/// 조합 중 + `key:Ctrl+B` → 직전 조합만 flush + 영문 전환 + passthrough ('b' 미유입).
#[test]
fn test_auto_english_ctrl_b_composing_flush_passthrough() {
    let (mut engine, config) = make_engine_with_auto_english(vec!["key:Ctrl+B"]);
    engine.set_input_category(InputCategory::Korean);

    let none = ModifierState::default();
    engine.press_key(KeyCode::R, none, &config); // ㄱ
    engine.press_key(KeyCode::K, none, &config); // 가

    let ctrl = ModifierState {
        control: true,
        ..Default::default()
    };
    let result = engine.press_key(KeyCode::B, ctrl, &config);
    assert!(!result.consumed, "Ctrl+B passthrough (tmux prefix)");
    assert!(result.commit_changed, "조합 '가' 가 flush 되어야 함");
    assert_eq!(engine.commit_str(), "가", "flush 는 '가' 만 — 'b' 유입 금지");
    assert_eq!(engine.input_category(), InputCategory::English);
}

/// 정확 일치: `key:Ctrl+B` 등록 시 plain B 는 자모 조합, Ctrl+Alt+B/Ctrl+Shift+B 는 미발동.
#[test]
fn test_auto_english_ctrl_b_exact_match_only() {
    // plain B → 자모(ㅠ) 조합, 전환 없음.
    let (mut engine, config) = make_engine_with_auto_english(vec!["key:Ctrl+B"]);
    engine.set_input_category(InputCategory::Korean);
    let r = engine.press_key(KeyCode::B, ModifierState::default(), &config);
    assert!(r.consumed, "plain B 는 자모로 소비");
    assert_eq!(engine.preedit_str(), "ㅠ");
    assert_eq!(engine.input_category(), InputCategory::Korean);

    // Ctrl+Alt+B → alt 불일치 → 미발동 (기존 가드 통과).
    let (mut engine, config) = make_engine_with_auto_english(vec!["key:Ctrl+B"]);
    engine.set_input_category(InputCategory::Korean);
    let r = engine.press_key(
        KeyCode::B,
        ModifierState {
            control: true,
            alt: true,
            ..Default::default()
        },
        &config,
    );
    assert!(!r.consumed);
    assert_eq!(engine.input_category(), InputCategory::Korean);

    // Ctrl+Shift+B → shift 불일치 → 미발동.
    let (mut engine, config) = make_engine_with_auto_english(vec!["key:Ctrl+B"]);
    engine.set_input_category(InputCategory::Korean);
    let r = engine.press_key(
        KeyCode::B,
        ModifierState {
            control: true,
            shift: true,
            ..Default::default()
        },
        &config,
    );
    assert!(!r.consumed);
    assert_eq!(engine.input_category(), InputCategory::Korean);
}

/// char 가드 우회: 두벌식 B=ㅠ(자모, produces_char None)여도 Ctrl+B 는 발동한다.
#[test]
fn test_auto_english_ctrl_b_char_guard_bypass() {
    // 먼저 plain B 가 자모 ㅠ 를 산출함을 확인 → produces_char None 케이스임을 고정.
    let (mut engine, config) = make_engine_with_auto_english(vec!["key:Ctrl+B"]);
    engine.set_input_category(InputCategory::Korean);
    engine.press_key(KeyCode::B, ModifierState::default(), &config);
    assert_eq!(engine.preedit_str(), "ㅠ", "두벌식 B 는 자모 ㅠ");

    // 새 엔진에서 Ctrl+B → 자모 산출과 무관하게 트리거 발동.
    let (mut engine, config) = make_engine_with_auto_english(vec!["key:Ctrl+B"]);
    engine.set_input_category(InputCategory::Korean);
    let r = engine.press_key(
        KeyCode::B,
        ModifierState {
            control: true,
            ..Default::default()
        },
        &config,
    );
    assert!(!r.consumed);
    assert_eq!(engine.input_category(), InputCategory::English);
}

/// `key:Ctrl+Shift+B` 등록: Ctrl+Shift+B 만 발동, Ctrl+B 는 미발동(shift 정확 일치).
#[test]
fn test_auto_english_ctrl_shift_b_registered_exact() {
    // Ctrl+Shift+B → 발동.
    let (mut engine, config) = make_engine_with_auto_english(vec!["key:Ctrl+Shift+B"]);
    engine.set_input_category(InputCategory::Korean);
    let r = engine.press_key(
        KeyCode::B,
        ModifierState {
            control: true,
            shift: true,
            ..Default::default()
        },
        &config,
    );
    assert!(!r.consumed);
    assert_eq!(engine.input_category(), InputCategory::English);

    // Ctrl+B (shift 없음) → 미발동.
    let (mut engine, config) = make_engine_with_auto_english(vec!["key:Ctrl+Shift+B"]);
    engine.set_input_category(InputCategory::Korean);
    let r = engine.press_key(
        KeyCode::B,
        ModifierState {
            control: true,
            ..Default::default()
        },
        &config,
    );
    assert!(!r.consumed);
    assert_eq!(engine.input_category(), InputCategory::Korean);
}

/// 하위호환 pin: legacy `key:Escape` 는 Ctrl+Escape 에 발동하지 않고, 기존 가드
/// 반환값(idle=not_consumed, 조합 중=consumed committed)이 비트 그대로 유지된다.
#[test]
fn test_auto_english_legacy_escape_no_trigger_on_ctrl_escape() {
    // idle: Ctrl+Escape → 전환 없음, 기존 가드 not_consumed.
    let (mut engine, config) = make_engine_with_auto_english(vec!["key:Escape"]);
    engine.set_input_category(InputCategory::Korean);
    let r = engine.press_key(
        KeyCode::Escape,
        ModifierState {
            control: true,
            ..Default::default()
        },
        &config,
    );
    assert!(!r.consumed);
    assert!(!r.commit_changed);
    assert_eq!(engine.input_category(), InputCategory::Korean);

    // 조합 중: Ctrl+Escape → 기존 가드대로 소비(committed), 전환 없음.
    let (mut engine, config) = make_engine_with_auto_english(vec!["key:Escape"]);
    engine.set_input_category(InputCategory::Korean);
    engine.press_key(KeyCode::R, ModifierState::default(), &config); // ㄱ
    let r = engine.press_key(
        KeyCode::Escape,
        ModifierState {
            control: true,
            ..Default::default()
        },
        &config,
    );
    assert!(r.consumed, "조합 중 Ctrl+Escape 는 기존 가드대로 소비");
    assert!(r.commit_changed);
    assert_eq!(
        engine.input_category(),
        InputCategory::Korean,
        "legacy Escape 는 조합 modifier 에서 발동하지 않음"
    );
}

/// 하위호환 pin: `char:/` 는 Ctrl+Slash 에 발동하지 않는다(Character 는 비조합 전용).
#[test]
fn test_auto_english_char_slash_no_trigger_on_ctrl_slash() {
    let (mut engine, config) = make_engine_with_auto_english(vec!["char:/"]);
    engine.set_input_category(InputCategory::Korean);
    let r = engine.press_key(
        KeyCode::Slash,
        ModifierState {
            control: true,
            ..Default::default()
        },
        &config,
    );
    assert!(!r.consumed, "Ctrl+/ 는 char:/ 를 발동시키지 않음");
    assert_eq!(engine.input_category(), InputCategory::Korean);
}

/// 영문 모드에서는 조합 트리거도 no-op (매처가 한글 모드에서만 평가).
#[test]
fn test_auto_english_ctrl_b_noop_in_english_mode() {
    let (mut engine, config) = make_engine_with_auto_english(vec!["key:Ctrl+B"]);
    assert_eq!(engine.input_category(), InputCategory::English);
    let r = engine.press_key(
        KeyCode::B,
        ModifierState {
            control: true,
            ..Default::default()
        },
        &config,
    );
    assert!(!r.consumed);
    assert_eq!(engine.input_category(), InputCategory::English);
}

/// Super+Space, Alt+F1 조합 트리거도 발동 + passthrough (XIM/코어 경로 기준).
#[test]
fn test_auto_english_super_space_and_alt_f1() {
    // Super+Space → 발동.
    let (mut engine, config) = make_engine_with_auto_english(vec!["key:Super+Space"]);
    engine.set_input_category(InputCategory::Korean);
    let r = engine.press_key(
        KeyCode::Space,
        ModifierState {
            super_key: true,
            ..Default::default()
        },
        &config,
    );
    assert!(!r.consumed, "Super+Space passthrough");
    assert_eq!(engine.input_category(), InputCategory::English);

    // Alt+F1 → 발동.
    let (mut engine, config) = make_engine_with_auto_english(vec!["key:Alt+F1"]);
    engine.set_input_category(InputCategory::Korean);
    let r = engine.press_key(
        KeyCode::F1,
        ModifierState {
            alt: true,
            ..Default::default()
        },
        &config,
    );
    assert!(!r.consumed, "Alt+F1 passthrough");
    assert_eq!(engine.input_category(), InputCategory::English);
}

// ─────────────────────────────────────────────────────────────────────────────
// `try_auto_english_combo` — TSF `OnTestKeyDown` 투기적 호출용 멱등 헬퍼 (D-1)
//
// (조합 트리거 매칭/비매칭) × (조합 중/유휴) 4조합. 실 press flow(`press_key`)와
// 달리 이 헬퍼는 preedit/commit_buffer 를 절대 건드리지 않는다 — "멱등"은 반환값이
// 아니라 **관측 가능한 엔진 상태**(input_category, preedit, commit_buffer) 기준으로
// 검증한다: `match_auto_english_trigger` 자체가 input_category==Korean 에서만
// 매칭하므로, 카테고리가 이미 English 로 바뀐 뒤의 2번째 호출은 반환값이 false 로
// 바뀌는 게 정상이다(더 옮길 카테고리가 없다는 뜻) — 그래도 상태는 1번 호출 후와
// 완전히 동일해야 진짜 멱등이다.
// ─────────────────────────────────────────────────────────────────────────────

fn ctrl_b() -> (KeyCode, ModifierState) {
    (
        KeyCode::B,
        ModifierState {
            control: true,
            ..Default::default()
        },
    )
}

/// 등록되지 않은 조합 (Ctrl+Z) — 매칭 실패 판정용.
fn ctrl_z() -> (KeyCode, ModifierState) {
    (
        KeyCode::Z,
        ModifierState {
            control: true,
            ..Default::default()
        },
    )
}

/// 매칭 + 유휴: 카테고리가 English 로 전환되고, 연속 2회 호출해도 상태가 동일하다
/// (2회차는 반환값만 false — 이미 English 라 `match_auto_english_trigger` 가
/// Korean 전용 가드에 걸려 재매칭하지 않기 때문. 상태 자체는 불변).
#[test]
fn test_try_auto_english_combo_matched_idle() {
    let (mut engine, _config) = make_engine_with_auto_english(vec!["key:Ctrl+B"]);
    engine.set_input_category(InputCategory::Korean);
    let (kc, m) = ctrl_b();

    let first = engine.try_auto_english_combo(kc, m);
    assert!(first, "등록된 조합은 1회차에 매칭돼야 함");
    assert_eq!(engine.input_category(), InputCategory::English);
    assert!(engine.preedit_str().is_empty(), "유휴라 preedit 없음");
    assert!(engine.commit_str().is_empty(), "커밋 부작용 없어야 함");

    let second = engine.try_auto_english_combo(kc, m);
    assert!(!second, "이미 English 라 2회차는 더 옮길 카테고리가 없음");
    assert_eq!(
        engine.input_category(),
        InputCategory::English,
        "멱등: 상태는 1회차 이후와 동일해야 함"
    );
    assert!(engine.preedit_str().is_empty());
    assert!(engine.commit_str().is_empty(), "2회차도 커밋 부작용 없어야 함");
}

/// 매칭 + 조합 중: 카테고리는 전환되지만, **조합 중이던 preedit 는 그대로 남는다**
/// (flush/commit 은 이 헬퍼의 책임이 아니라 실 경로(press_key)로 미룸이 D-1 계약).
/// 연속 2회 호출해도 preedit·commit_buffer 가 추가로 건드려지지 않아야 진짜 멱등.
#[test]
fn test_try_auto_english_combo_matched_composing() {
    let (mut engine, config) = make_engine_with_auto_english(vec!["key:Ctrl+B"]);
    engine.set_input_category(InputCategory::Korean);
    let none = ModifierState::default();
    engine.press_key(KeyCode::R, none, &config); // ㄱ
    engine.press_key(KeyCode::K, none, &config); // 가
    assert_eq!(engine.preedit_str(), "가", "조합 중 상태 고정");

    let (kc, m) = ctrl_b();
    let first = engine.try_auto_english_combo(kc, m);
    assert!(first, "조합 중이어도 등록된 조합은 매칭돼야 함");
    assert_eq!(engine.input_category(), InputCategory::English);
    assert_eq!(
        engine.preedit_str(),
        "가",
        "커밋 부작용 없음 — flush 는 실 경로 몫"
    );
    assert!(
        engine.commit_str().is_empty(),
        "commit_buffer 에 아무 것도 쌓이면 안 됨(투기적 호출이 drain 안 됨)"
    );

    let second = engine.try_auto_english_combo(kc, m);
    assert!(!second, "이미 English 라 2회차는 더 옮길 카테고리가 없음");
    assert_eq!(engine.input_category(), InputCategory::English);
    assert_eq!(engine.preedit_str(), "가", "멱등: 2회차도 조합 그대로 보존");
    assert!(engine.commit_str().is_empty());
}

/// 비매칭 + 유휴: 등록 안 된 조합(Ctrl+Z)은 카테고리를 건드리지 않는다. 2회 호출해도
/// 상태 불변(반환값도 매번 false — 매칭 자체가 안 되니 상태 변화가 있을 수 없음).
#[test]
fn test_try_auto_english_combo_unmatched_idle() {
    let (mut engine, _config) = make_engine_with_auto_english(vec!["key:Ctrl+B"]);
    engine.set_input_category(InputCategory::Korean);
    let (kc, m) = ctrl_z();

    assert!(!engine.try_auto_english_combo(kc, m));
    assert_eq!(engine.input_category(), InputCategory::Korean);
    assert!(!engine.try_auto_english_combo(kc, m), "멱등: 2회차도 false");
    assert_eq!(engine.input_category(), InputCategory::Korean);
}

/// 비매칭 + 조합 중: 등록 안 된 조합은 진행 중인 한글 조합도, 카테고리도 건드리지
/// 않는다(앱 단축키 보호 — tmux 등 무관 조합이 한글 조합을 깨면 안 됨).
#[test]
fn test_try_auto_english_combo_unmatched_composing() {
    let (mut engine, config) = make_engine_with_auto_english(vec!["key:Ctrl+B"]);
    engine.set_input_category(InputCategory::Korean);
    let none = ModifierState::default();
    engine.press_key(KeyCode::R, none, &config); // ㄱ
    engine.press_key(KeyCode::K, none, &config); // 가
    assert_eq!(engine.preedit_str(), "가");

    let (kc, m) = ctrl_z();
    assert!(!engine.try_auto_english_combo(kc, m));
    assert_eq!(engine.input_category(), InputCategory::Korean);
    assert_eq!(engine.preedit_str(), "가", "무관 조합이 조합 중 상태를 깨면 안 됨");

    assert!(!engine.try_auto_english_combo(kc, m), "멱등: 2회차도 false");
    assert_eq!(engine.input_category(), InputCategory::Korean);
    assert_eq!(engine.preedit_str(), "가");
}

// ─────────────────────────────────────────────────────────────────────────────
// `is_valid_auto_english_key` — 트리거 표기 검증 술어 (CLI·설정앱·D-Bus 공용 SoT)
// ─────────────────────────────────────────────────────────────────────────────

/// 검증 술어가 인정해야 하는 표기 (출하 기본값 + 대표 문법).
const VALID_AUTO_ENGLISH_SPECS: &[&str] = &[
    "key:Escape",
    "char:/",
    "Escape",
    "key:Ctrl+B",
    "key:ShiftSemicolon",
    "key:shift+ctrl+B",
    "key:Super+Space",
];

/// 검증 술어가 거부해야 하는 표기.
const INVALID_AUTO_ENGLISH_SPECS: &[&str] = &[
    "key:Ctrl+",
    "char:",
    "key: Escape",
    "Ctrl+B",
    "key:Ctrl-Left",
    "key:escape",
    "",
];

#[test]
fn is_valid_auto_english_key_accepts_defaults() {
    for spec in VALID_AUTO_ENGLISH_SPECS {
        assert!(
            InputEngine::is_valid_auto_english_key(spec),
            "'{spec}' 는 유효한 자동 영문 트리거 표기여야 함"
        );
    }
}

#[test]
fn is_valid_auto_english_key_rejects() {
    for spec in INVALID_AUTO_ENGLISH_SPECS {
        assert!(
            !InputEngine::is_valid_auto_english_key(spec),
            "'{spec}' 는 거부되어야 함"
        );
    }
}

#[test]
fn is_valid_auto_english_key_agrees_with_parser() {
    // 얇은 래퍼가 파서와 어긋나지 않음을 고정 — 어긋나면 GUI/CLI 가 오탐한다.
    for spec in VALID_AUTO_ENGLISH_SPECS
        .iter()
        .chain(INVALID_AUTO_ENGLISH_SPECS)
    {
        assert_eq!(
            InputEngine::is_valid_auto_english_key(spec),
            InputEngine::parse_trigger_key(spec).is_some(),
            "'{spec}' 판정이 파서와 어긋남"
        );
    }
}

#[test]
fn parse_auto_english_triggers_drops_invalid_preserving_order() {
    // 로그 부수효과가 붙어도 결과 Vec 은 종전 `filter_map` 과 동일해야 한다(I5).
    let names: Vec<String> = ["key:Escape", "key:Ctrl+", "char:/", "Ctrl+B"]
        .iter()
        .map(|s| s.to_string())
        .collect();
    let legacy: Vec<AutoEnglishTrigger> = names
        .iter()
        .filter_map(|n| InputEngine::parse_trigger_key(n))
        .collect();
    assert_eq!(InputEngine::parse_auto_english_triggers(&names), legacy);
    assert_eq!(legacy.len(), 2);
    assert_eq!(legacy[1], AutoEnglishTrigger::Character('/'));
}
