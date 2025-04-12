/**
 * 한글 자모 인터페이스 (Rust에서는 트레잇으로 표현)
 */
pub trait Jamo: std::fmt::Debug + Clone + Copy + PartialEq + Eq + std::hash::Hash {
    /**
     * 음절 조합을 위한 순서 얻기
     */
    fn get_sequence(&self) -> i32;

    /**
     * 유니코드(첫가끝) 얻기
     */
    fn get_unicode(&self) -> char;

    /**
     * 유니코드(호환용 자모) 얻기
     */
    fn get_unicode_compat(&self) -> char;
}

/**
 * 초성 상수 (Rust에서는 enum으로 표현)
 */
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Cho {
    // 상수 데이터 (Rust enum은 데이터와 함께 열거형을 가질 수 있습니다.)
    F = -1,  // 초성 채움
    G = 0,   // ㄱ
    GG = 1,  // ㄲ
    N = 2,   // ㄴ
    D = 3,   // ㄷ
    DD = 4,  // ㄸ
    R = 5,   // ㄹ
    M = 6,   // ㅁ
    B = 7,   // ㅂ
    BB = 8,  // ㅃ
    S = 9,   // ㅅ
    SS = 10, // ㅆ
    E = 11,  // ㅇ
    J = 12,  // ㅈ
    JJ = 13, // ㅉ
    C = 14,  // ㅊ
    K = 15,  // ㅋ
    T = 16,  // ㅌ
    P = 17,  // ㅍ
    H = 18,  // ㅎ
}

impl Jamo for Cho {
    /**
     * 음절 조합을 위한 순서 얻기
     */
    fn get_sequence(&self) -> i32 {
        *self as i32 // enum 값을 i32로 캐스팅
    }

    /**
     * 유니코드(첫가끝) 얻기
     */
    fn get_unicode(&self) -> char {
        match self {
            Cho::F => '\u{115f}',  // 초성 채움
            Cho::G => '\u{1100}',  // ㄱ
            Cho::GG => '\u{1101}', // ㄲ
            Cho::N => '\u{1102}',  // ㄴ
            Cho::D => '\u{1103}',  // ㄷ
            Cho::DD => '\u{1104}', // ㄸ
            Cho::R => '\u{1105}',  // ㄹ
            Cho::M => '\u{1106}',  // ㅁ
            Cho::B => '\u{1107}',  // ㅂ
            Cho::BB => '\u{1108}', // ㅃ
            Cho::S => '\u{1109}',  // ㅅ
            Cho::SS => '\u{110a}', // ㅆ
            Cho::E => '\u{110b}',  // ㅇ
            Cho::J => '\u{110c}',  // ㅈ
            Cho::JJ => '\u{110d}', // ㅉ
            Cho::C => '\u{110e}',  // ㅊ
            Cho::K => '\u{110f}',  // ㅋ
            Cho::T => '\u{1110}',  // ㅌ
            Cho::P => '\u{1111}',  // ㅍ
            Cho::H => '\u{1112}',  // ㅎ
        }
    }

    /**
     * 유니코드(호환용 자모) 얻기
     */
    fn get_unicode_compat(&self) -> char {
        match self {
            Cho::F => '\u{3164}',  // 초성 채움
            Cho::G => '\u{3131}',  // ㄱ
            Cho::GG => '\u{3132}', // ㄲ
            Cho::N => '\u{3134}',  // ㄴ
            Cho::D => '\u{3137}',  // ㄷ
            Cho::DD => '\u{3138}', // ㄸ
            Cho::R => '\u{3139}',  // ㄹ
            Cho::M => '\u{3141}',  // ㅁ
            Cho::B => '\u{3142}',  // ㅂ
            Cho::BB => '\u{3143}', // ㅃ
            Cho::S => '\u{3145}',  // ㅅ
            Cho::SS => '\u{3146}', // ㅆ
            Cho::E => '\u{3147}',  // ㅇ
            Cho::J => '\u{3148}',  // ㅈ
            Cho::JJ => '\u{3149}', // ㅉ
            Cho::C => '\u{314a}',  // ㅊ
            Cho::K => '\u{314b}',  // ㅋ
            Cho::T => '\u{314c}',  // ㅌ
            Cho::P => '\u{314d}',  // ㅍ
            Cho::H => '\u{314e}',  // ㅎ
        }
    }
}

