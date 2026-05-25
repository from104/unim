//! 5행 표준 106키 키보드 모양 위젯.
//!
//! 기존 `unim-keymap-common::KeyboardWidget` (4×N 그리드, studio 편집·히트맵용) 과는
//! 별도 — 본 위젯은 **실제 키보드 모양**(stagger + 5줄) 으로 사용자에게 자판을 보여주고
//! 키 입력 시 시각 피드백을 주는 read-only 표시기.
//!
//! 키 단위 라벨 (좌상·좌하·우상·우하 4종) — 실제 키보드 관습 (위=shifted, 아래=base):
//! - 좌상: 영문 upper (예: `A`, shift 눌렀을 때)
//! - 좌하: 영문 lower (예: `a`, base)
//! - 우상: 한글 upper (예: 두벌식 shift+`q` → `ㅃ`)
//! - 우하: 한글 lower (현재 자판, 예: 두벌식 → `ㅂ`, base)
//!
//! 한글 매핑이 없는 키 (숫자·기호·특수) 는 우측 영역을 비워 둔다.

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use gtk4::glib::translate::{FromGlib, IntoGlib};
use gtk4::prelude::*;
use gtk4::{self as gtk, glib};

use unim::keystroke::profile::LayoutProfile;

use unim_keymap_common::keyboard_widget::{cell_label_at, KeyStat};

use crate::practice_engine::{find_cell_for_ascii, qwerty_position};

/// 키 한 칸의 정의 — 위에서 아래, 왼쪽에서 오른쪽 순서로 배치.
struct KeyDef {
    /// GDK keyval lowercase 기준 — 이 값으로 press/release 매칭.
    keyval: Option<u32>,
    /// 영문 lower 라벨 (좌상). 빈 문자열이면 미표시.
    eng_lower: &'static str,
    /// 영문 upper 라벨 (좌하).
    eng_upper: &'static str,
    /// 키 너비 multiplier (1.0 = 표준 키). Backspace·Tab·Enter·Shift·Space 등은 더 큼.
    width: f32,
    /// 한글 라벨을 조회할 때 사용할 ASCII byte. None 이면 한글 영역 항상 비움.
    /// (한글 매핑은 `find_cell_for_ascii` 로 lower/upper 모두 조회.)
    hangul_ascii: Option<u8>,
    /// 키 라벨 (영문 라벨이 1자가 아닌 특수키 — "Tab", "Esc" 등) — None 이면 eng_lower 표시.
    special_name: Option<&'static str>,
}

const UNIT: f32 = 46.0; // 1u 키 픽셀 크기.
const ROW_HEIGHT: f32 = 50.0;
/// 키 사이 시각 간격(px) — 6차: 2 → 5 로 늘려 경계 명확화.
const KEY_GAP: f32 = 5.0;

/// 5행 키보드 layout — stagger offset (key units) 와 키 정의.
///
/// 데이터 출처: 표준 ANSI 104 + 한/영·한자 (=106).
/// 한/영 키는 우측 Alt 옆, 한자 키는 우측 Ctrl 옆에 배치되는 것이 한국형 키보드의 통상.
fn build_layout() -> Vec<(f32, Vec<KeyDef>)> {
    use gtk::gdk::Key;
    vec![
        // Row 0 (숫자열) — offset 0
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
        // Row 1 (Tab + QWERTY) — offset 0 (Tab 키가 직접 들어옴)
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
                kd(Some(Key::backslash.into_glib()), "\\", "|", 1.5, None, None),
            ],
        ),
        // Row 2 (CapsLock + ASDF + Enter) — offset 0
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
        // Row 3 (LShift + ZXCV + RShift) — offset 0
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
        // Row 4 (Ctrl + Meta + Alt + 한자 + Space + 한/영 + Alt + Menu + Ctrl)
        // — 한국형 106키 배치: 좌 Ctrl-Meta-Alt-한자, Space(4u), 우 한영-Alt-Menu-Ctrl.
        // 5행 전체 폭 = 1.5+1.25+1.5+1.25+4.0+1.25+1.5+1.25+1.5 = 15.0u
        // (1~4행 폭과 정확히 일치 — 우측 공백 없음.)
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

fn kd(
    keyval: Option<u32>,
    eng_lower: &'static str,
    eng_upper: &'static str,
    width: f32,
    hangul_ascii: Option<u8>,
    special_name: Option<&'static str>,
) -> KeyDef {
    KeyDef {
        keyval,
        eng_lower,
        eng_upper,
        width,
        hangul_ascii,
        special_name,
    }
}

