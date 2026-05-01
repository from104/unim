//! 입력 엔진 모듈
//!
//! 실시간 키 입력을 처리하고 한국어 조합을 관리하는 핵심 엔진입니다.
//!
//! 외부 노출 경로(`crate::input_engine::{InputEngine, InputResult, PopupAction}`)는
//! 이전과 동일하다. 내부 구현은 책임별 서브모듈로 분리되어 있으며, 각 서브모듈은
//! `impl InputEngine { … }` 부분 블록을 다시 열어 메서드를 추가하는 분산 impl 패턴을
//! 사용한다.

mod candidates;
mod engine;
mod popup_dispatch;
mod press_key;
mod surrounding;
mod types;

pub use engine::InputEngine;
pub use types::{InputResult, PopupAction};

use crate::config::Config;
use crate::hangul::input_context::{ComposerType, HangulInputContext};

#[cfg(test)]
use crate::config::{ContentPurpose, InputCategory};
#[cfg(test)]
use crate::keycode::{KeyCode, ModifierState};

/// Config로부터 `HangulInputContext`를 구성.
///
/// 우선 `ProfileRegistry`로 `effective_layout_name()`에 해당하는 프로필을 찾고
/// inherits 해석 + `active_rule_sets` override를 적용해 `new_with_profile` 경로로
/// 생성. 어떤 단계라도 실패하면 enum 기반 legacy 경로로 안전 폴백한다.
///
/// 폴백이 발생하면 stderr에 원인을 기록하되, 엔진 시작은 계속한다.
fn build_korean_context(config: &Config, fallback_type: ComposerType) -> HangulInputContext {
    use crate::keystroke::profile::{resolve_inherits, ProfileRegistry};

    let name = config.engine.korean.effective_layout_name();
    let registry = ProfileRegistry::new();

    let Some(raw) = registry.find_raw(&name) else {
        eprintln!(
            "[UNIM] 프로필 '{}'을(를) 찾지 못함 → enum 경로로 폴백",
            name
        );
        return HangulInputContext::new(fallback_type);
    };

    let mut resolved = match resolve_inherits(&raw, &registry) {
        Ok(p) => p,
        Err(e) => {
            eprintln!(
                "[UNIM] 프로필 '{}' inherits 해석 실패: {e} → enum 경로로 폴백",
                name
            );
            return HangulInputContext::new(fallback_type);
        }
    };

    // Config의 active_rule_sets가 `Some(_)`이면 프로필 값을 override (§3.1).
    // 의미 일치:
    //   None         → 프로필 기본값(rule_sets.<name>.active 그대로) 사용
    //   Some(list)   → 명시적 활성 목록 (빈 list 포함, 빈 list = 모두 OFF)
    if let Some(list) = config.engine.korean.active_rule_sets.as_ref() {
        resolved.active_rule_sets = Some(list.clone());
    }

    match HangulInputContext::new_with_profile(&resolved) {
        Ok(ctx) => ctx,
        Err(e) => {
            eprintln!(
                "[UNIM] 프로필 '{}' 빌드 실패: {e:?} → enum 경로로 폴백",
                name
            );
            HangulInputContext::new(fallback_type)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_engine() -> InputEngine {
        let config = Config::default();
        InputEngine::new(&config)
    }

    #[test]
    fn test_engine_creation() {
        let engine = create_test_engine();
        assert_eq!(engine.input_category(), InputCategory::English);
        assert!(!engine.is_composing());
    }

    #[test]
    fn test_english_input() {
        let mut engine = create_test_engine();
        engine.set_input_category(InputCategory::English);

        let config = Config::default();
        let modifier = ModifierState::default();

        let result = engine.press_key(KeyCode::A, modifier, &config);
        assert!(result.consumed);
        assert!(result.commit_changed);
        assert_eq!(engine.commit_str(), "a");
    }

    #[test]
    fn test_input_category_toggle() {
        let mut engine = create_test_engine();
        assert_eq!(engine.input_category(), InputCategory::English);

        let config = Config::default();
        let modifier = ModifierState::default();

        engine.press_key(KeyCode::Korean, modifier, &config);
        assert_eq!(engine.input_category(), InputCategory::Korean);

        engine.press_key(KeyCode::Korean, modifier, &config);
        assert_eq!(engine.input_category(), InputCategory::English);
    }

    #[test]
    fn test_reset() {
        let mut engine = create_test_engine();
        engine.commit_buffer.push_str("test");

        engine.reset();
        assert!(engine.commit_str().is_empty());
        assert!(engine.preedit_str().is_empty());
    }

    // === 한국어 모드 기본 키 입력 테스트 ===

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

    // =========================================
    // 통합 테스트 시나리오
    // =========================================

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

    // === 자동 영문 전환 (auto-english) 테스트 ===

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
        use super::types::AutoEnglishTrigger;
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
        use super::types::AutoEnglishTrigger;
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
        use super::types::AutoEnglishTrigger;
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

    // === 이모지 팝업 트리거 테스트 ===

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
        engine.set_input_category(InputCategory::Korean);
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

    // === v1 프로필/active_rule_sets hot-rebuild 회귀 방지 ===

    /// T2-A: `set_korean_layout` 단독 호출이 v1 builder 경로(`build_korean_context`)를
    /// 거쳐 컨텍스트를 새로 만든다. 이전에는 `HangulInputContext::new(composer_type)`로
    /// v1 프로필을 우회했음. 본 테스트는 panic 없이 layout이 갱신됨을 보장.
    #[test]
    fn test_set_korean_layout_routes_through_v1_builder() {
        let mut config = Config::default();
        config.engine.korean.layout = "ko_2bulstd".to_string();
        let mut engine = InputEngine::new(&config);
        assert_eq!(engine.korean_layout, "ko_2bulstd");

        engine.set_korean_layout("ko_3bul_qwerty".to_string());
        assert_eq!(engine.korean_layout, "ko_3bul_qwerty");
        // 컨텍스트가 ThreeBul composer로 재생성되어야 함
        assert!(crate::config::is_sebeolsik_layout(&engine.korean_layout));
    }

    /// T2-A': `rebuild_korean_context`가 layout 동일 + active_rule_sets만 다른 경우에도
    /// 컨텍스트를 다시 만든다. 이전에는 hot-reload가 layout-only 비교라 누락됐음.
    #[test]
    fn test_rebuild_korean_context_applies_active_rule_sets() {
        let mut config = Config::default();
        config.engine.korean.layout = "ko_3bul390".to_string();
        let mut engine = InputEngine::new(&config);
        assert_eq!(engine.korean_layout, "ko_3bul390");

        // active_rule_sets만 변경 — layout은 동일
        config.engine.korean.active_rule_sets = Some(vec!["nonexistent_set".to_string()]);
        // panic 없이 통과해야 하며, rule_set이 없어도 폴백 경로로 컨텍스트가 살아 있어야 함
        engine.rebuild_korean_context(&config);
        assert_eq!(engine.korean_layout, "ko_3bul390");
    }

    /// T2-C: `rebuild_korean_context`가 layout 변경도 정상 처리한다.
    #[test]
    fn test_rebuild_korean_context_handles_layout_change() {
        let mut config = Config::default();
        config.engine.korean.layout = "ko_2bulstd".to_string();
        let mut engine = InputEngine::new(&config);
        assert_eq!(engine.korean_layout, "ko_2bulstd");

        config.engine.korean.layout = "ko_3bul_qwerty".to_string();
        engine.rebuild_korean_context(&config);
        assert_eq!(engine.korean_layout, "ko_3bul_qwerty");
    }

    /// builder의 3분기(None / Some(vec![]) / Some(vec![name])) 모두 panic 없이
    /// 컨텍스트를 만들어야 한다. 의미는 builder 단위 테스트가 따로 검증하지만,
    /// 본 테스트는 `build_korean_context` 호출 경로가 Option 타입을 그대로
    /// 받아들이는 것을 보장한다.
    #[test]
    fn test_build_korean_context_accepts_three_active_rule_sets_variants() {
        // None — 미설정, 프로필 기본값 사용
        let mut config = Config::default();
        config.engine.korean.layout = "ko_3bul390".to_string();
        config.engine.korean.active_rule_sets = None;
        let _ = InputEngine::new(&config);

        // Some(vec![]) — 사용자가 모두 OFF
        config.engine.korean.active_rule_sets = Some(Vec::new());
        let _ = InputEngine::new(&config);

        // Some(vec!["unknown"]) — 존재하지 않는 이름은 silently drop, 폴백 안전
        config.engine.korean.active_rule_sets = Some(vec!["__nonexistent__".to_string()]);
        let _ = InputEngine::new(&config);
    }

    // ====================================================================
    // 룰 B (schema v2 `key_meta.context_alt`) — `/` 키 컨텍스트 분기
    // ====================================================================

    fn make_3bul390_engine_no_auto_english() -> (InputEngine, Config) {
        let mut config = Config::default();
        config.engine.korean.layout = "ko_3bul390".to_string();
        // 룰 B 검증을 위해 auto_english 비활성 — 기본 trigger_keys에 char:/가 있어
        // 활성 시 / 키가 auto-english 트리거로 먼저 잡힌다.
        config.engine.auto_english.enabled = false;
        let mut engine = InputEngine::new(&config);
        engine.set_input_category(InputCategory::Korean);
        (engine, config)
    }

    /// 룰 B: preedit 빈 상태에서 영어 자판 `/` 키 → 리터럴 '/' commit
    #[test]
    fn test_rule_b_empty_preedit_commits_slash() {
        let (mut engine, config) = make_3bul390_engine_no_auto_english();
        let none = ModifierState::default();
        let result = engine.press_key(KeyCode::Slash, none, &config);
        assert!(result.consumed);
        assert!(result.commit_changed);
        assert_eq!(engine.commit_str(), "/");
        assert!(engine.preedit_str().is_empty());
    }

    /// 룰 B: 초성 ㄱ만 채워진 상태에서 `/` → ㄱ + ㅗ로 합성 (preedit "고")
    #[test]
    fn test_rule_b_choseong_only_keeps_jamo() {
        let (mut engine, config) = make_3bul390_engine_no_auto_english();
        let none = ModifierState::default();
        // ko_3bul390에서 'k' 키는 ㄱ (3nd 행)
        engine.press_key(KeyCode::K, none, &config);
        assert_eq!(engine.preedit_str(), "\u{3131}"); // ㄱ choseong-only

        // 영어 자판 `/` 키 → ko_3bul390 한국어 자판의 ㅗ로 매핑됨
        let result = engine.press_key(KeyCode::Slash, none, &config);
        assert!(result.consumed);
        // commit는 비어있어야 함 (음절 조합 진행 중)
        assert!(
            engine.commit_str().is_empty(),
            "commit='{}'",
            engine.commit_str()
        );
        assert_eq!(engine.preedit_str(), "고");
    }

    /// 룰 B: 초성+중성 채워진 상태에서 `/` → 리터럴 '/' commit (ㅘ로 합성 안 됨)
    #[test]
    fn test_rule_b_cho_jung_filled_commits_slash() {
        let (mut engine, config) = make_3bul390_engine_no_auto_english();
        let none = ModifierState::default();
        // ㄱ → preedit "ㄱ"
        engine.press_key(KeyCode::K, none, &config);
        // ko_3bul390에서 'f' 키는 ㅏ (3nd 행)
        engine.press_key(KeyCode::F, none, &config);
        assert_eq!(engine.preedit_str(), "가");

        // 영어 자판 / 입력 → preedit "가" flush + '/' commit (룰 B fallback)
        let result = engine.press_key(KeyCode::Slash, none, &config);
        assert!(result.consumed);
        assert!(result.commit_changed);
        assert!(
            engine.commit_str().ends_with('/'),
            "commit='{}'",
            engine.commit_str()
        );
        assert!(engine.preedit_str().is_empty());
    }

    /// 룰 B: 영문 모드에서 `/` → 룰 B 미적용 (한글 분기 진입 안 함)
    #[test]
    fn test_rule_b_english_mode_unaffected() {
        let (mut engine, config) = make_3bul390_engine_no_auto_english();
        engine.set_input_category(InputCategory::English);
        let none = ModifierState::default();
        let _ = engine.press_key(KeyCode::Slash, none, &config);
        // 영문 모드는 process_english_key 경로 — 룰 B 코드가 실행되지 않음.
        // 패닉 없으면 OK.
    }

    /// 룰 B 회귀: 두벌식(ko_2bulstd)은 key_meta가 없어 룰 B 미적용
    #[test]
    fn test_rule_b_two_bul_no_key_meta_branch() {
        let mut config = Config::default();
        config.engine.korean.layout = "ko_2bulstd".to_string();
        config.engine.auto_english.enabled = false;
        let mut engine = InputEngine::new(&config);
        engine.set_input_category(InputCategory::Korean);
        // 두벌식 키맵에는 key_meta가 없어 key_meta_map이 비어 있어야 함
        assert!(
            engine.key_meta_map.is_empty(),
            "ko_2bulstd should have empty key_meta_map"
        );
    }
}
