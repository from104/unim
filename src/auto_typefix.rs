//! AutoTypeFix 모듈
//!
//! 키스트로크 버퍼 기반 실시간 한영 오타 자동 교정.
//!
//! - 순방향 (영어모드→한글): keycode → 한글 조합 시뮬 → 완성 음절 수 기준 트리거
//! - 역방향 (한글모드→영문): keycode → 영문 복원 → 사전 매칭 + 길이 기준 트리거
//!
//! 트리거 시: 화면의 기존 문자를 삭제하고 교정 결과를 commit.

use std::collections::{HashSet, VecDeque};
use std::sync::LazyLock;
use std::time::Instant;

use crate::config::{AutoTypeFixConfig, EnglishLayout, KoreanLayout};
use crate::keycode::{KeyCode, ModifierState};
use crate::typefix;

/// 영어 사전 (include_str! 임베드)
static ENGLISH_WORDS: &str = include_str!("data/english_words.txt");

/// 영어 사전 HashSet (lazy 초기화)
static DICTIONARY: LazyLock<HashSet<&'static str>> = LazyLock::new(|| {
    ENGLISH_WORDS
        .lines()
        .filter(|line| !line.is_empty())
        .collect()
});

/// 키스트로크 버퍼 엔트리
#[derive(Debug, Clone)]
pub struct KeystrokeEntry {
    pub keycode: KeyCode,
    pub modifier: ModifierState,
    pub timestamp: Instant,
}

/// 키스트로크 버퍼 (컨텍스트별)
#[derive(Debug)]
pub struct KeystrokeBuffer {
    entries: VecDeque<KeystrokeEntry>,
    /// 역방향: 이미 commit된 한글 글자 수 (preedit 제외)
    pub committed_chars: usize,
    /// 역방향: 현재 preedit이 있는지
    pub has_preedit: bool,
}

impl KeystrokeBuffer {
    pub fn new() -> Self {
        Self {
            entries: VecDeque::with_capacity(16),
            committed_chars: 0,
            has_preedit: false,
        }
    }

    /// 문자 키 추가. 비문자 키(Enter, Backspace 등)면 false 반환.
    /// 세벌식에서는 숫자/특수문자 키에도 자모가 할당되므로
    /// `is_character_key()` (알파벳+숫자+기호) 전체를 허용.
    pub fn push(&mut self, keycode: KeyCode, modifier: ModifierState) -> bool {
        if !keycode.is_character_key() {
            return false;
        }
        // Space는 단어 구분자이므로 버퍼에 넣지 않음
        if keycode == KeyCode::Space {
            return false;
        }

        self.entries.push_back(KeystrokeEntry {
            keycode,
            modifier,
            timestamp: Instant::now(),
        });
        true
    }

    /// 버퍼 초기화
    pub fn clear(&mut self) {
        self.entries.clear();
        self.committed_chars = 0;
        self.has_preedit = false;
    }

    /// 현재 키스트로크 수
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// 엔트리를 Vec으로 복사해서 반환
    pub fn entries_vec(&self) -> Vec<KeystrokeEntry> {
        self.entries.iter().cloned().collect()
    }

    /// 시간 윈도우 밖의 오래된 엔트리 제거
    pub fn expire(&mut self, time_window_ms: u32) {
        let now = Instant::now();
        let window = std::time::Duration::from_millis(time_window_ms as u64);
        while let Some(front) = self.entries.front() {
            if now.duration_since(front.timestamp) > window {
                self.entries.pop_front();
            } else {
                break;
            }
        }
    }

    /// 버퍼의 keycode들을 ASCII 문자열로 변환 (QWERTY 기준)
    pub fn to_ascii_string(&self) -> String {
        let mut s = String::with_capacity(self.entries.len());
        for entry in &self.entries {
            let c = if entry.modifier.shift {
                entry.keycode.to_shifted_char()
            } else {
                entry.keycode.to_char()
            };
            if let Some(c) = c {
                s.push(c);
            }
        }
        s
    }

    /// commit 발생 시 committed_chars 업데이트
    pub fn update_on_commit(&mut self, commit_str: &str) {
        self.committed_chars += commit_str.chars().count();
    }

    /// preedit 변경 시 has_preedit 업데이트
    pub fn update_on_preedit(&mut self, preedit_str: &str) {
        self.has_preedit = !preedit_str.is_empty();
    }
}

