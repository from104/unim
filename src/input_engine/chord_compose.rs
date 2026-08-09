// chord_compose.rs
//! Chord 결합기 — 순수 함수, 외부 상태 의존 없음.
//!
//! chord 윈도우가 만료(flush)되었을 때 버퍼에 쌓인 자모·비자모 목록을
//! 받아 영역별로 결합하고 preedit inject 가능 여부를 결정한다.
//!
//! ## 통일 원칙 (Atomic Window Principle)
//!
//! | 케이스 | inject_to_preedit |
//! |---|---|
//! | 비자모 미포함 + 모든 영역 reduce 성공 (≤1) | `true` |
//! | 비자모 미포함 + 어떤 영역 reduce 실패 | `false`, fallback_jamos 채움 |
//! | 비자모 포함 | `false`, 자모 부분도 fallback_jamos로 commit |

use crate::hangul::composer::{CombinedJamoMap, JamoMeta};
use crate::hangul::jamo::{Cho, Jong, Jung, JamoEnum};

// ─── 입력 타입 ───────────────────────────────────────────────────────────────

/// chord 안에 들어온 한 키의 분류된 표현.
#[derive(Debug, Clone, Copy)]
pub enum ChordEntryKind {
    /// 자모 키 (cho/jung/jong).
    Jamo(JamoEnum),
    /// 비자모 출력 키 (`-`, `,`, 영문 등 keyboard_map 미매핑 일반 문자).
    NonJamo(char),
}

/// chord 안에 누적된 한 항목.
#[derive(Debug, Clone, Copy)]
pub struct ChordEntry {
    pub kind: ChordEntryKind,
    /// 입력 순서 (0-based). force_flush 시 stable sort 기준.
    pub input_order: u8,
    /// 자모 메타 (context_alt 조건 등). NonJamo의 경우 무시됨.
    pub meta: JamoMeta,
}

// ─── 결합 결과 타입 ───────────────────────────────────────────────────────────

/// `compose_chord`의 결과 — Atomic Window Principle 핵심 결정 단위.
#[derive(Debug, Clone)]
pub struct ChordResult {
    /// 초성 결합 결과. 영역 reduce 성공한 단일 자모.
    pub cho: Option<Cho>,
    /// 중성 결합 결과.
    pub jung: Option<Jung>,
    /// 종성 결합 결과.
    pub jong: Option<Jong>,
    /// 자모 영역 reduce 실패 시 fallback 호환자모 (cho→jung→jong 순).
    /// 성공·비자모만 있을 때는 비어있음.
    /// 비자모 포함 시 reduce 성공한 영역도 여기로 flatten.
    pub fallback_jamos: Vec<char>,
    /// 비자모 (입력 순서 보존).
    pub non_jamos: Vec<char>,
    /// 자모 input_order 큐 (backspace 용 — 입력 순서대로).
    pub input_order_jamos: Vec<JamoEnum>,
    /// preedit inject 가능 여부.
    /// `true`면 syllable preedit. `false`면 모두 commit 필수.
    pub inject_to_preedit: bool,
}

// ─── 핵심 함수 ────────────────────────────────────────────────────────────────

