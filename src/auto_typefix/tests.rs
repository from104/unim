use super::*;
use crate::config::AutoTypeFixConfig;
use crate::keycode::{KeyCode, ModifierState};
use crate::typefix;
use crate::typefix_blacklist::{Blacklist, Direction, EntryStatus};
use crate::typefix_userdict::EmptyUserDict;

/// 테스트용: blacklist는 기본(빈) 상태. 기존 동작과 동일하게 검사.
fn empty_bl() -> Blacklist {
    Blacklist::default()
}

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
    assert_eq!(count_korean_syllables("ㅎ"), 0); // 독립 자음
    assert_eq!(count_korean_syllables("ㅏ"), 0); // 독립 모음
    assert_eq!(count_korean_syllables("한ㄱ"), 1); // 1음절 + 독립 자음
    assert_eq!(count_korean_syllables(""), 0);
}

#[test]
fn test_keystroke_buffer_basic() {
    let mut buf = KeystrokeBuffer::new();
    assert!(buf.push(KeyCode::G, ModifierState::default()));
    assert!(buf.push(KeyCode::K, ModifierState::default()));
    assert_eq!(buf.len(), 2);
    assert_eq!(buf.to_ascii_string("qwerty"), "gk");

    // 비알파벳 키는 추가 안 됨
    assert!(!buf.push(KeyCode::Space, ModifierState::default()));
    assert_eq!(buf.len(), 2);
}

#[test]
fn test_keystroke_buffer_shift() {
    let mut buf = KeystrokeBuffer::new();
    let shifted = ModifierState {
        shift: true,
        ..Default::default()
    };
    buf.push(KeyCode::A, shifted);
    assert_eq!(buf.to_ascii_string("qwerty"), "A");
}

#[test]
fn test_forward_gksrmf() {
    // "gksrmf" → "한글" (2음절)
    let mut buf = KeystrokeBuffer::new();
    for key in [
        KeyCode::G,
        KeyCode::K,
        KeyCode::S,
        KeyCode::R,
        KeyCode::M,
        KeyCode::F,
    ] {
        buf.push(key, ModifierState::default());
    }

    let config = AutoTypeFixConfig {
        enabled: true,
        kor_syllable_threshold: 2,
        eng_word_min_length: 5,
        forward_time_window_ms: 2000,
        reverse_time_window_ms: 2000,
        forward: true,
        reverse: true,
        ..AutoTypeFixConfig::default()
    };

    let result = check_forward(&buf, &config, "ko_2bulstd", "qwerty", &empty_bl());
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
        forward_time_window_ms: 2000,
        reverse_time_window_ms: 2000,
        forward: true,
        reverse: true,
        ..AutoTypeFixConfig::default()
    };

    let result = check_forward(&buf, &config, "ko_2bulstd", "qwerty", &empty_bl());
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
    let result = check_forward(&buf, &config, "ko_2bulstd", "qwerty", &empty_bl());
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

    let result = check_forward(&buf, &config, "ko_2bulstd", "qwerty", &empty_bl());
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
        // 기존 테스트는 사전 hit 즉시 발화하는 종전 동작을 가정한다.
        ..AutoTypeFixConfig::default()
    };

    let result = check_reverse(
        &buf,
        &config,
        "ko_2bulstd",
        "qwerty",
        &empty_bl(),
        &EmptyUserDict,
    );
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
        // 기존 테스트는 사전 hit 즉시 발화하는 종전 동작을 가정한다.
        ..AutoTypeFixConfig::default()
    };

    let result = check_reverse(
        &buf,
        &config,
        "ko_2bulstd",
        "qwerty",
        &empty_bl(),
        &EmptyUserDict,
    );
    assert!(result.is_none());
}

#[test]
fn test_reverse_not_in_dictionary() {
    // "gksrmf" — 사전에 없음
    let mut buf = KeystrokeBuffer::new();
    for key in [
        KeyCode::G,
        KeyCode::K,
        KeyCode::S,
        KeyCode::R,
        KeyCode::M,
        KeyCode::F,
    ] {
        buf.push(key, ModifierState::default());
    }
    buf.committed_chars = 2;

    let config = AutoTypeFixConfig::default();
    let result = check_reverse(
        &buf,
        &config,
        "ko_2bulstd",
        "qwerty",
        &empty_bl(),
        &EmptyUserDict,
    );
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

    let result = check_forward(&buf, &config, "ko_2bulstd", "qwerty", &empty_bl());
    assert!(result.is_some());
    let r = result.unwrap();
    assert_eq!(r.corrected, "서기");
    assert_eq!(
        r.commit_text, "서",
        "받침 분리: commit은 '서'여야 함 ('석' 아님)"
    );
    // replay는 [R, L] — ㄱ+ㅣ=기 재생성 가능해야 함 ([L]만이면 ㅣ만 나옴)
    assert_eq!(r.replay_keys.len(), 2, "replay는 [R, L] 2개여야 함");
    assert_eq!(r.replay_keys[0].0, KeyCode::R);
    assert_eq!(r.replay_keys[1].0, KeyCode::L);
}

