use crate::hangul::char::HangulError;

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

/// 한글 초성을 나타내는 열거형입니다.
///
/// 현대 한글에서 사용되는 19개의 초성과 음절 구성 시 초성이 없는 경우를 나타내는
/// 초성 채움 문자(`F`)를 포함합니다. 각 variant는 해당 초성의 로마자 표기법을 따릅니다.
///
/// # 예시
///
/// ```
/// use unim::korean::jamo::{Cho, Jamo};
///
/// let giyeok = Cho::G;
/// assert_eq!(giyeok.get_sequence(), 0);
/// assert_eq!(giyeok.get_unicode(), '\u{1100}'); // ᄀ
/// assert_eq!(giyeok.get_unicode_compat(), '\u{3131}'); // ㄱ
///
/// let filler = Cho::F;
/// assert_eq!(filler.get_sequence(), -1);
/// assert_eq!(filler.get_unicode(), '\u{115f}'); // ᅟ
/// assert_eq!(filler.get_unicode_compat(), '\u{3164}'); // filler
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Cho {
    /// 초성 채움 문자 (U+115F).
    F = -1,
    /// ㄱ (giyeok)
    G = 0,
    /// ㄲ (ssanggiyeok)
    GG = 1,
    /// ㄴ (nieun)
    N = 2,
    /// ㄷ (digeut)
    D = 3,
    /// ㄸ (ssangdigeut)
    DD = 4,
    /// ㄹ (rieul)
    R = 5,
    /// ㅁ (mieum)
    M = 6,
    /// ㅂ (bieup)
    B = 7,
    /// ㅃ (ssangbieup)
    BB = 8,
    /// ㅅ (siot)
    S = 9,
    /// ㅆ (ssangsiot)
    SS = 10,
    /// ㅇ (ieung)
    E = 11,
    /// ㅈ (jieut)
    J = 12,
    /// ㅉ (ssangjieut)
    JJ = 13,
    /// ㅊ (chieut)
    C = 14,
    /// ㅋ (kieuk)
    K = 15,
    /// ㅌ (tieut)
    T = 16,
    /// ㅍ (pieup)
    P = 17,
    /// ㅎ (hieut)
    H = 18,
}

#[allow(non_upper_case_globals)]
impl Cho {
    pub const Filler: Cho = Cho::F;
    pub const Giyeok: Cho = Cho::G;
    pub const SsangGiyeok: Cho = Cho::GG;
    pub const Nieun: Cho = Cho::N;
    pub const Digeut: Cho = Cho::D;
    pub const SsangDigeut: Cho = Cho::DD;
    pub const Rieul: Cho = Cho::R;
    pub const Mieum: Cho = Cho::M;
    pub const Bieup: Cho = Cho::B;
    pub const SsangBieup: Cho = Cho::BB;
    pub const Siot: Cho = Cho::S;
    pub const SsangSiot: Cho = Cho::SS;
    pub const Ieung: Cho = Cho::E;
    pub const Jieut: Cho = Cho::J;
    pub const SsangJieut: Cho = Cho::JJ;
    pub const Chieut: Cho = Cho::C;
    pub const Kieuk: Cho = Cho::K;
    pub const Tieut: Cho = Cho::T;
    pub const Pieup: Cho = Cho::P;
    pub const Hieuh: Cho = Cho::H;
}

impl Jamo for Cho {
    /// 초성의 순서 값을 반환합니다. (0 ~ 18, 채움 문자는 -1)
    #[inline]
    fn get_sequence(&self) -> i32 {
        *self as i32
    }

    /// 유니코드 첫가끝 영역의 초성 문자를 반환합니다.
    ///
    /// 초성 유니코드는 U+1100부터 연속, 채움 문자는 U+115F입니다.
    #[inline]
    fn get_unicode(&self) -> char {
        const CHO_BASE: u32 = 0x1100; // ᄀ
        const CHO_FILLER: char = '\u{115F}'; // ᅟ

        match *self as i32 {
            -1 => CHO_FILLER,
            seq => char::from_u32(CHO_BASE + seq as u32).unwrap_or(CHO_FILLER),
        }
    }

