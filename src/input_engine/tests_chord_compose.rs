// tests_chord_compose.rs
//! `chord_compose` 단위 테스트.
//!
//! anmatae 자판의 combinations를 기반으로 `compose_chord` 알고리즘 정합성을 검증한다.
//! 외부 상태 의존 없음 — 순수 함수 테스트.

#[cfg(test)]
mod tests {
    use crate::hangul::composer::CombinedJamoMap;
    use crate::hangul::jamo::{Cho, Jong, Jung, JamoEnum};
    use crate::input_engine::chord_compose::{compose_chord, ChordEntry, ChordEntryKind};
    use crate::keystroke::profile::builtin::KO_3BUL_ANMATAE;
    use crate::keystroke::profile::build_combined_jamo_map;
    use crate::keystroke::profile::loader::parse_profile_str;

    // ── fixture ──────────────────────────────────────────────────────────────

    /// anmatae 자판 CombinedJamoMap 생성.
    fn make_anmatae_combined_jamo() -> CombinedJamoMap {
        let profile = parse_profile_str(KO_3BUL_ANMATAE).expect("anmatae profile parse");
        build_combined_jamo_map(&profile).expect("anmatae combined_jamo build")
    }

    /// 비어있는 CombinedJamoMap (결합 없음).
    fn make_empty_map() -> CombinedJamoMap {
        CombinedJamoMap::new()
    }

    fn cho_entry(c: Cho, order: u8) -> ChordEntry {
        ChordEntry {
            kind: ChordEntryKind::Jamo(JamoEnum::Cho(c)),
            input_order: order,
            meta: crate::hangul::composer::JamoMeta::default(),
        }
    }

    fn jung_entry(j: Jung, order: u8) -> ChordEntry {
        ChordEntry {
            kind: ChordEntryKind::Jamo(JamoEnum::Jung(j)),
            input_order: order,
            meta: crate::hangul::composer::JamoMeta::default(),
        }
    }

    fn jong_entry(j: Jong, order: u8) -> ChordEntry {
        ChordEntry {
            kind: ChordEntryKind::Jamo(JamoEnum::Jong(j)),
            input_order: order,
            meta: crate::hangul::composer::JamoMeta::default(),
        }
    }

    fn non_jamo_entry(c: char, order: u8) -> ChordEntry {
        ChordEntry {
            kind: ChordEntryKind::NonJamo(c),
            input_order: order,
            meta: crate::hangul::composer::JamoMeta::default(),
        }
    }

    // ═════════════════════════════════════════════════════════════════════════
    // A. 초성 결합
    // ═════════════════════════════════════════════════════════════════════════

    /// cho_2_forward_combine: [ㄱ, ㅎ] → ㅋ (anmatae: ㄱ+ㅎ=ㅋ)
    #[test]
    fn cho_2_forward_combine() {
        let map = make_anmatae_combined_jamo();
        let entries = vec![cho_entry(Cho::G, 0), cho_entry(Cho::H, 1)];
        let result = compose_chord(&entries, &map);

        assert_eq!(result.cho, Some(Cho::K), "ㄱ+ㅎ → ㅋ");
        assert!(result.fallback_jamos.is_empty(), "fallback 없음");
        assert!(result.inject_to_preedit, "preedit inject=true");
    }

    /// cho_2_reverse_combine: [ㅎ, ㄱ] → permutation으로 ㅋ 성공
    #[test]
    fn cho_2_reverse_combine() {
        let map = make_anmatae_combined_jamo();
        // 역순 입력
        let entries = vec![cho_entry(Cho::H, 0), cho_entry(Cho::G, 1)];
        let result = compose_chord(&entries, &map);

        assert_eq!(result.cho, Some(Cho::K), "역순 ㅎ+ㄱ → permutation → ㅋ");
        assert!(result.inject_to_preedit);
    }

    /// cho_2_no_combine: [ㄱ, ㅈ] — anmatae에 ㄱ+ㅈ 조합 없음 → fallback
    #[test]
    fn cho_2_no_combine() {
        let map = make_anmatae_combined_jamo();
        let entries = vec![cho_entry(Cho::G, 0), cho_entry(Cho::J, 1)];
        let result = compose_chord(&entries, &map);

        assert_eq!(result.cho, None);
        assert!(!result.inject_to_preedit, "inject=false");
        // fallback_jamos에 ㄱ, ㅈ 호환자모
        assert_eq!(result.fallback_jamos.len(), 2);
        assert_eq!(result.fallback_jamos[0], 'ㄱ');
        assert_eq!(result.fallback_jamos[1], 'ㅈ');
    }

