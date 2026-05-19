//! `LayoutProfile` ↔ JSON 직렬화 헬퍼.
//!
//! `unim::keystroke::profile::RawProfile`은 `Serialize` derive를 갖고 있어 직접
//! JSON으로 쓸 수 있다. 본 모듈은 (1) 런타임 표현 `LayoutProfile`을 `RawProfile`
//! 형태로 역변환하고 (2) 사용자 디렉토리(`~/.config/unim/layouts/{name}.json`)에
//! 저장하는 얇은 래퍼다.
//!
//! 라운드트립 보장:
//! - 빈 metadata/rule_sets는 None으로 환원되어 출력 JSON에 노출되지 않는다.
//! - deprecated 필드(`reinterpret`/`scope`/`bidirectional_combine`/`chord_window_ms`)는
//!   `#[serde(skip_serializing)]`로 본체에서 제외됨.

use std::collections::BTreeMap;
use std::fs;
use std::io;
use std::path::PathBuf;

use unim::keystroke::profile::schema::RawProfile;
use unim::keystroke::profile::{LayoutMetadata, LayoutProfile, RuleSet};

/// 사용자 디렉토리의 표준 키맵 저장 위치 — `~/.config/unim/layouts/{name}.json`.
///
/// `dirs::config_dir()`을 사용. 결과 디렉토리는 호출자가 필요 시 생성해야 한다.
pub fn layout_user_path(name: &str) -> Option<PathBuf> {
    dirs::config_dir().map(|p| p.join("unim").join("layouts").join(format!("{name}.json")))
}

/// `LayoutProfile`을 `RawProfile`로 역변환.
///
/// 빈 metadata/rule_sets는 None으로 정규화해 직렬화 결과에서 누락되게 한다.
pub fn to_raw(profile: &LayoutProfile) -> RawProfile {
    let metadata = if is_metadata_empty(&profile.metadata) {
        None
    } else {
        Some(profile.metadata.clone())
    };
    let rule_sets: Option<BTreeMap<String, RuleSet>> = if profile.rule_sets.is_empty() {
        None
    } else {
        Some(profile.rule_sets.clone())
    };

    RawProfile {
        language: profile.language.clone(),
        name: profile.name.clone(),
        r#type: profile.layout_type.clone(),
        layout: profile.layout.clone(),
        schema_version: Some(profile.schema_version),
        metadata,
        inherits: profile.inherits.clone(),
        combinations: profile.combinations.clone(),
        rule_sets,
        active_rule_sets: profile.active_rule_sets.clone(),
        key_meta: profile.key_meta.clone(),
        supports_moachigi: profile.moachigi.is_some(),
        bidirectional_combine: false,
        chord_window_ms: 0,
    }
}

/// `LayoutProfile`을 pretty JSON 문자열로.
pub fn to_pretty_json(profile: &LayoutProfile) -> Result<String, serde_json::Error> {
    serde_json::to_string_pretty(&to_raw(profile))
}

/// 사용자 디렉토리에 키맵을 저장. 디렉토리가 없으면 생성.
pub fn save_profile_json(profile: &LayoutProfile) -> Result<PathBuf, SaveError> {
    let path = layout_user_path(&profile.name).ok_or(SaveError::NoConfigDir)?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(SaveError::Io)?;
    }
    let json = to_pretty_json(profile).map_err(SaveError::Encode)?;
    fs::write(&path, json).map_err(SaveError::Io)?;
    Ok(path)
}

#[derive(Debug)]
pub enum SaveError {
    NoConfigDir,
    Io(io::Error),
    Encode(serde_json::Error),
}

impl std::fmt::Display for SaveError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SaveError::NoConfigDir => write!(f, "could not resolve user config directory"),
            SaveError::Io(e) => write!(f, "io error: {e}"),
            SaveError::Encode(e) => write!(f, "encode error: {e}"),
        }
    }
}

impl std::error::Error for SaveError {}

fn is_metadata_empty(m: &LayoutMetadata) -> bool {
    m.display_name.is_none()
        && m.author.is_none()
        && m.version.is_none()
        && m.license.is_none()
        && m.description.is_none()
        && m.source_url.is_none()
        && m.tags.is_empty()
}

#[cfg(test)]
mod tests {
    use super::*;
    use unim::keystroke::profile::{load_builtin_profile, parse_profile_str};

    /// 내장 한글 프로필 5종에 대해 read → serialize → re-parse 라운드트립.
    #[test]
    fn roundtrip_all_korean_builtins() {
        for name in [
            "ko_2bulstd",
            "ko_3bul390",
            "ko_3bul391",
            "ko_3bul_noshift",
            "ko_3bul_anmatae",
        ] {
            let original = load_builtin_profile(name)
                .unwrap_or_else(|e| panic!("load {name}: {e:?}"));
            let json = to_pretty_json(&original)
                .unwrap_or_else(|e| panic!("encode {name}: {e:?}"));
            let reparsed = parse_profile_str(&json)
                .unwrap_or_else(|e| panic!("re-parse {name}: {e:?}\n--- JSON ---\n{json}"));

            // 핵심 필드 비교
            assert_eq!(original.name, reparsed.name, "{name}: name");
            assert_eq!(original.language, reparsed.language, "{name}: language");
            assert_eq!(
                original.layout_type, reparsed.layout_type,
                "{name}: layout_type"
            );
            assert_eq!(
                original.schema_version, reparsed.schema_version,
                "{name}: schema_version"
            );
            assert_eq!(original.layout, reparsed.layout, "{name}: layout");
            assert_eq!(
                original.moachigi.is_some(),
                reparsed.moachigi.is_some(),
                "{name}: moachigi 마커"
            );
        }
    }

    #[test]
    fn anmatae_roundtrip_keeps_v3_marker() {
        let original = load_builtin_profile("ko_3bul_anmatae").unwrap();
        assert!(original.moachigi.is_some(), "v3 마커 사전 조건");

        let json = to_pretty_json(&original).unwrap();
        assert!(json.contains("\"schema_version\": 3"));
        assert!(json.contains("\"supports_moachigi\": true"));
        // deprecated 필드는 새 JSON에 노출되지 않아야 함
        assert!(!json.contains("\"bidirectional_combine\""));
        assert!(!json.contains("\"chord_window_ms\""));
    }

    #[test]
    fn empty_metadata_is_omitted() {
        // 최소 v1 프로필(메타데이터/조합 없음) → JSON에 metadata 키 없어야 함
        let minimal = r#"{
            "schema_version": 1,
            "language": "korean",
            "name": "tmp_empty_meta",
            "type": "2bul",
            "layout": {
                "upper": {"1st":[],"2nd":[],"3rd":[],"4th":[]},
                "lower": {"1st":[],"2nd":[],"3rd":[],"4th":[]}
            }
        }"#;
        let p = parse_profile_str(minimal).unwrap();
        let out = to_pretty_json(&p).unwrap();
        assert!(!out.contains("\"metadata\""), "empty metadata must be dropped");
        assert!(!out.contains("\"rule_sets\""), "empty rule_sets must be dropped");
    }

    #[test]
    fn layout_user_path_is_under_config() {
        // 환경에 따라 None일 수 있으나 Some이면 unim/layouts/X.json 형태여야 함
        if let Some(p) = layout_user_path("my_layout") {
            let s = p.to_string_lossy();
            assert!(s.ends_with("unim/layouts/my_layout.json"), "got {s}");
        }
    }
}
