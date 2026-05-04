//! 이모지 팝업 윈도우
//!
//! DBus `ShowEmojiPopupV2` 시그널을 받아 9×9 그리드 + 좌측 세로 9 탭으로 이모지를
//! 표시합니다. 키 처리는 엔진(`process_popup_key`)이 담당하고, GUI 는 `PopupNavigate`
//! 시그널만 받아 페이지/셀을 갱신합니다 — 한자/특수문자 팝업과 동일 패턴.
//!
//! POPUP_SPEC.md 규격 준수:
//! - 9×9 고정 grid (FlowBox 폐기)
//! - 좌측 세로 9 탭 (`최근/표정·인물/동물·자연/음식·음료/활동/여행·장소/사물/기호/국기`)
//! - 하단 페이지 인디케이터
//! - 검색 SearchEntry 제거 (PR #1 단계에서 데이터 측 검색 폐기)
//! - 셀 클릭 → DBus `CommitEmoji`
//!
//! PR #2 (emoji overhaul, OPTION X engine-driven):
//! 좌측 탭 클릭은 시각적 active 토글만 — 카테고리 전환은 키보드 Tab/ShiftTab 으로
//! 엔진이 처리한 뒤 `ShowEmojiPopupV2` 가 재발행되어 전체 화면이 갱신된다. 탭 클릭
//! 으로 카테고리를 직접 전환하려면 daemon 측 RPC (`EmojiSetCategory`) 가 추가되어야
//! 하며 — 이는 PR #4/#5 범위.

use gtk4::prelude::*;
use rust_i18n::t;
use unim::unim_log;

use crate::popup_positioning::{self, DisplayServer};

/// 이모지 그리드: 9×9 고정 (특수문자와 동일).
const MAX_ROWS: usize = 9;
const MAX_COLS: usize = 9;
const PAGE_SIZE: usize = MAX_ROWS * MAX_COLS;
/// 좌측 세로 탭 수 (Recent + 8 카테고리).
const TAB_COUNT: usize = 9;

const ICON_PREV_PAGE: &str = "◀";
const ICON_NEXT_PAGE: &str = "▶";

/// 카테고리 id 별 locale key — `data.rs::CATEGORIES` 와 1:1.
fn tab_label_for(cat_id: &str) -> String {
    let key = match cat_id {
        "Recent" => "emoji_tab_recent",
        "SmileysPeople" => "emoji_tab_smileys_people",
        "Animals" => "emoji_tab_animals_nature",
        "Food" => "emoji_tab_food_drink",
        "Activities" => "emoji_tab_activities",
        "Travel" => "emoji_tab_travel_places",
        "Objects" => "emoji_tab_objects",
        "Symbols" => "emoji_tab_symbols",
        "Flags" => "emoji_tab_flags",
        _ => return cat_id.to_string(),
    };
    t!(key).to_string()
}

/// 이모지 팝업 상태
pub struct EmojiPopup {
    pub window: gtk4::Window,
    display_server: DisplayServer,
    /// 헤더 라벨 ("「최근 사용」 → 이모지" 형식)
    header_label: gtk4::Label,
    /// 좌측 9 탭 ToggleButton (Recent + 8 카테고리)
    tab_buttons: Vec<gtk4::ToggleButton>,
    /// 탭 라벨 (단축키 가시성을 위해 우측 정렬되도록 별도 Label 위젯 사용)
    tab_labels: Vec<gtk4::Label>,
    /// 9×9 그리드 셀 라벨 (column-major: cells[col][row])
    cells: Vec<Vec<gtk4::Label>>,
    /// 상단 컬럼 헤더 라벨 (`top_row` 9 문자)
    col_headers: Vec<gtk4::Label>,
    /// 좌측 행 번호 라벨 (1..=9)
    row_numbers: Vec<gtk4::Label>,
    /// 푸터 컨테이너 (◀ + 페이지 라벨 + ▶)
    footer_box: gtk4::Box,
    /// 페이지 인디케이터 (footer)
    footer_label: gtk4::Label,
    /// 이전 페이지 버튼 (◀) — 단일 페이지 시 hide
    prev_page_btn: gtk4::Button,
    /// 다음 페이지 버튼 (▶) — 단일 페이지 시 hide
    next_page_btn: gtk4::Button,
    /// 현재 카테고리의 emoji 풀 (시작 카테고리만; 키 입력 시 daemon 이 ShowEmojiPopupV2 재발행)
    items: Vec<String>,
    /// 카테고리 id 캐시 (헤더 라벨용)
    current_cat_id: String,
    /// 현재 페이지 (0-based)
    current_page: usize,
    /// 총 페이지 수
    total_pages: usize,
    /// 선택 (col, row)
    sel_col: usize,
    sel_row: usize,
}

