//! 키 소비 판정 중 **플랫폼 중립**인 순수 로직.
//!
//! `key_handler`/`text_service` 는 `#[cfg(windows)]` 게이트 안에 있어 Linux 에서
//! 컴파일조차 되지 않으므로 단위 테스트를 붙일 수 없다. COM 이 필요 없는 판정만
//! 여기로 분리해 Linux 에서도 `cargo test -p unim-tsf` 로 검증한다.

use unim::keycode::ModifierState;

/// TSF keyboard-disabled 컨텍스트에서도 **예외적으로 소비해야 하는 키**인지.
///
/// Chromium 의 `InitializeDisabledContext` 처럼 호스트가 "이 문서는 입력을 받지
/// 않는다" 고 표시한 컨텍스트에서는 원칙적으로 모든 키를 앱에 통과시킨다
/// (스페이스 스크롤·`/` 검색·j/k 이동 등 브라우저 단축키 보존). 다만 다음 둘은
/// 종전대로 소비한다:
///
/// 1. **한/영 전환키** (단축키 조합이 아닐 때) — 통과시키면 대부분의 호스트가
///    `OnKeyDown` 을 호출하지 않아 토글 자체가 죽는다. `test_key_down` 의
///    `is_toggle` 분기와 정확히 같은 `shortcut_combo` 판정을 쓴다.
/// 2. **AutoTypeFix 토글 핫키** — 위와 동일한 이유(핫키가 죽는다).
///
/// # Arguments
/// * `is_toggle` - 이 키가 설정된 한/영 전환키인지 (`engine.is_toggle_key`)
/// * `self_is_modifier` - 그 키 자체가 수정자 키인지 (우Alt 토글 등)
/// * `is_atf_hotkey` - `engine.is_atf_hotkey(keycode, modifiers)` 결과
/// * `modifiers` - 현재 수정자 상태
pub fn keyboard_disabled_allows(
    is_toggle: bool,
    self_is_modifier: bool,
    is_atf_hotkey: bool,
    modifiers: ModifierState,
) -> bool {
    if is_atf_hotkey {
        return true;
    }
    if is_toggle {
        // 우Alt 처럼 키 자체가 수정자면 Alt 비트가 서 있어도 단축키 조합이 아니다.
        let shortcut_combo =
            modifiers.control || modifiers.super_key || (modifiers.alt && !self_is_modifier);
        return !shortcut_combo;
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mods() -> ModifierState {
        ModifierState::new()
    }

    #[test]
    fn plain_character_key_passes_through_when_disabled() {
        // 문자키·기능키 등 일반 키는 비활성 컨텍스트에서 소비하지 않는다.
        assert!(!keyboard_disabled_allows(false, false, false, mods()));
    }

    #[test]
    fn toggle_key_is_still_consumed() {
        assert!(keyboard_disabled_allows(true, false, false, mods()));
    }

    #[test]
    fn modifier_toggle_key_is_still_consumed() {
        // 우Alt 토글: 자기 자신이 Alt 라 alt 비트가 서 있어도 소비해야 한다.
        let mut m = mods();
        m.alt = true;
        assert!(keyboard_disabled_allows(true, true, false, m));
    }

    #[test]
    fn toggle_key_with_shortcut_combo_passes_through() {
        let mut m = mods();
        m.control = true;
        assert!(!keyboard_disabled_allows(true, false, false, m));

        let mut m = mods();
        m.super_key = true;
        assert!(!keyboard_disabled_allows(true, false, false, m));

        // 수정자가 아닌 토글키 + Alt = 단축키 조합 → 통과.
        let mut m = mods();
        m.alt = true;
        assert!(!keyboard_disabled_allows(true, false, false, m));
    }

    #[test]
    fn atf_hotkey_is_still_consumed() {
        // 기본 Shift+F8 처럼 수정자를 포함해도 ATF 핫키는 소비한다.
        let mut m = mods();
        m.shift = true;
        assert!(keyboard_disabled_allows(false, false, true, m));

        // Ctrl 조합 ATF 핫키도 마찬가지.
        let mut m = mods();
        m.control = true;
        assert!(keyboard_disabled_allows(false, false, true, m));
    }
}
