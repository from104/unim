//! 한자 후보 팝업 윈도우
//!
//! DBus `ShowHanjaPopup` 시그널을 수신하여
//! 커서 위치에 한자 후보 리스트를 표시합니다.
//! GTK4+libadwaita 기반, 기존 gtk-common/unim_hanja_popup.c 로직을 Rust로 재구현.

use gtk4::glib;
use gtk4::prelude::*;
use std::cell::RefCell;
use std::rc::Rc;
use unim::unim_log;

/// 페이지당 최대 후보 수
const PAGE_SIZE: usize = 9;

/// 한자 팝업 상태
struct HanjaState {
    /// 변환 대상 문자열
    target: String,
    /// 전체 후보 목록 (한자, 뜻풀이)
    candidates: Vec<(String, String)>,
    /// 현재 페이지 (0-indexed)
    page: usize,
    /// 현재 선택 인덱스 (페이지 내, 0-indexed)
    selection: usize,
}

impl HanjaState {
    fn total_pages(&self) -> usize {
        if self.candidates.is_empty() {
            1
        } else {
            (self.candidates.len() + PAGE_SIZE - 1) / PAGE_SIZE
        }
    }

    fn page_candidates(&self) -> &[(String, String)] {
        let start = self.page * PAGE_SIZE;
        let end = (start + PAGE_SIZE).min(self.candidates.len());
        if start >= self.candidates.len() {
            &[]
        } else {
            &self.candidates[start..end]
        }
    }

    fn selected_global_index(&self) -> usize {
        self.page * PAGE_SIZE + self.selection
    }
}

/// 한자 팝업 컨트롤러
pub struct HanjaPopup {
    window: gtk4::Window,
    list_box: gtk4::Box,
    page_label: gtk4::Label,
    target_label: gtk4::Label,
    state: Rc<RefCell<HanjaState>>,
    /// 선택 결과 콜백 (global index)
    on_select: Rc<RefCell<Option<Box<dyn Fn(usize)>>>>,
    /// 취소 콜백
    on_cancel: Rc<RefCell<Option<Box<dyn Fn()>>>>,
}

impl HanjaPopup {
    pub fn new() -> Self {
        let window = gtk4::Window::builder()
            .decorated(false)
            .resizable(false)
            .modal(false)
            .build();
        window.add_css_class("hanja-popup");

        // X11: override_redirect 설정 (포커스 탈취 방지)
        // Wayland에서는 무시됨
        window.connect_realize(|w| {
            if let Some(surface) = w.surface() {
                if let Some(toplevel) = surface.downcast_ref::<gtk4::gdk::Toplevel>() {
                    // 팝업이 포커스를 가져가지 않도록 설정
                    toplevel.set_decorated(false);
                }
            }
        });

        let main_box = gtk4::Box::new(gtk4::Orientation::Vertical, 0);
        main_box.add_css_class("hanja-container");

        // 타겟 레이블 (변환 대상 문자)
        let target_label = gtk4::Label::builder().halign(gtk4::Align::Start).build();
        target_label.add_css_class("hanja-target");
        main_box.append(&target_label);

        // 후보 리스트
        let list_box = gtk4::Box::new(gtk4::Orientation::Vertical, 0);
        list_box.add_css_class("hanja-list");
        main_box.append(&list_box);

        // 페이지 표시
        let page_label = gtk4::Label::builder().halign(gtk4::Align::Center).build();
        page_label.add_css_class("hanja-page");
        main_box.append(&page_label);

        window.set_child(Some(&main_box));

        let state = Rc::new(RefCell::new(HanjaState {
            target: String::new(),
            candidates: Vec::new(),
            page: 0,
            selection: 0,
        }));

        let popup = Self {
            window,
            list_box,
            page_label,
            target_label,
            state,
            on_select: Rc::new(RefCell::new(None)),
            on_cancel: Rc::new(RefCell::new(None)),
        };

        popup.setup_key_handler();

        // 포커스 잃으면 자동 취소 (Alt+Tab 등으로 다른 앱 전환 시)
        {
            let on_cancel = popup.on_cancel.clone();
            let window = popup.window.clone();
            popup.window.connect_is_active_notify(move |w| {
                if !w.is_active() && w.is_visible() {
                    if let Some(ref cb) = *on_cancel.borrow() {
                        cb();
                    }
                    window.set_visible(false);
                }
            });
        }

        popup
    }

