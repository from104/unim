//! 상황 알림(토스트) 창 — 포커스 창이 있는 모니터의 작업 영역 모서리에 잠깐 떴다 사라진다
//! (NOTIFY_SPEC §3.4, 결정 D6·Q4).
//!
//! - 별도 HWND 1개를 프로세스 수명 동안 재사용한다(후보 팝업 창과 독립). **최신 1장 교체**:
//!   새 토스트가 오면 내용·위치·타이머를 갈아끼운다.
//! - **포커스 절대 미탈취**: `WS_EX_NOACTIVATE | WS_EX_TOPMOST | WS_EX_TOOLWINDOW`(작업표시줄
//!   미표시), `WS_EX_TRANSPARENT` + `WM_NCHITTEST=HTTRANSPARENT`(클릭 투과), `WM_MOUSEACTIVATE=
//!   MA_NOACTIVATE`, 표시는 `SW_SHOWNOACTIVATE`/`SWP_NOACTIVATE`. 코너에 놓인 앱 버튼 클릭을 막지 않는다.
//! - 렌더러 측 `(kind, key_hash)` 3초 dedupe([`crate::toast_logic::ToastDedupe`]) — 호스트 앱 간 중복 제거.
//! - 색은 후보 팝업과 같은 팔레트([`crate::render::toast_colors`] = `current_palette`, 고대비 포함).
//! - UIA 통보: `UiaRaiseNotificationEvent`(ActionCompleted/MostRecent). 구 Windows 에서 임포트 누락으로
//!   프로세스가 뜨지 못하지 않도록 동적 로드하고, 실패는 무시한다.
//! - 로그에는 **kind 만** 남긴다(제목·본문 금지 — 입력 텍스트가 실릴 수 있다).
//!
//! 창 메시지·타이머는 UI 스레드 단일 소유다. `SetWindowPos`/`ShowWindow` 는 `WM_DPICHANGED` 등을 동기
//! 송신하므로, `TOAST` 상태 borrow 를 쥔 채 호출하지 않는다(재진입 borrow = abort).

use std::cell::RefCell;
use std::ffi::c_void;
use std::sync::{Arc, Mutex, OnceLock};
use std::time::Instant;

use windows::core::{implement, s, w, Error, IUnknown, Interface, Result as WinResult, BSTR, PCWSTR};
use windows::Win32::Foundation::{COLORREF, HWND, LPARAM, LRESULT, RECT, WPARAM};
use windows::Win32::Graphics::Gdi::*;
use windows::Win32::System::LibraryLoader::{GetProcAddress, LoadLibraryW};
use windows::Win32::System::Variant::{VARIANT, VT_BSTR, VT_I4};
use windows::Win32::UI::Accessibility::{
    IRawElementProviderSimple, IRawElementProviderSimple_Impl, NotificationKind_ActionCompleted,
    NotificationProcessing_MostRecent, ProviderOptions, ProviderOptions_ServerSideProvider,
    UiaHostProviderFromHwnd, UiaReturnRawElementProvider, UiaRootObjectId,
    UIA_AutomationIdPropertyId, UIA_ControlTypePropertyId, UIA_NamePropertyId, UIA_PATTERN_ID,
    UIA_PROPERTY_ID, UIA_TextControlTypeId,
};
use windows::Win32::UI::HiDpi::{GetDpiForMonitor, MDT_EFFECTIVE_DPI};
use windows::Win32::UI::WindowsAndMessaging::*;

use crate::logln;
use crate::protocol::ToastPayload;
use crate::render::{self, ToastColors};
use crate::toast_logic::{
    clamp_duration_ms, clean_line, clean_multiline, corner_position, spoken_text, Corner,
    rects_overlap, ToastDedupe, BODY_MAX_CHARS, TITLE_MAX_CHARS,
};

const CLASS_NAME: PCWSTR = w!("UNIM_ToastWnd");
/// 자동 소멸 타이머 ID (이 창 전용).
const TIMER_ID: usize = 0x7701;

// ─── 논리 레이아웃(96 DPI 기준 px; 표시 시 모니터 배율을 곱한다) ──────────────
const PAD: i32 = 12; // 안쪽 여백
const ACCENT_W: i32 = 4; // 좌측 강조 막대
const GAP: i32 = 4; // 제목-본문 간격
const MARGIN: i32 = 16; // 작업 영역 가장자리와의 간격
const MIN_TEXT_W: i32 = 140; // 본문 영역 최소 폭
const MAX_TEXT_W: i32 = 320; // 본문 영역 최대 폭
const BODY_MAX_LINES: i32 = 4;
const FONT_TITLE: i32 = 13; // bold
const FONT_BODY: i32 = 12;

