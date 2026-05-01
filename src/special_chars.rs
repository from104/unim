//! 초성 기반 특수문자 매핑 모듈
//!
//! Windows 한글 IME의 초성+한자키 특수문자 입력 기능과 동일한 매핑을 제공합니다.
//! 한글 초성(ㄱ~ㅎ)에 대응하는 특수문자 세트를 정의하고 검색합니다.

/// 초성 특수문자 항목
#[derive(Clone, Debug)]
pub struct SpecialCharEntry {
    /// 초성 문자 (예: 'ㄱ')
    pub choseong: char,
    /// 카테고리명 (예: "특수기호")
    pub category: &'static str,
    /// 특수문자 목록
    pub characters: &'static [char],
}

/// 주어진 문자가 한글 초성(자음)인지 판별합니다.
pub fn is_choseong(ch: char) -> bool {
    // 한글 호환 자모 자음 범위: ㄱ(U+3131) ~ ㅎ(U+314E)
    ('\u{3131}'..='\u{314E}').contains(&ch)
}

/// 초성 문자로 특수문자 항목을 검색합니다.
///
/// # 인자
///
/// * `ch` - 검색할 초성 문자 (예: 'ㄱ')
///
/// # 반환
///
/// 해당 초성의 특수문자 항목. 매핑이 없으면 None.
pub fn search_by_choseong(ch: char) -> Option<&'static SpecialCharEntry> {
    SPECIAL_CHAR_TABLE.iter().find(|entry| entry.choseong == ch)
}

// =====================================================================
// Windows IME 호환 초성 특수문자 매핑 테이블
// =====================================================================

/// ㄱ — 특수기호 (구두점, 일반 기호)
const CHARS_GIYEOK: &[char] = &[
    '！', '＇', '，', '．', '／', '：', '；', '？', '＾', '＿', '｀', '｜', '￣', '、', '。', '·',
    '‥', '…', '¨', '〃', '―', '∥', '＼', '∼', '´', '～', 'ˇ', '˘', '˝', '˚', '˙', '¸', '˛', '¡',
    '¿', 'ː',
];

/// ㄴ — 괄호류
const CHARS_NIEUN: &[char] = &[
    '＂', '（', '）', '［', '］', '｛', '｝', '\u{2018}', '\u{2019}', '\u{201C}', '\u{201D}', '〔',
    '〕', '〈', '〉', '《', '》', '「', '」', '『', '』', '【', '】',
];

/// ㄷ — 수학기호
const CHARS_DIGEUT: &[char] = &[
    '＋', '－', '＜', '＝', '＞', '±', '×', '÷', '≠', '≤', '≥', '∞', '∴', '♂', '♀', '∠', '⊥', '⌒',
    '∂', '∇', '≡', '≒', '≪', '≫', '√', '∽', '∝', '∵', '∫', '∬', '∈', '∋', '⊆', '⊇', '⊂', '⊃', '∪',
    '∩', '∧', '∨', '￢', '⇒', '⇔', '∀', '∃', '∮', '∑', '∏',
];

/// ㄹ — 단위기호
const CHARS_RIEUL: &[char] = &[
    '＄', '％', '￦', 'Ｆ', '′', '″', '℃', 'Å', '￠', '￡', '￥', '¤', '℉', '‰', '㎕', '㎖', '㎗',
    'ℓ', '㎘', '㏄', '㎣', '㎤', '㎥', '㎦', '㎙', '㎚', '㎛', '㎜', '㎝', '㎞', '㎟', '㎠', '㎡',
    '㎢', '㏊', '㎍', '㎎', '㎏', '㏏', '㎈', '㎉', '㏈', '㎧', '㎨', '㎰', '㎱', '㎲', '㎳', '㎴',
    '㎵', '㎶', '㎷', '㎸', '㎹', '㎀', '㎁', '㎂', '㎃', '㎄', '㎺', '㎻', '㎼', '㎽', '㎾', '㎿',
    '㎐', '㎑', '㎒', '㎓', '㎔', 'Ω', '㏀', '㏁', '㎊', '㎋', '㎌', '㏖', '㏅', '㎭', '㎮', '㎯',
    '㏛', '㎩', '㎪', '㎫', '㎬', '㏝', '㏐', '㏓', '㏃', '㏉', '㏜', '㏆',
];

