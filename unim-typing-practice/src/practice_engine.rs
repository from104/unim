//! 타자 연습 코어 — ASCII→자모 매핑, 한글 합성 시뮬레이션, WPM/정확도 통계.
//!
//! 외부 입력은 `handle_keypress(byte, shift)` 한 함수로 들어온다 — GTK
//! EventControllerKey에서 받은 keyval을 호출자가 byte로 변환해 넘긴다.
//! 본 모듈은 GTK 의존이 없어 단위 테스트 가능.

use std::collections::HashMap;
use std::time::Instant;

use unim::hangul::composer_with_2bul::HangulComposer2Bul;
use unim::hangul::composer_with_3bul::HangulComposer3Bul;
use unim::hangul::HangulComposer;
use unim::keystroke::profile::{LayoutProfile, LayoutRows};

use unim_keymap_common::keyboard_widget::KeyStat;

#[derive(Debug, Clone, Copy)]
pub enum ComposerKind {
    /// 비-한글(영문/Dvorak 등) — 키를 그대로 echo.
    Plain,
    Hangul2Bul,
    Hangul3Bul,
}

impl ComposerKind {
    pub fn from_profile(p: &LayoutProfile) -> Self {
        match p.layout_type.as_str() {
            "2bul" => ComposerKind::Hangul2Bul,
            "3bul" | "anmatae" | "moachigi_3bul" => ComposerKind::Hangul3Bul,
            _ => ComposerKind::Plain,
        }
    }
}

#[derive(Debug, Default, Clone, Copy)]
pub struct Stats {
    pub total_input_chars: u32,
    pub correct_chars: u32,
    pub error_chars: u32,
    pub elapsed_secs: f64,
}

impl Stats {
    pub fn wpm(&self) -> f64 {
        if self.elapsed_secs < 1.0 {
            return 0.0;
        }
        (self.correct_chars as f64 / 5.0) / (self.elapsed_secs / 60.0)
    }
    pub fn cpm(&self) -> f64 {
        if self.elapsed_secs < 1.0 {
            return 0.0;
        }
        self.correct_chars as f64 / (self.elapsed_secs / 60.0)
    }
    pub fn accuracy(&self) -> f64 {
        if self.total_input_chars == 0 {
            return 100.0;
        }
        (self.correct_chars as f64 / self.total_input_chars as f64) * 100.0
    }
    pub fn error_rate(&self) -> f64 {
        100.0 - self.accuracy()
    }
}

#[derive(Debug, Clone, Copy)]
#[allow(dead_code)] // `committed`는 후속 phase에서 색상 분기에 사용 예정.
pub struct StepOutcome {
    /// 지문과 비교한 결과 — 한 글자가 새로 commit되어 정답 여부가 결정된 경우.
    pub committed: Option<bool>,
    /// 진행률 (0.0..=1.0).
    pub progress: f64,
    /// 연습 완료 여부.
    pub done: bool,
}

pub struct PracticeSession {
    target: Vec<char>,
    /// 지문 cursor — 다음에 입력해야 할 글자.
    pub cursor: usize,
    /// 사용자가 친 한글 음절(또는 영문) 시퀀스(commit된 것).
    committed: Vec<char>,
    composer: ComposerKind,
    profile: LayoutProfile,
    composer2: Option<HangulComposer2Bul>,
    composer3: Option<HangulComposer3Bul>,
    pub stats: Stats,
    /// 셀 위치별 히트 통계 — 키 히트맵 입력.
    pub key_stats: HashMap<(u8, u8), KeyStat>,
    started_at: Option<Instant>,
}

// 외부에서 호출하지 않는 메서드 일부는 후속 phase(다시 시작 / 음절별 색칠)에서 사용 예정.
#[allow(dead_code)]
impl PracticeSession {
    pub fn new(profile: LayoutProfile, target: String) -> Self {
        let kind = ComposerKind::from_profile(&profile);
        let composer2 = match kind {
            ComposerKind::Hangul2Bul => HangulComposer2Bul::new_with_profile(&profile).ok(),
            _ => None,
        };
        let composer3 = match kind {
            ComposerKind::Hangul3Bul => HangulComposer3Bul::new_with_profile(&profile).ok(),
            _ => None,
        };
        Self {
            target: target.chars().collect(),
            cursor: 0,
            committed: Vec::new(),
            composer: kind,
            profile,
            composer2,
            composer3,
            stats: Stats::default(),
            key_stats: HashMap::new(),
            started_at: None,
        }
    }

