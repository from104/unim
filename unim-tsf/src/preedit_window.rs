//! TSF client-side preedit 오버레이 — 터미널·레거시(CUAS-unaware) 앱용
//!
//! wezterm·Telegram 등은 composition 을 즉시 OnCompositionTerminated 로 죽이고,
//! 문서 역방향 편집(ShiftStart 삭제)도 거부한다. 그래서 조합 중 음절을 앱 버퍼에
//! 쓰지 않고 이 오버레이 창에 그린다(리눅스 fcitx/ibus 의 client-side preedit 방식).
//! 확정(commit)된 글자만 앱에 insert_text 로 추가되므로 삭제가 영원히 불필요하다.
//!
//! ## 설계 결정
//! - `WS_EX_NOACTIVATE` 필수: 포커스 도난 → 타깃 앱 composition terminate 방지.
//! - GDI 만 사용. 윈도우 클래스는 최초 1회 등록(`OnceLock`).
//! - 한자 팝업(popup_window.rs)과 **별도 창** — preedit "한" + 후보창 동시 표시 가능.
//! - 텍스트 폭은 GetTextExtentPoint32W 로 측정해 창 크기를 음절 길이에 맞춘다.

use std::sync::OnceLock;

use windows::core::*;
use windows::Win32::Foundation::*;
use windows::Win32::Graphics::Gdi::*;
use windows::Win32::UI::WindowsAndMessaging::*;

// ─── 상수 ──────────────────────────────────────────────────────────────────

const WND_CLASS_NAME: PCWSTR = w!("UNIM_PreeditWnd");

/// 폰트 높이 (픽셀)
const FONT_H: i32 = 20;
/// 텍스트 주변 여백
const PAD_X: i32 = 8;
const PAD_Y: i32 = 4;
/// 밑줄 두께
const UNDERLINE_H: i32 = 2;
/// 측정 실패 시 글자당 추정 폭
const FALLBACK_CHAR_W: i32 = 18;

// 색상 (COLORREF = 0x00BBGGRR)
const COLOR_BG: COLORREF = COLORREF(0x00FAFAFA); // 거의 흰색
const COLOR_BORDER: COLORREF = COLORREF(0x00C0C0C0);
const COLOR_TEXT: COLORREF = COLORREF(0x00202020);
const COLOR_UNDERLINE: COLORREF = COLORREF(0x00D77800); // 파랑(#0078D7) — 미확정 표시

// ─── 클래스 등록 (1회) ─────────────────────────────────────────────────────

static WND_CLASS_ATOM: OnceLock<u16> = OnceLock::new();

unsafe fn ensure_wnd_class() {
    WND_CLASS_ATOM.get_or_init(|| {
        let wc = WNDCLASSEXW {
            cbSize: std::mem::size_of::<WNDCLASSEXW>() as u32,
            style: CS_HREDRAW | CS_VREDRAW,
            lpfnWndProc: Some(preedit_wnd_proc),
            cbClsExtra: 0,
            cbWndExtra: std::mem::size_of::<usize>() as i32,
            hInstance: crate::dll_instance().into(),
            hIcon: HICON::default(),
            hCursor: LoadCursorW(None, IDC_ARROW).unwrap_or_default(),
            hbrBackground: HBRUSH(GetStockObject(WHITE_BRUSH).0),
            lpszMenuName: PCWSTR::null(),
            lpszClassName: WND_CLASS_NAME,
            hIconSm: HICON::default(),
        };
        RegisterClassExW(&wc)
    });
}

// ─── 상태 ──────────────────────────────────────────────────────────────────

struct PreeditState {
    visible: bool,
    text: String,
}

impl Default for PreeditState {
    fn default() -> Self {
        Self {
            visible: false,
            text: String::new(),
        }
    }
}

fn to_wide(s: &str) -> Vec<u16> {
    s.encode_utf16().collect()
}

/// preedit 전용 폰트를 생성한다(호출자가 DeleteObject 책임).
unsafe fn create_font() -> HFONT {
    CreateFontW(
        FONT_H,
        0,
        0,
        0,
        FW_NORMAL.0 as i32,
        0,
        0,
        0,
        DEFAULT_CHARSET,
        OUT_DEFAULT_PRECIS,
        CLIP_DEFAULT_PRECIS,
        CLEARTYPE_QUALITY,
        (DEFAULT_PITCH.0 | FF_DONTCARE.0) as u32,
        w!("맑은 고딕"),
    )
}

/// 텍스트 폭(px)을 측정한다. 측정 실패 시 글자수×추정폭으로 대체.
unsafe fn measure_text_w(text: &str) -> i32 {
    let hdc = GetDC(None);
    if hdc == HDC::default() {
        return text.chars().count() as i32 * FALLBACK_CHAR_W;
    }
    let font = create_font();
    let old = SelectObject(hdc, font.into());
    let wide = to_wide(text);
    let mut size = SIZE::default();
    let ok = GetTextExtentPoint32W(hdc, &wide, &mut size).as_bool();
    SelectObject(hdc, old);
    let _ = DeleteObject(font.into());
    ReleaseDC(None, hdc);
    if ok && size.cx > 0 {
        size.cx
    } else {
        text.chars().count() as i32 * FALLBACK_CHAR_W
    }
}

