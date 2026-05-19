//! 4행 × 가변열 키보드 위젯 + 오타 히트맵 overlay.
//!
//! `gtk::Grid`에 각 셀을 `gtk::Button`으로 깐다. 위에 같은 사이즈의
//! `gtk::DrawingArea`를 `gtk::Overlay`로 얹어 cairo로 그라데이션을 그린다.
//! Overlay는 `set_can_target(false)`로 입력을 통과시켜 버튼 클릭을 가린다.
//!
//! 두 바이너리에서 공유:
//! - **studio**: 셀 클릭 → 키 정보 패널/편집 다이얼로그
//! - **typing-practice**: read-only + 종료 후 히트맵 paint

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use gtk4::prelude::*;
use gtk4::{self as gtk};

use unim::keystroke::profile::{LayoutProfile, LayoutRows};

/// 셀 위치 식별자 — 행(0..4) × 열(0..N).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct KeyCell {
    pub row: u8,
    pub col: u8,
}

impl KeyCell {
    pub fn as_pair(&self) -> (u8, u8) {
        (self.row, self.col)
    }
}

/// 한 셀의 시도/오타 통계 — 히트맵 입력.
#[derive(Debug, Default, Clone, Copy)]
pub struct KeyStat {
    pub attempts: u32,
    pub errors: u32,
}

impl KeyStat {
    pub fn ratio(&self) -> f64 {
        if self.attempts == 0 {
            0.0
        } else {
            self.errors as f64 / self.attempts as f64
        }
    }
}

/// 4×N 키보드 위젯.
pub struct KeyboardWidget {
    root: gtk::Overlay,
    grid: gtk::Grid,
    drawing: gtk::DrawingArea,
    /// 셀 버튼: `cells[row][col]`. 행마다 길이가 다를 수 있다(LayoutRows 따라).
    cells: Rc<RefCell<Vec<Vec<gtk::Button>>>>,
    stats: Rc<RefCell<HashMap<(u8, u8), KeyStat>>>,
    #[allow(clippy::type_complexity)] // GTK4 callback boxing — alias 시 자식 모듈에서 가독성 저하
    on_select: Rc<RefCell<Option<Box<dyn Fn(KeyCell)>>>>,
    selected: Rc<RefCell<Option<KeyCell>>>,
    upper: Rc<RefCell<bool>>,
}

impl KeyboardWidget {
    pub fn new() -> Self {
        let grid = gtk::Grid::builder()
            .row_spacing(4)
            .column_spacing(4)
            .halign(gtk::Align::Center)
            .valign(gtk::Align::Center)
            .margin_top(12)
            .margin_bottom(12)
            .margin_start(12)
            .margin_end(12)
            .build();

        let drawing = gtk::DrawingArea::new();
        drawing.set_can_target(false);
        drawing.set_hexpand(true);
        drawing.set_vexpand(true);

        let overlay = gtk::Overlay::new();
        overlay.set_child(Some(&grid));
        overlay.add_overlay(&drawing);

        let stats: Rc<RefCell<HashMap<(u8, u8), KeyStat>>> = Rc::new(RefCell::new(HashMap::new()));
        let cells: Rc<RefCell<Vec<Vec<gtk::Button>>>> = Rc::new(RefCell::new(Vec::new()));

        {
            let cells = cells.clone();
            let stats = stats.clone();
            drawing.set_draw_func(move |_, cr, _, _| {
                paint_heatmap(cr, &cells.borrow(), &stats.borrow());
            });
        }

        Self {
            root: overlay,
            grid,
            drawing,
            cells,
            stats,
            on_select: Rc::new(RefCell::new(None)),
            selected: Rc::new(RefCell::new(None)),
            upper: Rc::new(RefCell::new(false)),
        }
    }

    /// 위젯의 root (Overlay) — 부모 컨테이너에 append.
    pub fn root(&self) -> &gtk::Overlay {
        &self.root
    }

    /// Upper/Lower 모드 변경. 라벨 갱신은 호출자가 `populate(profile)` 재호출로.
    pub fn set_upper(&self, upper: bool) {
        *self.upper.borrow_mut() = upper;
    }

    pub fn is_upper(&self) -> bool {
        *self.upper.borrow()
    }

