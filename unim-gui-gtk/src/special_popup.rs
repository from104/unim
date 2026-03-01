//! 특수문자 그리드 팝업 윈도우
//!
//! DBus `ShowSpecialPopup` 시그널을 수신하여
//! 커서 위치에 9×9 그리드 형태로 특수문자를 표시합니다.
//! 열 우선 채움 레이아웃. GTK4 기반, 기존 gtk-common/unim_special_popup.c 로직 재구현.

use gtk4::glib;
use gtk4::prelude::*;
use std::cell::RefCell;
use std::rc::Rc;
use unim::unim_log;

/// 그리드 행/열 수
const GRID_ROWS: usize = 9;
const GRID_COLS: usize = 9;
/// 페이지당 최대 문자 수
const PAGE_SIZE: usize = GRID_ROWS * GRID_COLS;
/// 선택 플래시 시간 (ms)
const FLASH_DURATION_MS: u32 = 120;

/// 특수문자 팝업 상태
struct SpecialState {
    /// 변환 대상 초성
    target: String,
    /// 전체 특수문자 목록
    characters: Vec<String>,
    /// 상단 행 레이블 (영문 키, e.g., "QWERTYUIO")
    top_row: String,
    /// 현재 페이지 (0-indexed)
    page: usize,
    /// 현재 선택 위치 (행, 열)
    sel_row: usize,
    sel_col: usize,
}

impl SpecialState {
    fn total_pages(&self) -> usize {
        if self.characters.is_empty() {
            1
        } else {
            (self.characters.len() + PAGE_SIZE - 1) / PAGE_SIZE
        }
    }

    /// 현재 페이지의 문자들 (column-major: col 0 = row[0..9], col 1 = row[9..18], ...)
    fn page_characters(&self) -> &[String] {
        let start = self.page * PAGE_SIZE;
        let end = (start + PAGE_SIZE).min(self.characters.len());
        if start >= self.characters.len() {
            &[]
        } else {
            &self.characters[start..end]
        }
    }

    /// (row, col) → flat index in page (열 우선)
    fn grid_to_flat(&self, row: usize, col: usize) -> usize {
        col * GRID_ROWS + row
    }

    /// 현재 선택의 글로벌 인덱스
    fn selected_global_index(&self) -> usize {
        self.page * PAGE_SIZE + self.grid_to_flat(self.sel_row, self.sel_col)
    }

    /// 현재 페이지 문자 수에서 유효한 열 수
    fn active_cols(&self) -> usize {
        let count = self.page_characters().len();
        if count == 0 {
            0
        } else {
            (count + GRID_ROWS - 1) / GRID_ROWS
        }
    }

    /// 특정 열의 유효 행 수
    fn rows_in_col(&self, col: usize) -> usize {
        let count = self.page_characters().len();
        let start = col * GRID_ROWS;
        if start >= count {
            0
        } else {
            (count - start).min(GRID_ROWS)
        }
    }
}

/// 특수문자 팝업 컨트롤러
pub struct SpecialPopup {
    window: gtk4::Window,
    grid_container: gtk4::Grid,
    header_box: gtk4::Box,
    page_label: gtk4::Label,
    target_label: gtk4::Label,
    state: Rc<RefCell<SpecialState>>,
    /// 선택 결과 콜백 (global index)
    on_select: Rc<RefCell<Option<Box<dyn Fn(usize)>>>>,
    /// 취소 콜백
    on_cancel: Rc<RefCell<Option<Box<dyn Fn()>>>>,
}

impl SpecialPopup {
    pub fn new() -> Self {
        let window = gtk4::Window::builder()
            .decorated(false)
            .resizable(false)
            .modal(false)
            .build();
        window.add_css_class("special-popup");

        let main_box = gtk4::Box::new(gtk4::Orientation::Vertical, 0);
        main_box.add_css_class("special-container");

        // 타겟 레이블
        let target_label = gtk4::Label::builder().halign(gtk4::Align::Start).build();
        target_label.add_css_class("special-target");
        main_box.append(&target_label);

        // 열 헤더 (top_row 키 레이블)
        let header_box = gtk4::Box::new(gtk4::Orientation::Horizontal, 0);
        header_box.add_css_class("special-header");
        main_box.append(&header_box);

        // 그리드
        let grid_container = gtk4::Grid::builder()
            .row_homogeneous(true)
            .column_homogeneous(true)
            .row_spacing(1)
            .column_spacing(1)
            .build();
        grid_container.add_css_class("special-grid");
        main_box.append(&grid_container);

        // 페이지 표시
        let page_label = gtk4::Label::builder().halign(gtk4::Align::Center).build();
        page_label.add_css_class("special-page");
        main_box.append(&page_label);

        window.set_child(Some(&main_box));

        let state = Rc::new(RefCell::new(SpecialState {
            target: String::new(),
            characters: Vec::new(),
            top_row: String::new(),
            page: 0,
            sel_row: 0,
            sel_col: 0,
        }));

        let popup = Self {
            window,
            grid_container,
            header_box,
            page_label,
            target_label,
            state,
            on_select: Rc::new(RefCell::new(None)),
            on_cancel: Rc::new(RefCell::new(None)),
        };

        popup.setup_key_handler();

        // 포커스 잃으면 자동 취소 (Alt+Tab 등으로 다른 앱 전환 시)
        {
            let on_cancel = popup.on_cancel.clone();
            let window = popup.window.clone();
            popup.window.connect_is_active_notify(move |w| {
                if !w.is_active() && w.is_visible() {
                    if let Some(ref cb) = *on_cancel.borrow() {
                        cb();
                    }
                    window.set_visible(false);
                }
            });
        }

        popup
    }

