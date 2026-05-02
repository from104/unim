//! Wayland 팝업 소프트웨어 렌더러
//!
//! tiny-skia + cosmic-text로 Catppuccin Mocha 테마 팝업을 RGBA 버퍼로 렌더링합니다.
//! 결과 버퍼는 wl_shm를 통해 팝업 서피스에 attach됩니다.

use cosmic_text::{Attrs, Buffer, Color, FontSystem, Metrics, Shaping, SwashCache};
use tiny_skia::{Paint, PathBuilder, Pixmap, Rect, Transform};
use unim::popup::{PopupKind, PopupState};
use unim::unim_log;

// ─── Catppuccin Mocha 색상 ───
const BASE: Color = Color::rgb(0x1e, 0x1e, 0x2e);
const SURFACE0: Color = Color::rgb(0x31, 0x32, 0x44);
const OVERLAY0: Color = Color::rgb(0x6c, 0x70, 0x86);
const OVERLAY1: Color = Color::rgb(0x7f, 0x84, 0x9c);
const SUBTEXT0: Color = Color::rgb(0xa6, 0xad, 0xc8);
const TEXT: Color = Color::rgb(0xcd, 0xd6, 0xf4);
const BLUE: Color = Color::rgb(0x89, 0xb4, 0xfa);
const GREEN: Color = Color::rgb(0xa6, 0xe3, 0xa1);
const YELLOW: Color = Color::rgb(0xf9, 0xe2, 0xaf);
const SEL_HANJA_BG: Color = Color::rgb(0x33, 0x3c, 0x57); // rgba(137,180,250,0.2) on #1e1e2e
const SEL_SPECIAL_BG: Color = Color::rgb(0x40, 0x4f, 0x4b); // rgba(166,227,161,0.25) on #1e1e2e

// ─── 크기 상수 ───
const FONT_SIZE: f32 = 14.0;
const LINE_HEIGHT: f32 = 1.4;
const PADDING: f32 = 12.0;
const HEADER_H: f32 = 28.0;
const ROW_H: f32 = 26.0;
const CELL_SIZE: f32 = 30.0;

/// 렌더링 결과
pub struct RenderedPopup {
    pub pixels: Vec<u8>, // ARGB32 (wl_shm 용)
    pub width: u32,
    pub height: u32,
}

/// PopupState 기반 통합 렌더링 진입점
pub fn render_popup(state: &PopupState) -> RenderedPopup {
    match state.kind() {
        PopupKind::Hanja => {
            if state.is_hanja_expanded() {
                render_hanja_expanded(state)
            } else {
                render_hanja_compact(state)
            }
        }
        PopupKind::SpecialChar => render_special_from_state(state),
        // PR #5: 이모지 팝업 — 9×9 그리드 + 좌측 9 탭 + 하단 페이지 인디케이터.
        PopupKind::Emoji => render_emoji_from_state(state),
    }
}

