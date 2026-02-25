//! 공통 타입 정의
//!
//! 툴킷에 무관한 타입들: 상태, 액션, 상수 등.
//! 향후 `unim-gui-common` 크레이트로 추출될 대상입니다.

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
    /// 한자 팝업 표시
    ShowHanjaPopup {
        target: String,
        candidates: Vec<(String, String)>,
        cursor_x: i32,
        cursor_y: i32,
        cursor_width: i32,
        cursor_height: i32,
    },
    /// 특수문자 팝업 표시
    ShowSpecialPopup {
        target: String,
        characters: Vec<String>,
        top_row: String,
        cursor_x: i32,
        cursor_y: i32,
        cursor_width: i32,
        cursor_height: i32,
    },
    /// 팝업 숨김
    HidePopup,
    /// 설정 다이얼로그 열기
    OpenSettings,
}
