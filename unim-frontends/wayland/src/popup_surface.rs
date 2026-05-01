//! Wayland 팝업 서피스 관리
//!
//! wl_compositor surface + zwp_input_popup_surface_v2 + wl_shm 버퍼를 관리합니다.
//! 컴포지터가 팝업 위치를 자동 결정합니다.

use std::os::fd::AsFd;

use unim::popup::PopupState;
use unim::unim_log;
use wayland_client::{
    protocol::{
        wl_buffer::WlBuffer,
        wl_compositor::WlCompositor,
        wl_shm::{self, WlShm},
        wl_shm_pool::WlShmPool,
        wl_surface::WlSurface,
    },
    Connection, Dispatch, QueueHandle,
};
use wayland_protocols_misc::zwp_input_method_v2::client::{
    zwp_input_method_v2::ZwpInputMethodV2,
    zwp_input_popup_surface_v2::{self, ZwpInputPopupSurfaceV2},
};

use crate::popup_renderer::{self, RenderedPopup};
use crate::state::AppState;

/// Wayland 팝업 서피스
pub struct PopupSurface {
    surface: Option<WlSurface>,
    popup: Option<ZwpInputPopupSurfaceV2>,
    pool: Option<WlShmPool>,
    buffer: Option<WlBuffer>,
    /// 현재 SHM fd (memmap2)
    shm_file: Option<std::fs::File>,
    /// 통합 팝업 상태
    pub popup_state: Option<PopupState>,
    /// 현재 크기
    cur_width: u32,
    cur_height: u32,
}

#[allow(dead_code)]
impl PopupSurface {
    pub fn new() -> Self {
        Self {
            surface: None,
            popup: None,
            pool: None,
            buffer: None,
            shm_file: None,
            popup_state: None,
            cur_width: 0,
            cur_height: 0,
        }
    }

    /// 팝업 서피스 초기화 (setup 시 호출)
    pub fn init(
        &mut self,
        compositor: &WlCompositor,
        input_method: &ZwpInputMethodV2,
        qh: &QueueHandle<AppState>,
    ) {
        let surface = compositor.create_surface(qh, ());
        let popup = input_method.get_input_popup_surface(&surface, qh, ());
        unim_log!("WAYLAND", "팝업 서피스 생성 완료");
        self.surface = Some(surface);
        self.popup = Some(popup);
    }

    /// 팝업 표시 여부
    pub fn is_visible(&self) -> bool {
        self.popup_state.is_some()
    }

    /// 한자 팝업 표시
    pub fn show_hanja(
        &mut self,
        shm: &WlShm,
        qh: &QueueHandle<AppState>,
        target: &str,
        candidates: Vec<(String, String)>,
        top_row: &str,
    ) {
        self.popup_state = Some(PopupState::new_hanja_with_top_row(
            target, candidates, top_row,
        ));
        self.render_and_commit(shm, qh);
    }

    /// 특수문자 팝업 표시
    pub fn show_special(
        &mut self,
        shm: &WlShm,
        qh: &QueueHandle<AppState>,
        target: &str,
        characters: Vec<String>,
        top_row: &str,
    ) {
        self.popup_state = Some(PopupState::new_special(target, characters, top_row));
        self.render_and_commit(shm, qh);
    }

    /// 팝업 숨기기
    pub fn hide(&mut self) {
        if let Some(ref surface) = self.surface {
            // 빈 커밋으로 숨김
            surface.attach(None, 0, 0);
            surface.commit();
        }
        self.popup_state = None;
        // 이전 버퍼 정리
        if let Some(buf) = self.buffer.take() {
            buf.destroy();
        }
        unim_log!("WAYLAND", "팝업 숨김");
    }

    /// 팝업 재렌더링 (선택 변경 등)
    pub fn redraw(&mut self, shm: &WlShm, qh: &QueueHandle<AppState>) {
        if self.popup_state.is_some() {
            self.render_and_commit(shm, qh);
        }
    }

    /// 한자 즐겨찾기 플래그 일괄 적용 + 재렌더 (ShowHanja 직후 초기 fetch).
    pub fn apply_hanja_bookmark_flags(
        &mut self,
        flags: Vec<bool>,
        shm: &WlShm,
        qh: &QueueHandle<AppState>,
    ) {
        if let Some(ref mut ps) = self.popup_state {
            ps.set_bookmark_flags(flags);
        }
        self.render_and_commit(shm, qh);
    }

    /// 한자 즐겨찾기 단일 토글 반영 + 재렌더 (HanjaBookmarkChanged 시그널).
    pub fn apply_hanja_bookmark_changed(
        &mut self,
        index: usize,
        bookmarked: bool,
        shm: &WlShm,
        qh: &QueueHandle<AppState>,
    ) {
        if let Some(ref mut ps) = self.popup_state {
            ps.set_bookmark(index, bookmarked);
        }
        self.render_and_commit(shm, qh);
    }

