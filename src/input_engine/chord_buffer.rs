// chord_buffer.rs
//! 안마태 모아치기 — chord 윈도우 버퍼.
//!
//! `chord_window_ms > 0` 이고 `bidirectional_combine=true` 인 안마태 자판에서만 활성화.
//!
//! ## 동작 원리
//!
//! 첫 자모가 들어오면 타임스탬프를 기록하고 버퍼에 push한다.
//! 이후 자모가 들어올 때마다 `elapsed < chord_window_ms`면 버퍼에 계속 push.
//! 만료(또는 비-자모 키, 버퍼 MAX_SIZE, flush 강제 호출)가 되면 `take_chord()`로
//! 현재 버퍼를 꺼낸다. 호출자는 꺼낸 `Vec<JamoEntry>`를 영역별 분류 후 순차 add_jamo.
//!
//! ## chord_window_ms = 0
//!
//! 버퍼를 사용하지 않음. `push()` → 즉시 `None` 반환 (기존 즉시 처리 경로).
//!
//! ## 음절 경계
//!
//! flush된 버퍼를 영역별로 분류할 때:
//! - Cho 계열 자모 먼저 → 하나로 결합 시도 (순방향 → 역방향)
//! - Jung 계열 다음 → 하나로 결합 시도
//! - Jong 계열 마지막 → 하나로 결합 시도
//!
//! 단, 실제 결합 로직은 `HangulInputContext`(composer)에 위임하며,
//! 이 모듈은 **버퍼 관리와 만료 판단**만 담당한다.
//!
//! ## idle flush epoch
//!
//! `push()` 호출 시 반환된 epoch는 idle flush 타이머의 취소 판별에 사용한다.
//! `clear()` / `force_flush()` / layout-change / reset 등으로 버퍼가 폐기되면
//! epoch가 증가하여 이전 타이머가 발화해도 무시된다.

use crate::hangul::composer::JamoMeta;
use crate::hangul::jamo::JamoEnum;
use std::time::Instant;

/// chord 버퍼 최대 크기 (이 이상이면 즉시 flush).
pub const CHORD_BUFFER_MAX: usize = 8;

/// 버퍼에 저장되는 자모 + 메타 쌍.
#[derive(Debug, Clone)]
pub struct JamoEntry {
    pub jamo: JamoEnum,
    pub meta: JamoMeta,
}

/// 자모가 속하는 영역 (영역별 정렬·분류용).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum JamoRegion {
    Cho = 0,
    Jung = 1,
    Jong = 2,
}

impl JamoEntry {
    pub fn region(&self) -> JamoRegion {
        match self.jamo {
            JamoEnum::Cho(_) => JamoRegion::Cho,
            JamoEnum::Jung(_) => JamoRegion::Jung,
            JamoEnum::Jong(_) => JamoRegion::Jong,
            JamoEnum::Special(_) => JamoRegion::Cho, // 도달하지 않음
        }
    }
}

/// chord 결과: 버퍼가 만료됐을 때 꺼내는 자모 목록.
///
/// 영역별 정렬(Cho→Jung→Jong)을 미리 적용해 반환한다.
pub type ChordResult = Vec<JamoEntry>;

/// chord 윈도우 버퍼.
pub struct ChordBuffer {
    /// chord 윈도우 (ms). 0 = OFF.
    window_ms: u16,
    /// 현재 chord 버퍼.
    buffer: Vec<JamoEntry>,
    /// chord 첫 자모 도착 시각.
    start: Option<Instant>,
    /// idle flush 취소 판별용 epoch 카운터.
    ///
    /// 새 chord 시작(첫 자모 push) 시 증가. idle flush 타이머가 발화할 때
    /// 현재 epoch와 비교해 불일치하면 무시 (reset/layout-change/clear 등으로 이미 폐기).
    epoch: u64,
}

impl ChordBuffer {
    pub fn new(window_ms: u16) -> Self {
        Self {
            window_ms,
            buffer: Vec::with_capacity(8),
            start: None,
            epoch: 0,
        }
    }

    /// window_ms 업데이트 (설정 변경 시).
    pub fn set_window_ms(&mut self, window_ms: u16) {
        self.window_ms = window_ms;
    }

