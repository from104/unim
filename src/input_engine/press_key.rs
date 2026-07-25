//! Press 계열 키 처리 hot path.
//!
//! `press_key_code`/`press_key`/`process_korean_key`/`process_english_key`/
//! `parse_trigger_key`/`match_auto_english_trigger` + preedit/상태 helper들을 한 파일에 묶어
//! 인라인 가능성을 보존한다.

use super::engine::InputEngine;
use crate::input_engine::chord_buffer::ChordFlushResult;
use crate::input_engine::chord_compose::{ChordEntryKind, ChordResult};
use crate::input_engine::chord_compose::compose_chord;
use super::types::{AtfHotkey, AutoEnglishTrigger, InputResult};
use crate::config::{Config, InputCategory};
use crate::hangul::jamo::JamoEnum;
use crate::keycode::{KeyCode, ModifierState};
use crate::unim_log;

/// ATF 토글 핫키 오토리핏 디바운스 창.
///
/// 동일 키코드가 이 시간 이내에 다시 매칭되면(키 홀드에 의한 자동반복) 토글을
/// 생략하고 소비만 한다. 홀드가 지속되는 한 매 반복마다 시각이 갱신돼 rolling
/// window 로 차단이 유지된다. 일반적인 키보드 자동반복 주기(약 30~40ms)보다
/// 넉넉히 크되, 의도적 재토글을 방해하지 않을 만큼 짧게 잡는다.
const ATF_HOTKEY_DEBOUNCE: std::time::Duration = std::time::Duration::from_millis(300);

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
        // 고정키(Sticky Keys) 래치 잔류 마스킹 — 직전에 수정자 토글키(RightAlt 등)로
        // 전환이 성사됐다면, 이번 키의 잔류 수정자 비트 하나를 지운다(일회성 소비).
        // 이 처리는 어떤 분기보다 먼저 와야 아래의 단축키 가드(control||alt||super)가
        // 래치 잔류를 단축키로 오분류해 첫 자모를 버리는 것을 막는다. 마스크가 없는
        // 평소 경로(None)는 modifier 를 건드리지 않아 바이트 동일 무회귀.
        let mut modifier = modifier;
        if let Some(latch) = self.sticky_toggle_mask.take() {
            latch.clear_from(&mut modifier);
        }

        // 설정된 한/영 전환키 여부. RightAlt 처럼 그 자체가 수정자인 키도 토글키로
        // 지정될 수 있으므로(기본값 ["Korean","RightAlt"]) is_modifier 가드보다
        // 먼저 판정해 둔다.
        let is_toggle_key = self.toggle_keys.contains(&keycode);

        // 수정자 키만 누른 경우 무시 — 단, 토글키(RightAlt 등)는 아래에서 처리.
        if keycode.is_modifier() && !is_toggle_key {
            return InputResult::not_consumed();
        }

        // 팝업(한자/특수문자/이모지) 활성 상태에서 키 인터셉트.
        // PR #1: Emoji 팝업도 이 분기로 들어오도록 popup_state 진입.
        // (팝업 중에는 토글키도 다른 키와 동일하게 팝업이 우선 처리한다 —
        //  기존 한/영 키 동작과 동일.)
        if self.hanja_mode || self.special_char_mode || self.is_emoji_popup_active() {
            return self.process_popup_key(keycode, modifier, _config);
        }

        // 이모지 팝업 진입은 한자 키 dual-purpose 가 단일 진입점이다 (preedit/조합 idle
        // 상태에서 한자 키가 start_emoji_popup() 호출). 별도 단축키 트리거는 제거됨.

        // 한/영 전환 처리 (설정 기반) — RightAlt 같은 수정자 토글키도 처리하려면
        // 아래 Ctrl/Alt/Super 단축키 가드보다 먼저 검사해야 한다. (RightAlt 는 자기
        // 자신이 Alt 비트를 세우므로, 토글 분기가 단축키 가드 뒤에 있던 종전 코드에선
        // 항상 단축키로 분류돼 토글이 죽어 있었다 — VK_HANGUL 만 동작.)
        // 단, 토글키가 자신이 부여하는 수정자(RightAlt→Alt) 외의 단축키 수정자
        // (Ctrl/Super, 또는 토글키가 수정자가 아닌데 Alt)와 함께 눌리면 단축키로 보고
        // 토글하지 않는다(예: Ctrl+한/영, Ctrl+RightAlt 는 그대로 통과).
        if is_toggle_key {
            let self_is_modifier = keycode.is_modifier();
            let shortcut_combo = modifier.control
                || modifier.super_key
                || (modifier.alt && !self_is_modifier);
            if !shortcut_combo {
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

                // 고정키 충돌 방어: 토글키가 그 자체로 수정자(RightAlt 등)라면, 고정키
                // 래치가 다음 키에 그 수정자 비트를 잔류시킬 수 있다. 다음 키 1건에서
                // 해당 비트를 지우도록 마스크를 예약한다(비-수정자 토글키 Korean/Hangul
                // 은 래치가 없어 예약 안 함 → 평소 경로 무회귀).
                self.sticky_toggle_mask =
                    super::engine::LatchModifier::of_keycode(keycode);

                // 조합 중이었으면 commit이 발생했으므로 committed() 반환
                if was_composing {
                    return InputResult::committed();
                }
                return InputResult::consumed();
            }
        }

        // AutoTypeFix 토글 단축키 (설정 기반 — 기본 `Shift+F8` 가 전체 토글).
        //
        // 매칭은 수정자 **정확 일치**(`AtfHotkey`)다: 표기에 등장한 수정자는 눌려
        // 있어야 하고, 등장하지 않은 수정자는 눌리면 안 된다. 그래서 종전의
        // `shortcut_combo` 일괄 배제 가드가 필요 없다 — 설정에 없는 조합(사용자가
        // `F10` 만 등록했을 때의 Shift+F10 컨텍스트 메뉴, Ctrl+F10 등)은 매칭 자체가
        // 실패해 소비되지 않고 그대로 앱에 통과하므로, 앱 단축키 보호를 정확-일치가
        // 구조적으로 대신한다. 수정자 없는 기존 표기(`F10`)는 네 수정자가 전부 false 로
        // 파싱되어 종전 가드 시절과 동작이 동일하다(하위 호환).
        //
        // 이 분기는 한자 분기(아래)보다 앞서므로 `Shift+F9`=ATF, 맨 `F9`=한자/이모지 로
        // 자연히 갈린다. 매칭 시 config·상태는 건드리지 않고 pending_atf_toggle 에 대상
        // 플래그만 적재(InputResult ABI 보존) — 호스트가 take_atf_toggle() 로 드레인해
        // 반전·persist·통지한다. 팝업 활성 중에는 위 팝업 분기가 선행 처리하므로 여기
        // 도달하지 않는다.
        if let Some(kind) = self.atf_hotkey_kind(keycode, modifier) {
            // 오토리핏 디바운스: 동일 키 홀드로 press_key 가 연속 호출될 때 토글이
            // 반복 반전되는 것을 막는다. 소비는 항상 유지(홀드 중 키가 앱으로 새면
            // 안 됨)하되, ATF_HOTKEY_DEBOUNCE 이내 동일 키코드 재매칭이면 토글만
            // 생략하고 시각을 갱신해 홀드 내내 rolling window 로 차단한다.
            let now = std::time::Instant::now();
            let is_repeat = matches!(
                self.last_atf_hotkey,
                Some((last_key, last_at))
                    if last_key == keycode && now.duration_since(last_at) < ATF_HOTKEY_DEBOUNCE
            );
            self.last_atf_hotkey = Some((keycode, now));
            if !is_repeat {
                self.pending_atf_toggle = Some(kind);
            }
            return InputResult::consumed();
        }

        // Control/Alt가 눌린 경우 (단축키) 무시
        if modifier.control || modifier.alt || modifier.super_key {
            // 자동 영문 전환 — modifier 조합 트리거 (opt-in, 예: `key:Ctrl+B`).
            // 이 가드 블록은 Ctrl/Alt/Super 조합을 process_korean_key(:235)의
            // 비조합 매처보다 먼저 조기 return 하므로, 조합 트리거는 여기서 평가해야
            // press flow 에 도달한다. 매처의 정확-일치(ctrl/alt/super 평 bool) 덕에
            // legacy·비조합 트리거(전부 false)는 조합 modifier 와 구조적으로 불일치 →
            // 종전 '조합 시 불발' 이 그대로 보존된다.
            //
            // 발동 시 계약: 키는 **절대 소비하지 않는다**(tmux prefix Ctrl+B 등이
            // 앱으로 반드시 전달돼야 함). char 커밋도 하지 않는다 — 조합 중이면
            // 직전 preedit 만 flush(committed_passthrough), idle 이면 not_consumed.
            if self.match_auto_english_trigger(keycode, modifier).is_some() {
                let was_composing = self.korean_context.is_composing();
                if was_composing {
                    self.flush_preedit();
                }
                self.set_input_category(InputCategory::English);
                unim_log!(
                    "ENGINE",
                    "auto-english(combo): '{:?}' -> 영문 전환 + passthrough (composing={})",
                    keycode,
                    was_composing
                );
                return if was_composing {
                    InputResult::committed_passthrough()
                } else {
                    InputResult::not_consumed()
                };
            }

            // 조합 중이면 먼저 커밋
            if self.korean_context.is_composing() {
                self.flush_preedit();
                return InputResult::committed();
            }
            return InputResult::not_consumed();
        }

        // 비밀번호/PIN 필드에서는 한글 모드를 영어로 강제 전환
        if self.content_purpose.should_block_hangul()
            && self.input_category == InputCategory::Korean
        {
            unim_log!("ENGINE", "비밀번호 필드 감지: 영문 모드로 강제 전환");
            self.set_input_category(InputCategory::English);
        }

        // Hanja 키는 언어 모드 무관 dispatch — preedit/조합 상태에 따라 분기.
        //   * 조합 중 (Korean only): 한자 변환
        //   * idle (조합 없음): 이모지 팝업 트리거 (항상 ON)
        //
        // English 모드에서도 idle Hanja → 이모지가 동작해야 함. 종전엔 분기가
        // process_korean_key 안에 있어 영문 모드에서 Hanja 키가 not_consumed 로
        // 떨어져 첫 키 입력이 무시됐다.
        if self.hanja_keys.contains(&keycode) {
            // chord 버퍼에 대기 중인 자모가 있으면 먼저 flush — Space/Enter와 동일 패턴.
            // chord 진행 중 한자 키 입력 시 현재 chord 를 음절로 확정한 뒤 한자 변환 진입.
            // (preview inject 된 경우 finalize_chord_buffer 가 중복 처리 회피)
            self.finalize_chord_buffer();
            let idle =
                self.preedit_cache.is_empty() && !self.korean_context.is_composing();
            if idle {
                self.start_emoji_popup();
                return InputResult::consumed();
            }
            return self.start_hanja_conversion();
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

        // Hanja 키 dispatch 는 press_key() 상위에서 언어 모드 무관하게 처리됨
        // (영문 모드 첫 키 회귀 방지). 여기까진 도달하지 않는다.

        // 자동 영문 전환 (opt-in): 지정 트리거 키 입력 시 조합 커밋 + 영문 모드로 영구 전환.
        // 제어 키(Escape/Tab/Enter 등)는 passthrough, 문자 키(`/`/`:` 등)는 해당 문자 commit.
        // 이 경로는 **비조합** 트리거 전용이다 — Ctrl/Alt/Super 조합 트리거(`key:Ctrl+B`)는
        // press_key() 상위의 단축키 가드에서 이미 처리돼 여기 도달하지 않는다(정확-일치로
        // 조합 트리거는 modifier 전부 false 인 이 지점의 매처와 절대 일치하지 않음).
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
            // chord 버퍼에 대기 중인 자모가 있으면 먼저 finalize (preview 이미 반영 여부 자동 처리)
            self.finalize_chord_buffer();
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
            self.finalize_chord_buffer();
            if self.korean_context.is_composing() {
                self.flush_preedit();
                unim_log!("ENGINE", "Enter -> 조합 커밋 후 키 통과");
                return InputResult::committed_passthrough();
            }
            return InputResult::not_consumed();
        }

        // Tab 처리 - 조합 커밋 후 키 통과
        if keycode == KeyCode::Tab {
            self.finalize_chord_buffer();
            if self.korean_context.is_composing() {
                self.flush_preedit();
                unim_log!("ENGINE", "Tab -> 조합 커밋 후 키 통과");
                return InputResult::committed_passthrough();
            }
            return InputResult::not_consumed();
        }

        // Escape 처리 - 조합 커밋 후 키 통과
        if keycode == KeyCode::Escape {
            // chord 버퍼 폐기 (Escape는 입력 취소 의미)
            self.chord_buffer.clear();
            if self.korean_context.is_composing() {
                self.flush_preedit();
                unim_log!("ENGINE", "Escape -> 조합 커밋 후 키 통과");
                return InputResult::committed_passthrough();
            }
            return InputResult::not_consumed();
        }

        // Space 처리
        if keycode == KeyCode::Space {
            self.finalize_chord_buffer();
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

            // 룰 B (schema v2 `key_meta.context_alt`) — 한글 모드 + 팝업 비활성에서만.
            // 조건이 맞으면 정상 jamo 흐름으로 진입하고, 불충족이면 fallback 리터럴 commit.
            if matches!(self.input_category, InputCategory::Korean)
                && self.popup_state.is_none()
                && !self.hanja_mode
                && !self.special_char_mode
            {
                if let Some(meta) = self.key_meta_map.get(&c) {
                    if let Some(alt) = meta.context_alt.clone() {
                        use crate::keystroke::profile::ContextCondition as C;
                        let cond_ok = match alt.when {
                            C::Empty => !self.korean_context.is_composing(),
                            C::Composing => self.korean_context.is_composing(),
                            C::ChoseongOnly => self.korean_context.is_only_cho_filled(),
                            C::JungseongOnly => self.korean_context.is_only_jung_filled(),
                            C::ChoJungFilled => self.korean_context.is_cho_jung_filled(),
                            C::JongseongFilled => self.korean_context.is_jong_filled(),
                            C::LastIsCho => self.korean_context.last_jamo_is_cho(),
                            C::LastIsJung => self.korean_context.last_jamo_is_jung(),
                            C::LastIsJong => self.korean_context.last_jamo_is_jong(),
                        };
                        if !cond_ok {
                            unim_log!(
                                "ENGINE",
                                "key_meta.context_alt 불충족 → fallback '{}' commit",
                                alt.fallback
                            );
                            self.flush_preedit();
                            self.commit_buffer.push_str(&alt.fallback);
                            return InputResult::committed();
                        }
                    }
                }
            }

            // 키보드 맵에서 자모 찾기
            if let Some(ref keyboard_map) = self.keyboard_map {
                if let Some(jamo) = keyboard_map.get(&c) {
                    unim_log!("ENGINE", "자모 매핑: {:?}", jamo);

                    // Special 자모는 비-한국어 문자이므로 별도 처리.
                    // chord 버퍼가 활성 중이면 먼저 finalize 한다.
                    if let JamoEnum::Special(special_char) = jamo {
                        let ch_to_commit = *special_char;
                        unim_log!("ENGINE", "Special 자모 처리: '{}'", ch_to_commit);
                        self.finalize_chord_buffer();
                        self.flush_preedit();
                        self.commit_buffer.push(ch_to_commit);
                        return InputResult::committed();
                    }

                    // 일반 자모 — chord 경로 분기
                    let jamo_copy = *jamo;
                    let jamo_meta = self
                        .key_meta_map
                        .get(&c)
                        .map(|km| km.to_jamo_meta())
                        .unwrap_or_default();

                    if self.chord_buffer.is_active() {
                        // chord 모드: 버퍼에 push → flush 여부 결정
                        unim_log!("ENGINE", "chord push: {:?}", jamo_copy);
                        // push 직전 preview 상태 capture (chord_buffer 내부에서 만료/MAX flush 시 리셋되므로)
                        let prev_preview_active = self.chord_buffer.was_preview_injected();
                        let prev_was_atomic = self.chord_buffer.was_preview_atomic();
                        match self.chord_buffer.push_jamo(jamo_copy, jamo_meta) {
                            None => {
                                // 윈도우 내 누적 — 현재 buffer 부분 결합 결과를 preedit 에 반영
                                self.update_chord_preview();
                                unim_log!(
                                    "ENGINE",
                                    "chord 진행 중 (preview) -> preedit='{}'",
                                    self.preedit_cache
                                );
                                return if !self.commit_buffer.is_empty() {
                                    InputResult::committed()
                                } else {
                                    InputResult::preedit_updated()
                                };
                            }
                            Some(entries) => {
                                unim_log!(
                                    "ENGINE",
                                    "chord 만료 flush {} 자모 → 새 chord 시작 (prev_preview={}, atomic={})",
                                    entries.len(),
                                    prev_preview_active,
                                    prev_was_atomic
                                );
                                let new_chord_started = self.chord_buffer.has_pending();
                                if prev_preview_active {
                                    if new_chord_started {
                                        // 만료(expired) flush + 새 chord 1개 키. 직전 preview 의 컴포저 상태는
                                        // 그대로 두고, update_chord_preview 의 단일 키 sequential 경로가
                                        // process_jamo 로 결합 시도하도록 위임 (사용자가 원하는 "가" 형성).
                                    } else {
                                        // MAX flush: buffer 가 비었고 entries 는 이전 + 현재 자모 전체.
                                        // preview 흔적을 정확히 되돌린 뒤 전체 재적용.
                                        if prev_was_atomic {
                                            self.korean_context.clear_composing();
                                        } else {
                                            self.korean_context.pop_last_jamo();
                                        }
                                        self.apply_chord_entries(entries);
                                    }
                                } else {
                                    self.apply_chord_entries(entries);
                                }
                                if new_chord_started {
                                    self.update_chord_preview();
                                }
                                return if !self.commit_buffer.is_empty() {
                                    InputResult::committed()
                                } else {
                                    InputResult::preedit_updated()
                                };
                            }
                        }
                    } else {
                        // chord OFF (window_ms=0) → 즉시 처리 (기존 동작 100% 동일)
                        self.korean_context
                            .process_jamo_with_meta(jamo_copy, jamo_meta);

                        let committed = self.korean_context.get_committed();
                        unim_log!(
                            "ENGINE",
                            "context.committed='{}', context.preedit='{}'",
                            committed,
                            self.korean_context.get_preedit()
                        );

                        if !committed.is_empty() {
                            self.commit_buffer.push_str(committed);
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
            }

            // 자모가 아닌 문자 (기호 등)
            if self.chord_buffer.is_active() {
                // chord 모드: 비자모도 윈도우에 합류
                unim_log!("ENGINE", "chord push_non_jamo: '{}'", c);
                let prev_preview_active = self.chord_buffer.was_preview_injected();
                let prev_was_atomic = self.chord_buffer.was_preview_atomic();
                match self.chord_buffer.push_non_jamo(c) {
                    None => {
                        // 비자모 합류 — Case C 가 되므로 preview 는 폐기되고 빈 preedit.
                        // (실제 commit 은 만료/finalize 시점에 apply_chord_entries 로 처리)
                        self.update_chord_preview();
                        return if !self.commit_buffer.is_empty() {
                            InputResult::committed()
                        } else {
                            InputResult::preedit_updated()
                        };
                    }
                    Some(entries) => {
                        unim_log!(
                            "ENGINE",
                            "chord 만료 flush (비자모 트리거) {} 항목 (prev_preview={}, atomic={})",
                            entries.len(),
                            prev_preview_active,
                            prev_was_atomic
                        );
                        let new_chord_started = self.chord_buffer.has_pending();
                        if prev_preview_active {
                            if new_chord_started {
                                // 비자모로 새 chord 가 단일 키로 시작됨. 단일 비자모는 update_chord_preview 가
                                // flush_preedit + commit 처리. 직전 preview 컴포저 상태 그대로 둠.
                            } else {
                                // MAX flush — 정확한 undo 후 전체 재적용
                                if prev_was_atomic {
                                    self.korean_context.clear_composing();
                                } else {
                                    self.korean_context.pop_last_jamo();
                                }
                                self.apply_chord_entries(entries);
                            }
                        } else {
                            self.apply_chord_entries(entries);
                        }
                        if new_chord_started {
                            self.update_chord_preview();
                        }
                        return if !self.commit_buffer.is_empty() {
                            InputResult::committed()
                        } else {
                            InputResult::preedit_updated()
                        };
                    }
                }
            } else {
                // chord OFF: 기존 동작 유지 (즉시 commit)
                self.finalize_chord_buffer();
                self.flush_preedit();
                self.commit_buffer.push(c);
                return InputResult::committed();
            }
        }

        // 조합 중에 문자가 아닌 키(화살표, F키 등) 입력 시 chord flush 후 커밋 후 키 통과
        self.finalize_chord_buffer();
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
    /// - `"key:<keyspec>"` — `Functional`. `<keyspec>` 은 `(mod+)* <Name>` 형태.
    ///   - `<Name>` 은 `KeyCode::from_name` 호환(대소문자 구분: `B`/`Escape`/`F1`),
    ///     `"Shift<Name>"` 융합형 허용 (예: `key:ShiftSemicolon`).
    ///   - modifier 토큰(`+` 구분): `ctrl`/`control`, `alt`, `super`/`win`/`meta`,
    ///     `shift`. ASCII 대소문자 무관, 순서 무관, 중복 멱등. 예: `key:Ctrl+B`,
    ///     `key:Ctrl+Shift+B`, `key:Alt+F1`, `key:Super+Space`, `key:shift+ctrl+B`.
    ///   - modifier(ctrl/alt/super)는 **정확 일치** 의미로 저장된다(등장 = 필수).
    /// - `"char:<문자>"` — `Character`. `<문자>` 의 첫 char 만 사용 (예: `char:/`).
    /// - 무접두사 (legacy) — 종전 규칙대로 `Functional` 로 흡수
    ///   (`Escape` / `Slash` / `ShiftSemicolon` 등 호환). '+' 조합 문법은 `key:`
    ///   접두사 전용이며, 무접두사 `"Ctrl+B"` 는 legacy KeyCode 이름이 아니므로 `None`.
    ///
    /// # Returns
    ///
    /// - `Some(AutoEnglishTrigger::Functional { … })`
    /// - `Some(AutoEnglishTrigger::Character(ch))`
    /// - `None` — 알 수 없는 이름 / 미지 modifier 토큰 / 빈 base / 빈 char
    pub(super) fn parse_trigger_key(name: &str) -> Option<AutoEnglishTrigger> {
        if let Some(rest) = name.strip_prefix("char:") {
            let ch = rest.chars().next()?;
            return Some(AutoEnglishTrigger::Character(ch));
        }

        if let Some(rest) = name.strip_prefix("key:") {
            return Self::parse_keyspec(rest);
        }

        // legacy: 접두사 없이 KeyCode 이름 또는 "Shift<Name>" 만 들어온 경우
        // → Functional 로 흡수해 100% 호환성 유지. modifier 전부 false.
        let (code, shift) = Self::parse_functional_name(name)?;
        Some(AutoEnglishTrigger::Functional {
            code,
            shift,
            ctrl: false,
            alt: false,
            super_key: false,
        })
    }

    /// `key:` 접두사 뒤 keyspec(`(mod+)* <Name>`) 을 `Functional` 로 파싱한다.
    ///
    /// - `'+'` 가 없으면 단일 base 표기(legacy 와 동일 문법) — `parse_functional_name`
    ///   결과에 modifier 전부 false 를 붙여 반환한다. `key:Escape`/`key:ShiftSemicolon`
    ///   등 종전 `key:` 단일 표기의 파싱 결과가 바이트 동일하게 보존된다.
    ///   **trim 하지 않는다**: `key: Escape`(내부 공백) 같은 비정상 표기는 종전처럼
    ///   침묵 무시(`None`)된다 — 공백 관용은 `'+'` 조합 표기(아래) 전용이라, 단일
    ///   base 의 하위호환(비정상=무시)이 1비트도 변하지 않도록 유지한다.
    /// - `'+'` 가 있으면 마지막 토큰을 base(`parse_functional_name`)로, 앞 토큰들을
    ///   modifier 로 해석한다. 토큰별 공백은 trim(`key: Ctrl + B` 허용), modifier
    ///   이름은 ASCII 대소문자 무관·순서 무관·중복 멱등. `shift` 토큰이 있으면
    ///   base 의 shift 규칙을 덮어써 `Some(true)`(필수)로 만든다.
    /// - 미지 modifier 토큰, 빈 base(`key:Ctrl+`), 미지 base 는 `None`(침묵 무시).
    fn parse_keyspec(spec: &str) -> Option<AutoEnglishTrigger> {
        // '+' 없음 → 단일 base. legacy·종전 key: 단일 표기와 완전 동일 파싱.
        // trim 하지 않는다(하위호환): `key: Escape` 등 내부 공백 표기는 종전대로 None.
        if !spec.contains('+') {
            let (code, shift) = Self::parse_functional_name(spec)?;
            return Some(AutoEnglishTrigger::Functional {
                code,
                shift,
                ctrl: false,
                alt: false,
                super_key: false,
            });
        }

        let mut ctrl = false;
        let mut alt = false;
        let mut super_key = false;
        let mut shift_forced = false;

        // '+' 로 분할, 마지막 토큰 = base, 앞 토큰 = modifier.
        let mut tokens: Vec<&str> = spec.split('+').map(str::trim).collect();
        let base = tokens.pop()?; // split 결과는 항상 1개 이상 → pop 안전
        for tok in tokens {
            if tok.eq_ignore_ascii_case("ctrl") || tok.eq_ignore_ascii_case("control") {
                ctrl = true;
            } else if tok.eq_ignore_ascii_case("alt") {
                alt = true;
            } else if tok.eq_ignore_ascii_case("super")
                || tok.eq_ignore_ascii_case("win")
                || tok.eq_ignore_ascii_case("meta")
            {
                super_key = true;
            } else if tok.eq_ignore_ascii_case("shift") {
                shift_forced = true;
            } else {
                // 미지 modifier 토큰 → 현행 침묵 무시 정책.
                return None;
            }
        }

        // base 는 대소문자 구분(KeyCode::from_name) — `key:Ctrl+B` OK, `key:Ctrl+b` None.
        let (code, base_shift) = Self::parse_functional_name(base)?;
        let shift = if shift_forced { Some(true) } else { base_shift };

        Some(AutoEnglishTrigger::Functional {
            code,
            shift,
            ctrl,
            alt,
            super_key,
        })
    }

    /// Config 의 ATF 토글 단축키 표기를 `AtfHotkey` 로 파싱합니다.
    ///
    /// 표기 문법: `[수정자+]* <KeyName>` — `parse_keyspec`(자동 영문 전환의 `key:`
    /// 조합 문법)과 토큰 규칙을 일부러 일치시켰다.
    /// - modifier 토큰(`+` 구분): `ctrl`/`control`, `alt`, `super`/`win`/`meta`,
    ///   `shift`. ASCII 대소문자 무관, 순서 무관, 중복 멱등.
    ///   예: `Shift+F9`, `F10`, `Ctrl+Shift+F8`, `shift+ctrl+F8`.
    /// - 구분자는 `+` 가 정식이되, `+` 가 전혀 없는 표기에서는 `-` 를 대체
    ///   구분자로 허용한다(`Ctrl-Left` 관용 표기 수용). 혼용(`Ctrl+Shift-F8`)은
    ///   `+` 분할의 base(`Shift-F8`)가 미지 이름이 되어 종전대로 배제된다.
    /// - `<KeyName>` 은 `KeyCode::from_name` 호환(대소문자 구분: `F9`/`Escape`).
    ///   `auto_english` 와 달리 `"Shift<Name>"` 융합형은 **지원하지 않는다** —
    ///   ATF 핫키는 shift 를 `Shift+` 토큰으로만 표현해 문법을 하나로 유지한다.
    /// - modifier 는 네 개 모두 **정확 일치**(등장 = 필수, 미등장 = 금지)로 저장된다.
    ///   따라서 수정자 없는 기존 표기(`F10`)는 모든 수정자가 떼어진 경우에만 매칭 —
    ///   종전 `shortcut_combo` 가드 시절과 동작이 동일하다.
    /// - 수정자 키 자체(`RightAlt`/`LeftShift` 등)를 base 로 지정하면 `None` 이다.
    ///   `press_key` 의 `is_modifier()` 조기 반환 때문에 애초에 도달하지 않는
    ///   dead key 라, 목록 단계에서 배제해 소비 판정까지 일관되게 만든다.
    ///
    /// # 환경 제약
    ///
    /// - **Windows TSF/IMM32**: 조합 표기가 Linux 와 동일하게 동작한다 — test 단계
    ///   (`OnTestKeyDown`/`should_consume`)가 `is_atf_hotkey` 의 수정자 정확-일치로
    ///   조합을 소비해 press_key 매칭까지 도달시킨다. 조합의 맨 base 키는 소비되지
    ///   않아 원 기능(한자 등)이 보존된다.
    /// - **GTK3/4**: idle(조합 없음) 상태의 F 키는 IM 모듈을 거치지 않고 앱으로
    ///   직행하는 경로가 있어, 조합 여부와 무관하게 F 키 핫키가 불발할 수 있다.
    ///
    /// # Returns
    ///
    /// - `Some(AtfHotkey)`
    /// - `None` — 미지 modifier 토큰 / 미지·빈 base / 수정자 키 base
    /// CLI(`unim-cli config set`)·설정앱이 사용자 입력 표기를 저장 전에 검증하는
    /// 단일 진실 공급원이기도 하다. 반환값이 `None` 이면 그 표기는 엔진에서 조용히
    /// 버려지므로(핫키가 죽음) 호출부는 경고를 띄워야 한다.
    pub fn parse_atf_hotkey(spec: &str) -> Option<AtfHotkey> {
        let mut ctrl = false;
        let mut alt = false;
        let mut super_key = false;
        let mut shift = false;

        // 구분자로 분할, 마지막 토큰 = base, 앞 토큰 = modifier.
        // 구분자가 없으면 tokens 는 비고 base 만 남는다 → 수정자 전부 false (legacy 동치).
        //
        // 구분자는 '+' 우선, '+' 가 전혀 없으면 '-' 폴백("Ctrl-Left" 관용 표기).
        // KeyCode 이름에는 '-' 가 없어(from_name 전수 확인) bare 이름("F8")의
        // '-' 분할은 무연산이고, 종전에 성공하던 표기는 전부 '+' 경로 그대로다.
        //
        // `parse_keyspec` 과 달리 단일 base 도 trim 한다. ATF 핫키는 `key:` 같은
        // 접두사 문법을 거치지 않는 raw 설정 문자열이고, 종전 파서
        // (`KeyCode::from_name`) 역시 공백을 허용하지 않아 `" F10"` 은 어차피
        // Unknown → 배제였다. trim 은 그 실패를 성공으로 바꿀 뿐 기존에 성공하던
        // 표기의 결과를 바꾸지 않으므로 하위호환에 영향이 없다.
        let sep = if spec.contains('+') { '+' } else { '-' };
        let mut tokens: Vec<&str> = spec.split(sep).map(str::trim).collect();
        let base = tokens.pop()?; // split 결과는 항상 1개 이상 → pop 안전
        for tok in tokens {
            if tok.eq_ignore_ascii_case("ctrl") || tok.eq_ignore_ascii_case("control") {
                ctrl = true;
            } else if tok.eq_ignore_ascii_case("alt") {
                alt = true;
            } else if tok.eq_ignore_ascii_case("super")
                || tok.eq_ignore_ascii_case("win")
                || tok.eq_ignore_ascii_case("meta")
            {
                super_key = true;
            } else if tok.eq_ignore_ascii_case("shift") {
                shift = true;
            } else {
                // 미지 modifier 토큰 → 현행 침묵 무시 정책.
                return None;
            }
        }

        // base 는 대소문자 구분(KeyCode::from_name) — `Shift+F9` OK, `Shift+f9` None.
        let code = KeyCode::from_name(base);
        if code == KeyCode::Unknown || code.is_modifier() {
            return None;
        }

        Some(AtfHotkey {
            code,
            ctrl,
            alt,
            super_key,
            shift,
        })
    }

    /// `key:` keyspec 의 base 토큰과 legacy 무접두사 분기가 공유하는 KeyCode 파서.
    ///
    /// `"Shift<Name>"` 는 Shift 필수, 문자 키(알파벳/숫자/기호)는 Shift 없을 때만,
    /// 제어 키(Escape/Tab/Enter/F*/Arrows …)는 Shift 무관으로 매핑한다.
    /// (Ctrl/Alt/Super 조합 해석은 상위 `parse_keyspec` 이 담당하며, 이 함수는
    /// base key 이름만 다룬다.)
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
    /// 활성화 + 한글 모드에서만 평가. 두 호출 지점에서 재사용된다: (1) press_key()
    /// 상위 단축키 가드 — Ctrl/Alt/Super 조합 트리거용, (2) process_korean_key() —
    /// 비조합 트리거용. modifier 정확-일치 덕에 각 트리거는 자기 지점에서만 매칭한다.
    ///
    /// - `Functional { code, shift, ctrl, alt, super_key }` — `code == keycode`
    ///   + Ctrl/Alt/Super 정확 일치 + shift 조건. 조합 트리거는 char 가드를 우회.
    /// - `Character(ch)` — 키맵 거친 산출 char 비교(Ctrl/Alt/Super 조합 시 미매칭).
    ///   한국어 자판이 비-QWERTY
    ///   (예: 세벌식390) 인 경우엔 KeyboardMap 의 `Special(ch)` 매핑까지 확인하여
    ///   해당 자판에서 산출되는 실제 문자(`'/'` 등) 와 비교한다.
    pub(super) fn match_auto_english_trigger(
        &mut self,
        keycode: KeyCode,
        modifier: ModifierState,
    ) -> Option<AutoEnglishTrigger> {
        if !self.auto_english_enabled {
            return None;
        }
        if self.input_category != InputCategory::Korean {
            return None;
        }
        // produces_char_in_korean 은 (&mut self) 라 트리거 iter 클로저 안에서 호출 불가 —
        // 결과는 (keycode, shift) 에 결정적이므로 미리 한 번 계산해 둔다.
        let produced_char = self.produces_char_in_korean(keycode, modifier.shift);
        self.auto_english_triggers
            .iter()
            .find(|t| match t {
                AutoEnglishTrigger::Functional {
                    code,
                    shift,
                    ctrl,
                    alt,
                    super_key,
                } => {
                    *code == keycode
                        // Ctrl/Alt/Super 정확 일치. legacy·비조합 트리거는 세 값이 전부
                        // false 이므로, modifier 가 전부 false 인 비조합 press flow 지점
                        // (process_korean_key)에서 종전 비교와 완전 동치다. 조합 트리거
                        // (`key:Ctrl+B`)는 press_key 상위 가드 경로에서만 여기 도달한다.
                        && *ctrl == modifier.control
                        && *alt == modifier.alt
                        && *super_key == modifier.super_key
                        && match shift {
                            None => true,
                            Some(required) => *required == modifier.shift,
                        }
                        // 문자 키 char 가드 — 단, modifier 조합 트리거는 우회한다.
                        // 비조합 문자 키(`/`/`:` 등)는 한국어 모드에서 실제 char 가
                        // 산출될 때만 트리거한다. `produces_char_in_korean` 이 None 이면
                        // 자모 경로(예: 세벌식390 `slash_context_alt` 의 ㅗ) 가 활성이므로
                        // Functional 트리거를 양보하고 자모 흐름을 타게 한다. 제어 키
                        // (Escape/Tab 등)는 `is_character_key()=false` 라 우회한다.
                        // Ctrl/Alt/Super 조합 트리거(`key:Ctrl+B`)는 base 가 한글 자모
                        // (두벌식 B=ㅠ 등)여도 발동해야 하므로 char 가드를 건너뛴다.
                        && (*ctrl
                            || *alt
                            || *super_key
                            || !keycode.is_character_key()
                            || produced_char.is_some())
                }
                // Character 는 비조합 전용. Ctrl/Alt/Super 가 눌린 조합에서는 매칭하지
                // 않아, 가드 경로에서 `char:/` 가 `Ctrl+/` 에 새로 발동하는 오동작을
                // 차단한다. 비조합 press flow 지점에선 세 값이 항상 false 라 무변화.
                AutoEnglishTrigger::Character(ch) => {
                    !modifier.control
                        && !modifier.alt
                        && !modifier.super_key
                        && produced_char == Some(*ch)
                }
            })
            .copied()
    }

    /// 한국어 모드에서 `(keycode, shift)` 가 어떤 char 를 산출할지 계산한다.
    ///
    /// 산출 경로:
    /// 1. `english_keymap.get_char` 로 영문 자판의 char 를 얻는다 (e.g. `'G'`).
    /// 2. `key_meta.context_alt` 가 있으면 현재 preedit 상태로 분기 평가.
    ///    - 조건 충족(jamo 경로 활성) → `None` (자모 산출 예정).
    ///    - 조건 불충족(fallback 경로) → `Some(fallback 첫 char)` (e.g. 세벌식390 `/`).
    /// 3. KoreanLayout 의 `keyboard_map` 에서 영문 char 를 lookup.
    ///    - `Some(JamoEnum::Special(ch))` 면 그 자판이 의도한 비-자모 char.
    ///    - `Some(JamoEnum::Cho/Jung/Jong)` 면 한글 자모이므로 char 산출 없음 → `None`.
    ///    - 매핑이 없으면(QWERTY std 에서 기호 등) 영문 char 를 그대로 사용.
    /// 4. `keyboard_map` 자체가 없으면 영문 char 를 그대로 사용.
    fn produces_char_in_korean(&mut self, keycode: KeyCode, shift: bool) -> Option<char> {
        let en_ch = self.english_keymap.get_char(keycode, shift)?;

        // key_meta.context_alt 가 있으면 process_korean_key 의 룰 B 분기와 동일하게 평가.
        // 한글 모드에서 fallback 경로를 타는 키(예: 세벌식390 의 빈 preedit `/`)는
        // 사용자에게 실제로 char 가 commit 되므로 자동 영문 트리거 후보가 되어야 한다.
        if let Some(meta) = self.key_meta_map.get(&en_ch) {
            if let Some(alt) = meta.context_alt.as_ref() {
                use crate::keystroke::profile::ContextCondition as C;
                let cond_ok = match alt.when {
                    C::Empty => !self.korean_context.is_composing(),
                    C::Composing => self.korean_context.is_composing(),
                    C::ChoseongOnly => self.korean_context.is_only_cho_filled(),
                    C::JungseongOnly => self.korean_context.is_only_jung_filled(),
                    C::ChoJungFilled => self.korean_context.is_cho_jung_filled(),
                    C::JongseongFilled => self.korean_context.is_jong_filled(),
                    C::LastIsCho => self.korean_context.last_jamo_is_cho(),
                    C::LastIsJung => self.korean_context.last_jamo_is_jung(),
                    C::LastIsJong => self.korean_context.last_jamo_is_jong(),
                };
                return if cond_ok {
                    None
                } else {
                    alt.fallback.chars().next()
                };
            }
        }

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
    ///
    /// 단어 모드(`accumulate_word`)에서는 누적된 단어 전체(`word_buffer` + 현재 음절)를
    /// 표시해야 하므로 `get_preedit_display()` 를 사용한다. syllable 모드는
    /// `word_buffer` 가 항상 비어 있어 display == preedit 이라 동작 무변경.
    pub(super) fn update_preedit_cache(&mut self) {
        self.preedit_cache = self.korean_context.get_preedit_display();
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

    /// chord 버퍼의 현재 부분 결합 결과를 preedit 에 미리 inject 한다.
    ///
    /// chord 진행 중에도 화면에는 결합 진행 상황이 즉시 보이도록 하기 위한 핵심 경로다.
    /// `chord_buffer.push_*` 가 `None` (윈도우 내 누적) 을 반환했을 때 호출되며, 만료된
    /// 이전 chord 의 잔여 처리 직후 새 chord 의 첫 키에 대해서도 호출된다.
    ///
    /// ## 단일 키 vs 다중 키 분기 (Atomic Window Principle 의 preview 변형)
    /// - **단일 키 (len == 1)** — sequential 풀어쓰기 경로. `process_jamo_with_meta` 로
    ///   기존 컴포저 상태(직전 preedit) 와 결합 시도. ㄱ 단독 chord 후 ㅏ 단독 chord 가
    ///   들어와도 일반 한글 조합과 동일하게 "가" 로 합쳐진다. `preview_is_atomic = false`.
    /// - **다중 키 (len >= 2)** — atomic 모아쓰기 경로. `compose_chord` + `inject_chord_syllable`.
    ///   `preview_is_atomic = true`.
    ///
    /// ## 다중 키 진입 시 직전 preview 처리
    /// - 직전이 atomic preview 였다면 `inject_chord_syllable` 자체가 큐를 비우고 재구성하므로
    ///   추가 정리 불필요 (mid-chord 갱신).
    /// - 직전이 sequential preview 였다면 `process_jamo_with_meta` 가 컴포저에 자모 1개를
    ///   추가한 상태이므로 `pop_last_jamo` 로 그 자모를 되돌린 뒤 `flush_preedit` 으로
    ///   직전 음절을 commit, 그 후 atomic chord 를 inject.
    /// - 직전 preview 없음(첫 atomic) — `flush_preedit` 으로 무관한 직전 조합을 commit 후 inject.
    ///
    /// ## Case B/C (다중 키 결합 실패 또는 비자모 포함)
    /// 직전 preview 가 atomic 이었다면 `clear_composing` 으로 폐기. sequential 이었다면
    /// `pop_last_jamo` 로 1개 자모만 되돌리고 prior 상태 유지. 만료(idle) 시
    /// `apply_chord_entries` 가 fallback/비자모를 처리한다.
    ///
    /// idle / force flush 시 `preview_injected == true` 이면 호출자는 `apply_chord_entries`
    /// 를 다시 호출하지 않아 동일 음절 중복 commit 을 막는다.
    pub(super) fn update_chord_preview(&mut self) {
        let entries: Vec<crate::input_engine::chord_compose::ChordEntry> =
            self.chord_buffer.peek_entries().to_vec();
        if entries.is_empty() {
            self.update_preedit_cache();
            return;
        }

        let was_preview_active = self.chord_buffer.was_preview_injected();
        let was_atomic = self.chord_buffer.was_preview_atomic();

        if entries.len() == 1 {
            // 단일 키 chord — sequential 결합 경로.
            // 직전 컴포저 상태(이전 chord 결과 또는 sequential 누적)와 그대로 결합.
            let entry = &entries[0];
            match entry.kind {
                ChordEntryKind::Jamo(jamo) => {
                    self.korean_context.process_jamo_with_meta(jamo, entry.meta);
                    let committed = self.korean_context.get_committed();
                    if !committed.is_empty() {
                        self.commit_buffer.push_str(committed);
                        self.korean_context.clear_committed();
                    }
                    self.chord_buffer.mark_preview_injected(true);
                    self.chord_buffer.mark_preview_atomic(false);
                }
                ChordEntryKind::NonJamo(c) => {
                    // 단일 비자모 — 직전 조합 commit + 자체 commit (preview 없음).
                    self.flush_preedit();
                    self.commit_buffer.push(c);
                    self.chord_buffer.mark_preview_injected(false);
                    self.chord_buffer.mark_preview_atomic(false);
                }
            }
        } else {
            // 다중 키 chord — atomic 모아쓰기 경로.
            let combined_jamo = self.korean_context.get_combined_jamo();
            let result = compose_chord(&entries, combined_jamo);
            let case_a = result.inject_to_preedit && result.non_jamos.is_empty();

            if case_a {
                if was_preview_active && !was_atomic {
                    // sequential→atomic 전이: 직전 sequential 자모 1개를 되돌리고 prior commit.
                    self.korean_context.pop_last_jamo();
                }
                if !was_atomic {
                    // sequential→atomic 또는 첫 atomic 진입 — prior 조합을 commit.
                    // (atomic→atomic mid-chord 갱신은 inject_chord_syllable 이 자체적으로 큐를 재구성)
                    self.flush_preedit();
                }
                self.korean_context.inject_chord_syllable(
                    result.cho,
                    result.jung,
                    result.jong,
                    result.input_order_jamos,
                );
                self.chord_buffer.mark_preview_injected(true);
                self.chord_buffer.mark_preview_atomic(true);
            } else if was_preview_active {
                // Case A → B/C 전이: 직전 preview 폐기.
                if was_atomic {
                    self.korean_context.clear_composing();
                } else {
                    // sequential 상태에서 결합 실패 — 추가된 자모 1개만 되돌림.
                    self.korean_context.pop_last_jamo();
                }
                self.chord_buffer.mark_preview_injected(false);
                self.chord_buffer.mark_preview_atomic(false);
            }
        }

        self.update_preedit_cache();
    }

    /// chord 버퍼를 강제 비우고, preview 가 이미 inject 된 상태면 중복 처리 회피.
    ///
    /// `apply_chord_entries` 를 직접 호출하면 Case A 의 `flush_preedit + inject_chord_syllable`
    /// 가 preview 와 중복되어 동일 음절이 commit 에 두 번 들어간다. 이 헬퍼는 그 함정을
    /// 모든 호출자(Backspace/Enter/Tab/Space/Hanja/Special/비-문자키 등)에서 차단한다.
    pub(super) fn finalize_chord_buffer(&mut self) {
        let preview_was_injected = self.chord_buffer.was_preview_injected();
        let Some(entries) = self.chord_buffer.force_flush() else {
            return;
        };
        if !preview_was_injected {
            self.apply_chord_entries(entries);
        }
        // preview_injected == true: korean_context 가 이미 chord 음절을 들고 있음.
        // entries 는 폐기 — 호출자는 이후 flush_preedit / process_backspace 등으로 마무리.
    }

    /// chord 버퍼에서 꺼낸 항목 목록을 compositor에 적용한다 (Phase 3 — chord_compose 통합).
    ///
    /// ## 분기 원칙 (Atomic Window Principle)
    /// - 1키: 풀어쓰기 경로 — process_jamo_with_meta / NonJamo 즉시 commit.
    ///   composer 의 inline retry 블록이 시간차 자모 결합 처리.
    /// - 2+키: 모아쓰기 경로 — compose_chord() 로 영역별 결합 후 apply_chord_result().
    ///
    /// commit_buffer 갱신 포함. preedit_cache 갱신은 **호출자가** 필요 시 수행.
    pub(super) fn apply_chord_entries(&mut self, entries: ChordFlushResult) {
        if entries.is_empty() {
            return;
        }

        if entries.len() == 1 {
            // ── 1키 분기: 풀어쓰기 (기존 직렬 경로, retry 블록 활성)
            let entry = &entries[0];
            match entry.kind {
                ChordEntryKind::Jamo(jamo) => {
                    self.korean_context.process_jamo_with_meta(jamo, entry.meta);
                    let committed = self.korean_context.get_committed();
                    if !committed.is_empty() {
                        self.commit_buffer.push_str(committed);
                        self.korean_context.clear_committed();
                    }
                }
                ChordEntryKind::NonJamo(c) => {
                    self.flush_preedit();
                    self.commit_buffer.push(c);
                }
            }
        } else {
            // ── 2+키 분기: 모아쓰기 (chord_compose 경로)
            let combined_jamo = self.korean_context.get_combined_jamo();
            let result = compose_chord(&entries, combined_jamo);
            // Case B (모아쓰기 실패 + 비자모 없음): fallback commit 대신 sequential push
            // → composer inline retry 가 시간차 결합 시도, 결과는 preedit 으로 유지.
            // (사용자 결정: chord_window 만료 = preedit 갱신만, commit 안 함.
            //  단 Case A 는 inject_chord_syllable, Case C 는 비자모 강제 종결 그대로.)
            if !result.inject_to_preedit && result.non_jamos.is_empty() {
                let mut sorted = entries.clone();
                sorted.sort_by_key(|e| e.input_order);
                for entry in &sorted {
                    if let ChordEntryKind::Jamo(jamo) = entry.kind {
                        self.korean_context.process_jamo_with_meta(jamo, entry.meta);
                        let committed = self.korean_context.get_committed();
                        if !committed.is_empty() {
                            self.commit_buffer.push_str(committed);
                            self.korean_context.clear_committed();
                        }
                    }
                }
            } else {
                self.apply_chord_result(result);
            }
        }

        self.update_preedit_cache();
    }

    /// chord_compose 결과를 통일 원칙 표에 따라 엔진 상태에 적용한다.
    ///
    /// | 케이스 | 처리 |
    /// |---|---|
    /// | inject_to_preedit=true, non_jamos 비어있음 | preedit inject (inject_chord_syllable) |
    /// | inject_to_preedit=false, non_jamos 비어있음 | fallback_jamos commit |
    /// | non_jamos 있음 | 자모 부분 처리 후 non_jamos commit |
    fn apply_chord_result(&mut self, result: ChordResult) {
        if result.inject_to_preedit && result.non_jamos.is_empty() {
            // Case A: 전 영역 reduce 성공 + 비자모 없음 → preedit inject
            // 호출 전 기존 preedit 확정 (flush_preedit 는 composing 시에만 동작)
            self.flush_preedit();
            self.korean_context.inject_chord_syllable(
                result.cho,
                result.jung,
                result.jong,
                result.input_order_jamos,
            );
            // inject 후 committed 에 새로 들어간 것은 없음 (preedit 상태로 유지)
        } else if result.non_jamos.is_empty() {
            // Case B: 자모 결합 실패 + 비자모 없음 → 호환자모 commit
            self.flush_preedit();
            for c in &result.fallback_jamos {
                self.commit_buffer.push(*c);
            }
        } else {
            // Case C: 비자모 포함 → 자모 부분 처리 후 비자모 commit
            self.flush_preedit();
            if result.inject_to_preedit {
                // 자모 결합 성공 — syllable 을 inject 한 뒤 즉시 flush (commit 로 변환).
                // Case C 는 즉시 commit 되므로 input_order 추적 불필요 (빈 Vec 전달).
                self.korean_context.inject_chord_syllable(
                    result.cho,
                    result.jung,
                    result.jong,
                    Vec::new(),
                );
                self.flush_preedit();
            } else {
                // 자모 결합 실패 — 호환자모 commit
                for c in &result.fallback_jamos {
                    self.commit_buffer.push(*c);
                }
            }
            // 비자모 commit (입력 순서 보존)
            for c in &result.non_jamos {
                self.commit_buffer.push(*c);
            }
        }
    }

    /// 입력 카테고리를 토글합니다.
    pub(super) fn toggle_input_category(&mut self) {
        // chord 버퍼 남은 자모 finalize 후 조합 커밋 (preview 이미 반영 시 중복 commit 회피)
        self.finalize_chord_buffer();
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
