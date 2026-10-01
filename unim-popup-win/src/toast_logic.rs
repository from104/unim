//! 토스트 순수 로직 — 중복 억제·표시 시간·모서리 배치·문자열 정리 (NOTIFY_SPEC §3.4).
//!
//! Win32 에 의존하지 않는 순수 모듈이라 Linux `cargo test` 로 검증한다. 시각은 호출자가
//! `Instant` 로 넘긴다(경계값 테스트가 결정적).

use std::collections::HashMap;
use std::time::{Duration, Instant};

/// 동일 `(kind, key_hash)` 재발 폐기 창 — 코어 `NotifyGate` 의 `DEDUPE_WINDOW`(3,000 ms)와
/// 같은 값이다. TSF DLL 은 호스트 프로세스별이라 앱 간 중복을 못 보므로 렌더러가 다시 거른다.
pub const DEDUPE_WINDOW: Duration = Duration::from_millis(3000);
/// 기록 맵이 이 크기를 넘으면 만료분을 정리한다(무한 증가 방지).
const PRUNE_THRESHOLD: usize = 64;

/// 표시 시간 허용 범위(ms) — 설정 범위(`notify.duration_ms` 500~5000)와 같다.
pub const MIN_DURATION_MS: u32 = 500;
pub const MAX_DURATION_MS: u32 = 5000;
/// 페이로드가 0(미지정)일 때 쓰는 기본 표시 시간.
pub const DEFAULT_DURATION_MS: u32 = 2000;

/// 제목·본문 최대 글자 수(방어적 — 코어가 이미 줄여 보내지만 렌더러도 신뢰하지 않는다).
pub const TITLE_MAX_CHARS: usize = 40;
pub const BODY_MAX_CHARS: usize = 120;

/// `(kind, key_hash)` 3초 중복 억제기.
#[derive(Default)]
pub struct ToastDedupe {
    last: HashMap<(String, u64), Instant>,
}

impl ToastDedupe {
    /// 표시해도 되면 true 를 돌려주고 시각을 기록한다. 3 초 안의 동일 키 재발이면 false
    /// (기록은 갱신하지 않는다 — 계속 들어와도 창이 밀리지 않고 3 초마다 한 번씩 통과).
    pub fn admit(&mut self, kind: &str, key_hash: u64, now: Instant) -> bool {
        if let Some(t) = self.last.get(&(kind.to_string(), key_hash)) {
            if now.saturating_duration_since(*t) < DEDUPE_WINDOW {
                return false;
            }
        }
        if self.last.len() >= PRUNE_THRESHOLD {
            self.last
                .retain(|_, t| now.saturating_duration_since(*t) < DEDUPE_WINDOW);
        }
        self.last.insert((kind.to_string(), key_hash), now);
        true
    }
}

/// 표시 시간(ms) 보정: 0 → 기본값, 그 외 허용 범위로 클램프.
pub fn clamp_duration_ms(requested: u32) -> u32 {
    if requested == 0 {
        DEFAULT_DURATION_MS
    } else {
        requested.clamp(MIN_DURATION_MS, MAX_DURATION_MS)
    }
}

/// 모서리 설정 문자열 해석.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Corner {
    TopRight,
    BottomRight,
    TopLeft,
    BottomLeft,
}

impl Corner {
    /// `auto|top_right|bottom_right|top_left|bottom_left`. `auto`·미지 값은 우하단.
    pub fn parse(s: &str) -> Corner {
        match s.trim().to_ascii_lowercase().replace('-', "_").as_str() {
            "top_right" => Corner::TopRight,
            "top_left" => Corner::TopLeft,
            "bottom_left" => Corner::BottomLeft,
            _ => Corner::BottomRight,
        }
    }
}

/// 작업 영역 `(left, top, right, bottom)` 의 지정 모서리에 `w × h` 창을 놓는 좌상단 좌표.
///
/// `margin` 은 작업 영역 가장자리와의 간격(물리 px). 작업 영역보다 큰 창은 좌상단으로
/// 클램프해 화면 밖으로 나가지 않게 한다.
pub fn corner_position(
    corner: Corner,
    work: (i32, i32, i32, i32),
    w: i32,
    h: i32,
    margin: i32,
) -> (i32, i32) {
    let (l, t, r, b) = work;
    let x = match corner {
        Corner::TopRight | Corner::BottomRight => r - margin - w,
        Corner::TopLeft | Corner::BottomLeft => l + margin,
    };
    let y = match corner {
        Corner::BottomRight | Corner::BottomLeft => b - margin - h,
        Corner::TopRight | Corner::TopLeft => t + margin,
    };
    (x.max(l), y.max(t))
}

