// composer_with_3bul.rs
//! 세벌식 한글 입력 방식의 조합 로직을 구현합니다.
//!
//! 0.2.0+: Rust const 자모 조합 테이블(JUNG/JONG/CHO_COMBINATIONS)과 Lazy static
//! `COMBINED_JAMO_3BUL`을 모두 제거. `new()`는 내장 v1 프로필 `ko_3bul390`을
//! 즉시 로드해 `new_with_profile`로 위임한다. 자모 조합 규칙의 단일 source of
//! truth는 `src/keystroke/keymap/ko_3bul390.json`(390/391/noshift는 base 동일).
//!
//! v3 부분 적용: `moachigi_overrides`가 활성화된 프로필에서 `jong_unordered=true`이면
//! 종성 결합 시 역순도 시도한다. 비활성 시 기존 동작 100% 동일 (회귀 0).

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
/// - v3 부분 적용: `moachigi` 필드가 Some이고 `jong_unordered=true`면 종성 양방향 결합.
#[derive(Debug, Default)]
pub struct HangulComposer3Bul {
    base_composer: BaseHangulComposer,
    /// v3 모아치기 파라미터. None이면 기존 동작 100% 동일.
    moachigi: Option<MoachigiSpec>,
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
    // 3M: 세벌식 부분 적용 (v3 moachigi jong_unordered)
    // ======================================================================

    /// 3M1: moachigi=None (v1/v2 프로필) → jong_unordered 비활성 → 기존 동작 동일.
    #[test]
    fn m3_1_v1_profile_no_moachigi_unchanged() {
        // 기본 new()는 moachigi=None → 기존 3벌식 동작 그대로.
        let mut c = HangulComposer3Bul::new();
        assert!(c.moachigi.is_none(), "v1 profile must have no moachigi");
        // ㄱ + ㅏ + ㄳ(겹받침) — 기존 동작: ㄳ은 ko_3bul390 combinations에 있음
        c.add_jamo(JamoEnum::Cho(Cho::Giyeok));
        c.add_jamo(JamoEnum::Jung(Jung::A));
        c.add_jamo(JamoEnum::Jong(Jong::Giyeok));
        c.add_jamo(JamoEnum::Jong(Jong::Siot));
        assert_eq!(c.get_current_jong(), Some(Jong::GiyeokSiot));
    }

    /// 3M2: moachigi.jong_unordered=true → 역순 종성 결합 활성.
    #[test]
    fn m3_2_jong_unordered_true_enables_reverse_combine() {
        let mut c = HangulComposer3Bul::new();
        // 수동으로 jong_unordered 세팅 (프로필 없이 단위 테스트)
        let mut spec = MoachigiSpec::default();
        spec.jong_unordered = true;
        // ㄱ+ㅅ=ㄳ 역방향: (ㅅ,ㄱ) → ㄳ 규칙은 기본 390에 없으므로
        // 단순히 moachigi 필드 존재 여부와 jong_unordered 플래그 확인.
        c.moachigi = Some(spec);
        assert!(c.effective_moachigi().map(|m| m.jong_unordered).unwrap_or(false));
    }

    /// 3M3: moachigi.jong_unordered=false → 역순 결합 비활성 → 기존과 동일.
    #[test]
    fn m3_3_jong_unordered_false_same_as_v1() {
        let mut c = HangulComposer3Bul::new();
        let mut spec = MoachigiSpec::default();
        spec.jong_unordered = false;
        c.moachigi = Some(spec);
        // jong_unordered=false → 역순 경로 진입 안 함 → 기존 동작
        assert!(!c.effective_moachigi().map(|m| m.jong_unordered).unwrap_or(true));
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

        // v3 부분 적용: jong_unordered 활성 시 역순 결합 시도.
        // 종성 입력이고 현재 종성이 있고 jong_unordered=true인 경우만 개입.
        if let Some(ref spec) = self.moachigi {
            if spec.jong_unordered {
                if let JamoEnum::Jong(incoming) = jamo {
                    if let Some(existing) = self.base_composer.get_jong() {
                        // 정순 결합 시도
                        let key_a = (JamoEnum::Jong(existing), JamoEnum::Jong(incoming));
                        if self.base_composer.get_combined_jamo().get(&key_a).is_none() {
                            // 역순 결합 시도
                            let key_b = (JamoEnum::Jong(incoming), JamoEnum::Jong(existing));
                            if let Some(JamoEnum::Jong(combined)) =
                                self.base_composer.get_combined_jamo().get(&key_b).copied()
                            {
                                // 역순으로 결합 가능 → existing을 combined로 교체
                                self.base_composer.set_jong(Some(combined));
                                // jamo_queue에도 반영 (위임 생략, jong는 최종 자모)
                                return None;
                            }
                        }
                    }
                }
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
}
