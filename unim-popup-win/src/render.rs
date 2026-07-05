//! GDI 렌더 — 설계서 §5.4 (레이아웃·색·폰트·flash) 가 SoT.
//!
//! **금지**: 구 popup_window.rs 의 row-major 루프 포팅 금지. 셀 접근은 오로지
//! `RenderState::cell_at(row, col)` (= cells[col*rows+row]). 선택은 좌표 비교 1차.
//!
//! 모든 레이아웃 상수는 논리값. 실제 픽셀 = 논리값 × scale (DPI, §5.5).

use windows::core::PCWSTR;
use windows::Win32::Foundation::{COLORREF, RECT, SIZE};
use windows::Win32::Graphics::Gdi::*;
use windows::Win32::System::Registry::{RegGetValueW, HKEY_CURRENT_USER, RRF_RT_REG_DWORD};
use windows::Win32::UI::Accessibility::{HCF_HIGHCONTRASTON, HIGHCONTRASTW};
use windows::Win32::UI::WindowsAndMessaging::{
    SystemParametersInfoW, SPI_GETHIGHCONTRAST, SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS,
};
// GetSysColor / COLOR_* (SYS_COLOR_INDEX) 는 Gdi 모듈 소속 — 위 `Gdi::*` glob 로 유입.

use crate::logln;
use crate::protocol::{cell_flags, RenderState, RevEvent};

// ─── 테마 적응 팔레트 (I5) ──────────────────────────────────────────────────
//
// 팝업이 OS 데스크톱 테마(다크/라이트)와 고대비 모드를 따르도록 색을 런타임에
// 고른다. 다크는 기존 Catppuccin Mocha 를 그대로 유지하고, 라이트(Latte)와
// 고대비(GetSysColor 기반)를 새로 추가한다. 색만으로 상태를 구분하지 않도록
// 선택 셀에는 2px 대비 링(sel_ring), 활성 헤더에는 밑줄/막대 형태 단서를 곁들인다
// (WCAG 1.4.11). COLORREF = 0x00BBGGRR (BGR 주의).
#[derive(Clone, Copy)]
struct Palette {
    base: COLORREF,        // 배경
    surface0: COLORREF,    // 헤더/푸터/탭 배경
    text: COLORREF,        // 본문
    subtext0: COLORREF,    // 뜻풀이
    overlay1: COLORREF,    // 행/열 레이블
    sel_hanja: COLORREF,   // 한자 선택 배경
    sel_green: COLORREF,   // 특수/이모지 선택 배경 + 활성 탭
    flash: COLORREF,       // flash 선택 배경
    hdr_active: COLORREF,  // 활성 열/행 헤더
    hdr_inactive: COLORREF,// 비활성 열 헤더
    empty: COLORREF,       // 빈 셀
    rowhl: COLORREF,       // 행 보조 강조
    colhl: COLORREF,       // 열 보조 강조
    on_sel_text: COLORREF, // 선택 셀 위 텍스트(대비)
    star: COLORREF,        // 북마크 ★
    sel_ring: COLORREF,    // 선택 셀 2px 대비 링(형태 단서)
}

impl Palette {
    #[inline]
    fn sel_bg(&self, kind: u32) -> COLORREF {
        // 0=Hanja → blue, 1=SpecialChar / 2=Emoji → green (POPUP_SPEC §5.1).
        if kind == 0 {
            self.sel_hanja
        } else {
            self.sel_green
        }
    }
}

// 다크(Catppuccin Mocha) — 현행 유지가 기본값(테마 조회 실패 시 폴백).
const DARK: Palette = Palette {
    base: COLORREF(0x002e1e1e),        // #1e1e2e
    surface0: COLORREF(0x00443231),    // #313244
    text: COLORREF(0x00f4d6cd),        // #cdd6f4
    subtext0: COLORREF(0x00c8ada6),    // #a6adc8
    overlay1: COLORREF(0x009c847f),    // #7f849c
    sel_hanja: COLORREF(0x00fab489),   // #89b4fa
    sel_green: COLORREF(0x00a1e3a6),   // #a6e3a1
    flash: COLORREF(0x00afe2f9),       // #f9e2af
    hdr_active: COLORREF(0x00a1e3a6),  // #a6e3a1
    hdr_inactive: COLORREF(0x00afe2f9),// #f9e2af
    empty: COLORREF(0x003a2a2a),
    rowhl: COLORREF(0x00403030),
    colhl: COLORREF(0x00303a30),
    on_sel_text: COLORREF(0x00000000), // 검정 (파스텔 선택 배경 위 대비)
    star: COLORREF(0x00afe2f9),        // #f9e2af
    sel_ring: COLORREF(0x00000000),    // 검정 링 — 밝은 파스텔 선택을 또렷이 감쌈
};

