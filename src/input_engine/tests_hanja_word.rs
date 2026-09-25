//! 한자 단어 입력(HANJA_WORD_SPEC) 엔진 테스트.
//!
//! U8 단계 스모크(불변식 1개)에 이어, U10 이 §6.1 케이스 표 전량을 채운다. config
//! serde 왕복(필드 없는 YAML → `Hanja`, `HanjaHangul` 이 Compat 경유로 살아남는지)은
//! U1 에서 이미 `src/config.rs` 에 추가돼 있으므로 여기서 중복하지 않는다.
//!
//! 두벌식 키열 "대한민국" = `E O G K S A L S R N R` (대=E,O / 한=G,K,S / 민=A,L,S / 국=R,N,R).
//! 검증된 부분 매핑: R=ㄱ, K=ㅏ, N=ㅜ, S=ㄴ (기존 tests_scenarios.rs·이 파일 스모크 테스트로
//! 교차 확인됨).

use super::test_helpers::create_test_engine;
use super::{HanjaSource, InputEngine, InputResult, PopupAction, RECENT_SYLLABLE_CAP};
use crate::config::{Config, ContentPurpose, HanjaOutputFormat, InputCategory};
use crate::hangul::composer::JamoMeta;
use crate::hangul::jamo::{Cho, JamoEnum, Jong, Jung};
use crate::keycode::{KeyCode, ModifierState};

fn type_keys(engine: &mut InputEngine, config: &Config, keys: &[KeyCode]) {
    for &k in keys {
        engine.press_key(k, ModifierState::default(), config);
    }
}

const DAEHANMINGUK: [KeyCode; 11] = [
    KeyCode::E,
    KeyCode::O,
    KeyCode::G,
    KeyCode::K,
    KeyCode::S,
    KeyCode::A,
    KeyCode::L,
    KeyCode::S,
    KeyCode::R,
    KeyCode::N,
    KeyCode::R,
];

/// 불변식: 다음절 일치 없음 ∧ 형식 `Hanja` ∧ preedit 1자 ⇒ `InputResult`·commit_buffer·
/// `get_hanja_target()` 이 종전과 바이트 동일.
///
/// 호스트 능력 플래그 기본값(false — IMM32·`unim-capi`)에서는 "대한민"+"국" 도 버퍼 포함
/// 접미를 채택하지 않으므로 다음절 일치가 없다 → 종전 단음절 경로(확정·취소 모두).
/// (§6.1 대상① "호스트 플래그 false" 케이스를 겸한다.)
#[test]
fn smoke_invariant_single_syllable_byte_identical() {
    let config = Config::default();
    let m = ModifierState::default();

    // 확정 경로
    let mut e = create_test_engine();
    e.set_input_category(InputCategory::Korean);
    type_keys(&mut e, &config, &DAEHANMINGUK);
    assert_eq!(e.commit_str(), "대한민");
    assert_eq!(e.preedit_str(), "국");

    let r = e.press_key(KeyCode::F9, m, &config);
    assert_eq!(r, InputResult::hanja_candidates());
    assert_eq!(e.get_hanja_target(), "국");
    assert_eq!(e.hanja_source(), HanjaSource::Syllable);
    assert_eq!(e.commit_str(), "대한민");
    let first = e.get_hanja_candidates()[0].0.clone();

    let r = e.press_key(KeyCode::Num1, m, &config);
    assert_eq!(r, InputResult::committed());
    assert_eq!(e.commit_str(), format!("대한민{first}"));
    assert_eq!(e.commit_str(), "대한민國", "사전 첫 후보 리터럴 — 첫 후보 변수와의 동어반복 방지");
    assert_eq!(e.preedit_str(), "");
    assert!(e.take_hanja_replacement().is_none());
    assert!(!e.is_hanja_mode());

    // 취소 경로 — Escape 는 target("국")을 그대로 재커밋
    let mut e = create_test_engine();
    e.set_input_category(InputCategory::Korean);
    type_keys(&mut e, &config, &DAEHANMINGUK);
    e.press_key(KeyCode::F9, m, &config);
    let r = e.press_key(KeyCode::Escape, m, &config);
    assert_eq!(r, InputResult::committed());
    assert_eq!(e.commit_str(), "대한민국");
    assert!(!e.is_hanja_mode());
}

// =============================================================================
// 대상① — 음절 확정 모드
// =============================================================================

/// "대한민"(커밋)+"국"(preedit) + 호스트 플래그 true 상태를 만든다. 정합성 검증
/// 케이스들이 이 위에 `set_surrounding_text`/`set_surrounding_includes_preedit` 만
/// 얹어 F9 를 누른다.
fn setup_recent_word_pending(config: &Config) -> InputEngine {
    let mut e = create_test_engine();
    e.set_input_category(InputCategory::Korean);
    e.set_hanja_word_replace_capable(true);
    type_keys(&mut e, config, &DAEHANMINGUK);
    assert_eq!(e.commit_str(), "대한민");
    assert_eq!(e.preedit_str(), "국");
    e
}

/// 대상①-정상: "대한민"+"국" → target "대한민국"·확정 접두 3. surrounding 을 전혀 주지
/// 않은 컨텍스트이므로 "정합성 검증 | surrounding 비어 있음(통과)" 케이스도 겸한다.
#[test]
fn recent_word_target_matches_and_verifies_with_no_surrounding() {
    let config = Config::default();
    let mut e = setup_recent_word_pending(&config);

    let r = e.press_key(KeyCode::F9, ModifierState::default(), &config);
    assert_eq!(r, InputResult::hanja_candidates());
    assert_eq!(e.get_hanja_target(), "대한민국");
    assert_eq!(e.hanja_source(), HanjaSource::RecentWord);
    assert_eq!(e.hanja_committed_chars(), 3);
}

/// 대상①-불일치 폴백: 버퍼가 "뷁"(사전에 없는 음절)이면 "뷁국" 도 사전에 없어 종전대로
/// 마지막 음절 "국" 으로 폴백한다.
#[test]
fn recent_word_mismatch_falls_back_to_last_syllable() {
    let config = Config::default();
    let mut e = create_test_engine();
    e.set_input_category(InputCategory::Korean);
    e.set_hanja_word_replace_capable(true);
    e.recent_push_char('뷁');
    type_keys(&mut e, &config, &[KeyCode::R, KeyCode::N, KeyCode::R]); // "국"
    assert_eq!(e.preedit_str(), "국");

    let r = e.press_key(KeyCode::F9, ModifierState::default(), &config);
    assert_eq!(r, InputResult::hanja_candidates());
    assert_eq!(e.get_hanja_target(), "국");
    assert_eq!(e.hanja_source(), HanjaSource::Syllable);
}

/// 대상①-미완성 자모 폴백: 초성만 있으면(완성 음절 아님) 종전 규칙(마지막 글자) — 한자
/// 후보가 없으면 초성 특수문자로 전환된다(§2.2.2 1). 호스트 플래그 true + 버퍼에 완성
/// 음절이 쌓여 있어도("대한민") 단어 기능(RecentWord) 대상이 되지 않는다는 불변식을
/// 명시적으로 검증한다.
#[test]
fn incomplete_jamo_falls_back_to_legacy_single_char_target() {
    let config = Config::default();
    let mut e = create_test_engine();
    e.set_input_category(InputCategory::Korean);
    e.set_hanja_word_replace_capable(true);
    for c in "대한민".chars() {
        e.recent_push_char(c);
    }
    e.press_key(KeyCode::R, ModifierState::default(), &config); // "ㄱ" 초성만
    assert_eq!(e.preedit_str(), "ㄱ");

    let r = e.press_key(KeyCode::F9, ModifierState::default(), &config);
    assert_ne!(e.hanja_source(), HanjaSource::RecentWord, "미완성 자모는 단어 기능 대상이 아니다");
    assert_eq!(e.hanja_committed_chars(), 0);
    // "ㄱ" 은 한자 사전에 없어 특수문자 후보로 귀결된다 — 리셋 조건 테스트(818행 부근)와
    // 동일하게 결정적이므로 분기 없이 직접 단언한다.
    assert!(r.special_char_candidates_available, "미완성 자모 \"ㄱ\" → 특수문자 후보");
    assert_eq!(e.get_special_char_target(), "ㄱ");
}