/// 한자 팝업 compact(1×9) 모드 렌더링.
///
/// 행 우측에 ☆/★ 표시(즐겨찾기 여부)를 추가하고, 즐겨찾기 셀의 한자 글자색을
/// Catppuccin yellow(#f9e2af, [`YELLOW`]) 로 강조하여 다른 프런트엔드(XIM,
/// GTK Standalone, GTK IM, Qt IM, GNOME extension)와 시각 일관성을 맞춘다.
fn render_hanja_compact(state: &PopupState) -> RenderedPopup {
    let target = state.target();
    let page_items = state.hanja_page_items();
    let selected = state.sel_row();
    let current_page = state.current_page();
    let total_pages = state.total_pages();
    let item_count = page_items.len();

    let width = 360u32;
    let height = (PADDING + HEADER_H + 4.0 + (item_count as f32) * ROW_H + ROW_H + PADDING) as u32;

    let mut pixmap = Pixmap::new(width, height).unwrap();
    fill_rect(&mut pixmap, 0.0, 0.0, width as f32, height as f32, BASE);
    fill_rect(
        &mut pixmap,
        4.0,
        4.0,
        width as f32 - 8.0,
        HEADER_H,
        SURFACE0,
    );

    let mut font_system = FontSystem::new();
    let mut swash_cache = SwashCache::new();

    let header_text = format!("「{}」 → 한자", target);
    draw_text(
        &mut pixmap,
        &mut font_system,
        &mut swash_cache,
        &header_text,
        PADDING,
        8.0,
        FONT_SIZE,
        BLUE,
        (width as f32 - PADDING * 2.0) * 0.7,
    );

    let page_text = format!("{}/{}", current_page + 1, total_pages.max(1));
    draw_text_right(
        &mut pixmap,
        &mut font_system,
        &mut swash_cache,
        &page_text,
        width as f32 - PADDING,
        8.0,
        FONT_SIZE - 2.0,
        OVERLAY0,
    );

    let page_size = 9; // compact 모드 page_size (HANJA_PAGE_SIZE)
    let items_y = PADDING + HEADER_H + 4.0;
    let star_w = 18.0;
    let star_x = width as f32 - PADDING - star_w;
    for (i, (hanja, meaning)) in page_items.iter().enumerate() {
        let row_y = items_y + (i as f32) * ROW_H;
        let global_idx = current_page * page_size + i;
        let bookmarked = state.is_bookmarked(global_idx);

        if i == selected {
            fill_rect(
                &mut pixmap,
                4.0,
                row_y,
                width as f32 - 8.0,
                ROW_H,
                SEL_HANJA_BG,
            );
        }
        let text_y = row_y + 2.0;
        let num = format!("{}.", i + 1);
        draw_text(
            &mut pixmap,
            &mut font_system,
            &mut swash_cache,
            &num,
            PADDING,
            text_y,
            FONT_SIZE - 1.0,
            OVERLAY1,
            24.0,
        );
        // 즐겨찾기는 한자 글자색을 노랑으로 강조 (XIM hanja_window.rs:1007 패턴).
        let hanja_color = if bookmarked { YELLOW } else { TEXT };
        draw_text(
            &mut pixmap,
            &mut font_system,
            &mut swash_cache,
            hanja,
            PADDING + 28.0,
            text_y,
            FONT_SIZE + 2.0,
            hanja_color,
            60.0,
        );
        if !meaning.is_empty() {
            draw_text(
                &mut pixmap,
                &mut font_system,
                &mut swash_cache,
                meaning,
                PADDING + 90.0,
                text_y + 2.0,
                FONT_SIZE - 2.0,
                SUBTEXT0,
                width as f32 - PADDING * 2.0 - 90.0 - star_w - 4.0,
            );
        }
        // ☆/★ 별 표시 (행 우측 끝). XIM hanja_window.rs:851 패턴.
        let star_text = if bookmarked { "★" } else { "☆" };
        let star_color = if bookmarked { YELLOW } else { OVERLAY0 };
        draw_text(
            &mut pixmap,
            &mut font_system,
            &mut swash_cache,
            star_text,
            star_x,
            text_y,
            FONT_SIZE,
            star_color,
            star_w,
        );
    }

    let footer_y = items_y + (item_count as f32) * ROW_H + 4.0;
    draw_text(
        &mut pixmap,
        &mut font_system,
        &mut swash_cache,
        "← → 페이지 | 1~9 선택 | Space ★ | . 확장 | ESC 취소",
        PADDING,
        footer_y,
        FONT_SIZE - 3.0,
        OVERLAY0,
        width as f32 - PADDING * 2.0,
    );

    let pixels = rgba_to_argb32(pixmap.data());
    unim_log!(
        "WAYLAND",
        "한자 팝업 렌더링(compact): {}×{}, {} 후보",
        width,
        height,
        item_count
    );
    RenderedPopup {
        pixels,
        width,
        height,
    }
}

