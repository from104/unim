//! Practice + Result 페이지 — DESIGN.md + design/app.jsx 시안 1:1.
//!
//! 구조:
//!   HeaderBar (start: CorpusDropdown, center: WindowTitle, end: Restart + Menu)
//!   ViewSwitcher (segmented)
//!   ViewStack:
//!     - "practice" : grid (1fr 280px) + 키보드 카드
//!     - "result"   : ResultHeader + BigStatsCard + Duration|KeyCount + Heatmap

use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::f64::consts::PI;
use std::rc::Rc;

use gtk4::glib::translate::IntoGlib;
use gtk4::prelude::*;
use gtk4::{self as gtk, gdk, gio, glib};
use libadwaita::prelude::*;
use libadwaita::{self as adw};


use crate::active_layout;
use crate::corpus::{self, CorpusEntry, CorpusKind, USER_CORPUS_MAX_BYTES};
use crate::keyboard_view::KeyboardView;
use crate::practice_engine::{align_input_to_target, PracticeSession};
use crate::segmented_progress::SegmentedProgress;
use crate::triangle_spinner::TriangleSpinner;

const TICK_INTERVAL_MS: u32 = 100;
/// 한 TargetLine 의 최대 글자 수 — 이보다 길면 띄어쓰기 단위로 잘라 별도 줄로.
const MAX_CHARS_PER_LINE: usize = 30;

// 라이트 토큰. 다크 자동 분기는 미해결 (design-brief.md §16).
const COLOR_CORRECT: &str = "#1c66c9";
const COLOR_WRONG: &str = "#d24e15";
const COLOR_DIM: &str = "#b6b6b2";
const COLOR_ACCENT_F: (f64, f64, f64) = (0.11, 0.40, 0.79); // #1c66c9

