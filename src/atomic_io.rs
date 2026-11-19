//! 원자적 파일 저장 헬퍼 (config.yaml, typefix-userdict.yaml, typefix-blacklist.yaml,
//! 사용자 키맵 JSON 등 여러 저장 경로가 공유).
//!
//! 같은 디렉터리에 프로세스·시각 고유의 tmp 파일을 쓴 뒤 `rename`으로 교체하는
//! 방식으로 두 가지 문제를 막는다:
//!
//! 1. **저장 도중 전원 단절/OOM-kill** — 대상 경로에 직접 `fs::write`하면
//!    O_TRUNC로 먼저 파일을 비우므로, 쓰기 도중 중단되면 빈 파일/잘린 내용이
//!    그대로 남아 다음 로드 시 ParseError → 기본값 초기화로 이어진다.
//! 2. **동시 저장 프로세스 간 경합** — tmp 파일명이 고정(`foo.yaml.tmp`)이면,
//!    두 프로세스(예: 데몬 + 설정 GUI)가 겹쳐 쓸 때 한쪽의 `fs::write`가 진행
//!    중인 tmp를 다른 쪽이 `rename`해버리거나, 두 rename이 경합해 한쪽이
//!    ENOENT로 실패할 수 있다(GAP-config-durability-and-write-races-01).
//!    tmp 파일명에 PID+나노초를 넣어 프로세스별로 고유하게 만들면 이 경합이
//!    사라진다.
//!
//! 대상이 심볼릭 링크(dotfile 매니저 등으로 관리)인 경우, 링크 자체를
//! `rename`으로 교체하면 관리 체계가 끊기므로 링크를 따라가 실제 파일이
//! 있는 자리에 원자적으로 쓴다.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

/// 심볼릭 링크를 따라가 실제 쓰기 대상 경로를 반환한다.
///
/// 링크가 아니면 원본 경로 그대로. 링크가 깨져 있어 대상이 없으면 원본 경로로
/// 폴백한다(rename이 링크를 일반 파일로 대체하게 된다 — 깨진 링크이므로
/// 어차피 아무 데도 안 가리키고 있었던 상태와 크게 다르지 않다).
fn resolve_write_target(path: &Path) -> PathBuf {
    match fs::symlink_metadata(path) {
        Ok(meta) if meta.file_type().is_symlink() => {
            fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf())
        }
        _ => path.to_path_buf(),
    }
}

/// `{파일명}.tmp.{pid}.{nanos}` 형태의 프로세스·시각 고유 tmp 경로 생성.
fn unique_tmp_path(target: &Path) -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let pid = std::process::id();
    let mut name = target.file_name().map(|n| n.to_os_string()).unwrap_or_default();
    name.push(format!(".tmp.{pid}.{nanos}"));
    target.with_file_name(name)
}

/// `content`를 `path`에 원자적으로 저장한다.
///
/// - tmp 파일은 대상과 같은 디렉터리에 프로세스 고유 이름으로 생성한다 —
///   다른 파일시스템(`/tmp` 등)에 만들면 `rename`이 EXDEV로 실패하고, 이름이
///   고정이면 동시 저장 프로세스끼리 충돌한다.
/// - 대상이 심볼릭 링크면 링크를 따라가 실제 파일 위치에 쓴다.
/// - 기존 파일이 있으면 그 퍼미션을 tmp에 복사한다(그대로 두면 `rename`이
///   tmp의 모드를 가져와 umask에 따라 퍼미션이 바뀔 수 있다).
/// - 쓰기 또는 rename이 실패하면 tmp 파일을 정리한다.
pub fn atomic_write(path: &Path, content: impl AsRef<[u8]>) -> io::Result<()> {
    let target = resolve_write_target(path);
    if let Some(parent) = target.parent() {
        fs::create_dir_all(parent)?;
    }

    let tmp_path = unique_tmp_path(&target);
    if let Err(e) = fs::write(&tmp_path, content.as_ref()) {
        let _ = fs::remove_file(&tmp_path);
        return Err(e);
    }

    if let Ok(metadata) = fs::metadata(&target) {
        let _ = fs::set_permissions(&tmp_path, metadata.permissions());
    }

    if let Err(e) = fs::rename(&tmp_path, &target) {
        let _ = fs::remove_file(&tmp_path);
        return Err(e);
    }

    Ok(())
}

/// 읽을 수 없는 사용자 파일(파싱 실패·지원하지 않는 버전·읽기 오류)에 대한 저장 거부 오류.
///
/// `io::Error` 안에 실려 나가므로 호출자는 [`is_unreadable_err`] 로 구분해 사용자에게
/// "파일이 깨졌다" 를 안내할 수 있다.
#[derive(Debug)]
pub struct UnreadableFile(pub PathBuf);

impl std::fmt::Display for UnreadableFile {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "읽을 수 없는 파일이라 저장을 거부함 (덮어쓰면 단어가 사라짐): {}",
            self.0.display()
        )
    }
}

impl std::error::Error for UnreadableFile {}

pub fn unreadable_error(path: &Path) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, UnreadableFile(path.to_path_buf()))
}

/// 저장이 [`UnreadableFile`] 때문에 거부된 오류인지.
pub fn is_unreadable_err(e: &io::Error) -> bool {
    e.get_ref().is_some_and(|i| i.is::<UnreadableFile>())
}

/// [`read_yaml`] 결과.
pub enum YamlRead<T> {
    /// 파일 없음 또는 빈 파일 — 새로 만들어도 안전.
    Missing,
    /// 정상 파싱.
    Parsed(T),
    /// 존재하지만 읽거나 파싱할 수 없음. 덮어쓰기 금지.
    Unreadable,
}

