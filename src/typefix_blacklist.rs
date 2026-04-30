//! AutoTypeFix 학습형 오탐 억제 (Blacklist)
//!
//! 사용자가 자동교정 결과를 **백스페이스로 corrected 전체를 지우고** 바로
//! **언어 모드를 전환**하는 자연스러운 롤백 패턴을 관찰해, 해당 ASCII 시퀀스를
//! 임시 억제 단어로 `~/.config/unim/typefix-blacklist.yaml`에 기록한다.
//!
//! 엔트리 상태:
//! - `Tentative`: 자동 감지된 의심 단어. 억제 효과 있음. 기본 4시간 내 수동 확정 안 되면 Inactive로 전환.
//! - `Confirmed`: GUI에서 사용자가 확정. 영구 억제.
//! - `Inactive`: 만료됨. 기록은 유지되나 억제 효과 없음.
//!
//! 매칭 키: `(ascii, direction, korean_layout, english_layout)`. 같은 물리 키
//! 시퀀스도 레이아웃에 따라 다른 단어로 해석되므로 레이아웃별로 분리 저장.

use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use crate::config::{
    normalize_english_layout_name, normalize_korean_layout_name, EnglishLayout, KoreanLayout,
};

/// 레거시 enum 변종(`Dubeolsik` 등)·별칭(`2bul` 등)을 정식 이름으로 승격해 저장한다.
fn deserialize_korean_layout<'de, D>(deserializer: D) -> Result<KoreanLayout, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let s = String::deserialize(deserializer)?;
    Ok(normalize_korean_layout_name(&s))
}

/// 레거시 enum 변종(`Qwerty` 등)을 소문자 내장 이름으로 승격해 저장한다.
fn deserialize_english_layout<'de, D>(deserializer: D) -> Result<EnglishLayout, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let s = String::deserialize(deserializer)?;
    Ok(normalize_english_layout_name(&s))
}

/// 교정 방향.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Direction {
    /// 영어 모드 → 한글 교정
    Forward,
    /// 한글 모드 → 영어 교정
    Reverse,
}

/// AutoTypeFix 감지 경로에서 blacklist와 소통하기 위한 추상 게이트.
///
/// `auto_typefix.rs`의 `check_forward` / `check_reverse`는 이 trait를 통해서만
/// blacklist를 조회한다. 구현체는 [`Blacklist`]를 비롯해 테스트용 모의 객체
/// (예: `Blacklist::default()`) 무엇이든 가능하다.
///
/// 이 추상은 두 가지 목적을 만족한다:
/// 1. `auto_typefix.rs → typefix_blacklist.rs` 의존성을 **명시적인 trait import**로
///    승격하여 정적 분석 도구가 관계를 추적할 수 있게 한다.
/// 2. 감지 경로의 테스트에서 blacklist를 쉽게 스텁(stub)할 수 있게 한다.
pub trait BlacklistGate {
    /// 주어진 (ascii, direction, layouts) 조합이 **활성 억제 대상**인지 여부.
    fn is_suppressed(
        &self,
        ascii: &str,
        direction: Direction,
        korean_layout: &str,
        english_layout: &str,
    ) -> bool;

    /// 임시(tentative) 엔트리를 등록하거나 기존 엔트리의 hit을 누적한다.
    fn add_or_hit_tentative(
        &mut self,
        ascii: &str,
        direction: Direction,
        korean_layout: &str,
        english_layout: &str,
    );
}

impl BlacklistGate for Blacklist {
    fn is_suppressed(
        &self,
        ascii: &str,
        direction: Direction,
        korean_layout: &str,
        english_layout: &str,
    ) -> bool {
        Blacklist::is_suppressed(self, ascii, direction, korean_layout, english_layout)
    }

    fn add_or_hit_tentative(
        &mut self,
        ascii: &str,
        direction: Direction,
        korean_layout: &str,
        english_layout: &str,
    ) {
        Blacklist::add_or_hit_tentative(self, ascii, direction, korean_layout, english_layout)
    }
}

/// 엔트리 상태.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EntryStatus {
    /// 자동 감지됨 (억제 효과 있음, 만료 대상)
    Tentative,
    /// 사용자 확정 (억제 효과 있음, 영구)
    Confirmed,
    /// 만료/비활성화됨 (기록만 유지)
    Inactive,
}

impl EntryStatus {
    pub fn is_active(self) -> bool {
        matches!(self, EntryStatus::Tentative | EntryStatus::Confirmed)
    }
}

