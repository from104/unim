//! toolkit-free DBus 헬퍼 — popup 액션 전달
//!
//! hanja/special/emoji popup 에서 공통으로 쓰는 DBus fire-and-forget 함수들.
//! GTK·Qt 양쪽에서 재사용 가능. 모든 함수는 OS 스레드 + 임시 tokio 런타임으로
//! 메인 스레드를 블록하지 않는다.

use unim::unim_log;

use crate::types::{ACTIVE_CONTEXT_PATH, SETTINGS_TX};

// 특수문자 그리드는 column-major 9×9 — index 계산에 사용
const SPECIAL_MAX_ROWS: usize = 9;

// ─────────────────────────────────────────────────────────────
// 공통 헬퍼 매크로
// ─────────────────────────────────────────────────────────────

/// 현재 ACTIVE_CONTEXT_PATH 복사본 반환. 없으면 None.
fn get_context_path() -> Option<String> {
    ACTIVE_CONTEXT_PATH.lock().ok().and_then(|p| p.clone())
}

// ─────────────────────────────────────────────────────────────
// 한자 북마크 상태 비동기 조회
// ─────────────────────────────────────────────────────────────

/// 엔진에서 현재 한자 후보의 북마크 상태를 비동기로 받아 GUI 이벤트 루프에
/// `HanjaBookmarkStatesFetched` 액션을 발행하는 헬퍼.
///
/// 초기 별 상태를 표시하기 위해 사용된다 (GNOME extension.js:176 패턴).
pub fn fetch_bookmark_states_async(context_path: String) {
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
            let proxy = match zbus::Proxy::new(
                &conn,
                "org.atit.unim.InputMethod",
                context_path.as_str(),
                "org.atit.unim.InputContext",
            )
            .await
            {
                Ok(p) => p,
                Err(_) => return,
            };
            let states: Result<Vec<bool>, _> = proxy.call("GetHanjaBookmarkStates", &()).await;
            if let Ok(states) = states {
                let _ = tx.send(crate::types::GuiAction::HanjaBookmarkStatesFetched { states });
            }
        });
    });
}

// ─────────────────────────────────────────────────────────────
// popup 페이지·확장·북마크
// ─────────────────────────────────────────────────────────────

/// DBus를 통해 popup 페이지 이동 (마우스 ◀/▶ 클릭).
///
/// `direction`: 0 (or negative) = 이전 페이지, 1 (or positive) = 다음 페이지.
/// 한자/특수문자/이모지 popup 모두에 동작 — 데몬 popup_state 가 단일 진실 소스.
pub fn popup_change_page_via_dbus(direction: i32) {
    let Some(path) = get_context_path() else { return };
    unim_log!(
        "INDICATOR",
        "[Popup] PopupChangePage DBus 호출: dir={}, path={}",
        direction,
        path
    );
    std::thread::spawn(move || {
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        rt.block_on(async {
            if let Ok(conn) = zbus::Connection::session().await {
                let proxy = zbus::Proxy::new(
                    &conn,
                    "org.atit.unim.InputMethod",
                    path.as_str(),
                    "org.atit.unim.InputContext",
                )
                .await;
                if let Ok(proxy) = proxy {
                    let _: Result<(), _> = proxy.call("PopupChangePage", &(direction,)).await;
                }
            }
        });
    });
}

/// 한자 popup 확장 모드 토글 (마우스 ⊞/⊟ 아이콘 클릭).
///
/// 호출 context (gui-gtk 자체) 와 popup-owner context 가 다를 수 있어 데몬이
/// resolve_popup_owner 로 라우팅 보정 — 활성 popup 의 toggle_hanja_expanded()
/// 적용 후 PopupNavigate 시그널 발행.
pub fn toggle_popup_expand_via_dbus() {
    let Some(path) = get_context_path() else { return };
    unim_log!(
        "INDICATOR",
        "[Popup] TogglePopupExpand DBus 호출: path={}",
        path
    );
    std::thread::spawn(move || {
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        rt.block_on(async {
            if let Ok(conn) = zbus::Connection::session().await {
                let proxy = zbus::Proxy::new(
                    &conn,
                    "org.atit.unim.InputMethod",
                    path.as_str(),
                    "org.atit.unim.InputContext",
                )
                .await;
                if let Ok(proxy) = proxy {
                    let _: Result<(), _> = proxy.call("TogglePopupExpand", &()).await;
                }
            }
        });
    });
}

