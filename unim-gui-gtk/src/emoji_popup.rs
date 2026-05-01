//! 이모지 팝업 윈도우
//!
//! DBus `ShowEmojiPopup` 시그널을 받아 이모지 선택 팝업을 표시합니다.
//! - 카테고리 탭 (즐겨찾기 + `unim::hangul::emoji::categories()`)
//! - 검색 (`search_emoji(keyword)`)
//! - 즐겨찾기 MRU (선택 시 자동 갱신; 엔진이 `touch_favorite` 호출)
//!
//! GUI가 모든 상태를 자체 관리하고, 선택 확정 시 DBus `CommitEmoji`로 커밋합니다.

use std::cell::RefCell;
use std::rc::Rc;

use gtk4::prelude::*;
use unim::hangul::emoji;
use unim::unim_log;

use crate::popup_positioning::{self, DisplayServer};

/// 즐겨찾기 탭 식별자
const FAVORITES_KEY: &str = "favorites";
/// 검색 결과 탭 식별자
const SEARCH_KEY: &str = "search";
/// 한 줄당 최대 탭 수 (한자/특수 popup 9열과 통일)
const TAB_ROW_MAX: usize = 9;

/// 이모지 팝업 상태
pub struct EmojiPopup {
    pub window: gtk4::Window,
    display_server: DisplayServer,
    search_entry: gtk4::SearchEntry,
    stack: gtk4::Stack,
    favorites_flow: gtk4::FlowBox,
}

