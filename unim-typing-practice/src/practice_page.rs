//! 타자 연습 단일 페이지 — corpus 선택 + 지문 + 입력 + 통계 + 종료 후 히트맵.

use std::cell::RefCell;
use std::rc::Rc;

use gtk4::prelude::*;
use gtk4::{self as gtk, glib};
use libadwaita::prelude::*;
use libadwaita::{self as adw};

use unim_keymap_common::KeyboardWidget;

use crate::app::SharedAppState;
use crate::corpus::{self, CorpusKind};
use crate::practice_engine::PracticeSession;

const TICK_INTERVAL_MS: u32 = 100;

pub fn build(state: SharedAppState, toast: adw::ToastOverlay) -> (gtk::Widget, Rc<dyn Fn()>) {
    let scroller = gtk::ScrolledWindow::builder()
        .hscrollbar_policy(gtk::PolicyType::Never)
        .build();
    let outer = gtk::Box::new(gtk::Orientation::Vertical, 12);
    outer.set_margin_top(16);
    outer.set_margin_bottom(16);
    outer.set_margin_start(16);
    outer.set_margin_end(16);

    // 상단: corpus 선택
    let corpus_group = adw::PreferencesGroup::builder()
        .title(&*rust_i18n::t!("practice_corpus"))
        .build();
    let corpus_combo = adw::ComboRow::builder()
        .title(&*rust_i18n::t!("practice_corpus"))
        .build();
    let corpus_model = gtk::StringList::new(&[]);
    for kind in CorpusKind::all() {
        corpus_model.append(&rust_i18n::t!(kind.i18n_key()));
    }
    corpus_combo.set_model(Some(&corpus_model));
    corpus_combo.set_selected(0);
    corpus_group.add(&corpus_combo);
    outer.append(&corpus_group);

    // IME 경고
    let hint = gtk::Label::builder()
        .label(&*rust_i18n::t!("hint_ime_warning"))
        .css_classes(["dim-label", "caption"])
        .wrap(true)
        .xalign(0.0)
        .build();
    outer.append(&hint);

    // 지문 표시 — TextView (편집 불가, monospace, 색칠된 attribute 적용)
    let textview = gtk::TextView::builder()
        .editable(false)
        .cursor_visible(false)
        .monospace(true)
        .wrap_mode(gtk::WrapMode::WordChar)
        .pixels_above_lines(4)
        .pixels_below_lines(4)
        .build();
    textview.add_css_class("card");
    let text_scroll = gtk::ScrolledWindow::builder()
        .height_request(120)
        .child(&textview)
        .build();
    outer.append(&text_scroll);

    // 입력 영역 — Entry는 IM의 dead key를 우회할 수 없으므로 우리는 키 이벤트를 직접 받는다.
    // 따라서 입력 위젯은 visual placeholder Entry이고, 실제 이벤트는 EventControllerKey가 캡처.
    let input_entry = gtk::Entry::builder()
        .placeholder_text("type here / 여기에 입력")
        .input_purpose(gtk::InputPurpose::FreeForm)
        .build();
    outer.append(&input_entry);

    // 진행률 + 통계 박스
    let progress = gtk::ProgressBar::builder().fraction(0.0).show_text(true).build();
    outer.append(&progress);

    let stat_box = gtk::Box::new(gtk::Orientation::Horizontal, 16);
    stat_box.set_halign(gtk::Align::Center);
    let stat_wpm = make_stat(&rust_i18n::t!("stat_wpm"));
    let stat_cpm = make_stat(&rust_i18n::t!("stat_cpm"));
    let stat_acc = make_stat(&rust_i18n::t!("stat_accuracy"));
    let stat_err = make_stat(&rust_i18n::t!("stat_error_rate"));
    stat_box.append(&stat_wpm.0);
    stat_box.append(&stat_cpm.0);
    stat_box.append(&stat_acc.0);
    stat_box.append(&stat_err.0);
    outer.append(&stat_box);

    // 종료 후 히트맵 영역
    let heatmap_group = adw::PreferencesGroup::builder()
        .title(&*rust_i18n::t!("heatmap_title"))
        .build();
    heatmap_group.set_visible(false);
    let keyboard = Rc::new(KeyboardWidget::new());
    outer.append(keyboard.root());
    keyboard.root().set_visible(false);
    outer.append(&heatmap_group);

    // 액션 버튼
    let action_box = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    action_box.set_halign(gtk::Align::End);
    let btn_restart = gtk::Button::with_label(&rust_i18n::t!("btn_restart"));
    let btn_other = gtk::Button::with_label(&rust_i18n::t!("btn_other_corpus"));
    let btn_copy = gtk::Button::with_label(&rust_i18n::t!("btn_copy_result"));
    btn_copy.set_visible(false);
    action_box.append(&btn_restart);
    action_box.append(&btn_other);
    action_box.append(&btn_copy);
    outer.append(&action_box);

    scroller.set_child(Some(&outer));

    // 세션 상태
    let session: Rc<RefCell<Option<PracticeSession>>> = Rc::new(RefCell::new(None));

    // === EventControllerKey ===
    let key_ctrl = gtk::EventControllerKey::new();
    {
        let session = session.clone();
        let progress_c = progress.clone();
        let stat_wpm_c = stat_wpm.1.clone();
        let stat_cpm_c = stat_cpm.1.clone();
        let stat_acc_c = stat_acc.1.clone();
        let stat_err_c = stat_err.1.clone();
        let textview_c = textview.clone();
        let heatmap_group_c = heatmap_group.clone();
        let keyboard_c = keyboard.clone();
        let btn_copy_c = btn_copy.clone();
        let toast_c = toast.clone();
        let input_entry_c = input_entry.clone();
        key_ctrl.connect_key_pressed(move |_, key, _, modifier| {
            let mut sess = session.borrow_mut();
            let sess = match sess.as_mut() {
                Some(s) => s,
                None => return glib::Propagation::Proceed,
            };
            // 한글/특수키 인쇄 불가 키는 무시
            let unicode = key.to_unicode();
            let byte = match unicode {
                Some(c) if c.is_ascii_graphic() || c == ' ' => c as u8,
                Some(c) if c == '\n' => b' ', // newline 처리
                _ => return glib::Propagation::Proceed,
            };
            let shift = modifier.contains(gtk::gdk::ModifierType::SHIFT_MASK);
            let outcome = sess.handle_keypress(byte, shift);
            sess.tick();

            // UI 업데이트
            progress_c.set_fraction(outcome.progress);
            stat_wpm_c.set_text(&format!("{:.0}", sess.stats.wpm()));
            stat_cpm_c.set_text(&format!("{:.0}", sess.stats.cpm()));
            stat_acc_c.set_text(&format!("{:.0}%", sess.stats.accuracy()));
            stat_err_c.set_text(&format!("{:.0}%", sess.stats.error_rate()));
            paint_target(&textview_c, sess);

            if outcome.done {
                // 종료 — 히트맵 표시
                let stats_map: std::collections::HashMap<(u8, u8), _> =
                    sess.key_stats.clone();
                keyboard_c.set_heatmap(stats_map);
                keyboard_c.populate(sess.profile());
                keyboard_c.root().set_visible(true);
                heatmap_group_c.set_visible(true);
                btn_copy_c.set_visible(true);
                toast_c.add_toast(adw::Toast::new(
                    &rust_i18n::t!(
                        "toast_practice_done",
                        wpm = format!("{:.0}", sess.stats.wpm()),
                        acc = format!("{:.0}", sess.stats.accuracy())
                    ),
                ));
            }
            // 입력 위젯 텍스트는 우리 자체 관리(IME 우회) — 항상 비움
            input_entry_c.set_text("");
            glib::Propagation::Stop
        });
    }
    input_entry.add_controller(key_ctrl);

    // 30fps 틱 — elapsed_secs 갱신 → WPM도 시간 흐름에 따라 변함
    {
        let session = session.clone();
        let stat_wpm_c = stat_wpm.1.clone();
        let stat_cpm_c = stat_cpm.1.clone();
        glib::timeout_add_local(std::time::Duration::from_millis(TICK_INTERVAL_MS as u64), move || {
            if let Some(s) = session.borrow_mut().as_mut() {
                s.tick();
                stat_wpm_c.set_text(&format!("{:.0}", s.stats.wpm()));
                stat_cpm_c.set_text(&format!("{:.0}", s.stats.cpm()));
            }
            glib::ControlFlow::Continue
        });
    }

    // === 세션 시작/리셋 헬퍼 ===
    let session_for_start = session.clone();
    let textview_for_start = textview.clone();
    let progress_for_start = progress.clone();
    let stat_wpm_for_start = stat_wpm.1.clone();
    let stat_cpm_for_start = stat_cpm.1.clone();
    let stat_acc_for_start = stat_acc.1.clone();
    let stat_err_for_start = stat_err.1.clone();
    let keyboard_for_start = keyboard.clone();
    let heatmap_group_for_start = heatmap_group.clone();
    let btn_copy_for_start = btn_copy.clone();
    let input_entry_for_start = input_entry.clone();
    let state_for_start = state.clone();
    let corpus_combo_for_start = corpus_combo.clone();

    let start_session = Rc::new(move || {
        let profile = state_for_start.current_profile.borrow().clone();
        let Some(profile) = profile else {
            return;
        };
        let kind = CorpusKind::all()[corpus_combo_for_start.selected() as usize];
        let text = corpus::load(kind);
        let mut new_sess = PracticeSession::new(profile, text);
        new_sess.tick();
        paint_target(&textview_for_start, &new_sess);
        progress_for_start.set_fraction(0.0);
        stat_wpm_for_start.set_text("0");
        stat_cpm_for_start.set_text("0");
        stat_acc_for_start.set_text("100%");
        stat_err_for_start.set_text("0%");
        keyboard_for_start.root().set_visible(false);
        heatmap_group_for_start.set_visible(false);
        btn_copy_for_start.set_visible(false);
        input_entry_for_start.set_text("");
        input_entry_for_start.grab_focus();
        *session_for_start.borrow_mut() = Some(new_sess);
    });

    // 액션 이벤트
    {
        let start_session = start_session.clone();
        btn_restart.connect_clicked(move |_| start_session());
    }
    {
        let start_session = start_session.clone();
        btn_other.connect_clicked(move |_| start_session());
    }
    {
        let session = session.clone();
        let toast = toast.clone();
        btn_copy.connect_clicked(move |_| {
            if let Some(s) = session.borrow().as_ref() {
                let report = format!(
                    "WPM {:.0} / CPM {:.0} / accuracy {:.1}% / errors {} / time {:.1}s",
                    s.stats.wpm(),
                    s.stats.cpm(),
                    s.stats.accuracy(),
                    s.stats.error_chars,
                    s.stats.elapsed_secs
                );
                if let Some(display) = gtk::gdk::Display::default() {
                    display.clipboard().set_text(&report);
                }
                toast.add_toast(adw::Toast::new(&rust_i18n::t!("toast_copied")));
            }
        });
    }
    {
        let start_session = start_session.clone();
        corpus_combo.connect_selected_notify(move |_| {
            start_session();
        });
    }

    // refresh — 외부(사이드바)에서 프로필이 바뀔 때 호출.
    let refresh: Rc<dyn Fn()> = Rc::new(move || {
        start_session();
    });

    (scroller.upcast(), refresh)
}

