//! 상황 알림(토스트) — TSF 쪽 순수 로직 (NOTIFY_SPEC §3.4).
//!
//! 코어 `unim::notify` 의 [`NotifyGate`]·`render_with` 를 그대로 재사용해 이벤트를 걸러
//! 문구까지 만든다. Win32 에 의존하지 않으므로 플랫폼 중립(cfg 미게이트)이고 Linux
//! `cargo test` 로 검증한다(`key_gate` 와 같은 방식). 화면 표시는 하지 않는다 — 결과
//! [`ToastOut`] 을 `popup_ipc` worker 가 `cmd="toast"` 로 렌더러에 보낸다(DLL 은 UI 금지).
//!
//! **문서(ITfDocumentMgr) 단위 규칙**: Linux 데몬의 `context_id` 에 해당하는 값으로 문서 포인터를
//! 접은 `u32`(`doc_id_from_ptr`)를 쓴다. 비밀번호 진입 1회/10분 재알림(§2.2-4)은 게이트가 이 id 로
//! 센다. `OnUninitDocumentMgr` 가 `on_destroy` 에 해당한다.
//!
//! **로그 규칙**: 제목·본문은 로그에 남기지 않는다 — [`ToastOut`] 의 `Debug` 는 kind 와 길이만 찍는다.

use std::time::{Duration, Instant};

use unim::config::{ContentPurpose, NotifyConfig};
use unim::notify::{render_with, Lang, NotifyEvent, NotifyGate};

/// 렌더러가 아직 안 떠 있어 보류한 첫 토스트의 수명(NOTIFY_SPEC §3.4). 초과하면 폐기한다.
pub const PENDING_TOAST_TTL: Duration = Duration::from_millis(1500);

/// `ToastPayload::flags` 비트 — `popup_ipc::toast_flags` 와 같은 값(그쪽이 와이어 정의).
pub const FLAG_TEXT_SHOWN: u32 = 0x01;

/// 표시 경로로 넘길 토스트 1건. 와이어 `ToastPayload` 의 소스.
#[derive(Clone, PartialEq, Eq)]
pub struct ToastOut {
    /// `NotifyKind::as_str()`
    pub kind: &'static str,
    /// 렌더러 중복 억제 키의 FNV-1a 64 해시(원문은 보내지 않는다).
    pub key_hash: u64,
    pub title: String,
    pub body: String,
    pub duration_ms: u32,
    /// `NotifyCorner::as_str()`
    pub corner: &'static str,
    pub flags: u32,
}

/// 문구가 입력 텍스트를 담을 수 있어 길이만 찍는다.
impl std::fmt::Debug for ToastOut {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ToastOut")
            .field("kind", &self.kind)
            .field("key_hash", &self.key_hash)
            .field("title_chars", &self.title.chars().count())
            .field("body_chars", &self.body.chars().count())
            .field("duration_ms", &self.duration_ms)
            .field("corner", &self.corner)
            .field("flags", &self.flags)
            .finish()
    }
}

/// FNV-1a 64 — 프로세스·버전과 무관하게 같은 입력에 같은 값(앱 간 dedupe 키 일치용).
pub fn fnv1a64(s: &str) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in s.as_bytes() {
        h ^= u64::from(*b);
        h = h.wrapping_mul(0x0000_0100_0000_01b3);
    }
    h
}

/// 문서(ITfDocumentMgr) 포인터를 `context_id` 용 `u32` 로 접는다. 0 은 "문서 없음"이므로 쓰지 않는다.
pub fn doc_id_from_ptr(ptr: usize) -> u32 {
    let p = ptr as u64;
    let id = (p ^ (p >> 32)) as u32;
    if id == 0 {
        1
    } else {
        id
    }
}

/// 비밀번호 진입/이탈 이벤트 — 비차단→차단이면 새 문서의 `password_enter`, 차단→비차단이면
/// **떠난 문서의** `password_leave`(게이트의 진입 기록이 그 문서 키이므로). 데몬
/// `focus_transition_event` 와 같은 규칙. 이전 포커스가 없으면 비차단으로 본다.
fn focus_transition_event(
    prev: Option<(u32, ContentPurpose)>,
    new_id: u32,
    new: ContentPurpose,
    atf_on: bool,
) -> Option<NotifyEvent> {
    let prev_block = prev.is_some_and(|(_, p)| p.should_block_hangul());
    match (prev_block, new.should_block_hangul()) {
        (false, true) => Some(NotifyEvent::password_enter(new_id, atf_on)),
        (true, false) => prev.map(|(id, _)| NotifyEvent::password_leave(id)),
        _ => None,
    }
}

