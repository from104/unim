//! v1 자판 프로필 JSON 스키마 — serde 역직렬화 타입.
//!
//! 스펙: `docs/dev/plans/LAYOUT_PROFILE_V1.md`
//!
//! # 구조
//! - `RawProfile` — JSON에서 직접 역직렬화되는 평면 구조. v1 필드가 optional.
//! - `LayoutProfile` — 정규화 후의 런타임 표현.
//!
//! 0.2.0부터 v0(legacy) 스키마는 더 이상 지원되지 않는다. 로더는 v1 마커
//! (`schema_version`, `metadata`, `inherits`, `combinations`, `rule_sets`,
//! `active_rule_sets`) 중 하나라도 존재해야 v1으로 인식하고, 모두 없는 JSON은
//! `LoadError::UnsupportedSchema`로 거부한다.
//!
//! combinations 해석·inherits 병합·자모 enum 변환은 builder에서 수행한다.
//! 본 모듈은 순수 스키마(문자열 수준)만 다룬다.

use serde::Deserialize;
use std::collections::BTreeMap;

use super::localized::LocalizedText;

// ============================================================================
// JSON Raw 구조 (파일에서 바로 역직렬화)
// ============================================================================

/// JSON 파일에서 바로 역직렬화되는 원시 구조체.
///
/// 모든 v1 필드는 optional이지만, 로더(`parse_profile_str`)는 v1 마커 중
/// 하나라도 존재해야 수용한다(v0 거부).
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RawProfile {
    // ── 공통 필수 ──────────────────────────────────────
    pub language: String,
    pub name: String,
    #[serde(rename = "type")]
    pub r#type: String,
    pub layout: KeyLayout,

    // ── v1 필드 (optional, 단 하나 이상 존재해야 v1 인정) ──
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
// v0/v1 게이트
// ============================================================================

impl RawProfile {
    /// v1 스키마로 받아들일지 판정. v1 전용 필드 중 하나라도 존재해야 한다.
    ///
    /// 모든 마커가 부재하면 0.1.x 시기의 v0 포맷이며, 0.2.0부터는 거부.
    pub fn has_v1_markers(&self) -> bool {
        self.schema_version.is_some()
            || self.metadata.is_some()
            || self.inherits.is_some()
            || self.combinations.is_some()
            || self.rule_sets.is_some()
            || self.active_rule_sets.is_some()
    }
}

// ============================================================================
// 정규화된 런타임 표현
// ============================================================================

/// 정규화된 런타임 프로필 (v1 only).
///
/// JSON 구조를 1:1로 매핑하되, `rule_sets`의 legacy `reinterpret`만
/// `combinations`로 흡수한다. combinations 해석, inherits 병합,
/// active_rule_sets 적용은 builder에서.
#[derive(Debug, Clone)]
pub struct LayoutProfile {
    /// 항상 1 이상. 0.2.0부터 v0(=0)는 거부됨.
    pub schema_version: u8,
    pub language: String,
    pub name: String,
    /// `"2bul"` / `"3bul"` / `"qwerty"` / `"dvorak"` 등.
    pub layout_type: String,
    pub metadata: LayoutMetadata,
    pub inherits: Option<String>,
    pub layout: KeyLayout,
    /// v1 프로필은 자기 완결 — 항상 명시되어야 하지만, 영문 계열처럼
    /// 자모 조합이 의미 없는 경우 비어 있을 수 있다(빈 블록 또는 None).
    /// 한글 계열은 builder가 None을 거부.
    pub combinations: Option<CombinationsBlock>,
    pub rule_sets: BTreeMap<String, RuleSet>,
    /// `None`이면 각 rule_set의 `active` 값을 그대로 사용.
    /// `Some(list)`이면 이 목록의 이름만 active, 나머지는 강제 off.
    pub active_rule_sets: Option<Vec<String>>,
}

impl LayoutProfile {
    /// `RawProfile`을 정규화해 `LayoutProfile`로 변환.
    ///
    /// 호출자(`parse_profile_str`)가 사전에 `has_v1_markers()`로 v0를 거부했음을 가정.
    pub fn from_raw(raw: RawProfile) -> Self {
        let schema_version = raw.schema_version.unwrap_or(1);

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

    /// 0.1.x 시기의 v0 포맷 — v1 마커가 모두 부재한 JSON. 0.2.0에서는
    /// `RawProfile::has_v1_markers()`가 false를 반환해 거부 대상이다.
    fn v0_legacy_json() -> &'static str {
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
    fn legacy_v0_has_no_v1_markers() {
        let raw: RawProfile = serde_json::from_str(v0_legacy_json()).unwrap();
        assert!(
            !raw.has_v1_markers(),
            "legacy v0 JSON must have no v1 marker fields (loader rejects it)"
        );
    }

    #[test]
    fn v1_with_schema_version_is_v1() {
        let raw: RawProfile = serde_json::from_str(v1_json()).unwrap();
        assert!(raw.has_v1_markers());
    }

    #[test]
    fn v1_from_just_metadata_is_v1() {
        let json = r#"{
            "language": "korean",
            "name": "x",
            "type": "2bul",
            "metadata": {"author": "me"},
            "layout": {"upper":{},"lower":{}}
        }"#;
        let raw: RawProfile = serde_json::from_str(json).unwrap();
        assert!(raw.has_v1_markers());
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
        assert_eq!(
            rs.combinations.len(),
            2,
            "reinterpret가 combinations로 흡수"
        );
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
