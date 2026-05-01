//! TypeFix 모듈
//!
//! 한/영 오타 변환 기능을 제공합니다.
//! - `eng_to_kor`: 영문 타이핑을 한글로 변환 ("gksrmf" → "한글")
//! - `kor_to_eng`: 한글을 영문 키스트로크로 변환 ("한글" → "gksrmf")

use crate::hangul::input_context::{ComposerType, HangulInputContext};
use crate::hangul::jamo::JamoEnum;
use crate::keystroke::korean_to_keystrokes::korean_to_keystrokes;
use crate::keystroke::{get_keymap_json, KeyboardMap};
use std::collections::HashMap;

/// 영문 키스트로크를 한글로 변환합니다.
///
/// 사용자가 한글 모드를 켜지 않고 영문으로 입력한 경우의 변환입니다.
/// 예: "gksrmf" → "한글"
///
/// # Arguments
/// * `text` - 영문 키스트로크 문자열
/// * `korean_layout` - 한국어 키보드 레이아웃
/// * `english_layout` - 영어 키보드 레이아웃
pub fn eng_to_kor(text: &str, korean_layout: &str, english_layout: &str) -> String {
    let is_three_bul = crate::config::is_sebeolsik_layout(korean_layout);
    let composer_type = if is_three_bul {
        ComposerType::ThreeBul
    } else {
        ComposerType::TwoBul
    };

    let en_keymap = crate::config::english_layout_keymap_name(english_layout);
    let en_json = get_keymap_json(&en_keymap);
    let ko_json = get_keymap_json(korean_layout);

    let keyboard_map = KeyboardMap::create_keyboard_map_from_str(en_json, ko_json, is_three_bul);

    let mut context = HangulInputContext::new(composer_type);

    for c in text.chars() {
        if let Some(jamo) = keyboard_map.get(&c) {
            context.process_jamo(*jamo);
        } else {
            // 자모 매핑이 없는 문자는 그대로 통과
            context.commit();
            context.append_to_committed(c);
        }
    }

    context.commit();
    context.get_committed().to_string()
}

/// 한글을 영문 키스트로크로 변환합니다.
///
/// 사용자가 영문 모드를 켜지 않고 한글로 입력한 경우의 변환입니다.
/// 예: "한글" → "gksrmf"
///
/// # Arguments
/// * `text` - 한글 문자열
/// * `korean_layout` - 한국어 키보드 레이아웃
/// * `english_layout` - 영어 키보드 레이아웃
pub fn kor_to_eng(text: &str, korean_layout: &str, english_layout: &str) -> String {
    let is_three_bul = crate::config::is_sebeolsik_layout(korean_layout);

    let en_keymap = crate::config::english_layout_keymap_name(english_layout);
    let en_json = get_keymap_json(&en_keymap);
    let ko_json = get_keymap_json(korean_layout);

    let keyboard_map = KeyboardMap::create_keyboard_map_from_str(en_json, ko_json, is_three_bul);

    korean_to_keystrokes(text, &keyboard_map, is_three_bul)
}

/// 텍스트가 영문 키스트로크(한글로 변환 가능)인지 판별합니다.
///
/// 주어진 텍스트가 한글 키보드 매핑에 속하는 영문 문자로만 구성되어 있으면 true.
pub fn is_english_keystrokes(text: &str, keyboard_map: &HashMap<char, JamoEnum>) -> bool {
    if text.is_empty() {
        return false;
    }
    text.chars()
        .all(|c| keyboard_map.contains_key(&c) || c == ' ')
}

/// 텍스트가 한글(영문으로 변환 가능)인지 판별합니다.
pub fn is_korean_text(text: &str) -> bool {
    if text.is_empty() {
        return false;
    }
    text.chars().all(|c| {
        // 한글 음절 범위: U+AC00 ~ U+D7A3
        // 한글 자모 범위: U+3131 ~ U+3163
        ('\u{AC00}'..='\u{D7A3}').contains(&c) || ('\u{3131}'..='\u{3163}').contains(&c) || c == ' '
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_eng_to_kor_basic() {
        // "gksrmf" → "한글" (두벌식 + QWERTY)
        let result = eng_to_kor("gksrmf", "ko_2bulstd", "qwerty");
        assert_eq!(result, "한글");
    }

    #[test]
    fn test_eng_to_kor_sentence() {
        // "dkssudgktpdy" → "안녕하세요" (두벌식 + QWERTY)
        let result = eng_to_kor("dkssudgktpdy", "ko_2bulstd", "qwerty");
        assert_eq!(result, "안녕하세요");
    }

    #[test]
    fn test_kor_to_eng_basic() {
        // "한글" → "gksrmf" (두벌식 + QWERTY)
        let result = kor_to_eng("한글", "ko_2bulstd", "qwerty");
        assert_eq!(result, "gksrmf");
    }

    #[test]
    fn test_kor_to_eng_sentence() {
        // "안녕하세요" → "dkssudgktpdy" (두벌식 + QWERTY)
        let result = kor_to_eng("안녕하세요", "ko_2bulstd", "qwerty");
        assert_eq!(result, "dkssudgktpdy");
    }

    #[test]
    fn test_roundtrip_eng_kor_eng() {
        let original = "gksrmf";
        let korean = eng_to_kor(original, "ko_2bulstd", "qwerty");
        let back = kor_to_eng(&korean, "ko_2bulstd", "qwerty");
        assert_eq!(back, original);
    }

    #[test]
    fn test_roundtrip_kor_eng_kor() {
        let original = "한글";
        let english = kor_to_eng(original, "ko_2bulstd", "qwerty");
        let back = eng_to_kor(&english, "ko_2bulstd", "qwerty");
        assert_eq!(back, original);
    }

    #[test]
    fn test_is_korean_text() {
        assert!(is_korean_text("한글"));
        assert!(is_korean_text("안녕하세요"));
        assert!(!is_korean_text("hello"));
        assert!(!is_korean_text("한글hello"));
        assert!(!is_korean_text(""));
    }

    #[test]
    fn test_eng_to_kor_mixed() {
        // 영문+숫자 혼합: 숫자는 그대로
        let result = eng_to_kor("gksrmf123", "ko_2bulstd", "qwerty");
        assert_eq!(result, "한글123");
    }
}
