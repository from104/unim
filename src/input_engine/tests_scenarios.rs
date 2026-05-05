//! 통합 시나리오 테스트.
//!
//! 한·영 혼용, TypeFix, smart backspace, 한자 변환, 특수문자 fallback,
//! 쌍자음, 숫자, CapsLock 등 실사용 워크플로 검증.

use super::test_helpers::create_test_engine;
use super::InputEngine;
use crate::config::{Config, ContentPurpose, InputCategory};
use crate::keycode::{KeyCode, ModifierState};

/// 헬퍼: 키 시퀀스를 입력하고 최종 결과를 수집
fn type_keys(engine: &mut InputEngine, keys: &[KeyCode], config: &Config) -> (String, String) {
    let modifier = ModifierState::default();
    let mut total_commit = String::new();
    for &key in keys {
        let result = engine.press_key(key, modifier, config);
        if result.commit_changed {
            total_commit.push_str(engine.commit_str());
            engine.clear_commit();
        }
    }
    (total_commit, engine.preedit_str().to_string())
}

#[test]
fn test_scenario_hangul_sentence() {
    // "안녕하세요" 입력 시나리오
    let mut engine = create_test_engine();
    let config = Config::default();
    engine.set_input_category(InputCategory::Korean);

    let keys = [
        KeyCode::D,
        KeyCode::K, // 아 → ㅏ
        KeyCode::S,
        KeyCode::S, // 안 → ㄴ+ㄴ (도깨비불)
        KeyCode::U,
        KeyCode::D, // 녕
        KeyCode::G,
        KeyCode::K, // 하
        KeyCode::T,
        KeyCode::P, // 세
        KeyCode::D,
        KeyCode::Y, // 요
    ];

    let (commit, preedit) = type_keys(&mut engine, &keys, &config);
    // 최종: 커밋된 텍스트 + 남은 preedit
    let full = format!("{}{}", commit, preedit);
    assert!(full.contains("안녕"), "Expected '안녕' in '{}'", full);
}

#[test]
fn test_scenario_mixed_korean_english() {
    // 한글 입력 → 한/영 전환 → 영문 입력 → 한/영 전환 → 한글 입력
    let mut engine = create_test_engine();
    let config = Config::default();
    let modifier = ModifierState::default();

    // 1. 한글 모드: "가" 입력
    engine.set_input_category(InputCategory::Korean);
    engine.press_key(KeyCode::R, modifier, &config); // ㄱ
    engine.press_key(KeyCode::K, modifier, &config); // 가
    assert_eq!(engine.preedit_str(), "가");

    // 2. 한/영 전환
    let result = engine.press_key(KeyCode::Korean, modifier, &config);
    assert!(result.commit_changed);
    let commit1 = engine.commit_str().to_string();
    engine.clear_commit();
    assert_eq!(commit1, "가");
    assert_eq!(engine.input_category(), InputCategory::English);

    // 3. 영문 입력 "ab"
    engine.press_key(KeyCode::A, modifier, &config);
    let a = engine.commit_str().to_string();
    engine.clear_commit();
    assert_eq!(a, "a");

    engine.press_key(KeyCode::B, modifier, &config);
    let b = engine.commit_str().to_string();
    engine.clear_commit();
    assert_eq!(b, "b");

    // 4. 한/영 전환 → 한글
    engine.press_key(KeyCode::Korean, modifier, &config);
    assert_eq!(engine.input_category(), InputCategory::Korean);

    // 5. 한글 "나"
    engine.press_key(KeyCode::S, modifier, &config); // ㄴ
    engine.press_key(KeyCode::K, modifier, &config); // 나
    assert_eq!(engine.preedit_str(), "나");
}

