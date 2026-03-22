//! 팝업 윈도우 포지셔닝 및 디스플레이 서버 분기
//!
//! X11: GDK X11 surface를 통한 위치 설정
//! Wayland (Sway/Hyprland): gtk4-layer-shell
//! GNOME Wayland: 팝업 생성 건너뜀 (extension 처리)

use gtk4::prelude::*;
use unim::unim_log;

/// 디스플레이 서버 환경
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum DisplayServer {
    X11,
    WaylandLayerShell,
    GnomeWayland,
}

/// 현재 디스플레이 서버 환경 감지
pub fn detect_display_server() -> DisplayServer {
    // GNOME Wayland 감지
    if is_gnome_wayland() {
        return DisplayServer::GnomeWayland;
    }

    // X11 vs Wayland 감지
    let display = gtk4::gdk::Display::default().expect("No display");
    if display.backend().is_x11() {
        DisplayServer::X11
    } else {
        DisplayServer::WaylandLayerShell
    }
}

/// GNOME Wayland 환경인지 감지
fn is_gnome_wayland() -> bool {
    let is_wayland = std::env::var("XDG_SESSION_TYPE")
        .map(|v| v == "wayland")
        .unwrap_or(false);

    if !is_wayland {
        return false;
    }

    if std::env::var("GNOME_SHELL_SESSION_MODE").is_ok() {
        return true;
    }
    if let Ok(desktop) = std::env::var("XDG_CURRENT_DESKTOP") {
        if desktop.to_uppercase().contains("GNOME") {
            return true;
        }
    }
    if let Ok(session) = std::env::var("DESKTOP_SESSION") {
        if session.to_lowercase().contains("gnome") {
            return true;
        }
    }

    false
}

/// 팝업 윈도우를 커서 좌표에 배치 (화면 경계 보정 포함)
pub fn position_popup(
    window: &gtk4::Window,
    cursor_x: i32,
    cursor_y: i32,
    cursor_h: i32,
    display_server: DisplayServer,
) {
    match display_server {
        DisplayServer::X11 => {
            position_popup_x11(window, cursor_x, cursor_y, cursor_h);
        }
        DisplayServer::WaylandLayerShell => {
            position_popup_wayland(window, cursor_x, cursor_y, cursor_h);
        }
        DisplayServer::GnomeWayland => {}
    }
}

/// X11: GDK X11 surface를 통한 override-redirect + 절대좌표 배치
fn position_popup_x11(window: &gtk4::Window, cursor_x: i32, cursor_y: i32, cursor_h: i32) {
    let popup_x = cursor_x;
    let popup_y = cursor_y + cursor_h + 4;

    #[cfg(feature = "gdk4-x11")]
    {
        let cx = popup_x;
        let cy = popup_y;

        // realize 후 X11 surface에 접근하여 override-redirect 설정
        window.connect_realize(move |win| {
            let surface = match win.surface() {
                Some(s) => s,
                None => return,
            };

            if let Some(x11_surface) = surface.downcast_ref::<gdk4_x11::X11Surface>() {
                unsafe {
                    let x11_display = surface
                        .display()
                        .downcast::<gdk4_x11::X11Display>()
                        .unwrap();
                    let xdisplay = gdk4_x11::ffi::gdk_x11_display_get_xdisplay(
                        gtk4::glib::translate::ToGlibPtr::to_glib_none(&x11_display).0,
                    );
                    let xid = gdk4_x11::ffi::gdk_x11_surface_get_xid(
                        gtk4::glib::translate::ToGlibPtr::to_glib_none(x11_surface).0,
                    );

                    extern "C" {
                        fn XMoveWindow(
                            display: *mut std::ffi::c_void,
                            w: std::ffi::c_ulong,
                            x: std::ffi::c_int,
                            y: std::ffi::c_int,
                        ) -> std::ffi::c_int;
                        fn XFlush(display: *mut std::ffi::c_void) -> std::ffi::c_int;
                    }

                    XMoveWindow(xdisplay as *mut _, xid as _, cx, cy);
                    XFlush(xdisplay as *mut _);

                    unim_log!(
                        "INDICATOR",
                        "[Popup] X11 윈도우 배치: pos=({},{}), xid={}",
                        cx,
                        cy,
                        xid
                    );
                }
            }
        });
    }

    #[cfg(not(feature = "gdk4-x11"))]
    {
        let _ = (window, popup_x, popup_y);
        unim_log!(
            "INDICATOR",
            "[Popup] gdk4-x11 feature 미활성, X11 팝업 위치 설정 불가"
        );
    }
}

/// Wayland: gtk4-layer-shell 사용
fn position_popup_wayland(window: &gtk4::Window, cursor_x: i32, cursor_y: i32, cursor_h: i32) {
    #[cfg(feature = "wayland")]
    {
        use gtk4_layer_shell::{Edge, KeyboardInteractivity, Layer, LayerShell};

        if !LayerShell::is_supported() {
            unim_log!(
                "INDICATOR",
                "[Popup] gtk4-layer-shell 미지원, 팝업 표시 불가"
            );
            return;
        }

        window.init_layer_shell();
        window.set_layer(Layer::Overlay);
        window.set_keyboard_interactivity(KeyboardInteractivity::None);

        window.set_anchor(Edge::Top, true);
        window.set_anchor(Edge::Left, true);
        window.set_margin(Edge::Top, cursor_y + cursor_h + 4);
        window.set_margin(Edge::Left, cursor_x);

        unim_log!(
            "INDICATOR",
            "[Popup] Wayland layer-shell 팝업 배치: margin=({},{})",
            cursor_x,
            cursor_y + cursor_h + 4
        );
    }

    #[cfg(not(feature = "wayland"))]
    {
        let _ = (window, cursor_x, cursor_y, cursor_h);
        unim_log!(
            "INDICATOR",
            "[Popup] wayland feature 미활성, Wayland 팝업 불가"
        );
    }
}

/// 팝업 위치를 화면 경계 내로 보정
#[allow(dead_code)]
pub fn clamp_to_screen(
    popup_x: i32,
    popup_y: i32,
    popup_w: i32,
    popup_h: i32,
    cursor_y: i32,
) -> (i32, i32) {
    let display = gtk4::gdk::Display::default().expect("No display");
    let monitors = display.monitors();

    let (screen_w, screen_h) = if monitors.n_items() > 0 {
        let monitor = monitors.item(0).and_downcast::<gtk4::gdk::Monitor>().unwrap();
        let geo = monitor.geometry();
        (geo.width(), geo.height())
    } else {
        (1920, 1080)
    };

    let mut x = popup_x;
    let mut y = popup_y;

    if x + popup_w > screen_w {
        x = screen_w - popup_w - 4;
    }
    if y + popup_h > screen_h {
        y = cursor_y - popup_h - 4;
    }
    if x < 0 {
        x = 4;
    }
    if y < 0 {
        y = 4;
    }

    (x, y)
}
