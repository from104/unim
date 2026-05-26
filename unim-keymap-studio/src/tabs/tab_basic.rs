//! 기본 탭 — 언어 · 자판 유형 · 이름/표시 · 메타데이터 · 모아치기.
//!
//! 각 위젯은 `EditorState` setter 에 바인딩된다. 프로필 선택 변경 시 `refresh`
//! 콜백이 위젯 값을 다시 채우며, 이때 `loading` 가드로 change 핸들러의 역기록을
//! 막는다(피드백 루프 방지).

use std::cell::Cell;
use std::rc::Rc;

use gtk4::prelude::*;
use gtk4::{self as gtk};
use libadwaita::prelude::*;
use libadwaita::{self as adw};

use crate::state::editor_state::localized_lang;
use crate::state::{EditorState, SharedAppState};

/// EntryRow 텍스트를 EditorState 에 기록하는 setter.
type TextSetter = Rc<dyn Fn(&mut EditorState, String)>;

pub fn build(state: SharedAppState) -> gtk::Widget {
    let loading = Rc::new(Cell::new(false));

    // 빌트인 안내 배너.
    let banner = adw::Banner::builder()
        .title(rust_i18n::t!("basic_builtin_banner"))
        .revealed(false)
        .build();

    let page = adw::PreferencesPage::new();

    // ── 언어 및 종류 ────────────────────────────────────────────────────
    let group_lang = adw::PreferencesGroup::builder()
        .title(rust_i18n::t!("basic_lang_group"))
        .build();

    let lang_model = gtk::StringList::new(&[
        &rust_i18n::t!("dropdown_lang_korean"),
        &rust_i18n::t!("dropdown_lang_english"),
    ]);
    let lang_combo = adw::ComboRow::builder()
        .title(rust_i18n::t!("basic_lang_label"))
        .model(&lang_model)
        .build();
    group_lang.add(&lang_combo);

    let type_entry = adw::EntryRow::builder()
        .title(rust_i18n::t!("basic_layout_type_label"))
        .build();
    group_lang.add(&type_entry);
    page.add(&group_lang);

    // ── 이름과 표시 ──────────────────────────────────────────────────────
    let group_name = adw::PreferencesGroup::builder()
        .title(rust_i18n::t!("basic_name_group"))
        .build();

    let name_entry = adw::EntryRow::builder()
        .title(rust_i18n::t!("basic_name_label"))
        .build();
    group_name.add(&name_entry);

    let display_ko = adw::EntryRow::builder()
        .title(rust_i18n::t!("basic_display_name_ko"))
        .build();
    group_name.add(&display_ko);

    let display_en = adw::EntryRow::builder()
        .title(rust_i18n::t!("basic_display_name_en"))
        .build();
    group_name.add(&display_en);

    let inherits_entry = adw::EntryRow::builder()
        .title(rust_i18n::t!("basic_inherits_label"))
        .build();
    group_name.add(&inherits_entry);
    page.add(&group_name);

    // ── 메타데이터 ───────────────────────────────────────────────────────
    let group_meta = adw::PreferencesGroup::builder()
        .title(rust_i18n::t!("basic_meta_group"))
        .build();

    let author_entry = adw::EntryRow::builder()
        .title(rust_i18n::t!("meta_author"))
        .build();
    group_meta.add(&author_entry);

    let version_entry = adw::EntryRow::builder()
        .title(rust_i18n::t!("meta_version"))
        .build();
    group_meta.add(&version_entry);

    let license_entry = adw::EntryRow::builder()
        .title(rust_i18n::t!("meta_license"))
        .build();
    group_meta.add(&license_entry);

    let desc_ko = adw::EntryRow::builder()
        .title(rust_i18n::t!("basic_desc_ko"))
        .build();
    group_meta.add(&desc_ko);

    let desc_en = adw::EntryRow::builder()
        .title(rust_i18n::t!("basic_desc_en"))
        .build();
    group_meta.add(&desc_en);

    let tags_entry = adw::EntryRow::builder()
        .title(rust_i18n::t!("basic_tags_label"))
        .build();
    group_meta.add(&tags_entry);
    page.add(&group_meta);

    // ── 고급 (한글 전용) ─────────────────────────────────────────────────
    let group_adv = adw::PreferencesGroup::builder()
        .title(rust_i18n::t!("basic_advanced_group"))
        .build();
    let moachigi_switch = adw::SwitchRow::builder()
        .title(rust_i18n::t!("basic_moachigi_switch"))
        .subtitle(rust_i18n::t!("basic_moachigi_hint"))
        .build();
    group_adv.add(&moachigi_switch);
    page.add(&group_adv);

    // ── 바인딩 헬퍼 ──────────────────────────────────────────────────────
    // EntryRow → setter(&mut EditorState, String).
    let bind_text = {
        let state = state.clone();
        let loading = loading.clone();
        move |row: &adw::EntryRow, setter: TextSetter| {
            let state = state.clone();
            let loading = loading.clone();
            let row_clone = row.clone();
            row.connect_changed(move |_| {
                if loading.get() {
                    return;
                }
                if let Some(ed) = state.editor.borrow_mut().as_mut() {
                    setter(ed, row_clone.text().to_string());
                }
            });
        }
    };

    bind_text(
        &type_entry,
        Rc::new(|ed, v| ed.set_layout_type(&v)),
    );
    bind_text(
        &name_entry,
        Rc::new(|ed, v| ed.rename(v)),
    );
    bind_text(
        &display_ko,
        Rc::new(|ed, v| ed.set_display_name_ko(Some(v))),
    );
    bind_text(
        &display_en,
        Rc::new(|ed, v| ed.set_display_name_en(Some(v))),
    );
    bind_text(
        &inherits_entry,
        Rc::new(|ed, v| ed.set_inherits(Some(v))),
    );
    bind_text(
        &author_entry,
        Rc::new(|ed, v| ed.set_metadata_author(Some(v).filter(|s| !s.is_empty()))),
    );
    bind_text(
        &version_entry,
        Rc::new(|ed, v| ed.set_metadata_version(Some(v).filter(|s| !s.is_empty()))),
    );
    bind_text(
        &license_entry,
        Rc::new(|ed, v| ed.set_license(Some(v))),
    );
    bind_text(
        &desc_ko,
        Rc::new(|ed, v| ed.set_description_ko(Some(v))),
    );
    bind_text(
        &desc_en,
        Rc::new(|ed, v| ed.set_description_en(Some(v))),
    );
    bind_text(
        &tags_entry,
        Rc::new(|ed, v| {
            let tags: Vec<String> = v
                .split(',')
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .collect();
            ed.set_tags(tags);
        }),
    );

    // 언어 콤보 — 한글/영문 전환 + 탭 가시성 통지.
    {
        let state = state.clone();
        let loading = loading.clone();
        lang_combo.connect_selected_notify(move |c| {
            if loading.get() {
                return;
            }
            let is_korean = c.selected() == 0;
            let lang = if is_korean { "korean" } else { "english" };
            if let Some(ed) = state.editor.borrow_mut().as_mut() {
                ed.set_language(lang);
            }
            state.notify_language(is_korean);
        });
    }

    // 모아치기 스위치.
    {
        let state = state.clone();
        let loading = loading.clone();
        moachigi_switch.connect_active_notify(move |s| {
            if loading.get() {
                return;
            }
            if let Some(ed) = state.editor.borrow_mut().as_mut() {
                ed.set_supports_moachigi(s.is_active());
            }
        });
    }

    // ── refresh 콜백 ─────────────────────────────────────────────────────
    {
        let state_weak = Rc::downgrade(&state);
        let loading = loading.clone();
        let banner = banner.clone();
        let lang_combo = lang_combo.clone();
        let type_entry = type_entry.clone();
        let name_entry = name_entry.clone();
        let display_ko = display_ko.clone();
        let display_en = display_en.clone();
        let inherits_entry = inherits_entry.clone();
        let author_entry = author_entry.clone();
        let version_entry = version_entry.clone();
        let license_entry = license_entry.clone();
        let desc_ko = desc_ko.clone();
        let desc_en = desc_en.clone();
        let tags_entry = tags_entry.clone();
        let moachigi_switch = moachigi_switch.clone();
        let group_adv = group_adv.clone();
        state.register_refresh(Rc::new(move || {
            let Some(state) = state_weak.upgrade() else {
                return;
            };
            loading.set(true);

            let editor = state.editor.borrow();
            if let Some(ed) = editor.as_ref() {
                let p = &ed.buf;
                let is_korean = p.language == "korean";
                lang_combo.set_selected(if is_korean { 0 } else { 1 });
                type_entry.set_text(&p.layout_type);
                name_entry.set_text(&p.name);
                display_ko
                    .set_text(&localized_lang(p.metadata.display_name.as_ref(), "ko"));
                display_en
                    .set_text(&localized_lang(p.metadata.display_name.as_ref(), "en"));
                inherits_entry.set_text(p.inherits.as_deref().unwrap_or(""));
                author_entry.set_text(p.metadata.author.as_deref().unwrap_or(""));
                version_entry.set_text(p.metadata.version.as_deref().unwrap_or(""));
                license_entry.set_text(p.metadata.license.as_deref().unwrap_or(""));
                desc_ko.set_text(&localized_lang(p.metadata.description.as_ref(), "ko"));
                desc_en.set_text(&localized_lang(p.metadata.description.as_ref(), "en"));
                tags_entry.set_text(&p.metadata.tags.join(", "));
                moachigi_switch.set_active(p.moachigi.is_some());

                // 빌트인이면 이름 편집 잠금 + 배너.
                let is_builtin = state.is_builtin.get();
                name_entry.set_sensitive(!is_builtin);
                banner.set_revealed(is_builtin);

                // 모아치기는 한글일 때만.
                group_adv.set_visible(is_korean);
            } else {
                banner.set_revealed(false);
            }
            drop(editor);
            loading.set(false);
        }));
    }

    // 최종 레이아웃 — Banner + PreferencesPage.
    let root = gtk::Box::new(gtk::Orientation::Vertical, 0);
    root.append(&banner);
    root.append(&page);
    page.set_vexpand(true);
    root.upcast()
}
