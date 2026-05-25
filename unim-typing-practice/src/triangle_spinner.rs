//! 회전 삼각형 spinner — DESIGN.md §7.2 `tri-flip`.
//!
//! 14×14 영역의 SVG polygon `3,2 12,7 3,12` 를 X축 회전(rotateX) 시키는 효과.
//! cairo 의 `scale(1, cos(θ))` 로 평면 흉내. glib::timeout 으로 16ms 마다 갱신.

use std::cell::Cell;
use std::f64::consts::PI;
use std::rc::Rc;
use std::time::Instant;

use gtk4::prelude::*;
use gtk4::{self as gtk, glib};

const SIZE: i32 = 14;
const PERIOD_MS: u64 = 1100;

pub struct TriangleSpinner {
    area: gtk::DrawingArea,
    running: Rc<Cell<bool>>,
    start: Rc<Cell<Instant>>,
    timeout_armed: Rc<Cell<bool>>,
}

impl TriangleSpinner {
    pub fn new() -> Rc<Self> {
        let area = gtk::DrawingArea::new();
        area.set_content_width(SIZE);
        area.set_content_height(SIZE);
        area.set_halign(gtk::Align::Center);
        area.set_valign(gtk::Align::Center);

        let running: Rc<Cell<bool>> = Rc::new(Cell::new(false));
        let start: Rc<Cell<Instant>> = Rc::new(Cell::new(Instant::now()));

        let running_c = running.clone();
        let start_c = start.clone();
        area.set_draw_func(move |_area, cr, w, h| {
            let cx = w as f64 / 2.0;
            let cy = h as f64 / 2.0;

            let scale_y = if running_c.get() {
                let elapsed = start_c.get().elapsed().as_millis() as f64;
                let t = (elapsed % PERIOD_MS as f64) / PERIOD_MS as f64;
                (t * 2.0 * PI).cos()
            } else {
                1.0
            };
            let sy = scale_y.abs().max(0.05) * if scale_y >= 0.0 { 1.0 } else { -1.0 };

            cr.translate(cx, cy);
            cr.scale(1.0, sy);
            cr.translate(-7.0, -7.0);

            cr.move_to(3.0, 2.0);
            cr.line_to(12.0, 7.0);
            cr.line_to(3.0, 12.0);
            cr.close_path();

            // accent #1c66c9
            cr.set_source_rgba(0.11, 0.4, 0.79, 1.0);
            let _ = cr.fill();
        });

        Rc::new(Self {
            area,
            running,
            start,
            timeout_armed: Rc::new(Cell::new(false)),
        })
    }

    pub fn root(&self) -> &gtk::DrawingArea {
        &self.area
    }

    pub fn start(self: &Rc<Self>) {
        self.running.set(true);
        self.start.set(Instant::now());
        if self.timeout_armed.get() {
            return;
        }
        self.timeout_armed.set(true);

        let area = self.area.clone();
        let running = self.running.clone();
        let armed = self.timeout_armed.clone();
        glib::timeout_add_local(std::time::Duration::from_millis(16), move || {
            if running.get() {
                area.queue_draw();
                glib::ControlFlow::Continue
            } else {
                armed.set(false);
                glib::ControlFlow::Break
            }
        });
    }

    pub fn stop(&self) {
        self.running.set(false);
        self.area.queue_draw();
    }
}
