// use std::ptr; // 표준 라이브러리의 ptr 모듈을 가져옵니다.
// use x11::xlib::{
//     KeyPress, KeyPressMask, KeyReleaseMask, KeySym, XDefaultRootWindow, XLookupString, XNextEvent,
//     XOpenDisplay, XSelectInput,
// };

// fn is_hangul(c: char) -> bool {
//     ('\u{1100}'..='\u{11FF}').contains(&c) || ('\u{AC00}'..='\u{D7AF}').contains(&c)
// }

// fn detect_language(input: &str) -> &str {
//     if input.chars().all(|c| c.is_ascii_alphabetic()) {
//         "영어"
//     } else if input.chars().all(is_hangul) {
//         "한국어"
//     } else {
//         "알 수 없음"
//     }
// }

// fn correct_input(input: &str) -> String {
//     let korean_input_map = vec![
//         ("dkssudgktpdy", "안녕하세요"),
//     ];

//     for (wrong, correct) in korean_input_map {
//         if input == wrong {
//             return correct;
//         }
//     }

//     input
// }

// fn main() {
//     let display = unsafe { XOpenDisplay(ptr::null()) };
//     if display.is_null() {
//         eprintln!("X 디스플레이를 열 수 없습니다.");
//         return;
//     }
//     let root = unsafe { XDefaultRootWindow(display) };
//     unsafe {
//         XSelectInput(display, root, KeyPressMask | KeyReleaseMask);
//     }
//     loop {
//         let mut event = unsafe { std::mem::zeroed() };
//         unsafe { XNextEvent(display, &mut event) };
//         if unsafe { event.type_ } == KeyPress {
//             let mut buffer: [i8; 32] = [0; 32];
//             let mut keysym: KeySym = 0;
//             let mut key_event = unsafe { event.key };
//             unsafe {
//                 XLookupString(
//                     &mut key_event,
//                     buffer.as_mut_ptr(),
//                     32,
//                     &mut keysym,
//                     ptr::null_mut(),
//                 );
//             }
//             let input = unsafe { std::ffi::CStr::from_ptr(buffer.as_ptr()) }
//                 .to_str()
//                 .unwrap_or("")
//                 ;
//             let corrected_input = correct_input(&input);
//             let language = detect_language(&corrected_input);
//             println!("눌린 키: {:?}, 수정된 입력: {}, 언어: {}", input, corrected_input, language);
//         }
//     }
// }

pub mod hangul;

use crate::hangul::hangul::Hangul;
use crate::hangul::phoneme::{Onset, Nucleus, Coda};

fn main() {
    // Test case 1: Creating Hangul from phonemes
    let hangul1 = Hangul::from_phoneme(Onset::G, Nucleus::A, Coda::NG);
    println!("Hangul from phoneme (G, A, NG): {}", hangul1);

    // Test case 2: Creating Hangul from indices
    let hangul2 = Hangul::from_indices(0, 0, 0); // 가
    println!("Hangul from indices (0, 0, 0): {}", hangul2);

    // Test case 3: Creating Hangul from names
    let hangul3 = Hangul::from_names("G", "A", "NG");
    println!("Hangul from names (G, A, NG): {}", hangul3);

    // Test case 4: Creating Hangul from a syllable
    let hangul4 = Hangul::from_syllable('강');
    println!("Hangul from syllable '강': {}", hangul4);

    // Test case 5: Setting phonemes by indices
    let mut hangul5 = Hangul::new();
    hangul5.set_phoneme_by_indices(2, 0, 0); // 나
    println!("Hangul set by indices (2, 0, 0): {}", hangul5);

    // Test case 6: Setting phonemes by names
    let mut hangul6 = Hangul::new();
    hangul6.set_phoneme_by_names("N", "A", "NG"); // 낭
    println!("Hangul set by names (N, A, NG): {}", hangul6);

    // Test case 7: Getting Unicode values
    let hangul7 = Hangul::from_phoneme(Onset::D, Nucleus::U, Coda::L);
    println!("Unicode values of '둘': {:?}", hangul7.get_unicodes());

    // Test case 8: Getting the syllable
    let hangul8 = Hangul::from_phoneme(Onset::B, Nucleus::O, Coda::S);
    println!("Syllable of '봇': {}", hangul8.get_syllable());

    // Test case 9: Checking if filled
    let mut hangul9 = Hangul::new();
    hangul9.set_phoneme(Onset::G, Nucleus::A, Coda::NG);
    println!("Is filled only cho: {}", hangul9.is_filled_only_onset());
    println!("Is filled only jung: {}", hangul9.is_filled_only_nucleus());
    println!("Is filled only jong: {}", hangul9.is_filled_only_coda());
    println!("Is filled only jong: {}", hangul9);

    // Test case 10: Clear all phonemes
    let mut hangul10 = Hangul::from_phoneme(Onset::S, Nucleus::O, Coda::N);
    hangul10.clear();
    println!("Cleared Hangul: '{}'", hangul10);
}
