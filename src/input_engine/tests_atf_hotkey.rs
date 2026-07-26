//! AutoTypeFix 토글 단축키(`atf_hotkeys_*`) 매칭 회귀 테스트.
//!
//! ATF 핫키는 `[수정자+]* <KeyName>` 표기를 **수정자 정확 일치**로 매칭한다: 표기에
//! 등장한 수정자는 눌려 있어야 하고, 등장하지 않은 수정자는 눌리면 안 된다. 매칭 시
//! config·`InputResult` 를 건드리지 않고 엔진의 `pending_atf_toggle` 에 대상 플래그만
//! 적재한다(호스트가 `take_atf_toggle()` 로 드레인). 이 모듈은 ① 각 kind 매칭→소비+
//! 드레인 1회성, ② 설정에 없는 수정자 조합의 통과(미소비 — 앱 단축키 보호), ③ 조합
//! 표기(`Shift+F9`)의 매칭과 base 키(맨 `F9`) 미매칭, ④ 파서 규칙(대소문자·순서
//! 무관, `-` 폴백 구분자, 잘못된 표기 거부), ⑤ 기본값 `Shift+F8`, ⑥ 오토리핏
//! 디바운스, ⑦ `set_atf_hotkeys` 재적용을 고정한다.

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

/// 핫키를 전부 비운 config — opt-out(무동작) 경로 검증용.
fn config_without_atf_hotkeys() -> Config {
    config_with_atf_hotkeys(&[], &[], &[])
}

/// 지정한 수정자 하나만 눌린 상태.
fn mods_shift() -> ModifierState {
    ModifierState {
        shift: true,
        ..ModifierState::default()
    }
}

#[test]
fn is_atf_hotkey_reports_configured_keys() {
    let config = config_with_atf_hotkeys(&["F10"], &["F11"], &["F12"]);
    let engine = InputEngine::new(&config);
    let bare = ModifierState::default();
    assert!(engine.is_atf_hotkey(KeyCode::F10, bare));
    assert!(engine.is_atf_hotkey(KeyCode::F11, bare));
    assert!(engine.is_atf_hotkey(KeyCode::F12, bare));
    assert!(!engine.is_atf_hotkey(KeyCode::A, bare));
    // 빈 목록 엔진은 어떤 키도 ATF 핫키가 아니다.
    let plain = InputEngine::new(&config_without_atf_hotkeys());
    assert!(!plain.is_atf_hotkey(KeyCode::F10, bare));
}

