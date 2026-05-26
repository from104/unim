//! 공유 5행 stagger 키보드 위젯 — studio(편집)·typing-practice(연습) 공용.
//!
//! `gtk::Fixed` 절대 배치로 표준 106키 모양(stagger 5줄)을 그린다. 키 한 칸은
//! 4-corner 라벨: 좌상 영문 upper / 좌하 영문 lower / 우상 한글 upper / 우하 한글 lower.
//!
//! 두 모드를 한 위젯으로 지원:
//! - **클릭(studio)**: `set_on_select` 등록 시 글자키 클릭 → `(row,col)` 콜백 + 선택 표시.
//! - **연습(typing)**: `flash_key`(키 눌림 피드백) + `set_heatmap`(오타 5단계 색).
//!
//! `set_on_select`를 부르지 않으면 클릭 제스처·`kbv-clickable`이 전혀 붙지 않아
//! 연습 모드 외형/동작은 영향받지 않는다.

use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::rc::Rc;

use gtk4::glib::translate::{FromGlib, IntoGlib};
use gtk4::prelude::*;
use gtk4::{self as gtk, glib};

use unim::keystroke::profile::LayoutProfile;

use crate::keyboard_widget::{cell_label_at, KeyStat};

const UNIT: f32 = 46.0; // 1u 키 픽셀 크기.
// 한글 라벨(글자 높이가 ASCII 보다 큼)이 키 셀 min 높이 안에 들어오도록 여유를 둔다.
// 그러면 한글·영문 모두 height_request(min)로 클램프되어 행 높이가 일정해진다.
const ROW_HEIGHT: f32 = 54.0;
/// 키 사이 시각 간격(px).
const KEY_GAP: f32 = 5.0;

const HEAT_LEVELS: [&str; 5] = [
    "kbv-heat-1",
    "kbv-heat-2",
    "kbv-heat-3",
    "kbv-heat-4",
    "kbv-heat-5",
];

/// QWERTY 기준 ASCII 키 → (row, col) 매핑.
/// 숫자열 끝(index 13)에 backslash(`\`/`|`) 가 저장된다 (UNIM 키맵 데이터 규약).
pub fn qwerty_position(byte: u8) -> Option<(u8, u8)> {
    let b = byte.to_ascii_lowercase();
    const ROW0: &[u8] = b"`1234567890-=\\";
    const ROW1: &[u8] = b"qwertyuiop[]";
    const ROW2: &[u8] = b"asdfghjkl;'";
    const ROW3: &[u8] = b"zxcvbnm,./";
    if let Some(i) = ROW0.iter().position(|&c| c == b) {
        return Some((0, i as u8));
    }
    if let Some(i) = ROW1.iter().position(|&c| c == b) {
        return Some((1, i as u8));
    }
    if let Some(i) = ROW2.iter().position(|&c| c == b) {
        return Some((2, i as u8));
    }
    if let Some(i) = ROW3.iter().position(|&c| c == b) {
        return Some((3, i as u8));
    }
    None
}

/// 키 한 칸의 정의 — 위에서 아래, 왼쪽에서 오른쪽 순서로 배치.
struct KeyDef {
    /// GDK keyval lowercase 기준 — `flash_key` press/release 매칭. 없으면 미등록.
    keyval: Option<u32>,
    eng_lower: &'static str,
    eng_upper: &'static str,
    /// 키 너비 multiplier (1.0 = 표준 키).
    width: f32,
    /// QWERTY 위치·한글 라벨 산출용 ASCII byte. None 이면 특수키(라벨/매핑 없음).
    ascii: Option<u8>,
    /// 특수키 가운데 라벨 ("Tab"·"Shift"…). None 이면 4-corner 글자키.
    special_name: Option<&'static str>,
}

fn kd(
    keyval: Option<u32>,
    eng_lower: &'static str,
    eng_upper: &'static str,
    width: f32,
    ascii: Option<u8>,
    special_name: Option<&'static str>,
) -> KeyDef {
    KeyDef {
        keyval,
        eng_lower,
        eng_upper,
        width,
        ascii,
        special_name,
    }
}

