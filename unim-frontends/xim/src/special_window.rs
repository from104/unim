//! 특수문자 팝업 윈도우 모듈
//!
//! X11 Xlib/Xft를 사용하여 특수문자 후보 목록을 9x9 그리드로 표시하는 팝업 윈도우입니다.
//! 키 처리 및 상태 관리는 `unim::popup::PopupState`에 위임하고, 이 모듈은 렌더링만 담당합니다.

use std::os::raw::{c_int, c_ulong};

use unim::popup::PopupState;
use unim::unim_log;

use crate::dpi;

/// 마우스 페이지 이동 글리프 (Phase 9)
const ICON_PREV_PAGE: &str = "◀";
const ICON_NEXT_PAGE: &str = "▶";

/// 특수문자 팝업 마우스 클릭 결과
pub enum SpecialClickResult {
    /// 셀 선택 (행, 열)
    Select(usize, usize),
    /// 다음 페이지 (우클릭 또는 ▶ 좌클릭)
    NextPage,
    /// 이전 페이지 (◀ 좌클릭, Phase 9)
    PrevPage,
    /// 이벤트 소비됨
    Consumed,
}

/// 점 (x, y) 가 (x0, y0, x1, y1) 사각형 내부인지 판정.
/// (0, 0, 0, 0) 은 비활성 영역 — 항상 false.
fn hit_rect(x: c_int, y: c_int, rect: (c_int, c_int, c_int, c_int)) -> bool {
    let (x0, y0, x1, y1) = rect;
    if x0 == 0 && y0 == 0 && x1 == 0 && y1 == 0 {
        return false;
    }
    x >= x0 && x < x1 && y >= y0 && y < y1
}

/// 특수문자 팝업 윈도우
pub struct SpecialWindow {
    /// X11 윈도우 ID
    window: c_ulong,
    /// XftDraw 컨텍스트
    xft_draw: *mut x11::xft::XftDraw,
    /// XftFont (메인 폰트)
    xft_font: *mut x11::xft::XftFont,
    /// 텍스트 색상 (#cdd6f4)
    text_color: x11::xft::XftColor,
    /// 선택 배경 색상 (#404f4b — rgba(166,227,161,0.25) on #1e1e2e)
    sel_bg_color: x11::xft::XftColor,
    /// 헤더/활성 열 헤더 색상 (#a6e3a1 Green)
    header_color: x11::xft::XftColor,
    /// 페이지 정보 색상 (#6c7086)
    page_color: x11::xft::XftColor,
    /// 행 번호 색상 (#7f849c Overlay1)
    number_color: x11::xft::XftColor,
    /// 비활성 열 헤더 색상 (#f9e2af Yellow)
    yellow_color: x11::xft::XftColor,
    /// 헤더 배경 색상 (#313244 Surface0)
    header_bg_color: x11::xft::XftColor,
    /// Flash 효과 색상 (#526354 — 선택+밝기 부스트)
    flash_color: x11::xft::XftColor,

    /// 통합 팝업 상태
    popup_state: Option<PopupState>,
    /// 윈도우 크기
    size: (u16, u16),
    /// 셀 크기
    cell_w: c_int,
    cell_h: c_int,
    /// DPI 스케일 팩터
    scale_factor: f64,
    /// ◀ 버튼 영역 (푸터 좌측, 페이지 라벨 옆). (x0, y0, x1, y1).
    /// (0,0,0,0) 이면 비활성 (단일 페이지). Phase 9.
    prev_btn_rect: (c_int, c_int, c_int, c_int),
    /// ▶ 버튼 영역 (푸터 우측, 페이지 라벨 옆). 위와 동일.
    next_btn_rect: (c_int, c_int, c_int, c_int),
}

impl SpecialWindow {
    /// 폰트 메트릭 기반 셀 크기 계산
    fn compute_cell_size(
        display: *mut x11::xlib::Display,
        xft_font: *mut x11::xft::XftFont,
        scale_factor: f64,
    ) -> (c_int, c_int) {
        let test = "가";
        let test_bytes = test.as_bytes();
        unsafe {
            let mut extents: x11::xrender::XGlyphInfo = std::mem::zeroed();
            x11::xft::XftTextExtentsUtf8(
                display,
                xft_font,
                test_bytes.as_ptr(),
                test_bytes.len() as c_int,
                &mut extents,
            );
            let padding = dpi::scale(6, scale_factor);
            let w = (extents.xOff as c_int).max(dpi::scale(16, scale_factor)) + padding;
            let h = (extents.height as c_int).max(dpi::scale(14, scale_factor)) + padding;
            (w, h)
        }
    }

