//! `org.atit.unim.Popup` DBus interface 서버 구현.
//!
//! popup 관련 signal·method 의 단일 SoT. daemon(`org.atit.unim.InputContext`) 의
//! popup 표면을 외부 frontend(GNOME extension / popup-service GUI) 가 직접 사용하지
//! 않도록 모두 본 서비스로 이관.
//!
//! 구성:
//!   - signals 8 개 : `show_hanja_popup`, `show_special_popup`, `show_emoji_popup_v2`,
//!     `hide_popup`, `popup_navigate`, `popup_render`, `hanja_bookmark_changed`,
//!     `hanja_candidates_reordered`.
//!   - methods 13 개 : 한자·특수·이모지 CRUD + 페이지 이동 + expand 토글 +
//!     bookmark 토글 + 카테고리 전환 + MRU 조회 + cancel.
//!
//! Phase 1 (본 commit): skeleton — signal 정의 + method stub.
//! Phase 2 : daemon InputContext signal 구독 → 본 인터페이스 signal 재발행.
//! Phase 3 : method body 를 daemon InputContext popup method forward 로 구현.

use std::sync::Arc;
use tokio::sync::Mutex;
use zbus::{interface, SignalContext};

use unim::unim_log;
use unim_dbus::client::{InputContextProxy, InputMethodProxy};

/// `org.atit.unim.Popup` interface 서버.
///
/// daemon InputContextProxy 핸들을 보유하여 Phase 3 에서 method 호출을 daemon 으로
/// forward 한다. Phase 1 에선 핸들 보관만 — method body 는 stub.
pub struct PopupServer {
    /// daemon InputMethodProxy — 글로벌 emoji method (`commit_emoji`,
    /// `set_emoji_category`, `get_emoji_recent`) forward 용. lazy 로 잡아 둔다.
    pub im_proxy: Arc<Mutex<Option<InputMethodProxy<'static>>>>,
    /// 현재 popup-owner InputContext proxy — `unim_gui_common::types::ACTIVE_CONTEXT_PATH`
    /// 에 따라 동적으로 갱신. Phase 3 에서 method forward 시 사용.
    pub ic_proxy: Arc<Mutex<Option<InputContextProxy<'static>>>>,
}

impl PopupServer {
    pub fn new() -> Self {
        Self {
            im_proxy: Arc::new(Mutex::new(None)),
            ic_proxy: Arc::new(Mutex::new(None)),
        }
    }
}

impl Default for PopupServer {
    fn default() -> Self {
        Self::new()
    }
}

#[interface(name = "org.atit.unim.Popup")]
impl PopupServer {
    // =========================================
    // Signals (popup-service → frontend)
    // =========================================

    /// 한자 popup 표시 (daemon InputContext signal 미러).
    #[zbus(signal)]
    pub async fn show_hanja_popup(
        signal_ctx: &SignalContext<'_>,
        target: &str,
        candidates: Vec<(String, String)>,
        top_row: &str,
        cursor_x: i32,
        cursor_y: i32,
        cursor_width: i32,
        cursor_height: i32,
    ) -> zbus::Result<()>;

    /// 특수문자 popup 표시.
    #[zbus(signal)]
    pub async fn show_special_popup(
        signal_ctx: &SignalContext<'_>,
        target: &str,
        characters: Vec<String>,
        top_row: &str,
        cursor_x: i32,
        cursor_y: i32,
        cursor_width: i32,
        cursor_height: i32,
    ) -> zbus::Result<()>;

    /// 이모지 popup 표시 v2.
    #[zbus(signal)]
    #[allow(clippy::too_many_arguments)]
    pub async fn show_emoji_popup_v2(
        signal_ctx: &SignalContext<'_>,
        target_cat_id: &str,
        items: Vec<String>,
        top_row: &str,
        recent: Vec<String>,
        categories: Vec<(String, String, String, u32)>,
        cursor_x: i32,
        cursor_y: i32,
        cursor_width: i32,
        cursor_height: i32,
        home_row: &str,
    ) -> zbus::Result<()>;

