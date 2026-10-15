//! 입력 컨텍스트 소유자 추적과 회수.
//!
//! `CreateInputContext` 로 만든 컨텍스트는 클라이언트가 `Destroy` 를 부르지 않고 죽으면 영원히
//! 남는다(CI 실측: create 428 / destroy 76). 전역 모드 전파 루프는 살아 있는 컨텍스트 수만큼
//! 도므로 누적은 곧 지연이다. 이 모듈은 컨텍스트를 만든 **호출자의 unique name** 을 기록해 두고,
//! 그 연결이 끊기면(`NameOwnerChanged`, 감시 태스크는 `service::spawn_frontend_watcher` 하나를
//! 공유한다) 그 소유자의 컨텍스트를 전부 회수한다.
//!
//! 회수와 명시적 `Destroy` 는 **같은 함수**([`destroy_context`])를 탄다 — 엔진 통지
//! (`EngineRequest::DestroyContext`, 알림 게이트의 비밀번호 칸 기록 등 per-context 상태 정리 포함),
//! `last_active` 캐시 무효화, object_server 에서 핸들러 제거.
//!
//! 로그는 개수와 컨텍스트 id 만 남긴다(사용자 텍스트·창 제목 평문 금지, HANJA_WORD_SPEC §2.8).

use std::collections::{BTreeSet, HashMap};
use std::sync::{Arc, Mutex};

use tokio::sync::{mpsc, oneshot};
use unim::unim_log;
use zbus::zvariant::ObjectPath;
use zbus::Connection;

use crate::ibus_compat::ibus_context::IBusInputContextHandler;
use crate::service::{EngineRequest, InputContextHandler};

/// 컨텍스트 종류 — object_server 에서 핸들러를 제거할 때 타입과 경로 형식을 정한다.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ContextKind {
    /// `org.atit.unim.InputContext` (`/org/atit/unim/InputContext_<id>`)
    Unim,
    /// `org.freedesktop.IBus.InputContext` (메인 서비스·포털 공통,
    /// `/org/freedesktop/IBus/InputContext_<id>`)
    IBus,
}

impl ContextKind {
    /// 이 종류·id 의 object path 문자열.
    pub fn object_path(self, id: u32) -> String {
        match self {
            ContextKind::Unim => format!("{}{}", crate::INPUT_CONTEXT_PATH_PREFIX, id),
            ContextKind::IBus => format!("{}{}", IBUS_IC_PATH_PREFIX, id),
        }
    }
}

/// IBus InputContext 객체 경로 접두사 (메인 서비스·포털 공통).
pub(crate) const IBUS_IC_PATH_PREFIX: &str = "/org/freedesktop/IBus/InputContext_";

/// owner → 컨텍스트 매핑의 순수 자료구조 (락·IO 없음 — 단위 시험 대상).
///
/// 불변식: `by_owner` 의 모든 id 는 `owner_of` 에 같은 owner 로 존재하고 그 역도 성립한다.
/// 빈 owner 항목은 남지 않는다.
#[derive(Default, Debug)]
pub struct OwnerMap {
    by_owner: HashMap<String, BTreeSet<u32>>,
    owner_of: HashMap<u32, (String, ContextKind)>,
}

impl OwnerMap {
    /// 컨텍스트를 owner 에 묶는다. 같은 id 를 다시 등록하면 소유자를 갈아끼운다(옛 항목 제거).
    pub fn register(&mut self, owner: &str, id: u32, kind: ContextKind) {
        self.forget(id);
        self.by_owner
            .entry(owner.to_string())
            .or_default()
            .insert(id);
        self.owner_of.insert(id, (owner.to_string(), kind));
    }

    /// 컨텍스트 하나를 제거한다(명시적 `Destroy` 경로). 반환값: 실제로 있었으면 true — 멱등.
    pub fn forget(&mut self, id: u32) -> bool {
        let Some((owner, _)) = self.owner_of.remove(&id) else {
            return false;
        };
        if let Some(ids) = self.by_owner.get_mut(&owner) {
            ids.remove(&id);
            if ids.is_empty() {
                self.by_owner.remove(&owner);
            }
        }
        true
    }

    /// owner 의 컨텍스트를 전부 꺼내고 매핑에서 지운다(id 오름차순). 없으면 빈 벡터 — 멱등.
    /// 다른 owner 의 항목은 건드리지 않는다.
    pub fn take_owner(&mut self, owner: &str) -> Vec<(u32, ContextKind)> {
        let Some(ids) = self.by_owner.remove(owner) else {
            return Vec::new();
        };
        ids.into_iter()
            .filter_map(|id| self.owner_of.remove(&id).map(|(_, kind)| (id, kind)))
            .collect()
    }

    /// 추적 중인 컨텍스트 수.
    pub fn len(&self) -> usize {
        self.owner_of.len()
    }

    pub fn is_empty(&self) -> bool {
        self.owner_of.is_empty()
    }

    /// 해당 id 가 추적 중인가.
    pub fn contains(&self, id: u32) -> bool {
        self.owner_of.contains_key(&id)
    }

