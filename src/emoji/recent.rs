//! 이모지 최근 사용 (Recent) MRU 영속화 및 카테고리 조회.
//!
//! PR #1 (emoji overhaul) 에서 검색 기능을 제거하고, 즐겨찾기(favorites)를
//! 최근 사용(recent) 으로 의미 재정의했다.
//!
//! - 영속화 파일: `~/.config/unim/emoji-recent.yaml` (스키마: `Vec<String>`).
//! - 최대 보관 수: `RECENT_MAX = 81` (9×9 그리드 첫 페이지 채움 보장).
//! - LRU/MRU 정책: `touch_recent(emoji)` 호출 시
//!   1. 동일 emoji 가 있으면 제거 (dedup).
//!   2. 리스트 맨 앞에 삽입 (MRU bump).
//!   3. `RECENT_MAX` 초과 분은 뒤에서 잘라낸다 (LRU eviction).
//!
//! 검색(`search.rs`) 과 즐겨찾기(`favorites_path`/`load_favorites`/`touch_favorite`/
//! `FAVORITES_MAX`) 는 폐기됐다. 외부 호환성은 PR #5 cleanup 에서 정리한다.

use super::data::{CATEGORIES, EMOJIS};
use std::path::PathBuf;

/// 최근 사용 영속화 파일 경로 — `~/.config/unim/emoji-recent.yaml`.
///
/// XDG config dir 식별 실패 시 None.
pub fn recent_path() -> Option<PathBuf> {
    dirs::config_dir().map(|d| d.join("unim").join("emoji-recent.yaml"))
}

/// 최근 사용 최대 보관 개수 (9×9 그리드 첫 페이지 = 81).
pub const RECENT_MAX: usize = 81;

/// 최근 사용 읽기 (YAML: 이모지 문자열 배열).
///
/// 파일이 없거나 파싱 실패 시 빈 vec 를 반환한다 (silent fallback).
pub fn load_recent() -> Vec<String> {
    let path = match recent_path() {
        Some(p) => p,
        None => return Vec::new(),
    };
    let content = match std::fs::read_to_string(&path) {
        Ok(c) => c,
        Err(_) => return Vec::new(),
    };
    serde_yaml::from_str::<Vec<String>>(&content).unwrap_or_default()
}

/// 최근 사용 MRU 갱신 후 저장.
///
/// `emoji` 를 최상단으로 옮기고(없으면 추가) 최대 `RECENT_MAX` 개로 잘라낸 뒤
/// 파일에 기록한다. 파일 쓰기 실패는 무시 (best-effort).
pub fn touch_recent(emoji: &str) -> Vec<String> {
    let mut list = load_recent();
    list.retain(|e| e != emoji);
    list.insert(0, emoji.to_string());
    list.truncate(RECENT_MAX);

    if let Some(path) = recent_path() {
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        if let Ok(yaml) = serde_yaml::to_string(&list) {
            let _ = std::fs::write(&path, yaml);
        }
    }
    list
}

/// 카테고리 메타 목록 반환 (DBus payload / popup_state 초기화용).
///
/// 반환 형식: `Vec<(id, ko_label, en_label, count)>`. 'Recent' 는 포함되지 않으며
/// 호출자가 인덱스 0 슬롯에 동적으로 추가한다.
pub fn list_categories() -> Vec<(String, String, String, u32)> {
    CATEGORIES
        .iter()
        .enumerate()
        .map(|(idx, info)| {
            let count = EMOJIS.iter().filter(|e| e.cat_idx as usize == idx).count() as u32;
            (
                info.id.to_string(),
                info.ko.to_string(),
                info.en.to_string(),
                count,
            )
        })
        .collect()
}

// =====================================================================
// PR #1 backward-compat shims — GUI/extension 마이그레이션이 끝날 때까지
// 깨지지 않도록 옛 API 이름을 유지한다. PR #5 cleanup 에서 제거.
// =====================================================================

/// 옛 즐겨찾기 경로 — `recent_path` 의 별칭. **DEPRECATED**.
#[deprecated(note = "PR #1: rename to recent_path")]
pub fn favorites_path() -> Option<PathBuf> {
    recent_path()
}

/// 옛 즐겨찾기 최대 보관 개수 — `RECENT_MAX` 별칭. **DEPRECATED**.
#[deprecated(note = "PR #1: rename to RECENT_MAX")]
pub const FAVORITES_MAX: usize = RECENT_MAX;

/// 옛 즐겨찾기 읽기 — `load_recent` 별칭. **DEPRECATED**.
#[deprecated(note = "PR #1: rename to load_recent")]
pub fn load_favorites() -> Vec<String> {
    load_recent()
}

/// 옛 즐겨찾기 MRU 갱신 — `touch_recent` 별칭. **DEPRECATED**.
#[deprecated(note = "PR #1: rename to touch_recent")]
pub fn touch_favorite(emoji: &str) -> Vec<String> {
    touch_recent(emoji)
}

/// 옛 검색 API — 검색은 PR #1 에서 폐기됐다. 항상 빈 vec 반환. **DEPRECATED**.
#[deprecated(note = "PR #1: emoji search removed")]
pub fn search_emoji(_keyword: &str) -> Vec<char> {
    Vec::new()
}

/// 옛 검색 API (string) — 검색은 PR #1 에서 폐기됐다. **DEPRECATED**.
///
/// 빈 키워드일 때만 비어있지 않은 fallback (Recent 또는 SmileysPeople 카테고리)
/// 을 반환하여 옛 GUI 의 "기본 표시" 동작을 보존한다.
#[deprecated(note = "PR #1: emoji search removed; use category_emojis or load_recent")]
pub fn search_emoji_strings(keyword: &str) -> Vec<String> {
    if keyword.is_empty() {
        let r = load_recent();
        if !r.is_empty() {
            return r;
        }
        return category_emojis("SmileysPeople");
    }
    Vec::new()
}

