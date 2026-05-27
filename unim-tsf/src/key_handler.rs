//! 키 이벤트 처리 — VK → KeyCode → InputEngine

use windows::core::BOOL;
use windows::Win32::Foundation::*;
use windows::Win32::UI::Input::KeyboardAndMouse::*;
use windows::Win32::UI::TextServices::*;

use unim::config::{Config, InputCategory};
use unim::input_engine::{InputEngine, InputResult};
use unim::keycode::{KeyCode, ModifierState};

use crate::auto_typefix::{self, AutoTypeFixState};
use crate::composition::{self, CompositionManager};
use crate::popup_window::PopupWindow;

/// 현재 Win32 수정자 키 상태를 조회합니다.
fn get_modifier_state() -> ModifierState {
    unsafe {
        let shift = GetKeyState(VK_SHIFT.0 as i32) < 0;
        let control = GetKeyState(VK_CONTROL.0 as i32) < 0;
        let alt = GetKeyState(VK_MENU.0 as i32) < 0;
        let super_key = GetKeyState(VK_LWIN.0 as i32) < 0 || GetKeyState(VK_RWIN.0 as i32) < 0;
        let caps_lock = (GetKeyState(VK_CAPITAL.0 as i32) & 0x01) != 0;
        let num_lock = (GetKeyState(VK_NUMLOCK.0 as i32) & 0x01) != 0;

        ModifierState {
            shift,
            control,
            alt,
            super_key,
            caps_lock,
            num_lock,
        }
    }
}

/// OnTestKeyDown: 이 키를 소비할지 판단합니다.
pub fn test_key_down(
    engine: &InputEngine,
    _config: &Config,
    wparam: WPARAM,
    _context: Option<&ITfContext>,
    popup_active: bool,
) -> bool {
    let vk = wparam.0 as u16;
    let keycode = KeyCode::from_win32_vk(vk);
    let modifiers = get_modifier_state();

    // 수정자 키만 누른 경우 통과
    if keycode.is_modifier() {
        return false;
    }

    // Ctrl+Shift+Space: 수동 AutoTypeFix 소비
    if modifiers.control && modifiers.shift && keycode == KeyCode::Space {
        return true;
    }

    // 팝업 활성 시: 팝업 내비게이션 키를 모두 소비
    // (엔진이 press_key 내부에서 process_popup_key 로 처리)
    if popup_active {
        // Ctrl/Alt/Super 조합은 팝업 중에도 통과 (시스템 단축키 허용)
        if modifiers.control || modifiers.alt || modifiers.super_key {
            return false;
        }
        return is_popup_key(keycode);
    }

    // Ctrl/Alt/Super 조합은 통과 (단축키)
    if modifiers.control || modifiers.alt || modifiers.super_key {
        return false;
    }

    // 한/영 전환 키는 항상 소비
    if keycode == KeyCode::Korean || keycode == KeyCode::RightAlt {
        return true;
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
    popup_win: &mut Option<PopupWindow>,
    atf_state: &mut AutoTypeFixState,
    context: &ITfContext,
    tid: u32,
    wparam: WPARAM,
    comp_sink: &ITfCompositionSink,
) -> bool {
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
        if let Some((_offset, delete_count, replacement)) = engine.typefix_convert(0) {
            comp_mgr.replace_surrounding(
                context,
                tid,
                delete_count as u32,
                &replacement,
                "",
                comp_sink,
            );
        }
        return true;
    }

    // ── Ctrl+Z AutoTypeFix 되돌리기 (press_key 전에 먼저 검사) ──
    if let Some(apply) = auto_typefix::try_undo(atf_state, keycode, modifiers) {
        comp_mgr.replace_surrounding(
            context,
            tid,
            apply.delete_chars,
            &apply.commit_text,
            &apply.replay_preedit,
            comp_sink,
        );
        return true;
    }

    // ── Backspace 관찰 (blacklist 재트리거 감지용) ──
    if keycode == KeyCode::Backspace {
        auto_typefix::observe_backspace(atf_state, &config.engine.auto_typefix);
    }

    let was_composing = engine.is_composing();
    let prev_mode = engine.input_category();
    let result = engine.press_key(keycode, modifiers, config);

    // 모드 전환 관찰 (사용자 수동 전환 — ATF 자체 전환과 구분은 process_after_key 내부에서)
    let current_mode = engine.input_category();
    if prev_mode != current_mode {
        auto_typefix::observe_mode_switch(atf_state, &config.engine.auto_typefix);
    }

    if !result.consumed && !result.commit_changed && !result.preedit_changed {
        // 팝업 중에도 소비하지 않은 키면 팝업 닫기 (Esc 등은 엔진이 HidePopup emit)
        return false;
    }

    // ── 팝업 액션 drain ──
    // press_key 가 내부적으로 process_popup_key 를 수행했으면 PopupAction 이 쌓임.
    // ShowHanja/ShowSpecial/ShowEmoji → 팝업 표시
    // PopupNavigate/PageJump/HanjaBookmarkChanged/HanjaCandidatesReordered → 팝업 갱신
    // HidePopup → 팝업 숨김 (선택 확정 또는 Esc)
    drain_popup_actions(engine, popup_win, context, tid, result, comp_mgr);

    // ── commit 처리 ──
    let commit_str_for_atf: Option<String>;
    if result.commit_changed {
        let commit = engine.commit_str().to_string();
        engine.clear_commit();

        commit_str_for_atf = if !commit.is_empty() {
            Some(commit.clone())
        } else {
            None
        };

        if !commit.is_empty() {
            if was_composing || comp_mgr.is_active() {
                comp_mgr.end_composition_with_text(context, tid, &commit);
            } else {
                comp_mgr.insert_text(context, tid, &commit);
            }
        }
    } else {
        commit_str_for_atf = None;
    }

    // ── preedit 처리 ──
    let preedit_str_for_atf: Option<String>;
    if result.preedit_changed {
        let preedit = engine.preedit_str().to_string();
        preedit_str_for_atf = Some(preedit.clone());

        if preedit.is_empty() {
            if comp_mgr.is_active() {
                comp_mgr.end_composition(context, tid);
            }
        } else if comp_mgr.is_active() {
            comp_mgr.update_composition(context, tid, &preedit);
        } else {
            comp_mgr.start_composition(context, tid, &preedit, comp_sink);
        }
    } else {
        preedit_str_for_atf = None;
    }

    // ── AutoTypeFix 오케스트레이션 ──
    // 팝업 활성 중에는 발동 금지 (엔진이 popup_key 처리 중)
    let popup_active = popup_win.as_ref().map(|w| w.is_active()).unwrap_or(false);
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
            comp_mgr.replace_surrounding(
                context,
                tid,
                apply.delete_chars,
                &apply.commit_text,
                &apply.replay_preedit,
                comp_sink,
            );
        }
    }

    result.consumed
}

