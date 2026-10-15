//! `ReplaceSurroundingText` 교체 검증 — 관측 전용(동작 변경 없음).
//!
//! 대상③(커서가 걸친 단어) 확정은 프런트가 `delete_surrounding(앞 a, 뒤 b)` + `commit` 으로
//! 문서를 고친다. 이때 프런트/앱마다 삭제 **단위**(문자 vs 바이트)가 다를 수 있다
//! (HANJA_WORD_SPEC §6.5 (10) — mutter `delete_surrounding` 실측). 이 모듈은 시그널 발행 직전의
//! surrounding 과 발행 뒤 처음 도착한 `SetSurroundingText` 를 코드포인트 단위로 비교해
//! 실제 삭제·삽입량을 구하고 로그 한 줄(`[REPLACE_VERIFY] …`)로 판정을 남긴다.
//!
//! 프라이버시(HANJA_WORD_SPEC §2.8): 보관하는 텍스트는 **메모리에만** 두고, 결과가 오거나 취소(FocusIn/Out·파괴)되면 즉시, 아니면 3초 지난 뒤
//! **다음 엔진 요청이 들어올 때** 청산한다(별도 타이머 없음 — 요청이 없으면 그 사이 메모리에 남는다).
//! 로그에는 컨텍스트 번호·프런트 이름·길이·판정만 남기며 텍스트 평문은 절대 남기지 않는다.
//!
//! 판정 로직([`judge`])은 순수 함수라 시간·IO 없이 단위 시험한다.

use std::collections::HashMap;
use std::time::{Duration, Instant};

/// 발행 뒤 이 시간 안에 도착한 첫 `SetSurroundingText` 만 비교한다.
pub const VERIFY_WINDOW: Duration = Duration::from_secs(3);

/// 판정.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Verdict {
    /// 기대한 그대로 교체됨(문서가 `앞 − a + commit + 뒤 − b` 와 정확히 일치).
    Ok,
    /// 기대보다 더 많이 지웠다(예: 문자 a 개가 아니라 바이트 a×3 을 지움).
    Over,
    /// 기대보다 적게 지웠다.
    Under,
    /// 삭제량은 맞거나 어긋났으나 삽입 길이·내용이 기대와 다르다(커밋 누락 등).
    Mismatch,
    /// 판정 불가 — surrounding 창이 이동했거나 관련 없는 변경이다(접두·접미가 거의 없음).
    Unknown,
    /// 3초 안에 다음 `SetSurroundingText` 가 오지 않았다.
    Timeout,
}

impl Verdict {
    pub fn as_str(self) -> &'static str {
        match self {
            Verdict::Ok => "ok",
            Verdict::Over => "over",
            Verdict::Under => "under",
            Verdict::Mismatch => "mismatch",
            Verdict::Unknown => "unknown",
            Verdict::Timeout => "timeout",
        }
    }
}

/// 문서 비교로 구한 실제 변화량(코드포인트). `before`/`after` 는 커서 기준 삭제 길이.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Observed {
    pub before: u32,
    pub after: u32,
    pub ins: u32,
}

/// 공통 접두 길이.
fn common_prefix(a: &[char], b: &[char]) -> usize {
    a.iter().zip(b.iter()).take_while(|(x, y)| x == y).count()
}

/// 접두와 겹치지 않는 범위에서의 공통 접미 길이.
fn common_suffix(a: &[char], b: &[char], prefix: usize) -> usize {
    let max = a.len().min(b.len()).saturating_sub(prefix);
    a.iter()
        .rev()
        .zip(b.iter().rev())
        .take(max)
        .take_while(|(x, y)| x == y)
        .count()
}

