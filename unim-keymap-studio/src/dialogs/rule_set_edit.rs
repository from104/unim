//! rule_set 생성/편집 다이얼로그 (Phase E) — 이름 · 설명(ko/en) · 활성.
//!
//! 조합(combinations) 편집은 확장 탭의 rule_set Expander 안에서 별도로 한다.
//! 편집 모드에서는 이름을 고정(rename 미지원 — 삭제 후 재생성으로 대체).

use std::rc::Rc;

use gtk4::prelude::*;
use gtk4::{self as gtk};
use libadwaita::prelude::*;
use libadwaita::{self as adw};

use crate::state::editor_state::localized_lang;
use crate::state::SharedAppState;

/// rule_set 다이얼로그. `edit_name` Some 이면 편집(이름 고정), None 이면 생성.
pub fn open(
    parent: &impl IsA<gtk::Widget>,
    state: SharedAppState,
    edit_name: Option<String>,
    on_done: Rc<dyn Fn()>,
) {
    // 기존 값 읽기.
    let (init_ko, init_en, init_active) = {
        let editor = state.editor.borrow();
        match (editor.as_ref(), edit_name.as_ref()) {
            (Some(ed), Some(name)) => match ed.rule_sets().get(name) {
                Some(rs) => (
                    localized_lang(rs.description.as_ref(), "ko"),
                    localized_lang(rs.description.as_ref(), "en"),
                    rs.active,
                ),
                None => (String::new(), String::new(), false),
            },
            _ => (String::new(), String::new(), false),
        }
    };

    let window = parent.root().and_then(|r| r.downcast::<gtk::Window>().ok());
    let heading = if edit_name.is_some() {
        rust_i18n::t!("rule_set_edit_title")
    } else {
        rust_i18n::t!("rule_set_add_title")
    };
    let dialog = adw::MessageDialog::new(window.as_ref(), Some(&heading), None);
    dialog.add_response("cancel", &rust_i18n::t!("btn_cancel"));
    dialog.add_response("ok", &rust_i18n::t!("btn_ok"));
    dialog.set_response_appearance("ok", adw::ResponseAppearance::Suggested);
    dialog.set_default_response(Some("ok"));
    dialog.set_close_response("cancel");

    let name_entry = adw::EntryRow::builder()
        .title(rust_i18n::t!("rule_set_name_label"))
        .text(edit_name.as_deref().unwrap_or(""))
        .build();
    name_entry.set_sensitive(edit_name.is_none());

    let desc_ko = adw::EntryRow::builder()
        .title(rust_i18n::t!("rule_set_desc_ko"))
        .text(&init_ko)
        .build();
    let desc_en = adw::EntryRow::builder()
        .title(rust_i18n::t!("rule_set_desc_en"))
        .text(&init_en)
        .build();
    let active_switch = adw::SwitchRow::builder()
        .title(rust_i18n::t!("rule_set_active"))
        .active(init_active)
        .build();

    let list = gtk::ListBox::builder()
        .selection_mode(gtk::SelectionMode::None)
        .css_classes(["boxed-list"])
        .build();
    list.append(&name_entry);
    list.append(&desc_ko);
    list.append(&desc_en);
    list.append(&active_switch);

    let body = gtk::Box::new(gtk::Orientation::Vertical, 8);
    body.append(&list);
    dialog.set_extra_child(Some(&body));

    {
        let state = state.clone();
        let edit_name = edit_name.clone();
        dialog.connect_response(None, move |_, resp| {
            if resp != "ok" {
                return;
            }
            let name = match &edit_name {
                Some(n) => n.clone(),
                None => name_entry.text().to_string(),
            };
            if name.is_empty() {
                state.toast(&rust_i18n::t!("rule_set_name_required"));
                return;
            }
            if let Some(ed) = state.editor.borrow_mut().as_mut() {
                if edit_name.is_none() && !ed.add_rule_set(name.clone()) {
                    state.toast(&rust_i18n::t!("rule_set_name_exists"));
                    return;
                }
                ed.set_rule_set_description(
                    &name,
                    desc_ko.text().to_string(),
                    desc_en.text().to_string(),
                );
                ed.toggle_rule_set(&name, active_switch.is_active());
            }
            on_done();
        });
    }

    dialog.present();
}
