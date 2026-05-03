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

use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap};

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
    /// schema_version 2 신규 — 키별 메타데이터.
    /// 키는 layout 셀과 동일한 컨벤션의 리터럴 문자열(예: `"/"`, `"ᆮ"`).
    /// PR-A는 schema·파싱만. 동작 적용은 PR-B.
    #[serde(default)]
    pub key_meta: Option<HashMap<String, KeyMeta>>,
}

// ============================================================================
// schema_version 2 — 키 메타데이터 (PR-A: dangling)
// ============================================================================

/// 키 단위 메타데이터. 룰 A·B 동작의 데이터 표현.
///
/// - `vowel_combine_head`: 룰 A. 이 키의 모음만 이중모음(ㅘ/ㅙ/ㅚ/ㅝ/ㅞ/ㅟ) 결합 가능.
///   누락(`None`)이면 결합 가능(`true`)으로 해석 — 두벌식 호환.
/// - `context_alt`: 룰 B. preedit 상태에 따른 키 출력 분기.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct KeyMeta {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub vowel_combine_head: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub context_alt: Option<ContextAlt>,
}

impl KeyMeta {
    /// `KeyMeta`를 composer 큐에 보관될 `JamoMeta`로 변환.
    ///
    /// `vowel_combine_head` 누락 시 `true`(결합 가능)로 해석 — 두벌식 호환.
    pub fn to_jamo_meta(&self) -> crate::hangul::composer::JamoMeta {
        crate::hangul::composer::JamoMeta {
            vowel_combine_head: self.vowel_combine_head.unwrap_or(true),
        }
    }
}

/// 컨텍스트 분기 규칙. preedit 상태(`when`)가 true면 `to`, 아니면 `fallback`.
///
/// 예: `/` 키, preedit 초성-only일 때 ㅗ, 그 외 리터럴 `/`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ContextAlt {
    pub when: ContextCondition,
    pub to: String,
    pub fallback: String,
}

/// preedit 상태 조건 — `key_meta.context_alt.when` 으로 사용.
///
/// 두 축으로 분류:
/// - **상태 축**: 현재 `HangulChar`의 cho/jung/jong 채워짐 패턴.
/// - **마지막 자모 축**: composer 큐의 마지막 자모 종류 (도깨비불 등 시퀀스 분기).
///
/// JSON에서는 snake_case로 표기(`"choseong_only"`, `"jongseong_filled"` 등).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ContextCondition {
    // === 상태 축 ===
    /// 조합 중 아님 — preedit 비어 있음 (cho/jung/jong 모두 없음).
    Empty,
    /// 조합 중 — cho/jung/jong 중 하나라도 있음.
    Composing,
    /// 초성 1개만 채워짐 (jung·jong 없음).
    ChoseongOnly,
    /// 중성만 채워짐 (cho 없이 jung만, jong 없음).
    JungseongOnly,
    /// 초성+중성 채워짐, 종성 없음.
    ChoJungFilled,
    /// 종성이 채워진 상태 (cho/jung 동반 여부 무관).
    JongseongFilled,
    // === 마지막 자모 축 (큐 back) ===
    /// 큐의 마지막 자모가 초성.
    LastIsCho,
    /// 큐의 마지막 자모가 중성.
    LastIsJung,
    /// 큐의 마지막 자모가 종성.
    LastIsJong,
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
    /// QWERTY 중간: A-L(;'). `3rd`는 역사적 오기지만 v0와 호환 유지.
    #[serde(rename = "3rd", default)]
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
    /// schema_version 2 — rule_set이 토글하는 키 메타데이터.
    /// active=true일 때 base `key_meta`에 병합 (rule_set 우선). active=false면 무시.
    /// 룰 A(vowel_combine_head)·룰 B(context_alt)를 자판 단위로 켜고 끌 수 있게 하는 표현.
    #[serde(default)]
    pub key_meta: Option<HashMap<String, KeyMeta>>,
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
            || self.key_meta.is_some()
    }
}

// ============================================================================
// 정규화된 런타임 표현
// ============================================================================