/// 교체 결과 판정 — 순수 함수.
///
/// - `old`: 발행 직전 surrounding, `cursor`: 그 커서(코드포인트 오프셋)
/// - `a`/`b`: 기대 앞/뒤 삭제 길이, `commit`: 삽입 텍스트
/// - `new`: 발행 뒤 처음 도착한 surrounding
///
/// 반환 `(관측값, 판정)`. 비교 불가(`Unknown` 중 관측값을 못 구한 경우)면 관측값이 `None`.
pub fn judge(
    old: &[char],
    cursor: usize,
    a: usize,
    b: usize,
    commit: &[char],
    new: &[char],
) -> (Option<Observed>, Verdict) {
    let lo = old.len();
    let ln = new.len();
    if lo == 0 || ln == 0 {
        return (None, Verdict::Unknown);
    }
    let c = cursor.min(lo);

    // 1) 기대한 문서와 정확히 일치하면 ok.
    let start = c.saturating_sub(a);
    let end = (c + b).min(lo);
    let mut expected: Vec<char> = Vec::with_capacity(start + commit.len() + (lo - end));
    expected.extend_from_slice(&old[..start]);
    expected.extend_from_slice(commit);
    expected.extend_from_slice(&old[end..]);
    if expected.as_slice() == new {
        return (
            Some(Observed {
                before: (c - start) as u32,
                after: (end - c) as u32,
                ins: commit.len() as u32,
            }),
            Verdict::Ok,
        );
    }

    // 2) 공통 접두·접미로 실제 삭제·삽입 구간을 구한다.
    let p = common_prefix(old, new);
    let s = common_suffix(old, new, p);
    let e = lo - s; // old 에서 지워진 구간 [p, e)
    let ie = ln - s; // new 에서 삽입된 구간 [p, ie)
    let ins = ie - p;
    // 지워진 구간이 커서에 닿아 있어야(p ≤ c ≤ e) 이 교체의 결과로 볼 수 있다.
    let touches = p <= c && c <= e;
    let before = if p < c { c.min(e) - p } else { 0 };
    let after = e.saturating_sub(c.max(p));
    let observed = Observed {
        before: before as u32,
        after: after as u32,
        ins: ins as u32,
    };

    // 접두·접미가 전혀 없고 삽입 길이도 commit 과 다르면 창이 이동했거나 무관한 변경이다.
    // 커서에 닿지 않는 변경도 이 교체의 결과로 단정할 수 없다.
    if !touches || (p + s == 0 && ins != commit.len()) {
        return (Some(observed), Verdict::Unknown);
    }

    if ins != commit.len() {
        return (Some(observed), Verdict::Mismatch);
    }
    // 삽입 길이는 맞다 — 삭제량으로 분류한다(정확 일치는 위에서 걸러졌으므로 내용 불일치 포함).
    let verdict = if before == a && after == b {
        Verdict::Mismatch
    } else if before >= a && after >= b {
        Verdict::Over
    } else if before <= a && after <= b {
        Verdict::Under
    } else {
        Verdict::Mismatch
    };
    (Some(observed), verdict)
}

/// 로그 한 줄을 만든다 — 평문 없음.
pub fn format_line(
    ctx: u32,
    frontend: &str,
    a: u32,
    b: u32,
    k: u32,
    observed: Option<Observed>,
    verdict: Verdict,
) -> String {
    let actual = match observed {
        Some(o) => format!("before={}, after={}, ins={}", o.before, o.after, o.ins),
        None => "before=-, after=-, ins=-".to_string(),
    };
    format!(
        "[REPLACE_VERIFY] ctx={} frontend={} expect(before={}, after={}, ins={}) actual({}) verdict={}",
        ctx,
        frontend,
        a,
        b,
        k,
        actual,
        verdict.as_str()
    )
}

/// 발행 시점에 보관하는 상태 — 메모리 전용.
#[derive(Debug)]
struct Pending {
    old: Vec<char>,
    cursor: usize,
    a: u32,
    b: u32,
    commit: Vec<char>,
    issued: Instant,
    frontend: String,
}

/// 컨텍스트별 대기 상태 모음.
#[derive(Debug, Default)]
pub struct ReplaceVerifier {
    pending: HashMap<u32, Pending>,
}

impl ReplaceVerifier {
    pub fn new() -> Self {
        Self::default()
    }

