//! 한자 사전 모듈
//!
//! libhangul의 hanja.txt 사전 파일을 파싱하고 검색하는 기능을 제공합니다.
//! 사전 데이터는 빌드 시 바이너리에 임베드됩니다.

use std::collections::HashMap;

/// 한자 사전 데이터 (빌드 시 임베드)
const HANJA_DATA: &str = include_str!("../data/hanja.txt");

/// 한자 항목
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HanjaEntry {
    /// 한글 발음 (검색 키)
    pub hangul: String,
    /// 한자 문자열 (한자 + 혼합 문자 포함 가능)
    pub hanja: String,
    /// 뜻풀이
    pub meaning: String,
}

impl HanjaEntry {
    /// 한자의 첫 번째 문자를 반환합니다.
    /// 순수 한자 문자가 필요한 경우 사용합니다.
    pub fn first_hanja_char(&self) -> Option<char> {
        self.hanja.chars().next()
    }
}

/// `parse_dictionary` 의 반환 묶음.
///
/// (한글 발음 → 항목 목록, (한자, 음) → 글자별 뜻, 한자 → 글자별 뜻 폴백)
type ParsedDictionary = (
    HashMap<String, Vec<HanjaEntry>>,
    HashMap<(char, char), String>,
    HashMap<char, String>,
);

/// 한자 사전
///
/// 한글 발음을 키로 한자 후보 목록을 검색합니다.
/// 사전은 빌드 시 임베드된 데이터로 초기화됩니다.
pub struct HanjaDictionary {
    /// 한글 발음 -> 한자 목록 매핑
    entries: HashMap<String, Vec<HanjaEntry>>,
    /// 단음절 표제어의 글자별 뜻 역색인.
    ///
    /// 키: (한자, 그 표제어의 한글 발음 1글자). 값: 뜻의 첫 항목(쉼표 앞).
    /// 다음절 후보의 뜻을 글자별로 합성할 때(`display_meaning`) 쓴다.
    char_meaning: HashMap<(char, char), String>,
    /// `char_meaning` 폴백(한자 단독 → 첫 등장 뜻).
    ///
    /// 두음법칙·다독음으로 (한자, 단어 속 음) 조합이 `char_meaning` 에 없을 때 쓴다.
    /// 예: `역사:歷史` 의 歷 은 단음절 표제어가 "력"이라 (歷,'역') 키가 없다.
    char_meaning_fallback: HashMap<char, String>,
}

impl Default for HanjaDictionary {
    fn default() -> Self {
        Self::new()
    }
}

impl HanjaDictionary {
    /// 새 한자 사전을 생성합니다.
    ///
    /// 임베드된 사전 데이터를 파싱하여 초기화합니다.
    pub fn new() -> Self {
        let (entries, char_meaning, char_meaning_fallback) = Self::parse_dictionary(HANJA_DATA);
        Self {
            entries,
            char_meaning,
            char_meaning_fallback,
        }
    }

    /// 사전 데이터를 파싱합니다.
    ///
    /// 한글 발음 목록과 함께, 단음절 표제어로부터 글자별 뜻 역색인·폴백도 같이 만든다
    /// (`display_meaning` 이 다음절 후보의 뜻을 합성할 때 쓴다).
    fn parse_dictionary(data: &str) -> ParsedDictionary {
        let mut entries: HashMap<String, Vec<HanjaEntry>> = HashMap::new();
        let mut char_meaning: HashMap<(char, char), String> = HashMap::new();
        let mut char_meaning_fallback: HashMap<char, String> = HashMap::new();

        for line in data.lines() {
            // 주석 및 빈 줄 무시
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }

            // 형식: 한글:한자:설명
            let parts: Vec<&str> = line.splitn(3, ':').collect();
            if parts.len() >= 2 {
                let hangul = parts[0].to_string();
                let hanja = parts[1].to_string();
                let meaning = if parts.len() >= 3 {
                    parts[2].to_string()
                } else {
                    String::new()
                };

                // 빈 항목 무시
                if hangul.is_empty() || hanja.is_empty() {
                    continue;
                }

                let entry = HanjaEntry {
                    hangul: hangul.clone(),
                    hanja,
                    meaning,
                };

                // 단음절 표제어만 글자별 뜻 역색인 대상으로 삼는다.
                if entry.hangul.chars().count() == 1 {
                    if let (Some(hangul_char), Some(hanja_char)) =
                        (entry.hangul.chars().next(), entry.first_hanja_char())
                    {
                        let first_meaning = entry
                            .meaning
                            .split(',')
                            .next()
                            .map(str::trim)
                            .filter(|s| !s.is_empty());
                        if let Some(first_meaning) = first_meaning {
                            char_meaning
                                .entry((hanja_char, hangul_char))
                                .or_insert_with(|| first_meaning.to_string());
                            char_meaning_fallback
                                .entry(hanja_char)
                                .or_insert_with(|| first_meaning.to_string());
                        }
                    }
                }

                entries.entry(hangul).or_default().push(entry);
            }
        }

