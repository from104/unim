//! 키코드 정의 모듈
//!
//! 물리적 키보드 키코드와 수정자 키 상태를 정의합니다.
//! X11 및 Wayland 키코드와의 변환을 지원합니다.
//!
//! # 모듈 구성
//!
//! - [`conversion`]: 플랫폼별 키코드 변환 (X11/evdev/Win32) 및 레이아웃 매핑
//! - [`modifiers`]: [`ModifierState`] — Shift/Ctrl/Alt/Super 상태
//! - 본 파일: [`KeyCode`] enum 정의 + 분류 헬퍼 + `Display` 구현
//!
//! 외부 노출 심볼은 `pub use` 로 평탄화하여 기존 `crate::keycode::KeyCode`,
//! `crate::keycode::ModifierState` 경로 호환성을 유지한다.

use std::fmt;

mod conversion;
mod modifiers;

#[cfg(test)]
mod tests;

pub use modifiers::{ModifierState, UNIM_KEY_REPEAT_MASK, UNIM_REPEAT_AWARE_MASK};

/// 키보드 키코드 열거형
///
/// 물리적 키보드 키를 추상화하여 다양한 프론트엔드에서 통일된 방식으로 사용할 수 있도록 합니다.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
#[repr(u16)]
pub enum KeyCode {
    // 알파벳 키 (A-Z)
    A = 0x04,
    B = 0x05,
    C = 0x06,
    D = 0x07,
    E = 0x08,
    F = 0x09,
    G = 0x0A,
    H = 0x0B,
    I = 0x0C,
    J = 0x0D,
    K = 0x0E,
    L = 0x0F,
    M = 0x10,
    N = 0x11,
    O = 0x12,
    P = 0x13,
    Q = 0x14,
    R = 0x15,
    S = 0x16,
    T = 0x17,
    U = 0x18,
    V = 0x19,
    W = 0x1A,
    X = 0x1B,
    Y = 0x1C,
    Z = 0x1D,

    // 숫자 키 (0-9)
    Num1 = 0x1E,
    Num2 = 0x1F,
    Num3 = 0x20,
    Num4 = 0x21,
    Num5 = 0x22,
    Num6 = 0x23,
    Num7 = 0x24,
    Num8 = 0x25,
    Num9 = 0x26,
    Num0 = 0x27,

    // 특수 키
    Enter = 0x28,
    Escape = 0x29,
    Backspace = 0x2A,
    Tab = 0x2B,
    Space = 0x2C,

    // 기호 키
    Minus = 0x2D,        // - _
    Equal = 0x2E,        // = +
    BracketLeft = 0x2F,  // [ {
    BracketRight = 0x30, // ] }
    Backslash = 0x31,    // \ |
    Semicolon = 0x33,    // ; :
    Quote = 0x34,        // ' "
    Backquote = 0x35,    // ` ~
    Comma = 0x36,        // , <
    Period = 0x37,       // . >
    Slash = 0x38,        // / ?

    // 기능 키
    CapsLock = 0x39,
    F1 = 0x3A,
    F2 = 0x3B,
    F3 = 0x3C,
    F4 = 0x3D,
    F5 = 0x3E,
    F6 = 0x3F,
    F7 = 0x40,
    F8 = 0x41,
    F9 = 0x42,
    F10 = 0x43,
    F11 = 0x44,
    F12 = 0x45,

    // 편집 키
    Insert = 0x49,
    Home = 0x4A,
    PageUp = 0x4B,
    Delete = 0x4C,
    End = 0x4D,
    PageDown = 0x4E,

    // 화살표 키
    Right = 0x4F,
    Left = 0x50,
    Down = 0x51,
    Up = 0x52,

    // 한국어/한자 키
    Korean = 0x90, // 한/영 전환
    Hanja = 0x91,  // 한자 변환

    // 수정자 키
    LeftControl = 0xE0,
    LeftShift = 0xE1,
    LeftAlt = 0xE2,
    LeftSuper = 0xE3,
    RightControl = 0xE4,
    RightShift = 0xE5,
    RightAlt = 0xE6,
    RightSuper = 0xE7,

    // 알 수 없는 키
    #[default]
    Unknown = 0xFFFF,
}

impl KeyCode {
    /// 해당 KeyCode가 문자 입력 키인지 확인합니다.
    pub fn is_character_key(&self) -> bool {
        self.to_char().is_some()
    }

    /// 해당 KeyCode가 수정자 키인지 확인합니다.
    pub fn is_modifier(&self) -> bool {
        matches!(
            self,
            KeyCode::LeftControl
                | KeyCode::LeftShift
                | KeyCode::LeftAlt
                | KeyCode::LeftSuper
                | KeyCode::RightControl
                | KeyCode::RightShift
                | KeyCode::RightAlt
                | KeyCode::RightSuper
        )
    }

    /// 해당 KeyCode가 알파벳 키(A-Z)인지 확인합니다.
    pub fn is_alpha(&self) -> bool {
        matches!(
            self,
            KeyCode::A
                | KeyCode::B
                | KeyCode::C
                | KeyCode::D
                | KeyCode::E
                | KeyCode::F
                | KeyCode::G
                | KeyCode::H
                | KeyCode::I
                | KeyCode::J
                | KeyCode::K
                | KeyCode::L
                | KeyCode::M
                | KeyCode::N
                | KeyCode::O
                | KeyCode::P
                | KeyCode::Q
                | KeyCode::R
                | KeyCode::S
                | KeyCode::T
                | KeyCode::U
                | KeyCode::V
                | KeyCode::W
                | KeyCode::X
                | KeyCode::Y
                | KeyCode::Z
        )
    }
}

impl fmt::Display for KeyCode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?}", self)
    }
}
