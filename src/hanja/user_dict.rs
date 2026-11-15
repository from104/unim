//! 한자 사용자 사전
//!
//! 내장 한자 사전(libhangul `hanja.txt`)에 없는 이름·기관명·전문용어를 사용자가
//! `한글 → 한자(+뜻)` 로 등록한다. 규격: `docs/dev/specs/HANJA_USERDICT_SPEC.md`.
//!
//! - 파일: `<config_dir>/unim/hanja-userdict.yaml` (`version: 1`)
//! - 읽을 수 없는 파일은 **절대 덮어쓰지 않는다**(호출자는 [`HanjaUserDict::load`] 의
//!   `Err` 를 보고 쓰기를 거부하고, [`HanjaUserDict::quarantine`] 으로 보관만 한다).
//! - 무효 항목은 파일·메모리에 **보존**하되 조회 인덱스([`HanjaUserIndex`])에서 뺀다.
//! - 프로세스 전역 캐시 [`shared_index`] 는 호출마다 `stat` 1회, 서명 `(mtime, len)` 이
//!   바뀐 때만 다시 파싱한다.
//! - 로그에는 개수·행·열만 남긴다. 한글·한자·뜻 평문과 serde_yaml 오류 문장은 금지.

use super::dict::HanjaEntry;
use crate::input_engine::HANJA_MAX_KEY_CHARS;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::ffi::OsString;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::SystemTime;

/// 파일 형식 버전.
pub const USERDICT_VERSION: u8 = 1;
/// 뜻 최대 글자 수.
pub const MEANING_MAX_CHARS: usize = 60;

fn default_version() -> u8 {
    USERDICT_VERSION
}

/// 사용자 사전 항목.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct HanjaUserEntry {
    pub hangul: String,
    pub hanja: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub meaning: Option<String>,
    /// 추가된 Unix 시각(초).
    #[serde(default)]
    pub added_at: u64,
}

/// 사용자 사전 컨테이너.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct HanjaUserDict {
    #[serde(default = "default_version")]
    pub version: u8,
    #[serde(default)]
    pub entries: Vec<HanjaUserEntry>,
}

impl Default for HanjaUserDict {
    fn default() -> Self {
        Self {
            version: USERDICT_VERSION,
            entries: Vec::new(),
        }
    }
}

/// 항목 검증 오류. `pos` 는 0부터 세는 글자 위치.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum EntryError {
    HangulEmpty,
    HangulNotSyllable { ch: char, pos: usize },
    HangulTooLong { len: usize },
    HanjaLenMismatch { hangul: usize, hanja: usize },
    HanjaNotHan { ch: char, pos: usize },
    MeaningTooLong { len: usize },
    MeaningControl,
    Duplicate,
    NotFound,
}

/// 파일 읽기 오류.
#[derive(Debug)]
pub enum LoadError {
    Io(std::io::Error),
    /// `message` 는 화면 표시용. **로그에 남기지 말 것**(항목 내용이 섞일 수 있다).
    Parse {
        line: Option<usize>,
        column: Option<usize>,
        message: String,
    },
    NewerVersion(u8),
}

impl std::fmt::Display for LoadError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LoadError::Io(e) => write!(f, "{e}"),
            LoadError::Parse {
                line,
                column,
                message,
            } => match (line, column) {
                (Some(l), Some(c)) => write!(f, "{l}행 {c}열: {message}"),
                _ => write!(f, "{message}"),
            },
            LoadError::NewerVersion(v) => write!(f, "version {v}"),
        }
    }
}

impl std::error::Error for LoadError {}

// ───────────────────────── 검증 ─────────────────────────

fn is_syllable(c: char) -> bool {
    ('\u{AC00}'..='\u{D7A3}').contains(&c)
}

/// CJK 표의문자(통합 확장 A–H·호환·호환 보충). 부수·〇·々 는 불가.
fn is_han(c: char) -> bool {
    matches!(c as u32,
        0x3400..=0x4DBF
        | 0x4E00..=0x9FFF
        | 0xF900..=0xFAFF
        | 0x20000..=0x2A6DF
        | 0x2A700..=0x2EE5F
        | 0x2F800..=0x2FA1F
        | 0x30000..=0x323AF)
}

fn is_meaning_control(c: char) -> bool {
    c.is_control() || matches!(c, '\u{2028}' | '\u{2029}')
}

/// 다듬지 않고 그대로 검사한다(저장된 항목 검사용). 중복은 보지 않는다.
fn check_fields(hangul: &str, hanja: &str, meaning: Option<&str>) -> Result<(), EntryError> {
    let hangul_len = hangul.chars().count();
    if hangul_len == 0 {
        return Err(EntryError::HangulEmpty);
    }
    if let Some((pos, ch)) = hangul.chars().enumerate().find(|(_, c)| !is_syllable(*c)) {
        return Err(EntryError::HangulNotSyllable { ch, pos });
    }
    if hangul_len > HANJA_MAX_KEY_CHARS {
        return Err(EntryError::HangulTooLong { len: hangul_len });
    }
    let hanja_len = hanja.chars().count();
    if hanja_len != hangul_len {
        return Err(EntryError::HanjaLenMismatch {
            hangul: hangul_len,
            hanja: hanja_len,
        });
    }
    if let Some((pos, ch)) = hanja.chars().enumerate().find(|(_, c)| !is_han(*c)) {
        return Err(EntryError::HanjaNotHan { ch, pos });
    }
    if let Some(m) = meaning {
        if m.chars().any(is_meaning_control) {
            return Err(EntryError::MeaningControl);
        }
        let len = m.chars().count();
        if len > MEANING_MAX_CHARS {
            return Err(EntryError::MeaningTooLong { len });
        }
    }
    Ok(())
}

