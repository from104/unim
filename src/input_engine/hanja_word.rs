//! 한자 단어 입력 — 최근 확정 음절 버퍼 + 변환 대상 결정 (HANJA_WORD_SPEC).
//!
//! - 버퍼: `press_key` 래퍼(`recent_track_after_key`)가 키마다 커밋 델타를 흡수하고
//!   리셋 조건(비한글 커밋·통과 키·한/영 전환·팝업·비밀번호)에서 비운다(§2.2.1).
//! - 대상 결정: `resolve_hanja_target` — 대상①(버퍼+preedit / 단어 모드 preedit 의 최장
//!   사전 접미) → 종전(마지막 음절), idle 이면 대상②(앱 선택 영역)(§2.2.2·§2.3·§2.4).
//! - 팝업 중 한자키 재타 = target 접미 축소(`shrink_hanja_target`, Q7(a)).

use super::engine::InputEngine;
use super::types::{HanjaSource, InputResult, HANJA_MAX_KEY_CHARS, RECENT_SYLLABLE_CAP};
use crate::config::InputCategory;
use crate::hangul::char::HangulCharExt;
use crate::keycode::{KeyCode, ModifierState};
use crate::unim_log;

/// 변환 대상 결정 결과.
pub(super) enum Resolve {
    /// 팝업을 띄울 대상.
    Target(HanjaTargetSpec),
    /// idle + 앱 선택 영역이 있으나 사전 정확 일치 한글이 아님 — 호출부는 종전 idle
    /// 동작(이모지 팝업)으로 폴백한다(Q8(a)). 한자 팝업은 띄우지 않는다.
    NoMatchSelection,
    /// 대상 없음(비밀번호 차단·idle 선택 없음·조합 중 preedit 비어 있음).
    None,
}

/// 한자 팝업 대상 명세 — 진입 시 확정돼 팝업 내내 불변(축소 키 제외).
pub(super) struct HanjaTargetSpec {
    /// 사전 키 = 헤더 = 즐겨찾기 키.
    pub(super) key: String,
    pub(super) source: HanjaSource,
    /// 확정 접두 길이 (`RecentWord` 만 > 0).
    pub(super) committed: u32,
    /// 취소 시 재커밋 텍스트 (= 진입 시 preedit 전체, 대상② 는 "").
    pub(super) recommit: String,
    /// 확정 문자열 앞/뒤에 그대로 붙일 문자열.
    pub(super) prefix: String,
    pub(super) suffix: String,
}

impl InputEngine {
    // =========================================
    // 최근 확정 음절 버퍼
    // =========================================

    /// 최근 확정 음절 버퍼를 비웁니다.
    pub fn recent_clear(&mut self) {
        self.recent_syllables.clear();
    }

    /// 버퍼 끝 1자를 제거합니다 (Backspace 통과 — 앱이 커서 앞 1자를 지웠다).
    pub(super) fn recent_pop(&mut self) {
        self.recent_syllables.pop();
    }

    /// 커밋된 문자 1개를 버퍼에 반영합니다.
    ///
    /// 한글 완성 음절이면 뒤에 붙이고(상한 초과 시 앞에서 버림), 그 외 문자는 문맥을
    /// 끊으므로 버퍼를 비운다. 비밀번호/PIN 차단 중이면 push 대신 비운다 — chord idle
    /// 타이머 경로는 키 래퍼를 거치지 않으므로 훅 자체가 fail-closed 여야 한다(§2.8).
    ///
    /// `pub`: 데몬의 AutoTypeFix 순방향 교정 뒤 버퍼 시드(외부 크레이트)가 호출한다.
    pub fn recent_push_char(&mut self, c: char) {
        if self.content_purpose.should_block_hangul() {
            self.recent_clear();
            return;
        }
        if !c.is_hangul_syllable() {
            self.recent_clear();
            return;
        }
        if self.recent_syllables.chars().count() >= RECENT_SYLLABLE_CAP {
            let n = self.recent_syllables.chars().next().map(char::len_utf8).unwrap_or(0);
            self.recent_syllables.drain(..n);
        }
        self.recent_syllables.push(c);
    }

