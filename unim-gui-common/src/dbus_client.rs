//! DBus 통신 클라이언트
//!
//! 엔진 데몬과의 DBus 시그널 구독 및 메서드 호출.
//! GlobalModeChanged + InputContext 팝업 시그널을 모두 구독합니다.

use std::sync::mpsc::Sender;
use std::sync::{Arc, RwLock};
use std::time::Duration;

use unim::status::InputCategory;
use unim::unim_log;
use unim_dbus::client::InputMethodProxy;

use crate::types::{GuiAction, IndicatorState, ACTIVE_CONTEXT_PATH, UNIM_BUS_NAME};

/// DBus GlobalModeChanged 시그널 구독하여 트레이 업데이트 (비동기)
/// NameOwnerChanged 시그널을 감시하여 데몬 시작/종료 시 자동 재연결
pub async fn watch_dbus_signals(
    state: Arc<RwLock<IndicatorState>>,
    tray_update_tx: std::sync::mpsc::Sender<()>,
    popup_tx: Sender<GuiAction>,
) {
    use futures_util::StreamExt;

    loop {
        // DBus 연결
        let connection = match zbus::Connection::session().await {
            Ok(conn) => conn,
            Err(e) => {
                unim_log!("INDICATOR", "DBus 세션 연결 실패: {}", e);
                tokio::time::sleep(Duration::from_secs(5)).await;
                continue;
            }
        };

        unim_log!("INDICATOR", "[DBus] 세션 버스 연결됨, 서비스 감시 시작...");

        // org.freedesktop.DBus 프록시 (NameOwnerChanged 감시용)
        let dbus_proxy = match zbus::fdo::DBusProxy::new(&connection).await {
            Ok(p) => p,
            Err(e) => {
                unim_log!("INDICATOR", "DBusProxy 생성 실패: {}", e);
                tokio::time::sleep(Duration::from_secs(5)).await;
                continue;
            }
        };

        // 현재 서비스 소유자 확인
        let has_owner = dbus_proxy
            .name_has_owner(UNIM_BUS_NAME.try_into().unwrap())
            .await
            .unwrap_or_default();

        if has_owner {
            unim_log!(
                "INDICATOR",
                "[DBus] {} 서비스 발견, 연결 시도...",
                UNIM_BUS_NAME
            );
            // 서비스가 있으면 즉시 연결 시도
            watch_mode_signals(
                &connection,
                state.clone(),
                tray_update_tx.clone(),
                popup_tx.clone(),
            )
            .await;
        } else {
            unim_log!(
                "INDICATOR",
                "[DBus] {} 서비스 없음, DBus Activation 시도...",
                UNIM_BUS_NAME
            );

            // DBus Activation: StartServiceByName 호출로 데몬 자동 시작
            match dbus_proxy
                .start_service_by_name(UNIM_BUS_NAME.try_into().unwrap(), 0)
                .await
            {
                Ok(_) => {
                    unim_log!("INDICATOR", "[DBus] {} 서비스 활성화 성공", UNIM_BUS_NAME);
                    // 활성화 후 잠시 대기 후 연결
                    tokio::time::sleep(Duration::from_millis(500)).await;
                    watch_mode_signals(
                        &connection,
                        state.clone(),
                        tray_update_tx.clone(),
                        popup_tx.clone(),
                    )
                    .await;
                }
                Err(e) => {
                    unim_log!(
                        "INDICATOR",
                        "[DBus] {} 서비스 활성화 실패: {}, 대기 중...",
                        UNIM_BUS_NAME,
                        e
                    );
                }
            }
        }

        // NameOwnerChanged 시그널 구독
        let mut stream = match dbus_proxy.receive_name_owner_changed().await {
            Ok(s) => s,
            Err(e) => {
                unim_log!("INDICATOR", "NameOwnerChanged 구독 실패: {}", e);
                tokio::time::sleep(Duration::from_secs(5)).await;
                continue;
            }
        };

        // 서비스 소유자 변경 감시
        while let Some(signal) = stream.next().await {
            if let Ok(args) = signal.args() {
                let name = args.name.as_str();
                if name != UNIM_BUS_NAME {
                    continue;
                }

                let old_owner = args.old_owner.as_ref().map(|s| s.as_str()).unwrap_or("");
                let new_owner = args.new_owner.as_ref().map(|s| s.as_str()).unwrap_or("");

                if old_owner.is_empty() && !new_owner.is_empty() {
                    // 서비스 등장
                    unim_log!(
                        "INDICATOR",
                        "[DBus] {} 서비스 등장 (owner: {})",
                        UNIM_BUS_NAME,
                        new_owner
                    );

                    // 모드 시그널 감시 시작 (서비스 종료될 때까지)
                    watch_mode_signals(
                        &connection,
                        state.clone(),
                        tray_update_tx.clone(),
                        popup_tx.clone(),
                    )
                    .await;
                } else if !old_owner.is_empty() && new_owner.is_empty() {
                    // 서비스 소멸
                    unim_log!("INDICATOR", "[DBus] {} 서비스 소멸", UNIM_BUS_NAME);

                    // 연결 안됨 상태로 표시 (기본값 English로 리셋하지 않음)
                    let _ = tray_update_tx.send(());
                }
            }
        }

        // 스트림 종료 시 재연결 시도
        unim_log!(
            "INDICATOR",
            "[DBus] NameOwnerChanged 스트림 종료, 재연결 시도..."
        );
        tokio::time::sleep(Duration::from_secs(2)).await;
    }
}

