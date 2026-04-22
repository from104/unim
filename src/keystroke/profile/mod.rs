//! 자판 프로필 v1 스키마 — 로더·해석·병합.
//!
//! 전체 기획: `docs/plans/LAYOUT_PROFILE_V1.md`
//! 구현 하네스: `docs/plans/LAYOUT_PROFILE_V1_IMPL.md`
//!
//! # Phase 1 (본 커밋) 범위
//! - v0/v1 JSON 구조 정의 (serde).
//! - `schema_version` 판별 + v0 자동 승격.
//! - 내장 9종 로드 경로.
//!
//! combinations 병합·inherits 해석·Composer 통합은 이후 Phase에서 추가.

pub mod builtin;
pub mod loader;
pub mod localized;
pub mod schema;

pub use loader::{load_builtin_profile, parse_profile_str, LoadError};
pub use localized::LocalizedText;
pub use schema::{
    CombinationsBlock, KeyLayout, LayoutMetadata, LayoutProfile, LayoutRows, RawTriple,
    ReinterpretTriple, RuleSet, SchemaKind,
};
