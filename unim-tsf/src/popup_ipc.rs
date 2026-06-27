//! 팝업 IPC 클라이언트 — out-of-process 렌더러(`unim-popup-win.exe`)로 명명 파이프 전송.
//!
//! in-proc HWND 팝업(구 `popup_window.rs`)을 폐기하고, 한자/특수문자/이모지 후보
//! 렌더링을 별도 프로세스로 분리한다 (Linux `unim-popup-service` 와 대칭, Mozc/Weasel 노선).
//! 본 모듈은 **클라이언트** 측 — 엔진 SoT `PopupViewModel` 을 와이어 타입으로
//! 평탄화해 fire-and-forget 으로 파이프에 쓴다.
//!
//! 근거 설계서: `docs/dev/windows/popup-renderer-design.md` §3·§4·§6 (동결 스펙).
//!
//! ## 절대 불변식 — 비차단
//! IME(OnKeyDown/OnSetFocus) 스레드에서는 `sync_channel(8)::try_send` 만 한다.
//! 파이프 `CreateFileW`/`WaitNamedPipeW`/`CreateProcessW` 는 **전부 worker 스레드**
//! 에서만 호출한다. 어떤 실패도 panic 금지 — `dbg_log` 후 무시한다.
//! **팝업이 안 떠도 타이핑·조합은 절대 영향받지 않는다.**

#![allow(dead_code)] // 일부 와이어 필드/상수는 렌더러 쪽 계약용 (사본 드리프트 방지).

use std::sync::mpsc::{sync_channel, SyncSender, TrySendError};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use serde::{Deserialize, Serialize};

use crate::register::dbg_log;

// ════════════════════════════════════════════════════════════════════════════
// §3.3 IPC wire 타입 (동결 — unim-popup-win/src/protocol.rs 와 동일 사본)
//
// 필드 추가는 반드시 #[serde(default)] (하위호환) + 설계서 갱신 + WIRE_VERSION 유지.
// JSON 키 이름·의미를 임의 변경 금지 (설계서 §3.5).
// ════════════════════════════════════════════════════════════════════════════

pub const WIRE_VERSION: u32 = 1;
/// 파이프 베이스 이름. 실제 이름은 `<base>.<session_id>` 형태.
pub const PIPE_BASE_NAME: &str = r"\\.\pipe\unim-popup-win";
pub const PIPE_SDDL: &str = "D:(A;;GRGW;;;WD)(A;;GRGW;;;AC)S:(ML;;NW;;;LW)";
pub const MAX_LINE_BYTES: usize = 1024 * 1024;
pub const FLASH_MS: u32 = 140;