/// 같은 문서 안에서 목적이 바뀐 경우(OnEndEdit 의 InputScope 변경) 전이 이벤트.
fn purpose_transition_event(
    prev: ContentPurpose,
    new: ContentPurpose,
    context_id: u32,
    atf_on: bool,
) -> Option<NotifyEvent> {
    match (prev.should_block_hangul(), new.should_block_hangul()) {
        (false, true) => Some(NotifyEvent::password_enter(context_id, atf_on)),
        (true, false) => Some(NotifyEvent::password_leave(context_id)),
        _ => None,
    }
}

/// TSF 서비스 인스턴스당 1개 — 게이트와 현재 포커스 문서를 묶는다. STA 스레드 전용.
pub struct TsfNotify {
    gate: NotifyGate,
    /// `notify.language=auto` 일 때 쓸 UI 언어 판정(활성화 시 1회).
    auto_lang: Lang,
    /// 현재 포커스 문서 `(doc_id, 목적)`. 포커스가 없으면 `None`.
    focus: Option<(u32, ContentPurpose)>,
}

impl TsfNotify {
    pub fn new(auto_lang: Lang) -> Self {
        Self {
            gate: NotifyGate::new(),
            auto_lang,
            focus: None,
        }
    }

    /// 현재 포커스 문서 id (없으면 0). ATF 이벤트 등 키 경로 이벤트의 `context_id` 로 쓴다.
    pub fn focus_id(&self) -> u32 {
        self.focus.map_or(0, |(id, _)| id)
    }

    /// 이벤트 묶음 하나(키 1회분)를 심사해 표시할 토스트로 바꾼다.
    ///
    /// 규칙 6(ATF 교정이 통과하면 같은 묶음의 ATF 유발 `mode_changed` 제거)은 `offer_batch` 가
    /// 처리하므로, 한 키에서 생긴 이벤트를 **한 번에** 넘겨야 한다. `purpose` 는 그 문서의
    /// **현재** 목적(비밀번호 상태의 텍스트 이벤트 폐기용).
    pub fn dispatch(
        &mut self,
        evs: Vec<NotifyEvent>,
        purpose: ContentPurpose,
        cfg: &NotifyConfig,
        now: Instant,
    ) -> Vec<ToastOut> {
        if evs.is_empty() {
            return Vec::new();
        }
        let lang = Lang::resolve(cfg.language, self.auto_lang);
        self.gate
            .offer_batch(evs, purpose, cfg, now)
            .into_iter()
            .map(|ev| {
                // 우리가 GDI 로 직접 그리므로 마크업 이스케이프는 하지 않는다.
                let text = render_with(&ev, lang, cfg.show_text, false);
                let text_shown = cfg.show_text && ev.kind.carries_text();
                // 텍스트를 담는 종류가 가림 상태면 문구가 모두 같으므로 키도 접는다 — 입력 단어
                // 에서 유도한 해시가 파이프로 나가지 않고, 같은 문구의 중복은 렌더러가 거른다.
                let key = if ev.kind.carries_text() && !cfg.show_text {
                    ""
                } else {
                    ev.dedupe_key(cfg.show_text)
                };
                ToastOut {
                    kind: ev.kind.as_str(),
                    key_hash: fnv1a64(key),
                    title: text.title,
                    body: text.body,
                    duration_ms: cfg.duration_ms,
                    corner: cfg.corner.as_str(),
                    flags: if text_shown { FLAG_TEXT_SHOWN } else { 0 },
                }
            })
            .collect()
    }

