//! Content purpose + surrounding text + smart_backspace + typefix_convert.

use super::engine::InputEngine;
use super::types::PopupAction;
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
            // 한자 단어 게이트 — 반드시 이 분기의 첫 줄(아래 flush_preedit 앞, HANJA_WORD_SPEC
            // §2.8). 한자 팝업은 preedit 을 유지한 채 열리므로 정리가 flush 뒤에 오면 "국" 이
            // commit_buffer 로 흘러 다음 키 처리에서 비번 필드로 나간다.
            //   - 최근 확정 음절 버퍼 비움
            //   - 목적 통지보다 먼저 온 surrounding(선택 스냅샷) 잔류 제거
            //   - 팝업이 열려 있으면 재커밋 없이 닫는다(비번 필드에 원문 재삽입 금지)
            self.recent_clear();
            self.surrounding_text.clear();
            self.surrounding_cursor = 0;
            self.surrounding_anchor = 0;
            let popup_open = if self.hanja_mode {
                self.cancel_hanja();
                true
            } else if self.special_char_mode {
                self.cancel_special_char();
                true
            } else if self.is_emoji_popup_active() {
                self.cancel_emoji_popup();
                true
            } else {
                false
            };
            if popup_open {
                self.popup_pending_action = Some(PopupAction::HidePopup);
            }

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

    /// 비밀번호/PIN 진입 직전에 저장해 둔 입력 카테고리(필드를 벗어나면 복구될 값).
    ///
    /// 엔진을 재생성하는 호출부가 목적과 함께 이관할 때 쓴다 — 저장값을 잃으면
    /// 비밀번호 칸을 벗어난 뒤 영문 강제가 풀리지 않는다.
    pub fn saved_category(&self) -> Option<InputCategory> {
        self.saved_category
    }

    /// Surrounding text를 설정합니다.
    ///
    /// 비밀번호/PIN 필드(`should_block_hangul`)에서는 surrounding text(평문일 수
    /// 있음)를 엔진 메모리에 보관하지 않는다. 빈 값으로 덮어 **기존 잔류까지 제거**하며,
    /// 이로써 이를 소비하는 `GlobalTypeFix`/`smart_backspace`/`typefix_convert` 경로가
    /// 자연히 무력화된다. 필드를 벗어나면(비-비밀번호 목적) 이후 호출부터 정상 저장이
    /// 재개된다(fail-closed — 이탈 신호가 늦어도 비번 평문은 잔류하지 않는다).
    ///
    /// 호출 사실 자체는 `surrounding_seen` 으로 기억한다(빈 값·비밀번호 필드 포함) — 이
    /// 컨텍스트에서는 이후 빈 surrounding 이 한자 대상①의 접두 검증 실패가 된다
    /// (HANJA_WORD_SPEC Q9(b), Wayland 미수신 마커 `("",0,0)`).
    pub fn set_surrounding_text(&mut self, text: String, cursor_pos: u32, anchor_pos: u32) {
        self.surrounding_seen = true;
        // 보고 수신 = 신선(차단 여부와 무관). 대상③ 소진·선택 불명 표시도 새 보고로 해제(I3).
        self.surrounding_stale = false;
        self.surrounding_word_exhausted = false;
        self.selection_unknown_once = false;
        if self.content_purpose.should_block_hangul() {
            self.surrounding_text.clear();
            self.surrounding_cursor = 0;
            self.surrounding_anchor = 0;
            self.learn_obs = None;
            return;
        }
        self.observe_preedit_inclusion(&text, cursor_pos, anchor_pos);
        self.surrounding_text = text;
        self.surrounding_cursor = cursor_pos;
        self.surrounding_anchor = anchor_pos;
    }

    /// surrounding 이 preedit 을 포함하는 앱(LibreOffice VCL 등)을 **관측으로 학습**한다
    /// (HANJA_WORD_SPEC §2.2.3). 저장값은 건드리지 않고 비교만 한다.
    ///
    /// 증거 = cursor==anchor 인 조합 중 보고 두 건이, 중간 확정 없이(`recent_*` 변경 시
    /// `learn_obs` 해제), preedit 끝 글자가 서로 다르고(P1≠P2 끝 글자), 각각 `before` 가
    /// 그 preedit 으로 끝나며, preedit 을 뺀 앞부분과 그 길이가 같은 경우. preedit 미포함
    /// 앱은 preedit 이 바뀌어도 `before` 가 변하지 않으므로 오탐할 수 없다. 모아치기처럼
    /// 끝 글자가 같은 변화는 증거가 아니다. 증거가 아닌 보고는 관측을 비운다(fail-safe).
    ///
    /// 학습 모드에서 조합 중이 아닌 보고의 `before` 가 최근 확정 버퍼로 끝나지 않으면
    /// 버퍼를 비운다 — 늦게 도착한 보고의 드리프트로 엉뚱한 글자를 지우는 것을 막는다
    /// (실패 방향 = 종전 1자, 안전).
    fn observe_preedit_inclusion(&mut self, text: &str, cursor_pos: u32, anchor_pos: u32) {
        // 모아치기 대기 중 자모는 preedit 캐시에만 있을 수 있어 둘 다 본다(idle 오판 방지).
        let composing = self.korean_context.is_composing() || !self.preedit_str().is_empty();
        let cursor = cursor_pos as usize;
        let before: String = text.chars().take(cursor).collect();
        let before_len = before.chars().count();
        // 커서 단위 불일치(오프셋 초과)·선택 영역은 증거가 아니다.
        if cursor_pos != anchor_pos || before_len != cursor {
            self.learn_obs = None;
            return;
        }
        if !composing {
            self.learn_obs = None;
            if self.learned_includes_preedit
                && !self.recent_syllables.is_empty()
                && !before.ends_with(self.recent_syllables.as_str())
            {
                self.recent_clear();
            }
            return;
        }
        let pre = self.preedit_str();
        let Some(tail) = pre.chars().last() else {
            self.learn_obs = None;
            return;
        };
        let pre_len = pre.chars().count();
        if !before.ends_with(pre) {
            self.learn_obs = None; // 미포함 보고(또는 늦은 보고) — 증거 아님
            return;
        }
        let head: String = before.chars().take(before_len - pre_len).collect();
        if let Some(prev) = &self.learn_obs {
            if prev.tail != tail && prev.head == head {
                self.learned_includes_preedit = true;
            }
        }
        self.learn_obs = Some(super::engine::PreeditLearnObs { head, tail });
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
    ///
    /// **미배선(실험적)** — 현재 호출자는 테스트뿐. 전 배선은 v0.4.x 이후 (FUNC-LINUX-05).
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

        // 선택 영역이 없거나 오프셋이 문자 길이를 넘으면(단위 불일치) 변환하지 않음 —
        // 클램프하면 실제 선택과 다른 구간을 지운다(HANJA_WORD_SPEC §2.4.1).
        let (start, end) = self.selection_span()?;
        let chars: Vec<char> = self.surrounding_text.chars().collect();
        let cursor = self.surrounding_cursor as usize;
        let word: String = chars[start..end].iter().collect();
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

#[cfg(test)]
mod tests {
    use super::super::test_helpers::create_test_engine;
    use crate::config::ContentPurpose;

    /// 비밀번호 필드에서는 surrounding text 를 저장하지 않고(빈 값), 기존 잔류도 즉시
    /// 제거한다 — 비번 평문이 엔진 메모리에 남아 GlobalTypeFix/SmartBackspace 로 새어
    /// 나가지 않게 한다.
    #[test]
    fn password_field_ignores_and_clears_surrounding_text() {
        let mut engine = create_test_engine();

        // 일반 필드에서 저장된 기존 surrounding.
        engine.set_surrounding_text("secret".to_string(), 6, 0);
        assert_eq!(engine.surrounding_text().0, "secret");

        // 비밀번호 진입: 저장 시도 무시 + 기존 잔류 제거.
        engine.set_content_purpose(ContentPurpose::Password);
        engine.set_surrounding_text("dkssud".to_string(), 6, 0);
        let (text, cursor, anchor) = engine.surrounding_text();
        assert!(text.is_empty(), "비번 필드 surrounding text 는 저장되지 않아야 함");
        assert_eq!(cursor, 0);
        assert_eq!(anchor, 0);
    }

    /// PIN 필드도 동일하게 차단한다(`should_block_hangul` 계열 전체).
    #[test]
    fn pin_field_ignores_surrounding_text() {
        let mut engine = create_test_engine();
        engine.set_content_purpose(ContentPurpose::Pin);
        engine.set_surrounding_text("1234".to_string(), 4, 0);
        assert!(engine.surrounding_text().0.is_empty());
    }

    /// 필드를 벗어나면(Normal) 이후 호출부터 정상 저장이 재개된다.
    #[test]
    fn normal_field_after_password_stores_surrounding_text() {
        let mut engine = create_test_engine();
        engine.set_content_purpose(ContentPurpose::Password);
        engine.set_surrounding_text("dkssud".to_string(), 6, 0);
        assert!(engine.surrounding_text().0.is_empty());

        // 필드 벗어남 → 정상 저장 재개.
        engine.set_content_purpose(ContentPurpose::Normal);
        engine.set_surrounding_text("hello".to_string(), 5, 0);
        let (text, cursor, anchor) = engine.surrounding_text();
        assert_eq!(text, "hello");
        assert_eq!(cursor, 5);
        assert_eq!(anchor, 0);
    }
}
