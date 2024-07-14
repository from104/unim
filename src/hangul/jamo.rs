// hangul_jamo.rs

use std::str::FromStr;
use std::hash::Hash;

pub trait Jamo: Eq + Hash {
    fn get_sequence(&self) -> i32;
    fn get_unicode(&self) -> char;
    fn get_unicode_compat(&self) -> char;
}

#[derive(Debug, Copy, Clone, Eq, PartialEq, Hash)]
pub enum Cho {
    F = -1, G = 0, GG, N, D, DD, R, M, B, BB, S, SS, E, J, JJ, C, K, T, P, H,
}

impl Jamo for Cho {
    fn get_sequence(&self) -> i32 {
        *self as i32
    }
    
    fn get_unicode(&self) -> char {
        match *self {
            Cho::F => '\u{115f}',
            _ => std::char::from_u32('\u{1100}' as u32 + *self as u32).unwrap(),
        }
    }

    fn get_unicode_compat(&self) -> char {
        match *self {
            Cho::F => '\u{3164}',
            _ => std::char::from_u32('\u{3131}' as u32 + *self as u32).unwrap(),
        }
    }
}

impl Cho {
    pub fn to_jong(&self) -> Result<Jong, String> {
        match *self {
            Cho::E => Ok(Jong::NG),
            Cho::R => Ok(Jong::L),
            _ => Jong::from_cho(*self),
        }
    }
}

impl FromStr for Cho {
    type Err = ();
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "F" => Ok(Cho::F),
            "G" => Ok(Cho::G),
            "GG" => Ok(Cho::GG),
            "N" => Ok(Cho::N),
            "D" => Ok(Cho::D),
            "DD" => Ok(Cho::DD),
            "R" => Ok(Cho::R),
            "M" => Ok(Cho::M),
            "B" => Ok(Cho::B),
            "BB" => Ok(Cho::BB),
            "S" => Ok(Cho::S),
            "SS" => Ok(Cho::SS),
            "E" => Ok(Cho::E),
            "J" => Ok(Cho::J),
            "JJ" => Ok(Cho::JJ),
            "C" => Ok(Cho::C),
            "K" => Ok(Cho::K),
            "T" => Ok(Cho::T),
            "P" => Ok(Cho::P),
            "H" => Ok(Cho::H),
            // Add more cases for other possible values of Cho
            _ => Err(()),
        }
    }
}

#[derive(Debug, Copy, Clone, Eq, PartialEq, Hash)]
pub enum Jung {
    F = -1, A = 0, AE, YA, YAE, EO, E, YEO, YE, O, WA, WAE, OE, YO, U, WEO, WE, WI, YU, EU, YI, I,
}

impl Jamo for Jung {
    fn get_sequence(&self) -> i32 {
        *self as i32
    }
    
    fn get_unicode(&self) -> char {
        match *self {
            Jung::F => '\u{1160}',
            _ => std::char::from_u32('\u{1161}' as u32 + *self as u32).unwrap(),
        }
    }

    fn get_unicode_compat(&self) -> char {
        match *self {
            Jung::F => '\u{3164}',
            _ => std::char::from_u32('\u{3150}' as u32 + *self as u32).unwrap(),
        }
    }
}

impl FromStr for Jung {
    type Err = ();
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "F" => Ok(Jung::F),
            "A" => Ok(Jung::A),
            "AE" => Ok(Jung::AE),
            "YA" => Ok(Jung::YA),
            "YAE" => Ok(Jung::YAE),
            "EO" => Ok(Jung::EO),
            "E" => Ok(Jung::E),
            "YEO" => Ok(Jung::YEO),
            "YE" => Ok(Jung::YE),
            "O" => Ok(Jung::O),
            "WA" => Ok(Jung::WA),
            "WAE" => Ok(Jung::WAE),
            "OE" => Ok(Jung::OE),
            "YO" => Ok(Jung::YO),
            "U" => Ok(Jung::U),
            "WEO" => Ok(Jung::WEO),
            "WE" => Ok(Jung::WE),
            "WI" => Ok(Jung::WI),
            "YU" => Ok(Jung::YU),
            "EU" => Ok(Jung::EU),
            "YI" => Ok(Jung::YI),
            "I" => Ok(Jung::I),
            // Add more cases for other possible values of Cho
            _ => Err(()),
        }
    }
}