/// 억제 단어 엔트리.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BlacklistEntry {
    pub ascii: String,
    pub direction: Direction,
    /// 자판 프로필 이름. 역직렬화 시 레거시 enum 이름·별칭을 정식 이름으로 정규화.
    #[serde(deserialize_with = "deserialize_korean_layout")]
    pub korean_layout: KoreanLayout,
    /// 영문 자판 프로필 이름. 역직렬화 시 레거시 enum 이름을 소문자로 승격.
    #[serde(deserialize_with = "deserialize_english_layout")]
    pub english_layout: EnglishLayout,
    pub status: EntryStatus,
    /// 최초 관찰 시각 (Unix seconds)
    pub observed_at: u64,
    /// 최근 감지 시각 (Unix seconds)
    pub last_seen_at: u64,
    /// 누적 감지 횟수
    pub hit_count: u32,
}

fn default_version() -> u8 {
    1
}

/// 억제 단어 컨테이너. 파일 R/W + 매칭 + 만료 처리를 담당한다.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Blacklist {
    #[serde(default = "default_version")]
    pub version: u8,
    #[serde(default)]
    pub entries: Vec<BlacklistEntry>,

    /// 런타임 상태 — 파일로 직렬화하지 않음.
    #[serde(skip)]
    last_modified: Option<SystemTime>,
    #[serde(skip)]
    last_checked: Option<SystemTime>,
}

impl Default for Blacklist {
    fn default() -> Self {
        Self {
            version: 1,
            entries: Vec::new(),
            last_modified: None,
            last_checked: None,
        }
    }
}

