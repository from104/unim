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

    // 파일명별 append 핸들을 스레드-로컬 캐시한다. 과거엔 로그 한 줄마다
    // OpenOptions::open → writeln → drop(close) 로 open/close syscall 이 매번 발생했다.
    // 진단 로그가 켜져 있으면 키 입력마다 이 경로를 타므로 open/close 비용이 누적됐다.
    // append 모드라 한 파일을 여러 핸들이 가리켜도 안전하며, Rust std 는 기본적으로
    // FILE_SHARE_DELETE 를 켜므로 캐시 핸들이 열려 있어도 로그 파일 삭제/로테이션이 막히지
    // 않는다. TSF/IMM32 는 STA(단일 스레드) 위주라 스레드당 핸들 1개면 충분하다.
    thread_local! {
        static LOG_HANDLES: std::cell::RefCell<std::collections::HashMap<String, std::fs::File>> =
            std::cell::RefCell::new(std::collections::HashMap::new());
    }

    let pid = std::process::id();
    LOG_HANDLES.with(|cache| {
        let mut map = cache.borrow_mut();
        if !map.contains_key(file) {
            let path = std::env::temp_dir().join(file);
            match std::fs::OpenOptions::new().create(true).append(true).open(path) {
                Ok(fh) => {
                    map.insert(file.to_string(), fh);
                }
                // 열기 실패 시 조용히 포기(크래시 없음). 다음 호출에서 재시도한다.
                Err(_) => return,
            }
        }
        if let Some(f) = map.get_mut(file) {
            let _ = writeln!(f, "[{component} {pid}] {msg}");
        }
    });
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
