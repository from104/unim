//! 자판 탭 — 5행 stagger 키보드 + 키 셀 클릭 편집.
//!
//! 키 셀을 클릭하면 `dialogs::key_edit` 다이얼로그가 열려 위쪽(Shift)/아래쪽(평문)
//! 라벨을 한 글자씩 편집한다. 편집 후 키보드가 즉시 재구성된다.

use std::cell::RefCell;
use std::rc::Rc;

use gtk4::prelude::*;
use gtk4::{self as gtk};

use unim_keymap_common::keyboard_view::KeyboardView;

use crate::dialogs::key_edit;
use crate::state::SharedAppState;

pub fn build(state: SharedAppState) -> gtk::Widget {
    let keyboard = KeyboardView::new();

    let card = gtk::Frame::builder()
        .css_classes(["studio-keyboard-card"])
        .halign(gtk::Align::Center)
        .margin_top(16)
        .margin_bottom(8)
        .child(keyboard.root())
        .build();

    let hint = gtk::Label::builder()
        .label(rust_i18n::t!("keymap_click_hint"))
        .halign(gtk::Align::Center)
        .css_classes(["dim-label", "caption"])
        .margin_bottom(8)
        .build();

    let root = gtk::Box::new(gtk::Orientation::Vertical, 0);
    root.append(&card);
    root.append(&hint);

    // 키 셀 클릭 → key_edit 다이얼로그.
    {
        let state = state.clone();
        let keyboard_weak = Rc::downgrade(&keyboard);
        let root_weak = root.downgrade();
        keyboard.set_on_select(move |row, col| {
            let Some(root) = root_weak.upgrade() else {
                return;
            };
            let is_korean = state
                .editor
                .borrow()
                .as_ref()
                .map(|e| e.buf.language == "korean")
                .unwrap_or(false);
            let on_done: Rc<dyn Fn()> = {
                let state = state.clone();
                let keyboard_weak = keyboard_weak.clone();
                Rc::new(move || {
                    if let Some(kb) = keyboard_weak.upgrade() {
                        repopulate(&kb, &state);
                    }
                })
            };
            key_edit::open(&root, state.clone(), row, col, is_korean, on_done);
        });
    }

    // refresh 콜백 — 프로필 선택 변경 시 키보드 재구성.
    {
        let state_weak = Rc::downgrade(&state);
        let keyboard = keyboard.clone();
        // 마지막으로 그린 자판 추적 (불필요한 재구성 방지는 생략, 항상 재구성).
        let _last: Rc<RefCell<Option<String>>> = Rc::new(RefCell::new(None));
        state.register_refresh(Rc::new(move || {
            if let Some(state) = state_weak.upgrade() {
                repopulate(&keyboard, &state);
            }
        }));
    }

    root.upcast()
}

/// 현재 editor 버퍼로 키보드 라벨 재구성. 한글 자판이면 ko=buf, en=None(폴백 라벨).
/// 영문 자판이면 en=buf.
fn repopulate(keyboard: &Rc<KeyboardView>, state: &SharedAppState) {
    let editor = state.editor.borrow();
    let Some(ed) = editor.as_ref() else {
        keyboard.populate(None, None);
        return;
    };
    if ed.buf.language == "korean" {
        keyboard.populate(Some(&ed.buf), None);
    } else {
        keyboard.populate(None, Some(&ed.buf));
    }
}
