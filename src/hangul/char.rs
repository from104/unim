/**
 * 한글 음절 조합 및 검사 기능 (유니코드 5.2)
 *
 * 한글 자모를 사용하여 음절을 조합하거나 검사하는 기능을 제공합니다.
 * 유니코드 5.2 기준의 한글 음절 규칙을 따릅니다.
 */
use crate::hangul::jamo::*;

/// 디버그 로깅 매크로 (UNIM_DEVELOP=1 환경변수로 활성화)
macro_rules! unim_debug {
    ($($arg:tt)*) => {
        if std::env::var("UNIM_DEVELOP").map(|v| v == "1").unwrap_or(false) {
            eprintln!("[UNIM-CHAR] {}", format!($($arg)*));
        }
    };
}

/**
 * 한글 처리 관련 오류를 나타내는 열거형입니다.
 */
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HangulError {
    /// 유효하지 않은 한글 음절 코드가 입력되었을 때 발생하는 오류입니다.
    InvalidSyllable(char),
    /// 유효하지 않은 자모 순서값이 입력되었을 때 발생하는 오류입니다.
    InvalidSequence { kind: &'static str, value: i32 },
    /// 자모 문자열 파싱에 실패했을 때 발생하는 오류입니다.
    JamoParse(String),
    /// 초성, 중성, 종성 조합 규칙에 맞지 않아 음절을 생성할 수 없을 때 발생하는 오류입니다.
    CompositionFailure,
    /// 문자열로부터 `HangulChar`를 파싱하는 데 실패했을 때 발생하는 오류입니다.
    ParseError(String),
    /// 자모 변환에 실패했을 때 발생하는 오류입니다. (예: 초성→종성 변환 불가)
    ConversionError(&'static str),
}

impl fmt::Display for HangulError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            HangulError::InvalidSyllable(c) => {
                write!(f, "유효하지 않은 한글 음절 코드입니다: {}", c)
            }
            HangulError::InvalidSequence { kind, value } => {
                write!(f, "유효하지 않은 {} 순서값입니다: {}", kind, value)
            }
            HangulError::JamoParse(s) => write!(f, "자모 문자열 파싱 실패: {}", s),
            HangulError::CompositionFailure => write!(f, "올바른 한글 음절 조합이 아닙니다."),
            HangulError::ParseError(s) => write!(f, "HangulChar 파싱 실패: {}", s),
            HangulError::ConversionError(s) => write!(f, "자모 변환 실패: {}", s),
        }
    }
}

impl std::error::Error for HangulError {}

/**
 * 한글 문자 구조체
 */
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct HangulChar {
    // 초성,중성,종성
    choseong: Option<Cho>,
    jungseong: Option<Jung>,
    jongseong: Option<Jong>,
}

// 상수 정의
pub const CHOSEONG_NUMBER: usize = 19;
pub const JUNGSEONG_NUMBER: usize = 21;
pub const JONGSEONG_NUMBER: usize = 28;

// 한글 음절 유니코드 시작 코드
pub const SYLLABLE_BASE: u32 = 0xAC00;
// 한글 음절 갯수
pub const SYLLABLE_NUMBER: usize = CHOSEONG_NUMBER * JUNGSEONG_NUMBER * JONGSEONG_NUMBER;

impl HangulChar {
    /**
     * 한글 생성자
     */
    pub fn new() -> Self {
        HangulChar::default() // Default trait 구현으로 더 간결하게
    }

    /**
     * 한글 생성자 (초종중성 객체로)
     * @param cho
     * @param jung
     * @param jong
     */
    pub fn from_jamo_objects(cho: Option<Cho>, jung: Option<Jung>, jong: Option<Jong>) -> Self {
        HangulChar {
            choseong: cho,
            jungseong: jung,
            jongseong: jong,
        }
    }

    /**
     * 한글 생성자 (초종중성 순서로)
     * @param cho
     * @param jung
     * @param jong
     */
    pub fn from_jamo_sequences(cho_seq: i32, jung_seq: i32, jong_seq: i32) -> Self {
        HangulChar {
            choseong: get_cho_by_sequence(cho_seq),
            jungseong: get_jung_by_sequence(jung_seq),
            jongseong: get_jong_by_sequence(jong_seq),
        }
    }

