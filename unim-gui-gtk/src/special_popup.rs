//! 특수문자 팝업 윈도우
//!
//! DBus ShowSpecialPopup 시그널을 받아 특수문자 그리드를 표시합니다.
//! POPUP_SPEC.md Section 4 규격을 준수합니다.
//! 9x9 그리드, 열 우선(column-major) 채움 레이아웃.

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
    top_row_box: gtk4::Box,
    footer_label: gtk4::Label,
    display_server: DisplayServer,
    /// 전체 문자 목록
    characters: Vec<String>,
    /// 상단 행 문자열
    top_row: String,
    /// 현재 페이지 (0-based)
    current_page: usize,
    /// 총 페이지 수
    total_pages: usize,
    /// 현재 선택 (col, row)
    sel_col: usize,
    sel_row: usize,
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
        window.add_css_class("unim-special-popup");

        let vbox = gtk4::Box::new(gtk4::Orientation::Vertical, 4);
        vbox.set_margin_start(8);
        vbox.set_margin_end(8);
        vbox.set_margin_top(8);
        vbox.set_margin_bottom(8);

        // 상단 행 (초성 표시)
        let top_row_box = gtk4::Box::new(gtk4::Orientation::Horizontal, 2);
        top_row_box.add_css_class("special-top-row");
        vbox.append(&top_row_box);

        // 그리드
        let grid = gtk4::Grid::new();
        grid.add_css_class("special-grid");
        grid.set_row_spacing(1);
        grid.set_column_spacing(1);
        grid.set_row_homogeneous(true);
        grid.set_column_homogeneous(true);

        // 9x9 셀 미리 생성
        let mut cells = Vec::with_capacity(MAX_COLS);
        for col in 0..MAX_COLS {
            let mut col_cells = Vec::with_capacity(MAX_ROWS);
            for row in 0..MAX_ROWS {
                let label = gtk4::Label::new(None);
                label.add_css_class("special-cell");
                label.set_width_chars(2);
                label.set_halign(gtk4::Align::Center);

                // 클릭 이벤트
                let gesture = gtk4::GestureClick::new();
                let c = col;
                let r = row;
                gesture.connect_released(move |_, _, _, _| {
                    select_special_via_dbus(c, r);
                });
                label.add_controller(gesture);

                grid.attach(&label, col as i32, row as i32, 1, 1);
                col_cells.push(label);
            }
            cells.push(col_cells);
        }

        vbox.append(&grid);

        // 페이지 라벨
        let footer_label = gtk4::Label::new(None);
        footer_label.add_css_class("special-footer");
        footer_label.set_halign(gtk4::Align::End);
        vbox.append(&footer_label);

        window.set_child(Some(&vbox));

        // AT-SPI 접근성
        window.update_property(&[gtk4::accessible::Property::Label("특수문자 선택")]);

        Self {
            window,
            grid,
            top_row_box,
            footer_label,
            display_server,
            characters: Vec::new(),
            top_row: String::new(),
            current_page: 0,
            total_pages: 0,
            sel_col: 0,
            sel_row: 0,
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
        if self.display_server == DisplayServer::GnomeWayland {
            return;
        }

        let _ = target;

        // 활성 컨텍스트 경로 저장
        {
            use unim_gui_common::types::ACTIVE_CONTEXT_PATH;
            if let Ok(mut path) = ACTIVE_CONTEXT_PATH.lock() {
                *path = Some(context_path);
            }
        }

        self.characters = characters;
        self.top_row = top_row;
        self.total_pages = (self.characters.len() + PAGE_SIZE - 1) / PAGE_SIZE;
        self.current_page = 0;
        self.sel_col = 0;
        self.sel_row = 0;

        self.update_top_row();
        self.update_grid();

        popup_positioning::position_popup(&self.window, x, y, h, self.display_server);
        self.window.set_visible(true);
        unim_log!(
            "INDICATOR",
            "[Popup] 특수문자 팝업 표시: count={}, pages={}",
            self.characters.len(),
            self.total_pages
        );
    }

    /// 상단 행 업데이트
    fn update_top_row(&self) {
        // 기존 자식 제거
        while let Some(child) = self.top_row_box.first_child() {
            self.top_row_box.remove(&child);
        }

        for ch in self.top_row.chars() {
            let label = gtk4::Label::new(Some(&ch.to_string()));
            label.add_css_class("special-top-cell");
            label.set_width_chars(2);
            self.top_row_box.append(&label);
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

        // 열 우선(column-major) 채움
        for col in 0..MAX_COLS {
            for row in 0..MAX_ROWS {
                let idx = col * MAX_ROWS + row;
                let label = &self.cells[col][row];

                if idx < page_count {
                    let char_idx = start + idx;
                    label.set_text(&self.characters[char_idx]);
                    label.set_visible(true);
                    label.remove_css_class("special-cell-selected");
                } else if col < self.active_cols {
                    label.set_text("");
                    label.set_visible(true);
                    label.remove_css_class("special-cell-selected");
                } else {
                    label.set_visible(false);
                    label.remove_css_class("special-cell-selected");
                }
            }
        }

        // 페이지 표시
        if self.total_pages > 1 {
            self.footer_label
                .set_text(&format!("{}/{}", self.current_page + 1, self.total_pages));
            self.footer_label.set_visible(true);
        } else {
            self.footer_label.set_visible(false);
        }

        // 초기 선택 하이라이트
        self.update_selection();
    }

    /// 선택 하이라이트 업데이트
    fn update_selection(&self) {
        // 모든 셀에서 선택 제거
        for col_cells in &self.cells {
            for cell in col_cells {
                cell.remove_css_class("special-cell-selected");
            }
        }

        // 현재 선택 셀 하이라이트
        if self.sel_col < MAX_COLS && self.sel_row < MAX_ROWS {
            self.cells[self.sel_col][self.sel_row].add_css_class("special-cell-selected");
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

/// 특수문자 팝업용 CSS
pub fn popup_css() -> &'static str {
    r#"
    .unim-special-popup {
        background-color: rgba(30, 30, 46, 0.95);
        border: 1px solid rgba(255, 255, 255, 0.15);
        border-radius: 12px;
    }

    .special-top-row {
        margin-bottom: 4px;
    }

    .special-top-cell {
        color: #89b4fa;
        font-size: 13px;
        font-weight: 600;
        min-width: 30px;
        min-height: 20px;
    }

    .special-grid {
        background: transparent;
    }

    .special-cell {
        color: #cdd6f4;
        font-size: 14px;
        min-width: 30px;
        min-height: 30px;
        border-radius: 4px;
        padding: 2px;
    }

    .special-cell:hover {
        background-color: rgba(137, 180, 250, 0.15);
    }

    .special-cell-selected {
        background-color: rgba(137, 180, 250, 0.3);
        color: white;
        font-weight: 700;
    }

    .special-footer {
        color: #6c7086;
        font-size: 11px;
        padding: 2px 4px 0 0;
    }
    "#
}
