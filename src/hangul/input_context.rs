//! 한글 입력 컨텍스트 관리 모듈
//!
//! 키보드 레이아웃, 현재 조합 상태, preedit/committed 문자열 등을 관리합니다.

use crate::hangul::composer::{HangulComposer, JamoMeta};
use crate::hangul::composer_with_2bul::HangulComposer2Bul;
use crate::hangul::composer_with_3bul::HangulComposer3Bul;
use crate::hangul::composer_with_3bul_moachigi::HangulComposer3BulMoachigi;
use crate::hangul::jamo::JamoEnum;
use crate::unim_log;

/// 지원하는 한글 컴포저 타입
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ComposerType {
    #[default]
    TwoBul,
    ThreeBul,
    /// v3 — 세벌식 모아치기(3-beol moachigi) 전용 composer.
    /// 안마태 자판을 포함하는 모든 `layout_type: "moachigi_3bul"` 자판에 사용.
    ThreeBulMoachigi,
}

impl ComposerType {
    /// 컴포저 타입에 맞는 인스턴스를 생성합니다.
    fn create_composer(self) -> Box<dyn HangulComposer> {
        match self {
            ComposerType::TwoBul => Box::new(HangulComposer2Bul::new()),
            ComposerType::ThreeBul => Box::new(HangulComposer3Bul::new()),
            ComposerType::ThreeBulMoachigi => Box::new(HangulComposer3BulMoachigi::new()),
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
            "3bul" => {
                let composer = HangulComposer3Bul::new_with_profile(profile)?;
                Ok(Self {
                    composer: Box::new(composer),
                    preedit: String::new(),
                    committed: String::new(),
                    composer_type: ComposerType::ThreeBul,
                })
            }
            // v3 — 세벌식 모아치기 전용 composer (안마태 포함).
            // JSON "type" 정식 값: "moachigi_3bul". "anmatae"도 수용 (레거시·테스트 호환).
            "moachigi_3bul" | "anmatae" => {
                let composer = HangulComposer3BulMoachigi::new_with_profile(profile)?;
                Ok(Self {
                    composer: Box::new(composer),
                    preedit: String::new(),
                    committed: String::new(),
                    composer_type: ComposerType::ThreeBulMoachigi,
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
    pub fn backspace(&mut self) -> bool {
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
        self.composer.force_compose_korean();
        self.preedit.clear();
        self.committed.clear();
    }

    /// IME 리셋 - 현재 조합을 포기하고 상태만 초기화합니다.
    /// (committed 문자열은 유지하지 않음)
    pub fn reset(&mut self) {
        self.composer.force_compose_korean();
        self.preedit.clear();
        self.committed.clear();
    }

    /// Committed 문자열만 비웁니다 (preedit 유지).
    #[inline]
    pub fn clear_committed(&mut self) {
        self.committed.clear();
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
}
