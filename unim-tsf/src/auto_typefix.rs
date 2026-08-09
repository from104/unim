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
    /// 코어 `AutoTypeFixResult::replace_composition` 전파 — `true` 면 라이브 조합(word
    /// 모드)을 조합 SetText 로 치환하는 경로(key_handler)를 택한다. `false` 면 기존
    /// replace_surrounding 삭제 경로(음절/비협조앱) — 바이트 동일 무회귀 경로.
    pub replace_composition: bool,
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

    /// 비밀번호/PIN 필드 진입 시 민감 잔류 제거.
    ///
    /// 키스트로크 버퍼·undo 원문·최근 교정 이력을 모두 비운다(비번 직전 타이핑까지
    /// 제거). `reset_on_focus` 와 동일한 3필드 클리어지만, 호출 의도(비번 게이트)를
    /// 명시하는 별도 진입점이다 — OnSetFocus 의 스퓨리어스-포커스 보존 최적화
    /// (`atf_reset_should_skip`)를 우회해 **무조건** 클리어할 때 쓴다(fail-closed).
    /// blacklist/user_dict(디스크 학습본)는 건드리지 않는다 — ATF 관찰 게이트가 비번
    /// 중 학습 자체를 차단하므로 잔류 원천이 없다.
    pub fn clear_sensitive(&mut self) {
        self.buf.clear();
        self.undo = None;
        self.recent_corrections.clear();
    }

    /// Excel 셀 첫타 ATF 유실 게이트용 — 현재 키스트로크 버퍼 길이.
    ///
    /// 스퓨리어스 OnSetFocus 가 ATF 버퍼를 비우면 forward.rs 의 `len() < 2` 게이트가
    /// 미충족돼 첫 타 순방향 교정이 유실된다(가설). OnSetFocus 의 3중 게이트가 "보존할
    /// 버퍼가 있는지(`buf_len() > 0`)"를 판정하는 데 쓴다. 읽기 전용(부작용 없음).
    pub fn buf_len(&self) -> usize {
        self.buf.len()
    }

    /// Phase 4(S1): 보유 영문 Backspace 시 키스트로크 버퍼를 1개 동기 축소한다.
    ///
    /// TSF 보유 영문(english_hold)에서 Backspace 가 들어오면 보유 문자열은 1자 줄지만
    /// ATF 버퍼는 그대로라, 이후 순방향 교정의 delete_chars(버퍼 길이 기반)가 보유 영문
    /// 길이와 어긋나 mismatch degrade 로 떨어진다. 버퍼도 함께 -1 해(둘 다 키당 1자)
    /// `english_hold.len() == buf.len()` 불변식을 유지해 정상 교정 매칭을 지킨다.
    pub fn pop_keystroke(&mut self) -> bool {
        self.buf.pop_last()
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
        // undo 는 라이브 조합 치환이 아니라 확정 텍스트 되돌리기(surrounding) 경로.
        replace_composition: false,
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

        // 라이브 조합(word 모드) 반영 — 코어 check_forward/check_reverse 가 이 값으로
        // replace_composition(조합 SetText 치환) 여부를 판정한다. 여기(check 직전)는 아직
        // 아래 교정 후 engine.reset()/set_word_mode 이전이라 현재 포커스의 word 모드가 그대로다.
        state.buf.word_mode = engine.is_word_mode();

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
            crate::register::dbg_log_ev!(
                &format!(
                    "ATF rev-check: buf_len={} has_preedit={} committed={} fix={}",
                    state.buf.committed_chars,
                    state.buf.has_preedit,
                    state.buf.committed_chars,
                    fix_opt.is_some()
                ),
                "ATF rev-check: buf='{}' has_preedit={} committed={} fix={}",
                state.buf.to_ascii_string(&config.engine.english.layout),
                state.buf.has_preedit,
                state.buf.committed_chars,
                fix_opt.is_some()
            );
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

            // Phase 4(순방향 word 모드): 전체 단어를 word 라이브 조합으로 재구성하려면 전체
            // 키스트로크가 필요하다(fix.replay_keys 는 마지막 음절만 담음). 아래 buf.clear()
            // 전에 전체 시퀀스를 포착한다. 비-word/역방향에서는 사용하지 않으므로 무해.
            let all_keys: Vec<(KeyCode, ModifierState)> = state
                .buf
                .entries_vec()
                .iter()
                .map(|e| (e.keycode, e.modifier))
                .collect();

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
                // 순방향 (영→한): 엔진 리셋 후 replay 재생 (engine_worker:849-880).
                //
                // Phase 4(word 모드 분기): 코어 word 모드(winword 게이트)였으면, 리셋한 엔진을
                // 다시 word 모드로 두고 **전체 키스트로크**(all_keys)를 재생한다. 그러면
                // preedit_str()(=word_buffer+현재 음절)이 전체 한글 단어가 되어, 호출부가 보유
                // 영문 라이브 조합을 그 단어로 SetText 치환하고 committed=0 word 라이브 조합으로
                // 계속 보유할 수 있다(엔진 word_buffer 와 정합, 이어치기). commit_text 는 "" 로
                // 둔다(전체가 단일 라이브 조합). 비-word 는 종전대로 마지막 음절만 replay.
                let was_word = engine.is_word_mode();
                let current_cat = engine.input_category();
                // I3(perf): new(config) 재생성 대신 reset() — 6.76MB 한자사전 재파싱 +
                // 북마크 디스크IO 를 매 교정마다 반복하지 않도록. reset() 이 조합·팝업·
                // 버퍼 상태를 모두 비우고 config 파생 캐시(키맵/사전/레이아웃)는 동일
                // config 이므로 그대로 유효하다.
                engine.reset();
                engine.set_input_category(current_cat);
                // reset() 은 accumulate_word 플래그를 보존하므로, new(config) 와 동일하게
                // commit_unit 기반으로 재동기화한다(비-word/Smart 분기에서 이전 word 상태가
                // 잔류하지 않도록). 아래 was_word 분기가 필요 시 다시 true 로 덮는다.
                engine.set_word_mode(config.engine.korean.commit_unit == unim::config::CommitUnit::Word);

                let (commit_text, replay_preedit) = if was_word {
                    engine.set_word_mode(true);
                    for (k, m) in &all_keys {
                        engine.press_key(*k, *m, config);
                    }
                    let full_word = engine.preedit_str().to_string();
                    engine.clear_commit();
                    (String::new(), full_word)
                } else {
                    for (k, m) in &fix.replay_keys {
                        engine.press_key(*k, *m, config);
                    }
                    let rp = engine.preedit_str().to_string();
                    engine.clear_commit();
                    (fix.commit_text.clone(), rp)
                };

                unim_log!(
                    "TSF_ATF",
                    "[TSF AutoTypeFix] 순방향 replay(word={}): preedit='{}', commit='{}'",
                    was_word,
                    replay_preedit,
                    commit_text
                );

                // 순방향(영→한) delete_chars = 입력한 영문 전체 길이.
                //
                // 과거엔 engine_worker(Linux DBus)를 따라 -1 했으나, 그건 Linux 가 트리거
                // 키의 commit 을 억제해 앱에 N-1 자만 있기 때문이다. TSF 경로는 영문 모드에서
                // 각 키가 조합 없이 즉시 commit 되어 앱(메모장 등)에 N 자가 모두 있으므로,
                // -1 하면 커서 앞 N-1 자만 지워져 첫 글자가 잔류한다("ntkd"→"n서기" 버그).
                // (역방향(한→영)은 마지막 음절이 조합 중=미commit 이라 별도 -1 보정이 정당.)
                // word 모드(Phase 4)는 호출부가 delete_chars 를 SetText 치환 길이검증에만 쓴다.
                let delete_chars = fix.delete_chars;

                Some(AutoFixApply {
                    delete_chars,
                    commit_text,
                    replay_preedit,
                    end_composition: false,
                    // 코어: 순방향 word 모드 = 보유 영문 라이브 조합 → SetText 치환.
                    replace_composition: fix.replace_composition,
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
                // I3(perf): new(config) 재생성 대신 reset() (순방향과 동일 이유 — 사전
                // 재로딩 회피). 역방향은 replay 가 없어 조합이 뒤따르지 않지만, 후속
                // 타이핑을 위해 new() 와 동일한 config 기반 accumulate_word 로 맞춘다.
                engine.reset();
                engine.set_input_category(current_cat);
                engine.set_word_mode(config.engine.korean.commit_unit == unim::config::CommitUnit::Word);

                let committed = fix
                    .delete_chars
                    .saturating_sub(if fix.clear_preedit { 1 } else { 0 });
                // fix.commit_text 이동 전에 Copy 필드 포착.
                let replace_composition = fix.replace_composition;

                crate::register::dbg_log_ev!(
                    &format!(
                        "ATF rev-apply: committed={} end_comp={} insert_len={}",
                        committed, fix.clear_preedit, fix.commit_text.chars().count()
                    ),
                    "ATF rev-apply: committed={} end_comp={} insert='{}'",
                    committed, fix.clear_preedit, fix.commit_text
                );

                Some(AutoFixApply {
                    delete_chars: committed,
                    commit_text: fix.commit_text,
                    replay_preedit: String::new(),
                    end_composition: fix.clear_preedit,
                    // 코어: 역방향 word 모드(committed=0) = 라이브 조합 → 영문 SetText 치환.
                    replace_composition,
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
