//! AutoTypeFix 오케스트레이션 (TSF 로컬 포팅)
//!
//! engine_worker.rs:679-900 의 per-context 오케스트레이션을 TSF 단일 인스턴스로 포팅.
//! Linux daemon 과 달리 컨텍스트가 하나이므로 HashMap 없이 단일 구조체로 관리.

use std::time::{Duration, Instant};

use unim::auto_typefix::{self, KeystrokeBuffer};
use unim::config::{Config, InputCategory};
use unim::input_engine::InputEngine;
use unim::keycode::{KeyCode, ModifierState};
use unim::typefix_blacklist::{Blacklist, Direction};
use unim::typefix_userdict::UserDictionary;
use unim::unim_log;

// ── 내부 상태 구조체 ──────────────────────────────────────────────────────────

/// Ctrl+Z 되돌리기 전용 상태.
/// AutoTypeFix 트리거 직후 생성, 10초 후 또는 다음 알파벳 입력 시 만료.
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

/// 최근 AutoTypeFix 교정 기록 (재트리거 blacklist 학습용).
/// engine_worker.rs 의 RecentCorrection 과 동일 구조.
struct RecentCorrection {
    ascii: String,
    direction: Direction,
    korean_layout: unim::config::KoreanLayout,
    english_layout: unim::config::EnglishLayout,
    corrected_at: Instant,
    erasure_observed: bool,
    mode_switch_observed: bool,
}

// ── AutoTypeFix 적용 결과 (호출자에게 반환) ────────────────────────────────────

/// handle_key_down 이 AutoTypeFix 트리거를 감지했을 때 반환하는 적용 명령.
/// composition.rs 의 replace_surrounding 에 전달된다.
#[derive(Debug)]
pub struct AutoFixApply {
    /// 커서 앞에서 삭제할 글자 수
    pub delete_chars: u32,
    /// 삭제 후 삽입할 텍스트
    pub commit_text: String,
    /// 순방향(영→한) replay preedit. 역방향이면 빈 문자열.
    pub replay_preedit: String,
}

// ── AutoTypeFixState 공개 구조체 ───────────────────────────────────────────────

/// TSF STA 스레드 전용 AutoTypeFix 상태.
/// UnimTextService 가 Mutex<AutoTypeFixState> 로 보유한다.
pub struct AutoTypeFixState {
    buf: KeystrokeBuffer,
    undo: Option<UndoState>,
    recent_corrections: Vec<RecentCorrection>,
    blacklist: Blacklist,
    user_dict: UserDictionary,
}

impl AutoTypeFixState {
    pub fn new() -> Self {
        Self {
            buf: KeystrokeBuffer::new(),
            undo: None,
            recent_corrections: Vec::new(),
            blacklist: Blacklist::load_from_default_path(),
            user_dict: UserDictionary::load_from_default_path(),
        }
    }

    /// config reload 시 blacklist/user_dict 재로드 (mtime 기반).
    pub fn reload_external_data(&mut self, atf_config: &unim::config::AutoTypeFixConfig) {
        self.blacklist.reload_if_changed();
        self.blacklist
            .expire_tentatives(atf_config.tentative_expiry_hours);
        self.user_dict.reload_if_changed();
    }

    /// 포커스 이동 시 상태 초기화 (engine_worker handle_focus_in 대응).
    pub fn reset_on_focus(&mut self) {
        self.buf.clear();
        self.undo = None;
        self.recent_corrections.clear();
    }
}

// ── rollback 관찰 헬퍼 ────────────────────────────────────────────────────────

fn rollback_threshold_met(r: &RecentCorrection) -> bool {
    match r.direction {
        Direction::Forward => r.erasure_observed && r.mode_switch_observed,
        Direction::Reverse => r.erasure_observed || r.mode_switch_observed,
    }
}

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

