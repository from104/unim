//! 한글 입력 컨텍스트 관리 모듈
//!
//! 키보드 레이아웃, 현재 조합 상태, preedit/committed 문자열 등을 관리합니다.

use crate::hangul::composer::{CombinedJamoMap, HangulComposer, JamoMeta};
use crate::hangul::composer_with_2bul::HangulComposer2Bul;
use crate::hangul::composer_with_3bul::HangulComposer3Bul;
use crate::hangul::jamo::{Cho, Jong, Jung, JamoEnum};
use crate::input_engine::chord_compose::{ChordEntry, ChordEntryKind, compose_chord};
use crate::unim_log;

/// 지원하는 한글 컴포저 타입.
///
/// Phase 3-rework2: `ThreeBulMoachigi` 폐기. 안마태 자판은 `ThreeBul`로 통합.
/// `HangulComposer3Bul`이 `bidirectional_combine` 옵션으로 모아치기 동작 담당.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ComposerType {
    #[default]
    TwoBul,
    ThreeBul,
}

impl ComposerType {
    /// 컴포저 타입에 맞는 인스턴스를 생성합니다.
    fn create_composer(self) -> Box<dyn HangulComposer> {
        match self {
            ComposerType::TwoBul => Box::new(HangulComposer2Bul::new()),
            ComposerType::ThreeBul => Box::new(HangulComposer3Bul::new()),
        }
    }
}

/// 한글 입력 과정을 관리하는 컨텍스트
///
/// # 예시
/// ```ignore
/// let mut ctx = HangulInputContext::new(ComposerType::TwoBul);
/// ctx.process_jamo(JamoEnum::Cho(Cho::G));  // ㄱ
/// ctx.process_jamo(JamoEnum::Jung(Jung::A)); // 가
/// assert_eq!(ctx.get_preedit(), "가");
/// ```
pub struct HangulInputContext {
    composer: Box<dyn HangulComposer>,
    preedit: String,
    committed: String,
    composer_type: ComposerType,
    /// chord inject 후 보관하는 입력 순서 자모 목록.
    /// backspace 시 마지막 자모를 pop 한 뒤 chord_compose 재실행 → 재inject.
    /// inject 외 경로(sequential add_jamo, commit, clear, reset)에서는 비어있음.
    chord_input_order: Vec<JamoEnum>,
}

impl Default for HangulInputContext {
    fn default() -> Self {
        Self::new(ComposerType::default())
    }
}

impl HangulInputContext {
    /// 새로운 `HangulInputContext`를 생성합니다.
    pub fn new(composer_type: ComposerType) -> Self {
        Self {
            composer: composer_type.create_composer(),
            preedit: String::new(),
            committed: String::new(),
            composer_type,
            chord_input_order: Vec::new(),
        }
    }

    /// 자판 프로필(`LayoutProfile`)로부터 컨텍스트를 생성합니다.
    ///
    /// `profile.layout_type`(`"2bul"` / `"3bul"`)에 따라 해당 Composer를
    /// `new_with_profile`로 만들고, combinations + 활성 rule_sets 병합을 적용합니다.
    /// `layout_type`이 이 중 어느 것도 아니면 `TwoBul`로 안전 폴백(영문 계열).
    ///
    /// 스펙: `docs/plans/LAYOUT_PROFILE_V1.md` §5.2, IMPL §2.4.
    pub fn new_with_profile(
        profile: &crate::keystroke::profile::LayoutProfile,
    ) -> Result<Self, crate::keystroke::profile::BuildError> {
        match profile.layout_type.as_str() {
            // Phase 3-rework2: "moachigi_3bul" / "anmatae"도 ThreeBul 경로로 통합.
            // HangulComposer3Bul이 moachigi.bidirectional_combine 옵션으로 처리.
            "3bul" | "moachigi_3bul" | "anmatae" => {
                let composer = HangulComposer3Bul::new_with_profile(profile)?;
                Ok(Self {
                    composer: Box::new(composer),
                    preedit: String::new(),
                    committed: String::new(),
                    composer_type: ComposerType::ThreeBul,
                    chord_input_order: Vec::new(),
                })
            }
            // "2bul" 또는 영문 계열(qwerty/dvorak/...): 한글 조합 경로는 2벌식 기반.
            _ => {
                let composer = HangulComposer2Bul::new_with_profile(profile)?;
                Ok(Self {
                    composer: Box::new(composer),
                    preedit: String::new(),
                    committed: String::new(),
                    composer_type: ComposerType::TwoBul,
                    chord_input_order: Vec::new(),
                })
            }
        }
    }

