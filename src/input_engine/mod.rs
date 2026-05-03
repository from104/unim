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
pub use types::{InputResult, PageDirection, PopupAction};

use crate::config::Config;
use crate::hangul::input_context::{ComposerType, HangulInputContext};

#[cfg(test)]
mod test_helpers;
#[cfg(test)]
mod tests_composition;
#[cfg(test)]
mod tests_scenarios;
#[cfg(test)]
mod tests_auto_english;
#[cfg(test)]
mod tests_emoji_trigger;
#[cfg(test)]
mod tests_popup_change_page;
#[cfg(test)]
mod tests_profile;

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
    use super::test_helpers::create_test_engine;
    use crate::config::{Config, InputCategory};
    use crate::keycode::{KeyCode, ModifierState};

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
}
