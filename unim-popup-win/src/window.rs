//! 팝업 HWND — 프로세스당 1회 생성 후 show/hide 만 (설계서 §5.3).
//!
//! 위치 = 무조건 owner 모니터(rcWork) 정중앙 (§5.5 동결 알고리즘). GetCursorPos 금지.
//! WM_DESTROY 에서 PostQuitMessage 금지 (§8.2 — 호스트/프로세스 오염 방지).

use std::cell::RefCell;
use std::sync::{Arc, Mutex};

use windows::core::Result as WinResult;
use windows::core::{implement, Error, IUnknown, BSTR, PCWSTR};
use windows::Win32::Foundation::{COLORREF, HWND, LPARAM, LRESULT, POINT, RECT, WPARAM};
use windows::Win32::Graphics::Gdi::*;
use windows::Win32::System::Variant::{VARIANT, VT_BSTR, VT_I4};
use windows::Win32::UI::Accessibility::{
    IRawElementProviderSimple, IRawElementProviderSimple_Impl, ProviderOptions,
    ProviderOptions_ServerSideProvider, UiaHostProviderFromHwnd, UiaReturnRawElementProvider,
    UiaRootObjectId, UIA_AutomationIdPropertyId, UIA_ControlTypePropertyId, UIA_ListControlTypeId,
    UIA_NamePropertyId, UIA_PATTERN_ID, UIA_PROPERTY_ID,
};
use windows::Win32::UI::HiDpi::{GetDpiForMonitor, MDT_EFFECTIVE_DPI};
use windows::Win32::UI::WindowsAndMessaging::*;

use crate::logln;
use crate::pipe_server::{try_send_reverse, RevSender};
use crate::protocol::{RenderState, RevEvent, FLASH_MS};
use crate::render;

const CLASS_NAME: PCWSTR = windows::core::w!("UNIM_PopupRendererWnd");
const FLASH_TIMER_ID: usize = 1;

thread_local! {
    /// UI 스레드 전용 — 현재 표시 중 상태. WM_PAINT/WM_DPICHANGED 가 참조한다.
    static UI_STATE: RefCell<UiState> = RefCell::new(UiState::default());
}

#[derive(Default)]
struct UiState {
    hwnd: Option<HWND>,
    last: Option<RenderState>,
    last_owner_hwnd: u64,
    scale: f64,
    visible: bool,
    flash_until: u64, // GetTickCount64 기준 flash 만료 시각 (0 = 비활성)
    // ─── 역방향 송신(§11.E) ───
    owner_conn_handle: isize, // 현재 owner 연결 핸들 raw (0 = 없음). WriteFile 대상.
    last_render_seq: u64,     // 역이벤트 envelope 의 seq echo.
    mouse_hook: isize,        // WH_MOUSE_LL 훅 핸들 raw (0 = 미설치). 표시 중에만 설치.
    // 역방향 비차단 송신 채널(§11.E / B4 freeze 수정). UI 스레드는 이 채널에
    // try_send 만 하고 실제 블로킹 WriteFile 은 writer 스레드가 수행한다.
    rev_tx: Option<RevSender>,
    // ─── I11: UIA(UI Automation) 접근성 제공자 ───
    /// UIA 코어가 읽는 접근성 스냅샷. UI 스레드가 show_render/hide 에서 갱신하고
    /// provider 메서드(다른 스레드 가능)가 lock 해서 읽는다. thread_local 직접
    /// 접근 대신 Arc<Mutex<_>> 로 복제해 크로스스레드 안전 확보(ui_element.rs 원칙).
    a11y_snap: Arc<Mutex<A11ySnapshot>>,
    /// WM_GETOBJECT 최초 처리 시 지연 생성되는 루트 제공자(이후 재사용).
    a11y_provider: Option<IRawElementProviderSimple>,
}

unsafe fn now_ms() -> u64 {
    windows::Win32::System::SystemInformation::GetTickCount64()
}

/// 역방향 비차단 송신 채널 핸들을 UI 스레드 상태에 등록한다 (UI 스레드에서 1회 호출).
/// 이후 모든 역이벤트는 이 채널로 try_send 되어 UI 스레드가 절대 블록되지 않는다(B4).
pub fn set_rev_sender(tx: RevSender) {
    UI_STATE.with(|st| st.borrow_mut().rev_tx = Some(tx));
}

