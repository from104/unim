//! 대상③ 적합성 픽스처 러너 — `tests/contract/surrounding_word_contract.json`.
//!
//! 사건열(caps·focus_in·field_focus·surrounding·hint·key·commit·hanja·resume 등)을 엔진에
//! 흘리고, `hanja`·`resume`·`expect` 가 붙은 사건 뒤의 관측값을 픽스처의 기대와 비교한다.
//! 같은 파일을 다른 언어의 러너(C·node)도 읽을 수 있게 엔진 내부 이름은 쓰지 않는다.
//! 시각 `t` 는 호스트가 `set_now_ms` 로 넘기는 단조 ms 다 — 코어는 시계를 읽지 않는다.

use super::test_helpers::create_test_engine;
use super::{DeferTrigger, HanjaSource, InputEngine, SURROUNDING_HINT_FIELD_FOCUS};
use crate::config::{Config, ContentPurpose};
use crate::keycode::{KeyCode, ModifierState};
use serde_json::Value;

const FIXTURE: &str = include_str!("../../tests/contract/surrounding_word_contract.json");

fn u(v: &Value, k: &str) -> u64 {
    v.get(k).and_then(Value::as_u64).unwrap_or(0)
}

fn observe(e: &InputEngine, poll: Option<&str>) -> Value {
    let deadline = e.deferred_hanja_deadline();
    let result = if deadline.is_some() {
        "defer"
    } else if e.is_hanja_mode() {
        "hanja"
    } else if e.is_emoji_popup_active() {
        "emoji"
    } else {
        "none"
    };
    let span = e
        .surrounding_word_span()
        .filter(|_| e.hanja_source() == HanjaSource::SurroundingWord)
        .map(|(a, b)| serde_json::json!([a, b]))
        .unwrap_or(Value::Null);
    serde_json::json!({
        "result": result,
        "reason": e.surrounding_word_last_reason(),
        "deadline": deadline,
        "pending": deadline.is_some(),
        "span": span,
        "poll": poll,
    })
}

fn check(case: &str, i: usize, got: &Value, expect: &Value) -> Result<(), String> {
    let Some(map) = expect.as_object() else { return Ok(()) };
    for (k, want) in map {
        let have = &got[k];
        if have != want {
            return Err(format!("{case} #{i}: {k}: 기대 {want} / 실제 {have}"));
        }
    }
    Ok(())
}

fn press(e: &mut InputEngine, k: KeyCode, m: ModifierState) {
    e.press_key(k, m, &Config::default());
}

/// 사례 실행 방식.
#[derive(Clone, Copy, PartialEq)]
enum Mode {
    /// 사건열을 그대로 돌리며 `expect` 를 비교한다.
    Check,
    /// `expect` 는 무시하고 매 사건 뒤 보류가 없음을 단언한다(S 컨텍스트로 바꿔 돌린다).
    NeverDefer,
}

/// 한 사례를 돌린다. `time` 은 사건 시각을 바꾸는 사상(S 컨텍스트 시각 무관 시험용).
fn run_case(c: &Value, time: &dyn Fn(u64) -> u64) -> Result<(), String> {
    run_case_mode(c, time, Mode::Check)
}

