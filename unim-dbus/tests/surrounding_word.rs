//! 대상③ 커서 주변 단어(HANJA_WORD_SPEC v1.6 §2.4b·§4.6) p2p 시험.
//!
//! `surrounding_order.rs` 와 같은 하네스 — 버스 데몬 없이 `UnixStream::pair()` 로 서버/클라이언트를
//! 연결하고, 호스트의 `~/.config/unim` 에 닿지 않도록 HOME·XDG 를 임시 디렉터리로 돌린다.
//! 클라이언트 쪽 `MessageStream` 으로 데몬이 내는 시그널(`ReplaceSurroundingText` 등)을 수집한다.

use futures_util::StreamExt;
use std::sync::OnceLock;
use std::time::Duration;
use tokio::net::UnixStream;
use tokio::sync::mpsc;
use unim::config::Config;
use unim_dbus::engine_worker::spawn_engine_worker;
use unim_dbus::service::InputMethodService;
use unim_dbus::INPUT_METHOD_PATH;
use zbus::connection::Builder;
use zbus::message::{Flags, Message, Type};
use zbus::MessageStream;

const IM_IFACE: &str = "org.atit.unim.InputMethod";
const IC_IFACE: &str = "org.atit.unim.InputContext";
/// evdev KEY_HANJA(123) — 기본 한자키.
const KEY_HANJA: u32 = 123;
/// Escape (evdev 1).
const KEY_ESC: u32 = 1;

static HOME: OnceLock<tempfile::TempDir> = OnceLock::new();

/// 서버 연결을 `Builder::serve_at` 으로 만들기 위한 빈 인터페이스.
///
/// zbus 4.4 의 `Connection::object_server()` 는 메서드 호출 수신 태스크를 띄우기만 하고
/// 그 태스크가 수신 구독(add_match)을 마칠 때까지 기다리지 않는다. 그 사이 도착한 메서드
/// 호출은 받을 스트림이 없어 응답 없이 버려진다 — 병렬 실행으로 CPU 가 붐비면 첫
/// `CreateInputContext` 가 이 틈에 떨어져 시험이 영원히 멈췄다. `Builder` 는 인터페이스가
/// 하나라도 있으면 구독 완료를 기다린 뒤 소켓 읽기를 시작하므로 이 틈이 없다.
struct ObjectServerReady;

#[zbus::interface(name = "org.atit.unim.test.Ready")]
impl ObjectServerReady {}

const READY_PATH: &str = "/org/atit/unim/test/ready";

struct Harness {
    #[allow(dead_code)]
    server: zbus::Connection,
    client: zbus::Connection,
    path: String,
    /// 시그널 수집용 스트림. 읽지 않는 시험은 `None`(zbus 수신 큐가 차면 연결이 멈춘다).
    signals: Option<MessageStream>,
}

async fn setup() -> zbus::Result<Harness> {
    setup_with(true).await
}

async fn setup_with(collect_signals: bool) -> zbus::Result<Harness> {
    // 호스트 설정·데이터 디렉터리 격리 — 프로세스 전체에서 한 번만 (edition 2021 — set_var 는 안전 함수)
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

    let (notify_tx, _notify_rx) = mpsc::channel(16);
    let engine_tx = spawn_engine_worker(Config::default(), notify_tx);
    let svc = InputMethodService::new(Config::default(), engine_tx, server.clone());
    server.object_server().at(INPUT_METHOD_PATH, svc).await?;

    // 시그널 수신은 호출 전에 구독해 둔다.
    let signals = collect_signals.then(|| MessageStream::from(&client));
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
    Ok(Harness { server, client, path, signals })
}

impl Harness {
    /// 응답 없이(NoReplyExpected) 보내는 메서드 — 프런트의 fire-and-forget 과 같다.
    async fn send_no_reply<B>(&self, member: &str, body: &B) -> zbus::Result<()>
    where
        B: serde::Serialize + zbus::zvariant::DynamicType,
    {
        let msg = Message::method(self.path.as_str(), member)?
            .interface(IC_IFACE)?
            .with_flags(Flags::NoReplyExpected)?
            .build(body)?;
        self.client.send(&msg).await
    }

    async fn caps(&self, caps: u32) -> zbus::Result<()> {
        self.send_no_reply("SetClientCapabilities", &(caps,)).await
    }

