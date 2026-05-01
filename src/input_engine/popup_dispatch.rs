//! `PopupAction` 펌프 + 팝업 키 처리.
//!
//! `take_popup_action`/`keycode_to_popup_key`/`process_popup_key`/`popup_select`/
//! `popup_cancel`/`popup_state(_mut)`을 모아 한자/특수문자/이모지 팝업의 디스패치를
//! 담당한다.

use super::engine::InputEngine;
use super::types::{InputResult, PopupAction};
use crate::config::Config;
use crate::keycode::{KeyCode, ModifierState};
use crate::popup::{EmojiCatMeta, PopupKey, PopupKeyResult, PopupKind, PopupState};
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

    /// 팝업(한자/특수문자/이모지) 활성 상태에서 키를 처리합니다.
    pub(super) fn process_popup_key(
        &mut self,
        keycode: KeyCode,
        _modifier: ModifierState,
        config: &Config,
    ) -> InputResult {
        let popup_key = Self::keycode_to_popup_key(keycode);

        let (result, is_emoji_kind, prev_cat_index) = if let Some(ref mut state) = self.popup_state {
            let is_emoji = state.kind() == PopupKind::Emoji;
            let prev_cat = state.emoji_cat_index();
            (state.handle_key(popup_key), is_emoji, prev_cat)
        } else {
            (PopupKeyResult::NotHandled, false, 0)
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
                // PR #1: 이모지 팝업이고 카테고리가 바뀐 경우 emoji pool 교체.
                if is_emoji_kind {
                    let cur_cat = self
                        .popup_state
                        .as_ref()
                        .map(|s| s.emoji_cat_index())
                        .unwrap_or(prev_cat_index);
                    if cur_cat != prev_cat_index {
                        self.refresh_emoji_category_items(cur_cat);
                    }
                }
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
        } else if self.is_emoji_popup_active() {
            // PR #1: 이모지 commit + Recent MRU bump.
            if let Some(emoji) = self.emoji_at_global_index(abs_index) {
                unim_log!(
                    "ENGINE",
                    "팝업 이모지 선택: [{}] '{}'",
                    abs_index,
                    emoji
                );
                self.commit_buffer.push_str(&emoji);
                let _ = crate::emoji::touch_recent(&emoji);
                self.cancel_emoji_popup();
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
        } else if self.is_emoji_popup_active() {
            // 이모지 팝업은 trigger 가 modifier 조합이라 commit 할 원본 키가 없다 — 그냥 닫는다.
            self.cancel_emoji_popup();
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

    // =========================================
    // 이모지 팝업 (PR #1, OPTION X engine-driven)
    // =========================================

    /// 현재 이모지 팝업이 활성 상태인지.
    pub fn is_emoji_popup_active(&self) -> bool {
        self.popup_state
            .as_ref()
            .map(|s| s.kind() == PopupKind::Emoji)
            .unwrap_or(false)
    }

    /// 이모지 팝업 진입 — `popup_state` 에 `PopupKind::Emoji` 분기 생성.
    ///
    /// 시작 카테고리는 'Recent' (cat_index=0). Recent 가 비어있더라도 빈 페이지가
    /// 그려진다 (사용자 결정 #11 — 빈 셀은 그대로 둔다).
    pub(super) fn start_emoji_popup(&mut self) {
        // 카테고리 메타: Recent (runtime 동적) + data.rs 의 8 카테고리.
        let recent = crate::emoji::load_recent();
        let mut categories: Vec<EmojiCatMeta> = Vec::with_capacity(9);
        categories.push(EmojiCatMeta {
            id: "Recent".to_string(),
            label_ko: "최근 사용".to_string(),
            label_en: "Recent".to_string(),
            total: recent.len(),
        });
        for (id, ko, en, count) in crate::emoji::list_categories() {
            categories.push(EmojiCatMeta {
                id,
                label_ko: ko,
                label_en: en,
                total: count as usize,
            });
        }

        // 초기 카테고리: Recent (cat_index=0). items 는 recent 슬라이스 그대로.
        let cat_index: usize = 0;
        let total_in_cat = recent.len();
        let items: Vec<String> = recent.clone();

        let popup_state = PopupState::new_emoji(
            cat_index,
            items.clone(),
            total_in_cat,
            &self.top_row_labels,
            categories.clone(),
            recent.clone(),
        );
        self.popup_state = Some(popup_state);

        // PopupAction::ShowEmoji payload — DBus 가 시그널 payload 로 변환한다.
        let target_cat_id = categories
            .get(cat_index)
            .map(|c| c.id.clone())
            .unwrap_or_default();
        let cat_payload: Vec<(String, String, String, u32)> = categories
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
        self.popup_pending_action = Some(PopupAction::ShowEmoji {
            target_cat_id,
            items,
            top_row: self.top_row_labels.clone(),
            recent,
            categories: cat_payload,
        });
    }

    /// 이모지 팝업 종료 — popup_state 클리어. 호출자는 별도로 HidePopup 발행.
    pub(super) fn cancel_emoji_popup(&mut self) {
        self.popup_state = None;
    }

    /// 카테고리 전환 시 popup_state.items 갱신 (engine 이 emoji pool 보유).
    pub(super) fn refresh_emoji_category_items(&mut self, cat_index: usize) {
        let Some(state) = self.popup_state.as_ref() else {
            return;
        };
        if state.kind() != PopupKind::Emoji {
            return;
        }
        let cats = state.emoji_categories();
        let Some(cat) = cats.get(cat_index) else {
            return;
        };
        let cat_id = cat.id.clone();
        let (new_items, total_in_cat) = if cat_id == "Recent" {
            let recent = crate::emoji::load_recent();
            (recent.clone(), recent.len())
        } else {
            let pool = crate::emoji::category_emojis(&cat_id);
            let total = pool.len();
            (pool, total)
        };
        if let Some(state) = self.popup_state.as_mut() {
            state.replace_for_emoji_category(cat_index, new_items, total_in_cat);
        }
    }

    /// 현재 popup_state 의 cat_index 가 가리키는 카테고리 풀에서 abs_index 의 emoji 를 조회.
    ///
    /// abs_index 는 `popup_state.items` 의 카테고리 페이지 슬라이스 인덱스가 아니라
    /// **카테고리 전체 풀의 글로벌 인덱스** 다 (popup_state 가 페이지 슬라이스가 아닌
    /// 전체 카테고리 풀을 담고 있으므로 directly indexable).
    pub(super) fn emoji_at_global_index(&self, abs_index: usize) -> Option<String> {
        let state = self.popup_state.as_ref()?;
        if state.kind() != PopupKind::Emoji {
            return None;
        }
        state.get_item(abs_index).map(|s| s.to_string())
    }
}