/// 메인 빌더 — 헤더바 위젯을 부착한 뒤 본문 위젯을 반환.
pub fn build(toast: adw::ToastOverlay, header: &adw::HeaderBar) -> gtk::Widget {
    // 1) 활성 자판.
    let (layout_code, profile) = active_layout::load_active_profile();
    // 런타임 자판 변경을 위해 slot 패턴 — polling 시 reload 가 슬롯만 교체.
    let profile_slot: Rc<RefCell<unim::keystroke::profile::LayoutProfile>> =
        Rc::new(RefCell::new(profile.clone()));
    let layout_code_slot: Rc<RefCell<String>> = Rc::new(RefCell::new(layout_code.clone()));
    let (en_code_initial, en_profile_initial) = active_layout::load_active_english_profile();
    let en_code_slot: Rc<RefCell<String>> = Rc::new(RefCell::new(en_code_initial));
    let en_profile_slot: Rc<RefCell<Option<unim::keystroke::profile::LayoutProfile>>> =
        Rc::new(RefCell::new(en_profile_initial));
    let display_name = profile_display_name(&profile, &layout_code);
    let ko_part = if display_name == layout_code {
        layout_code.clone()
    } else {
        format!("{} · {}", display_name, layout_code)
    };
    let en_code = en_code_slot.borrow().clone();
    let en_label = active_layout::english_layout_label(&en_code);
    let en_part = if en_label == en_code {
        en_code.clone()
    } else {
        format!("{} · {}", en_label, en_code)
    };
    // 한·영 자판 모두 표시 — 예: "두벌식 표준 · ko_2bulstd / QWERTY · en_qwerty".
    let subtitle = format!("{ko_part} / {en_part}");
    let title_widget = adw::WindowTitle::new(&rust_i18n::t!("app_title"), &subtitle);
    header.set_title_widget(Some(&title_widget));

    // 2) 헤더바 — start: corpus DropDown (빌트인 3 + 사용자 정의 N).
    let corpus_entries: Rc<RefCell<Vec<CorpusEntry>>> = Rc::new(RefCell::new(Vec::new()));
    let corpus_dropdown = gtk::DropDown::new(None::<gtk::StringList>, None::<gtk::Expression>);
    corpus_dropdown.set_tooltip_text(Some(&rust_i18n::t!("header_corpus_label")));
    corpus_dropdown.set_valign(gtk::Align::Center);
    // 헤더의 셀렉티드 표시는 단순 라벨, 팝오버 리스트에만 호버 수정/삭제 아이콘.
    corpus_dropdown.set_factory(Some(&make_corpus_main_factory(corpus_entries.clone())));
    corpus_dropdown.set_list_factory(Some(&make_corpus_list_factory(
        corpus_entries.clone(),
        corpus_dropdown.clone(),
        toast.clone(),
    )));
    rebuild_corpus_dropdown(&corpus_dropdown, &corpus_entries, None);
    header.pack_start(&corpus_dropdown);

    // 3) 헤더바 — end: menu + restart.
    let menu_btn = gtk::MenuButton::builder()
        .icon_name("open-menu-symbolic")
        .tooltip_text(rust_i18n::t!("header_menu_tooltip"))
        .build();
    let menu_model = gio::Menu::new();
    let mi_copy = gio::MenuItem::new(
        Some(&rust_i18n::t!("menu_copy_result")),
        Some("win.copy-result"),
    );
    mi_copy.set_attribute_value("accel", Some(&"<Primary><Shift>c".to_variant()));
    menu_model.append_item(&mi_copy);
    let mi_result = gio::MenuItem::new(
        Some(&rust_i18n::t!("menu_view_heatmap")),
        Some("win.show-result"),
    );
    mi_result.set_attribute_value("accel", Some(&"<Primary>2".to_variant()));
    menu_model.append_item(&mi_result);
    let mi_practice = gio::MenuItem::new(
        Some(&rust_i18n::t!("menu_back_to_practice")),
        Some("win.show-practice"),
    );
    mi_practice.set_attribute_value("accel", Some(&"<Primary>1".to_variant()));
    menu_model.append_item(&mi_practice);

    // 가져오기 — 사용자 정의 지문 (2000 byte 상한).
    let import_section = gio::Menu::new();
    import_section.append(
        Some(&rust_i18n::t!("menu_import_file")),
        Some("win.import-corpus-file"),
    );
    import_section.append(
        Some(&rust_i18n::t!("menu_import_clipboard")),
        Some("win.import-corpus-clipboard"),
    );
    menu_model.append_section(None, &import_section);
    // 이름 변경/삭제는 코퍼스 드롭다운의 행 호버 시 우측 아이콘으로 노출 (set_list_factory).

    let popover_menu = gtk::PopoverMenu::from_model(Some(&menu_model));
    menu_btn.set_popover(Some(&popover_menu));
    header.pack_end(&menu_btn);

    let btn_restart = gtk::Button::builder()
        .icon_name("view-refresh-symbolic")
        .tooltip_text(rust_i18n::t!("header_restart_tooltip"))
        .build();
    header.pack_end(&btn_restart);

    // UNIM 설정 호출 버튼 — 도움말 ? 아이콘 우측 (시각 순서).
    // pack_end 는 우→좌로 쌓이므로 settings 를 help *보다 먼저* 호출 →
    // 결과 시각 순서: [help] [settings] [restart] [menu].
    let btn_settings = gtk::Button::builder()
        .icon_name("preferences-system-symbolic")
        .tooltip_text(rust_i18n::t!("header_settings_tooltip"))
        .build();
    btn_settings.connect_clicked(|_| {
        // 비차단 spawn — 실패해도 typing-practice 동작에 영향 없음.
        let _ = std::process::Command::new("unim-settings").spawn();
    });
    header.pack_end(&btn_settings);

    // ? 도움말 버튼 — 설정의 좌측.
    let btn_help = gtk::Button::builder()
        .icon_name("help-about-symbolic")
        .tooltip_text(rust_i18n::t!("header_help_tooltip"))
        .build();
    header.pack_end(&btn_help);

    // 4) ViewStack + ViewSwitcher.
    let view_stack = adw::ViewStack::new();
    let view_switcher = adw::ViewSwitcher::builder()
        .stack(&view_stack)
        .policy(adw::ViewSwitcherPolicy::Wide)
        .build();
    view_switcher.set_halign(gtk::Align::Center);

    // =================================================================
    // Practice 페이지
    // =================================================================
    let practice_root = gtk::Box::new(gtk::Orientation::Vertical, 18);
    practice_root.set_margin_top(16);
    practice_root.set_margin_bottom(20);
    practice_root.set_margin_start(20);
    practice_root.set_margin_end(20);

    // 줄 진행 상태.
    let lines: Rc<RefCell<Vec<String>>> = Rc::new(RefCell::new(Vec::new()));
    let line_idx: Rc<RefCell<usize>> = Rc::new(RefCell::new(0));
    let started_pressed: Rc<Cell<bool>> = Rc::new(Cell::new(false));

    // 좌 컬럼.
    // TargetCard — full-height: 자식 vexpand=true 로 카드가 row 영역을 꽉 채움.
    let target_lines_box = gtk::Box::new(gtk::Orientation::Vertical, 0);
    target_lines_box.add_css_class("typing-target-card");
    target_lines_box.set_valign(gtk::Align::Fill);
    target_lines_box.set_vexpand(true);

    let target_scroll = gtk::ScrolledWindow::builder()
        .hscrollbar_policy(gtk::PolicyType::Never)
        .min_content_height(220)
        .vexpand(true)
        .child(&target_lines_box)
        .build();
    target_scroll.set_valign(gtk::Align::Fill);

    let target_line_widgets: Rc<RefCell<Vec<TargetLineWidget>>> =
        Rc::new(RefCell::new(Vec::new()));

    // InputField — 컨테이너 카드 + Entry(frame 없음) + IME 칩.
    let input_entry = gtk::Entry::builder()
        .placeholder_text(rust_i18n::t!("input_placeholder"))
        .hexpand(true)
        .has_frame(false)
        .build();
    input_entry.add_css_class("typing-input");
    let ime_chip = gtk::Label::builder()
        .label(rust_i18n::t!("ime_chip_hangul"))
        .css_classes(["typing-ime-chip"])
        .valign(gtk::Align::Center)
        .build();
    let input_card = gtk::Box::new(gtk::Orientation::Horizontal, 10);
    input_card.add_css_class("typing-input-card");
    input_card.set_hexpand(true);
    input_card.append(&input_entry);
    input_card.append(&ime_chip);

    // 좌/우 컬럼 vbox 폐기 — 2×2 Grid 로 직접 attach (아래 columns 참조).
    // target_scroll, input_card, line_pos_card, stats_card, progress_card 는
    // 각자 Grid 셀에 attach 된다.

    // LinePosCard — horizontal baseline.
    let line_pos_caption = gtk::Label::builder()
        .label(rust_i18n::t!("line_position_caption"))
        .css_classes(["typing-card-label", "dim-label"])
        .xalign(0.0)
        .hexpand(true)
        .build();
    let line_pos_label = gtk::Label::builder()
        .label("0 / 0")
        .css_classes(["typing-numeric-prominent"])
        .xalign(1.0)
        .build();
    let line_pos_box = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    line_pos_box.set_valign(gtk::Align::Baseline);
    line_pos_box.append(&line_pos_caption);
    line_pos_box.append(&line_pos_label);
    let line_pos_card = make_card(&line_pos_box, 12, 14);

    // StatsCard 2×2.
    let stat_wpm = make_stat_cell(
        &rust_i18n::t!("stat_caption_wpm"),
        &rust_i18n::t!("stat_wpm"),
        None,
    );
    let stat_cpm = make_stat_cell(
        &rust_i18n::t!("stat_caption_cpm"),
        &rust_i18n::t!("stat_cpm"),
        None,
    );
    let stat_acc = make_stat_cell(
        &rust_i18n::t!("stat_accuracy"),
        "%",
        Some("typing-stat-correct"),
    );
    let stat_err = make_stat_cell(
        &rust_i18n::t!("stat_error_rate"),
        "%",
        Some("typing-stat-wrong"),
    );
    let stats_grid = gtk::Grid::builder()
        .row_spacing(10)
        .column_spacing(14)
        .column_homogeneous(true)
        .build();
    stats_grid.attach(&stat_wpm.0, 0, 0, 1, 1);
    stats_grid.attach(&stat_cpm.0, 1, 0, 1, 1);
    stats_grid.attach(&stat_acc.0, 0, 1, 1, 1);
    stats_grid.attach(&stat_err.0, 1, 1, 1, 1);
    let stats_card = make_card(&stats_grid, 14, 14);

    // ProgressCard — caption + percent + 20-seg bar.
    let progress_caption = gtk::Label::builder()
        .label(rust_i18n::t!("progress_label"))
        .css_classes(["typing-card-label", "dim-label"])
        .xalign(0.0)
        .hexpand(true)
        .build();
    let progress_percent = gtk::Label::builder()
        .label("0%")
        .css_classes(["typing-numeric-small"])
        .xalign(1.0)
        .build();
    let progress_top = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    progress_top.set_valign(gtk::Align::Baseline);
    progress_top.append(&progress_caption);
    progress_top.append(&progress_percent);
    let progress_bar = SegmentedProgress::new(20);
    let progress_box = gtk::Box::new(gtk::Orientation::Vertical, 8);
    progress_box.append(&progress_top);
    progress_box.append(progress_bar.root());
    let progress_card = make_card(&progress_box, 12, 14);

    // 4×10 Grid 레이아웃 (사용자 요구):
    //   col 0–7 (좌, 8/10)        col 8–9 (우, 2/10)
    //   row 0–2 : target_scroll    row 0  : line_pos_card
    //                              row 1–2: stats_card
    //   row 3   : input_card       row 3  : progress_card
    // - 좌 target 3행 = 우 LinePos(1행) + Stats(2행) 합한 높이
    // - 좌 input 1행 = 우 progress 1행, 동일 row 3 → 세로 위치 + 높이 동일.
    target_scroll.set_hexpand(true);
    target_scroll.set_vexpand(true);
    input_card.set_hexpand(true);
    line_pos_card.add_css_class("typing-right-col");
    stats_card.add_css_class("typing-right-col");
    progress_card.add_css_class("typing-right-col");
    line_pos_card.set_hexpand(true);
    stats_card.set_hexpand(true);
    stats_card.set_vexpand(true);
    progress_card.set_hexpand(true);

    let columns = gtk::Grid::builder()
        .column_spacing(18)
        .row_spacing(12)
        .column_homogeneous(true)
        .hexpand(true)
        .vexpand(true)
        .build();
    columns.attach(&target_scroll, 0, 0, 7, 3);
    columns.attach(&line_pos_card, 7, 0, 3, 1);
    columns.attach(&stats_card, 7, 1, 3, 2);
    columns.attach(&input_card, 0, 3, 7, 1);
    columns.attach(&progress_card, 7, 3, 3, 1);
    practice_root.append(&columns);

    // 키보드 카드 — cardBgSoft, 가운데 정렬. slot 패턴으로 reload 시 교체 가능.
    let kbd_view_slot: Rc<RefCell<Rc<KeyboardView>>> =
        Rc::new(RefCell::new(KeyboardView::new(
            Some(&profile),
            en_profile_slot.borrow().as_ref(),
        )));
    let kbd_card_inner = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    kbd_card_inner.append(kbd_view_slot.borrow().root());
    let kbd_card = gtk::Frame::new(None);
    kbd_card.add_css_class("typing-keyboard-card");
    kbd_card.set_child(Some(&kbd_card_inner));
    kbd_card.set_halign(gtk::Align::Center);

    let kbd_holder = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    kbd_holder.set_hexpand(true);
    kbd_holder.set_halign(gtk::Align::Fill);
    kbd_holder.set_margin_top(8);
    let l = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    let r = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    l.set_hexpand(true);
    r.set_hexpand(true);
    kbd_holder.append(&l);
    kbd_holder.append(&kbd_card);
    kbd_holder.append(&r);
    practice_root.append(&kbd_holder);

    let practice_scroller = gtk::ScrolledWindow::builder()
        .hscrollbar_policy(gtk::PolicyType::Never)
        .child(&practice_root)
        .build();

    view_stack.add_titled_with_icon(
        &practice_scroller,
        Some("practice"),
        &rust_i18n::t!("view_practice"),
        "input-keyboard-symbolic",
    );

    // =================================================================
    // Result 페이지 — Practice 와 동일한 4×10 그리드 + KeyboardView 히트맵.
    // 그리드 좌표 매핑 (4×10):
    //   (0,0,7,2) BigStats    | (7,0,3,1) 세션 메타
    //                         | (7,1,3,2) KeyCount
    //   (0,2,7,2) Sparkline   | (7,3,3,1) 액션 버튼
    // 그리드 아래 KeyboardView 히트맵 — Practice keyboard 와 동일 위치/디자인.
    // =================================================================
    let result_root = gtk::Box::new(gtk::Orientation::Vertical, 18);
    result_root.set_margin_top(16);
    result_root.set_margin_bottom(20);
    result_root.set_margin_start(20);
    result_root.set_margin_end(20);

    // BigStats — WPM/CPM/Acc/Err 가로 4 (시각 메인).
    let big_wpm = make_big_stat(
        &rust_i18n::t!("stat_caption_wpm"),
        &rust_i18n::t!("stat_wpm"),
        None,
    );
    let big_cpm = make_big_stat(
        &rust_i18n::t!("stat_caption_cpm"),
        &rust_i18n::t!("stat_cpm"),
        None,
    );
    let big_acc = make_big_stat(
        &rust_i18n::t!("stat_accuracy"),
        "%",
        Some("typing-stat-correct"),
    );
    let big_err = make_big_stat(
        &rust_i18n::t!("stat_error_rate"),
        "%",
        Some("typing-stat-wrong"),
    );
    let big_grid = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    big_grid.append(&big_wpm.0);
    big_grid.append(&make_v_divider());
    big_grid.append(&big_cpm.0);
    big_grid.append(&make_v_divider());
    big_grid.append(&big_acc.0);
    big_grid.append(&make_v_divider());
    big_grid.append(&big_err.0);
    big_wpm.0.set_hexpand(true);
    big_cpm.0.set_hexpand(true);
    big_acc.0.set_hexpand(true);
    big_err.0.set_hexpand(true);
    let big_card = make_card(&big_grid, 20, 16);
    big_card.set_hexpand(true);
    big_card.set_vexpand(true);

    // 세션 메타 — caption + corpus 이름 (Practice line_pos_card 위치).
    let res_caption = gtk::Label::builder()
        .label(rust_i18n::t!("result_session_label"))
        .css_classes(["typing-card-label", "dim-label"])
        .xalign(0.0)
        .build();
    let res_title = gtk::Label::builder()
        .label(&subtitle)
        .css_classes(["title-4"])
        .xalign(0.0)
        .wrap(true)
        .build();
    let res_meta_box = gtk::Box::new(gtk::Orientation::Vertical, 2);
    res_meta_box.append(&res_caption);
    res_meta_box.append(&res_title);
    let res_meta_card = make_card(&res_meta_box, 14, 10);
    res_meta_card.add_css_class("typing-right-col");
    res_meta_card.set_hexpand(true);

    // KeyCount (Practice stats_card 위치).
    let kc_caption = gtk::Label::builder()
        .label(rust_i18n::t!("result_label_key_stats"))
        .css_classes(["typing-card-label", "dim-label"])
        .xalign(0.0)
        .build();
    let kc_typed = make_key_count_row(&rust_i18n::t!("result_label_typed"), false);
    let kc_errors = make_key_count_row(&rust_i18n::t!("result_label_errors"), true);
    let kc_bs = make_key_count_row(&rust_i18n::t!("result_label_backspace"), false);
    let kc_time = make_key_count_row(&rust_i18n::t!("result_label_time"), false);
    let kc_box = gtk::Box::new(gtk::Orientation::Vertical, 6);
    kc_box.append(&kc_caption);
    kc_box.append(&kc_typed.0);
    kc_box.append(&kc_errors.0);
    kc_box.append(&kc_bs.0);
    kc_box.append(&kc_time.0);
    let kc_card = make_card(&kc_box, 14, 14);
    kc_card.add_css_class("typing-right-col");
    kc_card.set_hexpand(true);
    kc_card.set_vexpand(true);

    // Sparkline — 줄별 WPM 추이 (Practice input_card 위치).
    let spark_caption = gtk::Label::builder()
        .label(rust_i18n::t!("result_label_per_line_wpm"))
        .css_classes(["typing-card-label", "dim-label"])
        .xalign(0.0)
        .build();
    let spark_area = gtk::DrawingArea::new();
    spark_area.set_content_height(56);
    spark_area.set_hexpand(true);
    let sparkline_data: Rc<RefCell<Vec<f64>>> = Rc::new(RefCell::new(Vec::new()));
    {
        let data = sparkline_data.clone();
        spark_area.set_draw_func(move |_, cr, w, h| {
            let data = data.borrow();
            draw_sparkline(cr, w as f64, h as f64, &data);
        });
    }
    let dur_box = gtk::Box::new(gtk::Orientation::Vertical, 6);
    dur_box.append(&spark_caption);
    dur_box.append(&spark_area);
    let dur_card = make_card(&dur_box, 14, 10);
    dur_card.set_hexpand(true);

    // 액션 — Copy / Restart (Practice progress_card 위치).
    let btn_copy = gtk::Button::builder()
        .label(rust_i18n::t!("btn_copy"))
        .css_classes(["typing-pill-btn"])
        .hexpand(true)
        .build();
    let btn_restart_res = gtk::Button::builder()
        .label(rust_i18n::t!("btn_restart"))
        .css_classes(["typing-pill-btn", "suggested-action"])
        .hexpand(true)
        .build();
    let res_actions = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    res_actions.append(&btn_copy);
    res_actions.append(&btn_restart_res);
    let res_actions_card = make_card(&res_actions, 14, 10);
    res_actions_card.add_css_class("typing-right-col");
    res_actions_card.set_hexpand(true);

    let result_columns = gtk::Grid::builder()
        .column_spacing(18)
        .row_spacing(12)
        .column_homogeneous(true)
        .hexpand(true)
        .vexpand(true)
        .build();
    result_columns.attach(&big_card, 0, 0, 7, 2);
    result_columns.attach(&res_meta_card, 7, 0, 3, 1);
    result_columns.attach(&kc_card, 7, 1, 3, 2);
    result_columns.attach(&dur_card, 0, 2, 7, 2);
    result_columns.attach(&res_actions_card, 7, 3, 3, 1);
    result_root.append(&result_columns);

    // 키보드 히트맵 — Practice KeyboardView 와 동일 시각 디자인.
    let heat_view_slot: Rc<RefCell<Rc<KeyboardView>>> =
        Rc::new(RefCell::new(KeyboardView::new(
            Some(&profile),
            en_profile_slot.borrow().as_ref(),
        )));
    let heat_card_inner = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    heat_card_inner.append(heat_view_slot.borrow().root());
    let heat_card = gtk::Frame::new(None);
    heat_card.add_css_class("typing-keyboard-card");
    heat_card.set_child(Some(&heat_card_inner));
    heat_card.set_halign(gtk::Align::Center);

    let heat_holder = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    heat_holder.set_hexpand(true);
    heat_holder.set_halign(gtk::Align::Fill);
    heat_holder.set_margin_top(8);
    let hl = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    let hr = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    hl.set_hexpand(true);
    hr.set_hexpand(true);
    heat_holder.append(&hl);
    heat_holder.append(&heat_card);
    heat_holder.append(&hr);
    result_root.append(&heat_holder);

    let result_empty = adw::StatusPage::builder()
        .icon_name("emblem-default-symbolic")
        .title(rust_i18n::t!("result_summary"))
        .description(rust_i18n::t!("result_no_data"))
        .build();
    result_empty.add_css_class("compact");

    let result_inner_stack = gtk::Stack::new();
    result_inner_stack.set_transition_type(gtk::StackTransitionType::Crossfade);
    result_inner_stack.add_named(&result_empty, Some("empty"));
    result_inner_stack.add_named(&result_root, Some("data"));
    result_inner_stack.set_visible_child_name("empty");

    let result_scroller = gtk::ScrolledWindow::builder()
        .hscrollbar_policy(gtk::PolicyType::Never)
        .child(&result_inner_stack)
        .build();

    view_stack.add_titled_with_icon(
        &result_scroller,
        Some("result"),
        &rust_i18n::t!("view_result"),
        "view-list-symbolic",
    );

    // 메인 body.
    let body = gtk::Box::new(gtk::Orientation::Vertical, 0);
    body.append(&view_switcher);
    body.append(&view_stack);
    view_stack.set_vexpand(true);
    view_switcher.set_margin_top(6);
    view_switcher.set_margin_bottom(6);

    // ── 세션 상태 ────────────────────────────────────────────────────────
    let session: Rc<RefCell<Option<PracticeSession>>> = Rc::new(RefCell::new(None));
    let last_finished: Rc<RefCell<bool>> = Rc::new(RefCell::new(false));
    let preedit_text: Rc<RefCell<String>> = Rc::new(RefCell::new(String::new()));
    let progress_bar_rc: Rc<SegmentedProgress> = Rc::new(progress_bar);

    // ── 키맵 시각 피드백 ────────────────────────────────────────────────
    let key_ctrl = gtk::EventControllerKey::new();
    key_ctrl.set_propagation_phase(gtk::PropagationPhase::Capture);
    {
        let slot = kbd_view_slot.clone();
        key_ctrl.connect_key_pressed(move |_, key, _, _| {
            slot.borrow().flash_key(key.into_glib());
            glib::Propagation::Proceed
        });
    }
    {
        let slot = kbd_view_slot.clone();
        key_ctrl.connect_key_released(move |_, key, _, _| {
            slot.borrow().flash_key(key.into_glib());
        });
    }
    body.add_controller(key_ctrl);

    // ── finalize_line ─────────────────────────────────────────────────────
    let finalize_line = {
        let session = session.clone();
        let lines = lines.clone();
        let line_idx = line_idx.clone();
        let line_pos_label_c = line_pos_label.clone();
        let progress_bar_c = progress_bar_rc.clone();
        let progress_percent_c = progress_percent.clone();
        let target_line_widgets_c = target_line_widgets.clone();
        let target_scroll_c2 = target_scroll.clone();
        let input_entry_c = input_entry.clone();
        let heat_view_c = heat_view_slot.clone();
        let result_inner_stack_c = result_inner_stack.clone();
        let view_stack_c = view_stack.clone();
        let last_finished_c = last_finished.clone();
        let toast_c = toast.clone();
        let stat_wpm_c = stat_wpm.1.clone();
        let stat_cpm_c = stat_cpm.1.clone();
        let stat_acc_c = stat_acc.1.clone();
        let stat_err_c = stat_err.1.clone();
        let big_wpm_c = big_wpm.1.clone();
        let big_cpm_c = big_cpm.1.clone();
        let big_acc_c = big_acc.1.clone();
        let big_err_c = big_err.1.clone();
        let kc_typed_c = kc_typed.1.clone();
        let kc_errors_c = kc_errors.1.clone();
        let kc_bs_c = kc_bs.1.clone();
        let kc_time_c = kc_time.1.clone();
        let sparkline_data_c = sparkline_data.clone();
        let spark_area_c = spark_area.clone();
        let preedit_for_finalize = preedit_text.clone();
        Rc::new(move |text: &str| {
            let mut sess_ref = session.borrow_mut();
            let Some(sess) = sess_ref.as_mut() else {
                return;
            };
            sess.commit_line(text);
            sess.tick();

            let (wpm_s, cpm_s, acc_s, err_s) = format_stats(sess);
            stat_wpm_c.set_text(&wpm_s);
            stat_cpm_c.set_text(&cpm_s);
            stat_acc_c.set_text(&acc_s);
            stat_err_c.set_text(&err_s);

            let next_idx = *line_idx.borrow() + 1;
            let total = lines.borrow().len();
            if next_idx < total {
                *line_idx.borrow_mut() = next_idx;
                let next_line = lines.borrow()[next_idx].clone();
                sess.advance_to_line(next_line);
                progress_bar_c.set_fraction(0.0);
                progress_percent_c.set_text("0%");
                paint_target_lines(
                    &target_line_widgets_c.borrow(),
                    sess,
                    &lines.borrow(),
                    next_idx,
                    false,
                    &target_scroll_c2,
                );
                drop(sess_ref);
                preedit_for_finalize.borrow_mut().clear();
                input_entry_c.set_text("");
                line_pos_label_c.set_text(&line_position_text(next_idx, total));
                input_entry_c.grab_focus();
            } else {
                paint_target_lines(
                    &target_line_widgets_c.borrow(),
                    sess,
                    &lines.borrow(),
                    next_idx,
                    true,
                    &target_scroll_c2,
                );
                let stats_map: HashMap<(u8, u8), _> = sess.key_stats.clone();
                heat_view_c.borrow().set_heatmap(stats_map);

                big_wpm_c.set_text(&wpm_s);
                big_cpm_c.set_text(&cpm_s);
                big_acc_c.set_text(&acc_s);
                big_err_c.set_text(&err_s);

                kc_typed_c.set_text(&format!("{}", sess.stats.total_input_chars));
                kc_errors_c.set_text(&format!("{}", sess.stats.error_chars));
                kc_bs_c.set_text(&format!("{}", sess.backspace_count));
                kc_time_c.set_text(&format_duration(sess.stats.elapsed_secs));

                *sparkline_data_c.borrow_mut() = sess.wpm_per_line.clone();
                spark_area_c.queue_draw();

                result_inner_stack_c.set_visible_child_name("data");
                view_stack_c.set_visible_child_name("result");
                *last_finished_c.borrow_mut() = true;

                let stats = sess.stats;
                toast_c.add_toast(adw::Toast::new(&rust_i18n::t!(
                    "toast_practice_done",
                    wpm = format!("{:.0}", stats.wpm()),
                    acc = format!("{:.0}", stats.accuracy())
                )));
            }
        })
    };

    // ── do_evaluate ──────────────────────────────────────────────────────
    let do_evaluate = {
        let session = session.clone();
        let lines_c = lines.clone();
        let line_idx_c = line_idx.clone();
        let target_line_widgets_c = target_line_widgets.clone();
        let target_scroll_c = target_scroll.clone();
        let progress_bar_c = progress_bar_rc.clone();
        let progress_percent_c = progress_percent.clone();
        let started_pressed_c = started_pressed.clone();
        let finalize_line_c = finalize_line.clone();
        let preedit_c = preedit_text.clone();
        Rc::new(move |committed: &str, allow_complete: bool| {
            let pre = preedit_c.borrow().clone();
            let combined = format!("{}{}", committed, pre);
            let mut sess_ref = session.borrow_mut();
            let Some(sess) = sess_ref.as_mut() else {
                return;
            };
            if !started_pressed_c.get() && !combined.is_empty() {
                started_pressed_c.set(true);
            }
            let eval = sess.evaluate(&combined);
            progress_bar_c.set_fraction(eval.progress);
            progress_percent_c.set_text(&format!(
                "{}%",
                (eval.progress * 100.0).round() as i32
            ));
            paint_target_lines(
                &target_line_widgets_c.borrow(),
                sess,
                &lines_c.borrow(),
                *line_idx_c.borrow(),
                false,
                &target_scroll_c,
            );

            // 자동 진행 — `check_line_done(force=false)`:
            //   input == target (정확) OR
            //   input 단어 개수 ≥ target 단어 개수 AND input 이 whitespace 로 끝남.
            let _ = allow_complete;
            let _ = eval;
            let target_text = sess.target_text();
            if pre.is_empty() && check_line_done(&target_text, committed, false) {
                drop(sess_ref);
                finalize_line_c(committed);
            }
        })
    };

    // try_advance — Enter/Tab 명시 트리거. force=true → check_line_done 통과.
    // 자동 진행 (Space 또는 input==target) 은 do_evaluate 가 처리.
    let try_advance = {
        let session_c = session.clone();
        let finalize_line_c = finalize_line.clone();
        Rc::new(move |text: &str, force: bool| -> bool {
            let target_text = {
                let s = session_c.borrow();
                s.as_ref().map(|s| s.target_text()).unwrap_or_default()
            };
            if check_line_done(&target_text, text, force) {
                finalize_line_c(text);
                true
            } else {
                false
            }
        })
    };

    // Entry::changed — 평가/색칠. 자동 진행은 do_evaluate 내부에서
    // `all_words_match` 일치 시 처리됨.
    {
        let do_evaluate_c = do_evaluate.clone();
        input_entry.connect_changed(move |entry| {
            let text = entry.text().to_string();
            do_evaluate_c(&text, true);
        });
    }

    // preedit_changed + 붙여넣기 차단.
    // 타자 연습은 직접 키 입력만 측정해야 하므로 Ctrl+V / 컨텍스트 메뉴 / 중간 클릭
    // primary-selection paste 를 전부 차단한다.
    if let Some(delegate) = input_entry.delegate() {
        if let Ok(text_widget) = delegate.downcast::<gtk::Text>() {
            let preedit_c = preedit_text.clone();
            let input_entry_c = input_entry.clone();
            let do_evaluate_c = do_evaluate.clone();
            text_widget.connect_preedit_changed(move |_, pre| {
                *preedit_c.borrow_mut() = pre.to_string();
                let committed = input_entry_c.text().to_string();
                do_evaluate_c(&committed, false);
            });
            // Ctrl+V·중간 클릭 paste 등 GtkText 가 발화하는 모든 paste 액션을 막는다.
            text_widget.connect_paste_clipboard(|t| {
                t.stop_signal_emission_by_name("paste-clipboard");
            });
            // GtkText 가 표준으로 제공하는 clipboard.paste GAction 자체도 비활성화
            // (컨텍스트 메뉴의 "붙여넣기" 항목이 dim 처리됨).
            text_widget.action_set_enabled("clipboard.paste", false);
        }
    }

    // Enter — 강제 다음 줄 (force=true).
    {
        let try_advance_c = try_advance.clone();
        input_entry.connect_activate(move |entry| {
            let text = entry.text().to_string();
            try_advance_c(&text, true);
        });
    }

    // Tab — entry capture phase 에서 키 직접 잡아 강제 진행 (포커스 이동 차단).
    {
        let try_advance_c = try_advance.clone();
        let input_entry_c = input_entry.clone();
        let ke = gtk::EventControllerKey::new();
        ke.set_propagation_phase(gtk::PropagationPhase::Capture);
        ke.connect_key_pressed(move |_, key, _, _| {
            if key == gdk::Key::Tab || key == gdk::Key::ISO_Left_Tab {
                let text = input_entry_c.text().to_string();
                if try_advance_c(&text, true) {
                    return glib::Propagation::Stop;
                }
            }
            glib::Propagation::Proceed
        });
        input_entry.add_controller(ke);
    }

    // 100ms tick — 누적 stat 갱신.
    {
        let session = session.clone();
        let stat_wpm_c = stat_wpm.1.clone();
        let stat_cpm_c = stat_cpm.1.clone();
        let stat_acc_c = stat_acc.1.clone();
        let stat_err_c = stat_err.1.clone();
        glib::timeout_add_local(
            std::time::Duration::from_millis(TICK_INTERVAL_MS as u64),
            move || {
                if let Some(s) = session.borrow_mut().as_mut() {
                    s.tick();
                    let (wpm_s, cpm_s, acc_s, err_s) = format_stats(s);
                    stat_wpm_c.set_text(&wpm_s);
                    stat_cpm_c.set_text(&cpm_s);
                    stat_acc_c.set_text(&acc_s);
                    stat_err_c.set_text(&err_s);
                }
                glib::ControlFlow::Continue
            },
        );
    }

    // ── start_session ─────────────────────────────────────────────────────
    let session_for_start = session.clone();
    let profile_slot_for_start = profile_slot.clone();
    let layout_code_slot_for_start = layout_code_slot.clone();
    let last_finished_for_start = last_finished.clone();
    let input_entry_for_start = input_entry.clone();
    let progress_bar_for_start = progress_bar_rc.clone();
    let progress_percent_for_start = progress_percent.clone();
    let stat_wpm_for_start = stat_wpm.1.clone();
    let stat_cpm_for_start = stat_cpm.1.clone();
    let stat_acc_for_start = stat_acc.1.clone();
    let stat_err_for_start = stat_err.1.clone();
    let view_stack_for_start = view_stack.clone();
    let corpus_dropdown_for_start = corpus_dropdown.clone();
    let corpus_entries_for_start = corpus_entries.clone();
    let lines_for_start = lines.clone();
    let line_idx_for_start = line_idx.clone();
    let line_pos_label_for_start = line_pos_label.clone();
    let started_pressed_for_start = started_pressed.clone();
    let target_lines_box_for_start = target_lines_box.clone();
    let target_line_widgets_for_start = target_line_widgets.clone();
    let target_scroll_for_start = target_scroll.clone();
    let preedit_for_start = preedit_text.clone();

    let start_session = Rc::new(move || {
        let idx = corpus_dropdown_for_start.selected() as usize;
        let text = {
            let entries = corpus_entries_for_start.borrow();
            entries
                .get(idx)
                .or_else(|| entries.first())
                .map(|e| e.text())
                .unwrap_or_default()
        };
        let split: Vec<String> = text
            .split('\n')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .flat_map(|line| split_long_line(&line, MAX_CHARS_PER_LINE))
            .collect();
        if split.is_empty() {
            return;
        }
        *lines_for_start.borrow_mut() = split.clone();
        *line_idx_for_start.borrow_mut() = 0;
        started_pressed_for_start.set(false);
        preedit_for_start.borrow_mut().clear();

        rebuild_target_lines(
            &target_lines_box_for_start,
            &target_line_widgets_for_start,
            split.len(),
        );

        let first_line = split[0].clone();
        let new_sess = PracticeSession::new(
            profile_slot_for_start.borrow().clone(),
            layout_code_slot_for_start.borrow().clone(),
            first_line,
        );
        paint_target_lines(
            &target_line_widgets_for_start.borrow(),
            &new_sess,
            &split,
            0,
            false,
            &target_scroll_for_start,
        );

        input_entry_for_start.set_text("");
        line_pos_label_for_start.set_text(&line_position_text(0, split.len()));
        progress_bar_for_start.set_fraction(0.0);
        progress_percent_for_start.set_text("0%");
        stat_wpm_for_start.set_text("0");
        stat_cpm_for_start.set_text("0");
        stat_acc_for_start.set_text("100");
        stat_err_for_start.set_text("0");
        *session_for_start.borrow_mut() = Some(new_sess);
        *last_finished_for_start.borrow_mut() = false;
        view_stack_for_start.set_visible_child_name("practice");
        input_entry_for_start.grab_focus();
    });

    {
        let s = start_session.clone();
        btn_restart.connect_clicked(move |_| s());
    }
    {
        let s = start_session.clone();
        btn_restart_res.connect_clicked(move |_| s());
    }
    {
        let s = start_session.clone();
        corpus_dropdown.connect_selected_notify(move |_| s());
    }

    // ── win.* 액션 ────────────────────────────────────────────────────────
    let action_group = gio::SimpleActionGroup::new();

    let act_copy = gio::SimpleAction::new("copy-result", None);
    {
        let session = session.clone();
        let toast = toast.clone();
        let last_finished = last_finished.clone();
        act_copy.connect_activate(move |_, _| {
            if !*last_finished.borrow() {
                return;
            }
            if let Some(s) = session.borrow().as_ref() {
                let report = format!(
                    "WPM {:.0} / CPM {:.0} / accuracy {:.1}% / errors {} / time {:.1}s",
                    s.stats.wpm(),
                    s.stats.cpm(),
                    s.stats.accuracy(),
                    s.stats.error_chars,
                    s.stats.elapsed_secs
                );
                if let Some(display) = gdk::Display::default() {
                    display.clipboard().set_text(&report);
                }
                toast.add_toast(adw::Toast::new(&rust_i18n::t!("toast_copied")));
            }
        });
    }
    action_group.add_action(&act_copy);
    {
        let s_act = act_copy.clone();
        btn_copy.connect_clicked(move |_| s_act.activate(None));
    }

    let act_show_result = gio::SimpleAction::new("show-result", None);
    {
        let view_stack = view_stack.clone();
        act_show_result.connect_activate(move |_, _| {
            view_stack.set_visible_child_name("result");
        });
    }
    action_group.add_action(&act_show_result);

    let act_show_practice = gio::SimpleAction::new("show-practice", None);
    {
        let view_stack = view_stack.clone();
        let input_entry_c = input_entry.clone();
        act_show_practice.connect_activate(move |_, _| {
            view_stack.set_visible_child_name("practice");
            input_entry_c.grab_focus();
        });
    }
    action_group.add_action(&act_show_practice);

    // Restart 액션 — 헤더/결과 버튼이 직접 start_session 을 호출하지만,
    // 단축키(Ctrl+R) 에서 활성화하려면 SimpleAction 매개체가 필요.
    let act_restart = gio::SimpleAction::new("restart", None);
    {
        let s = start_session.clone();
        act_restart.connect_activate(move |_, _| s());
    }
    action_group.add_action(&act_restart);

    // Help 액션 — F1 단축키.
    let act_help = gio::SimpleAction::new("help", None);
    {
        let body_weak = body.downgrade();
        act_help.connect_activate(move |_, _| {
            if let Some(body) = body_weak.upgrade() {
                show_help_dialog(&body);
            }
        });
    }
    action_group.add_action(&act_help);

    // 헤더 ? 버튼 — 같은 다이얼로그를 직접 호출.
    {
        let body_weak = body.downgrade();
        btn_help.connect_clicked(move |_| {
            if let Some(body) = body_weak.upgrade() {
                show_help_dialog(&body);
            }
        });
    }

    // 사용자 정의 지문 가져오기 — 파일 / 클립보드.
    let act_import_file = gio::SimpleAction::new("import-corpus-file", None);
    {
        let body_weak = body.downgrade();
        let dropdown = corpus_dropdown.clone();
        let entries = corpus_entries.clone();
        let toast_c = toast.clone();
        act_import_file.connect_activate(move |_, _| {
            if let Some(body) = body_weak.upgrade() {
                pick_corpus_file(&body, dropdown.clone(), entries.clone(), toast_c.clone());
            }
        });
    }
    action_group.add_action(&act_import_file);

    let act_import_clip = gio::SimpleAction::new("import-corpus-clipboard", None);
    {
        let dropdown = corpus_dropdown.clone();
        let entries = corpus_entries.clone();
        let toast_c = toast.clone();
        act_import_clip.connect_activate(move |_, _| {
            import_corpus_from_clipboard(dropdown.clone(), entries.clone(), toast_c.clone());
        });
    }
    action_group.add_action(&act_import_clip);

    // 이름 변경/삭제 액션 제거 — 드롭다운 호버 아이콘으로 직접 다이얼로그 호출.

    body.insert_action_group("win", Some(&action_group));
    // 헤더는 body 외부 위젯이라 별도 등록 — 안 하면 헤더의 메뉴/버튼이 비활성화.
    header.insert_action_group("win", Some(&action_group));

    // 단축키 — Managed scope 으로 윈도우 전체에서 동작.
    {
        let sc = gtk::ShortcutController::new();
        sc.set_scope(gtk::ShortcutScope::Managed);
        let bindings = [
            ("F1", "win.help"),
            ("<Primary>r", "win.restart"),
            ("<Primary><Shift>c", "win.copy-result"),
            ("<Primary>1", "win.show-practice"),
            ("<Primary>2", "win.show-result"),
            ("<Primary>o", "win.import-corpus-file"),
            ("<Primary><Shift>v", "win.import-corpus-clipboard"),
        ];
        for (trigger, action) in bindings {
            sc.add_shortcut(gtk::Shortcut::new(
                gtk::ShortcutTrigger::parse_string(trigger),
                Some(gtk::NamedAction::new(action)),
            ));
        }
        body.add_controller(sc);
    }

    // 탭 전환 시 자동 포커스 — view_switcher 직접 클릭, act_show_practice 액션,
    // 시작 시 모두 동일 경로로 input_entry 에 포커스.
    {
        let input_entry_c = input_entry.clone();
        view_stack.connect_visible_child_notify(move |stack| {
            if stack.visible_child_name().as_deref() == Some("practice") {
                let input_entry_c = input_entry_c.clone();
                // 탭 전환 직후 idle 에서 grab — 위젯 realize 보장.
                glib::idle_add_local_once(move || {
                    input_entry_c.grab_focus();
                });
            }
        });
    }

    // 자판 변경 reload — UNIM 설정에서 한·영 자판이 바뀌면 위젯/세션을 갈아끼움.
    let do_reload_layout: Rc<dyn Fn()> = {
        let title_widget = title_widget.clone();
        let res_title = res_title.clone();
        let kbd_card_inner = kbd_card_inner.clone();
        let heat_card_inner = heat_card_inner.clone();
        let kbd_view_slot = kbd_view_slot.clone();
        let heat_view_slot = heat_view_slot.clone();
        let profile_slot = profile_slot.clone();
        let layout_code_slot = layout_code_slot.clone();
        let en_code_slot = en_code_slot.clone();
        let en_profile_slot = en_profile_slot.clone();
        let start_session = start_session.clone();
        let toast = toast.clone();
        Rc::new(move || {
            let (real_ko, new_profile) = active_layout::load_active_profile();
            let new_en = active_layout::read_english_layout_name();
            let display_name = profile_display_name(&new_profile, &real_ko);
            let ko_part = if display_name == real_ko {
                real_ko.clone()
            } else {
                format!("{} · {}", display_name, real_ko)
            };
            let en_label = active_layout::english_layout_label(&new_en);
            let en_part = if en_label == new_en {
                new_en.clone()
            } else {
                format!("{} · {}", en_label, new_en)
            };
            let subtitle = format!("{ko_part} / {en_part}");

            title_widget.set_subtitle(&subtitle);
            res_title.set_text(&subtitle);

            // 새 영문 profile 도 함께 로드 (자판 변경된 한·영 모두 반영).
            let (_, new_en_profile) = active_layout::load_active_english_profile();
            let new_kbd = KeyboardView::new(Some(&new_profile), new_en_profile.as_ref());
            {
                let old = kbd_view_slot.borrow();
                kbd_card_inner.remove(old.root());
            }
            kbd_card_inner.append(new_kbd.root());
            *kbd_view_slot.borrow_mut() = new_kbd;

            let new_heat = KeyboardView::new(Some(&new_profile), new_en_profile.as_ref());
            {
                let old = heat_view_slot.borrow();
                heat_card_inner.remove(old.root());
            }
            heat_card_inner.append(new_heat.root());
            *heat_view_slot.borrow_mut() = new_heat;

            *profile_slot.borrow_mut() = new_profile;
            *layout_code_slot.borrow_mut() = real_ko;
            *en_code_slot.borrow_mut() = new_en;
            *en_profile_slot.borrow_mut() = new_en_profile;

            toast.add_toast(adw::Toast::new(&rust_i18n::t!(
                "toast_layout_changed",
                label = subtitle
            )));
            start_session();
        })
    };

    // Polling — 2초마다 데몬에 활성 자판 조회, 변경 시 reload.
    {
        let layout_code_slot = layout_code_slot.clone();
        let en_code_slot = en_code_slot.clone();
        let do_reload = do_reload_layout.clone();
        glib::timeout_add_seconds_local(2, move || {
            let new_ko = active_layout::read_korean_layout_name();
            let new_en = active_layout::read_english_layout_name();
            let cur_ko = layout_code_slot.borrow().clone();
            let cur_en = en_code_slot.borrow().clone();
            if new_ko != cur_ko || new_en != cur_en {
                do_reload();
            }
            glib::ControlFlow::Continue
        });
    }

    // 첫 세션 자동 시작.
    start_session();

    // 첫 진입 자동 포커스 — window.present 후 위젯 realize/map 완료를 위해
    // 짧은 timeout 으로 1회 추가. start_session 내부 grab_focus 는 위젯 미실현
    // 단계에서 호출돼 무시될 수 있다.
    {
        let input_entry_c = input_entry.clone();
        glib::timeout_add_local_once(std::time::Duration::from_millis(150), move || {
            input_entry_c.grab_focus();
        });
    }

    body.upcast()
}

