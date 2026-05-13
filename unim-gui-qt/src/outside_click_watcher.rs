//! Popup 외부 클릭 dismiss watcher (X11 한정).
//!
//! 단계 2 변형: popup 실제 좌표를 QML이 보고 → static POPUP_RECT에 저장 →
//! watcher thread가 매 polling cycle마다 read. winId helper 없이 정확한 좌표 확보.
//!
//! Wayland/Mac 등에선 `xcb::Connection::connect` 실패 후 즉시 종료(safe noop).

use std::sync::atomic::{AtomicBool, AtomicI32, Ordering};
use std::sync::{Arc, OnceLock};
use std::thread;
use std::time::Duration;

use unim::unim_log;

pub struct PopupRect {
    pub x: AtomicI32,
    pub y: AtomicI32,
    pub w: AtomicI32,
    pub h: AtomicI32,
}

static POPUP_RECT: OnceLock<Arc<PopupRect>> = OnceLock::new();

fn popup_rect() -> &'static Arc<PopupRect> {
    POPUP_RECT.get_or_init(|| {
        Arc::new(PopupRect {
            x: AtomicI32::new(0),
            y: AtomicI32::new(0),
            w: AtomicI32::new(0),
            h: AtomicI32::new(0),
        })
    })
}

/// QML이 popup 실제 좌표/크기를 보고할 때 호출.
pub fn set_popup_rect(x: i32, y: i32, w: i32, h: i32) {
    let r = popup_rect();
    r.x.store(x, Ordering::SeqCst);
    r.y.store(y, Ordering::SeqCst);
    r.w.store(w, Ordering::SeqCst);
    r.h.store(h, Ordering::SeqCst);
}

pub struct OutsideClickWatcher {
    active: Arc<AtomicBool>,
}

impl OutsideClickWatcher {
    pub fn new() -> Self {
        Self {
            active: Arc::new(AtomicBool::new(false)),
        }
    }

    /// dismiss 콜백을 받아 새 watcher thread 시작.
    /// popup 좌표는 static POPUP_RECT에서 매번 동적으로 read.
    pub fn start<F>(&self, on_dismiss: F)
    where
        F: Fn() + Send + 'static,
    {
        self.active.store(false, Ordering::SeqCst);
        self.active.store(true, Ordering::SeqCst);
        let active = self.active.clone();

        thread::spawn(move || {
            let (conn, _screen_num) = match xcb::Connection::connect(None) {
                Ok(c) => c,
                Err(e) => {
                    unim_log!(
                        "INDICATOR",
                        "[Popup-Qt] outside-click: xcb connect 실패: {:?}",
                        e
                    );
                    return;
                }
            };
            let setup = conn.get_setup();
            let screen = match setup.roots().next() {
                Some(s) => s,
                None => return,
            };
            let root = screen.root();
            unim_log!("INDICATOR", "[Popup-Qt] outside-click watcher 시작 (동적 rect)");

            let rect = popup_rect();
            let mut prev_pressed = false;

            while active.load(Ordering::SeqCst) {
                thread::sleep(Duration::from_millis(16));

                let cookie = conn.send_request(&xcb::x::QueryPointer { window: root });
                let reply = match conn.wait_for_reply(cookie) {
                    Ok(r) => r,
                    Err(_) => continue,
                };

                let mask = reply.mask();
                let is_pressed = mask.contains(xcb::x::KeyButMask::BUTTON1)
                    || mask.contains(xcb::x::KeyButMask::BUTTON2)
                    || mask.contains(xcb::x::KeyButMask::BUTTON3);

                if !prev_pressed && is_pressed {
                    let rx = reply.root_x() as i32;
                    let ry = reply.root_y() as i32;
                    let px = rect.x.load(Ordering::SeqCst);
                    let py = rect.y.load(Ordering::SeqCst);
                    let pw = rect.w.load(Ordering::SeqCst);
                    let ph = rect.h.load(Ordering::SeqCst);
                    let inside =
                        pw > 0 && ph > 0 && rx >= px && rx < px + pw && ry >= py && ry < py + ph;
                    unim_log!(
                        "INDICATOR",
                        "[Popup-Qt] click ({},{}) popup=({},{},{}x{}) inside={}",
                        rx,
                        ry,
                        px,
                        py,
                        pw,
                        ph,
                        inside
                    );
                    if !inside {
                        on_dismiss();
                        break;
                    }
                }
                prev_pressed = is_pressed;
            }
            unim_log!("INDICATOR", "[Popup-Qt] outside-click watcher 종료");
        });
    }

    pub fn stop(&self) {
        self.active.store(false, Ordering::SeqCst);
    }
}

impl Default for OutsideClickWatcher {
    fn default() -> Self {
        Self::new()
    }
}
