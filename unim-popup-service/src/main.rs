//! UNIM popup service — standalone process.
//!
//! 책임: daemon DBus signal로부터 popup ViewModel을 받아 GTK4로 렌더링 + 트레이.
//! X11/Wayland 환경 자동 검출 후 적절한 backend 사용.

#![allow(dead_code)]

mod backend;
mod popup;
mod single_instance;

use std::sync::{Arc, RwLock};
use unim::unim_log;

rust_i18n::i18n!("locales", fallback = "en");

fn init_locale() {
    let lang = std::env::var("LC_ALL")
        .ok()
        .filter(|s| !s.is_empty())
        .or_else(|| std::env::var("LC_MESSAGES").ok().filter(|s| !s.is_empty()))
        .or_else(|| std::env::var("LANG").ok())
        .unwrap_or_default();
    let locale = if lang.starts_with("ko") { "ko" } else { "en" };
    rust_i18n::set_locale(locale);
}

fn detect_session() -> &'static str {
    if std::env::var("WAYLAND_DISPLAY").is_ok() {
        "wayland"
    } else if std::env::var("DISPLAY").is_ok() {
        "x11"
    } else {
        "unknown"
    }
}

fn main() {
    // 단일 인스턴스 검증
    let _lock = match single_instance::acquire() {
        Some(f) => f,
        None => {
            eprintln!("[unim-popup-service] 이미 다른 인스턴스가 실행 중입니다.");
            return;
        }
    };

    init_locale();
    unim_log!("INDICATOR", "UNIM popup-service 시작 (session={})", detect_session());

    // Phase 1: skeleton — GTK Application 초기화만, 실제 popup·tray는 Phase 2에서.
    let _state: Arc<RwLock<()>> = Arc::new(RwLock::new(()));

    let app = gtk4::Application::builder()
        .application_id("org.atit.unim.PopupService")
        .flags(gtk4::gio::ApplicationFlags::FLAGS_NONE)
        .build();

    // GTK signal로 activate 발사되도록 ID activate connect
    use gtk4::prelude::*;
    app.connect_activate(|_| {
        unim_log!("INDICATOR", "[popup-service] GTK application activated");
    });

    // 빈 인자로 실행 (CLI 인자 무시)
    app.run_with_args::<&str>(&[]);
}
