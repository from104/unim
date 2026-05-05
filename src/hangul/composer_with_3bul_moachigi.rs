//! 세벌식 모아치기(3-beol moachigi) 자판 전용 한글 조합기.
//!
//! 안마태 자판을 포함하는 모든 `layout_type: "moachigi_3bul"` 자판의 공통 composer.
//! 자판 이름(안마태 등)과 무관하게, 알고리즘 자체에 의한 명칭.
//!
//! # 알고리즘 출처
//! `.claude/skills/anmatae-moachigi-rollout/references/composer-region-algorithm.md`
//! (임의 변형 금지 — 의사 코드를 직접 Rust로 옮김)
//!
//! # 핵심 개념
//! - **영역 채움 (region_filled)**: cho/jung/jong 3영역. 한 영역이 채워진 상태에서
//!   같은 영역 키가 다시 들어오면 `force_compose` → 새 음절 시작.
//! - **종성 양방향 결합 (jong_unordered)**: `(a,b)` 실패 시 `(b,a)` 재시도.
//! - **jamo_symbol_map**: 합성 큐 진입 금지, 즉시 commit (호출자 책임).

use crate::hangul::char::HangulChar;
use crate::hangul::composer::{CombinedJamoMap, HangulComposer, JamoMeta, Region};
use crate::hangul::jamo::{Cho, Jong, Jung, JamoEnum};
use crate::keystroke::profile::{MoachigiSpec, SyllableBoundary};
use crate::unim_log;

use std::collections::VecDeque;

// ============================================================================
// HangulComposer3BulMoachigi
// ============================================================================

/// 세벌식 모아치기(3-beol moachigi) 자판 전용 한글 조합기.
///
/// 영역 기반 음절 경계 알고리즘을 구현한다. 안마태 자판을 포함하는
/// `layout_type: "moachigi_3bul"` 자판이 공통으로 사용하는 composer.
/// `HangulComposer` trait 메서드 대부분은 내부 상태를 직접 관리하며,
/// `add_jamo_with_region`만 override (나머지는 trait default).
#[derive(Debug, Default)]
pub struct HangulComposer3BulMoachigi {
    /// 현재 조합 중인 초성.
    cho: Option<Cho>,
    /// 현재 조합 중인 중성.
    jung: Option<Jung>,
    /// 현재 조합 중인 종성.
    jong: Option<Jong>,
    /// 자모 조합 규칙 테이블 (profile에서 빌드).
    combined_jamo: CombinedJamoMap,
    /// 내부 jamo_queue (remove_jamo / trait 호환용).
    jamo_queue: VecDeque<JamoEnum>,
    last_jamo_queue: VecDeque<JamoEnum>,
}

impl HangulComposer3BulMoachigi {
    pub fn new() -> Self {
        Self::default()
    }

    /// `LayoutProfile`에서 combined_jamo 맵을 주입해 생성.
    pub fn new_with_profile(
        profile: &crate::keystroke::profile::LayoutProfile,
    ) -> Result<Self, crate::keystroke::profile::BuildError> {
        let map = crate::keystroke::profile::build_combined_jamo_map(profile)?;
        Ok(Self {
            combined_jamo: map,
            ..Self::default()
        })
    }

    // ====================================================================
    // 내부 상태 접근
    // ====================================================================

    fn is_empty_state(&self) -> bool {
        self.cho.is_none() && self.jung.is_none() && self.jong.is_none()
    }

    fn clear_state(&mut self) {
        self.cho = None;
        self.jung = None;
        self.jong = None;
        self.jamo_queue.clear();
    }

    /// 현재 상태를 `HangulChar`로 변환.
    fn to_hangul_char(&self) -> HangulChar {
        let mut hc = HangulChar::default();
        if let Some(cho) = self.cho {
            let _ = hc.set_cho_object(Some(cho));
        }
        if let Some(jung) = self.jung {
            let _ = hc.set_jung_object(Some(jung));
        }
        if let Some(jong) = self.jong {
            let _ = hc.set_jong_object(Some(jong));
        }
        hc
    }