/// 두 사각형 `(left, top, right, bottom)` 이 겹치는지(우·하 경계는 제외 — 맞닿기만 하면 겹침 아님).
/// 폭·높이가 0 이하인 빈 사각형은 겹치지 않는다. 토스트는 후보 팝업과 겹치면 숨는다(NOTIFY_SPEC §3.4).
pub fn rects_overlap(a: (i32, i32, i32, i32), b: (i32, i32, i32, i32)) -> bool {
    let empty = |r: (i32, i32, i32, i32)| r.2 <= r.0 || r.3 <= r.1;
    if empty(a) || empty(b) {
        return false;
    }
    a.0 < b.2 && b.0 < a.2 && a.1 < b.3 && b.1 < a.3
}

/// 한 줄 정리 + 최대 `max` 글자(초과 시 마지막 글자를 `…`). 제어문자는 공백.
/// 줄바꿈이 필요한 본문은 [`clean_multiline`] 을 쓴다.
pub fn clean_line(s: &str, max: usize) -> String {
    let flat: String = s
        .chars()
        .map(|c| if c.is_control() { ' ' } else { c })
        .collect();
    truncate_chars(flat.trim(), max)
}

/// 본문 정리: 제어문자(개행 제외)를 공백으로, 최대 `max` 글자.
pub fn clean_multiline(s: &str, max: usize) -> String {
    let flat: String = s
        .chars()
        .map(|c| if c.is_control() && c != '\n' { ' ' } else { c })
        .collect();
    truncate_chars(flat.trim(), max)
}

fn truncate_chars(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        return s.to_string();
    }
    let mut out: String = s.chars().take(max.saturating_sub(1)).collect();
    out.push('…');
    out
}

