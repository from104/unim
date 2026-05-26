//! 헤더 좌측 3중 깊이 자판 드롭다운 — 언어 > 출처 > 자판명.
//!
//! 구현 전략 B (안정 우선): `MenuButton` + 커스텀 `Popover`. Popover 내부에
//! `ListBox` 두 섹션(한글/영문) × 두 서브섹션(빌트인/사용자)을 `ExpanderRow`로
//! 펼침/접힘. 자판 선택은 leaf `ActionRow` 클릭 → `on_select(&str)` 콜백.
//!
//! `TreeListModel` 전략 A 는 GTK4 0.9 의 ListView/TreeExpander 통합이 까다로워
//! 후순위. 본 전략 B 는 `adw::ExpanderRow` 가 자동으로 토글·들여쓰기를 처리
//! 하므로 코드량이 적고 동작이 예측 가능.

use std::cell::RefCell;
use std::rc::Rc;

use gtk4::prelude::*;
use gtk4::{self as gtk};
use libadwaita::prelude::*;
use libadwaita::{self as adw};

use unim::keystroke::profile::{builtin::BUILTIN_NAMES, LayoutProfile};
use unim_keymap_common::SharedRegistry;

type SelectCallback = Rc<RefCell<Option<Box<dyn Fn(&str)>>>>;

pub struct ProfileDropdown {
    button: gtk::MenuButton,
    popover: gtk::Popover,
    body: gtk::Box,
    on_select: SelectCallback,
    registry: SharedRegistry,
    current_label: Rc<RefCell<String>>,
}

impl ProfileDropdown {
    pub fn new(registry: SharedRegistry) -> Rc<Self> {
        // 본문(트리)을 담을 박스 — refresh 때 자식을 비우고 다시 채움.
        let body = gtk::Box::builder()
            .orientation(gtk::Orientation::Vertical)
            .spacing(0)
            .build();

        let scrolled = gtk::ScrolledWindow::builder()
            .hscrollbar_policy(gtk::PolicyType::Never)
            .vscrollbar_policy(gtk::PolicyType::Automatic)
            .min_content_width(320)
            .min_content_height(360)
            .max_content_height(520)
            .propagate_natural_height(true)
            .child(&body)
            .build();

        let popover = gtk::Popover::builder()
            .has_arrow(true)
            .autohide(true)
            .child(&scrolled)
            .build();
        popover.add_css_class("menu");

        let button = gtk::MenuButton::builder()
            .label(rust_i18n::t!("header_profile_dropdown_label"))
            .tooltip_text(rust_i18n::t!("header_profile_dropdown_tooltip"))
            .popover(&popover)
            .build();
        button.add_css_class("flat");

        let dropdown = Rc::new(Self {
            button,
            popover,
            body,
            on_select: Rc::new(RefCell::new(None)),
            registry,
            current_label: Rc::new(RefCell::new(String::new())),
        });
        dropdown.refresh();
        dropdown
    }

    pub fn widget(&self) -> &gtk::MenuButton {
        &self.button
    }

