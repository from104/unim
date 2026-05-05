// tests_chord.rs
//! chord_window_ms (옵션 2) 통합 테스트.
//!
//! ## 테스트 전략
//!
//! chord 경로는 `InputEngine` 레벨에서 `chord_buffer.window_ms`를 직접 주입해 테스트한다.
//! - `window_ms=0`: 기존 즉시 처리 (회귀 0)
//! - `window_ms=5000`: 아주 긴 윈도우로 만료 없이 버퍼 동작 검증
//!
//! C-1 / C-2 시나리오는 force_flush를 통해 음절 경계를 명시적으로 제어한다.

#[cfg(test)]
mod tests {
    use crate::config::Config;
    use crate::hangul::composer::JamoMeta;
    use crate::hangul::jamo::{Cho, Jong, Jung, JamoEnum};
    use crate::input_engine::chord_buffer::JamoEntry;
    use crate::input_engine::InputEngine;

    /// chord OFF 엔진 생성 (기본 두벌식 — supports_moachigi=false → chord OFF)
    fn engine_chord_off() -> InputEngine {
        InputEngine::new(&Config::default())
    }

    /// 안마태 자판 + chord 강제 활성 (window=5000ms)
    fn engine_anmatae_chord(window_ms: u16) -> InputEngine {
        let mut config = Config::default();
        config.engine.korean.layout = "ko_3bul_anmatae".to_string();
        config.engine.korean.bidirectional_combine = Some(true);
        config.engine.korean.chord_window_ms = Some(window_ms);
        let mut e = InputEngine::new(&config);
        // 한국어 모드로 전환
        e.set_input_category(crate::config::InputCategory::Korean);
        e
    }

    /// chord 버퍼에 자모 목록을 직접 주입 후 flush해 commit을 얻는 헬퍼.
    /// `force_flush` 시뮬레이션 — 실제 키 경로를 거치지 않고 apply_chord_entries를 직접 테스트.
    fn flush_entries(engine: &mut InputEngine, entries: Vec<JamoEntry>) -> String {
        engine.apply_chord_entries(entries);
        // commit_buffer 내용 수집 후 초기화
        let out = engine.commit_str().to_string();
        engine.clear_commit();
        // preedit도 flush (음절 완성)
        if engine.is_composing() {
            engine.flush_preedit();
            let preedit_commit = engine.commit_str().to_string();
            engine.clear_commit();
            format!("{}{}", out, preedit_commit)
        } else {
            out
        }
    }

    fn entry(jamo: JamoEnum) -> JamoEntry {
        JamoEntry { jamo, meta: JamoMeta::default() }
    }

    // =========================================================================
    // chord OFF 회귀 (기존 동작 100% 동일)
    // =========================================================================

    /// chord-off: window_ms=0 → chord_buffer.is_active()=false → 즉시 처리
    #[test]
    fn chord_off_engine_is_not_active() {
        let e = engine_chord_off();
        assert!(!e.chord_buffer.is_active(), "두벌식은 chord OFF");
    }

    /// chord-off: 안마태 자판 + window_ms=0 → chord OFF
    #[test]
    fn chord_off_anmatae_zero_window() {
        let e = engine_anmatae_chord(0);
        assert!(!e.chord_buffer.is_active(), "window=0 → chord OFF");
    }

    /// chord-on: 안마태 자판 + window_ms=50 → chord 활성
    #[test]
    fn chord_on_anmatae_fifty() {
        let e = engine_anmatae_chord(50);
        assert!(e.chord_buffer.is_active(), "window=50 → chord ON");
    }

    // =========================================================================
    // apply_chord_entries 단위 테스트
    // =========================================================================

    /// chord-50-syllable: ㄱ ㅏ → "가"
    #[test]
    fn chord_syllable_ga() {
        let mut e = engine_anmatae_chord(5000);
        let entries = vec![
            entry(JamoEnum::Cho(Cho::G)),
            entry(JamoEnum::Jung(Jung::A)),
        ];
        let result = flush_entries(&mut e, entries);
        assert_eq!(result, "가", "ㄱ+ㅏ chord → 가");
    }

