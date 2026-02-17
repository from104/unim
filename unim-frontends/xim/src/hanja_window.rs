//! 한자 후보 팝업 윈도우 모듈
//!
//! X11 Xlib/Xft를 사용하여 한자 후보 목록을 표시하는 팝업 윈도우입니다.
//! PeWindow와 동일한 패턴으로 override_redirect 윈도우를 사용합니다.

use std::os::raw::c_int;

use unim::unim_log;

/// 한자 팝업에서 키 처리 결과
pub enum HanjaAction {
    /// 후보 선택 (0-based 인덱스)
    Select(u32),
    /// 취소 (Esc)
    Cancel,
    /// 다음 페이지 (→)
    NextPage,
    /// 이전 페이지 (←)
    PrevPage,
    /// 팝업이 내부적으로 처리한 키 (↑↓ 등)
    Consumed,
    /// 처리하지 않음 → 팝업 닫고 키 바이패스
    None,
}

/// 페이지당 표시할 후보 수
const PAGE_SIZE: usize = 9;

/// 한자 후보 팝업 윈도우
pub struct HanjaWindow {
    /// X11 윈도우 ID
    window: c_ulong,
    /// XftDraw 컨텍스트
    xft_draw: *mut x11::xft::XftDraw,
    /// XftFont
    xft_font: *mut x11::xft::XftFont,
    /// 텍스트 색상
    text_color: x11::xft::XftColor,
    /// 선택 배경 색상
    sel_bg_color: x11::xft::XftColor,
    /// 선택 텍스트 색상
    sel_text_color: x11::xft::XftColor,
    /// 페이지 정보 색상
    page_color: x11::xft::XftColor,
    /// 후보 목록 (한자, 뜻)
    candidates: Vec<(String, String)>,
    /// 대상 문자열
    target: String,
    /// 현재 페이지 (0-based)
    current_page: usize,
    /// 선택된 인덱스 (페이지 내, 0-based)
    selected_index: usize,
    /// 윈도우 크기
    size: (u16, u16),
}

use std::os::raw::c_ulong;

