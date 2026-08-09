//! UNIM 중앙 집중식 로깅 모듈
//!
//! `UNIM_DEVELOP=1` 환경 변수가 설정되면 모든 로그를 콘솔과 파일에 기록합니다.
//! 로그 형식: [YYYY/MM/DD HH:MM:SS] - [모듈] - 메세지
//!
//! 로그 파일은 `~/.unim-log/{session-tag}_{YYYY-MM-DD}_{progname}-{pid}.log` 로
//! 윈도우 세션·날짜·프로세스(앱) 단위로 분리 저장됩니다.
//! session-tag 우선순위: XDG_SESSION_ID > WAYLAND_DISPLAY > DISPLAY.
//! progname 은 `/proc/self/comm` 의 호스트 프로세스 이름 (GTK/Qt IM 모듈은
//! 호스트 앱 프로세스 안에서 동작하므로 자연스럽게 앱별 분리됨).

use std::fs::OpenOptions;
use std::io::Write;
use std::path::PathBuf;
use std::sync::OnceLock;

/// 개발 모드 상태 (한번만 체크)
static DEVELOP_MODE: OnceLock<bool> = OnceLock::new();

/// 개발 모드 체크
///
/// `UNIM_DEVELOP=1` 환경 변수가 설정되어 있으면 true를 반환합니다.
/// 이 값은 프로세스 수명 동안 캐시됩니다.
pub fn is_develop_mode() -> bool {
    *DEVELOP_MODE.get_or_init(|| {
        std::env::var("UNIM_DEVELOP")
            .map(|v| v == "1")
            .unwrap_or(false)
    })
}

fn sanitize_tag(s: &str) -> String {
    s.chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' || c == '_' {
                c
            } else {
                '-'
            }
        })
        .collect()
}

/// 윈도우 세션 식별자 (파일명에 안전한 형태)
fn session_tag() -> String {
    fn nonempty(key: &str) -> Option<String> {
        std::env::var(key).ok().filter(|v| !v.is_empty())
    }

    if let Some(v) = nonempty("XDG_SESSION_ID") {
        return format!("xdg-{}", sanitize_tag(&v));
    }
    if let Some(v) = nonempty("WAYLAND_DISPLAY") {
        return format!("wl-{}", sanitize_tag(&v));
    }
    if let Some(v) = nonempty("DISPLAY") {
        return format!("x11-{}", sanitize_tag(&v));
    }
    "unknown".to_string()
}

/// 호스트 프로세스 이름 (GTK/Qt IM 모듈은 호스트 앱 안에서 동작).
/// `/proc/self/comm` → 실패 시 `current_exe()` basename → 실패 시 "unknown".
fn process_name() -> String {
    if let Ok(s) = std::fs::read_to_string("/proc/self/comm") {
        let trimmed = s.trim();
        if !trimmed.is_empty() {
            return sanitize_tag(trimmed);
        }
    }
    if let Ok(exe) = std::env::current_exe() {
        if let Some(name) = exe.file_name().and_then(|n| n.to_str()) {
            return sanitize_tag(name);
        }
    }
    "unknown".to_string()
}

/// 현재 세션·날짜·프로세스에 해당하는 로그 파일 경로.
pub fn log_file_path() -> Option<PathBuf> {
    let home = dirs::home_dir()?;
    let log_dir = home.join(".unim-log");
    let _ = std::fs::create_dir_all(&log_dir);
    let date = chrono::Local::now().format("%Y-%m-%d");
    let pid = std::process::id();
    Some(log_dir.join(format!(
        "{}_{}_{}-{}.log",
        session_tag(),
        date,
        process_name(),
        pid
    )))
}

/// 로그 출력 함수
///
/// 개발 모드가 활성화되면 로그를 콘솔(stderr)과 세션별 로그 파일에 기록합니다.
///
/// # Arguments
///
/// * `module` - 로그를 출력하는 모듈 이름 (예: "ENGINE", "DBUS", "XIM")
/// * `message` - 로그 메시지
pub fn log_message(module: &str, message: &str) {
    if !is_develop_mode() {
        return;
    }

    let timestamp = chrono::Local::now().format("%Y/%m/%d %H:%M:%S");
    let log_line = format!("[{}] - [{}] - {}", timestamp, module, message);

    // 콘솔 출력
    eprintln!("{}", log_line);

    // 파일 출력
    if let Some(log_path) = log_file_path() {
        if let Ok(mut file) = OpenOptions::new().create(true).append(true).open(&log_path) {
            let _ = writeln!(file, "{}", log_line);
        }
    }
}

/// 통합 로깅 매크로
///
/// `UNIM_DEVELOP=1` 환경 변수가 설정되면 로그를 출력합니다.
///
/// # Examples
///
/// ```no_run
/// use unim::unim_log;
///
/// let keycode = 42u16;
/// let new_mode = "Korean";
/// unim_log!("ENGINE", "키 입력 처리: keycode={:?}", keycode);
/// unim_log!("DBUS", "모드 변경: {:?}", new_mode);
/// ```
#[macro_export]
macro_rules! unim_log {
    ($module:expr, $($arg:tt)*) => {
        $crate::logging::log_message($module, &format!($($arg)*))
    };
}
