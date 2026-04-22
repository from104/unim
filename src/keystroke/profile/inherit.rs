//! `inherits` 체인 해석 — Phase 2 단계에서는 **최소 스텁**.
//!
//! 현재 내장 9종은 모두 `inherits`가 없어 동작 경로가 타지 않는다.
//! Phase 3에서 사용자 디렉토리 스캔과 함께 전체 해석 알고리즘
//! (`LAYOUT_PROFILE_V1.md` §4)을 구현한다.

use std::fmt;

use super::schema::LayoutProfile;

/// 상속 해석 실패 원인.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InheritError {
    /// base 프로필을 같은 네임스페이스에서 찾지 못함.
    BaseNotFound(String),
    /// inherits 체인에서 순환 감지.
    Cycle(Vec<String>),
    /// Phase 3에서 구현 — 현재는 사용자 프로필 해석 경로 자체가 없음.
    NotImplemented {
        reason: &'static str,
    },
}

impl fmt::Display for InheritError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            InheritError::BaseNotFound(n) => write!(f, "inherits base profile not found: {n}"),
            InheritError::Cycle(chain) => write!(f, "inherits cycle detected: {}", chain.join(" → ")),
            InheritError::NotImplemented { reason } => {
                write!(f, "inherits resolution not implemented yet: {reason}")
            }
        }
    }
}

impl std::error::Error for InheritError {}

/// 프로필의 `inherits` 체인을 해석해 완전히 병합된 프로필을 반환.
///
/// **Phase 2 동작**:
/// - `inherits`가 `None`이면 입력을 그대로 반환(clone).
/// - `inherits`가 `Some(_)`이면 `InheritError::NotImplemented` 반환.
///
/// Phase 3에서 이 함수가 내장 9종 + 사용자 디렉토리를 검색하고 체인을 병합한다.
pub fn resolve(profile: &LayoutProfile) -> Result<LayoutProfile, InheritError> {
    if profile.inherits.is_none() {
        Ok(profile.clone())
    } else {
        Err(InheritError::NotImplemented {
            reason: "Phase 3에서 구현 예정 (사용자 디렉토리 스캔과 함께)",
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::keystroke::profile::loader::load_builtin_profile;

    #[test]
    fn no_inherits_passes_through_unchanged() {
        for name in crate::keystroke::profile::builtin::BUILTIN_NAMES {
            let profile = load_builtin_profile(name).unwrap();
            let resolved = resolve(&profile).unwrap();
            assert_eq!(resolved.name, profile.name);
            assert_eq!(resolved.layout_type, profile.layout_type);
            assert_eq!(resolved.combinations.is_some(), profile.combinations.is_some());
        }
    }

    #[test]
    fn inherits_present_returns_not_implemented() {
        let json = r#"{
            "schema_version": 1,
            "language": "korean",
            "name": "child",
            "type": "3bul",
            "inherits": "ko_3bul390",
            "layout": {
                "upper": {"1st":[],"2nd":[],"3nd":[],"4th":[]},
                "lower": {"1st":[],"2nd":[],"3nd":[],"4th":[]}
            }
        }"#;
        let profile = crate::keystroke::profile::parse_profile_str(json).unwrap();
        let err = resolve(&profile).unwrap_err();
        assert!(matches!(err, InheritError::NotImplemented { .. }));
    }
}
