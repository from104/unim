//! 상황 알림(토스트) 데몬 측 — 알림 채널 페이로드·전담 태스크·라우팅 (NOTIFY_SPEC §3.2)
//!
//! 엔진 워커(`std::thread`)는 [`build_notify_outs`] 로 게이트·문구를 끝낸 [`NotifyOut`] 을
//! 알림 채널에 `try_send` 만 한다(키 응답 경로 비차단). 이 채널을 받는 **전담 태스크 1개**
//! ([`spawn_notify_task`])만이 `Notify` 시그널 발행·`org.freedesktop.Notifications` 호출·
//! `replaces_id`·백오프 상태를 소유한다 — 알림마다 `tokio::spawn` 하면 생기는 `last_id` 경쟁
//! (뒤늦은 응답이 최신 1장을 덮어씀)이 구조적으로 없다.
//!
//! **로그 규칙(§2.3)**: `title`/`body` 는 어떤 로그에도 남기지 않는다 — `kind` 와 표시 경로만.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

use tokio::sync::{mpsc, RwLock};
use zbus::proxy;
use zbus::zvariant::Value;
use zbus::Connection;

use unim::config::{ContentPurpose, NotifyConfig};
use unim::notify::{render_with, Lang, NotifyEvent, NotifyGate};
use unim::unim_log;

use crate::service::FrontendMap;

/// 알림 채널(워커 → 전담 태스크) 용량. 가득 차면 워커는 폐기한다(`try_send`).
pub const NOTIFY_QUEUE_CAP: usize = 32;

/// `Notify` 시그널 flags: `show_text` 로 입력 텍스트가 문구에 포함됨(수신측 로그 금지).
pub const FLAG_TEXT_SHOWN: u32 = 0x01;
/// `Notify` 시그널 flags: 직전 알림을 교체한다(최신 1장 규칙, §2.2-3).
pub const FLAG_REPLACE: u32 = 0x02;

/// 확장이 `RegisterFrontend` 로 등록하는 알림 브리지 이름. 기존 `gnome-shell` 과 독립이라
/// 구버전 확장(`Notify` 미구독)은 이 이름이 없어 fdo 로 폴백한다(§3.2).
pub const NOTIFY_BRIDGE_NAME: &str = "gnome-shell-notify";

/// fdo 서버 부재·거부 시 재시도 보류 시간 (§3.2).
const FDO_BACKOFF: Duration = Duration::from_secs(60);
/// fdo 호출 한 번의 최대 대기. 알림 서버가 멈춰도 전담 태스크가 영구 정지하지 않게 한다.
const FDO_CALL_TIMEOUT: Duration = Duration::from_secs(3);

const FDO_APP_NAME: &str = "UNIM";
/// 알림 아이콘 이름 (P2 에서 실기 확정 — 지금은 설치된 트레이 아이콘 이름).
const FDO_APP_ICON: &str = "io.github.from104.unim.Indicator";

/// 표시 요청 1건. 게이트·문구 생성이 끝난 완성품이다.
#[derive(Clone, PartialEq, Eq)]
pub struct NotifyOut {
    /// DBus `kind` 문자열(snake_case, 테스트 액션은 `"test"`)
    pub kind: &'static str,
    pub title: String,
    /// 평문 본문 — `Notify` 시그널용(수신측이 St/MessageTray 로 그대로 그린다)
    pub body: String,
    /// `<`·`>`·`&` 를 이스케이프한 본문 — fdo 직접 호출용(body-markup 서버 대비, §2.3)
    pub body_markup: String,
    pub duration_ms: u32,
    /// 문구에 입력 텍스트(교정 전후·단어)가 실렸는가 → `FLAG_TEXT_SHOWN`
    pub text_shown: bool,
}

/// title/body/body_markup 에 입력 텍스트가 실릴 수 있어 길이만 찍는다.
impl std::fmt::Debug for NotifyOut {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("NotifyOut")
            .field("kind", &self.kind)
            .field("title_chars", &self.title.chars().count())
            .field("body_chars", &self.body.chars().count())
            .field("body_markup_chars", &self.body_markup.chars().count())
            .field("duration_ms", &self.duration_ms)
            .field("text_shown", &self.text_shown)
            .finish()
    }
}

