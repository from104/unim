//! 자동 한/영 전환 감지 모듈
//!
//! 키 입력 패턴을 분석하여 사용자가 의도한 입력 모드와 실제 모드가
//! 불일치하는 경우를 감지합니다.

use crate::config::InputCategory;
use crate::hangul::jamo::JamoEnum;
use std::collections::HashMap;

/// 슬라이딩 윈도우 기반 자동 전환 감지기
pub struct AutoSwitchDetector {
    /// 최근 입력된 키 히스토리 (영문 문자)
    key_history: Vec<char>,
    /// 윈도우 크기
    window_size: usize,
    /// 유효 한글 조합 연속 카운트
    valid_korean_streak: u32,
    /// 유효 영문 연속 카운트
    valid_english_streak: u32,
}

impl AutoSwitchDetector {
    /// 새 감지기를 생성합니다.
    pub fn new() -> Self {
        Self {
            key_history: Vec::with_capacity(8),
            window_size: 6,
            valid_korean_streak: 0,
            valid_english_streak: 0,
        }
    }

    /// 상태를 리셋합니다.
    pub fn reset(&mut self) {
        self.key_history.clear();
        self.valid_korean_streak = 0;
        self.valid_english_streak = 0;
    }

    /// 키 입력을 분석합니다.
    ///
    /// 현재 입력 모드와 키 입력 패턴을 비교하여 전환이 필요한지 판단합니다.
    ///
    /// # Arguments
    /// * `ch` - 입력된 영문 문자 (키보드 물리 키에 대응하는 영문자)
    /// * `current_mode` - 현재 입력 모드
    /// * `keyboard_map` - 한글 키보드 매핑 (영문키 → 자모)
    /// * `threshold` - 전환 감지 임계값 (0.0 ~ 1.0)
    ///
    /// # Returns
    /// * `Some(InputCategory)` - 전환 추천 모드
    /// * `None` - 전환 불필요
    pub fn feed(
        &mut self,
        ch: char,
        current_mode: InputCategory,
        keyboard_map: &HashMap<char, JamoEnum>,
        threshold: f32,
    ) -> Option<InputCategory> {
        // 공백/숫자/기호는 스트릭을 리셋
        if !ch.is_ascii_alphabetic() {
            self.valid_korean_streak = 0;
            self.valid_english_streak = 0;
            self.key_history.clear();
            return None;
        }

        // 히스토리에 추가
        self.key_history.push(ch);
        if self.key_history.len() > self.window_size {
            self.key_history.remove(0);
        }

        // 윈도우가 충분히 채워지지 않으면 판단하지 않음
        if self.key_history.len() < 4 {
            return None;
        }

        match current_mode {
            InputCategory::English => {
                // 영문 모드에서 한글 패턴 감지
                // 연속 키가 한글 자모 매핑에 속하는지 확인
                let korean_mapped = self
                    .key_history
                    .iter()
                    .filter(|c| keyboard_map.contains_key(c))
                    .count();
                let ratio = korean_mapped as f32 / self.key_history.len() as f32;

                // 모음이 포함되어 있는지 확인 (영문 입력에서 자음만 나열되면 한글이 아님)
                let has_vowel = self.key_history.iter().any(|c| {
                    keyboard_map
                        .get(c)
                        .map(|j| matches!(j, JamoEnum::Jung(_)))
                        .unwrap_or(false)
                });

                if ratio >= threshold && has_vowel {
                    self.valid_korean_streak += 1;
                    if self.valid_korean_streak >= 2 {
                        self.reset();
                        return Some(InputCategory::Korean);
                    }
                } else {
                    self.valid_korean_streak = 0;
                }
            }
            InputCategory::Korean => {
                // 한글 모드에서 영문 패턴 감지
                // 한글 조합이 유효하지 않은 연속 (모음 없이 자음만 연속 등)
                let all_consonants = self.key_history.iter().all(|c| {
                    keyboard_map
                        .get(c)
                        .map(|j| matches!(j, JamoEnum::Cho(_)))
                        .unwrap_or(true)
                });

                // 영문 단어 패턴: 모음(a,e,i,o,u)이 적절히 섞임
                let english_vowels = self
                    .key_history
                    .iter()
                    .filter(|c| "aeiou".contains(**c))
                    .count();
                let english_vowel_ratio =
                    english_vowels as f32 / self.key_history.len() as f32;
                let looks_english = (0.15..=0.6).contains(&english_vowel_ratio);

                if all_consonants || looks_english {
                    self.valid_english_streak += 1;
                    if self.valid_english_streak >= 2 {
                        self.reset();
                        return Some(InputCategory::English);
                    }
                } else {
                    self.valid_english_streak = 0;
                }
            }
        }

        None
    }
}

impl Default for AutoSwitchDetector {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::keystroke::{KeyboardMap, get_keymap_json};

    fn create_test_keyboard_map() -> HashMap<char, JamoEnum> {
        let en_json = get_keymap_json("en_qwerty");
        let ko_json = get_keymap_json("2bul");
        KeyboardMap::create_keyboard_map_from_str(en_json, ko_json, false)
    }

    #[test]
    fn test_detector_creation() {
        let detector = AutoSwitchDetector::new();
        assert!(detector.key_history.is_empty());
    }

    #[test]
    fn test_no_switch_on_few_keys() {
        let mut detector = AutoSwitchDetector::new();
        let map = create_test_keyboard_map();

        // 3개 키만 입력 → 판단하지 않음
        assert!(detector.feed('g', InputCategory::English, &map, 0.7).is_none());
        assert!(detector.feed('k', InputCategory::English, &map, 0.7).is_none());
        assert!(detector.feed('s', InputCategory::English, &map, 0.7).is_none());
    }

    #[test]
    fn test_reset() {
        let mut detector = AutoSwitchDetector::new();
        let map = create_test_keyboard_map();

        detector.feed('g', InputCategory::English, &map, 0.7);
        detector.feed('k', InputCategory::English, &map, 0.7);
        detector.reset();
        assert!(detector.key_history.is_empty());
    }

    #[test]
    fn test_space_resets_streak() {
        let mut detector = AutoSwitchDetector::new();
        let map = create_test_keyboard_map();

        detector.feed('g', InputCategory::English, &map, 0.7);
        detector.feed('k', InputCategory::English, &map, 0.7);
        // 공백은 리셋
        detector.feed(' ', InputCategory::English, &map, 0.7);
        assert!(detector.key_history.is_empty());
    }
}
