use crate::hangul::composer::HangulComposer;
use crate::hangul::jamo::JamoEnum;
use std::collections::HashMap;

/// 입력된 문자열을 한글로 변환합니다.
/// 키보드 입력에 해당하는 문자들을 한글로 조합하여 반환합니다.
pub fn keystrokes_to_hangul<T: HangulComposer>(
    input: &str,
    keyboard_map: &HashMap<char, JamoEnum>,
    composer: &mut T,
) -> String {
    let mut result = String::new();

    for c in input.chars() {
        // 키보드 맵에서 자모를 찾음
        if let Some(jamo) = keyboard_map.get(&c) {
            match jamo {
                JamoEnum::Special(special_char) => {
                    // 특수 문자는 바로 출력
                    if composer.is_compose() {
                        if let Some(hangul_char) = composer.force_compose_hangul() {
                            result.push(hangul_char);
                        }
                    }
                    result.push(*special_char);
                }
                _ => {
                    // 자모는 조합 시도
                    if let Some(completed_char) = composer.add_jamo(*jamo) {
                        result.push(completed_char);
                    }
                }
            }
        } else {
            // 한글이 아닌 문자가 입력되었을 때 현재 조합 중인 글자가 있다면 출력
            if composer.is_compose() {
                if let Some(hangul_char) = composer.force_compose_hangul() {
                    result.push(hangul_char);
                }
            }
            result.push(c);
        }
    }

    // 마지막으로 조합 중이던 글자가 있다면 출력
    if composer.is_compose() {
        if let Some(hangul_char) = composer.force_compose_hangul() {
            result.push(hangul_char);
        }
    }

    result
}
