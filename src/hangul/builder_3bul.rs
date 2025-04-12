use crate::hangul::builder::BaseHangulBuilder;
use crate::hangul::builder::HangulBuilder;
use crate::hangul::char::HangulChar;
use crate::hangul::jamo::*;
use serde_json::Value;
use std::collections::{HashMap, VecDeque};
use std::fs::File;
use std::io::Read;

/**
 * 3벌식 한글 조합기
 */
#[derive(Debug, Default)]
pub struct HangulBuilder3Bul {
    base_builder: BaseHangulBuilder,
    en_keymap_file: String,
    ko_keymap_file: String,
}

impl HangulBuilder3Bul {
    pub fn new(en_keymap_file: &str, ko_keymap_file: &str) -> Self {
        let mut builder = HangulBuilder3Bul {
            base_builder: BaseHangulBuilder::new(),
            en_keymap_file: en_keymap_file.to_string(),
            ko_keymap_file: ko_keymap_file.to_string(),
        };
        builder.initialize_combined_jamo();
        builder
    }

    /// 3벌식 키보드 매핑을 생성합니다.
    pub fn create_keyboard_map(
        &self,
        en_keymap_file: &str,
        ko_keymap_file: &str,
    ) -> HashMap<char, JamoEnum> {
        let mut keyboard_map = HashMap::new();

        // 영문 키맵 로드
        let mut file = File::open(en_keymap_file).expect("Failed to open English keymap file");
        let mut contents = String::new();
        file.read_to_string(&mut contents)
            .expect("Failed to read English keymap file");
        let en_keymap: Value =
            serde_json::from_str(&contents).expect("Failed to parse English keymap JSON");

        // 한글 키맵 로드
        let mut file = File::open(ko_keymap_file).expect("Failed to open Korean keymap file");
        let mut contents = String::new();
        file.read_to_string(&mut contents)
            .expect("Failed to read Korean keymap file");
        let ko_keymap: Value =
            serde_json::from_str(&contents).expect("Failed to parse Korean keymap JSON");

        // 영문-한글 매핑 생성
        let rows = ["1st", "2nd", "3nd", "4th"];

        // 먼저 lower 케이스만 처리
        for row in rows.iter() {
            if let (Some(en_row), Some(ko_row)) = (
                en_keymap["layout"]["lower"][row].as_array(),
                ko_keymap["layout"]["lower"][row].as_array(),
            ) {
                for (en_char, ko_char) in en_row.iter().zip(ko_row.iter()) {
                    if let (Some(en), Some(ko)) = (en_char.as_str(), ko_char.as_str()) {
                        if let Some(c) = en.chars().next() {
                            // 한글 자모를 JamoEnum으로 변환
                            let jamo = match ko.chars().next() {
                                Some('ㄱ') => Some(JamoEnum::Cho(Cho::G)),
                                Some('ㄲ') => Some(JamoEnum::Cho(Cho::GG)),
                                Some('ㄴ') => Some(JamoEnum::Cho(Cho::N)),
                                Some('ㄷ') => Some(JamoEnum::Cho(Cho::D)),
                                Some('ㄸ') => Some(JamoEnum::Cho(Cho::DD)),
                                Some('ㄹ') => Some(JamoEnum::Cho(Cho::R)),
                                Some('ㅁ') => Some(JamoEnum::Cho(Cho::M)),
                                Some('ㅂ') => Some(JamoEnum::Cho(Cho::B)),
                                Some('ㅃ') => Some(JamoEnum::Cho(Cho::BB)),
                                Some('ㅅ') => Some(JamoEnum::Cho(Cho::S)),
                                Some('ㅆ') => Some(JamoEnum::Cho(Cho::SS)),
                                Some('ㅇ') => Some(JamoEnum::Cho(Cho::E)),
                                Some('ㅈ') => Some(JamoEnum::Cho(Cho::J)),
                                Some('ㅉ') => Some(JamoEnum::Cho(Cho::JJ)),
                                Some('ㅊ') => Some(JamoEnum::Cho(Cho::C)),
                                Some('ㅋ') => Some(JamoEnum::Cho(Cho::K)),
                                Some('ㅌ') => Some(JamoEnum::Cho(Cho::T)),
                                Some('ㅍ') => Some(JamoEnum::Cho(Cho::P)),
                                Some('ㅎ') => Some(JamoEnum::Cho(Cho::H)),
                                Some('ㅏ') => Some(JamoEnum::Jung(Jung::A)),
                                Some('ㅐ') => Some(JamoEnum::Jung(Jung::AE)),
                                Some('ㅑ') => Some(JamoEnum::Jung(Jung::YA)),
                                Some('ㅒ') => Some(JamoEnum::Jung(Jung::YAE)),
                                Some('ㅓ') => Some(JamoEnum::Jung(Jung::EO)),
                                Some('ㅔ') => Some(JamoEnum::Jung(Jung::E)),
                                Some('ㅕ') => Some(JamoEnum::Jung(Jung::YEO)),
                                Some('ㅖ') => Some(JamoEnum::Jung(Jung::YE)),
                                Some('ㅗ') => Some(JamoEnum::Jung(Jung::O)),
                                Some('ㅘ') => Some(JamoEnum::Jung(Jung::WA)),
                                Some('ㅙ') => Some(JamoEnum::Jung(Jung::WAE)),
                                Some('ㅚ') => Some(JamoEnum::Jung(Jung::OE)),
                                Some('ㅛ') => Some(JamoEnum::Jung(Jung::YO)),
                                Some('ㅜ') => Some(JamoEnum::Jung(Jung::U)),
                                Some('ㅝ') => Some(JamoEnum::Jung(Jung::WEO)),
                                Some('ㅞ') => Some(JamoEnum::Jung(Jung::WE)),
                                Some('ㅟ') => Some(JamoEnum::Jung(Jung::WI)),
                                Some('ㅠ') => Some(JamoEnum::Jung(Jung::YU)),
                                Some('ㅡ') => Some(JamoEnum::Jung(Jung::EU)),
                                Some('ㅢ') => Some(JamoEnum::Jung(Jung::YI)),
                                Some('ㅣ') => Some(JamoEnum::Jung(Jung::I)),
                                Some('ᆨ') => Some(JamoEnum::Jong(Jong::G)),
                                Some('ᆩ') => Some(JamoEnum::Jong(Jong::GG)),
                                Some('ᆪ') => Some(JamoEnum::Jong(Jong::GS)),
                                Some('ᆫ') => Some(JamoEnum::Jong(Jong::N)),
                                Some('ᆬ') => Some(JamoEnum::Jong(Jong::NJ)),
                                Some('ᆭ') => Some(JamoEnum::Jong(Jong::NH)),
                                Some('ᆮ') => Some(JamoEnum::Jong(Jong::D)),
                                Some('ᆯ') => Some(JamoEnum::Jong(Jong::L)),
                                Some('ᆰ') => Some(JamoEnum::Jong(Jong::LG)),
                                Some('ᆱ') => Some(JamoEnum::Jong(Jong::LM)),
                                Some('ᆲ') => Some(JamoEnum::Jong(Jong::LB)),
                                Some('ᆳ') => Some(JamoEnum::Jong(Jong::LS)),
                                Some('ᆴ') => Some(JamoEnum::Jong(Jong::LT)),
                                Some('ᆵ') => Some(JamoEnum::Jong(Jong::LP)),
                                Some('ᆶ') => Some(JamoEnum::Jong(Jong::LH)),
                                Some('ᆷ') => Some(JamoEnum::Jong(Jong::M)),
                                Some('ᆸ') => Some(JamoEnum::Jong(Jong::B)),
                                Some('ᆹ') => Some(JamoEnum::Jong(Jong::BS)),
                                Some('ᆺ') => Some(JamoEnum::Jong(Jong::S)),
                                Some('ᆻ') => Some(JamoEnum::Jong(Jong::SS)),
                                Some('ᆼ') => Some(JamoEnum::Jong(Jong::NG)),
                                Some('ᆽ') => Some(JamoEnum::Jong(Jong::J)),
                                Some('ᆾ') => Some(JamoEnum::Jong(Jong::C)),
                                Some('ᆿ') => Some(JamoEnum::Jong(Jong::K)),
                                Some('ᇀ') => Some(JamoEnum::Jong(Jong::T)),
                                Some('ᇁ') => Some(JamoEnum::Jong(Jong::P)),
                                Some('ᇂ') => Some(JamoEnum::Jong(Jong::H)),
                                Some(c) => Some(JamoEnum::Special(c)), // 자모가 아닌 문자도 매핑
                                None => None,
                            };
                            if let Some(jamo) = jamo {
                                keyboard_map.insert(c, jamo);
                            }
                        }
                    }
                }
            }
        }

        // 그 다음 upper 케이스 처리 (lower에 없는 경우만)
        for row in rows.iter() {
            if let (Some(en_row), Some(ko_row)) = (
                en_keymap["layout"]["upper"][row].as_array(),
                ko_keymap["layout"]["upper"][row].as_array(),
            ) {
                for (en_char, ko_char) in en_row.iter().zip(ko_row.iter()) {
                    if let (Some(en), Some(ko)) = (en_char.as_str(), ko_char.as_str()) {
                        if let Some(c) = en.chars().next() {
                            // 이미 매핑된 키는 건너뛰기
                            if keyboard_map.contains_key(&c) {
                                continue;
                            }

                            // 한글 자모를 JamoEnum으로 변환
                            let jamo = match ko.chars().next() {
                                Some('ㄱ') => Some(JamoEnum::Cho(Cho::G)),
                                Some('ㄲ') => Some(JamoEnum::Cho(Cho::GG)),
                                Some('ㄴ') => Some(JamoEnum::Cho(Cho::N)),
                                Some('ㄷ') => Some(JamoEnum::Cho(Cho::D)),
                                Some('ㄸ') => Some(JamoEnum::Cho(Cho::DD)),
                                Some('ㄹ') => Some(JamoEnum::Cho(Cho::R)),
                                Some('ㅁ') => Some(JamoEnum::Cho(Cho::M)),
                                Some('ㅂ') => Some(JamoEnum::Cho(Cho::B)),
                                Some('ㅃ') => Some(JamoEnum::Cho(Cho::BB)),
                                Some('ㅅ') => Some(JamoEnum::Cho(Cho::S)),
                                Some('ㅆ') => Some(JamoEnum::Cho(Cho::SS)),
                                Some('ㅇ') => Some(JamoEnum::Cho(Cho::E)),
                                Some('ㅈ') => Some(JamoEnum::Cho(Cho::J)),
                                Some('ㅉ') => Some(JamoEnum::Cho(Cho::JJ)),
                                Some('ㅊ') => Some(JamoEnum::Cho(Cho::C)),
                                Some('ㅋ') => Some(JamoEnum::Cho(Cho::K)),
                                Some('ㅌ') => Some(JamoEnum::Cho(Cho::T)),
                                Some('ㅍ') => Some(JamoEnum::Cho(Cho::P)),
                                Some('ㅎ') => Some(JamoEnum::Cho(Cho::H)),
                                Some('ㅏ') => Some(JamoEnum::Jung(Jung::A)),
                                Some('ㅐ') => Some(JamoEnum::Jung(Jung::AE)),
                                Some('ㅑ') => Some(JamoEnum::Jung(Jung::YA)),
                                Some('ㅒ') => Some(JamoEnum::Jung(Jung::YAE)),
                                Some('ㅓ') => Some(JamoEnum::Jung(Jung::EO)),
                                Some('ㅔ') => Some(JamoEnum::Jung(Jung::E)),
                                Some('ㅕ') => Some(JamoEnum::Jung(Jung::YEO)),
                                Some('ㅖ') => Some(JamoEnum::Jung(Jung::YE)),
                                Some('ㅗ') => Some(JamoEnum::Jung(Jung::O)),
                                Some('ㅘ') => Some(JamoEnum::Jung(Jung::WA)),
                                Some('ㅙ') => Some(JamoEnum::Jung(Jung::WAE)),
                                Some('ㅚ') => Some(JamoEnum::Jung(Jung::OE)),
                                Some('ㅛ') => Some(JamoEnum::Jung(Jung::YO)),
                                Some('ㅜ') => Some(JamoEnum::Jung(Jung::U)),
                                Some('ㅝ') => Some(JamoEnum::Jung(Jung::WEO)),
                                Some('ㅞ') => Some(JamoEnum::Jung(Jung::WE)),
                                Some('ㅟ') => Some(JamoEnum::Jung(Jung::WI)),
                                Some('ㅠ') => Some(JamoEnum::Jung(Jung::YU)),
                                Some('ㅡ') => Some(JamoEnum::Jung(Jung::EU)),
                                Some('ㅢ') => Some(JamoEnum::Jung(Jung::YI)),
                                Some('ㅣ') => Some(JamoEnum::Jung(Jung::I)),
                                Some('ᆨ') => Some(JamoEnum::Jong(Jong::G)),
                                Some('ᆩ') => Some(JamoEnum::Jong(Jong::GG)),
                                Some('ᆪ') => Some(JamoEnum::Jong(Jong::GS)),
                                Some('ᆫ') => Some(JamoEnum::Jong(Jong::N)),
                                Some('ᆬ') => Some(JamoEnum::Jong(Jong::NJ)),
                                Some('ᆭ') => Some(JamoEnum::Jong(Jong::NH)),
                                Some('ᆮ') => Some(JamoEnum::Jong(Jong::D)),
                                Some('ᆯ') => Some(JamoEnum::Jong(Jong::L)),
                                Some('ᆰ') => Some(JamoEnum::Jong(Jong::LG)),
                                Some('ᆱ') => Some(JamoEnum::Jong(Jong::LM)),
                                Some('ᆲ') => Some(JamoEnum::Jong(Jong::LB)),
                                Some('ᆳ') => Some(JamoEnum::Jong(Jong::LS)),
                                Some('ᆴ') => Some(JamoEnum::Jong(Jong::LT)),
                                Some('ᆵ') => Some(JamoEnum::Jong(Jong::LP)),
                                Some('ᆶ') => Some(JamoEnum::Jong(Jong::LH)),
                                Some('ᆷ') => Some(JamoEnum::Jong(Jong::M)),
                                Some('ᆸ') => Some(JamoEnum::Jong(Jong::B)),
                                Some('ᆹ') => Some(JamoEnum::Jong(Jong::BS)),
                                Some('ᆺ') => Some(JamoEnum::Jong(Jong::S)),
                                Some('ᆻ') => Some(JamoEnum::Jong(Jong::SS)),
                                Some('ᆼ') => Some(JamoEnum::Jong(Jong::NG)),
                                Some('ᆽ') => Some(JamoEnum::Jong(Jong::J)),
                                Some('ᆾ') => Some(JamoEnum::Jong(Jong::C)),
                                Some('ᆿ') => Some(JamoEnum::Jong(Jong::K)),
                                Some('ᇀ') => Some(JamoEnum::Jong(Jong::T)),
                                Some('ᇁ') => Some(JamoEnum::Jong(Jong::P)),
                                Some('ᇂ') => Some(JamoEnum::Jong(Jong::H)),
                                Some(c) => Some(JamoEnum::Special(c)), // 자모가 아닌 문자도 매핑
                                None => None,
                            };
                            if let Some(jamo) = jamo {
                                keyboard_map.insert(c, jamo);
                            }
                        }
                    }
                }
            }
        }

        keyboard_map
    }