/// 대상①-확정 페이로드: `Some{delete_chars:3, preedit_chars:1, text}`, `commit_buffer`
/// 는 비고, `HidePopup` 이 발행된다(교체 채널 — commit_buffer 에 넣지 않는다).
#[test]
fn recent_word_confirmation_leaves_replacement_payload_and_empty_commit_buffer() {
    let config = Config::default();
    let mut e = setup_recent_word_pending(&config);
    e.press_key(KeyCode::F9, ModifierState::default(), &config);
    let first = e.get_hanja_candidates()[0].0.clone();

    let r = e.press_key(KeyCode::Num1, ModifierState::default(), &config);
    assert_eq!(r, InputResult::preedit_updated());
    // "대한민" 은 진입 전 이미 커밋된 것 — 교체는 여기 텍스트를 추가하지 않는다(페이로드로만 나간다).
    assert_eq!(e.commit_str(), "대한민", "대상① 교체는 commit_buffer 에 텍스트를 넣지 않는다");

    let rep = e.take_hanja_replacement().expect("교체 페이로드가 있어야 한다");
    assert_eq!(rep.delete_chars, 3);
    assert_eq!(rep.preedit_chars, 1);
    assert_eq!(rep.text, first);
    assert_eq!(rep.text, "大韓民國", "사전 첫 후보 리터럴 — 첫 후보 변수와의 동어반복 방지");

    assert!(matches!(e.take_popup_action(), Some(PopupAction::HidePopup)));
}

/// 대상①-키보드/직접 호출 두 경로: `press_key(F9)`(push) 와 `start_hanja_conversion()`
/// 직접 호출(pull — Qt/XIM `GetHanjaCandidates`)이 동일 버퍼 상태에서 같은 결론을 낸다.
#[test]
fn keyboard_and_direct_call_paths_agree_on_target() {
    let config = Config::default();

    // A: press_key(F9) — push 경로.
    let mut a = create_test_engine();
    a.set_input_category(InputCategory::Korean);
    a.set_hanja_word_replace_capable(true);
    type_keys(&mut a, &config, &DAEHANMINGUK);
    let ra = a.press_key(KeyCode::F9, ModifierState::default(), &config);

    // B: start_hanja_conversion() 직접 호출 — pull 경로. 동일 입력 시퀀스.
    let mut b = create_test_engine();
    b.set_input_category(InputCategory::Korean);
    b.set_hanja_word_replace_capable(true);
    type_keys(&mut b, &config, &DAEHANMINGUK);
    let rb = b.start_hanja_conversion();

    assert_eq!(ra, rb, "두 경로의 InputResult 가 같아야 한다");
    assert_eq!(a.get_hanja_target(), b.get_hanja_target());
    assert_eq!(a.hanja_source(), b.hanja_source());
    assert_eq!(a.hanja_committed_chars(), b.hanja_committed_chars());
    assert_eq!(a.get_hanja_candidates(), b.get_hanja_candidates(), "후보 목록도 동일해야 한다");

    assert_eq!(a.get_hanja_target(), "대한민국");
    assert_eq!(a.hanja_source(), HanjaSource::RecentWord);
    assert_eq!(a.hanja_committed_chars(), 3);
}

/// 대상①-chord: "대한"만 이미 확정(버퍼 시드), "민"은 (앞서 확정된 게 아니라) chord로
/// 막 조합돼 preedit 에 떠 있는 상태, "국"은 chord 버퍼에 새로 대기 중일 때 한자키 →
/// `finalize_chord_buffer()` 가 "국"을 preedit 으로 inject 하기 *직전*, 한자키 분기의
/// `recent_absorb_commit_delta()` 가 이미 확정된 "민"(방금 자신이 만든 커밋)을 pool 에
/// 흡수해야 한다 — 안 하면 pool 이 "대한"+"국" == "한국"(확정 접두 1)이 되어 "민"을
/// 오삭제한다(§2.2.1 chord 흡수 주석). 그래서 target 은 "대한민국"·확정 접두 3 이어야
/// 하고, "국"은 그대로 preedit 에 남아 있어야 한다(chord 대기분은 preedit 진입만, 아직
/// 확정 아님).
#[test]
fn chord_hanja_key_absorbs_pending_syllable_into_target() {
    let mut config = Config::default();
    config.engine.korean.layout = "ko_3bul_anmatae".to_string();
    config.engine.korean.bidirectional_combine = Some(true);
    config.engine.korean.chord_window_ms = Some(5000);
    let mut e = InputEngine::new(&config);
    e.set_input_category(InputCategory::Korean);
    e.set_hanja_word_replace_capable(true);

    // "대한"만 이미 앱에 확정된 것으로 시드 — "민"은 아직 커밋되지 않은 상태로 남겨서
    // 한자키 자신이 확정시키는 흡수 경로를 실제로 타게 한다.
    for c in "대한".chars() {
        e.recent_push_char(c);
    }

    // "민"(ㅁ+ㅣ+ㄴ)을 chord 창 안에 누적한 뒤 idle flush 로 조합 중 preedit 에 반영한다
    // (아직 확정 아님 — 진짜 "방금 확정한 걸 한자키가 흡수" 시나리오를 만들기 위함).
    e.chord_buffer.push_jamo(JamoEnum::Cho(Cho::M), JamoMeta::default());
    e.chord_buffer.push_jamo(JamoEnum::Jung(Jung::I), JamoMeta::default());
    e.chord_buffer.push_jamo(JamoEnum::Jong(Jong::N), JamoMeta::default());
    e.chord_idle_flush_pending();
    assert_eq!(e.preedit_str(), "민", "민이 조합 중 preedit 으로 반영돼야 한다");

    // "국"(ㄱ+ㅜ+ㄱ) chord 대기 — flush 전(push_jamo 만, 아직 finalize 안 함).
    e.chord_buffer.push_jamo(JamoEnum::Cho(Cho::G), JamoMeta::default());
    e.chord_buffer.push_jamo(JamoEnum::Jung(Jung::U), JamoMeta::default());
    e.chord_buffer.push_jamo(JamoEnum::Jong(Jong::G), JamoMeta::default());
    assert!(e.chord_pending_info().is_some(), "chord 버퍼 활성 (국 대기)");

    let r = e.press_key(KeyCode::F9, ModifierState::default(), &config);
    assert_eq!(r, InputResult::hanja_candidates());
    assert_eq!(e.preedit_str(), "국", "F9 가 finalize_chord_buffer 로 국을 preedit 에 inject");
    assert_eq!(e.get_hanja_target(), "대한민국");
    assert_eq!(e.hanja_source(), HanjaSource::RecentWord);
    assert_eq!(e.hanja_committed_chars(), 3);
}

// =============================================================================
// 정합성 검증(안전망) — §2.2.3
// =============================================================================

/// GTK 형: surrounding 의 커서 앞 텍스트가 "…대한민" 으로 끝나면 통과.
#[test]
fn prefix_verification_passes_gtk_style_full_before() {
    let config = Config::default();
    let mut e = setup_recent_word_pending(&config);
    let before = "안녕하세요 대한민";
    let n = before.chars().count() as u32;
    e.set_surrounding_text(before.to_string(), n, n);

    e.press_key(KeyCode::F9, ModifierState::default(), &config);
    assert_eq!(e.get_hanja_target(), "대한민국");
}

/// 잘린 형: 앱이 커서 앞을 "민" 한 글자만 보고해도(위젯이 잘라 보고), "대한민" 이 그
/// 접미이므로 통과한다.
#[test]
fn prefix_verification_passes_truncated_before() {
    let config = Config::default();
    let mut e = setup_recent_word_pending(&config);
    e.set_surrounding_text("민".to_string(), 1, 1);

    e.press_key(KeyCode::F9, ModifierState::default(), &config);
    assert_eq!(e.get_hanja_target(), "대한민국");
}

/// 불일치: surrounding 이 "xyz" 면 접두 대조가 실패 → 종전(마지막 음절) "국" 으로 퇴화.
#[test]
fn prefix_verification_fails_on_mismatched_surrounding() {
    let config = Config::default();
    let mut e = setup_recent_word_pending(&config);
    e.set_surrounding_text("xyz".to_string(), 3, 3);

    e.press_key(KeyCode::F9, ModifierState::default(), &config);
    assert_eq!(e.get_hanja_target(), "국", "불일치 surrounding → 단음절 퇴화");
}