    /// 대기 중인 검증 수(시험·진단용).
    pub fn len(&self) -> usize {
        self.pending.len()
    }

    pub fn is_empty(&self) -> bool {
        self.pending.is_empty()
    }

    /// 교체 시그널 발행 직전에 호출한다. 같은 컨텍스트의 이전 대기는 덮어쓴다.
    #[allow(clippy::too_many_arguments)]
    pub fn issue(
        &mut self,
        ctx: u32,
        frontend: &str,
        old_text: &str,
        cursor: u32,
        a: u32,
        b: u32,
        commit_text: &str,
        now: Instant,
    ) {
        self.pending.insert(
            ctx,
            Pending {
                old: old_text.chars().collect(),
                cursor: cursor as usize,
                a,
                b,
                commit: commit_text.chars().collect(),
                issued: now,
                frontend: if frontend.is_empty() {
                    "unknown".to_string()
                } else {
                    frontend.to_string()
                },
            },
        );
    }

    /// 같은 컨텍스트의 `SetSurroundingText` 도착. 대기가 있으면 로그 줄을 돌려준다.
    pub fn on_surrounding(&mut self, ctx: u32, new_text: &str, now: Instant) -> Option<String> {
        let p = self.pending.get(&ctx)?;
        let in_window = now.saturating_duration_since(p.issued) <= VERIFY_WINDOW;
        // 옛 문서와 똑같은 재보고(키 놓기 때 pull 경합·앱/mutter 재보고)는 교체 결과가 아니다 —
        // 대기를 남기고 다음 보고를 기다린다. 창이 지났으면 아래에서 timeout 으로 확정한다.
        if in_window && new_text.chars().eq(p.old.iter().copied()) {
            return None;
        }
        let p = self.pending.remove(&ctx)?;
        let k = p.commit.len() as u32;
        if !in_window {
            return Some(format_line(
                ctx,
                &p.frontend,
                p.a,
                p.b,
                k,
                None,
                Verdict::Timeout,
            ));
        }
        let new: Vec<char> = new_text.chars().collect();
        let (obs, verdict) = judge(
            &p.old,
            p.cursor,
            p.a as usize,
            p.b as usize,
            &p.commit,
            &new,
        );
        Some(format_line(ctx, &p.frontend, p.a, p.b, k, obs, verdict))
    }

    /// 만료된 대기를 모두 `timeout` 으로 확정해 로그 줄들을 돌려준다.
    pub fn sweep(&mut self, now: Instant) -> Vec<String> {
        let expired: Vec<u32> = self
            .pending
            .iter()
            .filter(|(_, p)| now.saturating_duration_since(p.issued) > VERIFY_WINDOW)
            .map(|(id, _)| *id)
            .collect();
        let mut lines = Vec::with_capacity(expired.len());
        for id in expired {
            if let Some(p) = self.pending.remove(&id) {
                lines.push(format_line(
                    id,
                    &p.frontend,
                    p.a,
                    p.b,
                    p.commit.len() as u32,
                    None,
                    Verdict::Timeout,
                ));
            }
        }
        lines.sort();
        lines
    }

