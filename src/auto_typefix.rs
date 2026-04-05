//! AutoTypeFix 모듈
//!
//! 키스트로크 버퍼 기반 실시간 한영 오타 자동 교정.
//!
//! - 방향 A (영어모드→한글): keycode → 한글 조합 시뮬 → 완성 음절 수 기준 트리거
//! - 방향 B (한글모드→영문): keycode → 영문 복원 → 사전 매칭 + 길이 기준 트리거
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
    /// 방향 B: 이미 commit된 한글 글자 수 (preedit 제외)
    pub committed_chars: usize,
    /// 방향 B: 현재 preedit이 있는지
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

    /// 문자 키 추가. 비문자 키면 false 반환.
    pub fn push(&mut self, keycode: KeyCode, modifier: ModifierState) -> bool {
        // 알파벳 키만 버퍼에 추가 (숫자, 특수문자 제외)
        let is_alpha = matches!(
            keycode,
            KeyCode::A | KeyCode::B | KeyCode::C | KeyCode::D | KeyCode::E |
            KeyCode::F | KeyCode::G | KeyCode::H | KeyCode::I | KeyCode::J |
            KeyCode::K | KeyCode::L | KeyCode::M | KeyCode::N | KeyCode::O |
            KeyCode::P | KeyCode::Q | KeyCode::R | KeyCode::S | KeyCode::T |
            KeyCode::U | KeyCode::V | KeyCode::W | KeyCode::X | KeyCode::Y |
            KeyCode::Z
        );
        if !is_alpha {
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
    /// 교정된 텍스트 (commit할 내용)
    pub corrected: String,
    /// 원래 텍스트 (되돌리기용)
    pub original: String,
    /// preedit을 비워야 하는지
    pub clear_preedit: bool,
}

/// 방향 A: 영어모드에서 한글 오타 감지
///
/// keycode 버퍼 → 한글 조합 시뮬레이션 → 완성 음절 수가 임계값 이상이면 트리거.
/// 초성+중성 이상이면 1음절로 카운트.
pub fn check_direction_a(
    buffer: &KeystrokeBuffer,
    config: &AutoTypeFixConfig,
    korean_layout: KoreanLayout,
    english_layout: EnglishLayout,
) -> Option<AutoTypeFixResult> {
    if !config.direction_a || buffer.len() < 2 {
        return None;
    }

    // keycode → ASCII 문자열 (영어 키 기준)
    let ascii = buffer.to_ascii_string();
    if ascii.is_empty() || !ascii.chars().all(|c| c.is_ascii_alphabetic()) {
        return None;
    }

    // 영어 사전에 있으면 진짜 영어 → 스킵
    let lower = ascii.to_lowercase();
    if DICTIONARY.contains(lower.as_str()) {
        return None;
    }

    // 한글 조합 시뮬레이션
    let converted = typefix::eng_to_kor(&ascii, korean_layout, english_layout);

    // 완성 음절 수 카운트 (초성+중성 이상 = 1음절)
    let syllable_count = count_korean_syllables(&converted);

    if syllable_count < config.kor_syllable_threshold as usize {
        return None;
    }

    // 화면에는 영문 N글자가 있으므로 N글자 삭제
    Some(AutoTypeFixResult {
        delete_chars: ascii.chars().count() as u32,
        corrected: converted.clone(),
        original: ascii,
        clear_preedit: false,
    })
}

/// 방향 B: 한글모드에서 영문 오타 감지
///
/// keycode 버퍼 → 영문 복원 → 사전 매칭 + 길이 기준 트리거.
/// 삭제할 글자 수 = committed_chars + (preedit이면 1)
pub fn check_direction_b(
    buffer: &KeystrokeBuffer,
    config: &AutoTypeFixConfig,
) -> Option<AutoTypeFixResult> {
    if !config.direction_b || buffer.len() < 2 {
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
        corrected: eng,
        original: String::new(), // 방향 B 되돌리기는 별도 처리
        clear_preedit: buffer.has_preedit,
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
    fn test_direction_a_gksrmf() {
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
            direction_a: true,
            direction_b: true,
        };

        let result = check_direction_a(&buf, &config, KoreanLayout::Dubeolsik, EnglishLayout::Qwerty);
        assert!(result.is_some());
        let r = result.unwrap();
        assert_eq!(r.corrected, "한글");
        assert_eq!(r.delete_chars, 6);
    }

    #[test]
    fn test_direction_a_4keys_2syllables() {
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
            direction_a: true,
            direction_b: true,
        };

        let result = check_direction_a(&buf, &config, KoreanLayout::Dubeolsik, EnglishLayout::Qwerty);
        assert!(result.is_some());
        let r = result.unwrap();
        assert_eq!(r.corrected, "하나");
        assert_eq!(r.delete_chars, 4);
    }

    #[test]
    fn test_direction_a_skip_real_english() {
        // "hello" 는 사전에 있으므로 스킵
        let mut buf = KeystrokeBuffer::new();
        for key in [KeyCode::H, KeyCode::E, KeyCode::L, KeyCode::L, KeyCode::O] {
            buf.push(key, ModifierState::default());
        }

        let config = AutoTypeFixConfig::default();
        let result = check_direction_a(&buf, &config, KoreanLayout::Dubeolsik, EnglishLayout::Qwerty);
        assert!(result.is_none());
    }

    #[test]
    fn test_direction_a_threshold_not_met() {
        // "gk" → "하" (1음절, 임계값 2 미달)
        let mut buf = KeystrokeBuffer::new();
        buf.push(KeyCode::G, ModifierState::default());
        buf.push(KeyCode::K, ModifierState::default());

        let config = AutoTypeFixConfig {
            kor_syllable_threshold: 2,
            ..AutoTypeFixConfig::default()
        };

        let result = check_direction_a(&buf, &config, KoreanLayout::Dubeolsik, EnglishLayout::Qwerty);
        assert!(result.is_none());
    }

    #[test]
    fn test_direction_b_hello() {
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

        let result = check_direction_b(&buf, &config);
        assert!(result.is_some());
        let r = result.unwrap();
        assert_eq!(r.corrected, "hello");
        assert_eq!(r.delete_chars, 4); // 3 committed + 1 preedit
        assert!(r.clear_preedit);
    }

    #[test]
    fn test_direction_b_short_word_skip() {
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

        let result = check_direction_b(&buf, &config);
        assert!(result.is_none());
    }

    #[test]
    fn test_direction_b_not_in_dictionary() {
        // "gksrmf" — 사전에 없음
        let mut buf = KeystrokeBuffer::new();
        for key in [KeyCode::G, KeyCode::K, KeyCode::S, KeyCode::R, KeyCode::M, KeyCode::F] {
            buf.push(key, ModifierState::default());
        }
        buf.committed_chars = 2;

        let config = AutoTypeFixConfig::default();
        let result = check_direction_b(&buf, &config);
        assert!(result.is_none());
    }
}
