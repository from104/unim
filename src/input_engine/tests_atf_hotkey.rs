//! AutoTypeFix 토글 단축키(`atf_hotkeys_*`) 매칭 회귀 테스트.
//!
//! ATF 핫키는 `toggle_keys` 와 동일한 단일 비수정자 키 목록 문법을 쓰되, 매칭 시
//! config·`InputResult` 를 건드리지 않고 엔진의 `pending_atf_toggle` 에 대상 플래그만
//! 적재한다(호스트가 `take_atf_toggle()` 로 드레인). 이 모듈은 ① 각 kind 매칭→소비+
//! 드레인 1회성, ② 수정자 동반 시 미소비, ③ 빈 기본값(옵트인) 무동작(무회귀),
//! ④ `set_atf_hotkeys` 재적용을 고정한다.

use super::types::AtfToggleKind;
use super::InputEngine;
use crate::config::Config;
use crate::keycode::{KeyCode, ModifierState};

/// ATF 핫키 3목록을 지정한 config 를 만든다 (그 외는 기본값).
fn config_with_atf_hotkeys(enabled: &[&str], forward: &[&str], reverse: &[&str]) -> Config {
    let mut config = Config::default();
    let to_vec = |xs: &[&str]| xs.iter().map(|s| s.to_string()).collect();
    config.engine.auto_typefix.toggle_enabled_keys = to_vec(enabled);
    config.engine.auto_typefix.toggle_forward_keys = to_vec(forward);
    config.engine.auto_typefix.toggle_reverse_keys = to_vec(reverse);
    config
}

#[test]
fn is_atf_hotkey_reports_configured_keys() {
    let config = config_with_atf_hotkeys(&["F10"], &["F11"], &["F12"]);
    let engine = InputEngine::new(&config);
    assert!(engine.is_atf_hotkey(KeyCode::F10));
    assert!(engine.is_atf_hotkey(KeyCode::F11));
    assert!(engine.is_atf_hotkey(KeyCode::F12));
    assert!(!engine.is_atf_hotkey(KeyCode::A));
    // 기본(빈 목록) 엔진은 어떤 키도 ATF 핫키가 아니다.
    let plain = InputEngine::new(&Config::default());
    assert!(!plain.is_atf_hotkey(KeyCode::F10));
}

#[test]
fn each_kind_matches_and_drains_once() {
    let config = config_with_atf_hotkeys(&["F10"], &["F11"], &["F12"]);
    let mut engine = InputEngine::new(&config);
    let mods = ModifierState::default();

    // 전체(enabled) 핫키 → 소비 + Enabled 드레인.
    let r = engine.press_key(KeyCode::F10, mods, &config);
    assert!(r.consumed, "ATF 전체 토글 핫키는 소비되어야 함");
    assert_eq!(engine.take_atf_toggle(), Some(AtfToggleKind::Enabled));
    // 드레인은 1회성 — 다시 꺼내면 None.
    assert_eq!(engine.take_atf_toggle(), None);

    // 순방향(forward) 핫키 → Forward.
    engine.press_key(KeyCode::F11, mods, &config);
    assert_eq!(engine.take_atf_toggle(), Some(AtfToggleKind::Forward));

    // 역방향(reverse) 핫키 → Reverse.
    engine.press_key(KeyCode::F12, mods, &config);
    assert_eq!(engine.take_atf_toggle(), Some(AtfToggleKind::Reverse));
}

#[test]
fn duplicate_key_prefers_enabled() {
    // 한 키가 여러 목록에 중복 지정되면 전체(Enabled) → 순방향 → 역방향 순 우선.
    let config = config_with_atf_hotkeys(&["F10"], &["F10"], &["F10"]);
    let mut engine = InputEngine::new(&config);
    engine.press_key(KeyCode::F10, ModifierState::default(), &config);
    assert_eq!(engine.take_atf_toggle(), Some(AtfToggleKind::Enabled));
}

#[test]
fn ctrl_modifier_is_shortcut_not_atf_toggle() {
    // Ctrl+핫키 는 단축키로 통과해야 한다(ATF 토글 X, 소비 X).
    let config = config_with_atf_hotkeys(&["F10"], &[], &[]);
    let mut engine = InputEngine::new(&config);

    let mods = ModifierState {
        control: true,
        ..ModifierState::default()
    };
    let r = engine.press_key(KeyCode::F10, mods, &config);
    assert!(!r.consumed, "Ctrl+핫키 는 단축키 — 통과");
    assert_eq!(
        engine.take_atf_toggle(),
        None,
        "수정자 동반 시 ATF 토글이 적재되지 않아야 함"
    );
}

#[test]
fn super_modifier_is_shortcut_not_atf_toggle() {
    let config = config_with_atf_hotkeys(&["F10"], &[], &[]);
    let mut engine = InputEngine::new(&config);

    let mods = ModifierState {
        super_key: true,
        ..ModifierState::default()
    };
    engine.press_key(KeyCode::F10, mods, &config);
    assert_eq!(engine.take_atf_toggle(), None);
}

#[test]
fn shift_modifier_is_shortcut_not_atf_toggle() {
    // Shift+핫키(예: Shift+F10 컨텍스트 메뉴)는 앱 단축키로 통과 — ATF 토글 X, 소비 X.
    let config = config_with_atf_hotkeys(&["F10"], &[], &[]);
    let mut engine = InputEngine::new(&config);

    let mods = ModifierState {
        shift: true,
        ..ModifierState::default()
    };
    let r = engine.press_key(KeyCode::F10, mods, &config);
    assert!(!r.consumed, "Shift+핫키 는 앱 단축키 — 통과(미소비)");
    assert_eq!(engine.take_atf_toggle(), None);
}

