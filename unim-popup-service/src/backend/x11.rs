//! 팝업 윈도우 포지셔닝 및 디스플레이 서버 분기
//!
//! X11: GDK X11 surface를 통한 위치 설정
//! Wayland (Sway/Hyprland): gtk4-layer-shell
//! GNOME Wayland: 팝업 생성 건너뜀 (extension 처리)
//!
//! `DisplayServer` enum, `detect_display_server`, `is_gnome_wayland`는
//! `unim-gui-common::popup_position`으로 추출됨. 여기서 re-export.

use gtk4::prelude::*;
use unim::unim_log;

pub use unim_gui_common::popup_position::{is_gnome_wayland, DisplayServer};

/// 현재 디스플레이 서버 환경 감지 (GDK 백엔드 확인 필요로 GTK 측에 유지)
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

/// X11: 팝업 타입 설정 + 화면 정중앙 배치 (실시간 산출)
///
/// 사용자(기현) 정책: cursor 위치 추적 시 fractional scaling·multi-monitor·CSD
/// 좌표 변환 누적 오차로 popup 이 어긋난 위치에 떠 인지 부담 ↑.
/// 항상 화면 정중앙에 고정해 위치 일관성·예측 가능성 확보.
///
/// 화면 크기와 popup 크기를 매 호출 시점 실시간 산출:
///   - 화면: `monitor.geometry × scale_factor` (X11 framebuffer 물리 픽셀)
///   - popup: `window.measure(Horizontal/Vertical, -1)` natural size
fn position_popup_x11(window: &gtk4::Window, _cursor_x: i32, _cursor_y: i32, _cursor_h: i32) {
    // 화면 크기 + scale_factor (X11 framebuffer 물리 픽셀)
    let display = gtk4::gdk::Display::default().expect("No display");
    let monitors = display.monitors();
    let (screen_w, screen_h, scale) = if monitors.n_items() > 0 {
        let monitor = monitors
            .item(0)
            .and_downcast::<gtk4::gdk::Monitor>()
            .unwrap();
        let geo = monitor.geometry();
        let s = monitor.scale_factor().max(1);
        (geo.width() * s, geo.height() * s, s)
    } else {
        (1920, 1080, 1)
    };

    // popup natural size 측정 — content 갱신 후 호출되므로 preferred size 반영.
    // measure() 결과는 logical 픽셀 → screen 과 단위 일관성 위해 scale 곱하기로 framebuffer 변환.
    // realize 전이라도 measure 자체는 가능. 측정 0 인 경우 100×100 logical fallback.
    let (_, nat_w, _, _) = window.measure(gtk4::Orientation::Horizontal, -1);
    let (_, nat_h, _, _) = window.measure(gtk4::Orientation::Vertical, -1);
    let popup_w = if nat_w > 0 { nat_w * scale } else { 100 * scale };
    let popup_h = if nat_h > 0 { nat_h * scale } else { 100 * scale };

    let popup_x = (screen_w - popup_w) / 2;
    let popup_y = (screen_h - popup_h) / 2;

    unim_log!(
        "INDICATOR",
        "[Popup] position_popup_x11: 정중앙 → pos=({},{}) popup=({}x{} phys, scale={}) screen=({}x{})",
        popup_x,
        popup_y,
        popup_w,
        popup_h,
        scale,
        screen_w,
        screen_h
    );

    #[cfg(feature = "x11-backend")]
    {
        // 이미 realize된 경우: 직접 이동
        if window.is_realized() {
            if let Some(surface) = window.surface() {
                if let Some(x11_surface) = surface.downcast_ref::<gdk4_x11::X11Surface>() {
                    unim_log!(
                        "INDICATOR",
                        "[Popup] window 이미 realized → 직접 XMoveWindow"
                    );
                    x11_move_window(&surface.display(), x11_surface, popup_x, popup_y);
                }
            }
            return;
        }

        // 최초 realize 시: 팝업 타입 설정 + 위치 지정
        let cx = popup_x;
        let cy = popup_y;
        unim_log!(
            "INDICATOR",
            "[Popup] window 미realize → connect_realize 예약: target=({},{})",
            cx,
            cy
        );
        window.connect_realize(move |win| {
            let surface = match win.surface() {
                Some(s) => s,
                None => {
                    unim_log!("INDICATOR", "[Popup] realize 콜백: surface=None — skip");
                    return;
                }
            };

            if let Some(x11_surface) = surface.downcast_ref::<gdk4_x11::X11Surface>() {
                let display = surface.display();
                unim_log!(
                    "INDICATOR",
                    "[Popup] realize 콜백 진입 → set_popup_type + XMoveWindow({},{})",
                    cx,
                    cy
                );
                x11_set_popup_type(&display, x11_surface);
                x11_move_window(&display, x11_surface, cx, cy);
            } else {
                unim_log!(
                    "INDICATOR",
                    "[Popup] realize 콜백: x11_surface 다운캐스트 실패 — Wayland surface?"
                );
            }
        });
    }

    #[cfg(not(feature = "x11-backend"))]
    {
        let _ = (window, popup_x, popup_y);
        unim_log!(
            "INDICATOR",
            "[Popup] gdk4-x11 feature 미활성, X11 팝업 위치 설정 불가"
        );
    }
}

