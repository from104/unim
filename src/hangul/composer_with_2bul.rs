// composer_with_2bul.rs
//! 두벌식 한글 입력 방식의 조합 로직을 구현합니다.

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

/// 배열 기반 조합 규칙으로 HashMap 빌드
fn build_jamo_map() -> CombinedJamoMap {
    let capacity = JUNG_COMBINATIONS.len() + JONG_COMBINATIONS.len();
    let mut map = HashMap::with_capacity(capacity);

    for &(a, b, c) in JUNG_COMBINATIONS {
        map.insert((JamoEnum::Jung(a), JamoEnum::Jung(b)), JamoEnum::Jung(c));
    }
    for &(a, b, c) in JONG_COMBINATIONS {
        map.insert((JamoEnum::Jong(a), JamoEnum::Jong(b)), JamoEnum::Jong(c));
    }

    map
}

/// 2벌식 자모 조합 테이블 (프로그램 시작 시 한 번만 초기화)
static COMBINED_JAMO_2BUL: Lazy<CombinedJamoMap> = Lazy::new(build_jamo_map);

// ============================================================================
// 2벌식 조합 규칙 검사 헬퍼
// ============================================================================

/// 2벌식 조합 규칙 위반 유형
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Compose2BulViolation {
    /// 초성 없이 [중성 → 종성] - 불가 (예: 'ㅏㄱ')
    JungThenJongWithoutCho,
    /// [종성 → 중성] - 도깨비불 현상으로 처리해야 함
    JongThenJung,
    /// 초성 없이 [중성 → 초성] - 분리해야 함 (예: 'ㅏ' + 'ㄱ')
    JungThenChoWithoutCho,
}

/// 현재 큐 상태에서 2벌식 규칙 위반 여부를 검사합니다.
///
/// # Returns
/// - `Some(위반유형)`: 규칙 위반 시
/// - `None`: 규칙 위반 없음, 조합 계속 가능
fn check_2bul_violation(base: &mut BaseHangulComposer) -> Option<Compose2BulViolation> {
    let queue = base.jamo_queue();
    if queue.len() < 1 {
        return None;
    }

    let last = *queue.back().unwrap();
    let prev = if queue.len() > 1 {
        Some(queue[queue.len() - 2])
    } else {
        None
    };

    match (prev, last) {
        // 규칙 1: 초성 없이 중성 뒤에 종성
        (Some(p), l) if p.is_jung() && l.is_jong() && !base.is_filled_cho() => {
            Some(Compose2BulViolation::JungThenJongWithoutCho)
        }
        // 규칙 2: 종성 뒤에 중성 (도깨비불)
        (Some(p), l) if p.is_jong() && l.is_jung() => Some(Compose2BulViolation::JongThenJung),
        // 규칙 3: 초성 없이 중성 뒤에 초성
        (Some(p), l) if p.is_jung() && l.is_cho() && !base.is_filled_cho() => {
            Some(Compose2BulViolation::JungThenChoWithoutCho)
        }
        // 규칙 위반 없음
        _ => None,
    }
}

// ============================================================================
// HangulComposer2Bul 구현
// ============================================================================

/// 두벌식 한글 입력 방식의 조합 로직을 구현한 컴포저입니다.
///
/// # 특징
/// - 도깨비불 현상 처리: 종성 + 중성 입력 시 종성을 분리하여 새 글자의 초성으로 이동
/// - 초성→종성 변환: 초성+중성 상태에서 초성 입력 시 자동으로 종성으로 변환
#[derive(Debug, Default)]
pub struct HangulComposer2Bul {
    base_composer: BaseHangulComposer,
}

impl HangulComposer2Bul {
    /// 새로운 `HangulComposer2Bul` 인스턴스를 생성합니다.
    pub fn new() -> Self {
        let mut composer = HangulComposer2Bul {
            base_composer: BaseHangulComposer::new(),
        };
        *composer.base_composer.combined_jamo() = COMBINED_JAMO_2BUL.clone();
        composer
    }

    /// 도깨비불 현상 처리
    ///
    /// 종성 다음에 중성이 입력되면:
    /// 1. 마지막 종성을 큐에서 제거
    /// 2. 현재까지의 글자를 완성
    /// 3. 제거된 종성을 초성으로 변환하여 새 글자 시작
    fn handle_dokkaebi_effect(&mut self, jamo: JamoEnum) -> Option<Option<char>> {
        let last_jamo = self.base_composer.jamo_queue().back().copied();

        if let Some(JamoEnum::Jong(jong)) = last_jamo {
            if jamo.is_jung() {
                self.base_composer.jamo_queue().pop_back();
                let completed = self.force_compose_korean();

                if let Ok(new_cho) = jong.to_cho() {
                    self.add_jamo(JamoEnum::Cho(new_cho));
                    self.add_jamo(jamo);
                    return Some(completed);
                }
            }
        }
        None
    }

    /// 초성→종성 변환 처리
    ///
    /// 초성+중성이 채워진 상태에서 초성이 입력되면 종성으로 변환합니다.
    fn handle_cho_after_jung(&mut self, jamo: JamoEnum) -> Option<Option<char>> {
        if self.base_composer.is_filled_cho() && self.base_composer.is_filled_jung() {
            if let JamoEnum::Cho(cho) = jamo {
                if let Ok(jong) = cho.to_jong() {
                    return Some(self.add_jamo(JamoEnum::Jong(jong)));
                }
            }
        }
        None
    }
}

impl HangulComposer for HangulComposer2Bul {
    fn add_jamo(&mut self, jamo: JamoEnum) -> Option<char> {
        if !self.base_composer.is_valid_jamo(&jamo) {
            return None;
        }

        // 1. 초성+중성 상태에서 초성 → 종성 변환
        if let Some(result) = self.handle_cho_after_jung(jamo) {
            return result;
        }

        // 2. 도깨비불 현상 처리
        if let Some(result) = self.handle_dokkaebi_effect(jamo) {
            return result;
        }

        // 3. 조합 규칙 검사를 포함한 자모 추가
        self.base_composer.add_jamo_with(jamo, |base| {
            if base.jamo_queue().is_empty() {
                base.clear();
                return true;
            }

            // 2벌식 규칙 위반 검사
            if check_2bul_violation(base).is_some() {
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

        if check_2bul_violation(&mut self.base_composer).is_some() {
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
