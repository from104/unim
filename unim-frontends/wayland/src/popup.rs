//! Wayland 팝업 서피스 관리
//!
//! `zwp_input_popup_surface_v2` 프로토콜을 사용하여
//! 한자/특수문자 후보 팝업을 표시합니다.
//!
//! 렌더링: tiny-skia (2D) + cosmic-text (텍스트 레이아웃)
//! 버퍼: wl_shm (memfd 기반 공유 메모리)

use std::os::fd::AsFd;

use cosmic_text::{
    Attrs, Buffer as TextBuffer, Color as CTextColor, Family, FontSystem, Metrics, Shaping,
    SwashCache,
};
use memmap2::MmapMut;
use tiny_skia::{Color, FillRule, Paint, PathBuilder, Pixmap, Rect, Transform};
use unim::unim_log;
use wayland_client::protocol::{
    wl_buffer::WlBuffer,
    wl_compositor::WlCompositor,
    wl_shm::{self, WlShm},
    wl_shm_pool::WlShmPool,
    wl_surface::WlSurface,
};
use wayland_client::{Connection, Dispatch, QueueHandle};
use wayland_protocols_misc::zwp_input_method_v2::client::zwp_input_popup_surface_v2::{
    self, ZwpInputPopupSurfaceV2,
};

use crate::state::AppState;

// Catppuccin Mocha 테마 색상 (tiny-skia Color는 const 생성 불가)
pub fn bg_color() -> Color {
    Color::from_rgba8(30, 30, 46, 255)
}
pub fn sel_bg_color() -> Color {
    Color::from_rgba8(137, 180, 250, 255)
}

pub const TEXT_COLOR: CTextColor = CTextColor::rgb(205, 214, 244); // #cdd6f4
pub const SEL_TEXT_COLOR: CTextColor = CTextColor::rgb(30, 30, 46); // #1e1e2e
pub const HEADER_COLOR: CTextColor = CTextColor::rgb(243, 139, 168); // #f38ba8
pub const PAGE_COLOR: CTextColor = CTextColor::rgb(108, 112, 134); // #6c7086

/// 공유 메모리 버퍼
struct ShmBuffer {
    pool: WlShmPool,
    buffer: WlBuffer,
    mmap: MmapMut,
    width: u32,
    height: u32,
}

impl ShmBuffer {
    /// 새 공유 메모리 버퍼 생성
    fn new(
        shm: &WlShm,
        qh: &QueueHandle<AppState>,
        width: u32,
        height: u32,
    ) -> Result<Self, String> {
        let stride = width * 4; // ARGB32
        let size = (stride * height) as usize;

        // memfd 생성
        let fd = nix::sys::memfd::memfd_create(
            c"unim-popup",
            nix::sys::memfd::MemFdCreateFlag::MFD_CLOEXEC
                | nix::sys::memfd::MemFdCreateFlag::MFD_ALLOW_SEALING,
        )
        .map_err(|e| format!("memfd_create 실패: {}", e))?;

        // 크기 설정
        nix::unistd::ftruncate(&fd, size as nix::libc::off_t)
            .map_err(|e| format!("ftruncate 실패: {}", e))?;

        // mmap
        let mmap = unsafe { MmapMut::map_mut(fd.as_fd().as_raw_fd()) }
            .map_err(|e| format!("mmap 실패: {}", e))?;

        // wl_shm_pool 생성
        let pool = shm.create_pool(fd.as_fd(), size as i32, qh, ());

        // wl_buffer 생성
        let buffer = pool.create_buffer(
            0,
            width as i32,
            height as i32,
            stride as i32,
            wl_shm::Format::Argb8888,
            qh,
            (),
        );

        Ok(Self {
            pool,
            buffer,
            mmap,
            width,
            height,
        })
    }

    /// 버퍼에 pixmap 데이터 쓰기
    fn write_pixels(&mut self, pixmap: &Pixmap) {
        let src = pixmap.data();
        let dst = &mut self.mmap[..];

        // tiny-skia는 RGBA, Wayland는 ARGB → 변환
        for i in 0..(self.width * self.height) as usize {
            let si = i * 4;
            dst[si] = src[si + 2]; // B
            dst[si + 1] = src[si + 1]; // G
            dst[si + 2] = src[si]; // R
            dst[si + 3] = src[si + 3]; // A
        }
    }

