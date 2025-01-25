/**
 * 한글 음절 조합 및 분해 (유니코드 5.2)
 *
 * @author "KiHyeon Seo" <from104@gmail.com>
 * @version 0.0.1
 */
// phoneme.rs 모듈을 가져옵니다. (phoneme.rs 파일이 같은 디렉토리에 있다고 가정)
use crate::hangul::jamo::*;
// 또는 use super::phoneme::*; // 모듈 구조에 따라

/**
 * 한글 문자 구조체
 */
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)] // 필요에 따라 trait 추가
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
pub const SYLLABLE_BASE: char = '\u{ac00}';
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
    pub fn from_jamo_sequences(cho: i32, jung: i32, jong: i32) -> Self {
        HangulChar {
            choseong: get_cho_by_sequence(cho),
            jungseong: get_jung_by_sequence(jung),
            jongseong: get_jong_by_sequence(jong),
        }
    }

    /**
     * 한글 생성자 (초종중성 객체 이름으로)
     * @param cho
     * @param jung
     * @param jong
     */
    pub fn from_jamo_names(cho: &str, jung: &str, jong: &str) -> Self {
        HangulChar {
            choseong: match Cho::from_str(cho.to_uppercase().as_str()) {
                Ok(c) => Some(c),
                Err(_) => None,
            },
            jungseong: match Jung::from_str(jung.to_uppercase().as_str()) {
                Ok(j) => Some(j),
                Err(_) => None,
            },
            jongseong: match Jong::from_str(jong.to_uppercase().as_str()) {
                Ok(jo) => Some(jo),
                Err(_) => None,
            },
        }
    }

    /**
     * 한글 생성자 (한글 음절로)
     * @param syllable
     */
    pub fn from_syllable(syllable: char) -> Self {
        let mut hangul_char = HangulChar::new();
        hangul_char.set_jamo_by_syllable(syllable);
        hangul_char
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
    pub fn set_jamo_sequences(&mut self, cho: i32, jung: i32, jong: i32) -> bool {
        self.set_cho_sequence(cho) & self.set_jung_sequence(jung) & self.set_jong_sequence(jong)
    }

    /**
     * 한글 자모 정하기 (초종중성 객체 이름으로)
     * @param cho
     * @param jung
     * @param jong
     */
    pub fn set_jamo_names(&mut self, cho: &str, jung: &str, jong: &str) -> bool {
        self.set_cho_name(cho) & self.set_jung_name(jung) & self.set_jong_name(jong)
    }

    /**
     * 한글 자모 정하기 (한글 음절로)
     * @param syllable
     */
    pub fn set_jamo_by_syllable(&mut self, syllable: char) {
        if syllable >= SYLLABLE_BASE
            && syllable < char::from_u32(SYLLABLE_BASE as u32 + SYLLABLE_NUMBER as u32).unwrap()
        {
            let syll_index = (syllable as u32 - SYLLABLE_BASE as u32) as usize;

            let jong_index = syll_index % JONGSEONG_NUMBER;
            let jung_index = (syll_index / JONGSEONG_NUMBER) % JUNGSEONG_NUMBER;
            let cho_index = syll_index / (JUNGSEONG_NUMBER * JONGSEONG_NUMBER);

            self.set_cho_sequence(cho_index as i32);
            self.set_jung_sequence(jung_index as i32);
            self.set_jong_sequence(jong_index as i32);
        } else {
            panic!("한글 코드를 벗어났음"); // 또는 Result를 사용하여 에러 처리
        }
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
    pub fn set_cho_sequence(&mut self, cho: i32) -> bool {
        if cho >= 0 && cho < CHOSEONG_NUMBER as i32 {
            self.set_cho_object(get_cho_by_sequence(cho));
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
    pub fn set_jung_sequence(&mut self, jung: i32) -> bool {
        if jung >= 0 && jung < JUNGSEONG_NUMBER as i32 {
            self.set_jung_object(get_jung_by_sequence(jung));
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
    pub fn set_jong_sequence(&mut self, jong: i32) -> bool {
        if jong >= 0 && jong < JONGSEONG_NUMBER as i32 {
            // 종성 갯수 확인 필요 - 원래 JONGSEONG_NUMBER 가 맞는지
            self.set_jong_object(get_jong_by_sequence(jong));
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
    pub fn set_cho_name(&mut self, cho: &str) -> bool {
        match Cho::from_str(cho.to_uppercase().as_str()) {
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
    pub fn set_jung_name(&mut self, jung: &str) -> bool {
        match Jung::from_str(jung.to_uppercase().as_str()) {
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
    pub fn set_jong_name(&mut self, jong: &str) -> bool {
        match Jong::from_str(jong.to_uppercase().as_str()) {
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
    pub fn get_unicodes(&self) -> Vec<char> {
        if self.is_empty() {
            panic!("자모가 비었음");
        } else if self.is_filled_cho() && !self.is_filled_jung() && self.is_filled_jong() {
            vec!['?']
        } else if !self.is_filled_jong() || self.jongseong == Some(Jong::E) {
            // 종성이 없으면
            vec![
                self.choseong
                    .map_or(Cho::F.get_unicode(), |c| c.get_unicode()),
                self.jungseong
                    .map_or(Jung::F.get_unicode(), |j| j.get_unicode()),
            ]
        } else {
            // 종성이 있으면
            vec![
                self.choseong
                    .map_or(Cho::F.get_unicode(), |c| c.get_unicode()),
                self.jungseong
                    .map_or(Jung::F.get_unicode(), |j| j.get_unicode()),
                self.jongseong.map_or('\u{0}', |jo| jo.get_unicode()),
            ]
        }
    }

    /**
     * 한글 음절 얻기
     * @return 한글 음절 또는 호환용 자모
     */
    pub fn get_syllable(&self) -> char {
        if self.is_empty() {
            panic!("자모가 비었음");
        } else if self.is_filled_only_cho() {
            self.get_cho_unicode_compat()
        } else if self.is_filled_only_jung() {
            self.get_jung_unicode_compat()
        } else if self.is_filled_only_jong() {
            self.get_jong_unicode_compat()
        } else if self.is_filled_cho() && self.is_filled_jung() {
            let choseong_seq = self.choseong.unwrap().get_sequence() as usize;
            let jungseong_seq = self.jungseong.unwrap().get_sequence() as usize;
            let jongseong_seq = self.jongseong.map_or(0, |jo| jo.get_sequence() as usize);

            let syllable_index = SYLLABLE_BASE as u32
                + ((choseong_seq * JUNGSEONG_NUMBER * JONGSEONG_NUMBER
                    + jungseong_seq * JONGSEONG_NUMBER
                    + jongseong_seq) as u32);

            char::from_u32(syllable_index).unwrap_or('?')
        } else {
            '?'
        }
    }
}

use std::fmt;
use std::str::FromStr;

impl fmt::Display for HangulChar {
    /**
     * 조합된 한글을 문자열 변환
     * @return 한글 음절 또는 첫가끝 코드 문자열
     */
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let h = self.get_syllable();
        if h != '?' {
            write!(f, "{}", h)
        } else {
            let unicodes = self.get_unicodes();
            let s: String = unicodes.iter().collect(); // Vec<char> to String
            write!(f, "{}", s) // 첫가끝 유니코드 문자열 표현 방식이 명확하지 않아 일단 이렇게 처리
        }
    }
}

impl FromStr for HangulChar {
    type Err = String; // or a more specific error type

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if s.len() == 1 {
            let c = s.chars().next().unwrap();
            if (SYLLABLE_BASE
                ..=char::from_u32(SYLLABLE_BASE as u32 + SYLLABLE_NUMBER as u32).unwrap())
                .contains(&c)
            {
                return Ok(HangulChar::from_syllable(c));
            } else if let Ok(cho) = Cho::from_str(s.to_uppercase().as_str()) {
                return Ok(HangulChar::from_jamo_objects(Some(cho), None, None));
            } else if let Ok(jung) = Jung::from_str(s.to_uppercase().as_str()) {
                return Ok(HangulChar::from_jamo_objects(None, Some(jung), None));
            } else if let Ok(jong) = Jong::from_str(s.to_uppercase().as_str()) {
                return Ok(HangulChar::from_jamo_objects(None, None, Some(jong)));
            }
        }
        Err(String::from(
            "문자열 파싱 실패: 한글 음절, 초성, 중성, 종성 중 하나의 형태여야 합니다.",
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hangul_char_creation() {
        let hangul_char = HangulChar::from_syllable('한');
        assert_eq!(hangul_char.get_cho(), Some(Cho::H));
        assert_eq!(hangul_char.get_jung(), Some(Jung::A));
        assert_eq!(hangul_char.get_jong(), Some(Jong::N));

        let hangul_char_jamo = HangulChar::from_jamo_names("ㄱ", "ㅏ", "ㄴ");
        assert_eq!(hangul_char_jamo.get_cho(), Some(Cho::G));
        assert_eq!(hangul_char_jamo.get_jung(), Some(Jung::A));
        assert_eq!(hangul_char_jamo.get_jong(), Some(Jong::N));
    }

    #[test]
    fn test_get_syllable() {
        let hangul_char = HangulChar::from_jamo_names("ㅎ", "ㅏ", "ㄴ");
        assert_eq!(hangul_char.get_syllable(), '한');

        let hangul_char_no_jong = HangulChar::from_jamo_names("ㄱ", "ㅏ", "E");
        assert_eq!(hangul_char_no_jong.get_syllable(), '가');

        let hangul_char_only_cho = HangulChar::from_jamo_names("ㄱ", "F", "E");
        assert_eq!(hangul_char_only_cho.get_syllable(), '\u{3131}'); // ㄱ 호환용 자모

        let hangul_char_empty = HangulChar::new();
        assert!(std::panic::catch_unwind(|| hangul_char_empty.get_syllable()).is_err());
        // 패닉 발생
    }

    #[test]
    fn test_to_string() {
        let hangul_char = HangulChar::from_jamo_names("ㅎ", "ㅏ", "ㄴ");
        assert_eq!(hangul_char.to_string(), "한");

        let hangul_char_only_cho = HangulChar::from_jamo_names("ㄱ", "F", "E");
        assert_eq!(hangul_char_only_cho.to_string(), "ㄱ"); // 호환용 자모로 표현
    }

    #[test]
    fn test_from_string() {
        let hangul_char = "한".parse::<HangulChar>().unwrap();
        assert_eq!(hangul_char.get_syllable(), '한');

        let cho_char = "ㄱ".parse::<HangulChar>().unwrap();
        assert_eq!(cho_char.get_cho(), Some(Cho::G));
        assert_eq!(cho_char.get_syllable(), '\u{3131}'); // ㄱ 호환용 자모

        let invalid_char_result = "abc".parse::<HangulChar>();
        assert!(invalid_char_result.is_err());
    }
}