thread_local! {
    /// UI 스레드 전용 상태.
    static TOAST: RefCell<ToastState> = RefCell::new(ToastState::default());
}

#[derive(Default)]
struct ToastState {
    hwnd: Option<HWND>,
    title: String,
    body: String,
    scale: f64,
    visible: bool,
    dedupe: ToastDedupe,
    /// UIA 제공자가 읽는 낭독 문자열(제공자는 다른 스레드에서 호출될 수 있어 공유 핸들).
    spoken: Arc<Mutex<String>>,
    provider: Option<IRawElementProviderSimple>,
}

/// 윈도우 클래스 등록 + 숨김 토스트 HWND 1회 생성. UI 스레드에서 호출.
pub fn create() -> WinResult<HWND> {
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
        if RegisterClassExW(&wc) == 0 {
            logln!(
                "toast: RegisterClassExW failed err={:?}",
                windows::Win32::Foundation::GetLastError()
            );
        }
        let hwnd = CreateWindowExW(
            WS_EX_NOACTIVATE
                | WS_EX_TOPMOST
                | WS_EX_TOOLWINDOW
                | WS_EX_LAYERED
                | WS_EX_TRANSPARENT,
            CLASS_NAME,
            w!(""),
            WS_POPUP,
            0,
            0,
            10,
            10,
            None,
            None,
            Some(hinstance.into()),
            None,
        )?;
        // 레이어드 창은 알파를 지정해야 보인다(불투명 100%).
        let _ = SetLayeredWindowAttributes(hwnd, COLORREF(0), 255, LWA_ALPHA);
        // Win11 라운드 코너 + DWM 그림자(구버전은 각 호출이 실패해도 무시).
        crate::window::apply_modern_frame(hwnd);
        TOAST.with(|t| {
            let mut t = t.borrow_mut();
            t.hwnd = Some(hwnd);
            t.scale = 1.0;
        });
        logln!("toast: window created hwnd={:?}", hwnd.0);
        Ok(hwnd)
    }
}

/// `cmd="toast"` 처리 — dedupe → 정리 → 배치 → 표시 → 타이머 → UIA 통보.
///
/// `owner_hwnd` 는 TSF 가 보낸 포커스 창(0 이면 `GetForegroundWindow()`).
pub fn show(p: &ToastPayload, owner_hwnd: u64) {
    let now = Instant::now();
    let Some(hwnd) = TOAST.with(|t| t.borrow().hwnd) else {
        return;
    };
    // 앱 간 중복 억제(§3.4) — 통과 시 기록까지 한다.
    let admitted = TOAST.with(|t| t.borrow_mut().dedupe.admit(&p.kind, p.key_hash, now));
    if !admitted {
        logln!("toast: dedupe drop kind={}", p.kind);
        return;
    }
    let title = clean_line(&p.title, TITLE_MAX_CHARS);
    let body = clean_multiline(&p.body, BODY_MAX_CHARS);
    if title.is_empty() && body.is_empty() {
        logln!("toast: empty payload kind={} — ignored", p.kind);
        return;
    }
    let duration = clamp_duration_ms(p.duration_ms);
    let corner = Corner::parse(&p.corner);

    // 배치 계산(쿼리 전용 Win32 — 메시지 디스패치 없음). borrow 를 쥐지 않는다.
    let (x, y, w, h, scale) = unsafe { place(hwnd, owner_hwnd, &title, &body, corner) };

    // 후보 팝업(한자·특수문자·이모지)이 보이고 토스트 자리와 겹치면 팝업 우선 — 토스트는 뜨지 않는다.
    if let Some(pr) = crate::window::visible_rect() {
        if rects_overlap((x, y, x + w, y + h), pr) {
            logln!("toast: overlaps popup — suppressed kind={}", p.kind);
            hide();
            return;
        }
    }

    let (spoken, provider) = TOAST.with(|t| {
        let mut t = t.borrow_mut();
        t.title = title.clone();
        t.body = body.clone();
        t.scale = scale;
        let spoken = spoken_text(&title, &body);
        if let Ok(mut s) = t.spoken.lock() {
            *s = spoken.clone();
        }
        (spoken, get_or_create_provider(&mut t, hwnd))
    });
    let was_visible = TOAST.with(|t| {
        let mut t = t.borrow_mut();
        let v = t.visible;
        t.visible = true;
        v
    });

    unsafe {
        // Z 순서는 건드리지 않는다(SWP_NOZORDER). 팝업과 겹치는 경우는 위에서 이미 걸러졌고,
        // 팝업이 나중에 뜨면 `yield_to_popup` 이 겹친 토스트를 숨긴다.
        let _ = SetWindowPos(
            hwnd,
            None,
            x,
            y,
            w,
            h,
            SWP_NOACTIVATE | SWP_NOZORDER,
        );
        if !was_visible {
            let _ = ShowWindow(hwnd, SW_SHOWNOACTIVATE);
        }
        let _ = InvalidateRect(Some(hwnd), None, true);
        // 같은 ID 로 다시 걸면 이전 타이머를 대체한다 → 최신 1장 교체 시 시간도 새로 센다.
        SetTimer(Some(hwnd), TIMER_ID, duration, None);
        if let Some(prov) = provider.as_ref() {
            raise_uia_notification(prov, &spoken);
        }
    }
    logln!(
        "toast: show kind={} xy=({x},{y}) wh=({w},{h}) scale={scale:.3} dur={duration}ms corner={corner:?}",
        p.kind
    );
}

