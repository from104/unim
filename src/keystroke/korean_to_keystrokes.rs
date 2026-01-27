use crate::korean::char::HangulChar;
use crate::korean::char::HangulCharExt;
use crate::korean::jamo::*;
use std::collections::HashMap;

/// 한국어 문자열을 해당하는 영어 키보드 입력 시퀀스로 변환하는 기능을 제공합니다.
///
/// 2벌식 표준 자판과 3벌식 390 자판을 지원하며, 입력된 `keyboard_map`과
/// `is_3bul` 플래그를 기반으로 변환 로직을 결정합니다.
/// 완성형 한국어 음절, 호환용 한국어 자모(3벌식의 경우), 그리고 기타 문자를 처리합니다.
///
/// 복합 자모 분해 규칙을 담는 구조체
struct CompoundMaps {
    jung: HashMap<Jung, Vec<Jung>>,
    cho: HashMap<Cho, Vec<Cho>>, // 3벌식 쌍자음용
    jong: HashMap<Jong, Vec<Jong>>,
}

/// 한국어 문자열을 해당하는 영어 키보드 입력 시퀀스로 변환합니다.
///
/// # Arguments
/// * `input` - 변환할 한국어 문자열.
/// * `keyboard_map` - 영어 키(`char`)와 한국어 자모(`JamoEnum`) 매핑 정보.
///                    `keyboard_map::KeyboardMap::create_keyboard_map` 함수로 생성됩니다.
/// * `is_3bul` - `true`이면 3벌식(390), `false`이면 2벌식(표준) 규칙을 적용합니다.
///
/// # Returns
/// * 입력된 한국어 문자열을 타이핑하기 위한 영어 키스트로크 시퀀스 문자열.
/// * 한국어 외 문자는 그대로 반환됩니다.
/// * 매핑되지 않는 자모가 포함된 경우 해당 자모는 무시될 수 있습니다.
pub fn korean_to_keystrokes(
    input: &str,
    keyboard_map: &HashMap<char, JamoEnum>,
    is_3bul: bool,
) -> String {
    // 1. 자모 -> 영어 키 역방향 매핑 생성 (소문자 우선)
    let reverse_jamo_map = build_reverse_jamo_map(keyboard_map);

    // 1.1 기타 문자 -> 영어 키 역방향 매핑 생성 (JamoEnum::Other, JamoEnum::Special)
    let reverse_char_map = build_reverse_char_map(keyboard_map);

    // 2. 복합 자모(이중모음, 쌍자음, 겹받침) 분해 규칙 정의
    let compound_maps = define_compound_maps();

    let mut result = String::new();

    // 3. 입력 문자열 순회 및 변환
    for c in input.chars() {
        if c.is_korean() {
            // 3.1. 완성형 한국어 음절 처리
            append_syllable_keystrokes(c, &reverse_jamo_map, &compound_maps, is_3bul, &mut result);
        // } else if is_3bul && c.is_korean_compat_jamo() {
        //     // 3.2. 호환용 한국어 자모 처리 (3벌식에서만)
        //     append_compat_jamo_keystrokes(c, &reverse_jamo_map, &compound_maps, &mut result);
        } else {
            // 3.3. 한국어이 아닌 다른 문자 처리 (수정됨)
            // 기타 문자 역방향 맵에서 문자에 해당하는 키를 찾습니다.
            if let Some(&key) = reverse_char_map.get(&c) {
                result.push(key);
            } else {
                // 매핑되는 키가 없으면 원본 문자를 그대로 추가 (또는 오류 처리)
                result.push(c);
            }
        }
    }

    result
}

