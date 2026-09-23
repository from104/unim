//! UNIM DBus 자동 회귀 테스트
//!
//! 실행 중인 unim-daemon에 DBus로 연결하여
//! 한글 조합 시나리오를 자동 검증합니다.
//!
//! Usage: unim-test-dbus [--verbose]
//!
//! ## 한자 단어(4) — 격리 데몬 전용
//!
//! `test_hanja_word`의 케이스 (4)는 `SetConfigYaml`로 `~/.config/unim/config.yaml`을
//! 고쳐 쓰고 원복하는 유일한 케이스다. 이 바이너리는 기본적으로 **실행 중인
//! 세션 데몬**(실사용 `unim-daemon`)에 붙는다 — `make test-dbus-auto`·`make
//! dev-test`가 그렇게 부른다. 그 상태로 케이스 4를 돌리면 실사용
//! `~/.config/unim/config.yaml`을 실제로 덮어썼다가 되돌리게 되므로, 환경변수
//! `UNIM_TEST_ISOLATED=1`이 설정돼 있을 때만 실행하고 그 외에는 SKIP한다.
//!
//! 격리 실행 레시피(§ 계획 문서 U13 "절대 규칙" 참조):
//! ```text
//! T=$(mktemp -d)
//! env -u DISPLAY -u WAYLAND_DISPLAY \
//!   HOME="$T" XDG_CONFIG_HOME="$T/config" XDG_DATA_HOME="$T/data" \
//!   XDG_CACHE_HOME="$T/cache" XDG_RUNTIME_DIR="$T/run" \
//!   UNIM_TEST_ISOLATED=1 \
//!   dbus-run-session -- bash -c '
//!     mkdir -p "$XDG_RUNTIME_DIR"; chmod 700 "$XDG_RUNTIME_DIR"
//!     ./target/release/unim-daemon -n &
//!     DAEMON_PID=$!
//!     sleep 1
//!     ./target/release/unim-test-dbus --verbose
//!     kill "$DAEMON_PID"
//!   '
//! ```
//! `XDG_RUNTIME_DIR`도 반드시 격리해야 한다 — `unim-daemon`이 그 안에 PID 파일을
//! 쓰고, 그 PID를 대상으로 한 kill 경로를 갖고 있다(`unim-daemon/src/main.rs`).

mod scenarios;

use futures_util::StreamExt;
use scenarios::{common_scenarios, scenarios_for_layout, KeyPress, Layout, TestScenario};
use std::time::Duration;
use unim_dbus::client::{InputContextProxy, InputMethodProxy, UnimClient};

// ANSI 색상
const RED: &str = "\x1b[0;31m";
const GREEN: &str = "\x1b[0;32m";
const YELLOW: &str = "\x1b[0;33m";
const BOLD: &str = "\x1b[1m";
const DIM: &str = "\x1b[2m";
const RESET: &str = "\x1b[0m";

struct TestResult {
    name: String,
    passed: bool,
    detail: Option<String>,
}

async fn run_scenario(
    client: &UnimClient,
    ctx_path: &str,
    scenario: &TestScenario,
    verbose: bool,
) -> TestResult {
    let name = scenario.name.to_string();

    // InputMethod 프록시로 한글/영문 모드 설정
    let im = match client.input_method().await {
        Ok(im) => im,
        Err(e) => {
            return TestResult {
                name,
                passed: false,
                detail: Some(format!("InputMethod 프록시 실패: {}", e)),
            };
        }
    };

    if let Err(e) = im.set_global_mode(scenario.initial_korean_mode).await {
        return TestResult {
            name,
            passed: false,
            detail: Some(format!("SetGlobalMode 실패: {}", e)),
        };
    }

    // InputContext 프록시
    let ic = match client.input_context(ctx_path).await {
        Ok(ic) => ic,
        Err(e) => {
            return TestResult {
                name,
                passed: false,
                detail: Some(format!("InputContext 프록시 실패: {}", e)),
            };
        }
    };

    // Focus in
    if let Err(e) = ic.focus_in("").await {
        return TestResult {
            name,
            passed: false,
            detail: Some(format!("FocusIn 실패: {}", e)),
        };
    }

    // 키 시퀀스 실행
    let mut total_commit = String::new();
    let mut last_preedit = String::new();
    let mut step_failures = Vec::new();

    for (i, key) in scenario.keys.iter().enumerate() {
        match ic.process_key_event(0, key.keycode, key.state).await {
            Ok((consumed, preedit, commit)) => {
                if verbose {
                    eprintln!(
                        "  {}step {}: keycode={}, state={} → consumed={}, preedit=\"{}\", commit=\"{}\"{}",
                        DIM, i, key.keycode, key.state, consumed, preedit, commit, RESET
                    );
                }

                last_preedit = preedit.clone();
                total_commit.push_str(&commit);

                // Step-by-step 검증
                if let Some(ref steps) = scenario.steps {
                    if let Some((exp_preedit, exp_commit)) = steps.get(i) {
                        if preedit != *exp_preedit {
                            step_failures.push(format!(
                                "step {}: preedit 기대=\"{}\" 실제=\"{}\"",
                                i, exp_preedit, preedit
                            ));
                        }
                        if commit != *exp_commit {
                            step_failures.push(format!(
                                "step {}: commit 기대=\"{}\" 실제=\"{}\"",
                                i, exp_commit, commit
                            ));
                        }
                    }
                }
            }
            Err(e) => {
                let _ = ic.reset().await;
                return TestResult {
                    name,
                    passed: false,
                    detail: Some(format!("step {}: ProcessKeyEvent 실패: {}", i, e)),
                };
            }
        }
    }

    // Reset
    let _ = ic.reset().await;

    // 검증
    let mut failures = step_failures;

    if !scenario.expected_final_preedit.is_empty()
        && last_preedit != scenario.expected_final_preedit
    {
        failures.push(format!(
            "final preedit: 기대=\"{}\" 실제=\"{}\"",
            scenario.expected_final_preedit, last_preedit
        ));
    }

    if !scenario.expected_total_commit.is_empty() && total_commit != scenario.expected_total_commit
    {
        failures.push(format!(
            "total commit: 기대=\"{}\" 실제=\"{}\"",
            scenario.expected_total_commit, total_commit
        ));
    }

    // 영문 모드: consumed=false 면 preedit/commit 비어야 함
    if !scenario.initial_korean_mode && last_preedit.is_empty() && total_commit.is_empty() {
        // OK
    }

    if failures.is_empty() {
        TestResult {
            name,
            passed: true,
            detail: None,
        }
    } else {
        TestResult {
            name,
            passed: false,
            detail: Some(failures.join("; ")),
        }
    }
}

// ─── 한자/특수문자 팝업 테스트 ────────────────────────────────────