    fn initialize_combined_jamo(&mut self) {
        let mut combined_jamo = HashMap::new();

        // 초성 조합 규칙
        let mut g_map = HashMap::new();
        g_map.insert(JamoEnum::Cho(Cho::G), JamoEnum::Cho(Cho::GG));
        combined_jamo.insert(JamoEnum::Cho(Cho::G), g_map);

        let mut d_map = HashMap::new();
        d_map.insert(JamoEnum::Cho(Cho::D), JamoEnum::Cho(Cho::DD));
        combined_jamo.insert(JamoEnum::Cho(Cho::D), d_map);

        let mut b_map = HashMap::new();
        b_map.insert(JamoEnum::Cho(Cho::B), JamoEnum::Cho(Cho::BB));
        combined_jamo.insert(JamoEnum::Cho(Cho::B), b_map);

        let mut s_map = HashMap::new();
        s_map.insert(JamoEnum::Cho(Cho::S), JamoEnum::Cho(Cho::SS));
        combined_jamo.insert(JamoEnum::Cho(Cho::S), s_map);

        let mut j_map = HashMap::new();
        j_map.insert(JamoEnum::Cho(Cho::J), JamoEnum::Cho(Cho::JJ));
        combined_jamo.insert(JamoEnum::Cho(Cho::J), j_map);

        // 중성 조합 규칙
        let mut o_map = HashMap::new();
        o_map.insert(JamoEnum::Jung(Jung::A), JamoEnum::Jung(Jung::WA));
        o_map.insert(JamoEnum::Jung(Jung::AE), JamoEnum::Jung(Jung::WAE));
        o_map.insert(JamoEnum::Jung(Jung::I), JamoEnum::Jung(Jung::OE));
        combined_jamo.insert(JamoEnum::Jung(Jung::O), o_map);

        let mut u_map = HashMap::new();
        u_map.insert(JamoEnum::Jung(Jung::EO), JamoEnum::Jung(Jung::WEO));
        u_map.insert(JamoEnum::Jung(Jung::E), JamoEnum::Jung(Jung::WE));
        u_map.insert(JamoEnum::Jung(Jung::I), JamoEnum::Jung(Jung::WI));
        combined_jamo.insert(JamoEnum::Jung(Jung::U), u_map);

        let mut eu_map = HashMap::new();
        eu_map.insert(JamoEnum::Jung(Jung::I), JamoEnum::Jung(Jung::YI));
        combined_jamo.insert(JamoEnum::Jung(Jung::EU), eu_map);

        // 종성 조합 규칙
        let mut jong_g_map = HashMap::new();
        jong_g_map.insert(JamoEnum::Jong(Jong::G), JamoEnum::Jong(Jong::GG));
        jong_g_map.insert(JamoEnum::Jong(Jong::S), JamoEnum::Jong(Jong::GS));
        combined_jamo.insert(JamoEnum::Jong(Jong::G), jong_g_map);

        let mut n_map = HashMap::new();
        n_map.insert(JamoEnum::Jong(Jong::J), JamoEnum::Jong(Jong::NJ));
        n_map.insert(JamoEnum::Jong(Jong::H), JamoEnum::Jong(Jong::NH));
        combined_jamo.insert(JamoEnum::Jong(Jong::N), n_map);

        let mut l_map = HashMap::new();
        l_map.insert(JamoEnum::Jong(Jong::G), JamoEnum::Jong(Jong::LG));
        l_map.insert(JamoEnum::Jong(Jong::M), JamoEnum::Jong(Jong::LM));
        l_map.insert(JamoEnum::Jong(Jong::B), JamoEnum::Jong(Jong::LB));
        l_map.insert(JamoEnum::Jong(Jong::S), JamoEnum::Jong(Jong::LS));
        l_map.insert(JamoEnum::Jong(Jong::T), JamoEnum::Jong(Jong::LT));
        l_map.insert(JamoEnum::Jong(Jong::P), JamoEnum::Jong(Jong::LP));
        l_map.insert(JamoEnum::Jong(Jong::H), JamoEnum::Jong(Jong::LH));
        combined_jamo.insert(JamoEnum::Jong(Jong::L), l_map);

        let mut jong_b_map = HashMap::new();
        jong_b_map.insert(JamoEnum::Jong(Jong::S), JamoEnum::Jong(Jong::BS));
        combined_jamo.insert(JamoEnum::Jong(Jong::B), jong_b_map);

        let mut s_jong_map = HashMap::new();
        s_jong_map.insert(JamoEnum::Jong(Jong::S), JamoEnum::Jong(Jong::SS));
        combined_jamo.insert(JamoEnum::Jong(Jong::S), s_jong_map);

        *self.base_builder.combined_jamo() = combined_jamo;
    }

