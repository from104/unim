//! Wayland 팝업 소프트웨어 렌더러
//!
//! tiny-skia + cosmic-text로 Catppuccin Mocha 테마 팝업을 RGBA 버퍼로 렌더링합니다.
//! 결과 버퍼는 wl_shm를 통해 팝업 서피스에 attach됩니다.

use cosmic_text::{Attrs, Buffer, Color, FontSystem, Metrics, Shaping, SwashCache};
use tiny_skia::{Paint, PathBuilder, Pixmap, Rect, Transform};
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
const SEL_HANJA_BG: Color = Color::rgb(0x31, 0x36, 0x54);
const SEL_SPECIAL_BG: Color = Color::rgb(0x2d, 0x40, 0x35);

// ─── 크기 상수 ───
const FONT_SIZE: f32 = 14.0;
const LINE_HEIGHT: f32 = 1.4;
const PADDING: f32 = 12.0;
const HEADER_H: f32 = 28.0;
const ROW_H: f32 = 26.0;
const CELL_SIZE: f32 = 30.0;
const PAGE_SIZE: usize = 9;

/// 렌더링 결과
pub struct RenderedPopup {
    pub pixels: Vec<u8>, // ARGB32 (wl_shm 용)
    pub width: u32,
    pub height: u32,
}

/// 한자 팝업 렌더링
pub fn render_hanja(
    target: &str,
    candidates: &[(String, String)],
    page: usize,
    selected: usize,
) -> RenderedPopup {
    let total_pages = (candidates.len() + PAGE_SIZE - 1) / PAGE_SIZE;
    let page_start = page * PAGE_SIZE;
    let page_items: Vec<_> = candidates.iter().skip(page_start).take(PAGE_SIZE).collect();
    let item_count = page_items.len();

    let width = 340u32;
    let height = (PADDING + HEADER_H + 4.0 + (item_count as f32) * ROW_H + ROW_H + PADDING) as u32;

    let mut pixmap = Pixmap::new(width, height).unwrap();

    // 배경
    fill_rect(&mut pixmap, 0.0, 0.0, width as f32, height as f32, BASE);

    // 헤더 배경
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

    // 헤더 텍스트
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

    // 페이지 번호
    let page_text = format!("{}/{}", page + 1, total_pages.max(1));
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

    // 후보 행
    let items_y = PADDING + HEADER_H + 4.0;
    for (i, (hanja, meaning)) in page_items.iter().enumerate() {
        let row_y = items_y + (i as f32) * ROW_H;

        // 선택 배경
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

        // 번호
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

        // 한자
        draw_text(
            &mut pixmap,
            &mut font_system,
            &mut swash_cache,
            hanja,
            PADDING + 28.0,
            text_y,
            FONT_SIZE + 2.0,
            TEXT,
            60.0,
        );

        // 뜻풀이
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
                width as f32 - PADDING * 2.0 - 90.0,
            );
        }
    }

    // 푸터
    let footer_y = items_y + (item_count as f32) * ROW_H + 4.0;
    draw_text(
        &mut pixmap,
        &mut font_system,
        &mut swash_cache,
        "← → 페이지 | 1~9 선택 | ESC 취소",
        PADDING,
        footer_y,
        FONT_SIZE - 3.0,
        OVERLAY0,
        width as f32 - PADDING * 2.0,
    );

    // RGBA → ARGB32 변환 (wl_shm 용)
    let pixels = rgba_to_argb32(pixmap.data());

    unim_log!(
        "WAYLAND",
        "한자 팝업 렌더링: {}×{}, {} 후보",
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

/// 특수문자 팝업 렌더링
pub fn render_special(
    target: &str,
    characters: &[String],
    top_row: &str,
    page: usize,
    sel_row: usize,
    sel_col: usize,
) -> RenderedPopup {
    const MAX_ROWS: usize = 9;
    const MAX_COLS: usize = 9;
    const GRID_PAGE: usize = MAX_ROWS * MAX_COLS;

    let total_pages = (characters.len() + GRID_PAGE - 1) / GRID_PAGE;
    let page_start = page * GRID_PAGE;
    let page_chars = if page_start >= characters.len() {
        0
    } else {
        (characters.len() - page_start).min(GRID_PAGE)
    };

    let cols = if page_chars == 0 {
        1
    } else {
        ((page_chars + MAX_ROWS - 1) / MAX_ROWS)
            .min(MAX_COLS)
            .max(1)
    };
    let rows = if page_chars == 0 {
        1
    } else {
        ((page_chars + cols - 1) / cols).min(MAX_ROWS).max(1)
    };

    let row_header_w = 24.0f32;
    let header_h = CELL_SIZE;
    let footer_h = 24.0f32;

    let width = (row_header_w + (cols as f32) * CELL_SIZE + PADDING) as u32;
    let height = (HEADER_H + header_h + (rows as f32) * CELL_SIZE + footer_h + PADDING) as u32;

    let mut pixmap = Pixmap::new(width, height).unwrap();

    // 배경
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
    let header_text = format!("「{}」 → 특수문자", target);
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

    let top_row_chars: Vec<char> = top_row.chars().collect();

    // 열 헤더 (qwertyuio)
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
        // 행 번호
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

        // 셀
        for c in 0..cols {
            let idx = c * rows + r; // 열 우선 채움
            if idx >= page_chars {
                continue;
            }
            let global_idx = page_start + idx;
            if global_idx >= characters.len() {
                continue;
            }

            let cx = row_header_w + (c as f32) * CELL_SIZE;
            let is_selected = r == sel_row && c == sel_col;

            if is_selected {
                fill_rect(&mut pixmap, cx, ry, CELL_SIZE, CELL_SIZE, SEL_SPECIAL_BG);
            }

            draw_text_centered(
                &mut pixmap,
                &mut font_system,
                &mut swash_cache,
                &characters[global_idx],
                cx,
                ry,
                CELL_SIZE,
                CELL_SIZE,
                FONT_SIZE,
                TEXT,
            );
        }
    }

    // 푸터
    let footer_y = grid_y + (rows as f32) * CELL_SIZE + 4.0;
    let footer_text = format!("[{}]  {}/{}", target, page + 1, total_pages.max(1));
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
        "특수문자 팝업 렌더링: {}×{}, {} 문자",
        width,
        height,
        page_chars
    );

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
