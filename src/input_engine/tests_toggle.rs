//! 한/영 전환키(`toggle_keys`) 처리 회귀 테스트.
//!
//! 핵심 회귀: RightAlt 처럼 그 자체가 수정자인 토글키는, `press_key` 의
//! `is_modifier()` 가드와 Ctrl/Alt/Super 단축키 가드가 토글 분기보다 앞에 있던
//! 종전 코드에서 항상 먼저 걸러져 **토글이 죽어 있었다**(VK_HANGUL 만 동작).
//! 이 모듈은 RightAlt 토글이 실제로 동작하고, 소비(consumed) 여부가 토글 동작과
//! 일치하며, 단축키 조합(Ctrl+RightAlt)은 토글하지 않음을 고정한다.

use super::InputEngine;
use crate::config::{Config, InputCategory};
use crate::keycode::{KeyCode, ModifierState};

/// RightAlt 는 실제 입력 시 자기 자신이 Alt 비트를 세운다.
fn alt_down() -> ModifierState {
    ModifierState {
        alt: true,
        ..ModifierState::default()
    }
}

#[test]
fn is_toggle_key_reports_default_config() {
    let config = Config::default();
    let engine = InputEngine::new(&config);
    // 기본 toggle_keys = ["Korean", "RightAlt"]
    assert!(engine.is_toggle_key(KeyCode::Korean));
    assert!(engine.is_toggle_key(KeyCode::RightAlt));
    assert!(!engine.is_toggle_key(KeyCode::A));
}

#[test]
fn right_alt_toggles_hanyeong() {
    let config = Config::default();
    let mut engine = InputEngine::new(&config);
    engine.set_input_category(InputCategory::Korean);

    // RightAlt(Alt 비트 동반) → 영문 전환 + 소비. (종전엔 not_consumed 로 토글 죽음.)
    let r = engine.press_key(KeyCode::RightAlt, alt_down(), &config);
    assert!(r.consumed, "RightAlt 토글은 소비되어야 함");
    assert_eq!(engine.input_category(), InputCategory::English);

    // 다시 누르면 한글로.
    let r2 = engine.press_key(KeyCode::RightAlt, alt_down(), &config);
    assert!(r2.consumed);
    assert_eq!(engine.input_category(), InputCategory::Korean);
}

#[test]
fn right_alt_toggles_even_without_alt_bit() {
    // 프런트엔드/플랫폼에 따라 RightAlt 누름 이벤트에서 Alt 비트가 아직 안 설정될
    // 수 있다. 그 경우에도 토글되어야 한다.
    let config = Config::default();
    let mut engine = InputEngine::new(&config);
    engine.set_input_category(InputCategory::Korean);

    let r = engine.press_key(KeyCode::RightAlt, ModifierState::default(), &config);
    assert!(r.consumed);
    assert_eq!(engine.input_category(), InputCategory::English);
}

#[test]
fn korean_key_still_toggles() {
    let config = Config::default();
    let mut engine = InputEngine::new(&config);
    engine.set_input_category(InputCategory::Korean);

    let r = engine.press_key(KeyCode::Korean, ModifierState::default(), &config);
    assert!(r.consumed);
    assert_eq!(engine.input_category(), InputCategory::English);
}

#[test]
fn ctrl_right_alt_is_shortcut_not_toggle() {
    // Ctrl+RightAlt 는 단축키로 통과해야 한다(토글 X, 소비 X).
    let config = Config::default();
    let mut engine = InputEngine::new(&config);
    engine.set_input_category(InputCategory::Korean);

    let mods = ModifierState {
        control: true,
        alt: true,
        ..ModifierState::default()
    };
    let r = engine.press_key(KeyCode::RightAlt, mods, &config);
    assert!(!r.consumed, "Ctrl+RightAlt 는 단축키 — 통과");
    assert_eq!(
        engine.input_category(),
        InputCategory::Korean,
        "모드 변화 없어야 함"
    );
}

#[test]
fn set_switch_keys_live_reloads_toggle_keys() {
    // F4 live-reload 갭 회귀: config.toggle_keys 를 바꾸고 set_switch_keys 를 호출하면
    // 살아있는 엔진이 새 키로 토글하고, 옛 키(RightAlt)는 더 이상 토글하지 않아야 한다.
    let mut config = Config::default();
    let mut engine = InputEngine::new(&config);
    engine.set_input_category(InputCategory::Korean);

    // toggle_keys 를 F8 하나로 교체 후 live 재적용.
    config.engine.toggle_keys = vec!["F8".to_string()];
    engine.set_switch_keys(&config);

    // 새 키(F8) 는 토글 + 소비.
    assert!(engine.is_toggle_key(KeyCode::F8), "새 토글키(F8) 인식");
    let r = engine.press_key(KeyCode::F8, ModifierState::default(), &config);
    assert!(r.consumed, "새 토글키(F8) 는 소비되어야 함");
    assert_eq!(engine.input_category(), InputCategory::English);

    // 옛 키(RightAlt) 는 더 이상 토글하지 않는다(무동작·비소비, 모드 불변).
    assert!(!engine.is_toggle_key(KeyCode::RightAlt), "옛 토글키 해제됨");
    let r2 = engine.press_key(KeyCode::RightAlt, alt_down(), &config);
    assert!(!r2.consumed, "옛 토글키(RightAlt) 는 무동작 — 통과");
    assert_eq!(
        engine.input_category(),
        InputCategory::English,
        "모드 불변"
    );
}