// ───────────────────────── 시각·서명 ─────────────────────────

fn now_unix() -> u64 {
    SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// 파일 서명 `(mtime, 길이)`. 파일이 없거나 stat 실패면 `None`.
type Signature = (SystemTime, u64);

fn file_signature(path: &Path) -> Option<Signature> {
    let m = fs::metadata(path).ok()?;
    Some((m.modified().ok()?, m.len()))
}

fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|p| p.into_inner())
}

// ───────────────────────── HanjaUserDict ─────────────────────────

impl HanjaUserDict {
    /// 기본 경로: `<config_dir>/unim/hanja-userdict.yaml`
    pub fn default_path() -> Option<PathBuf> {
        crate::paths::config_dir().map(|p| p.join("unim").join("hanja-userdict.yaml"))
    }

    /// 파일을 읽는다. 파일이 없으면 빈 사전. `version > 1` 이면 `NewerVersion`.
    pub fn load(path: &Path) -> Result<Self, LoadError> {
        let d = Self::load_lenient(path)?;
        if d.version > USERDICT_VERSION {
            return Err(LoadError::NewerVersion(d.version));
        }
        Ok(d)
    }

    /// `load` 와 같되 `version > 1` 도 최선으로 읽어 돌려준다(엔진 조회용).
    fn load_lenient(path: &Path) -> Result<Self, LoadError> {
        let content = match fs::read_to_string(path) {
            Ok(c) => c,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Self::default()),
            Err(e) => return Err(LoadError::Io(e)),
        };
        if content.trim().is_empty() {
            return Ok(Self::default());
        }
        serde_yaml::from_str::<Self>(&content).map_err(|e| {
            let loc = e.location();
            LoadError::Parse {
                line: loc.as_ref().map(|l| l.line()),
                column: loc.as_ref().map(|l| l.column()),
                message: e.to_string(),
            }
        })
    }

    pub fn load_default() -> Result<Self, LoadError> {
        match Self::default_path() {
            Some(p) => Self::load(&p),
            None => Ok(Self::default()),
        }
    }

    /// 원자적 저장. `version > 1` 이면 거부(모르는 필드 유실 방지).
    pub fn save(&self, path: &Path) -> std::io::Result<()> {
        if self.version > USERDICT_VERSION {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "hanja-userdict: 더 새 버전 파일은 수정하지 않는다",
            ));
        }
        let content = serde_yaml::to_string(self)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
        crate::atomic_io::atomic_write(path, content)
    }

    pub fn save_default(&self) -> std::io::Result<()> {
        let Some(path) = Self::default_path() else {
            return Err(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                "hanja-userdict 경로를 찾을 수 없음",
            ));
        };
        self.save(&path)
    }

    /// 입력을 다듬고(앞뒤 공백 제거, 유니코드 정규화 없음) 검증한다.
    /// 중복은 보지 않는다(`add`/`update_at` 이 본다).
    pub fn validate(
        hangul: &str,
        hanja: &str,
        meaning: Option<&str>,
    ) -> Result<(String, String, Option<String>), EntryError> {
        let hangul = hangul.trim();
        let hanja = hanja.trim();
        let meaning = meaning.map(str::trim).filter(|m| !m.is_empty());
        check_fields(hangul, hanja, meaning)?;
        Ok((
            hangul.to_string(),
            hanja.to_string(),
            meaning.map(str::to_string),
        ))
    }

    /// 저장된 항목의 유효성(목록 ⚠ 표시용). 앞 항목과 같은 쌍이면 `Duplicate`.
    /// 범위를 벗어난 `idx` 는 `None`.
    pub fn entry_error(&self, idx: usize) -> Option<EntryError> {
        let e = self.entries.get(idx)?;
        if let Err(err) = check_fields(&e.hangul, &e.hanja, e.meaning.as_deref()) {
            return Some(err);
        }
        let dup = self.entries[..idx]
            .iter()
            .any(|p| p.hangul == e.hangul && p.hanja == e.hanja);
        dup.then_some(EntryError::Duplicate)
    }

    /// 항목별 오류를 한 번에(HashSet 으로 O(n)).
    fn scan_errors(&self) -> Vec<Option<EntryError>> {
        let mut seen: HashSet<(&str, &str)> = HashSet::new();
        self.entries
            .iter()
            .map(|e| {
                if let Err(err) = check_fields(&e.hangul, &e.hanja, e.meaning.as_deref()) {
                    return Some(err);
                }
                if seen.insert((e.hangul.as_str(), e.hanja.as_str())) {
                    None
                } else {
                    Some(EntryError::Duplicate)
                }
            })
            .collect()
    }

    /// `(hangul, hanja)` 쌍의 위치(앞뒤 공백은 무시하고 찾는다).
    pub fn find(&self, hangul: &str, hanja: &str) -> Option<usize> {
        let (h, j) = (hangul.trim(), hanja.trim());
        self.entries
            .iter()
            .position(|e| e.hangul == h && e.hanja == j)
    }

    /// 검증 후 맨 뒤에 추가하고 위치를 돌려준다.
    pub fn add(
        &mut self,
        hangul: &str,
        hanja: &str,
        meaning: Option<String>,
    ) -> Result<usize, EntryError> {
        let (h, j, m) = Self::validate(hangul, hanja, meaning.as_deref())?;
        if self.find(&h, &j).is_some() {
            return Err(EntryError::Duplicate);
        }
        self.entries.push(HanjaUserEntry {
            hangul: h,
            hanja: j,
            meaning: m,
            added_at: now_unix(),
        });
        Ok(self.entries.len() - 1)
    }

    /// 위치·`added_at` 을 유지한 채 고친다. 자기 자신은 중복 검사에서 제외.
    pub fn update_at(
        &mut self,
        idx: usize,
        hangul: &str,
        hanja: &str,
        meaning: Option<String>,
    ) -> Result<(), EntryError> {
        if idx >= self.entries.len() {
            return Err(EntryError::NotFound);
        }
        let (h, j, m) = Self::validate(hangul, hanja, meaning.as_deref())?;
        let dup = self
            .entries
            .iter()
            .enumerate()
            .any(|(i, e)| i != idx && e.hangul == h && e.hanja == j);
        if dup {
            return Err(EntryError::Duplicate);
        }
        let e = &mut self.entries[idx];
        e.hangul = h;
        e.hanja = j;
        e.meaning = m;
        Ok(())
    }

    /// 쌍 하나(`hanja` 지정) 또는 그 한글의 전부(`None`)를 지우고 지운 개수를 돌려준다.
    pub fn remove(&mut self, hangul: &str, hanja: Option<&str>) -> usize {
        let h = hangul.trim();
        let j = hanja.map(str::trim);
        let before = self.entries.len();
        self.entries.retain(|e| {
            let same_hanja = match j {
                None => true,
                Some(j) => e.hanja == j,
            };
            !(e.hangul == h && same_hanja)
        });
        before - self.entries.len()
    }

    pub fn remove_at(&mut self, idx: usize) -> Option<HanjaUserEntry> {
        (idx < self.entries.len()).then(|| self.entries.remove(idx))
    }

    /// 읽을 수 없는 파일을 `<path>.broken-YYYYMMDD-HHMMSS` 로 이름만 바꿔 보관한다.
    /// 같은 이름이 있으면 `-1`, `-2`… 를 붙여 덮어쓰지 않는다.
    pub fn quarantine(path: &Path) -> std::io::Result<PathBuf> {
        let stamp = chrono::Local::now().format("%Y%m%d-%H%M%S").to_string();
        let make = |suffix: &str| -> PathBuf {
            let mut s: OsString = path.as_os_str().to_owned();
            s.push(format!(".broken-{stamp}{suffix}"));
            PathBuf::from(s)
        };
        let mut target = make("");
        let mut n = 0u32;
        while target.exists() {
            n += 1;
            if n > 1000 {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::AlreadyExists,
                    "보관 이름을 정할 수 없음",
                ));
            }
            target = make(&format!("-{n}"));
        }
        fs::rename(path, &target)?;
        Ok(target)
    }
}

