//! `LayoutProfile` → `CombinedJamoMap` 빌더.
//!
//! # 범위
//! - `build_combined_jamo_map(profile)` — 기본 `combinations` + 활성 `rule_sets`를
//!   병합해 최종 조합 테이블을 생성.
//! - `combinations`가 `None`이면 (영문 계열처럼) 빈 맵에서 시작.
//!   0.2.0부터는 v0 fallback 경로(Rust const 테이블 클론)를 제거 — v0 프로필은
//!   loader에서 거부된다.
//! - rule_set 엔트리는 `first` 자모 코드포인트로 스코프(cho/jung/jong)를 자동 판별.
//!
//! # 비범위
//! - `inherits` 체인 병합: 현재는 stub(`inherit::resolve`)이 거의 pass-through.
//! - 사용자 디렉토리(`~/.config/unim/layouts`) 스캔.

use std::collections::HashMap;
use std::fmt;

use once_cell::sync::Lazy;

use crate::hangul::composer::CombinedJamoMap;
use crate::hangul::jamo::{Cho, Jamo, JamoEnum, Jong, Jung};

use super::schema::{CombinationsBlock, KeyMeta, LayoutProfile, RawTriple, RuleSet};

// ============================================================================
// BuildError
// ============================================================================

/// 빌더 단계의 오류.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BuildError {
    /// `first`/`second`/`result`가 빈 문자열.
    EmptyJamoField {
        rule_set: Option<String>,
        field: &'static str,
    },
    /// 자모 문자열이 `Cho`/`Jung`/`Jong` 어느 변이체로도 해석되지 않음.
    UnknownJamo {
        text: String,
        expected_scope: &'static str,
        rule_set: Option<String>,
    },
    /// rule_set 엔트리의 `first` 코드포인트가 cho/jung/jong 범위 밖.
    ScopeInferenceFailed { text: String, rule_set: String },
}

impl fmt::Display for BuildError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            BuildError::EmptyJamoField { rule_set, field } => match rule_set {
                Some(n) => write!(f, "rule_set '{n}' entry has empty {field}"),
                None => write!(f, "combinations entry has empty {field}"),
            },
            BuildError::UnknownJamo {
                text,
                expected_scope,
                rule_set,
            } => match rule_set {
                Some(n) => write!(
                    f,
                    "rule_set '{n}': cannot resolve '{text}' as {expected_scope} jamo"
                ),
                None => write!(
                    f,
                    "combinations: cannot resolve '{text}' as {expected_scope} jamo"
                ),
            },
            BuildError::ScopeInferenceFailed { text, rule_set } => write!(
                f,
                "rule_set '{rule_set}': cannot infer scope of '{text}' (codepoint out of range)"
            ),
        }
    }
}

impl std::error::Error for BuildError {}

// ============================================================================
// 자모 역방향 조회 (char → Cho/Jung/Jong)
// ============================================================================

/// `Cho::from_sequence`을 0부터 순차 호출해 모든 변이체에 대해
/// `get_unicode()`(조합형)·`get_unicode_compat()`(호환) 두 문자를 키로 등록.
static CHO_BY_CHAR: Lazy<HashMap<char, Cho>> = Lazy::new(|| {
    let mut m = HashMap::new();
    for i in 0..256 {
        if let Some(c) = Cho::from_sequence(i) {
            m.insert(c.get_unicode(), c);
            m.insert(c.get_unicode_compat(), c);
        } else {
            break;
        }
    }
    m
});

static JUNG_BY_CHAR: Lazy<HashMap<char, Jung>> = Lazy::new(|| {
    let mut m = HashMap::new();
    for i in 0..256 {
        if let Some(j) = Jung::from_sequence(i) {
            m.insert(j.get_unicode(), j);
            m.insert(j.get_unicode_compat(), j);
        } else {
            break;
        }
    }
    m
});

static JONG_BY_CHAR: Lazy<HashMap<char, Jong>> = Lazy::new(|| {
    let mut m = HashMap::new();
    for i in 0..256 {
        if let Some(j) = Jong::from_sequence(i) {
            m.insert(j.get_unicode(), j);
            m.insert(j.get_unicode_compat(), j);
        } else {
            break;
        }
    }
    m
});

// ============================================================================
// 헬퍼: 문자열 → 자모
// ============================================================================

fn first_char_of(s: &str, field: &'static str, rule_set: Option<&str>) -> Result<char, BuildError> {
    s.chars().next().ok_or_else(|| BuildError::EmptyJamoField {
        rule_set: rule_set.map(String::from),
        field,
    })
}

