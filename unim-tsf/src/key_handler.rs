//! 키 이벤트 처리 — VK → KeyCode → InputEngine

use windows::Win32::Foundation::*;
use windows::Win32::UI::Input::KeyboardAndMouse::*;
use windows::Win32::UI::TextServices::*;

use unim::config::{Config, InputCategory};
use unim::input_engine::InputEngine;
use unim::keycode::{KeyCode, ModifierState};

use crate::composition::CompositionManager;

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
) -> bool {
    let vk = wparam.0 as u16;
    let keycode = KeyCode::from_win32_vk(vk);
    let modifiers = get_modifier_state();

    // 수정자 키만 누른 경우 통과
    if keycode.is_modifier() {
        return false;
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

/// OnKeyDown: 실제 키 처리 + 조합 갱신
pub fn handle_key_down(
    engine: &mut InputEngine,
    config: &Config,
    comp_mgr: &mut CompositionManager,
    context: &ITfContext,
    tid: u32,
    wparam: WPARAM,
    comp_sink: &ITfCompositionSink,
) -> bool {
    let vk = wparam.0 as u16;
    let keycode = KeyCode::from_win32_vk(vk);
    let modifiers = get_modifier_state();

    let was_composing = engine.is_composing();
    let result = engine.press_key(keycode, modifiers, config);

    if !result.consumed && !result.commit_changed && !result.preedit_changed {
        return false;
    }

    // commit 처리
    if result.commit_changed {
        let commit = engine.commit_str().to_string();
        engine.clear_commit();

        if !commit.is_empty() {
            if was_composing || comp_mgr.is_active() {
                comp_mgr.end_composition_with_text(context, tid, &commit);
            } else {
                comp_mgr.insert_text(context, tid, &commit);
            }
        }
    }

    // preedit 처리
    if result.preedit_changed {
        let preedit = engine.preedit_str().to_string();

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

    result.consumed
}
