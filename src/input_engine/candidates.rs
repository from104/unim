//! 한자 변환 + 특수문자 변환·선택·취소.

use super::engine::InputEngine;
use super::types::{InputResult, PopupAction};
use crate::popup::PopupState;
use crate::unim_log;

impl InputEngine {
    // =========================================
    // 한자 변환 관련 메서드
    // =========================================

    /// 한자 변환 모드를 시작합니다.
    ///
    /// 현재 preedit 또는 마지막 음절에서 한자 후보를 검색합니다.
    pub fn start_hanja_conversion(&mut self) -> InputResult {
        // 이미 한자 모드이면 무시
        if self.hanja_mode {
            return InputResult::consumed();
        }

        // 변환 대상 결정: preedit 우선, 없으면 마지막 커밋 음절
        let target = if !self.preedit_cache.is_empty() {
            // preedit의 마지막 음절
            self.preedit_cache.chars().last().map(|c| c.to_string())
        } else {
            // 커밋 버퍼의 마지막 음절 (이미 입력된 경우)
            None
        };

        if let Some(target_syllable) = target {
            let mut candidates = self.hanja_dict.search(&target_syllable);
            if !candidates.is_empty() {
                // 즐겨찾기 항목을 상단으로 정렬 (원본 상대 순서는 유지 — stable sort)
                let bookmarks = &self.hanja_bookmarks;
                candidates.sort_by_key(|e| !bookmarks.is_bookmarked(&target_syllable, &e.hanja));
                unim_log!(
                    "ENGINE",
                    "한자 후보 발견: '{}' -> {} 개",
                    target_syllable,
                    candidates.len()
                );
                self.hanja_target = target_syllable.clone();
                let hanja_pairs = candidates
                    .iter()
                    .map(|e| (e.hanja.clone(), e.meaning.clone()))
                    .collect::<Vec<_>>();
                let bookmark_flags: Vec<bool> = candidates
                    .iter()
                    .map(|e| {
                        self.hanja_bookmarks
                            .is_bookmarked(&target_syllable, &e.hanja)
                    })
                    .collect();
                self.hanja_candidates = candidates;
                self.hanja_mode = true;
                // expanded(9x9) 컬럼 라벨에 키맵별 top_row를 그대로 흘려보낸다 (special과 동일 source).
                let mut popup_state = PopupState::new_hanja_with_top_row(
                    &target_syllable,
                    hanja_pairs.clone(),
                    &self.top_row_labels,
                );
                popup_state.set_bookmark_flags(bookmark_flags);
                self.popup_state = Some(popup_state);
                // 팝업 액션 설정
                self.popup_pending_action = Some(PopupAction::ShowHanja {
                    target: target_syllable,
                    candidates: hanja_pairs,
                    top_row: self.top_row_labels.clone(),
                });
                return InputResult::hanja_candidates();
            }

            // 한자 후보 없음 → 초성이면 특수문자 검색 시도
            let ch = target_syllable.chars().next().unwrap_or('\0');
            if let Some(entry) = crate::special_chars::search_by_choseong(ch) {
                unim_log!(
                    "ENGINE",
                    "특수문자 후보 발견: '{}' ({}) -> {} 개",
                    ch,
                    entry.category,
                    entry.characters.len()
                );
                self.special_char_target = target_syllable;
                self.special_char_candidates = entry.characters.to_vec();
                self.special_char_mode = true;
                let chars: Vec<String> = self
                    .special_char_candidates
                    .iter()
                    .map(|c| c.to_string())
                    .collect();
                self.popup_state = Some(PopupState::new_special(
                    &self.special_char_target,
                    chars.clone(),
                    &self.top_row_labels,
                ));
                // 팝업 액션 설정
                self.popup_pending_action = Some(PopupAction::ShowSpecial {
                    target: self.special_char_target.clone(),
                    characters: chars,
                    top_row: self.top_row_labels.clone(),
                });
                return InputResult::special_char_candidates();
            }
        }

        unim_log!("ENGINE", "한자/특수문자 후보 없음");
        InputResult::consumed()
    }

    /// 현재 한자 모드 상태를 반환합니다.
    pub fn is_hanja_mode(&self) -> bool {
        self.hanja_mode
    }

