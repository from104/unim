//! DBus 통신 클라이언트
//!
//! 엔진 데몬과의 DBus 시그널 구독 및 메서드 호출.
//! 툴킷에 무관한 코드로, 향후 `unim-gui-common`으로 추출될 대상입니다.

use std::sync::mpsc::Sender;
use std::sync::{Arc, RwLock};
use std::time::Duration;

use unim::status::InputCategory;
use unim::unim_log;
use unim_dbus::client::InputMethodProxy;

use crate::types::{GuiAction, IndicatorState, UNIM_BUS_NAME};

/// DBus GlobalModeChanged 시그널 구독하여 트레이 업데이트 (비동기)
/// NameOwnerChanged 시그널을 감시하여 데몬 시작/종료 시 자동 재연결
pub async fn watch_dbus_signals(
    state: Arc<RwLock<IndicatorState>>,
    tray_update_tx: std::sync::mpsc::Sender<()>,
    popup_tx: Sender<GuiAction>,
) {
    let _ = &popup_tx; // popup_tx는 UpdateCategory 전달용으로 유지
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
        let has_owner = match dbus_proxy
            .name_has_owner(UNIM_BUS_NAME.try_into().unwrap())
            .await
        {
            Ok(has) => has,
            Err(_) => false,
        };

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

    // 팝업 시그널은 각 프론트엔드가 직접 처리 (unim-gui는 더 이상 팝업을 표시하지 않음)

    while let Some(signal) = mode_stream.next().await {
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

    unim_log!(
        "INDICATOR",
        "[DBus] GlobalModeChanged 스트림 종료 (서비스 종료?)"
    );
}