    /// FocusIn·FocusOut·컨텍스트 파괴 — 검증 취소(로그 없음, 보관 텍스트 즉시 폐기).
    pub fn cancel(&mut self, ctx: u32) -> bool {
        self.pending.remove(&ctx).is_some()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cs(s: &str) -> Vec<char> {
        s.chars().collect()
    }

    // 시험 문장: 앞부분 + "국어교육 한국어" (커서는 문서 끝). 한자 단어 "韓國語"(3자) 로 교체.
    const OLD: &str = "가나다라마바사아자차 한국어";

    #[test]
    fn ok_when_document_matches_expected() {
        let old = cs(OLD);
        let c = old.len();
        let new = cs("가나다라마바사아자차 韓國語");
        // 단어 "한국어" 는 앞 3자.
        let (obs, v) = judge(&old, c, 3, 0, &cs("韓國語"), &new);
        assert_eq!(v, Verdict::Ok);
        assert_eq!(
            obs,
            Some(Observed {
                before: 3,
                after: 0,
                ins: 3
            })
        );
    }

    #[test]
    fn ok_with_cursor_in_middle_and_after_deletion() {
        // 커서가 단어 중간: "한|국어" → 앞 1 + 뒤 2.
        let old = cs("가나 한국어 다라");
        let c = 4; // "가나 한" 뒤
        let new = cs("가나 韓國語 다라");
        let (obs, v) = judge(&old, c, 1, 2, &cs("韓國語"), &new);
        assert_eq!(v, Verdict::Ok);
        assert_eq!(obs.unwrap().after, 2);
    }

    #[test]
    fn over_when_deleted_more_than_expected() {
        // 한 글자 더 지움: 공백까지 삭제.
        let old = cs(OLD);
        let c = old.len();
        let new = cs("가나다라마바사아자차韓國語");
        let (obs, v) = judge(&old, c, 3, 0, &cs("韓國語"), &new);
        assert_eq!(v, Verdict::Over);
        let o = obs.unwrap();
        assert_eq!((o.before, o.after, o.ins), (4, 0, 3));
    }

    #[test]
    fn over_when_counted_in_bytes() {
        // 한글 3바이트 → a=3 문자가 아니라 9자가 지워진 경우(바이트 단위 삭제).
        let old = cs(OLD);
        let c = old.len();
        let kept: Vec<char> = old[..c - 9].to_vec();
        let mut new = kept;
        new.extend(cs("韓國語"));
        let (obs, v) = judge(&old, c, 3, 0, &cs("韓國語"), &new);
        assert_eq!(v, Verdict::Over);
        assert_eq!(obs.unwrap().before, 9);
    }

    #[test]
    fn under_when_deleted_less_than_expected() {
        // 한 글자만 지우고 삽입.
        let old = cs(OLD);
        let c = old.len();
        let mut new = old[..c - 1].to_vec();
        new.extend(cs("韓國語"));
        let (obs, v) = judge(&old, c, 3, 0, &cs("韓國語"), &new);
        assert_eq!(v, Verdict::Under);
        assert_eq!(obs.unwrap().before, 1);
    }

    #[test]
    fn mismatch_when_commit_missing() {
        // 삭제만 되고 커밋이 없다.
        let old = cs(OLD);
        let c = old.len();
        let new = old[..c - 3].to_vec();
        let (obs, v) = judge(&old, c, 3, 0, &cs("韓國語"), &new);
        assert_eq!(v, Verdict::Mismatch);
        assert_eq!(obs.unwrap().ins, 0);
    }

    #[test]
    fn unknown_when_window_moved() {
        // surrounding 창이 이동해 접두·접미가 전혀 없다.
        let old = cs("abcdefghij");
        let new = cs("klmnopqrstu");
        let (_, v) = judge(&old, 5, 3, 0, &cs("韓國語"), &new);
        assert_eq!(v, Verdict::Unknown);
    }

    #[test]
    fn unknown_when_change_not_at_cursor() {
        // 커서와 무관한 위치만 바뀜(문서 앞부분 편집).
        let old = cs(OLD);
        let c = old.len();
        let mut new = old.clone();
        new[0] = 'X';
        let (_, v) = judge(&old, c, 3, 0, &cs("韓國語"), &new);
        assert_eq!(v, Verdict::Unknown);
    }

    #[test]
    fn unknown_when_empty() {
        let (obs, v) = judge(&[], 0, 3, 0, &cs("韓國語"), &cs("x"));
        assert_eq!((obs, v), (None, Verdict::Unknown));
        let (obs, v) = judge(&cs("abc"), 3, 1, 0, &cs("x"), &[]);
        assert_eq!((obs, v), (None, Verdict::Unknown));
    }

    #[test]
    fn format_line_has_no_text_and_expected_shape() {
        let l = format_line(
            7,
            "GNOME",
            3,
            0,
            3,
            Some(Observed {
                before: 9,
                after: 0,
                ins: 3,
            }),
            Verdict::Over,
        );
        assert_eq!(
            l,
            "[REPLACE_VERIFY] ctx=7 frontend=GNOME expect(before=3, after=0, ins=3) \
             actual(before=9, after=0, ins=3) verdict=over"
        );
    }

    #[test]
    fn verifier_ok_flow_and_removal() {
        let t0 = Instant::now();
        let mut v = ReplaceVerifier::new();
        v.issue(
            5,
            "Qt6",
            OLD,
            OLD.chars().count() as u32,
            3,
            0,
            "韓國語",
            t0,
        );
        assert_eq!(v.len(), 1);
        let line = v
            .on_surrounding(
                5,
                "가나다라마바사아자차 韓國語",
                t0 + Duration::from_millis(400),
            )
            .unwrap();
        assert!(line.contains("ctx=5 frontend=Qt6"));
        assert!(line.ends_with("verdict=ok"));
        assert!(v.is_empty());
        // 대기가 없으면 아무것도 하지 않는다.
        assert!(v.on_surrounding(5, "x", t0).is_none());
    }

    #[test]
    fn verifier_keeps_pending_on_duplicate_old_report() {
        let t0 = Instant::now();
        let mut v = ReplaceVerifier::new();
        v.issue(
            7,
            "GNOME",
            OLD,
            OLD.chars().count() as u32,
            3,
            0,
            "韓國語",
            t0,
        );
        // 앱이 교체 적용 전에 옛 문서를 한 번 더 보고 — 소비하지 않는다.
        assert!(v
            .on_surrounding(7, OLD, t0 + Duration::from_millis(100))
            .is_none());
        assert_eq!(v.len(), 1);
        let line = v
            .on_surrounding(
                7,
                "가나다라마바사아자차 韓國語",
                t0 + Duration::from_millis(300),
            )
            .unwrap();
        assert!(line.ends_with("verdict=ok"));
        assert!(v.is_empty());
        // 창이 지난 뒤의 중복 보고는 timeout 으로 확정된다.
        v.issue(8, "GNOME", OLD, 14, 3, 0, "韓國語", t0);
        let line = v
            .on_surrounding(8, OLD, t0 + Duration::from_millis(3001))
            .unwrap();
        assert!(line.ends_with("verdict=timeout"));
    }

    #[test]
    fn verifier_timeout_on_late_surrounding() {
        let t0 = Instant::now();
        let mut v = ReplaceVerifier::new();
        v.issue(1, "", OLD, 14, 3, 0, "韓國語", t0);
        let line = v
            .on_surrounding(1, "무엇이든", t0 + Duration::from_millis(3001))
            .unwrap();
        assert!(line.contains("frontend=unknown"));
        assert!(line.contains("actual(before=-, after=-, ins=-)"));
        assert!(line.ends_with("verdict=timeout"));
    }

    #[test]
    fn verifier_sweep_expires_only_old_entries() {
        let t0 = Instant::now();
        let mut v = ReplaceVerifier::new();
        v.issue(1, "GTK3", OLD, 14, 3, 0, "韓國語", t0);
        v.issue(
            2,
            "GTK4",
            OLD,
            14,
            3,
            0,
            "韓國語",
            t0 + Duration::from_secs(2),
        );
        let lines = v.sweep(t0 + Duration::from_millis(3500));
        assert_eq!(lines.len(), 1);
        assert!(lines[0].contains("ctx=1 frontend=GTK3"));
        assert!(lines[0].ends_with("verdict=timeout"));
        assert_eq!(v.len(), 1);
        assert!(v.sweep(t0 + Duration::from_millis(3500)).is_empty());
    }

    #[test]
    fn verifier_cancel_drops_state() {
        let t0 = Instant::now();
        let mut v = ReplaceVerifier::new();
        v.issue(9, "GTK3", OLD, 14, 3, 0, "韓國語", t0);
        assert!(v.cancel(9));
        assert!(!v.cancel(9));
        assert!(v.on_surrounding(9, OLD, t0).is_none());
    }
}