/// 5행 키보드 layout — stagger offset (key units) 와 키 정의.
/// 표준 ANSI 104 + 한/영·한자 (=106). 1~5행 폭이 정확히 15.0u 로 일치.
fn build_layout() -> Vec<(f32, Vec<KeyDef>)> {
    use gtk::gdk::Key;
    vec![
        (
            0.0,
            vec![
                kd(Some(Key::grave.into_glib()), "`", "~", 1.0, Some(b'`'), None),
                kd(Some(Key::_1.into_glib()), "1", "!", 1.0, Some(b'1'), None),
                kd(Some(Key::_2.into_glib()), "2", "@", 1.0, Some(b'2'), None),
                kd(Some(Key::_3.into_glib()), "3", "#", 1.0, Some(b'3'), None),
                kd(Some(Key::_4.into_glib()), "4", "$", 1.0, Some(b'4'), None),
                kd(Some(Key::_5.into_glib()), "5", "%", 1.0, Some(b'5'), None),
                kd(Some(Key::_6.into_glib()), "6", "^", 1.0, Some(b'6'), None),
                kd(Some(Key::_7.into_glib()), "7", "&", 1.0, Some(b'7'), None),
                kd(Some(Key::_8.into_glib()), "8", "*", 1.0, Some(b'8'), None),
                kd(Some(Key::_9.into_glib()), "9", "(", 1.0, Some(b'9'), None),
                kd(Some(Key::_0.into_glib()), "0", ")", 1.0, Some(b'0'), None),
                kd(Some(Key::minus.into_glib()), "-", "_", 1.0, Some(b'-'), None),
                kd(Some(Key::equal.into_glib()), "=", "+", 1.0, Some(b'='), None),
                kd(Some(Key::BackSpace.into_glib()), "", "", 2.0, None, Some("Backspace")),
            ],
        ),
        (
            0.0,
            vec![
                kd(Some(Key::Tab.into_glib()), "", "", 1.5, None, Some("Tab")),
                kd(Some(Key::q.into_glib()), "q", "Q", 1.0, Some(b'q'), None),
                kd(Some(Key::w.into_glib()), "w", "W", 1.0, Some(b'w'), None),
                kd(Some(Key::e.into_glib()), "e", "E", 1.0, Some(b'e'), None),
                kd(Some(Key::r.into_glib()), "r", "R", 1.0, Some(b'r'), None),
                kd(Some(Key::t.into_glib()), "t", "T", 1.0, Some(b't'), None),
                kd(Some(Key::y.into_glib()), "y", "Y", 1.0, Some(b'y'), None),
                kd(Some(Key::u.into_glib()), "u", "U", 1.0, Some(b'u'), None),
                kd(Some(Key::i.into_glib()), "i", "I", 1.0, Some(b'i'), None),
                kd(Some(Key::o.into_glib()), "o", "O", 1.0, Some(b'o'), None),
                kd(Some(Key::p.into_glib()), "p", "P", 1.0, Some(b'p'), None),
                kd(Some(Key::bracketleft.into_glib()), "[", "{", 1.0, Some(b'['), None),
                kd(Some(Key::bracketright.into_glib()), "]", "}", 1.0, Some(b']'), None),
                kd(Some(Key::backslash.into_glib()), "\\", "|", 1.5, Some(b'\\'), None),
            ],
        ),
        (
            0.0,
            vec![
                kd(Some(Key::Caps_Lock.into_glib()), "", "", 1.75, None, Some("Caps")),
                kd(Some(Key::a.into_glib()), "a", "A", 1.0, Some(b'a'), None),
                kd(Some(Key::s.into_glib()), "s", "S", 1.0, Some(b's'), None),
                kd(Some(Key::d.into_glib()), "d", "D", 1.0, Some(b'd'), None),
                kd(Some(Key::f.into_glib()), "f", "F", 1.0, Some(b'f'), None),
                kd(Some(Key::g.into_glib()), "g", "G", 1.0, Some(b'g'), None),
                kd(Some(Key::h.into_glib()), "h", "H", 1.0, Some(b'h'), None),
                kd(Some(Key::j.into_glib()), "j", "J", 1.0, Some(b'j'), None),
                kd(Some(Key::k.into_glib()), "k", "K", 1.0, Some(b'k'), None),
                kd(Some(Key::l.into_glib()), "l", "L", 1.0, Some(b'l'), None),
                kd(Some(Key::semicolon.into_glib()), ";", ":", 1.0, Some(b';'), None),
                kd(Some(Key::apostrophe.into_glib()), "'", "\"", 1.0, Some(b'\''), None),
                kd(Some(Key::Return.into_glib()), "", "", 2.25, None, Some("Enter")),
            ],
        ),
        (
            0.0,
            vec![
                kd(Some(Key::Shift_L.into_glib()), "", "", 2.25, None, Some("Shift")),
                kd(Some(Key::z.into_glib()), "z", "Z", 1.0, Some(b'z'), None),
                kd(Some(Key::x.into_glib()), "x", "X", 1.0, Some(b'x'), None),
                kd(Some(Key::c.into_glib()), "c", "C", 1.0, Some(b'c'), None),
                kd(Some(Key::v.into_glib()), "v", "V", 1.0, Some(b'v'), None),
                kd(Some(Key::b.into_glib()), "b", "B", 1.0, Some(b'b'), None),
                kd(Some(Key::n.into_glib()), "n", "N", 1.0, Some(b'n'), None),
                kd(Some(Key::m.into_glib()), "m", "M", 1.0, Some(b'm'), None),
                kd(Some(Key::comma.into_glib()), ",", "<", 1.0, Some(b','), None),
                kd(Some(Key::period.into_glib()), ".", ">", 1.0, Some(b'.'), None),
                kd(Some(Key::slash.into_glib()), "/", "?", 1.0, Some(b'/'), None),
                kd(Some(Key::Shift_R.into_glib()), "", "", 2.75, None, Some("Shift")),
            ],
        ),
        (
            0.0,
            vec![
                kd(Some(Key::Control_L.into_glib()), "", "", 1.5, None, Some("Ctrl")),
                kd(None, "", "", 1.25, None, Some("Meta")),
                kd(Some(Key::Alt_L.into_glib()), "", "", 1.5, None, Some("Alt")),
                kd(Some(Key::Hangul_Hanja.into_glib()), "", "", 1.25, None, Some("한자")),
                kd(Some(Key::space.into_glib()), "", "", 4.0, None, Some("Space")),
                kd(Some(Key::Hangul.into_glib()), "", "", 1.25, None, Some("한/영")),
                kd(Some(Key::Alt_R.into_glib()), "", "", 1.5, None, Some("Alt")),
                kd(None, "", "", 1.25, None, Some("Menu")),
                kd(Some(Key::Control_R.into_glib()), "", "", 1.5, None, Some("Ctrl")),
            ],
        ),
    ]
}

