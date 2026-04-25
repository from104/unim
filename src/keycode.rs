//! 키코드 정의 모듈
//!
//! 물리적 키보드 키코드와 수정자 키 상태를 정의합니다.
//! X11 및 Wayland 키코드와의 변환을 지원합니다.

use std::collections::HashMap;
use std::fmt;
use std::sync::LazyLock;

use crate::keystroke::get_keymap_json;

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

            // 한국어/한자
            122 => KeyCode::Korean,
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

    /// 물리키를 (행, 열) 인덱스로 변환. 레이아웃 테이블 lookup용.
    fn physical_position(&self) -> Option<(usize, usize)> {
        match self {
            // Row 0: 숫자행 (14키)
            KeyCode::Backquote => Some((0, 0)),
            KeyCode::Num1 => Some((0, 1)),
            KeyCode::Num2 => Some((0, 2)),
            KeyCode::Num3 => Some((0, 3)),
            KeyCode::Num4 => Some((0, 4)),
            KeyCode::Num5 => Some((0, 5)),
            KeyCode::Num6 => Some((0, 6)),
            KeyCode::Num7 => Some((0, 7)),
            KeyCode::Num8 => Some((0, 8)),
            KeyCode::Num9 => Some((0, 9)),
            KeyCode::Num0 => Some((0, 10)),
            KeyCode::Minus => Some((0, 11)),
            KeyCode::Equal => Some((0, 12)),
            KeyCode::Backslash => Some((0, 13)),
            // Row 1: 상단 알파벳 (12키)
            KeyCode::Q => Some((1, 0)),
            KeyCode::W => Some((1, 1)),
            KeyCode::E => Some((1, 2)),
            KeyCode::R => Some((1, 3)),
            KeyCode::T => Some((1, 4)),
            KeyCode::Y => Some((1, 5)),
            KeyCode::U => Some((1, 6)),
            KeyCode::I => Some((1, 7)),
            KeyCode::O => Some((1, 8)),
            KeyCode::P => Some((1, 9)),
            KeyCode::BracketLeft => Some((1, 10)),
            KeyCode::BracketRight => Some((1, 11)),
            // Row 2: 홈행 (11키)
            KeyCode::A => Some((2, 0)),
            KeyCode::S => Some((2, 1)),
            KeyCode::D => Some((2, 2)),
            KeyCode::F => Some((2, 3)),
            KeyCode::G => Some((2, 4)),
            KeyCode::H => Some((2, 5)),
            KeyCode::J => Some((2, 6)),
            KeyCode::K => Some((2, 7)),
            KeyCode::L => Some((2, 8)),
            KeyCode::Semicolon => Some((2, 9)),
            KeyCode::Quote => Some((2, 10)),
            // Row 3: 하단 알파벳 (10키)
            KeyCode::Z => Some((3, 0)),
            KeyCode::X => Some((3, 1)),
            KeyCode::C => Some((3, 2)),
            KeyCode::V => Some((3, 3)),
            KeyCode::B => Some((3, 4)),
            KeyCode::N => Some((3, 5)),
            KeyCode::M => Some((3, 6)),
            KeyCode::Comma => Some((3, 7)),
            KeyCode::Period => Some((3, 8)),
            KeyCode::Slash => Some((3, 9)),
            _ => None,
        }
    }

    /// 지정된 영문 레이아웃에서 이 물리키가 생성하는 문자를 반환한다.
    ///
    /// Qwerty인 경우 기존 to_char()/to_shifted_char()를 직접 호출하여 성능 영향 제로.
    pub fn to_char_for_layout(&self, layout: &str, shifted: bool) -> Option<char> {
        let normalized = crate::config::normalize_english_layout_name(layout);
        if normalized == crate::config::ENGLISH_LAYOUT_QWERTY {
            return if shifted { self.to_shifted_char() } else { self.to_char() };
        }

        if *self == KeyCode::Space {
            return Some(' ');
        }

        let (row, col) = self.physical_position()?;
        let tables = &*LAYOUT_TABLES;
        let rows = tables.get(normalized.as_str())?;
        let row_data = &rows[row];
        if col >= row_data.len() {
            return None;
        }
        let (lower, upper) = row_data[col];
        Some(if shifted { upper } else { lower })
    }

    /// Windows Win32 Virtual Key 코드에서 KeyCode로 변환합니다.
    ///
    /// # Arguments
    ///
    /// * `vk` - Win32 Virtual Key 코드 (VK_*)
    ///
    /// # Returns
    ///
    /// 변환된 `KeyCode`, 매핑되지 않은 경우 `KeyCode::Unknown`
    pub fn from_win32_vk(vk: u16) -> Self {
        match vk {
            // 알파벳 (VK_A=0x41 ~ VK_Z=0x5A)
            0x41 => KeyCode::A,
            0x42 => KeyCode::B,
            0x43 => KeyCode::C,
            0x44 => KeyCode::D,
            0x45 => KeyCode::E,
            0x46 => KeyCode::F,
            0x47 => KeyCode::G,
            0x48 => KeyCode::H,
            0x49 => KeyCode::I,
            0x4A => KeyCode::J,
            0x4B => KeyCode::K,
            0x4C => KeyCode::L,
            0x4D => KeyCode::M,
            0x4E => KeyCode::N,
            0x4F => KeyCode::O,
            0x50 => KeyCode::P,
            0x51 => KeyCode::Q,
            0x52 => KeyCode::R,
            0x53 => KeyCode::S,
            0x54 => KeyCode::T,
            0x55 => KeyCode::U,
            0x56 => KeyCode::V,
            0x57 => KeyCode::W,
            0x58 => KeyCode::X,
            0x59 => KeyCode::Y,
            0x5A => KeyCode::Z,

            // 숫자 (VK_0=0x30 ~ VK_9=0x39)
            0x31 => KeyCode::Num1,
            0x32 => KeyCode::Num2,
            0x33 => KeyCode::Num3,
            0x34 => KeyCode::Num4,
            0x35 => KeyCode::Num5,
            0x36 => KeyCode::Num6,
            0x37 => KeyCode::Num7,
            0x38 => KeyCode::Num8,
            0x39 => KeyCode::Num9,
            0x30 => KeyCode::Num0,

            // 특수 키
            0x0D => KeyCode::Enter,    // VK_RETURN
            0x1B => KeyCode::Escape,   // VK_ESCAPE
            0x08 => KeyCode::Backspace, // VK_BACK
            0x09 => KeyCode::Tab,      // VK_TAB
            0x20 => KeyCode::Space,    // VK_SPACE

            // 기호 (OEM 키)
            0xBD => KeyCode::Minus,        // VK_OEM_MINUS (- _)
            0xBB => KeyCode::Equal,        // VK_OEM_PLUS (= +)
            0xDB => KeyCode::BracketLeft,  // VK_OEM_4 ([ {)
            0xDD => KeyCode::BracketRight, // VK_OEM_6 (] })
            0xDC => KeyCode::Backslash,    // VK_OEM_5 (\ |)
            0xBA => KeyCode::Semicolon,    // VK_OEM_1 (; :)
            0xDE => KeyCode::Quote,        // VK_OEM_7 (' ")
            0xC0 => KeyCode::Backquote,    // VK_OEM_3 (` ~)
            0xBC => KeyCode::Comma,        // VK_OEM_COMMA (, <)
            0xBE => KeyCode::Period,       // VK_OEM_PERIOD (. >)
            0xBF => KeyCode::Slash,        // VK_OEM_2 (/ ?)

            // 기능 키
            0x14 => KeyCode::CapsLock, // VK_CAPITAL
            0x70 => KeyCode::F1,       // VK_F1
            0x71 => KeyCode::F2,
            0x72 => KeyCode::F3,
            0x73 => KeyCode::F4,
            0x74 => KeyCode::F5,
            0x75 => KeyCode::F6,
            0x76 => KeyCode::F7,
            0x77 => KeyCode::F8,
            0x78 => KeyCode::F9,
            0x79 => KeyCode::F10,
            0x7A => KeyCode::F11,
            0x7B => KeyCode::F12,      // VK_F12

            // 편집 키
            0x2D => KeyCode::Insert,   // VK_INSERT
            0x24 => KeyCode::Home,     // VK_HOME
            0x21 => KeyCode::PageUp,   // VK_PRIOR
            0x2E => KeyCode::Delete,   // VK_DELETE
            0x23 => KeyCode::End,      // VK_END
            0x22 => KeyCode::PageDown, // VK_NEXT

            // 화살표
            0x27 => KeyCode::Right, // VK_RIGHT
            0x25 => KeyCode::Left,  // VK_LEFT
            0x28 => KeyCode::Down,  // VK_DOWN
            0x26 => KeyCode::Up,    // VK_UP

            // 한국어/한자
            0x15 => KeyCode::Korean, // VK_HANGUL
            0x19 => KeyCode::Hanja,  // VK_HANJA

            // 수정자
            0xA0 => KeyCode::LeftShift,    // VK_LSHIFT
            0xA1 => KeyCode::RightShift,   // VK_RSHIFT
            0xA2 => KeyCode::LeftControl,  // VK_LCONTROL
            0xA3 => KeyCode::RightControl, // VK_RCONTROL
            0xA4 => KeyCode::LeftAlt,      // VK_LMENU
            0xA5 => KeyCode::RightAlt,     // VK_RMENU
            0x5B => KeyCode::LeftSuper,    // VK_LWIN
            0x5C => KeyCode::RightSuper,   // VK_RWIN

            _ => KeyCode::Unknown,
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

    /// KeyCode의 설정용 이름을 반환합니다.
    pub fn name(&self) -> &'static str {
        match self {
            KeyCode::A => "A",
            KeyCode::B => "B",
            KeyCode::C => "C",
            KeyCode::D => "D",
            KeyCode::E => "E",
            KeyCode::F => "F",
            KeyCode::G => "G",
            KeyCode::H => "H",
            KeyCode::I => "I",
            KeyCode::J => "J",
            KeyCode::K => "K",
            KeyCode::L => "L",
            KeyCode::M => "M",
            KeyCode::N => "N",
            KeyCode::O => "O",
            KeyCode::P => "P",
            KeyCode::Q => "Q",
            KeyCode::R => "R",
            KeyCode::S => "S",
            KeyCode::T => "T",
            KeyCode::U => "U",
            KeyCode::V => "V",
            KeyCode::W => "W",
            KeyCode::X => "X",
            KeyCode::Y => "Y",
            KeyCode::Z => "Z",
            KeyCode::Num1 => "Num1",
            KeyCode::Num2 => "Num2",
            KeyCode::Num3 => "Num3",
            KeyCode::Num4 => "Num4",
            KeyCode::Num5 => "Num5",
            KeyCode::Num6 => "Num6",
            KeyCode::Num7 => "Num7",
            KeyCode::Num8 => "Num8",
            KeyCode::Num9 => "Num9",
            KeyCode::Num0 => "Num0",
            KeyCode::Enter => "Enter",
            KeyCode::Escape => "Escape",
            KeyCode::Backspace => "Backspace",
            KeyCode::Tab => "Tab",
            KeyCode::Space => "Space",
            KeyCode::Minus => "Minus",
            KeyCode::Equal => "Equal",
            KeyCode::BracketLeft => "BracketLeft",
            KeyCode::BracketRight => "BracketRight",
            KeyCode::Backslash => "Backslash",
            KeyCode::Semicolon => "Semicolon",
            KeyCode::Quote => "Quote",
            KeyCode::Backquote => "Backquote",
            KeyCode::Comma => "Comma",
            KeyCode::Period => "Period",
            KeyCode::Slash => "Slash",
            KeyCode::CapsLock => "CapsLock",
            KeyCode::F1 => "F1",
            KeyCode::F2 => "F2",
            KeyCode::F3 => "F3",
            KeyCode::F4 => "F4",
            KeyCode::F5 => "F5",
            KeyCode::F6 => "F6",
            KeyCode::F7 => "F7",
            KeyCode::F8 => "F8",
            KeyCode::F9 => "F9",
            KeyCode::F10 => "F10",
            KeyCode::F11 => "F11",
            KeyCode::F12 => "F12",
            KeyCode::Insert => "Insert",
            KeyCode::Home => "Home",
            KeyCode::PageUp => "PageUp",
            KeyCode::Delete => "Delete",
            KeyCode::End => "End",
            KeyCode::PageDown => "PageDown",
            KeyCode::Right => "Right",
            KeyCode::Left => "Left",
            KeyCode::Down => "Down",
            KeyCode::Up => "Up",
            KeyCode::Korean => "Korean",
            KeyCode::Hanja => "Hanja",
            KeyCode::LeftControl => "LeftControl",
            KeyCode::LeftShift => "LeftShift",
            KeyCode::LeftAlt => "LeftAlt",
            KeyCode::LeftSuper => "LeftSuper",
            KeyCode::RightControl => "RightControl",
            KeyCode::RightShift => "RightShift",
            KeyCode::RightAlt => "RightAlt",
            KeyCode::RightSuper => "RightSuper",
            KeyCode::Unknown => "Unknown",
        }
    }

    /// 설정용 이름에서 KeyCode로 변환합니다.
    pub fn from_name(name: &str) -> Self {
        match name {
            "A" => KeyCode::A,
            "B" => KeyCode::B,
            "C" => KeyCode::C,
            "D" => KeyCode::D,
            "E" => KeyCode::E,
            "F" => KeyCode::F,
            "G" => KeyCode::G,
            "H" => KeyCode::H,
            "I" => KeyCode::I,
            "J" => KeyCode::J,
            "K" => KeyCode::K,
            "L" => KeyCode::L,
            "M" => KeyCode::M,
            "N" => KeyCode::N,
            "O" => KeyCode::O,
            "P" => KeyCode::P,
            "Q" => KeyCode::Q,
            "R" => KeyCode::R,
            "S" => KeyCode::S,
            "T" => KeyCode::T,
            "U" => KeyCode::U,
            "V" => KeyCode::V,
            "W" => KeyCode::W,
            "X" => KeyCode::X,
            "Y" => KeyCode::Y,
            "Z" => KeyCode::Z,
            "Num1" => KeyCode::Num1,
            "Num2" => KeyCode::Num2,
            "Num3" => KeyCode::Num3,
            "Num4" => KeyCode::Num4,
            "Num5" => KeyCode::Num5,
            "Num6" => KeyCode::Num6,
            "Num7" => KeyCode::Num7,
            "Num8" => KeyCode::Num8,
            "Num9" => KeyCode::Num9,
            "Num0" => KeyCode::Num0,
            "Enter" => KeyCode::Enter,
            "Escape" => KeyCode::Escape,
            "Backspace" => KeyCode::Backspace,
            "Tab" => KeyCode::Tab,
            "Space" => KeyCode::Space,
            "Minus" => KeyCode::Minus,
            "Equal" => KeyCode::Equal,
            "BracketLeft" => KeyCode::BracketLeft,
            "BracketRight" => KeyCode::BracketRight,
            "Backslash" => KeyCode::Backslash,
            "Semicolon" => KeyCode::Semicolon,
            "Quote" => KeyCode::Quote,
            "Backquote" => KeyCode::Backquote,
            "Comma" => KeyCode::Comma,
            "Period" => KeyCode::Period,
            "Slash" => KeyCode::Slash,
            "CapsLock" => KeyCode::CapsLock,
            "F1" => KeyCode::F1,
            "F2" => KeyCode::F2,
            "F3" => KeyCode::F3,
            "F4" => KeyCode::F4,
            "F5" => KeyCode::F5,
            "F6" => KeyCode::F6,
            "F7" => KeyCode::F7,
            "F8" => KeyCode::F8,
            "F9" => KeyCode::F9,
            "F10" => KeyCode::F10,
            "F11" => KeyCode::F11,
            "F12" => KeyCode::F12,
            "Insert" => KeyCode::Insert,
            "Home" => KeyCode::Home,
            "PageUp" => KeyCode::PageUp,
            "Delete" => KeyCode::Delete,
            "End" => KeyCode::End,
            "PageDown" => KeyCode::PageDown,
            "Right" => KeyCode::Right,
            "Left" => KeyCode::Left,
            "Down" => KeyCode::Down,
            "Up" => KeyCode::Up,
            "Korean" => KeyCode::Korean,
            "Hanja" => KeyCode::Hanja,
            "LeftControl" => KeyCode::LeftControl,
            "LeftShift" => KeyCode::LeftShift,
            "LeftAlt" => KeyCode::LeftAlt,
            "LeftSuper" => KeyCode::LeftSuper,
            "RightControl" => KeyCode::RightControl,
            "RightShift" => KeyCode::RightShift,
            "RightAlt" => KeyCode::RightAlt,
            "RightSuper" => KeyCode::RightSuper,
            _ => KeyCode::Unknown,
        }
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
        const MOD1_MASK: u32 = 1 << 3; // Alt
        const MOD4_MASK: u32 = 1 << 6; // Super

        Self {
            shift: (mask & SHIFT_MASK) != 0,
            control: (mask & CONTROL_MASK) != 0,
            alt: (mask & MOD1_MASK) != 0,
            super_key: (mask & MOD4_MASK) != 0,
            caps_lock: (mask & LOCK_MASK) != 0,
            num_lock: false, // X11에서 별도 처리 필요
        }
    }

    /// Win32 수정자 비트마스크에서 ModifierState를 생성합니다.
    ///
    /// # Arguments
    ///
    /// * `modifiers` - 비트마스크: bit0=Shift, bit1=Control, bit2=Alt,
    ///                 bit3=Super(Win), bit4=CapsLock, bit5=NumLock
    pub fn from_win32_modifiers(modifiers: u32) -> Self {
        Self {
            shift: (modifiers & 0x01) != 0,
            control: (modifiers & 0x02) != 0,
            alt: (modifiers & 0x04) != 0,
            super_key: (modifiers & 0x08) != 0,
            caps_lock: (modifiers & 0x10) != 0,
            num_lock: (modifiers & 0x20) != 0,
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

/// JSON 키맵의 행 이름 (1st, 2nd, 3nd, 4th)
const ROW_NAMES: [&str; 4] = ["1st", "2nd", "3nd", "4th"];

/// 레이아웃별 물리키→문자 매핑 테이블 (JSON 키맵에서 동적 생성).
/// key: 영어 프로필 이름 (e.g. "dvorak") → value: rows[row_index][col_index] = (lower, upper)
static LAYOUT_TABLES: LazyLock<HashMap<String, Vec<Vec<(char, char)>>>> =
    LazyLock::new(|| {
        // Phase 9: QWERTY 는 to_char/to_shifted_char 빠른 경로로 처리되므로 테이블 불필요.
        let layouts: &[&str] = &[
            crate::config::ENGLISH_LAYOUT_DVORAK,
            crate::config::ENGLISH_LAYOUT_COLEMAK,
            crate::config::ENGLISH_LAYOUT_COLEMAK_DH,
            crate::config::ENGLISH_LAYOUT_WORKMAN,
        ];
        let mut tables: HashMap<String, Vec<Vec<(char, char)>>> = HashMap::new();
        for layout in layouts {
            let keymap_file = crate::config::english_layout_keymap_name(layout);
            let json_str = get_keymap_json(&keymap_file);
            let json: serde_json::Value = serde_json::from_str(json_str)
                .unwrap_or_else(|e| panic!("Failed to parse {keymap_file} keymap: {e}"));
            let lower = &json["layout"]["lower"];
            let upper = &json["layout"]["upper"];
            let mut rows = Vec::with_capacity(4);
            for row_name in &ROW_NAMES {
                let lower_row = lower[row_name].as_array().unwrap_or_else(|| {
                    panic!("Missing lower.{row_name} in {keymap_file}")
                });
                let upper_row = upper[row_name].as_array().unwrap_or_else(|| {
                    panic!("Missing upper.{row_name} in {keymap_file}")
                });
                let pairs: Vec<(char, char)> = lower_row
                    .iter()
                    .zip(upper_row.iter())
                    .map(|(l, u)| {
                        let lc = l.as_str().unwrap().chars().next().unwrap();
                        let uc = u.as_str().unwrap().chars().next().unwrap();
                        (lc, uc)
                    })
                    .collect();
                rows.push(pairs);
            }
            tables.insert((*layout).to_string(), rows);
        }
        tables
    });

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

    // ── to_char_for_layout 테스트 ──

    #[test]
    fn test_to_char_for_layout_qwerty_consistency() {
        // Qwerty: to_char_for_layout == to_char / to_shifted_char (모든 문자키)
        let all_char_keys = [
            KeyCode::A, KeyCode::B, KeyCode::C, KeyCode::D, KeyCode::E,
            KeyCode::F, KeyCode::G, KeyCode::H, KeyCode::I, KeyCode::J,
            KeyCode::K, KeyCode::L, KeyCode::M, KeyCode::N, KeyCode::O,
            KeyCode::P, KeyCode::Q, KeyCode::R, KeyCode::S, KeyCode::T,
            KeyCode::U, KeyCode::V, KeyCode::W, KeyCode::X, KeyCode::Y, KeyCode::Z,
            KeyCode::Num0, KeyCode::Num1, KeyCode::Num2, KeyCode::Num3, KeyCode::Num4,
            KeyCode::Num5, KeyCode::Num6, KeyCode::Num7, KeyCode::Num8, KeyCode::Num9,
            KeyCode::Minus, KeyCode::Equal, KeyCode::BracketLeft, KeyCode::BracketRight,
            KeyCode::Backslash, KeyCode::Semicolon, KeyCode::Quote, KeyCode::Backquote,
            KeyCode::Comma, KeyCode::Period, KeyCode::Slash, KeyCode::Space,
        ];
        for key in all_char_keys {
            assert_eq!(
                key.to_char_for_layout("qwerty", false),
                key.to_char(),
                "Qwerty lower mismatch for {:?}", key
            );
            assert_eq!(
                key.to_char_for_layout("qwerty", true),
                key.to_shifted_char(),
                "Qwerty upper mismatch for {:?}", key
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
            &[KeyCode::Backquote, KeyCode::Num1, KeyCode::Num2, KeyCode::Num3,
              KeyCode::Num4, KeyCode::Num5, KeyCode::Num6, KeyCode::Num7,
              KeyCode::Num8, KeyCode::Num9, KeyCode::Num0, KeyCode::Minus,
              KeyCode::Equal, KeyCode::Backslash],
            &[KeyCode::Q, KeyCode::W, KeyCode::E, KeyCode::R, KeyCode::T,
              KeyCode::Y, KeyCode::U, KeyCode::I, KeyCode::O, KeyCode::P,
              KeyCode::BracketLeft, KeyCode::BracketRight],
            &[KeyCode::A, KeyCode::S, KeyCode::D, KeyCode::F, KeyCode::G,
              KeyCode::H, KeyCode::J, KeyCode::K, KeyCode::L, KeyCode::Semicolon,
              KeyCode::Quote],
            &[KeyCode::Z, KeyCode::X, KeyCode::C, KeyCode::V, KeyCode::B,
              KeyCode::N, KeyCode::M, KeyCode::Comma, KeyCode::Period, KeyCode::Slash],
        ];
        let row_names = ["1st", "2nd", "3nd", "4th"];

        let layouts = [
            "dvorak",
            "colemak",
            "colemak_dh",
            "workman",
        ];

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
                        layout, keycode, expected_lower
                    );
                    assert_eq!(
                        keycode.to_char_for_layout(layout, true),
                        Some(expected_upper),
                        "{:?} {:?} upper: expected '{}' from JSON",
                        layout, keycode, expected_upper
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
}
