// builder2bul.rs
use crate::hangul::builder::*;
use crate::hangul::char::HangulChar;
use crate::hangul::jamo::{Cho, Jong, Jung, PhonemeEnum};
use std::collections::{HashMap, VecDeque};

/**
 * HangulBuilder2Bul 구조체 (Java의 HangulBuilder2Bul 클래스에 해당)
 */
#[derive(Debug, Default)]
pub struct HangulBuilder2Bul {
    base_builder: BaseHangulBuilder,
}

impl HangulBuilder2Bul {
    pub fn new() -> Self {
        let mut builder = HangulBuilder2Bul {
            base_builder: BaseHangulBuilder::new(),
        };
        builder.initialize_combined_jamo(); // combinedJamo 초기화
        builder
    }

    fn initialize_combined_jamo(&mut self) {
        let mut combined_jamo = HashMap::new();

        // Jung 조합
        let mut o_map = HashMap::new();
        o_map.insert(PhonemeEnum::Jung(Jung::A), PhonemeEnum::Jung(Jung::WA));
        o_map.insert(PhonemeEnum::Jung(Jung::AE), PhonemeEnum::Jung(Jung::WAE));
        o_map.insert(PhonemeEnum::Jung(Jung::I), PhonemeEnum::Jung(Jung::OE));
        combined_jamo.insert(PhonemeEnum::Jung(Jung::O), o_map);

        let mut u_map = HashMap::new();
        u_map.insert(PhonemeEnum::Jung(Jung::EO), PhonemeEnum::Jung(Jung::WEO));
        u_map.insert(PhonemeEnum::Jung(Jung::E), PhonemeEnum::Jung(Jung::WE));
        u_map.insert(PhonemeEnum::Jung(Jung::I), PhonemeEnum::Jung(Jung::WI));
        combined_jamo.insert(PhonemeEnum::Jung(Jung::U), u_map);

        let mut eu_map = HashMap::new();
        eu_map.insert(PhonemeEnum::Jung(Jung::I), PhonemeEnum::Jung(Jung::YI));
        combined_jamo.insert(PhonemeEnum::Jung(Jung::EU), eu_map);

        // Jong 조합
        let mut g_map = HashMap::new();
        g_map.insert(PhonemeEnum::Jong(Jong::S), PhonemeEnum::Jong(Jong::GS));
        combined_jamo.insert(PhonemeEnum::Jong(Jong::G), g_map);

        let mut n_map = HashMap::new();
        n_map.insert(PhonemeEnum::Jong(Jong::J), PhonemeEnum::Jong(Jong::NJ));
        n_map.insert(PhonemeEnum::Jong(Jong::H), PhonemeEnum::Jong(Jong::NH));
        combined_jamo.insert(PhonemeEnum::Jong(Jong::N), n_map);

        let mut l_map = HashMap::new();
        l_map.insert(PhonemeEnum::Jong(Jong::G), PhonemeEnum::Jong(Jong::LG));
        l_map.insert(PhonemeEnum::Jong(Jong::M), PhonemeEnum::Jong(Jong::LM));
        l_map.insert(PhonemeEnum::Jong(Jong::B), PhonemeEnum::Jong(Jong::LB));
        l_map.insert(PhonemeEnum::Jong(Jong::S), PhonemeEnum::Jong(Jong::LS));
        l_map.insert(PhonemeEnum::Jong(Jong::T), PhonemeEnum::Jong(Jong::LT));
        l_map.insert(PhonemeEnum::Jong(Jong::P), PhonemeEnum::Jong(Jong::LP));
        l_map.insert(PhonemeEnum::Jong(Jong::H), PhonemeEnum::Jong(Jong::LH));
        combined_jamo.insert(PhonemeEnum::Jong(Jong::L), l_map);

        let mut b_map = HashMap::new();
        b_map.insert(PhonemeEnum::Jong(Jong::S), PhonemeEnum::Jong(Jong::BS));
        combined_jamo.insert(PhonemeEnum::Jong(Jong::B), b_map);

        *self.base_builder.combined_jamo() = combined_jamo;
    }
}

impl HangulBuilder for HangulBuilder2Bul {
    fn jamo_queue(&mut self) -> &mut VecDeque<PhonemeEnum> {
        self.base_builder.jamo_queue()
    }

    fn last_jamo_queue(&mut self) -> &mut VecDeque<PhonemeEnum> {
        self.base_builder.last_jamo_queue()
    }

    fn combined_jamo(&mut self) -> &mut HashMap<PhonemeEnum, HashMap<PhonemeEnum, PhonemeEnum>> {
        self.base_builder.combined_jamo()
    }

    fn current_hangul(&mut self) -> &mut HangulChar {
        self.base_builder.current_hangul()
    }

    fn add_jamo(&mut self, jamo: PhonemeEnum) -> Option<char> {
        self.base_builder.add_jamo(jamo)
    }

    fn remove_jamo(&mut self) -> Option<PhonemeEnum> {
        self.base_builder.remove_jamo()
    }

    fn build_hangul(&mut self) -> bool {
        self.base_builder.build_hangul()
    }

    fn force_build_hangul(&mut self) -> Option<char> {
        self.base_builder.force_build_hangul()
    }

    fn is_build(&self) -> bool {
        self.base_builder.is_build()
    }

    // --- 내부 조합 함수 위임 ---
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

    fn get_combined_jamo(&self) -> &HashMap<PhonemeEnum, HashMap<PhonemeEnum, PhonemeEnum>> {
        self.base_builder.get_combined_jamo()
    }

    fn is_new_syllable(&self) -> bool {
        self.base_builder
            .peek_jamo_queue()
            .back()
            .map_or(false, |last_jamo| matches!(last_jamo, PhonemeEnum::Cho(_) if self.base_builder.get_current_jung().is_some()))
    }
}