/// PreeditState 를 HDC 에 렌더한다.
unsafe fn render(hdc: HDC, state: &PreeditState, win_w: i32, win_h: i32) {
    if !state.visible || state.text.is_empty() {
        return;
    }

    // 배경
    let bg_rect = RECT {
        left: 0,
        top: 0,
        right: win_w,
        bottom: win_h,
    };
    let bg = CreateSolidBrush(COLOR_BG);
    let _ = FillRect(hdc, &bg_rect, bg);
    let _ = DeleteObject(bg.into());

    // 테두리
    let border = CreateSolidBrush(COLOR_BORDER);
    let _ = FrameRect(hdc, &bg_rect, border);
    let _ = DeleteObject(border.into());

    // 텍스트
    let font = create_font();
    let old_font = SelectObject(hdc, font.into());
    SetBkMode(hdc, TRANSPARENT);
    SetTextColor(hdc, COLOR_TEXT);
    let mut text_rect = RECT {
        left: PAD_X,
        top: PAD_Y,
        right: win_w - PAD_X,
        bottom: win_h - PAD_Y,
    };
    let mut ws = to_wide(&state.text);
    DrawTextW(
        hdc,
        &mut ws,
        &mut text_rect,
        DT_LEFT | DT_VCENTER | DT_SINGLELINE | DT_NOPREFIX,
    );

    SelectObject(hdc, old_font);
    let _ = DeleteObject(font.into());

    // 밑줄 (미확정 표시)
    let underline = RECT {
        left: PAD_X,
        top: win_h - UNDERLINE_H - 1,
        right: win_w - PAD_X,
        bottom: win_h - 1,
    };
    let ul = CreateSolidBrush(COLOR_UNDERLINE);
    let _ = FillRect(hdc, &underline, ul);
    let _ = DeleteObject(ul.into());
}

// ─── 윈도우 프로시저 ────────────────────────────────────────────────────────

unsafe extern "system" fn preedit_wnd_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    match msg {
        WM_PAINT => {
            let state_ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut PreeditState;
            if !state_ptr.is_null() {
                let mut rc = RECT::default();
                let _ = GetClientRect(hwnd, &mut rc);
                let mut ps = PAINTSTRUCT::default();
                let hdc = BeginPaint(hwnd, &mut ps);
                if hdc != HDC::default() {
                    render(hdc, &*state_ptr, rc.right, rc.bottom);
                    let _ = EndPaint(hwnd, &ps);
                }
            }
            LRESULT(0)
        }
        WM_ERASEBKGND => LRESULT(1),
        _ => DefWindowProcW(hwnd, msg, wparam, lparam),
    }
}

// ─── 공개 API ───────────────────────────────────────────────────────────────

/// TSF STA 스레드에서 생성·갱신하는 client-side preedit 오버레이.
pub struct PreeditWindow {
    hwnd: HWND,
    state: Box<PreeditState>,
}

impl PreeditWindow {
    /// 창을 생성한다(아직 표시 안 함).
    pub fn create() -> windows::core::Result<Self> {
        unsafe {
            ensure_wnd_class();

            let hwnd = CreateWindowExW(
                WS_EX_TOPMOST | WS_EX_LAYERED | WS_EX_NOACTIVATE | WS_EX_TOOLWINDOW,
                WND_CLASS_NAME,
                w!(""),
                WS_POPUP,
                0,
                0,
                100,
                FONT_H + 2 * PAD_Y,
                None,
                None,
                Some(crate::dll_instance().into()),
                None,
            )?;

            SetLayeredWindowAttributes(hwnd, COLORREF(0), 255, LWA_ALPHA)?;

            let mut state = Box::new(PreeditState::default());
            SetWindowLongPtrW(
                hwnd,
                GWLP_USERDATA,
                state.as_mut() as *mut PreeditState as _,
            );

            Ok(Self { hwnd, state })
        }
    }

    /// preedit 텍스트를 표시·갱신한다. 빈 문자열이면 숨긴다.
    /// `pos`: 캐럿 스크린 좌표(텍스트 왼쪽 아래). None 이면 마우스 커서 근처.
    pub fn show(&mut self, text: &str, pos: Option<(i32, i32)>) {
        if text.is_empty() {
            self.hide();
            return;
        }

        self.state.visible = true;
        self.state.text = text.to_string();

        let tw = unsafe { measure_text_w(text) };
        let w = tw + 2 * PAD_X;
        let h = FONT_H + 2 * PAD_Y + UNDERLINE_H;

        let (x, y) = pos.unwrap_or_else(|| {
            let mut pt = POINT::default();
            unsafe {
                let _ = GetCursorPos(&mut pt);
            }
            (pt.x, pt.y)
        });

        unsafe {
            SetWindowPos(
                self.hwnd,
                Some(HWND_TOPMOST),
                x,
                y,
                w,
                h,
                SWP_SHOWWINDOW | SWP_NOACTIVATE,
            )
            .ok();
            let _ = InvalidateRect(Some(self.hwnd), None, true);
        }
    }

    /// 숨기고 상태 초기화.
    pub fn hide(&mut self) {
        if !self.state.visible {
            return;
        }
        self.state.visible = false;
        self.state.text.clear();
        unsafe {
            let _ = ShowWindow(self.hwnd, SW_HIDE);
        }
    }
}

impl Drop for PreeditWindow {
    fn drop(&mut self) {
        unsafe {
            SetWindowLongPtrW(self.hwnd, GWLP_USERDATA, 0);
            DestroyWindow(self.hwnd).ok();
        }
    }
}
