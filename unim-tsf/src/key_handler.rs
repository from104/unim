//! 키 이벤트 처리 — VK → KeyCode → InputEngine

use std::sync::atomic::{AtomicUsize, Ordering};

use windows::core::BOOL;
use windows::Win32::Foundation::*;
use windows::Win32::UI::TextServices::*;

use unim::config::{Config, InputCategory};
use unim::input_engine::InputEngine;
use unim::keycode::{KeyCode, ModifierState};

use crate::auto_typefix::{self, AutoTypeFixState};
use crate::composition::{self, CompositionManager, ReplaceOutcome};
use crate::popup_ipc::{to_render_state, PopupClient, RevEnvelope, RevEvent};
use crate::preedit_window::PreeditWindow;

/// `handle_key_down` 의 반환 — eaten + b1 Phase2 플러시 예약 신호.
///
/// `schedule_flush=true` 이면 순방향/역방향 synth 폴백이 Phase1(BS×N)만 수행하고
/// PendingInsert 를 적재했으므로, 호출부(text_service)가 SetTimer 로 Phase2 삽입을
/// 예약해야 한다.
pub struct KeyDownOutcome {
    pub eaten: bool,
    pub schedule_flush: bool,
}

/// 현재 Win32 수정자 키 상태를 조회합니다.
pub fn get_modifier_state() -> ModifierState {
    unim_windows_common::modifier::modifier_state_live()
}

/// OnTestKeyDown: 이 키를 소비할지 판단합니다.
pub fn test_key_down(
    engine: &InputEngine,
    config: &Config,
    wparam: WPARAM,
    _context: Option<&ITfContext>,
    popup_active: bool,
) -> bool {
    let vk = wparam.0 as u16;
    let keycode = KeyCode::from_win32_vk(vk);
    let modifiers = get_modifier_state();

    // [imm32 진입 진단] OnTestKeyDown 무조건 진입 로그. KakaoTalk/아래아한글에서
    // 키가 sink 에 아예 도달하는지(=이 줄이 찍히는지) 재현 확인용.
    crate::register::dbg_log(&format!(
        "OnTestKeyDown ENTER vk=0x{:02X} ctrl={} alt={} shift={} super={}",
        vk, modifiers.control, modifiers.alt, modifiers.shift, modifiers.super_key
    ));

    // 설정된 한/영 전환키 여부 (엔진과 동일 판정). RightAlt 같은 수정자 토글키도
    // 포함되므로 is_modifier 가드보다 먼저 구한다.
    let is_toggle = engine.is_toggle_key(keycode);

    // 수정자 키만 누른 경우 통과 — 단, 토글키(RightAlt 등)는 아래에서 소비 판정.
    if keycode.is_modifier() && !is_toggle {
        return false;
    }

    // Ctrl+Shift+Space: 수동 AutoTypeFix 소비
    if modifiers.control && modifiers.shift && keycode == KeyCode::Space {
        return true;
    }

    // 팝업 활성 시: 팝업 내비게이션 키를 모두 소비
    // (엔진이 press_key 내부에서 process_popup_key 로 처리)
    if popup_active {
        // Ctrl/Alt/Super 조합은 팝업 중에도 통과 (시스템 단축키 허용).
        // RightAlt 토글키도 Alt 비트를 세우므로 팝업 중엔 여기서 통과된다(종전 동작).
        if modifiers.control || modifiers.alt || modifiers.super_key {
            return false;
        }
        return is_popup_key(keycode);
    }

    // 한/영 전환 키는 소비 — RightAlt 같은 수정자 토글키도 포함하되, 단축키 조합
    // (Ctrl/Super, 또는 토글키가 수정자가 아닌데 Alt)일 때는 제외한다.
    // (엔진 press_key 의 토글 판정과 정확히 일치시켜, 소비 여부와 실제 토글 동작이
    //  어긋나지 않게 한다 — 어긋나면 RightAlt 가 먹히기만 하고 토글은 안 되는 버그.)
    if is_toggle {
        let self_is_modifier = keycode.is_modifier();
        let shortcut_combo = modifiers.control
            || modifiers.super_key
            || (modifiers.alt && !self_is_modifier);
        if !shortcut_combo {
            return true;
        }
    }

    // Ctrl/Alt/Super 조합은 통과 (단축키)
    if modifiers.control || modifiers.alt || modifiers.super_key {
        return false;
    }

    // 한자/F9 키
    if keycode == KeyCode::Hanja || keycode == KeyCode::F9 {
        return true;
    }

    // 한국어 모드: 문자 키 소비
    if engine.input_category() == InputCategory::Korean {
        if keycode.is_character_key() {
            return true;
        }
    }

    // 영문 모드 + ATF 순방향 ON: 문자 키를 소비해 IME 경로로 커밋한다.
    // 소비하지 않으면(OnTestKeyDown=FALSE) 대부분의 앱이 OnKeyDown 을 아예
    // 호출하지 않아 ATF 가 영문 키를 관찰할 수 없다 — IMM32 앱에서 순방향이
    // 무반응이던 원인. 엔진은 영문 문자를 consumed=true/commit='x' 로 직접
    // 커밋하므로(메모장 검증 경로) 일반 타이핑 결과는 동일하다.
    if engine.input_category() == InputCategory::English
        && keycode.is_character_key()
        && config.engine.auto_typefix.enabled
        && config.engine.auto_typefix.forward
    {
        return true;
    }

    // 조합 중: Backspace, Space, Enter 등 소비
    if engine.is_composing() {
        matches!(
            keycode,
            KeyCode::Backspace
                | KeyCode::Space
                | KeyCode::Enter
                | KeyCode::Tab
                | KeyCode::Escape
                | KeyCode::Left
                | KeyCode::Right
                | KeyCode::Up
                | KeyCode::Down
        )
    } else {
        false
    }
}

