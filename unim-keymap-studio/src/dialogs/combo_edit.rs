//! 조합 추가/편집 다이얼로그 — 같은 스코프 자모 1 + 1 = 1.
//!
//! 두 대상 지원:
//! - `Target::Base(kind)`   : 기본 조합 블록(cho/jung/jong 분리). 스코프 고정.
//! - `Target::RuleSet(name)`: rule_set 의 flat 조합. 스코프를 사용자가 선택.
//!
//! 저장 문자는 `jamo_catalog::combo_char` 규약(초·중성=호환, 종성=첫가끝).

use std::cell::Cell;
use std::rc::Rc;

use gtk4::prelude::*;
use gtk4::{self as gtk};
use libadwaita::prelude::*;
use libadwaita::{self as adw};

use unim::keystroke::profile::RawTriple;

use crate::helpers::jamo_catalog::{self, Scope};
use crate::state::{ComboKind, SharedAppState};

/// 조합 편집 대상.
pub enum Target {
    Base(ComboKind),
    RuleSet(String),
}

fn scope_of_kind(kind: ComboKind) -> Scope {
    match kind {
        ComboKind::Cho => Scope::Cho,
        ComboKind::Jung => Scope::Jung,
        ComboKind::Jong => Scope::Jong,
    }
}

fn infer_scope(ch: char) -> Option<Scope> {
    [Scope::Cho, Scope::Jung, Scope::Jong]
        .into_iter()
        .find(|&s| jamo_catalog::index_of_combo_char(s, ch).is_some())
}

fn scope_from_index(i: u32) -> Scope {
    match i {
        0 => Scope::Cho,
        1 => Scope::Jung,
        _ => Scope::Jong,
    }
}

fn scope_to_index(s: Scope) -> u32 {
    match s {
        Scope::Cho => 0,
        Scope::Jung => 1,
        Scope::Jong => 2,
    }
}

fn read_existing(state: &SharedAppState, target: &Target, idx: usize) -> Option<RawTriple> {
    let editor = state.editor.borrow();
    let ed = editor.as_ref()?;
    match target {
        Target::Base(k) => ed.combos(*k).get(idx).cloned(),
        Target::RuleSet(n) => ed.rule_set_combos(n).get(idx).cloned(),
    }
}