// ───────────────────────── 인덱스 ─────────────────────────

/// 조회용 인덱스: 한글 → 후보(파일 순). 유효 항목만 담는다.
#[derive(Clone, Debug, Default)]
pub struct HanjaUserIndex {
    map: HashMap<String, Vec<HanjaEntry>>,
    valid: usize,
    invalid: usize,
}

impl HanjaUserIndex {
    pub fn empty() -> Self {
        Self::default()
    }

    /// 유효 항목만, 파일 순서대로. 무효·중복 항목은 `invalid_count` 로만 센다.
    pub fn from_dict(d: &HanjaUserDict) -> Self {
        let mut idx = Self::default();
        for (e, err) in d.entries.iter().zip(d.scan_errors()) {
            if err.is_some() {
                idx.invalid += 1;
                continue;
            }
            idx.valid += 1;
            idx.map
                .entry(e.hangul.clone())
                .or_default()
                .push(HanjaEntry {
                    hangul: e.hangul.clone(),
                    hanja: e.hanja.clone(),
                    meaning: e.meaning.clone().unwrap_or_default(),
                });
        }
        idx
    }

    pub fn get(&self, hangul: &str) -> &[HanjaEntry] {
        self.map.get(hangul).map(Vec::as_slice).unwrap_or(&[])
    }

    pub fn contains(&self, hangul: &str) -> bool {
        self.map.contains_key(hangul)
    }

    pub fn valid_count(&self) -> usize {
        self.valid
    }

    pub fn invalid_count(&self) -> usize {
        self.invalid
    }
}

// ───────────────────────── 병합 ─────────────────────────

/// SPEC §3.1: 사용자 항목(순서) → 내장 항목(사용자와 같은 한자 제외).
/// 사용자 뜻이 비면 같은 한자의 내장 뜻을 쓴다.
pub fn merge_candidates(user: &[HanjaEntry], builtin: Vec<HanjaEntry>) -> Vec<HanjaEntry> {
    if user.is_empty() {
        return builtin;
    }
    let mut out: Vec<HanjaEntry> = Vec::with_capacity(user.len() + builtin.len());
    let mut seen: HashSet<&str> = HashSet::new();
    for u in user {
        if !seen.insert(u.hanja.as_str()) {
            continue;
        }
        let mut e = u.clone();
        if e.meaning.is_empty() {
            if let Some(b) = builtin.iter().find(|b| b.hanja == u.hanja) {
                e.meaning = b.meaning.clone();
            }
        }
        out.push(e);
    }
    out.extend(
        builtin
            .into_iter()
            .filter(|b| !seen.contains(b.hanja.as_str())),
    );
    out
}