/// 후보 팝업이 `popup`(화면 좌표 `(l,t,r,b)`)에 표시될 때 호출 — 보이는 토스트가 겹치면 숨긴다
/// (팝업 우선, NOTIFY_SPEC §3.4). 팝업 `SetWindowPos` 뒤, 어떤 borrow 도 쥐지 않고 부른다.
pub fn yield_to_popup(popup: (i32, i32, i32, i32)) {
    let Some(hwnd) = TOAST.with(|t| {
        let t = t.borrow();
        if t.visible {
            t.hwnd
        } else {
            None
        }
    }) else {
        return;
    };
    let mut r = RECT::default();
    if unsafe { GetWindowRect(hwnd, &mut r) }.is_err() {
        return;
    }
    if rects_overlap((r.left, r.top, r.right, r.bottom), popup) {
        logln!("toast: popup shown over toast — hidden");
        hide();
    }
}

/// 토스트를 즉시 숨긴다(타이머 만료·종료 경로).
pub fn hide() {
    let hwnd = TOAST.with(|t| {
        let mut t = t.borrow_mut();
        let hwnd = t.hwnd?;
        let was = t.visible;
        t.visible = false;
        t.title.clear();
        t.body.clear();
        Some((hwnd, was))
    });
    let Some((hwnd, was)) = hwnd else { return };
    unsafe {
        let _ = KillTimer(Some(hwnd), TIMER_ID);
        if was {
            let _ = ShowWindow(hwnd, SW_HIDE);
        }
    }
}

// ─── 배치·측정 ──────────────────────────────────────────────────────────────

/// 포커스 창이 있는 모니터(D6)의 작업 영역 모서리에 놓을 `(x, y, w, h, scale)`.
unsafe fn place(
    hwnd: HWND,
    owner_hwnd: u64,
    title: &str,
    body: &str,
    corner: Corner,
) -> (i32, i32, i32, i32, f64) {
    // 1. owner — 전달된 HWND 가 살아 있으면 그것, 아니면 포그라운드.
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
        MonitorFromPoint(
            windows::Win32::Foundation::POINT { x: 0, y: 0 },
            MONITOR_DEFAULTTOPRIMARY,
        )
    } else {
        MonitorFromWindow(owner, MONITOR_DEFAULTTONEAREST)
    };
    // 3. 작업 영역(작업표시줄 제외).
    let mut mi = MONITORINFO {
        cbSize: std::mem::size_of::<MONITORINFO>() as u32,
        ..Default::default()
    };
    let mut rc_work = RECT { left: 0, top: 0, right: 1920, bottom: 1080 };
    if GetMonitorInfoW(hmon, &mut mi).as_bool() {
        rc_work = mi.rcWork;
    } else {
        logln!("toast: GetMonitorInfoW failed — fallback 1920x1080");
    }
    // 4. 모니터 DPI.
    let (mut dpi_x, mut dpi_y) = (96u32, 96u32);
    let scale = match GetDpiForMonitor(hmon, MDT_EFFECTIVE_DPI, &mut dpi_x, &mut dpi_y) {
        Ok(()) => (dpi_x as f64) / 96.0,
        Err(_) => 1.0,
    };
    // 5. 콘텐츠 측정 → 창 크기.
    let (mut w, mut h) = measure(hwnd, title, body, scale);
    let work_w = rc_work.right - rc_work.left;
    let work_h = rc_work.bottom - rc_work.top;
    w = w.min(work_w);
    h = h.min(work_h);
    let (x, y) = corner_position(
        corner,
        (rc_work.left, rc_work.top, rc_work.right, rc_work.bottom),
        w,
        h,
        render::s(MARGIN, scale),
    );
    (x, y, w, h, scale)
}