    /// 선택 콜백 설정
    pub fn connect_select<F: Fn(usize) + 'static>(&self, f: F) {
        *self.on_select.borrow_mut() = Some(Box::new(f));
    }

    /// 취소 콜백 설정
    pub fn connect_cancel<F: Fn() + 'static>(&self, f: F) {
        *self.on_cancel.borrow_mut() = Some(Box::new(f));
    }

    /// 팝업 표시
    pub fn show(
        &self,
        target: &str,
        candidates: Vec<(String, String)>,
        cursor_x: i32,
        cursor_y: i32,
        _cursor_width: i32,
        cursor_height: i32,
    ) {
        {
            let mut state = self.state.borrow_mut();
            state.target = target.to_string();
            state.candidates = candidates;
            state.page = 0;
            state.selection = 0;
        }

        self.target_label
            .set_text(&format!("「{}」 → 한자", target));
        self.refresh_list();

        // 팝업 위치 설정 — 커서 아래에 표시
        // 화면 경계 체크는 present() 후에 수행
        let x = cursor_x.max(0);
        let y = (cursor_y + cursor_height).max(0);
        // 팝업 위치 설정 (X11: XMoveWindow, Wayland: no-op)
        crate::popup_position::position_popup_window(&self.window, x, y, 0);
        unim_log!(
            "INDICATOR",
            "한자 팝업 표시: target='{}', cursor=({},{})",
            target,
            cursor_x,
            cursor_y
        );
    }

    /// 팝업 숨김
    pub fn hide(&self) {
        self.window.set_visible(false);
    }

    /// 리스트 새로고침
    fn refresh_list(&self) {
        // 기존 자식 제거
        while let Some(child) = self.list_box.first_child() {
            self.list_box.remove(&child);
        }

        let state = self.state.borrow();
        let page_items = state.page_candidates();
        let selection = state.selection;

        for (i, (hanja, meaning)) in page_items.iter().enumerate() {
            let row = gtk4::Box::new(gtk4::Orientation::Horizontal, 8);
            row.add_css_class("hanja-row");
            if i == selection {
                row.add_css_class("hanja-selected");
            }

            let num = gtk4::Label::builder()
                .label(&format!("{}.", i + 1))
                .width_chars(2)
                .halign(gtk4::Align::End)
                .build();
            num.add_css_class("hanja-num");

            let char_label = gtk4::Label::builder().label(hanja).build();
            char_label.add_css_class("hanja-char");

            let meaning_label = gtk4::Label::builder()
                .label(meaning)
                .hexpand(true)
                .halign(gtk4::Align::Start)
                .ellipsize(gtk4::pango::EllipsizeMode::End)
                .build();
            meaning_label.add_css_class("hanja-meaning");

            row.append(&num);
            row.append(&char_label);
            row.append(&meaning_label);

            // 클릭 이벤트
            let on_select = self.on_select.clone();
            let page = state.page;
            let idx = i;
            let gesture = gtk4::GestureClick::new();
            gesture.connect_released(move |_, _, _, _| {
                let global_idx = page * PAGE_SIZE + idx;
                if let Some(ref cb) = *on_select.borrow() {
                    cb(global_idx);
                }
            });
            row.add_controller(gesture);

            self.list_box.append(&row);
        }

        // 페이지 표시
        let total = state.total_pages();
        if total > 1 {
            self.page_label
                .set_text(&format!("{}/{} (←→ 페이지 전환)", state.page + 1, total));
            self.page_label.set_visible(true);
        } else {
            self.page_label.set_visible(false);
        }
    }

    /// 키 핸들러 설정
    fn setup_key_handler(&self) {
        let state = self.state.clone();
        let on_select = self.on_select.clone();
        let on_cancel = self.on_cancel.clone();
        let list_box = self.list_box.clone();
        let page_label = self.page_label.clone();
        let window = self.window.clone();

        let controller = gtk4::EventControllerKey::new();
        controller.connect_key_pressed(move |_, key, _, _| {
            use gtk4::gdk::Key;

            match key {
                // 숫자 1-9: 직접 선택
                Key::_1
                | Key::_2
                | Key::_3
                | Key::_4
                | Key::_5
                | Key::_6
                | Key::_7
                | Key::_8
                | Key::_9 => {
                    let num = key.to_unicode().unwrap_or('0') as usize - '1' as usize;
                    let global_idx = {
                        let s = state.borrow();
                        if num < s.page_candidates().len() {
                            Some(s.page * PAGE_SIZE + num)
                        } else {
                            None
                        }
                    };
                    if let Some(idx) = global_idx {
                        if let Some(ref cb) = *on_select.borrow() {
                            cb(idx);
                        }
                    }
                    glib::Propagation::Stop
                }

                // Enter / Space: 현재 선택 확정
                Key::Return | Key::KP_Enter | Key::space => {
                    let global_idx = state.borrow().selected_global_index();
                    let valid = {
                        let s = state.borrow();
                        global_idx < s.candidates.len()
                    };
                    if valid {
                        if let Some(ref cb) = *on_select.borrow() {
                            cb(global_idx);
                        }
                    }
                    glib::Propagation::Stop
                }

                // Esc: 취소
                Key::Escape => {
                    if let Some(ref cb) = *on_cancel.borrow() {
                        cb();
                    }
                    window.set_visible(false);
                    glib::Propagation::Stop
                }

                // 위/아래: 선택 이동
                Key::Up | Key::KP_Up => {
                    {
                        let mut s = state.borrow_mut();
                        if s.selection > 0 {
                            s.selection -= 1;
                        } else if s.page > 0 {
                            s.page -= 1;
                            s.selection = PAGE_SIZE - 1;
                        }
                    }
                    refresh_list_static(&state, &list_box, &page_label, &on_select);
                    glib::Propagation::Stop
                }
                Key::Down | Key::KP_Down => {
                    {
                        let mut s = state.borrow_mut();
                        let page_count = s.page_candidates().len();
                        if s.selection + 1 < page_count {
                            s.selection += 1;
                        } else if s.page + 1 < s.total_pages() {
                            s.page += 1;
                            s.selection = 0;
                        }
                    }
                    refresh_list_static(&state, &list_box, &page_label, &on_select);
                    glib::Propagation::Stop
                }

                // 좌/우: 페이지 전환
                Key::Left | Key::KP_Left | Key::Page_Up => {
                    {
                        let mut s = state.borrow_mut();
                        if s.page > 0 {
                            s.page -= 1;
                            s.selection = 0;
                        }
                    }
                    refresh_list_static(&state, &list_box, &page_label, &on_select);
                    glib::Propagation::Stop
                }
                Key::Right | Key::KP_Right | Key::Page_Down => {
                    {
                        let mut s = state.borrow_mut();
                        if s.page + 1 < s.total_pages() {
                            s.page += 1;
                            s.selection = 0;
                        }
                    }
                    refresh_list_static(&state, &list_box, &page_label, &on_select);
                    glib::Propagation::Stop
                }

                _ => glib::Propagation::Proceed,
            }
        });

        self.window.add_controller(controller);
    }
}

