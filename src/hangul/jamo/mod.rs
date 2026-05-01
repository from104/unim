/// 한글 자모(초성, 중성, 종성)를 나타내는 타입들이 구현해야 하는 공통 트레이트입니다.
///
/// 이 트레이트는 각 자모 타입이 가져야 하는 기본적인 기능들을 정의합니다.
/// 예를 들어, 음절 조합에 사용될 순서 값이나 유니코드 문자 표현을 얻는 기능을 포함합니다.
pub trait Jamo: std::fmt::Debug + Clone + Copy + PartialEq + Eq + std::hash::Hash {
    /// 음절을 구성할 때 사용되는 자모의 순서 값을 반환합니다.
    ///
    /// 한글 음절은 초성, 중성, 종성의 순서로 조합됩니다. 이 순서 값은
    /// 유니코드 표준에 정의된 계산 방식(Korean Syllables algorithm)에 따라
    /// 음절 문자의 코드 포인트를 결정하는 데 사용됩니다.
    ///
    /// - 초성(Cho): 0부터 18까지의 값을 가집니다. (채움 문자는 -1)
    /// - 중성(Jung): 0부터 20까지의 값을 가집니다. (채움 문자는 -1)
    /// - 종성(Jong): 0부터 27까지의 값을 가집니다. (종성 없음은 0)
    ///
    /// # 반환값
    ///
    /// 해당 자모의 순서 값을 나타내는 `i32` 정수.
    fn get_sequence(&self) -> i32;

    /// 유니코드 첫가끝(Korean Jamo) 영역(U+1100-U+11FF)의 문자를 반환합니다.
    ///
    /// 이 영역의 문자들은 주로 음절 조합 알고리즘이나 언어 처리 시스템 내부에서 사용됩니다.
    /// 일반적인 텍스트 표시에는 호환용 자모 영역 문자가 더 자주 사용될 수 있습니다.
    ///
    /// # 반환값
    ///
    /// 해당 자모의 첫가끝 유니코드 문자 `char`.
    /// 종성 비움(`Jong::E`)의 경우 널 문자(`\u{0000}`)를 반환합니다.
    /// 초성/중성 채움(`Cho::F`, `Jung::F`)의 경우 각각 해당하는 첫가끝 채움 문자를 반환합니다.
    fn get_unicode(&self) -> char;

    /// 유니코드 호환용 자모(Korean Compatibility Jamo) 영역(U+3130-U+318F)의 문자를 반환합니다.
    ///
    /// 이 영역의 문자들은 키보드 입력이나 일반 텍스트 표시 등에서 흔히 사용되는 완성형 형태의 자모 문자입니다.
    ///
    /// # 반환값
    ///
    /// 해당 자모의 호환용 유니코드 문자 `char`.
    /// 종성 비움(`Jong::E`)의 경우 널 문자(`\u{0000}`)를 반환합니다.
    /// 초성/중성 채움(`Cho::F`, `Jung::F`)의 경우 호환용 한글 채움 문자(`\u{3164}`)를 반환합니다.
    fn get_unicode_compat(&self) -> char;

    /// 유니코드 호환용 자모를 반환합니다. (호환성 메서드)
    fn to_char(&self) -> char {
        self.get_unicode_compat()
    }

    /// 초성인지 확인합니다.
    fn is_cho(&self) -> bool {
        false
    }
    /// 중성인지 확인합니다.
    fn is_jung(&self) -> bool {
        false
    }
    /// 종성인지 확인합니다.
    fn is_jong(&self) -> bool {
        false
    }
}

mod cho;
mod jong;
mod jung;

pub use cho::Cho;
pub use jong::Jong;
pub use jung::Jung;

pub use Cho as Chosung;
pub use Jong as Jongsung;
pub use Jung as Jungsung;

// --- Helper Functions ---

/// 주어진 `Jamo` 객체가 초성인지 판별합니다.
///
/// # 매개변수
///
/// * `o` - 검사할 Jamo 객체
///
/// # 반환값
///
/// 초성이면 `true`, 아니면 `false`.
///
/// # 예시
///
/// ```
/// use unim::hangul::jamo::{is_cho, Cho, Jung, Jong};
///
/// assert!(is_cho(&Cho::G));
/// assert!(is_cho(&Cho::F));
/// assert!(!is_cho(&Jung::A));
/// assert!(!is_cho(&Jong::G));
/// ```
pub fn is_cho<T: Jamo>(o: &T) -> bool {
    o.is_cho()
}

