//! 특수문자 팝업 윈도우 모듈
//!
//! X11 Xlib/Xft를 사용하여 특수문자 후보 목록을 9x9 그리드로 표시하는 팝업 윈도우입니다.
//! 키 처리 및 상태 관리는 `unim::popup::PopupState`에 위임하고, 이 모듈은 렌더링만 담당합니다.

use std::os::raw::{c_int, c_ulong};

use unim::popup::{PopupKey, PopupKeyResult, PopupState};
use unim::unim_log;

use crate::dpi;

/// 특수문자 팝업에서 키 처리 결과
pub enum SpecialAction {
    /// 인덱스로 문자 선택 (전체 인덱스, 0-based)
    Select(usize),
    /// 취소 (Esc)
    Cancel,
    /// 다음 페이지
    NextPage,
    /// 이전 페이지
    PrevPage,
    /// 팝업이 내부적으로 처리한 키
    Consumed,
    /// 처리하지 않음 → 팝업 닫고 키 바이패스
    None,
}

/// 특수문자 팝업 윈도우
pub struct SpecialWindow {
    /// X11 윈도우 ID
    window: c_ulong,
    /// XftDraw 컨텍스트
    xft_draw: *mut x11::xft::XftDraw,
    /// XftFont (메인 폰트)
    xft_font: *mut x11::xft::XftFont,
    /// 텍스트 색상 (기본)
    text_color: x11::xft::XftColor,
    /// 선택 배경 색상
    sel_bg_color: x11::xft::XftColor,
    /// 헤더 텍스트 색상 (#a6e3a1 Green)
    header_color: x11::xft::XftColor,
    /// 페이지 정보 색상
    page_color: x11::xft::XftColor,

    /// 통합 팝업 상태
    popup_state: Option<PopupState>,
    /// 윈도우 크기
    size: (u16, u16),
    /// 셀 크기
    cell_w: c_int,
    cell_h: c_int,
    /// DPI 스케일 팩터
    scale_factor: f64,
}

/// X11 keysym → PopupKey 변환
fn keysym_to_popup_key(keysym: u32) -> PopupKey {
    match keysym {
        0xff1b => PopupKey::Escape,
        0x71 => PopupKey::Letter(0), // q
        0x77 => PopupKey::Letter(1), // w
        0x65 => PopupKey::Letter(2), // e
        0x72 => PopupKey::Letter(3), // r
        0x74 => PopupKey::Letter(4), // t
        0x79 => PopupKey::Letter(5), // y
        0x75 => PopupKey::Letter(6), // u
        0x69 => PopupKey::Letter(7), // i
        0x6f => PopupKey::Letter(8), // o
        0x31..=0x39 => PopupKey::Number((keysym - 0x30) as u8),
        0xff0d => PopupKey::Enter,
        0xff52 => PopupKey::Up,
        0xff54 => PopupKey::Down,
        0xff51 => PopupKey::Left,
        0xff53 => PopupKey::Right,
        0xff09 => PopupKey::Tab,
        0xfe20 => PopupKey::ShiftTab,
        0x20 => PopupKey::Space,
        0xff55 => PopupKey::PageUp,
        0xff56 => PopupKey::PageDown,
        0xff08 => PopupKey::Backspace,
        0xffe1..=0xffee | 0xff7f | 0xff14 => PopupKey::Modifier,
        _ => PopupKey::Other,
    }
}

