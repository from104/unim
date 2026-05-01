//! Press 계열 키 처리 hot path.
//!
//! `press_key_code`/`press_key`/`process_korean_key`/`process_english_key`/
//! `parse_trigger_key`/`match_auto_english_trigger` + preedit/상태 helper들을 한 파일에 묶어
//! 인라인 가능성을 보존한다.

use super::engine::InputEngine;
use super::types::{AutoEnglishTrigger, InputResult};
use crate::config::{Config, InputCategory};
use crate::hangul::jamo::JamoEnum;
use crate::keycode::{KeyCode, ModifierState};
use crate::unim_log;

impl InputEngine {
    /// 키 코드를 처리합니다.
    ///
    /// # Arguments
    ///
    /// * `hardware_code` - 하드웨어 키코드 (evdev)
    /// * `modifier` - 수정자 키 상태
    /// * `config` - 현재 설정
    ///
    /// # Returns
    ///
    /// 입력 처리 결과
    pub fn press_key_code(
        &mut self,
        hardware_code: u16,
        modifier: ModifierState,
        config: &Config,
    ) -> InputResult {
        let keycode = KeyCode::from_evdev_keycode(hardware_code);
        self.press_key(keycode, modifier, config)
    }

    /// KeyCode를 처리합니다.
    ///
    /// # Arguments
    ///
    /// * `keycode` - 변환된 키코드
    /// * `modifier` - 수정자 키 상태
    /// * `config` - 현재 설정
    ///
    /// # Returns
    ///
    /// 입력 처리 결과
    pub fn press_key(
        &mut self,
        keycode: KeyCode,
        modifier: ModifierState,
        _config: &Config,
    ) -> InputResult {
        // 수정자 키만 누른 경우 무시
        if keycode.is_modifier() {
            return InputResult::not_consumed();
        }

        // 팝업(한자/특수문자/이모지) 활성 상태에서 키 인터셉트.
        // PR #1: Emoji 팝업도 이 분기로 들어오도록 popup_state 진입.
        if self.hanja_mode || self.special_char_mode || self.is_emoji_popup_active() {
            return self.process_popup_key(keycode, modifier, _config);
        }

        // 이모지 팝업 트리거 (Super+. 등) — 단축키 early return 이전에 체크.
        // PR #1 (R1/R2 안전망):
        //   R1: 이미 emoji popup 이 떠 있으면 위 분기에서 처리 (재트리거 가드 자동).
        //   R2: 한자/특수 모드도 위 분기에서 흡수 — emoji 트리거가 그쪽 모드를 깨지 않는다.
        if self.matches_emoji_trigger(keycode, modifier) {
            unim_log!("ENGINE", "이모지 팝업 트리거 감지: {:?}", keycode);
            // 조합 중이면 먼저 커밋.
            let was_composing = self.korean_context.is_composing();
            if was_composing {
                self.flush_preedit();
            }
            // PR #1: PopupState::Emoji 진입 + 카테고리/페이지/Recent payload 구성.
            self.start_emoji_popup();
            if was_composing {
                return InputResult::committed();
            }
            return InputResult::consumed();
        }

        // Control/Alt가 눌린 경우 (단축키) 무시
        if modifier.control || modifier.alt || modifier.super_key {
            // 조합 중이면 먼저 커밋
            if self.korean_context.is_composing() {
                self.flush_preedit();
                return InputResult::committed();
            }
            return InputResult::not_consumed();
        }

        // 한/영 전환 처리 (설정 기반)
        if self.toggle_keys.contains(&keycode) {
            // 비밀번호 필드에서는 한/영 전환 차단
            if self.content_purpose.should_block_hangul()
                && self.input_category == InputCategory::English
            {
                unim_log!(
                    "ENGINE",
                    "한/영 전환 차단: content_purpose={:?}",
                    self.content_purpose
                );
                return InputResult::consumed();
            }

            // 조합 중이면 먼저 커밋
            let was_composing = self.korean_context.is_composing();
            self.toggle_input_category();

            // 조합 중이었으면 commit이 발생했으므로 committed() 반환
            if was_composing {
                return InputResult::committed();
            }
            return InputResult::consumed();
        }

        // 비밀번호/PIN 필드에서는 한글 모드를 영어로 강제 전환
        if self.content_purpose.should_block_hangul()
            && self.input_category == InputCategory::Korean
        {
            unim_log!("ENGINE", "비밀번호 필드 감지: 영문 모드로 강제 전환");
            self.set_input_category(InputCategory::English);
        }

        // 입력 카테고리에 따른 처리
        match self.input_category {
            InputCategory::Korean => self.process_korean_key(keycode, modifier),
            InputCategory::English => self.process_english_key(keycode, modifier),
        }
    }