// 라이트(Catppuccin Latte) — 라이트 데스크톱용 신규 팔레트.
const LIGHT: Palette = Palette {
    base: COLORREF(0x00f5f1ef),        // #eff1f5
    surface0: COLORREF(0x00dad0cc),    // #ccd0da
    text: COLORREF(0x00694f4c),        // #4c4f69
    subtext0: COLORREF(0x00856f6c),    // #6c6f85
    overlay1: COLORREF(0x00a18f8c),    // #8c8fa1
    sel_hanja: COLORREF(0x00f5661e),   // #1e66f5 (blue)
    sel_green: COLORREF(0x002ba040),   // #40a02b (green)
    flash: COLORREF(0x001d8edf),       // #df8e1d (yellow)
    hdr_active: COLORREF(0x002ba040),  // #40a02b
    hdr_inactive: COLORREF(0x001d8edf),// #df8e1d
    empty: COLORREF(0x00eae3e1),       // base 보다 약간 어두운 빈 셀
    rowhl: COLORREF(0x00fbe6dc),       // 연한 파랑 틴트
    colhl: COLORREF(0x00dcefdc),       // 연한 초록 틴트
    on_sel_text: COLORREF(0x00ffffff), // 흰색 (채도 높은 선택 배경 위 대비)
    star: COLORREF(0x001d8edf),        // #df8e1d
    sel_ring: COLORREF(0x00ffffff),    // 흰 링 — 채도 높은 선택을 또렷이 감쌈
};

/// 데스크톱 앱 테마가 라이트인지 조회한다.
///
/// `HKCU\...\Themes\Personalize\AppsUseLightTheme`(DWORD) 가 1 이면 라이트,
/// 0 이면 다크. 트레이 인디케이터(lang_bar.rs)는 트레이 배경 기준의
/// `SystemUsesLightTheme` 을 쓰지만, 팝업은 일반 앱 창이므로 앱 테마 기준의
/// `AppsUseLightTheme` 이 맞다. 키 부재/조회 실패 시 다크로 폴백(현행 유지).
fn apps_use_light_theme() -> bool {
    let subkey: Vec<u16> = "Software\\Microsoft\\Windows\\CurrentVersion\\Themes\\Personalize\0"
        .encode_utf16()
        .collect();
    let value: Vec<u16> = "AppsUseLightTheme\0".encode_utf16().collect();
    let mut data: u32 = 0; // 기본=다크
    let mut size = std::mem::size_of::<u32>() as u32;
    unsafe {
        let r = RegGetValueW(
            HKEY_CURRENT_USER,
            PCWSTR(subkey.as_ptr()),
            PCWSTR(value.as_ptr()),
            RRF_RT_REG_DWORD,
            None,
            Some(&mut data as *mut u32 as *mut core::ffi::c_void),
            Some(&mut size),
        );
        if r.is_err() {
            return false; // 조회 실패 → 다크 폴백
        }
    }
    data != 0
}

/// 고대비(High Contrast) 모드가 켜져 있는지 조회한다.
///
/// `SystemParametersInfo(SPI_GETHIGHCONTRAST)` 의 `dwFlags & HCF_HIGHCONTRASTON`.
/// 켜져 있으면 GetSysColor 기반 팔레트로 전환해 사용자가 지정한 고대비 배색을
/// 그대로 따른다(임의 색으로 덮어쓰지 않는다).
fn high_contrast_active() -> bool {
    unsafe {
        let mut hc = HIGHCONTRASTW {
            cbSize: std::mem::size_of::<HIGHCONTRASTW>() as u32,
            ..Default::default()
        };
        let ok = SystemParametersInfoW(
            SPI_GETHIGHCONTRAST,
            std::mem::size_of::<HIGHCONTRASTW>() as u32,
            Some(&mut hc as *mut _ as *mut core::ffi::c_void),
            SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS(0),
        );
        ok.is_ok() && (hc.dwFlags.0 & HCF_HIGHCONTRASTON.0) != 0
    }
}

/// 고대비 팔레트 — 사용자가 OS 에 지정한 시스템 색(GetSysColor)만 사용한다.
///
/// 활성/비활성 헤더를 색만으로 구분할 수 없으므로(고대비 팔레트는 색 수가 적다)
/// 밑줄/막대 형태 단서(paint_grid)가 실질 구분을 담당한다.
unsafe fn high_contrast_palette() -> Palette {
    let win = COLORREF(GetSysColor(COLOR_WINDOW));
    let txt = COLORREF(GetSysColor(COLOR_WINDOWTEXT));
    let hl = COLORREF(GetSysColor(COLOR_HIGHLIGHT));
    let hltext = COLORREF(GetSysColor(COLOR_HIGHLIGHTTEXT));
    let face = COLORREF(GetSysColor(COLOR_BTNFACE));
    let gray = COLORREF(GetSysColor(COLOR_GRAYTEXT));
    Palette {
        base: win,
        surface0: face,
        text: txt,
        subtext0: gray,
        overlay1: gray,
        sel_hanja: hl,
        sel_green: hl,
        flash: hl,
        hdr_active: txt,      // 활성 헤더 텍스트(밑줄 단서와 병행)
        hdr_inactive: gray,   // 비활성 헤더 텍스트
        empty: face,
        rowhl: face,
        colhl: face,
        on_sel_text: hltext,
        star: txt,
        sel_ring: hltext,     // 선택 배경(COLOR_HIGHLIGHT) 위 대비 링
    }
}

/// 현재 데스크톱 상태에 맞는 팔레트를 고른다: 고대비 우선 → 라이트 → 다크.
fn current_palette() -> Palette {
    if high_contrast_active() {
        unsafe { high_contrast_palette() }
    } else if apps_use_light_theme() {
        LIGHT
    } else {
        DARK
    }
}

