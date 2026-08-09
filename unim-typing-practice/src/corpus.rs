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
//!
//! 사용자 정의 지문은 `$XDG_CONFIG_HOME/unim-typing-practice/corpora/*.txt` 에
//! 저장된 일반 텍스트. 파일 stem(확장자 제외) 이 곧 표시 이름. 2000 byte 상한.

use std::fs;
use std::path::PathBuf;

const BUILTIN: &str = include_str!("../data/corpus_ko.txt");

/// 사용자 정의 지문 크기 상한 (byte).
pub const USER_CORPUS_MAX_BYTES: usize = 2000;

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

/// 사용자 corpus 1개의 메타 정보.
#[derive(Debug, Clone)]
pub struct UserCorpus {
    /// 파일 stem (확장자 제외) — 드롭다운 표시 이름.
    pub name: String,
    /// 절대 경로.
    pub path: PathBuf,
}

/// 통합 corpus 식별자 — 빌트인 3종 + 사용자 정의 N종.
#[derive(Debug, Clone)]
pub enum CorpusEntry {
    Builtin(CorpusKind),
    User(UserCorpus),
}

impl CorpusEntry {
    /// 드롭다운 표시용 라벨 (이미 번역 적용된 String).
    pub fn display_label(&self) -> String {
        match self {
            CorpusEntry::Builtin(k) => rust_i18n::t!(k.i18n_key()).to_string(),
            CorpusEntry::User(u) => u.name.clone(),
        }
    }
    /// 실제 본문 텍스트.
    pub fn text(&self) -> String {
        match self {
            CorpusEntry::Builtin(k) => load(*k),
            CorpusEntry::User(u) => fs::read_to_string(&u.path).unwrap_or_default(),
        }
    }
}

/// 사용자 corpus 디렉토리 — `$XDG_CONFIG_HOME/unim-typing-practice/corpora/`
/// (`XDG_CONFIG_HOME` 미설정 시 `$HOME/.config/...`).
pub fn user_corpus_dir() -> PathBuf {
    let base = std::env::var("XDG_CONFIG_HOME")
        .ok()
        .filter(|s| !s.is_empty())
        .map(PathBuf::from)
        .or_else(|| std::env::var("HOME").ok().map(|h| PathBuf::from(h).join(".config")))
        .unwrap_or_else(|| PathBuf::from("."));
    base.join("unim-typing-practice").join("corpora")
}

/// 저장된 사용자 corpus 목록 — 이름 사전식 정렬.
pub fn list_user_corpora() -> Vec<UserCorpus> {
    let dir = user_corpus_dir();
    let Ok(rd) = fs::read_dir(&dir) else {
        return Vec::new();
    };
    let mut out: Vec<UserCorpus> = Vec::new();
    for entry in rd.flatten() {
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("txt") {
            continue;
        }
        if let Some(stem) = path.file_stem().and_then(|s| s.to_str()) {
            out.push(UserCorpus {
                name: stem.to_string(),
                path,
            });
        }
    }
    out.sort_by(|a, b| a.name.cmp(&b.name));
    out
}

/// 사용자 corpus 저장. 2000 byte 초과면 거부.
///
/// `name` 은 파일 stem 으로 사용되며 안전 문자(영숫자/한글/`-`/`_`/공백) 만 허용.
/// 공백은 `_` 로 치환. 결과 stem 이 비면 거부.
pub fn save_user_corpus(name: &str, text: &str) -> Result<UserCorpus, String> {
    let bytes = text.len();
    if bytes > USER_CORPUS_MAX_BYTES {
        return Err(format!(
            "size {} bytes exceeds limit {}",
            bytes, USER_CORPUS_MAX_BYTES
        ));
    }
    // 줄바꿈을 \n 으로 통일: Windows(\r\n), Classic Mac(\r) → Unix(\n).
    // 이래야 start_session 의 split('\n') 이 모든 환경에서 의도대로 동작.
    let normalized = text.replace("\r\n", "\n").replace('\r', "\n");
    if normalized.trim().is_empty() {
        return Err("empty text".into());
    }
    let safe = sanitize_corpus_name(name)?;
    let dir = user_corpus_dir();
    fs::create_dir_all(&dir).map_err(|e| format!("mkdir: {e}"))?;
    let path = dir.join(format!("{}.txt", safe));
    fs::write(&path, &normalized).map_err(|e| format!("write: {e}"))?;
    Ok(UserCorpus { name: safe, path })
}