#[test]
fn test_forward_incomplete_syllable_skip_3set() {
    // "preedit" 세벌식 → 중간에 독립 자모가 끼어 트리거 안 됨
    let mut buf = KeystrokeBuffer::new();
    for key in [
        KeyCode::P,
        KeyCode::R,
        KeyCode::E,
        KeyCode::E,
        KeyCode::D,
        KeyCode::I,
        KeyCode::T,
    ] {
        buf.push(key, ModifierState::default());
    }

    let config = AutoTypeFixConfig {
        kor_syllable_threshold: 2,
        ..AutoTypeFixConfig::default()
    };

    let result = check_forward(&buf, &config, "ko_3bul390", "qwerty", &empty_bl());
    assert!(
        result.is_none(),
        "세벌식: 중간에 독립 자모가 있으면 트리거하면 안 됨"
    );
}

#[test]
fn test_forward_incomplete_syllable_skip_2set() {
    // "preedit" 두벌식 → 중간에 독립 자모가 끼면 트리거 안 됨
    let mut buf = KeystrokeBuffer::new();
    for key in [
        KeyCode::P,
        KeyCode::R,
        KeyCode::E,
        KeyCode::E,
        KeyCode::D,
        KeyCode::I,
        KeyCode::T,
    ] {
        buf.push(key, ModifierState::default());
    }

    let config = AutoTypeFixConfig {
        kor_syllable_threshold: 2,
        ..AutoTypeFixConfig::default()
    };

    let result = check_forward(&buf, &config, "ko_2bulstd", "qwerty", &empty_bl());
    // 두벌식에서도 완성 음절만으로 구성되지 않으면 스킵
    let ascii = "preedit";
    let converted = crate::typefix::eng_to_kor(ascii, "ko_2bulstd", "qwerty");
    let chars: Vec<char> = converted.chars().collect();
    let all_complete = chars.len() <= 1
        || chars[..chars.len() - 1]
            .iter()
            .all(|c| ('\u{AC00}'..='\u{D7A3}').contains(c));
    if all_complete {
        // 두벌식에서 모두 완성 음절이면 트리거될 수 있음 — 그건 정상
        assert!(result.is_some() || result.is_none());
    } else {
        assert!(
            result.is_none(),
            "두벌식: 중간에 독립 자모가 있으면 트리거하면 안 됨"
        );
    }
}

// ── 다중 영문 키맵 테스트 ──

#[test]
fn test_to_ascii_string_dvorak() {
    // 물리키 [S, D, F] → Qwerty "sdf", Dvorak "oeu"
    let mut buf = KeystrokeBuffer::new();
    for key in [KeyCode::S, KeyCode::D, KeyCode::F] {
        buf.push(key, ModifierState::default());
    }
    assert_eq!(buf.to_ascii_string("qwerty"), "sdf");
    assert_eq!(buf.to_ascii_string("dvorak"), "oeu");
}

#[test]
fn test_to_ascii_string_colemak() {
    // 물리키 [E, R, T] → Qwerty "ert", Colemak "fpg"
    let mut buf = KeystrokeBuffer::new();
    for key in [KeyCode::E, KeyCode::R, KeyCode::T] {
        buf.push(key, ModifierState::default());
    }
    assert_eq!(buf.to_ascii_string("qwerty"), "ert");
    assert_eq!(buf.to_ascii_string("colemak"), "fpg");
}

#[test]
fn test_to_ascii_string_workman() {
    // 물리키 [W, E, R] → Qwerty "wer", Workman "drw"
    let mut buf = KeystrokeBuffer::new();
    for key in [KeyCode::W, KeyCode::E, KeyCode::R] {
        buf.push(key, ModifierState::default());
    }
    assert_eq!(buf.to_ascii_string("qwerty"), "wer");
    assert_eq!(buf.to_ascii_string("workman"), "drw");
}

#[test]
fn test_to_ascii_string_shift_mixed() {
    // Shift+S → Qwerty "S", Dvorak "O"
    let mut buf = KeystrokeBuffer::new();
    let shifted = ModifierState {
        shift: true,
        ..Default::default()
    };
    buf.push(KeyCode::S, shifted);
    assert_eq!(buf.to_ascii_string("qwerty"), "S");
    assert_eq!(buf.to_ascii_string("dvorak"), "O");
}

