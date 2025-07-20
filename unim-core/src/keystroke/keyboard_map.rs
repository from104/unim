use serde::Deserialize;
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

#[derive(Debug, Clone, PartialEq, Eq, Hash, Deserialize)]
pub enum Key {
    Char(char),
    Ctrl(char),
    Raw(char),
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Deserialize)]
pub struct Keystroke {
    pub key: Key,
    pub shifted: bool,
}

#[derive(Debug, Deserialize)]
struct KeymapLayout {
    lower: HashMap<String, Vec<String>>,
    upper: HashMap<String, Vec<String>>,
}

#[derive(Debug, Deserialize)]
struct KeymapFile {
    layout: KeymapLayout,
}

fn get_keymap_path(name: &str) -> PathBuf {
    let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    path.push("src/keystroke/keymap");
    path.push(format!("{}.json", name));
    path
}

pub fn load_keymap(name: &str) -> Result<HashMap<String, Keystroke>, Box<dyn std::error::Error>> {
    let path = get_keymap_path(name);
    let data = fs::read_to_string(path)?;
    let keymap_file: KeymapFile = serde_json::from_str(&data)?;

    let mut map = HashMap::new();

    for (_row, keys) in keymap_file.layout.lower {
        for key_str in keys {
            map.insert(
                key_str.clone(),
                Keystroke {
                    key: Key::Char(key_str.chars().next().unwrap_or_default()),
                    shifted: false,
                },
            );
        }
    }
    for (_row, keys) in keymap_file.layout.upper {
        for key_str in keys {
            map.insert(
                key_str.clone(),
                Keystroke {
                    key: Key::Char(key_str.chars().next().unwrap_or_default()),
                    shifted: true,
                },
            );
        }
    }
    Ok(map)
}

pub fn load_keymap_rev(
    name: &str,
) -> Result<HashMap<Keystroke, String>, Box<dyn std::error::Error>> {
    let keymap = load_keymap(name)?;
    let mut rev_map = HashMap::new();
    for (s, ks) in keymap {
        rev_map.insert(ks, s);
    }
    Ok(rev_map)
}