    /// 유니코드 호환용 자모 영역의 초성 문자를 반환합니다.
    ///
    /// 호환용 자모는 연속되지 않아 배열 매핑을 사용합니다.
    #[inline]
    fn get_unicode_compat(&self) -> char {
        // 인덱스: F=-1 -> [0], G=0 -> [1], ... H=18 -> [19]
        const CHO_COMPAT: [char; 20] = [
            '\u{3164}', // F (Filler)
            '\u{3131}', // G  ㄱ
            '\u{3132}', // GG ㄲ
            '\u{3134}', // N  ㄴ
            '\u{3137}', // D  ㄷ
            '\u{3138}', // DD ㄸ
            '\u{3139}', // R  ㄹ
            '\u{3141}', // M  ㅁ
            '\u{3142}', // B  ㅂ
            '\u{3143}', // BB ㅃ
            '\u{3145}', // S  ㅅ
            '\u{3146}', // SS ㅆ
            '\u{3147}', // E  ㅇ
            '\u{3148}', // J  ㅈ
            '\u{3149}', // JJ ㅉ
            '\u{314a}', // C  ㅊ
            '\u{314b}', // K  ㅋ
            '\u{314c}', // T  ㅌ
            '\u{314d}', // P  ㅍ
            '\u{314e}', // H  ㅎ
        ];

        let idx = (*self as i32 + 1) as usize; // -1 -> 0, 0 -> 1, ..., 18 -> 19
        CHO_COMPAT.get(idx).copied().unwrap_or('\u{3164}')
    }
    fn to_char(&self) -> char {
        self.get_unicode_compat()
    }

    fn is_cho(&self) -> bool {
        true
    }
}