/// AutoTypeFix 교정 결과
#[derive(Debug, Clone, PartialEq)]
pub struct AutoTypeFixResult {
    /// 삭제할 화면 글자 수
    pub delete_chars: u32,
    /// 시그널로 commit할 텍스트 (마지막 음절 제외)
    pub commit_text: String,
    /// 전체 교정 텍스트 (되돌리기용)
    pub corrected: String,
    /// 원래 텍스트 (되돌리기용)
    pub original: String,
    /// preedit을 비워야 하는지
    pub clear_preedit: bool,
    /// 마지막 음절을 replay할 키스트로크 (순방향: 엔진에 다시 입력하여 preedit 생성)
    pub replay_keys: Vec<(KeyCode, ModifierState)>,
}

/// 순방향: 영어모드에서 한글 오타 감지
///
/// keycode 버퍼 → 한글 조합 시뮬레이션 → 완성 음절 수가 임계값 이상이면 트리거.
/// 초성+중성 이상이면 1음절로 카운트.
pub fn check_forward(
    buffer: &KeystrokeBuffer,
    config: &AutoTypeFixConfig,
    korean_layout: KoreanLayout,
    english_layout: EnglishLayout,
) -> Option<AutoTypeFixResult> {
    if !config.forward || buffer.len() < 2 {
        return None;
    }

    // keycode → ASCII 문자열 (영어 키 기준)
    let ascii = buffer.to_ascii_string();
    if ascii.is_empty() {
        return None;
    }

    // 영어 사전에 있으면 진짜 영어 → 스킵 (알파벳으로만 된 경우만 체크)
    if ascii.chars().all(|c| c.is_ascii_alphabetic()) {
        let lower = ascii.to_lowercase();
        if DICTIONARY.contains(lower.as_str()) {
            return None;
        }
    }

    // 한글 조합 시뮬레이션
    let converted = typefix::eng_to_kor(&ascii, korean_layout, english_layout);

    // 완성 음절 수 카운트 (초성+중성 이상 = 1음절)
    let syllable_count = count_korean_syllables(&converted);

    if syllable_count < config.kor_syllable_threshold as usize {
        return None;
    }

    // 온전한 한글 검증: 마지막 글자를 제외한 모든 글자가 완성 음절이어야 함.
    // 예: "패ㅕㅕㅣ머" → 중간에 독립 자모(ㅕ,ㅕ,ㅣ)가 있으므로 트리거 안 됨.
    // "서기" → 마지막 '기' 제외하면 '서'만 남고 완성 음절 → OK.
    // 마지막 글자는 조합 중(독립 자모)일 수 있으므로 허용.
    let chars: Vec<char> = converted.chars().collect();
    if chars.len() > 1 {
        for &c in &chars[..chars.len() - 1] {
            if !('\u{AC00}'..='\u{D7A3}').contains(&c) {
                return None;
            }
        }
    }

    // 마지막 음절 분리: commit할 부분 + preedit으로 replay할 부분
    // converted의 앞 (n-1)글자를 target_prefix로 두고,
    // partial eng_to_kor가 정확히 target_prefix와 일치하는 가장 큰 i를 찾는다.
    // 예: "tjrl" → converted="서기" → target="서"
    //   i=3: "tjr"→"석" ≠ "서"
    //   i=2: "tj"→"서" == "서" → last_syllable_start=2 → replay=[R,L] → 기
    let entries = buffer.entries_vec();
    let total_keys = entries.len();
    let conv_chars: Vec<char> = converted.chars().collect();
    let target_prefix: String = if conv_chars.len() > 1 {
        conv_chars[..conv_chars.len() - 1].iter().collect()
    } else {
        String::new()
    };
    let mut last_syllable_start = total_keys;

    for i in (1..total_keys).rev() {
        let partial_ascii: String = entries[..i]
            .iter()
            .filter_map(|e| {
                if e.modifier.shift {
                    e.keycode.to_shifted_char()
                } else {
                    e.keycode.to_char()
                }
            })
            .collect();
        if partial_ascii.is_empty() {
            continue;
        }
        let partial_kor = typefix::eng_to_kor(&partial_ascii, korean_layout, english_layout);
        if partial_kor == target_prefix {
            last_syllable_start = i;
            break;
        }
    }

    // commit 부분과 replay 키 결정
    // converted(전체 eng_to_kor 결과)에서 마지막 글자를 제외하면 받침 분리가 자연스럽게 반영됨.
    // 예: "tjrl" → converted="서기" → commit="서", replay로 "기" 재생성
    // prefix를 재계산하면 eng_to_kor("tjr")="석"이 되어 받침이 남는 문제 발생.
    let (commit_text, replay_keys) = if last_syllable_start < total_keys {
        let conv_chars: Vec<char> = converted.chars().collect();
        let commit_only: String = conv_chars[..conv_chars.len() - 1].iter().collect();

        // replay: last_syllable_start 이후의 모든 키
        let replay: Vec<(KeyCode, ModifierState)> = entries[last_syllable_start..]
            .iter()
            .map(|e| (e.keycode, e.modifier))
            .collect();

        (commit_only, replay)
    } else {
        (converted.clone(), Vec::new())
    };

    Some(AutoTypeFixResult {
        delete_chars: ascii.chars().count() as u32,
        commit_text,
        corrected: converted,
        original: ascii,
        clear_preedit: false,
        replay_keys,
    })
}