/// ㅁ — 도형기호/일반기호
const CHARS_MIEUM: &[char] = &[
    '＃', '＆', '＊', '＠', '§', '※', '☆', '★', '○', '●', '◎', '◇', '◆', '□', '■', '△', '▲', '▽',
    '▼', '→', '←', '↑', '↓', '↔', '〓', '◁', '◀', '▷', '▶', '♤', '♠', '♡', '♥', '♧', '♣', '◈', '▣',
    '◐', '◑', '▤', '▥', '▨', '▧', '▦', '▩', '♨', '☏', '☎', '☜', '☞', '¶', '†', '‡', '↕', '↗', '↙',
    '↖', '↘', '♭', '♩', '♪', '♬', '㉿', '㈜', '№', '㏇', '™', '㏂', '㏘', '℡', 'ª', 'º',
];

/// ㅂ — 선문자 (테이블, 박스 드로잉)
const CHARS_BIEUP: &[char] = &[
    '─', '│', '┌', '┐', '┘', '└', '├', '┬', '┤', '┴', '┼', '━', '┃', '┏', '┓', '┛', '┗', '┣', '┳',
    '┫', '┻', '╋', '┠', '┯', '┨', '┷', '┿', '┝', '┰', '┥', '┸', '╂', '┒', '┑', '┚', '┙', '┖', '┕',
    '┎', '┍', '┞', '┟', '┡', '┢', '┦', '┧', '┩', '┪', '┭', '┮', '┱', '┲', '┵', '┶', '┹', '┺', '┽',
    '┾', '╀', '╁', '╃', '╄', '╅', '╆', '╇', '╈', '╉', '╊',
];

/// ㅅ — 한글 기호 (괄호 자모, 원 자모)
const CHARS_SIOT: &[char] = &[
    '㉠', '㉡', '㉢', '㉣', '㉤', '㉥', '㉦', '㉧', '㉨', '㉩', '㉪', '㉫', '㉬', '㉭', '㉮', '㉯',
    '㉰', '㉱', '㉲', '㉳', '㉴', '㉵', '㉶', '㉷', '㉸', '㉹', '㉺', '㉻', '㈀', '㈁', '㈂', '㈃',
    '㈄', '㈅', '㈆', '㈇', '㈈', '㈉', '㈊', '㈋', '㈌', '㈍', '㈎', '㈏', '㈐', '㈑', '㈒', '㈓',
    '㈔', '㈕', '㈖', '㈗', '㈘', '㈙', '㈚', '㈛',
];

/// ㅇ — 원문자 (영문, 숫자)
const CHARS_IEUNG: &[char] = &[
    'ⓐ', 'ⓑ', 'ⓒ', 'ⓓ', 'ⓔ', 'ⓕ', 'ⓖ', 'ⓗ', 'ⓘ', 'ⓙ', 'ⓚ', 'ⓛ', 'ⓜ', 'ⓝ', 'ⓞ', 'ⓟ', 'ⓠ', 'ⓡ', 'ⓢ',
    'ⓣ', 'ⓤ', 'ⓥ', 'ⓦ', 'ⓧ', 'ⓨ', 'ⓩ', 'Ⓐ', 'Ⓑ', 'Ⓒ', 'Ⓓ', 'Ⓔ', 'Ⓕ', 'Ⓖ', 'Ⓗ', 'Ⓘ', 'Ⓙ', 'Ⓚ', 'Ⓛ',
    'Ⓜ', 'Ⓝ', 'Ⓞ', 'Ⓟ', 'Ⓠ', 'Ⓡ', 'Ⓢ', 'Ⓣ', 'Ⓤ', 'Ⓥ', 'Ⓦ', 'Ⓧ', 'Ⓨ', 'Ⓩ', '①', '②', '③', '④', '⑤',
    '⑥', '⑦', '⑧', '⑨', '⑩', '⑪', '⑫', '⑬', '⑭', '⑮',
];