/// 한자 팝업: 글자 조합 → Hanja키 → 후보 조회 → 선택 → 커밋
async fn test_hanja_popup(
    im: &InputMethodProxy<'_>,
    ic: &InputContextProxy<'_>,
    layout: &Layout,
    verbose: bool,
) -> Vec<TestResult> {
    let mut results = Vec::new();

    // 한글 모드 설정
    let _ = im.set_global_mode(true).await;

    // "한" 조합 (레이아웃별 키코드)
    let keys: Vec<KeyPress> = match layout {
        Layout::Sebeolsik390 => vec![
            KeyPress::new(50), // M → ㅎ
            KeyPress::new(33), // F → ㅏ
            KeyPress::new(31), // S → ㄴ종성
        ],
        Layout::Dubeolsik => vec![
            KeyPress::new(34), // G → ㅎ
            KeyPress::new(37), // K → ㅏ
            KeyPress::new(31), // S → ㄴ
        ],
    };

    let _ = ic.focus_in("").await;

    // "한" 입력
    for key in &keys {
        let _ = ic.process_key_event(0, key.keycode, key.state).await;
    }

    // Hanja 키 (evdev 123) 전송
    let hanja_result = ic.process_key_event(0, 123, 0).await;
    if verbose {
        eprintln!("  {}Hanja키 결과: {:?}{}", DIM, hanja_result, RESET);
    }

    // Pull 방식: get_hanja_candidates 호출
    match ic.get_hanja_candidates().await {
        Ok((target, candidates)) => {
            if verbose {
                eprintln!(
                    "  {}한자 후보: target=\"{}\", count={}{}",
                    DIM,
                    target,
                    candidates.len(),
                    RESET
                );
                for (i, (hanja, meaning)) in candidates.iter().take(3).enumerate() {
                    eprintln!("  {}  [{}] {} ({}){}", DIM, i, hanja, meaning, RESET);
                }
            }

            if target.is_empty() || candidates.is_empty() {
                results.push(TestResult {
                    name: "한자 후보 조회".to_string(),
                    passed: false,
                    detail: Some(format!(
                        "target=\"{}\", 후보={}개",
                        target,
                        candidates.len()
                    )),
                });
                let _ = ic.reset().await;
                return results;
            }

            results.push(TestResult {
                name: format!("한자 후보 조회 (\"{}\" → {}개)", target, candidates.len()),
                passed: true,
                detail: None,
            });

            // 첫 번째 한자 선택
            match ic.select_hanja(0).await {
                Ok(selected) => {
                    if verbose {
                        eprintln!("  {}선택된 한자: \"{}\"{}", DIM, selected, RESET);
                    }
                    let not_empty = !selected.is_empty();
                    results.push(TestResult {
                        name: format!("한자 선택 [0] → \"{}\"", selected),
                        passed: not_empty,
                        detail: if not_empty {
                            None
                        } else {
                            Some("빈 문자열 반환".to_string())
                        },
                    });
                }
                Err(e) => {
                    results.push(TestResult {
                        name: "한자 선택 [0]".to_string(),
                        passed: false,
                        detail: Some(format!("오류: {}", e)),
                    });
                }
            }
        }
        Err(e) => {
            results.push(TestResult {
                name: "한자 후보 조회".to_string(),
                passed: false,
                detail: Some(format!("오류: {}", e)),
            });
        }
    }

    let _ = ic.reset().await;

    // ── 한자 취소 테스트 ──
    // 다시 "한" 입력
    let _ = ic.focus_in("").await;
    for key in &keys {
        let _ = ic.process_key_event(0, key.keycode, key.state).await;
    }
    let _ = ic.process_key_event(0, 123, 0).await; // Hanja 키

    // 취소
    match ic.cancel_hanja().await {
        Ok(_commit) => {
            results.push(TestResult {
                name: "한자 취소".to_string(),
                passed: true,
                detail: None,
            });
        }
        Err(e) => {
            results.push(TestResult {
                name: "한자 취소".to_string(),
                passed: false,
                detail: Some(format!("오류: {}", e)),
            });
        }
    }

    let _ = ic.reset().await;
    results
}

/// 특수문자 팝업: 초성만 입력 → Hanja키 → 특수문자 후보 → 선택
async fn test_special_char_popup(
    im: &InputMethodProxy<'_>,
    ic: &InputContextProxy<'_>,
    layout: &Layout,
    verbose: bool,
) -> Vec<TestResult> {
    let mut results = Vec::new();

    // 한글 모드 설정
    let _ = im.set_global_mode(true).await;
    let _ = ic.focus_in("").await;

    // 초성만 입력: ㅎ (한자 후보가 없어서 특수문자로 전환됨)
    let choseong_key = match layout {
        Layout::Sebeolsik390 => KeyPress::new(50), // M → ㅎ
        Layout::Dubeolsik => KeyPress::new(34),    // G → ㅎ
    };

    let _ = ic
        .process_key_event(0, choseong_key.keycode, choseong_key.state)
        .await;

    // Hanja 키 → 한자 없음 → 특수문자 모드
    let hanja_result = ic.process_key_event(0, 123, 0).await;
    if verbose {
        eprintln!(
            "  {}특수문자 Hanja키 결과: {:?}{}",
            DIM, hanja_result, RESET
        );
    }

    // Pull 방식: get_special_char_candidates 호출
    match ic.get_special_char_candidates().await {
        Ok((target, characters, top_row)) => {
            if verbose {
                eprintln!(
                    "  {}특수문자 후보: target=\"{}\", count={}, top_row=\"{}\"{}",
                    DIM,
                    target,
                    characters.len(),
                    top_row,
                    RESET
                );
                for (i, ch) in characters.iter().take(5).enumerate() {
                    eprintln!("  {}  [{}] {}{}", DIM, i, ch, RESET);
                }
            }

            if target.is_empty() || characters.is_empty() {
                results.push(TestResult {
                    name: "특수문자 후보 조회".to_string(),
                    passed: false,
                    detail: Some(format!(
                        "target=\"{}\", 후보={}개",
                        target,
                        characters.len()
                    )),
                });
                let _ = ic.reset().await;
                return results;
            }

            results.push(TestResult {
                name: format!(
                    "특수문자 후보 조회 (\"{}\" → {}개)",
                    target,
                    characters.len()
                ),
                passed: true,
                detail: None,
            });

            // 첫 번째 특수문자 선택
            match ic.select_special_char(0).await {
                Ok(selected) => {
                    if verbose {
                        eprintln!("  {}선택된 특수문자: \"{}\"{}", DIM, selected, RESET);
                    }
                    let not_empty = !selected.is_empty();
                    results.push(TestResult {
                        name: format!("특수문자 선택 [0] → \"{}\"", selected),
                        passed: not_empty,
                        detail: if not_empty {
                            None
                        } else {
                            Some("빈 문자열 반환".to_string())
                        },
                    });
                }
                Err(e) => {
                    results.push(TestResult {
                        name: "특수문자 선택 [0]".to_string(),
                        passed: false,
                        detail: Some(format!("오류: {}", e)),
                    });
                }
            }
        }
        Err(e) => {
            results.push(TestResult {
                name: "특수문자 후보 조회".to_string(),
                passed: false,
                detail: Some(format!("오류: {}", e)),
            });
        }
    }

    let _ = ic.reset().await;

    // ── 특수문자 취소 테스트 ──
    let _ = ic.focus_in("").await;
    let _ = ic
        .process_key_event(0, choseong_key.keycode, choseong_key.state)
        .await;
    let _ = ic.process_key_event(0, 123, 0).await;

    match ic.cancel_special_char().await {
        Ok(_commit) => {
            results.push(TestResult {
                name: "특수문자 취소".to_string(),
                passed: true,
                detail: None,
            });
        }
        Err(e) => {
            results.push(TestResult {
                name: "특수문자 취소".to_string(),
                passed: false,
                detail: Some(format!("오류: {}", e)),
            });
        }
    }

    let _ = ic.reset().await;
    results
}