// ─── 논리 레이아웃 상수 ─────────────────────────────────────────────────────
const PAD: i32 = 8; // 외곽 여백
const HEADER_H: i32 = 28; // 헤더 바 높이
const FOOTER_H: i32 = 24; // 푸터 바 높이
const CELL_W: i32 = 44; // 격자 셀 폭 (§11.B 54→44 폭 축소; 비-compact 524→434px)
const CELL_H: i32 = 34; // 격자 셀 높이
const ROW_LABEL_W: i32 = 22; // 좌측 행 레이블 열 폭
const COL_LABEL_H: i32 = 22; // 상단 열 레이블 행 높이
const TAB_W: i32 = 76; // 이모지 좌측 탭 컬럼 폭 (§11.B 88→76)
const COMPACT_ROW_H: i32 = 30; // 한자 compact 행 높이
const COMPACT_MIN_W: i32 = 280; // compact 최소 폭
const COMPACT_MAX_W: i32 = 560; // compact 최대 폭
const GRID_COLS_FIXED: i32 = 9; // 격자 열 헤더 고정 9개 (팝업 폭 고정 정책)

// ─── footer 클릭 영역 폭 (§11.E — paint·hit_test 동일 상수) ──────────────────
const PAGE_BTN_W: i32 = 24; // 좌측 ◀(Prev) / (expand 없을 때) 우측 ▶(Next) 영역 폭
const EXPAND_BTN_W: i32 = 28; // 우측 ⊞/⊟(expand_toggle) 영역 폭

// 폰트 높이 (논리 px). DPI 스케일 적용은 측정/그리기 시점에 한다.
const FONT_HEADER: i32 = 13; // bold
const FONT_CELL: i32 = 16;
const FONT_HANJA: i32 = 18; // bold
const FONT_MEANING: i32 = 12;
const FONT_LABEL: i32 = 11; // bold

#[inline]
fn s(v: i32, scale: f64) -> i32 {
    ((v as f64) * scale).round() as i32
}

fn to_wide(s: &str) -> Vec<u16> {
    s.encode_utf16().collect()
}

unsafe fn make_font(height_logical: i32, scale: f64, bold: bool) -> HFONT {
    CreateFontW(
        -s(height_logical, scale),
        0,
        0,
        0,
        if bold { FW_BOLD.0 as i32 } else { FW_NORMAL.0 as i32 },
        0,
        0,
        0,
        DEFAULT_CHARSET,
        OUT_DEFAULT_PRECIS,
        CLIP_DEFAULT_PRECIS,
        CLEARTYPE_QUALITY,
        (DEFAULT_PITCH.0 | FF_DONTCARE.0) as u32,
        PCWSTR(to_wide("맑은 고딕\0").as_ptr()),
    )
}

unsafe fn fill(hdc: HDC, rect: &RECT, color: COLORREF) {
    let brush = CreateSolidBrush(color);
    let _ = FillRect(hdc, rect, brush);
    let _ = DeleteObject(brush.into());
}

/// 선택 셀 위에 `thickness`px 두께의 대비 링을 그린다 (WCAG 1.4.11 형태 단서).
///
/// 셀 배경색과 별개로 셀 테두리를 또렷하게 감싸, 색을 구분하기 어려운
/// 사용자(색약·고대비)도 선택 위치를 형태로 인지하게 한다. FrameRect(1px)를
/// 안쪽으로 1px 씩 좁혀 가며 `thickness` 번 그려 두께를 만든다.
unsafe fn draw_ring(hdc: HDC, rect: &RECT, color: COLORREF, thickness: i32) {
    let brush = CreateSolidBrush(color);
    let mut r = *rect;
    for _ in 0..thickness.max(1) {
        let _ = FrameRect(hdc, &r, brush);
        r.left += 1;
        r.top += 1;
        r.right -= 1;
        r.bottom -= 1;
        if r.right <= r.left || r.bottom <= r.top {
            break;
        }
    }
    let _ = DeleteObject(brush.into());
}

unsafe fn draw_text(hdc: HDC, text: &str, rect: &RECT, color: COLORREF, fmt: DRAW_TEXT_FORMAT) {
    if text.is_empty() {
        return;
    }
    SetTextColor(hdc, color);
    SetBkMode(hdc, TRANSPARENT);
    let mut wide = to_wide(text);
    let mut r = *rect;
    let _ = DrawTextW(hdc, &mut wide, &mut r, fmt);
}

/// 측정 전용: 한 폰트로 문자열 폭(논리 px, scale 적용 전 환산)을 잰다.
unsafe fn text_width(hdc: HDC, text: &str, font: HFONT) -> i32 {
    let old = SelectObject(hdc, font.into());
    let mut sz = SIZE::default();
    let wide = to_wide(text);
    let _ = GetTextExtentPoint32W(hdc, &wide, &mut sz);
    SelectObject(hdc, old);
    sz.cx
}

/// 콘텐츠 크기(픽셀) 계산. scale 이 이미 곱해진 최종 픽셀 크기를 돌려준다.
/// measuring HDC(메모리 DC) 가 필요해 호출자가 hdc 를 넘긴다.
pub unsafe fn measure(hdc: HDC, rs: &RenderState, scale: f64) -> (i32, i32) {
    if rs.is_compact() {
        measure_compact(hdc, rs, scale)
    } else {
        measure_grid(rs, scale)
    }
}

