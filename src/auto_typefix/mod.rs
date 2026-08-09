//! AutoTypeFix 모듈
//!
//! 키스트로크 버퍼 기반 실시간 한영 오타 자동 교정.
//!
//! - 순방향 (영어모드→한글): keycode → 한글 조합 시뮬 → 완성 음절 수 기준 트리거
//! - 역방향 (한글모드→영문): keycode → 영문 복원 → 사전 매칭 + 길이 기준 트리거
//!
//! 트리거 시: 화면의 기존 문자를 삭제하고 교정 결과를 commit.

use std::collections::HashSet;

use once_cell::sync::Lazy;

use crate::keycode::{KeyCode, ModifierState};

mod buffer;
mod dictionary;
mod forward;
mod reverse;

#[cfg(test)]
mod tests;

pub use buffer::{KeystrokeBuffer, KeystrokeEntry};
pub use dictionary::{count_korean_syllables, dictionary_contains};
pub use forward::check_forward;
pub use reverse::check_reverse;

/// 영어 사전 (include_str! 임베드)
static ENGLISH_WORDS: &str = include_str!("../data/english_words.txt");

/// 영어 사전 HashSet (lazy 초기화)
pub(crate) static DICTIONARY: Lazy<HashSet<&'static str>> = Lazy::new(|| {
    ENGLISH_WORDS
        .lines()
        .filter(|line| !line.is_empty())
        .collect()
});

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
    /// 진행 중 조합(preedit/보유 영문) 자체를 교정 결과로 치환해야 하는지.
    ///
    /// `KeystrokeBuffer::word_mode`(호출자가 `engine.is_word_mode()` 로 설정)가 켜진
    /// 라이브 조합에서만 `true`. 순방향은 word 모드면 `true`(보유 영문 라이브 조합),
    /// 역방향은 `word_mode && committed_chars == 0`(committed=0 단일 라이브 조합)일 때만
    /// `true`. 이때 프런트(Windows TSF)는 surrounding-text 삭제(비협조앱 차단·synth 강등)
    /// 대신 조합 SetText(`update_composition`/`end_composition_with_text`)로 치환한다.
    /// `false`(음절 모드/committed 섞임)면 기존 삭제 경로와 바이트 동일 — 무회귀 불변식.
    pub replace_composition: bool,
}
