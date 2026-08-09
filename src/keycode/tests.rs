//! [`KeyCode`] / [`ModifierState`] 통합 테스트.
//!
//! 분할 전 `src/keycode.rs` 의 `mod tests` 블록을 그대로 옮긴 것으로,
//! 외부 동작 변경이 없음을 보장한다.

use super::*;

#[test]
fn test_keycode_from_evdev() {
    assert_eq!(KeyCode::from_evdev_keycode(30), KeyCode::A);
    assert_eq!(KeyCode::from_evdev_keycode(48), KeyCode::B);
    assert_eq!(KeyCode::from_evdev_keycode(2), KeyCode::Num1);
    assert_eq!(KeyCode::from_evdev_keycode(28), KeyCode::Enter);
    assert_eq!(KeyCode::from_evdev_keycode(57), KeyCode::Space);
    assert_eq!(KeyCode::from_evdev_keycode(9999), KeyCode::Unknown);
}

#[test]
fn test_keycode_from_x11() {
    // X11 keycode = evdev + 8
    assert_eq!(KeyCode::from_x11_keycode(38), KeyCode::A); // 30 + 8
    assert_eq!(KeyCode::from_x11_keycode(56), KeyCode::B); // 48 + 8
}

#[test]
fn test_keycode_to_char() {
    assert_eq!(KeyCode::A.to_char(), Some('a'));
    assert_eq!(KeyCode::A.to_shifted_char(), Some('A'));
    assert_eq!(KeyCode::Num1.to_char(), Some('1'));
    assert_eq!(KeyCode::Num1.to_shifted_char(), Some('!'));
    assert_eq!(KeyCode::Enter.to_char(), None);
}

#[test]
fn test_modifier_state() {
    let empty = ModifierState::new();
    assert!(empty.is_empty());

    let shift_only = ModifierState {
        shift: true,
        ..Default::default()
    };
    assert!(shift_only.is_shift_only());
    assert!(!shift_only.is_empty());

    let from_x11 = ModifierState::from_x11_mask(0b0101); // Shift + Control
    assert!(from_x11.shift);
    assert!(from_x11.control);
    assert!(!from_x11.alt);
}

#[test]
fn test_keycode_from_win32_vk() {
    // 알파벳
    assert_eq!(KeyCode::from_win32_vk(0x41), KeyCode::A);
    assert_eq!(KeyCode::from_win32_vk(0x5A), KeyCode::Z);
    assert_eq!(KeyCode::from_win32_vk(0x48), KeyCode::H);

    // 숫자
    assert_eq!(KeyCode::from_win32_vk(0x30), KeyCode::Num0);
    assert_eq!(KeyCode::from_win32_vk(0x31), KeyCode::Num1);
    assert_eq!(KeyCode::from_win32_vk(0x39), KeyCode::Num9);

    // 특수 키
    assert_eq!(KeyCode::from_win32_vk(0x0D), KeyCode::Enter);
    assert_eq!(KeyCode::from_win32_vk(0x1B), KeyCode::Escape);
    assert_eq!(KeyCode::from_win32_vk(0x08), KeyCode::Backspace);
    assert_eq!(KeyCode::from_win32_vk(0x09), KeyCode::Tab);
    assert_eq!(KeyCode::from_win32_vk(0x20), KeyCode::Space);

    // OEM 기호 키
    assert_eq!(KeyCode::from_win32_vk(0xBD), KeyCode::Minus);
    assert_eq!(KeyCode::from_win32_vk(0xBB), KeyCode::Equal);
    assert_eq!(KeyCode::from_win32_vk(0xDB), KeyCode::BracketLeft);
    assert_eq!(KeyCode::from_win32_vk(0xDD), KeyCode::BracketRight);
    assert_eq!(KeyCode::from_win32_vk(0xDC), KeyCode::Backslash);
    assert_eq!(KeyCode::from_win32_vk(0xBA), KeyCode::Semicolon);
    assert_eq!(KeyCode::from_win32_vk(0xDE), KeyCode::Quote);
    assert_eq!(KeyCode::from_win32_vk(0xC0), KeyCode::Backquote);
    assert_eq!(KeyCode::from_win32_vk(0xBC), KeyCode::Comma);
    assert_eq!(KeyCode::from_win32_vk(0xBE), KeyCode::Period);
    assert_eq!(KeyCode::from_win32_vk(0xBF), KeyCode::Slash);

    // 기능 키
    assert_eq!(KeyCode::from_win32_vk(0x70), KeyCode::F1);
    assert_eq!(KeyCode::from_win32_vk(0x7B), KeyCode::F12);
    assert_eq!(KeyCode::from_win32_vk(0x14), KeyCode::CapsLock);

    // 편집/화살표 키
    assert_eq!(KeyCode::from_win32_vk(0x2D), KeyCode::Insert);
    assert_eq!(KeyCode::from_win32_vk(0x2E), KeyCode::Delete);
    assert_eq!(KeyCode::from_win32_vk(0x24), KeyCode::Home);
    assert_eq!(KeyCode::from_win32_vk(0x23), KeyCode::End);
    assert_eq!(KeyCode::from_win32_vk(0x27), KeyCode::Right);
    assert_eq!(KeyCode::from_win32_vk(0x25), KeyCode::Left);
    assert_eq!(KeyCode::from_win32_vk(0x26), KeyCode::Up);
    assert_eq!(KeyCode::from_win32_vk(0x28), KeyCode::Down);

    // 한국어/한자
    assert_eq!(KeyCode::from_win32_vk(0x15), KeyCode::Korean);
    assert_eq!(KeyCode::from_win32_vk(0x19), KeyCode::Hanja);

    // 수정자
    assert_eq!(KeyCode::from_win32_vk(0xA0), KeyCode::LeftShift);
    assert_eq!(KeyCode::from_win32_vk(0xA1), KeyCode::RightShift);
    assert_eq!(KeyCode::from_win32_vk(0xA2), KeyCode::LeftControl);
    assert_eq!(KeyCode::from_win32_vk(0xA5), KeyCode::RightAlt);
    assert_eq!(KeyCode::from_win32_vk(0x5B), KeyCode::LeftSuper);

    // 제네릭 수정자 VK (TSF/IMM32 키다운에서 실제 전달되는 값) — 좌측으로 매핑돼야
    // is_modifier()=true 가 되어 조합 중 수정자 키다운이 음절을 깨지 않는다.
    assert_eq!(KeyCode::from_win32_vk(0x10), KeyCode::LeftShift); // VK_SHIFT
    assert_eq!(KeyCode::from_win32_vk(0x11), KeyCode::LeftControl); // VK_CONTROL
    assert_eq!(KeyCode::from_win32_vk(0x12), KeyCode::LeftAlt); // VK_MENU

    // 알 수 없는 키
    assert_eq!(KeyCode::from_win32_vk(0xFFFF), KeyCode::Unknown);
}