    /// 현재 한자 후보 목록을 반환합니다.
    ///
    /// 각 항목은 (한자, 뜻풀이) 튜플입니다.
    pub fn get_hanja_candidates(&self) -> Vec<(String, String)> {
        self.hanja_candidates
            .iter()
            .map(|entry| (entry.hanja.clone(), entry.meaning.clone()))
            .collect()
    }

    /// 한자 변환 대상 문자열을 반환합니다.
    pub fn get_hanja_target(&self) -> &str {
        &self.hanja_target
    }

    /// 한자를 선택합니다.
    ///
    /// # 인자
    ///
    /// * `index` - 선택할 한자의 인덱스 (0부터 시작)
    ///
    /// # 반환
    ///
    /// 선택된 한자 문자열. 유효하지 않은 인덱스면 None.
    pub fn select_hanja(&mut self, index: usize) -> Option<String> {
        if !self.hanja_mode || index >= self.hanja_candidates.len() {
            return None;
        }

        let selected = &self.hanja_candidates[index];
        let hanja = selected.hanja.clone();

        unim_log!("ENGINE", "한자 선택: [{}] '{}'", index, hanja);

        // preedit에서 마지막 음절을 제거
        if !self.preedit_cache.is_empty() {
            self.korean_context.clear();
            self.preedit_cache.clear();
            // DBus 응답으로 한자를 반환하므로 commit_buffer에 추가하지 않음
            // (추가 시 다음 키 입력에 묻어나와 이중 커밋 발생)
        }

        self.cancel_hanja();
        Some(hanja)
    }

    /// 현재 한자 후보 목록의 즐겨찾기 상태를 반환합니다.
    ///
    /// 반환된 `Vec<bool>`은 `get_hanja_candidates()`와 동일한 순서로
    /// 각 후보의 즐겨찾기 여부를 나타냅니다.
    pub fn hanja_bookmark_states(&self) -> Vec<bool> {
        self.hanja_candidates
            .iter()
            .map(|e| {
                self.hanja_bookmarks
                    .is_bookmarked(&self.hanja_target, &e.hanja)
            })
            .collect()
    }

    /// 주어진 후보 인덱스의 즐겨찾기 상태를 토글합니다.
    ///
    /// 한자 모드가 아니거나 인덱스가 범위를 벗어나면 None을 반환합니다.
    /// 성공 시 `(new_index, 새 상태, 직전 상태)` 3-튜플을 반환한다 — 토글 직후
    /// 즐겨찾기 우선 정렬이 재적용되므로 토글된 한자의 인덱스가 바뀔 수 있다.
    /// 직전 상태(`was_bookmarked`)는 frontend가 ON→OFF 전환에 한해 시각 신호를
    /// 띄울 때 사용한다.
    ///
    /// 또한 [`PopupAction::HanjaCandidatesReordered`] 액션을 발행해 frontend가
    /// 후보 리스트·즐겨찾기·커서 위치를 한 번에 교체하도록 한다.
    pub fn toggle_hanja_bookmark(&mut self, index: usize) -> Option<(usize, bool, bool)> {
        if !self.hanja_mode || index >= self.hanja_candidates.len() {
            return None;
        }
        let hanja = self.hanja_candidates[index].hanja.clone();
        // 토글 직전 상태 — emit payload용. toggle() 호출 후의 새 상태와 함께 전달해
        // frontend가 ON→OFF 케이스만 골라 flash 등 시각 신호를 띄울 수 있게 한다.
        let was_state = self.hanja_bookmarks.is_bookmarked(&self.hanja_target, &hanja);
        let new_state = self.hanja_bookmarks.toggle(&self.hanja_target, &hanja);

        // (1) 후보 재정렬 — 자연 빈도순(dict.search)부터 다시 받아서 즐겨찾기 우선으로
        // stable sort 한다. 이전 로직(`self.hanja_candidates.sort_by_key(...)`)은
        // 해제(true→false) 시 모든 키가 동일해져 stable sort 가 입력 순서를 보존하므로
        // 방금 해제된 한자가 0번 자리에 그대로 머물렀다 — 자연 위치로 점프 못함.
        // dict 에서 빈도순을 매번 다시 받아 정렬 기반을 리셋한다.
        let target = self.hanja_target.clone();
        let mut fresh = self.hanja_dict.search(&target);
        let bookmarks_ref = &self.hanja_bookmarks;
        fresh.sort_by_key(|e| !bookmarks_ref.is_bookmarked(&target, &e.hanja));
        self.hanja_candidates = fresh;

        // (2) 토글된 한자의 새 위치 산출
        let new_index = self
            .hanja_candidates
            .iter()
            .position(|e| e.hanja == hanja)
            .unwrap_or(index);

        // (3) popup_state 일괄 갱신 — items/meanings/bookmarks/cursor
        let new_pairs: Vec<(String, String)> = self
            .hanja_candidates
            .iter()
            .map(|e| (e.hanja.clone(), e.meaning.clone()))
            .collect();
        let new_flags: Vec<bool> = self
            .hanja_candidates
            .iter()
            .map(|e| {
                self.hanja_bookmarks
                    .is_bookmarked(&self.hanja_target, &e.hanja)
            })
            .collect();

        let (page, sel_row, sel_col) = if let Some(state) = self.popup_state.as_mut() {
            let items: Vec<String> = new_pairs.iter().map(|(h, _)| h.clone()).collect();
            let meanings: Vec<String> = new_pairs.iter().map(|(_, m)| m.clone()).collect();
            state.replace_hanja_items(items, meanings, new_flags.clone());
            state.set_selected_global(new_index);
            (state.current_page(), state.sel_row(), state.sel_col())
        } else {
            (0, 0, 0)
        };

        // (4) PopupAction emit — 후보 + 즐겨찾기 + 커서 한 트랜잭션
        self.popup_pending_action = Some(PopupAction::HanjaCandidatesReordered {
            target: self.hanja_target.clone(),
            candidates: new_pairs,
            bookmarks: new_flags,
            new_cursor: new_index,
            page,
            sel_row,
            sel_col,
            bookmarked: new_state,
            was_bookmarked: was_state,
        });

        unim_log!(
            "ENGINE",
            "한자 즐겨찾기 토글+재정렬: target='{}', '{}' [{} -> {}] state={} (was={})",
            self.hanja_target,
            hanja,
            index,
            new_index,
            new_state,
            was_state
        );
        Some((new_index, new_state, was_state))
    }

