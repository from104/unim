//! 한자 후보 팝업 윈도우 모듈
//!
//! X11 Xlib/Xft를 사용하여 한자 후보 목록을 표시하는 팝업 윈도우입니다.
//! 키 처리 및 상태 관리는 `unim::popup::PopupState`에 위임하고, 이 모듈은 렌더링만 담당합니다.

use std::os::raw::c_int;

use unim::popup::PopupState;
use unim::unim_log;

use crate::dpi;

/// compact 모드 페이지당 후보 수 (1×9)
#[allow(dead_code)]
pub const PAGE_SIZE: usize = 9;
/// expanded 모드 페이지당 후보 수 (9×9)
const EXPANDED_PAGE_SIZE: usize = 81;
const EXPANDED_COLS: usize = 9;
const EXPANDED_ROWS: usize = 9;
/// 모드 표시 아이콘 (GTK Standalone·GNOME extension과 동일)
const ICON_EXPAND: &str = "⊞";
const ICON_COMPACT: &str = "⊟";

/// 한자 팝업 마우스 클릭 결과
pub enum HanjaClickResult {
    /// 후보 선택 (페이지 내 0-based 인덱스)
    Select(u32),
    /// 다음 페이지
    NextPage,
    /// 이벤트 소비됨
    Consumed,
}

/// 한자 후보 팝업 윈도우
pub struct HanjaWindow {
    /// X11 윈도우 ID
    window: c_ulong,
    /// XftDraw 컨텍스트
    xft_draw: *mut x11::xft::XftDraw,
    /// XftFont (메인: D2Coding 등 코딩 폰트)
    xft_font: *mut x11::xft::XftFont,
    /// XftFont (fallback: CJK 한자 포함 폰트)
    xft_font_fallback: *mut x11::xft::XftFont,
    // --- 색상 (Catppuccin Mocha) ---
    /// 배경 색상 (#1e1e2e)
    bg_color: x11::xft::XftColor,
    /// 테두리 색상 (#454768)
    border_color: x11::xft::XftColor,
    /// 헤더 배경 색상 (#313244)
    header_bg_color: x11::xft::XftColor,
    /// 헤더 텍스트 색상 (#89b4fa)
    header_color: x11::xft::XftColor,
    /// 본문 텍스트 색상 (#cdd6f4)
    text_color: x11::xft::XftColor,
    /// 번호 색상 (#7f849c)
    number_color: x11::xft::XftColor,
    /// 뜻풀이 색상 (#a6adc8)
    meaning_color: x11::xft::XftColor,
    /// 선택 배경 색상 (#313654)
    sel_bg_color: x11::xft::XftColor,
    /// 페이지 정보 색상 (#6c7086)
    page_color: x11::xft::XftColor,
    /// 즐겨찾기 강조 색상 (Catppuccin yellow #f9e2af) — compact 별 + expanded 셀
    bookmark_color: x11::xft::XftColor,
    /// Flash 효과 색상 (#3d4a6b — 선택+밝기 부스트)
    flash_color: x11::xft::XftColor,
    /// 통합 팝업 상태
    popup_state: Option<PopupState>,
    /// 윈도우 크기
    size: (u16, u16),
    /// DPI 스케일 팩터
    scale_factor: f64,
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
        let scale_factor = dpi::get_scale_factor(display, screen);

