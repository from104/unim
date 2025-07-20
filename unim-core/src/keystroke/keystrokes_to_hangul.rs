use crate::hangul::composer::HangulComposer;
use crate::hangul::input_context::{ComposerType, HangulInputContext};
use crate::hangul::jamo::JamoEnum;
use crate::keystroke::keyboard_map::{self, Keystroke};
use std::collections::HashMap;

/// Converts a slice of `Keystroke`s to a Hangul string.
/// This function is a high-level wrapper.
pub fn keystrokes_to_hangul_string(keystrokes: &[Keystroke], layout: &str) -> String {
    let composer_type = if layout.starts_with("ko_3") {
        ComposerType::ThreeBul
    } else {
        ComposerType::TwoBul
    };
    let mut context = HangulInputContext::new(composer_type);

    // Keystroke to JamoEnum mapping is needed for the context processing.
    // This requires a reverse map from Keystroke to Jamo, which is complex.
    // Let's simplify by creating a direct char -> jamo map for the given layout.
    // NOTE: This is a simplified approach. A more robust solution would handle
    // Keystroke -> JamoEnum mapping directly.
    let jamo_map = create_jamo_map(layout).unwrap_or_default();

    for keystroke in keystrokes {
        if let keyboard_map::Key::Char(c) = keystroke.key {
            // Use the char from the keystroke to find the corresponding Jamo.
            if let Some(jamo) = jamo_map.get(&c) {
                context.process_jamo(*jamo);
            } else {
                context.commit();
                // Here, we should append the non-jamo char to the result.
                // The current context logic doesn't directly support this,
                // so we'll handle it after the loop.
            }
        } else if let keyboard_map::Key::Raw(c) = keystroke.key {
            context.commit();
            context.append_to_committed(c);
        }
    }

    context.commit();
    context.get_committed().to_string()
}

// Helper function to create a char -> JamoEnum map for a given layout.
// This is a simplified stand-in for the original project's more complex logic.
fn create_jamo_map(layout: &str) -> Result<HashMap<char, JamoEnum>, Box<dyn std::error::Error>> {
    // This is a placeholder. In a real implementation, this would properly
    // load the corresponding 'en' and 'ko' maps and create the mapping.
    // For now, we'll just create a dummy map for 2-bul standard.
    let mut map = HashMap::new();
    if layout == "ko_2bulstd" {
        map.insert('r', JamoEnum::Cho(crate::hangul::jamo::Cho::G));
        map.insert('k', JamoEnum::Jung(crate::hangul::jamo::Jung::A));
        map.insert('s', JamoEnum::Cho(crate::hangul::jamo::Cho::N));
        map.insert('g', JamoEnum::Jong(crate::hangul::jamo::Jong::G));
        map.insert('f', JamoEnum::Cho(crate::hangul::jamo::Cho::R));
        map.insert('l', JamoEnum::Jong(crate::hangul::jamo::Jong::L));
    }
    Ok(map)
}