// =====================================================================
// 헬퍼들
// =====================================================================

fn format_stats(s: &PracticeSession) -> (String, String, String, String) {
    (
        format!("{:.0}", s.stats.wpm()),
        format!("{:.0}", s.stats.cpm()),
        format!("{:.0}", s.stats.accuracy()),
        format!("{:.0}", s.stats.error_rate()),
    )
}

fn format_duration(secs: f64) -> String {
    let s = secs.round() as u64;
    format!("{:02}:{:02}", s / 60, s % 60)
}

fn line_position_text(idx: usize, total: usize) -> String {
    format!("{} / {}", idx + 1, total)
}

/// 줄 완료 조건 — 자동(`force=false`) 또는 강제(`force=true`).
///
/// 자동: `input == target` (정확 일치) 또는
///   `input` 단어 개수 ≥ `target` 단어 개수이고 `input` 이 whitespace 로 끝남 (Space 트리거).
/// 강제: 무조건 true (Enter/Tab 명시 종료).
fn check_line_done(target: &str, input: &str, force: bool) -> bool {
    if force {
        return true;
    }
    if input == target {
        return true;
    }
    let tw_count = target.split_whitespace().count();
    let iw_count = input.split_whitespace().count();
    tw_count > 0
        && iw_count >= tw_count
        && input.ends_with(|c: char| c.is_whitespace())
}