impl Cho {
    /// 순서 값으로부터 초성(`Cho`)을 생성합니다.
    ///
    /// 유효한 초성 순서 값(-1 ~ 18)이 주어지면 해당하는 `Cho` variant를 `Some`으로 감싸 반환하고,
    /// 그 외의 값이 주어지면 `None`을 반환합니다.
    ///
    /// # 매개변수
    ///
    /// * `seq`: 초성의 순서 값 (`i32`).
    ///
    /// # 반환값
    ///
    /// * `Some(Cho)`: 주어진 순서 값에 해당하는 초성.
    /// * `None`: 유효하지 않은 순서 값인 경우.
    ///
    /// # 예시
    ///
    /// ```
    /// use unim::korean::jamo::Cho;
    ///
    /// assert_eq!(Cho::from_sequence(0), Some(Cho::G));
    /// assert_eq!(Cho::from_sequence(11), Some(Cho::E));
    /// assert_eq!(Cho::from_sequence(-1), Some(Cho::F));
    /// assert_eq!(Cho::from_sequence(19), None);
    /// ```
    pub fn from_sequence(seq: i32) -> Option<Cho> {
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

    /// 주어진 초성을 해당하는 종성으로 변환합니다.
    ///
    /// 모든 초성이 종성으로 변환될 수 있는 것은 아닙니다. 예를 들어, 초성 'ㄸ'(DD), 'ㅃ'(BB), 'ㅉ'(JJ)는
    /// 현대 한글에서 종성으로 사용되지 않으므로 변환할 수 없습니다. 초성 채움 문자(`F`) 또한 변환할 수 없습니다.
    ///
    /// 변환 규칙:
    /// * `ㅇ`(E) -> `ㅇ`(NG) (종성 'ㅇ')
    /// * `ㄹ`(R) -> `ㄹ`(L)
    /// * 그 외 대부분의 홑자음 초성은 동일한 형태의 종성으로 변환됩니다. (예: `ㄱ`(G) -> `ㄱ`(G))
    ///
    /// # 반환값
    ///
    /// * `Ok(Jong)`: 변환에 성공한 경우, 해당하는 종성(`Jong`) variant.
    /// * `Err(&'static str)`: 변환할 수 없는 초성인 경우, 에러 메시지 문자열 슬라이스.
    ///
    /// # 예시
    ///
    /// ```
    /// use unim::korean::jamo::{Cho, Jong};
    ///
    /// assert_eq!(Cho::G.to_jong(), Ok(Jong::G)); // ㄱ -> ㄱ
    /// assert_eq!(Cho::E.to_jong(), Ok(Jong::NG)); // ㅇ -> ㅇ (종성)
    /// assert!(Cho::DD.to_jong().is_err()); // ㄸ는 변환 불가
    /// assert!(Cho::F.to_jong().is_err()); // 채움 문자는 변환 불가
    /// ```
    pub fn to_jong(&self) -> Result<Jong, HangulError> {
        match self {
            Cho::G => Ok(Jong::G),   // ㄱ
            Cho::GG => Ok(Jong::GG), // ㄲ
            Cho::N => Ok(Jong::N),   // ㄴ
            Cho::D => Ok(Jong::D),   // ㄷ
            Cho::R => Ok(Jong::L),   // ㄹ
            Cho::M => Ok(Jong::M),   // ㅁ
            Cho::B => Ok(Jong::B),   // ㅂ
            Cho::S => Ok(Jong::S),   // ㅅ
            Cho::SS => Ok(Jong::SS), // ㅆ
            Cho::E => Ok(Jong::NG),  // ㅇ (초성) -> ㅇ (종성)
            Cho::J => Ok(Jong::J),   // ㅈ
            Cho::C => Ok(Jong::C),   // ㅊ
            Cho::K => Ok(Jong::K),   // ㅋ
            Cho::T => Ok(Jong::T),   // ㅌ
            Cho::P => Ok(Jong::P),   // ㅍ
            Cho::H => Ok(Jong::H),   // ㅎ
            // 변환 불가 초성들
            Cho::DD => Err(HangulError::ConversionError(
                "Cho::DD cannot be converted to Jong",
            )),
            Cho::BB => Err(HangulError::ConversionError(
                "Cho::BB cannot be converted to Jong",
            )),
            Cho::JJ => Err(HangulError::ConversionError(
                "Cho::JJ cannot be converted to Jong",
            )),
            Cho::F => Err(HangulError::ConversionError(
                "Cho::F cannot be converted to Jong",
            )),
        }
    }
}

/// 한글 중성을 나타내는 열거형입니다.
///
/// 현대 한글에서 사용되는 21개의 중성(단모음 및 복모음)과 음절 구성 시 중성이 없는 경우를 나타내는
/// 중성 채움 문자(`F`)를 포함합니다. 각 variant는 해당 중성의 로마자 표기법을 따릅니다.
///
/// # 예시
///
/// ```
/// use unim::korean::jamo::{Jung, Jamo};
///
/// let a = Jung::A;
/// assert_eq!(a.get_sequence(), 0);
/// assert_eq!(a.get_unicode(), '\u{1161}'); // ᅡ
/// assert_eq!(a.get_unicode_compat(), '\u{314f}'); // ㅏ
///
/// let wa = Jung::WA;
/// assert_eq!(wa.get_sequence(), 9);
/// assert_eq!(wa.get_unicode(), '\u{116a}'); // ᅪ
/// assert_eq!(wa.get_unicode_compat(), '\u{3158}'); // ㅘ
///
/// let filler = Jung::F;
/// assert_eq!(filler.get_sequence(), -1);
/// assert_eq!(filler.get_unicode(), '\u{1160}'); // ᅠ
/// assert_eq!(filler.get_unicode_compat(), '\u{3164}'); // filler
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Jung {
    /// 중성 채움 문자 (U+1160).
    F = -1,
    /// ㅏ (a)
    A = 0,
    /// ㅐ (ae)
    AE = 1,
    /// ㅑ (ya)
    YA = 2,
    /// ㅒ (yae)
    YAE = 3,
    /// ㅓ (eo)
    EO = 4,
    /// ㅔ (e)
    E = 5,
    /// ㅕ (yeo)
    YEO = 6,
    /// ㅖ (ye)
    YE = 7,
    /// ㅗ (o)
    O = 8,
    /// ㅘ (wa)
    WA = 9,
    /// ㅙ (wae)
    WAE = 10,
    /// ㅚ (oe)
    OE = 11,
    /// ㅛ (yo)
    YO = 12,
    /// ㅜ (u)
    U = 13,
    /// ㅝ (weo)
    WEO = 14,
    /// ㅞ (we)
    WE = 15,
    /// ㅟ (wi)
    WI = 16,
    /// ㅠ (yu)
    YU = 17,
    /// ㅡ (eu)
    EU = 18,
    /// ㅢ (yi)
    YI = 19,
    /// ㅣ (i)
    I = 20,
}

#[allow(non_upper_case_globals)]
impl Jung {
    pub const Filler: Jung = Jung::F;
    pub const Ae: Jung = Jung::AE;
    pub const Ya: Jung = Jung::YA;
    pub const Yae: Jung = Jung::YAE;
    pub const Eo: Jung = Jung::EO;
    pub const Yeo: Jung = Jung::YEO;
    pub const Ye: Jung = Jung::YE;
    pub const Wa: Jung = Jung::WA;
    pub const Wae: Jung = Jung::WAE;
    pub const Oe: Jung = Jung::OE;
    pub const Yo: Jung = Jung::YO;
    pub const Weo: Jung = Jung::WEO;
    pub const We: Jung = Jung::WE;
    pub const Wi: Jung = Jung::WI;
    pub const Yu: Jung = Jung::YU;
    pub const Eu: Jung = Jung::EU;
    pub const Yi: Jung = Jung::YI;
}

impl Jamo for Jung {
    /// 중성의 순서 값을 반환합니다. (0 ~ 20, 채움 문자는 -1)
    #[inline]
    fn get_sequence(&self) -> i32 {
        *self as i32
    }