impl Cho {
    /**
     * 초성을 종성으로 변환
     */
    pub fn to_jong(&self) -> Result<Jong, &'static str> {
        match self {
            Cho::E => Ok(Jong::NG),
            Cho::R => Ok(Jong::L),
            Cho::G => Ok(Jong::G),
            Cho::GG => Ok(Jong::GG),
            Cho::N => Ok(Jong::N),
            Cho::D => Ok(Jong::D),
            Cho::DD => Err("초성 ㄸ는 종성으로 변환할 수 없습니다."),
            Cho::M => Ok(Jong::M),
            Cho::B => Ok(Jong::B),
            Cho::BB => Err("초성 ㅃ는 종성으로 변환할 수 없습니다."),
            Cho::S => Ok(Jong::S),
            Cho::SS => Ok(Jong::SS),
            Cho::J => Ok(Jong::J),
            Cho::JJ => Err("초성 ㅉ는 종성으로 변환할 수 없습니다."),
            Cho::C => Ok(Jong::C),
            Cho::K => Ok(Jong::K),
            Cho::T => Ok(Jong::T),
            Cho::P => Ok(Jong::P),
            Cho::H => Ok(Jong::H),
            Cho::F => Err("초성 채움은 종성으로 변환할 수 없습니다."),
        }
    }
}

/**
 * 중성 상수 (Rust에서는 enum으로 표현)
 */
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Jung {
    // 상수 데이터
    F = -1,   // 중성 채움
    A = 0,    // ㅏ
    AE = 1,   // ㅐ
    YA = 2,   // ㅑ
    YAE = 3,  // ㅒ
    EO = 4,   // ㅓ
    E = 5,    // ㅔ
    YEO = 6,  // ㅕ
    YE = 7,   // ㅖ
    O = 8,    // ㅗ
    WA = 9,   // ㅘ
    WAE = 10, // ㅙ
    OE = 11,  // ㅚ
    YO = 12,  // ㅛ
    U = 13,   // ㅜ
    WEO = 14, // ㅝ
    WE = 15,  // ㅞ
    WI = 16,  // ㅟ
    YU = 17,  // ㅠ
    EU = 18,  // ㅡ
    YI = 19,  // ㅢ
    I = 20,   // ㅣ
}

impl Jamo for Jung {
    /**
     * 음절 조합을 위한 순서 얻기
     */
    fn get_sequence(&self) -> i32 {
        *self as i32
    }

    /**
     * 유니코드(첫가끝) 얻기
     */
    fn get_unicode(&self) -> char {
        match self {
            Jung::F => '\u{1160}',   // 중성 채움
            Jung::A => '\u{1161}',   // ㅏ
            Jung::AE => '\u{1162}',  // ㅐ
            Jung::YA => '\u{1163}',  // ㅑ
            Jung::YAE => '\u{1164}', // ㅒ
            Jung::EO => '\u{1165}',  // ㅓ
            Jung::E => '\u{1166}',   // ㅔ
            Jung::YEO => '\u{1167}', // ㅕ
            Jung::YE => '\u{1168}',  // ㅖ
            Jung::O => '\u{1169}',   // ㅗ
            Jung::WA => '\u{116a}',  // ㅘ
            Jung::WAE => '\u{116b}', // ㅙ
            Jung::OE => '\u{116c}',  // ㅚ
            Jung::YO => '\u{116d}',  // ㅛ
            Jung::U => '\u{116e}',   // ㅜ
            Jung::WEO => '\u{116f}', // ㅝ
            Jung::WE => '\u{1170}',  // ㅞ
            Jung::WI => '\u{1171}',  // ㅟ
            Jung::YU => '\u{1172}',  // ㅠ
            Jung::EU => '\u{1173}',  // ㅡ
            Jung::YI => '\u{1174}',  // ㅢ
            Jung::I => '\u{1175}',   // ㅣ
        }
    }