    /// 한자 모드를 취소합니다.
    pub fn cancel_hanja(&mut self) {
        self.hanja_mode = false;
        self.hanja_candidates.clear();
        self.hanja_target.clear();
        self.popup_state = None;

        // preedit도 클리어 (한자 선택 후 원래 한글이 남지 않도록)
        self.korean_context.clear();
        self.preedit_cache.clear();
    }

    // =========================================
    // 특수문자 변환 관련 메서드
    // =========================================

    /// 현재 특수문자 모드 상태를 반환합니다.
    pub fn is_special_char_mode(&self) -> bool {
        self.special_char_mode
    }

    /// 현재 특수문자 후보 목록을 반환합니다.
    pub fn get_special_char_candidates(&self) -> &[char] {
        &self.special_char_candidates
    }

    /// 특수문자 변환 대상 문자열(초성)을 반환합니다.
    pub fn get_special_char_target(&self) -> &str {
        &self.special_char_target
    }

    /// 특수문자를 선택합니다.
    ///
    /// # 인자
    ///
    /// * `index` - 선택할 특수문자의 인덱스 (0부터 시작)
    ///
    /// # 반환
    ///
    /// 선택된 특수문자. 유효하지 않은 인덱스면 None.
    pub fn select_special_char(&mut self, index: usize) -> Option<char> {
        if !self.special_char_mode || index >= self.special_char_candidates.len() {
            return None;
        }

        let selected = self.special_char_candidates[index];
        unim_log!("ENGINE", "특수문자 선택: [{}] '{}'", index, selected);

        // preedit(초성)을 제거
        self.korean_context.clear();
        self.preedit_cache.clear();
        // DBus 응답으로 특수문자를 반환하므로 commit_buffer에 추가하지 않음
        // (추가 시 다음 키 입력에 묻어나와 이중 커밋 발생)

        self.cancel_special_char();
        Some(selected)
    }

    /// 특수문자 모드를 취소합니다.
    pub fn cancel_special_char(&mut self) {
        self.special_char_mode = false;
        self.special_char_candidates.clear();
        self.special_char_target.clear();
        self.popup_state = None;

        // preedit도 클리어
        self.korean_context.clear();
        self.preedit_cache.clear();
    }
}
