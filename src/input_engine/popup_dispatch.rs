//! `PopupAction` 펌프 + 팝업 키 처리.
//!
//! `take_popup_action`/`keycode_to_popup_key`/`process_popup_key`/`popup_select`/
//! `popup_cancel`/`popup_state(_mut)`을 모아 한자/특수문자/이모지 팝업의 디스패치를
//! 담당한다.

use super::engine::InputEngine;
use super::types::{InputResult, PageDirection, PopupAction};
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
        modifier: ModifierState,
        config: &Config,
    ) -> InputResult {
        // Shift+Tab 은 별도 keysym(ISO_Left_Tab) 또는 evdev Tab+Shift 양쪽으로 들어올 수
        // 있다. evdev 기반 매핑(Tab) + modifier.shift 조합을 PopupKey::ShiftTab 으로 흡수.
        let popup_key = match keycode {
            KeyCode::Tab if modifier.shift => PopupKey::ShiftTab,
            _ => Self::keycode_to_popup_key(keycode),
        };

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
                // 이모지 팝업이고 카테고리가 바뀐 경우 emoji pool 교체 + ShowEmoji 재발행.
                // PopupNavigate 만으로는 GNOME extension 의 emoji popup 이 카테고리
                // 라벨/탭/아이콘풀을 재구성하지 못한다 (updateFromNavigate 는 페이지/그리드만
                // 갱신). 마우스 탭 클릭(SetEmojiCategory RPC)이 ShowEmojiPopupV2 를 재발행해
                // onShowEmoji 콜백으로 popup 전체를 다시 그리는 흐름과 동일하게 맞춘다.
                if is_emoji_kind {
                    let cur_cat = self
                        .popup_state
                        .as_ref()
                        .map(|s| s.emoji_cat_index())
                        .unwrap_or(prev_cat_index);
                    if cur_cat != prev_cat_index {
                        self.refresh_emoji_category_items(cur_cat);
                        if let Some(state) = self.popup_state.as_ref() {
                            if state.kind() == PopupKind::Emoji {
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
                                    .get(cur_cat)
                                    .map(|c| c.0.clone())
                                    .unwrap_or_default();
                                let items: Vec<String> =
                                    state.emoji_items().iter().cloned().collect();
                                let recent: Vec<String> =
                                    state.emoji_recent().iter().cloned().collect();
                                self.popup_pending_action = Some(PopupAction::ShowEmoji {
                                    target_cat_id,
                                    items,
                                    top_row: self.top_row_labels.clone(),
                                    recent,
                                    categories: cats,
                                });
                                return InputResult::preedit_updated();
                            }
                        }
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
                self.press_key(keycode, modifier, config)
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

    /// 마우스 ◀/▶ 버튼 등 외부 RPC에서 호출되는 페이지 이동.
    ///
    /// `PopupKey::PageUp/PageDown` 분기는 sel_row/sel_col 을 (0,0) 으로 리셋하므로
    /// 그 경로를 사용하지 않고 직접 `current_page` 를 wrap-around 갱신한 뒤
    /// `set_navigate_state` 로 prev cursor (row,col) 위치를 그대로 복원한다.
    ///
    /// total_pages <= 1 이거나 popup_state 가 없으면 `None` 반환 (no-op).
    /// 정상 처리되면 새 페이지 인덱스를 반환하고 `PopupAction::PageJump` 를
    /// pending 액션으로 설정한다 — DBus 측은 이를 받아 `PopupNavigate` 시그널을
    /// 발행한다.
    pub fn popup_change_page(&mut self, dir: PageDirection) -> Option<u32> {
        let state = self.popup_state.as_mut()?;
        let total = state.total_pages();
        if total <= 1 {
            return None;
        }
        let cur = state.current_page();
        let prev_row = state.sel_row();
        let prev_col = state.sel_col();
        let new_page = match dir {
            PageDirection::Next => (cur + 1) % total,
            PageDirection::Prev => {
                if cur == 0 {
                    total - 1
                } else {
                    cur - 1
                }
            }
        };
        // set_navigate_state 가 page 변경 + layout 재계산 + cursor 보정(min 적용)을 처리.
        state.set_navigate_state(new_page, prev_row, prev_col);

        self.popup_pending_action = Some(PopupAction::PageJump {
            page_index: new_page as u32,
        });
        Some(new_page as u32)
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
    /// 시작 카테고리: Recent 가 비어있지 않으면 'Recent' (cat_index=0).
    /// Recent 가 비어있으면 두 번째 탭(SmileysPeople, cat_index=1)으로 시작 —
    /// 빈 그리드 첫인상을 피하기 위해.
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

        // 초기 카테고리: Recent 가 비어있으면 cat_index=1 (두 번째 탭) 로 시작.
        // 사용자 첫인상에 빈 그리드가 보이지 않도록.
        let (cat_index, total_in_cat, items): (usize, usize, Vec<String>) = if recent.is_empty()
            && categories.len() > 1
        {
            let cat_id = categories[1].id.clone();
            let pool = crate::emoji::category_emojis(&cat_id);
            let total = pool.len();
            (1, total, pool)
        } else {
            (0, recent.len(), recent.clone())
        };

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
    ///
    /// GNOME extension 의 마우스 클릭 탭 전환 등 외부 RPC 도 사용한다 — 이 때문에
    /// 가시성을 `pub` 으로 둔다. `cat_index` 가 범위 밖이면 무시된다.
    pub fn refresh_emoji_category_items(&mut self, cat_index: usize) {
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
