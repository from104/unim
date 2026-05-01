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

/// X11: 팝업 타입 설정 + 절대좌표 배치
fn position_popup_x11(window: &gtk4::Window, cursor_x: i32, cursor_y: i32, cursor_h: i32) {
    // daemon이 보내는 좌표는 X11 물리 픽셀 — XMoveWindow도 물리 픽셀이므로 스케일 보정 불필요
    let popup_x = cursor_x;
    let popup_y = cursor_y + cursor_h + 4;

    // 화면 경계 보정 (X11 물리 좌표 기준)
    let (popup_x, popup_y) = clamp_to_screen_x11(popup_x, popup_y, 350, 350, cursor_y);

    #[cfg(feature = "gdk4-x11")]
    {
        // 이미 realize된 경우: 직접 이동
        if window.is_realized() {
            if let Some(surface) = window.surface() {
                if let Some(x11_surface) = surface.downcast_ref::<gdk4_x11::X11Surface>() {
                    x11_move_window(&surface.display(), x11_surface, popup_x, popup_y);
                }
            }
            return;
        }

        // 최초 realize 시: 팝업 타입 설정 + 위치 지정
        let cx = popup_x;
        let cy = popup_y;
        window.connect_realize(move |win| {
            let surface = match win.surface() {
                Some(s) => s,
                None => return,
            };

            if let Some(x11_surface) = surface.downcast_ref::<gdk4_x11::X11Surface>() {
                let display = surface.display();
                x11_set_popup_type(&display, x11_surface);
                x11_move_window(&display, x11_surface, cx, cy);
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

/// X11 윈도우 이동
#[cfg(feature = "gdk4-x11")]
fn x11_move_window(
    display: &gtk4::gdk::Display,
    x11_surface: &gdk4_x11::X11Surface,
    x: i32,
    y: i32,
) {
    unsafe {
        let x11_display = display.downcast_ref::<gdk4_x11::X11Display>().unwrap();
        let xdisplay = gdk4_x11::ffi::gdk_x11_display_get_xdisplay(
            gtk4::glib::translate::ToGlibPtr::to_glib_none(x11_display).0,
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

        XMoveWindow(xdisplay as *mut _, xid as _, x, y);
        XFlush(xdisplay as *mut _);

        unim_log!(
            "INDICATOR",
            "[Popup] X11 윈도우 배치: pos=({},{}), xid={}",
            x,
            y,
            xid
        );
    }
}

/// Xlib XSetWindowAttributes (override_redirect 설정용)
#[repr(C)]
struct XSetWindowAttributes {
    background_pixmap: u64,
    background_pixel: u64,
    border_pixmap: u64,
    border_pixel: u64,
    bit_gravity: i32,
    win_gravity: i32,
    backing_store: i32,
    _pad1: i32,
    backing_planes: u64,
    backing_pixel: u64,
    save_under: i32,
    _pad2: i32,
    event_mask: i64,
    do_not_propagate_mask: i64,
    override_redirect: i32,
    _pad3: i32,
    colormap: u64,
    cursor: u64,
}

/// X11 윈도우를 override-redirect 팝업으로 설정 (WM 우회 + 포커스 방지)
#[cfg(feature = "gdk4-x11")]
fn x11_set_popup_type(display: &gtk4::gdk::Display, x11_surface: &gdk4_x11::X11Surface) {
    unsafe {
        let x11_display = display.downcast_ref::<gdk4_x11::X11Display>().unwrap();
        let xdisplay = gdk4_x11::ffi::gdk_x11_display_get_xdisplay(
            gtk4::glib::translate::ToGlibPtr::to_glib_none(x11_display).0,
        );
        let xid = gdk4_x11::ffi::gdk_x11_surface_get_xid(
            gtk4::glib::translate::ToGlibPtr::to_glib_none(x11_surface).0,
        );

        extern "C" {
            fn XInternAtom(
                display: *mut std::ffi::c_void,
                atom_name: *const std::ffi::c_char,
                only_if_exists: std::ffi::c_int,
            ) -> std::ffi::c_ulong;
            fn XChangeProperty(
                display: *mut std::ffi::c_void,
                w: std::ffi::c_ulong,
                property: std::ffi::c_ulong,
                type_: std::ffi::c_ulong,
                format: std::ffi::c_int,
                mode: std::ffi::c_int,
                data: *const u8,
                nelements: std::ffi::c_int,
            ) -> std::ffi::c_int;
            fn XChangeWindowAttributes(
                display: *mut std::ffi::c_void,
                w: std::ffi::c_ulong,
                valuemask: std::ffi::c_ulong,
                attributes: *mut XSetWindowAttributes,
            ) -> std::ffi::c_int;
            fn XFlush(display: *mut std::ffi::c_void) -> std::ffi::c_int;
        }

        // override_redirect: WM을 우회하여 직접 위치 제어
        const CW_OVERRIDE_REDIRECT: std::ffi::c_ulong = 1 << 9;
        let mut attrs: XSetWindowAttributes = std::mem::zeroed();
        attrs.override_redirect = 1; // True
        XChangeWindowAttributes(
            xdisplay as *mut _,
            xid as _,
            CW_OVERRIDE_REDIRECT,
            &mut attrs,
        );

        // _NET_WM_WINDOW_TYPE_POPUP_MENU
        let wm_type = XInternAtom(
            xdisplay as *mut _,
            b"_NET_WM_WINDOW_TYPE\0".as_ptr() as *const _,
            0,
        );
        let popup_type = XInternAtom(
            xdisplay as *mut _,
            b"_NET_WM_WINDOW_TYPE_POPUP_MENU\0".as_ptr() as *const _,
            0,
        );
        const XA_ATOM: std::ffi::c_ulong = 4;
        const PROP_MODE_REPLACE: std::ffi::c_int = 0;

        XChangeProperty(
            xdisplay as *mut _,
            xid as _,
            wm_type,
            XA_ATOM,
            32,
            PROP_MODE_REPLACE,
            &popup_type as *const std::ffi::c_ulong as *const u8,
            1,
        );

        // _NET_WM_STATE_ABOVE: 항상 최상위
        let wm_state = XInternAtom(
            xdisplay as *mut _,
            b"_NET_WM_STATE\0".as_ptr() as *const _,
            0,
        );
        let state_above = XInternAtom(
            xdisplay as *mut _,
            b"_NET_WM_STATE_ABOVE\0".as_ptr() as *const _,
            0,
        );
        XChangeProperty(
            xdisplay as *mut _,
            xid as _,
            wm_state,
            XA_ATOM,
            32,
            PROP_MODE_REPLACE,
            &state_above as *const std::ffi::c_ulong as *const u8,
            1,
        );

        XFlush(xdisplay as *mut _);

        unim_log!(
            "INDICATOR",
            "[Popup] X11 팝업 타입 설정: override_redirect + POPUP_MENU + ABOVE, xid={}",
            xid
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
/// X11 물리 좌표 기준 화면 경계 보정
fn clamp_to_screen_x11(
    popup_x: i32,
    popup_y: i32,
    popup_w: i32,
    popup_h: i32,
    cursor_y: i32,
) -> (i32, i32) {
    // GDK monitor geometry는 논리 좌표 → scale을 곱해서 물리 좌표로 변환
    let display = gtk4::gdk::Display::default().expect("No display");
    let monitors = display.monitors();

    let (screen_w, screen_h) = if monitors.n_items() > 0 {
        let monitor = monitors
            .item(0)
            .and_downcast::<gtk4::gdk::Monitor>()
            .unwrap();
        let geo = monitor.geometry();
        let scale = monitor.scale_factor();
        (geo.width() * scale, geo.height() * scale)
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

/// Wayland 논리 좌표 기준 화면 경계 보정
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
        let monitor = monitors
            .item(0)
            .and_downcast::<gtk4::gdk::Monitor>()
            .unwrap();
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
