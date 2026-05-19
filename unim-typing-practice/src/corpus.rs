//! 내장 한글 지문 + 사용자 정의 텍스트 슬롯.
//!
//! `data/corpus_ko.txt`의 포맷:
//! ```text
//! ### short
//! 짧은 문장 1
//! 짧은 문장 2
//! ### medium
//! ...
//! ```
//! 같은 라벨 안의 모든 줄은 `\n`으로 합쳐져 한 덩어리로 노출된다.

const BUILTIN: &str = include_str!("../data/corpus_ko.txt");

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CorpusKind {
    Short,
    Medium,
    Long,
}

impl CorpusKind {
    pub fn all() -> [CorpusKind; 3] {
        [Self::Short, Self::Medium, Self::Long]
    }
    pub fn label(&self) -> &'static str {
        match self {
            CorpusKind::Short => "short",
            CorpusKind::Medium => "medium",
            CorpusKind::Long => "long",
        }
    }
    pub fn i18n_key(&self) -> &'static str {
        match self {
            CorpusKind::Short => "corpus_short",
            CorpusKind::Medium => "corpus_medium",
            CorpusKind::Long => "corpus_long",
        }
    }
}

pub fn load(kind: CorpusKind) -> String {
    parse_section(BUILTIN, kind.label())
}

fn parse_section(src: &str, label: &str) -> String {
    let mut out = String::new();
    let mut in_section = false;
    for line in src.lines() {
        if let Some(stripped) = line.strip_prefix("### ") {
            in_section = stripped.trim() == label;
            continue;
        }
        if in_section {
            if !out.is_empty() {
                out.push('\n');
            }
            out.push_str(line);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn short_corpus_is_non_empty() {
        let s = load(CorpusKind::Short);
        assert!(!s.is_empty());
        assert!(s.contains('하') || s.contains('한'));
    }

    #[test]
    fn medium_and_long_are_distinct() {
        assert_ne!(load(CorpusKind::Medium), load(CorpusKind::Long));
    }

    #[test]
    fn unknown_section_yields_empty() {
        assert_eq!(parse_section(BUILTIN, "ghost_section"), "");
    }

    #[test]
    fn parser_handles_simple_file() {
        let src = "### a\nfoo\n### b\nbar\nbaz\n";
        assert_eq!(parse_section(src, "a"), "foo");
        assert_eq!(parse_section(src, "b"), "bar\nbaz");
    }
}
