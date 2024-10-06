use std::collections::{HashMap, VecDeque};
use crate::hangul::phoneme::{Onset, Nucleus, Coda, PhonemeEnum};
use crate::hangul::hangul::Hangul;
use crate::hangul::builder::HangulBuilder;

pub struct HangulBuilder2Bul {
    jamo_queue: VecDeque<PhonemeEnum>,
    last_jamo_queue: VecDeque<PhonemeEnum>,
    combined_jamo: HashMap<PhonemeEnum, HashMap<PhonemeEnum, PhonemeEnum>>,
    current_hangul: Hangul,
}

impl HangulBuilder2Bul {
    pub fn new() -> Self {
        let mut builder = HangulBuilder2Bul {
            jamo_queue: VecDeque::new(),
            last_jamo_queue: VecDeque::new(),
            combined_jamo: HashMap::new(),
            current_hangul: Hangul::new(),
        };
        builder.init_combined_jamo();
        builder
    }

    fn init_combined_jamo(&mut self) {
        // 중성 조합
        self.add_combined_jamo(PhonemeEnum::Nucleus(Nucleus::O), PhonemeEnum::Nucleus(Nucleus::A), PhonemeEnum::Nucleus(Nucleus::WA));
        self.add_combined_jamo(PhonemeEnum::Nucleus(Nucleus::O), PhonemeEnum::Nucleus(Nucleus::AE), PhonemeEnum::Nucleus(Nucleus::WAE));
        self.add_combined_jamo(PhonemeEnum::Nucleus(Nucleus::O), PhonemeEnum::Nucleus(Nucleus::I), PhonemeEnum::Nucleus(Nucleus::OE));
        self.add_combined_jamo(PhonemeEnum::Nucleus(Nucleus::U), PhonemeEnum::Nucleus(Nucleus::EO), PhonemeEnum::Nucleus(Nucleus::WEO));
        self.add_combined_jamo(PhonemeEnum::Nucleus(Nucleus::U), PhonemeEnum::Nucleus(Nucleus::E), PhonemeEnum::Nucleus(Nucleus::WE));
        self.add_combined_jamo(PhonemeEnum::Nucleus(Nucleus::U), PhonemeEnum::Nucleus(Nucleus::I), PhonemeEnum::Nucleus(Nucleus::WI));
        self.add_combined_jamo(PhonemeEnum::Nucleus(Nucleus::EU), PhonemeEnum::Nucleus(Nucleus::I), PhonemeEnum::Nucleus(Nucleus::YI));

        // 종성 조합
        self.add_combined_jamo(PhonemeEnum::Coda(Coda::G), PhonemeEnum::Coda(Coda::S), PhonemeEnum::Coda(Coda::GS));
        self.add_combined_jamo(PhonemeEnum::Coda(Coda::N), PhonemeEnum::Coda(Coda::J), PhonemeEnum::Coda(Coda::NJ));
        self.add_combined_jamo(PhonemeEnum::Coda(Coda::N), PhonemeEnum::Coda(Coda::H), PhonemeEnum::Coda(Coda::NH));
        self.add_combined_jamo(PhonemeEnum::Coda(Coda::L), PhonemeEnum::Coda(Coda::G), PhonemeEnum::Coda(Coda::LG));
        self.add_combined_jamo(PhonemeEnum::Coda(Coda::L), PhonemeEnum::Coda(Coda::M), PhonemeEnum::Coda(Coda::LM));
        self.add_combined_jamo(PhonemeEnum::Coda(Coda::L), PhonemeEnum::Coda(Coda::B), PhonemeEnum::Coda(Coda::LB));
        self.add_combined_jamo(PhonemeEnum::Coda(Coda::L), PhonemeEnum::Coda(Coda::S), PhonemeEnum::Coda(Coda::LS));
        self.add_combined_jamo(PhonemeEnum::Coda(Coda::L), PhonemeEnum::Coda(Coda::T), PhonemeEnum::Coda(Coda::LT));
        self.add_combined_jamo(PhonemeEnum::Coda(Coda::L), PhonemeEnum::Coda(Coda::P), PhonemeEnum::Coda(Coda::LP));
        self.add_combined_jamo(PhonemeEnum::Coda(Coda::L), PhonemeEnum::Coda(Coda::H), PhonemeEnum::Coda(Coda::LH));
        self.add_combined_jamo(PhonemeEnum::Coda(Coda::B), PhonemeEnum::Coda(Coda::S), PhonemeEnum::Coda(Coda::BS));
    }

