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
//! v3 마커(`supports_moachigi`/`bidirectional_combine`/`chord_window_ms>0`) 또는
//! `schema_version == 3`이면 v3 검증 후 `LayoutProfile::from_raw_v3`로 정규화한다.
//!
//! Phase 3-rework2: `moachigi` 블록, `jamo_symbol_map`, `LayoutTypeV3`, `MoachigiOverride` 폐기.
//! v3 인식은 `supports_moachigi`/`bidirectional_combine`/`chord_window_ms` 3 최상위 필드로.

use std::fmt;

use crate::unim_log;

use super::builtin;
use super::schema::{LayoutProfile, RawProfile};

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
    /// 옛한글(고어) 자모 코드포인트가 layout/combinations에 발견됨.
    ///
    /// v3 스키마는 현대 한글 자모만 지원. 옛한글 처리는 별도 v4 설계 예정.
    ArchaicJamoNotSupported { codepoint: u32, location: String },
}

impl fmt::Display for LoadError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            LoadError::NotFound(name) => write!(f, "layout profile not found: {name}"),
            LoadError::Parse(e) => write!(f, "failed to parse layout profile JSON: {e}"),
            LoadError::UnsupportedSchema { profile_name } => write!(
                f,
                "layout profile '{profile_name}' uses the legacy v0 schema, which is no longer \
                 supported in 0.2.0+. Convert to v1 schema (see docs/archive/plans/LAYOUT_PROFILE_V1.md)."
            ),
            LoadError::ArchaicJamoNotSupported { codepoint, location } => write!(
                f,
                "archaic (old Korean) jamo U+{codepoint:04X} found at '{location}' — \
                 v3 schema supports modern Hangul jamo only. \
                 Archaic jamo support is planned for a future schema version."
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
            LoadError::ArchaicJamoNotSupported { .. } => None,
        }
    }
}

impl From<serde_json::Error> for LoadError {
    fn from(e: serde_json::Error) -> Self {
        LoadError::Parse(e)
    }
}

/// 코드포인트가 옛한글(고어) 범위에 속하는지 검사.
///
/// 거부 대상 범위:
/// - U+1140..=U+115F  Hangul Jamo 옛한글 초성 (ᅀ ᅌ ᅙ 등)
/// - U+11C3..=U+11FF  Hangul Jamo 옛한글 종성 (U+11C3 이후)
/// - U+302E..=U+302F  방점
/// - U+3165..=U+318E  Hangul Compatibility Jamo 옛한글 (ㆍ U+318D 포함)
/// - U+A960..=U+A97F  Hangul Jamo Extended-A
/// - U+D7B0..=U+D7FF  Hangul Jamo Extended-B
fn is_archaic_jamo(cp: u32) -> bool {
    matches!(cp,
        0x1140..=0x115F   // Hangul Jamo 옛한글 초성
        | 0x11C3..=0x11FF // Hangul Jamo 옛한글 종성 (현대 종성 마지막은 U+11C2 ᇂ)
        | 0x302E..=0x302F // 방점
        | 0x3165..=0x318E // Hangul Compatibility Jamo 옛한글
        | 0xA960..=0xA97F // Hangul Jamo Extended-A
        | 0xD7B0..=0xD7FF // Hangul Jamo Extended-B
    )
}

/// 문자열에서 옛한글 코드포인트를 검색. 발견 시 첫 번째 반환.
fn find_archaic_jamo(s: &str) -> Option<u32> {
    s.chars().map(|c| c as u32).find(|&cp| is_archaic_jamo(cp))
}