    /// 문자열을 한글로 변환합니다.
    pub fn convert_string(&mut self, input: &str) -> String {
        let keyboard_map = self.create_keyboard_map(&self.en_keymap_file, &self.ko_keymap_file);
        BaseHangulBuilder::convert_string(input, &keyboard_map, self)
    }

    /// 한글 문자열을 세벌식 영문 자판 입력으로 변환합니다.
    pub fn convert_korean_to_english(&self, input: &str) -> String {
        let keyboard_map = self.create_keyboard_map(&self.en_keymap_file, &self.ko_keymap_file);
        let mut reverse_map = HashMap::new();

        // 키보드 맵을 뒤집어서 자모 -> 키 매핑 생성할 때, lower 케이스 우선 적용
        // 우선 lower case 문자만 체크
        for (key, jamo) in keyboard_map.iter() {
            if !key.is_ascii_uppercase() {
                reverse_map.insert(*jamo, *key);
            }
        }

        // 이후 upper case 문자는 매핑되지 않은 자모에 대해서만 처리
        for (key, jamo) in keyboard_map.iter() {
            if key.is_ascii_uppercase() && !reverse_map.contains_key(jamo) {
                reverse_map.insert(*jamo, *key);
            }
        }

        // 이중 모음 분해 매핑 정의
        let mut compound_jung_map = HashMap::new();
        compound_jung_map.insert(Jung::WA, vec![Jung::O, Jung::A]); // ㅘ -> ㅗ + ㅏ
        compound_jung_map.insert(Jung::WAE, vec![Jung::O, Jung::AE]); // ㅙ -> ㅗ + ㅐ
        compound_jung_map.insert(Jung::OE, vec![Jung::O, Jung::I]); // ㅚ -> ㅗ + ㅣ
        compound_jung_map.insert(Jung::WEO, vec![Jung::U, Jung::EO]); // ㅝ -> ㅜ + ㅓ
        compound_jung_map.insert(Jung::WE, vec![Jung::U, Jung::E]); // ㅞ -> ㅜ + ㅔ
        compound_jung_map.insert(Jung::WI, vec![Jung::U, Jung::I]); // ㅟ -> ㅜ + ㅣ
        compound_jung_map.insert(Jung::YI, vec![Jung::EU, Jung::I]); // ㅢ -> ㅡ + ㅣ

        // 이중 자음(쌍자음) 분해 매핑 정의
        let mut compound_cho_map = HashMap::new();
        compound_cho_map.insert(Cho::GG, vec![Cho::G, Cho::G]); // ㄲ -> ㄱ + ㄱ
        compound_cho_map.insert(Cho::DD, vec![Cho::D, Cho::D]); // ㄸ -> ㄷ + ㄷ
        compound_cho_map.insert(Cho::BB, vec![Cho::B, Cho::B]); // ㅃ -> ㅂ + ㅂ
        compound_cho_map.insert(Cho::SS, vec![Cho::S, Cho::S]); // ㅆ -> ㅅ + ㅅ
        compound_cho_map.insert(Cho::JJ, vec![Cho::J, Cho::J]); // ㅉ -> ㅈ + ㅈ

        // 이중 종성(겹받침) 분해 매핑
        let mut compound_jong_map = HashMap::new();
        compound_jong_map.insert(Jong::GG, vec![Jong::G, Jong::G]); // ㄲ -> ㄱ + ㄱ
        compound_jong_map.insert(Jong::GS, vec![Jong::G, Jong::S]); // ㄳ -> ㄱ + ㅅ
        compound_jong_map.insert(Jong::NJ, vec![Jong::N, Jong::J]); // ㄵ -> ㄴ + ㅈ
        compound_jong_map.insert(Jong::NH, vec![Jong::N, Jong::H]); // ㄶ -> ㄴ + ㅎ
        compound_jong_map.insert(Jong::LG, vec![Jong::L, Jong::G]); // ㄺ -> ㄹ + ㄱ
        compound_jong_map.insert(Jong::LM, vec![Jong::L, Jong::M]); // ㄻ -> ㄹ + ㅁ
        compound_jong_map.insert(Jong::LB, vec![Jong::L, Jong::B]); // ㄼ -> ㄹ + ㅂ
        compound_jong_map.insert(Jong::LS, vec![Jong::L, Jong::S]); // ㄽ -> ㄹ + ㅅ
        compound_jong_map.insert(Jong::LT, vec![Jong::L, Jong::T]); // ㄾ -> ㄹ + ㅌ
        compound_jong_map.insert(Jong::LP, vec![Jong::L, Jong::P]); // ㄿ -> ㄹ + ㅍ
        compound_jong_map.insert(Jong::LH, vec![Jong::L, Jong::H]); // ㅀ -> ㄹ + ㅎ
        compound_jong_map.insert(Jong::BS, vec![Jong::B, Jong::S]); // ㅄ -> ㅂ + ㅅ
        compound_jong_map.insert(Jong::SS, vec![Jong::S, Jong::S]); // ㅆ -> ㅅ + ㅅ

        let mut result = String::new();

        for c in input.chars() {
            if c as u32 >= 0xAC00 && c as u32 <= 0xD7A3 {
                // 한글 완성형 문자인 경우
                let mut hangul_char = HangulChar::new();
                hangul_char.set_jamo_by_syllable(c);

                // 초성 변환 (이중자음 처리)
                if let Some(cho) = hangul_char.get_cho() {
                    if let Some(components) = compound_cho_map.get(&cho) {
                        // 이중자음인 경우 분해해서 처리
                        for &base_cho in components {
                            if let Some(&key) = reverse_map.get(&JamoEnum::Cho(base_cho)) {
                                result.push(key);
                            }
                        }
                    } else {
                        // 기본 자음인 경우 그대로 처리
                        if let Some(&key) = reverse_map.get(&JamoEnum::Cho(cho)) {
                            result.push(key);
                        }
                    }
                }

                // 중성 변환 (이중 모음 처리)
                if let Some(jung) = hangul_char.get_jung() {
                    if let Some(components) = compound_jung_map.get(&jung) {
                        // 이중 모음인 경우 분해해서 처리
                        for &base_jung in components {
                            if let Some(&key) = reverse_map.get(&JamoEnum::Jung(base_jung)) {
                                result.push(key);
                            }
                        }
                    } else {
                        // 기본 모음인 경우 그대로 처리
                        if let Some(&key) = reverse_map.get(&JamoEnum::Jung(jung)) {
                            result.push(key);
                        }
                    }
                }

                // 종성 변환 (겹받침 처리)
                if let Some(jong) = hangul_char.get_jong() {
                    if jong != Jong::E {
                        if let Some(components) = compound_jong_map.get(&jong) {
                            // 겹받침인 경우 분해해서 처리
                            for &base_jong in components {
                                if let Some(&key) = reverse_map.get(&JamoEnum::Jong(base_jong)) {
                                    result.push(key);
                                }
                            }
                        } else {
                            // 기본 받침인 경우 그대로 처리
                            if let Some(&key) = reverse_map.get(&JamoEnum::Jong(jong)) {
                                result.push(key);
                            }
                        }
                    }
                }
            } else if c as u32 >= 0x3131 && c as u32 <= 0x318E {
                // 한글 호환용 자모인 경우 (ㄱ, ㄴ, ㅏ, ㅑ 등)
                let mut found = false;

                // 초성(Cho) 확인 (이중자음 처리)
                for cho_val in 0..19 {
                    if let Some(cho) = get_cho_by_sequence(cho_val) {
                        let jamo_enum = JamoEnum::Cho(cho);
                        if jamo_enum.get_unicode_compat() == c {
                            if let Some(components) = compound_cho_map.get(&cho) {
                                // 이중자음인 경우 분해해서 처리
                                for &base_cho in components {
                                    if let Some(&key) = reverse_map.get(&JamoEnum::Cho(base_cho)) {
                                        result.push(key);
                                    }
                                }
                            } else {
                                // 기본 자음인 경우 그대로 처리
                                if let Some(&key) = reverse_map.get(&jamo_enum) {
                                    result.push(key);
                                }
                            }
                            found = true;
                            break;
                        }
                    }
                }

                // 중성(Jung) 확인 (이중모음 처리)
                if !found {
                    for jung_val in 0..21 {
                        if let Some(jung) = get_jung_by_sequence(jung_val) {
                            let jamo_enum = JamoEnum::Jung(jung);
                            if jamo_enum.get_unicode_compat() == c {
                                if let Some(components) = compound_jung_map.get(&jung) {
                                    // 이중 모음인 경우 분해해서 처리
                                    for &base_jung in components {
                                        if let Some(&key) =
                                            reverse_map.get(&JamoEnum::Jung(base_jung))
                                        {
                                            result.push(key);
                                        }
                                    }
                                } else {
                                    // 기본 모음인 경우 그대로 처리
                                    if let Some(&key) = reverse_map.get(&jamo_enum) {
                                        result.push(key);
                                    }
                                }
                                found = true;
                                break;
                            }
                        }
                    }
                }

                // 종성(Jong) 확인 (겹받침 처리)
                if !found {
                    for jong_val in 1..28 {
                        // 0은 종성 없음이므로 1부터 시작
                        if let Some(jong) = get_jong_by_sequence(jong_val) {
                            let jamo_enum = JamoEnum::Jong(jong);
                            if jamo_enum.get_unicode_compat() == c {
                                if let Some(components) = compound_jong_map.get(&jong) {
                                    // 겹받침인 경우 분해해서 처리
                                    for &base_jong in components {
                                        if let Some(&key) =
                                            reverse_map.get(&JamoEnum::Jong(base_jong))
                                        {
                                            result.push(key);
                                        }
                                    }
                                } else {
                                    // 기본 받침인 경우 그대로 처리
                                    if let Some(&key) = reverse_map.get(&jamo_enum) {
                                        result.push(key);
                                    }
                                }
                                found = true;
                                break;
                            }
                        }
                    }
                }

                // 매칭되는 키를 찾지 못한 경우 그대로 추가
                if !found {
                    result.push(c);
                }
            } else {
                // 한글이 아닌 경우 그대로 추가
                result.push(c);
            }
        }

        result
    }
}