type SelectCallback = Rc<RefCell<Option<Box<dyn Fn(u8, u8)>>>>;

/// 공유 5행 키보드 위젯.
pub struct KeyboardView {
    root: gtk::Fixed,
    /// keyval → 키 widget — `flash_key` press 피드백.
    by_keyval: Rc<RefCell<HashMap<u32, gtk::Widget>>>,
    /// (row, col) QWERTY 셀 → Frame — 선택 표시(studio)·히트맵(typing).
    by_cell: Rc<RefCell<HashMap<(u8, u8), gtk::Frame>>>,
    on_select: SelectCallback,
    selected: Rc<RefCell<Option<(u8, u8)>>>,
    /// `set_on_select` 호출 시 true — 글자키에 클릭 제스처·`kbv-clickable` 부여.
    clickable: Rc<Cell<bool>>,
}

impl KeyboardView {
    /// 빈 위젯 생성. 라벨은 `populate`로 채운다 (studio: 프로필 변경 시 재호출).
    pub fn new() -> Rc<Self> {
        let root = gtk::Fixed::builder()
            .halign(gtk::Align::Center)
            .hexpand(false)
            .build();
        // 폭 700 (15u × 46 = 690 + 안전 10). 높이 = 5행 × ROW_HEIGHT − KEY_GAP/2.
        root.set_size_request(700, (ROW_HEIGHT * 5.0 - KEY_GAP * 0.5).ceil() as i32);
        Rc::new(Self {
            root,
            by_keyval: Rc::new(RefCell::new(HashMap::new())),
            by_cell: Rc::new(RefCell::new(HashMap::new())),
            on_select: Rc::new(RefCell::new(None)),
            selected: Rc::new(RefCell::new(None)),
            clickable: Rc::new(Cell::new(false)),
        })
    }