/// 한 줄이 `max_chars` 보다 길면 띄어쓰기 단위로 잘라 여러 줄로 분할.
/// 띄어쓰기가 없는 long word 도 max_chars 단위로 강제 분할.
fn split_long_line(line: &str, max_chars: usize) -> Vec<String> {
    if line.chars().count() <= max_chars {
        return vec![line.to_string()];
    }
    let mut out: Vec<String> = Vec::new();
    let mut current = String::new();
    for word in line.split_whitespace() {
        let word_chars = word.chars().count();
        let cur_chars = current.chars().count();
        let separator = if current.is_empty() { 0 } else { 1 };
        if !current.is_empty() && cur_chars + separator + word_chars > max_chars {
            out.push(std::mem::take(&mut current));
        }
        if !current.is_empty() {
            current.push(' ');
        }
        current.push_str(word);
        // word 자체가 max_chars 초과 — 강제 분할.
        while current.chars().count() > max_chars {
            let taken: String = current.chars().take(max_chars).collect();
            out.push(taken);
            current = current.chars().skip(max_chars).collect();
        }
    }
    if !current.is_empty() {
        out.push(current);
    }
    if out.is_empty() {
        out.push(line.to_string());
    }
    out
}

fn profile_display_name(
    profile: &unim::keystroke::profile::LayoutProfile,
    fallback: &str,
) -> String {
    let n = profile.name.trim();
    if !n.is_empty() {
        return n.to_string();
    }
    fallback.to_string()
}