fn now_unix() -> u64 {
    SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

fn get_mtime(path: &Path) -> Option<SystemTime> {
    fs::metadata(path).and_then(|m| m.modified()).ok()
}

impl Blacklist {
    /// 기본 경로: `~/.config/unim/typefix-blacklist.yaml`
    pub fn default_path() -> Option<PathBuf> {
        dirs::config_dir().map(|p| p.join("unim").join("typefix-blacklist.yaml"))
    }

    /// 지정된 경로에서 로드. 파일이 없거나 파싱 실패 시 빈 Blacklist.
    pub fn load_from_path(path: &Path) -> Self {
        match fs::read_to_string(path) {
            Ok(content) => match serde_yaml::from_str::<Blacklist>(&content) {
                Ok(mut bl) => {
                    bl.last_modified = get_mtime(path);
                    bl.last_checked = Some(SystemTime::now());
                    bl
                }
                Err(e) => {
                    eprintln!("[UNIM] typefix-blacklist 파싱 실패: {}. 빈 목록 사용.", e);
                    Self::default()
                }
            },
            Err(_) => Self::default(),
        }
    }

    /// 기본 경로에서 로드.
    pub fn load_from_default_path() -> Self {
        match Self::default_path() {
            Some(path) => Self::load_from_path(&path),
            None => Self::default(),
        }
    }

    /// 지정된 경로에 원자적으로 저장 (tmp + rename).
    pub fn save_to_path(&self, path: &Path) -> std::io::Result<()> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let content = serde_yaml::to_string(self)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;

        let tmp_path = path.with_extension("yaml.tmp");
        fs::write(&tmp_path, content)?;
        fs::rename(&tmp_path, path)?;
        Ok(())
    }

    /// 기본 경로에 저장.
    pub fn save_to_default_path(&mut self) -> std::io::Result<()> {
        let Some(path) = Self::default_path() else {
            return Err(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                "typefix-blacklist 경로를 찾을 수 없음",
            ));
        };
        self.save_to_path(&path)?;
        self.last_modified = get_mtime(&path);
        Ok(())
    }

    /// 파일 변경 시 재로드 (2초 throttling).
    pub fn reload_if_changed(&mut self) -> bool {
        let now = SystemTime::now();
        if let Some(last) = self.last_checked {
            if let Ok(dur) = now.duration_since(last) {
                if dur.as_secs() < 2 {
                    return false;
                }
            }
        }
        self.last_checked = Some(now);

        let Some(path) = Self::default_path() else {
            return false;
        };
        let Some(current) = get_mtime(&path) else {
            return false;
        };
        let should_reload = match self.last_modified {
            Some(saved) => current > saved,
            None => true,
        };
        if !should_reload {
            return false;
        }

        let reloaded = Self::load_from_path(&path);
        let last_checked = self.last_checked;
        *self = reloaded;
        self.last_checked = last_checked;
        true
    }

    /// 해당 입력이 **활성** 억제 대상인지 확인.
    pub fn is_suppressed(&self, ascii: &str, direction: Direction, kl: &str, el: &str) -> bool {
        self.entries.iter().any(|e| {
            e.status.is_active()
                && e.direction == direction
                && e.korean_layout == kl
                && e.english_layout == el
                && e.ascii == ascii
        })
    }

    fn find_idx(&self, ascii: &str, direction: Direction, kl: &str, el: &str) -> Option<usize> {
        self.entries.iter().position(|e| {
            e.direction == direction
                && e.korean_layout == kl
                && e.english_layout == el
                && e.ascii == ascii
        })
    }

    /// tentative 엔트리 추가. 이미 있으면 hit_count 증가 + last_seen_at 갱신.
    /// 기존 엔트리가 Inactive이면 Tentative로 되살린다 (observed_at은 현재 시각으로 리셋).
    pub fn add_or_hit_tentative(&mut self, ascii: &str, direction: Direction, kl: &str, el: &str) {
        let now = now_unix();
        match self.find_idx(ascii, direction, kl, el) {
            Some(idx) => {
                let e = &mut self.entries[idx];
                e.hit_count = e.hit_count.saturating_add(1);
                e.last_seen_at = now;
                if e.status == EntryStatus::Inactive {
                    e.status = EntryStatus::Tentative;
                    e.observed_at = now;
                }
            }
            None => self.entries.push(BlacklistEntry {
                ascii: ascii.to_string(),
                direction,
                korean_layout: kl.to_string(),
                english_layout: el.to_string(),
                status: EntryStatus::Tentative,
                observed_at: now,
                last_seen_at: now,
                hit_count: 1,
            }),
        }
    }

    /// 만료 기간이 지난 Tentative 엔트리를 Inactive로 전환.
    /// Confirmed는 건드리지 않는다.
    pub fn expire_tentatives(&mut self, expiry_hours: u16) {
        let expiry_secs = (expiry_hours as u64) * 60 * 60;
        let now = now_unix();
        for e in self.entries.iter_mut() {
            if e.status == EntryStatus::Tentative && now.saturating_sub(e.observed_at) > expiry_secs
            {
                e.status = EntryStatus::Inactive;
            }
        }
    }

    pub fn promote_to_confirmed(&mut self, idx: usize) {
        if let Some(e) = self.entries.get_mut(idx) {
            e.status = EntryStatus::Confirmed;
        }
    }

    pub fn deactivate(&mut self, idx: usize) {
        if let Some(e) = self.entries.get_mut(idx) {
            e.status = EntryStatus::Inactive;
        }
    }

    pub fn reactivate_as_confirmed(&mut self, idx: usize) {
        if let Some(e) = self.entries.get_mut(idx) {
            e.status = EntryStatus::Confirmed;
            e.last_seen_at = now_unix();
        }
    }

    pub fn remove(&mut self, idx: usize) {
        if idx < self.entries.len() {
            self.entries.remove(idx);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{Duration, SystemTime};

    /// 테스트 전용 고유 임시 경로. tempfile 의존성 없이 `std::env::temp_dir()` + PID + nano 조합으로 충돌 회피.
    fn unique_temp_path(name: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        std::env::temp_dir().join(format!(
            "unim-test-{}-{}-{}.yaml",
            std::process::id(),
            nanos,
            name
        ))
    }

    fn sample_entry(ascii: &str, status: EntryStatus, observed_at: u64) -> BlacklistEntry {
        BlacklistEntry {
            ascii: ascii.to_string(),
            direction: Direction::Forward,
            korean_layout: "ko_2bulstd".to_string(),
            english_layout: "qwerty".to_string(),
            status,
            observed_at,
            last_seen_at: observed_at,
            hit_count: 1,
        }
    }

    #[test]
    fn is_suppressed_active_states() {
        let mut bl = Blacklist::default();
        bl.entries
            .push(sample_entry("gksrmf", EntryStatus::Tentative, 0));
        bl.entries
            .push(sample_entry("world", EntryStatus::Confirmed, 0));
        bl.entries
            .push(sample_entry("inactive", EntryStatus::Inactive, 0));

        assert!(bl.is_suppressed("gksrmf", Direction::Forward, "ko_2bulstd", "qwerty"));
        assert!(bl.is_suppressed("world", Direction::Forward, "ko_2bulstd", "qwerty"));
        assert!(!bl.is_suppressed("inactive", Direction::Forward, "ko_2bulstd", "qwerty"));
        // 다른 방향/레이아웃은 매칭 안 됨
        assert!(!bl.is_suppressed("gksrmf", Direction::Reverse, "ko_2bulstd", "qwerty"));
        assert!(!bl.is_suppressed("gksrmf", Direction::Forward, "ko_3bul390", "qwerty"));
        assert!(!bl.is_suppressed("gksrmf", Direction::Forward, "ko_2bulstd", "dvorak"));
    }

    #[test]
    fn add_or_hit_tentative_creates_and_hits() {
        let mut bl = Blacklist::default();
        bl.add_or_hit_tentative("gksrmf", Direction::Forward, "ko_2bulstd", "qwerty");
        assert_eq!(bl.entries.len(), 1);
        assert_eq!(bl.entries[0].hit_count, 1);
        assert_eq!(bl.entries[0].status, EntryStatus::Tentative);

        bl.add_or_hit_tentative("gksrmf", Direction::Forward, "ko_2bulstd", "qwerty");
        assert_eq!(bl.entries.len(), 1, "중복 생성 금지");
        assert_eq!(bl.entries[0].hit_count, 2);

        // 다른 방향은 별도 엔트리
        bl.add_or_hit_tentative("gksrmf", Direction::Reverse, "ko_2bulstd", "qwerty");
        assert_eq!(bl.entries.len(), 2);
    }

    #[test]
    fn add_or_hit_revives_inactive() {
        let mut bl = Blacklist::default();
        bl.entries
            .push(sample_entry("gksrmf", EntryStatus::Inactive, 0));
        bl.add_or_hit_tentative("gksrmf", Direction::Forward, "ko_2bulstd", "qwerty");
        assert_eq!(bl.entries.len(), 1);
        assert_eq!(bl.entries[0].status, EntryStatus::Tentative);
    }

    #[test]
    fn expire_tentatives_transitions_only_old() {
        let now = now_unix();
        let hour_secs = 60 * 60u64;
        let mut bl = Blacklist::default();
        // 10시간 전 tentative → 만료 대상 (4시간 임계)
        bl.entries.push(sample_entry(
            "old",
            EntryStatus::Tentative,
            now.saturating_sub(10 * hour_secs),
        ));
        // 2시간 전 tentative → 유지
        bl.entries.push(sample_entry(
            "fresh",
            EntryStatus::Tentative,
            now.saturating_sub(2 * hour_secs),
        ));
        // 오래된 confirmed → 유지
        bl.entries.push(sample_entry(
            "confirmed",
            EntryStatus::Confirmed,
            now.saturating_sub(10 * hour_secs),
        ));

        bl.expire_tentatives(4);

        assert_eq!(bl.entries[0].status, EntryStatus::Inactive);
        assert_eq!(bl.entries[1].status, EntryStatus::Tentative);
        assert_eq!(bl.entries[2].status, EntryStatus::Confirmed);
    }

    #[test]
    fn promote_deactivate_reactivate_remove() {
        let mut bl = Blacklist::default();
        bl.entries
            .push(sample_entry("a", EntryStatus::Tentative, 0));
        bl.promote_to_confirmed(0);
        assert_eq!(bl.entries[0].status, EntryStatus::Confirmed);
        bl.deactivate(0);
        assert_eq!(bl.entries[0].status, EntryStatus::Inactive);
        bl.reactivate_as_confirmed(0);
        assert_eq!(bl.entries[0].status, EntryStatus::Confirmed);
        bl.remove(0);
        assert!(bl.entries.is_empty());
        // 범위 밖 index는 no-op
        bl.promote_to_confirmed(0);
        bl.remove(0);
    }

    #[test]
    fn yaml_round_trip() {
        let path = unique_temp_path("round-trip");

        let mut bl = Blacklist::default();
        bl.entries
            .push(sample_entry("gksrmf", EntryStatus::Tentative, 1713258000));
        bl.entries
            .push(sample_entry("world", EntryStatus::Confirmed, 1713258000));

        bl.save_to_path(&path).unwrap();
        let reloaded = Blacklist::load_from_path(&path);

        assert_eq!(reloaded.entries.len(), 2);
        assert_eq!(reloaded.entries[0].ascii, "gksrmf");
        assert_eq!(reloaded.entries[0].status, EntryStatus::Tentative);
        assert_eq!(reloaded.entries[1].status, EntryStatus::Confirmed);
        assert_eq!(reloaded.version, 1);

        let _ = fs::remove_file(&path);
    }

    #[test]
    fn load_missing_file_returns_default() {
        let path = unique_temp_path("missing");
        let bl = Blacklist::load_from_path(&path);
        assert!(bl.entries.is_empty());
        assert_eq!(bl.version, 1);
    }

    // SystemTime 관련 sanity — 컴파일러가 의존성 제대로 잡는지
    #[test]
    fn now_unix_is_positive() {
        assert!(now_unix() > 0);
        let _ = SystemTime::now() + Duration::from_secs(1);
    }
}