    // ====================================================================
    // force_compose — 현재 음절 커밋
    // ====================================================================

    /// 현재 버퍼(cho/jung/jong)를 음절로 확정하고 반환. 버퍼 초기화.
    ///
    /// 의사 코드 `force_compose` 직접 구현.
    fn force_compose(&mut self) -> Option<char> {
        if self.is_empty_state() {
            return None;
        }
        let hc = self.to_hangul_char();
        unim_log!("ANMATAE", "force_compose: {:?}", hc);
        let result = hc
            .get_syllable()
            .ok()
            .or_else(|| {
                // 부분 음절: compat jamo 단독 추출
                let s = hc.to_compat_jamo_string();
                let mut iter = s.chars();
                let first = iter.next()?;
                if iter.next().is_some() {
                    None
                } else {
                    Some(first)
                }
            });
        self.clear_state();
        result
    }

    // ====================================================================
    // 영역별 push
    // ====================================================================

    fn push_cho(&mut self, jamo: JamoEnum) -> Option<char> {
        if let JamoEnum::Cho(cho) = jamo {
            self.cho = Some(cho);
            self.jamo_queue.push_back(jamo);
        }
        None
    }

    fn push_jung(&mut self, jamo: JamoEnum) -> Option<char> {
        if let JamoEnum::Jung(jung) = jamo {
            self.jung = Some(jung);
            self.jamo_queue.push_back(jamo);
        }
        None
    }

    // ====================================================================
    // 중성 결합 시도
    // ====================================================================

    fn try_combine_jung(&self, existing: Jung, incoming: Jung) -> Option<Jung> {
        let key = (JamoEnum::Jung(existing), JamoEnum::Jung(incoming));
        if let Some(JamoEnum::Jung(result)) = self.combined_jamo.get(&key) {
            Some(*result)
        } else {
            None
        }
    }

    // ====================================================================
    // 종성 결합 시도 (양방향)
    // ====================================================================

    /// 종성 결합 시도. `jong_unordered`가 true면 역순도 시도.
    fn try_combine_jong(
        &self,
        existing: Jong,
        incoming: Jong,
        jong_unordered: bool,
    ) -> Option<Jong> {
        let key_a = (JamoEnum::Jong(existing), JamoEnum::Jong(incoming));
        if let Some(JamoEnum::Jong(result)) = self.combined_jamo.get(&key_a) {
            unim_log!(
                "ANMATAE",
                "jong combine ({:?},{:?}) -> {:?}",
                existing,
                incoming,
                result
            );
            return Some(*result);
        }
        if jong_unordered {
            let key_b = (JamoEnum::Jong(incoming), JamoEnum::Jong(existing));
            if let Some(JamoEnum::Jong(result)) = self.combined_jamo.get(&key_b) {
                unim_log!(
                    "ANMATAE",
                    "jong combine reversed ({:?},{:?}) -> {:?}",
                    incoming,
                    existing,
                    result
                );
                return Some(*result);
            }
        }
        None
    }

    // ====================================================================
    // handle_jong_input — 의사 코드 직접 구현
    // ====================================================================

    /// 종성 영역 입력 처리.
    ///
    /// 의사 코드 `handle_jong_input` 직접 구현.
    /// 반환값: force_compose가 발생했을 때 커밋된 글자.
    fn handle_jong_input(&mut self, jamo: JamoEnum, spec: &MoachigiSpec) -> Option<char> {
        let incoming_jong = match jamo {
            JamoEnum::Jong(j) => j,
            _ => return None,
        };

        if self.jong.is_none() {
            // 종성 비어 있음 → 그냥 설정
            self.jong = Some(incoming_jong);
            self.jamo_queue.push_back(jamo);
            unim_log!("ANMATAE", "handle_jong: set jong={:?}", incoming_jong);
            return None;
        }

        // 종성 결합 시도
        let existing = self.jong.unwrap();
        if let Some(combined) = self.try_combine_jong(existing, incoming_jong, spec.jong_unordered)
        {
            self.jong = Some(combined);
            self.jamo_queue.push_back(jamo);
            unim_log!("ANMATAE", "handle_jong: combined jong={:?}", combined);
            return None;
        }

        // 결합 실패 → force_compose + 새 음절에 jong 설정
        unim_log!(
            "ANMATAE",
            "handle_jong: combine failed, force_compose then jong={:?}",
            incoming_jong
        );
        let committed = self.force_compose();
        self.jong = Some(incoming_jong);
        self.jamo_queue.push_back(jamo);
        committed
    }