impl EmojiPopup {
    /// 새 이모지 팝업 생성 — 빈 9×9 grid + 좌측 9 탭 골격을 만든다.
    pub fn new(app: &libadwaita::Application) -> Self {
        let display_server = popup_positioning::detect_display_server();

        let window = gtk4::Window::builder()
            .application(app)
            .decorated(false)
            .resizable(false)
            .build();
        window.set_focusable(false);
        window.add_css_class("unim-emoji-popup");

        // 외곽: vbox = [header, grid(통합), footer]
        // 통합 grid 레이아웃 (Clutter.GridLayout 패턴과 동일):
        //   col 0    : 카테고리 탭 (rows 1..9)
        //   col 1    : 행 번호    (rows 1..9)
        //   col 2..10: 컬럼 헤더(row 0) + 9×9 emoji 셀 (rows 1..9)
        // GtkGrid 가 같은 row 의 모든 자식을 단일 row 높이로 정렬해
        // 좌측 탭과 우측 emoji 행이 자연스레 위아래로 일치한다.
        let vbox = gtk4::Box::new(gtk4::Orientation::Vertical, 0);

        // 헤더 라벨 ("「Recent」 → 이모지")
        let header_label = gtk4::Label::new(None);
        header_label.add_css_class("popup-header");
        header_label.set_halign(gtk4::Align::Fill);
        header_label.set_xalign(0.0);
        vbox.append(&header_label);

        // 본문 통합 grid
        let grid = gtk4::Grid::new();
        grid.add_css_class("emoji-grid");
        grid.set_row_spacing(1);
        grid.set_column_spacing(1);
        grid.set_margin_top(4);

        // 좌측 카테고리 탭 (col 0, rows 1..=9) — emoji 행과 row 인덱스 매칭
        let mut tab_buttons: Vec<gtk4::ToggleButton> = Vec::with_capacity(TAB_COUNT);
        let mut tab_labels: Vec<gtk4::Label> = Vec::with_capacity(TAB_COUNT);
        let mut group_anchor: Option<gtk4::ToggleButton> = None;
        for i in 0..TAB_COUNT {
            let btn = gtk4::ToggleButton::new();
            btn.add_css_class("emoji-tab-button");
            btn.set_focusable(false);
            // 라벨은 우측 정렬 — "카테고리 (k)" 의 단축키가 우측 일관 위치에 노출.
            // ToggleButton::set_label() 의 자동 라벨은 xalign 노출이 없으므로 명시
            // Label 위젯을 child 로 연결한다.
            let label = gtk4::Label::new(Some(""));
            label.add_css_class("emoji-tab-label");
            label.set_xalign(1.0);
            label.set_halign(gtk4::Align::End);
            label.set_hexpand(true);
            btn.set_child(Some(&label));
            // 탭 그룹화 — 한 번에 하나만 active
            if let Some(anchor) = group_anchor.as_ref() {
                btn.set_group(Some(anchor));
            } else {
                group_anchor = Some(btn.clone());
            }
            // 탭 클릭은 시각적 active 토글만 — 실제 카테고리 전환은 키보드 Tab/ShiftTab
            // (엔진이 처리, ShowEmojiPopupV2 재발행). RPC 미배선 경고 로그.
            let tab_idx = i;
            btn.connect_clicked(move |b| {
                if b.is_active() {
                    unim_log!(
                        "INDICATOR",
                        "[EmojiPopup] 탭 클릭 idx={} — RPC 미배선 (PR #4/#5 에서 EmojiSetCategory 추가 예정)",
                        tab_idx
                    );
                }
            });
            grid.attach(&btn, 0, (i + 1) as i32, 1, 1);
            tab_buttons.push(btn);
            tab_labels.push(label);
        }

        // 컬럼 헤더 (top_row 9문자) — row 0, cols 2..=10
        let mut col_headers = Vec::with_capacity(MAX_COLS);
        for col in 0..MAX_COLS {
            let label = gtk4::Label::new(None);
            label.add_css_class("grid-header");
            label.set_halign(gtk4::Align::Center);
            grid.attach(&label, (col + 2) as i32, 0, 1, 1);
            col_headers.push(label);
        }

        // 행 번호 (1..=9) — col 1, rows 1..=9
        let mut row_numbers = Vec::with_capacity(MAX_ROWS);
        for row in 0..MAX_ROWS {
            let num_label = gtk4::Label::new(Some(&format!("{}", row + 1)));
            num_label.add_css_class("grid-row-number");
            num_label.set_halign(gtk4::Align::Center);
            grid.attach(&num_label, 1, (row + 1) as i32, 1, 1);
            row_numbers.push(num_label);
        }

        // 9×9 데이터 셀 (column-major: cells[col][row]) — cols 2..=10, rows 1..=9
        let mut cells = Vec::with_capacity(MAX_COLS);
        for col in 0..MAX_COLS {
            let mut col_cells = Vec::with_capacity(MAX_ROWS);
            for row in 0..MAX_ROWS {
                let label = gtk4::Label::new(None);
                label.add_css_class("grid-cell");
                label.set_halign(gtk4::Align::Center);

                // 클릭 → CommitEmoji (현 라벨 텍스트를 그대로 emoji 문자열로 사용)
                let gesture = gtk4::GestureClick::new();
                let label_weak = label.downgrade();
                gesture.connect_released(move |_, _, _, _| {
                    if let Some(lbl) = label_weak.upgrade() {
                        let text = lbl.text().to_string();
                        commit_emoji_via_dbus(text);
                    }
                });
                label.add_controller(gesture);

                grid.attach(&label, (col + 2) as i32, (row + 1) as i32, 1, 1);
                col_cells.push(label);
            }
            cells.push(col_cells);
        }

        vbox.append(&grid);

        // 푸터: [◀] [카테고리 n/N] [▶]
        let footer_box = gtk4::Box::new(gtk4::Orientation::Horizontal, 4);
        footer_box.add_css_class("popup-footer-box");

        let prev_page_btn = gtk4::Button::with_label(ICON_PREV_PAGE);
        prev_page_btn.add_css_class("popup-page-btn");
        prev_page_btn.add_css_class("flat");
        prev_page_btn.set_can_focus(false);
        prev_page_btn.set_focusable(false);
        prev_page_btn.set_tooltip_text(Some(&t!("popup_previous_page")));
        prev_page_btn.connect_clicked(|_| {
            crate::hanja_popup::popup_change_page_via_dbus(0);
        });
        footer_box.append(&prev_page_btn);

        let footer_label = gtk4::Label::new(None);
        footer_label.add_css_class("popup-footer");
        footer_label.set_halign(gtk4::Align::Center);
        footer_label.set_hexpand(true);
        footer_box.append(&footer_label);

        let next_page_btn = gtk4::Button::with_label(ICON_NEXT_PAGE);
        next_page_btn.add_css_class("popup-page-btn");
        next_page_btn.add_css_class("flat");
        next_page_btn.set_can_focus(false);
        next_page_btn.set_focusable(false);
        next_page_btn.set_tooltip_text(Some(&t!("popup_next_page")));
        next_page_btn.connect_clicked(|_| {
            crate::hanja_popup::popup_change_page_via_dbus(1);
        });
        footer_box.append(&next_page_btn);

        vbox.append(&footer_box);

        window.set_child(Some(&vbox));

        // ESC → 숨김 (한자/특수와 동일)
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

        // AT-SPI 접근성
        window.update_property(&[gtk4::accessible::Property::Label(
            t!("emoji_popup_title").as_ref(),
        )]);

        Self {
            window,
            display_server,
            header_label,
            tab_buttons,
            tab_labels,
            cells,
            col_headers,
            row_numbers,
            footer_box,
            footer_label,
            prev_page_btn,
            next_page_btn,
            items: Vec::new(),
            current_cat_id: String::new(),
            current_page: 0,
            total_pages: 0,
            sel_col: 0,
            sel_row: 0,
        }
    }

