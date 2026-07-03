//! UNIM 설정 다이얼로그 — 순수 Win32 네이티브 구현 (탭 구조)
//!
//! TSF DLL 내부에서 직접 띄우는 모달 설정 창.
//! - SysTabControl32 탭: [일반] [오타 교정] [억제 단어] [사용자 사전]
//! - 자체 WNDCLASS 등록 (OnceLock)
//! - EnableWindow(parent, false) + 자체 메시지 루프로 모달 격리
//! - 슬라이더(msctls_trackbar32), 체크박스(BS_AUTOCHECKBOX), 콤보박스(CBS_DROPDOWNLIST), EDIT
//! - OK/적용: 모든 탭 컨트롤 수집 후 save_to_default_path()
//! - 취소/WM_CLOSE: 미저장 닫기
//! - 패닉 금지, save 실패는 MessageBox
//! - 억제 단어/사용자 사전: config.yaml 와 독립된 별도 yaml, 변경 즉시 저장

#![allow(non_snake_case)]

use std::sync::OnceLock;

use windows::core::*;
use windows::Win32::Foundation::*;
use windows::Win32::Graphics::Gdi::{GetStockObject, HBRUSH, WHITE_BRUSH};
use windows::Win32::UI::Controls::*;
use windows::Win32::UI::Input::KeyboardAndMouse::EnableWindow;
use windows::Win32::UI::WindowsAndMessaging::*;

use unim::config::{
    CommitUnit, Config, InputCategory, ModeSharingMode,
    ENGLISH_LAYOUT_BUILTINS, KOREAN_LAYOUT_BUILTINS,
    AUTO_TYPEFIX_ENG_MIN_LENGTH_MAX, AUTO_TYPEFIX_ENG_MIN_LENGTH_MIN,
    AUTO_TYPEFIX_KOR_THRESHOLD_MAX, AUTO_TYPEFIX_KOR_THRESHOLD_MIN,
    AUTO_TYPEFIX_OBSERVATION_TIMEOUT_MAX, AUTO_TYPEFIX_OBSERVATION_TIMEOUT_MIN,
    AUTO_TYPEFIX_TENTATIVE_EXPIRY_MAX, AUTO_TYPEFIX_TENTATIVE_EXPIRY_MIN,
    AUTO_TYPEFIX_TIME_WINDOW_MAX, AUTO_TYPEFIX_TIME_WINDOW_MIN,
};
use unim::keystroke::profile::load_builtin_profile;
use unim::typefix_blacklist::{Blacklist, EntryStatus};
use unim::typefix_userdict::UserDictionary;

// ─── Win32 상수 (windows 0.58 에 없거나 타입 차이) ───────────────────────────

/// STATIC 컨트롤 스타일: 텍스트 중앙 정렬
const SS_CENTER_VALUE: u32 = 1u32;
/// Trackbar: 현재 위치 얻기 (TBM_GETPOS = WM_USER + 0 = 1024)
const TBM_GETPOS: u32 = 1024u32;
/// TCIF_TEXT 마스크 값
const TCIF_TEXT_VALUE: u32 = 1u32;
/// CBS_DROPDOWNLIST 숫자값
const CBS_DROPDOWNLIST_VALUE: u32 = 3u32;
/// ComboBox 알림: 선택 변경
const CBN_SELCHANGE: u32 = 1u32;
/// GroupBox 스타일 (BS_GROUPBOX)
const BS_GROUPBOX_VALUE: u32 = 7u32;
/// ES_AUTOHSCROLL
const ES_AUTOHSCROLL_VALUE: u32 = 128u32;

// ─── 탭 인덱스 ───────────────────────────────────────────────────────────────

const TAB_GENERAL: i32 = 0;
const TAB_TYPEFIX: i32 = 1;
const TAB_BLACKLIST: i32 = 2;
const TAB_USERDICT: i32 = 3;

// ─── 컨트롤 ID 상수 ─────────────────────────────────────────────────────────
// 일반 탭 (1xxx, 4xxx, 5xxx)

const ID_CMB_KOR_LAYOUT: u32 = 4001;
const ID_CMB_ENG_LAYOUT: u32 = 4002;
const ID_CMB_DEFAULT_CAT: u32 = 4003;
const ID_CMB_MODE_SHARING: u32 = 4004;
const ID_CMB_COMMIT_UNIT: u32 = 4005;

const ID_CHK_BIDIR_COMBINE: u32 = 5001;
const ID_TRK_CHORD_MS: u32 = 5002;
const ID_LBL_CHORD_MS: u32 = 5003;

const ID_EDT_TOGGLE_KEYS: u32 = 5010;
const ID_EDT_HANJA_KEYS: u32 = 5011;
const ID_EDT_WORD_MODE_APPS: u32 = 5012;

const ID_CHK_AUTO_ENG: u32 = 5020;
const ID_EDT_AUTO_ENG_TRIGGERS: u32 = 5021;

// 오타 교정 탭 (1xxx, 2xxx, 3xxx)

const ID_CHK_ATF_ENABLED: u32 = 1001;
const ID_CHK_ATF_FORWARD: u32 = 1002;
const ID_CHK_ATF_REVERSE: u32 = 1003;
const ID_CHK_ATF_SKIP_ENG_WORD: u32 = 1004;
const ID_CHK_ATF_SKIP_COMPLETE: u32 = 1005;
const ID_CHK_ATF_ROLLBACK: u32 = 1006;
const ID_CHK_ATF_USER_DICT: u32 = 1007;

const ID_TRK_KOR_THRESHOLD: u32 = 2001;
const ID_TRK_ENG_MIN_LEN: u32 = 2002;
const ID_TRK_FWD_TIME: u32 = 2003;
const ID_TRK_REV_TIME: u32 = 2004;
const ID_TRK_TENTATIVE: u32 = 2005;
const ID_TRK_OBS_TIMEOUT: u32 = 2006;

const ID_LBL_KOR_THRESHOLD: u32 = 3001;
const ID_LBL_ENG_MIN_LEN: u32 = 3002;
const ID_LBL_FWD_TIME: u32 = 3003;
const ID_LBL_REV_TIME: u32 = 3004;
const ID_LBL_TENTATIVE: u32 = 3005;
const ID_LBL_OBS_TIMEOUT: u32 = 3006;

// 공통 버튼

const ID_BTN_SET_DEFAULT: u32 = 6001;
const ID_BTN_OK: u32 = 7001;
const ID_BTN_CANCEL: u32 = 7002;
const ID_BTN_APPLY: u32 = 7003;

// 탭 컨트롤 자체

const ID_TABCTRL: u32 = 8001;

// 억제 단어 탭 (9xxx)

const ID_BL_LIST: u32 = 9001;
const ID_BL_BTN_DELETE: u32 = 9002;
const ID_BL_BTN_DEACTIVATE: u32 = 9003;
const ID_BL_BTN_CONFIRM: u32 = 9004;

// 사용자 사전 탭 (9xxx 계속)

const ID_UD_LIST: u32 = 9101;
const ID_UD_EDT_WORD: u32 = 9102;
const ID_UD_EDT_NOTE: u32 = 9103;
const ID_UD_BTN_ADD: u32 = 9104;
const ID_UD_BTN_UPDATE: u32 = 9105;
const ID_UD_BTN_DELETE: u32 = 9106;

// ─── 다이얼로그 크기 ─────────────────────────────────────────────────────────

const DLG_W: i32 = 500;
const DLG_H: i32 = 760;

/// 탭 컨트롤 영역 (상단)
const TAB_X: i32 = 8;
const TAB_Y: i32 = 8;
const TAB_W: i32 = DLG_W - 16;
/// 탭 컨트롤 전체 높이 (탭 헤더 + 페이지 영역)
const TAB_H: i32 = DLG_H - 60;

/// 탭 헤더 높이 (근사값 — 실제로는 TCM_ADJUSTRECT 로 얻지만, 단순 레이아웃에서 고정값 사용)
const TAB_HDR_H: i32 = 26;

/// 페이지 컨테이너 좌표 (탭 컨트롤 client 기준)
const PAGE_X: i32 = TAB_X + 4;
const PAGE_Y: i32 = TAB_Y + TAB_HDR_H + 2;
const PAGE_W: i32 = TAB_W - 8;
const PAGE_H: i32 = TAB_H - TAB_HDR_H - 6;

// ─── 윈도우 클래스 등록 ──────────────────────────────────────────────────────

const WND_CLASS_NAME: PCWSTR = w!("UNIM_SettingsDlg");
const PAGE_CLASS_NAME: PCWSTR = w!("UNIM_TabPage");

static WND_CLASS_ATOM: OnceLock<u16> = OnceLock::new();
static PAGE_CLASS_ATOM: OnceLock<u16> = OnceLock::new();

