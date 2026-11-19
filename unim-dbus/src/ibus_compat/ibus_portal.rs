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

use super::ibus_context::IBusInputContextHandler;
use crate::context_registry::{self, ContextKind, ContextRegistry};
use crate::service::EngineRequest;

/// IBus 포털 컨텍스트 ID 시작값
const IBUS_PORTAL_ID_BASE: u32 = 2_000_000;

use crate::context_registry::IBUS_IC_PATH_PREFIX as IBUS_PORTAL_IC_PATH_PREFIX;

/// IBus Portal 핸들러
pub struct IBusPortalHandler {
    next_context_id: Arc<AtomicU32>,
    engine_tx: mpsc::Sender<EngineRequest>,
    connection: zbus::Connection,
    contexts: ContextRegistry,
}

impl IBusPortalHandler {
    pub fn new(
        engine_tx: mpsc::Sender<EngineRequest>,
        connection: zbus::Connection,
        contexts: ContextRegistry,
    ) -> Self {
        Self {
            next_context_id: Arc::new(AtomicU32::new(IBUS_PORTAL_ID_BASE)),
            engine_tx,
            connection,
            contexts,
        }
    }
}

#[interface(name = "org.freedesktop.IBus.Portal")]
impl IBusPortalHandler {
    /// Flatpak 앱용 InputContext 생성
    async fn create_input_context(
        &self,
        client_name: &str,
        #[zbus(header)] hdr: zbus::message::Header<'_>,
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
            self.engine_tx.clone(),
            self.connection.clone(),
            self.contexts.clone(),
        );

        self.connection
            .object_server()
            .at(path.clone(), handler)
            .await
            .map_err(|e| zbus::fdo::Error::Failed(format!("Registration error: {}", e)))?;

        // 호출자 연결이 Destroy 없이 죽어도 회수되도록 소유자를 기록한다.
        // (IBus 호환 레이어는 같은 세션 버스 연결을 쓰므로 sender 가 있다.)
        context_registry::track_context(
            &self.connection,
            &self.engine_tx,
            &self.contexts,
            None,
            hdr.sender().map(|s| s.as_str()),
            ContextKind::IBus,
            id,
        )
        .await;

        unim_log!(
            "DAEMON",
            "[IBus Compat] Portal InputContext 생성: id={}, client='{}'",
            id,
            client_name
        );

        Ok(path.into())
    }
}
