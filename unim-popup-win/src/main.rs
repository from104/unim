//! UNIM Windows 팝업 렌더러 (out-of-process). 설계서 §5 구현.
//!
//! 싱글턴 상주 프로세스. 명명 파이프로 TSF DLL(클라이언트)의 render/hide 를 받아
//! 화면 정중앙에 한자/특수문자/이모지 후보 팝업을 GDI 로 그린다.
//!
//! 금지 (§8.2):
//! - 팝업 창 WM_DESTROY 에서 PostQuitMessage 금지 — shutdown/WM_ENDSESSION 단 한 곳만.
//! - GetCursorPos 기반 위치 폴백 금지 (§5.5).
//!
//! 크로스플랫폼 게이팅: 이 크레이트는 Windows 전용이지만 워크스페이스 멤버이므로
//! Linux `make build`(cargo build --workspace) / `cargo test --workspace` 에 포함된다.
//! unim-tsf 와 동일 패턴 — 모든 windows 의존 코드를 `#[cfg(windows)]` 로 게이트하고
//! non-Windows 에는 빈 `fn main()` 스텁만 둔다 (E0432 unresolved crate `windows` 방지).
#![cfg_attr(windows, windows_subsystem = "windows")]

#[cfg(not(windows))]
fn main() {
    // Windows 전용 렌더러 — 다른 플랫폼에서는 아무 일도 하지 않는다.
    // 워크스페이스 빌드/테스트 통과용 스텁.
}

#[cfg(windows)]
mod d2d;
#[cfg(windows)]
mod logging;
#[cfg(windows)]
mod pipe_server;
#[cfg(windows)]
mod protocol;
#[cfg(windows)]
mod render;
#[cfg(windows)]
mod window;

#[cfg(windows)]
use std::collections::VecDeque;
#[cfg(windows)]
use std::sync::{Arc, Mutex};

#[cfg(windows)]
use windows::core::PCWSTR;
#[cfg(windows)]
use windows::Win32::Foundation::{GetLastError, HANDLE, HWND};
#[cfg(windows)]
use windows::Win32::Storage::FileSystem::{
    CreateFileW, WriteFile, FILE_GENERIC_WRITE, FILE_SHARE_READ, FILE_SHARE_WRITE, OPEN_EXISTING,
};
#[cfg(windows)]
use windows::Win32::System::Threading::{CreateMutexW, GetCurrentProcessId};
#[cfg(windows)]
use windows::Win32::UI::HiDpi::{
    SetProcessDpiAwarenessContext, DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2,
};
#[cfg(windows)]
use windows::Win32::UI::WindowsAndMessaging::*;

#[cfg(windows)]
use pipe_server::{Incoming, IncomingQueue, WM_APP_CMD};
#[cfg(windows)]
use protocol::{RenderState, WireMsg};

#[cfg(windows)]
const SINGLETON_MUTEX: &str = r"Local\unim-popup-win-singleton";
#[cfg(windows)]
const VERSION: &str = env!("CARGO_PKG_VERSION");

#[cfg(windows)]
fn to_wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

#[cfg(windows)]
fn main() {
    let args: Vec<String> = std::env::args().collect();

    logging::init();
    logln!("=== unim-popup-win start v{VERSION} args={:?} ===", &args[1..]);

    // 진단/제어 인자 — 이들은 싱글턴/서버 기동 전에 처리.
    if args.iter().any(|a| a == "--ping") {
        let code = run_ping();
        logln!("--ping exit code={code}");
        std::process::exit(code);
    }
    if args.iter().any(|a| a == "--shutdown") {
        send_control("shutdown");
        logln!("--shutdown sent");
        std::process::exit(0);
    }

    // 싱글턴 — 세션별 네임스페이스. 중복 기동이면 즉시 exit 0.
    let _mutex = match acquire_singleton() {
        Some(h) => h,
        None => {
            logln!("singleton: already running — exit 0");
            std::process::exit(0);
        }
    };

    // Per-Monitor V2 DPI awareness.
    unsafe {
        match SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2) {
            Ok(()) => logln!("dpi: PER_MONITOR_AWARE_V2 set"),
            Err(e) => logln!("dpi: SetProcessDpiAwarenessContext failed ({e}) — continuing"),
        }
    }

    // 팝업 HWND 1회 생성 (UI 스레드 = 이 스레드).
    let hwnd = match window::create() {
        Ok(h) => h,
        Err(e) => {
            logln!("FATAL: window::create failed: {e}");
            std::process::exit(1);
        }
    };

    // 파이프 서버 (accept 스레드 + 연결당 reader 스레드).
    let queue: IncomingQueue = Arc::new(Mutex::new(VecDeque::new()));
    pipe_server::spawn_server(hwnd, Arc::clone(&queue));

    // 메시지 루프.
    logln!("entering message loop");
    run_message_loop(hwnd, queue);
    logln!("=== unim-popup-win exit ===");
}

