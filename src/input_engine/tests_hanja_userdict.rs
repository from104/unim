//! 한자 사용자 사전(HANJA_USERDICT_SPEC §3) 엔진 통합 시험.
//!
//! 사용자 층은 엔진별로 `pin_hanja_user_layer` 로 주입한다 — 전역 override 를 건드리지
//! 않으므로 병렬 시험이 서로 간섭하지 않는다(시험 빌드의 엔진은 기본으로 층이 고정된 빈 값).
//! 사용자 전용 단어 `서기현→徐基鉉` 은 내장 사전에 없다(아래 시험이 전제로 단언한다).

use super::test_helpers::create_test_engine;
use super::{HanjaSource, InputEngine, InputResult, PopupAction, SURROUNDING_CAP_REPLACE, SURROUNDING_CAP_SELECTION_RELIABLE};
use crate::config::{Config, InputCategory};
use crate::hanja::{HanjaUserDict, HanjaUserIndex};
use crate::keycode::{KeyCode, ModifierState};
use std::sync::Arc;

/// 즐겨찾기를 메모리 전용 빈 저장소로 바꾼 엔진 — 개발자의 실제 즐겨찾기 파일을
/// 읽지도 쓰지도 않는다.
fn fresh_engine() -> InputEngine {
    let mut e = create_test_engine();
    e.hanja_bookmarks = crate::hanja::HanjaBookmarkStore::default();
    e
}

fn user_index(items: &[(&str, &str, Option<&str>)]) -> Arc<HanjaUserIndex> {
    let mut d = HanjaUserDict::default();
    for (h, j, m) in items {
        d.add(h, j, m.map(String::from)).expect("유효한 시험 항목");
    }
    Arc::new(HanjaUserIndex::from_dict(&d))
}

fn type_keys(e: &mut InputEngine, config: &Config, keys: &[KeyCode]) {
    for &k in keys {
        e.press_key(k, ModifierState::default(), config);
    }
}

fn f9(e: &mut InputEngine) -> InputResult {
    e.press_key(KeyCode::F9, ModifierState::default(), &Config::default())
}

// 두벌식: 서=T,J 기=R,L 현=G,U,S
const SEOGIHYEON: [KeyCode; 7] = [
    KeyCode::T,
    KeyCode::J,
    KeyCode::R,
    KeyCode::L,
    KeyCode::G,
    KeyCode::U,
    KeyCode::S,
];

/// 음절 모드로 "서기"(커밋)+"현"(preedit) 을 만든 엔진.
fn recent_word_engine(user: Arc<HanjaUserIndex>) -> (InputEngine, Config) {
    let config = Config::default();
    let mut e = fresh_engine();
    e.pin_hanja_user_layer(user);
    e.set_input_category(InputCategory::Korean);
    e.set_hanja_word_replace_capable(true);
    type_keys(&mut e, &config, &SEOGIHYEON);
    assert_eq!(e.commit_str(), "서기");
    assert_eq!(e.preedit_str(), "현");
    (e, config)
}

fn sw_engine(user: Arc<HanjaUserIndex>, text: &str, cursor: u32) -> InputEngine {
    let mut e = fresh_engine();
    e.pin_hanja_user_layer(user);
    e.set_surrounding_word_caps(SURROUNDING_CAP_REPLACE | SURROUNDING_CAP_SELECTION_RELIABLE);
    e.set_surrounding_text(text.to_string(), cursor, cursor);
    e
}

fn hanjas(e: &InputEngine) -> Vec<String> {
    e.get_hanja_candidates().into_iter().map(|(h, _)| h).collect()
}

#[test]
fn premise_user_word_not_in_builtin() {
    let e = fresh_engine();
    assert!(!e.hanja_dict.contains("서기현"));
    assert!(e.hanja_dict.contains("기현"), "전제: 내장 '기현' (축소 경유지)");
}

// ── 무회귀: 빈 층은 내장과 동일 ──