    /// 정리
    fn destroy(&self) {
        self.buffer.destroy();
        self.pool.destroy();
    }
}

use std::os::fd::AsRawFd;

/// 팝업 서피스 상태
pub struct PopupSurface {
    surface: WlSurface,
    #[allow(dead_code)]
    popup: ZwpInputPopupSurfaceV2,
    shm_buffer: Option<ShmBuffer>,
    // 텍스트 렌더링
    pub font_system: FontSystem,
    pub swash_cache: SwashCache,
    // 팝업 위치 힌트
    pub text_rect: (i32, i32, i32, i32), // x, y, w, h
}

impl PopupSurface {
    /// 팝업 서피스 생성
    pub fn create(
        compositor: &WlCompositor,
        im: &wayland_protocols_misc::zwp_input_method_v2::client::zwp_input_method_v2::ZwpInputMethodV2,
        qh: &QueueHandle<AppState>,
    ) -> Self {
        let surface = compositor.create_surface(qh, ());
        let popup = im.get_input_popup_surface(&surface, qh, ());

        unim_log!("WAYLAND", "팝업 서피스 생성");

        Self {
            surface,
            popup,
            shm_buffer: None,
            font_system: FontSystem::new(),
            swash_cache: SwashCache::new(),
            text_rect: (0, 0, 0, 0),
        }
    }

    /// 렌더링 및 서피스 업데이트
    pub fn render(
        &mut self,
        shm: &WlShm,
        qh: &QueueHandle<AppState>,
        width: u32,
        height: u32,
        draw_fn: impl FnOnce(&mut Pixmap, &mut FontSystem, &mut SwashCache),
    ) {
        // 이전 버퍼 정리
        if let Some(ref old_buf) = self.shm_buffer {
            old_buf.destroy();
        }

        // pixmap 생성 및 렌더링
        let mut pixmap = Pixmap::new(width, height).expect("Pixmap 생성 실패");
        pixmap.fill(bg_color());
        draw_fn(&mut pixmap, &mut self.font_system, &mut self.swash_cache);

        // shm 버퍼 생성 및 픽셀 데이터 쓰기
        match ShmBuffer::new(shm, qh, width, height) {
            Ok(mut shm_buffer) => {
                shm_buffer.write_pixels(&pixmap);
                self.surface.attach(Some(&shm_buffer.buffer), 0, 0);
                self.surface
                    .damage_buffer(0, 0, width as i32, height as i32);
                self.surface.commit();
                self.shm_buffer = Some(shm_buffer);
            }
            Err(e) => {
                unim_log!("WAYLAND", "팝업 버퍼 생성 실패: {}", e);
            }
        }
    }

    /// 팝업 숨기기
    pub fn hide(&self) {
        self.surface.attach(None, 0, 0);
        self.surface.commit();
    }

    /// 팝업 파괴
    #[allow(dead_code)]
    pub fn destroy(self) {
        if let Some(ref buf) = self.shm_buffer {
            buf.destroy();
        }
        self.popup.destroy();
        self.surface.destroy();
        unim_log!("WAYLAND", "팝업 서피스 파괴");
    }
}

