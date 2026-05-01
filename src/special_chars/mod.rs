//! 초성 기반 특수문자 매핑 모듈
//!
//! Windows 한글 IME의 초성+한자키 특수문자 입력 기능과 동일한 매핑을 제공합니다.
//! 한글 초성(ㄱ~ㅎ)에 대응하는 특수문자 세트를 정의하고 검색합니다.

mod data;

use data::SPECIAL_CHAR_TABLE;

/// 초성 특수문자 항목
#[derive(Clone, Debug)]
pub struct SpecialCharEntry {
    /// 초성 문자 (예: 'ㄱ')
    pub choseong: char,
    /// 카테고리명 (예: "특수기호")
    pub category: &'static str,
    /// 특수문자 목록
    pub characters: &'static [char],
}

/// 주어진 문자가 한글 초성(자음)인지 판별합니다.
pub fn is_choseong(ch: char) -> bool {
    // 한글 호환 자모 자음 범위: ㄱ(U+3131) ~ ㅎ(U+314E)
    ('\u{3131}'..='\u{314E}').contains(&ch)
}

/// 초성 문자로 특수문자 항목을 검색합니다.
///
/// # 인자
///
/// * `ch` - 검색할 초성 문자 (예: 'ㄱ')
///
/// # 반환
///
/// 해당 초성의 특수문자 항목. 매핑이 없으면 None.
pub fn search_by_choseong(ch: char) -> Option<&'static SpecialCharEntry> {
    SPECIAL_CHAR_TABLE.iter().find(|entry| entry.choseong == ch)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_all_choseong_mapped() {
        // 14개 기본 초성 모두 매핑 확인
        let choseongs = [
            'ㄱ', 'ㄴ', 'ㄷ', 'ㄹ', 'ㅁ', 'ㅂ', 'ㅅ', 'ㅇ', 'ㅈ', 'ㅊ', 'ㅋ', 'ㅌ', 'ㅍ', 'ㅎ',
        ];
        for ch in choseongs {
            let result = search_by_choseong(ch);
            assert!(result.is_some(), "'{}' 매핑이 없음", ch);
            assert!(
                !result.unwrap().characters.is_empty(),
                "'{}' 문자 목록이 비어 있음",
                ch
            );
        }
    }

    #[test]
    fn test_search_giyeok() {
        let result = search_by_choseong('ㄱ');
        assert!(result.is_some());
        let entry = result.unwrap();
        assert_eq!(entry.choseong, 'ㄱ');
        assert_eq!(entry.category, "특수기호");
        assert!(entry.characters.len() > 10);
    }

    #[test]
    fn test_search_non_choseong() {
        // 완성형 음절은 매핑 없음
        assert!(search_by_choseong('가').is_none());
        assert!(search_by_choseong('a').is_none());
        assert!(search_by_choseong('1').is_none());
    }

    #[test]
    fn test_is_choseong() {
        // 기본 자음
        assert!(is_choseong('ㄱ'));
        assert!(is_choseong('ㅎ'));
        assert!(is_choseong('ㄴ'));

        // 쌍자음
        assert!(is_choseong('ㄲ'));
        assert!(is_choseong('ㄸ'));
        assert!(is_choseong('ㅃ'));
        assert!(is_choseong('ㅆ'));
        assert!(is_choseong('ㅉ'));

        // 비-자음
        assert!(!is_choseong('가'));
        assert!(!is_choseong('a'));
        assert!(!is_choseong('ㅏ')); // 모음
    }

    #[test]
    fn test_double_choseong_no_mapping() {
        // 쌍자음은 현재 매핑 없음 (Windows에서도 기본 자음만 지원)
        assert!(search_by_choseong('ㄲ').is_none());
        assert!(search_by_choseong('ㄸ').is_none());
    }

    #[test]
    fn test_table_count() {
        assert_eq!(SPECIAL_CHAR_TABLE.len(), 14);
    }
}
