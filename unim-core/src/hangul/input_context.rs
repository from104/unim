use crate::hangul::composer::HangulComposer;
use crate::hangul::composer_with_2bul::HangulComposer2Bul;
use crate::hangul::composer_with_3bul::HangulComposer3Bul; // TODO: Add 3bul composer if needed
use crate::hangul::jamo::{Jamo, JamoEnum}; // Import Jamo trait

/// 한글 입력 과정을 관리하는 컨텍스트입니다.
///
/// 키보드 레이아웃, 현재 조합 중인 컴포저 인스턴스,
/// 조합 전/후 텍스트 상태 등을 관리합니다.
pub struct HangulInputContext {
    /// 현재 사용 중인 한글 컴포저 (`HangulComposer` 트레이트 객체).
    /// 두벌식, 세벌식 등 다양한 컴포저 구현체를 담을 수 있습니다.
    composer: Box<dyn HangulComposer>,
    /// 현재 조합 중인 문자열 (Preedit). 사용자에게 보여지는 중간 결과입니다.
    preedit_string: String,
    /// 확정된(Commit) 문자열. 입력이 완료된 결과입니다.
    committed_string: String,
    // TODO: Add keyboard layout management if needed within this context
}

/// 지원하는 한글 컴포저 타입을 나타내는 열거형.
pub enum ComposerType {
    TwoBul,
    ThreeBul, // TODO: Add other composer types like ThreeBulFinal, etc.
}

impl HangulInputContext {
    /// 새로운 `HangulInputContext`를 생성합니다.
    ///
    /// # Arguments
    ///
    /// * `composer_type` - 사용할 한글 컴포저의 타입 (`ComposerType`).
    ///
    /// # Returns
    ///
    /// 초기화된 `HangulInputContext` 인스턴스.
    pub fn new(composer_type: ComposerType) -> Self {
        let composer: Box<dyn HangulComposer> = match composer_type {
            ComposerType::TwoBul => Box::new(HangulComposer2Bul::new()),
            ComposerType::ThreeBul => Box::new(HangulComposer3Bul::new()), // Ensure HangulComposer3Bul implements HangulComposer and has a new()
        };

        HangulInputContext {
            composer,
            preedit_string: String::new(),
            committed_string: String::new(),
        }
    }

    /// 자모를 입력받아 처리하고, 조합 상태 및 문자열을 업데이트합니다.
    ///
    /// # Arguments
    ///
    /// * `jamo` - 입력할 한글 자모 (`JamoEnum`).
    ///
    /// # Returns
    ///
    /// * `bool` - 입력 처리 성공 여부. 현재는 항상 true를 반환하지만,
    ///            향후 유효성 검사 등을 추가할 수 있습니다.
    pub fn process_jamo(&mut self, jamo: JamoEnum) -> bool {
        if let Some(committed_char) = self.composer.add_jamo(jamo) {
            // 이전 글자가 완성되어 commit됨
            self.committed_string.push(committed_char);
            self.update_preedit(); // 새 글자 조합 시작으로 preedit 업데이트
        } else {
            // 조합 계속 진행
            self.update_preedit();
        }
        true // Indicate success, could add validation later
    }

    /// 마지막 입력된 자모를 제거하고, 조합 상태 및 문자열을 업데이트합니다. (Backspace 기능)
    ///
    /// # Returns
    ///
    /// * `bool` - 제거 성공 여부.
    pub fn backspace(&mut self) -> bool {
        if self.composer.is_compose() {
            if self.composer.remove_jamo().is_some() {
                self.update_preedit();
                true
            } else {
                // 조합 중이지만 제거할 자모가 없는 이상한 상태 (이론상으론 발생 안 함)
                self.clear_preedit(); // 안전하게 preedit 초기화
                false
            }
        } else {
            // 조합 중이 아닐 때, 확정된 문자열에서 마지막 문자 제거 시도
            if self.committed_string.pop().is_some() {
                // Preedit은 이미 비어있으므로 업데이트 필요 없음
                true
            } else {
                false // 제거할 문자 없음
            }
        }
    }

    /// 현재 조합 중인 문자열(preedit)을 반환합니다.
    pub fn get_preedit(&self) -> String {
        self.preedit_string.clone()
    }

