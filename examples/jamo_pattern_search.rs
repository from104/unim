use unin::hangul::{char::HangulChar, jamo::*};

// Helper to convert JamoEnum to char, handling potential None cases
fn jamo_enum_to_char(jamo: &JamoEnum) -> char {
    match jamo {
        JamoEnum::Cho(c) => c.to_char(),
        JamoEnum::Jung(j) => j.to_char(),
        JamoEnum::Jong(j) => j.to_char(),
        // Special case might need specific handling depending on desired output
        JamoEnum::Special(s) => s.to_char(),
    }
}

/// 문자열을 자모 문자열과 원본 문자 인덱스 매핑 정보로 분해합니다.
/// 반환값: (자모 문자열, [각 자모에 해당하는 원본 문자열 시작 인덱스])
fn decompose_to_jamo_string(text: &str) -> (String, Vec<usize>) {
    let mut jamo_string = String::new();
    // 각 자모 문자가 원본 텍스트의 몇 번째 'char' 인덱스에서 왔는지 추적합니다.
    let mut jamo_to_origin_char_index = Vec::new();
    // 원본 텍스트의 char 인덱스를 저장합니다.
    let origin_char_indices: Vec<usize> = text.char_indices().map(|(i, _)| i).collect();
    let mut current_char_idx_in_origin = 0;

    for c in text.chars() {
        let original_char_start_index = origin_char_indices[current_char_idx_in_origin];
        let mut jamo_added = false;

        if let Some(hangul_char) = HangulChar::from_char(c) {
            if let Some((cho, jung, jong_opt)) = hangul_char.to_jamo_tuple() {
                jamo_string.push(cho.to_char());
                jamo_to_origin_char_index.push(original_char_start_index);
                jamo_string.push(jung.to_char());
                jamo_to_origin_char_index.push(original_char_start_index);
                if let Some(jong) = jong_opt {
                    jamo_string.push(jong.to_char());
                    jamo_to_origin_char_index.push(original_char_start_index);
                }
                jamo_added = true;
            } else if let Some(jamo) = hangul_char.to_jamo() {
                // 독립 자모 처리 (예: 'ㄱ', 'ㅏ')
                jamo_string.push(jamo_enum_to_char(&jamo));
                jamo_to_origin_char_index.push(original_char_start_index);
                jamo_added = true;
            }
        }

        // 한글이 아니거나, 분해되지 않는 한글 (호환용 자모 등)은 그대로 추가
        if !jamo_added {
            jamo_string.push(c);
            jamo_to_origin_char_index.push(original_char_start_index);
        }
        current_char_idx_in_origin += 1; // 다음 원본 문자로 이동
    }
    (jamo_string, jamo_to_origin_char_index)
}

/// 분해된 자모 문자열에서 패턴을 검색하고, 원본 문자열에서의 위치를 반환합니다.
/// 반환값: Vec<(원본 시작 인덱스, 원본 끝 인덱스(exclusive))>
fn find_jamo_pattern_in_text(text: &str, pattern: &str) -> Vec<(usize, usize)> {
    if pattern.is_empty() {
        return Vec::new();
    }

    let (jamo_string, jamo_to_origin_char_index) = decompose_to_jamo_string(text);
    let mut results = Vec::new();
    let jamo_chars: Vec<char> = jamo_string.chars().collect();
    let pattern_chars: Vec<char> = pattern.chars().collect();
    let pattern_len = pattern_chars.len();

    if jamo_chars.len() < pattern_len {
        return Vec::new(); // 검색 대상 문자열이 패턴보다 짧으면 검색 불가
    }

    for i in 0..=(jamo_chars.len() - pattern_len) {
        if jamo_chars[i..i + pattern_len] == pattern_chars[..] {
            // 매칭된 자모 시퀀스의 시작과 끝에 해당하는 원본 문자열 인덱스를 찾습니다.
            let origin_start_index = jamo_to_origin_char_index[i];
            let origin_end_char_start_index = jamo_to_origin_char_index[i + pattern_len - 1];

            // 끝 인덱스는 매칭된 마지막 자모가 속한 '원본 문자'의 끝 다음 위치여야 합니다.
            // 원본 문자열에서 origin_end_char_start_index 다음 문자의 시작 위치를 찾습니다.
            let origin_end_index = text
                .char_indices()
                .find(|(idx, _)| *idx == origin_end_char_start_index) // 마지막 자모가 속한 원본 문자를 찾고
                .map(|(idx, c)| idx + c.len_utf8()) // 그 문자의 끝 위치(시작 + 길이)를 계산
                .unwrap_or_else(|| text.len()); // 마지막 문자면 전체 길이

            results.push((origin_start_index, origin_end_index));
        }
    }

    // 중복 제거 (예: '가가'에서 'ㄱㅏ'를 찾으면 같은 시작 인덱스가 두 번 나올 수 있음)
    results.sort();
    results.dedup();
    results
}

fn main() {
    let text = "테스트 문자열입니다. 가격은 얼마? ㄱㅏ나다 123 ㄹㄹㅏ.";

    println!("원본 텍스트: \"{}\"", text);
    let (jamo_view, _) = decompose_to_jamo_string(text);
    println!("자모 분해: \"{}\"", jamo_view);
    println!("---");

    let patterns = [
        "ㅅㅡㅌ",
        "ㄱㅏㄱㅕ",
        "ㄱㅏ",
        "ㄹㅁㅏ",
        "ㄴㄷㅏ",
        " ",
        "ㄹㄹㅏ",
        "123",
    ];

    for pattern in patterns {
        let matches = find_jamo_pattern_in_text(text, pattern);
        println!("패턴 \"{}\" 검색 결과:", pattern);
        if matches.is_empty() {
            println!("  - 없음");
        } else {
            for (start, end) in matches {
                println!(
                    "  - 원본 인덱스 [{}..{}): \"{}\"",
                    start,
                    end,
                    &text[start..end]
                );
            }
        }
        println!("---");
    }
}