    /// 한국어 키 입력을 처리합니다.
    pub(super) fn process_korean_key(
        &mut self,
        keycode: KeyCode,
        modifier: ModifierState,
    ) -> InputResult {
        unim_log!(
            "ENGINE",
            "process_korean_key: keycode={:?}, shift={}, caps={}",
            keycode,
            modifier.shift,
            modifier.caps_lock
        );

        // Hanja 키 처리 - 한자 변환 모드 시작
        if keycode == KeyCode::Hanja {
            return self.start_hanja_conversion();
        }

        // 자동 영문 전환 (opt-in): 지정 트리거 키 입력 시 조합 커밋 + 영문 모드로 영구 전환.
        // 제어 키(Escape/Tab/Enter 등)는 passthrough, 문자 키(`/`/`:` 등)는 해당 문자 commit.
        if let Some(trigger) = self.match_auto_english_trigger(keycode, modifier) {
            let was_composing = self.korean_context.is_composing();
            if was_composing {
                self.flush_preedit();
            }
            self.set_input_category(InputCategory::English);

            let produced = match trigger {
                // Functional: 종전대로 QWERTY 영문 char 산출 (대부분 None — passthrough).
                AutoEnglishTrigger::Functional { .. } => {
                    if modifier.shift {
                        keycode.to_shifted_char()
                    } else {
                        keycode.to_char()
                    }
                }
                // Character: 트리거 등록 char 그대로 commit (비-QWERTY 한국어 안전).
                // 매칭 단계에서 이미 `produces_char_in_korean(...) == Some(ch)` 가 보장됐다.
                AutoEnglishTrigger::Character(ch) => Some(ch),
            };

            if let Some(c) = produced {
                self.commit_buffer.push(c);
                unim_log!(
                    "ENGINE",
                    "auto-english: '{:?}' -> 영문 전환 + commit '{}'",
                    keycode,
                    c
                );
                return InputResult::committed();
            }

            unim_log!(
                "ENGINE",
                "auto-english: '{:?}' -> 영문 전환 + passthrough (composing={})",
                keycode,
                was_composing
            );
            return if was_composing {
                InputResult::committed_passthrough()
            } else {
                InputResult::not_consumed()
            };
        }

        // Backspace 처리
        if keycode == KeyCode::Backspace {
            if self.korean_context.is_composing() {
                self.korean_context.backspace();
                self.update_preedit_cache();
                unim_log!("ENGINE", "Backspace -> preedit='{}'", self.preedit_cache);
                return InputResult::preedit_updated();
            }
            return InputResult::not_consumed();
        }

        // Enter 처리 - 조합 커밋 후 키 통과
        if keycode == KeyCode::Enter {
            if self.korean_context.is_composing() {
                self.flush_preedit();
                unim_log!("ENGINE", "Enter -> 조합 커밋 후 키 통과");
                return InputResult::committed_passthrough();
            }
            return InputResult::not_consumed();
        }

        // Tab 처리 - 조합 커밋 후 키 통과
        if keycode == KeyCode::Tab {
            if self.korean_context.is_composing() {
                self.flush_preedit();
                unim_log!("ENGINE", "Tab -> 조합 커밋 후 키 통과");
                return InputResult::committed_passthrough();
            }
            return InputResult::not_consumed();
        }

        // Escape 처리 - 조합 커밋 후 키 통과
        if keycode == KeyCode::Escape {
            if self.korean_context.is_composing() {
                self.flush_preedit();
                unim_log!("ENGINE", "Escape -> 조합 커밋 후 키 통과");
                return InputResult::committed_passthrough();
            }
            return InputResult::not_consumed();
        }

        // Space 처리
        if keycode == KeyCode::Space {
            if self.korean_context.is_composing() {
                self.flush_preedit();
            }
            self.commit_buffer.push(' ');
            return InputResult::committed();
        }

        // 문자 키 처리
        // 한국어 모드에서는 CapsLock을 무시하고 Shift만 적용 (쌍자음 입력용)
        // JSON 키맵 기반으로 레이아웃에 따른 문자 변환
        let ch = self.english_keymap.get_char(keycode, modifier.shift);

        if let Some(c) = ch {
            unim_log!("ENGINE", "문자 키: '{}'", c);

            // 키보드 맵에서 자모 찾기
            if let Some(ref keyboard_map) = self.keyboard_map {
                if let Some(jamo) = keyboard_map.get(&c) {
                    unim_log!("ENGINE", "자모 매핑: {:?}", jamo);

                    // Special 자모는 비-한국어 문자이므로 별도 처리
                    if let JamoEnum::Special(special_char) = jamo {
                        let ch_to_commit = *special_char; // 먼저 복사
                        unim_log!("ENGINE", "Special 자모 처리: '{}'", ch_to_commit);
                        // 조합 중이면 먼저 커밋
                        self.flush_preedit();
                        // Special 문자를 commit_buffer에 추가
                        self.commit_buffer.push(ch_to_commit);
                        return InputResult::committed();
                    }

                    // 일반 자모 입력
                    self.korean_context.process_jamo(*jamo);

                    // committed 문자가 있으면 commit_buffer에 추가
                    let committed = self.korean_context.get_committed();
                    unim_log!(
                        "ENGINE",
                        "context.committed='{}', context.preedit='{}'",
                        committed,
                        self.korean_context.get_preedit()
                    );

                    if !committed.is_empty() {
                        self.commit_buffer.push_str(committed);
                        // committed 문자열만 비우기 (preedit은 유지)
                        self.korean_context.clear_committed();
                        unim_log!(
                            "ENGINE",
                            "commit_buffer에 추가 후: '{}'",
                            self.commit_buffer
                        );
                    }

                    self.update_preedit_cache();
                    unim_log!(
                        "ENGINE",
                        "preedit_cache 업데이트 후: '{}'",
                        self.preedit_cache
                    );

                    if !self.commit_buffer.is_empty() {
                        unim_log!("ENGINE", "-> InputResult::committed()");
                        return InputResult::committed();
                    }
                    unim_log!("ENGINE", "-> InputResult::preedit_updated()");
                    return InputResult::preedit_updated();
                }
            }

            // 자모가 아닌 문자 (기호 등)
            self.flush_preedit();
            self.commit_buffer.push(c);
            return InputResult::committed();
        }

        // 조합 중에 문자가 아닌 키(화살표, F키 등) 입력 시 커밋 후 키 통과
        if self.korean_context.is_composing() {
            self.flush_preedit();
            unim_log!("ENGINE", "비문자키 -> 조합 커밋 후 키 통과");
            return InputResult::committed_passthrough();
        }

        InputResult::not_consumed()
    }