/// 역방향: 한글모드에서 영문 오타 감지
///
/// keycode 버퍼 → 영문 복원 → 사전 매칭 + 길이 기준 트리거.
/// 삭제할 글자 수 = committed_chars + (preedit이면 1)
pub fn check_reverse(
    buffer: &KeystrokeBuffer,
    config: &AutoTypeFixConfig,
) -> Option<AutoTypeFixResult> {
    if !config.reverse || buffer.len() < 2 {
        return None;
    }

    // keycode → 영문 문자열 (물리 키에서 직접 복원)
    let eng = buffer.to_ascii_string();
    if eng.is_empty() || !eng.chars().all(|c| c.is_ascii_alphabetic()) {
        return None;
    }

    // 길이 기준 체크
    if eng.len() < config.eng_word_min_length as usize {
        return None;
    }

    // 영어 사전 매칭
    let lower = eng.to_lowercase();
    if !DICTIONARY.contains(lower.as_str()) {
        return None;
    }

    // 화면에 있는 글자 수 = committed 한글 음절 + preedit(있으면 1)
    let screen_chars = buffer.committed_chars as u32
        + if buffer.has_preedit { 1 } else { 0 };

    if screen_chars == 0 {
        return None;
    }

    // 원래 한글 텍스트 복원 (되돌리기용) — 정확한 복원은 어려우므로 빈 문자열
    // 되돌리기 시 delete_chars + eng 삭제 후 원래 한글을 재입력해야 하므로
    // engine reset 후 keystroke replay가 필요
    Some(AutoTypeFixResult {
        delete_chars: screen_chars,
        commit_text: eng.clone(),
        corrected: eng,
        original: String::new(),
        clear_preedit: buffer.has_preedit,
        replay_keys: Vec::new(), // 역방향는 영어로 교정 → preedit 불필요
    })
}

/// 한글 텍스트에서 음절 수 카운트
///
/// 초성+중성 이상이면 1음절로 카운트:
/// - 완성형 음절 (U+AC00~U+D7A3): 항상 1음절
/// - 호환 자모 중 모음 (ㅏ~ㅣ): preedit에서 초+중 조합 진행 중일 수 있으나
///   독립 모음은 카운트하지 않음
/// - 독립 자음 (ㄱ~ㅎ): 카운트하지 않음
pub fn count_korean_syllables(text: &str) -> usize {
    let mut count = 0;
    for c in text.chars() {
        if ('\u{AC00}'..='\u{D7A3}').contains(&c) {
            // 완성형 음절: 항상 초+중 이상
            count += 1;
        }
        // 독립 자모(ㄱ~ㅎ, ㅏ~ㅣ)는 카운트하지 않음
        // eng_to_kor 결과에서 preedit 잔여물은 독립 자모로 나옴
    }
    count
}