// ─── 한자 단어 변환 테스트 (U13 L2, HANJA_WORD_SPEC §6.2) ────────────────
//
// evdev 키열로 "대한민국"을 입력 — 2벌식 전용(대·한·민·국 각 3키). 세벌식 390은
// 스캔코드 매핑이 달라 지침대로 스킵 표기만 남긴다.
const HANJA_WORD_KEYS_DUBEOLSIK: [u32; 11] = [18, 24, 34, 37, 31, 30, 38, 31, 19, 49, 19];

fn hanja_word_keys(layout: &Layout) -> Option<&'static [u32]> {
    match layout {
        Layout::Dubeolsik => Some(&HANJA_WORD_KEYS_DUBEOLSIK),
        Layout::Sebeolsik390 => None,
    }
}

fn skipped(name: &str, reason: &str) -> TestResult {
    TestResult {
        name: format!("{} (SKIP)", name),
        passed: true,
        detail: Some(reason.to_string()),
    }
}

/// `skipped()`가 만든 결과인지 — SKIP은 PASS로 집계하지 않는다(총계 왜곡 방지).
fn is_skipped(r: &TestResult) -> bool {
    r.name.ends_with(" (SKIP)")
}

/// §6.2 케이스 (1): 조합 중 대상① 최장 접미 — `AutoTypefixApply` 교체,
/// `CommitText` 미발행(교체 채널 단독).
async fn hw_case1_word_replace(
    im: &InputMethodProxy<'_>,
    ic: &InputContextProxy<'_>,
    keys: &[u32],
    verbose: bool,
) -> TestResult {
    let name = "한자 단어(1) 대상① 조합 중 교체: 대한민국→大韓民國".to_string();
    let _ = im.set_global_mode(true).await;
    if let Err(e) = ic.focus_in("").await {
        return TestResult { name, passed: false, detail: Some(format!("FocusIn 실패: {}", e)) };
    }
    let _ = ic.reset().await;

    for &kc in keys {
        if let Err(e) = ic.process_key_event(0, kc, 0).await {
            let _ = ic.reset().await;
            return TestResult { name, passed: false, detail: Some(format!("타이핑 실패(kc={}): {}", kc, e)) };
        }
    }
    let (target, candidates) = match ic.get_hanja_candidates().await {
        Ok(v) => v,
        Err(e) => {
            let _ = ic.reset().await;
            return TestResult { name, passed: false, detail: Some(format!("GetHanjaCandidates 오류: {}", e)) };
        }
    };
    if verbose {
        eprintln!("  {}case1 target=\"{}\" 후보={}개{}", DIM, target, candidates.len(), RESET);
    }
    if target != "대한민국" || candidates.is_empty() {
        let _ = ic.cancel_hanja().await;
        let _ = ic.reset().await;
        return TestResult {
            name,
            passed: false,
            detail: Some(format!("target 기대=\"대한민국\" 실제=\"{}\" (후보 {}개)", target, candidates.len())),
        };
    }

    let mut atf_stream = match ic.receive_auto_typefix_apply().await {
        Ok(s) => s,
        Err(e) => {
            let _ = ic.reset().await;
            return TestResult { name, passed: false, detail: Some(format!("AutoTypefixApply 구독 실패: {}", e)) };
        }
    };
    let mut commit_stream = match ic.receive_commit_text().await {
        Ok(s) => s,
        Err(e) => {
            let _ = ic.reset().await;
            return TestResult { name, passed: false, detail: Some(format!("CommitText 구독 실패: {}", e)) };
        }
    };

    if let Err(e) = ic.select_hanja(0).await {
        let _ = ic.reset().await;
        return TestResult { name, passed: false, detail: Some(format!("SelectHanja 오류: {}", e)) };
    }

    let signal = match tokio::time::timeout(Duration::from_millis(1500), atf_stream.next()).await {
        Ok(Some(s)) => s,
        _ => {
            let _ = ic.reset().await;
            return TestResult { name, passed: false, detail: Some("AutoTypefixApply 시그널 미수신".to_string()) };
        }
    };
    let args = match signal.args() {
        Ok(a) => a,
        Err(e) => {
            let _ = ic.reset().await;
            return TestResult { name, passed: false, detail: Some(format!("시그널 역직렬화 실패: {}", e)) };
        }
    };

    let mut failures = Vec::new();
    if *args.delete_chars() != 3 {
        failures.push(format!("delete_chars 기대=3 실제={}", args.delete_chars()));
    }
    let commit_text = args.commit_text().to_string();
    if commit_text != "大韓民國" {
        failures.push(format!("commit_text 기대=\"大韓民國\" 실제=\"{}\"", commit_text));
    }
    let preedit_text = args.preedit_text().to_string();
    if !preedit_text.is_empty() {
        failures.push(format!("preedit_text 기대=\"\" 실제=\"{}\"", preedit_text));
    }

    if let Ok(Some(_)) = tokio::time::timeout(Duration::from_millis(200), commit_stream.next()).await {
        failures.push("CommitText 시그널이 함께 발행됨 — 대상①은 교체 채널 단독이어야 함".to_string());
    }

    let _ = ic.reset().await;
    TestResult { name, passed: failures.is_empty(), detail: if failures.is_empty() { None } else { Some(failures.join("; ")) } }
}