    /// cho_1_only: [ㄱ] → cho=ㄱ, inject=true
    #[test]
    fn cho_1_only() {
        let map = make_anmatae_combined_jamo();
        let entries = vec![cho_entry(Cho::G, 0)];
        let result = compose_chord(&entries, &map);

        assert_eq!(result.cho, Some(Cho::G));
        assert!(result.inject_to_preedit);
        assert!(result.fallback_jamos.is_empty());
    }

    // ═════════════════════════════════════════════════════════════════════════
    // B. 중성 결합
    // ═════════════════════════════════════════════════════════════════════════

    /// jung_3_left_fold: [ㅗ, ㅏ, ㅣ] → ㅙ
    /// anmatae: ㅗ+ㅏ=ㅘ, ㅘ+ㅣ=ㅙ
    #[test]
    fn jung_3_left_fold() {
        let map = make_anmatae_combined_jamo();
        // 순서: ㅗ(0) ㅏ(1) ㅣ(2)
        let entries = vec![
            jung_entry(Jung::O, 0),
            jung_entry(Jung::A, 1),
            jung_entry(Jung::I, 2),
        ];
        let result = compose_chord(&entries, &map);

        assert_eq!(result.jung, Some(Jung::WAE), "ㅗ+ㅏ+ㅣ → ㅙ");
        assert!(result.inject_to_preedit);
        assert!(result.fallback_jamos.is_empty());
    }

    /// jung_3_permutation_required: [ㅣ, ㅗ, ㅏ] → permutation으로 ㅙ 성공
    #[test]
    fn jung_3_permutation_required() {
        let map = make_anmatae_combined_jamo();
        // 다른 입력 순서
        let entries = vec![
            jung_entry(Jung::I, 0),
            jung_entry(Jung::O, 1),
            jung_entry(Jung::A, 2),
        ];
        let result = compose_chord(&entries, &map);

        assert_eq!(result.jung, Some(Jung::WAE), "permutation → ㅙ");
        assert!(result.inject_to_preedit);
    }

    /// jung_3_no_full_reduce: [ㅏ, ㅏ, ㅓ] → 조합 없음 → fallback
    /// anmatae에 ㅏ+ㅏ=ㅑ가 있지만 ㅑ+ㅓ 조합 없음 → 6 perm 모두 실패
    #[test]
    fn jung_3_no_full_reduce() {
        let map = make_anmatae_combined_jamo();
        let entries = vec![
            jung_entry(Jung::A, 0),
            jung_entry(Jung::A, 1),
            jung_entry(Jung::Eo, 2),
        ];
        let result = compose_chord(&entries, &map);

        assert_eq!(result.jung, None);
        assert!(!result.inject_to_preedit);
        // fallback_jamos: 3개 호환자모 (원본 자모들)
        assert_eq!(result.fallback_jamos.len(), 3);
    }

    // ═════════════════════════════════════════════════════════════════════════
    // C. 종성 결합
    // ═════════════════════════════════════════════════════════════════════════

    /// jong_2_forward: [ᆨ, ᆯ] → anmatae: ᆨ+ᆯ=ᆰ (ㄱ+ㄹ=ㄺ)
    #[test]
    fn jong_2_forward_combine() {
        let map = make_anmatae_combined_jamo();
        let entries = vec![jong_entry(Jong::G, 0), jong_entry(Jong::L, 1)];
        let result = compose_chord(&entries, &map);

        assert_eq!(result.jong, Some(Jong::LG), "ᆨ+ᆯ → ᆰ");
        assert!(result.inject_to_preedit);
    }

    /// jong_2_reverse: [ᆯ, ᆨ] → permutation으로 ᆰ
    #[test]
    fn jong_2_reverse_combine() {
        let map = make_anmatae_combined_jamo();
        let entries = vec![jong_entry(Jong::L, 0), jong_entry(Jong::G, 1)];
        let result = compose_chord(&entries, &map);

        assert_eq!(result.jong, Some(Jong::LG), "역순 → permutation → ᆰ");
        assert!(result.inject_to_preedit);
    }

    /// jong_2_no_combine: [ᆨ, ᆷ] — anmatae에 해당 조합 없음
    #[test]
    fn jong_2_no_combine() {
        let map = make_anmatae_combined_jamo();
        // anmatae에 ᆨ+ᆷ 조합은 없음
        let entries = vec![jong_entry(Jong::G, 0), jong_entry(Jong::M, 1)];
        let result = compose_chord(&entries, &map);

        assert_eq!(result.jong, None);
        assert!(!result.inject_to_preedit);
        assert_eq!(result.fallback_jamos.len(), 2);
    }

    // ═════════════════════════════════════════════════════════════════════════
    // D. 비자모 분리
    // ═════════════════════════════════════════════════════════════════════════