    /// 프로필과 함께 생성 (typing: read-only 표시기 — 생성 즉시 채움).
    pub fn with_profiles(
        ko_profile: Option<&LayoutProfile>,
        en_profile: Option<&LayoutProfile>,
    ) -> Rc<Self> {
        let view = Self::new();
        view.populate(ko_profile, en_profile);
        view
    }

    pub fn root(&self) -> &gtk::Fixed {
        &self.root
    }

    /// 셀 클릭 콜백 등록 — 등록 즉시 클릭 모드 활성화(글자키 제스처·hover).
    pub fn set_on_select<F: Fn(u8, u8) + 'static>(self: &Rc<Self>, f: F) {
        *self.on_select.borrow_mut() = Some(Box::new(f));
        self.clickable.set(true);
    }

    /// 자판 라벨 재구성. ko_profile 의 lower/upper 한글 + en_profile 영문 라벨.
    pub fn populate(
        self: &Rc<Self>,
        ko_profile: Option<&LayoutProfile>,
        en_profile: Option<&LayoutProfile>,
    ) {
        while let Some(c) = self.root.first_child() {
            self.root.remove(&c);
        }
        self.by_keyval.borrow_mut().clear();
        self.by_cell.borrow_mut().clear();

        let clickable = self.clickable.get();
        let layout = build_layout();
        for (r_idx, (offset, row)) in layout.iter().enumerate() {
            let mut x: f32 = offset * UNIT;
            let y: f32 = r_idx as f32 * ROW_HEIGHT;
            for key in row {
                let w = key.width * UNIT - KEY_GAP;
                let h = ROW_HEIGHT - KEY_GAP;
                let cell = build_key_cell(key, ko_profile, en_profile, w as i32, h as i32);
                self.root
                    .put(&cell, (x + KEY_GAP * 0.5) as f64, (y + KEY_GAP * 0.5) as f64);

                if let Some(kv) = key.keyval {
                    self.by_keyval.borrow_mut().insert(kv, cell.clone().upcast());
                }

                if let Some(byte) = key.ascii {
                    if let Some((r, c)) = qwerty_position(byte) {
                        self.by_cell.borrow_mut().insert((r, c), cell.clone());
                        if clickable {
                            let gesture = gtk::GestureClick::new();
                            let this = Rc::downgrade(self);
                            gesture.connect_released(move |_, _, _, _| {
                                if let Some(this) = this.upgrade() {
                                    this.select_cell(Some((r, c)));
                                    if let Some(cb) = this.on_select.borrow().as_ref() {
                                        cb(r, c);
                                    }
                                }
                            });
                            cell.add_controller(gesture);
                            cell.add_css_class("kbv-clickable");
                        }
                    }
                }
                x += key.width * UNIT;
            }
        }

        // 선택 표시 복원 (studio).
        let sel = *self.selected.borrow();
        if sel.is_some() {
            self.select_cell(sel);
        }
    }

    /// 셀 선택 표시 토글 (studio).
    pub fn select_cell(&self, cell: Option<(u8, u8)>) {
        let map = self.by_cell.borrow();
        for frame in map.values() {
            frame.remove_css_class("kbv-selected");
        }
        *self.selected.borrow_mut() = cell;
        if let Some(c) = cell {
            if let Some(frame) = map.get(&c) {
                frame.add_css_class("kbv-selected");
            }
        }
    }

    /// 키 눌림 시각 피드백 (typing) — `kbv-pressed` 추가 후 150ms 뒤 자동 제거.
    pub fn flash_key(self: &Rc<Self>, keyval: u32) {
        let lower = unsafe {
            let k = gtk::gdk::Key::from_glib(keyval);
            k.to_lower().into_glib()
        };
        let cell = {
            let map = self.by_keyval.borrow();
            map.get(&keyval).or_else(|| map.get(&lower)).cloned()
        };
        if let Some(cell) = cell {
            cell.add_css_class("kbv-pressed");
            glib::timeout_add_local_once(std::time::Duration::from_millis(150), move || {
                cell.remove_css_class("kbv-pressed");
            });
        }
    }

    /// 히트맵 색 오버레이 (typing) — QWERTY 셀별 오타 빈도를 5단계로 토글.
    /// `stats` 의 max errors 를 1.0 으로 정규화. errors==0 셀은 색 없음.
    pub fn set_heatmap(&self, stats: HashMap<(u8, u8), KeyStat>) {
        let max_errors = stats.values().map(|s| s.errors).max().unwrap_or(0).max(1);
        let map = self.by_cell.borrow();
        for (cell_key, frame) in map.iter() {
            for level in HEAT_LEVELS.iter() {
                frame.remove_css_class(level);
            }
            if let Some(stat) = stats.get(cell_key) {
                if stat.errors == 0 {
                    continue;
                }
                let ratio = (stat.errors as f64) / (max_errors as f64);
                let idx = ((ratio * HEAT_LEVELS.len() as f64).ceil() as usize)
                    .clamp(1, HEAT_LEVELS.len());
                frame.add_css_class(HEAT_LEVELS[idx - 1]);
            }
        }
    }

    /// 모든 히트맵 클래스 제거.
    #[allow(dead_code)] // 세션 재시작 등 외부 리셋 진입점.
    pub fn clear_heatmap(&self) {
        let map = self.by_cell.borrow();
        for frame in map.values() {
            for level in HEAT_LEVELS.iter() {
                frame.remove_css_class(level);
            }
        }
    }
}