    /// 선택 콜백 설정
    pub fn connect_select<F: Fn(usize) + 'static>(&self, f: F) {
        *self.on_select.borrow_mut() = Some(Box::new(f));
    }

    /// 취소 콜백 설정
    pub fn connect_cancel<F: Fn() + 'static>(&self, f: F) {
        *self.on_cancel.borrow_mut() = Some(Box::new(f));
    }

    /// 팝업 표시
    pub fn show(
        &self,
        target: &str,
        characters: Vec<String>,
        top_row: &str,
        cursor_x: i32,
        cursor_y: i32,
        _cursor_width: i32,
        cursor_height: i32,
    ) {
        {
            let mut state = self.state.borrow_mut();
            state.target = target.to_string();
            state.characters = characters;
            state.top_row = top_row.to_string();
            state.page = 0;
            state.sel_row = 0;
            state.sel_col = 0;
        }

        self.target_label
            .set_text(&format!("「{}」 → 특수문자", target));
        self.refresh_grid();

        // 팝업 위치 설정 (X11: XMoveWindow, Wayland: no-op)
        crate::popup_position::position_popup_window(
            &self.window,
            cursor_x,
            cursor_y,
            cursor_height,
        );
        unim_log!(
            "INDICATOR",
            "특수문자 팝업 표시: target='{}', cursor=({},{})",
            target,
            cursor_x,
            cursor_y
        );
    }

    /// 팝업 숨김
    pub fn hide(&self) {
        self.window.set_visible(false);
    }

    /// 그리드 새로고침
    fn refresh_grid(&self) {
        // 기존 헤더 제거
        while let Some(child) = self.header_box.first_child() {
            self.header_box.remove(&child);
        }

        let state = self.state.borrow();

        // 행 번호 헤더 공간 (빈 라벨)
        let spacer = gtk4::Label::builder().label("  ").build();
        spacer.add_css_class("special-header-spacer");
        self.header_box.append(&spacer);

        // 열 헤더 (top_row 키)
        let top_chars: Vec<char> = state.top_row.chars().collect();
        let active_cols = state.active_cols();
        for col in 0..GRID_COLS.min(active_cols) {
            let key_char = top_chars.get(col).copied().unwrap_or(' ');
            let label = gtk4::Label::builder()
                .label(&key_char.to_string())
                .hexpand(true)
                .build();
            label.add_css_class("special-header-key");
            if col == state.sel_col {
                label.add_css_class("special-header-active");
            }
            self.header_box.append(&label);
        }

        // 그리드 자식 모두 제거
        while let Some(child) = self.grid_container.first_child() {
            self.grid_container.remove(&child);
        }

        let page_chars = state.page_characters();

        // 행 번호 + 셀 배치
        for row in 0..GRID_ROWS {
            // 행 번호 (1-9)
            let row_num = gtk4::Label::builder()
                .label(&format!("{}", row + 1))
                .build();
            row_num.add_css_class("special-row-num");
            self.grid_container.attach(&row_num, 0, row as i32, 1, 1);

            for col in 0..GRID_COLS.min(active_cols) {
                let flat_idx = col * GRID_ROWS + row;
                if flat_idx < page_chars.len() {
                    let ch = &page_chars[flat_idx];
                    let cell = gtk4::Label::builder()
                        .label(ch)
                        .hexpand(true)
                        .vexpand(true)
                        .build();
                    cell.add_css_class("special-cell");
                    if row == state.sel_row && col == state.sel_col {
                        cell.add_css_class("special-cell-selected");
                    }

                    // 클릭 이벤트
                    let on_select = self.on_select.clone();
                    let page = state.page;
                    let gesture = gtk4::GestureClick::new();
                    gesture.connect_released(move |_, _, _, _| {
                        let global_idx = page * PAGE_SIZE + flat_idx;
                        if let Some(ref cb) = *on_select.borrow() {
                            cb(global_idx);
                        }
                    });
                    cell.add_controller(gesture);

                    self.grid_container
                        .attach(&cell, (col + 1) as i32, row as i32, 1, 1);
                }
            }
        }

        // 페이지 표시
        let total = state.total_pages();
        if total > 1 {
            self.page_label
                .set_text(&format!("{}/{}", state.page + 1, total));
            self.page_label.set_visible(true);
        } else {
            self.page_label.set_visible(false);
        }
    }

    /// 키 핸들러 설정
    fn setup_key_handler(&self) {
        let state = self.state.clone();
        let on_select = self.on_select.clone();
        let on_cancel = self.on_cancel.clone();
        let window = self.window.clone();
        let grid_container = self.grid_container.clone();
        let header_box = self.header_box.clone();
        let page_label = self.page_label.clone();

        let controller = gtk4::EventControllerKey::new();
        controller.connect_key_pressed(move |_, key, _, _| {
            use gtk4::gdk::Key;

            match key {
                // 숫자 1-9: 현재 열에서 행 선택
                Key::_1
                | Key::_2
                | Key::_3
                | Key::_4
                | Key::_5
                | Key::_6
                | Key::_7
                | Key::_8
                | Key::_9 => {
                    let row = key.to_unicode().unwrap_or('0') as usize - '1' as usize;
                    let global_idx = {
                        let mut s = state.borrow_mut();
                        let valid_rows = s.rows_in_col(s.sel_col);
                        if row < valid_rows {
                            s.sel_row = row;
                            Some(s.selected_global_index())
                        } else {
                            None
                        }
                    };
                    if let Some(idx) = global_idx {
                        // 플래시 + 선택
                        flash_and_select(
                            &state,
                            &grid_container,
                            &header_box,
                            &page_label,
                            &on_select,
                            idx,
                        );
                    }
                    glib::Propagation::Stop
                }

                // Enter: 현재 선택 확정
                Key::Return | Key::KP_Enter => {
                    let global_idx = state.borrow().selected_global_index();
                    let valid = {
                        let s = state.borrow();
                        let flat = s.grid_to_flat(s.sel_row, s.sel_col);
                        flat < s.page_characters().len()
                    };
                    if valid {
                        flash_and_select(
                            &state,
                            &grid_container,
                            &header_box,
                            &page_label,
                            &on_select,
                            global_idx,
                        );
                    }
                    glib::Propagation::Stop
                }

                // Esc / BackSpace: 취소
                Key::Escape | Key::BackSpace => {
                    if let Some(ref cb) = *on_cancel.borrow() {
                        cb();
                    }
                    window.set_visible(false);
                    glib::Propagation::Stop
                }

                // 화살표 네비게이션
                Key::Up | Key::KP_Up => {
                    {
                        let mut s = state.borrow_mut();
                        if s.sel_row > 0 {
                            s.sel_row -= 1;
                        }
                    }
                    refresh_grid_static(
                        &state,
                        &grid_container,
                        &header_box,
                        &page_label,
                        &on_select,
                    );
                    glib::Propagation::Stop
                }
                Key::Down | Key::KP_Down => {
                    {
                        let mut s = state.borrow_mut();
                        let max_row = s.rows_in_col(s.sel_col);
                        if s.sel_row + 1 < max_row {
                            s.sel_row += 1;
                        }
                    }
                    refresh_grid_static(
                        &state,
                        &grid_container,
                        &header_box,
                        &page_label,
                        &on_select,
                    );
                    glib::Propagation::Stop
                }
                Key::Left | Key::KP_Left => {
                    {
                        let mut s = state.borrow_mut();
                        if s.sel_col > 0 {
                            s.sel_col -= 1;
                            // 행 범위 보정
                            let max_row = s.rows_in_col(s.sel_col);
                            if s.sel_row >= max_row && max_row > 0 {
                                s.sel_row = max_row - 1;
                            }
                        } else if s.page > 0 {
                            s.page -= 1;
                            s.sel_col = s.active_cols().saturating_sub(1);
                            s.sel_row = 0;
                        }
                    }
                    refresh_grid_static(
                        &state,
                        &grid_container,
                        &header_box,
                        &page_label,
                        &on_select,
                    );
                    glib::Propagation::Stop
                }
                Key::Right | Key::KP_Right => {
                    {
                        let mut s = state.borrow_mut();
                        let active = s.active_cols();
                        if s.sel_col + 1 < active {
                            s.sel_col += 1;
                            let max_row = s.rows_in_col(s.sel_col);
                            if s.sel_row >= max_row && max_row > 0 {
                                s.sel_row = max_row - 1;
                            }
                        } else if s.page + 1 < s.total_pages() {
                            s.page += 1;
                            s.sel_col = 0;
                            s.sel_row = 0;
                        }
                    }
                    refresh_grid_static(
                        &state,
                        &grid_container,
                        &header_box,
                        &page_label,
                        &on_select,
                    );
                    glib::Propagation::Stop
                }

                // top_row 키로 열 점프
                other => {
                    if let Some(ch) = other.to_unicode() {
                        let ch_upper = ch.to_ascii_uppercase();
                        let top_row = state.borrow().top_row.clone();
                        if let Some(col) = top_row.chars().position(|c| c == ch_upper) {
                            let valid = {
                                let s = state.borrow();
                                col < s.active_cols()
                            };
                            if valid {
                                {
                                    let mut s = state.borrow_mut();
                                    s.sel_col = col;
                                    let max_row = s.rows_in_col(col);
                                    if s.sel_row >= max_row && max_row > 0 {
                                        s.sel_row = max_row - 1;
                                    }
                                }
                                refresh_grid_static(
                                    &state,
                                    &grid_container,
                                    &header_box,
                                    &page_label,
                                    &on_select,
                                );
                                return glib::Propagation::Stop;
                            }
                        }
                    }
                    glib::Propagation::Proceed
                }
            }
        });

        self.window.add_controller(controller);
    }
}