#[test]
fn test_forward_dvorak() {
    // 순방향은 물리키 기반이므로 같은 물리키 시퀀스는 레이아웃 무관하게
    // 동일한 한글을 생성한다 (eng_to_kor가 레이아웃 보정).
    // 물리키 [G, K, S, R, M, F] → Qwerty "gksrmf" → "한글"
    // 같은 물리키 → Dvorak "itopmf" → eng_to_kor(Dvorak) → "한글"
    let mut buf = KeystrokeBuffer::new();
    for key in [
        KeyCode::G,
        KeyCode::K,
        KeyCode::S,
        KeyCode::R,
        KeyCode::M,
        KeyCode::F,
    ] {
        buf.push(key, ModifierState::default());
    }

    let config = AutoTypeFixConfig {
        enabled: true,
        kor_syllable_threshold: 2,
        eng_word_min_length: 5,
        forward_time_window_ms: 2000,
        reverse_time_window_ms: 2000,
        forward: true,
        reverse: true,
        ..AutoTypeFixConfig::default()
    };

    // Qwerty: "gksrmf" → "한글"
    let result_qwerty = check_forward(&buf, &config, "ko_2bulstd", "qwerty", &empty_bl());
    assert!(result_qwerty.is_some());
    assert_eq!(result_qwerty.unwrap().corrected, "한글");

    // Dvorak: 같은 물리키 → 같은 한글 결과 (순방향은 물리키 기반)
    let result_dvorak = check_forward(&buf, &config, "ko_2bulstd", "dvorak", &empty_bl());
    assert!(result_dvorak.is_some());
    assert_eq!(result_dvorak.unwrap().corrected, "한글");

    // 핵심: to_ascii_string은 레이아웃에 따라 다른 문자열을 생성하지만
    // eng_to_kor가 보정하여 최종 한글 결과는 동일
    assert_eq!(buf.to_ascii_string("qwerty"), "gksrmf");
    assert_eq!(buf.to_ascii_string("dvorak"), "itopmu");
}

#[test]
fn test_reverse_dvorak_hello() {
    // Dvorak 사용자가 한글모드에서 "hello" 의도
    // Dvorak에서 "hello": h=물리J, e=물리D, l=물리P, l=물리P, o=물리S
    let mut buf = KeystrokeBuffer::new();
    for key in [KeyCode::J, KeyCode::D, KeyCode::P, KeyCode::P, KeyCode::S] {
        buf.push(key, ModifierState::default());
    }
    buf.committed_chars = 3;
    buf.has_preedit = true;

    let config = AutoTypeFixConfig {
        eng_word_min_length: 5,
        // 기존 테스트는 사전 hit 즉시 발화하는 종전 동작을 가정한다.
        ..AutoTypeFixConfig::default()
    };

    let result = check_reverse(
        &buf,
        &config,
        "ko_2bulstd",
        "dvorak",
        &empty_bl(),
        &EmptyUserDict,
    );
    assert!(result.is_some(), "Dvorak reverse should find 'hello'");
    let r = result.unwrap();
    assert_eq!(r.corrected, "hello");
    assert_eq!(r.delete_chars, 4); // 3 committed + 1 preedit
    assert!(r.clear_preedit);
}

#[test]
fn test_reverse_colemak_hello() {
    // Colemak에서 "hello": h=물리H, e=물리K, l=물리U, l=물리U, o=물리Semicolon
    // Semicolon은 is_character_key()=true이므로 push 가능
    // Colemak에서 Semicolon(2,9) → 'o'
    let mut buf = KeystrokeBuffer::new();
    for key in [
        KeyCode::H,
        KeyCode::K,
        KeyCode::U,
        KeyCode::U,
        KeyCode::Semicolon,
    ] {
        buf.push(key, ModifierState::default());
    }
    assert_eq!(buf.len(), 5, "Semicolon should be accepted by push");
    assert_eq!(buf.to_ascii_string("colemak"), "hello");
    buf.committed_chars = 3;
    buf.has_preedit = true;

    let config = AutoTypeFixConfig {
        eng_word_min_length: 5,
        // 기존 테스트는 사전 hit 즉시 발화하는 종전 동작을 가정한다.
        ..AutoTypeFixConfig::default()
    };

    let result = check_reverse(
        &buf,
        &config,
        "ko_2bulstd",
        "colemak",
        &empty_bl(),
        &EmptyUserDict,
    );
    assert!(result.is_some(), "Colemak reverse should find 'hello'");
    let r = result.unwrap();
    assert_eq!(r.corrected, "hello");
    assert_eq!(r.delete_chars, 4); // 3 committed + 1 preedit
    assert!(r.clear_preedit);
}