/// `.typing-card` 카드 + 내부 padding (좌우 14, 상하 vp).
fn make_card<W: IsA<gtk::Widget>>(child: &W, vp: i32, hp: i32) -> gtk::Frame {
    let f = gtk::Frame::new(None);
    f.add_css_class("typing-card");
    f.set_child(Some(child));
    if let Some(inner) = f.child() {
        inner.set_margin_top(vp);
        inner.set_margin_bottom(vp);
        inner.set_margin_start(hp);
        inner.set_margin_end(hp);
    }
    f
}

/// StatCell (우 컬럼 2×2).
fn make_stat_cell(label: &str, unit: &str, value_class: Option<&str>) -> (gtk::Box, gtk::Label) {
    let b = gtk::Box::new(gtk::Orientation::Vertical, 2);
    b.set_halign(gtk::Align::Start);
    b.set_valign(gtk::Align::Center);

    let mut val_classes: Vec<&str> = vec!["typing-stat-value"];
    if let Some(c) = value_class {
        val_classes.push(c);
    }
    let val = gtk::Label::builder()
        .label("0")
        .css_classes(val_classes)
        .xalign(0.0)
        .build();
    let unit_lab = gtk::Label::builder()
        .label(unit)
        .css_classes(["typing-stat-unit"])
        .build();
    let val_row = gtk::Box::new(gtk::Orientation::Horizontal, 4);
    val_row.set_valign(gtk::Align::Baseline);
    val_row.append(&val);
    val_row.append(&unit_lab);

    let lab = gtk::Label::builder()
        .label(label)
        .css_classes(["typing-stat-label"])
        .xalign(0.0)
        .build();
    b.append(&val_row);
    b.append(&lab);
    (b, val)
}

