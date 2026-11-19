//! SetSurroundingText(응답 없이 보냄) → GetHanjaCandidates 순서 보장 시험.
//!
//! 데몬 InputContext 인터페이스는 메서드별 태스크를 만들지 않고 도착 순서대로 처리해야 한다
//! (zbus 4.4 의 `spawn` 속성은 반전되어 있다 — service.rs 주석 참조). 태스크가 갈라지면
//! 엔진 채널 진입 순서가 뒤집혀 낡은 선택으로 한자 변환이 일어난다.
//!
//! 버스 데몬 없이 `UnixStream::pair()` 로 서버/클라이언트를 p2p 연결해 구성한다.
//! 호스트의 `~/.config/unim` 에 닿지 않도록 HOME·XDG 경로를 임시 디렉터리로 돌린 뒤
//! 엔진 워커를 띄운다.

use tokio::net::UnixStream;
use tokio::sync::mpsc;
use unim::config::Config;
use unim_dbus::engine_worker::spawn_engine_worker;
use unim_dbus::service::{InputContextHandler, InputMethodService};
use unim_dbus::INPUT_METHOD_PATH;
use zbus::connection::Builder;
use zbus::message::{Flags, Message};
use zbus::object_server::Interface;

/// 서버 연결을 `Builder::serve_at` 으로 만들기 위한 빈 인터페이스.
///
/// zbus 4.4 의 `Connection::object_server()` 는 메서드 호출 수신 태스크를 띄우기만 하고
/// 그 태스크가 수신 구독(add_match)을 마칠 때까지 기다리지 않는다. 그 사이 도착한 메서드
/// 호출은 응답 없이 버려져 첫 `CreateInputContext` 가 영원히 멈출 수 있다. `Builder` 는
/// 인터페이스가 하나라도 있으면 구독 완료를 기다린 뒤 소켓 읽기를 시작한다.
struct ObjectServerReady;

#[zbus::interface(name = "org.atit.unim.test.Ready")]
impl ObjectServerReady {}

const READY_PATH: &str = "/org/atit/unim/test/ready";

const IM_IFACE: &str = "org.atit.unim.InputMethod";
const IC_IFACE: &str = "org.atit.unim.InputContext";

/// 응답 없이(NoReplyExpected) 보내는 SetSurroundingText
async fn send_no_reply(
    conn: &zbus::Connection,
    path: &str,
    text: &str,
    cursor: u32,
    anchor: u32,
) -> zbus::Result<()> {
    let msg = Message::method(path, "SetSurroundingText")?
        .interface(IC_IFACE)?
        .with_flags(Flags::NoReplyExpected)?
        .build(&(text, cursor, anchor))?;
    conn.send(&msg).await
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn set_surrounding_is_applied_before_following_call() -> zbus::Result<()> {
    // 호스트 설정·데이터 디렉터리 격리 (edition 2021 — set_var 는 안전 함수)
    let tmp = tempfile::tempdir().expect("tempdir");
    std::env::set_var("HOME", tmp.path());
    std::env::set_var("XDG_CONFIG_HOME", tmp.path().join("config"));
    std::env::set_var("XDG_DATA_HOME", tmp.path().join("data"));

    let (a, b) = UnixStream::pair().expect("socketpair");
    let guid = zbus::Guid::generate();
    let (server, client) = tokio::try_join!(
        // serve_at: 빌더가 ObjectServer 수신 구독을 마친 뒤에 소켓 읽기를 시작하게 한다
        // (`ObjectServerReady` 주석 참조).
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

    // 서버: 엔진 워커 + InputMethod 서비스
    let (notify_tx, _notify_rx) = mpsc::channel(16);
    let engine_tx = spawn_engine_worker(Config::default(), notify_tx);
    let svc = InputMethodService::new(Config::default(), engine_tx, server.clone());
    server.object_server().at(INPUT_METHOD_PATH, svc).await?;

    let reply = client
        .call_method(
            None::<&str>,
            INPUT_METHOD_PATH,
            Some(IM_IFACE),
            "CreateInputContext",
            &("test", "w1"),
        )
        .await?;
    let path: String = reply.body().deserialize()?;

    // 불변식: InputContext 는 직렬 디스패치(메서드별 태스크 없음)
    {
        let iface = server
            .object_server()
            .interface::<_, InputContextHandler>(path.as_str())
            .await?;
        let guard = iface.get().await;
        assert!(
            !guard.spawn_tasks_for_methods(),
            "InputContext 가 메서드별 태스크를 만든다 — zbus 4.4 는 `spawn = true` 가 끄는 쪽이다"
        );
    }

    for round in 0..20 {
        // (1,2) 와 (0,2) 를 번갈아 8회. 마지막 값은 회차마다 교대
        let last_full = round % 2 == 0;
        for i in 0..8 {
            let full = if i == 7 { last_full } else { i % 2 == 0 };
            let (cursor, anchor) = if full { (0, 2) } else { (1, 2) };
            send_no_reply(&client, &path, "서사", cursor, anchor).await?;
        }

        let reply = client
            .call_method(
                None::<&str>,
                path.as_str(),
                Some(IC_IFACE),
                "GetHanjaCandidates",
                &(),
            )
            .await?;
        let (target, _cands): (String, Vec<(String, String)>) = reply.body().deserialize()?;
        let want = if last_full { "서사" } else { "사" };
        assert_eq!(target, want, "round {round}: 낡은 선택으로 한자 변환됨");

        client
            .call_method(None::<&str>, path.as_str(), Some(IC_IFACE), "CancelHanja", &())
            .await?;
    }
    Ok(())
}
