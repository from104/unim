//! 팝업 윈도우 포지셔닝 유틸리티
//!
//! X11 환경에서 GTK4 윈도우를 커서 위치에 배치합니다.
//! Wayland에서는 GNOME Shell 확장이 담당하므로 no-op입니다.

use gtk4::prelude::*;
use unim::unim_log;

/// 팝업 윈도우를 커서 좌표 아래에 배치합니다.
///
/// - X11: GDK X11 FFI를 통해 XMoveWindow 직접 호출
/// - Wayland: 불가능 (GNOME Shell 확장이 담당)
pub fn position_popup_window(
    window: &gtk4::Window,
    cursor_x: i32,
    cursor_y: i32,
    cursor_height: i32,
) {
    let x = cursor_x.max(0);
    let y = (cursor_y + cursor_height + 4).max(0);

    window.present();

    #[cfg(feature = "gdk4-x11")]
    try_x11_position(window, x, y);

    #[cfg(not(feature = "gdk4-x11"))]
    {
        let _ = (x, y);
        unim_log!(
            "INDICATOR",
            "gdk4-x11 미포함 — 팝업 포지셔닝 생략 (Wayland에서는 GNOME 확장이 담당)"
        );
    }
}

/// X11에서 GDK X11 FFI를 통해 윈도우 위치 설정
///
/// gdk4-x11의 `xlib` feature를 사용하면 x11 크레이트 의존성이 필요하므로,
/// GDK/Xlib C API를 직접 FFI 호출하여 의존성을 최소화한다.
#[cfg(feature = "gdk4-x11")]
fn try_x11_position(window: &gtk4::Window, x: i32, y: i32) {
    use gdk4_x11::prelude::*;

    let Some(surface) = window.surface() else {
        return;
    };

    let Some(x11_surface) = surface.downcast_ref::<gdk4_x11::X11Surface>() else {
        unim_log!(
            "INDICATOR",
            "Wayland surface — 팝업 포지셔닝은 GNOME 확장이 담당"
        );
        return;
    };

    let Ok(x11_display) = surface.display().downcast::<gdk4_x11::X11Display>() else {
        return;
    };

    // C FFI — GDK X11 + Xlib 직접 호출
    extern "C" {
        fn gdk_x11_display_get_xdisplay(display: *mut std::ffi::c_void) -> *mut std::ffi::c_void;
        fn gdk_x11_surface_get_xid(surface: *mut std::ffi::c_void) -> u64;
        fn XMoveWindow(display: *mut std::ffi::c_void, w: u64, x: i32, y: i32) -> i32;
        fn XFlush(display: *mut std::ffi::c_void) -> i32;
    }

    unsafe {
        // GObject의 raw pointer를 직접 가져옴
        let display_ptr = x11_display.as_ptr() as *mut std::ffi::c_void;
        let surface_ptr = x11_surface.as_ptr() as *mut std::ffi::c_void;

        let xdisplay = gdk_x11_display_get_xdisplay(display_ptr);
        let xid = gdk_x11_surface_get_xid(surface_ptr);

        if xdisplay.is_null() || xid == 0 {
            unim_log!("INDICATOR", "X11 정보 획득 실패 — 포지셔닝 생략");
            return;
        }

        XMoveWindow(xdisplay, xid, x, y);
        XFlush(xdisplay);
    }

    unim_log!("INDICATOR", "X11 팝업 위치 설정: ({}, {})", x, y);
}
