//! `LayoutProfile` → `CombinedJamoMap` 빌더.
//!
//! # Phase 2 범위
//! - `build_combined_jamo_map(profile)` — 기본 `combinations` + 활성 `rule_sets`를
//!   병합해 최종 조합 테이블을 생성.
//! - `combinations` 필드가 `None`인 v0 프로필은 `layout_type`에 따라 Rust const
//!   (COMBINED_JAMO_2BUL / COMBINED_JAMO_3BUL)를 그대로 클론 — **behavior-preserving**.
//! - rule_set 엔트리는 `first` 자모 코드포인트로 스코프(cho/jung/jong)를 자동 판별.
//!
//! # 비범위 (Phase 3+)
//! - `inherits` 체인 병합: 현재는 stub(`inherit::resolve`)이 거의 pass-through.
//! - 사용자 디렉토리(`~/.config/unim/layouts`) 스캔.

use std::collections::HashMap;
use std::fmt;

use once_cell::sync::Lazy;

use crate::hangul::composer::CombinedJamoMap;
use crate::hangul::composer_with_2bul::COMBINED_JAMO_2BUL;
use crate::hangul::composer_with_3bul::COMBINED_JAMO_3BUL;
use crate::hangul::jamo::{Cho, Jamo, JamoEnum, Jong, Jung};

use super::schema::{CombinationsBlock, LayoutProfile, RawTriple, RuleSet};

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
    ScopeInferenceFailed {
        text: String,
        rule_set: String,
    },
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
///    `None`이면 `profile.layout_type`에 따라 Rust const(COMBINED_JAMO_2BUL /
///    COMBINED_JAMO_3BUL)를 클론(v0 호환 경로).
/// 2. 활성 rule_sets의 각 엔트리를 스코프 추론 후 map에 insert.
///    중복 키는 rule_set 쪽이 덮어쓴다(`LAYOUT_PROFILE_V1.md` §11).
pub fn build_combined_jamo_map(profile: &LayoutProfile) -> Result<CombinedJamoMap, BuildError> {
    let mut map = match &profile.combinations {
        None => fallback_for(&profile.layout_type),
        Some(block) => build_from_block(block)?,
    };

    let active = resolve_active_rule_set_names(profile);
    for name in &active {
        let rs = profile.rule_sets.get(name).expect("filtered by resolve");
        apply_rule_set(&mut map, name, rs)?;
    }

    Ok(map)
}

fn fallback_for(layout_type: &str) -> CombinedJamoMap {
    match layout_type {
        "3bul" => COMBINED_JAMO_3BUL.clone(),
        "2bul" => COMBINED_JAMO_2BUL.clone(),
        // 영문 프로필(qwerty, dvorak 등)은 결합 규칙 없음.
        _ => CombinedJamoMap::new(),
    }
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
    Ok((parse_cho(&t.first, rs)?, parse_cho(&t.second, rs)?, parse_cho(&t.result, rs)?))
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

fn apply_rule_set(
    map: &mut CombinedJamoMap,
    name: &str,
    rs: &RuleSet,
) -> Result<(), BuildError> {
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
    use crate::keystroke::profile::builtin::BUILTIN_NAMES;
    use crate::keystroke::profile::loader::load_builtin_profile;

    #[test]
    fn builtin_2bul_matches_static() {
        // Phase 6: v1 JSON combinations 경로로도 Rust 정적 테이블과 동일 결과.
        let profile = load_builtin_profile("ko_2bulstd").unwrap();
        let map = build_combined_jamo_map(&profile).unwrap();
        assert_eq!(
            map, *COMBINED_JAMO_2BUL,
            "ko_2bulstd v1 JSON은 기존 정적 테이블과 완전 일치해야 함"
        );
    }

    #[test]
    fn builtin_3bul_matches_static() {
        // ko_3bul_qwerty는 고유 조합이라 제외 (쿼티형 세벌식은 base와 다름).
        for name in &["ko_3bul390", "ko_3bul391", "ko_3bul_noshift"] {
            let profile = load_builtin_profile(name).unwrap();
            let map = build_combined_jamo_map(&profile).unwrap();
            assert_eq!(
                map, *COMBINED_JAMO_3BUL,
                "{name}: v1 3bul JSON은 기존 정적 테이블과 일치해야 함"
            );
        }
    }

    #[test]
    fn english_builtins_have_empty_combinations() {
        for name in &["en_qwerty", "en_dvorak", "en_colemak", "en_colemak_dh", "en_workman"] {
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
        assert_eq!(infer_scope('ㄱ'), Some(Scope::Cho), "U+3131 compat consonant");
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
        assert!(
            JONG_BY_CHAR.contains_key(&'ᆨ'),
            "U+11A8 jong ᆨ must resolve"
        );
        assert_eq!(JONG_BY_CHAR.get(&'ᆨ'), Some(&Jong::Giyeok));
    }

    #[test]
    fn cho_reverse_map_includes_compat_and_choseong() {
        assert_eq!(CHO_BY_CHAR.get(&'ㄱ'), Some(&Cho::Giyeok), "compat");
        assert_eq!(CHO_BY_CHAR.get(&'ᄀ'), Some(&Cho::Giyeok), "choseong");
    }

    #[test]
    fn composer_new_with_profile_matches_default_new_for_v0() {
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
            "HangulComposer2Bul::new_with_profile(v0)은 new()와 동일한 combined_jamo"
        );

        // 3벌식
        let p = load_builtin_profile("ko_3bul390").unwrap();
        let c_profile = HangulComposer3Bul::new_with_profile(&p).unwrap();
        let c_default = HangulComposer3Bul::new();
        assert_eq!(
            c_profile.get_combined_jamo(),
            c_default.get_combined_jamo(),
            "HangulComposer3Bul::new_with_profile(v0)은 new()와 동일한 combined_jamo"
        );
    }
}