/// §2.2.3: 접두 검증이 한 번 실패하면 더 짧은 버퍼 포함 접미도 믿지 않는다.
/// surrounding "abc민" 은 "대한민"(대한민국) 검증엔 실패하지만 "민"(민국) 검증엔 통과한다.
/// 이때 "민국" 으로 내려가면 드리프트를 반쯤 믿는 셈이라 마지막 음절 "국" 으로 퇴화해야 한다
/// (`recent_untrusted` — 이 분기를 지우면 target 이 "민국" 이 된다).
#[test]
fn prefix_verification_failure_distrusts_shorter_buffer_suffixes() {
    let config = Config::default();
    let mut e = setup_recent_word_pending(&config);
    e.set_surrounding_text("abc민".to_string(), 4, 4);

    e.press_key(KeyCode::F9, ModifierState::default(), &config);
    assert_eq!(e.get_hanja_target(), "국", "더 짧은 버퍼 접미(민국)로 내려가지 않는다");
    assert_eq!(e.hanja_committed_chars(), 0);
}

/// 커서 0(줄 첫머리): `before` 가 빈 문자열이면 무조건 실패(공허 통과 금지) → "국" 퇴화.
#[test]
fn prefix_verification_fails_on_cursor_zero_empty_before() {
    let config = Config::default();
    let mut e = setup_recent_word_pending(&config);
    e.set_surrounding_text("대한민국 만세".to_string(), 0, 0);

    e.press_key(KeyCode::F9, ModifierState::default(), &config);
    assert_eq!(e.get_hanja_target(), "국", "커서 0 → before 빈 문자열 → 퇴화");
}

/// TSF 형(`prefix+pre` 절): `surrounding_includes_preedit` 기본값 false 에서는 조합
/// 텍스트가 surrounding 에 포함돼 있어도(=Wayland/GNOME 클릭 드리프트와 동형) 실패해야
/// 하고, 그 플래그를 true 로 켠 TSF 에서만 통과해야 한다.
#[test]
fn prefix_verification_tsf_style_requires_flag() {
    let config = Config::default();

    // 플래그 false(기본) — 실패, "국" 으로 퇴화.
    let mut e = setup_recent_word_pending(&config);
    e.set_surrounding_text("대한민국".to_string(), 4, 4);
    e.press_key(KeyCode::F9, ModifierState::default(), &config);
    assert_eq!(e.get_hanja_target(), "국", "surrounding_includes_preedit=false → 퇴화");

    // 같은 입력 + 플래그 true — 통과.
    let mut e2 = setup_recent_word_pending(&config);
    e2.set_surrounding_includes_preedit(true);
    e2.set_surrounding_text("대한민국".to_string(), 4, 4);
    e2.press_key(KeyCode::F9, ModifierState::default(), &config);
    assert_eq!(e2.get_hanja_target(), "대한민국", "surrounding_includes_preedit=true → 통과");
}

/// 커서 오프셋이 문자 길이를 넘으면(단위 불일치) 무조건 실패 — 클램프 금지.
///
/// 두 번째 케이스가 핵심: surrounding "대한민"(3자) + 커서 5 는, 만약 구현이 커서를
/// 길이로 클램프한다면 `before` 가 "대한민" 그대로가 되어 접두 "대한민" 과 정확히
/// 일치 → 통과("대한민국")해 버린다. 클램프 없이 fail-closed 라면 오프셋 불일치로
/// 실패해 "국" 으로 퇴화해야 한다 — 이 입력이 클램프 유무를 가른다.
#[test]
fn prefix_verification_fails_when_cursor_exceeds_length() {
    let config = Config::default();
    let mut e = setup_recent_word_pending(&config);
    e.set_surrounding_text("대한민국".to_string(), 5, 0);

    e.press_key(KeyCode::F9, ModifierState::default(), &config);
    assert_eq!(e.get_hanja_target(), "국", "오프셋 초과 → 퇴화");

    // 클램프하면 오히려 통과해 버리는 함정 입력 — 반드시 실패(퇴화)해야 한다.
    let mut e2 = setup_recent_word_pending(&config);
    e2.set_surrounding_text("대한민".to_string(), 5, 5);

    e2.press_key(KeyCode::F9, ModifierState::default(), &config);
    assert_eq!(
        e2.get_hanja_target(),
        "국",
        "클램프하면 before==\"대한민\"==prefix 로 통과해 버리는 함정 — 클램프 금지로 실패해야 한다"
    );
}

/// Q9(b) — `surrounding_seen` 3단계 전이: 한 번도 안 받음(통과) / 받은 뒤 빈 값(퇴화) /
/// `reset()` 은 보존(여전히 퇴화) / 엔진 재생성만 false 로 복귀(다시 통과).
#[test]
fn surrounding_seen_three_stage_transition() {
    let config = Config::default();
    let m = ModifierState::default();

    // 한 번도 안 받음 — 검증 생략(통과).
    let mut e = setup_recent_word_pending(&config);
    e.press_key(KeyCode::F9, m, &config);
    assert_eq!(e.get_hanja_target(), "대한민국", "surrounding 미수신 → 통과");

    // 받은 뒤 빈 값 — 실패(퇴화).
    e = setup_recent_word_pending(&config);
    e.set_surrounding_text(String::new(), 0, 0);
    e.press_key(KeyCode::F9, m, &config);
    assert_eq!(e.get_hanja_target(), "국", "수신 후 빈 값 → 퇴화(Q9b)");

    // reset() 은 surrounding_seen 을 보존한다 — 재조합해도 여전히 퇴화.
    e.reset();
    e.set_input_category(InputCategory::Korean);
    e.set_hanja_word_replace_capable(true);
    type_keys(&mut e, &config, &DAEHANMINGUK);
    e.press_key(KeyCode::F9, m, &config);
    assert_eq!(e.get_hanja_target(), "국", "reset() 은 surrounding_seen 보존 → 여전히 퇴화");

    // 엔진 재생성만 surrounding_seen 을 false 로 되돌린다.
    let mut fresh = InputEngine::new(&config);
    fresh.set_input_category(InputCategory::Korean);
    fresh.set_hanja_word_replace_capable(true);
    type_keys(&mut fresh, &config, &DAEHANMINGUK);
    fresh.press_key(KeyCode::F9, m, &config);
    assert_eq!(fresh.get_hanja_target(), "대한민국", "엔진 재생성 → surrounding_seen=false → 통과");
}

// =============================================================================
// 버퍼 — 상한·Backspace·chord idle flush·수정자
// =============================================================================

/// 상한 17자 — 초과분은 앞에서 버린다.
#[test]
fn recent_buffer_caps_at_17_and_drops_oldest() {
    let mut e = create_test_engine();
    let chars: Vec<char> = (0u32..20).map(|i| char::from_u32(0xAC00 + i).unwrap()).collect();
    for &c in &chars {
        e.recent_push_char(c);
    }
    let expected: String = chars[3..].iter().collect(); // 마지막 17자
    assert_eq!(e.recent_syllables().chars().count(), RECENT_SYLLABLE_CAP);
    assert_eq!(e.recent_syllables(), expected);
}

/// Backspace 통과(조합 없음, 선택 없음) → 버퍼 끝 1자 pop.
#[test]
fn backspace_pass_through_pops_last_buffer_syllable() {
    let config = Config::default();
    let mut e = create_test_engine();
    e.set_input_category(InputCategory::Korean);
    for c in "대한민국".chars() {
        e.recent_push_char(c);
    }
    assert_eq!(e.recent_syllables(), "대한민국");

    e.press_key(KeyCode::Backspace, ModifierState::default(), &config);
    assert_eq!(e.recent_syllables(), "대한민", "Backspace 통과 → 버퍼 끝 1자 pop");
}

/// Backspace 통과 + 선택 영역이 있었으면(cursor != anchor) 버퍼 전체를 비운다.
#[test]
fn backspace_pass_through_with_selection_clears_buffer() {
    let config = Config::default();
    let mut e = create_test_engine();
    e.set_input_category(InputCategory::Korean);
    for c in "대한민국".chars() {
        e.recent_push_char(c);
    }
    e.set_surrounding_text("x".to_string(), 1, 0); // cursor != anchor → 선택 있음

    e.press_key(KeyCode::Backspace, ModifierState::default(), &config);
    assert!(e.recent_syllables().is_empty(), "선택 있으면 Backspace 통과 시 버퍼 전체 clear");
}