/// 키보드 모양 위젯.
pub struct KeyboardView {
    root: gtk::Fixed,
    /// keyval → 키 widget — press/release 시 CSS class 토글에 사용.
    by_keyval: Rc<RefCell<HashMap<u32, gtk::Widget>>>,
    /// (row, col) QWERTY 셀 좌표 → 키 widget — 히트맵 셀 색 칠하기용.
    /// profile 이 주어졌고 한글 매핑이 있는 셀만 등록된다.
    by_cell: Rc<RefCell<HashMap<(u8, u8), gtk::Frame>>>,
}

const HEAT_LEVELS: [&str; 5] = [
    "kbv-heat-1",
    "kbv-heat-2",
    "kbv-heat-3",
    "kbv-heat-4",
    "kbv-heat-5",
];

impl KeyboardView {
    /// 새 위젯 생성.
    /// - `ko_profile`: 한글 라벨 산출 (없으면 한글 영역 빈칸).
    /// - `en_profile`: 영문 라벨 산출 (없으면 KeyDef 의 정적 qwerty 라벨로 폴백).
    pub fn new(
        ko_profile: Option<&LayoutProfile>,
        en_profile: Option<&LayoutProfile>,
    ) -> Rc<Self> {
        let root = gtk::Fixed::builder()
            .halign(gtk::Align::Center)
            .hexpand(false)
            .build();
        // 폭 700 (15u × 46 = 690 + 안전 10).
        // 높이 = 마지막 row 의 bottom 위치(= 4*ROW_HEIGHT + KEY_GAP/2 + 키 height).
        // = 4*50 + 2.5 + 45 = 247.5 → 248 (아래 추가 빈 공간 0).
        root.set_size_request(700, (ROW_HEIGHT * 5.0 - KEY_GAP * 0.5).ceil() as i32);

        let by_keyval: Rc<RefCell<HashMap<u32, gtk::Widget>>> =
            Rc::new(RefCell::new(HashMap::new()));
        let by_cell: Rc<RefCell<HashMap<(u8, u8), gtk::Frame>>> =
            Rc::new(RefCell::new(HashMap::new()));

        let layout = build_layout();
        for (r_idx, (offset, row)) in layout.iter().enumerate() {
            let mut x: f32 = offset * UNIT;
            let y: f32 = r_idx as f32 * ROW_HEIGHT;
            for key in row {
                // 키 사이 간격(KEY_GAP)을 키 box 외부에서 빼고, 절반은 좌우 padding 으로.
                let w = key.width * UNIT - KEY_GAP;
                let h = ROW_HEIGHT - KEY_GAP;
                let cell = build_key_cell(key, ko_profile, en_profile, w as i32, h as i32);
                root.put(&cell, (x + KEY_GAP * 0.5) as f64, (y + KEY_GAP * 0.5) as f64);
                if let Some(kv) = key.keyval {
                    by_keyval.borrow_mut().insert(kv, cell.clone().upcast());
                }
                // 히트맵 매핑 — hangul_ascii 가 있으면 QWERTY 셀로 좌표 산출.
                if let (Some(p), Some(byte)) = (ko_profile, key.hangul_ascii) {
                    if let Some((r, c, _)) = find_cell_for_ascii(p, byte, false) {
                        by_cell.borrow_mut().insert((r, c), cell.clone());
                    }
                }
                x += key.width * UNIT;
            }
        }

        Rc::new(Self {
            root,
            by_keyval,
            by_cell,
        })
    }

    pub fn root(&self) -> &gtk::Fixed {
        &self.root
    }