// ───────────────────────── 프로세스 전역 캐시 ─────────────────────────

/// 파일 서명으로 재파싱 여부를 정하는 캐시. 경로를 받아 단위 시험할 수 있다.
struct UserLayerCache {
    path: Option<PathBuf>,
    /// 마지막으로 성공적으로 반영한 서명. `loaded` 가 false 면 무의미.
    sig: Option<Signature>,
    loaded: bool,
    index: Arc<HanjaUserIndex>,
    /// 경고를 이미 남긴 서명(같은 서명에 1회).
    warned_sig: Option<Option<Signature>>,
}

impl UserLayerCache {
    fn new(path: Option<PathBuf>) -> Self {
        Self {
            path,
            sig: None,
            loaded: false,
            index: Arc::new(HanjaUserIndex::empty()),
            warned_sig: None,
        }
    }

    fn refresh(&mut self) -> Arc<HanjaUserIndex> {
        let Some(path) = self.path.clone() else {
            return Arc::clone(&self.index);
        };
        let sig = file_signature(&path);
        if self.loaded && sig == self.sig {
            return Arc::clone(&self.index);
        }
        if sig.is_none() {
            // 파일 없음(또는 stat 실패) → 빈 층. 옛 내용을 붙들지 않는다.
            let had = self.index.valid_count() + self.index.invalid_count();
            self.index = Arc::new(HanjaUserIndex::empty());
            self.sig = None;
            self.loaded = true;
            if had > 0 {
                crate::unim_log!("ENGINE", "사용자 한자 사전 파일 없음 — 빈 층");
            }
            return Arc::clone(&self.index);
        }
        match HanjaUserDict::load_lenient(&path) {
            Ok(d) => {
                let idx = HanjaUserIndex::from_dict(&d);
                crate::unim_log!(
                    "ENGINE",
                    "사용자 한자 사전 로드: 유효 {}개, 무효 {}개",
                    idx.valid_count(),
                    idx.invalid_count()
                );
                self.index = Arc::new(idx);
                self.sig = sig;
                self.loaded = true;
            }
            Err(e) => {
                // 내장 사전은 계속 쓴다 → 빈 층. 내용이 섞일 수 있어 메시지는 남기지 않는다.
                self.index = Arc::new(HanjaUserIndex::empty());
                if self.warned_sig != Some(sig) {
                    self.warned_sig = Some(sig);
                    match &e {
                        LoadError::Parse { line, column, .. } => crate::unim_log!(
                            "ENGINE",
                            "사용자 한자 사전 파싱 실패(line {} col {}) — 내장 사전만 사용",
                            line.unwrap_or(0),
                            column.unwrap_or(0)
                        ),
                        LoadError::Io(_) => crate::unim_log!(
                            "ENGINE",
                            "사용자 한자 사전 읽기 실패 — 내장 사전만 사용"
                        ),
                        LoadError::NewerVersion(_) => {}
                    }
                }
                match e {
                    // 파싱 실패는 같은 서명이면 다시 읽지 않는다. I/O 실패는 매번 재시도.
                    LoadError::Io(_) => self.loaded = false,
                    _ => {
                        self.sig = sig;
                        self.loaded = true;
                    }
                }
            }
        }
        Arc::clone(&self.index)
    }
}

static OVERRIDE: Mutex<Option<Arc<HanjaUserIndex>>> = Mutex::new(None);

#[cfg(not(test))]
static SHARED: std::sync::OnceLock<Mutex<UserLayerCache>> = std::sync::OnceLock::new();

/// 프로세스 전역 사용자 층. 호출마다 `stat` 1회, 서명이 바뀐 때만 재파싱한다.
/// 패닉하지 않는다(Mutex 오염은 `into_inner` 로 계속).
/// 단위 시험(`cfg(test)`)의 기본값은 개발자의 실제 `~/.config` 를 읽지 않는 빈 층.
pub fn shared_index() -> Arc<HanjaUserIndex> {
    if let Some(o) = lock(&OVERRIDE).as_ref() {
        return Arc::clone(o);
    }
    #[cfg(test)]
    {
        Arc::new(HanjaUserIndex::empty())
    }
    #[cfg(not(test))]
    {
        let cache =
            SHARED.get_or_init(|| Mutex::new(UserLayerCache::new(HanjaUserDict::default_path())));
        lock(cache).refresh()
    }
}

/// 시험·타 크레이트 시험용. `Some` 이면 파일을 보지 않고 이것만 돌려준다.
#[doc(hidden)]
pub fn set_shared_override(idx: Option<Arc<HanjaUserIndex>>) {
    *lock(&OVERRIDE) = idx;
}

#[cfg(test)]
mod tests {
    use super::*;

    fn unique_temp_path(name: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        std::env::temp_dir().join(format!(
            "unim-hanja-ud-test-{}-{}-{}.yaml",
            std::process::id(),
            nanos,
            name
        ))
    }

    fn v(
        h: &str,
        j: &str,
        m: Option<&str>,
    ) -> Result<(String, String, Option<String>), EntryError> {
        HanjaUserDict::validate(h, j, m)
    }

    fn he(h: &str, j: &str, m: &str) -> HanjaEntry {
        HanjaEntry {
            hangul: h.into(),
            hanja: j.into(),
            meaning: m.into(),
        }
    }

