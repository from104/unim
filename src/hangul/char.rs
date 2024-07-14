// hangul_char.rs

use crate::hangul::jamo::{Cho, Jung, Jong, Jamo, get_cho_by_sequence, get_jung_by_sequence, get_jong_by_sequence};

pub struct HangulChar {
    choseong: Option<Cho>,
    jungseong: Option<Jung>,
    jongseong: Option<Jong>,
}

impl Default for HangulChar {
    fn default() -> Self {
        HangulChar::new()
    }
}

impl HangulChar {
    // 초성, 중성, 종성 갯수
    const CHOSEONG_NUMBER: i32 = 19;
    const JUNGSEONG_NUMBER: i32 = 21;
     const JONGSEONG_NUMBER: i32 = 28;
    
    // 한글 음절 유니코드 시작 코드
    const SYLLABLE_BASE: i32 = '\u{AC00}' as i32;
    // 한글 음절 갯수
    const SYLLABLE_NUMBER: i32 = HangulChar::CHOSEONG_NUMBER * HangulChar::JUNGSEONG_NUMBER * HangulChar::JONGSEONG_NUMBER;

    pub fn new() -> Self {
        HangulChar {
            choseong: None,
            jungseong: None,
            jongseong: None,
        }
    }

    pub fn from_jamo(cho: Cho, jung: Jung, jong: Jong) -> Self {
        let mut hangul_char = HangulChar::new();
        hangul_char.set_jamo(cho, jung, jong);
        hangul_char
    }

    pub fn from_indices(cho: i32, jung: i32, jong: i32) -> Self {
        let mut hangul_char = HangulChar::new();
        hangul_char.set_jamo_by_indices(cho, jung, jong);
        hangul_char
    }

    pub fn from_names(cho: &str, jung: &str, jong: &str) -> Self {
        let mut hangul_char = HangulChar::new();
        hangul_char.set_jamo_by_names(cho, jung, jong);
        hangul_char
    }

    pub fn from_syllable(syllable: char) -> Self {
        let mut hangul_char = HangulChar::new();
        hangul_char.set_jamo_by_syllable(syllable);
        hangul_char
    }

    pub fn set_jamo(&mut self, cho: Cho, jung: Jung, jong: Jong) -> bool {
        let (a, b, c) = (self.set_cho(Some(cho)), self.set_jung(Some(jung)), self.set_jong(Some(jong)));
        a && b && c
    }

    pub fn set_jamo_by_indices(&mut self, cho: i32, jung: i32, jong: i32) -> bool {
        let (a, b, c) = (self.set_cho_by_index(cho), self.set_jung_by_index(jung), self.set_jong_by_index(jong));
        a && b && c
    }

    pub fn set_jamo_by_names(&mut self, cho: &str, jung: &str, jong: &str) -> bool {
         let (a, b, c) = (self.set_cho_by_name(cho), self.set_jung_by_name(jung), self.set_jong_by_name(jong));
        a && b && c
    }

    pub fn set_jamo_by_syllable(&mut self, syllable: char) {
        let syllable_code = syllable as i32;
        if (HangulChar::SYLLABLE_BASE..(HangulChar::SYLLABLE_BASE + HangulChar::SYLLABLE_NUMBER)).contains(&syllable_code) {
            let syll = syllable_code - HangulChar::SYLLABLE_BASE;

            let jong = syll % HangulChar::JONGSEONG_NUMBER;
            let jung = (syll / HangulChar::JONGSEONG_NUMBER) % HangulChar::JUNGSEONG_NUMBER;
            let cho = syll / (HangulChar::JUNGSEONG_NUMBER * HangulChar::JONGSEONG_NUMBER);

            self.set_cho_by_index(cho);
            self.set_jung_by_index(jung);
            self.set_jong_by_index(jong);
        } else {
            panic!("Invalid Hangul syllable code");
        }
    }

    pub fn set_cho(&mut self, cho: Option<Cho>) -> bool {
        if let Some(cho) = cho {
            self.choseong = Some(cho);
            true
        } else {
            self.clear_cho();
            false
        }
    }

    pub fn set_jung(&mut self, jung: Option<Jung>) -> bool {
        if let Some(jung) = jung {
            self.jungseong = Some(jung);
            true
        } else {
            self.clear_jung();
            false
        }
    }

    pub fn set_jong(&mut self, jong: Option<Jong>) -> bool {
        if let Some(jong) = jong {
            self.jongseong = Some(jong);
            true
        } else {
            self.clear_jong();
            false
        }
    }

    pub fn set_cho_by_index(&mut self, cho: i32) -> bool {
        if (0..HangulChar::CHOSEONG_NUMBER).contains(&cho) {
            self.set_cho(get_cho_by_sequence(cho))
        } else {
            self.clear_cho();
            false
        }
    }

    pub fn set_jung_by_index(&mut self, jung: i32) -> bool {
        if (0..HangulChar::JUNGSEONG_NUMBER).contains(&jung) {
            self.set_jung(get_jung_by_sequence(jung))
        } else {
            self.clear_jung();
            false
        }
    }

