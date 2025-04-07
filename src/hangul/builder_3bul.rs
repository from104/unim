use crate::hangul::builder::*;
use crate::hangul::char::HangulChar;
use crate::hangul::jamo::{Cho, JamoEnum, Jong, Jung};
use std::collections::{HashMap, VecDeque};

/**
 * 3벌식 한글 조합기
 */
#[derive(Debug, Default)]
pub struct HangulBuilder3Bul {
    base_builder: BaseHangulBuilder,
}

impl HangulBuilder3Bul {
    pub fn new() -> Self {
        let mut builder = HangulBuilder3Bul {
            base_builder: BaseHangulBuilder::new(),
        };
        builder.initialize_combined_jamo();
        builder
    }

    /// 3벌식 키보드 매핑을 생성합니다.
    fn create_keyboard_map(&self) -> HashMap<char, JamoEnum> {
        let mut keyboard_map = HashMap::new();

        // 숫자 키 매핑
        keyboard_map.insert('1', JamoEnum::Jong(Jong::H));
        keyboard_map.insert('2', JamoEnum::Jong(Jong::SS));
        keyboard_map.insert('3', JamoEnum::Jong(Jong::B));
        keyboard_map.insert('4', JamoEnum::Jung(Jung::YO));
        keyboard_map.insert('5', JamoEnum::Jung(Jung::YU));
        keyboard_map.insert('6', JamoEnum::Jung(Jung::YA));
        keyboard_map.insert('7', JamoEnum::Jung(Jung::YE));
        keyboard_map.insert('8', JamoEnum::Jung(Jung::YI));
        keyboard_map.insert('9', JamoEnum::Jung(Jung::U));
        keyboard_map.insert('0', JamoEnum::Cho(Cho::K));

        // 첫번째 행
        keyboard_map.insert('q', JamoEnum::Jong(Jong::S));
        keyboard_map.insert('w', JamoEnum::Jong(Jong::L));
        keyboard_map.insert('e', JamoEnum::Jung(Jung::YEO));
        keyboard_map.insert('r', JamoEnum::Jung(Jung::AE));
        keyboard_map.insert('t', JamoEnum::Jung(Jung::EO));
        keyboard_map.insert('y', JamoEnum::Cho(Cho::R));
        keyboard_map.insert('u', JamoEnum::Cho(Cho::D));
        keyboard_map.insert('i', JamoEnum::Cho(Cho::M));
        keyboard_map.insert('o', JamoEnum::Cho(Cho::C));
        keyboard_map.insert('p', JamoEnum::Cho(Cho::P));

        // 두번째 행
        keyboard_map.insert('a', JamoEnum::Jong(Jong::NG));
        keyboard_map.insert('s', JamoEnum::Jong(Jong::N));
        keyboard_map.insert('d', JamoEnum::Jung(Jung::I));
        keyboard_map.insert('f', JamoEnum::Jung(Jung::A));
        keyboard_map.insert('g', JamoEnum::Jung(Jung::EU));
        keyboard_map.insert('h', JamoEnum::Cho(Cho::N));
        keyboard_map.insert('j', JamoEnum::Cho(Cho::E));
        keyboard_map.insert('k', JamoEnum::Cho(Cho::G));
        keyboard_map.insert('l', JamoEnum::Cho(Cho::J));
        keyboard_map.insert(';', JamoEnum::Cho(Cho::B));
        keyboard_map.insert('\'', JamoEnum::Cho(Cho::T));

        // 세번째 행
        keyboard_map.insert('z', JamoEnum::Jong(Jong::M));
        keyboard_map.insert('x', JamoEnum::Jong(Jong::G));
        keyboard_map.insert('c', JamoEnum::Jung(Jung::E));
        keyboard_map.insert('v', JamoEnum::Jung(Jung::O));
        keyboard_map.insert('b', JamoEnum::Jung(Jung::U));
        keyboard_map.insert('n', JamoEnum::Cho(Cho::S));
        keyboard_map.insert('m', JamoEnum::Cho(Cho::H));

        keyboard_map
    }