/// chord idle 만료(`chord_idle_flush_pending`, 래퍼 밖 훅)도 새로 커밋된 델타를 버퍼에
/// 흡수한다 — 타이머 경로는 `press_key` 를 거치지 않으므로 U8 이 스모크로만 검증했다.
#[test]
fn chord_idle_flush_pending_absorbs_committed_delta_into_recent_buffer() {
    let mut config = Config::default();
    config.engine.korean.layout = "ko_3bul_anmatae".to_string();
    config.engine.korean.bidirectional_combine = Some(true);
    config.engine.korean.chord_window_ms = Some(5000);
    let mut e = InputEngine::new(&config);
    e.set_input_category(InputCategory::Korean);

    // 이미 커밋된 "가"(직전 키 처리분) + chord 버퍼에 새 자모 1개 대기 상태를 시뮬레이션.
    e.commit_buffer.push('가');
    e.chord_buffer.push_jamo(JamoEnum::Cho(Cho::G), JamoMeta::default());
    let (commit, _preedit) = e.chord_idle_flush_pending();
    assert_eq!(commit.as_deref(), Some("가"));
    assert_eq!(e.recent_syllables(), "가", "타이머 경로도 최근 확정 음절 버퍼를 채운다");
}

/// 비밀번호 차단 중에는 chord idle flush 가 commit 을 반환하더라도 버퍼를 채우지 않는다
/// (`recent_push_char` 자체의 fail-closed).
#[test]
fn chord_idle_flush_does_not_fill_buffer_while_password_blocked() {
    let mut config = Config::default();
    config.engine.korean.layout = "ko_3bul_anmatae".to_string();
    config.engine.korean.bidirectional_combine = Some(true);
    config.engine.korean.chord_window_ms = Some(5000);
    let mut e = InputEngine::new(&config);
    e.set_input_category(InputCategory::Korean);
    e.set_content_purpose(ContentPurpose::Password);

    e.commit_buffer.push('가');
    e.chord_buffer.push_jamo(JamoEnum::Cho(Cho::G), JamoMeta::default());
    let (commit, _preedit) = e.chord_idle_flush_pending();
    assert_eq!(commit.as_deref(), Some("가"));
    assert!(
        e.recent_syllables().is_empty(),
        "비밀번호 차단 중엔 chord idle flush 도 버퍼를 채우지 않는다"
    );
}

/// 수정자 단독 키(예: LeftShift)는 리셋 조건에서 제외된다.
#[test]
fn lone_modifier_key_does_not_reset_buffer() {
    let config = Config::default();
    let mut e = create_test_engine();
    e.set_input_category(InputCategory::Korean);
    for c in "가나".chars() {
        e.recent_push_char(c);
    }
    e.press_key(KeyCode::LeftShift, ModifierState::default(), &config);
    assert_eq!(e.recent_syllables(), "가나", "수정자 단독 키는 버퍼를 비우지 않는다");
}

// =============================================================================
// 오프셋 방어
// =============================================================================

/// `set_surrounding_text("가나", 5, 3)` 뒤 한자키 → 패닉 없이 선택 없음으로 처리되고
/// idle 이모지로 폴백한다(범위 초과 거부 + a≥b 조기 반환).
#[test]
fn selection_offset_exceeding_length_is_rejected_without_panic() {
    let config = Config::default();
    let mut e = create_test_engine();
    e.set_surrounding_text("가나".to_string(), 5, 3);

    e.press_key(KeyCode::F9, ModifierState::default(), &config);
    assert!(!e.is_hanja_mode());
    assert!(e.is_emoji_popup_active(), "범위 초과 → 선택 없음 취급 → idle 이모지 폴백");
}

/// `("대한민국", 0, 12)` — 클램프했다면 "대한민국" 팝업이 떴을 입력이지만, 오프셋
/// 단위 불일치로 반드시 거부돼야 한다.
#[test]
fn selection_span_clamping_would_be_wrong_is_rejected() {
    let config = Config::default();
    let mut e = create_test_engine();
    e.set_surrounding_text("대한민국".to_string(), 0, 12);

    e.press_key(KeyCode::F9, ModifierState::default(), &config);
    assert!(!e.is_hanja_mode(), "클램프하면 '대한민국' 팝업이 뜬다 — 반드시 거부");
    assert!(e.is_emoji_popup_active());
}

/// `typefix_convert` 도 같은 판정(범위 초과 거부)을 공유한다.
#[test]
fn typefix_convert_rejects_out_of_range_offset_without_panic() {
    let mut e = create_test_engine();
    e.set_surrounding_text("가나".to_string(), 5, 3);
    assert!(e.typefix_convert(0).is_none());
}

/// 화이트박스: `selection_span` 의 a≥b 조기 반환(빈 선택 거부)과 커서/앵커 swap 허용을
/// 직접 검증한다. 18자 상한(`HANJA_MAX_KEY_CHARS`)은 19자 문자열이 사전에도 없어
/// 사전 미스와 구분이 안 되므로 대상②의 사전 미스 폴백 4종 테스트(아래
/// `selection_rejects_non_dictionary_and_falls_back_to_idle_emoji`)와 별도로 떼어
/// 검증하지 않는다 — 그쪽 주석 참조.
#[test]
fn selection_span_rejects_empty_range_and_normalizes_swapped_offsets() {
    let mut e = create_test_engine();

    e.set_surrounding_text("가나".to_string(), 1, 1);
    assert_eq!(e.selection_span(), None, "cursor==anchor → 선택 없음(a≥b 조기 반환)");

    e.set_surrounding_text("가나".to_string(), 2, 0);
    assert_eq!(e.selection_span(), Some((0, 2)), "cursor>anchor 도 (min,max) 로 정규화");
}

// =============================================================================
// 팝업 중 키
// =============================================================================

/// 팝업 중 키(PageDown·즐겨찾기 토글 Space·펼치기 토글 Period) → 래퍼 (2) `was_popup`
/// 이 셋 다 버퍼를 clear 한다 — target·확정 접두는 진입 시 이미 확정돼 무해. Space 는
/// 즐겨찾기를 토글하므로(실 파일 IO) 검증 뒤 한 번 더 눌러 원복한다.
#[test]
fn buffer_clears_on_in_popup_keys() {
    let config = Config::default();
    let m = ModifierState::default();
    for key in [KeyCode::PageDown, KeyCode::Space, KeyCode::Period] {
        let mut e = create_test_engine();
        e.set_input_category(InputCategory::Korean);
        type_keys(&mut e, &config, &DAEHANMINGUK);
        e.press_key(KeyCode::F9, m, &config);
        assert!(e.is_hanja_mode());
        assert!(!e.recent_syllables().is_empty(), "{key:?} 이전 전제조건: 버퍼가 채워져 있어야 한다");

        e.press_key(key, m, &config);
        assert!(e.recent_syllables().is_empty(), "{key:?} 팝업 중 키 후 버퍼 clear");

        if key == KeyCode::Space {
            e.press_key(KeyCode::Space, m, &config); // 즐겨찾기 토글 원복(실 파일 오염 방지)
        }
    }
}

// =============================================================================
// 팝업 중 한자키 재타 — Q7(a) target 접미 축소
// =============================================================================

/// 한자키 재타로 target 접미가 단계적으로 축소된다: "대한민국"(확정3) → "민국"(확정1)
/// → "국"(확정0·`Syllable`).
#[test]
fn hanja_key_repeat_shrinks_target_suffix_while_popup_open() {
    let config = Config::default();
    let mut e = setup_recent_word_pending(&config);
    let m = ModifierState::default();

    let r = e.press_key(KeyCode::F9, m, &config);
    assert_eq!(r, InputResult::hanja_candidates());
    assert_eq!(e.get_hanja_target(), "대한민국");
    assert_eq!(e.hanja_committed_chars(), 3);

    let r = e.press_key(KeyCode::F9, m, &config);
    assert_eq!(r, InputResult::hanja_candidates());
    assert_eq!(e.get_hanja_target(), "민국", "1차 축소 — 대한민국 → 민국");
    assert_eq!(e.hanja_committed_chars(), 1);
    assert_eq!(e.hanja_source(), HanjaSource::RecentWord);

    // 1차 축소 직후 확정 — 교체 페이로드가 축소된 대상 기준으로 나가야 한다
    // (delete_chars=확정접두 1, preedit_chars=진입 시 preedit 1자, text="民國").
    let sel = e.press_key(KeyCode::Num1, m, &config);
    assert_eq!(sel, InputResult::preedit_updated());
    let rep = e.take_hanja_replacement().expect("축소 후 확정도 교체 페이로드가 있어야 한다");
    assert_eq!(rep.delete_chars, 1);
    assert_eq!(rep.preedit_chars, 1);
    assert_eq!(rep.text, "民國", "축소된 대상 \"민국\" 의 한자 확정");

    // 별도 엔진 — 1차 축소 후 2차 축소까지 이어감을 검증.
    let mut e = setup_recent_word_pending(&config);
    e.press_key(KeyCode::F9, m, &config);
    e.press_key(KeyCode::F9, m, &config);
    let r = e.press_key(KeyCode::F9, m, &config);
    assert_eq!(r, InputResult::hanja_candidates());
    assert_eq!(e.get_hanja_target(), "국", "2차 축소 — 민국 → 국");
    assert_eq!(e.hanja_committed_chars(), 0);
    assert_eq!(e.hanja_source(), HanjaSource::Syllable);
}

