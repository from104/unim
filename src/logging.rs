//! UNIM 중앙 집중식 로깅 모듈
//!
//! `UNIM_DEVELOP=1` 환경 변수가 설정되면 모든 로그를 콘솔과 파일에 기록합니다.
//! 로그 형식: [YYYY/MM/DD HH:MM:SS] - [모듈] - 메세지

use std::fs::OpenOptions;
use std::io::Write;
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

/// 로그 출력 함수
///
/// 개발 모드가 활성화되면 로그를 콘솔(stderr)과 `~/.unim-errors.log` 파일에 기록합니다.
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
    if let Some(home) = dirs::home_dir() {
        let log_path = home.join(".unim-errors.log");
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
/// ```
/// use unim::unim_log;
///
/// unim_log!("ENGINE", "키 입력 처리: keycode={:?}", keycode);
/// unim_log!("DBUS", "모드 변경: {:?}", new_mode);
/// ```
#[macro_export]
macro_rules! unim_log {
    ($module:expr, $($arg:tt)*) => {
        $crate::logging::log_message($module, &format!($($arg)*))
    };
}
