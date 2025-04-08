// builder2bul.rs
use crate::hangul::builder::BaseHangulBuilder;
use crate::hangul::builder::HangulBuilder;
use crate::hangul::char::HangulChar;
use crate::hangul::jamo::*;
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
        builder.initialize_combined_jamo();
        builder
    }

    /// 2벌식 키보드 매핑을 생성합니다.
    fn create_keyboard_map(&self) -> HashMap<char, JamoEnum> {
        let mut keyboard_map = HashMap::new();

        // 초성 (Consonants)
        keyboard_map.insert('q', JamoEnum::Cho(Cho::B));
        keyboard_map.insert('w', JamoEnum::Cho(Cho::J));
        keyboard_map.insert('e', JamoEnum::Cho(Cho::D));
        keyboard_map.insert('r', JamoEnum::Cho(Cho::G));
        keyboard_map.insert('t', JamoEnum::Cho(Cho::S));
        keyboard_map.insert('a', JamoEnum::Cho(Cho::M));
        keyboard_map.insert('s', JamoEnum::Cho(Cho::N));
        keyboard_map.insert('d', JamoEnum::Cho(Cho::E));
        keyboard_map.insert('f', JamoEnum::Cho(Cho::R));
        keyboard_map.insert('g', JamoEnum::Cho(Cho::H));
        keyboard_map.insert('z', JamoEnum::Cho(Cho::K));
        keyboard_map.insert('x', JamoEnum::Cho(Cho::T));
        keyboard_map.insert('c', JamoEnum::Cho(Cho::C));
        keyboard_map.insert('v', JamoEnum::Cho(Cho::P));

        // 중성 (Vowels)
        keyboard_map.insert('y', JamoEnum::Jung(Jung::YO));
        keyboard_map.insert('u', JamoEnum::Jung(Jung::YEO));
        keyboard_map.insert('i', JamoEnum::Jung(Jung::YA));
        keyboard_map.insert('o', JamoEnum::Jung(Jung::AE));
        keyboard_map.insert('p', JamoEnum::Jung(Jung::E));
        keyboard_map.insert('h', JamoEnum::Jung(Jung::O));
        keyboard_map.insert('j', JamoEnum::Jung(Jung::EO));
        keyboard_map.insert('k', JamoEnum::Jung(Jung::A));
        keyboard_map.insert('l', JamoEnum::Jung(Jung::I));
        keyboard_map.insert('b', JamoEnum::Jung(Jung::YU));
        keyboard_map.insert('n', JamoEnum::Jung(Jung::U));
        keyboard_map.insert('m', JamoEnum::Jung(Jung::EU));

        // 대문자 (쌍자음) (Double Consonants)
        keyboard_map.insert('Q', JamoEnum::Cho(Cho::BB));
        keyboard_map.insert('W', JamoEnum::Cho(Cho::JJ));
        keyboard_map.insert('E', JamoEnum::Cho(Cho::DD));
        keyboard_map.insert('R', JamoEnum::Cho(Cho::GG));
        keyboard_map.insert('T', JamoEnum::Cho(Cho::SS));

        // 대문자 (복합 모음) (Complex Vowels)
        keyboard_map.insert('O', JamoEnum::Jung(Jung::YAE));
        keyboard_map.insert('P', JamoEnum::Jung(Jung::YE));

        keyboard_map
    }

    fn initialize_combined_jamo(&mut self) {
        let mut combined_jamo = HashMap::new();

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
        let mut g_map = HashMap::new();
        g_map.insert(JamoEnum::Jong(Jong::S), JamoEnum::Jong(Jong::GS));
        combined_jamo.insert(JamoEnum::Jong(Jong::G), g_map);

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

        let mut b_map = HashMap::new();
        b_map.insert(JamoEnum::Jong(Jong::S), JamoEnum::Jong(Jong::BS));
        combined_jamo.insert(JamoEnum::Jong(Jong::B), b_map);

        *self.base_builder.combined_jamo() = combined_jamo;
    }

    /// 문자열을 한글로 변환합니다.
    pub fn convert_string(&mut self, input: &str) -> String {
        let keyboard_map = self.create_keyboard_map();
        BaseHangulBuilder::convert_string(input, &keyboard_map, self)
    }

    /// 한글 문자열을 두벌식 영문 자판 입력으로 변환합니다.
    pub fn convert_korean_to_english(&self, input: &str) -> String {
        let keyboard_map = self.create_keyboard_map();
        let mut reverse_map = HashMap::new();

        // 키보드 맵을 뒤집어서 자모 -> 키 매핑 생성
        for (key, jamo) in keyboard_map.iter() {
            reverse_map.insert(*jamo, *key);
        }

        // 이중 모음 및 쌍자음 분해 매핑 정의 (분해되는 문자 -> 기본 구성 요소)
        let mut compound_jung_map = HashMap::new();
        compound_jung_map.insert(Jung::WA, vec![Jung::O, Jung::A]); // ㅘ -> ㅗ + ㅏ
        compound_jung_map.insert(Jung::WAE, vec![Jung::O, Jung::AE]); // ㅙ -> ㅗ + ㅐ
        compound_jung_map.insert(Jung::OE, vec![Jung::O, Jung::I]); // ㅚ -> ㅗ + ㅣ
        compound_jung_map.insert(Jung::WEO, vec![Jung::U, Jung::EO]); // ㅝ -> ㅜ + ㅓ
        compound_jung_map.insert(Jung::WE, vec![Jung::U, Jung::E]); // ㅞ -> ㅜ + ㅔ
        compound_jung_map.insert(Jung::WI, vec![Jung::U, Jung::I]); // ㅟ -> ㅜ + ㅣ
        compound_jung_map.insert(Jung::YI, vec![Jung::EU, Jung::I]); // ㅢ -> ㅡ + ㅣ

        let mut compound_cho_map = HashMap::new();
        compound_cho_map.insert(Cho::GG, vec![Cho::G, Cho::G]); // ㄲ -> ㄱ + ㄱ
        compound_cho_map.insert(Cho::DD, vec![Cho::D, Cho::D]); // ㄸ -> ㄷ + ㄷ
        compound_cho_map.insert(Cho::BB, vec![Cho::B, Cho::B]); // ㅃ -> ㅂ + ㅂ
        compound_cho_map.insert(Cho::SS, vec![Cho::S, Cho::S]); // ㅆ -> ㅅ + ㅅ
        compound_cho_map.insert(Cho::JJ, vec![Cho::J, Cho::J]); // ㅉ -> ㅈ + ㅈ

        let mut compound_jong_map = HashMap::new();
        compound_jong_map.insert(Jong::GG, vec![Jong::G, Jong::G]); // ㄲ -> ㄱ + ㄱ
        compound_jong_map.insert(Jong::GS, vec![Jong::G, Jong::S]); // ㄳ -> ㄱ + ㅅ
        compound_jong_map.insert(Jong::NJ, vec![Jong::N, Jong::J]); // ㄵ -> ㄴ + ㅈ
        compound_jong_map.insert(Jong::NH, vec![Jong::N, Jong::H]); // ㄶ -> ㄴ + ㅎ
        compound_jong_map.insert(Jong::LG, vec![Jong::L, Jong::G]); // ㄺ -> ㄹ + ㄱ
        compound_jong_map.insert(Jong::LM, vec![Jong::L, Jong::M]); // ㄻ -> ㄹ + ㅁ
        compound_jong_map.insert(Jong::LB, vec![Jong::L, Jong::B]); // ㄼ -> ㄹ + ㅂ
        compound_jong_map.insert(Jong::LS, vec![Jong::L, Jong::S]); // ㄽ -> ㄹ + ㅅ
        compound_jong_map.insert(Jong::LT, vec![Jong::L, Jong::T]); // ㄾ -> ㄹ + ㅌ
        compound_jong_map.insert(Jong::LP, vec![Jong::L, Jong::P]); // ㄿ -> ㄹ + ㅍ
        compound_jong_map.insert(Jong::LH, vec![Jong::L, Jong::H]); // ㅀ -> ㄹ + ㅎ
        compound_jong_map.insert(Jong::BS, vec![Jong::B, Jong::S]); // ㅄ -> ㅂ + ㅅ
        compound_jong_map.insert(Jong::SS, vec![Jong::S, Jong::S]); // ㅆ -> ㅅ + ㅅ

        let mut result = String::new();

        for c in input.chars() {
            if c as u32 >= 0xAC00 && c as u32 <= 0xD7A3 {
                // 한글 완성형 문자인 경우
                let mut hangul_char = HangulChar::new();
                hangul_char.set_jamo_by_syllable(c);

                // 초성 변환 (쌍자음 처리)
                if let Some(cho) = hangul_char.get_cho() {
                    // 쌍자음은 대문자로 변환
                    match cho {
                        Cho::GG => result.push('R'),
                        Cho::DD => result.push('E'),
                        Cho::BB => result.push('Q'),
                        Cho::SS => result.push('T'),
                        Cho::JJ => result.push('W'),
                        _ => {
                            // 기본 자음인 경우 그대로 처리
                            if let Some(&key) = reverse_map.get(&JamoEnum::Cho(cho)) {
                                result.push(key);
                            }
                        }
                    }
                }

                // 중성 변환 (이중 모음 처리)
                if let Some(jung) = hangul_char.get_jung() {
                    if let Some(components) = compound_jung_map.get(&jung) {
                        // 이중 모음인 경우 분해해서 처리
                        for &base_jung in components {
                            if let Some(&key) = reverse_map.get(&JamoEnum::Jung(base_jung)) {
                                result.push(key);
                            }
                        }
                    } else {
                        // 기본 모음인 경우 그대로 처리
                        if let Some(&key) = reverse_map.get(&JamoEnum::Jung(jung)) {
                            result.push(key);
                        }
                    }
                }

                // 종성 변환 (겹받침 처리)
                if let Some(jong) = hangul_char.get_jong() {
                    // Jong::E는 빈 종성을 의미함
                    if jong != Jong::E {
                        // 종성 쌍자음 처리 (직접 대문자로 매핑)
                        match jong {
                            Jong::GG => result.push('R'), // ㄲ -> R
                            Jong::SS => result.push('T'), // ㅆ -> T
                            _ => {
                                if let Some(components) = compound_jong_map.get(&jong) {
                                    // 겹받침인 경우 분해해서 처리 (두벌식에서는 초성으로 변환)
                                    for &base_jong in components {
                                        let cho = base_jong.to_cho();
                                        if let Some(&key) = reverse_map.get(&JamoEnum::Cho(cho)) {
                                            result.push(key);
                                        }
                                    }
                                } else {
                                    // 기본 받침인 경우 그대로 처리 (두벌식에서는 초성으로 변환)
                                    let cho = jong.to_cho();
                                    if let Some(&key) = reverse_map.get(&JamoEnum::Cho(cho)) {
                                        result.push(key);
                                    }
                                }
                            }
                        }
                    }
                }
            } else if c as u32 >= 0x3131 && c as u32 <= 0x318E {
                // 한글 호환용 자모인 경우 (ㄱ, ㄴ, ㅏ, ㅑ 등)

                // 1. 각 자모 enum 값을 확인하여 유니코드가 일치하는지 검사

                // 먼저 초성으로 시도
                let mut found = false;

                // 초성(Cho) 확인
                for cho_val in 0..19 {
                    if let Some(cho) = get_cho_by_sequence(cho_val) {
                        let jamo_enum = JamoEnum::Cho(cho);
                        if jamo_enum.get_unicode_compat() == c {
                            // 쌍자음은 대문자로 변환
                            match cho {
                                Cho::GG => result.push('R'),
                                Cho::DD => result.push('E'),
                                Cho::BB => result.push('Q'),
                                Cho::SS => result.push('T'),
                                Cho::JJ => result.push('W'),
                                _ => {
                                    // 기본 자음인 경우 그대로 처리
                                    if let Some(&key) = reverse_map.get(&jamo_enum) {
                                        result.push(key);
                                    }
                                }
                            }
                            found = true;
                            break;
                        }
                    }
                }

                // 중성(Jung) 확인
                if !found {
                    for jung_val in 0..21 {
                        if let Some(jung) = get_jung_by_sequence(jung_val) {
                            let jamo_enum = JamoEnum::Jung(jung);
                            if jamo_enum.get_unicode_compat() == c {
                                // 이중 모음 처리
                                if let Some(components) = compound_jung_map.get(&jung) {
                                    // 이중 모음인 경우 분해해서 처리
                                    for &base_jung in components {
                                        if let Some(&key) =
                                            reverse_map.get(&JamoEnum::Jung(base_jung))
                                        {
                                            result.push(key);
                                        }
                                    }
                                } else {
                                    // 기본 모음인 경우 그대로 처리
                                    if let Some(&key) = reverse_map.get(&jamo_enum) {
                                        result.push(key);
                                    }
                                }
                                found = true;
                                break;
                            }
                        }
                    }
                }

                // 종성(Jong) 확인 - 두벌식에서는 종성이 초성으로 변환됨
                if !found {
                    for jong_val in 1..28 {
                        // 0은 종성 없음이므로 1부터 시작
                        if let Some(jong) = get_jong_by_sequence(jong_val) {
                            let jamo_enum = JamoEnum::Jong(jong);
                            if jamo_enum.get_unicode_compat() == c {
                                // 종성 쌍자음 처리 (직접 대문자로 매핑)
                                match jong {
                                    Jong::GG => result.push('R'), // ㄲ -> R
                                    Jong::SS => result.push('T'), // ㅆ -> T
                                    _ => {
                                        // 겹받침 처리
                                        if let Some(components) = compound_jong_map.get(&jong) {
                                            // 겹받침인 경우 분해해서 처리 (두벌식에서는 초성으로 변환)
                                            for &base_jong in components {
                                                let cho = base_jong.to_cho();
                                                if let Some(&key) =
                                                    reverse_map.get(&JamoEnum::Cho(cho))
                                                {
                                                    result.push(key);
                                                }
                                            }
                                        } else {
                                            // 기본 받침인 경우 그대로 처리 (두벌식에서는 초성으로 변환)
                                            let cho = jong.to_cho();
                                            if let Some(&key) = reverse_map.get(&JamoEnum::Cho(cho))
                                            {
                                                result.push(key);
                                            }
                                        }
                                    }
                                }
                                found = true;
                                break;
                            }
                        }
                    }
                }

                // 매칭되는 키를 찾지 못한 경우 그대로 추가
                if !found {
                    result.push(c);
                }
            } else {
                // 한글이 아닌 경우 그대로 추가
                result.push(c);
            }
        }

        result
    }
}