impl HangulBuilder for HangulBuilder3Bul {
    fn add_jamo(&mut self, jamo: JamoEnum) -> Option<char> {
        let mut queue = VecDeque::new();
        queue.extend(self.base_builder.jamo_queue().iter().copied());
        queue.push_back(jamo);

        self.base_builder.jamo_queue().clear();
        self.base_builder.jamo_queue().extend(queue);

        if !self.build_hangul() {
            self.base_builder.jamo_queue().pop_back();
            self.build_hangul();
            let complete_hangul = self.base_builder.current_hangul().get_syllable();

            let current_queue: Vec<_> = self.base_builder.jamo_queue().iter().copied().collect();
            self.base_builder.last_jamo_queue().clear();
            self.base_builder.last_jamo_queue().extend(current_queue);
            self.base_builder.jamo_queue().clear();
            self.base_builder.jamo_queue().push_back(jamo);

            self.clear_jamo();
            self.build_hangul();
            Some(complete_hangul)
        } else {
            None
        }
    }

    fn remove_jamo(&mut self) -> Option<JamoEnum> {
        self.base_builder.remove_jamo()
    }

    fn build_hangul(&mut self) -> bool {
        // 큐가 비어있는지 먼저 확인
        if self.base_builder.jamo_queue().is_empty() {
            self.clear_jamo();
            return true;
        }

        // 큐의 내용을 복사하여 작업
        let queue_contents: Vec<_> = self.base_builder.jamo_queue().iter().copied().collect();
        let last_jamo = queue_contents.last().unwrap();
        let last_prev_jamo = if queue_contents.len() > 1 {
            Some(queue_contents[queue_contents.len() - 2])
        } else {
            None
        };

        // 현재 상태 확인
        let is_filled_jung = self.base_builder.current_hangul().is_filled_jung();

        // 3벌식 특수 규칙 검사
        match (last_prev_jamo, last_jamo) {
            // 초성+종성 또는 중성+종성만 있을 때 종성이 들어오면 실패
            (_, JamoEnum::Jong(_)) if !is_filled_jung => {
                return false;
            }
            // 중성이나 종성 다음에 초성이 오면 실패
            (Some(JamoEnum::Jung(_) | JamoEnum::Jong(_)), JamoEnum::Cho(_)) => return false,
            // 종성 다음에 중성이 오면 실패
            (Some(JamoEnum::Jong(_)), JamoEnum::Jung(_)) => return false,
            _ => {}
        }

        if !self.build_cho() || !self.build_jung() || !self.build_jong() {
            return false;
        }

        true
    }