    /// 자모를 입력받아 처리합니다 (default meta).
    ///
    /// 룰 A를 신경 쓰지 않는 caller(테스트, 자동 한영 변환 등) 호환 경로.
    /// `JamoMeta::default()`(=결합 가능)로 위임.
    ///
    /// # Returns
    /// 입력 처리 성공 여부
    #[inline]
    pub fn process_jamo(&mut self, jamo: JamoEnum) -> bool {
        self.process_jamo_with_meta(jamo, JamoMeta::default())
    }

    /// 자모와 키-출처 메타데이터를 함께 입력받아 처리합니다.
    ///
    /// 룰 A(`vowel_combine_head`) 같은 키-별 속성을 composer 큐까지 전달.
    /// 키맵 → key_meta_map → JamoMeta 변환은 호출자(`press_key`)가 담당.
    ///
    /// # Returns
    /// 입력 처리 성공 여부
    pub fn process_jamo_with_meta(&mut self, jamo: JamoEnum, meta: JamoMeta) -> bool {
        // sequential 자모 추가: chord inject 추적 종료.
        self.chord_input_order.clear();

        unim_log!(
            "CONTEXT",
            "process_jamo_with_meta: {:?} meta={:?}",
            jamo,
            meta
        );
        unim_log!(
            "CONTEXT",
            "  BEFORE: preedit='{}', committed='{}', composer={:?}",
            self.preedit,
            self.committed,
            self.composer.current_korean()
        );

        if let Some(committed_char) = self.composer.add_jamo_with_meta(jamo, meta) {
            unim_log!("CONTEXT", "  -> 음절 완성: '{}'", committed_char);
            self.committed.push(committed_char);
        }

        self.update_preedit();
        unim_log!(
            "CONTEXT",
            "  AFTER: preedit='{}', committed='{}'",
            self.preedit,
            self.committed
        );
        true
    }

    /// 마지막 입력된 자모를 제거합니다 (Backspace).
    ///
    /// chord inject 모드(`chord_input_order` 비어있지 않음)일 때:
    ///   - 입력 순서의 마지막 자모 pop → 남은 자모로 `compose_chord` 재실행 → 재inject.
    ///   - 자모가 0개 남으면 preedit 비우고 종료.
    ///
    /// sequential 모드일 때: 기존 composer.remove_jamo() 경로.
    pub fn backspace(&mut self) -> bool {
        // ── chord inject 모드 ──────────────────────────────────────────────
        if !self.chord_input_order.is_empty() {
            self.chord_input_order.pop(); // 마지막 자모 제거
            if self.chord_input_order.is_empty() {
                // 자모 전부 제거 → preedit 비움
                self.composer.clear_queues_synced();
                self.composer.clear_jamo();
                self.preedit.clear();
                return true;
            }
            // 남은 자모로 chord_compose 재실행
            let remaining = self.chord_input_order.clone();
            let combined_jamo = self.composer.get_combined_jamo().clone();
            let entries: Vec<ChordEntry> = remaining
                .iter()
                .enumerate()
                .map(|(i, j)| ChordEntry {
                    kind: ChordEntryKind::Jamo(*j),
                    input_order: i as u8,
                    meta: JamoMeta::default(),
                })
                .collect();
            let result = compose_chord(&entries, &combined_jamo);
            // inject_to_preedit=true면 재inject (비자모·fallback 없음 보장)
            if result.inject_to_preedit && result.non_jamos.is_empty() {
                self.composer.clear_queues_synced();
                self.composer.clear_jamo();
                let default_meta = JamoMeta::default();
                if let Some(c) = result.cho {
                    self.composer.push_back_synced(JamoEnum::Cho(c), default_meta);
                }
                if let Some(j) = result.jung {
                    self.composer.push_back_synced(JamoEnum::Jung(j), default_meta);
                }
                if let Some(jo) = result.jong {
                    self.composer.push_back_synced(JamoEnum::Jong(jo), default_meta);
                }
                self.composer.compose_korean();
                self.update_preedit();
            } else {
                // 재합성 실패(조합 안 되는 자모 조합 등) → preedit 비움
                self.chord_input_order.clear();
                self.composer.clear_queues_synced();
                self.composer.clear_jamo();
                self.preedit.clear();
            }
            return true;
        }

        // ── sequential 모드 ───────────────────────────────────────────────
        if self.composer.is_compose() {
            if self.composer.remove_jamo().is_some() {
                self.update_preedit();
                return true;
            }
            // 이상 상태 - 안전하게 초기화
            self.preedit.clear();
            return false;
        }

        // 조합 중이 아닐 때: committed에서 마지막 문자 제거
        self.committed.pop().is_some()
    }

