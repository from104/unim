//! 메인 윈도우 — 헤더(좌: ProfileDropdown + 저장 / 우: 도움말·설정·메뉴) + ViewStack 4탭.
//!
//! 구조:
//!   adw::ApplicationWindow
//!   └─ ToastOverlay
//!      └─ Box(vertical)
//!         ├─ adw::HeaderBar
//!         │   start: ProfileDropdown (3중 깊이) + [저장] [다른 이름으로 저장]
//!         │   title: WindowTitle (앱 이름 + 현재 자판)
//!         │   end:   [도움말 ?] [UNIM 설정 ⚙] [메뉴 ☰]
//!         ├─ adw::ViewSwitcherBar (segmented)
//!         └─ adw::ViewStack (기본 / 자판 / 조합 / 확장)
//!
//! editor 교체 후 UI 갱신은 `AppState::run_ui_refresh` 단일 진입점으로 통일.

use std::rc::Rc;

use gtk4::prelude::*;
use gtk4::{self as gtk, gio};
use libadwaita::prelude::*;
use libadwaita::{self as adw};

use unim_keymap_common::state::new_shared_registry;

use crate::dialogs::save_as;
use crate::header::{menu_actions, ProfileDropdown};
use crate::state::editor_state::is_builtin_selection;
use crate::state::{AppState, EditorState, SharedAppState};
use crate::tabs::{tab_basic, tab_combos, tab_extended, tab_keymap};