    /// chord 기능 활성 여부.
    #[inline]
    pub fn is_active(&self) -> bool {
        self.window_ms > 0
    }

    /// 현재 버퍼에 자모가 있는지.
    #[allow(dead_code)]
    #[inline]
    pub fn has_pending(&self) -> bool {
        !self.buffer.is_empty()
    }

    /// 현재 epoch 값 반환.
    ///
    /// idle flush 타이머 발사 시 이 값을 함께 전달해 나중에 유효성 검증에 사용한다.
    #[inline]
    pub fn current_epoch(&self) -> u64 {
        self.epoch
    }

    /// chord 윈도우 크기 반환 (ms).
    ///
    /// idle flush 타이머의 sleep 시간 계산에 사용한다.
    #[inline]
    pub fn window_ms_pub(&self) -> u16 {
        self.window_ms
    }

    /// 자모를 버퍼에 push한다.
    ///
    /// 반환값:
    /// - `None` → chord 진행 중, 아직 flush하지 않음
    /// - `Some(entries)` → 이전 chord가 만료됐으므로 flush. entries = 이전 chord 내용.
    ///   caller는 entries를 처리한 뒤 새 자모로 새 chord를 시작해야 한다.
    ///   (새 자모는 이미 내부적으로 새 버퍼에 push됨)
    ///
    /// chord OFF(`window_ms == 0`)이면 항상 `Some(vec![entry])` 즉시 반환.
    pub fn push(&mut self, jamo: JamoEnum, meta: JamoMeta) -> Option<ChordResult> {
        // chord OFF → 단일 자모를 즉시 반환 (기존 즉시 처리와 동일)
        if self.window_ms == 0 {
            return Some(vec![JamoEntry { jamo, meta }]);
        }

        let now = Instant::now();

        // 윈도우 만료 여부 체크 (단일 윈도우: 첫 타건 시점 기준 절대 만료)
        let expired = self.start.map_or(false, |t| {
            now.duration_since(t).as_millis() >= self.window_ms as u128
        });

        if expired && !self.buffer.is_empty() {
            // 이전 chord flush → 새 chord 시작
            let mut flushed = std::mem::take(&mut self.buffer);
            sort_by_region(&mut flushed);
            self.epoch = self.epoch.wrapping_add(1);
            self.buffer.push(JamoEntry { jamo, meta });
            self.start = Some(now);
            return Some(flushed);
        }

        // 버퍼가 MAX에 도달했으면 즉시 flush (현재 자모 포함)
        if self.buffer.len() >= CHORD_BUFFER_MAX {
            let mut flushed = std::mem::take(&mut self.buffer);
            flushed.push(JamoEntry { jamo, meta });
            sort_by_region(&mut flushed);
            self.epoch = self.epoch.wrapping_add(1);
            self.start = None;
            return Some(flushed);
        }

        // chord 진행 중: 버퍼에 추가
        if self.start.is_none() {
            // 새 chord 시작 — epoch 증가
            self.epoch = self.epoch.wrapping_add(1);
            self.start = Some(now);
        }
        self.buffer.push(JamoEntry { jamo, meta });
        None
    }

    /// 버퍼를 강제 flush (비-자모 키, Space, Enter 등).
    ///
    /// 버퍼가 비어 있으면 `None`.
    /// flush 후 epoch를 증가시켜 진행 중이던 idle flush 타이머를 무효화한다.
    pub fn force_flush(&mut self) -> Option<ChordResult> {
        if self.buffer.is_empty() {
            return None;
        }
        let mut flushed = std::mem::take(&mut self.buffer);
        sort_by_region(&mut flushed);
        self.epoch = self.epoch.wrapping_add(1);
        self.start = None;
        Some(flushed)
    }

    /// 버퍼 초기화 (reset/clear).
    ///
    /// epoch를 증가시켜 진행 중이던 idle flush 타이머를 무효화한다.
    pub fn clear(&mut self) {
        self.buffer.clear();
        self.start = None;
        self.epoch = self.epoch.wrapping_add(1);
    }

