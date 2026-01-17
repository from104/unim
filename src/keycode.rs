//! 키코드 정의 모듈
//!
//! 물리적 키보드 키코드와 수정자 키 상태를 정의합니다.
//! X11 및 Wayland 키코드와의 변환을 지원합니다.

use std::fmt;

/// 키보드 키코드 열거형
///
/// 물리적 키보드 키를 추상화하여 다양한 프론트엔드에서 통일된 방식으로 사용할 수 있도록 합니다.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
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
    Minus = 0x2D,         // - _
    Equal = 0x2E,         // = +
    BracketLeft = 0x2F,   // [ {
    BracketRight = 0x30,  // ] }
    Backslash = 0x31,     // \ |
    Semicolon = 0x33,     // ; :
    Quote = 0x34,         // ' "
    Backquote = 0x35,     // ` ~
    Comma = 0x36,         // , <
    Period = 0x37,        // . >
    Slash = 0x38,         // / ?

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

    // 한글/한자 키
    Hangul = 0x90,        // 한/영 전환
    Hanja = 0x91,         // 한자 변환

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
    Unknown = 0xFFFF,
}

impl KeyCode {
    /// X11 하드웨어 키코드에서 KeyCode로 변환합니다.
    ///
    /// X11 키코드는 일반적으로 evdev 키코드 + 8입니다.
    ///
    /// # Arguments
    ///
    /// * `code` - X11 하드웨어 키코드
    ///
    /// # Returns
    ///
    /// 변환된 `KeyCode`, 매핑되지 않은 경우 `KeyCode::Unknown`
    pub fn from_x11_keycode(code: u16) -> Self {
        // X11 keycode = evdev keycode + 8
        let evdev_code = code.saturating_sub(8);
        Self::from_evdev_keycode(evdev_code)
    }

    /// Wayland/evdev 키코드에서 KeyCode로 변환합니다.
    ///
    /// # Arguments
    ///
    /// * `code` - evdev 키코드
    ///
    /// # Returns
    ///
    /// 변환된 `KeyCode`, 매핑되지 않은 경우 `KeyCode::Unknown`
    pub fn from_evdev_keycode(code: u16) -> Self {
        match code {
            // 알파벳
            30 => KeyCode::A,
            48 => KeyCode::B,
            46 => KeyCode::C,
            32 => KeyCode::D,
            18 => KeyCode::E,
            33 => KeyCode::F,
            34 => KeyCode::G,
            35 => KeyCode::H,
            23 => KeyCode::I,
            36 => KeyCode::J,
            37 => KeyCode::K,
            38 => KeyCode::L,
            50 => KeyCode::M,
            49 => KeyCode::N,
            24 => KeyCode::O,
            25 => KeyCode::P,
            16 => KeyCode::Q,
            19 => KeyCode::R,
            31 => KeyCode::S,
            20 => KeyCode::T,
            22 => KeyCode::U,
            47 => KeyCode::V,
            17 => KeyCode::W,
            45 => KeyCode::X,
            21 => KeyCode::Y,
            44 => KeyCode::Z,

            // 숫자
            2 => KeyCode::Num1,
            3 => KeyCode::Num2,
            4 => KeyCode::Num3,
            5 => KeyCode::Num4,
            6 => KeyCode::Num5,
            7 => KeyCode::Num6,
            8 => KeyCode::Num7,
            9 => KeyCode::Num8,
            10 => KeyCode::Num9,
            11 => KeyCode::Num0,

            // 특수 키
            28 => KeyCode::Enter,
            1 => KeyCode::Escape,
            14 => KeyCode::Backspace,
            15 => KeyCode::Tab,
            57 => KeyCode::Space,

            // 기호
            12 => KeyCode::Minus,
            13 => KeyCode::Equal,
            26 => KeyCode::BracketLeft,
            27 => KeyCode::BracketRight,
            43 => KeyCode::Backslash,
            39 => KeyCode::Semicolon,
            40 => KeyCode::Quote,
            41 => KeyCode::Backquote,
            51 => KeyCode::Comma,
            52 => KeyCode::Period,
            53 => KeyCode::Slash,

            // 기능 키
            58 => KeyCode::CapsLock,
            59 => KeyCode::F1,
            60 => KeyCode::F2,
            61 => KeyCode::F3,
            62 => KeyCode::F4,
            63 => KeyCode::F5,
            64 => KeyCode::F6,
            65 => KeyCode::F7,
            66 => KeyCode::F8,
            67 => KeyCode::F9,
            68 => KeyCode::F10,
            87 => KeyCode::F11,
            88 => KeyCode::F12,

            // 편집 키
            110 => KeyCode::Insert,
            102 => KeyCode::Home,
            104 => KeyCode::PageUp,
            111 => KeyCode::Delete,
            107 => KeyCode::End,
            109 => KeyCode::PageDown,

            // 화살표
            106 => KeyCode::Right,
            105 => KeyCode::Left,
            108 => KeyCode::Down,
            103 => KeyCode::Up,

            // 한글/한자
            122 => KeyCode::Hangul,
            123 => KeyCode::Hanja,

            // 수정자
            29 => KeyCode::LeftControl,
            42 => KeyCode::LeftShift,
            56 => KeyCode::LeftAlt,
            125 => KeyCode::LeftSuper,
            97 => KeyCode::RightControl,
            54 => KeyCode::RightShift,
            100 => KeyCode::RightAlt,
            126 => KeyCode::RightSuper,

            _ => KeyCode::Unknown,
        }
    }