unsafe fn measure_compact(hdc: HDC, rs: &RenderState, scale: f64) -> (i32, i32) {
    let font_hanja = make_font(FONT_HANJA, scale, true);
    let font_mean = make_font(FONT_MEANING, scale, false);
    let mut max_w = s(COMPACT_MIN_W, scale);
    let rows = rs.rows;
    for r in 0..rows {
        if let Some(cell) = rs.cell_at(r, 0) {
            if cell.f & cell_flags::HAS_DATA == 0 {
                continue;
            }
            let label = rs.row_headers.get(r as usize).map(|h| h.0.as_str()).unwrap_or("");
            let star = if cell.f & cell_flags::BOOKMARKED != 0 { " ★" } else { "" };
            let main = format!("{label} {}{star}", cell.t);
            let w = s(ROW_LABEL_W, scale)
                + text_width(hdc, &main, font_hanja)
                + s(16, scale)
                + text_width(hdc, &cell.m, font_mean)
                + s(PAD * 3, scale);
            if w > max_w {
                max_w = w;
            }
        }
    }
    let _ = DeleteObject(font_hanja.into());
    let _ = DeleteObject(font_mean.into());
    let max_w = max_w.min(s(COMPACT_MAX_W, scale));
    let footer = if rs.show_footer { s(FOOTER_H, scale) } else { 0 };
    let h = s(HEADER_H, scale) + (rows as i32) * s(COMPACT_ROW_H, scale) + footer + s(PAD * 2, scale);
    (max_w, h)
}

fn measure_grid(rs: &RenderState, scale: f64) -> (i32, i32) {
    let emoji = !rs.tab_labels.is_empty();
    let tab_w = if emoji { s(TAB_W, scale) } else { 0 };
    // 열 헤더는 항상 9개 (팝업 폭 고정 정책 §5.4).
    let grid_w = s(ROW_LABEL_W, scale) + GRID_COLS_FIXED * s(CELL_W, scale);
    let w = s(PAD, scale) * 2 + tab_w + grid_w;
    let footer = if rs.show_footer { s(FOOTER_H, scale) } else { 0 };
    // 이모지 탭은 격자 행과 같은 높이(CELL_H)로 열 레이블 아래에서 시작해 행과 1:1 정렬한다.
    // 본문 행 수 = 격자 행과 탭 개수(이모지 9개) 중 큰 쪽.
    let n_tabs = if emoji { rs.tab_labels.len() as i32 } else { 0 };
    let body_rows = (rs.rows as i32).max(n_tabs);
    let body_h = s(COL_LABEL_H, scale) + body_rows * s(CELL_H, scale);
    let h = s(HEADER_H, scale) + body_h + footer + s(PAD * 2, scale);
    (w, h)
}

/// 전체 렌더. `flash_on` = 현재 flash 타이머 활성 여부 (선택 셀 노란 배경).
pub unsafe fn paint(hdc: HDC, rs: &RenderState, scale: f64, w: i32, h: i32, flash_on: bool) {
    // OS 테마/고대비에 맞는 팔레트를 매 페인트마다 감지(레지스트리+SPI 조회는
    // 저렴하고, WM_SETTINGCHANGE/WM_THEMECHANGED 재도색이 이 값을 갱신한다).
    let pal = current_palette();
    // 배경.
    let full = RECT { left: 0, top: 0, right: w, bottom: h };
    fill(hdc, &full, pal.base);
    SetBkMode(hdc, TRANSPARENT);

    // 헤더 바.
    let pad = s(PAD, scale);
    let header_rect = RECT {
        left: 0,
        top: 0,
        right: w,
        bottom: s(HEADER_H, scale),
    };
    fill(hdc, &header_rect, pal.surface0);
    let font_header = make_font(FONT_HEADER, scale, true);
    let old = SelectObject(hdc, font_header.into());
    let mut hr = header_rect;
    hr.left += pad;
    hr.right -= pad;
    draw_text(hdc, &rs.header_text, &hr, pal.text, DT_VCENTER | DT_SINGLELINE | DT_LEFT);
    SelectObject(hdc, old);
    let _ = DeleteObject(font_header.into());

    // 본문. 이모지 격자 셀 텍스트는 GDI 로 그리지 않고 모아서(emoji_cells) 마지막에
    // D2D 컬러 폰트로 덧그린다 (§11.A — GDI 전부 → D2D 이모지 텍스트 순서).
    let mut emoji_cells: Vec<crate::d2d::EmojiCell> = Vec::new();
    if rs.is_compact() {
        paint_compact(hdc, rs, scale, w, h, flash_on, &pal);
    } else {
        paint_grid(hdc, rs, scale, w, h, flash_on, &mut emoji_cells, &pal);
    }

    // 푸터 바.
    if rs.show_footer {
        let footer_rect = RECT {
            left: 0,
            top: h - s(FOOTER_H, scale),
            right: w,
            bottom: h,
        };
        fill(hdc, &footer_rect, pal.surface0);
        let font_f = make_font(FONT_MEANING, scale, false);
        let old = SelectObject(hdc, font_f.into());
        let mut fr = footer_rect;
        fr.left += pad;
        fr.right -= pad;
        draw_text(hdc, &rs.footer_text, &fr, pal.subtext0, DT_VCENTER | DT_SINGLELINE | DT_CENTER);
        // 페이지 ◀/▶ 글리프 (§11.E — total_pages>1 일 때만, hit_test 와 동일 좌표).
        if rs.total_pages > 1 {
            // 좌측 ◀ (Prev).
            let prev_rect = RECT {
                left: pad,
                top: footer_rect.top,
                right: pad + s(PAGE_BTN_W, scale),
                bottom: footer_rect.bottom,
            };
            draw_text(hdc, "◀", &prev_rect, pal.text, DT_VCENTER | DT_SINGLELINE | DT_CENTER);
            // 우측 ▶ (Next) — expand 가 우측을 차지하지 않을 때만 (겹침 방지).
            if !rs.expand_visible {
                let next_rect = RECT {
                    left: w - pad - s(PAGE_BTN_W, scale),
                    top: footer_rect.top,
                    right: w - pad,
                    bottom: footer_rect.bottom,
                };
                draw_text(hdc, "▶", &next_rect, pal.text, DT_VCENTER | DT_SINGLELINE | DT_CENTER);
            }
        }
        // expand 아이콘 (⊞/⊟) — 우측 끝. (hit_test 의 expand 영역과 동일.)
        if rs.expand_visible {
            let exp_rect = RECT {
                left: w - pad - s(EXPAND_BTN_W, scale),
                top: footer_rect.top,
                right: w - pad,
                bottom: footer_rect.bottom,
            };
            draw_text(hdc, &rs.expand_text, &exp_rect, pal.text, DT_VCENTER | DT_SINGLELINE | DT_CENTER);
        }
        SelectObject(hdc, old);
        let _ = DeleteObject(font_f.into());
    }

    // ─── 컬러 이모지 오버레이 (§11.A) — GDI 전부 끝난 뒤 마지막에 D2D 로 덧그린다.
    //     BindDC~EndDraw 구간엔 같은 HDC GDI 금지 → 여기가 paint 의 최종 단계.
    //     D2D 실패(폴백) 시 GDI 흑백으로 같은 셀을 그려 공백 방지. ───────────────
    if !emoji_cells.is_empty() {
        let full = RECT { left: 0, top: 0, right: w, bottom: h };
        // 글리프 px = 셀 높이 기반(중앙정렬). 셀보다 약간 작게.
        let font_px = (s(CELL_H, scale) as f32) * 0.62;
        let ok = crate::d2d::draw_emoji_cells(hdc, &full, font_px, &emoji_cells);
        if !ok {
            // 흑백 폴백 — GDI DrawText 로 동일 셀 텍스트(중앙) 그리기.
            let font_cell = make_font(FONT_CELL, scale, false);
            let old = SelectObject(hdc, font_cell.into());
            for c in &emoji_cells {
                let color = if c.on_selected { pal.on_sel_text } else { pal.text };
                draw_text(hdc, &c.text, &c.rect, color, DT_VCENTER | DT_SINGLELINE | DT_CENTER);
            }
            SelectObject(hdc, old);
            let _ = DeleteObject(font_cell.into());
            logln!("paint: D2D emoji unavailable — GDI monochrome fallback ({} cells)", emoji_cells.len());
        }
    }
}

