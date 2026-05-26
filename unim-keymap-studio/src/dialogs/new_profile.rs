//! 새 자판 만들기 (Ctrl+N) — 빈 템플릿.
//!
//! 같은 언어의 빌트인(en_qwerty / ko_2bulstd) 물리 배열을 빌려와 라벨만 비운다.
//! 이렇게 하면 키 그리드 크기가 보존되어 키 편집(set_key_label)이 곧바로 동작한다.

use gtk4::prelude::*;
use gtk4::{self as gtk};
use libadwaita::prelude::*;
use libadwaita::{self as adw};

use unim::keystroke::profile::{load_builtin_profile, LayoutProfile};

use crate::state::{EditorState, SharedAppState};

fn blank_layout(mut p: LayoutProfile, name: String, language: &str, layout_type: String) -> LayoutProfile {
    p.name = name;
    p.language = language.to_string();
    p.layout_type = layout_type;
    p.schema_version = 1;
    p.metadata = Default::default();
    p.inherits = None;
    p.combinations = None;
    p.rule_sets.clear();
    p.active_rule_sets = None;
    p.key_meta = None;
    p.moachigi = None;
    for rows in [&mut p.layout.lower, &mut p.layout.upper] {
        for v in [&mut rows.row1, &mut rows.row2, &mut rows.row3, &mut rows.row4] {
            for s in v.iter_mut() {
                s.clear();
            }
        }
    }
    p
}

pub fn open(parent: &impl IsA<gtk::Widget>, state: SharedAppState) {
    let window = parent.root().and_then(|r| r.downcast::<gtk::Window>().ok());
    let dialog = adw::MessageDialog::new(
        window.as_ref(),
        Some(&rust_i18n::t!("new_profile_title")),
        None,
    );
    dialog.add_response("cancel", &rust_i18n::t!("btn_cancel"));
    dialog.add_response("ok", &rust_i18n::t!("btn_ok"));
    dialog.set_response_appearance("ok", adw::ResponseAppearance::Suggested);
    dialog.set_default_response(Some("ok"));
    dialog.set_close_response("cancel");

    let name_entry = adw::EntryRow::builder()
        .title(rust_i18n::t!("new_profile_name"))
        .build();
    let lang_model = gtk::StringList::new(&[
        &rust_i18n::t!("dropdown_lang_korean"),
        &rust_i18n::t!("dropdown_lang_english"),
    ]);
    let lang_combo = adw::ComboRow::builder()
        .title(rust_i18n::t!("new_profile_language"))
        .model(&lang_model)
        .build();
    let type_entry = adw::EntryRow::builder()
        .title(rust_i18n::t!("basic_layout_type_label"))
        .build();

    let list = gtk::ListBox::builder()
        .selection_mode(gtk::SelectionMode::None)
        .css_classes(["boxed-list"])
        .build();
    list.append(&name_entry);
    list.append(&lang_combo);
    list.append(&type_entry);

    let body = gtk::Box::new(gtk::Orientation::Vertical, 8);
    body.append(&list);
    dialog.set_extra_child(Some(&body));

    {
        let state = state.clone();
        dialog.connect_response(None, move |_, resp| {
            if resp != "ok" {
                return;
            }
            let name = name_entry.text().to_string();
            if name.is_empty() {
                state.toast(&rust_i18n::t!("rule_set_name_required"));
                return;
            }
            let is_korean = lang_combo.selected() == 0;
            let language = if is_korean { "korean" } else { "english" };
            let base = if is_korean { "ko_2bulstd" } else { "en_qwerty" };
            let layout_type = {
                let t = type_entry.text().to_string();
                if t.is_empty() {
                    if is_korean { "2bul".to_string() } else { "qwerty".to_string() }
                } else {
                    t
                }
            };
            let Ok(base_profile) = load_builtin_profile(base) else {
                return;
            };
            let profile = blank_layout(base_profile, name.clone(), language, layout_type);

            let mut ed = EditorState::new(profile);
            ed.dirty = true; // 새 자판 — 미저장.
            *state.editor.borrow_mut() = Some(ed);
            *state.current_name.borrow_mut() = Some(name);
            state.is_builtin.set(false);
            state.run_ui_refresh();
        });
    }

    dialog.present();
}
