//! v1 프로필 / `active_rule_sets` hot-rebuild 회귀 + 룰 B(`/`키 컨텍스트 분기) +
//! 룰 A(`vowel_combine_head` 결합 키 제한) 테스트.
//!
//! 모두 `build_korean_context` 또는 한국어 자판 프로필 시스템을 검증한다.

use super::InputEngine;
use crate::config::{Config, InputCategory};
use crate::keycode::{KeyCode, ModifierState};

// === v1 프로필/active_rule_sets hot-rebuild 회귀 방지 ===

/// T2-A: `set_korean_layout` 단독 호출이 v1 builder 경로(`build_korean_context`)를
/// 거쳐 컨텍스트를 새로 만든다. 이전에는 `HangulInputContext::new(composer_type)`로
/// v1 프로필을 우회했음. 본 테스트는 panic 없이 layout이 갱신됨을 보장.
#[test]
fn test_set_korean_layout_routes_through_v1_builder() {
    let mut config = Config::default();
    config.engine.korean.layout = "ko_2bulstd".to_string();
    let mut engine = InputEngine::new(&config);
    assert_eq!(engine.korean_layout, "ko_2bulstd");

    engine.set_korean_layout("ko_3bul_qwerty".to_string());
    assert_eq!(engine.korean_layout, "ko_3bul_qwerty");
    // 컨텍스트가 ThreeBul composer로 재생성되어야 함
    assert!(crate::config::is_sebeolsik_layout(&engine.korean_layout));
}

/// T2-A': `rebuild_korean_context`가 layout 동일 + active_rule_sets만 다른 경우에도
/// 컨텍스트를 다시 만든다. 이전에는 hot-reload가 layout-only 비교라 누락됐음.
#[test]
fn test_rebuild_korean_context_applies_active_rule_sets() {
    let mut config = Config::default();
    config.engine.korean.layout = "ko_3bul390".to_string();
    let mut engine = InputEngine::new(&config);
    assert_eq!(engine.korean_layout, "ko_3bul390");

    // active_rule_sets만 변경 — layout은 동일
    config.engine.korean.active_rule_sets = Some(vec!["nonexistent_set".to_string()]);
    // panic 없이 통과해야 하며, rule_set이 없어도 폴백 경로로 컨텍스트가 살아 있어야 함
    engine.rebuild_korean_context(&config);
    assert_eq!(engine.korean_layout, "ko_3bul390");
}

/// T2-C: `rebuild_korean_context`가 layout 변경도 정상 처리한다.
#[test]
fn test_rebuild_korean_context_handles_layout_change() {
    let mut config = Config::default();
    config.engine.korean.layout = "ko_2bulstd".to_string();
    let mut engine = InputEngine::new(&config);
    assert_eq!(engine.korean_layout, "ko_2bulstd");

    config.engine.korean.layout = "ko_3bul_qwerty".to_string();
    engine.rebuild_korean_context(&config);
    assert_eq!(engine.korean_layout, "ko_3bul_qwerty");
}

/// builder의 3분기(None / Some(vec![]) / Some(vec![name])) 모두 panic 없이
/// 컨텍스트를 만들어야 한다. 의미는 builder 단위 테스트가 따로 검증하지만,
/// 본 테스트는 `build_korean_context` 호출 경로가 Option 타입을 그대로
/// 받아들이는 것을 보장한다.
#[test]
fn test_build_korean_context_accepts_three_active_rule_sets_variants() {
    // None — 미설정, 프로필 기본값 사용
    let mut config = Config::default();
    config.engine.korean.layout = "ko_3bul390".to_string();
    config.engine.korean.active_rule_sets = None;
    let _ = InputEngine::new(&config);

    // Some(vec![]) — 사용자가 모두 OFF
    config.engine.korean.active_rule_sets = Some(Vec::new());
    let _ = InputEngine::new(&config);

    // Some(vec!["unknown"]) — 존재하지 않는 이름은 silently drop, 폴백 안전
    config.engine.korean.active_rule_sets = Some(vec!["__nonexistent__".to_string()]);
    let _ = InputEngine::new(&config);
}

// ====================================================================
// 룰 B (schema v2 `key_meta.context_alt`) — `/` 키 컨텍스트 분기
// ====================================================================

fn make_3bul390_engine_no_auto_english() -> (InputEngine, Config) {
    let mut config = Config::default();
    config.engine.korean.layout = "ko_3bul390".to_string();
    // 룰 B 검증을 위해 auto_english 비활성 — 기본 trigger_keys에 char:/가 있어
    // 활성 시 / 키가 auto-english 트리거로 먼저 잡힌다.
    config.engine.auto_english.enabled = false;
    let mut engine = InputEngine::new(&config);
    engine.set_input_category(InputCategory::Korean);
    (engine, config)
}