    async fn surrounding(&self, text: &str, cursor: u32) -> zbus::Result<()> {
        self.send_no_reply("SetSurroundingText", &(text, cursor, cursor)).await
    }

    async fn hint(&self, kind: u32, keyval: u32, state: u32) -> zbus::Result<()> {
        self.send_no_reply("SurroundingHint", &(kind, keyval, state)).await
    }

    async fn process_key(&self, keycode: u32) -> zbus::Result<(bool, String, String)> {
        let reply = self
            .client
            .call_method(
                None::<&str>,
                self.path.as_str(),
                Some(IC_IFACE),
                "ProcessKeyEvent",
                &(0u32, keycode, 0u32),
            )
            .await?;
        reply.body().deserialize()
    }

    async fn get_hanja(&self) -> zbus::Result<(String, Vec<(String, String)>)> {
        let reply = self
            .client
            .call_method(None::<&str>, self.path.as_str(), Some(IC_IFACE), "GetHanjaCandidates", &())
            .await?;
        reply.body().deserialize()
    }

    async fn call0(&self, member: &str) -> zbus::Result<Message> {
        self.client
            .call_method(None::<&str>, self.path.as_str(), Some(IC_IFACE), member, &())
            .await
    }

    async fn select_hanja(&self, index: u32) -> zbus::Result<String> {
        let reply = self
            .client
            .call_method(None::<&str>, self.path.as_str(), Some(IC_IFACE), "SelectHanja", &(index,))
            .await?;
        reply.body().deserialize()
    }

    /// 정해진 시간 동안 도착하는 시그널을 수집한다(유휴 종료가 아니라 고정 창 — 보류 중 "아직 없음" 확인용).
    async fn collect_for(&mut self, ms: u64) -> Vec<Message> {
        let mut out = vec![];
        let stream = self.signals.as_mut().expect("시그널 수집이 꺼진 하네스");
        let end = tokio::time::Instant::now() + Duration::from_millis(ms);
        while let Ok(Some(Ok(msg))) = tokio::time::timeout_at(end, stream.next()).await {
            if msg.message_type() == Type::Signal {
                out.push(msg);
            }
        }
        out
    }

    /// 지금까지 도착한 시그널을 짧은 유휴 시간까지 모두 수집한다.
    async fn drain_signals(&mut self) -> Vec<Message> {
        let mut out = vec![];
        let stream = self.signals.as_mut().expect("시그널 수집이 꺼진 하네스");
        while let Ok(Some(Ok(msg))) =
            tokio::time::timeout(Duration::from_millis(300), stream.next()).await
        {
            if msg.message_type() == Type::Signal {
                out.push(msg);
            }
        }
        out
    }
}

fn member(m: &Message) -> String {
    m.header().member().map(|x| x.to_string()).unwrap_or_default()
}

fn members(msgs: &[Message]) -> Vec<String> {
    msgs.iter().map(member).collect()
}

/// `ShowHanjaPopup(target, candidates, top_row, x, y, w, h)`.
type ShowHanjaBody = (String, Vec<(String, String)>, String, i32, i32, i32, i32);

fn hanja_popup_targets(msgs: &[Message]) -> Vec<String> {
    msgs.iter()
        .filter(|m| member(m) == "ShowHanjaPopup")
        .map(|m| {
            let body: ShowHanjaBody = m.body().deserialize().expect("ShowHanjaPopup 본문");
            body.0
        })
        .collect()
}

fn has_emoji_popup(msgs: &[Message]) -> bool {
    msgs.iter().any(|m| member(m).starts_with("ShowEmoji"))
}

