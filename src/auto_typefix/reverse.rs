//! 역방향 (한글모드 → 영문) 오타 감지

use crate::config::AutoTypeFixConfig;
use crate::typefix_blacklist::{BlacklistGate, Direction};
use crate::typefix_userdict::UserDictGate;

use super::buffer::KeystrokeBuffer;
use super::{AutoTypeFixResult, DICTIONARY};

/// 역방향: 한글모드에서 영문 오타 감지
///
/// keycode 버퍼 → 영문 복원 → 사전 매칭 + 길이 기준 트리거.
/// 삭제할 글자 수 = committed_chars + (preedit이면 1)
///
/// `user_dict`에 등록된 단어는 내장 영어 사전(`DICTIONARY`) 조회와
/// `eng_word_min_length` 검사를 우회한다. CLI 명령어(`git`, `ls`, `rustc` 등)
/// 같은 짧고 특수한 단어를 위한 사용자 whitelist.
pub fn check_reverse(
    buffer: &KeystrokeBuffer,
    config: &AutoTypeFixConfig,
    korean_layout: &str,
    english_layout: &str,
    blacklist: &dyn BlacklistGate,
    user_dict: &dyn UserDictGate,
) -> Option<AutoTypeFixResult> {
    if !config.reverse || buffer.len() < 2 {
        return None;
    }

    // keycode → 영문 문자열 (지정된 영문 레이아웃 기준으로 복원)
    let eng = buffer.to_ascii_string(english_layout);
    if eng.is_empty() || !eng.chars().all(|c| c.is_ascii_alphabetic()) {
        return None;
    }

    // 학습형 억제: 해당 시퀀스가 blacklist에서 활성 상태이면 즉시 억제.
    if blacklist.is_suppressed(&eng, Direction::Reverse, korean_layout, english_layout) {
        return None;
    }

    let lower = eng.to_lowercase();
    // 사용자 사전에 있는 단어는 내장 사전·길이 검사를 우회한다.
    let in_user_dict = config.user_dict_enabled && user_dict.contains_reverse(&lower);

    if !in_user_dict {
        // 길이 기준 체크
        if eng.len() < config.eng_word_min_length as usize {
            return None;
        }
    }

    // 온전한 음절 검증: 버퍼의 한글이 모두 완성 음절(U+AC00~U+D7A3)로 구성된 경우
    // (= 정상 한글 입력으로 보임) `skip_on_complete_syllable=true`일 때 억제.
    //
    // commit된 글자는 음절 단위로 commit되므로 이미 완성 음절이다.
    // preedit이 남아있으면 조합 중인 자모가 있다는 뜻이므로 "모두 완성 음절"이 아니다.
    // 따라서 has_preedit == false 이면 버퍼 전체가 완성 음절 상태다.
    //
    // 사용자 사전 단어도 이 검사는 유지: 자연스러운 한글 문장 타이핑 중
    // 의도치 않은 교정을 막기 위함.
    if config.skip_on_complete_syllable && !buffer.has_preedit && buffer.committed_chars > 0 {
        return None;
    }

    if !in_user_dict {
        // 영어 사전 매칭
        if !DICTIONARY.contains(lower.as_str()) {
            return None;
        }
    }

    // 화면에 있는 글자 수 = committed 한글 음절 + preedit(있으면 1)
    let screen_chars = buffer.committed_chars as u32 + if buffer.has_preedit { 1 } else { 0 };

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
        // word 모드에서 한글 단어는 committed=0 인 단일 라이브 조합으로 떠 있다. 이때만
        // 프런트가 확정문 삭제(비협조앱 차단) 대신 조합 SetText 치환을 택한다. committed>0
        // (음절 모드 또는 확정 섞임)이거나 word 모드가 아니면 false → 기존 삭제 경로 바이트 동일.
        replace_composition: buffer.word_mode && buffer.committed_chars == 0,
    })
}