    /**
     * 자모 이름 문자열로부터 한글 객체를 생성합니다.
     *
     * 각 자모 이름을 파싱하여 해당하는 초성, 중성, 종성 객체를 생성하고
     * 이를 조합하여 HangulChar 객체를 반환합니다.
     *
     * @param cho_name - 초성 이름 문자열 (예: "ㄱ", "G", "기역" 등)
     * @param jung_name - 중성 이름 문자열 (예: "ㅏ", "A", "아" 등)
     * @param jong_name - 종성 이름 문자열 (예: "ㄴ", "N", "니은" 등)
     * @return Result<HangulChar, HangulError> - 성공 시 HangulChar 객체, 실패 시 에러
     *
     * 특징:
     * - 빈 문자열이 입력되면 해당 자모는 None으로 설정됩니다.
     * - 종성의 경우 "E" 또는 빈 문자열이면 종성 없음(None)으로 처리됩니다.
     * - 모든 입력은 대소문자 구분 없이 처리됩니다(내부적으로 대문자로 변환).
     * - 파싱 실패 시 구체적인 에러 메시지와 함께 HangulError를 반환합니다.
     *
     * 예시:
     * - from_jamo_names("ㄱ", "ㅏ", "ㄴ") -> '간' 음절에 해당하는 HangulChar
     * - from_jamo_names("ㅎ", "ㅏ", "E") -> '하' 음절에 해당하는 HangulChar
     * - from_jamo_names("ㄱ", "", "") -> 'ㄱ' 자모에 해당하는 HangulChar
     */
    pub fn from_jamo_names(
        cho_name: &str,
        jung_name: &str,
        jong_name: &str,
    ) -> Result<Self, HangulError> {
        // 초성 파싱: 빈 문자열이면 None, 아니면 Cho 열거형으로 변환 시도
        // 대문자로 변환하여 대소문자 구분 없이 처리
        let cho = if cho_name.is_empty() {
            Ok(None)
        } else {
            Cho::from_str(cho_name.to_uppercase().as_str())
                .map(Some)
                .map_err(|_| HangulError::JamoParse(format!("초성 파싱 실패: {}", cho_name)))
        }?;

        // 중성 파싱: 빈 문자열이면 None, 아니면 Jung 열거형으로 변환 시도
        // 대문자로 변환하여 대소문자 구분 없이 처리
        let jung = if jung_name.is_empty() {
            Ok(None)
        } else {
            Jung::from_str(jung_name.to_uppercase().as_str())
                .map(Some)
                .map_err(|_| HangulError::JamoParse(format!("중성 파싱 실패: {}", jung_name)))
        }?;

        // 종성 파싱: 빈 문자열이거나 "E"(종성 없음)이면 None, 아니면 Jong 열거형으로 변환 시도
        // 대문자로 변환하여 대소문자 구분 없이 처리
        let jong = if jong_name.is_empty() || jong_name.to_uppercase() == "E" {
            Ok(None)
        } else {
            Jong::from_str(jong_name.to_uppercase().as_str())
                .map(Some)
                .map_err(|_| HangulError::JamoParse(format!("종성 파싱 실패: {}", jong_name)))
        }?;

        // 파싱된 자모들로 HangulChar 객체 생성 및 반환
        Ok(HangulChar {
            choseong: cho,
            jungseong: jung,
            jongseong: jong,
        })
    }

