//! Over-The-Spot Preedit 윈도우 모듈
//!
//! PREEDIT_POSITION 스타일에서 서버가 직접 X11 윈도우를 생성하여
//! preedit 문자열을 렌더링합니다. XftFont를 사용하여 한국어을 렌더링합니다.
//!
//! 중요: Display 연결은 UnimHandler에서 관리하고 참조로 전달받음

use std::num::NonZeroU32;
use std::os::raw::{c_int, c_ulong};

use x11rb::protocol::xproto::ConfigureNotifyEvent;
use xim::ServerError;

/// Preedit 윈도우
///
/// Display는 소유하지 않고 참조만 가짐 (UnimHandler가 소유)
pub struct PeWindow {
    /// 윈도우 ID
    window: c_ulong,
    /// XftDraw 컨텍스트
    xft_draw: *mut x11::xft::XftDraw,
    /// XftFont
    xft_font: *mut x11::xft::XftFont,
    /// XftColor (검은색)
    xft_color: x11::xft::XftColor,
    /// 현재 preedit 문자열
    preedit: String,
    /// 윈도우 크기
    size: (u16, u16),
}

impl PeWindow {
    /// 새 Preedit 윈도우 생성
    pub fn new(
        display: *mut x11::xlib::Display,
        screen: c_int,
        app_win: Option<NonZeroU32>,
        spot_location: xim::Point,
    ) -> Result<Self, ServerError> {
        let root = unsafe { x11::xlib::XRootWindow(display, screen) };
        let white_pixel = unsafe { x11::xlib::XWhitePixel(display, screen) };
        let black_pixel = unsafe { x11::xlib::XBlackPixel(display, screen) };

        let font_size = 16u16;
        let padding_x = 8u16; // 좌우 여백
        let padding_y = 4u16; // 상하 여백
        let size = (
            (font_size as f32 * 1.5) as u16 + padding_x * 2, // 한국어 1글자 너비 + 좌우 여백
            font_size + padding_y * 2,                       // 폰트 높이 + 상하 여백
        );

        // 앱 윈도우가 있으면 앱 윈도우를 부모로 사용 (앱 종료 시 자동 삭제)
        let (parent, mut pos_x, mut pos_y) = match app_win {
            Some(win) => (
                win.get() as c_ulong,
                spot_location.x as c_int,
                spot_location.y as c_int,
            ),
            None => (root, spot_location.x as c_int, spot_location.y as c_int),
        };

        // 부모 윈도우 크기를 가져와서 경계 검사
        let (parent_width, parent_height) = unsafe {
            let mut attrs: x11::xlib::XWindowAttributes = std::mem::zeroed();
            if x11::xlib::XGetWindowAttributes(display, parent, &mut attrs) != 0 {
                (attrs.width as c_int, attrs.height as c_int)
            } else {
                // 실패 시 큰 값 사용 (경계 검사 무시)
                (10000, 10000)
            }
        };

        // 오른쪽 경계 검사: preedit 윈도우가 부모 오른쪽을 넘으면 왼쪽으로 이동
        if pos_x + (size.0 as c_int) > parent_width {
            pos_x = parent_width - (size.0 as c_int);
            if pos_x < 0 {
                pos_x = 0;
            }
        }

        // 아래 경계 검사: preedit 윈도우가 부모 아래를 넘으면 위로 이동 (커서 위)
        if pos_y + (size.1 as c_int) > parent_height {
            // 커서 위로 이동 (font height 정도 위)
            pos_y = pos_y - (size.1 as c_int) - (font_size as c_int);
            if pos_y < 0 {
                pos_y = 0;
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
                parent,
                pos_x,
                pos_y,
                size.0 as u32,
                size.1 as u32,
                0,
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
            return Err(ServerError::Internal("XCreateWindow failed".to_string()));
        }

        // _NET_WM_WINDOW_TYPE 설정
        unsafe {
            let window_type_atom = x11::xlib::XInternAtom(
                display,
                c"_NET_WM_WINDOW_TYPE".as_ptr(),
                x11::xlib::False,
            );
            let popup_atom = x11::xlib::XInternAtom(
                display,
                c"_NET_WM_WINDOW_TYPE_POPUP_MENU".as_ptr(),
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

        // WM_CLASS 설정
        unsafe {
            let wm_class: &[u8] = b"unim\0unim\0";
            let xa_string = 31u64;
            let xa_wm_class = x11::xlib::XInternAtom(
                display,
                c"WM_CLASS".as_ptr(),
                x11::xlib::False,
            );
            x11::xlib::XChangeProperty(
                display,
                window,
                xa_wm_class,
                xa_string,
                8,
                x11::xlib::PropModeReplace,
                wm_class.as_ptr(),
                (wm_class.len() - 1) as i32,
            );
        }

        // 윈도우 표시
        unsafe {
            x11::xlib::XMapWindow(display, window);
            x11::xlib::XFlush(display);
        }

        // XftDraw 생성
        let visual = unsafe { x11::xlib::XDefaultVisual(display, screen) };
        let colormap = unsafe { x11::xlib::XDefaultColormap(display, screen) };

        let xft_draw = unsafe { x11::xft::XftDrawCreate(display, window, visual, colormap) };

        if xft_draw.is_null() {
            unsafe {
                x11::xlib::XDestroyWindow(display, window);
            }
            return Err(ServerError::Internal("XftDrawCreate failed".to_string()));
        }

        // XftFont 로드
        let xft_font = unsafe {
            let font_pattern = b"D2Coding:size=14\0";
            x11::xft::XftFontOpenName(display, screen, font_pattern.as_ptr() as *const i8)
        };

        let xft_font = if xft_font.is_null() {
            unsafe {
                let fallback = b"monospace:size=14\0";
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
            return Err(ServerError::Internal("XftFontOpenName failed".to_string()));
        }

        // XftColor 할당
        let mut xft_color: x11::xft::XftColor = unsafe { std::mem::zeroed() };
        unsafe {
            let color_name = b"black\0";
            x11::xft::XftColorAllocName(
                display,
                visual,
                colormap,
                color_name.as_ptr() as *const i8,
                &mut xft_color,
            );
        }

        Ok(Self {
            window,
            xft_draw,
            xft_font,
            xft_color,
            preedit: String::with_capacity(10),
            size,
        })
    }

    /// 윈도우 정리 (Display는 닫지 않음 - UnimHandler가 소유)
    /// 앱이 먼저 종료되어 윈도우가 이미 삭제된 경우에도 안전하게 처리
    pub fn clean(self, display: *mut x11::xlib::Display, screen: c_int) {
        if display.is_null() {
            return;
        }

        unsafe {
            // 현재 에러 핸들러를 저장하고 더미 핸들러로 교체
            let old_handler = x11::xlib::XSetErrorHandler(Some(dummy_error_handler));

            let visual = x11::xlib::XDefaultVisual(display, screen);
            let colormap = x11::xlib::XDefaultColormap(display, screen);

            // Xft 리소스 정리
            x11::xft::XftColorFree(
                display,
                visual,
                colormap,
                &self.xft_color as *const _ as *mut _,
            );
            x11::xft::XftFontClose(display, self.xft_font);
            x11::xft::XftDrawDestroy(self.xft_draw);

            // 윈도우 삭제 (이미 삭제되었어도 에러 무시)
            x11::xlib::XDestroyWindow(display, self.window);
            x11::xlib::XFlush(display);

            // 에러 핸들러 복원
            x11::xlib::XSetErrorHandler(old_handler);
        }
    }

    /// 윈도우 ID
    pub fn window(&self) -> NonZeroU32 {
        NonZeroU32::new(self.window as u32).unwrap()
    }

    /// preedit 문자열 설정
    pub fn set_preedit(&mut self, s: &str) {
        self.preedit.clear();
        self.preedit.push_str(s);
    }

    /// 다시 그리기
    pub fn redraw(&mut self, display: *mut x11::xlib::Display) {
        if display.is_null() {
            return;
        }

        if self.preedit.is_empty() {
            unsafe {
                x11::xlib::XClearWindow(display, self.window);
                x11::xlib::XFlush(display);
            }
            return;
        }

        let text_bytes = self.preedit.as_bytes();
        let padding_x = 6;
        let padding_y = 4;

        // 텍스트 크기 측정
        let (text_width, text_height) = unsafe {
            let mut extents: x11::xrender::XGlyphInfo = std::mem::zeroed();
            x11::xft::XftTextExtentsUtf8(
                display,
                self.xft_font,
                text_bytes.as_ptr(),
                text_bytes.len() as c_int,
                &mut extents,
            );
            (extents.width as u16, extents.height as u16)
        };

        // 윈도우 크기를 텍스트에 맞게 조정
        let new_width = text_width + (padding_x * 2) as u16;
        let new_height = text_height.max(16) + (padding_y * 2) as u16;

        if new_width != self.size.0 || new_height != self.size.1 {
            self.size = (new_width, new_height);
            unsafe {
                x11::xlib::XResizeWindow(display, self.window, new_width as u32, new_height as u32);
            }
        }

        unsafe {
            x11::xlib::XClearWindow(display, self.window);
        }

        // 텍스트 그리기 (가운데 정렬)
        let x_pos = padding_x;
        let y_pos = self.size.1 as i32 - padding_y - 2;

        unsafe {
            x11::xft::XftDrawStringUtf8(
                self.xft_draw,
                &self.xft_color,
                self.xft_font,
                x_pos,
                y_pos,
                text_bytes.as_ptr(),
                text_bytes.len() as c_int,
            );
            x11::xlib::XFlush(display);
        }
    }

    /// Expose 이벤트 처리
    pub fn expose(&mut self, display: *mut x11::xlib::Display) {
        self.redraw(display);
    }

    /// ConfigureNotify 이벤트 처리
    pub fn configure_notify(&mut self, e: ConfigureNotifyEvent, display: *mut x11::xlib::Display) {
        self.size = (e.width, e.height);
        self.redraw(display);
    }

    /// 새로고침
    pub fn refresh(&mut self, display: *mut x11::xlib::Display) {
        self.redraw(display);
    }
}

/// X11 에러 무시용 더미 핸들러
unsafe extern "C" fn dummy_error_handler(
    _display: *mut x11::xlib::Display,
    _event: *mut x11::xlib::XErrorEvent,
) -> c_int {
    // 에러 무시 (앱 종료 시 윈도우가 이미 삭제되었을 수 있음)
    0
}
