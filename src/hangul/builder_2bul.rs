use std::collections::HashMap;
use serde_json::Value;
use crate::hangul::phoneme::{Onset, Nucleus, Coda, PhonemeEnum};
use crate::hangul::hangul::Hangul;
use std::str::FromStr;

pub struct HangulBuilder2Bul {
    phoneme_stack: Vec<PhonemeEnum>,
    last_phoneme_stack: Vec<PhonemeEnum>,
    combined_phoneme: HashMap<PhonemeEnum, HashMap<PhonemeEnum, PhonemeEnum>>,
    current_hangul: Hangul,
}

impl HangulBuilder2Bul {
    pub fn new() -> Self {
        let mut builder = HangulBuilder2Bul {
            phoneme_stack: Vec::new(),
            last_phoneme_stack: Vec::new(),
            combined_phoneme: HashMap::new(),
            current_hangul: Hangul::new(),
        };
        builder.init_combined_phoneme();
        builder
    }

    fn init_combined_phoneme(&mut self) {
        let combined_phoneme_data = r#"
        {
            "Nucleus": {
                "O": {
                    "A": "WA",
                    "AE": "WAE",
                    "I": "OE"
                },
                "U": {
                    "EO": "WEO",
                    "E": "WE",
                    "I": "WI"
                },
                "EU": {
                    "I": "YI"
                }
            },
            "Coda": {
                "G": {
                    "S": "GS"
                },
                "N": {
                    "J": "NJ",
                    "H": "NH"
                },
                "L": {
                    "G": "LG",
                    "M": "LM",
                    "B": "LB",
                    "S": "LS",
                    "T": "LT",
                    "P": "LP",
                    "H": "LH"
                },
                "B": {
                    "S": "BS"
                }
            }
        }
        "#;

        let data: serde_json::Value = serde_json::from_str(combined_phoneme_data).unwrap();

        for (phoneme_type, combinations) in data.as_object().unwrap() {
            for (base, targets) in combinations.as_object().unwrap() {
                let base_phoneme = match phoneme_type.as_str() {
                    "Nucleus" => PhonemeEnum::Nucleus(Nucleus::from_str(base.as_str()).unwrap()),
                    "Coda" => PhonemeEnum::Coda(Coda::from_str(base.as_str()).unwrap()),
                    _ => continue,
                };

                let mut inner_map = HashMap::new();
                for (target, result) in targets.as_object().unwrap() {
                    let target_phoneme = match phoneme_type.as_str() {
                        "Nucleus" => PhonemeEnum::Nucleus(Nucleus::from_str(target).unwrap()),
                        "Coda" => PhonemeEnum::Coda(Coda::from_str(target).unwrap()),
                        _ => continue,
                    };
                    let result_phoneme = match phoneme_type.as_str() {
                        "Nucleus" => PhonemeEnum::Nucleus(Nucleus::from_str(result.as_str().unwrap()).unwrap()),
                        "Coda" => PhonemeEnum::Coda(Coda::from_str(result.as_str().unwrap()).unwrap()),
                        _ => continue,
                    };
                    inner_map.insert(target_phoneme, result_phoneme);
                }
                self.combined_phoneme.insert(base_phoneme, inner_map);
            }
        }
    }

    pub fn add_phoneme(&mut self, phoneme: PhonemeEnum) -> Option<char> {
        self.phoneme_stack.push(phoneme);
        if self.build_hangul() {
            if self.is_complete() {
                let result = self.current_hangul.to_string().chars().next().unwrap();
                self.clear();
                Some(result)
            } else {
                None
            }
        } else {
            None
        }
    }

    pub fn remove_phoneme(&mut self) -> Option<PhonemeEnum> {
        self.phoneme_stack.pop().map(|phoneme| {
            self.build_hangul();
            phoneme
        })
    }

    fn build_onset(&mut self) -> bool {
        let onset_phonemes: Vec<&PhonemeEnum> = self.phoneme_stack.iter()
            .filter(|&p| matches!(p, PhonemeEnum::Onset(_)))
            .collect();

        if onset_phonemes.is_empty() {
            self.current_hangul.set_onset(None);
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
            self.current_hangul.set_onset(Some(onset));
        }
        true
    }

    fn build_nucleus(&mut self) -> bool {
        let nucleus_phonemes: Vec<&PhonemeEnum> = self.phoneme_stack.iter()
            .filter(|&p| matches!(p, PhonemeEnum::Nucleus(_)))
            .collect();

        if nucleus_phonemes.is_empty() {
            self.current_hangul.set_nucleus(None);
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
            self.current_hangul.set_nucleus(Some(nucleus));
        }
        true
    }

    fn build_coda(&mut self) -> bool {
        let coda_phonemes: Vec<&PhonemeEnum> = self.phoneme_stack.iter()
            .filter(|&p| matches!(p, PhonemeEnum::Coda(_)))
            .collect();

        if coda_phonemes.is_empty() {
            self.current_hangul.set_coda(None);
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
            self.current_hangul.set_coda(Some(coda));
        }
        true
    }

    fn build_hangul(&mut self) -> bool {
        self.build_onset() && self.build_nucleus() && self.build_coda()
    }

    pub fn force_build_hangul(&mut self) -> Option<char> {
        if self.is_complete() {
            let result = self.current_hangul.to_char().unwrap();
            self.clear();
            Some(result)
        } else {
            None
        }
    }

    pub fn is_complete(&self) -> bool {
        self.current_hangul.is_complete()
    }

    fn clear(&mut self) {
        self.phoneme_stack.clear();
        self.last_phoneme_stack.clear();
        self.current_hangul = Hangul::new();
    }

    fn combine_phonemes(&self, first: &PhonemeEnum, second: &PhonemeEnum) -> Option<PhonemeEnum> {
        self.combined_phoneme.get(first)
            .and_then(|map| map.get(second))
            .cloned()
    }

    pub fn is_build(&self) -> bool {
        !self.phoneme_stack.is_empty()
    }
}