/// 사용자 corpus 이름 변경.
///
/// - `new_name` 은 sanitize_corpus_name 검증 통과 필수.
/// - 새 이름이 기존과 동일하면 no-op.
/// - 새 이름의 파일이 이미 존재하면 거부 (중복).
/// - 본문은 그대로 유지 (재정규화 없음).
pub fn rename_user_corpus(old: &UserCorpus, new_name: &str) -> Result<UserCorpus, String> {
    let safe = sanitize_corpus_name(new_name)?;
    if safe == old.name {
        return Ok(old.clone());
    }
    let dir = user_corpus_dir();
    let new_path = dir.join(format!("{}.txt", safe));
    if new_path.exists() {
        return Err(format!("\"{safe}\" already exists"));
    }
    fs::rename(&old.path, &new_path).map_err(|e| format!("rename: {e}"))?;
    Ok(UserCorpus {
        name: safe,
        path: new_path,
    })
}

/// 사용자 corpus 삭제. 파일이 없으면 Ok(no-op).
pub fn delete_user_corpus(c: &UserCorpus) -> Result<(), String> {
    match fs::remove_file(&c.path) {
        Ok(_) => Ok(()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(format!("remove: {e}")),
    }
}

/// 파일 stem 으로 안전한 이름 산출 — 영숫자/한글/`_`/`-` 만 허용, 공백 → `_`,
/// 최대 64자. 결과가 빈 문자열이면 Err.
fn sanitize_corpus_name(name: &str) -> Result<String, String> {
    let safe: String = name
        .chars()
        .map(|c| if c.is_whitespace() { '_' } else { c })
        .filter(|c| c.is_alphanumeric() || *c == '_' || *c == '-')
        .take(64)
        .collect();
    if safe.is_empty() {
        return Err("invalid name".into());
    }
    Ok(safe)
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
    fn save_rejects_oversize() {
        let big = "가".repeat(USER_CORPUS_MAX_BYTES); // '가' = 3 byte → 6000 byte.
        let res = save_user_corpus("oversize_test", &big);
        assert!(res.is_err(), "2000 byte 초과는 거부");
    }

    #[test]
    fn save_rejects_empty() {
        assert!(save_user_corpus("empty_test", "   \n\t").is_err());
    }

    #[test]
    fn save_normalizes_line_endings() {
        let saved = save_user_corpus("crlf_test", "a\r\nb\rc\nd").expect("ok");
        let read = std::fs::read_to_string(&saved.path).expect("read");
        assert_eq!(read, "a\nb\nc\nd", "\\r\\n 과 \\r 가 \\n 으로 정규화되어야");
        let _ = std::fs::remove_file(&saved.path);
    }

    #[test]
    fn save_sanitizes_name() {
        // 공백 → _, 특수문자 제거. 빈 stem 은 거부.
        let res = save_user_corpus("!!!@@@", "hi");
        assert!(res.is_err(), "안전 문자 없으면 거부");
    }

    #[test]
    fn rename_rejects_duplicate() {
        let _ = save_user_corpus("dup_a", "x").expect("a");
        let b = save_user_corpus("dup_b", "y").expect("b");
        let err = rename_user_corpus(&b, "dup_a");
        assert!(err.is_err(), "기존과 충돌하는 이름은 거부");
        // cleanup
        let _ = delete_user_corpus(&UserCorpus {
            name: "dup_a".into(),
            path: user_corpus_dir().join("dup_a.txt"),
        });
        let _ = delete_user_corpus(&b);
    }

    #[test]
    fn rename_changes_path_and_name() {
        let c = save_user_corpus("ren_src", "hello").expect("save");
        let renamed = rename_user_corpus(&c, "ren_dst").expect("rename");
        assert_eq!(renamed.name, "ren_dst");
        assert!(renamed.path.exists());
        assert!(!c.path.exists(), "원본 파일은 사라져야");
        let _ = delete_user_corpus(&renamed);
    }

    #[test]
    fn delete_is_idempotent_on_missing() {
        let phantom = UserCorpus {
            name: "missing".into(),
            path: user_corpus_dir().join("nonexistent_phantom_corpus.txt"),
        };
        assert!(delete_user_corpus(&phantom).is_ok());
    }

    #[test]
    fn parser_handles_simple_file() {
        let src = "### a\nfoo\n### b\nbar\nbaz\n";
        assert_eq!(parse_section(src, "a"), "foo");
        assert_eq!(parse_section(src, "b"), "bar\nbaz");
    }
}