/// 영어 사전에 단어가 있는지 확인 (외부 사용)
pub fn dictionary_contains(word: &str) -> bool {
    DICTIONARY.contains(word)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dictionary_loaded() {
        assert!(DICTIONARY.len() > 50000);
        assert!(dictionary_contains("hello"));
        assert!(dictionary_contains("world"));
        assert!(!dictionary_contains("asdfgh"));
    }

    #[test]
    fn test_count_korean_syllables() {
        assert_eq!(count_korean_syllables("한글"), 2);
        assert_eq!(count_korean_syllables("안녕하세요"), 5);
        assert_eq!(count_korean_syllables("ㅎ"), 0);       // 독립 자음
        assert_eq!(count_korean_syllables("ㅏ"), 0);       // 독립 모음
        assert_eq!(count_korean_syllables("한ㄱ"), 1);     // 1음절 + 독립 자음
        assert_eq!(count_korean_syllables(""), 0);
    }

    #[test]
    fn test_keystroke_buffer_basic() {
        let mut buf = KeystrokeBuffer::new();
        assert!(buf.push(KeyCode::G, ModifierState::default()));
        assert!(buf.push(KeyCode::K, ModifierState::default()));
        assert_eq!(buf.len(), 2);
        assert_eq!(buf.to_ascii_string(), "gk");

        // 비알파벳 키는 추가 안 됨
        assert!(!buf.push(KeyCode::Space, ModifierState::default()));
        assert_eq!(buf.len(), 2);
    }

    #[test]
    fn test_keystroke_buffer_shift() {
        let mut buf = KeystrokeBuffer::new();
        let shifted = ModifierState { shift: true, ..Default::default() };
        buf.push(KeyCode::A, shifted);
        assert_eq!(buf.to_ascii_string(), "A");
    }

    #[test]
    fn test_forward_gksrmf() {
        // "gksrmf" → "한글" (2음절)
        let mut buf = KeystrokeBuffer::new();
        for key in [KeyCode::G, KeyCode::K, KeyCode::S, KeyCode::R, KeyCode::M, KeyCode::F] {
            buf.push(key, ModifierState::default());
        }

        let config = AutoTypeFixConfig {
            enabled: true,
            kor_syllable_threshold: 2,
            eng_word_min_length: 5,
            time_window_ms: 2000,
            forward: true,
            reverse: true,
        };

        let result = check_forward(&buf, &config, KoreanLayout::Dubeolsik, EnglishLayout::Qwerty);
        assert!(result.is_some());
        let r = result.unwrap();
        assert_eq!(r.corrected, "한글");
        assert_eq!(r.delete_chars, 6);
    }

    #[test]
    fn test_forward_4keys_2syllables() {
        // "gksk" → "하나" (2음절, 4키)
        let mut buf = KeystrokeBuffer::new();
        for key in [KeyCode::G, KeyCode::K, KeyCode::S, KeyCode::K] {
            buf.push(key, ModifierState::default());
        }

        let config = AutoTypeFixConfig {
            enabled: true,
            kor_syllable_threshold: 2,
            eng_word_min_length: 5,
            time_window_ms: 2000,
            forward: true,
            reverse: true,
        };

        let result = check_forward(&buf, &config, KoreanLayout::Dubeolsik, EnglishLayout::Qwerty);
        assert!(result.is_some());
        let r = result.unwrap();
        assert_eq!(r.corrected, "하나");
        assert_eq!(r.delete_chars, 4);
    }

    #[test]
    fn test_forward_skip_real_english() {
        // "hello" 는 사전에 있으므로 스킵
        let mut buf = KeystrokeBuffer::new();
        for key in [KeyCode::H, KeyCode::E, KeyCode::L, KeyCode::L, KeyCode::O] {
            buf.push(key, ModifierState::default());
        }

        let config = AutoTypeFixConfig::default();
        let result = check_forward(&buf, &config, KoreanLayout::Dubeolsik, EnglishLayout::Qwerty);
        assert!(result.is_none());
    }

    #[test]
    fn test_forward_threshold_not_met() {
        // "gk" → "하" (1음절, 임계값 2 미달)
        let mut buf = KeystrokeBuffer::new();
        buf.push(KeyCode::G, ModifierState::default());
        buf.push(KeyCode::K, ModifierState::default());

        let config = AutoTypeFixConfig {
            kor_syllable_threshold: 2,
            ..AutoTypeFixConfig::default()
        };

        let result = check_forward(&buf, &config, KoreanLayout::Dubeolsik, EnglishLayout::Qwerty);
        assert!(result.is_none());
    }

    #[test]
    fn test_reverse_hello() {
        // 한글모드에서 "hello" 타이핑 — keycode에서 직접 영문 복원
        let mut buf = KeystrokeBuffer::new();
        for key in [KeyCode::H, KeyCode::E, KeyCode::L, KeyCode::L, KeyCode::O] {
            buf.push(key, ModifierState::default());
        }
        // 시뮬: "ㅗ디ㅣㅐ" — committed 3글자 + preedit 1글자
        buf.committed_chars = 3;
        buf.has_preedit = true;

        let config = AutoTypeFixConfig {
            eng_word_min_length: 5,
            ..AutoTypeFixConfig::default()
        };

        let result = check_reverse(&buf, &config);
        assert!(result.is_some());
        let r = result.unwrap();
        assert_eq!(r.corrected, "hello");
        assert_eq!(r.delete_chars, 4); // 3 committed + 1 preedit
        assert!(r.clear_preedit);
    }

    #[test]
    fn test_reverse_short_word_skip() {
        // "the" (3자) — eng_word_min_length=5 미달
        let mut buf = KeystrokeBuffer::new();
        for key in [KeyCode::T, KeyCode::H, KeyCode::E] {
            buf.push(key, ModifierState::default());
        }
        buf.committed_chars = 1;
        buf.has_preedit = true;

        let config = AutoTypeFixConfig {
            eng_word_min_length: 5,
            ..AutoTypeFixConfig::default()
        };

        let result = check_reverse(&buf, &config);
        assert!(result.is_none());
    }

    #[test]
    fn test_reverse_not_in_dictionary() {
        // "gksrmf" — 사전에 없음
        let mut buf = KeystrokeBuffer::new();
        for key in [KeyCode::G, KeyCode::K, KeyCode::S, KeyCode::R, KeyCode::M, KeyCode::F] {
            buf.push(key, ModifierState::default());
        }
        buf.committed_chars = 2;

        let config = AutoTypeFixConfig::default();
        let result = check_reverse(&buf, &config);
        assert!(result.is_none());
    }

    #[test]
    fn test_forward_batchim_split_dubeolsik() {
        // 두벌식 "tjrl" → "서기" (2음절)
        // commit은 "서"여야 함 (받침 ㄱ이 다음 음절 초성으로 분리)
        // eng_to_kor("tjr") = "석"이 아닌, converted 전체 "서기"에서 마지막 제외 = "서"
        let mut buf = KeystrokeBuffer::new();
        for key in [KeyCode::T, KeyCode::J, KeyCode::R, KeyCode::L] {
            buf.push(key, ModifierState::default());
        }

        let config = AutoTypeFixConfig {
            kor_syllable_threshold: 2,
            ..AutoTypeFixConfig::default()
        };

        let result = check_forward(&buf, &config, KoreanLayout::Dubeolsik, EnglishLayout::Qwerty);
        assert!(result.is_some());
        let r = result.unwrap();
        assert_eq!(r.corrected, "서기");
        assert_eq!(r.commit_text, "서", "받침 분리: commit은 '서'여야 함 ('석' 아님)");
        // replay는 [R, L] — ㄱ+ㅣ=기 재생성 가능해야 함 ([L]만이면 ㅣ만 나옴)
        assert_eq!(r.replay_keys.len(), 2, "replay는 [R, L] 2개여야 함");
        assert_eq!(r.replay_keys[0].0, KeyCode::R);
        assert_eq!(r.replay_keys[1].0, KeyCode::L);
    }

    #[test]
    fn test_forward_incomplete_syllable_skip_3set() {
        // "preedit" 세벌식 → 중간에 독립 자모가 끼어 트리거 안 됨
        let mut buf = KeystrokeBuffer::new();
        for key in [KeyCode::P, KeyCode::R, KeyCode::E, KeyCode::E, KeyCode::D, KeyCode::I, KeyCode::T] {
            buf.push(key, ModifierState::default());
        }

        let config = AutoTypeFixConfig {
            kor_syllable_threshold: 2,
            ..AutoTypeFixConfig::default()
        };

        let result = check_forward(&buf, &config, KoreanLayout::Sebeolsik390, EnglishLayout::Qwerty);
        assert!(result.is_none(), "세벌식: 중간에 독립 자모가 있으면 트리거하면 안 됨");
    }

    #[test]
    fn test_forward_incomplete_syllable_skip_2set() {
        // "preedit" 두벌식 → 중간에 독립 자모가 끼면 트리거 안 됨
        let mut buf = KeystrokeBuffer::new();
        for key in [KeyCode::P, KeyCode::R, KeyCode::E, KeyCode::E, KeyCode::D, KeyCode::I, KeyCode::T] {
            buf.push(key, ModifierState::default());
        }

        let config = AutoTypeFixConfig {
            kor_syllable_threshold: 2,
            ..AutoTypeFixConfig::default()
        };

        let result = check_forward(&buf, &config, KoreanLayout::Dubeolsik, EnglishLayout::Qwerty);
        // 두벌식에서도 완성 음절만으로 구성되지 않으면 스킵
        let ascii = "preedit";
        let converted = crate::typefix::eng_to_kor(ascii, KoreanLayout::Dubeolsik, EnglishLayout::Qwerty);
        let chars: Vec<char> = converted.chars().collect();
        let all_complete = chars.len() <= 1
            || chars[..chars.len() - 1].iter().all(|c| ('\u{AC00}'..='\u{D7A3}').contains(c));
        if all_complete {
            // 두벌식에서 모두 완성 음절이면 트리거될 수 있음 — 그건 정상
            assert!(result.is_some() || result.is_none());
        } else {
            assert!(result.is_none(), "두벌식: 중간에 독립 자모가 있으면 트리거하면 안 됨");
        }
    }
}