    /// 이모지 팝업 표시 (`ShowEmojiPopupV2` 시그널 핸들러)
    ///
    /// # Arguments
    /// * `context_path` — DBus `CommitEmoji` 콜백용 InputContext 경로.
    /// * `target_cat_id` — 시작 카테고리 id ("Recent"/"SmileysPeople"/.../"Flags").
    /// * `items` — 시작 카테고리의 emoji 풀 (전체).
    /// * `top_row` — 활성 영문 키맵의 상단 9 문자.
    /// * `recent` — 'Recent' 탭 캐시 (현재 표시에는 직접 쓰지 않음 — daemon 의 popup_state 가
    ///   cat_index=0 일 때 items 에 동일 내용을 이미 담아 보낸다).
    /// * `categories` — 좌측 9 탭 메타 — `(id, ko, en, count)` 튜플 9개.
    #[allow(clippy::too_many_arguments)]
    pub fn show(
        &mut self,
        context_path: String,
        target_cat_id: String,
        items: Vec<String>,
        top_row: String,
        _recent: Vec<String>,
        categories: Vec<(String, String, String, u32)>,
        x: i32,
        y: i32,
        _w: i32,
        h: i32,
    ) {
        // GNOME Wayland: extension 이 팝업 전담 (GTK 윈도우가 포커스를 뺏어 FocusOut 유발)
        if self.display_server == DisplayServer::GnomeWayland {
            return;
        }

        // 활성 컨텍스트 경로 저장 (셀 클릭 → CommitEmoji DBus 호출용)
        {
            use unim_gui_common::types::ACTIVE_CONTEXT_PATH;
            if let Ok(mut path) = ACTIVE_CONTEXT_PATH.lock() {
                *path = Some(context_path);
            }
        }

        unim_log!(
            "INDICATOR",
            "[EmojiPopup] show 진입: cat='{}', items={}, cats={}, cursor=({},{},{})",
            target_cat_id,
            items.len(),
            categories.len(),
            x,
            y,
            h
        );

        // 좌측 탭 라벨 갱신 + active 동기화
        let mut active_idx: usize = 0;
        for (i, btn) in self.tab_buttons.iter().enumerate() {
            let label = &self.tab_labels[i];
            if let Some((id, _, _, _)) = categories.get(i) {
                label.set_text(&tab_label_for(id));
                btn.set_visible(true);
                if id == &target_cat_id {
                    active_idx = i;
                }
            } else {
                label.set_text("");
                btn.set_visible(false);
            }
        }
        if let Some(btn) = self.tab_buttons.get(active_idx) {
            btn.set_active(true);
        }

        // 컬럼 헤더 (top_row) 갱신
        let chars: Vec<char> = top_row.chars().collect();
        for (i, header) in self.col_headers.iter().enumerate() {
            let s = chars.get(i).map(|c| c.to_string()).unwrap_or_default();
            header.set_text(&s);
        }

        // 헤더 라벨: "「표정·인물」 → 이모지"
        let cat_label = categories
            .iter()
            .find(|(id, _, _, _)| id == &target_cat_id)
            .map(|(id, _, _, _)| tab_label_for(id))
            .unwrap_or_else(|| target_cat_id.clone());
        self.header_label.set_text(&format!("「{}」 → 이모지", cat_label));

        self.current_cat_id = target_cat_id;
        self.items = items;
        self.total_pages = if self.items.is_empty() {
            1
        } else {
            self.items.len().div_ceil(PAGE_SIZE)
        };
        self.current_page = 0;
        self.sel_col = 0;
        self.sel_row = 0;

        self.update_grid();

        popup_positioning::position_popup(&self.window, x, y, h, self.display_server);
        self.window.set_visible(true);
        unim_log!(
            "INDICATOR",
            "[EmojiPopup] 표시 완료: cat='{}', pages={}, realized={}",
            self.current_cat_id,
            self.total_pages,
            self.window.is_realized()
        );
    }

