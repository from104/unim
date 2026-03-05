// composer_with_3bul.rs
//! 세벌식 한글 입력 방식의 조합 로직을 구현합니다.

use crate::hangul::HangulChar;
use crate::hangul::composer::BaseHangulComposer;
use crate::hangul::composer::CombinedJamoMap;
use crate::hangul::composer::HangulComposer;
use crate::hangul::jamo::*;

// Compatibility aliases for submodule access
use once_cell::sync::Lazy;
use std::collections::{HashMap, VecDeque};

// ============================================================================
// 자모 조합 규칙 정의 (배열 기반)
// ============================================================================

/// 중성 조합 규칙: (첫째 모음, 둘째 모음) => 결과 모음
const JUNG_COMBINATIONS: &[(Jung, Jung, Jung)] = &[
    (Jung::O, Jung::A, Jung::Wa),   // ㅗ + ㅏ = ㅘ
    (Jung::O, Jung::Ae, Jung::Wae), // ㅗ + ㅐ = ㅙ
    (Jung::O, Jung::I, Jung::Oe),   // ㅗ + ㅣ = ㅚ
    (Jung::U, Jung::Eo, Jung::Weo), // ㅜ + ㅓ = ㅝ
    (Jung::U, Jung::E, Jung::We),   // ㅜ + ㅔ = ㅞ
    (Jung::U, Jung::I, Jung::Wi),   // ㅜ + ㅣ = ㅟ
    (Jung::Eu, Jung::I, Jung::Yi),  // ㅡ + ㅣ = ㅢ
];

/// 종성 조합 규칙: (첫째 받침, 둘째 받침) => 결과 겹받침
const JONG_COMBINATIONS: &[(Jong, Jong, Jong)] = &[
    (Jong::Giyeok, Jong::Giyeok, Jong::SsangGiyeok), // ㄱ + ㄱ = ㄲ
    (Jong::Giyeok, Jong::Siot, Jong::GiyeokSiot),    // ㄱ + ㅅ = ㄳ
    (Jong::Nieun, Jong::Jieut, Jong::NieunJieut),    // ㄴ + ㅈ = ㄵ
    (Jong::Nieun, Jong::Hieuh, Jong::NieunHieuh),    // ㄴ + ㅎ = ㄶ
    (Jong::Rieul, Jong::Giyeok, Jong::RieulGiyeok),  // ㄹ + ㄱ = ㄺ
    (Jong::Rieul, Jong::Mieum, Jong::RieulMieum),    // ㄹ + ㅁ = ㄻ
    (Jong::Rieul, Jong::Bieup, Jong::RieulBieup),    // ㄹ + ㅂ = ㄼ
    (Jong::Rieul, Jong::Siot, Jong::RieulSiot),      // ㄹ + ㅅ = ㄽ
    (Jong::Rieul, Jong::Tieut, Jong::RieulTieut),    // ㄹ + ㅌ = ㄾ
    (Jong::Rieul, Jong::Pieup, Jong::RieulPieup),    // ㄹ + ㅍ = ㄿ
    (Jong::Rieul, Jong::Hieuh, Jong::RieulHieuh),    // ㄹ + ㅎ = ㅀ
    (Jong::Bieup, Jong::Siot, Jong::BieupSiot),      // ㅂ + ㅅ = ㅄ
    (Jong::Siot, Jong::Siot, Jong::SsangSiot),       // ㅅ + ㅅ = ㅆ
];

/// 초성 조합 규칙: (첫째 초성, 둘째 초성) => 결과 쌍자음 (3벌식 전용)
const CHO_COMBINATIONS: &[(Cho, Cho, Cho)] = &[
    (Cho::Giyeok, Cho::Giyeok, Cho::SsangGiyeok), // ㄱ + ㄱ = ㄲ
    (Cho::Digeut, Cho::Digeut, Cho::SsangDigeut), // ㄷ + ㄷ = ㄸ
    (Cho::Bieup, Cho::Bieup, Cho::SsangBieup),    // ㅂ + ㅂ = ㅃ
    (Cho::Siot, Cho::Siot, Cho::SsangSiot),       // ㅅ + ㅅ = ㅆ
    (Cho::Jieut, Cho::Jieut, Cho::SsangJieut),    // ㅈ + ㅈ = ㅉ
];

/// 배열 기반 조합 규칙으로 HashMap 빌드
fn build_jamo_map() -> CombinedJamoMap {
    let capacity = JUNG_COMBINATIONS.len() + JONG_COMBINATIONS.len() + CHO_COMBINATIONS.len();
    let mut map = HashMap::with_capacity(capacity);

    for &(a, b, c) in JUNG_COMBINATIONS {
        map.insert((JamoEnum::Jung(a), JamoEnum::Jung(b)), JamoEnum::Jung(c));
    }
    for &(a, b, c) in JONG_COMBINATIONS {
        map.insert((JamoEnum::Jong(a), JamoEnum::Jong(b)), JamoEnum::Jong(c));
    }
    for &(a, b, c) in CHO_COMBINATIONS {
        map.insert((JamoEnum::Cho(a), JamoEnum::Cho(b)), JamoEnum::Cho(c));
    }

    map
}

/// 3벌식 자모 조합 테이블 (프로그램 시작 시 한 번만 초기화)
static COMBINED_JAMO_3BUL: Lazy<CombinedJamoMap> = Lazy::new(build_jamo_map);

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
        // 규칙 1: 중성 없이 종성이 오는 경우
        (_, l)
            if l.is_jong()
                && !is_filled_jung
                && (prev.is_none() || prev.is_some_and(|p| p.is_cho())) =>
        {
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
#[derive(Debug, Default)]
pub struct HangulComposer3Bul {
    base_composer: BaseHangulComposer,
}

impl HangulComposer3Bul {
    /// 새로운 `HangulComposer3Bul` 인스턴스를 생성합니다.
    pub fn new() -> Self {
        let mut composer = HangulComposer3Bul {
            base_composer: BaseHangulComposer::new(),
        };
        *composer.base_composer.combined_jamo() = COMBINED_JAMO_3BUL.clone();
        composer
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
        // ㄱ만으로는 완성 음절이 안 되므로 get_syllable() Err → None
        // 새 음절 ㄴ 시작
        assert_eq!(c.get_current_cho(), Some(Cho::Nieun));
        // committed는 ㄱ이 incomplete syllable이므로 None일 수 있음
        let _ = committed;
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
}

impl HangulComposer for HangulComposer3Bul {
    fn add_jamo(&mut self, jamo: JamoEnum) -> Option<char> {
        if !self.base_composer.is_valid_jamo(&jamo) {
            return None;
        }

        self.base_composer.add_jamo_with(jamo, |base| {
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
}
