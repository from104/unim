//! 현재 자판 복제 (Ctrl+D) — 현재 버퍼를 새 이름으로 사용자 자판 사본 생성.
//!
//! 사본은 미저장 상태로 editor 에 로드된다. 'Save'(Ctrl+S)로 사용자 폴더에 보관.

use gtk4::prelude::*;
use gtk4::{self as gtk};
use libadwaita::prelude::*;
use libadwaita::{self as adw};

use crate::state::{EditorState, SharedAppState};

pub fn open(parent: &impl IsA<gtk::Widget>, state: SharedAppState) {
    // 복제 원본 — 현재 editor 버퍼.
    let base = match state.editor.borrow().as_ref() {
        Some(ed) => ed.buf.clone(),
        None => return,
    };
    let prefill = format!("{}_copy", base.name);

    let window = parent.root().and_then(|r| r.downcast::<gtk::Window>().ok());
    let dialog = adw::MessageDialog::new(
        window.as_ref(),
        Some(&rust_i18n::t!("duplicate_profile_title")),
        None,
    );
    dialog.add_response("cancel", &rust_i18n::t!("btn_cancel"));
    dialog.add_response("ok", &rust_i18n::t!("btn_ok"));
    dialog.set_response_appearance("ok", adw::ResponseAppearance::Suggested);
    dialog.set_default_response(Some("ok"));
    dialog.set_close_response("cancel");

    let name_entry = adw::EntryRow::builder()
        .title(rust_i18n::t!("duplicate_profile_name"))
        .text(&prefill)
        .build();
    let list = gtk::ListBox::builder()
        .selection_mode(gtk::SelectionMode::None)
        .css_classes(["boxed-list"])
        .build();
    list.append(&name_entry);
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
            let mut copy = base.clone();
            copy.name = name.clone();
            let mut ed = EditorState::new(copy);
            ed.dirty = true;
            *state.editor.borrow_mut() = Some(ed);
            *state.current_name.borrow_mut() = Some(name);
            state.is_builtin.set(false);
            state.run_ui_refresh();
        });
    }

    dialog.present();
}