/// 조합 중 "확정 후 앱으로 통과"시켜야 하는 네비게이션/기능 키.
///
/// OnTestKeyDown 에서 이 키들은 현재 composition 을 확정하고 pIsEaten=FALSE 로
/// 흘려보낸다(NavilIME 패턴). 그래야 OnKeyDown 이 호출되지 않아 키가 IME 에
/// 먹히지 않고, 앱이 Enter(개행)·화살표(커서 이동)·Tab·Esc 를 직접 처리한다.
/// (commit 을 OnKeyDown 에서 한 뒤 false 를 반환하면 CUAS 가 이미 test 단계에서
/// claim 된 키를 앱으로 흘려보내지 않아 동작하지 않는다.) 팝업 활성 시에는
/// 이 키들이 후보 내비게이션용이므로 적용하지 않는다(호출부에서 가드).
pub fn is_commit_passthrough_key(keycode: KeyCode) -> bool {
    matches!(
        keycode,
        KeyCode::Enter
            | KeyCode::Tab
            | KeyCode::Escape
            | KeyCode::Left
            | KeyCode::Right
            | KeyCode::Up
            | KeyCode::Down
            | KeyCode::Home
            | KeyCode::End
            | KeyCode::PageUp
            | KeyCode::PageDown
    )
}

/// 팝업 활성 시 소비해야 할 키인지 판단.
///
/// 엔진의 `keycode_to_popup_key` 매핑과 동일한 범위를 소비한다.
fn is_popup_key(keycode: KeyCode) -> bool {
    matches!(
        keycode,
        // 숫자 직접 선택
        KeyCode::Num1
            | KeyCode::Num2
            | KeyCode::Num3
            | KeyCode::Num4
            | KeyCode::Num5
            | KeyCode::Num6
            | KeyCode::Num7
            | KeyCode::Num8
            | KeyCode::Num9
            // 방향키 내비게이션
            | KeyCode::Up
            | KeyCode::Down
            | KeyCode::Left
            | KeyCode::Right
            // 페이지 이동
            | KeyCode::PageUp
            | KeyCode::PageDown
            | KeyCode::Home
            | KeyCode::End
            // 기능키
            | KeyCode::Enter
            | KeyCode::Escape
            | KeyCode::Tab
            | KeyCode::Space    // 북마크 토글
            | KeyCode::Backspace
            | KeyCode::Period   // 9x9 ↔ compact 토글
            // 컬럼 점프: Q~O (물리 위치 기준)
            | KeyCode::Q
            | KeyCode::W
            | KeyCode::E
            | KeyCode::R
            | KeyCode::T
            | KeyCode::Y
            | KeyCode::U
            | KeyCode::I
            | KeyCode::O
            // 이모지 카테고리 단축키: A~L (홈 행)
            | KeyCode::A
            | KeyCode::S
            | KeyCode::D
            | KeyCode::F
            | KeyCode::G
            | KeyCode::H
            | KeyCode::J
            | KeyCode::K
            | KeyCode::L
    )
}

/// composition range 의 스크린 좌표를 구한다.
/// 실패 시 `None` 반환 → 호출자에서 마우스 커서 fallback.
fn get_composition_screen_pos(context: &ITfContext, tid: u32) -> Option<(i32, i32)> {
    unsafe {
        let view: ITfContextView = context.GetActiveView().ok()?;
        // 현재 selection range 를 구한다.
        // GetSelection(ec, ulindex, pselection, pcfetched) — ulindex: TF_DEFAULT_SELECTION(u32::MAX)
        let mut sel = TF_SELECTION::default();
        let mut fetched: u32 = 0;
        context
            .GetSelection(u32::MAX, 1, std::slice::from_mut(&mut sel), &mut fetched)
            .ok()?;
        if fetched == 0 {
            return None;
        }
        let range = sel.range.as_ref()?;
        let mut rect = RECT::default();
        let mut clipped = BOOL::default();
        view.GetTextExt(tid, range, &mut rect, &mut clipped)
            .ok()?;
        // 텍스트 rect 의 아래쪽 왼쪽을 팝업 위치로 사용
        Some((rect.left, rect.bottom))
    }
}