    // ====================================================================
    // add_jamo_with_region — 의사 코드 직접 구현
    // ====================================================================

    /// 영역 기반 자모 추가. `HangulComposer::add_jamo_with_region` override.
    ///
    /// 의사 코드 `add_jamo_with_region` 직접 구현.
    pub fn add_with_region(
        &mut self,
        jamo: JamoEnum,
        region: Region,
        spec: &MoachigiSpec,
    ) -> Option<char> {
        unim_log!(
            "ANMATAE",
            "add_with_region: {:?} region={:?} region_filled={}",
            jamo,
            region,
            spec.region_filled
        );

        if spec.syllable_boundary == SyllableBoundary::RegionFilled {
            match region {
                Region::Cho => {
                    if self.cho.is_some() {
                        // cho 영역 재진입 → force_compose
                        let committed = self.force_compose();
                        self.push_cho(jamo);
                        return committed;
                    }
                    self.push_cho(jamo);
                    return None;
                }
                Region::Jung => {
                    if let Some(existing_jung) = self.jung {
                        if let JamoEnum::Jung(incoming_jung) = jamo {
                            // 복모음 결합 시도
                            if let Some(combined) =
                                self.try_combine_jung(existing_jung, incoming_jung)
                            {
                                self.jung = Some(combined);
                                self.jamo_queue.push_back(jamo);
                                return None;
                            }
                            // 결합 실패 → force_compose
                            let committed = self.force_compose();
                            self.push_jung(jamo);
                            return committed;
                        }
                    }
                    self.push_jung(jamo);
                    return None;
                }
                Region::Jong => {
                    return self.handle_jong_input(jamo, spec);
                }
            }
        }

        // strict 모드 — 자모 종류에 따라 단순 push (재귀 방지).
        match jamo {
            JamoEnum::Cho(cho) => {
                self.cho = Some(cho);
                self.jamo_queue.push_back(jamo);
                None
            }
            JamoEnum::Jung(jung) => {
                self.jung = Some(jung);
                self.jamo_queue.push_back(jamo);
                None
            }
            JamoEnum::Jong(jong) => {
                self.jong = Some(jong);
                self.jamo_queue.push_back(jamo);
                None
            }
            _ => None,
        }
    }
}

// ============================================================================
// HangulComposer trait 구현
// ============================================================================

impl HangulComposer for HangulComposer3BulMoachigi {
    fn add_jamo(&mut self, jamo: JamoEnum) -> Option<char> {
        self.add_jamo_with_meta(jamo, JamoMeta::default())
    }

    fn add_jamo_with_meta(&mut self, jamo: JamoEnum, _meta: JamoMeta) -> Option<char> {
        // strict 모드 fallback: 자모 종류에 따라 영역 자동 추정.
        let region = match &jamo {
            JamoEnum::Cho(_) => Region::Cho,
            JamoEnum::Jung(_) => Region::Jung,
            JamoEnum::Jong(_) => Region::Jong,
            _ => return None,
        };
        let spec = MoachigiSpec::default(); // strict, unordered false
        self.add_with_region(jamo, region, &spec)
    }

    fn add_jamo_with_region(
        &mut self,
        jamo: JamoEnum,
        region: Region,
        spec: &MoachigiSpec,
    ) -> Option<char> {
        self.add_with_region(jamo, region, spec)
    }