    fn force_build_hangul(&mut self) -> Option<char> {
        self.base_builder.force_build_hangul()
    }

    fn is_build(&self) -> bool {
        self.base_builder.is_build()
    }

    fn is_new_syllable(&self) -> bool {
        self.base_builder.is_new_syllable()
    }

    fn build_cho(&mut self) -> bool {
        self.base_builder.build_cho()
    }

    fn build_jung(&mut self) -> bool {
        self.base_builder.build_jung()
    }

    fn build_jong(&mut self) -> bool {
        self.base_builder.build_jong()
    }

    fn clear_jamo(&mut self) {
        self.base_builder.clear_jamo()
    }

    fn get_current_cho(&self) -> Option<Cho> {
        self.base_builder.get_current_cho()
    }

    fn get_current_jung(&self) -> Option<Jung> {
        self.base_builder.get_current_jung()
    }

    fn get_current_jong(&self) -> Option<Jong> {
        self.base_builder.get_current_jong()
    }

    fn set_current_cho(&mut self, cho: Option<Cho>) -> bool {
        self.base_builder.set_current_cho(cho)
    }

    fn set_current_jung(&mut self, jung: Option<Jung>) -> bool {
        self.base_builder.set_current_jung(jung)
    }

    fn set_current_jong(&mut self, jong: Option<Jong>) -> bool {
        self.base_builder.set_current_jong(jong)
    }

    fn get_combined_jamo(&self) -> &HashMap<JamoEnum, HashMap<JamoEnum, JamoEnum>> {
        self.base_builder.get_combined_jamo()
    }

    fn jamo_queue(&mut self) -> &mut VecDeque<JamoEnum> {
        self.base_builder.jamo_queue()
    }

    fn last_jamo_queue(&mut self) -> &mut VecDeque<JamoEnum> {
        self.base_builder.last_jamo_queue()
    }

    fn combined_jamo(&mut self) -> &mut HashMap<JamoEnum, HashMap<JamoEnum, JamoEnum>> {
        self.base_builder.combined_jamo()
    }

    fn current_hangul(&mut self) -> &mut HangulChar {
        self.base_builder.current_hangul()
    }
}
