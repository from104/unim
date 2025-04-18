use crate::hangul::jamo::*;
use std::collections::HashMap;

/// 한글 문자열을 영문 자판 입력으로 변환하는 함수
pub fn hangul_to_keystrokes(
    input: &str,
    keyboard_map: &HashMap<char, JamoEnum>,
    is_3bul: bool, // true면 3벌식, false면 2벌식
) -> String {
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

    // 이중 자음(쌍자음) 분해 매핑 정의 (3벌식용)
    let mut compound_cho_map = HashMap::new();
    compound_cho_map.insert(Cho::GG, vec![Cho::G, Cho::G]); // ㄲ -> ㄱ + ㄱ
    compound_cho_map.insert(Cho::DD, vec![Cho::D, Cho::D]); // ㄸ -> ㄷ + ㄷ
    compound_cho_map.insert(Cho::BB, vec![Cho::B, Cho::B]); // ㅃ -> ㅂ + ㅂ
    compound_cho_map.insert(Cho::SS, vec![Cho::S, Cho::S]); // ㅆ -> ㅅ + ㅅ
    compound_cho_map.insert(Cho::JJ, vec![Cho::J, Cho::J]); // ㅉ -> ㅈ + ㅈ

    // 겹받침 분해 매핑 정의
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
            let mut hangul_char = crate::hangul::char::HangulChar::new();
            hangul_char.set_jamo_by_syllable(c);

            // 초성 변환 (이중자음 처리 - 3벌식일 경우만)
            if let Some(cho) = hangul_char.get_cho() {
                if is_3bul && compound_cho_map.contains_key(&cho) {
                    // 3벌식이고 이중자음인 경우 분해해서 처리
                    for &base_cho in compound_cho_map.get(&cho).unwrap() {
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
                            if is_3bul {
                                // 3벌식은 종성을 그대로 사용
                                if let Some(&key) = reverse_map.get(&JamoEnum::Jong(base_jong)) {
                                    result.push(key);
                                }
                            } else {
                                // 2벌식은 종성을 초성으로 변환하여 매핑
                                let cho = base_jong.to_cho();
                                if let Some(&key) = reverse_map.get(&JamoEnum::Cho(cho)) {
                                    result.push(key);
                                }
                            }
                        }
                    } else {
                        // 기본 받침인 경우
                        if is_3bul {
                            // 3벌식은 종성을 그대로 사용
                            if let Some(&key) = reverse_map.get(&JamoEnum::Jong(jong)) {
                                result.push(key);
                            }
                        } else {
                            // 2벌식은 종성을 초성으로 변환하여 처리
                            let cho = jong.to_cho();
                            if let Some(&key) = reverse_map.get(&JamoEnum::Cho(cho)) {
                                result.push(key);
                            }
                        }
                    }
                }
            }
        } else if c as u32 >= 0x3131 && c as u32 <= 0x318E && is_3bul {
            // 한글 호환용 자모인 경우 (ㄱ, ㄴ, ㅏ, ㅑ 등) - 3벌식에서만 처리
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
                                    if let Some(&key) = reverse_map.get(&JamoEnum::Jung(base_jung))
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
                    if let Some(jong) = get_jong_by_sequence(jong_val) {
                        let jamo_enum = JamoEnum::Jong(jong);
                        if jamo_enum.get_unicode_compat() == c {
                            if let Some(components) = compound_jong_map.get(&jong) {
                                // 겹받침인 경우 분해해서 처리
                                for &base_jong in components {
                                    if let Some(&key) = reverse_map.get(&JamoEnum::Jong(base_jong))
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