    /// 그리드 내용 업데이트 — 항상 9×9 그리드 유지 (한자 expanded / 특수문자 정책 일관).
    ///
    /// 페이지 항목 수가 81 미만이어도 시각 차원은 MAX_ROWS×MAX_COLS 로 일정.
    /// 빈 셀(idx >= page_count)은 빈 텍스트 + selected 클래스 제거 — 그리드가
    /// 출렁이지 않아 사용자가 셀 위치를 학습하기 쉽다.
    fn update_grid(&self) {
        let start = self.current_page * PAGE_SIZE;
        let page_count = if start >= self.items.len() {
            0
        } else {
            (self.items.len() - start).min(PAGE_SIZE)
        };

        for col in 0..MAX_COLS {
            for row in 0..MAX_ROWS {
                let idx = col * MAX_ROWS + row;
                let label = &self.cells[col][row];

                if idx < page_count {
                    let global = start + idx;
                    label.set_text(&self.items[global]);
                } else {
                    label.set_text("");
                }
                label.set_visible(true);
                label.remove_css_class("selected");
            }
        }

        // 컬럼 헤더 / 행 번호 — 항상 9 모두 가시.
        for header in self.col_headers.iter() {
            header.set_visible(true);
            header.remove_css_class("active");
        }
        for num in self.row_numbers.iter() {
            num.set_visible(true);
            num.remove_css_class("active");
        }

        // 페이지 인디케이터 — 단일 페이지(예: Recent) 에서도 ◀/▶ 와 N/M 표기 항상
        // 노출해 popup 크기 안정화 (탭 전환마다 footer 가 늘었다 줄면 인지 부담 ↑).
        // 데몬은 totalPages=1 에서 PopupChangePage 를 받으면 no-op.
        let total = self.total_pages.max(1);
        self.footer_label.set_text(&format!(
            "[{}]  {}/{}",
            tab_label_for(&self.current_cat_id),
            self.current_page + 1,
            total
        ));
        self.prev_page_btn.set_visible(true);
        self.next_page_btn.set_visible(true);
        self.footer_box.set_visible(true);

        self.update_selection();
    }