    /// 특수문자 팝업 생성
    pub fn new(
        display: *mut x11::xlib::Display,
        screen: c_int,
        x: c_int,
        y: c_int,
    ) -> Result<Self, String> {
        let root = unsafe { x11::xlib::XRootWindow(display, screen) };
        let scale_factor = dpi::get_scale_factor(display, screen);

        let size: (u16, u16) = (
            dpi::scale_u16(400, scale_factor),
            dpi::scale_u16(350, scale_factor),
        );

        // 화면 경계 보정
        let screen_w = unsafe { x11::xlib::XDisplayWidth(display, screen) };
        let screen_h = unsafe { x11::xlib::XDisplayHeight(display, screen) };
        let mut final_x = x;
        let mut final_y = y;
        if final_x + (size.0 as c_int) > screen_w {
            final_x = screen_w - (size.0 as c_int);
            if final_x < 0 {
                final_x = 0;
            }
        }
        if final_y + (size.1 as c_int) > screen_h {
            final_y = y - (size.1 as c_int) - 4; // POPUP_SPEC 6.2: 4px gap
            if final_y < 0 {
                final_y = 0;
            }
        }

        let mut swa: x11::xlib::XSetWindowAttributes = unsafe { std::mem::zeroed() };
        // Catppuccin Mocha Base: #1e1e2e
        let bg_pixel = unsafe {
            let colormap = x11::xlib::XDefaultColormap(display, screen);
            let mut xcolor: x11::xlib::XColor = std::mem::zeroed();
            xcolor.red = 30 * 257;
            xcolor.green = 30 * 257;
            xcolor.blue = 46 * 257;
            xcolor.flags = 7;
            x11::xlib::XAllocColor(display, colormap, &mut xcolor);
            xcolor.pixel
        };
        let border_pixel = unsafe {
            let colormap = x11::xlib::XDefaultColormap(display, screen);
            let mut xcolor: x11::xlib::XColor = std::mem::zeroed();
            xcolor.red = 69 * 257;
            xcolor.green = 71 * 257;
            xcolor.blue = 104 * 257;
            xcolor.flags = 7;
            x11::xlib::XAllocColor(display, colormap, &mut xcolor);
            xcolor.pixel
        };
        swa.background_pixel = bg_pixel;
        swa.border_pixel = border_pixel;
        swa.override_redirect = x11::xlib::True;
        swa.event_mask =
            x11::xlib::ExposureMask | x11::xlib::StructureNotifyMask | x11::xlib::ButtonPressMask;

        let window = unsafe {
            x11::xlib::XCreateWindow(
                display,
                root,
                final_x,
                final_y,
                size.0 as u32,
                size.1 as u32,
                1,
                x11::xlib::CopyFromParent,
                x11::xlib::InputOutput as u32,
                std::ptr::null_mut(),
                x11::xlib::CWBackPixel
                    | x11::xlib::CWBorderPixel
                    | x11::xlib::CWOverrideRedirect
                    | x11::xlib::CWEventMask,
                &mut swa,
            )
        };
        if window == 0 {
            return Err("XCreateWindow failed".to_string());
        }

        // _NET_WM_WINDOW_TYPE 설정
        unsafe {
            let wt_atom = x11::xlib::XInternAtom(
                display,
                b"_NET_WM_WINDOW_TYPE\0".as_ptr() as *const i8,
                x11::xlib::False,
            );
            let popup_atom = x11::xlib::XInternAtom(
                display,
                b"_NET_WM_WINDOW_TYPE_POPUP_MENU\0".as_ptr() as *const i8,
                x11::xlib::False,
            );
            let xa_atom = 4u64;
            x11::xlib::XChangeProperty(
                display,
                window,
                wt_atom,
                xa_atom,
                32,
                x11::xlib::PropModeReplace,
                &popup_atom as *const u64 as *const u8,
                1,
            );
        }

        // XftDraw 생성
        let visual = unsafe { x11::xlib::XDefaultVisual(display, screen) };
        let colormap = unsafe { x11::xlib::XDefaultColormap(display, screen) };
        let xft_draw = unsafe { x11::xft::XftDrawCreate(display, window, visual, colormap) };
        if xft_draw.is_null() {
            unsafe {
                x11::xlib::XDestroyWindow(display, window);
            }
            return Err("XftDrawCreate failed".to_string());
        }

        // 폰트 로드
        let xft_font = unsafe {
            let font = x11::xft::XftFontOpenName(
                display,
                screen,
                b"D2Coding:size=14\0".as_ptr() as *const i8,
            );
            if font.is_null() {
                x11::xft::XftFontOpenName(
                    display,
                    screen,
                    b"monospace:size=14\0".as_ptr() as *const i8,
                )
            } else {
                font
            }
        };
        if xft_font.is_null() {
            unsafe {
                x11::xft::XftDrawDestroy(xft_draw);
                x11::xlib::XDestroyWindow(display, window);
            }
            return Err("XftFontOpenName failed".to_string());
        }

        // 색상 할당 — Catppuccin Mocha 풀 팔레트
        let mut text_color: x11::xft::XftColor = unsafe { std::mem::zeroed() };
        let mut sel_bg_color: x11::xft::XftColor = unsafe { std::mem::zeroed() };
        let mut header_color: x11::xft::XftColor = unsafe { std::mem::zeroed() };
        let mut page_color: x11::xft::XftColor = unsafe { std::mem::zeroed() };
        let mut number_color: x11::xft::XftColor = unsafe { std::mem::zeroed() };
        let mut yellow_color: x11::xft::XftColor = unsafe { std::mem::zeroed() };
        let mut header_bg_color: x11::xft::XftColor = unsafe { std::mem::zeroed() };
        let mut flash_color: x11::xft::XftColor = unsafe { std::mem::zeroed() };

        unsafe {
            let alloc = |name: &[u8], color: &mut x11::xft::XftColor| {
                x11::xft::XftColorAllocName(
                    display,
                    visual,
                    colormap,
                    name.as_ptr() as *const i8,
                    color,
                );
            };
            alloc(b"#cdd6f4\0", &mut text_color); // Text
            alloc(b"#404f4b\0", &mut sel_bg_color); // rgba(166,227,161,0.25) on #1e1e2e
            alloc(b"#a6e3a1\0", &mut header_color); // Green (활성 열 헤더)
            alloc(b"#6c7086\0", &mut page_color); // Overlay0
            alloc(b"#7f849c\0", &mut number_color); // Overlay1 (행 번호)
            alloc(b"#f9e2af\0", &mut yellow_color); // Yellow (비활성 열 헤더)
            alloc(b"#313244\0", &mut header_bg_color); // Surface0 (헤더 배경)
            alloc(b"#526354\0", &mut flash_color); // Flash (선택 피드백)
        }

        unim_log!(
            "XIM_SPECIAL",
            "특수문자 팝업 생성: pos=({},{}), scale={:.2}",
            final_x,
            final_y,
            scale_factor
        );

        Ok(Self {
            window,
            xft_draw,
            xft_font,
            text_color,
            sel_bg_color,
            header_color,
            page_color,
            number_color,
            yellow_color,
            header_bg_color,
            flash_color,
            popup_state: None,
            size,
            cell_w: 0,
            cell_h: 0,
            scale_factor,
            prev_btn_rect: (0, 0, 0, 0),
            next_btn_rect: (0, 0, 0, 0),
        })
    }

