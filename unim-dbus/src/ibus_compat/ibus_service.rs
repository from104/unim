//! `org.freedesktop.IBus` 메인 서비스 구현
//!
//! IBus 클라이언트(`im-ibus.so`)가 호출하는 메인 인터페이스.
//! `CreateInputContext`로 입력 컨텍스트를 생성한다.

use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;
use tokio::sync::mpsc;
use zbus::interface;
use zbus::zvariant::{ObjectPath, OwnedObjectPath, Value};

use unim::unim_log;

use super::ibus_context::IBusInputContextHandler;
use crate::context_registry::{self, ContextKind, ContextRegistry};
use super::ibus_types;
use crate::service::EngineRequest;

/// IBus 컨텍스트 ID 시작값 (기존 UNIM 컨텍스트와 충돌 방지)
const IBUS_CONTEXT_ID_BASE: u32 = 1_000_000;

use crate::context_registry::IBUS_IC_PATH_PREFIX;

/// IBus 메인 서비스 핸들러
pub struct IBusServiceHandler {
    next_context_id: Arc<AtomicU32>,
    engine_tx: mpsc::Sender<EngineRequest>,
    connection: zbus::Connection,
    contexts: ContextRegistry,
}

impl IBusServiceHandler {
    pub fn new(
        engine_tx: mpsc::Sender<EngineRequest>,
        connection: zbus::Connection,
        contexts: ContextRegistry,
    ) -> Self {
        Self {
            next_context_id: Arc::new(AtomicU32::new(IBUS_CONTEXT_ID_BASE)),
            engine_tx,
            connection,
            contexts,
        }
    }
}

#[interface(name = "org.freedesktop.IBus")]
impl IBusServiceHandler {
    /// InputContext 생성
    async fn create_input_context(
        &self,
        client_name: &str,
        #[zbus(header)] hdr: zbus::message::Header<'_>,
    ) -> zbus::fdo::Result<OwnedObjectPath> {
        let id = self.next_context_id.fetch_add(1, Ordering::SeqCst);
        let path_str = format!("{}{}", IBUS_IC_PATH_PREFIX, id);
        let path = ObjectPath::try_from(path_str.as_str())
            .map_err(|e| zbus::fdo::Error::Failed(format!("Invalid path: {}", e)))?;

        // 엔진에 컨텍스트 생성
        let (response_tx, response_rx) = tokio::sync::oneshot::channel();
        self.engine_tx
            .send(EngineRequest::CreateContext {
                id,
                window_id: format!("ibus-{}-{}", client_name, id),
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
            "[IBus Compat] InputContext 생성: id={}, client='{}'",
            id,
            client_name
        );

        Ok(path.into())
    }

    async fn list_engines(&self) -> Vec<Value<'_>> {
        vec![ibus_types::serialize_engine_desc()]
    }

    async fn get_global_engine(&self) -> Value<'_> {
        ibus_types::serialize_engine_desc()
    }

    async fn set_global_engine(&self, _engine_name: &str) -> zbus::fdo::Result<()> {
        Ok(())
    }

    async fn get_use_sys_layout(&self) -> bool {
        true
    }

    async fn is_global_engine_enabled(&self) -> bool {
        true
    }

    async fn exit(&self, _restart: bool) -> zbus::fdo::Result<()> {
        Ok(())
    }
}
