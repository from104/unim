//! 전역 key_meta 편집 다이얼로그 (Phase F).
//!
//! - 키 식별자 (EntryRow, 한 글자 또는 ASCII 키).
//! - 룰 A: `vowel_combine_head` [미설정 / true / false].
//! - 룰 B: `context_alt` 사용 토글 + when(9종) + to + fallback.

use std::rc::Rc;

use gtk4::prelude::*;
use gtk4::{self as gtk};
use libadwaita::prelude::*;
use libadwaita::{self as adw};

use unim::keystroke::profile::{ContextAlt, ContextCondition, KeyMeta};

use crate::state::SharedAppState;

const WHEN_VARIANTS: [ContextCondition; 9] = [
    ContextCondition::Empty,
    ContextCondition::Composing,
    ContextCondition::ChoseongOnly,
    ContextCondition::JungseongOnly,
    ContextCondition::ChoJungFilled,
    ContextCondition::JongseongFilled,
    ContextCondition::LastIsCho,
    ContextCondition::LastIsJung,
    ContextCondition::LastIsJong,
];

const WHEN_KEYS: [&str; 9] = [
    "key_meta_when_empty",
    "key_meta_when_composing",
    "key_meta_when_choseong_only",
    "key_meta_when_jungseong_only",
    "key_meta_when_cho_jung_filled",
    "key_meta_when_jongseong_filled",
    "key_meta_when_last_is_cho",
    "key_meta_when_last_is_jung",
    "key_meta_when_last_is_jong",
];

fn when_index(c: &ContextCondition) -> u32 {
    WHEN_VARIANTS.iter().position(|v| v == c).unwrap_or(0) as u32
}

/// key_meta 편집 대상 — 전역 또는 특정 rule_set.
pub enum Target {
    Global,
    RuleSet(String),
}

impl Target {
    fn read(&self, state: &SharedAppState, key: &str) -> Option<KeyMeta> {
        let editor = state.editor.borrow();
        let ed = editor.as_ref()?;
        let list = match self {
            Target::Global => ed.key_meta_iter(),
            Target::RuleSet(n) => ed.rule_set_key_meta_iter(n),
        };
        list.into_iter().find(|(k, _)| k == key).map(|(_, m)| m)
    }
}

/// key_meta 편집 다이얼로그. `edit_key` Some 이면 편집, None 이면 추가.
pub fn open(
    parent: &impl IsA<gtk::Widget>,
    state: SharedAppState,
    target: Target,
    edit_key: Option<String>,
    on_done: Rc<dyn Fn()>,
) {
    // 기존 메타 읽기.
    let existing: Option<KeyMeta> =
        edit_key.as_ref().and_then(|k| target.read(&state, k));

    let window = parent.root().and_then(|r| r.downcast::<gtk::Window>().ok());
    let dialog = adw::MessageDialog::new(
        window.as_ref(),
        Some(&rust_i18n::t!("key_meta_dialog_title")),
        None,
    );
    dialog.add_response("cancel", &rust_i18n::t!("btn_cancel"));
    dialog.add_response("ok", &rust_i18n::t!("btn_ok"));
    dialog.set_response_appearance("ok", adw::ResponseAppearance::Suggested);
    dialog.set_default_response(Some("ok"));
    dialog.set_close_response("cancel");

    // 키 식별자.
    let key_entry = adw::EntryRow::builder()
        .title(rust_i18n::t!("key_meta_key_label"))
        .text(edit_key.as_deref().unwrap_or(""))
        .build();

    // 룰 A — vowel_combine_head [미설정/true/false].
    let vch_model = gtk::StringList::new(&[
        &rust_i18n::t!("key_meta_unset"),
        "true",
        "false",
    ]);
    let vch_combo = adw::ComboRow::builder()
        .title(rust_i18n::t!("key_meta_vowel_combine_head"))
        .model(&vch_model)
        .build();
    vch_combo.set_selected(match existing.as_ref().and_then(|m| m.vowel_combine_head) {
        None => 0,
        Some(true) => 1,
        Some(false) => 2,
    });

    let group1 = gtk::ListBox::builder()
        .selection_mode(gtk::SelectionMode::None)
        .css_classes(["boxed-list"])
        .build();
    group1.append(&key_entry);
    group1.append(&vch_combo);

    // 룰 B — context_alt.
    let ca_switch = adw::SwitchRow::builder()
        .title(rust_i18n::t!("key_meta_context_alt_use"))
        .build();
    let when_labels: Vec<String> = WHEN_KEYS
        .iter()
        .map(|k| rust_i18n::t!(*k).to_string())
        .collect();
    let when_refs: Vec<&str> = when_labels.iter().map(|s| s.as_str()).collect();
    let when_model = gtk::StringList::new(&when_refs);
    let when_combo = adw::ComboRow::builder()
        .title(rust_i18n::t!("key_meta_context_when"))
        .model(&when_model)
        .build();
    let to_entry = adw::EntryRow::builder()
        .title(rust_i18n::t!("key_meta_context_to"))
        .build();
    let fallback_entry = adw::EntryRow::builder()
        .title(rust_i18n::t!("key_meta_context_fallback"))
        .build();

    if let Some(ca) = existing.as_ref().and_then(|m| m.context_alt.as_ref()) {
        ca_switch.set_active(true);
        when_combo.set_selected(when_index(&ca.when));
        to_entry.set_text(&ca.to);
        fallback_entry.set_text(&ca.fallback);
    }

    let group2 = gtk::ListBox::builder()
        .selection_mode(gtk::SelectionMode::None)
        .css_classes(["boxed-list"])
        .build();
    group2.append(&ca_switch);
    group2.append(&when_combo);
    group2.append(&to_entry);
    group2.append(&fallback_entry);

    let body = gtk::Box::new(gtk::Orientation::Vertical, 12);
    body.append(&group1);
    body.append(&group2);
    dialog.set_extra_child(Some(&body));

    {
        let state = state.clone();
        let edit_key = edit_key.clone();
        dialog.connect_response(None, move |_, resp| {
            if resp != "ok" {
                return;
            }
            let key = key_entry.text().to_string();
            if key.is_empty() {
                state.toast(&rust_i18n::t!("key_meta_key_required"));
                return;
            }
            let vowel_combine_head = match vch_combo.selected() {
                1 => Some(true),
                2 => Some(false),
                _ => None,
            };
            let context_alt = if ca_switch.is_active() {
                Some(ContextAlt {
                    when: WHEN_VARIANTS[when_combo.selected() as usize].clone(),
                    to: to_entry.text().to_string(),
                    fallback: fallback_entry.text().to_string(),
                })
            } else {
                None
            };
            if let Some(ed) = state.editor.borrow_mut().as_mut() {
                let meta = KeyMeta {
                    vowel_combine_head,
                    context_alt,
                };
                match &target {
                    Target::Global => {
                        if let Some(old) = edit_key.as_ref() {
                            if old != &key {
                                ed.remove_key_meta(old);
                            }
                        }
                        ed.set_key_meta(key, meta);
                    }
                    Target::RuleSet(n) => {
                        if let Some(old) = edit_key.as_ref() {
                            if old != &key {
                                ed.remove_rule_set_key_meta(n, old);
                            }
                        }
                        ed.set_rule_set_key_meta(n, key, meta);
                    }
                }
            }
            on_done();
        });
    }

    dialog.present();
}
