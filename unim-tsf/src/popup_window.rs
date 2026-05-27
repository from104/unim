//! TSF 팝업 윈도우 — 한자/특수문자/이모지 후보 격자 (GDI 렌더)
//!
//! Win32 layered NOACTIVATE 윈도우로 9×9 격자·행열 레이블·페이지 인디케이터·
//! 북마크★ flash를 렌더한다. unim-windows/ui/popup.rs 의 egui 버전과 동일한
//! PopupAction 소비 로직을 사용한다.
//!
//! ## 설계 결정
//! - `WS_EX_NOACTIVATE` 필수: 포커스 도난 → 타깃 앱 composition terminate 방지.
//! - GDI 만 사용 (Direct2D/D3D 불필요), features `Win32_Graphics_Gdi` 이미 활성.
//! - 윈도우 클래스는 최초 1회 등록 (`OnceLock<ATOM>`).
//! - 생성·파괴·메시지 처리는 모두 TSF STA 스레드에서 수행.
//!   호스트 앱 메시지 펌프가 WM_PAINT/WM_TIMER 를 돌리므로 별도 스레드 불필요.

use std::sync::OnceLock;

use windows::core::*;
use windows::Win32::Foundation::*;
use windows::Win32::Graphics::Gdi::*;
use windows::Win32::System::SystemInformation::GetTickCount64;
use windows::Win32::UI::WindowsAndMessaging::*;

use unim::input_engine::PopupAction;

// ─── 상수 ──────────────────────────────────────────────────────────────────

const WND_CLASS_NAME: PCWSTR = w!("UNIM_PopupWnd");

/// 셀 크기 (픽셀)
const CELL_W: i32 = 52;
const CELL_H: i32 = 28;
/// 행·열 레이블 열 너비 / 행 높이
const LABEL_W: i32 = 20;
const LABEL_H: i32 = 20;
/// 여백
const MARGIN: i32 = 6;
/// 페이지 인디케이터 영역 높이
const PAGE_BAR_H: i32 = 22;
/// flash 지속 시간 (ms)
const FLASH_MS: u32 = 140;

// ─── 클래스 등록 (1회) ─────────────────────────────────────────────────────

static WND_CLASS_ATOM: OnceLock<u16> = OnceLock::new();