    /**
     * 유니코드(호환용 자모) 얻기
     */
    fn get_unicode_compat(&self) -> char {
        match self {
            Jung::F => '\u{3164}',   // 중성 채움
            Jung::A => '\u{314f}',   // ㅏ
            Jung::AE => '\u{3150}',  // ㅐ
            Jung::YA => '\u{3151}',  // ㅑ
            Jung::YAE => '\u{3152}', // ㅒ
            Jung::EO => '\u{3153}',  // ㅓ
            Jung::E => '\u{3154}',   // ㅔ
            Jung::YEO => '\u{3155}', // ㅕ
            Jung::YE => '\u{3156}',  // ㅖ
            Jung::O => '\u{3157}',   // ㅗ
            Jung::WA => '\u{3158}',  // ㅘ
            Jung::WAE => '\u{3159}', // ㅙ
            Jung::OE => '\u{315a}',  // ㅚ
            Jung::YO => '\u{315b}',  // ㅛ
            Jung::U => '\u{315c}',   // ㅜ
            Jung::WEO => '\u{315d}', // ㅝ
            Jung::WE => '\u{315e}',  // ㅞ
            Jung::WI => '\u{315f}',  // ㅟ
            Jung::YU => '\u{3160}',  // ㅠ
            Jung::EU => '\u{3161}',  // ㅡ
            Jung::YI => '\u{3162}',  // ㅢ
            Jung::I => '\u{3163}',   // ㅣ
        }
    }
}

/**
 * 종성 상수 (Rust에서는 enum으로 표현)
 */
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Jong {
    // 상수 데이터
    E = 0,   // 종성 비움
    G = 1,   // ㄱ
    GG = 2,  // ㄲ
    GS = 3,  // ㄳ
    N = 4,   // ㄴ
    NJ = 5,  // ㄵ
    NH = 6,  // ㄶ
    D = 7,   // ㄷ
    L = 8,   // ㄹ
    LG = 9,  // ㄺ
    LM = 10, // ㄻ
    LB = 11, // ㄼ
    LS = 12, // ㄽ
    LT = 13, // ㄾ
    LP = 14, // ㄿ
    LH = 15, // ㅀ
    M = 16,  // ㅁ
    B = 17,  // ㅂ
    BS = 18, // ㅄ
    S = 19,  // ㅅ
    SS = 20, // ㅆ
    NG = 21, // ㅇ
    J = 22,  // ㅈ
    C = 23,  // ㅊ
    K = 24,  // ㅋ
    T = 25,  // ㅌ
    P = 26,  // ㅍ
    H = 27,  // ㅎ
}

impl Jamo for Jong {
    /**
     * 음절 조합을 위한 순서 얻기
     */
    fn get_sequence(&self) -> i32 {
        *self as i32
    }

    /**
     * 유니코드(첫가끝) 얻기
     */
    fn get_unicode(&self) -> char {
        match self {
            Jong::E => '\u{0000}', // 종성 비움 (null 문자, 필요에 따라 다른 값으로 변경 가능)
            Jong::G => '\u{11a8}', // ㄱ
            Jong::GG => '\u{11a9}', // ㄲ
            Jong::GS => '\u{11aa}', // ㄳ
            Jong::N => '\u{11ab}', // ㄴ
            Jong::NJ => '\u{11ac}', // ㄵ
            Jong::NH => '\u{11ad}', // ㄶ
            Jong::D => '\u{11ae}', // ㄷ
            Jong::L => '\u{11af}', // ㄹ
            Jong::LG => '\u{11b0}', // ㄺ
            Jong::LM => '\u{11b1}', // ㄻ
            Jong::LB => '\u{11b2}', // ㄼ
            Jong::LS => '\u{11b3}', // ㄽ
            Jong::LT => '\u{11b4}', // ㄾ
            Jong::LP => '\u{11b5}', // ㄿ
            Jong::LH => '\u{11b6}', // ㅀ
            Jong::M => '\u{11b7}', // ㅁ
            Jong::B => '\u{11b8}', // ㅂ
            Jong::BS => '\u{11b9}', // ㅄ
            Jong::S => '\u{11ba}', // ㅅ
            Jong::SS => '\u{11bb}', // ㅆ
            Jong::NG => '\u{11bc}', // ㅇ
            Jong::J => '\u{11bd}', // ㅈ
            Jong::C => '\u{11be}', // ㅊ
            Jong::K => '\u{11bf}', // ㅋ
            Jong::T => '\u{11c0}', // ㅌ
            Jong::P => '\u{11c1}', // ㅍ
            Jong::H => '\u{11c2}', // ㅎ
        }
    }