    /// 윈도우 ID
    pub fn window_id(&self) -> u32 {
        self.window as u32
    }

    /// 윈도우 크기
    pub fn size(&self) -> (u16, u16) {
        self.size
    }

    /// 특수문자 설정 및 표시
    pub fn set_characters(
        &mut self,
        display: *mut x11::xlib::Display,
        screen: c_int,
        target: &str,
        characters: Vec<String>,
        top_row: &str,
    ) {
        self.popup_state = Some(PopupState::new_special(target, characters, top_row));

        // 셀 크기 계산 (폰트 메트릭 기반)
        let sf = self.scale_factor;
        let (cell_w, cell_h) = Self::compute_cell_size(display, self.xft_font, sf);
        self.cell_w = cell_w;
        self.cell_h = cell_h;

        let ps = self.popup_state.as_ref().unwrap();
        // 윈도우 크기 계산: 행 헤더(1열) + 데이터 열 + 여백, 열 헤더(1행) + 데이터 행 + 푸터
        let header_col_w = self.cell_w;
        let header_row_h = self.cell_h;
        let footer_h = dpi::scale(24, sf);
        let margin = dpi::scale(10, sf);
        let width = header_col_w + (ps.cols() as c_int) * self.cell_w + margin;
        let height = header_row_h + (ps.rows() as c_int) * self.cell_h + footer_h + margin;

        self.size = (width as u16, height as u16);
        unsafe {
            x11::xlib::XResizeWindow(display, self.window, width as u32, height as u32);
        }

        // 화면 경계 재검사
        let screen_w = unsafe { x11::xlib::XDisplayWidth(display, screen) };
        let screen_h = unsafe { x11::xlib::XDisplayHeight(display, screen) };
        let mut attrs: x11::xlib::XWindowAttributes = unsafe { std::mem::zeroed() };
        unsafe {
            x11::xlib::XGetWindowAttributes(display, self.window, &mut attrs);
        }
        let mut wx = attrs.x;
        let mut wy = attrs.y;
        if wx + width > screen_w {
            wx = screen_w - width;
        }
        if wy + height > screen_h {
            wy -= height + 4;
            if wy < 0 {
                wy = 0;
            }
        }
        unsafe {
            x11::xlib::XMoveWindow(display, self.window, wx, wy);
        }

        // 배경색 설정 (Catppuccin Mocha 배경)
        unsafe {
            let visual = x11::xlib::XDefaultVisual(display, screen);
            let colormap = x11::xlib::XDefaultColormap(display, screen);
            let mut bg_color: x11::xft::XftColor = std::mem::zeroed();
            x11::xft::XftColorAllocName(
                display,
                visual,
                colormap,
                b"#1e1e2e\0".as_ptr() as *const i8,
                &mut bg_color,
            );
            x11::xlib::XSetWindowBackground(display, self.window, bg_color.pixel);
            x11::xft::XftColorFree(display, visual, colormap, &bg_color as *const _ as *mut _);
        }

        unsafe {
            x11::xlib::XMapRaised(display, self.window);
            x11::xlib::XFlush(display);
        }

        self.redraw(display);
    }