/// 한자 팝업 expanded(9×9) 모드 렌더링.
///
/// 특수문자 그리드와 같은 9×9 셀 레이아웃이며, 즐겨찾기된 한자는 셀 글자색을
/// Catppuccin yellow(#f9e2af) 로 강조한다 (XIM hanja_window.rs:1005~1015 패턴).
fn render_hanja_expanded(state: &PopupState) -> RenderedPopup {
    let rows = state.rows();
    let cols = state.cols();
    let sel_row = state.sel_row();
    let sel_col = state.sel_col();
    let current_page = state.current_page();
    let total_pages = state.total_pages();
    let target = state.target();

    let row_header_w = 24.0f32;
    let header_h = CELL_SIZE;
    let footer_h = 24.0f32;

    let width = (row_header_w + (cols as f32) * CELL_SIZE + PADDING) as u32;
    let height = (HEADER_H + header_h + (rows as f32) * CELL_SIZE + footer_h + PADDING) as u32;

    let mut pixmap = Pixmap::new(width, height).unwrap();
    fill_rect(&mut pixmap, 0.0, 0.0, width as f32, height as f32, BASE);

    let mut font_system = FontSystem::new();
    let mut swash_cache = SwashCache::new();

    // 헤더
    fill_rect(
        &mut pixmap,
        4.0,
        4.0,
        width as f32 - 8.0,
        HEADER_H - 4.0,
        SURFACE0,
    );
    let header_text = format!("「{}」 → 한자", target);
    draw_text(
        &mut pixmap,
        &mut font_system,
        &mut swash_cache,
        &header_text,
        PADDING,
        6.0,
        FONT_SIZE - 1.0,
        BLUE,
        width as f32 - PADDING * 2.0,
    );

    let top_row_chars: Vec<char> = state.top_row().chars().collect();

    // 열 헤더 (top_row 또는 1~9)
    let col_header_y = HEADER_H;
    for c in 0..cols {
        let label = if c < top_row_chars.len() {
            top_row_chars[c].to_string()
        } else {
            format!("{}", c + 1)
        };
        let color = if c == sel_col { BLUE } else { OVERLAY0 };
        let cx = row_header_w + (c as f32) * CELL_SIZE;
        draw_text_centered(
            &mut pixmap,
            &mut font_system,
            &mut swash_cache,
            &label,
            cx,
            col_header_y,
            CELL_SIZE,
            CELL_SIZE,
            FONT_SIZE - 2.0,
            color,
        );
    }

    // 행 번호 + 셀
    let grid_y = HEADER_H + header_h;
    for r in 0..rows {
        let ry = grid_y + (r as f32) * CELL_SIZE;
        let label = format!("{}", r + 1);
        let color = if r == sel_row { BLUE } else { OVERLAY0 };
        draw_text_centered(
            &mut pixmap,
            &mut font_system,
            &mut swash_cache,
            &label,
            0.0,
            ry,
            row_header_w,
            CELL_SIZE,
            FONT_SIZE - 2.0,
            color,
        );

        for c in 0..cols {
            if let Some(ch) = state.cell_text(r, c) {
                let cx = row_header_w + (c as f32) * CELL_SIZE;
                let is_selected = r == sel_row && c == sel_col;
                if is_selected {
                    fill_rect(&mut pixmap, cx, ry, CELL_SIZE, CELL_SIZE, SEL_HANJA_BG);
                }
                // expanded 모드 col 우선 인덱싱 (popup_layout.rs:114-127)
                let global = current_page * (rows * cols) + c * rows + r;
                let bookmarked = state.is_bookmarked(global);
                let cell_color = if bookmarked { YELLOW } else { TEXT };
                draw_text_centered(
                    &mut pixmap,
                    &mut font_system,
                    &mut swash_cache,
                    ch,
                    cx,
                    ry,
                    CELL_SIZE,
                    CELL_SIZE,
                    FONT_SIZE,
                    cell_color,
                );
            }
        }
    }

    // 푸터
    let footer_y = grid_y + (rows as f32) * CELL_SIZE + 4.0;
    let footer_text = format!(
        "{}/{}  Space ★ | . 축소 | ESC 취소",
        current_page + 1,
        total_pages.max(1)
    );
    draw_text(
        &mut pixmap,
        &mut font_system,
        &mut swash_cache,
        &footer_text,
        PADDING,
        footer_y,
        FONT_SIZE - 3.0,
        OVERLAY0,
        width as f32 - PADDING * 2.0,
    );

    let pixels = rgba_to_argb32(pixmap.data());
    unim_log!(
        "WAYLAND",
        "한자 팝업 렌더링(expanded): {}×{}, {} 페이지",
        width,
        height,
        total_pages
    );
    RenderedPopup {
        pixels,
        width,
        height,
    }
}