    fn add_combined_jamo(&mut self, first: PhonemeEnum, second: PhonemeEnum, result: PhonemeEnum) {
        self.combined_jamo.entry(first)
            .or_insert_with(HashMap::new)
            .insert(second, result);
    }
}

impl HangulBuilder for HangulBuilder2Bul {
    fn jamo_queue(&mut self) -> &mut VecDeque<PhonemeEnum> {
        &mut self.jamo_queue
    }

    fn last_jamo_queue(&mut self) -> &mut VecDeque<PhonemeEnum> {
        &mut self.last_jamo_queue
    }

    fn combined_jamo(&self) -> &HashMap<PhonemeEnum, HashMap<PhonemeEnum, PhonemeEnum>> {
        &self.combined_jamo
    }

    fn current_hangul(&mut self) -> &mut Hangul {
        &mut self.current_hangul
    }

    fn add_jamo(&mut self, jamo: PhonemeEnum) -> Option<char> {
        self.jamo_queue.push_back(jamo);
        if self.build_hangul() {
            None
        } else {
            self.jamo_queue.pop_back();
            if let Some(PhonemeEnum::Coda(_)) = self.jamo_queue.back() {
                if let PhonemeEnum::Nucleus(_) = jamo {
                    // 도깨비불 현상 처리
                    let coda = self.jamo_queue.pop_back().unwrap();
                    self.build_hangul();
                    let complete_hangul = self.current_hangul.to_char();
                    self.last_jamo_queue = self.jamo_queue.clone();
                    self.jamo_queue.clear();
                    if let PhonemeEnum::Coda(c) = coda {
                        self.jamo_queue.push_back(PhonemeEnum::Onset(c.to_onset()));
                    }
                    self.jamo_queue.push_back(jamo);
                    self.clear();
                    self.build_hangul();
                    complete_hangul
                } else {
                    self.build_hangul();
                    let complete_hangul = self.current_hangul.to_char();
                    self.last_jamo_queue = self.jamo_queue.clone();
                    self.jamo_queue.clear();
                    self.jamo_queue.push_back(jamo);
                    self.clear();
                    self.build_hangul();
                    complete_hangul
                }
            } else {
                None
            }
        }
    }

    fn build_hangul(&mut self) -> bool {
        if self.jamo_queue.is_empty() {
            self.clear();
            return true;
        }

        let last_jamo = self.jamo_queue.back().unwrap();
        let last_prev_jamo = self.jamo_queue.get(self.jamo_queue.len() - 2);

        // 중성 다음으로 초성이 들어오면 종성으로 변환
        if self.current_hangul.nucleus().is_some() && matches!(last_jamo, PhonemeEnum::Onset(_)) {
            if let Some(PhonemeEnum::Onset(cho)) = self.jamo_queue.pop_back() {
                if let Ok(jong) = cho.to_coda() {
                    self.jamo_queue.push_back(PhonemeEnum::Coda(jong));
                } else {
                    self.jamo_queue.push_back(PhonemeEnum::Onset(cho));
                    return false;
                }
            }
        }

        // 초성이 없고 중성 다음에 종성이 오면 실패
        if self.current_hangul.onset().is_none() && 
           matches!(last_prev_jamo, Some(PhonemeEnum::Nucleus(_))) && 
           matches!(last_jamo, PhonemeEnum::Coda(_)) {
            return false;
        }

        // 종성 다음에 중성이 오면 실패
        if matches!(last_prev_jamo, Some(PhonemeEnum::Coda(_))) && 
           matches!(last_jamo, PhonemeEnum::Nucleus(_)) {
            return false;
        }

        self.build_onset() && self.build_nucleus() && self.build_coda()
    }
}