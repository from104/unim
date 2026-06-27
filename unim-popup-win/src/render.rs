//! GDI 렌더 — 설계서 §5.4 (레이아웃·색·폰트·flash) 가 SoT.
//!
//! **금지**: 구 popup_window.rs 의 row-major 루프 포팅 금지. 셀 접근은 오로지
//! `RenderState::cell_at(row, col)` (= cells[col*rows+row]). 선택은 좌표 비교 1차.
//!
//! 모든 레이아웃 상수는 논리값. 실제 픽셀 = 논리값 × scale (DPI, §5.5).

use windows::core::PCWSTR;
use windows::Win32::Foundation::{COLORREF, RECT, SIZE};
use windows::Win32::Graphics::Gdi::*;

use crate::logln;
use crate::protocol::{cell_flags, RenderState, RevEvent};

// ─── Catppuccin Mocha (COLORREF = 0x00BBGGRR — BGR 주의, 설계서 §5.4) ────────
const C_BASE: COLORREF = COLORREF(0x002e1e1e); // #1e1e2e 배경
const C_SURFACE0: COLORREF = COLORREF(0x00443231); // #313244 헤더/푸터 배경
const C_TEXT: COLORREF = COLORREF(0x00f4d6cd); // #cdd6f4 본문
const C_SUBTEXT0: COLORREF = COLORREF(0x00c8ada6); // #a6adc8 뜻풀이
const C_OVERLAY1: COLORREF = COLORREF(0x009c847f); // #7f849c 행/열 레이블
const C_SEL_HANJA: COLORREF = COLORREF(0x00fab489); // #89b4fa 한자 선택 배경 (blue)
const C_SEL_GREEN: COLORREF = COLORREF(0x00a1e3a6); // #a6e3a1 특수/이모지 선택 배경
const C_FLASH: COLORREF = COLORREF(0x00afe2f9); // #f9e2af flash / 비활성 열 헤더
const C_HDR_ACTIVE: COLORREF = COLORREF(0x00a1e3a6); // #a6e3a1 활성 열 헤더 (green)
const C_HDR_INACTIVE: COLORREF = COLORREF(0x00afe2f9); // #f9e2af 비활성 열 헤더 (yellow)
const C_EMPTY: COLORREF = COLORREF(0x003a2a2a); // base 보다 약간 밝은 빈 셀
const C_ROWHL: COLORREF = COLORREF(0x00403030); // 행 보조 강조 (연한 배경)
const C_COLHL: COLORREF = COLORREF(0x00303a30); // 열 보조 강조
const C_BLACK: COLORREF = COLORREF(0x00000000); // 선택 셀 위 텍스트 (대비)
const C_STAR: COLORREF = COLORREF(0x00afe2f9); // 북마크 ★ (yellow)

// ─── 논리 레이아웃 상수 ─────────────────────────────────────────────────────
const PAD: i32 = 8; // 외곽 여백
const HEADER_H: i32 = 28; // 헤더 바 높이
const FOOTER_H: i32 = 24; // 푸터 바 높이
const CELL_W: i32 = 44; // 격자 셀 폭 (§11.B 54→44 폭 축소; 비-compact 524→434px)
const CELL_H: i32 = 34; // 격자 셀 높이
const ROW_LABEL_W: i32 = 22; // 좌측 행 레이블 열 폭
const COL_LABEL_H: i32 = 22; // 상단 열 레이블 행 높이
const TAB_W: i32 = 76; // 이모지 좌측 탭 컬럼 폭 (§11.B 88→76)
const TAB_H: i32 = 30; // 이모지 탭 1개 높이
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
    let grid_h = s(COL_LABEL_H, scale) + (rs.rows as i32) * s(CELL_H, scale);
    // 이모지 탭 9개 높이가 격자보다 크면 그쪽을 따른다.
    let tab_h = if emoji { 9 * s(TAB_H, scale) } else { 0 };
    let body_h = grid_h.max(tab_h);
    let h = s(HEADER_H, scale) + body_h + footer + s(PAD * 2, scale);
    (w, h)
}