    /**
     * 한글 음절 문자로부터 `HangulChar` 객체를 생성합니다.
     *
     * 유니코드 한글 음절 영역(0xAC00-0xD7A3)에 있는 문자를 분해하여
     * 초성, 중성, 종성 정보를 가진 `HangulChar` 객체로 변환합니다.
     *
     * # 작동 원리
     * 1. 입력된 문자가 한글 음절 영역에 있는지 검사합니다.
     * 2. 한글 음절 시작점(0xAC00)을 기준으로 상대적 위치를 계산합니다.
     * 3. 상대적 위치를 이용해 초성, 중성, 종성의 인덱스를 계산합니다:
     *    - 초성 인덱스 = 상대코드 / (중성 수 * 종성 수)
     *    - 중성 인덱스 = (상대코드 % (중성 수 * 종성 수)) / 종성 수
     *    - 종성 인덱스 = 상대코드 % 종성 수
     * 4. 계산된 인덱스로 `HangulChar` 객체를 생성합니다.
     *
     * # 매개변수
     * * `syllable` - 분해할 한글 음절 문자 (예: '가', '한', '꿈')
     *
     * # 반환값
     * * `Ok(HangulChar)` - 성공적으로 분해된 `HangulChar` 객체
     * * `Err(HangulError::InvalidSyllable)` - 입력된 문자가 한글 음절 영역에 없는 경우
     *
     * # 예시
     * ```
     * use unim::hangul::*;
     * let korean = HangulChar::from_syllable('한').unwrap();
     * assert_eq!(korean.get_cho(), Some(Cho::H));
     * assert_eq!(korean.get_jung(), Some(Jung::A));
     * assert_eq!(korean.get_jong(), Some(Jong::N));
     * ```
     */
    pub fn from_syllable(syllable: char) -> Result<Self, HangulError> {
        unim_debug!("HangulChar::from_syllable: '{}'", syllable);
        let syllable_u32 = syllable as u32;

        // 입력된 문자가 한글 음절 영역(0xAC00-0xD7A3)에 있는지 검사
        if !(SYLLABLE_BASE..=SYLLABLE_BASE + SYLLABLE_NUMBER as u32 - 1).contains(&syllable_u32) {
            return Err(HangulError::InvalidSyllable(syllable));
        }

        // 한글 음절 시작점(0xAC00)을 기준으로 상대적 위치 계산
        let relative_code = (syllable_u32 - SYLLABLE_BASE) as usize;

        // 초성, 중성, 종성의 인덱스 계산
        let cho_seq = relative_code / (JUNGSEONG_NUMBER * JONGSEONG_NUMBER);
        let jung_seq = (relative_code % (JUNGSEONG_NUMBER * JONGSEONG_NUMBER)) / JONGSEONG_NUMBER;
        let jong_seq = relative_code % JONGSEONG_NUMBER;

        // 계산된 인덱스로 HangulChar 객체 생성 및 반환
        Ok(HangulChar::from_jamo_sequences(
            cho_seq as i32,
            jung_seq as i32,
            jong_seq as i32,
        ))
    }

    /// 한글 음절 문자로 객체 생성 (호환성 메서드)
    #[inline]
    pub fn from_char(c: char) -> Option<Self> {
        Self::from_syllable(c).ok()
    }

    /**
     * 한글 자모 정하기 (초종중성 객체로)
     * @param cho
     * @param jung
     * @param jong
     */
    pub fn set_jamo_objects(
        &mut self,
        cho: Option<Cho>,
        jung: Option<Jung>,
        jong: Option<Jong>,
    ) -> bool {
        self.set_cho_object(cho) & self.set_jung_object(jung) & self.set_jong_object(jong)
    }

    /**
     * 한글 자모 정하기 (초종중성 순서로)
     * @param cho
     * @param jung
     * @param jong
     */
    pub fn set_jamo_sequences(&mut self, cho_seq: i32, jung_seq: i32, jong_seq: i32) -> bool {
        self.set_cho_sequence(cho_seq)
            & self.set_jung_sequence(jung_seq)
            & self.set_jong_sequence(jong_seq)
    }

    /**
     * 한글 자모 정하기 (초종중성 객체 이름으로)
     * @param cho
     * @param jung
     * @param jong
     */
    pub fn set_jamo_names(&mut self, cho_name: &str, jung_name: &str, jong_name: &str) -> bool {
        self.set_cho_name(cho_name) & self.set_jung_name(jung_name) & self.set_jong_name(jong_name)
    }

    /**
     * 한글 자모 정하기 (한글 음절로)
     * @param syllable
     */
    pub fn set_jamo_by_syllable(&mut self, syllable: char) -> Result<(), HangulError> {
        let temp_char = HangulChar::from_syllable(syllable)?;
        *self = temp_char;
        Ok(())
    }

    /**
     * 초성 정하기 (객체로)
     * @param cho
     */
    pub fn set_cho_object(&mut self, cho: Option<Cho>) -> bool {
        self.choseong = cho;
        self.choseong.is_some()
    }

    /**
     * 중성 정하기 (객체로)
     * @param jung
     */
    pub fn set_jung_object(&mut self, jung: Option<Jung>) -> bool {
        self.jungseong = jung;
        self.jungseong.is_some()
    }

    /**
     * 종성 정하기 (객체로)
     * @param jong
     */
    pub fn set_jong_object(&mut self, jong: Option<Jong>) -> bool {
        self.jongseong = jong;
        self.jongseong.is_some()
    }

    /**
     * 초성 정하기 (순서로)
     * @param cho
     */
    pub fn set_cho_sequence(&mut self, cho_seq: i32) -> bool {
        if cho_seq >= 0 && cho_seq < CHOSEONG_NUMBER as i32 {
            self.set_cho_object(get_cho_by_sequence(cho_seq));
            true
        } else {
            self.clear_cho();
            false
        }
    }