    /// 포커스 문서가 바뀐 직후 호출(`ITfThreadMgrEventSink::OnSetFocus`). `new` 가 `None` 이면
    /// 포커스가 문서 밖으로 나간 것(비차단으로 취급). 비밀번호 진입/이탈 토스트를 돌려준다.
    pub fn on_focus(
        &mut self,
        new: Option<(u32, ContentPurpose)>,
        atf_on: bool,
        cfg: &NotifyConfig,
        now: Instant,
    ) -> Vec<ToastOut> {
        let prev = self.focus;
        self.focus = new;
        // 규칙 2(컨텍스트당 1회) 리셋 — 떠나는 문서만. 규칙 4 맵은 건드리지 않는다.
        if let Some((prev_id, _)) = prev {
            if new.map(|(id, _)| id) != Some(prev_id) {
                self.gate.on_focus_out(prev_id);
            }
        }
        let (new_id, new_purpose) = new.unwrap_or((0, ContentPurpose::Normal));
        match focus_transition_event(prev, new_id, new_purpose, atf_on) {
            Some(ev) => self.dispatch(vec![ev], new_purpose, cfg, now),
            None => Vec::new(),
        }
    }

    /// 포커스 문서의 목적이 재포커스 없이 바뀐 직후 호출(`OnEndEdit` 의 InputScope 변경).
    /// 포커스 문서가 없으면(이론상) 아무것도 하지 않는다.
    pub fn on_purpose_change(
        &mut self,
        new: ContentPurpose,
        atf_on: bool,
        cfg: &NotifyConfig,
        now: Instant,
    ) -> Vec<ToastOut> {
        let Some((id, prev)) = self.focus else {
            return Vec::new();
        };
        self.focus = Some((id, new));
        if prev != new {
            self.gate.on_purpose_change(id);
        }
        match purpose_transition_event(prev, new, id, atf_on) {
            Some(ev) => self.dispatch(vec![ev], new, cfg, now),
            None => Vec::new(),
        }
    }

    /// 문서 소멸(`OnUninitDocumentMgr`): 그 문서의 게이트 상태(규칙 2·4)를 모두 지운다.
    pub fn on_destroy(&mut self, doc_id: u32) {
        self.gate.on_destroy(doc_id);
        if self.focus.is_some_and(|(id, _)| id == doc_id) {
            self.focus = None;
        }
    }
}

/// 렌더러 연결 전에 도착한 토스트를 **1건만** 짧게 쥐고 있는 보류 슬롯 (§3.4 "첫 토스트 지연").
///
/// worker 가 소유한다. 새 항목은 이전 보류분을 덮어쓴다(최신 1건). 시각은 호출자가 넘긴다.
pub struct PendingToast<T> {
    slot: Option<(T, Instant)>,
}

impl<T> Default for PendingToast<T> {
    fn default() -> Self {
        Self { slot: None }
    }
}

impl<T> PendingToast<T> {
    /// `queued_at`(토스트가 IME 스레드에서 큐에 들어간 시각)을 수명 기준으로 보류한다.
    pub fn hold(&mut self, item: T, queued_at: Instant) {
        self.slot = Some((item, queued_at));
    }

    pub fn is_pending(&self) -> bool {
        self.slot.is_some()
    }

    /// 수명이 남았으면 꺼내 주고(슬롯 비움), 지났으면 폐기하고 `None`. 비어 있어도 `None`.
    pub fn take_fresh(&mut self, now: Instant) -> Option<T> {
        let (item, at) = self.slot.take()?;
        if now.saturating_duration_since(at) <= PENDING_TOAST_TTL {
            Some(item)
        } else {
            None
        }
    }

    /// 수명이 남았는가(꺼내지 않고 확인). 만료됐으면 슬롯을 비운다.
    pub fn alive(&mut self, now: Instant) -> bool {
        match &self.slot {
            Some((_, at)) if now.saturating_duration_since(*at) <= PENDING_TOAST_TTL => true,
            Some(_) => {
                self.slot = None;
                false
            }
            None => false,
        }
    }

    /// 보류분을 버린다(렌더러 종료·채널 닫힘).
    pub fn clear(&mut self) {
        self.slot = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use unim::config::{NotifyCorner, NotifyLanguage};
    use unim::notify::NotifyKind;
    use unim::typefix_blacklist::Direction;

    fn cfg() -> NotifyConfig {
        NotifyConfig::default()
    }

    fn kinds(outs: &[ToastOut]) -> Vec<&'static str> {
        outs.iter().map(|o| o.kind).collect()
    }

    fn t0() -> Instant {
        Instant::now()
    }

    // ── 해시·id ──

