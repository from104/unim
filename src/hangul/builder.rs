use crate::hangul::char::HangulChar;
use crate::hangul::jamo::JamoEnum;
use crate::hangul::jamo::*;
/**
 * 한글 조합 취상위 클래스
 * @author "KiHyeon Seo" <from104@gmail.com>
 * @version 0.0.1
 */
// builder.rs
use std::collections::{HashMap, VecDeque};

/**
 * 한글 빌더 트레잇 (Java의 abstract class HangulBuilder에 해당)
 */
pub trait HangulBuilder {
    /**
     * 한글 자모 입력.
     * @param jamo 자모 객체
     * @return 조합이 완료되면 완성된 한글 객체를 넘겨주고 조합중이면 None을 넘겨줌.
     */
    fn add_jamo(&mut self, jamo: JamoEnum) -> Option<char>;

    /**
     * 한글 자모 삭제. 마지막으로 입력했던 자모부터 삭제 후에 다시 조합.
     * @return 삭제된 자모 객체 또는 None
     */
    fn remove_jamo(&mut self) -> Option<JamoEnum>;

    /**
     * 스택에 쌓인 자모들로 한글 조합
     * @return 조합 성공 여부
     */
    fn build_hangul(&mut self) -> bool;

    /**
     * 강제로 조합 완료
     * @return 완성된 한글 객체 또는 None
     */
    fn force_build_hangul(&mut self) -> Option<char>;

    /**
     * 조합 중인지 여부
     * @return
     */
    fn is_build(&self) -> bool;

    /**
     * 새로운 음절이 시작되는지 확인
     * @return 새로운 음절 시작 여부
     */
    fn is_new_syllable(&self) -> bool;

    // --- 내부적으로 사용되는 함수들 (Java의 protected methods) ---

    /**
     * 한글 초성 조합 (내부 사용)
     * @return 성공 또는 실패
     */
    fn build_cho(&mut self) -> bool;

    /**
     * 한글 중성 조합 (내부 사용)
     * @return 성공 또는 실패
     */
    fn build_jung(&mut self) -> bool;

    /**
     * 한글 종성 조합 (내부 사용)
     * @return 성공 또는 실패
     */
    fn build_jong(&mut self) -> bool;

    /**
     * 자모 모두 지우기 (HangulChar의 clear() 와 유사, 필요에 따라 트레잇에 추가하거나 구현체에서 제공)
     */
    fn clear_jamo(&mut self);

    /**
     * 현재 조합된 초성 얻기 (HangulChar의 get_cho() 와 유사)
     */
    fn get_current_cho(&self) -> Option<Cho>;

    /**
     * 현재 조합된 중성 얻기 (HangulChar의 get_jung() 와 유사)
     */
    fn get_current_jung(&self) -> Option<Jung>;

    /**
     * 현재 조합된 종성 얻기 (HangulChar의 get_jong() 와 유사)
     */
    fn get_current_jong(&self) -> Option<Jong>;

    /**
     * 초성 설정 (HangulChar의 set_cho_object() 와 유사)
     */
    fn set_current_cho(&mut self, cho: Option<Cho>) -> bool;

    /**
     * 중성 설정 (HangulChar의 set_jung_object() 와 유사)
     */
    fn set_current_jung(&mut self, jung: Option<Jung>) -> bool;

    /**
     * 종성 설정 (HangulChar의 set_jong_object() 와 유사)
     */
    fn set_current_jong(&mut self, jong: Option<Jong>) -> bool;

    /**
     * 자모 조합 테이블 접근 (필요한 경우)
     */
    fn get_combined_jamo(&self) -> &HashMap<JamoEnum, HashMap<JamoEnum, JamoEnum>>;

    // 자모가 입력되는 순서대로 저장하는 큐
    fn jamo_queue(&mut self) -> &mut VecDeque<JamoEnum>;

    // 직전 큐
    fn last_jamo_queue(&mut self) -> &mut VecDeque<JamoEnum>;

    // 자모 조합 테이블
    fn combined_jamo(&mut self) -> &mut HashMap<JamoEnum, HashMap<JamoEnum, JamoEnum>>;

    // 현재 조합 중인 한글
    fn current_hangul(&mut self) -> &mut HangulChar;
}

/**
 * HangulBuilder 트레잇의 기본 구현을 제공하는 구조체
 */
