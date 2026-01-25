//! 3-Set (3-bul) Korean Input Simulation
//!
//! This example shows how the `KoreanComposer3Bul` handles 3-set input.
//! 3-set layouts distinguish between initial, middle, and final characters
//! at the hardware/layout level.

use unim::korean::{composer::KoreanComposer, composer_with_3bul::KoreanComposer3Bul, jamo::*};

fn main() {
    let mut composer = KoreanComposer3Bul::new();
    let mut result = String::new();

    // Input sequence for 3-set layout.
    // Unlike 2-set, the input type (Cho/Jung/Jong) is explicitly provided by the layout.
    let inputs = vec![
        JamoEnum::Cho(Chosung::Giyeok),   // Cho (Initial) ㄱ
        JamoEnum::Jung(Jungsung::A),      // Jung (Middle) ㅏ -> 가
        JamoEnum::Jong(Jongsung::Giyeok), // Jong (Final) ㄱ -> 각
        JamoEnum::Jong(Jongsung::Siot),   // Jong (Final) ㅅ -> 갃 (Complex final)

        // New character starts
        JamoEnum::Cho(Chosung::Nieun),    // Cho ㄴ -> Previous '갃' is finalized
        JamoEnum::Jung(Jungsung::Ae),     // Jung ㅐ -> 내
        JamoEnum::Jong(Jongsung::Nieun),  // Jong ㄴ -> 낸
        JamoEnum::Jong(Jongsung::Jieut),  // Jong ㅈ -> 낸ㅈ (Complex final)

        // Starting with a vowel (Middle)
        JamoEnum::Jung(Jungsung::Eo),     // Jung ㅓ -> Previous '낸ㅈ' is finalized, starts standalone Middle
        JamoEnum::Jong(Jongsung::Rieul),  // Jong ㄹ -> 얼

        // Starting with initial (Cho)
        JamoEnum::Cho(Chosung::Rieul),    // Cho ㄹ -> '얼' is finalized
        JamoEnum::Jung(Jungsung::A),      // Jung ㅏ -> 라
    ];

    println!("--- 3-Set Korean Input Simulation ---");

    for jamo in inputs {
        print!("Input: {:?} -> ", jamo.to_char());
        
        // add_jamo processes the input and returns a char if the buffer shifts.
        let completed_char = composer.add_jamo(jamo);

        if let Some(c) = completed_char {
            result.push(c);
            print!("Composed: '{}', ", c);
        }

        let current_syllable = composer.current_korean().to_char();
        if current_syllable != '\0' {
            println!("Current: '{}'", current_syllable);
        } else {
            println!("Current: -");
        }
    }

    // Capture the final character in the buffer.
    if let Some(last_char) = composer.force_compose_korean() {
        result.push(last_char);
    }

    println!("\nFinal Result: {}", result);
}