/// `keyboard_map` (영어 키 -> 자모)을 기반으로 역방향 맵 (자모 -> 영어 키)을 생성합니다.
/// JamoEnum::Cho, JamoEnum::Jung, JamoEnum::Jong 만 포함합니다.
/// 동일한 자모에 대해 대문자와 소문자 키가 모두 매핑된 경우, 소문자 키를 우선합니다.
///
/// # Arguments
/// * `keyboard_map` - 원본 영어 키 -> 자모 매핑.
///
/// # Returns
/// 자모(`JamoEnum`)에서 영어 키(`char`)로의 역방향 `HashMap`.
fn build_reverse_jamo_map(keyboard_map: &HashMap<char, JamoEnum>) -> HashMap<JamoEnum, char> {
    let mut reverse_map = HashMap::<JamoEnum, char>::new();

    for (&key, &jamo) in keyboard_map {
        // 자모 타입인 경우에만 처리
        if matches!(
            jamo,
            JamoEnum::Cho(_) | JamoEnum::Jung(_) | JamoEnum::Jong(_)
        ) {
            match reverse_map.get(&jamo) {
                Some(&existing_key) => {
                    if key.is_ascii_lowercase() && existing_key.is_ascii_uppercase() {
                        reverse_map.insert(jamo, key);
                    }
                }
                None => {
                    reverse_map.insert(jamo, key);
                }
            }
        }
    }
    reverse_map
}

/// `keyboard_map` (영어 키 -> 자모)을 기반으로 기타 문자 역방향 맵 (문자 -> 영어 키)을 생성합니다.
/// JamoEnum::Other 및 JamoEnum::Special 을 포함합니다.
/// 동일한 문자에 대해 대문자와 소문자 키가 모두 매핑된 경우, 어떤 키가 선택될지는 HashMap 구현에 따라 다름.
/// (필요하다면 소문자 우선 로직 추가)
///
/// # Arguments
/// * `keyboard_map` - 원본 영어 키 -> 자모 매핑.
///
/// # Returns
/// 문자(`char`)에서 영어 키(`char`)로의 역방향 `HashMap`.
fn build_reverse_char_map(keyboard_map: &HashMap<char, JamoEnum>) -> HashMap<char, char> {
    let mut reverse_map = HashMap::<char, char>::new();
    for (&key, &jamo) in keyboard_map {
        match jamo {
            JamoEnum::Special(c) => {
                // TODO: 소문자 우선 로직 필요시 추가
                reverse_map.insert(c, key);
            }
            _ => {}
        }
    }
    reverse_map
}

