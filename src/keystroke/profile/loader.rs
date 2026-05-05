//! 프로필 로더 — JSON 문자열 또는 내장 이름에서 `LayoutProfile` 생성.
//!
//! # 범위
//! - `parse_profile_str(json)` — serde 역직렬화 + v1/v3 마커 검증 + 정규화.
//! - `load_builtin_profile(name)` — 내장 종 조회 + 파싱.
//!
//! 0.2.0부터 v0(legacy) 스키마는 거부된다. v1 마커
//! (`schema_version`/`metadata`/`inherits`/`combinations`/`rule_sets`/`active_rule_sets`)
//! 중 하나라도 존재해야 v1로 인정한다. 사용자가 작성한 v0 JSON은
//! `LoadError::UnsupportedSchema`로 명확히 거부된다.
//!
//! v3 마커(`moachigi`/`jamo_symbol_map`) 또는 `schema_version == 3`이면
//! v3 검증을 추가로 수행하고 `LayoutProfile::from_raw_v3`로 정규화한다.

use std::fmt;

use crate::unim_log;

use super::builtin;
use super::schema::{LayoutProfile, LayoutTypeV3, RawProfile};

/// 프로필 로드 실패 원인.
#[derive(Debug)]
pub enum LoadError {
    /// 내장·사용자 네임스페이스 어디에도 해당 이름이 없음.
    NotFound(String),
    /// JSON 파싱 실패. serde 오류 메시지 포함.
    Parse(serde_json::Error),
    /// v0(0.1.x) 포맷 — v1 마커가 모두 부재. 0.2.0부터 거부.
    /// `name` 필드는 사용자 가시성을 위해 raw JSON에서 추출.
    UnsupportedSchema { profile_name: String },
    /// v3 — `type == "anmatae"` / `"moachigi_3bul"` 인데 `moachigi` 블록 없음.
    MissingMoachigiBlock { profile_name: String },
}

impl fmt::Display for LoadError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            LoadError::NotFound(name) => write!(f, "layout profile not found: {name}"),
            LoadError::Parse(e) => write!(f, "failed to parse layout profile JSON: {e}"),
            LoadError::UnsupportedSchema { profile_name } => write!(
                f,
                "layout profile '{profile_name}' uses the legacy v0 schema, which is no longer \
                 supported in 0.2.0+. Convert to v1 schema (see docs/dev/plans/LAYOUT_PROFILE_V1.md)."
            ),
            LoadError::MissingMoachigiBlock { profile_name } => write!(
                f,
                "layout profile '{profile_name}' has type 'anmatae' or 'moachigi_3bul' but is \
                 missing the required 'moachigi' block (v3 schema)."
            ),
        }
    }
}

impl std::error::Error for LoadError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            LoadError::NotFound(_) => None,
            LoadError::Parse(e) => Some(e),
            LoadError::UnsupportedSchema { .. } => None,
            LoadError::MissingMoachigiBlock { .. } => None,
        }
    }
}

impl From<serde_json::Error> for LoadError {
    fn from(e: serde_json::Error) -> Self {
        LoadError::Parse(e)
    }
}