    // ── 검증 ──

    #[test]
    fn validate_ok_and_trims() {
        let r = v(" 서기현 ", " 徐基鉉 ", Some("  내 이름  ")).unwrap();
        assert_eq!(
            r,
            ("서기현".into(), "徐基鉉".into(), Some("내 이름".into()))
        );
        // 공백뿐인 뜻은 없음
        assert_eq!(v("서", "徐", Some("   ")).unwrap().2, None);
        assert_eq!(v("서", "徐", None).unwrap().2, None);
    }

    #[test]
    fn validate_hangul_errors() {
        assert_eq!(v("", "", None), Err(EntryError::HangulEmpty));
        assert_eq!(v("   ", "徐", None), Err(EntryError::HangulEmpty));
        assert_eq!(
            v("ㄱ", "徐", None),
            Err(EntryError::HangulNotSyllable { ch: 'ㄱ', pos: 0 })
        );
        assert_eq!(
            v("서A", "徐基", None),
            Err(EntryError::HangulNotSyllable { ch: 'A', pos: 1 })
        );
        assert_eq!(
            v("서 기", "徐基", None),
            Err(EntryError::HangulNotSyllable { ch: ' ', pos: 1 })
        );
        assert_eq!(
            v("서1", "徐基", None),
            Err(EntryError::HangulNotSyllable { ch: '1', pos: 1 })
        );
    }

    #[test]
    fn validate_hangul_length_boundary() {
        let h18 = "가".repeat(18);
        let j18 = "徐".repeat(18);
        assert!(v(&h18, &j18, None).is_ok());
        let h19 = "가".repeat(19);
        let j19 = "徐".repeat(19);
        assert_eq!(
            v(&h19, &j19, None),
            Err(EntryError::HangulTooLong { len: 19 })
        );
    }

    #[test]
    fn validate_hanja_errors() {
        assert_eq!(
            v("서기현", "徐基", None),
            Err(EntryError::HanjaLenMismatch {
                hangul: 3,
                hanja: 2
            })
        );
        assert_eq!(
            v("서", "", None),
            Err(EntryError::HanjaLenMismatch {
                hangul: 1,
                hanja: 0
            })
        );
        assert_eq!(
            v("서기현", "徐A鉉", None),
            Err(EntryError::HanjaNotHan { ch: 'A', pos: 1 })
        );
        assert_eq!(
            v("다온", "多온", None),
            Err(EntryError::HanjaNotHan { ch: '온', pos: 1 })
        );
        assert_eq!(
            v("영", "〇", None),
            Err(EntryError::HanjaNotHan { ch: '〇', pos: 0 })
        );
        assert_eq!(
            v("일", "々", None),
            Err(EntryError::HanjaNotHan { ch: '々', pos: 0 })
        );
        // 부수 보충(U+2E80)·강희 부수(U+2F00)
        assert!(matches!(
            v("일", "\u{2E80}", None),
            Err(EntryError::HanjaNotHan { .. })
        ));
        assert!(matches!(
            v("일", "\u{2F00}", None),
            Err(EntryError::HanjaNotHan { .. })
        ));
    }

    #[test]
    fn validate_han_ranges_pass() {
        assert!(v("가", "\u{3400}", None).is_ok(), "확장 A");
        assert!(v("가", "\u{4E00}", None).is_ok(), "기본 시작");
        assert!(v("가", "\u{9FFF}", None).is_ok(), "기본 끝");
        assert!(v("불", "\u{F967}", None).is_ok(), "호환 한자 不");
        assert!(v("가", "\u{20000}", None).is_ok(), "확장 B");
        assert!(v("가", "\u{2A700}", None).is_ok(), "확장 C");
        assert!(v("가", "\u{2F800}", None).is_ok(), "호환 보충");
        assert!(v("가", "\u{30000}", None).is_ok(), "확장 G");
        // 경계 밖
        assert!(v("가", "\u{33FF}", None).is_err());
        assert!(v("가", "\u{4DC0}", None).is_err(), "주역 육십사괘");
        assert!(v("가", "\u{FB00}", None).is_err());
    }

    #[test]
    fn validate_no_normalization() {
        // 호환 한자 U+F967 은 NFC 로 바뀌지 않고 그대로 보존된다.
        let r = v("불", "\u{F967}", None).unwrap();
        assert_eq!(r.1, "\u{F967}");
    }

    #[test]
    fn validate_meaning() {
        assert!(v("서", "徐", Some(&"뜻".repeat(60))).is_ok());
        assert_eq!(
            v("서", "徐", Some(&"뜻".repeat(61))),
            Err(EntryError::MeaningTooLong { len: 61 })
        );
        assert_eq!(v("서", "徐", Some("a\nb")), Err(EntryError::MeaningControl));
        assert_eq!(v("서", "徐", Some("a\tb")), Err(EntryError::MeaningControl));
        assert_eq!(
            v("서", "徐", Some("a\u{2028}b")),
            Err(EntryError::MeaningControl)
        );
        // 일반 문장부호 허용
        assert!(v("서", "徐", Some("성씨: 서, (예) 'x'")).is_ok());
    }

    // ── 추가·찾기·수정·삭제 ──

