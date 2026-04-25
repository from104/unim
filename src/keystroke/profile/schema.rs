//! v0/v1 자판 프로필 JSON 스키마 — serde 역직렬화 타입.
//!
//! 스펙: `docs/plans/LAYOUT_PROFILE_V1.md`
//!
//! # 구조
//! - `RawProfile` — JSON에서 직접 역직렬화되는 평면 구조. 모든 v1 필드가 optional.
//! - `SchemaKind` — v0/v1 판별 결과.
//! - `LayoutProfile` — 판별·정규화 후의 런타임 표현.
//!
//! combinations 해석·inherits 병합·자모 enum 변환은 **Phase 2 이후** builder에서 수행한다.
//! 본 모듈은 순수 스키마(문자열 수준)만 다룬다.

use serde::Deserialize;
use std::collections::BTreeMap;

use super::localized::LocalizedText;

// ============================================================================
// JSON Raw 구조 (파일에서 바로 역직렬화)
// ============================================================================

/// JSON 파일에서 바로 역직렬화되는 원시 구조체.
///
/// v0와 v1의 모든 필드를 하나로 합쳐 두되, v1 전용 필드는 optional.
/// 역직렬화 후 `SchemaKind::detect(&raw)`로 v0/v1 판별.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RawProfile {
    // ── v0/v1 공통 ─────────────────────────────────────
    pub language: String,
    pub name: String,
    #[serde(rename = "type")]
    pub r#type: String,
    pub layout: KeyLayout,

    // ── v1 전용 (optional) ─────────────────────────────
    #[serde(default)]
    pub schema_version: Option<u8>,
    #[serde(default)]
    pub metadata: Option<LayoutMetadata>,
    #[serde(default)]
    pub inherits: Option<String>,
    #[serde(default)]
    pub combinations: Option<CombinationsBlock>,
    #[serde(default)]
    pub rule_sets: Option<BTreeMap<String, RuleSet>>,
    #[serde(default)]
    pub active_rule_sets: Option<Vec<String>>,
}

// ============================================================================
// Layout 구조 (v0/v1 공통)
// ============================================================================

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
pub struct KeyLayout {
    pub upper: LayoutRows,
    pub lower: LayoutRows,
}

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
pub struct LayoutRows {
    /// 숫자열. 3벌식은 14키, 2벌식/영문 계열은 14키.
    #[serde(rename = "1st", default)]
    pub row1: Vec<String>,
    /// QWERTY 상단: Q-P.
    #[serde(rename = "2nd", default)]
    pub row2: Vec<String>,
    /// QWERTY 중간: A-L(;'). `3nd`는 역사적 오기지만 v0와 호환 유지.
    #[serde(rename = "3nd", default)]
    pub row3: Vec<String>,
    /// QWERTY 하단: Z-M(,./).
    #[serde(rename = "4th", default)]
    pub row4: Vec<String>,
}

// ============================================================================
// v1 메타데이터
// ============================================================================

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(deny_unknown_fields)]
pub struct LayoutMetadata {
    #[serde(default)]
    pub display_name: Option<LocalizedText>,
    #[serde(default)]
    pub author: Option<String>,
    #[serde(default)]
    pub version: Option<String>,
    #[serde(default)]
    pub license: Option<String>,
    #[serde(default)]
    pub description: Option<LocalizedText>,
    #[serde(default)]
    pub source_url: Option<String>,
    #[serde(default)]
    pub tags: Vec<String>,
}

// ============================================================================
// v1 combinations 블록
// ============================================================================

/// 자판 기본 조합 규칙. 존재하면 자기 완결 — Rust const 미참조.
#[derive(Debug, Clone, Deserialize, Default)]
#[serde(deny_unknown_fields)]
pub struct CombinationsBlock {
    #[serde(default)]
    pub cho: Vec<RawTriple>,
    #[serde(default)]
    pub jung: Vec<RawTriple>,
    #[serde(default)]
    pub jong: Vec<RawTriple>,
}

