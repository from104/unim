//! 한자 즐겨찾기 저장소
//!
//! 사용자가 팝업에서 스페이스 키로 토글한 한자 후보를 영구 저장하고,
//! 다음 한자 변환 시 즐겨찾기 항목이 후보 목록 상단에 노출되도록 한다.
//!
//! 데이터는 `~/.local/share/unim/hanja-bookmarks.json` 에 JSON으로 저장된다.
//! 포맷: `{ "한글": ["漢", "韓", ...], ... }`

use std::collections::{BTreeMap, BTreeSet};
#[cfg(test)]
use std::path::Path;
use std::path::PathBuf;

/// 한자 즐겨찾기 저장소
///
/// 한글 음절(key) → 즐겨찾기된 한자 문자열 집합(value).
/// 각 토글 시 즉시 디스크에 저장된다.
#[derive(Debug, Default)]
pub struct HanjaBookmarkStore {
    /// 한글 → 즐겨찾기된 한자 집합
    entries: BTreeMap<String, BTreeSet<String>>,
    /// 저장 파일 경로 (None이면 메모리 전용)
    path: Option<PathBuf>,
}

impl HanjaBookmarkStore {
    /// 기본 경로(`~/.local/share/unim/hanja-bookmarks.json`)에서 로드.
    pub fn load_default() -> Self {
        Self::load_from_path(Self::default_path())
    }

    /// 지정된 경로에서 로드. 파일이 없거나 파싱 실패 시 빈 저장소.
    pub fn load_from_path(path: Option<PathBuf>) -> Self {
        let entries = if let Some(ref p) = path {
            match std::fs::read_to_string(p) {
                Ok(content) => serde_json::from_str::<BTreeMap<String, Vec<String>>>(&content)
                    .map(|raw| {
                        raw.into_iter()
                            .map(|(k, v)| (k, v.into_iter().collect::<BTreeSet<_>>()))
                            .collect()
                    })
                    .unwrap_or_default(),
                Err(_) => BTreeMap::new(),
            }
        } else {
            BTreeMap::new()
        };
        Self { entries, path }
    }

    /// 기본 저장 경로.
    fn default_path() -> Option<PathBuf> {
        crate::paths::data_dir().map(|p| p.join("unim").join("hanja-bookmarks.json"))
    }

    /// 즐겨찾기 여부 조회.
    pub fn is_bookmarked(&self, hangul: &str, hanja: &str) -> bool {
        self.entries
            .get(hangul)
            .is_some_and(|set| set.contains(hanja))
    }

    /// 즐겨찾기 토글. 새 상태(true=등록됨)를 반환하고 파일에 저장.
    pub fn toggle(&mut self, hangul: &str, hanja: &str) -> bool {
        let set = self.entries.entry(hangul.to_string()).or_default();
        let new_state = if set.contains(hanja) {
            set.remove(hanja);
            false
        } else {
            set.insert(hanja.to_string());
            true
        };
        if set.is_empty() {
            self.entries.remove(hangul);
        }
        self.save();
        new_state
    }

    /// 주어진 한글 키에 대한 즐겨찾기 한자 목록 (정렬된 순서).
    pub fn list(&self, hangul: &str) -> Vec<String> {
        self.entries
            .get(hangul)
            .map(|set| set.iter().cloned().collect())
            .unwrap_or_default()
    }

    /// 메모리 상태를 파일에 저장.
    fn save(&self) {
        let Some(ref p) = self.path else {
            return;
        };
        if let Some(parent) = p.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let raw: BTreeMap<&String, Vec<&String>> = self
            .entries
            .iter()
            .map(|(k, v)| (k, v.iter().collect()))
            .collect();
        if let Ok(content) = serde_json::to_string_pretty(&raw) {
            let _ = std::fs::write(p, content);
        }
    }

    /// 테스트용: 경로 설정
    #[cfg(test)]
    fn with_path(path: &Path) -> Self {
        Self::load_from_path(Some(path.to_path_buf()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn temp_path(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("unim-bm-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        dir.join(name)
    }

    #[test]
    fn toggle_adds_and_removes() {
        let path = temp_path("toggle_add_remove.json");
        let _ = std::fs::remove_file(&path);
        let mut store = HanjaBookmarkStore::with_path(&path);

        assert!(!store.is_bookmarked("한", "韓"));
        assert!(store.toggle("한", "韓"));
        assert!(store.is_bookmarked("한", "韓"));
        assert!(!store.toggle("한", "韓"));
        assert!(!store.is_bookmarked("한", "韓"));
    }

    #[test]
    fn persistence_roundtrip() {
        let path = temp_path("roundtrip.json");
        let _ = std::fs::remove_file(&path);
        {
            let mut store = HanjaBookmarkStore::with_path(&path);
            store.toggle("한", "韓");
            store.toggle("한", "漢");
            store.toggle("국", "國");
        }
        let store = HanjaBookmarkStore::with_path(&path);
        assert!(store.is_bookmarked("한", "韓"));
        assert!(store.is_bookmarked("한", "漢"));
        assert!(store.is_bookmarked("국", "國"));
        assert!(!store.is_bookmarked("한", "恨"));
    }

    #[test]
    fn list_returns_bookmarks_for_key() {
        let path = temp_path("list.json");
        let _ = std::fs::remove_file(&path);
        let mut store = HanjaBookmarkStore::with_path(&path);
        store.toggle("한", "韓");
        store.toggle("한", "漢");
        let list = store.list("한");
        assert_eq!(list.len(), 2);
        assert!(list.contains(&"韓".to_string()));
        assert!(list.contains(&"漢".to_string()));
    }

    #[test]
    fn empty_set_is_removed_from_entries() {
        let path = temp_path("empty_cleanup.json");
        let _ = std::fs::remove_file(&path);
        let mut store = HanjaBookmarkStore::with_path(&path);
        store.toggle("한", "韓");
        store.toggle("한", "韓");
        assert!(store.list("한").is_empty());
        // 파일에 빈 엔트리가 남아있지 않아야 재로드 시 깔끔
        let reloaded = HanjaBookmarkStore::with_path(&path);
        assert!(reloaded.list("한").is_empty());
    }

    #[test]
    fn corrupt_file_falls_back_to_empty() {
        let path = temp_path("corrupt.json");
        let mut f = std::fs::File::create(&path).unwrap();
        f.write_all(b"{not valid json").unwrap();
        let store = HanjaBookmarkStore::with_path(&path);
        assert!(!store.is_bookmarked("한", "韓"));
    }
}