/// 윈도우 클래스 등록 + 숨김 팝업 HWND 1회 생성. UI 스레드에서 호출.
pub fn create() -> windows::core::Result<HWND> {
    unsafe {
        let hinstance = windows::Win32::System::LibraryLoader::GetModuleHandleW(None)?;
        let wc = WNDCLASSEXW {
            cbSize: std::mem::size_of::<WNDCLASSEXW>() as u32,
            style: CS_HREDRAW | CS_VREDRAW,
            lpfnWndProc: Some(wnd_proc),
            cbClsExtra: 0,
            cbWndExtra: 0,
            hInstance: hinstance.into(),
            hIcon: Default::default(),
            hCursor: LoadCursorW(None, IDC_ARROW).unwrap_or_default(),
            hbrBackground: HBRUSH(std::ptr::null_mut()),
            lpszMenuName: PCWSTR::null(),
            lpszClassName: CLASS_NAME,
            hIconSm: Default::default(),
        };
        let atom = RegisterClassExW(&wc);
        if atom == 0 {
            logln!("window: RegisterClassExW failed err={:?}", windows::Win32::Foundation::GetLastError());
        }
        let hwnd = CreateWindowExW(
            WS_EX_NOACTIVATE | WS_EX_TOPMOST | WS_EX_TOOLWINDOW | WS_EX_LAYERED,
            CLASS_NAME,
            windows::core::w!(""),
            WS_POPUP | WS_BORDER,
            0,
            0,
            200,
            100,
            None,
            None,
            Some(hinstance.into()),
            None,
        )?;
        // 불투명도 100% (추후 반투명 여지).
        let _ = SetLayeredWindowAttributes(hwnd, COLORREF(0), 255, LWA_ALPHA);
        UI_STATE.with(|st| {
            let mut st = st.borrow_mut();
            st.hwnd = Some(hwnd);
            st.scale = 1.0;
        });
        logln!("window: created hwnd={:?}", hwnd.0);
        Ok(hwnd)
    }
}

/// render 처리 — 상태 교체 + 중앙 배치 + 표시. flash=true 면 선택 셀 flash 시작.
///
/// SetWindowPos 는 혼합 DPI 모니터 이동 시 WM_DPICHANGED 를 **동기** 송신해
/// wnd_proc 이 UI_STATE 를 재진입 borrow 한다. borrow 를 쥔 채 호출하면
/// BorrowError → extern "system" 경계 abort 이므로, 상태 갱신·배치 계산
/// (compute_placement 는 쿼리 전용 Win32 만 사용)을 마치고 borrow 를 해제한
/// 뒤에 SetWindowPos/InvalidateRect/SetTimer 를 호출한다.
pub fn show_render(rs: RenderState, owner_hwnd: u64, flash: bool, conn_handle: isize, seq: u64) {
    let plan = UI_STATE.with(|st| {
        let mut st = st.borrow_mut();
        let hwnd = st.hwnd?;
        st.last_owner_hwnd = owner_hwnd;
        st.owner_conn_handle = conn_handle;
        st.last_render_seq = seq;
        if flash {
            st.flash_until = unsafe { now_ms() } + FLASH_MS as u64;
        } else {
            st.flash_until = 0;
        }
        st.last = Some(rs);
        st.visible = true;
        // I11: 접근성 스냅샷 갱신(루트 Name). rs 는 last 로 이동했으므로 last 에서 읽는다.
        if let Some(rs_a) = st.last.as_ref() {
            if let Ok(mut snap) = st.a11y_snap.lock() {
                snap.name = a11y_name(rs_a);
                snap.visible = true;
            }
        }
        // 위치·크기 계산 (메시지 디스패치 없는 쿼리만 — borrow 보유 중 안전).
        let rs_ref = st.last.as_ref().unwrap();
        let (x, y, w, h, scale) = unsafe { compute_placement(hwnd, owner_hwnd, rs_ref) };
        st.scale = scale;
        Some((hwnd, x, y, w, h, scale))
    });
    let Some((hwnd, x, y, w, h, scale)) = plan else {
        return;
    };
    unsafe {
        let _ = SetWindowPos(
            hwnd,
            Some(HWND_TOPMOST),
            x,
            y,
            w,
            h,
            SWP_SHOWWINDOW | SWP_NOACTIVATE,
        );
        let _ = InvalidateRect(Some(hwnd), None, true);
        if flash {
            SetTimer(Some(hwnd), FLASH_TIMER_ID, FLASH_MS + 10, None);
        }
    }
    // 외부 클릭 감지용 저수준 마우스 훅 설치 (§11.F — 표시 중에만 상주). idempotent.
    install_mouse_hook();
    logln!(
        "window: show owner_hwnd={owner_hwnd} conn={conn_handle:#x} seq={seq} \
         xy=({x},{y}) wh=({w},{h}) scale={scale:.3} flash={flash}"
    );
}