#[test]
fn empty_layer_search_equals_builtin() {
    let mut e = fresh_engine();
    e.pin_hanja_user_layer(Arc::new(HanjaUserIndex::empty()));
    for k in ["현", "대한민국", "국어", "뷁"] {
        assert_eq!(e.hanja_search(k), e.hanja_dict.search(k), "{k}");
        assert_eq!(e.hanja_contains(k), e.hanja_dict.contains(k), "{k}");
    }
}

// ── 단음절 ──

#[test]
fn single_syllable_user_entry_first_and_dedupes_builtin() {
    let config = Config::default();
    let mut probe = fresh_engine();
    let builtin = probe.hanja_dict.search("현");
    assert!(builtin.len() >= 3, "전제: 내장 '현' 후보 3개 이상");
    let pick = builtin[2].clone();
    probe.pin_hanja_user_layer(user_index(&[("현", &pick.hanja, None)]));

    let mut e = fresh_engine();
    e.pin_hanja_user_layer(user_index(&[("현", &pick.hanja, None)]));
    e.set_input_category(InputCategory::Korean);
    type_keys(&mut e, &config, &[KeyCode::G, KeyCode::U, KeyCode::S]);
    assert_eq!(f9(&mut e), InputResult::hanja_candidates());
    assert_eq!(e.get_hanja_target(), "현");

    let got = hanjas(&e);
    assert_eq!(got[0], pick.hanja, "사용자 항목이 맨 앞");
    assert_eq!(got.iter().filter(|h| **h == pick.hanja).count(), 1, "중복 1회만");
    assert_eq!(got.len(), builtin.len());
    // 나머지는 내장 순서 유지.
    let rest: Vec<String> = builtin.iter().filter(|b| b.hanja != pick.hanja).map(|b| b.hanja.clone()).collect();
    assert_eq!(got[1..], rest[..]);
    // 뜻 폴백: 사용자 뜻이 없으면 내장 원본 뜻.
    assert_eq!(e.hanja_candidates[0].meaning, pick.meaning);
}

#[test]
fn single_syllable_user_meaning_overrides_builtin() {
    let config = Config::default();
    let probe = fresh_engine();
    let pick = probe.hanja_dict.search("현")[1].clone();
    let mut e = fresh_engine();
    e.pin_hanja_user_layer(user_index(&[("현", &pick.hanja, Some("내 뜻"))]));
    e.set_input_category(InputCategory::Korean);
    type_keys(&mut e, &config, &[KeyCode::G, KeyCode::U, KeyCode::S]);
    f9(&mut e);
    assert_eq!(e.get_hanja_candidates()[0], (pick.hanja.clone(), "내 뜻".to_string()));
}

// ── ① 방금 입력한 어절 ──

#[test]
fn recent_word_user_only_word_is_target_and_first() {
    let (mut e, _c) = recent_word_engine(user_index(&[("서기현", "徐基鉉", None)]));
    assert_eq!(f9(&mut e), InputResult::hanja_candidates());
    assert_eq!(e.get_hanja_target(), "서기현");
    assert_eq!(e.hanja_source(), HanjaSource::RecentWord);
    assert_eq!(e.hanja_committed_chars(), 2);
    assert_eq!(hanjas(&e), vec!["徐基鉉".to_string()]);
}

#[test]
fn recent_word_without_user_layer_falls_back_to_last_syllable() {
    let (mut e, _c) = recent_word_engine(Arc::new(HanjaUserIndex::empty()));
    f9(&mut e);
    assert_eq!(e.get_hanja_target(), "기현", "내장 '기현' — 사용자 층 없이는 서기현 불가");
}