/// `(first, second) → result` 조합 엔트리 (해석 전 문자열).
///
/// 자모 enum으로의 변환은 Phase 2 `builder`에서. 여기서는 순수 문자열.
#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct RawTriple {
    pub first: String,
    pub second: String,
    pub result: String,
}

/// `reinterpret` 엔트리 — 기획 초안에서는 별도 타입으로 분리되었으나 v1 최종안에선
/// 일반 `RawTriple`로 통합되었다. 이 타입은 **레거시 드래프트 JSON 호환용**으로 유지.
/// 로더가 감지하면 `combinations`에 등가로 흡수 후 무시한다.
#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ReinterpretTriple {
    pub from: String,
    pub input: String,
    pub to: String,
}

// ============================================================================
// v1 규칙 세트
// ============================================================================

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(deny_unknown_fields)]
pub struct RuleSet {
    /// 기본 활성 여부.
    #[serde(default)]
    pub active: bool,
    #[serde(default)]
    pub description: Option<LocalizedText>,
    /// 세트에 속하는 pair combinations. scope는 first 자모 코드포인트로 자동 판별.
    #[serde(default)]
    pub combinations: Vec<RawTriple>,
    /// 초안 시기 `reinterpret` 필드. v1 최종 스키마에서는 `combinations`로 통합되었으나
    /// 기존 드래프트 JSON 호환을 위해 필드를 남겨 둠. 로더가 `combinations`로 흡수.
    #[serde(default)]
    pub reinterpret: Vec<ReinterpretTriple>,
    /// 초안 시기 `scope` 필드. 자동 판별로 대체되어 무시되지만 파싱 에러 방지용으로 수용.
    #[serde(default)]
    pub scope: Option<String>,
}

// ============================================================================
// 판별
// ============================================================================

/// 프로필 JSON이 v0인지 v1인지.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SchemaKind {
    /// v0 (기존 포맷). `combinations`·`rule_sets` 등 없음.
    V0,
    /// v1 (자기 완결 포맷 또는 v1 전용 필드 중 하나라도 존재).
    V1,
}

impl SchemaKind {
    /// `LAYOUT_PROFILE_V1.md` §3.4 판별 규칙.
    pub fn detect(raw: &RawProfile) -> SchemaKind {
        if raw.schema_version.is_some()
            || raw.metadata.is_some()
            || raw.inherits.is_some()
            || raw.combinations.is_some()
            || raw.rule_sets.is_some()
            || raw.active_rule_sets.is_some()
        {
            SchemaKind::V1
        } else {
            SchemaKind::V0
        }
    }
}

// ============================================================================
// 정규화된 런타임 표현
// ============================================================================

/// 판별·정규화 후의 런타임 프로필.
///
/// Phase 1에서는 JSON 구조를 1:1로 매핑하기만 한다. combinations 해석,
/// inherits 병합, active_rule_sets 적용은 Phase 2 이후.
#[derive(Debug, Clone)]
pub struct LayoutProfile {
    /// 0 (v0에서 자동 승격) 또는 1.
    pub schema_version: u8,
    pub language: String,
    pub name: String,
    /// `"2bul"` / `"3bul"` / `"qwerty"` / `"dvorak"` 등.
    pub layout_type: String,
    pub metadata: LayoutMetadata,
    pub inherits: Option<String>,
    pub layout: KeyLayout,
    /// v0 프로필에서는 `None` — 기본 테이블(Rust const)을 런타임에서 상속.
    /// v1 프로필에서는 `Some(_)`이며 파일에 명시된 값 그대로.
    pub combinations: Option<CombinationsBlock>,
    pub rule_sets: BTreeMap<String, RuleSet>,
    /// `None`이면 각 rule_set의 `active` 값을 그대로 사용.
    /// `Some(list)`이면 이 목록의 이름만 active, 나머지는 강제 off.
    pub active_rule_sets: Option<Vec<String>>,
}

