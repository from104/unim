//! 컬러 이모지 렌더 — 설계서 §11.A 동결.
//!
//! GDI(배경·헤더·한자·라벨·footer)를 **먼저 전부** 그린 뒤, 마지막에 이모지 격자 셀
//! 텍스트만 Direct2D 로 덧그린다(`D2D1_DRAW_TEXT_OPTIONS_ENABLE_COLOR_FONT` → COLR/CPAL/
//! CBDT 컬러 글리프 자동 래스터). `ID2D1DCRenderTarget` 을 더블버퍼 mem HDC 에 BindDC.
//!
//! **금지**: BindDC~EndDraw 구간에 같은 HDC 로 GDI 그리기(드라이버 미정의). 따라서
//! 본 오버레이는 GDI 가 끝난 뒤 호출돼야 한다.
//!
//! **폴백**: Factory/DWrite/DCRenderTarget/TextFormat 중 어느 하나라도 생성 실패 시
//! `None` 캐시 → 호출자가 GDI 흑백 `draw_text` 로 폴백(패닉/공백 금지). thread_local 캐시는
//! UI 스레드 전용(D2D1CreateFactory SINGLE_THREADED).

use std::cell::RefCell;

use windows::core::PCWSTR;
use windows::Win32::Foundation::{D2DERR_RECREATE_TARGET, RECT};
use windows::Win32::Graphics::Direct2D::Common::{
    D2D1_ALPHA_MODE_IGNORE, D2D1_COLOR_F, D2D1_PIXEL_FORMAT, D2D_RECT_F,
};
use windows::Win32::Graphics::Direct2D::{
    D2D1CreateFactory, ID2D1DCRenderTarget, ID2D1Factory, ID2D1SolidColorBrush,
    D2D1_DRAW_TEXT_OPTIONS_ENABLE_COLOR_FONT, D2D1_FACTORY_TYPE_SINGLE_THREADED,
    D2D1_FEATURE_LEVEL_DEFAULT, D2D1_RENDER_TARGET_PROPERTIES, D2D1_RENDER_TARGET_TYPE_DEFAULT,
    D2D1_RENDER_TARGET_USAGE_NONE,
};
use windows::Win32::Graphics::DirectWrite::{
    DWriteCreateFactory, IDWriteFactory, IDWriteTextFormat, DWRITE_FACTORY_TYPE_SHARED,
    DWRITE_FONT_STRETCH_NORMAL, DWRITE_FONT_STYLE_NORMAL, DWRITE_FONT_WEIGHT_NORMAL,
    DWRITE_MEASURING_MODE_NATURAL, DWRITE_PARAGRAPH_ALIGNMENT_CENTER, DWRITE_TEXT_ALIGNMENT_CENTER,
};
use windows::Win32::Graphics::Dxgi::Common::DXGI_FORMAT_B8G8R8A8_UNORM;
use windows::Win32::Graphics::Gdi::HDC;

use crate::logln;

/// 셀 텍스트 + rect(px) — paint 가 산출해 넘긴다. 색은 항상 본문/대비색.
pub struct EmojiCell {
    pub text: String,
    pub rect: RECT,
    /// 선택 셀 위 텍스트는 검정 대비, 그 외 흰 본문색.
    pub on_selected: bool,
}

struct D2dState {
    // factory 는 RT·format 의 수명을 떠받치는 COM 루트라 직접 읽지 않아도
    // 반드시 살려둔다(드롭 시 RT/format 무효화). 그래서 dead_code 허용.
    #[allow(dead_code)]
    factory: ID2D1Factory,
    // dwrite 는 format 재생성(DPI 변경 시) 때 다시 쓰이므로 살려둔다.
    dwrite: IDWriteFactory,
    rt: ID2D1DCRenderTarget,
    format: IDWriteTextFormat,
    /// format 을 만든 글리프 크기(px). 이 값이 요청 font_px 와 달라지면(혼합 DPI
    /// 멀티모니터 이동/WM_DPICHANGED) format 을 재생성해 캐시를 무효화한다.
    /// TextFormat 의 폰트 크기는 불변이라 크기를 캐시 키로 삼아야 한다(§11.A).
    font_px: f32,
    brush_text: ID2D1SolidColorBrush,
    brush_black: ID2D1SolidColorBrush,
}