unsafe fn paint_compact(hdc: HDC, rs: &RenderState, scale: f64, w: i32, _h: i32, flash_on: bool, pal: &Palette) {
    let pad = s(PAD, scale);
    let row_h = s(COMPACT_ROW_H, scale);
    let top0 = s(HEADER_H, scale) + pad;
    let font_main = make_font(FONT_HANJA, scale, true);
    let font_mean = make_font(FONT_MEANING, scale, false);
    let font_label = make_font(FONT_LABEL, scale, true);
    for r in 0..rs.rows {
        let Some(cell) = rs.cell_at(r, 0) else { continue };
        let y = top0 + (r as i32) * row_h;
        let row_rect = RECT {
            left: pad,
            top: y,
            right: w - pad,
            bottom: y + row_h,
        };
        let has_data = cell.f & cell_flags::HAS_DATA != 0;
        if !has_data {
            continue; // 빈 셀: 텍스트 없음.
        }
        // 선택 판정 — 좌표 비교 1차 (§5.4 H4 재발 금지).
        let selected = r == rs.sel_row && rs.sel_col == 0;
        log_parity(cell.f, selected, r, 0);
        if selected {
            let bg = if flash_on { pal.flash } else { pal.sel_bg(rs.kind) };
            fill(hdc, &row_rect, bg);
            // 2px 대비 링(형태 단서) — 색 구분이 어려운 사용자도 선택 위치 인지.
            draw_ring(hdc, &row_rect, pal.sel_ring, s(2, scale));
        } else if cell.f & cell_flags::ROW_HIGHLIGHT != 0 {
            fill(hdc, &row_rect, pal.rowhl);
        }
        let text_color = if selected { pal.on_sel_text } else { pal.text };
        let mean_color = if selected { pal.on_sel_text } else { pal.subtext0 };
        // 행 레이블 (1.~9.).
        let label = rs.row_headers.get(r as usize).map(|h| h.0.as_str()).unwrap_or("");
        let old = SelectObject(hdc, font_label.into());
        let label_rect = RECT { left: pad + s(4, scale), top: y, right: pad + s(ROW_LABEL_W, scale), bottom: y + row_h };
        draw_text(hdc, label, &label_rect, if selected { pal.on_sel_text } else { pal.overlay1 }, DT_VCENTER | DT_SINGLELINE | DT_LEFT);
        SelectObject(hdc, old);
        // 한자 (+★).
        let old = SelectObject(hdc, font_main.into());
        let star = if cell.f & cell_flags::BOOKMARKED != 0 { " ★" } else { "" };
        let main = format!("{}{star}", cell.t);
        let hanja_left = pad + s(ROW_LABEL_W, scale) + s(4, scale);
        let hanja_rect = RECT { left: hanja_left, top: y, right: hanja_left + s(90, scale), bottom: y + row_h };
        let star_color = if selected { pal.on_sel_text } else { pal.star };
        // 한자+★ 를 한 번에 그리되 ★ 색은 별도 처리가 어렵다 → 본문색으로 일괄,
        // 단 미선택 시 ★ 강조를 위해 별색 그리기.
        if star.is_empty() || selected {
            draw_text(hdc, &main, &hanja_rect, text_color, DT_VCENTER | DT_SINGLELINE | DT_LEFT);
        } else {
            draw_text(hdc, &cell.t, &hanja_rect, text_color, DT_VCENTER | DT_SINGLELINE | DT_LEFT);
            let tw = text_width(hdc, &cell.t, font_main);
            let star_rect = RECT { left: hanja_left + tw + s(2, scale), top: y, right: hanja_left + s(90, scale), bottom: y + row_h };
            draw_text(hdc, "★", &star_rect, star_color, DT_VCENTER | DT_SINGLELINE | DT_LEFT);
        }
        SelectObject(hdc, old);
        // 뜻풀이 (인라인, 보조색).
        let old = SelectObject(hdc, font_mean.into());
        let mean_left = hanja_left + s(96, scale);
        let mean_rect = RECT { left: mean_left, top: y, right: w - pad, bottom: y + row_h };
        draw_text(hdc, &cell.m, &mean_rect, mean_color, DT_VCENTER | DT_SINGLELINE | DT_LEFT);
        SelectObject(hdc, old);
    }
    let _ = DeleteObject(font_main.into());
    let _ = DeleteObject(font_mean.into());
    let _ = DeleteObject(font_label.into());
}

