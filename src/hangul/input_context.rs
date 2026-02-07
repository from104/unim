//! 한글 입력 컨텍스트 관리 모듈
//!
//! 키보드 레이아웃, 현재 조합 상태, preedit/committed 문자열 등을 관리합니다.

use crate::hangul::composer::HangulComposer;
use crate::hangul::composer_with_2bul::HangulComposer2Bul;
use crate::hangul::composer_with_3bul::HangulComposer3Bul;
use crate::hangul::jamo::JamoEnum;
use crate::unim_log;

/// 지원하는 한글 컴포저 타입
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ComposerType {
    #[default]
    TwoBul,
    ThreeBul,
    // TODO: ThreeBulFinal, ThreeBulNoShift 등 추가 가능
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

    /// 자모를 입력받아 처리합니다.
    ///
    /// # Returns
    /// 입력 처리 성공 여부
    pub fn process_jamo(&mut self, jamo: JamoEnum) -> bool {
        unim_log!("CONTEXT", "process_jamo: {:?}", jamo);
        unim_log!(
            "CONTEXT",
            "  BEFORE: preedit='{}', committed='{}', composer={:?}",
            self.preedit,
            self.committed,
            self.composer.current_korean()
        );

        if let Some(committed_char) = self.composer.add_jamo(jamo) {
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

    #[test]
    fn test_default() {
        let ctx = HangulInputContext::default();
        assert_eq!(ctx.get_composer_type(), ComposerType::TwoBul);
    }
}