/// 정적 함수로 리스트 갱신 (클로저에서 사용)
fn refresh_list_static(
    state: &Rc<RefCell<HanjaState>>,
    list_box: &gtk4::Box,
    page_label: &gtk4::Label,
    on_select: &Rc<RefCell<Option<Box<dyn Fn(usize)>>>>,
) {
    // 기존 자식 제거
    while let Some(child) = list_box.first_child() {
        list_box.remove(&child);
    }

    let s = state.borrow();
    let page_items = s.page_candidates();
    let selection = s.selection;

    for (i, (hanja, meaning)) in page_items.iter().enumerate() {
        let row = gtk4::Box::new(gtk4::Orientation::Horizontal, 8);
        row.add_css_class("hanja-row");
        if i == selection {
            row.add_css_class("hanja-selected");
        }

        let num = gtk4::Label::builder()
            .label(&format!("{}.", i + 1))
            .width_chars(2)
            .halign(gtk4::Align::End)
            .build();
        num.add_css_class("hanja-num");

        let char_label = gtk4::Label::builder().label(hanja).build();
        char_label.add_css_class("hanja-char");

        let meaning_label = gtk4::Label::builder()
            .label(meaning)
            .hexpand(true)
            .halign(gtk4::Align::Start)
            .ellipsize(gtk4::pango::EllipsizeMode::End)
            .build();
        meaning_label.add_css_class("hanja-meaning");

        row.append(&num);
        row.append(&char_label);
        row.append(&meaning_label);

        let on_select_click = on_select.clone();
        let page = s.page;
        let idx = i;
        let gesture = gtk4::GestureClick::new();
        gesture.connect_released(move |_, _, _, _| {
            let global_idx = page * PAGE_SIZE + idx;
            if let Some(ref cb) = *on_select_click.borrow() {
                cb(global_idx);
            }
        });
        row.add_controller(gesture);

        list_box.append(&row);
    }

    let total = s.total_pages();
    if total > 1 {
        page_label.set_text(&format!("{}/{} (←→ 페이지 전환)", s.page + 1, total));
        page_label.set_visible(true);
    } else {
        page_label.set_visible(false);
    }
}