thread_local! {
    /// UI 스레드 전용 캐시. 첫 paint 때 1회 생성, 실패 시 영구 None(흑백 폴백).
    /// `Option<Option<..>>`: 바깥 None = 미시도, 안쪽 None = 시도했으나 실패(재시도 안 함).
    static D2D: RefCell<Option<Option<D2dState>>> = const { RefCell::new(None) };
}

fn to_wide(s: &str) -> Vec<u16> {
    s.encode_utf16().collect()
}

/// 이모지 TextFormat 을 지정 글리프 크기(px)로 만든다. TextFormat 의 폰트 크기는
/// 불변이므로 크기가 바뀌면(혼합 DPI) 이 함수로 새로 만들어 교체한다.
unsafe fn create_text_format(dwrite: &IDWriteFactory, font_px: f32) -> Option<IDWriteTextFormat> {
    // Segoe UI Emoji 1순위(폴백 체인이 비이모지 문자도 처리).
    let family = to_wide("Segoe UI Emoji\0");
    let locale = to_wide("\0");
    let format = match unsafe {
        dwrite.CreateTextFormat(
            PCWSTR(family.as_ptr()),
            None,
            DWRITE_FONT_WEIGHT_NORMAL,
            DWRITE_FONT_STYLE_NORMAL,
            DWRITE_FONT_STRETCH_NORMAL,
            font_px,
            PCWSTR(locale.as_ptr()),
        )
    } {
        Ok(f) => f,
        Err(e) => {
            logln!("d2d: CreateTextFormat failed ({e}) — GDI fallback");
            return None;
        }
    };
    let _ = unsafe { format.SetTextAlignment(DWRITE_TEXT_ALIGNMENT_CENTER) };
    let _ = unsafe { format.SetParagraphAlignment(DWRITE_PARAGRAPH_ALIGNMENT_CENTER) };
    Some(format)
}

/// 글리프 크기(px) — 셀 높이에 맞춰 호출자가 지정. scale(DPI) 반영은 호출자가
/// font_px 로 전달. 크기가 바뀌면 draw_emoji_cells 가 format 을 재생성한다.
unsafe fn create_state(font_px: f32) -> Option<D2dState> {
    let factory: ID2D1Factory =
        match unsafe { D2D1CreateFactory::<ID2D1Factory>(D2D1_FACTORY_TYPE_SINGLE_THREADED, None) } {
            Ok(f) => f,
            Err(e) => {
                logln!("d2d: D2D1CreateFactory failed ({e}) — GDI fallback");
                return None;
            }
        };
    let dwrite: IDWriteFactory =
        match unsafe { DWriteCreateFactory::<IDWriteFactory>(DWRITE_FACTORY_TYPE_SHARED) } {
            Ok(f) => f,
            Err(e) => {
                logln!("d2d: DWriteCreateFactory failed ({e}) — GDI fallback");
                return None;
            }
        };
    // DCRenderTarget — D3D 디바이스 없이 GDI HDC 위에서 동작. dpi=96 고정(우리가 px rect 전달).
    let props = D2D1_RENDER_TARGET_PROPERTIES {
        r#type: D2D1_RENDER_TARGET_TYPE_DEFAULT,
        pixelFormat: D2D1_PIXEL_FORMAT {
            format: DXGI_FORMAT_B8G8R8A8_UNORM,
            alphaMode: D2D1_ALPHA_MODE_IGNORE,
        },
        dpiX: 96.0,
        dpiY: 96.0,
        usage: D2D1_RENDER_TARGET_USAGE_NONE,
        minLevel: D2D1_FEATURE_LEVEL_DEFAULT,
    };
    let rt = match unsafe { factory.CreateDCRenderTarget(&props) } {
        Ok(rt) => rt,
        Err(e) => {
            logln!("d2d: CreateDCRenderTarget failed ({e}) — GDI fallback");
            return None;
        }
    };
    // TextFormat — 현재 DPI 반영 글리프 크기로 생성(크기 변경 시 재생성됨).
    let format = match unsafe { create_text_format(&dwrite, font_px) } {
        Some(f) => f,
        None => return None, // 로그는 create_text_format 내부에서.
    };
    // 본문(흰)·대비(검정) 브러시.
    let white = D2D1_COLOR_F { r: 0.804, g: 0.839, b: 0.957, a: 1.0 }; // #cdd6f4
    let black = D2D1_COLOR_F { r: 0.0, g: 0.0, b: 0.0, a: 1.0 };
    let brush_text = match unsafe { rt.CreateSolidColorBrush(&white, None) } {
        Ok(b) => b,
        Err(e) => {
            logln!("d2d: CreateSolidColorBrush(text) failed ({e}) — GDI fallback");
            return None;
        }
    };
    let brush_black = match unsafe { rt.CreateSolidColorBrush(&black, None) } {
        Ok(b) => b,
        Err(e) => {
            logln!("d2d: CreateSolidColorBrush(black) failed ({e}) — GDI fallback");
            return None;
        }
    };
    logln!("d2d: state created (color emoji enabled) font_px={font_px:.1}");
    Some(D2dState {
        factory,
        dwrite,
        rt,
        format,
        font_px,
        brush_text,
        brush_black,
    })
}

