//! 좌측 프로필 사이드바 — 내장/사용자 자판 목록.
//!
//! ListBox 두 섹션:
//! 1. 내장(`BUILTIN_NAMES`) — 사용자 디렉토리에서 override되면 "overrides" 뱃지.
//! 2. 사용자 전용 — `is_user_override == true`이면서 내장과 이름이 겹치지 않는 것.
//!
//! 행마다 v3 마커가 있으면 "moachigi" 뱃지, schema_version >= 3이면 "v3" 뱃지.
//! 선택 변경 시 callback(`fn(&str)`) 호출.

use std::cell::RefCell;
use std::rc::Rc;

use gtk4::prelude::*;
use gtk4::{self as gtk};
use libadwaita::prelude::*;
use libadwaita::{self as adw};

use crate::state::SharedRegistry;

type SelectCallback = Rc<RefCell<Option<Box<dyn Fn(&str)>>>>;

pub struct ProfileSidebar {
    root: gtk::ScrolledWindow,
    list: gtk::ListBox,
    on_select: SelectCallback,
    registry: SharedRegistry,
}

impl ProfileSidebar {
    pub fn new(registry: SharedRegistry) -> Self {
        let list = gtk::ListBox::builder()
            .selection_mode(gtk::SelectionMode::Single)
            .css_classes(["navigation-sidebar"])
            .build();

        let scrolled = gtk::ScrolledWindow::builder()
            .hscrollbar_policy(gtk::PolicyType::Never)
            .vscrollbar_policy(gtk::PolicyType::Automatic)
            .width_request(240)
            .child(&list)
            .build();

        let on_select: SelectCallback = Rc::new(RefCell::new(None));
        {
            let on_select = on_select.clone();
            list.connect_row_selected(move |_, row| {
                if let Some(r) = row {
                    if let Some(name) = unsafe {
                        r.data::<String>("profile_name")
                            .map(|p| p.as_ref().clone())
                    } {
                        if let Some(cb) = on_select.borrow().as_ref() {
                            cb(&name);
                        }
                    }
                }
            });
        }

        let sidebar = Self {
            root: scrolled,
            list,
            on_select,
            registry,
        };
        sidebar.refresh();
        sidebar
    }

    pub fn root(&self) -> &gtk::ScrolledWindow {
        &self.root
    }

    pub fn set_on_select<F: Fn(&str) + 'static>(&self, cb: F) {
        *self.on_select.borrow_mut() = Some(Box::new(cb));
    }

    /// 외부에서 프로필 선택 강제.
    pub fn select_by_name(&self, name: &str) {
        let mut row = self.list.first_child();
        while let Some(c) = row {
            if let Ok(lb_row) = c.clone().downcast::<gtk::ListBoxRow>() {
                if let Some(stored) = unsafe {
                    lb_row
                        .data::<String>("profile_name")
                        .map(|p| p.as_ref().clone())
                } {
                    if stored == name {
                        self.list.select_row(Some(&lb_row));
                        return;
                    }
                }
            }
            row = c.next_sibling();
        }
    }

    /// 첫 항목 선택 (initial activate용).
    pub fn select_first(&self) {
        let mut row = self.list.first_child();
        while let Some(c) = row {
            if let Ok(lb_row) = c.clone().downcast::<gtk::ListBoxRow>() {
                if lb_row.is_selectable() {
                    self.list.select_row(Some(&lb_row));
                    return;
                }
            }
            row = c.next_sibling();
        }
    }

    /// 레지스트리 재스캔 + 항목 다시 그리기.
    pub fn refresh(&self) {
        // 기존 비우기
        while let Some(c) = self.list.first_child() {
            self.list.remove(&c);
        }
        let registry = self.registry.borrow();

        // 1) 내장
        let header_builtin = make_header(&rust_i18n::t!("sidebar_builtin"));
        self.list.append(&header_builtin);

        for name in unim::keystroke::profile::builtin::BUILTIN_NAMES.iter() {
            let profile = registry.find_raw(name);
            let row = make_profile_row(
                name,
                profile.as_ref(),
                registry.is_user_override(name),
                false,
            );
            self.list.append(&row);
        }

        // 2) 사용자 전용 (내장과 이름이 겹치지 않는 것)
        let user_names: Vec<String> = registry
            .user_names()
            .into_iter()
            .filter(|n| {
                !unim::keystroke::profile::builtin::BUILTIN_NAMES.contains(&n.as_str())
            })
            .collect();

        if !user_names.is_empty() {
            let header_user = make_header(&rust_i18n::t!("sidebar_user"));
            self.list.append(&header_user);
            for name in user_names {
                let profile = registry.find_raw(&name);
                let row = make_profile_row(&name, profile.as_ref(), false, true);
                self.list.append(&row);
            }
        }
    }
}

fn make_header(label: &str) -> gtk::ListBoxRow {
    let lbl = gtk::Label::builder()
        .label(label)
        .css_classes(["heading", "dim-label"])
        .halign(gtk::Align::Start)
        .margin_start(12)
        .margin_top(12)
        .margin_bottom(4)
        .build();
    gtk::ListBoxRow::builder()
        .activatable(false)
        .selectable(false)
        .child(&lbl)
        .build()
}

fn make_profile_row(
    name: &str,
    profile: Option<&unim::keystroke::profile::LayoutProfile>,
    overrides_builtin: bool,
    user_only: bool,
) -> gtk::ListBoxRow {
    let row = adw::ActionRow::builder()
        .title(name)
        .activatable(true)
        .build();

    if let Some(p) = profile {
        let mut subtitle_parts = Vec::new();
        if !p.layout_type.is_empty() {
            subtitle_parts.push(p.layout_type.clone());
        }
        if p.schema_version >= 3 {
            subtitle_parts.push(rust_i18n::t!("badge_v3").to_string());
        }
        if p.moachigi.is_some() {
            subtitle_parts.push(rust_i18n::t!("badge_moachigi").to_string());
        }
        if !subtitle_parts.is_empty() {
            row.set_subtitle(&subtitle_parts.join(" · "));
        }
    }

    if overrides_builtin {
        let badge = gtk::Label::builder()
            .label(&*rust_i18n::t!("badge_override"))
            .css_classes(["caption", "accent"])
            .build();
        row.add_suffix(&badge);
    }
    if user_only {
        let badge = gtk::Label::builder()
            .label("●")
            .css_classes(["caption", "accent"])
            .build();
        row.add_suffix(&badge);
    }

    let lb_row = gtk::ListBoxRow::builder().child(&row).build();
    unsafe {
        lb_row.set_data("profile_name", name.to_string());
    }
    lb_row
}
