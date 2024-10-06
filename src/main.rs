use std::collections::HashMap;
use std::io::{self, Read};
use hangul::builder_2bul::HangulBuilder2Bul;

use crate::hangul::phoneme::{Onset, Nucleus, PhonemeEnum};

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
            break;
        }

        if let Some(phoneme) = keyboard_map.get(&input_char) {
            println!("자모 추가? {:?}", phoneme);
            if let Some(hangul_char) = hangul_builder.add_phoneme(*phoneme) {
                println!("자모 추가? {}", hangul_char);
            }
        } else {
            if hangul_builder.is_complete() {
                if let Some(hangul_char) = hangul_builder.force_build_hangul() {
                    print!("{}", hangul_char);
                }
            }
            println!("{}", input_char);
        }
    }

    Ok(())
}

fn init_keyboard_map(keyboard_map: &mut HashMap<char, PhonemeEnum>) {
    // 초성 (Consonants)
    keyboard_map.insert('q', PhonemeEnum::Onset(Onset::B));
    keyboard_map.insert('w', PhonemeEnum::Onset(Onset::J));
    keyboard_map.insert('e', PhonemeEnum::Onset(Onset::D));
    keyboard_map.insert('r', PhonemeEnum::Onset(Onset::G));
    keyboard_map.insert('t', PhonemeEnum::Onset(Onset::S));
    keyboard_map.insert('a', PhonemeEnum::Onset(Onset::M));
    keyboard_map.insert('s', PhonemeEnum::Onset(Onset::N));
    keyboard_map.insert('d', PhonemeEnum::Onset(Onset::E));
    keyboard_map.insert('f', PhonemeEnum::Onset(Onset::R));
    keyboard_map.insert('g', PhonemeEnum::Onset(Onset::H));
    keyboard_map.insert('z', PhonemeEnum::Onset(Onset::K));
    keyboard_map.insert('x', PhonemeEnum::Onset(Onset::T));
    keyboard_map.insert('c', PhonemeEnum::Onset(Onset::C));
    keyboard_map.insert('v', PhonemeEnum::Onset(Onset::P));

    // 중성 (Vowels)
    keyboard_map.insert('y', PhonemeEnum::Nucleus(Nucleus::YO));
    keyboard_map.insert('u', PhonemeEnum::Nucleus(Nucleus::YEO));
    keyboard_map.insert('i', PhonemeEnum::Nucleus(Nucleus::YA));
    keyboard_map.insert('o', PhonemeEnum::Nucleus(Nucleus::AE));
    keyboard_map.insert('p', PhonemeEnum::Nucleus(Nucleus::E));
    keyboard_map.insert('h', PhonemeEnum::Nucleus(Nucleus::O));
    keyboard_map.insert('j', PhonemeEnum::Nucleus(Nucleus::EO));
    keyboard_map.insert('k', PhonemeEnum::Nucleus(Nucleus::A));
    keyboard_map.insert('l', PhonemeEnum::Nucleus(Nucleus::I));
    keyboard_map.insert('b', PhonemeEnum::Nucleus(Nucleus::YU));
    keyboard_map.insert('n', PhonemeEnum::Nucleus(Nucleus::U));
    keyboard_map.insert('m', PhonemeEnum::Nucleus(Nucleus::EU));

    // 대문자 (쌍자음) (Double Consonants)
    keyboard_map.insert('Q', PhonemeEnum::Onset(Onset::BB));
    keyboard_map.insert('W', PhonemeEnum::Onset(Onset::JJ));
    keyboard_map.insert('E', PhonemeEnum::Onset(Onset::DD));
    keyboard_map.insert('R', PhonemeEnum::Onset(Onset::GG));
    keyboard_map.insert('T', PhonemeEnum::Onset(Onset::SS));

    // 대문자 (복합 모음) (Complex Vowels)
    keyboard_map.insert('O', PhonemeEnum::Nucleus(Nucleus::YAE));
    keyboard_map.insert('P', PhonemeEnum::Nucleus(Nucleus::YE));
}
