/// 한글 자모(초성, 중성, 종성)를 나타내는 타입들이 구현해야 하는 공통 트레이트입니다.
///
/// 이 트레이트는 각 자모 타입이 가져야 하는 기본적인 기능들을 정의합니다.
/// 예를 들어, 음절 조합에 사용될 순서 값이나 유니코드 문자 표현을 얻는 기능을 포함합니다.
pub trait Jamo: std::fmt::Debug + Clone + Copy + PartialEq + Eq + std::hash::Hash {
    /// 음절을 구성할 때 사용되는 자모의 순서 값을 반환합니다.
    ///
    /// 한글 음절은 초성, 중성, 종성의 순서로 조합됩니다. 이 순서 값은
    /// 유니코드 표준에 정의된 계산 방식(Hangul Syllables algorithm)에 따라
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

    /// 유니코드 첫가끝(Hangul Jamo) 영역(U+1100-U+11FF)의 문자를 반환합니다.
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

    /// 유니코드 호환용 자모(Hangul Compatibility Jamo) 영역(U+3130-U+318F)의 문자를 반환합니다.
    ///
    /// 이 영역의 문자들은 키보드 입력이나 일반 텍스트 표시 등에서 흔히 사용되는 완성형 형태의 자모 문자입니다.
    ///
    /// # 반환값
    ///
    /// 해당 자모의 호환용 유니코드 문자 `char`.
    /// 종성 비움(`Jong::E`)의 경우 널 문자(`\u{0000}`)를 반환합니다.
    /// 초성/중성 채움(`Cho::F`, `Jung::F`)의 경우 호환용 한글 채움 문자(`\u{3164}`)를 반환합니다.
    fn get_unicode_compat(&self) -> char;
}

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
    /// 초성 채움 문자 (U+115F). 음절에 초성이 없을 때 사용됩니다.
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
    /// ㅇ (ieung) - 초성으로 사용될 때 (예: "아")
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

impl Jamo for Cho {
    /// 초성의 순서 값을 반환합니다. (0 ~ 18, 채움 문자는 -1)
    #[inline]
    fn get_sequence(&self) -> i32 {
        *self as i32
    }

    /// 유니코드 첫가끝 영역의 초성 문자를 반환합니다.
    ///
    /// # 예시
    ///
    /// ```
    /// use unim::hangul::jamo::{Cho, Jamo};
    /// assert_eq!(Cho::G.get_unicode(), 'ᄀ'); // U+1100
    /// assert_eq!(Cho::F.get_unicode(), 'ᅟ'); // U+115F
    /// ```
    fn get_unicode(&self) -> char {
        match self {
            Cho::F => '\u{115f}',  // Hangul Choseong Filler
            Cho::G => '\u{1100}',  // ㄱ G
            Cho::GG => '\u{1101}', // ㄲ GG
            Cho::N => '\u{1102}',  // ㄴ N
            Cho::D => '\u{1103}',  // ㄷ D
            Cho::DD => '\u{1104}', // ㄸ DD
            Cho::R => '\u{1105}',  // ㄹ R
            Cho::M => '\u{1106}',  // ㅁ M
            Cho::B => '\u{1107}',  // ㅂ B
            Cho::BB => '\u{1108}', // ㅃ BB
            Cho::S => '\u{1109}',  // ㅅ S
            Cho::SS => '\u{110a}', // ㅆ SS
            Cho::E => '\u{110b}',  // ㅇ E
            Cho::J => '\u{110c}',  // ㅈ J
            Cho::JJ => '\u{110d}', // ㅉ JJ
            Cho::C => '\u{110e}',  // ㅊ C
            Cho::K => '\u{110f}',  // ㅋ K
            Cho::T => '\u{1110}',  // ㅌ T
            Cho::P => '\u{1111}',  // ㅍ P
            Cho::H => '\u{1112}',  // ㅎ H
        }
    }

