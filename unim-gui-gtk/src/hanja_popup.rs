//! 한자 팝업 윈도우
//!
//! DBus ShowHanjaPopup 시그널을 받아 한자 후보 목록을 표시합니다.
//! POPUP_SPEC.md Section 3 규격을 준수합니다.

use gtk4::prelude::*;
use unim::unim_log;

use crate::popup_positioning::{self, DisplayServer};

const COMPACT_PAGE_SIZE: usize = 9;
const EXPANDED_PAGE_SIZE: usize = 81;
const EXPANDED_COLS: usize = 9;
const EXPANDED_ROWS: usize = 9;
const ICON_EXPAND: &str = "⊞";
const ICON_COMPACT: &str = "⊟";

/// 한자 팝업 상태
pub struct HanjaPopup {
    pub window: gtk4::Window,
    /// 본문 컨테이너 — list_box 또는 grid를 자식으로 보유
    body_container: gtk4::Box,
    page_label: gtk4::Label,
    target_label: gtk4::Label,
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

        // 타겟 글자 라벨
        let target_label = gtk4::Label::new(None);
        target_label.add_css_class("hanja-target");
        target_label.set_halign(gtk4::Align::Start);
        vbox.append(&target_label);

        // 본문 컨테이너 — compact는 ListBox, expanded는 Grid를 동적으로 차일드로 둔다
        let body_container = gtk4::Box::new(gtk4::Orientation::Vertical, 0);
        body_container.add_css_class("hanja-body");
        vbox.append(&body_container);

        // 푸터: 페이지 라벨 + 확장 아이콘
        let footer = gtk4::Box::new(gtk4::Orientation::Horizontal, 0);
        footer.add_css_class("popup-footer-box");

        let page_label = gtk4::Label::new(None);
        page_label.add_css_class("page-label");
        page_label.set_halign(gtk4::Align::Start);
        page_label.set_hexpand(true);
        footer.append(&page_label);

        // 확장/축소 아이콘 — 현재는 시각적 표시만 담당 (GNOME extension도 동일하게
        // 클릭 콜백이 미배선). Period 키 입력으로 토글되며, navigate()가 cols 변화를
        // 감지하여 아이콘 텍스트를 갱신한다.
        let expand_icon = gtk4::Label::new(Some(ICON_EXPAND));
        expand_icon.add_css_class("popup-expand-icon");
        expand_icon.set_halign(gtk4::Align::End);
        footer.append(&expand_icon);

        vbox.append(&footer);

        window.set_child(Some(&vbox));

        // AT-SPI 접근성
        window.update_property(&[gtk4::accessible::Property::Label("한자 후보")]);

        Self {
            window,
            body_container,
            page_label,
            target_label,
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

        // 엔진에서 현재 북마크 상태를 비동기로 가져와 렌더를 보정
        fetch_bookmark_states_async(self.context_path.clone());

        popup_positioning::position_popup(&self.window, x, y, h, self.display_server);
        self.window.set_visible(true);
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

        // 푸터 갱신
        if total_pages > 1 {
            self.page_label
                .set_text(&format!("{}/{}", self.current_page + 1, total_pages));
        } else {
            self.page_label.set_text("");
        }
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

            row.set_child(Some(&hbox));
            list_box.append(&row);
        }

        // compact: 클릭 → global 인덱스로 SelectHanja
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

    /// expanded 모드 렌더 (9×9 Grid; col=0열은 1-9 번호, 셀은 한자 한 글자)
    fn render_grid(&self, start: usize, end: usize) {
        let grid = gtk4::Grid::new();
        grid.add_css_class("hanja-grid");
        grid.set_row_spacing(2);
        grid.set_column_spacing(2);
        self.body_container.append(&grid);

        // 가로 레이블 키 시퀀스(special과 동일). show() 시점에 ShowHanjaPopup signal payload의
        // top_row로 갱신된 self.top_row를 사용해 키맵 변경(qwerty/dvorak/colemak)에 동기화된다.
        let top_row_chars: Vec<char> = self.top_row.chars().collect();

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
            grid.attach(&header, col as i32, 0, 1, 1);

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
                grid.attach(&cell, col as i32, (row + 1) as i32, 1, 1);
            }
        }
    }

    /// 네비게이션 업데이트 (PopupNavigate 시그널)
    /// rows/cols 변화로 compact↔expanded 자동 전환을 감지한다.
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
}

