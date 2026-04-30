//! IBus 프로토콜 호환 레이어
//!
//! ibus-daemon의 DBus 인터페이스를 에뮬레이션하여
//! Flatpak/Snap 샌드박스 앱의 `im-ibus.so`가 UNIM을 IBus로 인식하게 한다.
//!
//! ## 구성
//!
//! - `ibus_types`: IBusText, IBusAttrList 등 GVariant 직렬화
//! - `address`: IBus 주소 파일 관리
//! - `ibus_service`: `org.freedesktop.IBus` 메인 서비스
//! - `ibus_context`: `org.freedesktop.IBus.InputContext` 구현
//! - `ibus_portal`: `org.freedesktop.portal.IBus` 포털 서비스

pub mod address;
pub mod ibus_context;
pub mod ibus_portal;
pub mod ibus_service;
pub mod ibus_types;

use tokio::sync::mpsc;
use unim::unim_log;

use crate::service::EngineRequest;
use ibus_portal::IBusPortalHandler;
use ibus_service::IBusServiceHandler;

/// IBus 호환 서비스명
const IBUS_BUS_NAME: &str = "org.freedesktop.IBus";
/// IBus Portal 서비스명
const IBUS_PORTAL_BUS_NAME: &str = "org.freedesktop.portal.IBus";

/// IBus 호환 레이어 시작
///
/// Session bus에 IBus 서비스와 Portal을 등록하고,
/// IBus 주소 파일을 생성하여 `im-ibus.so`가 연결할 수 있게 한다.
pub async fn start_ibus_compat(
    engine_tx: mpsc::Sender<EngineRequest>,
    connection: &zbus::Connection,
) -> Result<(), Box<dyn std::error::Error>> {
    // 1. IBus 메인 서비스 등록 (org.freedesktop.IBus)
    let ibus_handler = IBusServiceHandler::new(engine_tx.clone(), connection.clone());
    connection
        .object_server()
        .at("/org/freedesktop/IBus", ibus_handler)
        .await?;

    // IBus 서비스명 등록 (이미 사용 중이면 대기열에 등록)
    match connection.request_name(IBUS_BUS_NAME).await {
        Ok(_) => {
            unim_log!(
                "DAEMON",
                "[IBus Compat] '{}' 서비스명 등록 성공",
                IBUS_BUS_NAME
            );
        }
        Err(e) => {
            unim_log!(
                "DAEMON",
                "[IBus Compat] '{}' 서비스명 등록 실패 (ibus-daemon 실행 중?): {}",
                IBUS_BUS_NAME,
                e
            );
            return Err(Box::new(e));
        }
    }

    // 2. IBus Portal 서비스 등록 (org.freedesktop.portal.IBus)
    let portal_handler = IBusPortalHandler::new(engine_tx, connection.clone());
    connection
        .object_server()
        .at("/org/freedesktop/IBus/Portal", portal_handler)
        .await?;

    match connection.request_name(IBUS_PORTAL_BUS_NAME).await {
        Ok(_) => {
            unim_log!(
                "DAEMON",
                "[IBus Compat] '{}' Portal 서비스명 등록 성공",
                IBUS_PORTAL_BUS_NAME
            );
        }
        Err(e) => {
            unim_log!("DAEMON", "[IBus Compat] Portal 서비스명 등록 실패: {}", e);
            // Portal 실패는 치명적이지 않음 (네이티브 앱은 주소 파일로 연결)
        }
    }

    // 3. IBus 주소 파일 생성 (session bus 주소를 기록)
    match address::write_address_file() {
        Ok(path) => {
            unim_log!(
                "DAEMON",
                "[IBus Compat] 주소 파일 생성 완료: {}",
                path.display()
            );
        }
        Err(e) => {
            unim_log!(
                "DAEMON",
                "[IBus Compat] 주소 파일 생성 실패 (비치명적): {}",
                e
            );
        }
    }

    unim_log!("DAEMON", "[IBus Compat] IBus 호환 레이어 시작 완료");
    Ok(())
}

/// IBus 호환 레이어 정리 (데몬 종료 시)
pub fn stop_ibus_compat() {
    address::remove_address_file();
    unim_log!("DAEMON", "[IBus Compat] IBus 호환 레이어 정리 완료");
}
