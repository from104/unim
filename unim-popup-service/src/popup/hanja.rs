//! 한자 팝업 윈도우
//!
//! DBus ShowHanjaPopup 시그널을 받아 한자 후보 목록을 표시합니다.
//! POPUP_SPEC.md Section 3 규격을 준수합니다.

use gtk4::prelude::*;
use rust_i18n::t;
use unim::unim_log;

use unim_gui_common::popup_dbus::{
    cancel_hanja_via_dbus, fetch_bookmark_states_async, popup_change_page_via_dbus,
    select_hanja_via_dbus, toggle_hanja_bookmark_via_dbus, toggle_popup_expand_via_dbus,
};

use crate::backend::x11 as popup_positioning;
use crate::backend::x11::DisplayServer;

const COMPACT_PAGE_SIZE: usize = 9;
const EXPANDED_PAGE_SIZE: usize = 81;
const EXPANDED_COLS: usize = 9;
const EXPANDED_ROWS: usize = 9;
const ICON_EXPAND: &str = "⊞";
const ICON_COMPACT: &str = "⊟";
const ICON_PREV_PAGE: &str = "◀";
const ICON_NEXT_PAGE: &str = "▶";

/// ★ 해제 후 cursor 셀에 적용되는 yellow flash 지속시간 (ms).
/// CSS `.bookmark-flash` 와 짝.
const BOOKMARK_FLASH_DURATION_MS: u32 = 140;

/// 한자 팝업 상태
pub struct HanjaPopup {
    pub window: gtk4::Window,
    /// 본문 컨테이너 — list_box 또는 grid를 자식으로 보유
    body_container: gtk4::Box,
    page_label: gtk4::Label,
    target_label: gtk4::Label,
    /// 이전 페이지 버튼 (◀) — 단일 페이지 시 hide
    prev_page_btn: gtk4::Button,
    /// 다음 페이지 버튼 (▶) — 단일 페이지 시 hide
    next_page_btn: gtk4::Button,
    /// 확장/축소 토글 아이콘 라벨 (⊞/⊟)
    expand_icon: gtk4::Label,
    display_server: DisplayServer,
    /// 전체 후보 목록
    candidates: Vec<(String, String)>,
    /// 후보별 즐겨찾기 상태 (candidates와 동일 길이)
    bookmarks: Vec<bool>,
    /// 현재 페이지 (0-based)
    current_page: usize,
    /// 현재 선택 행 (페이지 내, 0-based)
    sel_row: usize,
    /// 현재 선택 열 (compact=0 고정, expanded=0..EXPANDED_COLS-1)
    sel_col: usize,
    /// 엔진 측 cols (1=compact, >1=expanded). Period 키 토글은 엔진이 결정.
    cols: usize,
    /// 저장된 context_path (DBus 콜백용)
    context_path: String,
    /// expanded(9x9) 컬럼 헤더 라벨 (활성 영문 키맵의 top_row).
    /// show()에서 ShowHanjaPopup signal payload의 top_row로 매번 갱신된다.
    /// QWERTYUIO는 안전망 default — payload 누락 시에도 렌더가 깨지지 않게 한다.
    top_row: String,
}