#[test]
fn recent_word_user_word_is_shrink_stop() {
    let (mut e, _c) = recent_word_engine(user_index(&[
        ("서기현", "徐基鉉", None),
        ("기현", "基鉉", None),
    ]));
    f9(&mut e);
    assert_eq!(e.get_hanja_target(), "서기현");
    // 한자키 재타 — 접미 축소가 사용자 단어 "기현" 을 경유한다.
    assert_eq!(f9(&mut e), InputResult::hanja_candidates());
    assert_eq!(e.get_hanja_target(), "기현");
    assert_eq!(hanjas(&e)[0], "基鉉");
}

#[test]
fn recent_word_user_entry_precedes_builtin_candidates() {
    let probe = fresh_engine();
    assert!(probe.hanja_dict.contains("서기"), "전제: 내장 '서기'");
    let (mut e, _c) = recent_word_engine(user_index(&[("서기현", "徐基鉉", None), ("현", "鉉", None)]));
    f9(&mut e);
    assert_eq!(e.get_hanja_target(), "서기현");
    // 축소: 서기현 → 기현(내장) → 현 : 사용자 鉉 이 맨 앞, 1회만.
    f9(&mut e);
    assert_eq!(e.get_hanja_target(), "기현");
    f9(&mut e);
    assert_eq!(e.get_hanja_target(), "현");
    let got = hanjas(&e);
    assert_eq!(got[0], "鉉");
    assert_eq!(got.iter().filter(|h| *h == "鉉").count(), 1);
}

// ── ② 선택 영역 ──

#[test]
fn selection_user_only_word_exact_match() {
    let mut e = fresh_engine();
    e.pin_hanja_user_layer(user_index(&[("서기현", "徐基鉉", None)]));
    e.set_surrounding_text("서기현".to_string(), 0, 3);
    assert!(e.has_selection());
    assert_eq!(f9(&mut e), InputResult::hanja_candidates());
    assert_eq!(e.get_hanja_target(), "서기현");
    assert_eq!(e.hanja_source(), HanjaSource::Selection);
    assert_eq!(hanjas(&e), vec!["徐基鉉".to_string()]);
}

#[test]
fn selection_builtin_word_user_entry_first_no_dup() {
    let probe = fresh_engine();
    let builtin = probe.hanja_dict.search("대한민국");
    assert_eq!(builtin[0].hanja, "大韓民國");
    let mut e = fresh_engine();
    e.pin_hanja_user_layer(user_index(&[("대한민국", "大韓民國", Some("우리나라")), ("대한민국", "大韓民国", None)]));
    e.set_surrounding_text("대한민국".to_string(), 0, 4);
    f9(&mut e);
    let got = hanjas(&e);
    assert_eq!(got[0], "大韓民國");
    assert_eq!(got[1], "大韓民国");
    assert_eq!(got.iter().filter(|h| *h == "大韓民國").count(), 1);
    assert_eq!(e.get_hanja_candidates()[0].1, "우리나라");
}

#[test]
fn selection_without_user_layer_rejects_user_only_word() {
    let mut e = fresh_engine();
    e.set_surrounding_text("서기현".to_string(), 0, 3);
    f9(&mut e);
    assert!(!e.is_hanja_mode());
}

// ── ③ 커서가 걸친 단어 ──

#[test]
fn surrounding_word_user_only_word_longest_match() {
    let mut e = sw_engine(user_index(&[("서기현", "徐基鉉", None)]), "서기현", 3);
    assert_eq!(f9(&mut e), InputResult::hanja_candidates());
    assert_eq!(e.hanja_source(), HanjaSource::SurroundingWord);
    assert_eq!(e.get_hanja_target(), "서기현");
    assert_eq!(hanjas(&e), vec!["徐基鉉".to_string()]);
    assert_eq!(e.surrounding_word_span(), Some((3, 0)));
}