/// GlobalModeChanged + 팝업 시그널 감시 (서비스가 연결된 상태에서 호출)
async fn watch_mode_signals(
    connection: &zbus::Connection,
    state: Arc<RwLock<IndicatorState>>,
    tray_update_tx: std::sync::mpsc::Sender<()>,
    popup_tx: Sender<GuiAction>,
) {
    use futures_util::StreamExt;

    // InputMethod 프록시 생성
    let proxy = match InputMethodProxy::new(connection).await {
        Ok(p) => p,
        Err(e) => {
            unim_log!("INDICATOR", "InputMethod 프록시 생성 실패: {}", e);
            return;
        }
    };

    // 초기 모드 조회
    match proxy.get_global_mode().await {
        Ok(is_korean) => {
            let category = if is_korean {
                InputCategory::Korean
            } else {
                InputCategory::English
            };
            if let Ok(mut s) = state.write() {
                s.category = category;
            }
            let _ = tray_update_tx.send(());
            let _ = popup_tx.send(GuiAction::UpdateCategory(category));
            unim_log!("INDICATOR", "[DBus] 초기 모드 조회: {:?}", category);
        }
        Err(e) => {
            unim_log!("INDICATOR", "초기 모드 조회 실패: {}", e);
        }
    }

    // GlobalModeChanged 시그널 구독
    unim_log!("INDICATOR", "[DBus] GlobalModeChanged 시그널 구독 시작...");

    let mut mode_stream = match proxy.receive_global_mode_changed().await {
        Ok(s) => s,
        Err(e) => {
            unim_log!("INDICATOR", "시그널 구독 실패: {}", e);
            return;
        }
    };

    // InputContext 팝업 시그널 구독 (path_namespace로 모든 컨텍스트 시그널 수신)
    let popup_rule = zbus::MatchRule::builder()
        .msg_type(zbus::message::Type::Signal)
        .interface("org.atit.unim.InputContext")
        .unwrap()
        .path_namespace("/org/atit/unim")
        .unwrap()
        .build();

    let mut popup_stream =
        match zbus::MessageStream::for_match_rule(popup_rule, connection, Some(64)).await {
            Ok(s) => s,
            Err(e) => {
                unim_log!("INDICATOR", "팝업 시그널 구독 실패: {}", e);
                return;
            }
        };

    unim_log!(
        "INDICATOR",
        "[DBus] InputContext 팝업 시그널 구독 시작 (path_namespace)"
    );

    // 두 스트림을 동시에 감시
    loop {
        tokio::select! {
            Some(signal) = mode_stream.next() => {
                handle_mode_signal(signal, &state, &tray_update_tx, &popup_tx);
            }
            Some(Ok(msg)) = popup_stream.next() => {
                handle_popup_signal(&msg, &popup_tx);
            }
            else => break,
        }
    }

    unim_log!("INDICATOR", "[DBus] 시그널 스트림 종료 (서비스 종료?)");
}