    pub fn set_jong_by_index(&mut self, jong: i32) -> bool {
        if (0..HangulChar::JONGSEONG_NUMBER).contains(&jong) {
            self.set_jong(get_jong_by_sequence(jong))
        } else {
            self.clear_jong();
            false
        }
    }
    pub fn set_cho_by_name(&mut self, cho: &str) -> bool {
        if let Ok(cho_enum) = cho.parse::<Cho>() {
            self.set_cho(Some(cho_enum))
        } else {
            self.clear_cho();
            false
        }
    }

    pub fn set_jung_by_name(&mut self, jung: &str) -> bool {
        if let Ok(jung_enum) = jung.parse::<Jung>() {
            self.set_jung(Some(jung_enum))
        } else {
            self.clear_jung();
            false
        }
    }

    pub fn set_jong_by_name(&mut self, jong: &str) -> bool {
        if let Ok(jong_enum) = jong.parse::<Jong>() {
            self.set_jong(Some(jong_enum))
        } else {
            self.clear_jong();
            false
        }
    }

    pub fn clear_cho(&mut self) {
        self.choseong = None;
    }

    pub fn clear_jung(&mut self) {
        self.jungseong = None;
    }

    pub fn clear_jong(&mut self) {
        self.jongseong = None;
    }

    pub fn clear(&mut self) {
        self.clear_cho();
        self.clear_jung();
        self.clear_jong();
    }

    pub fn is_filled_cho(&self) -> bool {
        self.choseong.is_some()
    }

    pub fn is_filled_jung(&self) -> bool {
        self.jungseong.is_some()
    }

    pub fn is_filled_jong(&self) -> bool {
        self.jongseong.is_some()
    }

    pub fn is_filled_only_cho(&self) -> bool {
        self.is_filled_cho() && !self.is_filled_jung() && !self.is_filled_jong()
    }

    pub fn is_filled_only_jung(&self) -> bool {
        !self.is_filled_cho() && self.is_filled_jung() && !self.is_filled_jong()
    }

    pub fn is_filled_only_jong(&self) -> bool {
        !self.is_filled_cho() && !self.is_filled_jung() && self.is_filled_jong()
    }

    pub fn is_empty(&self) -> bool {
        !self.is_filled_cho() && !self.is_filled_jung() && !self.is_filled_jong()
    }

    pub fn get_cho(&self) -> Option<Cho> {
        self.choseong
    }

    pub fn get_jung(&self) -> Option<Jung> {
        self.jungseong
    }

    pub fn get_jong(&self) -> Option<Jong> {
        self.jongseong
    }

    pub fn get_cho_unicode(&self) -> char {
        if let Some(cho) = self.choseong {
            cho.get_unicode()
        } else {
            '\0'
        }
    }

    pub fn get_jung_unicode(&self) -> char {
        if let Some(jung) = self.jungseong {
            jung.get_unicode()
        } else {
            '\0'
        }
    }

    pub fn get_jong_unicode(&self) -> char {
        if let Some(jong) = self.jongseong {
            jong.get_unicode()
        } else {
            '\0'
        }
    }

    pub fn get_cho_unicode_compat(&self) -> char {
        if let Some(cho) = self.choseong {
            cho.get_unicode_compat()
        } else {
            '\0'
        }
    }

    pub fn get_jung_unicode_compat(&self) -> char {
        if let Some(jung) = self.jungseong {
            jung.get_unicode_compat()
        } else {
            '\0'
        }
    }

    pub fn get_jong_unicode_compat(&self) -> char {
        if let Some(jong) = self.jongseong {
            jong.get_unicode_compat()
        } else {
            '\0'
        }
    }

    pub fn get_unicodes(&self) -> Vec<char> {
        if self.is_empty() {
            vec![' '] // 공백
        } else if self.is_filled_cho() && !self.is_filled_jung() && self.is_filled_jong() {
            vec!['?']
        } else if !self.is_filled_jong() {
            let mut c = vec!['\0'; 2];
            c[0] = self.get_cho_unicode();
            c[1] = self.get_jung_unicode();
            c
        } else {
            let mut c = vec!['\0'; 3];
            c[0] = self.get_cho_unicode();
            c[1] = self.get_jung_unicode();
            c[2] = self.get_jong_unicode();
            c
        }
    }

    pub fn get_syllable(&self) -> char {
        if self.is_empty() {
            ' '
        } else if self.is_filled_only_cho() {
            self.get_cho_unicode_compat()
        } else if self.is_filled_only_jung() {
            self.get_jung_unicode_compat()
        } else if self.is_filled_only_jong() {
            self.get_jong_unicode_compat()
        } else if self.is_filled_cho() && self.is_filled_jung() {
            let choseong = self.choseong.unwrap().get_sequence();
            let jungseong = self.jungseong.unwrap().get_sequence();
            let jongseong = if self.is_filled_jong() {
                self.jongseong.unwrap().get_sequence()
            } else {
                0
            };
            char::from_u32((HangulChar::SYLLABLE_BASE
                + choseong * HangulChar::JUNGSEONG_NUMBER * HangulChar::JONGSEONG_NUMBER
                + jungseong * HangulChar::JONGSEONG_NUMBER
                + jongseong) as u32).unwrap()
        } else {
            '?'
        }
    }

    pub fn to_string(&self) -> String {
        let syllable = self.get_syllable();
        if syllable != '?' {
            syllable.to_string()
        } else {
            self.get_unicodes().iter().collect()
        }
    }
}