#[test]
fn is_atf_hotkey_matches_exact_modifiers() {
    // 소비(test) 판정도 press_key 와 같은 **수정자 정확 일치**다. 이 판정을 쓰는
    // Windows TSF/IMM32 가 조합 표기를 test 단계에서 소비해야 press_key 에 도달해
    // Linux 와 동일하게 동작한다. 반대로 조합의 맨 base 키(맨 F8, 한자키 맨 F9)는
    // 소비되지 않아야 원래 기능(한자 변환 등)이 보존된다.
    let config = config_with_atf_hotkeys(&["Shift+F9"], &[], &[]);
    let engine = InputEngine::new(&config);
    assert!(
        engine.is_atf_hotkey(KeyCode::F9, mods_shift()),
        "조합 표기는 수정자가 함께 눌린 때 소비 대상이어야 함"
    );
    assert!(
        !engine.is_atf_hotkey(KeyCode::F9, ModifierState::default()),
        "조합 표기의 맨 base 키는 소비 대상이 아니어야 함(맨 F9 한자 보존)"
    );

    // 기본값(Shift+F8): 조합만 소비, 맨 F8·맨 F9 는 비소비 — Windows 무회귀.
    let default_engine = InputEngine::new(&Config::default());
    assert!(default_engine.is_atf_hotkey(KeyCode::F8, mods_shift()));
    assert!(!default_engine.is_atf_hotkey(KeyCode::F8, ModifierState::default()));
    assert!(!default_engine.is_atf_hotkey(KeyCode::F9, mods_shift()));
    assert!(!default_engine.is_atf_hotkey(KeyCode::F9, ModifierState::default()));
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

// ─────────────────────────────────────────────
// 설정에 없는 조합은 통과 — 정확-일치가 앱 단축키를 구조적으로 보호
// ─────────────────────────────────────────────

#[test]
fn unconfigured_ctrl_combo_passes_through() {
    // `F10` 만 등록 → Ctrl+F10 은 정확-일치 실패로 통과(ATF 토글 X, 소비 X).
    let config = config_with_atf_hotkeys(&["F10"], &[], &[]);
    let mut engine = InputEngine::new(&config);

    let mods = ModifierState {
        control: true,
        ..ModifierState::default()
    };
    let r = engine.press_key(KeyCode::F10, mods, &config);
    assert!(!r.consumed, "설정에 없는 Ctrl 조합은 단축키 — 통과");
    assert_eq!(
        engine.take_atf_toggle(),
        None,
        "미설정 조합은 ATF 토글이 적재되지 않아야 함"
    );
}

#[test]
fn unconfigured_super_combo_passes_through() {
    let config = config_with_atf_hotkeys(&["F10"], &[], &[]);
    let mut engine = InputEngine::new(&config);

    let mods = ModifierState {
        super_key: true,
        ..ModifierState::default()
    };
    let r = engine.press_key(KeyCode::F10, mods, &config);
    assert!(!r.consumed, "설정에 없는 Super 조합은 단축키 — 통과");
    assert_eq!(engine.take_atf_toggle(), None);
}

#[test]
fn unconfigured_shift_combo_passes_through() {
    // `F10` 만 등록 → Shift+F10(컨텍스트 메뉴)은 앱 단축키로 통과.
    // 종전에는 `shortcut_combo` 일괄 가드가 막았고, 지금은 정확-일치가 대신한다.
    let config = config_with_atf_hotkeys(&["F10"], &[], &[]);
    let mut engine = InputEngine::new(&config);

    let r = engine.press_key(KeyCode::F10, mods_shift(), &config);
    assert!(!r.consumed, "설정에 없는 Shift 조합은 앱 단축키 — 통과(미소비)");
    assert_eq!(engine.take_atf_toggle(), None);
}

// ─────────────────────────────────────────────
// 조합 표기(`Shift+F9`) 매칭
// ─────────────────────────────────────────────

#[test]
fn shift_combo_hotkey_matches_and_consumes() {
    // (a) Shift+F9 → 토글 적재 + 소비.
    let config = config_with_atf_hotkeys(&["Shift+F9"], &[], &[]);
    let mut engine = InputEngine::new(&config);

    let r = engine.press_key(KeyCode::F9, mods_shift(), &config);
    assert!(r.consumed, "설정된 Shift+F9 조합은 소비되어야 함");
    assert_eq!(engine.take_atf_toggle(), Some(AtfToggleKind::Enabled));
}

#[test]
fn bare_key_does_not_match_shift_combo_hotkey() {
    // (b) 맨 F9 → 정확-일치 실패(shift 미충족) → ATF 미매칭.
    // ATF 검사가 한자 분기보다 앞서지만 F9 는 한자/이모지 경로로 그대로 내려간다.
    let config = config_with_atf_hotkeys(&["Shift+F9"], &[], &[]);
    let mut engine = InputEngine::new(&config);

    engine.press_key(KeyCode::F9, ModifierState::default(), &config);
    assert_eq!(
        engine.take_atf_toggle(),
        None,
        "맨 F9 는 Shift+F9 핫키에 매칭되면 안 됨(한자 트리거로 양보)"
    );
}

#[test]
fn extra_modifier_does_not_match_shift_combo_hotkey() {
    // (c) Ctrl+Shift+F9 → 지정하지 않은 Ctrl 이 눌렸으므로 미매칭 → 통과.
    let config = config_with_atf_hotkeys(&["Shift+F9"], &[], &[]);
    let mut engine = InputEngine::new(&config);

    let mods = ModifierState {
        shift: true,
        control: true,
        ..ModifierState::default()
    };
    let r = engine.press_key(KeyCode::F9, mods, &config);
    assert!(!r.consumed, "미지정 수정자가 추가된 조합은 통과해야 함");
    assert_eq!(engine.take_atf_toggle(), None);
}

#[test]
fn multi_modifier_hotkey_matches_exactly() {
    // Ctrl+Shift+F8 등록 → 정확히 그 조합에서만 매칭.
    let config = config_with_atf_hotkeys(&["Ctrl+Shift+F8"], &[], &[]);
    let mut engine = InputEngine::new(&config);

    let exact = ModifierState {
        control: true,
        shift: true,
        ..ModifierState::default()
    };
    let r = engine.press_key(KeyCode::F8, exact, &config);
    assert!(r.consumed);
    assert_eq!(engine.take_atf_toggle(), Some(AtfToggleKind::Enabled));

    // 수정자 하나가 빠지면 미매칭.
    let partial = ModifierState {
        control: true,
        ..ModifierState::default()
    };
    let r = engine.press_key(KeyCode::F8, partial, &config);
    assert!(!r.consumed, "수정자 부분 일치는 매칭되면 안 됨");
    assert_eq!(engine.take_atf_toggle(), None);
}

// ─────────────────────────────────────────────
// 파서 단위 테스트
// ─────────────────────────────────────────────

#[test]
fn parser_accepts_modifiers_case_and_order_insensitively() {
    // 같은 조합을 여러 표기로 등록해도 모두 Shift+Ctrl+F8 로 파싱된다.
    let mods = ModifierState {
        control: true,
        shift: true,
        ..ModifierState::default()
    };
    for spec in [
        "Ctrl+Shift+F8",
        "shift+ctrl+F8",
        "CONTROL+SHIFT+F8",
        "Shift+Control+F8",
        // 토큰별 공백 허용 + 중복 멱등.
        " ctrl + shift + F8 ",
        "Ctrl+Ctrl+Shift+F8",
    ] {
        let config = config_with_atf_hotkeys(&[spec], &[], &[]);
        let mut engine = InputEngine::new(&config);
        let r = engine.press_key(KeyCode::F8, mods, &config);
        assert!(r.consumed, "'{spec}' 표기가 매칭되어야 함");
        assert_eq!(
            engine.take_atf_toggle(),
            Some(AtfToggleKind::Enabled),
            "'{spec}' 표기가 Enabled 토글을 적재해야 함"
        );
    }
}

#[test]
fn parser_accepts_super_aliases() {
    let mods = ModifierState {
        super_key: true,
        ..ModifierState::default()
    };
    for spec in ["Super+F8", "Win+F8", "Meta+F8"] {
        let config = config_with_atf_hotkeys(&[spec], &[], &[]);
        let mut engine = InputEngine::new(&config);
        engine.press_key(KeyCode::F8, mods, &config);
        assert_eq!(
            engine.take_atf_toggle(),
            Some(AtfToggleKind::Enabled),
            "'{spec}' 은 Super 별칭으로 인식되어야 함"
        );
    }
}

#[test]
fn parser_accepts_hyphen_separator_fallback() {
    // '+' 가 전혀 없는 표기는 '-' 를 대체 구분자로 허용한다("Ctrl-Left" 관용 표기).
    let mods_ctrl = ModifierState {
        control: true,
        ..ModifierState::default()
    };
    for (spec, key, mods) in [
        ("Ctrl-Left", KeyCode::Left, mods_ctrl),
        ("Shift-F8", KeyCode::F8, mods_shift()),
    ] {
        let config = config_with_atf_hotkeys(&[spec], &[], &[]);
        let mut engine = InputEngine::new(&config);
        let r = engine.press_key(key, mods, &config);
        assert!(r.consumed, "'{spec}' 하이픈 표기가 매칭되어야 함");
        assert_eq!(
            engine.take_atf_toggle(),
            Some(AtfToggleKind::Enabled),
            "'{spec}' 하이픈 표기가 Enabled 토글을 적재해야 함"
        );
    }

    // 다중 수정자 + 대소문자 무관도 '+' 경로와 동일하게 동작.
    let config = config_with_atf_hotkeys(&["ctrl-shift-F8"], &[], &[]);
    let mut engine = InputEngine::new(&config);
    engine.press_key(
        KeyCode::F8,
        ModifierState {
            control: true,
            shift: true,
            ..ModifierState::default()
        },
        &config,
    );
    assert_eq!(engine.take_atf_toggle(), Some(AtfToggleKind::Enabled));

    // 혼용 표기는 '+' 분할이 우선이라 base "Shift-F8" 이 미지 이름 → 종전대로 배제.
    let config = config_with_atf_hotkeys(&["Ctrl+Shift-F8"], &[], &[]);
    let engine = InputEngine::new(&config);
    assert!(engine.atf_hotkeys_enabled.is_empty());

    // bare 표기는 '-' 폴백 경로에서도 무연산(분할 결과 1토큰) — 하위 호환.
    let config = config_with_atf_hotkeys(&["F10"], &[], &[]);
    let engine = InputEngine::new(&config);
    assert!(engine.is_atf_hotkey(KeyCode::F10, ModifierState::default()));
}

#[test]
fn parser_rejects_malformed_specs() {
    // 미지 modifier 토큰 / 빈 base / 미지 base / 대소문자 틀린 base / 수정자 base.
    for spec in [
        "Hyper+F8",   // 미지 modifier
        "Shift+",     // 빈 base
        "Shift+Nope", // 미지 base
        "Shift+f9",   // base 는 대소문자 구분
        "NotAKey",    // 미지 단일 base
        "",           // 빈 표기
    ] {
        let config = config_with_atf_hotkeys(&[spec], &[], &[]);
        let engine = InputEngine::new(&config);
        assert!(
            engine.atf_hotkeys_enabled.is_empty(),
            "'{spec}' 은 파싱 거부되어 목록이 비어야 함"
        );
    }
}

#[test]
fn modifier_key_is_excluded_from_atf_hotkeys() {
    // 수정자 키(RightAlt 등)를 base 로 지정해도 파싱 단계에서 배제된다 —
    // press_key 의 is_modifier() 조기 반환과 정합(dead key 방지).
    let config = config_with_atf_hotkeys(&["RightAlt"], &["LeftControl"], &["Ctrl+LeftShift"]);
    let engine = InputEngine::new(&config);
    let bare = ModifierState::default();
    assert!(!engine.is_atf_hotkey(KeyCode::RightAlt, bare));
    assert!(!engine.is_atf_hotkey(KeyCode::LeftControl, bare));
    assert!(!engine.is_atf_hotkey(KeyCode::LeftShift, bare));
    assert!(engine.atf_hotkeys_enabled.is_empty());
    assert!(engine.atf_hotkeys_forward.is_empty());
    assert!(engine.atf_hotkeys_reverse.is_empty());
}

#[test]
fn bare_spec_stays_backward_compatible() {
    // 수정자 없는 기존 표기는 모든 수정자가 떼어졌을 때만 매칭 — 종전 동작 동일.
    let config = config_with_atf_hotkeys(&["F10"], &[], &[]);
    let mut engine = InputEngine::new(&config);

    let r = engine.press_key(KeyCode::F10, ModifierState::default(), &config);
    assert!(r.consumed);
    assert_eq!(engine.take_atf_toggle(), Some(AtfToggleKind::Enabled));
}

// ─────────────────────────────────────────────
// 오토리핏 디바운스 (기존 로직 유지)
// ─────────────────────────────────────────────

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
fn autorepeat_debounce_applies_to_modifier_combo() {
    // 조합 핫키도 동일하게 디바운스된다(디바운스는 키코드 기준).
    let config = config_with_atf_hotkeys(&["Shift+F9"], &[], &[]);
    let mut engine = InputEngine::new(&config);

    engine.press_key(KeyCode::F9, mods_shift(), &config);
    assert_eq!(engine.take_atf_toggle(), Some(AtfToggleKind::Enabled));

    let r = engine.press_key(KeyCode::F9, mods_shift(), &config);
    assert!(r.consumed);
    assert_eq!(engine.take_atf_toggle(), None);
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

// ─────────────────────────────────────────────
// 기본값 / opt-out / hot-reload
// ─────────────────────────────────────────────

#[test]
fn default_config_toggles_on_shift_f8() {
    // (e) 기본값은 전체 토글 `Shift+F8` (F9~F11 리매핑 소실 회피 — config.rs 참조).
    let config = Config::default();
    let mut engine = InputEngine::new(&config);

    let r = engine.press_key(KeyCode::F8, mods_shift(), &config);
    assert!(r.consumed, "기본값 Shift+F8 는 소비되어야 함");
    assert_eq!(engine.take_atf_toggle(), Some(AtfToggleKind::Enabled));
}

#[test]
fn default_config_bare_f8_is_not_atf() {
    // 기본값(Shift+F8)에서 맨 F8 은 정확-일치 불발로 ATF 가 아니다.
    // 맨 F9 도 종전대로 한자/이모지 트리거로 남는다.
    let config = Config::default();
    let mut engine = InputEngine::new(&config);

    for key in [KeyCode::F8, KeyCode::F9] {
        engine.press_key(key, ModifierState::default(), &config);
        assert_eq!(
            engine.take_atf_toggle(),
            None,
            "기본값에서 맨 {key:?} 는 ATF 토글이 아니어야 함"
        );
    }
}

#[test]
fn empty_lists_never_toggle() {
    // opt-out — 세 목록을 비우면 어떤 키도 ATF 토글을 적재하지 않는다.
    let config = config_without_atf_hotkeys();
    let mut engine = InputEngine::new(&config);
    for key in [KeyCode::F9, KeyCode::F10, KeyCode::F11, KeyCode::A] {
        engine.press_key(key, ModifierState::default(), &config);
        assert_eq!(
            engine.take_atf_toggle(),
            None,
            "빈 목록에서는 {key:?} 가 ATF 토글을 적재하면 안 됨"
        );
        engine.press_key(key, mods_shift(), &config);
        assert_eq!(
            engine.take_atf_toggle(),
            None,
            "빈 목록에서는 Shift+{key:?} 도 ATF 토글을 적재하면 안 됨"
        );
    }
}

#[test]
fn set_atf_hotkeys_reapplies_from_config() {
    // 빈 목록 엔진 → F10 무동작. reload 재적용 후엔 F10 이 전체 토글로 동작.
    let empty = config_without_atf_hotkeys();
    let mut engine = InputEngine::new(&empty);
    engine.press_key(KeyCode::F10, ModifierState::default(), &empty);
    assert_eq!(engine.take_atf_toggle(), None);

    let reloaded = config_with_atf_hotkeys(&["F10"], &[], &[]);
    engine.set_atf_hotkeys(&reloaded);
    assert!(engine.is_atf_hotkey(KeyCode::F10, ModifierState::default()));

    let r = engine.press_key(KeyCode::F10, ModifierState::default(), &reloaded);
    assert!(r.consumed);
    assert_eq!(engine.take_atf_toggle(), Some(AtfToggleKind::Enabled));
}

#[test]
fn set_atf_hotkeys_reapplies_modifier_combo() {
    // reload 로 조합 표기를 적용해도 정확-일치가 유지된다.
    let empty = config_without_atf_hotkeys();
    let mut engine = InputEngine::new(&empty);

    let reloaded = config_with_atf_hotkeys(&["Shift+F9"], &[], &[]);
    engine.set_atf_hotkeys(&reloaded);

    engine.press_key(KeyCode::F9, mods_shift(), &reloaded);
    assert_eq!(engine.take_atf_toggle(), Some(AtfToggleKind::Enabled));
}

#[test]
fn atf_hotkey_rejects_nonexistent_scrolllock() {
    // `ScrollLock`·`Pause` 는 `KeyCode::from_name` 에 아예 없는 이름이다(85개 전수 확인).
    // 종전 placeholder·CLI 경고 문구가 이 둘을 "권장 비입력 키"로 안내하고 있어,
    // 그대로 입력하면 검증이 무효로 판정한다. 문구 정정이 되돌아가지 않도록 코드로 고정.
    assert_eq!(InputEngine::parse_atf_hotkey("ScrollLock"), None);
    assert_eq!(InputEngine::parse_atf_hotkey("Pause"), None);
    // 대안으로 안내하는 비입력 키들은 실제로 유효해야 한다.
    for name in ["F1", "F12", "CapsLock", "Insert", "Home", "End", "PageUp", "PageDown"] {
        assert!(
            InputEngine::parse_atf_hotkey(name).is_some(),
            "'{name}' 는 유효한 ATF 핫키 base 여야 함"
        );
    }
    // 수정자 조합도 유효(커밋 7b7b751 — 양 플랫폼 지원 확인).
    assert!(InputEngine::parse_atf_hotkey("Shift+F9").is_some());
    assert!(InputEngine::parse_atf_hotkey("Ctrl-Left").is_some());
}