impl SpecialWindow {
    /// 특수문자 팝업 생성
    pub fn new(
        display: *mut x11::xlib::Display,
        screen: c_int,
        x: c_int,
        y: c_int,
    ) -> Result<Self, String> {
        let root = unsafe { x11::xlib::XRootWindow(display, screen) };
        let scale_factor = dpi::get_scale_factor(display, screen);

        let size: (u16, u16) = (dpi::scale_u16(400, scale_factor), dpi::scale_u16(350, scale_factor));

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
            final_y = y - (size.1 as c_int) - 20;
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

        // 색상 할당 — Catppuccin Mocha + Green 강조
        let mut text_color: x11::xft::XftColor = unsafe { std::mem::zeroed() };
        let mut sel_bg_color: x11::xft::XftColor = unsafe { std::mem::zeroed() };
        let mut header_color: x11::xft::XftColor = unsafe { std::mem::zeroed() };
        let mut page_color: x11::xft::XftColor = unsafe { std::mem::zeroed() };

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
            alloc(b"#a6e3a1\0", &mut header_color); // Green (특수문자 강조)
            alloc(b"#6c7086\0", &mut page_color); // Overlay0
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
            popup_state: None,
            size,
            cell_w: dpi::scale(30, scale_factor),
            cell_h: dpi::scale(30, scale_factor),
            scale_factor,
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

        // 셀 크기 계산
        let sf = self.scale_factor;
        self.cell_w = dpi::scale(30, sf);
        self.cell_h = dpi::scale(30, sf);

        let ps = self.popup_state.as_ref().unwrap();
        // 윈도우 크기 계산: 행 헤더(1열) + 데이터 열 + 여백, 열 헤더(1행) + 데이터 행 + 푸터
        let header_col_w = dpi::scale(30, sf);
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
            wy -= height + 20;
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

    /// 전체 인덱스로 문자 가져오기 (handler에서 직접 커밋용)
    pub fn get_character(&self, index: usize) -> Option<&str> {
        self.popup_state.as_ref()?.get_item(index)
    }

    /// 키 처리 — PopupState에 위임
    pub fn handle_key(&mut self, keysym: u32) -> SpecialAction {
        let popup_key = keysym_to_popup_key(keysym);
        let ps = match self.popup_state.as_mut() {
            Some(ps) => ps,
            None => return SpecialAction::None,
        };

        match ps.handle_key(popup_key) {
            PopupKeyResult::Select(idx) => SpecialAction::Select(idx),
            PopupKeyResult::Cancel => SpecialAction::Cancel,
            PopupKeyResult::Updated => {
                // Tab/ShiftTab/PageDown/PageUp/Space → 페이지 변경
                match popup_key {
                    PopupKey::Tab | PopupKey::PageDown | PopupKey::Space => {
                        SpecialAction::NextPage
                    }
                    PopupKey::ShiftTab | PopupKey::PageUp => SpecialAction::PrevPage,
                    _ => SpecialAction::Consumed,
                }
            }
            PopupKeyResult::Consumed => SpecialAction::Consumed,
            PopupKeyResult::NotHandled => SpecialAction::None,
        }
    }

    /// 마우스 클릭 처리
    pub fn handle_button_press(
        &mut self,
        button: u32,
        click_x: c_int,
        click_y: c_int,
        _display: *mut x11::xlib::Display,
    ) -> SpecialAction {
        let header_col_w = dpi::scale(30, self.scale_factor);
        let header_row_h = self.cell_h;

        let ps = match self.popup_state.as_mut() {
            Some(ps) => ps,
            None => return SpecialAction::Consumed,
        };

        match button {
            1 => {
                // 좌클릭 → 행/열 계산
                let col = (click_x - header_col_w) / self.cell_w;
                let row = (click_y - header_row_h) / self.cell_h;
                if col >= 0 && row >= 0 {
                    match ps.handle_click(row as usize, col as usize) {
                        PopupKeyResult::Select(idx) => SpecialAction::Select(idx),
                        _ => SpecialAction::Consumed,
                    }
                } else {
                    SpecialAction::Consumed
                }
            }
            3 => {
                // 우클릭 → 다음 페이지
                ps.handle_key(PopupKey::Tab);
                SpecialAction::NextPage
            }
            _ => SpecialAction::Consumed,
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
        let header_col_w: c_int = dpi::scale(30, sf);
        let header_row_h = self.cell_h;
        let top_row_chars: Vec<char> = ps.top_row().chars().collect();
        let rows = ps.rows();
        let cols = ps.cols();
        let sel_row = ps.sel_row();
        let sel_col = ps.sel_col();
        let text_nudge = dpi::scale(4, sf);
        let text_margin = dpi::scale(6, sf);

        // 1. 열 헤더 (top_row 레이블)
        for c in 0..cols {
            let label = if c < top_row_chars.len() {
                top_row_chars[c].to_string()
            } else {
                format!("{}", c + 1)
            };
            let color = if c == sel_col {
                &self.sel_bg_color
            } else {
                &self.header_color
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

        // 2. 행 헤더 (숫자 1-9)
        for r in 0..rows {
            let label = format!("{}", r + 1);
            let color = if r == sel_row {
                &self.sel_bg_color
            } else {
                &self.header_color
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

        // 4. 푸터 (대상 + 페이지 정보)
        let footer_y = header_row_h + (rows as c_int) * self.cell_h + dpi::scale(18, sf);
        let footer_text = format!(
            "[{}]  {} / {}",
            ps.target(),
            ps.current_page() + 1,
            ps.total_pages()
        );
        let fb = footer_text.as_bytes();
        unsafe {
            x11::xft::XftDrawStringUtf8(
                self.xft_draw,
                &self.page_color,
                self.xft_font,
                text_margin,
                footer_y,
                fb.as_ptr(),
                fb.len() as c_int,
            );
            x11::xlib::XFlush(display);
        }
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
