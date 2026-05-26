//! 한글 자모 카탈로그 — 초성(19)/중성(21)/종성(27, 없음 제외).
//!
//! 키 편집·조합 다이얼로그의 드롭다운 모델 소스. 각 엔트리는
//! - `compat`     : 호환 자모 (U+3131~, 예 `ㄱ`) — 키 라벨·표시용.
//! - `conjoining` : 첫가끝 자모 (U+1100~ / U+1161~ / U+11A8~) — 조합 RawTriple 용.

use gtk4::{self as gtk};

use unim::hangul::jamo::{Cho, Jamo, Jong, Jung};

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Scope {
    Cho,
    Jung,
    Jong,
}

#[derive(Clone, Copy)]
pub struct JamoEntry {
    pub compat: char,
    /// 첫가끝 자모 — 조합(combo) 다이얼로그(Phase D)에서 RawTriple 에 사용.
    #[allow(dead_code)]
    pub conjoining: char,
}

const FILLER: char = '\u{3164}';

fn push(out: &mut Vec<JamoEntry>, compat: char, conjoining: char) {
    if compat == '\u{0000}' || compat == FILLER {
        return;
    }
    out.push(JamoEntry { compat, conjoining });
}

/// 스코프별 자모 목록.
pub fn list(scope: Scope) -> Vec<JamoEntry> {
    let mut out = Vec::new();
    match scope {
        Scope::Cho => {
            for seq in 0..19 {
                if let Some(j) = Cho::from_sequence(seq) {
                    push(&mut out, j.get_unicode_compat(), j.get_unicode());
                }
            }
        }
        Scope::Jung => {
            for seq in 0..21 {
                if let Some(j) = Jung::from_sequence(seq) {
                    push(&mut out, j.get_unicode_compat(), j.get_unicode());
                }
            }
        }
        Scope::Jong => {
            // seq 0 = 종성 없음 — 제외.
            for seq in 1..28 {
                if let Some(j) = Jong::from_sequence(seq) {
                    push(&mut out, j.get_unicode_compat(), j.get_unicode());
                }
            }
        }
    }
    out
}

/// 드롭다운용 StringList — 저장 규약과 동일한 글자를 표시.
/// 초·중성은 호환 자모(`ㄱ`/`ㅗ`), **종성은 첫가끝 자모(`ᆨ`)** 를 보여준다.
/// 조합 다이얼로그·키 편집 헬퍼 양쪽에서 사용.
pub fn combo_string_list(scope: Scope) -> gtk::StringList {
    let sl = gtk::StringList::new(&[]);
    for e in list(scope) {
        sl.append(&combo_char(scope, &e).to_string());
    }
    sl
}

/// 호환 자모 문자의 스코프 내 인덱스 (드롭다운 사전 선택용; Phase D/F).
#[allow(dead_code)]
pub fn index_of_compat(scope: Scope, compat: char) -> Option<u32> {
    list(scope)
        .iter()
        .position(|e| e.compat == compat)
        .map(|i| i as u32)
}

/// 인덱스 → 엔트리.
pub fn entry_at(scope: Scope, index: u32) -> Option<JamoEntry> {
    list(scope).get(index as usize).copied()
}

/// 조합(combinations) RawTriple 에 들어갈 문자.
///
/// 빌트인 규약: 초성·중성은 호환 자모(`ㄱ`/`ㅗ`), 종성은 첫가끝 자모(`ᆨ`).
pub fn combo_char(scope: Scope, entry: &JamoEntry) -> char {
    match scope {
        Scope::Jong => entry.conjoining,
        _ => entry.compat,
    }
}

/// 조합 문자열로 스코프 내 인덱스 찾기 (편집 시 드롭다운 사전 선택).
pub fn index_of_combo_char(scope: Scope, ch: char) -> Option<u32> {
    list(scope)
        .iter()
        .position(|e| combo_char(scope, e) == ch)
        .map(|i| i as u32)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cho_list_has_19_entries() {
        assert_eq!(list(Scope::Cho).len(), 19);
    }

    #[test]
    fn jung_list_has_21_entries() {
        assert_eq!(list(Scope::Jung).len(), 21);
    }

    #[test]
    fn jong_list_has_27_entries() {
        // 종성 28종 중 '없음' 제외 → 27.
        assert_eq!(list(Scope::Jong).len(), 27);
    }

    #[test]
    fn cho_compat_chars_in_3131_block() {
        for e in list(Scope::Cho) {
            let cp = e.compat as u32;
            assert!(
                (0x3131..=0x314E).contains(&cp),
                "compat {cp:#x} out of 호환자모 자음 영역"
            );
        }
    }

    #[test]
    fn cho_conjoining_in_1100_block() {
        for e in list(Scope::Cho) {
            let cp = e.conjoining as u32;
            assert!((0x1100..=0x1112).contains(&cp), "conjoining {cp:#x} 초성 영역 밖");
        }
    }

    #[test]
    fn jung_conjoining_in_1161_block() {
        for e in list(Scope::Jung) {
            let cp = e.conjoining as u32;
            assert!((0x1161..=0x1175).contains(&cp), "conjoining {cp:#x} 중성 영역 밖");
        }
    }

    #[test]
    fn index_roundtrip() {
        let entries = list(Scope::Cho);
        let target = entries[5];
        let idx = index_of_compat(Scope::Cho, target.compat).unwrap();
        assert_eq!(entry_at(Scope::Cho, idx).unwrap().compat, target.compat);
    }
}
