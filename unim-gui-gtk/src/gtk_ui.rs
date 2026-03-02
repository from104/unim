//! GTK4/libadwaita UI
//!
//! 모드 팝업 윈도우, CSS 스타일, 설정 다이얼로그.
//! 이 모듈은 GTK4에 의존하므로 `unim-gui-common`에는 포함되지 않습니다.

use std::sync::mpsc::Receiver;
use std::sync::{Arc, Mutex, RwLock};
use std::time::Duration;

use gtk4::glib;
use gtk4::prelude::*;
use libadwaita as adw;
use libadwaita::prelude::*;
use unim::status::InputCategory;
use unim::unim_log;

use unim_gui_common::types::{GuiAction, IndicatorState};

use crate::settings_dialog;

/// GTK4/libadwaita 앱 실행
pub fn run_gtk_app(state: Arc<RwLock<IndicatorState>>, popup_rx: Arc<Mutex<Receiver<GuiAction>>>) {
    let app = adw::Application::builder()
        .application_id("io.github.from104.unim.gui")
        .build();

    app.connect_activate(move |app| {
        // 다크 모드 강제
        adw::StyleManager::default().set_color_scheme(adw::ColorScheme::ForceDark);

        load_css();
        let window = build_popup_window(app, state.clone());

        let window_clone = window.clone();
        let popup_rx_clone = popup_rx.clone();
        let app_clone = app.clone();
        glib::timeout_add_local(Duration::from_millis(50), move || {
            if let Ok(rx) = popup_rx_clone.lock() {
                while let Ok(action) = rx.try_recv() {
                    match action {
                        GuiAction::ShowModePopup => {
                            window_clone.present();
                        }
                        GuiAction::UpdateCategory(_category) => {
                            // UI 업데이트는 창이 표시될 때 자동 처리
                        }
                        GuiAction::OpenSettings => {
                            settings_dialog::show_settings_dialog(&app_clone);
                        }
                    }
                }
            }
            glib::ControlFlow::Continue
        });
    });

    app.run_with_args::<String>(&[]);
}

/// `unim-gui-gtk --settings` 모드: 설정 다이얼로그만 표시하고 종료
pub fn run_settings_only() {
    let app = adw::Application::builder()
        .application_id("io.github.from104.unim.settings")
        .build();

    app.connect_activate(|app| {
        adw::StyleManager::default().set_color_scheme(adw::ColorScheme::ForceDark);
        settings_dialog::show_settings_dialog(app);
    });

    app.run_with_args::<String>(&[]);
}

