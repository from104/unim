//! 상황 알림(토스트) 코어 — 이벤트 타입·게이트·문구 (NOTIFY_SPEC §2·§3.1c)
//!
//! 입력기 내부에서 일어난 "사용자가 알아야 할 상황"(자동교정 발동·억제, 비밀번호 칸 진입,
//! 한/영 전환 차단 등)을 **플랫폼 독립적인 이벤트**로 정의한다. 이 모듈은 UI·환경변수·
//! 시계에 의존하지 않는 순수 모듈이다(E3) — 표시는 데몬(DBus 시그널/fdo)·Windows TSF 가 한다.
//!
//! - [`NotifyEvent`]: 발생한 상황 1건(종류·중복 억제 키·컨텍스트·문구 파라미터).
//! - [`NotifyGate`]: 중복 억제·1회 규칙·설정 필터·민감정보 폐기(§2.2 규칙 1~6, §2.3 대책 3).
//! - [`render`]: ko/en 문구 생성. 기본(`show_text=false`)은 입력 텍스트를 **드러내지 않는다**(D1).
//! - [`Lang`]: 문구 언어. 로케일 환경변수는 코어가 읽지 않고 호출자가 값을 넘긴다.

use std::collections::{HashMap, HashSet};
use std::time::{Duration, Instant};

use crate::config::{ContentPurpose, NotifyConfig, NotifyLanguage};
use crate::input_engine::AtfToggleKind;
use crate::typefix_blacklist::Direction;

/// 동일 `(kind, key)` 재발을 폐기하는 창 (규칙 1).
pub const DEDUPE_WINDOW: Duration = Duration::from_millis(3000);

/// 비밀번호 칸 진입 알림의 재알림 간격 (규칙 4, E5). 설정 키가 아니다.
pub const PASSWORD_REANNOUNCE: Duration = Duration::from_secs(600);

/// 알림 제목 (표시 경로가 그대로 쓴다).
pub const NOTIFY_TITLE: &str = "UNIM";

/// 문구 파라미터(`{before}`·`{after}`·`{word}`) 최대 글자 수 (초과분은 `…`).
const PARAM_MAX_CHARS: usize = 16;
/// 한 줄 문구 최대 글자 수 (ko / en).
const LINE_MAX_CHARS_KO: usize = 40;
const LINE_MAX_CHARS_EN: usize = 60;

/// 알림 이벤트 종류 (DBus `kind` 의 원천).
///
/// `feature_result`(확장 로컬 발행, Q10)는 코어 이벤트가 아니므로 여기에 없다 — 설정명만
/// [`crate::config::NOTIFY_EVENT_NAMES`] 에 있다.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum NotifyKind {
    AtfCorrectedForward,
    AtfCorrectedReverse,
    AtfSuppressed,
    BlacklistLearned,
    PasswordEnter,
    PasswordLeave,
    ModeToggleSuppressed,
    ModeChanged,
    FeatureToggled,
}

impl NotifyKind {
    /// DBus `kind` 문자열 상수 (snake_case).
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::AtfCorrectedForward => "atf_corrected_forward",
            Self::AtfCorrectedReverse => "atf_corrected_reverse",
            Self::AtfSuppressed => "atf_suppressed",
            Self::BlacklistLearned => "blacklist_learned",
            Self::PasswordEnter => "password_enter",
            Self::PasswordLeave => "password_leave",
            Self::ModeToggleSuppressed => "mode_toggle_suppressed",
            Self::ModeChanged => "mode_changed",
            Self::FeatureToggled => "feature_toggled",
        }
    }

    /// `notify.events` 리스트 원소(설정명). 두 `AtfCorrected*` 는 `atf_corrected` 로 합친다.
    pub fn setting_name(&self) -> &'static str {
        match self {
            Self::AtfCorrectedForward | Self::AtfCorrectedReverse => "atf_corrected",
            other => other.as_str(),
        }
    }

    /// 입력 텍스트(교정 전후·단어)를 담을 수 있는 종류인가 — 비밀번호 상태에서 폐기 대상.
    pub fn carries_text(&self) -> bool {
        matches!(
            self,
            Self::AtfCorrectedForward
                | Self::AtfCorrectedReverse
                | Self::AtfSuppressed
                | Self::BlacklistLearned
        )
    }

    /// 규칙 2 대상 — `context_id` 당 1회(FocusOut·목적 변경 시 리셋).
    fn once_per_context(&self) -> bool {
        matches!(self, Self::AtfSuppressed | Self::ModeToggleSuppressed)
    }
}

/// 문구에 들어가는 kind 별 타입드 값. 쓰지 않는 필드는 기본값.
#[derive(Clone, Default, PartialEq, Eq)]
pub struct NotifyParams {
    /// 교정 전 텍스트 (`show_text=true` 일 때만 문구에 쓰인다)
    pub before: String,
    /// 교정 후 텍스트 (`show_text=true` 일 때만)
    pub after: String,
    /// 억제·학습된 단어 (`show_text=true` 일 때만)
    pub word: String,
    /// 임시 제외 시간(시간 단위, `blacklist_learned`)
    pub hours: u16,
    /// 바뀐 모드가 한글인지 (`mode_changed`)
    pub is_korean: bool,
    /// 토글된 ATF 기능 (`feature_toggled`)
    pub feature: Option<AtfToggleKind>,
    /// 토글 후 값 — 켜짐이면 true (`feature_toggled`)
    pub value: bool,
    /// 비밀번호 칸 진입 시점에 ATF 가 켜져 있었는지 (`password_enter` 문구 분기)
    pub atf_on: bool,
}

/// 입력 텍스트는 로그에 남기지 않는다 — 길이만 찍는다.
impl std::fmt::Debug for NotifyParams {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("NotifyParams")
            .field("before_chars", &self.before.chars().count())
            .field("after_chars", &self.after.chars().count())
            .field("word_chars", &self.word.chars().count())
            .field("hours", &self.hours)
            .field("is_korean", &self.is_korean)
            .field("feature", &self.feature)
            .field("value", &self.value)
            .field("atf_on", &self.atf_on)
            .finish()
    }
}

/// 알림 이벤트 1건.
#[derive(Clone, PartialEq, Eq)]
pub struct NotifyEvent {
    pub kind: NotifyKind,
    /// 중복 억제용 원시 키(§2.1 "key" 열). 가림 상태의 실효 키는 [`Self::dedupe_key`].
    pub key: String,
    /// IM 컨텍스트 id. 전역 이벤트(`SetGlobalMode`)는 0.
    pub context_id: u32,
    pub params: NotifyParams,
}

/// `key` 에 입력 단어가 들어갈 수 있어 길이만 찍는다.
impl std::fmt::Debug for NotifyEvent {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("NotifyEvent")
            .field("kind", &self.kind)
            .field("key_chars", &self.key.chars().count())
            .field("context_id", &self.context_id)
            .field("params", &self.params)
            .finish()
    }
}

