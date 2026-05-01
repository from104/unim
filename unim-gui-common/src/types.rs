//! 공통 타입 정의
//!
//! 툴킷에 무관한 타입들: 상태, 액션, 상수 등.

use std::sync::mpsc::Sender;
use std::sync::Mutex;

use unim::status::InputCategory;

/// 현재 활성 InputContext 경로 (팝업 콜백에서 DBus 호출 시 사용)
pub static ACTIVE_CONTEXT_PATH: std::sync::LazyLock<Mutex<Option<String>>> =
    std::sync::LazyLock::new(|| Mutex::new(None));

/// 설정 오픈 요청용 전역 Sender
pub static SETTINGS_TX: Mutex<Option<Sender<GuiAction>>> = Mutex::new(None);

/// DBus 서비스 이름
pub const UNIM_BUS_NAME: &str = "org.atit.unim.InputMethod";

/// 인디케이터 상태
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct IndicatorState {
    pub category: InputCategory,
}

impl Default for IndicatorState {
    fn default() -> Self {
        Self {
            category: InputCategory::English,
        }
    }
}

/// GUI 액션 (트레이/DBus → UI 이벤트 루프)
#[derive(Debug, Clone)]
pub enum GuiAction {
    ShowModePopup,
    UpdateCategory(InputCategory),
    /// 설정 다이얼로그 열기
    OpenSettings,
    /// 한자 팝업 표시
    ShowHanjaPopup {
        context_path: String,
        target: String,
        candidates: Vec<(String, String)>,
        /// 활성 영문 키맵 top_row (expanded 9x9 컬럼 헤더; 특수문자와 동일 source).
        top_row: String,
        x: i32,
        y: i32,
        w: i32,
        h: i32,
    },
    /// 특수문자 팝업 표시
    ShowSpecialPopup {
        context_path: String,
        target: String,
        characters: Vec<String>,
        top_row: String,
        x: i32,
        y: i32,
        w: i32,
        h: i32,
    },
    /// 이모지 팝업 표시 (Super+. 트리거).
    ///
    /// PR #2 (emoji overhaul, OPTION X engine-driven): payload 가 한자/특수문자처럼
    /// 카테고리·페이지·MRU 데이터를 통째로 담는다 (`ShowEmojiPopupV2` 시그널).
    /// 옛 `ShowEmojiPopup` 시그널 (cursor 만) 은 dbus_client 에서 v2 가 도착하면
    /// drop 되어 중복 표시를 막는다 — PR #5 에서 v1 시그널 자체 제거 예정.
    ShowEmojiPopup {
        context_path: String,
        /// 시작 카테고리 id ("Recent"/"SmileysPeople"/.../"Flags").
        target_cat_id: String,
        /// 시작 카테고리의 emoji 풀 (전체).
        items: Vec<String>,
        /// 활성 영문 키맵 상단 9 문자 (Letter 키 점프 + 컬럼 헤더).
        top_row: String,
        /// 'Recent' 탭 캐시 (MRU 81개).
        recent: Vec<String>,
        /// 좌측 9 탭 메타 — `(id, ko, en, count)` 튜플 9개.
        categories: Vec<(String, String, String, u32)>,
        x: i32,
        y: i32,
        w: i32,
        h: i32,
    },
    /// 팝업 숨김
    HidePopup,
    /// 한자 즐겨찾기 상태 변경 (index, bookmarked)
    HanjaBookmarkChanged {
        index: u32,
        bookmarked: bool,
    },
    /// 한자 즐겨찾기 상태 일괄 적용 (첫 렌더 색상 동기화용)
    HanjaBookmarkStatesFetched {
        states: Vec<bool>,
    },
    /// 한자 후보 재정렬 (즐겨찾기 토글 직후, 커서 점프 포함)
    HanjaCandidatesReordered {
        candidates: Vec<(String, String)>,
        bookmarks: Vec<bool>,
        new_cursor: u32,
        page: i32,
        sel_row: i32,
        sel_col: i32,
    },
    /// 팝업 네비게이션 (페이지/선택 변경)
    PopupNavigate {
        page: i32,
        total_pages: i32,
        selected: i32,
        rows: i32,
        cols: i32,
        sel_row: i32,
        sel_col: i32,
    },
}