/// v3 프로필 전용 추가 검증.
fn validate_v3(raw: &RawProfile) -> Result<(), LoadError> {
    let lt = LayoutTypeV3::from_str(&raw.r#type);
    if matches!(lt, LayoutTypeV3::Moachigi3Bul) && raw.moachigi.is_none() {
        return Err(LoadError::MissingMoachigiBlock {
            profile_name: raw.name.clone(),
        });
    }
    // jamo_symbol_map 키와 layout 키 충돌 경고 (에러 아님).
    if let Some(sym_map) = &raw.jamo_symbol_map {
        let layout_keys: std::collections::HashSet<&str> = raw
            .layout
            .lower
            .row1
            .iter()
            .chain(raw.layout.lower.row2.iter())
            .chain(raw.layout.lower.row3.iter())
            .chain(raw.layout.lower.row4.iter())
            .chain(raw.layout.upper.row1.iter())
            .chain(raw.layout.upper.row2.iter())
            .chain(raw.layout.upper.row3.iter())
            .chain(raw.layout.upper.row4.iter())
            .map(|s| s.as_str())
            .collect();
        for key in sym_map.keys() {
            if layout_keys.contains(key.as_str()) {
                unim_log!(
                    "LOADER",
                    "v3 jamo_symbol_map key '{}' collides with layout key in '{}' \
                     — symbol takes priority",
                    key,
                    raw.name
                );
            }
        }
    }
    Ok(())
}

/// JSON 문자열을 파싱해 `LayoutProfile`로 변환.
///
/// v1 마커가 하나도 없으면 `LoadError::UnsupportedSchema`로 거부한다 (v0 거부).
/// v3 마커가 존재하면 추가 검증 후 v3 경로로 파싱한다.
pub fn parse_profile_str(json: &str) -> Result<LayoutProfile, LoadError> {
    let raw: RawProfile = serde_json::from_str(json)?;
    if !raw.has_v1_markers() {
        let profile_name = raw.name.clone();
        unim_log!(
            "LOADER",
            "WARNING: layout profile '{}' uses the legacy v0 schema and will be rejected. \
             Convert to v1 (see docs/dev/plans/LAYOUT_PROFILE_V1.md).",
            profile_name
        );
        return Err(LoadError::UnsupportedSchema { profile_name });
    }
    if raw.is_v3() {
        validate_v3(&raw)?;
        Ok(LayoutProfile::from_raw_v3(raw))
    } else {
        Ok(LayoutProfile::from_raw(raw))
    }
}

/// 내장 프로필을 이름으로 로드. `name`은 정식 이름(`en_qwerty`) 또는 별칭(`qwerty`) 허용.
pub fn load_builtin_profile(name: &str) -> Result<LayoutProfile, LoadError> {
    let json =
        builtin::get_builtin_json(name).ok_or_else(|| LoadError::NotFound(name.to_string()))?;
    parse_profile_str(json)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::keystroke::profile::builtin::BUILTIN_NAMES;

    #[test]
    fn loads_all_builtin_profiles() {
        // 모든 내장 프로필은 v1, v2, 또는 v3 포맷.
        for name in BUILTIN_NAMES {
            let profile = load_builtin_profile(name)
                .unwrap_or_else(|e| panic!("failed to load builtin {name}: {e}"));
            assert!(
                matches!(profile.schema_version, 1 | 2 | 3),
                "{name}: 내장 프로필은 v1/v2/v3 (was {})",
                profile.schema_version
            );
            assert!(!profile.name.is_empty(), "{name}: name 필드 있어야 함");

            // 한국어 내장은 자기 완결 combinations 필수. 영문 계열은 jamo 조합 없음.
            if profile.language == "korean" {
                assert!(
                    profile.combinations.is_some(),
                    "{name}: 한국어 내장은 combinations 필요"
                );
            } else {
                assert!(
                    profile.combinations.is_none(),
                    "{name}: 영문 내장은 combinations 없음"
                );
            }
        }
    }

    #[test]
    fn builtin_aliases_produce_identical_profile_content() {
        let via_alias = load_builtin_profile("2bul").unwrap();
        let via_fullname = load_builtin_profile("ko_2bulstd").unwrap();
        assert_eq!(via_alias.name, via_fullname.name);
        assert_eq!(via_alias.layout_type, via_fullname.layout_type);
        assert_eq!(via_alias.language, via_fullname.language);
    }

    #[test]
    fn parses_v1_inline_json() {
        let json = r#"{
            "schema_version": 1,
            "language": "korean",
            "name": "tiny",
            "type": "3bul",
            "layout": {
                "upper": {"1st":[],"2nd":[],"3rd":[],"4th":[]},
                "lower": {"1st":[],"2nd":[],"3rd":[],"4th":[]}
            },
            "combinations": {
                "cho": [{"first":"ㄱ","second":"ㄱ","result":"ㄲ"}],
                "jung": [],
                "jong": []
            }
        }"#;
        let profile = parse_profile_str(json).unwrap();
        assert_eq!(profile.schema_version, 1);
        assert!(profile.combinations.is_some());
        // v3 필드는 기본값
        assert!(profile.moachigi.is_none());
        assert!(profile.jamo_symbol_map.is_empty());
        assert_eq!(profile.layout_type_v3, LayoutTypeV3::Jamo);
    }

    #[test]
    fn unknown_builtin_is_not_found_error() {
        let err = load_builtin_profile("no_such_profile").unwrap_err();
        assert!(matches!(err, LoadError::NotFound(_)));
    }

    #[test]
    fn malformed_json_is_parse_error() {
        let err = parse_profile_str("{ not valid json").unwrap_err();
        assert!(matches!(err, LoadError::Parse(_)));
    }

    /// 0.2.0 신규 — v0 포맷(=v1 마커 모두 부재) JSON은 명시적으로 거부된다.
    #[test]
    fn legacy_v0_json_is_rejected_as_unsupported_schema() {
        let v0_json = r##"{
            "language": "korean",
            "name": "legacy_user_layout",
            "type": "2bul",
            "layout": {
                "upper": {"1st":[],"2nd":[],"3rd":[],"4th":[]},
                "lower": {"1st":[],"2nd":[],"3rd":[],"4th":[]}
            }
        }"##;
        let err = parse_profile_str(v0_json).unwrap_err();
        match err {
            LoadError::UnsupportedSchema { profile_name } => {
                assert_eq!(profile_name, "legacy_user_layout");
            }
            other => panic!("expected UnsupportedSchema, got {other:?}"),
        }
    }

    // ======================================================================
    // v3 테스트
    // ======================================================================

    /// sv3-anmatae: v3 layout_type=anmatae + moachigi 블록 → v3 경로 파싱.
    #[test]
    fn sv3_anmatae_parses_correctly() {
        let json = r#"{
            "schema_version": 3,
            "language": "korean",
            "name": "anmatae_test",
            "type": "anmatae",
            "layout": {
                "upper": {"1st":[],"2nd":[],"3rd":[],"4th":[]},
                "lower": {"1st":[],"2nd":[],"3rd":[],"4th":[]}
            },
            "combinations": {"cho":[],"jung":[],"jong":[]},
            "moachigi": {
                "syllable_boundary": "region_filled",
                "jong_unordered": true,
                "jung_unordered": false
            }
        }"#;
        let profile = parse_profile_str(json).unwrap();
        assert_eq!(profile.schema_version, 3);
        assert_eq!(profile.layout_type, "anmatae");
        assert_eq!(profile.layout_type_v3, LayoutTypeV3::Moachigi3Bul);
        let m = profile.moachigi.as_ref().expect("moachigi should be Some");
        assert_eq!(m.syllable_boundary, crate::keystroke::profile::schema::SyllableBoundary::RegionFilled);
        assert!(m.jong_unordered);
        assert!(!m.jung_unordered);
        assert!(m.region_filled);
    }

    /// sv3-missing-moachigi: type=anmatae, moachigi 없음 → MissingMoachigiBlock.
    #[test]
    fn sv3_missing_moachigi_block_is_error() {
        let json = r#"{
            "schema_version": 3,
            "language": "korean",
            "name": "broken_anmatae",
            "type": "anmatae",
            "layout": {
                "upper": {"1st":[],"2nd":[],"3rd":[],"4th":[]},
                "lower": {"1st":[],"2nd":[],"3rd":[],"4th":[]}
            },
            "combinations": {"cho":[],"jung":[],"jong":[]}
        }"#;
        let err = parse_profile_str(json).unwrap_err();
        match err {
            LoadError::MissingMoachigiBlock { profile_name } => {
                assert_eq!(profile_name, "broken_anmatae");
            }
            other => panic!("expected MissingMoachigiBlock, got {other:?}"),
        }
    }

    /// sv3-moachigi-3bul: type=moachigi_3bul → LayoutTypeV3::Moachigi3Bul.
    #[test]
    fn sv3_moachigi_3bul_parses_correctly() {
        let json = r#"{
            "schema_version": 3,
            "language": "korean",
            "name": "3bul_moachigi_test",
            "type": "moachigi_3bul",
            "layout": {
                "upper": {"1st":[],"2nd":[],"3rd":[],"4th":[]},
                "lower": {"1st":[],"2nd":[],"3rd":[],"4th":[]}
            },
            "combinations": {"cho":[],"jung":[],"jong":[]},
            "moachigi": {
                "syllable_boundary": "strict",
                "jong_unordered": false,
                "jung_unordered": false
            }
        }"#;
        let profile = parse_profile_str(json).unwrap();
        assert_eq!(profile.layout_type_v3, LayoutTypeV3::Moachigi3Bul);
        let m = profile.moachigi.as_ref().unwrap();
        assert_eq!(m.syllable_boundary, crate::keystroke::profile::schema::SyllableBoundary::Strict);
        assert!(!m.jong_unordered);
    }

    /// sv3-active-empty: active_rule_sets=[] → base moachigi 그대로 (overrides 없음).
    #[test]
    fn sv3_active_empty_rule_sets_uses_base_moachigi() {
        let json = r#"{
            "schema_version": 3,
            "language": "korean",
            "name": "anmatae_no_rules",
            "type": "anmatae",
            "layout": {
                "upper": {"1st":[],"2nd":[],"3rd":[],"4th":[]},
                "lower": {"1st":[],"2nd":[],"3rd":[],"4th":[]}
            },
            "combinations": {"cho":[],"jung":[],"jong":[]},
            "moachigi": {
                "syllable_boundary": "region_filled",
                "jong_unordered": false
            },
            "active_rule_sets": []
        }"#;
        let profile = parse_profile_str(json).unwrap();
        let merged = profile.merged_moachigi().expect("merged_moachigi should be Some");
        // active_rule_sets==[] → override 없음 → base 값 그대로
        assert!(!merged.jong_unordered);
        assert!(merged.region_filled);
    }

    /// sv3-jong-unordered: moachigi_jong_unordered 활성 → jong_unordered=true.
    #[test]
    fn sv3_rule_set_override_jong_unordered() {
        let json = r#"{
            "schema_version": 3,
            "language": "korean",
            "name": "anmatae_jong",
            "type": "anmatae",
            "layout": {
                "upper": {"1st":[],"2nd":[],"3rd":[],"4th":[]},
                "lower": {"1st":[],"2nd":[],"3rd":[],"4th":[]}
            },
            "combinations": {"cho":[],"jung":[],"jong":[]},
            "moachigi": {
                "syllable_boundary": "region_filled",
                "jong_unordered": false
            },
            "rule_sets": {
                "moachigi_jong_unordered": {
                    "active": true,
                    "moachigi_overrides": {
                        "jong_unordered": true
                    }
                }
            }
        }"#;
        let profile = parse_profile_str(json).unwrap();
        let merged = profile.merged_moachigi().expect("merged_moachigi should be Some");
        assert!(merged.jong_unordered, "rule_set override should activate jong_unordered");
        assert!(merged.region_filled, "base region_filled should be preserved");
    }

    /// sv3-rule-merge: 두 moachigi rule_set 동시 활성 → 나중 rule_set 우선.
    #[test]
    fn sv3_two_rule_sets_layer_merge_later_wins() {
        let json = r#"{
            "schema_version": 3,
            "language": "korean",
            "name": "anmatae_merge",
            "type": "anmatae",
            "layout": {
                "upper": {"1st":[],"2nd":[],"3rd":[],"4th":[]},
                "lower": {"1st":[],"2nd":[],"3rd":[],"4th":[]}
            },
            "combinations": {"cho":[],"jung":[],"jong":[]},
            "moachigi": {
                "syllable_boundary": "strict",
                "jong_unordered": false
            },
            "rule_sets": {
                "set_a": {
                    "active": true,
                    "moachigi_overrides": {
                        "jong_unordered": true,
                        "syllable_boundary": "region_filled"
                    }
                },
                "set_b": {
                    "active": true,
                    "moachigi_overrides": {
                        "jong_unordered": false
                    }
                }
            },
            "active_rule_sets": ["set_a", "set_b"]
        }"#;
        let profile = parse_profile_str(json).unwrap();
        let merged = profile.merged_moachigi().unwrap();
        // set_b 나중 → jong_unordered = false (set_b가 덮음)
        assert!(!merged.jong_unordered, "set_b (later) should override set_a jong_unordered");
        // syllable_boundary는 set_b가 건드리지 않으므로 set_a 값 유지
        assert_eq!(
            merged.syllable_boundary,
            crate::keystroke::profile::schema::SyllableBoundary::RegionFilled
        );
    }

    /// sv3-symbol-emit: jamo_symbol_map 파싱.
    #[test]
    fn sv3_jamo_symbol_map_parses() {
        let json = r#"{
            "schema_version": 3,
            "language": "korean",
            "name": "anmatae_sym",
            "type": "anmatae",
            "layout": {
                "upper": {"1st":[],"2nd":[],"3rd":[],"4th":[]},
                "lower": {"1st":[],"2nd":[],"3rd":[],"4th":[]}
            },
            "combinations": {"cho":[],"jung":[],"jong":[]},
            "moachigi": {
                "syllable_boundary": "region_filled",
                "jong_unordered": false
            },
            "jamo_symbol_map": {
                "ᄛ": {"emit_char": "\"", "comment": "여는 인용부호"}
            }
        }"#;
        let profile = parse_profile_str(json).unwrap();
        assert_eq!(profile.jamo_symbol_map.len(), 1);
        let sym = profile.jamo_symbol_map.get("ᄛ").expect("symbol key should exist");
        assert_eq!(sym.emit_char, '"');
        assert_eq!(sym.comment.as_deref(), Some("여는 인용부호"));
    }

    /// sv3-implicit-marker: schema_version 없이 moachigi 블록만으로 v3 인식.
    #[test]
    fn sv3_implicit_marker_via_moachigi() {
        let json = r#"{
            "language": "korean",
            "name": "implicit_v3",
            "type": "anmatae",
            "layout": {
                "upper": {"1st":[],"2nd":[],"3rd":[],"4th":[]},
                "lower": {"1st":[],"2nd":[],"3rd":[],"4th":[]}
            },
            "combinations": {"cho":[],"jung":[],"jong":[]},
            "moachigi": {
                "syllable_boundary": "region_filled",
                "jong_unordered": true
            }
        }"#;
        let profile = parse_profile_str(json).unwrap();
        assert_eq!(profile.layout_type_v3, LayoutTypeV3::Moachigi3Bul);
        assert!(profile.moachigi.is_some());
    }

    /// R3a: ko_3bul390 로드 → v1/v2 경로, moachigi None, 기존 동작.
    #[test]
    fn r3a_ko_3bul390_is_v1_path_moachigi_none() {
        let profile = load_builtin_profile("ko_3bul390").unwrap();
        assert!(
            matches!(profile.schema_version, 1 | 2),
            "ko_3bul390 must be v1/v2"
        );
        assert!(profile.moachigi.is_none(), "ko_3bul390 must have no moachigi");
        assert!(profile.jamo_symbol_map.is_empty());
        assert_eq!(profile.layout_type_v3, LayoutTypeV3::Jamo);
    }

    /// R3b: ko_3bul391 로드 → v1/v2 경로.
    #[test]
    fn r3b_ko_3bul391_is_v1_path() {
        let profile = load_builtin_profile("ko_3bul391").unwrap();
        assert!(matches!(profile.schema_version, 1 | 2));
        assert!(profile.moachigi.is_none());
    }

    /// R3c: ko_3bul_noshift 로드 → v1/v2 경로.
    #[test]
    fn r3c_ko_3bul_noshift_is_v1_path() {
        let profile = load_builtin_profile("ko_3bul_noshift").unwrap();
        assert!(matches!(profile.schema_version, 1 | 2));
        assert!(profile.moachigi.is_none());
    }

    // ======================================================================
    // kr1~kr7: KeyToRegionMap 로더 테스트 (Phase 3 Final)
    // ======================================================================

    /// kr1: `_region_map` 명시된 자판 → cho/jung/jong 키가 정확히 Region으로 매핑.
    #[test]
    fn kr1_region_map_from_explicit_json() {
        use crate::keystroke::profile::schema::Region;
        let json = r#"{
            "schema_version": 3,
            "language": "korean",
            "name": "kr1_test",
            "type": "moachigi_3bul",
            "layout": {"upper": {}, "lower": {}},
            "_region_map": {
                "cho":  {"f": "ᄀ", "a": "ᄇ"},
                "jung": {"j": "ᅡ", "k": "ᅵ"},
                "jong": {"v": "ᆨ", "m": "ᆫ"}
            },
            "combinations": {"cho":[],"jung":[],"jong":[]},
            "moachigi": {"syllable_boundary": "region_filled", "jong_unordered": false}
        }"#;
        let profile = parse_profile_str(json).unwrap();
        let map = &profile.key_to_region_map;
        assert_eq!(map.len(), 6, "6개 키 전부 매핑");
        assert_eq!(map.get("f"), Some(&Region::Cho));
        assert_eq!(map.get("a"), Some(&Region::Cho));
        assert_eq!(map.get("j"), Some(&Region::Jung));
        assert_eq!(map.get("k"), Some(&Region::Jung));
        assert_eq!(map.get("v"), Some(&Region::Jong));
        assert_eq!(map.get("m"), Some(&Region::Jong));
    }

    /// kr2: `_region_map` 없을 때 layout.lower 자모에서 자동 추론.
    #[test]
    fn kr2_region_map_auto_infer_from_layout() {
        use crate::keystroke::profile::schema::Region;
        // layout.lower.row2 = ["ᄀ", "ᅡ", "ᆨ"] (cho, jung, jong 각 1개)
        let json = r#"{
            "schema_version": 3,
            "language": "korean",
            "name": "kr2_test",
            "type": "moachigi_3bul",
            "layout": {
                "upper": {},
                "lower": {
                    "2nd": ["ᄀ", "ᅡ", "ᆨ"]
                }
            },
            "combinations": {"cho":[],"jung":[],"jong":[]},
            "moachigi": {"syllable_boundary": "region_filled", "jong_unordered": false}
        }"#;
        let profile = parse_profile_str(json).unwrap();
        let map = &profile.key_to_region_map;
        // 자모 리터럴이 키로 들어감 (안마태와 달리 레이아웃 셀 = 자모 문자열)
        assert_eq!(map.get("ᄀ"), Some(&Region::Cho),  "ᄀ → Cho (U+1100)");
        assert_eq!(map.get("ᅡ"), Some(&Region::Jung), "ᅡ → Jung (U+1161)");
        assert_eq!(map.get("ᆨ"), Some(&Region::Jong), "ᆨ → Jong (U+11A8)");
    }

    /// kr3: jamo_symbol_map 키 입력 → 즉시 commit 경로 (composer 큐 무영향).
    /// 로더 레벨에서 jamo_symbol_map이 정확히 6개 파싱됨을 검증.
    #[test]
    fn kr3_jamo_symbol_map_parses_6_entries() {
        let profile = load_builtin_profile("ko_anmatae").unwrap();
        let sym = &profile.jamo_symbol_map;
        assert_eq!(sym.len(), 6, "anmatae jamo_symbol_map = 6개 (B/G/J/N/T/W)");
        // B → " (U+201C)
        assert_eq!(sym.get("B").map(|s| s.emit_char), Some('\u{201C}'), "B → 여는 큰따옴표");
        // G → " (U+201D)
        assert_eq!(sym.get("G").map(|s| s.emit_char), Some('\u{201D}'), "G → 닫는 큰따옴표");
        // J → · (U+00B7)
        assert_eq!(sym.get("J").map(|s| s.emit_char), Some('\u{00B7}'), "J → 가운뎃점");
        // N → ' (U+2018)
        assert_eq!(sym.get("N").map(|s| s.emit_char), Some('\u{2018}'), "N → 여는 작은따옴표");
        // T → … (U+2026)
        assert_eq!(sym.get("T").map(|s| s.emit_char), Some('\u{2026}'), "T → 줄임표");
        // W → ' (U+2019)
        assert_eq!(sym.get("W").map(|s| s.emit_char), Some('\u{2019}'), "W → 닫는 작은따옴표");
    }

    /// kr4: anmatae.json 로드 시 key_to_region_map에 31개 키 (cho:10 + jung:10 + jong:11).
    #[test]
    fn kr4_anmatae_json_region_map_key_count() {
        use crate::keystroke::profile::schema::Region;
        let profile = load_builtin_profile("ko_anmatae").unwrap();
        let map = &profile.key_to_region_map;
        // cho: a d e f g q r s t w = 10
        // jung: h i j k l o p ; u y = 10
        // jong: , . / ? b c m n v x z = 11
        let cho_count = map.values().filter(|&&r| r == Region::Cho).count();
        let jung_count = map.values().filter(|&&r| r == Region::Jung).count();
        let jong_count = map.values().filter(|&&r| r == Region::Jong).count();
        assert_eq!(cho_count, 10, "cho 10개");
        assert_eq!(jung_count, 10, "jung 10개");
        assert_eq!(jong_count, 11, "jong 11개");
        assert_eq!(map.len(), 31, "total 31개");
        // 대표 키 검증
        assert_eq!(map.get("f"), Some(&Region::Cho),  "f → Cho (ᄀ)");
        assert_eq!(map.get("j"), Some(&Region::Jung), "j → Jung (ᅡ)");
        assert_eq!(map.get(","), Some(&Region::Jong), ", → Jong (ᆷ)");
    }

    /// kr5: v1 자판(ko_3bul390) 로드 시 key_to_region_map 비어있음.
    #[test]
    fn kr5_v1_profile_key_to_region_empty() {
        let profile = load_builtin_profile("ko_3bul390").unwrap();
        assert!(
            profile.key_to_region_map.is_empty(),
            "v1 자판은 key_to_region_map 없음 (Region 개념 미사용)"
        );
    }

    /// kr6: anmatae.json + active_rule_sets=[] → base moachigi 그대로.
    /// effective_moachigi = base (jong_unordered=true, region_filled=true).
    #[test]
    fn kr6_anmatae_active_empty_uses_base_moachigi() {
        let json = r#"{
            "schema_version": 3,
            "language": "korean",
            "name": "kr6_test",
            "type": "moachigi_3bul",
            "layout": {"upper": {}, "lower": {}},
            "combinations": {"cho":[],"jung":[],"jong":[]},
            "moachigi": {
                "syllable_boundary": "region_filled",
                "jong_unordered": true,
                "jung_unordered": false,
                "region_filled": true
            },
            "active_rule_sets": []
        }"#;
        let profile = parse_profile_str(json).unwrap();
        let merged = profile.merged_moachigi().expect("merged_moachigi Some");
        assert!(merged.jong_unordered, "base jong_unordered=true 유지");
        assert!(merged.region_filled, "base region_filled=true 유지");
        assert!(!merged.jung_unordered, "base jung_unordered=false 유지");
    }

    /// kr7: anmatae.json + active_rule_sets=[moachigi_strict] → jong_unordered=false.
    #[test]
    fn kr7_anmatae_strict_rule_set_disables_jong_unordered() {
        let profile = load_builtin_profile("ko_anmatae").unwrap();
        // ko_anmatae의 active_rule_sets = ["moachigi_jong_unordered"] (기본)
        // 이를 moachigi_strict로 교체한 JSON으로 테스트
        let json = r#"{
            "schema_version": 3,
            "language": "korean",
            "name": "kr7_test",
            "type": "moachigi_3bul",
            "layout": {"upper": {}, "lower": {}},
            "combinations": {"cho":[],"jung":[],"jong":[]},
            "moachigi": {
                "syllable_boundary": "region_filled",
                "jong_unordered": true,
                "region_filled": true
            },
            "rule_sets": {
                "moachigi_strict": {
                    "active": true,
                    "moachigi_overrides": {
                        "syllable_boundary": "strict",
                        "jong_unordered": false
                    }
                }
            },
            "active_rule_sets": ["moachigi_strict"]
        }"#;
        let profile = parse_profile_str(json).unwrap();
        let merged = profile.merged_moachigi().expect("merged_moachigi Some");
        assert!(!merged.jong_unordered, "strict 룰셋 → jong_unordered=false");
        assert_eq!(
            merged.syllable_boundary,
            crate::keystroke::profile::schema::SyllableBoundary::Strict,
            "strict 룰셋 → syllable_boundary=Strict"
        );
        // key_to_region_map은 rule_set과 무관 (로더 시점에 고정)
        assert!(
            profile.key_to_region_map.is_empty(),
            "kr7 test profile has no _region_map"
        );
    }
}