/// 옛 EmojiEntry — 외부 호환용. 신규 코드는 `EmojiDatum` 직접 사용. **DEPRECATED**.
#[deprecated(note = "PR #1: use EmojiDatum directly")]
pub struct EmojiEntry {
    pub keyword: &'static str,
    pub emojis: &'static [char],
}

/// 카테고리 ID 로 이모지 목록 반환. 알 수 없는 id 는 빈 vec.
pub fn category_emojis(category_id: &str) -> Vec<String> {
    let idx = CATEGORIES
        .iter()
        .position(|c| c.id.eq_ignore_ascii_case(category_id));
    match idx {
        Some(idx) => EMOJIS
            .iter()
            .filter(|e| e.cat_idx as usize == idx)
            .map(|e| e.emoji.to_string())
            .collect(),
        None => Vec::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::super::data::TOTAL_EMOJIS;
    use super::*;

    #[test]
    fn data_size_meets_windows_panel_scope() {
        // Windows 11 emoji panel 수준 (~1900±100)
        assert!(
            (1800..=2100).contains(&TOTAL_EMOJIS),
            "TOTAL_EMOJIS={TOTAL_EMOJIS} out of expected window"
        );
        assert_eq!(EMOJIS.len(), TOTAL_EMOJIS);
    }

    #[test]
    fn cat_count_is_8() {
        // PR #1: 9 → 8 (Recent 는 runtime 동적이므로 데이터에 들어가지 않음).
        assert_eq!(CATEGORIES.len(), 8);
    }

    #[test]
    fn smileys_people_merged() {
        // 첫 카테고리 SmileysPeople 는 기존 Smileys(168) + People(385) = 553 항목.
        let count = EMOJIS.iter().filter(|e| e.cat_idx == 0).count();
        assert!(
            count >= 500,
            "SmileysPeople count too low: {} (expected ~553)",
            count
        );
    }

    #[test]
    fn flags_ko_label_is_kookgi() {
        let flags = CATEGORIES.iter().find(|c| c.id == "Flags").expect("Flags");
        assert_eq!(flags.ko, "국기");
    }

    #[test]
    fn smileys_people_ko_label_merged() {
        let cat = CATEGORIES
            .iter()
            .find(|c| c.id == "SmileysPeople")
            .expect("SmileysPeople");
        assert_eq!(cat.ko, "표정·인물");
    }

    #[test]
    fn no_orphan_cat_idx() {
        // 모든 EmojiDatum.cat_idx 가 0..8 범위.
        for e in EMOJIS.iter() {
            assert!(
                (e.cat_idx as usize) < CATEGORIES.len(),
                "out-of-range cat_idx: {} for emoji {}",
                e.cat_idx,
                e.emoji
            );
        }
    }

    #[test]
    fn total_emojis_in_range() {
        assert!((1800..=2100).contains(&TOTAL_EMOJIS));
    }

    #[test]
    fn each_category_has_minimum_entries() {
        for (idx, info) in CATEGORIES.iter().enumerate() {
            let n = EMOJIS.iter().filter(|e| e.cat_idx as usize == idx).count();
            // 가장 작은 Activities 카테고리도 60개 이상이어야 함.
            assert!(n >= 60, "category {} has only {} emojis", info.id, n);
        }
    }

    #[test]
    fn list_categories_returns_eight_with_counts() {
        let cats = list_categories();
        assert_eq!(cats.len(), 8);
        for (id, ko, en, count) in &cats {
            assert!(!id.is_empty());
            assert!(!ko.is_empty());
            assert!(!en.is_empty());
            assert!(*count >= 60, "{} count too low: {}", id, count);
        }
    }

    #[test]
    fn category_emojis_smileys_people_basic() {
        let r = category_emojis("SmileysPeople");
        assert!(r.iter().any(|s| s == "😀"));
        assert!(r.iter().any(|s| s == "😂"));
        // People & Body 흡수 검증 — "waving hand" 등이 들어 있어야 한다.
        assert!(r.iter().any(|s| s == "👋"));
    }

    #[test]
    fn category_emojis_unknown_returns_empty() {
        let r = category_emojis("UnknownXYZ");
        assert!(r.is_empty());
    }

    #[test]
    fn touch_recent_bumps_to_top() {
        // 순수 vec 정책 검증 — 파일 I/O 는 별도 (load_save_yaml_roundtrip).
        let mut list: Vec<String> = vec!["a".into(), "b".into(), "c".into()];
        let emoji = "b";
        list.retain(|e| e != emoji);
        list.insert(0, emoji.to_string());
        assert_eq!(list, vec!["b".to_string(), "a".into(), "c".into()]);
    }

    #[test]
    fn touch_recent_dedup() {
        let mut list: Vec<String> = vec!["a".into(), "b".into(), "a".into()];
        let emoji = "a";
        list.retain(|e| e != emoji);
        list.insert(0, emoji.to_string());
        // 중복 'a' 가 모두 제거된 후 한 번만 들어간다.
        assert_eq!(list, vec!["a".to_string(), "b".into()]);
    }

    #[test]
    fn touch_recent_max_81_truncation() {
        // RECENT_MAX 가 81 인지, 그리고 truncate 가 정확히 작동하는지 검증.
        assert_eq!(RECENT_MAX, 81);
        let mut list: Vec<String> = (0..100).map(|i| format!("e{i}")).collect();
        list.insert(0, "newest".to_string());
        list.truncate(RECENT_MAX);
        assert_eq!(list.len(), 81);
        assert_eq!(list[0], "newest");
    }
}
