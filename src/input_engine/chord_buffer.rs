// chord_buffer.rs
//! 안마태 모아치기 — chord 윈도우 버퍼.
//!
//! `chord_window_ms > 0` 이고 `bidirectional_combine=true` 인 안마태 자판에서만 활성화.
//!
//! ## 동작 원리
//!
//! 첫 키가 들어오면 타임스탬프를 기록하고 버퍼에 push한다.
//! 이후 키가 들어올 때마다 `elapsed < chord_window_ms`면 버퍼에 계속 push.
//! 만료(또는 비-자모 키, 버퍼 MAX_SIZE, flush 강제 호출)가 되면 `force_flush()`로
//! 현재 버퍼를 꺼낸다. 호출자는 꺼낸 `Vec<ChordEntry>`를 apply_chord_entries로 처리한다.
//!
//! ## chord_window_ms = 0
//!
//! 버퍼를 사용하지 않음. `push_jamo()` / `push_non_jamo()` → 즉시 `Some(vec![entry])` 반환
//! (기존 즉시 처리 경로와 동일).
//!
//! ## Phase 2: 비자모 수용
//!
//! `push_non_jamo(c: char)` 추가 — chord 활성 시 비자모 문자도 윈도우에 합류.
//! 만료 시 ChordEntry 목록에 Jamo/NonJamo 혼합 반환. input_order로 입력 순서 보존.
//!
//! ## idle flush epoch
//!
//! `push_jamo()` / `push_non_jamo()` 호출 시 반환된 epoch는 idle flush 타이머의 취소 판별에 사용한다.
//! `clear()` / `force_flush()` / layout-change / reset 등으로 버퍼가 폐기되면
//! epoch가 증가하여 이전 타이머가 발화해도 무시된다.

use crate::hangul::composer::JamoMeta;
use crate::hangul::jamo::JamoEnum;
use crate::input_engine::chord_compose::{ChordEntry, ChordEntryKind};
use std::time::Instant;

/// chord 버퍼 최대 크기 (이 이상이면 즉시 flush).
/// Phase 2에서 16으로 상향 (자모+비자모 합산).
pub const CHORD_BUFFER_MAX: usize = 16;

/// chord flush 결과: 버퍼가 만료됐을 때 꺼내는 항목 목록.
///
/// input_order 순서 보존 (정렬은 Phase 3의 chord_compose가 담당).
/// chord_compose::ChordResult(구조체)와 이름 충돌을 피해 ChordFlushResult로 명명.
pub type ChordFlushResult = Vec<ChordEntry>;

/// chord 윈도우 버퍼.
pub struct ChordBuffer {
    /// chord 윈도우 (ms). 0 = OFF.
    window_ms: u16,
    /// 현재 chord 버퍼.
    buffer: Vec<ChordEntry>,
    /// chord 첫 키 도착 시각.
    start: Option<Instant>,
    /// idle flush 취소 판별용 epoch 카운터.
    ///
    /// 새 chord 시작(첫 키 push) 시 증가. idle flush 타이머가 발화할 때
    /// 현재 epoch와 비교해 불일치하면 무시 (reset/layout-change/clear 등으로 이미 폐기).
    epoch: u64,
    /// 현재 chord 내 다음 input_order 카운터.
    next_order: u8,
    /// 현재 buffer 의 부분 결합 결과가 preedit 에 미리 inject 된 상태.
    ///
    /// `update_chord_preview` 가 Case A(전 영역 reduce 성공) 결과를
    /// `inject_chord_syllable` 로 미리 주입했을 때 `true`. 이후 idle/force flush 시
    /// `apply_chord_entries` 를 다시 호출하면 동일 음절이 두 번 commit 되므로,
    /// 호출자는 이 플래그를 보고 중복 처리를 회피한다.
    ///
    /// `clear` / `force_flush` / 새 chord 시작 / 만료 flush / MAX flush 시 false 로 리셋.
    preview_injected: bool,
    /// 마지막 preview 가 atomic 모아쓰기(`inject_chord_syllable`) 였는지 여부.
    ///
    /// - `true`: 다중 키(buffer.len >= 2) atomic chord — composer 가 chord 음절로 통째 교체된 상태.
    /// - `false`: 단일 키(buffer.len == 1) sequential — composer 가 직전 상태 + 자모 1개로 누적된 상태.
    ///
    /// sequential → atomic 전이 시 composer 의 마지막 자모 1개를 pop 해 sequential 흔적을
    /// 지우고 직전 음절을 commit 한 뒤 atomic chord 를 inject 해야 한다. atomic → atomic
    /// (mid-chord) 갱신은 `inject_chord_syllable` 자체가 composer 큐를 비우고 재구성하므로 추가 처리 불필요.
    preview_is_atomic: bool,
}