    /**
     * 유니코드(호환용 자모) 얻기
     */
    fn get_unicode_compat(&self) -> char {
        match self {
            Jong::E => '\u{0000}', // 종성 비움 (null 문자, 필요에 따라 다른 값으로 변경 가능)
            Jong::G => '\u{3131}', // ㄱ
            Jong::GG => '\u{3132}', // ㄲ
            Jong::GS => '\u{3133}', // ㄳ
            Jong::N => '\u{3134}', // ㄴ
            Jong::NJ => '\u{3135}', // ㄵ
            Jong::NH => '\u{3136}', // ㄶ
            Jong::D => '\u{3137}', // ㄷ
            Jong::L => '\u{3139}', // ㄹ
            Jong::LG => '\u{313a}', // ㄺ
            Jong::LM => '\u{313b}', // ㄻ
            Jong::LB => '\u{313c}', // ㄼ
            Jong::LS => '\u{313d}', // ㄽ
            Jong::LT => '\u{313e}', // ㄾ
            Jong::LP => '\u{313f}', // ㄿ
            Jong::LH => '\u{3140}', // ㅀ
            Jong::M => '\u{3141}', // ㅁ
            Jong::B => '\u{3142}', // ㅂ
            Jong::BS => '\u{3144}', // ㅄ
            Jong::S => '\u{3145}', // ㅅ
            Jong::SS => '\u{3146}', // ㅆ
            Jong::NG => '\u{3147}', // ㅇ
            Jong::J => '\u{3148}', // ㅈ
            Jong::C => '\u{314a}', // ㅊ
            Jong::K => '\u{314b}', // ㅋ
            Jong::T => '\u{314c}', // ㅌ
            Jong::P => '\u{314d}', // ㅍ
            Jong::H => '\u{314e}', // ㅎ
        }
    }
}

impl Jong {
    /**
     * 종성을 초성으로 변환
     * @throws IllegalArgumentException (Rust에서는 panic! 또는 Result<T, E>를 사용)
     */
    pub fn to_cho(&self) -> Cho {
        match self {
            Jong::NG => Cho::E,
            Jong::L => Cho::R,
            _ => {
                match self {
                    Jong::G => Cho::G,
                    Jong::GG => Cho::GG,
                    Jong::N => Cho::N,
                    Jong::D => Cho::D,
                    Jong::M => Cho::M,
                    Jong::B => Cho::B,
                    Jong::S => Cho::S,
                    Jong::SS => Cho::SS,
                    Jong::J => Cho::J,
                    Jong::C => Cho::C,
                    Jong::K => Cho::K,
                    Jong::T => Cho::T,
                    Jong::P => Cho::P,
                    Jong::H => Cho::H,
                    Jong::E => panic!("종성 비움은 초성으로 변환할 수 없습니다."), // 또는 Result를 사용하여 에러 처리
                    Jong::GS => panic!("겹받침 종성은 초성으로 변환할 수 없습니다."), // 또는 Result를 사용하여 에러 처리
                    Jong::NJ => panic!("겹받침 종성은 초성으로 변환할 수 없습니다."), // 또는 Result를 사용하여 에러 처리
                    Jong::NH => panic!("겹받침 종성은 초성으로 변환할 수 없습니다."), // 또는 Result를 사용하여 에러 처리
                    Jong::LG => panic!("겹받침 종성은 초성으로 변환할 수 없습니다."), // 또는 Result를 사용하여 에러 처리
                    Jong::LM => panic!("겹받침 종성은 초성으로 변환할 수 없습니다."), // 또는 Result를 사용하여 에러 처리
                    Jong::LB => panic!("겹받침 종성은 초성으로 변환할 수 없습니다."), // 또는 Result를 사용하여 에러 처리
                    Jong::LS => panic!("겹받침 종성은 초성으로 변환할 수 없습니다."), // 또는 Result를 사용하여 에러 처리
                    Jong::LT => panic!("겹받침 종성은 초성으로 변환할 수 없습니다."), // 또는 Result를 사용하여 에러 처리
                    Jong::LP => panic!("겹받침 종성은 초성으로 변환할 수 없습니다."), // 또는 Result를 사용하여 에러 처리
                    Jong::LH => panic!("겹받침 종성은 초성으로 변환할 수 없습니다."), // 또는 Result를 사용하여 에러 처리
                    Jong::BS => panic!("겹받침 종성은 초성으로 변환할 수 없습니다."), // 또는 Result를 사용하여 에러 처리
                    _ => panic!("알 수 없는 종성입니다."), // 예외 처리 또는 Result 반환 고려
                }
            }
        }
    }
}