    /// chord-50-syllable: ㄱ ㅏ ᆷ → "감"
    #[test]
    fn chord_syllable_gam() {
        let mut e = engine_anmatae_chord(5000);
        let entries = vec![
            entry(JamoEnum::Cho(Cho::G)),
            entry(JamoEnum::Jung(Jung::A)),
            entry(JamoEnum::Jong(Jong::Mieum)),
        ];
        let result = flush_entries(&mut e, entries);
        assert_eq!(result, "감", "ㄱ+ㅏ+ᆷ chord → 감");
    }

    // =========================================================================
    // C-1: "그히" vs "킈" 해결 검증
    // =========================================================================

    /// chord-Q1-keui: ㄱ ㅎ ㅡ ㅣ → 50ms 안 → buffer 영역 분류 Cho[ㄱ,ㅎ] Jung[ㅡ,ㅣ]
    /// → 안마태 combinations: (ㄱ,ㅎ)→ㅋ, (ㅡ,ㅣ)→ㅢ → "킈"
    #[test]
    fn chord_q1_keui() {
        let mut e = engine_anmatae_chord(5000);
        // 영역 정렬: Cho 먼저 → Jung 순
        let entries = vec![
            entry(JamoEnum::Cho(Cho::G)),
            entry(JamoEnum::Cho(Cho::H)),
            entry(JamoEnum::Jung(Jung::Eu)),
            entry(JamoEnum::Jung(Jung::I)),
        ];
        let result = flush_entries(&mut e, entries);
        assert_eq!(result, "킈", "ㄱ+ㅎ+ㅡ+ㅣ chord → 킈");
    }

    /// chord-Q1-geuhi: ㄱ ㅡ (첫 chord) → "그" + ㅎ ㅣ (두 번째 chord) → "히" → "그히"
    #[test]
    fn chord_q1_geuhi() {
        let mut e = engine_anmatae_chord(5000);

        // 첫 chord: ㄱ ㅡ → "그"
        let entries1 = vec![
            entry(JamoEnum::Cho(Cho::G)),
            entry(JamoEnum::Jung(Jung::Eu)),
        ];
        let r1 = flush_entries(&mut e, entries1);
        assert_eq!(r1, "그", "첫 chord: 그");

        // 두 번째 chord: ㅎ ㅣ → "히"
        let entries2 = vec![
            entry(JamoEnum::Cho(Cho::H)),
            entry(JamoEnum::Jung(Jung::I)),
        ];
        let r2 = flush_entries(&mut e, entries2);
        assert_eq!(r2, "히", "두 번째 chord: 히");
    }

    // =========================================================================
    // C-2: "구하다" 해결 검증
    // =========================================================================

    /// chord-Q2-guhada: ㄱ ㅜ → "구" + ㅎ ㅏ → "하" + ㄷ ㅏ → "다" → "구하다"
    #[test]
    fn chord_q2_guhada() {
        let mut e = engine_anmatae_chord(5000);

        let chords: &[(&[JamoEnum], &str)] = &[
            (&[JamoEnum::Cho(Cho::G), JamoEnum::Jung(Jung::U)], "구"),
            (&[JamoEnum::Cho(Cho::H), JamoEnum::Jung(Jung::A)], "하"),
            (&[JamoEnum::Cho(Cho::D), JamoEnum::Jung(Jung::A)], "다"),
        ];

        let mut total = String::new();
        for (jamos, expected_syllable) in chords {
            let entries: Vec<JamoEntry> = jamos.iter().map(|j| entry(*j)).collect();
            let result = flush_entries(&mut e, entries);
            assert_eq!(&result, expected_syllable, "chord 음절 불일치");
            total.push_str(&result);
        }
        assert_eq!(total, "구하다", "chord-Q2: 구하다");
    }

    // =========================================================================
    // 종속성 테스트
    // =========================================================================

    /// chord-O1-off-O2-on: bidirectional_combine=false + chord_window_ms=50 → chord OFF
    #[test]
    fn chord_o1_off_o2_on() {
        let mut config = Config::default();
        config.engine.korean.layout = "ko_3bul_anmatae".to_string();
        config.engine.korean.bidirectional_combine = Some(false); // O1 OFF
        config.engine.korean.chord_window_ms = Some(50);          // O2 설정
        let e = InputEngine::new(&config);
        assert!(!e.chord_buffer.is_active(), "O1 OFF → chord 무시");
    }

