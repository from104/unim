use unim::hangul::{composer::HangulComposer, composer_with_3bul::HangulComposer3Bul, jamo::*};

fn main() {
    let mut composer = HangulComposer3Bul::new();
    let mut result = String::new();

    // 입력 순서 (3벌식 스타일): ㄱ, ㅏ, ㄱ, ㅅ, (완성), ㄴ, ㅐ, (완성) ...
    // 3벌식은 초성, 중성, 종성을 구분하여 입력합니다.
    let inputs = vec![
        JamoEnum::Cho(Chosung::Giyeok),   // ㄱ (초성)
        JamoEnum::Jung(Jungsung::A),      // ㅏ (중성) -> 가
        JamoEnum::Jong(Jongsung::Giyeok), // ㄱ (종성) -> 각
        JamoEnum::Jong(Jongsung::Siot),   // ㅅ (종성) -> 갃 (겹받침 조합)
        // 새 글자 시작 시 이전 글자 자동 완성되어야 함
        JamoEnum::Cho(Chosung::Nieun), // ㄴ (초성) -> 갃 나오고, 나 시작
        JamoEnum::Jung(Jungsung::Ae),  // ㅐ (중성) -> 갃, 내
        JamoEnum::Jong(Jongsung::Nieun), // ㄴ (종성) -> 갃, 낸
        JamoEnum::Jong(Jongsung::Jieut), // ㅈ (종성) -> 갃, 낸ㅈ (겹받침 조합)
        // 모음 입력으로 새 글자 시작
        JamoEnum::Jung(Jungsung::Eo), // ㅓ (중성) -> 갃낸ㅈ 나오고, ㅓ 시작 (초성 없이)
        JamoEnum::Jong(Jongsung::Rieul), // ㄹ (종성) -> 갃낸ㅈ, 얼
        // 자음 입력으로 새 글자 시작
        JamoEnum::Cho(Chosung::Rieul), // ㄹ (초성) -> 갃낸ㅈ얼 나오고, 라 시작
        JamoEnum::Jung(Jungsung::A),   // ㅏ (중성) -> 갃낸ㅈ얼, 라
    ];

    println!("3벌식 입력 시뮬레이션:");

    for jamo in inputs {
        print!(" 입력: {:?} -> ", jamo.to_char());
        let completed_char = composer.add_jamo(jamo);

        if let Some(c) = completed_char {
            result.push(c);
            print!("완성: '{}', ", c);
        }

        let current_syllable = composer.current_hangul().to_char();
        if current_syllable != '\0' {
            println!("현재 조합: '{}'", current_syllable);
        } else {
            println!("현재 조합: -");
        }
    }

    // 마지막 조합 중인 글자 처리
    if let Some(last_char) = composer.force_compose_hangul() {
        result.push(last_char);
    }

    println!("\n최종 결과: {}", result);
    // 예상 결과: 갃낸ㅈ얼라
}