#[test]
fn test_modifier_from_win32() {
    // 빈 상태
    let empty = ModifierState::from_win32_modifiers(0);
    assert!(empty.is_empty());
    assert!(!empty.caps_lock);

    // Shift만
    let shift_only = ModifierState::from_win32_modifiers(0x01);
    assert!(shift_only.shift);
    assert!(!shift_only.control);
    assert!(shift_only.is_shift_only());

    // Shift + Control
    let shift_ctrl = ModifierState::from_win32_modifiers(0x03);
    assert!(shift_ctrl.shift);
    assert!(shift_ctrl.control);
    assert!(!shift_ctrl.alt);

    // Alt만
    let alt_only = ModifierState::from_win32_modifiers(0x04);
    assert!(alt_only.alt);
    assert!(alt_only.is_alt_only());

    // CapsLock + NumLock
    let locks = ModifierState::from_win32_modifiers(0x30);
    assert!(locks.caps_lock);
    assert!(locks.num_lock);
    assert!(!locks.shift);

    // 전체 비트
    let all = ModifierState::from_win32_modifiers(0x3F);
    assert!(all.shift);
    assert!(all.control);
    assert!(all.alt);
    assert!(all.super_key);
    assert!(all.caps_lock);
    assert!(all.num_lock);
}

#[test]
fn test_keycode_is_modifier() {
    assert!(KeyCode::LeftShift.is_modifier());
    assert!(KeyCode::RightAlt.is_modifier());
    assert!(!KeyCode::A.is_modifier());
    assert!(!KeyCode::Space.is_modifier());
}

/// 제네릭 수정자 VK 키다운(Windows TSF/IMM32 가 실제 전달)은 is_modifier()=true 여야
/// 한다. 안 그러면 TSF modifier-combo 가드가 조합 중 한글을 잘못 커밋한다(세벌식
/// 시프트-자모 분리 버그의 근본). 또 character_key 가 아니어야 한다.
#[test]
fn test_generic_modifier_vk_is_modifier() {
    for vk in [0x10u16, 0x11, 0x12] {
        let kc = KeyCode::from_win32_vk(vk);
        assert!(kc.is_modifier(), "generic VK 0x{vk:02X} must be modifier");
        assert!(
            !kc.is_character_key(),
            "generic VK 0x{vk:02X} must not be a character key"
        );
    }
}

// ── to_char_for_layout 테스트 ──

