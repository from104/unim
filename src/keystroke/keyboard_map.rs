use crate::hangul::jamo::*;
use serde_json::Value;
use std::collections::HashMap;
use std::fs::File;
use std::io::Read;

/// 키보드 매핑 생성 관련 유틸리티
pub struct KeyboardMap;

impl KeyboardMap {
    /// 키보드 맵을 생성합니다.
    /// 2벌식과 3벌식에 따라 다른 매핑을 생성합니다.
    pub fn create_keyboard_map(
        en_keymap_file: &str,
        ko_keymap_file: &str,
        is_3bul: bool, // true면 3벌식, false면 2벌식
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
        let rows = if is_3bul {
            ["1st", "2nd", "3nd", "4th"] // 3벌식은 1st row부터
        } else {
            ["dummy", "2nd", "3nd", "4th"] // 2벌식은 2nd row부터 (첫 요소는 사용 안함)
        };

        // 먼저 lower 케이스만 처리
        for row in rows.iter() {
            // 2벌식의 경우 dummy 행은 건너뛰기
            if !is_3bul && *row == "dummy" {
                continue;
            }

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
                                _ => {
                                    if is_3bul {
                                        // 3벌식에만 존재하는 매핑 (종성)
                                        match ko.chars().next() {
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
                                        }
                                    } else {
                                        None
                                    }
                                }
                            };

                            if let Some(jamo) = jamo {
                                keyboard_map.insert(c, jamo);
                            }
                        }
                    }
                }
            }
        }

        // upper 케이스는 lower에 없는 경우에만 처리
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
                                _ => {
                                    if is_3bul {
                                        // 3벌식에만 존재하는 매핑 (종성)
                                        match ko.chars().next() {
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
                                        }
                                    } else {
                                        None
                                    }
                                }
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
}