impl NotifyEvent {
    fn new(kind: NotifyKind, key: String, context_id: u32, params: NotifyParams) -> Self {
        Self {
            kind,
            key,
            context_id,
            params,
        }
    }

    /// ATF 자동 교정 발동. `before` 는 역방향이면 근사 복원 텍스트.
    pub fn atf_corrected(direction: Direction, context_id: u32, before: &str, after: &str) -> Self {
        let kind = match direction {
            Direction::Forward => NotifyKind::AtfCorrectedForward,
            Direction::Reverse => NotifyKind::AtfCorrectedReverse,
        };
        Self::new(
            kind,
            before.to_string(),
            context_id,
            NotifyParams {
                before: before.to_string(),
                after: after.to_string(),
                ..NotifyParams::default()
            },
        )
    }

    /// 학습형 블랙리스트에 막혀 ATF 교정을 건너뜀.
    pub fn atf_suppressed(context_id: u32, word: &str) -> Self {
        Self::new(
            NotifyKind::AtfSuppressed,
            context_id.to_string(),
            context_id,
            NotifyParams {
                word: word.to_string(),
                ..NotifyParams::default()
            },
        )
    }

    /// 롤백 패턴을 학습해 단어를 임시 제외 등록함.
    pub fn blacklist_learned(context_id: u32, word: &str, hours: u16) -> Self {
        Self::new(
            NotifyKind::BlacklistLearned,
            word.to_string(),
            context_id,
            NotifyParams {
                word: word.to_string(),
                hours,
                ..NotifyParams::default()
            },
        )
    }

    /// 비밀번호 칸 진입(비차단→차단 전이). `atf_on` 은 ATF 마스터 스위치 상태.
    pub fn password_enter(context_id: u32, atf_on: bool) -> Self {
        Self::new(
            NotifyKind::PasswordEnter,
            context_id.to_string(),
            context_id,
            NotifyParams {
                atf_on,
                ..NotifyParams::default()
            },
        )
    }

    /// 비밀번호 칸 이탈(차단→비차단 전이).
    pub fn password_leave(context_id: u32) -> Self {
        Self::new(
            NotifyKind::PasswordLeave,
            context_id.to_string(),
            context_id,
            NotifyParams::default(),
        )
    }

    /// 비밀번호 칸이라 한/영 전환이 막힘 (`InputEngine::last_toggle_blocked`).
    pub fn mode_toggle_suppressed(context_id: u32) -> Self {
        Self::new(
            NotifyKind::ModeToggleSuppressed,
            context_id.to_string(),
            context_id,
            NotifyParams::default(),
        )
    }

    /// 한/영 모드가 바뀜. 전역 변경(`SetGlobalMode`)은 `context_id=0`.
    pub fn mode_changed(context_id: u32, is_korean: bool) -> Self {
        Self::new(
            NotifyKind::ModeChanged,
            if is_korean { "ko" } else { "en" }.to_string(),
            context_id,
            NotifyParams {
                is_korean,
                ..NotifyParams::default()
            },
        )
    }

    /// ATF 토글 단축키로 기능이 켜지거나 꺼짐.
    pub fn feature_toggled(context_id: u32, feature: AtfToggleKind, value: bool) -> Self {
        Self::new(
            NotifyKind::FeatureToggled,
            format!("{}:{}", feature_tag(feature), u8::from(value)),
            context_id,
            NotifyParams {
                feature: Some(feature),
                value,
                ..NotifyParams::default()
            },
        )
    }

    /// 규칙 1 에서 실제로 비교하는 키.
    ///
    /// 가림(`show_text=false`) 상태의 ATF 교정 알림은 문구가 모두 같으므로 키를 `""` 로
    /// 접어 3 초 안의 연속 교정이 한 번만 나가게 한다. 표시 상태는 `before` 로 구분한다(§2.1).
    pub fn dedupe_key(&self, show_text: bool) -> &str {
        match self.kind {
            NotifyKind::AtfCorrectedForward | NotifyKind::AtfCorrectedReverse if !show_text => "",
            _ => &self.key,
        }
    }
}

fn feature_tag(feature: AtfToggleKind) -> &'static str {
    match feature {
        AtfToggleKind::Enabled => "enabled",
        AtfToggleKind::Forward => "forward",
        AtfToggleKind::Reverse => "reverse",
    }
}

/// 알림 중복 억제·필터 게이트 (§2.2 규칙 1·2·4·5·6, §2.3 대책 3).
///
/// 엔진 워커 스레드가 단독 소유한다(`&mut self`). 시각은 호출자가 `now` 로 넘긴다 —
/// 테스트에서 경계값(599/600 s)을 결정적으로 다루기 위해서다.
#[derive(Default)]
pub struct NotifyGate {
    /// 규칙 1: `(kind, 실효 키)` → 마지막 통과 시각
    last_shown: HashMap<(NotifyKind, String), Instant>,
    /// 규칙 2: 이미 1회 낸 `(kind, context_id)`
    once_fired: HashSet<(NotifyKind, u32)>,
    /// 규칙 4: context_id → 마지막 비밀번호 진입 알림 시각 (`DestroyContext` 에서만 제거)
    password_announced: HashMap<u32, Instant>,
}

/// 맵 키(실효 키)에 입력 단어가 들어갈 수 있어 개수만 찍는다.
impl std::fmt::Debug for NotifyGate {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("NotifyGate")
            .field("last_shown", &self.last_shown.len())
            .field("once_fired", &self.once_fired.len())
            .field("password_announced", &self.password_announced.len())
            .finish()
    }
}

impl NotifyGate {
    pub fn new() -> Self {
        Self::default()
    }

    /// 이벤트 1건을 심사해 통과하면 그대로 돌려준다.
    ///
    /// `purpose` 는 해당 컨텍스트의 **현재** 콘텐츠 목적이다. 판정 순서:
    /// 1. `cfg.enabled=false` → 폐기(기록도 안 함).
    /// 2. `password_*` 전이 기록 — `events` 필터와 **무관하게** 수행(규칙 4).
    /// 3. 비밀번호/PIN 상태에서 텍스트를 담는 종류 폐기(§2.3 대책 3, `show_text` 무관).
    /// 4. `events` 필터(규칙 5), 컨텍스트당 1회(규칙 2), 3 초 중복(규칙 1).
    pub fn offer(
        &mut self,
        ev: NotifyEvent,
        purpose: ContentPurpose,
        cfg: &NotifyConfig,
        now: Instant,
    ) -> Option<NotifyEvent> {
        if !cfg.enabled {
            return None;
        }

        // 규칙 4: 전이 기록은 events 필터보다 먼저. 필터는 발화만 막고 기록은 막지 않는다.
        match ev.kind {
            NotifyKind::PasswordEnter => {
                let fire = match self.password_announced.get(&ev.context_id) {
                    Some(t) => now.saturating_duration_since(*t) >= PASSWORD_REANNOUNCE,
                    None => true,
                };
                if !fire {
                    return None;
                }
                self.password_announced.insert(ev.context_id, now);
            }
            NotifyKind::PasswordLeave if !self.password_announced.contains_key(&ev.context_id) => {
                return None;
            }
            _ => {}
        }

        if ev.kind.carries_text() && purpose.should_block_hangul() {
            return None;
        }
        if !cfg.event_enabled(ev.kind.setting_name()) {
            return None;
        }
        if ev.kind.once_per_context() && self.once_fired.contains(&(ev.kind, ev.context_id)) {
            return None;
        }

        let key = (ev.kind, ev.dedupe_key(cfg.show_text).to_string());
        if let Some(t) = self.last_shown.get(&key) {
            if now.saturating_duration_since(*t) < DEDUPE_WINDOW {
                return None;
            }
        }

        // 통과 — 상태 기록.
        self.prune(now);
        self.last_shown.insert(key, now);
        if ev.kind.once_per_context() {
            self.once_fired.insert((ev.kind, ev.context_id));
        }
        Some(ev)
    }