#[test]
fn test_to_char_for_layout_qwerty_consistency() {
    // Qwerty: to_char_for_layout == to_char / to_shifted_char (모든 문자키)
    let all_char_keys = [
        KeyCode::A,
        KeyCode::B,
        KeyCode::C,
        KeyCode::D,
        KeyCode::E,
        KeyCode::F,
        KeyCode::G,
        KeyCode::H,
        KeyCode::I,
        KeyCode::J,
        KeyCode::K,
        KeyCode::L,
        KeyCode::M,
        KeyCode::N,
        KeyCode::O,
        KeyCode::P,
        KeyCode::Q,
        KeyCode::R,
        KeyCode::S,
        KeyCode::T,
        KeyCode::U,
        KeyCode::V,
        KeyCode::W,
        KeyCode::X,
        KeyCode::Y,
        KeyCode::Z,
        KeyCode::Num0,
        KeyCode::Num1,
        KeyCode::Num2,
        KeyCode::Num3,
        KeyCode::Num4,
        KeyCode::Num5,
        KeyCode::Num6,
        KeyCode::Num7,
        KeyCode::Num8,
        KeyCode::Num9,
        KeyCode::Minus,
        KeyCode::Equal,
        KeyCode::BracketLeft,
        KeyCode::BracketRight,
        KeyCode::Backslash,
        KeyCode::Semicolon,
        KeyCode::Quote,
        KeyCode::Backquote,
        KeyCode::Comma,
        KeyCode::Period,
        KeyCode::Slash,
        KeyCode::Space,
    ];
    for key in all_char_keys {
        assert_eq!(
            key.to_char_for_layout("qwerty", false),
            key.to_char(),
            "Qwerty lower mismatch for {:?}",
            key
        );
        assert_eq!(
            key.to_char_for_layout("qwerty", true),
            key.to_shifted_char(),
            "Qwerty upper mismatch for {:?}",
            key
        );
    }
}

/// JSON 키맵에서 기대값을 읽어 to_char_for_layout() 결과와 비교하는 포괄적 테스트.
/// 모든 비-QWERTY 레이아웃의 모든 물리키를 JSON 원본과 교차 검증한다.
#[test]
fn test_to_char_for_layout_all_non_qwerty_vs_json() {
    use crate::keystroke::get_keymap_json;

    // 물리키 행별 배열 (QWERTY 물리 위치 순서)
    let row_keys: [&[KeyCode]; 4] = [
        &[
            KeyCode::Backquote,
            KeyCode::Num1,
            KeyCode::Num2,
            KeyCode::Num3,
            KeyCode::Num4,
            KeyCode::Num5,
            KeyCode::Num6,
            KeyCode::Num7,
            KeyCode::Num8,
            KeyCode::Num9,
            KeyCode::Num0,
            KeyCode::Minus,
            KeyCode::Equal,
            KeyCode::Backslash,
        ],
        &[
            KeyCode::Q,
            KeyCode::W,
            KeyCode::E,
            KeyCode::R,
            KeyCode::T,
            KeyCode::Y,
            KeyCode::U,
            KeyCode::I,
            KeyCode::O,
            KeyCode::P,
            KeyCode::BracketLeft,
            KeyCode::BracketRight,
        ],
        &[
            KeyCode::A,
            KeyCode::S,
            KeyCode::D,
            KeyCode::F,
            KeyCode::G,
            KeyCode::H,
            KeyCode::J,
            KeyCode::K,
            KeyCode::L,
            KeyCode::Semicolon,
            KeyCode::Quote,
        ],
        &[
            KeyCode::Z,
            KeyCode::X,
            KeyCode::C,
            KeyCode::V,
            KeyCode::B,
            KeyCode::N,
            KeyCode::M,
            KeyCode::Comma,
            KeyCode::Period,
            KeyCode::Slash,
        ],
    ];
    let row_names = ["1st", "2nd", "3rd", "4th"];

    let layouts = ["dvorak", "colemak", "colemak_dh", "workman"];

    for layout in layouts {
        let keymap_file = crate::config::english_layout_keymap_name(layout);
        let json_str = get_keymap_json(&keymap_file);
        let json: serde_json::Value = serde_json::from_str(json_str).unwrap();
        let lower_layout = &json["layout"]["lower"];
        let upper_layout = &json["layout"]["upper"];

        for (row_idx, row_name) in row_names.iter().enumerate() {
            let lower_row = lower_layout[row_name].as_array().unwrap();
            let upper_row = upper_layout[row_name].as_array().unwrap();
            let keys = row_keys[row_idx];

            for (col_idx, &keycode) in keys.iter().enumerate() {
                let expected_lower = lower_row[col_idx].as_str().unwrap().chars().next().unwrap();
                let expected_upper = upper_row[col_idx].as_str().unwrap().chars().next().unwrap();

                assert_eq!(
                    keycode.to_char_for_layout(layout, false),
                    Some(expected_lower),
                    "{:?} {:?} lower: expected '{}' from JSON",
                    layout,
                    keycode,
                    expected_lower
                );
                assert_eq!(
                    keycode.to_char_for_layout(layout, true),
                    Some(expected_upper),
                    "{:?} {:?} upper: expected '{}' from JSON",
                    layout,
                    keycode,
                    expected_upper
                );
            }
        }
    }
}

#[test]
fn test_to_char_for_layout_space_all_layouts() {
    for layout in crate::config::ENGLISH_LAYOUT_BUILTINS {
        assert_eq!(
            KeyCode::Space.to_char_for_layout(layout, false),
            Some(' '),
            "Space should be ' ' for {layout}"
        );
    }
}