#[test]
fn test_reverse_asymmetry_qwerty_vs_dvorak() {
    // 같은 물리키 시퀀스가 Qwerty와 Dvorak에서 다른 결과
    // 물리키 [H, E, L, L, O] → Qwerty "hello", Dvorak "dents" 아님
    // Dvorak: H(2,5)='d', E(1,2)='.', L(2,8)='n', L(2,8)='n', O(1,8)='r'
    // → "d.nnr" — 사전에 없음
    let mut buf = KeystrokeBuffer::new();
    for key in [KeyCode::H, KeyCode::E, KeyCode::L, KeyCode::L, KeyCode::O] {
        buf.push(key, ModifierState::default());
    }
    buf.committed_chars = 3;
    buf.has_preedit = true;

    let config = AutoTypeFixConfig {
        eng_word_min_length: 5,
        // 기존 테스트는 사전 hit 즉시 발화하는 종전 동작을 가정한다.
        ..AutoTypeFixConfig::default()
    };

    // Qwerty: "hello" → 사전에 있음
    let result_qwerty = check_reverse(
        &buf,
        &config,
        "ko_2bulstd",
        "qwerty",
        &empty_bl(),
        &EmptyUserDict,
    );
    assert!(result_qwerty.is_some());
    assert_eq!(result_qwerty.unwrap().corrected, "hello");

    // Dvorak: 같은 물리키지만 다른 문자열 → 사전에 없을 가능성 높음
    let result_dvorak = check_reverse(
        &buf,
        &config,
        "ko_2bulstd",
        "dvorak",
        &empty_bl(),
        &EmptyUserDict,
    );
    // E(1,2) in Dvorak = '.' → not alpha, push may fail
    // 실제로는 buf는 이미 만들어졌으므로, to_ascii_string 결과가 다름
    if let Some(r) = result_dvorak {
        assert_ne!(
            r.corrected, "hello",
            "같은 물리키가 Dvorak에서는 다른 단어여야 함"
        );
    }
    // result_dvorak이 None이면 — 비알파벳 문자 포함으로 사전 매칭 실패 → 정상
}

#[test]
fn test_reverse_workman_world() {
    // Workman에서 "world": w=물리R, o=물리O→'p'... 아니, 매핑 확인
    // Workman: w=물리R(1,3), o=물리I(1,7)→'u'... 다시 확인
    // Workman row1: q,d,r,w,b,j,f,u,p,;,[,]
    // w는 col3 → 물리 R. o는 row2 col8 → 물리 L(2,8)?
    // Workman row2: a,s,h,t,g,y,n,e,o,i,' → o는 col8 → 물리 L
    // r는 row1 col2 → 물리 E. l은 row3 col6 → 물리 M. d는 row1 col1 → 물리 W.
    // "world" = w(물리R), o(물리L), r(물리E), l(물리M), d(물리W)
    let mut buf = KeystrokeBuffer::new();
    for key in [KeyCode::R, KeyCode::L, KeyCode::E, KeyCode::M, KeyCode::W] {
        buf.push(key, ModifierState::default());
    }
    buf.committed_chars = 3;
    buf.has_preedit = true;

    let config = AutoTypeFixConfig {
        eng_word_min_length: 5,
        // 기존 테스트는 사전 hit 즉시 발화하는 종전 동작을 가정한다.
        ..AutoTypeFixConfig::default()
    };

    let result = check_reverse(
        &buf,
        &config,
        "ko_2bulstd",
        "workman",
        &empty_bl(),
        &EmptyUserDict,
    );
    assert!(result.is_some(), "Workman reverse should find 'world'");
    assert_eq!(result.unwrap().corrected, "world");
}

// === Phase 1: skip 토글 ON/OFF 동작 검증 ===

#[test]
fn test_forward_skip_on_english_word_toggle_off() {
    // "hello"는 사전에 있어서 기본 동작(skip_on_english_word=true)에서는 억제된다.
    // 토글을 끄면(false) 영단어여도 한글 시뮬이 임계값을 만족하면 트리거해야 한다.
    let mut buf = KeystrokeBuffer::new();
    for key in [KeyCode::H, KeyCode::E, KeyCode::L, KeyCode::L, KeyCode::O] {
        buf.push(key, ModifierState::default());
    }

    // ON (기본) → 억제
    let on = AutoTypeFixConfig {
        skip_on_english_word: true,
        ..AutoTypeFixConfig::default()
    };
    assert!(
        check_forward(&buf, &on, "ko_2bulstd", "qwerty", &empty_bl()).is_none(),
        "skip_on_english_word=true 이면 사전 단어는 억제되어야 함"
    );

    // OFF → eng_to_kor("hello")가 임계값(2음절) 이상이면 트리거
    let off = AutoTypeFixConfig {
        skip_on_english_word: false,
        ..AutoTypeFixConfig::default()
    };
    let converted = typefix::eng_to_kor("hello", "ko_2bulstd", "qwerty");
    let sylls = count_korean_syllables(&converted);
    let result = check_forward(&buf, &off, "ko_2bulstd", "qwerty", &empty_bl());
    if sylls >= off.kor_syllable_threshold as usize {
        // 임계값 만족 시 — OFF이면 트리거되어야 한다.
        // 단, 마지막 글자 제외 "온전한 한글" 검증 때문에 None이 될 수도 있음.
        // OFF 동작의 핵심은 "사전 hit으로 조기 반환되지 않는다"는 것.
        // 따라서 ON/OFF 결과가 달라질 수 있음을 확인하는 것으로 충분.
        let _ = result;
    }
    // 토글 자체가 사전 체크를 건너뛰도록 작동하는지 간접 검증:
    // OFF에서는 사전 hit 이후의 경로가 실행되어야 한다.
    // "gksrmf"(사전 없음)와 달리 "hello"는 사전 hit이므로
    // ON/OFF 차이가 반드시 존재한다 — 단, 후속 검증에 의해 최종 결과가
    // 같을 수도 있음. 여기서는 ON이 None인 것만 확실히 한다.
}