unsafe fn ensure_wnd_class() {
    WND_CLASS_ATOM.get_or_init(|| {
        let wc = WNDCLASSEXW {
            cbSize: std::mem::size_of::<WNDCLASSEXW>() as u32,
            style: CS_HREDRAW | CS_VREDRAW,
            lpfnWndProc: Some(settings_wnd_proc),
            cbClsExtra: 0,
            cbWndExtra: std::mem::size_of::<usize>() as i32,
            hInstance: crate::dll_instance().into(),
            hIcon: HICON::default(),
            hCursor: LoadCursorW(None, IDC_ARROW).unwrap_or_default(),
            hbrBackground: HBRUSH(GetStockObject(WHITE_BRUSH).0),
            lpszMenuName: PCWSTR::null(),
            lpszClassName: WND_CLASS_NAME,
            hIconSm: HICON::default(),
        };
        RegisterClassExW(&wc)
    });
    PAGE_CLASS_ATOM.get_or_init(|| {
        let wc = WNDCLASSEXW {
            cbSize: std::mem::size_of::<WNDCLASSEXW>() as u32,
            style: CS_HREDRAW | CS_VREDRAW,
            lpfnWndProc: Some(page_wnd_proc),
            cbClsExtra: 0,
            cbWndExtra: 0,
            hInstance: crate::dll_instance().into(),
            hIcon: HICON::default(),
            hCursor: LoadCursorW(None, IDC_ARROW).unwrap_or_default(),
            hbrBackground: HBRUSH(GetStockObject(WHITE_BRUSH).0),
            lpszMenuName: PCWSTR::null(),
            lpszClassName: PAGE_CLASS_NAME,
            hIconSm: HICON::default(),
        };
        RegisterClassExW(&wc)
    });
}

// ─── 페이지 WndProc (WM_HSCROLL 부모 전달) ───────────────────────────────────

unsafe extern "system" fn page_wnd_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    match msg {
        WM_HSCROLL => {
            // 트랙바 스크롤 알림을 메인 다이얼로그로 전달
            if let Ok(parent) = GetParent(hwnd) {
                if let Ok(dlg) = GetParent(parent) {
                    SendMessageW(dlg, WM_HSCROLL, Some(wparam), Some(lparam));
                } else {
                    SendMessageW(parent, WM_HSCROLL, Some(wparam), Some(lparam));
                }
            }
            LRESULT(0)
        }
        WM_COMMAND => {
            // 콤보/에딧 알림을 메인 다이얼로그로 전달
            if let Ok(parent) = GetParent(hwnd) {
                if let Ok(dlg) = GetParent(parent) {
                    SendMessageW(dlg, WM_COMMAND, Some(wparam), Some(lparam));
                } else {
                    SendMessageW(parent, WM_COMMAND, Some(wparam), Some(lparam));
                }
            }
            LRESULT(0)
        }
        _ => DefWindowProcW(hwnd, msg, wparam, lparam),
    }
}

// ─── 다이얼로그 상태 ─────────────────────────────────────────────────────────

/// 동적 룰셋 체크박스 항목
struct RuleSetEntry {
    /// 룰셋 이름 (config key)
    name: String,
    hwnd: HWND,
}

/// GWLP_USERDATA 로 HWND 에 붙어 다니는 상태.
struct DlgState {
    config: Config,
    parent: HWND,
    /// 탭 컨트롤 HWND
    hwnd_tab: HWND,
    /// 탭1 "일반" 페이지 컨테이너
    hwnd_page_general: HWND,
    /// 탭2 "오타 교정" 페이지 컨테이너
    hwnd_page_typefix: HWND,
    /// 탭3 "억제 단어" 페이지 컨테이너
    hwnd_page_blacklist: HWND,
    /// 탭4 "사용자 사전" 페이지 컨테이너
    hwnd_page_userdict: HWND,
    /// 현재 선택된 탭 인덱스
    cur_tab: i32,
    /// 동적 룰셋 체크박스 목록 (일반 탭)
    rule_set_entries: Vec<RuleSetEntry>,
    /// 룰셋 체크박스가 시작되는 y 오프셋 (재구성 시 기존 항목 삭제용)
    /// 실제로는 Vec 의 HWND 를 DestroyWindow 로 제거
    chord_slider_y: i32,  // 모아치기 슬라이더 y 위치 저장 (룰셋 재구성 후 위치 계산용)
    /// 억제 단어 데이터 (별도 yaml, config 와 독립)
    blacklist: Blacklist,
    /// 사용자 사전 데이터 (별도 yaml, config 와 독립)
    userdict: UserDictionary,
}

// ─── 헬퍼: 와이드 문자열 ─────────────────────────────────────────────────────