fn observe_rollback_event(recent: &mut Vec<RecentCorrection>, erasure: bool, switch: bool) {
    for r in recent.iter_mut() {
        if erasure {
            r.erasure_observed = true;
        }
        if switch {
            r.mode_switch_observed = true;
        }
    }
}

// ── 공개 함수 ─────────────────────────────────────────────────────────────────

/// Ctrl+Z AutoTypeFix 되돌리기 시도.
/// 활성 undo state 가 있으면 Some(apply) 반환 (호출자가 replace_surrounding 호출).
/// 그렇지 않으면 None (일반 Ctrl+Z 로 fallthrough).
pub fn try_undo(
    state: &mut AutoTypeFixState,
    keycode: KeyCode,
    modifier: ModifierState,
) -> Option<AutoFixApply> {
    if keycode != KeyCode::Z || !modifier.control || modifier.shift || modifier.alt {
        return None;
    }
    let obs = state.undo.take()?;
    if obs.is_expired() {
        return None;
    }
    let delete_chars = obs.corrected.chars().count() as u32;
    state.buf.clear();
    unim_log!(
        "TSF_ATF",
        "[TSF AutoTypeFix] Ctrl+Z 되돌리기: '{}' → '{}'",
        obs.corrected,
        obs.original
    );
    Some(AutoFixApply {
        delete_chars,
        commit_text: obs.original,
        replay_preedit: String::new(),
    })
}

/// Backspace 관찰: recent_corrections 에 erasure 플래그 세팅.
pub fn observe_backspace(state: &mut AutoTypeFixState, atf_config: &unim::config::AutoTypeFixConfig) {
    if atf_config.rollback_detection {
        observe_rollback_event(&mut state.recent_corrections, true, false);
    }
}

/// 모드 전환 관찰: recent_corrections 에 switch 플래그 세팅.
/// AutoTypeFix 자체 모드 전환에는 호출하지 말 것 (사용자 수동 전환만).
pub fn observe_mode_switch(
    state: &mut AutoTypeFixState,
    atf_config: &unim::config::AutoTypeFixConfig,
) {
    if atf_config.rollback_detection {
        observe_rollback_event(&mut state.recent_corrections, false, true);
    }
    // 모드 전환 시 키스트로크 버퍼 초기화 (engine_worker:613)
    state.buf.clear();
}

