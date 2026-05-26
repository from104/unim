//! JSON 내보내기(Ctrl+E) / 가져오기(Ctrl+I) — gtk::FileDialog 사용.

use std::rc::Rc;

use gtk4::prelude::*;
use gtk4::{self as gtk, gio};

use unim::keystroke::profile::parse_profile_str;
use unim_keymap_common::{save_profile_json, serializable::to_pretty_json};

use crate::state::SharedAppState;

fn parent_window(parent: &impl IsA<gtk::Widget>) -> Option<gtk::Window> {
    parent.root().and_then(|r| r.downcast::<gtk::Window>().ok())
}

/// 현재 자판을 임의 경로 .json 으로 내보낸다.
pub fn export(parent: &impl IsA<gtk::Widget>, state: SharedAppState) {
    let buf = match state.editor.borrow().as_ref() {
        Some(ed) => ed.buf.clone(),
        None => return,
    };
    let json = match to_pretty_json(&buf) {
        Ok(j) => j,
        Err(e) => {
            state.toast(&rust_i18n::t!("toast_save_failed", err = e.to_string()));
            return;
        }
    };

    let dialog = gtk::FileDialog::builder()
        .title(rust_i18n::t!("menu_export_json"))
        .initial_name(format!("{}.json", buf.name))
        .build();
    let window = parent_window(parent);
    let state = state.clone();
    dialog.save(window.as_ref(), gio::Cancellable::NONE, move |res| {
        if let Ok(file) = res {
            if let Some(path) = file.path() {
                match std::fs::write(&path, &json) {
                    Ok(_) => state.toast(&rust_i18n::t!(
                        "toast_exported",
                        path = path.to_string_lossy()
                    )),
                    Err(e) => {
                        state.toast(&rust_i18n::t!("toast_save_failed", err = e.to_string()))
                    }
                }
            }
        }
    });
}

/// 외부 .json 을 사용자 폴더로 가져온다. 성공 시 `on_imported(name)`.
pub fn import(
    parent: &impl IsA<gtk::Widget>,
    state: SharedAppState,
    on_imported: Rc<dyn Fn(String)>,
) {
    let filter = gtk::FileFilter::new();
    filter.add_suffix("json");
    filter.set_name(Some("JSON"));
    let filters = gio::ListStore::new::<gtk::FileFilter>();
    filters.append(&filter);

    let dialog = gtk::FileDialog::builder()
        .title(rust_i18n::t!("menu_import_json"))
        .filters(&filters)
        .build();
    let window = parent_window(parent);
    let state = state.clone();
    dialog.open(window.as_ref(), gio::Cancellable::NONE, move |res| {
        let Ok(file) = res else {
            return;
        };
        let Some(path) = file.path() else {
            return;
        };
        let txt = match std::fs::read_to_string(&path) {
            Ok(t) => t,
            Err(e) => {
                state.toast(&rust_i18n::t!("toast_save_failed", err = e.to_string()));
                return;
            }
        };
        let profile = match parse_profile_str(&txt) {
            Ok(p) => p,
            Err(e) => {
                state.toast(&rust_i18n::t!("toast_import_invalid", err = format!("{e:?}")));
                return;
            }
        };
        match save_profile_json(&profile) {
            Ok(_) => {
                state.toast(&rust_i18n::t!("toast_imported", name = profile.name.clone()));
                on_imported(profile.name.clone());
            }
            Err(e) => state.toast(&rust_i18n::t!("toast_save_failed", err = e.to_string())),
        }
    });
}