    fn initialize_combined_jamo(&mut self) {
        let mut combined_jamo = HashMap::new();

        // 초성 조합 규칙
        let mut g_map = HashMap::new();
        g_map.insert(JamoEnum::Cho(Cho::G), JamoEnum::Cho(Cho::GG));
        combined_jamo.insert(JamoEnum::Cho(Cho::G), g_map);

        let mut d_map = HashMap::new();
        d_map.insert(JamoEnum::Cho(Cho::D), JamoEnum::Cho(Cho::DD));
        combined_jamo.insert(JamoEnum::Cho(Cho::D), d_map);

        let mut b_map = HashMap::new();
        b_map.insert(JamoEnum::Cho(Cho::B), JamoEnum::Cho(Cho::BB));
        combined_jamo.insert(JamoEnum::Cho(Cho::B), b_map);

        let mut s_map = HashMap::new();
        s_map.insert(JamoEnum::Cho(Cho::S), JamoEnum::Cho(Cho::SS));
        combined_jamo.insert(JamoEnum::Cho(Cho::S), s_map);

        let mut j_map = HashMap::new();
        j_map.insert(JamoEnum::Cho(Cho::J), JamoEnum::Cho(Cho::JJ));
        combined_jamo.insert(JamoEnum::Cho(Cho::J), j_map);

        // 중성 조합 규칙
        let mut o_map = HashMap::new();
        o_map.insert(JamoEnum::Jung(Jung::A), JamoEnum::Jung(Jung::WA));
        o_map.insert(JamoEnum::Jung(Jung::AE), JamoEnum::Jung(Jung::WAE));
        o_map.insert(JamoEnum::Jung(Jung::I), JamoEnum::Jung(Jung::OE));
        combined_jamo.insert(JamoEnum::Jung(Jung::O), o_map);

        let mut u_map = HashMap::new();
        u_map.insert(JamoEnum::Jung(Jung::EO), JamoEnum::Jung(Jung::WEO));
        u_map.insert(JamoEnum::Jung(Jung::E), JamoEnum::Jung(Jung::WE));
        u_map.insert(JamoEnum::Jung(Jung::I), JamoEnum::Jung(Jung::WI));
        combined_jamo.insert(JamoEnum::Jung(Jung::U), u_map);

        let mut eu_map = HashMap::new();
        eu_map.insert(JamoEnum::Jung(Jung::I), JamoEnum::Jung(Jung::YI));
        combined_jamo.insert(JamoEnum::Jung(Jung::EU), eu_map);

        // 종성 조합 규칙
        let mut jong_g_map = HashMap::new();
        jong_g_map.insert(JamoEnum::Jong(Jong::G), JamoEnum::Jong(Jong::GG));
        jong_g_map.insert(JamoEnum::Jong(Jong::S), JamoEnum::Jong(Jong::GS));
        combined_jamo.insert(JamoEnum::Jong(Jong::G), jong_g_map);

        let mut n_map = HashMap::new();
        n_map.insert(JamoEnum::Jong(Jong::J), JamoEnum::Jong(Jong::NJ));
        n_map.insert(JamoEnum::Jong(Jong::H), JamoEnum::Jong(Jong::NH));
        combined_jamo.insert(JamoEnum::Jong(Jong::N), n_map);

        let mut l_map = HashMap::new();
        l_map.insert(JamoEnum::Jong(Jong::G), JamoEnum::Jong(Jong::LG));
        l_map.insert(JamoEnum::Jong(Jong::M), JamoEnum::Jong(Jong::LM));
        l_map.insert(JamoEnum::Jong(Jong::B), JamoEnum::Jong(Jong::LB));
        l_map.insert(JamoEnum::Jong(Jong::S), JamoEnum::Jong(Jong::LS));
        l_map.insert(JamoEnum::Jong(Jong::T), JamoEnum::Jong(Jong::LT));
        l_map.insert(JamoEnum::Jong(Jong::P), JamoEnum::Jong(Jong::LP));
        l_map.insert(JamoEnum::Jong(Jong::H), JamoEnum::Jong(Jong::LH));
        combined_jamo.insert(JamoEnum::Jong(Jong::L), l_map);

        let mut jong_b_map = HashMap::new();
        jong_b_map.insert(JamoEnum::Jong(Jong::S), JamoEnum::Jong(Jong::BS));
        combined_jamo.insert(JamoEnum::Jong(Jong::B), jong_b_map);

        let mut s_jong_map = HashMap::new();
        s_jong_map.insert(JamoEnum::Jong(Jong::S), JamoEnum::Jong(Jong::SS));
        combined_jamo.insert(JamoEnum::Jong(Jong::S), s_jong_map);

        *self.base_builder.combined_jamo() = combined_jamo;
    }

    /// 문자열을 한글로 변환합니다.
    pub fn convert_string(&mut self, input: &str) -> String {
        let keyboard_map = self.create_keyboard_map();
        BaseHangulBuilder::convert_string(input, &keyboard_map, self)
    }

    /// 한글 문자열을 세벌식 영문 자판 입력으로 변환합니다.
    pub fn convert_korean_to_english(&self, input: &str) -> String {
        let keyboard_map = self.create_keyboard_map();
        let mut reverse_map = HashMap::new();

        // 키보드 맵을 뒤집어서 자모 -> 키 매핑 생성
        for (key, jamo) in keyboard_map.iter() {
            reverse_map.insert(*jamo, *key);
        }

        let mut result = String::new();

        for c in input.chars() {
            if c as u32 >= 0xAC00 && c as u32 <= 0xD7A3 {
                // 한글 완성형 문자인 경우
                let mut hangul_char = HangulChar::new();
                hangul_char.set_jamo_by_syllable(c);

                // 초성 변환
                if let Some(cho) = hangul_char.get_cho() {
                    if let Some(&key) = reverse_map.get(&JamoEnum::Cho(cho)) {
                        result.push(key);
                    }
                }

                // 중성 변환
                if let Some(jung) = hangul_char.get_jung() {
                    if let Some(&key) = reverse_map.get(&JamoEnum::Jung(jung)) {
                        result.push(key);
                    }
                }

                // 종성 변환 (세벌식은 종성이 직접 매핑됨)
                if let Some(jong) = hangul_char.get_jong() {
                    // Jong::E는 빈 종성을 의미함
                    if jong != Jong::E {
                        if let Some(&key) = reverse_map.get(&JamoEnum::Jong(jong)) {
                            result.push(key);
                        }
                    }
                }
            } else {
                // 한글이 아닌 경우 그대로 추가
                result.push(c);
            }
        }

        result
    }
}

