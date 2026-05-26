//! 확장 탭 — 한글 자판 전용. rule_sets(규칙 세트) + 전역 key_meta.
//!
//! - 규칙 세트: ExpanderRow 당 1세트 (활성 토글 + flat 조합 CRUD + 메타 편집/삭제).
//! - 전역 key_meta: 키별 ActionRow (vowel_combine_head / context_alt 요약 + 편집/삭제).

use std::cell::RefCell;
use std::rc::Rc;

use gtk4::prelude::*;
use gtk4::{self as gtk};
use libadwaita::prelude::*;
use libadwaita::{self as adw};

use crate::dialogs::{combo_edit, key_meta_edit, rule_set_edit};
use crate::state::SharedAppState;

type Rebuild = Rc<dyn Fn()>;

pub fn build(state: SharedAppState) -> gtk::Widget {
    let page = adw::PreferencesPage::new();

    // 규칙 세트 그룹.
    let rs_group = adw::PreferencesGroup::builder()
        .title(rust_i18n::t!("extended_rule_sets_title"))
        .build();
    let rs_add = gtk::Button::builder()
        .icon_name("list-add-symbolic")
        .tooltip_text(rust_i18n::t!("extended_rule_sets_add"))
        .css_classes(["flat"])
        .valign(gtk::Align::Center)
        .build();
    rs_group.set_header_suffix(Some(&rs_add));
    page.add(&rs_group);

    // 전역 key_meta 그룹.
    let km_group = adw::PreferencesGroup::builder()
        .title(rust_i18n::t!("extended_key_meta_title"))
        .description(rust_i18n::t!("extended_key_meta_desc"))
        .build();
    let km_add = gtk::Button::builder()
        .icon_name("list-add-symbolic")
        .tooltip_text(rust_i18n::t!("extended_key_meta_add"))
        .css_classes(["flat"])
        .valign(gtk::Align::Center)
        .build();
    km_group.set_header_suffix(Some(&km_add));
    page.add(&km_group);

    let rs_rows: Rc<RefCell<Vec<gtk::Widget>>> = Rc::new(RefCell::new(Vec::new()));
    let km_rows: Rc<RefCell<Vec<gtk::Widget>>> = Rc::new(RefCell::new(Vec::new()));
    let rebuild_holder: Rc<RefCell<Option<Rebuild>>> = Rc::new(RefCell::new(None));

    let rebuild: Rebuild = {
        let state = state.clone();
        let rs_group = rs_group.clone();
        let km_group = km_group.clone();
        let rs_rows = rs_rows.clone();
        let km_rows = km_rows.clone();
        let rebuild_holder = rebuild_holder.clone();
        Rc::new(move || {
            let trigger = rebuild_holder.borrow().clone().expect("trigger set");

            // ── 규칙 세트 ──
            for w in rs_rows.borrow_mut().drain(..) {
                rs_group.remove(&w);
            }
            let names = state
                .editor
                .borrow()
                .as_ref()
                .map(|e| e.rule_set_names())
                .unwrap_or_default();

            if names.is_empty() {
                let empty = adw::ActionRow::builder()
                    .title(rust_i18n::t!("extended_rule_sets_empty"))
                    .sensitive(false)
                    .build();
                rs_group.add(&empty);
                rs_rows.borrow_mut().push(empty.upcast());
            }

            for name in names {
                let (active, combos) = {
                    let editor = state.editor.borrow();
                    let ed = editor.as_ref().unwrap();
                    let active = ed.rule_sets().get(&name).map(|r| r.active).unwrap_or(false);
                    (active, ed.rule_set_combos(&name))
                };

                let subtitle = if active {
                    rust_i18n::t!("badge_active").to_string()
                } else {
                    rust_i18n::t!("badge_inactive").to_string()
                };
                let expander = adw::ExpanderRow::builder()
                    .title(&name)
                    .subtitle(&subtitle)
                    .build();

                // 헤더 suffix: 메타 편집 + 삭제.
                let edit_meta = icon_btn("document-edit-symbolic", "btn_edit");
                {
                    let state = state.clone();
                    let trigger = trigger.clone();
                    let name = name.clone();
                    edit_meta.connect_clicked(move |b| {
                        rule_set_edit::open(b, state.clone(), Some(name.clone()), trigger.clone());
                    });
                }
                let del_set = icon_btn("user-trash-symbolic", "btn_remove");
                {
                    let state = state.clone();
                    let trigger = trigger.clone();
                    let name = name.clone();
                    del_set.connect_clicked(move |_| {
                        if let Some(ed) = state.editor.borrow_mut().as_mut() {
                            ed.remove_rule_set(&name);
                        }
                        trigger();
                    });
                }
                expander.add_suffix(&edit_meta);
                expander.add_suffix(&del_set);

                // 활성 토글.
                let active_row = adw::SwitchRow::builder()
                    .title(rust_i18n::t!("rule_set_active"))
                    .active(active)
                    .build();
                {
                    let state = state.clone();
                    let trigger = trigger.clone();
                    let name = name.clone();
                    active_row.connect_active_notify(move |s| {
                        if let Some(ed) = state.editor.borrow_mut().as_mut() {
                            ed.toggle_rule_set(&name, s.is_active());
                        }
                        trigger();
                    });
                }
                expander.add_row(&active_row);

                // 조합 목록.
                for (idx, t) in combos.iter().enumerate() {
                    let row = adw::ActionRow::builder()
                        .title(format!("{} + {} → {}", t.first, t.second, t.result))
                        .build();
                    let edit_c = icon_btn("document-edit-symbolic", "btn_edit");
                    {
                        let state = state.clone();
                        let trigger = trigger.clone();
                        let name = name.clone();
                        edit_c.connect_clicked(move |b| {
                            combo_edit::open(
                                b,
                                state.clone(),
                                combo_edit::Target::RuleSet(name.clone()),
                                Some(idx),
                                trigger.clone(),
                            );
                        });
                    }
                    let del_c = icon_btn("user-trash-symbolic", "btn_remove");
                    {
                        let state = state.clone();
                        let trigger = trigger.clone();
                        let name = name.clone();
                        del_c.connect_clicked(move |_| {
                            if let Some(ed) = state.editor.borrow_mut().as_mut() {
                                ed.remove_rule_set_combo(&name, idx);
                            }
                            trigger();
                        });
                    }
                    row.add_suffix(&edit_c);
                    row.add_suffix(&del_c);
                    expander.add_row(&row);
                }

                // 조합 추가 행.
                let add_combo_row = adw::ActionRow::builder()
                    .title(rust_i18n::t!("rule_set_add_combo"))
                    .activatable(true)
                    .build();
                add_combo_row.add_prefix(&gtk::Image::from_icon_name("list-add-symbolic"));
                {
                    let state = state.clone();
                    let trigger = trigger.clone();
                    let name = name.clone();
                    add_combo_row.connect_activated(move |r| {
                        combo_edit::open(
                            r,
                            state.clone(),
                            combo_edit::Target::RuleSet(name.clone()),
                            None,
                            trigger.clone(),
                        );
                    });
                }
                expander.add_row(&add_combo_row);

                // ── 이 세트 한정 key_meta ──
                let km_header = adw::ActionRow::builder()
                    .title(rust_i18n::t!("extended_rule_set_key_meta"))
                    .css_classes(["dim-label"])
                    .sensitive(false)
                    .build();
                expander.add_row(&km_header);

                let rs_metas = state
                    .editor
                    .borrow()
                    .as_ref()
                    .map(|e| e.rule_set_key_meta_iter(&name))
                    .unwrap_or_default();
                for (mkey, meta) in rs_metas {
                    let row = adw::ActionRow::builder()
                        .title(rust_i18n::t!("key_meta_row_title", key = mkey.clone()))
                        .subtitle(meta_summary(&meta))
                        .build();
                    let edit_m = icon_btn("document-edit-symbolic", "btn_edit");
                    {
                        let state = state.clone();
                        let trigger = trigger.clone();
                        let name = name.clone();
                        let mkey = mkey.clone();
                        edit_m.connect_clicked(move |b| {
                            key_meta_edit::open(
                                b,
                                state.clone(),
                                key_meta_edit::Target::RuleSet(name.clone()),
                                Some(mkey.clone()),
                                trigger.clone(),
                            );
                        });
                    }
                    let del_m = icon_btn("user-trash-symbolic", "btn_remove");
                    {
                        let state = state.clone();
                        let trigger = trigger.clone();
                        let name = name.clone();
                        let mkey = mkey.clone();
                        del_m.connect_clicked(move |_| {
                            if let Some(ed) = state.editor.borrow_mut().as_mut() {
                                ed.remove_rule_set_key_meta(&name, &mkey);
                            }
                            trigger();
                        });
                    }
                    row.add_suffix(&edit_m);
                    row.add_suffix(&del_m);
                    expander.add_row(&row);
                }

                let add_km_row = adw::ActionRow::builder()
                    .title(rust_i18n::t!("rule_set_add_key_meta"))
                    .activatable(true)
                    .build();
                add_km_row.add_prefix(&gtk::Image::from_icon_name("list-add-symbolic"));
                {
                    let state = state.clone();
                    let trigger = trigger.clone();
                    let name = name.clone();
                    add_km_row.connect_activated(move |r| {
                        key_meta_edit::open(
                            r,
                            state.clone(),
                            key_meta_edit::Target::RuleSet(name.clone()),
                            None,
                            trigger.clone(),
                        );
                    });
                }
                expander.add_row(&add_km_row);

                rs_group.add(&expander);
                rs_rows.borrow_mut().push(expander.upcast());
            }

            // ── 전역 key_meta ──
            for w in km_rows.borrow_mut().drain(..) {
                km_group.remove(&w);
            }
            let metas = state
                .editor
                .borrow()
                .as_ref()
                .map(|e| e.key_meta_iter())
                .unwrap_or_default();

            if metas.is_empty() {
                let empty = adw::ActionRow::builder()
                    .title(rust_i18n::t!("extended_key_meta_empty"))
                    .sensitive(false)
                    .build();
                km_group.add(&empty);
                km_rows.borrow_mut().push(empty.upcast());
            }

            for (key, meta) in metas {
                let row = adw::ActionRow::builder()
                    .title(rust_i18n::t!("key_meta_row_title", key = key.clone()))
                    .subtitle(meta_summary(&meta))
                    .build();
                let edit_b = icon_btn("document-edit-symbolic", "btn_edit");
                {
                    let state = state.clone();
                    let trigger = trigger.clone();
                    let key = key.clone();
                    edit_b.connect_clicked(move |b| {
                        key_meta_edit::open(
                            b,
                            state.clone(),
                            key_meta_edit::Target::Global,
                            Some(key.clone()),
                            trigger.clone(),
                        );
                    });
                }
                let del_b = icon_btn("user-trash-symbolic", "btn_remove");
                {
                    let state = state.clone();
                    let trigger = trigger.clone();
                    let key = key.clone();
                    del_b.connect_clicked(move |_| {
                        if let Some(ed) = state.editor.borrow_mut().as_mut() {
                            ed.remove_key_meta(&key);
                        }
                        trigger();
                    });
                }
                row.add_suffix(&edit_b);
                row.add_suffix(&del_b);
                km_group.add(&row);
                km_rows.borrow_mut().push(row.upcast());
            }
        })
    };
    *rebuild_holder.borrow_mut() = Some(rebuild.clone());

    // 추가 버튼 연결.
    {
        let state = state.clone();
        let trigger = rebuild.clone();
        rs_add.connect_clicked(move |b| {
            rule_set_edit::open(b, state.clone(), None, trigger.clone());
        });
    }
    {
        let state = state.clone();
        let trigger = rebuild.clone();
        km_add.connect_clicked(move |b| {
            key_meta_edit::open(
                b,
                state.clone(),
                key_meta_edit::Target::Global,
                None,
                trigger.clone(),
            );
        });
    }

    {
        let rebuild = rebuild.clone();
        state.register_refresh(Rc::new(move || rebuild()));
    }

    page.upcast()
}

/// key_meta 요약 문자열 (전역·rule_set 공통).
fn meta_summary(meta: &unim::keystroke::profile::KeyMeta) -> String {
    let mut parts: Vec<String> = Vec::new();
    if let Some(v) = meta.vowel_combine_head {
        parts.push(format!("vowel_combine_head={v}"));
    }
    if let Some(ca) = &meta.context_alt {
        parts.push(format!("context_alt({:?}→{}/{})", ca.when, ca.to, ca.fallback));
    }
    parts.join("  ·  ")
}

fn icon_btn(icon: &str, tooltip_key: &str) -> gtk::Button {
    gtk::Button::builder()
        .icon_name(icon)
        .tooltip_text(rust_i18n::t!(tooltip_key))
        .css_classes(["flat"])
        .valign(gtk::Align::Center)
        .build()
}