    /// 영어 키 입력을 처리합니다.
    pub(super) fn process_english_key(
        &mut self,
        keycode: KeyCode,
        modifier: ModifierState,
    ) -> InputResult {
        // Space는 영문 키맵에 매핑돼 있지 않으므로 여기서 먼저 커밋한다.
        // 한국어 모드는 process_korean_key에서 동일하게 처리한다.
        //
        // 이전에는 Space를 not_consumed로 반환했는데, 영문 알파벳은 consumed=true/
        // commit='x' 경로를 타고 Space만 consumed=false가 돼서 GTK IM 모듈의 상태
        // 전환이 꼬여 gedit 등에서 공백이 간헐적으로 drop되는 회귀가 있었다.
        // GNOME 경로는 이미 Korean 모드에서 Space를 commit=' '로 받고 있었으므로,
        // 영문 모드에서도 같은 전략으로 통일한다.
        if keycode == KeyCode::Space {
            self.commit_buffer.push(' ');
            return InputResult::committed();
        }

        // JSON 키맵 기반으로 레이아웃에 따른 문자 변환
        // CapsLock은 알파벳 문자에만 적용 (숫자/기호는 Shift만)

        // 먼저 lower case 문자를 확인하여 알파벳인지 판단
        // 이렇게 하면 Dvorak/Colemak 등 커스텀 키맵에서도 정상 동작
        let lower_char = self.english_keymap.get_char(keycode, false);
        let is_alpha_output = lower_char.map(|c| c.is_ascii_alphabetic()).unwrap_or(false);

        let shifted = if is_alpha_output {
            // 알파벳 출력: Shift XOR CapsLock (둘 다 켜면 소문자)
            modifier.shift ^ modifier.caps_lock
        } else {
            // 숫자/기호 출력: Shift만 적용, CapsLock 무시
            modifier.shift
        };
        let ch = self.english_keymap.get_char(keycode, shifted);

        if let Some(c) = ch {
            self.commit_buffer.push(c);
            return InputResult::committed();
        }

        InputResult::not_consumed()
    }

