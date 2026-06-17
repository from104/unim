//! Debug logging to %TEMP% files and optional OutputDebugStringW sink.

/// 진단 로그 한 줄을 `%TEMP%\{file}` 에 append 한다. `also_output_debug_string`이면
/// `OutputDebugStringW`에도 쓴다. 태그 라인 형식: `[{component} {PID}] {msg}`.
///
/// **게이트 없음**: 이 함수는 호출되면 무조건 쓴다. ON/OFF 판단(UNIM_DEBUG_LOG /
/// cfg!(debug_assertions))은 consumer의 얇은 래퍼에서 한다 (동작 보존).
/// 실패해도 무시(크래시 없음).
///
/// - `component`: 로그 태그 라벨 (예: "unim-tsf", "unim-imm32").
/// - `file`:      `%TEMP%` 하위 파일명 (예: "unim-tsf.log").
/// - `msg`:       로그 본문.
/// - `also_output_debug_string`: true면 OutputDebugStringW 동시 출력(IMM32 동작).
pub fn dbg_log(component: &str, file: &str, msg: &str, also_output_debug_string: bool) {
    if also_output_debug_string {
        #[link(name = "kernel32")]
        extern "system" {
            fn OutputDebugStringW(lpoutputstring: windows::core::PCWSTR);
        }
        let tagged = format!("[{component} {}] {msg}\0", std::process::id());
        let wide: Vec<u16> = tagged.encode_utf16().collect();
        unsafe { OutputDebugStringW(windows::core::PCWSTR(wide.as_ptr())); }
    }

    use std::io::Write;
    let path = std::env::temp_dir().join(file);
    if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(path) {
        let _ = writeln!(f, "[{component} {}] {msg}", std::process::id());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// dbg_log with also_output_debug_string=false writes a line to %TEMP%/<file>.
    /// We use a unique temp filename to avoid colliding with real log files.
    #[test]
    fn dbg_log_writes_to_temp_file() {
        let file_name = format!("unim-test-dbg-{}.log", std::process::id());
        let path = std::env::temp_dir().join(&file_name);
        // Clean up any leftover from a prior run.
        let _ = std::fs::remove_file(&path);

        dbg_log("test-component", &file_name, "hello world", false);

        let contents = std::fs::read_to_string(&path).expect("log file should be written");
        assert!(
            contents.contains("test-component"),
            "tag missing: {contents}"
        );
        assert!(contents.contains("hello world"), "msg missing: {contents}");

        // Clean up.
        let _ = std::fs::remove_file(&path);
    }

    /// dbg_log appends multiple lines (does not truncate).
    #[test]
    fn dbg_log_appends() {
        let file_name = format!("unim-test-dbg-append-{}.log", std::process::id());
        let path = std::env::temp_dir().join(&file_name);
        let _ = std::fs::remove_file(&path);

        dbg_log("comp", &file_name, "line one", false);
        dbg_log("comp", &file_name, "line two", false);

        let contents = std::fs::read_to_string(&path).expect("log file should exist");
        assert!(contents.contains("line one"), "first line missing");
        assert!(contents.contains("line two"), "second line missing");

        let _ = std::fs::remove_file(&path);
    }

    /// dbg_log with also_output_debug_string=true does not panic (OutputDebugStringW may be a no-op
    /// outside a debugger, but must not crash).
    #[test]
    fn dbg_log_output_debug_string_no_panic() {
        let file_name = format!("unim-test-dbg-ods-{}.log", std::process::id());
        let path = std::env::temp_dir().join(&file_name);
        let _ = std::fs::remove_file(&path);

        // Should not panic even with also_output_debug_string=true.
        dbg_log("test-ods", &file_name, "ods test", true);

        let contents = std::fs::read_to_string(&path).expect("log file should be written");
        assert!(contents.contains("ods test"));

        let _ = std::fs::remove_file(&path);
    }
}