fn parse_cho(s: &str, rule_set: Option<&str>) -> Result<Cho, BuildError> {
    let c = first_char_of(s, "first/second/result", rule_set)?;
    CHO_BY_CHAR
        .get(&c)
        .copied()
        .ok_or_else(|| BuildError::UnknownJamo {
            text: s.to_string(),
            expected_scope: "cho",
            rule_set: rule_set.map(String::from),
        })
}

fn parse_jung(s: &str, rule_set: Option<&str>) -> Result<Jung, BuildError> {
    let c = first_char_of(s, "first/second/result", rule_set)?;
    JUNG_BY_CHAR
        .get(&c)
        .copied()
        .ok_or_else(|| BuildError::UnknownJamo {
            text: s.to_string(),
            expected_scope: "jung",
            rule_set: rule_set.map(String::from),
        })
}

fn parse_jong(s: &str, rule_set: Option<&str>) -> Result<Jong, BuildError> {
    let c = first_char_of(s, "first/second/result", rule_set)?;
    JONG_BY_CHAR
        .get(&c)
        .copied()
        .ok_or_else(|| BuildError::UnknownJamo {
            text: s.to_string(),
            expected_scope: "jong",
            rule_set: rule_set.map(String::from),
        })
}

// ============================================================================
// 스코프 추론 (rule_set 엔트리용)
// ============================================================================

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Scope {
    Cho,
    Jung,
    Jong,
}

/// 유니코드 코드포인트로 scope를 판별. 모호하면 `None`.
///
/// 규약 (`LAYOUT_PROFILE_V1.md` §3.3):
/// - 초성: U+1100–U+1112 (조합형) / U+3131–U+314E (호환 자음).
/// - 중성: U+1161–U+1175 (조합형) / U+314F–U+3163 (호환 모음).
/// - 종성: **U+11A8–U+11C2만 허용** (호환 자모 불허).
///
/// 호환 자음 영역(U+3131–U+314E)은 Cho로 간주 — 현 v1 프로필 구현상
/// rule_set 내에서 이 영역이 종성으로 쓰일 일이 없음. 종성 쓰려면 반드시 U+11xx.
fn infer_scope(c: char) -> Option<Scope> {
    let cp = c as u32;
    match cp {
        0x1100..=0x1112 => Some(Scope::Cho),
        0x1161..=0x1175 => Some(Scope::Jung),
        0x11A8..=0x11C2 => Some(Scope::Jong),
        0x3131..=0x314E => Some(Scope::Cho),
        0x314F..=0x3163 => Some(Scope::Jung),
        _ => None,
    }
}

// ============================================================================
// 활성 rule_sets 해소
// ============================================================================

/// 프로필의 `active_rule_sets` 설정을 해석해 실제 활성화할 rule_set 이름 목록을 반환.
///
/// - `active_rule_sets = Some(list)`: 목록에 있는 이름만 active. `rule_sets`에 없는 이름은
///   조용히 drop.
/// - `active_rule_sets = None`: 각 `rule_sets.<name>.active` 값을 그대로 사용.
///
/// 빈 목록(`Some(vec![])`)은 "모든 rule_set off"를 의미 — 단순·직관적.
pub fn resolve_active_rule_set_names(profile: &LayoutProfile) -> Vec<String> {
    if let Some(list) = &profile.active_rule_sets {
        list.iter()
            .filter(|name| profile.rule_sets.contains_key(name.as_str()))
            .cloned()
            .collect()
    } else {
        profile
            .rule_sets
            .iter()
            .filter(|(_, rs)| rs.active)
            .map(|(name, _)| name.clone())
            .collect()
    }
}

// ============================================================================
// 공개 API: build_combined_jamo_map
// ============================================================================