/// OnKeyDown: 실제 키 처리 + 조합 갱신 + 팝업 라우팅 + AutoTypeFix
pub fn handle_key_down(
    engine: &mut InputEngine,
    config: &Config,
    comp_mgr: &mut CompositionManager,
    popup: &mut PopupClient,
    preedit_win: &mut Option<PreeditWindow>,
    atf_state: &mut AutoTypeFixState,
    context: &ITfContext,
    tid: u32,
    wparam: WPARAM,
    comp_sink: &ITfCompositionSink,
    // composition 미지원 앱(wezterm 등) 폴백 모드. text_service 가 전달.
    composition_unsupported: bool,
    // 폴백 모드에서 문서에 떠있는 미확정 글자 수 (다음 키에서 지울 양).
    fallback_pending: &AtomicUsize,
) -> KeyDownOutcome {
    let vk = wparam.0 as u16;
    let keycode = KeyCode::from_win32_vk(vk);
    let modifiers = get_modifier_state();

    // ── Ctrl+Shift+Space: 수동 AutoTypeFix (typefix_convert) ──
    //
    // 갭 2 수정: TSF ReadOnly EditSession 으로 선택 영역 텍스트를 읽어
    // engine.set_surrounding_text() 설정 후 typefix_convert() 를 호출한다.
    // 선택 읽기 실패(비선택 상태, 앱 거부 등) 시 기존처럼 set_surrounding 없이 호출(fallback).
    if modifiers.control && modifiers.shift && keycode == KeyCode::Space {
        // 선택 영역 읽기 시도
        if let Some(sel) = composition::read_selection_text(context, tid) {
            // 선택 영역이 실제로 존재하는 경우만 (cursor != anchor)
            if sel.cursor != sel.anchor {
                engine.set_surrounding_text(sel.surrounding_text, sel.cursor, sel.anchor);
            }
        }
        // surrounding 설정 여부에 무관하게 typefix_convert 호출
        let mut schedule_flush = false;
        if let Some((_offset, delete_count, replacement)) = engine.typefix_convert(0) {
            let outcome = comp_mgr.replace_surrounding(
                context,
                tid,
                delete_count as u32,
                &replacement,
                "",
                comp_sink,
            );
            // 수동 typefix 는 preedit="" 라 Normal(native) 또는 SynthBatch(synth)만
            // 발생한다(PhaseSplit 은 미발생 — exhaustive 위해 arm 만 존재).
            match outcome {
                ReplaceOutcome::Normal => {}
                ReplaceOutcome::PhaseSplit => schedule_flush = true, // native(형식상 안전망)
                ReplaceOutcome::SynthBatch => engine.remove_preedit(),
            }
        }
        return KeyDownOutcome { eaten: true, schedule_flush };
    }

    // ── Ctrl+Z AutoTypeFix 되돌리기 (press_key 전에 먼저 검사) ──
    if let Some(apply) = auto_typefix::try_undo(atf_state, keycode, modifiers) {
        let outcome = comp_mgr.replace_surrounding(
            context,
            tid,
            apply.delete_chars,
            &apply.commit_text,
            &apply.replay_preedit,
            comp_sink,
        );
        // undo 는 replay_preedit="" 라 Normal(native) 또는 SynthBatch(synth)만 발생한다
        // (PhaseSplit 은 미발생 — exhaustive 위해 arm 만 존재).
        let mut schedule_flush = false;
        match outcome {
            ReplaceOutcome::Normal => {}
            ReplaceOutcome::PhaseSplit => schedule_flush = true,
            ReplaceOutcome::SynthBatch => engine.remove_preedit(),
        }
        return KeyDownOutcome { eaten: true, schedule_flush };
    }

    // ── Backspace 관찰 (blacklist 재트리거 감지용) ──
    if keycode == KeyCode::Backspace {
        auto_typefix::observe_backspace(atf_state, &config.engine.auto_typefix);
    }

    let was_composing = engine.is_composing();
    let prev_mode = engine.input_category();
    let result = engine.press_key(keycode, modifiers, config);

    crate::register::dbg_log(&format!(
        "handle_key_down: vk=0x{:02X} consumed={} commit_changed={} preedit_changed={} was_composing={} comp_active={}",
        vk, result.consumed, result.commit_changed, result.preedit_changed, was_composing, comp_mgr.is_active()
    ));

    // 모드 전환 관찰 (사용자 수동 전환 — ATF 자체 전환과 구분은 process_after_key 내부에서)
    let current_mode = engine.input_category();
    if prev_mode != current_mode {
        auto_typefix::observe_mode_switch(atf_state, &config.engine.auto_typefix);
    }

    if !result.consumed && !result.commit_changed && !result.preedit_changed {
        // 팝업 중에도 소비하지 않은 키면 팝업 닫기 (Esc 등은 엔진이 HidePopup emit)
        return KeyDownOutcome { eaten: false, schedule_flush: false };
    }

    // ── 팝업 액션 drain ──
    // press_key 가 내부적으로 process_popup_key 를 수행했으면 PopupAction 이 쌓임.
    // 액션 루프는 first(Show*)/hide(HidePopup)/flash(★해제) 플래그만 수집하고,
    // 렌더 데이터는 엔진 SoT view_model → to_render_state(§4) → send_render 로 보낸다.
    // 이것이 H3/H4/H5/H9/H14 일괄 해소 지점 (첫 Show 부터 격자·헤더·뜻·★ 정확).
    drain_popup_actions(engine, popup);

    // ── commit / preedit 처리 ──
    let commit_str_for_atf: Option<String>;
    let preedit_str_for_atf: Option<String>;

    if composition_unsupported {
        // ── 폴백 경로: client-side preedit (wezterm 등 터미널·레거시 앱) ──
        //
        // 과거 방식(매 키마다 문서를 del+재삽입)은 터미널에서 깨졌다: ShiftStart
        // 역방향 삭제를 앱이 거부(shifted=0) → 삭제가 안 먹고 삽입만 누적되어
        // "ㄱ기깋기혀현현"(실측)이 됐다. 삽입(SetText)만 정상 동작.
        //
        // 새 방식(리눅스 fcitx/ibus 의 client-side preedit): 조합 중 음절(preedit)은
        // 문서가 아니라 UNIM 오버레이 창(preedit_win)에 그린다. 엔진 commit 은
        // append-only 라 절대 되돌릴 필요가 없으므로, 확정된 글자만 insert_text 로
        // 문서에 추가한다 → 삭제가 영원히 불필요.
        let commit = if result.commit_changed {
            let c = engine.commit_str().to_string();
            engine.clear_commit();
            c
        } else {
            String::new()
        };
        let preedit = engine.preedit_str().to_string();
        // 폴백 경로에서는 ATF(주변 텍스트 교체)도 동일 삭제 한계로 깨지므로 비활성.
        commit_str_for_atf = None;
        preedit_str_for_atf = None;

        if result.commit_changed || result.preedit_changed {
            crate::register::dbg_log(&format!(
                "fallback(overlay): commit='{}' preedit='{}'",
                commit, preedit
            ));
            // 1) 확정 글자 → 문서에 영구 삽입 (삭제 없음, wezterm 정상 동작)
            if !commit.is_empty() {
                comp_mgr.insert_text(context, tid, &commit);
            }
            // 2) 조합 중 음절 → 오버레이 창 (캐럿 위치는 selection 기준)
            if preedit.is_empty() {
                if let Some(win) = preedit_win.as_mut() {
                    win.hide();
                }
            } else {
                let pos = get_composition_screen_pos(context, tid);
                let win = preedit_win
                    .get_or_insert_with(|| PreeditWindow::create().expect("PreeditWindow 생성 실패"));
                win.show(&preedit, pos);
            }
            // reload-guard / is_busy 가 참조하는 "활성 preedit" 표식.
            fallback_pending.store(preedit.chars().count(), Ordering::SeqCst);
        }
    } else {
        // ── 정상 경로 (composition 지원 앱: 메모장 등) ──
        let commit = if result.commit_changed {
            let c = engine.commit_str().to_string();
            engine.clear_commit();
            c
        } else {
            String::new()
        };
        let preedit = engine.preedit_str().to_string();
        commit_str_for_atf = if !commit.is_empty() { Some(commit.clone()) } else { None };
        preedit_str_for_atf = if result.preedit_changed { Some(preedit.clone()) } else { None };

        let composing = was_composing || comp_mgr.is_active();

        if !commit.is_empty() && result.preedit_changed && !preedit.is_empty() && composing {
            // 음절 전환(확정 + 새 조합)을 단일 edit session 으로 처리한다.
            // 과거엔 end_composition_with_text(commit) + start_composition(preedit) 를
            // 두 개의 별도 sync 세션으로 호출했는데, CUAS(wezterm 등 IMM32 브리지)가
            // "조합 종료 직후 새 조합 시작"을 거부해 새 composition 을 즉시
            // OnCompositionTerminated 시켰다(매 음절 전환마다 오버레이로 떨어짐).
            // 한 트랜잭션(EndComposition→StartComposition)으로 합치면 CUAS 가 연속
            // 조합으로 인식할 여지가 생긴다.
            comp_mgr.commit_and_restart(context, tid, &commit, &preedit, comp_sink);
        } else {
            // commit 처리
            if !commit.is_empty() {
                if composing {
                    comp_mgr.end_composition_with_text(context, tid, &commit);
                } else {
                    // 비조합 commit(영문 문자 등)은 단순 삽입(InsertTextEditSession).
                    //
                    // 과거엔 replace_surrounding(delete=0) 으로 composition 으로 감싸
                    // 삽입했으나, 영문 비조합 글자는 미확정 조합이 아닌데도 매 글자
                    // StartComposition→EndComposition 을 돌려 CUAS(카톡 등 IMM32 브리지)
                    // 가 즉시-terminate 학습(cuas_windows)하게 만들었다 → ATF 게이트가
                    // 영구 폴백되어 교정이 아예 안 됨(B2 1차 원인). 단순 SetText 삽입은
                    // composition 라이프사이클을 만들지 않아 오학습을 막는다. (delete=0
                    // 삽입은 ShiftStart 역확장이 없어 Blink 첫 글자 누락과도 무관.)
                    comp_mgr.insert_text(context, tid, &commit);
                }
            }
            // preedit 처리
            if result.preedit_changed {
                if preedit.is_empty() {
                    if comp_mgr.is_active() {
                        comp_mgr.end_composition(context, tid);
                    }
                } else if comp_mgr.is_active() {
                    comp_mgr.update_composition(context, tid, &preedit);
                } else {
                    comp_mgr.start_composition(context, tid, &preedit, comp_sink);
                }
            }
        }
    }

    // ── AutoTypeFix 오케스트레이션 ──
    // 팝업 활성 중에는 발동 금지 (엔진이 popup_key 처리 중).
    //
    // CUAS/폴백(터미널·레거시·카톡 등 IMM32 브리지) 경로에서도 ATF 를 적용한다.
    // 과거엔 `!composition_unsupported` 가드로 CUAS 앱에서 ATF 가 전면 봉쇄돼
    // 원문이 잔류했다(B2 1차 원인). replace_surrounding 이 GetSelection 기반
    // ShiftStart 역확장 + shifted 검증으로 확정 텍스트(result-string)까지 교체
    // 하도록 보강(composition.rs)됐으므로, CUAS 에서도 동일 경로로 교정한다.
    let popup_active = popup.is_active();
    let mut schedule_flush = false;
    if !popup_active {
        if let Some(apply) = auto_typefix::process_after_key(
            atf_state,
            engine,
            config,
            keycode,
            modifiers,
            prev_mode,
            was_composing,
            commit_str_for_atf.as_deref(),
            preedit_str_for_atf.as_deref(),
        ) {
            // 역방향(한→영): replace_surrounding 전에 활성 composition 을 먼저 종료해
            // 조합 중 음절을 문서에서 제거한다. 그래야 남은 committed 한글만 delete 하면
            // 화면의 한글이 정확히 모두 사라진다(조합 음절 잔류 방지).
            if apply.end_composition && comp_mgr.is_active() {
                comp_mgr.end_composition(context, tid);
            }
            let outcome = comp_mgr.replace_surrounding(
                context,
                tid,
                apply.delete_chars,
                &apply.commit_text,
                &apply.replay_preedit,
                comp_sink,
            );
            match outcome {
                ReplaceOutcome::Normal => {}
                ReplaceOutcome::PhaseSplit => {
                    // native(full=true) 펌프-분할: 세션1 이 full 조합을 만들었고 commit
                    // 정착은 SetTimer → WM_UNIM_FLUSH2 → Phase2b(commit_and_restart)로
                    // 수행한다. 마지막 음절("기")을 TSF 조합으로 유지하므로 엔진의 잔여
                    // preedit("기")을 **보존**한다(remove_preedit 금지). 그래야 사용자가
                    // 이어서 받침을 쳐도 엔진 상태가 일치한다(replay 후 engine.clear_commit()
                    // 은 commit 만 비우고 preedit "기" 는 보존 — auto_typefix.rs:389 확인).
                    schedule_flush = true;
                    crate::register::dbg_log(
                        "native: PhaseSplit → SetTimer→WM_UNIM_FLUSH2 Phase2b, 엔진 preedit 보존",
                    );
                }
                ReplaceOutcome::SynthBatch => {
                    // 단일배치가 교정문 전체(commit+마지막음절)를 확정했다 →
                    // 보존돼 있던 마지막음절 preedit("기")까지 비워 문서=엔진 동기.
                    // schedule_flush 는 false 유지(타이머/Phase2 미진입).
                    engine.remove_preedit();
                    crate::register::dbg_log("synth 단일배치 → 엔진 preedit 클리어(no flush)");
                }
            }
        }
    }

    KeyDownOutcome { eaten: result.consumed, schedule_flush }
}