    pub fn target(&self) -> &[char] {
        &self.target
    }

    pub fn committed_chars(&self) -> &[char] {
        &self.committed
    }

    pub fn restart(&mut self) {
        self.cursor = 0;
        self.committed.clear();
        self.stats = Stats::default();
        self.key_stats.clear();
        self.started_at = None;
        // composer 재생성
        self.composer2 = match self.composer {
            ComposerKind::Hangul2Bul => HangulComposer2Bul::new_with_profile(&self.profile).ok(),
            _ => None,
        };
        self.composer3 = match self.composer {
            ComposerKind::Hangul3Bul => HangulComposer3Bul::new_with_profile(&self.profile).ok(),
            _ => None,
        };
    }

    pub fn profile(&self) -> &LayoutProfile {
        &self.profile
    }

    pub fn elapsed_secs(&self) -> f64 {
        self.started_at
            .map(|s| s.elapsed().as_secs_f64())
            .unwrap_or(0.0)
    }

    /// 사용자 입력 1키 처리.
    ///
    /// `byte`: ASCII 출력 가능 문자. shift는 layout.upper/lower 선택에 사용.
    pub fn handle_keypress(&mut self, byte: u8, shift: bool) -> StepOutcome {
        if self.started_at.is_none() {
            self.started_at = Some(Instant::now());
        }
        // 키 → 자모 라벨 찾기 + 셀 좌표 기록
        let cell = find_cell_for_ascii(&self.profile, byte, shift);
        if let Some((row, col, label)) = cell {
            let entry = self.key_stats.entry((row, col)).or_default();
            entry.attempts += 1;

            let committed = self.simulate(label.as_str());
            self.update_progress(committed, &label, row, col)
        } else {
            // 매핑 없는 키 — 통계만 attempts 카운트 안 하고 통과
            StepOutcome {
                committed: None,
                progress: self.progress(),
                done: false,
            }
        }
    }

    /// 외부에서 시간을 갱신해 stats.elapsed_secs를 최신화 — UI 30fps 타이머가 호출.
    pub fn tick(&mut self) {
        self.stats.elapsed_secs = self.elapsed_secs();
    }

    fn progress(&self) -> f64 {
        if self.target.is_empty() {
            return 1.0;
        }
        self.cursor.min(self.target.len()) as f64 / self.target.len() as f64
    }

    fn update_progress(
        &mut self,
        committed_char: Option<char>,
        _label: &str,
        row: u8,
        col: u8,
    ) -> StepOutcome {
        let mut committed_ok = None;
        if let Some(ch) = committed_char {
            // 공백·newline은 그대로 비교; 한글은 음절 단위로 비교
            self.committed.push(ch);
            self.stats.total_input_chars += 1;
            let expected = self.target.get(self.cursor).copied();
            let ok = expected == Some(ch);
            if ok {
                self.stats.correct_chars += 1;
                self.cursor += 1;
            } else {
                self.stats.error_chars += 1;
                let entry = self.key_stats.entry((row, col)).or_default();
                entry.errors += 1;
                // 오타는 cursor 그대로 둔다 (사용자가 다시 시도) — 더 단순한 흐름.
                // 단 같은 키를 다시 누를 때 우리 cursor는 그대로 — UX로는 cursor도 진행시켜
                // 다음 글자로 넘어가는 게 자연스럽다.
                self.cursor += 1;
            }
            committed_ok = Some(ok);
            self.stats.elapsed_secs = self.elapsed_secs();
        }
        let progress = self.progress();
        let done = self.cursor >= self.target.len();
        StepOutcome {
            committed: committed_ok,
            progress,
            done,
        }
    }