    /// `commit_buffer[recent_mark..]` 의 문자를 버퍼에 흡수하고 mark 를 끝으로 전진한다.
    ///
    /// 래퍼 (4) 와 한자키 분기(chord `finalize_chord_buffer()` 가 같은 키 안에서 확정한
    /// 음절 "민" 을 `start_hanja_conversion()` 전에 풀에 넣기 위해)가 같은 헬퍼를 쓴다.
    /// mark 이후만 보므로 한 키 안에서 여러 번 불려도 이중 push 가 없다.
    pub(super) fn recent_absorb_commit_delta(&mut self) {
        let mark = self.recent_mark.min(self.commit_buffer.len());
        let appended: Vec<char> = self
            .commit_buffer
            .get(mark..)
            .map(|s| s.chars().collect())
            .unwrap_or_default();
        for c in appended {
            self.recent_push_char(c);
        }
        self.recent_mark = self.commit_buffer.len();
    }

    /// `press_key` 래퍼의 후처리 — 이번 키의 결과로 버퍼를 갱신한다(§2.2.1 리셋 조건표).
    pub(super) fn recent_track_after_key(
        &mut self,
        keycode: KeyCode,
        modifier: ModifierState,
        r: &InputResult,
        before_len: usize,
        was_popup: bool,
        cat_before: InputCategory,
    ) {
        // (0) 비밀번호/PIN: fail-closed — 채우지 않는다.
        if self.content_purpose.should_block_hangul() {
            self.recent_clear();
            self.recent_mark = self.commit_buffer.len();
            return;
        }
        // (1) commit_buffer 가 줄었다 = 내부 reset 등 → 스냅샷 무효.
        // (2) 팝업 중 키(확정/취소/미지원 키 재처리/내비) — 팝업 공통 리셋.
        // (3) 한/영 전환(토글키·auto-english·AutoTypeFix).
        if self.commit_buffer.len() < before_len || was_popup || self.input_category != cat_before
        {
            self.recent_clear();
            self.recent_mark = self.commit_buffer.len();
            return;
        }
        // (4) 커밋 델타 흡수 — mark 이후만(한자키 분기의 중간 흡수와 이중 push 방지).
        let appended_any = self.commit_buffer.len() > before_len;
        self.recent_absorb_commit_delta();
        // (5) 앱으로 통과한 키 / 단축키 조합.
        if keycode.is_modifier() {
            return; // 수정자 단독 키는 리셋하지 않는다
        }
        if modifier.control || modifier.alt || modifier.super_key {
            self.recent_clear();
            return;
        }
        if !r.consumed {
            if keycode == KeyCode::Backspace
                && !appended_any
                && !self.korean_context.is_composing()
            {
                // 앱이 커서 앞 1자를 지웠다. 선택 영역이 있었으면 그 전체가 지워졌다.
                if self.surrounding_cursor != self.surrounding_anchor {
                    self.recent_clear();
                } else {
                    self.recent_pop();
                }
            } else {
                self.recent_clear();
            }
        }
    }

    // =========================================
    // 접근자
    // =========================================

    /// 취소·포커스 이탈 시 앱에 되돌려 줄 텍스트(= 팝업 진입 당시 preedit 전체,
    /// 대상② 는 빈 문자열). 한자 모드가 아니면 빈 문자열.
    ///
    /// `hanja_target` 만 세팅된 외부 조립 상태(`Syllable` + 재커밋 미설정)는 종전대로
    /// target 을 돌려준다 — 종전 `get_hanja_target()` 재커밋과 바이트 동일.
    pub fn hanja_cancel_text(&self) -> String {
        if !self.hanja_mode {
            return String::new();
        }
        if self.hanja_recommit.is_empty() && self.hanja_source == HanjaSource::Syllable {
            return self.hanja_target.clone();
        }
        self.hanja_recommit.clone()
    }

    /// 현재 한자 팝업 대상의 출처.
    pub fn hanja_source(&self) -> HanjaSource {
        self.hanja_source
    }

