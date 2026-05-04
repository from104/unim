//! 팝업 핵심 상태 및 생성자
//!
//! 한자/특수문자 팝업의 단일 진실 소스 `PopupState` 구조체와
//! 생성자(new_hanja, new_special)를 정의합니다.
//!
//! 키/클릭 처리는 [`super::popup_keys`], 레이아웃·접근자는
//! [`super::popup_layout`] 형제 모듈에서 분산 `impl` 블록으로 제공됩니다.
//! 테스트는 헬퍼 공유를 위해 본 파일 하단에 일괄 유지합니다.

use super::popup_keys::{PopupKind, EMOJI_PAGE_SIZE, HANJA_PAGE_SIZE, SPECIAL_PAGE_SIZE};

/// 이모지 카테고리 메타 (popup_state 가 보유, GUI 가 좌측 탭 그릴 때 사용).
///
/// PR #1: 'Recent' 는 cat_index 0 에 위치하며 runtime 동적이다.
/// 이외 8개는 `src/emoji/data.rs::CATEGORIES` 에 정적으로 정의된다.
#[derive(Debug, Clone)]
pub struct EmojiCatMeta {
    /// 카테고리 ID — "Recent" | "SmileysPeople" | "Animals" | ... | "Flags".
    pub id: String,
    /// 한국어 라벨 ("최근 사용", "표정·인물", ...).
    pub label_ko: String,
    /// 영어 라벨 ("Recent", "Smileys & people", ...).
    pub label_en: String,
    /// 카테고리 내 총 emoji 수 (페이지 인디케이터 / total_pages 계산용).
    pub total: usize,
}

/// 팝업 상태 (단일 진실 소스)
#[derive(Debug, Clone)]
pub struct PopupState {
    pub(super) kind: PopupKind,
    /// 현재 페이지의 항목 목록 (한자/특수문자/이모지 공용).
    ///
    /// 이모지의 경우 "현재 페이지의 81개 슬라이스" 가 들어있다 — 카테고리 전환 시
    /// 엔진이 `replace_for_emoji_category` 를 호출해 새 슬라이스로 갱신한다.
    pub(super) items: Vec<String>,
    /// 한자 뜻 (한자 팝업에서만 사용, 특수문자/이모지는 빈 벡터)
    pub(super) meanings: Vec<String>,
    /// 즐겨찾기 플래그 (한자 팝업 전용, items와 동일 길이)
    pub(super) bookmarks: Vec<bool>,
    /// 대상 글자 (초성 또는 한글; 이모지에선 카테고리 id 를 그대로 빌려 사용).
    pub(super) target: String,
    /// 상단 행 레이블 (특수문자/이모지: "QWERTYUIO" 등)
    pub(super) top_row: String,
    /// 현재 페이지 (0-based)
    pub(super) current_page: usize,
    /// 전체 페이지 수
    pub(super) total_pages: usize,
    /// 선택 행 (특수문자/이모지: 0..rows-1, 한자: 0..page_items-1)
    pub(super) sel_row: usize,
    /// 선택 열 (한자 compact 는 항상 0)
    pub(super) sel_col: usize,
    /// 현재 페이지 행 수
    pub(super) rows: usize,
    /// 현재 페이지 열 수
    pub(super) cols: usize,
    /// 페이지당 최대 항목 수
    pub(super) page_size: usize,
    /// 한자 팝업 확장 모드 여부 (true면 9×9 그리드, false면 1×9 리스트)
    pub(super) hanja_expanded: bool,

    // === Emoji 전용 필드 (다른 kind 에선 기본값 / 빈 컨테이너) ===
    /// 카테고리 인덱스 (0=최근, 1..=8=SmileysPeople..Flags).
    pub(super) cat_index: usize,
    /// 카테고리 메타 — 좌측 9 탭. Hanja/SpecialChar 는 빈 벡터.
    pub(super) categories: Vec<EmojiCatMeta>,
    /// 현재 'Recent' 탭에 표시할 emoji 캐시 (cat_index=0 일 때 items 와 동일).
    /// MRU bump 시 엔진이 갱신한다.
    pub(super) recent_emojis: Vec<String>,
}

impl PopupState {
    /// 한자 팝업 생성 (기본 top_row "QWERTYUIO" 사용).
    ///
    /// 키맵에 의존하는 컬럼 라벨이 필요하면 [`Self::new_hanja_with_top_row`]를
    /// 호출한다. 본 헬퍼는 기존 호출자(테스트 다수)의 호환을 위해 유지된다.
    pub fn new_hanja(target: &str, candidates: Vec<(String, String)>) -> Self {
        Self::new_hanja_with_top_row(target, candidates, "QWERTYUIO")
    }

    /// 한자 팝업 생성 — expanded(9x9) 모드 컬럼 라벨을 외부 레이아웃에서 주입.
    ///
    /// # Arguments
    /// * `target` - 변환 대상 한글 문자열
    /// * `candidates` - (한자, 뜻) 쌍의 벡터
    /// * `top_row` - 활성 영문 키맵의 가장 위 9문자 (예: "QWERTYUIO", "',.PYFGCR")
    pub fn new_hanja_with_top_row(
        target: &str,
        candidates: Vec<(String, String)>,
        top_row: &str,
    ) -> Self {
        let total = candidates.len();
        let items: Vec<String> = candidates.iter().map(|(h, _)| h.clone()).collect();
        let meanings: Vec<String> = candidates.iter().map(|(_, m)| m.clone()).collect();
        let total_pages = if total == 0 {
            1
        } else {
            total.div_ceil(HANJA_PAGE_SIZE)
        };
        let page_items = total.min(HANJA_PAGE_SIZE);

        let bookmarks = vec![false; total];
        Self {
            kind: PopupKind::Hanja,
            items,
            meanings,
            bookmarks,
            target: target.to_string(),
            top_row: top_row.to_string(),
            current_page: 0,
            total_pages,
            sel_row: 0,
            sel_col: 0,
            rows: page_items,
            cols: 1,
            page_size: HANJA_PAGE_SIZE,
            hanja_expanded: false,
            cat_index: 0,
            categories: Vec::new(),
            recent_emojis: Vec::new(),
        }
    }

