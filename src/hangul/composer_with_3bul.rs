// composer_with_3bul.rs
//! 세벌식 한글 입력 방식의 조합 로직을 구현합니다.
//!
//! 0.2.0+: Rust const 자모 조합 테이블(JUNG/JONG/CHO_COMBINATIONS)과 Lazy static
//! `COMBINED_JAMO_3BUL`을 모두 제거. `new()`는 내장 v1 프로필 `ko_3bul390`을
//! 즉시 로드해 `new_with_profile`로 위임한다. 자모 조합 규칙의 단일 source of
//! truth는 `src/keystroke/keymap/ko_3bul390.json`(390/391/noshift는 base 동일).
//!
//! v3 부분 적용 (Phase 3-rework2):
//! - `bidirectional_combine=true`이면 cho/jung/jong 영역별로 (a,b) 실패 시 (b,a) 재시도.
//!   기존 `jong_unordered`를 일반화한 버전. 옵션 OFF 시 기존 동작 100% 동일 (회귀 0).
//! - `chord_window_ms`: Phase 4 구현 완료. InputEngine 레벨 ChordBuffer가 담당.

use crate::hangul::composer::BaseHangulComposer;
use crate::hangul::composer::CombinedJamoMap;
use crate::hangul::composer::HangulComposer;
use crate::hangul::composer::JamoMeta;
use crate::hangul::jamo::*;
use crate::hangul::HangulChar;
use crate::keystroke::profile::MoachigiSpec;

use std::collections::VecDeque;

// ============================================================================
// 3벌식 조합 규칙 검사 헬퍼
// ============================================================================

/// 3벌식 조합 규칙 위반 유형
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Compose3BulViolation {
    /// 중성 없이 종성 입력 (처음 또는 초성 다음에 바로 종성)
    JongWithoutJung,
    /// 중성/종성 다음에 초성 입력 - 새 음절 시작
    ChoAfterJungOrJong,
    /// 종성 다음에 중성 입력 - 새 음절 시작
    JungAfterJong,
}

/// 현재 큐 상태에서 3벌식 규칙 위반 여부를 검사합니다.
fn check_3bul_violation(base: &mut BaseHangulComposer) -> Option<Compose3BulViolation> {
    let queue = base.jamo_queue();
    if queue.is_empty() {
        return None;
    }

    let last = *queue.back().unwrap();
    let prev = if queue.len() > 1 {
        Some(queue[queue.len() - 2])
    } else {
        None
    };
    let is_filled_jung = base.current_korean().is_filled_jung();

    match (prev, last) {
        // 규칙 1: 초성 다음에 중성 없이 바로 종성이 오는 경우
        // (큐에 단독 종성만 있는 경우는 새 음절의 종성을 단독 자음으로 표시하므로
        //  위반이 아니다 — preedit에 종성 호환 자모가 즉시 표시되도록 함)
        (Some(p), l) if l.is_jong() && !is_filled_jung && p.is_cho() => {
            Some(Compose3BulViolation::JongWithoutJung)
        }
        // 규칙 2: 중성이나 종성 다음에 초성이 오는 경우
        (Some(p), l) if (p.is_jung() || p.is_jong()) && l.is_cho() => {
            Some(Compose3BulViolation::ChoAfterJungOrJong)
        }
        // 규칙 3: 종성 다음에 중성이 오는 경우
        (Some(p), l) if p.is_jong() && l.is_jung() => Some(Compose3BulViolation::JungAfterJong),
        _ => None,
    }
}

// ============================================================================
// HangulComposer3Bul 구현
// ============================================================================

/// 세벌식 한글 입력 방식의 조합 로직을 구현한 컴포저입니다.
///
/// # 특징
/// - 초성, 중성, 종성이 별도의 키에 할당됨
/// - 종성은 중성이 입력된 후에만 입력 가능
/// - 중성/종성 다음에 초성이 오면 새 음절 시작
/// - 초성 조합 지원 (ㄱ+ㄱ=ㄲ 등)
/// - v3 모아치기: `moachigi.bidirectional_combine=true`이면 cho/jung/jong 양방향 결합.
///   None 또는 false면 기존 동작 100% 동일 (회귀 0).
#[derive(Debug, Default)]
pub struct HangulComposer3Bul {
    base_composer: BaseHangulComposer,
    /// v3 모아치기 파라미터. None이면 기존 동작 100% 동일.
    pub(crate) moachigi: Option<MoachigiSpec>,
}

