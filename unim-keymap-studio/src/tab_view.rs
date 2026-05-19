//! 보기 탭 — 읽기 전용 메타데이터·키보드·선택 키 정보 패널.

use std::cell::RefCell;
use std::rc::Rc;

use gtk4::prelude::*;
use gtk4::{self as gtk};
use libadwaita::prelude::*;
use libadwaita::{self as adw};

use unim_keymap_common::KeyboardWidget;

use crate::app::SharedAppState;

pub fn build(state: SharedAppState) -> (gtk::Widget, Rc<dyn Fn()>) {
    let scroller = gtk::ScrolledWindow::builder()
        .hscrollbar_policy(gtk::PolicyType::Never)
        .build();

    let outer = gtk::Box::new(gtk::Orientation::Vertical, 12);
    outer.set_margin_top(16);
    outer.set_margin_bottom(16);
    outer.set_margin_start(16);
    outer.set_margin_end(16);

    // 1) 메타데이터 그룹
    let meta_group = adw::PreferencesGroup::builder()
        .title(&*rust_i18n::t!("view_metadata"))
        .build();
    let row_name = make_action_row(&rust_i18n::t!("dialog_field_name"), "");
    let row_lang = make_action_row(&rust_i18n::t!("meta_language"), "");
    let row_type = make_action_row(&rust_i18n::t!("meta_type"), "");
    let row_schema = make_action_row(&rust_i18n::t!("meta_schema_version"), "");
    let row_author = make_action_row(&rust_i18n::t!("meta_author"), "");
    let row_version = make_action_row(&rust_i18n::t!("meta_version"), "");
    let row_desc = make_action_row(&rust_i18n::t!("meta_description"), "");
    let row_license = make_action_row(&rust_i18n::t!("meta_license"), "");
    meta_group.add(&row_name);
    meta_group.add(&row_lang);
    meta_group.add(&row_type);
    meta_group.add(&row_schema);
    meta_group.add(&row_author);
    meta_group.add(&row_version);
    meta_group.add(&row_desc);
    meta_group.add(&row_license);
    outer.append(&meta_group);

    // 2) 키보드 + Upper/Lower 토글
    let kb_group = adw::PreferencesGroup::builder()
        .title(&*rust_i18n::t!("edit_layout"))
        .build();

    let toggle_box = gtk::Box::new(gtk::Orientation::Horizontal, 6);
    toggle_box.set_halign(gtk::Align::End);
    let toggle_upper = gtk::ToggleButton::with_label(&rust_i18n::t!("case_upper"));
    let toggle_lower = gtk::ToggleButton::with_label(&rust_i18n::t!("case_lower"));
    toggle_lower.set_active(true);
    toggle_upper.set_group(Some(&toggle_lower));
    toggle_box.append(&toggle_lower);
    toggle_box.append(&toggle_upper);
    outer.append(&toggle_box);

    let keyboard = Rc::new(KeyboardWidget::new());
    outer.append(keyboard.root());
    outer.append(&kb_group);

    // 3) 선택 키 정보 패널
    let info_group = adw::PreferencesGroup::builder()
        .title(&*rust_i18n::t!("key_panel_meta"))
        .build();
    let row_jamo = make_action_row(&rust_i18n::t!("key_panel_jamo"), &rust_i18n::t!("key_panel_empty"));
    let row_combos = make_action_row(&rust_i18n::t!("key_panel_combos"), "—");
    info_group.add(&row_jamo);
    info_group.add(&row_combos);
    outer.append(&info_group);

    scroller.set_child(Some(&outer));

    let info_jamo = row_jamo.clone();
    let info_combos = row_combos.clone();
    {
        let state = state.clone();
        let kb = keyboard.clone();
        keyboard.set_on_select(move |cell| {
            if let Some(label) = kb.cell_label(cell) {
                info_jamo.set_subtitle(&label);
                // 참여 조합 — combinations에 first/second/result 중 label과 동일한 게 있으면 표시
                if let Some(p) = state.current_profile.borrow().as_ref() {
                    if let Some(combos) = p.combinations.as_ref() {
                        let mut hits = Vec::new();
                        for t in combos
                            .cho
                            .iter()
                            .chain(combos.jung.iter())
                            .chain(combos.jong.iter())
                        {
                            if t.first == label || t.second == label || t.result == label {
                                hits.push(format!("{}+{}→{}", t.first, t.second, t.result));
                            }
                        }
                        if hits.is_empty() {
                            info_combos.set_subtitle("—");
                        } else {
                            info_combos.set_subtitle(&hits.join("  ·  "));
                        }
                    }
                }
            }
        });
    }

    // Upper/Lower 토글
    {
        let state = state.clone();
        let kb = keyboard.clone();
        toggle_upper.connect_toggled(move |b| {
            if !b.is_active() {
                return;
            }
            kb.set_upper(true);
            if let Some(p) = state.current_profile.borrow().as_ref() {
                kb.populate(p);
            }
        });
    }
    {
        let state = state.clone();
        let kb = keyboard.clone();
        toggle_lower.connect_toggled(move |b| {
            if !b.is_active() {
                return;
            }
            kb.set_upper(false);
            if let Some(p) = state.current_profile.borrow().as_ref() {
                kb.populate(p);
            }
        });
    }

    let row_name_c = row_name.clone();
    let row_lang_c = row_lang.clone();
    let row_type_c = row_type.clone();
    let row_schema_c = row_schema.clone();
    let row_author_c = row_author.clone();
    let row_version_c = row_version.clone();
    let row_desc_c = row_desc.clone();
    let row_license_c = row_license.clone();
    let kb_handle = keyboard.clone();
    let info_jamo_c = row_jamo.clone();
    let info_combos_c = row_combos.clone();
    let state_for_refresh = state.clone();

    let refresh: Rc<dyn Fn()> = Rc::new(move || {
        let cur = state_for_refresh.current_profile.borrow();
        if let Some(p) = cur.as_ref() {
            row_name_c.set_subtitle(&p.name);
            row_lang_c.set_subtitle(&p.language);
            row_type_c.set_subtitle(&p.layout_type);
            row_schema_c.set_subtitle(&p.schema_version.to_string());
            row_author_c.set_subtitle(p.metadata.author.as_deref().unwrap_or("—"));
            row_version_c.set_subtitle(p.metadata.version.as_deref().unwrap_or("—"));
            let desc = p
                .metadata
                .description
                .as_ref()
                .map(|t| t.resolve(unim_keymap_common::detect_locale()).to_string())
                .unwrap_or_else(|| "—".into());
            row_desc_c.set_subtitle(&desc);
            row_license_c.set_subtitle(p.metadata.license.as_deref().unwrap_or("—"));
            kb_handle.populate(p);
            info_jamo_c.set_subtitle(&rust_i18n::t!("key_panel_empty"));
            info_combos_c.set_subtitle("—");
        } else {
            row_name_c.set_subtitle(&rust_i18n::t!("no_profile_hint"));
        }
    });

    let _ = RefCell::new(()); // silence unused warnings in future expansions

    (scroller.upcast(), refresh)
}

fn make_action_row(title: &str, subtitle: &str) -> adw::ActionRow {
    adw::ActionRow::builder()
        .title(title)
        .subtitle(subtitle)
        .build()
}
