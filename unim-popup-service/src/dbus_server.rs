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

use zbus::{interface, Connection, SignalContext};

use unim::unim_log;
use unim_dbus::client::{InputContextProxy, InputMethodProxy};
use unim_gui_common::types::ACTIVE_CONTEXT_PATH;

/// `org.atit.unim.Popup` 가 노출되는 단일 path. PopupServer 객체 위치.
pub const POPUP_OBJECT_PATH: &str = "/org/atit/unim/popup";

/// `org.atit.unim.Popup` interface 서버.
///
/// daemon InputMethod / InputContext popup method 를 매 호출 시 proxy 생성으로
/// forward 한다. InputContext path 는 `ACTIVE_CONTEXT_PATH` (popup-owner 가
/// 직전 ShowHanjaPopup 등으로 갱신한 path) 를 사용.
pub struct PopupServer {
    conn: Connection,
}

impl PopupServer {
    pub fn new(conn: Connection) -> Self {
        Self { conn }
    }

    /// 현재 popup-owner InputContext path 조회. 없으면 Err.
    fn active_context_path(&self) -> zbus::fdo::Result<String> {
        ACTIVE_CONTEXT_PATH
            .lock()
            .ok()
            .and_then(|guard| guard.clone())
            .ok_or_else(|| {
                zbus::fdo::Error::Failed(
                    "active InputContext path 없음 (popup signal 미수신 상태)".into(),
                )
            })
    }

    /// daemon InputContext proxy 동적 생성. popup-owner path 기반.
    async fn ic_proxy(&self) -> zbus::fdo::Result<InputContextProxy<'static>> {
        let path = self.active_context_path()?;
        let owned_path = zbus::zvariant::ObjectPath::try_from(path.clone())
            .map_err(|e| {
                zbus::fdo::Error::Failed(format!("ObjectPath 변환 실패 '{}': {}", path, e))
            })?
            .into_owned();
        InputContextProxy::builder(&self.conn)
            .path(owned_path)
            .map_err(|e| zbus::fdo::Error::Failed(format!("InputContextProxy path 실패: {}", e)))?
            .build()
            .await
            .map_err(|e| zbus::fdo::Error::Failed(format!("InputContextProxy build 실패: {}", e)))
    }

    /// daemon InputMethod proxy 동적 생성. path/service 고정.
    async fn im_proxy(&self) -> zbus::fdo::Result<InputMethodProxy<'_>> {
        InputMethodProxy::new(&self.conn)
            .await
            .map_err(|e| zbus::fdo::Error::Failed(format!("InputMethodProxy 실패: {}", e)))
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

    /// 한자 후보 목록 조회 — daemon `GetHanjaCandidates` forward.
    /// daemon proxy 는 `(target, Vec<(hanja, meaning)>)` 만 반환하므로 `top_row` 는
    /// 빈 문자열로 채운다 (top_row 는 ShowHanjaPopup 시그널 payload 로 따로 전달됨).
    async fn get_hanja_candidates(
        &self,
    ) -> zbus::fdo::Result<(String, Vec<(String, String)>, String)> {
        let proxy = self.ic_proxy().await?;
        let (target, candidates) = proxy
            .get_hanja_candidates()
            .await
            .map_err(|e| zbus::fdo::Error::Failed(format!("GetHanjaCandidates forward: {}", e)))?;
        Ok((target, candidates, String::new()))
    }

    async fn select_hanja(&self, index: u32) -> zbus::fdo::Result<String> {
        let proxy = self.ic_proxy().await?;
        proxy
            .select_hanja(index)
            .await
            .map_err(|e| zbus::fdo::Error::Failed(format!("SelectHanja forward: {}", e)))
    }

    async fn get_hanja_bookmark_states(&self) -> zbus::fdo::Result<Vec<bool>> {
        let proxy = self.ic_proxy().await?;
        proxy.get_hanja_bookmark_states().await.map_err(|e| {
            zbus::fdo::Error::Failed(format!("GetHanjaBookmarkStates forward: {}", e))
        })
    }

    async fn toggle_hanja_bookmark(&self, index: u32) -> zbus::fdo::Result<(u32, bool)> {
        let proxy = self.ic_proxy().await?;
        proxy.toggle_hanja_bookmark(index).await.map_err(|e| {
            zbus::fdo::Error::Failed(format!("ToggleHanjaBookmark forward: {}", e))
        })
    }

    async fn popup_change_page(&self, direction: i32) -> zbus::fdo::Result<()> {
        let proxy = self.ic_proxy().await?;
        proxy
            .popup_change_page(direction)
            .await
            .map_err(|e| zbus::fdo::Error::Failed(format!("PopupChangePage forward: {}", e)))
    }

    async fn toggle_popup_expand(&self) -> zbus::fdo::Result<()> {
        let proxy = self.ic_proxy().await?;
        proxy
            .toggle_popup_expand()
            .await
            .map_err(|e| zbus::fdo::Error::Failed(format!("TogglePopupExpand forward: {}", e)))
    }

    async fn cancel_hanja(&self) -> zbus::fdo::Result<String> {
        let proxy = self.ic_proxy().await?;
        proxy
            .cancel_hanja()
            .await
            .map_err(|e| zbus::fdo::Error::Failed(format!("CancelHanja forward: {}", e)))
    }

    async fn get_special_char_candidates(
        &self,
    ) -> zbus::fdo::Result<(String, Vec<String>, String)> {
        let proxy = self.ic_proxy().await?;
        proxy.get_special_char_candidates().await.map_err(|e| {
            zbus::fdo::Error::Failed(format!("GetSpecialCharCandidates forward: {}", e))
        })
    }

    async fn select_special_char(&self, idx: u32) -> zbus::fdo::Result<String> {
        let proxy = self.ic_proxy().await?;
        proxy
            .select_special_char(idx)
            .await
            .map_err(|e| zbus::fdo::Error::Failed(format!("SelectSpecialChar forward: {}", e)))
    }

    async fn cancel_special_char(&self) -> zbus::fdo::Result<String> {
        let proxy = self.ic_proxy().await?;
        proxy
            .cancel_special_char()
            .await
            .map_err(|e| zbus::fdo::Error::Failed(format!("CancelSpecialChar forward: {}", e)))
    }

    async fn commit_emoji(&self, emoji: &str) -> zbus::fdo::Result<()> {
        let proxy = self.im_proxy().await?;
        proxy
            .commit_emoji(emoji)
            .await
            .map_err(|e| zbus::fdo::Error::Failed(format!("CommitEmoji forward: {}", e)))
    }

    async fn set_emoji_category(&self, idx: u32) -> zbus::fdo::Result<()> {
        let proxy = self.im_proxy().await?;
        proxy
            .set_emoji_category(idx)
            .await
            .map_err(|e| zbus::fdo::Error::Failed(format!("SetEmojiCategory forward: {}", e)))
    }

    async fn get_emoji_recent(&self) -> zbus::fdo::Result<Vec<String>> {
        let proxy = self.im_proxy().await?;
        proxy
            .get_emoji_recent()
            .await
            .map_err(|e| zbus::fdo::Error::Failed(format!("GetEmojiRecent forward: {}", e)))
    }
}

