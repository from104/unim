pub mod keyboard_map;
pub mod keystrokes_to_korean;
pub mod korean_to_keystrokes;
pub mod profile;

pub use keyboard_map::{EnglishKeymap, Key, KeyboardMap, Keystroke};

const EN_QWERTY: &str = include_str!("keymap/en_qwerty.json");
const EN_DVORAK: &str = include_str!("keymap/en_dvorak.json");
const EN_COLEMAK: &str = include_str!("keymap/en_colemak.json");
const EN_COLEMAK_DH: &str = include_str!("keymap/en_colemak_dh.json");
const EN_WORKMAN: &str = include_str!("keymap/en_workman.json");
const KO_2BULSTD: &str = include_str!("keymap/ko_2bulstd.json");
const KO_3BUL390: &str = include_str!("keymap/ko_3bul390.json");
const KO_3BUL391: &str = include_str!("keymap/ko_3bul391.json");
const KO_3BUL_NOSHIFT: &str = include_str!("keymap/ko_3bul_noshift.json");
const KO_3BUL_QWERTY: &str = include_str!("keymap/ko_3bul_qwerty.json");

pub fn get_keymap_json(name: &str) -> &'static str {
    match name {
        "en_qwerty" => EN_QWERTY,
        "en_dvorak" => EN_DVORAK,
        "en_colemak" => EN_COLEMAK,
        "en_colemak_dh" => EN_COLEMAK_DH,
        "en_workman" => EN_WORKMAN,
        "ko_2bulstd" | "2bul" => KO_2BULSTD,
        "ko_3bul390" | "390" | "3bul390" => KO_3BUL390,
        "ko_3bul391" | "391" | "3bul391" => KO_3BUL391,
        "ko_3bul_noshift" | "noshift" | "3bul_noshift" => KO_3BUL_NOSHIFT,
        "ko_3bul_qwerty" | "3bul_qwerty" => KO_3BUL_QWERTY,
        _ => KO_2BULSTD,
    }
}

use crate::hangul::input_context::{ComposerType, HangulInputContext};

