//! 내장 자판 프로필 9종 — v0 JSON을 `include_str!`로 컴파일 시 임베드.
//!
//! Phase 6에서 `docs/plans/new_keymaps/*.json` 드래프트가 이 위치로 이관되어
//! v1 자기 완결 프로필로 대체된다. 그 시점까지는 기존 v0 포맷 그대로 로드되며,
//! `LayoutProfile::from_raw`의 v0 자동 승격 경로를 거친다.

/// 영어 QWERTY.
pub const EN_QWERTY: &str = include_str!("../keymap/en_qwerty.json");
/// 영어 Dvorak.
pub const EN_DVORAK: &str = include_str!("../keymap/en_dvorak.json");
/// 영어 Colemak.
pub const EN_COLEMAK: &str = include_str!("../keymap/en_colemak.json");
/// 영어 Colemak-DH.
pub const EN_COLEMAK_DH: &str = include_str!("../keymap/en_colemak_dh.json");
/// 영어 Workman.
pub const EN_WORKMAN: &str = include_str!("../keymap/en_workman.json");
/// 한국어 2벌식 표준.
pub const KO_2BULSTD: &str = include_str!("../keymap/ko_2bulstd.json");
/// 한국어 세벌식 390.
pub const KO_3BUL390: &str = include_str!("../keymap/ko_3bul390.json");
/// 한국어 세벌식 최종(391).
pub const KO_3BUL391: &str = include_str!("../keymap/ko_3bul391.json");
/// 한국어 세벌식 순아래(noshift).
pub const KO_3BUL_NOSHIFT: &str = include_str!("../keymap/ko_3bul_noshift.json");

/// 내장 프로필 정식 이름 목록 (로더 조회·CLI `layout list`용).
pub const BUILTIN_NAMES: [&str; 9] = [
    "en_qwerty",
    "en_dvorak",
    "en_colemak",
    "en_colemak_dh",
    "en_workman",
    "ko_2bulstd",
    "ko_3bul390",
    "ko_3bul391",
    "ko_3bul_noshift",
];

/// `name` 또는 `src/keystroke/mod.rs:17-30`의 별칭(`2bul`/`390`/`3bul391` 등)으로
/// 내장 프로필 JSON을 조회. 기존 `get_keymap_json` 별칭 규약을 그대로 따라가
/// Phase 2에서 로더가 교체 투입될 때 호출 호환을 유지한다.
pub fn get_builtin_json(name: &str) -> Option<&'static str> {
    match name {
        "en_qwerty" | "qwerty" => Some(EN_QWERTY),
        "en_dvorak" | "dvorak" => Some(EN_DVORAK),
        "en_colemak" | "colemak" => Some(EN_COLEMAK),
        "en_colemak_dh" | "colemak_dh" => Some(EN_COLEMAK_DH),
        "en_workman" | "workman" => Some(EN_WORKMAN),
        "ko_2bulstd" | "2bulstd" | "2bul" => Some(KO_2BULSTD),
        "ko_3bul390" | "3bul390" | "390" => Some(KO_3BUL390),
        "ko_3bul391" | "3bul391" | "391" => Some(KO_3BUL391),
        "ko_3bul_noshift" | "3bul_noshift" | "noshift" => Some(KO_3BUL_NOSHIFT),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_builtin_names_resolve() {
        for name in BUILTIN_NAMES {
            assert!(
                get_builtin_json(name).is_some(),
                "builtin name {name} should resolve"
            );
        }
    }

    #[test]
    fn aliases_resolve_to_same_content() {
        assert_eq!(get_builtin_json("2bul"), get_builtin_json("ko_2bulstd"));
        assert_eq!(get_builtin_json("390"), get_builtin_json("ko_3bul390"));
        assert_eq!(get_builtin_json("noshift"), get_builtin_json("ko_3bul_noshift"));
    }

    #[test]
    fn unknown_name_returns_none() {
        assert!(get_builtin_json("nonexistent").is_none());
    }

    #[test]
    fn all_builtin_json_non_empty() {
        for name in BUILTIN_NAMES {
            let json = get_builtin_json(name).unwrap();
            assert!(!json.is_empty(), "{name} json should be non-empty");
            assert!(json.contains("\"layout\""), "{name} must have layout");
        }
    }
}