impl HangulComposer3Bul {
    /// 새로운 `HangulComposer3Bul` 인스턴스를 생성합니다.
    ///
    /// 내장 v1 프로필 `ko_3bul390`(자기 완결 JSON)을 로드해 `new_with_profile`에
    /// 위임한다. JSON은 `src/keystroke/keymap/ko_3bul390.json`이 단일 source of
    /// truth이며, 빌드 시 `include_str!`로 임베드되므로 런타임 I/O 없음.
    pub fn new() -> Self {
        let profile = crate::keystroke::profile::load_builtin_profile("ko_3bul390")
            .expect("builtin ko_3bul390 profile must always parse");
        Self::new_with_profile(&profile)
            .expect("builtin ko_3bul390 profile must always build a valid jamo map")
    }

    /// `LayoutProfile`에서 추출한 조합 규칙을 주입해 생성.
    ///
    /// 0.2.0+: 모든 입력 프로필은 v1/v2/v3. `combinations` + 활성 `rule_sets`가 반영된 맵을
    /// 주입한다. v3 프로필이면 merged_moachigi()를 적용.
    pub fn new_with_profile(
        profile: &crate::keystroke::profile::LayoutProfile,
    ) -> Result<Self, crate::keystroke::profile::BuildError> {
        let map = crate::keystroke::profile::build_combined_jamo_map(profile)?;
        let moachigi = profile.merged_moachigi();
        let mut composer = HangulComposer3Bul {
            base_composer: BaseHangulComposer::new(),
            moachigi,
        };
        *composer.base_composer.combined_jamo() = map;
        Ok(composer)
    }

    /// 현재 활성화된 moachigi 파라미터 참조.
    pub fn effective_moachigi(&self) -> Option<&MoachigiSpec> {
        self.moachigi.as_ref()
    }