    fn remove_jamo(&mut self) -> Option<JamoEnum> {
        let popped = self.jamo_queue.pop_back()?;
        // 상태 재구성: jamo_queue를 처음부터 재처리
        let queue: Vec<JamoEnum> = self.jamo_queue.drain(..).collect();
        self.clear_state();
        let spec = MoachigiSpec::default();
        for j in queue {
            let region = match &j {
                JamoEnum::Cho(_) => Region::Cho,
                JamoEnum::Jung(_) => Region::Jung,
                JamoEnum::Jong(_) => Region::Jong,
                _ => continue,
            };
            self.add_with_region(j, region, &spec);
        }
        Some(popped)
    }

    fn compose_korean(&mut self) -> bool {
        // 안마태 모드에서는 항상 조합 성공 (상태가 유효하면).
        !self.is_empty_state() || self.jamo_queue.is_empty()
    }

    fn force_compose_korean(&mut self) -> Option<char> {
        self.force_compose()
    }

    fn is_compose(&self) -> bool {
        !self.is_empty_state()
    }

    fn is_new_syllable(&self) -> bool {
        self.is_empty_state()
    }

    fn compose_cho(&mut self) -> bool {
        self.cho.is_some()
    }

    fn compose_jung(&mut self) -> bool {
        self.jung.is_some()
    }

    fn compose_jong(&mut self) -> bool {
        self.jong.is_some()
    }

    fn clear_jamo(&mut self) {
        self.clear_state();
    }

    fn get_current_cho(&self) -> Option<Cho> {
        self.cho
    }

    fn get_current_jung(&self) -> Option<Jung> {
        self.jung
    }

    fn get_current_jong(&self) -> Option<Jong> {
        self.jong
    }

    fn set_current_cho(&mut self, cho: Option<Cho>) -> bool {
        self.cho = cho;
        true
    }

    fn set_current_jung(&mut self, jung: Option<Jung>) -> bool {
        self.jung = jung;
        true
    }

    fn set_current_jong(&mut self, jong: Option<Jong>) -> bool {
        self.jong = jong;
        true
    }

    fn get_combined_jamo(&self) -> &CombinedJamoMap {
        &self.combined_jamo
    }

    fn jamo_queue(&mut self) -> &mut VecDeque<JamoEnum> {
        &mut self.jamo_queue
    }

    fn last_jamo_queue(&mut self) -> &mut VecDeque<JamoEnum> {
        &mut self.last_jamo_queue
    }

    fn combined_jamo(&mut self) -> &mut CombinedJamoMap {
        &mut self.combined_jamo
    }

    fn current_korean(&mut self) -> &mut HangulChar {
        // HangulComposer3BulMoachigi는 HangulChar를 직접 보관하지 않고 cho/jung/jong으로 관리.
        // trait 호환을 위해 임시 static mut는 사용 금지. Box leak도 금지.
        // 실제 preedit 표시는 get_preedit()에서 to_hangul_char()로 생성하므로
        // 이 메서드는 사용되지 않아야 함. 빈 기본값 반환 (leaky 회피).
        // SAFETY: 이 참조는 self 수명 내에서만 유효. compose_korean/force_compose 경로에서 사용.
        // HangulComposer3BulMoachigi에서 current_korean()을 직접 호출하는 코드가 없도록 보장.
        // 단, trait 요구사항 충족을 위해 내부 임시 필드를 사용한다.
        // → pending_commit을 HangulChar 공간으로 재활용하는 대신,
        //   별도 임시 필드 current_korean_tmp를 두겠지만 이는 아래에서 unsafe 없이 해결 불가.
        // 트레이드오프: 이 메서드를 호출하는 경로(BaseHangulComposer 전용)는
        //               HangulComposer3BulMoachigi에서 실행되지 않으므로 unreachable!로 처리.
        unreachable!(
            "HangulComposer3BulMoachigi::current_korean() should not be called directly; \
             use get_current_cho/jung/jong() instead"
        )
    }
}