    /// 셀 클릭 콜백 등록. 새 콜백이 이전 콜백을 대체.
    pub fn set_on_select<F: Fn(KeyCell) + 'static>(&self, cb: F) {
        *self.on_select.borrow_mut() = Some(Box::new(cb));
    }

    /// 외부에서 선택 상태를 설정.
    pub fn select_cell(&self, cell: Option<KeyCell>) {
        let prev = *self.selected.borrow();
        if let Some(p) = prev {
            if let Some(btn) = self.cell_button(p) {
                btn.remove_css_class("keymap-cell-selected");
            }
        }
        *self.selected.borrow_mut() = cell;
        if let Some(c) = cell {
            if let Some(btn) = self.cell_button(c) {
                btn.add_css_class("keymap-cell-selected");
            }
        }
    }

    pub fn selected(&self) -> Option<KeyCell> {
        *self.selected.borrow()
    }

    /// 프로필을 받아 그리드 셀을 새로 만든다.
    pub fn populate(&self, profile: &LayoutProfile) {
        while let Some(c) = self.grid.first_child() {
            self.grid.remove(&c);
        }
        self.cells.borrow_mut().clear();
        *self.selected.borrow_mut() = None;

        let rows_ref = if self.is_upper() {
            &profile.layout.upper
        } else {
            &profile.layout.lower
        };
        let rows = [
            &rows_ref.row1,
            &rows_ref.row2,
            &rows_ref.row3,
            &rows_ref.row4,
        ];

        // QWERTY stagger — 단순 들여쓰기.
        let row_offsets: [i32; 4] = [0, 0, 1, 2];

        let mut grid_cells: Vec<Vec<gtk::Button>> = Vec::with_capacity(4);
        for (r_idx, row) in rows.iter().enumerate() {
            let mut row_buttons = Vec::with_capacity(row.len());
            for (c_idx, label) in row.iter().enumerate() {
                let btn = make_key_button(label);
                let cell = KeyCell {
                    row: r_idx as u8,
                    col: c_idx as u8,
                };
                let on_select = self.on_select.clone();
                let selected = self.selected.clone();
                let cells_handle = self.cells.clone();
                btn.connect_clicked(move |_| {
                    let prev = *selected.borrow();
                    if let Some(p) = prev {
                        if let Some(cells) = cells_handle.borrow().get(p.row as usize) {
                            if let Some(b) = cells.get(p.col as usize) {
                                b.remove_css_class("keymap-cell-selected");
                            }
                        }
                    }
                    *selected.borrow_mut() = Some(cell);
                    if let Some(cells) = cells_handle.borrow().get(cell.row as usize) {
                        if let Some(b) = cells.get(cell.col as usize) {
                            b.add_css_class("keymap-cell-selected");
                        }
                    }
                    if let Some(cb) = on_select.borrow().as_ref() {
                        cb(cell);
                    }
                });
                self.grid.attach(
                    &btn,
                    c_idx as i32 + row_offsets[r_idx],
                    r_idx as i32,
                    1,
                    1,
                );
                row_buttons.push(btn);
            }
            grid_cells.push(row_buttons);
        }
        *self.cells.borrow_mut() = grid_cells;
        self.drawing.queue_draw();
    }

    fn cell_button(&self, cell: KeyCell) -> Option<gtk::Button> {
        let cells = self.cells.borrow();
        cells
            .get(cell.row as usize)
            .and_then(|row| row.get(cell.col as usize).cloned())
    }

    /// 셀 라벨 조회 — 편집/정보 패널용.
    pub fn cell_label(&self, cell: KeyCell) -> Option<String> {
        self.cell_button(cell).map(|b| {
            b.child()
                .and_then(|c| c.downcast::<gtk::Label>().ok())
                .map(|l| l.text().to_string())
                .unwrap_or_default()
        })
    }

    /// 히트맵 갱신 + 재페인트.
    pub fn set_heatmap(&self, stats: HashMap<(u8, u8), KeyStat>) {
        *self.stats.borrow_mut() = stats;
        self.drawing.queue_draw();
    }

    /// 히트맵 클리어.
    pub fn clear_heatmap(&self) {
        self.stats.borrow_mut().clear();
        self.drawing.queue_draw();
    }
}

impl Default for KeyboardWidget {
    fn default() -> Self {
        Self::new()
    }
}