    /// non_jamo_appended: [ㄱ(0), ㅏ(1), '-'(2)] → 자모 reduce 성공 → inject=true, non_jamos=['-']
    ///
    /// Phase 3 업데이트: 자모 reduce 성공이면 비자모 유무와 무관하게 inject_to_preedit=true.
    /// apply_chord_result Case C 에서 inject 후 비자모 commit.
    #[test]
    fn non_jamo_appended() {
        let map = make_anmatae_combined_jamo();
        let entries = vec![
            cho_entry(Cho::G, 0),
            jung_entry(Jung::A, 1),
            non_jamo_entry('-', 2),
        ];
        let result = compose_chord(&entries, &map);

        // 자모 reduce 성공 → inject=true (비자모는 non_jamos 에)
        assert!(result.inject_to_preedit, "자모 성공 + 비자모 → inject=true");
        assert_eq!(result.cho, Some(Cho::G));
        assert_eq!(result.jung, Some(Jung::A));
        assert_eq!(result.non_jamos, vec!['-']);
        assert!(result.fallback_jamos.is_empty(), "fallback 없음 (reduce 성공)");
    }

    /// non_jamo_reorder_preserved: [','(0), ㄱ(1), '-'(2), ㅏ(3)]
    /// → cho=ㄱ, jung=ㅏ, non_jamos=[',', '-'] (입력 순서)
    #[test]
    fn non_jamo_reorder_preserved() {
        let map = make_anmatae_combined_jamo();
        let entries = vec![
            non_jamo_entry(',', 0),
            cho_entry(Cho::G, 1),
            non_jamo_entry('-', 2),
            jung_entry(Jung::A, 3),
        ];
        let result = compose_chord(&entries, &map);

        // 자모 reduce 성공 → inject=true
        assert!(result.inject_to_preedit, "자모 성공 + 비자모 → inject=true");
        assert_eq!(result.cho, Some(Cho::G));
        assert_eq!(result.jung, Some(Jung::A));
        // non_jamos는 input_order 순: ',', '-'
        assert_eq!(result.non_jamos, vec![',', '-']);
        assert!(result.fallback_jamos.is_empty());
    }

    // ═════════════════════════════════════════════════════════════════════════
    // E. 통합 시나리오
    // ═════════════════════════════════════════════════════════════════════════

    /// full_syllable_3_jamo: [ㄱ(0), ㅏ(1), ᆷ(2)] → cho=ㄱ, jung=ㅏ, jong=ᆷ, inject=true ("감")
    #[test]
    fn full_syllable_3_jamo() {
        let map = make_anmatae_combined_jamo();
        let entries = vec![
            cho_entry(Cho::G, 0),
            jung_entry(Jung::A, 1),
            jong_entry(Jong::M, 2),
        ];
        let result = compose_chord(&entries, &map);

        assert_eq!(result.cho, Some(Cho::G));
        assert_eq!(result.jung, Some(Jung::A));
        assert_eq!(result.jong, Some(Jong::M));
        assert!(result.inject_to_preedit, "3자모 조합 → preedit inject");
        assert!(result.fallback_jamos.is_empty());
        assert!(result.non_jamos.is_empty());
    }

    /// chord_with_non_jamo_mix: [ㄱ(0), ㅏ(1), '-'(2)] → 자모 성공 + 비자모 포함 → inject=true
    ///
    /// Phase 3 업데이트: 자모 reduce 성공이면 inject=true.
    /// apply_chord_result 가 Case C (non_jamos 있음) 로 분기해 syllable inject + 비자모 commit.
    #[test]
    fn chord_with_non_jamo_mix() {
        let map = make_anmatae_combined_jamo();
        let entries = vec![
            cho_entry(Cho::G, 0),
            jung_entry(Jung::A, 1),
            non_jamo_entry('-', 2),
        ];
        let result = compose_chord(&entries, &map);

        assert!(result.inject_to_preedit, "자모 성공 + 비자모 → inject=true");
        assert_eq!(result.cho, Some(Cho::G));
        assert_eq!(result.jung, Some(Jung::A));
        assert_eq!(result.non_jamos, vec!['-']);
        assert!(result.fallback_jamos.is_empty());
    }

    /// empty_input: [] → 모두 None, inject=false
    #[test]
    fn empty_input() {
        let map = make_empty_map();
        let result = compose_chord(&[], &map);

        assert_eq!(result.cho, None);
        assert_eq!(result.jung, None);
        assert_eq!(result.jong, None);
        assert!(result.fallback_jamos.is_empty());
        assert!(result.non_jamos.is_empty());
        // inject_to_preedit: 빈 입력 → 자모 없음 (has_jamo=false) → inject=false
        assert!(!result.inject_to_preedit);
    }