    /// 유니코드 첫가끝 영역의 중성 문자를 반환합니다.
    ///
    /// 중성 유니코드는 U+1160(채움)부터 연속입니다.
    #[inline]
    fn get_unicode(&self) -> char {
        const JUNG_BASE: u32 = 0x1161; // ᅡ (A)
        const JUNG_FILLER: char = '\u{1160}'; // ᅠ

        match *self as i32 {
            -1 => JUNG_FILLER,
            seq => char::from_u32(JUNG_BASE + seq as u32).unwrap_or(JUNG_FILLER),
        }
    }

    /// 유니코드 호환용 자모 영역의 중성 문자를 반환합니다.
    ///
    /// 중성 호환용 유니코드는 U+314F부터 연속입니다.
    #[inline]
    fn get_unicode_compat(&self) -> char {
        const JUNG_COMPAT_BASE: u32 = 0x314F; // ㅏ (A=0)
        const JUNG_FILLER: char = '\u{3164}';

        match *self as i32 {
            -1 => JUNG_FILLER,
            seq => char::from_u32(JUNG_COMPAT_BASE + seq as u32).unwrap_or(JUNG_FILLER),
        }
    }
    fn to_char(&self) -> char {
        self.get_unicode_compat()
    }

    fn is_jung(&self) -> bool {
        true
    }
}

impl Jung {
    /// 순서 값으로부터 중성(`Jung`)을 생성합니다.
    ///
    /// 유효한 중성 순서 값(-1 ~ 20)이 주어지면 해당하는 `Jung` variant를 `Some`으로 감싸 반환하고,
    /// 그 외의 값이 주어지면 `None`을 반환합니다.
    ///
    /// # 매개변수
    ///
    /// * `seq`: 중성의 순서 값 (`i32`).
    ///
    /// # 반환값
    ///
    /// * `Some(Jung)`: 주어진 순서 값에 해당하는 중성.
    /// * `None`: 유효하지 않은 순서 값인 경우.
    ///
    /// # 예시
    ///
    /// ```
    /// use unim::korean::jamo::Jung;
    ///
    /// assert_eq!(Jung::from_sequence(0), Some(Jung::A));
    /// assert_eq!(Jung::from_sequence(20), Some(Jung::I));
    /// assert_eq!(Jung::from_sequence(-1), Some(Jung::F));
    /// assert_eq!(Jung::from_sequence(21), None);
    /// ```
    pub fn from_sequence(seq: i32) -> Option<Jung> {
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
}

/// 한글 종성을 나타내는 열거형입니다.
///
/// 현대 한글에서 사용되는 27개의 종성(홑받침 및 겹받침)과 음절에 종성이 없는 경우를 나타내는
/// 종성 비움 문자(`E`)를 포함합니다. 각 variant는 해당 종성의 로마자 표기법을 따릅니다.
/// 종성 'ㅇ'은 `NG`로 표기됩니다.
///
/// # 예시
///
/// ```
/// use unim::korean::jamo::{Jong, Jamo};
///
/// let giyeok_batchim = Jong::G;
/// assert_eq!(giyeok_batchim.get_sequence(), 1);
/// assert_eq!(giyeok_batchim.get_unicode(), '\u{11a8}'); // ᆨ
/// assert_eq!(giyeok_batchim.get_unicode_compat(), '\u{3131}'); // ㄱ
///
/// let ieung_batchim = Jong::NG; // 종성 'ㅇ'
/// assert_eq!(ieung_batchim.get_sequence(), 21);
/// assert_eq!(ieung_batchim.get_unicode(), '\u{11bc}'); // ᆼ
/// assert_eq!(ieung_batchim.get_unicode_compat(), '\u{3147}'); // ㅇ
///
/// let no_batchim = Jong::E; // 종성 없음
/// assert_eq!(no_batchim.get_sequence(), 0);
/// assert_eq!(no_batchim.get_unicode(), '\u{0000}'); // Null character
/// assert_eq!(no_batchim.get_unicode_compat(), '\u{0000}'); // Null character
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Jong {
    /// 종성 비움 (받침 없음). 순서 값은 0입니다.
    E = 0,
    /// ㄱ (giyeok)
    G = 1,
    /// ㄲ (ssanggiyeok)
    GG = 2,
    /// ㄳ (giyeok-siot)
    GS = 3,
    /// ㄴ (nieun)
    N = 4,
    /// ㄵ (nieun-jieut)
    NJ = 5,
    /// ㄶ (nieun-hieut)
    NH = 6,
    /// ㄷ (digeut)
    D = 7,
    /// ㄹ (rieul)
    L = 8,
    /// ㄺ (rieul-giyeok)
    LG = 9,
    /// ㄻ (rieul-mieum)
    LM = 10,
    /// ㄼ (rieul-bieup)
    LB = 11,
    /// ㄽ (rieul-siot)
    LS = 12,
    /// ㄾ (rieul-tieut)
    LT = 13,
    /// ㄿ (rieul-pieup)
    LP = 14,
    /// ㅀ (rieul-hieut)
    LH = 15,
    /// ㅁ (mieum)
    M = 16,
    /// ㅂ (bieup)
    B = 17,
    /// ㅄ (bieup-siot)
    BS = 18,
    /// ㅅ (siot)
    S = 19,
    /// ㅆ (ssangsiot)
    SS = 20,
    /// ㅇ (ieung) - 종성으로 사용될 때 (예: "강")
    NG = 21,
    /// ㅈ (jieut)
    J = 22,
    /// ㅊ (chieut)
    C = 23,
    /// ㅋ (kieuk)
    K = 24,
    /// ㅌ (tieut)
    T = 25,
    /// ㅍ (pieup)
    P = 26,
    /// ㅎ (hieut)
    H = 27,
}

#[allow(non_upper_case_globals)]
impl Jong {
    pub const Empty: Jong = Jong::E;
    pub const Giyeok: Jong = Jong::G;
    pub const SsangGiyeok: Jong = Jong::GG;
    pub const GiyeokSiot: Jong = Jong::GS;
    pub const Nieun: Jong = Jong::N;
    pub const NieunJieut: Jong = Jong::NJ;
    pub const NieunHieuh: Jong = Jong::NH;
    pub const Digeut: Jong = Jong::D;
    pub const Rieul: Jong = Jong::L;
    pub const RieulGiyeok: Jong = Jong::LG;
    pub const RieulMieum: Jong = Jong::LM;
    pub const RieulBieup: Jong = Jong::LB;
    pub const RieulSiot: Jong = Jong::LS;
    pub const RieulTieut: Jong = Jong::LT;
    pub const RieulPieup: Jong = Jong::LP;
    pub const RieulHieuh: Jong = Jong::LH;
    pub const Mieum: Jong = Jong::M;
    pub const Bieup: Jong = Jong::B;
    pub const BieupSiot: Jong = Jong::BS;
    pub const Siot: Jong = Jong::S;
    pub const SsangSiot: Jong = Jong::SS;
    pub const Ieung: Jong = Jong::NG;
    pub const Jieut: Jong = Jong::J;
    pub const Chieut: Jong = Jong::C;
    pub const Kieuk: Jong = Jong::K;
    pub const Tieut: Jong = Jong::T;
    pub const Pieup: Jong = Jong::P;
    pub const Hieuh: Jong = Jong::H;
}

impl Jamo for Jong {
    /// 종성의 순서 값을 반환합니다. (0 ~ 27)
    #[inline]
    fn get_sequence(&self) -> i32 {
        *self as i32
    }

