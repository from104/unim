//! 엔진 워커 모듈
//!
//! `InputEngine`은 `Send + Sync`를 구현하지 않으므로,
//! 별도의 전용 스레드에서 실행되어야 합니다.
//! 이 모듈은 엔진 스레드 실행 및 요청 처리를 담당합니다.

use std::collections::HashMap;
use std::thread;

use tokio::sync::mpsc;

use crate::service::{EngineRequest, EngineResponse};
use unim::auto_typefix::{self, KeystrokeBuffer};
use unim::config::Config;
use unim::input_engine::InputEngine;
use unim::keycode::{KeyCode, ModifierState};
use unim::unim_log;

/// window_id에서 앱 식별자를 추출합니다.
/// 형식: "app_name:window_specific_id" → "app_name"
/// ':' 가 없으면 window_id 전체를 app_id로 사용합니다.
fn extract_app_id(window_id: &str) -> &str {
    window_id.split(':').next().unwrap_or(window_id)
}

/// 엔진 워커를 시작하고 요청 수신 채널을 반환합니다.
///
/// # Returns
///
/// 엔진 워커에게 요청을 보낼 수 있는 `mpsc::Sender`
pub fn spawn_engine_worker(config: Config) -> mpsc::Sender<EngineRequest> {
    let (tx, rx) = mpsc::channel::<EngineRequest>(256);

    thread::spawn(move || {
        run_engine_worker(rx, config);
    });

    tx
}