impl HanjaPopup {
    /// 새 한자 팝업 생성
    pub fn new(app: &libadwaita::Application) -> Self {
        let display_server = popup_positioning::detect_display_server();

        let window = gtk4::Window::builder()
            .application(app)
            .decorated(false)
            .resizable(false)
            .default_width(280)
            .build();
        window.set_focusable(false);
        window.add_css_class("unim-hanja-popup");

        // 메인 컨테이너
        let vbox = gtk4::Box::new(gtk4::Orientation::Vertical, 0);
        vbox.add_css_class("unim-hanja-vbox");

        // 헤더: [⊞/⊟ expand 토글] [target_label]
        // expand 아이콘을 헤더 좌측 최상단에 두면 마우스 빠른 모드 전환 + 확장 모드
        // 헤더에 한자 뜻이 라벨에 채워져 일관된 위치에 표시된다.
        let header_box = gtk4::Box::new(gtk4::Orientation::Horizontal, 4);
        header_box.add_css_class("popup-header-box");

        let expand_icon = gtk4::Label::new(Some(ICON_EXPAND));
        expand_icon.add_css_class("popup-expand-icon");
        expand_icon.set_halign(gtk4::Align::Start);
        // 마우스 클릭 → TogglePopupExpand RPC (popup-owner 라우팅 보정).
        let click_gesture = gtk4::GestureClick::new();
        click_gesture.connect_released(|_, _, _, _| {
            toggle_popup_expand_via_dbus();
        });
        expand_icon.add_controller(click_gesture);
        // 호버 시 마우스 커서 변경 (시각 피드백)
        expand_icon.set_cursor_from_name(Some("pointer"));
        header_box.append(&expand_icon);

        let target_label = gtk4::Label::new(None);
        target_label.add_css_class("hanja-target");
        target_label.set_halign(gtk4::Align::Start);
        target_label.set_hexpand(true);
        target_label.set_xalign(0.0);
        header_box.append(&target_label);

        vbox.append(&header_box);

        // 본문 컨테이너 — compact는 ListBox, expanded는 Grid를 동적으로 차일드로 둔다
        let body_container = gtk4::Box::new(gtk4::Orientation::Vertical, 0);
        body_container.add_css_class("hanja-body");
        vbox.append(&body_container);

        // 푸터: [◀] [페이지 n/N] [▶]
        // 한자 popup 은 단일 페이지여도 항상 가시 — popup 크기 안정 정책.
        let footer = gtk4::Box::new(gtk4::Orientation::Horizontal, 4);
        footer.add_css_class("popup-footer-box");

        let prev_page_btn = gtk4::Button::with_label(ICON_PREV_PAGE);
        prev_page_btn.add_css_class("popup-page-btn");
        prev_page_btn.add_css_class("flat");
        prev_page_btn.set_can_focus(false);
        prev_page_btn.set_focusable(false);
        prev_page_btn.set_tooltip_text(Some(&t!("popup_previous_page")));
        prev_page_btn.connect_clicked(|_| {
            popup_change_page_via_dbus(0);
        });
        footer.append(&prev_page_btn);

        let page_label = gtk4::Label::new(None);
        page_label.add_css_class("page-label");
        page_label.set_halign(gtk4::Align::Center);
        page_label.set_hexpand(true);
        footer.append(&page_label);

        let next_page_btn = gtk4::Button::with_label(ICON_NEXT_PAGE);
        next_page_btn.add_css_class("popup-page-btn");
        next_page_btn.add_css_class("flat");
        next_page_btn.set_can_focus(false);
        next_page_btn.set_focusable(false);
        next_page_btn.set_tooltip_text(Some(&t!("popup_next_page")));
        next_page_btn.connect_clicked(|_| {
            popup_change_page_via_dbus(1);
        });
        footer.append(&next_page_btn);

        vbox.append(&footer);

        window.set_child(Some(&vbox));

        // AT-SPI 접근성
        window.update_property(&[gtk4::accessible::Property::Label("한자 후보")]);

        // RC2: 팝업이 숨겨질 때 (어떤 경로든) CancelHanja RPC 호출 → 엔진 상태 정리.
        // 키 grab 없이 outside-click 등 비정상 닫힘 후에도 엔진이 한자 모드에서
        // 빠져나오도록 보장한다. 정상 선택/취소 경로에서는 엔진이 이미 cancel을
        // 처리했으므로 중복 호출이 돼도 no-op (엔진이 idempotent 보장).
        window.connect_hide(|_| {
            cancel_hanja_via_dbus();
        });

        // X11 outside-click dismiss: popup 영역 밖 클릭 시 window.hide()
        // 키 grab 절대 금지 (ghostty key-lock 회귀 방지) — 마우스 grab 만.
        // connect_hide → cancel_hanja_via_dbus 가 이미 등록되어 있으므로
        // 여기서는 hide()만 호출하면 된다.
        #[cfg(feature = "x11-backend")]
        if display_server == DisplayServer::X11 {
            let window_weak = window.downgrade();
            popup_positioning::x11_install_outside_click_handler(&window, move || {
                if let Some(win) = window_weak.upgrade() {
                    unim_log!("INDICATOR", "[Popup] 한자 외부 클릭 → hide()");
                    win.set_visible(false);
                }
            });
        }

        Self {
            window,
            body_container,
            page_label,
            target_label,
            prev_page_btn,
            next_page_btn,
            expand_icon,
            display_server,
            candidates: Vec::new(),
            bookmarks: Vec::new(),
            current_page: 0,
            sel_row: 0,
            sel_col: 0,
            cols: 1,
            context_path: String::new(),
            top_row: "QWERTYUIO".to_string(),
        }
    }

    /// 현재 페이지 사이즈 (compact=9, expanded=81)
    fn page_size(&self) -> usize {
        if self.cols > 1 {
            EXPANDED_PAGE_SIZE
        } else {
            COMPACT_PAGE_SIZE
        }
    }

