//! 키코드 정의 모듈
//!
//! 물리적 키보드 키코드와 수정자 키 상태를 정의합니다.
//! X11 및 Wayland 키코드와의 변환을 지원합니다.

use std::collections::HashMap;
use std::fmt;
use std::sync::LazyLock;

use crate::config::EnglishLayout;
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
    pub fn to_char_for_layout(&self, layout: EnglishLayout, shifted: bool) -> Option<char> {
        if layout == EnglishLayout::Qwerty {
            return if shifted { self.to_shifted_char() } else { self.to_char() };
        }

        if *self == KeyCode::Space {
            return Some(' ');
        }

        let (row, col) = self.physical_position()?;
        let tables = &*LAYOUT_TABLES;
        let rows = tables.get(&layout)?;
        let row_data = &rows[row];
        if col >= row_data.len() {
            return None;
        }
        let (lower, upper) = row_data[col];
        Some(if shifted { upper } else { lower })
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
/// key: (EnglishLayout, row, col) → value: (lower_char, upper_char)
static LAYOUT_TABLES: LazyLock<HashMap<EnglishLayout, Vec<Vec<(char, char)>>>> =
    LazyLock::new(|| {
        let layouts = [
            EnglishLayout::Dvorak,
            EnglishLayout::Colemak,
            EnglishLayout::ColemakDh,
            EnglishLayout::Workman,
        ];
        let mut tables = HashMap::new();
        for layout in layouts {
            let json_str = get_keymap_json(layout.keymap_name());
            let json: serde_json::Value = serde_json::from_str(json_str)
                .unwrap_or_else(|e| panic!("Failed to parse {} keymap: {}", layout.keymap_name(), e));
            let lower = &json["layout"]["lower"];
            let upper = &json["layout"]["upper"];
            let mut rows = Vec::with_capacity(4);
            for row_name in &ROW_NAMES {
                let lower_row = lower[row_name]
                    .as_array()
                    .unwrap_or_else(|| panic!("Missing lower.{} in {}", row_name, layout.keymap_name()));
                let upper_row = upper[row_name]
                    .as_array()
                    .unwrap_or_else(|| panic!("Missing upper.{} in {}", row_name, layout.keymap_name()));
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
            tables.insert(layout, rows);
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
    fn test_keycode_is_modifier() {
        assert!(KeyCode::LeftShift.is_modifier());
        assert!(KeyCode::RightAlt.is_modifier());
        assert!(!KeyCode::A.is_modifier());
        assert!(!KeyCode::Space.is_modifier());
    }

    // ── to_char_for_layout 테스트 ──

    #[test]
    fn test_to_char_for_layout_qwerty_consistency() {
        use crate::config::EnglishLayout;
        // Qwerty 레이아웃: to_char_for_layout == to_char / to_shifted_char
        let alpha_keys = [
            KeyCode::A, KeyCode::B, KeyCode::C, KeyCode::D, KeyCode::E,
            KeyCode::F, KeyCode::G, KeyCode::H, KeyCode::I, KeyCode::J,
            KeyCode::K, KeyCode::L, KeyCode::M, KeyCode::N, KeyCode::O,
            KeyCode::P, KeyCode::Q, KeyCode::R, KeyCode::S, KeyCode::T,
            KeyCode::U, KeyCode::V, KeyCode::W, KeyCode::X, KeyCode::Y, KeyCode::Z,
        ];
        for key in alpha_keys {
            assert_eq!(
                key.to_char_for_layout(EnglishLayout::Qwerty, false),
                key.to_char(),
                "Qwerty lower mismatch for {:?}", key
            );
            assert_eq!(
                key.to_char_for_layout(EnglishLayout::Qwerty, true),
                key.to_shifted_char(),
                "Qwerty upper mismatch for {:?}", key
            );
        }

        // 숫자키
        let num_keys = [
            KeyCode::Num1, KeyCode::Num2, KeyCode::Num3, KeyCode::Num4, KeyCode::Num5,
            KeyCode::Num6, KeyCode::Num7, KeyCode::Num8, KeyCode::Num9, KeyCode::Num0,
        ];
        for key in num_keys {
            assert_eq!(
                key.to_char_for_layout(EnglishLayout::Qwerty, false),
                key.to_char(),
                "Qwerty num lower mismatch for {:?}", key
            );
            assert_eq!(
                key.to_char_for_layout(EnglishLayout::Qwerty, true),
                key.to_shifted_char(),
                "Qwerty num upper mismatch for {:?}", key
            );
        }

        // 특수키
        let special_keys = [
            KeyCode::Minus, KeyCode::Equal, KeyCode::BracketLeft, KeyCode::BracketRight,
            KeyCode::Backslash, KeyCode::Semicolon, KeyCode::Quote, KeyCode::Backquote,
            KeyCode::Comma, KeyCode::Period, KeyCode::Slash,
        ];
        for key in special_keys {
            assert_eq!(
                key.to_char_for_layout(EnglishLayout::Qwerty, false),
                key.to_char(),
                "Qwerty special lower mismatch for {:?}", key
            );
            assert_eq!(
                key.to_char_for_layout(EnglishLayout::Qwerty, true),
                key.to_shifted_char(),
                "Qwerty special upper mismatch for {:?}", key
            );
        }
    }

    #[test]
    fn test_to_char_for_layout_dvorak_alphabet() {
        use crate::config::EnglishLayout;
        let cases: &[(KeyCode, char, char)] = &[
            (KeyCode::S, 'o', 'O'),
            (KeyCode::D, 'e', 'E'),
            (KeyCode::F, 'u', 'U'),
            (KeyCode::G, 'i', 'I'),
            (KeyCode::H, 'd', 'D'),
            (KeyCode::J, 'h', 'H'),
            (KeyCode::K, 't', 'T'),
            (KeyCode::L, 'n', 'N'),
            (KeyCode::Q, '\'', '"'),
            (KeyCode::W, ',', '<'),
            (KeyCode::E, '.', '>'),
            (KeyCode::R, 'p', 'P'),
        ];
        for &(key, lower, upper) in cases {
            assert_eq!(
                key.to_char_for_layout(EnglishLayout::Dvorak, false),
                Some(lower),
                "Dvorak lower mismatch for {:?}", key
            );
            assert_eq!(
                key.to_char_for_layout(EnglishLayout::Dvorak, true),
                Some(upper),
                "Dvorak upper mismatch for {:?}", key
            );
        }
    }

    #[test]
    fn test_to_char_for_layout_colemak_alphabet() {
        use crate::config::EnglishLayout;
        let cases: &[(KeyCode, char, char)] = &[
            (KeyCode::E, 'f', 'F'),
            (KeyCode::R, 'p', 'P'),
            (KeyCode::T, 'g', 'G'),
            (KeyCode::S, 'r', 'R'),
            (KeyCode::D, 's', 'S'),
            (KeyCode::F, 't', 'T'),
            (KeyCode::K, 'e', 'E'),
            (KeyCode::J, 'n', 'N'),
            (KeyCode::Y, 'j', 'J'),
            (KeyCode::Semicolon, 'o', 'O'),
            (KeyCode::P, ';', ':'),
        ];
        for &(key, lower, upper) in cases {
            assert_eq!(
                key.to_char_for_layout(EnglishLayout::Colemak, false),
                Some(lower),
                "Colemak lower mismatch for {:?}", key
            );
            assert_eq!(
                key.to_char_for_layout(EnglishLayout::Colemak, true),
                Some(upper),
                "Colemak upper mismatch for {:?}", key
            );
        }
    }

    #[test]
    fn test_to_char_for_layout_colemak_dh_differences() {
        use crate::config::EnglishLayout;
        // ColemakDh vs Colemak 차이점
        // T(1,4): Colemak='g', ColemakDh='b'
        assert_eq!(KeyCode::T.to_char_for_layout(EnglishLayout::Colemak, false), Some('g'));
        assert_eq!(KeyCode::T.to_char_for_layout(EnglishLayout::ColemakDh, false), Some('b'));

        // G(2,4): Colemak='d', ColemakDh='g'
        assert_eq!(KeyCode::G.to_char_for_layout(EnglishLayout::Colemak, false), Some('d'));
        assert_eq!(KeyCode::G.to_char_for_layout(EnglishLayout::ColemakDh, false), Some('g'));

        // B(3,4): Colemak='b', ColemakDh='v'
        assert_eq!(KeyCode::B.to_char_for_layout(EnglishLayout::Colemak, false), Some('b'));
        assert_eq!(KeyCode::B.to_char_for_layout(EnglishLayout::ColemakDh, false), Some('v'));

        // V(3,3): Colemak='v', ColemakDh='d'
        assert_eq!(KeyCode::V.to_char_for_layout(EnglishLayout::Colemak, false), Some('v'));
        assert_eq!(KeyCode::V.to_char_for_layout(EnglishLayout::ColemakDh, false), Some('d'));

        // H(2,5): Colemak='h', ColemakDh='m'
        assert_eq!(KeyCode::H.to_char_for_layout(EnglishLayout::Colemak, false), Some('h'));
        assert_eq!(KeyCode::H.to_char_for_layout(EnglishLayout::ColemakDh, false), Some('m'));

        // M(3,6): Colemak='m', ColemakDh='h'
        assert_eq!(KeyCode::M.to_char_for_layout(EnglishLayout::Colemak, false), Some('m'));
        assert_eq!(KeyCode::M.to_char_for_layout(EnglishLayout::ColemakDh, false), Some('h'));
    }

    #[test]
    fn test_to_char_for_layout_workman_alphabet() {
        use crate::config::EnglishLayout;
        let cases: &[(KeyCode, char, char)] = &[
            (KeyCode::W, 'd', 'D'),
            (KeyCode::E, 'r', 'R'),
            (KeyCode::R, 'w', 'W'),
            (KeyCode::D, 'h', 'H'),
            (KeyCode::F, 't', 'T'),
            (KeyCode::Y, 'j', 'J'),
            (KeyCode::U, 'f', 'F'),
            (KeyCode::I, 'u', 'U'),
            (KeyCode::O, 'p', 'P'),
            (KeyCode::H, 'y', 'Y'),
        ];
        for &(key, lower, upper) in cases {
            assert_eq!(
                key.to_char_for_layout(EnglishLayout::Workman, false),
                Some(lower),
                "Workman lower mismatch for {:?}", key
            );
            assert_eq!(
                key.to_char_for_layout(EnglishLayout::Workman, true),
                Some(upper),
                "Workman upper mismatch for {:?}", key
            );
        }
    }

    #[test]
    fn test_to_char_for_layout_dvorak_special_keys() {
        use crate::config::EnglishLayout;
        // Dvorak 특수키 매핑
        assert_eq!(KeyCode::Minus.to_char_for_layout(EnglishLayout::Dvorak, false), Some('['));
        assert_eq!(KeyCode::Equal.to_char_for_layout(EnglishLayout::Dvorak, false), Some(']'));
        assert_eq!(KeyCode::BracketLeft.to_char_for_layout(EnglishLayout::Dvorak, false), Some('/'));
        assert_eq!(KeyCode::BracketRight.to_char_for_layout(EnglishLayout::Dvorak, false), Some('='));
        assert_eq!(KeyCode::Semicolon.to_char_for_layout(EnglishLayout::Dvorak, false), Some('s'));
        assert_eq!(KeyCode::Quote.to_char_for_layout(EnglishLayout::Dvorak, false), Some('-'));
        assert_eq!(KeyCode::Slash.to_char_for_layout(EnglishLayout::Dvorak, false), Some('z'));
    }

    #[test]
    fn test_to_char_for_layout_dvorak_shift_special_keys() {
        use crate::config::EnglishLayout;
        assert_eq!(KeyCode::Minus.to_char_for_layout(EnglishLayout::Dvorak, true), Some('{'));
        assert_eq!(KeyCode::Equal.to_char_for_layout(EnglishLayout::Dvorak, true), Some('}'));
        assert_eq!(KeyCode::BracketLeft.to_char_for_layout(EnglishLayout::Dvorak, true), Some('?'));
        assert_eq!(KeyCode::BracketRight.to_char_for_layout(EnglishLayout::Dvorak, true), Some('+'));
        assert_eq!(KeyCode::Semicolon.to_char_for_layout(EnglishLayout::Dvorak, true), Some('S'));
        assert_eq!(KeyCode::Quote.to_char_for_layout(EnglishLayout::Dvorak, true), Some('_'));
        assert_eq!(KeyCode::Slash.to_char_for_layout(EnglishLayout::Dvorak, true), Some('Z'));
    }

    #[test]
    fn test_to_char_for_layout_space_all_layouts() {
        use crate::config::EnglishLayout;
        for layout in [
            EnglishLayout::Qwerty,
            EnglishLayout::Dvorak,
            EnglishLayout::Colemak,
            EnglishLayout::ColemakDh,
            EnglishLayout::Workman,
        ] {
            assert_eq!(
                KeyCode::Space.to_char_for_layout(layout, false),
                Some(' '),
                "Space should be ' ' for {:?}", layout
            );
            assert_eq!(
                KeyCode::Space.to_char_for_layout(layout, true),
                Some(' '),
                "Shift+Space should be ' ' for {:?}", layout
            );
        }
    }

    #[test]
    fn test_to_char_for_layout_shift_uppercase() {
        use crate::config::EnglishLayout;
        // 각 레이아웃에서 Shift+알파벳이 대문자 반환
        let layouts = [
            (EnglishLayout::Dvorak, KeyCode::S, 'O'),     // S→o, Shift→O
            (EnglishLayout::Colemak, KeyCode::F, 'T'),    // F→t, Shift→T
            (EnglishLayout::ColemakDh, KeyCode::T, 'B'),  // T→b, Shift→B
            (EnglishLayout::Workman, KeyCode::W, 'D'),    // W→d, Shift→D
            (EnglishLayout::Dvorak, KeyCode::J, 'H'),     // J→h, Shift→H
            (EnglishLayout::Colemak, KeyCode::K, 'E'),    // K→e, Shift→E
            (EnglishLayout::Workman, KeyCode::D, 'H'),    // D→h, Shift→H
        ];
        for (layout, key, expected_upper) in layouts {
            assert_eq!(
                key.to_char_for_layout(layout, true),
                Some(expected_upper),
                "Shift mismatch for {:?} on {:?}", key, layout
            );
        }
    }
}