/// 스크린리더로 읽을 문자열: 제목과 본문을 공백으로 잇는다(빈 쪽은 생략).
pub fn spoken_text(title: &str, body: &str) -> String {
    match (title.is_empty(), body.is_empty()) {
        (true, true) => String::new(),
        (false, true) => title.to_string(),
        (true, false) => body.to_string(),
        (false, false) => format!("{title} {body}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn t0() -> Instant {
        Instant::now()
    }

    #[test]
    fn dedupe_blocks_same_key_within_window_and_passes_after() {
        let mut d = ToastDedupe::default();
        let base = t0();
        assert!(d.admit("atf_corrected_forward", 1, base));
        assert!(!d.admit("atf_corrected_forward", 1, base + Duration::from_millis(2999)));
        assert!(d.admit("atf_corrected_forward", 1, base + Duration::from_millis(3000)));
    }

    #[test]
    fn dedupe_distinguishes_kind_and_key() {
        let mut d = ToastDedupe::default();
        let base = t0();
        assert!(d.admit("a", 1, base));
        assert!(d.admit("a", 2, base), "다른 key_hash 는 통과");
        assert!(d.admit("b", 1, base), "다른 kind 는 통과");
        assert!(!d.admit("a", 1, base + Duration::from_millis(10)));
    }

    #[test]
    fn dedupe_blocked_attempt_does_not_extend_window() {
        let mut d = ToastDedupe::default();
        let base = t0();
        assert!(d.admit("k", 7, base));
        assert!(!d.admit("k", 7, base + Duration::from_millis(2000)));
        // 2 초 시점의 차단이 창을 밀지 않으므로 3 초 시점에 통과한다.
        assert!(d.admit("k", 7, base + Duration::from_millis(3000)));
    }

    #[test]
    fn dedupe_prunes_expired_entries() {
        let mut d = ToastDedupe::default();
        let base = t0();
        for i in 0..(PRUNE_THRESHOLD as u64 + 10) {
            assert!(d.admit("k", i, base));
        }
        // 창이 지난 뒤 새 항목을 넣으면 만료분이 정리된다.
        assert!(d.admit("k", 9999, base + Duration::from_secs(10)));
        assert!(d.last.len() <= 2, "len={}", d.last.len());
    }

    #[test]
    fn duration_clamped_to_setting_range() {
        assert_eq!(clamp_duration_ms(0), DEFAULT_DURATION_MS);
        assert_eq!(clamp_duration_ms(1), MIN_DURATION_MS);
        assert_eq!(clamp_duration_ms(499), MIN_DURATION_MS);
        assert_eq!(clamp_duration_ms(2000), 2000);
        assert_eq!(clamp_duration_ms(5000), 5000);
        assert_eq!(clamp_duration_ms(u32::MAX), MAX_DURATION_MS);
    }

    #[test]
    fn corner_parse_defaults_to_bottom_right() {
        assert_eq!(Corner::parse("auto"), Corner::BottomRight);
        assert_eq!(Corner::parse(""), Corner::BottomRight);
        assert_eq!(Corner::parse("garbage"), Corner::BottomRight);
        assert_eq!(Corner::parse("top_right"), Corner::TopRight);
        assert_eq!(Corner::parse("TOP-LEFT"), Corner::TopLeft);
        assert_eq!(Corner::parse("bottom_left"), Corner::BottomLeft);
    }

    #[test]
    fn corner_position_four_corners() {
        let work = (100, 50, 1100, 850); // 1000 × 800 작업 영역(주 모니터가 아닌 좌표 포함)
        let (w, h, m) = (300, 80, 16);
        assert_eq!(corner_position(Corner::TopLeft, work, w, h, m), (116, 66));
        assert_eq!(corner_position(Corner::TopRight, work, w, h, m), (784, 66));
        assert_eq!(corner_position(Corner::BottomLeft, work, w, h, m), (116, 754));
        assert_eq!(corner_position(Corner::BottomRight, work, w, h, m), (784, 754));
    }

    #[test]
    fn corner_position_negative_origin_secondary_monitor() {
        // 주 모니터 왼쪽에 놓인 보조 모니터(좌표가 음수).
        let work = (-1920, 0, 0, 1040);
        let (x, y) = corner_position(Corner::BottomRight, work, 320, 90, 20);
        assert_eq!((x, y), (-1920 + 1920 - 20 - 320, 1040 - 20 - 90));
        assert!(x >= work.0 && x + 320 <= work.2);
    }

    #[test]
    fn corner_position_clamps_oversized_window() {
        let work = (0, 0, 200, 100);
        let (x, y) = corner_position(Corner::BottomRight, work, 500, 300, 16);
        assert_eq!((x, y), (0, 0), "작업 영역보다 크면 좌상단으로 클램프");
    }

    #[test]
    fn clean_line_flattens_and_truncates() {
        assert_eq!(clean_line("  a\tb\nc  ", 10), "a b c");
        let long: String = "가".repeat(50);
        let out = clean_line(&long, 10);
        assert_eq!(out.chars().count(), 10);
        assert!(out.ends_with('…'));
        assert_eq!(clean_line("", 5), "");
    }

    #[test]
    fn clean_multiline_keeps_newlines() {
        assert_eq!(clean_multiline("a\nb\tc\r", 20), "a\nb c");
        assert_eq!(clean_multiline("abcdef", 4), "abc…");
    }

    #[test]
    fn spoken_text_joins_nonempty_parts() {
        assert_eq!(spoken_text("UNIM", "자동 교정"), "UNIM 자동 교정");
        assert_eq!(spoken_text("UNIM", ""), "UNIM");
        assert_eq!(spoken_text("", "본문"), "본문");
        assert_eq!(spoken_text("", ""), "");
    }

    #[test]
    fn rects_overlap_detects_intersection_and_touching() {
        let a = (0, 0, 100, 50);
        assert!(rects_overlap(a, (99, 49, 200, 200)));
        assert!(rects_overlap(a, (10, 10, 20, 20))); // 포함
        assert!(!rects_overlap(a, (100, 0, 200, 50))); // 오른쪽 맞닿음
        assert!(!rects_overlap(a, (0, 50, 100, 90))); // 아래 맞닿음
        assert!(!rects_overlap(a, (300, 300, 400, 400)));
        assert!(!rects_overlap(a, (10, 10, 10, 40))); // 빈 사각형
    }
}
