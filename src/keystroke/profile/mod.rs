//! 자판 프로필 v1 스키마 — 로더·해석·병합.
//!
//! 전체 기획: `docs/dev/plans/LAYOUT_PROFILE_V1.md`
//! 구현 하네스: `docs/dev/plans/LAYOUT_PROFILE_V1_IMPL.md`
//!
//! # 스키마 정책 (0.2.0+)
//! v0 (legacy) 스키마는 더 이상 지원하지 않는다. JSON에 v1 마커
//! (`schema_version`, `metadata`, `inherits`, `combinations`, `rule_sets`,
//! `active_rule_sets`) 중 하나라도 존재해야 v1으로 인정된다. 모두 부재한 v0
//! JSON은 `LoadError::UnsupportedSchema`로 거부된다.
//!
//! # 구성
//! - `schema` — RawProfile / LayoutProfile serde 타입.
//! - `loader` — JSON 문자열·내장 이름·v0 거부.
//! - `builder` — combinations + 활성 rule_sets 병합 → `CombinedJamoMap`.
//! - `registry` — 내장 + `~/.config/unim/layouts/*.json` 통합 네임스페이스 + mtime hot-reload.
//! - `inherit` — 재귀 체인 해석 + 행·규칙세트·metadata 병합 + 순환 탐지.

pub mod builder;
pub mod builtin;
pub mod inherit;
pub mod loader;
pub mod localized;
pub mod registry;
pub mod schema;

pub use builder::{build_combined_jamo_map, resolve_active_rule_set_names, BuildError};
pub use inherit::{resolve as resolve_inherits, InheritError};
pub use loader::{load_builtin_profile, parse_profile_str, LoadError};
pub use localized::LocalizedText;
pub use registry::{ProfileRegistry, ScanReport};
pub use schema::{
    CombinationsBlock, KeyLayout, LayoutMetadata, LayoutProfile, LayoutRows, RawTriple,
    ReinterpretTriple, RuleSet,
};
