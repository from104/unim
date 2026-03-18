//! 이모지 입력 모듈
//!
//! 키워드 기반 이모지 검색을 제공합니다.
//! 기존 특수문자 팝업 인프라(`ShowSpecial` 시그널)를 그대로 재활용합니다.

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
        // 기본: 인기 이모지
        return POPULAR_EMOJIS.to_vec();
    }

    let mut results = Vec::new();
    for entry in EMOJI_TABLE.iter() {
        if entry.keyword.contains(keyword) || keyword.contains(entry.keyword) {
            results.extend_from_slice(entry.emojis);
        }
    }

    if results.is_empty() {
        // fallback: 인기 이모지
        POPULAR_EMOJIS.to_vec()
    } else {
        results
    }
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
        assert!(!results.is_empty()); // fallback to popular
    }
}
