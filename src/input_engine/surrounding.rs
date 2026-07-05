//! Content purpose + surrounding text + smart_backspace + typefix_convert.

use super::engine::InputEngine;
use crate::config::{ContentPurpose, InputCategory};
use crate::unim_log;

impl InputEngine {
    // =========================================
    // Content Type / Surrounding Text
    // =========================================

    /// 입력 필드의 목적을 설정합니다.
    ///
    /// 비밀번호/PIN 필드에 진입하면 직전 입력 카테고리를 저장하고 영문으로 강제
    /// 전환하며, 필드를 벗어나면(비-비밀번호 목적으로 전환) 저장한 직전 카테고리를
    /// 복구한다. 이로써 "비밀번호 칸 임시 영문 → 벗어나면 직전 한/영 복구"가 양
    /// 플랫폼(Linux immodule focus_in / Windows TSF InputScope)에서 동작한다.
    ///
    /// 상태머신(멱등 — 동일 목적 반복 호출은 무시):
    /// - 비밀번호/PIN 진입(`should_block_hangul`) && 저장 없음: 현재 카테고리를
    ///   저장한 뒤 영문 강제. 이미 저장돼 있으면(Password→Pin 연쇄) 재저장하지 않아
    ///   최초 진입 직전 상태를 보존한다.
    /// - 비-비밀번호 진입 && 저장 있음: 저장값을 복구한 뒤 클리어. 이미 영문이었으면
    ///   복구도 영문이라 변화가 없다(자연).
    pub fn set_content_purpose(&mut self, purpose: ContentPurpose) {
        // 멱등: 동일 목적 반복 호출은 무시 (이중 저장/복구 방지, 평소 경로 무회귀).
        if self.content_purpose == purpose {
            return;
        }
        unim_log!(
            "ENGINE",
            "content_purpose 변경: {:?} -> {:?}",
            self.content_purpose,
            purpose
        );
        self.content_purpose = purpose;

        if purpose.should_block_hangul() {
            // 비밀번호/PIN 진입: 직전 카테고리 저장(최초 1회) + 영문 강제.
            if self.saved_category.is_none() {
                self.saved_category = Some(self.input_category);
            }
            if self.input_category != InputCategory::English {
                self.flush_preedit();
                self.input_category = InputCategory::English;
                self.update_status_file();
            }
        } else if let Some(saved) = self.saved_category.take() {
            // 비-비밀번호 진입(필드 벗어남): 저장한 직전 카테고리 복구 + 클리어.
            if self.input_category != saved {
                self.flush_preedit();
                self.input_category = saved;
                self.update_status_file();
            }
        }
    }

    /// 현재 content purpose를 반환합니다.
    pub fn content_purpose(&self) -> ContentPurpose {
        self.content_purpose
    }

    /// Surrounding text를 설정합니다.
    pub fn set_surrounding_text(&mut self, text: String, cursor_pos: u32, anchor_pos: u32) {
        self.surrounding_text = text;
        self.surrounding_cursor = cursor_pos;
        self.surrounding_anchor = anchor_pos;
    }

    /// Surrounding text를 반환합니다.
    pub fn surrounding_text(&self) -> (&str, u32, u32) {
        (
            &self.surrounding_text,
            self.surrounding_cursor,
            self.surrounding_anchor,
        )
    }

    // =========================================
    // Smart Backspace (자모 단위 삭제)
    // =========================================

    /// Smart Backspace: 커밋된 한글 글자를 자모 단위로 삭제합니다.
    ///
    /// 조합 중이 아닌 상태에서 백스페이스를 누르면, surrounding text의 커서 앞
    /// 한글 글자를 분해하여 마지막 자모를 제거하고 재조합합니다.
    ///
    /// # Returns
    /// * `Some((1, replacement))` - 1글자 삭제 후 대체 텍스트
    /// * `None` - surrounding text가 없거나 한글이 아님
    pub fn smart_backspace(&self) -> Option<(u32, String)> {
        if self.surrounding_text.is_empty() || self.surrounding_cursor == 0 {
            return None;
        }

        // 커서 앞의 마지막 문자를 가져옴
        let text_before: String = self
            .surrounding_text
            .chars()
            .take(self.surrounding_cursor as usize)
            .collect();

        let last_char = text_before.chars().last()?;

        // 한글 음절인지 확인 (U+AC00 ~ U+D7A3)
        if !('\u{AC00}'..='\u{D7A3}').contains(&last_char) {
            return None;
        }

        // 음절을 분해
        use crate::hangul::char::HangulChar;
        let hchar = HangulChar::from_syllable(last_char).ok()?;

        let cho = hchar.get_cho();
        let jung = hchar.get_jung();
        let jong = hchar.get_jong();

        use crate::hangul::jamo::Jong;

        // 종성이 있으면 종성을 제거
        if let Some(j) = jong {
            if j != Jong::E {
                // 종성을 제거하고 초+중으로 재조합
                let new_char = HangulChar::from_jamo_sequences(
                    cho.unwrap() as i32,
                    jung.unwrap() as i32,
                    Jong::E as i32,
                );
                return Some((1, new_char.to_string()));
            }
        }

        // 중성이 있으면 중성을 제거 → 초성만 남음
        if jung.is_some() {
            if let Some(c) = cho {
                // 초성만으로 HangulChar 생성
                let result = HangulChar::from_jamo_sequences(
                    c as i32, -1, // 중성 없음
                    -1, // 종성 없음
                );
                return Some((1, result.to_string()));
            }
        }

        // 초성만 있으면 글자 전체 삭제 (빈 문자열 반환)
        Some((1, String::new()))
    }

