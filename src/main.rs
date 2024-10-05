use std::collections::HashMap;
use std::io::{self, Read};
use crate::hangul::phoneme::{Onset, Nucleus, Coda, Jamo};
use crate::hangul::builder::HangulBuilder2Bul;

pub mod hangul;

fn main() -> io::Result<()> {
    let mut km2 = HashMap::new();
    init_keyboard_map(&mut km2);

    let mut hb2 = HangulBuilder2Bul::new();
    let mut buffer = [0; 1];

    loop {
        if io::stdin().read_exact(&mut buffer).is_err() {
            break;
        }

        let c = buffer[0] as char;
        if c == '.' {
            break;
        }

        if let Some(jamo) = km2.get(&c) {
            if let Some(hangul) = hb2.add_jamo(*jamo) {
                print!("{}", hangul);
            }
        } else {
            if hb2.is_build() {
                if let Some(hangul) = hb2.force_build_hangul() {
                    print!("{}", hangul);
                }
            }
            print!("{}", c);
        }
    }

    Ok(())
}

fn init_keyboard_map(km2: &mut HashMap<char, Jamo>) {
    // 초성
    km2.insert('q', Jamo::Onset(Onset::B));
    km2.insert('w', Jamo::Onset(Onset::J));
    km2.insert('e', Jamo::Onset(Onset::D));
    km2.insert('r', Jamo::Onset(Onset::G));
    km2.insert('t', Jamo::Onset(Onset::S));
    km2.insert('a', Jamo::Onset(Onset::M));
    km2.insert('s', Jamo::Onset(Onset::N));
    km2.insert('d', Jamo::Onset(Onset::E));
    km2.insert('f', Jamo::Onset(Onset::R));
    km2.insert('g', Jamo::Onset(Onset::H));
    km2.insert('z', Jamo::Onset(Onset::K));
    km2.insert('x', Jamo::Onset(Onset::T));
    km2.insert('c', Jamo::Onset(Onset::C));
    km2.insert('v', Jamo::Onset(Onset::P));

    // 중성
    km2.insert('y', Jamo::Nucleus(Nucleus::YO));
    km2.insert('u', Jamo::Nucleus(Nucleus::YEO));
    km2.insert('i', Jamo::Nucleus(Nucleus::YA));
    km2.insert('o', Jamo::Nucleus(Nucleus::AE));
    km2.insert('p', Jamo::Nucleus(Nucleus::E));
    km2.insert('h', Jamo::Nucleus(Nucleus::O));
    km2.insert('j', Jamo::Nucleus(Nucleus::EO));
    km2.insert('k', Jamo::Nucleus(Nucleus::A));
    km2.insert('l', Jamo::Nucleus(Nucleus::I));
    km2.insert('b', Jamo::Nucleus(Nucleus::YU));
    km2.insert('n', Jamo::Nucleus(Nucleus::U));
    km2.insert('m', Jamo::Nucleus(Nucleus::EU));

    // 대문자 (쌍자음)
    km2.insert('Q', Jamo::Onset(Onset::BB));
    km2.insert('W', Jamo::Onset(Onset::JJ));
    km2.insert('E', Jamo::Onset(Onset::DD));
    km2.insert('R', Jamo::Onset(Onset::GG));
    km2.insert('T', Jamo::Onset(Onset::SS));

    // 대문자 (복합 모음)
    km2.insert('O', Jamo::Nucleus(Nucleus::YAE));
    km2.insert('P', Jamo::Nucleus(Nucleus::YE));
}