#[test]
fn modifier_key_is_excluded_from_atf_hotkeys() {
    // 수정자 키(RightAlt 등)를 핫키로 지정해도 파싱 단계에서 배제된다 —
    // press_key 의 is_modifier() 조기 반환과 정합(dead key 방지).
    let config = config_with_atf_hotkeys(&["RightAlt"], &["LeftControl"], &["LeftShift"]);
    let engine = InputEngine::new(&config);
    assert!(!engine.is_atf_hotkey(KeyCode::RightAlt));
    assert!(!engine.is_atf_hotkey(KeyCode::LeftControl));
    assert!(!engine.is_atf_hotkey(KeyCode::LeftShift));
}

#[test]
fn autorepeat_within_window_suppresses_toggle() {
    // 동일 키 홀드에 의한 자동반복(디바운스 창 이내 재매칭)은 소비는 유지하되
    // 토글을 생략한다 — 홀드 중 토글이 수십 번 반전되는 것 방지.
    let config = config_with_atf_hotkeys(&["F10"], &[], &[]);
    let mut engine = InputEngine::new(&config);

    // 첫 눌림 → 토글 적재.
    let r = engine.press_key(KeyCode::F10, ModifierState::default(), &config);
    assert!(r.consumed);
    assert_eq!(engine.take_atf_toggle(), Some(AtfToggleKind::Enabled));

    // 즉시 재매칭(자동반복) → 소비는 유지, 토글은 억제.
    let r = engine.press_key(KeyCode::F10, ModifierState::default(), &config);
    assert!(r.consumed, "자동반복 중에도 키는 소비되어야 함(앱 유출 방지)");
    assert_eq!(
        engine.take_atf_toggle(),
        None,
        "디바운스 창 이내 재매칭은 토글을 적재하지 않아야 함"
    );
}

#[test]
fn autorepeat_different_key_not_suppressed() {
    // 다른 키코드는 디바운스 대상이 아니다 — 즉시 눌러도 각자 토글된다.
    let config = config_with_atf_hotkeys(&["F10"], &["F11"], &[]);
    let mut engine = InputEngine::new(&config);

    engine.press_key(KeyCode::F10, ModifierState::default(), &config);
    assert_eq!(engine.take_atf_toggle(), Some(AtfToggleKind::Enabled));

    // 다른 키(F11) 를 즉시 눌러도 억제되지 않는다.
    engine.press_key(KeyCode::F11, ModifierState::default(), &config);
    assert_eq!(engine.take_atf_toggle(), Some(AtfToggleKind::Forward));
}

#[test]
fn re_toggle_after_debounce_window() {
    // 디바운스 창을 넘겨(홀드 해제 후 재입력) 다시 누르면 정상 토글 —
    // 영구 잠금이 아님을 보장. 실시간 sleep 없이 마지막 매칭 시각을 과거로
    // 앞당겨 창 경과를 결정론적으로 재현한다.
    let config = config_with_atf_hotkeys(&["F10"], &[], &[]);
    let mut engine = InputEngine::new(&config);

    engine.press_key(KeyCode::F10, ModifierState::default(), &config);
    assert_eq!(engine.take_atf_toggle(), Some(AtfToggleKind::Enabled));

    // 마지막 매칭 시각을 400ms 과거로 밀어 디바운스 창(300ms) 밖으로 만든다.
    let aged = std::time::Instant::now() - std::time::Duration::from_millis(400);
    engine.last_atf_hotkey = Some((KeyCode::F10, aged));

    engine.press_key(KeyCode::F10, ModifierState::default(), &config);
    assert_eq!(
        engine.take_atf_toggle(),
        Some(AtfToggleKind::Enabled),
        "디바운스 창 경과 후 재입력은 정상 토글되어야 함"
    );
}

#[test]
fn empty_default_config_never_toggles() {
    // 옵트인 — 기본 빈 목록에서는 어떤 키도 ATF 토글을 적재하지 않아야 한다(무회귀).
    let config = Config::default();
    let mut engine = InputEngine::new(&config);
    for key in [KeyCode::F10, KeyCode::F11, KeyCode::F12, KeyCode::A] {
        engine.press_key(key, ModifierState::default(), &config);
        assert_eq!(
            engine.take_atf_toggle(),
            None,
            "빈 목록에서는 {key:?} 가 ATF 토글을 적재하면 안 됨"
        );
    }
}

#[test]
fn set_atf_hotkeys_reapplies_from_config() {
    // 기본(빈 목록) 엔진 → F10 무동작. reload 재적용 후엔 F10 이 전체 토글로 동작.
    let mut engine = InputEngine::new(&Config::default());
    let empty = Config::default();
    engine.press_key(KeyCode::F10, ModifierState::default(), &empty);
    assert_eq!(engine.take_atf_toggle(), None);

    let reloaded = config_with_atf_hotkeys(&["F10"], &[], &[]);
    engine.set_atf_hotkeys(&reloaded);
    assert!(engine.is_atf_hotkey(KeyCode::F10));

    let r = engine.press_key(KeyCode::F10, ModifierState::default(), &reloaded);
    assert!(r.consumed);
    assert_eq!(engine.take_atf_toggle(), Some(AtfToggleKind::Enabled));
}