/// 엔진 워커 메인 루프 (블로킹)
fn run_engine_worker(mut rx: mpsc::Receiver<EngineRequest>, mut config: Config) {
    let mut contexts: HashMap<u32, InputEngine> = HashMap::new();
    // 앱별 모드 저장: app_id -> InputCategory (PerApp 모드용)
    let mut app_modes: HashMap<String, unim::config::InputCategory> = HashMap::new();
    // 컨텍스트 -> 창 ID 매핑
    let mut context_windows: HashMap<u32, String> = HashMap::new();
    // 마지막 포커스된 컨텍스트 ID (글로벌 TypeFix용)
    let mut last_focused_context_id: Option<u32> = None;
    // AutoTypeFix: 컨텍스트별 키스트로크 버퍼
    let mut keystroke_buffers: HashMap<u32, KeystrokeBuffer> = HashMap::new();
    // AutoTypeFix: 마지막 교정 결과 (Ctrl+Z 되돌리기용)
    // (delete_chars, corrected_text, original_text)
    let mut last_autofix: HashMap<u32, (u32, String, String)> = HashMap::new();

    unim_log!("ENGINE_WORKER", "[Engine Worker] 시작됨");

    // 블로킹으로 요청 수신 (tokio 런타임 밖에서 실행)
    while let Some(request) = rx.blocking_recv() {
        // 설정 파일 변경 여부 확인 및 리로드 (Throttling 적용됨)
        if config.reload_if_changed() {
            unim_log!(
                "ENGINE_WORKER",
                "[Engine Worker] 설정 파일 변경 감지 - 리로드 완료"
            );
            // 기존 엔진들의 레이아웃도 업데이트
            for engine in contexts.values_mut() {
                engine.set_korean_layout(config.engine.korean.layout);
                engine.set_english_layout(config.engine.english.layout);
            }
        }

        match request {
            EngineRequest::CreateContext {
                id,
                window_id,
                response,
            } => {
                let mut engine = InputEngine::new(&config);

                // PerApp 모드에서는 앱별 저장된 모드 적용
                if config.engine.mode_sharing == unim::config::ModeSharingMode::PerApp {
                    let app_id = extract_app_id(&window_id);
                    if let Some(&saved_mode) = app_modes.get(app_id) {
                        engine.set_input_category(saved_mode);
                        unim_log!(
                            "ENGINE_WORKER",
                            "[Engine Worker] 앱별 모드 복원: app_id={}, mode={:?}",
                            app_id,
                            saved_mode
                        );
                    }
                }

                context_windows.insert(id, window_id);
                contexts.insert(id, engine);
                unim_log!("ENGINE_WORKER", "[Engine Worker] 컨텍스트 생성: id={}", id);
                let _ = response.send(());
            }

            EngineRequest::DestroyContext { id } => {
                contexts.remove(&id);
                unim_log!("ENGINE_WORKER", "[Engine Worker] 컨텍스트 파괴: id={}", id);
            }

            EngineRequest::ProcessKey {
                context_id,
                keyval: _,
                keycode,
                state,
                response,
            } => {
                // 팝업 바이패스: 다른 context에 한자/특수문자 팝업이 활성이고
                // 현재 context의 윈도우가 unim-gui (인디케이터)이면 키를 소비하지 않음
                // → GNOME extension이 consumed=false를 받아 키를 GTK 팝업 윈도우로 전달
                let popup_bypass = contexts.iter().any(|(&id, engine)| {
                    id != context_id && (engine.is_hanja_mode() || engine.is_special_char_mode())
                }) && context_windows
                    .get(&context_id)
                    .map(|wid| wid.starts_with("unim-gui-"))
                    .unwrap_or(false);

                if popup_bypass {
                    unim_log!(
                        "ENGINE_WORKER",
                        "[Engine Worker] ProcessKey 바이패스 (팝업 활성, unim-gui 윈도우): context_id={}",
                        context_id
                    );
                    let _ = response.send(EngineResponse {
                        consumed: false,
                        preedit: None,
                        commit: None,
                        mode_changed: None,
                        popup_action: None,
                        auto_typefix: None,
                    });
                    continue;
                }

                let resp = if let Some(engine) = contexts.get_mut(&context_id) {
                    // keycode를 KeyCode로 변환
                    let key = KeyCode::from_evdev_keycode(keycode as u16);
                    let modifier = ModifierState::from_x11_mask(state);
                    let atf_config = &config.engine.auto_typefix;

                    // Ctrl+Z: AutoTypeFix 되돌리기
                    if key == KeyCode::Z
                        && modifier.control
                        && !modifier.shift
                        && !modifier.alt
                    {
                        if let Some((_del, corrected, original)) =
                            last_autofix.remove(&context_id)
                        {
                            let delete_chars = corrected.chars().count() as u32;
                            keystroke_buffers.remove(&context_id);

                            unim_log!(
                                "ENGINE_WORKER",
                                "[Engine Worker] AutoTypeFix 되돌리기: '{}' → '{}'",
                                corrected,
                                original
                            );

                            let _ = response.send(EngineResponse {
                                consumed: true,
                                preedit: None,
                                commit: None,
                                mode_changed: None,
                                popup_action: None,
                                auto_typefix: Some((delete_chars, original, String::new())),
                            });
                            continue;
                        }
                    }

                    // 처리 전 상태 저장
                    let prev_mode = engine.input_category();

                    // 키 처리
                    let result = engine.press_key(key, modifier, &config);

                    // 모드 변경 감지
                    let current_mode = engine.input_category();
                    let mut mode_changed = if prev_mode != current_mode {
                        // 모드 변경 시 버퍼 초기화
                        keystroke_buffers.remove(&context_id);
                        last_autofix.remove(&context_id);

                        // PerApp 모드에서는 앱별 모드 저장
                        if config.engine.mode_sharing == unim::config::ModeSharingMode::PerApp {
                            if let Some(window_id) = context_windows.get(&context_id) {
                                let app_id = extract_app_id(window_id);
                                app_modes.insert(app_id.to_string(), current_mode);
                                unim_log!(
                                    "ENGINE_WORKER",
                                    "[Engine Worker] 앱별 모드 저장: app_id={}, mode={:?}",
                                    app_id,
                                    current_mode
                                );
                            }
                        }
                        Some(current_mode == unim::config::InputCategory::Korean)
                    } else {
                        None
                    };

                    // 응답 생성
                    let mut preedit = if result.preedit_changed {
                        Some(engine.preedit_str().to_string())
                    } else {
                        None
                    };

                    let commit = if result.commit_changed {
                        let s = engine.commit_str().to_string();
                        engine.clear_commit();
                        if !s.is_empty() {
                            Some(s)
                        } else {
                            None
                        }
                    } else {
                        None
                    };

                    // 팝업 동작 감지
                    let popup_action = engine.take_popup_action();

                    // === AutoTypeFix: 키스트로크 버퍼 기반 감지 ===
                    let auto_typefix_result = if atf_config.enabled
                        && mode_changed.is_none()
                        && popup_action.is_none()
                    {
                        let buf = keystroke_buffers
                            .entry(context_id)
                            .or_insert_with(KeystrokeBuffer::new);

                        // 알파벳 키면 버퍼에 추가
                        if buf.push(key, modifier) {
                            // 시간 윈도우 밖 엔트리 제거
                            buf.expire(atf_config.time_window_ms);

                            // commit/preedit 추적 (방향 B용)
                            if let Some(ref c) = commit {
                                buf.update_on_commit(c);
                            }
                            if let Some(ref p) = preedit {
                                buf.update_on_preedit(p);
                            } else if commit.is_some() {
                                // commit은 있지만 preedit 변경 없음
                                // (preedit이 이전 값 유지 — 변경 없으므로 건드리지 않음)
                            }

                            // 방향에 따라 감지
                            let fix = match current_mode {
                                unim::config::InputCategory::English => {
                                    auto_typefix::check_direction_a(
                                        buf,
                                        atf_config,
                                        config.engine.korean.layout,
                                        config.engine.english.layout,
                                    )
                                }
                                unim::config::InputCategory::Korean => {
                                    auto_typefix::check_direction_b(buf, atf_config)
                                }
                            };

                            if let Some(ref fix) = fix {
                                // 되돌리기용 저장
                                last_autofix.insert(
                                    context_id,
                                    (fix.delete_chars, fix.corrected.clone(), fix.original.clone()),
                                );

                                unim_log!(
                                    "ENGINE_WORKER",
                                    "[Engine Worker] AutoTypeFix: delete={}, corrected='{}', clear_preedit={}",
                                    fix.delete_chars,
                                    fix.corrected,
                                    fix.clear_preedit
                                );

                                // 교정 후 모드 전환: 영→한이면 한글로, 한→영이면 영어로
                                let new_mode = match current_mode {
                                    unim::config::InputCategory::English => unim::config::InputCategory::Korean,
                                    unim::config::InputCategory::Korean => unim::config::InputCategory::English,
                                };
                                engine.set_input_category(new_mode);
                                config.engine.default_category = new_mode;
                                // Global 동기화는 contexts borrow 해제 후 별도 처리
                                mode_changed = Some(new_mode == unim::config::InputCategory::Korean);

                                unim_log!(
                                    "ENGINE_WORKER",
                                    "[Engine Worker] AutoTypeFix 모드 전환: {:?}",
                                    new_mode
                                );

                                // 교정 후 버퍼 초기화
                                buf.clear();

                                // 방향 A: 마지막 음절을 엔진에 replay하여 preedit 상태 생성
                                if !fix.replay_keys.is_empty() {
                                    // 엔진 리셋 (조합 상태 초기화)
                                    let current_category = engine.input_category();
                                    *engine = InputEngine::new(&config);
                                    engine.set_input_category(current_category);

                                    // replay 키를 엔진에 밀어넣기
                                    for (key, modifier) in &fix.replay_keys {
                                        engine.press_key(*key, *modifier, &config);
                                    }

                                    // replay 결과: preedit이 마지막 음절
                                    let replay_preedit = engine.preedit_str().to_string();
                                    // replay에서 발생한 commit은 무시 (시그널의 commit_text에 이미 포함)
                                    engine.clear_commit();

                                    unim_log!(
                                        "ENGINE_WORKER",
                                        "[Engine Worker] AutoTypeFix replay: preedit='{}', commit_text='{}'",
                                        replay_preedit,
                                        fix.commit_text
                                    );

                                    // preedit은 시그널 경유로 프론트엔드가 처리
                                    // EngineResponse.preedit은 비워둠 (타이밍 문제 방지)
                                    preedit = Some(String::new());

                                    Some((fix.delete_chars, fix.commit_text.clone(), replay_preedit))
                                } else {
                                    Some((fix.delete_chars, fix.commit_text.clone(), String::new()))
                                }
                            } else {
                                // 교정 안 됨 → 되돌리기 기록 삭제
                                last_autofix.remove(&context_id);
                                None
                            }
                        } else {
                            // 비알파벳 키 → 버퍼 초기화
                            buf.clear();
                            last_autofix.remove(&context_id);
                            None
                        }
                    } else {
                        // 모드 변경 또는 팝업 활성 → 버퍼 초기화
                        if mode_changed.is_some() || popup_action.is_some() {
                            keystroke_buffers.remove(&context_id);
                            last_autofix.remove(&context_id);
                        }
                        None
                    };

                    EngineResponse {
                        consumed: result.consumed,
                        preedit,
                        commit,
                        mode_changed,
                        popup_action,
                        auto_typefix: auto_typefix_result,
                    }
                } else {
                    EngineResponse {
                        consumed: false,
                        preedit: None,
                        commit: None,
                        mode_changed: None,
                        popup_action: None,
                        auto_typefix: None,
                    }
                };

                // AutoTypeFix 모드 전환 시 Global 동기화 (contexts borrow 해제 후)
                if resp.auto_typefix.is_some()
                    && config.engine.mode_sharing == unim::config::ModeSharingMode::Global
                {
                    let new_mode = config.engine.default_category;
                    for (&cid, eng) in contexts.iter_mut() {
                        if cid != context_id {
                            eng.set_input_category(new_mode);
                        }
                    }
                }

                let _ = response.send(resp);
            }

            EngineRequest::FocusIn {
                context_id,
                window_id,
                response,
            } => {
                // 마지막 포커스된 컨텍스트 추적 (글로벌 TypeFix용)
                last_focused_context_id = Some(context_id);

                // AutoTypeFix: 포커스 변경 시 word_buffer 초기화
                keystroke_buffers.remove(&context_id);
                last_autofix.remove(&context_id);

                // 컨텍스트-창 매핑은 항상 업데이트 (팝업 바이패스 등에서 필요)
                context_windows.insert(context_id, window_id.clone());

                // PerApp 모드에서는 앱별 저장된 모드를 적용
                let app_id = extract_app_id(&window_id);
                if config.engine.mode_sharing == unim::config::ModeSharingMode::PerApp {
                    if let Some(engine) = contexts.get_mut(&context_id) {
                        if let Some(&saved_mode) = app_modes.get(app_id) {
                            engine.set_input_category(saved_mode);
                        }
                    }
                }

                // 앱별 기본 모드 규칙 적용 (최초 포커스 시)
                if !config.engine.app_rules.is_empty() {
                    if let Some(engine) = contexts.get_mut(&context_id) {
                        // 아직 app_modes에 저장된 적 없으면 (최초 방문) 규칙 적용
                        if !app_modes.contains_key(app_id) {
                            for rule in &config.engine.app_rules {
                                if window_id.contains(&rule.app_pattern) {
                                    engine.set_input_category(rule.default_category);
                                    app_modes
                                        .insert(app_id.to_string(), rule.default_category);
                                    unim_log!(
                                        "ENGINE_WORKER",
                                        "[Engine Worker] 앱 규칙 적용: pattern='{}', window_id={}, mode={:?}",
                                        rule.app_pattern,
                                        window_id,
                                        rule.default_category
                                    );
                                    break;
                                }
                            }
                        }
                    }
                }

                // 현재 컨텍스트의 입력 모드 반환 (UI 동기화용)
                let is_korean = contexts
                    .get(&context_id)
                    .map(|e| e.input_category() == unim::config::InputCategory::Korean)
                    .unwrap_or(false);
                unim_log!(
                    "ENGINE_WORKER",
                    "[Engine Worker] FocusIn: context_id={}, window_id={}, is_korean={}",
                    context_id,
                    window_id,
                    is_korean
                );
                let _ = response.send(is_korean);
            }

            EngineRequest::FocusOut {
                context_id,
                response,
            } => {
                // AutoTypeFix: 포커스 아웃 시 word_buffer 초기화
                keystroke_buffers.remove(&context_id);
                last_autofix.remove(&context_id);

                let commit = if let Some(engine) = contexts.get_mut(&context_id) {
                    // 팝업 활성 상태이면 취소하고 트리거 문자를 커밋 텍스트에 포함
                    let mut commit_text = String::new();
                    if engine.is_hanja_mode() {
                        let t = engine.get_hanja_target().to_string();
                        engine.cancel_hanja();
                        if !t.is_empty() {
                            commit_text = t;
                        }
                        unim_log!(
                            "ENGINE_WORKER",
                            "[Engine Worker] FocusOut: 한자 팝업 취소, context_id={}",
                            context_id
                        );
                    } else if engine.is_special_char_mode() {
                        let t = engine.get_special_char_target().to_string();
                        engine.cancel_special_char();
                        if !t.is_empty() {
                            commit_text = t;
                        }
                        unim_log!(
                            "ENGINE_WORKER",
                            "[Engine Worker] FocusOut: 특수문자 팝업 취소, context_id={}",
                            context_id
                        );
                    } else {
                        let preedit = engine.preedit_str();
                        if !preedit.is_empty() {
                            commit_text = preedit.to_string();
                        }
                    }

                    // 엔진 초기화 (입력 모드 유지)
                    let current_mode = engine.input_category();
                    *engine = InputEngine::new(&config);
                    if config.engine.mode_sharing != unim::config::ModeSharingMode::Global {
                        engine.set_input_category(current_mode);
                    }

                    if commit_text.is_empty() {
                        None
                    } else {
                        Some(commit_text)
                    }
                } else {
                    None
                };
                let _ = response.send(commit);
            }

            EngineRequest::Reset {
                context_id,
                response,
            } => {
                let commit = if let Some(engine) = contexts.get_mut(&context_id) {
                    // 팝업 활성 상태이면 취소하고 트리거 문자 반환
                    let mut commit_text = String::new();
                    if engine.is_hanja_mode() {
                        let t = engine.get_hanja_target().to_string();
                        engine.cancel_hanja();
                        if !t.is_empty() {
                            commit_text = t;
                        }
                    } else if engine.is_special_char_mode() {
                        let t = engine.get_special_char_target().to_string();
                        engine.cancel_special_char();
                        if !t.is_empty() {
                            commit_text = t;
                        }
                    } else {
                        // 팝업 없으면 조합 중 preedit 커밋
                        let preedit = engine.preedit_str();
                        if !preedit.is_empty() {
                            commit_text = preedit.to_string();
                        }
                    }

                    // 엔진 초기화 (입력 모드 유지)
                    let current_mode = engine.input_category();
                    *engine = InputEngine::new(&config);
                    engine.set_input_category(current_mode);

                    if commit_text.is_empty() {
                        None
                    } else {
                        Some(commit_text)
                    }
                } else {
                    None
                };
                let _ = response.send(commit);
            }

            EngineRequest::SetGlobalMode { is_korean } => {
                let category = if is_korean {
                    unim::config::InputCategory::Korean
                } else {
                    unim::config::InputCategory::English
                };

                // config의 기본 카테고리는 항상 업데이트 (새 컨텍스트 생성/리셋 시 적용)
                config.engine.default_category = category;

                // Global 모드에서만 모든 컨텍스트의 입력 카테고리 변경
                if config.engine.mode_sharing == unim::config::ModeSharingMode::Global {
                    for engine in contexts.values_mut() {
                        engine.set_input_category(category);
                    }
                    unim_log!(
                        "ENGINE_WORKER",
                        "[Engine Worker] 전역 모드 변경: {:?}",
                        category
                    );
                } else {
                    unim_log!(
                        "ENGINE_WORKER",
                        "[Engine Worker] PerApp 모드 - 전역 동기화 생략"
                    );
                }
            }

            // =========================================
            // 한자 변환 요청 처리
            // =========================================
            EngineRequest::GetHanjaCandidates {
                context_id,
                response,
            } => {
                let resp = if let Some(engine) = contexts.get_mut(&context_id) {
                    // 먼저 한자 변환을 시작하여 후보를 생성
                    engine.start_hanja_conversion();
                    // Pull 방식 요청이므로 Push용 popup_pending_action 소비하여 제거
                    // (이후 ProcessKeyEvent에서 stale 시그널이 발행되지 않도록)
                    engine.take_popup_action();

                    crate::service::HanjaCandidateResponse {
                        target: engine.get_hanja_target().to_string(),
                        candidates: engine.get_hanja_candidates(),
                    }
                } else {
                    crate::service::HanjaCandidateResponse {
                        target: String::new(),
                        candidates: Vec::new(),
                    }
                };
                let _ = response.send(resp);
            }

            EngineRequest::SelectHanja {
                context_id,
                index,
                response,
            } => {
                let result = if let Some(engine) = contexts.get_mut(&context_id) {
                    engine.select_hanja(index)
                } else {
                    None
                };
                let _ = response.send(result);
            }

            EngineRequest::CancelHanja {
                context_id,
                response,
            } => {
                let target = if let Some(engine) = contexts.get_mut(&context_id) {
                    // cancel 전에 hanja_target(원래 한글)을 저장하여 즉시 커밋할 수 있도록 반환
                    let t = engine.get_hanja_target().to_string();
                    let result = if !t.is_empty() { Some(t) } else { None };
                    engine.cancel_hanja();
                    result
                } else {
                    None
                };
                let _ = response.send(target);
            }

            // =========================================
            // 특수문자 변환 요청 처리
            // =========================================
            EngineRequest::GetSpecialCharCandidates {
                context_id,
                response,
            } => {
                let top_row = config.engine.english.layout.top_row_labels().to_string();
                let resp = if let Some(engine) = contexts.get_mut(&context_id) {
                    // start_hanja_conversion이 이미 특수문자 fallback을 처리하므로
                    // 엔진의 특수문자 모드 상태를 확인
                    if engine.is_special_char_mode() {
                        crate::service::SpecialCharResponse {
                            target: engine.get_special_char_target().to_string(),
                            characters: engine
                                .get_special_char_candidates()
                                .iter()
                                .map(|c| c.to_string())
                                .collect(),
                            top_row,
                        }
                    } else {
                        crate::service::SpecialCharResponse {
                            target: String::new(),
                            characters: Vec::new(),
                            top_row,
                        }
                    }
                } else {
                    crate::service::SpecialCharResponse {
                        target: String::new(),
                        characters: Vec::new(),
                        top_row,
                    }
                };
                let _ = response.send(resp);
            }

            EngineRequest::SelectSpecialChar {
                context_id,
                index,
                response,
            } => {
                let result = if let Some(engine) = contexts.get_mut(&context_id) {
                    engine.select_special_char(index).map(|c| c.to_string())
                } else {
                    None
                };
                let _ = response.send(result);
            }

            EngineRequest::CancelSpecialChar {
                context_id,
                response,
            } => {
                let target = if let Some(engine) = contexts.get_mut(&context_id) {
                    // cancel 전에 special_char_target(원래 초성)을 저장하여 즉시 커밋할 수 있도록 반환
                    let t = engine.get_special_char_target().to_string();
                    let result = if !t.is_empty() { Some(t) } else { None };
                    engine.cancel_special_char();
                    result
                } else {
                    None
                };
                let _ = response.send(target);
            }

            EngineRequest::ReportCursorRect { .. } => {
                // 커서 위치는 service.rs의 InputContextHandler에서 직접 처리
                // 엔진 워커는 무시
            }

            EngineRequest::SetContentType {
                context_id,
                purpose,
            } => {
                if let Some(engine) = contexts.get_mut(&context_id) {
                    let content_purpose = unim::config::ContentPurpose::from_u32(purpose);
                    engine.set_content_purpose(content_purpose);
                    unim_log!(
                        "ENGINE_WORKER",
                        "[Engine Worker] SetContentType: context_id={}, purpose={:?}",
                        context_id,
                        content_purpose
                    );
                }
            }

            EngineRequest::SetSurroundingText {
                context_id,
                text,
                cursor_pos,
                anchor_pos,
            } => {
                if let Some(engine) = contexts.get_mut(&context_id) {
                    engine.set_surrounding_text(text, cursor_pos, anchor_pos);
                }
            }

            EngineRequest::GlobalTypeFix {
                direction,
                response,
            } => {
                let result = if let Some(ctx_id) = last_focused_context_id {
                    if let Some(engine) = contexts.get_mut(&ctx_id) {
                        unim_log!(
                            "ENGINE_WORKER",
                            "[Engine Worker] GlobalTypeFix: context_id={}, direction={}",
                            ctx_id,
                            direction
                        );
                        engine.typefix_convert(direction)
                    } else {
                        None
                    }
                } else {
                    unim_log!(
                        "ENGINE_WORKER",
                        "[Engine Worker] GlobalTypeFix: 포커스된 컨텍스트 없음"
                    );
                    None
                };
                let _ = response.send(result);
            }

            EngineRequest::SmartBackspace {
                context_id,
                response,
            } => {
                let result = if let Some(engine) = contexts.get_mut(&context_id) {
                    engine.smart_backspace()
                } else {
                    None
                };
                let _ = response.send(result);
            }

            EngineRequest::SearchEmoji { keyword, response } => {
                let results = unim::hangul::emoji::search_emoji(&keyword);
                let emoji_strings: Vec<String> = results.iter().map(|c| c.to_string()).collect();
                let _ = response.send(emoji_strings);
            }
        }
    }

    unim_log!("ENGINE_WORKER", "[Engine Worker] 종료됨");
}
