//! Korean Syllable Generation
//!
//! This example demonstrates how to generate Korean syllables programmatically
//! by iterating through all possible Initial (Cho), Middle (Jung), and Final (Jong) sequences.

use unim::korean::char::{CHOSEONG_NUMBER, JONGSEONG_NUMBER, JUNGSEONG_NUMBER};
use unim::korean::KoreanChar;

fn main() {
    println!("--- Korean Syllable Matrix Generation ---");
    
    // There are 19 Initials, 21 Middles, and 28 Finals (including empty).
    // Total possible composed syllables: 19 * 21 * 28 = 11,172 characters.
    
    for cho_seq in 0..CHOSEONG_NUMBER {
        println!("\n[Initial Sequence Index: {}]", cho_seq);
        for jung_seq in 0..JUNGSEONG_NUMBER {
            for jong_seq in 0..JONGSEONG_NUMBER {
                // Direct calculation using the Unicode Korean algorithm
                let syllable = KoreanChar::from_jamo_sequences(
                    cho_seq as i32,
                    jung_seq as i32,
                    jong_seq as i32,
                );
                print!("{}", syllable);
            }
            // Space between middle combinations for readability
            print!(" ");
        }
        println!();
    }
    
    println!("\nSuccessfully generated the basic Korean matrix.");
}
