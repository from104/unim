//! 엔진 워커 모듈
//!
//! `InputEngine`은 `Send + Sync`를 구현하지 않으므로,
//! 별도의 전용 스레드에서 실행되어야 합니다.
//! 이 모듈은 엔진 스레드 실행 및 요청 처리를 담당합니다.

use std::collections::HashMap;
use std::thread;
use std::time::{Duration, Instant};

use tokio::sync::mpsc;

use crate::service::{
    popup_render_flags, EmojiShowPayload, EngineRequest, EngineResponse, PopupRenderPayload,
};
use unim::auto_typefix::{self, KeystrokeBuffer};
use unim::config::{Config, EnglishLayout, KoreanLayout};
use unim::input_engine::{InputEngine, PageDirection};
use unim::keycode::{KeyCode, ModifierState};
use unim::popup::PopupKind;
use unim::typefix_blacklist::{Blacklist, Direction};
use unim::typefix_userdict::UserDictionary;
use unim::unim_log;

/// Ctrl+Z 되돌리기 전용 상태. AutoTypeFix 트리거 직후 생성되며,
/// 다음 이벤트 중 하나로 소멸된다:
/// - Ctrl+Z → 되돌리기 수행 후 제거
/// - 10초 타임아웃 / 포커스 변경 → 제거
struct UndoState {
    corrected: String,
    original: String,
    observed_at: Instant,
}

impl UndoState {
    fn is_expired(&self) -> bool {
        self.observed_at.elapsed().as_secs() > 10
    }
}

/// 최근 AutoTypeFix 교정 기록. 관찰 창 내에서 사용자가
/// **BS(preedit 삭제 포함) AND 모드 전환**을 모두 수행하면
/// 오탐으로 간주해 blacklist에 tentative로 등록한다.
///
/// 두 이벤트의 순서는 무관(BS→전환, 전환→BS, BS→전환→BS 모두 허용).
/// 각각 단독으로는 정당한 편집/전환일 수 있으므로 AND 게이트로 구분한다.
struct RecentCorrection {
    ascii: String,
    direction: Direction,
    korean_layout: KoreanLayout,
    english_layout: EnglishLayout,
    corrected_at: Instant,
    erasure_observed: bool,
    mode_switch_observed: bool,
}

/// window_id에서 앱 식별자를 추출합니다.
/// 형식: "app_name:window_specific_id" → "app_name"
/// ':' 가 없으면 window_id 전체를 app_id로 사용합니다.
fn extract_app_id(window_id: &str) -> &str {
    window_id.split(':').next().unwrap_or(window_id)
}

/// 활성 popup 의 view_model 을 DBus payload 로 변환. popup 비활성이면 None.
///
/// engine 의 `home_row_labels` 를 view_model 에 주입 (이모지 카테고리 단축키 표시용).
fn build_render_state(engine: &unim::input_engine::InputEngine) -> Option<PopupRenderPayload> {
    let state = engine.popup_state()?;
    let vm = state.view_model(engine.home_row_labels());

    let kind = match vm.kind {
        unim::popup::PopupKind::Hanja => 0u32,
        unim::popup::PopupKind::SpecialChar => 1u32,
        unim::popup::PopupKind::Emoji => 2u32,
    };

    let rows = vm.cells.len() as u32;
    let cols = vm.cells.first().map(|r| r.len()).unwrap_or(0) as u32;

    // column-major 직렬화: cells[col*rows + row] = cells[row][col]
    let mut cells_flat: Vec<(String, String, u32)> =
        Vec::with_capacity((rows * cols) as usize);
    for c in 0..cols as usize {
        for r in 0..rows as usize {
            let cell_opt = vm.cells.get(r).and_then(|row| row.get(c)).and_then(|x| x.as_ref());
            match cell_opt {
                Some(cd) => {
                    let mut flags = popup_render_flags::HAS_DATA;
                    if cd.is_selected {
                        flags |= popup_render_flags::SELECTED;
                    }
                    if cd.is_col_highlight {
                        flags |= popup_render_flags::COL_HIGHLIGHT;
                    }
                    if cd.is_row_highlight {
                        flags |= popup_render_flags::ROW_HIGHLIGHT;
                    }
                    if cd.is_bookmarked {
                        flags |= popup_render_flags::BOOKMARKED;
                    }
                    cells_flat.push((
                        cd.text.clone(),
                        cd.meaning.clone().unwrap_or_default(),
                        flags,
                    ));
                }
                None => {
                    cells_flat.push((String::new(), String::new(), 0));
                }
            }
        }
    }

    let col_headers: Vec<(String, bool)> = vm
        .col_headers
        .iter()
        .zip(vm.col_header_active.iter())
        .map(|(t, a)| (t.clone(), *a))
        .collect();
    let row_headers: Vec<(String, bool)> = vm
        .row_headers
        .iter()
        .zip(vm.row_header_active.iter())
        .map(|(t, a)| (t.clone(), *a))
        .collect();

    Some(PopupRenderPayload {
        kind,
        target: vm.target,
        header_text: vm.header_text,
        footer_text: vm.footer_text,
        show_footer: vm.show_footer,
        rows,
        cols,
        sel_row: vm.sel_row as u32,
        sel_col: vm.sel_col as u32,
        current_page: vm.current_page as u32,
        total_pages: vm.total_pages as u32,
        cells: cells_flat,
        col_headers,
        row_headers,
        expand_visible: vm.expand_visible,
        expand_text: vm.expand_text,
        tab_labels: vm.tab_labels,
        active_tab_index: vm.active_tab_index as u32,
    })
}

/// Popup RPC (PopupChangePage / ToggleHanjaBookmark) 라우팅 보정.
///
/// 호출 context (`caller`) 에 popup 이 활성이면 그대로 반환. 비활성이면
/// `popup_state` 가 `Some` 인 다른 context 를 찾아 그 ID 를 반환한다 — GNOME
/// extension 은 자체 InputContext 를 두지만, 실제 popup 은 다른 frontend
/// (GTK4_IM, Qt, XIM 등) 의 context 에 살 수 있다. 시그널은 글로벌 구독으로
/// 모두 받지만 RPC 만 호출 context 로 가는 비대칭을 보정한다.
///
/// 활성 popup 을 가진 context 가 없으면 `caller` 를 그대로 반환 — 호출자가
/// "no popup" no-op 분기로 빠지게 한다.
fn resolve_popup_owner(contexts: &HashMap<u32, InputEngine>, caller: u32) -> u32 {
    if contexts
        .get(&caller)
        .and_then(|e| e.popup_state())
        .is_some()
    {
        return caller;
    }
    contexts
        .iter()
        .find(|(_, e)| e.popup_state().is_some())
        .map(|(id, _)| *id)
        .unwrap_or(caller)
}