fn run_case_mode(c: &Value, time: &dyn Fn(u64) -> u64, mode: Mode) -> Result<(), String> {
    let name = c["name"].as_str().unwrap_or("?").to_string();
    let mut e = create_test_engine();
    for (i, ev) in c["events"].as_array().unwrap().iter().enumerate() {
        let t = time(u(ev, "t"));
        e.set_now_ms(t);
        let mut poll: Option<&str> = None;
        match ev["ev"].as_str().unwrap() {
            "caps" => {
                let v = u(ev, "v") as u32;
                // NeverDefer: 같은 사건열을 S 비트 컨텍스트(REPLACE|SELECTION_RELIABLE, P 는 유지)로 돌린다.
                e.set_surrounding_word_caps(if mode == Mode::NeverDefer { 0x3 | (v & 0x4) } else { v });
            }
            "purpose" => e.set_content_purpose(match ev["v"].as_str().unwrap() {
                "password" => ContentPurpose::Password,
                _ => ContentPurpose::Normal,
            }),
            "focus_in" => e.surrounding_word_focus_in(),
            "field_focus" => e.surrounding_hint(SURROUNDING_HINT_FIELD_FOCUS, 0, 0),
            "surrounding" => {
                let cur = u(ev, "cursor") as u32;
                let anchor = ev.get("anchor").and_then(Value::as_u64).map_or(cur, |a| a as u32);
                e.set_surrounding_text(ev["text"].as_str().unwrap().to_string(), cur, anchor);
            }
            "hint" => e.surrounding_hint(u(ev, "kind") as u32, u(ev, "key") as u32, u(ev, "state") as u32),
            "key" => match ev["k"].as_str().unwrap() {
                "F1" => press(&mut e, KeyCode::F1, ModifierState::default()),
                "Shift" => press(&mut e, KeyCode::LeftShift, ModifierState::default()),
                "ShiftLeft" => press(
                    &mut e,
                    KeyCode::Left,
                    ModifierState { shift: true, ..Default::default() },
                ),
                "CtrlA" => press(
                    &mut e,
                    KeyCode::A,
                    ModifierState { control: true, ..Default::default() },
                ),
                k => return Err(format!("{name} #{i}: 알 수 없는 key {k}")),
            },
            "commit" => e.note_surrounding_commit(),
            "hanja" => press(&mut e, KeyCode::F9, ModifierState::default()),
            "resume" => {
                e.resume_deferred_hanja(t);
            }
            // 호스트의 재판정 시도(R9) — "wait" 는 계속 대기(기한 갱신 가능), "done" 은 판정 끝.
            "poll" => {
                let trigger = match ev["trigger"].as_str().unwrap_or("") {
                    "report" => DeferTrigger::Report,
                    "deadline" => DeferTrigger::Deadline,
                    other => return Err(format!("{name} #{i}: 알 수 없는 trigger {other}")),
                };
                poll = Some(if e.poll_deferred_hanja(t, trigger).is_none() { "wait" } else { "done" });
            }
            "reset" => e.reset(),
            "recreate" => {
                // 엔진 재생성(FocusOut·Reset) — 이월 묶음만 건너간다. 문서(surrounding)는 이월되지 않는다.
                let carry = e.surrounding_word_carry();
                e = create_test_engine();
                e.restore_surrounding_word_carry(carry);
                e.set_now_ms(t);
            }
            other => return Err(format!("{name} #{i}: 알 수 없는 사건 {other}")),
        }
        match mode {
            Mode::Check => {
                if let Some(exp) = ev.get("expect") {
                    check(&name, i, &observe(&e, poll), exp)?;
                }
            }
            Mode::NeverDefer => {
                if e.deferred_hanja_deadline().is_some() {
                    return Err(format!("{name} #{i}: S 컨텍스트에서 Defer 발생"));
                }
            }
        }
    }
    Ok(())
}

fn cases() -> Vec<Value> {
    let v: Value = serde_json::from_str(FIXTURE).expect("픽스처 JSON");
    assert_eq!(v["version"], 1);
    v["cases"].as_array().unwrap().clone()
}

#[test]
fn contract_fixture_all_cases() {
    let cs = cases();
    assert!(cs.len() >= 30, "픽스처 사례 수: {}", cs.len());
    let fails: Vec<String> = cs
        .iter()
        .filter_map(|c| run_case(c, &|t| t).err())
        .collect();
    assert!(fails.is_empty(), "적합성 실패:\n{}", fails.join("\n"));
}

/// S 비트가 있는 컨텍스트는 시각과 무관하다 — 시각을 0 으로 누르거나 1000 배로 늘려도 같은 결과.
#[test]
fn contract_s_contexts_are_time_independent() {
    let mut n = 0;
    for c in cases() {
        let caps = c["events"][0]["v"].as_u64().unwrap_or(0) as u32;
        if caps & super::SURROUNDING_CAP_SELECTION_RELIABLE == 0 {
            continue;
        }
        n += 1;
        for (label, f) in [
            ("t=0", Box::new(|_t| 0u64) as Box<dyn Fn(u64) -> u64>),
            ("t*1000", Box::new(|t| t * 1000)),
            ("t+1e9", Box::new(|t| t + 1_000_000_000)),
        ] {
            // 기대의 deadline 은 push 전용 — S 사례는 deadline 을 기대하지 않는다.
            run_case(&c, &*f).unwrap_or_else(|m| panic!("[{label}] {m}"));
        }
    }
    assert!(n >= 5, "S 사례 수: {n}");
}

/// 같은 사건열을 S 컨텍스트로 돌리면 push 전용 상태(보류·후보)는 전혀 만들어지지 않는다 —
/// 키·커밋·resume·poll·재생성·목적 사건까지 전부 재생한다.
#[test]
fn contract_s_context_never_defers() {
    let mut n = 0;
    for c in cases() {
        n += 1;
        run_case_mode(&c, &|t| t, Mode::NeverDefer).unwrap_or_else(|m| panic!("{m}"));
    }
    assert!(n >= 30);
}