/// 단어 모드(`commit_unit=Word`)에서도 한자키 재타로 target 접미가 축소된다.
/// "오늘대한민국" → F9(대한민국·WordBuffer) → F9(축소: 민국·WordBuffer) → 확정하면
/// 접두 "오늘대한" 은 보존되고 축소된 "민국" 만 한자로 바뀐다("오늘대한民國").
/// 취소(Esc)는 항상 원본 preedit 전체를 그대로 재커밋한다.
#[test]
fn hanja_key_repeat_shrinks_word_buffer_target_and_confirms_with_prefix_preserved() {
    let config = Config::default();
    let m = ModifierState::default();

    let mut e = create_test_engine();
    e.set_input_category(InputCategory::Korean);
    e.set_word_mode(true);
    e.preedit_cache = "오늘대한민국".to_string();

    e.press_key(KeyCode::F9, m, &config);
    assert_eq!(e.get_hanja_target(), "대한민국");
    assert_eq!(e.hanja_source(), HanjaSource::WordBuffer);

    let r = e.press_key(KeyCode::F9, m, &config);
    assert_eq!(r, InputResult::hanja_candidates());
    assert_eq!(e.get_hanja_target(), "민국", "단어 모드 1차 축소 — 대한민국 → 민국");
    assert_eq!(e.hanja_source(), HanjaSource::WordBuffer);

    let sel = e.press_key(KeyCode::Num1, m, &config);
    assert_eq!(sel, InputResult::committed());
    assert_eq!(e.commit_str(), "오늘대한民國", "축소된 \"민국\" 만 한자로, 나머지 접두는 보존");

    // 별도 엔진 — 취소는 축소 여부와 무관하게 preedit 전체를 그대로 재커밋한다.
    let mut e2 = create_test_engine();
    e2.set_input_category(InputCategory::Korean);
    e2.set_word_mode(true);
    e2.preedit_cache = "오늘대한민국".to_string();
    e2.press_key(KeyCode::F9, m, &config);
    e2.press_key(KeyCode::F9, m, &config);
    let r2 = e2.press_key(KeyCode::Escape, m, &config);
    assert_eq!(r2, InputResult::committed());
    assert_eq!(e2.commit_str(), "오늘대한민국", "축소 중 취소해도 preedit 전체 재커밋");
}

/// 축소 불가 경로 — 대상②(Selection)는 target 이 그대로 재개(popup_cancel + 즉시
/// 재개, 대상②는 여전히 같은 선택 영역을 본다)되고, 1자 target(`Syllable`)은 취소
/// 재커밋 뒤 idle 이모지로 폴백한다. 둘 다 `shrink_hanja_target` 이 `None` 을 돌려
/// `process_popup_key`(미지원 키 재처리)로 떨어진 결과다.
#[test]
fn hanja_key_repeat_falls_through_when_not_shrinkable() {
    let config = Config::default();
    let m = ModifierState::default();

    // 대상②(Selection).
    let mut e = create_test_engine();
    e.set_surrounding_text("대한민국".to_string(), 0, 4);
    e.press_key(KeyCode::F9, m, &config);
    assert_eq!(e.hanja_source(), HanjaSource::Selection);

    let r = e.press_key(KeyCode::F9, m, &config);
    assert_eq!(r, InputResult::hanja_candidates());
    assert!(e.is_hanja_mode());
    assert_eq!(e.get_hanja_target(), "대한민국", "Selection 은 축소하지 않고 그대로 재개");
    assert_eq!(e.hanja_source(), HanjaSource::Selection);

    // 1자 target(Syllable).
    let mut e2 = create_test_engine();
    e2.set_input_category(InputCategory::Korean);
    type_keys(&mut e2, &config, &DAEHANMINGUK);
    e2.press_key(KeyCode::F9, m, &config);
    assert_eq!(e2.get_hanja_target(), "국");
    assert_eq!(e2.hanja_source(), HanjaSource::Syllable);

    let r2 = e2.press_key(KeyCode::F9, m, &config);
    assert_eq!(r2, InputResult::consumed());
    assert!(!e2.is_hanja_mode());
    assert!(e2.is_emoji_popup_active(), "1자 target 취소 후 idle 이모지로 폴백");
    assert_eq!(e2.commit_str(), "대한민국", "취소 재커밋 국 이 대한민 뒤에 붙는다");
}

/// SPEC:72 "팝업 열림·축소 불가 대상·pull: 같은 팝업 재발행" — pull 경로(`GetHanjaCandidates`
/// = `start_hanja_conversion` 직접 호출, push 의 `press_key(F9)` 를 거치지 않음)로 팝업이
/// 이미 열린 상태에서 다시 호출하면, 엔진은 `start_hanja_conversion` 첫머리의 "이미
/// 한자 모드이면 무시" 가드에 걸려 `consumed()` 를 돌려주고 target·팝업 상태는 그대로
/// 유지한다(재판정·재축소 없음). "재발행" 을 이 무변화 유지로 해석한 결과이며, 이 결과가
/// SPEC 문구와 어긋난다고 보면 `open_issues` 로 넘긴다.
#[test]
fn pull_path_second_call_while_popup_open_leaves_non_shrinkable_target_unchanged() {
    let config = Config::default();
    let mut e = create_test_engine();
    e.set_input_category(InputCategory::Korean);
    type_keys(&mut e, &config, &DAEHANMINGUK);

    let r1 = e.start_hanja_conversion();
    assert_eq!(r1, InputResult::hanja_candidates());
    assert_eq!(e.get_hanja_target(), "국");
    assert_eq!(e.hanja_source(), HanjaSource::Syllable);
    e.take_popup_action(); // 최초 ShowHanja 드레인 — 이 테스트의 관심사는 재호출 쪽.

    let r2 = e.start_hanja_conversion();
    assert_eq!(r2, InputResult::consumed(), "이미 한자 모드 — 재판정 없이 무시");
    assert!(e.is_hanja_mode(), "팝업은 열린 채로 유지된다");
    assert_eq!(e.get_hanja_target(), "국", "대상 불변(축소 없음)");
    assert!(e.take_popup_action().is_none(), "재호출은 새 ShowHanja 액션을 내보내지 않는다");
}

/// pull 경로(`GetHanjaCandidates` → `start_hanja_conversion`: Qt·GTK4·GTK3 X11·XIM)의
/// 한자키 재타도 push 와 같이 대상① 접미를 축소한다(SPEC:72 Q7(a) — 경로 구분 없음).
#[test]
fn pull_path_second_call_while_popup_open_shrinks_recent_word_target() {
    let config = Config::default();
    let mut e = setup_recent_word_pending(&config);

    assert_eq!(e.start_hanja_conversion(), InputResult::hanja_candidates());
    assert_eq!(e.get_hanja_target(), "대한민국");
    e.take_popup_action();

    assert_eq!(e.start_hanja_conversion(), InputResult::hanja_candidates());
    assert_eq!(e.get_hanja_target(), "민국", "pull 재호출 — 대한민국 → 민국");
    assert_eq!(e.hanja_committed_chars(), 1);
    assert!(e.is_hanja_mode());
}

// =============================================================================
// 리셋 조건 — 파라미터화
// =============================================================================

