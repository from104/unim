//! 특수문자 팝업 윈도우
//!
//! DBus ShowSpecialPopup 시그널을 받아 특수문자 그리드를 표시합니다.
//! POPUP_SPEC.md Section 4 규격을 준수합니다.
//! 9x9 그리드, 열 우선(column-major) 채움 레이아웃.
//! GNOME extension과 동일한 디자인/레이아웃.

use gtk4::prelude::*;
use unim::unim_log;

use crate::popup_positioning::{self, DisplayServer};

const MAX_ROWS: usize = 9;
const MAX_COLS: usize = 9;
const PAGE_SIZE: usize = MAX_ROWS * MAX_COLS; // 81

/// 특수문자 팝업 상태
pub struct SpecialPopup {
    pub window: gtk4::Window,
    #[allow(dead_code)]
    grid: gtk4::Grid,
    header_label: gtk4::Label,
    footer_label: gtk4::Label,
    display_server: DisplayServer,
    /// 전체 문자 목록
    characters: Vec<String>,
    /// 대상 문자
    target: String,
    /// 상단 행 문자열
    top_row: String,
    /// 현재 페이지 (0-based)
    current_page: usize,
    /// 총 페이지 수
    total_pages: usize,
    /// 현재 선택 (col, row)
    sel_col: usize,
    sel_row: usize,
    /// 열 헤더 라벨 (col_headers[col])
    col_headers: Vec<gtk4::Label>,
    /// 행 번호 라벨 (row_numbers[row])
    row_numbers: Vec<gtk4::Label>,
    /// 그리드 셀 라벨들 (column-major: cells[col][row])
    cells: Vec<Vec<gtk4::Label>>,
    /// 현재 페이지의 행/열 수
    active_rows: usize,
    active_cols: usize,
}

impl SpecialPopup {
    /// 새 특수문자 팝업 생성
    pub fn new(app: &libadwaita::Application) -> Self {
        let display_server = popup_positioning::detect_display_server();

        let window = gtk4::Window::builder()
            .application(app)
            .decorated(false)
            .resizable(false)
            .build();
        window.set_focusable(false);
        window.add_css_class("unim-special-popup");

        let vbox = gtk4::Box::new(gtk4::Orientation::Vertical, 0);

        // 헤더 라벨 (「X」 → 특수문자)
        let header_label = gtk4::Label::new(None);
        header_label.add_css_class("popup-header");
        header_label.set_halign(gtk4::Align::Fill);
        header_label.set_xalign(0.0);
        vbox.append(&header_label);

        // 그리드 (열 헤더 + 행 번호 + 데이터 셀 통합)
        let grid = gtk4::Grid::new();
        grid.add_css_class("special-grid");
        grid.set_row_spacing(1);
        grid.set_column_spacing(1);
        grid.set_margin_top(4);
        grid.set_margin_bottom(2);

        // (0, 0): 코너 빈 셀
        let corner = gtk4::Label::new(None);
        corner.add_css_class("grid-row-number");
        grid.attach(&corner, 0, 0, 1, 1);

        // 열 헤더 (grid row 0, columns 1-9)
        let mut col_headers = Vec::with_capacity(MAX_COLS);
        for col in 0..MAX_COLS {
            let label = gtk4::Label::new(None);
            label.add_css_class("grid-header");
            label.set_halign(gtk4::Align::Center);
            grid.attach(&label, (col + 1) as i32, 0, 1, 1);
            col_headers.push(label);
        }

        // 행 번호 (grid column 0, rows 1-9)
        let mut row_numbers = Vec::with_capacity(MAX_ROWS);
        for row in 0..MAX_ROWS {
            let num_label = gtk4::Label::new(Some(&format!("{}", row + 1)));
            num_label.add_css_class("grid-row-number");
            num_label.set_halign(gtk4::Align::Center);
            grid.attach(&num_label, 0, (row + 1) as i32, 1, 1);
            row_numbers.push(num_label);
        }

        // 9x9 데이터 셀 (grid columns 1-9, rows 1-9)
        let mut cells = Vec::with_capacity(MAX_COLS);
        for col in 0..MAX_COLS {
            let mut col_cells = Vec::with_capacity(MAX_ROWS);
            for row in 0..MAX_ROWS {
                let label = gtk4::Label::new(None);
                label.add_css_class("grid-cell");
                label.set_halign(gtk4::Align::Center);

                // 클릭 이벤트
                let gesture = gtk4::GestureClick::new();
                let c = col;
                let r = row;
                gesture.connect_released(move |_, _, _, _| {
                    select_special_via_dbus(c, r);
                });
                label.add_controller(gesture);

                grid.attach(&label, (col + 1) as i32, (row + 1) as i32, 1, 1);
                col_cells.push(label);
            }
            cells.push(col_cells);
        }

        vbox.append(&grid);

        // 페이지 라벨
        let footer_label = gtk4::Label::new(None);
        footer_label.add_css_class("popup-footer");
        footer_label.set_halign(gtk4::Align::Center);
        vbox.append(&footer_label);

        window.set_child(Some(&vbox));

        // AT-SPI 접근성
        window.update_property(&[gtk4::accessible::Property::Label("특수문자 선택")]);

        Self {
            window,
            grid,
            header_label,
            footer_label,
            display_server,
            characters: Vec::new(),
            target: String::new(),
            top_row: String::new(),
            current_page: 0,
            total_pages: 0,
            sel_col: 0,
            sel_row: 0,
            col_headers,
            row_numbers,
            cells,
            active_rows: 0,
            active_cols: 0,
        }
    }