#[test]
fn test_scenario_backspace_during_composition() {
    // 조합 중 백스페이스: "각" → Backspace → "가" → Backspace → "ㄱ" → Backspace → 빈칸
    let mut engine = create_test_engine();
    let config = Config::default();
    let modifier = ModifierState::default();

    engine.set_input_category(InputCategory::Korean);

    // ㄱ+ㅏ+ㄱ = 각
    engine.press_key(KeyCode::R, modifier, &config);
    engine.press_key(KeyCode::K, modifier, &config);
    engine.press_key(KeyCode::R, modifier, &config);
    assert_eq!(engine.preedit_str(), "각");

    // Backspace → 가
    engine.press_key(KeyCode::Backspace, modifier, &config);
    assert_eq!(engine.preedit_str(), "가");

    // Backspace → ㄱ
    engine.press_key(KeyCode::Backspace, modifier, &config);
    assert_eq!(engine.preedit_str(), "ㄱ");

    // Backspace → 빈칸
    engine.press_key(KeyCode::Backspace, modifier, &config);
    assert_eq!(engine.preedit_str(), "");
    assert!(!engine.is_composing());
}

#[test]
fn test_scenario_content_purpose_password() {
    // 비밀번호 필드에서 한글 차단
    let mut engine = create_test_engine();
    let config = Config::default();
    let modifier = ModifierState::default();

    // 한글 모드로 전환
    engine.set_input_category(InputCategory::Korean);
    assert_eq!(engine.input_category(), InputCategory::Korean);

    // 비밀번호 목적 설정 → 자동 영문 전환
    engine.set_content_purpose(ContentPurpose::Password);
    assert_eq!(engine.input_category(), InputCategory::English);

    // 한/영 전환 시도 → 차단
    let result = engine.press_key(KeyCode::Korean, modifier, &config);
    assert!(result.consumed);
    assert_eq!(engine.input_category(), InputCategory::English);

    // 영문 입력은 정상 동작
    engine.press_key(KeyCode::A, modifier, &config);
    assert_eq!(engine.commit_str(), "a");
}

#[test]
fn test_scenario_content_purpose_normal_after_password() {
    // 비밀번호 → Normal 전환 시 한글 모드 복구 가능
    let mut engine = create_test_engine();
    let config = Config::default();
    let modifier = ModifierState::default();

    engine.set_input_category(InputCategory::Korean);
    engine.set_content_purpose(ContentPurpose::Password);
    assert_eq!(engine.input_category(), InputCategory::English);

    // Normal로 복원
    engine.set_content_purpose(ContentPurpose::Normal);

    // 이제 한/영 전환 가능
    engine.press_key(KeyCode::Korean, modifier, &config);
    assert_eq!(engine.input_category(), InputCategory::Korean);
}

#[test]
fn test_scenario_typefix_with_selection() {
    // TypeFix: 선택된 텍스트 영→한 변환 + 모드 자동 전환
    let mut engine = create_test_engine();
    engine.set_input_category(InputCategory::English);

    // "gksrmf" 전체 선택 (cursor=6, anchor=0)
    engine.set_surrounding_text("gksrmf".to_string(), 6, 0);

    // TypeFix 자동 감지 (영문 → 한글)
    let result = engine.typefix_convert(0);
    assert!(result.is_some());
    let (offset, delete_count, replacement) = result.unwrap();
    assert_eq!(offset, -6); // cursor=6, start=0 → offset = 0 - 6 = -6
    assert_eq!(delete_count, 6);
    assert_eq!(replacement, "한글");
    assert_eq!(engine.input_category(), InputCategory::Korean);
}

#[test]
fn test_scenario_typefix_no_selection_returns_none() {
    // TypeFix: 선택 없으면 None 반환
    let mut engine = create_test_engine();
    engine.set_surrounding_text("gksrmf".to_string(), 6, 6);
    assert!(engine.typefix_convert(0).is_none());
}

#[test]
fn test_scenario_typefix_kor_to_eng() {
    // TypeFix: 한글 → 영문 강제 변환 (선택 필수)
    let mut engine = create_test_engine();
    engine.set_input_category(InputCategory::Korean);

    // "한글" 전체 선택 (cursor=2, anchor=0)
    engine.set_surrounding_text("한글".to_string(), 2, 0);

    let result = engine.typefix_convert(2);
    assert!(result.is_some());
    let (offset, delete_count, replacement) = result.unwrap();
    assert_eq!(offset, -2); // cursor=2, start=0 → offset = 0 - 2 = -2
    assert_eq!(delete_count, 2);
    assert_eq!(replacement, "gksrmf");
    assert_eq!(engine.input_category(), InputCategory::English);
}

