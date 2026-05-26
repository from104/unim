//! "다른 이름으로 저장" 다이얼로그 — 실시간 이름 충돌 검증.
//!
//! 흐름:
//! 1. EntryRow 에 새 식별자 입력 → `validate_new_name` 실시간 검증.
//! 2. 검증 결과에 따라 hint 라벨 메시지·색상 + "저장" 버튼 sensitivity 갱신.
//! 3. "저장" 클릭 → `EditorState::save_as` → 성공 시 toast + `on_saved(new_name)`.

use std::rc::Rc;

use gtk4::prelude::*;
use gtk4::{self as gtk};
use libadwaita::prelude::*;
use libadwaita::{self as adw};

use unim_keymap_common::layout_user_path;

use crate::helpers::name_validator::{conflict_message_key, validate_new_name, NameConflict};
use crate::state::SharedAppState;

/// Save As 다이얼로그를 띄운다.
///
/// - `prefill` : 초기 이름 (빌트인 편집이면 `{name}_copy` 권장).
/// - `on_saved`: 저장 성공 후 호출 (새 이름 전달) — registry rescan·드롭다운 갱신용.
pub fn open(
    parent: &impl IsA<gtk::Widget>,
    state: SharedAppState,
    prefill: &str,
    on_saved: Rc<dyn Fn(String)>,
) {
    let window = parent.root().and_then(|r| r.downcast::<gtk::Window>().ok());

    let dialog = adw::MessageDialog::new(
        window.as_ref(),
        Some(&rust_i18n::t!("save_as_title")),
        None,
    );
    dialog.add_response("cancel", &rust_i18n::t!("btn_cancel"));
    dialog.add_response("save", &rust_i18n::t!("btn_save"));
    dialog.set_response_appearance("save", adw::ResponseAppearance::Suggested);
    dialog.set_default_response(Some("save"));
    dialog.set_close_response("cancel");

    let entry = adw::EntryRow::builder()
        .title(rust_i18n::t!("save_as_name_label"))
        .text(prefill)
        .build();

    let hint = gtk::Label::builder()
        .halign(gtk::Align::Start)
        .wrap(true)
        .css_classes(["caption"])
        .margin_start(4)
        .margin_top(4)
        .build();

    let preview = gtk::Label::builder()
        .halign(gtk::Align::Start)
        .wrap(true)
        .css_classes(["caption", "dim-label"])
        .margin_start(4)
        .margin_top(2)
        .build();

    let list = gtk::ListBox::builder()
        .selection_mode(gtk::SelectionMode::None)
        .css_classes(["boxed-list"])
        .build();
    list.append(&entry);

    let body = gtk::Box::new(gtk::Orientation::Vertical, 8);
    body.append(&list);
    body.append(&hint);
    body.append(&preview);
    dialog.set_extra_child(Some(&body));

    // 실시간 검증 클로저.
    let validate = {
        let state = state.clone();
        let entry = entry.clone();
        let hint = hint.clone();
        let preview = preview.clone();
        let dialog = dialog.clone();
        Rc::new(move || {
            let name = entry.text().to_string();
            let trimmed = name.trim();
            let current = state.current_name.borrow().clone();
            let conflict =
                validate_new_name(trimmed, &state.registry.borrow(), current.as_deref());

            hint.set_label(&rust_i18n::t!(conflict_message_key(conflict)));
            hint.remove_css_class("error");
            hint.remove_css_class("success");
            match conflict {
                NameConflict::None => hint.add_css_class("success"),
                _ => hint.add_css_class("error"),
            }

            // 저장 경로 미리보기.
            match layout_user_path(trimmed) {
                Some(p) if conflict == NameConflict::None => {
                    preview.set_label(&rust_i18n::t!(
                        "save_as_path_preview",
                        path = p.to_string_lossy()
                    ));
                    preview.set_visible(true);
                }
                _ => preview.set_visible(false),
            }

            dialog.set_response_enabled("save", conflict == NameConflict::None);
        })
    };

    {
        let validate = validate.clone();
        entry.connect_changed(move |_| validate());
    }
    validate(); // 초기 1회.

    // 응답 처리.
    {
        let state = state.clone();
        let entry = entry.clone();
        dialog.connect_response(None, move |_, resp| {
            if resp != "save" {
                return;
            }
            let new_name = entry.text().to_string().trim().to_string();
            let result = state
                .editor
                .borrow_mut()
                .as_mut()
                .map(|ed| ed.save_as(new_name.clone()));
            match result {
                Some(Ok(path)) => {
                    state.toast(&rust_i18n::t!(
                        "toast_saved",
                        path = path.to_string_lossy()
                    ));
                    on_saved(new_name);
                }
                Some(Err(e)) => {
                    state.toast(&rust_i18n::t!("toast_save_failed", err = e.to_string()));
                }
                None => {}
            }
        });
    }

    dialog.present();
}