/// 이모지 팝업 렌더링 (PR #5 — PopupState 기반).
///
/// `render_special_from_state` 의 9×9 그리드 + 컬럼/행 헤더 + 페이지 푸터 패턴을
/// 차용하고, 좌측에 9 카테고리 탭(Recent + 8) 을 추가한다. 활성 탭은 Green 강조,
/// 비활성은 Overlay1.
fn render_emoji_from_state(state: &PopupState) -> RenderedPopup {
    let rows = state.rows();
    let cols = state.cols();
    let sel_row = state.sel_row();
    let sel_col = state.sel_col();
    let cat_index = state.emoji_cat_index();
    let categories = state.emoji_categories();

    let tab_w = 90.0f32;
    let row_header_w = 24.0f32;
    let header_h = CELL_SIZE;
    let footer_h = 24.0f32;

    let width = (tab_w + row_header_w + (cols as f32) * CELL_SIZE + PADDING) as u32;
    let height = (HEADER_H + header_h + (rows as f32) * CELL_SIZE + footer_h + PADDING) as u32;

    let mut pixmap = Pixmap::new(width, height).unwrap();
    fill_rect(&mut pixmap, 0.0, 0.0, width as f32, height as f32, BASE);

    // 좌측 탭 영역 배경 (Surface0)
    fill_rect(&mut pixmap, 0.0, 0.0, tab_w, height as f32, SURFACE0);

    let mut font_system = FontSystem::new();
    let mut swash_cache = SwashCache::new();

    // 헤더 (그리드 영역)
    fill_rect(
        &mut pixmap,
        tab_w + 4.0,
        4.0,
        width as f32 - tab_w - 8.0,
        HEADER_H - 4.0,
        SURFACE0,
    );
    let cat_label = categories
        .get(cat_index)
        .map(|c| c.label_ko.clone())
        .unwrap_or_default();
    let header_text = format!("이모지 → {}", cat_label);
    draw_text(
        &mut pixmap,
        &mut font_system,
        &mut swash_cache,
        &header_text,
        tab_w + PADDING,
        6.0,
        FONT_SIZE - 1.0,
        GREEN,
        width as f32 - tab_w - PADDING * 2.0,
    );

    // 좌측 9 탭 (Recent + 8 카테고리)
    for (i, cat) in categories.iter().enumerate().take(9) {
        let ty = HEADER_H + (i as f32) * CELL_SIZE;
        if i == cat_index {
            fill_rect(&mut pixmap, 0.0, ty, tab_w, CELL_SIZE, SEL_SPECIAL_BG);
        }
        let label = if cat.label_ko.is_empty() {
            cat.id.clone()
        } else {
            cat.label_ko.clone()
        };
        let color = if i == cat_index { GREEN } else { OVERLAY1 };
        draw_text(
            &mut pixmap,
            &mut font_system,
            &mut swash_cache,
            &label,
            6.0,
            ty + (CELL_SIZE - FONT_SIZE) / 2.0,
            FONT_SIZE - 2.0,
            color,
            tab_w - 12.0,
        );
    }

    // 열 헤더 (top_row 레이블)
    let top_row_chars: Vec<char> = state.top_row().chars().collect();
    let grid_x_origin = tab_w + row_header_w;
    let col_header_y = HEADER_H;
    for c in 0..cols {
        let label = if c < top_row_chars.len() {
            top_row_chars[c].to_string()
        } else {
            format!("{}", c + 1)
        };
        let color = if c == sel_col { GREEN } else { YELLOW };
        let cx = grid_x_origin + (c as f32) * CELL_SIZE;
        draw_text_centered(
            &mut pixmap,
            &mut font_system,
            &mut swash_cache,
            &label,
            cx,
            col_header_y,
            CELL_SIZE,
            CELL_SIZE,
            FONT_SIZE - 2.0,
            color,
        );
    }

    // 행 번호 + 셀
    let grid_y = HEADER_H + header_h;
    for r in 0..rows {
        let ry = grid_y + (r as f32) * CELL_SIZE;
        let label = format!("{}", r + 1);
        let color = if r == sel_row { GREEN } else { OVERLAY1 };
        draw_text_centered(
            &mut pixmap,
            &mut font_system,
            &mut swash_cache,
            &label,
            tab_w,
            ry,
            row_header_w,
            CELL_SIZE,
            FONT_SIZE - 2.0,
            color,
        );

        for c in 0..cols {
            if let Some(ch) = state.cell_text(r, c) {
                let cx = grid_x_origin + (c as f32) * CELL_SIZE;
                let is_selected = r == sel_row && c == sel_col;
                if is_selected {
                    fill_rect(&mut pixmap, cx, ry, CELL_SIZE, CELL_SIZE, SEL_SPECIAL_BG);
                }
                draw_text_centered(
                    &mut pixmap,
                    &mut font_system,
                    &mut swash_cache,
                    ch,
                    cx,
                    ry,
                    CELL_SIZE,
                    CELL_SIZE,
                    FONT_SIZE,
                    TEXT,
                );
            }
        }
    }

    // 푸터 ([cat]  page/total)
    let footer_y = grid_y + (rows as f32) * CELL_SIZE + 4.0;
    let footer_text = format!(
        "[{}]  {}/{}",
        cat_label,
        state.current_page() + 1,
        state.total_pages().max(1)
    );
    draw_text(
        &mut pixmap,
        &mut font_system,
        &mut swash_cache,
        &footer_text,
        tab_w + PADDING,
        footer_y,
        FONT_SIZE - 3.0,
        OVERLAY0,
        width as f32 - tab_w - PADDING * 2.0,
    );

    let pixels = rgba_to_argb32(pixmap.data());
    unim_log!("WAYLAND", "이모지 팝업 렌더링: {}×{}, cat={}", width, height, cat_index);
    RenderedPopup {
        pixels,
        width,
        height,
    }
}