/// press_key 이후 AutoTypeFix 오케스트레이션 (engine_worker:668-940 포팅).
///
/// - `keycode`/`modifier`: 방금 누른 키
/// - `prev_mode`: press_key 호출 전 모드
/// - `was_composing`: press_key 호출 전 조합 상태 (조합 중 발동 금지 게이트)
/// - `commit_str`: press_key 결과의 commit (None = 변경 없음)
/// - `preedit_str`: press_key 결과의 preedit (None = 변경 없음)
///
/// Some(apply) 이면 호출자가 composition_mgr.replace_surrounding 을 호출해야 함.
pub fn process_after_key(
    state: &mut AutoTypeFixState,
    engine: &mut InputEngine,
    config: &Config,
    keycode: KeyCode,
    modifier: ModifierState,
    prev_mode: InputCategory,
    was_composing: bool,
    commit_str: Option<&str>,
    preedit_str: Option<&str>,
) -> Option<AutoFixApply> {
    let atf_config = &config.engine.auto_typefix;

    // 조합 중 발동 금지 (engine_worker 의 "popup_action.is_none() && mode_changed.is_none()" 게이트에 대응)
    if was_composing {
        state.buf.clear();
        return None;
    }

    // 만료된 undo 상태 정리
    if state.undo.as_ref().map(|u| u.is_expired()).unwrap_or(false) {
        state.undo = None;
    }

    // 만료된 recent_corrections 정리
    let obs_window = Duration::from_secs(atf_config.observation_timeout_secs as u64);
    state
        .recent_corrections
        .retain(|r| r.corrected_at.elapsed() <= obs_window);

    // atf_config.enabled 게이트
    if !atf_config.enabled {
        state.buf.clear();
        return None;
    }

    let current_mode = engine.input_category();
    // 모드가 이미 바뀌었으면(press_key 내부 전환) 버퍼 초기화 후 종료
    if prev_mode != current_mode {
        state.buf.clear();
        return None;
    }

    // buf.push: 비알파벳 키면 false
    if buf_push_key(&mut state.buf, keycode, modifier) {
        // 시간 윈도우 expire
        let window_ms = match current_mode {
            InputCategory::English => atf_config.forward_time_window_ms,
            InputCategory::Korean => atf_config.reverse_time_window_ms,
        };
        state.buf.expire(window_ms);

        // commit/preedit 추적 (역방향용)
        if let Some(c) = commit_str {
            state.buf.update_on_commit(c);
        }
        if let Some(p) = preedit_str {
            state.buf.update_on_preedit(p);
        }

        // 방향별 check
        let (fix_opt, direction) = match current_mode {
            InputCategory::English => (
                auto_typefix::check_forward(
                    &state.buf,
                    atf_config,
                    &config.engine.korean.layout,
                    &config.engine.english.layout,
                    &state.blacklist,
                ),
                Direction::Forward,
            ),
            InputCategory::Korean => (
                auto_typefix::check_reverse(
                    &state.buf,
                    atf_config,
                    &config.engine.korean.layout,
                    &config.engine.english.layout,
                    &state.blacklist,
                    &state.user_dict,
                ),
                Direction::Reverse,
            ),
        };

        // 재트리거 감지 + blacklist tentative 등록 (engine_worker:727-777)
        let fix_opt = if let Some(fix) = fix_opt {
            let suppression_key = match direction {
                Direction::Forward => fix.original.clone(),
                Direction::Reverse => fix.corrected.clone(),
            };
            let retrigger_layouts = if atf_config.rollback_detection {
                find_retrigger_layouts(&state.recent_corrections, &suppression_key, direction)
            } else {
                None
            };
            if let Some((kl, el)) = retrigger_layouts {
                state
                    .blacklist
                    .add_or_hit_tentative(&suppression_key, direction, &kl, &el);
                if let Err(e) = state.blacklist.save_to_default_path() {
                    unim_log!("TSF_ATF", "[TSF AutoTypeFix] blacklist 저장 실패: {}", e);
                }
                // 해당 항목 제거
                state.recent_corrections.retain(|r| {
                    !(r.direction == direction
                        && r.ascii == suppression_key
                        && rollback_threshold_met(r))
                });
                unim_log!(
                    "TSF_ATF",
                    "[TSF AutoTypeFix] 재트리거 감지({:?}) → 억제 + blacklist tentative: '{}'",
                    direction,
                    suppression_key
                );
                state.buf.clear();
                None
            } else {
                Some((fix, suppression_key))
            }
        } else {
            // 교정 안 됨 → undo 상태 폐기
            state.undo = None;
            None
        };

        if let Some((fix, suppression_key)) = fix_opt {
            let fix_has_replay = !fix.replay_keys.is_empty();

            // undo 상태 저장 (engine_worker:782-789)
            state.undo = Some(UndoState {
                corrected: fix.corrected.clone(),
                original: fix.original.clone(),
                observed_at: Instant::now(),
            });

            // recent_corrections 기록 (engine_worker:800-810)
            state.recent_corrections.push(RecentCorrection {
                ascii: suppression_key,
                direction,
                korean_layout: config.engine.korean.layout.clone(),
                english_layout: config.engine.english.layout.clone(),
                corrected_at: Instant::now(),
                erasure_observed: false,
                mode_switch_observed: false,
            });

            unim_log!(
                "TSF_ATF",
                "[TSF AutoTypeFix] 트리거: delete={}, corrected='{}', clear_preedit={}",
                fix.delete_chars,
                fix.corrected,
                fix.clear_preedit
            );

            // 교정 후 모드 전환 (engine_worker:820-833)
            let new_mode = match current_mode {
                InputCategory::English => InputCategory::Korean,
                InputCategory::Korean => InputCategory::English,
            };
            engine.set_input_category(new_mode);

            unim_log!(
                "TSF_ATF",
                "[TSF AutoTypeFix] 모드 전환: {:?} → {:?}",
                current_mode,
                new_mode
            );

            // buf_ascii 캡처 (역방향 delete_chars 계산용) — buf.clear() 전에
            let buf_ascii = state.buf.to_ascii_string(&config.engine.english.layout);

            // 버퍼 초기화
            state.buf.clear();

            if fix_has_replay {
                // 순방향 (영→한): 엔진 리셋 후 replay_keys 재생 (engine_worker:849-880)
                let current_cat = engine.input_category();
                *engine = InputEngine::new(config);
                engine.set_input_category(current_cat);

                for (k, m) in &fix.replay_keys {
                    engine.press_key(*k, *m, config);
                }

                let replay_preedit = engine.preedit_str().to_string();
                engine.clear_commit();

                unim_log!(
                    "TSF_ATF",
                    "[TSF AutoTypeFix] 순방향 replay: preedit='{}', commit='{}'",
                    replay_preedit,
                    fix.commit_text
                );

                // 순방향(영→한) delete_chars = 입력한 영문 전체 길이.
                //
                // 과거엔 engine_worker(Linux DBus)를 따라 -1 했으나, 그건 Linux 가 트리거
                // 키의 commit 을 억제해 앱에 N-1 자만 있기 때문이다. TSF 경로는 영문 모드에서
                // 각 키가 조합 없이 즉시 commit 되어 앱(메모장 등)에 N 자가 모두 있으므로,
                // -1 하면 커서 앞 N-1 자만 지워져 첫 글자가 잔류한다("ntkd"→"n서기" 버그).
                // (역방향(한→영)은 마지막 음절이 조합 중=미commit 이라 별도 -1 보정이 정당.)
                let delete_chars = fix.delete_chars;

                Some(AutoFixApply {
                    delete_chars,
                    commit_text: fix.commit_text,
                    replay_preedit,
                })
            } else {
                // 역방향 (한→영): delete_chars 재계산 (engine_worker:882-915)
                let current_cat = engine.input_category();
                *engine = InputEngine::new(config);
                engine.set_input_category(current_cat);

                // trigger 키 제외 시뮬: ascii_before_trigger 로 kor_sim 산출
                let ascii_before_trigger =
                    &buf_ascii[..buf_ascii.len().saturating_sub(1)];
                let kor_sim = unim::typefix::eng_to_kor(
                    ascii_before_trigger,
                    &config.engine.korean.layout,
                    &config.engine.english.layout,
                );
                // trigger 키의 ProcessKeyEvent 응답이 preedit clear → 화면에서 사라짐 → 1 차감
                let real_delete = kor_sim.chars().count().saturating_sub(1) as u32;

                unim_log!(
                    "TSF_ATF",
                    "[TSF AutoTypeFix] 역방향 real_delete: ascii='{}', kor_sim='{}', real_delete={}",
                    buf_ascii,
                    kor_sim,
                    real_delete
                );

                Some(AutoFixApply {
                    delete_chars: real_delete,
                    commit_text: fix.commit_text,
                    replay_preedit: String::new(),
                })
            }
        } else {
            None
        }
    } else {
        // 비알파벳 키 → 버퍼 초기화 (engine_worker:923-931)
        state.buf.clear();
        // Backspace 이외는 undo 맥락 종료
        if keycode != KeyCode::Backspace {
            state.undo = None;
        }
        None
    }
}

// ── 내부 헬퍼 ─────────────────────────────────────────────────────────────────

/// buf.push 래퍼: Space 는 KeystrokeBuffer 내부에서 이미 걸러지므로 그대로 위임.
fn buf_push_key(buf: &mut KeystrokeBuffer, keycode: KeyCode, modifier: ModifierState) -> bool {
    buf.push(keycode, modifier)
}