    /// 현재 확정된 문자열(committed)을 반환합니다.
    pub fn get_committed(&self) -> &str {
        &self.committed_string
    }

    /// 현재 조합 중인 내용을 강제로 확정(commit)하고 preedit을 비웁니다.
    ///
    /// # Returns
    ///
    /// * `Option<char>` - 강제 조합으로 확정된 문자 (있을 경우).
    pub fn commit(&mut self) -> Option<char> {
        let composed_char = self.composer.force_compose_hangul();
        if let Some(c) = composed_char {
            self.committed_string.push(c);
        }
        self.clear_preedit(); // Preedit 비우기
        composed_char
    }

    /// 비-한글 문자를 확정 문자열에 추가합니다.
    /// 이 메서드는 현재 조합 상태를 변경하지 않습니다. (commit()은 별도 호출 필요)
    ///
    /// # Arguments
    ///
    /// * `c` - 추가할 문자.
    pub fn commit_char(&mut self, c: char) {
        self.committed_string.push(c);
    }

    /// `HangulComposer`의 현재 상태를 기반으로 preedit 문자열을 업데이트합니다.
    fn update_preedit(&mut self) {
        // Use get_syllable() which returns Result<char, HangulError>
        if let Ok(composed_char) = self.composer.current_hangul().get_syllable() {
            self.preedit_string = composed_char.to_string();
        } else {
            // 조합된 글자가 없는 경우 (예: 초성만 입력)
            // Use get_unicode_compat() to get the character representation
            if let Some(first_jamo) = self.composer.jamo_queue().front() {
                // 첫 자모만이라도 표시 (예: 'ㄱ')
                self.preedit_string = first_jamo.get_unicode_compat().to_string();
            } else {
                self.preedit_string.clear();
            }
        }
    }

    /// Preedit 문자열을 비웁니다.
    fn clear_preedit(&mut self) {
        self.preedit_string.clear();
        // Composer의 상태도 초기화해야 할 수 있음 (상황에 따라)
        // 예: self.composer.clear() 또는 force_compose_hangul() 호출
    }

    /// 입력 컨텍스트의 상태를 초기화합니다. (Preedit, Committed 문자열 비우기)
    /// Composer 자체의 리셋은 포함하지 않을 수 있습니다 (선택 사항).
    pub fn clear(&mut self) {
        self.composer.force_compose_hangul(); // 남아있는 조합 강제 완료 시도
        self.clear_preedit();
        self.committed_string.clear();
    }

    /// 현재 조합 중인지 여부를 반환합니다.
    pub fn is_composing(&self) -> bool {
        self.composer.is_compose() || !self.preedit_string.is_empty()
    }

    pub fn append_to_committed(&mut self, c: char) {
        self.commit();
        self.committed_string.push(c);
    }
}

// --- 유닛 테스트 ---
#[cfg(test)]
mod tests {
    use super::*;
    use crate::hangul::jamo::*;

    #[test]
    fn test_context_basic_composition() {
        let mut context = HangulInputContext::new(ComposerType::TwoBul);

        // ㄱ 입력
        context.process_jamo(JamoEnum::Cho(Cho::G));
        assert!(context.is_composing());
        assert_eq!(context.get_preedit(), "ㄱ");
        assert_eq!(context.get_committed(), "");

        // ㅏ 입력
        context.process_jamo(JamoEnum::Jung(Jung::A));
        assert!(context.is_composing());
        assert_eq!(context.get_preedit(), "가");
        assert_eq!(context.get_committed(), "");

        // ㄱ 입력 (종성)
        context.process_jamo(JamoEnum::Cho(Cho::G));
        assert!(context.is_composing());
        assert_eq!(context.get_preedit(), "각");
        assert_eq!(context.get_committed(), "");
    }