/// b1 Phase2 — 보류 삽입(PendingInsert)을 TSF 로 삽입한다 (타이머 발화 / race-flush
/// 공용). 1회성: take_pending_insert() 가 None 이면 즉시 반환(중복 호출 무해).
///
/// - commit_text("서")는 composition 으로 감싸 확정 삽입.
/// - last_syllable("기")은 start_composition 으로 미확정 조합 유지.
/// - Blink 가 신규 조합을 즉시 terminate 하면 확정 폴백 + 엔진 preedit 제거(동기).
///
/// 호출자는 engine·comp_mgr 락을 보유한 상태로 호출한다(SendInput 미사용이라
/// edit session 안전). 어떤 실패도 패닉 금지(dbg_log).
///
/// 반환: 실제로 삽입을 수행했으면 true(타이머가 KillTimer 판단에 사용).
pub fn flush_pending_insert(
    engine: &mut InputEngine,
    comp_mgr: &mut CompositionManager,
    context: &ITfContext,
    tid: u32,
    comp_sink: &ITfCompositionSink,
) -> bool {
    let Some(pending) = crate::synth_input::take_pending_insert() else {
        return false;
    };
    // D3: 실제 삽입을 1회 수행하는 이 시점에 read-back 게이트도 함께 해제한다(타이머·
    // race-flush·WM_UNIM_FLUSH·degrade 공용 진입점이라 stale 게이트 잔존을 원천 차단).
    crate::synth_input::disarm_readback_gate();
    crate::register::dbg_log(&format!(
        "b1: flush commit_len={} last_len={}",
        pending.commit_text.chars().count(),
        pending.last_syllable.chars().count()
    ));
    let composed_fallback = comp_mgr.insert_pending(
        context,
        tid,
        &pending.commit_text,
        &pending.last_syllable,
        comp_sink,
    );
    // 마지막 음절을 조합으로 유지하지 못하고 확정 폴백했으면(Blink 즉시 terminate)
    // 엔진의 잔여 preedit("기")도 비워 "문서=확정, 엔진=빈 조합" 동기를 맞춘다.
    if composed_fallback {
        engine.remove_preedit();
        crate::register::dbg_log("b1: flush compose 폴백 → 엔진 preedit 리셋");
    }
    true
}

