// hangul.rs
use std::fmt;

use super::phoneme::{
    get_coda_by_index, get_nucleus_by_index, get_onset_by_index, Coda, Nucleus, Onset, Phoneme,
};

#[derive(Clone, Debug)]
pub struct Hangul {
    pub(crate) onset: Option<Onset>,
    pub(crate) nucleus: Option<Nucleus>,
    pub(crate) coda: Option<Coda>,
}

impl Default for Hangul {
    fn default() -> Self {
        Self::new()
    }
}

impl Hangul {
    // 초성, 중성, 종성 갯수
    const NUMBER_OF_ONSETS: i32 = 19;
    const NUMBER_OF_NUCLEI: i32 = 21;
    const NUMBER_OF_CODAS: i32 = 28;

    // 한글 음절 유니코드 시작 코드
    const FIRST_UNICODE_OF_SYLLABLE_OF_HANGUL: i32 = '\u{AC00}' as i32;
    // 한글 음절 갯수
    const NUMBER_OF_SYLLABLES_OF_HANGUL: i32 =
        Hangul::NUMBER_OF_ONSETS * Hangul::NUMBER_OF_NUCLEI * Hangul::NUMBER_OF_CODAS;

    pub fn new() -> Self {
        Hangul {
            onset: None,
            nucleus: None,
            coda: None,
        }
    }

    pub fn from_phoneme(onset: Onset, nucleus: Nucleus, coda: Coda) -> Self {
        let mut hangul = Hangul::new();
        hangul.set_phoneme(onset, nucleus, coda);
        hangul
    }

    pub fn from_indices(onset: i32, nucleus: i32, coda: i32) -> Self {
        let mut hangul = Hangul::new();
        hangul.set_phoneme_by_indices(onset, nucleus, coda);
        hangul
    }

    pub fn from_names(onset: &str, nucleus: &str, coda: &str) -> Self {
        let mut hangul = Hangul::new();
        hangul.set_phoneme_by_names(onset, nucleus, coda);
        hangul
    }

    pub fn from_syllable(syllable: char) -> Self {
        let mut hangul = Hangul::new();
        hangul.set_phoneme_by_syllable(syllable);
        hangul
    }

    pub fn set_phoneme(&mut self, onset: Onset, nucleus: Nucleus, coda: Coda) -> bool {
        let (a, b, c) = (
            self.set_onset(Some(onset)),
            self.set_nucleus(Some(nucleus)),
            self.set_coda(Some(coda)),
        );
        a && b && c
    }

    pub fn set_phoneme_by_indices(&mut self, onset: i32, nucleus: i32, coda: i32) -> bool {
        let (a, b, c) = (
            self.set_onset_by_index(onset),
            self.set_nucleus_by_index(nucleus),
            self.set_coda_by_index(coda),
        );
        a && b && c
    }

    pub fn set_phoneme_by_names(&mut self, onset: &str, nucleus: &str, coda: &str) -> bool {
        let (a, b, c) = (
            self.set_onset_by_name(onset),
            self.set_nucleus_by_name(nucleus),
            self.set_coda_by_name(coda),
        );
        a && b && c
    }

    pub fn set_phoneme_by_syllable(&mut self, syllable: char) {
        let syllable_code = syllable as i32;
        if (Hangul::FIRST_UNICODE_OF_SYLLABLE_OF_HANGUL
            ..(Hangul::FIRST_UNICODE_OF_SYLLABLE_OF_HANGUL + Hangul::NUMBER_OF_SYLLABLES_OF_HANGUL))
            .contains(&syllable_code)
        {
            let syll = syllable_code - Hangul::FIRST_UNICODE_OF_SYLLABLE_OF_HANGUL;

            let coda = syll % Hangul::NUMBER_OF_CODAS;
            let nucleus = (syll / Hangul::NUMBER_OF_CODAS) % Hangul::NUMBER_OF_NUCLEI;
            let onset = syll / (Hangul::NUMBER_OF_NUCLEI * Hangul::NUMBER_OF_CODAS);

            self.set_onset_by_index(onset);
            self.set_nucleus_by_index(nucleus);
            self.set_coda_by_index(coda);
        } else {
            panic!("Invalid Hangul syllable code");
        }
    }

    pub fn set_onset(&mut self, onset: Option<Onset>) -> bool {
        if let Some(onset) = onset {
            self.onset = Some(onset);
            true
        } else {
            self.clear_onset();
            false
        }
    }

    pub fn set_nucleus(&mut self, nucleus: Option<Nucleus>) -> bool {
        if let Some(nucleus) = nucleus {
            self.nucleus = Some(nucleus);
            true
        } else {
            self.clear_nucleus();
            false
        }
    }

    pub fn set_coda(&mut self, coda: Option<Coda>) -> bool {
        if let Some(coda) = coda {
            self.coda = Some(coda);
            true
        } else {
            self.clear_coda();
            false
        }
    }

    pub fn set_onset_by_index(&mut self, onset: i32) -> bool {
        if (0..Hangul::NUMBER_OF_ONSETS).contains(&onset) {
            self.set_onset(get_onset_by_index(onset))
        } else {
            self.clear_onset();
            false
        }
    }

    pub fn set_nucleus_by_index(&mut self, nucleus: i32) -> bool {
        if (0..Hangul::NUMBER_OF_NUCLEI).contains(&{ nucleus }) {
            self.set_nucleus(get_nucleus_by_index(nucleus))
        } else {
            self.clear_nucleus();
            false
        }
    }