/// 주어진 `Jamo` 객체가 중성인지 판별합니다.
///
/// # 매개변수
///
/// * `o` - 검사할 Jamo 객체
///
/// # 반환값
///
/// 중성이면 `true`, 아니면 `false`.
///
/// # 예시
///
/// ```
/// use unim::hangul::jamo::{is_jung, Cho, Jung, Jong};
///
/// assert!(is_jung(&Jung::A));
/// assert!(is_jung(&Jung::F));
/// assert!(!is_jung(&Cho::G));
/// assert!(!is_jung(&Jong::G));
/// ```
pub fn is_jung<T: Jamo>(o: &T) -> bool {
    o.is_jung()
}

/// 주어진 `Jamo` 객체가 종성인지 판별합니다.
///
/// # 매개변수
///
/// * `o` - 검사할 Jamo 객체
///
/// # 반환값
///
/// 종성이면 `true`, 아니면 `false`.
///
/// # 예시
///
/// ```
/// use unim::hangul::jamo::{is_jong, Cho, Jung, Jong};
///
/// assert!(is_jong(&Jong::G));
/// assert!(is_jong(&Jong::E)); // 종성 없음도 종성 범위에 포함
/// assert!(!is_jong(&Cho::G));
/// assert!(!is_jong(&Jung::A));
/// ```
pub fn is_jong<T: Jamo>(o: &T) -> bool {
    o.is_jong()
}

/// 주어진 `Jamo` 객체가 한글 자모(초성, 중성, 종성)인지 판별합니다.
///
/// `is_cho()`, `is_jung()`, `is_jong()` 함수 중 하나라도 `true`를 반환하면 `true`를 반환합니다.
/// 이는 해당 객체가 유효한 한글 자모 순서 값을 가지고 있음을 의미합니다.
///
/// # 타입 매개변수
///
/// * `T`: `Jamo` 트레이트를 구현하는 타입.
///
/// # 매개변수
///
/// * `o`: 자모인지 검사할 `Jamo` 객체에 대한 참조.
///
/// # 반환값
///
/// 초성, 중성, 종성 중 하나이면 `true`, 아니면 `false`.
///
/// # 예시
///
/// ```
/// use unim::hangul::jamo::{is_jamo, Cho, Jung, Jong, JamoEnum};
///
/// assert!(is_jamo(&Cho::G));
/// assert!(is_jamo(&Jung::A));
/// assert!(is_jamo(&Jong::G));
/// assert!(is_jamo(&Jong::E)); // 종성 없음도 자모로 간주
///
/// // JamoEnum 예시
/// let cho_enum = JamoEnum::Cho(Cho::N);
/// assert!(is_jamo(&cho_enum));
///
/// // Special 문자는 자모가 아님
/// let special_enum = JamoEnum::Special('a');
/// // assert!(!is_jamo(&special_enum)); // JamoEnum::Special의 sequence는 -1이므로 is_cho나 is_jung은 true 반환 가능성 있음.
/// // 이 함수는 sequence 기반이므로 Special 문자의 경우 의도대로 동작하지 않을 수 있음.
/// // 타입을 직접 확인하는 것이 더 안전합니다.
/// ```
///
/// # 주의
/// 이 함수는 `get_sequence()` 값에만 의존하므로, `JamoEnum::Special`과 같이
/// 자모가 아니면서 우연히 자모 순서 범위 내의 값을 반환하는 `Jamo` 구현체가 있다면
/// 잘못된 결과를 반환할 수 있습니다. 타입을 명시적으로 확인하는 것이 더 안전할 수 있습니다.
pub fn is_jamo<T: Jamo>(o: &T) -> bool {
    is_cho(o) || is_jung(o) || is_jong(o)
}

/// 순서 값으로 초성을 얻습니다.
///
/// # 매개변수
///
/// * `seq` - 초성의 순서 값
///
/// # 반환값
///
/// 해당 순서 값의 초성이 있으면 `Some(Cho)`, 없으면 `None`을 반환합니다.
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