/// 팝업 숨김 (owner 규칙은 호출자/main 에서 판정).
/// show_render 와 동일 패턴 — borrow 해제 후 Win32 호출 (ShowWindow 도
/// WM_WINDOWPOSCHANGED 등을 동기 송신한다).
pub fn hide() {
    // 훅 해제는 borrow 밖에서 (UnhookWindowsHookEx 가 메시지를 송신할 수 있음).
    uninstall_mouse_hook();
    let hwnd = UI_STATE.with(|st| {
        let mut st = st.borrow_mut();
        let hwnd = st.hwnd?;
        if !st.visible {
            return None;
        }
        st.visible = false;
        st.flash_until = 0;
        st.owner_conn_handle = 0; // 역송신 대상 무효화 (stale 송신 방지).
        // I11: 접근성 스냅샷도 숨김 반영(followup 이벤트/트리 구현 시 사용).
        if let Ok(mut snap) = st.a11y_snap.lock() {
            snap.visible = false;
        }
        Some(hwnd)
    });
    let Some(hwnd) = hwnd else {
        return;
    };
    unsafe {
        let _ = ShowWindow(hwnd, SW_HIDE);
        let _ = KillTimer(Some(hwnd), FLASH_TIMER_ID);
    }
    logln!("window: hide");
}

/// §5.5 동결 위치 알고리즘. 반환: (x, y, w, h, scale).
unsafe fn compute_placement(
    hwnd: HWND,
    owner_hwnd: u64,
    rs: &RenderState,
) -> (i32, i32, i32, i32, f64) {
    // 1. owner 결정.
    let owner = if owner_hwnd != 0 {
        let h = HWND(owner_hwnd as *mut _);
        if IsWindow(Some(h)).as_bool() {
            h
        } else {
            GetForegroundWindow()
        }
    } else {
        GetForegroundWindow()
    };
    // 2. 모니터.
    let hmon: HMONITOR = if owner.0.is_null() {
        MonitorFromPoint(windows::Win32::Foundation::POINT { x: 0, y: 0 }, MONITOR_DEFAULTTOPRIMARY)
    } else {
        MonitorFromWindow(owner, MONITOR_DEFAULTTONEAREST)
    };
    // 3. rcWork.
    let mut mi = MONITORINFO {
        cbSize: std::mem::size_of::<MONITORINFO>() as u32,
        ..Default::default()
    };
    let mut rc_work = RECT { left: 0, top: 0, right: 1920, bottom: 1080 };
    if GetMonitorInfoW(hmon, &mut mi).as_bool() {
        rc_work = mi.rcWork;
    } else {
        logln!("window: GetMonitorInfoW failed — fallback 1920x1080");
    }
    // 4. DPI.
    let mut dpi_x: u32 = 96;
    let mut dpi_y: u32 = 96;
    let scale = match GetDpiForMonitor(hmon, MDT_EFFECTIVE_DPI, &mut dpi_x, &mut dpi_y) {
        Ok(()) => (dpi_x as f64) / 96.0,
        Err(_) => {
            logln!("window: GetDpiForMonitor failed — dpi=96");
            1.0
        }
    };
    // 5. 콘텐츠 측정 (메모리 DC).
    let screen_dc = GetDC(Some(hwnd));
    let mem_dc = CreateCompatibleDC(Some(screen_dc));
    let (mut w, mut h) = render::measure(mem_dc, rs, scale);
    let _ = DeleteDC(mem_dc);
    let _ = ReleaseDC(Some(hwnd), screen_dc);
    let work_w = rc_work.right - rc_work.left;
    let work_h = rc_work.bottom - rc_work.top;
    w = w.min(work_w);
    h = h.min(work_h);
    // 6. 배치 — 캐럿 앵커(있으면) 우선, 실패 시 정중앙 폴백.
    //    caret_rect(left,top,right,bottom, 물리 픽셀) 하단에 팝업을 붙인다.
    //    화면 아래로 넘치면 캐럿 위로 플립, 위·아래 모두 안 들어가면 중앙 폴백.
    //    이로써 Windows 돋보기 확대 뷰포트가 캐럿을 따라갈 때 팝업도 함께 따라온다.
    let center = |w: i32, h: i32| {
        (
            rc_work.left + (work_w - w) / 2,
            rc_work.top + (work_h - h) / 2,
        )
    };
    let (x, y, anchored) = match rs.caret_rect {
        // 전(全) 0 rect 는 무효(초기화 안 됨) — 중앙 폴백. TSF 측에서도 걸러지나 이중 방어.
        Some((cl, ct, _cr, cb)) if !(cl == 0 && ct == 0 && cb == 0) => {
            // x: 캐럿 좌측 정렬 후 작업영역 안으로 클램프.
            let mut px = cl;
            if px + w > rc_work.right {
                px = rc_work.right - w;
            }
            if px < rc_work.left {
                px = rc_work.left;
            }
            // y: 캐럿 하단(cb) 우선, 넘치면 캐럿 위(ct - h)로 플립.
            if cb + h <= rc_work.bottom {
                (px, cb, true)
            } else if ct - h >= rc_work.top {
                (px, ct - h, true)
            } else {
                let (cx, cy) = center(w, h);
                (cx, cy, false)
            }
        }
        _ => {
            let (cx, cy) = center(w, h);
            (cx, cy, false)
        }
    };
    logln!(
        "window: place anchored={anchored} owner_valid={} caret={:?} rcWork=({},{},{},{}) dpi={dpi_x} scale={scale:.3} wh=({w},{h}) xy=({x},{y})",
        !owner.0.is_null(),
        rs.caret_rect,
        rc_work.left, rc_work.top, rc_work.right, rc_work.bottom
    );
    (x, y, w, h, scale)
}