impl EmojiPopup {
    /// 새 이모지 팝업 생성
    pub fn new(app: &libadwaita::Application) -> Self {
        let display_server = popup_positioning::detect_display_server();

        let window = gtk4::Window::builder()
            .application(app)
            .decorated(false)
            .resizable(false)
            .default_width(380)
            .default_height(320)
            .build();
        window.add_css_class("unim-emoji-popup");

        let vbox = gtk4::Box::new(gtk4::Orientation::Vertical, 6);
        vbox.set_margin_top(8);
        vbox.set_margin_bottom(8);
        vbox.set_margin_start(8);
        vbox.set_margin_end(8);

        // 검색 엔트리
        let search_entry = gtk4::SearchEntry::new();
        search_entry.set_placeholder_text(Some("이모지 검색 (예: 사랑, 동물, 음식)"));
        search_entry.add_css_class("emoji-search");
        vbox.append(&search_entry);

        // 카테고리 탭 전환기 (상단) — 2줄로 분할된 커스텀 ToggleButton bar.
        // GTK4 StackSwitcher는 단일행 전용이라 한자/특수 popup의 9열 통일 디자인을
        // 따르기 위해 ToggleButton을 직접 배치한다.
        let stack = gtk4::Stack::new();
        stack.set_transition_type(gtk4::StackTransitionType::SlideLeftRight);
        stack.set_vexpand(true);

        // 탭 컨테이너 (vertical) — 안에 row 2개 (horizontal)
        let tab_bar = gtk4::Box::new(gtk4::Orientation::Vertical, 2);
        tab_bar.set_halign(gtk4::Align::Center);
        tab_bar.add_css_class("emoji-tabs");

        // 즐겨찾기 + 카테고리 + (검색은 동적으로 추가됨) 합산 후 2줄 분배
        let mut tab_specs: Vec<(String, String)> = Vec::new(); // (key, label)
        tab_specs.push((FAVORITES_KEY.to_string(), "★ 즐겨찾기".to_string()));
        for (id, ko_name, _en_name, _count) in emoji::list_categories() {
            tab_specs.push((id, ko_name));
        }

        let total = tab_specs.len();
        let first_row_size = if total <= TAB_ROW_MAX {
            total
        } else {
            (total + 1) / 2 // 17 → 9, 16 → 8
        };

        let row1 = gtk4::Box::new(gtk4::Orientation::Horizontal, 2);
        row1.set_halign(gtk4::Align::Center);
        row1.add_css_class("emoji-tabs-row");
        tab_bar.append(&row1);
        let row2_opt = if total > first_row_size {
            let row2 = gtk4::Box::new(gtk4::Orientation::Horizontal, 2);
            row2.set_halign(gtk4::Align::Center);
            row2.add_css_class("emoji-tabs-row");
            tab_bar.append(&row2);
            Some(row2)
        } else {
            None
        };

        // 모든 토글 버튼을 그룹으로 묶어서 라디오 동작 (한 번에 하나만 active)
        let tab_buttons: Rc<RefCell<Vec<(String, gtk4::ToggleButton)>>> =
            Rc::new(RefCell::new(Vec::with_capacity(total)));
        let mut group_leader: Option<gtk4::ToggleButton> = None;
        for (i, (key, label)) in tab_specs.iter().enumerate() {
            let btn = gtk4::ToggleButton::with_label(label);
            btn.add_css_class("emoji-tab");
            if let Some(ref leader) = group_leader {
                btn.set_group(Some(leader));
            } else {
                group_leader = Some(btn.clone());
            }
            // 클릭 시 stack 페이지 전환
            let stack_weak = stack.downgrade();
            let key_clone = key.clone();
            btn.connect_toggled(move |b| {
                if !b.is_active() {
                    return;
                }
                if let Some(stack) = stack_weak.upgrade() {
                    stack.set_visible_child_name(&key_clone);
                }
            });

            let target = if i < first_row_size {
                &row1
            } else {
                row2_opt
                    .as_ref()
                    .expect("row2 must exist when i >= first_row_size")
            };
            target.append(&btn);
            tab_buttons.borrow_mut().push((key.clone(), btn));
        }

        // 즐겨찾기 탭을 기본 active
        if let Some((_, btn)) = tab_buttons.borrow().first() {
            btn.set_active(true);
        }

        vbox.append(&tab_bar);

        // 즐겨찾기 페이지
        let favorites_flow = Self::make_flow();
        let favorites_page = Self::wrap_scroll(&favorites_flow);
        stack.add_titled(&favorites_page, Some(FAVORITES_KEY), "★ 즐겨찾기");

        // 카테고리 페이지들
        for (id, ko_name, _en_name, _count) in emoji::list_categories() {
            let flow = Self::make_flow();
            for emoji_str in emoji::category_emojis(&id) {
                flow.insert(&Self::make_emoji_cell(&emoji_str), -1);
            }
            let page = Self::wrap_scroll(&flow);
            stack.add_titled(&page, Some(&id), &ko_name);
        }

        // 검색 결과 페이지 (초기에는 숨김 — 입력 시 추가)
        let search_flow = Self::make_flow();
        let search_page_added = Rc::new(RefCell::new(false));

        vbox.append(&stack);
        window.set_child(Some(&vbox));

        // 즐겨찾기 기본 채우기 (MRU 영속화 미구현 — popular fallback 사용)
        {
            let mru = emoji::load_favorites();
            let items = if mru.is_empty() {
                emoji::search_emoji_strings("")
            } else {
                mru
            };
            Self::fill_flow_with_strings(&favorites_flow, &items);
        }

        // FlowBox 선택(클릭/Enter) → commit
        Self::attach_flow_activation(&favorites_flow);
        // 각 카테고리 FlowBox에도 activation 필요
        let mut child = stack.first_child();
        while let Some(page) = child {
            if let Some(scroll) = page.downcast_ref::<gtk4::ScrolledWindow>() {
                if let Some(fb_widget) = scroll.child() {
                    if let Ok(fb) = fb_widget.downcast::<gtk4::FlowBox>() {
                        Self::attach_flow_activation(&fb);
                    }
                }
            }
            child = page.next_sibling();
        }
        Self::attach_flow_activation(&search_flow);

        // 검색 입력 처리 — 검색 시 stack에 검색 페이지 추가하고 활성화한다.
        // 커스텀 탭바에는 검색 결과용 버튼은 만들지 않는다 (탭 토글이 active=false면 다른 탭으로
        // 자동 전환되지 않도록 하기 위해 검색 시 모든 탭 active 해제).
        let stack_weak = stack.downgrade();
        let search_flow_clone = search_flow.clone();
        let search_added_clone = search_page_added.clone();
        let tab_buttons_search = tab_buttons.clone();
        search_entry.connect_search_changed(move |entry| {
            let keyword = entry.text().to_string();
            if keyword.is_empty() {
                // 검색 페이지 숨기고 즐겨찾기로 복귀
                if let Some(stack) = stack_weak.upgrade() {
                    if *search_added_clone.borrow() {
                        stack.set_visible_child_name(FAVORITES_KEY);
                    }
                }
                // 즐겨찾기 탭 버튼 active
                if let Some((_, btn)) = tab_buttons_search.borrow().first() {
                    btn.set_active(true);
                }
                return;
            }

            let results = emoji::search_emoji(&keyword);
            let strings: Vec<String> = results.iter().map(|c| c.to_string()).collect();
            Self::fill_flow_with_strings(&search_flow_clone, &strings);

            if let Some(stack) = stack_weak.upgrade() {
                if !*search_added_clone.borrow() {
                    let page = Self::wrap_scroll(&search_flow_clone);
                    stack.add_titled(&page, Some(SEARCH_KEY), "🔍 검색");
                    *search_added_clone.borrow_mut() = true;
                }
                stack.set_visible_child_name(SEARCH_KEY);
            }
        });

        // ESC로 닫기
        let key_controller = gtk4::EventControllerKey::new();
        let window_weak = window.downgrade();
        key_controller.connect_key_pressed(move |_, key, _, _| {
            if key == gtk4::gdk::Key::Escape {
                if let Some(w) = window_weak.upgrade() {
                    w.set_visible(false);
                }
                return gtk4::glib::Propagation::Stop;
            }
            gtk4::glib::Propagation::Proceed
        });
        window.add_controller(key_controller);

        // 포커스 상실 시 숨김
        let window_focus_clone = window.clone();
        window.connect_is_active_notify(move |w| {
            if !w.is_active() {
                window_focus_clone.set_visible(false);
            }
        });

        window.update_property(&[gtk4::accessible::Property::Label("이모지 선택")]);

        // 이후 `tab_bar`, `tab_buttons`, `search_flow`, `search_page_added`는 클로저가 소유하므로
        // 구조체 필드로 유지할 필요 없음 (컴파일러가 원본 값을 drop 처리)
        let _ = tab_bar;
        let _ = tab_buttons;
        let _ = search_flow;
        let _ = search_page_added;

        Self {
            window,
            display_server,
            search_entry,
            stack,
            favorites_flow,
        }
    }