/// 한자 후보 즐겨찾기 토글 (마우스 우클릭).
///
/// `global_index` 는 전체 후보 인덱스 (페이지 로컬이 아님). 데몬이
/// resolve_popup_owner 로 popup-owner context 에 라우팅 + ToggleHanjaBookmark
/// 처리 후 HanjaBookmarkChanged + HanjaCandidatesReordered 시그널 발행.
pub fn toggle_hanja_bookmark_via_dbus(global_index: u32) {
    let Some(path) = get_context_path() else { return };
    unim_log!(
        "INDICATOR",
        "[Popup] ToggleHanjaBookmark DBus 호출: idx={}, path={}",
        global_index,
        path
    );
    std::thread::spawn(move || {
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        rt.block_on(async {
            if let Ok(conn) = zbus::Connection::session().await {
                let proxy = zbus::Proxy::new(
                    &conn,
                    "org.atit.unim.InputMethod",
                    path.as_str(),
                    "org.atit.unim.InputContext",
                )
                .await;
                if let Ok(proxy) = proxy {
                    let _: Result<(u32, bool), _> =
                        proxy.call("ToggleHanjaBookmark", &(global_index,)).await;
                }
            }
        });
    });
}

// ─────────────────────────────────────────────────────────────
// 한자 선택 / 취소
// ─────────────────────────────────────────────────────────────

/// DBus를 통해 한자 선택 (page_local_index).
pub fn select_hanja_via_dbus(page_local_index: u32) {
    let Some(path) = get_context_path() else { return };
    unim_log!(
        "INDICATOR",
        "[Popup] 한자 선택 DBus 호출: index={}, path={}",
        page_local_index,
        path
    );
    std::thread::spawn(move || {
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        rt.block_on(async {
            if let Ok(conn) = zbus::Connection::session().await {
                let proxy = zbus::Proxy::new(
                    &conn,
                    "org.atit.unim.InputMethod",
                    path.as_str(),
                    "org.atit.unim.InputContext",
                )
                .await;
                if let Ok(proxy) = proxy {
                    let _: Result<String, _> =
                        proxy.call("SelectHanja", &(page_local_index,)).await;
                }
            }
        });
    });
}

/// RC2: 팝업이 숨겨질 때 엔진 한자 상태 정리.
///
/// `window.connect_hide` 콜백에서 호출된다. 정상 선택/취소 경로에서는 no-op.
/// outside-click, 프로그램 종료 등 비정상 닫힘 후 엔진이 한자 대기 상태에
/// 걸리지 않도록 보장한다.
pub fn cancel_hanja_via_dbus() {
    let Some(path) = get_context_path() else { return };
    unim_log!(
        "INDICATOR",
        "[Popup] CancelHanja (hide hook) DBus 호출: path={}",
        path
    );
    std::thread::spawn(move || {
        let rt = match tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
        {
            Ok(r) => r,
            Err(_) => return,
        };
        rt.block_on(async {
            if let Ok(conn) = zbus::Connection::session().await {
                if let Ok(proxy) = zbus::Proxy::new(
                    &conn,
                    "org.atit.unim.InputMethod",
                    path.as_str(),
                    "org.atit.unim.InputContext",
                )
                .await
                {
                    let _: Result<String, _> = proxy.call("CancelHanja", &()).await;
                }
            }
        });
    });
}

// ─────────────────────────────────────────────────────────────
// 특수문자 선택 / 취소
// ─────────────────────────────────────────────────────────────

/// DBus를 통해 특수문자 선택 (column-major index).
///
/// `col`, `row`: 0-based. 인덱스 = `col * SPECIAL_MAX_ROWS + row`.
pub fn select_special_via_dbus(col: usize, row: usize) {
    let index = (col * SPECIAL_MAX_ROWS + row) as u32;
    let Some(path) = get_context_path() else { return };
    unim_log!(
        "INDICATOR",
        "[Popup] 특수문자 선택 DBus 호출: index={} (col={}, row={}), path={}",
        index,
        col,
        row,
        path
    );
    std::thread::spawn(move || {
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        rt.block_on(async {
            if let Ok(conn) = zbus::Connection::session().await {
                let proxy = zbus::Proxy::new(
                    &conn,
                    "org.atit.unim.InputMethod",
                    path.as_str(),
                    "org.atit.unim.InputContext",
                )
                .await;
                if let Ok(proxy) = proxy {
                    let _: Result<String, _> = proxy.call("SelectSpecialChar", &(index,)).await;
                }
            }
        });
    });
}