    /// 키 눌림 시각 피드백 — `pressed` CSS class 추가 후 150ms 뒤 자동 제거.
    pub fn flash_key(self: &Rc<Self>, keyval: u32) {
        // 대문자 keyval 도 들어올 수 있다 — lowercase 정규화 시도.
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

    /// 히트맵 색 오버레이 — QWERTY 셀별 오타 빈도를 5단계 클래스로 토글.
    /// `stats` 의 max errors 를 1.0 으로 정규화하여 각 셀 ratio 산출.
    /// ratio 0 인 셀은 색 없음, 0 < r ≤ 1 은 5단계 (옅음→진함) 중 하나.
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

    // 특수키 (Tab/Caps/Shift/Space/한영/한자/Enter/Backspace 등) — 가운데 큰 라벨.
    if let Some(name) = key.special_name {
        let lab = gtk::Label::builder()
            .label(name)
            .css_classes(["caption", "kbv-special-label"])
            .single_line_mode(true)
            .halign(gtk::Align::Center)
            .valign(gtk::Align::Center)
            .build();
        frame.set_child(Some(&lab));
        return frame;
    }

    // 4-corner 라벨 그리드 — Grid 2×2.
    let grid = gtk::Grid::builder()
        .row_spacing(0)
        .column_spacing(0)
        .row_homogeneous(true)
        .column_homogeneous(true)
        .hexpand(true)
        .vexpand(true)
        .margin_top(2)
        .margin_bottom(2)
        .margin_start(3)
        .margin_end(3)
        .build();

    // 영문 — physical QWERTY 위치(KeyDef.hangul_ascii 의 byte → row/col) 로
    // en_profile 에서 동적 lookup. en_profile 이 없으면 KeyDef 의 정적 qwerty 라벨로 폴백.
    let (eng_lower_text, eng_upper_text) = match (en_profile, key.hangul_ascii) {
        (Some(p), Some(byte)) => match qwerty_position(byte) {
            Some((r, c)) => (
                cell_label_at(&p.layout.lower, r, c).to_string(),
                cell_label_at(&p.layout.upper, r, c).to_string(),
            ),
            None => (key.eng_lower.to_string(), key.eng_upper.to_string()),
        },
        _ => (key.eng_lower.to_string(), key.eng_upper.to_string()),
    };
    let eng_upper = make_corner_label(
        &eng_upper_text,
        "kbv-eng-upper",
        gtk::Align::Start,
        gtk::Align::Start,
    );
    let eng_lower = make_corner_label(
        &eng_lower_text,
        "kbv-eng-lower",
        gtk::Align::Start,
        gtk::Align::End,
    );
    grid.attach(&eng_upper, 0, 0, 1, 1);
    grid.attach(&eng_lower, 0, 1, 1, 1);

    // 한글 — 우상 = upper, 우하 = lower. ko_profile 있을 때만.
    let (hangul_lower, hangul_upper) = match (ko_profile, key.hangul_ascii) {
        (Some(p), Some(byte)) => (
            find_cell_for_ascii(p, byte, false).map(|(_, _, s)| s).unwrap_or_default(),
            find_cell_for_ascii(p, byte, true).map(|(_, _, s)| s).unwrap_or_default(),
        ),
        _ => (String::new(), String::new()),
    };
    let han_upper = make_corner_label(
        &hangul_upper,
        "kbv-han-upper",
        gtk::Align::End,
        gtk::Align::Start,
    );
    let han_lower = make_corner_label(
        &hangul_lower,
        "kbv-han-lower",
        gtk::Align::End,
        gtk::Align::End,
    );
    grid.attach(&han_upper, 1, 0, 1, 1);
    grid.attach(&han_lower, 1, 1, 1, 1);

    frame.set_child(Some(&grid));
    frame
}

fn make_corner_label(
    text: &str,
    css: &'static str,
    halign: gtk::Align,
    valign: gtk::Align,
) -> gtk::Label {
    // `caption` 클래스는 Adwaita 가 자동 opacity 0.55 를 적용하므로 제거 —
    // 4-corner 라벨의 위계는 본 위젯 CSS (`.kbv-eng-lower` 등) 에서 직접 관리.
    gtk::Label::builder()
        .label(text)
        .css_classes([css])
        .halign(halign)
        .valign(valign)
        .single_line_mode(true)
        .build()
}

/// 본 위젯 전용 CSS — 호출자가 `apply_css` 단계에서 한 번 주입.
///
/// DESIGN.md §6.2: 라운드 7, shadowKey(inset + 작은 drop), border-bottom 2px.
/// 라벨 위계: 메인(Base) 11pt 700, 보조(Shift) 9pt 500/opacity 0.6.
pub const KEYBOARD_VIEW_CSS: &str = r#"
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
.kbv-key.kbv-pressed {
    background: @accent_bg_color;
    border-color: @accent_color;
    box-shadow: inset 0 1px 2px alpha(black, 0.15);
}
.kbv-key.kbv-pressed label {
    color: @accent_fg_color;
}
/* 영문 — 메인(좌하, Base)은 진하게 / 보조(좌상, Shift)는 옅게.
 * DESIGN.md §7.8.1 메인 15px/700, 보조 11px/500 fgFaint. */
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
/* 한글 — accent 색, 동일 위계. */
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
/* 히트맵 5단계 — 옅은 노랑(약한 오타) → 진한 빨강(잦은 오타).
 * set_heatmap 호출 시 셀별로 한 클래스만 부여. */
.kbv-key.kbv-heat-1 { background: alpha(#facc15, 0.20); border-color: alpha(#facc15, 0.5); }
.kbv-key.kbv-heat-2 { background: alpha(#fb923c, 0.28); border-color: alpha(#fb923c, 0.55); }
.kbv-key.kbv-heat-3 { background: alpha(#f97316, 0.36); border-color: alpha(#f97316, 0.65); }
.kbv-key.kbv-heat-4 { background: alpha(#ef4444, 0.42); border-color: alpha(#ef4444, 0.7); }
.kbv-key.kbv-heat-5 { background: alpha(#dc2626, 0.55); border-color: alpha(#dc2626, 0.85); }
"#;