    /// popup 숨김.
    #[zbus(signal)]
    pub async fn hide_popup(signal_ctx: &SignalContext<'_>) -> zbus::Result<()>;

    /// popup 페이지·선택 이동.
    #[zbus(signal)]
    #[allow(clippy::too_many_arguments)]
    pub async fn popup_navigate(
        signal_ctx: &SignalContext<'_>,
        page: i32,
        total_pages: i32,
        selected: i32,
        rows: i32,
        cols: i32,
        sel_row: i32,
        sel_col: i32,
    ) -> zbus::Result<()>;

    /// popup 통합 view-model (engine `PopupViewModel` 평면 표현).
    #[zbus(signal)]
    #[allow(clippy::too_many_arguments)]
    pub async fn popup_render(
        signal_ctx: &SignalContext<'_>,
        kind: u32,
        texts: (String, String, String, String),
        layout: (u32, u32, u32, u32, u32, u32),
        flags: (bool, bool),
        cells: Vec<(String, String, u32)>,
        col_headers: Vec<(String, bool)>,
        row_headers: Vec<(String, bool)>,
        tab_labels: Vec<String>,
        active_tab_index: u32,
    ) -> zbus::Result<()>;

    /// 한자 즐겨찾기 상태 변경.
    #[zbus(signal)]
    pub async fn hanja_bookmark_changed(
        signal_ctx: &SignalContext<'_>,
        index: u32,
        bookmarked: bool,
    ) -> zbus::Result<()>;

    /// 한자 후보 재정렬 (즐겨찾기 토글 후 promote/restore).
    #[zbus(signal)]
    #[allow(clippy::too_many_arguments)]
    pub async fn hanja_candidates_reordered(
        signal_ctx: &SignalContext<'_>,
        target: &str,
        hanjas: Vec<String>,
        meanings: Vec<String>,
        bookmarks: Vec<bool>,
        new_cursor: u32,
        page: i32,
        sel_row: i32,
        sel_col: i32,
        bookmarked: bool,
        was_bookmarked: bool,
    ) -> zbus::Result<()>;

    // =========================================
    // Methods (frontend → popup-service → daemon InputContext)
    //
    // Phase 1: stub — 호출 시 로깅 후 default 값 반환.
    // Phase 3: daemon InputContext popup method forward 로 교체.
    // =========================================

    /// 한자 후보 목록 조회 — daemon `GetHanjaCandidates` forward (Phase 3).
    async fn get_hanja_candidates(
        &self,
    ) -> zbus::fdo::Result<(String, Vec<(String, String)>, String)> {
        unim_log!("POPUP", "[Popup] GetHanjaCandidates (stub Phase 1)");
        Ok((String::new(), Vec::new(), String::new()))
    }

    /// 한자 선택 — daemon `SelectHanja` forward.
    async fn select_hanja(&self, _index: u32) -> zbus::fdo::Result<String> {
        unim_log!("POPUP", "[Popup] SelectHanja stub idx={}", _index);
        Ok(String::new())
    }

    /// 한자 즐겨찾기 상태 조회 — daemon `GetHanjaBookmarkStates` forward.
    async fn get_hanja_bookmark_states(&self) -> zbus::fdo::Result<Vec<bool>> {
        unim_log!("POPUP", "[Popup] GetHanjaBookmarkStates stub");
        Ok(Vec::new())
    }

    /// 한자 즐겨찾기 토글 — daemon `ToggleHanjaBookmark` forward.
    async fn toggle_hanja_bookmark(&self, _index: u32) -> zbus::fdo::Result<(u32, bool)> {
        unim_log!("POPUP", "[Popup] ToggleHanjaBookmark stub idx={}", _index);
        Ok((0, false))
    }

    /// popup 페이지 이동 — daemon `PopupChangePage` forward.
    async fn popup_change_page(&self, _direction: i32) -> zbus::fdo::Result<()> {
        unim_log!("POPUP", "[Popup] PopupChangePage stub dir={}", _direction);
        Ok(())
    }

    /// 한자 popup compact↔expanded 토글 — daemon `TogglePopupExpand` forward.
    async fn toggle_popup_expand(&self) -> zbus::fdo::Result<()> {
        unim_log!("POPUP", "[Popup] TogglePopupExpand stub");
        Ok(())
    }

    /// 한자 모드 취소 — daemon `CancelHanja` forward.
    async fn cancel_hanja(&self) -> zbus::fdo::Result<String> {
        unim_log!("POPUP", "[Popup] CancelHanja stub");
        Ok(String::new())
    }

    /// 특수문자 후보 목록 조회 — daemon `GetSpecialCharCandidates` forward.
    async fn get_special_char_candidates(
        &self,
    ) -> zbus::fdo::Result<(String, Vec<String>, String)> {
        unim_log!("POPUP", "[Popup] GetSpecialCharCandidates stub");
        Ok((String::new(), Vec::new(), String::new()))
    }

    /// 특수문자 선택 — daemon `SelectSpecialChar` forward.
    async fn select_special_char(&self, _idx: u32) -> zbus::fdo::Result<String> {
        unim_log!("POPUP", "[Popup] SelectSpecialChar stub idx={}", _idx);
        Ok(String::new())
    }

    /// 특수문자 모드 취소 — daemon `CancelSpecialChar` forward.
    async fn cancel_special_char(&self) -> zbus::fdo::Result<String> {
        unim_log!("POPUP", "[Popup] CancelSpecialChar stub");
        Ok(String::new())
    }

    /// 이모지 commit — daemon InputMethod `CommitEmoji` forward.
    async fn commit_emoji(&self, _emoji: &str) -> zbus::fdo::Result<()> {
        unim_log!("POPUP", "[Popup] CommitEmoji stub '{}'", _emoji);
        Ok(())
    }

    /// 이모지 카테고리 전환 — daemon InputMethod `SetEmojiCategory` forward.
    async fn set_emoji_category(&self, _idx: u32) -> zbus::fdo::Result<()> {
        unim_log!("POPUP", "[Popup] SetEmojiCategory stub idx={}", _idx);
        Ok(())
    }

    /// 이모지 MRU 조회 — daemon InputMethod `GetEmojiRecent` forward.
    async fn get_emoji_recent(&self) -> zbus::fdo::Result<Vec<String>> {
        unim_log!("POPUP", "[Popup] GetEmojiRecent stub");
        Ok(Vec::new())
    }
}
