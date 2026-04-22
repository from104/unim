//! 자판 프로필 v1 스키마 — 로더·해석·병합.
//!
//! 전체 기획: `docs/plans/LAYOUT_PROFILE_V1.md`
//! 구현 하네스: `docs/plans/LAYOUT_PROFILE_V1_IMPL.md`
//!
//! # Phase 1
//! - v0/v1 JSON 구조 정의 (serde).
//! - `schema_version` 판별 + v0 자동 승격.
//! - 내장 9종 로드 경로.
//!
//! # Phase 2 (본 커밋)
//! - `builder::build_combined_jamo_map` — combinations + 활성 rule_sets 병합.
//! - `inherit::resolve` — 현재 `inherits`가 없을 때 pass-through (stub).
//! - `HangulComposer{2,3}Bul::new_with_profile` 통합은 `crate::hangul`에서.
//!
//! 사용자 디렉토리 스캔·inherits 전체 해석은 Phase 3+.

pub mod builder;
pub mod builtin;
pub mod inherit;
pub mod loader;
pub mod localized;
pub mod schema;

pub use builder::{build_combined_jamo_map, resolve_active_rule_set_names, BuildError};
pub use inherit::{resolve as resolve_inherits, InheritError};
pub use loader::{load_builtin_profile, parse_profile_str, LoadError};
pub use localized::LocalizedText;
pub use schema::{
    CombinationsBlock, KeyLayout, LayoutMetadata, LayoutProfile, LayoutRows, RawTriple,
    ReinterpretTriple, RuleSet, SchemaKind,
};