/// 싱글턴 뮤텍스 확보. 이미 존재하면 None.
#[cfg(windows)]
fn acquire_singleton() -> Option<HANDLE> {
    unsafe {
        let name = to_wide(SINGLETON_MUTEX);
        let handle = CreateMutexW(None, true, PCWSTR(name.as_ptr())).ok()?;
        let err = GetLastError();
        // ERROR_ALREADY_EXISTS = 183.
        if err.0 == 183 {
            return None;
        }
        Some(handle)
    }
}

// ─── 메시지 루프 + owner 규칙 (§3.4) ─────────────────────────────────────────

/// UI 스레드 전용 owner 상태. reader 스레드는 만지지 않는다.
#[cfg(windows)]
struct OwnerState {
    current_owner_pid: Option<u32>,
    current_owner_conn: Option<u64>,
    /// 재연결/재도색용 마지막 render 캐시는 window 모듈이 보유. 여기선 pid/conn 만.
    last_render_seq: u64,
}

#[cfg(windows)]
fn run_message_loop(hwnd: HWND, queue: IncomingQueue) {
    let mut owner = OwnerState {
        current_owner_pid: None,
        current_owner_conn: None,
        last_render_seq: 0,
    };
    unsafe {
        let mut msg = MSG::default();
        loop {
            let r = GetMessageW(&mut msg, None, 0, 0);
            if r.0 <= 0 {
                // WM_QUIT (0) 또는 에러 (-1) — 루프 종료.
                break;
            }
            // WM_APP_CMD: reader 스레드가 큐를 채우고 깨운 신호.
            if msg.hwnd == hwnd && msg.message == WM_APP_CMD {
                drain_queue(&queue, &mut owner);
                continue;
            }
            // WM_ENDSESSION 은 sent message 라 GetMessageW 큐에 나타나지 않는다 —
            // 로그오프/셧다운 종료 경로(§5.2)는 window::wnd_proc 의 WM_ENDSESSION
            // 케이스에서 처리한다 (§8.2 PostQuitMessage 허용 지점 1).
            let _ = TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
    }
}

#[cfg(windows)]
fn drain_queue(queue: &IncomingQueue, owner: &mut OwnerState) {
    loop {
        let item = {
            let Ok(mut q) = queue.lock() else { return };
            q.pop_front()
        };
        let Some(item) = item else { return };
        match item {
            Incoming::Msg { conn_id, conn_handle, msg } => handle_msg(conn_id, conn_handle, *msg, owner),
            Incoming::Disconnected { conn_id, pid } => {
                // owner 연결 단절 → 즉시 숨김 (§3.4-3).
                if owner.current_owner_conn == Some(conn_id) {
                    logln!("owner disconnected conn_id={conn_id} pid={pid} — hiding popup");
                    window::hide();
                    owner.current_owner_pid = None;
                    owner.current_owner_conn = None;
                } else {
                    logln!("non-owner disconnected conn_id={conn_id} pid={pid}");
                }
            }
        }
    }
}

#[cfg(windows)]
fn handle_msg(conn_id: u64, conn_handle: isize, msg: WireMsg, owner: &mut OwnerState) {
    match msg.cmd.as_str() {
        "render" => {
            let Some(rs) = msg.render else {
                logln!("render cmd without render state pid={} seq={} — ignored", msg.pid, msg.seq);
                return;
            };
            owner.current_owner_pid = Some(msg.pid);
            owner.current_owner_conn = Some(conn_id);
            owner.last_render_seq = msg.seq;
            log_render_summary(conn_id, &msg.pid, msg.seq, msg.first, msg.flash, &rs);
            let flash = msg.flash.unwrap_or(false);
            let owner_hwnd = msg.owner_hwnd.unwrap_or(0);
            // 역방향 송신(§11.E)에 쓸 owner 연결 핸들·seq·owner_hwnd 를 window 에 보관.
            window::show_render(rs, owner_hwnd, flash, conn_handle, msg.seq);
        }
        "hide" => {
            // owner pid 가 보낸 경우에만 숨김 (§3.4-2).
            if owner.current_owner_pid == Some(msg.pid) {
                logln!("hide from owner pid={} seq={} — hiding", msg.pid, msg.seq);
                window::hide();
                owner.current_owner_pid = None;
                owner.current_owner_conn = None;
            } else {
                logln!(
                    "stale hide from pid={} (owner={:?}) seq={} — ignored",
                    msg.pid, owner.current_owner_pid, msg.seq
                );
            }
        }
        "ping" => {
            // V1 클라이언트는 응답을 읽지 않으나, 진단 CLI 호환을 위해 로그만.
            logln!("ping from pid={} seq={}", msg.pid, msg.seq);
        }
        "shutdown" => {
            logln!("shutdown cmd from pid={} seq={} — PostQuitMessage", msg.pid, msg.seq);
            unsafe { PostQuitMessage(0) }; // §8.2 허용 지점 2.
        }
        other => {
            logln!("unknown cmd '{other}' from pid={} seq={} — ignored", msg.pid, msg.seq);
        }
    }
}

#[cfg(windows)]
fn log_render_summary(
    conn_id: u64,
    pid: &u32,
    seq: u64,
    first: Option<bool>,
    flash: Option<bool>,
    rs: &RenderState,
) {
    logln!(
        "recv render conn_id={conn_id} pid={pid} seq={seq} first={first:?} flash={flash:?} \
         kind={} rows={}x{} page={}/{} sel=({},{}) cells={} col_headers={} tabs={} \
         expand_visible={}",
        rs.kind,
        rs.rows,
        rs.cols,
        rs.current_page,
        rs.total_pages,
        rs.sel_row,
        rs.sel_col,
        rs.cells.len(),
        rs.col_headers.len(),
        rs.tab_labels.len(),
        rs.expand_visible,
    );
}

// ─── --ping / --shutdown 헬퍼 ────────────────────────────────────────────────

/// 기존 서버에 control 메시지(shutdown 등)를 보낸다 (fire-and-forget).
#[cfg(windows)]
fn send_control(cmd: &str) {
    let pid = unsafe { GetCurrentProcessId() };
    let line = format!(
        "{{\"v\":1,\"cmd\":\"{cmd}\",\"pid\":{pid},\"seq\":0}}\n"
    );
    let name = pipe_server::pipe_name();
    unsafe {
        let name_w = to_wide(&name);
        let handle = CreateFileW(
            PCWSTR(name_w.as_ptr()),
            FILE_GENERIC_WRITE.0,
            FILE_SHARE_READ | FILE_SHARE_WRITE,
            None,
            OPEN_EXISTING,
            Default::default(),
            None,
        );
        match handle {
            Ok(h) => {
                let bytes = line.as_bytes();
                let mut written: u32 = 0;
                let _ = WriteFile(h, Some(bytes), Some(&mut written), None);
                logln!("send_control '{cmd}' wrote {written} bytes");
                let _ = windows::Win32::Foundation::CloseHandle(h);
            }
            Err(e) => {
                logln!("send_control '{cmd}' connect failed ({e}) err={:?}", GetLastError());
            }
        }
    }
}

/// --ping: 서버에 ping 을 쓰고 exit code 0(연결 성공)/2(연결 실패) 반환.
/// V1 은 응답을 읽지 않으므로 연결 성립 = 생존으로 간주.
#[cfg(windows)]
fn run_ping() -> i32 {
    let pid = unsafe { GetCurrentProcessId() };
    let line = format!("{{\"v\":1,\"cmd\":\"ping\",\"pid\":{pid},\"seq\":0}}\n");
    let name = pipe_server::pipe_name();
    unsafe {
        let name_w = to_wide(&name);
        let handle = CreateFileW(
            PCWSTR(name_w.as_ptr()),
            FILE_GENERIC_WRITE.0,
            FILE_SHARE_READ | FILE_SHARE_WRITE,
            None,
            OPEN_EXISTING,
            Default::default(),
            None,
        );
        match handle {
            Ok(h) => {
                let mut written: u32 = 0;
                let _ = WriteFile(h, Some(line.as_bytes()), Some(&mut written), None);
                let _ = windows::Win32::Foundation::CloseHandle(h);
                logln!("--ping: connected, wrote {written} bytes");
                0
            }
            Err(e) => {
                logln!("--ping: no server ({e}) err={:?}", GetLastError());
                2
            }
        }
    }
}