    pub fn set_on_select<F: Fn(&str) + 'static>(self: &Rc<Self>, cb: F) {
        *self.on_select.borrow_mut() = Some(Box::new(cb));
    }

    /// 외부에서 선택 표시 갱신 (콜백은 발사하지 않음 — 라벨만).
    pub fn set_current_label(&self, label: &str) {
        *self.current_label.borrow_mut() = label.to_string();
        self.button.set_label(label);
    }

    /// 첫 선택 가능 자판 이름 반환 (자동 선택용). 비어 있으면 None.
    pub fn first_selectable(&self) -> Option<String> {
        let registry = self.registry.borrow();
        for code in ["korean", "english"] {
            for name in BUILTIN_NAMES {
                if let Some(p) = registry.find_raw(name) {
                    if p.language == code {
                        return Some(name.to_string());
                    }
                }
            }
        }
        None
    }

    /// 자판 이름으로 적절한 라벨을 만들어 버튼 갱신.
    pub fn select_by_name(self: &Rc<Self>, name: &str) {
        let registry = self.registry.borrow();
        if let Some(p) = registry.find_raw(name) {
            let label = label_for_profile(&p);
            drop(registry);
            self.set_current_label(&label);
        } else {
            self.set_current_label(name);
        }
    }

    /// 레지스트리 재스캔 + 트리 다시 그리기.
    pub fn refresh(self: &Rc<Self>) {
        while let Some(c) = self.body.first_child() {
            self.body.remove(&c);
        }
        let registry = self.registry.borrow();

        for (lang_code, lang_label) in [
            ("korean", rust_i18n::t!("dropdown_lang_korean").to_string()),
            ("english", rust_i18n::t!("dropdown_lang_english").to_string()),
        ] {
            let lang_header = gtk::Label::builder()
                .label(&lang_label)
                .halign(gtk::Align::Start)
                .css_classes(["heading", "dim-label"])
                .margin_start(12)
                .margin_top(8)
                .margin_bottom(4)
                .build();
            self.body.append(&lang_header);

            let list = gtk::ListBox::builder()
                .selection_mode(gtk::SelectionMode::None)
                .css_classes(["boxed-list"])
                .margin_start(8)
                .margin_end(8)
                .margin_bottom(8)
                .build();

            // 빌트인
            let builtin_names: Vec<&str> = BUILTIN_NAMES
                .iter()
                .filter(|n| {
                    registry
                        .find_raw(n)
                        .map(|p| p.language == lang_code)
                        .unwrap_or(false)
                })
                .copied()
                .collect();

            let builtin_exp = adw::ExpanderRow::builder()
                .title(rust_i18n::t!("dropdown_source_builtin"))
                .expanded(true)
                .build();
            if builtin_names.is_empty() {
                let empty = adw::ActionRow::builder()
                    .title(rust_i18n::t!("dropdown_empty"))
                    .sensitive(false)
                    .build();
                builtin_exp.add_row(&empty);
            } else {
                for name in builtin_names {
                    let p = registry.find_raw(name);
                    let row = make_leaf_row(name, p.as_ref(), self);
                    builtin_exp.add_row(&row);
                }
            }
            list.append(&builtin_exp);

            // 사용자 (해당 언어)
            let user_names: Vec<String> = registry
                .user_names()
                .into_iter()
                .filter(|n| !BUILTIN_NAMES.contains(&n.as_str()))
                .filter(|n| {
                    registry
                        .find_raw(n)
                        .map(|p| p.language == lang_code)
                        .unwrap_or(false)
                })
                .collect();

            let user_exp = adw::ExpanderRow::builder()
                .title(rust_i18n::t!("dropdown_source_user"))
                .expanded(!user_names.is_empty())
                .build();
            if user_names.is_empty() {
                let empty = adw::ActionRow::builder()
                    .title(rust_i18n::t!("dropdown_empty"))
                    .sensitive(false)
                    .build();
                user_exp.add_row(&empty);
            } else {
                for name in user_names {
                    let p = registry.find_raw(&name);
                    let row = make_leaf_row(&name, p.as_ref(), self);
                    user_exp.add_row(&row);
                }
            }
            list.append(&user_exp);

            self.body.append(&list);
        }
    }
}

/// 자판 leaf 행 — display_name 우선, 식별자 보조. 클릭 시 콜백 발사 + popover 닫기.
fn make_leaf_row(
    name: &str,
    profile: Option<&LayoutProfile>,
    dropdown: &Rc<ProfileDropdown>,
) -> adw::ActionRow {
    let label = profile
        .map(label_for_profile)
        .unwrap_or_else(|| name.to_string());
    let row = adw::ActionRow::builder()
        .title(&label)
        .subtitle(name)
        .activatable(true)
        .build();
    if let Some(p) = profile {
        if !p.layout_type.is_empty() {
            let badge = gtk::Label::builder()
                .label(&p.layout_type)
                .css_classes(["caption", "dim-label"])
                .build();
            row.add_suffix(&badge);
        }
    }
    let name_owned = name.to_string();
    let dropdown_weak = Rc::downgrade(dropdown);
    row.connect_activated(move |_| {
        if let Some(dd) = dropdown_weak.upgrade() {
            dd.popover.popdown();
            dd.set_current_label(&label);
            if let Some(cb) = dd.on_select.borrow().as_ref() {
                cb(&name_owned);
            }
        }
    });
    row
}

/// 화면 표시용 라벨 — display_name 의 ko 우선, 없으면 name.
fn label_for_profile(profile: &LayoutProfile) -> String {
    use unim::keystroke::profile::LocalizedText;
    let display = match &profile.metadata.display_name {
        Some(LocalizedText::Single(s)) => Some(s.clone()),
        Some(LocalizedText::Map(map)) => {
            let loc = rust_i18n::locale();
            let loc_str: &str = &loc;
            map.get(loc_str)
                .or_else(|| map.get("ko"))
                .or_else(|| map.get("en"))
                .cloned()
        }
        None => None,
    };
    match display {
        Some(s) if !s.is_empty() => format!("{} ({})", s, profile.name),
        _ => profile.name.clone(),
    }
}
