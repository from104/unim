//! UNIM popup DBus payload 공유 타입.
//!
//! daemon(unim-dbus)이 산출하고 popup-service / GNOME extension / IM 모듈 등
//! popup 시각화를 담당하는 모든 frontend 가 그대로 consume 하는 평면 표현.
//!
//! 한 책임, 한 소유: popup payload 정의는 본 crate 가 단일 SoT.
//! 변경 시 dbus interface 변경(`unim-dbus/src/service.rs`)도 동반 필요.

/// 팝업 페이지 이동 응답 — service.rs 가 `PopupNavigate` 시그널 페이로드로 변환.
#[derive(Debug, Clone, Copy)]
pub struct PopupNavigatePayload {
    pub page: u32,
    pub total_pages: u32,
    pub selected: u32,
    pub rows: u32,
    pub cols: u32,
    pub sel_row: u32,
    pub sel_col: u32,
}

/// 이모지 카테고리 전환 응답 — `ShowEmojiPopupV2` 시그널 재발행용 payload.
#[derive(Debug, Clone)]
pub struct EmojiShowPayload {
    /// 새 활성 카테고리 id (예: "SmileysPeople").
    pub target_cat_id: String,
    /// 해당 카테고리 emoji 풀 전체 (페이지 슬라이싱은 GUI 측 책임).
    pub items: Vec<String>,
    /// 현재 활성 영문 키맵의 상단 행 9 문자.
    pub top_row: String,
    /// MRU Recent 캐시 (popup payload 호환용).
    pub recent: Vec<String>,
    /// 카테고리 메타 9 튜플 (id, ko, en, total).
    pub categories: Vec<(String, String, String, u32)>,
    /// 현재 활성 영문 키맵의 홈 행 9 문자 (이모지 카테고리 단축키 표시용).
    pub home_row: String,
}

/// `PopupRender` 시그널 페이로드 — engine `PopupViewModel` 의 DBus-friendly 평면 표현.
///
/// daemon = SoT. 모든 frontend (GNOME extension / unim-popup-service / 향후 추가될 모듈)
/// 가 본 페이로드를 그대로 렌더링하여 헤더·푸터·셀·탭·확장 아이콘 등을 표시한다.
/// 서식 문자열·visibility 조건이 모두 daemon 산출이라 frontend 간 표시 일관성 자동 보장.
#[derive(Debug, Clone)]
pub struct PopupRenderPayload {
    /// 0=Hanja, 1=SpecialChar, 2=Emoji
    pub kind: u32,
    pub target: String,
    pub header_text: String,
    pub footer_text: String,
    pub show_footer: bool,
    pub rows: u32,
    pub cols: u32,
    pub sel_row: u32,
    pub sel_col: u32,
    pub current_page: u32,
    pub total_pages: u32,
    /// 셀 데이터 — column-major, 길이 = rows * cols. 각 (text, meaning, flags).
    /// flags 비트: 0x01=has_data, 0x02=selected, 0x04=col_highlight,
    ///            0x08=row_highlight, 0x10=bookmarked.
    /// has_data=0 이면 빈 셀 (text/meaning 무시).
    pub cells: Vec<(String, String, u32)>,
    /// 컬럼 헤더 (text, is_active) — 한자 compact 는 빈 벡터.
    pub col_headers: Vec<(String, bool)>,
    /// 행 헤더 (text, is_active).
    pub row_headers: Vec<(String, bool)>,
    /// 한자 expand 토글 아이콘 가시성 (한자 popup 만 true).
    pub expand_visible: bool,
    /// 한자 expand 토글 텍스트 ("⊞" / "⊟").
    pub expand_text: String,
    /// 이모지 좌측 9 카테고리 탭 라벨 (단축키 prefix 포함). 그 외 popup 은 빈 벡터.
    pub tab_labels: Vec<String>,
    /// 이모지 활성 탭 인덱스 (0..9).
    pub active_tab_index: u32,
}

/// PopupRenderPayload 의 cell flag 비트 정의.
pub mod popup_render_flags {
    pub const HAS_DATA: u32 = 0x01;
    pub const SELECTED: u32 = 0x02;
    pub const COL_HIGHLIGHT: u32 = 0x04;
    pub const ROW_HIGHLIGHT: u32 = 0x08;
    pub const BOOKMARKED: u32 = 0x10;
}

/// 한자 후보 응답.
#[derive(Debug)]
pub struct HanjaCandidateResponse {
    /// 변환 대상 문자열
    pub target: String,
    /// 후보 목록 (한자, 뜻풀이)
    pub candidates: Vec<(String, String)>,
    /// 영문 키맵의 상단 행 레이블 (expanded 9x9 컬럼 헤더용; 특수문자와 동일 source).
    pub top_row: String,
    /// PopupRender 페이로드 — Standalone 모드에서 ShowHanjaPopup 직후 popup_render
    /// 시그널 발행에 사용. popup 비활성이면 None.
    pub render_state: Option<PopupRenderPayload>,
}

/// 특수문자 후보 응답.
#[derive(Debug)]
pub struct SpecialCharResponse {
    /// 변환 대상 초성
    pub target: String,
    /// 특수문자 목록
    pub characters: Vec<String>,
    /// 영문 키맵의 상단 행 레이블 (예: "QWERTYUIO")
    pub top_row: String,
    /// PopupRender 페이로드 — Standalone 모드에서 ShowSpecialPopup 직후 popup_render
    /// 시그널 발행에 사용. popup 비활성이면 None.
    pub render_state: Option<PopupRenderPayload>,
}