fn build_key_cell(
    key: &KeyDef,
    ko_profile: Option<&LayoutProfile>,
    en_profile: Option<&LayoutProfile>,
    w: i32,
    h: i32,
) -> gtk::Frame {
    let frame = gtk::Frame::builder()
        .width_request(w)
        .height_request(h)
        .css_classes(["kbv-key"])
        .build();

    // 특수키 — 가운데 라벨.
    if let Some(name) = key.special_name {
        let lab = gtk::Label::builder()
            .label(name)
            .css_classes(["kbv-special-label"])
            .single_line_mode(true)
            .halign(gtk::Align::Center)
            .valign(gtk::Align::Center)
            .build();
        frame.set_child(Some(&lab));
        return frame;
    }

    // 4-corner 라벨 그리드 — 2×2.
    let grid = gtk::Grid::builder()
        .row_homogeneous(true)
        .column_homogeneous(true)
        .hexpand(true)
        .vexpand(true)
        .margin_top(1)
        .margin_bottom(1)
        .margin_start(3)
        .margin_end(3)
        .build();

    // 영문 — en_profile 의 QWERTY 위치 동적 lookup, 없으면 KeyDef 정적 라벨 폴백.
    let (eng_lower_text, eng_upper_text) = match (en_profile, key.ascii) {
        (Some(p), Some(byte)) => match qwerty_position(byte) {
            Some((r, c)) => (
                cell_label_at(&p.layout.lower, r, c).to_string(),
                cell_label_at(&p.layout.upper, r, c).to_string(),
            ),
            None => (key.eng_lower.to_string(), key.eng_upper.to_string()),
        },
        _ => (key.eng_lower.to_string(), key.eng_upper.to_string()),
    };
    grid.attach(
        &corner(&eng_upper_text, "kbv-eng-upper", gtk::Align::Start, gtk::Align::Start),
        0,
        0,
        1,
        1,
    );
    grid.attach(
        &corner(&eng_lower_text, "kbv-eng-lower", gtk::Align::Start, gtk::Align::End),
        0,
        1,
        1,
        1,
    );

    // 한글 — 우상 upper / 우하 lower. ko_profile 있을 때만.
    let (han_lower, han_upper) = match (ko_profile, key.ascii) {
        (Some(p), Some(byte)) => match qwerty_position(byte) {
            Some((r, c)) => (
                cell_label_at(&p.layout.lower, r, c).to_string(),
                cell_label_at(&p.layout.upper, r, c).to_string(),
            ),
            None => (String::new(), String::new()),
        },
        _ => (String::new(), String::new()),
    };
    grid.attach(
        &corner(&han_upper, "kbv-han-upper", gtk::Align::End, gtk::Align::Start),
        1,
        0,
        1,
        1,
    );
    grid.attach(
        &corner(&han_lower, "kbv-han-lower", gtk::Align::End, gtk::Align::End),
        1,
        1,
        1,
        1,
    );

    frame.set_child(Some(&grid));
    frame
}

