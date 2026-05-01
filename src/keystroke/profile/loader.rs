//! 프로필 로더 — JSON 문자열 또는 내장 이름에서 `LayoutProfile` 생성.
//!
//! # 범위
//! - `parse_profile_str(json)` — serde 역직렬화 + v1 마커 검증 + 정규화.
//! - `load_builtin_profile(name)` — 내장 10종 조회 + 파싱.
//!
//! 0.2.0부터 v0(legacy) 스키마는 거부된다. v1 마커
//! (`schema_version`/`metadata`/`inherits`/`combinations`/`rule_sets`/`active_rule_sets`)
//! 중 하나라도 존재해야 v1로 인정한다. 사용자가 작성한 v0 JSON은
//! `LoadError::UnsupportedSchema`로 명확히 거부된다.

use std::fmt;

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
}

impl fmt::Display for LoadError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            LoadError::NotFound(name) => write!(f, "layout profile not found: {name}"),
            LoadError::Parse(e) => write!(f, "failed to parse layout profile JSON: {e}"),
            LoadError::UnsupportedSchema { profile_name } => write!(
                f,
                "layout profile '{profile_name}' uses the legacy v0 schema, which is no longer supported in 0.2.0+. Convert to v1 schema (see docs/dev/plans/LAYOUT_PROFILE_V1.md)."
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
        }
    }
}

impl From<serde_json::Error> for LoadError {
    fn from(e: serde_json::Error) -> Self {
        LoadError::Parse(e)
    }
}

/// JSON 문자열을 파싱해 `LayoutProfile`로 변환.
///
/// v1 마커가 하나도 없으면 `LoadError::UnsupportedSchema`로 거부한다 (v0 거부).
pub fn parse_profile_str(json: &str) -> Result<LayoutProfile, LoadError> {
    let raw: RawProfile = serde_json::from_str(json)?;
    if !raw.has_v1_markers() {
        let profile_name = raw.name.clone();
        // 콘솔 경고 — UNIM_DEVELOP=1 외에도 항상 stderr로 안내.
        eprintln!(
            "[unim] WARNING: layout profile '{profile_name}' uses the legacy v0 schema and will be rejected. \
             Convert to v1 (see docs/dev/plans/LAYOUT_PROFILE_V1.md)."
        );
        return Err(LoadError::UnsupportedSchema { profile_name });
    }
    Ok(LayoutProfile::from_raw(raw))
}

/// 내장 10종 중 하나를 로드. `name`은 정식 이름(`en_qwerty`) 또는 별칭(`qwerty`) 허용.
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
    fn loads_all_builtin_profiles_as_v1() {
        // Phase 6 이관 후: 모든 내장 프로필은 v1 또는 v2 포맷.
        // v2는 schema 확장(key_meta 등)을 담은 자판에서만 사용.
        for name in BUILTIN_NAMES {
            let profile = load_builtin_profile(name)
                .unwrap_or_else(|e| panic!("failed to load builtin {name}: {e}"));
            assert!(
                matches!(profile.schema_version, 1 | 2),
                "{name}: 내장 프로필은 v1 또는 v2 (was {})",
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
                "upper": {"1st":[],"2nd":[],"3nd":[],"4th":[]},
                "lower": {"1st":[],"2nd":[],"3nd":[],"4th":[]}
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
        // 모든 v1 마커가 부재한 0.1.x 시기 포맷.
        let v0_json = r##"{
            "language": "korean",
            "name": "legacy_user_layout",
            "type": "2bul",
            "layout": {
                "upper": {"1st":[],"2nd":[],"3nd":[],"4th":[]},
                "lower": {"1st":[],"2nd":[],"3nd":[],"4th":[]}
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
}