unsafe fn paint_grid(
    hdc: HDC,
    rs: &RenderState,
    scale: f64,
    _w: i32,
    _h: i32,
    flash_on: bool,
    emoji_cells: &mut Vec<crate::d2d::EmojiCell>,
    pal: &Palette,
) {
    let pad = s(PAD, scale);
    let emoji = !rs.tab_labels.is_empty();
    // 컬러 이모지 셀 텍스트는 D2D 가 담당 (§11.A) → kind==2 격자만.
    let color_emoji = rs.kind == 2;
    let tab_w = if emoji { s(TAB_W, scale) } else { 0 };
    let body_top = s(HEADER_H, scale) + pad;
    let grid_left = pad + tab_w;
    // 격자 셀 시작 y(열 레이블 아래). 이모지 탭도 여기서 시작해 격자 행과 1:1 정렬한다.
    let cells_top = body_top + s(COL_LABEL_H, scale);

    // 이모지 좌측 9 카테고리 탭.
    if emoji {
        let font_tab = make_font(FONT_LABEL, scale, true);
        let old = SelectObject(hdc, font_tab.into());
        for (i, label) in rs.tab_labels.iter().enumerate() {
            // 탭을 격자 행과 1:1 정렬: 열 레이블 아래(cells_top)에서 CELL_H 간격·높이로.
            let y = cells_top + (i as i32) * s(CELL_H, scale);
            let tab_rect = RECT { left: pad, top: y, right: pad + tab_w - s(4, scale), bottom: y + s(CELL_H, scale) - s(2, scale) };
            let active = (i as u32) == rs.active_tab_index;
            if active {
                fill(hdc, &tab_rect, pal.sel_green);
                // 활성 탭에도 대비 링 — 색 단독 의존 완화(형태 단서).
                draw_ring(hdc, &tab_rect, pal.sel_ring, s(2, scale));
            } else {
                fill(hdc, &tab_rect, pal.surface0);
            }
            // 카테고리 레이블 우측 정렬 (POPUP_SPEC §11 v3.2): 레이블 끝에 붙은
            // 선택키 "(a)/(s)/..." 가 모든 행에서 같은 우측 기준선에 정렬되어
            // 세로 열로 보이게 한다. 탭 영역폭(tab_w) 고정이므로 DT_RIGHT 만으로
            // 충분 — 별도 폭 측정 불필요. 이모지 모드(tab_labels 비어있지 않음)에서만 그려진다.
            let mut tr = tab_rect;
            tr.right -= s(6, scale);
            draw_text(hdc, label, &tr, if active { pal.on_sel_text } else { pal.text }, DT_VCENTER | DT_SINGLELINE | DT_RIGHT);
        }
        SelectObject(hdc, old);
        let _ = DeleteObject(font_tab.into());
    }

    // 상단 열 레이블 (항상 9개 — 팝업 폭 고정 정책).
    let font_label = make_font(FONT_LABEL, scale, true);
    let old = SelectObject(hdc, font_label.into());
    let cells_left = grid_left + s(ROW_LABEL_W, scale);
    for c in 0..GRID_COLS_FIXED {
        let (text, active) = rs
            .col_headers
            .get(c as usize)
            .map(|h| (h.0.as_str(), h.1))
            .unwrap_or(("", false));
        let x = cells_left + c * s(CELL_W, scale);
        let lr = RECT { left: x, top: body_top, right: x + s(CELL_W, scale), bottom: body_top + s(COL_LABEL_H, scale) };
        let col = if active { pal.hdr_active } else { pal.hdr_inactive };
        draw_text(hdc, text, &lr, col, DT_VCENTER | DT_SINGLELINE | DT_CENTER);
        // 활성 열 헤더 형태 단서: 하단 2px 밑줄. 색(활성=녹/비활성=황) 단독 의존을
        // 완화해 색약·고대비 사용자도 활성 열을 형태로 구분(WCAG 1.4.11).
        if active && !text.is_empty() {
            let ul = RECT {
                left: x + s(6, scale),
                top: lr.bottom - s(2, scale),
                right: x + s(CELL_W, scale) - s(6, scale),
                bottom: lr.bottom,
            };
            fill(hdc, &ul, pal.hdr_active);
        }
    }
    // 좌측 행 레이블. (cells_top 은 위에서 정의)
    for r in 0..rs.rows {
        let (text, active) = rs
            .row_headers
            .get(r as usize)
            .map(|h| (h.0.as_str(), h.1))
            .unwrap_or(("", false));
        let y = cells_top + (r as i32) * s(CELL_H, scale);
        let lr = RECT { left: grid_left, top: y, right: grid_left + s(ROW_LABEL_W, scale), bottom: y + s(CELL_H, scale) };
        let col = if active { pal.hdr_active } else { pal.overlay1 };
        draw_text(hdc, text, &lr, col, DT_VCENTER | DT_SINGLELINE | DT_CENTER);
        // 활성 행 헤더 형태 단서: 좌측 2px 세로 막대. 색 단독 의존 완화.
        if active && !text.is_empty() {
            let bar = RECT {
                left: grid_left,
                top: y + s(4, scale),
                right: grid_left + s(2, scale),
                bottom: y + s(CELL_H, scale) - s(4, scale),
            };
            fill(hdc, &bar, pal.hdr_active);
        }
    }
    SelectObject(hdc, old);
    let _ = DeleteObject(font_label.into());

    // 격자 셀 — column-major 접근만.
    let font_cell = make_font(FONT_CELL, scale, false);
    let old = SelectObject(hdc, font_cell.into());
    for c in 0..GRID_COLS_FIXED as u32 {
        for r in 0..rs.rows {
            let x = cells_left + (c as i32) * s(CELL_W, scale);
            let y = cells_top + (r as i32) * s(CELL_H, scale);
            let cr = RECT { left: x, top: y, right: x + s(CELL_W, scale) - s(2, scale), bottom: y + s(CELL_H, scale) - s(2, scale) };
            // cols 범위 밖 (col_headers 는 9개지만 데이터는 cols 열만) → 빈 셀.
            let cell = if c < rs.cols { rs.cell_at(r, c) } else { None };
            match cell {
                Some(cell) if cell.f & cell_flags::HAS_DATA != 0 => {
                    let selected = r == rs.sel_row && c == rs.sel_col;
                    log_parity(cell.f, selected, r, c);
                    if selected {
                        let bg = if flash_on { pal.flash } else { pal.sel_bg(rs.kind) };
                        fill(hdc, &cr, bg);
                        // 2px 대비 링(형태 단서, WCAG 1.4.11).
                        draw_ring(hdc, &cr, pal.sel_ring, s(2, scale));
                    } else if cell.f & cell_flags::ROW_HIGHLIGHT != 0 {
                        fill(hdc, &cr, pal.rowhl);
                    } else if cell.f & cell_flags::COL_HIGHLIGHT != 0 {
                        fill(hdc, &cr, pal.colhl);
                    }
                    let text_color = if selected { pal.on_sel_text } else { pal.text };
                    let main = if cell.f & cell_flags::BOOKMARKED != 0 {
                        format!("{} ★", cell.t)
                    } else {
                        cell.t.clone()
                    };
                    if color_emoji {
                        // 글리프는 GDI 로 그리지 않고 모았다가 D2D 컬러 폰트로 (§11.A).
                        emoji_cells.push(crate::d2d::EmojiCell {
                            text: main,
                            rect: cr,
                            on_selected: selected,
                        });
                    } else {
                        draw_text(hdc, &main, &cr, text_color, DT_VCENTER | DT_SINGLELINE | DT_CENTER);
                    }
                }
                _ => {
                    // 빈 셀: 연한 배경, 텍스트 없음.
                    fill(hdc, &cr, pal.empty);
                }
            }
        }
    }
    SelectObject(hdc, old);
    let _ = DeleteObject(font_cell.into());
}