impl ChordBuffer {
    pub fn new(window_ms: u16) -> Self {
        Self {
            window_ms,
            buffer: Vec::with_capacity(16),
            start: None,
            epoch: 0,
            next_order: 0,
            preview_injected: false,
            preview_is_atomic: false,
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

    /// 현재 버퍼에 항목이 있는지.
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

    /// 현재 버퍼의 ChordEntry 슬라이스를 반환 (preview 계산용).
    ///
    /// chord 진행 중에도 buffer 를 비파괴적으로 들여다보고 부분 결합 결과를
    /// preedit 에 미리 inject 하기 위해 사용한다.
    #[inline]
    pub fn peek_entries(&self) -> &[ChordEntry] {
        &self.buffer
    }

    /// preview 가 inject 되었는지 여부.
    ///
    /// `true` 이면 `apply_chord_entries` 호출자가 중복 처리를 피하도록 분기.
    #[inline]
    pub fn was_preview_injected(&self) -> bool {
        self.preview_injected
    }

    /// preview 상태를 갱신한다.
    ///
    /// `update_chord_preview` 가 Case A 결과를 inject 한 직후 `true`,
    /// 비-Case-A(B/C) 로 전이되어 preview 를 폐기할 때 `false` 로 호출.
    #[inline]
    pub fn mark_preview_injected(&mut self, v: bool) {
        self.preview_injected = v;
    }

    /// 마지막 preview 가 atomic(다중 키 inject)이었는지 반환.
    #[inline]
    pub fn was_preview_atomic(&self) -> bool {
        self.preview_is_atomic
    }

    /// 마지막 preview 의 atomic 여부를 갱신한다.
    ///
    /// - `true`: `inject_chord_syllable` 호출 직후 (다중 키).
    /// - `false`: `process_jamo_with_meta` 호출 직후 (단일 키 sequential) 또는 preview 폐기.
    #[inline]
    pub fn mark_preview_atomic(&mut self, v: bool) {
        self.preview_is_atomic = v;
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
    pub fn push_jamo(&mut self, jamo: JamoEnum, meta: JamoMeta) -> Option<ChordFlushResult> {
        let entry = ChordEntry {
            kind: ChordEntryKind::Jamo(jamo),
            input_order: 0, // push_inner에서 할당
            meta,
        };
        self.push_inner(entry)
    }

    /// 비자모 문자를 버퍼에 push한다.
    ///
    /// chord 활성 시 윈도우에 합류. 만료 시 ChordEntry::NonJamo로 반환.
    /// chord OFF(`window_ms == 0`)이면 항상 `Some(vec![entry])` 즉시 반환.
    pub fn push_non_jamo(&mut self, c: char) -> Option<ChordFlushResult> {
        let entry = ChordEntry {
            kind: ChordEntryKind::NonJamo(c),
            input_order: 0, // push_inner에서 할당
            meta: JamoMeta::default(),
        };
        self.push_inner(entry)
    }

    /// 내부 push 헬퍼 — 윈도우 만료/MAX_SIZE 로직 공유.
    ///
    /// input_order를 자동 부여한 뒤 버퍼에 추가한다.
    fn push_inner(&mut self, mut entry: ChordEntry) -> Option<ChordFlushResult> {
        // chord OFF → 단일 항목을 즉시 반환 (기존 즉시 처리와 동일)
        if self.window_ms == 0 {
            entry.input_order = 0;
            return Some(vec![entry]);
        }

        let now = Instant::now();

        // 윈도우 만료 여부 체크 (단일 윈도우: 첫 타건 시점 기준 절대 만료)
        let expired = self.start.is_some_and(|t| {
            now.duration_since(t).as_millis() >= self.window_ms as u128
        });

        if expired && !self.buffer.is_empty() {
            // 이전 chord flush → 새 chord 시작
            let flushed = std::mem::take(&mut self.buffer);
            self.epoch = self.epoch.wrapping_add(1);
            self.next_order = 0;
            self.preview_injected = false;
            self.preview_is_atomic = false;
            entry.input_order = self.next_order;
            self.next_order = self.next_order.wrapping_add(1);
            self.buffer.push(entry);
            self.start = Some(now);
            return Some(flushed);
        }

        // 버퍼가 MAX에 도달했으면 즉시 flush (현재 항목 포함)
        if self.buffer.len() >= CHORD_BUFFER_MAX {
            let mut flushed = std::mem::take(&mut self.buffer);
            entry.input_order = self.next_order;
            flushed.push(entry);
            self.epoch = self.epoch.wrapping_add(1);
            self.next_order = 0;
            self.start = None;
            self.preview_injected = false;
            self.preview_is_atomic = false;
            return Some(flushed);
        }

        // chord 진행 중: 버퍼에 추가
        if self.start.is_none() {
            // 새 chord 시작 — epoch 증가, next_order 리셋
            self.epoch = self.epoch.wrapping_add(1);
            self.next_order = 0;
            self.preview_injected = false;
            self.preview_is_atomic = false;
            self.start = Some(now);
        }
        entry.input_order = self.next_order;
        self.next_order = self.next_order.wrapping_add(1);
        self.buffer.push(entry);
        None
    }

    /// 버퍼를 강제 flush (비-자모 키, Space, Enter 등).
    ///
    /// 버퍼가 비어 있으면 `None`.
    /// flush 후 epoch를 증가시켜 진행 중이던 idle flush 타이머를 무효화한다.
    pub fn force_flush(&mut self) -> Option<ChordFlushResult> {
        if self.buffer.is_empty() {
            self.preview_injected = false;
            self.preview_is_atomic = false;
            return None;
        }
        let flushed = std::mem::take(&mut self.buffer);
        self.epoch = self.epoch.wrapping_add(1);
        self.next_order = 0;
        self.start = None;
        self.preview_injected = false;
        self.preview_is_atomic = false;
        Some(flushed)
    }

    /// 버퍼 초기화 (reset/clear).
    ///
    /// epoch를 증가시켜 진행 중이던 idle flush 타이머를 무효화한다.
    pub fn clear(&mut self) {
        self.buffer.clear();
        self.start = None;
        self.next_order = 0;
        self.epoch = self.epoch.wrapping_add(1);
        self.preview_injected = false;
        self.preview_is_atomic = false;
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
        let result = buf.push_jamo(cho(Cho::G), JamoMeta::default());
        assert!(result.is_some(), "chord OFF → 즉시 flush");
        let entries = result.unwrap();
        assert_eq!(entries.len(), 1);
        assert!(matches!(
            entries[0].kind,
            ChordEntryKind::Jamo(JamoEnum::Cho(Cho::G))
        ));
    }

    /// chord ON → 자모 누적 후 force_flush
    #[test]
    fn chord_on_force_flush() {
        let mut buf = ChordBuffer::new(50);
        // ㄱ push → None (진행 중)
        assert!(buf.push_jamo(cho(Cho::G), JamoMeta::default()).is_none());
        assert_eq!(buf.len(), 1);
        // ㅏ push → None
        assert!(buf.push_jamo(jung(Jung::A), JamoMeta::default()).is_none());
        assert_eq!(buf.len(), 2);
        // force_flush → [ㄱ, ㅏ] (input_order 순서)
        let result = buf.force_flush().unwrap();
        assert_eq!(result.len(), 2);
        assert!(matches!(result[0].kind, ChordEntryKind::Jamo(JamoEnum::Cho(_))));
        assert!(matches!(result[1].kind, ChordEntryKind::Jamo(JamoEnum::Jung(_))));
        // input_order 확인
        assert_eq!(result[0].input_order, 0);
        assert_eq!(result[1].input_order, 1);
        assert!(!buf.has_pending());
    }

    /// input_order 보존: 입력 순서대로 0,1,2,...
    #[test]
    fn chord_input_order_preserved() {
        let mut buf = ChordBuffer::new(5000);
        buf.push_jamo(jong(Jong::Giyeok), JamoMeta::default());
        buf.push_jamo(cho(Cho::G), JamoMeta::default());
        buf.push_jamo(jung(Jung::A), JamoMeta::default());
        let result = buf.force_flush().unwrap();
        assert_eq!(result.len(), 3);
        // input_order는 입력 순서 그대로 (sort 없음 — Phase 3의 chord_compose가 담당)
        assert_eq!(result[0].input_order, 0);
        assert_eq!(result[1].input_order, 1);
        assert_eq!(result[2].input_order, 2);
        // 첫 번째는 Jong (입력 순서 보존)
        assert!(matches!(result[0].kind, ChordEntryKind::Jamo(JamoEnum::Jong(_))));
        assert!(matches!(result[1].kind, ChordEntryKind::Jamo(JamoEnum::Cho(_))));
        assert!(matches!(result[2].kind, ChordEntryKind::Jamo(JamoEnum::Jung(_))));
    }

    /// MAX_SIZE(16) 도달 시 즉시 flush.
    #[test]
    fn chord_max_size_flush() {
        let mut buf = ChordBuffer::new(5000); // 아주 긴 윈도우
        // 처음 16개: None (진행 중, 버퍼에 16개 누적)
        for i in 0..16 {
            let r = buf.push_jamo(cho(Cho::G), JamoMeta::default());
            assert!(r.is_none(), "push {}: 아직 flush 안 됨", i);
        }
        assert_eq!(buf.len(), 16, "버퍼에 16개 누적");
        // 17번째: len==16 >= MAX → 즉시 flush (16개) + 새 항목 포함 = 17개
        let r = buf.push_jamo(cho(Cho::H), JamoMeta::default());
        assert!(r.is_some(), "17번째 push → MAX flush");
        let entries = r.unwrap();
        assert_eq!(entries.len(), 17, "flush 결과: 기존 16 + 새 1 = 17");
        assert_eq!(buf.len(), 0, "flush 후 버퍼 비어 있음");
    }

    /// 빈 버퍼 force_flush → None
    #[test]
    fn chord_empty_force_flush() {
        let mut buf = ChordBuffer::new(50);
        assert!(buf.force_flush().is_none());
    }

    /// chord-Q1-keui: ㄱ ㅎ ㅡ ㅣ → input_order 순서로 반환
    #[test]
    fn chord_q1_keui_input_order() {
        let mut buf = ChordBuffer::new(5000);
        buf.push_jamo(cho(Cho::G), JamoMeta::default());
        buf.push_jamo(cho(Cho::H), JamoMeta::default());
        buf.push_jamo(jung(Jung::Eu), JamoMeta::default());
        buf.push_jamo(jung(Jung::I), JamoMeta::default());
        let result = buf.force_flush().unwrap();
        assert_eq!(result.len(), 4);
        // input_order 순서
        for (i, e) in result.iter().enumerate() {
            assert_eq!(e.input_order, i as u8);
        }
        // 모두 Jamo
        assert!(result.iter().all(|e| matches!(e.kind, ChordEntryKind::Jamo(..))));
    }

    // =========================================================================
    // Phase 2 신규 테스트
    // =========================================================================

    /// chord_push_non_jamo_buffer_within_window:
    /// chord 활성 + 비자모 push → 윈도우 안 누적 (None 반환), force_flush 시 ChordEntry로 반환.
    #[test]
    fn chord_push_non_jamo_buffer_within_window() {
        let mut buf = ChordBuffer::new(5000);
        // 비자모 '-' push → None (진행 중)
        let r = buf.push_non_jamo('-');
        assert!(r.is_none(), "chord 활성 + 비자모 → None (윈도우 내 누적)");
        assert_eq!(buf.len(), 1);
        // force_flush → NonJamo 항목 반환
        let entries = buf.force_flush().unwrap();
        assert_eq!(entries.len(), 1);
        assert!(matches!(entries[0].kind, ChordEntryKind::NonJamo('-')));
        assert_eq!(entries[0].input_order, 0);
    }

    /// chord_push_jamo_then_non_jamo:
    /// 혼합 input. 만료 시 entries 길이 2, 종류 [Jamo, NonJamo].
    #[test]
    fn chord_push_jamo_then_non_jamo() {
        let mut buf = ChordBuffer::new(5000);
        buf.push_jamo(cho(Cho::G), JamoMeta::default());
        buf.push_non_jamo('-');
        let entries = buf.force_flush().unwrap();
        assert_eq!(entries.len(), 2);
        assert!(matches!(entries[0].kind, ChordEntryKind::Jamo(JamoEnum::Cho(Cho::G))));
        assert!(matches!(entries[1].kind, ChordEntryKind::NonJamo('-')));
        assert_eq!(entries[0].input_order, 0);
        assert_eq!(entries[1].input_order, 1);
    }

    /// chord_max_size_16:
    /// 16 entries 누적 후 17번째 → 즉시 flush (비자모 포함).
    #[test]
    fn chord_max_size_16() {
        let mut buf = ChordBuffer::new(5000);
        // 8개 자모
        for _ in 0..8 {
            assert!(buf.push_jamo(cho(Cho::G), JamoMeta::default()).is_none());
        }
        // 8개 비자모
        for _ in 0..8 {
            assert!(buf.push_non_jamo('a').is_none());
        }
        assert_eq!(buf.len(), 16, "16개 누적");
        // 17번째 → 즉시 flush
        let r = buf.push_jamo(cho(Cho::H), JamoMeta::default());
        assert!(r.is_some());
        let entries = r.unwrap();
        assert_eq!(entries.len(), 17);
        assert_eq!(buf.len(), 0);
    }

    /// chord_off_non_jamo_immediate_commit:
    /// chord OFF (window=0) + 비자모 → 즉시 반환 (Some).
    #[test]
    fn chord_off_non_jamo_immediate_commit() {
        let mut buf = ChordBuffer::new(0);
        let r = buf.push_non_jamo('-');
        assert!(r.is_some(), "chord OFF + 비자모 → 즉시 반환");
        let entries = r.unwrap();
        assert_eq!(entries.len(), 1);
        assert!(matches!(entries[0].kind, ChordEntryKind::NonJamo('-')));
    }

    // =========================================================================
    // epoch 검증 테스트
    // =========================================================================

    /// epoch: 초기값 0, 첫 자모 push → epoch 1
    #[test]
    fn chord_epoch_increments_on_first_push() {
        let mut buf = ChordBuffer::new(50);
        assert_eq!(buf.current_epoch(), 0);
        buf.push_jamo(cho(Cho::G), JamoMeta::default());
        assert_eq!(buf.current_epoch(), 1, "첫 push → epoch 증가");
    }

    /// epoch: clear() 후 epoch 증가 → idle flush 타이머 무효화
    #[test]
    fn chord_epoch_increments_on_clear() {
        let mut buf = ChordBuffer::new(50);
        buf.push_jamo(cho(Cho::G), JamoMeta::default());
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
        buf.push_jamo(cho(Cho::G), JamoMeta::default());
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
        buf.push_jamo(cho(Cho::G), JamoMeta::default());
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
        buf.push_jamo(cho(Cho::G), JamoMeta::default());
        let start_epoch = buf.current_epoch();
        // 두 번째 자모 — epoch 변화 없어야 함 (새 chord 시작이 아니므로)
        buf.push_jamo(jung(Jung::A), JamoMeta::default());
        assert_eq!(buf.current_epoch(), start_epoch, "sliding window 아님 — epoch 불변");
        assert_eq!(buf.len(), 2);
    }
}