/// (7) caps 0x3 → 커서 끝 "국어교육" → 한자키 → SelectHanja(0) →
/// `ReplaceSurroundingText(4, 0, "國語敎育", "국어교육")` + `HidePopup`.
/// `CommitText`·`AutoTypefixApply` 는 나오지 않는다.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn target3_confirm_emits_replace_surrounding_text() -> zbus::Result<()> {
    let mut h = setup().await?;
    h.caps(0x3).await?;
    h.surrounding("국어교육 만세", 4).await?;
    h.process_key(KEY_HANJA).await?;
    let opened = h.drain_signals().await;
    assert_eq!(hanja_popup_targets(&opened), vec!["국어교육"], "{:?}", members(&opened));

    let text = h.select_hanja(0).await?;
    assert_eq!(text, "國語敎育");
    let sigs = h.drain_signals().await;
    let names = members(&sigs);
    let replace_at = names.iter().position(|n| n == "ReplaceSurroundingText");
    let hide_at = names.iter().position(|n| n == "HidePopup");
    assert!(replace_at.is_some() && hide_at.is_some(), "{names:?}");
    assert!(replace_at < hide_at, "교체가 HidePopup 보다 먼저: {names:?}");
    assert!(!names.iter().any(|n| n == "CommitText" || n == "AutoTypefixApply"), "{names:?}");
    let (a, b, commit, expect): (u32, u32, String, String) =
        sigs[replace_at.unwrap()].body().deserialize()?;
    assert_eq!((a, b, commit.as_str(), expect.as_str()), (4, 0, "國語敎育", "국어교육"));
    Ok(())
}

/// (7b) 키보드 확정: 팝업이 열린 상태에서 숫자키 1 → 같은 InputContext path 로 교체 시그널.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn target3_keyboard_confirm_emits_replace_surrounding_text() -> zbus::Result<()> {
    let mut h = setup().await?;
    h.caps(0x3).await?;
    h.surrounding("국어교육", 4).await?;
    h.process_key(KEY_HANJA).await?;
    let _ = h.drain_signals().await;
    // evdev KEY_1 = 2
    let (consumed, _preedit, _commit) = h.process_key(2).await?;
    assert!(consumed);
    let sigs = h.drain_signals().await;
    let names = members(&sigs);
    assert!(names.iter().any(|n| n == "ReplaceSurroundingText"), "{names:?}");
    assert!(!names.iter().any(|n| n == "CommitText" || n == "AutoTypefixApply"), "{names:?}");
    let rs = sigs.iter().find(|m| member(m) == "ReplaceSurroundingText").unwrap();
    let (a, b, commit, expect): (u32, u32, String, String) = rs.body().deserialize()?;
    assert_eq!((a, b, commit.as_str(), expect.as_str()), (4, 0, "國語敎育", "국어교육"));
    Ok(())
}

/// (8) 커서가 가운데(cursor=2)이면 첫 후보는 앞 2·뒤 2 전체 단어, 재타하면 다음 후보("국어").
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn target3_repress_cycles_candidates() -> zbus::Result<()> {
    let mut h = setup().await?;
    h.caps(0x3).await?;
    h.surrounding("국어교육", 2).await?;
    h.process_key(KEY_HANJA).await?;
    h.process_key(KEY_HANJA).await?;
    let sigs = h.drain_signals().await;
    assert_eq!(hanja_popup_targets(&sigs), vec!["국어교육", "국어"], "{:?}", members(&sigs));
    Ok(())
}

/// (9) 능력 미선언 → 대상③ 없이 종전 동작(이모지 팝업).
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn without_caps_hanja_key_opens_emoji() -> zbus::Result<()> {
    let mut h = setup().await?;
    h.surrounding("국어교육", 4).await?;
    h.process_key(KEY_HANJA).await?;
    let sigs = h.drain_signals().await;
    assert!(hanja_popup_targets(&sigs).is_empty(), "{:?}", members(&sigs));
    assert!(has_emoji_popup(&sigs), "{:?}", members(&sigs));
    Ok(())
}

/// (10) caps 0x5(포인터 힌트) → PRIMARY 변경 힌트 뒤에는 이모지, 일반 클릭 힌트 +
/// 새 surrounding 뒤에는 다시 대상③.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn pointer_hints_gate_target3() -> zbus::Result<()> {
    let mut h = setup().await?;
    h.caps(0x5).await?;
    h.surrounding("국어교육", 4).await?;
    h.hint(4, 0, 0).await?; // PRIMARY_CHANGED
    h.process_key(KEY_HANJA).await?;
    let sigs = h.drain_signals().await;
    assert!(hanja_popup_targets(&sigs).is_empty(), "{:?}", members(&sigs));
    assert!(has_emoji_popup(&sigs), "{:?}", members(&sigs));

    h.process_key(KEY_ESC).await?; // 이모지 팝업 닫기
    h.hint(2, 0, 0).await?; // POINTER_PLAIN — 거부 플래그 해제(낡음)
    h.surrounding("국어교육", 4).await?; // 새 보고 = 신선
    h.process_key(KEY_HANJA).await?;
    let sigs = h.drain_signals().await;
    assert_eq!(hanja_popup_targets(&sigs), vec!["국어교육"], "{:?}", members(&sigs));
    Ok(())
}