    /**
     * 중성 정하기 (순서로)
     * @param jung
     */
    pub fn set_jung_sequence(&mut self, jung_seq: i32) -> bool {
        if jung_seq >= 0 && jung_seq < JUNGSEONG_NUMBER as i32 {
            self.set_jung_object(get_jung_by_sequence(jung_seq));
            true
        } else {
            self.clear_jung();
            false
        }
    }

    /**
     * 종성 정하기 (순서로)
     * @param jong
     */
    pub fn set_jong_sequence(&mut self, jong_seq: i32) -> bool {
        if jong_seq >= 0 && jong_seq < JONGSEONG_NUMBER as i32 {
            self.set_jong_object(get_jong_by_sequence(jong_seq));
            true
        } else {
            self.clear_jong();
            false
        }
    }

    /**
     * 초성 정하기 (객체 이름으로)
     * @param cho
     */
    pub fn set_cho_name(&mut self, cho_name: &str) -> bool {
        match Cho::from_str(cho_name.to_uppercase().as_str()) {
            Ok(c) => {
                self.set_cho_object(Some(c));
                true
            }
            Err(_) => {
                self.clear_cho();
                false
            }
        }
    }

    /**
     * 중성 정하기 (객체 이름으로)
     * @param jung
     */
    pub fn set_jung_name(&mut self, jung_name: &str) -> bool {
        match Jung::from_str(jung_name.to_uppercase().as_str()) {
            Ok(j) => {
                self.set_jung_object(Some(j));
                true
            }
            Err(_) => {
                self.clear_jung();
                false
            }
        }
    }

    /**
     * 종성 정하기 (객체 이름으로)
     * @param jong
     */
    pub fn set_jong_name(&mut self, jong_name: &str) -> bool {
        match Jong::from_str(jong_name.to_uppercase().as_str()) {
            Ok(jo) => {
                self.set_jong_object(Some(jo));
                true
            }
            Err(_) => {
                self.clear_jong();
                false
            }
        }
    }

    /**
     * 초성 지우기
     */
    pub fn clear_cho(&mut self) {
        self.choseong = None;
    }

    /**
     * 중성 지우기
     */
    pub fn clear_jung(&mut self) {
        self.jungseong = None;
    }

    /**
     * 종성 지우기
     */
    pub fn clear_jong(&mut self) {
        self.jongseong = None;
    }

    /**
     * 자모 모두 지우기
     */
    pub fn clear(&mut self) {
        self.clear_cho();
        self.clear_jung();
        self.clear_jong();
    }

    /**
     * 초성이 있는지 여부
     * @return
     */
    pub fn is_filled_cho(&self) -> bool {
        self.choseong.is_some()
    }

    /**
     * 중성이 있는지 여부
     * @return
     */
    pub fn is_filled_jung(&self) -> bool {
        self.jungseong.is_some()
    }

    /**
     * 종성이 있는지 여부
     * @return
     */
    pub fn is_filled_jong(&self) -> bool {
        self.jongseong.is_some()
    }

    /**
     * 초성만 있는지 여부
     * @return
     */
    pub fn is_filled_only_cho(&self) -> bool {
        self.is_filled_cho() && !self.is_filled_jung() && !self.is_filled_jong()
    }

    /**
     * 중성만 있는지 여부
     * @return
     */
    pub fn is_filled_only_jung(&self) -> bool {
        !self.is_filled_cho() && self.is_filled_jung() && !self.is_filled_jong()
    }

    /**
     * 종성만 있는지 여부
     * @return
     */
    pub fn is_filled_only_jong(&self) -> bool {
        !self.is_filled_cho() && !self.is_filled_jung() && self.is_filled_jong()
    }

    /**
     * 자모가 다 비었는지 여부
     * @return
     */
    pub fn is_empty(&self) -> bool {
        !self.is_filled_cho() && !self.is_filled_jung() && !self.is_filled_jong()
    }

    /**
     * 초성 얻기
     * @return
     */
    pub fn get_cho(&self) -> Option<Cho> {
        self.choseong
    }

    /**
     * 중성 얻기
     * @return
     */
    pub fn get_jung(&self) -> Option<Jung> {
        self.jungseong
    }

    /**
     * 종성 얻기
     * @return
     */
    pub fn get_jong(&self) -> Option<Jong> {
        self.jongseong
    }

    /**
     * 초성 유니코드(첫가끝) 얻기
     * @return
     */
    pub fn get_cho_unicode(&self) -> char {
        self.choseong.map_or('\u{0}', |c| c.get_unicode()) // Option 사용하여 안전하게 접근
    }