    /// owner 가 가진 컨텍스트 수.
    pub fn count_for(&self, owner: &str) -> usize {
        self.by_owner.get(owner).map_or(0, BTreeSet::len)
    }
}

/// 데몬 전체가 공유하는 레지스트리 핸들 (복제 = 같은 맵).
///
/// `std::sync::Mutex` 를 쓰며 락은 `.await` 를 가로지르지 않는다.
#[derive(Clone, Default)]
pub struct ContextRegistry {
    inner: Arc<Mutex<OwnerMap>>,
}

impl ContextRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    fn with<R>(&self, f: impl FnOnce(&mut OwnerMap) -> R) -> R {
        // 독 상태(poison)여도 맵 자체는 일관적(단순 삽입·삭제뿐)이므로 그대로 이어서 쓴다.
        let mut guard = self.inner.lock().unwrap_or_else(|e| e.into_inner());
        f(&mut guard)
    }

    pub fn register(&self, owner: &str, id: u32, kind: ContextKind) {
        self.with(|m| m.register(owner, id, kind));
    }

    pub fn forget(&self, id: u32) -> bool {
        self.with(|m| m.forget(id))
    }

    pub fn take_owner(&self, owner: &str) -> Vec<(u32, ContextKind)> {
        self.with(|m| m.take_owner(owner))
    }

    pub fn len(&self) -> usize {
        self.with(|m| m.len())
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub fn contains(&self, id: u32) -> bool {
        self.with(|m| m.contains(id))
    }
}

/// 컨텍스트 파괴 공통 경로 — 명시적 `Destroy` 와 연결 종료 회수가 함께 쓴다. **멱등**:
/// 이미 파괴됐어도(엔진 `HashMap::remove`, `ObjectServer::remove` 의 `Err(InterfaceNotFound)` 는 버린다) 패닉·오류가 없다.
///
/// 1. 레지스트리에서 제거
/// 2. `last_active` 캐시가 이 컨텍스트를 가리키면 비움 — 죽은 path 로 글로벌
///    CommitEmoji/TriggerAction 시그널이 가는 것을 방지
/// 3. 엔진에 파괴 통지 — per-context 상태와 알림 게이트의 비밀번호 칸 기록 정리.
///    팝업이 열려 있었으면(UNIM 컨텍스트) `HidePopup` 발행
/// 4. object_server 에서 핸들러 제거 — zbus 내부 라우팅 테이블이 id 마다 누적되는 것 방지
pub async fn destroy_context(
    connection: &Connection,
    engine_tx: &mpsc::Sender<EngineRequest>,
    registry: &ContextRegistry,
    last_active: Option<&Arc<Mutex<Option<String>>>>,
    kind: ContextKind,
    id: u32,
) {
    registry.forget(id);

    let path_str = kind.object_path(id);
    if let Some(last_active) = last_active {
        let mut last = last_active.lock().unwrap_or_else(|e| e.into_inner());
        if last.as_deref() == Some(path_str.as_str()) {
            *last = None;
        }
    }

    let (response_tx, response_rx) = oneshot::channel();
    engine_tx
        .send(EngineRequest::DestroyContext {
            id,
            response: response_tx,
        })
        .await
        .ok();
    let had_popup = response_rx.await.unwrap_or(false);

    if let Ok(path) = ObjectPath::try_from(path_str.as_str()) {
        // 팝업이 열린 채 파괴됐으면 그 컨텍스트 path 로 HidePopup 을 낸다 — 앱이 팝업을 연 채
        // 죽어도 팝업 창이 화면에 남지 않게. 열려 있지 않았으면 내지 않는다: 경로 무관 구독자
        // (unim-gui-common)가 다른 앱의 열린 팝업까지 닫아 엔진과 어긋나는 것을 막는다.
        // IBus 컨텍스트는 팝업 시그널이 없다.
        if had_popup && kind == ContextKind::Unim {
            let _ = connection
                .emit_signal(
                    None::<&str>,
                    &path,
                    "org.atit.unim.InputContext",
                    "HidePopup",
                    &(),
                )
                .await;
        }
        let server = connection.object_server();
        let _ = match kind {
            ContextKind::Unim => server.remove::<InputContextHandler, _>(path).await,
            ContextKind::IBus => server.remove::<IBusInputContextHandler, _>(path).await,
        };
    }
}

/// 소유자(unique name) 연결 종료 시 그 소유자의 컨텍스트를 전부 회수한다. 반환값: 회수한 개수.
/// 같은 owner 에 대해 다시 불러도(또는 `Destroy` 와 경합해도) 안전하다 — 맵에서 한 번만 꺼내진다.
pub async fn reclaim_owner(
    connection: &Connection,
    engine_tx: &mpsc::Sender<EngineRequest>,
    registry: &ContextRegistry,
    last_active: Option<&Arc<Mutex<Option<String>>>>,
    owner: &str,
) -> usize {
    let taken = registry.take_owner(owner);
    if taken.is_empty() {
        return 0;
    }
    for &(id, kind) in &taken {
        destroy_context(connection, engine_tx, registry, last_active, kind, id).await;
    }
    let ids: Vec<u32> = taken.iter().map(|&(id, _)| id).collect();
    unim_log!(
        "DBUS",
        "[DBus] 연결 종료 {} — 컨텍스트 {}개 회수: ids={:?}",
        owner,
        ids.len(),
        ids
    );
    ids.len()
}