    /// Config 의 자동 영문 전환 트리거 이름을 `AutoEnglishTrigger` 로 파싱합니다.
    ///
    /// 표기 문법(접두사 기반):
    /// - `"key:<Name>"`     — `Functional`. `<Name>` 은 `KeyCode::from_name` 호환,
    ///                         `"Shift<Name>"` 가상 이름 허용 (예: `key:ShiftSemicolon`).
    /// - `"char:<문자>"`    — `Character`. `<문자>` 의 첫 char 만 사용 (예: `char:/`).
    /// - 무접두사 (legacy)  — 종전 규칙대로 `Functional` 로 흡수
    ///                         (`Escape` / `Slash` / `ShiftSemicolon` 등 호환).
    ///
    /// # Returns
    ///
    /// - `Some(AutoEnglishTrigger::Functional { … })`
    /// - `Some(AutoEnglishTrigger::Character(ch))`
    /// - `None` — 알 수 없는 이름 / 빈 char
    pub(super) fn parse_trigger_key(name: &str) -> Option<AutoEnglishTrigger> {
        if let Some(rest) = name.strip_prefix("char:") {
            let ch = rest.chars().next()?;
            return Some(AutoEnglishTrigger::Character(ch));
        }

        if let Some(rest) = name.strip_prefix("key:") {
            let (code, shift) = Self::parse_functional_name(rest)?;
            return Some(AutoEnglishTrigger::Functional { code, shift });
        }

        // legacy: 접두사 없이 KeyCode 이름 또는 "Shift<Name>" 만 들어온 경우
        // → Functional 로 흡수해 100% 호환성 유지.
        let (code, shift) = Self::parse_functional_name(name)?;
        Some(AutoEnglishTrigger::Functional { code, shift })
    }

    /// `key:` 접두사 분기와 legacy 분기에서 공유하는 KeyCode 파서.
    ///
    /// `"Shift<Name>"` 는 Shift 필수, 문자 키(알파벳/숫자/기호)는 Shift 없을 때만,
    /// 제어 키(Escape/Tab/Enter/F*/Arrows …)는 Shift 무관으로 매핑한다.
    fn parse_functional_name(name: &str) -> Option<(KeyCode, Option<bool>)> {
        if let Some(stripped) = name.strip_prefix("Shift") {
            let code = KeyCode::from_name(stripped);
            if code == KeyCode::Unknown {
                return None;
            }
            return Some((code, Some(true)));
        }

        let code = KeyCode::from_name(name);
        if code == KeyCode::Unknown {
            return None;
        }

        // 문자 키(기호·숫자)는 Shift 없을 때만 매칭 (shift 조합은 `"Shift<Name>"` 로 지정).
        // 그 외 제어 키(Escape/Tab/Enter/F*/Arrows 등)는 Shift 무관.
        let shift_sensitive = matches!(
            code,
            KeyCode::A
                | KeyCode::B
                | KeyCode::C
                | KeyCode::D
                | KeyCode::E
                | KeyCode::F
                | KeyCode::G
                | KeyCode::H
                | KeyCode::I
                | KeyCode::J
                | KeyCode::K
                | KeyCode::L
                | KeyCode::M
                | KeyCode::N
                | KeyCode::O
                | KeyCode::P
                | KeyCode::Q
                | KeyCode::R
                | KeyCode::S
                | KeyCode::T
                | KeyCode::U
                | KeyCode::V
                | KeyCode::W
                | KeyCode::X
                | KeyCode::Y
                | KeyCode::Z
                | KeyCode::Num0
                | KeyCode::Num1
                | KeyCode::Num2
                | KeyCode::Num3
                | KeyCode::Num4
                | KeyCode::Num5
                | KeyCode::Num6
                | KeyCode::Num7
                | KeyCode::Num8
                | KeyCode::Num9
                | KeyCode::Minus
                | KeyCode::Equal
                | KeyCode::BracketLeft
                | KeyCode::BracketRight
                | KeyCode::Backslash
                | KeyCode::Semicolon
                | KeyCode::Quote
                | KeyCode::Backquote
                | KeyCode::Comma
                | KeyCode::Period
                | KeyCode::Slash
                | KeyCode::Space
        );

        Some((code, if shift_sensitive { Some(false) } else { None }))
    }