/// LayoutRows의 한 행에서 col 인덱스의 라벨을 가져온다 (없으면 빈 문자열).
pub fn cell_label_at(rows: &LayoutRows, row: u8, col: u8) -> &str {
    let row_slice: &Vec<String> = match row {
        0 => &rows.row1,
        1 => &rows.row2,
        2 => &rows.row3,
        3 => &rows.row4,
        _ => return "",
    };
    row_slice.get(col as usize).map(|s| s.as_str()).unwrap_or("")
}

fn make_key_button(label: &str) -> gtk::Button {
    let lbl = gtk::Label::builder()
        .label(if label.is_empty() { " " } else { label })
        .single_line_mode(true)
        .build();
    gtk::Button::builder()
        .width_request(46)
        .height_request(46)
        .css_classes(["keymap-cell"])
        .child(&lbl)
        .build()
}

fn paint_heatmap(
    cr: &cairo::Context,
    cells: &[Vec<gtk::Button>],
    stats: &HashMap<(u8, u8), KeyStat>,
) {
    if stats.is_empty() {
        return;
    }
    for (r_idx, row) in cells.iter().enumerate() {
        for (c_idx, btn) in row.iter().enumerate() {
            let stat = stats
                .get(&(r_idx as u8, c_idx as u8))
                .copied()
                .unwrap_or_default();
            if stat.attempts == 0 {
                continue;
            }
            let alloc = btn.allocation();
            // 버튼 좌표(부모 = Grid)를 overlay(DrawingArea) 좌표계로 변환.
            // Grid는 Overlay의 첫 child이므로 grid 자체의 offset도 더해 준다.
            let parent_x = btn.parent().map(|p| p.allocation().x()).unwrap_or(0);
            let parent_y = btn.parent().map(|p| p.allocation().y()).unwrap_or(0);
            let x = (alloc.x() + parent_x) as f64;
            let y = (alloc.y() + parent_y) as f64;
            let w = alloc.width() as f64;
            let h = alloc.height() as f64;
            let (r, g, b) = ratio_to_rgb(stat.ratio());
            cr.set_source_rgba(r, g, b, 0.45);
            cr.rectangle(x, y, w, h);
            let _ = cr.fill();
        }
    }
}

/// 0.0(=녹) ~ 0.5(=황) ~ 1.0(=적) 보간을 RGB로.
pub fn ratio_to_rgb(ratio: f64) -> (f64, f64, f64) {
    let r = ratio.clamp(0.0, 1.0);
    if r < 0.5 {
        let t = r / 0.5;
        (t, 1.0, 0.0)
    } else {
        let t = (r - 0.5) / 0.5;
        (1.0, 1.0 - t, 0.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ratio_to_rgb_endpoints() {
        let (r, g, b) = ratio_to_rgb(0.0);
        assert!((r - 0.0).abs() < 1e-9);
        assert!((g - 1.0).abs() < 1e-9);
        assert!((b - 0.0).abs() < 1e-9);

        let (r, g, b) = ratio_to_rgb(1.0);
        assert!((r - 1.0).abs() < 1e-9);
        assert!((g - 0.0).abs() < 1e-9);
        assert!((b - 0.0).abs() < 1e-9);
    }

    #[test]
    fn ratio_to_rgb_midpoint_is_yellow() {
        let (r, g, b) = ratio_to_rgb(0.5);
        assert!((r - 1.0).abs() < 1e-9);
        assert!((g - 1.0).abs() < 1e-9);
        assert!((b - 0.0).abs() < 1e-9);
    }

    #[test]
    fn keystat_ratio_zero_attempts() {
        let s = KeyStat::default();
        assert_eq!(s.ratio(), 0.0);
    }

    #[test]
    fn keystat_ratio_half() {
        let s = KeyStat {
            attempts: 4,
            errors: 2,
        };
        assert!((s.ratio() - 0.5).abs() < 1e-9);
    }

    #[test]
    fn cell_label_at_returns_empty_for_oob() {
        let rows = LayoutRows {
            row1: vec!["a".into(), "b".into()],
            row2: vec![],
            row3: vec![],
            row4: vec![],
        };
        assert_eq!(cell_label_at(&rows, 0, 0), "a");
        assert_eq!(cell_label_at(&rows, 0, 1), "b");
        assert_eq!(cell_label_at(&rows, 0, 5), "");
        assert_eq!(cell_label_at(&rows, 7, 0), "");
    }
}
