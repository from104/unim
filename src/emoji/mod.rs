//! 이모지 입력 모듈
//!
//! Unicode 15.0 fully-qualified 이모지 데이터(`data.rs`, 자동 생성)와
//! 한국어/영어 키워드 검색 로직(`search.rs`)을 묶는다.

mod data;
mod search;

pub use data::{EmojiCategoryInfo, EmojiDatum, CATEGORIES, EMOJIS, TOTAL_EMOJIS};
pub use search::{
    category_emojis, favorites_path, list_categories, load_favorites, search_emoji,
    search_emoji_strings, touch_favorite, EmojiEntry, FAVORITES_MAX,
};
