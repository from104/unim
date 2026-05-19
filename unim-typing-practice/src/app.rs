//! 메인 윈도우 — 좌측 사이드바 + 우측 단일 PracticePage.

use std::cell::RefCell;
use std::rc::Rc;

use gtk4::prelude::*;
use gtk4::{self as gtk};
use libadwaita::{self as adw};

use unim::keystroke::profile::LayoutProfile;
use unim_keymap_common::{state, ProfileSidebar, SharedRegistry};

use crate::practice_page;

pub struct AppState {
    pub registry: SharedRegistry,
    pub current_name: RefCell<Option<String>>,
    pub current_profile: RefCell<Option<LayoutProfile>>,
}

pub type SharedAppState = Rc<AppState>;

pub fn build_window(app: &adw::Application) {
    let registry = state::new_shared_registry();
    let state: SharedAppState = Rc::new(AppState {
        registry: registry.clone(),
        current_name: RefCell::new(None),
        current_profile: RefCell::new(None),
    });

    let toast_overlay = adw::ToastOverlay::new();

    let sidebar = ProfileSidebar::new(registry.clone());
    let (page, page_refresh) = practice_page::build(state.clone(), toast_overlay.clone());

    {
        let state = state.clone();
        let refresh = page_refresh.clone();
        sidebar.set_on_select(move |name| {
            let profile = state.registry.borrow().find_raw(name);
            *state.current_name.borrow_mut() = Some(name.to_string());
            *state.current_profile.borrow_mut() = profile;
            refresh();
        });
    }

    let header = adw::HeaderBar::new();
    let title = gtk::Label::new(Some(&rust_i18n::t!("app_title")));
    title.add_css_class("title");
    header.set_title_widget(Some(&title));

    let body = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    body.append(sidebar.root());
    body.append(&gtk::Separator::new(gtk::Orientation::Vertical));
    body.append(&page);
    page.set_hexpand(true);
    page.set_vexpand(true);
    body.set_vexpand(true);

    let outer = gtk::Box::new(gtk::Orientation::Vertical, 0);
    outer.append(&header);
    outer.append(&body);

    toast_overlay.set_child(Some(&outer));

    let window = adw::ApplicationWindow::builder()
        .application(app)
        .title(&*rust_i18n::t!("app_title"))
        .default_width(1000)
        .default_height(680)
        .content(&toast_overlay)
        .build();

    sidebar.select_first();
    apply_css();
    window.present();
}

fn apply_css() {
    let provider = gtk::CssProvider::new();
    provider.load_from_data(
        r#"
button.keymap-cell {
    font-family: monospace;
    padding: 4px;
}
button.keymap-cell-selected {
    background: alpha(@accent_bg_color, 0.35);
    border: 2px solid @accent_color;
}
"#,
    );
    if let Some(display) = gtk::gdk::Display::default() {
        gtk::style_context_add_provider_for_display(
            &display,
            &provider,
            gtk::STYLE_PROVIDER_PRIORITY_APPLICATION,
        );
    }
}