    #[test]
    fn add_find_and_duplicate() {
        let mut d = HanjaUserDict::default();
        assert_eq!(d.add("서기현", "徐基鉉", Some("내 이름".into())), Ok(0));
        assert_eq!(
            d.add("서기현", "徐起鉉", None),
            Ok(1),
            "한 한글에 한자 여럿"
        );
        assert_eq!(
            d.add(" 서기현 ", " 徐基鉉 ", None),
            Err(EntryError::Duplicate)
        );
        assert_eq!(
            d.add("서기현", "徐基A", None),
            Err(EntryError::HanjaNotHan { ch: 'A', pos: 2 })
        );
        assert_eq!(d.entries.len(), 2);
        assert_eq!(d.find("서기현", "徐起鉉"), Some(1));
        assert_eq!(d.find("서기현", "없음"), None);
        assert!(d.entries[0].added_at > 0);
        assert_eq!(d.entries[0].meaning.as_deref(), Some("내 이름"));
    }

    #[test]
    fn update_at_keeps_position_and_added_at() {
        let mut d = HanjaUserDict::default();
        d.add("가", "加", None).unwrap();
        d.add("나", "那", None).unwrap();
        d.add("다", "多", None).unwrap();
        d.entries[1].added_at = 12345;

        d.update_at(1, "내", "內", Some("안".into())).unwrap();
        assert_eq!(d.entries.len(), 3);
        assert_eq!(d.entries[1].hangul, "내");
        assert_eq!(d.entries[1].hanja, "內");
        assert_eq!(d.entries[1].meaning.as_deref(), Some("안"));
        assert_eq!(d.entries[1].added_at, 12345, "added_at 유지");
        assert_eq!(d.entries[0].hangul, "가");
        assert_eq!(d.entries[2].hangul, "다");
    }

    #[test]
    fn update_at_duplicate_excludes_self() {
        let mut d = HanjaUserDict::default();
        d.add("가", "加", None).unwrap();
        d.add("나", "那", None).unwrap();
        // 자기 자신과 같은 값으로 고치기(뜻만 변경)는 허용
        assert!(d.update_at(1, "나", "那", Some("뜻".into())).is_ok());
        // 다른 항목과 같은 쌍은 거부
        assert_eq!(d.update_at(1, "가", "加", None), Err(EntryError::Duplicate));
        // 무효 값·범위 밖
        assert!(matches!(
            d.update_at(1, "나", "X", None),
            Err(EntryError::HanjaLenMismatch { .. } | EntryError::HanjaNotHan { .. })
        ));
        assert_eq!(d.update_at(9, "나", "那", None), Err(EntryError::NotFound));
        assert_eq!(
            d.entries[1].hanja, "那",
            "실패한 수정은 항목을 바꾸지 않는다"
        );
    }

    #[test]
    fn remove_pair_and_all() {
        let mut d = HanjaUserDict::default();
        d.add("서", "徐", None).unwrap();
        d.add("서", "書", None).unwrap();
        d.add("가", "加", None).unwrap();
        assert_eq!(d.remove("서", Some("徐")), 1);
        assert_eq!(d.remove("서", Some("徐")), 0);
        assert_eq!(d.entries.len(), 2);
        d.add("서", "西", None).unwrap();
        assert_eq!(d.remove("서", None), 2);
        assert_eq!(d.entries.len(), 1);
        assert_eq!(d.entries[0].hangul, "가");
    }

    #[test]
    fn remove_at_bounds() {
        let mut d = HanjaUserDict::default();
        d.add("가", "加", None).unwrap();
        assert!(d.remove_at(5).is_none());
        let e = d.remove_at(0).unwrap();
        assert_eq!(e.hanja, "加");
        assert!(d.entries.is_empty());
    }

    // ── 로드·저장 ──

    #[test]
    fn load_missing_is_empty_ok() {
        let p = unique_temp_path("missing");
        let d = HanjaUserDict::load(&p).unwrap();
        assert!(d.entries.is_empty());
        assert_eq!(d.version, 1);
    }

    #[test]
    fn load_empty_or_comment_only_file() {
        let p = unique_temp_path("empty");
        fs::write(&p, "  \n").unwrap();
        assert!(HanjaUserDict::load(&p).unwrap().entries.is_empty());
        let _ = fs::remove_file(&p);
    }

    #[test]
    fn load_corrupt_is_err_and_file_untouched() {
        let p = unique_temp_path("corrupt");
        let body = "version: 1\nentries:\n  - hangul: [서\n";
        fs::write(&p, body).unwrap();
        match HanjaUserDict::load(&p) {
            Err(LoadError::Parse { line, column, .. }) => {
                assert!(line.is_some());
                assert!(column.is_some());
            }
            other => panic!("Parse 오류여야 함: {other:?}"),
        }
        assert_eq!(
            fs::read_to_string(&p).unwrap(),
            body,
            "읽기가 파일을 건드리지 않는다"
        );
        let _ = fs::remove_file(&p);
    }

    #[test]
    fn load_wrong_type_is_parse_error() {
        let p = unique_temp_path("wrongtype");
        fs::write(&p, "entries: 3\n").unwrap();
        assert!(matches!(
            HanjaUserDict::load(&p),
            Err(LoadError::Parse { .. })
        ));
        let _ = fs::remove_file(&p);
    }

    #[test]
    fn load_unreadable_is_io_error() {
        // 디렉터리를 파일로 읽으려 하면 NotFound 가 아닌 I/O 오류
        let dir = std::env::temp_dir();
        assert!(matches!(HanjaUserDict::load(&dir), Err(LoadError::Io(_))));
    }

