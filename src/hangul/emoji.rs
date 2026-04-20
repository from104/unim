//! 이모지 입력 모듈
//!
//! 키워드 기반 이모지 검색, 카테고리 목록, 즐겨찾기 (MRU) 저장소.
//! GTK 팝업은 이 모듈의 API를 통해 데이터를 가져옵니다.

use std::path::PathBuf;

/// 이모지 항목
pub struct EmojiEntry {
    /// 키워드 (검색용, 한글)
    pub keyword: &'static str,
    /// 이모지 문자들
    pub emojis: &'static [char],
}

/// 키워드로 이모지를 검색합니다.
///
/// 한글 키워드가 포함된 카테고리의 이모지를 반환합니다.
/// 빈 키워드면 전체 인기 이모지를 반환합니다.
pub fn search_emoji(keyword: &str) -> Vec<char> {
    if keyword.is_empty() {
        return POPULAR_EMOJIS.to_vec();
    }

    let mut results = Vec::new();
    for entry in EMOJI_TABLE.iter() {
        if entry.keyword.contains(keyword) || keyword.contains(entry.keyword) {
            results.extend_from_slice(entry.emojis);
        }
    }

    if results.is_empty() {
        POPULAR_EMOJIS.to_vec()
    } else {
        results
    }
}

/// 카테고리 목록 (GUI 탭 구성용)
pub fn categories() -> &'static [EmojiEntry] {
    EMOJI_TABLE
}

/// 인기(기본) 이모지 목록
pub fn popular_emojis() -> &'static [char] {
    POPULAR_EMOJIS
}

/// 즐겨찾기 저장 경로
pub fn favorites_path() -> Option<PathBuf> {
    dirs::config_dir().map(|d| d.join("unim").join("emoji-favorites.yaml"))
}

/// 즐겨찾기 최대 개수 (MRU)
pub const FAVORITES_MAX: usize = 32;

/// 즐겨찾기 읽기 (YAML: 이모지 문자열 배열)
///
/// 파일이 없거나 파싱 실패 시 빈 벡터를 반환합니다.
pub fn load_favorites() -> Vec<String> {
    let path = match favorites_path() {
        Some(p) => p,
        None => return Vec::new(),
    };
    let content = match std::fs::read_to_string(&path) {
        Ok(c) => c,
        Err(_) => return Vec::new(),
    };
    serde_yaml::from_str::<Vec<String>>(&content).unwrap_or_default()
}

/// 즐겨찾기 MRU 갱신 후 저장
///
/// `emoji`를 최상단으로 옮기고(없으면 추가) 최대 `FAVORITES_MAX`개로 잘라낸 뒤 파일에 기록합니다.
pub fn touch_favorite(emoji: &str) -> Vec<String> {
    let mut list = load_favorites();
    list.retain(|e| e != emoji);
    list.insert(0, emoji.to_string());
    list.truncate(FAVORITES_MAX);

    if let Some(path) = favorites_path() {
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        if let Ok(yaml) = serde_yaml::to_string(&list) {
            let _ = std::fs::write(&path, yaml);
        }
    }
    list
}

/// 인기 이모지 (기본 표시용)
const POPULAR_EMOJIS: &[char] = &[
    '😀', '😂', '🥲', '😍', '🥰', '😊', '😎', '🤔', '😅', '😭',
    '🙏', '👍', '👎', '❤', '🔥', '✨', '🎉', '💯', '👀', '🤣',
    '😱', '😤', '🥺', '😢', '💪', '🙌', '👏', '🤝', '✅', '❌',
    '⭐', '💡', '📌', '🎵', '🎶', '☕', '🍕', '🍺', '🚀', '💻',
    '📱', '⏰', '🔔', '📧', '🗓', '📎', '✏', '📝', '🔑', '🏠',
    '🚗', '✈', '🌍', '🌙', '☀', '🌧', '❄', '🌸', '🍀', '🌈',
    '🐱', '🐶', '🦊', '🐻', '🐼', '🐰', '🦁', '🐯', '🐸', '🦄',
    '💎', '🎁', '🎂', '🏆', '🥇', '🎯', '♻', '💤', '💬', '🔗',
];