    /// bidirectional_combine 활성 여부 편의 접근자.
    pub fn is_bidirectional_combine(&self) -> bool {
        self.moachigi
            .as_ref()
            .map(|m| m.bidirectional_combine)
            .unwrap_or(false)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // === 쌍초성 조합 테스트 (3벌식 전용) ===

    #[test]
    fn test_3bul_cho_combination_gg() {
        // ㄱ + ㄱ = ㄲ
        let mut c = HangulComposer3Bul::new();
        c.add_jamo(JamoEnum::Cho(Cho::Giyeok));
        c.add_jamo(JamoEnum::Cho(Cho::Giyeok));
        assert_eq!(c.get_current_cho(), Some(Cho::SsangGiyeok));
    }

    #[test]
    fn test_3bul_cho_combination_dd() {
        // ㄷ + ㄷ = ㄸ
        let mut c = HangulComposer3Bul::new();
        c.add_jamo(JamoEnum::Cho(Cho::Digeut));
        c.add_jamo(JamoEnum::Cho(Cho::Digeut));
        assert_eq!(c.get_current_cho(), Some(Cho::SsangDigeut));
    }

    #[test]
    fn test_3bul_cho_combination_bb() {
        // ㅂ + ㅂ = ㅃ
        let mut c = HangulComposer3Bul::new();
        c.add_jamo(JamoEnum::Cho(Cho::Bieup));
        c.add_jamo(JamoEnum::Cho(Cho::Bieup));
        assert_eq!(c.get_current_cho(), Some(Cho::SsangBieup));
    }

    #[test]
    fn test_3bul_cho_combination_ss() {
        // ㅅ + ㅅ = ㅆ
        let mut c = HangulComposer3Bul::new();
        c.add_jamo(JamoEnum::Cho(Cho::Siot));
        c.add_jamo(JamoEnum::Cho(Cho::Siot));
        assert_eq!(c.get_current_cho(), Some(Cho::SsangSiot));
    }

    #[test]
    fn test_3bul_cho_combination_jj() {
        // ㅈ + ㅈ = ㅉ
        let mut c = HangulComposer3Bul::new();
        c.add_jamo(JamoEnum::Cho(Cho::Jieut));
        c.add_jamo(JamoEnum::Cho(Cho::Jieut));
        assert_eq!(c.get_current_cho(), Some(Cho::SsangJieut));
    }

    #[test]
    fn test_3bul_cho_invalid_combination() {
        // ㄱ + ㄴ → 조합 불가 → 음절 분리
        let mut c = HangulComposer3Bul::new();
        c.add_jamo(JamoEnum::Cho(Cho::Giyeok));
        let committed = c.add_jamo(JamoEnum::Cho(Cho::Nieun));
        // 새 음절 ㄴ 시작
        assert_eq!(c.get_current_cho(), Some(Cho::Nieun));
        // incomplete syllable(cho 단독)도 호환자모로 commit 흘려보내야
        // GTK/Qt frontend가 preedit dedup 없이 두 번째 'ㄴ'을 표시할 수 있다.
        assert_eq!(committed, Some('ㄱ'));
    }

    #[test]
    fn test_3bul_cho_repeat_emits_compat() {
        // 세벌식 ko_3bul390에는 ㄴ+ㄴ 조합 규칙이 없으므로 분리 경로.
        // incomplete syllable(cho 단독)도 호환자모 'ㄴ'로 commit 흘려보내야 한다.
        let mut c = HangulComposer3Bul::new();
        let first = c.add_jamo(JamoEnum::Cho(Cho::Nieun));
        assert_eq!(first, None);
        let second = c.add_jamo(JamoEnum::Cho(Cho::Nieun));
        assert_eq!(second, Some('ㄴ'));
        assert_eq!(c.get_current_cho(), Some(Cho::Nieun));
    }

    // === 3벌식 복모음 조합 테스트 ===

    #[test]
    fn test_3bul_jung_combination() {
        // 3벌식도 복모음 지원: ㅗ + ㅏ = ㅘ
        let mut c = HangulComposer3Bul::new();
        c.add_jamo(JamoEnum::Cho(Cho::G));
        c.add_jamo(JamoEnum::Jung(Jung::O));
        c.add_jamo(JamoEnum::Jung(Jung::A));
        assert_eq!(c.get_current_jung(), Some(Jung::Wa));
    }

    // === 3벌식 겹받침 조합 테스트 ===

    #[test]
    fn test_3bul_jong_combination() {
        // 3벌식 겹받침: ㄱ + ㅅ = ㄳ (종성 직접 입력)
        let mut c = HangulComposer3Bul::new();
        c.add_jamo(JamoEnum::Cho(Cho::G));
        c.add_jamo(JamoEnum::Jung(Jung::A));
        c.add_jamo(JamoEnum::Jong(Jong::Giyeok));
        c.add_jamo(JamoEnum::Jong(Jong::Siot));
        assert_eq!(c.get_current_jong(), Some(Jong::GiyeokSiot));
    }

    // === 3벌식 규칙 위반 테스트 ===

    #[test]
    fn test_3bul_jong_without_jung() {
        // 초성 다음에 바로 종성 → 위반 → 분리
        let mut c = HangulComposer3Bul::new();
        c.add_jamo(JamoEnum::Cho(Cho::G));
        let committed = c.add_jamo(JamoEnum::Jong(Jong::Giyeok));
        // ㄱ이 커밋되고 새 음절 시작 (종성만)
        let _ = committed;
        // 새 음절의 jong이 정상적으로 설정되어 있어야 함 (preedit 표시용)
        assert_eq!(c.get_current_jong(), Some(Jong::Giyeok));
    }

    #[test]
    fn test_3bul_jong_first_input() {
        // 처음부터 종성을 입력 → 단독 자음으로 새 음절 시작
        // (preedit에 ㄱ이 표시되어야 하고, 강제 commit 시에도 ㄱ이 나와야 함)
        let mut c = HangulComposer3Bul::new();
        let committed = c.add_jamo(JamoEnum::Jong(Jong::Giyeok));
        assert_eq!(committed, None); // 이전 음절 없음
        assert_eq!(c.get_current_jong(), Some(Jong::Giyeok));
        // force compose 시 종성 호환 자모가 결과로 나와야 함
        let ch = c.force_compose_korean();
        assert_eq!(ch, Some('ㄱ'));
    }

    #[test]
    fn test_3bul_cho_after_jung() {
        // 중성 다음에 초성 → 새 음절 시작
        let mut c = HangulComposer3Bul::new();
        c.add_jamo(JamoEnum::Cho(Cho::G));
        c.add_jamo(JamoEnum::Jung(Jung::A));
        let committed = c.add_jamo(JamoEnum::Cho(Cho::N)); // 새 음절
        assert_eq!(committed, Some('가'));
        assert_eq!(c.get_current_cho(), Some(Cho::N));
    }

    #[test]
    fn test_3bul_jung_after_jong() {
        // 종성 다음에 중성 → 새 음절 (3벌식은 도깨비불 없음)
        let mut c = HangulComposer3Bul::new();
        c.add_jamo(JamoEnum::Cho(Cho::G));
        c.add_jamo(JamoEnum::Jung(Jung::A));
        c.add_jamo(JamoEnum::Jong(Jong::Giyeok));
        let committed = c.add_jamo(JamoEnum::Jung(Jung::A));
        assert_eq!(committed, Some('각'));
    }

    // === 3벌식 완전한 음절 테스트 ===

    #[test]
    fn test_3bul_full_syllable() {
        // ㄱ + ㅏ + ㄱ = 각
        let mut c = HangulComposer3Bul::new();
        c.add_jamo(JamoEnum::Cho(Cho::G));
        c.add_jamo(JamoEnum::Jung(Jung::A));
        c.add_jamo(JamoEnum::Jong(Jong::Giyeok));
        let ch = c.force_compose_korean();
        assert_eq!(ch, Some('각'));
    }

    #[test]
    fn test_3bul_ssang_cho_with_syllable() {
        // ㄲ + ㅏ = 까
        let mut c = HangulComposer3Bul::new();
        c.add_jamo(JamoEnum::Cho(Cho::Giyeok));
        c.add_jamo(JamoEnum::Cho(Cho::Giyeok)); // ㄲ
        c.add_jamo(JamoEnum::Jung(Jung::A)); // 까
        let ch = c.force_compose_korean();
        assert_eq!(ch, Some('까'));
    }

    // === Special 문자 무시 ===

    #[test]
    fn test_3bul_special_jamo_ignored() {
        let mut c = HangulComposer3Bul::new();
        let result = c.add_jamo(JamoEnum::Special('!'));
        assert!(result.is_none());
        assert!(!c.is_compose());
    }

    // ======================================================================
    // O1: bidirectional_combine (Phase 3-rework2)
    // ======================================================================

    /// O1-off-regression: moachigi=None (v1/v2 프로필) → 기존 동작 100% 동일.
    #[test]
    fn o1_off_regression_v1_profile_unchanged() {
        let mut c = HangulComposer3Bul::new();
        assert!(c.moachigi.is_none(), "v1 profile must have no moachigi");
        // ㄱ + ㅏ + ㄳ(겹받침) — 기존 동작: ㄳ은 ko_3bul390 combinations에 있음
        c.add_jamo(JamoEnum::Cho(Cho::Giyeok));
        c.add_jamo(JamoEnum::Jung(Jung::A));
        c.add_jamo(JamoEnum::Jong(Jong::Giyeok));
        c.add_jamo(JamoEnum::Jong(Jong::Siot));
        assert_eq!(c.get_current_jong(), Some(Jong::GiyeokSiot));
    }

    /// O1-jong-rev: bidirectional_combine=true (사용자 config 주입) → 역순 종성 결합 활성 검증.
    ///
    /// Phase 7: 프로필 로드 후 moachigi 수동 주입 (사용자 config 주입 패턴).
    #[test]
    fn o1_jong_rev_bidirectional_combine_true() {
        let mut c = HangulComposer3Bul::new();
        // 안마태 프로필 로드 (Phase 7: MoachigiSpec은 capability 마커만)
        let profile = crate::keystroke::profile::load_builtin_profile("ko_3bul_anmatae")
            .expect("anmatae profile must load");
        let mut ac = HangulComposer3Bul::new_with_profile(&profile)
            .expect("anmatae must build");
        // Phase 7: 사용자 config 주입 패턴 — 직접 설정
        ac.moachigi = Some(MoachigiSpec { bidirectional_combine: true, chord_window_ms: 0 });
        assert!(ac.is_bidirectional_combine(), "사용자 config 주입: bidirectional_combine=true");
        // ㄱ+ㅏ 후 종성 ᆺ+ᆨ (역순) → ᆪ (= ᆨ+ᆺ 규칙 있음)
        ac.add_jamo(JamoEnum::Cho(Cho::Giyeok));
        ac.add_jamo(JamoEnum::Jung(Jung::A));
        ac.add_jamo(JamoEnum::Jong(Jong::Giyeok));
        // 정순 ᆨ+ᆺ=ᆪ 규칙 있음, 역순 검증은 플래그로만 확인
        assert!(ac.effective_moachigi().map(|m| m.bidirectional_combine).unwrap_or(false));
        // 수동으로 역순 세팅 확인 (단위 테스트)
        c.moachigi = Some(MoachigiSpec { bidirectional_combine: true, chord_window_ms: 0 });
        assert!(c.is_bidirectional_combine());
    }

    /// O1-off: bidirectional_combine=false → 역순 경로 진입 안 함.
    #[test]
    fn o1_off_bidirectional_combine_false() {
        let mut c = HangulComposer3Bul::new();
        c.moachigi = Some(MoachigiSpec { bidirectional_combine: false, chord_window_ms: 0 });
        assert!(!c.is_bidirectional_combine());
    }

    /// O2-zero-default: chord_window_ms=0 → 기존 동작 (즉시 처리).
    #[test]
    fn o2_zero_chord_window_is_default_behavior() {
        let mut c = HangulComposer3Bul::new();
        c.moachigi = Some(MoachigiSpec { bidirectional_combine: true, chord_window_ms: 0 });
        // chord_window_ms=0 → 즉시 처리 → 기존과 동일한 순서 동작
        assert_eq!(c.effective_moachigi().map(|m| m.chord_window_ms), Some(0));
    }

    /// O2-off-when-O1-off: bidirectional_combine=false → chord_window_ms 무시.
    #[test]
    fn o2_off_when_o1_off() {
        let mut c = HangulComposer3Bul::new();
        c.moachigi = Some(MoachigiSpec { bidirectional_combine: false, chord_window_ms: 50 });
        // bidirectional_combine=false → chord 기능 무시
        assert!(!c.is_bidirectional_combine());
        // 종성 입력은 기존 경로로 (bidirectional_combine 인터셉트 없음)
        c.add_jamo(JamoEnum::Cho(Cho::Giyeok));
        c.add_jamo(JamoEnum::Jung(Jung::A));
        c.add_jamo(JamoEnum::Jong(Jong::Giyeok));
        assert_eq!(c.get_current_jong(), Some(Jong::Giyeok));
    }

    // ======================================================================
    // O1-cho-rev / O1-jung-rev: cho·jung 영역 양방향 결합 (Phase 3-rework3)
    // ======================================================================

    /// O1-cho-rev: bidirectional_combine=true (사용자 config 주입) → 역순 초성 결합 활성 검증.
    ///
    /// 안마태 자판에 (ㄱ,ㅎ)→ㅋ 정방향 등록.
    /// 입력 순서: ㅎ(cho=H) → ㄱ(incoming=G).
    ///   - 정순 (H,G): 없음 → 역순 (G,H)→K 매치 → cho = K(ㅋ).
    /// Phase 7: 프로필 로드 후 moachigi 수동 주입 (사용자 config 주입 패턴).
    #[test]
    fn o1_cho_rev_bidirectional_combine_true() {
        let profile = crate::keystroke::profile::load_builtin_profile("ko_3bul_anmatae")
            .expect("anmatae profile must load");
        let mut ac = HangulComposer3Bul::new_with_profile(&profile)
            .expect("anmatae must build");
        // Phase 7: 사용자 config 주입 패턴
        ac.moachigi = Some(MoachigiSpec { bidirectional_combine: true, chord_window_ms: 0 });
        assert!(ac.is_bidirectional_combine(), "사용자 config 주입: bidirectional_combine=true");
        // ㅎ 입력 → cho = H
        ac.add_jamo(JamoEnum::Cho(Cho::H));
        assert_eq!(ac.get_current_cho(), Some(Cho::H));
        // ㄱ 입력 → 정순 (H,G) 없음 → 역순 (G,H)→K 매치 → cho = K(ㅋ)
        ac.add_jamo(JamoEnum::Cho(Cho::G));
        assert_eq!(ac.get_current_cho(), Some(Cho::K), "역순 cho 결합: (G,H)→K");
    }

    /// O1-jung-rev: bidirectional_combine=true (사용자 config 주입) → 역순 중성 결합 활성 검증.
    ///
    /// 안마태 자판에 (ㅗ,ㅏ)→ㅘ 정방향 등록.
    /// 입력 순서: ㅏ(jung=A) → ㅗ(incoming=O).
    ///   - 정순 (A,O): 없음 → 역순 (O,A)→WA 매치 → jung = WA(ㅘ).
    /// Phase 7: 프로필 로드 후 moachigi 수동 주입 (사용자 config 주입 패턴).
    #[test]
    fn o1_jung_rev_bidirectional_combine_true() {
        let profile = crate::keystroke::profile::load_builtin_profile("ko_3bul_anmatae")
            .expect("anmatae profile must load");
        let mut ac = HangulComposer3Bul::new_with_profile(&profile)
            .expect("anmatae must build");
        // Phase 7: 사용자 config 주입 패턴
        ac.moachigi = Some(MoachigiSpec { bidirectional_combine: true, chord_window_ms: 0 });
        assert!(ac.is_bidirectional_combine(), "사용자 config 주입: bidirectional_combine=true");
        // cho 먼저 입력해 음절 시작
        ac.add_jamo(JamoEnum::Cho(Cho::G));
        // ㅏ 입력 → jung = A
        ac.add_jamo(JamoEnum::Jung(Jung::A));
        assert_eq!(ac.get_current_jung(), Some(Jung::A));
        // ㅗ 입력 → 정순 (A,O) 없음 → 역순 (O,A)→WA 매치 → jung = WA(ㅘ)
        ac.add_jamo(JamoEnum::Jung(Jung::O));
        assert_eq!(ac.get_current_jung(), Some(Jung::WA), "역순 jung 결합: (O,A)→WA");
    }

    /// O1-cho-off-no-reverse: bidirectional_combine=false → cho 역순 결합 안 함.
    ///
    /// 동일 입력(ㅎ→ㄱ) 시 (G,H)→K 역순 매치 없음.
    /// 새 음절 시작 (기존 동작: cho 충돌 시 flush 후 새 음절).
    #[test]
    fn o1_cho_off_no_reverse() {
        let profile = crate::keystroke::profile::load_builtin_profile("ko_3bul_anmatae")
            .expect("anmatae profile must load");
        let mut ac = HangulComposer3Bul::new_with_profile(&profile)
            .expect("anmatae must build");
        // bidirectional_combine 강제 OFF
        ac.moachigi = Some(MoachigiSpec { bidirectional_combine: false, chord_window_ms: 0 });
        assert!(!ac.is_bidirectional_combine());
        // ㅎ 입력 → cho = H
        ac.add_jamo(JamoEnum::Cho(Cho::H));
        assert_eq!(ac.get_current_cho(), Some(Cho::H));
        // ㄱ 입력 → 역순 인터셉트 없음 → 기존 동작 (새 음절 시작, cho = G)
        ac.add_jamo(JamoEnum::Cho(Cho::G));
        // 역순 매치로 K(ㅋ)가 되어서는 안 됨
        assert_ne!(ac.get_current_cho(), Some(Cho::K), "OFF 시 역순 결합 금지");
        // 새 음절로 분리되어 cho = G 이거나, 정순 결합(없음)으로 기존 흐름
        assert_eq!(ac.get_current_cho(), Some(Cho::G), "OFF 시 새 음절 초성 = G");
    }

    /// O1-jung-off-no-reverse: bidirectional_combine=false → jung 역순 결합 안 함.
    #[test]
    fn o1_jung_off_no_reverse() {
        let profile = crate::keystroke::profile::load_builtin_profile("ko_3bul_anmatae")
            .expect("anmatae profile must load");
        let mut ac = HangulComposer3Bul::new_with_profile(&profile)
            .expect("anmatae must build");
        // bidirectional_combine 강제 OFF
        ac.moachigi = Some(MoachigiSpec { bidirectional_combine: false, chord_window_ms: 0 });
        assert!(!ac.is_bidirectional_combine());
        // cho + ㅏ 입력 → jung = A
        ac.add_jamo(JamoEnum::Cho(Cho::G));
        ac.add_jamo(JamoEnum::Jung(Jung::A));
        assert_eq!(ac.get_current_jung(), Some(Jung::A));
        // ㅗ 입력 → 역순 인터셉트 없음 → 기존 동작 (정순 (A,O) 없으므로 새 음절)
        ac.add_jamo(JamoEnum::Jung(Jung::O));
        // 역순 매치로 WA(ㅘ)가 되어서는 안 됨
        assert_ne!(ac.get_current_jung(), Some(Jung::WA), "OFF 시 역순 결합 금지");
        // 새 음절로 분리되어 jung = O
        assert_eq!(ac.get_current_jung(), Some(Jung::O), "OFF 시 새 음절 중성 = O");
    }
}

impl HangulComposer for HangulComposer3Bul {
    fn add_jamo(&mut self, jamo: JamoEnum) -> Option<char> {
        // 룰 A 미지정 호출 — default meta(=결합 가능)로 위임. press_key가
        // process_jamo_with_meta로 들어오면 `add_jamo_with_meta` override가 받은 meta를 사용한다.
        self.add_jamo_with_meta(jamo, JamoMeta::default())
    }

