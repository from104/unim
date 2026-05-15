//! popup-service `org.atit.unim.Popup` interface 호출 헬퍼.
//!
//! popup 클릭/페이지/취소/북마크/이모지 commit 등 frontend → popup-service RPC.
//! 호출 대상은 모두 다음으로 고정:
//!   - service : `org.atit.unim.PopupService`
//!   - path    : `/org/atit/unim/popup`
//!   - interface: `org.atit.unim.Popup`
//!
//! popup-owner InputContext routing 은 popup-service 내부의 forward 계층
//! ([`crate::types::ACTIVE_CONTEXT_PATH`] 참조) 가 담당한다. caller 는 path 를
//! 알 필요 없음.
//!
//! 모든 헬퍼는 fire-and-forget — std::thread + 임시 tokio 런타임으로 메인 스레드
//! 를 블록하지 않는다.

use unim::unim_log;

use crate::types::{GuiAction, ACTIVE_CONTEXT_PATH, SETTINGS_TX};

// popup-service Popup interface target — 단일 SoT.
const POPUP_BUS: &str = "org.atit.unim.PopupService";
const POPUP_PATH: &str = "/org/atit/unim/popup";
const POPUP_IFACE: &str = "org.atit.unim.Popup";

// 특수문자 그리드는 column-major 9×9 — index 계산에 사용.
const SPECIAL_MAX_ROWS: usize = 9;

// ─────────────────────────────────────────────────────────────
// 공통 헬퍼
// ─────────────────────────────────────────────────────────────

/// popup 활성 가드. ACTIVE_CONTEXT_PATH 가 비어있으면 popup 비활성으로 간주.
fn popup_active() -> bool {
    ACTIVE_CONTEXT_PATH
        .lock()
        .ok()
        .and_then(|p| p.clone())
        .is_some()
}