    /// 이 키 입력에 매칭되는 자동 영문 트리거가 있으면 그 트리거를 반환.
    ///
    /// 활성화 + 한글 모드에서만 평가.
    ///
    /// - `Functional { code, shift }` — `(keycode, shift)` 직접 비교.
    /// - `Character(ch)` — 키맵 거친 산출 char 비교. 한국어 자판이 비-QWERTY
    ///   (예: 세벌식390) 인 경우엔 KeyboardMap 의 `Special(ch)` 매핑까지 확인하여
    ///   해당 자판에서 산출되는 실제 문자(`'/'` 등) 와 비교한다.
    pub(super) fn match_auto_english_trigger(
        &self,
        keycode: KeyCode,
        modifier: ModifierState,
    ) -> Option<AutoEnglishTrigger> {
        if !self.auto_english_enabled {
            return None;
        }
        if self.input_category != InputCategory::Korean {
            return None;
        }
        self.auto_english_triggers
            .iter()
            .find(|t| match t {
                AutoEnglishTrigger::Functional { code, shift } => {
                    *code == keycode
                        && match shift {
                            None => true,
                            Some(required) => *required == modifier.shift,
                        }
                }
                AutoEnglishTrigger::Character(ch) => {
                    self.produces_char_in_korean(keycode, modifier.shift) == Some(*ch)
                }
            })
            .copied()
    }

    /// 한국어 모드에서 `(keycode, shift)` 가 어떤 char 를 산출할지 계산한다.
    ///
    /// 산출 경로:
    /// 1. `english_keymap.get_char` 로 영문 자판의 char 를 얻는다 (e.g. `'G'`).
    /// 2. KoreanLayout 의 `keyboard_map` 에서 그 char 를 lookup.
    ///    - `Some(JamoEnum::Special(ch))` 면 그 자판이 의도한 비-자모 char (e.g. 세벌식390 의 `'/'`).
    ///    - `Some(JamoEnum::Cho/Jung/Jong)` 면 한글 자모이므로 char 산출 없음 → `None`.
    ///    - 매핑이 없으면(QWERTY std 에서 기호 등) 영문 char 를 그대로 사용.
    /// 3. `keyboard_map` 자체가 없으면 영문 char 를 그대로 사용.
    fn produces_char_in_korean(&self, keycode: KeyCode, shift: bool) -> Option<char> {
        let en_ch = self.english_keymap.get_char(keycode, shift)?;
        let Some(ref kmap) = self.keyboard_map else {
            return Some(en_ch);
        };
        match kmap.get(&en_ch) {
            Some(JamoEnum::Special(c)) => Some(*c),
            Some(_) => None, // 자모로 매핑됨 → char 산출 아님
            None => Some(en_ch),
        }
    }


    /// preedit 캐시를 업데이트합니다.
    pub(super) fn update_preedit_cache(&mut self) {
        self.preedit_cache = self.korean_context.get_preedit().to_string();
    }

    /// preedit을 commit_buffer로 플러시합니다.
    pub(super) fn flush_preedit(&mut self) {
        if self.korean_context.is_composing() {
            self.korean_context.commit();
            let committed = self.korean_context.get_committed();
            self.commit_buffer.push_str(committed);
            self.korean_context.clear();
            self.preedit_cache.clear();
        }
    }

    /// 입력 카테고리를 토글합니다.
    pub(super) fn toggle_input_category(&mut self) {
        // 조합 중이면 먼저 플러시
        self.flush_preedit();

        self.input_category = match self.input_category {
            InputCategory::Korean => InputCategory::English,
            InputCategory::English => InputCategory::Korean,
        };

        // 상태 파일 업데이트
        self.update_status_file();
    }

    /// 상태 파일을 업데이트합니다.
    pub(super) fn update_status_file(&self) {
        let status_category = match self.input_category {
            InputCategory::Korean => crate::status::InputCategory::Korean,
            InputCategory::English => crate::status::InputCategory::English,
        };
        // 오류 발생 시 무시 (로깅은 하지 않음 - 성능을 위해)
        let _ = crate::status::set_status(status_category);
    }
}
