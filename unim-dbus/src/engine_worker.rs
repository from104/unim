//! 엔진 워커 모듈
//!
//! `InputEngine`은 `Send + Sync`를 구현하지 않으므로,
//! 별도의 전용 스레드에서 실행되어야 합니다.
//! 이 모듈은 엔진 스레드 실행 및 요청 처리를 담당합니다.

use std::collections::HashMap;
use std::thread;

use tokio::sync::mpsc;

use crate::service::{EngineRequest, EngineResponse, PopupAction};
use unim::config::Config;
use unim::input_engine::InputEngine;
use unim::keycode::{KeyCode, ModifierState};
use unim::unim_log;

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
    // 창별 모드 저장: window_id -> InputCategory
    let mut window_modes: HashMap<String, unim::config::InputCategory> = HashMap::new();
    // 컨텍스트 -> 창 ID 매핑
    let mut context_windows: HashMap<u32, String> = HashMap::new();

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

                // PerWindow 모드에서는 창별 저장된 모드 적용
                if config.engine.mode_sharing == unim::config::ModeSharingMode::PerWindow {
                    if let Some(&saved_mode) = window_modes.get(&window_id) {
                        engine.set_input_category(saved_mode);
                        unim_log!(
                            "ENGINE_WORKER",
                            "[Engine Worker] 창별 모드 복원: window_id={}, mode={:?}",
                            window_id,
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
                let resp = if let Some(engine) = contexts.get_mut(&context_id) {
                    // keycode를 KeyCode로 변환
                    let key = KeyCode::from_evdev_keycode(keycode as u16);
                    let modifier = ModifierState::from_x11_mask(state);

                    // 처리 전 상태 저장
                    let prev_mode = engine.input_category();

                    // 키 처리
                    let result = engine.press_key(key, modifier, &config);

                    // 모드 변경 감지
                    let current_mode = engine.input_category();
                    let mode_changed = if prev_mode != current_mode {
                        // PerWindow 모드에서는 창별 모드 저장
                        if config.engine.mode_sharing == unim::config::ModeSharingMode::PerWindow {
                            if let Some(window_id) = context_windows.get(&context_id) {
                                window_modes.insert(window_id.clone(), current_mode);
                                unim_log!(
                                    "ENGINE_WORKER",
                                    "[Engine Worker] 창별 모드 저장: window_id={}, mode={:?}",
                                    window_id,
                                    current_mode
                                );
                            }
                        }
                        Some(current_mode == unim::config::InputCategory::Korean)
                    } else {
                        None
                    };

                    // 응답 생성
                    let preedit = if result.preedit_changed {
                        Some(engine.preedit_str().to_string())
                    } else {
                        None
                    };

                    let commit = if result.commit_changed {
                        let s = engine.commit_str().to_string();
                        // 커밋 버퍼를 읽은 후 반드시 비워야 함 (누적 방지)
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
                    let popup_action = if engine.is_hanja_mode() {
                        // 한자 모드 진입
                        Some(PopupAction::ShowHanja {
                            target: engine.get_hanja_target().to_string(),
                            candidates: engine.get_hanja_candidates(),
                        })
                    } else if engine.is_special_char_mode() {
                        // 특수문자 모드 진입
                        let top_row = config.engine.english.layout.top_row_labels().to_string();
                        Some(PopupAction::ShowSpecial {
                            target: engine.get_special_char_target().to_string(),
                            characters: engine
                                .get_special_char_candidates()
                                .iter()
                                .map(|c| c.to_string())
                                .collect(),
                            top_row,
                        })
                    } else {
                        None
                    };

                    EngineResponse {
                        consumed: result.consumed,
                        preedit,
                        commit,
                        mode_changed,
                        popup_action,
                    }
                } else {
                    EngineResponse {
                        consumed: false,
                        preedit: None,
                        commit: None,
                        mode_changed: None,
                        popup_action: None,
                    }
                };

                let _ = response.send(resp);
            }

            EngineRequest::FocusIn {
                context_id,
                window_id,
                response,
            } => {
                // PerWindow 모드에서는 창별 모드를 적용
                if config.engine.mode_sharing == unim::config::ModeSharingMode::PerWindow {
                    if let Some(engine) = contexts.get_mut(&context_id) {
                        if let Some(&saved_mode) = window_modes.get(&window_id) {
                            engine.set_input_category(saved_mode);
                        }
                    }
                    // 컨텍스트-창 매핑 업데이트
                    context_windows.insert(context_id, window_id.clone());
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
                let commit = if let Some(engine) = contexts.get_mut(&context_id) {
                    let preedit = engine.preedit_str();
                    if !preedit.is_empty() {
                        // 현재 모드 저장 (Global 모드가 아닌 경우)
                        let current_mode = engine.input_category();
                        // 조합 중인 텍스트를 커밋으로 변환하기 위해 엔진 리셋
                        // (flush_preedit이 private이므로 대안)
                        let commit_text = preedit.to_string();
                        *engine = InputEngine::new(&config);
                        // Global 모드가 아닌 경우 저장된 모드 복원 (PerApp, PerWindow)
                        if config.engine.mode_sharing != unim::config::ModeSharingMode::Global {
                            engine.set_input_category(current_mode);
                        }
                        Some(commit_text)
                    } else {
                        None
                    }
                } else {
                    None
                };
                let _ = response.send(commit);
            }

            EngineRequest::Reset { context_id } => {
                if let Some(engine) = contexts.get_mut(&context_id) {
                    // Reset은 조합 상태만 초기화 - 입력 모드(한/영)는 항상 유지
                    let current_mode = engine.input_category();
                    *engine = InputEngine::new(&config);
                    engine.set_input_category(current_mode);
                }
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

            EngineRequest::CancelHanja { context_id } => {
                if let Some(engine) = contexts.get_mut(&context_id) {
                    engine.cancel_hanja();
                }
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
                            top_row: top_row,
                        }
                    } else {
                        crate::service::SpecialCharResponse {
                            target: String::new(),
                            characters: Vec::new(),
                            top_row: top_row,
                        }
                    }
                } else {
                    crate::service::SpecialCharResponse {
                        target: String::new(),
                        characters: Vec::new(),
                        top_row: top_row,
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

            EngineRequest::CancelSpecialChar { context_id } => {
                if let Some(engine) = contexts.get_mut(&context_id) {
                    engine.cancel_special_char();
                }
            }

            EngineRequest::ReportCursorRect { .. } => {
                // 커서 위치는 service.rs의 InputContextHandler에서 직접 처리
                // 엔진 워커는 무시
            }
        }
    }

    unim_log!("ENGINE_WORKER", "[Engine Worker] 종료됨");
}