#[derive(Debug, Copy, Clone, Eq, PartialEq, Hash)]
pub enum Jong {
    E = 0, G, GG, GS, N, NJ, NH, D, L, LG, LM, LB, LS, LT, LP, LH, M, B, BS, S, SS, NG, J, C, K, T, P, H,
}

impl Jamo for Jong {
    fn get_sequence(&self) -> i32 {
        *self as i32
    }
    
    fn get_unicode(&self) -> char {
        match *self {
            Jong::E => '\u{0000}',
            _ => std::char::from_u32('\u{11a8}' as u32 + *self as u32).unwrap(),
        }
    }

    fn get_unicode_compat(&self) -> char {
        match *self {
            Jong::E => '\u{0000}',
            _ => std::char::from_u32('\u{3131}' as u32 + *self as u32).unwrap(),
        }
    }
}

impl Jong {
    pub fn to_cho(&self) -> Result<Cho, String> {
        match *self {
            Jong::NG => Ok(Cho::E),
            Jong::L => Ok(Cho::R),
            _ => Cho::from_jong(*self),
        }
    }

    pub fn from_cho(cho: Cho) -> Result<Jong, String> {
        match cho {
            Cho::G => Ok(Jong::G),
            Cho::GG => Ok(Jong::GG),
            Cho::N => Ok(Jong::N),
            Cho::D => Ok(Jong::D),
            Cho::R => Ok(Jong::L),
            Cho::M => Ok(Jong::M),
            Cho::B => Ok(Jong::B),
            Cho::S => Ok(Jong::S),
            Cho::SS => Ok(Jong::SS),
            Cho::E => Ok(Jong::NG),
            Cho::J => Ok(Jong::J),
            Cho::C => Ok(Jong::C),
            Cho::K => Ok(Jong::K),
            Cho::T => Ok(Jong::T),
            Cho::P => Ok(Jong::P),
            Cho::H => Ok(Jong::H),
            _ => Err("Invalid Cho".to_string()),
        }
    }
}

impl FromStr for Jong {
    type Err = ();
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "E" => Ok(Jong::E),
            "G" => Ok(Jong::G),
            "GG" => Ok(Jong::GG),
            "GS" => Ok(Jong::GS),
            "N" => Ok(Jong::N),
            "NJ" => Ok(Jong::NJ),
            "NH" => Ok(Jong::NH),
            "D" => Ok(Jong::D),
            "L" => Ok(Jong::L),
            "LG" => Ok(Jong::LG),
            "LM" => Ok(Jong::LM),
            "LB" => Ok(Jong::LB),
            "LS" => Ok(Jong::LS),
            "LT" => Ok(Jong::LT),
            "LP" => Ok(Jong::LP),
            "LH" => Ok(Jong::LH),
            "M" => Ok(Jong::M),
            "B" => Ok(Jong::B),
            "BS" => Ok(Jong::BS),
            "S" => Ok(Jong::S),
            "SS" => Ok(Jong::SS),
            "NG" => Ok(Jong::NG),
            "J" => Ok(Jong::J),
            "C" => Ok(Jong::C),
            "K" => Ok(Jong::K),
            "T" => Ok(Jong::T),
            "P" => Ok(Jong::P),
            "H" => Ok(Jong::H),            
            // Add more cases for other possible values of Cho
            _ => Err(()),
        }
    }
}

impl Cho {
    pub fn from_jong(jong: Jong) -> Result<Cho, String> {
        match jong {
            Jong::G => Ok(Cho::G),
            Jong::GG => Ok(Cho::GG),
            Jong::N => Ok(Cho::N),
            Jong::D => Ok(Cho::D),
            Jong::L => Ok(Cho::R),
            Jong::M => Ok(Cho::M),
            Jong::B => Ok(Cho::B),
            Jong::S => Ok(Cho::S),
            Jong::SS => Ok(Cho::SS),
            Jong::NG => Ok(Cho::E),
            Jong::J => Ok(Cho::J),
            Jong::C => Ok(Cho::C),
            Jong::K => Ok(Cho::K),
            Jong::T => Ok(Cho::T),
            Jong::P => Ok(Cho::P),
            Jong::H => Ok(Cho::H),
            _ => Err("Invalid Jong".to_string()),
        }
    }
}