/// CSS 스타일 로드
fn load_css() {
    let provider = gtk4::CssProvider::new();
    provider.load_from_data(
        r#"
        /* 프리미엄 다크 테마 디자인 */
        window.popup-window {
            background-color: #1e1e2e;
            color: #cdd6f4;
            border: 1px solid rgba(255, 255, 255, 0.1);
            border-radius: 20px;
            box-shadow: 0 10px 30px rgba(0, 0, 0, 0.5);
        }

        .main-container {
            padding: 24px;
        }
        
        .mode-tile-container {
            margin-bottom: 20px;
        }

        .mode-button {
            min-width: 100px;
            min-height: 100px;
            font-size: 38px;
            font-weight: 800;
            border-radius: 16px;
            background: rgba(255, 255, 255, 0.05);
            color: rgba(255, 255, 255, 0.6);
            border: 2px solid transparent;
            transition: all 250ms cubic-bezier(0.4, 0, 0.2, 1);
            box-shadow: 0 4px 6px rgba(0, 0, 0, 0.1);
        }
        
        .mode-button:hover {
            background: rgba(255, 255, 255, 0.1);
            transform: translateY(-2px);
            box-shadow: 0 6px 12px rgba(0, 0, 0, 0.2);
        }
        
        .korean-btn.mode-active {
            background: linear-gradient(135deg, #3584e4 0%, #1c71d8 100%);
            color: white;
            border-color: rgba(255, 255, 255, 0.3);
            box-shadow: 0 8px 20px rgba(53, 132, 228, 0.4);
        }

        .english-btn.mode-active {
            background: linear-gradient(135deg, #5e5c64 0%, #3d3d3d 100%);
            color: white;
            border-color: rgba(255, 255, 255, 0.3);
            box-shadow: 0 8px 20px rgba(0, 0, 0, 0.3);
        }
        
        .title-section {
            margin-bottom: 16px;
        }

        .title-label {
            font-size: 20px;
            font-weight: 800;
            color: white;
            letter-spacing: -0.5px;
        }
        
        .status-badge {
            font-size: 13px;
            font-weight: 500;
            padding: 4px 12px;
            border-radius: 20px;
            background: rgba(255, 255, 255, 0.08);
            color: rgba(255, 255, 255, 0.7);
        }

        .settings-button {
            background: transparent;
            color: rgba(255, 255, 255, 0.5);
            font-weight: 600;
            border-radius: 12px;
            padding: 8px 0;
            transition: all 200ms ease;
        }

        .settings-button:hover {
            background: rgba(255, 255, 255, 0.05);
            color: white;
        }

        "#,
    );

    gtk4::style_context_add_provider_for_display(
        &gtk4::gdk::Display::default().expect("Could not connect to a display."),
        &provider,
        gtk4::STYLE_PROVIDER_PRIORITY_APPLICATION,
    );
}

/// 팝업 윈도우 생성
fn build_popup_window(app: &adw::Application, state: Arc<RwLock<IndicatorState>>) -> adw::Window {
    let window = adw::Window::builder()
        .application(app)
        .default_width(320)
        .resizable(false)
        .deletable(true)
        .build();

    window.add_css_class("popup-window");

    let current_category = state
        .read()
        .map(|s| s.category)
        .unwrap_or(InputCategory::English);

    // 메인 컨테이너
    let main_box = gtk4::Box::new(gtk4::Orientation::Vertical, 0);
    main_box.add_css_class("main-container");

    // 타이틀 섹션
    let title_box = gtk4::Box::new(gtk4::Orientation::Vertical, 4);
    title_box.add_css_class("title-section");

    let title_label = gtk4::Label::builder()
        .label("UNIM")
        .halign(gtk4::Align::Start)
        .build();
    title_label.add_css_class("title-label");

    let status_badge = gtk4::Label::builder()
        .label(match current_category {
            InputCategory::Korean => "한국어 입력 중",
            InputCategory::English => "영어 입력 중",
        })
        .halign(gtk4::Align::Start)
        .build();
    status_badge.add_css_class("status-badge");

    title_box.append(&title_label);
    title_box.append(&status_badge);

    // 타일 버튼 섹션
    let button_box = gtk4::Box::new(gtk4::Orientation::Horizontal, 16);
    button_box.add_css_class("mode-tile-container");
    button_box.set_halign(gtk4::Align::Center);

    let korean_btn = gtk4::Button::builder()
        .label("한")
        .tooltip_text("한국어 모드로 전환")
        .build();
    korean_btn.add_css_class("mode-button");
    korean_btn.add_css_class("korean-btn");

    let english_btn = gtk4::Button::builder()
        .label("A")
        .tooltip_text("영어 모드로 전환")
        .build();
    english_btn.add_css_class("mode-button");
    english_btn.add_css_class("english-btn");

    match current_category {
        InputCategory::Korean => korean_btn.add_css_class("mode-active"),
        InputCategory::English => english_btn.add_css_class("mode-active"),
    }

    button_box.append(&korean_btn);
    button_box.append(&english_btn);

    // 설정 버튼
    let settings_btn = gtk4::Button::builder()
        .label("설정 도구 열기")
        .halign(gtk4::Align::Fill)
        .build();
    settings_btn.add_css_class("settings-button");

    main_box.append(&title_box);
    main_box.append(&button_box);
    main_box.append(&settings_btn);

    window.set_content(Some(&main_box));

    // 이벤트 핸들러
    let status_badge_clone = status_badge.clone();
    let korean_btn_clone = korean_btn.clone();
    let english_btn_clone = english_btn.clone();
    let state_clone = state.clone();
    korean_btn.connect_clicked(move |_| {
        if let Ok(mut s) = state_clone.write() {
            s.category = InputCategory::Korean;
            korean_btn_clone.add_css_class("mode-active");
            english_btn_clone.remove_css_class("mode-active");
            status_badge_clone.set_text("한국어 입력 중");
            unim_log!("INDICATOR", "한국어 모드로 전환");
        }
    });

    let status_badge_clone2 = status_badge.clone();
    let korean_btn_clone2 = korean_btn.clone();
    let english_btn_clone2 = english_btn.clone();
    let state_clone2 = state.clone();
    english_btn.connect_clicked(move |_| {
        if let Ok(mut s) = state_clone2.write() {
            s.category = InputCategory::English;
            english_btn_clone2.add_css_class("mode-active");
            korean_btn_clone2.remove_css_class("mode-active");
            status_badge_clone2.set_text("영어 입력 중");
            unim_log!("INDICATOR", "영어 모드로 전환");
        }
    });

    settings_btn.connect_clicked(|_| {
        open_settings();
    });

    // 창 닫기 관련 제어
    let window_clone = window.clone();
    let key_controller = gtk4::EventControllerKey::new();
    key_controller.connect_key_pressed(move |_, key, _, _| {
        if key == gtk4::gdk::Key::Escape {
            window_clone.close();
            glib::Propagation::Stop
        } else {
            glib::Propagation::Proceed
        }
    });
    window.add_controller(key_controller);

    let window_focus_clone = window.clone();
    window.connect_is_active_notify(move |w| {
        if !w.is_active() {
            window_focus_clone.set_visible(false);
        }
    });

    window
}

/// 내장 설정 다이얼로그를 GTK 이벤트 루프에 GuiAction으로 요청
fn open_settings() {
    use unim_gui_common::types::SETTINGS_TX;
    if let Ok(tx) = SETTINGS_TX.lock() {
        if let Some(tx) = tx.as_ref() {
            let _ = tx.send(GuiAction::OpenSettings);
        }
    }
}