// =============================================================================
// Phase 2 — daemon InputContext signal 구독 → PopupServer signal 재발행.
// =============================================================================

/// daemon 의 InputContext popup signal 을 path_namespace 로 모두 구독하여
/// 본 서비스의 `/org/atit/unim/popup` path 의 `org.atit.unim.Popup` interface
/// signal 로 그대로 재발행한다.
///
/// 외부 frontend(GNOME extension 등) 는 popup-service 의 signal 만 구독하면
/// daemon 위치를 알 필요가 없다. popup-service GTK4 popup window 자체는 별도
/// 경로(`unim_gui_common::dbus_client::watch_dbus_signals` → popup_tx 채널) 로
/// 동일 signal 을 받아 그리므로 영향 없음.
///
/// 영원히 실행되는 future. 메인 connection 을 공유하므로 connection 종료 시 함께 종료.
pub async fn forward_daemon_popup_signals(connection: Connection) {
    use futures_util::StreamExt;

    let rule = zbus::MatchRule::builder()
        .msg_type(zbus::message::Type::Signal)
        .interface("org.atit.unim.InputContext")
        .expect("interface name 정상")
        .path_namespace("/org/atit/unim")
        .expect("path namespace 정상")
        .build();

    let mut stream =
        match zbus::MessageStream::for_match_rule(rule, &connection, Some(64)).await {
            Ok(s) => s,
            Err(e) => {
                unim_log!("INDICATOR", "[Popup] daemon signal 구독 실패: {}", e);
                return;
            }
        };

    let signal_ctx = match SignalContext::new(&connection, POPUP_OBJECT_PATH) {
        Ok(c) => c,
        Err(e) => {
            unim_log!("INDICATOR", "[Popup] SignalContext 생성 실패: {}", e);
            return;
        }
    };

    unim_log!(
        "INDICATOR",
        "[Popup] daemon InputContext popup signal → org.atit.unim.Popup 재발행 시작"
    );

    while let Some(Ok(msg)) = stream.next().await {
        let header = msg.header();
        let member = match header.member() {
            Some(m) => m.to_string(),
            None => continue,
        };
        if let Err(e) = re_emit(&signal_ctx, &msg, member.as_str()).await {
            unim_log!("INDICATOR", "[Popup] {} 재발행 실패: {}", member, e);
        }
    }

    unim_log!("INDICATOR", "[Popup] daemon signal 스트림 종료");
}

