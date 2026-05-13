//! popup backend — X11/Wayland 환경별 분기.

pub mod x11;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BackendKind {
    X11,
    WaylandInputPopup,
    WaylandStandalone,
    Unsupported,
}

pub fn detect() -> BackendKind {
    if std::env::var("WAYLAND_DISPLAY").is_ok() {
        BackendKind::WaylandStandalone
    } else if std::env::var("DISPLAY").is_ok() {
        BackendKind::X11
    } else {
        BackendKind::Unsupported
    }
}