    /// chord-supports-false: 두벌식(supports_moachigi=false) + chord_window_ms=50 → OFF
    #[test]
    fn chord_supports_false() {
        let mut config = Config::default();
        config.engine.korean.layout = "ko_2bulstd".to_string();
        config.engine.korean.chord_window_ms = Some(50);
        let e = InputEngine::new(&config);
        assert!(!e.chord_buffer.is_active(), "supports_moachigi=false → chord OFF");
    }

    // =========================================================================
    // idle flush API 테스트 — chord_idle_flush_commit() + chord_pending_info()
    //
    // 참고: 실제 tokio::time::sleep 기반 idle flush 타이머는 integration 테스트
    // (unim-daemon 레벨)에서 검증해야 하므로, 여기선 엔진 레벨 API의 정합성을
    // 검증한다. sleep 없이 직접 push 후 chord_idle_flush_commit() 호출.
    // =========================================================================

    /// idle-flush-single: ㄱ push → chord_pending_info Some → idle_flush_commit → "ㄱ"
    ///
    /// 사용자 명세: 첫 타건 후 N ms 안에 추가 키 없으면 단일 자모 일반 풀어쓰기 commit.
    /// chord_idle_flush_commit() 이 이 동작을 구현.
    #[test]
    fn idle_flush_single() {
        let mut e = engine_anmatae_chord(50);
        // ㄱ 단독 push → chord 진행 중 (버퍼에 1개)
        let push_result = e.chord_buffer.push(
            JamoEnum::Cho(Cho::G),
            JamoMeta::default(),
        );
        assert!(push_result.is_none(), "chord 진행 중 — None 반환");
        assert!(e.chord_pending_info().is_some(), "chord_pending_info Some");

        // idle flush 시뮬레이션 (실제 타이머 없이 직접 호출)
        let commit = e.chord_idle_flush_commit();
        assert_eq!(commit.as_deref(), Some("ㄱ"), "단일 자모 → 'ㄱ' commit");
        assert!(e.chord_pending_info().is_none(), "flush 후 pending 없음");
    }

    /// idle-flush-syllable: ㄱ ㅏ ᆷ push → idle_flush_commit → "감"
    ///
    /// 사용자 명세: 윈도우 안에 3개 자모 push → 만료 시 묶음처리 → "감"
    #[test]
    fn idle_flush_syllable() {
        let mut e = engine_anmatae_chord(50);
        // 영역 정렬: Cho, Jung, Jong 순으로 직접 push
        e.chord_buffer.push(JamoEnum::Cho(Cho::G), JamoMeta::default());
        e.chord_buffer.push(JamoEnum::Jung(Jung::A), JamoMeta::default());
        e.chord_buffer.push(JamoEnum::Jong(Jong::Mieum), JamoMeta::default());
        assert!(e.chord_pending_info().is_some(), "3자모 대기 중");

        let commit = e.chord_idle_flush_commit();
        assert_eq!(commit.as_deref(), Some("감"), "ㄱ+ㅏ+ᆷ → '감' commit");
    }

    /// idle-flush-cancel-on-reset: push 후 reset() → chord_pending_info None (epoch 무효)
    ///
    /// reset() 은 chord_buffer.clear() 포함 → epoch 증가 → 이전 타이머 무효.
    #[test]
    fn idle_flush_cancel_on_reset() {
        let mut e = engine_anmatae_chord(50);
        e.chord_buffer.push(JamoEnum::Cho(Cho::G), JamoMeta::default());
        let (epoch_before, _) = e.chord_pending_info().unwrap();

        // reset() → chord_buffer.clear() → epoch 증가
        e.reset();
        assert!(e.chord_pending_info().is_none(), "reset 후 pending 없음");
        assert!(!e.chord_epoch_valid(epoch_before), "이전 epoch 무효");
    }