fn corner(text: &str, css: &'static str, halign: gtk::Align, valign: gtk::Align) -> gtk::Label {
    gtk::Label::builder()
        .label(text)
        .css_classes([css])
        .halign(halign)
        .valign(valign)
        .single_line_mode(true)
        .build()
}

/// 공유 키보드 CSS — 각 앱이 `apply_css` 단계에서 한 번 주입.
///
/// 라벨 위계: 메인(Base) 11pt 700, 보조(Shift) 9pt 500/opacity 0.55.
/// 히트맵 5단계(옅은 노랑→진한 빨강), 클릭 모드(hover·selected)는 studio에서만 노출.
pub const KEYBOARD_CSS: &str = r#"
.kbv-key {
    background: @card_bg_color;
    border: 1px solid alpha(@borders, 0.8);
    border-bottom: 2px solid alpha(@borders, 1.0);
    border-radius: 7px;
    padding: 0;
    box-shadow: inset 0 1px 0 alpha(white, 0.6),
                0 1px 0 alpha(black, 0.04),
                0 2px 3px alpha(black, 0.04);
    transition: all 120ms ease-out;
}
.kbv-key:hover {
    border-color: alpha(@accent_color, 0.5);
}
.kbv-key.kbv-clickable:hover {
    border-color: alpha(@accent_color, 0.6);
    background: alpha(@accent_bg_color, 0.12);
}
.kbv-key.kbv-selected {
    background: alpha(@accent_bg_color, 0.30);
    border: 2px solid @accent_color;
}
.kbv-key.kbv-pressed {
    background: @accent_bg_color;
    border-color: @accent_color;
    box-shadow: inset 0 1px 2px alpha(black, 0.15);
}
.kbv-key.kbv-pressed label {
    color: @accent_fg_color;
}
.kbv-eng-lower {
    font-family: "JetBrains Mono", "D2Coding", monospace;
    font-size: 11pt;
    font-weight: 700;
}
.kbv-eng-upper {
    font-family: "JetBrains Mono", "D2Coding", monospace;
    font-size: 9pt;
    font-weight: 500;
    opacity: 0.55;
}
.kbv-han-lower {
    font-size: 11pt;
    font-weight: 700;
    color: @accent_color;
}
.kbv-han-upper {
    font-size: 9pt;
    font-weight: 500;
    color: @accent_color;
    opacity: 0.55;
}
.kbv-special-label {
    font-size: 9pt;
    font-weight: 600;
    opacity: 0.7;
}
.kbv-key.kbv-heat-1 { background: alpha(#facc15, 0.20); border-color: alpha(#facc15, 0.5); }
.kbv-key.kbv-heat-2 { background: alpha(#fb923c, 0.28); border-color: alpha(#fb923c, 0.55); }
.kbv-key.kbv-heat-3 { background: alpha(#f97316, 0.36); border-color: alpha(#f97316, 0.65); }
.kbv-key.kbv-heat-4 { background: alpha(#ef4444, 0.42); border-color: alpha(#ef4444, 0.7); }
.kbv-key.kbv-heat-5 { background: alpha(#dc2626, 0.55); border-color: alpha(#dc2626, 0.85); }
"#;
