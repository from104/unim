use crate::hangul::char::HangulChar;
use crate::hangul::jamo::PhonemeEnum;
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
    fn add_jamo(&mut self, jamo: PhonemeEnum) -> Option<char>;

    /**
     * 한글 자모 삭제. 마지막으로 입력했던 자모부터 삭제 후에 다시 조합.
     * @return 삭제된 자모 객체 또는 None
     */
    fn remove_jamo(&mut self) -> Option<PhonemeEnum>;

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
    fn get_combined_jamo(&self) -> &HashMap<PhonemeEnum, HashMap<PhonemeEnum, PhonemeEnum>>;

    // 자모가 입력되는 순서대로 저장하는 큐
    fn jamo_queue(&mut self) -> &mut VecDeque<PhonemeEnum>;

    // 직전 큐
    fn last_jamo_queue(&mut self) -> &mut VecDeque<PhonemeEnum>;

    // 자모 조합 테이블
    fn combined_jamo(&mut self) -> &mut HashMap<PhonemeEnum, HashMap<PhonemeEnum, PhonemeEnum>>;

    // 현재 조합 중인 한글
    fn current_hangul(&mut self) -> &mut HangulChar;
}

/**
 * HangulBuilder 트레잇의 기본 구현을 제공하는 구조체
 */
#[derive(Debug, Default)]
pub struct BaseHangulBuilder {
    m_jamo_queue: VecDeque<PhonemeEnum>,
    m_last_jamo_queue: VecDeque<PhonemeEnum>,
    combined_jamo: HashMap<PhonemeEnum, HashMap<PhonemeEnum, PhonemeEnum>>,
    current_hangul_char: HangulChar,
}

impl BaseHangulBuilder {
    pub fn new() -> Self {
        BaseHangulBuilder::default()
    }

    pub fn is_new_syllable_internal(&self) -> bool {
        self.m_jamo_queue
            .back()
            .map_or(false, |last_jamo| matches!(last_jamo, PhonemeEnum::Cho(_) if self.current_hangul_char.is_filled_jung()))
    }

    pub fn peek_jamo_queue(&self) -> &VecDeque<PhonemeEnum> {
        &self.m_jamo_queue
    }
}

impl HangulBuilder for BaseHangulBuilder {
    fn add_jamo(&mut self, jamo: PhonemeEnum) -> Option<char> {
        let mut completed_char = None;

        // 초성이 들어왔고 이전에 중성이 있었다면 이전 글자를 완성
        if matches!(jamo, PhonemeEnum::Cho(_))
            && self.current_hangul_char.is_filled_jung()
            && self.build_hangul()
        {
            completed_char = Some(self.current_hangul_char.get_syllable());
            self.current_hangul_char.clear();
            self.m_jamo_queue.clear();
        }

        // 새로운 자모 추가
        self.m_jamo_queue.push_back(jamo);
        if !self.build_hangul() {
            self.m_jamo_queue.pop_back();
            self.build_hangul();
        }

        completed_char
    }

    fn remove_jamo(&mut self) -> Option<PhonemeEnum> {
        if self.m_jamo_queue.is_empty() {
            None
        } else {
            let jamo = self.m_jamo_queue.pop_back().unwrap();
            self.build_hangul();
            Some(jamo)
        }
    }

    fn build_hangul(&mut self) -> bool {
        if self.m_jamo_queue.is_empty() {
            self.current_hangul_char.clear();
            return true;
        }

        let last_jamo = *self.m_jamo_queue.back().unwrap();
        let last_prev_jamo = if self.m_jamo_queue.len() > 1 {
            Some(*self.m_jamo_queue.get(self.m_jamo_queue.len() - 2).unwrap())
        } else {
            None
        };

        // 중성 다음으로 초성이 들어오면 종성으로
        if self.current_hangul_char.is_filled_jung() {
            if let PhonemeEnum::Cho(cho) = last_jamo {
                match cho.to_jong() {
                    Ok(jong) => {
                        self.m_jamo_queue.pop_back();
                        self.m_jamo_queue.push_back(PhonemeEnum::Jong(jong));
                    }
                    Err(_) => return false,
                }
            }
        }

        // 초성이 없고 중성 다음에 종성이 오면
        if !self.current_hangul_char.is_filled_cho()
            && last_prev_jamo.map_or(false, |j| matches!(j, PhonemeEnum::Jung(_)))
            && matches!(last_jamo, PhonemeEnum::Jong(_))
        {
            return false;
        }

        // 종성 다음에 중성이 오면
        if last_prev_jamo.map_or(false, |j| matches!(j, PhonemeEnum::Jong(_)))
            && matches!(last_jamo, PhonemeEnum::Jung(_))
        {
            return false;
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
            self.current_hangul_char.clear();
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
                if let PhonemeEnum::Cho(c) = p {
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
                if let Some(combined_map) = self.combined_jamo.get(&PhonemeEnum::Cho(cho)) {
                    if let Some(PhonemeEnum::Cho(new_cho)) =
                        combined_map.get(&PhonemeEnum::Cho(*next_cho))
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
                if let PhonemeEnum::Jung(j) = p {
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
                if let Some(combined_map) = self.combined_jamo.get(&PhonemeEnum::Jung(jung)) {
                    if let Some(PhonemeEnum::Jung(new_jung)) =
                        combined_map.get(&PhonemeEnum::Jung(*next_jung))
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
                if let PhonemeEnum::Jong(j) = p {
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
                if let Some(combined_map) = self.combined_jamo.get(&PhonemeEnum::Jong(jong)) {
                    if let Some(PhonemeEnum::Jong(new_jong)) =
                        combined_map.get(&PhonemeEnum::Jong(*next_jong))
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

    fn get_combined_jamo(&self) -> &HashMap<PhonemeEnum, HashMap<PhonemeEnum, PhonemeEnum>> {
        &self.combined_jamo
    }

    fn jamo_queue(&mut self) -> &mut VecDeque<PhonemeEnum> {
        &mut self.m_jamo_queue
    }

    fn last_jamo_queue(&mut self) -> &mut VecDeque<PhonemeEnum> {
        &mut self.m_last_jamo_queue
    }

    fn combined_jamo(&mut self) -> &mut HashMap<PhonemeEnum, HashMap<PhonemeEnum, PhonemeEnum>> {
        &mut self.combined_jamo
    }

    fn current_hangul(&mut self) -> &mut HangulChar {
        &mut self.current_hangul_char
    }
}