/// 컨텍스트를 만든 호출자를 기록한다. `sender` 가 `None`(p2p 연결 — 버스 없음)이면 기록하지 않는다:
/// 그 연결은 `NameOwnerChanged` 를 낼 수 없어 회수 대상이 될 수 없다.
///
/// 등록 직후 소유자가 아직 살아 있는지 `NameHasOwner` 로 한 번 확인한다 — 클라이언트가
/// `CreateInputContext` 응답을 받기 전에 죽으면 `NameOwnerChanged` 가 등록보다 먼저 도착해
/// 영영 회수되지 못하기 때문이다. 이미 죽었으면 즉시 회수한다.
pub(crate) async fn track_context(
    connection: &Connection,
    engine_tx: &mpsc::Sender<EngineRequest>,
    registry: &ContextRegistry,
    last_active: Option<&Arc<Mutex<Option<String>>>>,
    sender: Option<&str>,
    kind: ContextKind,
    id: u32,
) {
    let Some(owner) = sender else {
        return;
    };
    registry.register(owner, id, kind);

    let alive = match zbus::fdo::DBusProxy::new(connection).await {
        Ok(proxy) => match zbus::names::BusName::try_from(owner) {
            Ok(name) => proxy.name_has_owner(name).await.unwrap_or(true),
            Err(_) => true,
        },
        Err(_) => true,
    };
    if !alive {
        reclaim_owner(connection, engine_tx, registry, last_active, owner).await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const A: &str = ":1.10";
    const B: &str = ":1.20";

    #[test]
    fn register_and_take_returns_all_ids_sorted() {
        let mut m = OwnerMap::default();
        m.register(A, 3, ContextKind::Unim);
        m.register(A, 1, ContextKind::Unim);
        m.register(A, 1_000_002, ContextKind::IBus);
        assert_eq!(m.len(), 3);
        assert_eq!(m.count_for(A), 3);
        let got = m.take_owner(A);
        assert_eq!(
            got,
            vec![
                (1, ContextKind::Unim),
                (3, ContextKind::Unim),
                (1_000_002, ContextKind::IBus)
            ]
        );
        assert!(m.is_empty());
        assert_eq!(m.count_for(A), 0);
    }

    #[test]
    fn take_owner_is_idempotent() {
        let mut m = OwnerMap::default();
        m.register(A, 1, ContextKind::Unim);
        assert_eq!(m.take_owner(A).len(), 1);
        assert!(m.take_owner(A).is_empty(), "두 번째 회수는 빈 결과");
        assert!(m.take_owner("없는-owner").is_empty());
    }

    #[test]
    fn other_owner_is_untouched() {
        let mut m = OwnerMap::default();
        m.register(A, 1, ContextKind::Unim);
        m.register(B, 2, ContextKind::Unim);
        m.register(B, 3, ContextKind::IBus);
        assert_eq!(m.take_owner(A), vec![(1, ContextKind::Unim)]);
        assert!(!m.contains(1));
        assert!(m.contains(2) && m.contains(3));
        assert_eq!(m.count_for(B), 2);
    }

    #[test]
    fn forget_is_idempotent_and_prunes_empty_owner() {
        let mut m = OwnerMap::default();
        m.register(A, 1, ContextKind::Unim);
        assert!(m.forget(1), "첫 제거는 true");
        assert!(!m.forget(1), "재제거는 false (멱등)");
        assert!(
            m.take_owner(A).is_empty(),
            "Destroy 된 컨텍스트는 회수 대상이 아니다"
        );
        assert!(m.is_empty());
    }

    #[test]
    fn forget_leaves_siblings_for_reclaim() {
        let mut m = OwnerMap::default();
        m.register(A, 1, ContextKind::Unim);
        m.register(A, 2, ContextKind::Unim);
        assert!(m.forget(1));
        assert_eq!(m.take_owner(A), vec![(2, ContextKind::Unim)]);
    }

    #[test]
    fn reregister_moves_ownership() {
        let mut m = OwnerMap::default();
        m.register(A, 1, ContextKind::Unim);
        m.register(B, 1, ContextKind::Unim);
        assert_eq!(m.len(), 1);
        assert!(m.take_owner(A).is_empty());
        assert_eq!(m.take_owner(B), vec![(1, ContextKind::Unim)]);
    }

    #[test]
    fn object_paths_match_creation_format() {
        assert_eq!(
            ContextKind::Unim.object_path(15),
            "/org/atit/unim/InputContext_15"
        );
        assert_eq!(
            ContextKind::IBus.object_path(1_000_001),
            "/org/freedesktop/IBus/InputContext_1000001"
        );
    }

    #[test]
    fn registry_handle_shares_state() {
        let r = ContextRegistry::new();
        let r2 = r.clone();
        r.register(A, 7, ContextKind::Unim);
        assert!(r2.contains(7));
        assert!(r2.forget(7));
        assert!(r.is_empty());
    }
}