    /// 전체 인덱스로 문자 가져오기
    #[allow(dead_code)]
    pub fn get_character(&self, index: usize) -> Option<&str> {
        self.popup_state.as_ref()?.get_item(index)
    }

    /// 엔진의 PopupNavigate 시그널로 상태 갱신 + 다시 그리기
    pub fn update_from_navigate(
        &mut self,
        page: usize,
        sel_row: usize,
        sel_col: usize,
        display: *mut x11::xlib::Display,
    ) {
        if let Some(ps) = self.popup_state.as_mut() {
            ps.set_navigate_state(page, sel_row, sel_col);
        }
        self.redraw(display);
    }

    /// 마우스 클릭 처리 — 셀 좌표 계산만 수행 (키 처리는 엔진에 위임)
    pub fn handle_button_press(
        &self,
        button: u32,
        click_x: c_int,
        click_y: c_int,
    ) -> SpecialClickResult {
        let header_col_w = self.cell_w;
        let header_row_h = self.cell_h;

        if self.popup_state.is_none() {
            return SpecialClickResult::Consumed;
        }

        match button {
            1 => {
                // Phase 9: 푸터 ◀/▶ 영역 우선 hit-test (단일 페이지면 비활성).
                if hit_rect(click_x, click_y, self.prev_btn_rect) {
                    unim_log!("XIM_SPECIAL", "좌클릭 ◀ → 이전 페이지 요청");
                    return SpecialClickResult::PrevPage;
                }
                if hit_rect(click_x, click_y, self.next_btn_rect) {
                    unim_log!("XIM_SPECIAL", "좌클릭 ▶ → 다음 페이지 요청");
                    return SpecialClickResult::NextPage;
                }
                // 좌클릭 → 행/열 계산
                let col = (click_x - header_col_w) / self.cell_w;
                let row = (click_y - header_row_h) / self.cell_h;
                let ps = self.popup_state.as_ref().unwrap();
                if col >= 0 && row >= 0 && (row as usize) < ps.rows() && (col as usize) < ps.cols()
                {
                    unim_log!("XIM_SPECIAL", "좌클릭 선택: row={}, col={}", row, col);
                    SpecialClickResult::Select(row as usize, col as usize)
                } else {
                    SpecialClickResult::Consumed
                }
            }
            3 => {
                // 우클릭 → 다음 페이지 요청
                unim_log!("XIM_SPECIAL", "우클릭 → 다음 페이지 요청");
                SpecialClickResult::NextPage
            }
            _ => SpecialClickResult::Consumed,
        }
    }