    /**
     * 중성 유니코드(첫가끝) 얻기
     * @return
     */
    pub fn get_jung_unicode(&self) -> char {
        self.jungseong.map_or('\u{0}', |j| j.get_unicode())
    }

    /**
     * 종성 유니코드(첫가끝) 얻기
     * @return
     */
    pub fn get_jong_unicode(&self) -> char {
        self.jongseong.map_or('\u{0}', |jo| {
            if jo.get_sequence() != 0 {
                jo.get_unicode()
            } else {
                '\u{0}'
            }
        })
    }

    /**
     * 초성 유니코드(호환용) 얻기
     * @return
     */
    pub fn get_cho_unicode_compat(&self) -> char {
        self.choseong.map_or('\u{0}', |c| c.get_unicode_compat())
    }

    /**
     * 중성 유니코드(호환용) 얻기
     * @return
     */
    pub fn get_jung_unicode_compat(&self) -> char {
        self.jungseong.map_or('\u{0}', |j| j.get_unicode_compat())
    }

    /**
     * 종성 유니코드(호환용) 얻기
     * @return
     */
    pub fn get_jong_unicode_compat(&self) -> char {
        self.jongseong.map_or('\u{0}', |jo| {
            if jo.get_sequence() != 0 {
                jo.get_unicode_compat()
            } else {
                '\u{0}'
            }
        })
    }

    /**
     * 한글 첫가끝 조합된 유니코드 얻기
     * @return 첫가끝 문자 배열
     */
    pub fn get_unicodes(&self) -> Result<Vec<char>, HangulError> {
        match (self.choseong, self.jungseong, self.jongseong) {
            (Some(cho), Some(jung), Some(jong)) => Ok(vec![
                cho.get_unicode(),
                jung.get_unicode(),
                jong.get_unicode(),
            ]),
            (Some(cho), Some(jung), None) => Ok(vec![cho.get_unicode(), jung.get_unicode()]),
            _ => Err(HangulError::CompositionFailure), // 그 외는 유효한 첫가끝 조합 아님
        }
    }

    /**
     * 한글 음절 얻기
     * @return 한글 음절 또는 호환용 자모
     */
    pub fn get_syllable(&self) -> Result<char, HangulError> {
        unim_debug!(
            "get_syllable: cho={:?}, jung={:?}, jong={:?}",
            self.choseong,
            self.jungseong,
            self.jongseong
        );

        // 채움 문자(Filler, Empty)를 None으로 정규화
        let cho = self.choseong.filter(|c| *c != Cho::Filler);
        let jung = self.jungseong.filter(|j| *j != Jung::Filler);
        let jong = self.jongseong.filter(|j| *j != Jong::Empty);

        let result = match (cho, jung, jong) {
            // 완전한 음절 (초성 + 중성 + 종성)
            (Some(c), Some(j), Some(jo)) => {
                let cho_seq = c.get_sequence() as u32;
                let jung_seq = j.get_sequence() as u32;
                let jong_seq = jo.get_sequence() as u32;

                let syllable_code = SYLLABLE_BASE
                    + cho_seq * (JUNGSEONG_NUMBER * JONGSEONG_NUMBER) as u32
                    + jung_seq * JONGSEONG_NUMBER as u32
                    + jong_seq;

                char::from_u32(syllable_code).ok_or(HangulError::InvalidSyllable(
                    char::from_u32(syllable_code).unwrap_or('?'),
                ))
            }
            // 완전한 음절 (초성 + 중성, 종성 없음)
            (Some(c), Some(j), None) => {
                let cho_seq = c.get_sequence() as u32;
                let jung_seq = j.get_sequence() as u32;

                let syllable_code = SYLLABLE_BASE
                    + cho_seq * (JUNGSEONG_NUMBER * JONGSEONG_NUMBER) as u32
                    + jung_seq * JONGSEONG_NUMBER as u32;

                char::from_u32(syllable_code).ok_or(HangulError::InvalidSyllable(
                    char::from_u32(syllable_code).unwrap_or('?'),
                ))
            }
            // --- 단독 자모 처리 ---
            (Some(c), None, None) => Ok(c.get_unicode_compat()), // 초성만
            (None, Some(j), None) => Ok(j.get_unicode_compat()), // 중성만
            (None, None, Some(jo)) => Ok(jo.get_unicode_compat()), // 종성만
            _ => Err(HangulError::CompositionFailure),           // 그 외 조합 불가
        };
        unim_debug!("  -> result: {:?}", result);
        result
    }