impl NotifyOut {
    /// `Notify` 시그널 flags 값.
    pub fn flags(&self) -> u32 {
        let mut f = FLAG_REPLACE;
        if self.text_shown {
            f |= FLAG_TEXT_SHOWN;
        }
        f
    }

    /// `TriggerAction("notify_test")` 용 — 고정 문구, 입력 텍스트 없음.
    pub fn test(cfg: &NotifyConfig, lang: Lang) -> Self {
        let body = match lang {
            Lang::Ko => "알림 시험",
            Lang::En => "Notification test",
        };
        Self {
            kind: "test",
            title: unim::notify::NOTIFY_TITLE.to_string(),
            body: body.to_string(),
            body_markup: body.to_string(),
            duration_ms: cfg.duration_ms,
            text_shown: false,
        }
    }
}

/// 자동 언어 판정 — POSIX 우선순위 `LC_ALL` > `LC_MESSAGES` > `LANG` (코어는 env 를 읽지 않는다).
pub fn detect_auto_lang() -> Lang {
    let get = |k: &str| std::env::var(k).ok();
    let (a, m, l) = (get("LC_ALL"), get("LC_MESSAGES"), get("LANG"));
    Lang::detect(a.as_deref(), m.as_deref(), l.as_deref())
}

/// 이벤트 묶음을 게이트로 심사해 표시 요청으로 만든다 (엔진 워커 호출용 순수 함수).
///
/// `evs` 는 한 요청에서 모은 이벤트 전부다 — 규칙 6(ATF 교정 통과 시 같은 묶음의
/// `ModeChanged` 제거)은 [`NotifyGate::offer_batch`] 가 처리한다.
pub fn build_notify_outs(
    gate: &mut NotifyGate,
    evs: Vec<NotifyEvent>,
    purpose: ContentPurpose,
    cfg: &NotifyConfig,
    auto_lang: Lang,
    now: Instant,
) -> Vec<NotifyOut> {
    let lang = Lang::resolve(cfg.language, auto_lang);
    gate.offer_batch(evs, purpose, cfg, now)
        .into_iter()
        .map(|ev| {
            let kind = ev.kind;
            let plain = render_with(&ev, lang, cfg.show_text, false);
            let marked = render_with(&ev, lang, cfg.show_text, true);
            NotifyOut {
                kind: kind.as_str(),
                title: plain.title,
                body: plain.body,
                body_markup: marked.body,
                duration_ms: cfg.duration_ms,
                text_shown: cfg.show_text && kind.carries_text(),
            }
        })
        .collect()
}

/// 표시 경로.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Route {
    /// InputMethod `Notify` 시그널 (GNOME 확장 브리지가 그린다)
    Signal,
    /// 데몬이 `org.freedesktop.Notifications` 를 직접 호출
    Fdo,
}

/// 등록 목록으로 경로를 고른다: `gnome-shell-notify` 등록자가 1명 이상이면 시그널, 아니면 fdo.
pub fn choose_route(frontends: &FrontendMap) -> Route {
    if frontends
        .get(NOTIFY_BRIDGE_NAME)
        .is_some_and(|owners| !owners.is_empty())
    {
        Route::Signal
    } else {
        Route::Fdo
    }
}

/// 대기열을 비워 **마지막 1건**만 남긴다 (최신 1장 규칙, §2.2-3).
pub fn drain_latest(first: NotifyOut, rx: &mut mpsc::Receiver<NotifyOut>) -> NotifyOut {
    let mut latest = first;
    while let Ok(next) = rx.try_recv() {
        latest = next;
    }
    latest
}

/// fdo 호출 실패 뒤 재시도 보류 상태 (알림 서버 늦은 기동 대비 — 영구 차단 아님).
#[derive(Debug, Default)]
pub struct Backoff {
    retry_after: Option<Instant>,
}

impl Backoff {
    /// 지금 호출을 건너뛰어야 하는가.
    pub fn blocked(&self, now: Instant) -> bool {
        self.retry_after.is_some_and(|t| now < t)
    }

    /// 실패 기록 — 이후 [`FDO_BACKOFF`] 동안 호출을 건너뛴다.
    pub fn trip(&mut self, now: Instant) {
        self.retry_after = Some(now + FDO_BACKOFF);
    }

    /// 성공 기록 — 보류 해제.
    pub fn clear(&mut self) {
        self.retry_after = None;
    }
}