/// 마우스 클릭(클라이언트 좌표 px)을 격자/탭/footer 영역에 매핑한다 (§11.E).
///
/// **paint 와 동일 모듈·동일 상수·동일 `s()` 로 작성** — rect 가 1px라도 어긋나면
/// 오선택(§11.I). 격자 셀 존재 확인은 반드시 column-major `cell_at(row,col)` 사용
/// (= cells[col*rows+row]; row-major 환산 금지, H3 재발 방지).
///
/// `(x, y)` 는 창 클라이언트 좌표, `(w, h)` 는 클라이언트 폭/높이(footer 영역 계산용).
/// footer 의 페이지/expand 버튼을 먼저 본 뒤 본문(격자/탭/compact)을 본다.
pub fn hit_test(rs: &RenderState, scale: f64, x: i32, y: i32, w: i32, h: i32) -> Option<RevEvent> {
    // footer ◀/▶/⊞/⊟ 우선 (창 맨 아래 — 본문과 y 가 겹치지 않음).
    if let Some(ev) = hit_test_footer(rs, scale, x, y, w, h) {
        return Some(ev);
    }
    // 본문 영역.
    if rs.is_compact() {
        hit_test_compact(rs, scale, x, y)
    } else {
        hit_test_grid(rs, scale, x, y)
    }
}