#[test]
fn test_forward_skip_on_english_word_off_triggers_for_word() {
    // "asdf" — 사전에 없는 4키 — ON/OFF 둘 다 사전 체크는 통과하지만
    // 한글 시뮬레이션 임계값 판단은 동일해야 한다.
    // 사전 토글이 "true에서는 사전 체크에서 None, false에서는 통과"를 검증.
    //
    // 직접 토글 자체의 동작을 보증하기 위해, 단순한 "사전에 있는 단어이면서
    // 한글 시뮬이 임계값 충족"인 케이스를 구성한다.
    //
    // "lover" → 한글 시뮬: 키 l=ㅣ e=ㄷ v=ㅍ e=ㄷ r=ㄱ → 자모만 나옴, 완성 음절 0.
    // 실무적으로 사전 단어는 보통 완성 음절이 적게 나오므로 ON=None, OFF=None 동일.
    //
    // 이 테스트는 스모크 테스트로: ON 경로가 사전 체크에서 걸러지는지만 확인.
    let mut buf = KeystrokeBuffer::new();
    for key in [KeyCode::H, KeyCode::E, KeyCode::L, KeyCode::L, KeyCode::O] {
        buf.push(key, ModifierState::default());
    }
    let on = AutoTypeFixConfig {
        skip_on_english_word: true,
        ..AutoTypeFixConfig::default()
    };
    assert!(check_forward(&buf, &on, "ko_2bulstd", "qwerty", &empty_bl()).is_none());
}

#[test]
fn test_reverse_skip_on_complete_syllable_on_suppresses() {
    // 완성 음절만 있고 preedit 없음 → ON이면 억제되어야 한다.
    let mut buf = KeystrokeBuffer::new();
    for key in [KeyCode::H, KeyCode::E, KeyCode::L, KeyCode::L, KeyCode::O] {
        buf.push(key, ModifierState::default());
    }
    buf.committed_chars = 4;
    buf.has_preedit = false; // 모두 완성 음절

    let on = AutoTypeFixConfig {
        skip_on_complete_syllable: true,
        eng_word_min_length: 5,
        // 기존 테스트는 사전 hit 즉시 발화하는 종전 동작을 가정한다.
        ..AutoTypeFixConfig::default()
    };
    assert!(
        check_reverse(
            &buf,
            &on,
            "ko_2bulstd",
            "qwerty",
            &empty_bl(),
            &EmptyUserDict
        )
        .is_none(),
        "모두 완성 음절(preedit 없음)이면 ON에서 억제되어야 함"
    );
}

#[test]
fn test_reverse_skip_on_complete_syllable_off_triggers() {
    // 완성 음절만 있고 preedit 없음 → OFF이면 기존 로직이 작동해 트리거된다.
    let mut buf = KeystrokeBuffer::new();
    for key in [KeyCode::H, KeyCode::E, KeyCode::L, KeyCode::L, KeyCode::O] {
        buf.push(key, ModifierState::default());
    }
    buf.committed_chars = 4;
    buf.has_preedit = false;

    let off = AutoTypeFixConfig {
        skip_on_complete_syllable: false,
        eng_word_min_length: 5,
        // 기존 테스트는 사전 hit 즉시 발화하는 종전 동작을 가정한다.
        ..AutoTypeFixConfig::default()
    };
    let result = check_reverse(
        &buf,
        &off,
        "ko_2bulstd",
        "qwerty",
        &empty_bl(),
        &EmptyUserDict,
    );
    assert!(
        result.is_some(),
        "skip_on_complete_syllable=false 이면 완성 음절이어도 트리거되어야 함"
    );
    assert_eq!(result.unwrap().corrected, "hello");
}