    /// 특수문자 팝업 생성
    ///
    /// # Arguments
    /// * `target` - 대상 초성
    /// * `characters` - 특수문자 벡터
    /// * `top_row` - 상단 키 레이블 (예: "QWERTYUIO")
    pub fn new_special(target: &str, characters: Vec<String>, top_row: &str) -> Self {
        let total = characters.len();
        let total_pages = if total == 0 {
            1
        } else {
            total.div_ceil(SPECIAL_PAGE_SIZE)
        };

        let mut state = Self {
            kind: PopupKind::SpecialChar,
            items: characters,
            meanings: Vec::new(),
            bookmarks: Vec::new(),
            target: target.to_string(),
            top_row: top_row.to_string(),
            current_page: 0,
            total_pages,
            sel_row: 0,
            sel_col: 0,
            rows: 0,
            cols: 0,
            page_size: SPECIAL_PAGE_SIZE,
            hanja_expanded: false,
            cat_index: 0,
            categories: Vec::new(),
            recent_emojis: Vec::new(),
        };
        state.update_page_layout();
        state
    }

    /// 이모지 팝업 생성 (PR #1 emoji overhaul, OPTION X engine-driven).
    ///
    /// `items` 는 **현재 카테고리의 1 페이지(최대 81개) 슬라이스** 다 — 전체 emoji
    /// pool 은 호출자(엔진) 가 보관한다. 카테고리 전환 시 엔진이
    /// [`PopupState::replace_for_emoji_category`] 를 호출해 새 슬라이스로 교체한다.
    ///
    /// # Arguments
    ///
    /// * `cat_index` - 초기 카테고리 인덱스 (0=Recent, 1..=8=Smileys..Flags).
    /// * `items` - 현재 카테고리의 emoji 페이지 (이모지 문자열 vec).
    /// * `total_in_cat` - 현재 카테고리의 전체 emoji 수 (total_pages 계산용).
    /// * `top_row` - 활성 영문 키맵의 상단 9 문자 (Letter 키 점프 + 컬럼 헤더).
    /// * `categories` - 좌측 9 탭 메타 (Recent + 8 카테고리).
    /// * `recent_emojis` - 'Recent' 탭에 표시할 MRU 이모지 (cat_index=0 일 때 items 와 동일 가능).
    pub fn new_emoji(
        cat_index: usize,
        items: Vec<String>,
        total_in_cat: usize,
        top_row: &str,
        categories: Vec<EmojiCatMeta>,
        recent_emojis: Vec<String>,
    ) -> Self {
        let target_id = categories
            .get(cat_index)
            .map(|c| c.id.clone())
            .unwrap_or_default();
        let total_pages = if total_in_cat == 0 {
            1
        } else {
            total_in_cat.div_ceil(EMOJI_PAGE_SIZE)
        };

        let mut state = Self {
            kind: PopupKind::Emoji,
            items,
            meanings: Vec::new(),
            bookmarks: Vec::new(),
            target: target_id,
            top_row: top_row.to_string(),
            current_page: 0,
            total_pages,
            sel_row: 0,
            sel_col: 0,
            rows: 0,
            cols: 0,
            page_size: EMOJI_PAGE_SIZE,
            hanja_expanded: false,
            cat_index,
            categories,
            recent_emojis,
        };
        state.update_page_layout();
        state
    }
}

#[cfg(test)]
mod tests {
    use super::super::popup_keys::{PopupKey, PopupKeyResult};
    use super::*;

    // --- 특수문자 팝업 테스트 ---

    fn make_special(n: usize) -> PopupState {
        let chars: Vec<String> = (0..n).map(|i| format!("C{}", i)).collect();
        PopupState::new_special("ㄱ", chars, "QWERTYUIO")
    }

    #[test]
    fn special_page_layout_full() {
        let state = make_special(81);
        assert_eq!(state.rows, 9);
        assert_eq!(state.cols, 9);
        assert_eq!(state.total_pages, 1);
    }

    #[test]
    fn special_page_layout_partial() {
        // 20 chars → cols = ceil(20/9) = 3, rows = MAX_ROWS = 9 (시각·엔진 인덱싱 일치).
        // 빈 셀(idx >= 20)은 cell_exists 가 false 반환하여 자동 비활성.
        let state = make_special(20);
        assert_eq!(state.cols, 3);
        assert_eq!(state.rows, 9);
        assert_eq!(state.total_pages, 1);
    }

    #[test]
    fn special_page_layout_multi_page() {
        // 100 chars → 2 pages
        let state = make_special(100);
        assert_eq!(state.total_pages, 2);
        assert_eq!(state.rows, 9);
        assert_eq!(state.cols, 9);
    }

    #[test]
    fn special_column_first_index() {
        // 열 우선 채움 검증: (row=0, col=0)=0, (row=1, col=0)=1, ..., (row=0, col=1)=rows
        let state = make_special(81);
        assert_eq!(state.special_global_index(0, 0), Some(0));
        assert_eq!(state.special_global_index(1, 0), Some(1));
        assert_eq!(state.special_global_index(8, 0), Some(8));
        assert_eq!(state.special_global_index(0, 1), Some(9));
        assert_eq!(state.special_global_index(8, 8), Some(80));
    }

    #[test]
    fn special_number_key_selects() {
        let mut state = make_special(81);
        // 숫자 1 → row 0, 즉시 선택
        let result = state.handle_key(PopupKey::Number(1));
        assert_eq!(result, PopupKeyResult::Select(0));
    }

    #[test]
    fn special_number_key_in_column() {
        let mut state = make_special(81);
        // 열 2로 이동 후 숫자 3 → (row=2, col=2) = 2*9+2 = 20
        state.handle_key(PopupKey::Right);
        state.handle_key(PopupKey::Right);
        let result = state.handle_key(PopupKey::Number(3));
        assert_eq!(result, PopupKeyResult::Select(20));
    }

    #[test]
    fn special_letter_jump_column() {
        let mut state = make_special(81);
        // 'E' = Letter(2) → col 2
        state.handle_key(PopupKey::Letter(2));
        assert_eq!(state.sel_col, 2);
        assert_eq!(state.sel_row, 0);
    }

    #[test]
    fn special_letter_invalid_cell_fallback() {
        // 20 chars: cols=3, rows=7. col 2의 마지막 유효 행은?
        // col2: indices 14..19 → 6개 (rows 0-5)
        let mut state = make_special(20);
        state.sel_row = 6; // row 6
        state.handle_key(PopupKey::Letter(2)); // col 2
                                               // col 2에서 row 6은 idx=2*7+6=20, 범위 밖 → 마지막 유효 행으로
        assert!(state.cell_exists(state.sel_row, state.sel_col));
    }

    #[test]
    fn special_arrow_up_wrap() {
        let mut state = make_special(81);
        assert_eq!(state.sel_row, 0);
        state.handle_key(PopupKey::Up);
        assert_eq!(state.sel_row, 8); // 래핑
    }