#[derive(Debug, Default)]
pub struct BaseHangulBuilder {
    m_jamo_queue: VecDeque<JamoEnum>,
    m_last_jamo_queue: VecDeque<JamoEnum>,
    combined_jamo: HashMap<JamoEnum, HashMap<JamoEnum, JamoEnum>>,
    current_hangul_char: HangulChar,
}

impl BaseHangulBuilder {
    pub fn new() -> Self {
        BaseHangulBuilder::default()
    }

    /// 입력된 문자열을 한글로 변환합니다.
    /// 키보드 입력에 해당하는 문자들을 한글로 조합하여 반환합니다.
    pub fn convert_string<T: HangulBuilder>(
        input: &str,
        keyboard_map: &HashMap<char, JamoEnum>,
        builder: &mut T,
    ) -> String {
        let mut result = String::new();

        for c in input.chars() {
            // 키보드 맵에서 자모를 찾음
            if let Some(jamo) = keyboard_map.get(&c) {
                // 자모를 추가하고 조합이 완료되면 출력
                if let Some(completed_char) = builder.add_jamo(*jamo) {
                    result.push(completed_char);
                }
            } else {
                // 한글이 아닌 문자가 입력되었을 때 현재 조합 중인 글자가 있다면 출력
                if builder.is_build() {
                    if let Some(hangul_char) = builder.force_build_hangul() {
                        result.push(hangul_char);
                    }
                }
                result.push(c);
            }
        }

        // 마지막으로 조합 중이던 글자가 있다면 출력
        if builder.is_build() {
            if let Some(hangul_char) = builder.force_build_hangul() {
                result.push(hangul_char);
            }
        }

        result
    }

    pub fn is_new_syllable_internal(&self) -> bool {
        self.m_jamo_queue
            .back().is_some_and(|last_jamo| matches!(last_jamo, JamoEnum::Cho(_) if self.current_hangul_char.is_filled_jung()))
    }

    pub fn jamo_queue(&mut self) -> &mut VecDeque<JamoEnum> {
        &mut self.m_jamo_queue
    }

    pub fn get_cho(&self) -> Option<Cho> {
        self.current_hangul_char.get_cho()
    }

    pub fn get_jung(&self) -> Option<Jung> {
        self.current_hangul_char.get_jung()
    }

    pub fn get_jong(&self) -> Option<Jong> {
        self.current_hangul_char.get_jong()
    }

    pub fn set_cho(&mut self, cho: Option<Cho>) {
        self.current_hangul_char.set_cho_object(cho);
    }

    pub fn set_jung(&mut self, jung: Option<Jung>) {
        self.current_hangul_char.set_jung_object(jung);
    }

    pub fn set_jong(&mut self, jong: Option<Jong>) {
        self.current_hangul_char.set_jong_object(jong);
    }

    pub fn clear_cho(&mut self) {
        self.current_hangul_char.clear_cho();
    }

    pub fn clear_jung(&mut self) {
        self.current_hangul_char.clear_jung();
    }

    pub fn clear_jong(&mut self) {
        self.current_hangul_char.clear_jong();
    }

    pub fn clear(&mut self) {
        self.current_hangul_char.clear();
    }

    pub fn is_filled_cho(&self) -> bool {
        self.current_hangul_char.is_filled_cho()
    }

    pub fn is_filled_jung(&self) -> bool {
        self.current_hangul_char.is_filled_jung()
    }

    pub fn is_filled_jong(&self) -> bool {
        self.current_hangul_char.is_filled_jong()
    }

    fn build_cho(&mut self) -> bool {
        let mut cho_vec = Vec::new();

        // 초성만 걸러냄
        for jamo in &self.m_jamo_queue {
            if let JamoEnum::Cho(cho) = jamo {
                cho_vec.push(*cho);
            }
        }

        if cho_vec.is_empty() {
            self.clear_cho();
        } else {
            self.set_cho(Some(cho_vec[0]));
            if cho_vec.len() > 1 {
                cho_vec.remove(0);
                for cho in cho_vec {
                    let first_jamo = JamoEnum::Cho(self.get_cho().unwrap());
                    let second_jamo = JamoEnum::Cho(cho);

                    if let Some(combined_map) = self.combined_jamo.get(&first_jamo) {
                        if let Some(JamoEnum::Cho(combined_cho)) = combined_map.get(&second_jamo) {
                            self.set_cho(Some(*combined_cho));
                        } else {
                            return false;
                        }
                    } else {
                        return false;
                    }
                }
            }
        }
        true
    }