/// 룰 B: preedit 빈 상태에서 영어 자판 `/` 키 → 리터럴 '/' commit
#[test]
fn test_rule_b_empty_preedit_commits_slash() {
    let (mut engine, config) = make_3bul390_engine_no_auto_english();
    let none = ModifierState::default();
    let result = engine.press_key(KeyCode::Slash, none, &config);
    assert!(result.consumed);
    assert!(result.commit_changed);
    assert_eq!(engine.commit_str(), "/");
    assert!(engine.preedit_str().is_empty());
}

/// 룰 B: 초성 ㄱ만 채워진 상태에서 `/` → ㄱ + ㅗ로 합성 (preedit "고")
#[test]
fn test_rule_b_choseong_only_keeps_jamo() {
    let (mut engine, config) = make_3bul390_engine_no_auto_english();
    let none = ModifierState::default();
    // ko_3bul390에서 'k' 키는 ㄱ (3rd 행)
    engine.press_key(KeyCode::K, none, &config);
    assert_eq!(engine.preedit_str(), "\u{3131}"); // ㄱ choseong-only

    // 영어 자판 `/` 키 → ko_3bul390 한국어 자판의 ㅗ로 매핑됨
    let result = engine.press_key(KeyCode::Slash, none, &config);
    assert!(result.consumed);
    // commit는 비어있어야 함 (음절 조합 진행 중)
    assert!(
        engine.commit_str().is_empty(),
        "commit='{}'",
        engine.commit_str()
    );
    assert_eq!(engine.preedit_str(), "고");
}

/// 룰 B: 초성+중성 채워진 상태에서 `/` → 리터럴 '/' commit (ㅘ로 합성 안 됨)
#[test]
fn test_rule_b_cho_jung_filled_commits_slash() {
    let (mut engine, config) = make_3bul390_engine_no_auto_english();
    let none = ModifierState::default();
    // ㄱ → preedit "ㄱ"
    engine.press_key(KeyCode::K, none, &config);
    // ko_3bul390에서 'f' 키는 ㅏ (3rd 행)
    engine.press_key(KeyCode::F, none, &config);
    assert_eq!(engine.preedit_str(), "가");

    // 영어 자판 / 입력 → preedit "가" flush + '/' commit (룰 B fallback)
    let result = engine.press_key(KeyCode::Slash, none, &config);
    assert!(result.consumed);
    assert!(result.commit_changed);
    assert!(
        engine.commit_str().ends_with('/'),
        "commit='{}'",
        engine.commit_str()
    );
    assert!(engine.preedit_str().is_empty());
}

/// 룰 B: 영문 모드에서 `/` → 룰 B 미적용 (한글 분기 진입 안 함)
#[test]
fn test_rule_b_english_mode_unaffected() {
    let (mut engine, config) = make_3bul390_engine_no_auto_english();
    engine.set_input_category(InputCategory::English);
    let none = ModifierState::default();
    let _ = engine.press_key(KeyCode::Slash, none, &config);
    // 영문 모드는 process_english_key 경로 — 룰 B 코드가 실행되지 않음.
    // 패닉 없으면 OK.
}

/// 룰 B 회귀: 두벌식(ko_2bulstd)은 key_meta가 없어 룰 B 미적용
#[test]
fn test_rule_b_two_bul_no_key_meta_branch() {
    let mut config = Config::default();
    config.engine.korean.layout = "ko_2bulstd".to_string();
    config.engine.auto_english.enabled = false;
    let mut engine = InputEngine::new(&config);
    engine.set_input_category(InputCategory::Korean);
    // 두벌식 키맵에는 key_meta가 없어 key_meta_map이 비어 있어야 함
    assert!(
        engine.key_meta_map.is_empty(),
        "ko_2bulstd should have empty key_meta_map"
    );
}

// ========================================================================
// 룰 A — 이중모음 결합 키 제한 (vowel_combine_head)
//
// 세벌식 390/391의 lower row4 4번째(v 키 ㅗ)·5번째(b 키 ㅜ)는
// `vowel_combine_head=false`로 명시되어 단순 모음만. 후속 ㅏ/ㅐ/ㅣ/ㅓ/ㅔ가
// 와도 ㅘ/ㅙ/ㅚ/ㅝ/ㅞ/ㅟ로 합용 시도 안 함, 새 음절로 분리.
// 같은 자판의 `/` 키 ㅗ(룰 B로 진입)와 `9` 키 ㅜ는 결합 가능.
// ========================================================================

/// 룰 A negative: ㄱ + v(ㅗ_simple) + ㅏ → "고" commit + 새 음절 "ㅏ" 시작
#[test]
fn test_rule_a_v_key_o_does_not_combine_with_a() {
    let (mut engine, config) = make_3bul390_engine_no_auto_english();
    let none = ModifierState::default();
    // ㄱ → preedit "ㄱ"
    engine.press_key(KeyCode::K, none, &config);
    // v 키 → ㅗ (vowel_combine_head=false). preedit "고".
    engine.press_key(KeyCode::V, none, &config);
    assert_eq!(engine.preedit_str(), "고");
    // ㅏ → 합용 거부. "고" commit + 새 음절 "ㅏ".
    engine.press_key(KeyCode::F, none, &config);
    assert!(
        engine.commit_str().ends_with('고'),
        "expected '고' committed, got commit='{}'",
        engine.commit_str()
    );
    assert_eq!(engine.preedit_str(), "\u{314F}"); // ㅏ
}