    fn simulate(&mut self, label: &str) -> Option<char> {
        // 한 ASCII 키 누르기에 대응하는 commit 글자(있다면) 반환.
        // 한글: composer 결과 — 새 글자가 confirmed 될 때만 Some.
        // 영문/Plain: label의 첫 글자를 즉시 commit.
        match self.composer {
            ComposerKind::Plain => label.chars().next(),
            ComposerKind::Hangul2Bul | ComposerKind::Hangul3Bul => {
                // label은 자모(또는 영문/기호)일 수 있음. 자모면 composer에 add, 아니면 commit.
                if let Some(first_ch) = label.chars().next() {
                    if let Ok(jamo) = jamo_from_char(first_ch) {
                        if matches!(self.composer, ComposerKind::Hangul2Bul) {
                            if let Some(c2) = self.composer2.as_mut() {
                                return c2.add_jamo(jamo);
                            }
                        } else if let Some(c3) = self.composer3.as_mut() {
                            return c3.add_jamo(jamo);
                        }
                        return None;
                    }
                    // 자모가 아닌 일반 문자(공백/기호/영문 등) — composer flush 후 commit
                    return Some(first_ch);
                }
                None
            }
        }
    }
}

/// LayoutProfile에서 ASCII 키 코드(`byte`)에 대응하는 셀(row, col, 라벨)을 찾는다.
///
/// 비교 대상: layout.{lower|upper}.row_n의 모든 셀 — 해당 셀 라벨이 한 글자고
/// 그 글자가 ASCII 영역의 lowercase(byte)와 일치하면 매치.
/// 단, 실제로는 layout 셀에 한글 자모가 들어있으므로 ASCII 키 자체 매칭은 어렵다.
/// 그래서 **QWERTY 기준 위치**로 역색인 — 즉 (ASCII byte) → QWERTY position →
/// 같은 자리의 layout 셀 라벨을 가져옴.
///
/// `shift`가 true면 upper, false면 lower.
pub fn find_cell_for_ascii(
    profile: &LayoutProfile,
    byte: u8,
    shift: bool,
) -> Option<(u8, u8, String)> {
    let (row, col) = qwerty_position(byte)?;
    let rows = if shift {
        &profile.layout.upper
    } else {
        &profile.layout.lower
    };
    let label = cell_label(rows, row, col)?;
    Some((row, col, label.to_string()))
}

fn cell_label(rows: &LayoutRows, row: u8, col: u8) -> Option<&str> {
    let slice: &Vec<String> = match row {
        0 => &rows.row1,
        1 => &rows.row2,
        2 => &rows.row3,
        3 => &rows.row4,
        _ => return None,
    };
    slice.get(col as usize).map(|s| s.as_str()).filter(|s| !s.is_empty())
}

/// QWERTY 기준 ASCII 키 → (row, col) 매핑.
///
/// row0 = 숫자열, row1 = QWERTY, row2 = ASDF, row3 = ZXCV.
pub fn qwerty_position(byte: u8) -> Option<(u8, u8)> {
    // lowercase 정규화
    let b = if byte.is_ascii_uppercase() {
        byte.to_ascii_lowercase()
    } else {
        byte
    };
    const ROW0: &[u8] = b"`1234567890-=";
    const ROW1: &[u8] = b"qwertyuiop[]";
    const ROW2: &[u8] = b"asdfghjkl;'";
    const ROW3: &[u8] = b"zxcvbnm,./";
    if let Some(i) = ROW0.iter().position(|&c| c == b) {
        return Some((0, i as u8));
    }
    if let Some(i) = ROW1.iter().position(|&c| c == b) {
        return Some((1, i as u8));
    }
    if let Some(i) = ROW2.iter().position(|&c| c == b) {
        return Some((2, i as u8));
    }
    if let Some(i) = ROW3.iter().position(|&c| c == b) {
        return Some((3, i as u8));
    }
    None
}