/// 정규화된 런타임 프로필 (v1·v2).
///
/// JSON 구조를 1:1로 매핑하되, `rule_sets`의 legacy `reinterpret`만
/// `combinations`로 흡수한다. combinations 해석, inherits 병합,
/// active_rule_sets 적용은 builder에서.
#[derive(Debug, Clone)]
pub struct LayoutProfile {
    /// 1 또는 2. 0.2.0부터 v0(=0)는 거부됨. PR-A에서 v2 신설(`key_meta` 도입).
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
    /// schema_version 2 신규 — 키별 메타데이터. PR-A에서는 dangling(미사용).
    /// 키는 layout 셀과 동일한 컨벤션의 리터럴 문자열(예: `"/"`, `"ᆮ"`).
    pub key_meta: Option<HashMap<String, KeyMeta>>,
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
            key_meta: raw.key_meta,
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
                    "3rd": ["ㅁ","ㄴ","ㅇ","ㄹ","ㅎ","ㅗ","ㅓ","ㅏ","ㅣ",":","\""],
                    "4th": ["ㅋ","ㅌ","ㅊ","ㅍ","ㅠ","ㅜ","ㅡ","<",">","?"]
                },
                "lower": {
                    "1st": ["`","1","2","3","4","5","6","7","8","9","0","-","=","\\"],
                    "2nd": ["ㅂ","ㅈ","ㄷ","ㄱ","ㅅ","ㅛ","ㅕ","ㅑ","ㅐ","ㅔ","[","]"],
                    "3rd": ["ㅁ","ㄴ","ㅇ","ㄹ","ㅎ","ㅗ","ㅓ","ㅏ","ㅣ",";","'"],
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
                "upper": {"1st": [], "2nd": [], "3rd": [], "4th": []},
                "lower": {"1st": [], "2nd": [], "3rd": [], "4th": []}
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
                "upper": {"1st":[],"2nd":[],"3rd":[],"4th":[]},
                "lower": {"1st":[],"2nd":[],"3rd":[],"4th":[]}
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
                    "upper":{"1st":[],"2nd":[],"3rd":[],"4th":[]},
                    "lower":{"1st":[],"2nd":[],"3rd":[],"4th":[]}
                }
            }"#,
        )
        .unwrap();
        let profile = LayoutProfile::from_raw(raw);
        let desc = profile.metadata.description.as_ref().unwrap();
        assert_eq!(desc.resolve("ko"), "단일");
        assert_eq!(desc.resolve("xx"), "단일");
    }

    // ========================================================================
    // schema_version 2 — key_meta (PR-A)
    // ========================================================================

    /// schema_version 2 + 키 메타데이터를 포함한 JSON이 LayoutProfile로 정상 파싱.
    /// PR-A에서는 dangling 필드이므로 동작 영향 없음 — 파싱 성공만 검증.
    #[test]
    fn schema_v2_key_meta_parses_successfully() {
        let json = r#"{
            "schema_version": 2,
            "language": "korean",
            "name": "v2_test",
            "type": "3bul",
            "layout": {
                "upper": {"1st":[],"2nd":[],"3rd":[],"4th":[]},
                "lower": {"1st":[],"2nd":[],"3rd":[],"4th":[]}
            },
            "combinations": {"cho":[],"jung":[],"jong":[]},
            "key_meta": {
                "/": {
                    "vowel_combine_head": true,
                    "context_alt": {
                        "when": "choseong_only",
                        "to": "ㅗ",
                        "fallback": "/"
                    }
                },
                "9": {
                    "vowel_combine_head": true
                }
            }
        }"#;
        let raw: RawProfile = serde_json::from_str(json).unwrap();
        let profile = LayoutProfile::from_raw(raw);
        assert_eq!(profile.schema_version, 2);
        let km = profile.key_meta.as_ref().expect("key_meta present");
        let slash = km.get("/").expect("'/' key meta present");
        assert_eq!(slash.vowel_combine_head, Some(true));
        let alt = slash.context_alt.as_ref().expect("context_alt present");
        assert_eq!(alt.when, ContextCondition::ChoseongOnly);
        assert_eq!(alt.to, "ㅗ");
        assert_eq!(alt.fallback, "/");
        let nine = km.get("9").expect("'9' key meta present");
        assert_eq!(nine.vowel_combine_head, Some(true));
        assert!(nine.context_alt.is_none());
    }

    /// `KeyMeta`/`ContextAlt`/`ContextCondition` round-trip 직렬화/역직렬화.
    /// `skip_serializing_if = "Option::is_none"` 적용 검증 포함.
    #[test]
    fn key_meta_round_trip_serde() {
        let original = KeyMeta {
            vowel_combine_head: Some(true),
            context_alt: Some(ContextAlt {
                when: ContextCondition::ChoseongOnly,
                to: "ㅗ".to_string(),
                fallback: "/".to_string(),
            }),
        };
        let json = serde_json::to_string(&original).unwrap();
        // snake_case 직렬화 확인 — `choseong_only`로 출력되어야 함.
        assert!(
            json.contains(r#""when":"choseong_only""#),
            "ContextCondition::ChoseongOnly는 snake_case로 직렬화: got {json}"
        );
        let decoded: KeyMeta = serde_json::from_str(&json).unwrap();
        assert_eq!(decoded.vowel_combine_head, Some(true));
        let alt = decoded.context_alt.expect("round-trip preserves context_alt");
        assert_eq!(alt.when, ContextCondition::ChoseongOnly);
        assert_eq!(alt.to, "ㅗ");
        assert_eq!(alt.fallback, "/");

        // 빈 KeyMeta는 두 필드 모두 생략되어야 함 (skip_serializing_if).
        let empty = KeyMeta::default();
        let empty_json = serde_json::to_string(&empty).unwrap();
        assert_eq!(
            empty_json, "{}",
            "default KeyMeta는 빈 객체로 직렬화: got {empty_json}"
        );
    }

    /// `key_meta` 부재인 기존 v1 자판 JSON은 그대로 파싱되어야 한다 (default None).
    #[test]
    fn v1_without_key_meta_still_parses() {
        let raw: RawProfile = serde_json::from_str(v1_json()).unwrap();
        let profile = LayoutProfile::from_raw(raw);
        assert_eq!(profile.schema_version, 1);
        assert!(
            profile.key_meta.is_none(),
            "v1 JSON에서는 key_meta 누락 → None"
        );
    }

    /// `ContextCondition` 9개 변이체가 모두 snake_case JSON으로 round-trip되는지.
    #[test]
    fn context_condition_all_variants_round_trip() {
        let cases = [
            (ContextCondition::Empty, "\"empty\""),
            (ContextCondition::Composing, "\"composing\""),
            (ContextCondition::ChoseongOnly, "\"choseong_only\""),
            (ContextCondition::JungseongOnly, "\"jungseong_only\""),
            (ContextCondition::ChoJungFilled, "\"cho_jung_filled\""),
            (ContextCondition::JongseongFilled, "\"jongseong_filled\""),
            (ContextCondition::LastIsCho, "\"last_is_cho\""),
            (ContextCondition::LastIsJung, "\"last_is_jung\""),
            (ContextCondition::LastIsJong, "\"last_is_jong\""),
        ];
        for (variant, json) in cases {
            let serialized = serde_json::to_string(&variant).unwrap();
            assert_eq!(
                serialized, json,
                "직렬화: {variant:?} -> {json}, 실제={serialized}"
            );
            let decoded: ContextCondition = serde_json::from_str(json).unwrap();
            assert_eq!(decoded, variant, "역직렬화: {json} -> {variant:?}");
        }
    }

    /// `ContextCondition`은 `snake_case` 변이체만 허용. 잘못된 값은 파싱 에러.
    #[test]
    fn key_meta_rejects_unknown_when_value() {
        let json = r#"{
            "schema_version": 2,
            "language": "korean",
            "name": "bad",
            "type": "3bul",
            "layout": {
                "upper": {"1st":[],"2nd":[],"3rd":[],"4th":[]},
                "lower": {"1st":[],"2nd":[],"3rd":[],"4th":[]}
            },
            "key_meta": {
                "/": {
                    "context_alt": {
                        "when": "foo",
                        "to": "ㅗ",
                        "fallback": "/"
                    }
                }
            }
        }"#;
        let result: Result<RawProfile, _> = serde_json::from_str(json);
        assert!(
            result.is_err(),
            "unknown `when` 값 'foo'는 파싱 에러여야 함"
        );
    }
}