/// b1[D] Phase2b — 메시지 펌프(WM_UNIM_FLUSH2) 후 보류 재시작을 commit_and_restart 로
/// 확정한다. Phase2a(insert_pending case3)가 세션A 조합을 만들고 PENDING_RESTART 를
/// 적재한 뒤 호출부가 펌프를 돌린 다음 진입한다. 보류 재시작이 없으면 no-op.
///
/// 호출자는 engine→composition 락을 보유한 상태로 호출한다(SendInput 미사용 →
/// edit session 안전). 어떤 실패도 패닉 금지(dbg_log).
/// 반환: 실제로 확정을 수행했으면 true.
pub fn flush_restart_phase_b(
    engine: &mut InputEngine,
    comp_mgr: &mut CompositionManager,
    context: &ITfContext,
    tid: u32,
    comp_sink: &ITfCompositionSink,
) -> bool {
    let Some((commit_text, last_syllable)) = crate::synth_input::take_pending_restart() else {
        return false;
    };
    crate::register::dbg_log(&format!(
        "b1[D]: Phase2b flush commit_len={} last_len={}",
        commit_text.chars().count(),
        last_syllable.chars().count()
    ));
    let composed_fallback =
        comp_mgr.insert_restart_phase_b(context, tid, &commit_text, &last_syllable, comp_sink);
    if composed_fallback {
        engine.remove_preedit();
        crate::register::dbg_log("b1[D]: Phase2b compose 폴백 → 엔진 preedit 리셋");
    }
    true
}

