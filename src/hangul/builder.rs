use std::collections::HashMap;

use super::hangul::Hangul;
use super::phoneme::{
    is_coda, is_nucleus, is_onset, is_phoneme, Coda, Nucleus, Onset, Phoneme,
};

// pub trait Hangul: Phoneme + 'static {}

pub struct HangulBuilder {
    hangul: Hangul,
    phoneme_stack: Vec<dyn Phoneme>,
    last_phoneme_stack: Vec<PhonemeEnum>,
    combined_phoneme: HashMap<PhonemeEnum, HashMap<PhonemeEnum, PhonemeEnum>>,
}

pub trait HangulBuilder2Bul {
    fn new() -> Self;
    fn add_phoneme(&mut self, phoneme: PhonemeEnum) -> &Hangul;
    fn remove_phoneme(&mut self) -> Option<&HangulBuilder>;
    fn build_onset(&mut self) -> bool;
    fn build_nucleus(&mut self) -> bool;
    fn build_coda(&mut self) -> bool;
    fn build_hangul(&mut self) -> Option<PhonemeEnum>;
    fn force_build_hangul(&mut self) -> Option<Box<PhonemeEnum>>;
    fn is_build(&self) -> bool;
}

impl HangulBuilder2Bul for HangulBuilder {
    fn new() -> Self {
        HangulBuilder {
            hangul: Hangul::new(),
            phoneme_stack: Vec::new(),
            last_phoneme_stack: Vec::new(),
            combined_phoneme: HashMap::new(),
        }
    }

    fn add_phoneme(&mut self, phoneme: PhonemeEnum) -> &Hangul {
        if is_phoneme(&phoneme) {
            self.phoneme_stack.push(phoneme);
            if build_hangul().is_some() {
                self.last_phoneme_stack = self.phoneme_stack.clone();
            }
        }
        &self.hangul
    }
    fn remove_phoneme(&mut self) -> Option<&HangulBuilder> {
        if self.phoneme_stack.is_empty() {
            None
        } else {
            let phoneme: PhonemeEnum = self.phoneme_stack.pop().unwrap();
            self.build_hangul();
            Some(self)
        }
    }
    fn build_onset(&mut self) -> bool {
        let mut phoneme_array: Vec<Option<PhonemeEnum>> = Vec::new();
        let mut first_phoneme: Option<PhonemeEnum> = None;
        let mut second_phoneme: Option<PhonemeEnum> = None;

        // 초성만 걸러냄
        for phoneme in &self.phoneme_stack {
            if is_onset(phoneme) {
                phoneme_array.push(Some(phoneme.clone()));
            }
        }

        if phoneme_array.is_empty() {
            self.hangul.clear_onset();
        } else {
            let onset = match phoneme_array[0] {
                Some(PhonemeEnum::Onset(o)) => Some(o),
                _ => None,
            };
            self.hangul.set_onset(onset);
            if phoneme_array.len() > 1 {
                phoneme_array.remove(0);
                for phoneme in phoneme_array {
                    let first_phoneme = self.hangul.get_onset().clone();
                    let second_phoneme = phoneme.clone();
                    if let Some(combined_phoneme_map) = self.combined_phoneme.get(&first_phoneme) {
                        if let Some(combined_phoneme) = combined_phoneme_map.get(&second_phoneme) {
                            self.hangul.set_onset(combined_phoneme.clone());
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
    fn build_nucleus(&mut self) -> bool {
        let mut phoneme_array: Vec<&PhonemeEnum> = Vec::new();
        let mut first_phoneme: Option<Box<dyn Phoneme>> = None;
        let mut second_phoneme: Option<PhonemeEnum> = None;

        for phoneme in &self.phoneme_stack {
            if is_coda(phoneme) {
                phoneme_array.push(phoneme.clone());
            }
        }

        if phoneme_array.is_empty() {
            self.clear_nucleus();
        } else {
            first_phoneme = Some(phoneme_array[0].clone());
            if phoneme_array.len() > 1 {
                phoneme_array.remove(0);
                for phoneme in phoneme_array {
                    first_phoneme = Some(self.get_nucleus().clone());
                    second_phoneme = Some(phoneme.clone());
                    if let Some(combined_phoneme_map) =
                        self.combined_phoneme.get(&first_phoneme.unwrap())
                    {
                        if let Some(combined_phoneme) =
                            combined_phoneme_map.get(&second_phoneme.unwrap())
                        {
                            self.set_nucleus(combined_phoneme.clone());
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
    fn build_coda(&mut self) -> bool {
        let mut phoneme_array: Vec<PhonemeEnum> = Vec::new();
        let mut phoneme_array: Vec<Box<dyn Phoneme>> = Vec::new();
        let mut first_phoneme: Option<Box<dyn Phoneme>> = None;
        let mut second_phoneme: Option<Box<dyn Phoneme>> = None;

        for phoneme in &self.phoneme_stack {
            if is_nucleus(phoneme) {
                phoneme_array.push(phoneme.clone());
            }
        }

        if phoneme_array.is_empty() {
            self.clear_coda();
        } else {
            first_phoneme = Some(phoneme_array[0].clone());
            if phoneme_array.len() > 1 {
                phoneme_array.remove(0);
                for phoneme in phoneme_array {
                    first_phoneme = Some(self.get_coda().clone());
                    second_phoneme = Some(phoneme.clone());
                    if let Some(combined_phoneme_map) =
                        self.combined_phoneme.get(&first_phoneme.unwrap())
                    {
                        if let Some(combined_phoneme) =
                            combined_phoneme_map.get(&second_phoneme.unwrap())
                        {
                            self.set_coda(combined_phoneme.clone());
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
    fn build_hangul(&mut self) -> Option<Box<dyn Phoneme>> {
        self.build_onset();
        self.build_nucleus();
        self.build_coda();
        None
    }
    fn force_build_hangul(&mut self) -> Option<Box<dyn Phoneme>> {
        if self.is_build() {
            self.build_hangul();
            let complete_hangul = Hangul {
                onset: self.get_onset().clone(),
                nucleus: self.get_nucleus().clone(),
                coda: self.get_coda().clone(),
            };
            self.clear();
            self.phoneme_stack.clear();
            self.last_phoneme_stack.clear();
            Some(Box::new(complete_hangul))
        } else {
            None
        }
    }

    fn is_build(&self) -> bool {
        !self.phoneme_stack.is_empty()
    }
}