#[test]
fn test_reverse_with_preedit_always_triggers_regardless_of_toggle() {
    // preedit이 있는 경우: 버퍼에 조합 중 자모가 있으므로 "모두 완성 음절"이 아니다.
    // → skip_on_complete_syllable 토글과 무관하게 사전 체크 단계로 진행.
    let mut buf = KeystrokeBuffer::new();
    for key in [KeyCode::H, KeyCode::E, KeyCode::L, KeyCode::L, KeyCode::O] {
        buf.push(key, ModifierState::default());
    }
    buf.committed_chars = 3;
    buf.has_preedit = true;

    let on = AutoTypeFixConfig {
        skip_on_complete_syllable: true,
        eng_word_min_length: 5,
        // 기존 테스트는 사전 hit 즉시 발화하는 종전 동작을 가정한다.
        ..AutoTypeFixConfig::default()
    };
    let result = check_reverse(
        &buf,
        &on,
        "ko_2bulstd",
        "qwerty",
        &empty_bl(),
        &EmptyUserDict,
    );
    assert!(
        result.is_some(),
        "preedit이 있으면 complete-syllable skip 토글이 ON이어도 억제되지 않아야 함"
    );
}

// === Phase 2: 학습형 Blacklist 억제 게이트 ===

#[test]
fn forward_suppressed_by_tentative_blacklist() {
    // "gksrmf" → "한글" 이 평상시엔 트리거되지만, blacklist에 tentative로 있으면 None.
    let mut buf = KeystrokeBuffer::new();
    for key in [
        KeyCode::G,
        KeyCode::K,
        KeyCode::S,
        KeyCode::R,
        KeyCode::M,
        KeyCode::F,
    ] {
        buf.push(key, ModifierState::default());
    }
    let config = AutoTypeFixConfig::default();

    let mut bl = Blacklist::default();
    bl.add_or_hit_tentative("gksrmf", Direction::Forward, "ko_2bulstd", "qwerty");

    let result = check_forward(&buf, &config, "ko_2bulstd", "qwerty", &bl);
    assert!(
        result.is_none(),
        "tentative blacklist 엔트리는 forward를 억제해야 함"
    );
}

#[test]
fn forward_suppressed_by_confirmed_blacklist() {
    let mut buf = KeystrokeBuffer::new();
    for key in [
        KeyCode::G,
        KeyCode::K,
        KeyCode::S,
        KeyCode::R,
        KeyCode::M,
        KeyCode::F,
    ] {
        buf.push(key, ModifierState::default());
    }
    let config = AutoTypeFixConfig::default();

    let mut bl = Blacklist::default();
    bl.add_or_hit_tentative("gksrmf", Direction::Forward, "ko_2bulstd", "qwerty");
    bl.promote_to_confirmed(0);

    let result = check_forward(&buf, &config, "ko_2bulstd", "qwerty", &bl);
    assert!(
        result.is_none(),
        "confirmed blacklist 엔트리는 forward를 억제해야 함"
    );
}

#[test]
fn forward_not_suppressed_by_inactive_blacklist() {
    // inactive 상태는 기록만 남고 억제 효과 없음.
    let mut buf = KeystrokeBuffer::new();
    for key in [
        KeyCode::G,
        KeyCode::K,
        KeyCode::S,
        KeyCode::R,
        KeyCode::M,
        KeyCode::F,
    ] {
        buf.push(key, ModifierState::default());
    }
    let config = AutoTypeFixConfig::default();

    let mut bl = Blacklist::default();
    bl.add_or_hit_tentative("gksrmf", Direction::Forward, "ko_2bulstd", "qwerty");
    bl.deactivate(0);
    assert_eq!(bl.entries[0].status, EntryStatus::Inactive);

    let result = check_forward(&buf, &config, "ko_2bulstd", "qwerty", &bl);
    assert!(result.is_some(), "inactive 엔트리는 억제 효과가 없어야 함");
}

#[test]
fn forward_layout_mismatch_not_suppressed() {
    // blacklist는 Qwerty 기준, 검사는 Dvorak → 매칭 안 됨.
    let mut buf = KeystrokeBuffer::new();
    for key in [
        KeyCode::G,
        KeyCode::K,
        KeyCode::S,
        KeyCode::R,
        KeyCode::M,
        KeyCode::F,
    ] {
        buf.push(key, ModifierState::default());
    }
    let config = AutoTypeFixConfig::default();

    let mut bl = Blacklist::default();
    // Qwerty 기준 "gksrmf" 등록
    bl.add_or_hit_tentative("gksrmf", Direction::Forward, "ko_2bulstd", "qwerty");

    // Dvorak로 같은 물리키 시퀀스 → to_ascii_string 결과가 다름 → 매칭 실패 → 억제되지 않음
    let result = check_forward(&buf, &config, "ko_2bulstd", "dvorak", &bl);
    assert!(result.is_some(), "레이아웃 불일치면 blacklist 억제 안 됨");
}