    /// 현재 조합 중인 문자열(preedit)을 반환합니다.
    #[inline]
    pub fn get_preedit(&self) -> &str {
        &self.preedit
    }

    /// 현재 확정된 문자열(committed)을 반환합니다.
    #[inline]
    pub fn get_committed(&self) -> &str {
        &self.committed
    }

    /// 현재 조합 중인 내용을 강제로 확정(commit)합니다.
    pub fn commit(&mut self) -> Option<char> {
        self.chord_input_order.clear();
        let c = self.composer.force_compose_korean();
        if let Some(ch) = c {
            self.committed.push(ch);
        }
        self.preedit.clear();
        c
    }

    /// 비-한글 문자를 확정 문자열에 추가합니다.
    #[inline]
    pub fn commit_char(&mut self, c: char) {
        self.committed.push(c);
    }

    /// 조합을 확정하고 문자를 추가합니다.
    pub fn append_to_committed(&mut self, c: char) {
        self.commit();
        self.committed.push(c);
    }

    /// 입력 컨텍스트를 완전히 초기화합니다.
    pub fn clear(&mut self) {
        self.chord_input_order.clear();
        self.composer.force_compose_korean();
        self.preedit.clear();
        self.committed.clear();
    }

    /// IME 리셋 - 현재 조합을 포기하고 상태만 초기화합니다.
    /// (committed 문자열은 유지하지 않음)
    pub fn reset(&mut self) {
        self.chord_input_order.clear();
        self.composer.force_compose_korean();
        self.preedit.clear();
        self.committed.clear();
    }

    /// Committed 문자열만 비웁니다 (preedit 유지).
    #[inline]
    pub fn clear_committed(&mut self) {
        self.committed.clear();
    }

    /// 조합 중 상태(컴포저 큐 + preedit)만 비웁니다 (committed 유지).
    ///
    /// chord preview 가 이전 단계에 inject 한 부분 음절을 폐기하고
    /// 새 입력 결과로 다시 채울 때 사용한다. `commit()` 과 달리
    /// `force_compose_korean()` 을 호출하지 않으므로 진행 중 음절을
    /// committed 에 흘려보내지 않는다.
    pub fn clear_composing(&mut self) {
        self.chord_input_order.clear();
        self.composer.clear_queues_synced();
        self.composer.clear_jamo();
        self.preedit.clear();
    }

    /// composer 큐의 마지막 자모 한 개를 제거하고 preedit 을 재계산합니다.
    ///
    /// `backspace()` 가 `chord_input_order` 가 비어있지 않으면 chord 분기로 가서
    /// 다른 데이터를 건드리는 점을 회피하기 위한 직접 경로다. chord preview 의
    /// sequential→atomic 전이 시 `process_jamo_with_meta` 가 추가한 자모 1개를
    /// 안전하게 되돌리는 용도. `chord_input_order` 는 손대지 않는다.
    pub fn pop_last_jamo(&mut self) -> Option<JamoEnum> {
        let popped = self.composer.remove_jamo();
        self.update_preedit();
        popped
    }