/// 팝업 윈도우 클래스를 1회 등록한다. DLL 로드 후 첫 show() 시 호출.
unsafe fn ensure_wnd_class() {
    WND_CLASS_ATOM.get_or_init(|| {
        let wc = WNDCLASSEXW {
            cbSize: std::mem::size_of::<WNDCLASSEXW>() as u32,
            style: CS_HREDRAW | CS_VREDRAW,
            lpfnWndProc: Some(popup_wnd_proc),
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

// ─── 팝업 상태 ─────────────────────────────────────────────────────────────

/// `PopupWindow` 가 렌더에 사용하는 내부 상태.
struct PopupState {
    visible: bool,
    /// 후보 문자 (한자/특수/이모지 공통)
    candidates: Vec<String>,
    /// 한자 뜻풀이 (한자 팝업만)
    descriptions: Vec<String>,
    /// 한자 즐겨찾기 플래그 (한자 팝업만, candidates 와 1:1)
    bookmarks: Vec<bool>,
    page: usize,
    total_pages: usize,
    selected: usize,
    rows: usize,
    cols: usize,
    sel_row: usize,
    sel_col: usize,
    /// 한자 팝업이면 true (★ flash 판정용)
    is_hanja: bool,
    /// 북마크 ★ flash 종료 tick (GetTickCount64 기준), None=비활성
    flash_until_tick: Option<u64>,
    /// 컬럼 레이블 (Q~O 물리 위치, top_row)
    top_row: String,
}

impl Default for PopupState {
    fn default() -> Self {
        Self {
            visible: false,
            candidates: Vec::new(),
            descriptions: Vec::new(),
            bookmarks: Vec::new(),
            page: 0,
            total_pages: 1,
            selected: 0,
            rows: 3,
            cols: 3,
            sel_row: 0,
            sel_col: 0,
            is_hanja: false,
            flash_until_tick: None,
            top_row: String::new(),
        }
    }
}

impl PopupState {
    /// PopupAction 에 따라 내부 상태를 갱신한다.
    fn handle_action(&mut self, action: PopupAction) {
        match action {
            PopupAction::ShowHanja {
                target,
                candidates,
                top_row,
            } => {
                self.visible = true;
                self.candidates = candidates.iter().map(|(c, _)| c.clone()).collect();
                self.descriptions = candidates.iter().map(|(_, d)| d.clone()).collect();
                self.bookmarks = vec![false; candidates.len()];
                self.selected = 0;
                self.sel_row = 0;
                self.sel_col = 0;
                self.page = 0;
                self.is_hanja = true;
                self.top_row = top_row;
                self.flash_until_tick = None;
                let _ = target; // used in title (rendered via title field if needed)
            }
            PopupAction::ShowSpecial {
                target,
                characters,
                top_row,
            } => {
                self.visible = true;
                self.candidates = characters;
                self.descriptions = Vec::new();
                self.bookmarks = Vec::new();
                self.selected = 0;
                self.sel_row = 0;
                self.sel_col = 0;
                self.page = 0;
                self.is_hanja = false;
                self.top_row = top_row;
                self.flash_until_tick = None;
                let _ = target;
            }
            PopupAction::ShowEmoji {
                top_row, items, ..
            } => {
                self.visible = true;
                self.candidates = items;
                self.descriptions = Vec::new();
                self.bookmarks = Vec::new();
                self.selected = 0;
                self.sel_row = 0;
                self.sel_col = 0;
                self.page = 0;
                self.is_hanja = false;
                self.top_row = top_row;
                self.flash_until_tick = None;
            }
            PopupAction::HidePopup => {
                self.visible = false;
                self.candidates.clear();
                self.descriptions.clear();
                self.bookmarks.clear();
                self.is_hanja = false;
                self.flash_until_tick = None;
            }
            PopupAction::PopupNavigate {
                page,
                total_pages,
                selected,
                rows,
                cols,
                sel_row,
                sel_col,
            } => {
                self.page = page;
                self.total_pages = total_pages;
                self.selected = selected;
                self.rows = rows;
                self.cols = cols;
                self.sel_row = sel_row;
                self.sel_col = sel_col;
            }
            PopupAction::HanjaBookmarkChanged { index, bookmarked } => {
                if let Some(b) = self.bookmarks.get_mut(index) {
                    *b = bookmarked;
                }
            }
            PopupAction::HanjaCandidatesReordered {
                candidates,
                bookmarks,
                new_cursor,
                page,
                sel_row,
                sel_col,
                bookmarked,
                ..
            } => {
                self.candidates = candidates.iter().map(|(c, _)| c.clone()).collect();
                self.descriptions = candidates.iter().map(|(_, d)| d.clone()).collect();
                self.bookmarks = bookmarks;
                self.selected = new_cursor;
                self.page = page;
                self.sel_row = sel_row;
                self.sel_col = sel_col;
                // ★ 해제(bookmarked==false) 시 flash
                if !bookmarked {
                    let until = unsafe { GetTickCount64() } + FLASH_MS as u64;
                    self.flash_until_tick = Some(until);
                } else {
                    self.flash_until_tick = None;
                }
            }
            PopupAction::PageJump { page_index } => {
                self.page = page_index as usize;
            }
        }
    }

    fn flash_active(&self) -> bool {
        match self.flash_until_tick {
            Some(until) => {
                let now = unsafe { GetTickCount64() };
                now < until
            }
            None => false,
        }
    }

    /// 현재 페이지의 후보 슬라이스를 반환한다 (인덱스 범위 안전).
    fn page_candidates(&self) -> &[String] {
        let cols = self.cols.max(1);
        let per_page = self.rows * cols;
        if per_page == 0 {
            return &[];
        }
        let start = self.page * per_page;
        if start >= self.candidates.len() {
            &[]
        } else {
            let end = (start + per_page).min(self.candidates.len());
            &self.candidates[start..end]
        }
    }

    /// 창 전체 크기 (픽셀) 계산.
    fn window_size(&self) -> (i32, i32) {
        let cols = self.cols.max(1) as i32;
        let rows = self.rows.max(1) as i32;
        let w = MARGIN + LABEL_W + cols * CELL_W + MARGIN;
        let h = MARGIN + LABEL_H + rows * CELL_H + MARGIN + PAGE_BAR_H;
        (w, h)
    }
}

// ─── GDI 렌더 헬퍼 ─────────────────────────────────────────────────────────

/// `str` → UTF-16 Vec (NUL 미포함)
fn to_wide(s: &str) -> Vec<u16> {
    s.encode_utf16().collect()
}

/// 지정 색상 브러시로 RECT 를 채운다.
unsafe fn fill_rect_color(hdc: HDC, rect: &RECT, color: COLORREF) {
    let brush = CreateSolidBrush(color);
    let _ = FillRect(hdc, rect, brush);
    let _ = DeleteObject(brush);
}

/// 셀 RECT 계산 (격자 원점: (MARGIN + LABEL_W, MARGIN + LABEL_H)).
fn cell_rect(col: i32, row: i32) -> RECT {
    let x = MARGIN + LABEL_W + col * CELL_W;
    let y = MARGIN + LABEL_H + row * CELL_H;
    RECT {
        left: x,
        top: y,
        right: x + CELL_W,
        bottom: y + CELL_H,
    }
}

/// PopupState 를 HDC 에 렌더한다.
unsafe fn render(hdc: HDC, state: &PopupState) {
    if !state.visible || state.candidates.is_empty() {
        return;
    }

    let cols = state.cols.max(1) as i32;
    let rows = state.rows.max(1) as i32;
    let flash = state.flash_active();
    let page_cands = state.page_candidates();

    // ── 배경 ──
    let (w, h) = state.window_size();
    let bg_rect = RECT {
        left: 0,
        top: 0,
        right: w,
        bottom: h,
    };
    fill_rect_color(hdc, &bg_rect, COLORREF(0x00F5F5F5));

    // 기본 폰트 설정
    let font = CreateFontW(
        16,
        0,
        0,
        0,
        FW_NORMAL.0 as i32,
        0,
        0,
        0,
        DEFAULT_CHARSET.0 as u32,
        OUT_DEFAULT_PRECIS.0 as u32,
        CLIP_DEFAULT_PRECIS.0 as u32,
        CLEARTYPE_QUALITY.0 as u32,
        (DEFAULT_PITCH.0 | FF_DONTCARE.0) as u32,
        w!("맑은 고딕"),
    );
    let old_font = SelectObject(hdc, font);
    SetBkMode(hdc, TRANSPARENT);

    // ── 열 레이블 (top_row 문자: Q~O 물리 위치) ──
    let top_chars: Vec<char> = state.top_row.chars().collect();
    for col in 0..cols {
        let lx = MARGIN + LABEL_W + col * CELL_W;
        let mut label_rect = RECT {
            left: lx,
            top: MARGIN,
            right: lx + CELL_W,
            bottom: MARGIN + LABEL_H,
        };
        let col_selected = (col as usize) == state.sel_col;
        if col_selected {
            fill_rect_color(hdc, &label_rect, COLORREF(0x00FFDDAA)); // 연주황
            SetTextColor(hdc, COLORREF(0x00000000));
        } else {
            fill_rect_color(hdc, &label_rect, COLORREF(0x00DDDDDD));
            SetTextColor(hdc, COLORREF(0x00444444));
        }
        if let Some(&ch) = top_chars.get(col as usize) {
            let s = ch.to_string();
            let mut ws = to_wide(&s);
            DrawTextW(
                hdc,
                &mut ws,
                &mut label_rect,
                DT_CENTER | DT_VCENTER | DT_SINGLELINE,
            );
        }
    }

    // ── 행 레이블 (1~9) ──
    for row in 0..rows {
        let ly = MARGIN + LABEL_H + row * CELL_H;
        let mut label_rect = RECT {
            left: MARGIN,
            top: ly,
            right: MARGIN + LABEL_W,
            bottom: ly + CELL_H,
        };
        let row_selected = (row as usize) == state.sel_row;
        if row_selected {
            fill_rect_color(hdc, &label_rect, COLORREF(0x00FFDDAA));
            SetTextColor(hdc, COLORREF(0x00000000));
        } else {
            fill_rect_color(hdc, &label_rect, COLORREF(0x00DDDDDD));
            SetTextColor(hdc, COLORREF(0x00444444));
        }
        let num = format!("{}", row + 1);
        let mut ws = to_wide(&num);
        DrawTextW(
            hdc,
            &mut ws,
            &mut label_rect,
            DT_CENTER | DT_VCENTER | DT_SINGLELINE,
        );
    }

    // ── 셀 격자 ──
    let per_page = (rows * cols) as usize;
    for i in 0..per_page {
        let row = (i / cols as usize) as i32;
        let col = (i % cols as usize) as i32;
        let abs_idx = state.page * per_page + i;
        let mut rect = cell_rect(col, row);

        let is_selected = abs_idx == state.selected;
        let cand = page_cands.get(i);

        // 셀 배경
        // BGR 순서: Windows COLORREF 는 0x00BBGGRR
        let bg_color = if is_selected && state.is_hanja && flash {
            COLORREF(0x00AFE2F9) // ★ flash: 연노랑 (#F9E2AF in RGB → BGR 0xAFE2F9)
        } else if is_selected {
            COLORREF(0x00D77800) // 선택 강조: 파랑 (#0078D7 in RGB → BGR 0xD77800)
        } else if cand.is_some() {
            COLORREF(0x00FFFFFF) // 일반 셀: 흰색
        } else {
            COLORREF(0x00EEEEEE) // 빈 셀: 연회색
        };
        fill_rect_color(hdc, &rect, bg_color);

        // 셀 테두리
        let frame_brush = CreateSolidBrush(COLORREF(0x00CCCCCC));
        let _ = FrameRect(hdc, &rect, frame_brush);
        let _ = DeleteObject(frame_brush);

        // 텍스트
        if let Some(cand_str) = cand {
            if is_selected {
                SetTextColor(hdc, COLORREF(0x00FFFFFF));
            } else {
                SetTextColor(hdc, COLORREF(0x00222222));
            }

            // 한자 팝업: ★ 북마크 표시
            let bookmarked = state.is_hanja
                && state.bookmarks.get(abs_idx).copied().unwrap_or(false);
            let display = if bookmarked {
                format!("{}★", cand_str)
            } else {
                cand_str.clone()
            };
            let mut ws = to_wide(&display);
            rect.left += 2;
            rect.right -= 2;
            DrawTextW(
                hdc,
                &mut ws,
                &mut rect,
                DT_CENTER | DT_VCENTER | DT_SINGLELINE | DT_END_ELLIPSIS,
            );
        }
    }

    // ── 페이지 인디케이터 ──
    let page_y = h - PAGE_BAR_H;
    let mut page_rect = RECT {
        left: 0,
        top: page_y,
        right: w,
        bottom: h,
    };
    fill_rect_color(hdc, &page_rect, COLORREF(0x00E0E0E0));
    SetTextColor(hdc, COLORREF(0x00333333));
    let page_label = format!("{}/{}", state.page + 1, state.total_pages.max(1));
    let mut ws_page = to_wide(&page_label);
    DrawTextW(
        hdc,
        &mut ws_page,
        &mut page_rect,
        DT_CENTER | DT_VCENTER | DT_SINGLELINE,
    );

    // ── 정리 ──
    SelectObject(hdc, old_font);
    let _ = DeleteObject(font);
}

// ─── 윈도우 프로시저 ────────────────────────────────────────────────────────

unsafe extern "system" fn popup_wnd_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    match msg {
        WM_PAINT => {
            let state_ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut PopupState;
            if !state_ptr.is_null() {
                let mut ps = PAINTSTRUCT::default();
                let hdc = BeginPaint(hwnd, &mut ps);
                if hdc != HDC::default() {
                    render(hdc, &*state_ptr);
                    let _ = EndPaint(hwnd, &ps);
                }
            }
            LRESULT(0)
        }
        WM_TIMER => {
            // flash 타이머: 만료 시 다시 그려 flash 해제
            let state_ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut PopupState;
            if !state_ptr.is_null() {
                if !(*state_ptr).flash_active() {
                    KillTimer(hwnd, 1).ok();
                }
                let _ = InvalidateRect(hwnd, None, FALSE);
            }
            LRESULT(0)
        }
        WM_ERASEBKGND => LRESULT(1), // 배경 지우기 방지 (flickering 감소)
        WM_DESTROY => {
            PostQuitMessage(0);
            LRESULT(0)
        }
        _ => DefWindowProcW(hwnd, msg, wparam, lparam),
    }
}

// ─── PopupWindow 공개 API ───────────────────────────────────────────────────

/// TSF STA 스레드에서 생성·파괴·갱신하는 팝업 윈도우.
pub struct PopupWindow {
    hwnd: HWND,
    /// Box 로 힙에 고정. GWLP_USERDATA 에 raw pointer 저장.
    state: Box<PopupState>,
}

impl PopupWindow {
    /// 팝업 윈도우를 생성한다. 아직 표시하지 않음.
    pub fn create() -> windows::core::Result<Self> {
        unsafe {
            ensure_wnd_class();

            let hwnd = CreateWindowExW(
                WS_EX_TOPMOST | WS_EX_LAYERED | WS_EX_NOACTIVATE | WS_EX_TOOLWINDOW,
                WND_CLASS_NAME,
                w!(""),
                WS_POPUP | WS_BORDER,
                0,
                0,
                200,
                100,
                None,
                None,
                crate::dll_instance(),
                None,
            )?;

            // 레이어드 윈도우 불투명도 100%
            SetLayeredWindowAttributes(hwnd, COLORREF(0), 255, LWA_ALPHA)?;

            let mut state = Box::new(PopupState::default());
            // raw ptr 을 GWLP_USERDATA 에 저장 (윈도우 생존 기간 동안 유효)
            SetWindowLongPtrW(hwnd, GWLP_USERDATA, state.as_mut() as *mut PopupState as isize);

            Ok(Self { hwnd, state })
        }
    }

    /// 팝업 활성 여부
    pub fn is_active(&self) -> bool {
        self.state.visible
    }

    /// 초기 ShowHanja/ShowSpecial/ShowEmoji 액션으로 팝업을 표시한다.
    /// `pos`: 커서 아래 스크린 좌표. None 이면 마우스 커서 근처 fallback.
    pub fn show(&mut self, action: PopupAction, pos: Option<(i32, i32)>) {
        self.state.handle_action(action);

        if !self.state.visible {
            return;
        }

        let (w, h) = self.state.window_size();

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
                HWND_TOPMOST,
                x,
                y,
                w,
                h,
                SWP_SHOWWINDOW | SWP_NOACTIVATE,
            )
            .ok();
            let _ = InvalidateRect(self.hwnd, None, TRUE);
        }
    }

    /// 내비게이션/페이지/재정렬 등 후속 PopupAction 으로 상태를 갱신한다.
    /// HidePopup 수신 시 윈도우를 숨긴다.
    pub fn update(&mut self, action: PopupAction) {
        let was_flash = self.state.flash_active();
        self.state.handle_action(action);

        if !self.state.visible {
            // HidePopup 수신 → 윈도우 숨김
            unsafe {
                let _ = ShowWindow(self.hwnd, SW_HIDE);
                KillTimer(self.hwnd, 1).ok();
            }
            return;
        }

        // flash 시작 시 타이머 세트
        let now_flash = self.state.flash_active();
        if now_flash && !was_flash {
            unsafe {
                SetTimer(self.hwnd, 1, FLASH_MS + 10, None);
            }
        }

        let (w, h) = self.state.window_size();
        unsafe {
            SetWindowPos(
                self.hwnd,
                HWND_TOPMOST,
                0,
                0,
                w,
                h,
                SWP_NOMOVE | SWP_NOACTIVATE | SWP_NOZORDER,
            )
            .ok();
            let _ = InvalidateRect(self.hwnd, None, FALSE);
        }
    }

    /// 팝업을 숨기고 상태를 초기화한다.
    pub fn hide(&mut self) {
        self.state.visible = false;
        unsafe {
            let _ = ShowWindow(self.hwnd, SW_HIDE);
            KillTimer(self.hwnd, 1).ok();
        }
    }
}

impl Drop for PopupWindow {
    fn drop(&mut self) {
        unsafe {
            // GWLP_USERDATA null 화 (WM_PAINT race 방지)
            SetWindowLongPtrW(self.hwnd, GWLP_USERDATA, 0);
            DestroyWindow(self.hwnd).ok();
        }
    }
}