/// 순서 값으로 중성을 얻습니다.
///
/// # 매개변수
///
/// * `seq` - 중성의 순서 값
///
/// # 반환값
///
/// 해당 순서 값의 중성이 있으면 `Some(Jung)`, 없으면 `None`을 반환합니다.
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

/// 순서 값으로 종성을 얻습니다.
///
/// # 매개변수
///
/// * `seq` - 종성의 순서 값
///
/// # 반환값
///
/// 해당 순서 값의 종성이 있으면 `Some(Jong)`, 없으면 `None`을 반환합니다.
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

/// 문자열을 초성으로 변환하는 구현
///
/// 문자열로부터 초성을 생성합니다.
/// 한글 자모 문자나 영어 알파벳 표기를 사용할 수 있습니다.
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
            _ => Err(format!("유효하지 않은 초성 문자열입니다: '{}'", s)),
        }
    }
}

/// 문자열을 중성으로 변환하는 구현
///
/// 문자열로부터 중성을 생성합니다.
/// 한국어 자모 문자나 영어 알파벳 표기를 사용할 수 있습니다.
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
            _ => Err(format!("유효하지 않은 중성 문자열입니다: '{}'", s)),
        }
    }
}

/// 문자열을 종성으로 변환하는 구현
///
/// 문자열로부터 종성을 생성합니다.
/// 한국어 자모 문자나 영어 알파벳 표기를 사용할 수 있습니다.
/// 빈 문자열은 종성 비움(E)으로 처리됩니다.
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
            _ => Err(format!("유효하지 않은 종성 문자열입니다: '{}'", s)),
        }
    }
}

/// 자모를 통합적으로 다루는 열거형
///
/// 초성, 중성, 종성 및 특수 문자를 모두 포함할 수 있는 통합 타입입니다.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum JamoEnum {
    /// 초성
    Cho(Cho),
    /// 중성
    Jung(Jung),
    /// 종성
    Jong(Jong),
    /// 자모가 아닌 특수 문자
    Special(char),
}

impl Jamo for JamoEnum {
    /// 자모의 순서 값을 반환합니다.
    fn get_sequence(&self) -> i32 {
        match self {
            JamoEnum::Cho(cho) => cho.get_sequence(),
            JamoEnum::Jung(jung) => jung.get_sequence(),
            JamoEnum::Jong(jong) => jong.get_sequence(),
            JamoEnum::Special(_) => -1,
        }
    }

    /// 유니코드 첫가끝 영역의 문자를 반환합니다.
    fn get_unicode(&self) -> char {
        match self {
            JamoEnum::Cho(cho) => cho.get_unicode(),
            JamoEnum::Jung(jung) => jung.get_unicode(),
            JamoEnum::Jong(jong) => jong.get_unicode(),
            JamoEnum::Special(c) => *c,
        }
    }

    /// 유니코드 호환용 자모 영역의 문자를 반환합니다.
    fn get_unicode_compat(&self) -> char {
        match self {
            JamoEnum::Cho(cho) => cho.get_unicode_compat(),
            JamoEnum::Jung(jung) => jung.get_unicode_compat(),
            JamoEnum::Jong(jong) => jong.get_unicode_compat(),
            JamoEnum::Special(c) => *c,
        }
    }

    fn is_cho(&self) -> bool {
        matches!(self, JamoEnum::Cho(_))
    }

    fn is_jung(&self) -> bool {
        matches!(self, JamoEnum::Jung(_))
    }

    fn is_jong(&self) -> bool {
        matches!(self, JamoEnum::Jong(_))
    }
}

impl JamoEnum {
    /// 초성인지 확인합니다.
    #[inline]
    pub fn is_cho(&self) -> bool {
        matches!(self, JamoEnum::Cho(_))
    }

    /// 중성인지 확인합니다.
    #[inline]
    pub fn is_jung(&self) -> bool {
        matches!(self, JamoEnum::Jung(_))
    }

    /// 종성인지 확인합니다.
    #[inline]
    pub fn is_jong(&self) -> bool {
        matches!(self, JamoEnum::Jong(_))
    }

    /// 특수 문자인지 확인합니다.
    #[inline]
    pub fn is_special(&self) -> bool {
        matches!(self, JamoEnum::Special(_))
    }
}