/// §2.2.1 리셋 조건표의 대표 지점들을 한 번에 훑는다(≈13 지점).
#[test]
fn buffer_resets_on_documented_conditions() {
    let config = Config::default();

    // Space — 비한글 커밋(공백).
    {
        let mut e = create_test_engine();
        e.set_input_category(InputCategory::Korean);
        for c in "가나".chars() {
            e.recent_push_char(c);
        }
        e.press_key(KeyCode::Space, ModifierState::default(), &config);
        assert!(e.recent_syllables().is_empty(), "Space 커밋 후 버퍼 리셋");
    }
    // Enter — 앱으로 통과.
    {
        let mut e = create_test_engine();
        e.set_input_category(InputCategory::Korean);
        for c in "가나".chars() {
            e.recent_push_char(c);
        }
        e.press_key(KeyCode::Enter, ModifierState::default(), &config);
        assert!(e.recent_syllables().is_empty(), "Enter 통과 후 버퍼 리셋");
    }
    // Tab — 앱으로 통과.
    {
        let mut e = create_test_engine();
        e.set_input_category(InputCategory::Korean);
        for c in "가나".chars() {
            e.recent_push_char(c);
        }
        e.press_key(KeyCode::Tab, ModifierState::default(), &config);
        assert!(e.recent_syllables().is_empty(), "Tab 통과 후 버퍼 리셋");
    }
    // Escape — 앱으로 통과.
    {
        let mut e = create_test_engine();
        e.set_input_category(InputCategory::Korean);
        for c in "가나".chars() {
            e.recent_push_char(c);
        }
        e.press_key(KeyCode::Escape, ModifierState::default(), &config);
        assert!(e.recent_syllables().is_empty(), "Escape 통과 후 버퍼 리셋");
    }
    // Left / Home / Delete — 내비게이션 통과 키.
    for key in [KeyCode::Left, KeyCode::Home, KeyCode::Delete] {
        let mut e = create_test_engine();
        e.set_input_category(InputCategory::Korean);
        for c in "가나".chars() {
            e.recent_push_char(c);
        }
        e.press_key(key, ModifierState::default(), &config);
        assert!(e.recent_syllables().is_empty(), "{key:?} 통과 후 버퍼 리셋");
    }
    // Ctrl+A — 단축키 조합.
    {
        let mut e = create_test_engine();
        e.set_input_category(InputCategory::Korean);
        for c in "가나".chars() {
            e.recent_push_char(c);
        }
        let m = ModifierState {
            control: true,
            ..Default::default()
        };
        e.press_key(KeyCode::A, m, &config);
        assert!(e.recent_syllables().is_empty(), "Ctrl+A 후 버퍼 리셋");
    }
    // 토글키(한/영 전환).
    {
        let mut e = create_test_engine();
        e.set_input_category(InputCategory::Korean);
        for c in "가나".chars() {
            e.recent_push_char(c);
        }
        e.press_key(KeyCode::Korean, ModifierState::default(), &config);
        assert!(e.recent_syllables().is_empty(), "한/영 전환 후 버퍼 리셋");
    }
    // 영문자 커밋(비한글).
    {
        let mut e = create_test_engine();
        e.set_input_category(InputCategory::English);
        for c in "가나".chars() {
            e.recent_push_char(c);
        }
        e.press_key(KeyCode::A, ModifierState::default(), &config);
        assert!(e.recent_syllables().is_empty(), "영문 커밋 후 버퍼 리셋");
    }
    // 쉼표/숫자/기호 커밋(영문 모드).
    {
        let mut e = create_test_engine();
        e.set_input_category(InputCategory::English);
        for c in "가나".chars() {
            e.recent_push_char(c);
        }
        e.press_key(KeyCode::Comma, ModifierState::default(), &config);
        assert!(e.recent_syllables().is_empty(), "쉼표 커밋 후 버퍼 리셋");
    }
    // 이모지 확정(팝업).
    {
        let mut e = create_test_engine();
        e.set_input_category(InputCategory::Korean);
        for c in "가나".chars() {
            e.recent_push_char(c);
        }
        e.press_key(KeyCode::F9, ModifierState::default(), &config); // idle, 선택 없음 → 이모지 팝업
        assert!(e.is_emoji_popup_active());
        e.press_key(KeyCode::Num1, ModifierState::default(), &config); // 이모지 확정
        assert!(e.recent_syllables().is_empty(), "이모지 확정 후 버퍼 리셋");
    }
    // 한자 확정(팝업).
    {
        let mut e = create_test_engine();
        e.set_input_category(InputCategory::Korean);
        type_keys(&mut e, &config, &DAEHANMINGUK);
        assert!(!e.recent_syllables().is_empty());
        e.press_key(KeyCode::F9, ModifierState::default(), &config);
        e.press_key(KeyCode::Num1, ModifierState::default(), &config);
        assert!(e.recent_syllables().is_empty(), "한자 확정 후 버퍼 리셋");
    }
    // reset().
    {
        let mut e = create_test_engine();
        for c in "가나".chars() {
            e.recent_push_char(c);
        }
        e.reset();
        assert!(e.recent_syllables().is_empty(), "reset() 후 버퍼 리셋");
    }
    // set_input_category(실제 변경).
    {
        let mut e = create_test_engine();
        e.set_input_category(InputCategory::Korean);
        for c in "가나".chars() {
            e.recent_push_char(c);
        }
        e.set_input_category(InputCategory::English);
        assert!(e.recent_syllables().is_empty(), "카테고리 전환 후 버퍼 리셋");
    }
    // 비밀번호 진입.
    {
        let mut e = create_test_engine();
        e.set_input_category(InputCategory::Korean);
        for c in "가나".chars() {
            e.recent_push_char(c);
        }
        e.set_content_purpose(ContentPurpose::Password);
        assert!(e.recent_syllables().is_empty(), "비밀번호 진입 후 버퍼 리셋");
    }
    // 한국어 모드 쉼표/숫자 커밋 — 영문 모드(위 "쉼표/숫자/기호 커밋")와 동일 경로.
    for key in [KeyCode::Comma, KeyCode::Num1] {
        let mut e = create_test_engine();
        e.set_input_category(InputCategory::Korean);
        for c in "가나".chars() {
            e.recent_push_char(c);
        }
        e.press_key(key, ModifierState::default(), &config);
        assert!(e.recent_syllables().is_empty(), "한국어 모드 {key:?} 커밋 후 버퍼 리셋");
    }
    // 특수문자 자모(keyboard_map JamoEnum::Special) 직접 커밋 — 조합을 거치지 않는
    // 비한글 확정도 리셋 조건이다(§2.2.1).
    {
        let mut config2 = Config::default();
        config2.engine.korean.layout = "ko_anmatae".to_string();
        let mut e = InputEngine::new(&config2);
        e.set_input_category(InputCategory::Korean);
        for c in "가나".chars() {
            e.recent_push_char(c);
        }
        let shift = ModifierState {
            shift: true,
            ..Default::default()
        };
        e.press_key(KeyCode::Q, shift, &config2); // 안마태 Shift+Q → 「(특수문자 자모)
        assert_eq!(e.commit_str(), "\u{300C}");
        assert!(e.recent_syllables().is_empty(), "특수문자 자모 커밋 후 버퍼 리셋");
    }
    // 특수문자 팝업 확정 — 미완성 자모 "ㄱ" → F9 특수문자 팝업 → Num1 확정.
    {
        let mut e = create_test_engine();
        e.set_input_category(InputCategory::Korean);
        for c in "가나".chars() {
            e.recent_push_char(c);
        }
        e.press_key(KeyCode::R, ModifierState::default(), &config); // "ㄱ" 초성만
        let r = e.press_key(KeyCode::F9, ModifierState::default(), &config);
        assert!(r.special_char_candidates_available, "초성만 있으면 특수문자 후보가 있어야 한다");
        e.press_key(KeyCode::Num1, ModifierState::default(), &config);
        assert!(e.recent_syllables().is_empty(), "특수문자 확정 후 버퍼 리셋");
    }
}

// =============================================================================
// 단어 확정 모드(`commit_unit=Word`)
// =============================================================================

/// 단어 모드-정상: "오늘대한민국" → target "대한민국"·접두 "오늘" 보존, 확정은
/// 접두+한자를 한 번에 커밋한다.
#[test]
fn word_mode_matches_full_preedit_with_committed_prefix_preserved() {
    let config = Config::default();
    let mut e = create_test_engine();
    e.set_input_category(InputCategory::Korean);
    e.set_word_mode(true);
    e.preedit_cache = "오늘대한민국".to_string();

    let r = e.press_key(KeyCode::F9, ModifierState::default(), &config);
    assert_eq!(r, InputResult::hanja_candidates());
    assert_eq!(e.get_hanja_target(), "대한민국");
    assert_eq!(e.hanja_source(), HanjaSource::WordBuffer);
    assert_eq!(e.hanja_committed_chars(), 0);

    let first = e.get_hanja_candidates()[0].0.clone();
    let sel = e.press_key(KeyCode::Num1, ModifierState::default(), &config);
    assert_eq!(sel, InputResult::committed());
    assert_eq!(e.commit_str(), format!("오늘{first}"));
    assert_eq!(e.commit_str(), "오늘大韓民國", "사전 첫 후보 리터럴 — 첫 후보 변수와의 동어반복 방지");
}