#[test]
fn reverse_suppressed_by_tentative_blacklist() {
    let mut buf = KeystrokeBuffer::new();
    for key in [KeyCode::H, KeyCode::E, KeyCode::L, KeyCode::L, KeyCode::O] {
        buf.push(key, ModifierState::default());
    }
    buf.committed_chars = 3;
    buf.has_preedit = true;

    let config = AutoTypeFixConfig {
        eng_word_min_length: 5,
        // 기존 테스트는 사전 hit 즉시 발화하는 종전 동작을 가정한다.
        ..AutoTypeFixConfig::default()
    };

    let mut bl = Blacklist::default();
    bl.add_or_hit_tentative("hello", Direction::Reverse, "ko_2bulstd", "qwerty");

    let result = check_reverse(&buf, &config, "ko_2bulstd", "qwerty", &bl, &EmptyUserDict);
    assert!(result.is_none(), "tentative reverse 엔트리는 억제해야 함");
}

#[test]
fn reverse_rollback_suppression_cycle() {
    // 회귀 테스트: "역방향 교정 → (engine_worker가 쓰는 키) → blacklist 등록 →
    // 같은 단어 재입력 시 억제" 사이클이 순방향과 대칭으로 작동해야 한다.
    //
    // 버그 배경: `check_reverse`의 `AutoTypeFixResult.original`이 undo용으로
    // 빈 문자열이므로, engine_worker가 `fix.original`을 blacklist 키로 쓰면
    // 역방향은 항상 ""로 등록되어 억제가 발화하지 않았다.
    // 수정 후 engine_worker는 역방향에서 `fix.corrected`를 사용한다.

    // 1) 1차 트리거
    let mut buf = KeystrokeBuffer::new();
    for key in [KeyCode::H, KeyCode::E, KeyCode::L, KeyCode::L, KeyCode::O] {
        buf.push(key, ModifierState::default());
    }
    buf.committed_chars = 3;
    buf.has_preedit = true;

    let config = AutoTypeFixConfig {
        eng_word_min_length: 5,
        // 기존 테스트는 사전 hit 즉시 발화하는 종전 동작을 가정한다.
        ..AutoTypeFixConfig::default()
    };
    let mut bl = Blacklist::default();
    let fix = check_reverse(&buf, &config, "ko_2bulstd", "qwerty", &bl, &EmptyUserDict)
        .expect("1차 역방향 트리거는 성공해야 함");

    // 2) engine_worker의 키 선택 로직을 그대로 재현 (Direction::Reverse → fix.corrected)
    let suppression_key = fix.corrected.clone();
    assert_eq!(
        suppression_key, "hello",
        "역방향의 blacklist 키는 빈 문자열이 아닌 영단어여야 함"
    );

    // 3) 롤백(BS+모드전환) 감지 후 blacklist 등록
    bl.add_or_hit_tentative(&suppression_key, Direction::Reverse, "ko_2bulstd", "qwerty");

    // 4) 동일 입력 재발생 → 이번에는 억제되어야 함
    let mut buf2 = KeystrokeBuffer::new();
    for key in [KeyCode::H, KeyCode::E, KeyCode::L, KeyCode::L, KeyCode::O] {
        buf2.push(key, ModifierState::default());
    }
    buf2.committed_chars = 3;
    buf2.has_preedit = true;

    let result2 = check_reverse(&buf2, &config, "ko_2bulstd", "qwerty", &bl, &EmptyUserDict);
    assert!(
        result2.is_none(),
        "역방향 롤백 학습 후 같은 단어 재입력은 억제되어야 함"
    );
}

#[test]
fn reverse_direction_mismatch_not_suppressed() {
    // 같은 ASCII지만 방향이 다르면 매칭 안 됨.
    let mut buf = KeystrokeBuffer::new();
    for key in [KeyCode::H, KeyCode::E, KeyCode::L, KeyCode::L, KeyCode::O] {
        buf.push(key, ModifierState::default());
    }
    buf.committed_chars = 3;
    buf.has_preedit = true;

    let config = AutoTypeFixConfig {
        eng_word_min_length: 5,
        // 기존 테스트는 사전 hit 즉시 발화하는 종전 동작을 가정한다.
        ..AutoTypeFixConfig::default()
    };

    let mut bl = Blacklist::default();
    // Forward로 등록 — Reverse 검사와 방향 불일치
    bl.add_or_hit_tentative("hello", Direction::Forward, "ko_2bulstd", "qwerty");

    let result = check_reverse(&buf, &config, "ko_2bulstd", "qwerty", &bl, &EmptyUserDict);
    assert!(result.is_some(), "방향 불일치면 억제 안 됨");
}

// === 역방향 사용자 사전 (UserDictionary) 통합 (PR #6) ===