    /// 다시 그리기 — PopupState에서 데이터를 읽어 렌더링
    pub fn redraw(&mut self, display: *mut x11::xlib::Display) {
        if display.is_null() {
            return;
        }

        let ps = match self.popup_state.as_ref() {
            Some(ps) => ps,
            None => return,
        };

        unsafe {
            x11::xlib::XClearWindow(display, self.window);
        }

        let sf = self.scale_factor;
        let header_col_w: c_int = self.cell_w;
        let header_row_h = self.cell_h;
        let top_row_chars: Vec<char> = ps.top_row().chars().collect();
        let rows = ps.rows();
        let cols = ps.cols();
        let sel_row = ps.sel_row();
        let sel_col = ps.sel_col();
        let text_nudge = dpi::scale(4, sf);
        let text_margin = dpi::scale(6, sf);

        // 0. 헤더 배경 (Surface0 #313244) — 열 헤더 행 전체
        unsafe {
            let gc = x11::xlib::XCreateGC(display, self.window, 0, std::ptr::null_mut());
            x11::xlib::XSetForeground(display, gc, self.header_bg_color.pixel);
            x11::xlib::XFillRectangle(
                display,
                self.window,
                gc,
                0,
                0,
                self.size.0 as u32,
                header_row_h as u32,
            );
            x11::xlib::XFreeGC(display, gc);
        }

        // 1. 열 헤더 (top_row 레이블) — 활성=Green, 비활성=Yellow
        for c in 0..cols {
            let label = if c < top_row_chars.len() {
                top_row_chars[c].to_string()
            } else {
                format!("{}", c + 1)
            };
            let color = if c == sel_col {
                &self.header_color // 활성: Green #a6e3a1
            } else {
                &self.yellow_color // 비활성: Yellow #f9e2af
            };
            let x = header_col_w + (c as c_int) * self.cell_w + self.cell_w / 2 - text_nudge;
            let y = header_row_h - text_margin;
            let bytes = label.as_bytes();
            unsafe {
                x11::xft::XftDrawStringUtf8(
                    self.xft_draw,
                    color,
                    self.xft_font,
                    x,
                    y,
                    bytes.as_ptr(),
                    bytes.len() as c_int,
                );
            }
        }

        // 2. 행 헤더 (숫자 1-9) — 활성=Green, 비활성=Overlay1
        for r in 0..rows {
            let label = format!("{}", r + 1);
            let color = if r == sel_row {
                &self.header_color // 활성: Green #a6e3a1
            } else {
                &self.number_color // 비활성: Overlay1 #7f849c
            };
            let x = text_margin;
            let y = header_row_h + (r as c_int) * self.cell_h + self.cell_h - text_margin;
            let bytes = label.as_bytes();
            unsafe {
                x11::xft::XftDrawStringUtf8(
                    self.xft_draw,
                    color,
                    self.xft_font,
                    x,
                    y,
                    bytes.as_ptr(),
                    bytes.len() as c_int,
                );
            }
        }

        // 3. 그리드 셀
        for c in 0..cols {
            for r in 0..rows {
                let cell_x = header_col_w + (c as c_int) * self.cell_w;
                let cell_y = header_row_h + (r as c_int) * self.cell_h;

                let is_selected = r == sel_row && c == sel_col;

                if let Some(ch) = ps.cell_text(r, c) {
                    // 선택 셀 배경
                    if is_selected {
                        unsafe {
                            let gc =
                                x11::xlib::XCreateGC(display, self.window, 0, std::ptr::null_mut());
                            x11::xlib::XSetForeground(display, gc, self.sel_bg_color.pixel);
                            x11::xlib::XFillRectangle(
                                display,
                                self.window,
                                gc,
                                cell_x,
                                cell_y,
                                self.cell_w as u32,
                                self.cell_h as u32,
                            );
                            x11::xlib::XFreeGC(display, gc);
                        }
                    }

                    let color = &self.text_color;
                    let bytes = ch.as_bytes();
                    // 셀 내 텍스트 가운데 정렬
                    let tx = cell_x + self.cell_w / 2 - text_margin;
                    let ty = cell_y + self.cell_h - text_margin;
                    unsafe {
                        x11::xft::XftDrawStringUtf8(
                            self.xft_draw,
                            color,
                            self.xft_font,
                            tx,
                            ty,
                            bytes.as_ptr(),
                            bytes.len() as c_int,
                        );
                    }
                }
            }
        }

        // 4. 푸터 (대상 + 페이지 정보 + ◀/▶ 마우스 페이지 버튼)
        let footer_y = header_row_h + (rows as c_int) * self.cell_h + dpi::scale(18, sf);
        let total_pages = ps.total_pages();
        let multi = total_pages > 1;
        // 좌측: [target]  텍스트
        let target_text = format!("[{}]", ps.target());
        let target_bytes = target_text.as_bytes();
        unsafe {
            x11::xft::XftDrawStringUtf8(
                self.xft_draw,
                &self.page_color,
                self.xft_font,
                text_margin,
                footer_y,
                target_bytes.as_ptr(),
                target_bytes.len() as c_int,
            );
        }

        // 우측 정렬: ◀  n/N  ▶
        let page_str = format!("{} / {}", ps.current_page() + 1, total_pages);
        let line_h = self.cell_h;
        unsafe {
            let mut page_ext: x11::xrender::XGlyphInfo = std::mem::zeroed();
            x11::xft::XftTextExtentsUtf8(
                display,
                self.xft_font,
                page_str.as_bytes().as_ptr(),
                page_str.as_bytes().len() as c_int,
                &mut page_ext,
            );
            let next_bytes = ICON_NEXT_PAGE.as_bytes();
            let mut next_ext: x11::xrender::XGlyphInfo = std::mem::zeroed();
            x11::xft::XftTextExtentsUtf8(
                display,
                self.xft_font,
                next_bytes.as_ptr(),
                next_bytes.len() as c_int,
                &mut next_ext,
            );
            let prev_bytes = ICON_PREV_PAGE.as_bytes();
            let mut prev_ext: x11::xrender::XGlyphInfo = std::mem::zeroed();
            x11::xft::XftTextExtentsUtf8(
                display,
                self.xft_font,
                prev_bytes.as_ptr(),
                prev_bytes.len() as c_int,
                &mut prev_ext,
            );

            let gap = dpi::scale(8, sf);
            let right_edge = (self.size.0 as c_int) - text_margin;
            // 우→좌: ▶ → 페이지 → ◀
            let next_x = right_edge - next_ext.xOff as c_int;
            let page_x = next_x - gap - page_ext.xOff as c_int;
            let prev_x = page_x - gap - prev_ext.xOff as c_int;

            // hit-target ≥28px (WCAG 2.5.5 초과). 글리프는 그대로, rect만 확장.
            let min_hit = dpi::scale(28, sf);
            let expand_rect = |cx: c_int, cy_top: c_int, cy_bot: c_int, glyph_w: c_int| -> (c_int, c_int, c_int, c_int) {
                let cur_w = glyph_w + gap;
                let extra_w = (min_hit - cur_w).max(0) / 2;
                let cur_h = cy_bot - cy_top;
                let extra_h = (min_hit - cur_h).max(0) / 2;
                (
                    cx - gap / 2 - extra_w,
                    cy_top - extra_h,
                    cx + glyph_w + gap / 2 + extra_w,
                    cy_bot + extra_h,
                )
            };
            if multi {
                x11::xft::XftDrawStringUtf8(
                    self.xft_draw,
                    &self.page_color,
                    self.xft_font,
                    prev_x,
                    footer_y,
                    prev_bytes.as_ptr(),
                    prev_bytes.len() as c_int,
                );
                self.prev_btn_rect = expand_rect(
                    prev_x,
                    footer_y - line_h + dpi::scale(2, sf),
                    footer_y + dpi::scale(4, sf),
                    prev_ext.xOff as c_int,
                );
            } else {
                self.prev_btn_rect = (0, 0, 0, 0);
            }

            // 페이지 번호 (항상 표시)
            x11::xft::XftDrawStringUtf8(
                self.xft_draw,
                &self.page_color,
                self.xft_font,
                page_x,
                footer_y,
                page_str.as_bytes().as_ptr(),
                page_str.as_bytes().len() as c_int,
            );

            if multi {
                x11::xft::XftDrawStringUtf8(
                    self.xft_draw,
                    &self.page_color,
                    self.xft_font,
                    next_x,
                    footer_y,
                    next_bytes.as_ptr(),
                    next_bytes.len() as c_int,
                );
                self.next_btn_rect = expand_rect(
                    next_x,
                    footer_y - line_h + dpi::scale(2, sf),
                    footer_y + dpi::scale(4, sf),
                    next_ext.xOff as c_int,
                );
            } else {
                self.next_btn_rect = (0, 0, 0, 0);
            }

            x11::xlib::XFlush(display);
        }
    }

