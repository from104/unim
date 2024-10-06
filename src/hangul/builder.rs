use std::collections::{HashMap, VecDeque};
use crate::hangul::phoneme::{Onset, Nucleus, Coda, PhonemeEnum};
use crate::hangul::hangul::Hangul;

pub trait HangulBuilder {
    // 자모가 입력되는 순서대로 저장하는 큐
    fn jamo_queue(&mut self) -> &mut VecDeque<PhonemeEnum>;
    
    // 직전 큐
    fn last_jamo_queue(&mut self) -> &mut VecDeque<PhonemeEnum>;
    
    // 자모 조합 테이블
    fn combined_jamo(&self) -> &HashMap<PhonemeEnum, HashMap<PhonemeEnum, PhonemeEnum>>;
    
    // 현재 조합 중인 한글
    fn current_hangul(&mut self) -> &mut Hangul;

    // 한글 자모 입력
    fn add_jamo(&mut self, jamo: PhonemeEnum) -> Option<char>;

    // 한글 자모 삭제
    fn remove_jamo(&mut self) -> Option<PhonemeEnum> {
        if self.jamo_queue().is_empty() {
            None
        } else {
            let jamo = self.jamo_queue().pop_back().unwrap();
            self.build_hangul();
            Some(jamo)
        }
    }

    // 한글 초성 조합
    fn build_onset(&mut self) -> bool {
        let onset_phonemes: Vec<&PhonemeEnum> = self.jamo_queue().iter()
            .filter(|&p| matches!(p, PhonemeEnum::Onset(_)))
            .collect();

        if onset_phonemes.is_empty() {
            self.current_hangul().set_onset(None);
        } else {
            let mut onset = match onset_phonemes[0] {
                PhonemeEnum::Onset(onset) => *onset,
                _ => unreachable!(),
            };
            for phoneme in onset_phonemes.iter().skip(1) {
                if let Some(combined) = self.combine_phonemes(&PhonemeEnum::Onset(onset), phoneme) {
                    if let PhonemeEnum::Onset(new_onset) = combined {
                        onset = new_onset;
                    } else {
                        return false;
                    }
                } else {
                    return false;
                }
            }
            self.current_hangul().set_onset(Some(onset));
        }
        true
    }

    // 한글 중성 조합
    fn build_nucleus(&mut self) -> bool {
        let nucleus_phonemes: Vec<&PhonemeEnum> = self.jamo_queue().iter()
            .filter(|&p| matches!(p, PhonemeEnum::Nucleus(_)))
            .collect();

        if nucleus_phonemes.is_empty() {
            self.current_hangul().set_nucleus(None);
        } else {
            let mut nucleus = match nucleus_phonemes[0] {
                PhonemeEnum::Nucleus(nucleus) => *nucleus,
                _ => unreachable!(),
            };
            for phoneme in nucleus_phonemes.iter().skip(1) {
                if let Some(combined) = self.combine_phonemes(&PhonemeEnum::Nucleus(nucleus), phoneme) {
                    if let PhonemeEnum::Nucleus(new_nucleus) = combined {
                        nucleus = new_nucleus;
                    } else {
                        return false;
                    }
                } else {
                    return false;
                }
            }
            self.current_hangul().set_nucleus(Some(nucleus));
        }
        true
    }

    // 한글 종성 조합
    fn build_coda(&mut self) -> bool {
        let coda_phonemes: Vec<&PhonemeEnum> = self.jamo_queue().iter()
            .filter(|&p| matches!(p, PhonemeEnum::Coda(_)))
            .collect();

        if coda_phonemes.is_empty() {
            self.current_hangul().set_coda(None);
        } else {
            let mut coda = match coda_phonemes[0] {
                PhonemeEnum::Coda(coda) => *coda,
                _ => unreachable!(),
            };
            for phoneme in coda_phonemes.iter().skip(1) {
                if let Some(combined) = self.combine_phonemes(&PhonemeEnum::Coda(coda), phoneme) {
                    if let PhonemeEnum::Coda(new_coda) = combined {
                        coda = new_coda;
                    } else {
                        return false;
                    }
                } else {
                    return false;
                }
            }
            self.current_hangul().set_coda(Some(coda));
        }
        true
    }

    // 스택에 쌓인 자모들로 한글 조합
    fn build_hangul(&mut self) -> bool;

    // 강제로 조합 완료
    fn force_build_hangul(&mut self) -> Option<char> {
        if self.is_build() {
            self.build_hangul();
            let complete_hangul = self.current_hangul().to_char();
            self.clear();
            complete_hangul
        } else {
            None
        }
    }

    // 조합 중인지 여부
    fn is_build(&self) -> bool {
        !self.jamo_queue().is_empty()
    }

    // 자모 조합
    fn combine_phonemes(&self, first: &PhonemeEnum, second: &PhonemeEnum) -> Option<PhonemeEnum> {
        self.combined_jamo().get(first)
            .and_then(|map| map.get(second))
            .cloned()
    }

    // 초기화
    fn clear(&mut self) {
        self.jamo_queue().clear();
        self.last_jamo_queue().clear();
        *self.current_hangul() = Hangul::new();
    }
}