unsafe fn line_height(hdc: HDC, font: HFONT) -> i32 {
    let old = SelectObject(hdc, font.into());
    let mut tm = TEXTMETRICW::default();
    let _ = GetTextMetricsW(hdc, &mut tm);
    SelectObject(hdc, old);
    tm.tmHeight
}

/// 창 크기(px). 본문은 최대 폭에서 줄바꿈하고 [`BODY_MAX_LINES`] 줄까지만 센다.
unsafe fn measure(hwnd: HWND, title: &str, body: &str, scale: f64) -> (i32, i32) {
    let screen_dc = GetDC(Some(hwnd));
    let dc = CreateCompatibleDC(Some(screen_dc));
    let f_title = render::make_font(FONT_TITLE, scale, true);
    let f_body = render::make_font(FONT_BODY, scale, false);

    let max_w = render::s(MAX_TEXT_W, scale);
    let min_w = render::s(MIN_TEXT_W, scale);
    let (mut text_w, mut text_h) = (0i32, 0i32);

    if !title.is_empty() {
        let old = SelectObject(dc, f_title.into());
        let mut rc = RECT { left: 0, top: 0, right: max_w, bottom: 0 };
        let mut wide = render::to_wide(title);
        DrawTextW(dc, &mut wide, &mut rc, DT_CALCRECT | DT_SINGLELINE | DT_NOPREFIX);
        SelectObject(dc, old);
        text_w = text_w.max(rc.right - rc.left);
        text_h += (rc.bottom - rc.top).max(line_height(dc, f_title));
    }
    if !body.is_empty() {
        if !title.is_empty() {
            text_h += render::s(GAP, scale);
        }
        let old = SelectObject(dc, f_body.into());
        let mut rc = RECT { left: 0, top: 0, right: max_w, bottom: 0 };
        let mut wide = render::to_wide(body);
        DrawTextW(dc, &mut wide, &mut rc, DT_CALCRECT | DT_WORDBREAK | DT_NOPREFIX);
        SelectObject(dc, old);
        let lh = line_height(dc, f_body);
        text_w = text_w.max(rc.right - rc.left);
        text_h += (rc.bottom - rc.top).min(lh * BODY_MAX_LINES).max(lh);
    }

    let _ = DeleteObject(f_title.into());
    let _ = DeleteObject(f_body.into());
    let _ = DeleteDC(dc);
    let _ = ReleaseDC(Some(hwnd), screen_dc);

    let text_w = text_w.clamp(min_w, max_w);
    let w = render::s(ACCENT_W, scale) + render::s(PAD, scale) * 2 + text_w;
    let h = render::s(PAD, scale) * 2 + text_h;
    (w, h)
}

// ─── 도색 ───────────────────────────────────────────────────────────────────

