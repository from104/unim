use super::Jamo;

/// 한글 중성을 나타내는 열거형입니다.
///
/// 현대 한글에서 사용되는 21개의 중성(단모음 및 복모음)과 음절 구성 시 중성이 없는 경우를 나타내는
/// 중성 채움 문자(`F`)를 포함합니다. 각 variant는 해당 중성의 로마자 표기법을 따릅니다.
///
/// # 예시
///
/// ```
/// use unim::hangul::jamo::{Jung, Jamo};
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
    /// use unim::hangul::jamo::Jung;
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