    /// 현재 한자 팝업의 확정 접두 길이(대상① `RecentWord` 만 > 0).
    pub fn hanja_committed_chars(&self) -> u32 {
        self.hanja_committed_chars
    }

    /// 최근 확정 음절 버퍼(진단·테스트용).
    pub fn recent_syllables(&self) -> &str {
        &self.recent_syllables
    }

    /// 설정된 한자키(`hanja_keys`)인지 판정합니다.
    pub fn is_hanja_key(&self, keycode: KeyCode) -> bool {
        self.hanja_keys.contains(&keycode)
    }

    /// 앱 선택 영역이 있는지(대상② 후보). 비밀번호 차단 중이면 false.
    pub fn has_selection(&self) -> bool {
        !self.content_purpose.should_block_hangul()
            && !self.surrounding_text.is_empty()
            && self.selection_span_inner(false).is_some()
    }

    /// 한자 단어 대상 필드를 기본값으로 되돌린다(교체 페이로드는 호스트가 drain 하므로
    /// 건드리지 않는다).
    pub(super) fn reset_hanja_word_fields(&mut self) {
        self.hanja_source = HanjaSource::Syllable;
        self.hanja_committed_chars = 0;
        self.hanja_recommit.clear();
        self.hanja_commit_prefix.clear();
        self.hanja_commit_suffix.clear();
    }

    // =========================================
    // 대상 결정
    // =========================================

    /// 한자 변환 대상을 결정한다(§2.2.2·§2.3·§2.4).
    pub(super) fn resolve_hanja_target(&self) -> Resolve {
        // pull 경로(GetHanjaCandidates)는 press_key 의 영문 강제를 거치지 않는다 — 여기서 차단.
        if self.content_purpose.should_block_hangul() {
            return Resolve::None;
        }
        let pre: Vec<char> = self.preedit_cache.chars().collect();
        let p = pre.len();
        if p == 0 && !self.korean_context.is_composing() {
            return self.selection_target(); // 대상②
        }
        let Some(&last) = pre.last() else {
            return Resolve::None;
        };
        let pre_s = self.preedit_cache.clone();
        let word_mode = self.is_word_mode();
        let spec_last = |source| HanjaTargetSpec {
            key: last.to_string(),
            source,
            committed: 0,
            recommit: pre_s.clone(),
            prefix: pre[..p - 1].iter().collect(),
            suffix: String::new(),
        };
        // 미완성 자모 → 종전(마지막 글자, 초성 특수문자 폴백은 start_hanja_conversion).
        if !last.is_hangul_syllable() {
            return Resolve::Target(spec_last(if word_mode {
                HanjaSource::WordBuffer
            } else {
                HanjaSource::Syllable
            }));
        }
        let pool: Vec<char> = if word_mode {
            pre.clone()
        } else {
            self.recent_syllables.chars().chain(pre.iter().copied()).collect()
        };
        let n = pool.len();
        let mut recent_untrusted = false;
        for l in (2..=n.min(HANJA_MAX_KEY_CHARS)).rev() {
            // 버퍼 글자를 포함하는 접미(l > p)는 교체 채널을 드레인하는 호스트에서만,
            // 그리고 접두 검증이 한 번 실패하면 더 짧은 버퍼 포함 접미도 믿지 않는다.
            if l > p && (!self.hanja_word_replace_capable || recent_untrusted) {
                continue;
            }
            let suf = &pool[n - l..];
            if !suf.iter().all(|c| c.is_hangul_syllable()) {
                continue;
            }
            let key: String = suf.iter().collect();
            if !self.hanja_dict.contains(&key) {
                continue;
            }
            if l > p {
                // 대상① 음절 모드 — 이미 앱에 확정된 접두(l - p 자)를 교체한다.
                let prefix: String = pool[n - l..n - p].iter().collect();
                if !self.recent_prefix_verified(&prefix, &pre_s) {
                    recent_untrusted = true;
                    continue;
                }
                return Resolve::Target(HanjaTargetSpec {
                    key,
                    source: HanjaSource::RecentWord,
                    committed: (l - p) as u32,
                    recommit: pre_s,
                    prefix: String::new(),
                    suffix: String::new(),
                });
            }
            // 단어 모드 / preedit 내부 일치 — 접미 앞 preedit 은 커밋 접두로 보존.
            return Resolve::Target(HanjaTargetSpec {
                key,
                source: HanjaSource::WordBuffer,
                committed: 0,
                recommit: pre_s.clone(),
                prefix: pre[..p - l].iter().collect(),
                suffix: String::new(),
            });
        }
        // 종전(마지막 음절). 단어 모드는 접두를 보존한다.
        Resolve::Target(spec_last(if word_mode {
            HanjaSource::WordBuffer
        } else {
            HanjaSource::Syllable
        }))
    }