unsafe fn paint(hdc: HDC, w: i32, h: i32, scale: f64, title: &str, body: &str, c: &ToastColors) {
    // 배경 + 1px 테두리.
    let full = RECT { left: 0, top: 0, right: w, bottom: h };
    render::fill(hdc, &full, c.border);
    let inner = RECT { left: 1, top: 1, right: w - 1, bottom: h - 1 };
    render::fill(hdc, &inner, c.bg);
    // 좌측 강조 막대(색 + 형태 단서).
    let bar = RECT { left: 1, top: 1, right: 1 + render::s(ACCENT_W, scale), bottom: h - 1 };
    render::fill(hdc, &bar, c.accent);

    let left = render::s(ACCENT_W, scale) + render::s(PAD, scale);
    let right = w - render::s(PAD, scale);
    let mut y = render::s(PAD, scale);
    if !title.is_empty() {
        let f = render::make_font(FONT_TITLE, scale, true);
        let old = SelectObject(hdc, f.into());
        let lh = line_height(hdc, f);
        let rc = RECT { left, top: y, right, bottom: y + lh };
        render::draw_text(hdc, title, &rc, c.fg, DT_LEFT | DT_TOP | DT_SINGLELINE | DT_END_ELLIPSIS | DT_NOPREFIX);
        SelectObject(hdc, old);
        let _ = DeleteObject(f.into());
        y += lh;
        if !body.is_empty() {
            y += render::s(GAP, scale);
        }
    }
    if !body.is_empty() {
        let f = render::make_font(FONT_BODY, scale, false);
        let old = SelectObject(hdc, f.into());
        let rc = RECT { left, top: y, right, bottom: h - render::s(PAD, scale) };
        let color = if title.is_empty() { c.fg } else { c.sub };
        render::draw_text(hdc, body, &rc, color, DT_LEFT | DT_TOP | DT_WORDBREAK | DT_END_ELLIPSIS | DT_NOPREFIX);
        SelectObject(hdc, old);
        let _ = DeleteObject(f.into());
    }
}

extern "system" fn wnd_proc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    unsafe {
        match msg {
            WM_PAINT => {
                let mut ps = PAINTSTRUCT::default();
                let hdc = BeginPaint(hwnd, &mut ps);
                if !hdc.is_invalid() {
                    let colors = render::toast_colors();
                    TOAST.with(|t| {
                        let t = t.borrow();
                        let mut rc = RECT::default();
                        let _ = GetClientRect(hwnd, &mut rc);
                        let (w, h) = (rc.right - rc.left, rc.bottom - rc.top);
                        // 더블 버퍼링 — 깜빡임 방지.
                        let mem = CreateCompatibleDC(Some(hdc));
                        let bmp = CreateCompatibleBitmap(hdc, w, h);
                        let old = SelectObject(mem, bmp.into());
                        paint(mem, w, h, t.scale.max(1.0), &t.title, &t.body, &colors);
                        let _ = BitBlt(hdc, 0, 0, w, h, Some(mem), 0, 0, SRCCOPY);
                        SelectObject(mem, old);
                        let _ = DeleteObject(bmp.into());
                        let _ = DeleteDC(mem);
                    });
                    let _ = EndPaint(hwnd, &ps);
                }
                LRESULT(0)
            }
            WM_ERASEBKGND => LRESULT(1),
            // 포커스·활성화 절대 불가 + 클릭 투과.
            WM_MOUSEACTIVATE => LRESULT(MA_NOACTIVATE as isize),
            WM_NCHITTEST => LRESULT(HTTRANSPARENT as isize),
            WM_TIMER if wparam.0 == TIMER_ID => {
                hide();
                LRESULT(0)
            }
            // OS 테마/고대비 변경 → 표시 중이면 팔레트 재감지를 위해 재도색.
            WM_SETTINGCHANGE | 0x031A => {
                let visible = TOAST.with(|t| t.borrow().visible);
                if visible {
                    let _ = InvalidateRect(Some(hwnd), None, true);
                }
                DefWindowProcW(hwnd, msg, wparam, lparam)
            }
            WM_GETOBJECT if lparam.0 as i32 == UiaRootObjectId => {
                let prov = TOAST.with(|t| get_or_create_provider(&mut t.borrow_mut(), hwnd));
                match prov {
                    Some(p) => UiaReturnRawElementProvider(hwnd, wparam, lparam, &p),
                    None => DefWindowProcW(hwnd, msg, wparam, lparam),
                }
            }
            // 후보 팝업 창과 같은 규칙: 이 창의 파괴가 프로세스를 끝내면 안 된다.
            WM_DESTROY => LRESULT(0),
            _ => DefWindowProcW(hwnd, msg, wparam, lparam),
        }
    }
}

// ─── UIA ────────────────────────────────────────────────────────────────────

fn variant_i4(v: i32) -> VARIANT {
    let mut var = VARIANT::default();
    // SAFETY: VARIANT union — vt 를 VT_I4 로 지정한 뒤 lVal 만 기록한다.
    unsafe {
        let inner = &mut var.Anonymous.Anonymous;
        inner.vt = VT_I4;
        inner.Anonymous.lVal = v;
    }
    var
}