/// YAML 파일을 읽는다. 실패 시 위치(줄·열)만 로그에 남긴다 — 단어는 사용자 데이터라
/// serde 오류 문구를 싣지 않는다. `log` 는 true 일 때만 기록(저장 전 점검은 조용히).
pub fn read_yaml<T: serde::de::DeserializeOwned>(path: &Path, what: &str, log: bool) -> YamlRead<T> {
    let content = match fs::read_to_string(path) {
        Ok(c) => c,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return YamlRead::Missing,
        Err(_) => {
            if log {
                crate::unim_log!("ENGINE", "{what} 읽기 실패 — 읽기 전용(빈 목록)으로 동작");
            }
            return YamlRead::Unreadable;
        }
    };
    if content.trim().is_empty() {
        return YamlRead::Missing;
    }
    match serde_yaml::from_str::<T>(&content) {
        Ok(v) => YamlRead::Parsed(v),
        Err(e) => {
            if log {
                let loc = e.location();
                crate::unim_log!(
                    "ENGINE",
                    "{what} 파싱 실패 (줄 {}, 열 {}) — 읽기 전용(빈 목록)으로 동작",
                    loc.as_ref().map_or(0, |l| l.line()),
                    loc.as_ref().map_or(0, |l| l.column())
                );
            }
            YamlRead::Unreadable
        }
    }
}

/// 깨진 파일을 `<path>.broken-YYYYMMDD-HHMMSS` 로 옮긴다(덮어쓰지 않음). 새 파일로 시작할 때 쓴다.
pub fn quarantine(path: &Path) -> io::Result<PathBuf> {
    let stamp = chrono::Local::now().format("%Y%m%d-%H%M%S").to_string();
    let make = |suffix: &str| -> PathBuf {
        let mut s = path.as_os_str().to_owned();
        s.push(format!(".broken-{stamp}{suffix}"));
        PathBuf::from(s)
    };
    let mut target = make("");
    let mut n = 0u32;
    while target.exists() {
        n += 1;
        if n > 1000 {
            return Err(io::Error::new(
                io::ErrorKind::AlreadyExists,
                "보관 이름을 정할 수 없음",
            ));
        }
        target = make(&format!("-{n}"));
    }
    fs::rename(path, &target)?;
    Ok(target)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    fn unique_temp_path(name: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        std::env::temp_dir().join(format!(
            "unim-atomic-io-test-{}-{}-{}",
            std::process::id(),
            nanos,
            name
        ))
    }

    #[test]
    fn writes_and_replaces_content() {
        let path = unique_temp_path("basic");
        atomic_write(&path, "hello").unwrap();
        assert_eq!(fs::read_to_string(&path).unwrap(), "hello");
        atomic_write(&path, "world").unwrap();
        assert_eq!(fs::read_to_string(&path).unwrap(), "world");
        let _ = fs::remove_file(&path);
    }

    // 이하 3건은 Unix 전용 API(퍼미션 비트·symlink)를 쓴다. Windows 타깃에서는
    // 컴파일되지 않으므로 cfg 로 제외한다 — 검증 대상 동작 자체가 Unix 고유다.
    #[test]
    #[cfg(unix)]
    fn preserves_permissions_across_rewrite() {
        use std::os::unix::fs::PermissionsExt;
        let path = unique_temp_path("perms");
        atomic_write(&path, "a").unwrap();
        fs::set_permissions(&path, fs::Permissions::from_mode(0o600)).unwrap();
        atomic_write(&path, "b").unwrap();
        let mode = fs::metadata(&path).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode, 0o600);
        let _ = fs::remove_file(&path);
    }

    #[test]
    #[cfg(unix)]
    fn follows_symlink_to_real_target() {
        let real = unique_temp_path("real");
        let link = unique_temp_path("link");
        atomic_write(&real, "original").unwrap();
        std::os::unix::fs::symlink(&real, &link).unwrap();

        atomic_write(&link, "updated-via-link").unwrap();

        // 링크는 그대로 심볼릭 링크여야 한다.
        assert!(fs::symlink_metadata(&link).unwrap().file_type().is_symlink());
        // 실제 대상 파일 내용이 갱신되어 있어야 한다.
        assert_eq!(fs::read_to_string(&real).unwrap(), "updated-via-link");
        assert_eq!(fs::read_to_string(&link).unwrap(), "updated-via-link");

        let _ = fs::remove_file(&link);
        let _ = fs::remove_file(&real);
    }

    #[test]
    fn concurrent_unique_tmp_names_do_not_collide() {
        let target = unique_temp_path("target-for-tmp-naming");
        let a = unique_tmp_path(&target);
        // 나노초 해상도 충돌을 피하기 위해 살짝 지연 후 재계산.
        std::thread::sleep(Duration::from_millis(1));
        let b = unique_tmp_path(&target);
        assert_ne!(a, b, "고정 tmp 파일명은 동시 저장 시 경합을 일으킨다");
    }

    #[test]
    #[cfg(unix)]
    fn cleans_up_tmp_on_broken_symlink_fallback() {
        // 깨진 심링크(대상 없음)에 쓰면 canonicalize가 실패해 원본 경로로
        // 폴백하고, 최종적으로 일반 파일로 교체되어야 한다.
        let missing_target = unique_temp_path("missing-target");
        let link = unique_temp_path("broken-link");
        std::os::unix::fs::symlink(&missing_target, &link).unwrap();

        atomic_write(&link, "recovered").unwrap();

        assert!(!fs::symlink_metadata(&link).unwrap().file_type().is_symlink());
        assert_eq!(fs::read_to_string(&link).unwrap(), "recovered");
        let _ = fs::remove_file(&link);
    }
}
