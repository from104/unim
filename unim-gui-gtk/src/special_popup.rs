//! 특수문자 팝업 윈도우
//!
//! DBus ShowSpecialPopup 시그널을 받아 특수문자 그리드를 표시합니다.
//! POPUP_SPEC.md Section 4 규격을 준수합니다.
//! 9x9 그리드, 열 우선(column-major) 채움 레이아웃.
//! GNOME extension과 동일한 디자인/레이아웃.

use gtk4::prelude::*;
use rust_i18n::t;
use unim::unim_log;

use crate::popup_positioning::{self, DisplayServer};

const MAX_ROWS: usize = 9;
const MAX_COLS: usize = 9;
const PAGE_SIZE: usize = MAX_ROWS * MAX_COLS; // 81

const ICON_PREV_PAGE: &str = "◀";
const ICON_NEXT_PAGE: &str = "▶";

/// 특수문자 팝업 상태
pub struct SpecialPopup {
    pub window: gtk4::Window,
    #[allow(dead_code)]
    grid: gtk4::Grid,
    header_label: gtk4::Label,
    footer_box: gtk4::Box,
    footer_label: gtk4::Label,
    /// 이전 페이지 버튼 (◀) — 단일 페이지 시 hide
    prev_page_btn: gtk4::Button,
    /// 다음 페이지 버튼 (▶) — 단일 페이지 시 hide
    next_page_btn: gtk4::Button,
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

        // 푸터: [◀] [target n/N] [▶]
        let footer_box = gtk4::Box::new(gtk4::Orientation::Horizontal, 4);
        footer_box.add_css_class("popup-footer-box");
        footer_box.set_halign(gtk4::Align::Fill);

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

        // AT-SPI 접근성
        window.update_property(&[gtk4::accessible::Property::Label("특수문자 선택")]);

        Self {
            window,
            grid,
            header_label,
            footer_box,
            footer_label,
            prev_page_btn,
            next_page_btn,
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
            self.display_server,
            x,
            y,
            h
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

    /// 열 헤더 업데이트 — 항상 9 컬럼 모두 표시 (top_row 가 짧으면 빈 라벨).
    /// 한자 popup expanded 모드와 마찬가지로 그리드 차원이 페이지마다 출렁이지
    /// 않도록 9×9 시각 일관성을 유지한다.
    fn update_col_headers(&self) {
        let chars: Vec<char> = self.top_row.chars().collect();
        for (i, header) in self.col_headers.iter().enumerate() {
            if i < chars.len() {
                header.set_text(&chars[i].to_string());
            } else {
                header.set_text("");
            }
            header.set_visible(true);
        }
    }

    /// 그리드 내용 업데이트 — 9×9 그리드 항상 유지 (페이지 항목 수 < 81 이어도).
    /// 빈 셀은 reactive=false 효과를 위해 `set_visible(true)` + 빈 텍스트로 표시.
    fn update_grid(&mut self) {
        let start = self.current_page * PAGE_SIZE;
        let page_count = (self.characters.len() - start).min(PAGE_SIZE);

        // 9×9 그리드 항상 강제 (한자 expanded 정책과 동일).
        // active_rows/active_cols 는 헤더/번호 강조에만 사용되므로 9 로 고정.
        self.active_rows = MAX_ROWS;
        self.active_cols = MAX_COLS;

        // 열 헤더 업데이트
        self.update_col_headers();

        // 행 번호 항상 9 행 모두 가시
        for num_label in self.row_numbers.iter() {
            num_label.set_visible(true);
        }

        // 열 우선(column-major) 채움 — 81 셀 전부 visible 유지, 데이터 없는 셀은
        // 빈 텍스트.
        for col in 0..MAX_COLS {
            for row in 0..MAX_ROWS {
                let idx = col * MAX_ROWS + row;
                let label = &self.cells[col][row];

                if idx < page_count {
                    let char_idx = start + idx;
                    label.set_text(&self.characters[char_idx]);
                } else {
                    label.set_text("");
                }
                label.set_visible(true);
                label.remove_css_class("selected");
            }
        }

        // 페이지 표시: 단일 페이지면 footer_box 자체 hide (◀/▶ 도 함께 사라짐)
        if self.total_pages > 1 {
            self.footer_label.set_text(&format!(
                "[{}]  {}/{}",
                self.target,
                self.current_page + 1,
                self.total_pages
            ));
            self.prev_page_btn.set_visible(true);
            self.next_page_btn.set_visible(true);
            self.footer_box.set_visible(true);
        } else {
            self.footer_box.set_visible(false);
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
    ///
    /// 9×9 그리드는 시각적으로 항상 고정 — 엔진이 보내는 rows/cols 는 시각
    /// 차원에 영향을 주지 않고, 선택 셀(sel_row/sel_col) 만 갱신한다.
    /// 한자 popup expanded 모드와 동일 정책.
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
            self.update_grid();
        }

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

    /// 통합 PopupRender 시그널 핸들러 (Phase B 통합 SoT).
    ///
    /// daemon 산출 view_model 의 미리 포맷된 문자열을 적용 — 헤더 ("「ㄱ」 → 특수문자")
    /// + footer ("[ㄱ] 1/3") + show_footer (단일 페이지면 footer hide).
    pub fn update_from_render(
        &mut self,
        header_text: &str,
        footer_text: &str,
        show_footer: bool,
    ) {
        self.header_label.set_text(header_text);
        self.footer_label.set_text(footer_text);
        self.footer_box.set_visible(show_footer);
        self.prev_page_btn.set_visible(show_footer);
        self.next_page_btn.set_visible(show_footer);
    }
}

/// DBus를 통해 특수문자 선택
fn select_special_via_dbus(col: usize, row: usize) {
    use unim_gui_common::types::ACTIVE_CONTEXT_PATH;

    // column-major 인덱스 계산
    let index = (col * MAX_ROWS + row) as u32;

    let context_path = { ACTIVE_CONTEXT_PATH.lock().ok().and_then(|p| p.clone()) };

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
                        let _: Result<String, _> = proxy.call("SelectSpecialChar", &(index,)).await;
                    }
                }
            });
        });
    }
}

/// 특수문자 팝업용 CSS — 한자 popup 의 generated CSS 에 통합되어 있다.
///
/// `tools/popup-styles/popup_tokens.toml` + `templates/gtk_hanja_popup.css.tmpl`
/// 에서 `.unim-special-popup` 룰셋이 함께 생성되어 `popup_styles.generated.css`
/// 에 포함된다 — `hanja_popup::popup_css()` 가 그 전체를 임베드하므로 본 함수는
/// 빈 문자열만 반환한다 (gtk_ui::load_css 의 concat 호환용 stub).
pub fn popup_css() -> &'static str {
    ""
}