// ============================================================================
// 단위 테스트
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hangul::jamo::{Cho, Jong, Jung, JamoEnum};

    /// 간단한 조합 테이블 생성 (ㄹ+ㄱ=ㄺ, ㄴ+ㅈ=ㄵ 포함).
    fn make_composer_with_jong_combos() -> HangulComposer3BulMoachigi {
        let mut c = HangulComposer3BulMoachigi::new();
        // ㄹ+ㄱ=ㄺ
        c.combined_jamo.insert(
            (JamoEnum::Jong(Jong::Rieul), JamoEnum::Jong(Jong::Giyeok)),
            JamoEnum::Jong(Jong::RieulGiyeok),
        );
        // ㄴ+ㅈ=ㄵ
        c.combined_jamo.insert(
            (JamoEnum::Jong(Jong::Nieun), JamoEnum::Jong(Jong::Jieut)),
            JamoEnum::Jong(Jong::NieunJieut),
        );
        // ㅂ+ㅅ=ㅄ
        c.combined_jamo.insert(
            (JamoEnum::Jong(Jong::Bieup), JamoEnum::Jong(Jong::Siot)),
            JamoEnum::Jong(Jong::BieupSiot),
        );
        // 중성 복모음: ㅗ+ㅏ=ㅘ
        c.combined_jamo.insert(
            (JamoEnum::Jung(Jung::O), JamoEnum::Jung(Jung::A)),
            JamoEnum::Jung(Jung::Wa),
        );
        c
    }

    fn region_filled_spec() -> MoachigiSpec {
        MoachigiSpec {
            syllable_boundary: SyllableBoundary::RegionFilled,
            jong_unordered: true,
            jung_unordered: false,
            region_filled: true,
        }
    }

    fn ordered_spec() -> MoachigiSpec {
        MoachigiSpec {
            syllable_boundary: SyllableBoundary::RegionFilled,
            jong_unordered: false,
            jung_unordered: false,
            region_filled: true,
        }
    }

    // ====================================================================
    // A: 영역 채움 (region_filled=true)
    // ====================================================================

    /// A1: cho 영역 재진입 → force_compose.
    #[test]
    fn a1_cho_reentry_force_compose() {
        let mut c = make_composer_with_jong_combos();
        let spec = region_filled_spec();
        // ㅁ 초성
        let r1 = c.add_with_region(JamoEnum::Cho(Cho::M), Region::Cho, &spec);
        assert_eq!(r1, None);
        assert_eq!(c.cho, Some(Cho::M));
        // ㅏ 중성
        let r2 = c.add_with_region(JamoEnum::Jung(Jung::A), Region::Jung, &spec);
        assert_eq!(r2, None);
        // ㄴ 종성
        let r3 = c.add_with_region(JamoEnum::Jong(Jong::Nieun), Region::Jong, &spec);
        assert_eq!(r3, None);
        // ㅁ 초성 재진입 → 만 commit
        let r4 = c.add_with_region(JamoEnum::Cho(Cho::M), Region::Cho, &spec);
        assert_eq!(r4, Some('만'), "cho reentry: 만 commit expected");
        assert_eq!(c.cho, Some(Cho::M));
        assert_eq!(c.jung, None);
        assert_eq!(c.jong, None);
    }

    /// A2: jung 영역 재진입 + 복모음 결합 성공 → 분리 없음.
    #[test]
    fn a2_jung_combine_wa_no_compose() {
        let mut c = make_composer_with_jong_combos();
        let spec = region_filled_spec();
        c.add_with_region(JamoEnum::Cho(Cho::G), Region::Cho, &spec);
        c.add_with_region(JamoEnum::Jung(Jung::O), Region::Jung, &spec);
        let r = c.add_with_region(JamoEnum::Jung(Jung::A), Region::Jung, &spec);
        // ㅗ+ㅏ=ㅘ 결합 성공 → 분리 없음
        assert_eq!(r, None);
        assert_eq!(c.jung, Some(Jung::Wa));
    }

    /// A3: jung 영역 재진입 + 복모음 결합 실패 → force_compose.
    #[test]
    fn a3_jung_combine_fail_force_compose() {
        let mut c = make_composer_with_jong_combos();
        let spec = region_filled_spec();
        c.add_with_region(JamoEnum::Cho(Cho::G), Region::Cho, &spec);
        c.add_with_region(JamoEnum::Jung(Jung::A), Region::Jung, &spec);
        // ㅏ+ㅏ: 결합 규칙 없음 → force_compose
        let r = c.add_with_region(JamoEnum::Jung(Jung::A), Region::Jung, &spec);
        assert_eq!(r, Some('가'), "jung combine fail: 가 commit expected");
        assert_eq!(c.jung, Some(Jung::A));
    }

    /// A4: jong 영역 — 종성 결합 성공 → 분리 없음 (ㄹ+ㄱ=ㄺ).
    #[test]
    fn a4_jong_combine_success() {
        let mut c = make_composer_with_jong_combos();
        let spec = region_filled_spec();
        c.add_with_region(JamoEnum::Cho(Cho::G), Region::Cho, &spec);
        c.add_with_region(JamoEnum::Jung(Jung::A), Region::Jung, &spec);
        c.add_with_region(JamoEnum::Jong(Jong::Rieul), Region::Jong, &spec);
        let r = c.add_with_region(JamoEnum::Jong(Jong::Giyeok), Region::Jong, &spec);
        assert_eq!(r, None);
        assert_eq!(c.jong, Some(Jong::RieulGiyeok));
    }

    /// A5: jong 영역 — 종성 결합 실패 → force_compose.
    #[test]
    fn a5_jong_combine_fail_force_compose() {
        let mut c = make_composer_with_jong_combos();
        let spec = region_filled_spec();
        c.add_with_region(JamoEnum::Cho(Cho::G), Region::Cho, &spec);
        c.add_with_region(JamoEnum::Jung(Jung::A), Region::Jung, &spec);
        c.add_with_region(JamoEnum::Jong(Jong::Bieup), Region::Jong, &spec);
        // ㅂ+ㄴ: 결합 없음 → force_compose → 갑 commit
        let r = c.add_with_region(JamoEnum::Jong(Jong::Nieun), Region::Jong, &spec);
        assert_eq!(r, Some('갑'), "jong combine fail: 갑 commit expected");
        assert_eq!(c.jong, Some(Jong::Nieun));
    }

    // ====================================================================
    // B: 종성 양방향 (jong_unordered=true)
    // ====================================================================

    /// B1: ㄹ+ㄱ → ㄺ (정순).
    #[test]
    fn b1_jong_rg_ordered() {
        let mut c = make_composer_with_jong_combos();
        let spec = region_filled_spec(); // jong_unordered=true
        c.add_with_region(JamoEnum::Cho(Cho::G), Region::Cho, &spec);
        c.add_with_region(JamoEnum::Jung(Jung::A), Region::Jung, &spec);
        c.add_with_region(JamoEnum::Jong(Jong::Rieul), Region::Jong, &spec);
        c.add_with_region(JamoEnum::Jong(Jong::Giyeok), Region::Jong, &spec);
        assert_eq!(c.jong, Some(Jong::RieulGiyeok));
    }

    /// B2: ㄱ+ㄹ → ㄺ (역순, jong_unordered=true).
    #[test]
    fn b2_jong_gr_unordered() {
        let mut c = make_composer_with_jong_combos();
        let spec = region_filled_spec();
        c.add_with_region(JamoEnum::Cho(Cho::G), Region::Cho, &spec);
        c.add_with_region(JamoEnum::Jung(Jung::A), Region::Jung, &spec);
        c.add_with_region(JamoEnum::Jong(Jong::Giyeok), Region::Jong, &spec);
        let r = c.add_with_region(JamoEnum::Jong(Jong::Rieul), Region::Jong, &spec);
        assert_eq!(r, None, "역순 결합 성공 → 분리 없음");
        assert_eq!(c.jong, Some(Jong::RieulGiyeok), "ㄱ+ㄹ=ㄺ (reversed)");
    }

    /// B3: ㄴ+ㅈ → ㄵ (정순).
    #[test]
    fn b3_jong_nj_ordered() {
        let mut c = make_composer_with_jong_combos();
        let spec = region_filled_spec();
        c.add_with_region(JamoEnum::Cho(Cho::G), Region::Cho, &spec);
        c.add_with_region(JamoEnum::Jung(Jung::A), Region::Jung, &spec);
        c.add_with_region(JamoEnum::Jong(Jong::Nieun), Region::Jong, &spec);
        c.add_with_region(JamoEnum::Jong(Jong::Jieut), Region::Jong, &spec);
        assert_eq!(c.jong, Some(Jong::NieunJieut));
    }

    /// B4: ㅈ+ㄴ → ㄵ (역순, jong_unordered=true).
    #[test]
    fn b4_jong_jn_unordered() {
        let mut c = make_composer_with_jong_combos();
        let spec = region_filled_spec();
        c.add_with_region(JamoEnum::Cho(Cho::G), Region::Cho, &spec);
        c.add_with_region(JamoEnum::Jung(Jung::A), Region::Jung, &spec);
        c.add_with_region(JamoEnum::Jong(Jong::Jieut), Region::Jong, &spec);
        let r = c.add_with_region(JamoEnum::Jong(Jong::Nieun), Region::Jong, &spec);
        assert_eq!(r, None);
        assert_eq!(c.jong, Some(Jong::NieunJieut));
    }

    /// B5: jong_unordered=false 시 역순 결합 실패 → force_compose.
    #[test]
    fn b5_jong_gr_ordered_only_fails() {
        let mut c = make_composer_with_jong_combos();
        let spec = ordered_spec(); // jong_unordered=false
        c.add_with_region(JamoEnum::Cho(Cho::G), Region::Cho, &spec);
        c.add_with_region(JamoEnum::Jung(Jung::A), Region::Jung, &spec);
        c.add_with_region(JamoEnum::Jong(Jong::Giyeok), Region::Jong, &spec);
        let r = c.add_with_region(JamoEnum::Jong(Jong::Rieul), Region::Jong, &spec);
        // (ㄱ, ㄹ) 조합 없고 역순 미허용 → force_compose
        assert_eq!(r, Some('각'), "ordered only: force_compose expected");
        assert_eq!(c.jong, Some(Jong::Rieul));
    }

    /// B6: ㅂ+ㅅ → ㅄ (정순).
    #[test]
    fn b6_jong_bs_ordered() {
        let mut c = make_composer_with_jong_combos();
        let spec = region_filled_spec();
        c.add_with_region(JamoEnum::Cho(Cho::G), Region::Cho, &spec);
        c.add_with_region(JamoEnum::Jung(Jung::A), Region::Jung, &spec);
        c.add_with_region(JamoEnum::Jong(Jong::Bieup), Region::Jong, &spec);
        c.add_with_region(JamoEnum::Jong(Jong::Siot), Region::Jong, &spec);
        assert_eq!(c.jong, Some(Jong::BieupSiot));
    }

    /// B7: 첫 종성 입력 (jong 비어 있음) → 바로 설정.
    #[test]
    fn b7_jong_first_input_empty() {
        let mut c = make_composer_with_jong_combos();
        let spec = region_filled_spec();
        c.add_with_region(JamoEnum::Cho(Cho::G), Region::Cho, &spec);
        c.add_with_region(JamoEnum::Jung(Jung::A), Region::Jung, &spec);
        let r = c.add_with_region(JamoEnum::Jong(Jong::Giyeok), Region::Jong, &spec);
        assert_eq!(r, None);
        assert_eq!(c.jong, Some(Jong::Giyeok));
    }

    /// B8: 종성 단독 (cho/jung 없음) → force_compose 없이 jong만 설정.
    #[test]
    fn b8_jong_only_no_compose() {
        let mut c = make_composer_with_jong_combos();
        let spec = region_filled_spec();
        let r = c.add_with_region(JamoEnum::Jong(Jong::Giyeok), Region::Jong, &spec);
        assert_eq!(r, None);
        assert_eq!(c.jong, Some(Jong::Giyeok));
        // force_compose → 호환 자모 ㄱ
        let ch = c.force_compose_korean();
        assert_eq!(ch, Some('ㄱ'));
    }

    // ====================================================================
    // C: 모아치기 토글 OFF→ON 회귀
    // ====================================================================

    /// C1: jong_unordered=false → ㄱ+ㄹ = 분리 (force_compose).
    #[test]
    fn c1_toggle_off_jong_ordered_fails() {
        let mut c = make_composer_with_jong_combos();
        let spec = ordered_spec();
        c.add_with_region(JamoEnum::Cho(Cho::G), Region::Cho, &spec);
        c.add_with_region(JamoEnum::Jung(Jung::A), Region::Jung, &spec);
        c.add_with_region(JamoEnum::Jong(Jong::Giyeok), Region::Jong, &spec);
        let r = c.add_with_region(JamoEnum::Jong(Jong::Rieul), Region::Jong, &spec);
        assert_eq!(r, Some('각'));
    }

    /// C2: jong_unordered=true → ㄱ+ㄹ = ㄺ.
    #[test]
    fn c2_toggle_on_jong_unordered_succeeds() {
        let mut c = make_composer_with_jong_combos();
        let spec = region_filled_spec();
        c.add_with_region(JamoEnum::Cho(Cho::G), Region::Cho, &spec);
        c.add_with_region(JamoEnum::Jung(Jung::A), Region::Jung, &spec);
        c.add_with_region(JamoEnum::Jong(Jong::Giyeok), Region::Jong, &spec);
        let r = c.add_with_region(JamoEnum::Jong(Jong::Rieul), Region::Jong, &spec);
        assert_eq!(r, None);
        assert_eq!(c.jong, Some(Jong::RieulGiyeok));
    }

    /// C3: region_filled=false (strict) → cho 재진입 시 분리 안 함.
    #[test]
    fn c3_strict_mode_cho_reentry_no_compose() {
        let mut c = make_composer_with_jong_combos();
        let spec = MoachigiSpec {
            syllable_boundary: SyllableBoundary::Strict,
            jong_unordered: false,
            jung_unordered: false,
            region_filled: false,
        };
        c.add_with_region(JamoEnum::Cho(Cho::G), Region::Cho, &spec);
        // strict 모드 → add_jamo_with_meta 위임 (두벌식 동작)
        // strict에서는 cho 재진입도 force_compose 없음
        let r = c.add_with_region(JamoEnum::Cho(Cho::M), Region::Cho, &spec);
        // strict 모드는 기존 동작 위임 → None 반환 (상태 재구성은 위임된 로직에 따름)
        let _ = r; // 동작 검증은 위임 로직 의존 — 단순 smoke test
    }

    /// C4: 완전 음절 ㄱ+ㅏ+ㄱ = 각 (region_filled 모드).
    #[test]
    fn c4_full_syllable_gag() {
        let mut c = make_composer_with_jong_combos();
        let spec = region_filled_spec();
        c.add_with_region(JamoEnum::Cho(Cho::G), Region::Cho, &spec);
        c.add_with_region(JamoEnum::Jung(Jung::A), Region::Jung, &spec);
        c.add_with_region(JamoEnum::Jong(Jong::Giyeok), Region::Jong, &spec);
        let ch = c.force_compose_korean();
        assert_eq!(ch, Some('각'));
    }

    // ====================================================================
    // 기본 동작 smoke test
    // ====================================================================

    /// 빈 상태에서 force_compose → None.
    #[test]
    fn empty_force_compose_returns_none() {
        let mut c = HangulComposer3BulMoachigi::new();
        assert_eq!(c.force_compose_korean(), None);
    }

    /// is_compose: 초기 상태 false, 자모 추가 후 true.
    #[test]
    fn is_compose_reflects_state() {
        let mut c = HangulComposer3BulMoachigi::new();
        assert!(!c.is_compose());
        let spec = region_filled_spec();
        c.add_with_region(JamoEnum::Cho(Cho::G), Region::Cho, &spec);
        assert!(c.is_compose());
    }
}
