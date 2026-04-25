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
    /// 팝업 숨김
    HidePopup,
    /// 한자 즐겨찾기 상태 변경 (index, bookmarked)
    HanjaBookmarkChanged {
        index: u32,
        bookmarked: bool,
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
