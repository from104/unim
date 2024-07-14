use std::collections::HashMap;
use std::io::{self, Read};
use crate::hangul_char::{HangulChar, Cho, Jung};
use crate::hangul_jamo::HangulJamo;
use crate::hangul_builder_2bul::HangulBuilder2Bul;

fn main() -> io::Result<()> {
    let mut key_map: HashMap<char, Object> = HashMap::new();
    let mut input_char;
    let mut hangul_builder = HangulBuilder2Bul::new();
    
    key_map.insert('q', Cho::B.into());
    key_map.insert('w', Cho::J.into());
    key_map.insert('e', Cho::D.into());
    key_map.insert('r', Cho::G.into());
    key_map.insert('t', Cho::S.into());
    key_map.insert('y', Jung::YO.into());
    key_map.insert('u', Jung::YEO.into());
    key_map.insert('i', Jung::YA.into());
    key_map.insert('o', Jung::AE.into());
    key_map.insert('p', Jung::E.into());
    key_map.insert('a', Cho::M.into());
    key_map.insert('s', Cho::N.into());
    key_map.insert('d', Cho::E.into());
    key_map.insert('f', Cho::R.into());
    key_map.insert('g', Cho::H.into());
    key_map.insert('h', Jung::O.into());
    key_map.insert('j', Jung::EO.into());
    key_map.insert('k', Jung::A.into());
    key_map.insert('l', Jung::I.into());
    key_map.insert('z', Cho::K.into());
    key_map.insert('x', Cho::T.into());
    key_map.insert('c', Cho::C.into());
    key_map.insert('v', Cho::P.into());
    key_map.insert('b', Jung::YU.into());
    key_map.insert('n', Jung::U.into());
    key_map.insert('m', Jung::EU.into());

    key_map.insert('Q', Cho::BB.into());
    key_map.insert('W', Cho::JJ.into());
    key_map.insert('E', Cho::DD.into());
    key_map.insert('R', Cho::GG.into());
    key_map.insert('T', Cho::SS.into());
    key_map.insert('Y', Jung::YO.into());
    key_map.insert('U', Jung::YEO.into());
    key_map.insert('I', Jung::YA.into());
    key_map.insert('O', Jung::YAE.into());
    key_map.insert('P', Jung::YE.into());
    key_map.insert('A', Cho::M.into());
    key_map.insert('S', Cho::N.into());
    key_map.insert('D', Cho::E.into());
    key_map.insert('F', Cho::R.into());
    key_map.insert('G', Cho::H.into());
    key_map.insert('H', Jung::O.into());
    key_map.insert('J', Jung::EO.into());
    key_map.insert('K', Jung::A.into());
    key_map.insert('L', Jung::I.into());
    key_map.insert('Z', Cho::K.into());
    key_map.insert('X', Cho::T.into());
    key_map.insert('C', Cho::C.into());
    key_map.insert('V', Cho::P.into());
    key_map.insert('B', Jung::YU.into());
    key_map.insert('N', Jung::U.into());
    key_map.insert('M', Jung::EU.into());
    
    let stdin = io::stdin();
    let mut handle = stdin.lock();

    loop {
        let mut buffer = [0; 1];
        if handle.read(&mut buffer)? == 0 {
            break;
        }
        input_char = buffer[0] as char;
        if let Some(jamo) = key_map.get(&input_char) {
            if let Some(hangul) = hangul_builder.add_jamo(jamo.clone()) {
                print!("{}", hangul);
            }
        } else {
            if hangul_builder.is_build() {
                if let Some(hangul) = hangul_builder.force_build_hangul() {
                    print!("{}", hangul);
                }
            }
            print!("{}", input_char);
        }
        if input_char == '.' {
            break;
        }
    }

    Ok(())
}