    /// 한 요청(`ProcessKey` 1회)에서 모은 이벤트를 한꺼번에 심사한다 (규칙 6).
    ///
    /// ATF 교정이 통과하면 같은 묶음의 `ModeChanged`(ATF 유발 모드 전환)는 **심사 없이
    /// 제거**한다 — 교정 문구의 방향 표기가 모드 변화를 이미 알린다. 폐기된 `ModeChanged` 가
    /// 중복 억제 상태를 더럽히지 않도록 교정 판정을 먼저 끝낸 뒤 `ModeChanged` 를 심사한다.
    /// 출력 순서는 입력 순서를 따른다.
    pub fn offer_batch(
        &mut self,
        evs: Vec<NotifyEvent>,
        purpose: ContentPurpose,
        cfg: &NotifyConfig,
        now: Instant,
    ) -> Vec<NotifyEvent> {
        let mut slots: Vec<Option<NotifyEvent>> = Vec::with_capacity(evs.len());
        let mut deferred: Vec<(usize, NotifyEvent)> = Vec::new();
        for ev in evs {
            if ev.kind == NotifyKind::ModeChanged {
                deferred.push((slots.len(), ev));
                slots.push(None);
            } else {
                slots.push(self.offer(ev, purpose, cfg, now));
            }
        }
        let atf_passed = slots.iter().flatten().any(|e| {
            matches!(
                e.kind,
                NotifyKind::AtfCorrectedForward | NotifyKind::AtfCorrectedReverse
            )
        });
        if !atf_passed {
            for (idx, ev) in deferred {
                slots[idx] = self.offer(ev, purpose, cfg, now);
            }
        }
        slots.into_iter().flatten().collect()
    }

    /// FocusOut: 규칙 2(컨텍스트당 1회) 리셋. 규칙 4 맵은 건드리지 않는다.
    pub fn on_focus_out(&mut self, context_id: u32) {
        self.once_fired.retain(|(_, ctx)| *ctx != context_id);
    }

    /// 콘텐츠 목적 변경: 규칙 2 리셋.
    pub fn on_purpose_change(&mut self, context_id: u32) {
        self.on_focus_out(context_id);
    }

    /// `DestroyContext`: 컨텍스트의 모든 상태 제거(규칙 4 맵 포함).
    pub fn on_destroy(&mut self, context_id: u32) {
        self.on_focus_out(context_id);
        self.password_announced.remove(&context_id);
    }

    /// 만료된 중복 억제 기록을 정리한다 (무한 증가 방지).
    fn prune(&mut self, now: Instant) {
        if self.last_shown.len() >= 64 {
            self.last_shown
                .retain(|_, t| now.saturating_duration_since(*t) < DEDUPE_WINDOW);
        }
    }
}

/// 문구 언어.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Lang {
    Ko,
    En,
}

impl Lang {
    /// POSIX 로케일 우선순위 `LC_ALL` > `LC_MESSAGES` > `LANG` — 처음으로 비어 있지 않은
    /// 값이 `ko` 로 시작하면 한국어, 그 외·전부 비면 영어. 코어는 env 를 읽지 않으므로
    /// 호출자가 세 값을 넘긴다.
    pub fn detect(lc_all: Option<&str>, lc_messages: Option<&str>, lang: Option<&str>) -> Lang {
        [lc_all, lc_messages, lang]
            .into_iter()
            .flatten()
            .map(str::trim)
            .find(|v| !v.is_empty())
            .map_or(Lang::En, |v| {
                if v.to_ascii_lowercase().starts_with("ko") {
                    Lang::Ko
                } else {
                    Lang::En
                }
            })
    }

    /// `notify.language` 설정을 해석한다. `auto` 면 호출자가 판정한 `auto_lang` 을 쓴다.
    pub fn resolve(setting: NotifyLanguage, auto_lang: Lang) -> Lang {
        match setting {
            NotifyLanguage::Ko => Lang::Ko,
            NotifyLanguage::En => Lang::En,
            NotifyLanguage::Auto => auto_lang,
        }
    }
}

/// 표시용으로 완성된 알림 문구.
#[derive(Clone, PartialEq, Eq)]
pub struct Rendered {
    pub title: String,
    pub body: String,
}

/// 본문에 입력 텍스트가 실릴 수 있어 길이만 찍는다.
impl std::fmt::Debug for Rendered {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Rendered")
            .field("title_chars", &self.title.chars().count())
            .field("body_chars", &self.body.chars().count())
            .finish()
    }
}

/// 이벤트를 문구로 만든다 (§2.1 문구 표). 본문의 `<`·`>`·`&` 는 이스케이프한다
/// (fdo body-markup 서버 대비, §2.3). 이스케이프가 필요 없는 경로는 [`render_with`].
pub fn render(ev: &NotifyEvent, lang: Lang, show_text: bool) -> Rendered {
    render_with(ev, lang, show_text, true)
}

/// [`render`] 의 이스케이프 선택 버전 — `escape=false` 면 본문을 원문 그대로 둔다.
pub fn render_with(ev: &NotifyEvent, lang: Lang, show_text: bool, escape: bool) -> Rendered {
    let line = limit_chars(
        &compose(ev, lang, show_text),
        match lang {
            Lang::Ko => LINE_MAX_CHARS_KO,
            Lang::En => LINE_MAX_CHARS_EN,
        },
    );
    Rendered {
        title: NOTIFY_TITLE.to_string(),
        body: if escape { escape_markup(&line) } else { line },
    }
}

/// `<`·`>`·`&` 를 마크업 엔티티로 바꾼다.
pub fn escape_markup(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            other => out.push(other),
        }
    }
    out
}