impl LayoutProfile {
    /// `RawProfile`을 판별 + 정규화해 `LayoutProfile`로 변환.
    pub fn from_raw(raw: RawProfile) -> Self {
        let kind = SchemaKind::detect(&raw);

        let schema_version = match kind {
            SchemaKind::V0 => 0,
            SchemaKind::V1 => raw.schema_version.unwrap_or(1),
        };

        // rule_sets의 legacy `reinterpret` 필드를 combinations로 흡수.
        let rule_sets = raw
            .rule_sets
            .unwrap_or_default()
            .into_iter()
            .map(|(name, mut rs)| {
                if !rs.reinterpret.is_empty() {
                    let converted = rs.reinterpret.drain(..).map(|r| RawTriple {
                        first: r.from,
                        second: r.input,
                        result: r.to,
                    });
                    rs.combinations.extend(converted);
                }
                // scope 필드는 v1 최종안에서 무시하지만 호환 파싱을 위해 수용만.
                rs.scope = None;
                (name, rs)
            })
            .collect();

        LayoutProfile {
            schema_version,
            language: raw.language,
            name: raw.name,
            layout_type: raw.r#type,
            metadata: raw.metadata.unwrap_or_default(),
            inherits: raw.inherits,
            layout: raw.layout,
            combinations: raw.combinations,
            rule_sets,
            active_rule_sets: raw.active_rule_sets,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn v0_json() -> &'static str {
        // `#` 문자가 JSON 내부에 있으므로 `r##"..."##`로 2단 raw string 사용.
        r##"{
            "language": "korean",
            "name": "2bulstd",
            "type": "2bul",
            "layout": {
                "upper": {
                    "1st": ["~","!","@","#","$","%","^","&","*","(",")","_","+","|"],
                    "2nd": ["ㅃ","ㅉ","ㄸ","ㄲ","ㅆ","ㅛ","ㅕ","ㅑ","ㅒ","ㅖ","{","}"],
                    "3nd": ["ㅁ","ㄴ","ㅇ","ㄹ","ㅎ","ㅗ","ㅓ","ㅏ","ㅣ",":","\""],
                    "4th": ["ㅋ","ㅌ","ㅊ","ㅍ","ㅠ","ㅜ","ㅡ","<",">","?"]
                },
                "lower": {
                    "1st": ["`","1","2","3","4","5","6","7","8","9","0","-","=","\\"],
                    "2nd": ["ㅂ","ㅈ","ㄷ","ㄱ","ㅅ","ㅛ","ㅕ","ㅑ","ㅐ","ㅔ","[","]"],
                    "3nd": ["ㅁ","ㄴ","ㅇ","ㄹ","ㅎ","ㅗ","ㅓ","ㅏ","ㅣ",";","'"],
                    "4th": ["ㅋ","ㅌ","ㅊ","ㅍ","ㅠ","ㅜ","ㅡ",",",".","/"]
                }
            }
        }"##
    }

