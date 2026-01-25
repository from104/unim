//! String Deconstruction Example
//!
//! Demonstrates how to iterate through a string and deconstruct each Korean 
//! syllable into its individual Jamo components (Initial, Middle, Final).

use unim::korean::{char::KoreanChar, jamo::*};

fn deconstruct_string(input: &str) {
    println!("Processing: \"{}\"", input);
    
    for c in input.chars() {
        if let Some(korean_char) = KoreanChar::from_char(c) {
            // Attempt to get the (Cho, Jung, Jong) tuple
            if let Some((cho, jung, jong)) = korean_char.to_jamo_tuple() {
                println!(
                    "  '{}' -> Cho: '{}', Jung: '{}', Jong: '{}'",
                    c,
                    cho.to_char(),
                    jung.to_char(),
                    jong.map_or(' ', |j| j.to_char())
                );
            } else if let Some(jamo) = korean_char.to_jamo() {
                // If the character is a standalone Jamo (not a composed syllable)
                match jamo {
                    JamoEnum::Cho(v) => println!("  '{}' -> Standalone Initial: '{}'", c, v.to_char()),
                    JamoEnum::Jung(v) => println!("  '{}' -> Standalone Middle: '{}'", c, v.to_char()),
                    JamoEnum::Jong(v) => println!("  '{}' -> Standalone Final: '{}'", c, v.to_char()),
                    _ => println!("  '{}' -> Other Korean unit", c),
                }
            }
        } else {
            println!("  '{}' -> Non-Korean character", c);
        }
    }
    println!("----------------------------------");
}

fn main() {
    // 1. Standard composed syllables
    deconstruct_string("안녕");
    
    // 2. Mixed content
    deconstruct_string("Rust 1.0");
    
    // 3. Standalone Jamo sequence
    deconstruct_string("ㄱㅏㄴㅏ");
}