/// 키워드 → 이모지 매핑 테이블
static EMOJI_TABLE: &[EmojiEntry] = &[
    EmojiEntry {
        keyword: "웃음",
        emojis: &['😀', '😃', '😄', '😁', '😆', '😅', '🤣', '😂', '🙂', '😉', '😊', '😇'],
    },
    EmojiEntry {
        keyword: "사랑",
        emojis: &['❤', '🧡', '💛', '💚', '💙', '💜', '🖤', '🤍', '💕', '💞', '💓', '💗', '💖', '💘', '💝', '😍', '🥰', '😘'],
    },
    EmojiEntry {
        keyword: "슬픔",
        emojis: &['😢', '😭', '😥', '😿', '🥲', '😞', '😔', '😟', '🙁', '😩', '😫'],
    },
    EmojiEntry {
        keyword: "화남",
        emojis: &['😠', '😡', '🤬', '😤', '💢', '👿'],
    },
    EmojiEntry {
        keyword: "손",
        emojis: &['👍', '👎', '👏', '🙌', '🤝', '✊', '👊', '🤛', '🤜', '✌', '🤞', '🤟', '👌', '🤙', '👋', '🤚', '✋', '🖖', '👆', '👇', '👈', '👉', '☝', '🖕', '💪'],
    },
    EmojiEntry {
        keyword: "음식",
        emojis: &['🍕', '🍔', '🍟', '🌭', '🍿', '🧂', '🥚', '🍳', '🥞', '🧇', '🥓', '🍖', '🍗', '🥩', '🌮', '🌯', '🥗', '🍜', '🍝', '🍣', '🍱', '🍙', '🍚', '🍘', '🥟', '🍤'],
    },
    EmojiEntry {
        keyword: "음료",
        emojis: &['☕', '🍵', '🥤', '🧃', '🍺', '🍻', '🥂', '🍷', '🥃', '🍸', '🍹', '🧉'],
    },
    EmojiEntry {
        keyword: "동물",
        emojis: &['🐱', '🐶', '🐭', '🐹', '🐰', '🦊', '🐻', '🐼', '🐨', '🐯', '🦁', '🐮', '🐷', '🐸', '🐵', '🐔', '🐧', '🐦', '🦅', '🦆', '🦉', '🐴', '🦄'],
    },
    EmojiEntry {
        keyword: "날씨",
        emojis: &['☀', '🌤', '⛅', '🌥', '☁', '🌦', '🌧', '⛈', '🌩', '🌨', '❄', '🌬', '💨', '🌪', '🌈', '🌙', '⭐', '🌟', '💫'],
    },
    EmojiEntry {
        keyword: "꽃",
        emojis: &['🌸', '🌹', '🌺', '🌻', '🌼', '🌷', '💐', '🌾', '🍀', '🍁', '🍂', '🍃', '🌿', '🪴'],
    },
    EmojiEntry {
        keyword: "교통",
        emojis: &['🚗', '🚕', '🚙', '🚌', '🚎', '🏎', '🚓', '🚑', '🚒', '🚐', '🛻', '🚚', '🚛', '🚜', '🏍', '🛵', '🚲', '✈', '🚀', '🛸', '🚁', '🚂', '🚆', '🚇'],
    },
    EmojiEntry {
        keyword: "사무",
        emojis: &['💻', '🖥', '⌨', '🖱', '🖨', '📱', '☎', '📧', '✉', '📮', '📬', '📝', '✏', '📎', '📌', '📍', '🗓', '📅', '📆', '📋', '📁', '📂', '🗂'],
    },
    EmojiEntry {
        keyword: "축하",
        emojis: &['🎉', '🎊', '🎈', '🎁', '🎂', '🎃', '🏆', '🥇', '🥈', '🥉', '🏅', '🎯', '🎵', '🎶', '🎸', '🎹'],
    },
    EmojiEntry {
        keyword: "기호",
        emojis: &['✅', '❌', '⭕', '❗', '❓', '‼', '⁉', '💯', '🔥', '✨', '💡', '🔔', '🔕', '🔗', '🔑', '🔒', '🔓', '♻', '⚠', '🚫', '⛔', '📛'],
    },
    EmojiEntry {
        keyword: "깃발",
        emojis: &['🏁', '🚩', '🎌', '🏴', '🏳'],
    },
    EmojiEntry {
        keyword: "스포츠",
        emojis: &['⚽', '🏀', '🏈', '⚾', '🥎', '🎾', '🏐', '🏉', '🥏', '🎱', '🏓', '🏸', '🏒', '🥊', '🥋', '⛳', '🏊', '🏄', '🚴', '🏋'],
    },
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_search_popular() {
        let results = search_emoji("");
        assert!(!results.is_empty());
        assert!(results.contains(&'😀'));
    }

    #[test]
    fn test_search_by_keyword() {
        let results = search_emoji("사랑");
        assert!(!results.is_empty());
        assert!(results.contains(&'❤'));
    }

    #[test]
    fn test_search_animal() {
        let results = search_emoji("동물");
        assert!(!results.is_empty());
        assert!(results.contains(&'🐱'));
    }

    #[test]
    fn test_search_no_match_returns_popular() {
        let results = search_emoji("존재하지않는키워드");
        assert!(!results.is_empty());
    }

    #[test]
    fn test_categories_non_empty() {
        let cats = categories();
        assert!(cats.len() >= 10);
        assert!(cats.iter().any(|c| c.keyword == "음식"));
    }

    #[test]
    fn test_popular_emojis_stable() {
        let a = popular_emojis();
        let b = popular_emojis();
        assert_eq!(a.len(), b.len());
        assert_eq!(a.first(), b.first());
    }
}
