//! 키스트로크 버퍼
//!
//! AutoTypeFix 컨텍스트별로 키스트로크를 보관하고 ASCII로 변환한다.

use std::collections::VecDeque;
use std::time::Instant;

use crate::keycode::{KeyCode, ModifierState};

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

    /// 버퍼의 keycode들을 지정된 영문 레이아웃 기준 ASCII 문자열로 변환
    pub fn to_ascii_string(&self, english_layout: &str) -> String {
        let mut s = String::with_capacity(self.entries.len());
        for entry in &self.entries {
            if let Some(c) = entry
                .keycode
                .to_char_for_layout(english_layout, entry.modifier.shift)
            {
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
