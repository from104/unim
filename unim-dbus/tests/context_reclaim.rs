//! 연결 종료 시 입력 컨텍스트 회수 p2p 시험 (`context_registry`).
//!
//! p2p 연결에는 버스가 없어 `NameOwnerChanged` 도, 메시지 sender 도 없다. 그래서
//! (1) p2p 로 만든 컨텍스트는 추적 대상이 아님을 확인하고, (2) 소유자를 수동 등록한 뒤
//! 감시 태스크가 부르는 `reclaim_owner` 를 직접 불러 회수·멱등·`Destroy` 경합을 검증한다.
//! 하네스는 `surrounding_word.rs` 와 같다(`ObjectServerReady` — zbus 4.4 등록 경합 회피,
//! HOME·XDG 임시 디렉터리 격리).

use std::sync::OnceLock;
use tokio::net::UnixStream;
use tokio::sync::mpsc;
use unim::config::Config;
use unim_dbus::context_registry::{reclaim_owner, ContextKind, ContextRegistry};
use unim_dbus::engine_worker::spawn_engine_worker;
use unim_dbus::ibus_compat::ibus_context::IBusInputContextHandler;
use unim_dbus::ibus_compat::ibus_service::IBusServiceHandler;
use unim_dbus::service::{EngineRequest, InputContextHandler, InputMethodService};
use unim_dbus::INPUT_METHOD_PATH;
use zbus::connection::Builder;
use zbus::zvariant::OwnedObjectPath;

const IM_IFACE: &str = "org.atit.unim.InputMethod";
const IC_IFACE: &str = "org.atit.unim.InputContext";
const IBUS_IFACE: &str = "org.freedesktop.IBus";
const IBUS_PATH: &str = "/org/freedesktop/IBus";
const READY_PATH: &str = "/org/atit/unim/test/ready";
const OWNER: &str = ":1.4242";
const OTHER: &str = ":1.7";

static HOME: OnceLock<tempfile::TempDir> = OnceLock::new();

struct ObjectServerReady;

#[zbus::interface(name = "org.atit.unim.test.Ready")]
impl ObjectServerReady {}

struct Harness {
    server: zbus::Connection,
    client: zbus::Connection,
    engine_tx: mpsc::Sender<EngineRequest>,
    registry: ContextRegistry,
    last_active: std::sync::Arc<std::sync::Mutex<Option<String>>>,
}

async fn setup() -> zbus::Result<Harness> {
    let tmp = HOME.get_or_init(|| {
        let tmp = tempfile::tempdir().expect("tempdir");
        std::env::set_var("HOME", tmp.path());
        std::env::set_var("XDG_CONFIG_HOME", tmp.path().join("config"));
        std::env::set_var("XDG_DATA_HOME", tmp.path().join("data"));
        tmp
    });
    let _ = tmp;

    let (a, b) = UnixStream::pair().expect("socketpair");
    let guid = zbus::Guid::generate();
    let (server, client) = tokio::try_join!(
        async {
            Builder::unix_stream(a)
                .server(guid)?
                .p2p()
                .serve_at(READY_PATH, ObjectServerReady)?
                .build()
                .await
        },
        async { Builder::unix_stream(b).p2p().build().await },
    )?;

    let (notify_tx, _notify_rx) = mpsc::channel(16);
    let engine_tx = spawn_engine_worker(Config::default(), notify_tx);
    let svc = InputMethodService::new(Config::default(), engine_tx.clone(), server.clone());
    let registry = svc.context_registry();
    let last_active = svc.last_active_input_context_path();
    server.object_server().at(INPUT_METHOD_PATH, svc).await?;
    server
        .object_server()
        .at(
            IBUS_PATH,
            IBusServiceHandler::new(engine_tx.clone(), server.clone(), registry.clone()),
        )
        .await?;
    Ok(Harness {
        server,
        client,
        engine_tx,
        registry,
        last_active,
    })
}

impl Harness {
    async fn create_unim(&self) -> zbus::Result<(String, u32)> {
        let reply = self
            .client
            .call_method(
                None::<&str>,
                INPUT_METHOD_PATH,
                Some(IM_IFACE),
                "CreateInputContext",
                &("test", "w1"),
            )
            .await?;
        let path: String = reply.body().deserialize()?;
        let id = path.rsplit('_').next().unwrap().parse().unwrap();
        Ok((path, id))
    }