/// layout/combinations/rule_sets 전체에서 옛한글 자모 거부.
fn reject_archaic_jamo(raw: &RawProfile) -> Result<(), LoadError> {
    let check = |s: &str, loc: &str| -> Result<(), LoadError> {
        if let Some(cp) = find_archaic_jamo(s) {
            return Err(LoadError::ArchaicJamoNotSupported {
                codepoint: cp,
                location: loc.to_string(),
            });
        }
        Ok(())
    };

    // layout.lower/upper 행 검사
    for (row_name, row) in [
        ("layout.lower.1st", &raw.layout.lower.row1),
        ("layout.lower.2nd", &raw.layout.lower.row2),
        ("layout.lower.3rd", &raw.layout.lower.row3),
        ("layout.lower.4th", &raw.layout.lower.row4),
        ("layout.upper.1st", &raw.layout.upper.row1),
        ("layout.upper.2nd", &raw.layout.upper.row2),
        ("layout.upper.3rd", &raw.layout.upper.row3),
        ("layout.upper.4th", &raw.layout.upper.row4),
    ] {
        for cell in row.iter() {
            check(cell, row_name)?;
        }
    }

    // combinations 검사
    if let Some(combos) = &raw.combinations {
        for triple in &combos.cho {
            check(&triple.first,  "combinations.cho.first")?;
            check(&triple.second, "combinations.cho.second")?;
            check(&triple.result, "combinations.cho.result")?;
        }
        for triple in &combos.jung {
            check(&triple.first,  "combinations.jung.first")?;
            check(&triple.second, "combinations.jung.second")?;
            check(&triple.result, "combinations.jung.result")?;
        }
        for triple in &combos.jong {
            check(&triple.first,  "combinations.jong.first")?;
            check(&triple.second, "combinations.jong.second")?;
            check(&triple.result, "combinations.jong.result")?;
        }
    }

    // rule_sets combinations 검사
    if let Some(rule_sets) = &raw.rule_sets {
        for (rs_name, rs) in rule_sets {
            for triple in &rs.combinations {
                check(&triple.first,  &format!("rule_sets[{rs_name}].combinations.first"))?;
                check(&triple.second, &format!("rule_sets[{rs_name}].combinations.second"))?;
                check(&triple.result, &format!("rule_sets[{rs_name}].combinations.result"))?;
            }
        }
    }

    Ok(())
}