    // ═════════════════════════════════════════════════════════════════════════
    // F. input_order_jamos 검증
    // ═════════════════════════════════════════════════════════════════════════

    /// input_order_jamos: 자모가 input_order 순으로 정렬되어야 함
    #[test]
    fn input_order_jamos_sorted() {
        let map = make_anmatae_combined_jamo();
        // 역순 입력: jong(0) cho(1) jung(2)
        let entries = vec![
            jong_entry(Jong::M, 0),
            cho_entry(Cho::G, 1),
            jung_entry(Jung::A, 2),
        ];
        let result = compose_chord(&entries, &map);

        // input_order_jamos: Jong(ᆷ), Cho(ㄱ), Jung(ㅏ) 순 (input_order 0,1,2)
        assert_eq!(result.input_order_jamos.len(), 3);
        assert!(matches!(result.input_order_jamos[0], JamoEnum::Jong(Jong::M)));
        assert!(matches!(result.input_order_jamos[1], JamoEnum::Cho(Cho::G)));
        assert!(matches!(result.input_order_jamos[2], JamoEnum::Jung(Jung::A)));
    }

    // ═════════════════════════════════════════════════════════════════════════
    // G. 추가 cho 조합 anmatae
    // ═════════════════════════════════════════════════════════════════════════

    /// cho ㄷ+ㅎ → ㅌ (anmatae)
    #[test]
    fn cho_d_h_to_t() {
        let map = make_anmatae_combined_jamo();
        let entries = vec![cho_entry(Cho::D, 0), cho_entry(Cho::H, 1)];
        let result = compose_chord(&entries, &map);
        assert_eq!(result.cho, Some(Cho::T));
        assert!(result.inject_to_preedit);
    }

    /// cho ㅂ+ㅎ → ㅍ (anmatae)
    #[test]
    fn cho_b_h_to_p() {
        let map = make_anmatae_combined_jamo();
        let entries = vec![cho_entry(Cho::B, 0), cho_entry(Cho::H, 1)];
        let result = compose_chord(&entries, &map);
        assert_eq!(result.cho, Some(Cho::P));
        assert!(result.inject_to_preedit);
    }

    /// cho ㅈ+ㅎ → ㅊ (anmatae)
    #[test]
    fn cho_j_h_to_c() {
        let map = make_anmatae_combined_jamo();
        let entries = vec![cho_entry(Cho::J, 0), cho_entry(Cho::H, 1)];
        let result = compose_chord(&entries, &map);
        assert_eq!(result.cho, Some(Cho::C));
        assert!(result.inject_to_preedit);
    }

    // ═════════════════════════════════════════════════════════════════════════
    // H. 중성 2자모
    // ═════════════════════════════════════════════════════════════════════════

    /// jung ㅏ+ㅣ → ㅐ
    #[test]
    fn jung_2_a_i_to_ae() {
        let map = make_anmatae_combined_jamo();
        let entries = vec![jung_entry(Jung::A, 0), jung_entry(Jung::I, 1)];
        let result = compose_chord(&entries, &map);
        assert_eq!(result.jung, Some(Jung::Ae));
        assert!(result.inject_to_preedit);
    }

    /// jung ㅗ+ㅣ → ㅚ
    #[test]
    fn jung_2_o_i_to_oi() {
        let map = make_anmatae_combined_jamo();
        let entries = vec![jung_entry(Jung::O, 0), jung_entry(Jung::I, 1)];
        let result = compose_chord(&entries, &map);
        assert_eq!(result.jung, Some(Jung::OE));
        assert!(result.inject_to_preedit);
    }

    // ═════════════════════════════════════════════════════════════════════════
    // I. 단일 자모 각 영역
    // ═════════════════════════════════════════════════════════════════════════

    #[test]
    fn single_jung_only() {
        let map = make_anmatae_combined_jamo();
        let entries = vec![jung_entry(Jung::A, 0)];
        let result = compose_chord(&entries, &map);
        assert_eq!(result.jung, Some(Jung::A));
        assert!(result.inject_to_preedit);
    }

    #[test]
    fn single_jong_only() {
        let map = make_anmatae_combined_jamo();
        let entries = vec![jong_entry(Jong::M, 0)];
        let result = compose_chord(&entries, &map);
        assert_eq!(result.jong, Some(Jong::M));
        assert!(result.inject_to_preedit);
    }

    #[test]
    fn single_non_jamo_only() {
        let map = make_empty_map();
        let entries = vec![non_jamo_entry('-', 0)];
        let result = compose_chord(&entries, &map);
        assert!(!result.inject_to_preedit);
        assert_eq!(result.non_jamos, vec!['-']);
        assert!(result.fallback_jamos.is_empty());
    }
}
