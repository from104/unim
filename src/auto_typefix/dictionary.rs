//! 한글 음절 카운트 + 영어 사전 조회

use super::DICTIONARY;

/// 한글 텍스트에서 음절 수 카운트
///
/// 초성+중성 이상이면 1음절로 카운트:
/// - 완성형 음절 (U+AC00~U+D7A3): 항상 1음절
/// - 호환 자모 중 모음 (ㅏ~ㅣ): preedit에서 초+중 조합 진행 중일 수 있으나
///   독립 모음은 카운트하지 않음
/// - 독립 자음 (ㄱ~ㅎ): 카운트하지 않음
pub fn count_korean_syllables(text: &str) -> usize {
    let mut count = 0;
    for c in text.chars() {
        if ('\u{AC00}'..='\u{D7A3}').contains(&c) {
            // 완성형 음절: 항상 초+중 이상
            count += 1;
        }
        // 독립 자모(ㄱ~ㅎ, ㅏ~ㅣ)는 카운트하지 않음
        // eng_to_kor 결과에서 preedit 잔여물은 독립 자모로 나옴
    }
    count
}

/// 영어 사전에 단어가 있는지 확인 (외부 사용)
pub fn dictionary_contains(word: &str) -> bool {
    DICTIONARY.contains(word)
}
