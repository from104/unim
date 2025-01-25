use hangul::builder::HangulBuilder;
use hangul::builder_2bul::HangulBuilder2Bul;
use std::collections::HashMap;
use std::io::{self, Read};

use crate::hangul::jamo::{Cho, Jung, PhonemeEnum};

pub mod hangul;

fn main() -> io::Result<()> {
    let mut keyboard_map = HashMap::new();
    init_keyboard_map(&mut keyboard_map);

    let mut hangul_builder = HangulBuilder2Bul::new();
    let mut input_buffer = [0; 1];

    loop {
        if io::stdin().read_exact(&mut input_buffer).is_err() {
            break;
        }

        let input_char = input_buffer[0] as char;
        if input_char == '.' {
            // 입력이 끝났을 때 마지막으로 조합 중이던 글자가 있다면 출력
            if hangul_builder.is_build() {
                if let Some(hangul_char) = hangul_builder.force_build_hangul() {
                    print!("{}", hangul_char);
                }
            }
            break;
        }

        if let Some(phoneme) = keyboard_map.get(&input_char) {
            // add_jamo의 결과를 바로 출력하지 않고, 새로운 글자가 시작될 때만 이전 글자를 출력
            if let Some(completed_char) = hangul_builder.add_jamo(*phoneme) {
                // 새로운 글자가 시작될 때만 이전 글자를 출력
                if hangul_builder.is_new_syllable() {
                    print!("{}", completed_char);
                }
            }
        } else {
            // 한글이 아닌 문자가 입력되었을 때 현재 조합 중인 글자가 있다면 출력
            if hangul_builder.is_build() {
                if let Some(hangul_char) = hangul_builder.force_build_hangul() {
                    print!("{}", hangul_char);
                }
            }
            print!("{}", input_char);
        }
    }

    println!(); // 마지막 줄바꿈 추가
    Ok(())
}

fn init_keyboard_map(keyboard_map: &mut HashMap<char, PhonemeEnum>) {
    // 초성 (Consonants)
    keyboard_map.insert('q', PhonemeEnum::Cho(Cho::B));
    keyboard_map.insert('w', PhonemeEnum::Cho(Cho::J));
    keyboard_map.insert('e', PhonemeEnum::Cho(Cho::D));
    keyboard_map.insert('r', PhonemeEnum::Cho(Cho::G));
    keyboard_map.insert('t', PhonemeEnum::Cho(Cho::S));
    keyboard_map.insert('a', PhonemeEnum::Cho(Cho::M));
    keyboard_map.insert('s', PhonemeEnum::Cho(Cho::N));
    keyboard_map.insert('d', PhonemeEnum::Cho(Cho::E));
    keyboard_map.insert('f', PhonemeEnum::Cho(Cho::R));
    keyboard_map.insert('g', PhonemeEnum::Cho(Cho::H));
    keyboard_map.insert('z', PhonemeEnum::Cho(Cho::K));
    keyboard_map.insert('x', PhonemeEnum::Cho(Cho::T));
    keyboard_map.insert('c', PhonemeEnum::Cho(Cho::C));
    keyboard_map.insert('v', PhonemeEnum::Cho(Cho::P));

    // 중성 (Vowels)
    keyboard_map.insert('y', PhonemeEnum::Jung(Jung::YO));
    keyboard_map.insert('u', PhonemeEnum::Jung(Jung::YEO));
    keyboard_map.insert('i', PhonemeEnum::Jung(Jung::YA));
    keyboard_map.insert('o', PhonemeEnum::Jung(Jung::AE));
    keyboard_map.insert('p', PhonemeEnum::Jung(Jung::E));
    keyboard_map.insert('h', PhonemeEnum::Jung(Jung::O));
    keyboard_map.insert('j', PhonemeEnum::Jung(Jung::EO));
    keyboard_map.insert('k', PhonemeEnum::Jung(Jung::A));
    keyboard_map.insert('l', PhonemeEnum::Jung(Jung::I));
    keyboard_map.insert('b', PhonemeEnum::Jung(Jung::YU));
    keyboard_map.insert('n', PhonemeEnum::Jung(Jung::U));
    keyboard_map.insert('m', PhonemeEnum::Jung(Jung::EU));

    // 대문자 (쌍자음) (Double Consonants)
    keyboard_map.insert('Q', PhonemeEnum::Cho(Cho::BB));
    keyboard_map.insert('W', PhonemeEnum::Cho(Cho::JJ));
    keyboard_map.insert('E', PhonemeEnum::Cho(Cho::DD));
    keyboard_map.insert('R', PhonemeEnum::Cho(Cho::GG));
    keyboard_map.insert('T', PhonemeEnum::Cho(Cho::SS));

    // 대문자 (복합 모음) (Complex Vowels)
    keyboard_map.insert('O', PhonemeEnum::Jung(Jung::YAE));
    keyboard_map.insert('P', PhonemeEnum::Jung(Jung::YE));
}