    /// 유니코드 첫가끝 영역의 종성 문자를 반환합니다.
    /// 종성 비움(`E`)의 경우 널 문자(`\u{0000}`)를 반환합니다.
    ///
    /// # 예시
    ///
    /// ```
    /// use unim::korean::jamo::{Jong, Jamo};
    /// assert_eq!(Jong::G.get_unicode(), 'ᆨ'); // U+11A8
    /// assert_eq!(Jong::LG.get_unicode(), 'ᆰ'); // U+11B0
    /// assert_eq!(Jong::E.get_unicode(), '\u{0000}'); // Null
    /// ```
    #[inline]
    fn get_unicode(&self) -> char {
        const JONG_BASE: u32 = 0x11A7; // E=0 -> '\0', seq=1 -> U+11A8

        match *self as i32 {
            0 => '\u{0000}', // 종성 없음
            seq => char::from_u32(JONG_BASE + seq as u32).unwrap_or('\u{0000}'),
        }
    }

    /// 유니코드 호환용 자모 영역의 종성 문자를 반환합니다.
    ///
    /// 종성 호환용 유니코드는 연속되지 않아 배열 매핑을 사용합니다.
    #[inline]
    fn get_unicode_compat(&self) -> char {
        // 인덱스: E=0 -> [0], G=1 -> [1], ..., H=27 -> [27]
        const JONG_COMPAT: [char; 28] = [
            '\u{0000}', // E  (없음)
            '\u{3131}', // G  ㄱ
            '\u{3132}', // GG ㄲ
            '\u{3133}', // GS ㄳ
            '\u{3134}', // N  ㄴ
            '\u{3135}', // NJ ㄵ
            '\u{3136}', // NH ㄶ
            '\u{3137}', // D  ㄷ
            '\u{3139}', // L  ㄹ
            '\u{313a}', // LG ㄺ
            '\u{313b}', // LM ㄻ
            '\u{313c}', // LB ㄼ
            '\u{313d}', // LS ㄽ
            '\u{313e}', // LT ㄾ
            '\u{313f}', // LP ㄿ
            '\u{3140}', // LH ㅀ
            '\u{3141}', // M  ㅁ
            '\u{3142}', // B  ㅂ
            '\u{3144}', // BS ㅄ
            '\u{3145}', // S  ㅅ
            '\u{3146}', // SS ㅆ
            '\u{3147}', // NG ㅇ
            '\u{3148}', // J  ㅈ
            '\u{314a}', // C  ㅊ
            '\u{314b}', // K  ㅋ
            '\u{314c}', // T  ㅌ
            '\u{314d}', // P  ㅍ
            '\u{314e}', // H  ㅎ
        ];

        let idx = *self as usize;
        JONG_COMPAT.get(idx).copied().unwrap_or('\u{0000}')
    }
    fn to_char(&self) -> char {
        self.get_unicode_compat()
    }