    /// 현재 조합 중인지 여부를 반환합니다.
    #[inline]
    pub fn is_composing(&self) -> bool {
        self.composer.is_compose() || !self.preedit.is_empty()
    }

    /// 현재 조합 상태가 "초성만" 채워진 상태인지 반환합니다.
    /// 세벌식 `key_meta.context_alt.when == "choseong_only"` 분기에서 사용.
    #[inline]
    pub fn is_only_cho_filled(&self) -> bool {
        self.composer.get_current_cho().is_some()
            && self.composer.get_current_jung().is_none()
            && self.composer.get_current_jong().is_none()
    }

    /// "중성만" 채워진 상태 (cho 없이 jung만, jong 없음).
    /// `context_alt.when == "jungseong_only"` 분기.
    #[inline]
    pub fn is_only_jung_filled(&self) -> bool {
        self.composer.get_current_cho().is_none()
            && self.composer.get_current_jung().is_some()
            && self.composer.get_current_jong().is_none()
    }

    /// "초성+중성" 채워짐, 종성 없음.
    /// `context_alt.when == "cho_jung_filled"` 분기.
    #[inline]
    pub fn is_cho_jung_filled(&self) -> bool {
        self.composer.get_current_cho().is_some()
            && self.composer.get_current_jung().is_some()
            && self.composer.get_current_jong().is_none()
    }

    /// 종성이 들어 있는 상태 (cho/jung 동반 여부 무관).
    /// `context_alt.when == "jongseong_filled"` 분기.
    #[inline]
    pub fn is_jong_filled(&self) -> bool {
        self.composer.get_current_jong().is_some()
    }

    /// 큐의 마지막 자모가 초성인지.
    /// `context_alt.when == "last_is_cho"` 분기.
    pub fn last_jamo_is_cho(&mut self) -> bool {
        matches!(self.composer.jamo_queue().back(), Some(JamoEnum::Cho(_)))
    }

    /// 큐의 마지막 자모가 중성인지.
    /// `context_alt.when == "last_is_jung"` 분기.
    pub fn last_jamo_is_jung(&mut self) -> bool {
        matches!(self.composer.jamo_queue().back(), Some(JamoEnum::Jung(_)))
    }

    /// 큐의 마지막 자모가 종성인지.
    /// `context_alt.when == "last_is_jong"` 분기.
    pub fn last_jamo_is_jong(&mut self) -> bool {
        matches!(self.composer.jamo_queue().back(), Some(JamoEnum::Jong(_)))
    }

    /// chord 결합 결과를 preedit 에 직접 주입합니다 (Phase 3 모아쓰기 경로).
    ///
    /// 호출 전 기존 preedit 는 flush_preedit() 으로 확정되어 있어야 합니다.
    /// jamo_queue 를 [cho?, jung?, jong?] 순으로 재구성하고 compose_korean() 을 통해
    /// current_korean_char 를 갱신한다. is_compose()=true 상태를 유지하므로
    /// 이후 flush_preedit() → commit() → force_compose_korean() 경로가 올바르게 동작.
    ///
    /// # 인자
    /// - `cho`: 초성 (없으면 None)
    /// - `jung`: 중성 (없으면 None)
    /// - `jong`: 종성 (없으면 None)
    /// - `input_order`: chord 입력 순서 자모 목록 (backspace 역제거용). 빈 Vec 이면 추적 비활성.
    ///
    /// 모두 None 이면 no-op (빈 preedit).
    pub fn inject_chord_syllable(
        &mut self,
        cho: Option<Cho>,
        jung: Option<Jung>,
        jong: Option<Jong>,
        input_order: Vec<JamoEnum>,
    ) {
        if cho.is_none() && jung.is_none() && jong.is_none() {
            return;
        }
        // chord_input_order 갱신 — backspace 역제거 추적.
        self.chord_input_order = input_order;

        // 큐 초기화 (호출자가 이미 flush_preedit 완료 — 기존 조합 없음).
        // push_back_synced 로 jamo_queue+meta_queue 를 동기화하면서 재구성.
        self.composer.clear_queues_synced();
        self.composer.clear_jamo();
        // [cho?, jung?, jong?] 순서로 push_back_synced.
        // compose_korean() 이 jamo_queue+meta_queue 를 left-fold 하므로
        // 각 영역에 정확히 1개씩만 push 하면 올바른 음절이 조합된다.
        let default_meta = JamoMeta::default();
        if let Some(c) = cho {
            self.composer.push_back_synced(JamoEnum::Cho(c), default_meta);
        }
        if let Some(j) = jung {
            self.composer.push_back_synced(JamoEnum::Jung(j), default_meta);
        }
        if let Some(jo) = jong {
            self.composer.push_back_synced(JamoEnum::Jong(jo), default_meta);
        }
        // compose_korean() 으로 current_korean_char 갱신 → update_preedit() 로 preedit 설정.
        self.composer.compose_korean();
        self.update_preedit();
    }