/// RC2: 팝업이 숨겨질 때 엔진 특수문자 상태 정리.
pub fn cancel_special_via_dbus() {
    let Some(path) = get_context_path() else { return };
    unim_log!(
        "INDICATOR",
        "[Popup] CancelSpecialChar (hide hook) DBus 호출: path={}",
        path
    );
    std::thread::spawn(move || {
        let rt = match tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
        {
            Ok(r) => r,
            Err(_) => return,
        };
        rt.block_on(async {
            if let Ok(conn) = zbus::Connection::session().await {
                if let Ok(proxy) = zbus::Proxy::new(
                    &conn,
                    "org.atit.unim.InputMethod",
                    path.as_str(),
                    "org.atit.unim.InputContext",
                )
                .await
                {
                    let _: Result<String, _> =
                        proxy.call("CancelSpecialChar", &()).await;
                }
            }
        });
    });
}

// ─────────────────────────────────────────────────────────────
// 이모지 커밋 / 카테고리
// ─────────────────────────────────────────────────────────────

/// 셀 클릭 콜백에서 호출 — 라벨 텍스트(emoji)를 그대로 받아 DBus `CommitEmoji` RPC.
pub fn commit_emoji_via_dbus(emoji_str: String) {
    if emoji_str.is_empty() {
        return;
    }
    let context_path = ACTIVE_CONTEXT_PATH.lock().ok().and_then(|p| p.clone());
    if let Some(path) = context_path {
        unim_log!(
            "INDICATOR",
            "[EmojiPopup] CommitEmoji DBus 호출: emoji='{}', path={}",
            emoji_str,
            path
        );
        std::thread::spawn(move || {
            let rt = match tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
            {
                Ok(r) => r,
                Err(_) => return,
            };
            rt.block_on(async {
                if let Ok(conn) = zbus::Connection::session().await {
                    let proxy = zbus::Proxy::new(
                        &conn,
                        "org.atit.unim.InputMethod",
                        path.as_str(),
                        "org.atit.unim.InputContext",
                    )
                    .await;
                    if let Ok(proxy) = proxy {
                        let _: Result<(), _> =
                            proxy.call("CommitEmoji", &(emoji_str.as_str(),)).await;
                    }
                }
            });
        });
    } else {
        unim_log!(
            "INDICATOR",
            "[EmojiPopup] CommitEmoji 실패: ACTIVE_CONTEXT_PATH가 비어있음"
        );
    }
}

/// 탭 클릭 콜백에서 호출 — daemon `SetEmojiCategory(idx)` RPC.
///
/// `SetEmojiCategory`는 `org.atit.unim.InputMethod` 인터페이스(글로벌
/// `/org/atit/unim/InputMethod` path)에 정의되어 있다.
pub fn set_emoji_category_via_dbus(idx: u32) {
    unim_log!(
        "INDICATOR",
        "[EmojiPopup] SetEmojiCategory DBus 호출: idx={}",
        idx
    );
    std::thread::spawn(move || {
        let rt = match tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
        {
            Ok(r) => r,
            Err(_) => return,
        };
        rt.block_on(async {
            let conn = match zbus::Connection::session().await {
                Ok(c) => c,
                Err(e) => {
                    unim_log!(
                        "INDICATOR",
                        "[EmojiPopup] SetEmojiCategory 세션 버스 연결 실패: {}",
                        e
                    );
                    return;
                }
            };
            let proxy = match zbus::Proxy::new(
                &conn,
                "org.atit.unim.InputMethod",
                "/org/atit/unim/InputMethod",
                "org.atit.unim.InputMethod",
            )
            .await
            {
                Ok(p) => p,
                Err(e) => {
                    unim_log!(
                        "INDICATOR",
                        "[EmojiPopup] SetEmojiCategory proxy 생성 실패: {}",
                        e
                    );
                    return;
                }
            };
            if let Err(e) = proxy
                .call::<_, _, ()>("SetEmojiCategory", &(idx,))
                .await
            {
                unim_log!(
                    "INDICATOR",
                    "[EmojiPopup] SetEmojiCategory RPC 실패: idx={}, err={}",
                    idx,
                    e
                );
            }
        });
    });
}
