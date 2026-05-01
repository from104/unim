//! 입력 처리 결과 및 팝업 액션 타입.
//!
//! `PopupAction`(엔진 → 프런트엔드 알림)과 `InputResult`(키 처리 후 상태 변화)를
//! 정의한다. 외부에서는 `crate::input_engine::{PopupAction, InputResult}` 경로로
//! 동일하게 접근한다.

/// 팝업 동작 (ProcessKeyEvent 후 발생)
#[derive(Debug, Clone)]
pub enum PopupAction {
    /// 한자 팝업 표시
    ShowHanja {
        target: String,
        candidates: Vec<(String, String)>,
        /// 활성 영문 키맵의 상단 행 라벨 (expanded 9x9 컬럼 헤더용).
        top_row: String,
    },
    /// 특수문자 팝업 표시
    ShowSpecial {
        target: String,
        characters: Vec<String>,
        top_row: String,
    },
    /// 이모지 팝업 표시 (Super+. 단축키)
    ///
    /// 엔진은 트리거만 담당하고, GUI가 카테고리/검색/즐겨찾기 상태를 자체 관리합니다.
    ShowEmoji,
    /// 팝업 숨김
    HidePopup,
    /// 페이지/선택 변경 (UI 업데이트용)
    PopupNavigate {
        page: usize,
        total_pages: usize,
        selected: usize,
        rows: usize,
        cols: usize,
        sel_row: usize,
        sel_col: usize,
    },
    /// 한자 즐겨찾기 상태 변경 (UI 갱신용)
    HanjaBookmarkChanged {
        /// 전체 후보 인덱스 (0-based)
        index: usize,
        /// 새 즐겨찾기 상태 (true=등록됨)
        bookmarked: bool,
    },
    /// 한자 후보가 즐겨찾기 토글로 재정렬됨.
    ///
    /// frontend는 후보 리스트 + 즐겨찾기 플래그 + cursor 위치를 한 번에 교체해야 한다.
    /// SelectHanja 인덱스 미스매치를 피하려면 frontend가 이 액션을 받기 전엔
    /// 새 후보로 selection을 보내지 않아야 한다.
    HanjaCandidatesReordered {
        /// 변환 대상 음절 (그대로 유지)
        target: String,
        /// (한자, 뜻) 쌍 — 재정렬된 새 순서
        candidates: Vec<(String, String)>,
        /// candidates와 동일 순서의 즐겨찾기 플래그
        bookmarks: Vec<bool>,
        /// 토글된 한자의 새 전체 인덱스 (커서 점프 위치)
        new_cursor: usize,
        /// 새 페이지 (0-based)
        page: usize,
        /// 새 sel_row
        sel_row: usize,
        /// 새 sel_col
        sel_col: usize,
        /// 토글된 한자의 새 즐겨찾기 상태 (편의용)
        bookmarked: bool,
    },
}

/// 입력 처리 결과
///
/// 키 입력 처리 후의 상태 변화를 나타냅니다.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[repr(C)]
pub struct InputResult {
    /// 키 입력이 소비되었는지 여부
    /// true면 애플리케이션으로 전달하지 않음
    pub consumed: bool,
    /// preedit 문자열이 변경되었는지 여부
    pub preedit_changed: bool,
    /// commit 문자열이 변경되었는지 여부  
    pub commit_changed: bool,
    /// 한자 후보가 사용 가능한지 여부
    pub hanja_candidates_available: bool,
    /// 특수문자 후보가 사용 가능한지 여부
    pub special_char_candidates_available: bool,
}

impl InputResult {
    /// 키가 소비되지 않은 결과
    pub fn not_consumed() -> Self {
        Self {
            consumed: false,
            preedit_changed: false,
            commit_changed: false,
            hanja_candidates_available: false,
            special_char_candidates_available: false,
        }
    }

    /// 키가 소비된 결과
    pub fn consumed() -> Self {
        Self {
            consumed: true,
            preedit_changed: false,
            commit_changed: false,
            hanja_candidates_available: false,
            special_char_candidates_available: false,
        }
    }

    /// preedit 변경된 결과
    pub fn preedit_updated() -> Self {
        Self {
            consumed: true,
            preedit_changed: true,
            commit_changed: false,
            hanja_candidates_available: false,
            special_char_candidates_available: false,
        }
    }

    /// commit 발생한 결과
    pub fn committed() -> Self {
        Self {
            consumed: true,
            preedit_changed: true,
            commit_changed: true,
            hanja_candidates_available: false,
            special_char_candidates_available: false,
        }
    }

    /// commit 발생 후 키 통과 (Enter, Tab 등 특수키용)
    /// commit은 발생하지만 키는 애플리케이션으로 전달됨
    pub fn committed_passthrough() -> Self {
        Self {
            consumed: false,
            preedit_changed: true,
            commit_changed: true,
            hanja_candidates_available: false,
            special_char_candidates_available: false,
        }
    }

    /// 한자 후보 사용 가능 (preedit 유지 — 팝업 중 조합 문자 표시)
    pub fn hanja_candidates() -> Self {
        Self {
            consumed: true,
            preedit_changed: true,
            commit_changed: false,
            hanja_candidates_available: true,
            special_char_candidates_available: false,
        }
    }

    /// 특수문자 후보 사용 가능 (preedit 유지 — 팝업 중 조합 문자 표시)
    pub fn special_char_candidates() -> Self {
        Self {
            consumed: true,
            preedit_changed: true,
            commit_changed: false,
            hanja_candidates_available: false,
            special_char_candidates_available: true,
        }
    }
}