    /// 현재 자모 조합 테이블을 반환합니다 (chord_compose 에 전달용).
    #[inline]
    pub fn get_combined_jamo(&self) -> &CombinedJamoMap {
        self.composer.get_combined_jamo()
    }

    /// 현재 사용 중인 컴포저 타입을 반환합니다.
    #[inline]
    pub fn get_composer_type(&self) -> ComposerType {
        self.composer_type
    }

    /// 컴포저 타입을 변경합니다 (조합 중인 내용은 확정됨).
    pub fn set_composer_type(&mut self, composer_type: ComposerType) {
        if self.composer_type != composer_type {
            self.commit(); // 현재 조합 확정
            self.composer = composer_type.create_composer();
            self.composer_type = composer_type;
        }
    }

    /// preedit 문자열을 업데이트합니다 (내부용).
    fn update_preedit(&mut self) {
        let current = self.composer.current_korean();
        unim_log!(
            "CONTEXT",
            "update_preedit: composer.current_korean()={:?}",
            current
        );

        self.preedit = match current.get_syllable() {
            Ok(ch) => {
                unim_log!("CONTEXT", "  -> get_syllable() = Ok('{}')", ch);
                ch.to_string()
            }
            Err(_) => {
                let compat = current.to_compat_jamo_string();
                unim_log!(
                    "CONTEXT",
                    "  -> get_syllable() = Err, to_compat_jamo_string()='{}'",
                    compat
                );
                compat
            }
        };
    }
}

// === 유닛 테스트 ===
#[cfg(test)]
mod tests {
    use super::*;
    use crate::hangul::jamo::*;

    #[test]
    fn test_basic_composition() {
        let mut ctx = HangulInputContext::new(ComposerType::TwoBul);

        ctx.process_jamo(JamoEnum::Cho(Cho::G));
        assert!(ctx.is_composing());
        assert_eq!(ctx.get_preedit(), "ㄱ");

        ctx.process_jamo(JamoEnum::Jung(Jung::A));
        assert_eq!(ctx.get_preedit(), "가");

        ctx.process_jamo(JamoEnum::Cho(Cho::G)); // 종성
        assert_eq!(ctx.get_preedit(), "각");
        assert_eq!(ctx.get_committed(), "");
    }

    #[test]
    fn test_commit() {
        let mut ctx = HangulInputContext::new(ComposerType::TwoBul);
        ctx.process_jamo(JamoEnum::Cho(Cho::G));
        ctx.process_jamo(JamoEnum::Jung(Jung::A));
        ctx.process_jamo(JamoEnum::Cho(Cho::G));

        assert_eq!(ctx.commit(), Some('각'));
        assert!(!ctx.is_composing());
        assert_eq!(ctx.get_preedit(), "");
        assert_eq!(ctx.get_committed(), "각");
    }

    #[test]
    fn test_backspace() {
        let mut ctx = HangulInputContext::new(ComposerType::TwoBul);
        ctx.process_jamo(JamoEnum::Cho(Cho::G));
        ctx.process_jamo(JamoEnum::Jung(Jung::A));
        ctx.process_jamo(JamoEnum::Cho(Cho::G));

        assert!(ctx.backspace()); // 각 -> 가
        assert_eq!(ctx.get_preedit(), "가");
        assert!(ctx.backspace()); // 가 -> ㄱ
        assert_eq!(ctx.get_preedit(), "ㄱ");
        assert!(ctx.backspace()); // ㄱ -> ""
        assert_eq!(ctx.get_preedit(), "");
        assert!(!ctx.backspace()); // 빈 상태
    }

