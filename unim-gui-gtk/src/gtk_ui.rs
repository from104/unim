//! GTK4/libadwaita UI
//!
//! 모드 팝업 윈도우, 한자/특수문자 팝업, CSS 스타일, 설정 다이얼로그.

use std::cell::RefCell;
use std::rc::Rc;
use std::sync::mpsc::Receiver;
use std::sync::{Arc, Mutex, RwLock};
use std::time::Duration;

use gtk4::glib;
use gtk4::prelude::*;
use libadwaita as adw;
use libadwaita::prelude::*;
use rust_i18n::t;
use unim::status::InputCategory;
use unim::unim_log;

use unim_gui_common::types::{GuiAction, IndicatorState};

use crate::emoji_popup::EmojiPopup;
use crate::hanja_popup::HanjaPopup;
use crate::settings_dialog;
use crate::special_popup::SpecialPopup;

/// GTK4/libadwaita 앱 실행
pub fn run_gtk_app(state: Arc<RwLock<IndicatorState>>, popup_rx: Arc<Mutex<Receiver<GuiAction>>>) {
    let app = adw::Application::builder()
        .application_id("io.github.from104.unim.gui")
        .build();

    app.connect_activate(move |app| {
        // 다크 모드 강제
        adw::StyleManager::default().set_color_scheme(adw::ColorScheme::ForceDark);

        load_css();
        let mode_window = build_popup_window(app, state.clone());

        // 한자/특수문자/이모지 팝업 생성
        let hanja_popup = Rc::new(RefCell::new(HanjaPopup::new(app)));
        let special_popup = Rc::new(RefCell::new(SpecialPopup::new(app)));
        let emoji_popup = Rc::new(RefCell::new(EmojiPopup::new(app)));

        let mode_window_clone = mode_window.clone();
        let popup_rx_clone = popup_rx.clone();
        let app_clone = app.clone();
        let hanja_clone = hanja_popup.clone();
        let special_clone = special_popup.clone();
        let emoji_clone = emoji_popup.clone();

        glib::timeout_add_local(Duration::from_millis(50), move || {
            if let Ok(rx) = popup_rx_clone.lock() {
                while let Ok(action) = rx.try_recv() {
                    match action {
                        GuiAction::ShowModePopup => {
                            mode_window_clone.present();
                        }
                        GuiAction::UpdateCategory(_category) => {
                            // UI 업데이트는 창이 표시될 때 자동 처리
                        }
                        GuiAction::OpenSettings => {
                            settings_dialog::show_settings_dialog(&app_clone);
                        }
                        GuiAction::ShowHanjaPopup {
                            context_path,
                            target,
                            candidates,
                            x,
                            y,
                            w,
                            h,
                        } => {
                            // 특수문자 팝업이 열려있으면 닫기
                            special_clone.borrow().hide();
                            hanja_clone.borrow_mut().show(
                                context_path,
                                &target,
                                candidates,
                                x,
                                y,
                                w,
                                h,
                            );
                        }
                        GuiAction::ShowSpecialPopup {
                            context_path,
                            target,
                            characters,
                            top_row,
                            x,
                            y,
                            w,
                            h,
                        } => {
                            // 한자 팝업이 열려있으면 닫기
                            hanja_clone.borrow().hide();
                            emoji_clone.borrow().hide();
                            special_clone.borrow_mut().show(
                                context_path,
                                &target,
                                characters,
                                top_row,
                                x,
                                y,
                                w,
                                h,
                            );
                        }
                        GuiAction::ShowEmojiPopup {
                            context_path,
                            x,
                            y,
                            w,
                            h,
                        } => {
                            hanja_clone.borrow().hide();
                            special_clone.borrow().hide();
                            emoji_clone.borrow_mut().show(context_path, x, y, w, h);
                        }
                        GuiAction::HidePopup => {
                            hanja_clone.borrow().hide();
                            special_clone.borrow().hide();
                            emoji_clone.borrow().hide();
                        }
                        GuiAction::HanjaBookmarkChanged { index, bookmarked } => {
                            if hanja_clone.borrow().is_visible() {
                                hanja_clone
                                    .borrow_mut()
                                    .set_bookmark(index, bookmarked);
                            }
                        }
                        GuiAction::HanjaBookmarkStatesFetched { states } => {
                            // 첫 렌더 색상 누락 방지: 일괄 setter로 update_page 강제
                            if hanja_clone.borrow().is_visible() {
                                hanja_clone.borrow_mut().set_bookmark_flags(states);
                            }
                        }
                        GuiAction::HanjaCandidatesReordered {
                            candidates,
                            bookmarks,
                            new_cursor: _,
                            page,
                            sel_row,
                            sel_col,
                        } => {
                            // 즐겨찾기 토글 후 재정렬 — 후보·즐겨찾기·커서 일괄 갱신
                            if hanja_clone.borrow().is_visible() {
                                hanja_clone.borrow_mut().replace_candidates(
                                    candidates, bookmarks, page, sel_row, sel_col,
                                );
                            }
                        }
                        GuiAction::PopupNavigate {
                            page,
                            total_pages,
                            selected,
                            rows,
                            cols,
                            sel_row,
                            sel_col,
                        } => {
                            if hanja_clone.borrow().is_visible() {
                                hanja_clone.borrow_mut().navigate(
                                    page,
                                    total_pages,
                                    selected,
                                    rows,
                                    cols,
                                    sel_row,
                                    sel_col,
                                );
                            }
                            if special_clone.borrow().is_visible() {
                                special_clone.borrow_mut().navigate(
                                    page,
                                    total_pages,
                                    selected,
                                    rows,
                                    cols,
                                    sel_row,
                                    sel_col,
                                );
                            }
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
        // 설정 다이얼로그는 시스템 테마를 따른다 (다른 창의 ForceDark와 독립)
        adw::StyleManager::default().set_color_scheme(adw::ColorScheme::Default);
        settings_dialog::show_settings_dialog(app);
    });

    app.run_with_args::<String>(&[]);
}

/// CSS 스타일 로드
fn load_css() {
    let provider = gtk4::CssProvider::new();

    // 기존 모드 팝업 CSS + 한자/특수문자 팝업 CSS
    let css = format!(
        "{}\n{}\n{}\n{}",
        MODE_POPUP_CSS,
        crate::hanja_popup::popup_css(),
        crate::special_popup::popup_css(),
        crate::emoji_popup::popup_css()
    );
    provider.load_from_data(&css);

    gtk4::style_context_add_provider_for_display(
        &gtk4::gdk::Display::default().expect("Could not connect to a display."),
        &provider,
        gtk4::STYLE_PROVIDER_PRIORITY_APPLICATION,
    );
}

/// 모드 팝업 CSS (기존)
const MODE_POPUP_CSS: &str = r#"
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
"#;

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
            InputCategory::Korean => t!("modepopup_status_korean"),
            InputCategory::English => t!("modepopup_status_english"),
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

    // "한"/"A"는 mnemonic이라 번역 대상 아님 (한국어 글자 자체가 시각 식별자).
    let korean_btn = gtk4::Button::builder()
        .label("한")
        .tooltip_text(t!("modepopup_btn_korean_tooltip"))
        .build();
    korean_btn.add_css_class("mode-button");
    korean_btn.add_css_class("korean-btn");

    let english_btn = gtk4::Button::builder()
        .label("A")
        .tooltip_text(t!("modepopup_btn_english_tooltip"))
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
        .label(t!("modepopup_btn_open_settings"))
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
            status_badge_clone.set_text(&t!("modepopup_status_korean"));
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
            status_badge_clone2.set_text(&t!("modepopup_status_english"));
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