    #[test]
    fn newer_version_load_and_save_refused() {
        let p = unique_temp_path("newer");
        let body = "version: 2\nentries:\n  - hangul: 서\n    hanja: 徐\n    future_field: x\n";
        fs::write(&p, body).unwrap();
        assert!(matches!(
            HanjaUserDict::load(&p),
            Err(LoadError::NewerVersion(2))
        ));
        // 엔진용 최선 읽기는 가능
        let d = HanjaUserDict::load_lenient(&p).unwrap();
        assert_eq!(d.version, 2);
        assert_eq!(d.entries.len(), 1);
        // 저장은 거부, 파일 그대로
        assert!(d.save(&p).is_err());
        assert_eq!(fs::read_to_string(&p).unwrap(), body);
        let _ = fs::remove_file(&p);
    }

    #[test]
    fn save_round_trip() {
        let p = unique_temp_path("roundtrip");
        let mut d = HanjaUserDict::default();
        d.add("서기현", "徐基鉉", Some("내 이름: 서".into()))
            .unwrap();
        d.add("다온", "多溫", None).unwrap();
        d.save(&p).unwrap();
        let r = HanjaUserDict::load(&p).unwrap();
        assert_eq!(r.version, 1);
        assert_eq!(r.entries, d.entries);
        let text = fs::read_to_string(&p).unwrap();
        assert!(!text.contains("meaning: null"), "None 은 키 생략");
        let _ = fs::remove_file(&p);
    }

    #[test]
    fn invalid_entries_preserved_through_save() {
        let p = unique_temp_path("invalid");
        let body = "version: 1\nentries:\n  - hangul: 서기현\n    hanja: 徐基鉉\n    added_at: 7\n  - hangul: 다온\n    hanja: 多온\n    added_at: 8\n  - hangul: ㄱ\n    hanja: 徐\n";
        fs::write(&p, body).unwrap();
        let mut d = HanjaUserDict::load(&p).unwrap();
        assert_eq!(d.entries.len(), 3, "무효 항목도 로드된다");
        assert_eq!(d.entry_error(0), None);
        assert_eq!(
            d.entry_error(1),
            Some(EntryError::HanjaNotHan { ch: '온', pos: 1 })
        );
        assert_eq!(
            d.entry_error(2),
            Some(EntryError::HangulNotSyllable { ch: 'ㄱ', pos: 0 })
        );
        assert_eq!(d.entry_error(3), None, "범위 밖");

        d.add("가", "加", None).unwrap();
        d.save(&p).unwrap();
        let r = HanjaUserDict::load(&p).unwrap();
        assert_eq!(r.entries.len(), 4);
        assert_eq!(r.entries[1].hanja, "多온");
        assert_eq!(r.entries[0].added_at, 7);
        let _ = fs::remove_file(&p);
    }

    #[test]
    fn entry_error_flags_later_duplicate() {
        let mut d = HanjaUserDict::default();
        d.entries.push(HanjaUserEntry {
            hangul: "서".into(),
            hanja: "徐".into(),
            meaning: None,
            added_at: 0,
        });
        d.entries.push(HanjaUserEntry {
            hangul: "서".into(),
            hanja: "徐".into(),
            meaning: None,
            added_at: 0,
        });
        assert_eq!(d.entry_error(0), None);
        assert_eq!(d.entry_error(1), Some(EntryError::Duplicate));
    }

    #[test]
    fn save_creates_parent_dir() {
        let base = unique_temp_path("nested");
        let p = base
            .with_extension("d")
            .join("sub")
            .join("hanja-userdict.yaml");
        let d = HanjaUserDict::default();
        d.save(&p).unwrap();
        assert!(p.exists());
        let _ = fs::remove_dir_all(base.with_extension("d"));
    }

    #[test]
    fn quarantine_renames_without_data_loss() {
        let p = unique_temp_path("quar");
        let body = "version: 1\nentries: [깨짐\n";
        fs::write(&p, body).unwrap();
        let q1 = HanjaUserDict::quarantine(&p).unwrap();
        assert!(!p.exists());
        assert_eq!(fs::read_to_string(&q1).unwrap(), body);
        let name = q1.file_name().unwrap().to_string_lossy().into_owned();
        let pname = p.file_name().unwrap().to_string_lossy().into_owned();
        assert!(name.starts_with(&format!("{pname}.broken-")), "{name}");
        let stamp = &name[pname.len() + ".broken-".len()..];
        assert_eq!(stamp.len(), "YYYYMMDD-HHMMSS".len());
        assert_eq!(stamp.as_bytes()[8], b'-');

        // 같은 초에 한 번 더 — 덮어쓰지 않는다
        fs::write(&p, "두번째").unwrap();
        let q2 = HanjaUserDict::quarantine(&p).unwrap();
        assert_ne!(q1, q2);
        assert_eq!(fs::read_to_string(&q1).unwrap(), body);
        assert_eq!(fs::read_to_string(&q2).unwrap(), "두번째");

        // 없는 파일은 오류
        assert!(HanjaUserDict::quarantine(&p).is_err());
        let _ = fs::remove_file(&q1);
        let _ = fs::remove_file(&q2);
    }

    // ── 인덱스 ──

