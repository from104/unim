//! 프로필 네임스페이스 레지스트리 — 내장 9종 + 사용자 디렉토리 통합.
//!
//! 스펙: `docs/plans/LAYOUT_PROFILE_V1.md` §4.3 해석 범위.
//!
//! - 같은 `name`이 겹치면 **사용자 디렉토리 우선**(override 가능).
//! - `inherits`는 같은 네임스페이스에서만 조회.
//! - 파일 변경은 mtime 기반 자동 재로드 (`typefix_blacklist.rs` 패턴 재사용).
//!
//! # Phase 3 범위
//! - 사용자 디렉토리 `~/.config/unim/layouts/*.json` 스캔.
//! - `find_raw(name)` — 사용자 우선, 내장 fallback.
//! - `reload_if_changed()` — 2초 throttling + 디렉토리 mtime 변화 감지.
//!
//! inherits 체인 해석은 `inherit::resolve`가 레지스트리를 인자로 받아 수행.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use super::builtin;
use super::loader::parse_profile_str;
use super::schema::LayoutProfile;

/// 레지스트리 갱신 주기 (초). `typefix_blacklist.rs`와 동일한 값.
const RELOAD_THROTTLE_SECS: u64 = 2;

/// 내장 + 사용자 프로필 네임스페이스.
///
/// `default()` 또는 `new()`는 시스템 기본 디렉토리(`~/.config/unim/layouts/`)를 사용한다.
/// 테스트에서는 `with_user_dir(path)`로 경로를 지정.
#[derive(Debug, Clone)]
pub struct ProfileRegistry {
    /// 사용자 프로필 루트 디렉토리. `None`이면 사용자 프로필 없음(내장만).
    user_dir: Option<PathBuf>,
    /// 파일명(확장자 제거) → 파싱된 프로필.
    ///
    /// **Key는 파일 stem이 아니라 프로필 `name` 필드**다. §4.3은 name 기반 조회를 요구.
    user_profiles: BTreeMap<String, LayoutProfile>,
    /// 디렉토리 전체 mtime. 파일 추가/삭제/수정 모두 감지.
    dir_mtime: Option<SystemTime>,
    /// 마지막 스캔 시각 (throttling용).
    last_checked: Option<SystemTime>,
}

impl Default for ProfileRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl ProfileRegistry {
    /// 기본 경로(`~/.config/unim/layouts/`) 기반 레지스트리. 초기 스캔 수행.
    pub fn new() -> Self {
        let mut reg = Self {
            user_dir: default_user_dir(),
            user_profiles: BTreeMap::new(),
            dir_mtime: None,
            last_checked: None,
        };
        let _ = reg.scan();
        reg
    }

    /// 사용자 디렉토리 없이 내장만 보는 레지스트리 — inherits·validate 단독 테스트용.
    pub fn builtin_only() -> Self {
        Self {
            user_dir: None,
            user_profiles: BTreeMap::new(),
            dir_mtime: None,
            last_checked: None,
        }
    }

    /// 지정 경로를 사용자 디렉토리로 사용. 초기 스캔 수행.
    pub fn with_user_dir(dir: PathBuf) -> Self {
        let mut reg = Self {
            user_dir: Some(dir),
            user_profiles: BTreeMap::new(),
            dir_mtime: None,
            last_checked: None,
        };
        let _ = reg.scan();
        reg
    }

    /// 현재 사용 중인 사용자 디렉토리 경로.
    pub fn user_dir(&self) -> Option<&Path> {
        self.user_dir.as_deref()
    }