/// §6.2 케이스 (1b): 케이스 (1)과 동일한 조합·기대치를, pull(`GetHanjaCandidates`)
/// 대신 push 경로로 재현한다 — `ProcessKeyEvent(123)`이 `ShowHanjaPopup`
/// 시그널을 발행한다(`unim-dbus/src/service.rs` PopupAction::ShowHanja 처리부).
/// Wayland·GNOME 프런트가 실사용하는 경로이며, 케이스 (1)~(6)은 전부 pull
/// 단독이라 이 경로가 L2에서 한 번도 검증되지 않았다.
async fn hw_case1b_push_word_replace(
    im: &InputMethodProxy<'_>,
    ic: &InputContextProxy<'_>,
    keys: &[u32],
    verbose: bool,
) -> TestResult {
    let name = "한자 단어(1b) 대상① push 경로(ShowHanjaPopup) 조합 중 교체: 대한민국→大韓民國".to_string();
    let _ = im.set_global_mode(true).await;
    if let Err(e) = ic.focus_in("").await {
        return TestResult { name, passed: false, detail: Some(format!("FocusIn 실패: {}", e)) };
    }
    let _ = ic.reset().await;

    for &kc in keys {
        if let Err(e) = ic.process_key_event(0, kc, 0).await {
            let _ = ic.reset().await;
            return TestResult { name, passed: false, detail: Some(format!("타이핑 실패(kc={}): {}", kc, e)) };
        }
    }

    // 한자키(123)를 누르기 *전에* 구독해야 한다 — ProcessKeyEvent 응답 처리
    // 안에서 바로 시그널을 쏜다(service.rs).
    let mut popup_stream = match ic.receive_show_hanja_popup().await {
        Ok(s) => s,
        Err(e) => {
            let _ = ic.reset().await;
            return TestResult { name, passed: false, detail: Some(format!("ShowHanjaPopup 구독 실패: {}", e)) };
        }
    };
    if let Err(e) = ic.process_key_event(0, 123, 0).await {
        let _ = ic.reset().await;
        return TestResult { name, passed: false, detail: Some(format!("한자키 전달 실패: {}", e)) };
    }

    let popup_signal = match tokio::time::timeout(Duration::from_millis(1500), popup_stream.next()).await {
        Ok(Some(s)) => s,
        _ => {
            let _ = ic.reset().await;
            return TestResult { name, passed: false, detail: Some("ShowHanjaPopup 시그널 미수신".to_string()) };
        }
    };
    let popup_args = match popup_signal.args() {
        Ok(a) => a,
        Err(e) => {
            let _ = ic.reset().await;
            return TestResult { name, passed: false, detail: Some(format!("시그널 역직렬화 실패: {}", e)) };
        }
    };
    let target = popup_args.target().to_string();
    let candidates_len = popup_args.candidates().len();
    if verbose {
        eprintln!("  {}case1b target=\"{}\" 후보={}개{}", DIM, target, candidates_len, RESET);
    }
    if target != "대한민국" || candidates_len == 0 {
        let _ = ic.cancel_hanja().await;
        let _ = ic.reset().await;
        return TestResult {
            name,
            passed: false,
            detail: Some(format!("target 기대=\"대한민국\" 실제=\"{}\" (후보 {}개)", target, candidates_len)),
        };
    }

    let mut atf_stream = match ic.receive_auto_typefix_apply().await {
        Ok(s) => s,
        Err(e) => {
            let _ = ic.reset().await;
            return TestResult { name, passed: false, detail: Some(format!("AutoTypefixApply 구독 실패: {}", e)) };
        }
    };
    let mut commit_stream = match ic.receive_commit_text().await {
        Ok(s) => s,
        Err(e) => {
            let _ = ic.reset().await;
            return TestResult { name, passed: false, detail: Some(format!("CommitText 구독 실패: {}", e)) };
        }
    };

    if let Err(e) = ic.select_hanja(0).await {
        let _ = ic.reset().await;
        return TestResult { name, passed: false, detail: Some(format!("SelectHanja 오류: {}", e)) };
    }

    let signal = match tokio::time::timeout(Duration::from_millis(1500), atf_stream.next()).await {
        Ok(Some(s)) => s,
        _ => {
            let _ = ic.reset().await;
            return TestResult { name, passed: false, detail: Some("AutoTypefixApply 시그널 미수신".to_string()) };
        }
    };
    let args = match signal.args() {
        Ok(a) => a,
        Err(e) => {
            let _ = ic.reset().await;
            return TestResult { name, passed: false, detail: Some(format!("시그널 역직렬화 실패: {}", e)) };
        }
    };

    let mut failures = Vec::new();
    if *args.delete_chars() != 3 {
        failures.push(format!("delete_chars 기대=3 실제={}", args.delete_chars()));
    }
    let commit_text = args.commit_text().to_string();
    if commit_text != "大韓民國" {
        failures.push(format!("commit_text 기대=\"大韓民國\" 실제=\"{}\"", commit_text));
    }
    let preedit_text = args.preedit_text().to_string();
    if !preedit_text.is_empty() {
        failures.push(format!("preedit_text 기대=\"\" 실제=\"{}\"", preedit_text));
    }

    if let Ok(Some(_)) = tokio::time::timeout(Duration::from_millis(200), commit_stream.next()).await {
        failures.push("CommitText 시그널이 함께 발행됨 — 대상①은 교체 채널 단독이어야 함".to_string());
    }

    let _ = ic.reset().await;
    TestResult { name, passed: failures.is_empty(), detail: if failures.is_empty() { None } else { Some(failures.join("; ")) } }
}

/// §6.2 케이스 (2): 대상②(선택 영역) — 일반 `CommitText` 확정(위젯 치환 경로).
async fn hw_case2_selection_commit(
    im: &InputMethodProxy<'_>,
    ic: &InputContextProxy<'_>,
    verbose: bool,
) -> TestResult {
    let name = "한자 단어(2) 대상② 선택 확정: 대한민국→大韓民國 (CommitText)".to_string();
    let _ = im.set_global_mode(true).await;
    if let Err(e) = ic.focus_in("").await {
        return TestResult { name, passed: false, detail: Some(format!("FocusIn 실패: {}", e)) };
    }
    let _ = ic.reset().await;

    if let Err(e) = ic.set_surrounding_text("대한민국 만세", 0, 4).await {
        let _ = ic.reset().await;
        return TestResult { name, passed: false, detail: Some(format!("SetSurroundingText 실패: {}", e)) };
    }

    let mut commit_stream = match ic.receive_commit_text().await {
        Ok(s) => s,
        Err(e) => {
            let _ = ic.reset().await;
            return TestResult { name, passed: false, detail: Some(format!("CommitText 구독 실패: {}", e)) };
        }
    };

    let (target, candidates) = match ic.get_hanja_candidates().await {
        Ok(v) => v,
        Err(e) => {
            let _ = ic.reset().await;
            return TestResult { name, passed: false, detail: Some(format!("GetHanjaCandidates 오류: {}", e)) };
        }
    };
    if verbose {
        eprintln!("  {}case2 target=\"{}\" 후보={}개{}", DIM, target, candidates.len(), RESET);
    }
    if target != "대한민국" || candidates.is_empty() {
        let _ = ic.cancel_hanja().await;
        let _ = ic.reset().await;
        return TestResult {
            name,
            passed: false,
            detail: Some(format!("target 기대=\"대한민국\" 실제=\"{}\" (후보 {}개)", target, candidates.len())),
        };
    }

    if let Err(e) = ic.select_hanja(0).await {
        let _ = ic.reset().await;
        return TestResult { name, passed: false, detail: Some(format!("SelectHanja 오류: {}", e)) };
    }

    let signal = match tokio::time::timeout(Duration::from_millis(1500), commit_stream.next()).await {
        Ok(Some(s)) => s,
        _ => {
            let _ = ic.reset().await;
            return TestResult { name, passed: false, detail: Some("CommitText 시그널 미수신".to_string()) };
        }
    };
    let args = match signal.args() {
        Ok(a) => a,
        Err(e) => {
            let _ = ic.reset().await;
            return TestResult { name, passed: false, detail: Some(format!("시그널 역직렬화 실패: {}", e)) };
        }
    };
    let text = args.text().to_string();

    let _ = ic.reset().await;
    if text == "大韓民國" {
        TestResult { name, passed: true, detail: None }
    } else {
        TestResult { name, passed: false, detail: Some(format!("CommitText 기대=\"大韓民國\" 실제=\"{}\"", text)) }
    }
}