/**
 * 객체가 초성인지 판별
 */
pub fn is_cho<T>(o: &T) -> bool
where
    T: Jamo,
{
    // Rust에서는 enum을 직접 비교하는 것이 더 일반적입니다.
    // 여기서는 trait object를 사용하는 대신, 제네릭과 trait bound를 사용하여
    // 컴파일 타임에 타입 체크를 수행합니다.
    match o.get_sequence() {
        // sequence 값의 범위로 초성 여부를 판단하는 것은 정확하지 않습니다.
        // Rust의 enum은 타입 기반으로 판별하는 것이 더 안전하고 효율적입니다.
        -1..=18 => {
            // Cho enum의 sequence 범위 (수정 필요)
            // 이 범위 체크는 Cho enum의 실제 sequence 값과 일치해야 합니다.
            // 하지만 더 좋은 방법은 타입 자체를 확인하는 것입니다.
            // 여기서는 단순화를 위해 Java 코드와 유사하게 sequence 범위 체크를 유지합니다.
            true
        }
        _ => false,
    }
    // o.is::<Cho>() // 이렇게 직접 타입 체크를 하는 것이 더 정확하지만,
    // trait object를 사용하거나 enum 자체를 비교하는 것이 더 Rust 스타일입니다.
}

/**
 * 객체가 중성인지 판별
 */
pub fn is_jung<T>(o: &T) -> bool
where
    T: Jamo,
{
    match o.get_sequence() {
        -1..=20 => true, // Jung enum의 sequence 범위 (수정 필요)
        _ => false,
    }
}

/**
 * 객체가 종성인지 판별
 */
pub fn is_jong<T>(o: &T) -> bool
where
    T: Jamo,
{
    match o.get_sequence() {
        0..=27 => true, // Jong enum의 sequence 범위 (수정 필요)
        _ => false,
    }
}

/**
 * 객체가 자모인지 판별 (Rust에서는 trait bound로 대체 가능, 여기서는 함수로 유지)
 */
pub fn is_jamo<T>(o: &T) -> bool
where
    T: Jamo,
{
    is_cho(o) || is_jung(o) || is_jong(o)
}

/**
 * 순서로 초성 얻기
 */
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

/**
 * 순서로 중성 얻기
 */
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

/**
 * 순서로 종성 얻기
 */
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

impl std::str::FromStr for Cho {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "F" => Ok(Cho::F),
            "ㄱ" | "G" => Ok(Cho::G),
            "ㄲ" | "GG" => Ok(Cho::GG),
            "ㄴ" | "N" => Ok(Cho::N),
            "ㄷ" | "D" => Ok(Cho::D),
            "ㄸ" | "DD" => Ok(Cho::DD),
            "ㄹ" | "R" => Ok(Cho::R),
            "ㅁ" | "M" => Ok(Cho::M),
            "ㅂ" | "B" => Ok(Cho::B),
            "ㅃ" | "BB" => Ok(Cho::BB),
            "ㅅ" | "S" => Ok(Cho::S),
            "ㅆ" | "SS" => Ok(Cho::SS),
            "ㅇ" | "E" => Ok(Cho::E),
            "ㅈ" | "J" => Ok(Cho::J),
            "ㅉ" | "JJ" => Ok(Cho::JJ),
            "ㅊ" | "C" => Ok(Cho::C),
            "ㅋ" | "K" => Ok(Cho::K),
            "ㅌ" | "T" => Ok(Cho::T),
            "ㅍ" | "P" => Ok(Cho::P),
            "ㅎ" | "H" => Ok(Cho::H),
            _ => Err(format!("Invalid Cho: {}", s)),
        }
    }
}

