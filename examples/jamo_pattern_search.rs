//! Korean Jamo Pattern Search
//!
//! This advanced example demonstrates how to perform a "fuzzy" search by decomposing
//! target text into its component Jamo and matching against a Jamo pattern.
//! This is useful for features like Jamo-based autocompletion or filtering.

use unim::hangul::{char::HangulChar, jamo::*};

/// Helper to extract raw char from JamoEnum
fn jamo_enum_to_raw(jamo: &JamoEnum) -> char {
    match jamo {
        JamoEnum::Cho(c) => c.to_char(),
        JamoEnum::Jung(j) => j.to_char(),
        JamoEnum::Jong(j) => j.to_char(),
        JamoEnum::Special(s) => *s,
    }
}

/// Decomposes a string into a sequence of Jamo and tracks their original source indices.
/// Returns: (Jamo sequence string, List of original byte offsets for each Jamo)
fn decompose_with_mapping(text: &str) -> (String, Vec<usize>) {
    let mut jamo_seq = String::new();
    let mut mapping = Vec::new();

    for (idx, c) in text.char_indices() {
        let mut added = false;

        if let Some(korean_char) = HangulChar::from_char(c) {
            if let Some((cho, jung, jong_opt)) = korean_char.to_jamo_tuple() {
                // Initial
                jamo_seq.push(cho.to_char());
                mapping.push(idx);
                // Middle
                jamo_seq.push(jung.to_char());
                mapping.push(idx);
                // Final (if exists)
                if let Some(jong) = jong_opt {
                    jamo_seq.push(jong.to_char());
                    mapping.push(idx);
                }
                added = true;
            } else if let Some(jamo) = korean_char.to_jamo() {
                // Standalone Jamo (non-composed)
                jamo_seq.push(jamo_enum_to_raw(&jamo));
                mapping.push(idx);
                added = true;
            }
        }

        // Non-Korean or fallback
        if !added {
            jamo_seq.push(c);
            mapping.push(idx);
        }
    }
    (jamo_seq, mapping)
}

/// Finds a Jamo pattern within the text and returns original string ranges.
fn find_pattern(text: &str, pattern: &str) -> Vec<(usize, usize)> {
    if pattern.is_empty() {
        return vec![];
    }

    let (jamo_seq, mapping) = decompose_with_mapping(text);
    let mut matches = Vec::new();

    let jamo_chars: Vec<char> = jamo_seq.chars().collect();
    let pattern_chars: Vec<char> = pattern.chars().collect();
    let p_len = pattern_chars.len();

    if jamo_chars.len() < p_len {
        return vec![];
    }

    for i in 0..=(jamo_chars.len() - p_len) {
        if jamo_chars[i..i + p_len] == pattern_chars[..] {
            let start_byte = mapping[i];
            let last_jamo_byte = mapping[i + p_len - 1];

            // Calculate the end byte based on the UTF-8 length of the last character
            let end_byte = text
                .char_indices()
                .find(|(idx, _)| *idx == last_jamo_byte)
                .map(|(idx, c)| idx + c.len_utf8())
                .unwrap_or(text.len());

            matches.push((start_byte, end_byte));
        }
    }

    matches.sort();
    matches.dedup();
    matches
}

fn main() {
    let text = "Rust로 작성된 unim 라이브러리 테스트. 가격은 얼마?";
    let pattern = "ㄱㅏㄱㅕㄱ"; // Searching for "가격" as Jamo

    println!("Full Text: \"{}\"", text);
    println!("Pattern:   \"{}\"", pattern);
    println!("---");

    let results = find_pattern(text, pattern);

    if results.is_empty() {
        println!("No matches found.");
    } else {
        for (start, end) in results {
            println!(
                "Match at offset {}..{}: \"{}\"",
                start,
                end,
                &text[start..end]
            );
        }
    }

    // Additional Test with simple consonants
    let char_pattern = "ㄹㅏ";
    let char_results = find_pattern(text, char_pattern);
    println!("\nPattern:   \"{}\"", char_pattern);
    for (start, end) in char_results {
        println!(
            "Match at offset {}..{}: \"{}\"",
            start,
            end,
            &text[start..end]
        );
    }
}
