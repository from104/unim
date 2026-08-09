//! 헤더 ☰ 메뉴 액션 — 새 자판 / 복제 / 내보내기 / 가져오기 / 폴더 열기 + 도움말.
//!
//! 모든 액션은 `win.*` 그룹에 등록된다. 메뉴 모델·단축키 결선은 app.rs.

use std::rc::Rc;

use gtk4::prelude::*;
use gtk4::gio;
use libadwaita as adw;

use unim_keymap_common::layout_user_path;

use crate::dialogs::{duplicate_profile, help, import_export, new_profile};
use crate::state::SharedAppState;

/// 헤더 메뉴 액션 6종 + 도움말을 `win.*` 그룹에 등록.
///
/// `select_profile`: import 후 사용자 폴더에서 자판을 재선택하는 콜백 (app.rs 제공).
pub fn register(
    state: SharedAppState,
    window: adw::ApplicationWindow,
    select_profile: Rc<dyn Fn(String)>,
) {
    let add = |name: &str, cb: Box<dyn Fn()>| {
        let action = gio::SimpleAction::new(name, None);
        action.connect_activate(move |_, _| cb());
        state.action_group.add_action(&action);
    };

    // 새 자판.
    {
        let state = state.clone();
        let window = window.clone();
        add(
            "new-profile",
            Box::new(move || new_profile::open(&window, state.clone())),
        );
    }
    // 현재 자판 복제.
    {
        let state = state.clone();
        let window = window.clone();
        add(
            "duplicate-profile",
            Box::new(move || duplicate_profile::open(&window, state.clone())),
        );
    }
    // JSON 내보내기.
    {
        let state = state.clone();
        let window = window.clone();
        add(
            "export-json",
            Box::new(move || import_export::export(&window, state.clone())),
        );
    }
    // JSON 가져오기.
    {
        let state = state.clone();
        let window = window.clone();
        let select_profile = select_profile.clone();
        add(
            "import-json",
            Box::new(move || {
                import_export::import(&window, state.clone(), select_profile.clone())
            }),
        );
    }
    // 사용자 자판 폴더 열기.
    {
        let state = state.clone();
        add(
            "open-user-folder",
            Box::new(move || {
                if let Some(dir) = layout_user_path("_").and_then(|p| p.parent().map(|d| d.to_path_buf()))
                {
                    let _ = std::fs::create_dir_all(&dir);
                    if std::process::Command::new("xdg-open").arg(&dir).spawn().is_err() {
                        state.toast(&rust_i18n::t!("toast_open_folder_failed"));
                    }
                }
            }),
        );
    }
    // 도움말.
    {
        let window = window.clone();
        add("help", Box::new(move || help::open(&window)));
    }
}

/// 헤더 ☰ 버튼에 붙일 메뉴 모델.
pub fn build_menu_model() -> gio::Menu {
    let menu = gio::Menu::new();

    let sec_profile = gio::Menu::new();
    append(&sec_profile, "menu_new_profile", "win.new-profile", "<Primary>n");
    append(
        &sec_profile,
        "menu_duplicate_profile",
        "win.duplicate-profile",
        "<Primary>d",
    );
    menu.append_section(None, &sec_profile);

    let sec_io = gio::Menu::new();
    append(&sec_io, "btn_revert", "win.revert", "");
    append(&sec_io, "menu_export_json", "win.export-json", "<Primary>e");
    append(&sec_io, "menu_import_json", "win.import-json", "<Primary>i");
    append(&sec_io, "menu_open_user_folder", "win.open-user-folder", "");
    menu.append_section(None, &sec_io);

    let sec_help = gio::Menu::new();
    append(&sec_help, "menu_help", "win.help", "F1");
    menu.append_section(None, &sec_help);

    menu
}

fn append(menu: &gio::Menu, label_key: &str, action: &str, accel: &str) {
    let item = gio::MenuItem::new(Some(&rust_i18n::t!(label_key)), Some(action));
    if !accel.is_empty() {
        item.set_attribute_value("accel", Some(&accel.to_variant()));
    }
    menu.append_item(&item);
}
