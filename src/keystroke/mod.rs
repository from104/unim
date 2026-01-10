pub mod hangul_to_keystrokes;
pub mod keyboard_map;
pub mod keystrokes_to_hangul;

pub use keyboard_map::{Key, Keystroke, KeyboardMap};

use crate::hangul::input_context::{ComposerType, HangulInputContext};

/// Converts a string to a vector of `Keystroke`s based on a given keyboard layout.
pub fn string_to_keystrokes(s: &str, layout: &str) -> Vec<Keystroke> {
    // This is a simplified version. For now, we assume standard mapping or raw chars.
    // In a full implementation, this would use a reverse keymap for the English layout.
    s.chars().map(|c| Keystroke {
        key: Key::Raw(c),
        shifted: c.is_uppercase(),
    }).collect()
}

/// Converts a vector of `Keystroke`s to a string based on a given keyboard layout.
pub fn keystrokes_to_string(keystrokes: &[Keystroke], layout: &str) -> String {
    let is_three_bul = layout.starts_with("ko_3");
    let composer_type = if is_three_bul {
        ComposerType::ThreeBul
    } else {
        ComposerType::TwoBul
    };
    
    let mut context = HangulInputContext::new(composer_type);
    
    // Keymap files are relative to root
    let en_keymap = "src/keystroke/keymap/en_qwerty.json";
    let ko_keymap = format!("src/keystroke/keymap/{}.json", layout);
    
    let keyboard_map = KeyboardMap::create_keyboard_map(en_keymap, &ko_keymap, is_three_bul);
    
    for ks in keystrokes {
        match ks.key {
            Key::Raw(c) | Key::Char(c) => {
                if let Some(jamo) = keyboard_map.get(&c) {
                    context.process_jamo(jamo.clone());
                } else {
                    context.append_to_committed(c);
                }
            }
            _ => {}
        }
    }
    
    context.commit();
    context.get_committed().to_string()
}