/// X11 외부 클릭 dismiss 설정 — **grab 없는 QueryPointer polling**
///
/// 이전 시도(GrabPointer Sync + ReplayPointer)는 GNOME mutter X11 환경에서 grab
/// status=Success를 받음에도 ButtonPress event가 우리 conn에 도달하지 않아 클릭이
/// 통째로 freeze되는 증상이 있었음 (mutter / GTK4 popup 내부 input redirect 와의
/// 미식별 충돌). grab 자체를 포기하고 **mouse pointer 상태 polling**으로 전환.
///
/// 동작:
/// 1. GrabPointer 호출하지 않음 → 모든 클릭은 X server가 원래대로 underlying client에
///    dispatch (= pass-through 자동 보장).
/// 2. 16ms timer로 `xcb::x::QueryPointer` 발송, root 좌표 + button mask 수신.
/// 3. button mask edge detection (prev=0 → curr=1)으로 press 시점 감지.
/// 4. press 좌표가 popup geometry 밖이면 → `on_dismiss()` 호출하고 polling 종료.
/// 5. inside는 무시(자동 dispatch로 popup widget이 받음).
///
/// 장점:
/// - grab 없음 → freeze 절대 발생 안 함
/// - pass-through 자동 (X server 정상 dispatch)
/// - mutter/GTK4 grab 충돌과 무관
///
/// 한계:
/// - press~다음 polling iteration(최대 16ms) 사이 race. 일반 click(>50ms)에 무해.
/// - inside-click 정확도는 popup widget의 normal event handling이 담당.
#[cfg(all(feature = "x11-backend", feature = "x11-backend"))]
pub fn x11_install_outside_click_handler(
    window: &gtk4::Window,
    on_dismiss: impl Fn() + 'static,
) {
    use gtk4::glib;
    use std::cell::Cell;
    use std::rc::Rc;

    let on_dismiss = Rc::new(on_dismiss);
    let on_dismiss_for_map = on_dismiss.clone();

    window.connect_map(move |win| {
        // popup window의 X11 xid
        let surface = match win.surface() {
            Some(s) => s,
            None => return,
        };
        let x11_surface = match surface.downcast_ref::<gdk4_x11::X11Surface>() {
            Some(s) => s,
            None => return,
        };
        let popup_xid_raw: u32 = unsafe {
            gdk4_x11::ffi::gdk_x11_surface_get_xid(
                gtk4::glib::translate::ToGlibPtr::to_glib_none(x11_surface).0,
            ) as u32
        };

        // 별도 XCB connection (read-only — query 전용, grab 없음)
        let (conn, _screen_num) = match xcb::Connection::connect(None) {
            Ok(v) => v,
            Err(e) => {
                unim_log!("INDICATOR", "[Popup] xcb connect 실패: {:?}", e);
                return;
            }
        };
        let setup = conn.get_setup();
        let screen = match setup.roots().next() {
            Some(s) => s,
            None => return,
        };
        let root: xcb::x::Window = screen.root();
        let popup_window: xcb::x::Window = xcb::XidNew::new(popup_xid_raw);

        unim_log!(
            "INDICATOR",
            "[Popup] polling 모드 시작 (grab 없음, pass-through 자동): popup_xid={}",
            popup_xid_raw
        );

        let win_weak = win.downgrade();
        let on_dismiss_poll = on_dismiss_for_map.clone();
        let conn_rc = Rc::new(conn);
        let conn_for_timer = conn_rc.clone();
        let prev_pressed = Rc::new(Cell::new(false));

        glib::timeout_add_local(std::time::Duration::from_millis(16), move || {
            let conn = &*conn_for_timer;

            let win = match win_weak.upgrade() {
                Some(w) => w,
                None => return glib::ControlFlow::Break,
            };
            if !win.is_visible() {
                return glib::ControlFlow::Break;
            }

            // QueryPointer로 현재 mouse 좌표 + button mask
            let qp = conn.send_request(&xcb::x::QueryPointer { window: root });
            let reply = match conn.wait_for_reply(qp) {
                Ok(r) => r,
                Err(e) => {
                    unim_log!("INDICATOR", "[Popup] QueryPointer 실패: {:?}", e);
                    return glib::ControlFlow::Continue;
                }
            };

            let mask = reply.mask();
            let is_pressed = mask.contains(xcb::x::KeyButMask::BUTTON1)
                || mask.contains(xcb::x::KeyButMask::BUTTON2)
                || mask.contains(xcb::x::KeyButMask::BUTTON3);

            let prev = prev_pressed.get();
            prev_pressed.set(is_pressed);

            // edge detection: 0 → 1 (press 발생)
            if !prev && is_pressed {
                let root_x = reply.root_x() as i32;
                let root_y = reply.root_y() as i32;

                let geo_cookie = conn.send_request(&xcb::x::GetGeometry {
                    drawable: xcb::x::Drawable::Window(popup_window),
                });
                let inside = match conn.wait_for_reply(geo_cookie) {
                    Ok(reply) => {
                        let px = reply.x() as i32;
                        let py = reply.y() as i32;
                        let pw = reply.width() as i32;
                        let ph = reply.height() as i32;
                        let inside = root_x >= px && root_x < px + pw
                            && root_y >= py && root_y < py + ph;
                        unim_log!(
                            "INDICATOR",
                            "[Popup] click edge: pos=({},{}) popup=({},{},{}x{}) inside={}",
                            root_x, root_y, px, py, pw, ph, inside
                        );
                        inside
                    }
                    Err(e) => {
                        unim_log!("INDICATOR", "[Popup] GetGeometry 실패 → outside: {:?}", e);
                        false
                    }
                };

                if !inside {
                    unim_log!("INDICATOR", "[Popup] outside click 감지 → dismiss");
                    (on_dismiss_poll)();
                    return glib::ControlFlow::Break;
                }
                // inside: popup widget이 자체 처리 (GTK이 normal dispatch로 받음)
            }

            glib::ControlFlow::Continue
        });
    });

    let _ = on_dismiss; // Rc 소유권 유지
}