extern "system" fn wnd_proc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    unsafe {
        match msg {
            WM_PAINT => {
                let mut ps = PAINTSTRUCT::default();
                let hdc = BeginPaint(hwnd, &mut ps);
                if !hdc.is_invalid() {
                    UI_STATE.with(|st| {
                        let st = st.borrow();
                        if let Some(rs) = st.last.as_ref() {
                            let mut rc = RECT::default();
                            let _ = GetClientRect(hwnd, &mut rc);
                            let flash_on = st.flash_until != 0 && now_ms() < st.flash_until;
                            // 더블 버퍼링 — 깜빡임 방지.
                            let w = rc.right - rc.left;
                            let h = rc.bottom - rc.top;
                            let mem = CreateCompatibleDC(Some(hdc));
                            let bmp = CreateCompatibleBitmap(hdc, w, h);
                            let old = SelectObject(mem, bmp.into());
                            render::paint(mem, rs, st.scale, w, h, flash_on);
                            let _ = BitBlt(hdc, 0, 0, w, h, Some(mem), 0, 0, SRCCOPY);
                            SelectObject(mem, old);
                            let _ = DeleteObject(bmp.into());
                            let _ = DeleteDC(mem);
                            logln!("window: WM_PAINT page={}/{} sel=({},{}) flash_on={flash_on}", rs.current_page, rs.total_pages, rs.sel_row, rs.sel_col);
                        }
                    });
                    let _ = EndPaint(hwnd, &ps);
                }
                LRESULT(0)
            }
            // I11: UIA 접근성 트리 진입점. UIA 루트 오브젝트(UiaRootObjectId) 요청만
            // 자체 서버측 제공자로 응답하고, 그 외 MSAA OBJID(클라이언트/캐럿 등)는
            // 기본 처리에 위임한다(오분류로 다른 접근성 경로를 막지 않기 위함).
            WM_GETOBJECT if lparam.0 as i32 == UiaRootObjectId => {
                match get_or_create_provider(hwnd) {
                    Some(prov) => {
                        logln!("window: WM_GETOBJECT(UiaRootObjectId) → provider 반환");
                        UiaReturnRawElementProvider(hwnd, wparam, lparam, &prov)
                    }
                    None => DefWindowProcW(hwnd, msg, wparam, lparam),
                }
            }
            WM_TIMER if wparam.0 == FLASH_TIMER_ID => {
                let still = UI_STATE.with(|st| {
                    let st = st.borrow();
                    st.flash_until != 0 && now_ms() < st.flash_until
                });
                if !still {
                    let _ = KillTimer(Some(hwnd), FLASH_TIMER_ID);
                    UI_STATE.with(|st| st.borrow_mut().flash_until = 0);
                    logln!("window: flash end");
                }
                let _ = InvalidateRect(Some(hwnd), None, true);
                LRESULT(0)
            }
            WM_DPICHANGED => {
                // 보이는 동안 모니터 DPI 변경 — 마지막 상태로 재측정·재배치.
                logln!("window: WM_DPICHANGED");
                let (rs, owner) = UI_STATE.with(|st| {
                    let st = st.borrow();
                    (st.last.clone(), st.last_owner_hwnd)
                });
                if let Some(rs) = rs {
                    let (x, y, w, h, scale) = compute_placement(hwnd, owner, &rs);
                    UI_STATE.with(|st| st.borrow_mut().scale = scale);
                    let _ = SetWindowPos(hwnd, Some(HWND_TOPMOST), x, y, w, h, SWP_NOACTIVATE);
                    let _ = InvalidateRect(Some(hwnd), None, true);
                }
                LRESULT(0)
            }
            // OS 테마/고대비 변경 → 팔레트 재감지를 위해 재도색(표시 중일 때만).
            // WM_SETTINGCHANGE(다크/라이트·고대비 토글), WM_THEMECHANGED(0x031A).
            // render::paint 가 current_palette() 로 매 페인트 팔레트를 다시 고르므로
            // InvalidateRect 만으로 새 테마가 반영된다.
            WM_SETTINGCHANGE | 0x031A => {
                let visible = UI_STATE.with(|st| st.borrow().visible);
                if visible {
                    logln!("window: theme/settings change (msg={msg:#x}) — repaint");
                    let _ = InvalidateRect(Some(hwnd), None, true);
                }
                DefWindowProcW(hwnd, msg, wparam, lparam)
            }
            WM_MOUSEACTIVATE => LRESULT(MA_NOACTIVATE as isize),
            // 좌클릭 내부 히트테스트 → 역IPC 송신 (§11.E). 외부 클릭은 여기로 안 옴
            // (창 밖이라 WM 미수신) — LL 훅이 담당. 포커스는 MA_NOACTIVATE 로 안 가져감.
            WM_LBUTTONDOWN => {
                let x = (lparam.0 & 0xFFFF) as i16 as i32; // GET_X_LPARAM
                let y = ((lparam.0 >> 16) & 0xFFFF) as i16 as i32; // GET_Y_LPARAM
                handle_left_click(hwnd, x, y);
                LRESULT(0)
            }
            WM_RBUTTONDOWN => {
                logln!("window: RBUTTONDOWN ignored (no-op)");
                LRESULT(0)
            }
            WM_ERASEBKGND => LRESULT(1), // 더블버퍼링 — 배경 지우기 방지.
            WM_ENDSESSION => {
                // 로그오프/셧다운 — sent message 라 wnd_proc 로 직접 전달된다.
                // 종료 사유 로그 후 정상 종료 (§5.2 / §8.2 PostQuitMessage 허용 지점 1).
                // wparam != 0 이면 세션이 실제로 종료됨.
                if wparam.0 != 0 {
                    logln!("window: WM_ENDSESSION (ENDSESSION) — shutting down");
                    PostQuitMessage(0);
                } else {
                    logln!("window: WM_ENDSESSION cancelled (wparam=0)");
                }
                LRESULT(0)
            }
            WM_DESTROY => {
                // §8.2: 팝업 창 WM_DESTROY 에서 PostQuitMessage 금지.
                logln!("window: WM_DESTROY (no PostQuitMessage)");
                LRESULT(0)
            }
            _ => DefWindowProcW(hwnd, msg, wparam, lparam),
        }
    }
}

