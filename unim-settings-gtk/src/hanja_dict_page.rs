//! 한자 사용자 사전 설정 페이지 (HANJA_USERDICT_SPEC §6·§8).
//!
//! - 파일(`<config_dir>/unim/hanja-userdict.yaml`)을 직접 읽고 쓴다. 엔진은 mtime 으로 감지한다.
//! - 모든 쓰기는 **디스크를 다시 읽은 뒤** `(한글, 한자)` 쌍으로 항목을 찾아 수행한다(인덱스 금지).
//! - 읽을 수 없는 파일은 절대 덮어쓰지 않는다 — 오류 행 + [보관하고 새로 시작] 만 제공.
//! - 로그에는 항목 내용을 남기지 않는다(`io::ErrorKind` 만).

use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::time::SystemTime;

use gtk4::gdk;
use gtk4::glib;
use gtk4::prelude::*;
use libadwaita as adw;
use libadwaita::prelude::*;
use rust_i18n::t;

use unim::hanja::user_dict::{
    file_signature, EntryError, HanjaUserDict, HanjaUserEntry, LoadError, Signature,
};
use unim::input_engine::HANJA_MAX_KEY_CHARS;
use unim::unim_log;

/// 삭제 되돌리기 토스트 유지 시간(초).
const UNDO_TIMEOUT_SECS: u32 = 5;
/// 짧은 안내 토스트 유지 시간(초).
const NOTE_TIMEOUT_SECS: u32 = 2;

struct Refs {
    window: adw::PreferencesWindow,
    page: adw::PreferencesPage,
    group: adw::PreferencesGroup,
    add_btn: gtk4::Button,
    /// 그룹에 들어 있는 모든 행(항목·빈 상태·오류 행 포함) — 재구성 때 제거용.
    all_rows: RefCell<Vec<adw::ActionRow>>,
    /// 항목 행만(목록 순서). 포커스 복원용.
    entry_rows: RefCell<Vec<adw::ActionRow>>,
    /// 마지막으로 반영한 파일 mtime(없으면 `None`).
    last_mtime: Cell<Option<SystemTime>>,
    /// 가장 최근 삭제 되돌리기 토스트.
    undo_toast: RefCell<Option<adw::Toast>>,
}

// ─────────────────────────────────────────────────────────────
// 공용 도우미
// ─────────────────────────────────────────────────────────────

fn file_mtime() -> Option<SystemTime> {
    let path = HanjaUserDict::default_path()?;
    std::fs::metadata(path).ok()?.modified().ok()
}

fn esc(s: &str) -> String {
    glib::markup_escape_text(s).to_string()
}

fn note(refs: &Refs, text: &str) {
    refs.window.add_toast(
        adw::Toast::builder()
            .title(text)
            .timeout(NOTE_TIMEOUT_SECS)
            .build(),
    );
}

fn log_save_failed(e: &std::io::Error) {
    // 항목 내용·serde 문장 금지 — 종류만.
    unim_log!(
        "INDICATOR",
        "[Settings] hanja-userdict 저장 실패: {:?}",
        e.kind()
    );
}

fn note_save_failed(refs: &Refs, e: &std::io::Error) {
    log_save_failed(e);
    note(
        refs,
        &t!(
            "settings_toast_save_failed",
            err = format!("{:?}", e.kind())
        ),
    );
}