    fn is_jong(&self) -> bool {
        true
    }
}

impl Jong {
    /// 순서 값으로부터 종성(`Jong`)을 생성합니다.
    ///
    /// 유효한 종성 순서 값(0 ~ 27)이 주어지면 해당하는 `Jong` variant를 `Some`으로 감싸 반환하고,
    /// 그 외의 값이 주어지면 `None`을 반환합니다.
    ///
    /// # 매개변수
    ///
    /// * `seq`: 종성의 순서 값 (`i32`).
    ///
    /// # 반환값
    ///
    /// * `Some(Jong)`: 주어진 순서 값에 해당하는 종성.
    /// * `None`: 유효하지 않은 순서 값인 경우.
    ///
    /// # 예시
    ///
    /// ```
    /// use unim::korean::jamo::Jong;
    ///
    /// assert_eq!(Jong::from_sequence(1), Some(Jong::G)); // ㄱ 받침
    /// assert_eq!(Jong::from_sequence(21), Some(Jong::NG)); // ㅇ 받침
    /// assert_eq!(Jong::from_sequence(0), Some(Jong::E)); // 받침 없음
    /// assert_eq!(Jong::from_sequence(28), None);
    /// ```
    pub fn from_sequence(seq: i32) -> Option<Jong> {
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

    /// 주어진 종성을 해당하는 초성으로 변환합니다.
    ///
    /// 홑받침 종성만 초성으로 변환 가능합니다. 종성 'ㅇ'(NG)은 초성 'ㅇ'(E)으로 변환됩니다.
    /// 겹받침 종성이나 종성 비움(`E`)은 초성으로 변환할 수 없으며, 이 경우 패닉이 발생합니다.
    /// 이 함수는 주로 음운 규칙 적용 등 특정 상황에서 사용될 수 있습니다.
    ///
    /// # 반환값
    ///
    /// * `Ok(Cho)`: 변환된 초성(`Cho`) variant.
    /// * `Err(&'static str)`: 변환할 수 없는 종성인 경우, 에러 메시지 문자열 슬라이스.
    ///
    /// # 예시
    ///
    /// ```
    /// use unim::korean::jamo::{Cho, Jong};
    ///
    /// assert_eq!(Jong::G.to_cho(), Ok(Cho::G)); // ㄱ 받침 -> ㄱ 초성
    /// assert_eq!(Jong::NG.to_cho(), Ok(Cho::E)); // ㅇ 받침 -> ㅇ 초성
    /// // assert_eq!(Jong::LG.to_cho(), ...); // 패닉 발생!
    /// // assert_eq!(Jong::E.to_cho(), ...); // 패닉 발생!
    /// ```
    ///
    /// ```
    /// use unim::korean::jamo::Jong;
    /// // 겹받침 변환 시도 (Err 반환)
    /// assert!(Jong::LG.to_cho().is_err());
    /// ```
    ///
    /// ```
    /// use unim::korean::jamo::Jong;
    /// // 종성 비움 변환 시도 (Err 반환)
    /// assert!(Jong::E.to_cho().is_err());
    /// ```
    pub fn to_cho(&self) -> Result<Cho, HangulError> {
        match self {
            Jong::NG => Ok(Cho::E),
            Jong::L => Ok(Cho::R),
            Jong::G => Ok(Cho::G),
            Jong::GG => Ok(Cho::GG),
            Jong::N => Ok(Cho::N),
            Jong::D => Ok(Cho::D),
            Jong::M => Ok(Cho::M),
            Jong::B => Ok(Cho::B),
            Jong::S => Ok(Cho::S),
            Jong::SS => Ok(Cho::SS),
            Jong::J => Ok(Cho::J),
            Jong::C => Ok(Cho::C),
            Jong::K => Ok(Cho::K),
            Jong::T => Ok(Cho::T),
            Jong::P => Ok(Cho::P),
            Jong::H => Ok(Cho::H),
            Jong::E => Err(HangulError::ConversionError(
                "Jong::E cannot be converted to Cho",
            )),
            Jong::GS => Err(HangulError::ConversionError(
                "Jong::GS cannot be converted to Cho",
            )),
            Jong::NJ => Err(HangulError::ConversionError(
                "Jong::NJ cannot be converted to Cho",
            )),
            Jong::NH => Err(HangulError::ConversionError(
                "Jong::NH cannot be converted to Cho",
            )),
            Jong::LG => Err(HangulError::ConversionError(
                "Jong::LG cannot be converted to Cho",
            )),
            Jong::LM => Err(HangulError::ConversionError(
                "Jong::LM cannot be converted to Cho",
            )),
            Jong::LB => Err(HangulError::ConversionError(
                "Jong::LB cannot be converted to Cho",
            )),
            Jong::LS => Err(HangulError::ConversionError(
                "Jong::LS cannot be converted to Cho",
            )),
            Jong::LT => Err(HangulError::ConversionError(
                "Jong::LT cannot be converted to Cho",
            )),
            Jong::LP => Err(HangulError::ConversionError(
                "Jong::LP cannot be converted to Cho",
            )),
            Jong::LH => Err(HangulError::ConversionError(
                "Jong::LH cannot be converted to Cho",
            )),
            Jong::BS => Err(HangulError::ConversionError(
                "Jong::BS cannot be converted to Cho",
            )),
        }
    }
}

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
/// use unim::korean::jamo::{is_cho, Cho, Jung, Jong};
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
/// use unim::korean::jamo::{is_jung, Cho, Jung, Jong};
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
/// use unim::korean::jamo::{is_jong, Cho, Jung, Jong};
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
/// use unim::korean::jamo::{is_jamo, Cho, Jung, Jong, JamoEnum};
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

pub use Cho as Chosung;
pub use Jong as Jongsung;
pub use Jung as Jungsung;

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
