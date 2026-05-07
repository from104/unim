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
    use crate::input_engine::chord_compose::{ChordEntry, ChordEntryKind};
    use crate::input_engine::InputEngine;
    use crate::keycode::{KeyCode, ModifierState};

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

    /// chord 버퍼에 항목 목록을 직접 주입 후 flush해 commit을 얻는 헬퍼.
    /// `force_flush` 시뮬레이션 — 실제 키 경로를 거치지 않고 apply_chord_entries를 직접 테스트.
    fn flush_entries(engine: &mut InputEngine, entries: Vec<ChordEntry>) -> String {
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

    fn entry(jamo: JamoEnum) -> ChordEntry {
        ChordEntry {
            kind: ChordEntryKind::Jamo(jamo),
            input_order: 0,
            meta: JamoMeta::default(),
        }
    }

    fn non_jamo_entry(c: char) -> ChordEntry {
        ChordEntry {
            kind: ChordEntryKind::NonJamo(c),
            input_order: 0,
            meta: JamoMeta::default(),
        }
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

    /// chord-bidirectional-reverse-cho: 역순 ㅎ+ㄱ → ㅋ.
    ///
    /// 회귀 가드 — Phase 7 와이어링 누락 버그(build_korean_context가 사용자 config의
    /// bidirectional_combine을 composer.moachigi에 주입하지 않던 문제) 재발 방지.
    /// 정순 (ㄱ,ㅎ)→ㅋ 만 keymap에 정의되어 있고, 역순 (ㅎ,ㄱ)는 composer의
    /// bidirectional_combine retry 경로가 활성화되어야만 ㅋ로 결합된다.
    /// 본 테스트가 실패하면 user config → composer 와이어링 회귀.
    #[test]
    fn chord_bidirectional_reverse_cho_h_g() {
        let mut e = engine_anmatae_chord(5000);
        // 역순 입력: ㅎ → ㄱ → ㅏ. 정순 (ㄱ,ㅎ)→ㅋ 키맵 정의를 (ㅎ,ㄱ) 로 역참조.
        let entries = vec![
            entry(JamoEnum::Cho(Cho::H)),
            entry(JamoEnum::Cho(Cho::G)),
            entry(JamoEnum::Jung(Jung::A)),
        ];
        let result = flush_entries(&mut e, entries);
        assert_eq!(
            result, "카",
            "역순 ㅎ+ㄱ → bidirectional retry → ㅋ + ㅏ = 카"
        );
    }

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
            let entries: Vec<ChordEntry> = jamos.iter().map(|j| entry(*j)).collect();
            let result = flush_entries(&mut e, entries);
            assert_eq!(&result, expected_syllable, "chord 음절 불일치");
            total.push_str(&result);
        }
        assert_eq!(total, "구하다", "chord-Q2: 구하다");
    }

    // =========================================================================
    // 종속성 테스트
    // =========================================================================

    /// chord-O1-off-O2-on: bidirectional_combine=false + chord_window_ms=50 → chord ON
    ///
    /// Phase 5a~: chord_window_ms 와 bidirectional_combine 은 독립 게이트.
    /// chord 타이밍 윈도우(ChordBuffer) 활성화는 chord_window_ms > 0 만으로 결정.
    /// bidirectional_combine=false 는 composer retry/chord_compose permutation 게이트이며
    /// ChordBuffer 활성화와 무관하다.
    #[test]
    fn chord_o1_off_o2_on() {
        let mut config = Config::default();
        config.engine.korean.layout = "ko_3bul_anmatae".to_string();
        config.engine.korean.bidirectional_combine = Some(false); // O1 OFF (bidir 게이트만)
        config.engine.korean.chord_window_ms = Some(50);          // O2 설정 → chord ON
        let e = InputEngine::new(&config);
        assert!(e.chord_buffer.is_active(), "chord_window_ms=50 → chord ON (bidir과 독립)");
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

    /// idle-flush-single: ㄱ push → chord_pending_info Some → idle_flush_pending → preedit='ㄱ'
    ///
    /// 사용자 명세 v4 후속: chord_window 만료 = preedit 갱신만, commit 안 함.
    /// 단일 자모 ㄱ 은 풀어쓰기로 composer 에 push 되어 preedit 유지 → 후속 키와 결합 가능.
    #[test]
    fn idle_flush_single() {
        let mut e = engine_anmatae_chord(50);
        // ㄱ 단독 push → chord 진행 중 (버퍼에 1개)
        let push_result = e.chord_buffer.push_jamo(
            JamoEnum::Cho(Cho::G),
            JamoMeta::default(),
        );
        assert!(push_result.is_none(), "chord 진행 중 — None 반환");
        assert!(e.chord_pending_info().is_some(), "chord_pending_info Some");

        // idle flush 시뮬레이션 (preedit 유지 모드)
        let (commit, preedit) = e.chord_idle_flush_pending();
        assert!(commit.is_none(), "단일 자모는 commit 안 함 (preedit 유지)");
        assert_eq!(preedit, "ㄱ", "preedit='ㄱ' 유지");
        assert!(e.chord_pending_info().is_none(), "flush 후 pending 없음");
    }

    /// idle-flush-syllable: ㄱ ㅏ ᆷ push → idle_flush_pending → preedit='감'
    ///
    /// 사용자 명세: 윈도우 안에 3개 자모 → 만료 시 묶음처리 → '감' preedit (commit 안 함).
    /// 한자 변환 등 후속 처리 가능 상태 유지.
    #[test]
    fn idle_flush_syllable() {
        let mut e = engine_anmatae_chord(50);
        // 영역 정렬: Cho, Jung, Jong 순으로 직접 push
        e.chord_buffer.push_jamo(JamoEnum::Cho(Cho::G), JamoMeta::default());
        e.chord_buffer.push_jamo(JamoEnum::Jung(Jung::A), JamoMeta::default());
        e.chord_buffer.push_jamo(JamoEnum::Jong(Jong::Mieum), JamoMeta::default());
        assert!(e.chord_pending_info().is_some(), "3자모 대기 중");

        let (commit, preedit) = e.chord_idle_flush_pending();
        assert!(commit.is_none(), "모아쓰기 결과는 commit 안 함 (preedit 유지)");
        assert_eq!(preedit, "감", "ㄱ+ㅏ+ᆷ → '감' preedit");
    }

    /// idle-flush-cancel-on-reset: push 후 reset() → chord_pending_info None (epoch 무효)
    ///
    /// reset() 은 chord_buffer.clear() 포함 → epoch 증가 → 이전 타이머 무효.
    #[test]
    fn idle_flush_cancel_on_reset() {
        let mut e = engine_anmatae_chord(50);
        e.chord_buffer.push_jamo(JamoEnum::Cho(Cho::G), JamoMeta::default());
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
        e.chord_buffer.push_jamo(JamoEnum::Cho(Cho::G), JamoMeta::default());
        e.chord_buffer.push_jamo(JamoEnum::Jung(Jung::A), JamoMeta::default());
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
        e.chord_buffer.push_jamo(JamoEnum::Cho(Cho::G), JamoMeta::default());
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
        e.chord_buffer.push_jamo(JamoEnum::Cho(Cho::G), JamoMeta::default());
        let (epoch_after_first, _) = e.chord_pending_info().unwrap();

        // 두 번째 자모: epoch 변화 없어야 함 (단일 윈도우)
        e.chord_buffer.push_jamo(JamoEnum::Jung(Jung::A), JamoMeta::default());
        let (epoch_after_second, _) = e.chord_pending_info().unwrap();
        assert_eq!(
            epoch_after_first, epoch_after_second,
            "단일 윈도우: 두 번째 자모 후 epoch 불변 (sliding 아님)"
        );
        assert!(e.chord_epoch_valid(epoch_after_first), "동일 epoch 유효");
    }

    /// idle-flush-many-keys: ㄱ ㅎ ㅡ ㅣ push → idle_flush_pending → preedit='킈'
    ///
    /// 사용자 명세: 4개 자모 50ms 안 → 5번째 키 없어도 50ms 후 자동 '킈' preedit (commit 안 함).
    #[test]
    fn idle_flush_many_keys() {
        let mut e = engine_anmatae_chord(50);
        // 영역 정렬 적용: Cho, Cho, Jung, Jung
        e.chord_buffer.push_jamo(JamoEnum::Cho(Cho::G), JamoMeta::default());
        e.chord_buffer.push_jamo(JamoEnum::Cho(Cho::H), JamoMeta::default());
        e.chord_buffer.push_jamo(JamoEnum::Jung(Jung::Eu), JamoMeta::default());
        e.chord_buffer.push_jamo(JamoEnum::Jung(Jung::I), JamoMeta::default());
        assert!(e.chord_pending_info().is_some(), "4자모 대기 중");

        let (commit, preedit) = e.chord_idle_flush_pending();
        assert!(commit.is_none(), "모아쓰기 결과는 commit 안 함 (preedit 유지)");
        assert_eq!(preedit, "킈", "ㄱ+ㅎ+ㅡ+ㅣ → '킈' preedit");
    }

    // =========================================================================
    // Phase 5 fix: 한자 키 chord flush 보완
    // =========================================================================

    /// chord-flush-on-hanja: chord 진행 중(ㄱ ㅏ 버퍼) 한자 키 → "가" commit + 한자 변환 진입.
    ///
    /// press_key.rs hanja_keys dispatch 직전에 force_flush + apply가 추가되어,
    /// chord 버퍼가 활성 상태에서 한자 키가 들어와도 현재 chord를 먼저 음절로 확정한다.
    /// (chord 진행 중 preedit 무표시는 사용자 결정 — chord 끝나고 표시해도 충분)
    #[test]
    fn chord_flush_on_hanja() {
        let config = {
            let mut c = Config::default();
            c.engine.korean.layout = "ko_3bul_anmatae".to_string();
            c.engine.korean.bidirectional_combine = Some(true);
            c.engine.korean.chord_window_ms = Some(5000);
            c
        };
        let mut e = InputEngine::new(&config);
        e.set_input_category(crate::config::InputCategory::Korean);

        let m = ModifierState::default();

        // chord 버퍼에 ㄱ (R) + ㅏ (K) 를 push — press_key 경로 사용
        // 안마태 자판에서 R=ㄱ(Cho), K=ㅏ(Jung) 매핑 (anmatae 프로필 기준)
        // 직접 push 방식으로 chord 버퍼 활성 상태 만들기
        e.chord_buffer.push_jamo(JamoEnum::Cho(Cho::G), JamoMeta::default());
        e.chord_buffer.push_jamo(JamoEnum::Jung(Jung::A), JamoMeta::default());
        assert!(e.chord_pending_info().is_some(), "chord 버퍼 활성 (ㄱ+ㅏ 대기)");

        // 한자 키 press → force_flush → "가" 조합 → start_hanja_conversion
        let result = e.press_key(KeyCode::Hanja, m, &config);

        // hanja_mode 진입 확인 (hanja_mode=true → ShowHanja 결과)
        assert!(
            e.hanja_mode,
            "한자 키 후 hanja_mode=true (한자 변환 진입)"
        );

        // chord flush로 "가"가 preedit에 남았거나 commit에 들어왔어야 함
        // start_hanja_conversion은 preedit "가"를 후보로 올리므로 preedit는 "가"
        let preedit = e.preedit_str();
        assert_eq!(preedit, "가", "chord flush 후 한자 변환 대상은 '가'");

        let _ = result; // consumed() 여부는 구현 내부
    }

    // =========================================================================
    // Phase 7 신규: opt-config-none-default-off / opt-config-some-on /
    //               opt-supports-false-ignore
    // =========================================================================

    /// opt-config-none-default-off: 안마태 자판 + 사용자 config 두 값 None → chord OFF.
    ///
    /// Phase 7 OPT-IN: 자판 선택만으로 모아치기 자동 활성화 안 됨.
    /// 사용자가 명시적으로 bidirectional_combine=Some(true) + chord_window_ms=Some(N)
    /// 설정해야만 chord ON.
    #[test]
    fn opt_config_none_default_off() {
        let mut config = Config::default();
        config.engine.korean.layout = "ko_3bul_anmatae".to_string();
        // 사용자 config: 두 값 모두 None (명시 활성화 없음)
        config.engine.korean.bidirectional_combine = None;
        config.engine.korean.chord_window_ms = None;
        let window = InputEngine::compute_chord_window_ms(&config);
        assert_eq!(window, 0, "사용자 config None → chord OFF (OPT-IN 디폴트)");
        let e = InputEngine::new(&config);
        assert!(!e.chord_buffer.is_active(), "chord OFF 확인");
    }

    /// opt-config-some-on: 안마태 자판 + 사용자 config 두 값 Some → chord 활성.
    ///
    /// Phase 7 OPT-IN: 사용자가 명시 활성화 시 chord ON.
    #[test]
    fn opt_config_some_on() {
        let mut config = Config::default();
        config.engine.korean.layout = "ko_3bul_anmatae".to_string();
        config.engine.korean.bidirectional_combine = Some(true);
        config.engine.korean.chord_window_ms = Some(50);
        let window = InputEngine::compute_chord_window_ms(&config);
        assert_eq!(window, 50, "사용자 config Some(50) → chord_window=50");
        let e = InputEngine::new(&config);
        assert!(e.chord_buffer.is_active(), "chord ON 확인");
    }

    /// opt-supports-false-ignore: supports_moachigi=false 자판 + 사용자 config Some → 무시.
    ///
    /// Phase 7: supports_moachigi=false이면 capability 게이트에서 강제 OFF.
    /// 사용자 config 값이 Some이어도 무시됨.
    #[test]
    fn opt_supports_false_ignore() {
        let mut config = Config::default();
        // ko_2bulstd: supports_moachigi=false
        config.engine.korean.layout = "ko_2bulstd".to_string();
        config.engine.korean.bidirectional_combine = Some(true);
        config.engine.korean.chord_window_ms = Some(50);
        let window = InputEngine::compute_chord_window_ms(&config);
        assert_eq!(window, 0, "supports_moachigi=false → 사용자 config 무시, chord OFF");
        let e = InputEngine::new(&config);
        assert!(!e.chord_buffer.is_active(), "chord OFF 확인");
    }

    // =========================================================================
    // Phase 2 신규: 비자모 수용 + press_key 라우팅
    // =========================================================================

    /// chord_non_jamo_commit_with_jamo:
    /// chord 활성 + 자모 + 비자모 혼합 apply_chord_entries → 자모 음절 + 비자모 commit 순서 검증.
    ///
    /// entries: [Jamo(ㄱ), Jamo(ㅏ), NonJamo('-')]
    /// 기대: "가-" (자모 음절 후 비자모 commit)
    #[test]
    fn chord_non_jamo_commit_with_jamo() {
        let mut e = engine_anmatae_chord(5000);
        let entries = vec![
            entry(JamoEnum::Cho(Cho::G)),
            entry(JamoEnum::Jung(Jung::A)),
            non_jamo_entry('-'),
        ];
        let result = flush_entries(&mut e, entries);
        assert_eq!(result, "가-", "ㄱ+ㅏ+'-' → 가-");
    }

    /// chord_non_jamo_only_commit:
    /// chord 활성 + 비자모만 apply_chord_entries → 비자모 그대로 commit.
    #[test]
    fn chord_non_jamo_only_commit() {
        let mut e = engine_anmatae_chord(5000);
        let entries = vec![non_jamo_entry('a'), non_jamo_entry('b')];
        let result = flush_entries(&mut e, entries);
        assert_eq!(result, "ab", "비자모만 → ab commit");
    }

    /// chord_off_non_jamo_immediate:
    /// chord OFF (window=0) + 비자모 push_non_jamo → 즉시 반환 (Some).
    ///
    /// chord_buffer API 레벨 검증: chord OFF 시 push_non_jamo → Some([NonJamo]) 즉시 반환.
    #[test]
    fn chord_off_non_jamo_immediate() {
        use crate::input_engine::chord_buffer::ChordBuffer;
        let mut buf = ChordBuffer::new(0); // chord OFF
        let r = buf.push_non_jamo('-');
        assert!(r.is_some(), "chord OFF + 비자모 push_non_jamo → Some 즉시 반환");
        let entries = r.unwrap();
        assert_eq!(entries.len(), 1);
        assert!(matches!(entries[0].kind, ChordEntryKind::NonJamo('-')));
    }

    /// chord_max_size_16_mixed:
    /// 자모 8개 + 비자모 8개 = 16개 누적 후 17번째 → 즉시 flush.
    /// CHORD_BUFFER_MAX=16 검증.
    #[test]
    fn chord_max_size_16_mixed() {
        use crate::input_engine::chord_buffer::ChordBuffer;
        let mut buf = ChordBuffer::new(5000);
        for _ in 0..8 {
            assert!(buf.push_jamo(JamoEnum::Cho(Cho::G), JamoMeta::default()).is_none());
        }
        for _ in 0..8 {
            assert!(buf.push_non_jamo('x').is_none());
        }
        assert_eq!(buf.len(), 16, "16개 누적 확인");
        // 17번째 push → MAX flush
        let r = buf.push_jamo(JamoEnum::Cho(Cho::H), JamoMeta::default());
        assert!(r.is_some(), "17번째 → MAX flush");
        assert_eq!(r.unwrap().len(), 17, "flush 결과 17개");
        assert_eq!(buf.len(), 0);
    }

    // =========================================================================
    // Phase 3 신규: chord_compose 통합 — 시나리오 1-7 + 분기 직접 테스트
    // =========================================================================

    /// phase3_scenario_1_ga:
    /// 2키 [Cho(ㄱ), Jung(ㅏ)] → inject_to_preedit=true → preedit "가", commit 비어있음.
    #[test]
    fn phase3_scenario_1_ga() {
        let mut e = engine_anmatae_chord(5000);
        let entries = vec![
            ChordEntry { kind: ChordEntryKind::Jamo(JamoEnum::Cho(Cho::G)), input_order: 0, meta: JamoMeta::default() },
            ChordEntry { kind: ChordEntryKind::Jamo(JamoEnum::Jung(Jung::A)), input_order: 1, meta: JamoMeta::default() },
        ];
        e.apply_chord_entries(entries);
        let committed = e.commit_str().to_string();
        let preedit = e.preedit_str().to_string();
        assert_eq!(committed, "", "시나리오1: commit 비어있음 (preedit inject)");
        assert_eq!(preedit, "가", "시나리오1: preedit '가'");
    }

    /// phase3_scenario_2_gam:
    /// 3키 [Cho(ㄱ), Jung(ㅏ), Jong(ᆷ)] → preedit "감".
    #[test]
    fn phase3_scenario_2_gam() {
        let mut e = engine_anmatae_chord(5000);
        let entries = vec![
            ChordEntry { kind: ChordEntryKind::Jamo(JamoEnum::Cho(Cho::G)), input_order: 0, meta: JamoMeta::default() },
            ChordEntry { kind: ChordEntryKind::Jamo(JamoEnum::Jung(Jung::A)), input_order: 1, meta: JamoMeta::default() },
            ChordEntry { kind: ChordEntryKind::Jamo(JamoEnum::Jong(Jong::Mieum)), input_order: 2, meta: JamoMeta::default() },
        ];
        e.apply_chord_entries(entries);
        let committed = e.commit_str().to_string();
        let preedit = e.preedit_str().to_string();
        assert_eq!(committed, "", "시나리오2: commit 비어있음");
        assert_eq!(preedit, "감", "시나리오2: preedit '감'");
    }

    /// phase3_scenario_5_ga_dash:
    /// 3키 [Cho(ㄱ), Jung(ㅏ), NonJamo('-')] → Case C: commit "가-", preedit "".
    #[test]
    fn phase3_scenario_5_ga_dash() {
        let mut e = engine_anmatae_chord(5000);
        let entries = vec![
            ChordEntry { kind: ChordEntryKind::Jamo(JamoEnum::Cho(Cho::G)), input_order: 0, meta: JamoMeta::default() },
            ChordEntry { kind: ChordEntryKind::Jamo(JamoEnum::Jung(Jung::A)), input_order: 1, meta: JamoMeta::default() },
            ChordEntry { kind: ChordEntryKind::NonJamo('-'), input_order: 2, meta: JamoMeta::default() },
        ];
        e.apply_chord_entries(entries);
        // 비자모 포함 → flush_preedit 후 commit
        let committed = e.commit_str().to_string();
        let preedit = e.preedit_str().to_string();
        assert_eq!(committed, "가-", "시나리오5: '가-' commit");
        assert_eq!(preedit, "", "시나리오5: preedit 비어있음");
    }

    /// phase3_scenario_6_h_g_to_k:
    /// 2키 [Cho(ㅎ), Cho(ㄱ)] → anmatae 조합 ㅎ+ㄱ=ㅋ → preedit "ㅋ", commit "".
    #[test]
    fn phase3_scenario_6_h_g_to_k() {
        let mut e = engine_anmatae_chord(5000);
        let entries = vec![
            ChordEntry { kind: ChordEntryKind::Jamo(JamoEnum::Cho(Cho::H)), input_order: 0, meta: JamoMeta::default() },
            ChordEntry { kind: ChordEntryKind::Jamo(JamoEnum::Cho(Cho::G)), input_order: 1, meta: JamoMeta::default() },
        ];
        e.apply_chord_entries(entries);
        let committed = e.commit_str().to_string();
        let preedit = e.preedit_str().to_string();
        assert_eq!(committed, "", "시나리오6: commit 비어있음");
        assert_eq!(preedit, "ㅋ", "시나리오6: preedit 'ㅋ' (ㅎ+ㄱ=ㅋ)");
    }

    /// phase3_scenario_7_kkya:
    /// 4키 [Cho(ㄱ), Cho(ㄱ), Jung(ㅏ), Jung(ㅏ)] → 모아쓰기 결합 실패 (ㅏ+ㅏ 조합 없음).
    /// 사용자 명세 v4 후속: Case B 는 fallback commit 대신 sequential push (composer
    /// inline retry) → 일반 결합 규칙으로 점진 처리. 결과는 composer 의존.
    /// 검증: 빈 결과 아님 + 입력 자모 흔적이 commit/preedit 어딘가에 남음.
    #[test]
    fn phase3_scenario_7_kkya() {
        let mut e = engine_anmatae_chord(5000);
        let entries = vec![
            ChordEntry { kind: ChordEntryKind::Jamo(JamoEnum::Cho(Cho::G)), input_order: 0, meta: JamoMeta::default() },
            ChordEntry { kind: ChordEntryKind::Jamo(JamoEnum::Cho(Cho::G)), input_order: 1, meta: JamoMeta::default() },
            ChordEntry { kind: ChordEntryKind::Jamo(JamoEnum::Jung(Jung::A)), input_order: 2, meta: JamoMeta::default() },
            ChordEntry { kind: ChordEntryKind::Jamo(JamoEnum::Jung(Jung::A)), input_order: 3, meta: JamoMeta::default() },
        ];
        e.apply_chord_entries(entries);
        let preedit = e.preedit_str().to_string();
        let committed = e.commit_str().to_string();
        let combined = format!("{}{}", committed, preedit);
        // sequential push 결과: composer 가 점진 결합/분리 → ㄱ/ㄲ/ㅏ 흔적 + 빈 결과 아님.
        assert!(!combined.is_empty(), "시나리오7: 입력 흔적 남음 (combined='{}')", combined);
        assert!(
            combined.contains('ㄱ') || combined.contains('ㄲ') || combined.contains('가'),
            "시나리오7: 초성 ㄱ/ㄲ/가 흔적 (실제='{}')", combined
        );
        assert!(
            combined.contains('ㅏ') || combined.contains('가'),
            "시나리오7: 중성 ㅏ 흔적 (실제='{}')", combined
        );
    }

    /// phase3_chord_fail_falls_back:
    /// 2키 [Cho(ㄱ), Cho(ㅈ)] → 모아쓰기 결합 실패 (ㄱ+ㅈ 조합 없음).
    /// 사용자 명세 v4 후속: Case B sequential push → composer 가 ㄱ commit + ㅈ preedit
    /// (또는 합쳐서 commit) 등 일반 결합 규칙 처리.
    #[test]
    fn phase3_chord_fail_falls_back() {
        let mut e = engine_anmatae_chord(5000);
        let entries = vec![
            ChordEntry { kind: ChordEntryKind::Jamo(JamoEnum::Cho(Cho::G)), input_order: 0, meta: JamoMeta::default() },
            ChordEntry { kind: ChordEntryKind::Jamo(JamoEnum::Cho(Cho::J)), input_order: 1, meta: JamoMeta::default() },
        ];
        e.apply_chord_entries(entries);
        let committed = e.commit_str().to_string();
        let preedit = e.preedit_str().to_string();
        let combined = format!("{}{}", committed, preedit);
        // ㄱ + ㅈ 흔적이 어딘가에 남음 (commit 또는 preedit).
        assert!(!combined.is_empty(), "조합 실패: 입력 흔적 남음 (combined='{}')", combined);
        assert!(combined.contains('ㄱ'), "조합 실패: ㄱ 흔적 (실제='{}')", combined);
        assert!(combined.contains('ㅈ'), "조합 실패: ㅈ 흔적 (실제='{}')", combined);
    }

    /// phase3_single_jamo_sequential:
    /// 1키 [Cho(ㄱ)] → 1키 분기 (풀어쓰기) → composer 직접 호출 → preedit "ㄱ".
    #[test]
    fn phase3_single_jamo_sequential() {
        let mut e = engine_anmatae_chord(5000);
        let entries = vec![
            ChordEntry { kind: ChordEntryKind::Jamo(JamoEnum::Cho(Cho::G)), input_order: 0, meta: JamoMeta::default() },
        ];
        e.apply_chord_entries(entries);
        let committed = e.commit_str().to_string();
        let preedit = e.preedit_str().to_string();
        assert_eq!(committed, "", "1키 분기: commit 비어있음");
        assert_eq!(preedit, "ㄱ", "1키 분기: preedit 'ㄱ'");
    }

    /// phase3_single_non_jamo_commit:
    /// 1키 [NonJamo('-')] → 1키 분기 → flush_preedit + commit "-".
    #[test]
    fn phase3_single_non_jamo_commit() {
        let mut e = engine_anmatae_chord(5000);
        let entries = vec![
            ChordEntry { kind: ChordEntryKind::NonJamo('-'), input_order: 0, meta: JamoMeta::default() },
        ];
        e.apply_chord_entries(entries);
        let committed = e.commit_str().to_string();
        let preedit = e.preedit_str().to_string();
        assert_eq!(committed, "-", "1키 비자모: '-' commit");
        assert_eq!(preedit, "", "1키 비자모: preedit 비어있음");
    }

    /// phase3_keui_regression:
    /// 4키 [Cho(ㄱ), Cho(ㅎ), Jung(ㅡ), Jung(ㅣ)] → chord_compose → "킈".
    /// idle_flush_many_keys 와 동일 시나리오를 apply_chord_entries 직접 경로로 검증.
    #[test]
    fn phase3_keui_regression() {
        let mut e = engine_anmatae_chord(5000);
        let entries = vec![
            ChordEntry { kind: ChordEntryKind::Jamo(JamoEnum::Cho(Cho::G)), input_order: 0, meta: JamoMeta::default() },
            ChordEntry { kind: ChordEntryKind::Jamo(JamoEnum::Cho(Cho::H)), input_order: 1, meta: JamoMeta::default() },
            ChordEntry { kind: ChordEntryKind::Jamo(JamoEnum::Jung(Jung::Eu)), input_order: 2, meta: JamoMeta::default() },
            ChordEntry { kind: ChordEntryKind::Jamo(JamoEnum::Jung(Jung::I)), input_order: 3, meta: JamoMeta::default() },
        ];
        e.apply_chord_entries(entries);
        let preedit = e.preedit_str().to_string();
        // inject_to_preedit=true → "킈" preedit (음절 형성)
        assert_eq!(preedit, "킈", "phase3 keui 회귀: preedit '킈'");
        assert_eq!(e.commit_str(), "", "phase3 keui 회귀: commit 비어있음");
    }
}