        (entries, char_meaning, char_meaning_fallback)
    }

    /// 한글 발음으로 한자를 검색합니다.
    ///
    /// # 인자
    ///
    /// * `hangul` - 검색할 한글 발음 (예: "가", "한")
    ///
    /// # 반환
    ///
    /// 해당 발음의 한자 후보 목록. 없으면 빈 벡터.
    /// 결과는 빈도순 (사전 내 순서)으로 정렬되어 있습니다.
    pub fn search(&self, hangul: &str) -> Vec<HanjaEntry> {
        self.entries.get(hangul).cloned().unwrap_or_default()
    }

    /// 사전에 해당 한글 발음의 항목이 있는지 확인합니다.
    ///
    /// 선택 영역(대상②) 판정처럼 후보 목록 자체는 필요 없고
    /// 존재 여부만 필요할 때 `search` 대신 쓴다.
    pub fn contains(&self, hangul: &str) -> bool {
        self.entries.contains_key(hangul)
    }

    /// 한글 음절의 마지막 문자로 한자를 검색합니다.
    ///
    /// preedit 문자열에서 마지막 음절만 추출하여 검색합니다.
    ///
    /// # 인자
    ///
    /// * `text` - 입력 텍스트 (예: "대한민국" → "국"으로 검색)
    ///
    /// # 반환
    ///
    /// (검색된 음절, 한자 후보 목록) 튜플. 음절이 없거나 결과가 없으면 None.
    pub fn search_last_syllable(&self, text: &str) -> Option<(String, Vec<HanjaEntry>)> {
        let last_char = text.chars().last()?;
        let key = last_char.to_string();
        let results = self.search(&key);
        if results.is_empty() {
            None
        } else {
            Some((key, results))
        }
    }

    /// 후보 팝업에 표시할 뜻을 계산합니다.
    ///
    /// 원본 뜻(`entry.meaning`)이 있고 표제어(`entry.hangul`)와 다르면 그대로 쓴다.
    /// 다음절 항목은 뜻이 비어 있거나 표제어를 되풀이하는 경우가 많아(예: `국가:國家:`)
    /// 그 경우 글자별 뜻을 `char_meaning` 역색인으로 합성해 `" · "` 로 이어 붙인다.
    /// 역색인에 없는 글자(두음법칙·다독음)는 `char_meaning_fallback`(한자 단독,
    /// 첫 등장 뜻)으로 대신해 빈칸을 남기지 않는다.
    ///
    /// 단, 폴백은 **다음절에만** 쓴다. 단음절은 그 글자 자체가 표제어라 폴백이
    /// 곧 다른 음의 뜻이 된다(예: `가:价:` → '착할 개'). 뜻이 빈 단음절은 빈 채로 둔다.
    pub fn display_meaning(&self, entry: &HanjaEntry) -> String {
        if !entry.meaning.is_empty() && entry.meaning != entry.hangul {
            return entry.meaning.clone();
        }

        let hangul_chars: Vec<char> = entry.hangul.chars().collect();
        let hanja_chars: Vec<char> = entry.hanja.chars().collect();
        if hangul_chars.is_empty() || hangul_chars.len() != hanja_chars.len() {
            // 글자 수가 어긋나면(혼합 문자 등) 합성 불가 — 원본 그대로 반환
            return entry.meaning.clone();
        }

        let allow_fallback = hangul_chars.len() > 1;
        hangul_chars
            .into_iter()
            .zip(hanja_chars)
            .filter_map(|(h, c)| {
                self.char_meaning
                    .get(&(c, h))
                    .or_else(|| {
                        allow_fallback
                            .then(|| self.char_meaning_fallback.get(&c))
                            .flatten()
                    })
                    .cloned()
            })
            .collect::<Vec<_>>()
            .join(" · ")
    }

    /// 사전에 포함된 총 항목 수를 반환합니다.
    pub fn entry_count(&self) -> usize {
        self.entries.values().map(|v| v.len()).sum()
    }

    /// 사전에 포함된 고유 한글 키 수를 반환합니다.
    pub fn key_count(&self) -> usize {
        self.entries.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dictionary_load() {
        let dict = HanjaDictionary::new();
        // 사전이 비어있지 않은지 확인
        assert!(dict.entry_count() > 0, "사전이 비어 있음");
        assert!(dict.key_count() > 0, "키가 없음");
    }

    #[test]
    fn test_search_single_syllable() {
        let dict = HanjaDictionary::new();

        // "가" 검색
        let results = dict.search("가");
        assert!(!results.is_empty(), "'가' 검색 결과가 없음");

        // 첫 번째 결과 확인 (可, 家 등이 있어야 함)
        let first = &results[0];
        assert_eq!(first.hangul, "가");
        assert!(!first.hanja.is_empty());
    }

    #[test]
    fn test_search_last_syllable() {
        let dict = HanjaDictionary::new();

        // "대한민국" → "국"으로 검색
        let result = dict.search_last_syllable("대한민국");
        assert!(result.is_some());

        let (syllable, entries) = result.unwrap();
        assert_eq!(syllable, "국");
        assert!(!entries.is_empty());
    }

    #[test]
    fn test_search_not_found() {
        let dict = HanjaDictionary::new();

        // 존재하지 않는 키 검색
        let results = dict.search("ㅋㅋㅋ");
        assert!(results.is_empty());
    }

    #[test]
    fn test_first_hanja_char() {
        let entry = HanjaEntry {
            hangul: "가".to_string(),
            hanja: "可".to_string(),
            meaning: "옳을 가".to_string(),
        };
        assert_eq!(entry.first_hanja_char(), Some('可'));
    }

    #[test]
    fn test_contains() {
        let dict = HanjaDictionary::new();
        assert!(dict.contains("대한민국"));
        assert!(!dict.contains("뷁"));
    }

    #[test]
    fn test_display_meaning_composes_multisyllable() {
        // 국가:國家: — 뜻이 비어 있어 글자별 뜻(국:國:나라 국, 가:家:집 가)을 합성해야 한다.
        let dict = HanjaDictionary::new();
        let entry = dict
            .search("국가")
            .into_iter()
            .find(|e| e.hanja == "國家")
            .expect("국가:國家 항목이 사전에 있어야 함");
        assert_eq!(dict.display_meaning(&entry), "나라 국 · 집 가");
    }

    #[test]
    fn test_display_meaning_falls_back_on_readings_mismatch() {
        // 역사:歷史: — 歷 은 두음법칙으로 단음절 표제어가 "력"이라 (歷,'역') 역색인이 미스한다.
        // 폴백(한자 단독)으로 빈 항 없이 채워져야 한다.
        let dict = HanjaDictionary::new();
        let entry = dict
            .search("역사")
            .into_iter()
            .find(|e| e.hanja == "歷史")
            .expect("역사:歷史 항목이 사전에 있어야 함");
        let meaning = dict.display_meaning(&entry);
        assert!(!meaning.is_empty(), "빈 뜻이면 안 됨");
        assert!(
            meaning.split(" · ").all(|part| !part.is_empty()),
            "빈 항이 섞이면 안 됨: {meaning:?}"
        );
    }

    #[test]
    fn test_display_meaning_keeps_original_when_present() {
        // 단음절 등 원본 뜻이 있고 표제어와 다르면 합성하지 않고 원본을 그대로 쓴다.
        let dict = HanjaDictionary::new();
        let entry = dict
            .search("가")
            .into_iter()
            .find(|e| e.hanja == "家")
            .expect("가:家 항목이 사전에 있어야 함");
        assert_eq!(dict.display_meaning(&entry), "집 가");
    }

    #[test]
    fn test_display_meaning_single_syllable_never_borrows_other_reading() {
        // 뜻이 빈 단음절은 폴백으로 다른 음의 뜻을 빌려 오면 안 된다
        // (가:价: 가 '착할 개' 로 보이던 결함 — 사전 실측 407건).
        let dict = HanjaDictionary::new();
        let mut borrowed = Vec::new();
        for entries in dict.entries.values() {
            for e in entries {
                if e.hangul.chars().count() != 1 || !e.meaning.is_empty() {
                    continue;
                }
                let shown = dict.display_meaning(e);
                let reading = e.hangul.chars().next().unwrap();
                if !shown.is_empty() && !shown.ends_with(reading) {
                    borrowed.push(format!("{}:{}→{}", e.hangul, e.hanja, shown));
                }
            }
        }
        assert!(borrowed.is_empty(), "다른 음 뜻 차용 {}건: {:?}", borrowed.len(), &borrowed[..borrowed.len().min(5)]);
    }
}