/// BigStat (결과 페이지).
fn make_big_stat(label: &str, unit: &str, value_class: Option<&str>) -> (gtk::Box, gtk::Label) {
    let b = gtk::Box::new(gtk::Orientation::Vertical, 4);
    b.set_halign(gtk::Align::Center);
    b.set_valign(gtk::Align::Center);

    let mut val_classes: Vec<&str> = vec!["typing-big-value"];
    if let Some(c) = value_class {
        val_classes.push(c);
    }
    let val = gtk::Label::builder()
        .label("—")
        .css_classes(val_classes)
        .build();
    let unit_lab = gtk::Label::builder()
        .label(unit)
        .css_classes(["typing-big-unit"])
        .build();
    let val_row = gtk::Box::new(gtk::Orientation::Horizontal, 6);
    val_row.set_valign(gtk::Align::Baseline);
    val_row.set_halign(gtk::Align::Center);
    val_row.append(&val);
    val_row.append(&unit_lab);

    let lab = gtk::Label::builder()
        .label(label)
        .css_classes(["typing-big-label"])
        .build();
    b.append(&val_row);
    b.append(&lab);
    (b, val)
}

/// BigStatsCard 셀 사이 vertical divider.
fn make_v_divider() -> gtk::Separator {
    let sep = gtk::Separator::new(gtk::Orientation::Vertical);
    sep.set_margin_top(8);
    sep.set_margin_bottom(8);
    sep
}

/// KeyCount row — label ↔ value.
fn make_key_count_row(label: &str, wrong: bool) -> (gtk::Box, gtk::Label) {
    let row = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    row.set_valign(gtk::Align::Baseline);
    let lab = gtk::Label::builder()
        .label(label)
        .css_classes(["dim-label"])
        .xalign(0.0)
        .hexpand(true)
        .build();
    let mut val_classes: Vec<&str> = vec!["typing-numeric-small"];
    if wrong {
        val_classes.push("typing-stat-wrong");
    }
    let val = gtk::Label::builder()
        .label("—")
        .css_classes(val_classes)
        .xalign(1.0)
        .build();
    row.append(&lab);
    row.append(&val);
    (row, val)
}

// =====================================================================
// TargetLine 위젯
// =====================================================================

pub struct TargetLineWidget {
    pub row: gtk::Box,
    pub spinner: Rc<TriangleSpinner>,
    pub check: gtk::Image,
    pub label: gtk::Label,
}

fn make_target_line_widget() -> TargetLineWidget {
    let spinner = TriangleSpinner::new();
    spinner.root().set_visible(false);

    let check = gtk::Image::from_icon_name("emblem-ok-symbolic");
    check.set_halign(gtk::Align::Center);
    check.set_valign(gtk::Align::Center);
    check.set_pixel_size(12);
    check.add_css_class("typing-line-check");
    check.set_visible(false);

    let marker_box = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    marker_box.set_size_request(20, -1);
    marker_box.set_halign(gtk::Align::Center);
    marker_box.set_valign(gtk::Align::Center);
    marker_box.append(spinner.root());
    marker_box.append(&check);

    let label = gtk::Label::builder()
        .label("")
        .css_classes(["typing-target-line-label"])
        .halign(gtk::Align::Start)
        .xalign(0.0)
        .use_markup(true)
        .wrap(true)
        .wrap_mode(gtk::pango::WrapMode::WordChar)
        .single_line_mode(false)
        .build();
    label.set_hexpand(true);
    // 줄간격 130 % 은 build_current_line_markup / dim markup 안의 outer
    // `<span line_height='1.3'>` (Pango 1.50+) 으로 적용된다.

    let row = gtk::Box::new(gtk::Orientation::Horizontal, 10);
    row.add_css_class("typing-target-line");
    row.append(&marker_box);
    row.append(&label);

    TargetLineWidget {
        row,
        spinner,
        check,
        label,
    }
}

fn rebuild_target_lines(
    container: &gtk::Box,
    widgets: &Rc<RefCell<Vec<TargetLineWidget>>>,
    line_count: usize,
) {
    while let Some(c) = container.first_child() {
        container.remove(&c);
    }
    let mut vec = widgets.borrow_mut();
    vec.clear();
    for _ in 0..line_count {
        let w = make_target_line_widget();
        container.append(&w.row);
        vec.push(w);
    }
}

fn paint_target_lines(
    widgets: &[TargetLineWidget],
    sess: &PracticeSession,
    lines: &[String],
    cur_line: usize,
    all_done: bool,
    sw: &gtk::ScrolledWindow,
) {
    for (i, w) in widgets.iter().enumerate() {
        let line_text = lines.get(i).map(String::as_str).unwrap_or("");
        let esc = glib::markup_escape_text(line_text);

        if all_done || i < cur_line {
            // done 줄 — 그 줄의 final input 으로 오타/정타 회고 표시.
            w.row.remove_css_class("typing-line-current");
            let final_input = sess.line_inputs.get(i).map(String::as_str).unwrap_or("");
            w.label
                .set_markup(&build_word_aware_markup(line_text, final_input));
            w.spinner.root().set_visible(false);
            w.spinner.stop();
            w.check.set_visible(true);
        } else if i == cur_line {
            w.row.add_css_class("typing-line-current");
            w.label
                .set_markup(&build_word_aware_markup(line_text, sess.last_input()));
            w.spinner.root().set_visible(true);
            w.spinner.start();
            w.check.set_visible(false);
        } else {
            // pending 줄 — dim.
            w.row.remove_css_class("typing-line-current");
            w.label.set_markup(&format!(
                "<span line_height='1.3' foreground='{}'>{}</span>",
                COLOR_DIM, esc
            ));
            w.spinner.root().set_visible(false);
            w.spinner.stop();
            w.check.set_visible(false);
        }
    }

    // 자동 스크롤 — 현재 줄 + (있으면) 다음 줄까지 viewport 안에 보이도록.
    // layout 끝난 뒤에 측정해야 정확 → idle_add 로 1 frame 지연.
    if !all_done && cur_line < widgets.len() {
        let sw_c = sw.clone();
        let cur_row = widgets[cur_line].row.clone();
        let next_idx = (cur_line + 1).min(widgets.len() - 1);
        let next_row = widgets[next_idx].row.clone();
        glib::idle_add_local_once(move || {
            ensure_visible(&sw_c, &cur_row, &next_row);
        });
    }
}

/// 현재 줄 + 다음 줄이 viewport 안에 보이도록 vadjustment 조정.
fn ensure_visible(sw: &gtk::ScrolledWindow, cur_row: &gtk::Box, next_row: &gtk::Box) {
    let viewport_h = sw.height() as f64;
    if viewport_h <= 0.0 {
        return;
    }
    let vadj = sw.vadjustment();
    let v = vadj.value();
    // cur_row 의 sw 좌표계 top.
    let cur_top = cur_row
        .translate_coordinates(sw, 0.0, 0.0)
        .map(|(_, y)| y)
        .unwrap_or(0.0);
    // next_row 의 sw 좌표계 top + height = bottom.
    let next_top = next_row
        .translate_coordinates(sw, 0.0, 0.0)
        .map(|(_, y)| y)
        .unwrap_or(0.0);
    let next_h = next_row.height() as f64;
    let next_bottom = next_top + next_h;

    if cur_top < 0.0 {
        vadj.set_value((v + cur_top).max(0.0));
    } else if next_bottom > viewport_h {
        let upper = vadj.upper() - viewport_h;
        vadj.set_value((v + (next_bottom - viewport_h)).min(upper.max(0.0)));
    }
}