    #[test]
    fn special_arrow_down_wrap() {
        let mut state = make_special(81);
        state.sel_row = 8;
        state.handle_key(PopupKey::Down);
        assert_eq!(state.sel_row, 0); // 래핑
    }

    #[test]
    fn special_arrow_left_wrap() {
        let mut state = make_special(81);
        assert_eq!(state.sel_col, 0);
        state.handle_key(PopupKey::Left);
        assert_eq!(state.sel_col, 8); // 래핑
    }

    #[test]
    fn special_arrow_right_wrap() {
        let mut state = make_special(81);
        state.sel_col = 8;
        state.handle_key(PopupKey::Right);
        assert_eq!(state.sel_col, 0); // 래핑
    }

    #[test]
    fn special_arrow_down_invalid_cell_reset() {
        // 20 chars: cols=3, rows=7, col 2 has 6 items (row 0-5)
        let mut state = make_special(20);
        state.sel_col = 2;
        state.sel_row = 5; // 마지막 유효 행
        state.handle_key(PopupKey::Down);
        // row 6은 유효하지 않으므로 row 0으로 리셋
        assert_eq!(state.sel_row, 0);
    }

    #[test]
    fn special_tab_next_page() {
        let mut state = make_special(100);
        assert_eq!(state.current_page, 0);
        state.handle_key(PopupKey::Tab);
        assert_eq!(state.current_page, 1);
        assert_eq!(state.sel_row, 0);
        assert_eq!(state.sel_col, 0);
    }

    #[test]
    fn special_tab_wrap_page() {
        let mut state = make_special(100);
        state.current_page = 1; // 마지막 페이지
        state.handle_key(PopupKey::Tab);
        assert_eq!(state.current_page, 0); // 래핑
    }

    #[test]
    fn special_shift_tab_prev_page() {
        let mut state = make_special(100);
        state.current_page = 1;
        state.update_page_layout();
        state.handle_key(PopupKey::ShiftTab);
        assert_eq!(state.current_page, 0);
    }

    #[test]
    fn special_shift_tab_wrap_page() {
        let mut state = make_special(100);
        assert_eq!(state.current_page, 0);
        state.handle_key(PopupKey::ShiftTab);
        assert_eq!(state.current_page, 1); // 래핑
    }

    #[test]
    fn special_enter_select() {
        let mut state = make_special(81);
        state.sel_row = 3;
        state.sel_col = 2;
        let result = state.handle_key(PopupKey::Enter);
        // (row=3, col=2) → 2*9+3 = 21
        assert_eq!(result, PopupKeyResult::Select(21));
    }

    #[test]
    fn special_escape_cancel() {
        let mut state = make_special(81);
        let result = state.handle_key(PopupKey::Escape);
        assert_eq!(result, PopupKeyResult::Cancel);
    }

    #[test]
    fn special_other_key_not_handled() {
        let mut state = make_special(81);
        let result = state.handle_key(PopupKey::Other);
        assert_eq!(result, PopupKeyResult::NotHandled);
    }

    #[test]
    fn special_home_jumps_to_first_page() {
        let mut state = make_special(100);
        state.current_page = 1;
        state.sel_row = 5;
        state.sel_col = 2;
        state.update_page_layout();
        let result = state.handle_key(PopupKey::Home);
        assert_eq!(result, PopupKeyResult::Updated);
        assert_eq!(state.current_page, 0);
        assert_eq!(state.sel_row, 0);
        assert_eq!(state.sel_col, 0);
    }

    #[test]
    fn special_end_jumps_to_last_data_cell() {
        // 100 chars: page 0 = 81 (9×9), page 1 = 19 (cols=3, rows=9 고정).
        // column-major 채움: col=0 chars 0..8, col=1 chars 9..17, col=2 char 18.
        // last index in page 1 = 18, col=18/9=2, row=18%9=0.
        let mut state = make_special(100);
        let result = state.handle_key(PopupKey::End);
        assert_eq!(result, PopupKeyResult::Updated);
        assert_eq!(state.current_page, 1);
        assert_eq!(state.sel_col, 2);
        assert_eq!(state.sel_row, 0);
    }

    #[test]
    fn special_end_full_page_last_cell() {
        // 81 chars: 1 page, last index = 80 → col=8, row=8
        let mut state = make_special(81);
        state.handle_key(PopupKey::End);
        assert_eq!(state.current_page, 0);
        assert_eq!(state.sel_col, 8);
        assert_eq!(state.sel_row, 8);
    }

    #[test]
    fn special_modifier_consumed() {
        let mut state = make_special(81);
        let result = state.handle_key(PopupKey::Modifier);
        assert_eq!(result, PopupKeyResult::Consumed);
    }

    #[test]
    fn special_click_select() {
        let mut state = make_special(81);
        let result = state.handle_click(2, 3);
        // (row=2, col=3) → 3*9+2 = 29
        assert_eq!(result, PopupKeyResult::Select(29));
    }

    #[test]
    fn special_click_invalid() {
        let mut state = make_special(20);
        // (row=8, col=8) → 유효하지 않음
        let result = state.handle_click(8, 8);
        assert_eq!(result, PopupKeyResult::Consumed);
    }

    #[test]
    fn special_page_layout_second_page() {
        // 100 chars: page 0 = 81, page 1 = 19
        let mut state = make_special(100);
        state.current_page = 1;
        state.update_page_layout();
        // 19 chars: cols = ceil(19/9) = 3, rows = MAX_ROWS = 9 (고정).
        assert_eq!(state.cols, 3);
        assert_eq!(state.rows, 9);
    }

    #[test]
    fn special_space_next_page() {
        let mut state = make_special(100);
        state.handle_key(PopupKey::Space);
        assert_eq!(state.current_page, 1);
    }

    #[test]
    fn special_empty() {
        // 빈 페이지: cols=1 유지(분기 분리 보존), rows 는 9 고정 (grid).
        let state = make_special(0);
        assert_eq!(state.total_pages, 1);
        assert_eq!(state.rows, 9);
        assert_eq!(state.cols, 1);
    }

    // --- 한자 팝업 테스트 ---

    fn make_hanja(n: usize) -> PopupState {
        let candidates: Vec<(String, String)> = (0..n)
            .map(|i| (format!("漢{}", i), format!("뜻{}", i)))
            .collect();
        PopupState::new_hanja("한", candidates)
    }