    fn make_flow() -> gtk4::FlowBox {
        let flow = gtk4::FlowBox::new();
        flow.set_selection_mode(gtk4::SelectionMode::Single);
        // 한자/특수 popup 9열과 통일. row-major fill (FlowBox 기본) 보장.
        flow.set_max_children_per_line(9);
        flow.set_min_children_per_line(9);
        flow.set_row_spacing(2);
        flow.set_column_spacing(2);
        flow.set_homogeneous(true);
        flow.add_css_class("emoji-flow");
        flow
    }

    fn wrap_scroll(flow: &gtk4::FlowBox) -> gtk4::ScrolledWindow {
        gtk4::ScrolledWindow::builder()
            .hscrollbar_policy(gtk4::PolicyType::Never)
            .vscrollbar_policy(gtk4::PolicyType::Automatic)
            .min_content_height(220)
            .child(flow)
            .build()
    }

    fn make_emoji_cell(emoji_str: &str) -> gtk4::FlowBoxChild {
        let label = gtk4::Label::new(Some(emoji_str));
        label.add_css_class("emoji-cell");
        let child = gtk4::FlowBoxChild::new();
        child.set_child(Some(&label));
        child.set_focusable(true);
        // 이모지 문자열을 tooltip에 박아서 나중에 선택 시 읽어올 수 있도록
        child.set_tooltip_text(Some(emoji_str));
        child
    }

    fn fill_flow_with_strings(flow: &gtk4::FlowBox, items: &[String]) {
        // 기존 자식 제거
        while let Some(child) = flow.first_child() {
            flow.remove(&child);
        }
        if items.is_empty() {
            let label = gtk4::Label::new(Some("(항목 없음)"));
            label.add_css_class("emoji-empty");
            let child = gtk4::FlowBoxChild::new();
            child.set_child(Some(&label));
            child.set_focusable(false);
            flow.insert(&child, -1);
            return;
        }
        for s in items {
            flow.insert(&Self::make_emoji_cell(s), -1);
        }
    }

    fn attach_flow_activation(flow: &gtk4::FlowBox) {
        flow.connect_child_activated(|_, child| {
            // tooltip_text에 저장해둔 이모지 문자열 추출
            let emoji_str = child
                .tooltip_text()
                .map(|s| s.to_string())
                .unwrap_or_default();
            if emoji_str.is_empty() {
                return;
            }
            commit_emoji_via_dbus(emoji_str);
        });
    }