// ─── 마우스 클릭 → 역IPC (§11.E) ────────────────────────────────────────────

/// 내부 좌클릭(클라이언트 좌표) 히트테스트 후 역이벤트 송신. 적중 없으면 로그만.
fn handle_left_click(hwnd: HWND, x: i32, y: i32) {
    let ev = UI_STATE.with(|st| {
        let st = st.borrow();
        if !st.visible {
            return None;
        }
        let rs = st.last.as_ref()?;
        // 클라이언트 rect 로 w/h 확보 (footer 우측/하단 좌표 계산용).
        let mut rc = RECT::default();
        let _ = unsafe { GetClientRect(hwnd, &mut rc) };
        let (w, h) = (rc.right - rc.left, rc.bottom - rc.top);
        render::hit_test(rs, st.scale, x, y, w, h)
    });
    match ev {
        Some(ev) => {
            logln!("window: LBUTTONDOWN ({x},{y}) hit → {ev:?}");
            send_rev(&ev);
        }
        None => {
            logln!("window: LBUTTONDOWN ({x},{y}) no hit (header/margin) — ignored");
        }
    }
}

/// 현재 owner 연결로 역이벤트를 보낸다 (UI 스레드 전용 — UI_STATE 에서 핸들/seq/owner 읽음).
///
/// **비차단** — 채널에 try_send 만 한다(가득 차면 drop). 실제 블로킹 WriteFile 은
/// writer 스레드가 수행하므로 클릭/훅 핸들러 안에서 호출해도 UI 펌프가 멎지 않는다(B4).
fn send_rev(ev: &RevEvent) {
    let (handle, owner_hwnd, seq, tx) = UI_STATE.with(|st| {
        let st = st.borrow();
        (
            st.owner_conn_handle,
            st.last_owner_hwnd,
            st.last_render_seq,
            st.rev_tx.clone(),
        )
    });
    let Some(tx) = tx else {
        logln!("send_rev: rev sender not initialized — dropped evt={ev:?}");
        return;
    };
    let msg = ev.to_wire(owner_hwnd, seq);
    try_send_reverse(&tx, handle, msg);
}