    #[test]
    fn test_dokkaebi() {
        let mut ctx = HangulInputContext::new(ComposerType::TwoBul);
        ctx.process_jamo(JamoEnum::Cho(Cho::G));
        ctx.process_jamo(JamoEnum::Jung(Jung::A));
        ctx.process_jamo(JamoEnum::Cho(Cho::G)); // 각

        ctx.process_jamo(JamoEnum::Jung(Jung::A)); // 도깨비불
        assert_eq!(ctx.get_preedit(), "가");
        assert_eq!(ctx.get_committed(), "가");
    }

    #[test]
    fn test_clear() {
        let mut ctx = HangulInputContext::new(ComposerType::TwoBul);
        ctx.process_jamo(JamoEnum::Cho(Cho::G));
        ctx.process_jamo(JamoEnum::Jung(Jung::A));
        ctx.commit();
        ctx.process_jamo(JamoEnum::Cho(Cho::N));

        ctx.clear();
        assert_eq!(ctx.get_committed(), "");
        assert_eq!(ctx.get_preedit(), "");
        assert!(!ctx.is_composing());
    }

    #[test]
    fn test_composer_type_change() {
        let mut ctx = HangulInputContext::new(ComposerType::TwoBul);
        assert_eq!(ctx.get_composer_type(), ComposerType::TwoBul);

        ctx.process_jamo(JamoEnum::Cho(Cho::G));
        ctx.process_jamo(JamoEnum::Jung(Jung::A));

        ctx.set_composer_type(ComposerType::ThreeBul);
        assert_eq!(ctx.get_composer_type(), ComposerType::ThreeBul);
        assert_eq!(ctx.get_committed(), "가"); // 기존 조합 확정됨
        assert_eq!(ctx.get_preedit(), "");
    }

    #[test]
    fn test_3bul_basic() {
        let mut ctx = HangulInputContext::new(ComposerType::ThreeBul);
        ctx.process_jamo(JamoEnum::Cho(Cho::G));
        assert_eq!(ctx.get_preedit(), "ㄱ");
    }

    // ========================================================================
    // ContextCondition helper 단위 테스트
    // ========================================================================

    /// 빈 상태 — 모든 조건 false 외 is_only_jung_filled/is_jong_filled도 false.
    #[test]
    fn context_helpers_empty_state() {
        let mut ctx = HangulInputContext::new(ComposerType::ThreeBul);
        assert!(!ctx.is_composing());
        assert!(!ctx.is_only_cho_filled());
        assert!(!ctx.is_only_jung_filled());
        assert!(!ctx.is_cho_jung_filled());
        assert!(!ctx.is_jong_filled());
        assert!(!ctx.last_jamo_is_cho());
        assert!(!ctx.last_jamo_is_jung());
        assert!(!ctx.last_jamo_is_jong());
    }

    /// 초성만 채워짐.
    #[test]
    fn context_helpers_choseong_only() {
        let mut ctx = HangulInputContext::new(ComposerType::ThreeBul);
        ctx.process_jamo(JamoEnum::Cho(Cho::G));
        assert!(ctx.is_composing());
        assert!(ctx.is_only_cho_filled());
        assert!(!ctx.is_only_jung_filled());
        assert!(!ctx.is_cho_jung_filled());
        assert!(!ctx.is_jong_filled());
        assert!(ctx.last_jamo_is_cho());
        assert!(!ctx.last_jamo_is_jung());
        assert!(!ctx.last_jamo_is_jong());
    }

    /// 중성만 (cho 없이 jung만).
    #[test]
    fn context_helpers_jungseong_only() {
        let mut ctx = HangulInputContext::new(ComposerType::ThreeBul);
        ctx.process_jamo(JamoEnum::Jung(Jung::A));
        assert!(ctx.is_composing());
        assert!(!ctx.is_only_cho_filled());
        assert!(ctx.is_only_jung_filled());
        assert!(!ctx.is_cho_jung_filled());
        assert!(!ctx.is_jong_filled());
        assert!(ctx.last_jamo_is_jung());
    }