    /// 한자 팝업 표시
    #[allow(clippy::too_many_arguments)] // payload 평면 매개변수 (target/candidates/row/col/page/sel 묶음)
    pub fn show(
        &mut self,
        context_path: String,
        target: &str,
        candidates: Vec<(String, String)>,
        top_row: &str,
        x: i32,
        y: i32,
        _w: i32,
        h: i32,
    ) {
        // GNOME Wayland: extension이 팝업 전담 (GTK 윈도우가 포커스를 뺏어 FocusOut 유발)
        if self.display_server == DisplayServer::GnomeWayland {
            return;
        }

        unim_log!(
            "INDICATOR",
            "[Popup] 한자 show() 진입: display_server={:?}, top_row='{}', cursor=({},{},{})",
            self.display_server,
            top_row,
            x,
            y,
            h
        );

        // 활성 컨텍스트 경로 저장 — special/emoji popup 과 동일 패턴.
        // dbus_server.rs::ic_proxy 가 본 mutex 에서 path 를 읽어 daemon 으로 forward.
        // 누락 시 GetHanjaBookmarkStates 가 "active InputContext path 없음" 에러로
        // silent fail → 첫 렌더의 북마크 스타일 미적용 (관측 #2042).
        {
            use unim_gui_common::types::ACTIVE_CONTEXT_PATH;
            if let Ok(mut path) = ACTIVE_CONTEXT_PATH.lock() {
                *path = Some(context_path.clone());
            }
        }

        self.context_path = context_path;
        // 초기 북마크 상태는 모두 false로 출발 — DBus fetch 결과가 돌아오면
        // set_bookmark_states()로 일괄 갱신된다 (GNOME extension.js:176 패턴).
        self.bookmarks = vec![false; candidates.len()];
        self.candidates = candidates;
        // payload 누락 방어: 비어 있으면 기존 default(QWERTYUIO) 유지
        if !top_row.is_empty() {
            self.top_row = top_row.to_string();
        }
        self.current_page = 0;
        self.sel_row = 0;
        self.sel_col = 0;
        self.cols = 1;

        self.target_label
            .set_text(&format!("「{}」 → 한자", target));
        self.update_page();

        popup_positioning::position_popup(&self.window, x, y, h, self.display_server);
        self.window.set_visible(true);

        // 엔진에서 현재 북마크 상태를 비동기로 가져와 렌더를 보정.
        // set_visible(true) 이후에 호출해야 응답 도착 시점에 is_visible() 가드가
        // 참이 되어 set_bookmark_flags → update_page 가 실행된다 (첫 렌더 색상 보정).
        fetch_bookmark_states_async(self.context_path.clone());
        unim_log!(
            "INDICATOR",
            "[Popup] 한자 팝업 표시 완료: target='{}', realized={}",
            target,
            self.window.is_realized()
        );
    }

    /// 현재 페이지의 후보 목록 업데이트 (compact=ListBox 1×9, expanded=Grid 9×9)
    fn update_page(&self) {
        // 기존 차일드 모두 제거 (compact↔expanded 위젯 트리 재구성)
        while let Some(child) = self.body_container.first_child() {
            self.body_container.remove(&child);
        }

        let page_size = self.page_size();
        let total_pages = (self.candidates.len() + page_size - 1) / page_size.max(1);
        let start = self.current_page * page_size;
        let end = (start + page_size).min(self.candidates.len());

        if self.cols > 1 {
            self.render_grid(start, end);
        } else {
            self.render_list(start, end);
        }

        // 한자 popup 은 단일 페이지여도 풋터 항상 가시 — popup 크기 안정 정책.
        // ◀/▶ 도 노출 유지 (단일 페이지일 때 클릭은 데몬이 page wrap no-op 처리).
        // PopupRender 시그널이 도달하면 update_from_render 가 footer_text/visibility 를
        // daemon SoT 로 덮어쓴다 — 본 줄들은 첫 idle window 폴백.
        self.page_label
            .set_text(&format!("{}/{}", self.current_page + 1, total_pages));
        self.prev_page_btn.set_visible(true);
        self.next_page_btn.set_visible(true);
        self.expand_icon.set_text(if self.cols > 1 {
            ICON_COMPACT
        } else {
            ICON_EXPAND
        });
    }

