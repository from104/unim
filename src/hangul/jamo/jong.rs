use super::Cho;
use super::Jamo;
use crate::hangul::char::HangulError;

/// 한글 종성을 나타내는 열거형입니다.
///
/// 현대 한글에서 사용되는 27개의 종성(홑받침 및 겹받침)과 음절에 종성이 없는 경우를 나타내는
/// 종성 비움 문자(`E`)를 포함합니다. 각 variant는 해당 종성의 로마자 표기법을 따릅니다.
/// 종성 'ㅇ'은 `NG`로 표기됩니다.
///
/// # 예시
///
/// ```
/// use unim::hangul::jamo::{Jong, Jamo};
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
    /// use unim::hangul::jamo::{Jong, Jamo};
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
    /// use unim::hangul::jamo::Jong;
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
    /// use unim::hangul::jamo::{Cho, Jong};
    ///
    /// assert_eq!(Jong::G.to_cho(), Ok(Cho::G)); // ㄱ 받침 -> ㄱ 초성
    /// assert_eq!(Jong::NG.to_cho(), Ok(Cho::E)); // ㅇ 받침 -> ㅇ 초성
    /// // assert_eq!(Jong::LG.to_cho(), ...); // 패닉 발생!
    /// // assert_eq!(Jong::E.to_cho(), ...); // 패닉 발생!
    /// ```
    ///
    /// ```
    /// use unim::hangul::jamo::Jong;
    /// // 겹받침 변환 시도 (Err 반환)
    /// assert!(Jong::LG.to_cho().is_err());
    /// ```
    ///
    /// ```
    /// use unim::hangul::jamo::Jong;
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