    /// 지울 확정 접두(`prefix`)가 앱 문서의 커서 앞과 맞는지 대조한다(§2.2.3 안전망).
    ///
    /// - surrounding 을 한 번도 받지 않은 컨텍스트(XIM 등)는 검증 생략(통과).
    /// - 받은 적 있는 컨텍스트에서 빈 surrounding 은 실패(Wayland 미수신 마커, Q9(b)).
    /// - 커서 오프셋 초과(단위 불일치)·커서 앞 빈 문자열은 실패(fail-closed).
    /// - `prefix+pre` 절은 surrounding 이 조합 텍스트를 포함하는 호스트(TSF)에서만.
    fn recent_prefix_verified(&self, prefix: &str, pre: &str) -> bool {
        if self.surrounding_text.is_empty() {
            return !self.surrounding_seen;
        }
        let n = self.surrounding_text.chars().count();
        let cur = self.surrounding_cursor as usize;
        if cur > n {
            return false; // 오프셋 초과 = 단위 불일치 → 클램프 금지
        }
        let before: String = self.surrounding_text.chars().take(cur).collect();
        if before.is_empty() {
            return false; // "" 는 모든 문자열의 접미 — 잘림 절 공허 통과 금지
        }
        // GTK/Qt/Wayland/GNOME: surrounding 은 preedit 을 포함하지 않는다.
        if before.ends_with(prefix) || prefix.ends_with(before.as_str()) {
            return true;
        }
        // Wayland/GNOME 클릭 드리프트("…대한민국" 뒤 클릭 + "국")에서 full 절이 뚫리지 않게.
        if !self.surrounding_includes_preedit {
            return false;
        }
        let full = format!("{prefix}{pre}");
        before.ends_with(&full) || full.ends_with(before.as_str())
    }

    /// 선택 구간 `(a, b)` (문자 단위, a < b).
    ///
    /// `max(cursor, anchor)` 가 문자 길이를 넘으면 단위 불일치로 **거부**(None + 로그) —
    /// 클램프하면 바이트 오프셋 "대한민국" 의 "대한"(0,6) 이 (0,4) 로 삼켜져 target
    /// "대한민국" 팝업 → "大韓民國민국" 이 된다(§2.4.1). a ≥ b 도 None.
    pub(super) fn selection_span(&self) -> Option<(usize, usize)> {
        self.selection_span_inner(true)
    }

    /// `selection_span` 과 같은 판정이되 초과 로그를 남기지 않는다. idle 한자키는
    /// `has_selection` → `selection_target` 으로 한 키에 두 번 판정하므로, 로그는
    /// 실제로 선택을 쓰는 쪽에서만 1회 남긴다(§2.4.1).
    fn selection_span_inner(&self, log: bool) -> Option<(usize, usize)> {
        let n = self.surrounding_text.chars().count();
        let hi = self.surrounding_cursor.max(self.surrounding_anchor) as usize;
        if hi > n {
            if log {
                unim_log!("ENGINE", "surrounding 오프셋 초과({} > {}자) — 선택 무시", hi, n);
            }
            return None;
        }
        let a = self.surrounding_cursor.min(self.surrounding_anchor) as usize;
        (a < hi).then_some((a, hi))
    }