/// chord 자모·비자모 목록을 받아 영역별 결합 후 `ChordResult`를 반환.
///
/// `entries`는 input_order 순으로 정렬되어 있지 않아도 된다 (내부에서 정렬).
pub fn compose_chord(entries: &[ChordEntry], combined_jamo: &CombinedJamoMap) -> ChordResult {
    // ── 1. 영역 분리 ───────────────────────────────────────────────────────
    // input_order 기준 stable sort 후 분리.
    let mut sorted: Vec<ChordEntry> = entries.to_vec();
    sorted.sort_by_key(|e| e.input_order);

    let mut cho_jamos: Vec<Cho> = Vec::new();
    let mut jung_jamos: Vec<Jung> = Vec::new();
    let mut jong_jamos: Vec<Jong> = Vec::new();
    let mut non_jamos: Vec<char> = Vec::new();
    let mut input_order_jamos: Vec<JamoEnum> = Vec::new();

    for entry in &sorted {
        match entry.kind {
            ChordEntryKind::Jamo(JamoEnum::Cho(c)) => {
                cho_jamos.push(c);
                input_order_jamos.push(JamoEnum::Cho(c));
            }
            ChordEntryKind::Jamo(JamoEnum::Jung(j)) => {
                jung_jamos.push(j);
                input_order_jamos.push(JamoEnum::Jung(j));
            }
            ChordEntryKind::Jamo(JamoEnum::Jong(j)) => {
                jong_jamos.push(j);
                input_order_jamos.push(JamoEnum::Jong(j));
            }
            ChordEntryKind::Jamo(JamoEnum::Special(c)) => {
                // Special은 NonJamo로 처리
                non_jamos.push(c);
            }
            ChordEntryKind::NonJamo(c) => {
                non_jamos.push(c);
            }
        }
    }

    // ── 2. 영역별 reduce ──────────────────────────────────────────────────
    let cho_result = reduce_cho(&cho_jamos, combined_jamo);
    let jung_result = reduce_jung(&jung_jamos, combined_jamo);
    let jong_result = reduce_jong(&jong_jamos, combined_jamo);

    let all_reduced = cho_result.is_ok() && jung_result.is_ok() && jong_result.is_ok();
    // 자모가 하나도 없으면 inject 의미 없음 (비자모만 있는 경우)
    let has_jamo = !cho_jamos.is_empty() || !jung_jamos.is_empty() || !jong_jamos.is_empty();

    // ── 3. 통일 원칙 결정 ─────────────────────────────────────────────────
    if all_reduced && has_jamo {
        // 자모 영역 전부 reduce 성공 + 자모 있음 (비자모 유무 무관).
        // inject_to_preedit=true: 비자모 없으면 preedit inject, 있으면 syllable commit 후 비자모 commit.
        ChordResult {
            cho: cho_result.ok().flatten(),
            jung: jung_result.ok().flatten(),
            jong: jong_result.ok().flatten(),
            fallback_jamos: Vec::new(),
            non_jamos,
            input_order_jamos,
            inject_to_preedit: true,
        }
    } else {
        // 어떤 영역 reduce 실패 → 모두 호환자모 commit (비자모도 뒤에 붙임)
        //
        // fallback_jamos: cho→jung→jong 순 호환자모.
        // reduce 부분 성공/실패 혼재 시 단순화 — 모두 호환자모 commit.
        let mut fallback_jamos: Vec<char> = Vec::new();

        // cho: reduce 성공이면 그 결과, 실패면 원래 자모들
        match cho_result {
            Ok(Some(c)) => fallback_jamos.push(Jamo::get_unicode_compat(&c)),
            Ok(None) => {}
            Err(ref jamos) => {
                for c in jamos {
                    fallback_jamos.push(Jamo::get_unicode_compat(c));
                }
            }
        }

        // jung: reduce 성공이면 그 결과, 실패면 원래 자모들
        match jung_result {
            Ok(Some(j)) => fallback_jamos.push(Jamo::get_unicode_compat(&j)),
            Ok(None) => {}
            Err(ref jamos) => {
                for j in jamos {
                    fallback_jamos.push(Jamo::get_unicode_compat(j));
                }
            }
        }

        // jong: reduce 성공이면 그 결과, 실패면 원래 자모들
        match jong_result {
            Ok(Some(j)) => fallback_jamos.push(Jamo::get_unicode_compat(&j)),
            Ok(None) => {}
            Err(ref jamos) => {
                for j in jamos {
                    fallback_jamos.push(Jamo::get_unicode_compat(j));
                }
            }
        }

        ChordResult {
            cho: None,
            jung: None,
            jong: None,
            fallback_jamos,
            non_jamos,
            input_order_jamos,
            inject_to_preedit: false,
        }
    }
}

// ─── 영역별 reduce 함수 ──────────────────────────────────────────────────────

// 성공: Ok(Some(결합결과)) 또는 Ok(None) (0자모)
// 실패: Err(원래_자모_목록) — 호환자모 변환은 호출자가 수행

/// 초성 영역 reduce. 최대 2자모, 2 permutation 시도.
fn reduce_cho(
    jamos: &[Cho],
    map: &CombinedJamoMap,
) -> Result<Option<Cho>, Vec<Cho>> {
    match jamos.len() {
        0 => Ok(None),
        1 => Ok(Some(jamos[0])),
        2 => {
            // 2 permutation: (a,b) 와 (b,a)
            let (a, b) = (jamos[0], jamos[1]);
            if let Some(JamoEnum::Cho(r)) = map.get(&(JamoEnum::Cho(a), JamoEnum::Cho(b))) {
                return Ok(Some(*r));
            }
            if let Some(JamoEnum::Cho(r)) = map.get(&(JamoEnum::Cho(b), JamoEnum::Cho(a))) {
                return Ok(Some(*r));
            }
            Err(jamos.to_vec())
        }
        _ => {
            // cho > 2: 모든 permutation × left-fold 시도
            if let Some(result) = try_all_permutations_cho(jamos, map) {
                Ok(Some(result))
            } else {
                Err(jamos.to_vec())
            }
        }
    }
}