    /// compact 모드 렌더 (1×9 ListBox)
    fn render_list(&self, start: usize, end: usize) {
        let list_box = gtk4::ListBox::new();
        list_box.add_css_class("hanja-list");
        list_box.set_selection_mode(gtk4::SelectionMode::Single);

        let scroll = gtk4::ScrolledWindow::builder()
            .hscrollbar_policy(gtk4::PolicyType::Never)
            .vscrollbar_policy(gtk4::PolicyType::Never)
            .min_content_height(28 * COMPACT_PAGE_SIZE as i32)
            .build();
        scroll.set_child(Some(&list_box));
        self.body_container.append(&scroll);

        for (i, (hanja, meaning)) in self.candidates[start..end].iter().enumerate() {
            let global_idx = start + i;
            let row = gtk4::ListBoxRow::new();
            let hbox = gtk4::Box::new(gtk4::Orientation::Horizontal, 8);
            hbox.set_margin_start(8);
            hbox.set_margin_end(8);

            let num_label = gtk4::Label::new(Some(&format!("{}.", i + 1)));
            num_label.add_css_class("hanja-num");
            num_label.set_width_chars(2);
            num_label.set_xalign(1.0);
            hbox.append(&num_label);

            let hanja_label = gtk4::Label::new(Some(hanja));
            hanja_label.add_css_class("hanja-char");
            hbox.append(&hanja_label);

            let meaning_label = gtk4::Label::new(Some(meaning));
            meaning_label.add_css_class("hanja-meaning");
            meaning_label.set_hexpand(true);
            meaning_label.set_halign(gtk4::Align::Start);
            meaning_label.set_ellipsize(gtk4::pango::EllipsizeMode::End);
            hbox.append(&meaning_label);

            let bookmarked = self.bookmarks.get(global_idx).copied().unwrap_or(false);
            let star_label = gtk4::Label::new(Some(if bookmarked { "★" } else { "☆" }));
            star_label.add_css_class("hanja-bookmark");
            if bookmarked {
                star_label.add_css_class("bookmarked");
                row.add_css_class("bookmarked");
            }
            hbox.append(&star_label);

            // 우클릭 → 즐겨찾기 토글 (GNOME extension button-press button=3 정합).
            // GestureClick 의 기본 button=0 은 모든 버튼이라, set_button(3) 으로 우클릭만.
            let right_gesture = gtk4::GestureClick::new();
            right_gesture.set_button(gtk4::gdk::BUTTON_SECONDARY);
            let global_for_right = global_idx as u32;
            right_gesture.connect_released(move |_, _, _, _| {
                toggle_hanja_bookmark_via_dbus(global_for_right);
            });
            row.add_controller(right_gesture);

            row.set_child(Some(&hbox));
            list_box.append(&row);
        }

        // compact: 좌클릭/Enter → global 인덱스로 SelectHanja
        let page_start = start;
        list_box.connect_row_activated(move |_lb, row| {
            let global = page_start + row.index() as usize;
            select_hanja_via_dbus(global as u32);
        });

        // 현재 선택 행 표시
        if let Some(row) = list_box.row_at_index(self.sel_row as i32) {
            list_box.select_row(Some(&row));
        }
    }