/// 한 줄·최대 `max` 글자로 줄인다 (초과 시 마지막 글자를 `…` 로). 제어문자는 공백으로.
fn limit_chars(s: &str, max: usize) -> String {
    let flat: String = s
        .chars()
        .map(|c| if c.is_control() { ' ' } else { c })
        .collect();
    if flat.chars().count() <= max {
        return flat;
    }
    let mut out: String = flat.chars().take(max.saturating_sub(1)).collect();
    out.push('…');
    out
}

fn compose(ev: &NotifyEvent, lang: Lang, show_text: bool) -> String {
    let p = &ev.params;
    let ko = lang == Lang::Ko;
    // 텍스트 노출이 켜져 있어도 파라미터가 비어 있으면 가림 문구로 둔다.
    let before = limit_chars(&p.before, PARAM_MAX_CHARS);
    let after = limit_chars(&p.after, PARAM_MAX_CHARS);
    let word = limit_chars(&p.word, PARAM_MAX_CHARS);
    match ev.kind {
        NotifyKind::AtfCorrectedForward | NotifyKind::AtfCorrectedReverse => {
            if show_text && !p.before.is_empty() && !p.after.is_empty() {
                if ko {
                    format!("자동 교정: {before} → {after}")
                } else {
                    format!("Auto-fixed: {before} → {after}")
                }
            } else {
                let forward = ev.kind == NotifyKind::AtfCorrectedForward;
                match (ko, forward) {
                    (true, true) => "자동 교정됨 (영→한)".to_string(),
                    (true, false) => "자동 교정됨 (한→영)".to_string(),
                    (false, true) => "Auto-fixed (EN→KO)".to_string(),
                    (false, false) => "Auto-fixed (KO→EN)".to_string(),
                }
            }
        }
        NotifyKind::AtfSuppressed => match (show_text && !p.word.is_empty(), ko) {
            (true, true) => format!("자동 교정 건너뜀: '{word}'"),
            (true, false) => format!("Auto-fix skipped: '{word}'"),
            (false, true) => "자동 교정 건너뜀: 제외 단어".to_string(),
            (false, false) => "Auto-fix skipped: excluded word".to_string(),
        },
        NotifyKind::BlacklistLearned => {
            let h = p.hours;
            match (show_text && !p.word.is_empty(), ko) {
                (true, true) => format!("'{word}' 자동 교정 임시 제외 ({h}시간)"),
                (true, false) => format!("'{word}' auto-fix paused ({h}h)"),
                (false, true) => format!("자동 교정 임시 제외 ({h}시간)"),
                (false, false) => format!("Auto-fix paused for this word ({h}h)"),
            }
        }
        // 아래 세 종류는 고정 문자열만 — 파라미터 금지(§2.3).
        NotifyKind::PasswordEnter => match (p.atf_on, ko) {
            (true, true) => "비밀번호 칸: 영문 고정, 자동 교정 꺼짐".to_string(),
            (true, false) => "Password field: English locked, auto-fix off".to_string(),
            (false, true) => "비밀번호 칸: 영문 고정".to_string(),
            (false, false) => "Password field: English locked".to_string(),
        },
        NotifyKind::PasswordLeave => if ko {
            "비밀번호 칸 벗어남"
        } else {
            "Left password field"
        }
        .to_string(),
        NotifyKind::ModeToggleSuppressed => if ko {
            "비밀번호 칸: 한/영 전환 막힘"
        } else {
            "Password field: language toggle blocked"
        }
        .to_string(),
        NotifyKind::ModeChanged => match (p.is_korean, ko) {
            (true, true) => "한글",
            (false, true) => "영문",
            (true, false) => "Korean",
            (false, false) => "English",
        }
        .to_string(),
        NotifyKind::FeatureToggled => {
            let on = p.value;
            match (p.feature.unwrap_or(AtfToggleKind::Enabled), ko) {
                (AtfToggleKind::Enabled, true) => {
                    format!("자동 교정 {}", if on { "켜짐" } else { "꺼짐" })
                }
                (AtfToggleKind::Forward, true) => {
                    format!("자동 교정(영→한) {}", if on { "켜짐" } else { "꺼짐" })
                }
                (AtfToggleKind::Reverse, true) => {
                    format!("자동 교정(한→영) {}", if on { "켜짐" } else { "꺼짐" })
                }
                (AtfToggleKind::Enabled, false) => {
                    format!("Auto-fix {}", if on { "on" } else { "off" })
                }
                (AtfToggleKind::Forward, false) => {
                    format!("Auto-fix (EN→KO) {}", if on { "on" } else { "off" })
                }
                (AtfToggleKind::Reverse, false) => {
                    format!("Auto-fix (KO→EN) {}", if on { "on" } else { "off" })
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const NORMAL: ContentPurpose = ContentPurpose::Normal;

    fn cfg() -> NotifyConfig {
        NotifyConfig::default()
    }

    fn cfg_events(events: &[&str]) -> NotifyConfig {
        NotifyConfig {
            events: events.iter().map(|s| s.to_string()).collect(),
            ..NotifyConfig::default()
        }
    }

    fn secs(base: Instant, s: u64) -> Instant {
        base + Duration::from_secs(s)
    }

    fn corrected(ctx: u32, before: &str) -> NotifyEvent {
        NotifyEvent::atf_corrected(Direction::Forward, ctx, before, "한글")
    }

    // ── 규칙 1: 3000ms 중복 억제 ──────────────────────────────

    #[test]
    fn rule1_same_kind_and_key_within_3s_dropped() {
        let t0 = Instant::now();
        let mut g = NotifyGate::new();
        let c = NotifyConfig {
            show_text: true,
            ..cfg()
        };
        assert!(g.offer(corrected(1, "gksrmf"), NORMAL, &c, t0).is_some());
        let near = t0 + Duration::from_millis(2999);
        assert!(g.offer(corrected(1, "gksrmf"), NORMAL, &c, near).is_none());
        // 경계: 정확히 3000ms 면 통과.
        let edge = t0 + Duration::from_millis(3000);
        assert!(g.offer(corrected(1, "gksrmf"), NORMAL, &c, edge).is_some());
    }

    #[test]
    fn rule1_different_key_or_kind_not_deduped() {
        let t0 = Instant::now();
        let mut g = NotifyGate::new();
        let c = NotifyConfig {
            show_text: true,
            ..cfg()
        };
        assert!(g.offer(corrected(1, "gksrmf"), NORMAL, &c, t0).is_some());
        assert!(g.offer(corrected(1, "rkskek"), NORMAL, &c, t0).is_some());
        let rev = NotifyEvent::atf_corrected(Direction::Reverse, 1, "gksrmf", "hello");
        assert!(g.offer(rev, NORMAL, &c, t0).is_some());
    }

    #[test]
    fn rule1_masked_corrections_collapse_to_one_key() {
        // show_text=false 면 키가 "" 로 접혀 다른 단어도 3초 안에는 1회만.
        let t0 = Instant::now();
        let mut g = NotifyGate::new();
        let c = cfg();
        assert!(g.offer(corrected(1, "aaa"), NORMAL, &c, t0).is_some());
        assert!(g
            .offer(corrected(2, "bbb"), NORMAL, &c, secs(t0, 1))
            .is_none());
        assert!(g
            .offer(corrected(2, "bbb"), NORMAL, &c, secs(t0, 3))
            .is_some());
    }

    #[test]
    fn rule1_dropped_event_does_not_extend_window() {
        // 폐기된 재발은 시각을 갱신하지 않는다 (t0+2s 폐기 후 t0+3s 에 통과).
        let t0 = Instant::now();
        let mut g = NotifyGate::new();
        let c = cfg();
        assert!(g.offer(corrected(1, "a"), NORMAL, &c, t0).is_some());
        assert!(g
            .offer(corrected(1, "a"), NORMAL, &c, secs(t0, 2))
            .is_none());
        assert!(g
            .offer(corrected(1, "a"), NORMAL, &c, secs(t0, 3))
            .is_some());
    }

    // ── 규칙 2: context_id 당 1회 ─────────────────────────────

    #[test]
    fn rule2_suppressed_once_per_context_until_focus_out() {
        let t0 = Instant::now();
        let mut g = NotifyGate::new();
        let c = cfg();
        assert!(g
            .offer(NotifyEvent::atf_suppressed(7, "abc"), NORMAL, &c, t0)
            .is_some());
        // 3초가 지나도 같은 칸은 폐기.
        assert!(g
            .offer(
                NotifyEvent::atf_suppressed(7, "abc"),
                NORMAL,
                &c,
                secs(t0, 10)
            )
            .is_none());
        // 다른 칸은 통과.
        assert!(g
            .offer(
                NotifyEvent::atf_suppressed(8, "abc"),
                NORMAL,
                &c,
                secs(t0, 10)
            )
            .is_some());
        // FocusOut 후 재허용.
        g.on_focus_out(7);
        assert!(g
            .offer(
                NotifyEvent::atf_suppressed(7, "abc"),
                NORMAL,
                &c,
                secs(t0, 20)
            )
            .is_some());
    }

    #[test]
    fn rule2_toggle_suppressed_reset_by_purpose_change() {
        let t0 = Instant::now();
        let mut g = NotifyGate::new();
        let c = cfg();
        let ev = || NotifyEvent::mode_toggle_suppressed(3);
        assert!(g.offer(ev(), NORMAL, &c, t0).is_some());
        assert!(g.offer(ev(), NORMAL, &c, secs(t0, 10)).is_none());
        g.on_purpose_change(3);
        assert!(g.offer(ev(), NORMAL, &c, secs(t0, 20)).is_some());
        // 파괴도 리셋.
        assert!(g.offer(ev(), NORMAL, &c, secs(t0, 30)).is_none());
        g.on_destroy(3);
        assert!(g.offer(ev(), NORMAL, &c, secs(t0, 40)).is_some());
    }

    // ── 규칙 4: 비밀번호 진입 칸당 1회 + 10분 재알림 ──────────

    #[test]
    fn rule4_password_enter_once_then_reannounce_after_600s() {
        let t0 = Instant::now();
        let mut g = NotifyGate::new();
        let c = cfg();
        let ev = || NotifyEvent::password_enter(5, true);
        assert!(g.offer(ev(), ContentPurpose::Password, &c, t0).is_some());
        // 경계값: 599s 폐기 / 600s 통과.
        assert!(g
            .offer(ev(), ContentPurpose::Password, &c, secs(t0, 599))
            .is_none());
        assert!(g
            .offer(ev(), ContentPurpose::Password, &c, secs(t0, 600))
            .is_some());
        // 600s 에 갱신됐으므로 다시 599s 뒤(1199s)는 폐기, 1200s 는 통과.
        assert!(g
            .offer(ev(), ContentPurpose::Password, &c, secs(t0, 1199))
            .is_none());
        assert!(g
            .offer(ev(), ContentPurpose::Password, &c, secs(t0, 1200))
            .is_some());
    }

    #[test]
    fn rule4_password_enter_is_per_context() {
        let t0 = Instant::now();
        let mut g = NotifyGate::new();
        let c = cfg();
        assert!(g
            .offer(
                NotifyEvent::password_enter(1, true),
                ContentPurpose::Password,
                &c,
                t0
            )
            .is_some());
        assert!(g
            .offer(
                NotifyEvent::password_enter(2, true),
                ContentPurpose::Password,
                &c,
                t0
            )
            .is_some());
    }

    #[test]
    fn rule4_destroy_clears_password_record() {
        let t0 = Instant::now();
        let mut g = NotifyGate::new();
        let c = cfg();
        let ev = || NotifyEvent::password_enter(5, false);
        assert!(g.offer(ev(), ContentPurpose::Password, &c, t0).is_some());
        // FocusOut·목적 변경으로는 리셋되지 않는다.
        g.on_focus_out(5);
        g.on_purpose_change(5);
        assert!(g
            .offer(ev(), ContentPurpose::Password, &c, secs(t0, 10))
            .is_none());
        g.on_destroy(5);
        assert!(g
            .offer(ev(), ContentPurpose::Password, &c, secs(t0, 11))
            .is_some());
    }

    #[test]
    fn rule4_leave_only_for_recorded_context() {
        let t0 = Instant::now();
        let mut g = NotifyGate::new();
        let c = cfg_events(&["password_enter", "password_leave"]);
        // 기록 없는 칸의 leave 는 불발.
        assert!(g
            .offer(NotifyEvent::password_leave(9), NORMAL, &c, t0)
            .is_none());
        assert!(g
            .offer(
                NotifyEvent::password_enter(9, true),
                ContentPurpose::Password,
                &c,
                t0
            )
            .is_some());
        assert!(g
            .offer(NotifyEvent::password_leave(9), NORMAL, &c, secs(t0, 5))
            .is_some());
    }

    #[test]
    fn rule4_transition_recorded_even_if_enter_filtered_out() {
        // password_enter 를 events 에서 뺀 사용자가 password_leave 만 켠 경우.
        let t0 = Instant::now();
        let mut g = NotifyGate::new();
        let c = cfg_events(&["password_leave"]);
        assert!(g
            .offer(
                NotifyEvent::password_enter(4, true),
                ContentPurpose::Password,
                &c,
                t0
            )
            .is_none());
        assert!(g
            .offer(NotifyEvent::password_leave(4), NORMAL, &c, secs(t0, 5))
            .is_some());
    }

    #[test]
    fn rule4_disabled_gate_records_nothing() {
        let t0 = Instant::now();
        let mut g = NotifyGate::new();
        let off = NotifyConfig {
            enabled: false,
            ..cfg_events(&["password_enter", "password_leave"])
        };
        assert!(g
            .offer(
                NotifyEvent::password_enter(4, true),
                ContentPurpose::Password,
                &off,
                t0
            )
            .is_none());
        // 켜진 뒤 leave: 기록이 없었으므로 불발.
        let on = cfg_events(&["password_enter", "password_leave"]);
        assert!(g
            .offer(NotifyEvent::password_leave(4), NORMAL, &on, secs(t0, 5))
            .is_none());
        // 진입도 첫 알림으로 취급된다 (disabled 중 기록 안 됨).
        assert!(g
            .offer(
                NotifyEvent::password_enter(4, true),
                ContentPurpose::Password,
                &on,
                secs(t0, 6)
            )
            .is_some());
    }

    // ── 규칙 5: enabled / events 필터 ─────────────────────────

    #[test]
    fn rule5_enabled_false_drops_everything() {
        let t0 = Instant::now();
        let mut g = NotifyGate::new();
        let off = NotifyConfig {
            enabled: false,
            ..cfg()
        };
        assert!(g.offer(corrected(1, "a"), NORMAL, &off, t0).is_none());
        assert!(g
            .offer(NotifyEvent::mode_toggle_suppressed(1), NORMAL, &off, t0)
            .is_none());
    }

    #[test]
    fn rule5_event_not_in_list_dropped_and_atf_corrected_covers_both_directions() {
        let t0 = Instant::now();
        let mut g = NotifyGate::new();
        let only_suppressed = cfg_events(&["atf_suppressed"]);
        assert!(g
            .offer(corrected(1, "a"), NORMAL, &only_suppressed, t0)
            .is_none());
        assert!(g
            .offer(
                NotifyEvent::atf_suppressed(1, "a"),
                NORMAL,
                &only_suppressed,
                t0
            )
            .is_some());

        let only_corrected = cfg_events(&["atf_corrected"]);
        let rev = NotifyEvent::atf_corrected(Direction::Reverse, 1, "", "hello");
        assert!(g.offer(rev, NORMAL, &only_corrected, t0).is_some());
        assert!(g
            .offer(corrected(1, "a"), NORMAL, &only_corrected, t0)
            .is_some());
        // 미지 설정명은 무시, 기본 꺼짐 이벤트(mode_changed)는 기본 목록에서 폐기.
        assert!(g
            .offer(NotifyEvent::mode_changed(0, true), NORMAL, &cfg(), t0)
            .is_none());
    }

    // ── 규칙 6: ATF 유발 mode_changed 생략 ────────────────────

    #[test]
    fn rule6_atf_correction_suppresses_mode_changed_in_batch() {
        let t0 = Instant::now();
        let mut g = NotifyGate::new();
        let c = cfg_events(&["atf_corrected", "mode_changed"]);
        let out = g.offer_batch(
            vec![NotifyEvent::mode_changed(1, true), corrected(1, "gksrmf")],
            NORMAL,
            &c,
            t0,
        );
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].kind, NotifyKind::AtfCorrectedForward);
        // 폐기된 mode_changed 는 중복 억제 기록을 남기지 않는다 → 곧바로 단독 발화 가능.
        let again = g.offer(NotifyEvent::mode_changed(0, true), NORMAL, &c, t0);
        assert!(again.is_some());
    }

    #[test]
    fn rule6_mode_changed_passes_when_atf_corrected_is_off() {
        let t0 = Instant::now();
        let mut g = NotifyGate::new();
        let c = cfg_events(&["mode_changed"]);
        let out = g.offer_batch(
            vec![corrected(1, "gksrmf"), NotifyEvent::mode_changed(1, true)],
            NORMAL,
            &c,
            t0,
        );
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].kind, NotifyKind::ModeChanged);
    }

    #[test]
    fn rule6_atf_filtered_by_gate_does_not_suppress_mode_changed() {
        // 교정이 3초 중복으로 폐기됐다면 "통과"가 아니므로 mode_changed 가 나간다.
        let t0 = Instant::now();
        let mut g = NotifyGate::new();
        let c = cfg_events(&["atf_corrected", "mode_changed"]);
        assert!(g.offer(corrected(1, "a"), NORMAL, &c, t0).is_some());
        let out = g.offer_batch(
            vec![corrected(1, "a"), NotifyEvent::mode_changed(1, true)],
            NORMAL,
            &c,
            secs(t0, 1),
        );
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].kind, NotifyKind::ModeChanged);
    }

    #[test]
    fn batch_preserves_input_order_and_other_kinds() {
        let t0 = Instant::now();
        let mut g = NotifyGate::new();
        let c = cfg_events(&["atf_suppressed", "feature_toggled", "mode_changed"]);
        let out = g.offer_batch(
            vec![
                NotifyEvent::feature_toggled(1, AtfToggleKind::Enabled, false),
                NotifyEvent::mode_changed(1, false),
                NotifyEvent::atf_suppressed(1, "x"),
            ],
            NORMAL,
            &c,
            t0,
        );
        let kinds: Vec<_> = out.iter().map(|e| e.kind).collect();
        assert_eq!(
            kinds,
            vec![
                NotifyKind::FeatureToggled,
                NotifyKind::ModeChanged,
                NotifyKind::AtfSuppressed
            ]
        );
    }

    // ── §2.3 민감정보 ─────────────────────────────────────────

    #[test]
    fn reverse_with_empty_before_falls_back_to_masked_text_even_with_show_text() {
        let ev = NotifyEvent::atf_corrected(Direction::Reverse, 1, "", "hello");
        let r = render(&ev, Lang::Ko, true);
        assert_eq!(r.body, "자동 교정됨 (한→영)");
        let r = render(&ev, Lang::En, true);
        assert_eq!(r.body, "Auto-fixed (KO→EN)");
        // 둘 다 있으면 전후 텍스트를 보인다.
        let ev = NotifyEvent::atf_corrected(Direction::Reverse, 1, "gksrmf", "hello");
        assert_eq!(render(&ev, Lang::Ko, true).body, "자동 교정: gksrmf → hello");
    }

    #[test]
    fn debug_output_redacts_input_text() {
        let ev = NotifyEvent::atf_corrected(Direction::Forward, 1, "SECRETBEFORE", "SECRETAFTER");
        let bl = NotifyEvent::blacklist_learned(1, "SECRETWORD", 2);
        let r = render(&ev, Lang::En, true);
        let mut g = NotifyGate::new();
        let c = NotifyConfig {
            show_text: true,
            ..cfg()
        };
        g.offer(ev.clone(), NORMAL, &c, Instant::now());
        for d in [
            format!("{ev:?}"),
            format!("{bl:?}"),
            format!("{:?}", ev.params),
            format!("{r:?}"),
            format!("{g:?}"),
        ] {
            assert!(!d.contains("SECRET"), "{d}");
        }
    }

    #[test]
    fn password_and_pin_state_discards_text_kinds_regardless_of_show_text() {
        let t0 = Instant::now();
        let c = NotifyConfig {
            show_text: true,
            events: crate::config::NOTIFY_EVENT_NAMES
                .iter()
                .map(|s| s.to_string())
                .collect(),
            ..cfg()
        };
        for purpose in [ContentPurpose::Password, ContentPurpose::Pin] {
            let mut g = NotifyGate::new();
            assert!(g.offer(corrected(1, "pw"), purpose, &c, t0).is_none());
            let rev = NotifyEvent::atf_corrected(Direction::Reverse, 1, "", "pw");
            assert!(g.offer(rev, purpose, &c, t0).is_none());
            assert!(g
                .offer(NotifyEvent::atf_suppressed(1, "pw"), purpose, &c, t0)
                .is_none());
            assert!(g
                .offer(NotifyEvent::blacklist_learned(1, "pw", 4), purpose, &c, t0)
                .is_none());
            // 고정 문구 종류는 통과한다.
            assert!(g
                .offer(NotifyEvent::mode_toggle_suppressed(1), purpose, &c, t0)
                .is_some());
        }
        // Normal 이면 같은 이벤트가 통과 — 폐기가 목적 때문임을 확인.
        let mut g = NotifyGate::new();
        assert!(g.offer(corrected(1, "pw"), NORMAL, &c, t0).is_some());
    }

    #[test]
    fn masked_text_excludes_before_after_and_word() {
        let evs = [
            NotifyEvent::atf_corrected(Direction::Forward, 1, "SECRETBEFORE", "SECRETAFTER"),
            NotifyEvent::atf_corrected(Direction::Reverse, 1, "SECRETBEFORE", "SECRETAFTER"),
            NotifyEvent::atf_suppressed(1, "SECRETWORD"),
            NotifyEvent::blacklist_learned(1, "SECRETWORD", 4),
        ];
        for ev in &evs {
            for lang in [Lang::Ko, Lang::En] {
                let r = render(ev, lang, false);
                assert!(
                    !r.body.contains("SECRET"),
                    "{:?} {:?}: {}",
                    ev.kind,
                    lang,
                    r.body
                );
            }
        }
    }

    #[test]
    fn password_texts_are_fixed_strings() {
        // 파라미터가 무엇이든 고정 문구.
        let mut ev = NotifyEvent::password_enter(1, true);
        ev.params.before = "SECRET".into();
        ev.params.word = "SECRET".into();
        for show in [false, true] {
            assert!(!render(&ev, Lang::Ko, show).body.contains("SECRET"));
        }
    }

    // ── 문구 ──────────────────────────────────────────────────

    #[test]
    fn render_masked_texts_match_spec() {
        let f = NotifyEvent::atf_corrected(Direction::Forward, 1, "a", "b");
        let r = NotifyEvent::atf_corrected(Direction::Reverse, 1, "a", "b");
        assert_eq!(render(&f, Lang::Ko, false).body, "자동 교정됨 (영→한)");
        assert_eq!(render(&r, Lang::Ko, false).body, "자동 교정됨 (한→영)");
        assert_eq!(render(&f, Lang::En, false).body, "Auto-fixed (EN→KO)");
        assert_eq!(render(&r, Lang::En, false).body, "Auto-fixed (KO→EN)");
        assert_eq!(render(&f, Lang::Ko, false).title, "UNIM");
        assert_eq!(
            render(&NotifyEvent::atf_suppressed(1, "w"), Lang::Ko, false).body,
            "자동 교정 건너뜀: 제외 단어"
        );
        assert_eq!(
            render(&NotifyEvent::atf_suppressed(1, "w"), Lang::En, false).body,
            "Auto-fix skipped: excluded word"
        );
        assert_eq!(
            render(&NotifyEvent::blacklist_learned(1, "w", 4), Lang::Ko, false).body,
            "자동 교정 임시 제외 (4시간)"
        );
        assert_eq!(
            render(&NotifyEvent::blacklist_learned(1, "w", 4), Lang::En, false).body,
            "Auto-fix paused for this word (4h)"
        );
    }

    #[test]
    fn render_shown_texts_match_spec() {
        let f = NotifyEvent::atf_corrected(Direction::Forward, 1, "gksrmf", "한글");
        assert_eq!(render(&f, Lang::Ko, true).body, "자동 교정: gksrmf → 한글");
        assert_eq!(render(&f, Lang::En, true).body, "Auto-fixed: gksrmf → 한글");
        assert_eq!(
            render(&NotifyEvent::atf_suppressed(1, "hello"), Lang::Ko, true).body,
            "자동 교정 건너뜀: 'hello'"
        );
        assert_eq!(
            render(
                &NotifyEvent::blacklist_learned(1, "hello", 4),
                Lang::Ko,
                true
            )
            .body,
            "'hello' 자동 교정 임시 제외 (4시간)"
        );
        assert_eq!(
            render(
                &NotifyEvent::blacklist_learned(1, "hello", 4),
                Lang::En,
                true
            )
            .body,
            "'hello' auto-fix paused (4h)"
        );
        // 파라미터가 비면 show_text 여도 가림 문구.
        let empty = NotifyEvent::atf_suppressed(1, "");
        assert_eq!(
            render(&empty, Lang::Ko, true).body,
            "자동 교정 건너뜀: 제외 단어"
        );
    }

    #[test]
    fn render_fixed_and_state_texts() {
        let t = |ev: NotifyEvent, lang| render(&ev, lang, false).body;
        assert_eq!(
            t(NotifyEvent::password_enter(1, true), Lang::Ko),
            "비밀번호 칸: 영문 고정, 자동 교정 꺼짐"
        );
        assert_eq!(
            t(NotifyEvent::password_enter(1, false), Lang::Ko),
            "비밀번호 칸: 영문 고정"
        );
        assert_eq!(
            t(NotifyEvent::password_enter(1, true), Lang::En),
            "Password field: English locked, auto-fix off"
        );
        assert_eq!(
            t(NotifyEvent::password_enter(1, false), Lang::En),
            "Password field: English locked"
        );
        assert_eq!(
            t(NotifyEvent::password_leave(1), Lang::Ko),
            "비밀번호 칸 벗어남"
        );
        assert_eq!(
            t(NotifyEvent::password_leave(1), Lang::En),
            "Left password field"
        );
        assert_eq!(
            t(NotifyEvent::mode_toggle_suppressed(1), Lang::Ko),
            "비밀번호 칸: 한/영 전환 막힘"
        );
        assert_eq!(
            t(NotifyEvent::mode_toggle_suppressed(1), Lang::En),
            "Password field: language toggle blocked"
        );
        assert_eq!(t(NotifyEvent::mode_changed(0, true), Lang::Ko), "한글");
        assert_eq!(t(NotifyEvent::mode_changed(0, false), Lang::Ko), "영문");
        assert_eq!(t(NotifyEvent::mode_changed(0, true), Lang::En), "Korean");
        assert_eq!(t(NotifyEvent::mode_changed(0, false), Lang::En), "English");
        let ft = |k, v| NotifyEvent::feature_toggled(1, k, v);
        assert_eq!(
            t(ft(AtfToggleKind::Enabled, true), Lang::Ko),
            "자동 교정 켜짐"
        );
        assert_eq!(
            t(ft(AtfToggleKind::Enabled, false), Lang::Ko),
            "자동 교정 꺼짐"
        );
        assert_eq!(
            t(ft(AtfToggleKind::Forward, true), Lang::Ko),
            "자동 교정(영→한) 켜짐"
        );
        assert_eq!(
            t(ft(AtfToggleKind::Reverse, false), Lang::Ko),
            "자동 교정(한→영) 꺼짐"
        );
        assert_eq!(t(ft(AtfToggleKind::Enabled, true), Lang::En), "Auto-fix on");
        assert_eq!(
            t(ft(AtfToggleKind::Forward, false), Lang::En),
            "Auto-fix (EN→KO) off"
        );
    }

    #[test]
    fn render_param_truncation_and_line_cap() {
        let long = "x".repeat(40);
        let ev = NotifyEvent::atf_corrected(Direction::Forward, 1, &long, &long);
        for (lang, cap) in [(Lang::Ko, 40), (Lang::En, 60)] {
            let r = render(&ev, lang, true);
            assert!(r.body.chars().count() <= cap, "{lang:?}: {}", r.body);
            assert!(r.body.contains('…'), "{}", r.body);
        }
        // 파라미터는 16자 이내(…포함)로 줄고, 줄이 허용 한도 안이면 양쪽이 모두 보인다.
        let ev = NotifyEvent::atf_suppressed(1, &"w".repeat(30));
        let r = render(&ev, Lang::En, true);
        assert_eq!(r.body, format!("Auto-fix skipped: '{}…'", "w".repeat(15)));
    }

    #[test]
    fn all_rendered_lengths_within_caps() {
        let evs = vec![
            NotifyEvent::atf_corrected(Direction::Forward, 1, "a", "b"),
            NotifyEvent::atf_corrected(Direction::Reverse, 1, "a", "b"),
            NotifyEvent::atf_suppressed(1, "w"),
            NotifyEvent::blacklist_learned(1, "w", 12),
            NotifyEvent::password_enter(1, true),
            NotifyEvent::password_enter(1, false),
            NotifyEvent::password_leave(1),
            NotifyEvent::mode_toggle_suppressed(1),
            NotifyEvent::mode_changed(0, true),
            NotifyEvent::feature_toggled(1, AtfToggleKind::Forward, true),
            NotifyEvent::feature_toggled(1, AtfToggleKind::Reverse, false),
        ];
        for ev in &evs {
            for show in [false, true] {
                assert!(render(ev, Lang::Ko, show).body.chars().count() <= 40);
                assert!(render(ev, Lang::En, show).body.chars().count() <= 60);
            }
        }
    }

    #[test]
    fn render_escapes_markup_and_flattens_control_chars() {
        let ev = NotifyEvent::atf_corrected(Direction::Forward, 1, "<b>&", "a\nb");
        let r = render(&ev, Lang::En, true);
        assert_eq!(r.body, "Auto-fixed: &lt;b&gt;&amp; → a b");
        let raw = render_with(&ev, Lang::En, true, false);
        assert_eq!(raw.body, "Auto-fixed: <b>& → a b");
    }

    // ── Lang ──────────────────────────────────────────────────

    #[test]
    fn lang_detect_posix_priority() {
        // LC_ALL 이 최우선.
        assert_eq!(
            Lang::detect(
                Some("en_US.UTF-8"),
                Some("ko_KR.UTF-8"),
                Some("ko_KR.UTF-8")
            ),
            Lang::En
        );
        // 비어 있는 값은 건너뛴다.
        assert_eq!(
            Lang::detect(Some(""), Some("ko_KR.UTF-8"), Some("en_US.UTF-8")),
            Lang::Ko
        );
        assert_eq!(Lang::detect(None, None, Some("ko_KR.UTF-8")), Lang::Ko);
        assert_eq!(Lang::detect(None, Some("en_US"), Some("ko_KR")), Lang::En);
        // 전부 비면 영어, ko 접두가 아니면 영어.
        assert_eq!(Lang::detect(None, None, None), Lang::En);
        assert_eq!(Lang::detect(Some(""), Some("  "), Some("")), Lang::En);
        assert_eq!(Lang::detect(None, None, Some("C")), Lang::En);
        assert_eq!(Lang::detect(None, None, Some("KO_kr")), Lang::Ko);
    }

    #[test]
    fn lang_resolve_setting() {
        assert_eq!(Lang::resolve(NotifyLanguage::Ko, Lang::En), Lang::Ko);
        assert_eq!(Lang::resolve(NotifyLanguage::En, Lang::Ko), Lang::En);
        assert_eq!(Lang::resolve(NotifyLanguage::Auto, Lang::Ko), Lang::Ko);
        assert_eq!(Lang::resolve(NotifyLanguage::Auto, Lang::En), Lang::En);
    }

    // ── kind 상수 ─────────────────────────────────────────────

    #[test]
    fn kind_strings_and_setting_names() {
        assert_eq!(
            NotifyKind::AtfCorrectedForward.as_str(),
            "atf_corrected_forward"
        );
        assert_eq!(
            NotifyKind::AtfCorrectedReverse.as_str(),
            "atf_corrected_reverse"
        );
        assert_eq!(
            NotifyKind::AtfCorrectedForward.setting_name(),
            "atf_corrected"
        );
        assert_eq!(
            NotifyKind::AtfCorrectedReverse.setting_name(),
            "atf_corrected"
        );
        for k in [
            NotifyKind::AtfSuppressed,
            NotifyKind::BlacklistLearned,
            NotifyKind::PasswordEnter,
            NotifyKind::PasswordLeave,
            NotifyKind::ModeToggleSuppressed,
            NotifyKind::ModeChanged,
            NotifyKind::FeatureToggled,
        ] {
            assert_eq!(k.setting_name(), k.as_str());
            assert!(crate::config::NOTIFY_EVENT_NAMES.contains(&k.setting_name()));
        }
    }

    #[test]
    fn dedupe_key_masks_corrections_only_when_hidden() {
        let ev = corrected(1, "gksrmf");
        assert_eq!(ev.dedupe_key(false), "");
        assert_eq!(ev.dedupe_key(true), "gksrmf");
        assert_eq!(
            NotifyEvent::blacklist_learned(1, "w", 4).dedupe_key(false),
            "w"
        );
        assert_eq!(
            NotifyEvent::feature_toggled(1, AtfToggleKind::Reverse, true).key,
            "reverse:1"
        );
        assert_eq!(NotifyEvent::mode_changed(0, false).key, "en");
    }
}
