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

/// 한자 사전
///
/// 한글 발음을 키로 한자 후보 목록을 검색합니다.
/// 사전은 빌드 시 임베드된 데이터로 초기화됩니다.
pub struct HanjaDictionary {
    /// 한글 발음 -> 한자 목록 매핑
    entries: HashMap<String, Vec<HanjaEntry>>,
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
        let entries = Self::parse_dictionary(HANJA_DATA);
        Self { entries }
    }

    /// 사전 데이터를 파싱합니다.
    fn parse_dictionary(data: &str) -> HashMap<String, Vec<HanjaEntry>> {
        let mut entries: HashMap<String, Vec<HanjaEntry>> = HashMap::new();

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

                entries.entry(hangul).or_default().push(entry);
            }
        }

        entries
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

    /// 사전에 포함된 총 항목 수를 반환합니다.
    pub fn entry_count(&self) -> usize {
        self.entries.values().map(|v| v.len()).sum()
    }

    /// 사전에 포함된 고유 한글 키 수를 반환합니다.
    pub fn key_count(&self) -> usize {
        self.entries.len()
    }
}

/// 사용자 사전
///
/// 한자 선택 빈도를 학습하여 자주 쓰는 한자를 상위에 노출합니다.
/// JSON 형식으로 `~/.local/share/unim/user_dict.json`에 저장됩니다.
pub struct UserDictionary {
    /// 한글 → { 한자 → 선택 횟수 }
    frequencies: HashMap<String, HashMap<String, u32>>,
    /// 파일 경로
    path: Option<std::path::PathBuf>,
}

impl UserDictionary {
    /// 기본 경로에서 사용자 사전을 로드합니다.
    pub fn load_default() -> Self {
        let path = Self::default_path();
        Self::load_from_path(path)
    }

    /// 지정된 경로에서 사용자 사전을 로드합니다.
    pub fn load_from_path(path: Option<std::path::PathBuf>) -> Self {
        let frequencies = if let Some(ref p) = path {
            match std::fs::read_to_string(p) {
                Ok(content) => {
                    serde_json::from_str(&content).unwrap_or_default()
                }
                Err(_) => HashMap::new(),
            }
        } else {
            HashMap::new()
        };

        Self { frequencies, path }
    }

    /// 기본 사용자 사전 경로를 반환합니다.
    fn default_path() -> Option<std::path::PathBuf> {
        dirs::data_dir().map(|p| p.join("unim").join("user_dict.json"))
    }

    /// 한자 선택을 기록합니다.
    pub fn record_selection(&mut self, hangul: &str, hanja: &str) {
        let entry = self
            .frequencies
            .entry(hangul.to_string())
            .or_default();
        let count = entry.entry(hanja.to_string()).or_insert(0);
        *count += 1;

        // 자동 저장
        self.save();
    }

    /// 한자 후보를 빈도순으로 정렬합니다.
    ///
    /// 사용자가 선택한 빈도가 높은 항목을 앞으로 이동합니다.
    /// 원본 목록을 변경하지 않고 새 목록을 반환합니다.
    pub fn sort_by_frequency(&self, hangul: &str, candidates: &[HanjaEntry]) -> Vec<HanjaEntry> {
        let mut sorted = candidates.to_vec();
        if let Some(freq_map) = self.frequencies.get(hangul) {
            sorted.sort_by(|a, b| {
                let freq_a = freq_map.get(&a.hanja).unwrap_or(&0);
                let freq_b = freq_map.get(&b.hanja).unwrap_or(&0);
                freq_b.cmp(freq_a) // 내림차순
            });
        }
        sorted
    }

    /// 사전을 파일에 저장합니다.
    fn save(&self) {
        if let Some(ref p) = self.path {
            if let Some(parent) = p.parent() {
                let _ = std::fs::create_dir_all(parent);
            }
            if let Ok(content) = serde_json::to_string_pretty(&self.frequencies) {
                let _ = std::fs::write(p, content);
            }
        }
    }
}

impl Default for UserDictionary {
    fn default() -> Self {
        Self::load_default()
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
}