    #[test]
    fn hanja_basic_layout() {
        let state = make_hanja(20);
        assert_eq!(state.total_pages, 3); // ceil(20/9)
        assert_eq!(state.rows, 9); // 첫 페이지 9개
        assert_eq!(state.cols, 1);
    }

    #[test]
    fn hanja_number_select() {
        let mut state = make_hanja(20);
        let result = state.handle_key(PopupKey::Number(3));
        assert_eq!(result, PopupKeyResult::Select(2)); // 0-based
    }

    #[test]
    fn hanja_number_out_of_range() {
        let mut state = make_hanja(3);
        let result = state.handle_key(PopupKey::Number(5));
        assert_eq!(result, PopupKeyResult::Consumed);
    }

    #[test]
    fn hanja_arrow_down_wrap() {
        let mut state = make_hanja(9);
        state.sel_row = 8;
        state.handle_key(PopupKey::Down);
        assert_eq!(state.sel_row, 0);
    }

    #[test]
    fn hanja_arrow_up_wrap() {
        let mut state = make_hanja(9);
        assert_eq!(state.sel_row, 0);
        state.handle_key(PopupKey::Up);
        assert_eq!(state.sel_row, 8);
    }

    #[test]
    fn hanja_right_next_page() {
        let mut state = make_hanja(20);
        state.handle_key(PopupKey::Right);
        assert_eq!(state.current_page, 1);
        assert_eq!(state.sel_row, 0);
    }

    #[test]
    fn hanja_right_wrap_page() {
        let mut state = make_hanja(20);
        state.current_page = 2; // 마지막 페이지 (3페이지 중)
        state.update_page_layout();
        state.handle_key(PopupKey::Right);
        assert_eq!(state.current_page, 0); // 래핑
    }

    #[test]
    fn hanja_left_prev_page() {
        let mut state = make_hanja(20);
        state.current_page = 1;
        state.update_page_layout();
        state.handle_key(PopupKey::Left);
        assert_eq!(state.current_page, 0);
    }

    #[test]
    fn hanja_left_wrap_page() {
        let mut state = make_hanja(20);
        state.handle_key(PopupKey::Left);
        assert_eq!(state.current_page, 2); // 래핑
    }

    #[test]
    fn hanja_space_toggles_bookmark() {
        let mut state = make_hanja(20);
        // 첫 번째 후보가 선택된 상태
        let result = state.handle_key(PopupKey::Space);
        assert_eq!(result, PopupKeyResult::ToggleBookmark(0));
        // Space 자체는 페이지를 바꾸지 않는다
        assert_eq!(state.current_page, 0);
    }

    #[test]
    fn hanja_tab_next_page() {
        let mut state = make_hanja(20);
        state.handle_key(PopupKey::Tab);
        assert_eq!(state.current_page, 1);
    }

    #[test]
    fn hanja_bookmark_flag_accessors() {
        let mut state = make_hanja(5);
        assert!(!state.is_bookmarked(0));
        state.set_bookmark(0, true);
        assert!(state.is_bookmarked(0));
        state.set_bookmark_flags(vec![false, true, false, true, false]);
        assert!(!state.is_bookmarked(0));
        assert!(state.is_bookmarked(1));
        assert!(state.is_bookmarked(3));
    }

    #[test]
    fn hanja_space_second_page_toggles_correct_global_index() {
        let mut state = make_hanja(20);
        state.handle_key(PopupKey::Right); // page 1
        let result = state.handle_key(PopupKey::Space);
        // page 1 * 9 + sel_row 0 = 9
        assert_eq!(result, PopupKeyResult::ToggleBookmark(9));
    }

    #[test]
    fn hanja_compact_home_jumps_to_first_page_first_row() {
        let mut state = make_hanja(20);
        state.current_page = 2;
        state.sel_row = 1;
        state.update_page_layout();
        let result = state.handle_key(PopupKey::Home);
        assert_eq!(result, PopupKeyResult::Updated);
        assert_eq!(state.current_page, 0);
        assert_eq!(state.sel_row, 0);
        assert_eq!(state.sel_col, 0);
    }

    #[test]
    fn hanja_compact_end_jumps_to_last_page_last_row() {
        // 20 candidates, compact: 9/page → 3 pages, last page has 2 items (idx 18, 19)
        let mut state = make_hanja(20);
        let result = state.handle_key(PopupKey::End);
        assert_eq!(result, PopupKeyResult::Updated);
        assert_eq!(state.current_page, 2);
        assert_eq!(state.sel_col, 0);
        // page 2 has 2 items (rows=2), last row = 1
        assert_eq!(state.sel_row, 1);
    }

    #[test]
    fn hanja_expanded_end_jumps_to_last_data_cell() {
        // 100 candidates, expanded: 81/page → 2 pages, last page has 19 items.
        // 19 → cols=3, rows=9 (고정). last index 18 → col=18/9=2, row=18%9=0.
        let mut state = make_hanja(100);
        state.handle_key(PopupKey::Period); // expanded
        let result = state.handle_key(PopupKey::End);
        assert_eq!(result, PopupKeyResult::Updated);
        assert_eq!(state.current_page, 1);
        assert_eq!(state.sel_col, 2);
        assert_eq!(state.sel_row, 0);
    }

    #[test]
    fn hanja_period_toggles_expanded_mode() {
        let mut state = make_hanja(100);
        assert!(!state.is_hanja_expanded());
        // compact: 9 per page → 12 pages
        assert_eq!(state.total_pages, 12);
        state.handle_key(PopupKey::Period);
        assert!(state.is_hanja_expanded());
        // expanded: 81 per page → 2 pages
        assert_eq!(state.total_pages, 2);
        assert_eq!(state.current_page, 0);
        assert_eq!(state.sel_row, 0);
        assert_eq!(state.sel_col, 0);
    }

    #[test]
    fn hanja_expanded_layout_full_page() {
        let mut state = make_hanja(81);
        state.handle_key(PopupKey::Period);
        assert_eq!(state.rows, 9);
        assert_eq!(state.cols, 9);
    }

    #[test]
    fn hanja_expanded_arrow_right_wraps_column() {
        let mut state = make_hanja(81);
        state.handle_key(PopupKey::Period);
        state.handle_key(PopupKey::Right);
        assert_eq!(state.sel_col, 1);
        // 오른쪽 끝에서 wrap
        for _ in 0..8 {
            state.handle_key(PopupKey::Right);
        }
        assert_eq!(state.sel_col, 0);
    }

