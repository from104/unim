//! 메인 윈도우 — 좌측 사이드바 + 우측 (View/Edit) 스택.

use std::cell::RefCell;
use std::rc::Rc;

use gtk4::prelude::*;
use gtk4::{self as gtk};
use libadwaita::{self as adw};

use unim::keystroke::profile::LayoutProfile;
use unim_keymap_common::{state, ProfileSidebar, SharedRegistry};

use crate::editor_state::EditorState;
use crate::{tab_edit, tab_view};

/// 두 탭이 공유하는 상태.
pub struct AppState {
    pub registry: SharedRegistry,
    pub current_name: RefCell<Option<String>>,
    pub current_profile: RefCell<Option<LayoutProfile>>,
    pub editor: RefCell<Option<EditorState>>,
}

pub type SharedAppState = Rc<AppState>;

pub fn build_window(app: &adw::Application) {
    let registry = state::new_shared_registry();
    let state: SharedAppState = Rc::new(AppState {
        registry: registry.clone(),
        current_name: RefCell::new(None),
        current_profile: RefCell::new(None),
        editor: RefCell::new(None),
    });

    let toast_overlay = adw::ToastOverlay::new();

    // 좌측: ProfileSidebar
    let sidebar = ProfileSidebar::new(registry.clone());

    // 우측: HeaderBar + ViewSwitcher + ViewStack
    let stack = adw::ViewStack::new();

    let (view_page, view_refresh) = tab_view::build(state.clone());
    let (edit_page, edit_refresh) = tab_edit::build(state.clone(), toast_overlay.clone());

    stack.add_titled_with_icon(
        &view_page,
        Some("view"),
        &rust_i18n::t!("tab_view"),
        "view-list-symbolic",
    );
    stack.add_titled_with_icon(
        &edit_page,
        Some("edit"),
        &rust_i18n::t!("tab_edit"),
        "document-edit-symbolic",
    );

    let switcher = adw::ViewSwitcher::builder()
        .stack(&stack)
        .policy(adw::ViewSwitcherPolicy::Wide)
        .build();

    let header = adw::HeaderBar::builder().title_widget(&switcher).build();

    // 사이드바 선택 변경 → 두 탭에 알림
    {
        let state = state.clone();
        let view_refresh = view_refresh.clone();
        let edit_refresh = edit_refresh.clone();
        sidebar.set_on_select(move |name| {
            let profile = state.registry.borrow().find_raw(name);
            *state.current_name.borrow_mut() = Some(name.to_string());
            *state.current_profile.borrow_mut() = profile.clone();
            *state.editor.borrow_mut() = profile.clone().map(EditorState::new);
            view_refresh();
            edit_refresh();
        });
    }

    // 본문 가로 레이아웃
    let body = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    body.append(sidebar.root());
    let separator = gtk::Separator::new(gtk::Orientation::Vertical);
    body.append(&separator);
    body.append(&stack);
    stack.set_hexpand(true);
    stack.set_vexpand(true);

    let outer = gtk::Box::new(gtk::Orientation::Vertical, 0);
    outer.append(&header);
    outer.append(&body);
    body.set_vexpand(true);

    toast_overlay.set_child(Some(&outer));

    let window = adw::ApplicationWindow::builder()
        .application(app)
        .title(&*rust_i18n::t!("app_title"))
        .default_width(960)
        .default_height(640)
        .content(&toast_overlay)
        .build();

    // 첫 항목 자동 선택
    sidebar.select_first();

    // 공유 CSS — 키 셀 하이라이트
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