    /// KeyCode를 문자로 변환합니다 (Shift 없이).
    ///
    /// # Returns
    ///
    /// 해당 키의 문자, 문자 키가 아닌 경우 `None`
    pub fn to_char(&self) -> Option<char> {
        match self {
            KeyCode::A => Some('a'),
            KeyCode::B => Some('b'),
            KeyCode::C => Some('c'),
            KeyCode::D => Some('d'),
            KeyCode::E => Some('e'),
            KeyCode::F => Some('f'),
            KeyCode::G => Some('g'),
            KeyCode::H => Some('h'),
            KeyCode::I => Some('i'),
            KeyCode::J => Some('j'),
            KeyCode::K => Some('k'),
            KeyCode::L => Some('l'),
            KeyCode::M => Some('m'),
            KeyCode::N => Some('n'),
            KeyCode::O => Some('o'),
            KeyCode::P => Some('p'),
            KeyCode::Q => Some('q'),
            KeyCode::R => Some('r'),
            KeyCode::S => Some('s'),
            KeyCode::T => Some('t'),
            KeyCode::U => Some('u'),
            KeyCode::V => Some('v'),
            KeyCode::W => Some('w'),
            KeyCode::X => Some('x'),
            KeyCode::Y => Some('y'),
            KeyCode::Z => Some('z'),
            KeyCode::Num1 => Some('1'),
            KeyCode::Num2 => Some('2'),
            KeyCode::Num3 => Some('3'),
            KeyCode::Num4 => Some('4'),
            KeyCode::Num5 => Some('5'),
            KeyCode::Num6 => Some('6'),
            KeyCode::Num7 => Some('7'),
            KeyCode::Num8 => Some('8'),
            KeyCode::Num9 => Some('9'),
            KeyCode::Num0 => Some('0'),
            KeyCode::Space => Some(' '),
            KeyCode::Minus => Some('-'),
            KeyCode::Equal => Some('='),
            KeyCode::BracketLeft => Some('['),
            KeyCode::BracketRight => Some(']'),
            KeyCode::Backslash => Some('\\'),
            KeyCode::Semicolon => Some(';'),
            KeyCode::Quote => Some('\''),
            KeyCode::Backquote => Some('`'),
            KeyCode::Comma => Some(','),
            KeyCode::Period => Some('.'),
            KeyCode::Slash => Some('/'),
            _ => None,
        }
    }