/// (11) 응답 없이 보낸 힌트가 바로 뒤의 GetHanjaCandidates 에 반영된다(직렬 처리).
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn no_reply_hint_applies_before_following_call() -> zbus::Result<()> {
    let h = setup_with(false).await?;
    h.caps(0x3).await?;
    for round in 0..20 {
        h.surrounding("국어교육", 4).await?;
        let hinted = round % 2 == 1;
        if hinted {
            h.hint(2, 0, 0).await?; // POINTER_PLAIN — 문서가 바뀔 수 있음(낡음)
        }
        let (target, _cands) = h.get_hanja().await?;
        let want = if hinted { "" } else { "국어교육" };
        assert_eq!(target, want, "round {round}");
        h.call0("CancelHanja").await?;
    }
    Ok(())
}

// =============================================================================
// 판정 보류(Defer, 통합 계획 P2) — push 컨텍스트(caps 0x5)
// =============================================================================

fn is_popup_signal(m: &Message) -> bool {
    let n = member(m);
    n == "ShowHanjaPopup" || n.starts_with("ShowEmoji") || n == "ShowSpecialPopup"
}

/// 낡음 거절(R7): 한자키는 consumed(팝업 없음)로 즉시 답하고, 100ms 뒤 도착한 SetSurroundingText 가
/// 재판정해 같은 InputContext path 로 한자 팝업을 낸다.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn defer_stale_then_report_opens_hanja() -> zbus::Result<()> {
    let mut h = setup().await?;
    h.caps(0x5).await?;
    h.surrounding("국어교육", 4).await?;
    h.hint(2, 0, 0).await?; // POINTER_PLAIN — 플래그 해제 + 낡음
    let (consumed, preedit, commit) = h.process_key(KEY_HANJA).await?;
    assert!(consumed && preedit.is_empty() && commit.is_empty(), "보류 응답은 consumed·빈 결과");
    let early = h.collect_for(100).await;
    assert!(!early.iter().any(is_popup_signal), "보류 중에는 팝업 없음: {:?}", members(&early));

    h.surrounding("국어교육", 4).await?;
    let sigs = h.drain_signals().await;
    assert_eq!(hanja_popup_targets(&sigs), vec!["국어교육"], "{:?}", members(&sigs));
    assert!(!has_emoji_popup(&sigs), "{:?}", members(&sigs));
    // 늦게 열린 팝업도 평소 팝업처럼 확정된다.
    assert_eq!(h.select_hanja(0).await?, "國語敎育");
    Ok(())
}

/// 보고가 오지 않으면 기한(150ms)에 이모지 폴백.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn defer_stale_deadline_opens_emoji() -> zbus::Result<()> {
    let mut h = setup().await?;
    h.caps(0x5).await?;
    h.surrounding("국어교육", 4).await?;
    h.hint(2, 0, 0).await?;
    let (consumed, _p, _c) = h.process_key(KEY_HANJA).await?;
    assert!(consumed);
    let sigs = h.drain_signals().await;
    assert!(hanja_popup_targets(&sigs).is_empty(), "{:?}", members(&sigs));
    assert!(has_emoji_popup(&sigs), "{:?}", members(&sigs));
    Ok(())
}

/// R10: 보류 중 다른 키가 오면 보류가 취소된다 — 팝업은 나오지 않고 그 키는 정상 처리된다.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn defer_cancelled_by_other_key() -> zbus::Result<()> {
    let mut h = setup().await?;
    h.caps(0x5).await?;
    h.surrounding("국어교육", 4).await?;
    h.hint(2, 0, 0).await?;
    assert!(h.process_key(KEY_HANJA).await?.0);
    // evdev KEY_A(30) — 기본 모드에 따라 조합(preedit) 또는 직접 입력(commit)
    let (consumed, preedit, commit) = h.process_key(30).await?;
    assert!(
        consumed && !(preedit.is_empty() && commit.is_empty()),
        "취소를 일으킨 키는 정상 처리: consumed={consumed} preedit={}자 commit={}자",
        preedit.chars().count(),
        commit.chars().count()
    );
    // 보류 기한(150ms)과 늦은 보고가 지나가도 팝업은 없다.
    h.surrounding("국어교육", 4).await?;
    let sigs = h.drain_signals().await;
    assert!(!sigs.iter().any(is_popup_signal), "{:?}", members(&sigs));
    Ok(())
}