impl HanjaWindow {
    /// 한자 팝업 윈도우 생성
    pub fn new(
        display: *mut x11::xlib::Display,
        screen: c_int,
        x: c_int,
        y: c_int,
    ) -> Result<Self, String> {
        let root = unsafe { x11::xlib::XRootWindow(display, screen) };
        let white_pixel = unsafe { x11::xlib::XWhitePixel(display, screen) };
        let black_pixel = unsafe { x11::xlib::XBlackPixel(display, screen) };

        // 초기 크기 (후보 설정 시 조정)
        let size: (u16, u16) = (300, 200);

        // 화면 크기 가져오기
        let screen_w = unsafe { x11::xlib::XDisplayWidth(display, screen) };
        let screen_h = unsafe { x11::xlib::XDisplayHeight(display, screen) };

        // 화면 경계 보정
        let mut final_x = x;
        let mut final_y = y;

        if final_x + (size.0 as c_int) > screen_w {
            final_x = screen_w - (size.0 as c_int);
            if final_x < 0 {
                final_x = 0;
            }
        }
        if final_y + (size.1 as c_int) > screen_h {
            final_y = y - (size.1 as c_int) - 20; // 커서 위로
            if final_y < 0 {
                final_y = 0;
            }
        }

        let mut swa: x11::xlib::XSetWindowAttributes = unsafe { std::mem::zeroed() };
        swa.background_pixel = white_pixel;
        swa.border_pixel = black_pixel;
        swa.override_redirect = x11::xlib::True;
        swa.event_mask = x11::xlib::ExposureMask | x11::xlib::StructureNotifyMask;

        let window = unsafe {
            x11::xlib::XCreateWindow(
                display,
                root,
                final_x,
                final_y,
                size.0 as u32,
                size.1 as u32,
                1, // border width
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
            let window_type_atom = x11::xlib::XInternAtom(
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
                window_type_atom,
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

        // XftFont 로드
        let xft_font = unsafe {
            let font_pattern = b"D2Coding:size=13\0";
            x11::xft::XftFontOpenName(display, screen, font_pattern.as_ptr() as *const i8)
        };
        let xft_font = if xft_font.is_null() {
            unsafe {
                let fallback = b"monospace:size=13\0";
                x11::xft::XftFontOpenName(display, screen, fallback.as_ptr() as *const i8)
            }
        } else {
            xft_font
        };
        if xft_font.is_null() {
            unsafe {
                x11::xft::XftDrawDestroy(xft_draw);
                x11::xlib::XDestroyWindow(display, window);
            }
            return Err("XftFontOpenName failed".to_string());
        }

        // 색상 할당
        let mut text_color: x11::xft::XftColor = unsafe { std::mem::zeroed() };
        let mut sel_bg_color: x11::xft::XftColor = unsafe { std::mem::zeroed() };
        let mut sel_text_color: x11::xft::XftColor = unsafe { std::mem::zeroed() };
        let mut page_color: x11::xft::XftColor = unsafe { std::mem::zeroed() };

        unsafe {
            x11::xft::XftColorAllocName(
                display,
                visual,
                colormap,
                b"#1e1e2e\0".as_ptr() as *const i8,
                &mut text_color,
            );
            x11::xft::XftColorAllocName(
                display,
                visual,
                colormap,
                b"#89b4fa\0".as_ptr() as *const i8,
                &mut sel_bg_color,
            );
            x11::xft::XftColorAllocName(
                display,
                visual,
                colormap,
                b"#1e1e2e\0".as_ptr() as *const i8,
                &mut sel_text_color,
            );
            x11::xft::XftColorAllocName(
                display,
                visual,
                colormap,
                b"#6c7086\0".as_ptr() as *const i8,
                &mut page_color,
            );
        }

        unim_log!("XIM_HANJA", "한자 팝업 생성: pos=({},{})", final_x, final_y);

        Ok(Self {
            window,
            xft_draw,
            xft_font,
            text_color,
            sel_bg_color,
            sel_text_color,
            page_color,
            candidates: Vec::new(),
            target: String::new(),
            current_page: 0,
            selected_index: 0,
            size,
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

    /// 후보 설정 및 표시
    pub fn set_candidates(
        &mut self,
        display: *mut x11::xlib::Display,
        screen: c_int,
        target: &str,
        candidates: Vec<(String, String)>,
    ) {
        self.target = target.to_string();
        self.candidates = candidates;
        self.current_page = 0;
        self.selected_index = 0;

        // 윈도우 크기 계산
        let line_h = self.line_height(display);
        let page_count = self.page_items().len();
        // 후보 행 + 페이지 정보 행 + 여백
        let height = (page_count as u16 + 1) * (line_h as u16) + 8;
        let width: u16 = 300;

        self.size = (width, height);
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
        if wx + (width as c_int) > screen_w {
            wx = screen_w - (width as c_int);
        }
        if wy + (height as c_int) > screen_h {
            wy = wy - (height as c_int) - 20;
            if wy < 0 {
                wy = 0;
            }
        }
        unsafe {
            x11::xlib::XMoveWindow(display, self.window, wx, wy);
            x11::xlib::XMapRaised(display, self.window);
            x11::xlib::XFlush(display);
        }

        self.redraw(display);
    }

    /// 현재 페이지의 후보 항목
    fn page_items(&self) -> &[(String, String)] {
        let start = self.current_page * PAGE_SIZE;
        let end = (start + PAGE_SIZE).min(self.candidates.len());
        if start >= self.candidates.len() {
            &[]
        } else {
            &self.candidates[start..end]
        }
    }

    /// 총 페이지 수
    fn total_pages(&self) -> usize {
        if self.candidates.is_empty() {
            1
        } else {
            (self.candidates.len() + PAGE_SIZE - 1) / PAGE_SIZE
        }
    }

    /// 행 높이 계산
    fn line_height(&self, display: *mut x11::xlib::Display) -> c_int {
        let test = "漢가";
        let test_bytes = test.as_bytes();
        unsafe {
            let mut extents: x11::xrender::XGlyphInfo = std::mem::zeroed();
            x11::xft::XftTextExtentsUtf8(
                display,
                self.xft_font,
                test_bytes.as_ptr(),
                test_bytes.len() as c_int,
                &mut extents,
            );
            (extents.height as c_int).max(18) + 4
        }
    }

    /// 키 처리
    pub fn handle_key(&mut self, keyval: u32) -> HanjaAction {
        // xim crate에서는 keyval 대신 X keycode를 사용하므로
        // 여기서는 일반적인 X11 keysym 값을 사용
        match keyval {
            // 숫자 1-9
            0x31..=0x39 => {
                let idx = (keyval - 0x31) as usize;
                if idx < self.page_items().len() {
                    let global_idx = self.current_page * PAGE_SIZE + idx;
                    HanjaAction::Select(global_idx as u32)
                } else {
                    HanjaAction::None
                }
            }
            // Escape
            0xff1b => HanjaAction::Cancel,
            // Right arrow / space
            0xff53 | 0x20 => {
                if self.current_page + 1 < self.total_pages() {
                    self.current_page += 1;
                    self.selected_index = 0;
                }
                HanjaAction::NextPage
            }
            // Left arrow / BackSpace
            0xff51 | 0xff08 => {
                if self.current_page > 0 {
                    self.current_page -= 1;
                    self.selected_index = 0;
                }
                HanjaAction::PrevPage
            }
            // Down arrow
            0xff54 => {
                let count = self.page_items().len();
                if count > 0 {
                    self.selected_index = (self.selected_index + 1) % count;
                }
                HanjaAction::Consumed
            }
            // Up arrow
            0xff52 => {
                let count = self.page_items().len();
                if count > 0 {
                    if self.selected_index == 0 {
                        self.selected_index = count - 1;
                    } else {
                        self.selected_index -= 1;
                    }
                }
                HanjaAction::Consumed
            }
            // Enter/Return
            0xff0d => {
                let count = self.page_items().len();
                if count > 0 && self.selected_index < count {
                    let global_idx = self.current_page * PAGE_SIZE + self.selected_index;
                    HanjaAction::Select(global_idx as u32)
                } else {
                    HanjaAction::Consumed
                }
            }
            // 모디파이어 키 (Shift, Ctrl, Alt, CapsLock, Super, Num_Lock, Scroll_Lock 등) → 무시
            0xffe1..=0xffee | 0xff7f | 0xff14 => HanjaAction::Consumed,
            _ => HanjaAction::None,
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

        let line_h = self.line_height(display);
        let padding_x = 6;
        let items = self.page_items().to_vec();

        for (i, (hanja, meaning)) in items.iter().enumerate() {
            let y_pos = (i as c_int + 1) * line_h;

            // 선택된 항목 배경
            if i == self.selected_index {
                unsafe {
                    // XftColor에서 pixel 추출하여 GC로 사각형 채우기
                    let gc = x11::xlib::XCreateGC(display, self.window, 0, std::ptr::null_mut());
                    x11::xlib::XSetForeground(display, gc, self.sel_bg_color.pixel);
                    x11::xlib::XFillRectangle(
                        display,
                        self.window,
                        gc,
                        0,
                        y_pos - line_h + 4,
                        self.size.0 as u32,
                        line_h as u32,
                    );
                    x11::xlib::XFreeGC(display, gc);
                }
            }

            // 텍스트: "N. 漢 뜻"
            let text = format!("{}. {} {}", i + 1, hanja, meaning);
            let text_bytes = text.as_bytes();
            let color = if i == self.selected_index {
                &self.sel_text_color
            } else {
                &self.text_color
            };

            unsafe {
                x11::xft::XftDrawStringUtf8(
                    self.xft_draw,
                    color,
                    self.xft_font,
                    padding_x,
                    y_pos,
                    text_bytes.as_ptr(),
                    text_bytes.len() as c_int,
                );
            }
        }

        // 페이지 정보
        let page_text = format!("{} / {}", self.current_page + 1, self.total_pages());
        let page_bytes = page_text.as_bytes();
        let page_y = (items.len() as c_int + 1) * line_h;

        unsafe {
            x11::xft::XftDrawStringUtf8(
                self.xft_draw,
                &self.page_color,
                self.xft_font,
                padding_x,
                page_y,
                page_bytes.as_ptr(),
                page_bytes.len() as c_int,
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

            // 색상 해제
            x11::xft::XftColorFree(
                display,
                visual,
                colormap,
                &self.text_color as *const _ as *mut _,
            );
            x11::xft::XftColorFree(
                display,
                visual,
                colormap,
                &self.sel_bg_color as *const _ as *mut _,
            );
            x11::xft::XftColorFree(
                display,
                visual,
                colormap,
                &self.sel_text_color as *const _ as *mut _,
            );
            x11::xft::XftColorFree(
                display,
                visual,
                colormap,
                &self.page_color as *const _ as *mut _,
            );

            x11::xft::XftFontClose(display, self.xft_font);
            x11::xft::XftDrawDestroy(self.xft_draw);
            x11::xlib::XDestroyWindow(display, self.window);
            x11::xlib::XFlush(display);

            x11::xlib::XSetErrorHandler(old_handler);
        }
        unim_log!("XIM_HANJA", "한자 팝업 정리됨");
    }

    // 숨기기 (윈도우 언맵)
    // pub fn hide(&self, display: *mut x11::xlib::Display) {
    //     unsafe {
    //         x11::xlib::XUnmapWindow(display, self.window);
    //         x11::xlib::XFlush(display);
    //     }
    // }
}

/// X11 에러 무시용 더미 핸들러
unsafe extern "C" fn dummy_error_handler(
    _display: *mut x11::xlib::Display,
    _event: *mut x11::xlib::XErrorEvent,
) -> c_int {
    0
}