    /// 한자 후보 재정렬 + 커서 점프 + 재렌더 (HanjaCandidatesReordered 시그널).
    ///
    /// 다른 프런트엔드(GTK Standalone/IM, Qt IM, GNOME extension)와 동일하게
    /// payload의 page/sel_row/sel_col 을 직접 적용한다. set_selected_global 은
    /// new_cursor 로 페이지·행·열을 한 번에 재계산하므로 일관된 결과를 낸다.
    pub fn apply_hanja_candidates_reordered(
        &mut self,
        candidates: Vec<(String, String)>,
        bookmarks: Vec<bool>,
        new_cursor: u32,
        shm: &WlShm,
        qh: &QueueHandle<AppState>,
    ) {
        if let Some(ref mut ps) = self.popup_state {
            let (items, meanings): (Vec<String>, Vec<String>) = candidates.into_iter().unzip();
            ps.replace_hanja_items(items, meanings, bookmarks);
            ps.set_selected_global(new_cursor as usize);
        }
        self.render_and_commit(shm, qh);
    }

    /// 팝업 네비게이션(페이지·선택 변경) 적용 + 재렌더 (PopupNavigate 시그널).
    pub fn apply_popup_navigate(
        &mut self,
        page: i32,
        sel_row: i32,
        sel_col: i32,
        shm: &WlShm,
        qh: &QueueHandle<AppState>,
    ) {
        if let Some(ref mut ps) = self.popup_state {
            ps.set_navigate_state(
                page.max(0) as usize,
                sel_row.max(0) as usize,
                sel_col.max(0) as usize,
            );
        }
        self.render_and_commit(shm, qh);
    }

    /// 렌더링 + SHM 버퍼 커밋
    fn render_and_commit(&mut self, shm: &WlShm, qh: &QueueHandle<AppState>) {
        let surface = match self.surface {
            Some(ref s) => s.clone(),
            None => {
                unim_log!("WAYLAND", "팝업 서피스 미초기화");
                return;
            }
        };

        let rendered = match &self.popup_state {
            Some(ps) => popup_renderer::render_popup(ps),
            None => return,
        };

        self.attach_buffer(&surface, shm, qh, &rendered);
    }

    /// SHM 버퍼 생성 및 attach
    fn attach_buffer(
        &mut self,
        surface: &WlSurface,
        shm: &WlShm,
        qh: &QueueHandle<AppState>,
        rendered: &RenderedPopup,
    ) {
        let stride = rendered.width * 4;
        let size = (stride * rendered.height) as usize;

        // 이전 버퍼/풀 정리
        if let Some(buf) = self.buffer.take() {
            buf.destroy();
        }
        if let Some(pool) = self.pool.take() {
            pool.destroy();
        }

        // tmpfile 기반 SHM 풀 생성
        let file = match create_shm_file(size) {
            Ok(f) => f,
            Err(e) => {
                unim_log!("WAYLAND", "SHM 파일 생성 실패: {}", e);
                return;
            }
        };

        // 픽셀 데이터 쓰기
        {
            use std::io::Write;
            let mut writer = std::io::BufWriter::new(&file);
            if writer.write_all(&rendered.pixels).is_err() {
                unim_log!("WAYLAND", "SHM 쓰기 실패");
                return;
            }
        }

        let pool = shm.create_pool(file.as_fd(), size as i32, qh, ());
        let buffer = pool.create_buffer(
            0,
            rendered.width as i32,
            rendered.height as i32,
            stride as i32,
            wl_shm::Format::Argb8888,
            qh,
            (),
        );

        surface.attach(Some(&buffer), 0, 0);
        surface.damage_buffer(0, 0, rendered.width as i32, rendered.height as i32);
        surface.commit();

        self.pool = Some(pool);
        self.buffer = Some(buffer);
        self.shm_file = Some(file);
        self.cur_width = rendered.width;
        self.cur_height = rendered.height;
    }

    /// 정리
    pub fn destroy(&mut self) {
        if let Some(buf) = self.buffer.take() {
            buf.destroy();
        }
        if let Some(pool) = self.pool.take() {
            pool.destroy();
        }
        if let Some(popup) = self.popup.take() {
            popup.destroy();
        }
        if let Some(surface) = self.surface.take() {
            surface.destroy();
        }
        self.popup_state = None;
    }
}

/// tmpfile 기반 SHM 파일 생성
fn create_shm_file(size: usize) -> Result<std::fs::File, String> {
    use nix::sys::memfd;
    use std::os::fd::FromRawFd;

    // memfd_create 사용 (Linux 3.17+)
    let name = std::ffi::CString::new("unim-popup").map_err(|e| e.to_string())?;
    let fd = memfd::memfd_create(&name, memfd::MemFdCreateFlag::MFD_CLOEXEC)
        .map_err(|e| format!("memfd_create: {}", e))?;

    // 파일 크기 설정
    nix::unistd::ftruncate(&fd, size as nix::libc::off_t)
        .map_err(|e| format!("ftruncate: {}", e))?;

    let file = unsafe { std::fs::File::from_raw_fd(std::os::fd::IntoRawFd::into_raw_fd(fd)) };
    Ok(file)
}

// ─── Dispatch 구현 ───

impl Dispatch<ZwpInputPopupSurfaceV2, ()> for AppState {
    fn event(
        _state: &mut Self,
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
                    "팝업 text_input_rectangle: ({}, {}, {}, {})",
                    x,
                    y,
                    width,
                    height
                );
                // 컴포지터가 보내는 위치 힌트 — 현재는 로그만
            }
            _ => {}
        }
    }
}

// wl_surface Dispatch
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

// wl_compositor Dispatch
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

// wl_shm Dispatch
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

// wl_shm_pool Dispatch
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

// wl_buffer Dispatch
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
                // 버퍼 릴리스 — 현재는 무시 (다음 렌더링에서 새 버퍼 생성)
            }
            _ => {}
        }
    }
}