    /// Flash 효과: 선택 셀을 밝은 색으로 120ms 표시 후 반환
    pub fn flash_selection(&mut self, display: *mut x11::xlib::Display) {
        let ps = match self.popup_state.as_ref() {
            Some(ps) => ps,
            None => return,
        };

        let sel_row = ps.sel_row();
        let sel_col = ps.sel_col();
        let header_col_w = self.cell_w;
        let header_row_h = self.cell_h;
        let cell_x = header_col_w + (sel_col as c_int) * self.cell_w;
        let cell_y = header_row_h + (sel_row as c_int) * self.cell_h;

        // Flash 색상으로 셀 배경 렌더링
        unsafe {
            let gc = x11::xlib::XCreateGC(display, self.window, 0, std::ptr::null_mut());
            x11::xlib::XSetForeground(display, gc, self.flash_color.pixel);
            x11::xlib::XFillRectangle(
                display,
                self.window,
                gc,
                cell_x,
                cell_y,
                self.cell_w as u32,
                self.cell_h as u32,
            );
            x11::xlib::XFreeGC(display, gc);
        }

        // 셀 텍스트 재렌더링 (flash 배경 위에)
        if let Some(ch) = ps.cell_text(sel_row, sel_col) {
            let sf = self.scale_factor;
            let text_margin = dpi::scale(6, sf);
            let tx = cell_x + self.cell_w / 2 - dpi::scale(4, sf);
            let ty = cell_y + self.cell_h - text_margin;
            let bytes = ch.as_bytes();
            unsafe {
                x11::xft::XftDrawStringUtf8(
                    self.xft_draw,
                    &self.text_color,
                    self.xft_font,
                    tx,
                    ty,
                    bytes.as_ptr(),
                    bytes.len() as c_int,
                );
            }
        }

        unsafe {
            x11::xlib::XFlush(display);
        }

        // 120ms 대기 (POPUP_SPEC flash duration)
        std::thread::sleep(std::time::Duration::from_millis(120));
    }