/// engine 의 pending PopupAction 을 모두 소비해 out-of-process 렌더러로 송신한다 (§6.5).
///
/// 액션 루프는 **플래그만 수집**한다:
/// - `first` = Show*(ShowHanja/ShowSpecial/ShowEmoji) 수신 (새 팝업).
/// - `hide`  = HidePopup 수신 (선택 확정 또는 Esc).
/// - `flash` = HanjaCandidatesReordered 에서 `was_bookmarked && !bookmarked` (★ 해제 신호).
///
/// 렌더 데이터는 액션이 아니라 **엔진 SoT** 에서 추출한다:
/// `engine.popup_state().view_model(home_row)` → `to_render_state`(§4 column-major 평탄화)
/// → `send_render`. rows/cols/total_pages·헤더·뜻·탭·초기★ 가 첫 Show 부터 정확하므로
/// H3(전치)/H4(하이라이트)/H5(첫표시 격자)/H9(헤더·뜻)/H14(초기★)가 일괄 해소된다.
///
/// `selected`(=sel_row) 필드는 어디서도 사용하지 않는다 (좌표 SoT 만 신뢰).
fn drain_popup_actions(engine: &mut InputEngine, popup: &mut PopupClient) {
    use unim::input_engine::PopupAction;

    let mut first = false; // Show* 수신
    let mut hide = false; // HidePopup 수신
    let mut flash = false; // ★ 해제 신호
    let mut any = false;

    while let Some(action) = engine.take_popup_action() {
        any = true;
        // PopupAction 의 모든 variant 를 명시 match (신규 variant 추가 시 컴파일 에러로 감지).
        match &action {
            PopupAction::ShowHanja { .. }
            | PopupAction::ShowSpecial { .. }
            | PopupAction::ShowEmoji { .. } => first = true,
            PopupAction::HidePopup => hide = true,
            PopupAction::HanjaCandidatesReordered {
                bookmarked,
                was_bookmarked,
                ..
            } => flash = *was_bookmarked && !*bookmarked,
            // Navigate/PageJump/BookmarkChanged → view_model 이 전부 반영하므로 플래그 불필요.
            PopupAction::PopupNavigate { .. }
            | PopupAction::PageJump { .. }
            | PopupAction::HanjaBookmarkChanged { .. } => {}
        }
        crate::register::dbg_log(&format!(
            "popup_ipc: drain action variant first={first} hide={hide} flash={flash}"
        ));
    }

    if hide {
        popup.hide();
        return;
    }
    if !any {
        return;
    }

    // 엔진 SoT 에서 완성된 view model 추출 (H3/H4/H5/H9/H14 일괄 해소 지점).
    let home_row = engine.home_row_labels().to_string();
    if let Some(state) = engine.popup_state() {
        let rs = to_render_state(&state.view_model(&home_row));
        popup.send_render(rs, first, flash);
    } else if popup.is_active() {
        // 액션은 있었는데 상태가 없음 — 방어적 Hide.
        crate::register::dbg_log("popup_ipc: actions present but popup_state()=None → defensive hide");
        popup.hide();
    }
}

