//! 조합 탭 — 한글 자판 전용. 초성/중성/종성 각 섹션의 1+1=1 조합 CRUD.
//!
//! 각 섹션은 `adw::PreferencesGroup` 이며 헤더에 [+] 추가 버튼, 본문에 조합 행
//! ("ㄱ + ㄱ → ㄲ", suffix [편집][삭제]) 를 둔다. 추가/편집은 `combo_edit` 다이얼로그.

use std::cell::RefCell;
use std::rc::Rc;

use gtk4::prelude::*;
use gtk4::{self as gtk};
use libadwaita::prelude::*;
use libadwaita::{self as adw};

use crate::dialogs::combo_edit;
use crate::state::{ComboKind, SharedAppState};

/// 조합 섹션 재구성 트리거.
type Rebuild = Rc<dyn Fn()>;

struct Section {
    kind: ComboKind,
    group: adw::PreferencesGroup,
    add_btn: gtk::Button,
    rows: Rc<RefCell<Vec<gtk::Widget>>>,
}

pub fn build(state: SharedAppState) -> gtk::Widget {
    let page = adw::PreferencesPage::new();

    let defs = [
        (ComboKind::Cho, "combos_cho"),
        (ComboKind::Jung, "combos_jung"),
        (ComboKind::Jong, "combos_jong"),
    ];

    let mut sections: Vec<Section> = Vec::new();
    for (kind, title_key) in defs {
        let group = adw::PreferencesGroup::builder()
            .title(&*rust_i18n::t!(title_key))
            .build();
        let add_btn = gtk::Button::builder()
            .icon_name("list-add-symbolic")
            .tooltip_text(rust_i18n::t!("btn_add"))
            .css_classes(["flat"])
            .valign(gtk::Align::Center)
            .build();
        group.set_header_suffix(Some(&add_btn));
        page.add(&group);
        sections.push(Section {
            kind,
            group,
            add_btn,
            rows: Rc::new(RefCell::new(Vec::new())),
        });
    }

    let sections = Rc::new(sections);
    let rebuild_holder: Rc<RefCell<Option<Rebuild>>> = Rc::new(RefCell::new(None));

    let rebuild: Rebuild = {
        let state = state.clone();
        let sections = sections.clone();
        let rebuild_holder = rebuild_holder.clone();
        Rc::new(move || {
            let trigger = rebuild_holder
                .borrow()
                .clone()
                .expect("rebuild trigger set");
            for sec in sections.iter() {
                // 기존 행 제거.
                for w in sec.rows.borrow_mut().drain(..) {
                    sec.group.remove(&w);
                }
                let combos = state
                    .editor
                    .borrow()
                    .as_ref()
                    .map(|e| e.combos(sec.kind))
                    .unwrap_or_default();

                if combos.is_empty() {
                    let empty = adw::ActionRow::builder()
                        .title(rust_i18n::t!("combo_section_empty"))
                        .sensitive(false)
                        .build();
                    sec.group.add(&empty);
                    sec.rows.borrow_mut().push(empty.upcast());
                    continue;
                }

                for (idx, t) in combos.iter().enumerate() {
                    let row = adw::ActionRow::builder()
                        .title(format!("{} + {} → {}", t.first, t.second, t.result))
                        .subtitle(format!("#{idx}"))
                        .build();

                    let edit_btn = gtk::Button::builder()
                        .icon_name("document-edit-symbolic")
                        .tooltip_text(rust_i18n::t!("btn_edit"))
                        .css_classes(["flat"])
                        .valign(gtk::Align::Center)
                        .build();
                    {
                        let state = state.clone();
                        let trigger = trigger.clone();
                        let kind = sec.kind;
                        edit_btn.connect_clicked(move |b| {
                            combo_edit::open(
                                b,
                                state.clone(),
                                combo_edit::Target::Base(kind),
                                Some(idx),
                                trigger.clone(),
                            );
                        });
                    }

                    let del_btn = gtk::Button::builder()
                        .icon_name("user-trash-symbolic")
                        .tooltip_text(rust_i18n::t!("btn_remove"))
                        .css_classes(["flat"])
                        .valign(gtk::Align::Center)
                        .build();
                    {
                        let state = state.clone();
                        let trigger = trigger.clone();
                        let kind = sec.kind;
                        del_btn.connect_clicked(move |_| {
                            if let Some(ed) = state.editor.borrow_mut().as_mut() {
                                ed.remove_combo(kind, idx);
                            }
                            trigger();
                        });
                    }

                    row.add_suffix(&edit_btn);
                    row.add_suffix(&del_btn);
                    sec.group.add(&row);
                    sec.rows.borrow_mut().push(row.upcast());
                }
            }
        })
    };
    *rebuild_holder.borrow_mut() = Some(rebuild.clone());

    // 추가 버튼 연결.
    for sec in sections.iter() {
        let state = state.clone();
        let trigger = rebuild.clone();
        let kind = sec.kind;
        sec.add_btn.connect_clicked(move |b| {
            combo_edit::open(
                b,
                state.clone(),
                combo_edit::Target::Base(kind),
                None,
                trigger.clone(),
            );
        });
    }

    // refresh — 프로필 변경 시 재구성.
    {
        let rebuild = rebuild.clone();
        state.register_refresh(Rc::new(move || rebuild()));
    }

    page.upcast()
}