impl HangulBuilder for HangulBuilder2Bul {
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

    fn add_jamo(&mut self, jamo: JamoEnum) -> Option<char> {
        if !matches!(
            jamo,
            JamoEnum::Cho(_) | JamoEnum::Jung(_) | JamoEnum::Jong(_)
        ) {
            return None;
        }

        // 중성 다음으로 초성이 들어오면 종성으로 변환 시도
        if self.base_builder.is_filled_jung() {
            if let JamoEnum::Cho(cho) = jamo {
                if let Ok(jong) = cho.to_jong() {
                    return self.base_builder.add_jamo(JamoEnum::Jong(jong));
                }
            }
        }

        // 도깨비불 현상 처리 (종성 + 중성 입력시)
        let last_jamo = self.base_builder.jamo_queue().back().copied();
        if let Some(JamoEnum::Jong(jong)) = last_jamo {
            if matches!(jamo, JamoEnum::Jung(_)) {
                // 마지막 종성 큐에서 빼고
                self.base_builder.jamo_queue().pop_back();
                // 현재 글자 완성
                let current_char = self.base_builder.force_build_hangul();

                // 종성을 초성으로 변환하여 새로운 글자 시작
                self.base_builder.add_jamo(JamoEnum::Cho(jong.to_cho()));
                self.base_builder.add_jamo(jamo);

                return current_char;
            }
        }

        self.base_builder.add_jamo(jamo)
    }