    fn v1_json() -> &'static str {
        r#"{
            "schema_version": 1,
            "language": "korean",
            "name": "test",
            "type": "3bul",
            "metadata": {
                "display_name": "테스트",
                "description": {"ko": "설명", "en": "Description"}
            },
            "layout": {
                "upper": {"1st": [], "2nd": [], "3nd": [], "4th": []},
                "lower": {"1st": [], "2nd": [], "3nd": [], "4th": []}
            },
            "combinations": {
                "cho": [{"first":"ㄱ","second":"ㄱ","result":"ㄲ"}],
                "jung": [],
                "jong": []
            },
            "rule_sets": {
                "test_set": {
                    "active": true,
                    "description": "순아래받침",
                    "combinations": [
                        {"first":"ᆫ","second":"ᆫ","result":"ᆮ"}
                    ]
                }
            },
            "active_rule_sets": ["test_set"]
        }"#
    }

    #[test]
    fn detects_v0_when_no_v1_fields() {
        let raw: RawProfile = serde_json::from_str(v0_json()).unwrap();
        assert_eq!(SchemaKind::detect(&raw), SchemaKind::V0);
    }

    #[test]
    fn detects_v1_from_schema_version() {
        let raw: RawProfile = serde_json::from_str(v1_json()).unwrap();
        assert_eq!(SchemaKind::detect(&raw), SchemaKind::V1);
    }

    #[test]
    fn detects_v1_from_just_metadata() {
        let json = r#"{
            "language": "korean",
            "name": "x",
            "type": "2bul",
            "metadata": {"author": "me"},
            "layout": {"upper":{},"lower":{}}
        }"#;
        let raw: RawProfile = serde_json::from_str(json).unwrap();
        assert_eq!(SchemaKind::detect(&raw), SchemaKind::V1);
    }

    #[test]
    fn v0_promoted_has_schema_zero_and_no_combinations() {
        let raw: RawProfile = serde_json::from_str(v0_json()).unwrap();
        let profile = LayoutProfile::from_raw(raw);
        assert_eq!(profile.schema_version, 0);
        assert!(profile.combinations.is_none());
        assert!(profile.rule_sets.is_empty());
        assert_eq!(profile.name, "2bulstd");
        assert_eq!(profile.layout_type, "2bul");
    }

    #[test]
    fn v1_preserves_combinations_and_rule_sets() {
        let raw: RawProfile = serde_json::from_str(v1_json()).unwrap();
        let profile = LayoutProfile::from_raw(raw);
        assert_eq!(profile.schema_version, 1);
        let combos = profile.combinations.as_ref().unwrap();
        assert_eq!(combos.cho.len(), 1);
        assert_eq!(combos.cho[0].result, "ㄲ");
        assert_eq!(profile.rule_sets.len(), 1);
        assert_eq!(
            profile.active_rule_sets.as_deref(),
            Some(&["test_set".to_string()][..])
        );
    }

    #[test]
    fn legacy_reinterpret_absorbed_into_combinations() {
        let json = r#"{
            "schema_version": 1,
            "language": "korean",
            "name": "x",
            "type": "3bul",
            "layout": {
                "upper": {"1st":[],"2nd":[],"3nd":[],"4th":[]},
                "lower": {"1st":[],"2nd":[],"3nd":[],"4th":[]}
            },
            "rule_sets": {
                "r": {
                    "active": true,
                    "scope": "jong",
                    "combinations": [{"first":"ᆯ","second":"ᆨ","result":"ᆰ"}],
                    "reinterpret": [{"from":"ᆶ","input":"ᆫ","to":"ᆴ"}]
                }
            }
        }"#;
        let raw: RawProfile = serde_json::from_str(json).unwrap();
        let profile = LayoutProfile::from_raw(raw);
        let rs = profile.rule_sets.get("r").unwrap();
        assert_eq!(rs.combinations.len(), 2, "reinterpret가 combinations로 흡수");
        assert!(rs.reinterpret.is_empty(), "reinterpret는 drain되어 비움");
        let absorbed = &rs.combinations[1];
        assert_eq!(absorbed.first, "ᆶ");
        assert_eq!(absorbed.second, "ᆫ");
        assert_eq!(absorbed.result, "ᆴ");
    }

    #[test]
    fn localized_description_parses_object() {
        let raw: RawProfile = serde_json::from_str(v1_json()).unwrap();
        let profile = LayoutProfile::from_raw(raw);
        let desc = profile.metadata.description.as_ref().unwrap();
        assert_eq!(desc.resolve("ko"), "설명");
        assert_eq!(desc.resolve("en"), "Description");
    }

    #[test]
    fn localized_description_parses_single_string() {
        let raw: RawProfile = serde_json::from_str(
            r#"{
                "schema_version": 1,
                "language": "korean",
                "name": "x",
                "type": "2bul",
                "metadata": {"description": "단일"},
                "layout": {
                    "upper":{"1st":[],"2nd":[],"3nd":[],"4th":[]},
                    "lower":{"1st":[],"2nd":[],"3nd":[],"4th":[]}
                }
            }"#,
        )
        .unwrap();
        let profile = LayoutProfile::from_raw(raw);
        let desc = profile.metadata.description.as_ref().unwrap();
        assert_eq!(desc.resolve("ko"), "단일");
        assert_eq!(desc.resolve("xx"), "단일");
    }
}