    #[test]
    fn fnv1a64_is_stable_and_distinguishes() {
        // 표준 FNV-1a 64 테스트 벡터.
        assert_eq!(fnv1a64(""), 0xcbf2_9ce4_8422_2325);
        assert_eq!(fnv1a64("a"), 0xaf63_dc4c_8601_ec8c);
        assert_eq!(fnv1a64("foobar"), 0x8594_4171_f739_67e8);
        assert_ne!(fnv1a64("rkskek"), fnv1a64("rkskel"));
    }

    #[test]
    fn doc_id_is_nonzero_and_stable() {
        assert_ne!(doc_id_from_ptr(0), 0);
        assert_eq!(doc_id_from_ptr(0x7ff6_1234_5670), doc_id_from_ptr(0x7ff6_1234_5670));
        assert_ne!(doc_id_from_ptr(0x7ff6_1234_5670), doc_id_from_ptr(0x7ff6_1234_5680));
    }

    // ── dispatch: 문구·키·플래그 ──

    #[test]
    fn dispatch_masks_text_by_default_and_folds_key() {
        let mut n = TsfNotify::new(Lang::Ko);
        let ev = NotifyEvent::atf_corrected(Direction::Forward, 1, "rkskek", "가나다");
        let outs = n.dispatch(vec![ev], ContentPurpose::Normal, &cfg(), t0());
        assert_eq!(outs.len(), 1);
        let o = &outs[0];
        assert_eq!(o.kind, "atf_corrected_forward");
        assert!(!o.body.contains("rkskek") && !o.body.contains("가나다"), "show_text=false 는 가림");
        assert_eq!(o.key_hash, fnv1a64(""), "가림 상태의 텍스트 종류는 키를 접는다");
        assert_eq!(o.flags, 0);
        assert_eq!(o.corner, "auto");
        assert_eq!(o.duration_ms, cfg().duration_ms);
        assert_eq!(o.title, "UNIM");
    }

    #[test]
    fn dispatch_show_text_sets_flag_and_real_key() {
        let mut n = TsfNotify::new(Lang::Ko);
        let mut c = cfg();
        c.show_text = true;
        c.corner = NotifyCorner::TopLeft;
        let ev = NotifyEvent::atf_corrected(Direction::Forward, 1, "rkskek", "가나다");
        let outs = n.dispatch(vec![ev], ContentPurpose::Normal, &c, t0());
        let o = &outs[0];
        assert!(o.body.contains("가나다"));
        assert_eq!(o.flags, FLAG_TEXT_SHOWN);
        assert_eq!(o.key_hash, fnv1a64("rkskek"));
        assert_eq!(o.corner, "top_left");
    }

    #[test]
    fn dispatch_language_resolution() {
        let ev = || NotifyEvent::mode_toggle_suppressed(7);
        let mut c = cfg();
        c.language = NotifyLanguage::En;
        let en = TsfNotify::new(Lang::Ko).dispatch(vec![ev()], ContentPurpose::Normal, &c, t0());
        c.language = NotifyLanguage::Ko;
        let ko = TsfNotify::new(Lang::En).dispatch(vec![ev()], ContentPurpose::Normal, &c, t0());
        c.language = NotifyLanguage::Auto;
        let auto_en = TsfNotify::new(Lang::En).dispatch(vec![ev()], ContentPurpose::Normal, &c, t0());
        let auto_ko = TsfNotify::new(Lang::Ko).dispatch(vec![ev()], ContentPurpose::Normal, &c, t0());
        assert_ne!(en[0].body, ko[0].body);
        assert_eq!(en[0].body, auto_en[0].body);
        assert_eq!(ko[0].body, auto_ko[0].body);
    }

    #[test]
    fn dispatch_disabled_or_filtered_events_dropped() {
        let mut n = TsfNotify::new(Lang::Ko);
        let mut c = cfg();
        c.enabled = false;
        assert!(n
            .dispatch(vec![NotifyEvent::mode_toggle_suppressed(1)], ContentPurpose::Normal, &c, t0())
            .is_empty());
        let mut c = cfg();
        c.events = vec!["atf_suppressed".to_string()];
        assert!(n
            .dispatch(vec![NotifyEvent::mode_toggle_suppressed(1)], ContentPurpose::Normal, &c, t0())
            .is_empty());
    }