/// fdo `expire_timeout`(ms, i32). GNOME Shell 은 무시한다(KDE 등에서만 유효).
fn fdo_expire_timeout(duration_ms: u32) -> i32 {
    i32::try_from(duration_ms).unwrap_or(i32::MAX)
}

/// `org.freedesktop.Notifications` 프록시 (알림 서버 표준 인터페이스).
#[proxy(
    interface = "org.freedesktop.Notifications",
    default_service = "org.freedesktop.Notifications",
    default_path = "/org/freedesktop/Notifications"
)]
trait FdoNotifications {
    #[allow(clippy::too_many_arguments)]
    fn notify(
        &self,
        app_name: &str,
        replaces_id: u32,
        app_icon: &str,
        summary: &str,
        body: &str,
        actions: &[&str],
        hints: HashMap<&str, Value<'_>>,
        expire_timeout: i32,
    ) -> zbus::Result<u32>;
}

/// 알림 전담 태스크를 시작한다. 데몬 기동 시 한 번만 호출한다.
///
/// 이 태스크가 `Notify` 시그널 발행·fdo 호출·`last_id`(replaces_id)·백오프를 독점한다.
pub fn spawn_notify_task(
    connection: Connection,
    frontends: Arc<RwLock<FrontendMap>>,
    rx: mpsc::Receiver<NotifyOut>,
) -> tokio::task::JoinHandle<()> {
    tokio::spawn(run_notify_task(connection, frontends, rx))
}

