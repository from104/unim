//! 2-Set (2-bul) Hangul Input Simulation
//!
//! This example demonstrates how the `HangulComposer2Bul` handles 2-set Hangul input.
//! It simulates consecutive key presses and shows the state of composition including
//! "Dokkaebibul" (shifting a consonant to the next syllable's initial position).

use unim::hangul::{composer::HangulComposer, composer_with_2bul::HangulComposer2Bul, jamo::*};

fn main() {
    let mut composer = HangulComposer2Bul::new();
    let mut result = String::new();

    // Input sequence simulating key presses for "안녕하세요" and more.
    // In 2-set, the composer automatically decides if a consonant is an initial (Cho) or final (Jong).
    let inputs = vec![
        JamoEnum::Cho(Chosung::Hieuh),  // ㅎ
        JamoEnum::Jung(Jungsung::A),    // ㅏ -> 하
        JamoEnum::Cho(Chosung::Nieun),  // ㄴ -> 한
        JamoEnum::Cho(Chosung::Giyeok), // ㄱ -> In 2-set, this becomes a final: '한' (but may shift later)
        JamoEnum::Jung(Jungsung::Eu),   // ㅡ -> Dokkaebibul: 'ㄴ' stays as final of '한', 'ㄱ' becomes initial of '그'
        JamoEnum::Cho(Chosung::Rieul),  // ㄹ -> '글'
        JamoEnum::Cho(Chosung::Ieung),  // ㅇ -> '글' complete, 'ㅇ' starts new char
        JamoEnum::Jung(Jungsung::I),    // ㅣ -> '이'
        
        // Complex combinations and double vowels
        JamoEnum::Cho(Chosung::Giyeok), // ㄱ
        JamoEnum::Jung(Jungsung::O),    // ㅗ -> 고
        JamoEnum::Jung(Jungsung::A),    // ㅏ -> 과 (vowel combination)
        JamoEnum::Cho(Chosung::Nieun),  // ㄴ -> 관
        JamoEnum::Cho(Chosung::Jieut),  // ㅈ -> 관zzz? (composition logic determines final/initial)
        JamoEnum::Jung(Jungsung::Eu),   // ㅡ -> Shifts 'ㅈ' to '즈'
        JamoEnum::Cho(Chosung::Siot),   // ㅅ -> 즛
    ];

    println!("--- 2-Set Hangul Input Simulation ---");

    for jamo in inputs {
        print!("Input: {:?} -> ", jamo.to_char());
        
        // add_jamo returns a character if a previous syllable is finalized.
        let completed_char = composer.add_jamo(jamo);

        if let Some(c) = completed_char {
            result.push(c);
            print!("Composed: '{}', ", c);
        }

        // Check the character currently being composed.
        let current_syllable = composer.current_hangul().to_char();
        if current_syllable != '\0' {
            println!("Current: '{}'", current_syllable);
        } else {
            println!("Current: -");
        }
    }

    // Force finalize any remaining composition buffer.
    if let Some(last_char) = composer.force_compose_hangul() {
        result.push(last_char);
    }

    println!("\nFinal Result: {}", result);
    // Explanation:
    // 1. "한" + "ㄱ" -> composition waits.
    // 2. + "ㅡ" -> "한" finalized, "그" starts.
    // 3. "그" + "ㄹ" -> "글".
    // 4. "글" + "ㅇ" -> "글" finalized, "ㅇ" starts.
}