/// 한 글자(자모 또는 영문)를 `JamoEnum`으로 변환. 자모가 아니면 Err.
fn jamo_from_char(ch: char) -> Result<unim::hangul::JamoEnum, ()> {
    use std::str::FromStr;
    use unim::hangul::{Cho, JamoEnum, Jong, Jung};
    let s = ch.to_string();
    // 호환 자모(U+3131..U+318E)와 초성/중성/종성 영역(U+1100..U+11FF) 모두 FromStr이 수용.
    if let Ok(cho) = Cho::from_str(&s) {
        return Ok(JamoEnum::Cho(cho));
    }
    if let Ok(jung) = Jung::from_str(&s) {
        return Ok(JamoEnum::Jung(jung));
    }
    if let Ok(jong) = Jong::from_str(&s) {
        return Ok(JamoEnum::Jong(jong));
    }
    Err(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use unim::keystroke::profile::load_builtin_profile;

    #[test]
    fn qwerty_position_lowercase() {
        assert_eq!(qwerty_position(b'a'), Some((2, 0)));
        assert_eq!(qwerty_position(b'q'), Some((1, 0)));
        assert_eq!(qwerty_position(b'z'), Some((3, 0)));
        assert_eq!(qwerty_position(b'1'), Some((0, 1)));
    }

    #[test]
    fn qwerty_position_uppercase_normalises() {
        assert_eq!(qwerty_position(b'A'), Some((2, 0)));
    }

    #[test]
    fn qwerty_position_unknown_byte() {
        assert_eq!(qwerty_position(b' '), None);
        assert_eq!(qwerty_position(b'!'), None);
    }

    #[test]
    fn find_cell_for_2bulstd_r_is_choseong_g() {
        // 두벌식 표준: `r` 위치(QWERTY row=1, col=3)는 lower row2[3] = "ㄱ"
        let p = load_builtin_profile("ko_2bulstd").unwrap();
        let cell = find_cell_for_ascii(&p, b'r', false);
        assert!(cell.is_some(), "find_cell_for_ascii returned None");
        let (_r, _c, label) = cell.unwrap();
        assert_eq!(label, "ㄱ");
    }

    #[test]
    fn find_cell_for_2bulstd_k_is_a_vowel() {
        let p = load_builtin_profile("ko_2bulstd").unwrap();
        let cell = find_cell_for_ascii(&p, b'k', false);
        assert!(cell.is_some());
        // `k` = row2 col=7 = lower["ㅏ"]
        assert_eq!(cell.unwrap().2, "ㅏ");
    }

    #[test]
    fn stats_wpm_short_session_is_zero() {
        let s = Stats {
            total_input_chars: 5,
            correct_chars: 5,
            error_chars: 0,
            elapsed_secs: 0.5,
        };
        assert_eq!(s.wpm(), 0.0);
    }

    #[test]
    fn stats_wpm_basic() {
        // 60초 동안 정답 60자 → 60/5 = 12 wpm
        let s = Stats {
            total_input_chars: 60,
            correct_chars: 60,
            error_chars: 0,
            elapsed_secs: 60.0,
        };
        assert!((s.wpm() - 12.0).abs() < 1e-9);
    }

    #[test]
    fn stats_accuracy_basic() {
        let s = Stats {
            total_input_chars: 10,
            correct_chars: 7,
            error_chars: 3,
            elapsed_secs: 30.0,
        };
        assert!((s.accuracy() - 70.0).abs() < 1e-9);
        assert!((s.error_rate() - 30.0).abs() < 1e-9);
    }

    #[test]
    fn stats_accuracy_empty_session_is_full() {
        assert_eq!(Stats::default().accuracy(), 100.0);
    }

    #[test]
    fn composer_kind_2bul_detected() {
        let p = load_builtin_profile("ko_2bulstd").unwrap();
        assert!(matches!(ComposerKind::from_profile(&p), ComposerKind::Hangul2Bul));
    }

    #[test]
    fn composer_kind_anmatae_is_3bul() {
        let p = load_builtin_profile("ko_3bul_anmatae").unwrap();
        assert!(matches!(ComposerKind::from_profile(&p), ComposerKind::Hangul3Bul));
    }

    #[test]
    fn composer_kind_plain_for_english() {
        let p = load_builtin_profile("en_qwerty").unwrap();
        assert!(matches!(ComposerKind::from_profile(&p), ComposerKind::Plain));
    }
}
