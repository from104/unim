//! 명명 파이프 서버 — 설계서 §3.1 / §5.2 스레드 모델.
//!
//! - accept 스레드: `CreateNamedPipeW` (PIPE_UNLIMITED_INSTANCES) 루프, 연결마다 reader spawn.
//! - 연결당 reader 스레드: 라인 단위 파싱 → `IncomingQueue` push → `PostMessageW` 로 UI wake.
//! - **reader 스레드에서 HWND 직접 조작 금지** (§5.2). UI 스레드만 창을 만진다.
//! - SDDL 보안 기술자: ALL APPLICATION PACKAGES + Low-IL 쓰기 허용 (§3.1).

use std::collections::VecDeque;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use windows::core::PCWSTR;
use windows::Win32::Foundation::{
    CloseHandle, GetLastError, HANDLE, HWND, INVALID_HANDLE_VALUE, LPARAM, WPARAM,
};
use windows::Win32::Security::Authorization::ConvertStringSecurityDescriptorToSecurityDescriptorW;
use windows::Win32::Security::{PSECURITY_DESCRIPTOR, SECURITY_ATTRIBUTES};
use windows::Win32::Storage::FileSystem::{
    ReadFile, FILE_FLAG_FIRST_PIPE_INSTANCE, PIPE_ACCESS_DUPLEX,
};
use windows::Win32::System::Pipes::{
    ConnectNamedPipe, CreateNamedPipeW, GetNamedPipeClientProcessId, PIPE_READMODE_BYTE,
    PIPE_TYPE_BYTE, PIPE_UNLIMITED_INSTANCES, PIPE_WAIT,
};
use windows::Win32::System::RemoteDesktop::ProcessIdToSessionId;
use windows::Win32::System::Threading::GetCurrentProcessId;

use crate::logln;
use crate::protocol::{WireMsg, MAX_LINE_BYTES, PIPE_BASE_NAME, PIPE_SDDL, WIRE_VERSION};

/// reader → UI 스레드로 전달되는 항목. `conn_id` 는 owner 단절 추적용.
pub enum Incoming {
    /// 정상 파싱된 메시지 (conn_id 동봉). WireMsg 가 커서 Box 로 간접화 (clippy).
    /// `conn_handle` = 이 연결의 파이프 핸들 raw(isize) — 역방향 송신(§11.E)에 owner
    /// 연결로 WriteFile 하기 위해 UI 스레드가 OwnerState/window 에 보관한다. reader 와
    /// UI 가 같은 핸들을 동시에 ReadFile/WriteFile 하나 방향이 반대라 안전(byte pipe).
    Msg { conn_id: u64, conn_handle: isize, msg: Box<WireMsg> },
    /// 연결 종료 — owner 가 이 conn 이면 UI 스레드가 즉시 숨김 (§3.4-3).
    Disconnected { conn_id: u64, pid: u32 },
}

/// reader 스레드와 UI 스레드가 공유하는 큐.
pub type IncomingQueue = Arc<Mutex<VecDeque<Incoming>>>;

/// UI 스레드 wake 용 커스텀 메시지.
pub const WM_APP_CMD: u32 = windows::Win32::UI::WindowsAndMessaging::WM_APP + 1;

static NEXT_CONN_ID: AtomicU64 = AtomicU64::new(1);

/// 세션별 파이프 이름: `\\.\pipe\unim-popup-win.<session_id>`.
pub fn pipe_name() -> String {
    let mut sid: u32 = 0;
    let pid = unsafe { GetCurrentProcessId() };
    let ok = unsafe { ProcessIdToSessionId(pid, &mut sid) };
    if ok.is_err() {
        logln!("pipe_name: ProcessIdToSessionId failed err={:?}", unsafe {
            GetLastError()
        });
    }
    format!("{PIPE_BASE_NAME}.{sid}")
}

fn to_wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