    async fn create_ibus(&self) -> zbus::Result<(String, u32)> {
        let reply = self
            .client
            .call_method(
                None::<&str>,
                IBUS_PATH,
                Some(IBUS_IFACE),
                "CreateInputContext",
                &("test",),
            )
            .await?;
        let path: OwnedObjectPath = reply.body().deserialize()?;
        let id = path.as_str().rsplit('_').next().unwrap().parse().unwrap();
        Ok((path.to_string(), id))
    }

    async fn unim_alive(&self, path: &str) -> bool {
        self.server
            .object_server()
            .interface::<_, InputContextHandler>(path)
            .await
            .is_ok()
    }

    async fn ibus_alive(&self, path: &str) -> bool {
        self.server
            .object_server()
            .interface::<_, IBusInputContextHandler>(path)
            .await
            .is_ok()
    }

    async fn reclaim(&self, owner: &str) -> usize {
        reclaim_owner(
            &self.server,
            &self.engine_tx,
            &self.registry,
            Some(&self.last_active),
            owner,
        )
        .await
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn p2p_contexts_are_not_tracked() {
    let h = setup().await.unwrap();
    let (path, _) = h.create_unim().await.unwrap();
    let (ipath, _) = h.create_ibus().await.unwrap();
    assert!(
        h.registry.is_empty(),
        "sender 없는 p2p 컨텍스트는 추적 대상이 아니다"
    );
    assert!(h.unim_alive(&path).await && h.ibus_alive(&ipath).await);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn reclaim_removes_owner_contexts_and_is_idempotent() {
    let h = setup().await.unwrap();
    let (p1, id1) = h.create_unim().await.unwrap();
    let (p2, id2) = h.create_unim().await.unwrap();
    let (ip, iid) = h.create_ibus().await.unwrap();
    let (other_path, other_id) = h.create_unim().await.unwrap();
    h.registry.register(OWNER, id1, ContextKind::Unim);
    h.registry.register(OWNER, id2, ContextKind::Unim);
    h.registry.register(OWNER, iid, ContextKind::IBus);
    h.registry.register(OTHER, other_id, ContextKind::Unim);
    *h.last_active.lock().unwrap() = Some(p2.clone());

    assert_eq!(h.reclaim(OWNER).await, 3);
    assert!(
        !h.unim_alive(&p1).await && !h.unim_alive(&p2).await,
        "UNIM 컨텍스트 제거됨"
    );
    assert!(!h.ibus_alive(&ip).await, "IBus 컨텍스트 제거됨");
    assert!(
        h.unim_alive(&other_path).await,
        "다른 owner 의 컨텍스트는 그대로"
    );
    assert!(h.registry.contains(other_id) && h.registry.len() == 1);
    assert_eq!(
        *h.last_active.lock().unwrap(),
        None,
        "last_active 가 죽은 path 를 가리키지 않는다"
    );

    assert_eq!(h.reclaim(OWNER).await, 0, "두 번째 회수는 no-op");
    assert_eq!(h.reclaim("없는-owner").await, 0);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn explicit_destroy_then_reclaim_does_not_double_destroy() {
    let h = setup().await.unwrap();
    let (p1, id1) = h.create_unim().await.unwrap();
    let (p2, id2) = h.create_unim().await.unwrap();
    let (ip, iid) = h.create_ibus().await.unwrap();
    h.registry.register(OWNER, id1, ContextKind::Unim);
    h.registry.register(OWNER, id2, ContextKind::Unim);
    h.registry.register(OWNER, iid, ContextKind::IBus);

    // 앱이 Destroy 를 부른 컨텍스트는 레지스트리에서 빠진다.
    h.client
        .call_method(None::<&str>, p1.as_str(), Some(IC_IFACE), "Destroy", &())
        .await
        .unwrap();
    h.client
        .call_method(
            None::<&str>,
            ip.as_str(),
            Some("org.freedesktop.IBus.InputContext"),
            "Destroy",
            &(),
        )
        .await
        .unwrap();
    assert!(!h.registry.contains(id1) && !h.registry.contains(iid));
    assert!(!h.unim_alive(&p1).await && !h.ibus_alive(&ip).await);

    // 남은 것만 회수된다.
    assert_eq!(h.reclaim(OWNER).await, 1);
    assert!(!h.unim_alive(&p2).await);
    // 회수된 컨텍스트에 늦게 도착한 Destroy 는 오류 응답일 뿐 패닉하지 않는다.
    let late = h
        .client
        .call_method(None::<&str>, p2.as_str(), Some(IC_IFACE), "Destroy", &())
        .await;
    assert!(late.is_err());
}