/// engine 의 pending PopupAction 을 모두 소비해 popup_win 에 반영한다.
///
/// Show* 액션(ShowHanja/ShowSpecial/ShowEmoji) → `popup_win` 초기화 후 표시.
/// 이후 내비게이션 액션 → 기존 팝업 갱신.
/// HidePopup → 팝업 숨김 (선택 확정 또는 Esc).
///
/// # 갭 1 수정
/// 기존 구현은 `hanja_candidates_available || special_char_candidates_available` 플래그가
/// 켜진 경우에만 첫 take_popup_action() 을 호출했다. 이로 인해 Super+. 단축키로 트리거되는
/// ShowEmoji 는 이 플래그가 세워지지 않아 최초 show() 호출을 놓쳤다.
/// 수정: 플래그 체크를 제거하고 단일 drain 루프에서 Show* / 내비게이션 / HidePopup 을 모두 처리.
fn drain_popup_actions(
    engine: &mut InputEngine,
    popup_win: &mut Option<PopupWindow>,
    context: &ITfContext,
    tid: u32,
    _result: InputResult,
    _comp_mgr: &mut CompositionManager,
) {
    use unim::input_engine::PopupAction;

    loop {
        match engine.take_popup_action() {
            Some(action) => match &action {
                PopupAction::ShowHanja { .. }
                | PopupAction::ShowSpecial { .. }
                | PopupAction::ShowEmoji { .. } => {
                    // Show* 계열: 팝업 창을 (재)초기화 후 표시
                    let pos = get_composition_screen_pos(context, tid);
                    let win = popup_win.get_or_insert_with(|| {
                        PopupWindow::create().expect("PopupWindow 생성 실패")
                    });
                    win.show(action, pos);
                }
                _ => {
                    // PopupNavigate / PageJump / HanjaBookmarkChanged /
                    // HanjaCandidatesReordered / HidePopup 등 → 기존 팝업 갱신
                    if let Some(win) = popup_win.as_mut() {
                        win.update(action);
                    }
                }
            },
            None => break,
        }
    }
}