/// 텍스트를 pixmap에 렌더링하는 헬퍼
pub fn draw_text(
    pixmap: &mut Pixmap,
    font_system: &mut FontSystem,
    swash_cache: &mut SwashCache,
    x: f32,
    y: f32,
    width: f32,
    text: &str,
    font_size: f32,
    color: CTextColor,
) {
    let metrics = Metrics::new(font_size, font_size * 1.2);
    let mut buffer = TextBuffer::new(font_system, metrics);
    buffer.set_size(font_system, Some(width), None);
    buffer.set_text(
        font_system,
        text,
        Attrs::new().family(Family::SansSerif),
        Shaping::Advanced,
    );
    buffer.shape_until_scroll(font_system, false);

    // 렌더링
    buffer.draw(font_system, swash_cache, color, |gx, gy, _w, _h, gc| {
        let px = x as i32 + gx;
        let py = y as i32 + gy;
        if px >= 0 && py >= 0 && px < pixmap.width() as i32 && py < pixmap.height() as i32 {
            let a = gc.a();
            if a > 0 {
                let idx = (py as u32 * pixmap.width() + px as u32) as usize * 4;
                let data = pixmap.data_mut();
                if idx + 3 < data.len() {
                    let alpha = a as f32 / 255.0;
                    let inv = 1.0 - alpha;
                    data[idx] = (gc.r() as f32 * alpha + data[idx] as f32 * inv) as u8;
                    data[idx + 1] = (gc.g() as f32 * alpha + data[idx + 1] as f32 * inv) as u8;
                    data[idx + 2] = (gc.b() as f32 * alpha + data[idx + 2] as f32 * inv) as u8;
                    data[idx + 3] = (a as f32 + data[idx + 3] as f32 * inv) as u8;
                }
            }
        }
    });
}

/// 사각형을 pixmap에 그리는 헬퍼
pub fn draw_rect(pixmap: &mut Pixmap, x: f32, y: f32, w: f32, h: f32, color: Color) {
    if let Some(rect) = Rect::from_xywh(x, y, w, h) {
        let mut paint = Paint::default();
        paint.set_color(color);
        let mut pb = PathBuilder::new();
        pb.push_rect(rect);
        if let Some(path) = pb.finish() {
            pixmap.fill_path(
                &path,
                &paint,
                FillRule::Winding,
                Transform::identity(),
                None,
            );
        }
    }
}

// ============================================================
// Dispatch 구현들
// ============================================================

// --- WlCompositor ---
impl Dispatch<WlCompositor, ()> for AppState {
    fn event(
        _state: &mut Self,
        _proxy: &WlCompositor,
        _event: wayland_client::protocol::wl_compositor::Event,
        _data: &(),
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
    ) {
    }
}

// --- WlShm ---
impl Dispatch<WlShm, ()> for AppState {
    fn event(
        _state: &mut Self,
        _proxy: &WlShm,
        _event: wl_shm::Event,
        _data: &(),
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
    ) {
    }
}

// --- WlShmPool ---
impl Dispatch<WlShmPool, ()> for AppState {
    fn event(
        _state: &mut Self,
        _proxy: &WlShmPool,
        _event: wayland_client::protocol::wl_shm_pool::Event,
        _data: &(),
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
    ) {
    }
}

// --- WlBuffer ---
impl Dispatch<WlBuffer, ()> for AppState {
    fn event(
        _state: &mut Self,
        _proxy: &WlBuffer,
        event: wayland_client::protocol::wl_buffer::Event,
        _data: &(),
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
    ) {
        match event {
            wayland_client::protocol::wl_buffer::Event::Release => {
                // 버퍼 릴리스 — 재사용 가능
            }
            _ => {}
        }
    }
}

// --- WlSurface ---
impl Dispatch<WlSurface, ()> for AppState {
    fn event(
        _state: &mut Self,
        _proxy: &WlSurface,
        _event: wayland_client::protocol::wl_surface::Event,
        _data: &(),
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
    ) {
    }
}

// --- ZwpInputPopupSurfaceV2 ---
impl Dispatch<ZwpInputPopupSurfaceV2, ()> for AppState {
    fn event(
        state: &mut Self,
        _proxy: &ZwpInputPopupSurfaceV2,
        event: zwp_input_popup_surface_v2::Event,
        _data: &(),
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
    ) {
        match event {
            zwp_input_popup_surface_v2::Event::TextInputRectangle {
                x,
                y,
                width,
                height,
            } => {
                unim_log!(
                    "WAYLAND",
                    "팝업 text_input_rectangle: ({},{}) {}x{}",
                    x,
                    y,
                    width,
                    height
                );
                if let Some(ref mut popup) = state.popup_surface {
                    popup.text_rect = (x, y, width, height);
                }
            }
            _ => {}
        }
    }
}