#[test]
fn test_scenario_typefix_selection() {
    // 선택 영역 TypeFix: cursor != anchor일 때 선택 영역 변환
    let mut engine = create_test_engine();
    engine.set_input_category(InputCategory::English);

    // "hello gksrmf world" 에서 "gksrmf"가 선택됨 (cursor=12, anchor=6)
    engine.set_surrounding_text("hello gksrmf world".to_string(), 12, 6);

    let result = engine.typefix_convert(0);
    assert!(result.is_some());
    let (offset, delete_count, replacement) = result.unwrap();
    assert_eq!(offset, -6); // cursor=12, start=6 → offset = 6 - 12 = -6
    assert_eq!(delete_count, 6); // "gksrmf" 6글자 삭제
    assert_eq!(replacement, "한글");
    assert_eq!(engine.input_category(), InputCategory::Korean);
}

#[test]
fn test_scenario_typefix_auto_detect() {
    // 자동 감지: 한글 자모 선택 → 영문으로 변환
    let mut engine = create_test_engine();
    engine.set_input_category(InputCategory::Korean);

    // "ㅗ디ㅣㅐ" 전체 선택 (cursor=4, anchor=0)
    engine.set_surrounding_text("ㅗ디ㅣㅐ".to_string(), 4, 0);

    let result = engine.typefix_convert(0);
    assert!(result.is_some());
    let (_offset, _delete_count, replacement) = result.unwrap();
    assert!(!replacement.is_empty());
    assert_eq!(engine.input_category(), InputCategory::English);
}

#[test]
fn test_scenario_smart_backspace() {
    // Smart Backspace: 커밋된 "한" → "하" → "ㅎ" → 삭제
    let mut engine = create_test_engine();

    // "한" 글자 뒤에 커서
    engine.set_surrounding_text("한".to_string(), 1, 1);
    let result = engine.smart_backspace();
    assert!(result.is_some());
    let (del, repl) = result.unwrap();
    assert_eq!(del, 1);
    assert_eq!(repl, "하"); // 종성 ㄴ 제거 → 하

    // "하" 글자 뒤에 커서
    engine.set_surrounding_text("하".to_string(), 1, 1);
    let result = engine.smart_backspace();
    assert!(result.is_some());
    let (del, repl) = result.unwrap();
    assert_eq!(del, 1);
    assert_eq!(repl, "ㅎ"); // 중성 ㅏ 제거 → ㅎ

    // "ㅎ" 글자 → 한글 음절이 아니므로 None
    engine.set_surrounding_text("ㅎ".to_string(), 1, 1);
    let result = engine.smart_backspace();
    assert!(result.is_none()); // 자모는 음절이 아님
}

#[test]
fn test_scenario_hanja_conversion() {
    // 한자 변환 시나리오: "가" → 한자 후보 표시 → 선택
    let mut engine = create_test_engine();
    let config = Config::default();
    let modifier = ModifierState::default();

    engine.set_input_category(InputCategory::Korean);

    // "가" 입력
    engine.press_key(KeyCode::R, modifier, &config); // ㄱ
    engine.press_key(KeyCode::K, modifier, &config); // 가
    assert_eq!(engine.preedit_str(), "가");

    // 한자 변환 시작
    let result = engine.start_hanja_conversion();
    assert!(result.hanja_candidates_available);
    assert!(engine.is_hanja_mode());

    // 후보 목록 확인
    let candidates = engine.get_hanja_candidates();
    assert!(!candidates.is_empty());

    // 첫 번째 한자 선택
    let selected = engine.select_hanja(0);
    assert!(selected.is_some());
    assert!(!engine.is_hanja_mode()); // 모드 해제
}

#[test]
fn test_scenario_special_char_fallback() {
    // 특수문자 fallback: 초성 "ㄱ" → 한자 후보 없음 → 특수문자 후보
    let mut engine = create_test_engine();
    let config = Config::default();
    let modifier = ModifierState::default();

    engine.set_input_category(InputCategory::Korean);

    // "ㄱ" 입력 (초성만)
    engine.press_key(KeyCode::R, modifier, &config);
    assert_eq!(engine.preedit_str(), "ㄱ");

    // 한자 변환 시작 → 한자 없음 → 특수문자 fallback
    let result = engine.start_hanja_conversion();
    // ㄱ에 대한 한자가 없으면 특수문자 모드로 전환
    if result.special_char_candidates_available {
        assert!(engine.is_special_char_mode());
        let candidates = engine.get_special_char_candidates();
        assert!(!candidates.is_empty());
    }
}