/// 특수문자 팝업 렌더링 (PopupState 기반)
fn render_special_from_state(state: &PopupState) -> RenderedPopup {
    let rows = state.rows();
    let cols = state.cols();
    let sel_row = state.sel_row();
    let sel_col = state.sel_col();

    let row_header_w = 24.0f32;
    let header_h = CELL_SIZE;
    let footer_h = 24.0f32;

    let width = (row_header_w + (cols as f32) * CELL_SIZE + PADDING) as u32;
    let height = (HEADER_H + header_h + (rows as f32) * CELL_SIZE + footer_h + PADDING) as u32;

    let mut pixmap = Pixmap::new(width, height).unwrap();
    fill_rect(&mut pixmap, 0.0, 0.0, width as f32, height as f32, BASE);

    let mut font_system = FontSystem::new();
    let mut swash_cache = SwashCache::new();

    // 헤더
    fill_rect(
        &mut pixmap,
        4.0,
        4.0,
        width as f32 - 8.0,
        HEADER_H - 4.0,
        SURFACE0,
    );
    let header_text = format!("「{}」 → 특수문자", state.target());
    draw_text(
        &mut pixmap,
        &mut font_system,
        &mut swash_cache,
        &header_text,
        PADDING,
        6.0,
        FONT_SIZE - 1.0,
        GREEN,
        width as f32 - PADDING * 2.0,
    );

    let top_row_chars: Vec<char> = state.top_row().chars().collect();

    // 열 헤더
    let col_header_y = HEADER_H;
    for c in 0..cols {
        let label = if c < top_row_chars.len() {
            top_row_chars[c].to_string()
        } else {
            format!("{}", c + 1)
        };
        let color = if c == sel_col { GREEN } else { YELLOW };
        let cx = row_header_w + (c as f32) * CELL_SIZE;
        draw_text_centered(
            &mut pixmap,
            &mut font_system,
            &mut swash_cache,
            &label,
            cx,
            col_header_y,
            CELL_SIZE,
            CELL_SIZE,
            FONT_SIZE - 2.0,
            color,
        );
    }

    // 행 번호 + 셀
    let grid_y = HEADER_H + header_h;
    for r in 0..rows {
        let ry = grid_y + (r as f32) * CELL_SIZE;
        let label = format!("{}", r + 1);
        let color = if r == sel_row { GREEN } else { YELLOW };
        draw_text_centered(
            &mut pixmap,
            &mut font_system,
            &mut swash_cache,
            &label,
            0.0,
            ry,
            row_header_w,
            CELL_SIZE,
            FONT_SIZE - 2.0,
            color,
        );

        for c in 0..cols {
            if let Some(ch) = state.cell_text(r, c) {
                let cx = row_header_w + (c as f32) * CELL_SIZE;
                let is_selected = r == sel_row && c == sel_col;
                if is_selected {
                    fill_rect(&mut pixmap, cx, ry, CELL_SIZE, CELL_SIZE, SEL_SPECIAL_BG);
                }
                draw_text_centered(
                    &mut pixmap,
                    &mut font_system,
                    &mut swash_cache,
                    ch,
                    cx,
                    ry,
                    CELL_SIZE,
                    CELL_SIZE,
                    FONT_SIZE,
                    TEXT,
                );
            }
        }
    }

    // 푸터
    let footer_y = grid_y + (rows as f32) * CELL_SIZE + 4.0;
    let footer_text = format!(
        "[{}]  {}/{}",
        state.target(),
        state.current_page() + 1,
        state.total_pages().max(1)
    );
    draw_text(
        &mut pixmap,
        &mut font_system,
        &mut swash_cache,
        &footer_text,
        PADDING,
        footer_y,
        FONT_SIZE - 3.0,
        OVERLAY0,
        width as f32 - PADDING * 2.0,
    );

    let pixels = rgba_to_argb32(pixmap.data());
    unim_log!("WAYLAND", "특수문자 팝업 렌더링: {}×{}", width, height);
    RenderedPopup {
        pixels,
        width,
        height,
    }
}