/// 단어 모드-취소: preedit 전체("오늘대한민국")를 그대로 재커밋한다(§2.3 결함 수정 —
/// preedit 2자 이상에서 앞부분이 사라지지 않는다).
#[test]
fn word_mode_cancel_recommits_full_preedit() {
    let config = Config::default();
    let mut e = create_test_engine();
    e.set_input_category(InputCategory::Korean);
    e.set_word_mode(true);
    e.preedit_cache = "오늘대한민국".to_string();

    e.press_key(KeyCode::F9, ModifierState::default(), &config);
    let r = e.press_key(KeyCode::Escape, ModifierState::default(), &config);
    assert_eq!(r, InputResult::committed());
    assert_eq!(e.commit_str(), "오늘대한민국");
}

/// 단어 모드-불일치 폴백에서도 접두를 보존한다: "오늘국" 은 "오늘국"·"늘국" 이 사전에
/// 없어 마지막 음절 "국" 으로 폴백하지만, "오늘" 은 여전히 커밋 접두로 남는다.
#[test]
fn word_mode_fallback_preserves_prefix_when_no_dict_suffix_matches() {
    let config = Config::default();
    let mut e = create_test_engine();
    e.set_input_category(InputCategory::Korean);
    e.set_word_mode(true);
    e.preedit_cache = "오늘국".to_string();

    e.press_key(KeyCode::F9, ModifierState::default(), &config);
    assert_eq!(e.get_hanja_target(), "국");
    assert_eq!(e.hanja_source(), HanjaSource::WordBuffer);

    let first = e.get_hanja_candidates()[0].0.clone();
    e.press_key(KeyCode::Num1, ModifierState::default(), &config);
    assert_eq!(e.commit_str(), format!("오늘{first}"), "불일치 폴백에서도 접두 보존");
    assert_eq!(e.commit_str(), "오늘國", "사전 첫 후보 리터럴 — 첫 후보 변수와의 동어반복 방지");
}

// =============================================================================
// 대상② — 선택 영역
// =============================================================================

/// 대상②-정상: 선택 "대한민국" → target "대한민국", 확정은 일반 커밋(위젯이 선택을
/// 치환) — 교체 페이로드 없음.
#[test]
fn selection_target_matches_full_word_and_confirms_via_general_commit() {
    let config = Config::default();
    let mut e = create_test_engine();
    e.set_surrounding_text("대한민국".to_string(), 0, 4);
    assert!(e.has_selection());

    let r = e.press_key(KeyCode::F9, ModifierState::default(), &config);
    assert_eq!(r, InputResult::hanja_candidates());
    assert_eq!(e.get_hanja_target(), "대한민국");
    assert_eq!(e.hanja_source(), HanjaSource::Selection);
    assert_eq!(e.hanja_committed_chars(), 0);

    let first = e.get_hanja_candidates()[0].0.clone();
    e.press_key(KeyCode::Num1, ModifierState::default(), &config);
    assert_eq!(e.commit_str(), first, "대상② 확정은 일반 커밋(위젯이 선택을 치환)");
    assert_eq!(e.commit_str(), "大韓民國", "사전 첫 후보 리터럴 — 첫 후보 변수와의 동어반복 방지");
    assert!(e.take_hanja_replacement().is_none(), "대상② 는 교체 페이로드 없음");
}

/// 대상②-공백 보존(Q3): 앞뒤 공백은 판정에서 제외되지만 확정 시 되붙인다.
#[test]
fn selection_target_preserves_surrounding_whitespace() {
    let config = Config::default();
    let mut e = create_test_engine();
    e.set_surrounding_text(" 대한민국 ".to_string(), 0, 6);

    e.press_key(KeyCode::F9, ModifierState::default(), &config);
    assert_eq!(e.get_hanja_target(), "대한민국");

    let first = e.get_hanja_candidates()[0].0.clone();
    e.press_key(KeyCode::Num1, ModifierState::default(), &config);
    assert_eq!(e.commit_str(), format!(" {first} "));
    assert_eq!(e.commit_str(), " 大韓民國 ", "사전 첫 후보 리터럴 — 첫 후보 변수와의 동어반복 방지");
}

/// 대상②-거부 4종(불일치·비한글·19자·공백만) → idle 이모지로 폴백(Q8(a)), 선택 텍스트는
/// 건드리지 않는다. "가"*19 는 18자 상한(`HANJA_MAX_KEY_CHARS`) 가드와 사전 미스가
/// 같은 결과(NoMatchSelection)로 수렴해 이 테스트만으로는 어느 쪽이 걸렸는지 구분할 수
/// 없다 — 상한 자체의 조기 반환은 `selection_span_rejects_empty_range_and_normalizes_swapped_offsets`
/// 옆에 남긴 주석대로 별도로 떼어 검증하지 않는다(19자 사전 단어가 존재하지 않는 한
/// 분리 불가능).
#[test]
fn selection_rejects_non_dictionary_and_falls_back_to_idle_emoji() {
    let config = Config::default();
    let cases = ["뷁뷁", "abcd", &"가".repeat(19), "   "];
    for sel in cases {
        let mut e = create_test_engine();
        e.set_surrounding_text(sel.to_string(), 0, sel.chars().count() as u32);
        e.press_key(KeyCode::F9, ModifierState::default(), &config);
        assert!(!e.is_hanja_mode(), "{sel:?} 는 한자 팝업으로 이어지면 안 된다");
        assert!(e.is_emoji_popup_active(), "{sel:?} → idle 이모지 폴백(Q8a)");
    }
}

/// 선택 없음 + idle → 이모지 팝업(종전 v3.2, 회귀 없음).
#[test]
fn idle_no_selection_opens_emoji_popup() {
    let config = Config::default();
    let mut e = create_test_engine();
    e.press_key(KeyCode::F9, ModifierState::default(), &config);
    assert!(e.is_emoji_popup_active());
    assert!(!e.is_hanja_mode());
}

/// 조합 중이면 선택을 무시하고 대상①(조합 중 음절)을 우선한다(§2.4.4, 모호성 제거).
#[test]
fn composing_preedit_takes_priority_over_selection() {
    let config = Config::default();
    let mut e = create_test_engine();
    e.set_input_category(InputCategory::Korean);
    e.set_surrounding_text("대한민국".to_string(), 0, 4);
    e.press_key(KeyCode::R, ModifierState::default(), &config);
    e.press_key(KeyCode::K, ModifierState::default(), &config); // "가" 조합 중
    assert_eq!(e.preedit_str(), "가");
    assert!(e.is_composing());

    e.press_key(KeyCode::F9, ModifierState::default(), &config);
    assert_eq!(e.get_hanja_target(), "가", "조합 중이면 대상① 우선, 선택 무시");
    assert_ne!(e.hanja_source(), HanjaSource::Selection);
}

/// 대상②-취소: 재커밋 텍스트가 없으므로(Selection `recommit=""`) 취소해도 아무것도
/// 커밋되지 않는다.
#[test]
fn selection_cancel_does_not_recommit_anything() {
    let config = Config::default();
    let mut e = create_test_engine();
    e.set_surrounding_text("대한민국".to_string(), 0, 4);
    e.press_key(KeyCode::F9, ModifierState::default(), &config);
    assert!(e.is_hanja_mode());

    e.press_key(KeyCode::Escape, ModifierState::default(), &config);
    assert!(e.commit_str().is_empty(), "대상② 취소는 재커밋 없음(recommit 빈 문자열)");
    assert!(!e.is_hanja_mode());
}

// =============================================================================
// 출력 형식
// =============================================================================

/// 서식 3종 × 단음절 확정 — `select_hanja` 한 곳에서 조립되므로 키보드 경로만
/// 대표로 검증한다. 기대값은 `format.render()` 호출이 아니라 리터럴이다 — 구현이 부르는
/// 함수로 기대값을 만들면 그 함수의 결함이 동어반복으로 가려진다.
#[test]
fn hanja_output_format_applies_to_single_syllable_confirmation() {
    let cases = [
        (HanjaOutputFormat::Hanja, "國"),
        (HanjaOutputFormat::HangulHanja, "국(國)"),
        (HanjaOutputFormat::HanjaHangul, "國(국)"),
    ];
    for (format, expected_body) in cases {
        let mut config = Config::default();
        config.engine.korean.hanja_output_format = format;
        let mut e = InputEngine::new(&config);
        e.set_input_category(InputCategory::Korean);
        type_keys(&mut e, &config, &DAEHANMINGUK);
        e.press_key(KeyCode::F9, ModifierState::default(), &config);
        let first = e.get_hanja_candidates()[0].0.clone();
        assert_eq!(first, "國", "{format:?}: 사전 첫 후보");

        e.press_key(KeyCode::Num1, ModifierState::default(), &config);
        assert_eq!(e.commit_str(), format!("대한민{expected_body}"), "{format:?}");
    }
}