    #[test]
    fn hanja_expanded_number_selects_in_column() {
        let mut state = make_hanja(81);
        state.handle_key(PopupKey::Period);
        state.handle_key(PopupKey::Right);
        state.handle_key(PopupKey::Right);
        // col=2, Number 3 → row=2 → global = 2*9+2 = 20
        let result = state.handle_key(PopupKey::Number(3));
        assert_eq!(result, PopupKeyResult::Select(20));
    }

    #[test]
    fn hanja_expanded_space_toggles_selected_bookmark() {
        let mut state = make_hanja(81);
        state.handle_key(PopupKey::Period);
        // 기본 (0,0) → global 0
        let result = state.handle_key(PopupKey::Space);
        assert_eq!(result, PopupKeyResult::ToggleBookmark(0));
    }

    #[test]
    fn hanja_expanded_tab_advances_page() {
        let mut state = make_hanja(100);
        state.handle_key(PopupKey::Period);
        assert_eq!(state.current_page, 0);
        state.handle_key(PopupKey::Tab);
        assert_eq!(state.current_page, 1);
    }

    #[test]
    fn hanja_period_twice_returns_to_compact() {
        let mut state = make_hanja(50);
        state.handle_key(PopupKey::Period);
        assert!(state.is_hanja_expanded());
        state.handle_key(PopupKey::Period);
        assert!(!state.is_hanja_expanded());
        assert_eq!(state.cols, 1);
        assert_eq!(state.page_size, 9);
    }

    #[test]
    fn hanja_new_default_top_row_is_qwertyuio() {
        let state = make_hanja(9);
        assert_eq!(state.top_row(), "QWERTYUIO");
    }

    #[test]
    fn hanja_new_with_top_row_dvorak() {
        let candidates: Vec<(String, String)> = (0..9)
            .map(|i| (format!("漢{}", i), format!("뜻{}", i)))
            .collect();
        let state = PopupState::new_hanja_with_top_row("한", candidates, "',.PYFGCR");
        assert_eq!(state.top_row(), "',.PYFGCR");
    }

    #[test]
    fn hanja_new_with_top_row_colemak() {
        let candidates: Vec<(String, String)> = (0..9)
            .map(|i| (format!("漢{}", i), format!("뜻{}", i)))
            .collect();
        let state = PopupState::new_hanja_with_top_row("한", candidates, "QWFPGJLUY");
        assert_eq!(state.top_row(), "QWFPGJLUY");
    }

    #[test]
    fn hanja_expanded_letter_jumps_column() {
        let mut state = make_hanja(81);
        state.handle_key(PopupKey::Period); // expanded 진입
        let result = state.handle_key(PopupKey::Letter(2)); // E 키 = col 2
        assert_eq!(result, PopupKeyResult::Updated);
        assert_eq!(state.sel_col, 2);
        // Number(1) → row=0 → global = 2*9+0 = 18
        let r2 = state.handle_key(PopupKey::Number(1));
        assert_eq!(r2, PopupKeyResult::Select(18));
    }

    #[test]
    fn hanja_compact_letter_not_handled() {
        let mut state = make_hanja(9);
        // compact에서는 Letter는 통과(NotHandled) — 회귀 방지
        let result = state.handle_key(PopupKey::Letter(2));
        assert_eq!(result, PopupKeyResult::NotHandled);
    }

    #[test]
    fn hanja_expanded_letter_out_of_range_no_change() {
        let mut state = make_hanja(10); // 페이지 1: 10개 → cols=2
        state.handle_key(PopupKey::Period);
        // cols=2 인 페이지에서 Letter(5) (Y 위치) 누르면 sel_col 변동 없음
        let before = state.sel_col;
        let result = state.handle_key(PopupKey::Letter(5));
        assert_eq!(result, PopupKeyResult::Updated);
        assert_eq!(state.sel_col, before);
    }

    #[test]
    fn hanja_expanded_click_selects() {
        let mut state = make_hanja(81);
        state.handle_key(PopupKey::Period);
        let result = state.handle_click(2, 3);
        // col-first: 3*9 + 2 = 29
        assert_eq!(result, PopupKeyResult::Select(29));
    }

    #[test]
    fn hanja_backspace_prev_page() {
        let mut state = make_hanja(20);
        state.current_page = 1;
        state.update_page_layout();
        state.handle_key(PopupKey::Backspace);
        assert_eq!(state.current_page, 0);
    }

    #[test]
    fn hanja_enter_select() {
        let mut state = make_hanja(9);
        state.sel_row = 4;
        let result = state.handle_key(PopupKey::Enter);
        assert_eq!(result, PopupKeyResult::Select(4));
    }

    #[test]
    fn hanja_escape_cancel() {
        let mut state = make_hanja(9);
        let result = state.handle_key(PopupKey::Escape);
        assert_eq!(result, PopupKeyResult::Cancel);
    }

    #[test]
    fn hanja_letter_not_handled() {
        let mut state = make_hanja(9);
        let result = state.handle_key(PopupKey::Letter(0));
        assert_eq!(result, PopupKeyResult::NotHandled);
    }

    #[test]
    fn hanja_click_select() {
        let mut state = make_hanja(9);
        let result = state.handle_click(3, 0);
        assert_eq!(result, PopupKeyResult::Select(3));
    }

    #[test]
    fn hanja_page_items() {
        let state = make_hanja(20);
        let items = state.hanja_page_items();
        assert_eq!(items.len(), 9);
        assert_eq!(items[0].0, "漢0");
        assert_eq!(items[0].1, "뜻0");
    }

    #[test]
    fn hanja_last_page_items() {
        let mut state = make_hanja(20);
        state.current_page = 2;
        state.update_page_layout();
        let items = state.hanja_page_items();
        assert_eq!(items.len(), 2); // 20 - 18 = 2
    }

    #[test]
    fn hanja_empty() {
        let state = make_hanja(0);
        assert_eq!(state.total_pages, 1);
        assert_eq!(state.rows, 0);
    }

    #[test]
    fn hanja_second_page_number_select() {
        let mut state = make_hanja(20);
        state.handle_key(PopupKey::Right); // page 1
        let result = state.handle_key(PopupKey::Number(1));
        assert_eq!(result, PopupKeyResult::Select(9)); // page 1, idx 0
    }

    // --- 경계 조건 테스트 ---

    #[test]
    fn special_single_char() {
        // 1 char → cols=1, rows=9 (고정). 단일 셀 (0,0) 만 채워짐.
        let state = make_special(1);
        assert_eq!(state.rows, 9);
        assert_eq!(state.cols, 1);
        assert_eq!(state.total_pages, 1);
    }

