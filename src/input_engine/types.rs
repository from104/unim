//! 입력 처리 결과 및 팝업 액션 타입.
//!
//! `PopupAction`(엔진 → 프런트엔드 알림)과 `InputResult`(키 처리 후 상태 변화)를
//! 정의한다. 외부에서는 `crate::input_engine::{PopupAction, InputResult}` 경로로
//! 동일하게 접근한다.

use crate::keycode::KeyCode;

/// 자동 영문 전환 트리거 — 두 카테고리.
///
/// - `Functional`: 제어 키(Escape/Tab/F*/Arrow* …) 또는 Shift 명시된 문자 키.
///   `keycode == code && shift 조건` 으로 비교한다 (현 로직 유지).
/// - `Character`: 키맵을 거쳐 산출된 char 와 비교한다. 비-QWERTY 한국어
///   레이아웃(예: 세벌식390) 에서도 사용자가 의도한 문자(`'/'` 등) 가 어떤
///   keycode 로 매핑되든 정확히 잡아낸다. shift 조건은 별도로 비교하지 않으며,
///   사용자가 `'/'` 와 `'?'` 를 구분하려면 둘 다 등록해야 한다.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum AutoEnglishTrigger {
    /// KeyCode 기반 매칭.
    /// - `shift = None`: shift 무관 매칭
    /// - `shift = Some(true)`: shift 필수
    /// - `shift = Some(false)`: shift 없을 때만
    Functional { code: KeyCode, shift: Option<bool> },
    /// 키맵 산출 char 매칭. shift 무관 — 산출된 글자가 `ch` 와 같으면 발동.
    Character(char),
}

/// 팝업 페이지 이동 방향 (마우스 ◀/▶ 버튼 등에서 사용).
///
/// 한자/특수문자/이모지 팝업의 wrap-around 페이지 이동에 공통으로 쓰인다.
/// `Prev`는 이전 페이지(첫 페이지에서는 마지막으로 wrap), `Next`는 다음 페이지
/// (마지막 페이지에서는 첫 페이지로 wrap).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PageDirection {
    /// 이전 페이지로 이동 (첫 페이지면 마지막으로 wrap).
    Prev,
    /// 다음 페이지로 이동 (마지막 페이지면 첫 페이지로 wrap).
    Next,
}

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
    /// PR #1 (emoji overhaul, OPTION X): payload 가 한자/특수문자처럼
    /// 카테고리·페이지·MRU 데이터를 함께 push 하도록 확장됐다.
    ///
    /// - `target_cat_id`: 시작 카테고리 id ("Recent" / "SmileysPeople" / ...).
    /// - `items`: 시작 카테고리의 1 페이지 (최대 81개) 슬라이스.
    /// - `top_row`: 활성 영문 키맵 상단 9 문자 (Letter 키 점프 + 컬럼 헤더).
    /// - `recent`: 'Recent' 탭에 표시할 MRU 이모지 (시작 시 캐시).
    /// - `categories`: 좌측 9 탭 메타 — `(id, ko, en, count)` 4-튜플.
    ///
    /// 옛 `ShowEmoji` (cursor only) 시그널은 PR #5 cleanup 까지 dual-emit 한다 —
    /// `unim-dbus/src/service.rs` 가 양쪽 시그널을 모두 발행 (backward compat).
    ShowEmoji {
        target_cat_id: String,
        items: Vec<String>,
        top_row: String,
        recent: Vec<String>,
        categories: Vec<(String, String, String, u32)>,
        /// 활성 영문 키맵 홈 행 9 문자 (이모지 카테고리 단축키 표시용).
        home_row: String,
    },
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
        /// 토글된 한자의 새 즐겨찾기 상태 (편의용).
        /// `was_bookmarked`와 항상 반대 (toggle).
        bookmarked: bool,
        /// 토글 직전의 즐겨찾기 상태.
        ///
        /// frontend가 ON→OFF 전환을 감지해 "원위치 점프 flash" 시각 신호를 띄우는 용도.
        /// `was_bookmarked == true && bookmarked == false` 이면 사용자가 별을 해제한 케이스.
        was_bookmarked: bool,
    },
    /// 마우스 ◀/▶ 버튼 등 외부 RPC로 페이지가 이동했음을 알리는 액션.
    ///
    /// 기존 `PopupNavigate`로 prefix-가능하지만, frontend가 "사용자가 명시적으로
    /// 페이지를 바꿨다"는 시각 단서(짧은 transition 등)를 띄울 수 있도록 별도 액션으로 분리.
    /// payload는 새 페이지 인덱스 단일 값 — 페이지 외 상태(cursor 등)는
    /// 동시에 함께 발행되는 `PopupNavigate`가 전달한다.
    PageJump {
        /// 이동 후 페이지 인덱스 (0-based).
        page_index: u32,
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
