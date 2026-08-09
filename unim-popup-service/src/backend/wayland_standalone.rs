//! Wayland standalone backend — xdg_toplevel 기반 popup window.
//!
//! 컴포지터(KWin/Mutter)가 wlr-layer-shell 또는 input_popup_surface_v2를 지원하지
//! 않거나 Phase 5가 비활성화된 환경에서의 1차 fallback.
//!
//! 전략:
//! - GTK4 `gtk4::Window`를 그대로 사용 (default xdg_toplevel으로 매핑)
//! - 좌표는 daemon이 전달한 cursor anchor + compute_popup_xy 결과로 직접 set
//! - outside-click dismiss: Wayland에서는 `xdg_popup.grab`이 필요하나 transient_for
//!   parent toplevel이 없으므로 client-side 폴링 대신 GTK4의 `connect_close_request`
//!   + focus-out hint 활용. 완전한 grab은 Phase 5 input_popup_v2에서 처리.
//! - KWin reposition 문제 회피: show/hide 트릭 (fcitx5 패턴) — visible=false 후
//!   set_default_size, set_default_widget 등 적용 후 다시 visible=true.

#![cfg(feature = "wayland-backend")]

use unim::unim_log;

/// gtk4-layer-shell이 supported 환경(=wlr/KWin layer-shell-v1)인지 확인.
pub fn layer_shell_supported() -> bool {
    gtk4_layer_shell::is_supported()
}

/// popup window를 Wayland 환경에서 표시할 좌표로 이동.
///
/// 현재 GTK4-Wayland는 `gtk4::Window::set_position` 미지원이므로 layer-shell 사용 가능 시
/// margin으로 anchor 위치 조절, 그 외엔 default size만 설정.
pub fn position_popup(window: &gtk4::Window, cursor_x: i32, cursor_y: i32, cursor_h: i32) {
    use gtk4_layer_shell::{Edge, KeyboardMode, Layer, LayerShell};

    if !gtk4_layer_shell::is_supported() {
        unim_log!(
            "INDICATOR",
            "[Popup-Wayland] layer-shell 미지원 — 정확 위치 불가, 기본 위치로 표시"
        );
        return;
    }

    // layer-shell 초기화 (idempotent)
    if !window.is_layer_window() {
        window.init_layer_shell();
    }
    window.set_layer(Layer::Overlay);
    window.set_keyboard_mode(KeyboardMode::OnDemand);

    // anchor 좌상단 + margin 으로 cursor 위치 근접
    window.set_anchor(Edge::Top, true);
    window.set_anchor(Edge::Left, true);
    window.set_margin(Edge::Top, cursor_y + cursor_h);
    window.set_margin(Edge::Left, cursor_x);

    unim_log!(
        "INDICATOR",
        "[Popup-Wayland] layer-shell anchor TL margin=({},{})",
        cursor_x,
        cursor_y + cursor_h
    );
}