/// footer 전용 히트테스트 — footer top = client_h - s(FOOTER_H).
/// page ◀/▶ 와 expand ⊞/⊟ 만 판정. paint 의 footer 좌표와 1:1.
fn hit_test_footer(rs: &RenderState, scale: f64, x: i32, y: i32, w: i32, h: i32) -> Option<RevEvent> {
    use crate::protocol::evt_kind;
    if !rs.show_footer {
        return None;
    }
    let pad = s(PAD, scale);
    let footer_top = h - s(FOOTER_H, scale);
    if y < footer_top || y >= h {
        return None;
    }
    // expand ⊞/⊟ — 우측 끝 (paint 와 동일). page 우측 ▶ 와 상호배타(겹침 방지).
    if rs.expand_visible {
        let exp_left = w - pad - s(EXPAND_BTN_W, scale);
        if x >= exp_left && x < w - pad {
            return Some(RevEvent::ExpandToggle);
        }
    }
    if rs.total_pages > 1 {
        // 좌측 ◀ (Prev).
        let prev_right = pad + s(PAGE_BTN_W, scale);
        if x >= pad && x < prev_right {
            return Some(RevEvent::PageClick { dir: evt_kind::PAGE_DIR_PREV });
        }
        // 우측 ▶ (Next) — expand 가 우측을 차지하지 않을 때만.
        if !rs.expand_visible {
            let next_left = w - pad - s(PAGE_BTN_W, scale);
            if x >= next_left && x < w - pad {
                return Some(RevEvent::PageClick { dir: evt_kind::PAGE_DIR_NEXT });
            }
        }
    }
    None
}

/// 격자 모드 셀/탭 히트테스트 — paint_grid 와 동일 좌표 공식.
fn hit_test_grid(rs: &RenderState, scale: f64, x: i32, y: i32) -> Option<RevEvent> {
    let pad = s(PAD, scale);
    let emoji = !rs.tab_labels.is_empty();
    let tab_w = if emoji { s(TAB_W, scale) } else { 0 };
    let body_top = s(HEADER_H, scale) + pad;
    let grid_left = pad + tab_w;
    // 격자 셀 시작 y(열 레이블 아래). 탭도 여기서 시작해 행과 1:1 정렬(paint_grid 와 동일 공식).
    let cells_top = body_top + s(COL_LABEL_H, scale);

    // 이모지 좌측 탭 (paint_grid 의 tab_rect 와 동일: (pad, cells_top + i*CELL_H)).
    if emoji {
        let tab_h = s(CELL_H, scale);
        // 탭 가시 폭: paint 는 [pad, pad + tab_w - s(4)]. 적중 폭은 [pad, pad+tab_w) 로 둔다.
        if x >= pad && x < pad + tab_w && y >= cells_top {
            let i = (y - cells_top) / tab_h;
            if i >= 0 && (i as usize) < rs.tab_labels.len() {
                return Some(RevEvent::TabClick { index: i as u32 });
            }
        }
    }

    // 격자 셀 (paint_grid: cells_left = grid_left + ROW_LABEL_W).
    let cells_left = grid_left + s(ROW_LABEL_W, scale);
    if x >= cells_left && y >= cells_top {
        let col = (x - cells_left) / s(CELL_W, scale);
        let row = (y - cells_top) / s(CELL_H, scale);
        if (0..GRID_COLS_FIXED).contains(&col) && row >= 0 && (row as u32) < rs.rows {
            let (row, col) = (row as u32, col as u32);
            // 반드시 column-major cell_at 으로 HAS_DATA 확인 (§11.I).
            if let Some(cell) = rs.cell_at(row, col) {
                if cell.f & cell_flags::HAS_DATA != 0 {
                    return Some(RevEvent::CellClick { row, col });
                }
            }
        }
    }
    None
}

/// 한자 compact 행 히트테스트 — paint_compact 와 동일 좌표 공식.
fn hit_test_compact(rs: &RenderState, scale: f64, x: i32, y: i32) -> Option<RevEvent> {
    let pad = s(PAD, scale);
    let top0 = s(HEADER_H, scale) + pad;
    let row_h = s(COMPACT_ROW_H, scale);
    // compact 행은 [pad .. w-pad] 가로폭 전체. x 가 좌측 여백 밖이면 무시.
    if x < pad || y < top0 {
        return None;
    }
    let row = (y - top0) / row_h;
    if row < 0 || (row as u32) >= rs.rows {
        return None;
    }
    let row = row as u32;
    if let Some(cell) = rs.cell_at(row, 0) {
        if cell.f & cell_flags::HAS_DATA != 0 {
            return Some(RevEvent::CellClick { row, col: 0 });
        }
    }
    None
}

/// 좌표 기준 선택과 SELECTED 플래그가 어긋나면 경고 로그 (§5.4 디버그 신호).
/// 렌더는 좌표 기준을 따른다.
#[inline]
fn log_parity(flags: u32, coord_selected: bool, row: u32, col: u32) {
    let flag_selected = flags & cell_flags::SELECTED != 0;
    if flag_selected != coord_selected {
        logln!(
            "parity-mismatch (row={row},col={col}) coord_selected={coord_selected} \
             flag_selected={flag_selected} — rendering by coordinate"
        );
    }
}
