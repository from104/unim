//! TEMP repro: shift-jongseong attach divergence (세벌식390 + live rule_sets)
use super::InputEngine;
use crate::config::{Config, InputCategory};
use crate::keycode::{KeyCode, ModifierState};

fn live_config() -> Config {
    let mut c = Config::default();
    c.engine.korean.layout = "ko_3bul390".to_string();
    c.engine.korean.active_rule_sets = Some(vec![
        "slash_context_alt".to_string(),
        "vowel_strict".to_string(),
        "sun_arae_batchim".to_string(),
    ]);
    c
}

fn sh() -> ModifierState {
    ModifierState { shift: true, ..Default::default() }
}
fn no() -> ModifierState {
    ModifierState::default()
}

fn dump(engine: &mut InputEngine, kc: KeyCode, m: ModifierState, cfg: &Config, label: &str) {
    let r = engine.press_key(kc, m, cfg);
    eprintln!(
        "[REPRO] {label}: consumed={} commit_changed={} preedit_changed={} commit='{}' preedit='{}'",
        r.consumed, r.commit_changed, r.preedit_changed,
        engine.commit_str(), engine.preedit_str()
    );
    engine.clear_commit();
}

#[test]
fn repro_ga_then_shift_w_jong() {
    let cfg = live_config();
    let mut engine = InputEngine::new(&cfg);
    engine.set_input_category(InputCategory::Korean);
    eprintln!("--- 가 (ㄱ=K, ㅏ=F) then Shift+W (=ㅌ jong) ---");
    dump(&mut engine, KeyCode::K, no(), &cfg, "K(ㄱ)");
    dump(&mut engine, KeyCode::F, no(), &cfg, "F(ㅏ)");
    dump(&mut engine, KeyCode::W, sh(), &cfg, "Shift+W(ㅌ-jong)");
    eprintln!("EXPECT: preedit should be '같' (가+ㅌ batchim)");
}

#[test]
fn repro_logged_zaqwe_sequence() {
    let cfg = live_config();
    let mut engine = InputEngine::new(&cfg);
    engine.set_input_category(InputCategory::Korean);
    eprintln!("--- logged shifted Z A Q W E 1 sequence ---");
    dump(&mut engine, KeyCode::Z, sh(), &cfg, "Shift+Z");
    dump(&mut engine, KeyCode::A, sh(), &cfg, "Shift+A");
    dump(&mut engine, KeyCode::Q, sh(), &cfg, "Shift+Q");
    dump(&mut engine, KeyCode::W, sh(), &cfg, "Shift+W");
    dump(&mut engine, KeyCode::E, sh(), &cfg, "Shift+E");
}

fn cfg_with(rs: Option<Vec<&str>>) -> Config {
    let mut c = Config::default();
    c.engine.korean.layout = "ko_3bul390".to_string();
    c.engine.korean.active_rule_sets = rs.map(|v| v.iter().map(|s| s.to_string()).collect());
    c
}

fn type_ga(cfg: &Config, label: &str) {
    let mut e = InputEngine::new(cfg);
    e.set_input_category(InputCategory::Korean);
    // ㄱ = qwerty 'k' lower, ㅏ = qwerty 'f' lower (per PROBE: F plain preedit='ㅏㄲ')
    e.press_key(KeyCode::K, no(), cfg); // ㄱ
    e.clear_commit();
    e.press_key(KeyCode::F, no(), cfg); // ㅏ?
    eprintln!("[GA {label}] after K,F: commit='{}' preedit='{}'", e.commit_str(), e.preedit_str());
}

#[test]
fn repro_rule_set_isolation() {
    eprintln!("=== Does 가 form under different rule_set configs? ===");
    type_ga(&cfg_with(None), "None(profile default)");
    type_ga(&cfg_with(Some(vec![])), "empty(all off)");
    type_ga(&cfg_with(Some(vec!["slash_context_alt"])), "slash_only");
    type_ga(&cfg_with(Some(vec!["vowel_strict"])), "vowel_strict");
    type_ga(&cfg_with(Some(vec!["sun_arae_batchim"])), "sun_arae_batchim");
    type_ga(&cfg_with(Some(vec!["slash_context_alt","vowel_strict","sun_arae_batchim"])), "ALL(live)");
}

#[test]
fn repro_what_keys_make_ga() {
    // probe each lower-row key -> char/jamo to identify ㄱ and ㅏ keys
    let cfg = live_config();
    let mut engine = InputEngine::new(&cfg);
    engine.set_input_category(InputCategory::Korean);
    for kc in [KeyCode::A, KeyCode::S, KeyCode::D, KeyCode::F, KeyCode::G,
               KeyCode::H, KeyCode::J, KeyCode::K, KeyCode::L,
               KeyCode::Q, KeyCode::W, KeyCode::E, KeyCode::R, KeyCode::T,
               KeyCode::Y, KeyCode::U, KeyCode::I, KeyCode::O, KeyCode::P,
               KeyCode::Z, KeyCode::X, KeyCode::C, KeyCode::V, KeyCode::B,
               KeyCode::N, KeyCode::M] {
        let mut e2 = InputEngine::new(&cfg);
        e2.set_input_category(InputCategory::Korean);
        let r = e2.press_key(kc, no(), &cfg);
        let rs = e2.press_key(kc, sh(), &cfg);
        eprintln!("[PROBE] {:?}: plain(commit='{}' preedit='{}' cc={}) | shift(commit='{}' preedit='{}' cc={})",
            kc, e2.commit_str(), e2.preedit_str(), r.commit_changed,
            { let mut e3 = InputEngine::new(&cfg); e3.set_input_category(InputCategory::Korean); e3.press_key(kc, sh(), &cfg); e3.commit_str().to_string() },
            { let mut e3 = InputEngine::new(&cfg); e3.set_input_category(InputCategory::Korean); e3.press_key(kc, sh(), &cfg); e3.preedit_str().to_string() },
            rs.commit_changed);
    }
}
