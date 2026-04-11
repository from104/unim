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

    // ===== Key Bindings Section =====
    let key_group = adw::PreferencesGroup::builder().title("키 설정").build();

    // Toggle keys (한/영 전환)
    let toggle_row = adw::ActionRow::builder().title("한/영 전환 키").build();
    let toggle_entry = gtk4::Entry::new();
    toggle_entry.set_valign(gtk4::Align::Center);
    toggle_entry.set_width_chars(20);
    {
        let s = state.borrow();
        toggle_entry.set_text(&s.config.engine.toggle_keys.join(", "));
    }
    toggle_row.add_suffix(&toggle_entry);
    let state_clone = state.clone();
    toggle_entry.connect_changed(move |entry| {
        let mut s = state_clone.borrow_mut();
        if s.updating {
            return;
        }
        let text = entry.text().to_string();
        let keys: Vec<String> = text
            .split(',')
            .map(|k| k.trim().to_string())
            .filter(|k| !k.is_empty())
            .collect();
        if !keys.is_empty() {
            let keys_str = keys.join(",");
            s.config.engine.toggle_keys = keys;
            save_and_notify(&s.config, "toggle_keys", &keys_str);
        }
    });
    key_group.add(&toggle_row);

    // Hanja keys (한자/특수문자)
    let hanja_row = adw::ActionRow::builder().title("한자 키").build();
    let hanja_entry = gtk4::Entry::new();
    hanja_entry.set_valign(gtk4::Align::Center);
    hanja_entry.set_width_chars(20);
    {
        let s = state.borrow();
        hanja_entry.set_text(&s.config.engine.hanja_keys.join(", "));
    }
    hanja_row.add_suffix(&hanja_entry);
    let state_clone = state.clone();
    hanja_entry.connect_changed(move |entry| {
        let mut s = state_clone.borrow_mut();
        if s.updating {
            return;
        }
        let text = entry.text().to_string();
        let keys: Vec<String> = text
            .split(',')
            .map(|k| k.trim().to_string())
            .filter(|k| !k.is_empty())
            .collect();
        if !keys.is_empty() {
            let keys_str = keys.join(",");
            s.config.engine.hanja_keys = keys;
            save_and_notify(&s.config, "hanja_keys", &keys_str);
        }
    });
    key_group.add(&hanja_row);

    content.append(&key_group);

    // ===== AutoTypeFix Section =====
    let atf_group = adw::PreferencesGroup::builder()
        .title("자동 오타 교정 (AutoTypeFix)")
        .description("입력 중 한/영 오타를 실시간으로 감지하여 자동 교정")
        .build();

    // Enabled toggle
    let atf_enabled_row = adw::ActionRow::builder()
        .title("자동 오타 교정 사용")
        .build();
    let atf_enabled_sw = gtk4::Switch::new();
    atf_enabled_sw.set_valign(gtk4::Align::Center);
    {
        let s = state.borrow();
        atf_enabled_sw.set_active(s.config.engine.auto_typefix.enabled);
    }
    atf_enabled_row.add_suffix(&atf_enabled_sw);
    atf_enabled_row.set_activatable_widget(Some(&atf_enabled_sw));
    atf_group.add(&atf_enabled_row);

    // 순방향 (영→한) toggle
    let fwd_row = adw::ActionRow::builder()
        .title("순방향 (영→한) 교정")
        .subtitle("영어 모드에서 한글을 치려고 한 경우")
        .build();
    let fwd_sw = gtk4::Switch::new();
    fwd_sw.set_valign(gtk4::Align::Center);
    {
        let s = state.borrow();
        fwd_sw.set_active(s.config.engine.auto_typefix.forward);
    }
    fwd_row.add_suffix(&fwd_sw);
    fwd_row.set_activatable_widget(Some(&fwd_sw));
    atf_group.add(&fwd_row);

    // 역방향 (한→영) toggle
    let rev_row = adw::ActionRow::builder()
        .title("역방향 (한→영) 교정")
        .subtitle("한글 모드에서 영어를 치려고 한 경우")
        .build();
    let rev_sw = gtk4::Switch::new();
    rev_sw.set_valign(gtk4::Align::Center);
    {
        let s = state.borrow();
        rev_sw.set_active(s.config.engine.auto_typefix.reverse);
    }
    rev_row.add_suffix(&rev_sw);
    rev_row.set_activatable_widget(Some(&rev_sw));
    atf_group.add(&rev_row);

    // Korean syllable threshold
    let kor_thresh_row = adw::ActionRow::builder()
        .title("한글 음절 임계값")
        .subtitle("영→한 교정에 필요한 완성 음절 수 (2~5)")
        .build();
    let kor_thresh_spin = gtk4::SpinButton::with_range(2.0, 5.0, 1.0);
    kor_thresh_spin.set_valign(gtk4::Align::Center);
    {
        let s = state.borrow();
        kor_thresh_spin.set_value(s.config.engine.auto_typefix.kor_syllable_threshold as f64);
    }
    kor_thresh_row.add_suffix(&kor_thresh_spin);
    atf_group.add(&kor_thresh_row);

    // English word min length
    let eng_len_row = adw::ActionRow::builder()
        .title("영문 단어 최소 길이")
        .subtitle("한→영 교정에 필요한 영문 단어 길이 (5~10)")
        .build();
    let eng_len_spin = gtk4::SpinButton::with_range(5.0, 10.0, 1.0);
    eng_len_spin.set_valign(gtk4::Align::Center);
    {
        let s = state.borrow();
        eng_len_spin.set_value(s.config.engine.auto_typefix.eng_word_min_length as f64);
    }
    eng_len_row.add_suffix(&eng_len_spin);
    atf_group.add(&eng_len_row);

    // Time window
    let time_row = adw::ActionRow::builder()
        .title("시간 윈도우 (ms)")
        .subtitle("이 시간 내의 연속 키스트로크만 검사 (500~5000)")
        .build();
    let time_spin = gtk4::SpinButton::with_range(500.0, 5000.0, 100.0);
    time_spin.set_valign(gtk4::Align::Center);
    {
        let s = state.borrow();
        time_spin.set_value(s.config.engine.auto_typefix.time_window_ms as f64);
    }
    time_row.add_suffix(&time_spin);
    atf_group.add(&time_row);

    // AutoTypeFix callbacks
    let state_clone = state.clone();
    atf_enabled_sw.connect_active_notify(move |sw| {
        let mut s = state_clone.borrow_mut();
        if s.updating { return; }
        s.config.engine.auto_typefix.enabled = sw.is_active();
        save_and_notify(&s.config, "auto_typefix", if sw.is_active() { "true" } else { "false" });
    });

    let state_clone = state.clone();
    fwd_sw.connect_active_notify(move |sw| {
        let mut s = state_clone.borrow_mut();
        if s.updating { return; }
        s.config.engine.auto_typefix.forward = sw.is_active();
        let _ = s.config.save_to_default_path();
    });

    let state_clone = state.clone();
    rev_sw.connect_active_notify(move |sw| {
        let mut s = state_clone.borrow_mut();
        if s.updating { return; }
        s.config.engine.auto_typefix.reverse = sw.is_active();
        let _ = s.config.save_to_default_path();
    });

    let state_clone = state.clone();
    kor_thresh_spin.connect_value_changed(move |spin| {
        let mut s = state_clone.borrow_mut();
        if s.updating { return; }
        s.config.engine.auto_typefix.kor_syllable_threshold = spin.value() as u8;
        let _ = s.config.save_to_default_path();
    });

    let state_clone = state.clone();
    eng_len_spin.connect_value_changed(move |spin| {
        let mut s = state_clone.borrow_mut();
        if s.updating { return; }
        s.config.engine.auto_typefix.eng_word_min_length = spin.value() as u8;
        let _ = s.config.save_to_default_path();
    });

    let state_clone = state.clone();
    time_spin.connect_value_changed(move |spin| {
        let mut s = state_clone.borrow_mut();
        if s.updating { return; }
        s.config.engine.auto_typefix.time_window_ms = spin.value() as u32;
        let _ = s.config.save_to_default_path();
    });

    content.append(&atf_group);

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