/// 단어 단위 + greedy 매칭 markup.
///
/// target/input 둘 다 띄어쓰기 단위 split → i-th input word 를 i-th target word 에
/// greedy subsequence 매칭 (`align_input_to_target`). 각 target 글자별:
/// - 매칭됨 → 파랑(600w)
/// - 매칭 안 됨 + 해당 input word 가 입력됨 → 주황 wavy(600w)
/// - 해당 input word 미입력 → dim
///
/// 띄어쓰기 자체는 dim.
fn build_word_aware_markup(target_text: &str, input_text: &str) -> String {
    let target_words: Vec<&str> = target_text.split_whitespace().collect();
    let input_words: Vec<&str> = input_text.split_whitespace().collect();

    // 각 target 단어에 대해 char-level matched 결과 미리 계산.
    let word_matches: Vec<Vec<bool>> = target_words
        .iter()
        .enumerate()
        .map(|(i, tw)| {
            let target_chars: Vec<char> = tw.chars().collect();
            match input_words.get(i).filter(|w| !w.is_empty()) {
                Some(iw) => {
                    let input_chars: Vec<char> = iw.chars().collect();
                    align_input_to_target(&target_chars, &input_chars)
                }
                None => vec![false; target_chars.len()],
            }
        })
        .collect();

    let mut out = String::with_capacity(target_text.len() * 5 + 32);
    out.push_str("<span line_height='1.3'>");

    let mut word_idx: usize = 0;
    let mut char_in_word: usize = 0;
    let mut in_word = false;

    for ch in target_text.chars() {
        let s = ch.to_string();
        let esc = glib::markup_escape_text(&s);
        if ch.is_whitespace() {
            if in_word {
                word_idx += 1;
                char_in_word = 0;
                in_word = false;
            }
            out.push_str(&format!(
                "<span foreground='{}'>{}</span>",
                COLOR_DIM, esc
            ));
            continue;
        }
        in_word = true;
        let iw_present = input_words
            .get(word_idx)
            .map(|w| !w.is_empty())
            .unwrap_or(false);
        let markup = if !iw_present {
            // 단어 미입력 → dim.
            format!("<span foreground='{}'>{}</span>", COLOR_DIM, esc)
        } else if word_matches
            .get(word_idx)
            .and_then(|m| m.get(char_in_word))
            .copied()
            .unwrap_or(false)
        {
            format!(
                "<span foreground='{}' weight='600'>{}</span>",
                COLOR_CORRECT, esc
            )
        } else {
            format!(
                "<span foreground='{}' weight='600' underline='error' underline_color='{}'>{}</span>",
                COLOR_WRONG, COLOR_WRONG, esc
            )
        };
        out.push_str(&markup);
        char_in_word += 1;
    }
    out.push_str("</span>");
    out
}

// =====================================================================
// Sparkline (DurationCard)
// =====================================================================

fn draw_sparkline(cr: &gtk::cairo::Context, w: f64, h: f64, data: &[f64]) {
    if data.len() < 2 {
        return;
    }
    let pad = 4.0;
    let cw = w - pad * 2.0;
    let ch = h - pad * 2.0;
    let max = data.iter().cloned().fold(0.0_f64, f64::max).max(1.0);
    let dx = cw / (data.len() - 1) as f64;
    let yof = |v: f64| pad + ch - (v / max).clamp(0.0, 1.0) * ch;

    // 영역.
    cr.move_to(pad, pad + ch);
    for (i, v) in data.iter().enumerate() {
        let x = pad + i as f64 * dx;
        cr.line_to(x, yof(*v));
    }
    cr.line_to(pad + cw, pad + ch);
    cr.close_path();
    cr.set_source_rgba(COLOR_ACCENT_F.0, COLOR_ACCENT_F.1, COLOR_ACCENT_F.2, 0.25);
    let _ = cr.fill();

    // 라인.
    cr.move_to(pad, yof(data[0]));
    for (i, v) in data.iter().enumerate().skip(1) {
        cr.line_to(pad + i as f64 * dx, yof(*v));
    }
    cr.set_source_rgba(COLOR_ACCENT_F.0, COLOR_ACCENT_F.1, COLOR_ACCENT_F.2, 1.0);
    cr.set_line_width(1.5);
    let _ = cr.stroke();

    // 마지막 점.
    let last = *data.last().unwrap();
    let lx = pad + (data.len() - 1) as f64 * dx;
    cr.arc(lx, yof(last), 1.8, 0.0, 2.0 * PI);
    cr.set_source_rgba(COLOR_ACCENT_F.0, COLOR_ACCENT_F.1, COLOR_ACCENT_F.2, 1.0);
    let _ = cr.fill();
}

// =====================================================================
// 도움말 다이얼로그 (F1 또는 헤더 ?)
// =====================================================================

fn show_help_dialog(parent: &impl IsA<gtk::Widget>) {
    let dialog = adw::MessageDialog::builder()
        .heading(&*rust_i18n::t!("help_dialog_title"))
        .body(&*rust_i18n::t!("help_dialog_intro"))
        .modal(true)
        .build();
    if let Some(root) = parent.root() {
        if let Ok(window) = root.downcast::<gtk::Window>() {
            dialog.set_transient_for(Some(&window));
        }
    }

    // 단축키 표.
    let head = gtk::Label::builder()
        .label(rust_i18n::t!("help_dialog_shortcuts"))
        .css_classes(["heading"])
        .xalign(0.0)
        .build();
    let grid = gtk::Grid::builder()
        .row_spacing(8)
        .column_spacing(18)
        .build();
    let shortcuts: [(&str, &str); 7] = [
        ("F1", "help_shortcut_help"),
        ("Ctrl + R", "help_shortcut_restart"),
        ("Ctrl + Shift + C", "help_shortcut_copy"),
        ("Ctrl + 1", "help_shortcut_practice"),
        ("Ctrl + 2", "help_shortcut_result"),
        ("Ctrl + O", "help_shortcut_import_file"),
        ("Ctrl + Shift + V", "help_shortcut_import_clip"),
    ];
    for (i, (key, label_key)) in shortcuts.iter().enumerate() {
        let chip = gtk::Label::builder()
            .label(*key)
            .css_classes(["typing-ime-chip"])
            .halign(gtk::Align::Start)
            .build();
        let desc = gtk::Label::builder()
            .label(&*rust_i18n::t!(*label_key))
            .xalign(0.0)
            .hexpand(true)
            .build();
        grid.attach(&chip, 0, i as i32, 1, 1);
        grid.attach(&desc, 1, i as i32, 1, 1);
    }

    let extra = gtk::Box::new(gtk::Orientation::Vertical, 10);
    extra.set_margin_top(10);
    extra.append(&head);
    extra.append(&grid);
    dialog.set_extra_child(Some(&extra));

    dialog.add_response("close", &rust_i18n::t!("help_dialog_close"));
    dialog.set_default_response(Some("close"));
    dialog.set_close_response("close");
    dialog.connect_response(None, |dlg, _| dlg.close());
    dialog.present();
}

// =====================================================================
// 사용자 정의 지문 — 가져오기/드롭다운 갱신 (corpus.rs 와 짝)
// =====================================================================

/// 드롭다운 모델을 빌트인 3 + 사용자 N 으로 재구성.
/// `select_name = Some(name)` 이면 그 사용자 corpus 를 선택, None 이면 0 번.
///
/// 주의: `dropdown.set_model()` 은 selected-notify 시그널을 발화시키고 그 핸들러가
/// `entries.borrow()` 를 시도하므로, 본 함수는 borrow_mut 을 먼저 drop 한 뒤
/// 모델/선택을 설정한다.
fn rebuild_corpus_dropdown(
    dropdown: &gtk::DropDown,
    entries: &Rc<RefCell<Vec<CorpusEntry>>>,
    select_name: Option<&str>,
) {
    let (labels, target_idx) = {
        let mut entries = entries.borrow_mut();
        entries.clear();
        for k in CorpusKind::all() {
            entries.push(CorpusEntry::Builtin(k));
        }
        for u in corpus::list_user_corpora() {
            entries.push(CorpusEntry::User(u));
        }
        let labels: Vec<String> = entries.iter().map(|e| e.display_label()).collect();
        let mut idx: u32 = 0;
        if let Some(name) = select_name {
            for (i, e) in entries.iter().enumerate() {
                if let CorpusEntry::User(u) = e {
                    if u.name == name {
                        idx = i as u32;
                        break;
                    }
                }
            }
        }
        (labels, idx)
    };
    let strs: Vec<&str> = labels.iter().map(String::as_str).collect();
    let model = gtk::StringList::new(&strs);
    dropdown.set_model(Some(&model));
    dropdown.set_selected(target_idx);
}

/// 가져온 텍스트를 저장하고 드롭다운 갱신. 토스트로 결과 알림.
fn ingest_user_text(
    name_hint: &str,
    text: &str,
    dropdown: &gtk::DropDown,
    entries: &Rc<RefCell<Vec<CorpusEntry>>>,
    toast: &adw::ToastOverlay,
) {
    match corpus::save_user_corpus(name_hint, text) {
        Ok(saved) => {
            rebuild_corpus_dropdown(dropdown, entries, Some(&saved.name));
            toast.add_toast(adw::Toast::new(&rust_i18n::t!(
                "toast_corpus_imported",
                name = saved.name.clone()
            )));
        }
        Err(msg) => {
            toast.add_toast(adw::Toast::new(&rust_i18n::t!(
                "toast_corpus_import_failed",
                reason = msg
            )));
        }
    }
}

/// 파일 선택기 → 텍스트 읽기 → ingest_user_text.
fn pick_corpus_file(
    parent: &impl IsA<gtk::Widget>,
    dropdown: gtk::DropDown,
    entries: Rc<RefCell<Vec<CorpusEntry>>>,
    toast: adw::ToastOverlay,
) {
    let filter = gtk::FileFilter::new();
    filter.set_name(Some("Text files (*.txt)"));
    filter.add_pattern("*.txt");
    let filters = gio::ListStore::new::<gtk::FileFilter>();
    filters.append(&filter);

    let dialog = gtk::FileDialog::builder()
        .title(&*rust_i18n::t!("dialog_pick_corpus_title"))
        .modal(true)
        .filters(&filters)
        .build();
    let parent_window = parent
        .as_ref()
        .root()
        .and_then(|r| r.downcast::<gtk::Window>().ok());
    let toast_c = toast.clone();
    dialog.open(parent_window.as_ref(), gio::Cancellable::NONE, move |res| {
        let Ok(file) = res else { return };
        let Some(path) = file.path() else { return };
        let raw = match std::fs::read_to_string(&path) {
            Ok(s) => s,
            Err(e) => {
                toast_c.add_toast(adw::Toast::new(&rust_i18n::t!(
                    "toast_corpus_import_failed",
                    reason = e.to_string()
                )));
                return;
            }
        };
        let name = path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("imported")
            .to_string();
        ingest_user_text(&name, &raw, &dropdown, &entries, &toast_c);
    });
}