/// 복합 자모(이중모음, 쌍자음, 겹받침)의 분해 규칙을 정의하는 `HashMap`들을 생성합니다.
///
/// # Returns
/// `CompoundMaps` 구조체 인스턴스.
fn define_compound_maps() -> CompoundMaps {
    // 이중 모음 분해 매핑 (Jung::WA -> [Jung::O, Jung::A])
    let compound_jung_map = HashMap::<Jung, Vec<Jung>>::from([
        (Jung::WA, vec![Jung::O, Jung::A]),   // ㅘ -> ㅗ + ㅏ
        (Jung::WAE, vec![Jung::O, Jung::AE]), // ㅙ -> ㅗ + ㅐ
        (Jung::OE, vec![Jung::O, Jung::I]),   // ㅚ -> ㅗ + ㅣ
        (Jung::WEO, vec![Jung::U, Jung::EO]), // ㅝ -> ㅜ + ㅓ
        (Jung::WE, vec![Jung::U, Jung::E]),   // ㅞ -> ㅜ + ㅔ
        (Jung::WI, vec![Jung::U, Jung::I]),   // ㅟ -> ㅜ + ㅣ
        (Jung::YI, vec![Jung::EU, Jung::I]),  // ㅢ -> ㅡ + ㅣ
    ]);

    // 이중 자음(쌍자음) 분해 매핑 (Cho::GG -> [Cho::G, Cho::G]) - 3벌식용
    let compound_cho_map = HashMap::<Cho, Vec<Cho>>::from([
        (Cho::GG, vec![Cho::G, Cho::G]), // ㄲ -> ㄱ + ㄱ
        (Cho::DD, vec![Cho::D, Cho::D]), // ㄸ -> ㄷ + ㄷ
        (Cho::BB, vec![Cho::B, Cho::B]), // ㅃ -> ㅂ + ㅂ
        (Cho::SS, vec![Cho::S, Cho::S]), // ㅆ -> ㅅ + ㅅ
        (Cho::JJ, vec![Cho::J, Cho::J]), // ㅉ -> ㅈ + ㅈ
    ]);

    // 겹받침 분해 매핑 (Jong::GS -> [Jong::G, Jong::S])
    let compound_jong_map = HashMap::<Jong, Vec<Jong>>::from([
        (Jong::GG, vec![Jong::G, Jong::G]), // ㄲ -> ㄱ + ㄱ
        (Jong::GS, vec![Jong::G, Jong::S]), // ㄳ -> ㄱ + ㅅ
        (Jong::NJ, vec![Jong::N, Jong::J]), // ㄵ -> ㄴ + ㅈ
        (Jong::NH, vec![Jong::N, Jong::H]), // ㄶ -> ㄴ + ㅎ
        (Jong::LG, vec![Jong::L, Jong::G]), // ㄺ -> ㄹ + ㄱ
        (Jong::LM, vec![Jong::L, Jong::M]), // ㄻ -> ㄹ + ㅁ
        (Jong::LB, vec![Jong::L, Jong::B]), // ㄼ -> ㄹ + ㅂ
        (Jong::LS, vec![Jong::L, Jong::S]), // ㄽ -> ㄹ + ㅅ
        (Jong::LT, vec![Jong::L, Jong::T]), // ㄾ -> ㄹ + ㅌ
        (Jong::LP, vec![Jong::L, Jong::P]), // ㄿ -> ㄹ + ㅍ
        (Jong::LH, vec![Jong::L, Jong::H]), // ㅀ -> ㄹ + ㅎ
        (Jong::BS, vec![Jong::B, Jong::S]), // ㅄ -> ㅂ + ㅅ
        (Jong::SS, vec![Jong::S, Jong::S]), // ㅆ -> ㅅ + ㅅ (종성)
    ]);

    CompoundMaps {
        jung: compound_jung_map,
        cho: compound_cho_map,
        jong: compound_jong_map,
    }
}

/// 완성형 한국어 음절(`c`)을 받아 초/중/종성으로 분해하고, 각 자모에 해당하는 키스트로크를
/// `result` 문자열에 추가합니다.
///
/// # Arguments
/// * `c` - 처리할 완성형 한국어 음절 문자.
/// * `reverse_map` - 자모 -> 영어 키 역방향 매핑.
/// * `compound_maps` - 복합 자모 분해 규칙.
/// * `is_3bul` - 3벌식 여부 플래그.
/// * `result` - 키스트로크를 추가할 대상 문자열 버퍼.
fn append_syllable_keystrokes(
    c: char,
    reverse_map: &HashMap<JamoEnum, char>,
    compound_maps: &CompoundMaps,
    is_3bul: bool,
    result: &mut String,
) {
    let mut korean_char = HangulChar::new();
    // 음절 분해 시 발생할 수 있는 오류는 무시 (예: 잘못된 유니코드). 이후 get_jamo에서 None으로 처리됨.
    let _ = korean_char.set_jamo_by_syllable(c);

    // 초성 처리
    if let Some(cho) = korean_char.get_cho() {
        append_jamo_keystrokes(
            &JamoEnum::Cho(cho),
            reverse_map,
            compound_maps,
            is_3bul,
            result,
        );
    }
    // 중성 처리
    if let Some(jung) = korean_char.get_jung() {
        append_jamo_keystrokes(
            &JamoEnum::Jung(jung),
            reverse_map,
            compound_maps,
            is_3bul,
            result,
        );
    }
    // 종성 처리 (종성이 있는 경우 '\u{11A7}' (Jong::E - 없음)이 아님)
    if let Some(jong) = korean_char.get_jong() {
        if jong != Jong::E {
            append_jamo_keystrokes(
                &JamoEnum::Jong(jong),
                reverse_map,
                compound_maps,
                is_3bul,
                result,
            );
        }
    }
}

