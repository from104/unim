//! popup IPC wire 타입 — docs/dev/windows/popup-renderer-design.md §3 동결.
//! unim-tsf/src/popup_ipc.rs 와 unim-popup-win/src/protocol.rs 양쪽에 동일 정의.
//! 필드 추가는 반드시 #[serde(default)] (하위호환) + 설계서 갱신 + v 유지.
use serde::{Deserialize, Serialize};

pub const WIRE_VERSION: u32 = 1;
pub const PIPE_BASE_NAME: &str = r"\\.\pipe\unim-popup-win"; // + "." + session_id
pub const PIPE_SDDL: &str = "D:(A;;GRGW;;;WD)(A;;GRGW;;;AC)S:(ML;;NW;;;LW)";
pub const MAX_LINE_BYTES: usize = 1024 * 1024;
pub const FLASH_MS: u32 = 140;

pub mod cell_flags {
    pub const HAS_DATA: u32 = 0x01;
    pub const SELECTED: u32 = 0x02;
    pub const COL_HIGHLIGHT: u32 = 0x04;
    pub const ROW_HIGHLIGHT: u32 = 0x08;
    pub const BOOKMARKED: u32 = 0x10;
}

/// 역방향(렌더러→TSF) 이벤트 서브타입 상수 — 설계서 §11.D 동결. 양 크레이트 동일.
pub mod evt_kind {
    pub const CELL_CLICK: &str = "cell_click";
    pub const PAGE_CLICK: &str = "page_click";
    pub const TAB_CLICK: &str = "tab_click";
    pub const EXPAND_TOGGLE: &str = "expand_toggle";
    pub const OUTSIDE_CANCEL: &str = "outside_cancel";
    pub const PAGE_DIR_PREV: u32 = 0;
    pub const PAGE_DIR_NEXT: u32 = 1;
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WireCell {
    pub t: String, // text
    pub m: String, // meaning (한자 외 "")
    pub f: u32,    // cell_flags 비트합
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RenderState {
    pub kind: u32,
    pub target: String,
    pub header_text: String,
    pub footer_text: String,
    pub show_footer: bool,
    pub rows: u32,
    pub cols: u32,
    pub sel_row: u32,
    pub sel_col: u32,
    pub current_page: u32,
    pub total_pages: u32,
    /// column-major: cells[col * rows + row], len == rows*cols
    pub cells: Vec<WireCell>,
    pub col_headers: Vec<(String, bool)>,
    pub row_headers: Vec<(String, bool)>,
    pub expand_visible: bool,
    pub expand_text: String,
    pub tab_labels: Vec<String>,
    pub active_tab_index: u32,
    /// TIP 가 채우는 캐럿(조합 range) 스크린 rect (left, top, right, bottom, 물리 픽셀).
    /// 렌더러가 이 rect 하단에 팝업을 앵커링해 Windows 돋보기 확대 뷰포트를 따라간다.
    /// `None`(GetTextExt 실패·미지원 앱)이면 렌더러가 모니터 중앙으로 폴백.
    /// `skip_serializing_if=Option::is_none` → None 일 때 정방향 골든 라인 바이트 불변.
    /// 필드 추가 시 반드시 unim-tsf/src/popup_ipc.rs 사본과 동일하게 유지.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub caret_rect: Option<(i32, i32, i32, i32)>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WireMsg {
    pub v: u32,
    /// 정방향 "render" | "hide" | "ping" | "shutdown" / 역방향 "evt"
    pub cmd: String,
    pub pid: u32,
    pub seq: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub first: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub flash: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub owner_hwnd: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub render: Option<RenderState>,
    // ─── 역방향(렌더러→TSF) 필드 — 설계서 §11.D 동결. 전부 옵셔널이라 정방향 무영향
    //     (skip_serializing_if=Option::is_none → 정방향 골든 라인 바이트 불변). ───
    /// "cell_click"|"page_click"|"tab_click"|"expand_toggle"|"outside_cancel"
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub evt: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub row: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub col: Option<u32>,
    /// 0=Prev, 1=Next
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dir: Option<u32>,
    /// tab index
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub index: Option<u32>,
}

/// 렌더러가 히트테스트로 산출한 역이벤트(렌더러→TSF). `WireMsg` 직렬화 직전 형태.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RevEvent {
    CellClick { row: u32, col: u32 },
    PageClick { dir: u32 },
    TabClick { index: u32 },
    ExpandToggle,
    OutsideCancel,
}

impl RevEvent {
    /// 역방향 envelope `WireMsg` 로 변환. pid=0(역방향은 식별을 owner_hwnd+seq 로),
    /// seq=마지막 수신 render 의 seq echo, owner_hwnd=대상 호스트 HWND.
    pub fn to_wire(&self, owner_hwnd: u64, seq: u64) -> WireMsg {
        let mut m = WireMsg {
            v: WIRE_VERSION,
            cmd: "evt".to_string(),
            pid: 0,
            seq,
            first: None,
            flash: None,
            owner_hwnd: Some(owner_hwnd),
            render: None,
            evt: None,
            row: None,
            col: None,
            dir: None,
            index: None,
        };
        match *self {
            RevEvent::CellClick { row, col } => {
                m.evt = Some(evt_kind::CELL_CLICK.to_string());
                m.row = Some(row);
                m.col = Some(col);
            }
            RevEvent::PageClick { dir } => {
                m.evt = Some(evt_kind::PAGE_CLICK.to_string());
                m.dir = Some(dir);
            }
            RevEvent::TabClick { index } => {
                m.evt = Some(evt_kind::TAB_CLICK.to_string());
                m.index = Some(index);
            }
            RevEvent::ExpandToggle => {
                m.evt = Some(evt_kind::EXPAND_TOGGLE.to_string());
            }
            RevEvent::OutsideCancel => {
                m.evt = Some(evt_kind::OUTSIDE_CANCEL.to_string());
            }
        }
        m
    }
}

impl RenderState {
    /// column-major 접근의 단일 진실 — 설계서 §5.4 "cells[col * rows + row] 만 사용".
    /// 범위 밖이면 None. row-major 환산(i/cols, i%cols)을 코드 어디에서도 쓰지 말 것.
    #[inline]
    pub fn cell_at(&self, row: u32, col: u32) -> Option<&WireCell> {
        if row >= self.rows || col >= self.cols {
            return None;
        }
        let idx = (col * self.rows + row) as usize;
        self.cells.get(idx)
    }