    #[test]
    fn special_exactly_one_page() {
        let state = make_special(81);
        assert_eq!(state.total_pages, 1);
    }

    #[test]
    fn special_one_over_page() {
        let state = make_special(82);
        assert_eq!(state.total_pages, 2);
    }

    #[test]
    fn hanja_exactly_one_page() {
        let state = make_hanja(9);
        assert_eq!(state.total_pages, 1);
    }

    #[test]
    fn hanja_one_over_page() {
        let state = make_hanja(10);
        assert_eq!(state.total_pages, 2);
    }

    #[test]
    fn special_page_switch_resets_selection() {
        let mut state = make_special(100);
        state.sel_row = 5;
        state.sel_col = 3;
        state.handle_key(PopupKey::Tab);
        assert_eq!(state.sel_row, 0);
        assert_eq!(state.sel_col, 0);
    }

    #[test]
    fn hanja_page_switch_resets_selection() {
        let mut state = make_hanja(20);
        state.sel_row = 5;
        state.handle_key(PopupKey::Right);
        assert_eq!(state.sel_row, 0);
    }

    #[test]
    fn special_get_item() {
        let state = make_special(10);
        assert_eq!(state.get_item(0), Some("C0"));
        assert_eq!(state.get_item(9), Some("C9"));
        assert_eq!(state.get_item(10), None);
    }

    #[test]
    fn hanja_get_meaning() {
        let state = make_hanja(5);
        assert_eq!(state.get_meaning(0), Some("뜻0"));
        assert_eq!(state.get_meaning(4), Some("뜻4"));
        assert_eq!(state.get_meaning(5), None);
    }

    #[test]
    fn special_cell_text() {
        let state = make_special(81);
        assert_eq!(state.cell_text(0, 0), Some("C0"));
        assert_eq!(state.cell_text(0, 1), Some("C9")); // 열 우선
        assert_eq!(state.cell_text(8, 8), Some("C80"));
    }

    #[test]
    fn special_single_page_no_tab_effect() {
        let mut state = make_special(10);
        state.handle_key(PopupKey::Tab);
        assert_eq!(state.current_page, 0); // 단일 페이지면 변경 없음
    }

    // --- 즐겨찾기 정렬 + 커서 이동 테스트 ---

    #[test]
    fn set_selected_global_compact_first_page() {
        // compact: page_size=9, 단일 컬럼
        let mut state = make_hanja(20);
        state.set_selected_global(3);
        assert_eq!(state.current_page, 0);
        assert_eq!(state.sel_row, 3);
        assert_eq!(state.sel_col, 0);
    }

    #[test]
    fn set_selected_global_compact_other_page() {
        let mut state = make_hanja(20);
        state.set_selected_global(11);
        assert_eq!(state.current_page, 1); // 11 / 9 = 1
        assert_eq!(state.sel_row, 2); // 11 % 9 = 2
        assert_eq!(state.sel_col, 0);
    }

    #[test]
    fn set_selected_global_expanded() {
        // expanded: 9x9, col 우선 인덱싱
        let mut state = make_hanja(81);
        state.handle_key(PopupKey::Period); // expanded 진입
        state.set_selected_global(10);
        // page_offset=10, rows=9 → col=10/9=1, row=10%9=1
        assert_eq!(state.current_page, 0);
        assert_eq!(state.sel_col, 1);
        assert_eq!(state.sel_row, 1);
    }

    #[test]
    fn replace_hanja_items_recomputes_pages() {
        let mut state = make_hanja(20);
        state.replace_hanja_items(
            vec!["A".into(), "B".into(), "C".into()],
            vec!["a".into(), "b".into(), "c".into()],
            vec![true, false, false],
        );
        assert_eq!(state.total_items(), 3);
        assert_eq!(state.total_pages(), 1);
        assert!(state.is_bookmarked(0));
        assert!(!state.is_bookmarked(1));
    }

    /// toggle 모방: 후보 재정렬 + set_selected_global → 토글된 한자가 새 위치로 점프
    #[test]
    fn toggle_bookmark_reorders_candidates_and_moves_cursor() {
        // 초기: [A, B, C] (모두 false)
        let mut state = PopupState::new_hanja(
            "한",
            vec![
                ("A".into(), "a".into()),
                ("B".into(), "b".into()),
                ("C".into(), "c".into()),
            ],
        );
        // C를 bookmark 토글 → 정렬 후 [C, A, B], C는 인덱스 0
        let new_items: Vec<String> = vec!["C".into(), "A".into(), "B".into()];
        let new_meanings: Vec<String> = vec!["c".into(), "a".into(), "b".into()];
        let new_bookmarks: Vec<bool> = vec![true, false, false];
        state.replace_hanja_items(new_items, new_meanings, new_bookmarks);
        state.set_selected_global(0);
        assert_eq!(state.get_item(0), Some("C"));
        assert!(state.is_bookmarked(0));
        assert_eq!(state.sel_row, 0);
        assert_eq!(state.sel_col, 0);

        // 다시 C ★ 해제 → [A, B, C], C는 인덱스 2
        state.replace_hanja_items(
            vec!["A".into(), "B".into(), "C".into()],
            vec!["a".into(), "b".into(), "c".into()],
            vec![false, false, false],
        );
        state.set_selected_global(2);
        assert_eq!(state.get_item(2), Some("C"));
        assert!(!state.is_bookmarked(2));
        assert_eq!(state.sel_row, 2);
    }

    #[test]
    fn set_selected_global_out_of_range_no_change() {
        let mut state = make_hanja(5);
        let before_row = state.sel_row;
        state.set_selected_global(100);
        assert_eq!(state.sel_row, before_row);
    }

    /// 즐겨찾기 해제 시 stable sort 결과 검증.
    /// 후보 [A*, B*, C, D, E] (A·B 즐겨찾기) 상태에서 A 해제 →
    /// 정렬 결과 [B*, A, C, D, E], 새 인덱스 1로 점프하고 그 자리는 페이지 0의 row 1.
    #[test]
    fn unbookmark_keeps_stable_order_for_remaining_bookmarks() {
        let mut state = PopupState::new_hanja(
            "한",
            vec![
                ("A".into(), "a".into()),
                ("B".into(), "b".into()),
                ("C".into(), "c".into()),
                ("D".into(), "d".into()),
                ("E".into(), "e".into()),
            ],
        );
        // 초기: A·B 즐겨찾기. 정렬은 외부에서 사전 적용된 상태로 진입한다고 가정 (input_engine과 동일).
        state.set_bookmark_flags(vec![true, true, false, false, false]);
        // A를 해제: 새 정렬은 [B*, A, C, D, E], A의 새 인덱스 = 1.
        state.replace_hanja_items(
            vec!["B".into(), "A".into(), "C".into(), "D".into(), "E".into()],
            vec!["b".into(), "a".into(), "c".into(), "d".into(), "e".into()],
            vec![true, false, false, false, false],
        );
        state.set_selected_global(1);

        assert_eq!(state.get_item(1), Some("A"));
        assert!(!state.is_bookmarked(1));
        assert!(state.is_bookmarked(0)); // B는 여전히 즐겨찾기
        assert_eq!(state.current_page, 0);
        assert_eq!(state.sel_row, 1);
        assert_eq!(state.sel_col, 0);
    }