/// ㅈ — 로마숫자
const CHARS_JIEUT: &[char] = &[
    'Ⅰ', 'Ⅱ', 'Ⅲ', 'Ⅳ', 'Ⅴ', 'Ⅵ', 'Ⅶ', 'Ⅷ', 'Ⅸ', 'Ⅹ', 'Ⅺ', 'Ⅻ', 'ⅰ', 'ⅱ', 'ⅲ', 'ⅳ', 'ⅴ', 'ⅵ', 'ⅶ',
    'ⅷ', 'ⅸ', 'ⅹ', 'ⅺ', 'ⅻ',
];

/// ㅊ — 분수/첨자
const CHARS_CHIEUT: &[char] = &[
    '½', '⅓', '⅔', '¼', '¾', '⅛', '⅜', '⅝', '⅞', '¹', '²', '³', '⁴', 'ⁿ', '₁', '₂', '₃', '₄',
];

/// ㅋ — 괄호 숫자/이중원문자
const CHARS_KIEUK: &[char] = &[
    '⑴', '⑵', '⑶', '⑷', '⑸', '⑹', '⑺', '⑻', '⑼', '⑽', '⑾', '⑿', '⒀', '⒁', '⒂', '⒃', '⒄', '⒅', '⒆',
    '⒇',
];

/// ㅌ — 원 숫자/괄호 한글
const CHARS_TIEUT: &[char] = &[
    '⒜', '⒝', '⒞', '⒟', '⒠', '⒡', '⒢', '⒣', '⒤', '⒥', '⒦', '⒧', '⒨', '⒩', '⒪', '⒫', '⒬', '⒭', '⒮',
    '⒯', '⒰', '⒱', '⒲', '⒳', '⒴', '⒵',
];

/// ㅍ — 그리스/라틴 문자
const CHARS_PIEUP: &[char] = &[
    'Α', 'Β', 'Γ', 'Δ', 'Ε', 'Ζ', 'Η', 'Θ', 'Ι', 'Κ', 'Λ', 'Μ', 'Ν', 'Ξ', 'Ο', 'Π', 'Ρ', 'Σ', 'Τ',
    'Υ', 'Φ', 'Χ', 'Ψ', 'Ω', 'α', 'β', 'γ', 'δ', 'ε', 'ζ', 'η', 'θ', 'ι', 'κ', 'λ', 'μ', 'ν', 'ξ',
    'ο', 'π', 'ρ', 'σ', 'τ', 'υ', 'φ', 'χ', 'ψ', 'ω',
];

/// ㅎ — 기타기호 (카드, 음악, 날씨 등)
const CHARS_HIEUT: &[char] = &[
    '♠', '♤', '♥', '♡', '♣', '♧', '♦', '◇', '★', '☆', '■', '□', '▲', '△', '▼', '▽', '◆', '◇', '○',
    '●', '◎', '⊙', '◈', '♨', '☏', '☎', '☜', '☞', '☝', '☟', '✓', '✗', '✕', '♩', '♪', '♬', '♭', '♯',
];