    /// 유니코드 호환용 자모 영역의 초성 문자를 반환합니다.
    ///
    /// # 예시
    ///
    /// ```
    /// use unim::hangul::jamo::{Cho, Jamo};
    /// assert_eq!(Cho::G.get_unicode_compat(), 'ㄱ'); // U+3131
    /// assert_eq!(Cho::F.get_unicode_compat(), 'ㅤ'); // U+3164 (Hangul Filler)
    /// ```
    fn get_unicode_compat(&self) -> char {
        match self {
            Cho::F => '\u{3164}',  // Hangul Filler (호환용 초성 채움 문자는 별도로 없음)
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
    pub fn to_jong(&self) -> Result<Jong, &'static str> {
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
            Cho::DD => Err("초성 'ㄸ'는 종성으로 변환할 수 없습니다."),
            Cho::BB => Err("초성 'ㅃ'는 종성으로 변환할 수 없습니다."),
            Cho::JJ => Err("초성 'ㅉ'는 종성으로 변환할 수 없습니다."),
            Cho::F => Err("초성 채움은 종성으로 변환할 수 없습니다."),
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
    /// 중성 채움 문자 (U+1160). 음절에 중성이 없을 때 사용됩니다. (이론상으로만 존재, 실제 한글 표기에는 거의 사용되지 않음)
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

impl Jamo for Jung {
    /// 중성의 순서 값을 반환합니다. (0 ~ 20, 채움 문자는 -1)
    #[inline]
    fn get_sequence(&self) -> i32 {
        *self as i32
    }

    /// 유니코드 첫가끝 영역의 중성 문자를 반환합니다.
    ///
    /// # 예시
    ///
    /// ```
    /// use unim::hangul::jamo::{Jung, Jamo};
    /// assert_eq!(Jung::A.get_unicode(), 'ᅡ'); // U+1161
    /// assert_eq!(Jung::WA.get_unicode(), 'ᅪ'); // U+116A
    /// assert_eq!(Jung::F.get_unicode(), 'ᅠ'); // U+1160
    /// ```
    fn get_unicode(&self) -> char {
        match self {
            Jung::F => '\u{1160}',   // Hangul Jungseong Filler
            Jung::A => '\u{1161}',   // ㅏ A
            Jung::AE => '\u{1162}',  // ㅐ AE
            Jung::YA => '\u{1163}',  // ㅑ YA
            Jung::YAE => '\u{1164}', // ㅒ YAE
            Jung::EO => '\u{1165}',  // ㅓ EO
            Jung::E => '\u{1166}',   // ㅔ E
            Jung::YEO => '\u{1167}', // ㅕ YEO
            Jung::YE => '\u{1168}',  // ㅖ YE
            Jung::O => '\u{1169}',   // ㅗ O
            Jung::WA => '\u{116a}',  // ㅘ WA
            Jung::WAE => '\u{116b}', // ㅙ WAE
            Jung::OE => '\u{116c}',  // ㅚ OE
            Jung::YO => '\u{116d}',  // ㅛ YO
            Jung::U => '\u{116e}',   // ㅜ U
            Jung::WEO => '\u{116f}', // ㅝ WEO
            Jung::WE => '\u{1170}',  // ㅞ WE
            Jung::WI => '\u{1171}',  // ㅟ WI
            Jung::YU => '\u{1172}',  // ㅠ YU
            Jung::EU => '\u{1173}',  // ㅡ EU
            Jung::YI => '\u{1174}',  // ㅢ YI
            Jung::I => '\u{1175}',   // ㅣ I
        }
    }

    /// 유니코드 호환용 자모 영역의 중성 문자를 반환합니다.
    ///
    /// # 예시
    ///
    /// ```
    /// use unim::hangul::jamo::{Jung, Jamo};
    /// assert_eq!(Jung::A.get_unicode_compat(), 'ㅏ'); // U+314F
    /// assert_eq!(Jung::WA.get_unicode_compat(), 'ㅘ'); // U+3158
    /// assert_eq!(Jung::F.get_unicode_compat(), 'ㅤ'); // U+3164 (Hangul Filler)
    /// ```
    fn get_unicode_compat(&self) -> char {
        match self {
            Jung::F => '\u{3164}',   // Hangul Filler (호환용 중성 채움 문자는 별도로 없음)
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
    fn get_unicode(&self) -> char {
        match self {
            Jong::E => '\u{0000}',  // 종성 없음 (널 문자 사용)
            Jong::G => '\u{11a8}',  // ㄱ G
            Jong::GG => '\u{11a9}', // ㄲ GG
            Jong::GS => '\u{11aa}', // ㄳ GS
            Jong::N => '\u{11ab}',  // ㄴ N
            Jong::NJ => '\u{11ac}', // ㄵ NJ
            Jong::NH => '\u{11ad}', // ㄶ NH
            Jong::D => '\u{11ae}',  // ㄷ D
            Jong::L => '\u{11af}',  // ㄹ L
            Jong::LG => '\u{11b0}', // ㄺ LG
            Jong::LM => '\u{11b1}', // ㄻ LM
            Jong::LB => '\u{11b2}', // ㄼ LB
            Jong::LS => '\u{11b3}', // ㄽ LS
            Jong::LT => '\u{11b4}', // ㄾ LT
            Jong::LP => '\u{11b5}', // ㄿ LP
            Jong::LH => '\u{11b6}', // ㅀ LH
            Jong::M => '\u{11b7}',  // ㅁ M
            Jong::B => '\u{11b8}',  // ㅂ B
            Jong::BS => '\u{11b9}', // ㅄ BS
            Jong::S => '\u{11ba}',  // ㅅ S
            Jong::SS => '\u{11bb}', // ㅆ SS
            Jong::NG => '\u{11bc}', // ㅇ NG
            Jong::J => '\u{11bd}',  // ㅈ J
            Jong::C => '\u{11be}',  // ㅊ C
            Jong::K => '\u{11bf}',  // ㅋ K
            Jong::T => '\u{11c0}',  // ㅌ T
            Jong::P => '\u{11c1}',  // ㅍ P
            Jong::H => '\u{11c2}',  // ㅎ H
        }
    }

    /// 유니코드 호환용 자모 영역의 종성 문자를 반환합니다.
    /// 종성 비움(`E`)의 경우 널 문자(`\u{0000}`)를 반환합니다.
    ///
    /// # 예시
    ///
    /// ```
    /// use unim::hangul::jamo::{Jong, Jamo};
    /// assert_eq!(Jong::G.get_unicode_compat(), 'ㄱ'); // U+3131
    /// assert_eq!(Jong::LG.get_unicode_compat(), 'ㄺ'); // U+313A
    /// assert_eq!(Jong::E.get_unicode_compat(), '\u{0000}'); // Null
    /// ```
    fn get_unicode_compat(&self) -> char {
        match self {
            Jong::E => '\u{0000}',  // 종성 없음 (널 문자 사용)
            Jong::G => '\u{3131}',  // ㄱ
            Jong::GG => '\u{3132}', // ㄲ
            Jong::GS => '\u{3133}', // ㄳ
            Jong::N => '\u{3134}',  // ㄴ
            Jong::NJ => '\u{3135}', // ㄵ
            Jong::NH => '\u{3136}', // ㄶ
            Jong::D => '\u{3137}',  // ㄷ
            Jong::L => '\u{3139}',  // ㄹ
            Jong::LG => '\u{313a}', // ㄺ
            Jong::LM => '\u{313b}', // ㄻ
            Jong::LB => '\u{313c}', // ㄼ
            Jong::LS => '\u{313d}', // ㄽ
            Jong::LT => '\u{313e}', // ㄾ
            Jong::LP => '\u{313f}', // ㄿ
            Jong::LH => '\u{3140}', // ㅀ
            Jong::M => '\u{3141}',  // ㅁ
            Jong::B => '\u{3142}',  // ㅂ
            Jong::BS => '\u{3144}', // ㅄ
            Jong::S => '\u{3145}',  // ㅅ
            Jong::SS => '\u{3146}', // ㅆ
            Jong::NG => '\u{3147}', // ㅇ
            Jong::J => '\u{3148}',  // ㅈ
            Jong::C => '\u{314a}',  // ㅊ
            Jong::K => '\u{314b}',  // ㅋ
            Jong::T => '\u{314c}',  // ㅌ
            Jong::P => '\u{314d}',  // ㅍ
            Jong::H => '\u{314e}',  // ㅎ
        }
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
    pub fn to_cho(&self) -> Result<Cho, &'static str> {
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
            Jong::E => Err("종성 비움은 초성으로 변환할 수 없습니다."),
            Jong::GS => Err("겹받침 종성 'ㄳ'는(은) 초성으로 변환할 수 없습니다."),
            Jong::NJ => Err("겹받침 종성 'ㄵ'는(은) 초성으로 변환할 수 없습니다."),
            Jong::NH => Err("겹받침 종성 'ㄶ'는(은) 초성으로 변환할 수 없습니다."),
            Jong::LG => Err("겹받침 종성 'ㄺ'는(은) 초성으로 변환할 수 없습니다."),
            Jong::LM => Err("겹받침 종성 'ㄻ'는(은) 초성으로 변환할 수 없습니다."),
            Jong::LB => Err("겹받침 종성 'ㄼ'는(은) 초성으로 변환할 수 없습니다."),
            Jong::LS => Err("겹받침 종성 'ㄽ'는(은) 초성으로 변환할 수 없습니다."),
            Jong::LT => Err("겹받침 종성 'ㄾ'는(은) 초성으로 변환할 수 없습니다."),
            Jong::LP => Err("겹받침 종성 'ㄿ'는(은) 초성으로 변환할 수 없습니다."),
            Jong::LH => Err("겹받침 종성 'ㅀ'는(은) 초성으로 변환할 수 없습니다."),
            Jong::BS => Err("겹받침 종성 'ㅄ'는(은) 초성으로 변환할 수 없습니다."),
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
/// use unim::hangul::jamo::{is_cho, Cho, Jung, Jong};
///
/// assert!(is_cho(&Cho::G));
/// assert!(is_cho(&Cho::F));
/// assert!(!is_cho(&Jung::A));
/// assert!(!is_cho(&Jong::G));
/// ```
pub fn is_cho<T: Jamo>(o: &T) -> bool {
    matches!(o.get_sequence(), -1..=18)
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
    matches!(o.get_sequence(), -1..=20)
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
    matches!(o.get_sequence(), 0..=27)
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
/// 한글 자모 문자나 영문 알파벳 표기를 사용할 수 있습니다.
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
/// 한글 자모 문자나 영문 알파벳 표기를 사용할 수 있습니다.
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
/// 한글 자모 문자나 영문 알파벳 표기를 사용할 수 있습니다.
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
}