    /// 특수문자 팝업 표시
    pub fn show(
        &mut self,
        context_path: String,
        target: &str,
        characters: Vec<String>,
        top_row: String,
        x: i32,
        y: i32,
        _w: i32,
        h: i32,
    ) {
        // GNOME Wayland: extension이 팝업 전담 (GTK 윈도우가 포커스를 뺏어 FocusOut 유발)
        if self.display_server == DisplayServer::GnomeWayland {
            return;
        }

        // 활성 컨텍스트 경로 저장
        {
            use unim_gui_common::types::ACTIVE_CONTEXT_PATH;
            if let Ok(mut path) = ACTIVE_CONTEXT_PATH.lock() {
                *path = Some(context_path);
            }
        }

        self.target = target.to_string();
        self.characters = characters;
        self.top_row = top_row;
        self.total_pages = (self.characters.len() + PAGE_SIZE - 1) / PAGE_SIZE;
        self.current_page = 0;
        self.sel_col = 0;
        self.sel_row = 0;

        unim_log!(
            "INDICATOR",
            "[Popup] 특수문자 show() 진입: display_server={:?}, cursor=({},{},{})",
            self.display_server, x, y, h
        );

        // 헤더 텍스트
        self.header_label
            .set_text(&format!("「{}」 → 특수문자", self.target));

        self.update_grid();

        popup_positioning::position_popup(&self.window, x, y, h, self.display_server);
        self.window.set_visible(true);
        unim_log!(
            "INDICATOR",
            "[Popup] 특수문자 팝업 표시 완료: count={}, pages={}, realized={}",
            self.characters.len(),
            self.total_pages,
            self.window.is_realized()
        );
    }

    /// 열 헤더 업데이트
    fn update_col_headers(&self) {
        let chars: Vec<char> = self.top_row.chars().collect();
        for (i, header) in self.col_headers.iter().enumerate() {
            if i < chars.len() && i < self.active_cols {
                header.set_text(&chars[i].to_string());
                header.set_visible(true);
            } else {
                header.set_text("");
                header.set_visible(false);
            }
        }
    }

    /// 그리드 내용 업데이트
    fn update_grid(&mut self) {
        let start = self.current_page * PAGE_SIZE;
        let page_count = (self.characters.len() - start).min(PAGE_SIZE);

        // 현재 페이지의 행/열 수 계산
        self.active_cols = if page_count == 0 {
            0
        } else {
            (page_count + MAX_ROWS - 1) / MAX_ROWS
        };
        self.active_rows = if self.active_cols == 0 {
            0
        } else {
            page_count.min(MAX_ROWS)
        };

        // 열 헤더 업데이트
        self.update_col_headers();

        // 행 번호 가시성 업데이트
        for (row, num_label) in self.row_numbers.iter().enumerate() {
            num_label.set_visible(row < self.active_rows);
        }

        // 열 우선(column-major) 채움
        for col in 0..MAX_COLS {
            for row in 0..MAX_ROWS {
                let idx = col * MAX_ROWS + row;
                let label = &self.cells[col][row];

                if idx < page_count {
                    let char_idx = start + idx;
                    label.set_text(&self.characters[char_idx]);
                    label.set_visible(true);
                    label.remove_css_class("selected");
                } else if col < self.active_cols && row < self.active_rows {
                    label.set_text("");
                    label.set_visible(true);
                    label.remove_css_class("selected");
                } else {
                    label.set_visible(false);
                    label.remove_css_class("selected");
                }
            }
        }

        // 페이지 표시
        if self.total_pages > 1 {
            self.footer_label.set_text(&format!(
                "[{}]  {}/{}",
                self.target,
                self.current_page + 1,
                self.total_pages
            ));
            self.footer_label.set_visible(true);
        } else {
            self.footer_label.set_visible(false);
        }

        // 선택 하이라이트
        self.update_selection();
    }

