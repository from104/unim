use super::Jamo;
use super::Jong;
use crate::hangul::char::HangulError;

/// 한글 초성을 나타내는 열거형입니다.
///
/// 현대 한글에서 사용되는 19개의 초성과 음절 구성 시 초성이 없는 경우를 나타내는
/// 초성 채움 문자(`F`)를 포함합니다. 각 variant는 해당 초성의 로마자 표기법을 따릅니다.
///
/// # 예시
///
/// ```
/// use unim::hangul::jamo::{Cho, Jamo};
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
    /// use unim::hangul::jamo::Cho;
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
    /// use unim::hangul::jamo::{Cho, Jong};
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