    // =========================================
    // TypeFix (한/영 오타 변환)
    // =========================================

    /// TypeFix 변환을 수행합니다.
    ///
    /// 선택된 텍스트(cursor != anchor)만 변환합니다.
    /// 선택 영역이 없으면 변환하지 않습니다.
    ///
    /// 변환 후 결과 언어에 맞게 입력 모드를 자동 전환합니다.
    ///
    /// # Arguments
    /// * `direction` - 0: 자동 감지, 1: 영→한, 2: 한→영
    ///
    /// # Returns
    /// * `Some((offset_from_cursor, delete_chars, replacement))` - 변환 성공
    ///   - `offset_from_cursor`: 커서로부터의 오프셋 (음수=앞, 0=현재/뒤)
    ///   - `delete_chars`: 삭제할 문자 수
    ///   - `replacement`: 대체 텍스트
    /// * `None` - 선택 영역이 없거나 surrounding text가 없음
    pub fn typefix_convert(&mut self, direction: u32) -> Option<(i32, u32, String)> {
        if self.surrounding_text.is_empty() {
            return None;
        }

        let chars: Vec<char> = self.surrounding_text.chars().collect();
        let cursor = self.surrounding_cursor as usize;
        let anchor = self.surrounding_anchor as usize;

        // 선택 영역이 없으면 변환하지 않음
        if cursor == anchor {
            return None;
        }

        let start = cursor.min(anchor);
        let end = cursor.max(anchor);
        let word: String = chars[start..end.min(chars.len())].iter().collect();
        let delete_chars = word.chars().count() as u32;

        // 커서로부터의 오프셋 계산: 음수=커서 앞, 0=커서 뒤
        let offset_from_cursor = (start as i32) - (cursor as i32);

        if word.is_empty() {
            return None;
        }

        let korean_layout = self.korean_layout.as_str();
        let english_layout = self.english_layout.as_str();

        // 자동 감지 + 변환
        let (replacement, target_mode) = match direction {
            1 => {
                // 영→한 강제
                let converted = crate::typefix::eng_to_kor(&word, korean_layout, english_layout);
                if converted != word {
                    (Some(converted), Some(InputCategory::Korean))
                } else {
                    (None, None)
                }
            }
            2 => {
                // 한→영 강제
                let converted = crate::typefix::kor_to_eng(&word, korean_layout, english_layout);
                if converted != word {
                    (Some(converted), Some(InputCategory::English))
                } else {
                    (None, None)
                }
            }
            _ => {
                // 자동 감지: 한글이면 한→영, 영문이면 영→한
                if crate::typefix::is_korean_text(&word) {
                    let converted =
                        crate::typefix::kor_to_eng(&word, korean_layout, english_layout);
                    if converted != word {
                        (Some(converted), Some(InputCategory::English))
                    } else {
                        (None, None)
                    }
                } else {
                    let converted =
                        crate::typefix::eng_to_kor(&word, korean_layout, english_layout);
                    if converted != word {
                        (Some(converted), Some(InputCategory::Korean))
                    } else {
                        (None, None)
                    }
                }
            }
        };

        if let Some(ref repl) = replacement {
            // 변환 결과 언어로 입력 모드 자동 전환
            if let Some(mode) = target_mode {
                if self.input_category != mode {
                    unim_log!(
                        "ENGINE",
                        "TypeFix 모드 전환: {:?} → {:?}",
                        self.input_category,
                        mode
                    );
                    self.input_category = mode;
                    self.update_status_file();
                }
            }
            Some((offset_from_cursor, delete_chars, repl.clone()))
        } else {
            None
        }
    }
}