// ─── 내부 렌더링 헬퍼 ───

fn fill_rect(pixmap: &mut Pixmap, x: f32, y: f32, w: f32, h: f32, color: Color) {
    let Some(rect) = Rect::from_xywh(x, y, w, h) else {
        return;
    };
    let path = PathBuilder::from_rect(rect);
    let mut paint = Paint::default();
    paint.set_color_rgba8(color.r(), color.g(), color.b(), color.a());
    paint.anti_alias = false;
    pixmap.fill_path(
        &path,
        &paint,
        tiny_skia::FillRule::Winding,
        Transform::identity(),
        None,
    );
}

fn draw_text(
    pixmap: &mut Pixmap,
    font_system: &mut FontSystem,
    swash_cache: &mut SwashCache,
    text: &str,
    x: f32,
    y: f32,
    size: f32,
    color: Color,
    max_width: f32,
) {
    let metrics = Metrics::new(size, size * LINE_HEIGHT);
    let mut buffer = Buffer::new(font_system, metrics);
    buffer.set_size(font_system, Some(max_width), Some(size * LINE_HEIGHT * 2.0));
    buffer.set_text(font_system, text, Attrs::new(), Shaping::Advanced);
    buffer.shape_until_scroll(font_system, false);

    buffer.draw(
        font_system,
        swash_cache,
        color,
        |px, py, _w, _h, buf_color| {
            let dx = x as i32 + px;
            let dy = y as i32 + py;
            if dx >= 0 && dy >= 0 && (dx as u32) < pixmap.width() && (dy as u32) < pixmap.height() {
                let idx = ((dy as u32) * pixmap.width() + (dx as u32)) as usize * 4;
                let data = pixmap.data_mut();
                if idx + 3 < data.len() {
                    let a = buf_color.a() as u32;
                    if a > 0 {
                        // Alpha blending
                        let inv_a = 255 - a;
                        data[idx] =
                            ((buf_color.r() as u32 * a + data[idx] as u32 * inv_a) / 255) as u8;
                        data[idx + 1] =
                            ((buf_color.g() as u32 * a + data[idx + 1] as u32 * inv_a) / 255) as u8;
                        data[idx + 2] =
                            ((buf_color.b() as u32 * a + data[idx + 2] as u32 * inv_a) / 255) as u8;
                        data[idx + 3] = (a + data[idx + 3] as u32 * inv_a / 255).min(255) as u8;
                    }
                }
            }
        },
    );
}