#[test]
fn right_alt_commits_composition_before_toggle() {
    // 조합 중 RightAlt → 커밋 발생 + 영문 전환.
    let config = Config::default();
    let mut engine = InputEngine::new(&config);
    engine.set_input_category(InputCategory::Korean);

    engine.press_key(KeyCode::R, ModifierState::default(), &config); // ㄱ
    engine.press_key(KeyCode::K, ModifierState::default(), &config); // 가
    assert!(engine.is_composing(), "조합 중이어야 함");

    let r = engine.press_key(KeyCode::RightAlt, alt_down(), &config);
    assert!(r.commit_changed, "조합이 커밋되어야 함");
    assert_eq!(engine.commit_str(), "가");
    assert_eq!(engine.input_category(), InputCategory::English);
}

// ─────────────────────────────────────────────────────────────────────────────
// `parse_switch_key` — 전환키/한자키 표기 검증 술어 (CLI·설정앱·D-Bus 공용 SoT)
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn parse_switch_key_accepts_shipping_defaults() {
    // 출하 기본값과 대표 대안 키. **수정자 키 자체가 유효**해야 한다 —
    // ATF 핫키 파서(`parse_atf_hotkey`)를 이 필드에 재사용하면 `RightAlt` 가
    // `is_modifier()` 배제로 오탐되므로, 그 회귀를 여기서 고정한다.
    for name in ["Korean", "RightAlt", "Hanja", "F9", "CapsLock", "LeftShift"] {
        assert_eq!(
            InputEngine::parse_switch_key(name),
            Some(KeyCode::from_name(name)),
            "'{name}' 는 유효한 전환키 이름이어야 함"
        );
    }
}

#[test]
fn parse_switch_key_rejects_combo_and_typo() {
    // 조합 표기·오타·별칭·공백·대소문자 불일치는 전부 거부(엔진 필터와 동치).
    for name in [
        "Shift+Space",
        "Control+Enter",
        "Ctrl-Left",
        "Hangul",
        "RigthAlt",
        "ScrollLock",
        "Pause",
        " F9",
        "hanja",
        "",
    ] {
        assert_eq!(
            InputEngine::parse_switch_key(name),
            None,
            "'{name}' 는 거부되어야 함"
        );
    }
}

#[test]
fn parse_switch_key_matches_engine_filter() {
    // 검증 술어(`parse_switch_key`)와 엔진이 실제로 적용하는 필터의 동치를 고정한다.
    // 어긋나면 GUI/CLI 가 "유효"라고 한 키가 엔진에서 죽거나 그 반대가 된다.
    let specs = ["Korean", "RigthAlt", "F9", "Shift+F9", "Hanja", ""];
    let mut config = Config::default();
    config.engine.toggle_keys = specs.iter().map(|s| s.to_string()).collect();
    config.engine.hanja_keys = specs.iter().map(|s| s.to_string()).collect();

    let expected = specs
        .iter()
        .filter(|s| InputEngine::parse_switch_key(s).is_some())
        .count();
    let engine = InputEngine::new(&config);
    assert_eq!(engine.toggle_keys.len(), expected);
    assert_eq!(engine.hanja_keys.len(), expected);

    // 재적용 경로(`set_switch_keys`)도 동일 필터여야 한다.
    let mut engine2 = InputEngine::new(&Config::default());
    engine2.set_switch_keys(&config);
    assert_eq!(engine2.toggle_keys.len(), expected);
    assert_eq!(engine2.hanja_keys.len(), expected);
}

#[test]
fn parse_switch_keys_drops_invalid_preserving_order() {
    // 로그 부수효과가 붙어도 결과 Vec 은 종전 `map + filter` 와 바이트 동일해야 한다.
    let names: Vec<String> = ["Korean", "RigthAlt", "F9", "Shift+F9", "Hanja"]
        .iter()
        .map(|s| s.to_string())
        .collect();
    let legacy: Vec<KeyCode> = names
        .iter()
        .map(|n| KeyCode::from_name(n))
        .filter(|k| *k != KeyCode::Unknown)
        .collect();
    assert_eq!(
        InputEngine::parse_switch_keys(&names, "한/영 전환키"),
        legacy
    );
    assert_eq!(legacy, vec![KeyCode::Korean, KeyCode::F9, KeyCode::Hanja]);
}

#[test]
fn parse_switch_keys_result_stable_across_repeat_calls() {
    // B2: 로그 dedupe 게이트(`log_state_changed`)는 부수효과일 뿐, 파싱 결과에는
    // 영향을 주면 안 된다 — 반복 호출(리로드 루프가 여러 컨텍스트를 순회하는 상황을
    // 흉내)해도 두 번째 호출의 Vec<KeyCode> 가 첫 번째와 동일해야 한다.
    let names: Vec<String> = ["Korean", "RigthAlt", "F9"]
        .iter()
        .map(|s| s.to_string())
        .collect();
    let first = InputEngine::parse_switch_keys(&names, "테스트 전용 필드 A");
    let second = InputEngine::parse_switch_keys(&names, "테스트 전용 필드 A");
    assert_eq!(first, second);
    assert_eq!(first, vec![KeyCode::Korean, KeyCode::F9]);
}