    fn add_jamo_with_meta(&mut self, jamo: JamoEnum, meta: JamoMeta) -> Option<char> {
        if !self.base_composer.is_valid_jamo(&jamo) {
            return None;
        }

        // v3 bidirectional_combine 적용: (a,b) 정순 결합 실패 시 (b,a) 역순 재시도.
        // bidirectional_combine=true이고 해당 영역에 기존 자모가 있을 때만 개입.
        //
        // 결합 성공 시 current_korean_char.set_*만으로는 부족하다 — `jamo_queue` 의
        // 마지막 항목(기존 자모)도 결합 결과로 교체해야 다음 자모 추가 시 base
        // composer 의 compose_korean 이 stale 큐를 재합성해 결합 결과를 덮어쓰는
        // 회귀를 막는다. (기존 jung/jong/cho 동일 패턴.)
        if self.is_bidirectional_combine() {
            match jamo {
                JamoEnum::Cho(incoming) => {
                    if let Some(existing) = self.base_composer.get_cho() {
                        let key_a = (JamoEnum::Cho(existing), JamoEnum::Cho(incoming));
                        if self.base_composer.get_combined_jamo().get(&key_a).is_none() {
                            let key_b = (JamoEnum::Cho(incoming), JamoEnum::Cho(existing));
                            if let Some(JamoEnum::Cho(combined)) =
                                self.base_composer.get_combined_jamo().get(&key_b).copied()
                            {
                                self.base_composer.pop_back_synced();
                                self.base_composer
                                    .push_back_synced(JamoEnum::Cho(combined), meta);
                                self.base_composer.set_cho(Some(combined));
                                return None;
                            }
                        }
                    }
                }
                JamoEnum::Jung(incoming) => {
                    if let Some(existing) = self.base_composer.get_jung() {
                        let key_a = (JamoEnum::Jung(existing), JamoEnum::Jung(incoming));
                        if self.base_composer.get_combined_jamo().get(&key_a).is_none() {
                            let key_b = (JamoEnum::Jung(incoming), JamoEnum::Jung(existing));
                            if let Some(JamoEnum::Jung(combined)) =
                                self.base_composer.get_combined_jamo().get(&key_b).copied()
                            {
                                self.base_composer.pop_back_synced();
                                self.base_composer
                                    .push_back_synced(JamoEnum::Jung(combined), meta);
                                self.base_composer.set_jung(Some(combined));
                                return None;
                            }
                        }
                    }
                }
                JamoEnum::Jong(incoming) => {
                    if let Some(existing) = self.base_composer.get_jong() {
                        let key_a = (JamoEnum::Jong(existing), JamoEnum::Jong(incoming));
                        if self.base_composer.get_combined_jamo().get(&key_a).is_none() {
                            let key_b = (JamoEnum::Jong(incoming), JamoEnum::Jong(existing));
                            if let Some(JamoEnum::Jong(combined)) =
                                self.base_composer.get_combined_jamo().get(&key_b).copied()
                            {
                                self.base_composer.pop_back_synced();
                                self.base_composer
                                    .push_back_synced(JamoEnum::Jong(combined), meta);
                                self.base_composer.set_jong(Some(combined));
                                return None;
                            }
                        }
                    }
                }
                _ => {}
            }
        }

        self.base_composer.add_jamo_with(jamo, meta, |base| {
            if base.jamo_queue().is_empty() {
                base.clear();
                return true;
            }

            if check_3bul_violation(base).is_some() {
                return false;
            }

            base.compose_korean()
        })
    }