    /// 이모지 팝업 표시
    pub fn show(&mut self, context_path: String, x: i32, y: i32, _w: i32, h: i32) {
        if self.display_server == DisplayServer::GnomeWayland {
            // GNOME Wayland: extension의 EmojiPopup이 팝업 전담
            // (GTK 윈도우가 포커스를 뺏어 FocusOut 유발하므로 전면 스킵)
            return;
        }

        {
            use unim_gui_common::types::ACTIVE_CONTEXT_PATH;
            if let Ok(mut path) = ACTIVE_CONTEXT_PATH.lock() {
                *path = Some(context_path);
            }
        }

        unim_log!(
            "INDICATOR",
            "[EmojiPopup] show() 진입: display_server={:?}, cursor=({},{},{})",
            self.display_server,
            x,
            y,
            h
        );

        // 즐겨찾기 갱신 — MRU 우선, 비어있으면 popular fallback.
        {
            let mru = emoji::load_favorites();
            let items = if mru.is_empty() {
                emoji::search_emoji_strings("")
            } else {
                mru
            };
            Self::fill_flow_with_strings(&self.favorites_flow, &items);
        }
        Self::attach_flow_activation(&self.favorites_flow);

        // 검색 엔트리 비움 + 포커스
        self.search_entry.set_text("");
        self.stack.set_visible_child_name(FAVORITES_KEY);

        popup_positioning::position_popup(&self.window, x, y, h, self.display_server);
        self.window.set_visible(true);
        self.search_entry.grab_focus();
    }

    /// 이모지 팝업 숨김
    pub fn hide(&self) {
        self.window.set_visible(false);
    }
}

/// DBus를 통해 이모지 커밋
fn commit_emoji_via_dbus(emoji_str: String) {
    use unim_gui_common::types::ACTIVE_CONTEXT_PATH;

    let context_path = ACTIVE_CONTEXT_PATH.lock().ok().and_then(|p| p.clone());

    if let Some(path) = context_path {
        unim_log!(
            "INDICATOR",
            "[EmojiPopup] CommitEmoji DBus 호출: emoji='{}', path={}",
            emoji_str,
            path
        );
        std::thread::spawn(move || {
            let rt = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .unwrap();
            rt.block_on(async {
                if let Ok(conn) = zbus::Connection::session().await {
                    let proxy = zbus::Proxy::new(
                        &conn,
                        "org.atit.unim.InputMethod",
                        path.as_str(),
                        "org.atit.unim.InputContext",
                    )
                    .await;
                    if let Ok(proxy) = proxy {
                        let _: Result<(), _> =
                            proxy.call("CommitEmoji", &(emoji_str.as_str(),)).await;
                    }
                }
            });
        });
    } else {
        unim_log!(
            "INDICATOR",
            "[EmojiPopup] CommitEmoji 실패: ACTIVE_CONTEXT_PATH가 비어있음"
        );
    }
}

/// 이모지 팝업 CSS
pub fn popup_css() -> &'static str {
    r#"
    .unim-emoji-popup {
        background-color: rgba(30, 30, 46, 0.97);
        border: 1px solid rgba(255, 255, 255, 0.15);
        border-radius: 12px;
    }

    .unim-emoji-popup .emoji-search {
        background-color: #313244;
        color: #cdd6f4;
        border-radius: 8px;
        padding: 4px 8px;
    }

    .unim-emoji-popup .emoji-tabs {
        padding: 2px 0;
    }

    .unim-emoji-popup .emoji-tabs button {
        min-width: 36px;
        padding: 2px 8px;
        color: #cdd6f4;
        background: transparent;
        border-radius: 6px;
    }

    .unim-emoji-popup .emoji-tabs button:checked {
        background-color: rgba(166, 227, 161, 0.25);
        color: #a6e3a1;
        font-weight: bold;
    }

    .unim-emoji-popup .emoji-flow {
        background: transparent;
        padding: 4px;
    }

    .unim-emoji-popup .emoji-cell {
        font-size: 22px;
        min-width: 32px;
        min-height: 32px;
        padding: 2px;
    }

    .unim-emoji-popup flowboxchild {
        border-radius: 6px;
        padding: 2px;
    }

    .unim-emoji-popup flowboxchild:selected,
    .unim-emoji-popup flowboxchild:focus {
        background-color: rgba(166, 227, 161, 0.25);
    }

    .unim-emoji-popup flowboxchild:hover {
        background-color: rgba(255, 255, 255, 0.08);
    }

    .unim-emoji-popup .emoji-empty {
        color: #6c7086;
        font-style: italic;
        padding: 12px;
    }
    "#
}