async fn run_notify_task(
    connection: Connection,
    frontends: Arc<RwLock<FrontendMap>>,
    mut rx: mpsc::Receiver<NotifyOut>,
) {
    // fdo 프록시는 첫 필요 시점에 한 번 만든다(build 는 서버 부재여도 실패하지 않는다).
    let mut fdo_proxy: Option<FdoNotificationsProxy<'static>> = None;
    let mut last_id: u32 = 0;
    let mut backoff = Backoff::default();

    while let Some(first) = rx.recv().await {
        let out = drain_latest(first, &mut rx);
        let route = {
            let map = frontends.read().await;
            choose_route(&map)
        };
        match route {
            Route::Signal => {
                let res = connection
                    .emit_signal(
                        None::<&str>,
                        crate::INPUT_METHOD_PATH,
                        "org.atit.unim.InputMethod",
                        "Notify",
                        &(
                            out.kind,
                            out.title.as_str(),
                            out.body.as_str(),
                            out.duration_ms,
                            out.flags(),
                        ),
                    )
                    .await;
                match res {
                    Ok(()) => unim_log!("NOTIFY", "[Notify] kind={} 경로=signal", out.kind),
                    Err(e) => unim_log!(
                        "NOTIFY",
                        "[Notify] kind={} 경로=signal 발행 실패: {}",
                        out.kind,
                        e
                    ),
                }
            }
            Route::Fdo => {
                let now = Instant::now();
                if backoff.blocked(now) {
                    // 백오프 중 폐기 — 매 건 로그는 스팸이라 남기지 않는다(진입 시 1회 로그함).
                    continue;
                }
                if fdo_proxy.is_none() {
                    fdo_proxy = FdoNotificationsProxy::builder(&connection)
                        .cache_properties(zbus::proxy::CacheProperties::No)
                        .build()
                        .await
                        .ok();
                }
                let Some(proxy) = fdo_proxy.as_ref() else {
                    backoff.trip(now);
                    unim_log!(
                        "NOTIFY",
                        "[Notify] kind={} 경로=fdo 프록시 생성 실패 — {}초 백오프",
                        out.kind,
                        FDO_BACKOFF.as_secs()
                    );
                    continue;
                };
                let mut hints: HashMap<&str, Value<'_>> = HashMap::new();
                // D4: GNOME 알림 목록에 남기지 않는다. urgency=normal — low 는 GNOME 이 배너를 안 띄운다.
                hints.insert("transient", Value::Bool(true));
                hints.insert("urgency", Value::U8(1));
                let call = proxy.notify(
                    FDO_APP_NAME,
                    last_id,
                    FDO_APP_ICON,
                    &out.title,
                    &out.body_markup,
                    &[],
                    hints,
                    fdo_expire_timeout(out.duration_ms),
                );
                match tokio::time::timeout(FDO_CALL_TIMEOUT, call).await {
                    Ok(Ok(id)) => {
                        last_id = id;
                        backoff.clear();
                        unim_log!("NOTIFY", "[Notify] kind={} 경로=fdo", out.kind);
                    }
                    Ok(Err(e)) => {
                        backoff.trip(now);
                        unim_log!(
                            "NOTIFY",
                            "[Notify] kind={} 경로=fdo 실패: {} — {}초 백오프",
                            out.kind,
                            e,
                            FDO_BACKOFF.as_secs()
                        );
                    }
                    Err(_) => {
                        backoff.trip(now);
                        unim_log!(
                            "NOTIFY",
                            "[Notify] kind={} 경로=fdo 응답 시간 초과 — {}초 백오프",
                            out.kind,
                            FDO_BACKOFF.as_secs()
                        );
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;
    use unim::config::NotifyLanguage;
    use unim::notify::NotifyKind;
    use unim::typefix_blacklist::Direction;

    #[test]
    fn notify_out_debug_redacts_text() {
        let mut o = out("atf_corrected_forward", "비밀문장SECRET");
        o.title = "제목SECRET".into();
        o.body_markup = "마크SECRET".into();
        let d = format!("{o:?}");
        assert!(!d.contains("SECRET") && !d.contains("비밀") && !d.contains("마크"), "{d}");
        assert!(d.contains("body_chars: 10"), "{d}");
    }

    fn out(kind: &'static str, body: &str) -> NotifyOut {
        NotifyOut {
            kind,
            title: "UNIM".into(),
            body: body.into(),
            body_markup: body.into(),
            duration_ms: 2000,
            text_shown: false,
        }
    }

    fn cfg() -> NotifyConfig {
        NotifyConfig {
            language: NotifyLanguage::Ko,
            ..NotifyConfig::default()
        }
    }

    fn map(entries: &[(&str, &[&str])]) -> FrontendMap {
        entries
            .iter()
            .map(|(n, owners)| {
                (
                    n.to_string(),
                    owners.iter().map(|o| o.to_string()).collect::<HashSet<_>>(),
                )
            })
            .collect()
    }

    #[test]
    fn route_signal_only_when_bridge_registered() {
        assert_eq!(choose_route(&FrontendMap::new()), Route::Fdo);
        // 구버전 확장: gnome-shell 만 등록 → fdo 폴백(유실 없음)
        assert_eq!(
            choose_route(&map(&[("gnome-shell", &[":1.5"])])),
            Route::Fdo
        );
        assert_eq!(
            choose_route(&map(&[
                ("gnome-shell", &[":1.5"]),
                (NOTIFY_BRIDGE_NAME, &[":1.5"])
            ])),
            Route::Signal
        );
    }

    #[test]
    fn route_ignores_empty_owner_set() {
        assert_eq!(choose_route(&map(&[(NOTIFY_BRIDGE_NAME, &[])])), Route::Fdo);
    }

    #[test]
    fn flags_replace_always_text_shown_on_demand() {
        let mut o = out("atf_corrected_forward", "x");
        assert_eq!(o.flags(), FLAG_REPLACE);
        o.text_shown = true;
        assert_eq!(o.flags(), FLAG_REPLACE | FLAG_TEXT_SHOWN);
    }

    #[tokio::test]
    async fn drain_latest_keeps_last_only() {
        let (tx, mut rx) = mpsc::channel(8);
        tx.send(out("a", "1")).await.unwrap();
        tx.send(out("b", "2")).await.unwrap();
        tx.send(out("c", "3")).await.unwrap();
        let first = rx.recv().await.unwrap();
        let latest = drain_latest(first, &mut rx);
        assert_eq!(latest.kind, "c");
        assert!(rx.try_recv().is_err(), "대기열이 비어야 한다");
    }

    #[tokio::test]
    async fn drain_latest_single_passthrough() {
        let (tx, mut rx) = mpsc::channel(8);
        tx.send(out("only", "1")).await.unwrap();
        let first = rx.recv().await.unwrap();
        assert_eq!(drain_latest(first, &mut rx).kind, "only");
    }

    #[test]
    fn backoff_blocks_for_sixty_seconds() {
        let t0 = Instant::now();
        let mut b = Backoff::default();
        assert!(!b.blocked(t0));
        b.trip(t0);
        assert!(b.blocked(t0 + Duration::from_secs(59)));
        assert!(!b.blocked(t0 + Duration::from_secs(60)));
        b.trip(t0);
        b.clear();
        assert!(!b.blocked(t0 + Duration::from_secs(1)));
    }

    #[test]
    fn expire_timeout_saturates() {
        assert_eq!(fdo_expire_timeout(2000), 2000);
        assert_eq!(fdo_expire_timeout(u32::MAX), i32::MAX);
    }

    #[test]
    fn build_outs_masks_text_by_default() {
        let mut gate = NotifyGate::new();
        let ev = NotifyEvent::atf_corrected(Direction::Forward, 1, "gksrmf", "한글");
        let outs = build_notify_outs(
            &mut gate,
            vec![ev],
            ContentPurpose::Normal,
            &cfg(),
            Lang::En,
            Instant::now(),
        );
        assert_eq!(outs.len(), 1);
        assert_eq!(outs[0].kind, "atf_corrected_forward");
        assert!(!outs[0].body.contains("gksrmf") && !outs[0].body.contains("한글"));
        assert!(!outs[0].text_shown);
        assert_eq!(outs[0].duration_ms, cfg().duration_ms);
    }

    #[test]
    fn build_outs_show_text_sets_flag_and_escapes_markup_copy_only() {
        let mut gate = NotifyGate::new();
        let c = NotifyConfig {
            show_text: true,
            ..cfg()
        };
        let ev = NotifyEvent::atf_corrected(Direction::Forward, 1, "a<b", "c&d");
        let outs = build_notify_outs(
            &mut gate,
            vec![ev],
            ContentPurpose::Normal,
            &c,
            Lang::En,
            Instant::now(),
        );
        assert_eq!(outs.len(), 1);
        assert!(outs[0].text_shown);
        assert!(
            outs[0].body.contains("a<b"),
            "시그널용은 평문: {}",
            outs[0].body
        );
        assert!(
            outs[0].body_markup.contains("a&lt;b"),
            "{}",
            outs[0].body_markup
        );
        assert!(
            outs[0].body_markup.contains("c&amp;d"),
            "{}",
            outs[0].body_markup
        );
    }

    #[test]
    fn build_outs_language_setting_wins_over_auto() {
        let mut gate = NotifyGate::new();
        let ev = NotifyEvent::mode_toggle_suppressed(3);
        let c = NotifyConfig {
            language: NotifyLanguage::En,
            ..NotifyConfig::default()
        };
        let outs = build_notify_outs(
            &mut gate,
            vec![ev],
            ContentPurpose::Password,
            &c,
            Lang::Ko,
            Instant::now(),
        );
        assert_eq!(outs.len(), 1);
        assert!(
            outs[0].body.starts_with("Password field"),
            "{}",
            outs[0].body
        );
    }

    #[test]
    fn build_outs_rule6_drops_atf_induced_mode_changed() {
        let mut gate = NotifyGate::new();
        let evs = vec![
            NotifyEvent::atf_corrected(Direction::Forward, 1, "rkskek", "가나다"),
            NotifyEvent::mode_changed(1, true),
        ];
        let outs = build_notify_outs(
            &mut gate,
            evs,
            ContentPurpose::Normal,
            &NotifyConfig {
                events: vec!["atf_corrected".into(), "mode_changed".into()],
                ..cfg()
            },
            Lang::Ko,
            Instant::now(),
        );
        let kinds: Vec<&str> = outs.iter().map(|o| o.kind).collect();
        assert_eq!(kinds, vec![NotifyKind::AtfCorrectedForward.as_str()]);
    }

    #[test]
    fn build_outs_disabled_emits_nothing() {
        let mut gate = NotifyGate::new();
        let c = NotifyConfig {
            enabled: false,
            ..cfg()
        };
        let outs = build_notify_outs(
            &mut gate,
            vec![NotifyEvent::mode_toggle_suppressed(1)],
            ContentPurpose::Password,
            &c,
            Lang::Ko,
            Instant::now(),
        );
        assert!(outs.is_empty());
    }

    #[test]
    fn test_out_has_fixed_text_and_kind() {
        let o = NotifyOut::test(&cfg(), Lang::Ko);
        assert_eq!(o.kind, "test");
        assert_eq!(o.body, "알림 시험");
        assert!(!o.text_shown);
        assert_eq!(NotifyOut::test(&cfg(), Lang::En).body, "Notification test");
    }
}
