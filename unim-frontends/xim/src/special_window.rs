//! 특수문자 팝업 윈도우 모듈
//!
//! X11 Xlib/Xft를 사용하여 특수문자 후보 목록을 9x9 그리드로 표시하는 팝업 윈도우입니다.
//! HanjaWindow와 동일한 패턴으로 override_redirect 윈도우를 사용합니다.

use std::os::raw::{c_int, c_ulong};

use unim::unim_log;

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

/// 그리드 상수 (GTK 특수문자 팝업과 동일)
const MAX_ROWS: usize = 9;
const MAX_COLS: usize = 9;
const PAGE_SIZE: usize = MAX_ROWS * MAX_COLS; // 81

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

    /// 전체 특수문자 배열
    characters: Vec<String>,
    /// 변환 대상 초성
    target: String,
    /// 상단 행 레이블 (예: "QWERTYUIO")
    top_row: String,
    /// 현재 페이지 (0-based)
    current_page: usize,
    /// 전체 페이지 수
    total_pages: usize,
    /// 현재 페이지 행 수
    rows: usize,
    /// 현재 페이지 열 수
    cols: usize,
    /// 선택 커서 (행)
    sel_row: usize,
    /// 선택 커서 (열)
    sel_col: usize,
    /// 윈도우 크기
    size: (u16, u16),
    /// 셀 크기
    cell_w: c_int,
    cell_h: c_int,
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

        let size: (u16, u16) = (400, 350);

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
            "특수문자 팝업 생성: pos=({},{})",
            final_x,
            final_y
        );

        Ok(Self {
            window,
            xft_draw,
            xft_font,
            text_color,
            sel_bg_color,
            header_color,
            page_color,
            characters: Vec::new(),
            target: String::new(),
            top_row: String::new(),
            current_page: 0,
            total_pages: 1,
            rows: 0,
            cols: 0,
            sel_row: 0,
            sel_col: 0,
            size,
            cell_w: 28,
            cell_h: 28,
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
        self.target = target.to_string();
        self.characters = characters;
        self.top_row = top_row.to_string();
        self.current_page = 0;
        self.sel_row = 0;
        self.sel_col = 0;
        self.total_pages = if self.characters.is_empty() {
            1
        } else {
            (self.characters.len() + PAGE_SIZE - 1) / PAGE_SIZE
        };

        self.update_page_layout();

        // 셀 크기 계산
        self.cell_w = 28;
        self.cell_h = 28;

        // 윈도우 크기 계산: 행 헤더(1열) + 데이터 열 + 여백, 열 헤더(1행) + 데이터 행 + 푸터
        let header_col_w = 30; // 행 번호 열 폭
        let header_row_h = self.cell_h; // 열 헤더 행 높이
        let footer_h = 24; // 페이지 정보 행 높이
        let width = header_col_w + (self.cols as c_int) * self.cell_w + 10;
        let height = header_row_h + (self.rows as c_int) * self.cell_h + footer_h + 10;

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

    /// 현재 페이지 레이아웃 재계산
    fn update_page_layout(&mut self) {
        let page_start = self.current_page * PAGE_SIZE;
        let page_chars = if page_start >= self.characters.len() {
            0
        } else {
            (self.characters.len() - page_start).min(PAGE_SIZE)
        };

        // 열 수: ceil(page_chars / MAX_ROWS)
        self.cols = if page_chars == 0 {
            1
        } else {
            ((page_chars + MAX_ROWS - 1) / MAX_ROWS)
                .min(MAX_COLS)
                .max(1)
        };
        // 행 수: ceil(page_chars / cols)
        self.rows = if page_chars == 0 {
            1
        } else {
            ((page_chars + self.cols - 1) / self.cols)
                .min(MAX_ROWS)
                .max(1)
        };
    }

    /// 현재 선택된 문자의 전체 인덱스
    fn selected_global_index(&self) -> Option<usize> {
        let page_start = self.current_page * PAGE_SIZE;
        // 열 우선 채움: index = col * rows + row
        let idx = self.sel_col * self.rows + self.sel_row;
        let page_chars = if page_start >= self.characters.len() {
            0
        } else {
            (self.characters.len() - page_start).min(PAGE_SIZE)
        };
        if idx < page_chars {
            Some(page_start + idx)
        } else {
            None
        }
    }

    /// 특정 그리드 위치에 해당하는 문자 가져오기
    fn char_at(&self, row: usize, col: usize) -> Option<&str> {
        let page_start = self.current_page * PAGE_SIZE;
        let idx = col * self.rows + row; // 열 우선 채움
        let global_idx = page_start + idx;
        if global_idx < self.characters.len() {
            Some(&self.characters[global_idx])
        } else {
            None
        }
    }

    /// 전체 인덱스로 문자 가져오기 (handler에서 직접 커밋용)
    pub fn get_character(&self, index: usize) -> Option<&str> {
        self.characters.get(index).map(|s| s.as_str())
    }

    /// 키 처리
    pub fn handle_key(&mut self, keysym: u32) -> SpecialAction {
        match keysym {
            // Escape
            0xff1b => SpecialAction::Cancel,

            // 열 점프 (물리 키: qwertyuio)
            0x71 | 0x77 | 0x65 | 0x72 | 0x74 | 0x79 | 0x75 | 0x69 | 0x6f => {
                let physical_keys: &[u32] = &[0x71, 0x77, 0x65, 0x72, 0x74, 0x79, 0x75, 0x69, 0x6f];
                if let Some(col_idx) = physical_keys.iter().position(|&k| k == keysym) {
                    if col_idx < self.cols {
                        self.sel_col = col_idx;
                        if self.char_at(self.sel_row, self.sel_col).is_none() {
                            // 해당 열의 마지막 유효한 행으로 이동
                            for r in (0..self.rows).rev() {
                                if self.char_at(r, self.sel_col).is_some() {
                                    self.sel_row = r;
                                    break;
                                }
                            }
                        }
                    }
                }
                SpecialAction::Consumed
            }

            // 숫자 1-9 → 행 선택
            0x31..=0x39 => {
                let row_idx = (keysym - 0x31) as usize;
                if row_idx < self.rows && self.char_at(row_idx, self.sel_col).is_some() {
                    self.sel_row = row_idx;
                    // 즉시 선택
                    if let Some(idx) = self.selected_global_index() {
                        return SpecialAction::Select(idx);
                    }
                }
                SpecialAction::Consumed
            }

            // Enter/Return → 현재 선택 확정
            0xff0d => {
                if let Some(idx) = self.selected_global_index() {
                    SpecialAction::Select(idx)
                } else {
                    SpecialAction::Consumed
                }
            }

            // 방향키
            0xff52 => {
                // Up
                if self.sel_row > 0 {
                    self.sel_row -= 1;
                } else {
                    self.sel_row = self.rows - 1;
                }
                // 유효한 셀인지 검증
                while self.sel_row > 0 && self.char_at(self.sel_row, self.sel_col).is_none() {
                    self.sel_row -= 1;
                }
                SpecialAction::Consumed
            }
            0xff54 => {
                // Down
                if self.sel_row + 1 < self.rows {
                    self.sel_row += 1;
                } else {
                    self.sel_row = 0;
                }
                if self.char_at(self.sel_row, self.sel_col).is_none() {
                    self.sel_row = 0;
                }
                SpecialAction::Consumed
            }
            0xff51 => {
                // Left
                if self.sel_col > 0 {
                    self.sel_col -= 1;
                } else {
                    self.sel_col = self.cols - 1;
                }
                if self.char_at(self.sel_row, self.sel_col).is_none() {
                    self.sel_row = 0;
                }
                SpecialAction::Consumed
            }
            0xff53 => {
                // Right
                if self.sel_col + 1 < self.cols {
                    self.sel_col += 1;
                } else {
                    self.sel_col = 0;
                }
                if self.char_at(self.sel_row, self.sel_col).is_none() {
                    self.sel_row = 0;
                }
                SpecialAction::Consumed
            }

            // Tab → 다음 페이지
            0xff09 => {
                if self.total_pages > 1 {
                    self.current_page = (self.current_page + 1) % self.total_pages;
                    self.update_page_layout();
                    self.sel_row = 0;
                    self.sel_col = 0;
                }
                SpecialAction::NextPage
            }
            // Shift+Tab (ISO_Left_Tab / BackTab)
            0xfe20 => {
                if self.total_pages > 1 {
                    self.current_page = if self.current_page > 0 {
                        self.current_page - 1
                    } else {
                        self.total_pages - 1
                    };
                    self.update_page_layout();
                    self.sel_row = 0;
                    self.sel_col = 0;
                }
                SpecialAction::PrevPage
            }

            // 모디파이어 키 무시
            0xffe1..=0xffee | 0xff7f | 0xff14 => SpecialAction::Consumed,

            _ => SpecialAction::None,
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
        let header_col_w = 30;
        let header_row_h = self.cell_h;

        match button {
            1 => {
                // 좌클릭 → 행/열 계산
                let col = (click_x - header_col_w) / self.cell_w;
                let row = (click_y - header_row_h) / self.cell_h;
                if col >= 0 && (col as usize) < self.cols && row >= 0 && (row as usize) < self.rows
                {
                    let r = row as usize;
                    let c = col as usize;
                    if self.char_at(r, c).is_some() {
                        self.sel_row = r;
                        self.sel_col = c;
                        if let Some(idx) = self.selected_global_index() {
                            return SpecialAction::Select(idx);
                        }
                    }
                }
                SpecialAction::Consumed
            }
            3 => {
                // 우클릭 → 다음 페이지
                if self.total_pages > 1 {
                    self.current_page = (self.current_page + 1) % self.total_pages;
                    self.update_page_layout();
                    self.sel_row = 0;
                    self.sel_col = 0;
                }
                SpecialAction::NextPage
            }
            _ => SpecialAction::Consumed,
        }
    }

    /// 다시 그리기
    pub fn redraw(&mut self, display: *mut x11::xlib::Display) {
        if display.is_null() {
            return;
        }

        unsafe {
            x11::xlib::XClearWindow(display, self.window);
        }

        let header_col_w: c_int = 30;
        let header_row_h = self.cell_h;
        let top_row_chars: Vec<char> = self.top_row.chars().collect();

        // 1. 열 헤더 (top_row 레이블)
        for c in 0..self.cols {
            let label = if c < top_row_chars.len() {
                top_row_chars[c].to_string()
            } else {
                format!("{}", c + 1)
            };
            let color = if c == self.sel_col {
                &self.sel_bg_color
            } else {
                &self.header_color
            };
            let x = header_col_w + (c as c_int) * self.cell_w + self.cell_w / 2 - 4;
            let y = header_row_h - 6;
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
        for r in 0..self.rows {
            let label = format!("{}", r + 1);
            let color = if r == self.sel_row {
                &self.sel_bg_color
            } else {
                &self.header_color
            };
            let x = 6;
            let y = header_row_h + (r as c_int) * self.cell_h + self.cell_h - 6;
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
        for c in 0..self.cols {
            for r in 0..self.rows {
                let cell_x = header_col_w + (c as c_int) * self.cell_w;
                let cell_y = header_row_h + (r as c_int) * self.cell_h;

                let is_selected = r == self.sel_row && c == self.sel_col;

                if let Some(ch) = self.char_at(r, c) {
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
                    let tx = cell_x + self.cell_w / 2 - 6;
                    let ty = cell_y + self.cell_h - 6;
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
        let footer_y = header_row_h + (self.rows as c_int) * self.cell_h + 18;
        let footer_text = format!(
            "[{}]  {} / {}",
            self.target,
            self.current_page + 1,
            self.total_pages
        );
        let fb = footer_text.as_bytes();
        unsafe {
            x11::xft::XftDrawStringUtf8(
                self.xft_draw,
                &self.page_color,
                self.xft_font,
                6,
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
