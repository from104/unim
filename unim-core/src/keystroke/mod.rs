pub mod hangul_to_keystrokes;
pub mod keyboard_map;
pub mod keystrokes_to_hangul;

use keyboard_map::{Key, Keystroke};

/// Converts a string to a vector of `Keystroke`s based on a given keyboard layout.
pub fn string_to_keystrokes(s: &str, layout: &str) -> Vec<Keystroke> {
    let keymap = keyboard_map::load_keymap(layout).unwrap_or_default();
    let mut keystrokes = Vec::new();
    for ch in s.chars() {
        if let Some(key) = keymap.get(&ch.to_string()) {
            keystrokes.push(key.clone());
        } else {
            // If the character is not in the keymap, treat it as a direct input.
            keystrokes.push(Keystroke {
                key: Key::Raw(ch),
                shifted: false,
            });
        }
    }
    keystrokes
}

/// Converts a vector of `Keystroke`s to a string based on a given keyboard layout.
pub fn keystrokes_to_string(keystrokes: &[Keystroke], layout: &str) -> String {
    if layout.starts_with("ko_") {
        keystrokes_to_hangul::keystrokes_to_hangul_string(keystrokes, layout)
    } else {
        // For non-Korean layouts, just map the keystrokes back to characters.
        let keymap_rev = keyboard_map::load_keymap_rev(layout).unwrap_or_default();
        let mut result = String::new();
        for keystroke in keystrokes {
            if let Some(ch) = keymap_rev.get(keystroke) {
                result.push_str(ch);
            } else if let Key::Raw(ch) = &keystroke.key {
                result.push(*ch);
            }
        }
        result
    }
}