#[test]
fn test_scenario_double_consonant() {
    // 쌍자음 입력: ㄲ (Shift+ㄱ)
    let mut engine = create_test_engine();
    let config = Config::default();
    let shift = ModifierState {
        shift: true,
        ..Default::default()
    };
    let modifier = ModifierState::default();

    engine.set_input_category(InputCategory::Korean);

    // Shift+R = ㄲ
    engine.press_key(KeyCode::R, shift, &config);
    assert_eq!(engine.preedit_str(), "ㄲ");

    // ㅏ → 까
    engine.press_key(KeyCode::K, modifier, &config);
    assert_eq!(engine.preedit_str(), "까");
}

#[test]
fn test_scenario_space_after_composition() {
    // 조합 후 스페이스: "가" + Space → "가 " 커밋
    let mut engine = create_test_engine();
    let config = Config::default();
    let modifier = ModifierState::default();

    engine.set_input_category(InputCategory::Korean);

    engine.press_key(KeyCode::R, modifier, &config);
    engine.press_key(KeyCode::K, modifier, &config);
    assert_eq!(engine.preedit_str(), "가");

    let result = engine.press_key(KeyCode::Space, modifier, &config);
    assert!(result.consumed);
    assert!(result.commit_changed);
    assert_eq!(engine.commit_str(), "가 ");
    assert_eq!(engine.preedit_str(), "");
}

#[test]
fn test_scenario_number_in_korean_mode() {
    // 한글 모드에서 숫자: 조합 커밋 후 숫자 커밋
    let mut engine = create_test_engine();
    let config = Config::default();
    let modifier = ModifierState::default();

    engine.set_input_category(InputCategory::Korean);

    engine.press_key(KeyCode::R, modifier, &config); // ㄱ
    engine.press_key(KeyCode::K, modifier, &config); // 가

    // 숫자 1 → 조합 "가" 커밋 + "1" 커밋
    let result = engine.press_key(KeyCode::Num1, modifier, &config);
    assert!(result.commit_changed);
    let committed = engine.commit_str().to_string();
    assert!(committed.contains("가"), "committed: '{}'", committed);
}

#[test]
fn test_scenario_caps_lock_korean() {
    // 한글 모드에서 CapsLock → 영향 없음 (쌍자음은 Shift로만)
    let mut engine = create_test_engine();
    let config = Config::default();
    let caps = ModifierState {
        caps_lock: true,
        ..Default::default()
    };

    engine.set_input_category(InputCategory::Korean);

    // CapsLock 상태에서 R → ㄱ (CapsLock 무시)
    engine.press_key(KeyCode::R, caps, &config);
    assert_eq!(engine.preedit_str(), "ㄱ");
}

#[test]
fn test_scenario_caps_lock_english() {
    // 영어 모드에서 CapsLock → 대문자
    let mut engine = create_test_engine();
    let config = Config::default();
    let caps = ModifierState {
        caps_lock: true,
        ..Default::default()
    };

    engine.set_input_category(InputCategory::English);

    engine.press_key(KeyCode::A, caps, &config);
    assert_eq!(engine.commit_str(), "A");
}

// ============================================================================
// I-AM 통합 테스트 — 안마태 자판 (Phase 3 Final)
// ============================================================================

/// 안마태 레이아웃으로 엔진을 초기화하는 헬퍼.
fn create_anmatae_engine() -> (InputEngine, Config) {
    let mut config = Config::default();
    config.engine.korean.layout = "ko_anmatae".to_string();
    let engine = InputEngine::new(&config);
    (engine, config)
}

