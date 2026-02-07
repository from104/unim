use crate::hangul::composer::HangulComposer;
use crate::hangul::input_context::HangulInputContext;
use crate::hangul::JamoEnum;
use std::collections::HashMap;

/// 영어 키보드 입력 문자열을 한국어로 변환하는 기능을 제공합니다.
///
/// 이 모듈은 사용자가 입력한 영어 키보드 문자를 한국어 자모로 변환하고,
/// `HangulComposer` 인터페이스를 통해 자모를 조합하여 한국어 음절을 생성합니다.
/// 한국어이 아닌 문자(예: 영어, 숫자, 특수 문자)는 그대로 출력됩니다.
///
/// 영어 키보드 입력 문자열을 한국어로 변환합니다.
///
/// 입력된 각 문자에 대해 다음과 같은 처리를 수행합니다:
/// 1. `keyboard_map`을 사용하여 문자에 해당하는 자모를 찾습니다.
/// 2. 자모가 발견되면:
///    - 특수 문자(`JamoEnum::Special`)인 경우 현재 조합 중인 글자를 완성하고 특수 문자를 추가합니다.
///    - 일반 자모인 경우 `HangulComposer`에 추가하여 조합을 시도합니다.
///      완성된 음절이 있으면 결과에 추가합니다.
/// 3. 자모가 발견되지 않으면(한국어이 아닌 문자):
///    - 현재 조합 중인 글자가 있다면 완성하여 결과에 추가합니다.
///    - 입력 문자를 그대로 결과에 추가합니다.
/// 4. 모든 문자 처리 후 아직 조합 중인 글자가 있다면 강제로 완성하여 결과에 추가합니다.
///
/// # 타입 매개변수
/// * `T` - 한국어 자모 조합을 담당하는 `HangulComposer` 트레이트를 구현한 타입입니다.
///
/// # 인자
/// * `input` - 변환할 영어 키보드 입력 문자열입니다.
/// * `keyboard_map` - 영어 키(`char`)와 한국어 자모(`JamoEnum`) 간의 매핑 정보입니다.
/// * `composer` - 한국어 자모를 조합하는 `HangulComposer` 인스턴스입니다.
///
/// # 반환값
/// 변환된 한국어 문자열을 반환합니다. 완성된 한국어 음절, 특수 문자, 기타 입력 문자가 포함됩니다.
///
/// # 예시
/// ```
/// use std::collections::HashMap;
/// use unim::hangul::jamo::JamoEnum;
/// use unim::hangul::Cho;
/// use unim::hangul::HangulComposer2Bul;
/// use unim::hangul::HangulComposer;
/// use unim::keystroke::keystrokes_to_korean::keystrokes_to_korean;
///
/// let mut keyboard_map = HashMap::new();
/// keyboard_map.insert('g', JamoEnum::Cho(Cho::H)); // ㅎ
///
/// let mut composer = HangulComposer2Bul::new();
/// let result = keystrokes_to_korean("g", &keyboard_map, &mut composer);
/// assert_eq!(result, "ㅎ");
/// ```
pub fn keystrokes_to_korean<T: HangulComposer>(
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
            // 2. 키보드 맵에서 자모를 찾지 못한 경우 (영어, 숫자, 기타 문자)
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
/// * `composer` - 한국어 자모 조합기입니다.
/// * `result` - 완성된 글자를 추가할 결과 문자열 버퍼입니다.
///
/// 이 함수는 코드 중복을 줄이기 위해 사용됩니다. 조합 중인 글자를 강제로 완성시켜야 할
/// 여러 시점(특수 문자 만남, 비한국어 문자 만남, 처리 완료 등)에서 호출됩니다.
#[inline]
fn flush_composer_to_result<T: HangulComposer>(composer: &mut T, result: &mut String) {
    if composer.is_compose() {
        if let Some(korean_char) = composer.force_compose_korean() {
            result.push(korean_char);
        }
    }
}

/// 영어 키보드 입력 문자열을 처리하여 `HangulInputContext`의 상태를 업데이트하고,
/// 최종 확정된 문자열을 반환합니다.
///
/// 이 함수는 각 키 입력(`char`)을 `keyboard_map`을 통해 한국어 자모로 변환하고,
/// `HangulInputContext`의 `process_jamo` 메서드를 호출하여 조합 상태를 관리합니다.
/// 한국어 자모로 변환되지 않는 문자는 현재 조합 상태를 확정(commit)시키기만 하고,
/// 해당 문자는 무시됩니다.
///
/// # 인자
/// * `input` - 처리할 영어 키보드 입력 문자열입니다.
/// * `keyboard_map` - 영어 키(`char`)와 한국어 자모(`JamoEnum`) 간의 매핑 정보입니다.
/// * `context` - 한국어 입력 상태를 관리하는 `HangulInputContext` 인스턴스입니다.
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
/// // assert_eq!(final_string, "한국어");
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
            // 2. 키보드 맵에서 자모를 찾지 못한 경우 (영어, 숫자, 기타 문자)
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
    use crate::hangul::input_context::{ComposerType, HangulInputContext};
    use crate::hangul::jamo::*;
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
        let keyboard_map = create_test_map();

        // process_keystrokes_with_context를 사용하여 preedit 상태 확인
        // (process_keystrokes는 내부에서 commit을 호출하므로 preedit 확인 불가)
        context.process_jamo(JamoEnum::Cho(Cho::G));
        assert_eq!(context.get_preedit(), "ㄱ");
        context.process_jamo(JamoEnum::Jung(Jung::A));
        assert_eq!(context.get_preedit(), "가");
        context.process_jamo(JamoEnum::Cho(Cho::N));
        assert_eq!(context.get_preedit(), "간");

        // 공백 대신 commit 호출
        context.commit();
        assert_eq!(context.get_committed(), "간");
        assert_eq!(context.get_preedit(), "");
    }

    #[test]
    fn test_process_keystrokes_non_korean() {
        let mut context = HangulInputContext::new(ComposerType::TwoBul);
        let keyboard_map = create_test_map();

        // 직접 자모 입력으로 테스트
        context.process_jamo(JamoEnum::Cho(Cho::G));
        context.process_jamo(JamoEnum::Jung(Jung::A));
        context.process_jamo(JamoEnum::Cho(Cho::G)); // 도깨비불 현상으로 '가' 확정
        context.process_jamo(JamoEnum::Jung(Jung::A));
        assert_eq!(context.get_committed(), "가");
        assert_eq!(context.get_preedit(), "가");

        context.commit();
        assert_eq!(context.get_committed(), "가가");

        // ㄴ 입력
        context.process_jamo(JamoEnum::Cho(Cho::N));
        assert_eq!(context.get_preedit(), "ㄴ");

        context.commit();
        assert_eq!(context.get_committed(), "가가ㄴ");
    }
}