/// 이모지 셀 텍스트를 컬러로 덧그린다. 성공 시 true, 폴백(흑백 GDI 필요) 시 false.
///
/// `hdc` = 더블버퍼 mem HDC(이미 GDI 그리기 완료). `full` = 클라이언트 전체 rect(px).
/// BindDC ~ EndDraw 구간에는 같은 HDC 로 GDI 그리기 금지(호출 전 GDI 완료 보장).
pub fn draw_emoji_cells(hdc: HDC, full: &RECT, font_px: f32, cells: &[EmojiCell]) -> bool {
    if cells.is_empty() {
        return true; // 그릴 게 없음 — 폴백 불필요.
    }
    D2D.with(|cell| {
        let mut slot = cell.borrow_mut();
        if slot.is_none() {
            *slot = Some(unsafe { create_state(font_px) });
        }
        // 혼합 DPI 멀티모니터: 최초 렌더 DPI 에 동결된 글리프 크기를 갱신한다.
        // font_px 가 바뀌면(WM_DPICHANGED/모니터 이동으로 scale 재계산) TextFormat 을
        // 재생성해 캐시 무효화. 실패 시 이번 프레임은 옛 크기 유지(공백/폴백 회피).
        if let Some(Some(st)) = slot.as_mut() {
            if (st.font_px - font_px).abs() > 0.5 {
                match unsafe { create_text_format(&st.dwrite, font_px) } {
                    Some(fmt) => {
                        st.format = fmt;
                        st.font_px = font_px;
                        logln!("d2d: text format regenerated for DPI change font_px={font_px:.1}");
                    }
                    None => {
                        logln!("d2d: text format regen failed — keeping old size this frame");
                    }
                }
            }
        }
        let Some(Some(st)) = slot.as_ref() else {
            return false; // 생성 실패 캐시됨 → GDI 흑백 폴백.
        };
        unsafe { paint_with(st, hdc, full, cells) }
    })
}

unsafe fn paint_with(st: &D2dState, hdc: HDC, full: &RECT, cells: &[EmojiCell]) -> bool {
    // BindDC — 매 paint 호출 시 rect 갱신(§11.A: RT 는 BindDC 시 rect 만 바뀜).
    if let Err(e) = unsafe { st.rt.BindDC(hdc, full) } {
        logln!("d2d: BindDC failed ({e}) — GDI fallback this frame");
        return false;
    }
    unsafe { st.rt.BeginDraw() };
    for c in cells {
        let layout = D2D_RECT_F {
            left: c.rect.left as f32,
            top: c.rect.top as f32,
            right: c.rect.right as f32,
            bottom: c.rect.bottom as f32,
        };
        let brush: &ID2D1SolidColorBrush = if c.on_selected {
            &st.brush_black
        } else {
            &st.brush_text
        };
        let wide = to_wide(&c.text);
        unsafe {
            st.rt.DrawText(
                &wide,
                &st.format,
                &layout,
                brush,
                D2D1_DRAW_TEXT_OPTIONS_ENABLE_COLOR_FONT,
                DWRITE_MEASURING_MODE_NATURAL,
            );
        }
    }
    // EndDraw — D2DERR_RECREATE_TARGET 이면 RT 폐기·재생성(다음 프레임에서 복구).
    match unsafe { st.rt.EndDraw(None, None) } {
        Ok(()) => true,
        Err(e) if e.code() == D2DERR_RECREATE_TARGET => {
            logln!("d2d: EndDraw RECREATE_TARGET — dropping cached state, will recreate next frame");
            // 캐시 무효화 → 다음 draw 에서 create_state 재시도. 이번 프레임은 폴백.
            D2D.with(|cell| *cell.borrow_mut() = None);
            false
        }
        Err(e) => {
            logln!("d2d: EndDraw failed ({e}) — GDI fallback this frame");
            false
        }
    }
}