/// 중성 영역 reduce. 최대 3자모, 모든 permutation (n! = 6) × left-fold.
fn reduce_jung(
    jamos: &[Jung],
    map: &CombinedJamoMap,
) -> Result<Option<Jung>, Vec<Jung>> {
    match jamos.len() {
        0 => Ok(None),
        1 => Ok(Some(jamos[0])),
        _ => {
            if let Some(result) = try_all_permutations_jung(jamos, map) {
                Ok(Some(result))
            } else {
                Err(jamos.to_vec())
            }
        }
    }
}

/// 종성 영역 reduce. 최대 3자모, 모든 permutation (n! = 6) × left-fold.
fn reduce_jong(
    jamos: &[Jong],
    map: &CombinedJamoMap,
) -> Result<Option<Jong>, Vec<Jong>> {
    match jamos.len() {
        0 => Ok(None),
        1 => Ok(Some(jamos[0])),
        _ => {
            if let Some(result) = try_all_permutations_jong(jamos, map) {
                Ok(Some(result))
            } else {
                Err(jamos.to_vec())
            }
        }
    }
}

// ─── Permutation + Left-fold 헬퍼 ────────────────────────────────────────────

/// 배열에 대한 모든 permutation을 생성 (Heap's algorithm).
fn permutations<T: Clone>(arr: &[T]) -> Vec<Vec<T>> {
    let n = arr.len();
    if n == 0 {
        return vec![vec![]];
    }
    if n == 1 {
        return vec![arr.to_vec()];
    }
    let mut result = Vec::new();
    let mut arr = arr.to_vec();
    heap_permutation(&mut arr, n, &mut result);
    result
}

fn heap_permutation<T: Clone>(arr: &mut Vec<T>, k: usize, result: &mut Vec<Vec<T>>) {
    if k == 1 {
        result.push(arr.clone());
        return;
    }
    for i in 0..k {
        heap_permutation(arr, k - 1, result);
        if k % 2 == 0 {
            arr.swap(i, k - 1);
        } else {
            arr.swap(0, k - 1);
        }
    }
}

/// 주어진 Cho 순서에서 left-fold 결합 시도. 성공하면 Some(결과), 실패하면 None.
fn left_fold_cho(perm: &[Cho], map: &CombinedJamoMap) -> Option<Cho> {
    if perm.is_empty() {
        return None;
    }
    let mut acc = perm[0];
    for &next in perm.iter().skip(1) {
        match map.get(&(JamoEnum::Cho(acc), JamoEnum::Cho(next))) {
            Some(JamoEnum::Cho(r)) => acc = *r,
            _ => return None,
        }
    }
    Some(acc)
}

/// 주어진 Jung 순서에서 left-fold 결합 시도.
fn left_fold_jung(perm: &[Jung], map: &CombinedJamoMap) -> Option<Jung> {
    if perm.is_empty() {
        return None;
    }
    let mut acc = perm[0];
    for &next in perm.iter().skip(1) {
        match map.get(&(JamoEnum::Jung(acc), JamoEnum::Jung(next))) {
            Some(JamoEnum::Jung(r)) => acc = *r,
            _ => return None,
        }
    }
    Some(acc)
}

/// 주어진 Jong 순서에서 left-fold 결합 시도.
fn left_fold_jong(perm: &[Jong], map: &CombinedJamoMap) -> Option<Jong> {
    if perm.is_empty() {
        return None;
    }
    let mut acc = perm[0];
    for &next in perm.iter().skip(1) {
        match map.get(&(JamoEnum::Jong(acc), JamoEnum::Jong(next))) {
            Some(JamoEnum::Jong(r)) => acc = *r,
            _ => return None,
        }
    }
    Some(acc)
}

fn try_all_permutations_cho(jamos: &[Cho], map: &CombinedJamoMap) -> Option<Cho> {
    for perm in permutations(jamos) {
        if let Some(r) = left_fold_cho(&perm, map) {
            return Some(r);
        }
    }
    None
}

fn try_all_permutations_jung(jamos: &[Jung], map: &CombinedJamoMap) -> Option<Jung> {
    for perm in permutations(jamos) {
        if let Some(r) = left_fold_jung(&perm, map) {
            return Some(r);
        }
    }
    None
}

fn try_all_permutations_jong(jamos: &[Jong], map: &CombinedJamoMap) -> Option<Jong> {
    for perm in permutations(jamos) {
        if let Some(r) = left_fold_jong(&perm, map) {
            return Some(r);
        }
    }
    None
}

use crate::hangul::jamo::Jamo;