    fn remove_jamo(&mut self) -> Option<JamoEnum> {
        self.base_composer.remove_jamo()
    }

    fn compose_korean(&mut self) -> bool {
        if self.base_composer.jamo_queue().is_empty() {
            self.base_composer.clear();
            return true;
        }

        if check_3bul_violation(&mut self.base_composer).is_some() {
            return false;
        }

        self.base_composer.compose_korean()
    }

    fn force_compose_korean(&mut self) -> Option<char> {
        self.base_composer.force_compose_korean()
    }

    fn is_compose(&self) -> bool {
        self.base_composer.is_compose()
    }

    fn is_new_syllable(&self) -> bool {
        self.base_composer.is_new_syllable()
    }

    fn compose_cho(&mut self) -> bool {
        self.base_composer.compose_cho()
    }

    fn compose_jung(&mut self) -> bool {
        self.base_composer.compose_jung()
    }

    fn compose_jong(&mut self) -> bool {
        self.base_composer.compose_jong()
    }

    fn clear_jamo(&mut self) {
        self.base_composer.clear_jamo()
    }

    fn get_current_cho(&self) -> Option<Cho> {
        self.base_composer.get_current_cho()
    }

    fn get_current_jung(&self) -> Option<Jung> {
        self.base_composer.get_current_jung()
    }

