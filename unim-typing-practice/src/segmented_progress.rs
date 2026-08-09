//! 20-segment progress bar — DESIGN.md §7.6 ProgressCard.
//!
//! 균등 분할 segment, filled / partial / empty 3 상태. CSS 로 색·보더 제어.

use gtk4::prelude::*;
use gtk4::{self as gtk};

pub struct SegmentedProgress {
    root: gtk::Box,
    segments: Vec<gtk::Box>,
}

impl SegmentedProgress {
    pub fn new(count: usize) -> Self {
        let root = gtk::Box::new(gtk::Orientation::Horizontal, 2);
        root.set_homogeneous(true);
        let mut segments = Vec::with_capacity(count);
        for _ in 0..count {
            let s = gtk::Box::new(gtk::Orientation::Horizontal, 0);
            s.set_size_request(-1, 6);
            s.set_hexpand(true);
            s.add_css_class("typing-progress-seg");
            root.append(&s);
            segments.push(s);
        }
        Self { root, segments }
    }

    pub fn root(&self) -> &gtk::Box {
        &self.root
    }

    pub fn set_fraction(&self, p: f64) {
        let total = self.segments.len() as f64;
        if total == 0.0 {
            return;
        }
        let p = p.clamp(0.0, 1.0);
        let exact = p * total;
        let filled = exact.floor() as usize;
        let has_partial = (exact - filled as f64) > 0.001;
        for (i, w) in self.segments.iter().enumerate() {
            w.remove_css_class("typing-progress-seg-filled");
            w.remove_css_class("typing-progress-seg-partial");
            if i < filled {
                w.add_css_class("typing-progress-seg-filled");
            } else if has_partial && i == filled {
                w.add_css_class("typing-progress-seg-partial");
            }
        }
    }
}