    /// idle-flush-cancel-on-focusout: chord_idle_flush_commit()은 FocusOut 경로와 동일 구현.
    ///
    /// reset_engine_and_capture_commit()에서 chord_idle_flush_commit() 먼저 호출 →
    /// commit 텍스트에 chord 결과 포함됨을 검증.
    #[test]
    fn idle_flush_cancel_on_focusout() {
        let mut e = engine_anmatae_chord(50);
        e.chord_buffer.push(JamoEnum::Cho(Cho::G), JamoMeta::default());
        e.chord_buffer.push(JamoEnum::Jung(Jung::A), JamoMeta::default());
        assert!(e.chord_pending_info().is_some());

        // chord_idle_flush_commit() = FocusOut 경로의 chord 처리와 동일 호출
        let commit = e.chord_idle_flush_commit();
        assert_eq!(commit.as_deref(), Some("가"), "FocusOut → chord flush → '가'");
        assert!(e.chord_pending_info().is_none(), "flush 후 pending 없음");
    }

    /// idle-flush-cancel-on-layout-change: 레이아웃 변경 시 chord_buffer.clear() → epoch 증가
    ///
    /// engine.rs rebuild_korean_context / set_korean_layout 에서 chord_buffer.clear() 호출.
    #[test]
    fn idle_flush_cancel_on_layout_change() {
        let mut e = engine_anmatae_chord(50);
        e.chord_buffer.push(JamoEnum::Cho(Cho::G), JamoMeta::default());
        let (epoch_before, _) = e.chord_pending_info().unwrap();

        // 레이아웃 변경 — chord_buffer.clear() + epoch 증가
        let mut config = Config::default();
        config.engine.korean.layout = "ko_2bulstd".to_string();
        e.rebuild_korean_context(&config);

        assert!(e.chord_pending_info().is_none(), "레이아웃 변경 후 pending 없음");
        assert!(!e.chord_epoch_valid(epoch_before), "이전 epoch 무효");
    }

    /// idle-flush-window-stable: 단일 윈도우 검증 — 두 번째 자모 push 후 epoch 불변
    ///
    /// 사용자 명세 "첫 타건 후 N ms": start 시점은 첫 자모에 고정.
    /// sliding window 아님 → 두 번째 자모가 와도 epoch 변화 없음.
    #[test]
    fn idle_flush_window_stable() {
        let mut e = engine_anmatae_chord(50);
        e.chord_buffer.push(JamoEnum::Cho(Cho::G), JamoMeta::default());
        let (epoch_after_first, _) = e.chord_pending_info().unwrap();

        // 두 번째 자모: epoch 변화 없어야 함 (단일 윈도우)
        e.chord_buffer.push(JamoEnum::Jung(Jung::A), JamoMeta::default());
        let (epoch_after_second, _) = e.chord_pending_info().unwrap();
        assert_eq!(
            epoch_after_first, epoch_after_second,
            "단일 윈도우: 두 번째 자모 후 epoch 불변 (sliding 아님)"
        );
        assert!(e.chord_epoch_valid(epoch_after_first), "동일 epoch 유효");
    }

    /// idle-flush-many-keys: ㄱ ㅎ ㅡ ㅣ push → idle_flush_commit → "킈"
    ///
    /// 사용자 명세: 4개 자모 50ms 안 → 5번째 키 없어도 50ms 후 자동 "킈" commit.
    /// 여기서는 sleep 없이 직접 flush 호출로 결과 검증.
    #[test]
    fn idle_flush_many_keys() {
        let mut e = engine_anmatae_chord(50);
        // 영역 정렬 적용: Cho, Cho, Jung, Jung
        e.chord_buffer.push(JamoEnum::Cho(Cho::G), JamoMeta::default());
        e.chord_buffer.push(JamoEnum::Cho(Cho::H), JamoMeta::default());
        e.chord_buffer.push(JamoEnum::Jung(Jung::Eu), JamoMeta::default());
        e.chord_buffer.push(JamoEnum::Jung(Jung::I), JamoMeta::default());
        assert!(e.chord_pending_info().is_some(), "4자모 대기 중");

        let commit = e.chord_idle_flush_commit();
        assert_eq!(commit.as_deref(), Some("킈"), "ㄱ+ㅎ+ㅡ+ㅣ → '킈' commit");
    }
}
