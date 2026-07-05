//! 렌더러 로그 — `%TEMP%\unim-popup-win.log` (설계서 §5.6).
//!
//! 형식: `[unim-popup-win pid=N tid=N 2026-06-12T12:34:56.789] message` (ms 정밀도, append).
//! 5 MiB 초과 시 `.old` 로 rename 후 새로 시작 (기동 시 회전 판정).
//!
//! 의도적으로 의존성 0 — std + windows 만으로 ms 타임스탬프를 만든다 (chrono 미사용:
//! 렌더러는 panic=abort 환경이고 의존 그래프를 최소로 유지).

use std::fs::{File, OpenOptions};
use std::io::Write;
use std::path::PathBuf;
use std::sync::OnceLock;
use std::sync::Mutex;

use windows::Win32::System::SystemInformation::GetLocalTime;
use windows::Win32::System::Threading::{GetCurrentProcessId, GetCurrentThreadId};

const ROTATE_BYTES: u64 = 5 * 1024 * 1024;

struct LogState {
    file: Option<File>,
    pid: u32,
}

static LOG: OnceLock<Mutex<LogState>> = OnceLock::new();

fn log_path() -> PathBuf {
    let dir = std::env::temp_dir();
    dir.join("unim-popup-win.log")
}

/// 기동 시 1회 호출. 5 MiB 초과 회전 후 append 핸들 확보.
pub fn init() {
    let path = log_path();
    // 회전: 기존 파일이 5 MiB 초과면 .old 로 rename.
    if let Ok(meta) = std::fs::metadata(&path) {
        if meta.len() > ROTATE_BYTES {
            let old = path.with_extension("log.old");
            let _ = std::fs::remove_file(&old);
            let _ = std::fs::rename(&path, &old);
        }
    }
    let file = OpenOptions::new().create(true).append(true).open(&path).ok();
    let pid = unsafe { GetCurrentProcessId() };
    let _ = LOG.set(Mutex::new(LogState { file, pid }));
}

fn timestamp() -> String {
    // GetLocalTime 로 ms 정밀도 ISO-유사 문자열 생성.
    let st = unsafe { GetLocalTime() };
    format!(
        "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}.{:03}",
        st.wYear, st.wMonth, st.wDay, st.wHour, st.wMinute, st.wSecond, st.wMilliseconds
    )
}

/// 한 줄 로그. 실패는 조용히 무시 (로깅이 동작을 막지 않는다).
pub fn log(msg: &str) {
    let Some(cell) = LOG.get() else { return };
    let Ok(mut state) = cell.lock() else { return };
    let tid = unsafe { GetCurrentThreadId() };
    let pid = state.pid;
    let line = format!(
        "[unim-popup-win pid={pid} tid={tid} {}] {msg}\n",
        timestamp()
    );
    if let Some(f) = state.file.as_mut() {
        let _ = f.write_all(line.as_bytes());
        let _ = f.flush();
    }
}

/// 편의 매크로 — `logln!("connected pid={}", pid)`.
#[macro_export]
macro_rules! logln {
    ($($arg:tt)*) => {
        $crate::logging::log(&format!($($arg)*))
    };
}