/// JSON 문자열을 파싱해 `LayoutProfile`로 변환.
///
/// v1 마커가 하나도 없으면 `LoadError::UnsupportedSchema`로 거부한다 (v0 거부).
/// v3 마커가 존재하면 v3 경로로 파싱한다.
pub fn parse_profile_str(json: &str) -> Result<LayoutProfile, LoadError> {
    let raw: RawProfile = serde_json::from_str(json)?;
    if !raw.has_v1_markers() {
        let profile_name = raw.name.clone();
        unim_log!(
            "LOADER",
            "WARNING: layout profile '{}' uses the legacy v0 schema and will be rejected. \
             Convert to v1 (see docs/archive/plans/LAYOUT_PROFILE_V1.md).",
            profile_name
        );
        return Err(LoadError::UnsupportedSchema { profile_name });
    }
    // 옛한글 자모 거부 — 모든 스키마 버전에 적용.
    reject_archaic_jamo(&raw)?;
    if raw.is_v3() {
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
                matches!(profile.schema_version, 1..=3),
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
    // v3 테스트 (Phase 3-rework2: supports_moachigi/bidirectional_combine/chord_window_ms)
    // ======================================================================

    /// sv3-anmatae: supports_moachigi=true, bidirectional_combine=true → v3 경로.
    /// Phase 7: supports_moachigi=true → capability 마커 Some. 두 옵션값은 무시.
    #[test]
    fn sv3_anmatae_parses_correctly() {
        let json = r#"{
            "schema_version": 3,
            "language": "korean",
            "name": "anmatae_test",
            "type": "3bul",
            "layout": {
                "upper": {"1st":[],"2nd":[],"3rd":[],"4th":[]},
                "lower": {"1st":[],"2nd":[],"3rd":[],"4th":[]}
            },
            "combinations": {"cho":[],"jung":[],"jong":[]},
            "supports_moachigi": true,
            "bidirectional_combine": true
        }"#;
        let profile = parse_profile_str(json).unwrap();
        assert_eq!(profile.schema_version, 3);
        let m = profile.moachigi.as_ref().expect("moachigi should be Some (capability)");
        // Phase 7: 키맵 값 무시 → false/0. 사용자 config가 진실 공급원.
        assert!(!m.bidirectional_combine, "Phase7: 키맵 값 무시");
    }

    /// sv3-missing-moachigi: supports_moachigi=false → moachigi None (정상).
    #[test]
    fn sv3_no_supports_moachigi_is_ok() {
        let json = r#"{
            "schema_version": 3,
            "language": "korean",
            "name": "3bul_test",
            "type": "3bul",
            "layout": {
                "upper": {"1st":[],"2nd":[],"3rd":[],"4th":[]},
                "lower": {"1st":[],"2nd":[],"3rd":[],"4th":[]}
            },
            "combinations": {"cho":[],"jung":[],"jong":[]}
        }"#;
        let profile = parse_profile_str(json).unwrap();
        assert!(profile.moachigi.is_none(), "supports_moachigi=false → moachigi None");
    }

    /// sv3-chord (Phase 7): 키맵 chord_window_ms=50 → MoachigiSpec 무시. 파싱은 OK.
    #[test]
    fn sv3_chord_window_ms_parses() {
        let json = r#"{
            "schema_version": 3,
            "language": "korean",
            "name": "anmatae_chord",
            "type": "3bul",
            "layout": {
                "upper": {"1st":[],"2nd":[],"3rd":[],"4th":[]},
                "lower": {"1st":[],"2nd":[],"3rd":[],"4th":[]}
            },
            "combinations": {"cho":[],"jung":[],"jong":[]},
            "supports_moachigi": true,
            "bidirectional_combine": true,
            "chord_window_ms": 50
        }"#;
        let profile = parse_profile_str(json).unwrap();
        let m = profile.moachigi.as_ref().unwrap();
        // Phase 7: 키맵 값 무시 → 항상 0. 사용자 config 주입 경로만 유효.
        assert_eq!(m.chord_window_ms, 0, "Phase7: 키맵 chord_window_ms 무시");
    }

    /// sv3-implicit-marker (Phase 7): supports_moachigi=true → v3 인식 + capability 마커. 옵션값 무시.
    #[test]
    fn sv3_implicit_marker_via_supports_moachigi() {
        let json = r#"{
            "language": "korean",
            "name": "implicit_v3",
            "type": "3bul",
            "layout": {
                "upper": {"1st":[],"2nd":[],"3rd":[],"4th":[]},
                "lower": {"1st":[],"2nd":[],"3rd":[],"4th":[]}
            },
            "combinations": {"cho":[],"jung":[],"jong":[]},
            "supports_moachigi": true,
            "bidirectional_combine": true
        }"#;
        let profile = parse_profile_str(json).unwrap();
        assert!(profile.moachigi.is_some(), "supports_moachigi=true → capability 마커 Some");
        let m = profile.moachigi.as_ref().unwrap();
        // Phase 7: 키맵 값 무시 → false. 사용자 config가 진실 공급원.
        assert!(!m.bidirectional_combine, "Phase7: 키맵 값 무시");
    }

    /// R3a: ko_3bul390 로드 → v1/v2 경로, moachigi None.
    #[test]
    fn r3a_ko_3bul390_is_v1_path_moachigi_none() {
        let profile = load_builtin_profile("ko_3bul390").unwrap();
        assert!(
            matches!(profile.schema_version, 1 | 2),
            "ko_3bul390 must be v1/v2"
        );
        assert!(profile.moachigi.is_none(), "ko_3bul390 must have no moachigi");
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

    /// ko_3bul_anmatae: 내장 안마태 로드 → v3, supports_moachigi=true, moachigi capability 마커.
    ///
    /// Phase 7: 키맵 JSON의 bidirectional_combine/chord_window_ms 제거됨.
    /// MoachigiSpec은 capability 마커(Some)만. 두 옵션값은 사용자 config 소관.
    #[test]
    fn e4b_anmatae_layout_loads_with_moachigi() {
        let profile = load_builtin_profile("ko_3bul_anmatae")
            .expect("ko_3bul_anmatae should load");
        assert_eq!(profile.name, "ko_3bul_anmatae");
        assert_eq!(profile.schema_version, 3);
        let m = profile.moachigi.as_ref().expect("anmatae must have moachigi (capability)");
        // Phase 7: 키맵 값 무시 → 항상 default. 사용자 config에서 주입됨.
        assert!(!m.bidirectional_combine, "Phase7: 키맵 값 무시 → false");
        assert_eq!(m.chord_window_ms, 0, "Phase7: 키맵 값 무시 → 0");
    }

    // ======================================================================
    // E1~E4: 옛한글 거부 테스트 (유지 — Phase 3-rework2에서도 절대 규칙)
    // ======================================================================

    /// E1: layout에 U+318D(ㆍ 아래아 compat) 진입 → ArchaicJamoNotSupported.
    #[test]
    fn e1_layout_archaic_jamo_rejected() {
        let json = r#"{
            "schema_version": 3,
            "language": "korean",
            "name": "e1_test",
            "type": "3bul",
            "layout": {
                "upper": {},
                "lower": { "2nd": ["ᄀ", "ㆍ"] }
            },
            "combinations": {"cho":[],"jung":[],"jong":[]},
            "supports_moachigi": true
        }"#;
        let err = parse_profile_str(json).unwrap_err();
        match err {
            LoadError::ArchaicJamoNotSupported { codepoint, .. } => {
                assert_eq!(codepoint, 0x318D, "U+318D (ㆍ) 거부");
            }
            other => panic!("expected ArchaicJamoNotSupported, got {other:?}"),
        }
    }

    /// E2: combinations.jong에 U+11C3 (현대 종성 마지막 U+11C2 이후) 진입 → 거부.
    #[test]
    fn e2_combinations_archaic_jong_rejected() {
        let json = r#"{
            "schema_version": 3,
            "language": "korean",
            "name": "e2_test",
            "type": "3bul",
            "layout": {"upper": {}, "lower": {}},
            "combinations": {
                "cho": [],
                "jung": [],
                "jong": [{"first": "ᆨ", "second": "ᆯ", "result": "ᇃ"}]
            },
            "supports_moachigi": true
        }"#;
        let err = parse_profile_str(json).unwrap_err();
        match err {
            LoadError::ArchaicJamoNotSupported { codepoint, .. } => {
                assert_eq!(codepoint, 0x11C3, "U+11C3 거부");
            }
            other => panic!("expected ArchaicJamoNotSupported, got {other:?}"),
        }
    }

    /// E3: rule_sets combinations에 옛한글 코드포인트 → 거부.
    #[test]
    fn e3_rule_sets_archaic_jamo_rejected() {
        let json = r#"{
            "schema_version": 1,
            "language": "korean",
            "name": "e3_test",
            "type": "3bul",
            "layout": {"upper": {}, "lower": {}},
            "combinations": {"cho":[],"jung":[],"jong":[]},
            "rule_sets": {
                "bad_set": {
                    "active": true,
                    "combinations": [{"first": "ᆨ", "second": "ᆯ", "result": "ᇃ"}]
                }
            }
        }"#;
        let err = parse_profile_str(json).unwrap_err();
        match err {
            LoadError::ArchaicJamoNotSupported { codepoint, .. } => {
                assert_eq!(codepoint, 0x11C3, "rule_sets 내 U+11C3 거부");
            }
            other => panic!("expected ArchaicJamoNotSupported, got {other:?}"),
        }
    }

    /// E4: 정상 자판(ko_3bul390) 로드 → 옛한글 거부 없음 (false positive 없음).
    #[test]
    fn e4_normal_layout_not_rejected() {
        let profile = load_builtin_profile("ko_3bul390")
            .expect("ko_3bul390 should load without ArchaicJamoNotSupported");
        assert_eq!(profile.language, "korean");
    }

    /// E4b: 정상 v3 자판(ko_3bul_anmatae) 로드 → 옛한글 거부 없음.
    #[test]
    fn e4b_anmatae_not_rejected_by_archaic_check() {
        load_builtin_profile("ko_3bul_anmatae")
            .expect("ko_3bul_anmatae should load without ArchaicJamoNotSupported");
    }
}
