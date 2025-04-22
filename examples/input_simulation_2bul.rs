use unin::hangul::{composer::HangulComposer, composer_with_2bul::HangulComposer2Bul, jamo::*};

fn main() {
    let mut composer = HangulComposer2Bul::new();
    let mut result = String::new();

    // 입력 순서: ㅎ, ㅏ, ㄴ, ㄱ, ㅡ, ㄹ, ㅇ, ㅣ (안녕하세요) + 추가 테스트
    let inputs = vec![
        JamoEnum::Cho(Chosung::Hieuh),  // ㅎ
        JamoEnum::Jung(Jungsung::A),    // ㅏ -> 하
        JamoEnum::Cho(Chosung::Nieun),  // ㄴ -> 한
        JamoEnum::Cho(Chosung::Giyeok), // ㄱ (종성 뒤 초성 -> 받침으로?) -> 한ㄱ? 아니면 안? -> 안
        JamoEnum::Jung(Jungsung::Eu),   // ㅡ (도깨비불: ㄱ받침 탈락 후 초성ㄱ + ㅡ) -> 안, 그
        JamoEnum::Cho(Chosung::Rieul),  // ㄹ -> 글
        JamoEnum::Cho(Chosung::Ieung),  // ㅇ -> 글ㅇ? -> 그ㄹ? -> 글
        JamoEnum::Jung(Jungsung::I),    // ㅣ -> 글, 이
        // 추가: 겹받침 및 복모음 테스트
        JamoEnum::Cho(Chosung::Giyeok), // ㄱ -> 이ㄱ
        JamoEnum::Jung(Jungsung::O),    // ㅗ -> 이, 고
        JamoEnum::Jung(Jungsung::A),    // ㅏ -> 이, 과
        JamoEnum::Cho(Chosung::Nieun),  // ㄴ -> 이, 관
        JamoEnum::Cho(Chosung::Jieut),  // ㅈ -> 이, 관ㅈ -> 관? -> 관
        JamoEnum::Jung(Jungsung::Eu),   // ㅡ (도깨비불: ㅈ 탈락 -> 초성ㅈ + ㅡ) -> 관, 즈
        JamoEnum::Cho(Chosung::Siot),   // ㅅ -> 관, 즛
    ];

    println!("2벌식 입력 시뮬레이션:");

    for jamo in inputs {
        print!(" 입력: {:?} -> ", jamo.to_char());
        let completed_char = composer.add_jamo(jamo);

        if let Some(c) = completed_char {
            result.push(c);
            print!("완성: '{}', ", c);
        }

        let current_syllable = composer.current_hangul().to_char();
        if current_syllable != '\0' {
            // Check if there's a composing character
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

    // 예상 결과: 안녕하세요 관즛 (시뮬레이션 동작에 따라 약간 다를 수 있음)
    // 실제 테스트 결과: 안 글이 관즛
    // 이유:
    // 1. 한 + ㄱ -> '한' 완성, 'ㄱ' 시작
    // 2. ㄱ + ㅡ -> '그'
    // 3. 그 + ㄹ -> '글'
    // 4. 글 + ㅇ -> '글' 완성, 'ㅇ' 시작
    // 5. ㅇ + ㅣ -> '이'
    // 6. 이 + ㄱ -> '익'
    // 7. 익 + ㅗ -> '이', '고'
    // 8. 고 + ㅏ -> '과'
    // 9. 과 + ㄴ -> '관'
    // 10. 관 + ㅈ -> '관' 완성, 'ㅈ' 시작
    // 11. ㅈ + ㅡ -> '즈'
    // 12. 즈 + ㅅ -> '즛'
    // 최종: 안글이관즛
}
