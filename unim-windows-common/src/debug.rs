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

    // 로그 파일 1개당 최대 크기 — 넘으면 `<file>.1` 로 회전한다(1세대만 보관, 이전
    // `.1` 은 덮어씀). 무회전 누적으로 로그가 무한정 자라던 것을 막는다.
    const MAX_LOG_BYTES: u64 = 5 * 1024 * 1024;

    // 파일명별 (append 핸들, 현재 크기) 를 스레드-로컬 캐시한다. 과거엔 로그 한
    // 줄마다 OpenOptions::open → writeln → drop(close) 로 open/close syscall 이
    // 매번 발생했다. 진단 로그가 켜져 있으면 키 입력마다 이 경로를 타므로 open/close
    // 비용이 누적됐다. append 모드라 한 파일을 여러 핸들이 가리켜도 안전하며, Rust
    // std 는 기본적으로 FILE_SHARE_DELETE 를 켜므로 캐시 핸들이 열려 있어도 로그
    // 파일 rename(회전)이 막히지 않는다. TSF/IMM32 는 STA(단일 스레드) 위주라
    // 스레드당 핸들 1개면 충분하다 — 크기 카운터도 스레드-로컬 근사치라 여러
    // 스레드가 같은 파일에 동시에 쓰면 약간 어긋날 수 있지만(회전 시점이 몇 줄
    // 이르거나 늦는 정도), STA 가정상 실질적으로 단일 스레드만 쓴다.
    thread_local! {
        static LOG_HANDLES: std::cell::RefCell<std::collections::HashMap<String, (std::fs::File, u64)>> =
            std::cell::RefCell::new(std::collections::HashMap::new());
    }

    let pid = std::process::id();
    let line = format!("[{component} {pid}] {msg}\n");
    let line_len = line.len() as u64;

    LOG_HANDLES.with(|cache| {
        let mut map = cache.borrow_mut();
        let path = std::env::temp_dir().join(file);

        if !map.contains_key(file) {
            match std::fs::OpenOptions::new().create(true).append(true).open(&path) {
                Ok(fh) => {
                    let size = fh.metadata().map(|m| m.len()).unwrap_or(0);
                    map.insert(file.to_string(), (fh, size));
                }
                // 열기 실패 시 조용히 포기(크래시 없음). 다음 호출에서 재시도한다.
                Err(_) => return,
            }
        }

        if let Some((fh, size)) = map.get_mut(file) {
            if *size + line_len > MAX_LOG_BYTES {
                let rotated = std::env::temp_dir().join(format!("{file}.1"));
                let _ = std::fs::remove_file(&rotated);
                // 열린 핸들이 있어도 FILE_SHARE_DELETE 덕에 rename 은 성공한다(위 주석).
                let _ = std::fs::rename(&path, &rotated);
                match std::fs::OpenOptions::new().create(true).append(true).open(&path) {
                    Ok(new_fh) => {
                        *fh = new_fh;
                        *size = 0;
                    }
                    // 재오픈 실패 시 기존(회전된 경로를 가리키는 채로 남은) 핸들에
                    // 계속 쓴다 — 크래시보다는 낫다(최선 노력).
                    Err(_) => {}
                }
            }
            if fh.write_all(line.as_bytes()).is_ok() {
                *size += line_len;
            }
        }
    });
}

/// 프로세스당 1회, 버전·빌드 타임스탬프·로드된 DLL 경로+mtime 을 남기는 진단 배너.
///
/// `dbg_log` 와 마찬가지로 이 함수 자체엔 게이트가 없다 — ON/OFF 판단은 호출자(각
/// crate 의 얇은 wrapper, 예: `unim-tsf::register::log_startup_banner`)가 한다.
/// 실제 "프로세스당 1회" 는 함수-로컬 `OnceLock` 이 보장한다 — 여러 스레드/여러
/// TIP 인스턴스가 동시에 최초 호출해도 기록은 딱 한 번만 일어난다(경쟁한 나머지
/// 호출은 조용히 스킵). `dll_path` 파일의 mtime 을 못 읽으면(권한·경합 등) "?" 로
/// 남기고 나머지 필드는 그대로 기록한다 — 배너 자체가 실패로 사라지진 않는다.
pub fn log_startup_banner(
    component: &str,
    file: &str,
    version: &str,
    build_timestamp: &str,
    dll_path: &str,
) {
    use std::sync::OnceLock;
    static BANNER_ONCE: OnceLock<()> = OnceLock::new();
    BANNER_ONCE.get_or_init(|| {
        let mtime_epoch = std::fs::metadata(dll_path)
            .and_then(|m| m.modified())
            .ok()
            .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
            .map(|d| d.as_secs().to_string())
            .unwrap_or_else(|| "?".to_string());
        let msg = format!(
            "===== UNIM startup banner ===== version={version} build={build_timestamp} \
             dll={dll_path} dll_mtime_epoch={mtime_epoch}"
        );
        dbg_log(component, file, &msg, false);
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