/// 엔진에서 현재 한자 후보의 북마크 상태를 비동기로 받아 GUI 이벤트 루프에
/// `HanjaBookmarkChanged` 액션을 다량 발행하는 헬퍼.
///
/// 초기 별 상태를 표시하기 위해 사용된다 (GNOME extension.js:176 패턴).
fn fetch_bookmark_states_async(context_path: String) {
    use unim_gui_common::types::SETTINGS_TX;

    let tx_opt = SETTINGS_TX.lock().ok().and_then(|g| g.clone());
    let Some(tx) = tx_opt else { return };

    std::thread::spawn(move || {
        let rt = match tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
        {
            Ok(r) => r,
            Err(_) => return,
        };
        rt.block_on(async move {
            let conn = match zbus::Connection::session().await {
                Ok(c) => c,
                Err(_) => return,
            };
            let proxy = match zbus::Proxy::new(
                &conn,
                "org.atit.unim.InputMethod",
                context_path.as_str(),
                "org.atit.unim.InputContext",
            )
            .await
            {
                Ok(p) => p,
                Err(_) => return,
            };
            let states: Result<Vec<bool>, _> = proxy.call("GetHanjaBookmarkStates", &()).await;
            if let Ok(states) = states {
                // 첫 렌더 색상 누락 방지: 일괄 setter 1회로 set_bookmark_flags →
                // update_page() 강제. (이전엔 true 인덱스만 개별 발행해 race로
                // 첫 렌더에서 색이 누락되곤 했음)
                let _ = tx
                    .send(unim_gui_common::types::GuiAction::HanjaBookmarkStatesFetched { states });
            }
        });
    });
}

/// DBus를 통해 한자 선택
fn select_hanja_via_dbus(page_local_index: u32) {
    use unim_gui_common::types::ACTIVE_CONTEXT_PATH;

    let context_path = { ACTIVE_CONTEXT_PATH.lock().ok().and_then(|p| p.clone()) };

    if let Some(path) = context_path {
        unim_log!(
            "INDICATOR",
            "[Popup] 한자 선택 DBus 호출: index={}, path={}",
            page_local_index,
            path
        );
        // 비동기 DBus 호출을 별도 스레드에서 수행
        std::thread::spawn(move || {
            let rt = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .unwrap();
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
                        let _: Result<String, _> =
                            proxy.call("SelectHanja", &(page_local_index,)).await;
                    }
                }
            });
        });
    }
}

/// 한자 팝업용 CSS (GNOME extension stylesheet.css와 동일 디자인)
pub fn popup_css() -> &'static str {
    r#"
    .unim-hanja-popup {
        background-color: rgba(30, 30, 46, 0.95);
        border: 1px solid rgba(255, 255, 255, 0.15);
        border-radius: 12px;
        padding: 12px;
        min-width: 280px;
        max-width: 420px;
    }

    .unim-hanja-vbox {
        padding: 0;
        margin: 0;
    }

    .hanja-target {
        background-color: #313244;
        color: #89b4fa;
        font-size: 13px;
        font-weight: bold;
        padding: 6px 8px;
        border-radius: 4px;
        margin-bottom: 6px;
    }

    .unim-hanja-popup .hanja-list {
        background: transparent;
        border-radius: 6px;
    }

    .unim-hanja-popup .hanja-list row {
        background: transparent;
        border-radius: 6px;
        min-height: 28px;
        padding: 4px 8px;
    }

    .unim-hanja-popup .hanja-list row:selected {
        background-color: rgba(137, 180, 250, 0.2);
    }

    .hanja-num {
        color: #7f849c;
        font-size: 12px;
        min-width: 20px;
    }

    .hanja-char {
        color: #cdd6f4;
        font-size: 18px;
        font-weight: bold;
    }

    .hanja-meaning {
        color: #a6adc8;
        font-size: 12px;
    }

    .hanja-bookmark {
        color: #6c7086;
        font-size: 14px;
        margin-left: 8px;
        min-width: 16px;
    }

    .hanja-bookmark.bookmarked {
        color: #f9e2af;
    }

    .unim-hanja-popup .hanja-list row.bookmarked {
        background-color: rgba(249, 226, 175, 0.08);
    }

    .unim-hanja-popup .hanja-list row:selected .hanja-char,
    .unim-hanja-popup .hanja-list row:selected .hanja-meaning,
    .unim-hanja-popup .hanja-list row:selected .hanja-num {
        color: #cdd6f4;
    }

    .page-label {
        color: #6c7086;
        font-size: 12px;
        padding: 4px 8px 2px 0;
    }

    .popup-footer-box {
        margin-top: 4px;
    }

    .popup-expand-icon {
        color: #7f849c;
        font-size: 14px;
        padding: 2px 6px;
    }

    .unim-hanja-popup .hanja-grid {
        padding: 4px;
    }

    .unim-hanja-popup .grid-row-number {
        color: #7f849c;
        font-size: 11px;
        min-width: 22px;
        min-height: 22px;
    }

    .unim-hanja-popup .grid-cell {
        background: transparent;
        color: #cdd6f4;
        font-size: 16px;
        min-width: 28px;
        min-height: 28px;
        border-radius: 4px;
        padding: 2px;
    }

    .unim-hanja-popup .grid-cell:hover {
        background-color: rgba(137, 180, 250, 0.15);
    }

    .unim-hanja-popup .grid-cell-selected {
        background-color: rgba(137, 180, 250, 0.3);
    }

    .unim-hanja-popup .grid-cell.bookmarked {
        color: #f9e2af;
    }
    "#
}