    /// `HangulChar`가 단일 자모를 나타내는 경우 `JamoEnum`으로 변환합니다.
    pub fn get_jamo_enum(&self) -> Result<JamoEnum, HangulError> {
        match (self.choseong, self.jungseong, self.jongseong) {
            (Some(c), None, None) => Ok(JamoEnum::Cho(c)),
            (None, Some(j), None) => Ok(JamoEnum::Jung(j)),
            (None, None, Some(j)) => Ok(JamoEnum::Jong(j)),
            _ => Err(HangulError::CompositionFailure),
        }
    }

    /// 자모 튜플 반환 (호환성 메서드)
    pub fn to_jamo_tuple(&self) -> Option<(Cho, Jung, Option<Jong>)> {
        match (self.choseong, self.jungseong, self.jongseong) {
            (Some(c), Some(j), jo) => Some((c, j, jo.filter(|&j| j != Jong::Empty))),
            _ => None,
        }
    }

    /// 자모 열거형 반환 (호환성 메서드)
    pub fn to_jamo(&self) -> Option<JamoEnum> {
        self.get_jamo_enum().ok()
    }

    /// 한글 음절 얻기 (호환성 메서드)
    #[inline]
    pub fn to_char(&self) -> char {
        self.get_syllable().unwrap_or('\u{0000}')
    }

    /**
     * 현재 `HangulChar` 객체에 설정된 자모들을 호환용 자모 문자열로 변환합니다.
     *
     * 초성, 중성, 종성이 설정된 경우, 각각에 해당하는 호환용 한글 자모 문자를 찾아
     * 순서대로 이어붙인 문자열을 반환합니다.
     * 설정되지 않은 성분은 무시됩니다.
     *
     * # 반환값
     * * `String` - 변환된 호환용 자모 문자열 (예: "ㄱㅏㄴ" 또는 "ㅎㅏ")
     *
     * # 예시
     * ```
     * use unim::hangul::*;
     * let korean = HangulChar::from_syllable('한').unwrap();
     * assert_eq!(korean.to_compat_jamo_string(), "ㅎㅏㄴ");
     *
     * let korean_ga = HangulChar::from_syllable('가').unwrap();
     * assert_eq!(korean_ga.to_compat_jamo_string(), "ㄱㅏ");
     *
     * let mut korean_g = HangulChar::new();
     * korean_g.set_cho_object(Some(Cho::G));
     * assert_eq!(korean_g.to_compat_jamo_string(), "ㄱ");
     * ```
     */
    pub fn to_compat_jamo_string(&self) -> String {
        let mut s = String::new();
        if let Some(cho) = self.choseong {
            s.push(JamoEnum::Cho(cho).get_unicode_compat());
        }
        if let Some(jung) = self.jungseong {
            s.push(JamoEnum::Jung(jung).get_unicode_compat());
        }
        // 종성은 None이 아닌 경우, 그리고 실제 종성 값이 있는 경우 (Jong::E 제외)에만 추가
        if let Some(jong) = self.jongseong {
            if jong != Jong::E {
                // 종성 없음을 나타내는 Jong::E는 추가하지 않음
                s.push(JamoEnum::Jong(jong).get_unicode_compat());
            }
        }
        s
    }
}

use std::fmt;
use std::str::FromStr;

impl fmt::Display for HangulChar {
    /**
     * 조합된 한글을 문자얼로 변환합니다.
     * 조합이 가능한 경우 한글 음절 문자를 반환하고,
     * 불가능한 경우(예: 자모가 하나만 있거나 잘못된 조합) 호환용 자모들을 반환합니다.
     */
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.get_syllable() {
            Ok(syllable) => write!(f, "{}", syllable),
            Err(_) => write!(f, "{}", self.to_compat_jamo_string()),
        }
    }
}

impl FromStr for HangulChar {
    type Err = HangulError;

    /**
     * 문자열로부터 `HangulChar`를 생성합니다.
     *
     * 다음 형식의 문자열을 파싱할 수 있습니다:
     * 1. 한글 음절 문자 하나 (예: "한", "가")
     * 2. 한글 호환용 자모 문자 하나 (예: "ㄱ", "ㅏ", "ㄴ") - 파싱 시 해당 자모만 설정됨
     *
     * 다른 형식의 문자열은 오류를 반환합니다.
     *
     * @param s 파싱할 문자열
     * @return Ok(HangulChar) 파싱 성공
     * @return Err(HangulError::ParseError) 파싱 실패 (유효하지 않은 형식)
     * @return Err(HangulError::InvalidSyllable) `from_syllable` 내부 오류
     */
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let chars: Vec<char> = s.chars().collect();