    /// KeyCode를 문자로 변환합니다 (Shift 포함).
    ///
    /// # Returns
    ///
    /// 해당 키의 Shift 문자, 문자 키가 아닌 경우 `None`
    pub fn to_shifted_char(&self) -> Option<char> {
        match self {
            KeyCode::A => Some('A'),
            KeyCode::B => Some('B'),
            KeyCode::C => Some('C'),
            KeyCode::D => Some('D'),
            KeyCode::E => Some('E'),
            KeyCode::F => Some('F'),
            KeyCode::G => Some('G'),
            KeyCode::H => Some('H'),
            KeyCode::I => Some('I'),
            KeyCode::J => Some('J'),
            KeyCode::K => Some('K'),
            KeyCode::L => Some('L'),
            KeyCode::M => Some('M'),
            KeyCode::N => Some('N'),
            KeyCode::O => Some('O'),
            KeyCode::P => Some('P'),
            KeyCode::Q => Some('Q'),
            KeyCode::R => Some('R'),
            KeyCode::S => Some('S'),
            KeyCode::T => Some('T'),
            KeyCode::U => Some('U'),
            KeyCode::V => Some('V'),
            KeyCode::W => Some('W'),
            KeyCode::X => Some('X'),
            KeyCode::Y => Some('Y'),
            KeyCode::Z => Some('Z'),
            KeyCode::Num1 => Some('!'),
            KeyCode::Num2 => Some('@'),
            KeyCode::Num3 => Some('#'),
            KeyCode::Num4 => Some('$'),
            KeyCode::Num5 => Some('%'),
            KeyCode::Num6 => Some('^'),
            KeyCode::Num7 => Some('&'),
            KeyCode::Num8 => Some('*'),
            KeyCode::Num9 => Some('('),
            KeyCode::Num0 => Some(')'),
            KeyCode::Space => Some(' '),
            KeyCode::Minus => Some('_'),
            KeyCode::Equal => Some('+'),
            KeyCode::BracketLeft => Some('{'),
            KeyCode::BracketRight => Some('}'),
            KeyCode::Backslash => Some('|'),
            KeyCode::Semicolon => Some(':'),
            KeyCode::Quote => Some('"'),
            KeyCode::Backquote => Some('~'),
            KeyCode::Comma => Some('<'),
            KeyCode::Period => Some('>'),
            KeyCode::Slash => Some('?'),
            _ => None,
        }
    }

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
}

impl Default for KeyCode {
    fn default() -> Self {
        KeyCode::Unknown
    }
}

impl fmt::Display for KeyCode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?}", self)
    }
}

/// 수정자 키 상태
///
/// Shift, Control, Alt, Super 키의 현재 눌림 상태를 나타냅니다.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[repr(C)]
pub struct ModifierState {
    /// Shift 키 눌림 상태
    pub shift: bool,
    /// Control 키 눌림 상태
    pub control: bool,
    /// Alt 키 눌림 상태
    pub alt: bool,
    /// Super (Windows/Command) 키 눌림 상태
    pub super_key: bool,
    /// Caps Lock 활성화 상태
    pub caps_lock: bool,
    /// Num Lock 활성화 상태
    pub num_lock: bool,
}

impl ModifierState {
    /// 새로운 빈 ModifierState를 생성합니다.
    pub fn new() -> Self {
        Self::default()
    }

    /// X11 수정자 마스크에서 ModifierState를 생성합니다.
    ///
    /// # Arguments
    ///
    /// * `mask` - X11 수정자 마스크 (GDK 스타일)
    pub fn from_x11_mask(mask: u32) -> Self {
        const SHIFT_MASK: u32 = 1 << 0;
        const LOCK_MASK: u32 = 1 << 1;
        const CONTROL_MASK: u32 = 1 << 2;
        const MOD1_MASK: u32 = 1 << 3;  // Alt
        const MOD4_MASK: u32 = 1 << 6;  // Super

        Self {
            shift: (mask & SHIFT_MASK) != 0,
            control: (mask & CONTROL_MASK) != 0,
            alt: (mask & MOD1_MASK) != 0,
            super_key: (mask & MOD4_MASK) != 0,
            caps_lock: (mask & LOCK_MASK) != 0,
            num_lock: false, // X11에서 별도 처리 필요
        }
    }

    /// 수정자가 하나도 눌리지 않은 상태인지 확인합니다.
    pub fn is_empty(&self) -> bool {
        !self.shift && !self.control && !self.alt && !self.super_key
    }

    /// Shift만 눌린 상태인지 확인합니다.
    pub fn is_shift_only(&self) -> bool {
        self.shift && !self.control && !self.alt && !self.super_key
    }

    /// Control만 눌린 상태인지 확인합니다.
    pub fn is_control_only(&self) -> bool {
        !self.shift && self.control && !self.alt && !self.super_key
    }

    /// Alt만 눌린 상태인지 확인합니다.
    pub fn is_alt_only(&self) -> bool {
        !self.shift && !self.control && self.alt && !self.super_key
    }
}

#[cfg(test)]
mod tests {
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
        assert_eq!(KeyCode::from_x11_keycode(38), KeyCode::A);  // 30 + 8
        assert_eq!(KeyCode::from_x11_keycode(56), KeyCode::B);  // 48 + 8
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
    fn test_keycode_is_modifier() {
        assert!(KeyCode::LeftShift.is_modifier());
        assert!(KeyCode::RightAlt.is_modifier());
        assert!(!KeyCode::A.is_modifier());
        assert!(!KeyCode::Space.is_modifier());
    }
}