impl HangulBuilder for HangulBuilder3Bul {
    fn add_jamo(&mut self, jamo: JamoEnum) -> Option<char> {
        let mut queue = VecDeque::new();
        queue.extend(self.base_builder.jamo_queue().iter().copied());
        queue.push_back(jamo);

        self.base_builder.jamo_queue().clear();
        self.base_builder.jamo_queue().extend(queue);

        if !self.build_hangul() {
            self.base_builder.jamo_queue().pop_back();
            self.build_hangul();
            let complete_hangul = self.base_builder.current_hangul().get_syllable();

            let current_queue: Vec<_> = self.base_builder.jamo_queue().iter().copied().collect();
            self.base_builder.last_jamo_queue().clear();
            self.base_builder.last_jamo_queue().extend(current_queue);
            self.base_builder.jamo_queue().clear();
            self.base_builder.jamo_queue().push_back(jamo);

            self.clear_jamo();
            self.build_hangul();
            Some(complete_hangul)
        } else {
            None
        }
    }

    fn remove_jamo(&mut self) -> Option<JamoEnum> {
        self.base_builder.remove_jamo()
    }

    fn build_hangul(&mut self) -> bool {
        // 큐가 비어있는지 먼저 확인
        if self.base_builder.jamo_queue().is_empty() {
            self.clear_jamo();
            return true;
        }

        // 큐의 내용을 복사하여 작업
        let queue_contents: Vec<_> = self.base_builder.jamo_queue().iter().copied().collect();
        let last_jamo = queue_contents.last().unwrap();
        let last_prev_jamo = if queue_contents.len() > 1 {
            Some(queue_contents[queue_contents.len() - 2])
        } else {
            None
        };

        // 현재 상태 확인
        let is_filled_jung = self.base_builder.current_hangul().is_filled_jung();

        // 3벌식 특수 규칙 검사
        match (last_prev_jamo, last_jamo) {
            // 초성+종성 또는 중성+종성만 있을 때 종성이 들어오면 실패
            (_, JamoEnum::Jong(_)) if !is_filled_jung => {
                return false;
            }
            // 중성이나 종성 다음에 초성이 오면 실패
            (Some(JamoEnum::Jung(_) | JamoEnum::Jong(_)), JamoEnum::Cho(_)) => return false,
            // 종성 다음에 중성이 오면 실패
            (Some(JamoEnum::Jong(_)), JamoEnum::Jung(_)) => return false,
            _ => {}
        }

        if !self.build_cho() || !self.build_jung() || !self.build_jong() {
            return false;
        }

        true
    }

    fn force_build_hangul(&mut self) -> Option<char> {
        self.base_builder.force_build_hangul()
    }

    fn is_build(&self) -> bool {
        self.base_builder.is_build()
    }

    fn is_new_syllable(&self) -> bool {
        self.base_builder.is_new_syllable()
    }

    fn build_cho(&mut self) -> bool {
        self.base_builder.build_cho()
    }

    fn build_jung(&mut self) -> bool {
        self.base_builder.build_jung()
    }

    fn build_jong(&mut self) -> bool {
        self.base_builder.build_jong()
    }

    fn clear_jamo(&mut self) {
        self.base_builder.clear_jamo()
    }

    fn get_current_cho(&self) -> Option<Cho> {
        self.base_builder.get_current_cho()
    }

    fn get_current_jung(&self) -> Option<Jung> {
        self.base_builder.get_current_jung()
    }

    fn get_current_jong(&self) -> Option<Jong> {
        self.base_builder.get_current_jong()
    }

    fn set_current_cho(&mut self, cho: Option<Cho>) -> bool {
        self.base_builder.set_current_cho(cho)
    }

    fn set_current_jung(&mut self, jung: Option<Jung>) -> bool {
        self.base_builder.set_current_jung(jung)
    }

    fn set_current_jong(&mut self, jong: Option<Jong>) -> bool {
        self.base_builder.set_current_jong(jong)
    }

    fn get_combined_jamo(&self) -> &HashMap<JamoEnum, HashMap<JamoEnum, JamoEnum>> {
        self.base_builder.get_combined_jamo()
    }

    fn jamo_queue(&mut self) -> &mut VecDeque<JamoEnum> {
        self.base_builder.jamo_queue()
    }

    fn last_jamo_queue(&mut self) -> &mut VecDeque<JamoEnum> {
        self.base_builder.last_jamo_queue()
    }

    fn combined_jamo(&mut self) -> &mut HashMap<JamoEnum, HashMap<JamoEnum, JamoEnum>> {
        self.base_builder.combined_jamo()
    }

    fn current_hangul(&mut self) -> &mut HangulChar {
        self.base_builder.current_hangul()
    }
}