/// Ctrl+Z AutoTypeFix 되돌리기 시도.
/// 해당 컨텍스트에 활성 UndoState가 있고 키가 Ctrl+Z이면 되돌리기 응답을 반환한다.
/// 그렇지 않으면 None (일반 키 처리 계속).
fn try_autotypefix_undo(
    undo_states: &mut HashMap<u32, UndoState>,
    keystroke_buffers: &mut HashMap<u32, KeystrokeBuffer>,
    context_id: u32,
    key: KeyCode,
    modifier: ModifierState,
) -> Option<EngineResponse> {
    if !(key == KeyCode::Z && modifier.control && !modifier.shift && !modifier.alt) {
        return None;
    }
    let obs = undo_states.remove(&context_id)?;
    let delete_chars = obs.corrected.chars().count() as u32;
    keystroke_buffers.remove(&context_id);
    unim_log!(
        "ENGINE_WORKER",
        "[Engine Worker] AutoTypeFix 되돌리기: '{}' → '{}'",
        obs.corrected,
        obs.original
    );
    Some(EngineResponse {
        consumed: true,
        preedit: None,
        commit: None,
        mode_changed: None,
        popup_action: None,
        auto_typefix: Some((delete_chars, obs.original, String::new())),
        render_state: None,
    })
}

