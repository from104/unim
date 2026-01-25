pub mod korean_to_keystrokes;
pub mod keyboard_map;
pub mod keystrokes_to_korean;

pub use keyboard_map::{Key, Keystroke, KeyboardMap};

const EN_QWERTY: &str = include_str!("keymap/en_qwerty.json");
const EN_DVORAK: &str = include_str!("keymap/en_dvorak.json");
const KO_2BULSTD: &str = include_str!("keymap/ko_2bulstd.json");
const KO_3BUL390: &str = include_str!("keymap/ko_3bul390.json");
const KO_3BUL391: &str = include_str!("keymap/ko_3bul391.json");

pub fn get_keymap_json(name: &str) -> &'static str {
    match name {
        "en_qwerty" => EN_QWERTY,
        "en_dvorak" => EN_DVORAK,
        "ko_2bulstd" | "2bul" => KO_2BULSTD,
        "ko_3bul390" | "390" | "3bul390" => KO_3BUL390,
        "ko_3bul391" | "391" | "3bul391" => KO_3BUL391,
        _ => KO_2BULSTD,
    }
}

use crate::korean::input_context::{ComposerType, KoreanInputContext};

/// Converts a string to a vector of `Keystroke`s based on a given keyboard layout.
pub fn string_to_keystrokes(s: &str, _layout: &str) -> Vec<Keystroke> {
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
    
    let mut context = KoreanInputContext::new(composer_type);

    let en_json = get_keymap_json(if layout.contains("dvorak") { "en_dvorak" } else { "en_qwerty" });
    let ko_json = get_keymap_json(layout);
    
    let keyboard_map = KeyboardMap::create_keyboard_map_from_str(en_json, ko_json, is_three_bul);
    
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