        if chars.len() != 1 {
            return Err(HangulError::ParseError(format!(
                "길이가 1인 문자열이어야 합니다: '{}'",
                s
            )));
        }

        let c = chars[0];
        let c_u32 = c as u32;

        // 1. 한글 음절 범위 확인
        if (SYLLABLE_BASE..=SYLLABLE_BASE + SYLLABLE_NUMBER as u32 - 1).contains(&c_u32) {
            return HangulChar::from_syllable(c); // InvalidSyllable 오류 전파 가능
        }

        // 2. 호환용 자모 확인 (jamo.rs의 FromStr 활용)
        if let Ok(cho) = Cho::from_str(s) {
            Ok(HangulChar::from_jamo_objects(Some(cho), None, None))
        } else if let Ok(jung) = Jung::from_str(s) {
            Ok(HangulChar::from_jamo_objects(None, Some(jung), None))
        } else if let Ok(jong) = Jong::from_str(s) {
            Ok(HangulChar::from_jamo_objects(None, None, Some(jong)))
        } else {
            Err(HangulError::ParseError(format!(
                "한글 음절 또는 호환용 자모가 아닙니다: '{}'",
                s
            )))
        }
    }
}

/// 한글 문자 관련 확장 메서드를 제공하는 trait입니다.
pub trait HangulCharExt {
    /// 현재 문자가 완성형 한글 음절인지 확인합니다.
    fn is_hangul_syllable(&self) -> bool;

    /// 현재 문자가 호환용 한글 자모인지 확인합니다. (0x3130-0x318F)
    fn is_hangul_compat_jamo(&self) -> bool;

    /// 현재 문자가 첫가끝 영역의 초성인지 확인합니다. (0x1100-0x115F)
    fn is_hangul_choseong(&self) -> bool;

    /// 현재 문자가 첫가끝 영역의 중성인지 확인합니다. (0x1160-0x11A7)
    fn is_hangul_jungseong(&self) -> bool;

    /// 현재 문자가 첫가끝 영역의 종성인지 확인합니다. (0x11A8-0x11FF)
    fn is_hangul_jongseong(&self) -> bool;

    /// 현재 문자가 첫가끝 영역의 자모인지 확인합니다. (0x1100-0x11FF)
    fn is_hangul_jamo(&self) -> bool;

    /// 현재 문자가 한글(완성형 음절, 첫가끝 자모, 호환용 자모)인지 확인합니다.
    fn is_hangul(&self) -> bool;

    // --- Compatibility Methods ---
    fn is_korean_syllable(&self) -> bool {
        self.is_hangul_syllable()
    }
    fn is_korean_compat_jamo(&self) -> bool {
        self.is_hangul_compat_jamo()
    }
    fn is_korean_choseong(&self) -> bool {
        self.is_hangul_choseong()
    }
    fn is_korean_jungseong(&self) -> bool {
        self.is_hangul_jungseong()
    }
    fn is_korean_jongseong(&self) -> bool {
        self.is_hangul_jongseong()
    }
    fn is_korean_jamo(&self) -> bool {
        self.is_hangul_jamo()
    }
    fn is_korean(&self) -> bool {
        self.is_hangul()
    }
}

impl HangulCharExt for char {
    fn is_hangul_syllable(&self) -> bool {
        let c = *self as u32;
        (SYLLABLE_BASE..=SYLLABLE_BASE + SYLLABLE_NUMBER as u32 - 1).contains(&c)
    }

    fn is_hangul_compat_jamo(&self) -> bool {
        let c = *self as u32;
        // 호환용 한글 자모 영역 (0x3130-0x318F)
        (0x3130..=0x318F).contains(&c)
    }

    fn is_hangul_choseong(&self) -> bool {
        let c = *self as u32;
        // 첫가끝 초성 영역 (0x1100-0x115F)
        (0x1100..=0x115F).contains(&c)
    }

    fn is_hangul_jungseong(&self) -> bool {
        let c = *self as u32;
        // 첫가끝 중성 영역 (0x1160-0x11A7)
        (0x1160..=0x11A7).contains(&c)
    }

    fn is_hangul_jongseong(&self) -> bool {
        let c = *self as u32;
        // 첫가끝 종성 영역 (0x11A8-0x11FF)
        (0x11A8..=0x11FF).contains(&c)
    }