    fn build_jung(&mut self) -> bool {
        let mut jung_vec = Vec::new();

        // 중성만 걸러냄
        for jamo in &self.m_jamo_queue {
            if let JamoEnum::Jung(jung) = jamo {
                jung_vec.push(*jung);
            }
        }

        if jung_vec.is_empty() {
            self.clear_jung();
        } else {
            self.set_jung(Some(jung_vec[0]));
            if jung_vec.len() > 1 {
                jung_vec.remove(0);
                for jung in jung_vec {
                    let first_jamo = JamoEnum::Jung(self.get_jung().unwrap());
                    let second_jamo = JamoEnum::Jung(jung);

                    if let Some(combined_map) = self.combined_jamo.get(&first_jamo) {
                        if let Some(JamoEnum::Jung(combined_jung)) = combined_map.get(&second_jamo)
                        {
                            self.set_jung(Some(*combined_jung));
                        } else {
                            return false;
                        }
                    } else {
                        return false;
                    }
                }
            }
        }
        true
    }

    fn build_jong(&mut self) -> bool {
        let mut jong_vec = Vec::new();

        // 종성만 걸러냄
        for jamo in &self.m_jamo_queue {
            if let JamoEnum::Jong(jong) = jamo {
                jong_vec.push(*jong);
            }
        }

        if jong_vec.is_empty() {
            self.clear_jong();
        } else {
            self.set_jong(Some(jong_vec[0]));
            if jong_vec.len() > 1 {
                jong_vec.remove(0);
                for jong in jong_vec {
                    let first_jamo = JamoEnum::Jong(self.get_jong().unwrap());
                    let second_jamo = JamoEnum::Jong(jong);

                    if let Some(combined_map) = self.combined_jamo.get(&first_jamo) {
                        if let Some(JamoEnum::Jong(combined_jong)) = combined_map.get(&second_jamo)
                        {
                            self.set_jong(Some(*combined_jong));
                        } else {
                            return false;
                        }
                    } else {
                        return false;
                    }
                }
            }
        }
        true
    }
}

impl HangulBuilder for BaseHangulBuilder {
    fn add_jamo(&mut self, jamo: JamoEnum) -> Option<char> {
        self.m_jamo_queue.push_back(jamo);
        if !self.build_hangul() {
            self.m_jamo_queue.pop_back();
            self.build_hangul();
            let complete_hangul = self.current_hangul_char.get_syllable();
            self.m_last_jamo_queue.clear();
            self.m_last_jamo_queue.extend(&self.m_jamo_queue);
            self.m_jamo_queue.clear();
            self.m_jamo_queue.push_back(jamo);
            self.clear();
            self.build_hangul();
            Some(complete_hangul)
        } else {
            None
        }
    }

    fn remove_jamo(&mut self) -> Option<JamoEnum> {
        if self.m_jamo_queue.is_empty() {
            None
        } else {
            let jamo = self.m_jamo_queue.pop_back();
            self.build_hangul();
            jamo
        }
    }

    fn build_hangul(&mut self) -> bool {
        if self.m_jamo_queue.is_empty() {
            self.clear();
            return true;
        }

        if !self.build_cho() || !self.build_jung() || !self.build_jong() {
            return false;
        }

        true
    }

    fn force_build_hangul(&mut self) -> Option<char> {
        if self.is_build() {
            self.build_hangul();
            let complete_hangul = self.current_hangul_char.get_syllable();
            self.clear();
            self.m_jamo_queue.clear();
            self.m_last_jamo_queue.clear();
            Some(complete_hangul)
        } else {
            None
        }
    }

    fn is_build(&self) -> bool {
        !self.m_jamo_queue.is_empty()
    }

    fn is_new_syllable(&self) -> bool {
        self.is_new_syllable_internal()
    }

    // --- 내부 조합 함수 구현 ---

