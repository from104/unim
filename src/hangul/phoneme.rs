// hangul_phoneme.rs

use std::any::Any;
use std::hash::Hash;
use std::str::FromStr;

pub trait Phoneme: Eq + Hash {
    fn get_unicode_compat(&self) -> char;
    fn get_unicode(&self) -> char;
    fn get_index(&self) -> i32;
    fn as_any(&self) -> &dyn Any; // 이 메서드를 추가하여 객체 안전성 확보
}


#[derive(Debug, Copy, Clone, Eq, PartialEq, Hash)]
pub enum Onset {
    F = -1, G = 0, GG, N, D, DD, R, M, B, BB, S, SS, E, J, JJ, C, K, T, P, H,
}

impl Phoneme for Onset {
    fn as_any(&self) -> &dyn Any {
        self
    }
    fn get_index(&self) -> i32 {
        *self as i32
    }
    fn get_unicode(&self) -> char {
        match *self {
            Onset::F => '\u{115f}',
            _ => std::char::from_u32('\u{1100}' as u32 + *self as u32).unwrap(),
        }
    }

    fn get_unicode_compat(&self) -> char {
        match *self {
            Onset::F => '\u{3164}',
            Onset::G => '\u{3131}',
            Onset::GG => '\u{3132}',
            Onset::N => '\u{3134}',
            Onset::D => '\u{3137}',
            Onset::DD => '\u{3138}',
            Onset::R => '\u{3139}',
            Onset::M => '\u{3141}',
            Onset::B => '\u{3142}',
            Onset::BB => '\u{3143}',
            Onset::S => '\u{3145}',
            Onset::SS => '\u{3146}',
            Onset::E => '\u{3147}',
            Onset::J => '\u{3148}',
            Onset::JJ => '\u{3149}',
            Onset::C => '\u{314a}',
            Onset::K => '\u{314b}',
            Onset::T => '\u{314c}',
            Onset::P => '\u{314d}',
            Onset::H => '\u{314e}',
        }
    }
}

impl Onset {
    pub fn to_coda(&self) -> Result<Coda, String> {
        match *self {
            Onset::E => Ok(Coda::NG),
            Onset::R => Ok(Coda::L),
            _ => Coda::from_onset(*self),
        }
    }
}

impl FromStr for Onset {
    type Err = ();
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "F" => Ok(Onset::F),
            "G" => Ok(Onset::G),
            "GG" => Ok(Onset::GG),
            "N" => Ok(Onset::N),
            "D" => Ok(Onset::D),
            "DD" => Ok(Onset::DD),
            "R" => Ok(Onset::R),
            "M" => Ok(Onset::M),
            "B" => Ok(Onset::B),
            "BB" => Ok(Onset::BB),
            "S" => Ok(Onset::S),
            "SS" => Ok(Onset::SS),
            "E" => Ok(Onset::E),
            "J" => Ok(Onset::J),
            "JJ" => Ok(Onset::JJ),
            "C" => Ok(Onset::C),
            "K" => Ok(Onset::K),
            "T" => Ok(Onset::T),
            "P" => Ok(Onset::P),
            "H" => Ok(Onset::H),
            // Add more cases for other possible values of Onset
            _ => Err(()),
        }
    }
}

#[derive(Debug, Copy, Clone, Eq, PartialEq, Hash)]
pub enum Nucleus {
    F = -1, A = 0, AE, YA, YAE, EO, E, YEO, YE, O, WA, WAE, OE, YO, U, WEO, WE, WI, YU, EU, YI, I, 
}

impl Phoneme for Nucleus {
    fn as_any(&self) -> &dyn Any {
        self
    }
    fn get_index(&self) -> i32 {
        *self as i32
    }
    fn get_unicode(&self) -> char {
        match *self {
            Nucleus::F => '\u{1160}',
            _ => std::char::from_u32('\u{1161}' as u32 + *self as u32).unwrap(),
        }
    }
    fn get_unicode_compat(&self) -> char {
        match *self {
            Nucleus::F => '\u{3164}',
            _ => std::char::from_u32('\u{314f}' as u32 + *self as u32).unwrap(),
        }
    }
}

// impl Nucleus {
//     fn get_index(&self) -> i32 {
//         *self as i32
//     }
// }