    // =========================================================
    // PR #1 emoji overhaul — PopupState::Emoji 분기 테스트
    // =========================================================

    fn make_emoji_categories() -> Vec<EmojiCatMeta> {
        vec![
            EmojiCatMeta {
                id: "Recent".into(),
                label_ko: "최근 사용".into(),
                label_en: "Recent".into(),
                total: 0,
            },
            EmojiCatMeta {
                id: "SmileysPeople".into(),
                label_ko: "표정·인물".into(),
                label_en: "Smileys & people".into(),
                total: 553,
            },
            EmojiCatMeta {
                id: "Animals".into(),
                label_ko: "동물·자연".into(),
                label_en: "Animals & nature".into(),
                total: 153,
            },
            EmojiCatMeta {
                id: "Food".into(),
                label_ko: "음식·음료".into(),
                label_en: "Food & drink".into(),
                total: 135,
            },
            EmojiCatMeta {
                id: "Activities".into(),
                label_ko: "활동".into(),
                label_en: "Activities".into(),
                total: 85,
            },
            EmojiCatMeta {
                id: "Travel".into(),
                label_ko: "여행·장소".into(),
                label_en: "Travel & places".into(),
                total: 218,
            },
            EmojiCatMeta {
                id: "Objects".into(),
                label_ko: "사물".into(),
                label_en: "Objects".into(),
                total: 262,
            },
            EmojiCatMeta {
                id: "Symbols".into(),
                label_ko: "기호".into(),
                label_en: "Symbols".into(),
                total: 223,
            },
            EmojiCatMeta {
                id: "Flags".into(),
                label_ko: "국기".into(),
                label_en: "Flags".into(),
                total: 269,
            },
        ]
    }

    fn make_emoji_state(start_cat: usize, n: usize) -> PopupState {
        let items: Vec<String> = (0..n).map(|i| format!("E{i}")).collect();
        PopupState::new_emoji(
            start_cat,
            items,
            n,
            "QWERTYUIO",
            make_emoji_categories(),
            Vec::new(),
        )
    }

    #[test]
    fn emoji_page_layout_full() {
        // 81개 → rows=9, cols=9, total_pages=1
        let state = make_emoji_state(1, 81);
        assert_eq!(state.rows, 9);
        assert_eq!(state.cols, 9);
        assert_eq!(state.total_pages, 1);
        assert_eq!(state.kind(), PopupKind::Emoji);
    }

    #[test]
    fn emoji_page_layout_partial() {
        // 50개 → cols=ceil(50/9)=6, rows=ceil(50/6)=9
        let state = make_emoji_state(1, 50);
        assert_eq!(state.cols, 6);
        assert_eq!(state.rows, 9);
        assert_eq!(state.total_pages, 1);
    }

    #[test]
    fn emoji_arrow_navigation() {
        let mut state = make_emoji_state(1, 81);
        state.handle_key(PopupKey::Right);
        state.handle_key(PopupKey::Down);
        assert_eq!(state.sel_col, 1);
        assert_eq!(state.sel_row, 1);
    }

    #[test]
    fn emoji_tab_switch_category() {
        let mut state = make_emoji_state(0, 0);
        // Tab → cat_index +1, current_page=0, sel=(0,0)
        state.handle_key(PopupKey::Tab);
        assert_eq!(state.emoji_cat_index(), 1);
        assert_eq!(state.current_page, 0);
        assert_eq!(state.sel_row, 0);
        assert_eq!(state.sel_col, 0);
    }

    #[test]
    fn emoji_tab_wrap_around() {
        let mut state = make_emoji_state(8, 269);
        // 8 → Tab → 0 (Recent)
        state.handle_key(PopupKey::Tab);
        assert_eq!(state.emoji_cat_index(), 0);
    }

    #[test]
    fn emoji_shift_tab_prev_category() {
        let mut state = make_emoji_state(0, 0);
        state.handle_key(PopupKey::ShiftTab);
        // 0 → ShiftTab → 8 (Flags), wrap around backwards
        assert_eq!(state.emoji_cat_index(), 8);
    }

    #[test]
    fn emoji_page_down_up() {
        // 200개 → 81+81+38 → total_pages=3
        let mut state = make_emoji_state(1, 200);
        assert_eq!(state.total_pages, 3);
        state.handle_key(PopupKey::PageDown);
        assert_eq!(state.current_page, 1);
        state.handle_key(PopupKey::PageUp);
        assert_eq!(state.current_page, 0);
        // PageUp from page 0 wraps to last
        state.handle_key(PopupKey::PageUp);
        assert_eq!(state.current_page, 2);
    }

    #[test]
    fn emoji_home_jumps_to_first_page() {
        let mut state = make_emoji_state(1, 200);
        state.current_page = 2;
        state.sel_row = 4;
        state.sel_col = 3;
        state.update_page_layout();
        let result = state.handle_key(PopupKey::Home);
        assert_eq!(result, PopupKeyResult::Updated);
        assert_eq!(state.current_page, 0);
        assert_eq!(state.sel_row, 0);
        assert_eq!(state.sel_col, 0);
    }

    #[test]
    fn emoji_end_jumps_to_last_data_cell() {
        // 200 emojis → 81+81+38, last page has 38 items.
        // 38 → cols=ceil(38/9)=5, rows=9 (고정). last idx=37 → col=37/9=4, row=37%9=1.
        let mut state = make_emoji_state(1, 200);
        let result = state.handle_key(PopupKey::End);
        assert_eq!(result, PopupKeyResult::Updated);
        assert_eq!(state.current_page, 2);
        assert_eq!(state.sel_col, 4);
        assert_eq!(state.sel_row, 1);
    }

