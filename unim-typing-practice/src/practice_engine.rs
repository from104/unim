//! 타자 연습 코어 — 5차 단순화.
//!
//! 4차까지는 키 단위로 한글 합성을 시뮬레이션하고 음절 commit을 추적했다.
//! 5차에서는 입력을 `gtk::Entry` 가 native 로 처리한다 — IME(unim) 가 한글 조합·
//! 한자 변환·백스페이스·스페이스를 모두 운영체제 수준에서 처리하므로 본 모듈은
//! **순수 통계만** 책임진다.
//!
//! 외부에서 호출하는 것:
//! - `new(profile, target)` — 첫 줄 target 으로 세션 시작.
//! - `advance_to_line(target)` — 다음 줄로 전환 (target 만 교체, 통계 유지).
//! - `evaluate(text)` — 현재 입력 텍스트를 받아 target 과 prefix 비교 → 색칠/통계.
//! - `commit_line()` — 줄 완료 시점 호출, 누적 통계로 흘려보냄.
//! - `tick()` — UI 타이머에서 elapsed 갱신.
//!
//! GTK 의존이 없어 단위 테스트 가능.

use std::collections::HashMap;
use std::time::Instant;

use unim::keystroke::profile::LayoutProfile;

use unim_keymap_common::keyboard_widget::KeyStat;

#[derive(Debug, Default, Clone, Copy)]
pub struct Stats {
    /// 정확도/오타율 계산용 — 음절/글자 단위 (target 기준).
    pub total_input_chars: u32,
    pub correct_chars: u32,
    pub error_chars: u32,
    /// CPM/WPM 계산용 — 키 단위 (한글 음절은 자모 키스트로크 수로 분해).
    /// 예: '정' 정타 = +3타 (ㅈ+ㅓ+ㅇ), 영문 'a' 정타 = +1타.
    pub correct_keystrokes: u32,
    pub elapsed_secs: f64,
}