    #[test]
    fn test_context_commit() {
        let mut context = HangulInputContext::new(ComposerType::TwoBul);
        context.process_jamo(JamoEnum::Cho(Cho::G));
        context.process_jamo(JamoEnum::Jung(Jung::A));
        context.process_jamo(JamoEnum::Cho(Cho::G)); // 각

        let committed = context.commit(); // 강제 커밋
        assert_eq!(committed, Some('각'));
        assert!(!context.is_composing());
        assert_eq!(context.get_preedit(), "");
        assert_eq!(context.get_committed(), "각");

        // 새 글자 시작
        context.process_jamo(JamoEnum::Cho(Cho::N));
        assert!(context.is_composing());
        assert_eq!(context.get_preedit(), "ㄴ");
        assert_eq!(context.get_committed(), "각");
    }

    #[test]
    fn test_context_backspace() {
        let mut context = HangulInputContext::new(ComposerType::TwoBul);
        context.process_jamo(JamoEnum::Cho(Cho::G)); // ㄱ
        context.process_jamo(JamoEnum::Jung(Jung::A)); // 가
        context.process_jamo(JamoEnum::Cho(Cho::G)); // 각

        assert_eq!(context.get_preedit(), "각");
        assert!(context.backspace()); // 각 -> 가
        assert_eq!(context.get_preedit(), "가");
        assert!(context.backspace()); // 가 -> ㄱ
        assert_eq!(context.get_preedit(), "ㄱ");
        assert!(context.backspace()); // ㄱ -> ""
        assert_eq!(context.get_preedit(), "");
        assert!(!context.is_composing());

        // 빈 상태에서 backspace
        assert!(!context.backspace());

        // Committed된 내용 삭제
        context.process_jamo(JamoEnum::Cho(Cho::H));
        context.process_jamo(JamoEnum::Jung(Jung::A)); // 하
        context.commit(); // "하" 확정
        assert_eq!(context.get_committed(), "하");

        context.process_jamo(JamoEnum::Cho(Cho::N)); // ㄴ 입력 시작
        assert_eq!(context.get_preedit(), "ㄴ");
        assert!(context.is_composing());

        assert!(context.backspace()); // ㄴ 삭제
        assert!(!context.is_composing());
        assert_eq!(context.get_preedit(), "");
        assert_eq!(context.get_committed(), "하"); // Committed는 유지

        assert!(context.backspace()); // "하" 삭제
        assert_eq!(context.get_committed(), "");
        assert!(!context.backspace()); // 더 이상 삭제할 것 없음
    }

    #[test]
    fn test_context_dokkaebi() {
        let mut context = HangulInputContext::new(ComposerType::TwoBul);
        context.process_jamo(JamoEnum::Cho(Cho::G));
        context.process_jamo(JamoEnum::Jung(Jung::A));
        context.process_jamo(JamoEnum::Cho(Cho::G)); // 각
        assert_eq!(context.get_preedit(), "각");
        assert_eq!(context.get_committed(), "");

        // 도깨비불 발생: '각' + 'ㅏ' -> '가', 'ㄱㅏ'
        context.process_jamo(JamoEnum::Jung(Jung::A));
        assert!(context.is_composing());
        assert_eq!(context.get_preedit(), "가"); // 새 preedit '가'
        assert_eq!(context.get_committed(), "가"); // 이전 '가'가 commit됨
    }

    #[test]
    fn test_context_clear() {
        let mut context = HangulInputContext::new(ComposerType::TwoBul);
        context.process_jamo(JamoEnum::Cho(Cho::G));
        context.process_jamo(JamoEnum::Jung(Jung::A));
        context.process_jamo(JamoEnum::Cho(Cho::G)); // 각

        context.commit(); // "각" 확정
        context.process_jamo(JamoEnum::Cho(Cho::N)); // ㄴ preedit
        assert_eq!(context.get_committed(), "각");
        assert_eq!(context.get_preedit(), "ㄴ");

        context.clear();
        assert_eq!(context.get_committed(), "");
        assert_eq!(context.get_preedit(), "");
        assert!(!context.is_composing());
    }

    // TODO: 3벌식 테스트 추가 (HangulComposer3Bul 구현 후)
    #[test]
    #[ignore] // Ignore until HangulComposer3Bul is ready and imported
    fn test_context_3bul_basic() {
        let mut context = HangulInputContext::new(ComposerType::ThreeBul);
        // Add 3bul specific tests here
        context.process_jamo(JamoEnum::Cho(Cho::G));
        assert_eq!(context.get_preedit(), "ㄱ");
    }
}