    /// 사용자 디렉토리를 스캔해 `user_profiles`를 채운다.
    ///
    /// 개별 파일 파싱 실패는 경고 로그 + skip. 전체 함수는 **에러를 반환하지 않는다**
    /// (디렉토리 부재는 정상 상태).
    pub fn scan(&mut self) -> ScanReport {
        let mut report = ScanReport::default();
        self.user_profiles.clear();
        self.last_checked = Some(SystemTime::now());

        let Some(dir) = self.user_dir.clone() else {
            self.dir_mtime = None;
            return report;
        };
        self.dir_mtime = dir_mtime(&dir);

        let entries = match fs::read_dir(&dir) {
            Ok(e) => e,
            Err(_) => return report,
        };

        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) != Some("json") {
                continue;
            }
            let content = match fs::read_to_string(&path) {
                Ok(s) => s,
                Err(e) => {
                    eprintln!("[UNIM] layout profile 읽기 실패: {} — {e}", path.display());
                    report.errors.push((path, e.to_string()));
                    continue;
                }
            };
            match parse_profile_str(&content) {
                Ok(profile) => {
                    let name = profile.name.clone();
                    if let Some(prev) = self.user_profiles.insert(name.clone(), profile) {
                        eprintln!(
                            "[UNIM] 중복된 프로필 이름 '{name}' — 늦게 읽힌 파일 {} 로 덮음 (이전: {})",
                            path.display(),
                            prev.name
                        );
                        report.duplicates.push(name);
                    } else {
                        report.loaded.push(path);
                    }
                }
                Err(e) => {
                    eprintln!("[UNIM] layout profile 파싱 실패: {} — {e}", path.display());
                    report.errors.push((path, e.to_string()));
                }
            }
        }
        report
    }

    /// 디렉토리 mtime이 변경됐으면 재스캔. 2초 throttling.
    ///
    /// Returns `true` if a rescan was performed.
    pub fn reload_if_changed(&mut self) -> bool {
        let now = SystemTime::now();
        if let Some(last) = self.last_checked {
            if let Ok(dur) = now.duration_since(last) {
                if dur.as_secs() < RELOAD_THROTTLE_SECS {
                    return false;
                }
            }
        }
        self.last_checked = Some(now);

        let Some(dir) = self.user_dir.as_deref() else {
            return false;
        };
        let current = dir_mtime(dir);
        let should_reload = match (self.dir_mtime, current) {
            (None, None) => false,
            (Some(prev), Some(cur)) => cur > prev,
            _ => true,
        };
        if !should_reload {
            return false;
        }
        self.scan();
        true
    }

    /// 네임스페이스에서 raw 프로필을 찾는다 (사용자 우선, 내장 fallback).
    ///
    /// "raw"는 **inherits 미해석** 상태. 체인 해석은 `inherit::resolve` 몫.
    pub fn find_raw(&self, name: &str) -> Option<LayoutProfile> {
        if let Some(p) = self.user_profiles.get(name) {
            return Some(p.clone());
        }
        let json = builtin::get_builtin_json(name)?;
        parse_profile_str(json).ok()
    }

    /// 네임스페이스 전체 이름 목록 (정렬, 중복 제거 — 사용자 우선).
    pub fn list_names(&self) -> Vec<String> {
        let mut set: BTreeMap<String, ()> = BTreeMap::new();
        for n in self.user_profiles.keys() {
            set.insert(n.clone(), ());
        }
        for n in builtin::BUILTIN_NAMES {
            set.entry(n.to_string()).or_insert(());
        }
        set.into_keys().collect()
    }

    /// 사용자 디렉토리에만 존재하는 프로필 이름(내장과의 충돌 여부 무관).
    pub fn user_names(&self) -> Vec<String> {
        self.user_profiles.keys().cloned().collect()
    }

    /// `name`이 사용자 디렉토리에서 온 프로필인지 — override 여부 확인용.
    pub fn is_user_override(&self, name: &str) -> bool {
        self.user_profiles.contains_key(name)
    }
}

/// 레지스트리 스캔 결과 리포트 (진단·테스트용).
#[derive(Debug, Default, Clone)]
pub struct ScanReport {
    /// 성공적으로 로드된 파일들.
    pub loaded: Vec<PathBuf>,
    /// 같은 `name`을 가진 프로필이 두 개 이상 있어 덮어써진 경우.
    pub duplicates: Vec<String>,
    /// 읽기 또는 파싱 실패 — (path, 에러 메시지).
    pub errors: Vec<(PathBuf, String)>,
}

fn default_user_dir() -> Option<PathBuf> {
    crate::paths::config_dir().map(|p| p.join("unim").join("layouts"))
}