/// §6.2 케이스 (3): 불일치/비한글 선택 → 후보 없음. Q8(a) 채택(§2.1)에 따라
/// idle 이모지 팝업으로 폴백한다(실측 확인 — `HANJA_WORD_SPEC.md` §6.2 원문의
/// "이모지 팝업 시그널 미발행" 표기는 이 폴백과 어긋난다. open_issues 참조).
async fn hw_case3_unmatched_selection(
    im: &InputMethodProxy<'_>,
    ic: &InputContextProxy<'_>,
    verbose: bool,
) -> TestResult {
    let name = "한자 단어(3) 불일치 선택 → 후보 없음 + 이모지 폴백(Q8a)".to_string();
    let _ = im.set_global_mode(true).await;
    if let Err(e) = ic.focus_in("").await {
        return TestResult { name, passed: false, detail: Some(format!("FocusIn 실패: {}", e)) };
    }
    let _ = ic.reset().await;

    if let Err(e) = ic.set_surrounding_text("abc def", 0, 3).await {
        let _ = ic.reset().await;
        return TestResult { name, passed: false, detail: Some(format!("SetSurroundingText 실패: {}", e)) };
    }

    // 세 시그널 모두 pull(GetHanjaCandidates) *이전에* 구독한다 — pull 자체가
    // 잘못해서 팝업/커밋을 쏘는 회귀를 놓치지 않기 위해서다(§4.5: 불일치
    // 선택은 손대지 않아야 한다). 이후 push(한자키) 이후에도 같은 스트림을
    // 계속 지켜본다.
    let mut emoji_stream = match ic.receive_show_emoji_popup_v2().await {
        Ok(s) => s,
        Err(e) => {
            let _ = ic.reset().await;
            return TestResult { name, passed: false, detail: Some(format!("ShowEmojiPopupV2 구독 실패: {}", e)) };
        }
    };
    let mut hanja_popup_stream = match ic.receive_show_hanja_popup().await {
        Ok(s) => s,
        Err(e) => {
            let _ = ic.reset().await;
            return TestResult { name, passed: false, detail: Some(format!("ShowHanjaPopup 구독 실패: {}", e)) };
        }
    };
    let mut commit_stream = match ic.receive_commit_text().await {
        Ok(s) => s,
        Err(e) => {
            let _ = ic.reset().await;
            return TestResult { name, passed: false, detail: Some(format!("CommitText 구독 실패: {}", e)) };
        }
    };

    // Qt/XIM 실사용 순서(pull 우선, 실패 시 raw 키 재전달) 그대로 재현한다
    // (`unim-frontends/qt5/src/input_context.cpp:429-455`) — push(ProcessKeyEvent)와
    // pull(GetHanjaCandidates)을 같은 한자키에 동시에 걸면 `start_hanja_conversion()`이
    // 이미 대상② 매칭 실패로 `hanja_mode=false` 인 채 두 번 불려도 멱등이라 안전하다.
    // (참고: 매칭에 *성공*하는 케이스에서 push+pull을 같이 걸면 두 번째 호출이
    // Q7(a) "팝업 중 재타" 축소 분기를 밟는다 — case1/2/4/6이 pull 단독인 이유.)
    let (target, candidates) = match ic.get_hanja_candidates().await {
        Ok(v) => v,
        Err(e) => {
            let _ = ic.reset().await;
            return TestResult { name, passed: false, detail: Some(format!("GetHanjaCandidates 오류: {}", e)) };
        }
    };
    if verbose {
        eprintln!("  {}case3 target=\"{}\" 후보={}개{}", DIM, target, candidates.len(), RESET);
    }

    let mut failures = Vec::new();
    if !target.is_empty() || !candidates.is_empty() {
        failures.push(format!("후보 있음(기대: 없음) target=\"{}\" 후보={}개", target, candidates.len()));
    }

    // pull 그 자체는 아무 시그널도 쏘지 않아야 한다(§4.5: 선택 텍스트 불변).
    if let Ok(Some(_)) = tokio::time::timeout(Duration::from_millis(200), emoji_stream.next()).await {
        failures.push("ShowEmojiPopupV2 이 pull(GetHanjaCandidates) 단계에서 조기 발행됨".to_string());
    }
    if let Ok(Some(_)) = tokio::time::timeout(Duration::from_millis(200), hanja_popup_stream.next()).await {
        failures.push("ShowHanjaPopup 이 pull(GetHanjaCandidates) 단계에서 발행됨".to_string());
    }
    if let Ok(Some(_)) = tokio::time::timeout(Duration::from_millis(200), commit_stream.next()).await {
        failures.push("CommitText 가 pull(GetHanjaCandidates) 단계에서 발행됨".to_string());
    }

    if let Err(e) = ic.process_key_event(0, 123, 0).await {
        let _ = ic.reset().await;
        return TestResult { name, passed: false, detail: Some(format!("Hanja키(폴백 전달) 실패: {}", e)) };
    }

    let emoji_ok = tokio::time::timeout(Duration::from_millis(1000), emoji_stream.next()).await;
    if !matches!(emoji_ok, Ok(Some(_))) {
        failures.push("ShowEmojiPopupV2 미수신 — Q8(a) idle 폴백 기대".to_string());
    }
    // 한자 팝업/일반 커밋은 이 폴백 경로에서 나오면 안 된다(§4.5: 선택 텍스트
    // 미변경 — ShowHanjaPopup·CommitText 둘 다 발행 금지).
    if let Ok(Some(_)) = tokio::time::timeout(Duration::from_millis(200), hanja_popup_stream.next()).await {
        failures.push("ShowHanjaPopup 이 한자키 폴백 이후 발행됨 — 선택 텍스트가 손대짐".to_string());
    }
    if let Ok(Some(_)) = tokio::time::timeout(Duration::from_millis(200), commit_stream.next()).await {
        failures.push("CommitText 가 한자키 폴백 이후 발행됨 — 선택 텍스트가 손대짐".to_string());
    }

    let _ = ic.cancel_hanja().await;
    let _ = ic.reset().await;
    TestResult { name, passed: failures.is_empty(), detail: if failures.is_empty() { None } else { Some(failures.join("; ")) } }
}