    /// 선택 하이라이트 업데이트 (GNOME extension과 동일 방식)
    fn update_selection(&self) {
        // 모든 셀에서 선택 제거
        for col_cells in &self.cells {
            for cell in col_cells {
                cell.remove_css_class("selected");
            }
        }

        // 현재 선택 셀 하이라이트
        if self.sel_col < MAX_COLS && self.sel_row < MAX_ROWS {
            self.cells[self.sel_col][self.sel_row].add_css_class("selected");
        }

        // 활성 열 헤더 하이라이트
        for (col, header) in self.col_headers.iter().enumerate() {
            if col == self.sel_col {
                header.add_css_class("active");
            } else {
                header.remove_css_class("active");
            }
        }

        // 활성 행 번호 하이라이트
        for (row, num) in self.row_numbers.iter().enumerate() {
            if row == self.sel_row {
                num.add_css_class("active");
            } else {
                num.remove_css_class("active");
            }
        }
    }

    /// 네비게이션 업데이트 (PopupNavigate 시그널)
    pub fn navigate(
        &mut self,
        page: i32,
        _total_pages: i32,
        _selected: i32,
        rows: i32,
        cols: i32,
        sel_row: i32,
        sel_col: i32,
    ) {
        let new_page = page.max(0) as usize;

        if new_page != self.current_page {
            self.current_page = new_page;
            self.update_grid();
        }

        self.active_rows = rows.max(0) as usize;
        self.active_cols = cols.max(0) as usize;
        self.sel_row = sel_row.max(0) as usize;
        self.sel_col = sel_col.max(0) as usize;
        self.update_selection();
    }

    /// 팝업 숨김
    pub fn hide(&self) {
        self.window.set_visible(false);
    }

    /// 팝업이 현재 표시 중인지
    pub fn is_visible(&self) -> bool {
        self.window.is_visible()
    }
}

/// DBus를 통해 특수문자 선택
fn select_special_via_dbus(col: usize, row: usize) {
    use unim_gui_common::types::ACTIVE_CONTEXT_PATH;

    // column-major 인덱스 계산
    let index = (col * MAX_ROWS + row) as u32;

    let context_path = {
        ACTIVE_CONTEXT_PATH
            .lock()
            .ok()
            .and_then(|p| p.clone())
    };

    if let Some(path) = context_path {
        unim_log!(
            "INDICATOR",
            "[Popup] 특수문자 선택 DBus 호출: index={} (col={}, row={}), path={}",
            index,
            col,
            row,
            path
        );
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
                            proxy.call("SelectSpecialChar", &(index,)).await;
                    }
                }
            });
        });
    }
}

/// 특수문자 팝업용 CSS (GNOME extension stylesheet.css와 동일 디자인)
pub fn popup_css() -> &'static str {
    r#"
    .unim-special-popup {
        background-color: rgba(30, 30, 46, 0.95);
        border: 1px solid rgba(255, 255, 255, 0.15);
        border-radius: 12px;
        padding: 12px;
    }

    .unim-special-popup .popup-header {
        background-color: #313244;
        color: #a6e3a1;
        font-size: 13px;
        font-weight: bold;
        padding: 6px 8px;
        border-radius: 4px;
        margin-bottom: 6px;
    }

    .unim-special-popup .special-grid {
        background: transparent;
    }

    .unim-special-popup .grid-cell {
        color: #cdd6f4;
        font-size: 16px;
        min-width: 28px;
        min-height: 28px;
        border-radius: 4px;
    }

    .unim-special-popup .grid-cell:hover {
        background-color: rgba(255, 255, 255, 0.05);
    }

    .unim-special-popup .grid-cell.selected {
        background-color: rgba(166, 227, 161, 0.25);
        font-weight: bold;
    }

    .unim-special-popup .grid-header {
        color: #f9e2af;
        font-weight: bold;
        font-size: 11px;
        min-width: 28px;
        min-height: 28px;
    }

    .unim-special-popup .grid-header.active {
        color: #a6e3a1;
    }

    .unim-special-popup .grid-row-number {
        color: #7f849c;
        font-weight: bold;
        font-size: 12px;
        min-width: 20px;
    }

    .unim-special-popup .grid-row-number.active {
        color: #a6e3a1;
    }

    .unim-special-popup .popup-footer {
        color: #6c7086;
        font-size: 12px;
    }
    "#
}