/// 서식 3종 × 단어 확정 — 대상①(RecentWord) 교체 페이로드에도 동일하게 적용된다.
/// 기대값은 리터럴(`format.render()` 미사용, 위 테스트와 같은 이유).
#[test]
fn hanja_output_format_applies_to_word_replacement_payload() {
    let cases = [
        (HanjaOutputFormat::Hanja, "大韓民國"),
        (HanjaOutputFormat::HangulHanja, "대한민국(大韓民國)"),
        (HanjaOutputFormat::HanjaHangul, "大韓民國(대한민국)"),
    ];
    for (format, expected_body) in cases {
        let mut config = Config::default();
        config.engine.korean.hanja_output_format = format;
        let mut e = InputEngine::new(&config);
        e.set_input_category(InputCategory::Korean);
        e.set_hanja_word_replace_capable(true);
        type_keys(&mut e, &config, &DAEHANMINGUK);
        e.press_key(KeyCode::F9, ModifierState::default(), &config);
        assert_eq!(e.get_hanja_target(), "대한민국");
        let first = e.get_hanja_candidates()[0].0.clone();
        assert_eq!(first, "大韓民國", "{format:?}: 사전 첫 후보");

        e.press_key(KeyCode::Num1, ModifierState::default(), &config);
        let rep = e.take_hanja_replacement().expect("교체 페이로드");
        assert_eq!(rep.text, expected_body, "{format:?}");
        assert_eq!(rep.delete_chars, 3);
    }
}

// =============================================================================
// 즐겨찾기
// =============================================================================

/// 단어 키("대한민국")로도 즐겨찾기 토글이 동작하고, `HanjaCandidatesReordered.target`
/// 이 "대한민국" 이다. 상태는 재토글로 원복해 실제 즐겨찾기 파일을 건드리지 않는다
/// (기존 `tests_popup_change_page.rs` 의 double-toggle 관례와 동일).
///
/// 알려진 한계(open_issues 참조): `HanjaBookmarkStore` 는 `InputEngine::new()` 안에서
/// `load_default()`(실 `UNIM_DATA_DIR`/`~/.local/share`)로 고정 로드되고, 이 테스트
/// 소유 파일 밖(엔진 생성자, `src/input_engine/engine.rs`)에 경로 주입 지점이 없어
/// double-toggle 원복 외의 완전한 파일 격리는 이 U10 단위에서 하지 않는다.
#[test]
fn bookmark_toggle_on_word_target_reorders_and_restores() {
    let config = Config::default();
    let mut e = create_test_engine();
    e.set_input_category(InputCategory::Korean);
    e.set_word_mode(true);
    e.preedit_cache = "대한민국".to_string();
    e.press_key(KeyCode::F9, ModifierState::default(), &config);
    assert_eq!(e.get_hanja_target(), "대한민국");

    let original = e.hanja_bookmark_states()[0];
    let (idx, state, was) = e.toggle_hanja_bookmark(0).expect("토글 성공");
    assert_eq!(was, original);
    assert_eq!(state, !original);
    match e.take_popup_action() {
        Some(PopupAction::HanjaCandidatesReordered { target, .. }) => {
            assert_eq!(target, "대한민국");
        }
        other => panic!("HanjaCandidatesReordered 예상, got {other:?}"),
    }

    // 상태 원복.
    e.toggle_hanja_bookmark(idx).expect("복원 토글");
    assert_eq!(e.hanja_bookmark_states()[0], original);
}

// =============================================================================
// 비밀번호 게이트(fail-closed) — §2.8
// =============================================================================
// "진입 시 버퍼 비움" 은 `buffer_resets_on_documented_conditions` 이, "chord 비번
// 게이트" 는 `chord_idle_flush_does_not_fill_buffer_while_password_blocked` 이 각각
// 이미 커버한다.

/// pull 경로(`GetHanjaCandidates` = `start_hanja_conversion` 직접 호출)는 `press_key`
/// 의 영문 강제를 거치지 않으므로 함수 자체에서 차단한다.
#[test]
fn password_blocks_pull_path_start_hanja_conversion() {
    let mut e = create_test_engine();
    e.set_input_category(InputCategory::Korean);
    e.set_content_purpose(ContentPurpose::Password);
    e.preedit_cache = "국".to_string(); // 프런트가 실수로 조합 중처럼 보이게 하더라도

    let r = e.start_hanja_conversion();
    assert_eq!(r, InputResult::consumed());
    assert!(!e.is_hanja_mode());
}

/// 팝업이 열린 채 비밀번호 목적이 도착하면 재커밋 없이 닫는다(비번 필드에 원문 재삽입
/// 금지) — 정리가 `flush_preedit` 보다 먼저여야 한다(§2.8).
#[test]
fn password_during_open_popup_closes_without_recommit() {
    let config = Config::default();
    let mut e = create_test_engine();
    e.set_input_category(InputCategory::Korean);
    type_keys(&mut e, &config, &DAEHANMINGUK);
    e.press_key(KeyCode::F9, ModifierState::default(), &config);
    assert!(e.is_hanja_mode());

    e.take_popup_action(); // 진입 시 ShowHanja 드레인 — 관심사는 비번 진입 뒤 액션.
    e.set_content_purpose(ContentPurpose::Password);
    // "대한민" 은 진입 전 이미 커밋된 것 — 비밀번호 진입은 거기에 재커밋 텍스트를
    // 추가하지 않는다(원문 재삽입 금지).
    assert_eq!(e.commit_str(), "대한민", "재커밋 없이 닫혀야 한다");
    assert!(!e.is_hanja_mode());
    // 데몬은 이 액션으로 HidePopup 을 보낸다 — 없으면 팝업 창이 비번 필드 위에 남는다.
    assert!(
        matches!(e.take_popup_action(), Some(PopupAction::HidePopup)),
        "비밀번호 진입은 HidePopup 을 남겨야 한다"
    );
}

/// 위와 동일하되 대상①(`RecentWord`) 팝업 — 호스트 플래그 true 로 교체 페이로드가
/// 걸려 있는 상태에서 비밀번호 목적이 도착해도 페이로드를 남기지 않고 닫혀야 한다.
#[test]
fn password_during_open_recent_word_popup_closes_without_replacement_payload() {
    let config = Config::default();
    let mut e = setup_recent_word_pending(&config);
    e.press_key(KeyCode::F9, ModifierState::default(), &config);
    assert!(e.is_hanja_mode());
    assert_eq!(e.hanja_source(), HanjaSource::RecentWord);

    e.set_content_purpose(ContentPurpose::Password);
    assert_eq!(e.commit_str(), "대한민", "재커밋 없이 닫혀야 한다");
    assert!(!e.is_hanja_mode());
    assert!(e.take_hanja_replacement().is_none(), "비밀번호 진입은 교체 페이로드를 남기지 않는다");
}

/// 목적 통지보다 먼저 온 선택 스냅샷은 비밀번호 진입 시 즉시 제거된다(잔류 방지).
#[test]
fn password_entry_clears_stale_selection_snapshot() {
    let mut e = create_test_engine();
    e.set_surrounding_text("대한민국".to_string(), 0, 4);
    assert!(e.has_selection());

    e.set_content_purpose(ContentPurpose::Password);
    assert!(!e.has_selection());
    assert!(e.surrounding_text().0.is_empty());
}

/// 한자 사전은 프로세스당 한 벌 — 엔진(데몬 컨텍스트)마다 ~100MB 를 새로 파싱하면
/// 컨텍스트 수만큼 메모리가 쌓인다(2026-09 CI 기능 시험 러너 사망).
#[test]
fn engines_share_one_hanja_dictionary() {
    let config = Config::default();
    let a = InputEngine::new(&config);
    let b = InputEngine::new(&config);
    assert!(std::sync::Arc::ptr_eq(&a.hanja_dict, &b.hanja_dict));
}