fn dir_mtime(dir: &Path) -> Option<SystemTime> {
    fs::metadata(dir).and_then(|m| m.modified()).ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn temp_subdir(name: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        let dir = std::env::temp_dir().join(format!(
            "unim-profile-registry-{}-{}-{}",
            std::process::id(),
            nanos,
            name
        ));
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn write_profile(dir: &Path, filename: &str, json: &str) {
        let mut f = fs::File::create(dir.join(filename)).unwrap();
        f.write_all(json.as_bytes()).unwrap();
    }

    fn minimal_v1_profile_json(name: &str) -> String {
        format!(
            r#"{{
                "schema_version": 1,
                "language": "korean",
                "name": "{name}",
                "type": "3bul",
                "layout": {{
                    "upper": {{"1st":[],"2nd":[],"3rd":[],"4th":[]}},
                    "lower": {{"1st":[],"2nd":[],"3rd":[],"4th":[]}}
                }}
            }}"#
        )
    }

    #[test]
    fn builtin_only_registry_finds_all_builtins() {
        let reg = ProfileRegistry::builtin_only();
        for name in builtin::BUILTIN_NAMES {
            let p = reg.find_raw(name).unwrap_or_else(|| panic!("{name}"));
            assert!(!p.name.is_empty());
        }
        assert_eq!(reg.list_names().len(), builtin::BUILTIN_NAMES.len());
        assert!(reg.user_names().is_empty());
    }

    #[test]
    fn user_profile_overrides_builtin_by_name() {
        let dir = temp_subdir("override");
        // 내장 name "ko_3bul390"을 사용자가 덮어씀 (type을 2bul로 바꿔 차이 명확화)
        let json = r#"{
            "schema_version": 1,
            "language": "korean",
            "name": "ko_3bul390",
            "type": "2bul",
            "metadata": {"author": "user-override"},
            "layout": {
                "upper": {"1st":[],"2nd":[],"3rd":[],"4th":[]},
                "lower": {"1st":[],"2nd":[],"3rd":[],"4th":[]}
            }
        }"#;
        write_profile(&dir, "my_3bul390.json", json);

        let reg = ProfileRegistry::with_user_dir(dir.clone());
        let p = reg.find_raw("ko_3bul390").unwrap();
        assert_eq!(p.layout_type, "2bul", "사용자 파일이 내장을 덮어씀");
        assert_eq!(p.metadata.author.as_deref(), Some("user-override"));
        assert!(reg.is_user_override("ko_3bul390"));

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn name_field_is_key_not_file_stem() {
        // 파일명(`custom.json`)이 아니라 JSON 내부 name("my_layout")로 조회돼야 함.
        let dir = temp_subdir("name-key");
        write_profile(&dir, "custom.json", &minimal_v1_profile_json("my_layout"));

        let reg = ProfileRegistry::with_user_dir(dir.clone());
        assert!(reg.find_raw("my_layout").is_some(), "JSON name 필드로 조회");
        assert!(
            reg.find_raw("custom").is_none(),
            "파일 stem으로 조회되면 안 됨"
        );
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn malformed_json_logged_and_skipped() {
        let dir = temp_subdir("malformed");
        write_profile(&dir, "broken.json", "{ not valid json");
        write_profile(&dir, "ok.json", &minimal_v1_profile_json("good_one"));

        let reg = ProfileRegistry::with_user_dir(dir.clone());
        assert!(reg.find_raw("good_one").is_some());
        assert_eq!(reg.user_names().len(), 1);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn non_json_files_are_ignored() {
        let dir = temp_subdir("non-json");
        write_profile(&dir, "readme.md", "hello");
        write_profile(&dir, "config.yaml", "foo: bar");
        write_profile(&dir, "real.json", &minimal_v1_profile_json("real"));

        let reg = ProfileRegistry::with_user_dir(dir.clone());
        assert_eq!(reg.user_names(), vec!["real".to_string()]);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn missing_user_dir_is_silent_and_empty() {
        let dir = temp_subdir("missing");
        let nonexistent = dir.join("does-not-exist");
        let reg = ProfileRegistry::with_user_dir(nonexistent);
        assert!(reg.user_names().is_empty());
        // 내장은 여전히 조회 가능
        assert!(reg.find_raw("ko_3bul390").is_some());
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn list_names_deduplicates_user_vs_builtin() {
        let dir = temp_subdir("dedup");
        // 내장과 겹치는 이름 하나 + 완전 신규 하나
        write_profile(
            &dir,
            "override.json",
            &minimal_v1_profile_json("ko_3bul390"),
        );
        write_profile(&dir, "new.json", &minimal_v1_profile_json("my_custom"));

        let reg = ProfileRegistry::with_user_dir(dir.clone());
        let names = reg.list_names();
        assert!(names.contains(&"ko_3bul390".to_string()));
        assert!(names.contains(&"my_custom".to_string()));
        // 중복 없음 — 내장 9 + 사용자 고유 1 = 10
        assert_eq!(names.len(), builtin::BUILTIN_NAMES.len() + 1);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn reload_throttled_within_2_seconds() {
        let dir = temp_subdir("throttle");
        write_profile(&dir, "a.json", &minimal_v1_profile_json("a"));
        let mut reg = ProfileRegistry::with_user_dir(dir.clone());
        assert_eq!(reg.user_names(), vec!["a".to_string()]);

        // 곧바로 다시 호출 → throttle로 false
        write_profile(&dir, "b.json", &minimal_v1_profile_json("b"));
        let did = reg.reload_if_changed();
        assert!(!did, "2초 내 재호출은 throttle됨");
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn reload_picks_up_new_file_after_throttle() {
        let dir = temp_subdir("reload");
        write_profile(&dir, "a.json", &minimal_v1_profile_json("a"));
        let mut reg = ProfileRegistry::with_user_dir(dir.clone());
        // 강제로 last_checked를 과거로 밀어 throttle 우회
        reg.last_checked = Some(SystemTime::UNIX_EPOCH);

        // mtime이 바뀌도록 파일 추가 + 디렉토리 mtime 변경 유도 (touch)
        write_profile(&dir, "b.json", &minimal_v1_profile_json("b"));
        // ensure mtime advances — dir_mtime 가 이전 scan 시점이므로 자동 >.
        // dir_mtime을 인위적으로 과거로 밀어 확실히 재로드 트리거.
        reg.dir_mtime = Some(SystemTime::UNIX_EPOCH);

        let did = reg.reload_if_changed();
        assert!(did, "디렉토리 mtime 변경 감지 후 rescan");
        let names = reg.user_names();
        assert!(names.contains(&"a".to_string()));
        assert!(names.contains(&"b".to_string()));
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn scan_report_records_load_success() {
        let dir = temp_subdir("report");
        write_profile(&dir, "a.json", &minimal_v1_profile_json("alpha"));
        write_profile(&dir, "bad.json", "{ broken");
        let mut reg = ProfileRegistry::with_user_dir(dir.clone());
        let report = reg.scan();
        assert_eq!(report.loaded.len(), 1);
        assert_eq!(report.errors.len(), 1);
        let _ = fs::remove_dir_all(&dir);
    }
}