    fn build_cho(&mut self) -> bool {
        let cho_phonemes: Vec<Cho> = self
            .m_jamo_queue
            .iter()
            .filter_map(|p| {
                if let JamoEnum::Cho(c) = p {
                    Some(*c)
                } else {
                    None
                }
            })
            .collect();

        if cho_phonemes.is_empty() {
            self.current_hangul_char.clear_cho();
        } else {
            let mut cho = cho_phonemes[0];
            for next_cho in cho_phonemes.iter().skip(1) {
                if let Some(combined_map) = self.combined_jamo.get(&JamoEnum::Cho(cho)) {
                    if let Some(JamoEnum::Cho(new_cho)) =
                        combined_map.get(&JamoEnum::Cho(*next_cho))
                    {
                        cho = *new_cho;
                    } else {
                        return false;
                    }
                } else {
                    return false;
                }
            }
            self.current_hangul_char.set_cho_object(Some(cho));
        }
        true
    }

    fn build_jung(&mut self) -> bool {
        let jung_phonemes: Vec<Jung> = self
            .m_jamo_queue
            .iter()
            .filter_map(|p| {
                if let JamoEnum::Jung(j) = p {
                    Some(*j)
                } else {
                    None
                }
            })
            .collect();

        if jung_phonemes.is_empty() {
            self.current_hangul_char.clear_jung();
        } else {
            let mut jung = jung_phonemes[0];
            for next_jung in jung_phonemes.iter().skip(1) {
                if let Some(combined_map) = self.combined_jamo.get(&JamoEnum::Jung(jung)) {
                    if let Some(JamoEnum::Jung(new_jung)) =
                        combined_map.get(&JamoEnum::Jung(*next_jung))
                    {
                        jung = *new_jung;
                    } else {
                        return false;
                    }
                } else {
                    return false;
                }
            }
            self.current_hangul_char.set_jung_object(Some(jung));
        }
        true
    }

    fn build_jong(&mut self) -> bool {
        let jong_phonemes: Vec<Jong> = self
            .m_jamo_queue
            .iter()
            .filter_map(|p| {
                if let JamoEnum::Jong(j) = p {
                    Some(*j)
                } else {
                    None
                }
            })
            .collect();

        if jong_phonemes.is_empty() {
            self.current_hangul_char.clear_jong();
        } else {
            let mut jong = jong_phonemes[0];
            for next_jong in jong_phonemes.iter().skip(1) {
                if let Some(combined_map) = self.combined_jamo.get(&JamoEnum::Jong(jong)) {
                    if let Some(JamoEnum::Jong(new_jong)) =
                        combined_map.get(&JamoEnum::Jong(*next_jong))
                    {
                        jong = *new_jong;
                    } else {
                        return false;
                    }
                } else {
                    return false;
                }
            }
            self.current_hangul_char.set_jong_object(Some(jong));
        }
        true
    }

    fn clear_jamo(&mut self) {
        self.current_hangul_char.clear();
    }

    fn get_current_cho(&self) -> Option<Cho> {
        self.current_hangul_char.get_cho()
    }

    fn get_current_jung(&self) -> Option<Jung> {
        self.current_hangul_char.get_jung()
    }

    fn get_current_jong(&self) -> Option<Jong> {
        self.current_hangul_char.get_jong()
    }

    fn set_current_cho(&mut self, cho: Option<Cho>) -> bool {
        self.current_hangul_char.set_cho_object(cho)
    }

    fn set_current_jung(&mut self, jung: Option<Jung>) -> bool {
        self.current_hangul_char.set_jung_object(jung)
    }

    fn set_current_jong(&mut self, jong: Option<Jong>) -> bool {
        self.current_hangul_char.set_jong_object(jong)
    }

    fn get_combined_jamo(&self) -> &HashMap<JamoEnum, HashMap<JamoEnum, JamoEnum>> {
        &self.combined_jamo
    }

    fn jamo_queue(&mut self) -> &mut VecDeque<JamoEnum> {
        &mut self.m_jamo_queue
    }

    fn last_jamo_queue(&mut self) -> &mut VecDeque<JamoEnum> {
        &mut self.m_last_jamo_queue
    }

    fn combined_jamo(&mut self) -> &mut HashMap<JamoEnum, HashMap<JamoEnum, JamoEnum>> {
        &mut self.combined_jamo
    }

    fn current_hangul(&mut self) -> &mut HangulChar {
        &mut self.current_hangul_char
    }
}
