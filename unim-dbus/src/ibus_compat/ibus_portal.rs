//! `org.freedesktop.IBus.Portal` 구현
//!
//! Flatpak 앱이 session bus를 통해 IBus에 접근하는 포털.
//! `CreateInputContext`를 메인 IBus 서비스에 위임한다.

use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;
use tokio::sync::mpsc;
use zbus::interface;
use zbus::zvariant::{ObjectPath, OwnedObjectPath};

use unim::unim_log;

use crate::service::EngineRequest;
use super::ibus_context::IBusInputContextHandler;

/// IBus 포털 컨텍스트 ID 시작값
const IBUS_PORTAL_ID_BASE: u32 = 2_000_000;

/// IBus InputContext 객체 경로 접두사 (Portal용)
const IBUS_PORTAL_IC_PATH_PREFIX: &str = "/org/freedesktop/IBus/InputContext_";

/// IBus Portal 핸들러
pub struct IBusPortalHandler {
    next_context_id: Arc<AtomicU32>,
    engine_tx: mpsc::Sender<EngineRequest>,
    connection: zbus::Connection,
}

impl IBusPortalHandler {
    pub fn new(
        engine_tx: mpsc::Sender<EngineRequest>,
        connection: zbus::Connection,
    ) -> Self {
        Self {
            next_context_id: Arc::new(AtomicU32::new(IBUS_PORTAL_ID_BASE)),
            engine_tx,
            connection,
        }
    }
}

#[interface(name = "org.freedesktop.IBus.Portal")]
impl IBusPortalHandler {
    /// Flatpak 앱용 InputContext 생성
    async fn create_input_context(
        &self,
        client_name: &str,
    ) -> zbus::fdo::Result<OwnedObjectPath> {
        let id = self.next_context_id.fetch_add(1, Ordering::SeqCst);
        let path_str = format!("{}{}", IBUS_PORTAL_IC_PATH_PREFIX, id);
        let path = ObjectPath::try_from(path_str.as_str())
            .map_err(|e| zbus::fdo::Error::Failed(format!("Invalid path: {}", e)))?;

        // 엔진에 컨텍스트 생성
        let (response_tx, response_rx) = tokio::sync::oneshot::channel();
        self.engine_tx
            .send(EngineRequest::CreateContext {
                id,
                window_id: format!("ibus-portal-{}-{}", client_name, id),
                response: response_tx,
            })
            .await
            .map_err(|e| zbus::fdo::Error::Failed(format!("Engine error: {}", e)))?;

        response_rx
            .await
            .map_err(|e| zbus::fdo::Error::Failed(format!("Response error: {}", e)))?;

        // IBus InputContext 핸들러 등록
        let handler = IBusInputContextHandler::new(
            id,
            &path_str,
            self.engine_tx.clone(),
            self.connection.clone(),
        );

        self.connection
            .object_server()
            .at(path.clone(), handler)
            .await
            .map_err(|e| zbus::fdo::Error::Failed(format!("Registration error: {}", e)))?;

        unim_log!(
            "DAEMON",
            "[IBus Compat] Portal InputContext 생성: id={}, client='{}'",
            id,
            client_name
        );

        Ok(path.into())
    }
}
