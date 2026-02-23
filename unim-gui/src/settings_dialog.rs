//! UNIM 설정 다이얼로그
//!
//! Rust/GTK4/libadwaita 기반 설정 UI.
//! `unim::config::Config`를 직접 사용하여 설정을 읽고 저장합니다.

use gtk4::glib;
use gtk4::prelude::*;
use libadwaita as adw;
use libadwaita::prelude::*;
use unim::config::{Config, EnglishLayout, InputCategory, KoreanLayout, ModeSharingMode};
use unim::unim_log;

use std::cell::RefCell;
use std::rc::Rc;

/// 설정 다이얼로그 상태
struct SettingsState {
    config: Config,
    updating: bool,
}

/// 설정 다이얼로그를 생성하고 표시합니다.
pub fn show_settings_dialog(app: &adw::Application) {
    let window = adw::Window::builder()
        .application(app)
        .title("UNIM 설정")
        .default_width(480)
        .default_height(-1)
        .resizable(false)
        .modal(true)
        .build();

    // 설정 로드
    let config = Config::load_from_default_path();
    let state = Rc::new(RefCell::new(SettingsState {
        config,
        updating: false,
    }));

    // 메인 컨테이너
    let outer_box = gtk4::Box::new(gtk4::Orientation::Vertical, 0);
    let header_bar = adw::HeaderBar::new();
    outer_box.append(&header_bar);

    let content = gtk4::Box::new(gtk4::Orientation::Vertical, 0);
    content.set_margin_start(24);
    content.set_margin_end(24);
    content.set_margin_top(8);
    content.set_margin_bottom(24);

    // ===== Keyboard Layout Section =====
    let layout_group = adw::PreferencesGroup::builder().title("자판 배열").build();

    // Korean Layout
    let korean_row = adw::ComboRow::builder().title("한국어 자판").build();
    let korean_items: Vec<&str> = KoreanLayout::all()
        .iter()
        .map(|l| l.display_name())
        .collect();
    let korean_list = gtk4::StringList::new(&korean_items);
    korean_row.set_model(Some(&korean_list));
    {
        let s = state.borrow();
        let idx = KoreanLayout::all()
            .iter()
            .position(|l| *l == s.config.engine.korean.layout)
            .unwrap_or(0);
        korean_row.set_selected(idx as u32);
    }
    let state_clone = state.clone();
    korean_row.connect_selected_notify(move |row| {
        let mut s = state_clone.borrow_mut();
        if s.updating {
            return;
        }
        let idx = row.selected() as usize;
        if let Some(layout) = KoreanLayout::all().get(idx) {
            s.config.engine.korean.layout = *layout;
            save_and_notify(&s.config, "korean_layout", &format!("{:?}", layout));
        }
    });
    layout_group.add(&korean_row);

    // English Layout
    let english_row = adw::ComboRow::builder().title("영어 자판").build();
    let english_items: Vec<&str> = EnglishLayout::all()
        .iter()
        .map(|l| l.display_name())
        .collect();
    let english_list = gtk4::StringList::new(&english_items);
    english_row.set_model(Some(&english_list));
    {
        let s = state.borrow();
        let idx = EnglishLayout::all()
            .iter()
            .position(|l| *l == s.config.engine.english.layout)
            .unwrap_or(0);
        english_row.set_selected(idx as u32);
    }
    let state_clone = state.clone();
    english_row.connect_selected_notify(move |row| {
        let mut s = state_clone.borrow_mut();
        if s.updating {
            return;
        }
        let idx = row.selected() as usize;
        if let Some(layout) = EnglishLayout::all().get(idx) {
            s.config.engine.english.layout = *layout;
            save_and_notify(&s.config, "english_layout", &format!("{:?}", layout));
        }
    });
    layout_group.add(&english_row);

    // Initial Mode
    let initial_mode_row = adw::ComboRow::builder().title("초기 입력 모드").build();
    let mode_items = gtk4::StringList::new(&["한국어", "영어"]);
    initial_mode_row.set_model(Some(&mode_items));
    {
        let s = state.borrow();
        let idx = match s.config.engine.default_category {
            InputCategory::Korean => 0,
            InputCategory::English => 1,
        };
        initial_mode_row.set_selected(idx);
    }
    let state_clone = state.clone();
    initial_mode_row.connect_selected_notify(move |row| {
        let mut s = state_clone.borrow_mut();
        if s.updating {
            return;
        }
        let cat = if row.selected() == 0 {
            InputCategory::Korean
        } else {
            InputCategory::English
        };
        s.config.engine.default_category = cat;
        save_and_notify(&s.config, "default_category", &format!("{:?}", cat));
    });
    layout_group.add(&initial_mode_row);

    // Mode Sharing
    let mode_sharing_row = adw::ComboRow::builder().title("모드 공유").build();
    let sharing_items: Vec<&str> = ModeSharingMode::all()
        .iter()
        .map(|m| m.display_name())
        .collect();
    let sharing_list = gtk4::StringList::new(&sharing_items);
    mode_sharing_row.set_model(Some(&sharing_list));
    {
        let s = state.borrow();
        let idx = ModeSharingMode::all()
            .iter()
            .position(|m| *m == s.config.engine.mode_sharing)
            .unwrap_or(0);
        mode_sharing_row.set_selected(idx as u32);
    }
    let state_clone = state.clone();
    mode_sharing_row.connect_selected_notify(move |row| {
        let mut s = state_clone.borrow_mut();
        if s.updating {
            return;
        }
        let idx = row.selected() as usize;
        if let Some(mode) = ModeSharingMode::all().get(idx) {
            s.config.engine.mode_sharing = *mode;
            save_and_notify(&s.config, "mode_sharing", &format!("{:?}", mode));
        }
    });
    layout_group.add(&mode_sharing_row);

    content.append(&layout_group);

    // ===== Auto Switch Section =====
    let switch_group = adw::PreferencesGroup::builder().title("자동 전환").build();

    let auto_switch_row = adw::ActionRow::builder().title("자동 전환 사용").build();
    let auto_switch_widget = gtk4::Switch::new();
    auto_switch_widget.set_valign(gtk4::Align::Center);
    {
        let s = state.borrow();
        auto_switch_widget.set_active(s.config.engine.auto_switch.enabled);
    }
    auto_switch_row.add_suffix(&auto_switch_widget);
    auto_switch_row.set_activatable_widget(Some(&auto_switch_widget));

    // Threshold row
    let threshold_row = adw::ActionRow::builder().title("감지 임계값").build();
    let threshold_box = gtk4::Box::new(gtk4::Orientation::Horizontal, 8);
    threshold_box.set_valign(gtk4::Align::Center);

    let threshold_scale = gtk4::Scale::with_range(gtk4::Orientation::Horizontal, 0.0, 1.0, 0.05);
    threshold_scale.set_draw_value(false);
    threshold_scale.set_size_request(140, -1);
    {
        let s = state.borrow();
        threshold_scale.set_value(s.config.engine.auto_switch.threshold as f64);
        threshold_scale.set_sensitive(s.config.engine.auto_switch.enabled);
    }

    let threshold_label = gtk4::Label::new(None);
    {
        let s = state.borrow();
        threshold_label.set_text(&format!(
            "{:.0}%",
            s.config.engine.auto_switch.threshold * 100.0
        ));
        threshold_label.set_sensitive(s.config.engine.auto_switch.enabled);
    }

    threshold_box.append(&threshold_scale);
    threshold_box.append(&threshold_label);
    threshold_row.add_suffix(&threshold_box);

    // Auto switch toggle callback
    let state_clone = state.clone();
    let scale_ref = threshold_scale.clone();
    let label_ref = threshold_label.clone();
    auto_switch_widget.connect_active_notify(move |sw| {
        let mut s = state_clone.borrow_mut();
        if s.updating {
            return;
        }
        let enabled = sw.is_active();
        s.config.engine.auto_switch.enabled = enabled;
        scale_ref.set_sensitive(enabled);
        label_ref.set_sensitive(enabled);
        save_and_notify(
            &s.config,
            "auto_switch_enabled",
            if enabled { "true" } else { "false" },
        );
    });

    // Threshold value callback
    let state_clone = state.clone();
    let label_ref2 = threshold_label.clone();
    threshold_scale.connect_value_changed(move |scale| {
        let mut s = state_clone.borrow_mut();
        if s.updating {
            return;
        }
        let val = scale.value() as f32;
        s.config.engine.auto_switch.threshold = val;
        label_ref2.set_text(&format!("{:.0}%", val * 100.0));
        save_and_notify(&s.config, "auto_switch_threshold", &format!("{}", val));
    });

    switch_group.add(&auto_switch_row);
    switch_group.add(&threshold_row);
    content.append(&switch_group);

    // Clamp for nice libadwaita look
    let clamp = adw::Clamp::builder()
        .maximum_size(600)
        .child(&content)
        .build();

    outer_box.append(&clamp);
    window.set_content(Some(&outer_box));
    window.present();

    unim_log!("INDICATOR", "설정 다이얼로그 표시");
}

/// 설정을 저장하고 DBus를 통해 데몬에 변경을 알립니다.
fn save_and_notify(config: &Config, key: &str, value: &str) {
    // 파일 저장
    if let Err(e) = config.save_to_default_path() {
        unim_log!("INDICATOR", "설정 저장 실패: {}", e);
    }

    // DBus 알림 (fire-and-forget)
    let key = key.to_string();
    let value = value.to_string();
    glib::spawn_future_local(async move {
        if let Ok(conn) = zbus::Connection::session().await {
            let _ = conn
                .call_method(
                    Some("org.atit.unim.InputMethod"),
                    "/org/atit/unim/InputMethod",
                    Some("org.atit.unim.InputMethod"),
                    "SetConfig",
                    &(&key, &value),
                )
                .await;
        }
    });
}