/// 영문 키보드 입력 문자열을 한글로 변환하는 기능을 제공합니다.
///
/// 이 모듈은 사용자가 입력한 영문 키보드 문자를 한글 자모로 변환하고,
/// `HangulComposer` 인터페이스를 통해 자모를 조합하여 한글 음절을 생성합니다.
/// 한글이 아닌 문자(예: 영문, 숫자, 특수 문자)는 그대로 출력됩니다.
///
/// 영문 키보드 입력 문자열을 한글로 변환합니다.
///
/// 입력된 각 문자에 대해 다음과 같은 처리를 수행합니다:
/// 1. `keyboard_map`을 사용하여 문자에 해당하는 자모를 찾습니다.
/// 2. 자모가 발견되면:
///    - 특수 문자(`JamoEnum::Special`)인 경우 현재 조합 중인 글자를 완성하고 특수 문자를 추가합니다.
///    - 일반 자모인 경우 `HangulComposer`에 추가하여 조합을 시도합니다.
///      완성된 음절이 있으면 결과에 추가합니다.
/// 3. 자모가 발견되지 않으면(한글이 아닌 문자):
///    - 현재 조합 중인 글자가 있다면 완성하여 결과에 추가합니다.
///    - 입력 문자를 그대로 결과에 추가합니다.
/// 4. 모든 문자 처리 후 아직 조합 중인 글자가 있다면 강제로 완성하여 결과에 추가합니다.
///
/// # 타입 매개변수
/// * `T` - 한글 자모 조합을 담당하는 `HangulComposer` 트레이트를 구현한 타입입니다.
///
/// # 인자
/// * `input` - 변환할 영문 키보드 입력 문자열입니다.
/// * `keyboard_map` - 영문 키(`char`)와 한글 자모(`JamoEnum`) 간의 매핑 정보입니다.
/// * `composer` - 한글 자모를 조합하는 `HangulComposer` 인스턴스입니다.
///
/// # 반환값
/// 변환된 한글 문자열을 반환합니다. 완성된 한글 음절, 특수 문자, 기타 입력 문자가 포함됩니다.
///
/// # 예시
/// ```
/// use std::collections::HashMap;
/// use my_crate::hangul::jamo::JamoEnum;
/// use my_crate::hangul::composer::StandardHangulComposer;
/// use my_crate::keystroke::keystrokes_to_hangul;
///
/// let mut keyboard_map = HashMap::new();
/// // 키보드 맵 초기화... (예: 'g' -> JamoEnum::Cho(Cho::G))
///
/// let mut composer = StandardHangulComposer::new();
/// let result = keystrokes_to_hangul("gksrmf", &keyboard_map, &mut composer);
/// assert_eq!(result, "한글");
/// ```
pub fn keystrokes_to_hangul<T: HangulComposer>(
    input: &str,
    keyboard_map: &HashMap<char, JamoEnum>,
    composer: &mut T,
) -> String {
    let mut result = String::new();

    for c in input.chars() {
        match keyboard_map.get(&c) {
            // 1. 키보드 맵에서 자모를 찾은 경우
            Some(jamo) => match jamo {
                // 1.1. 특수 문자 자모인 경우 (예: 한자, 특수 기호 등)
                JamoEnum::Special(special_char) => {
                    // 현재 조합 중인 글자가 있으면 먼저 완성
                    flush_composer_to_result(composer, &mut result);
                    // 특수 문자를 결과에 추가
                    result.push(*special_char);
                }
                // 1.2. 일반 자모(초성, 중성, 종성)인 경우
                _ => {
                    // 자모를 조합기에 추가하고, 완성된 글자가 있으면 결과에 추가
                    if let Some(completed_char) = composer.add_jamo(*jamo) {
                        result.push(completed_char);
                    }
                }
            },
            // 2. 키보드 맵에서 자모를 찾지 못한 경우 (영문, 숫자, 기타 문자)
            None => {
                // 현재 조합 중인 글자가 있으면 먼저 완성
                flush_composer_to_result(composer, &mut result);
                // 원본 문자를 그대로 결과에 추가
                result.push(c);
            }
        }
    }

    // 마지막으로 조합 중이던 글자가 있다면 출력
    flush_composer_to_result(composer, &mut result);

    result
}

/// 조합기에 현재 조합 중인 글자가 있는 경우, 강제로 완성하여 결과 문자열에 추가합니다.
///
/// # 인자
/// * `composer` - 한글 자모 조합기입니다.
/// * `result` - 완성된 글자를 추가할 결과 문자열 버퍼입니다.
///
/// 이 함수는 코드 중복을 줄이기 위해 사용됩니다. 조합 중인 글자를 강제로 완성시켜야 할
/// 여러 시점(특수 문자 만남, 비한글 문자 만남, 처리 완료 등)에서 호출됩니다.
#[inline]
fn flush_composer_to_result<T: HangulComposer>(composer: &mut T, result: &mut String) {
    if composer.is_compose() {
        if let Some(hangul_char) = composer.force_compose_hangul() {
            result.push(hangul_char);
        }
    }
}

/// 영문 키보드 입력 문자열을 처리하여 `HangulInputContext`의 상태를 업데이트하고,
/// 최종 확정된 문자열을 반환합니다.
///
/// 이 함수는 각 키 입력(`char`)을 `keyboard_map`을 통해 한글 자모로 변환하고,
/// `HangulInputContext`의 `process_jamo` 메서드를 호출하여 조합 상태를 관리합니다.
/// 한글 자모로 변환되지 않는 문자는 현재 조합 상태를 확정(commit)시키기만 하고,
/// 해당 문자는 무시됩니다.
///
/// # 인자
/// * `input` - 처리할 영문 키보드 입력 문자열입니다.
/// * `keyboard_map` - 영문 키(`char`)와 한글 자모(`JamoEnum`) 간의 매핑 정보입니다.
/// * `context` - 한글 입력 상태를 관리하는 `HangulInputContext` 인스턴스입니다.
///
/// # 반환값
/// 모든 입력 처리 후 `HangulInputContext`에 최종적으로 확정된(committed) 문자열 전체를 반환합니다.
/// (주의: 이 함수는 입력 문자열 처리에 따른 *새로운* 확정 문자만 반환하는 것이 아니라,
/// 컨텍스트의 *전체* 확정 문자열을 반환합니다.)
///
/// # 예시
/// ```ignore
/// // 사용 예시는 HangulInputContext와 KeyboardMap 설정이 필요합니다.
/// // let mut context = HangulInputContext::new(ComposerType::TwoBul);
/// // let keyboard_map = ...;
/// // let final_string = process_keystrokes("gksrmf", &keyboard_map, &mut context);
/// // assert_eq!(final_string, "한글");
/// ```
pub fn process_keystrokes(
    input: &str,
    keyboard_map: &HashMap<char, JamoEnum>,
    context: &mut HangulInputContext,
) -> String {
    for c in input.chars() {
        match keyboard_map.get(&c) {
            // 1. 키보드 맵에서 자모를 찾은 경우
            Some(jamo) => {
                // 자모를 컨텍스트에 전달하여 처리
                context.process_jamo(*jamo);
            }
            // 2. 키보드 맵에서 자모를 찾지 못한 경우 (영문, 숫자, 기타 문자)
            None => {
                // 현재 조합 중인 내용을 먼저 확정 (commit)
                context.commit();
                // 해당 문자는 현재 로직에서 처리되지 않고 무시됨.
                // 필요하다면 여기에 문자를 committed_string 등에 추가하는 로직 구현 가능
            }
        }
    }

    // 모든 입력 처리 후, 마지막 조합 상태를 확정
    context.commit();

    // 최종 확정된 문자열 반환
    context.get_committed().to_string()
}