    /// 선택 하이라이트 갱신 (셀 + 컬럼 헤더 + 행 번호)
    fn update_selection(&self) {
        for col_cells in &self.cells {
            for cell in col_cells {
                cell.remove_css_class("selected");
            }
        }
        if self.sel_col < MAX_COLS && self.sel_row < MAX_ROWS {
            self.cells[self.sel_col][self.sel_row].add_css_class("selected");
        }
        for (col, header) in self.col_headers.iter().enumerate() {
            if col == self.sel_col {
                header.add_css_class("active");
            } else {
                header.remove_css_class("active");
            }
        }
        for (row, num) in self.row_numbers.iter().enumerate() {
            if row == self.sel_row {
                num.add_css_class("active");
            } else {
                num.remove_css_class("active");
            }
        }
    }

    /// `PopupNavigate` 시그널 핸들러 — 페이지/셀 갱신.
    ///
    /// 카테고리 전환은 daemon 이 `ShowEmojiPopupV2` 를 재발행하여 [`Self::show`] 가
    /// 다시 호출되므로, 본 함수는 같은 카테고리 내 페이지/셀 변경만 처리한다.
    pub fn navigate(
        &mut self,
        page: i32,
        _total_pages: i32,
        _selected: i32,
        _rows: i32,
        _cols: i32,
        sel_row: i32,
        sel_col: i32,
    ) {
        let new_page = page.max(0) as usize;
        if new_page != self.current_page {
            self.current_page = new_page;
            // 페이지 변경 시 grid 전체 재렌더 (selection 도 갱신됨)
            self.sel_row = sel_row.max(0) as usize;
            self.sel_col = sel_col.max(0) as usize;
            self.update_grid();
        } else {
            self.sel_row = sel_row.max(0) as usize;
            self.sel_col = sel_col.max(0) as usize;
            self.update_selection();
        }
    }