impl FromStr for Nucleus {
    type Err = ();
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "F" => Ok(Nucleus::F),
            "A" => Ok(Nucleus::A),
            "AE" => Ok(Nucleus::AE),
            "YA" => Ok(Nucleus::YA),
            "YAE" => Ok(Nucleus::YAE),
            "EO" => Ok(Nucleus::EO),
            "E" => Ok(Nucleus::E),
            "YEO" => Ok(Nucleus::YEO),
            "YE" => Ok(Nucleus::YE),
            "O" => Ok(Nucleus::O),
            "WA" => Ok(Nucleus::WA),
            "WAE" => Ok(Nucleus::WAE),
            "OE" => Ok(Nucleus::OE),
            "YO" => Ok(Nucleus::YO),
            "U" => Ok(Nucleus::U),
            "WEO" => Ok(Nucleus::WEO),
            "WE" => Ok(Nucleus::WE),
            "WI" => Ok(Nucleus::WI),
            "YU" => Ok(Nucleus::YU),
            "EU" => Ok(Nucleus::EU),
            "YI" => Ok(Nucleus::YI),
            "I" => Ok(Nucleus::I),
            // Add more cases for other possible values of Onset
            _ => Err(()),
        }
    }
}

#[derive(Debug, Copy, Clone, Eq, PartialEq, Hash)]
pub enum Coda {
    E = 0, G, GG, GS, N, NJ, NH, D, L, LG, LM, LB, LS, LT, LP, LH, M, B, BS, S, SS, NG, J, C, K, T, P, H,
}

impl Phoneme for Coda {
    fn as_any(&self) -> &dyn Any {
        self
    }
    fn get_index(&self) -> i32 {
        *self as i32
    }
    fn get_unicode(&self) -> char {
        match *self {
            Coda::E => '\u{0000}',
            _ => std::char::from_u32('\u{11a8}' as u32 + *self as u32).unwrap(),
        }
    }
    fn get_unicode_compat(&self) -> char {
        match *self {
            Coda::E => '\u{0000}',
            _ => std::char::from_u32('\u{3131}' as u32 + *self as u32).unwrap(),
        }
    }
}

impl Coda {
    pub fn to_onset(&self) -> Result<Onset, String> {
        match *self {
            Coda::NG => Ok(Onset::E),
            Coda::L => Ok(Onset::R),
            _ => Onset::from_coda(*self),
        }
    }

    pub fn from_onset(onset: Onset) -> Result<Coda, String> {
        match onset {
            Onset::G => Ok(Coda::G),
            Onset::GG => Ok(Coda::GG),
            Onset::N => Ok(Coda::N),
            Onset::D => Ok(Coda::D),
            Onset::R => Ok(Coda::L),
            Onset::M => Ok(Coda::M),
            Onset::B => Ok(Coda::B),
            Onset::S => Ok(Coda::S),
            Onset::SS => Ok(Coda::SS),
            Onset::E => Ok(Coda::NG),
            Onset::J => Ok(Coda::J),
            Onset::C => Ok(Coda::C),
            Onset::K => Ok(Coda::K),
            Onset::T => Ok(Coda::T),
            Onset::P => Ok(Coda::P),
            Onset::H => Ok(Coda::H),
            _ => Err("Invalid onset".to_string()),
        }
    }
}

impl FromStr for Coda {
    type Err = ();
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "E" => Ok(Coda::E),
            "G" => Ok(Coda::G),
            "GG" => Ok(Coda::GG),
            "GS" => Ok(Coda::GS),
            "N" => Ok(Coda::N),
            "NJ" => Ok(Coda::NJ),
            "NH" => Ok(Coda::NH),
            "D" => Ok(Coda::D),
            "L" => Ok(Coda::L),
            "LG" => Ok(Coda::LG),
            "LM" => Ok(Coda::LM),
            "LB" => Ok(Coda::LB),
            "LS" => Ok(Coda::LS),
            "LT" => Ok(Coda::LT),
            "LP" => Ok(Coda::LP),
            "LH" => Ok(Coda::LH),
            "M" => Ok(Coda::M),
            "B" => Ok(Coda::B),
            "BS" => Ok(Coda::BS),
            "S" => Ok(Coda::S),
            "SS" => Ok(Coda::SS),
            "NG" => Ok(Coda::NG),
            "J" => Ok(Coda::J),
            "C" => Ok(Coda::C),
            "K" => Ok(Coda::K),
            "T" => Ok(Coda::T),
            "P" => Ok(Coda::P),
            "H" => Ok(Coda::H),
            // Add more cases for other possible values of Onset
            _ => Err(()),
        }
    }
}