/// §6.2 케이스 (4, 선택): `hanja_output_format=HangulHanja` → `대한민국(大韓民國)`.
/// `GetConfigYaml` → 키 패치 → `SetConfigYaml`, 종료 시 원문 YAML로 원복.
async fn hw_case4_output_format(
    im: &InputMethodProxy<'_>,
    ic: &InputContextProxy<'_>,
    keys: &[u32],
    verbose: bool,
) -> TestResult {
    let name = "한자 단어(4) hanja_output_format=HangulHanja → 대한민국(大韓民國)".to_string();

    // 이 케이스만 실세션 config.yaml을 SetConfigYaml로 고쳐 쓴다 — 격리 데몬
    // (UNIM_TEST_ISOLATED=1)에서만 실행한다. 파일 상단 문서 주석의 레시피 참조.
    if std::env::var("UNIM_TEST_ISOLATED").as_deref() != Ok("1") {
        return skipped(
            &name,
            "UNIM_TEST_ISOLATED=1 미설정 — 실세션 데몬 config.yaml 훼손 방지를 위해 격리 데몬에서만 실행(파일 상단 문서 주석 레시피 참조)",
        );
    }

    let original = match im.get_config_yaml().await {
        Ok(y) => y,
        Err(e) => return TestResult { name, passed: false, detail: Some(format!("GetConfigYaml 실패: {}", e)) },
    };
    if !original.contains("hanja_output_format: Hanja\n") {
        return TestResult {
            name,
            passed: false,
            detail: Some("YAML에서 'hanja_output_format: Hanja' 기본값 라인을 찾을 수 없음".to_string()),
        };
    }
    let patched = original.replacen("hanja_output_format: Hanja\n", "hanja_output_format: HangulHanja\n", 1);

    let result = hw_case4_inner(im, ic, &patched, keys, verbose).await;

    // 원복 — 결과와 무관하게 항상 되돌린다.
    if let Err(e) = im.set_config_yaml(&original).await {
        return TestResult {
            name: result.name,
            passed: false,
            detail: Some(format!("{} + [설정 원복 실패: {}]", result.detail.unwrap_or_default(), e)),
        };
    }
    // 엔진 워커의 `Config::reload_if_changed()`는 마지막 확인(직전 요청들로 방금
    // 갱신됨) 이후 2초 미만이면 파일을 다시 읽지 않는다(`src/config.rs`). 원복이
    // 실제로 적용됐는지 보장하려면 그 창을 넘겨야 한다.
    tokio::time::sleep(Duration::from_millis(2100)).await;

    // 원복 검증 — SetConfigYaml 호출이 성공해도 실제로 원본과 같은 내용으로
    // 저장됐는지는 별개다. GetConfigYaml을 재호출해 `original`(이미 같은
    // serde 직렬화를 한 번 거친 값)과 바이트 단위로 비교한다.
    match im.get_config_yaml().await {
        Ok(restored) if restored == original => result,
        Ok(restored) => TestResult {
            name: result.name,
            passed: false,
            detail: Some(format!(
                "{} + [설정 원복 검증 실패: 재조회 결과가 원본과 다름 (길이 원본={} 재조회={})]",
                result.detail.unwrap_or_default(),
                original.len(),
                restored.len()
            )),
        },
        Err(e) => TestResult {
            name: result.name,
            passed: false,
            detail: Some(format!("{} + [원복 검증용 GetConfigYaml 실패: {}]", result.detail.unwrap_or_default(), e)),
        },
    }
}

async fn hw_case4_inner(
    im: &InputMethodProxy<'_>,
    ic: &InputContextProxy<'_>,
    patched_yaml: &str,
    keys: &[u32],
    verbose: bool,
) -> TestResult {
    let name = "한자 단어(4) hanja_output_format=HangulHanja → 대한민국(大韓民國)".to_string();

    if let Err(e) = im.set_config_yaml(patched_yaml).await {
        return TestResult { name, passed: false, detail: Some(format!("SetConfigYaml 실패: {}", e)) };
    }
    // 위와 동일한 2초 throttle 회피 — 이전 케이스들의 잦은 요청이 `last_checked`
    // 를 방금 갱신해 놨을 수 있어 300ms로는 부족했다(실측: 서식 미적용 FAIL).
    tokio::time::sleep(Duration::from_millis(2100)).await;

    let _ = im.set_global_mode(true).await;
    if let Err(e) = ic.focus_in("").await {
        return TestResult { name, passed: false, detail: Some(format!("FocusIn 실패: {}", e)) };
    }
    let _ = ic.reset().await;

    for &kc in keys {
        if let Err(e) = ic.process_key_event(0, kc, 0).await {
            let _ = ic.reset().await;
            return TestResult { name, passed: false, detail: Some(format!("타이핑 실패(kc={}): {}", kc, e)) };
        }
    }
    let (target, candidates) = match ic.get_hanja_candidates().await {
        Ok(v) => v,
        Err(e) => {
            let _ = ic.reset().await;
            return TestResult { name, passed: false, detail: Some(format!("GetHanjaCandidates 오류: {}", e)) };
        }
    };
    if target != "대한민국" || candidates.is_empty() {
        let _ = ic.cancel_hanja().await;
        let _ = ic.reset().await;
        return TestResult { name, passed: false, detail: Some(format!("target 기대=\"대한민국\" 실제=\"{}\"", target)) };
    }

    let mut atf_stream = match ic.receive_auto_typefix_apply().await {
        Ok(s) => s,
        Err(e) => {
            let _ = ic.reset().await;
            return TestResult { name, passed: false, detail: Some(format!("AutoTypefixApply 구독 실패: {}", e)) };
        }
    };

    if let Err(e) = ic.select_hanja(0).await {
        let _ = ic.reset().await;
        return TestResult { name, passed: false, detail: Some(format!("SelectHanja 오류: {}", e)) };
    }

    let signal = match tokio::time::timeout(Duration::from_millis(1500), atf_stream.next()).await {
        Ok(Some(s)) => s,
        _ => {
            let _ = ic.reset().await;
            return TestResult { name, passed: false, detail: Some("AutoTypefixApply 시그널 미수신".to_string()) };
        }
    };
    let args = match signal.args() {
        Ok(a) => a,
        Err(e) => {
            let _ = ic.reset().await;
            return TestResult { name, passed: false, detail: Some(format!("시그널 역직렬화 실패: {}", e)) };
        }
    };
    let commit_text = args.commit_text().to_string();
    if verbose {
        eprintln!("  {}case4 commit_text=\"{}\"{}", DIM, commit_text, RESET);
    }

    let _ = ic.reset().await;
    if commit_text == "대한민국(大韓民國)" {
        TestResult { name, passed: true, detail: None }
    } else {
        TestResult {
            name,
            passed: false,
            detail: Some(format!("commit_text 기대=\"대한민국(大韓民國)\" 실제=\"{}\"", commit_text)),
        }
    }
}