/// 프로필로부터 최종 `CombinedJamoMap`을 생성.
///
/// 흐름:
/// 1. 기본 조합 — `profile.combinations`가 `Some`이면 그 값에서 파싱.
///    `None`이면 빈 맵(영문 계열). 0.2.0부터 v0 fallback(Rust const 클론)은 제거됨.
/// 2. 활성 rule_sets의 각 엔트리를 스코프 추론 후 map에 insert.
///    중복 키는 rule_set 쪽이 덮어쓴다(`LAYOUT_PROFILE_V1.md` §11).
/// 프로필의 `key_meta`를 char 키 기준 runtime 맵으로 변환합니다.
///
/// JSON의 키는 단일 문자만 인정 (영어 자판 char). 다중 문자 키 항목은 무시.
/// `key_meta` 자체가 없으면 빈 맵 반환.
pub fn build_key_meta_char_map(profile: &LayoutProfile) -> HashMap<char, KeyMeta> {
    let mut map: HashMap<char, KeyMeta> = HashMap::new();
    let Some(ref raw) = profile.key_meta else {
        return map;
    };
    for (k, meta) in raw {
        let mut chars = k.chars();
        if let (Some(c), None) = (chars.next(), chars.next()) {
            map.insert(c, meta.clone());
        }
    }
    map
}

pub fn build_combined_jamo_map(profile: &LayoutProfile) -> Result<CombinedJamoMap, BuildError> {
    let mut map = match &profile.combinations {
        None => CombinedJamoMap::new(),
        Some(block) => build_from_block(block)?,
    };

    let active = resolve_active_rule_set_names(profile);
    for name in &active {
        let rs = profile.rule_sets.get(name).expect("filtered by resolve");
        apply_rule_set(&mut map, name, rs)?;
    }

    Ok(map)
}

fn build_from_block(block: &CombinationsBlock) -> Result<CombinedJamoMap, BuildError> {
    let mut m = CombinedJamoMap::new();
    for t in &block.cho {
        let (a, b, c) = parse_cho_triple(t, None)?;
        m.insert((JamoEnum::Cho(a), JamoEnum::Cho(b)), JamoEnum::Cho(c));
    }
    for t in &block.jung {
        let (a, b, c) = parse_jung_triple(t, None)?;
        m.insert((JamoEnum::Jung(a), JamoEnum::Jung(b)), JamoEnum::Jung(c));
    }
    for t in &block.jong {
        let (a, b, c) = parse_jong_triple(t, None)?;
        m.insert((JamoEnum::Jong(a), JamoEnum::Jong(b)), JamoEnum::Jong(c));
    }
    Ok(m)
}

fn parse_cho_triple(t: &RawTriple, rs: Option<&str>) -> Result<(Cho, Cho, Cho), BuildError> {
    Ok((
        parse_cho(&t.first, rs)?,
        parse_cho(&t.second, rs)?,
        parse_cho(&t.result, rs)?,
    ))
}

fn parse_jung_triple(t: &RawTriple, rs: Option<&str>) -> Result<(Jung, Jung, Jung), BuildError> {
    Ok((
        parse_jung(&t.first, rs)?,
        parse_jung(&t.second, rs)?,
        parse_jung(&t.result, rs)?,
    ))
}

fn parse_jong_triple(t: &RawTriple, rs: Option<&str>) -> Result<(Jong, Jong, Jong), BuildError> {
    Ok((
        parse_jong(&t.first, rs)?,
        parse_jong(&t.second, rs)?,
        parse_jong(&t.result, rs)?,
    ))
}

