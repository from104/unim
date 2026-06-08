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
    /// 역방향(한→영): replace_surrounding 전에 활성 composition 을 먼저 종료해야 함.
    /// 역방향은 항상 조합 중 발동하므로, 조합 음절을 end_composition 으로 제거한 뒤
    /// 확정된 committed 글자만 delete_chars 로 지운다. (순방향/undo 는 false.)
    pub end_composition: bool,
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
        end_composition: false,
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
    _was_composing: bool,
    commit_str: Option<&str>,
    preedit_str: Option<&str>,
) -> Option<AutoFixApply> {
    let atf_config = &config.engine.auto_typefix;

    // [주의] 과거엔 여기서 `if was_composing { buf.clear(); return None; }` 로 조합 중
    // 발동을 막았으나, 이는 역방향(한→영)을 원천 봉쇄하는 버그였다: 한글은 둘째 키부터
    // 항상 조합 중(was_composing=true)이라 버퍼가 1글자도 못 쌓여 check_reverse 에 도달
    // 못 했다. 실제로 차단해야 하는 것은 popup 활성(key_handler 에서 !popup_active 로 가드)과
    // 모드 변경(아래 prev_mode != current_mode)이며, Linux engine_worker 에도 was_composing
    // 게이트는 없다. 순방향(영문 모드=비조합)은 이 게이트와 무관했다.

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

        // [진단] 역방향 체크 상태 — 항상 켜진 dbg_log 로 출력(unim_log 는 UNIM_DEVELOP 전용이라
        // 배포 DLL 에서 NO-OP). 역방향 미발동/조건 탈락 원인을 실측하기 위함.
        if matches!(direction, Direction::Reverse) {
            crate::register::dbg_log(&format!(
                "ATF rev-check: buf='{}' has_preedit={} committed={} fix={}",
                state.buf.to_ascii_string(&config.engine.english.layout),
                state.buf.has_preedit,
                state.buf.committed_chars,
                fix_opt.is_some()
            ));
        }

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
                    end_composition: false,
                })
            } else {
                // 역방향 (한→영): 활성 composition(조합 중 음절)을 먼저 종료해 제거하고,
                // 확정된(committed) 한글만 delete 후 영어를 삽입한다.
                //
                // 과거엔 kor_sim.chars().count()-1 로 삭제 수를 재계산했는데, 이 -1 은
                // 옛 composition 모델("트리거 키 응답이 마지막 음절 preedit 를 자동 제거")
                // 가정이었다. 새 모델(commit_and_restart)에선 그 가정이 깨져 확정 한글이
                // 잔류했다("hello"→"녀o"). 코어 check_reverse 가 계산한 fix.delete_chars
                // (= committed + 조합중 1)에서 조합중 1(clear_preedit)을 빼면 committed 수다.
                // 조합 음절은 end_composition(end_composition=true)으로 제거하므로 여기선
                // committed 만 delete 한다.
                let current_cat = engine.input_category();
                *engine = InputEngine::new(config);
                engine.set_input_category(current_cat);

                let committed = fix
                    .delete_chars
                    .saturating_sub(if fix.clear_preedit { 1 } else { 0 });

                crate::register::dbg_log(&format!(
                    "ATF rev-apply: committed={} end_comp={} insert='{}'",
                    committed, fix.clear_preedit, fix.commit_text
                ));

                Some(AutoFixApply {
                    delete_chars: committed,
                    commit_text: fix.commit_text,
                    replay_preedit: String::new(),
                    end_composition: fix.clear_preedit,
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