/// 셀 플래그 — `unim_popup_types::popup_render_flags` 와 비트 동일.
pub mod cell_flags {
    pub const HAS_DATA: u32 = 0x01;
    pub const SELECTED: u32 = 0x02;
    pub const COL_HIGHLIGHT: u32 = 0x04;
    pub const ROW_HIGHLIGHT: u32 = 0x08;
    pub const BOOKMARKED: u32 = 0x10;
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WireCell {
    pub t: String, // text
    pub m: String, // meaning (한자 외 "")
    pub f: u32,    // cell_flags 비트합
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RenderState {
    pub kind: u32,
    pub target: String,
    pub header_text: String,
    pub footer_text: String,
    pub show_footer: bool,
    pub rows: u32,
    pub cols: u32,
    pub sel_row: u32,
    pub sel_col: u32,
    pub current_page: u32,
    pub total_pages: u32,
    /// column-major: cells[col * rows + row], len == rows*cols
    pub cells: Vec<WireCell>,
    pub col_headers: Vec<(String, bool)>,
    pub row_headers: Vec<(String, bool)>,
    pub expand_visible: bool,
    pub expand_text: String,
    pub tab_labels: Vec<String>,
    pub active_tab_index: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WireMsg {
    pub v: u32,
    /// 정방향(클→렌더러): "render" | "hide" | "ping" | "shutdown".
    /// 역방향(렌더러→클): "evt".
    pub cmd: String,
    pub pid: u32,
    pub seq: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub first: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub flash: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub owner_hwnd: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub render: Option<RenderState>,
    // ── §11.D 역방향(렌더러 → TSF) 필드 (cmd="evt"). 전부 옵셔널 + skip_serializing_if
    //    → 정방향 직렬화 골든 라인 바이트 불변. 필드 순서(직렬화 키 순서)는 동결:
    //    v,cmd,pid,seq,first,flash,owner_hwnd,render,evt,row,col,dir,index ──
    /// 역방향 서브타입: cell_click|page_click|tab_click|expand_toggle|outside_cancel.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub evt: Option<String>,
    /// 격자 셀 행 (0-based). 한자 compact 는 col 항상 0.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub row: Option<u32>,
    /// 격자 셀 열 (0-based).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub col: Option<u32>,
    /// 페이지 방향: 0=Prev, 1=Next.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dir: Option<u32>,
    /// 이모지 카테고리 탭 인덱스 (0..8).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub index: Option<u32>,
}

/// 역방향 이벤트 서브타입·상수 (양 크레이트 동일 — §11.D 동결).
pub mod evt_kind {
    pub const CELL_CLICK: &str = "cell_click";
    pub const PAGE_CLICK: &str = "page_click";
    pub const TAB_CLICK: &str = "tab_click";
    pub const EXPAND_TOGGLE: &str = "expand_toggle";
    pub const OUTSIDE_CANCEL: &str = "outside_cancel";
    /// page_click.dir 값.
    pub const PAGE_DIR_PREV: u32 = 0;
    pub const PAGE_DIR_NEXT: u32 = 1;
}

// ════════════════════════════════════════════════════════════════════════════
// §11.G 역채널 — 렌더러 클릭 이벤트를 TSF 스레드로 마샬링하는 RevEvent + 큐.
//
// reader 서브스레드(ReadFile 블로킹)가 역방향 WireMsg 라인을 파싱해 RevEvent 로
// 변환 후 RevChannel(공유 큐 + message-only HWND)에 push + PostMessageW 한다.
// 적용은 TSF 스레드 wndproc 에서만 (엔진 락 + edit session 필요).
// ════════════════════════════════════════════════════════════════════════════

/// TSF 스레드로 마샬링되는 역방향 마우스 이벤트.
///
/// `owner_hwnd`+`seq` 로 어느 팝업 인스턴스 대상인지 식별한다 (stale 차단).
#[derive(Debug, Clone)]
pub enum RevEvent {
    /// 격자/리스트 셀 클릭 → 선택 확정.
    CellClick { row: u32, col: u32 },
    /// 페이지 ◀/▶ 클릭. dir: 0=Prev, 1=Next.
    PageClick { dir: u32 },
    /// 이모지 카테고리 탭 클릭.
    TabClick { index: u32 },
    /// 한자 확장/축소 토글 (⊞/⊟).
    ExpandToggle,
    /// 팝업 밖 클릭 → 취소.
    OutsideCancel,
}

/// 마샬링된 역이벤트 1건 (식별 메타데이터 포함).
#[derive(Debug, Clone)]
pub struct RevEnvelope {
    /// 렌더러가 echo 한 대상 호스트 HWND.
    pub owner_hwnd: u64,
    /// 렌더러가 echo 한 마지막 수신 render seq.
    pub seq: u64,
    pub event: RevEvent,
}

/// reader 서브스레드 → TSF 스레드 마샬링 채널.
///
/// reader 가 `queue` 에 push 후 `notify_hwnd`(message-only HWND)로 `WM_UNIM_REV`
/// PostMessage. wndproc 이 queue 를 drain 해 엔진에 적용한다.
pub struct RevChannel {
    pub queue: std::sync::Mutex<std::collections::VecDeque<RevEnvelope>>,
    /// TSF 스레드의 message-only HWND (PostMessage 대상). raw isize(0=미설정).
    notify_hwnd: std::sync::atomic::AtomicIsize,
}

/// `WM_UNIM_REV` — message-only HWND 로 보내는 "역이벤트 도착" wake 메시지.
/// WM_APP(0x8000) + 0x21 — 다른 커스텀 메시지와 충돌 회피.
pub const WM_UNIM_REV: u32 = 0x8000 + 0x21;

impl RevChannel {
    fn new() -> Self {
        Self {
            queue: std::sync::Mutex::new(std::collections::VecDeque::new()),
            notify_hwnd: std::sync::atomic::AtomicIsize::new(0),
        }
    }

    /// TSF 스레드가 만든 message-only HWND 를 등록(ActivateEx). 0 이면 해제(Deactivate).
    pub fn set_notify_hwnd(&self, hwnd: isize) {
        self.notify_hwnd.store(hwnd, Ordering::SeqCst);
    }

    /// reader 서브스레드에서 호출: 큐에 push 후 wndproc 깨우기.
    fn push(&self, env: RevEnvelope) {
        {
            let mut q = self.queue.lock().unwrap();
            // 폭주 방지: 64개 초과면 가장 오래된 것 drop.
            while q.len() >= 64 {
                q.pop_front();
            }
            q.push_back(env);
        }
        let hwnd = self.notify_hwnd.load(Ordering::SeqCst);
        if hwnd != 0 {
            unsafe {
                use windows::Win32::Foundation::{HWND, LPARAM, WPARAM};
                use windows::Win32::UI::WindowsAndMessaging::PostMessageW;
                let _ = PostMessageW(
                    Some(HWND(hwnd as *mut core::ffi::c_void)),
                    WM_UNIM_REV,
                    WPARAM(0),
                    LPARAM(0),
                );
            }
        }
    }

    /// wndproc 에서 호출: 큐의 모든 역이벤트를 꺼낸다 (FIFO).
    pub fn drain(&self) -> Vec<RevEnvelope> {
        let mut q = self.queue.lock().unwrap();
        q.drain(..).collect()
    }
}

// ════════════════════════════════════════════════════════════════════════════
// §4 PopupViewModel → RenderState 변환 (동결 — column-major 평탄화)
// ════════════════════════════════════════════════════════════════════════════

/// 엔진 SoT view model 을 와이어 `RenderState` 로 평탄화한다.
///
/// `cells` 는 **column-major** (`cells[col * rows + row]`). 좌표/치수는 일절
/// 전송하지 않는다 — 위치는 렌더러가 화면 중앙 배치로 전담.
///
/// 이 함수가 H3(전치)/H4(하이라이트)/H5(첫표시 격자)/H9(헤더·뜻)/H14(초기★) 일괄
/// 해소 지점이다 — rows/cols/total_pages·헤더·뜻·탭·초기★ 가 전부 엔진 SoT 산출.
pub fn to_render_state(vm: &unim::popup::PopupViewModel) -> RenderState {
    use cell_flags::*;
    use unim::popup::PopupKind;

    let rows = vm.cells.len();
    let cols = vm.cells.first().map_or(0, |r| r.len());
    let mut cells = Vec::with_capacity(rows * cols);
    // column-major: 바깥 루프 = 열, 안쪽 = 행.
    for c in 0..cols {
        for r in 0..rows {
            match vm.cells[r][c].as_ref() {
                Some(cd) => {
                    let mut f = HAS_DATA;
                    if cd.is_selected {
                        f |= SELECTED;
                    }
                    if cd.is_col_highlight {
                        f |= COL_HIGHLIGHT;
                    }
                    if cd.is_row_highlight {
                        f |= ROW_HIGHLIGHT;
                    }
                    if cd.is_bookmarked {
                        f |= BOOKMARKED;
                    }
                    cells.push(WireCell {
                        t: cd.text.clone(),
                        m: cd.meaning.clone().unwrap_or_default(),
                        f,
                    });
                }
                None => cells.push(WireCell {
                    t: String::new(),
                    m: String::new(),
                    f: 0,
                }),
            }
        }
    }

    // PopupKind: enum repr 미지정이므로 match 로 명시 변환 (POPUP_SPEC §2.5).
    let kind: u32 = match vm.kind {
        PopupKind::Hanja => 0,
        PopupKind::SpecialChar => 1,
        PopupKind::Emoji => 2,
    };

    RenderState {
        kind,
        target: vm.target.clone(),
        header_text: vm.header_text.clone(),
        footer_text: vm.footer_text.clone(),
        show_footer: vm.show_footer,
        rows: rows as u32,
        cols: cols as u32,
        sel_row: vm.sel_row as u32,
        sel_col: vm.sel_col as u32,
        current_page: vm.current_page as u32,
        total_pages: vm.total_pages as u32,
        cells,
        col_headers: vm
            .col_headers
            .iter()
            .cloned()
            .zip(vm.col_header_active.iter().copied())
            .collect(),
        row_headers: vm
            .row_headers
            .iter()
            .cloned()
            .zip(vm.row_header_active.iter().copied())
            .collect(),
        expand_visible: vm.expand_visible,
        expand_text: vm.expand_text.clone(),
        tab_labels: vm.tab_labels.clone(),
        active_tab_index: vm.active_tab_index as u32,
    }
}

// ════════════════════════════════════════════════════════════════════════════
// §6.2 PopupClient — text_service/key_handler 가 보는 표면
// ════════════════════════════════════════════════════════════════════════════

/// worker 스레드로 보내는 명령. IME 스레드는 try_send 로만 enqueue.
enum Cmd {
    Render { msg: WireMsg },
    Hide { msg: WireMsg },
}

/// 팝업 IPC 클라이언트. worker 스레드를 소유하며 비차단 큐로 통신한다.
pub struct PopupClient {
    tx: SyncSender<Cmd>,
    /// 팝업 표시 중 여부 (로컬 미러 — test_key_down 의 popup_active 게이트용).
    active: Arc<AtomicBool>,
    /// 클라이언트별 단조 증가 시퀀스.
    seq: u64,
    /// 역채널 마샬링 큐 (worker reader 서브스레드 → TSF 스레드 wndproc).
    /// TextService 가 message-only HWND 를 등록하고 wndproc 에서 drain 한다.
    rev: Arc<RevChannel>,
    /// 마지막 send_render 의 owner_hwnd (역이벤트 stale 판정용).
    last_owner_hwnd: u64,
    /// 마지막 send_render 의 seq (역이벤트 stale 판정용).
    last_render_seq: u64,
}

impl PopupClient {
    /// worker 스레드 기동. 파이프 연결은 lazy (첫 send 시).
    pub fn new() -> Self {
        let (tx, rx) = sync_channel::<Cmd>(8);
        let active = Arc::new(AtomicBool::new(false));
        let active_worker = Arc::clone(&active);
        let rev = Arc::new(RevChannel::new());
        let rev_worker = Arc::clone(&rev);
        let pid = std::process::id();

        std::thread::Builder::new()
            .name("unim-popup-ipc".to_string())
            .spawn(move || worker_loop(rx, active_worker, rev_worker))
            .ok(); // 스레드 기동 실패도 무시 — IME 영향 0.

        dbg_log(&format!("popup_ipc: PopupClient created pid={pid}"));
        Self {
            tx,
            active,
            seq: 0,
            rev,
            last_owner_hwnd: 0,
            last_render_seq: 0,
        }
    }

    /// 역채널 핸들 (TextService 가 message-only HWND 등록·큐 drain 에 사용).
    pub fn rev_channel(&self) -> Arc<RevChannel> {
        Arc::clone(&self.rev)
    }

    /// 마지막 send_render 의 (owner_hwnd, seq) — 역이벤트 stale 판정용.
    pub fn last_render_ident(&self) -> (u64, u64) {
        (self.last_owner_hwnd, self.last_render_seq)
    }

    /// 팝업 표시 중 여부 (로컬 미러).
    pub fn is_active(&self) -> bool {
        self.active.load(Ordering::SeqCst)
    }

    /// Show*/갱신 — view model 전송. `first`=Show* 액션 직후 여부, `flash`=★해제 신호.
    pub fn send_render(&mut self, rs: RenderState, first: bool, flash: bool) {
        self.seq += 1;
        let owner_hwnd = current_foreground_hwnd();
        // 역이벤트 stale 판정용으로 기록 (이 render 의 seq/owner 와 매칭하는 evt 만 적용).
        self.last_owner_hwnd = owner_hwnd;
        self.last_render_seq = self.seq;
        let kind = rs.kind;
        let rows = rs.rows;
        let cols = rs.cols;
        let cur = rs.current_page;
        let total = rs.total_pages;
        let sel = (rs.sel_row, rs.sel_col);
        let ncells = rs.cells.len();
        let msg = WireMsg {
            v: WIRE_VERSION,
            cmd: "render".to_string(),
            pid: std::process::id(),
            seq: self.seq,
            first: Some(first),
            flash: Some(flash),
            owner_hwnd: Some(owner_hwnd),
            render: Some(rs),
            // 역방향 필드 — 정방향 메시지는 항상 None (직렬화 생략).
            evt: None,
            row: None,
            col: None,
            dir: None,
            index: None,
        };
        // 전이 전 상태도 함께 로깅.
        let was_active = self.active.swap(true, Ordering::SeqCst);
        dbg_log(&format!(
            "popup_ipc: send render seq={} first={} flash={} kind={} {}x{} page {}/{} sel=({},{}) cells={} owner_hwnd={:#x} active:{}→true",
            self.seq, first, flash, kind, rows, cols, cur, total, sel.0, sel.1, ncells, owner_hwnd, was_active
        ));
        match self.tx.try_send(Cmd::Render { msg }) {
            Ok(()) => {}
            Err(TrySendError::Full(_)) => {
                dbg_log(&format!(
                    "popup_ipc: queue full, dropped cmd=render seq={}",
                    self.seq
                ));
            }
            Err(TrySendError::Disconnected(_)) => {
                dbg_log(&format!(
                    "popup_ipc: worker disconnected, dropped cmd=render seq={}",
                    self.seq
                ));
            }
        }
    }

    /// HidePopup/포커스 전환/리로드 — Hide 전송 + active=false. 이미 비활성이면 no-op.
    pub fn hide(&mut self) {
        // 이미 비활성이면 no-op (불필요한 stale hide 송신 억제).
        if !self.active.swap(false, Ordering::SeqCst) {
            return;
        }
        self.seq += 1;
        let msg = WireMsg {
            v: WIRE_VERSION,
            cmd: "hide".to_string(),
            pid: std::process::id(),
            seq: self.seq,
            first: None,
            flash: None,
            owner_hwnd: None,
            render: None,
            evt: None,
            row: None,
            col: None,
            dir: None,
            index: None,
        };
        dbg_log(&format!(
            "popup_ipc: send hide seq={} active:true→false",
            self.seq
        ));
        match self.tx.try_send(Cmd::Hide { msg }) {
            Ok(()) => {}
            Err(TrySendError::Full(_)) => {
                dbg_log(&format!(
                    "popup_ipc: queue full, dropped cmd=hide seq={}",
                    self.seq
                ));
            }
            Err(TrySendError::Disconnected(_)) => {
                dbg_log(&format!(
                    "popup_ipc: worker disconnected, dropped cmd=hide seq={}",
                    self.seq
                ));
            }
        }
    }
}

impl Default for PopupClient {
    fn default() -> Self {
        Self::new()
    }
}

/// 호스트 포그라운드 창 HWND (모니터 선정용). 비차단·즉시. 실패 시 0.
fn current_foreground_hwnd() -> u64 {
    unsafe {
        use windows::Win32::UI::WindowsAndMessaging::GetForegroundWindow;
        GetForegroundWindow().0 as u64
    }
}

// ════════════════════════════════════════════════════════════════════════════
// §6.3 worker 스레드 — 연결·전송 상태기계 (모든 파이프/프로세스 API 는 여기서만)
// ════════════════════════════════════════════════════════════════════════════

mod worker {
    use super::*;
    use std::sync::mpsc::Receiver;
    use std::time::Instant;

    use windows::core::PCWSTR;
    use windows::Win32::Foundation::{
        CloseHandle, GetLastError, ERROR_FILE_NOT_FOUND, ERROR_IO_PENDING, ERROR_PIPE_BUSY,
        GENERIC_READ, GENERIC_WRITE, HANDLE,
    };
    use windows::Win32::Storage::FileSystem::{
        CreateFileW, ReadFile, WriteFile, FILE_FLAG_OVERLAPPED, FILE_SHARE_NONE, OPEN_EXISTING,
    };
    use windows::Win32::System::Pipes::WaitNamedPipeW;
    use windows::Win32::System::RemoteDesktop::ProcessIdToSessionId;
    use windows::Win32::System::IO::{GetOverlappedResult, OVERLAPPED};
    use windows::Win32::System::Threading::{CreateEventW, WaitForSingleObject, INFINITE};

    // ════════════════════════════════════════════════════════════════════════
    // B4-2 OVERLAPPED I/O 헬퍼 — 같은 DUPLEX 핸들에서 read/write 직렬화 데드락 회피.
    //
    // 동기 핸들은 OS 가 한 핸들의 모든 I/O 를 직렬화한다. reader 서브스레드의 블로킹
    // ReadFile 진행 중이면 worker 의 WriteFile(render) 이 그 read 완료까지 막혀 팝업이
    // 다시 뜨지 않는다. FILE_FLAG_OVERLAPPED 로 열고 각 I/O 를 자체 이벤트가 달린
    // OVERLAPPED 로 수행하면 read 와 write 가 동시 진행되어 서로를 막지 않는다.
    //
    // panic=abort 이므로 어떤 실패도 Err 로 환원해 호출부가 dbg_log 후 graceful 처리.
    // ════════════════════════════════════════════════════════════════════════

    /// 자동 리셋 이벤트를 소유. Drop 시 이벤트 핸들을 닫는다. I/O 호출마다 1개 생성
    /// (저빈도 IPC라 오버헤드 무시 가능).
    struct OvlEvent {
        event: HANDLE,
    }

    impl OvlEvent {
        /// 자동 리셋·초기 비신호 이벤트 생성. 실패 시 None.
        fn new() -> Option<Self> {
            let event = unsafe { CreateEventW(None, false, false, PCWSTR::null()) };
            match event {
                Ok(h) if !h.is_invalid() => Some(Self { event: h }),
                _ => {
                    dbg_log(&format!(
                        "popup_ipc: CreateEventW failed GetLastError={}",
                        unsafe { GetLastError() }.0
                    ));
                    None
                }
            }
        }
    }

    impl Drop for OvlEvent {
        fn drop(&mut self) {
            unsafe {
                let _ = CloseHandle(self.event);
            }
        }
    }

    /// overlapped WriteFile 을 동기처럼 수행. 전송 바이트 수 반환. 실패 시 Err.
    fn overlapped_write(handle: HANDLE, bytes: &[u8]) -> Result<u32, ()> {
        let ovl_ev = OvlEvent::new().ok_or(())?;
        let mut ovl = OVERLAPPED::default();
        ovl.hEvent = ovl_ev.event;
        let mut written: u32 = 0;
        let r = unsafe { WriteFile(handle, Some(bytes), Some(&mut written), Some(&mut ovl)) };
        if r.is_err() {
            let e = unsafe { GetLastError() };
            if e != ERROR_IO_PENDING {
                return Err(());
            }
            unsafe {
                WaitForSingleObject(ovl_ev.event, INFINITE);
            }
            let mut transferred: u32 = 0;
            let gr = unsafe { GetOverlappedResult(handle, &ovl, &mut transferred, false) };
            if gr.is_err() {
                return Err(());
            }
            written = transferred;
        }
        Ok(written)
    }

    /// overlapped ReadFile 을 동기처럼 수행. 읽은 바이트 수 반환. 0 바이트/실패는 Err.
    fn overlapped_read(handle: HANDLE, buf: &mut [u8]) -> Result<u32, ()> {
        let ovl_ev = OvlEvent::new().ok_or(())?;
        let mut ovl = OVERLAPPED::default();
        ovl.hEvent = ovl_ev.event;
        let mut read: u32 = 0;
        let r = unsafe { ReadFile(handle, Some(buf), Some(&mut read), Some(&mut ovl)) };
        if r.is_err() {
            let e = unsafe { GetLastError() };
            if e != ERROR_IO_PENDING {
                return Err(());
            }
            unsafe {
                WaitForSingleObject(ovl_ev.event, INFINITE);
            }
            let mut transferred: u32 = 0;
            let gr = unsafe { GetOverlappedResult(handle, &ovl, &mut transferred, false) };
            if gr.is_err() {
                return Err(());
            }
            read = transferred;
        }
        if read == 0 {
            return Err(());
        }
        Ok(read)
    }

    /// worker 의 영속 연결 상태.
    struct PipeConn {
        handle: Option<HANDLE>,
        pipe_name_w: Vec<u16>,
        /// 마지막 render WireMsg 캐시 — 재연결 성공 시 active 면 재전송.
        last_render: Option<WireMsg>,
        /// 마지막 spawn 시도 시각 (5초 rate-limit).
        last_spawn: Option<Instant>,
        /// 역채널 마샬링 큐 (reader 서브스레드가 push).
        rev: Arc<RevChannel>,
        /// 현재 연결에 대한 reader 서브스레드 세대 토큰. close 시 증가해 옛 reader
        /// 가 push 못 하도록 무효화한다 (핸들 재사용 race 방지).
        reader_gen: Arc<std::sync::atomic::AtomicU64>,
    }

    impl PipeConn {
        fn new(rev: Arc<RevChannel>) -> Self {
            let name = pipe_name();
            dbg_log(&format!("popup_ipc: worker pipe target = {name}"));
            let pipe_name_w: Vec<u16> = name.encode_utf16().chain(std::iter::once(0)).collect();
            Self {
                handle: None,
                pipe_name_w,
                last_render: None,
                last_spawn: None,
                rev,
                reader_gen: Arc::new(std::sync::atomic::AtomicU64::new(0)),
            }
        }

        fn close(&mut self) {
            if let Some(h) = self.handle.take() {
                // reader 세대 무효화 → 옛 reader 의 push/계속 read 차단.
                self.reader_gen.fetch_add(1, Ordering::SeqCst);
                unsafe {
                    // CloseHandle 이 reader 의 블로킹 ReadFile 을 에러로 깨워 종료시킨다.
                    let _ = CloseHandle(h);
                }
                dbg_log("popup_ipc: pipe handle closed");
            }
        }

        /// 파이프 핸들을 보장한다. 없으면 연결 시도(+필요 시 spawn). 실패 시 false.
        fn ensure(&mut self) -> bool {
            if self.handle.is_some() {
                return true;
            }
            // 1차 연결.
            if self.try_connect() {
                self.on_connected();
                return true;
            }
            // 연결 불가 → spawn 폴백(rate-limit) 후 대기·재시도 1회.
            super::spawn::maybe_spawn_renderer(&mut self.last_spawn);
            unsafe {
                // 300ms 까지 서버 인스턴스 가용 대기.
                let _ = WaitNamedPipeW(PCWSTR(self.pipe_name_w.as_ptr()), 300);
            }
            if self.try_connect() {
                self.on_connected();
                return true;
            }
            dbg_log("popup_ipc: ensure_pipe failed (renderer unreachable) — dropping");
            false
        }

        fn try_connect(&mut self) -> bool {
            unsafe {
                // 파이프는 이미 PIPE_ACCESS_DUPLEX → READ|WRITE 로 열어 역채널 ReadFile
                // 을 같은 연결에서 수행한다 (§11.G).
                match CreateFileW(
                    PCWSTR(self.pipe_name_w.as_ptr()),
                    GENERIC_READ.0 | GENERIC_WRITE.0,
                    FILE_SHARE_NONE,
                    None,
                    OPEN_EXISTING,
                    // B4-2: FILE_FLAG_OVERLAPPED 필수 — 같은 DUPLEX 핸들에서 worker 의
                    // WriteFile 과 reader 의 ReadFile 이 직렬화되어 데드락하는 것을 막는다.
                    // 동기 핸들이면 reader 의 블로킹 ReadFile 진행 중 do_write 의 WriteFile
                    // 이 read 완료까지 막혀 render 가 나가지 못해 팝업 재오픈 불가.
                    FILE_FLAG_OVERLAPPED,
                    None,
                ) {
                    Ok(h) => {
                        self.handle = Some(h);
                        dbg_log("popup_ipc: connect success (duplex read|write)");
                        // 이 연결 전용 reader 서브스레드 기동 (역채널 ReadFile 블로킹).
                        self.spawn_reader(h);
                        true
                    }
                    Err(_) => {
                        let err = GetLastError();
                        let reason = if err == ERROR_FILE_NOT_FOUND {
                            "ERROR_FILE_NOT_FOUND (no server)"
                        } else if err == ERROR_PIPE_BUSY {
                            "ERROR_PIPE_BUSY"
                        } else {
                            "other"
                        };
                        dbg_log(&format!(
                            "popup_ipc: connect failed GetLastError={} ({reason})",
                            err.0
                        ));
                        false
                    }
                }
            }
        }

        /// 현재 연결 핸들에서 역방향 라인을 읽는 reader 서브스레드를 기동한다.
        ///
        /// 같은 DUPLEX 핸들에 worker 가 WriteFile, reader 가 ReadFile (방향 분리라 안전).
        /// `close()` 가 핸들을 CloseHandle 하면 ReadFile 이 에러로 풀려 reader 가 종료.
        /// `reader_gen` 으로 옛 reader 의 stale push 를 차단한다.
        fn spawn_reader(&self, handle: HANDLE) {
            let rev = Arc::clone(&self.rev);
            let gen_arc = Arc::clone(&self.reader_gen);
            let my_gen = gen_arc.load(Ordering::SeqCst);
            // HANDLE 은 Send 아님 → raw isize 로 넘긴다.
            let raw: isize = handle.0 as isize;
            let spawn_res = std::thread::Builder::new()
                .name("unim-popup-rev".to_string())
                .spawn(move || {
                    reader_loop(raw, rev, gen_arc, my_gen);
                });
            if spawn_res.is_err() {
                dbg_log("popup_ipc: reader thread spawn failed (reverse channel disabled)");
            }
        }

        /// 연결 직후 캐시 재전송 (렌더러 재시작/크래시 복원).
        fn on_connected(&mut self) {
            if let Some(msg) = self.last_render.clone() {
                dbg_log(&format!(
                    "popup_ipc: reconnect → resend cached render seq={}",
                    msg.seq
                ));
                self.write_msg(&msg);
            }
        }

        /// JSON + "\n" 쓰기. broken pipe 시 1회 재연결·재시도.
        fn write_msg(&mut self, msg: &WireMsg) {
            if !self.ensure() {
                return;
            }
            let mut line = match serde_json::to_string(msg) {
                Ok(s) => s,
                Err(e) => {
                    dbg_log(&format!("popup_ipc: serialize failed: {e}"));
                    return;
                }
            };
            line.push('\n');
            let bytes = line.as_bytes();
            if bytes.len() > MAX_LINE_BYTES {
                dbg_log(&format!(
                    "popup_ipc: line too large ({} > {}), dropping",
                    bytes.len(),
                    MAX_LINE_BYTES
                ));
                return;
            }

            if self.do_write(bytes) {
                return;
            }
            // 1회 재연결 재시도.
            self.close();
            if !self.ensure() {
                return;
            }
            if !self.do_write(bytes) {
                dbg_log("popup_ipc: write retry failed, dropping");
            }
        }

        /// 단일 overlapped WriteFile. 성공 true. 실패 시 close 하고 false.
        /// B4-2: overlapped 라 reader 의 ReadFile 진행 중에도 즉시 시작된다(직렬화 없음).
        fn do_write(&mut self, bytes: &[u8]) -> bool {
            let Some(h) = self.handle else {
                return false;
            };
            match overlapped_write(h, bytes) {
                Ok(written) => {
                    dbg_log(&format!("popup_ipc: wrote {written} bytes"));
                    true
                }
                Err(()) => {
                    let err = unsafe { GetLastError() };
                    dbg_log(&format!(
                        "popup_ipc: WriteFile failed GetLastError={}",
                        err.0
                    ));
                    self.close();
                    false
                }
            }
        }
    }

    /// `<base>.<session_id>` 파이프 이름.
    fn pipe_name() -> String {
        let mut sid: u32 = 0;
        let ok = unsafe { ProcessIdToSessionId(std::process::id(), &mut sid).is_ok() };
        if !ok {
            let err = unsafe { GetLastError() };
            dbg_log(&format!(
                "popup_ipc: ProcessIdToSessionId failed GetLastError={}, sid=0",
                err.0
            ));
        }
        format!("{PIPE_BASE_NAME}.{sid}")
    }

    /// 역채널 reader 서브스레드 본문 — DUPLEX 핸들에서 `\n` 종결 JSON 라인을 읽어
    /// `RevEnvelope` 로 마샬링한다 (렌더러 reader_loop 동형).
    ///
    /// `gen_arc`/`my_gen`: 핸들 close 시 worker 가 gen 을 증가시킨다. gen 이 바뀌면
    /// 이 reader 의 핸들은 더 이상 유효하지 않으므로 push 없이 종료한다.
    /// 어떤 실패도 panic 금지 — 로그 후 종료(타이핑 영향 0).
    fn reader_loop(
        raw_handle: isize,
        rev: Arc<RevChannel>,
        gen_arc: Arc<std::sync::atomic::AtomicU64>,
        my_gen: u64,
    ) {
        let handle = HANDLE(raw_handle as *mut core::ffi::c_void);
        dbg_log(&format!("popup_rev: reader started gen={my_gen}"));
        let mut acc: Vec<u8> = Vec::with_capacity(256);
        let mut buf = [0u8; 1024];
        loop {
            // close 로 세대가 바뀌었으면 종료 (핸들 무효화됨).
            if gen_arc.load(Ordering::SeqCst) != my_gen {
                dbg_log("popup_rev: reader gen changed → stop");
                return;
            }
            // B4-2: overlapped read — worker 의 WriteFile(render) 을 막지 않는다.
            let read = match overlapped_read(handle, &mut buf) {
                Ok(n) => n,
                Err(()) => {
                    let err = unsafe { GetLastError() };
                    dbg_log(&format!(
                        "popup_rev: ReadFile end/err GetLastError={} → stop",
                        err.0
                    ));
                    return;
                }
            };
            acc.extend_from_slice(&buf[..read as usize]);
            // 누적 버퍼 폭주 방지 (악성/깨진 스트림).
            if acc.len() > MAX_LINE_BYTES {
                dbg_log("popup_rev: reverse line too large, clearing buffer");
                acc.clear();
            }
            // 완성된 라인(\n 단위)들을 파싱.
            while let Some(pos) = acc.iter().position(|&b| b == b'\n') {
                let line: Vec<u8> = acc.drain(..=pos).collect();
                let line = &line[..line.len() - 1]; // '\n' 제거
                if line.is_empty() {
                    continue;
                }
                // gen 재확인 (긴 처리 중 close 났을 수 있음).
                if gen_arc.load(Ordering::SeqCst) != my_gen {
                    return;
                }
                match std::str::from_utf8(line) {
                    Ok(s) => parse_and_push(s.trim(), &rev),
                    Err(_) => dbg_log("popup_rev: non-utf8 reverse line, skipped"),
                }
            }
        }
    }

    /// 역방향 JSON 라인을 파싱해 RevEnvelope 로 변환 후 채널에 push.
    fn parse_and_push(line: &str, rev: &Arc<RevChannel>) {
        let msg: WireMsg = match serde_json::from_str(line) {
            Ok(m) => m,
            Err(e) => {
                dbg_log(&format!("popup_rev: parse failed: {e}"));
                return;
            }
        };
        if msg.v != WIRE_VERSION {
            dbg_log(&format!("popup_rev: version mismatch v={} → ignore", msg.v));
            return;
        }
        if msg.cmd != "evt" {
            dbg_log(&format!("popup_rev: non-evt cmd='{}' → ignore", msg.cmd));
            return;
        }
        let Some(ref evt) = msg.evt else {
            dbg_log("popup_rev: evt field missing → ignore");
            return;
        };
        let owner_hwnd = msg.owner_hwnd.unwrap_or(0);
        let event = match evt.as_str() {
            evt_kind::CELL_CLICK => RevEvent::CellClick {
                row: msg.row.unwrap_or(0),
                col: msg.col.unwrap_or(0),
            },
            evt_kind::PAGE_CLICK => RevEvent::PageClick {
                dir: msg.dir.unwrap_or(evt_kind::PAGE_DIR_NEXT),
            },
            evt_kind::TAB_CLICK => RevEvent::TabClick {
                index: msg.index.unwrap_or(0),
            },
            evt_kind::EXPAND_TOGGLE => RevEvent::ExpandToggle,
            evt_kind::OUTSIDE_CANCEL => RevEvent::OutsideCancel,
            other => {
                dbg_log(&format!("popup_rev: unknown evt='{other}' → ignore"));
                return;
            }
        };
        dbg_log(&format!(
            "popup_rev: recv evt='{}' owner_hwnd={:#x} seq={} → marshal",
            evt, owner_hwnd, msg.seq
        ));
        rev.push(RevEnvelope {
            owner_hwnd,
            seq: msg.seq,
            event,
        });
    }

    /// worker 메인 루프. 채널이 닫히면 종료.
    pub fn run(rx: Receiver<Cmd>, active: Arc<AtomicBool>, rev: Arc<RevChannel>) {
        dbg_log("popup_ipc: worker started");
        let mut conn = PipeConn::new(rev);
        for cmd in rx.iter() {
            match cmd {
                Cmd::Render { msg } => {
                    conn.last_render = Some(msg.clone());
                    conn.write_msg(&msg);
                }
                Cmd::Hide { msg } => {
                    // hide 후 캐시 비움 — 재연결 시 안 떠 있던 팝업을 부활시키지 않도록.
                    let _ = &active;
                    conn.last_render = None;
                    conn.write_msg(&msg);
                }
            }
        }
        conn.close();
        dbg_log("popup_ipc: worker stopped (channel closed)");
    }
}

// ════════════════════════════════════════════════════════════════════════════
// §6.4 on-demand spawn 폴백 (worker 스레드 전용)
// ════════════════════════════════════════════════════════════════════════════

mod spawn {
    use super::*;
    use std::time::{Duration, Instant};

    use windows::core::{HSTRING, PCWSTR, PWSTR};
    use windows::Win32::Foundation::{CloseHandle, GetLastError, HANDLE};
    use windows::Win32::Security::{GetTokenInformation, TokenIsAppContainer, TOKEN_QUERY};
    use windows::Win32::System::Registry::{
        RegGetValueW, HKEY_LOCAL_MACHINE, RRF_RT_REG_SZ,
    };
    use windows::Win32::System::Threading::{
        CreateProcessW, GetCurrentProcess, OpenProcessToken, CREATE_NO_WINDOW,
        DETACHED_PROCESS, PROCESS_INFORMATION, STARTUPINFOW,
    };

    const RENDERER_EXE: &str = "unim-popup-win.exe";
    const SPAWN_RATE_LIMIT: Duration = Duration::from_secs(5);

    /// 렌더러 spawn 시도(폴백). rate-limit·AppContainer 가드 적용.
    pub fn maybe_spawn_renderer(last_spawn: &mut Option<Instant>) {
        // (1) AppContainer 면 spawn 불가 환경 — 생략, Run 키 인스턴스에 의존.
        if is_app_container() {
            dbg_log("popup_ipc: AppContainer detected, spawn skipped (rely on Run-key instance)");
            return;
        }
        // (4) rate-limit: 마지막 시도 후 5초 이내 재시도 금지.
        if let Some(prev) = *last_spawn {
            if prev.elapsed() < SPAWN_RATE_LIMIT {
                dbg_log("popup_ipc: spawn rate-limited (<5s since last attempt)");
                return;
            }
        }
        *last_spawn = Some(Instant::now());

        // (2) exe 탐색: ① DLL 모듈 디렉터리 ② HKLM InstallDir.
        let exe = match find_renderer_exe() {
            Some(p) => p,
            None => {
                dbg_log("popup_ipc: renderer exe not found (module dir / HKLM InstallDir)");
                return;
            }
        };
        dbg_log(&format!("popup_ipc: spawning renderer: {}", exe));
        spawn_detached(&exe);
    }

    /// 현재 프로세스 토큰이 AppContainer 인지.
    fn is_app_container() -> bool {
        unsafe {
            let mut token = HANDLE::default();
            if OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut token).is_err() {
                dbg_log(&format!(
                    "popup_ipc: OpenProcessToken failed GetLastError={}",
                    GetLastError().0
                ));
                return false;
            }
            let mut is_ac: u32 = 0;
            let mut ret_len: u32 = 0;
            let ok = GetTokenInformation(
                token,
                TokenIsAppContainer,
                Some(&mut is_ac as *mut u32 as *mut core::ffi::c_void),
                std::mem::size_of::<u32>() as u32,
                &mut ret_len,
            )
            .is_ok();
            let _ = CloseHandle(token);
            if !ok {
                dbg_log(&format!(
                    "popup_ipc: GetTokenInformation(TokenIsAppContainer) failed GetLastError={}",
                    GetLastError().0
                ));
                return false;
            }
            is_ac != 0
        }
    }

    /// 렌더러 exe 경로 탐색: ① DLL 모듈 디렉터리 ② HKLM\SOFTWARE\atit.org\UNIM\InstallDir.
    fn find_renderer_exe() -> Option<String> {
        // ① DLL 모듈 경로 기준.
        if let Some(dir) = dll_module_dir() {
            let cand = std::path::Path::new(&dir).join(RENDERER_EXE);
            if cand.exists() {
                return Some(cand.to_string_lossy().into_owned());
            }
            dbg_log(&format!(
                "popup_ipc: renderer not in module dir ({})",
                cand.display()
            ));
        }
        // ② HKLM InstallDir.
        if let Some(dir) = hklm_install_dir() {
            let cand = std::path::Path::new(&dir).join(RENDERER_EXE);
            if cand.exists() {
                return Some(cand.to_string_lossy().into_owned());
            }
            dbg_log(&format!(
                "popup_ipc: renderer not in HKLM InstallDir ({})",
                cand.display()
            ));
        }
        None
    }

    /// DLL 모듈 파일이 위치한 디렉터리.
    fn dll_module_dir() -> Option<String> {
        unsafe {
            let hmodule = crate::dll_instance();
            let mut buf = [0u16; 260];
            let len = windows::Win32::System::LibraryLoader::GetModuleFileNameW(
                Some(hmodule),
                &mut buf,
            );
            if len == 0 {
                dbg_log(&format!(
                    "popup_ipc: GetModuleFileNameW failed GetLastError={}",
                    GetLastError().0
                ));
                return None;
            }
            let path = String::from_utf16_lossy(&buf[..len as usize]);
            std::path::Path::new(&path)
                .parent()
                .map(|p| p.to_string_lossy().into_owned())
        }
    }

    /// HKLM\SOFTWARE\atit.org\UNIM\InstallDir 문자열 값.
    fn hklm_install_dir() -> Option<String> {
        unsafe {
            let subkey: HSTRING = "SOFTWARE\\atit.org\\UNIM".into();
            let value: HSTRING = "InstallDir".into();
            let mut buf = [0u16; 520];
            let mut size: u32 = (buf.len() * std::mem::size_of::<u16>()) as u32;
            let rc = RegGetValueW(
                HKEY_LOCAL_MACHINE,
                PCWSTR(subkey.as_ptr()),
                PCWSTR(value.as_ptr()),
                RRF_RT_REG_SZ,
                None,
                Some(buf.as_mut_ptr() as *mut core::ffi::c_void),
                Some(&mut size),
            );
            if rc.is_err() {
                dbg_log(&format!("popup_ipc: HKLM InstallDir read failed rc={}", rc.0));
                return None;
            }
            // size 는 바이트 수 (NUL 포함). u16 길이로 환산하고 NUL 제거.
            let nchars = (size as usize / std::mem::size_of::<u16>()).saturating_sub(1);
            Some(String::from_utf16_lossy(&buf[..nchars]))
        }
    }

    /// CREATE_NO_WINDOW | DETACHED_PROCESS, 상속 핸들 없음으로 분리 기동.
    fn spawn_detached(exe: &str) {
        unsafe {
            let mut cmdline: Vec<u16> = format!("\"{exe}\"")
                .encode_utf16()
                .chain(std::iter::once(0))
                .collect();
            let mut si = STARTUPINFOW::default();
            si.cb = std::mem::size_of::<STARTUPINFOW>() as u32;
            let mut pi = PROCESS_INFORMATION::default();
            let r = CreateProcessW(
                PCWSTR::null(),
                Some(PWSTR(cmdline.as_mut_ptr())),
                None,
                None,
                false,
                CREATE_NO_WINDOW | DETACHED_PROCESS,
                None,
                PCWSTR::null(),
                &si,
                &mut pi,
            );
            match r {
                Ok(()) => {
                    dbg_log("popup_ipc: CreateProcessW success (renderer spawned)");
                    // 핸들은 즉시 닫음 — 분리 프로세스, 추적 불필요.
                    let _ = CloseHandle(pi.hProcess);
                    let _ = CloseHandle(pi.hThread);
                }
                Err(_) => {
                    dbg_log(&format!(
                        "popup_ipc: CreateProcessW failed GetLastError={}",
                        GetLastError().0
                    ));
                }
            }
        }
    }
}

/// worker 스레드 진입점 — 실제 파이프 IPC 상태기계.
fn worker_loop(
    rx: std::sync::mpsc::Receiver<Cmd>,
    active: Arc<AtomicBool>,
    rev: Arc<RevChannel>,
) {
    worker::run(rx, active, rev);
}

// ════════════════════════════════════════════════════════════════════════════
// §10.1 to_render_state 단위 테스트
// ════════════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;
    use super::cell_flags::*;
    use unim::popup::PopupState;

    /// 평탄화된 column-major cells 에서 (row, col) 셀 조회.
    fn cell_at<'a>(rs: &'a RenderState, row: u32, col: u32) -> &'a WireCell {
        &rs.cells[(col * rs.rows + row) as usize]
    }

    #[test]
    fn special_20_items_column_major() {
        // special 20개 → rows=9, cols=3 (rows 고정 정책).
        let state = PopupState::new_special(
            "ㄱ",
            (0..20).map(|i| format!("S{i}")).collect(),
            "QWERTYUIO",
        );
        let rs = to_render_state(&state.view_model(""));

        assert_eq!(rs.kind, 1, "special kind == 1");
        assert_eq!(rs.rows, 9);
        assert_eq!(rs.cols, 3);
        assert_eq!(rs.cells.len(), 27, "rows*cols = 9*3");

        // column-major: cells[9] = col1 row0 = items[9] = "S9".
        assert_eq!(rs.cells[9].t, "S9");
        assert_eq!(cell_at(&rs, 0, 1).t, "S9");
        // (0,0) 선택 셀.
        assert_eq!(cell_at(&rs, 0, 0).t, "S0");
        assert!(cell_at(&rs, 0, 0).f & HAS_DATA != 0);
        assert!(cell_at(&rs, 0, 0).f & SELECTED != 0);
        // col=2 row=2 이상은 빈 셀 (idx 2*9+2=20 >= 20 → 데이터 없음).
        assert_eq!(cell_at(&rs, 2, 2).f, 0, "empty cell flags == 0");
        assert_eq!(cell_at(&rs, 2, 2).t, "");
        // special meaning 없음.
        assert_eq!(cell_at(&rs, 0, 0).m, "");
    }

    #[test]
    fn hanja_compact_empty_col_headers_and_meaning() {
        // 한자 compact 2후보 → col_headers 빈 배열, meaning 전달.
        let candidates = vec![
            ("韓".to_string(), "나라 한".to_string()),
            ("漢".to_string(), "한나라 한".to_string()),
        ];
        let state = PopupState::new_hanja("한", candidates);
        let rs = to_render_state(&state.view_model(""));

        assert_eq!(rs.kind, 0, "hanja kind == 0");
        // compact: cols=1, col_headers 빈 배열.
        assert_eq!(rs.cols, 1);
        assert!(rs.col_headers.is_empty(), "hanja compact col_headers == []");
        // 2 후보 → rows=2.
        assert_eq!(rs.rows, 2);
        assert_eq!(rs.cells.len(), 2);
        // 첫 셀: 한자 + meaning.
        assert_eq!(cell_at(&rs, 0, 0).t, "韓");
        assert_eq!(cell_at(&rs, 0, 0).m, "나라 한");
        assert!(cell_at(&rs, 0, 0).f & HAS_DATA != 0);
        assert!(cell_at(&rs, 0, 0).f & SELECTED != 0);
        assert_eq!(cell_at(&rs, 1, 0).t, "漢");
        assert_eq!(cell_at(&rs, 1, 0).m, "한나라 한");
    }

    #[test]
    fn emoji_has_nine_tabs() {
        use unim::popup::EmojiCatMeta;
        // 이모지 9 카테고리 → tab_labels 9개.
        let categories: Vec<EmojiCatMeta> = (0..9)
            .map(|i| EmojiCatMeta {
                id: format!("cat{i}"),
                label_ko: format!("탭{i}"),
                label_en: format!("Tab{i}"),
                total: 10,
            })
            .collect();
        let items: Vec<String> = (0..20).map(|i| format!("E{i}")).collect();
        let state = PopupState::new_emoji(0, items, 20, "QWERTYUIO", categories, Vec::new());
        let rs = to_render_state(&state.view_model("ASDFGHJKL"));

        assert_eq!(rs.kind, 2, "emoji kind == 2");
        assert_eq!(rs.tab_labels.len(), 9, "emoji tab_labels == 9");
        assert_eq!(rs.active_tab_index, 0);
    }

    #[test]
    fn golden_json_line_stable() {
        // 골든 JSON 라인 — 사본 드리프트 검출 (양 크레이트 동일 문자열 박기).
        // 작은 결정적 케이스를 직렬화해 키 순서·필드명을 고정한다.
        let candidates = vec![("韓".to_string(), "나라 한".to_string())];
        let state = PopupState::new_hanja("한", candidates);
        let rs = to_render_state(&state.view_model(""));
        let msg = WireMsg {
            v: WIRE_VERSION,
            cmd: "render".to_string(),
            pid: 1234,
            seq: 1,
            first: Some(true),
            flash: Some(false),
            owner_hwnd: Some(0),
            render: Some(rs),
            evt: None,
            row: None,
            col: None,
            dir: None,
            index: None,
        };
        let line = serde_json::to_string(&msg).unwrap();
        // 한 줄(개행 없음) 보장.
        assert!(!line.contains('\n'), "wire line must not contain newline");
        // 핵심 필드 키가 직렬화에 존재.
        for key in [
            "\"v\":1",
            "\"cmd\":\"render\"",
            "\"first\":true",
            "\"flash\":false",
            "\"kind\":0",
            "\"target\":\"한\"",
            "\"韓\"",
            "\"나라 한\"",
        ] {
            assert!(line.contains(key), "golden line missing {key}: {line}");
        }
    }

    /// 사본 드리프트 검출 — unim-popup-win/src/protocol.rs 의
    /// `GOLDEN_RENDER_LINE` 과 **바이트 동일** 문자열을 양 크레이트에 박아,
    /// 한쪽 와이어 타입(필드명·순서·옵셔널 직렬화)이 바뀌면 roundtrip 이
    /// 깨지도록 한다(설계서 §10.1). 이 문자열을 수정할 때는 반드시 양쪽을
    /// 동시에 갱신할 것.
    const GOLDEN_RENDER_LINE: &str = r#"{"v":1,"cmd":"render","pid":4242,"seq":7,"first":true,"flash":false,"owner_hwnd":123456,"render":{"kind":1,"target":"ㄱ","header_text":"「ㄱ」 → 특수문자","footer_text":"1/3","show_footer":true,"rows":9,"cols":3,"sel_row":0,"sel_col":0,"current_page":0,"total_pages":3,"cells":[{"t":"S0","m":"","f":3}],"col_headers":[["Q",false],["W",false]],"row_headers":[["1.",true],["2.",false]],"expand_visible":false,"expand_text":"","tab_labels":[],"active_tab_index":0}}"#;

    #[test]
    fn golden_render_line_byte_equal_with_renderer() {
        // 렌더러(A) 사본과 동일 라인을 우리 와이어 타입으로 파싱→재직렬화하면
        // 바이트 동일이어야 한다(필드명·순서·skip_serializing_if 동기 보장).
        let parsed: WireMsg = serde_json::from_str(GOLDEN_RENDER_LINE)
            .expect("renderer golden line must parse with TSF wire types");
        let reser = serde_json::to_string(&parsed).unwrap();
        assert_eq!(
            reser, GOLDEN_RENDER_LINE,
            "wire serialization drifted from unim-popup-win/src/protocol.rs"
        );
    }

    // ── §11.D 역방향(렌더러 → TSF) 골든 라인 5종 ─────────────────────────────
    //
    // 양 크레이트(unim-tsf / unim-popup-win)에 **동일 문자열**을 박아 사본 드리프트를
    // 검출한다. 직렬화 키 순서 = WireMsg 필드 선언 순서:
    //   v,cmd,pid,seq,first,flash,owner_hwnd,render,evt,row,col,dir,index
    // 렌더러는 pid 를 0 으로 채워 보낸다(역방향은 pid 의미 없음 — 식별은 owner_hwnd+seq).
    const GOLDEN_REV_CELL_CLICK: &str =
        r#"{"v":1,"cmd":"evt","pid":0,"seq":7,"owner_hwnd":123456,"evt":"cell_click","row":2,"col":3}"#;
    const GOLDEN_REV_PAGE_CLICK: &str =
        r#"{"v":1,"cmd":"evt","pid":0,"seq":7,"owner_hwnd":123456,"evt":"page_click","dir":1}"#;
    const GOLDEN_REV_TAB_CLICK: &str =
        r#"{"v":1,"cmd":"evt","pid":0,"seq":7,"owner_hwnd":123456,"evt":"tab_click","index":4}"#;
    const GOLDEN_REV_EXPAND_TOGGLE: &str =
        r#"{"v":1,"cmd":"evt","pid":0,"seq":7,"owner_hwnd":123456,"evt":"expand_toggle"}"#;
    const GOLDEN_REV_OUTSIDE_CANCEL: &str =
        r#"{"v":1,"cmd":"evt","pid":0,"seq":7,"owner_hwnd":123456,"evt":"outside_cancel"}"#;

    /// 역방향 evt WireMsg 빌더 (테스트·검증 전용 — 렌더러 송신 형태 재현).
    fn rev_msg(evt: &str) -> WireMsg {
        WireMsg {
            v: WIRE_VERSION,
            cmd: "evt".to_string(),
            pid: 0,
            seq: 7,
            first: None,
            flash: None,
            owner_hwnd: Some(123456),
            render: None,
            evt: Some(evt.to_string()),
            row: None,
            col: None,
            dir: None,
            index: None,
        }
    }

    #[test]
    fn golden_rev_lines_byte_equal_with_renderer() {
        // 각 역골든 라인: 우리 타입으로 직접 구성 → to_string 이 바이트 동일.
        let mut cell = rev_msg(evt_kind::CELL_CLICK);
        cell.row = Some(2);
        cell.col = Some(3);
        assert_eq!(
            serde_json::to_string(&cell).unwrap(),
            GOLDEN_REV_CELL_CLICK,
            "cell_click reverse line drifted"
        );

        let mut page = rev_msg(evt_kind::PAGE_CLICK);
        page.dir = Some(evt_kind::PAGE_DIR_NEXT);
        assert_eq!(
            serde_json::to_string(&page).unwrap(),
            GOLDEN_REV_PAGE_CLICK,
            "page_click reverse line drifted"
        );

        let mut tab = rev_msg(evt_kind::TAB_CLICK);
        tab.index = Some(4);
        assert_eq!(
            serde_json::to_string(&tab).unwrap(),
            GOLDEN_REV_TAB_CLICK,
            "tab_click reverse line drifted"
        );

        let expand = rev_msg(evt_kind::EXPAND_TOGGLE);
        assert_eq!(
            serde_json::to_string(&expand).unwrap(),
            GOLDEN_REV_EXPAND_TOGGLE,
            "expand_toggle reverse line drifted"
        );

        let cancel = rev_msg(evt_kind::OUTSIDE_CANCEL);
        assert_eq!(
            serde_json::to_string(&cancel).unwrap(),
            GOLDEN_REV_OUTSIDE_CANCEL,
            "outside_cancel reverse line drifted"
        );
    }

    #[test]
    fn golden_rev_lines_roundtrip() {
        // 렌더러(A) 사본 라인을 파싱→재직렬화하면 바이트 동일이어야 한다.
        for line in [
            GOLDEN_REV_CELL_CLICK,
            GOLDEN_REV_PAGE_CLICK,
            GOLDEN_REV_TAB_CLICK,
            GOLDEN_REV_EXPAND_TOGGLE,
            GOLDEN_REV_OUTSIDE_CANCEL,
        ] {
            let parsed: WireMsg = serde_json::from_str(line)
                .unwrap_or_else(|e| panic!("reverse golden line must parse: {line} ({e})"));
            let reser = serde_json::to_string(&parsed).unwrap();
            assert_eq!(reser, line, "reverse wire serialization drifted: {line}");
        }
    }

    #[test]
    fn forward_golden_unaffected_by_reverse_fields() {
        // 정방향 골든 라인은 역방향 필드(전부 None)라 바이트 불변이어야 한다 (머지 게이트).
        let parsed: WireMsg = serde_json::from_str(GOLDEN_RENDER_LINE).unwrap();
        assert!(parsed.evt.is_none());
        assert!(parsed.row.is_none());
        assert!(parsed.col.is_none());
        assert!(parsed.dir.is_none());
        assert!(parsed.index.is_none());
        let reser = serde_json::to_string(&parsed).unwrap();
        assert_eq!(
            reser, GOLDEN_RENDER_LINE,
            "reverse fields must not appear in forward serialization"
        );
    }
}
