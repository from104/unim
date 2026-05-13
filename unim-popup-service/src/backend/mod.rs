//! popup backend — X11/Wayland 환경별 분기.
//!
//! - x11: override-redirect + XCB outside-click polling (default)
//! - wayland_standalone: layer-shell anchor (feature `wayland-backend`)
//! - (Phase 5) wayland_input_popup: zwp_input_popup_surface_v2 (TODO)

pub mod x11;

#[cfg(feature = "wayland-backend")]
pub mod wayland_standalone;

#[cfg(feature = "wayland-backend")]
pub mod wayland_input_popup;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BackendKind {
    X11,
    WaylandInputPopup,
    WaylandStandalone,
    Unsupported,
}

pub fn detect() -> BackendKind {
    if std::env::var("WAYLAND_DISPLAY").is_ok() {
        // 우선 zwp_input_popup_surface_v2 지원 검출 (Phase 5).
        #[cfg(feature = "wayland-backend")]
        {
            if wayland_input_popup::is_supported() {
                return BackendKind::WaylandInputPopup;
            }
        }
        BackendKind::WaylandStandalone
    } else if std::env::var("DISPLAY").is_ok() {
        BackendKind::X11
    } else {
        BackendKind::Unsupported
    }
}
