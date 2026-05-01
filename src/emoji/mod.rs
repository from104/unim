//! 이모지 입력 모듈
//!
//! Unicode 15.0 fully-qualified 이모지 데이터(`data.rs`, 자동 생성)와
//! 최근 사용(MRU) 영속화·카테고리 조회(`recent.rs`)를 묶는다.
//!
//! PR #1 (emoji overhaul): 검색 기능 폐기, 즐겨찾기 → 최근 사용으로 의미 재정의.

mod data;
mod recent;

pub use data::{EmojiCategoryInfo, EmojiDatum, CATEGORIES, EMOJIS, TOTAL_EMOJIS};
pub use recent::{
    category_emojis, list_categories, load_recent, recent_path, touch_recent, RECENT_MAX,
};
// PR #1 backward-compat shims (DEPRECATED — PR #5 cleanup 대상).
#[allow(deprecated)]
pub use recent::{
    favorites_path, load_favorites, search_emoji, search_emoji_strings, touch_favorite,
    EmojiEntry, FAVORITES_MAX,
};