/// SDDL 문자열로 SECURITY_ATTRIBUTES 를 만든다. 실패해도 None 반환(기본 보안으로 계속).
/// 반환된 PSECURITY_DESCRIPTOR 는 LocalFree 로 해제해야 하나, 서버 수명 = 프로세스 수명이라
/// 의도적으로 leak 한다 (상주 프로세스, 1회 할당).
fn build_security_attributes() -> Option<SECURITY_ATTRIBUTES> {
    let sddl_w = to_wide(PIPE_SDDL);
    let mut psd = PSECURITY_DESCRIPTOR::default();
    let r = unsafe {
        ConvertStringSecurityDescriptorToSecurityDescriptorW(
            PCWSTR(sddl_w.as_ptr()),
            1, // SDDL_REVISION_1
            &mut psd,
            None,
        )
    };
    match r {
        Ok(()) => {
            logln!("security: SDDL applied = {PIPE_SDDL}");
            Some(SECURITY_ATTRIBUTES {
                nLength: std::mem::size_of::<SECURITY_ATTRIBUTES>() as u32,
                lpSecurityDescriptor: psd.0,
                bInheritHandle: false.into(),
            })
        }
        Err(e) => {
            logln!(
                "security: SDDL parse FAILED ({e}) err={:?} — falling back to default ACL",
                unsafe { GetLastError() }
            );
            None
        }
    }
}

/// accept 루프를 별도 스레드에서 돌린다. UI 스레드는 즉시 메시지 펌프로 복귀.
pub fn spawn_server(msg_hwnd: HWND, queue: IncomingQueue) {
    let name = pipe_name();
    let hwnd_raw = msg_hwnd.0 as isize;
    std::thread::spawn(move || {
        // SECURITY_ATTRIBUTES 는 스레드 로컬로 1회 구성 후 모든 인스턴스에서 재사용.
        let sa = build_security_attributes();
        let name_w = to_wide(&name);
        let mut first_instance = true;
        let mut instance_count: u64 = 0;
        loop {
            let flags = if first_instance {
                PIPE_ACCESS_DUPLEX | FILE_FLAG_FIRST_PIPE_INSTANCE
            } else {
                PIPE_ACCESS_DUPLEX
            };
            let sa_ptr: Option<*const SECURITY_ATTRIBUTES> =
                sa.as_ref().map(|s| s as *const SECURITY_ATTRIBUTES);
            let handle = unsafe {
                CreateNamedPipeW(
                    PCWSTR(name_w.as_ptr()),
                    flags,
                    PIPE_TYPE_BYTE | PIPE_READMODE_BYTE | PIPE_WAIT,
                    PIPE_UNLIMITED_INSTANCES,
                    MAX_LINE_BYTES as u32, // out buffer
                    MAX_LINE_BYTES as u32, // in buffer
                    0,                     // default timeout
                    sa_ptr,
                )
            };
            first_instance = false;
            if handle == INVALID_HANDLE_VALUE {
                logln!("pipe: CreateNamedPipeW failed err={:?}", unsafe {
                    GetLastError()
                });
                // 폭주 방지 — 잠깐 쉬고 재시도.
                std::thread::sleep(std::time::Duration::from_millis(500));
                continue;
            }
            // 클라이언트 연결 대기 (블로킹).
            let connected = unsafe { ConnectNamedPipe(handle, None) };
            // ConnectNamedPipe 는 클라이언트가 CreateNamedPipe~ConnectNamedPipe 사이에
            // 이미 연결됐으면 ERROR_PIPE_CONNECTED(=Err) 를 반환하나 정상 연결이다.
            if connected.is_err() {
                let e = unsafe { GetLastError() };
                // ERROR_PIPE_CONNECTED = 535.
                if e.0 != 535 {
                    logln!("pipe: ConnectNamedPipe failed err={e:?}");
                    unsafe {
                        let _ = CloseHandle(handle);
                    }
                    continue;
                }
            }
            instance_count += 1;
            let conn_id = NEXT_CONN_ID.fetch_add(1, Ordering::SeqCst);
            // 클라이언트 pid 조회 (진단·owner 단절 로그용).
            let mut client_pid: u32 = 0;
            let _ = unsafe { GetNamedPipeClientProcessId(handle, &mut client_pid) };
            logln!(
                "pipe: client connected conn_id={conn_id} client_pid={client_pid} \
                 active_instances={instance_count}"
            );
            // 연결당 reader 스레드.
            let q = Arc::clone(&queue);
            let h_val = handle.0 as isize;
            std::thread::spawn(move || {
                reader_loop(HANDLE(h_val as *mut _), conn_id, client_pid, q, hwnd_raw);
            });
        }
    });
}