pub fn build_window(app: &adw::Application) {
    let registry = new_shared_registry();
    let toast = adw::ToastOverlay::new();
    let state = AppState::new(registry.clone(), toast.clone());

    // 헤더 — 좌측 ProfileDropdown + 저장 버튼들.
    let dropdown = ProfileDropdown::new(registry.clone());
    let header = adw::HeaderBar::builder().build();
    header.pack_start(dropdown.widget());

    let btn_save = gtk::Button::builder()
        .icon_name("document-save-symbolic")
        .tooltip_text(rust_i18n::t!("btn_save"))
        .action_name("win.save")
        .build();
    header.pack_start(&btn_save);

    let btn_save_as = gtk::Button::builder()
        .icon_name("document-save-as-symbolic")
        .tooltip_text(rust_i18n::t!("btn_save_as"))
        .action_name("win.save-as")
        .build();
    header.pack_start(&btn_save_as);

    let title = adw::WindowTitle::new(&rust_i18n::t!("app_title"), "");
    header.set_title_widget(Some(&title));

    // 헤더 우측 — 시각 순서(좌→우): [도움말 ?] [UNIM 설정 ⚙] [메뉴 ☰].
    let btn_menu = gtk::MenuButton::builder()
        .icon_name("open-menu-symbolic")
        .tooltip_text(rust_i18n::t!("header_menu_tooltip"))
        .menu_model(&menu_actions::build_menu_model())
        .build();
    header.pack_end(&btn_menu);

    let btn_settings = gtk::Button::builder()
        .icon_name("preferences-system-symbolic")
        .tooltip_text(rust_i18n::t!("header_settings_tooltip"))
        .build();
    btn_settings.connect_clicked(|_| {
        let _ = std::process::Command::new("unim-settings-gtk").spawn();
    });
    header.pack_end(&btn_settings);

    let btn_help = gtk::Button::builder()
        .icon_name("help-about-symbolic")
        .tooltip_text(rust_i18n::t!("header_help_tooltip"))
        .action_name("win.help")
        .build();
    header.pack_end(&btn_help);

    // ViewStack 4 탭.
    let stack = adw::ViewStack::new();
    let _page_basic = stack.add_titled_with_icon(
        &tab_basic::build(state.clone()),
        Some("basic"),
        &rust_i18n::t!("tab_basic"),
        "preferences-system-symbolic",
    );
    let _page_keymap = stack.add_titled_with_icon(
        &tab_keymap::build(state.clone()),
        Some("keymap"),
        &rust_i18n::t!("tab_keymap"),
        "input-keyboard-symbolic",
    );
    let page_combos = stack.add_titled_with_icon(
        &tab_combos::build(state.clone()),
        Some("combos"),
        &rust_i18n::t!("tab_combos"),
        "view-list-symbolic",
    );
    let page_extended = stack.add_titled_with_icon(
        &tab_extended::build(state.clone()),
        Some("extended"),
        &rust_i18n::t!("tab_extended"),
        "applications-engineering-symbolic",
    );

    let switcher_bar = adw::ViewSwitcherBar::builder()
        .stack(&stack)
        .reveal(true)
        .build();

    let body = gtk::Box::new(gtk::Orientation::Vertical, 0);
    body.append(&switcher_bar);
    body.append(&stack);
    stack.set_vexpand(true);
    stack.set_hexpand(true);

    let outer = gtk::Box::new(gtk::Orientation::Vertical, 0);
    outer.append(&header);
    outer.append(&body);
    body.set_vexpand(true);

    toast.set_child(Some(&outer));

    body.insert_action_group("win", Some(&state.action_group));
    header.insert_action_group("win", Some(&state.action_group));

    let window = adw::ApplicationWindow::builder()
        .application(app)
        .title(rust_i18n::t!("app_title"))
        .default_width(1024)
        .default_height(720)
        .content(&toast)
        .build();

    // 언어 변경 옵저버 — 기본 탭에서 한글/영문 전환 시 조합·확장 탭 가시성 토글.
    {
        let page_combos = page_combos.clone();
        let page_extended = page_extended.clone();
        state.register_language_observer(Rc::new(move |is_korean| {
            page_combos.set_visible(is_korean);
            page_extended.set_visible(is_korean);
        }));
    }

    // 전체 UI 갱신 진입점 — editor/current_name/is_builtin 을 읽어 UI 동기화.
    {
        let state_weak = Rc::downgrade(&state);
        let title = title.clone();
        let dropdown = dropdown.clone();
        let page_combos = page_combos.clone();
        let page_extended = page_extended.clone();
        state.set_ui_refresh(Rc::new(move || {
            let Some(state) = state_weak.upgrade() else {
                return;
            };
            let name = state.current_name.borrow().clone().unwrap_or_default();
            let is_korean = state
                .editor
                .borrow()
                .as_ref()
                .map(|e| e.buf.language == "korean")
                .unwrap_or(false);
            let is_builtin = state.is_builtin.get();
            page_combos.set_visible(is_korean);
            page_extended.set_visible(is_korean);
            let subtitle = if is_builtin {
                format!("{}  ·  {}", name, &*rust_i18n::t!("badge_builtin"))
            } else {
                name.clone()
            };
            title.set_subtitle(&subtitle);
            dropdown.select_by_name(&name);
            state.refresh_all();
        }));
    }

    // import 후 사용자 폴더에서 자판 재선택하는 콜백.
    let select_profile: Rc<dyn Fn(String)> = {
        let state = state.clone();
        let dropdown = dropdown.clone();
        Rc::new(move |name: String| {
            state.registry.borrow_mut().scan();
            dropdown.refresh();
            apply_profile_selection(state.clone(), &name);
        })
    };

    // ── 액션: win.save ───────────────────────────────────────────────────
    let act_save = gio::SimpleAction::new("save", None);
    {
        let state = state.clone();
        let dropdown = dropdown.clone();
        act_save.connect_activate(move |_, _| {
            if state.is_builtin.get() {
                state.toast(&rust_i18n::t!("toast_builtin_save_blocked"));
                return;
            }
            let res = state.editor.borrow_mut().as_mut().map(|e| e.save());
            match res {
                Some(Ok(path)) => {
                    state.toast(&rust_i18n::t!("toast_saved", path = path.to_string_lossy()));
                    state.registry.borrow_mut().scan();
                    dropdown.refresh();
                    state.run_ui_refresh();
                }
                Some(Err(e)) => {
                    state.toast(&rust_i18n::t!("toast_save_failed", err = e.to_string()));
                }
                None => {}
            }
        });
    }
    state.action_group.add_action(&act_save);

    // ── 액션: win.save-as ────────────────────────────────────────────────
    let act_save_as = gio::SimpleAction::new("save-as", None);
    {
        let state = state.clone();
        let window = window.clone();
        let select_profile = select_profile.clone();
        act_save_as.connect_activate(move |_, _| {
            if state.editor.borrow().is_none() {
                return;
            }
            let prefill = {
                let cur = state.current_name.borrow().clone().unwrap_or_default();
                if state.is_builtin.get() {
                    format!("{cur}_copy")
                } else {
                    cur
                }
            };
            let on_saved: Rc<dyn Fn(String)> = select_profile.clone();
            save_as::open(&window, state.clone(), &prefill, on_saved);
        });
    }
    state.action_group.add_action(&act_save_as);

    // ── 액션: win.revert ─────────────────────────────────────────────────
    let act_revert = gio::SimpleAction::new("revert", None);
    {
        let state = state.clone();
        act_revert.connect_activate(move |_, _| {
            let had = {
                let mut editor = state.editor.borrow_mut();
                match editor.as_mut() {
                    Some(ed) => {
                        ed.revert();
                        true
                    }
                    None => false,
                }
            };
            if had {
                state.run_ui_refresh();
                state.toast(&rust_i18n::t!("toast_reverted"));
            }
        });
    }
    state.action_group.add_action(&act_revert);

    // 탭 전환 액션 (Ctrl+1~4).
    for (name, child) in [
        ("tab-basic", "basic"),
        ("tab-keymap", "keymap"),
        ("tab-combos", "combos"),
        ("tab-extended", "extended"),
    ] {
        let action = gio::SimpleAction::new(name, None);
        let stack = stack.clone();
        let child = child.to_string();
        action.connect_activate(move |_, _| {
            stack.set_visible_child_name(&child);
        });
        state.action_group.add_action(&action);
    }

    // 헤더 메뉴 액션 (새 자판/복제/내보내기/가져오기/폴더/도움말).
    menu_actions::register(state.clone(), window.clone(), select_profile.clone());

    // 저장 버튼 sensitivity — 빌트인이면 'Save' disable.
    {
        let state_weak = Rc::downgrade(&state);
        let act_save = act_save.clone();
        state.register_refresh(Rc::new(move || {
            if let Some(state) = state_weak.upgrade() {
                let has_editor = state.editor.borrow().is_some();
                act_save.set_enabled(has_editor && !state.is_builtin.get());
            }
        }));
    }

    // 단축키 컨트롤러.
    install_shortcuts(&window);

    // ProfileDropdown 선택 콜백 — dirty 가드 후 자판 변경.
    {
        let state = state.clone();
        let window = window.clone();
        let dropdown_weak = Rc::downgrade(&dropdown);
        dropdown.set_on_select(move |name| {
            let name = name.to_string();
            let proceed: Rc<dyn Fn()> = {
                let state = state.clone();
                let name = name.clone();
                Rc::new(move || apply_profile_selection(state.clone(), &name))
            };
            let dirty = state
                .editor
                .borrow()
                .as_ref()
                .map(|e| e.dirty)
                .unwrap_or(false);
            if dirty {
                guard_discard(&window, state.clone(), proceed, dropdown_weak.clone());
            } else {
                proceed();
            }
        });
    }

    apply_css();

    // 첫 자판 자동 선택.
    if let Some(first) = dropdown.first_selectable() {
        apply_profile_selection(state.clone(), &first);
    }

    window.present();
}

