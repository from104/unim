use std::collections::HashMap;

use super::char::HangulChar;
use super::jamo::{is_cho, is_jong, is_jung,  Jamo, Jung};

// pub trait HangulChar: Jamo + 'static {}

pub struct HangulBuilder2Bul {
    hangul_char: HangulChar,
    jamo_stack: Vec<Box<dyn Jamo>>,
    last_jamo_stack: Vec<Box<dyn Jamo>>,
    combined_jamo: HashMap<Box<dyn Jamo>, HashMap<Box<dyn Jamo>, Box<dyn Jamo>>>,
}

impl HangulBuilder2Bul {
    fn new() -> Self {
        HangulBuilder2Bul {
            hangul_char: HangulChar::new(),
            jamo_stack: Vec::new(),
            last_jamo_stack: Vec::new(),
            combined_jamo: [
                (Box::new(Jung::O), {
                    let mut map = HashMap::new();
                    map.insert(
                        Box::new(Jung::A) as Box<dyn Jamo>,
                        Box::new(Jung::WA) as Box<dyn Jamo>,
                    );
                    map.insert(
                        Box::new(Jung::AE) as Box<dyn Jamo>,
                        Box::new(Jung::WAE) as Box<dyn Jamo>,
                    );
                    map.insert(
                        Box::new(Jung::I) as Box<dyn Jamo>,
                        Box::new(Jung::OE) as Box<dyn Jamo>,
                    );
                    map
                }),
                (Box::new(Jung::U) as Box<dyn Jamo>, {
                    let mut map = HashMap::new();
                    map.insert(
                        Box::new(Jung::EO) as Box<dyn Jamo>,
                        Box::new(Jung::WEO) as Box<dyn Jamo>,
                    );
                    map.insert(
                        Box::new(Jung::E) as Box<dyn Jamo>,
                        Box::new(Jung::WE) as Box<dyn Jamo>,
                    );
                    map.insert(
                        Box::new(Jung::I) as Box<dyn Jamo>,
                        Box::new(Jung::WI) as Box<dyn Jamo>,
                    );
                    map
                }),
                (Box::new(Jung::EU) as Box<dyn Jamo>, {
                    let mut map = HashMap::new();
                    map.insert(
                        Box::new(Jung::I) as Box<dyn Jamo>,
                        Box::new(Jung::YI) as Box<dyn Jamo>,
                    );
                    map
                }),
            ]
            .iter()
            .cloned()
            .collect(),
        }
    }

    fn add_jamo(&mut self, jamo: Box<dyn Jamo>) -> &HangulBuilder;

    fn remove_jamo(&mut self) -> Option<&HangulBuilder> {
        if self.jamo_stack.is_empty() {
            None
        } else {
            let jamo: JamoEnum = self.jamo_stack.pop().unwrap();
            self.build_hangul();
            Some(self)
        }
    }
    fn build_cho(&mut self) -> bool {
        let mut jamo_array: Vec<Option<JamoEnum>> = Vec::new();
        let mut jamo_array: Vec<Option<Box<dyn Jamo>>> = Vec::new();
        let mut first_jamo: Option<Box<dyn Jamo>> = None;
        let mut second_jamo: Option<Box<dyn Jamo>> = None;

        for jamo in &self.jamo_stack {
            if is_cho(jamo) {
                jamo_array.push(jamo.clone());
            }
        }

        if jamo_array.is_empty() {
            self.clear_cho();
        } else {
            first_jamo = Some(jamo_array[0].clone());
            if jamo_array.len() > 1 {
                jamo_array.remove(0);
                for jamo in jamo_array {
                    first_jamo = Some(self.get_cho().clone());
                    second_jamo = Some(jamo.clone());
                    if let Some(combined_jamo_map) = self.combined_jamo.get(&first_jamo.unwrap()) {
                        if let Some(combined_jamo) = combined_jamo_map.get(&second_jamo.unwrap()) {
                            self.set_cho(combined_jamo.clone());
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
    fn build_jung(&mut self) -> bool {
        let mut jamo_array: Vec<JamoEnum> = Vec::new();
        let mut jamo_array: Vec<Box<dyn Jamo>> = Vec::new();
        let mut first_jamo: Option<Box<dyn Jamo>> = None;
        let mut second_jamo: Option<Box<dyn Jamo>> = None;

        for jamo in &self.jamo_stack {
            if is_jung(jamo) {
                jamo_array.push(jamo.clone());
            }
        }

        if jamo_array.is_empty() {
            self.clear_jung();
        } else {
            first_jamo = Some(jamo_array[0].clone());
            if jamo_array.len() > 1 {
                jamo_array.remove(0);
                for jamo in jamo_array {
                    first_jamo = Some(self.get_jung().clone());
                    second_jamo = Some(jamo.clone());
                    if let Some(combined_jamo_map) = self.combined_jamo.get(&first_jamo.unwrap()) {
                        if let Some(combined_jamo) = combined_jamo_map.get(&second_jamo.unwrap()) {
                            self.set_jung(combined_jamo.clone());
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
    fn build_jong(&mut self) -> bool {
        let mut jamo_array: Vec<JamoEnum> = Vec::new();
        let mut jamo_array: Vec<Box<dyn Jamo>> = Vec::new();
        let mut first_jamo: Option<Box<dyn Jamo>> = None;
        let mut second_jamo: Option<Box<dyn Jamo>> = None;

        for jamo in &self.jamo_stack {
            if is_jong(jamo) {
                jamo_array.push(jamo.clone());
            }
        }

        if jamo_array.is_empty() {
            self.clear_jong();
        } else {
            first_jamo = Some(jamo_array[0].clone());
            if jamo_array.len() > 1 {
                jamo_array.remove(0);
                for jamo in jamo_array {
                    first_jamo = Some(self.get_jong().clone());
                    second_jamo = Some(jamo.clone());
                    if let Some(combined_jamo_map) = self.combined_jamo.get(&first_jamo.unwrap()) {
                        if let Some(combined_jamo) = combined_jamo_map.get(&second_jamo.unwrap()) {
                            self.set_jong(combined_jamo.clone());
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
    fn build_hangul(&mut self) -> Option<Box<dyn Jamo>> {
        self.build_cho();
        self.build_jung();
        self.build_jong();
        None
    }
    fn force_build_hangul(&mut self) -> Option<Box<dyn Jamo>> {
        if self.is_build() {
            self.build_hangul();
            let complete_hangul = HangulChar {
                cho: self.get_cho().clone(),
                jung: self.get_jung().clone(),
                jong: self.get_jong().clone(),
            };
            self.clear();
            self.jamo_stack.clear();
            self.last_jamo_stack.clear();
            Some(Box::new(complete_hangul))
        } else {
            None
        }
    }

    fn is_build(&self) -> bool {
        !self.jamo_stack.is_empty()
    }
}