fn to_wide_nul(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

fn to_wide_mut(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

// ─── 헬퍼: 컨트롤 생성 ──────────────────────────────────────────────────────

unsafe fn create_static(
    parent: HWND,
    text: &str,
    x: i32, y: i32, w: i32, h: i32,
) -> HWND {
    let text_w = to_wide_nul(text);
    CreateWindowExW(
        WINDOW_EX_STYLE::default(),
        w!("STATIC"),
        PCWSTR(text_w.as_ptr()),
        WS_CHILD | WS_VISIBLE,
        x, y, w, h,
        Some(parent),
        None,
        Some(crate::dll_instance().into()),
        None,
    ).unwrap_or_default()
}

unsafe fn create_groupbox(
    parent: HWND,
    text: &str,
    x: i32, y: i32, w: i32, h: i32,
) -> HWND {
    let text_w = to_wide_nul(text);
    CreateWindowExW(
        WINDOW_EX_STYLE::default(),
        w!("BUTTON"),
        PCWSTR(text_w.as_ptr()),
        WS_CHILD | WS_VISIBLE | WINDOW_STYLE(BS_GROUPBOX_VALUE),
        x, y, w, h,
        Some(parent),
        None,
        Some(crate::dll_instance().into()),
        None,
    ).unwrap_or_default()
}

unsafe fn create_checkbox(
    parent: HWND,
    text: &str,
    id: u32,
    x: i32, y: i32, w: i32, h: i32,
    checked: bool,
) -> HWND {
    let text_w = to_wide_nul(text);
    let hwnd = CreateWindowExW(
        WINDOW_EX_STYLE::default(),
        w!("BUTTON"),
        PCWSTR(text_w.as_ptr()),
        WS_CHILD | WS_VISIBLE | WS_TABSTOP | WINDOW_STYLE(BS_AUTOCHECKBOX as u32),
        x, y, w, h,
        Some(parent),
        Some(HMENU(id as *mut core::ffi::c_void)),
        Some(crate::dll_instance().into()),
        None,
    ).unwrap_or_default();
    if checked {
        SendMessageW(hwnd, BM_SETCHECK, Some(WPARAM(BST_CHECKED.0 as usize)), Some(LPARAM(0)));
    }
    hwnd
}

unsafe fn create_trackbar(
    parent: HWND,
    id: u32,
    x: i32, y: i32, w: i32, h: i32,
    min: i32, max: i32, pos: i32,
) -> HWND {
    let hwnd = CreateWindowExW(
        WINDOW_EX_STYLE::default(),
        w!("msctls_trackbar32"),
        PCWSTR::null(),
        WS_CHILD | WS_VISIBLE | WS_TABSTOP | WINDOW_STYLE(TBS_HORZ | TBS_AUTOTICKS),
        x, y, w, h,
        Some(parent),
        Some(HMENU(id as *mut core::ffi::c_void)),
        Some(crate::dll_instance().into()),
        None,
    ).unwrap_or_default();
    let range_param = (((max as u32) << 16) | (min as u32 & 0xFFFF)) as isize;
    SendMessageW(hwnd, TBM_SETRANGE, Some(WPARAM(1)), Some(LPARAM(range_param)));
    SendMessageW(hwnd, TBM_SETPOS, Some(WPARAM(1)), Some(LPARAM(pos as isize)));
    hwnd
}

unsafe fn create_value_label(
    parent: HWND,
    id: u32,
    text: &str,
    x: i32, y: i32, w: i32, h: i32,
) -> HWND {
    let text_w = to_wide_nul(text);
    CreateWindowExW(
        WINDOW_EX_STYLE::default(),
        w!("STATIC"),
        PCWSTR(text_w.as_ptr()),
        WS_CHILD | WS_VISIBLE | WINDOW_STYLE(SS_CENTER_VALUE),
        x, y, w, h,
        Some(parent),
        Some(HMENU(id as *mut core::ffi::c_void)),
        Some(crate::dll_instance().into()),
        None,
    ).unwrap_or_default()
}

unsafe fn create_combobox(
    parent: HWND,
    id: u32,
    x: i32, y: i32, w: i32, h: i32,
    items: &[&str],
    selected_idx: usize,
) -> HWND {
    let hwnd = CreateWindowExW(
        WINDOW_EX_STYLE::default(),
        w!("COMBOBOX"),
        PCWSTR::null(),
        WS_CHILD | WS_VISIBLE | WS_TABSTOP | WINDOW_STYLE(CBS_DROPDOWNLIST_VALUE),
        x, y, w, h,
        Some(parent),
        Some(HMENU(id as *mut core::ffi::c_void)),
        Some(crate::dll_instance().into()),
        None,
    ).unwrap_or_default();
    for item in items.iter() {
        let item_w = to_wide_nul(item);
        SendMessageW(hwnd, CB_ADDSTRING, Some(WPARAM(0)), Some(LPARAM(item_w.as_ptr() as isize)));
    }
    SendMessageW(hwnd, CB_SETCURSEL, Some(WPARAM(selected_idx)), Some(LPARAM(0)));
    hwnd
}

unsafe fn create_edit(
    parent: HWND,
    id: u32,
    text: &str,
    x: i32, y: i32, w: i32, h: i32,
) -> HWND {
    let text_w = to_wide_nul(text);
    CreateWindowExW(
        WINDOW_EX_STYLE(WS_EX_CLIENTEDGE.0),
        w!("EDIT"),
        PCWSTR(text_w.as_ptr()),
        WS_CHILD | WS_VISIBLE | WS_TABSTOP | WINDOW_STYLE(ES_AUTOHSCROLL_VALUE),
        x, y, w, h,
        Some(parent),
        Some(HMENU(id as *mut core::ffi::c_void)),
        Some(crate::dll_instance().into()),
        None,
    ).unwrap_or_default()
}

unsafe fn create_button(
    parent: HWND,
    text: &str,
    id: u32,
    x: i32, y: i32, w: i32, h: i32,
) -> HWND {
    let text_w = to_wide_nul(text);
    CreateWindowExW(
        WINDOW_EX_STYLE::default(),
        w!("BUTTON"),
        PCWSTR(text_w.as_ptr()),
        WS_CHILD | WS_VISIBLE | WS_TABSTOP | WINDOW_STYLE(BS_PUSHBUTTON as u32),
        x, y, w, h,
        Some(parent),
        Some(HMENU(id as *mut core::ffi::c_void)),
        Some(crate::dll_instance().into()),
        None,
    ).unwrap_or_default()
}

// ─── 헬퍼: 값 읽기 ──────────────────────────────────────────────────────────

unsafe fn trackbar_get_pos(hwnd: HWND) -> i32 {
    SendMessageW(hwnd, TBM_GETPOS, Some(WPARAM(0)), Some(LPARAM(0))).0 as i32
}

unsafe fn checkbox_checked(hwnd: HWND) -> bool {
    SendMessageW(hwnd, BM_GETCHECK, Some(WPARAM(0)), Some(LPARAM(0))).0 == BST_CHECKED.0 as isize
}

unsafe fn combobox_get_sel(hwnd: HWND) -> usize {
    let r = SendMessageW(hwnd, CB_GETCURSEL, Some(WPARAM(0)), Some(LPARAM(0))).0;
    if r < 0 { 0 } else { r as usize }
}

unsafe fn edit_get_text(hwnd: HWND) -> String {
    let len = GetWindowTextLengthW(hwnd);
    if len <= 0 {
        return String::new();
    }
    let mut buf: Vec<u16> = vec![0u16; (len + 1) as usize];
    GetWindowTextW(hwnd, &mut buf);
    let s = String::from_utf16_lossy(&buf[..len as usize]);
    s
}

/// "a, b, c" → Vec<String> (공백 trim, 빈 항목 제거)
fn parse_key_list(s: &str) -> Vec<String> {
    s.split(',')
        .map(|t| t.trim().to_string())
        .filter(|t| !t.is_empty())
        .collect()
}

// ─── 헬퍼: 라벨 업데이트 ────────────────────────────────────────────────────

unsafe fn update_value_label(hwnd_dlg: HWND, label_id: u32, value: i32) {
    let text = value.to_string();
    let text_w = to_wide_nul(&text);
    // 라벨은 page 컨테이너 자식 — FindDlgItem 대신 재귀 탐색
    if let Some(lbl) = find_child_by_id(hwnd_dlg, label_id) {
        let _ = SetWindowTextW(lbl, PCWSTR(text_w.as_ptr()));
    }
}

/// hwnd 와 모든 자식 창에서 컨트롤 ID 로 HWND 찾기 (1단계 깊이 탐색)
unsafe fn find_child_by_id(hwnd: HWND, id: u32) -> Option<HWND> {
    // 직접 자식에서 찾기
    if let Ok(h) = GetDlgItem(Some(hwnd), id as i32) {
        if !h.is_invalid() {
            return Some(h);
        }
    }
    // 자식 창을 열거하며 재귀
    let mut found: Option<HWND> = None;
    let found_ptr = &mut found as *mut Option<HWND>;
    struct SearchCtx { id: u32, found: *mut Option<HWND> }
    let mut ctx = SearchCtx { id, found: found_ptr };
    let ctx_ptr = &mut ctx as *mut SearchCtx as isize;

    unsafe extern "system" fn enum_cb(child: HWND, lparam: LPARAM) -> BOOL {
        let ctx = &mut *(lparam.0 as *mut SearchCtx);
        let cid = GetDlgCtrlID(child) as u32;
        if cid == ctx.id {
            *ctx.found = Some(child);
            return FALSE; // 중단
        }
        // 한 단계 더 내려가기
        let _ = EnumChildWindows(Some(child), Some(enum_cb), lparam);
        if (*ctx.found).is_some() {
            return FALSE;
        }
        TRUE
    }

    let _ = EnumChildWindows(Some(hwnd), Some(enum_cb), LPARAM(ctx_ptr));
    found
}

// ─── 헬퍼: 부모 복구 ────────────────────────────────────────────────────────

unsafe fn restore_parent(parent: HWND) {
    if !parent.is_invalid() {
        let _ = EnableWindow(parent, true);
    }
}

// ─── 탭 삽입 헬퍼 ───────────────────────────────────────────────────────────

unsafe fn tab_insert(hwnd_tab: HWND, idx: usize, label: &str) {
    let mut label_w = to_wide_mut(label);
    let mut ti = TCITEMW {
        mask: TCITEMHEADERA_MASK(TCIF_TEXT_VALUE),
        pszText: PWSTR(label_w.as_mut_ptr()),
        cchTextMax: label_w.len() as i32,
        ..Default::default()
    };
    SendMessageW(
        hwnd_tab,
        TCM_INSERTITEMW,
        Some(WPARAM(idx)),
        Some(LPARAM(&mut ti as *mut TCITEMW as isize)),
    );
}

// ─── 룰셋 체크박스 동적 재구성 ───────────────────────────────────────────────

/// 현재 일반 탭 페이지에서 룰셋 체크박스를 모두 파괴하고 새로 생성.
///
/// `start_y`: 룰셋 체크박스가 배치될 첫 번째 y 좌표 (페이지 내부 좌표).
/// 반환값: 마지막으로 사용된 y + row_h + gap (다음 컨트롤 y).
unsafe fn rebuild_rule_sets(
    page: HWND,
    state: &mut DlgState,
    layout_name: &str,
    start_y: i32,
) -> i32 {
    let lm = 12i32;
    let row_h = 22i32;
    let gap = 4i32;
    let chk_w = PAGE_W - lm * 2 - 20;

    // 기존 체크박스 파괴
    for entry in state.rule_set_entries.drain(..) {
        let _ = DestroyWindow(entry.hwnd);
    }

    // 프로필 로드 시도
    let profile = match load_builtin_profile(layout_name) {
        Ok(p) => p,
        Err(_) => return start_y,
    };

    if profile.rule_sets.is_empty() {
        return start_y;
    }

    let config_active = state.config.engine.korean.active_rule_sets.clone();
    let profile_default: Option<&Vec<String>> = profile.active_rule_sets.as_ref();

    let mut y = start_y;
    for (set_name, rs) in &profile.rule_sets {
        let is_active = match &config_active {
            Some(list) => list.contains(set_name),
            None => match profile_default {
                Some(list) => list.contains(set_name),
                None => rs.active,
            },
        };
        let label = rs
            .description
            .as_ref()
            .map(|d| format!("{} ({})", d.resolve("ko"), set_name))
            .unwrap_or_else(|| set_name.clone());

        let hwnd = create_checkbox(page, &label, 0, lm + 20, y, chk_w, row_h, is_active);
        state.rule_set_entries.push(RuleSetEntry {
            name: set_name.clone(),
            hwnd,
        });
        y += row_h + gap;
    }

    y + gap
}

/// 현재 rule_set_entries 체크박스 상태를 config 에 반영
unsafe fn collect_rule_sets(state: &mut DlgState) {
    if state.rule_set_entries.is_empty() {
        // 룰셋 없는 자판은 None 유지
        return;
    }
    let mut active: Vec<String> = Vec::new();
    for entry in &state.rule_set_entries {
        if checkbox_checked(entry.hwnd) {
            active.push(entry.name.clone());
        }
    }
    state.config.engine.korean.active_rule_sets = Some(active);
}

// ─── 탭1 "일반" 페이지 컨트롤 생성 ─────────────────────────────────────────

/// 일반 탭 컨트롤 배치. 룰셋은 `rebuild_rule_sets` 호출로 별도 구성.
/// 반환: (룰셋 시작 y, 룰셋 이후 컨트롤 배치용 state 저장을 위해 필요한 chord_y)
unsafe fn build_page_general(page: HWND, config: &Config) -> i32 {
    let lm = 12i32;
    let mut y = 10i32;
    let row_h = 22i32;
    let gap = 6i32;
    let cmb_w = 180i32;
    let lbl_w = 120i32;
    let full_w = PAGE_W - lm * 2;

    // ── 자판 그룹 ─────────────────────────────────────────────────────────────
    create_groupbox(page, " 자판 ", lm - 4, y - 4, full_w + 8, row_h * 3 + gap * 4, );
    y += 6;

    // 한국어 자판
    create_static(page, "한국어 자판:", lm, y, lbl_w, row_h);
    let kor_sel = KOREAN_LAYOUT_BUILTINS.iter()
        .position(|&s| s == config.engine.korean.layout)
        .unwrap_or(0);
    create_combobox(page, ID_CMB_KOR_LAYOUT, lm + lbl_w + 4, y, cmb_w, 200,
        KOREAN_LAYOUT_BUILTINS, kor_sel);
    y += row_h + gap;

    // 영문 자판
    create_static(page, "영문 자판:", lm, y, lbl_w, row_h);
    let eng_sel = ENGLISH_LAYOUT_BUILTINS.iter()
        .position(|&s| s == config.engine.english.layout)
        .unwrap_or(0);
    create_combobox(page, ID_CMB_ENG_LAYOUT, lm + lbl_w + 4, y, cmb_w, 200,
        ENGLISH_LAYOUT_BUILTINS, eng_sel);
    y += row_h + gap;

    // 한글 확정 단위 (음절/단어/스마트) — 자판 그룹 3번째 행(그룹박스 높이에 이미 예약됨).
    create_static(page, "한글 확정 단위:", lm, y, lbl_w, row_h);
    let cu_items: Vec<&str> = CommitUnit::all().iter().map(|u| u.display_name()).collect();
    let cu_sel = CommitUnit::all().iter()
        .position(|&u| u == config.engine.korean.commit_unit)
        .unwrap_or(0);
    create_combobox(page, ID_CMB_COMMIT_UNIT, lm + lbl_w + 4, y, cmb_w, 200,
        &cu_items, cu_sel);
    y += row_h + gap * 2;

    // ── 규칙 세트 (동적 — 자판 콤보 변경 시 재구성) ───────────────────────────
    // 그룹박스 헤더만 배치하고, 체크박스는 rebuild_rule_sets 에서 채움.
    // 그룹박스 높이는 나중에 알 수 없으므로 헤더 라벨만 배치.
    create_static(page, "[ 규칙 세트 ]", lm, y, 200, row_h);
    y += row_h + 2;

    // 룰셋 체크박스 시작 y 반환 (build_controls_general 의 caller 에서 rebuild_rule_sets 호출)
    y
}

/// 룰셋 이후 나머지 일반 탭 컨트롤 배치.
unsafe fn build_page_general_rest(page: HWND, config: &Config, mut y: i32) {
    let lm = 12i32;
    let row_h = 22i32;
    let gap = 6i32;
    let trk_w = 260i32;
    let trk_h = 26i32;
    let lbl_val_w = 50i32;
    let lbl_w = 150i32;
    let full_w = PAGE_W - lm * 2;
    let lbl_x = lm + trk_w + gap;
    let edit_w = full_w - lbl_w - 8;

    y += gap;

    // ── 모아치기 ────────────────────────────────────────────────────────────
    create_static(page, "[ 모아치기 ]", lm, y, 200, row_h);
    y += row_h + 2;

    let bidir = config.engine.korean.bidirectional_combine.unwrap_or(false);
    create_checkbox(page, "양방향 조합 (모아치기)", ID_CHK_BIDIR_COMBINE,
        lm + 10, y, full_w - 10, row_h, bidir);
    y += row_h + gap;

    // chord_window_ms 슬라이더 (0=OFF, 10~200)
    let chord_val = config.engine.korean.chord_window_ms.unwrap_or(0) as i32;
    let chord_display = if chord_val == 0 { "OFF".to_string() } else { chord_val.to_string() };
    create_static(page, "조합 창 (ms, 0=OFF / 10~200):", lm, y, 230, row_h);
    create_value_label(page, ID_LBL_CHORD_MS, &chord_display, lbl_x, y, lbl_val_w, row_h);
    y += row_h;
    create_trackbar(page, ID_TRK_CHORD_MS, lm, y, trk_w, trk_h, 0, 200, chord_val);
    y += trk_h + gap * 2;

    // ── 입력 모드 ────────────────────────────────────────────────────────────
    create_static(page, "[ 입력 모드 ]", lm, y, 200, row_h);
    y += row_h + 2;

    // 시작 입력 모드
    create_static(page, "시작 입력 모드:", lm, y, lbl_w, row_h);
    let cat_items: &[&str] = &["영문", "한국어"];
    let cat_sel = match config.engine.default_category {
        InputCategory::English => 0,
        InputCategory::Korean => 1,
    };
    create_combobox(page, ID_CMB_DEFAULT_CAT, lm + lbl_w + 4, y, 130, 120,
        cat_items, cat_sel);
    y += row_h + gap;

    // 모드 공유
    create_static(page, "모드 공유:", lm, y, lbl_w, row_h);
    let share_items: &[&str] = &["전역 공유", "앱별 독립"];
    let share_sel = match config.engine.mode_sharing {
        ModeSharingMode::Global => 0,
        ModeSharingMode::PerApp => 1,
    };
    create_combobox(page, ID_CMB_MODE_SHARING, lm + lbl_w + 4, y, 130, 120,
        share_items, share_sel);
    y += row_h + gap * 2;

    // ── 키 설정 ─────────────────────────────────────────────────────────────
    create_static(page, "[ 키 설정 ]", lm, y, 200, row_h);
    y += row_h + 2;

    create_static(page, "한/영 전환키:", lm, y, lbl_w, row_h);
    create_edit(page, ID_EDT_TOGGLE_KEYS,
        &config.engine.toggle_keys.join(", "),
        lm + lbl_w + 4, y, edit_w, row_h);
    y += row_h + gap;

    create_static(page, "한자키:", lm, y, lbl_w, row_h);
    create_edit(page, ID_EDT_HANJA_KEYS,
        &config.engine.hanja_keys.join(", "),
        lm + lbl_w + 4, y, edit_w, row_h);
    y += row_h + gap * 2;

    // ── 자동 영문 전환 ───────────────────────────────────────────────────────
    create_static(page, "[ 자동 영문 전환 ]", lm, y, 200, row_h);
    y += row_h + 2;

    let ae = &config.engine.auto_english;
    create_checkbox(page, "자동 영문 전환 활성화", ID_CHK_AUTO_ENG,
        lm + 10, y, full_w - 10, row_h, ae.enabled);
    y += row_h + gap;

    create_static(page, "트리거 키:", lm, y, lbl_w, row_h);
    create_edit(page, ID_EDT_AUTO_ENG_TRIGGERS,
        &ae.trigger_keys.join(", "),
        lm + lbl_w + 4, y, edit_w, row_h);
    y += row_h + gap * 2;

    // ── 단어 모드 앱 ─────────────────────────────────────────────────────────
    // Smart 확정 단위 게이트가 단어 단위로 조합할 앱(실행 파일명 정확일치, 쉼표 구분).
    // 기본값 winword.exe/wmux.exe. 앱 호환을 코드 릴리스와 분리(config.yaml 핫리로드).
    create_static(page, "[ 단어 모드 ]", lm, y, 200, row_h);
    y += row_h + 2;
    create_static(page, "단어 모드 앱:", lm, y, lbl_w, row_h);
    create_edit(page, ID_EDT_WORD_MODE_APPS,
        &config.engine.korean.word_mode_apps.join(", "),
        lm + lbl_w + 4, y, edit_w, row_h);
    y += row_h + gap * 2;

    // ── 기본 입력기 ──────────────────────────────────────────────────────────
    create_static(page, "[ 기본 입력기 ]", lm, y, 200, row_h);
    y += row_h + 4;
    create_button(page, "기본 입력기로 설정", ID_BTN_SET_DEFAULT, lm, y, 150, 26);
    let _ = y;
}

// ─── 탭2 "오타 교정" 페이지 컨트롤 생성 ────────────────────────────────────

unsafe fn build_page_typefix(page: HWND, config: &Config) {
    let atf = &config.engine.auto_typefix;
    let lm = 12i32;
    let mut y = 10i32;
    let row_h = 22i32;
    let gap = 6i32;
    let trk_w = 300i32;
    let trk_h = 26i32;
    let lbl_val_w = 50i32;
    let chk_w = PAGE_W - lm * 2;
    let lbl_x = lm + trk_w + gap;

    create_static(page, "[ AutoTypeFix — 자동 오타 교정 ]", lm, y, 350, row_h);
    y += row_h + gap;

    create_checkbox(page, "AutoTypeFix 활성화", ID_CHK_ATF_ENABLED,
        lm, y, chk_w, row_h, atf.enabled);
    y += row_h + gap;
    create_checkbox(page, "순방향 교정 (영→한)", ID_CHK_ATF_FORWARD,
        lm + 20, y, chk_w, row_h, atf.forward);
    y += row_h + gap;
    create_checkbox(page, "역방향 교정 (한→영)", ID_CHK_ATF_REVERSE,
        lm + 20, y, chk_w, row_h, atf.reverse);
    y += row_h + gap;
    create_checkbox(page, "영단어 사전 hit 시 순방향 억제", ID_CHK_ATF_SKIP_ENG_WORD,
        lm + 20, y, chk_w, row_h, atf.skip_on_english_word);
    y += row_h + gap;
    create_checkbox(page, "완성 음절만이면 역방향 억제", ID_CHK_ATF_SKIP_COMPLETE,
        lm + 20, y, chk_w, row_h, atf.skip_on_complete_syllable);
    y += row_h + gap;
    create_checkbox(page, "재트리거 학습 억제 (rollback detection)", ID_CHK_ATF_ROLLBACK,
        lm + 20, y, chk_w, row_h, atf.rollback_detection);
    y += row_h + gap;
    create_checkbox(page, "사용자 사전 사용", ID_CHK_ATF_USER_DICT,
        lm + 20, y, chk_w, row_h, atf.user_dict_enabled);
    y += row_h + gap * 2;

    // 슬라이더: kor_syllable_threshold
    create_static(page, "순방향 트리거: 한글 음절 수 (2~6)", lm, y, 280, row_h);
    create_value_label(page, ID_LBL_KOR_THRESHOLD, &atf.kor_syllable_threshold.to_string(), lbl_x, y, lbl_val_w, row_h);
    y += row_h;
    create_trackbar(page, ID_TRK_KOR_THRESHOLD, lm, y, trk_w, trk_h,
        AUTO_TYPEFIX_KOR_THRESHOLD_MIN as i32, AUTO_TYPEFIX_KOR_THRESHOLD_MAX as i32,
        atf.kor_syllable_threshold as i32);
    y += trk_h + gap;

    // 슬라이더: eng_word_min_length
    create_static(page, "역방향 트리거: 영문 단어 최소 길이 (3~8)", lm, y, 280, row_h);
    create_value_label(page, ID_LBL_ENG_MIN_LEN, &atf.eng_word_min_length.to_string(), lbl_x, y, lbl_val_w, row_h);
    y += row_h;
    create_trackbar(page, ID_TRK_ENG_MIN_LEN, lm, y, trk_w, trk_h,
        AUTO_TYPEFIX_ENG_MIN_LENGTH_MIN as i32, AUTO_TYPEFIX_ENG_MIN_LENGTH_MAX as i32,
        atf.eng_word_min_length as i32);
    y += trk_h + gap;

    // 슬라이더: forward_time_window_ms
    create_static(page, "순방향 시간 윈도우 (ms, 500~5000)", lm, y, 280, row_h);
    create_value_label(page, ID_LBL_FWD_TIME, &atf.forward_time_window_ms.to_string(), lbl_x, y, lbl_val_w, row_h);
    y += row_h;
    create_trackbar(page, ID_TRK_FWD_TIME, lm, y, trk_w, trk_h,
        AUTO_TYPEFIX_TIME_WINDOW_MIN as i32, AUTO_TYPEFIX_TIME_WINDOW_MAX as i32,
        atf.forward_time_window_ms as i32);
    y += trk_h + gap;

    // 슬라이더: reverse_time_window_ms
    create_static(page, "역방향 시간 윈도우 (ms, 500~5000)", lm, y, 280, row_h);
    create_value_label(page, ID_LBL_REV_TIME, &atf.reverse_time_window_ms.to_string(), lbl_x, y, lbl_val_w, row_h);
    y += row_h;
    create_trackbar(page, ID_TRK_REV_TIME, lm, y, trk_w, trk_h,
        AUTO_TYPEFIX_TIME_WINDOW_MIN as i32, AUTO_TYPEFIX_TIME_WINDOW_MAX as i32,
        atf.reverse_time_window_ms as i32);
    y += trk_h + gap;

    // 슬라이더: tentative_expiry_hours
    create_static(page, "임시 억제 만료 (시간, 1~12)", lm, y, 280, row_h);
    create_value_label(page, ID_LBL_TENTATIVE, &atf.tentative_expiry_hours.to_string(), lbl_x, y, lbl_val_w, row_h);
    y += row_h;
    create_trackbar(page, ID_TRK_TENTATIVE, lm, y, trk_w, trk_h,
        AUTO_TYPEFIX_TENTATIVE_EXPIRY_MIN as i32, AUTO_TYPEFIX_TENTATIVE_EXPIRY_MAX as i32,
        atf.tentative_expiry_hours as i32);
    y += trk_h + gap;

    // 슬라이더: observation_timeout_secs
    create_static(page, "재트리거 관찰 창 (초, 5~15)", lm, y, 280, row_h);
    create_value_label(page, ID_LBL_OBS_TIMEOUT, &atf.observation_timeout_secs.to_string(), lbl_x, y, lbl_val_w, row_h);
    y += row_h;
    create_trackbar(page, ID_TRK_OBS_TIMEOUT, lm, y, trk_w, trk_h,
        AUTO_TYPEFIX_OBSERVATION_TIMEOUT_MIN as i32, AUTO_TYPEFIX_OBSERVATION_TIMEOUT_MAX as i32,
        atf.observation_timeout_secs as i32);
    let _ = y;
}

// ─── 헬퍼: ListBox 항목 추가 / 전체 리프레시 ────────────────────────────────

unsafe fn listbox_clear(hwnd: HWND) {
    SendMessageW(hwnd, LB_RESETCONTENT, Some(WPARAM(0)), Some(LPARAM(0)));
}

unsafe fn listbox_add(hwnd: HWND, text: &str) {
    let w = to_wide_nul(text);
    SendMessageW(hwnd, LB_ADDSTRING, Some(WPARAM(0)), Some(LPARAM(w.as_ptr() as isize)));
}

unsafe fn listbox_get_sel(hwnd: HWND) -> Option<usize> {
    let r = SendMessageW(hwnd, LB_GETCURSEL, Some(WPARAM(0)), Some(LPARAM(0))).0;
    if r < 0 { None } else { Some(r as usize) }
}

// ─── 억제 단어 탭 리프레시 ──────────────────────────────────────────────────

unsafe fn refresh_blacklist_page(page: HWND, blacklist: &Blacklist) {
    if let Some(lb) = find_child_by_id(page, ID_BL_LIST) {
        listbox_clear(lb);
        for entry in &blacklist.entries {
            let dir = match entry.direction {
                unim::typefix_blacklist::Direction::Forward  => "순방향",
                unim::typefix_blacklist::Direction::Reverse  => "역방향",
            };
            let status = match entry.status {
                EntryStatus::Tentative => "임시",
                EntryStatus::Confirmed => "확정",
                EntryStatus::Inactive  => "비활성",
            };
            let text = format!("[{}] {} ({} / {})", status, entry.ascii, dir, entry.korean_layout);
            listbox_add(lb, &text);
        }
    }
}

// ─── 사용자 사전 탭 리프레시 ────────────────────────────────────────────────

unsafe fn refresh_userdict_page(page: HWND, userdict: &UserDictionary) {
    if let Some(lb) = find_child_by_id(page, ID_UD_LIST) {
        listbox_clear(lb);
        for entry in &userdict.reverse_words {
            let note = entry.note.as_deref().unwrap_or("");
            let text = if note.is_empty() {
                entry.word.clone()
            } else {
                format!("{} — {}", entry.word, note)
            };
            listbox_add(lb, &text);
        }
    }
}

// ─── 탭3 "억제 단어" 페이지 컨트롤 생성 ────────────────────────────────────

unsafe fn build_page_blacklist(page: HWND, blacklist: &Blacklist) {
    let lm = 12i32;
    let mut y = 10i32;
    let row_h = 22i32;
    let gap = 6i32;
    let btn_w = 100i32;
    let btn_h = 26i32;
    let full_w = PAGE_W - lm * 2;
    let list_h = PAGE_H - 120;

    create_static(page, "[ 억제 단어 (AutoTypeFix 자동 교정 제외 목록) ]", lm, y, full_w, row_h);
    y += row_h + gap;

    // ListBox (LBS_NOTIFY | WS_BORDER | WS_VSCROLL)
    CreateWindowExW(
        WINDOW_EX_STYLE(WS_EX_CLIENTEDGE.0),
        w!("LISTBOX"),
        PCWSTR::null(),
        WS_CHILD | WS_VISIBLE | WS_TABSTOP | WS_VSCROLL | WINDOW_STYLE(LBS_NOTIFY as u32),
        lm, y, full_w, list_h,
        Some(page),
        Some(HMENU(ID_BL_LIST as *mut core::ffi::c_void)),
        Some(crate::dll_instance().into()),
        None,
    ).unwrap_or_default();
    y += list_h + gap;

    // 버튼 행
    let btn_y = y;
    create_button(page, "삭제",       ID_BL_BTN_DELETE,     lm,                    btn_y, btn_w, btn_h);
    create_button(page, "비활성화",   ID_BL_BTN_DEACTIVATE, lm + btn_w + gap,      btn_y, btn_w, btn_h);
    create_button(page, "확정(영구)", ID_BL_BTN_CONFIRM,    lm + (btn_w + gap) * 2, btn_y, btn_w + 20, btn_h);

    // 초기 데이터 채우기
    if let Some(lb) = find_child_by_id(page, ID_BL_LIST) {
        listbox_clear(lb);
        for entry in &blacklist.entries {
            let dir = match entry.direction {
                unim::typefix_blacklist::Direction::Forward  => "순방향",
                unim::typefix_blacklist::Direction::Reverse  => "역방향",
            };
            let status = match entry.status {
                EntryStatus::Tentative => "임시",
                EntryStatus::Confirmed => "확정",
                EntryStatus::Inactive  => "비활성",
            };
            let text = format!("[{}] {} ({} / {})", status, entry.ascii, dir, entry.korean_layout);
            listbox_add(lb, &text);
        }
    }
}

// ─── 탭4 "사용자 사전" 페이지 컨트롤 생성 ──────────────────────────────────

unsafe fn build_page_userdict(page: HWND, userdict: &UserDictionary) {
    let lm = 12i32;
    let mut y = 10i32;
    let row_h = 22i32;
    let gap = 6i32;
    let btn_w = 80i32;
    let btn_h = 26i32;
    let full_w = PAGE_W - lm * 2;
    let lbl_w = 60i32;
    let edit_w = full_w - lbl_w - 8;
    let list_h = PAGE_H - 170;

    create_static(page, "[ 사용자 사전 (역방향 교정 허용 단어 목록) ]", lm, y, full_w, row_h);
    y += row_h + gap;

    // ListBox
    CreateWindowExW(
        WINDOW_EX_STYLE(WS_EX_CLIENTEDGE.0),
        w!("LISTBOX"),
        PCWSTR::null(),
        WS_CHILD | WS_VISIBLE | WS_TABSTOP | WS_VSCROLL | WINDOW_STYLE(LBS_NOTIFY as u32),
        lm, y, full_w, list_h,
        Some(page),
        Some(HMENU(ID_UD_LIST as *mut core::ffi::c_void)),
        Some(crate::dll_instance().into()),
        None,
    ).unwrap_or_default();
    y += list_h + gap;

    // 입력 영역
    create_static(page, "단어:", lm, y, lbl_w, row_h);
    create_edit(page, ID_UD_EDT_WORD, "", lm + lbl_w + 4, y, edit_w, row_h);
    y += row_h + gap;

    create_static(page, "메모:", lm, y, lbl_w, row_h);
    create_edit(page, ID_UD_EDT_NOTE, "", lm + lbl_w + 4, y, edit_w, row_h);
    y += row_h + gap;

    // 버튼 행
    create_button(page, "추가",   ID_UD_BTN_ADD,    lm,                    y, btn_w, btn_h);
    create_button(page, "수정",   ID_UD_BTN_UPDATE, lm + btn_w + gap,      y, btn_w, btn_h);
    create_button(page, "삭제",   ID_UD_BTN_DELETE, lm + (btn_w + gap) * 2, y, btn_w, btn_h);

    // 초기 데이터 채우기
    if let Some(lb) = find_child_by_id(page, ID_UD_LIST) {
        listbox_clear(lb);
        for entry in &userdict.reverse_words {
            let note = entry.note.as_deref().unwrap_or("");
            let text = if note.is_empty() {
                entry.word.clone()
            } else {
                format!("{} — {}", entry.word, note)
            };
            listbox_add(lb, &text);
        }
    }
}

// ─── 전체 컨트롤 빌드 ───────────────────────────────────────────────────────

unsafe fn build_all_controls(hwnd: HWND, state: &mut DlgState) {
    // InitCommonControlsEx: trackbar + tab 클래스 등록
    let icc = INITCOMMONCONTROLSEX {
        dwSize: std::mem::size_of::<INITCOMMONCONTROLSEX>() as u32,
        dwICC: ICC_BAR_CLASSES | ICC_TAB_CLASSES,
    };
    let _ = InitCommonControlsEx(&icc);

    // ── 탭 컨트롤 생성 ───────────────────────────────────────────────────────
    let hwnd_tab = CreateWindowExW(
        WINDOW_EX_STYLE::default(),
        w!("SysTabControl32"),
        PCWSTR::null(),
        WS_CHILD | WS_VISIBLE | WS_CLIPSIBLINGS,
        TAB_X, TAB_Y, TAB_W, TAB_H,
        Some(hwnd),
        Some(HMENU(ID_TABCTRL as *mut core::ffi::c_void)),
        Some(crate::dll_instance().into()),
        None,
    ).unwrap_or_default();
    state.hwnd_tab = hwnd_tab;

    tab_insert(hwnd_tab, 0, "일반");
    tab_insert(hwnd_tab, 1, "오타 교정");
    tab_insert(hwnd_tab, 2, "억제 단어");
    tab_insert(hwnd_tab, 3, "사용자 사전");

    // ── 탭1 페이지 컨테이너 ──────────────────────────────────────────────────
    let hwnd_page_general = CreateWindowExW(
        WINDOW_EX_STYLE::default(),
        PAGE_CLASS_NAME,
        PCWSTR::null(),
        WS_CHILD | WS_VISIBLE | WS_CLIPSIBLINGS,
        PAGE_X, PAGE_Y, PAGE_W, PAGE_H,
        Some(hwnd_tab),
        None,
        Some(crate::dll_instance().into()),
        None,
    ).unwrap_or_default();
    state.hwnd_page_general = hwnd_page_general;

    // ── 탭2 페이지 컨테이너 ──────────────────────────────────────────────────
    let hwnd_page_typefix = CreateWindowExW(
        WINDOW_EX_STYLE::default(),
        PAGE_CLASS_NAME,
        PCWSTR::null(),
        WS_CHILD | WS_CLIPSIBLINGS,  // 초기에는 숨김
        PAGE_X, PAGE_Y, PAGE_W, PAGE_H,
        Some(hwnd_tab),
        None,
        Some(crate::dll_instance().into()),
        None,
    ).unwrap_or_default();
    state.hwnd_page_typefix = hwnd_page_typefix;

    // ── 탭3 페이지 컨테이너 ──────────────────────────────────────────────────
    let hwnd_page_blacklist = CreateWindowExW(
        WINDOW_EX_STYLE::default(),
        PAGE_CLASS_NAME,
        PCWSTR::null(),
        WS_CHILD | WS_CLIPSIBLINGS,  // 초기에는 숨김
        PAGE_X, PAGE_Y, PAGE_W, PAGE_H,
        Some(hwnd_tab),
        None,
        Some(crate::dll_instance().into()),
        None,
    ).unwrap_or_default();
    state.hwnd_page_blacklist = hwnd_page_blacklist;

    // ── 탭4 페이지 컨테이너 ──────────────────────────────────────────────────
    let hwnd_page_userdict = CreateWindowExW(
        WINDOW_EX_STYLE::default(),
        PAGE_CLASS_NAME,
        PCWSTR::null(),
        WS_CHILD | WS_CLIPSIBLINGS,  // 초기에는 숨김
        PAGE_X, PAGE_Y, PAGE_W, PAGE_H,
        Some(hwnd_tab),
        None,
        Some(crate::dll_instance().into()),
        None,
    ).unwrap_or_default();
    state.hwnd_page_userdict = hwnd_page_userdict;

    // ── 일반 탭 컨트롤 배치 ──────────────────────────────────────────────────
    let rule_sets_start_y = build_page_general(hwnd_page_general, &state.config);
    // chord_slider_y 에 rule_sets_start_y 를 보존 — 자판 변경 시 rebuild_rule_sets 에서 재사용
    state.chord_slider_y = rule_sets_start_y;
    let layout_name = state.config.engine.korean.layout.clone();
    let after_rulesets_y = rebuild_rule_sets(hwnd_page_general, state, &layout_name, rule_sets_start_y);
    build_page_general_rest(hwnd_page_general, &state.config, after_rulesets_y);

    // ── 오타 교정 탭 컨트롤 배치 ────────────────────────────────────────────
    build_page_typefix(hwnd_page_typefix, &state.config);

    // ── 억제 단어 탭 컨트롤 배치 ────────────────────────────────────────────
    build_page_blacklist(hwnd_page_blacklist, &state.blacklist);

    // ── 사용자 사전 탭 컨트롤 배치 ──────────────────────────────────────────
    build_page_userdict(hwnd_page_userdict, &state.userdict);

    // ── 하단 버튼 (메인 창 자식) ─────────────────────────────────────────────
    let btn_y = DLG_H - 44;
    create_button(hwnd, "확인",   ID_BTN_OK,     DLG_W - 3 * 88 - 8, btn_y, 80, 28);
    create_button(hwnd, "취소",   ID_BTN_CANCEL, DLG_W - 2 * 88 - 8, btn_y, 80, 28);
    create_button(hwnd, "적용",   ID_BTN_APPLY,  DLG_W - 1 * 88 - 8, btn_y, 80, 28);
}

// ─── 탭 페이지 전환 ─────────────────────────────────────────────────────────

unsafe fn switch_tab(state: &mut DlgState, tab_idx: i32) {
    state.cur_tab = tab_idx;

    let pages: &[(HWND, i32)] = &[
        (state.hwnd_page_general,   TAB_GENERAL),
        (state.hwnd_page_typefix,   TAB_TYPEFIX),
        (state.hwnd_page_blacklist, TAB_BLACKLIST),
        (state.hwnd_page_userdict,  TAB_USERDICT),
    ];

    for &(hwnd_page, idx) in pages {
        let style = GetWindowLongW(hwnd_page, GWL_STYLE) as u32;
        let show  = if tab_idx == idx { WS_VISIBLE.0 } else { 0u32 };
        SetWindowLongW(hwnd_page, GWL_STYLE,
            ((style & !WS_VISIBLE.0) | show) as i32);
        let _ = ShowWindow(hwnd_page, if tab_idx == idx { SW_SHOW } else { SW_HIDE });
    }
}

// ─── 컨트롤 수집: 현재 상태 → Config 반영 ───────────────────────────────────

unsafe fn get_ctrl(page: HWND, id: u32) -> Option<HWND> {
    GetDlgItem(Some(page), id as i32).ok().filter(|h| !h.is_invalid())
}

unsafe fn collect_general(state: &mut DlgState) {
    let pg = state.hwnd_page_general;

    // 한국어 자판
    if let Some(h) = get_ctrl(pg, ID_CMB_KOR_LAYOUT) {
        let idx = combobox_get_sel(h);
        if idx < KOREAN_LAYOUT_BUILTINS.len() {
            let new_layout = KOREAN_LAYOUT_BUILTINS[idx].to_string();
            if new_layout != state.config.engine.korean.layout {
                state.config.engine.korean.layout = new_layout;
            }
        }
    }

    // 영문 자판
    if let Some(h) = get_ctrl(pg, ID_CMB_ENG_LAYOUT) {
        let idx = combobox_get_sel(h);
        if idx < ENGLISH_LAYOUT_BUILTINS.len() {
            state.config.engine.english.layout = ENGLISH_LAYOUT_BUILTINS[idx].to_string();
        }
    }

    // 한글 확정 단위 (음절/단어/스마트) — 콤보 인덱스 = CommitUnit::all() 순서.
    if let Some(h) = get_ctrl(pg, ID_CMB_COMMIT_UNIT) {
        let idx = combobox_get_sel(h);
        let all = CommitUnit::all();
        if idx < all.len() {
            state.config.engine.korean.commit_unit = all[idx];
        }
    }

    // 규칙 세트
    collect_rule_sets(state);

    // 모아치기
    if let Some(h) = get_ctrl(pg, ID_CHK_BIDIR_COMBINE) {
        state.config.engine.korean.bidirectional_combine = Some(checkbox_checked(h));
    }
    if let Some(h) = get_ctrl(pg, ID_TRK_CHORD_MS) {
        let v = trackbar_get_pos(h) as u16;
        // 유효값: 0(OFF) 또는 10-200
        let validated = if v == 0 || (v >= 10 && v <= 200) { v } else { 0 };
        state.config.engine.korean.chord_window_ms = Some(validated);
    }

    // 시작 입력 모드
    if let Some(h) = get_ctrl(pg, ID_CMB_DEFAULT_CAT) {
        state.config.engine.default_category = if combobox_get_sel(h) == 1 {
            InputCategory::Korean
        } else {
            InputCategory::English
        };
    }

    // 모드 공유
    if let Some(h) = get_ctrl(pg, ID_CMB_MODE_SHARING) {
        state.config.engine.mode_sharing = if combobox_get_sel(h) == 1 {
            ModeSharingMode::PerApp
        } else {
            ModeSharingMode::Global
        };
    }

    // 전환키
    if let Some(h) = get_ctrl(pg, ID_EDT_TOGGLE_KEYS) {
        state.config.engine.toggle_keys = parse_key_list(&edit_get_text(h));
    }

    // 한자키
    if let Some(h) = get_ctrl(pg, ID_EDT_HANJA_KEYS) {
        state.config.engine.hanja_keys = parse_key_list(&edit_get_text(h));
    }

    // 단어 모드 앱 화이트리스트 (Smart 게이트 정확일치 대상). 빈 목록도 유효.
    if let Some(h) = get_ctrl(pg, ID_EDT_WORD_MODE_APPS) {
        state.config.engine.korean.word_mode_apps = parse_key_list(&edit_get_text(h));
    }

    // 자동 영문
    if let Some(h) = get_ctrl(pg, ID_CHK_AUTO_ENG) {
        state.config.engine.auto_english.enabled = checkbox_checked(h);
    }
    if let Some(h) = get_ctrl(pg, ID_EDT_AUTO_ENG_TRIGGERS) {
        state.config.engine.auto_english.trigger_keys = parse_key_list(&edit_get_text(h));
    }
}

unsafe fn collect_typefix(state: &mut DlgState) {
    let pg = state.hwnd_page_typefix;

    let chk = |id: u32| -> bool {
        get_ctrl(pg, id).map_or(false, |h| checkbox_checked(h))
    };
    let trk = |id: u32| -> i32 {
        get_ctrl(pg, id).map_or(0, |h| trackbar_get_pos(h))
    };

    state.config.engine.auto_typefix.enabled                   = chk(ID_CHK_ATF_ENABLED);
    state.config.engine.auto_typefix.forward                   = chk(ID_CHK_ATF_FORWARD);
    state.config.engine.auto_typefix.reverse                   = chk(ID_CHK_ATF_REVERSE);
    state.config.engine.auto_typefix.skip_on_english_word      = chk(ID_CHK_ATF_SKIP_ENG_WORD);
    state.config.engine.auto_typefix.skip_on_complete_syllable = chk(ID_CHK_ATF_SKIP_COMPLETE);
    state.config.engine.auto_typefix.rollback_detection        = chk(ID_CHK_ATF_ROLLBACK);
    state.config.engine.auto_typefix.user_dict_enabled         = chk(ID_CHK_ATF_USER_DICT);

    state.config.engine.auto_typefix.kor_syllable_threshold    = trk(ID_TRK_KOR_THRESHOLD) as u8;
    state.config.engine.auto_typefix.eng_word_min_length       = trk(ID_TRK_ENG_MIN_LEN) as u8;
    state.config.engine.auto_typefix.forward_time_window_ms    = trk(ID_TRK_FWD_TIME) as u32;
    state.config.engine.auto_typefix.reverse_time_window_ms    = trk(ID_TRK_REV_TIME) as u32;
    state.config.engine.auto_typefix.tentative_expiry_hours    = trk(ID_TRK_TENTATIVE) as u16;
    state.config.engine.auto_typefix.observation_timeout_secs  = trk(ID_TRK_OBS_TIMEOUT) as u8;
}

unsafe fn collect_all(state: &mut DlgState) {
    collect_general(state);
    collect_typefix(state);
}

// ─── 저장 시도 ───────────────────────────────────────────────────────────────

unsafe fn try_save(hwnd: HWND, config: &Config) {
    if let Err(e) = config.save_to_default_path() {
        let msg = format!("설정 저장 실패:\n{}", e);
        let msg_w = to_wide_nul(&msg);
        let title_w = to_wide_nul("UNIM 설정");
        MessageBoxW(
            Some(hwnd),
            PCWSTR(msg_w.as_ptr()),
            PCWSTR(title_w.as_ptr()),
            MB_OK | MB_ICONERROR,
        );
    }
}

// ─── WM_HSCROLL 처리 ─────────────────────────────────────────────────────────

unsafe fn handle_hscroll(hwnd_dlg: HWND, hwnd_ctrl: HWND) {
    let ctrl_id = GetDlgCtrlID(hwnd_ctrl) as u32;

    // 오타 교정 탭 슬라이더
    let typefix_lbl = match ctrl_id {
        ID_TRK_KOR_THRESHOLD => Some(ID_LBL_KOR_THRESHOLD),
        ID_TRK_ENG_MIN_LEN   => Some(ID_LBL_ENG_MIN_LEN),
        ID_TRK_FWD_TIME      => Some(ID_LBL_FWD_TIME),
        ID_TRK_REV_TIME      => Some(ID_LBL_REV_TIME),
        ID_TRK_TENTATIVE     => Some(ID_LBL_TENTATIVE),
        ID_TRK_OBS_TIMEOUT   => Some(ID_LBL_OBS_TIMEOUT),
        _ => None,
    };
    if let Some(lbl_id) = typefix_lbl {
        let pos = trackbar_get_pos(hwnd_ctrl);
        update_value_label(hwnd_dlg, lbl_id, pos);
        return;
    }

    // 일반 탭 슬라이더
    if ctrl_id == ID_TRK_CHORD_MS {
        let pos = trackbar_get_pos(hwnd_ctrl);
        let display = if pos == 0 { "OFF".to_string() } else { pos.to_string() };
        let display_w = to_wide_nul(&display);
        if let Some(lbl) = find_child_by_id(hwnd_dlg, ID_LBL_CHORD_MS) {
            let _ = SetWindowTextW(lbl, PCWSTR(display_w.as_ptr()));
        }
    }
}

// ─── WM_COMMAND 처리 (자판 콤보 변경 시 룰셋 재구성) ────────────────────────

unsafe fn handle_command(hwnd: HWND, state: &mut DlgState, ctrl_id: u32, notify_code: u32) {
    match ctrl_id {
        ID_BTN_OK => {
            collect_all(state);
            try_save(hwnd, &state.config);
            let parent = state.parent;
            restore_parent(parent);
            let _ = DestroyWindow(hwnd);
        }
        ID_BTN_CANCEL => {
            let parent = state.parent;
            restore_parent(parent);
            let _ = DestroyWindow(hwnd);
        }
        ID_BTN_APPLY => {
            collect_all(state);
            try_save(hwnd, &state.config);
        }
        ID_BTN_SET_DEFAULT => {
            match crate::register::set_as_default() {
                Ok(()) => {
                    let msg_w   = to_wide_nul("기본 입력기로 설정되었습니다.");
                    let title_w = to_wide_nul("UNIM 설정");
                    MessageBoxW(Some(hwnd), PCWSTR(msg_w.as_ptr()), PCWSTR(title_w.as_ptr()),
                        MB_OK | MB_ICONINFORMATION);
                }
                Err(e) => {
                    let msg_str = format!("기본 입력기 설정 실패:\n{}", e);
                    let msg_w   = to_wide_nul(&msg_str);
                    let title_w = to_wide_nul("UNIM 설정");
                    MessageBoxW(Some(hwnd), PCWSTR(msg_w.as_ptr()), PCWSTR(title_w.as_ptr()),
                        MB_OK | MB_ICONERROR);
                }
            }
        }
        ID_CMB_KOR_LAYOUT if notify_code == CBN_SELCHANGE => {
            // 한국어 자판 변경 → 룰셋 재구성
            let pg = state.hwnd_page_general;
            if let Some(h) = get_ctrl(pg, ID_CMB_KOR_LAYOUT) {
                let idx = combobox_get_sel(h);
                if idx < KOREAN_LAYOUT_BUILTINS.len() {
                    let new_layout = KOREAN_LAYOUT_BUILTINS[idx].to_string();
                    // 이전 자판의 rule_sets 캐시 저장 후 새 자판으로 전환
                    state.config.engine.korean.cache_active_rule_sets();
                    state.config.engine.korean.layout = new_layout.clone();
                    // chord_slider_y 는 build_all_controls 에서 rule_sets_start_y 로 설정됨
                    let rule_sets_start_y = state.chord_slider_y;
                    let _after_y = rebuild_rule_sets(pg, state, &new_layout, rule_sets_start_y);
                    // rest 컨트롤(모아치기/모드/키설정/자동영문)은 이미 고정 위치에 생성되어 있어
                    // 룰셋 수가 달라지면 겹칠 수 있음 — 향후 스크롤 창으로 개선 예정
                }
            }
        }

        // ── 억제 단어 탭 버튼 ────────────────────────────────────────────────
        ID_BL_BTN_DELETE => {
            let pg = state.hwnd_page_blacklist;
            if let Some(lb) = find_child_by_id(pg, ID_BL_LIST) {
                if let Some(sel) = listbox_get_sel(lb) {
                    state.blacklist.remove(sel);
                    let _ = state.blacklist.save_to_default_path();
                    refresh_blacklist_page(pg, &state.blacklist);
                }
            }
        }
        ID_BL_BTN_DEACTIVATE => {
            let pg = state.hwnd_page_blacklist;
            if let Some(lb) = find_child_by_id(pg, ID_BL_LIST) {
                if let Some(sel) = listbox_get_sel(lb) {
                    // 현재 상태에 따라 토글: 활성→비활성, 비활성→재활성
                    let is_active = state.blacklist.entries.get(sel)
                        .map_or(false, |e| e.status.is_active());
                    if is_active {
                        state.blacklist.deactivate(sel);
                    } else {
                        state.blacklist.reactivate_as_confirmed(sel);
                    }
                    let _ = state.blacklist.save_to_default_path();
                    refresh_blacklist_page(pg, &state.blacklist);
                }
            }
        }
        ID_BL_BTN_CONFIRM => {
            let pg = state.hwnd_page_blacklist;
            if let Some(lb) = find_child_by_id(pg, ID_BL_LIST) {
                if let Some(sel) = listbox_get_sel(lb) {
                    state.blacklist.promote_to_confirmed(sel);
                    let _ = state.blacklist.save_to_default_path();
                    refresh_blacklist_page(pg, &state.blacklist);
                }
            }
        }

        // ── 사용자 사전 탭 버튼 ──────────────────────────────────────────────
        ID_UD_BTN_ADD => {
            let pg = state.hwnd_page_userdict;
            let word = find_child_by_id(pg, ID_UD_EDT_WORD)
                .map(|h| edit_get_text(h))
                .unwrap_or_default();
            if !word.is_empty() {
                let note = find_child_by_id(pg, ID_UD_EDT_NOTE)
                    .map(|h| edit_get_text(h))
                    .filter(|s| !s.is_empty());
                if state.userdict.add(&word, note) {
                    let _ = state.userdict.save_to_default_path();
                    refresh_userdict_page(pg, &state.userdict);
                    // 입력 필드 초기화
                    if let Some(h) = find_child_by_id(pg, ID_UD_EDT_WORD) {
                        let _ = SetWindowTextW(h, w!(""));
                    }
                    if let Some(h) = find_child_by_id(pg, ID_UD_EDT_NOTE) {
                        let _ = SetWindowTextW(h, w!(""));
                    }
                } else {
                    let msg_w   = to_wide_nul("추가 실패: 이미 존재하거나 영문이 아닌 단어입니다.");
                    let title_w = to_wide_nul("UNIM 설정");
                    MessageBoxW(Some(hwnd), PCWSTR(msg_w.as_ptr()), PCWSTR(title_w.as_ptr()),
                        MB_OK | MB_ICONWARNING);
                }
            }
        }
        ID_UD_BTN_UPDATE => {
            let pg = state.hwnd_page_userdict;
            if let Some(lb) = find_child_by_id(pg, ID_UD_LIST) {
                if let Some(sel) = listbox_get_sel(lb) {
                    let word = find_child_by_id(pg, ID_UD_EDT_WORD)
                        .map(|h| edit_get_text(h))
                        .unwrap_or_default();
                    if !word.is_empty() {
                        let note = find_child_by_id(pg, ID_UD_EDT_NOTE)
                            .map(|h| edit_get_text(h))
                            .filter(|s| !s.is_empty());
                        if state.userdict.update_at(sel, &word, note) {
                            let _ = state.userdict.save_to_default_path();
                            refresh_userdict_page(pg, &state.userdict);
                        } else {
                            let msg_w   = to_wide_nul("수정 실패: 중복이거나 영문이 아닌 단어입니다.");
                            let title_w = to_wide_nul("UNIM 설정");
                            MessageBoxW(Some(hwnd), PCWSTR(msg_w.as_ptr()), PCWSTR(title_w.as_ptr()),
                                MB_OK | MB_ICONWARNING);
                        }
                    }
                }
            }
        }
        ID_UD_BTN_DELETE => {
            let pg = state.hwnd_page_userdict;
            if let Some(lb) = find_child_by_id(pg, ID_UD_LIST) {
                if let Some(sel) = listbox_get_sel(lb) {
                    state.userdict.remove_at(sel);
                    let _ = state.userdict.save_to_default_path();
                    refresh_userdict_page(pg, &state.userdict);
                }
            }
        }

        _ => {}
    }
}

// ─── WndProc ─────────────────────────────────────────────────────────────────

unsafe extern "system" fn settings_wnd_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    match msg {
        WM_CREATE => {
            let cs = &*(lparam.0 as *const CREATESTRUCTW);
            let state_ptr = cs.lpCreateParams as *mut DlgState;
            SetWindowLongPtrW(hwnd, GWLP_USERDATA, state_ptr as _);

            let state = &mut *state_ptr;
            build_all_controls(hwnd, state);
            LRESULT(0)
        }

        WM_COMMAND => {
            let ctrl_id     = (wparam.0 & 0xFFFF) as u32;
            let notify_code = ((wparam.0 >> 16) & 0xFFFF) as u32;
            let state = &mut *(GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut DlgState);
            handle_command(hwnd, state, ctrl_id, notify_code);
            LRESULT(0)
        }

        WM_NOTIFY => {
            let nmhdr = &*(lparam.0 as *const NMHDR);
            // TCN_SELCHANGE 는 u32(0xFFFFFED9) — windows 0.58 상수 사용
            if nmhdr.code == TCN_SELCHANGE {
                let state = &mut *(GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut DlgState);
                let tab_idx =
                    SendMessageW(state.hwnd_tab, TCM_GETCURSEL, Some(WPARAM(0)), Some(LPARAM(0))).0 as i32;
                switch_tab(state, tab_idx);
            }
            LRESULT(0)
        }

        WM_HSCROLL => {
            let hwnd_ctrl = HWND(lparam.0 as *mut core::ffi::c_void);
            handle_hscroll(hwnd, hwnd_ctrl);
            LRESULT(0)
        }

        WM_CLOSE => {
            let state = &*(GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *const DlgState);
            let parent = state.parent;
            restore_parent(parent);
            let _ = DestroyWindow(hwnd);
            LRESULT(0)
        }

        WM_DESTROY => {
            let ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut DlgState;
            if !ptr.is_null() {
                drop(Box::from_raw(ptr));
                SetWindowLongPtrW(hwnd, GWLP_USERDATA, 0);
            }
            PostQuitMessage(0);
            LRESULT(0)
        }

        _ => DefWindowProcW(hwnd, msg, wparam, lparam),
    }
}

// ─── 공개 진입점 ─────────────────────────────────────────────────────────────

/// 설정 다이얼로그를 모달로 표시한다.
pub fn show_settings_dialog(hwnd_parent: HWND) {
    unsafe {
        ensure_wnd_class();

        let config = Config::load_from_default_path();
        let blacklist = Blacklist::load_from_default_path();
        let userdict = UserDictionary::load_from_default_path();

        let state = Box::new(DlgState {
            config,
            parent: hwnd_parent,
            hwnd_tab: HWND::default(),
            hwnd_page_general: HWND::default(),
            hwnd_page_typefix: HWND::default(),
            hwnd_page_blacklist: HWND::default(),
            hwnd_page_userdict: HWND::default(),
            cur_tab: TAB_GENERAL,
            rule_set_entries: Vec::new(),
            chord_slider_y: 0,
            blacklist,
            userdict,
        });
        let state_ptr = Box::into_raw(state);

        if !hwnd_parent.is_invalid() {
            let _ = EnableWindow(hwnd_parent, false);
        }

        let hwnd_dlg = CreateWindowExW(
            WS_EX_DLGMODALFRAME | WS_EX_WINDOWEDGE,
            WND_CLASS_NAME,
            w!("UNIM 입력기 설정"),
            WS_OVERLAPPED | WS_CAPTION | WS_SYSMENU | WS_VISIBLE,
            CW_USEDEFAULT, CW_USEDEFAULT, DLG_W, DLG_H,
            Some(hwnd_parent),
            None,
            Some(crate::dll_instance().into()),
            Some(state_ptr as *const core::ffi::c_void),
        );

        let hwnd_dlg = match hwnd_dlg {
            Ok(h) => h,
            Err(_) => {
                drop(Box::from_raw(state_ptr));
                restore_parent(hwnd_parent);
                return;
            }
        };

        // 부모 기준 중앙 배치
        if !hwnd_parent.is_invalid() {
            let mut pr = RECT::default();
            if GetWindowRect(hwnd_parent, &mut pr).is_ok() {
                let cx = (pr.left + pr.right) / 2 - DLG_W / 2;
                let cy = (pr.top + pr.bottom) / 2 - DLG_H / 2;
                let _ = SetWindowPos(hwnd_dlg, None, cx, cy, 0, 0, SWP_NOSIZE | SWP_NOZORDER);
            }
        }

        // 자체 모달 메시지 루프
        let mut msg = MSG::default();
        loop {
            let ret = GetMessageW(&mut msg, None, 0, 0);
            if ret.0 == 0 || ret.0 == -1 {
                break;
            }
            if IsDialogMessageW(hwnd_dlg, &msg).as_bool() {
                continue;
            }
            let _ = TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }

        restore_parent(hwnd_parent);
    }
}