    /// 레이아웃 모드 판정 (동결): col_headers 가 비면 한자 compact 리스트.
    #[inline]
    pub fn is_compact(&self) -> bool {
        self.col_headers.is_empty()
    }
}

// ─── §10.1 자동 검증 (직렬화 골든 라인 + column-major 교차 검증) ────────────
#[cfg(test)]
mod tests {
    use super::*;

    /// 설계서 §3.2 예시 JSON 의 골든 라인. 동일 문자열을 unim-tsf 측 테스트에도
    /// 박아 사본 드리프트를 테스트 타임에 검출한다.
    const GOLDEN_RENDER_LINE: &str = r#"{"v":1,"cmd":"render","pid":4242,"seq":7,"first":true,"flash":false,"owner_hwnd":123456,"render":{"kind":1,"target":"ㄱ","header_text":"「ㄱ」 → 특수문자","footer_text":"1/3","show_footer":true,"rows":9,"cols":3,"sel_row":0,"sel_col":0,"current_page":0,"total_pages":3,"cells":[{"t":"S0","m":"","f":3}],"col_headers":[["Q",false],["W",false]],"row_headers":[["1.",true],["2.",false]],"expand_visible":false,"expand_text":"","tab_labels":[],"active_tab_index":0}}"#;

    fn golden_msg() -> WireMsg {
        WireMsg {
            v: 1,
            cmd: "render".into(),
            pid: 4242,
            seq: 7,
            first: Some(true),
            flash: Some(false),
            owner_hwnd: Some(123456),
            render: Some(RenderState {
                kind: 1,
                target: "ㄱ".into(),
                header_text: "「ㄱ」 → 특수문자".into(),
                footer_text: "1/3".into(),
                show_footer: true,
                rows: 9,
                cols: 3,
                sel_row: 0,
                sel_col: 0,
                current_page: 0,
                total_pages: 3,
                cells: vec![WireCell {
                    t: "S0".into(),
                    m: "".into(),
                    f: cell_flags::HAS_DATA | cell_flags::SELECTED,
                }],
                col_headers: vec![("Q".into(), false), ("W".into(), false)],
                row_headers: vec![("1.".into(), true), ("2.".into(), false)],
                expand_visible: false,
                expand_text: "".into(),
                tab_labels: vec![],
                active_tab_index: 0,
                caret_rect: None,
            }),
            evt: None,
            row: None,
            col: None,
            dir: None,
            index: None,
        }
    }

    #[test]
    fn golden_serialize_is_byte_equal() {
        let line = serde_json::to_string(&golden_msg()).unwrap();
        assert_eq!(line, GOLDEN_RENDER_LINE, "wire serialization drifted");
        // 라인에 개행이 없어야 한다 (한 줄 = 한 메시지 불변식).
        assert!(!line.contains('\n'));
    }

    #[test]
    fn golden_roundtrip() {
        let parsed: WireMsg = serde_json::from_str(GOLDEN_RENDER_LINE).unwrap();
        let reser = serde_json::to_string(&parsed).unwrap();
        assert_eq!(reser, GOLDEN_RENDER_LINE);
    }

    #[test]
    fn unknown_fields_ignored() {
        // 알 수 없는 필드는 무시되어야 한다 (하위호환 — §3.3 파싱 규약).
        let line = r#"{"v":1,"cmd":"hide","pid":1,"seq":2,"extra_field":99}"#;
        let parsed: WireMsg = serde_json::from_str(line).unwrap();
        assert_eq!(parsed.cmd, "hide");
        assert_eq!(parsed.pid, 1);
    }