/// signal 멤버명으로 분기하여 PopupServer signal 발행.
///
/// daemon InputContext signal 의 시그너처와 PopupServer signal 의 시그너처는
/// Phase 1 설계 시 1:1 일치하도록 맞춰 두었다. deserialize 실패는 시그너처
/// drift 의 신호이므로 ERROR 로그.
async fn re_emit(
    ctx: &SignalContext<'_>,
    msg: &zbus::Message,
    member: &str,
) -> zbus::Result<()> {
    match member {
        "ShowHanjaPopup" => {
            let (target, candidates, top_row, x, y, w, h): (
                String,
                Vec<(String, String)>,
                String,
                i32,
                i32,
                i32,
                i32,
            ) = msg.body().deserialize()?;
            PopupServer::show_hanja_popup(ctx, &target, candidates, &top_row, x, y, w, h)
                .await?;
        }
        "ShowSpecialPopup" => {
            let (target, characters, top_row, x, y, w, h): (
                String,
                Vec<String>,
                String,
                i32,
                i32,
                i32,
                i32,
            ) = msg.body().deserialize()?;
            PopupServer::show_special_popup(ctx, &target, characters, &top_row, x, y, w, h)
                .await?;
        }
        "ShowEmojiPopupV2" => {
            let (target_cat_id, items, top_row, recent, categories, x, y, w, h, home_row): (
                String,
                Vec<String>,
                String,
                Vec<String>,
                Vec<(String, String, String, u32)>,
                i32,
                i32,
                i32,
                i32,
                String,
            ) = msg.body().deserialize()?;
            PopupServer::show_emoji_popup_v2(
                ctx,
                &target_cat_id,
                items,
                &top_row,
                recent,
                categories,
                x,
                y,
                w,
                h,
                &home_row,
            )
            .await?;
        }
        "HidePopup" => {
            PopupServer::hide_popup(ctx).await?;
        }
        "PopupNavigate" => {
            let (page, total_pages, selected, rows, cols, sel_row, sel_col): (
                i32,
                i32,
                i32,
                i32,
                i32,
                i32,
                i32,
            ) = msg.body().deserialize()?;
            PopupServer::popup_navigate(
                ctx,
                page,
                total_pages,
                selected,
                rows,
                cols,
                sel_row,
                sel_col,
            )
            .await?;
        }
        "PopupRender" => {
            type PopupRenderTuple = (
                u32,
                (String, String, String, String),
                (u32, u32, u32, u32, u32, u32),
                (bool, bool),
                Vec<(String, String, u32)>,
                Vec<(String, bool)>,
                Vec<(String, bool)>,
                Vec<String>,
                u32,
            );
            let (kind, texts, layout, flags, cells, col_headers, row_headers, tab_labels, active_tab_index): PopupRenderTuple =
                msg.body().deserialize()?;
            PopupServer::popup_render(
                ctx,
                kind,
                texts,
                layout,
                flags,
                cells,
                col_headers,
                row_headers,
                tab_labels,
                active_tab_index,
            )
            .await?;
        }
        "HanjaBookmarkChanged" => {
            let (index, bookmarked): (u32, bool) = msg.body().deserialize()?;
            PopupServer::hanja_bookmark_changed(ctx, index, bookmarked).await?;
        }
        "HanjaCandidatesReordered" => {
            let (
                target,
                hanjas,
                meanings,
                bookmarks,
                new_cursor,
                page,
                sel_row,
                sel_col,
                bookmarked,
                was_bookmarked,
            ): (
                String,
                Vec<String>,
                Vec<String>,
                Vec<bool>,
                u32,
                i32,
                i32,
                i32,
                bool,
                bool,
            ) = msg.body().deserialize()?;
            PopupServer::hanja_candidates_reordered(
                ctx,
                &target,
                hanjas,
                meanings,
                bookmarks,
                new_cursor,
                page,
                sel_row,
                sel_col,
                bookmarked,
                was_bookmarked,
            )
            .await?;
        }
        _ => {
            // popup 외 signal (CommitText, UpdatePreedit 등) — 무시.
        }
    }
    Ok(())
}