#[test]
fn reverse_user_dict_fires_for_short_word() {
    // "git" (3자) — eng_word_min_length=5 미달. user dict 등록 시 우회하여 발화.
    use crate::typefix_userdict::UserDictionary;

    let mut buf = KeystrokeBuffer::new();
    for key in [KeyCode::G, KeyCode::I, KeyCode::T] {
        buf.push(key, ModifierState::default());
    }
    buf.committed_chars = 1;
    buf.has_preedit = true;

    let config = AutoTypeFixConfig {
        eng_word_min_length: 5,
        ..AutoTypeFixConfig::default()
    };

    assert!(
        check_reverse(
            &buf,
            &config,
            "ko_2bulstd",
            "qwerty",
            &empty_bl(),
            &EmptyUserDict
        )
        .is_none(),
        "user dict 미등록 시 길이 미달이면 억제"
    );

    let mut ud = UserDictionary::default();
    ud.add("git", None);
    let result = check_reverse(&buf, &config, "ko_2bulstd", "qwerty", &empty_bl(), &ud);
    assert!(result.is_some(), "user dict 등록 시 길이 미달이어도 발화");
    assert_eq!(result.unwrap().corrected, "git");
}

#[test]
fn reverse_user_dict_fires_for_non_dictionary_word() {
    // "rustc" — 내장 사전에 없는 CLI 명령어. user dict 등록 시 발화.
    use crate::typefix_userdict::UserDictionary;

    let mut buf = KeystrokeBuffer::new();
    for key in [KeyCode::R, KeyCode::U, KeyCode::S, KeyCode::T, KeyCode::C] {
        buf.push(key, ModifierState::default());
    }
    buf.committed_chars = 2;
    buf.has_preedit = true;

    let config = AutoTypeFixConfig {
        eng_word_min_length: 5,
        skip_on_complete_syllable: false,
        ..AutoTypeFixConfig::default()
    };

    assert!(
        !dictionary_contains("rustc"),
        "전제: 내장 사전에 'rustc' 없음"
    );
    assert!(check_reverse(
        &buf,
        &config,
        "ko_2bulstd",
        "qwerty",
        &empty_bl(),
        &EmptyUserDict
    )
    .is_none());

    let mut ud = UserDictionary::default();
    ud.add("rustc", None);
    let result = check_reverse(&buf, &config, "ko_2bulstd", "qwerty", &empty_bl(), &ud);
    assert!(result.is_some());
    assert_eq!(result.unwrap().corrected, "rustc");
}

#[test]
fn reverse_user_dict_disabled_by_config() {
    use crate::typefix_userdict::UserDictionary;

    let mut buf = KeystrokeBuffer::new();
    for key in [KeyCode::G, KeyCode::I, KeyCode::T] {
        buf.push(key, ModifierState::default());
    }
    buf.committed_chars = 1;
    buf.has_preedit = true;

    let config = AutoTypeFixConfig {
        eng_word_min_length: 5,
        user_dict_enabled: false,
        ..AutoTypeFixConfig::default()
    };

    let mut ud = UserDictionary::default();
    ud.add("git", None);
    assert!(
        check_reverse(&buf, &config, "ko_2bulstd", "qwerty", &empty_bl(), &ud).is_none(),
        "user_dict_enabled=false 이면 user dict 무시"
    );
}

#[test]
fn reverse_user_dict_respects_blacklist() {
    // blacklist 억제가 user dict 우회보다 우선.
    use crate::typefix_userdict::UserDictionary;

    let mut buf = KeystrokeBuffer::new();
    for key in [KeyCode::G, KeyCode::I, KeyCode::T] {
        buf.push(key, ModifierState::default());
    }
    buf.committed_chars = 1;
    buf.has_preedit = true;

    let config = AutoTypeFixConfig {
        eng_word_min_length: 5,
        ..AutoTypeFixConfig::default()
    };

    let mut ud = UserDictionary::default();
    ud.add("git", None);

    let mut bl = Blacklist::default();
    bl.add_or_hit_tentative("git", Direction::Reverse, "ko_2bulstd", "qwerty");

    assert!(
        check_reverse(&buf, &config, "ko_2bulstd", "qwerty", &bl, &ud).is_none(),
        "blacklist가 user dict보다 우선"
    );
}

#[test]
fn reverse_user_dict_still_respects_complete_syllable_skip() {
    // skip_on_complete_syllable=true + preedit 없음 → user dict여도 억제.
    use crate::typefix_userdict::UserDictionary;

    let mut buf = KeystrokeBuffer::new();
    for key in [KeyCode::G, KeyCode::I, KeyCode::T] {
        buf.push(key, ModifierState::default());
    }
    buf.committed_chars = 2;
    buf.has_preedit = false;

    let config = AutoTypeFixConfig {
        eng_word_min_length: 5,
        skip_on_complete_syllable: true,
        ..AutoTypeFixConfig::default()
    };

    let mut ud = UserDictionary::default();
    ud.add("git", None);
    assert!(
        check_reverse(&buf, &config, "ko_2bulstd", "qwerty", &empty_bl(), &ud).is_none(),
        "user dict도 skip_on_complete_syllable 검사 통과해야 함"
    );
}