/// 전체 렌더. `flash_on` = 현재 flash 타이머 활성 여부 (선택 셀 노란 배경).
pub unsafe fn paint(hdc: HDC, rs: &RenderState, scale: f64, w: i32, h: i32, flash_on: bool) {
    // 배경.
    let full = RECT { left: 0, top: 0, right: w, bottom: h };
    fill(hdc, &full, C_BASE);
    SetBkMode(hdc, TRANSPARENT);

    // 헤더 바.
    let pad = s(PAD, scale);
    let header_rect = RECT {
        left: 0,
        top: 0,
        right: w,
        bottom: s(HEADER_H, scale),
    };
    fill(hdc, &header_rect, C_SURFACE0);
    let font_header = make_font(FONT_HEADER, scale, true);
    let old = SelectObject(hdc, font_header.into());
    let mut hr = header_rect;
    hr.left += pad;
    hr.right -= pad;
    draw_text(hdc, &rs.header_text, &hr, C_TEXT, DT_VCENTER | DT_SINGLELINE | DT_LEFT);
    SelectObject(hdc, old);
    let _ = DeleteObject(font_header.into());

    // 본문. 이모지 격자 셀 텍스트는 GDI 로 그리지 않고 모아서(emoji_cells) 마지막에
    // D2D 컬러 폰트로 덧그린다 (§11.A — GDI 전부 → D2D 이모지 텍스트 순서).
    let mut emoji_cells: Vec<crate::d2d::EmojiCell> = Vec::new();
    if rs.is_compact() {
        paint_compact(hdc, rs, scale, w, h, flash_on);
    } else {
        paint_grid(hdc, rs, scale, w, h, flash_on, &mut emoji_cells);
    }

    // 푸터 바.
    if rs.show_footer {
        let footer_rect = RECT {
            left: 0,
            top: h - s(FOOTER_H, scale),
            right: w,
            bottom: h,
        };
        fill(hdc, &footer_rect, C_SURFACE0);
        let font_f = make_font(FONT_MEANING, scale, false);
        let old = SelectObject(hdc, font_f.into());
        let mut fr = footer_rect;
        fr.left += pad;
        fr.right -= pad;
        draw_text(hdc, &rs.footer_text, &fr, C_SUBTEXT0, DT_VCENTER | DT_SINGLELINE | DT_CENTER);
        // 페이지 ◀/▶ 글리프 (§11.E — total_pages>1 일 때만, hit_test 와 동일 좌표).
        if rs.total_pages > 1 {
            // 좌측 ◀ (Prev).
            let prev_rect = RECT {
                left: pad,
                top: footer_rect.top,
                right: pad + s(PAGE_BTN_W, scale),
                bottom: footer_rect.bottom,
            };
            draw_text(hdc, "◀", &prev_rect, C_TEXT, DT_VCENTER | DT_SINGLELINE | DT_CENTER);
            // 우측 ▶ (Next) — expand 가 우측을 차지하지 않을 때만 (겹침 방지).
            if !rs.expand_visible {
                let next_rect = RECT {
                    left: w - pad - s(PAGE_BTN_W, scale),
                    top: footer_rect.top,
                    right: w - pad,
                    bottom: footer_rect.bottom,
                };
                draw_text(hdc, "▶", &next_rect, C_TEXT, DT_VCENTER | DT_SINGLELINE | DT_CENTER);
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
            draw_text(hdc, &rs.expand_text, &exp_rect, C_TEXT, DT_VCENTER | DT_SINGLELINE | DT_CENTER);
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
                let color = if c.on_selected { C_BLACK } else { C_TEXT };
                draw_text(hdc, &c.text, &c.rect, color, DT_VCENTER | DT_SINGLELINE | DT_CENTER);
            }
            SelectObject(hdc, old);
            let _ = DeleteObject(font_cell.into());
            logln!("paint: D2D emoji unavailable — GDI monochrome fallback ({} cells)", emoji_cells.len());
        }
    }
}