fn apply_rule_set(map: &mut CombinedJamoMap, name: &str, rs: &RuleSet) -> Result<(), BuildError> {
    for t in &rs.combinations {
        let first_ch = first_char_of(&t.first, "first", Some(name))?;
        let scope = infer_scope(first_ch).ok_or_else(|| BuildError::ScopeInferenceFailed {
            text: t.first.clone(),
            rule_set: name.to_string(),
        })?;
        match scope {
            Scope::Cho => {
                let (a, b, c) = parse_cho_triple(t, Some(name))?;
                map.insert((JamoEnum::Cho(a), JamoEnum::Cho(b)), JamoEnum::Cho(c));
            }
            Scope::Jung => {
                let (a, b, c) = parse_jung_triple(t, Some(name))?;
                map.insert((JamoEnum::Jung(a), JamoEnum::Jung(b)), JamoEnum::Jung(c));
            }
            Scope::Jong => {
                let (a, b, c) = parse_jong_triple(t, Some(name))?;
                map.insert((JamoEnum::Jong(a), JamoEnum::Jong(b)), JamoEnum::Jong(c));
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hangul::composer::CombinedJamoMap;
    use crate::keystroke::profile::builtin::BUILTIN_NAMES;
    use crate::keystroke::profile::loader::load_builtin_profile;

    /// 재사용용: 두벌식 표준 조합 규칙(중성 7개 + 종성 11개). 0.2.0부터 const 테이블이
    /// 사라지므로 cross-check용 ground truth를 인라인으로 둔다.
    fn expected_2bul_combinations() -> CombinedJamoMap {
        use crate::hangul::jamo::{JamoEnum, Jong, Jung};
        let mut m = HashMap::new();
        // 중성 7
        let jung = [
            (Jung::O, Jung::A, Jung::Wa),
            (Jung::O, Jung::Ae, Jung::Wae),
            (Jung::O, Jung::I, Jung::Oe),
            (Jung::U, Jung::Eo, Jung::Weo),
            (Jung::U, Jung::E, Jung::We),
            (Jung::U, Jung::I, Jung::Wi),
            (Jung::Eu, Jung::I, Jung::Yi),
        ];
        for (a, b, c) in jung {
            m.insert((JamoEnum::Jung(a), JamoEnum::Jung(b)), JamoEnum::Jung(c));
        }
        // 종성 11
        let jong = [
            (Jong::Giyeok, Jong::Giyeok, Jong::SsangGiyeok),
            (Jong::Giyeok, Jong::Siot, Jong::GiyeokSiot),
            (Jong::Nieun, Jong::Jieut, Jong::NieunJieut),
            (Jong::Nieun, Jong::Hieuh, Jong::NieunHieuh),
            (Jong::Rieul, Jong::Giyeok, Jong::RieulGiyeok),
            (Jong::Rieul, Jong::Mieum, Jong::RieulMieum),
            (Jong::Rieul, Jong::Bieup, Jong::RieulBieup),
            (Jong::Rieul, Jong::Siot, Jong::RieulSiot),
            (Jong::Rieul, Jong::Tieut, Jong::RieulTieut),
            (Jong::Rieul, Jong::Pieup, Jong::RieulPieup),
            (Jong::Rieul, Jong::Hieuh, Jong::RieulHieuh),
            (Jong::Bieup, Jong::Siot, Jong::BieupSiot),
            (Jong::Siot, Jong::Siot, Jong::SsangSiot),
        ];
        for (a, b, c) in jong {
            m.insert((JamoEnum::Jong(a), JamoEnum::Jong(b)), JamoEnum::Jong(c));
        }
        m
    }

    /// 재사용용: 3벌식 base 조합 규칙(중성 7 + 종성 11 + 초성 5).
    fn expected_3bul_base_combinations() -> CombinedJamoMap {
        use crate::hangul::jamo::{Cho, JamoEnum};
        let mut m = expected_2bul_combinations();
        // 초성 5 (3벌식 전용)
        let cho = [
            (Cho::Giyeok, Cho::Giyeok, Cho::SsangGiyeok),
            (Cho::Digeut, Cho::Digeut, Cho::SsangDigeut),
            (Cho::Bieup, Cho::Bieup, Cho::SsangBieup),
            (Cho::Siot, Cho::Siot, Cho::SsangSiot),
            (Cho::Jieut, Cho::Jieut, Cho::SsangJieut),
        ];
        for (a, b, c) in cho {
            m.insert((JamoEnum::Cho(a), JamoEnum::Cho(b)), JamoEnum::Cho(c));
        }
        m
    }

    #[test]
    fn builtin_2bul_matches_expected_table() {
        // 0.2.0: const 테이블이 사라졌으므로 인라인 ground truth와 cross-check.
        let profile = load_builtin_profile("ko_2bulstd").unwrap();
        let map = build_combined_jamo_map(&profile).unwrap();
        assert_eq!(
            map,
            expected_2bul_combinations(),
            "ko_2bulstd v1 JSON은 두벌식 기본 조합 규칙과 일치해야 함"
        );
    }

    #[test]
    fn builtin_3bul_matches_expected_table() {
        // ko_3bul_qwerty는 고유 조합이라 제외 (쿼티형 세벌식은 base와 다름).
        let expected = expected_3bul_base_combinations();
        for name in &["ko_3bul390", "ko_3bul391", "ko_3bul_noshift"] {
            let profile = load_builtin_profile(name).unwrap();
            let map = build_combined_jamo_map(&profile).unwrap();
            assert_eq!(
                map, expected,
                "{name}: v1 3bul JSON은 3벌식 기본 조합 규칙과 일치해야 함"
            );
        }
    }

    #[test]
    fn english_builtins_have_empty_combinations() {
        for name in &[
            "en_qwerty",
            "en_dvorak",
            "en_colemak",
            "en_colemak_dh",
            "en_workman",
        ] {
            let profile = load_builtin_profile(name).unwrap();
            let map = build_combined_jamo_map(&profile).unwrap();
            assert!(map.is_empty(), "{name}: 영문 프로필은 조합 규칙 없음");
        }
    }

    #[test]
    fn all_builtins_build_without_error() {
        for name in BUILTIN_NAMES {
            let profile = load_builtin_profile(name).unwrap();
            let result = build_combined_jamo_map(&profile);
            assert!(result.is_ok(), "{name}: build 실패 — {:?}", result.err());
        }
    }

    #[test]
    fn v1_self_contained_cho_combination() {
        let json = r#"{
            "schema_version": 1,
            "language": "korean",
            "name": "t",
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
        let profile = crate::keystroke::profile::parse_profile_str(json).unwrap();
        let map = build_combined_jamo_map(&profile).unwrap();
        let key = (JamoEnum::Cho(Cho::Giyeok), JamoEnum::Cho(Cho::Giyeok));
        assert_eq!(map.get(&key), Some(&JamoEnum::Cho(Cho::SsangGiyeok)));
    }

    #[test]
    fn active_rule_set_applies_combinations() {
        let json = r#"{
            "schema_version": 1,
            "language": "korean",
            "name": "t",
            "type": "3bul",
            "layout": {
                "upper": {"1st":[],"2nd":[],"3nd":[],"4th":[]},
                "lower": {"1st":[],"2nd":[],"3nd":[],"4th":[]}
            },
            "combinations": {"cho":[],"jung":[],"jong":[]},
            "rule_sets": {
                "extra": {
                    "active": true,
                    "combinations": [
                        {"first":"ᆨ","second":"ᇂ","result":"ᆿ"}
                    ]
                }
            }
        }"#;
        let profile = crate::keystroke::profile::parse_profile_str(json).unwrap();
        let map = build_combined_jamo_map(&profile).unwrap();
        let key = (JamoEnum::Jong(Jong::Giyeok), JamoEnum::Jong(Jong::Hieuh));
        assert_eq!(
            map.get(&key),
            Some(&JamoEnum::Jong(Jong::Kieuk)),
            "rule_set의 (ᆨ,ᇂ)→ᆿ이 jong scope로 판별되어 map에 들어가야 함"
        );
    }

    #[test]
    fn inactive_rule_set_is_skipped() {
        let json = r#"{
            "schema_version": 1,
            "language": "korean",
            "name": "t",
            "type": "3bul",
            "layout": {
                "upper": {"1st":[],"2nd":[],"3nd":[],"4th":[]},
                "lower": {"1st":[],"2nd":[],"3nd":[],"4th":[]}
            },
            "combinations": {"cho":[],"jung":[],"jong":[]},
            "rule_sets": {
                "off": {
                    "active": false,
                    "combinations": [{"first":"ᆨ","second":"ᇂ","result":"ᆿ"}]
                }
            }
        }"#;
        let profile = crate::keystroke::profile::parse_profile_str(json).unwrap();
        let map = build_combined_jamo_map(&profile).unwrap();
        assert!(map.is_empty(), "active=false rule_set은 반영되지 않아야 함");
    }

    #[test]
    fn active_rule_sets_override_takes_precedence() {
        // 개별 active=true 인 세트를 override로 강제 비활성화.
        let json = r#"{
            "schema_version": 1,
            "language": "korean",
            "name": "t",
            "type": "3bul",
            "layout": {
                "upper": {"1st":[],"2nd":[],"3nd":[],"4th":[]},
                "lower": {"1st":[],"2nd":[],"3nd":[],"4th":[]}
            },
            "combinations": {"cho":[],"jung":[],"jong":[]},
            "rule_sets": {
                "a": {"active": true, "combinations": [{"first":"ᆨ","second":"ᇂ","result":"ᆿ"}]},
                "b": {"active": true, "combinations": [{"first":"ᆫ","second":"ᇂ","result":"ᆭ"}]}
            },
            "active_rule_sets": ["a"]
        }"#;
        let profile = crate::keystroke::profile::parse_profile_str(json).unwrap();
        let map = build_combined_jamo_map(&profile).unwrap();
        let ka = (JamoEnum::Jong(Jong::Giyeok), JamoEnum::Jong(Jong::Hieuh));
        let kb = (JamoEnum::Jong(Jong::Nieun), JamoEnum::Jong(Jong::Hieuh));
        assert!(map.contains_key(&ka), "a는 active_rule_sets 포함");
        assert!(!map.contains_key(&kb), "b는 제외 — override 강제");
    }

    #[test]
    fn empty_active_rule_sets_means_all_off() {
        let json = r#"{
            "schema_version": 1,
            "language": "korean",
            "name": "t",
            "type": "3bul",
            "layout": {
                "upper": {"1st":[],"2nd":[],"3nd":[],"4th":[]},
                "lower": {"1st":[],"2nd":[],"3nd":[],"4th":[]}
            },
            "combinations": {"cho":[],"jung":[],"jong":[]},
            "rule_sets": {
                "a": {"active": true, "combinations": [{"first":"ᆨ","second":"ᇂ","result":"ᆿ"}]}
            },
            "active_rule_sets": []
        }"#;
        let profile = crate::keystroke::profile::parse_profile_str(json).unwrap();
        let map = build_combined_jamo_map(&profile).unwrap();
        assert!(
            map.is_empty(),
            "빈 active_rule_sets는 모든 rule_set off를 의미"
        );
    }

    #[test]
    fn unknown_jamo_returns_error() {
        let json = r#"{
            "schema_version": 1,
            "language": "korean",
            "name": "t",
            "type": "3bul",
            "layout": {
                "upper": {"1st":[],"2nd":[],"3nd":[],"4th":[]},
                "lower": {"1st":[],"2nd":[],"3nd":[],"4th":[]}
            },
            "combinations": {
                "cho": [{"first":"X","second":"ㄱ","result":"ㄲ"}],
                "jung": [],
                "jong": []
            }
        }"#;
        let profile = crate::keystroke::profile::parse_profile_str(json).unwrap();
        let err = build_combined_jamo_map(&profile).unwrap_err();
        assert!(matches!(err, BuildError::UnknownJamo { .. }));
    }

    #[test]
    fn infer_scope_for_various_codepoints() {
        assert_eq!(
            infer_scope('ㄱ'),
            Some(Scope::Cho),
            "U+3131 compat consonant"
        );
        assert_eq!(infer_scope('ᄀ'), Some(Scope::Cho), "U+1100 choseong");
        assert_eq!(infer_scope('ㅏ'), Some(Scope::Jung), "U+314F compat vowel");
        assert_eq!(infer_scope('ᅡ'), Some(Scope::Jung), "U+1161 jungseong");
        assert_eq!(infer_scope('ᆨ'), Some(Scope::Jong), "U+11A8 jongseong");
        assert_eq!(infer_scope('ᇂ'), Some(Scope::Jong), "U+11C2 jongseong");
        assert_eq!(infer_scope('가'), None, "완성형은 범위 밖");
        assert_eq!(infer_scope('A'), None, "영문자");
    }

    #[test]
    fn jong_reverse_map_includes_u11xx() {
        assert!(JONG_BY_CHAR.contains_key(&'ᆨ'), "U+11A8 jong ᆨ must resolve");
        assert_eq!(JONG_BY_CHAR.get(&'ᆨ'), Some(&Jong::Giyeok));
    }

    #[test]
    fn cho_reverse_map_includes_compat_and_choseong() {
        assert_eq!(CHO_BY_CHAR.get(&'ㄱ'), Some(&Cho::Giyeok), "compat");
        assert_eq!(CHO_BY_CHAR.get(&'ᄀ'), Some(&Cho::Giyeok), "choseong");
    }

    #[test]
    fn composer_new_with_profile_matches_default_new_for_builtin() {
        use crate::hangul::composer::HangulComposer;
        use crate::hangul::composer_with_2bul::HangulComposer2Bul;
        use crate::hangul::composer_with_3bul::HangulComposer3Bul;

        // 2벌식
        let p = load_builtin_profile("ko_2bulstd").unwrap();
        let c_profile = HangulComposer2Bul::new_with_profile(&p).unwrap();
        let c_default = HangulComposer2Bul::new();
        assert_eq!(
            c_profile.get_combined_jamo(),
            c_default.get_combined_jamo(),
            "HangulComposer2Bul::new_with_profile(ko_2bulstd)는 new()와 동일한 combined_jamo"
        );

        // 3벌식
        let p = load_builtin_profile("ko_3bul390").unwrap();
        let c_profile = HangulComposer3Bul::new_with_profile(&p).unwrap();
        let c_default = HangulComposer3Bul::new();
        assert_eq!(
            c_profile.get_combined_jamo(),
            c_default.get_combined_jamo(),
            "HangulComposer3Bul::new_with_profile(ko_3bul390)는 new()와 동일한 combined_jamo"
        );
    }
}