/// 조합 다이얼로그를 연다. `edit_idx` Some 이면 편집, None 이면 추가.
pub fn open(
    parent: &impl IsA<gtk::Widget>,
    state: SharedAppState,
    target: Target,
    edit_idx: Option<usize>,
    on_done: Rc<dyn Fn()>,
) {
    let is_rule_set = matches!(target, Target::RuleSet(_));
    let fixed_scope = match &target {
        Target::Base(k) => Some(scope_of_kind(*k)),
        Target::RuleSet(_) => None,
    };

    let existing = edit_idx.and_then(|i| read_existing(&state, &target, i));

    let initial_scope = fixed_scope.unwrap_or_else(|| {
        existing
            .as_ref()
            .and_then(|t| t.first.chars().next())
            .and_then(infer_scope)
            .unwrap_or(Scope::Cho)
    });
    let scope = Rc::new(Cell::new(initial_scope));

    let window = parent.root().and_then(|r| r.downcast::<gtk::Window>().ok());
    let heading = if edit_idx.is_some() {
        rust_i18n::t!("combo_dialog_edit_title")
    } else {
        rust_i18n::t!("combo_dialog_add_title")
    };
    let dialog = adw::MessageDialog::new(window.as_ref(), Some(&heading), None);
    dialog.add_response("cancel", &rust_i18n::t!("btn_cancel"));
    dialog.add_response("ok", &rust_i18n::t!("btn_ok"));
    dialog.set_response_appearance("ok", adw::ResponseAppearance::Suggested);
    dialog.set_default_response(Some("ok"));
    dialog.set_close_response("cancel");

    let list = gtk::ListBox::builder()
        .selection_mode(gtk::SelectionMode::None)
        .css_classes(["boxed-list"])
        .build();

    // rule_set 대상이면 스코프 선택 ComboRow.
    let scope_combo = if is_rule_set {
        let model = gtk::StringList::new(&[
            &rust_i18n::t!("key_edit_scope_cho"),
            &rust_i18n::t!("key_edit_scope_jung"),
            &rust_i18n::t!("key_edit_scope_jong"),
        ]);
        let combo = adw::ComboRow::builder()
            .title(rust_i18n::t!("key_edit_scope_label"))
            .model(&model)
            .build();
        combo.set_selected(scope_to_index(initial_scope));
        list.append(&combo);
        Some(combo)
    } else {
        None
    };

    let make_dd = |scope: Scope| {
        let dd = gtk::DropDown::new(
            Some(jamo_catalog::combo_string_list(scope)),
            None::<gtk::Expression>,
        );
        dd.set_valign(gtk::Align::Center);
        dd
    };
    let dd_first = make_dd(initial_scope);
    let dd_second = make_dd(initial_scope);
    let dd_result = make_dd(initial_scope);

    let preselect = |t: &RawTriple, scope: Scope, a: &gtk::DropDown, b: &gtk::DropDown, c: &gtk::DropDown| {
        if let Some(ch) = t.first.chars().next() {
            if let Some(i) = jamo_catalog::index_of_combo_char(scope, ch) {
                a.set_selected(i);
            }
        }
        if let Some(ch) = t.second.chars().next() {
            if let Some(i) = jamo_catalog::index_of_combo_char(scope, ch) {
                b.set_selected(i);
            }
        }
        if let Some(ch) = t.result.chars().next() {
            if let Some(i) = jamo_catalog::index_of_combo_char(scope, ch) {
                c.set_selected(i);
            }
        }
    };
    if let Some(t) = &existing {
        preselect(t, initial_scope, &dd_first, &dd_second, &dd_result);
    }

    let row_first = adw::ActionRow::builder()
        .title(rust_i18n::t!("combo_dialog_first"))
        .build();
    row_first.add_suffix(&dd_first);
    let row_second = adw::ActionRow::builder()
        .title(rust_i18n::t!("combo_dialog_second"))
        .build();
    row_second.add_suffix(&dd_second);
    let row_result = adw::ActionRow::builder()
        .title(rust_i18n::t!("combo_dialog_result"))
        .build();
    row_result.add_suffix(&dd_result);
    list.append(&row_first);
    list.append(&row_second);
    list.append(&row_result);

    // 스코프 변경 → 자모 드롭다운 모델 교체.
    if let Some(combo) = &scope_combo {
        let scope = scope.clone();
        let dd_first = dd_first.clone();
        let dd_second = dd_second.clone();
        let dd_result = dd_result.clone();
        combo.connect_selected_notify(move |c| {
            let s = scope_from_index(c.selected());
            scope.set(s);
            dd_first.set_model(Some(&jamo_catalog::combo_string_list(s)));
            dd_second.set_model(Some(&jamo_catalog::combo_string_list(s)));
            dd_result.set_model(Some(&jamo_catalog::combo_string_list(s)));
        });
    }

    let hint = gtk::Label::builder()
        .label(rust_i18n::t!("combo_dialog_scope_hint"))
        .halign(gtk::Align::Start)
        .css_classes(["caption", "dim-label"])
        .margin_top(4)
        .build();

    let body = gtk::Box::new(gtk::Orientation::Vertical, 8);
    body.append(&list);
    body.append(&hint);
    dialog.set_extra_child(Some(&body));

    {
        let state = state.clone();
        let scope = scope.clone();
        dialog.connect_response(None, move |_, resp| {
            if resp != "ok" {
                return;
            }
            let s = scope.get();
            let pick = |dd: &gtk::DropDown| -> Option<char> {
                jamo_catalog::entry_at(s, dd.selected()).map(|e| jamo_catalog::combo_char(s, &e))
            };
            let (Some(f), Some(sec), Some(r)) =
                (pick(&dd_first), pick(&dd_second), pick(&dd_result))
            else {
                return;
            };
            let triple = RawTriple {
                first: f.to_string(),
                second: sec.to_string(),
                result: r.to_string(),
            };
            if let Some(ed) = state.editor.borrow_mut().as_mut() {
                match (&target, edit_idx) {
                    (Target::Base(k), Some(i)) => {
                        ed.update_combo(*k, i, triple);
                    }
                    (Target::Base(k), None) => ed.push_combo(*k, triple),
                    (Target::RuleSet(n), Some(i)) => {
                        ed.update_rule_set_combo(n, i, triple);
                    }
                    (Target::RuleSet(n), None) => ed.push_rule_set_combo(n, triple),
                }
            }
            on_done();
        });
    }

    dialog.present();
}