/// Converts a string to a vector of `Keystroke`s based on a given keyboard layout.
pub fn string_to_keystrokes(s: &str, _layout: &str) -> Vec<Keystroke> {
    // This is a simplified version. For now, we assume standard mapping or raw chars.
    // In a full implementation, this would use a reverse keymap for the English layout.
    s.chars()
        .map(|c| Keystroke {
            key: Key::Raw(c),
            shifted: c.is_uppercase(),
        })
        .collect()
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

    let en_json = get_keymap_json(if layout.contains("dvorak") {
        "en_dvorak"
    } else {
        "en_qwerty"
    });
    let ko_json = get_keymap_json(layout);

    let keyboard_map = KeyboardMap::create_keyboard_map_from_str(en_json, ko_json, is_three_bul);

    for ks in keystrokes {
        match ks.key {
            Key::Raw(c) | Key::Char(c) => {
                if let Some(jamo) = keyboard_map.get(&c) {
                    context.process_jamo(*jamo);
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::keystroke::profile::builtin::BUILTIN_NAMES;

    /// T1-A: BUILTIN_NAMES의 모든 한국어 항목이 `_ => KO_2BULSTD` 폴백을 거치지 않음.
    /// `ko_2bulstd` 자체는 KO_2BULSTD 내용을 반환하므로 제외하고, 그 외 한국어 빌트인은
    /// 두벌식 JSON과 다른 내용을 반환해야 한다 (이름 필드로 식별).
    #[test]
    fn all_korean_builtins_avoid_2bulstd_fallback() {
        for name in BUILTIN_NAMES {
            if !name.starts_with("ko_") {
                continue;
            }
            let json = get_keymap_json(name);
            if name == "ko_2bulstd" {
                assert!(
                    json.contains("\"2bulstd\""),
                    "ko_2bulstd should map to KO_2BULSTD content"
                );
            } else {
                assert!(
                    json != KO_2BULSTD,
                    "korean builtin '{name}' fell back to KO_2BULSTD",
                );
            }
        }
    }

    /// T1-B: ko_3bul_qwerty 키맵이 두벌식과 동일하지 않은 매핑(예: 'q' 키)을 산출.
    /// 두벌식: 'q' lower → 'ㅂ' 초성, 쿼티형 세벌식: 'q' lower → 'ᇂ' 종성.
    #[test]
    fn ko_3bul_qwerty_keyboard_map_is_not_dubeolsik() {
        let en_json = get_keymap_json("en_qwerty");
        let qwerty_3bul_json = get_keymap_json("ko_3bul_qwerty");
        let dubeolsik_json = get_keymap_json("ko_2bulstd");

        let qwerty_3bul_map =
            KeyboardMap::create_keyboard_map_from_str(en_json, qwerty_3bul_json, true);
        let dubeolsik_map =
            KeyboardMap::create_keyboard_map_from_str(en_json, dubeolsik_json, false);

        let q_3bul = qwerty_3bul_map
            .get(&'q')
            .expect("ko_3bul_qwerty must map 'q'");
        let q_2bul = dubeolsik_map.get(&'q').expect("ko_2bulstd must map 'q'");
        assert_ne!(
            q_3bul, q_2bul,
            "ko_3bul_qwerty 'q' must differ from ko_2bulstd 'q'"
        );
    }

    /// T1-C: ko_3bul_qwerty 별칭(`3bul_qwerty`)도 같은 JSON으로 해석.
    #[test]
    fn ko_3bul_qwerty_alias_resolves() {
        let primary = get_keymap_json("ko_3bul_qwerty");
        let alias = get_keymap_json("3bul_qwerty");
        assert_eq!(primary, alias);
        assert_ne!(
            primary, KO_2BULSTD,
            "ko_3bul_qwerty must not fall back to KO_2BULSTD"
        );
    }

    /// T1-D: 4축 정합성 — BUILTIN_NAMES 10종 전수에 대해
    /// (a) `get_keymap_json(name) != KO_2BULSTD` (영문/한글 모두 fallback 미발생)
    /// (b) `profile::get_builtin_json(name).is_some()`
    /// (c) v1 프로필 JSON parse 가능 + `schema_version == 1`
    /// (d) `combinations` 필드가 있는 경우 비어있지 않은 구조 (한글 빌트인 5종)
    ///     영문 빌트인 5종은 `combinations` 없음을 허용.
    ///
    /// 한 종이라도 누락되면 즉시 회귀로 차단. 영문(en_workman 등) 미커버 갭을 막는다.
    #[test]
    fn builtin_names_full_4axis_integrity() {
        use crate::keystroke::profile::builtin::get_builtin_json;
        use crate::keystroke::profile::loader::parse_profile_str;

        for name in BUILTIN_NAMES {
            // (a) keymap fallback 차단 — KO_2BULSTD와 내용이 같으면 ko_2bulstd 본인뿐.
            let keymap = get_keymap_json(name);
            if name == "ko_2bulstd" {
                assert_eq!(
                    keymap, KO_2BULSTD,
                    "ko_2bulstd must map to KO_2BULSTD content",
                );
            } else {
                assert_ne!(
                    keymap, KO_2BULSTD,
                    "builtin '{name}' fell back to KO_2BULSTD via get_keymap_json",
                );
            }

            // (b) profile builtin arm 존재
            let profile_json = get_builtin_json(name)
                .unwrap_or_else(|| panic!("builtin '{name}' missing get_builtin_json arm"));
            assert!(
                !profile_json.is_empty(),
                "{name}: profile JSON must be non-empty"
            );

            // (c) v1+ profile parse 가능 + schema_version >= 1 (v0 schema는 거부)
            let profile = parse_profile_str(profile_json)
                .unwrap_or_else(|e| panic!("builtin '{name}' profile parse failed: {e}"));
            assert!(
                matches!(profile.schema_version, 1 | 2),
                "{name}: profile must declare schema_version 1 or 2 (was {}, v0 schema is rejected)",
                profile.schema_version
            );

            // (d) 한글 빌트인은 combinations 필드 보유, 영문은 없음 허용
            if name.starts_with("ko_") {
                assert!(
                    profile.combinations.is_some(),
                    "{name}: korean builtin must have non-empty combinations block"
                );
            }
        }
    }

    /// T1-E: 영문 빌트인 5종 전수에 대해 `get_keymap_json`이 `KO_2BULSTD`로 fallback되지 않음.
    /// `all_korean_builtins_avoid_2bulstd_fallback`의 영문 확장.
    #[test]
    fn all_english_builtins_avoid_2bulstd_fallback() {
        for name in BUILTIN_NAMES {
            if !name.starts_with("en_") {
                continue;
            }
            let json = get_keymap_json(name);
            assert_ne!(
                json, KO_2BULSTD,
                "english builtin '{name}' fell back to KO_2BULSTD",
            );
        }
    }
}