    #[test]
    fn dispatch_dedupes_within_three_seconds() {
        let mut n = TsfNotify::new(Lang::Ko);
        let base = t0();
        let mk = || NotifyEvent::atf_corrected(Direction::Forward, 1, "abc", "가");
        assert_eq!(n.dispatch(vec![mk()], ContentPurpose::Normal, &cfg(), base).len(), 1);
        assert!(n
            .dispatch(vec![mk()], ContentPurpose::Normal, &cfg(), base + Duration::from_millis(2999))
            .is_empty());
        assert_eq!(
            n.dispatch(vec![mk()], ContentPurpose::Normal, &cfg(), base + Duration::from_millis(3000))
                .len(),
            1
        );
    }

    #[test]
    fn password_state_discards_text_events() {
        // ATF 교정·억제·블랙리스트 학습은 비밀번호 칸에서 폐기 — show_text 와 무관.
        let mut n = TsfNotify::new(Lang::Ko);
        let mut c = cfg();
        c.show_text = true;
        let evs = vec![
            NotifyEvent::atf_corrected(Direction::Forward, 1, "secret", "비밀"),
            NotifyEvent::atf_suppressed(1, "secret"),
            NotifyEvent::blacklist_learned(1, "secret", 24),
        ];
        let outs = n.dispatch(evs, ContentPurpose::Password, &c, t0());
        assert!(outs.is_empty(), "{outs:?}");
        // PIN 도 같다.
        let outs = n.dispatch(
            vec![NotifyEvent::atf_corrected(Direction::Reverse, 2, "x", "y")],
            ContentPurpose::Pin,
            &c,
            t0(),
        );
        assert!(outs.is_empty());
    }

    #[test]
    fn atf_correction_suppresses_mode_changed_in_same_batch() {
        let mut n = TsfNotify::new(Lang::Ko);
        let mut c = cfg();
        c.events.push("mode_changed".to_string());
        let evs = vec![
            NotifyEvent::atf_corrected(Direction::Forward, 1, "abc", "가"),
            NotifyEvent::mode_changed(1, true),
        ];
        let outs = n.dispatch(evs, ContentPurpose::Normal, &c, t0());
        assert_eq!(kinds(&outs), vec!["atf_corrected_forward"]);
        // 교정 없이 모드만 바뀌면 mode_changed 가 나간다(events 에 켜 둔 경우).
        let outs = n.dispatch(vec![NotifyEvent::mode_changed(1, false)], ContentPurpose::Normal, &c, t0());
        assert_eq!(kinds(&outs), vec!["mode_changed"]);
    }

    #[test]
    fn debug_never_prints_text() {
        let mut n = TsfNotify::new(Lang::Ko);
        let mut c = cfg();
        c.show_text = true;
        let outs = n.dispatch(
            vec![NotifyEvent::atf_corrected(Direction::Forward, 1, "rkskek", "가나다")],
            ContentPurpose::Normal,
            &c,
            t0(),
        );
        let dbg = format!("{:?}", outs[0]);
        assert!(!dbg.contains("가나다") && !dbg.contains("rkskek"), "{dbg}");
        assert!(dbg.contains("atf_corrected_forward"));
    }

    // ── 포커스·비밀번호 전이 ──

    #[test]
    fn password_enter_once_per_document_then_reannounce_after_ten_minutes() {
        let mut n = TsfNotify::new(Lang::Ko);
        let base = t0();
        let pw = Some((10, ContentPurpose::Password));
        let outs = n.on_focus(pw, true, &cfg(), base);
        assert_eq!(kinds(&outs), vec!["password_enter"]);
        // 포커스 소실 → 같은 문서 재포커스(10분 미만) = 재알림 없음.
        assert!(n.on_focus(None, true, &cfg(), base + Duration::from_secs(5)).is_empty());
        assert!(n.on_focus(pw, true, &cfg(), base + Duration::from_secs(10)).is_empty());
        // 경계값: 599 s 는 아직, 600 s 는 재알림. (문서 이탈 후 재진입)
        n.on_focus(None, true, &cfg(), base + Duration::from_secs(20));
        assert!(n.on_focus(pw, true, &cfg(), base + Duration::from_secs(599)).is_empty());
        n.on_focus(None, true, &cfg(), base + Duration::from_secs(599));
        let outs = n.on_focus(pw, true, &cfg(), base + Duration::from_secs(600));
        assert_eq!(kinds(&outs), vec!["password_enter"]);
    }

