//! GitHub 호환 헤딩 슬러그 생성.
//!
//! `docs/user/**` 의 문서들은 서로를 `#42-한자-변환-hanja` 같은 GitHub 앵커로 링크한다.
//! 병합 HTML 에서 그 링크를 내부 앵커로 되살리려면 GitHub 이 붙이는 것과 **같은** id 를
//! 만들어야 한다. github-slugger 의 규칙을 그대로 따른다:
//!
//! 1. 소문자화
//! 2. 영숫자(유니코드 포함)·`-`·`_`·공백 이외의 문자는 **삭제**(하이픈으로 치환하지 않는다)
//! 3. 공백을 `-` 로 치환
//! 4. 같은 문서 안에서 중복된 슬러그는 `-1`, `-2` … 접미사로 구분
//!
//! 2번이 "삭제"인 점이 중요하다. `5.6 ... (Keymap Studio / Typing Practice)` 는
//! `/` 양옆 공백이 남아 `keymap-studio--typing-practice` 처럼 하이픈 2개가 된다.

use std::collections::HashMap;

/// 문서 하나에 대한 슬러그 발급기. 중복 슬러그에 GitHub 과 같은 순번을 붙인다.
#[derive(Default)]
pub struct Slugger {
    seen: HashMap<String, usize>,
}

impl Slugger {
    pub fn new() -> Self {
        Self::default()
    }

    /// 헤딩의 **평문** 텍스트(인라인 마크업이 제거된 상태)를 받아 id 를 발급한다.
    pub fn slug(&mut self, text: &str) -> String {
        let base = base_slug(text);
        match self.seen.get_mut(&base) {
            Some(count) => {
                *count += 1;
                format!("{base}-{count}")
            }
            None => {
                self.seen.insert(base.clone(), 0);
                base
            }
        }
    }
}

fn base_slug(text: &str) -> String {
    text.to_lowercase()
        .chars()
        .filter(|c| c.is_alphanumeric() || *c == '-' || *c == '_' || *c == ' ')
        .map(|c| if c == ' ' { '-' } else { c })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn punctuation_is_dropped_not_replaced() {
        assert_eq!(
            base_slug("5.6 Keyboard Layout Tools (Keymap Studio / Typing Practice)"),
            "56-keyboard-layout-tools-keymap-studio--typing-practice"
        );
    }

    #[test]
    fn hangul_survives() {
        assert_eq!(base_slug("4.2 한자 변환 (Hanja)"), "42-한자-변환-hanja");
        assert_eq!(base_slug("빌드 실패"), "빌드-실패");
    }

    #[test]
    fn em_dash_and_symbols_are_dropped() {
        assert_eq!(base_slug("방법 1 — 자동 설치"), "방법-1--자동-설치");
    }

    #[test]
    fn duplicates_get_numeric_suffix() {
        let mut s = Slugger::new();
        assert_eq!(s.slug("Notes"), "notes");
        assert_eq!(s.slug("Notes"), "notes-1");
        assert_eq!(s.slug("Notes"), "notes-2");
    }
}
