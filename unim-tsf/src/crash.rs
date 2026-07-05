//! 크래시 리포팅 — panic=abort 하에서 abort 직전 크래시 마커를 남긴다.
//!
//! ## 배경(착시 정리)
//! 이 크레이트는 COM/C-ABI 경계(msctf.dll ↔ unim_tsf.dll)를 넘는 언와인딩이 UB 라
//! 워크스페이스 전역 `panic = "abort"` 로 빌드된다(루트 Cargo.toml). 그 결과
//! 코드 곳곳의 `std::panic::catch_unwind(...)` 는 **아무것도 잡지 못하는 no-op** 이다:
//! 패닉 런타임이 랜딩 패드에 도달하기 전에 abort 를 호출하기 때문이다. 즉
//! catch_unwind 들은 "패닉이 COM 경계를 안 넘는다"는 **안전 착시**만 줄 뿐이며,
//! 실제 장애는 여전히 abort → 호스트 앱(msctf/워드/크롬) 프로세스 종료로 나타나
//! UNIM 에 귀속되지 않고 **불가시**하다.
//!
//! ## 이 모듈이 하는 일
//! `std::panic::set_hook` 은 panic=abort 여도 **abort 직전에 반드시 호출**된다.
//! DLL 로드 시(`DllMain` PROCESS_ATTACH) 훅을 1회 설치해, 패닉 발생 시 abort 로
//! 프로세스가 사라지기 전에 크래시 마커(타임스탬프·위치·스레드·메시지)를
//! `%APPDATA%\unim\crash\` 에 파일로 남긴다. 이로써 장애가 UNIM 귀속·가시화된다.
//!
//! ## 안전 규칙(재진입·경계)
//! 훅 본문은 **절대 패닉하면 안 된다**(double panic → 즉시 abort, 마커 유실).
//! 따라서 `unwrap`/인덱싱/할당 실패 패닉을 유발할 수 있는 연산을 피하고, 모든
//! 오류를 `let _ =`/`match` 로 삼킨다. COM 재진입을 피하려 windows COM API 는
//! 호출하지 않고 `std::fs`·`std::env` 와 경량 syscall(GetLocalTime)만 쓴다.
//! 파일명은 pid+epoch millis 로 충돌을 피해 여러 스레드가 동시에 패닉해도
//! 서로 덮어쓰지 않는다.

use std::path::PathBuf;
use std::sync::Once;

/// DLL 로드 시 1회 호출. 패닉 훅을 설치한다(멱등 — Once 가드).
///
/// `set_hook` 자체는 내부 RwLock 취득 + Box 할당뿐이라 로더 락 하에서도 안전하다.
/// 훅 클로저의 파일 I/O 는 설치 시점이 아니라 **패닉 시점**에만 실행된다.
pub fn install_panic_hook() {
    static ONCE: Once = Once::new();
    ONCE.call_once(|| {
        std::panic::set_hook(Box::new(|info| {
            // 훅은 절대 패닉하면 안 된다 — 본문 전체를 무패닉으로 작성한다.
            write_crash_marker(info);
        }));
    });
}

/// 크래시 마커를 파일로 기록한다. 어떤 실패도 조용히 무시(무패닉).
fn write_crash_marker(info: &std::panic::PanicHookInfo<'_>) {
    // 1) 패닉 메시지 payload 추출(&str / String 두 케이스).
    let msg: &str = if let Some(s) = info.payload().downcast_ref::<&str>() {
        s
    } else if let Some(s) = info.payload().downcast_ref::<String>() {
        s.as_str()
    } else {
        "<non-string panic payload>"
    };

    // 2) 위치(파일:행:열).
    let location = match info.location() {
        Some(l) => format!("{}:{}:{}", l.file(), l.line(), l.column()),
        None => "<unknown>".to_string(),
    };

    // 3) 스레드 이름 + 벽시계 시각.
    let thread_name = std::thread::current().name().unwrap_or("<unnamed>").to_string();
    let (stamp, epoch_ms) = local_timestamp();
    let pid = std::process::id();

    let body = format!(
        "UNIM TSF crash marker\n\
         time:     {stamp}\n\
         pid:      {pid}\n\
         thread:   {thread_name}\n\
         location: {location}\n\
         message:  {msg}\n"
    );

    // 4) 대상 디렉터리 결정 후 파일 기록. 실패 시 %TEMP% 폴백.
    let file_name = format!("crash-{pid}-{epoch_ms}.txt");
    if let Some(dir) = crash_dir() {
        let _ = std::fs::create_dir_all(&dir);
        if std::fs::write(dir.join(&file_name), &body).is_ok() {
            return;
        }
    }
    // 폴백: %TEMP%\unim\crash\ (APPDATA 미해석 또는 쓰기 실패 시 마커 유실 방지).
    let fallback = std::env::temp_dir().join("unim").join("crash");
    let _ = std::fs::create_dir_all(&fallback);
    let _ = std::fs::write(fallback.join(&file_name), &body);
}

/// 크래시 마커 디렉터리 `<roaming>\unim\crash` 를 결정한다.
///
/// sandbox_paths::init 이 set 한 실제(리다이렉트 안 된) 로밍 베이스를 우선 쓰고,
/// 없으면 `%APPDATA%`, 다시 `%USERPROFILE%\AppData\Roaming` 으로 폴백한다.
/// (config.rs 와 동일하게 베이스에 `unim` 을 join 한 뒤 `crash` 를 붙인다.)
fn crash_dir() -> Option<PathBuf> {
    let base = env_nonempty("UNIM_DATA_DIR")
        .or_else(|| env_nonempty("UNIM_CONFIG_DIR"))
        .or_else(|| env_nonempty("APPDATA"))
        .or_else(|| env_nonempty("USERPROFILE").map(|p| p.join("AppData").join("Roaming")))?;
    Some(base.join("unim").join("crash"))
}

fn env_nonempty(key: &str) -> Option<PathBuf> {
    match std::env::var(key) {
        Ok(v) if !v.is_empty() => Some(PathBuf::from(v)),
        _ => None,
    }
}

/// (사람이 읽는 로컬 시각 문자열, epoch millis) 를 반환한다.
///
/// epoch millis 는 파일명 충돌 회피에, 시각 문자열은 마커 가독성에 쓴다.
/// GetLocalTime 은 SYSTEMTIME 구조체만 채우는 경량 syscall 로 재진입 안전하다.
fn local_timestamp() -> (String, u128) {
    let epoch_ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0);
    let st = unsafe { windows::Win32::System::SystemInformation::GetLocalTime() };
    let stamp = format!(
        "{:04}-{:02}-{:02} {:02}:{:02}:{:02}.{:03} (local)",
        st.wYear, st.wMonth, st.wDay, st.wHour, st.wMinute, st.wSecond, st.wMilliseconds
    );
    (stamp, epoch_ms)
}