    #[test]
    fn password_enter_body_mentions_no_typed_text_and_has_atf_variant() {
        let mut a = TsfNotify::new(Lang::Ko);
        let mut b = TsfNotify::new(Lang::Ko);
        let on = a.on_focus(Some((1, ContentPurpose::Password)), true, &cfg(), t0());
        let off = b.on_focus(Some((1, ContentPurpose::Password)), false, &cfg(), t0());
        assert_eq!(on.len(), 1);
        assert_eq!(off.len(), 1);
        assert_ne!(on[0].body, off[0].body, "atf_on 에 따라 문구가 다르다");
    }

    #[test]
    fn password_enter_not_fired_for_normal_focus_and_other_doc_same_state() {
        let mut n = TsfNotify::new(Lang::Ko);
        assert!(n.on_focus(Some((1, ContentPurpose::Normal)), true, &cfg(), t0()).is_empty());
        // 비밀번호 → 다른 비밀번호 문서: 차단 상태 불변이라 전이 없음(진입 알림은 1회만).
        let outs = n.on_focus(Some((2, ContentPurpose::Password)), true, &cfg(), t0());
        assert_eq!(kinds(&outs), vec!["password_enter"]);
        assert!(n.on_focus(Some((3, ContentPurpose::Password)), true, &cfg(), t0()).is_empty());
    }

    #[test]
    fn password_leave_only_when_enter_was_recorded_and_enabled() {
        let mut n = TsfNotify::new(Lang::Ko);
        let mut c = cfg();
        c.events.push("password_leave".to_string());
        let base = t0();
        n.on_focus(Some((1, ContentPurpose::Password)), true, &c, base);
        // 비밀번호 칸 → 일반 문서: 떠난 문서(1)의 leave.
        let outs = n.on_focus(Some((2, ContentPurpose::Normal)), true, &c, base);
        assert_eq!(kinds(&outs), vec!["password_leave"]);
        // 기본 events 에는 leave 가 없어 나가지 않는다.
        let mut m = TsfNotify::new(Lang::Ko);
        m.on_focus(Some((1, ContentPurpose::Password)), true, &cfg(), base);
        assert!(m.on_focus(Some((2, ContentPurpose::Normal)), true, &cfg(), base).is_empty());
    }

    #[test]
    fn enter_recorded_even_when_enter_filtered_so_leave_still_fires() {
        // 규칙 4: password_enter 를 events 에서 뺀 사용자가 leave 만 켜도 진입 기록은 남는다.
        let mut n = TsfNotify::new(Lang::Ko);
        let mut c = cfg();
        c.events = vec!["password_leave".to_string()];
        let base = t0();
        assert!(n.on_focus(Some((1, ContentPurpose::Password)), true, &c, base).is_empty());
        let outs = n.on_focus(None, true, &c, base);
        assert_eq!(kinds(&outs), vec!["password_leave"]);
    }

    #[test]
    fn mid_focus_purpose_change_enter_and_leave() {
        let mut n = TsfNotify::new(Lang::Ko);
        let mut c = cfg();
        c.events.push("password_leave".to_string());
        let base = t0();
        n.on_focus(Some((5, ContentPurpose::Normal)), true, &c, base);
        let enter = n.on_purpose_change(ContentPurpose::Password, true, &c, base);
        assert_eq!(kinds(&enter), vec!["password_enter"]);
        // 같은 목적 재호출은 전이 없음.
        assert!(n.on_purpose_change(ContentPurpose::Password, true, &c, base).is_empty());
        let leave = n.on_purpose_change(ContentPurpose::Normal, true, &c, base);
        assert_eq!(kinds(&leave), vec!["password_leave"]);
        // 포커스 문서가 없으면 무시.
        let mut empty = TsfNotify::new(Lang::Ko);
        assert!(empty.on_purpose_change(ContentPurpose::Password, true, &c, base).is_empty());
    }