/// xcb feature 비활성 빌드용 stub
#[cfg(all(feature = "x11-backend", not(feature = "x11-backend")))]
pub fn x11_install_outside_click_handler(
    window: &gtk4::Window,
    on_dismiss: impl Fn() + 'static,
) {
    let _ = (window, on_dismiss);
    unim_log!("INDICATOR", "[Popup] xcb feature 미활성 — outside-click dismiss 불가");
}

/// X11 윈도우 이동
#[cfg(feature = "x11-backend")]
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
#[cfg(feature = "x11-backend")]
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
            c"_NET_WM_WINDOW_TYPE".as_ptr(),
            0,
        );
        let popup_type = XInternAtom(
            xdisplay as *mut _,
            c"_NET_WM_WINDOW_TYPE_POPUP_MENU".as_ptr(),
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
            c"_NET_WM_STATE".as_ptr(),
            0,
        );
        let state_above = XInternAtom(
            xdisplay as *mut _,
            c"_NET_WM_STATE_ABOVE".as_ptr(),
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
///
/// 본 경로는 `wayland-backend` feature 가 켜진 경우에만 실 동작한다.
/// 시스템에 `libgtk4-layer-shell` 라이브러리가 없는 환경(예: Ubuntu 24.04 noble,
/// Plasma 5.x 표준 패키지)에서는 feature off 로 빌드되어 popup 표시가 불가하다.
/// 특히 KDE Plasma 5.x Wayland 는 공식 미지원 — X11 세션 또는 GNOME 사용을 권장.
fn position_popup_wayland(window: &gtk4::Window, cursor_x: i32, cursor_y: i32, cursor_h: i32) {
    #[cfg(feature = "wayland-backend")]
    {
        use gtk4_layer_shell::{Edge, KeyboardMode, Layer, LayerShell};

        if !gtk4_layer_shell::is_supported() {
            unim_log!(
                "INDICATOR",
                "[Popup] gtk4-layer-shell 미지원, 팝업 표시 불가"
            );
            return;
        }

        window.init_layer_shell();
        window.set_layer(Layer::Overlay);
        window.set_keyboard_mode(KeyboardMode::None);

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

    #[cfg(not(feature = "wayland-backend"))]
    {
        let _ = (window, cursor_x, cursor_y, cursor_h);
        unim_log!(
            "INDICATOR",
            "[Popup] wayland feature 미활성, Wayland 팝업 불가 (KDE Plasma 5.x Wayland 미지원 — X11 세션 권장)"
        );
    }
}

// clamp_to_screen_x11 제거됨 — popup 이 항상 화면 정중앙에 고정되어 화면 경계 보정 불필요.