/// §6.2 케이스 (5): 팝업 열림 중 `SetContentType(Password)` → `HidePopup` 수신,
/// `CommitText` 미발행(비번 필드 재커밋 금지, §2.8).
async fn hw_case5_password_gate(
    im: &InputMethodProxy<'_>,
    ic: &InputContextProxy<'_>,
    layout: &Layout,
    verbose: bool,
) -> TestResult {
    let name = "한자 단어(5) 팝업 중 SetContentType(Password) → HidePopup, CommitText 없음".to_string();
    let _ = im.set_global_mode(true).await;
    if let Err(e) = ic.focus_in("").await {
        return TestResult { name, passed: false, detail: Some(format!("FocusIn 실패: {}", e)) };
    }
    let _ = ic.set_content_type(0).await; // Normal 로 리셋(이전 케이스 잔류 방지)
    let _ = ic.reset().await;

    // "한" 음절만 조합 — 팝업이 열리기만 하면 되므로 단어 변환은 불필요.
    let keys: &[u32] = match layout {
        Layout::Dubeolsik => &[34, 37, 31],
        Layout::Sebeolsik390 => &[50, 33, 31],
    };
    for &kc in keys {
        if let Err(e) = ic.process_key_event(0, kc, 0).await {
            let _ = ic.reset().await;
            return TestResult { name, passed: false, detail: Some(format!("타이핑 실패(kc={}): {}", kc, e)) };
        }
    }
    let (target, candidates) = match ic.get_hanja_candidates().await {
        Ok(v) => v,
        Err(e) => {
            let _ = ic.reset().await;
            return TestResult { name, passed: false, detail: Some(format!("GetHanjaCandidates 오류: {}", e)) };
        }
    };
    if verbose {
        eprintln!("  {}case5 target=\"{}\" 후보={}개{}", DIM, target, candidates.len(), RESET);
    }
    if target.is_empty() || candidates.is_empty() {
        let _ = ic.reset().await;
        return TestResult { name, passed: false, detail: Some("팝업이 열리지 않음(후보 없음)".to_string()) };
    }

    let mut hide_stream = match ic.receive_hide_popup().await {
        Ok(s) => s,
        Err(e) => {
            let _ = ic.reset().await;
            return TestResult { name, passed: false, detail: Some(format!("HidePopup 구독 실패: {}", e)) };
        }
    };
    let mut commit_stream = match ic.receive_commit_text().await {
        Ok(s) => s,
        Err(e) => {
            let _ = ic.reset().await;
            return TestResult { name, passed: false, detail: Some(format!("CommitText 구독 실패: {}", e)) };
        }
    };

    if let Err(e) = ic.set_content_type(1).await {
        // 1 = ContentPurpose::Password
        let _ = ic.reset().await;
        return TestResult { name, passed: false, detail: Some(format!("SetContentType 실패: {}", e)) };
    }

    let mut failures = Vec::new();
    let hide_ok = tokio::time::timeout(Duration::from_millis(1500), hide_stream.next()).await;
    if !matches!(hide_ok, Ok(Some(_))) {
        failures.push("HidePopup 시그널 미수신".to_string());
    }
    if let Ok(Some(_)) = tokio::time::timeout(Duration::from_millis(200), commit_stream.next()).await {
        failures.push("CommitText 발행됨(비번 필드 재커밋 금지 위반)".to_string());
    }

    let _ = ic.set_content_type(0).await;
    let _ = ic.reset().await;
    TestResult { name, passed: failures.is_empty(), detail: if failures.is_empty() { None } else { Some(failures.join("; ")) } }
}

/// §6.2 케이스 (6): AutoTypeFix 순방향 교정("eogksals"→"대한민") 직후 최근 확정
/// 음절 버퍼가 시드되어, 바로 이어친 "국"+한자키가 4음절 target "대한민국"으로 이어진다.
async fn hw_case6_atf_seed(
    im: &InputMethodProxy<'_>,
    ic: &InputContextProxy<'_>,
    verbose: bool,
) -> TestResult {
    let name = "한자 단어(6) ATF 순방향 시드 → target==대한민국".to_string();
    // 영문 모드로 "eogksals"(2벌식 "대한민"의 로마자 오타) 타이핑 — ATF 순방향이
    // 한글로 교정하며 한글 모드로 자동 전환한다(§2.2.1 최근 확정 음절 버퍼 흡수).
    if let Err(e) = ic.focus_in("").await {
        return TestResult { name, passed: false, detail: Some(format!("FocusIn 실패: {}", e)) };
    }
    let _ = ic.reset().await;
    let _ = im.set_global_mode(false).await;

    // ATF 발동 확인은 `GetGlobalMode` 가 아니라 `AutoTypefixApply` 시그널로 한다 —
    // `GetGlobalMode`(InputMethodService::global_mode)는 `SetGlobalMode` RPC로만
    // 갱신되고 ProcessKeyEvent 응답의 `mode_changed`(ATF 자동 전환 포함)는 그 상태를
    // 갱신하지 않은 채 `GlobalModeChanged` 시그널만 쏜다(service.rs:476 vs :2028-2034
    // 실측 — 자동 전환 뒤 `GetGlobalMode` 가 계속 이전 값을 돌려준다). 이 어긋남은
    // U13 소유 파일 밖이라 여기서 고치지 않고 시그널 기반 검증으로 우회한다.
    let mut atf_stream = match ic.receive_auto_typefix_apply().await {
        Ok(s) => s,
        Err(e) => {
            let _ = ic.reset().await;
            return TestResult { name, passed: false, detail: Some(format!("AutoTypefixApply 구독 실패: {}", e)) };
        }
    };

    // e o g k s a l s → 2벌식 "대한민" 오타 (evdev 18 24 34 37 31 30 38 31). ATF는
    // 자모 단위로 즉시 평가되므로 보통 4키("eogk"→"대하") 만에 발동하고, 나머지
    // 키는 이미 한글 모드로 전환된 채 자연 조합으로 이어져 "한","민"을 완성한다
    // (실측: delete=4, corrected='대하').
    let atf_keys = [18u32, 24, 34, 37, 31, 30, 38, 31];
    for &kc in &atf_keys {
        match ic.process_key_event(0, kc, 0).await {
            Ok(r) => {
                if verbose {
                    eprintln!("  {}case6 kc={} -> {:?}{}", DIM, kc, r, RESET);
                }
            }
            Err(e) => {
                let _ = ic.reset().await;
                return TestResult { name, passed: false, detail: Some(format!("타이핑 실패(kc={}): {}", kc, e)) };
            }
        }
    }

    let atf_fired = tokio::time::timeout(Duration::from_millis(800), atf_stream.next()).await;
    if !matches!(atf_fired, Ok(Some(_))) {
        let _ = ic.reset().await;
        return TestResult { name, passed: false, detail: Some("ATF 순방향 교정 미발동(AutoTypefixApply 시그널 미수신)".to_string()) };
    }

    // 이어서 "국" (2벌식: ㄱㅜㄱ = R N R = 19 49 19)
    for &kc in &[19u32, 49, 19] {
        if let Err(e) = ic.process_key_event(0, kc, 0).await {
            let _ = ic.reset().await;
            return TestResult { name, passed: false, detail: Some(format!("타이핑 실패(kc={}): {}", kc, e)) };
        }
    }
    let (target, candidates) = match ic.get_hanja_candidates().await {
        Ok(v) => v,
        Err(e) => {
            let _ = ic.reset().await;
            return TestResult { name, passed: false, detail: Some(format!("GetHanjaCandidates 오류: {}", e)) };
        }
    };
    if verbose {
        eprintln!("  {}case6 target=\"{}\" 후보={}개{}", DIM, target, candidates.len(), RESET);
    }

    let _ = ic.cancel_hanja().await;
    let _ = ic.reset().await;

    if target == "대한민국" && !candidates.is_empty() {
        TestResult { name, passed: true, detail: None }
    } else {
        TestResult {
            name,
            passed: false,
            detail: Some(format!("target 기대=\"대한민국\" 실제=\"{}\" (후보 {}개)", target, candidates.len())),
        }
    }
}