    #[test]
    fn once_per_context_rule_resets_on_focus_out_and_destroy() {
        let mut n = TsfNotify::new(Lang::Ko);
        let base = t0();
        n.on_focus(Some((1, ContentPurpose::Normal)), true, &cfg(), base);
        let id = n.focus_id();
        assert_eq!(id, 1);
        let mk = |ctx| NotifyEvent::mode_toggle_suppressed(ctx);
        assert_eq!(n.dispatch(vec![mk(id)], ContentPurpose::Normal, &cfg(), base).len(), 1);
        // 규칙 2: 같은 컨텍스트에서는 3 초가 지나도 1회.
        let later = base + Duration::from_secs(10);
        assert!(n.dispatch(vec![mk(id)], ContentPurpose::Normal, &cfg(), later).is_empty());
        // FocusOut(다른 문서로 이동) 후 되돌아오면 다시 1회.
        n.on_focus(Some((2, ContentPurpose::Normal)), true, &cfg(), later);
        n.on_focus(Some((1, ContentPurpose::Normal)), true, &cfg(), later);
        assert_eq!(n.dispatch(vec![mk(1)], ContentPurpose::Normal, &cfg(), later).len(), 1);
        // DestroyContext 도 리셋하고, 포커스 문서였다면 포커스를 비운다.
        n.on_destroy(1);
        assert_eq!(n.focus_id(), 0);
    }

    #[test]
    fn destroy_clears_password_announcement() {
        let mut n = TsfNotify::new(Lang::Ko);
        let base = t0();
        let pw = Some((9, ContentPurpose::Password));
        assert_eq!(n.on_focus(pw, true, &cfg(), base).len(), 1);
        n.on_focus(None, true, &cfg(), base);
        n.on_destroy(9);
        // 소멸 후 같은 id(재사용 포인터)로 다시 들어오면 새 칸으로 본다.
        assert_eq!(n.on_focus(pw, true, &cfg(), base + Duration::from_secs(4)).len(), 1);
    }

    #[test]
    fn feature_toggled_and_toggle_blocked_kinds() {
        use unim::input_engine::AtfToggleKind;
        let mut n = TsfNotify::new(Lang::Ko);
        let outs = n.dispatch(
            vec![NotifyEvent::feature_toggled(1, AtfToggleKind::Enabled, true)],
            ContentPurpose::Normal,
            &cfg(),
            t0(),
        );
        assert_eq!(kinds(&outs), vec!["feature_toggled"]);
        assert_eq!(NotifyKind::FeatureToggled.as_str(), "feature_toggled");
        // 비밀번호 칸에서도 한/영 전환 차단 안내는 나간다(텍스트를 담지 않는 종류).
        let outs = n.dispatch(
            vec![NotifyEvent::mode_toggle_suppressed(2)],
            ContentPurpose::Password,
            &cfg(),
            t0(),
        );
        assert_eq!(kinds(&outs), vec!["mode_toggle_suppressed"]);
    }

    // ── 보류 슬롯 ──

    #[test]
    fn pending_holds_one_and_overwrites() {
        let mut p: PendingToast<&str> = PendingToast::default();
        let base = t0();
        assert!(!p.is_pending());
        p.hold("first", base);
        p.hold("second", base + Duration::from_millis(100));
        assert!(p.is_pending());
        assert_eq!(p.take_fresh(base + Duration::from_millis(200)), Some("second"));
        assert!(!p.is_pending());
        assert_eq!(p.take_fresh(base), None);
    }

    #[test]
    fn pending_ttl_boundary_1500ms() {
        let base = t0();
        let mut p: PendingToast<u8> = PendingToast::default();
        p.hold(1, base);
        assert_eq!(p.take_fresh(base + Duration::from_millis(1500)), Some(1), "1500 ms 까지 유효");
        p.hold(2, base);
        assert_eq!(p.take_fresh(base + Duration::from_millis(1501)), None, "초과 시 폐기");
        assert!(!p.is_pending(), "폐기 후 슬롯은 비어 있다");
    }

    #[test]
    fn pending_alive_expires_and_clear() {
        let base = t0();
        let mut p: PendingToast<u8> = PendingToast::default();
        p.hold(1, base);
        assert!(p.alive(base + Duration::from_millis(1000)));
        assert!(p.is_pending(), "alive 는 꺼내지 않는다");
        assert!(!p.alive(base + Duration::from_millis(1600)));
        assert!(!p.is_pending());
        p.hold(3, base);
        p.clear();
        assert!(!p.is_pending());
    }

    #[test]
    fn pending_ttl_matches_spec() {
        assert_eq!(PENDING_TOAST_TTL, Duration::from_millis(1500));
    }
}
