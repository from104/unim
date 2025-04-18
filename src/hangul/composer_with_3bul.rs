use crate::hangul::char::HangulChar;
use crate::hangul::composer::BaseHangulComposer;
use crate::hangul::composer::HangulComposer;
use crate::hangul::jamo::*;
use std::collections::{HashMap, VecDeque};

/**
 * 3벌식 한글 조합기
 */
#[derive(Debug, Default)]
pub struct HangulComposer3Bul {
    base_composer: BaseHangulComposer,
}

impl HangulComposer3Bul {
    pub fn new() -> Self {
        let mut composer = HangulComposer3Bul {
            base_composer: BaseHangulComposer::new(),
        };
        composer.initialize_combined_jamo();
        composer
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

        *self.base_composer.combined_jamo() = combined_jamo;
    }
}

impl HangulComposer for HangulComposer3Bul {
    fn add_jamo(&mut self, jamo: JamoEnum) -> Option<char> {
        let mut queue = VecDeque::new();
        queue.extend(self.base_composer.jamo_queue().iter().copied());
        queue.push_back(jamo);

        self.base_composer.jamo_queue().clear();
        self.base_composer.jamo_queue().extend(queue);

        if !self.compose_hangul() {
            self.base_composer.jamo_queue().pop_back();
            self.compose_hangul();
            let complete_hangul = self.base_composer.current_hangul().get_syllable();

            let current_queue: Vec<_> = self.base_composer.jamo_queue().iter().copied().collect();
            self.base_composer.last_jamo_queue().clear();
            self.base_composer.last_jamo_queue().extend(current_queue);
            self.base_composer.jamo_queue().clear();
            self.base_composer.jamo_queue().push_back(jamo);

            self.clear_jamo();
            self.compose_hangul();
            Some(complete_hangul)
        } else {
            None
        }
    }

    fn remove_jamo(&mut self) -> Option<JamoEnum> {
        self.base_composer.remove_jamo()
    }

    fn compose_hangul(&mut self) -> bool {
        // 큐가 비어있는지 먼저 확인
        if self.base_composer.jamo_queue().is_empty() {
            self.clear_jamo();
            return true;
        }

        // 큐의 내용을 복사하여 작업
        let queue_contents: Vec<_> = self.base_composer.jamo_queue().iter().copied().collect();
        let last_jamo = queue_contents.last().unwrap();
        let last_prev_jamo = if queue_contents.len() > 1 {
            Some(queue_contents[queue_contents.len() - 2])
        } else {
            None
        };

        // 현재 상태 확인
        let is_filled_jung = self.base_composer.current_hangul().is_filled_jung();

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

        if !self.compose_cho() || !self.compose_jung() || !self.compose_jong() {
            return false;
        }

        true
    }

    fn force_compose_hangul(&mut self) -> Option<char> {
        self.base_composer.force_compose_hangul()
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

    fn get_combined_jamo(&self) -> &HashMap<JamoEnum, HashMap<JamoEnum, JamoEnum>> {
        self.base_composer.get_combined_jamo()
    }

    fn jamo_queue(&mut self) -> &mut VecDeque<JamoEnum> {
        self.base_composer.jamo_queue()
    }

    fn last_jamo_queue(&mut self) -> &mut VecDeque<JamoEnum> {
        self.base_composer.last_jamo_queue()
    }

    fn combined_jamo(&mut self) -> &mut HashMap<JamoEnum, HashMap<JamoEnum, JamoEnum>> {
        self.base_composer.combined_jamo()
    }

    fn current_hangul(&mut self) -> &mut HangulChar {
        self.base_composer.current_hangul()
    }
}
