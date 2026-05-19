//! 편집 탭 — 메타·키 라벨·조합·rule_sets·moachigi 마커 편집 + 저장.

use std::cell::RefCell;
use std::rc::Rc;

use gtk4::prelude::*;
use gtk4::{self as gtk};
use libadwaita::prelude::*;
use libadwaita::{self as adw};

use unim::keystroke::profile::RawTriple;
use unim_keymap_common::{KeyCell, KeyboardWidget};

use crate::app::SharedAppState;
use crate::editor_state::ComboKind;

pub fn build(state: SharedAppState, toast: adw::ToastOverlay) -> (gtk::Widget, Rc<dyn Fn()>) {
    let scroller = gtk::ScrolledWindow::builder()
        .hscrollbar_policy(gtk::PolicyType::Never)
        .build();
    let outer = gtk::Box::new(gtk::Orientation::Vertical, 12);
    outer.set_margin_top(16);
    outer.set_margin_bottom(16);
    outer.set_margin_start(16);
    outer.set_margin_end(16);

    // === 메타데이터 그룹 ===
    let meta_group = adw::PreferencesGroup::builder()
        .title(&*rust_i18n::t!("edit_metadata"))
        .build();
    let row_name = adw::EntryRow::builder()
        .title(&*rust_i18n::t!("dialog_field_name"))
        .build();
    let row_author = adw::EntryRow::builder()
        .title(&*rust_i18n::t!("meta_author"))
        .build();
    let row_version = adw::EntryRow::builder()
        .title(&*rust_i18n::t!("meta_version"))
        .build();
    let row_desc = adw::EntryRow::builder()
        .title(&*rust_i18n::t!("meta_description"))
        .build();
    meta_group.add(&row_name);
    meta_group.add(&row_author);
    meta_group.add(&row_version);
    meta_group.add(&row_desc);
    outer.append(&meta_group);

    // === 키 배열 그룹 ===
    let layout_group = adw::PreferencesGroup::builder()
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
    outer.append(&layout_group);

    // === 조합 규칙 그룹 ===
    let combos_group = adw::PreferencesGroup::builder()
        .title(&*rust_i18n::t!("edit_combinations"))
        .build();
    let cho_expander = adw::ExpanderRow::builder()
        .title(&*rust_i18n::t!("combos_cho"))
        .build();
    let jung_expander = adw::ExpanderRow::builder()
        .title(&*rust_i18n::t!("combos_jung"))
        .build();
    let jong_expander = adw::ExpanderRow::builder()
        .title(&*rust_i18n::t!("combos_jong"))
        .build();
    combos_group.add(&cho_expander);
    combos_group.add(&jung_expander);
    combos_group.add(&jong_expander);
    outer.append(&combos_group);

    // === 규칙 세트 그룹 ===
    let rules_group = adw::PreferencesGroup::builder()
        .title(&*rust_i18n::t!("rule_sets_title"))
        .build();
    outer.append(&rules_group);

    // === 모아치기 그룹 ===
    let moachigi_group = adw::PreferencesGroup::builder()
        .title(&*rust_i18n::t!("moachigi_group"))
        .build();
    let moachigi_switch = adw::SwitchRow::builder()
        .title(&*rust_i18n::t!("moachigi_capability"))
        .subtitle(&*rust_i18n::t!("moachigi_capability_hint"))
        .build();
    moachigi_group.add(&moachigi_switch);
    outer.append(&moachigi_group);

    // === 액션 버튼 그룹 ===
    let action_box = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    action_box.set_halign(gtk::Align::End);
    action_box.set_margin_top(8);
    let btn_revert = gtk::Button::with_label(&rust_i18n::t!("btn_revert"));
    let btn_save_as = gtk::Button::with_label(&rust_i18n::t!("btn_save_as"));
    let btn_save = gtk::Button::with_label(&rust_i18n::t!("btn_save"));
    btn_save.add_css_class("suggested-action");
    action_box.append(&btn_revert);
    action_box.append(&btn_save_as);
    action_box.append(&btn_save);
    outer.append(&action_box);

    scroller.set_child(Some(&outer));

    // === 상태 동기화: 입력 ↔ EditorState ===
    let updating = Rc::new(RefCell::new(false));
    {
        let state = state.clone();
        let updating = updating.clone();
        row_name.connect_changed(move |r| {
            if *updating.borrow() {
                return;
            }
            if let Some(ed) = state.editor.borrow_mut().as_mut() {
                ed.rename(r.text().to_string());
            }
        });
    }
    {
        let state = state.clone();
        let updating = updating.clone();
        row_author.connect_changed(move |r| {
            if *updating.borrow() {
                return;
            }
            if let Some(ed) = state.editor.borrow_mut().as_mut() {
                let t = r.text();
                ed.set_metadata_author(if t.is_empty() { None } else { Some(t.to_string()) });
            }
        });
    }
    {
        let state = state.clone();
        let updating = updating.clone();
        row_version.connect_changed(move |r| {
            if *updating.borrow() {
                return;
            }
            if let Some(ed) = state.editor.borrow_mut().as_mut() {
                let t = r.text();
                ed.set_metadata_version(if t.is_empty() { None } else { Some(t.to_string()) });
            }
        });
    }
    {
        let state = state.clone();
        let updating = updating.clone();
        row_desc.connect_changed(move |r| {
            if *updating.borrow() {
                return;
            }
            if let Some(ed) = state.editor.borrow_mut().as_mut() {
                let t = r.text();
                ed.set_metadata_description(if t.is_empty() {
                    None
                } else {
                    Some(t.to_string())
                });
            }
        });
    }
    {
        let state = state.clone();
        let updating = updating.clone();
        moachigi_switch.connect_active_notify(move |s| {
            if *updating.borrow() {
                return;
            }
            if let Some(ed) = state.editor.borrow_mut().as_mut() {
                ed.set_supports_moachigi(s.is_active());
            }
        });
    }

    // === 키 셀 클릭 → 라벨 편집 다이얼로그 ===
    {
        let state = state.clone();
        let kb = keyboard.clone();
        let toast = toast.clone();
        keyboard.set_on_select(move |cell: KeyCell| {
            let upper = kb.is_upper();
            let current_label = kb.cell_label(cell).unwrap_or_default();
            let parent = active_window();
            let dialog = adw::MessageDialog::builder()
                .heading(&*rust_i18n::t!("dialog_edit_key_title"))
                .body(&format!(
                    "{} ({}, row {} col {})",
                    current_label,
                    if upper { "upper" } else { "lower" },
                    cell.row,
                    cell.col
                ))
                .modal(true)
                .transient_for(parent.as_ref().unwrap_or(&gtk::Window::new()))
                .build();
            let entry = gtk::Entry::builder().text(&current_label).build();
            dialog.set_extra_child(Some(&entry));
            dialog.add_response("cancel", "Cancel");
            dialog.add_response("ok", "OK");
            dialog.set_default_response(Some("ok"));

            let state_cb = state.clone();
            let kb_cb = kb.clone();
            let toast_cb = toast.clone();
            dialog.connect_response(None, move |dlg: &adw::MessageDialog, resp: &str| {
                if resp == "ok" {
                    let new_text = entry.text().to_string();
                    if let Some(ed) = state_cb.editor.borrow_mut().as_mut() {
                        if ed.set_key_label(upper, cell.row, cell.col, new_text) {
                            kb_cb.populate(&ed.buf);
                        } else {
                            toast_cb.add_toast(adw::Toast::new("invalid cell"));
                        }
                    }
                }
                dlg.close();
            });
            dialog.present();
        });
    }

    // === Upper/Lower 토글 ===
    {
        let state = state.clone();
        let kb = keyboard.clone();
        toggle_upper.connect_toggled(move |b| {
            if !b.is_active() {
                return;
            }
            kb.set_upper(true);
            if let Some(ed) = state.editor.borrow().as_ref() {
                kb.populate(&ed.buf);
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
            if let Some(ed) = state.editor.borrow().as_ref() {
                kb.populate(&ed.buf);
            }
        });
    }

    // === 저장 / 다른 이름으로 / 원본 복원 ===
    {
        let state = state.clone();
        let toast = toast.clone();
        let row_name_c = row_name.clone();
        let updating = updating.clone();
        let kb = keyboard.clone();
        let cho_expander_c = cho_expander.clone();
        let jung_expander_c = jung_expander.clone();
        let jong_expander_c = jong_expander.clone();
        let row_author_c = row_author.clone();
        let row_version_c = row_version.clone();
        let row_desc_c = row_desc.clone();
        let moachigi_c = moachigi_switch.clone();
        btn_revert.connect_clicked(move |_| {
            *updating.borrow_mut() = true;
            if let Some(ed) = state.editor.borrow_mut().as_mut() {
                ed.revert();
                row_name_c.set_text(&ed.buf.name);
                row_author_c.set_text(ed.buf.metadata.author.as_deref().unwrap_or(""));
                row_version_c.set_text(ed.buf.metadata.version.as_deref().unwrap_or(""));
                let d = ed
                    .buf
                    .metadata
                    .description
                    .as_ref()
                    .map(|t| t.resolve(unim_keymap_common::detect_locale()).to_string())
                    .unwrap_or_default();
                row_desc_c.set_text(&d);
                moachigi_c.set_active(ed.buf.moachigi.is_some());
                kb.populate(&ed.buf);
                refresh_combo_section(&cho_expander_c, ed, ComboKind::Cho, state.clone());
                refresh_combo_section(&jung_expander_c, ed, ComboKind::Jung, state.clone());
                refresh_combo_section(&jong_expander_c, ed, ComboKind::Jong, state.clone());
            }
            *updating.borrow_mut() = false;
            toast.add_toast(adw::Toast::new(&rust_i18n::t!("toast_revert")));
        });
    }
    {
        let state = state.clone();
        let toast = toast.clone();
        btn_save.connect_clicked(move |_| {
            let result = state
                .editor
                .borrow_mut()
                .as_mut()
                .map(|ed| ed.save());
            match result {
                Some(Ok(path)) => {
                    toast.add_toast(adw::Toast::new(
                        &rust_i18n::t!("toast_saved", path = path.display().to_string()),
                    ));
                    state.registry.borrow_mut().scan();
                }
                Some(Err(e)) => {
                    toast.add_toast(adw::Toast::new(
                        &rust_i18n::t!("toast_save_failed", err = e.to_string()),
                    ));
                }
                None => {}
            }
        });
    }
    {
        let state = state.clone();
        let toast = toast.clone();
        let row_name_c = row_name.clone();
        btn_save_as.connect_clicked(move |_| {
            let parent = active_window();
            let dialog = adw::MessageDialog::builder()
                .heading(&*rust_i18n::t!("dialog_save_as_title"))
                .modal(true)
                .transient_for(parent.as_ref().unwrap_or(&gtk::Window::new()))
                .build();
            let entry = gtk::Entry::builder()
                .text(
                    state
                        .editor
                        .borrow()
                        .as_ref()
                        .map(|e| e.buf.name.clone())
                        .unwrap_or_default(),
                )
                .build();
            dialog.set_extra_child(Some(&entry));
            dialog.add_response("cancel", "Cancel");
            dialog.add_response("ok", "OK");
            dialog.set_default_response(Some("ok"));

            let state_cb = state.clone();
            let toast_cb = toast.clone();
            let row_name_cb = row_name_c.clone();
            dialog.connect_response(None, move |dlg: &adw::MessageDialog, resp: &str| {
                if resp == "ok" {
                    let new_name = entry.text().to_string();
                    if !new_name.is_empty() {
                        if let Some(ed) = state_cb.editor.borrow_mut().as_mut() {
                            ed.rename(new_name.clone());
                            row_name_cb.set_text(&new_name);
                            match ed.save() {
                                Ok(path) => toast_cb.add_toast(adw::Toast::new(
                                    &rust_i18n::t!(
                                        "toast_saved",
                                        path = path.display().to_string()
                                    ),
                                )),
                                Err(e) => toast_cb.add_toast(adw::Toast::new(
                                    &rust_i18n::t!(
                                        "toast_save_failed",
                                        err = e.to_string()
                                    ),
                                )),
                            }
                            state_cb.registry.borrow_mut().scan();
                        }
                    }
                }
                dlg.close();
            });
            dialog.present();
        });
    }

    // === Refresh callback (사이드바 선택 변경 시) ===
    let row_name_c = row_name.clone();
    let row_author_c = row_author.clone();
    let row_version_c = row_version.clone();
    let row_desc_c = row_desc.clone();
    let moachigi_c = moachigi_switch.clone();
    let kb_handle = keyboard.clone();
    let cho_e = cho_expander.clone();
    let jung_e = jung_expander.clone();
    let jong_e = jong_expander.clone();
    let rules_group_c = rules_group.clone();
    let state_for_refresh = state.clone();
    let updating_c = updating.clone();

    let refresh: Rc<dyn Fn()> = Rc::new(move || {
        *updating_c.borrow_mut() = true;
        let editor_borrow = state_for_refresh.editor.borrow();
        if let Some(ed) = editor_borrow.as_ref() {
            row_name_c.set_text(&ed.buf.name);
            row_author_c.set_text(ed.buf.metadata.author.as_deref().unwrap_or(""));
            row_version_c.set_text(ed.buf.metadata.version.as_deref().unwrap_or(""));
            let d = ed
                .buf
                .metadata
                .description
                .as_ref()
                .map(|t| t.resolve(unim_keymap_common::detect_locale()).to_string())
                .unwrap_or_default();
            row_desc_c.set_text(&d);
            moachigi_c.set_active(ed.buf.moachigi.is_some());
            kb_handle.populate(&ed.buf);

            // 조합 섹션 갱신
            refresh_combo_section(&cho_e, ed, ComboKind::Cho, state_for_refresh.clone());
            refresh_combo_section(&jung_e, ed, ComboKind::Jung, state_for_refresh.clone());
            refresh_combo_section(&jong_e, ed, ComboKind::Jong, state_for_refresh.clone());

            // rule_sets 섹션 갱신
            rebuild_rule_sets(&rules_group_c, ed, state_for_refresh.clone());
        }
        *updating_c.borrow_mut() = false;
    });

    (scroller.upcast(), refresh)
}

fn refresh_combo_section(
    expander: &adw::ExpanderRow,
    ed: &crate::editor_state::EditorState,
    kind: ComboKind,
    _state: SharedAppState,
) {
    // 기존 자식 제거
    while let Some(c) = expander.first_child() {
        // ExpanderRow의 자식은 내부 위젯과 추가된 row들이 섞여 있어 안전한 API가 제한적.
        // adw::ExpanderRow는 `remove`를 제공.
        if let Ok(row) = c.clone().downcast::<adw::ActionRow>() {
            expander.remove(&row);
        } else {
            break; // 첫 non-ActionRow에 도달하면 내부 위젯이므로 중단.
        }
    }
    let combos = ed.combos(kind);
    if combos.is_empty() {
        let empty_row = adw::ActionRow::builder()
            .title(&*rust_i18n::t!("combo_section_empty"))
            .build();
        expander.add_row(&empty_row);
    } else {
        for (i, t) in combos.iter().enumerate() {
            let row = adw::ActionRow::builder()
                .title(&format!("{} + {} → {}", t.first, t.second, t.result))
                .subtitle(&format!("#{i}"))
                .build();
            expander.add_row(&row);
        }
    }
    // TODO: + 추가/삭제 버튼은 후속 phase.
    let _ = RawTriple {
        first: String::new(),
        second: String::new(),
        result: String::new(),
    };
}

fn rebuild_rule_sets(
    _group: &adw::PreferencesGroup,
    _ed: &crate::editor_state::EditorState,
    _state: SharedAppState,
) {
    // TODO: rule_sets SwitchRow 그룹 동적 구성. 현재 단계에서는 메타·키·조합·moachigi만.
}

fn active_window() -> Option<gtk::Window> {
    let app = gtk::gio::Application::default()?
        .downcast::<gtk::Application>()
        .ok()?;
    app.active_window()
}