    /// Expose 이벤트 처리
    pub fn expose(&mut self, display: *mut x11::xlib::Display) {
        self.redraw(display);
    }

    /// 윈도우 정리
    pub fn clean(self, display: *mut x11::xlib::Display, screen: c_int) {
        if display.is_null() {
            return;
        }
        unsafe {
            let old_handler = x11::xlib::XSetErrorHandler(Some(dummy_error_handler));
            let visual = x11::xlib::XDefaultVisual(display, screen);
            let colormap = x11::xlib::XDefaultColormap(display, screen);

            let colors: &[&x11::xft::XftColor] = &[
                &self.text_color,
                &self.sel_bg_color,
                &self.header_color,
                &self.page_color,
                &self.number_color,
                &self.yellow_color,
                &self.header_bg_color,
                &self.flash_color,
            ];
            for color in colors {
                x11::xft::XftColorFree(display, visual, colormap, *color as *const _ as *mut _);
            }
            x11::xft::XftFontClose(display, self.xft_font);
            x11::xft::XftDrawDestroy(self.xft_draw);
            x11::xlib::XDestroyWindow(display, self.window);
            x11::xlib::XFlush(display);
            x11::xlib::XSetErrorHandler(old_handler);
        }
        unim_log!("XIM_SPECIAL", "특수문자 팝업 정리됨");
    }
}

/// X11 에러 무시용 더미 핸들러
unsafe extern "C" fn dummy_error_handler(
    _display: *mut x11::xlib::Display,
    _event: *mut x11::xlib::XErrorEvent,
) -> c_int {
    0
}