    #[test]
    fn index_from_dict_skips_invalid() {
        let mut d = HanjaUserDict::default();
        d.add("서기현", "徐基鉉", Some("내 이름".into())).unwrap();
        d.add("서기현", "徐起鉉", None).unwrap();
        d.entries.push(HanjaUserEntry {
            hangul: "다온".into(),
            hanja: "多온".into(),
            meaning: None,
            added_at: 0,
        });
        d.entries.push(HanjaUserEntry {
            hangul: "서기현".into(),
            hanja: "徐基鉉".into(),
            meaning: None,
            added_at: 0,
        });
        d.add("가", "加", None).unwrap();

        let idx = HanjaUserIndex::from_dict(&d);
        assert_eq!(idx.valid_count(), 3);
        assert_eq!(idx.invalid_count(), 2);
        assert!(idx.contains("서기현"));
        assert!(idx.contains("가"));
        assert!(!idx.contains("다온"), "무효 항목은 조회에서 빠진다");
        let g = idx.get("서기현");
        assert_eq!(g.len(), 2, "중복 항목 제외");
        assert_eq!(g[0].hanja, "徐基鉉");
        assert_eq!(g[0].meaning, "내 이름");
        assert_eq!(g[1].hanja, "徐起鉉");
        assert_eq!(g[1].meaning, "");
        assert!(idx.get("없음").is_empty());
        assert!(HanjaUserIndex::empty().get("가").is_empty());
    }

    // ── 병합 ──

    #[test]
    fn merge_order_dedupe_and_meaning() {
        let user = vec![he("서기현", "徐基鉉", ""), he("서기현", "新", "사용자 뜻")];
        let builtin = vec![
            he("서기현", "西紀玄", "내장1"),
            he("서기현", "徐基鉉", "내장 원본 뜻"),
            he("서기현", "新", "내장 뜻"),
            he("서기현", "書記賢", "내장3"),
        ];
        let m = merge_candidates(&user, builtin);
        let hanja: Vec<&str> = m.iter().map(|e| e.hanja.as_str()).collect();
        assert_eq!(hanja, ["徐基鉉", "新", "西紀玄", "書記賢"]);
        assert_eq!(m[0].meaning, "내장 원본 뜻", "사용자 뜻 없으면 내장 뜻");
        assert_eq!(m[1].meaning, "사용자 뜻", "사용자 뜻 우선");
    }

    #[test]
    fn merge_user_only_and_empty_user() {
        let user = vec![he("서", "徐", "")];
        let m = merge_candidates(&user, vec![]);
        assert_eq!(m.len(), 1);
        assert_eq!(m[0].meaning, "");
        let b = vec![he("서", "書", "글")];
        assert_eq!(merge_candidates(&[], b.clone()), b);
    }

    // ── 캐시 ──

    #[test]
    fn cache_reload_on_change_and_delete() {
        let p = unique_temp_path("cache");
        let mut c = UserLayerCache::new(Some(p.clone()));
        assert!(!c.refresh().contains("서"), "파일 없음 → 빈 층");

        let mut d = HanjaUserDict::default();
        d.add("서", "徐", None).unwrap();
        d.save(&p).unwrap();
        let i1 = c.refresh();
        assert!(i1.contains("서"));
        let i1b = c.refresh();
        assert!(Arc::ptr_eq(&i1, &i1b), "서명이 같으면 같은 Arc");

        // 내용이 바뀌면(길이가 달라진다) 다시 읽는다. mtime 해상도에 기대지 않는다.
        d.add("가", "加", None).unwrap();
        d.save(&p).unwrap();
        let i2 = c.refresh();
        assert!(i2.contains("가") && i2.contains("서"));
        assert!(!Arc::ptr_eq(&i1, &i2));

        // 삭제 → 빈 층
        fs::remove_file(&p).unwrap();
        assert!(!c.refresh().contains("서"));
    }

    #[test]
    fn cache_parse_failure_gives_empty_then_recovers() {
        let p = unique_temp_path("cache-bad");
        let mut c = UserLayerCache::new(Some(p.clone()));
        let mut d = HanjaUserDict::default();
        d.add("서", "徐", None).unwrap();
        d.save(&p).unwrap();
        assert!(c.refresh().contains("서"));

        fs::write(&p, "entries: [깨짐 깨짐 깨짐\n").unwrap();
        let bad = c.refresh();
        assert_eq!(bad.valid_count(), 0, "파싱 실패 → 빈 층");
        assert!(
            Arc::ptr_eq(&bad, &c.refresh()),
            "같은 서명이면 재파싱하지 않는다"
        );
        assert_eq!(
            fs::read_to_string(&p).unwrap(),
            "entries: [깨짐 깨짐 깨짐\n",
            "캐시는 파일을 절대 쓰지 않는다"
        );

        d.save(&p).unwrap();
        assert!(c.refresh().contains("서"), "고치면 복구");
        let _ = fs::remove_file(&p);
    }

    #[test]
    fn cache_newer_version_read_best_effort() {
        let p = unique_temp_path("cache-v2");
        fs::write(&p, "version: 2\nentries:\n  - hangul: 서\n    hanja: 徐\n").unwrap();
        let mut c = UserLayerCache::new(Some(p.clone()));
        assert!(c.refresh().contains("서"));
        let _ = fs::remove_file(&p);
    }

    #[test]
    fn cache_without_path_is_empty() {
        let mut c = UserLayerCache::new(None);
        assert_eq!(c.refresh().valid_count(), 0);
    }

    #[test]
    fn shared_index_default_is_empty_in_tests() {
        // cfg(test) 기본값은 실제 ~/.config 를 읽지 않는다.
        assert_eq!(shared_index().valid_count(), 0);
    }
}
