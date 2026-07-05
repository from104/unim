//! AutoTypeFix 사용자 사전 (User Dictionary)
//!
//! 역방향 교정(한글모드 → 영문) 시, 내장 영어 사전에 없거나 `eng_word_min_length`
//! 미만인 단어를 사용자가 직접 whitelist로 등록할 수 있다. 주 용도는 `git`, `ls`,
//! `kubectl`, `cargo` 같은 CLI 명령어·축약어·고유명사.
//!
//! 파일: `~/.config/unim/typefix-userdict.yaml`
//!
//! 블랙리스트([`crate::typefix_blacklist`])와 의미가 반대이므로 파일을 분리한다.
//! 단어는 영문 그 자체만 저장한다 — 키스트로크 버퍼의 `to_ascii_string(english_layout)`
//! 가 사용자의 현재 영문 레이아웃으로 이미 ASCII화해 주므로 레이아웃별 분리가 불필요.

use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

/// 사용자 사전 접근 추상화. [`crate::auto_typefix::check_reverse`]가 사용.
///
/// 테스트에서는 빈 구현으로 쉽게 스텁할 수 있다.
pub trait UserDictGate {
    /// 주어진 단어(lowercase로 정규화됨)가 역방향 사용자 사전에 있는지.
    fn contains_reverse(&self, word_lower: &str) -> bool;
}

/// 빈 사용자 사전 (어떤 단어도 매칭하지 않음). 테스트·초기화 기본값.
#[derive(Debug, Default, Clone, Copy)]
pub struct EmptyUserDict;

impl UserDictGate for EmptyUserDict {
    fn contains_reverse(&self, _word_lower: &str) -> bool {
        false
    }
}

impl UserDictGate for UserDictionary {
    fn contains_reverse(&self, word_lower: &str) -> bool {
        UserDictionary::contains_reverse(self, word_lower)
    }
}

/// 역방향 사전 엔트리.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ReverseWord {
    /// 영문 단어 (대소문자 원본 유지, 매칭은 소문자로).
    pub word: String,
    /// 설명/메모 (선택).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
    /// 추가된 Unix 시각.
    pub added_at: u64,
}

fn default_version() -> u8 {
    1
}

/// 사용자 사전 컨테이너. 파일 R/W + 매칭 + 핫리로드를 담당.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct UserDictionary {
    #[serde(default = "default_version")]
    pub version: u8,
    #[serde(default)]
    pub reverse_words: Vec<ReverseWord>,

    #[serde(skip)]
    last_modified: Option<SystemTime>,
    #[serde(skip)]
    last_checked: Option<SystemTime>,
}

impl Default for UserDictionary {
    fn default() -> Self {
        Self {
            version: 1,
            reverse_words: Vec::new(),
            last_modified: None,
            last_checked: None,
        }
    }
}