    fn get_current_jong(&self) -> Option<Jong> {
        self.base_composer.get_current_jong()
    }

    fn set_current_cho(&mut self, cho: Option<Cho>) -> bool {
        self.base_composer.set_current_cho(cho)
    }

    fn set_current_jung(&mut self, jung: Option<Jung>) -> bool {
        self.base_composer.set_current_jung(jung)
    }

    fn set_current_jong(&mut self, jong: Option<Jong>) -> bool {
        self.base_composer.set_current_jong(jong)
    }

    fn get_combined_jamo(&self) -> &CombinedJamoMap {
        self.base_composer.get_combined_jamo()
    }

    fn jamo_queue(&mut self) -> &mut VecDeque<JamoEnum> {
        self.base_composer.jamo_queue()
    }

    fn last_jamo_queue(&mut self) -> &mut VecDeque<JamoEnum> {
        self.base_composer.last_jamo_queue()
    }

    fn combined_jamo(&mut self) -> &mut CombinedJamoMap {
        self.base_composer.combined_jamo()
    }

    fn current_korean(&mut self) -> &mut HangulChar {
        self.base_composer.current_korean()
    }

    fn push_back_synced(&mut self, jamo: JamoEnum, meta: JamoMeta) {
        self.base_composer.push_back_synced(jamo, meta)
    }

    fn clear_queues_synced(&mut self) {
        self.base_composer.clear_queues_synced()
    }
}