unsafe fn paint_compact(hdc: HDC, rs: &RenderState, scale: f64, w: i32, _h: i32, flash_on: bool) {
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
            let bg = if flash_on { C_FLASH } else { sel_color(rs.kind) };
            fill(hdc, &row_rect, bg);
        } else if cell.f & cell_flags::ROW_HIGHLIGHT != 0 {
            fill(hdc, &row_rect, C_ROWHL);
        }
        let text_color = if selected { C_BLACK } else { C_TEXT };
        let mean_color = if selected { C_BLACK } else { C_SUBTEXT0 };
        // 행 레이블 (1.~9.).
        let label = rs.row_headers.get(r as usize).map(|h| h.0.as_str()).unwrap_or("");
        let old = SelectObject(hdc, font_label.into());
        let label_rect = RECT { left: pad + s(4, scale), top: y, right: pad + s(ROW_LABEL_W, scale), bottom: y + row_h };
        draw_text(hdc, label, &label_rect, if selected { C_BLACK } else { C_OVERLAY1 }, DT_VCENTER | DT_SINGLELINE | DT_LEFT);
        SelectObject(hdc, old);
        // 한자 (+★).
        let old = SelectObject(hdc, font_main.into());
        let star = if cell.f & cell_flags::BOOKMARKED != 0 { " ★" } else { "" };
        let main = format!("{}{star}", cell.t);
        let hanja_left = pad + s(ROW_LABEL_W, scale) + s(4, scale);
        let hanja_rect = RECT { left: hanja_left, top: y, right: hanja_left + s(90, scale), bottom: y + row_h };
        let star_color = if selected { C_BLACK } else { C_STAR };
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
) {
    let pad = s(PAD, scale);
    let emoji = !rs.tab_labels.is_empty();
    // 컬러 이모지 셀 텍스트는 D2D 가 담당 (§11.A) → kind==2 격자만.
    let color_emoji = rs.kind == 2;
    let tab_w = if emoji { s(TAB_W, scale) } else { 0 };
    let body_top = s(HEADER_H, scale) + pad;
    let grid_left = pad + tab_w;

    // 이모지 좌측 9 카테고리 탭.
    if emoji {
        let font_tab = make_font(FONT_LABEL, scale, true);
        let old = SelectObject(hdc, font_tab.into());
        for (i, label) in rs.tab_labels.iter().enumerate() {
            let y = body_top + (i as i32) * s(TAB_H, scale);
            let tab_rect = RECT { left: pad, top: y, right: pad + tab_w - s(4, scale), bottom: y + s(TAB_H, scale) - s(2, scale) };
            let active = (i as u32) == rs.active_tab_index;
            if active {
                fill(hdc, &tab_rect, C_SEL_GREEN);
            } else {
                fill(hdc, &tab_rect, C_SURFACE0);
            }
            // 카테고리 레이블 우측 정렬 (POPUP_SPEC §11 v3.2): 레이블 끝에 붙은
            // 선택키 "(a)/(s)/..." 가 모든 행에서 같은 우측 기준선에 정렬되어
            // 세로 열로 보이게 한다. 탭 영역폭(tab_w) 고정이므로 DT_RIGHT 만으로
            // 충분 — 별도 폭 측정 불필요. 이모지 모드(tab_labels 비어있지 않음)에서만 그려진다.
            let mut tr = tab_rect;
            tr.right -= s(6, scale);
            draw_text(hdc, label, &tr, if active { C_BLACK } else { C_TEXT }, DT_VCENTER | DT_SINGLELINE | DT_RIGHT);
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
        let col = if active { C_HDR_ACTIVE } else { C_HDR_INACTIVE };
        draw_text(hdc, text, &lr, col, DT_VCENTER | DT_SINGLELINE | DT_CENTER);
    }
    // 좌측 행 레이블.
    let cells_top = body_top + s(COL_LABEL_H, scale);
    for r in 0..rs.rows {
        let (text, active) = rs
            .row_headers
            .get(r as usize)
            .map(|h| (h.0.as_str(), h.1))
            .unwrap_or(("", false));
        let y = cells_top + (r as i32) * s(CELL_H, scale);
        let lr = RECT { left: grid_left, top: y, right: grid_left + s(ROW_LABEL_W, scale), bottom: y + s(CELL_H, scale) };
        let col = if active { C_HDR_ACTIVE } else { C_OVERLAY1 };
        draw_text(hdc, text, &lr, col, DT_VCENTER | DT_SINGLELINE | DT_CENTER);
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
                        let bg = if flash_on { C_FLASH } else { sel_color(rs.kind) };
                        fill(hdc, &cr, bg);
                    } else if cell.f & cell_flags::ROW_HIGHLIGHT != 0 {
                        fill(hdc, &cr, C_ROWHL);
                    } else if cell.f & cell_flags::COL_HIGHLIGHT != 0 {
                        fill(hdc, &cr, C_COLHL);
                    }
                    let text_color = if selected { C_BLACK } else { C_TEXT };
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
                    fill(hdc, &cr, C_EMPTY);
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

    // 이모지 좌측 탭 (paint_grid 의 tab_rect 와 동일: (pad, body_top + i*s(TAB_H))).
    if emoji {
        let tab_h = s(TAB_H, scale);
        // 탭 가시 폭: paint 는 [pad, pad + tab_w - s(4)]. 적중 폭은 [pad, pad+tab_w) 로 둔다.
        if x >= pad && x < pad + tab_w && y >= body_top {
            let i = (y - body_top) / tab_h;
            if i >= 0 && (i as usize) < rs.tab_labels.len() {
                return Some(RevEvent::TabClick { index: i as u32 });
            }
        }
    }

    // 격자 셀 (paint_grid: cells_left = grid_left + ROW_LABEL_W, cells_top = body_top + COL_LABEL_H).
    let cells_left = grid_left + s(ROW_LABEL_W, scale);
    let cells_top = body_top + s(COL_LABEL_H, scale);
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

#[inline]
fn sel_color(kind: u32) -> COLORREF {
    // 0=Hanja → blue, 1=SpecialChar / 2=Emoji → green (POPUP_SPEC §5.1).
    if kind == 0 {
        C_SEL_HANJA
    } else {
        C_SEL_GREEN
    }
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