/// 역채널 마우스 이벤트를 엔진에 적용한다 (§11.G — TSF 스레드 wndproc 에서만 호출).
///
/// **코어 무수정 제약**: `popup_select`/`popup_cancel` 은 `pub(super)` 라 직접 호출
/// 불가하므로, 동등한 **공개 `press_key` 경로**로 라우팅한다:
/// - cell 선택 → `handle_click` 으로 sel 이동 후 `press_key(Enter)` (Enter→Select→popup_select)
/// - 외부 취소 → `press_key(Escape)` (Escape→Cancel→popup_cancel: 원본 한글 커밋+HidePopup)
/// - 탭 전환 → `press_key(CatLetter 키)` (엔진 카테고리 점프 + ShowEmoji 재발행)
/// 키보드 경로와 **완전 동일한 엔진 상태 전이**를 재사용하므로 SoT 일관성이 보장된다.
///
/// 적용 후 `drain_popup_actions` 로 view_model 재전송/hide 하고, `commit_buffer` 가
/// 차 있으면 보관된 `ITfContext` 로 비조합 문서 삽입한다.
///
/// 어떤 실패도 panic 금지 — 전부 `dbg_log`. 엔진 락은 호출자(wndproc)가 짧게 보유.
/// `last_owner`/`last_seq` 는 마지막 send_render 의 식별자 (stale 차단용).
#[allow(clippy::too_many_arguments)]
pub fn apply_reverse_event(
    engine: &mut InputEngine,
    config: &Config,
    comp_mgr: &mut CompositionManager,
    popup: &mut PopupClient,
    context: Option<&ITfContext>,
    tid: u32,
    comp_sink: &ITfCompositionSink,
    env: &RevEnvelope,
    last_owner: u64,
    last_seq: u64,
) {
    use unim::keycode::{KeyCode, ModifierState};

    // ── stale 차단: 포커스 전환 race 로 옛 팝업에 보낸 이벤트는 무시 ──
    // owner_hwnd 와 seq 가 모두 마지막 send_render 와 일치해야 적용한다.
    if env.owner_hwnd != last_owner || env.seq != last_seq {
        crate::register::dbg_log(&format!(
            "popup_rev: stale evt owner={:#x}/cur={:#x} seq={}/cur={} → ignore",
            env.owner_hwnd, last_owner, env.seq, last_seq
        ));
        return;
    }
    // 팝업이 이미 닫혔으면 무시.
    if engine.popup_state().is_none() {
        crate::register::dbg_log("popup_rev: popup_state()=None → ignore evt");
        return;
    }

    let modifiers = ModifierState::default();
    crate::register::dbg_log(&format!("popup_rev: apply {:?}", env.event));

    match &env.event {
        RevEvent::CellClick { row, col } => {
            // 1) 클릭한 셀로 selection 이동 (column-major 좌표 그대로 엔진에 전달).
            let result = match engine.popup_state_mut() {
                Some(state) => state.handle_click(*row as usize, *col as usize),
                None => return,
            };
            use unim::popup::PopupKeyResult;
            match result {
                PopupKeyResult::Select(_) => {
                    // 키보드 Enter 와 동일 경로로 확정 (popup_select → commit_buffer+HidePopup).
                    let _ = engine.press_key(KeyCode::Enter, modifiers, config);
                }
                _ => {
                    crate::register::dbg_log(
                        "popup_rev: cell_click non-select (empty/consumed) → re-render only",
                    );
                }
            }
        }
        RevEvent::PageClick { dir } => {
            let page_dir = if *dir == crate::popup_ipc::evt_kind::PAGE_DIR_PREV {
                unim::input_engine::PageDirection::Prev
            } else {
                unim::input_engine::PageDirection::Next
            };
            let _ = engine.popup_change_page(page_dir);
        }
        RevEvent::TabClick { index } => {
            // 이모지 카테고리 점프 — 키보드 CatLetter(홈행 ASDFGHJKL) 경로 재사용.
            // 엔진이 refresh_emoji_category_items + ShowEmoji 재발행 + sel 리셋을 일괄 처리.
            let key = cat_index_to_keycode(*index);
            if let Some(kc) = key {
                let _ = engine.press_key(kc, modifiers, config);
            } else {
                crate::register::dbg_log(&format!(
                    "popup_rev: tab_click index={index} out of 0..8 → ignore"
                ));
            }
        }
        RevEvent::ExpandToggle => {
            // 한자 확장/축소 — PopupState 의 공개 메서드 직접 호출 (키보드 Period 동치).
            if let Some(state) = engine.popup_state_mut() {
                state.toggle_hanja_expanded();
            }
        }
        RevEvent::OutsideCancel => {
            // 키보드 Escape 와 동일 경로 → popup_cancel (원본 한글 커밋 + HidePopup).
            let _ = engine.press_key(KeyCode::Escape, modifiers, config);
        }
    }

    // ── 적용 후 재렌더/확정 처리 ──
    // pending PopupAction 을 소비해 view_model 재전송 또는 hide.
    drain_popup_actions(engine, popup);

    // 마우스 확정 텍스트(commit_buffer)가 있으면 보관 컨텍스트로 비조합 문서 삽입.
    let commit = if !engine.commit_str().is_empty() {
        let c = engine.commit_str().to_string();
        engine.clear_commit();
        c
    } else {
        String::new()
    };
    if !commit.is_empty() {
        match context {
            Some(ctx) => {
                // 비조합 삽입 (키보드 Enter 와 달리 활성 composition 없음).
                // 잔류 composition 이 있으면 먼저 종료해 잔상 방지.
                if comp_mgr.is_active() {
                    comp_mgr.end_composition(ctx, tid);
                }
                comp_mgr.insert_text(ctx, tid, &commit);
                crate::register::dbg_log(&format!("popup_rev: commit inserted '{commit}'"));
            }
            None => {
                crate::register::dbg_log(&format!(
                    "popup_rev: no context — commit '{commit}' deferred (dropped)"
                ));
            }
        }
    }
    let _ = comp_sink; // 비조합 삽입 경로는 comp_sink 불필요 (서명 일관성 위해 보유).
}

/// 이모지 카테고리 인덱스(0..8) → 홈행 물리 키 KeyCode (ASDFGHJKL).
///
/// 엔진 `keycode_to_popup_key` 의 `KeyCode::A..L → CatLetter(0..8)` 매핑과 동일.
fn cat_index_to_keycode(index: u32) -> Option<unim::keycode::KeyCode> {
    use unim::keycode::KeyCode;
    Some(match index {
        0 => KeyCode::A,
        1 => KeyCode::S,
        2 => KeyCode::D,
        3 => KeyCode::F,
        4 => KeyCode::G,
        5 => KeyCode::H,
        6 => KeyCode::J,
        7 => KeyCode::K,
        8 => KeyCode::L,
        _ => return None,
    })
}