/// 검증 오류를 사람이 읽는 문장으로. `pos` 는 0 기준이므로 1 기준으로 바꿔 보인다.
fn err_text(e: &EntryError) -> String {
    match e {
        EntryError::HangulEmpty => t!("hanja_dict_err_hangul_empty").to_string(),
        EntryError::HangulNotSyllable { ch, pos } => t!(
            "hanja_dict_err_hangul_not_syllable",
            pos = (pos + 1).to_string(),
            ch = ch.to_string()
        )
        .to_string(),
        EntryError::HangulTooLong { len } => t!(
            "hanja_dict_err_hangul_too_long",
            len = len.to_string(),
            max = HANJA_MAX_KEY_CHARS.to_string()
        )
        .to_string(),
        EntryError::HanjaLenMismatch { hangul, hanja } => t!(
            "hanja_dict_err_len_mismatch",
            hangul = hangul.to_string(),
            hanja = hanja.to_string()
        )
        .to_string(),
        EntryError::HanjaNotHan { ch, pos } => t!(
            "hanja_dict_err_hanja_not_han",
            pos = (pos + 1).to_string(),
            ch = ch.to_string()
        )
        .to_string(),
        EntryError::MeaningTooLong { len } => t!(
            "hanja_dict_err_meaning_too_long",
            len = len.to_string(),
            max = unim::hanja::user_dict::MEANING_MAX_CHARS.to_string()
        )
        .to_string(),
        EntryError::MeaningControl => t!("hanja_dict_err_meaning_control").to_string(),
        EntryError::Duplicate => t!("hanja_dict_err_duplicate").to_string(),
        EntryError::NotFound => t!("hanja_dict_err_not_found").to_string(),
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Field {
    Hangul,
    Hanja,
    Meaning,
}

fn error_field(e: &EntryError) -> Field {
    match e {
        EntryError::HangulEmpty
        | EntryError::HangulNotSyllable { .. }
        | EntryError::HangulTooLong { .. } => Field::Hangul,
        EntryError::MeaningTooLong { .. } | EntryError::MeaningControl => Field::Meaning,
        _ => Field::Hanja,
    }
}

fn find_scrolled(w: &gtk4::Widget) -> Option<gtk4::ScrolledWindow> {
    if let Some(sw) = w.downcast_ref::<gtk4::ScrolledWindow>() {
        return Some(sw.clone());
    }
    let mut child = w.first_child();
    while let Some(c) = child {
        if let Some(found) = find_scrolled(&c) {
            return Some(found);
        }
        child = c.next_sibling();
    }
    None
}

/// 행이 그려질 때의 스냅샷과 완전히 같은 항목의 현재 위치.
/// 표시 위치가 그대로 맞으면 그 자리, 아니면 같은 항목 중 마지막. 없으면 `None`.
fn locate(dict: &HanjaUserDict, snap: &HanjaUserEntry, shown_idx: usize) -> Option<usize> {
    if dict.entries.get(shown_idx) == Some(snap) {
        return Some(shown_idx);
    }
    dict.entries.iter().rposition(|e| e == snap)
}

/// 지금 키보드 포커스가 놓인 항목 행의 위치.
fn focused_entry_index(refs: &Refs) -> Option<usize> {
    let focus = gtk4::prelude::GtkWindowExt::focus(&refs.window)?;
    refs.entry_rows.borrow().iter().position(|row| {
        let w: &gtk4::Widget = row.upcast_ref();
        focus == *w || focus.is_ancestor(w)
    })
}

// ─────────────────────────────────────────────────────────────
// 페이지
// ─────────────────────────────────────────────────────────────

pub fn build_hanja_dict_page(window: &adw::PreferencesWindow) -> adw::PreferencesPage {
    let page = adw::PreferencesPage::builder()
        .title(t!("page_hanja_dict_title"))
        .icon_name("font-x-generic-symbolic")
        .build();

    let group = adw::PreferencesGroup::builder()
        .title(t!("hanja_dict_group_title"))
        .build();

    let add_btn = gtk4::Button::builder()
        .icon_name("list-add-symbolic")
        .tooltip_text(t!("hanja_dict_btn_add_tooltip"))
        .valign(gtk4::Align::Center)
        .css_classes(vec!["flat"])
        .build();
    add_btn.update_property(&[gtk4::accessible::Property::Label(&t!(
        "hanja_dict_btn_add_tooltip"
    ))]);
    group.set_header_suffix(Some(&add_btn));
    page.add(&group);

    let refs = Rc::new(Refs {
        window: window.clone(),
        page: page.clone(),
        group,
        add_btn: add_btn.clone(),
        all_rows: RefCell::new(Vec::new()),
        entry_rows: RefCell::new(Vec::new()),
        last_mtime: Cell::new(None),
        undo_toast: RefCell::new(None),
    });

    {
        let refs_c = refs.clone();
        add_btn.connect_clicked(move |_| show_edit_dialog(&refs_c, None));
    }

    refill(&refs, None);

    // mtime 폴링 — CLI·다른 설정 창이 파일을 바꾸면 2초 안에 반영.
    let weak = Rc::downgrade(&refs);
    glib::timeout_add_seconds_local(2, move || match weak.upgrade() {
        Some(refs) => {
            if file_mtime() != refs.last_mtime.get() {
                refill(&refs, None);
            }
            glib::ControlFlow::Continue
        }
        None => glib::ControlFlow::Break,
    });

    page
}

/// 목록을 다시 그린다. 스크롤 위치를 보존하고, 포커스는 `focus_idx`(없으면 지금 포커스가
/// 있던 항목 위치)의 행으로 되돌린다.
fn refill(refs: &Rc<Refs>, focus_idx: Option<usize>) {
    let page_w: &gtk4::Widget = refs.page.upcast_ref();
    let scroll_value = find_scrolled(page_w).map(|sw| {
        let adj = sw.vadjustment();
        (adj.clone(), adj.value())
    });
    let explicit_focus = focus_idx.is_some();
    let want_focus = focus_idx.or_else(|| focused_entry_index(refs));

    for row in refs.all_rows.borrow().iter() {
        refs.group.remove(row);
    }
    refs.all_rows.borrow_mut().clear();
    refs.entry_rows.borrow_mut().clear();

    // 읽기 전에 mtime 을 기록 — 그 사이 쓰기가 있으면 다음 폴링이 다시 그린다.
    refs.last_mtime.set(file_mtime());

    match HanjaUserDict::load_default() {
        Ok(dict) => render_list(refs, &dict),
        Err(e) => render_error(refs, &e),
    }

    // 레이아웃이 끝난 뒤 스크롤 → 포커스 순서로 복원.
    let refs_c = refs.clone();
    glib::idle_add_local_once(move || {
        if let Some((adj, value)) = scroll_value {
            adj.set_value(value);
        }
        let Some(idx) = want_focus else { return };
        let rows = refs_c.entry_rows.borrow();
        if let Some(last) = rows.len().checked_sub(1) {
            rows[idx.min(last)].grab_focus();
        } else if explicit_focus {
            refs_c.add_btn.grab_focus();
        }
    });
}

fn push_row(refs: &Refs, row: &adw::ActionRow, is_entry: bool) {
    refs.group.add(row);
    refs.all_rows.borrow_mut().push(row.clone());
    if is_entry {
        refs.entry_rows.borrow_mut().push(row.clone());
    }
}

fn render_list(refs: &Rc<Refs>, dict: &HanjaUserDict) {
    refs.add_btn.set_sensitive(true);
    let total = dict.entries.len();
    let invalid = (0..total)
        .filter(|&i| dict.entry_error(i).is_some())
        .count();

    let desc = if invalid == 0 {
        t!("hanja_dict_group_desc_count", count = total.to_string())
    } else {
        t!(
            "hanja_dict_group_desc_invalid",
            count = total.to_string(),
            invalid = invalid.to_string()
        )
    };
    refs.group.set_description(Some(desc.as_ref()));

    if dict.entries.is_empty() {
        let row = adw::ActionRow::builder()
            .title(t!("hanja_dict_empty"))
            .subtitle(t!("hanja_dict_empty_subtitle"))
            .sensitive(false)
            .build();
        push_row(refs, &row, false);
        return;
    }

    for (idx, entry) in dict.entries.iter().enumerate() {
        let subtitle = match dict.entry_error(idx) {
            Some(err) => t!("hanja_dict_invalid_prefix", reason = err_text(&err)).to_string(),
            None => match entry.meaning.as_deref().filter(|m| !m.is_empty()) {
                Some(m) => t!("hanja_dict_meaning_fmt", meaning = m).to_string(),
                None => t!("hanja_dict_meaning_auto").to_string(),
            },
        };
        let row = adw::ActionRow::builder()
            .title(esc(&format!("{} → {}", entry.hangul, entry.hanja)))
            .subtitle(esc(&subtitle))
            .activatable(true)
            .build();

        // 클릭한 행 전체 = 편집. 오른쪽 연필은 시각 단서일 뿐이다.
        let hint = gtk4::Image::from_icon_name("document-edit-symbolic");
        hint.add_css_class("dim-label");
        row.add_suffix(&hint);

        let del_btn = gtk4::Button::builder()
            .icon_name("user-trash-symbolic")
            .tooltip_text(t!("hanja_dict_btn_delete"))
            .valign(gtk4::Align::Center)
            .css_classes(vec!["flat"])
            .build();
        del_btn.update_property(&[gtk4::accessible::Property::Label(&t!(
            "hanja_dict_btn_delete"
        ))]);
        row.add_suffix(&del_btn);

        {
            let refs_c = refs.clone();
            let snap = entry.clone();
            row.connect_activated(move |_| {
                show_edit_dialog(&refs_c, Some((snap.clone(), idx)));
            });
        }
        {
            let refs_c = refs.clone();
            let snap = entry.clone();
            del_btn.connect_clicked(move |_| delete_entry(&refs_c, &snap, idx));
        }

        push_row(refs, &row, true);
    }
}

/// 읽을 수 없는 파일: 오류 행(+ 보관 버튼)만 보이고 추가는 막는다.
fn render_error(refs: &Rc<Refs>, err: &LoadError) {
    refs.add_btn.set_sensitive(false);
    refs.group
        .set_description(Some(t!("hanja_dict_corrupt_desc").as_ref()));

    let path = HanjaUserDict::default_path()
        .map(|p| p.display().to_string())
        .unwrap_or_default();
    let (title, reason) = match err {
        LoadError::NewerVersion(v) => (
            t!("hanja_dict_newer_version"),
            t!("hanja_dict_newer_version_reason", version = v.to_string()).to_string(),
        ),
        other => (t!("hanja_dict_corrupt"), other.to_string()),
    };
    let row = adw::ActionRow::builder()
        .title(esc(&title))
        .subtitle(esc(&format!("{reason}\n{path}")))
        .build();
    row.add_css_class("error");

    // 보관(이름 바꾸기)은 문법이 깨진 파일에만 — 새 버전 파일은 건드리지 않는다.
    if matches!(err, LoadError::Parse { .. }) {
        let btn = gtk4::Button::builder()
            .label(t!("hanja_dict_btn_quarantine"))
            .valign(gtk4::Align::Center)
            .css_classes(vec!["suggested-action"])
            .build();
        let refs_c = refs.clone();
        btn.connect_clicked(move |_| quarantine(&refs_c));
        row.add_suffix(&btn);
    }
    push_row(refs, &row, false);
}

fn quarantine(refs: &Rc<Refs>) {
    let Some(path) = HanjaUserDict::default_path() else {
        return;
    };
    match HanjaUserDict::quarantine(&path) {
        Ok(_) => note(refs, &t!("hanja_dict_quarantined")),
        Err(e) => note_save_failed(refs, &e),
    }
    refill(refs, None);
}

// ─────────────────────────────────────────────────────────────
// 삭제 + 되돌리기
// ─────────────────────────────────────────────────────────────

fn delete_entry(refs: &Rc<Refs>, snap: &HanjaUserEntry, shown_idx: usize) {
    let mut dict = match HanjaUserDict::load_default() {
        Ok(d) => d,
        Err(_) => {
            refill(refs, None);
            return;
        }
    };
    let Some(idx) = locate(&dict, snap, shown_idx) else {
        // 그 사이 파일이 바뀌어 이 행을 못 찾는다 — 엉뚱한 항목을 지우지 않고 목록만 새로 고친다.
        note(refs, &t!("hanja_dict_err_changed"));
        refill(refs, None);
        return;
    };
    let Some(entry) = dict.remove_at(idx) else {
        return;
    };
    let len_after = dict.entries.len();
    if let Err(e) = dict.save_default() {
        note_save_failed(refs, &e);
        return;
    }
    // 삭제 직후 파일 서명 — 되돌리기 때 "그 사이 바뀌었나" 판단 기준.
    let sig_after = HanjaUserDict::default_path().and_then(|p| file_signature(&p));

    // 같은 자리(마지막이면 앞) 행에 포커스, 스크롤 유지.
    refill(refs, Some(idx.min(len_after.saturating_sub(1))));

    let toast = adw::Toast::builder()
        .title(t!("hanja_dict_toast_deleted"))
        .button_label(t!("toast_undo"))
        .timeout(UNDO_TIMEOUT_SECS)
        .build();
    {
        let refs_c = refs.clone();
        toast.connect_button_clicked(move |_| undo_delete(&refs_c, &entry, idx, sig_after));
    }
    // 앞선 되돌리기 토스트는 걷어 새 토스트가 곧바로 보이게 한다.
    if let Some(prev) = refs.undo_toast.replace(Some(toast.clone())) {
        prev.dismiss();
    }
    refs.window.add_toast(toast);
}

fn undo_delete(refs: &Rc<Refs>, entry: &HanjaUserEntry, idx: usize, sig_after: Option<Signature>) {
    let mut dict = match HanjaUserDict::load_default() {
        Ok(d) => d,
        Err(_) => {
            refill(refs, None);
            return;
        }
    };
    // 그동안 파일이 바뀌지 않았으면 원래 자리, 바뀌었으면(그리고 완전히 같은 항목이
    // 이미 다시 들어와 있지 않으면) 맨 끝.
    let unchanged = sig_after.is_some()
        && HanjaUserDict::default_path().and_then(|p| file_signature(&p)) == sig_after;
    let pos = if unchanged {
        idx.min(dict.entries.len())
    } else if dict.entries.iter().any(|e| e == entry) {
        refill(refs, None);
        return;
    } else {
        dict.entries.len()
    };
    dict.entries.insert(pos, entry.clone());
    if let Err(e) = dict.save_default() {
        note_save_failed(refs, &e);
        return;
    }
    note(refs, &t!("hanja_dict_toast_restored"));
    refill(refs, Some(pos));
}

// ─────────────────────────────────────────────────────────────
// 추가/편집 대화상자
// ─────────────────────────────────────────────────────────────

/// 칸 + 그 아래 안내/오류 라벨.
fn field_block(row: &adw::EntryRow) -> (gtk4::Box, gtk4::Label) {
    let group = adw::PreferencesGroup::new();
    group.add(row);
    let label = gtk4::Label::builder()
        .xalign(0.0)
        .wrap(true)
        .margin_start(12)
        .margin_end(12)
        .css_classes(vec!["caption"])
        .build();
    let b = gtk4::Box::new(gtk4::Orientation::Vertical, 6);
    b.append(&group);
    b.append(&label);
    (b, label)
}

fn set_msg(label: &gtk4::Label, row: &adw::EntryRow, text: Option<&str>, is_error: bool) {
    match text {
        Some(t) => {
            label.set_label(t);
            label.set_visible(true);
        }
        None => {
            label.set_label("");
            label.set_visible(false);
        }
    }
    if is_error {
        label.remove_css_class("dim-label");
        label.add_css_class("error");
        row.add_css_class("error");
    } else {
        label.remove_css_class("error");
        label.add_css_class("dim-label");
        row.remove_css_class("error");
    }
}

/// `original = None` 이면 추가, `Some((행 스냅샷, 표시 위치))` 면 그 항목 편집.
fn show_edit_dialog(refs: &Rc<Refs>, original: Option<(HanjaUserEntry, usize)>) {
    // 대화상자를 열기 직전에 디스크를 다시 읽는다. 읽을 수 없으면 열지 않는다.
    let (init_hangul, init_hanja, init_meaning) = match HanjaUserDict::load_default() {
        Err(_) => {
            refill(refs, None);
            return;
        }
        Ok(d) => match &original {
            None => (String::new(), String::new(), String::new()),
            Some((snap, shown)) => match locate(&d, snap, *shown) {
                Some(i) => {
                    let e = &d.entries[i];
                    (
                        e.hangul.clone(),
                        e.hanja.clone(),
                        e.meaning.clone().unwrap_or_default(),
                    )
                }
                None => {
                    note(refs, &t!("hanja_dict_err_not_found"));
                    refill(refs, None);
                    return;
                }
            },
        },
    };
    let is_edit = original.is_some();

    let dialog = adw::Window::builder()
        .transient_for(&refs.window)
        .modal(true)
        .title(
            if is_edit {
                t!("hanja_dict_dialog_edit_title")
            } else {
                t!("hanja_dict_dialog_add_title")
            }
            .to_string(),
        )
        .default_width(460)
        .build();

    let toolbar = adw::ToolbarView::new();
    toolbar.add_top_bar(&adw::HeaderBar::new());

    let content = gtk4::Box::new(gtk4::Orientation::Vertical, 14);
    content.set_margin_top(18);
    content.set_margin_bottom(18);
    content.set_margin_start(18);
    content.set_margin_end(18);

    let hangul_row = adw::EntryRow::builder()
        .title(t!("hanja_dict_field_hangul"))
        .text(&init_hangul)
        .build();
    let hanja_row = adw::EntryRow::builder()
        .title(t!("hanja_dict_field_hanja"))
        .text(&init_hanja)
        .build();
    let meaning_row = adw::EntryRow::builder()
        .title(t!("hanja_dict_field_meaning"))
        .text(&init_meaning)
        .build();

    let (hangul_block, hangul_msg) = field_block(&hangul_row);
    let (hanja_block, hanja_msg) = field_block(&hanja_row);
    let (meaning_block, meaning_msg) = field_block(&meaning_row);
    content.append(&hangul_block);
    content.append(&hanja_block);
    content.append(&meaning_block);

    let btn_box = gtk4::Box::new(gtk4::Orientation::Horizontal, 8);
    btn_box.set_halign(gtk4::Align::End);
    let cancel_btn = gtk4::Button::with_label(&t!("common_cancel"));
    let save_btn = gtk4::Button::with_label(
        if is_edit {
            t!("common_save")
        } else {
            t!("common_add")
        }
        .as_ref(),
    );
    save_btn.add_css_class("suggested-action");
    save_btn.set_sensitive(false);
    btn_box.append(&cancel_btn);
    btn_box.append(&save_btn);
    content.append(&btn_box);

    toolbar.set_content(Some(&content));
    dialog.set_content(Some(&toolbar));

    // ── 실시간 검증 ──────────────────────────────────────────
    let hint = t!("hanja_dict_field_hanja_hint").to_string();
    let refresh: Rc<dyn Fn()> = {
        let (hangul_row, hanja_row, meaning_row) =
            (hangul_row.clone(), hanja_row.clone(), meaning_row.clone());
        let (hangul_msg, hanja_msg, meaning_msg) =
            (hangul_msg.clone(), hanja_msg.clone(), meaning_msg.clone());
        let save_btn = save_btn.clone();
        let original = original.clone();
        Rc::new(move || {
            let h = hangul_row.text().to_string();
            let j = hanja_row.text().to_string();
            let m = meaning_row.text().to_string();

            let mut error: Option<EntryError> = None;
            let mut pending = false; // 아직 다 안 썼을 뿐 — 오류로 겁주지 않는다.
            match HanjaUserDict::validate(&h, &j, Some(&m)) {
                Ok((vh, vj, _)) => match HanjaUserDict::load_default() {
                    Ok(d) => {
                        let own = original
                            .as_ref()
                            .map(|(snap, _)| snap.hangul == vh && snap.hanja == vj)
                            .unwrap_or(false);
                        if d.find(&vh, &vj).is_some() && !own {
                            error = Some(EntryError::Duplicate);
                        }
                    }
                    Err(_) => {
                        // 파일이 그 사이 읽을 수 없게 됐다 — 저장 불가.
                        pending = true;
                    }
                },
                Err(EntryError::HangulEmpty) => pending = true,
                Err(EntryError::HanjaLenMismatch { hanja: 0, .. }) => pending = true,
                Err(e) => error = Some(e),
            }

            let field = error.as_ref().map(error_field);
            let text = error.as_ref().map(err_text);
            let show = |f: Field| -> Option<&str> {
                if field == Some(f) {
                    text.as_deref()
                } else {
                    None
                }
            };
            set_msg(
                &hangul_msg,
                &hangul_row,
                show(Field::Hangul),
                show(Field::Hangul).is_some(),
            );
            // 한자 칸은 오류가 없을 때 입력 안내를 보인다.
            match show(Field::Hanja) {
                Some(t) => set_msg(&hanja_msg, &hanja_row, Some(t), true),
                None => set_msg(&hanja_msg, &hanja_row, Some(&hint), false),
            }
            set_msg(
                &meaning_msg,
                &meaning_row,
                show(Field::Meaning),
                show(Field::Meaning).is_some(),
            );

            save_btn.set_sensitive(error.is_none() && !pending);
        })
    };
    for row in [&hangul_row, &hanja_row, &meaning_row] {
        let r = refresh.clone();
        row.connect_changed(move |_| r());
    }
    refresh();

    // ── 저장 ────────────────────────────────────────────────
    let do_save: Rc<dyn Fn()> = {
        let refs = refs.clone();
        let dialog = dialog.downgrade();
        let (hangul_row, hanja_row, meaning_row) =
            (hangul_row.clone(), hanja_row.clone(), meaning_row.clone());
        let (hanja_msg, save_btn) = (hanja_msg.clone(), save_btn.clone());
        let original = original.clone();
        Rc::new(move || {
            if !save_btn.is_sensitive() {
                return;
            }
            let h = hangul_row.text().to_string();
            let j = hanja_row.text().to_string();
            let m = meaning_row.text().to_string();
            let meaning = (!m.trim().is_empty()).then_some(m);

            // 쓰기마다 디스크를 다시 읽고, 인덱스가 아니라 쌍으로 찾는다.
            let mut dict = match HanjaUserDict::load_default() {
                Ok(d) => d,
                Err(_) => {
                    if let Some(d) = dialog.upgrade() {
                        d.close();
                    }
                    refill(&refs, None);
                    return;
                }
            };
            let res: Result<usize, EntryError> = match &original {
                None => dict.add(&h, &j, meaning),
                Some((snap, shown)) => match locate(&dict, snap, *shown) {
                    Some(i) => dict.update_at(i, &h, &j, meaning).map(|_| i),
                    None => Err(EntryError::NotFound),
                },
            };
            let pos = match res {
                Ok(p) => p,
                Err(e) => {
                    set_msg(&hanja_msg, &hanja_row, Some(&err_text(&e)), true);
                    return;
                }
            };
            if let Err(e) = dict.save_default() {
                note_save_failed(&refs, &e);
                return;
            }
            note(
                &refs,
                &if original.is_some() {
                    t!("settings_toast_saved")
                } else {
                    t!("settings_toast_added")
                },
            );
            if let Some(d) = dialog.upgrade() {
                d.close();
            }
            // 고치거나 새로 넣은 행으로 포커스를 돌려보낸다.
            refill(&refs, Some(pos));
        })
    };
    {
        let f = do_save.clone();
        save_btn.connect_clicked(move |_| f());
    }
    // Enter: 한글 → 한자 → (유효하면) 저장.
    {
        let hanja_row = hanja_row.clone();
        hangul_row.connect_entry_activated(move |_| {
            hanja_row.grab_focus();
        });
    }
    for row in [&hanja_row, &meaning_row] {
        let f = do_save.clone();
        row.connect_entry_activated(move |_| f());
    }

    {
        let d = dialog.downgrade();
        cancel_btn.connect_clicked(move |_| {
            if let Some(d) = d.upgrade() {
                d.close();
            }
        });
    }
    // Esc = 닫기.
    {
        let key = gtk4::EventControllerKey::new();
        let d = dialog.downgrade();
        key.connect_key_pressed(move |_, k, _, _| {
            if k == gdk::Key::Escape {
                if let Some(d) = d.upgrade() {
                    d.close();
                }
                glib::Propagation::Stop
            } else {
                glib::Propagation::Proceed
            }
        });
        dialog.add_controller(key);
    }

    gtk4::prelude::GtkWindowExt::set_focus(&dialog, Some(&hangul_row));
    dialog.present();
    let hangul_row_c = hangul_row.clone();
    glib::idle_add_local_once(move || {
        hangul_row_c.grab_focus();
    });
}