// ─── WH_MOUSE_LL 외부 클릭 훅 (§11.F) ───────────────────────────────────────

/// 팝업 표시 중에만 설치 (상주 금지). 이미 설치돼 있으면 no-op (idempotent).
/// UI 스레드(메시지 루프 보유)에서 호출돼야 한다.
fn install_mouse_hook() {
    let already = UI_STATE.with(|st| st.borrow().mouse_hook != 0);
    if already {
        return;
    }
    let hook = unsafe {
        SetWindowsHookExW(WH_MOUSE_LL, Some(low_level_mouse_proc), None, 0)
    };
    match hook {
        Ok(h) => {
            UI_STATE.with(|st| st.borrow_mut().mouse_hook = h.0 as isize);
            logln!("hook: WH_MOUSE_LL installed handle={:#x}", h.0 as isize);
        }
        Err(e) => {
            logln!("hook: SetWindowsHookExW failed ({e}) — outside-click cancel disabled");
        }
    }
}

/// 훅 해제 (hide/Deactivate/shutdown 보장 — §11.I 수명). 미설치면 no-op.
fn uninstall_mouse_hook() {
    let raw = UI_STATE.with(|st| {
        let mut st = st.borrow_mut();
        let raw = st.mouse_hook;
        st.mouse_hook = 0;
        raw
    });
    if raw == 0 {
        return;
    }
    let r = unsafe { UnhookWindowsHookEx(HHOOK(raw as *mut _)) };
    match r {
        Ok(()) => logln!("hook: WH_MOUSE_LL uninstalled"),
        Err(e) => logln!("hook: UnhookWindowsHookEx failed ({e})"),
    }
}

/// 저수준 마우스 콜백 — UI 스레드에서 실행 (훅 설치 스레드). 무거운 작업·재진입 금지.
/// 팝업 밖 좌클릭이면 outside_cancel 역송신 + 즉시 hide, 그리고 **비소비 패스쓰루**
/// (CallNextHookEx — 클릭을 앱에 그대로 전달, 요구 (4)). 안이면 무시(wndproc 가 처리).
extern "system" fn low_level_mouse_proc(code: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    unsafe {
        if code == HC_ACTION as i32 && wparam.0 as u32 == WM_LBUTTONDOWN {
            // lparam → MSLLHOOKSTRUCT.
            let ms = &*(lparam.0 as *const MSLLHOOKSTRUCT);
            let pt: POINT = ms.pt; // 스크린 좌표.
            let outside = UI_STATE.with(|st| {
                let st = st.borrow();
                if !st.visible {
                    return false; // 안 보이면 관여 안 함.
                }
                let Some(hwnd) = st.hwnd else { return false };
                let mut rc = RECT::default();
                if GetWindowRect(hwnd, &mut rc).is_err() {
                    return false;
                }
                // 팝업 rect 밖이면 외부 클릭.
                pt.x < rc.left || pt.x >= rc.right || pt.y < rc.top || pt.y >= rc.bottom
            });
            if outside {
                logln!("hook: outside LBUTTONDOWN at ({},{}) — outside_cancel + passthrough", pt.x, pt.y);
                // 역송신(취소) — TSF 가 popup_cancel 후 정방향 hide 를 보낸다(SoT 일원화).
                send_rev(&RevEvent::OutsideCancel);
                // 시각 지연 0 — 렌더러도 즉시 hide (이중 hide 무해). hide 가 훅을 해제.
                hide();
                // **비소비 패스쓰루**: 클릭을 앱에 그대로 전달 (return 1 금지).
            }
            // 내부 클릭은 무시 — wndproc 의 WM_LBUTTONDOWN 이 단독 처리 (중복 방지).
        }
        // 항상 다음 훅으로 패스 (절대 소비 금지, 요구 (4)).
        CallNextHookEx(None, code, wparam, lparam)
    }
}

