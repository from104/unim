//! 도움말 다이얼로그 (F1) — 사용법 + 단축키.

use gtk4::prelude::*;
use gtk4::{self as gtk};
use libadwaita::prelude::*;
use libadwaita::{self as adw};

const SHORTCUTS: [(&str, &str); 8] = [
    ("F1", "help_shortcut_help"),
    ("Ctrl + N", "help_shortcut_new"),
    ("Ctrl + D", "help_shortcut_duplicate"),
    ("Ctrl + S", "help_shortcut_save"),
    ("Ctrl + Shift + S", "help_shortcut_save_as"),
    ("Ctrl + E", "help_shortcut_export"),
    ("Ctrl + I", "help_shortcut_import"),
    ("Ctrl + 1 / 2 / 3 / 4", "help_shortcut_tabs"),
];

pub fn open(parent: &impl IsA<gtk::Widget>) {
    let window = parent.root().and_then(|r| r.downcast::<gtk::Window>().ok());
    let dialog = adw::MessageDialog::new(
        window.as_ref(),
        Some(&rust_i18n::t!("help_dialog_title")),
        None,
    );
    dialog.add_response("close", &rust_i18n::t!("btn_ok"));
    dialog.set_default_response(Some("close"));
    dialog.set_close_response("close");

    let body = gtk::Box::new(gtk::Orientation::Vertical, 12);

    let intro = gtk::Label::builder()
        .label(rust_i18n::t!("help_dialog_intro"))
        .halign(gtk::Align::Start)
        .wrap(true)
        .build();
    body.append(&intro);

    let grid = gtk::Grid::builder()
        .row_spacing(6)
        .column_spacing(18)
        .build();
    for (i, (keys, desc_key)) in SHORTCUTS.iter().enumerate() {
        let key_label = gtk::Label::builder()
            .label(*keys)
            .halign(gtk::Align::Start)
            .css_classes(["monospace", "dim-label"])
            .build();
        let desc_label = gtk::Label::builder()
            .label(rust_i18n::t!(*desc_key))
            .halign(gtk::Align::Start)
            .build();
        grid.attach(&key_label, 0, i as i32, 1, 1);
        grid.attach(&desc_label, 1, i as i32, 1, 1);
    }
    body.append(&grid);

    dialog.set_extra_child(Some(&body));
    dialog.present();
}