impl Stats {
    pub fn wpm(&self) -> f64 {
        if self.elapsed_secs < 1.0 {
            return 0.0;
        }
        // 영문 5타 = 1단어 표준. 한글 음절도 자모 키스트로크 합으로 환산.
        (self.correct_keystrokes as f64 / 5.0) / (self.elapsed_secs / 60.0)
    }
    pub fn cpm(&self) -> f64 {
        if self.elapsed_secs < 1.0 {
            return 0.0;
        }
        // 분당 타수 — 한글 한 음절은 자모 수만큼 가산 (한국 표준 측정 방식).
        self.correct_keystrokes as f64 / (self.elapsed_secs / 60.0)
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

/// 현재 줄의 입력 텍스트 vs target 평가 결과.
#[derive(Debug, Clone, Copy)]
#[allow(dead_code)] // target_chars 는 테스트 / 외부 진단용 — UI 측은 input/correct 만 사용.
pub struct LineEval {
    /// prefix 일치 글자 수 (앞에서부터 같은 글자).
    pub correct_prefix: usize,
    /// 입력 텍스트 총 글자 수.
    pub input_chars: usize,
    /// target 총 글자 수.
    pub target_chars: usize,
    /// target == input — 줄 완료 신호.
    pub line_complete: bool,
    /// 입력 진행률 0..=1 (현재 줄 기준).
    pub progress: f64,
}

pub struct PracticeSession {
    target: Vec<char>,
    /// 현재 줄의 마지막 평가 — paint·통계 계산 공유용.
    last_eval: LineEval,
    profile: LayoutProfile,
    /// 한글 자판 코드 (예: "ko_2bulstd", "ko_3bul390") — 히트맵에서 한글 음절을
    /// 영문 키스트로크로 분해할 때 사용.
    layout_code: String,
    /// 누적 통계 — 여러 줄에 걸쳐 누적.
    pub stats: Stats,
    /// 키 히트맵 입력 — target 글자 위치(셀)별 오타 카운트.
    /// 줄 완료 시점에 mismatched 글자의 셀 좌표를 카운트.
    pub key_stats: HashMap<(u8, u8), KeyStat>,
    /// 줄별 누적 WPM — Sparkline 입력 (DESIGN.md §15.3).
    pub wpm_per_line: Vec<f64>,
    /// 백스페이스 카운트 — KeyCountCard.
    pub backspace_count: u32,
    /// 현재 줄의 마지막 입력 텍스트 (preedit 포함). paint 의 word-by-word 비교용.
    last_input: String,
    /// 이미 commit 된 줄들의 final input — done 줄의 오타/정타 표시 회고용.
    pub line_inputs: Vec<String>,
    started_at: Option<Instant>,
}

impl PracticeSession {
    pub fn new(profile: LayoutProfile, layout_code: String, target: String) -> Self {
        let target_chars: Vec<char> = target.chars().collect();
        let eval = LineEval {
            correct_prefix: 0,
            input_chars: 0,
            target_chars: target_chars.len(),
            line_complete: target_chars.is_empty(),
            progress: if target_chars.is_empty() { 1.0 } else { 0.0 },
        };
        Self {
            target: target_chars,
            last_eval: eval,
            profile,
            layout_code,
            stats: Stats::default(),
            key_stats: HashMap::new(),
            wpm_per_line: Vec::new(),
            backspace_count: 0,
            last_input: String::new(),
            line_inputs: Vec::new(),
            started_at: None,
        }
    }

    pub fn last_input(&self) -> &str {
        &self.last_input
    }

    #[allow(dead_code)] // 외부/테스트용 진단 슬롯 — paint/진행은 target_text 사용.
    pub fn target_len(&self) -> usize {
        self.target.len()
    }

    pub fn target_text(&self) -> String {
        self.target.iter().collect()
    }

    #[allow(dead_code)] // 테스트/외부 진단용 — 페이지는 last_eval 만 사용.
    pub fn target(&self) -> &[char] {
        &self.target
    }

    #[allow(dead_code)] // 외부/테스트용 진단 슬롯 — paint 는 last_input 사용.
    pub fn last_eval(&self) -> LineEval {
        self.last_eval
    }

    #[allow(dead_code)] // 외부/테스트용 — Result UI 가 KeyboardView 로 옮기며 직접 호출은 없음.
    pub fn profile(&self) -> &LayoutProfile {
        &self.profile
    }

    pub fn elapsed_secs(&self) -> f64 {
        self.started_at
            .map(|s| s.elapsed().as_secs_f64())
            .unwrap_or(0.0)
    }

    /// 외부에서 시간을 갱신해 stats.elapsed_secs를 최신화 — UI 타이머가 호출.
    pub fn tick(&mut self) {
        self.stats.elapsed_secs = self.elapsed_secs();
    }

    /// 다음 줄로 전환 — target 만 교체. **누적 통계는 유지**.
    pub fn advance_to_line(&mut self, target: String) {
        let target_chars: Vec<char> = target.chars().collect();
        self.last_eval = LineEval {
            correct_prefix: 0,
            input_chars: 0,
            target_chars: target_chars.len(),
            line_complete: target_chars.is_empty(),
            progress: if target_chars.is_empty() { 1.0 } else { 0.0 },
        };
        self.target = target_chars;
        self.last_input.clear();
    }

    /// 현재 입력 텍스트를 받아 target 과 prefix 비교 → LineEval 반환.
    ///
    /// 호출자(GTK Entry::changed)는 결과를 보고 색칠을 다시 칠하고, 완료(`line_complete`)
    /// 시점에 `commit_line()` 을 부른다.
    ///
    /// 통계는 **줄 완료 시점에만** 누적되도록 본 함수는 누적을 건드리지 않는다.
    /// 단 started_at 은 첫 입력 시점에 한 번 세팅.
    pub fn evaluate(&mut self, text: &str) -> LineEval {
        if self.started_at.is_none() && !text.is_empty() {
            self.started_at = Some(Instant::now());
        }
        self.last_input = text.to_string();
        let input: Vec<char> = text.chars().collect();
        let mut correct_prefix = 0usize;
        for (i, ch) in input.iter().enumerate() {
            match self.target.get(i) {
                Some(t) if t == ch => correct_prefix += 1,
                _ => break,
            }
        }
        let target_chars = self.target.len();
        let input_chars = input.len();
        let line_complete = input.len() == target_chars && correct_prefix == target_chars;
        let progress = if target_chars == 0 {
            1.0
        } else {
            (input_chars.min(target_chars)) as f64 / target_chars as f64
        };
        self.last_eval = LineEval {
            correct_prefix,
            input_chars,
            target_chars,
            line_complete,
            progress,
        };
        self.last_eval
    }

    /// 줄 완료 시점에 호출 — 현재 줄의 텍스트를 한 번 더 받아 누적 통계에 합산.
    ///
    /// 표시(build_word_aware_markup)와 같은 단어 단위 greedy 매칭으로 정타/오타를 계산.
    /// target/input 모두 whitespace 로 단어 분리 후 같은 인덱스 단어쌍에 align_input_to_target
    /// 을 적용. matched=true 글자 합 → correct, target 글자 수 합 → total, 차 → error.
    /// 공백 자체는 통계 대상에서 제외(표시도 dim).
    ///
    /// 키 히트맵: target 글자마다 attempts++, matched=false 면 errors++.
    pub fn commit_line(&mut self, final_text: &str) {
        let target_text: String = self.target.iter().collect();
        let target_words: Vec<&str> = target_text.split_whitespace().collect();
        let input_words: Vec<&str> = final_text.split_whitespace().collect();

        let mut total = 0usize;
        let mut correct = 0usize;
        let mut correct_keystrokes = 0u32;

        for (i, t_word) in target_words.iter().enumerate() {
            let t_chars: Vec<char> = t_word.chars().collect();
            let i_chars: Vec<char> = input_words
                .get(i)
                .map(|s| s.chars().collect())
                .unwrap_or_default();
            let matched = align_input_to_target(&t_chars, &i_chars);
            for (j, t_ch) in t_chars.iter().enumerate() {
                total += 1;
                let ok = matched[j];
                if ok {
                    correct += 1;
                }
                let cells = target_char_to_cells(&self.profile, &self.layout_code, *t_ch);
                if ok {
                    // 한글 음절은 자모 키스트로크 수만큼 가산 (예: '정' = +3타).
                    correct_keystrokes += cells.len() as u32;
                }
                for (row, col) in cells {
                    let entry = self.key_stats.entry((row, col)).or_default();
                    entry.attempts += 1;
                    if !ok {
                        entry.errors += 1;
                    }
                }
            }
        }

        let errors = total - correct;
        self.stats.total_input_chars += total as u32;
        self.stats.correct_chars += correct as u32;
        self.stats.error_chars += errors as u32;
        self.stats.correct_keystrokes += correct_keystrokes;
        self.stats.elapsed_secs = self.elapsed_secs();
        self.wpm_per_line.push(self.stats.wpm());
        self.line_inputs.push(final_text.to_string());
    }
}

/// Greedy subsequence 매칭 — input 글자들을 target 안에 순서대로 가능한 한 왼쪽부터 매칭.
/// target 글자별 매칭 여부 반환. input 길이 != target 길이 무관.
///
/// 예: target=[정,다,운], input=[정,운] → [true, false, true].
///     target=[민,주,주,의], input=[ㅁ,주] → [false, true, false, false].
///
/// 표시(build_word_aware_markup)와 통계(commit_line) 가 같은 함수를 공유한다.
pub fn align_input_to_target(target: &[char], input: &[char]) -> Vec<bool> {
    let mut matched = vec![false; target.len()];
    let mut t_idx = 0;
    for ic in input {
        let mut search = t_idx;
        while search < target.len() && target[search] != *ic {
            search += 1;
        }
        if search < target.len() {
            matched[search] = true;
            t_idx = search + 1;
        }
    }
    matched
}

/// `keyboard_view` 가 한글 라벨 산출에 사용하는 헬퍼 — ASCII 키 코드(`byte`)에
/// 대응하는 셀(row, col, 라벨)을 찾는다.
///
/// 4차 엔진과 동일 시그너처를 유지하여 `keyboard_view.rs` 호환성을 보장한다.
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
    let slice: &Vec<String> = match row {
        0 => &rows.row1,
        1 => &rows.row2,
        2 => &rows.row3,
        3 => &rows.row4,
        _ => return None,
    };
    let label = slice
        .get(col as usize)
        .map(|s| s.as_str())
        .filter(|s| !s.is_empty())?;
    Some((row, col, label.to_string()))
}

/// QWERTY 기준 ASCII 키 → (row, col) 매핑.
pub fn qwerty_position(byte: u8) -> Option<(u8, u8)> {
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

/// target 글자 → 해당 글자를 입력하기 위해 실제로 눌러야 하는 QWERTY 셀 목록.
///
/// 한글 음절(완성형 가-힣) 은 `unim::typefix::kor_to_eng` 로 영문 키스트로크
/// 문자열로 분해한 뒤 각 ASCII 글자를 QWERTY 셀 좌표로 변환한다.
/// 예: `'정'` (ko_2bulstd) → "wjd" → [(1,1),(2,6),(1,2)].
/// 그 외(영문/숫자/기호) 는 `target_char_to_cell` 단일 매핑.
/// 공백·탭은 빈 Vec.
fn target_char_to_cells(profile: &LayoutProfile, layout_code: &str, ch: char) -> Vec<(u8, u8)> {
    if ch == ' ' || ch == '\t' || ch == '\n' {
        return Vec::new();
    }
    let cp = ch as u32;
    if (0xAC00..=0xD7A3).contains(&cp) {
        let s = ch.to_string();
        let eng = unim::typefix::kor_to_eng(&s, layout_code, "en_qwerty");
        return eng
            .chars()
            .filter_map(|c| {
                let b = if c.is_ascii_uppercase() {
                    c.to_ascii_lowercase() as u8
                } else {
                    c as u8
                };
                qwerty_position(b)
            })
            .collect();
    }
    target_char_to_cell(profile, ch).into_iter().collect()
}

/// target 문자가 해당 자판에서 어느 QWERTY 셀(row, col) 에 대응되는지 찾는다.
///
/// 자판 lower/upper rows 를 순회하여 첫 글자가 일치하는 셀의 (row, col) 반환.
/// 일치 셀이 여러 개면 첫 매치를 채택. 공백·탭·일치 없음은 None.
fn target_char_to_cell(profile: &LayoutProfile, ch: char) -> Option<(u8, u8)> {
    if ch == ' ' || ch == '\t' || ch == '\n' {
        return None;
    }
    let s = ch.to_string();
    for (rows_kind, layer) in [&profile.layout.lower, &profile.layout.upper].iter().enumerate() {
        let _ = rows_kind;
        for (row_idx, row) in [&layer.row1, &layer.row2, &layer.row3, &layer.row4]
            .iter()
            .enumerate()
        {
            for (col, cell) in row.iter().enumerate() {
                if cell == &s {
                    return Some((row_idx as u8, col as u8));
                }
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use unim::keystroke::profile::load_builtin_profile;

    fn dummy_profile() -> LayoutProfile {
        load_builtin_profile("en_qwerty").unwrap()
    }

    #[test]
    fn evaluate_prefix_match() {
        let mut s = PracticeSession::new(dummy_profile(), "en_qwerty".into(), "hello".into());
        let e = s.evaluate("hel");
        assert_eq!(e.correct_prefix, 3);
        assert_eq!(e.input_chars, 3);
        assert_eq!(e.target_chars, 5);
        assert!(!e.line_complete);
    }

    #[test]
    fn evaluate_full_match_completes() {
        let mut s = PracticeSession::new(dummy_profile(), "en_qwerty".into(), "hi".into());
        let e = s.evaluate("hi");
        assert!(e.line_complete);
        assert_eq!(e.correct_prefix, 2);
    }

    #[test]
    fn evaluate_mismatch_breaks_prefix() {
        let mut s = PracticeSession::new(dummy_profile(), "en_qwerty".into(), "hello".into());
        let e = s.evaluate("helXo");
        assert_eq!(e.correct_prefix, 3);
        assert!(!e.line_complete);
    }

    #[test]
    fn commit_line_accumulates_stats() {
        let mut s = PracticeSession::new(dummy_profile(), "en_qwerty".into(), "abc".into());
        s.evaluate("abc");
        s.commit_line("abc");
        assert_eq!(s.stats.total_input_chars, 3);
        assert_eq!(s.stats.correct_chars, 3);
        assert_eq!(s.stats.error_chars, 0);
    }

    #[test]
    fn commit_line_partial_counts_errors() {
        // greedy: target=[a,b,c], input=[a,X,c] → matched=[T,F,T].
        let mut s = PracticeSession::new(dummy_profile(), "en_qwerty".into(), "abc".into());
        s.commit_line("aXc");
        assert_eq!(s.stats.total_input_chars, 3);
        assert_eq!(s.stats.correct_chars, 2);
        assert_eq!(s.stats.error_chars, 1);
    }

    #[test]
    fn align_examples_user_provided() {
        // 사용자 제시 4개 케이스 — 표시 로직과 통계가 동일하게 동작해야 함.
        let cases: &[(&str, &str, Vec<bool>)] = &[
            ("정다운", "정운", vec![true, false, true]),
            ("하늘이", "흐늘", vec![false, true, false]),
            ("다홍치마", "다치", vec![true, false, true, false]),
            ("민주주의", "ㅁ주", vec![false, true, false, false]),
        ];
        for (target, input, expected) in cases {
            let t: Vec<char> = target.chars().collect();
            let i: Vec<char> = input.chars().collect();
            assert_eq!(
                align_input_to_target(&t, &i),
                *expected,
                "target={target}, input={input}"
            );
        }
    }

    #[test]
    fn commit_line_greedy_missing_char() {
        // 정다운/정운 → correct=2(정,운), error=1(다).
        let mut s = PracticeSession::new(dummy_profile(), "en_qwerty".into(), "정다운".into());
        s.commit_line("정운");
        assert_eq!(s.stats.total_input_chars, 3);
        assert_eq!(s.stats.correct_chars, 2);
        assert_eq!(s.stats.error_chars, 1);
        assert!((s.stats.accuracy() - 200.0 / 3.0).abs() < 1e-6);
    }

    #[test]
    fn commit_line_greedy_word_split() {
        // 두 단어: "하늘이 위에" vs "흐늘 위에"
        // 단어1: 하늘이/흐늘 → matched=[F,T,F] (correct=1)
        // 단어2: 위에/위에 → matched=[T,T] (correct=2)
        // total=5, correct=3, error=2.
        let mut s = PracticeSession::new(dummy_profile(), "en_qwerty".into(), "하늘이 위에".into());
        s.commit_line("흐늘 위에");
        assert_eq!(s.stats.total_input_chars, 5);
        assert_eq!(s.stats.correct_chars, 3);
        assert_eq!(s.stats.error_chars, 2);
    }

    #[test]
    fn commit_line_accumulates_hangul_keystrokes() {
        // '정' = wjd 3타, '다' = ek 2타, '운' = dns 3타. 전부 정타면 +8타.
        let mut s = PracticeSession::new(dummy_profile(), "ko_2bulstd".into(), "정다운".into());
        s.commit_line("정다운");
        assert_eq!(s.stats.correct_chars, 3);
        assert_eq!(s.stats.correct_keystrokes, 8);
        // '정','운'만 정타(매칭) → 매칭된 음절의 키스트로크만 가산 = 3+3 = 6타.
        let mut s2 = PracticeSession::new(dummy_profile(), "ko_2bulstd".into(), "정다운".into());
        s2.commit_line("정운");
        assert_eq!(s2.stats.correct_chars, 2);
        assert_eq!(s2.stats.correct_keystrokes, 6);
    }

    #[test]
    fn target_char_to_cells_hangul_decomposes_to_keystrokes() {
        // 2벌식 '정' = ㅈ+ㅓ+ㅇ → "wjd" → 3개 셀.
        let cells = target_char_to_cells(&dummy_profile(), "ko_2bulstd", '정');
        assert_eq!(cells.len(), 3, "정 → wjd 3개 셀");

        // 영문 'a' = 단일 셀.
        let cells_ascii = target_char_to_cells(&dummy_profile(), "ko_2bulstd", 'a');
        assert_eq!(cells_ascii.len(), 1);

        // 공백 = 빈 Vec.
        assert!(target_char_to_cells(&dummy_profile(), "ko_2bulstd", ' ').is_empty());
    }

    #[test]
    fn commit_line_word_count_mismatch_treats_missing_as_errors() {
        // target 단어 3개, input 단어 1개 → 빠진 단어 글자는 전부 오타.
        let mut s = PracticeSession::new(dummy_profile(), "en_qwerty".into(), "a b c".into());
        s.commit_line("a");
        // 단어1: a/a → matched=[T]
        // 단어2: b/(없음) → matched=[F]
        // 단어3: c/(없음) → matched=[F]
        // total=3, correct=1, error=2.
        assert_eq!(s.stats.total_input_chars, 3);
        assert_eq!(s.stats.correct_chars, 1);
        assert_eq!(s.stats.error_chars, 2);
    }

    #[test]
    fn advance_to_line_preserves_stats() {
        let mut s = PracticeSession::new(dummy_profile(), "en_qwerty".into(), "abc".into());
        s.commit_line("abc");
        s.advance_to_line("xyz".into());
        assert_eq!(s.stats.total_input_chars, 3);
        assert_eq!(s.target(), &['x', 'y', 'z']);
    }

    #[test]
    fn stats_wpm_short_session_is_zero() {
        let s = Stats {
            total_input_chars: 5,
            correct_chars: 5,
            error_chars: 0,
            correct_keystrokes: 5,
            elapsed_secs: 0.5,
        };
        assert_eq!(s.wpm(), 0.0);
    }

    #[test]
    fn stats_wpm_basic() {
        // 60타 1분 → CPM 60 → WPM 12.
        let s = Stats {
            total_input_chars: 60,
            correct_chars: 60,
            error_chars: 0,
            correct_keystrokes: 60,
            elapsed_secs: 60.0,
        };
        assert!((s.wpm() - 12.0).abs() < 1e-9);
        assert!((s.cpm() - 60.0).abs() < 1e-9);
    }

    #[test]
    fn stats_cpm_uses_keystrokes_not_chars() {
        // 한글 시뮬레이션: 5음절 = 15타 (음절당 평균 3타) 1분 → CPM 15.
        let s = Stats {
            total_input_chars: 5,
            correct_chars: 5,
            error_chars: 0,
            correct_keystrokes: 15,
            elapsed_secs: 60.0,
        };
        assert!((s.cpm() - 15.0).abs() < 1e-9);
        assert!((s.wpm() - 3.0).abs() < 1e-9);
    }

    #[test]
    fn stats_accuracy_basic() {
        let s = Stats {
            total_input_chars: 10,
            correct_chars: 7,
            error_chars: 3,
            correct_keystrokes: 7,
            elapsed_secs: 30.0,
        };
        assert!((s.accuracy() - 70.0).abs() < 1e-9);
        assert!((s.error_rate() - 30.0).abs() < 1e-9);
    }

    #[test]
    fn stats_accuracy_empty_session_is_full() {
        assert_eq!(Stats::default().accuracy(), 100.0);
    }
}