// ─── I11: 후보 팝업 UIA(UI Automation) 서버측 제공자 ─────────────────────────
//
// 후보 팝업은 WM_GETOBJECT 를 무시하던 GDI 창이라 Narrator/NVDA 접근성 트리에
// 전혀 노출되지 않았다. TSF 측 I8(UILess ITfCandidateListUIElement)은 CUAS 경유
// 앱만 커버하므로, 오버레이/GDI 렌더러 경로는 이 창이 직접 UIA 루트 제공자를
// 노출해야 스크린리더가 "후보 창이 떴다"는 사실과 이름을 인지할 수 있다.
//
// ## 현 구현 범위(의도적 부분 구현 — 나머지는 followup)
// - **루트 제공자만**: ControlType=List, AutomationId="UNIM_Candidate_Window", Name.
// - HostRawElementProvider 는 `UiaHostProviderFromHwnd` 로 창 기본 접근성(바운딩
//   렉트/포커스)을 합성해 준다.
// - **미구현(followup)**: ListItem 자식 트리 + SelectionItem(IsSelected) 패턴 +
//   SelectionItem_ElementSelected / MenuOpened·MenuClosed 이벤트. 이는
//   IRawElementProviderFragment(Root) + 런타임 ID + GetBoundingRectangle +
//   UiaRaiseAutomationEvent 를 요구해 분량이 크므로 별도 작업으로 분리한다.
//   현재 스냅샷(A11ySnapshot)에 selection/items 를 담을 수 있게 뼈대를 남겨 둔다.
//
// ## 스레드 안전
// UIA 코어는 서버측 제공자(ProviderOptions_ServerSideProvider)를 창 소유 스레드가
// 아닌 스레드에서 호출할 수 있다. 따라서 provider 는 thread_local `UI_STATE` 를
// 직접 만지지 않고, UI 스레드가 갱신하는 `Arc<Mutex<A11ySnapshot>>` 스냅샷과
// HWND raw(isize)만 보유한다(ui_element.rs 와 동일 원칙). 모든 COM-facing 메서드는
// 절대 패닉하지 않는다(`panic=abort` 하에서 호스트 앱을 조용히 죽이지 않기 위함).

/// UIA 제공자가 읽는 접근성 스냅샷. UI 스레드가 show_render/hide 에서 갱신한다.
#[derive(Default)]
struct A11ySnapshot {
    /// 루트 Name(스크린리더 낭독용). header_text 우선, 없으면 기본 라벨.
    name: String,
    /// 표시 여부(followup 이벤트/트리 구현 시 사용 — 현재는 미소비).
    #[allow(dead_code)]
    visible: bool,
}

/// RenderState → 루트 Name 문자열. 헤더가 있으면 그대로, 없으면 기본 라벨.
fn a11y_name(rs: &RenderState) -> String {
    if rs.header_text.trim().is_empty() {
        "UNIM 후보 목록".to_string()
    } else {
        rs.header_text.clone()
    }
}

/// VT_I4 VARIANT 생성(ControlType 등 정수 프로퍼티용).
fn variant_i4(v: i32) -> VARIANT {
    let mut var = VARIANT::default();
    // SAFETY: VARIANT 는 union — vt 를 VT_I4 로 지정한 뒤 대응 필드(lVal)만 기록한다.
    unsafe {
        let inner = &mut var.Anonymous.Anonymous;
        inner.vt = VT_I4;
        inner.Anonymous.lVal = v;
    }
    var
}

