//! `inherits` 체인 해석 + 병합 — `LAYOUT_PROFILE_V1.md` §4 구현.
//!
//! # 동작
//! - `resolve(&profile, &registry)` — child 프로필의 inherits 체인을 재귀 해석 + 병합.
//! - inherits 미설정이면 입력 복제 pass-through.
//! - base 미존재·순환 감지 시 `InheritError`.
//!
//! # 병합 규칙 (§4.1)
//! - `language`/`type`: child 우선. 다르면 경고(본 구현은 조용히 child 채택).
//! - `metadata`: 얕은 병합, child 키가 base를 덮어씀.
//! - `layout.{upper,lower}.{row1..row4}`: **행 단위 덮어쓰기**. child의 행이 비어 있으면 base 유지.
//! - `combinations`: child에 있으면 **자기 완결**(base 상속 안 함). 없으면 base 상속.
//! - `rule_sets`: base + child 합집합. 같은 이름은 **child 덮어쓰기**(§3.5.5).
//! - `active_rule_sets`: child 우선 완전 교체(§3.5.5).
//!
//! # 순환 탐지 (§4.2)
//! 현재 해석 중인 프로필 이름을 `Vec<String>` chain으로 추적. 다시 등장하면 에러.

use std::fmt;

use super::registry::ProfileRegistry;
use super::schema::{KeyLayout, LayoutMetadata, LayoutProfile, LayoutRows};

/// 상속 해석 실패 원인.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InheritError {
    /// base 프로필을 네임스페이스에서 찾지 못함.
    BaseNotFound { child: String, missing_base: String },
    /// inherits 체인에서 순환 감지. 감지된 지점까지의 체인 포함.
    Cycle(Vec<String>),
}

impl fmt::Display for InheritError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            InheritError::BaseNotFound {
                child,
                missing_base,
            } => write!(
                f,
                "inherits base profile '{missing_base}' not found (referenced by '{child}')"
            ),
            InheritError::Cycle(chain) => {
                write!(f, "inherits cycle detected: {}", chain.join(" → "))
            }
        }
    }
}

impl std::error::Error for InheritError {}

/// 프로필의 `inherits` 체인을 해석해 완전히 병합된 프로필을 반환.
///
/// 레지스트리가 제공한 원시 base 프로필들이 재귀적으로 해석되고 병합된다.
pub fn resolve(
    profile: &LayoutProfile,
    registry: &ProfileRegistry,
) -> Result<LayoutProfile, InheritError> {
    let mut chain: Vec<String> = Vec::new();
    resolve_with_chain(profile, registry, &mut chain)
}

fn resolve_with_chain(
    profile: &LayoutProfile,
    registry: &ProfileRegistry,
    chain: &mut Vec<String>,
) -> Result<LayoutProfile, InheritError> {
    // 순환 탐지 — 현재 이름이 이미 체인에 있으면 에러.
    if chain.iter().any(|n| n == &profile.name) {
        let mut reported = chain.clone();
        reported.push(profile.name.clone());
        return Err(InheritError::Cycle(reported));
    }

    let Some(base_name) = profile.inherits.as_ref() else {
        // inherits 없음 — 복제 후 반환.
        return Ok(profile.clone());
    };

    // base를 네임스페이스에서 찾는다.
    let base_raw = registry
        .find_raw(base_name)
        .ok_or_else(|| InheritError::BaseNotFound {
            child: profile.name.clone(),
            missing_base: base_name.clone(),
        })?;

    // 재귀 해석 — base가 또 inherits를 가질 수 있음.
    chain.push(profile.name.clone());
    let resolved_base = resolve_with_chain(&base_raw, registry, chain)?;
    chain.pop();

    Ok(merge(resolved_base, profile.clone()))
}

/// §4.1 병합. `child`의 값이 `base`의 값을 덮는다(필드별 규칙 적용).
fn merge(base: LayoutProfile, child: LayoutProfile) -> LayoutProfile {
    LayoutProfile {
        schema_version: child.schema_version.max(base.schema_version),
        language: child.language,
        name: child.name,
        layout_type: child.layout_type,
        metadata: merge_metadata(base.metadata, child.metadata),
        // inherits는 해석 완료 상태이므로 병합 결과에 남기지 않는다.
        inherits: None,
        layout: merge_layout(base.layout, child.layout),
        combinations: child.combinations.or(base.combinations),
        rule_sets: {
            // base + child 합집합. 같은 이름은 child 우선.
            let mut merged = base.rule_sets;
            for (name, rs) in child.rule_sets {
                merged.insert(name, rs);
            }
            merged
        },
        active_rule_sets: child.active_rule_sets.or(base.active_rule_sets),
    }
}