/// 룰 A positive: ㄱ + /(룰 B → ㅗ_head) + ㅏ → preedit "과" (정상 합용)
#[test]
fn test_rule_a_slash_key_o_combines_with_a_via_rule_b() {
    let (mut engine, config) = make_3bul390_engine_no_auto_english();
    let none = ModifierState::default();
    engine.press_key(KeyCode::K, none, &config); // ㄱ
    engine.press_key(KeyCode::Slash, none, &config); // 룰 B: ㅗ + head=true
    assert_eq!(engine.preedit_str(), "고");
    engine.press_key(KeyCode::F, none, &config); // ㅏ
    // 합용 성공 → preedit "과", commit 비어있음.
    assert!(
        engine.commit_str().is_empty(),
        "expected empty commit, got '{}'",
        engine.commit_str()
    );
    assert_eq!(engine.preedit_str(), "과");
}

/// 룰 A negative: ㄱ + b(ㅜ_simple) + ㅓ → "구" commit + 새 음절 "ㅓ"
#[test]
fn test_rule_a_b_key_u_does_not_combine_with_eo() {
    let (mut engine, config) = make_3bul390_engine_no_auto_english();
    let none = ModifierState::default();
    engine.press_key(KeyCode::K, none, &config); // ㄱ
    engine.press_key(KeyCode::B, none, &config); // ㅜ (head=false)
    assert_eq!(engine.preedit_str(), "구");
    // ko_3bul390에서 ㅓ는 't' 키 (lower 2nd 슬롯 5).
    engine.press_key(KeyCode::T, none, &config); // ㅓ
    assert!(
        engine.commit_str().ends_with('구'),
        "expected '구' committed, got commit='{}'",
        engine.commit_str()
    );
    assert_eq!(engine.preedit_str(), "\u{3153}"); // ㅓ
}

/// 룰 A positive: ㄱ + 9(ㅜ_head) + ㅓ → preedit "궈" (정상 합용)
#[test]
fn test_rule_a_nine_key_u_combines_with_eo() {
    let (mut engine, config) = make_3bul390_engine_no_auto_english();
    let none = ModifierState::default();
    engine.press_key(KeyCode::K, none, &config); // ㄱ
    engine.press_key(KeyCode::Num9, none, &config); // ㅜ (head=true)
    assert_eq!(engine.preedit_str(), "구");
    engine.press_key(KeyCode::T, none, &config); // ㅓ
    assert!(
        engine.commit_str().is_empty(),
        "expected empty commit, got '{}'",
        engine.commit_str()
    );
    assert_eq!(engine.preedit_str(), "궈");
}

/// 룰 A 회귀: 두벌식(ko_2bulstd)은 key_meta 부재 → 모든 ㅗ가 결합 가능 → ㅗ+ㅏ→ㅘ
#[test]
fn test_rule_a_two_bul_default_combines_o_a() {
    let mut config = Config::default();
    config.engine.korean.layout = "ko_2bulstd".to_string();
    config.engine.auto_english.enabled = false;
    let mut engine = InputEngine::new(&config);
    engine.set_input_category(InputCategory::Korean);
    let none = ModifierState::default();
    // 두벌식 ㅗ는 'h' 키 (lower 3rd slot 9). qwerty 'h' 슬롯은 KeyCode::H.
    // 두벌식 ㅏ는 'k' 키 (lower 3rd slot 7).
    // 두벌식 ㄱ은 'r' 키 (lower 2nd slot 1).
    engine.press_key(KeyCode::R, none, &config); // ㄱ
    engine.press_key(KeyCode::H, none, &config); // ㅗ
    engine.press_key(KeyCode::K, none, &config); // ㅏ → ㅘ 정상 합용
    assert!(
        engine.commit_str().is_empty(),
        "expected empty commit, got '{}'",
        engine.commit_str()
    );
    assert_eq!(engine.preedit_str(), "과");
}

/// 룰 A: ko_3bul391에서도 v/b 키 룰 동일 적용
#[test]
fn test_rule_a_v_key_3bul391_does_not_combine() {
    let mut config = Config::default();
    config.engine.korean.layout = "ko_3bul391".to_string();
    config.engine.auto_english.enabled = false;
    let mut engine = InputEngine::new(&config);
    engine.set_input_category(InputCategory::Korean);
    let none = ModifierState::default();
    engine.press_key(KeyCode::K, none, &config); // ㄱ
    engine.press_key(KeyCode::V, none, &config); // ㅗ_simple
    assert_eq!(engine.preedit_str(), "고");
    engine.press_key(KeyCode::F, none, &config); // ㅏ → 분리
    assert!(
        engine.commit_str().ends_with('고'),
        "expected '고' committed, got commit='{}'",
        engine.commit_str()
    );
    assert_eq!(engine.preedit_str(), "\u{314F}"); // ㅏ
}