impl Onset {
    pub fn from_coda(coda: Coda) -> Result<Onset, String> {
        match coda {
            Coda::G => Ok(Onset::G),
            Coda::GG => Ok(Onset::GG),
            Coda::N => Ok(Onset::N),
            Coda::D => Ok(Onset::D),
            Coda::L => Ok(Onset::R),
            Coda::M => Ok(Onset::M),
            Coda::B => Ok(Onset::B),
            Coda::S => Ok(Onset::S),
            Coda::SS => Ok(Onset::SS),
            Coda::NG => Ok(Onset::E),
            Coda::J => Ok(Onset::J),
            Coda::C => Ok(Onset::C),
            Coda::K => Ok(Onset::K),
            Coda::T => Ok(Onset::T),
            Coda::P => Ok(Onset::P),
            Coda::H => Ok(Onset::H),
            _ => Err("Invalid Coda".to_string()),
        }
    }
}

// 헬퍼 함수들

pub fn is_onset(phoneme: &impl Phoneme) -> bool {
    phoneme.as_any().is::<Onset>()
}

pub fn is_nucleus(phoneme: &impl Phoneme) -> bool {
    phoneme.as_any().is::<Nucleus>()
}

pub fn is_coda(phoneme: &impl Phoneme) -> bool {
    phoneme.as_any().is::<Coda>()
}

pub fn is_phoneme(phoneme: &impl Phoneme) -> bool {
    is_onset(phoneme) || is_nucleus(phoneme) || is_coda(phoneme)
}

 pub fn get_onset_by_index(idx: i32) -> Option<Onset> {
    match idx {
        -1 => Some(Onset::F),
        0 => Some(Onset::G),
        1 => Some(Onset::GG),
        2 => Some(Onset::N),
        3 => Some(Onset::D),
        4 => Some(Onset::DD),
        5 => Some(Onset::R),
        6 => Some(Onset::M),
        7 => Some(Onset::B),
        8 => Some(Onset::BB),
        9 => Some(Onset::S),
        10 => Some(Onset::SS),
        11 => Some(Onset::E),
        12 => Some(Onset::J),
        13 => Some(Onset::JJ),
        14 => Some(Onset::C),
        15 => Some(Onset::K),
        16 => Some(Onset::T),
        17 => Some(Onset::P),
        18 => Some(Onset::H),
        _ => None,
    }
}

pub fn get_nucleus_by_index(idx: i32) -> Option<Nucleus> {
    match idx {
        -1 => Some(Nucleus::F),
        0 => Some(Nucleus::A),
        1 => Some(Nucleus::AE),
        2 => Some(Nucleus::YA),
        3 => Some(Nucleus::YAE),
        4 => Some(Nucleus::EO),
        5 => Some(Nucleus::E),
        6 => Some(Nucleus::YEO),
        7 => Some(Nucleus::YE),
        8 => Some(Nucleus::O),
        9 => Some(Nucleus::WA),
        10 => Some(Nucleus::WAE),
        11 => Some(Nucleus::OE),
        12 => Some(Nucleus::YO),
        13 => Some(Nucleus::U),
        14 => Some(Nucleus::WEO),
        15 => Some(Nucleus::WE),
        16 => Some(Nucleus::WI),
        17 => Some(Nucleus::YU),
        18 => Some(Nucleus::EU),
        19 => Some(Nucleus::YI),
        20 => Some(Nucleus::I),
        _ => None,
    }
}

pub fn get_coda_by_index(idx: i32) -> Option<Coda> {
    match idx {
        0 => Some(Coda::E),
        1 => Some(Coda::G),
        2 => Some(Coda::GG),
        3 => Some(Coda::GS),
        4 => Some(Coda::N),
        5 => Some(Coda::NJ),
        6 => Some(Coda::NH),
        7 => Some(Coda::D),
        8 => Some(Coda::L),
        9 => Some(Coda::LG),
        10 => Some(Coda::LM),
        11 => Some(Coda::LB),
        12 => Some(Coda::LS),
        13 => Some(Coda::LT),
        14 => Some(Coda::LP),
        15 => Some(Coda::LH),
        16 => Some(Coda::M),
        17 => Some(Coda::B),
        18 => Some(Coda::BS),
        19 => Some(Coda::S),
        20 => Some(Coda::SS),
        21 => Some(Coda::NG),
        22 => Some(Coda::J),
        23 => Some(Coda::C),
        24 => Some(Coda::K),
        25 => Some(Coda::T),
        26 => Some(Coda::P),
        27 => Some(Coda::H),
        _ => None,
    }
}