    #[test]
    fn emoji_letter_jump_qwerty() {
        let mut state = make_emoji_state(1, 81);
        // Letter(2) (E) → col 2
        state.handle_key(PopupKey::Letter(2));
        assert_eq!(state.sel_col, 2);
        assert_eq!(state.sel_row, 0);
    }

    #[test]
    fn emoji_number_select() {
        let mut state = make_emoji_state(1, 81);
        // sel_col=0, Number(3) → row 2 → global 0*9+2 = 2 → Select(2)
        let r = state.handle_key(PopupKey::Number(3));
        assert_eq!(r, PopupKeyResult::Select(2));
    }

    #[test]
    fn emoji_escape_cancels() {
        let mut state = make_emoji_state(1, 81);
        let r = state.handle_key(PopupKey::Escape);
        assert_eq!(r, PopupKeyResult::Cancel);
    }

    #[test]
    fn emoji_space_consumed_no_commit() {
        // 이모지에선 Space 가 commit/페이지 전환 모두 아니다 — Consumed.
        let mut state = make_emoji_state(1, 81);
        let r = state.handle_key(PopupKey::Space);
        assert_eq!(r, PopupKeyResult::Consumed);
        assert_eq!(state.current_page, 0);
    }

    #[test]
    fn emoji_replace_for_category_resets_page_and_sel() {
        let mut state = make_emoji_state(0, 0);
        state.handle_key(PopupKey::Tab); // cat_index=1
        // 시뮬레이션: 카테고리 1 의 풀 553 개 가 들어옴.
        let pool: Vec<String> = (0..553).map(|i| format!("p{i}")).collect();
        state.replace_for_emoji_category(1, pool, 553);
        assert_eq!(state.emoji_cat_index(), 1);
        assert_eq!(state.total_pages, 553_usize.div_ceil(81));
        assert_eq!(state.current_page, 0);
        assert_eq!(state.sel_row, 0);
        assert_eq!(state.sel_col, 0);
        // target 도 카테고리 id 로 갱신.
        assert_eq!(state.target(), "SmileysPeople");
    }

    #[test]
    fn emoji_update_recent_grows_total_pages_when_on_recent_tab() {
        let mut state = make_emoji_state(0, 0);
        assert_eq!(state.total_pages, 1);
        let recent: Vec<String> = (0..82).map(|i| format!("r{i}")).collect();
        state.update_emoji_recent(recent);
        assert_eq!(state.total_pages, 2);
        assert_eq!(state.emoji_recent().len(), 82);
    }

    #[test]
    fn emoji_recent_max_truncation_via_touch() {
        // touch 정책 자체는 emoji::recent::touch_recent 가 담당. 본 테스트는
        // popup_state 가 update_emoji_recent 를 호출했을 때 81 초과를 그대로
        // 반영하는지(엔진 책임) 확인 — popup_state 는 truncate 하지 않음.
        let mut state = make_emoji_state(0, 0);
        let recent: Vec<String> = (0..200).map(|i| format!("r{i}")).collect();
        state.update_emoji_recent(recent);
        assert_eq!(state.emoji_recent().len(), 200);
    }

    #[test]
    fn emoji_global_index_uses_special_column_first() {
        // Emoji 도 SpecialChar 와 동일하게 col*rows + row (열 우선) 인덱싱.
        let state = make_emoji_state(1, 81);
        assert_eq!(state.special_global_index(0, 0), Some(0));
        assert_eq!(state.special_global_index(1, 0), Some(1));
        assert_eq!(state.special_global_index(0, 1), Some(9));
    }

    #[test]
    fn emoji_click_select_returns_global_index() {
        let mut state = make_emoji_state(1, 81);
        let r = state.handle_click(2, 3);
        // (row=2, col=3) → 3*9+2 = 29
        assert_eq!(r, PopupKeyResult::Select(29));
    }

    #[test]
    fn emoji_view_model_basic() {
        let state = make_emoji_state(1, 50);
        let vm = state.view_model();
        assert_eq!(vm.kind, PopupKind::Emoji);
        // 푸터: 카테고리 한글 라벨 + page 1/1
        assert!(vm.footer_text.contains("표정·인물"));
        assert!(vm.footer_text.contains("1 / 1"));
    }

    /// 즐겨찾기 해제 시 새 인덱스가 현재 페이지를 벗어나면 페이지가 점프해야 한다.
    /// 10개 후보 중 X만 즐겨찾기 → X 해제 → stable sort 결과 X 인덱스 9 (페이지 1의 row 0).
    /// 사용자 보고 케이스: 해제 후 "그 자리에 그대로" 보이지 않고 페이지가 넘어가야 한다.
    #[test]
    fn unbookmark_jumps_page_when_new_index_overflows_current_page() {
        let mut state = PopupState::new_hanja(
            "한",
            vec![
                ("X".into(), "x".into()),
                ("A".into(), "a".into()),
                ("B".into(), "b".into()),
                ("C".into(), "c".into()),
                ("D".into(), "d".into()),
                ("E".into(), "e".into()),
                ("F".into(), "f".into()),
                ("G".into(), "g".into()),
                ("H".into(), "h".into()),
                ("I".into(), "i".into()),
            ],
        );
        // X만 즐겨찾기 (정렬 후 [X*, A..I]).
        state.set_bookmark_flags(vec![
            true, false, false, false, false, false, false, false, false, false,
        ]);
        assert_eq!(state.total_pages, 2);
        assert_eq!(state.current_page, 0);

        // X 해제 → stable sort 결과 [A, B, C, D, E, F, G, H, I, X], X의 새 인덱스 = 9.
        state.replace_hanja_items(
            vec![
                "A".into(),
                "B".into(),
                "C".into(),
                "D".into(),
                "E".into(),
                "F".into(),
                "G".into(),
                "H".into(),
                "I".into(),
                "X".into(),
            ],
            vec![
                "a".into(),
                "b".into(),
                "c".into(),
                "d".into(),
                "e".into(),
                "f".into(),
                "g".into(),
                "h".into(),
                "i".into(),
                "x".into(),
            ],
            vec![
                false, false, false, false, false, false, false, false, false, false,
            ],
        );
        state.set_selected_global(9);

        assert_eq!(state.get_item(9), Some("X"));
        assert!(!state.is_bookmarked(9));
        // 새 인덱스가 첫 페이지(0..=8)를 벗어나 페이지 1로 점프해야 한다.
        assert_eq!(state.current_page, 1);
        assert_eq!(state.sel_row, 0); // 9 % 9 = 0
        assert_eq!(state.sel_col, 0); // compact: 단일 컬럼
    }
}
