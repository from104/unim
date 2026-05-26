//! 키 셀 편집 다이얼로그 — 위쪽(Shift)/아래쪽(평문) 각 한 글자.
//!
//! 한글 자판이면 초/중/종성 빠른 선택 드롭다운으로 자모를 채울 수 있다.
//! 검증: 각 입력은 0자(비움) 또는 정확히 1자.

use std::rc::Rc;

use gtk4::prelude::*;
use gtk4::{self as gtk};
use libadwaita::prelude::*;
use libadwaita::{self as adw};

use unim_keymap_common::keyboard_widget::cell_label_at;

use crate::helpers::jamo_catalog::{self, Scope};
use crate::state::SharedAppState;

/// 키 편집 다이얼로그를 연다.
pub fn open(
    parent: &impl IsA<gtk::Widget>,
    state: SharedAppState,
    row: u8,
    col: u8,
    is_korean: bool,
    on_done: Rc<dyn Fn()>,
) {
    // 현재 라벨 읽기.
    let (cur_lower, cur_upper) = {
        let editor = state.editor.borrow();
        match editor.as_ref() {
            Some(ed) => (
                cell_label_at(&ed.buf.layout.lower, row, col).to_string(),
                cell_label_at(&ed.buf.layout.upper, row, col).to_string(),
            ),
            None => return,
        }
    };

    let window = parent.root().and_then(|r| r.downcast::<gtk::Window>().ok());
    let dialog = adw::MessageDialog::new(
        window.as_ref(),
        Some(&rust_i18n::t!("key_edit_title", row = (row + 1), col = (col + 1))),
        None,
    );
    dialog.add_response("cancel", &rust_i18n::t!("btn_cancel"));
    dialog.add_response("apply", &rust_i18n::t!("btn_apply"));
    dialog.set_response_appearance("apply", adw::ResponseAppearance::Suggested);
    dialog.set_default_response(Some("apply"));
    dialog.set_close_response("cancel");

    let upper_entry = adw::EntryRow::builder()
        .title(rust_i18n::t!("key_edit_upper_label"))
        .text(&cur_upper)
        .build();
    let lower_entry = adw::EntryRow::builder()
        .title(rust_i18n::t!("key_edit_lower_label"))
        .text(&cur_lower)
        .build();

    let list = gtk::ListBox::builder()
        .selection_mode(gtk::SelectionMode::None)
        .css_classes(["boxed-list"])
        .build();
    list.append(&upper_entry);
    list.append(&lower_entry);

    let body = gtk::Box::new(gtk::Orientation::Vertical, 12);
    body.append(&list);

    // 한글 자모 빠른 선택 (한글 자판 전용).
    if is_korean {
        let helper_label = gtk::Label::builder()
            .label(rust_i18n::t!("key_edit_jamo_helper"))
            .halign(gtk::Align::Start)
            .css_classes(["heading"])
            .build();
        body.append(&helper_label);

        let scope_model = gtk::StringList::new(&[
            &rust_i18n::t!("key_edit_scope_cho"),
            &rust_i18n::t!("key_edit_scope_jung"),
            &rust_i18n::t!("key_edit_scope_jong"),
        ]);
        let scope_combo = adw::ComboRow::builder()
            .title(rust_i18n::t!("key_edit_scope_label"))
            .model(&scope_model)
            .build();

        let jamo_dd = gtk::DropDown::new(
            Some(jamo_catalog::combo_string_list(Scope::Cho)),
            None::<gtk::Expression>,
        );
        jamo_dd.set_valign(gtk::Align::Center);

        let scope_list = gtk::ListBox::builder()
            .selection_mode(gtk::SelectionMode::None)
            .css_classes(["boxed-list"])
            .build();
        let jamo_row = adw::ActionRow::builder()
            .title(rust_i18n::t!("key_edit_jamo_label"))
            .build();
        jamo_row.add_suffix(&jamo_dd);
        scope_list.append(&scope_combo);
        scope_list.append(&jamo_row);
        body.append(&scope_list);

        // 스코프 변경 → 자모 드롭다운 모델 교체.
        {
            let jamo_dd = jamo_dd.clone();
            scope_combo.connect_selected_notify(move |c| {
                let scope = scope_from_index(c.selected());
                jamo_dd.set_model(Some(&jamo_catalog::combo_string_list(scope)));
            });
        }

        // 채우기 버튼.
        let btn_box = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        btn_box.set_halign(gtk::Align::End);
        let btn_fill_upper = gtk::Button::with_label(&rust_i18n::t!("key_edit_fill_upper"));
        let btn_fill_lower = gtk::Button::with_label(&rust_i18n::t!("key_edit_fill_lower"));
        btn_box.append(&btn_fill_upper);
        btn_box.append(&btn_fill_lower);
        body.append(&btn_box);

        let pick_jamo = {
            let scope_combo = scope_combo.clone();
            let jamo_dd = jamo_dd.clone();
            Rc::new(move || -> Option<char> {
                let scope = scope_from_index(scope_combo.selected());
                jamo_catalog::entry_at(scope, jamo_dd.selected())
                    .map(|e| jamo_catalog::combo_char(scope, &e))
            })
        };
        {
            let pick = pick_jamo.clone();
            let upper_entry = upper_entry.clone();
            btn_fill_upper.connect_clicked(move |_| {
                if let Some(ch) = pick() {
                    upper_entry.set_text(&ch.to_string());
                }
            });
        }
        {
            let pick = pick_jamo.clone();
            let lower_entry = lower_entry.clone();
            btn_fill_lower.connect_clicked(move |_| {
                if let Some(ch) = pick() {
                    lower_entry.set_text(&ch.to_string());
                }
            });
        }
    }

    dialog.set_extra_child(Some(&body));

    // 응답 처리.
    {
        let state = state.clone();
        let upper_entry = upper_entry.clone();
        let lower_entry = lower_entry.clone();
        dialog.connect_response(None, move |_, resp| {
            if resp != "apply" {
                return;
            }
            let upper = upper_entry.text().to_string();
            let lower = lower_entry.text().to_string();
            // 검증 — 각 0자 또는 1자.
            if upper.chars().count() > 1 || lower.chars().count() > 1 {
                state.toast(&rust_i18n::t!("key_edit_validation_one_char"));
                return;
            }
            if let Some(ed) = state.editor.borrow_mut().as_mut() {
                ed.set_key_label(false, row, col, lower);
                ed.set_key_label(true, row, col, upper);
            }
            on_done();
        });
    }

    dialog.present();
}

fn scope_from_index(i: u32) -> Scope {
    match i {
        0 => Scope::Cho,
        1 => Scope::Jung,
        _ => Scope::Jong,
    }
}