fn now_unix() -> u64 {
    SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

fn get_mtime(path: &Path) -> Option<SystemTime> {
    fs::metadata(path).and_then(|m| m.modified()).ok()
}

fn normalize(word: &str) -> String {
    word.trim().to_lowercase()
}

impl UserDictionary {
    /// 기본 경로: `~/.config/unim/typefix-userdict.yaml`
    pub fn default_path() -> Option<PathBuf> {
        crate::paths::config_dir().map(|p| p.join("unim").join("typefix-userdict.yaml"))
    }

    pub fn load_from_path(path: &Path) -> Self {
        match fs::read_to_string(path) {
            Ok(content) => match serde_yaml::from_str::<UserDictionary>(&content) {
                Ok(mut ud) => {
                    ud.last_modified = get_mtime(path);
                    ud.last_checked = Some(SystemTime::now());
                    ud
                }
                Err(e) => {
                    eprintln!("[UNIM] typefix-userdict 파싱 실패: {}. 빈 사전 사용.", e);
                    Self::default()
                }
            },
            Err(_) => Self::default(),
        }
    }

    pub fn load_from_default_path() -> Self {
        match Self::default_path() {
            Some(path) => Self::load_from_path(&path),
            None => Self::default(),
        }
    }

    pub fn save_to_path(&self, path: &Path) -> std::io::Result<()> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let content = serde_yaml::to_string(self)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
        let tmp_path = path.with_extension("yaml.tmp");
        fs::write(&tmp_path, content)?;
        fs::rename(&tmp_path, path)?;
        Ok(())
    }

    pub fn save_to_default_path(&mut self) -> std::io::Result<()> {
        let Some(path) = Self::default_path() else {
            return Err(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                "typefix-userdict 경로를 찾을 수 없음",
            ));
        };
        self.save_to_path(&path)?;
        self.last_modified = get_mtime(&path);
        Ok(())
    }

    /// 파일 변경 시 재로드 (2초 throttle). 외부 편집 또는 CLI/GUI 변경을 반영.
    pub fn reload_if_changed(&mut self) -> bool {
        let now = SystemTime::now();
        if let Some(last) = self.last_checked {
            if let Ok(dur) = now.duration_since(last) {
                if dur.as_secs() < 2 {
                    return false;
                }
            }
        }
        self.last_checked = Some(now);

        let Some(path) = Self::default_path() else {
            return false;
        };
        let Some(current) = get_mtime(&path) else {
            return false;
        };
        let should_reload = match self.last_modified {
            Some(saved) => current > saved,
            None => true,
        };
        if !should_reload {
            return false;
        }

        let reloaded = Self::load_from_path(&path);
        let last_checked = self.last_checked;
        *self = reloaded;
        self.last_checked = last_checked;
        true
    }

    /// 소문자 기준 단어 매칭. 역방향 트리거 핫패스에서 호출되므로 짧게.
    pub fn contains_reverse(&self, word_lower: &str) -> bool {
        self.reverse_words
            .iter()
            .any(|e| e.word.to_lowercase() == word_lower)
    }

    /// 단어 추가. 공백·빈 문자열·비영문은 거부. 중복(대소문자 무시)도 거부.
    /// 성공 시 true.
    pub fn add(&mut self, word: &str, note: Option<String>) -> bool {
        let trimmed = word.trim();
        if trimmed.is_empty() {
            return false;
        }
        if !trimmed.chars().all(|c| c.is_ascii_alphabetic()) {
            return false;
        }
        let key = normalize(trimmed);
        if self
            .reverse_words
            .iter()
            .any(|e| e.word.to_lowercase() == key)
        {
            return false;
        }
        self.reverse_words.push(ReverseWord {
            word: trimmed.to_string(),
            note: note.filter(|n| !n.trim().is_empty()),
            added_at: now_unix(),
        });
        true
    }

    /// 단어 제거 (대소문자 무시). 성공 시 true.
    pub fn remove_by_word(&mut self, word: &str) -> bool {
        let key = normalize(word);
        let before = self.reverse_words.len();
        self.reverse_words.retain(|e| e.word.to_lowercase() != key);
        self.reverse_words.len() != before
    }

    pub fn remove_at(&mut self, idx: usize) -> bool {
        if idx < self.reverse_words.len() {
            self.reverse_words.remove(idx);
            true
        } else {
            false
        }
    }

    /// idx 위치 엔트리를 새 word/note로 갱신. 중복 또는 비영문이면 실패.
    pub fn update_at(&mut self, idx: usize, new_word: &str, new_note: Option<String>) -> bool {
        let trimmed = new_word.trim();
        if trimmed.is_empty() || !trimmed.chars().all(|c| c.is_ascii_alphabetic()) {
            return false;
        }
        let key = normalize(trimmed);
        if self
            .reverse_words
            .iter()
            .enumerate()
            .any(|(i, e)| i != idx && e.word.to_lowercase() == key)
        {
            return false;
        }
        if let Some(e) = self.reverse_words.get_mut(idx) {
            e.word = trimmed.to_string();
            e.note = new_note.filter(|n| !n.trim().is_empty());
            true
        } else {
            false
        }
    }

    pub fn clear(&mut self) {
        self.reverse_words.clear();
    }

    pub fn len(&self) -> usize {
        self.reverse_words.len()
    }

    pub fn is_empty(&self) -> bool {
        self.reverse_words.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn unique_temp_path(name: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        std::env::temp_dir().join(format!(
            "unim-ud-test-{}-{}-{}.yaml",
            std::process::id(),
            nanos,
            name
        ))
    }

    #[test]
    fn add_basic_and_duplicate() {
        let mut ud = UserDictionary::default();
        assert!(ud.add("git", None));
        assert_eq!(ud.len(), 1);
        // 대소문자 무시 중복 거부
        assert!(!ud.add("GIT", Some("dup".into())));
        assert!(!ud.add("git", None));
        assert_eq!(ud.len(), 1);
    }

    #[test]
    fn add_rejects_empty_and_non_alpha() {
        let mut ud = UserDictionary::default();
        assert!(!ud.add("", None));
        assert!(!ud.add("   ", None));
        assert!(!ud.add("git 123", None));
        assert!(!ud.add("git-lfs", None), "하이픈 포함 거부 (영문만)");
        assert!(!ud.add("한글", None));
        assert_eq!(ud.len(), 0);
    }

    #[test]
    fn contains_normalizes_stored_word_to_lower() {
        // 저장된 단어는 대소문자 원본 유지 (예: "Kubectl")이지만 매칭은 소문자 기준.
        // 호출자가 lowercase 정규화 후 조회해야 한다는 계약.
        let mut ud = UserDictionary::default();
        ud.add("Kubectl", None);
        assert!(ud.contains_reverse("kubectl"));
        assert!(
            !ud.contains_reverse("Kubectl"),
            "호출자가 소문자로 정규화해야 함"
        );
    }

    #[test]
    fn remove_by_word_case_insensitive() {
        let mut ud = UserDictionary::default();
        ud.add("Cargo", None);
        assert!(ud.remove_by_word("cargo"));
        assert!(!ud.remove_by_word("cargo"));
        assert!(ud.is_empty());
    }

    #[test]
    fn remove_at_out_of_bounds() {
        let mut ud = UserDictionary::default();
        ud.add("ls", None);
        assert!(!ud.remove_at(5));
        assert!(ud.remove_at(0));
        assert!(ud.is_empty());
    }

    #[test]
    fn update_rejects_duplicate_and_invalid() {
        let mut ud = UserDictionary::default();
        ud.add("git", None);
        ud.add("ls", None);
        // 1번 항목 "ls"를 "git"으로 바꾸려 함 → 중복 거부
        assert!(!ud.update_at(1, "git", None));
        // 비영문 거부
        assert!(!ud.update_at(1, "1ls", None));
        // 정상 변경
        assert!(ud.update_at(1, "less", Some("less pager".into())));
        assert_eq!(ud.reverse_words[1].word, "less");
        assert_eq!(ud.reverse_words[1].note.as_deref(), Some("less pager"));
    }

    #[test]
    fn yaml_round_trip() {
        let path = unique_temp_path("round-trip");
        let mut ud = UserDictionary::default();
        ud.add("git", Some("git command".into()));
        ud.add("kubectl", None);
        ud.save_to_path(&path).unwrap();

        let reloaded = UserDictionary::load_from_path(&path);
        assert_eq!(reloaded.reverse_words.len(), 2);
        assert_eq!(reloaded.reverse_words[0].word, "git");
        assert_eq!(
            reloaded.reverse_words[0].note.as_deref(),
            Some("git command")
        );
        assert_eq!(reloaded.reverse_words[1].word, "kubectl");
        assert!(reloaded.reverse_words[1].note.is_none());
        assert_eq!(reloaded.version, 1);
        let _ = fs::remove_file(&path);
    }

    #[test]
    fn load_missing_file_returns_default() {
        let path = unique_temp_path("missing");
        let ud = UserDictionary::load_from_path(&path);
        assert!(ud.is_empty());
        assert_eq!(ud.version, 1);
    }

    #[test]
    fn empty_user_dict_gate_never_matches() {
        let g = EmptyUserDict;
        assert!(!g.contains_reverse("git"));
        assert!(!g.contains_reverse(""));
    }
}