    /// §10.1: 20개 아이템 rows=9/cols=3 케이스에서 column-major 교차 일치.
    /// cell_at(0,1).t == items[9] (special_global_index 와 동일 규칙).
    #[test]
    fn column_major_cross_check() {
        let rows: u32 = 9;
        let cols: u32 = 3;
        // 20개 아이템: S0..S19, 그 뒤 빈 셀로 27까지 채움 (column-major).
        let total = (rows * cols) as usize;
        let mut cells = Vec::with_capacity(total);
        // column-major 배치: cells[col*rows + row] = item(col*rows + row 의 보이는 순서)
        // popup_layout 의 special_global_index 와 동일하게, 평면 인덱스 == 아이템 번호.
        for idx in 0..total {
            if idx < 20 {
                cells.push(WireCell {
                    t: format!("S{idx}"),
                    m: String::new(),
                    f: cell_flags::HAS_DATA,
                });
            } else {
                cells.push(WireCell {
                    t: String::new(),
                    m: String::new(),
                    f: 0,
                });
            }
        }
        let rs = RenderState {
            kind: 1,
            target: "ㄱ".into(),
            header_text: String::new(),
            footer_text: String::new(),
            show_footer: false,
            rows,
            cols,
            sel_row: 0,
            sel_col: 0,
            current_page: 0,
            total_pages: 1,
            cells,
            col_headers: vec![("Q".into(), true); 9],
            row_headers: vec![("1.".into(), true); 9],
            expand_visible: false,
            expand_text: String::new(),
            tab_labels: vec![],
            active_tab_index: 0,
            caret_rect: None,
        };
        // col=1, row=0 → 평면 인덱스 1*9+0 = 9 → "S9".
        assert_eq!(rs.cell_at(0, 1).unwrap().t, "S9");
        // col=0, row=0 → 0 → "S0".
        assert_eq!(rs.cell_at(0, 0).unwrap().t, "S0");
        // col=2, row=1 → 2*9+1 = 19 → "S19" (마지막 데이터 셀).
        assert_eq!(rs.cell_at(1, 2).unwrap().t, "S19");
        // col=2, row=2 → 20 → 빈 셀.
        assert_eq!(rs.cell_at(2, 2).unwrap().t, "");
        assert_eq!(rs.cell_at(2, 2).unwrap().f, 0);
        // 범위 밖.
        assert!(rs.cell_at(9, 0).is_none());
        assert!(rs.cell_at(0, 3).is_none());
    }

    // ─── §11.D 역방향 골든 라인 5종 (byte-equal 교차 — unim-tsf 측 테스트와 동일 문자열) ───
    // 직렬화 키 순서 = WireMsg 필드 선언 순서:
    //   v,cmd,pid,seq,first,flash,owner_hwnd,render,evt,row,col,dir,index
    // 렌더러는 pid=0, seq=마지막 render seq echo, owner_hwnd=대상 호스트로 채운다.
    const REV_CELL_CLICK: &str = r#"{"v":1,"cmd":"evt","pid":0,"seq":7,"owner_hwnd":123456,"evt":"cell_click","row":2,"col":3}"#;
    const REV_PAGE_CLICK: &str = r#"{"v":1,"cmd":"evt","pid":0,"seq":7,"owner_hwnd":123456,"evt":"page_click","dir":1}"#;
    const REV_TAB_CLICK: &str = r#"{"v":1,"cmd":"evt","pid":0,"seq":7,"owner_hwnd":123456,"evt":"tab_click","index":4}"#;
    const REV_EXPAND_TOGGLE: &str = r#"{"v":1,"cmd":"evt","pid":0,"seq":7,"owner_hwnd":123456,"evt":"expand_toggle"}"#;
    const REV_OUTSIDE_CANCEL: &str = r#"{"v":1,"cmd":"evt","pid":0,"seq":7,"owner_hwnd":123456,"evt":"outside_cancel"}"#;

    #[test]
    fn reverse_golden_lines_byte_equal() {
        let cases: [(RevEvent, &str); 5] = [
            (RevEvent::CellClick { row: 2, col: 3 }, REV_CELL_CLICK),
            (
                RevEvent::PageClick { dir: evt_kind::PAGE_DIR_NEXT },
                REV_PAGE_CLICK,
            ),
            (RevEvent::TabClick { index: 4 }, REV_TAB_CLICK),
            (RevEvent::ExpandToggle, REV_EXPAND_TOGGLE),
            (RevEvent::OutsideCancel, REV_OUTSIDE_CANCEL),
        ];
        for (evt, golden) in cases {
            let line = serde_json::to_string(&evt.to_wire(123456, 7)).unwrap();
            assert_eq!(&line, golden, "reverse wire serialization drifted: {evt:?}");
            assert!(!line.contains('\n'), "reverse line must be single-line");
        }
    }

    /// 역방향 필드 추가가 정방향 골든 라인을 깨지 않음을 한 번 더 못박는다
    /// (skip_serializing_if=Option::is_none → None 필드 직렬화 생략).
    #[test]
    fn forward_golden_unaffected_by_reverse_fields() {
        let line = serde_json::to_string(&golden_msg()).unwrap();
        assert_eq!(line, GOLDEN_RENDER_LINE);
        assert!(!line.contains("\"evt\""));
        assert!(!line.contains("\"row\""));
    }
}