/// FocusIn 처리: AutoTypeFix 상태 초기화 + window_id 매핑 + PerApp/app_rules 기반 모드 적용.
/// 반환값: 해당 컨텍스트의 현재 입력 모드가 한국어인지 여부(UI 동기화용).
#[allow(clippy::too_many_arguments)]
fn handle_focus_in(
    contexts: &mut HashMap<u32, InputEngine>,
    app_modes: &mut HashMap<String, unim::config::InputCategory>,
    context_windows: &mut HashMap<u32, String>,
    keystroke_buffers: &mut HashMap<u32, KeystrokeBuffer>,
    undo_states: &mut HashMap<u32, UndoState>,
    recent_corrections: &mut HashMap<u32, Vec<RecentCorrection>>,
    config: &Config,
    context_id: u32,
    window_id: &str,
) -> bool {
    // AutoTypeFix 상태 초기화 — 다른 창으로 포커스가 이동했으므로 버퍼 보존 의미 없음.
    keystroke_buffers.remove(&context_id);
    undo_states.remove(&context_id);
    recent_corrections.remove(&context_id);

    // 컨텍스트-창 매핑은 항상 업데이트 (팝업 바이패스 등에서 사용)
    context_windows.insert(context_id, window_id.to_string());

    let app_id = extract_app_id(window_id);

    // PerApp 모드: 앱별 저장된 입력 카테고리 복원
    if config.engine.mode_sharing == unim::config::ModeSharingMode::PerApp {
        if let Some(engine) = contexts.get_mut(&context_id) {
            if let Some(&saved_mode) = app_modes.get(app_id) {
                engine.set_input_category(saved_mode);
            }
        }
    }

    // 앱별 기본 모드 규칙 적용 (해당 앱을 처음 본 경우에만)
    if !config.engine.app_rules.is_empty() {
        if let Some(engine) = contexts.get_mut(&context_id) {
            if !app_modes.contains_key(app_id) {
                for rule in &config.engine.app_rules {
                    if window_id.contains(&rule.app_pattern) {
                        engine.set_input_category(rule.default_category);
                        app_modes.insert(app_id.to_string(), rule.default_category);
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

    contexts
        .get(&context_id)
        .map(|e| e.input_category() == unim::config::InputCategory::Korean)
        .unwrap_or(false)
}

/// 포커스 아웃 / Reset 공통 처리: 팝업을 커밋 텍스트로 변환하고 엔진을 초기화한다.
///
/// `preserve_mode=true` (FocusOut / Reset의 PerApp·Context-local 모드 유지) 시
/// 초기화 후에도 이전 입력 카테고리를 복원한다.
/// 반환값: 커밋할 텍스트(preedit/팝업 타겟) 또는 None.
fn reset_engine_and_capture_commit(
    engine: &mut InputEngine,
    config: &Config,
    preserve_mode: bool,
) -> Option<String> {
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
        let preedit = engine.preedit_str();
        if !preedit.is_empty() {
            commit_text = preedit.to_string();
        }
    }

    let current_mode = engine.input_category();
    *engine = InputEngine::new(config);
    if preserve_mode {
        engine.set_input_category(current_mode);
    }

    if commit_text.is_empty() {
        None
    } else {
        Some(commit_text)
    }
}

/// 방향별 rollback 관찰 게이트.
///
/// 롤백이 "관찰 완료"로 간주되기 위한 조건. 실제 blacklist 등록은 이 함수 결과만으로
/// 일어나지 않고, 동일 ASCII의 AutoTypeFix **재트리거** 시점에 일어난다 ([`is_retrigger`]).
///
/// - **Forward**: BS **AND** 모드 전환을 모두 관찰.
///   (순방향은 replay_keys로 preedit이 살아있어 BS가 IM → engine까지 forward됨.)
/// - **Reverse**: BS **OR** 모드 전환 중 하나만 관찰되어도 충분.
///   역방향은 `clear_preedit=true` 이후 GTK/Qt IM module이 BS를 consume하지 않아
///   engine_worker까지 도달하지 않는 구조적 제약이 있으므로, 관찰창 내의 수동
///   모드 전환만으로도 롤백 시도로 간주한다.
fn rollback_threshold_met(r: &RecentCorrection) -> bool {
    match r.direction {
        Direction::Forward => r.erasure_observed && r.mode_switch_observed,
        Direction::Reverse => r.erasure_observed || r.mode_switch_observed,
    }
}

/// 현재 시도가 "재트리거" 인지 판정하며, 매칭 항목의 레이아웃을 반환한다.
///
/// 관찰창 내 RecentCorrection 중 **동일 ASCII + 동일 방향**으로
/// [`rollback_threshold_met`] 이 `true` 인 항목이 있으면 `Some((kl, el))`.
/// 반환된 레이아웃은 사용자가 실제로 타이핑한 당시의 레이아웃이며, blacklist
/// 등록에 그대로 사용된다.
///
/// 재트리거 판정이 나면 2차 AutoTypeFix를 억제(None)하고 해당 ASCII를
/// blacklist tentative로 등록한다 — 즉 "롤백 관찰 + 같은 단어 재입력"만이
/// 등록 조건. 롤백 관찰 자체는 플래그로만 남아 있어, 재입력이 없으면
/// observation_timeout 이후 자연 만료되어 오탐이 남지 않는다.
fn find_retrigger_layouts(
    recent: &[RecentCorrection],
    ascii: &str,
    direction: Direction,
) -> Option<(unim::config::KoreanLayout, unim::config::EnglishLayout)> {
    recent
        .iter()
        .find(|r| r.direction == direction && r.ascii == ascii && rollback_threshold_met(r))
        .map(|r| (r.korean_layout.clone(), r.english_layout.clone()))
}

/// BS / 모드 전환 관찰을 RecentCorrection 플래그에 반영한다. 순서 무관.
///
/// 등록은 여기서 하지 않는다 — `find_retrigger_layouts` 기반 재트리거 감지 시점까지 연기된다.
fn observe_rollback_event(
    recent_corrections: &mut HashMap<u32, Vec<RecentCorrection>>,
    context_id: u32,
    erasure: bool,
    switch: bool,
) {
    let Some(list) = recent_corrections.get_mut(&context_id) else {
        return;
    };
    for r in list.iter_mut() {
        if erasure {
            r.erasure_observed = true;
        }
        if switch {
            r.mode_switch_observed = true;
        }
    }
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
    // AutoTypeFix: Ctrl+Z 되돌리기 전용 상태
    let mut undo_states: HashMap<u32, UndoState> = HashMap::new();
    // AutoTypeFix: 재트리거 감지용 최근 교정 기록 (컨텍스트별)
    let mut recent_corrections: HashMap<u32, Vec<RecentCorrection>> = HashMap::new();
    // AutoTypeFix: 학습형 억제 단어 목록 (파일 기반, mtime 감지 reload)
    let mut blacklist = Blacklist::load_from_default_path();
    // AutoTypeFix: 역방향 사용자 사전 (파일 기반, mtime 감지 reload)
    let mut user_dict = UserDictionary::load_from_default_path();

    unim_log!("ENGINE_WORKER", "[Engine Worker] 시작됨");

    // 블로킹으로 요청 수신 (tokio 런타임 밖에서 실행)
    while let Some(request) = rx.blocking_recv() {
        // 설정 파일 변경 여부 확인 및 리로드 (Throttling 적용됨)
        if config.reload_if_changed() {
            unim_log!(
                "ENGINE_WORKER",
                "[Engine Worker] 설정 파일 변경 감지 - 리로드 완료"
            );
            // 기존 엔진들의 레이아웃·v1 프로필을 갱신.
            //
            // `rebuild_korean_context`는 layout 동일·active_rule_sets만 변경된 경우에도
            // v1 builder 경로로 한국어 컨텍스트를 다시 만든다. 이전 코드는
            // `set_korean_layout`만 호출해 layout 동일이면 no-op이라 GUI/CLI에서
            // active_rule_sets 토글이 즉시 반영되지 않는 회귀가 있었음.
            for engine in contexts.values_mut() {
                engine.rebuild_korean_context(&config);
                engine.set_english_layout(config.engine.english.layout.clone());
            }
        }

        // 학습형 억제 단어 목록: GUI가 파일을 직접 수정할 수 있으므로
        // 매 요청마다 mtime 기반 reload(2초 throttling) + 만료 tentative 전환.
        if blacklist.reload_if_changed() {
            unim_log!(
                "ENGINE_WORKER",
                "[Engine Worker] typefix-blacklist 파일 변경 감지 - 리로드 완료"
            );
        }
        blacklist.expire_tentatives(config.engine.auto_typefix.tentative_expiry_hours);

        // 역방향 사용자 사전: CLI/GUI가 파일 수정 시 반영.
        if user_dict.reload_if_changed() {
            unim_log!(
                "ENGINE_WORKER",
                "[Engine Worker] typefix-userdict 파일 변경 감지 - 리로드 완료"
            );
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
                // 컨텍스트 ID에 묶인 모든 per-context 상태를 함께 정리한다.
                // (이관 누락 시 context_id가 단조 증가하는 IBus Portal 경로에서 HashMap이 무제한 누적됨)
                contexts.remove(&id);
                context_windows.remove(&id);
                keystroke_buffers.remove(&id);
                undo_states.remove(&id);
                recent_corrections.remove(&id);
                if last_focused_context_id == Some(id) {
                    last_focused_context_id = None;
                }
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
                        render_state: None,
                    });
                    continue;
                }

                let resp = if let Some(engine) = contexts.get_mut(&context_id) {
                    // keycode를 KeyCode로 변환
                    let key = KeyCode::from_evdev_keycode(keycode as u16);
                    let modifier = ModifierState::from_x11_mask(state);
                    let atf_config = &config.engine.auto_typefix;

                    // 만료된 undo 상태 정리 (10초 타임아웃)
                    if let Some(obs) = undo_states.get(&context_id) {
                        if obs.is_expired() {
                            undo_states.remove(&context_id);
                        }
                    }

                    // 만료된 recent_corrections 정리 (관찰 창)
                    let observation_window =
                        Duration::from_secs(atf_config.observation_timeout_secs as u64);
                    if let Some(list) = recent_corrections.get_mut(&context_id) {
                        list.retain(|r| r.corrected_at.elapsed() <= observation_window);
                        if list.is_empty() {
                            recent_corrections.remove(&context_id);
                        }
                    }

                    // Backspace: 지우는 행위 감지 (preedit BS도 키 레벨에서는 동일).
                    // 여기선 플래그만 세팅. 실제 blacklist 등록은 동일 ASCII 재트리거 시.
                    if key == KeyCode::Backspace && atf_config.rollback_detection {
                        observe_rollback_event(&mut recent_corrections, context_id, true, false);
                    }

                    // Ctrl+Z: AutoTypeFix 되돌리기 (헬퍼에 위임)
                    if let Some(undo_resp) = try_autotypefix_undo(
                        &mut undo_states,
                        &mut keystroke_buffers,
                        context_id,
                        key,
                        modifier,
                    ) {
                        let _ = response.send(undo_resp);
                        continue;
                    }

                    // 처리 전 상태 저장
                    let prev_mode = engine.input_category();

                    // 키 처리
                    let result = engine.press_key(key, modifier, &config);

                    // 모드 변경 감지
                    let current_mode = engine.input_category();
                    let mut mode_changed = if prev_mode != current_mode {
                        // 모드 전환 관찰 → 플래그만 세팅. 실제 blacklist 등록은 동일 ASCII
                        // 재트리거 시점(AutoTypeFix 결과 발생 직전)에서 수행.
                        // AutoTypeFix에 의한 자동 전환(아래 블록)은 별도 경로이므로 기록하지 않는다.
                        if atf_config.rollback_detection {
                            observe_rollback_event(
                                &mut recent_corrections,
                                context_id,
                                false,
                                true,
                            );
                        }
                        // 모드 변경 시 키스트로크 버퍼 초기화
                        keystroke_buffers.remove(&context_id);

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
                    let mut fix_has_replay = false;
                    let mut auto_typefix_result = if atf_config.enabled
                        && mode_changed.is_none()
                        && popup_action.is_none()
                    {
                        let buf = keystroke_buffers
                            .entry(context_id)
                            .or_insert_with(KeystrokeBuffer::new);

                        // 알파벳 키면 버퍼에 추가
                        if buf.push(key, modifier) {
                            // 시간 윈도우 밖 엔트리 제거 (방향별 독립 윈도우)
                            let window_ms = match current_mode {
                                unim::config::InputCategory::English => {
                                    atf_config.forward_time_window_ms
                                }
                                unim::config::InputCategory::Korean => {
                                    atf_config.reverse_time_window_ms
                                }
                            };
                            buf.expire(window_ms);

                            // commit/preedit 추적 (역방향용)
                            if let Some(ref c) = commit {
                                buf.update_on_commit(c);
                            }
                            if let Some(ref p) = preedit {
                                buf.update_on_preedit(p);
                            } else if commit.is_some() {
                                // commit은 있지만 preedit 변경 없음
                                // (preedit이 이전 값 유지 — 변경 없으므로 건드리지 않음)
                            }

                            // 방향에 따라 감지 (blacklist 억제 게이트 포함)
                            let (fix, direction) = match current_mode {
                                unim::config::InputCategory::English => (
                                    auto_typefix::check_forward(
                                        buf,
                                        atf_config,
                                        &config.engine.korean.layout,
                                        &config.engine.english.layout,
                                        &blacklist,
                                    ),
                                    Direction::Forward,
                                ),
                                unim::config::InputCategory::Korean => (
                                    auto_typefix::check_reverse(
                                        buf,
                                        atf_config,
                                        &config.engine.korean.layout,
                                        &config.engine.english.layout,
                                        &blacklist,
                                        &user_dict,
                                    ),
                                    Direction::Reverse,
                                ),
                            };

                            // 재트리거 감지: 관찰창 내에서 동일 ASCII의 AutoTypeFix가
                            // 이미 rollback 관찰(BS/모드전환) 된 적 있으면, 이번 트리거를
                            // 억제하고 blacklist에 tentative로 등록한다.
                            let fix = if let Some(fix) = fix {
                                let key = match direction {
                                    Direction::Forward => fix.original.clone(),
                                    Direction::Reverse => fix.corrected.clone(),
                                };
                                // 매칭되는 RecentCorrection 항목의 레이아웃을 캡처
                                // (등록 시 사용 — 사용자가 실제로 타이핑한 당시 레이아웃을 반영).
                                let retrigger_layouts = if atf_config.rollback_detection {
                                    recent_corrections.get(&context_id).and_then(|list| {
                                        find_retrigger_layouts(list, &key, direction)
                                    })
                                } else {
                                    None
                                };
                                if let Some((kl, el)) = retrigger_layouts {
                                    blacklist.add_or_hit_tentative(&key, direction, &kl, &el);
                                    if let Err(e) = blacklist.save_to_default_path() {
                                        unim_log!(
                                            "ENGINE_WORKER",
                                            "[Engine Worker] blacklist 저장 실패: {}",
                                            e
                                        );
                                    }
                                    if let Some(list) = recent_corrections.get_mut(&context_id) {
                                        list.retain(|r| {
                                            !(r.direction == direction
                                                && r.ascii == key
                                                && rollback_threshold_met(r))
                                        });
                                        if list.is_empty() {
                                            recent_corrections.remove(&context_id);
                                        }
                                    }
                                    unim_log!(
                                        "ENGINE_WORKER",
                                        "[Engine Worker] 재트리거 감지({:?}) → 억제 + blacklist tentative 추가: '{}'",
                                        direction,
                                        key
                                    );
                                    // 버퍼는 초기화하여 같은 키스트로크가 또 감지되지 않도록 한다.
                                    buf.clear();
                                    None
                                } else {
                                    Some(fix)
                                }
                            } else {
                                None
                            };

                            if let Some(ref fix) = fix {
                                fix_has_replay = !fix.replay_keys.is_empty();
                                // Ctrl+Z 되돌리기용 상태 저장
                                undo_states.insert(
                                    context_id,
                                    UndoState {
                                        corrected: fix.corrected.clone(),
                                        original: fix.original.clone(),
                                        observed_at: Instant::now(),
                                    },
                                );
                                // 재트리거 감지용 최근 교정 기록.
                                // blacklist 매칭 키는 `check_forward` / `check_reverse`가
                                // `is_suppressed(&ascii, ...)`에 넘기는 `buffer.to_ascii_string()`과
                                // 일치해야 한다. 순방향은 `fix.original`이 그 ASCII지만,
                                // 역방향은 `original`이 Ctrl+Z undo 용도로 비어 있으므로
                                // `corrected`(eng)을 사용한다.
                                let suppression_key = match direction {
                                    Direction::Forward => fix.original.clone(),
                                    Direction::Reverse => fix.corrected.clone(),
                                };
                                recent_corrections.entry(context_id).or_default().push(
                                    RecentCorrection {
                                        ascii: suppression_key,
                                        direction,
                                        korean_layout: config.engine.korean.layout.clone(),
                                        english_layout: config.engine.english.layout.clone(),
                                        corrected_at: Instant::now(),
                                        erasure_observed: false,
                                        mode_switch_observed: false,
                                    },
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
                                    unim::config::InputCategory::English => {
                                        unim::config::InputCategory::Korean
                                    }
                                    unim::config::InputCategory::Korean => {
                                        unim::config::InputCategory::English
                                    }
                                };
                                engine.set_input_category(new_mode);
                                config.engine.default_category = new_mode;
                                // Global 동기화는 contexts borrow 해제 후 별도 처리
                                mode_changed =
                                    Some(new_mode == unim::config::InputCategory::Korean);

                                unim_log!(
                                    "ENGINE_WORKER",
                                    "[Engine Worker] AutoTypeFix 모드 전환: {:?}",
                                    new_mode
                                );

                                // 버퍼 초기화 전에 ascii 미리 캡처
                                // (역방향의 delete_chars 계산에 사용)
                                let buf_ascii = buf.to_ascii_string(&config.engine.english.layout);

                                // 교정 후 버퍼 초기화
                                buf.clear();

                                // 순방향: 마지막 음절을 엔진에 replay하여 preedit 상태 생성
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

                                    Some((
                                        fix.delete_chars,
                                        fix.commit_text.clone(),
                                        replay_preedit,
                                    ))
                                } else {
                                    // 역방향: 엔진 리셋 (한글 조합 상태 제거)
                                    let current_category = engine.input_category();
                                    *engine = InputEngine::new(&config);
                                    engine.set_input_category(current_category);

                                    // delete_chars = trigger 키 제외한 키스트로크의 eng_to_kor 글자 수.
                                    // trigger 키의 commit은 억제(final_commit=None)되고
                                    // preedit은 cleared(final_preedit=Some(""))되므로,
                                    // trigger 키가 만든 글자는 화면에 없다.
                                    // 따라서 trigger 전까지의 키스트로크만 시뮬하여 글자 수 산출.
                                    let ascii_before_trigger =
                                        &buf_ascii[..buf_ascii.len().saturating_sub(1)];
                                    let kor_sim = unim::typefix::eng_to_kor(
                                        ascii_before_trigger,
                                        &config.engine.korean.layout,
                                        &config.engine.english.layout,
                                    );
                                    // trigger 키 제외 시뮬 결과에서 마지막 글자는 preedit(조합 중).
                                    // trigger 키의 ProcessKeyEvent 응답이 preedit을 clear하므로
                                    // 그 글자는 화면에서 사라짐 → 추가로 1개 차감.
                                    let real_delete =
                                        kor_sim.chars().count().saturating_sub(1) as u32;

                                    unim_log!(
                                        "ENGINE_WORKER",
                                        "[Engine Worker] 역방향 real_delete: ascii='{}', trigger='{}', kor_sim='{}', real_delete={}, layout={:?}",
                                        buf_ascii,
                                        buf_ascii.chars().last().unwrap_or(' '),
                                        kor_sim,
                                        real_delete,
                                        config.engine.korean.layout
                                    );

                                    Some((real_delete, fix.commit_text.clone(), String::new()))
                                }
                            } else {
                                // 교정 안 됨 → 사용자가 다른 알파벳 키를 계속 입력한 상황.
                                // 직전 교정에 대한 Ctrl+Z 되돌리기 여지는 사라졌으므로 undo 상태 폐기.
                                undo_states.remove(&context_id);
                                None
                            }
                        } else {
                            // 비알파벳 키 → 버퍼 초기화.
                            // Backspace는 undo 상태를 남겨두고(추가 삭제 중일 수 있음),
                            // 그 외(Enter/Space/Tab 등)는 "되돌리기 맥락 종료"로 간주하여 폐기.
                            buf.clear();
                            if key != KeyCode::Backspace {
                                undo_states.remove(&context_id);
                            }
                            None
                        }
                    } else {
                        // 모드 변경 또는 팝업 활성 → 버퍼 초기화.
                        // 모드 변경 옵저버 처리는 이미 mode_changed 감지 블록에서 완료됨.
                        if mode_changed.is_some() || popup_action.is_some() {
                            keystroke_buffers.remove(&context_id);
                        }
                        None
                    };

                    // AutoTypeFix 트리거 시:
                    // 순방향 (영→한, replay_keys 있음): commit 억제 + delete_chars-1
                    //   영어 모드에서 현재 키 commit을 막아야 시그널이 깔끔하게 교체
                    // 역방향 (한→영, replay_keys 없음): commit/preedit 그대로 유지
                    //   한글 조합은 이미 화면에 있고, 시그널이 교체 처리
                    let (final_preedit, final_commit) =
                        if let Some(ref mut atf) = auto_typefix_result {
                            if !fix_has_replay {
                                // 역방향: commit/preedit 억제, delete_chars 유지
                                // 한글은 키:글자 비율이 1:1이 아니므로
                                // committed_chars + has_preedit이 정확한 화면 글자 수
                                (Some(String::new()), None)
                            } else {
                                // 순방향: commit 억제, delete_chars 1 감소
                                if atf.0 > 0 {
                                    atf.0 -= 1;
                                }
                                (Some(String::new()), None)
                            }
                        } else {
                            (preedit, commit)
                        };

                    let render_state = build_render_state(engine);
                    EngineResponse {
                        consumed: result.consumed,
                        preedit: final_preedit,
                        commit: final_commit,
                        mode_changed,
                        popup_action,
                        auto_typefix: auto_typefix_result,
                        render_state,
                    }
                } else {
                    EngineResponse {
                        consumed: false,
                        preedit: None,
                        commit: None,
                        mode_changed: None,
                        popup_action: None,
                        auto_typefix: None,
                        render_state: None,
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

                let is_korean = handle_focus_in(
                    &mut contexts,
                    &mut app_modes,
                    &mut context_windows,
                    &mut keystroke_buffers,
                    &mut undo_states,
                    &mut recent_corrections,
                    &config,
                    context_id,
                    &window_id,
                );
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
                // AutoTypeFix: 포커스 아웃 시 word_buffer, undo 상태, 재트리거 기록 초기화
                keystroke_buffers.remove(&context_id);
                undo_states.remove(&context_id);
                recent_corrections.remove(&context_id);

                // PerApp / Context-local 모드이면 엔진 초기화 후에도 이전 모드를 유지.
                // Global 모드에서는 전역 default_category가 다시 적용되도록 preserve=false.
                let preserve_mode =
                    config.engine.mode_sharing != unim::config::ModeSharingMode::Global;
                let commit = contexts.get_mut(&context_id).and_then(|engine| {
                    reset_engine_and_capture_commit(engine, &config, preserve_mode)
                });
                unim_log!(
                    "ENGINE_WORKER",
                    "[Engine Worker] FocusOut: context_id={}, commit={:?}",
                    context_id,
                    commit
                );
                let _ = response.send(commit);
            }

            EngineRequest::Reset {
                context_id,
                response,
            } => {
                // Reset은 사용자가 명시적으로 요청한 리셋 — 현재 입력 모드는 항상 유지.
                let commit = contexts
                    .get_mut(&context_id)
                    .and_then(|engine| reset_engine_and_capture_commit(engine, &config, true));
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
                // 활성 영문 키맵의 top_row를 응답에 포함시켜 frontend의 9x9 컬럼 헤더가
                // 키맵 변경에 동기화되도록 한다 (특수문자 응답과 동일 source).
                let top_row =
                    unim::config::english_layout_top_row_labels(&config.engine.english.layout)
                        .to_string();
                let resp = if let Some(engine) = contexts.get_mut(&context_id) {
                    // 먼저 한자 변환을 시작하여 후보를 생성
                    engine.start_hanja_conversion();
                    // Pull 방식 요청이므로 Push용 popup_pending_action 소비하여 제거
                    // (이후 ProcessKeyEvent에서 stale 시그널이 발행되지 않도록)
                    engine.take_popup_action();

                    let render_state = build_render_state(engine);
                    crate::service::HanjaCandidateResponse {
                        target: engine.get_hanja_target().to_string(),
                        candidates: engine.get_hanja_candidates(),
                        top_row,
                        render_state,
                    }
                } else {
                    crate::service::HanjaCandidateResponse {
                        target: String::new(),
                        candidates: Vec::new(),
                        top_row,
                        render_state: None,
                    }
                };
                let _ = response.send(resp);
            }

            EngineRequest::SelectHanja {
                context_id,
                index,
                response,
            } => {
                // popup-owner 라우팅 (ToggleHanjaBookmark 와 동일 사유 — 마우스 클릭이
                // GNOME extension 자체 context 로 들어와도 popup 은 GTK4_IM 등 다른
                // context 에 살아있을 수 있다).
                let target_id = resolve_popup_owner(&contexts, context_id);
                let result = if let Some(engine) = contexts.get_mut(&target_id) {
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
                let target_id = resolve_popup_owner(&contexts, context_id);
                let target = if let Some(engine) = contexts.get_mut(&target_id) {
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

            EngineRequest::GetHanjaBookmarkStates {
                context_id,
                response,
            } => {
                let target_id = resolve_popup_owner(&contexts, context_id);
                let states = contexts
                    .get(&target_id)
                    .map(|engine| engine.hanja_bookmark_states())
                    .unwrap_or_default();
                let _ = response.send(states);
            }

            EngineRequest::ToggleHanjaBookmark {
                context_id,
                index,
                response,
            } => {
                // popup-owner 로 라우팅: 호출 context (예: GNOME extension 자체 context)
                // 에 popup 이 없을 수 있다. 그 경우 popup_state 가 활성인 다른 context 로
                // fallback 한다. 구독은 모든 context의 시그널을 글로벌로 받는데,
                // RPC 만 호출 context 로 가는 비대칭을 보정.
                let target_id = resolve_popup_owner(&contexts, context_id);
                let result = if let Some(engine) = contexts.get_mut(&target_id) {
                    let toggled = engine.toggle_hanja_bookmark(index);
                    // 토글이 자체 emit한 PopupAction(HanjaCandidatesReordered)을
                    // 함께 빼내 RPC 호출자가 시그널로 발행하도록 한다.
                    let action = engine.take_popup_action();
                    let render = build_render_state(engine);
                    toggled.map(|(idx, b, _was)| (idx, b, action, render))
                } else {
                    None
                };
                let _ = response.send(result);
            }

            EngineRequest::PopupChangePage {
                context_id,
                direction,
                response,
            } => {
                // popup-owner 로 라우팅 (ToggleHanjaBookmark 와 동일 사유).
                let target_id = resolve_popup_owner(&contexts, context_id);
                let payload: Option<PopupRenderPayload> =
                    if let Some(engine) = contexts.get_mut(&target_id) {
                        // direction <= 0: Prev, > 0: Next.
                        let dir = if direction <= 0 {
                            PageDirection::Prev
                        } else {
                            PageDirection::Next
                        };
                        let changed = engine.popup_change_page(dir).is_some();
                        // popup_change_page 가 PageJump 액션을 pending 에 넣어둠 — 여기서 비워준다
                        // (시그널로는 PopupNavigate 만 발행하므로 PageJump 는 폐기).
                        let _ = engine.take_popup_action();
                        if changed {
                            build_render_state(engine)
                        } else {
                            None
                        }
                    } else {
                        None
                    };
                let _ = response.send(payload);
            }

            EngineRequest::TogglePopupExpand {
                context_id,
                response,
            } => {
                // popup-owner 로 라우팅 (PopupChangePage 와 동일 사유).
                // GNOME extension·gui-gtk 에서 마우스로 ⊞/⊟ 아이콘 클릭 시 호출 —
                // popup_state 가 Hanja 면 toggle_hanja_expanded() 적용 후 갱신된
                // view_model 을 반환.
                let target_id = resolve_popup_owner(&contexts, context_id);
                let payload: Option<PopupRenderPayload> =
                    if let Some(engine) = contexts.get_mut(&target_id) {
                        let toggled = if let Some(state) = engine.popup_state_mut() {
                            if state.kind() == unim::popup::PopupKind::Hanja {
                                state.toggle_hanja_expanded();
                                true
                            } else {
                                false
                            }
                        } else {
                            false
                        };
                        if toggled {
                            build_render_state(engine)
                        } else {
                            None
                        }
                    } else {
                        None
                    };
                let _ = response.send(payload);
            }

            // =========================================
            // 특수문자 변환 요청 처리
            // =========================================
            EngineRequest::GetSpecialCharCandidates {
                context_id,
                response,
            } => {
                let top_row =
                    unim::config::english_layout_top_row_labels(&config.engine.english.layout)
                        .to_string();
                let resp = if let Some(engine) = contexts.get_mut(&context_id) {
                    // start_hanja_conversion이 이미 특수문자 fallback을 처리하므로
                    // 엔진의 특수문자 모드 상태를 확인
                    if engine.is_special_char_mode() {
                        let render_state = build_render_state(engine);
                        crate::service::SpecialCharResponse {
                            target: engine.get_special_char_target().to_string(),
                            characters: engine
                                .get_special_char_candidates()
                                .iter()
                                .map(|c| c.to_string())
                                .collect(),
                            top_row,
                            render_state,
                        }
                    } else {
                        crate::service::SpecialCharResponse {
                            target: String::new(),
                            characters: Vec::new(),
                            top_row,
                            render_state: None,
                        }
                    }
                } else {
                    crate::service::SpecialCharResponse {
                        target: String::new(),
                        characters: Vec::new(),
                        top_row,
                        render_state: None,
                    }
                };
                let _ = response.send(resp);
            }

            EngineRequest::SelectSpecialChar {
                context_id,
                index,
                response,
            } => {
                let target_id = resolve_popup_owner(&contexts, context_id);
                let result = if let Some(engine) = contexts.get_mut(&target_id) {
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
                let target_id = resolve_popup_owner(&contexts, context_id);
                let target = if let Some(engine) = contexts.get_mut(&target_id) {
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

            EngineRequest::UserDictAdd {
                word,
                note,
                response,
            } => {
                let note_opt = if note.trim().is_empty() {
                    None
                } else {
                    Some(note)
                };
                let ok = user_dict.add(&word, note_opt);
                if ok {
                    if let Err(e) = user_dict.save_to_default_path() {
                        unim_log!(
                            "ENGINE_WORKER",
                            "[Engine Worker] user-dict 저장 실패: {}",
                            e
                        );
                    } else {
                        unim_log!(
                            "ENGINE_WORKER",
                            "[Engine Worker] user-dict 추가: '{}' (총 {}개)",
                            word,
                            user_dict.len()
                        );
                    }
                }
                let _ = response.send(ok);
            }

            EngineRequest::UserDictRemove { word, response } => {
                let ok = user_dict.remove_by_word(&word);
                if ok {
                    if let Err(e) = user_dict.save_to_default_path() {
                        unim_log!(
                            "ENGINE_WORKER",
                            "[Engine Worker] user-dict 저장 실패: {}",
                            e
                        );
                    } else {
                        unim_log!(
                            "ENGINE_WORKER",
                            "[Engine Worker] user-dict 제거: '{}' (총 {}개)",
                            word,
                            user_dict.len()
                        );
                    }
                }
                let _ = response.send(ok);
            }

            EngineRequest::UserDictList { response } => {
                let list: Vec<(String, String, u64)> = user_dict
                    .reverse_words
                    .iter()
                    .map(|e| {
                        (
                            e.word.clone(),
                            e.note.clone().unwrap_or_default(),
                            e.added_at,
                        )
                    })
                    .collect();
                let _ = response.send(list);
            }

            EngineRequest::UserDictUpdate {
                index,
                word,
                note,
                response,
            } => {
                let note_opt = if note.trim().is_empty() {
                    None
                } else {
                    Some(note)
                };
                let ok = user_dict.update_at(index as usize, &word, note_opt);
                if ok {
                    if let Err(e) = user_dict.save_to_default_path() {
                        unim_log!(
                            "ENGINE_WORKER",
                            "[Engine Worker] user-dict 저장 실패: {}",
                            e
                        );
                    }
                }
                let _ = response.send(ok);
            }

            EngineRequest::RegisterUserDictFromSelection { response } => {
                // 마지막 포커스된 컨텍스트의 surrounding text에서 선택 영역을 추출하여
                // 사용자 사전에 등록한다. TypeFix와 동일한 패턴.
                let word = if let Some(ctx_id) = last_focused_context_id {
                    if let Some(engine) = contexts.get(&ctx_id) {
                        let (text, cursor, anchor) = engine.surrounding_text();
                        let (text_owned, cursor, anchor) =
                            (text.to_string(), cursor as usize, anchor as usize);
                        if !text_owned.is_empty() && cursor != anchor {
                            let chars: Vec<char> = text_owned.chars().collect();
                            let start = cursor.min(anchor);
                            let end = cursor.max(anchor).min(chars.len());
                            let selection: String = chars[start..end].iter().collect();
                            // 한글이면 영문으로 변환, 이미 알파벳이면 그대로
                            let candidate = if selection.chars().all(|c| c.is_ascii_alphabetic()) {
                                selection.clone()
                            } else {
                                unim::typefix::kor_to_eng(
                                    &selection,
                                    &config.engine.korean.layout,
                                    &config.engine.english.layout,
                                )
                            };
                            // 최종 영문 알파벳만인지 확인
                            let trimmed = candidate.trim();
                            if !trimmed.is_empty()
                                && trimmed.chars().all(|c| c.is_ascii_alphabetic())
                            {
                                if user_dict.add(trimmed, None) {
                                    if let Err(e) = user_dict.save_to_default_path() {
                                        unim_log!(
                                            "ENGINE_WORKER",
                                            "[Engine Worker] user-dict 저장 실패: {}",
                                            e
                                        );
                                        String::new()
                                    } else {
                                        unim_log!(
                                            "ENGINE_WORKER",
                                            "[Engine Worker] user-dict 단축키 등록: '{}' (원본: '{}')",
                                            trimmed,
                                            selection
                                        );
                                        trimmed.to_string()
                                    }
                                } else {
                                    unim_log!(
                                        "ENGINE_WORKER",
                                        "[Engine Worker] user-dict 단축키 등록 실패(중복/비유효): '{}'",
                                        trimmed
                                    );
                                    String::new()
                                }
                            } else {
                                unim_log!(
                                    "ENGINE_WORKER",
                                    "[Engine Worker] user-dict 단축키: 변환 결과 비유효 '{}'",
                                    candidate
                                );
                                String::new()
                            }
                        } else {
                            unim_log!(
                                "ENGINE_WORKER",
                                "[Engine Worker] user-dict 단축키: 선택 영역 없음"
                            );
                            String::new()
                        }
                    } else {
                        String::new()
                    }
                } else {
                    String::new()
                };
                let _ = response.send(word);
            }

            EngineRequest::SetEmojiCategory { idx, response } => {
                // GNOME extension 등 GUI 의 마우스 클릭 탭 전환을 위해 마지막 포커스
                // 컨텍스트의 popup_state(PopupKind::Emoji) 의 cat_index/items 를 갱신하고
                // payload 를 응답한다.
                let payload: Option<EmojiShowPayload> = (|| {
                    let ctx_id = last_focused_context_id?;
                    let engine = contexts.get_mut(&ctx_id)?;
                    // popup_state 가 emoji 인지 확인
                    if !engine.is_emoji_popup_active() {
                        unim_log!(
                            "ENGINE_WORKER",
                            "[Engine Worker] SetEmojiCategory(idx={}): emoji popup 비활성 — skip",
                            idx
                        );
                        return None;
                    }
                    let idx_usize = idx as usize;
                    // 카테고리 인덱스 범위 검증
                    let cat_count = engine
                        .popup_state()
                        .map(|s| s.emoji_categories().len())
                        .unwrap_or(0);
                    if idx_usize >= cat_count {
                        unim_log!(
                            "ENGINE_WORKER",
                            "[Engine Worker] SetEmojiCategory(idx={}): 범위 밖 (cats={}) — skip",
                            idx,
                            cat_count
                        );
                        return None;
                    }
                    // popup_state 갱신 (cat_index/items/페이지 리셋)
                    engine.refresh_emoji_category_items(idx_usize);

                    // 갱신된 popup_state 에서 ShowEmojiPopupV2 payload 추출
                    let state = engine.popup_state()?;
                    if state.kind() != PopupKind::Emoji {
                        return None;
                    }
                    let cats: Vec<(String, String, String, u32)> = state
                        .emoji_categories()
                        .iter()
                        .map(|c| {
                            (
                                c.id.clone(),
                                c.label_ko.clone(),
                                c.label_en.clone(),
                                c.total as u32,
                            )
                        })
                        .collect();
                    let target_cat_id = cats
                        .get(idx_usize)
                        .map(|c| c.0.clone())
                        .unwrap_or_default();
                    let items: Vec<String> = state.emoji_items().iter().cloned().collect();
                    let recent: Vec<String> = state.emoji_recent().iter().cloned().collect();
                    let top_row = unim::config::english_layout_top_row_labels(
                        &config.engine.english.layout,
                    )
                    .to_string();
                    let home_row = unim::config::english_layout_home_row_labels(
                        &config.engine.english.layout,
                    )
                    .to_string();
                    unim_log!(
                        "ENGINE_WORKER",
                        "[Engine Worker] SetEmojiCategory(idx={}): cat='{}', items={}, cats={}, home='{}'",
                        idx,
                        target_cat_id,
                        items.len(),
                        cats.len(),
                        home_row
                    );
                    Some(EmojiShowPayload {
                        target_cat_id,
                        items,
                        top_row,
                        recent,
                        categories: cats,
                        home_row,
                    })
                })();
                let _ = response.send(payload);
            }
        }
    }

    unim_log!("ENGINE_WORKER", "[Engine Worker] 종료됨");
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mk_rc(direction: Direction, erasure: bool, switch: bool) -> RecentCorrection {
        RecentCorrection {
            ascii: "speed".to_string(),
            direction,
            korean_layout: "ko_3bul390".to_string(),
            english_layout: "qwerty".to_string(),
            corrected_at: Instant::now(),
            erasure_observed: erasure,
            mode_switch_observed: switch,
        }
    }

    #[test]
    fn forward_requires_both_flags() {
        assert!(!rollback_threshold_met(&mk_rc(
            Direction::Forward,
            false,
            false
        )));
        assert!(!rollback_threshold_met(&mk_rc(
            Direction::Forward,
            true,
            false
        )));
        assert!(!rollback_threshold_met(&mk_rc(
            Direction::Forward,
            false,
            true
        )));
        assert!(rollback_threshold_met(&mk_rc(
            Direction::Forward,
            true,
            true
        )));
    }

    #[test]
    fn reverse_allows_either_flag() {
        assert!(!rollback_threshold_met(&mk_rc(
            Direction::Reverse,
            false,
            false
        )));
        // BS는 역방향에서 실제로는 도달하지 못하지만, 논리적으로는 OR의 한 쪽을 구성.
        assert!(rollback_threshold_met(&mk_rc(
            Direction::Reverse,
            true,
            false
        )));
        // 핵심 케이스: 관찰창 내의 수동 모드 전환만으로도 역방향은 관찰 완료.
        assert!(rollback_threshold_met(&mk_rc(
            Direction::Reverse,
            false,
            true
        )));
        assert!(rollback_threshold_met(&mk_rc(
            Direction::Reverse,
            true,
            true
        )));
    }

    #[test]
    fn find_retrigger_layouts_requires_threshold_met() {
        // 플래그 없으면 매칭 안 됨
        let list = vec![mk_rc(Direction::Reverse, false, false)];
        assert_eq!(
            find_retrigger_layouts(&list, "speed", Direction::Reverse),
            None
        );

        // 역방향: switch만 세팅되어도 매칭 → 레이아웃 반환
        let list = vec![mk_rc(Direction::Reverse, false, true)];
        assert_eq!(
            find_retrigger_layouts(&list, "speed", Direction::Reverse),
            Some(("ko_3bul390".to_string(), "qwerty".to_string()))
        );

        // 순방향: switch만 세팅 → AND 미달 → 매칭 안 됨
        let list = vec![mk_rc(Direction::Forward, false, true)];
        assert_eq!(
            find_retrigger_layouts(&list, "speed", Direction::Forward),
            None
        );

        // 순방향: BS + switch 모두 → 매칭
        let list = vec![mk_rc(Direction::Forward, true, true)];
        assert!(find_retrigger_layouts(&list, "speed", Direction::Forward).is_some());
    }

    #[test]
    fn find_retrigger_layouts_direction_and_ascii_specific() {
        // 역방향 "speed" 엔트리, 역방향 "other"로 질의 → 매칭 안 됨
        let list = vec![mk_rc(Direction::Reverse, false, true)]; // ascii="speed"
        assert_eq!(
            find_retrigger_layouts(&list, "other", Direction::Reverse),
            None
        );
        // 같은 ASCII지만 방향이 다르면 매칭 안 됨
        assert_eq!(
            find_retrigger_layouts(&list, "speed", Direction::Forward),
            None
        );
    }

    #[test]
    fn observe_rollback_event_sets_flags_only() {
        // 회귀 테스트: observation은 플래그만 세팅, blacklist에는 영향 없음.
        let mut recent: HashMap<u32, Vec<RecentCorrection>> = HashMap::new();
        recent.insert(9, vec![mk_rc(Direction::Reverse, false, false)]);

        observe_rollback_event(&mut recent, 9, false, true);
        let list = recent.get(&9).unwrap();
        assert_eq!(list.len(), 1, "관찰은 엔트리를 제거하지 않는다");
        assert!(!list[0].erasure_observed);
        assert!(list[0].mode_switch_observed);

        observe_rollback_event(&mut recent, 9, true, false);
        let list = recent.get(&9).unwrap();
        assert!(list[0].erasure_observed);
        assert!(list[0].mode_switch_observed);
    }

    // 주의: 재트리거 등록 경로의 end-to-end 테스트는 작성하지 않는다.
    // 성공 시 `Blacklist::save_to_default_path()`가 실제 사용자
    // `~/.config/unim/typefix-blacklist.yaml`을 덮어써 기존 엔트리를 파괴하기 때문이다.
    // 로직은 `rollback_threshold_met` + `find_retrigger_layouts` + `observe_rollback_event`의
    // 순수 함수 검증으로 충분히 커버된다.
}