/// GlobalModeChanged 시그널 처리
fn handle_mode_signal(
    signal: unim_dbus::client::GlobalModeChanged,
    state: &Arc<RwLock<IndicatorState>>,
    tray_update_tx: &std::sync::mpsc::Sender<()>,
    popup_tx: &Sender<GuiAction>,
) {
    match signal.args() {
        Ok(args) => {
            let is_korean = args.is_korean;
            let category = if is_korean {
                InputCategory::Korean
            } else {
                InputCategory::English
            };

            let should_update = {
                if let Ok(s) = state.read() {
                    s.category != category
                } else {
                    true
                }
            };

            if should_update {
                if let Ok(mut s) = state.write() {
                    s.category = category;
                }
                unim_log!("INDICATOR", "[DBus] 모드 변경 감지: {:?}", category);
                let _ = tray_update_tx.send(());
                let _ = popup_tx.send(GuiAction::UpdateCategory(category));
            }
        }
        Err(e) => {
            unim_log!("INDICATOR", "시그널 인자 파싱 오류: {}", e);
        }
    }
}

/// InputContext 팝업 시그널 처리
fn handle_popup_signal(msg: &zbus::Message, popup_tx: &Sender<GuiAction>) {
    let header = msg.header();
    let member = match header.member() {
        Some(m) => m.to_string(),
        None => return,
    };
    let context_path = header.path().map(|p| p.to_string()).unwrap_or_default();

    match member.as_str() {
        "ShowHanjaPopup" => {
            // 7-tuple: top_row가 3번째에 추가됨 (특수문자와 동일 시그니처).
            if let Ok((target, candidates, top_row, x, y, w, h)) =
                msg.body()
                    .deserialize::<(String, Vec<(String, String)>, String, i32, i32, i32, i32)>()
            {
                // 활성 컨텍스트 경로 저장
                if let Ok(mut path) = ACTIVE_CONTEXT_PATH.lock() {
                    *path = Some(context_path.clone());
                }
                unim_log!(
                    "INDICATOR",
                    "[Popup] ShowHanjaPopup 수신: target='{}', count={}, top_row='{}', pos=({},{})",
                    target,
                    candidates.len(),
                    top_row,
                    x,
                    y
                );
                let _ = popup_tx.send(GuiAction::ShowHanjaPopup {
                    context_path,
                    target,
                    candidates,
                    top_row,
                    x,
                    y,
                    w,
                    h,
                });
            }
        }
        "ShowSpecialPopup" => {
            if let Ok((target, characters, top_row, x, y, w, h)) =
                msg.body()
                    .deserialize::<(String, Vec<String>, String, i32, i32, i32, i32)>()
            {
                if let Ok(mut path) = ACTIVE_CONTEXT_PATH.lock() {
                    *path = Some(context_path.clone());
                }
                unim_log!(
                    "INDICATOR",
                    "[Popup] ShowSpecialPopup 수신: target='{}', count={}, pos=({},{})",
                    target,
                    characters.len(),
                    x,
                    y
                );
                let _ = popup_tx.send(GuiAction::ShowSpecialPopup {
                    context_path,
                    target,
                    characters,
                    top_row,
                    x,
                    y,
                    w,
                    h,
                });
            }
        }
        "ShowEmojiPopupV2" => {
            // GNOME 환경에선 GNOME extension 의 emoji_popup 이 (앞으로 PR #3 에서)
            // 같은 시그널을 직접 받아 자체 St 위젯으로 표시한다. standalone GUI 도
            // 같은 시그널을 받아 GTK4 popup 을 띄우면 두 popup 이 동시에 떠 키보드
            // 포커스 race 가 난다 — GNOME 에서는 standalone 측 표시를 skip 한다.
            let is_gnome = std::env::var("XDG_CURRENT_DESKTOP")
                .ok()
                .map(|v| v.to_ascii_uppercase().contains("GNOME"))
                .unwrap_or(false);
            if is_gnome {
                unim_log!(
                    "INDICATOR",
                    "[Popup] ShowEmojiPopupV2 skip (GNOME 환경 — extension 처리)"
                );
            } else if let Ok((
                target_cat_id,
                items,
                top_row,
                recent,
                categories,
                x,
                y,
                w,
                h,
            )) = msg.body().deserialize::<(
                String,
                Vec<String>,
                String,
                Vec<String>,
                Vec<(String, String, String, u32)>,
                i32,
                i32,
                i32,
                i32,
            )>() {
                if let Ok(mut path) = ACTIVE_CONTEXT_PATH.lock() {
                    *path = Some(context_path.clone());
                }
                unim_log!(
                    "INDICATOR",
                    "[Popup] ShowEmojiPopupV2 수신: cat='{}', items={}, recent={}, cats={}, pos=({},{},{},{})",
                    target_cat_id,
                    items.len(),
                    recent.len(),
                    categories.len(),
                    x,
                    y,
                    w,
                    h
                );
                let _ = popup_tx.send(GuiAction::ShowEmojiPopup {
                    context_path,
                    target_cat_id,
                    items,
                    top_row,
                    recent,
                    categories,
                    x,
                    y,
                    w,
                    h,
                });
            }
        }
        "HidePopup" => {
            unim_log!("INDICATOR", "[Popup] HidePopup 수신");
            let _ = popup_tx.send(GuiAction::HidePopup);
        }
        "HanjaBookmarkChanged" => {
            if let Ok((index, bookmarked)) = msg.body().deserialize::<(u32, bool)>() {
                unim_log!(
                    "INDICATOR",
                    "[Popup] HanjaBookmarkChanged 수신: index={}, bookmarked={}",
                    index,
                    bookmarked
                );
                let _ = popup_tx.send(GuiAction::HanjaBookmarkChanged { index, bookmarked });
            }
        }
        "HanjaCandidatesReordered" => {
            if let Ok((
                target,
                hanjas,
                meanings,
                bookmarks,
                new_cursor,
                page,
                sel_row,
                sel_col,
                _bookmarked,
            )) = msg.body().deserialize::<(
                String,
                Vec<String>,
                Vec<String>,
                Vec<bool>,
                u32,
                i32,
                i32,
                i32,
                bool,
            )>() {
                unim_log!(
                    "INDICATOR",
                    "[Popup] HanjaCandidatesReordered 수신: target='{}', count={}, new_cursor={}, page={}",
                    target,
                    hanjas.len(),
                    new_cursor,
                    page
                );
                let pairs: Vec<(String, String)> = hanjas
                    .into_iter()
                    .zip(meanings.into_iter().chain(std::iter::repeat(String::new())))
                    .collect();
                let _ = popup_tx.send(GuiAction::HanjaCandidatesReordered {
                    candidates: pairs,
                    bookmarks,
                    new_cursor,
                    page,
                    sel_row,
                    sel_col,
                });
            }
        }
        "PopupNavigate" => {
            if let Ok((page, total_pages, selected, rows, cols, sel_row, sel_col)) = msg
                .body()
                .deserialize::<(i32, i32, i32, i32, i32, i32, i32)>()
            {
                let _ = popup_tx.send(GuiAction::PopupNavigate {
                    page,
                    total_pages,
                    selected,
                    rows,
                    cols,
                    sel_row,
                    sel_col,
                });
            }
        }
        _ => {}
    }
}