/// 클립보드 → ingest_user_text.
fn import_corpus_from_clipboard(
    dropdown: gtk::DropDown,
    entries: Rc<RefCell<Vec<CorpusEntry>>>,
    toast: adw::ToastOverlay,
) {
    let Some(display) = gdk::Display::default() else {
        return;
    };
    let clipboard = display.clipboard();
    let toast_c = toast.clone();
    clipboard.read_text_async(gio::Cancellable::NONE, move |res| {
        let text = match res {
            Ok(Some(t)) if !t.trim().is_empty() => t.to_string(),
            _ => {
                toast_c.add_toast(adw::Toast::new(&rust_i18n::t!(
                    "toast_corpus_import_failed",
                    reason = format!("empty clipboard (limit {} bytes)", USER_CORPUS_MAX_BYTES)
                )));
                return;
            }
        };
        let now = glib::DateTime::now_local()
            .map(|d| d.format("%Y%m%d_%H%M%S").map(|s| s.to_string()).ok())
            .ok()
            .flatten()
            .unwrap_or_else(|| "clip".to_string());
        let name = format!("clipboard_{}", now);
        ingest_user_text(&name, &text, &dropdown, &entries, &toast_c);
    });
}

/// 현재 선택된 사용자 corpus 이름 변경 다이얼로그.
fn show_rename_dialog(
    parent: &impl IsA<gtk::Widget>,
    corpus: corpus::UserCorpus,
    dropdown: gtk::DropDown,
    entries: Rc<RefCell<Vec<CorpusEntry>>>,
    toast: adw::ToastOverlay,
) {
    let dialog = adw::MessageDialog::builder()
        .heading(&*rust_i18n::t!("dialog_rename_title"))
        .body(&*rust_i18n::t!("dialog_rename_body"))
        .modal(true)
        .build();
    if let Some(root) = parent.as_ref().root() {
        if let Ok(window) = root.downcast::<gtk::Window>() {
            dialog.set_transient_for(Some(&window));
        }
    }

    let entry = gtk::Entry::builder()
        .text(&corpus.name)
        .placeholder_text(&*rust_i18n::t!("dialog_rename_placeholder"))
        .activates_default(true)
        .build();
    let extra = gtk::Box::new(gtk::Orientation::Vertical, 8);
    extra.set_margin_top(8);
    extra.append(&entry);
    dialog.set_extra_child(Some(&extra));

    dialog.add_response("cancel", &rust_i18n::t!("dialog_cancel"));
    dialog.add_response("rename", &rust_i18n::t!("dialog_rename_btn"));
    dialog.set_response_appearance("rename", adw::ResponseAppearance::Suggested);
    dialog.set_default_response(Some("rename"));
    dialog.set_close_response("cancel");

    let entry_c = entry.clone();
    let old = corpus.clone();
    let toast_c = toast.clone();
    dialog.connect_response(None, move |dlg, resp| {
        if resp == "rename" {
            let new_name = entry_c.text().to_string();
            match corpus::rename_user_corpus(&old, &new_name) {
                Ok(renamed) => {
                    rebuild_corpus_dropdown(&dropdown, &entries, Some(&renamed.name));
                    toast_c.add_toast(adw::Toast::new(&rust_i18n::t!(
                        "toast_corpus_renamed",
                        old = old.name.clone(),
                        new = renamed.name.clone()
                    )));
                }
                Err(msg) => {
                    toast_c.add_toast(adw::Toast::new(&rust_i18n::t!(
                        "toast_corpus_rename_failed",
                        reason = msg
                    )));
                }
            }
        }
        dlg.close();
    });
    dialog.present();
}

/// 현재 선택된 사용자 corpus 삭제 확인 다이얼로그.
fn show_delete_dialog(
    parent: &impl IsA<gtk::Widget>,
    corpus: corpus::UserCorpus,
    dropdown: gtk::DropDown,
    entries: Rc<RefCell<Vec<CorpusEntry>>>,
    toast: adw::ToastOverlay,
) {
    let dialog = adw::MessageDialog::builder()
        .heading(&*rust_i18n::t!("dialog_delete_title"))
        .body(&*rust_i18n::t!(
            "dialog_delete_body",
            name = corpus.name.clone()
        ))
        .modal(true)
        .build();
    if let Some(root) = parent.as_ref().root() {
        if let Ok(window) = root.downcast::<gtk::Window>() {
            dialog.set_transient_for(Some(&window));
        }
    }

    dialog.add_response("cancel", &rust_i18n::t!("dialog_cancel"));
    dialog.add_response("delete", &rust_i18n::t!("dialog_delete_btn"));
    dialog.set_response_appearance("delete", adw::ResponseAppearance::Destructive);
    dialog.set_default_response(Some("cancel"));
    dialog.set_close_response("cancel");

    let toast_c = toast.clone();
    let target = corpus.clone();
    dialog.connect_response(None, move |dlg, resp| {
        if resp == "delete" {
            match corpus::delete_user_corpus(&target) {
                Ok(_) => {
                    rebuild_corpus_dropdown(&dropdown, &entries, None);
                    toast_c.add_toast(adw::Toast::new(&rust_i18n::t!(
                        "toast_corpus_deleted",
                        name = target.name.clone()
                    )));
                }
                Err(msg) => {
                    toast_c.add_toast(adw::Toast::new(&rust_i18n::t!(
                        "toast_corpus_delete_failed",
                        reason = msg
                    )));
                }
            }
        }
        dlg.close();
    });
    dialog.present();
}

// =====================================================================
// 코퍼스 드롭다운 — ListItem factory (호버 시 이름변경/삭제 아이콘)
// =====================================================================

/// 헤더에 표시되는 현재 선택 위젯 — 단순 라벨만.
fn make_corpus_main_factory(
    entries: Rc<RefCell<Vec<CorpusEntry>>>,
) -> gtk::SignalListItemFactory {
    let factory = gtk::SignalListItemFactory::new();
    factory.connect_setup(|_, list_item| {
        let li = list_item.downcast_ref::<gtk::ListItem>().unwrap();
        let label = gtk::Label::builder().xalign(0.0).build();
        li.set_child(Some(&label));
    });
    factory.connect_bind(move |_, list_item| {
        let li = list_item.downcast_ref::<gtk::ListItem>().unwrap();
        let pos = li.position() as usize;
        let text = entries
            .borrow()
            .get(pos)
            .map(|e| e.display_label())
            .unwrap_or_default();
        if let Some(label) = li.child().and_downcast::<gtk::Label>() {
            label.set_text(&text);
        }
    });
    factory
}

/// 팝오버 리스트 — 사용자 코퍼스 행에 호버 시 이름변경/삭제 아이콘 노출.
fn make_corpus_list_factory(
    entries: Rc<RefCell<Vec<CorpusEntry>>>,
    dropdown: gtk::DropDown,
    toast: adw::ToastOverlay,
) -> gtk::SignalListItemFactory {
    let factory = gtk::SignalListItemFactory::new();
    factory.connect_bind(move |_, list_item| {
        let li = list_item.downcast_ref::<gtk::ListItem>().unwrap();
        let pos = li.position() as usize;
        let entry = entries.borrow().get(pos).cloned();

        let row = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        row.add_css_class("typing-corpus-row");
        let label = gtk::Label::builder()
            .label(
                entry
                    .as_ref()
                    .map(|e| e.display_label())
                    .unwrap_or_default(),
            )
            .xalign(0.0)
            .hexpand(true)
            .build();
        row.append(&label);

        if let Some(CorpusEntry::User(u)) = entry {
            let btn_rename = gtk::Button::from_icon_name("document-edit-symbolic");
            btn_rename.add_css_class("flat");
            btn_rename.add_css_class("typing-row-action");
            btn_rename.set_tooltip_text(Some(&rust_i18n::t!("dialog_rename_title")));
            {
                let dropdown = dropdown.clone();
                let entries = entries.clone();
                let toast = toast.clone();
                let u = u.clone();
                btn_rename.connect_clicked(move |btn| {
                    close_dropdown_popover(&dropdown);
                    show_rename_dialog(
                        btn,
                        u.clone(),
                        dropdown.clone(),
                        entries.clone(),
                        toast.clone(),
                    );
                });
            }
            row.append(&btn_rename);

            let btn_delete = gtk::Button::from_icon_name("user-trash-symbolic");
            btn_delete.add_css_class("flat");
            btn_delete.add_css_class("typing-row-action");
            btn_delete.add_css_class("typing-row-action-danger");
            btn_delete.set_tooltip_text(Some(&rust_i18n::t!("dialog_delete_title")));
            {
                let dropdown = dropdown.clone();
                let entries = entries.clone();
                let toast = toast.clone();
                btn_delete.connect_clicked(move |btn| {
                    close_dropdown_popover(&dropdown);
                    show_delete_dialog(
                        btn,
                        u.clone(),
                        dropdown.clone(),
                        entries.clone(),
                        toast.clone(),
                    );
                });
            }
            row.append(&btn_delete);
        }

        li.set_child(Some(&row));
    });
    factory
}

/// GtkDropDown 내부 위젯 트리에서 첫 Popover 를 찾아 popdown.
/// GtkDropDown 은 GTK 4.10 부터 직접 `popdown()` 메서드가 있으나 trait gating 이슈로
/// 동적 walk 가 가장 안전.
fn close_dropdown_popover(dropdown: &gtk::DropDown) {
    if let Some(pop) = find_first_popover(dropdown.upcast_ref::<gtk::Widget>()) {
        pop.popdown();
    }
}

fn find_first_popover(w: &gtk::Widget) -> Option<gtk::Popover> {
    let mut child = w.first_child();
    while let Some(c) = child {
        if let Ok(pop) = c.clone().downcast::<gtk::Popover>() {
            return Some(pop);
        }
        if let Some(p) = find_first_popover(&c) {
            return Some(p);
        }
        child = c.next_sibling();
    }
    None
}