    /// 초성+중성, 종성 없음.
    #[test]
    fn context_helpers_cho_jung_filled() {
        let mut ctx = HangulInputContext::new(ComposerType::ThreeBul);
        ctx.process_jamo(JamoEnum::Cho(Cho::G));
        ctx.process_jamo(JamoEnum::Jung(Jung::A));
        assert!(ctx.is_composing());
        assert!(!ctx.is_only_cho_filled());
        assert!(!ctx.is_only_jung_filled());
        assert!(ctx.is_cho_jung_filled());
        assert!(!ctx.is_jong_filled());
        assert!(ctx.last_jamo_is_jung());
    }

    /// 종성까지 채워짐.
    #[test]
    fn context_helpers_jongseong_filled() {
        let mut ctx = HangulInputContext::new(ComposerType::ThreeBul);
        ctx.process_jamo(JamoEnum::Cho(Cho::G));
        ctx.process_jamo(JamoEnum::Jung(Jung::A));
        ctx.process_jamo(JamoEnum::Jong(Jong::Giyeok));
        assert!(ctx.is_composing());
        assert!(!ctx.is_cho_jung_filled());
        assert!(ctx.is_jong_filled());
        assert!(ctx.last_jamo_is_jong());
        assert!(!ctx.last_jamo_is_jung());
    }

    #[test]
    fn test_default() {
        let ctx = HangulInputContext::default();
        assert_eq!(ctx.get_composer_type(), ComposerType::TwoBul);
    }

    #[test]
    fn new_with_profile_2bul_from_builtin() {
        let profile = crate::keystroke::profile::load_builtin_profile("ko_2bulstd").unwrap();
        let mut ctx = HangulInputContext::new_with_profile(&profile).unwrap();
        assert_eq!(ctx.get_composer_type(), ComposerType::TwoBul);
        // 기본 조합 동작 확인: ㄱ + ㅏ + ㄱ = 각
        ctx.process_jamo(JamoEnum::Cho(Cho::G));
        ctx.process_jamo(JamoEnum::Jung(Jung::A));
        ctx.process_jamo(JamoEnum::Cho(Cho::G));
        assert_eq!(ctx.get_preedit(), "각");
    }

    #[test]
    fn new_with_profile_3bul_from_builtin() {
        let profile = crate::keystroke::profile::load_builtin_profile("ko_3bul390").unwrap();
        let mut ctx = HangulInputContext::new_with_profile(&profile).unwrap();
        assert_eq!(ctx.get_composer_type(), ComposerType::ThreeBul);
        ctx.process_jamo(JamoEnum::Cho(Cho::G));
        ctx.process_jamo(JamoEnum::Jung(Jung::A));
        assert_eq!(ctx.get_preedit(), "가");
    }

    #[test]
    fn new_with_profile_english_type_falls_back_to_2bul() {
        // 영문 프로필(qwerty 등)도 한글 컨텍스트 생성 경로상 2벌식 composer로 폴백.
        let profile = crate::keystroke::profile::load_builtin_profile("en_qwerty").unwrap();
        let ctx = HangulInputContext::new_with_profile(&profile).unwrap();
        assert_eq!(ctx.get_composer_type(), ComposerType::TwoBul);
    }

    // ========================================================================
    // Phase 4: chord inject backspace 단위 테스트
    // ========================================================================