/// 그리드 갱신 정적 함수
fn refresh_grid_static(
    state: &Rc<RefCell<SpecialState>>,
    grid: &gtk4::Grid,
    header_box: &gtk4::Box,
    page_label: &gtk4::Label,
    on_select: &Rc<RefCell<Option<Box<dyn Fn(usize)>>>>,
) {
    // 헤더 갱신
    while let Some(child) = header_box.first_child() {
        header_box.remove(&child);
    }

    let s = state.borrow();

    let spacer = gtk4::Label::builder().label("  ").build();
    spacer.add_css_class("special-header-spacer");
    header_box.append(&spacer);

    let top_chars: Vec<char> = s.top_row.chars().collect();
    let active_cols = s.active_cols();
    for col in 0..GRID_COLS.min(active_cols) {
        let key_char = top_chars.get(col).copied().unwrap_or(' ');
        let label = gtk4::Label::builder()
            .label(&key_char.to_string())
            .hexpand(true)
            .build();
        label.add_css_class("special-header-key");
        if col == s.sel_col {
            label.add_css_class("special-header-active");
        }
        header_box.append(&label);
    }

    // 그리드 갱신
    while let Some(child) = grid.first_child() {
        grid.remove(&child);
    }

    let page_chars = s.page_characters();

    for row in 0..GRID_ROWS {
        let row_num = gtk4::Label::builder()
            .label(&format!("{}", row + 1))
            .build();
        row_num.add_css_class("special-row-num");
        grid.attach(&row_num, 0, row as i32, 1, 1);

        for col in 0..GRID_COLS.min(active_cols) {
            let flat_idx = col * GRID_ROWS + row;
            if flat_idx < page_chars.len() {
                let ch = &page_chars[flat_idx];
                let cell = gtk4::Label::builder()
                    .label(ch)
                    .hexpand(true)
                    .vexpand(true)
                    .build();
                cell.add_css_class("special-cell");
                if row == s.sel_row && col == s.sel_col {
                    cell.add_css_class("special-cell-selected");
                }

                let on_sel = on_select.clone();
                let page = s.page;
                let gesture = gtk4::GestureClick::new();
                gesture.connect_released(move |_, _, _, _| {
                    let global_idx = page * PAGE_SIZE + flat_idx;
                    if let Some(ref cb) = *on_sel.borrow() {
                        cb(global_idx);
                    }
                });
                cell.add_controller(gesture);

                grid.attach(&cell, (col + 1) as i32, row as i32, 1, 1);
            }
        }
    }

    let total = s.total_pages();
    if total > 1 {
        page_label.set_text(&format!("{}/{}", s.page + 1, total));
        page_label.set_visible(true);
    } else {
        page_label.set_visible(false);
    }
}

/// 선택 플래시 효과 후 콜백 호출
fn flash_and_select(
    state: &Rc<RefCell<SpecialState>>,
    grid: &gtk4::Grid,
    header_box: &gtk4::Box,
    page_label: &gtk4::Label,
    on_select: &Rc<RefCell<Option<Box<dyn Fn(usize)>>>>,
    global_idx: usize,
) {
    // 선택 하이라이트 갱신
    refresh_grid_static(state, grid, header_box, page_label, on_select);

    // 플래시 후 선택 실행
    let on_select_clone = on_select.clone();
    glib::timeout_add_local_once(
        std::time::Duration::from_millis(FLASH_DURATION_MS as u64),
        move || {
            if let Some(ref cb) = *on_select_clone.borrow() {
                cb(global_idx);
            }
        },
    );
}
