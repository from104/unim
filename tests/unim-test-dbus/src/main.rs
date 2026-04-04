//! UNIM DBus 자동 회귀 테스트
//!
//! 실행 중인 unim-daemon에 DBus로 연결하여
//! 한글 조합 시나리오를 자동 검증합니다.
//!
//! Usage: unim-test-dbus [--verbose]

mod scenarios;

use scenarios::{common_scenarios, scenarios_for_layout, KeyPress, Layout, TestScenario};
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

    if !scenario.expected_total_commit.is_empty()
        && total_commit != scenario.expected_total_commit
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
                    "  {}한자 후보: target=\"{}\", count={}{}", DIM, target, candidates.len(), RESET
                );
                for (i, (hanja, meaning)) in candidates.iter().take(3).enumerate() {
                    eprintln!("  {}  [{}] {} ({}){}", DIM, i, hanja, meaning, RESET);
                }
            }

            if target.is_empty() || candidates.is_empty() {
                results.push(TestResult {
                    name: "한자 후보 조회".to_string(),
                    passed: false,
                    detail: Some(format!("target=\"{}\", 후보={}개", target, candidates.len())),
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
                        detail: if not_empty { None } else { Some("빈 문자열 반환".to_string()) },
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

    let _ = ic.process_key_event(0, choseong_key.keycode, choseong_key.state).await;

    // Hanja 키 → 한자 없음 → 특수문자 모드
    let hanja_result = ic.process_key_event(0, 123, 0).await;
    if verbose {
        eprintln!("  {}특수문자 Hanja키 결과: {:?}{}", DIM, hanja_result, RESET);
    }

    // Pull 방식: get_special_char_candidates 호출
    match ic.get_special_char_candidates().await {
        Ok((target, characters, top_row)) => {
            if verbose {
                eprintln!(
                    "  {}특수문자 후보: target=\"{}\", count={}, top_row=\"{}\"{}",
                    DIM, target, characters.len(), top_row, RESET
                );
                for (i, ch) in characters.iter().take(5).enumerate() {
                    eprintln!("  {}  [{}] {}{}", DIM, i, ch, RESET);
                }
            }

            if target.is_empty() || characters.is_empty() {
                results.push(TestResult {
                    name: "특수문자 후보 조회".to_string(),
                    passed: false,
                    detail: Some(format!("target=\"{}\", 후보={}개", target, characters.len())),
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
                println!("{}레이아웃 감지 실패 — 2벌식으로 기본 설정{}", YELLOW, RESET);
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