/// 한 연결의 reader: 바이트 스트림을 라인으로 쪼개 파싱, 큐 push, UI wake.
fn reader_loop(
    handle: HANDLE,
    conn_id: u64,
    client_pid: u32,
    queue: IncomingQueue,
    hwnd_raw: isize,
) {
    let conn_handle = handle.0 as isize; // 역방향 송신(§11.E)용 raw 핸들.
    let mut acc: Vec<u8> = Vec::with_capacity(4096);
    let mut buf = vec![0u8; 64 * 1024];
    loop {
        let mut read: u32 = 0;
        let r = unsafe { ReadFile(handle, Some(&mut buf), Some(&mut read), None) };
        if r.is_err() || read == 0 {
            logln!(
                "pipe: reader EOF/broken conn_id={conn_id} client_pid={client_pid} err={:?}",
                unsafe { GetLastError() }
            );
            break;
        }
        acc.extend_from_slice(&buf[..read as usize]);
        // 누적 버퍼에서 \n 기준으로 라인 추출.
        while let Some(pos) = acc.iter().position(|&b| b == b'\n') {
            let line: Vec<u8> = acc.drain(..=pos).collect();
            let line = &line[..line.len() - 1]; // \n 제거
            handle_line(line, conn_id, conn_handle, &queue, hwnd_raw);
        }
        // 라인 종결 없이 비대해진 버퍼는 버린다 (악성/깨진 스트림 방어).
        if acc.len() > MAX_LINE_BYTES {
            logln!(
                "pipe: oversized partial line dropped conn_id={conn_id} bytes={}",
                acc.len()
            );
            acc.clear();
        }
    }
    // 연결 종료 통지 — owner 단절 시 UI 스레드가 즉시 숨김.
    if let Ok(mut q) = queue.lock() {
        q.push_back(Incoming::Disconnected {
            conn_id,
            pid: client_pid,
        });
    }
    wake_ui(hwnd_raw);
    unsafe {
        let _ = CloseHandle(handle);
    }
}

fn handle_line(line: &[u8], conn_id: u64, conn_handle: isize, queue: &IncomingQueue, hwnd_raw: isize) {
    if line.is_empty() {
        return;
    }
    if line.len() > MAX_LINE_BYTES {
        logln!("pipe: line too long ({} bytes) dropped conn_id={conn_id}", line.len());
        return;
    }
    let text = match std::str::from_utf8(line) {
        Ok(t) => t,
        Err(e) => {
            logln!("pipe: non-UTF8 line dropped conn_id={conn_id} err={e}");
            return;
        }
    };
    let msg: WireMsg = match serde_json::from_str(text) {
        Ok(m) => m,
        Err(e) => {
            logln!("pipe: JSON parse failed conn_id={conn_id} err={e}");
            return;
        }
    };
    if msg.v != WIRE_VERSION {
        logln!(
            "pipe: version mismatch v={} (expected {WIRE_VERSION}) conn_id={conn_id} — ignored",
            msg.v
        );
        return;
    }
    if let Ok(mut q) = queue.lock() {
        q.push_back(Incoming::Msg {
            conn_id,
            conn_handle,
            msg: Box::new(msg),
        });
    }
    wake_ui(hwnd_raw);
}

/// 역방향 송신(렌더러→TSF, §11.C/E) — owner 연결 핸들로 JSON line+`\n` WriteFile.
/// fire-and-forget. 실패는 로그만(타이핑·표시 무영향). `conn_handle` = `Incoming::Msg`
/// 에서 받은 raw isize. `WriteFile` 은 reader 의 `ReadFile` 과 방향이 반대라 동시 안전.
pub fn send_reverse(conn_handle: isize, msg: &WireMsg) {
    if conn_handle == 0 {
        logln!("send_reverse: no owner conn handle — dropped evt={:?}", msg.evt);
        return;
    }
    let line = match serde_json::to_string(msg) {
        Ok(mut s) => {
            s.push('\n');
            s
        }
        Err(e) => {
            logln!("send_reverse: serialize failed ({e})");
            return;
        }
    };
    let handle = HANDLE(conn_handle as *mut _);
    let bytes = line.as_bytes();
    let mut written: u32 = 0;
    let ok = unsafe {
        windows::Win32::Storage::FileSystem::WriteFile(handle, Some(bytes), Some(&mut written), None)
    }
    .is_ok();
    if ok {
        logln!(
            "send_reverse: wrote {written} bytes evt={:?} seq={} owner_hwnd={:?}",
            msg.evt, msg.seq, msg.owner_hwnd
        );
    } else {
        logln!("send_reverse: WriteFile failed err={:?} evt={:?}", unsafe { GetLastError() }, msg.evt);
    }
}

fn wake_ui(hwnd_raw: isize) {
    unsafe {
        let _ = windows::Win32::UI::WindowsAndMessaging::PostMessageW(
            Some(HWND(hwnd_raw as *mut _)),
            WM_APP_CMD,
            WPARAM(0),
            LPARAM(0),
        );
    }
}