/// 전체 초성 특수문자 매핑 테이블
static SPECIAL_CHAR_TABLE: &[SpecialCharEntry] = &[
    SpecialCharEntry {
        choseong: 'ㄱ',
        category: "특수기호",
        characters: CHARS_GIYEOK,
    },
    SpecialCharEntry {
        choseong: 'ㄴ',
        category: "괄호류",
        characters: CHARS_NIEUN,
    },
    SpecialCharEntry {
        choseong: 'ㄷ',
        category: "수학기호",
        characters: CHARS_DIGEUT,
    },
    SpecialCharEntry {
        choseong: 'ㄹ',
        category: "단위기호",
        characters: CHARS_RIEUL,
    },
    SpecialCharEntry {
        choseong: 'ㅁ',
        category: "도형기호",
        characters: CHARS_MIEUM,
    },
    SpecialCharEntry {
        choseong: 'ㅂ',
        category: "선문자",
        characters: CHARS_BIEUP,
    },
    SpecialCharEntry {
        choseong: 'ㅅ',
        category: "한글기호",
        characters: CHARS_SIOT,
    },
    SpecialCharEntry {
        choseong: 'ㅇ',
        category: "원문자",
        characters: CHARS_IEUNG,
    },
    SpecialCharEntry {
        choseong: 'ㅈ',
        category: "로마숫자",
        characters: CHARS_JIEUT,
    },
    SpecialCharEntry {
        choseong: 'ㅊ',
        category: "분수/첨자",
        characters: CHARS_CHIEUT,
    },
    SpecialCharEntry {
        choseong: 'ㅋ',
        category: "괄호숫자",
        characters: CHARS_KIEUK,
    },
    SpecialCharEntry {
        choseong: 'ㅌ',
        category: "괄호영문",
        characters: CHARS_TIEUT,
    },
    SpecialCharEntry {
        choseong: 'ㅍ',
        category: "그리스문자",
        characters: CHARS_PIEUP,
    },
    SpecialCharEntry {
        choseong: 'ㅎ',
        category: "기타기호",
        characters: CHARS_HIEUT,
    },
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_all_choseong_mapped() {
        // 14개 기본 초성 모두 매핑 확인
        let choseongs = [
            'ㄱ', 'ㄴ', 'ㄷ', 'ㄹ', 'ㅁ', 'ㅂ', 'ㅅ', 'ㅇ', 'ㅈ', 'ㅊ', 'ㅋ', 'ㅌ', 'ㅍ', 'ㅎ',
        ];
        for ch in choseongs {
            let result = search_by_choseong(ch);
            assert!(result.is_some(), "'{}' 매핑이 없음", ch);
            assert!(
                !result.unwrap().characters.is_empty(),
                "'{}' 문자 목록이 비어 있음",
                ch
            );
        }
    }

    #[test]
    fn test_search_giyeok() {
        let result = search_by_choseong('ㄱ');
        assert!(result.is_some());
        let entry = result.unwrap();
        assert_eq!(entry.choseong, 'ㄱ');
        assert_eq!(entry.category, "특수기호");
        assert!(entry.characters.len() > 10);
    }

    #[test]
    fn test_search_non_choseong() {
        // 완성형 음절은 매핑 없음
        assert!(search_by_choseong('가').is_none());
        assert!(search_by_choseong('a').is_none());
        assert!(search_by_choseong('1').is_none());
    }

    #[test]
    fn test_is_choseong() {
        // 기본 자음
        assert!(is_choseong('ㄱ'));
        assert!(is_choseong('ㅎ'));
        assert!(is_choseong('ㄴ'));

        // 쌍자음
        assert!(is_choseong('ㄲ'));
        assert!(is_choseong('ㄸ'));
        assert!(is_choseong('ㅃ'));
        assert!(is_choseong('ㅆ'));
        assert!(is_choseong('ㅉ'));

        // 비-자음
        assert!(!is_choseong('가'));
        assert!(!is_choseong('a'));
        assert!(!is_choseong('ㅏ')); // 모음
    }

    #[test]
    fn test_double_choseong_no_mapping() {
        // 쌍자음은 현재 매핑 없음 (Windows에서도 기본 자음만 지원)
        assert!(search_by_choseong('ㄲ').is_none());
        assert!(search_by_choseong('ㄸ').is_none());
    }

    #[test]
    fn test_table_count() {
        assert_eq!(SPECIAL_CHAR_TABLE.len(), 14);
    }
}
