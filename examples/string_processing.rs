use unim::hangul::{char::HangulChar, jamo::*};

fn process_string(input: &str) {
    println!("문자열 처리: \"{}\"", input);
    for c in input.chars() {
        if let Some(hangul_char) = HangulChar::from_char(c) {
            if let Some((cho, jung, jong)) = hangul_char.to_jamo_tuple() {
                println!(
                    "  '{}': 초성 '{}', 중성 '{}', 종성 '{}'",
                    c,
                    cho.to_char(),
                    jung.to_char(),
                    jong.map_or(' ', |j| j.to_char())
                );
            } else if let Some(jamo) = hangul_char.to_jamo() {
                // 만약 입력된 문자가 완성형 한글이 아니라 자모 자체라면
                match jamo {
                    JamoEnum::Cho(cho) => println!("  '{}': 초성 자모 '{}'", c, cho.to_char()),
                    JamoEnum::Jung(jung) => println!("  '{}': 중성 자모 '{}'", c, jung.to_char()),
                    JamoEnum::Jong(jong) => println!("  '{}': 종성 자모 '{}'", c, jong.to_char()),
                    JamoEnum::Special(_) => println!("  '{}': 특수 문자 (HangulChar로 처리됨)", c),
                }
            }
        } else {
            println!("  '{}': 한글 아님", c);
        }
    }
    println!("---");
}

fn main() {
    process_string("안녕하세요");
    process_string("unim 라이브러리 예제");
    process_string("ㄱㅏㄴㅏㄷㅏㄹㅏ"); // 자모가 포함된 경우
}