// --- 유닛 테스트 (HangulInputContext 구조 및 KeyboardMap 필요) ---
#[cfg(test)]
mod tests {
    use super::*;
    use crate::hangul::jamo::*;
    use crate::keystroke::input_context::{ComposerType, HangulInputContext};
    // keyboard_map 모듈이나 해당 함수가 실제 프로젝트 구조에 맞게 존재하는지 확인 필요
    // use crate::keystroke::keyboard_map;

    // Helper to create a simple 2bul map for testing
    fn create_test_map() -> HashMap<char, JamoEnum> {
        let mut map = HashMap::new();
        map.insert('r', JamoEnum::Cho(Cho::G));
        map.insert('k', JamoEnum::Jung(Jung::A));
        map.insert('s', JamoEnum::Cho(Cho::N)); // For "간"
        map.insert('f', JamoEnum::Cho(Cho::R));
        map.insert('m', JamoEnum::Jung(Jung::EU)); // 'ㅡ' for "글"
                                                   // Add more mappings as needed for tests
        map
    }

    #[test]
    fn test_process_keystrokes_basic() {
        let mut context = HangulInputContext::new(ComposerType::TwoBul);
        let keyboard_map = create_test_map(); // Use helper map for isolated test
                                              // "rks" -> 간
        process_keystrokes("r", &keyboard_map, &mut context);
        assert_eq!(context.get_preedit(), "ㄱ");
        process_keystrokes("k", &keyboard_map, &mut context);
        assert_eq!(context.get_preedit(), "가");
        process_keystrokes("s", &keyboard_map, &mut context);
        assert_eq!(context.get_preedit(), "간");
        process_keystrokes(" ", &keyboard_map, &mut context); // 공백 입력시 commit
        assert_eq!(context.get_preedit(), "");
        // Assuming non-mapped chars commit and append. Need commit_char in context.
        // assert_eq!(context.get_committed(), "간 ");

        // "fm" -> 를 (or 글 depending on layout and composer)
        // This part depends heavily on actual keyboard layout and composer logic
        // process_keystrokes("f", &keyboard_map, &mut context);
        // assert_eq!(context.get_preedit(), "ㄹ");
        // process_keystrokes("m", &keyboard_map, &mut context);
        // assert_eq!(context.get_preedit(), "를"); // or "르"
        // process_keystrokes("f", &keyboard_map, &mut context); // Jongseong 'ㄹ'
        // assert_eq!(context.get_preedit(), "를"); // or "를"
        // process_keystrokes("!", &keyboard_map, &mut context); // Non-hangul
        // assert_eq!(context.get_preedit(), "");
        // assert_eq!(context.get_committed(), "간 를!"); // Example
    }

    #[test]
    fn test_process_keystrokes_non_hangul() {
        let mut context = HangulInputContext::new(ComposerType::TwoBul);
        let keyboard_map = create_test_map();

        process_keystrokes("rkrk", &keyboard_map, &mut context); // "가가" (preedit 상태)
        assert_eq!(context.get_preedit(), "가가");
        process_keystrokes("a", &keyboard_map, &mut context); // 'a' is not mapped, commits "가가", 'a' is ignored
        assert_eq!(context.get_committed(), "가가");
        assert_eq!(context.get_preedit(), "");
        process_keystrokes("b", &keyboard_map, &mut context); // 'b' is not mapped, commits nothing, 'b' is ignored
        assert_eq!(context.get_committed(), "가가");
        assert_eq!(context.get_preedit(), "");
        process_keystrokes("c", &keyboard_map, &mut context); // 'c' is not mapped, commits nothing, 'c' is ignored
        assert_eq!(context.get_committed(), "가가");
        assert_eq!(context.get_preedit(), "");
        process_keystrokes("s", &keyboard_map, &mut context); // 's' -> 'ㄴ' (preedit 상태)
        assert_eq!(context.get_preedit(), "ㄴ");
        assert_eq!(context.get_committed(), "가가");

        // Final state check after committing the last 'ㄴ'
        context.commit(); // 명시적으로 마지막 상태 commit
        assert_eq!(context.get_committed(), "가가ㄴ");
        assert_eq!(context.get_preedit(), "");
        // Expected committed string: "가가ㄴ" because non-mapped chars 'a', 'b', 'c' are ignored.
        println!("Final committed: {}", context.get_committed());
        assert_eq!(context.get_committed(), "가가ㄴ");
    }
}