    /// 대상② — 앱 선택 영역(앞뒤 공백 제외)이 전부 완성 음절·18자 이하·사전 정확 일치일
    /// 때만 target(§2.4.1). 앞뒤 공백은 커밋 접두/접미로 되붙인다(§2.4.3, Q3).
    fn selection_target(&self) -> Resolve {
        if self.content_purpose.should_block_hangul() {
            return Resolve::None;
        }
        let Some((a, b)) = self.selection_span() else {
            return Resolve::None;
        };
        let chars: Vec<char> = self.surrounding_text.chars().collect();
        let sel = &chars[a..b];
        let lead = sel.iter().take_while(|c| c.is_whitespace()).count();
        if lead == sel.len() {
            return Resolve::NoMatchSelection; // 공백만
        }
        let trail = sel.iter().rev().take_while(|c| c.is_whitespace()).count();
        let core = &sel[lead..sel.len() - trail];
        if core.len() > HANJA_MAX_KEY_CHARS || !core.iter().all(|c| c.is_hangul_syllable()) {
            return Resolve::NoMatchSelection;
        }
        let key: String = core.iter().collect();
        if !self.hanja_dict.contains(&key) {
            return Resolve::NoMatchSelection;
        }
        Resolve::Target(HanjaTargetSpec {
            key,
            source: HanjaSource::Selection,
            committed: 0,
            recommit: String::new(),
            prefix: sel[..lead].iter().collect(),
            suffix: sel[sel.len() - trail..].iter().collect(),
        })
    }

    /// 팝업 중 한자키 재타 — target 접미를 1자 이상 줄여 팝업을 다시 띄운다(Q7(a)).
    ///
    /// 대상①(`RecentWord`/`WordBuffer`)이고 target 이 2자 이상일 때만. 더 짧은 접미 가운데
    /// 사전에 있는 가장 긴 것(1자는 마지막 음절)으로 옮긴다 — "대한민국" → (한민국 없음)
    /// → "민국" → "국". 옮길 대상이 없거나(길이 1·대상②·후보 없음) 조건이 아니면 `None`
    /// 을 돌려 호출부가 현행 경로(팝업 미지원 키 재처리)로 간다.
    ///
    /// 확정 접두가 줄어도 남은 접두는 원래 접두의 접미라 §2.2.3 검증을 그대로 만족한다.
    pub(super) fn shrink_hanja_target(&mut self) -> Option<InputResult> {
        if !self.hanja_mode
            || !matches!(self.hanja_source, HanjaSource::RecentWord | HanjaSource::WordBuffer)
        {
            return None;
        }
        let key: Vec<char> = self.hanja_target.chars().collect();
        let l = key.len();
        if l < 2 {
            return None;
        }
        let pre: Vec<char> = self.hanja_recommit.chars().collect();
        let p = pre.len();
        let committed = self.hanja_committed_chars as usize;
        // pool = 확정 접두 + 진입 시 preedit (단어 모드/내부 일치는 preedit 그대로).
        let pool: Vec<char> = key[..committed.min(l)].iter().chain(pre.iter()).copied().collect();
        let n = pool.len();
        let word_mode = self.is_word_mode();
        for l2 in (1..l).rev() {
            if l2 > n {
                continue;
            }
            let new_key: String = pool[n - l2..].iter().collect();
            if l2 >= 2 && !self.hanja_dict.contains(&new_key) {
                continue;
            }
            let candidates = self.hanja_dict.search(&new_key);
            if candidates.is_empty() {
                continue;
            }
            let (source, committed2, prefix) = if l2 > p {
                (HanjaSource::RecentWord, (l2 - p) as u32, String::new())
            } else {
                let src = if l2 >= 2 || word_mode {
                    HanjaSource::WordBuffer
                } else {
                    HanjaSource::Syllable
                };
                (src, 0, pre[..p - l2].iter().collect())
            };
            let spec = HanjaTargetSpec {
                key: new_key,
                source,
                committed: committed2,
                recommit: self.hanja_recommit.clone(),
                prefix,
                suffix: String::new(),
            };
            unim_log!("ENGINE", "한자 대상 축소: {}자 -> {}자", l, l2);
            self.open_hanja_popup(spec, candidates);
            return Some(InputResult::hanja_candidates());
        }
        None
    }
}