/// popup-service Popup interface 의 method 를 fire-and-forget 호출.
///
/// 인자는 zbus body tuple. 반환값은 무시 (`Result<_, _>` 만 캡처).
fn call_popup_service<Args>(method: &'static str, args: Args)
where
    Args: serde::Serialize + zbus::zvariant::DynamicType + Send + 'static,
{
    std::thread::spawn(move || {
        let rt = match tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
        {
            Ok(r) => r,
            Err(e) => {
                unim_log!("INDICATOR", "[Popup] tokio rt 실패 ({}): {}", method, e);
                return;
            }
        };
        rt.block_on(async move {
            let conn = match zbus::Connection::session().await {
                Ok(c) => c,
                Err(e) => {
                    unim_log!("INDICATOR", "[Popup] 세션 버스 ({}) 실패: {}", method, e);
                    return;
                }
            };
            let proxy = match zbus::Proxy::new(&conn, POPUP_BUS, POPUP_PATH, POPUP_IFACE).await {
                Ok(p) => p,
                Err(e) => {
                    unim_log!("INDICATOR", "[Popup] proxy ({}) 실패: {}", method, e);
                    return;
                }
            };
            if let Err(e) = proxy.call::<_, _, ()>(method, &args).await {
                unim_log!("INDICATOR", "[Popup] RPC ({}) 실패: {}", method, e);
            }
        });
    });
}

// ─────────────────────────────────────────────────────────────
// 한자 북마크 상태 비동기 조회 (응답 필요)
// ─────────────────────────────────────────────────────────────

/// popup-service `GetHanjaBookmarkStates` 호출 후 결과를 `GuiAction::HanjaBookmarkStatesFetched`
/// 로 GUI 이벤트 루프에 전달한다.
///
/// 호출자가 context_path 를 보유한 시점에 호출되지만, popup-service Popup interface
/// 는 path 가 고정이므로 인자는 무시. 시그너처는 호출자 변경 최소화를 위해 유지.
pub fn fetch_bookmark_states_async(_context_path: String) {
    let tx_opt = SETTINGS_TX.lock().ok().and_then(|g| g.clone());
    let Some(tx) = tx_opt else { return };

    std::thread::spawn(move || {
        let rt = match tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
        {
            Ok(r) => r,
            Err(_) => return,
        };
        rt.block_on(async move {
            let conn = match zbus::Connection::session().await {
                Ok(c) => c,
                Err(_) => return,
            };
            let proxy = match zbus::Proxy::new(&conn, POPUP_BUS, POPUP_PATH, POPUP_IFACE).await {
                Ok(p) => p,
                Err(_) => return,
            };
            let states: Result<Vec<bool>, _> = proxy.call("GetHanjaBookmarkStates", &()).await;
            if let Ok(states) = states {
                let _ = tx.send(GuiAction::HanjaBookmarkStatesFetched { states });
            }
        });
    });
}

// ─────────────────────────────────────────────────────────────
// popup 페이지·확장·북마크
// ─────────────────────────────────────────────────────────────

pub fn popup_change_page_via_dbus(direction: i32) {
    if !popup_active() {
        return;
    }
    unim_log!("INDICATOR", "[Popup] PopupChangePage 호출: dir={}", direction);
    call_popup_service("PopupChangePage", (direction,));
}

pub fn toggle_popup_expand_via_dbus() {
    if !popup_active() {
        return;
    }
    unim_log!("INDICATOR", "[Popup] TogglePopupExpand 호출");
    call_popup_service("TogglePopupExpand", ());
}

pub fn toggle_hanja_bookmark_via_dbus(global_index: u32) {
    if !popup_active() {
        return;
    }
    unim_log!(
        "INDICATOR",
        "[Popup] ToggleHanjaBookmark 호출: idx={}",
        global_index
    );
    call_popup_service("ToggleHanjaBookmark", (global_index,));
}

// ─────────────────────────────────────────────────────────────
// 한자 선택 / 취소
// ─────────────────────────────────────────────────────────────

pub fn select_hanja_via_dbus(page_local_index: u32) {
    if !popup_active() {
        return;
    }
    unim_log!(
        "INDICATOR",
        "[Popup] SelectHanja 호출: idx={}",
        page_local_index
    );
    call_popup_service("SelectHanja", (page_local_index,));
}

/// 팝업이 숨겨질 때 엔진 한자 상태 정리 (window.connect_hide 콜백). 활성 가드 없이
/// 호출 — popup 비활성 시 popup-service forward 에서 active path 부재로 즉시 fail
/// 로그만 남기고 종료.
pub fn cancel_hanja_via_dbus() {
    unim_log!("INDICATOR", "[Popup] CancelHanja 호출 (hide hook)");
    call_popup_service("CancelHanja", ());
}

// ─────────────────────────────────────────────────────────────
// 특수문자 선택 / 취소
// ─────────────────────────────────────────────────────────────

pub fn select_special_via_dbus(col: usize, row: usize) {
    let index = (col * SPECIAL_MAX_ROWS + row) as u32;
    if !popup_active() {
        return;
    }
    unim_log!(
        "INDICATOR",
        "[Popup] SelectSpecialChar 호출: idx={} (col={}, row={})",
        index,
        col,
        row
    );
    call_popup_service("SelectSpecialChar", (index,));
}

pub fn cancel_special_via_dbus() {
    unim_log!("INDICATOR", "[Popup] CancelSpecialChar 호출 (hide hook)");
    call_popup_service("CancelSpecialChar", ());
}

// ─────────────────────────────────────────────────────────────
// 이모지 커밋 / 카테고리
// ─────────────────────────────────────────────────────────────

/// 셀 클릭 콜백에서 호출 — popup-service `CommitEmoji` RPC.
pub fn commit_emoji_via_dbus(emoji_str: String) {
    if emoji_str.is_empty() {
        return;
    }
    if !popup_active() {
        unim_log!(
            "INDICATOR",
            "[EmojiPopup] CommitEmoji 실패: ACTIVE_CONTEXT_PATH 비어있음"
        );
        return;
    }
    unim_log!("INDICATOR", "[EmojiPopup] CommitEmoji 호출: '{}'", emoji_str);
    // (String,) tuple — popup-service 인터페이스 시그너처 일치.
    call_popup_service("CommitEmoji", (emoji_str,));
}

/// 탭 클릭 콜백 — popup-service `SetEmojiCategory(idx)` RPC.
pub fn set_emoji_category_via_dbus(idx: u32) {
    unim_log!("INDICATOR", "[EmojiPopup] SetEmojiCategory 호출: idx={}", idx);
    call_popup_service("SetEmojiCategory", (idx,));
}