        // 초기 크기 (후보 설정 시 조정)
        let size: (u16, u16) = (
            dpi::scale_u16(300, scale_factor),
            dpi::scale_u16(200, scale_factor),
        );

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
            final_y = y - (size.1 as c_int) - 4; // 커서 위로 (POPUP_SPEC 6.2: 4px gap)
            if final_y < 0 {
                final_y = 0;
            }
        }

        let mut swa: x11::xlib::XSetWindowAttributes = unsafe { std::mem::zeroed() };
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

        // XftFont 로드 (메인 폰트)
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

        // CJK fallback 폰트 로드 (한자 글리프 포함)
        let xft_font_fallback = unsafe {
            let cjk_fonts: &[&[u8]] = &[
                b"Noto Sans CJK KR:size=13\0",
                b"Noto Serif CJK KR:size=13\0",
                b"UnBatang:size=13\0",
                b"sans:size=13\0",
            ];
            let mut font: *mut x11::xft::XftFont = std::ptr::null_mut();
            for pattern in cjk_fonts {
                font = x11::xft::XftFontOpenName(display, screen, pattern.as_ptr() as *const i8);
                if !font.is_null() {
                    if x11::xft::XftCharExists(display, font, 0x6F22) != 0 {
                        break;
                    }
                    x11::xft::XftFontClose(display, font);
                    font = std::ptr::null_mut();
                }
            }
            font
        };
        if !xft_font_fallback.is_null() {
            unim_log!("XIM_HANJA", "CJK fallback 폰트 로드 성공");
        } else {
            unim_log!("XIM_HANJA", "CJK fallback 폰트 로드 실패 — 한자 표시 불가");
        }

        // 색상 할당 — Catppuccin Mocha 팔레트
        let mut bg_color: x11::xft::XftColor = unsafe { std::mem::zeroed() };
        let mut border_color: x11::xft::XftColor = unsafe { std::mem::zeroed() };
        let mut header_bg_color: x11::xft::XftColor = unsafe { std::mem::zeroed() };
        let mut header_color: x11::xft::XftColor = unsafe { std::mem::zeroed() };
        let mut text_color: x11::xft::XftColor = unsafe { std::mem::zeroed() };
        let mut number_color: x11::xft::XftColor = unsafe { std::mem::zeroed() };
        let mut meaning_color: x11::xft::XftColor = unsafe { std::mem::zeroed() };
        let mut sel_bg_color: x11::xft::XftColor = unsafe { std::mem::zeroed() };
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
            alloc(b"#1e1e2e\0", &mut bg_color);
            alloc(b"#454768\0", &mut border_color);
            alloc(b"#313244\0", &mut header_bg_color);
            alloc(b"#89b4fa\0", &mut header_color);
            alloc(b"#cdd6f4\0", &mut text_color);
            alloc(b"#7f849c\0", &mut number_color);
            alloc(b"#a6adc8\0", &mut meaning_color);
            alloc(b"#333c57\0", &mut sel_bg_color);
            alloc(b"#6c7086\0", &mut page_color);
        }
        let mut bookmark_color: x11::xft::XftColor = unsafe { std::mem::zeroed() };
        unsafe {
            x11::xft::XftColorAllocName(
                display,
                visual,
                colormap,
                b"#f9e2af\0".as_ptr() as *const i8,
                &mut bookmark_color,
            );
        }
        let mut flash_color: x11::xft::XftColor = unsafe { std::mem::zeroed() };
        unsafe {
            x11::xft::XftColorAllocName(
                display,
                visual,
                colormap,
                b"#3d4a6b\0".as_ptr() as *const i8,
                &mut flash_color,
            );
        }

        unim_log!(
            "XIM_HANJA",
            "한자 팝업 생성: pos=({},{}), scale={:.2}",
            final_x,
            final_y,
            scale_factor
        );

        Ok(Self {
            window,
            xft_draw,
            xft_font,
            xft_font_fallback,
            bg_color,
            border_color,
            header_bg_color,
            header_color,
            text_color,
            number_color,
            meaning_color,
            sel_bg_color,
            page_color,
            bookmark_color,
            flash_color,
            popup_state: None,
            size,
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

    /// 후보 설정 및 표시
    pub fn set_candidates(
        &mut self,
        display: *mut x11::xlib::Display,
        screen: c_int,
        target: &str,
        candidates: Vec<(String, String)>,
    ) {
        self.popup_state = Some(PopupState::new_hanja(target, candidates));

        let ps = self.popup_state.as_ref().unwrap();

        // 윈도우 크기 계산: 헤더 + 후보 행 + 안내 행 + 패딩
        // expanded(9×9) 모드는 9행 + 헤더 1행 + 더 넓은 너비가 필요하므로 분기.
        let sf = self.scale_factor;
        let line_h = self.line_height(display);
        let padding_y = dpi::scale(8, sf);
        let header_h = line_h + dpi::scale(4, sf);
        let footer_h = line_h + dpi::scale(4, sf);
        let gap = dpi::scale(4, sf);
        let (width, height) = if ps.is_hanja_expanded() {
            // 9 cells × ~32px + col-header 행 = 9 + 1 = 10 rows
            let grid_rows = (EXPANDED_ROWS + 1) as c_int;
            let h = (padding_y + header_h + gap + grid_rows * line_h + footer_h + padding_y) as u16;
            (dpi::scale_u16(440, sf), h)
        } else {
            let page_count = ps.rows();
            let items_h = (page_count as c_int) * line_h;
            let h = (padding_y + header_h + gap + items_h + footer_h + padding_y) as u16;
            (dpi::scale_u16(340, sf), h)
        };

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
            wy = wy - (height as c_int) - 4;
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

    /// 전체 인덱스로 한자 문자열 가져오기
    #[allow(dead_code)]
    pub fn get_candidate(&self, index: usize) -> Option<&str> {
        self.popup_state.as_ref()?.get_item(index)
    }

    /// 특정 후보의 즐겨찾기 상태 갱신 (HanjaBookmarkChanged 시그널에서 호출)
    pub fn set_bookmark(
        &mut self,
        index: usize,
        bookmarked: bool,
        display: *mut x11::xlib::Display,
    ) {
        if let Some(ps) = self.popup_state.as_mut() {
            ps.set_bookmark(index, bookmarked);
            self.redraw(display);
        }
    }

    /// 전체 즐겨찾기 상태 일괄 반영 (초기 fetch 결과)
    pub fn set_bookmark_flags(&mut self, flags: Vec<bool>, display: *mut x11::xlib::Display) {
        if let Some(ps) = self.popup_state.as_mut() {
            ps.set_bookmark_flags(flags);
            self.redraw(display);
        }
    }

    /// 한자 후보를 즐겨찾기 정렬 결과로 일괄 교체하고 커서를 점프시킨다.
    /// (HanjaCandidatesReordered 시그널)
    ///
    /// 페이로드의 `page`/`sel_row`/`sel_col`을 직접 적용한다. 다른 프런트엔드와
    /// 동일 시맨틱: 엔진이 정렬 후 산출한 cursor 좌표를 그대로 신뢰. 과거에는
    /// `set_selected_global(new_cursor)` 한 번 호출로 page/row/col을 재계산했지만,
    /// 이는 엔진의 col-우선 인덱싱 정책에만 의존하는 단일 경로였다 (defect 1c).
    /// 이제 `_` 인자로 새 cursor 정합 검증을 위해 받기만 하고, 실제 적용은
    /// `replace_hanja_items` + `set_navigate_state`로 명시 분리.
    #[allow(clippy::too_many_arguments)]
    pub fn replace_candidates(
        &mut self,
        candidates: Vec<(String, String)>,
        bookmarks: Vec<bool>,
        _new_cursor: usize,
        page: usize,
        sel_row: usize,
        sel_col: usize,
        display: *mut x11::xlib::Display,
    ) {
        if let Some(ps) = self.popup_state.as_mut() {
            let items: Vec<String> = candidates.iter().map(|(h, _)| h.clone()).collect();
            let meanings: Vec<String> = candidates.iter().map(|(_, m)| m.clone()).collect();
            ps.replace_hanja_items(items, meanings, bookmarks);
            ps.set_navigate_state(page, sel_row, sel_col);
            self.redraw(display);
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
            (extents.height as c_int).max(dpi::scale(18, self.scale_factor))
                + dpi::scale(4, self.scale_factor)
        }
    }

    /// 엔진의 PopupNavigate 시그널로 상태 갱신 + 다시 그리기.
    /// cols 변화(1↔9)는 윈도우 크기 변경을 동반하므로 resize도 함께 적용한다.
    pub fn update_from_navigate(
        &mut self,
        page: usize,
        sel_row: usize,
        sel_col: usize,
        display: *mut x11::xlib::Display,
    ) {
        let needs_resize = if let Some(ps) = self.popup_state.as_mut() {
            let prev_expanded = ps.is_hanja_expanded();
            ps.set_navigate_state(page, sel_row, sel_col);
            prev_expanded != ps.is_hanja_expanded()
        } else {
            false
        };
        if needs_resize {
            self.resize_for_mode(display);
        }
        self.redraw(display);
    }

    /// 현재 모드(compact/expanded)에 맞춰 윈도우 크기를 재계산해 적용.
    /// Period 키로 토글된 직후 PopupNavigate가 새 cols로 들어올 때 호출된다.
    fn resize_for_mode(&mut self, display: *mut x11::xlib::Display) {
        let ps = match self.popup_state.as_ref() {
            Some(ps) => ps,
            None => return,
        };
        let sf = self.scale_factor;
        let line_h = self.line_height(display);
        let padding_y = dpi::scale(8, sf);
        let header_h = line_h + dpi::scale(4, sf);
        let footer_h = line_h + dpi::scale(4, sf);
        let gap = dpi::scale(4, sf);
        let (width, height) = if ps.is_hanja_expanded() {
            let grid_rows = (EXPANDED_ROWS + 1) as c_int;
            let h = (padding_y + header_h + gap + grid_rows * line_h + footer_h + padding_y) as u16;
            (dpi::scale_u16(440, sf), h)
        } else {
            let page_count = ps.rows();
            let items_h = (page_count as c_int) * line_h;
            let h = (padding_y + header_h + gap + items_h + footer_h + padding_y) as u16;
            (dpi::scale_u16(340, sf), h)
        };
        self.size = (width, height);
        unsafe {
            x11::xlib::XResizeWindow(display, self.window, width as u32, height as u32);
            x11::xlib::XFlush(display);
        }
    }

    /// 마우스 버튼 클릭 처리 — 행 인덱스 계산만 수행 (키 처리는 엔진에 위임).
    /// expanded(9×9) 모드의 좌클릭은 col/row → 글로벌 인덱스 변환과 합성 키 시퀀스가
    /// 비자명하므로 현재는 키보드 전용으로 한정한다 (Consumed 반환).
    /// compact 모드 동작은 기존과 동일.
    pub fn handle_button_press(
        &self,
        button: u32,
        y: c_int,
        display: *mut x11::xlib::Display,
    ) -> HanjaClickResult {
        let ps = match self.popup_state.as_ref() {
            Some(ps) => ps,
            None => return HanjaClickResult::Consumed,
        };

        let line_h = self.line_height(display);

        match button {
            1 => {
                if line_h == 0 {
                    return HanjaClickResult::Consumed;
                }
                if ps.is_hanja_expanded() {
                    // 9×9 grid 클릭 → 키보드 전용으로 한정 (별도 합성 키 시퀀스 필요).
                    unim_log!("XIM_HANJA", "expanded 모드 좌클릭은 무시 (keyboard-only)");
                    return HanjaClickResult::Consumed;
                }
                let idx = ((y - dpi::scale(4, self.scale_factor)) / line_h) as usize;
                if idx < ps.rows() {
                    unim_log!("XIM_HANJA", "좌클릭 선택: row={}", idx);
                    HanjaClickResult::Select(idx as u32)
                } else {
                    HanjaClickResult::Consumed
                }
            }
            3 => {
                // 우클릭 → 다음 페이지
                unim_log!("XIM_HANJA", "우클릭 → 다음 페이지 요청");
                HanjaClickResult::NextPage
            }
            _ => HanjaClickResult::Consumed,
        }
    }

    /// UTF-8 문자열을 font fallback 적용하여 렌더링
    fn draw_string_with_fallback(
        &self,
        display: *mut x11::xlib::Display,
        color: &x11::xft::XftColor,
        x: c_int,
        y: c_int,
        text: &str,
    ) {
        if self.xft_font_fallback.is_null() {
            let bytes = text.as_bytes();
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
            return;
        }

        let mut cursor_x = x;
        let mut batch_start: usize = 0;
        let mut batch_use_fallback = false;
        let chars: Vec<(usize, char)> = text.char_indices().collect();

        for i in 0..=chars.len() {
            let use_fallback = if i < chars.len() {
                let ch = chars[i].1;
                let code = ch as u32;
                unsafe {
                    x11::xft::XftCharExists(display, self.xft_font, code as std::os::raw::c_uint)
                        == 0
                }
            } else {
                !batch_use_fallback
            };

            if i > batch_start && (use_fallback != batch_use_fallback || i == chars.len()) {
                let batch_end = if i < chars.len() {
                    chars[i].0
                } else {
                    text.len()
                };
                let batch_bytes = &text.as_bytes()[chars[batch_start].0..batch_end];
                let font = if batch_use_fallback {
                    self.xft_font_fallback
                } else {
                    self.xft_font
                };

                unsafe {
                    x11::xft::XftDrawStringUtf8(
                        self.xft_draw,
                        color,
                        font,
                        cursor_x,
                        y,
                        batch_bytes.as_ptr(),
                        batch_bytes.len() as c_int,
                    );

                    let mut extents: x11::xrender::XGlyphInfo = std::mem::zeroed();
                    x11::xft::XftTextExtentsUtf8(
                        display,
                        font,
                        batch_bytes.as_ptr(),
                        batch_bytes.len() as c_int,
                        &mut extents,
                    );
                    cursor_x += extents.xOff as c_int;
                }

                batch_start = i;
            }

            if i < chars.len() && i == batch_start {
                batch_use_fallback = use_fallback;
            }
        }
    }

    /// 다시 그리기 — PopupState에서 데이터를 읽어 Catppuccin Mocha 렌더링.
    /// is_hanja_expanded()로 compact(1×9) ↔ expanded(9×9) 분기.
    pub fn redraw(&mut self, display: *mut x11::xlib::Display) {
        if display.is_null() {
            return;
        }

        let expanded = match self.popup_state.as_ref() {
            Some(ps) => ps.is_hanja_expanded(),
            None => return,
        };
        if expanded {
            self.redraw_expanded(display);
        } else {
            self.redraw_compact(display);
        }
    }

    /// compact 모드 (1×9 list) 렌더 — 기존 동작.
    fn redraw_compact(&mut self, display: *mut x11::xlib::Display) {
        let ps = match self.popup_state.as_ref() {
            Some(ps) => ps,
            None => return,
        };

        unsafe {
            x11::xlib::XClearWindow(display, self.window);
        }

        let sf = self.scale_factor;
        let line_h = self.line_height(display);
        let padding_x: c_int = dpi::scale(12, sf);
        let padding_y: c_int = dpi::scale(8, sf);
        let items = ps.hanja_page_items();
        let selected_index = ps.sel_row();

        // 헤더 배경 (#313244)
        let header_h = line_h + dpi::scale(4, sf);
        let inset = dpi::scale(4, sf);
        unsafe {
            let gc = x11::xlib::XCreateGC(display, self.window, 0, std::ptr::null_mut());
            x11::xlib::XSetForeground(display, gc, self.header_bg_color.pixel);
            x11::xlib::XFillRectangle(
                display,
                self.window,
                gc,
                inset,
                inset,
                (self.size.0 as c_int - inset * 2) as u32,
                header_h as u32,
            );
            x11::xlib::XFreeGC(display, gc);
        }

        // 헤더 텍스트: 「한」 → 한자  1/3
        let text_offset = dpi::scale(2, sf);
        let header_text = format!("「{}」 → 한자", ps.target());
        self.draw_string_with_fallback(
            display,
            &self.header_color,
            padding_x,
            padding_y + line_h - text_offset,
            &header_text,
        );
        // 페이지 번호 (헤더 오른쪽)
        let page_str = format!("{}/{}", ps.current_page() + 1, ps.total_pages());
        let page_bytes = page_str.as_bytes();
        unsafe {
            let mut extents: x11::xrender::XGlyphInfo = std::mem::zeroed();
            x11::xft::XftTextExtentsUtf8(
                display,
                self.xft_font,
                page_bytes.as_ptr(),
                page_bytes.len() as c_int,
                &mut extents,
            );
            let page_x = (self.size.0 as c_int) - padding_x - extents.xOff as c_int;
            x11::xft::XftDrawStringUtf8(
                self.xft_draw,
                &self.page_color,
                self.xft_font,
                page_x,
                padding_y + line_h - text_offset,
                page_bytes.as_ptr(),
                page_bytes.len() as c_int,
            );
        }

        // 후보 행
        let items_start_y = padding_y + header_h + dpi::scale(4, sf);
        for (i, (hanja, meaning)) in items.iter().enumerate() {
            let y_pos = items_start_y + (i as c_int) * line_h + line_h - text_offset;
            let row_top = items_start_y + (i as c_int) * line_h;

            // 선택된 항목 배경 (#313654)
            if i == selected_index {
                unsafe {
                    let gc = x11::xlib::XCreateGC(display, self.window, 0, std::ptr::null_mut());
                    x11::xlib::XSetForeground(display, gc, self.sel_bg_color.pixel);
                    x11::xlib::XFillRectangle(
                        display,
                        self.window,
                        gc,
                        inset,
                        row_top,
                        (self.size.0 as c_int - inset * 2) as u32,
                        line_h as u32,
                    );
                    x11::xlib::XFreeGC(display, gc);
                }
            }

            // 번호 "N." (#7f849c)
            let num_text = format!("{}.", i + 1);
            let num_bytes = num_text.as_bytes();
            unsafe {
                x11::xft::XftDrawStringUtf8(
                    self.xft_draw,
                    &self.number_color,
                    self.xft_font,
                    padding_x,
                    y_pos,
                    num_bytes.as_ptr(),
                    num_bytes.len() as c_int,
                );
            }

            // 한자 문자 (#cdd6f4, fallback font)
            let hanja_x = padding_x + dpi::scale(28, sf);
            self.draw_string_with_fallback(display, &self.text_color, hanja_x, y_pos, hanja);

            // 한자 글자 너비 측정
            let hanja_width = unsafe {
                let mut ext: x11::xrender::XGlyphInfo = std::mem::zeroed();
                let font = if !self.xft_font_fallback.is_null() {
                    self.xft_font_fallback
                } else {
                    self.xft_font
                };
                x11::xft::XftTextExtentsUtf8(
                    display,
                    font,
                    hanja.as_bytes().as_ptr(),
                    hanja.as_bytes().len() as c_int,
                    &mut ext,
                );
                ext.xOff as c_int
            };

            // 뜻풀이 (#a6adc8)
            if !meaning.is_empty() {
                let meaning_x = hanja_x + hanja_width + dpi::scale(8, sf);
                let meaning_bytes = meaning.as_bytes();
                unsafe {
                    x11::xft::XftDrawStringUtf8(
                        self.xft_draw,
                        &self.meaning_color,
                        self.xft_font,
                        meaning_x,
                        y_pos,
                        meaning_bytes.as_ptr(),
                        meaning_bytes.len() as c_int,
                    );
                }
            }

            // 즐겨찾기 별 (☆/★) — 행 오른쪽 끝. Xft fallback 경로로 렌더 (CJK 폰트).
            let global_idx = ps.current_page() * 9 + i;
            let bookmarked = ps.is_bookmarked(global_idx);
            let star_text = if bookmarked { "★" } else { "☆" };
            let star_width = unsafe {
                let mut ext: x11::xrender::XGlyphInfo = std::mem::zeroed();
                let font = if !self.xft_font_fallback.is_null() {
                    self.xft_font_fallback
                } else {
                    self.xft_font
                };
                x11::xft::XftTextExtentsUtf8(
                    display,
                    font,
                    star_text.as_bytes().as_ptr(),
                    star_text.as_bytes().len() as c_int,
                    &mut ext,
                );
                ext.xOff as c_int
            };
            let star_x = (self.size.0 as c_int) - padding_x - star_width;
            // 즐겨찾기 별: Catppuccin 노랑(#f9e2af), 미설정: 회색(#6c7086)
            let star_color = if bookmarked {
                &self.bookmark_color
            } else {
                &self.page_color
            };
            self.draw_string_with_fallback(display, star_color, star_x, y_pos, star_text);
        }

        // 하단 안내 + ⊞ 모드 토글 아이콘 (compact 상태)
        let footer_y = items_start_y + (items.len() as c_int) * line_h + line_h;
        let footer = "← → 페이지 | 1~9 선택 | . 확장 | ESC 취소";
        let footer_bytes = footer.as_bytes();
        unsafe {
            x11::xft::XftDrawStringUtf8(
                self.xft_draw,
                &self.page_color,
                self.xft_font,
                padding_x,
                footer_y,
                footer_bytes.as_ptr(),
                footer_bytes.len() as c_int,
            );
        }
        // ⊞ 아이콘 (compact 모드에서는 expand 표시)
        let icon_x = (self.size.0 as c_int) - padding_x - dpi::scale(14, sf);
        self.draw_string_with_fallback(display, &self.page_color, icon_x, footer_y, ICON_EXPAND);
        unsafe {
            x11::xlib::XFlush(display);
        }
    }

    /// expanded 모드 (9×9 grid) 렌더 — col 우선 인덱싱(idx = col*9 + row).
    /// 헤더 row(상단)에 1-9 col 번호, 셀에 한자 한 글자.
    fn redraw_expanded(&mut self, display: *mut x11::xlib::Display) {
        let ps = match self.popup_state.as_ref() {
            Some(ps) => ps,
            None => return,
        };

        unsafe {
            x11::xlib::XClearWindow(display, self.window);
        }

        let sf = self.scale_factor;
        let line_h = self.line_height(display);
        let padding_x: c_int = dpi::scale(12, sf);
        let padding_y: c_int = dpi::scale(8, sf);
        let inset = dpi::scale(4, sf);
        let header_h = line_h + dpi::scale(4, sf);
        let text_offset = dpi::scale(2, sf);
        let total_pages = ps.total_pages();
        let current_page = ps.current_page();
        let sel_row = ps.sel_row();
        let sel_col = ps.sel_col();
        let page_start = current_page * EXPANDED_PAGE_SIZE;

        // 헤더 배경
        unsafe {
            let gc = x11::xlib::XCreateGC(display, self.window, 0, std::ptr::null_mut());
            x11::xlib::XSetForeground(display, gc, self.header_bg_color.pixel);
            x11::xlib::XFillRectangle(
                display,
                self.window,
                gc,
                inset,
                inset,
                (self.size.0 as c_int - inset * 2) as u32,
                header_h as u32,
            );
            x11::xlib::XFreeGC(display, gc);
        }
        // 헤더 텍스트
        let header_text = format!("「{}」 → 한자", ps.target());
        self.draw_string_with_fallback(
            display,
            &self.header_color,
            padding_x,
            padding_y + line_h - text_offset,
            &header_text,
        );
        // 페이지 번호
        let page_str = format!("{}/{}", current_page + 1, total_pages);
        let page_bytes = page_str.as_bytes();
        unsafe {
            let mut extents: x11::xrender::XGlyphInfo = std::mem::zeroed();
            x11::xft::XftTextExtentsUtf8(
                display,
                self.xft_font,
                page_bytes.as_ptr(),
                page_bytes.len() as c_int,
                &mut extents,
            );
            let page_x = (self.size.0 as c_int) - padding_x - extents.xOff as c_int;
            x11::xft::XftDrawStringUtf8(
                self.xft_draw,
                &self.page_color,
                self.xft_font,
                page_x,
                padding_y + line_h - text_offset,
                page_bytes.as_ptr(),
                page_bytes.len() as c_int,
            );
        }

        // 좌측 행 번호 영역 폭 (UX 일관성: special·GTK·Qt·GNOME과 동일하게 행 번호 헤더 노출)
        let row_label_w = dpi::scale(20, sf);

        // 셀 너비 = (가용 폭 - 행 번호 영역) / 9
        let avail_w = (self.size.0 as c_int) - 2 * padding_x - row_label_w;
        let cell_w = avail_w / EXPANDED_COLS as c_int;
        let grid_left = padding_x + row_label_w;
        let grid_top = padding_y + header_h + dpi::scale(4, sf);

        // 컬럼 헤더 — special과 동일하게 가로 레이블 키 시퀀스(Q/W/E/.../O).
        // 활성=header_color, 비활성=number_color (special의 active/inactive 대응).
        const TOP_ROW: &[u8] = b"QWERTYUIO";
        for col in 0..EXPANDED_COLS {
            let label = (TOP_ROW[col] as char).to_string();
            let cx = grid_left + (col as c_int) * cell_w + cell_w / 2 - dpi::scale(4, sf);
            let cy = grid_top + line_h - text_offset;
            let color = if col == sel_col {
                &self.header_color
            } else {
                &self.number_color
            };
            self.draw_string_with_fallback(display, color, cx, cy, &label);
        }

        // 셀 배치 — col 우선 인덱싱
        let cells_top = grid_top + line_h;

        // 행 번호 헤더 (좌측 1..9). sel_row면 header_color, 아니면 number_color.
        for row in 0..EXPANDED_ROWS {
            let label = format!("{}", row + 1);
            let rx = padding_x + dpi::scale(4, sf);
            let ry = cells_top + (row as c_int) * line_h + line_h - text_offset;
            let color = if row == sel_row {
                &self.header_color
            } else {
                &self.number_color
            };
            self.draw_string_with_fallback(display, color, rx, ry, &label);
        }

        for col in 0..EXPANDED_COLS {
            for row in 0..EXPANDED_ROWS {
                let offset = col * EXPANDED_ROWS + row;
                let global = page_start + offset;
                let hanja = match ps.get_item(global) {
                    Some(s) => s,
                    None => continue,
                };
                let cx = grid_left + (col as c_int) * cell_w;
                let cy = cells_top + (row as c_int) * line_h;
                // 선택 셀 배경
                if col == sel_col && row == sel_row {
                    unsafe {
                        let gc =
                            x11::xlib::XCreateGC(display, self.window, 0, std::ptr::null_mut());
                        x11::xlib::XSetForeground(display, gc, self.sel_bg_color.pixel);
                        x11::xlib::XFillRectangle(
                            display,
                            self.window,
                            gc,
                            cx,
                            cy,
                            cell_w as u32,
                            line_h as u32,
                        );
                        x11::xlib::XFreeGC(display, gc);
                    }
                }
                let bookmarked = ps.is_bookmarked(global);
                // 즐겨찾기 셀: Catppuccin 노랑(#f9e2af), 일반: 본문 텍스트색(#cdd6f4)
                let color = if bookmarked {
                    &self.bookmark_color
                } else {
                    &self.text_color
                };
                // 셀 가운데 근처에 한자 (cell_w 기반 대략적 정렬 — 정밀 측정은 비용 큼)
                let text_x = cx + dpi::scale(4, sf);
                let text_y = cy + line_h - text_offset;
                self.draw_string_with_fallback(display, color, text_x, text_y, hanja);
            }
        }

        // 푸터: 안내 + ⊟ 아이콘
        let footer_y = cells_top + (EXPANDED_ROWS as c_int) * line_h + line_h;
        let footer = "← → ↑ ↓ 이동 | 1~9 선택 | . 축소 | ESC 취소";
        let footer_bytes = footer.as_bytes();
        unsafe {
            x11::xft::XftDrawStringUtf8(
                self.xft_draw,
                &self.page_color,
                self.xft_font,
                padding_x,
                footer_y,
                footer_bytes.as_ptr(),
                footer_bytes.len() as c_int,
            );
        }
        let icon_x = (self.size.0 as c_int) - padding_x - dpi::scale(14, sf);
        self.draw_string_with_fallback(display, &self.page_color, icon_x, footer_y, ICON_COMPACT);
        unsafe {
            x11::xlib::XFlush(display);
        }
    }

    /// Flash 효과: 선택 행을 밝은 색으로 120ms 표시 후 반환
    /// 현재는 미사용 (POPUP_SPEC: flash는 특수문자 전용), 향후 활용 가능
    #[allow(dead_code)]
    pub fn flash_selection(&mut self, display: *mut x11::xlib::Display) {
        let ps = match self.popup_state.as_ref() {
            Some(ps) => ps,
            None => return,
        };

        let sf = self.scale_factor;
        let line_h = self.line_height(display);
        let padding_y: c_int = dpi::scale(8, sf);
        let header_h = line_h + dpi::scale(4, sf);
        let items_start_y = padding_y + header_h + dpi::scale(4, sf);
        let inset = dpi::scale(4, sf);
        let selected = ps.sel_row();
        let row_top = items_start_y + (selected as c_int) * line_h;

        // Flash 색상으로 선택 행 배경 렌더링
        unsafe {
            let gc = x11::xlib::XCreateGC(display, self.window, 0, std::ptr::null_mut());
            x11::xlib::XSetForeground(display, gc, self.flash_color.pixel);
            x11::xlib::XFillRectangle(
                display,
                self.window,
                gc,
                inset,
                row_top,
                (self.size.0 as c_int - inset * 2) as u32,
                line_h as u32,
            );
            x11::xlib::XFreeGC(display, gc);
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
                &self.bg_color,
                &self.border_color,
                &self.header_bg_color,
                &self.header_color,
                &self.text_color,
                &self.number_color,
                &self.meaning_color,
                &self.sel_bg_color,
                &self.page_color,
                &self.bookmark_color,
                &self.flash_color,
            ];
            for color in colors {
                x11::xft::XftColorFree(display, visual, colormap, *color as *const _ as *mut _);
            }

            x11::xft::XftFontClose(display, self.xft_font);
            if !self.xft_font_fallback.is_null() {
                x11::xft::XftFontClose(display, self.xft_font_fallback);
            }
            x11::xft::XftDrawDestroy(self.xft_draw);
            x11::xlib::XDestroyWindow(display, self.window);
            x11::xlib::XFlush(display);

            x11::xlib::XSetErrorHandler(old_handler);
        }
        unim_log!("XIM_HANJA", "한자 팝업 정리됨");
    }
}

/// X11 에러 무시용 더미 핸들러
unsafe extern "C" fn dummy_error_handler(
    _display: *mut x11::xlib::Display,
    _event: *mut x11::xlib::XErrorEvent,
) -> c_int {
    0
}
