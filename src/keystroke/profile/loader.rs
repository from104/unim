//! 프로필 로더 — JSON 문자열 또는 내장 이름에서 `LayoutProfile` 생성.
//!
//! # Phase 1 범위
//! - `parse_profile_str(json)` — serde 역직렬화 + v0/v1 판별 + 정규화.
//! - `load_builtin_profile(name)` — 내장 9종 조회 + 파싱.
//!
//! inherits 해석, 사용자 디렉토리 스캔, combinations 병합은 Phase 2/3에서 추가.

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
}

impl fmt::Display for LoadError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            LoadError::NotFound(name) => write!(f, "layout profile not found: {name}"),
            LoadError::Parse(e) => write!(f, "failed to parse layout profile JSON: {e}"),
        }
    }
}

impl std::error::Error for LoadError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            LoadError::NotFound(_) => None,
            LoadError::Parse(e) => Some(e),
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
/// v0/v1 판별과 v0 자동 승격은 `LayoutProfile::from_raw`가 처리.
pub fn parse_profile_str(json: &str) -> Result<LayoutProfile, LoadError> {
    let raw: RawProfile = serde_json::from_str(json)?;
    Ok(LayoutProfile::from_raw(raw))
}

/// 내장 9종 중 하나를 로드. `name`은 정식 이름(`en_qwerty`) 또는 별칭(`qwerty`) 허용.
pub fn load_builtin_profile(name: &str) -> Result<LayoutProfile, LoadError> {
    let json = builtin::get_builtin_json(name)
        .ok_or_else(|| LoadError::NotFound(name.to_string()))?;
    parse_profile_str(json)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::keystroke::profile::builtin::BUILTIN_NAMES;

    #[test]
    fn loads_all_builtin_profiles_as_v1() {
        // Phase 6 이관 후: 모든 내장 프로필은 v1 포맷 (영문 계열은 combinations 없음).
        for name in BUILTIN_NAMES {
            let profile = load_builtin_profile(name)
                .unwrap_or_else(|e| panic!("failed to load builtin {name}: {e}"));
            assert_eq!(
                profile.schema_version, 1,
                "{name}: Phase 6 이관 후 모든 내장은 v1 포맷"
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
}