    /// 시나리오 1: ㅎㄱ → chord "ㅋ" → BS → "ㅎ" → BS → ""
    /// ㅎ+ㄱ 은 combined_jamo 조합이 없으므로 현 테스트에서는
    /// 직접 inject_chord_syllable 로 preedit "ㅋ" 를 주입하고
    /// input_order=[ㅎ,ㄱ] 로 추적한 뒤 backspace 2번 동작을 검증.
    #[test]
    fn backspace_after_chord_inject_pops_to_h() {
        let mut ctx = HangulInputContext::new(ComposerType::ThreeBul);
        // chord 결과 "ㅋ"(cho-only) 를 직접 inject.
        // input_order 는 [ㅎ, ㄱ] (입력 순서).
        ctx.inject_chord_syllable(
            Some(Cho::K), // ㅋ
            None,
            None,
            vec![JamoEnum::Cho(Cho::H), JamoEnum::Cho(Cho::G)],
        );
        assert_eq!(ctx.get_preedit(), "ㅋ");

        // backspace 1번 → input_order 에서 ㄱ pop → [ㅎ] 남음 → chord_compose([ㅎ]) → "ㅎ"
        assert!(ctx.backspace());
        assert_eq!(ctx.get_preedit(), "ㅎ");

        // backspace 1번 → [ㅎ] pop → [] 비어있음 → preedit ""
        assert!(ctx.backspace());
        assert_eq!(ctx.get_preedit(), "");
    }

    /// 시나리오 2: ㄱㅏㅁ chord → "감" → BS → "가" → BS → "ㄱ" → BS → ""
    #[test]
    fn backspace_after_chord_gam_pops_step_by_step() {
        let mut ctx = HangulInputContext::new(ComposerType::ThreeBul);
        // "감" inject (cho=ㄱ, jung=ㅏ, jong=ㅁ), input_order=[ㄱ,ㅏ,ㅁ]
        ctx.inject_chord_syllable(
            Some(Cho::G),
            Some(Jung::A),
            Some(Jong::M),
            vec![
                JamoEnum::Cho(Cho::G),
                JamoEnum::Jung(Jung::A),
                JamoEnum::Jong(Jong::M),
            ],
        );
        assert_eq!(ctx.get_preedit(), "감");

        // BS → ㅁ pop → [ㄱ, ㅏ] → chord_compose([ㄱ,ㅏ]) → "가"
        assert!(ctx.backspace());
        assert_eq!(ctx.get_preedit(), "가");

        // BS → ㅏ pop → [ㄱ] → chord_compose([ㄱ]) → "ㄱ"
        assert!(ctx.backspace());
        assert_eq!(ctx.get_preedit(), "ㄱ");

        // BS → ㄱ pop → [] → preedit ""
        assert!(ctx.backspace());
        assert_eq!(ctx.get_preedit(), "");
    }

    /// 시나리오 3: sequential 입력 후 backspace 는 chord_input_order 비어있으므로 기존 경로.
    #[test]
    fn backspace_after_sequential_unchanged() {
        let mut ctx = HangulInputContext::new(ComposerType::TwoBul);
        ctx.process_jamo(JamoEnum::Cho(Cho::G));
        ctx.process_jamo(JamoEnum::Jung(Jung::A));
        assert_eq!(ctx.get_preedit(), "가");

        assert!(ctx.backspace()); // ㅏ 제거 → "ㄱ"
        assert_eq!(ctx.get_preedit(), "ㄱ");

        assert!(ctx.backspace()); // ㄱ 제거 → ""
        assert_eq!(ctx.get_preedit(), "");

        assert!(!ctx.backspace()); // 빈 상태
    }

    /// 시나리오 4: chord inject 후 sequential 자모 추가 → chord_input_order 비워짐 → 이후 BS 는 sequential 경로.
    #[test]
    fn backspace_after_chord_then_sequential_clears_chord_order() {
        let mut ctx = HangulInputContext::new(ComposerType::ThreeBul);
        // "가" chord inject
        ctx.inject_chord_syllable(
            Some(Cho::G),
            Some(Jung::A),
            None,
            vec![JamoEnum::Cho(Cho::G), JamoEnum::Jung(Jung::A)],
        );
        assert_eq!(ctx.get_preedit(), "가");

        // sequential ㅁ 추가 → chord_input_order 비워짐 → "감"
        ctx.process_jamo(JamoEnum::Jong(Jong::M));
        assert_eq!(ctx.get_preedit(), "감");

        // backspace → sequential 경로 → ㅁ 제거 → "가"
        assert!(ctx.backspace());
        assert_eq!(ctx.get_preedit(), "가");
    }
}