fn merge_metadata(mut base: LayoutMetadata, child: LayoutMetadata) -> LayoutMetadata {
    if child.display_name.is_some() {
        base.display_name = child.display_name;
    }
    if child.author.is_some() {
        base.author = child.author;
    }
    if child.version.is_some() {
        base.version = child.version;
    }
    if child.license.is_some() {
        base.license = child.license;
    }
    if child.description.is_some() {
        base.description = child.description;
    }
    if child.source_url.is_some() {
        base.source_url = child.source_url;
    }
    if !child.tags.is_empty() {
        base.tags = child.tags;
    }
    base
}

fn merge_layout(base: KeyLayout, child: KeyLayout) -> KeyLayout {
    KeyLayout {
        upper: merge_rows(base.upper, child.upper),
        lower: merge_rows(base.lower, child.lower),
    }
}

/// 행 단위 덮어쓰기 — child 행이 비어 있으면 base 유지.
///
/// 스키마상 row 필드는 `#[serde(default)]`이므로 생략 시 빈 `Vec`으로 들어온다.
/// 진짜 "빈 행"과 "미지정"을 구별하지 못한다는 약점이 있지만, 실용적으로는
/// "모든 키가 없는 행"이란 의미가 없으므로 empty = not specified로 간주한다.
fn merge_rows(base: LayoutRows, child: LayoutRows) -> LayoutRows {
    LayoutRows {
        row1: if child.row1.is_empty() {
            base.row1
        } else {
            child.row1
        },
        row2: if child.row2.is_empty() {
            base.row2
        } else {
            child.row2
        },
        row3: if child.row3.is_empty() {
            base.row3
        } else {
            child.row3
        },
        row4: if child.row4.is_empty() {
            base.row4
        } else {
            child.row4
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::keystroke::profile::loader::parse_profile_str;

    fn reg_builtin_only() -> ProfileRegistry {
        ProfileRegistry::builtin_only()
    }

    #[test]
    fn no_inherits_passes_through_unchanged() {
        for name in crate::keystroke::profile::builtin::BUILTIN_NAMES {
            let profile = crate::keystroke::profile::loader::load_builtin_profile(name).unwrap();
            let resolved = resolve(&profile, &reg_builtin_only()).unwrap();
            assert_eq!(resolved.name, profile.name);
            assert_eq!(resolved.layout_type, profile.layout_type);
            assert_eq!(
                resolved.combinations.is_some(),
                profile.combinations.is_some()
            );
        }
    }

    #[test]
    fn inherits_nonexistent_base_returns_error() {
        let json = r#"{
            "schema_version": 1,
            "language": "korean",
            "name": "child",
            "type": "3bul",
            "inherits": "no_such_base",
            "layout": {
                "upper": {"1st":[],"2nd":[],"3nd":[],"4th":[]},
                "lower": {"1st":[],"2nd":[],"3nd":[],"4th":[]}
            }
        }"#;
        let profile = parse_profile_str(json).unwrap();
        let err = resolve(&profile, &reg_builtin_only()).unwrap_err();
        assert!(matches!(err, InheritError::BaseNotFound { .. }));
    }

    #[test]
    fn inherits_cycle_two_profiles_detected() {
        // 사용자 디렉토리 기반 2단 순환: a → b → a
        let dir = tempdir("inherit-cycle");
        write(
            &dir,
            "a.json",
            r#"{"schema_version":1,"language":"korean","name":"a","type":"3bul",
                "inherits":"b",
                "layout":{"upper":{"1st":[],"2nd":[],"3nd":[],"4th":[]},
                          "lower":{"1st":[],"2nd":[],"3nd":[],"4th":[]}}}"#,
        );
        write(
            &dir,
            "b.json",
            r#"{"schema_version":1,"language":"korean","name":"b","type":"3bul",
                "inherits":"a",
                "layout":{"upper":{"1st":[],"2nd":[],"3nd":[],"4th":[]},
                          "lower":{"1st":[],"2nd":[],"3nd":[],"4th":[]}}}"#,
        );
        let reg = ProfileRegistry::with_user_dir(dir.clone());
        let a = reg.find_raw("a").unwrap();
        let err = resolve(&a, &reg).unwrap_err();
        match err {
            InheritError::Cycle(chain) => {
                assert!(chain.contains(&"a".to_string()));
                assert!(chain.contains(&"b".to_string()));
            }
            e => panic!("expected Cycle, got {e:?}"),
        }
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn inherits_self_cycle_detected() {
        let dir = tempdir("inherit-self");
        write(
            &dir,
            "self.json",
            r#"{"schema_version":1,"language":"korean","name":"self_loop","type":"3bul",
                "inherits":"self_loop",
                "layout":{"upper":{"1st":[],"2nd":[],"3nd":[],"4th":[]},
                          "lower":{"1st":[],"2nd":[],"3nd":[],"4th":[]}}}"#,
        );
        let reg = ProfileRegistry::with_user_dir(dir.clone());
        let p = reg.find_raw("self_loop").unwrap();
        let err = resolve(&p, &reg).unwrap_err();
        assert!(matches!(err, InheritError::Cycle(_)));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn inherits_merges_layout_row_wise() {
        // base: upper.row2에 ["A"], lower 전부. child: lower.row2만 덮어씀.
        let dir = tempdir("row-merge");
        write(
            &dir,
            "base.json",
            r#"{"schema_version":1,"language":"english","name":"row_base","type":"qwerty",
                "layout":{
                    "upper":{"1st":[],"2nd":["A","B"],"3nd":[],"4th":[]},
                    "lower":{"1st":[],"2nd":["a","b"],"3nd":["c"],"4th":[]}}}"#,
        );
        write(
            &dir,
            "child.json",
            r#"{"schema_version":1,"language":"english","name":"row_child","type":"qwerty",
                "inherits":"row_base",
                "layout":{
                    "upper":{"1st":[],"2nd":[],"3nd":[],"4th":[]},
                    "lower":{"1st":[],"2nd":["X","Y"],"3nd":[],"4th":[]}}}"#,
        );
        let reg = ProfileRegistry::with_user_dir(dir.clone());
        let child = reg.find_raw("row_child").unwrap();
        let merged = resolve(&child, &reg).unwrap();
        // child가 덮어쓴 행
        assert_eq!(
            merged.layout.lower.row2,
            vec!["X".to_string(), "Y".to_string()]
        );
        // base에서 상속된 행 (child가 비움 = 유지)
        assert_eq!(
            merged.layout.upper.row2,
            vec!["A".to_string(), "B".to_string()]
        );
        assert_eq!(merged.layout.lower.row3, vec!["c".to_string()]);
        // inherits는 해석 완료 후 None
        assert!(merged.inherits.is_none());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn inherits_combinations_child_wins_self_contained() {
        // base: combinations Some. child: combinations Some → child가 완전 교체.
        let dir = tempdir("combo-child-wins");
        write(
            &dir,
            "base.json",
            r##"{"schema_version":1,"language":"korean","name":"combo_base","type":"3bul",
                 "layout":{"upper":{"1st":[],"2nd":[],"3nd":[],"4th":[]},
                           "lower":{"1st":[],"2nd":[],"3nd":[],"4th":[]}},
                 "combinations":{
                    "cho":[{"first":"ㄱ","second":"ㄱ","result":"ㄲ"}],
                    "jung":[],"jong":[]}}"##,
        );
        write(
            &dir,
            "child.json",
            r##"{"schema_version":1,"language":"korean","name":"combo_child","type":"3bul",
                 "inherits":"combo_base",
                 "layout":{"upper":{"1st":[],"2nd":[],"3nd":[],"4th":[]},
                           "lower":{"1st":[],"2nd":[],"3nd":[],"4th":[]}},
                 "combinations":{
                    "cho":[{"first":"ㄷ","second":"ㄷ","result":"ㄸ"}],
                    "jung":[],"jong":[]}}"##,
        );
        let reg = ProfileRegistry::with_user_dir(dir.clone());
        let child = reg.find_raw("combo_child").unwrap();
        let merged = resolve(&child, &reg).unwrap();
        let combos = merged.combinations.as_ref().unwrap();
        assert_eq!(combos.cho.len(), 1, "child의 combinations가 완전 교체");
        assert_eq!(combos.cho[0].first, "ㄷ");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn inherits_combinations_child_absent_inherits_base() {
        let dir = tempdir("combo-inherit");
        write(
            &dir,
            "base.json",
            r##"{"schema_version":1,"language":"korean","name":"combo_base2","type":"3bul",
                 "layout":{"upper":{"1st":[],"2nd":[],"3nd":[],"4th":[]},
                           "lower":{"1st":[],"2nd":[],"3nd":[],"4th":[]}},
                 "combinations":{
                    "cho":[{"first":"ㄱ","second":"ㄱ","result":"ㄲ"}],
                    "jung":[],"jong":[]}}"##,
        );
        write(
            &dir,
            "child.json",
            r#"{"schema_version":1,"language":"korean","name":"combo_child2","type":"3bul",
                "inherits":"combo_base2",
                "layout":{"upper":{"1st":[],"2nd":[],"3nd":[],"4th":[]},
                          "lower":{"1st":[],"2nd":[],"3nd":[],"4th":[]}}}"#,
        );
        let reg = ProfileRegistry::with_user_dir(dir.clone());
        let child = reg.find_raw("combo_child2").unwrap();
        let merged = resolve(&child, &reg).unwrap();
        let combos = merged.combinations.as_ref().unwrap();
        assert_eq!(combos.cho.len(), 1);
        assert_eq!(combos.cho[0].result, "ㄲ");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn inherits_rule_sets_union_child_overrides_same_name() {
        let dir = tempdir("ruleset-merge");
        write(
            &dir,
            "base.json",
            r#"{"schema_version":1,"language":"korean","name":"rs_base","type":"3bul",
                "layout":{"upper":{"1st":[],"2nd":[],"3nd":[],"4th":[]},
                          "lower":{"1st":[],"2nd":[],"3nd":[],"4th":[]}},
                "rule_sets":{
                    "only_in_base":{"active":true,"combinations":[]},
                    "shared":{"active":false,"combinations":[]}}}"#,
        );
        write(
            &dir,
            "child.json",
            r#"{"schema_version":1,"language":"korean","name":"rs_child","type":"3bul",
                "inherits":"rs_base",
                "layout":{"upper":{"1st":[],"2nd":[],"3nd":[],"4th":[]},
                          "lower":{"1st":[],"2nd":[],"3nd":[],"4th":[]}},
                "rule_sets":{
                    "only_in_child":{"active":true,"combinations":[]},
                    "shared":{"active":true,"combinations":[]}}}"#,
        );
        let reg = ProfileRegistry::with_user_dir(dir.clone());
        let child = reg.find_raw("rs_child").unwrap();
        let merged = resolve(&child, &reg).unwrap();
        assert_eq!(merged.rule_sets.len(), 3, "합집합");
        assert!(merged.rule_sets.contains_key("only_in_base"));
        assert!(merged.rule_sets.contains_key("only_in_child"));
        // 같은 이름은 child 덮어쓰기
        assert!(merged.rule_sets.get("shared").unwrap().active);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn inherits_three_level_chain_resolves() {
        // grandparent → parent → child, 각 단계에서 metadata 덮어쓰기
        let dir = tempdir("three-level");
        write(
            &dir,
            "gp.json",
            r##"{"schema_version":1,"language":"korean","name":"gp","type":"3bul",
                 "metadata":{"author":"gp-author","version":"1.0"},
                 "layout":{"upper":{"1st":[],"2nd":[],"3nd":[],"4th":[]},
                           "lower":{"1st":[],"2nd":[],"3nd":[],"4th":[]}},
                 "combinations":{
                    "cho":[{"first":"ㄱ","second":"ㄱ","result":"ㄲ"}],
                    "jung":[],"jong":[]}}"##,
        );
        write(
            &dir,
            "p.json",
            r#"{"schema_version":1,"language":"korean","name":"p","type":"3bul",
                "inherits":"gp",
                "metadata":{"author":"p-author"},
                "layout":{"upper":{"1st":[],"2nd":[],"3nd":[],"4th":[]},
                          "lower":{"1st":[],"2nd":[],"3nd":[],"4th":[]}}}"#,
        );
        write(
            &dir,
            "c.json",
            r#"{"schema_version":1,"language":"korean","name":"c","type":"3bul",
                "inherits":"p",
                "layout":{"upper":{"1st":[],"2nd":[],"3nd":[],"4th":[]},
                          "lower":{"1st":[],"2nd":[],"3nd":[],"4th":[]}}}"#,
        );
        let reg = ProfileRegistry::with_user_dir(dir.clone());
        let c = reg.find_raw("c").unwrap();
        let merged = resolve(&c, &reg).unwrap();
        // p가 덮어씀
        assert_eq!(merged.metadata.author.as_deref(), Some("p-author"));
        // p가 덮어쓰지 않은 필드는 gp에서 상속
        assert_eq!(merged.metadata.version.as_deref(), Some("1.0"));
        // combinations는 gp→p→c 경로에서 gp만 Some → 전부 상속
        let combos = merged.combinations.as_ref().unwrap();
        assert_eq!(combos.cho.len(), 1);
        let _ = std::fs::remove_dir_all(&dir);
    }

    // ── 테스트 유틸 ─────────────────────────────────────────────

    fn tempdir(name: &str) -> std::path::PathBuf {
        use std::time::{SystemTime, UNIX_EPOCH};
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        let dir = std::env::temp_dir().join(format!(
            "unim-inherit-test-{}-{}-{}",
            std::process::id(),
            nanos,
            name
        ));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn write(dir: &std::path::Path, filename: &str, content: &str) {
        use std::io::Write;
        let mut f = std::fs::File::create(dir.join(filename)).unwrap();
        f.write_all(content.as_bytes()).unwrap();
    }
}
