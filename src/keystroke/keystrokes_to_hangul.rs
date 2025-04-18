use crate::hangul::composer::HangulComposer;
use crate::hangul::jamo::JamoEnum;
use std::collections::HashMap;

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