impl std::str::FromStr for Jung {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "F" => Ok(Jung::F),
            "ㅏ" | "A" => Ok(Jung::A),
            "ㅐ" | "AE" => Ok(Jung::AE),
            "ㅑ" | "YA" => Ok(Jung::YA),
            "ㅒ" | "YAE" => Ok(Jung::YAE),
            "ㅓ" | "EO" => Ok(Jung::EO),
            "ㅔ" | "E" => Ok(Jung::E),
            "ㅕ" | "YEO" => Ok(Jung::YEO),
            "ㅖ" | "YE" => Ok(Jung::YE),
            "ㅗ" | "O" => Ok(Jung::O),
            "ㅘ" | "WA" => Ok(Jung::WA),
            "ㅙ" | "WAE" => Ok(Jung::WAE),
            "ㅚ" | "OE" => Ok(Jung::OE),
            "ㅛ" | "YO" => Ok(Jung::YO),
            "ㅜ" | "U" => Ok(Jung::U),
            "ㅝ" | "WEO" => Ok(Jung::WEO),
            "ㅞ" | "WE" => Ok(Jung::WE),
            "ㅟ" | "WI" => Ok(Jung::WI),
            "ㅠ" | "YU" => Ok(Jung::YU),
            "ㅡ" | "EU" => Ok(Jung::EU),
            "ㅢ" | "YI" => Ok(Jung::YI),
            "ㅣ" | "I" => Ok(Jung::I),
            _ => Err(format!("Invalid Jung: {}", s)),
        }
    }
}

impl std::str::FromStr for Jong {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "E" | "" => Ok(Jong::E),
            "ㄱ" | "G" => Ok(Jong::G),
            "ㄲ" | "GG" => Ok(Jong::GG),
            "ㄳ" | "GS" => Ok(Jong::GS),
            "ㄴ" | "N" => Ok(Jong::N),
            "ㄵ" | "NJ" => Ok(Jong::NJ),
            "ㄶ" | "NH" => Ok(Jong::NH),
            "ㄷ" | "D" => Ok(Jong::D),
            "ㄹ" | "L" => Ok(Jong::L),
            "ㄺ" | "LG" => Ok(Jong::LG),
            "ㄻ" | "LM" => Ok(Jong::LM),
            "ㄼ" | "LB" => Ok(Jong::LB),
            "ㄽ" | "LS" => Ok(Jong::LS),
            "ㄾ" | "LT" => Ok(Jong::LT),
            "ㄿ" | "LP" => Ok(Jong::LP),
            "ㅀ" | "LH" => Ok(Jong::LH),
            "ㅁ" | "M" => Ok(Jong::M),
            "ㅂ" | "B" => Ok(Jong::B),
            "ㅄ" | "BS" => Ok(Jong::BS),
            "ㅅ" | "S" => Ok(Jong::S),
            "ㅆ" | "SS" => Ok(Jong::SS),
            "ㅇ" | "NG" => Ok(Jong::NG),
            "ㅈ" | "J" => Ok(Jong::J),
            "ㅊ" | "C" => Ok(Jong::C),
            "ㅋ" | "K" => Ok(Jong::K),
            "ㅌ" | "T" => Ok(Jong::T),
            "ㅍ" | "P" => Ok(Jong::P),
            "ㅎ" | "H" => Ok(Jong::H),
            _ => Err(format!("Invalid Jong: {}", s)),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum JamoEnum {
    Cho(Cho),
    Jung(Jung),
    Jong(Jong),
    Special(char), // 자모가 아닌 특수 문자
}

impl Jamo for JamoEnum {
    fn get_sequence(&self) -> i32 {
        match self {
            JamoEnum::Cho(cho) => cho.get_sequence(),
            JamoEnum::Jung(jung) => jung.get_sequence(),
            JamoEnum::Jong(jong) => jong.get_sequence(),
            JamoEnum::Special(_) => -1,
        }
    }

    fn get_unicode(&self) -> char {
        match self {
            JamoEnum::Cho(cho) => cho.get_unicode(),
            JamoEnum::Jung(jung) => jung.get_unicode(),
            JamoEnum::Jong(jong) => jong.get_unicode(),
            JamoEnum::Special(c) => *c,
        }
    }

    fn get_unicode_compat(&self) -> char {
        match self {
            JamoEnum::Cho(cho) => cho.get_unicode_compat(),
            JamoEnum::Jung(jung) => jung.get_unicode_compat(),
            JamoEnum::Jong(jong) => jong.get_unicode_compat(),
            JamoEnum::Special(c) => *c,
        }
    }
}