fn variant_bstr(s: &str) -> VARIANT {
    let mut var = VARIANT::default();
    // SAFETY: vt=VT_BSTR 후 bstrVal 에 새 BSTR 을 넣는다(해제 책임은 UIA 코어, VariantClear).
    unsafe {
        let inner = &mut var.Anonymous.Anonymous;
        inner.vt = VT_BSTR;
        inner.Anonymous.bstrVal = std::mem::ManuallyDrop::new(BSTR::from(s));
    }
    var
}

/// 토스트 창 UIA 루트 제공자. HWND raw + 낭독 문자열 공유 핸들만 보유(크로스스레드 안전).
#[implement(IRawElementProviderSimple)]
struct ToastProvider {
    hwnd: isize,
    spoken: Arc<Mutex<String>>,
}

impl IRawElementProviderSimple_Impl for ToastProvider_Impl {
    fn ProviderOptions(&self) -> WinResult<ProviderOptions> {
        Ok(ProviderOptions_ServerSideProvider)
    }

    fn GetPatternProvider(&self, _patternid: UIA_PATTERN_ID) -> WinResult<IUnknown> {
        // 지원하는 컨트롤 패턴 없음(S_OK + null).
        Err(Error::empty())
    }

    fn GetPropertyValue(&self, propertyid: UIA_PROPERTY_ID) -> WinResult<VARIANT> {
        let v = if propertyid == UIA_ControlTypePropertyId {
            variant_i4(UIA_TextControlTypeId.0)
        } else if propertyid == UIA_AutomationIdPropertyId {
            variant_bstr("UNIM_Toast_Window")
        } else if propertyid == UIA_NamePropertyId {
            let name = self.spoken.lock().map(|s| s.clone()).unwrap_or_default();
            variant_bstr(&name)
        } else {
            VARIANT::default()
        };
        Ok(v)
    }

    fn HostRawElementProvider(&self) -> WinResult<IRawElementProviderSimple> {
        // SAFETY: 저장해 둔 HWND raw 로 창 핸들을 재구성한다(창은 프로세스 수명).
        unsafe { UiaHostProviderFromHwnd(HWND(self.hwnd as *mut _)) }
    }
}

fn get_or_create_provider(t: &mut ToastState, hwnd: HWND) -> Option<IRawElementProviderSimple> {
    if t.provider.is_none() {
        let prov: IRawElementProviderSimple = ToastProvider {
            hwnd: hwnd.0 as isize,
            spoken: Arc::clone(&t.spoken),
        }
        .into();
        t.provider = Some(prov);
    }
    t.provider.clone()
}

type UiaRaiseNotificationEventFn = unsafe extern "system" fn(
    provider: *mut c_void,
    kind: i32,
    processing: i32,
    display: *mut c_void,
    activity_id: *mut c_void,
) -> i32;

/// `UiaRaiseNotificationEvent`(Windows 10 1709+)를 동적으로 해석한다 — 정적 임포트면 구버전에서
/// 렌더러 자체가 뜨지 못한다. 못 찾으면 `None`(통보 생략).
fn uia_notification_fn() -> Option<UiaRaiseNotificationEventFn> {
    static PROC: OnceLock<Option<usize>> = OnceLock::new();
    let addr = PROC.get_or_init(|| unsafe {
        let lib = LoadLibraryW(w!("uiautomationcore.dll")).ok()?;
        GetProcAddress(lib, s!("UiaRaiseNotificationEvent")).map(|f| f as usize)
    });
    // SAFETY: 주소는 같은 시그니처의 export 에서 얻었다.
    addr.map(|a| unsafe { std::mem::transmute::<usize, UiaRaiseNotificationEventFn>(a) })
}

/// 스크린리더(Narrator 등)에 토스트 문구를 알린다. 실패는 로그만 남기고 무시한다.
unsafe fn raise_uia_notification(provider: &IRawElementProviderSimple, text: &str) {
    if text.is_empty() {
        return;
    }
    let Some(f) = uia_notification_fn() else {
        return;
    };
    let display = BSTR::from(text);
    let activity = BSTR::from("UNIM_Toast");
    // SAFETY: BSTR 은 포인터 하나 크기 — UiaRaiseNotificationEvent 는 문자열을 복사해 간다.
    let hr = f(
        provider.as_raw(),
        NotificationKind_ActionCompleted.0,
        NotificationProcessing_MostRecent.0,
        std::mem::transmute_copy(&display),
        std::mem::transmute_copy(&activity),
    );
    if hr < 0 {
        logln!("toast: UIA notification failed hr={hr:#x} — ignored");
    }
}