/// R5·R6·R8: 필드 포커스 뒤 키 없는 커서 이동(클릭) 직후의 한자키는 안정(300ms)까지 보류했다가 한자.
/// 보류 중 같은 커서의 재송신(이동 아님)이 와도 일찍 이모지로 떨어지지 않고 안정 시각까지 기다린다.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn defer_settle_after_click_then_hanja() -> zbus::Result<()> {
    let mut h = setup().await?;
    h.caps(0x5).await?;
    h.hint(6, 0, 0).await?; // FIELD_FOCUS
    h.surrounding("국어교육", 0).await?; // 기준점
    let _ = h.collect_for(330).await; // 포커스 뒤 SETTLE 이 지나야 이동이 클릭으로 센다
    h.surrounding("국어교육", 4).await?; // 키 없는 커서 이동(클릭)
    assert!(h.process_key(KEY_HANJA).await?.0);
    let early = h.collect_for(100).await;
    assert!(!early.iter().any(is_popup_signal), "안정 대기 중 팝업 없음: {:?}", members(&early));
    h.surrounding("국어교육", 4).await?; // 재송신 — 이동 아님, 계속 대기
    let sigs = h.drain_signals().await;
    assert_eq!(hanja_popup_targets(&sigs), vec!["국어교육"], "{:?}", members(&sigs));
    assert!(!has_emoji_popup(&sigs), "{:?}", members(&sigs));
    Ok(())
}

/// 대기 중 또 이동이 오면 안정 시각이 늦춰진다(한자키 시점부터 450ms 상한 안). 타이머는 옛 기한에
/// 깨어 새 기한까지 다시 잔다.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn defer_settle_second_move_extends_deadline() -> zbus::Result<()> {
    let mut h = setup().await?;
    h.caps(0x5).await?;
    h.hint(6, 0, 0).await?;
    h.surrounding("국어교육", 0).await?;
    let _ = h.collect_for(330).await;
    h.surrounding("국어교육", 4).await?; // 이동 1
    assert!(h.process_key(KEY_HANJA).await?.0);
    let _ = h.collect_for(100).await;
    h.surrounding("국어교육", 3).await?; // 이동 2 — 안정 시각이 +300 뒤로
    // 옛 기한(이동 1 + 300ms)이 지나도 아직 팝업이 없어야 한다.
    let mid = h.collect_for(230).await;
    assert!(!mid.iter().any(is_popup_signal), "연장된 기한 전에는 팝업 없음: {:?}", members(&mid));
    let sigs = h.drain_signals().await;
    assert_eq!(hanja_popup_targets(&sigs).len(), 1, "{:?}", members(&sigs));
    assert!(!has_emoji_popup(&sigs), "{:?}", members(&sigs));
    Ok(())
}

/// R10: 보류 중 Reset·Destroy(컨텍스트 파괴)·POINTER_PLAIN 힌트가 오면 보류가 취소돼 늦은 팝업이 없다.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn defer_cancelled_by_reset_destroy_and_pointer() -> zbus::Result<()> {
    for how in ["Reset", "Destroy", "Pointer"] {
        let mut h = setup().await?;
        h.caps(0x5).await?;
        h.surrounding("국어교육", 4).await?;
        h.hint(2, 0, 0).await?;
        assert!(h.process_key(KEY_HANJA).await?.0, "{how}");
        match how {
            "Pointer" => h.hint(2, 0, 0).await?,
            m => {
                h.call0(m).await?;
            }
        }
        h.surrounding("국어교육", 4).await?; // 늦은 보고도 팝업을 만들지 못한다
        let sigs = h.drain_signals().await;
        assert!(!sigs.iter().any(is_popup_signal), "{how}: {:?}", members(&sigs));
    }
    Ok(())
}