    /// expanded 모드 렌더 (9×9 Grid + 좌측 행 번호 + 상단 열 헤더).
    /// (0, 0) corner / row 0 = col 헤더 / col 0 = row 번호 / 셀은 (col+1, row+1).
    /// special_popup.rs 레이아웃과 동일 정합 (UX 일관성).
    fn render_grid(&self, start: usize, end: usize) {
        let grid = gtk4::Grid::new();
        grid.add_css_class("hanja-grid");
        grid.set_row_spacing(2);
        grid.set_column_spacing(2);
        self.body_container.append(&grid);

        // (0, 0): corner 빈 셀
        let corner = gtk4::Label::new(None);
        corner.add_css_class("grid-row-number");
        grid.attach(&corner, 0, 0, 1, 1);

        // 가로 레이블 키 시퀀스(special과 동일). show() 시점에 ShowHanjaPopup signal payload의
        // top_row로 갱신된 self.top_row를 사용해 키맵 변경(qwerty/dvorak/colemak)에 동기화된다.
        let top_row_chars: Vec<char> = self.top_row.chars().collect();

        // 행 번호 (col 0, row 1..=9) — sel_row면 active CSS (special과 정합)
        for row in 0..EXPANDED_ROWS {
            let num_label = gtk4::Label::new(Some(&format!("{}", row + 1)));
            num_label.add_css_class("grid-row-number");
            num_label.set_halign(gtk4::Align::Center);
            if row == self.sel_row {
                num_label.add_css_class("active");
            }
            grid.attach(&num_label, 0, (row + 1) as i32, 1, 1);
        }

        // GNOME extension JS와 동일하게 col 우선 인덱싱: idx = col * rows + row
        for col in 0..EXPANDED_COLS {
            // 가로 레이블 헤더 (Q/W/E/.../O — Letter 키 위치). sel_col이면 active CSS.
            let header_text = top_row_chars
                .get(col)
                .map(|c| c.to_string())
                .unwrap_or_default();
            let header = gtk4::Label::new(Some(&header_text));
            header.add_css_class("grid-header");
            if col == self.sel_col {
                header.add_css_class("active");
            }
            grid.attach(&header, (col + 1) as i32, 0, 1, 1);

            for row in 0..EXPANDED_ROWS {
                let offset = col * EXPANDED_ROWS + row;
                let global = start + offset;
                if global >= end {
                    continue;
                }
                let (hanja, _meaning) = &self.candidates[global];
                let cell = gtk4::Button::new();
                cell.add_css_class("grid-cell");
                cell.set_label(hanja);
                if global == start + self.sel_col * EXPANDED_ROWS + self.sel_row {
                    cell.add_css_class("grid-cell-selected");
                }
                if self.bookmarks.get(global).copied().unwrap_or(false) {
                    cell.add_css_class("bookmarked");
                }
                let global_for_click = global as u32;
                cell.connect_clicked(move |_| {
                    select_hanja_via_dbus(global_for_click);
                });
                // 우클릭 → 즐겨찾기 토글 (compact 와 동일).
                let right_gesture = gtk4::GestureClick::new();
                right_gesture.set_button(gtk4::gdk::BUTTON_SECONDARY);
                let global_for_right = global as u32;
                right_gesture.connect_released(move |_, _, _, _| {
                    toggle_hanja_bookmark_via_dbus(global_for_right);
                });
                cell.add_controller(right_gesture);
                grid.attach(&cell, (col + 1) as i32, (row + 1) as i32, 1, 1);
            }
        }
    }