fn draw_text_right(
    pixmap: &mut Pixmap,
    font_system: &mut FontSystem,
    swash_cache: &mut SwashCache,
    text: &str,
    right_x: f32,
    y: f32,
    size: f32,
    color: Color,
) {
    // 텍스트 너비 측정 후 오른쪽 정렬
    let metrics = Metrics::new(size, size * LINE_HEIGHT);
    let mut buffer = Buffer::new(font_system, metrics);
    buffer.set_size(font_system, Some(200.0), Some(size * LINE_HEIGHT * 2.0));
    buffer.set_text(font_system, text, Attrs::new(), Shaping::Advanced);
    buffer.shape_until_scroll(font_system, false);

    // 텍스트 너비 계산
    let mut max_x = 0.0f32;
    for run in buffer.layout_runs() {
        for glyph in run.glyphs.iter() {
            let gx = glyph.x + glyph.w;
            if gx > max_x {
                max_x = gx;
            }
        }
    }

    let x = right_x - max_x;
    buffer.draw(
        font_system,
        swash_cache,
        color,
        |px, py, _w, _h, buf_color| {
            let dx = x as i32 + px;
            let dy = y as i32 + py;
            if dx >= 0 && dy >= 0 && (dx as u32) < pixmap.width() && (dy as u32) < pixmap.height() {
                let idx = ((dy as u32) * pixmap.width() + (dx as u32)) as usize * 4;
                let data = pixmap.data_mut();
                if idx + 3 < data.len() {
                    let a = buf_color.a() as u32;
                    if a > 0 {
                        let inv_a = 255 - a;
                        data[idx] =
                            ((buf_color.r() as u32 * a + data[idx] as u32 * inv_a) / 255) as u8;
                        data[idx + 1] =
                            ((buf_color.g() as u32 * a + data[idx + 1] as u32 * inv_a) / 255) as u8;
                        data[idx + 2] =
                            ((buf_color.b() as u32 * a + data[idx + 2] as u32 * inv_a) / 255) as u8;
                        data[idx + 3] = (a + data[idx + 3] as u32 * inv_a / 255).min(255) as u8;
                    }
                }
            }
        },
    );
}

fn draw_text_centered(
    pixmap: &mut Pixmap,
    font_system: &mut FontSystem,
    swash_cache: &mut SwashCache,
    text: &str,
    cell_x: f32,
    cell_y: f32,
    cell_w: f32,
    cell_h: f32,
    size: f32,
    color: Color,
) {
    let metrics = Metrics::new(size, size * LINE_HEIGHT);
    let mut buffer = Buffer::new(font_system, metrics);
    buffer.set_size(font_system, Some(cell_w), Some(cell_h));
    buffer.set_text(font_system, text, Attrs::new(), Shaping::Advanced);
    buffer.shape_until_scroll(font_system, false);

    // 텍스트 너비 계산
    let mut text_w = 0.0f32;
    for run in buffer.layout_runs() {
        for glyph in run.glyphs.iter() {
            let gx = glyph.x + glyph.w;
            if gx > text_w {
                text_w = gx;
            }
        }
    }

    let x = cell_x + (cell_w - text_w) / 2.0;
    let y = cell_y + (cell_h - size * LINE_HEIGHT) / 2.0;

    buffer.draw(
        font_system,
        swash_cache,
        color,
        |px, py, _w, _h, buf_color| {
            let dx = x as i32 + px;
            let dy = y as i32 + py;
            if dx >= 0 && dy >= 0 && (dx as u32) < pixmap.width() && (dy as u32) < pixmap.height() {
                let idx = ((dy as u32) * pixmap.width() + (dx as u32)) as usize * 4;
                let data = pixmap.data_mut();
                if idx + 3 < data.len() {
                    let a = buf_color.a() as u32;
                    if a > 0 {
                        let inv_a = 255 - a;
                        data[idx] =
                            ((buf_color.r() as u32 * a + data[idx] as u32 * inv_a) / 255) as u8;
                        data[idx + 1] =
                            ((buf_color.g() as u32 * a + data[idx + 1] as u32 * inv_a) / 255) as u8;
                        data[idx + 2] =
                            ((buf_color.b() as u32 * a + data[idx + 2] as u32 * inv_a) / 255) as u8;
                        data[idx + 3] = (a + data[idx + 3] as u32 * inv_a / 255).min(255) as u8;
                    }
                }
            }
        },
    );
}

/// RGBA → ARGB32 (wl_shm WL_SHM_FORMAT_ARGB8888)
/// tiny-skia: R G B A (바이트 순서)
/// wl_shm ARGB8888: B G R A (little-endian에서 메모리 순서)
fn rgba_to_argb32(rgba: &[u8]) -> Vec<u8> {
    let mut argb = vec![0u8; rgba.len()];
    for i in (0..rgba.len()).step_by(4) {
        argb[i] = rgba[i + 2]; // B
        argb[i + 1] = rgba[i + 1]; // G
        argb[i + 2] = rgba[i]; // R
        argb[i + 3] = rgba[i + 3]; // A
    }
    argb
}