    fn remove_jamo(&mut self) -> Option<JamoEnum> {
        self.base_builder.remove_jamo()
    }

    fn build_hangul(&mut self) -> bool {
        if self.base_builder.jamo_queue().is_empty() {
            self.base_builder.clear();
            return true;
        }

        // 마지막 자모와 그 이전 자모 확인
        let queue = self.base_builder.jamo_queue();
        let last_jamo = *queue.back().unwrap();
        let last_prev_jamo = if queue.len() > 1 {
            Some(*queue.get(queue.len() - 2).unwrap())
        } else {
            None
        };

        // 초성이 없고 중성 다음에 종성이 오면
        if !self.base_builder.is_filled_cho()
            && last_prev_jamo.is_some_and(|j| matches!(j, JamoEnum::Jung(_)))
            && matches!(last_jamo, JamoEnum::Jong(_))
        {
            return false;
        }

        // 종성 다음에 중성이 오면
        if last_prev_jamo.is_some_and(|j| matches!(j, JamoEnum::Jong(_)))
            && matches!(last_jamo, JamoEnum::Jung(_))
        {
            return false;
        }

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

    fn get_combined_jamo(&self) -> &HashMap<JamoEnum, HashMap<JamoEnum, JamoEnum>> {
        self.base_builder.get_combined_jamo()
    }

    fn is_new_syllable(&self) -> bool {
        self.base_builder.is_new_syllable()
    }
}