/// 단일 `JamoEnum` (초성, 중성, 또는 종성)을 받아 해당하는 키스트로크(들)를
/// `result` 문자열에 추가합니다.
///
/// 복합 자모(이중모음, 쌍자음, 겹받침)를 분해하고, 2벌식/3벌식 규칙에 따라
/// (특히 종성의 경우) 올바른 키를 찾아 추가합니다.
///
/// # Arguments
/// * `jamo_enum` - 처리할 자모 (`JamoEnum::Cho`, `JamoEnum::Jung`, `JamoEnum::Jong`).
/// * `reverse_map` - 자모 -> 영어 키 역방향 매핑.
/// * `compound_maps` - 복합 자모 분해 규칙.
/// * `is_3bul` - 3벌식 여부 플래그.
/// * `result` - 키스트로크를 추가할 대상 문자열 버퍼.
fn append_jamo_keystrokes(
    jamo_enum: &JamoEnum,
    reverse_map: &HashMap<JamoEnum, char>,
    compound_maps: &CompoundMaps,
    is_3bul: bool,
    result: &mut String,
) {
    match jamo_enum {
        JamoEnum::Cho(cho) => {
            // 3벌식이고 복합 초성(쌍자음)인 경우 분해
            if is_3bul {
                if let Some(components) = compound_maps.cho.get(cho) {
                    for &base_cho in components {
                        if let Some(&key) = reverse_map.get(&JamoEnum::Cho(base_cho)) {
                            result.push(key);
                        }
                    }
                    return; // 분해 처리 했으므로 종료
                }
            }
            // 단일 초성 또는 2벌식의 경우
            if let Some(&key) = reverse_map.get(jamo_enum) {
                result.push(key);
            }
        }
        JamoEnum::Jung(jung) => {
            // 복합 중성(이중모음)인 경우 분해
            if let Some(components) = compound_maps.jung.get(jung) {
                for &base_jung in components {
                    if let Some(&key) = reverse_map.get(&JamoEnum::Jung(base_jung)) {
                        result.push(key);
                    }
                }
            } else {
                // 단일 중성
                if let Some(&key) = reverse_map.get(jamo_enum) {
                    result.push(key);
                }
            }
        }
        JamoEnum::Jong(jong) => {
            // 복합 종성(겹받침)인 경우 분해
            if let Some(components) = compound_maps.jong.get(jong) {
                for &base_jong in components {
                    let jamo_to_lookup = if is_3bul {
                        // 3벌식: 분해된 종성 자모 그대로 사용
                        JamoEnum::Jong(base_jong)
                    } else {
                        // 2벌식: 분해된 종성 자모를 초성으로 변환하여 사용
                        JamoEnum::Cho(base_jong.to_cho().unwrap())
                    };
                    if let Some(&key) = reverse_map.get(&jamo_to_lookup) {
                        result.push(key);
                    }
                }
            } else {
                // 단일 종성
                let jamo_to_lookup = if is_3bul {
                    // 3벌식: 단일 종성 자모 그대로 사용
                    *jamo_enum
                } else {
                    // 2벌식: 단일 종성 자모를 초성으로 변환하여 사용
                    JamoEnum::Cho(jong.to_cho().unwrap())
                };
                if let Some(&key) = reverse_map.get(&jamo_to_lookup) {
                    result.push(key);
                }
            }
        }
        // JamoEnum::Special은 이 함수에서 직접 처리하지 않음 (keyboard_map 정의에 따름)
        _ => { /* Do nothing for Special or potential future variants */ }
    }
}