// 헬퍼 함수들

pub fn is_cho(jamo: &impl Jamo) -> bool {
    matches!(jamo.get_unicode(), '\u{1100}'..='\u{1112}')
}

pub fn is_jung(jamo: &impl Jamo) -> bool {
    matches!(jamo.get_unicode(), '\u{1161}'..='\u{1175}')
}

pub fn is_jong(jamo: &impl Jamo) -> bool {
    matches!(jamo.get_unicode(), '\u{11A8}'..='\u{11C2}')
}

pub fn is_jamo(jamo: &impl Jamo) -> bool {
    is_cho(jamo) || is_jung(jamo) || is_jong(jamo)
}

pub fn get_cho_by_sequence(seq: i32) -> Option<Cho> {
    match seq {
        -1 => Some(Cho::F),
        0 => Some(Cho::G),
        1 => Some(Cho::GG),
        2 => Some(Cho::N),
        3 => Some(Cho::D),
        4 => Some(Cho::DD),
        5 => Some(Cho::R),
        6 => Some(Cho::M),
        7 => Some(Cho::B),
        8 => Some(Cho::BB),
        9 => Some(Cho::S),
        10 => Some(Cho::SS),
        11 => Some(Cho::E),
        12 => Some(Cho::J),
        13 => Some(Cho::JJ),
        14 => Some(Cho::C),
        15 => Some(Cho::K),
        16 => Some(Cho::T),
        17 => Some(Cho::P),
        18 => Some(Cho::H),
        _ => None,
    }
}

pub fn get_jung_by_sequence(seq: i32) -> Option<Jung> {
    match seq {
        -1 => Some(Jung::F),
        0 => Some(Jung::A),
        1 => Some(Jung::AE),
        2 => Some(Jung::YA),
        3 => Some(Jung::YAE),
        4 => Some(Jung::EO),
        5 => Some(Jung::E),
        6 => Some(Jung::YEO),
        7 => Some(Jung::YE),
        8 => Some(Jung::O),
        9 => Some(Jung::WA),
        10 => Some(Jung::WAE),
        11 => Some(Jung::OE),
        12 => Some(Jung::YO),
        13 => Some(Jung::U),
        14 => Some(Jung::WEO),
        15 => Some(Jung::WE),
        16 => Some(Jung::WI),
        17 => Some(Jung::YU),
        18 => Some(Jung::EU),
        19 => Some(Jung::YI),
        20 => Some(Jung::I),
        _ => None,
    }
}

pub fn get_jong_by_sequence(seq: i32) -> Option<Jong> {
    match seq {
        0 => Some(Jong::E),
        1 => Some(Jong::G),
        2 => Some(Jong::GG),
        3 => Some(Jong::GS),
        4 => Some(Jong::N),
        5 => Some(Jong::NJ),
        6 => Some(Jong::NH),
        7 => Some(Jong::D),
        8 => Some(Jong::L),
        9 => Some(Jong::LG),
        10 => Some(Jong::LM),
        11 => Some(Jong::LB),
        12 => Some(Jong::LS),
        13 => Some(Jong::LT),
        14 => Some(Jong::LP),
        15 => Some(Jong::LH),
        16 => Some(Jong::M),
        17 => Some(Jong::B),
        18 => Some(Jong::BS),
        19 => Some(Jong::S),
        20 => Some(Jong::SS),
        21 => Some(Jong::NG),
        22 => Some(Jong::J),
        23 => Some(Jong::C),
        24 => Some(Jong::K),
        25 => Some(Jong::T),
        26 => Some(Jong::P),
        27 => Some(Jong::H),
        _ => None,
    }
}