    fn is_hangul_jamo(&self) -> bool {
        // 첫가끝 자모 영역 전체 (0x1100-0x11FF)
        self.is_hangul_choseong() || self.is_hangul_jungseong() || self.is_hangul_jongseong()
    }

    fn is_hangul(&self) -> bool {
        self.is_hangul_syllable() || self.is_hangul_jamo() || self.is_hangul_compat_jamo()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_korean_char_creation() {
        let korean_char = HangulChar::from_syllable('한').unwrap();
        assert_eq!(korean_char.get_cho(), Some(Cho::Hieuh));
        assert_eq!(korean_char.get_jung(), Some(Jung::A));
        assert_eq!(korean_char.get_jong(), Some(Jong::Nieun));

        let korean_char_jamo = HangulChar::from_jamo_names("ㄱ", "ㅏ", "ㄴ").unwrap();
        assert_eq!(korean_char_jamo.get_cho(), Some(Cho::Giyeok));
        assert_eq!(korean_char_jamo.get_jung(), Some(Jung::A));
        assert_eq!(korean_char_jamo.get_jong(), Some(Jong::Nieun));
    }

    #[test]
    fn test_get_syllable() {
        let korean_char = HangulChar::from_jamo_names("ㅎ", "ㅏ", "ㄴ").unwrap();
        assert_eq!(korean_char.get_syllable().unwrap(), '한');

        let korean_char_no_jong = HangulChar::from_jamo_names("ㄱ", "ㅏ", "E").unwrap();
        assert_eq!(korean_char_no_jong.get_syllable().unwrap(), '가');

        let korean_char_only_cho = HangulChar::from_jamo_names("ㄱ", "F", "E").unwrap();
        assert_eq!(korean_char_only_cho.get_syllable().unwrap(), '\u{3131}'); // ㄱ 호환용 자모

        let korean_char_empty = HangulChar::new();
        assert!(korean_char_empty.get_syllable().is_err()); // 빈 상태는 Err 반환
    }

    #[test]
    fn test_to_string() {
        let korean_char = HangulChar::from_jamo_names("ㅎ", "ㅏ", "ㄴ").unwrap();
        assert_eq!(korean_char.to_string(), "한");

        let korean_char_only_cho = HangulChar::from_jamo_names("ㄱ", "F", "E").unwrap();
        assert_eq!(korean_char_only_cho.to_string(), "ㄱ"); // 호환용 자모로 표현
    }

    #[test]
    fn test_from_string() {
        let korean_char = "한".parse::<HangulChar>().unwrap();
        assert_eq!(korean_char.get_syllable().unwrap(), '한');

        let cho_char = "ㄱ".parse::<HangulChar>().unwrap();
        assert_eq!(cho_char.get_cho(), Some(Cho::G));
        assert_eq!(cho_char.get_syllable().unwrap(), '\u{3131}'); // ㄱ 호환용 자모

        let invalid_char_result = "abc".parse::<HangulChar>();
        assert!(invalid_char_result.is_err());
    }

    #[test]
    fn test_to_compat_jamo_string() {
        let korean_han = HangulChar::from_syllable('한').unwrap();
        assert_eq!(korean_han.to_compat_jamo_string(), "ㅎㅏㄴ");

        let korean_ga = HangulChar::from_syllable('가').unwrap();
        assert_eq!(korean_ga.to_compat_jamo_string(), "ㄱㅏ");

        let korean_gg = HangulChar::from_syllable('까').unwrap();
        assert_eq!(korean_gg.to_compat_jamo_string(), "ㄲㅏ");

        let korean_eobs = HangulChar::from_syllable('없').unwrap();
        assert_eq!(korean_eobs.to_compat_jamo_string(), "ㅇㅓㅄ");

        let mut korean_cho = HangulChar::new();
        korean_cho.set_cho_object(Some(Cho::Bieup));
        assert_eq!(korean_cho.to_compat_jamo_string(), "ㅂ");

        let mut korean_jung = HangulChar::new();
        korean_jung.set_jung_object(Some(Jung::Ya));
        assert_eq!(korean_jung.to_compat_jamo_string(), "ㅑ");

        let mut korean_jong = HangulChar::new();
        korean_jong.set_jong_object(Some(Jong::Rieul));
        assert_eq!(korean_jong.to_compat_jamo_string(), "ㄹ"); // Jong::Empty 가 아니므로 추가됨

        let korean_empty = HangulChar::new();
        assert_eq!(korean_empty.to_compat_jamo_string(), "");
    }
}