    /// 팝업 숨김
    pub fn hide(&self) {
        self.window.set_visible(false);
    }

    /// 팝업이 현재 표시 중인지
    pub fn is_visible(&self) -> bool {
        self.window.is_visible()
    }

    /// 통합 PopupRender 시그널 핸들러 (Phase B 통합 SoT).
    ///
    /// daemon 산출 view_model 의 미리 포맷된 문자열을 적용 — 헤더 ("「Smileys」 → 이모지")
    /// + footer ("[Smileys] 1/3") + show_footer + tab_labels (단축키 prefix 포함,
    /// "Smileys (a)" 형식). 카테고리 탭 라벨은 daemon 이 home_row 기반으로 산출.
    pub fn update_from_render(
        &mut self,
        header_text: &str,
        footer_text: &str,
        _show_footer: bool,
        tab_labels: &[String],
    ) {
        self.header_label.set_text(header_text);
        self.footer_label.set_text(footer_text);
        // Footer 항상 노출 — popup 크기 안정화. daemon hint(_show_footer) 는 무시.
        self.footer_box.set_visible(true);
        self.prev_page_btn.set_visible(true);
        self.next_page_btn.set_visible(true);
        // 탭 라벨 — daemon 이 단축키 prefix 포함하여 산출. tab_labels 와 self.tab_labels
        // 는 같은 순서 (categories 와 1:1). 부족하면 빈 라벨.
        for (i, label) in self.tab_labels.iter().enumerate() {
            if let Some(text) = tab_labels.get(i) {
                label.set_text(text);
            }
        }
    }
}

/// 셀 클릭 콜백에서 호출 — 라벨 텍스트(emoji)를 그대로 받아 DBus `CommitEmoji` RPC.
///
/// 한자/특수문자는 column-major 인덱스만 보내지만 (`SelectHanja`/`SelectSpecialChar`),
/// 이모지는 그런 인덱스 RPC (`SelectEmojiAtIndex`) 가 PR #1 에 추가되지 않았다 —
/// 기존 `CommitEmoji(emoji)` 를 그대로 사용. 라벨 텍스트가 빈 문자열이면 no-op.
fn commit_emoji_via_dbus(emoji_str: String) {
    use unim_gui_common::types::ACTIVE_CONTEXT_PATH;

    if emoji_str.is_empty() {
        return;
    }

    let context_path = ACTIVE_CONTEXT_PATH.lock().ok().and_then(|p| p.clone());

    if let Some(path) = context_path {
        unim_log!(
            "INDICATOR",
            "[EmojiPopup] CommitEmoji DBus 호출: emoji='{}', path={}",
            emoji_str,
            path
        );
        std::thread::spawn(move || {
            let rt = match tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
            {
                Ok(r) => r,
                Err(_) => return,
            };
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

/// 이모지 팝업 CSS — `tools/popup-styles/popup_tokens.toml` +
/// `templates/gtk_hanja_popup.css.tmpl` 에서 `.unim-emoji-popup` 룰셋이 함께
/// 생성되어 `popup_styles.generated.css` 에 포함된다 — `hanja_popup::popup_css()`
/// 가 그 전체를 임베드하므로 본 함수는 빈 문자열만 반환한다 (gtk_ui::load_css
/// 의 concat 호환용 stub).
pub fn popup_css() -> &'static str {
    ""
}