/// 자판 선택 적용 — registry 에서 editor 재생성 + is_builtin 갱신 + UI 갱신.
fn apply_profile_selection(state: SharedAppState, name: &str) {
    let profile = state.registry.borrow().find_raw(name);
    *state.current_name.borrow_mut() = Some(name.to_string());
    *state.editor.borrow_mut() = profile.map(EditorState::new);

    let is_builtin = is_builtin_selection(name, &state.registry.borrow());
    state.is_builtin.set(is_builtin);

    state.run_ui_refresh();

    if is_builtin {
        state.toast(&rust_i18n::t!("toast_builtin_save_blocked"));
    }
}

/// 미저장 변경 가드 — dirty 상태에서 자판 이동 시 확인 다이얼로그.
fn guard_discard(
    window: &adw::ApplicationWindow,
    state: SharedAppState,
    proceed: Rc<dyn Fn()>,
    dropdown_weak: std::rc::Weak<ProfileDropdown>,
) {
    let dialog = adw::MessageDialog::new(
        Some(window),
        Some(&rust_i18n::t!("discard_changes_title")),
        Some(&rust_i18n::t!("discard_changes_body")),
    );
    dialog.add_response("keep", &rust_i18n::t!("discard_btn_keep"));
    dialog.add_response("discard", &rust_i18n::t!("discard_btn_discard"));
    dialog.set_response_appearance("discard", adw::ResponseAppearance::Destructive);
    dialog.set_default_response(Some("keep"));
    dialog.set_close_response("keep");

    let current = state.current_name.borrow().clone();
    dialog.connect_response(None, move |_, resp| {
        if resp == "discard" {
            proceed();
        } else if let (Some(dd), Some(cur)) = (dropdown_weak.upgrade(), current.as_ref()) {
            dd.select_by_name(cur);
        }
    });
    dialog.present();
}

/// 윈도우에 단축키 컨트롤러 설치 (win.* 액션 트리거).
fn install_shortcuts(window: &adw::ApplicationWindow) {
    let controller = gtk::ShortcutController::new();
    controller.set_scope(gtk::ShortcutScope::Managed);
    let bindings = [
        ("F1", "win.help"),
        ("<Primary>n", "win.new-profile"),
        ("<Primary>d", "win.duplicate-profile"),
        ("<Primary>s", "win.save"),
        ("<Primary><Shift>s", "win.save-as"),
        ("<Primary>e", "win.export-json"),
        ("<Primary>i", "win.import-json"),
        ("<Primary>1", "win.tab-basic"),
        ("<Primary>2", "win.tab-keymap"),
        ("<Primary>3", "win.tab-combos"),
        ("<Primary>4", "win.tab-extended"),
    ];
    for (trigger, action) in bindings {
        if let Some(t) = gtk::ShortcutTrigger::parse_string(trigger) {
            let a = gtk::NamedAction::new(action);
            controller.add_shortcut(gtk::Shortcut::new(Some(t), Some(a)));
        }
    }
    window.add_controller(controller);
}

fn apply_css() {
    let provider = gtk::CssProvider::new();
    let css = format!(
        r#"
.studio-keyboard-card {{
    background: transparent;
    border: none;
}}
{}
"#,
        unim_keymap_common::keyboard_view::KEYBOARD_CSS
    );
    provider.load_from_data(&css);
    if let Some(display) = gtk::gdk::Display::default() {
        gtk::style_context_add_provider_for_display(
            &display,
            &provider,
            gtk::STYLE_PROVIDER_PRIORITY_APPLICATION,
        );
    }
}
