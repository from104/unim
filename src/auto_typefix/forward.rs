//! 순방향 (영어모드 → 한글) 오타 감지

use crate::config::AutoTypeFixConfig;
use crate::keycode::{KeyCode, ModifierState};
use crate::typefix;
use crate::typefix_blacklist::{BlacklistGate, Direction};

use super::buffer::KeystrokeBuffer;
use super::dictionary::count_korean_syllables;
use super::{AutoTypeFixResult, DICTIONARY};

/// 순방향: 영어모드에서 한글 오타 감지
///
/// keycode 버퍼 → 한글 조합 시뮬레이션 → 완성 음절 수가 임계값 이상이면 트리거.
/// 초성+중성 이상이면 1음절로 카운트.
pub fn check_forward(
    buffer: &KeystrokeBuffer,
    config: &AutoTypeFixConfig,
    korean_layout: &str,
    english_layout: &str,
    blacklist: &dyn BlacklistGate,
) -> Option<AutoTypeFixResult> {
    if !config.forward || buffer.len() < 2 {
        return None;
    }

    // keycode → ASCII 문자열 (지정된 영문 레이아웃 기준)
    let ascii = buffer.to_ascii_string(english_layout);
    if ascii.is_empty() {
        return None;
    }

    // 학습형 억제: 해당 시퀀스가 blacklist에서 활성 상태이면 즉시 억제.
    if blacklist.is_suppressed(&ascii, Direction::Forward, korean_layout, english_layout) {
        return None;
    }

    // 영어 사전에 있으면 진짜 영어 → 스킵 (알파벳으로만 된 경우만 체크).
    // `skip_on_english_word=false`인 경우 이 억제 로직을 건너뛴다.
    if config.skip_on_english_word && ascii.chars().all(|c| c.is_ascii_alphabetic()) {
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
                e.keycode
                    .to_char_for_layout(english_layout, e.modifier.shift)
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