#[test]
fn surrounding_word_repress_cycles_through_user_words() {
    let mut e = sw_engine(
        user_index(&[("서기현", "徐基鉉", None), ("기현", "基鉉", None)]),
        "서기현",
        3,
    );
    f9(&mut e);
    assert_eq!(e.get_hanja_target(), "서기현");
    assert_eq!(f9(&mut e), InputResult::hanja_candidates());
    assert_eq!(e.get_hanja_target(), "기현");
    assert_eq!(hanjas(&e)[0], "基鉉");
    assert!(matches!(e.take_popup_action(), Some(PopupAction::ShowHanja { .. })));
}

#[test]
fn surrounding_word_not_found_without_user_layer() {
    // 내장 사전만으로는 "서기현" 전체가 아니라 "현" 만 잡힌다.
    let mut e = sw_engine(Arc::new(HanjaUserIndex::empty()), "서기현", 3);
    f9(&mut e);
    assert_ne!(e.get_hanja_target(), "서기현");
}

// ── ★ 토글 ──

#[test]
fn bookmark_toggle_keeps_user_entries() {
    let (mut e, _c) = recent_word_engine(user_index(&[
        ("서기현", "徐基鉉", None),
        ("서기현", "徐基炫", None),
    ]));
    f9(&mut e);
    assert_eq!(hanjas(&e), vec!["徐基鉉".to_string(), "徐基炫".to_string()]);

    // 둘째 항목 ★ → 맨 앞으로 오고 사용자 항목은 둘 다 남는다.
    let (idx, state, was) = e.toggle_hanja_bookmark(1).expect("토글");
    assert!(state && !was);
    let after = hanjas(&e);
    assert_eq!(after, vec!["徐基炫".to_string(), "徐基鉉".to_string()]);
    assert_eq!(idx, 0);

    // 해제 → 자연 순서(사용자 파일 순)로 복귀, 항목 유지. (실제 즐겨찾기 파일 원복)
    e.toggle_hanja_bookmark(idx).expect("복원 토글");
    assert_eq!(hanjas(&e), vec!["徐基鉉".to_string(), "徐基炫".to_string()]);
}

#[test]
fn bookmark_toggle_with_builtin_mix_keeps_user_first() {
    let probe = fresh_engine();
    let builtin_len = probe.hanja_dict.search("현").len();
    let config = Config::default();
    let mut e = fresh_engine();
    e.pin_hanja_user_layer(user_index(&[("현", "鉉", None)]));
    e.set_input_category(InputCategory::Korean);
    type_keys(&mut e, &config, &[KeyCode::G, KeyCode::U, KeyCode::S]);
    f9(&mut e);
    let before = hanjas(&e);
    assert_eq!(before[0], "鉉");
    let n = before.len();
    assert!(n >= builtin_len);
    let (idx, _, _) = e.toggle_hanja_bookmark(n - 1).expect("토글");
    let mid = hanjas(&e);
    assert_eq!(mid.len(), n, "★ 토글 뒤 후보 수 불변");
    assert!(mid.contains(&"鉉".to_string()));
    e.toggle_hanja_bookmark(idx).expect("복원 토글");
    assert_eq!(hanjas(&e), before);
}

// ── 스냅샷 갱신 시점 ──

#[test]
fn layer_refreshes_on_new_popup_not_during_popup() {
    let (mut e, _c) = recent_word_engine(user_index(&[("서기현", "徐基鉉", None)]));
    // 고정 해제 → 한자키가 새 팝업을 열 때 전역 층(시험 기본값: 빈 층)으로 갱신한다.
    e.hanja_user_pinned = false;
    let before = Arc::as_ptr(&e.hanja_user);
    f9(&mut e);
    assert_ne!(Arc::as_ptr(&e.hanja_user), before, "새 팝업 한자키에서 갱신");
    assert_eq!(e.get_hanja_target(), "기현", "빈 층 → 서기현 은 대상이 아님");

    // 팝업이 열린 동안(재타)은 같은 스냅샷.
    let snap = Arc::as_ptr(&e.hanja_user);
    f9(&mut e);
    assert_eq!(Arc::as_ptr(&e.hanja_user), snap);
}