    /// 네비게이션 업데이트 (PopupNavigate 시그널)
    /// rows/cols 변화로 compact↔expanded 자동 전환을 감지한다.
    #[allow(clippy::too_many_arguments)] // PopupNavigate 시그널 payload 평면 매개변수
    pub fn navigate(
        &mut self,
        page: i32,
        _total_pages: i32,
        _selected: i32,
        _rows: i32,
        cols: i32,
        sel_row: i32,
        sel_col: i32,
    ) {
        let new_page = page.max(0) as usize;
        let new_cols = cols.max(1) as usize;
        let layout_changed = new_page != self.current_page || new_cols != self.cols;

        self.current_page = new_page;
        self.cols = new_cols;
        self.sel_row = sel_row.max(0) as usize;
        self.sel_col = sel_col.max(0) as usize;

        if layout_changed {
            self.update_page();
        } else {
            // 동일 레이아웃에서 셀 selected만 재적용 — 단순화를 위해 통째 재렌더
            self.update_page();
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
    /// daemon 산출 view_model 의 미리 포맷된 문자열을 적용 — header (target_label),
    /// footer (page_label) text, expand 아이콘 (⊞/⊟) 텍스트/visibility.
    /// 셀/그리드 갱신은 PopupNavigate 시그널이 dual-emit 되어 기존 `navigate()`
    /// 가 그대로 처리.
    pub fn update_from_render(
        &mut self,
        header_text: &str,
        footer_text: &str,
        show_footer: bool,
        expand_visible: bool,
        expand_text: &str,
    ) {
        self.target_label.set_text(header_text);
        self.page_label.set_text(footer_text);
        self.page_label.set_visible(show_footer);
        self.prev_page_btn.set_visible(show_footer);
        self.next_page_btn.set_visible(show_footer);
        if expand_visible {
            self.expand_icon.set_text(expand_text);
            self.expand_icon.set_visible(true);
        } else {
            self.expand_icon.set_visible(false);
        }
    }

    /// 특정 후보의 즐겨찾기 상태를 갱신 (HanjaBookmarkChanged 시그널에서 호출)
    pub fn set_bookmark(&mut self, global_index: u32, bookmarked: bool) {
        let idx = global_index as usize;
        if idx >= self.candidates.len() {
            return;
        }
        if self.bookmarks.len() < self.candidates.len() {
            self.bookmarks.resize(self.candidates.len(), false);
        }
        self.bookmarks[idx] = bookmarked;
        // 현재 페이지에 포함된 행만 실제 화면에 보이므로, 해당 인덱스가
        // 현재 페이지에 속하면 즉시 재렌더한다.
        let page_size = self.page_size();
        let start = self.current_page * page_size;
        let end = (start + page_size).min(self.candidates.len());
        if idx >= start && idx < end {
            self.update_page();
        }
    }

    /// 즐겨찾기 플래그를 일괄 설정한다 (fetch 결과 적용용).
    ///
    /// 길이가 다르면 candidates 길이에 맞춰 자르거나 false로 채운다.
    /// 호출 후 무조건 update_page()를 호출해 첫 렌더에서도 색이 적용되도록 한다.
    pub fn set_bookmark_flags(&mut self, flags: Vec<bool>) {
        let n = self.candidates.len();
        let mut bookmarks = flags;
        bookmarks.resize(n, false);
        self.bookmarks = bookmarks;
        if self.window.is_visible() {
            self.update_page();
        }
    }

    /// 한자 후보를 즐겨찾기 정렬 결과로 일괄 교체하고 커서 위치를 점프시킨다.
    /// `HanjaCandidatesReordered` 시그널 처리용.
    pub fn replace_candidates(
        &mut self,
        candidates: Vec<(String, String)>,
        bookmarks: Vec<bool>,
        page: i32,
        sel_row: i32,
        sel_col: i32,
    ) {
        let n = candidates.len();
        self.candidates = candidates;
        let mut bm = bookmarks;
        bm.resize(n, false);
        self.bookmarks = bm;
        self.current_page = page.max(0) as usize;
        self.sel_row = sel_row.max(0) as usize;
        self.sel_col = sel_col.max(0) as usize;
        if self.window.is_visible() {
            self.update_page();
        }
    }

    /// 현재 cursor 셀(`sel_row`, `sel_col`)에 짧은 yellow flash 효과를 부여한다.
    ///
    /// `replace_candidates()` 직후 ★ 해제(true → false) 일 때 GTK UI 가 호출.
    /// 적용 후 BOOKMARK_FLASH_DURATION_MS 후 클래스 자동 제거.
    pub fn flash_cursor_cell(&self) {
        if !self.window.is_visible() {
            return;
        }

        // body_container 의 첫 자식: ScrolledWindow(ListBox) 또는 Grid.
        // self.cols 로 현재 모드 판별 후 위젯을 찾는다.
        let Some(first_child) = self.body_container.first_child() else {
            return;
        };

        let target: Option<gtk4::Widget> = if self.cols > 1 {
            // expanded: Grid 직접. sel_row/sel_col → Grid.child_at(col+1, row+1).
            // (0,0) corner / row 0 헤더 / col 0 행번호. 셀은 (col+1, row+1).
            first_child
                .downcast_ref::<gtk4::Grid>()
                .and_then(|g| g.child_at((self.sel_col + 1) as i32, (self.sel_row + 1) as i32))
        } else {
            // compact: ScrolledWindow → ListBox → row_at_index(sel_row)
            first_child
                .downcast_ref::<gtk4::ScrolledWindow>()
                .and_then(|sw| sw.child())
                .and_then(|w| w.downcast::<gtk4::ListBox>().ok())
                .and_then(|lb| lb.row_at_index(self.sel_row as i32))
                .map(|r| r.upcast::<gtk4::Widget>())
        };

        let Some(widget) = target else {
            return;
        };
        widget.add_css_class("bookmark-flash");
        let weak = widget.downgrade();
        gtk4::glib::timeout_add_local_once(
            std::time::Duration::from_millis(BOOKMARK_FLASH_DURATION_MS as u64),
            move || {
                if let Some(w) = weak.upgrade() {
                    w.remove_css_class("bookmark-flash");
                }
            },
        );
    }
}

/// 엔진에서 현재 한자 후보의 북마크 상태를 비동기로 받아 GUI 이벤트 루프에
/// `HanjaBookmarkChanged` 액션을 다량 발행하는 헬퍼.
///
/// 초기 별 상태를 표시하기 위해 사용된다 (GNOME extension.js:176 패턴).
/// 한자 팝업용 CSS (GNOME extension stylesheet.css와 동일 디자인)
/// 한자 popup CSS — `tools/popup-styles/popup_tokens.toml` + GTK template 에서
/// 자동 생성된다 (`make gen-popup-css`). 직접 편집 금지.
pub fn popup_css() -> &'static str {
    include_str!("popup_styles.generated.css")
}