/// U13 L2 — `HANJA_WORD_SPEC.md` §6.2 케이스 전량. 세벌식 390은 evdev 키맵
/// 미검증으로 조합 의존 케이스를 스킵하고 그 사실을 결과에 남긴다.
async fn test_hanja_word(
    im: &InputMethodProxy<'_>,
    ic: &InputContextProxy<'_>,
    layout: &Layout,
    verbose: bool,
) -> Vec<TestResult> {
    let mut results = Vec::new();

    let Some(keys) = hanja_word_keys(layout) else {
        results.push(skipped(
            "한자 단어(1,2,3,4,6) 조합 키열",
            "세벌식 390 evdev 키맵 미검증 — 지침대로 2벌식 전용, 스킵 표기",
        ));
        results.push(hw_case5_password_gate(im, ic, layout, verbose).await);
        return results;
    };

    results.push(hw_case1_word_replace(im, ic, keys, verbose).await);
    results.push(hw_case1b_push_word_replace(im, ic, keys, verbose).await);
    results.push(hw_case2_selection_commit(im, ic, verbose).await);
    results.push(hw_case3_unmatched_selection(im, ic, verbose).await);
    results.push(hw_case4_output_format(im, ic, keys, verbose).await);
    results.push(hw_case5_password_gate(im, ic, layout, verbose).await);
    results.push(hw_case6_atf_seed(im, ic, verbose).await);

    results
}

#[tokio::main]
async fn main() {
    let verbose = std::env::args().any(|a| a == "--verbose" || a == "-v");

    println!("{}═══ UNIM DBus 자동 테스트 ═══{}", BOLD, RESET);
    println!();

    // 데몬 연결
    let client = match UnimClient::connect().await {
        Ok(c) => c,
        Err(e) => {
            eprintln!("{}❌ 데몬 연결 실패: {}{}", RED, e, RESET);
            eprintln!("데몬이 실행 중인지 확인: UNIM_DEVELOP=1 unim-daemon -n --replace &");
            std::process::exit(1);
        }
    };

    // 레이아웃 감지
    let layout = match client.input_method().await {
        Ok(im) => match im.get_config("korean_layout").await {
            Ok(name) => {
                let l = Layout::from_name(&name);
                println!("감지된 레이아웃: {} ({})", l.display_name(), name);
                l
            }
            Err(_) => {
                println!(
                    "{}레이아웃 감지 실패 — 2벌식으로 기본 설정{}",
                    YELLOW, RESET
                );
                Layout::Dubeolsik
            }
        },
        Err(_) => Layout::Dubeolsik,
    };
    println!();

    // 테스트용 컨텍스트 생성
    let ctx_path = match client.create_context("dbus-test", "").await {
        Ok(path) => {
            if verbose {
                eprintln!("{}컨텍스트 생성: {}{}", DIM, path, RESET);
            }
            path
        }
        Err(e) => {
            eprintln!("{}❌ 컨텍스트 생성 실패: {}{}", RED, e, RESET);
            std::process::exit(1);
        }
    };

    // 시나리오 수집: 레이아웃별 + 공통
    let mut all_scenarios = scenarios_for_layout(&layout);
    all_scenarios.extend(common_scenarios());

    let total = all_scenarios.len();
    let mut passed = 0;
    let mut failed = 0;
    let mut results = Vec::new();

    for scenario in &all_scenarios {
        let result = run_scenario(&client, &ctx_path, scenario, verbose).await;
        if result.passed {
            passed += 1;
            println!("  {}PASS{} {}", GREEN, RESET, result.name);
        } else {
            failed += 1;
            println!("  {}FAIL{} {}", RED, RESET, result.name);
            if let Some(ref detail) = result.detail {
                println!("       {}{}{}", DIM, detail, RESET);
            }
        }
        results.push(result);
    }

    // ── 한자/특수문자 팝업 테스트 ──
    println!();
    println!("{}── 팝업 테스트 ──{}", BOLD, RESET);

    let im = client.input_method().await.unwrap();
    let ic = client.input_context(&ctx_path).await.unwrap();

    let hanja_results = test_hanja_popup(&im, &ic, &layout, verbose).await;
    for r in &hanja_results {
        if r.passed {
            passed += 1;
            println!("  {}PASS{} {}", GREEN, RESET, r.name);
        } else {
            failed += 1;
            println!("  {}FAIL{} {}", RED, RESET, r.name);
            if let Some(ref d) = r.detail {
                println!("       {}{}{}", DIM, d, RESET);
            }
        }
    }
    let popup_count = hanja_results.len();
    results.extend(hanja_results);

    let special_results = test_special_char_popup(&im, &ic, &layout, verbose).await;
    for r in &special_results {
        if r.passed {
            passed += 1;
            println!("  {}PASS{} {}", GREEN, RESET, r.name);
        } else {
            failed += 1;
            println!("  {}FAIL{} {}", RED, RESET, r.name);
            if let Some(ref d) = r.detail {
                println!("       {}{}{}", DIM, d, RESET);
            }
        }
    }
    let popup_count = popup_count + special_results.len();
    results.extend(special_results);

    // ── 한자 단어 변환 테스트 (U13 L2) ──
    println!();
    println!("{}── 한자 단어 변환 테스트 ──{}", BOLD, RESET);

    let hanja_word_results = test_hanja_word(&im, &ic, &layout, verbose).await;
    let mut hanja_word_skipped = 0;
    for r in &hanja_word_results {
        if is_skipped(r) {
            hanja_word_skipped += 1;
            println!("  {}SKIP{} {}", YELLOW, RESET, r.name);
        } else if r.passed {
            passed += 1;
            println!("  {}PASS{} {}", GREEN, RESET, r.name);
        } else {
            failed += 1;
            println!("  {}FAIL{} {}", RED, RESET, r.name);
        }
        if let Some(ref d) = r.detail {
            println!("       {}{}{}", DIM, d, RESET);
        }
    }
    // SKIP은 passed/total 어디에도 넣지 않는다 — "ALL PASS (n/n)"이 스킵을
    // 통과로 세는 착시를 막기 위해서다.
    let popup_count = popup_count + hanja_word_results.len() - hanja_word_skipped;
    results.extend(hanja_word_results);

    let total = total + popup_count;

    // 컨텍스트 정리
    let _ = ic.destroy().await;

    // 결과 요약
    println!();
    if failed == 0 {
        println!(
            "{}{}✅ ALL PASS{} ({}/{})",
            GREEN, BOLD, RESET, passed, total
        );
    } else {
        println!(
            "{}{}❌ {} FAILED{} / {} passed / {} total",
            RED, BOLD, failed, RESET, passed, total
        );
        println!();
        println!("{}실패 목록:{}", YELLOW, RESET);
        for r in &results {
            if !r.passed {
                println!("  - {}", r.name);
                if let Some(ref d) = r.detail {
                    println!("    {}", d);
                }
            }
        }
    }

    std::process::exit(if failed > 0 { 1 } else { 0 });
}