fn paint_target(textview: &gtk::TextView, sess: &PracticeSession) {
    let buf = textview.buffer();
    // 전부 클리어 후 다시 채움(짧으니 비용 무시).
    let target: String = sess.target().iter().collect();
    buf.set_text(&target);
    // Tag 정의
    let table = buf.tag_table();
    let correct_tag = table
        .lookup("correct")
        .unwrap_or_else(|| buf.create_tag(Some("correct"), &[("foreground", &"#2da94f")]).unwrap());
    let _wrong_tag = table
        .lookup("wrong")
        .unwrap_or_else(|| buf.create_tag(Some("wrong"), &[("foreground", &"#d6453c")]).unwrap());
    let dim_tag = table
        .lookup("dim")
        .unwrap_or_else(|| buf.create_tag(Some("dim"), &[("foreground", &"#888888")]).unwrap());

    // 진행한 글자: 정답 글자만큼 correct, 오타는 wrong(현재는 cursor가 단순 증가하므로 정답/오타를 commit 시점에 구분)
    // 단순화: cursor 전까지 dim, 이후 default.
    let cursor = sess.cursor;
    if cursor > 0 {
        let start = buf.start_iter();
        let mut end = buf.start_iter();
        end.forward_chars(cursor as i32);
        buf.apply_tag(&dim_tag, &start, &end);
        // 정답 길이만큼 correct로 덮음
        let correct_len = sess.stats.correct_chars as i32;
        if correct_len > 0 {
            let s2 = buf.start_iter();
            let mut e2 = buf.start_iter();
            e2.forward_chars(correct_len.min(cursor as i32));
            buf.apply_tag(&correct_tag, &s2, &e2);
        }
    }
}

fn make_stat(label: &str) -> (gtk::Box, gtk::Label) {
    let b = gtk::Box::new(gtk::Orientation::Vertical, 2);
    b.set_halign(gtk::Align::Center);
    let lab = gtk::Label::builder()
        .label(label)
        .css_classes(["caption", "dim-label"])
        .build();
    let val = gtk::Label::builder()
        .label("0")
        .css_classes(["title-2"])
        .build();
    b.append(&val);
    b.append(&lab);
    (b, val)
}
