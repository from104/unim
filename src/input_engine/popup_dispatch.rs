//! `PopupAction` 펌프 + 팝업 키 처리.
//!
//! `take_popup_action`/`keycode_to_popup_key`/`process_popup_key`/`popup_select`/
//! `popup_cancel`/`popup_state(_mut)`을 모아 한자/특수문자/이모지 팝업의 디스패치를
//! 담당한다.

use super::engine::InputEngine;
use super::types::{InputResult, PopupAction};
use crate::config::Config;
use crate::keycode::{KeyCode, ModifierState};
use crate::popup::{PopupKey, PopupKeyResult, PopupState};
use crate::unim_log;

impl InputEngine {
    // =========================================
    // 팝업 키 핸들링
    // =========================================

    /// 처리 대기 중인 팝업 액션을 꺼냅니다.
    pub fn take_popup_action(&mut self) -> Option<PopupAction> {
        self.popup_pending_action.take()
    }

    /// KeyCode를 PopupKey로 변환합니다.
    pub(super) fn keycode_to_popup_key(keycode: KeyCode) -> PopupKey {
        match keycode {
            KeyCode::Num1 => PopupKey::Number(1),
            KeyCode::Num2 => PopupKey::Number(2),
            KeyCode::Num3 => PopupKey::Number(3),
            KeyCode::Num4 => PopupKey::Number(4),
            KeyCode::Num5 => PopupKey::Number(5),
            KeyCode::Num6 => PopupKey::Number(6),
            KeyCode::Num7 => PopupKey::Number(7),
            KeyCode::Num8 => PopupKey::Number(8),
            KeyCode::Num9 => PopupKey::Number(9),
            KeyCode::Enter => PopupKey::Enter,
            KeyCode::Escape => PopupKey::Escape,
            KeyCode::Up => PopupKey::Up,
            KeyCode::Down => PopupKey::Down,
            KeyCode::Left => PopupKey::Left,
            KeyCode::Right => PopupKey::Right,
            KeyCode::PageUp => PopupKey::PageUp,
            KeyCode::PageDown => PopupKey::PageDown,
            KeyCode::Tab => PopupKey::Tab,
            KeyCode::Space => PopupKey::Space,
            KeyCode::Backspace => PopupKey::Backspace,
            KeyCode::Period => PopupKey::Period,
            // 특수문자 팝업 열 점프: 물리 키 위치 기준 (레이아웃 무관)
            KeyCode::Q => PopupKey::Letter(0),
            KeyCode::W => PopupKey::Letter(1),
            KeyCode::E => PopupKey::Letter(2),
            KeyCode::R => PopupKey::Letter(3),
            KeyCode::T => PopupKey::Letter(4),
            KeyCode::Y => PopupKey::Letter(5),
            KeyCode::U => PopupKey::Letter(6),
            KeyCode::I => PopupKey::Letter(7),
            KeyCode::O => PopupKey::Letter(8),
            _ => PopupKey::Other,
        }
    }

    /// 팝업(한자/특수문자) 활성 상태에서 키를 처리합니다.
    pub(super) fn process_popup_key(
        &mut self,
        keycode: KeyCode,
        _modifier: ModifierState,
        config: &Config,
    ) -> InputResult {
        let popup_key = Self::keycode_to_popup_key(keycode);

        let result = if let Some(ref mut state) = self.popup_state {
            state.handle_key(popup_key)
        } else {
            PopupKeyResult::NotHandled
        };

        match result {
            PopupKeyResult::Select(abs_index) => self.popup_select(abs_index),

            PopupKeyResult::ToggleBookmark(abs_index) => {
                // toggle_hanja_bookmark 내부가 재정렬 + cursor 보정 + PopupAction
                // (HanjaCandidatesReordered) emit까지 처리한다.
                let _ = self.toggle_hanja_bookmark(abs_index);
                // 트리거 문자를 preedit으로 유지
                InputResult::preedit_updated()
            }

            PopupKeyResult::Cancel => {
                self.popup_cancel();
                InputResult::committed()
            }

            PopupKeyResult::Updated => {
                // PopupState 내부 상태가 변경됨 → PopupNavigate 액션 발행
                if let Some(ref state) = self.popup_state {
                    self.popup_pending_action = Some(PopupAction::PopupNavigate {
                        page: state.current_page(),
                        total_pages: state.total_pages(),
                        selected: state.sel_row(),
                        rows: state.rows(),
                        cols: state.cols(),
                        sel_row: state.sel_row(),
                        sel_col: state.sel_col(),
                    });
                }
                // preedit_updated: 트리거 문자를 preedit으로 유지
                InputResult::preedit_updated()
            }

            // preedit_updated: 트리거 문자를 preedit으로 유지
            PopupKeyResult::Consumed => InputResult::preedit_updated(),

            PopupKeyResult::NotHandled => {
                unim_log!("ENGINE", "팝업 미지원 키 {:?} → 팝업 닫고 재처리", keycode);
                self.popup_cancel();
                // 키를 다시 처리 (재귀 방지: popup 모드가 이미 해제됨)
                self.press_key(keycode, _modifier, config)
            }
        }
    }

    /// 팝업에서 항목 선택 처리
    pub(super) fn popup_select(&mut self, abs_index: usize) -> InputResult {
        if self.hanja_mode {
            if let Some(hanja) = self.select_hanja(abs_index) {
                unim_log!("ENGINE", "팝업 한자 선택: [{}] '{}'", abs_index, hanja);
                self.commit_buffer.push_str(&hanja);
                self.popup_pending_action = Some(PopupAction::HidePopup);
                return InputResult::committed();
            }
        } else if self.special_char_mode {
            if let Some(ch) = self.select_special_char(abs_index) {
                unim_log!("ENGINE", "팝업 특수문자 선택: [{}] '{}'", abs_index, ch);
                self.commit_buffer.push(ch);
                self.popup_pending_action = Some(PopupAction::HidePopup);
                return InputResult::committed();
            }
        }
        InputResult::consumed()
    }

    /// 팝업 취소 처리 — 원래 한글/초성을 그대로 커밋
    pub(super) fn popup_cancel(&mut self) {
        if self.hanja_mode {
            if !self.hanja_target.is_empty() {
                self.commit_buffer.push_str(&self.hanja_target);
            }
            self.cancel_hanja();
        } else if self.special_char_mode {
            if !self.special_char_target.is_empty() {
                self.commit_buffer.push_str(&self.special_char_target);
            }
            self.cancel_special_char();
        }
        self.popup_pending_action = Some(PopupAction::HidePopup);
    }

    /// 현재 팝업 상태에 대한 참조를 반환합니다.
    pub fn popup_state(&self) -> Option<&PopupState> {
        self.popup_state.as_ref()
    }

    /// 현재 팝업 상태에 대한 가변 참조를 반환합니다.
    pub fn popup_state_mut(&mut self) -> Option<&mut PopupState> {
        self.popup_state.as_mut()
    }
}
