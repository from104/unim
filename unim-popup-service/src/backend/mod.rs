//! popup backend — X11/Wayland 환경별 분기.
//!
//! Phase 1: 골격만. trait/enum/detection 정의.
//! Phase 3에서 x11 backend 본격 구현, Phase 4~5에서 wayland.

#[cfg(any())]
pub mod x11;

/// 검출된 backend 종류.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BackendKind {
    X11,
    WaylandInputPopup,
    WaylandStandalone,
    Unsupported,
}

pub fn detect() -> BackendKind {
    if std::env::var("WAYLAND_DISPLAY").is_ok() {
        // Phase 5에서 input_popup_surface_v2 검출 추가. 현재는 standalone.
        BackendKind::WaylandStandalone
    } else if std::env::var("DISPLAY").is_ok() {
        BackendKind::X11
    } else {
        BackendKind::Unsupported
    }
}
