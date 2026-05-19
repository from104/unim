//! 다국어 문자열 — 프로필의 description·display_name 등에서 사용.
//!
//! JSON에서 두 가지 형태를 모두 수용한다:
//! - 단일 문자열: `"description": "Some text"`
//! - 다국어 객체: `"description": { "ko": "한국어", "en": "English" }`
//!
//! 단일 문자열은 내부적으로 `{ "default": "..." }` 형태로 저장된다.
//! locale 조회 시 fallback: 요청 locale → `"en"` → `"default"` → 첫 값.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// 다국어 텍스트. serde untagged로 문자열/객체 둘 다 역직렬화.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(untagged)]
pub enum LocalizedText {
    /// 단일 문자열. 내부 표현에서는 `default` 키로 저장한 것과 동일하게 취급.
    Single(String),
    /// locale(ISO 639-1) → 문자열 맵. 예: `{"ko": "한글", "en": "Hangul"}`.
    Map(BTreeMap<String, String>),
}

impl LocalizedText {
    /// 요청 locale에 해당하는 텍스트를 반환.
    ///
    /// Fallback 순서:
    /// 1. 요청 locale 정확히 일치.
    /// 2. `"en"` 키.
    /// 3. `"default"` 키 (단일 문자열 케이스).
    /// 4. 맵의 첫 값.
    /// 5. 빈 문자열.
    pub fn resolve(&self, locale: &str) -> &str {
        match self {
            LocalizedText::Single(s) => s,
            LocalizedText::Map(m) => m
                .get(locale)
                .or_else(|| m.get("en"))
                .or_else(|| m.get("default"))
                .or_else(|| m.values().next())
                .map(String::as_str)
                .unwrap_or(""),
        }
    }

    /// 빈 `LocalizedText` 생성(빈 Single).
    pub fn empty() -> Self {
        LocalizedText::Single(String::new())
    }

    /// 모든 locale이 비어 있는지 검사.
    pub fn is_empty(&self) -> bool {
        match self {
            LocalizedText::Single(s) => s.is_empty(),
            LocalizedText::Map(m) => m.values().all(String::is_empty),
        }
    }
}

impl Default for LocalizedText {
    fn default() -> Self {
        LocalizedText::empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn single_string_resolves_regardless_of_locale() {
        let lt: LocalizedText = serde_json::from_str(r#""hello""#).unwrap();
        assert_eq!(lt.resolve("ko"), "hello");
        assert_eq!(lt.resolve("en"), "hello");
        assert_eq!(lt.resolve("xx"), "hello");
    }

    #[test]
    fn map_resolves_requested_locale() {
        let lt: LocalizedText = serde_json::from_str(r#"{"ko":"한글","en":"Hangul"}"#).unwrap();
        assert_eq!(lt.resolve("ko"), "한글");
        assert_eq!(lt.resolve("en"), "Hangul");
    }

    #[test]
    fn map_falls_back_to_en_then_first() {
        let lt: LocalizedText = serde_json::from_str(r#"{"ja":"日本語","en":"Hangul"}"#).unwrap();
        assert_eq!(lt.resolve("ko"), "Hangul", "ko 없으면 en fallback");

        let lt2: LocalizedText = serde_json::from_str(r#"{"ja":"日本語"}"#).unwrap();
        assert_eq!(lt2.resolve("ko"), "日本語", "en도 없으면 첫 값");
    }

    #[test]
    fn empty_string_is_considered_empty() {
        assert!(LocalizedText::empty().is_empty());
        let lt: LocalizedText = serde_json::from_str(r#""""#).unwrap();
        assert!(lt.is_empty());
    }

    #[test]
    fn default_key_fallback() {
        let lt: LocalizedText = serde_json::from_str(r#"{"default":"기본"}"#).unwrap();
        assert_eq!(lt.resolve("ko"), "기본");
    }
}