/// VT_BSTR VARIANT 생성(Name/AutomationId 등 문자열 프로퍼티용).
/// 반환된 VARIANT 의 BSTR 소유권은 호출자(UIA 코어)로 넘어가며 VariantClear 로 해제된다.
fn variant_bstr(s: &str) -> VARIANT {
    let mut var = VARIANT::default();
    // SAFETY: VARIANT union — vt=VT_BSTR 후 bstrVal 에 새로 할당한 BSTR 을 넣는다.
    // ManuallyDrop 로 감싸 Rust 가 조기 해제하지 않게 하고, 해제 책임은 UIA 코어에 있다.
    unsafe {
        let inner = &mut var.Anonymous.Anonymous;
        inner.vt = VT_BSTR;
        inner.Anonymous.bstrVal = std::mem::ManuallyDrop::new(BSTR::from(s));
    }
    var
}

/// 후보 팝업 UIA 루트 제공자. HWND raw + 접근성 스냅샷만 보유(크로스스레드 안전).
#[implement(IRawElementProviderSimple)]
struct CandidateProvider {
    /// 대상 창 HWND raw(크로스스레드 호출 대비 isize 로 보관, 사용 시 HWND 재구성).
    hwnd: isize,
    /// UI 스레드가 갱신하는 접근성 스냅샷 공유 핸들.
    snap: Arc<Mutex<A11ySnapshot>>,
}

impl IRawElementProviderSimple_Impl for CandidateProvider_Impl {
    fn ProviderOptions(&self) -> WinResult<ProviderOptions> {
        Ok(ProviderOptions_ServerSideProvider)
    }

    fn GetPatternProvider(&self, _patternid: UIA_PATTERN_ID) -> WinResult<IUnknown> {
        // 루트 단계에서 지원하는 컨트롤 패턴 없음. windows-rs 관례상 `Error::empty()`
        // 는 S_OK + null out-param 으로 매핑되어 "패턴 미지원"을 올바로 통지한다.
        // Selection 패턴 및 셀 SelectionItem 은 followup(자식 트리와 함께).
        Err(Error::empty())
    }

    fn GetPropertyValue(&self, propertyid: UIA_PROPERTY_ID) -> WinResult<VARIANT> {
        // 혼합 대소문자 UIA 상수는 match 패턴으로 쓰면 non_upper_case_globals 경고가
        // 나므로 `==` 비교 분기로 처리한다.
        let v = if propertyid == UIA_ControlTypePropertyId {
            variant_i4(UIA_ListControlTypeId.0)
        } else if propertyid == UIA_AutomationIdPropertyId {
            variant_bstr("UNIM_Candidate_Window")
        } else if propertyid == UIA_NamePropertyId {
            let name = self
                .snap
                .lock()
                .map(|s| s.name.clone())
                .unwrap_or_default();
            let name = if name.is_empty() {
                "UNIM 후보 목록".to_string()
            } else {
                name
            };
            variant_bstr(&name)
        } else {
            // 그 외 프로퍼티는 VT_EMPTY → UIA 가 기본값을 사용(요청 미지원 관례).
            VARIANT::default()
        };
        Ok(v)
    }

    fn HostRawElementProvider(&self) -> WinResult<IRawElementProviderSimple> {
        // 창 기본 접근성(바운딩 렉트/native window handle) 합성 제공자.
        // SAFETY: 저장해 둔 HWND raw 로 창 핸들을 재구성한다(창 수명 내 유효).
        unsafe { UiaHostProviderFromHwnd(HWND(self.hwnd as *mut _)) }
    }
}

/// WM_GETOBJECT 처리용 — 루트 제공자를 지연 생성(최초 1회) 후 클론 반환.
/// UI 스레드에서만 호출된다(wnd_proc). UI_STATE borrow 는 반환 전에 해제된다.
fn get_or_create_provider(hwnd: HWND) -> Option<IRawElementProviderSimple> {
    UI_STATE.with(|st| {
        let mut st = st.borrow_mut();
        if st.a11y_provider.is_none() {
            let snap = Arc::clone(&st.a11y_snap);
            let prov: IRawElementProviderSimple = CandidateProvider {
                hwnd: hwnd.0 as isize,
                snap,
            }
            .into();
            st.a11y_provider = Some(prov);
        }
        st.a11y_provider.clone()
    })
}
