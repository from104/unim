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
//! # Phase 2
//! - `builder::build_combined_jamo_map` — combinations + 활성 rule_sets 병합.
//! - `HangulComposer{2,3}Bul::new_with_profile` 통합.
//!
//! # Phase 3 (본 커밋)
//! - `registry::ProfileRegistry` — 내장 + `~/.config/unim/layouts/*.json` 통합 네임스페이스.
//! - mtime 기반 자동 재로드 (`typefix_blacklist.rs` 패턴).
//! - `inherit::resolve` — 재귀 체인 해석 + 행·규칙세트·metadata 병합 + 순환 탐지.
//!
//! Config 필드 연결·CLI·GUI는 Phase 4+.

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
    ReinterpretTriple, RuleSet, SchemaKind,
};