    pub fn set_coda_by_index(&mut self, coda: i32) -> bool {
        if (0..Hangul::NUMBER_OF_CODAS).contains(&{ coda }) {
            self.set_coda(get_coda_by_index(coda))
        } else {
            self.clear_coda();
            false
        }
    }
    pub fn set_onset_by_name(&mut self, onset: &str) -> bool {
        if let Ok(onset_enum) = onset.parse::<Onset>() {
            self.set_onset(Some(onset_enum))
        } else {
            self.clear_onset();
            false
        }
    }

    pub fn set_nucleus_by_name(&mut self, nucleus: &str) -> bool {
        if let Ok(nucleus_enum) = nucleus.parse::<Nucleus>() {
            self.set_nucleus(Some(nucleus_enum))
        } else {
            self.clear_nucleus();
            false
        }
    }

    pub fn set_coda_by_name(&mut self, coda: &str) -> bool {
        if let Ok(coda_enum) = coda.parse::<Coda>() {
            self.set_coda(Some(coda_enum))
        } else {
            self.clear_coda();
            false
        }
    }

    pub fn clear_onset(&mut self) {
        self.onset = None;
    }

    pub fn clear_nucleus(&mut self) {
        self.nucleus = None;
    }

    pub fn clear_coda(&mut self) {
        self.coda = None;
    }

    pub fn clear(&mut self) {
        self.clear_onset();
        self.clear_nucleus();
        self.clear_coda();
    }

    pub fn is_filled_onset(&self) -> bool {
        self.onset.is_some()
    }

    pub fn is_filled_nucleus(&self) -> bool {
        self.nucleus.is_some()
    }

    pub fn is_filled_coda(&self) -> bool {
        self.coda.is_some()
    }

    pub fn is_filled_only_onset(&self) -> bool {
        self.is_filled_onset() && !self.is_filled_nucleus() && !self.is_filled_coda()
    }

    pub fn is_filled_only_nucleus(&self) -> bool {
        !self.is_filled_onset() && self.is_filled_nucleus() && !self.is_filled_coda()
    }

    pub fn is_filled_only_coda(&self) -> bool {
        !self.is_filled_onset() && !self.is_filled_nucleus() && self.is_filled_coda()
    }

    pub fn is_empty(&self) -> bool {
        !self.is_filled_onset() && !self.is_filled_nucleus() && !self.is_filled_coda()
    }

    pub fn get_onset(&self) -> Option<Onset> {
        self.onset
    }

    pub fn get_nucleus(&self) -> Option<Nucleus> {
        self.nucleus
    }

    pub fn get_coda(&self) -> Option<Coda> {
        self.coda
    }

    pub fn get_onset_unicode(&self) -> char {
        if let Some(onset) = self.onset {
            onset.get_unicode()
        } else {
            '\0'
        }
    }

    pub fn get_nucleus_unicode(&self) -> char {
        if let Some(nucleus) = self.nucleus {
            nucleus.get_unicode()
        } else {
            '\0'
        }
    }

    pub fn get_coda_unicode(&self) -> char {
        if let Some(coda) = self.coda {
            coda.get_unicode()
        } else {
            '\0'
        }
    }

    pub fn get_onset_unicode_compat(&self) -> char {
        if let Some(onset) = self.onset {
            onset.get_unicode_compat()
        } else {
            '\0'
        }
    }

    pub fn get_nucleus_unicode_compat(&self) -> char {
        if let Some(nucleus) = self.nucleus {
            nucleus.get_unicode_compat()
        } else {
            '\0'
        }
    }

    pub fn get_coda_unicode_compat(&self) -> char {
        if let Some(coda) = self.coda {
            coda.get_unicode_compat()
        } else {
            '\0'
        }
    }

    pub fn get_unicodes(&self) -> Vec<char> {
        if self.is_empty() {
            vec![' '] // 공백
        } else if self.is_filled_onset() && !self.is_filled_nucleus() && self.is_filled_coda() {
            vec!['?']
        } else if !self.is_filled_coda() {
            let mut c = vec!['\0'; 2];
            c[0] = self.get_onset_unicode();
            c[1] = self.get_nucleus_unicode();
            c
        } else {
            let mut c = vec!['\0'; 3];
            c[0] = self.get_onset_unicode();
            c[1] = self.get_nucleus_unicode();
            c[2] = self.get_coda_unicode();
            c
        }
    }

    pub fn get_syllable(&self) -> char {
        if self.is_empty() {
            '\0'
        } else if self.is_filled_only_onset() {
            self.get_onset_unicode_compat()
        } else if self.is_filled_only_nucleus() {
            self.get_nucleus_unicode_compat()
        } else if self.is_filled_only_coda() {
            self.get_coda_unicode_compat()
        } else if self.is_filled_onset() && self.is_filled_nucleus() {
            let onset = self.onset.unwrap().get_index();
            let nucleus = self.nucleus.unwrap().get_index();
            let coda = if self.is_filled_coda() {
                self.coda.unwrap().get_index()
            } else {
                0
            };
            let onset_offset = onset * Hangul::NUMBER_OF_NUCLEI * Hangul::NUMBER_OF_CODAS;
            let nucleus_offset = nucleus * Hangul::NUMBER_OF_CODAS;
            let coda_offset = coda;
            let syllable = Hangul::FIRST_UNICODE_OF_SYLLABLE_OF_HANGUL
                + onset_offset
                + nucleus_offset
                + coda_offset;
            char::from_u32(syllable.try_into().unwrap()).unwrap()
        } else {
            '?'
        }
    }

}

impl fmt::Display for Hangul {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let syllable = self.get_syllable();
        if syllable != '?' {
            write!(f, "{}", syllable)
        } else {
            let unicodes: String = self.get_unicodes().iter().collect();
            write!(f, "{}", unicodes)
        }
    }
}