    /// idle flush 타이머 epoch 유효성 검증.
    ///
    /// 타이머 발화 시 전달받은 epoch가 현재 epoch와 일치하고 버퍼가 비어있지 않으면 `true`.
    /// 불일치 또는 버퍼 비어있으면 `false` (무시).
    pub fn is_idle_epoch_valid(&self, epoch: u64) -> bool {
        self.epoch == epoch && !self.buffer.is_empty()
    }

    /// 현재 버퍼 길이.
    #[cfg(test)]
    pub fn len(&self) -> usize {
        self.buffer.len()
    }
}

/// 자모 목록을 영역 순서(Cho→Jung→Jong)로 정렬한다.
///
/// 같은 영역 내에서는 입력 순서를 보존 (stable sort).
fn sort_by_region(entries: &mut Vec<JamoEntry>) {
    entries.sort_by_key(|e| e.region() as u8);
}

// ============================================================================
// 테스트
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hangul::jamo::{Cho, Jong, Jung};

    fn cho(c: Cho) -> JamoEnum {
        JamoEnum::Cho(c)
    }
    fn jung(j: Jung) -> JamoEnum {
        JamoEnum::Jung(j)
    }
    fn jong(j: Jong) -> JamoEnum {
        JamoEnum::Jong(j)
    }

    /// chord OFF (window_ms=0) → 즉시 flush (기존 동작 회귀 0)
    #[test]
    fn chord_off_immediate_flush() {
        let mut buf = ChordBuffer::new(0);
        let result = buf.push(cho(Cho::G), JamoMeta::default());
        assert!(result.is_some(), "chord OFF → 즉시 flush");
        let entries = result.unwrap();
        assert_eq!(entries.len(), 1);
        assert!(matches!(entries[0].jamo, JamoEnum::Cho(Cho::G)));
    }

    /// chord ON → 자모 누적 후 force_flush
    #[test]
    fn chord_on_force_flush() {
        let mut buf = ChordBuffer::new(50);
        // ㄱ push → None (진행 중)
        assert!(buf.push(cho(Cho::G), JamoMeta::default()).is_none());
        assert_eq!(buf.len(), 1);
        // ㅏ push → None
        assert!(buf.push(jung(Jung::A), JamoMeta::default()).is_none());
        assert_eq!(buf.len(), 2);
        // force_flush → [ㄱ, ㅏ] (Cho→Jung 순서)
        let result = buf.force_flush().unwrap();
        assert_eq!(result.len(), 2);
        assert!(matches!(result[0].jamo, JamoEnum::Cho(_)));
        assert!(matches!(result[1].jamo, JamoEnum::Jung(_)));
        assert!(!buf.has_pending());
    }

    /// chord 영역 정렬: Jong→Cho 입력 순서여도 Cho→Jong으로 정렬
    #[test]
    fn chord_sort_by_region() {
        let mut buf = ChordBuffer::new(50);
        buf.push(jong(Jong::Giyeok), JamoMeta::default());
        buf.push(cho(Cho::G), JamoMeta::default());
        buf.push(jung(Jung::A), JamoMeta::default());
        let result = buf.force_flush().unwrap();
        assert_eq!(result.len(), 3);
        assert!(matches!(result[0].jamo, JamoEnum::Cho(_)));
        assert!(matches!(result[1].jamo, JamoEnum::Jung(_)));
        assert!(matches!(result[2].jamo, JamoEnum::Jong(_)));
    }

    /// MAX_SIZE(8) 도달 시 즉시 flush.
    /// 버퍼가 이미 MAX(8)개 찼을 때 다음(9번째) push에서 flush 발생.
    #[test]
    fn chord_max_size_flush() {
        let mut buf = ChordBuffer::new(5000); // 아주 긴 윈도우
        // 처음 8개: None (진행 중, 버퍼에 8개 누적)
        for i in 0..8 {
            let r = buf.push(cho(Cho::G), JamoMeta::default());
            assert!(r.is_none(), "push {}: 아직 flush 안 됨", i);
        }
        assert_eq!(buf.len(), 8, "버퍼에 8개 누적");
        // 9번째: len==8 >= MAX → 즉시 flush (8개) + 새 자모 포함 = 9개
        let r = buf.push(cho(Cho::H), JamoMeta::default());
        assert!(r.is_some(), "9번째 push → MAX flush");
        let entries = r.unwrap();
        assert_eq!(entries.len(), 9, "flush 결과: 기존 8 + 새 1 = 9");
        assert_eq!(buf.len(), 0, "flush 후 버퍼 비어 있음");
    }

    /// 빈 버퍼 force_flush → None
    #[test]
    fn chord_empty_force_flush() {
        let mut buf = ChordBuffer::new(50);
        assert!(buf.force_flush().is_none());
    }

    /// chord-Q1-keui: ㄱ ㅎ ㅡ ㅣ → Cho[ㄱ,ㅎ], Jung[ㅡ,ㅣ] 순서로 분류
    #[test]
    fn chord_q1_keui_region_order() {
        let mut buf = ChordBuffer::new(5000);
        buf.push(cho(Cho::G), JamoMeta::default());
        buf.push(cho(Cho::H), JamoMeta::default());
        buf.push(jung(Jung::Eu), JamoMeta::default());
        buf.push(jung(Jung::I), JamoMeta::default());
        let result = buf.force_flush().unwrap();
        assert_eq!(result.len(), 4);
        // 처음 2개 Cho, 다음 2개 Jung
        assert!(matches!(result[0].jamo, JamoEnum::Cho(_)));
        assert!(matches!(result[1].jamo, JamoEnum::Cho(_)));
        assert!(matches!(result[2].jamo, JamoEnum::Jung(_)));
        assert!(matches!(result[3].jamo, JamoEnum::Jung(_)));
    }

    // =========================================================================
    // epoch 검증 테스트
    // =========================================================================

    /// epoch: 초기값 0, 첫 자모 push → epoch 1
    #[test]
    fn chord_epoch_increments_on_first_push() {
        let mut buf = ChordBuffer::new(50);
        assert_eq!(buf.current_epoch(), 0);
        buf.push(cho(Cho::G), JamoMeta::default());
        assert_eq!(buf.current_epoch(), 1, "첫 push → epoch 증가");
    }

    /// epoch: clear() 후 epoch 증가 → idle flush 타이머 무효화
    #[test]
    fn chord_epoch_increments_on_clear() {
        let mut buf = ChordBuffer::new(50);
        buf.push(cho(Cho::G), JamoMeta::default());
        let epoch_before = buf.current_epoch();
        buf.clear();
        assert!(
            buf.current_epoch() > epoch_before,
            "clear() → epoch 증가"
        );
        assert!(!buf.is_idle_epoch_valid(epoch_before), "이전 epoch는 무효");
    }

    /// epoch: force_flush() 후 epoch 증가
    #[test]
    fn chord_epoch_increments_on_force_flush() {
        let mut buf = ChordBuffer::new(50);
        buf.push(cho(Cho::G), JamoMeta::default());
        let epoch_before = buf.current_epoch();
        buf.force_flush();
        assert!(
            buf.current_epoch() > epoch_before,
            "force_flush() → epoch 증가"
        );
    }

    /// is_idle_epoch_valid: 버퍼 비어있으면 false
    #[test]
    fn chord_epoch_valid_requires_pending() {
        let mut buf = ChordBuffer::new(50);
        buf.push(cho(Cho::G), JamoMeta::default());
        let epoch = buf.current_epoch();
        buf.force_flush(); // 버퍼 비움
        assert!(
            !buf.is_idle_epoch_valid(epoch),
            "버퍼 비어있으면 epoch 일치해도 false"
        );
    }

    /// 단일 윈도우: 두 번째 자모 push 후 start 타임스탬프 갱신 안 됨
    /// (sliding window가 아님 — 첫 타건 시점 고정)
    #[test]
    fn chord_single_window_no_sliding() {
        let mut buf = ChordBuffer::new(200);
        buf.push(cho(Cho::G), JamoMeta::default());
        let start_epoch = buf.current_epoch();
        // 두 번째 자모 — epoch 변화 없어야 함 (새 chord 시작이 아니므로)
        buf.push(jung(Jung::A), JamoMeta::default());
        assert_eq!(buf.current_epoch(), start_epoch, "sliding window 아님 — epoch 불변");
        assert_eq!(buf.len(), 2);
    }
}