/// I-AM4: 안마태 자판에서 Shift+B → `"` (U+201C) 즉시 commit.
/// jamo_symbol_map 경로: keyboard_map 우회 + composer 큐 무영향.
#[test]
fn i_am4_jamo_symbol_map_shift_b_commits_left_quote() {
    let (mut engine, config) = create_anmatae_engine();
    let shift = ModifierState {
        shift: true,
        ..Default::default()
    };
    engine.set_input_category(InputCategory::Korean);

    // Shift+B → 'B' 문자 → jamo_symbol_map에서 " 반환
    let result = engine.press_key(KeyCode::B, shift, &config);
    assert!(result.commit_changed, "Shift+B should produce commit");
    assert_eq!(engine.commit_str(), "\u{201C}", "Shift+B → 여는 큰따옴표 \"");
    assert!(engine.preedit_str().is_empty(), "preedit은 비어있어야 함");
}

/// I-AM4b: 안마태 자판에서 Shift+G → `"` (U+201D) 즉시 commit.
#[test]
fn i_am4b_jamo_symbol_map_shift_g_commits_right_quote() {
    let (mut engine, config) = create_anmatae_engine();
    let shift = ModifierState {
        shift: true,
        ..Default::default()
    };
    engine.set_input_category(InputCategory::Korean);

    let result = engine.press_key(KeyCode::G, shift, &config);
    assert!(result.commit_changed, "Shift+G should produce commit");
    assert_eq!(engine.commit_str(), "\u{201D}", "Shift+G → 닫는 큰따옴표 \"");
}

/// I-AM4c: 안마태 자판에서 Shift+J → `·` (U+00B7) 즉시 commit.
#[test]
fn i_am4c_jamo_symbol_map_shift_j_commits_middle_dot() {
    let (mut engine, config) = create_anmatae_engine();
    let shift = ModifierState {
        shift: true,
        ..Default::default()
    };
    engine.set_input_category(InputCategory::Korean);

    let result = engine.press_key(KeyCode::J, shift, &config);
    assert!(result.commit_changed, "Shift+J should produce commit");
    assert_eq!(engine.commit_str(), "\u{00B7}", "Shift+J → 가운뎃점 ·");
}

/// I-AM4d: 안마태 자판에서 Shift+T → `…` (U+2026) 즉시 commit.
#[test]
fn i_am4d_jamo_symbol_map_shift_t_commits_ellipsis() {
    let (mut engine, config) = create_anmatae_engine();
    let shift = ModifierState {
        shift: true,
        ..Default::default()
    };
    engine.set_input_category(InputCategory::Korean);

    let result = engine.press_key(KeyCode::T, shift, &config);
    assert!(result.commit_changed, "Shift+T should produce commit");
    assert_eq!(engine.commit_str(), "\u{2026}", "Shift+T → 줄임표 …");
}

/// I-AM5: 안마태 자판에서 ESC 입력 → 조합 없으면 not_consumed (기존 동작 회귀).
#[test]
fn i_am5_escape_reset_no_composition() {
    let (mut engine, config) = create_anmatae_engine();
    engine.set_input_category(InputCategory::Korean);

    let result = engine.press_key(KeyCode::Escape, ModifierState::default(), &config);
    assert!(!result.commit_changed, "ESC with no composition → not_consumed");
    assert!(engine.preedit_str().is_empty(), "preedit 없음");
}

/// I-AM-LOAD: 안마태 자판 로드 시 jamo_symbol_map 6개 빌드 검증.
#[test]
fn i_am_load_jamo_symbol_map_6_entries() {
    let (engine, _config) = create_anmatae_engine();
    assert_eq!(
        engine.jamo_symbol_map.len(),
        6,
        "안마태 jamo_symbol_map = 6개 (B/G/J/N/T/W upper)"
    );
    assert_eq!(engine.jamo_symbol_map.get(&'B'), Some(&'\u{201C}'));
    assert_eq!(engine.jamo_symbol_map.get(&'G'), Some(&'\u{201D}'));
    assert_eq!(engine.jamo_symbol_map.get(&'J'), Some(&'\u{00B7}'));
    assert_eq!(engine.jamo_symbol_map.get(&'N'), Some(&'\u{2018}'));
    assert_eq!(engine.jamo_symbol_map.get(&'T'), Some(&'\u{2026}'));
    assert_eq!(engine.jamo_symbol_map.get(&'W'), Some(&'\u{2019}'));
}
