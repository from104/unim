//! 언어 바 아이템 — 한/영 모드 표시 버튼 (ITfLangBarItemButton)
//!
//! 갭1(엔진→langbar) · 갭2(langbar→엔진) 를 모두 닫는 구현.
//! LangBarState (Arc 공유) 를 통해 text_service 와 UnimLangBarButton 이
//! 동일 is_korean 플래그와 ITfLangBarItemSink 를 참조한다.

use std::sync::{Arc, Mutex};
use std::sync::atomic::{AtomicBool, Ordering};

use windows::core::*;
use windows::Win32::Foundation::*;
use windows::Win32::Graphics::Gdi::{
    CreateBitmap, CreateCompatibleBitmap, CreateCompatibleDC, CreateFontW, DeleteDC, DeleteObject,
    DrawTextW, GetDC, PatBlt, ReleaseDC, SelectObject, SetBkColor, SetBkMode, SetTextColor,
    BLACKNESS, CLEARTYPE_QUALITY, CLIP_DEFAULT_PRECIS, DEFAULT_CHARSET, DEFAULT_PITCH,
    DT_CENTER, DT_NOPREFIX, DT_SINGLELINE, DT_VCENTER, FF_DONTCARE, FW_SEMIBOLD, HBITMAP,
    HBRUSH, HGDIOBJ, OPAQUE, OUT_DEFAULT_PRECIS, TRANSPARENT, WHITENESS,
};
use windows::Win32::System::Diagnostics::Debug::Beep;
use windows::Win32::System::Registry::{
    RegGetValueW, HKEY_CURRENT_USER, RRF_RT_REG_DWORD,
};
use windows::Win32::UI::Accessibility::NotifyWinEvent;
use windows::Win32::UI::TextServices::*;
use windows::Win32::UI::WindowsAndMessaging::{
    AppendMenuW, CreateIconIndirect, CreatePopupMenu, CreateWindowExW, CS_NOCLOSE,
    DefWindowProcW, DestroyMenu, DestroyWindow, GetForegroundWindow, GetSystemMetrics,
    MessageBoxW, PostMessageW, RegisterClassExW, SetForegroundWindow, HCURSOR, HICON, ICONINFO,
    EVENT_OBJECT_NAMECHANGE, EVENT_OBJECT_STATECHANGE, MB_ICONINFORMATION, MB_OK, MENU_ITEM_FLAGS,
    OBJID_CLIENT, SM_CXSMICON, SM_CYSMICON, TPM_LEFTALIGN,
    TPM_NONOTIFY, TPM_RETURNCMD, TPM_TOPALIGN, WINDOW_EX_STYLE, WNDCLASSEXW, WM_NULL,
    WS_POPUP,
};

/// 문자열 메뉴 항목 플래그. Win32 `MF_STRING` == 0x0 이지만 windows-rs 0.62 가
/// 상수를 노출하지 않아 직접 정의한다 (AppendMenuW 의 uflags 인자용).
const MF_STRING: MENU_ITEM_FLAGS = MENU_ITEM_FLAGS(0x0000_0000);

// windows-rs 0.62 의 TrackPopupMenuEx 바인딩은 반환을 `BOOL` 로 감싸, TPM_RETURNCMD
// 로 받아야 하는 "선택된 메뉴 항목 ID(정수)" 를 그대로 꺼내기 어렵다(`.0` 접근이
// 단위형 `()` 으로 추론됨). 선택 ID 를 i32 로 직접 받기 위해 user32 의
// TrackPopupMenuEx 를 명시적으로 선언한다. HMENU/HWND 는 #[repr(transparent)]
// FFI-safe 타입이라 windows-rs 정의를 그대로 사용. (unim-tsf 는 edition 2024 →
// extern 블록은 반드시 `unsafe extern`.)
#[link(name = "user32")]
unsafe extern "system" {
    fn TrackPopupMenuEx(
        hmenu: windows::Win32::UI::WindowsAndMessaging::HMENU,
        uflags: u32,
        x: i32,
        y: i32,
        hwnd: HWND,
        lptpm: *const core::ffi::c_void,
    ) -> i32;
}

use unim::config::{Config, InputCategory};
use unim::input_engine::InputEngine;

use crate::globals;

/// 작업표시줄/시스템이 다크 테마인지 조회한다.
///
/// `HKCU\...\Themes\Personalize\SystemUsesLightTheme` (DWORD) 가 0 이면 다크,
/// 1 이면 라이트. 키가 없으면(구버전 등) 라이트로 간주한다. 트레이 아이콘 글자색
/// 결정에 쓴다 — 다크 배경엔 흰 글자, 라이트 배경엔 검은 글자.
fn system_uses_dark_theme() -> bool {
    let subkey: Vec<u16> = "Software\\Microsoft\\Windows\\CurrentVersion\\Themes\\Personalize\0"
        .encode_utf16()
        .collect();
    let value: Vec<u16> = "SystemUsesLightTheme\0".encode_utf16().collect();
    let mut data: u32 = 1; // 기본=라이트
    let mut size = std::mem::size_of::<u32>() as u32;
    unsafe {
        let r = RegGetValueW(
            HKEY_CURRENT_USER,
            PCWSTR(subkey.as_ptr()),
            PCWSTR(value.as_ptr()),
            RRF_RT_REG_DWORD,
            None,
            Some(&mut data as *mut u32 as *mut core::ffi::c_void),
            Some(&mut size),
        );
        if r.is_err() {
            return false; // 조회 실패 → 라이트로 폴백
        }
    }
    data == 0
}

/// 한/영 상태 텍스트("가"/"A")를 GDI 로 그려 작은 트레이용 HICON 을 만든다.
///
/// **투명 배경 + 테마 적응 글자색**: 1bpp AND 마스크에 글자 모양만 불투명으로
/// 새겨 배경을 투명하게 두고, 컬러 비트맵의 글자색을 다크 테마면 흰색,
/// 라이트 테마면 검은색으로 칠한다. 작업표시줄 어느 테마에서도 글자가 또렷하다.
///
/// .ico 리소스를 DLL 에 임베드하지 않고 런타임에 그려, 현재 입력 상태와 테마를
/// 함께 반영한다. 어떤 단계든 실패하면 `None` → 호출자(GetIcon)는 E_FAIL 폴백.
/// 반환 HICON 의 소유권은 OS 에 있다(GetIcon 계약 — OS 가 DestroyIcon).
fn create_status_icon(text: &str) -> Option<HICON> {
    unsafe {
        let cx = GetSystemMetrics(SM_CXSMICON);
        let cy = GetSystemMetrics(SM_CYSMICON);
        if cx <= 0 || cy <= 0 {
            return None;
        }

        let screen_dc = GetDC(None);
        if screen_dc.is_invalid() {
            return None;
        }

        let mem_dc = CreateCompatibleDC(Some(screen_dc));
        let color_bmp = CreateCompatibleBitmap(screen_dc, cx, cy);
        // 1bpp AND 마스크. 1=투명, 0=불투명(컬러 표시).
        let mask_bmp: HBITMAP = CreateBitmap(cx, cy, 1, 1, None);

        if mem_dc.is_invalid() || color_bmp.is_invalid() || mask_bmp.is_invalid() {
            let _ = DeleteObject(HGDIOBJ(color_bmp.0));
            let _ = DeleteObject(HGDIOBJ(mask_bmp.0));
            if !mem_dc.is_invalid() {
                let _ = DeleteDC(mem_dc);
            }
            ReleaseDC(None, screen_dc);
            return None;
        }

        // 한글 글리프가 있는 폰트(맑은 고딕)로 그린다. DEFAULT_GUI_FONT
        // (Tahoma/MS Sans Serif)는 한글 글리프가 없어 "가" 가 .notdef 빈 박스로만
        // 그려진다. 높이는 음수(문자 높이)여야 작은 아이콘에 맞다.
        let hfont = CreateFontW(
            -(cy * 3 / 4),
            0,
            0,
            0,
            FW_SEMIBOLD.0 as i32,
            0,
            0,
            0,
            DEFAULT_CHARSET,
            OUT_DEFAULT_PRECIS,
            CLIP_DEFAULT_PRECIS,
            CLEARTYPE_QUALITY,
            (DEFAULT_PITCH.0 | FF_DONTCARE.0) as u32,
            w!("Malgun Gothic"),
        );
        let mut wide: Vec<u16> = text.encode_utf16().collect();
        // 아이콘 전체 사각형에 수평·수직 중앙 정렬. TextOutW 고정 오프셋은
        // 폰트 셀 leading 때문에 글자가 아래로 처졌다.
        let mut rect = RECT { left: 0, top: 0, right: cx, bottom: cy };
        let dt_flags = DT_CENTER | DT_VCENTER | DT_SINGLELINE | DT_NOPREFIX;

        // ── (1) 1bpp 마스크: 글자=0(불투명), 배경=1(투명) ──
        // 1bpp DC 에서 흰색→1, 검은색→0 으로 매핑된다. 배경을 흰색(1=투명)으로
        // 채우고 글자를 검은색(0=불투명)으로 그리면, 글자 모양만 불투명해진다.
        let mask_dc = CreateCompatibleDC(Some(screen_dc));
        if !mask_dc.is_invalid() {
            let old = SelectObject(mask_dc, HGDIOBJ(mask_bmp.0));
            let _ = PatBlt(mask_dc, 0, 0, cx, cy, WHITENESS); // 전부 1(투명)
            let old_f = SelectObject(mask_dc, HGDIOBJ(hfont.0));
            SetBkMode(mask_dc, OPAQUE);
            SetBkColor(mask_dc, COLORREF(0x00FF_FFFF)); // 배경=흰(1)
            SetTextColor(mask_dc, COLORREF(0x0000_0000)); // 글자=검(0)
            let _ = DrawTextW(mask_dc, &mut wide, &mut rect, dt_flags);
            SelectObject(mask_dc, old_f);
            SelectObject(mask_dc, old);
            let _ = DeleteDC(mask_dc);
        }

        // ── (2) 컬러 비트맵: 글자색 = 테마 적응(다크→흰, 라이트→검) ──
        // 배경은 검정으로 채운다(마스크가 투명 처리하므로 안 보임). 글자만 칠한다.
        let text_color = if system_uses_dark_theme() {
            COLORREF(0x00FF_FFFF) // 흰 글자
        } else {
            COLORREF(0x0000_0000) // 검 글자
        };
        let old_bmp = SelectObject(mem_dc, HGDIOBJ(color_bmp.0));
        let _ = PatBlt(mem_dc, 0, 0, cx, cy, BLACKNESS); // 배경 검정(마스크로 투명)
        let old_font = SelectObject(mem_dc, HGDIOBJ(hfont.0));
        SetBkMode(mem_dc, TRANSPARENT);
        SetTextColor(mem_dc, text_color);
        let _ = DrawTextW(mem_dc, &mut wide, &mut rect, dt_flags);
        SelectObject(mem_dc, old_font);
        SelectObject(mem_dc, old_bmp);

        // CreateFontW 핸들은 스톡 오브젝트가 아니므로 반드시 DeleteObject.
        let _ = DeleteObject(HGDIOBJ(hfont.0));

        let ii = ICONINFO {
            fIcon: TRUE,
            xHotspot: 0,
            yHotspot: 0,
            hbmMask: mask_bmp,
            hbmColor: color_bmp,
        };
        // CreateIconIndirect 는 비트맵을 복사하므로 이후 우리 비트맵 삭제 가능.
        let hicon = CreateIconIndirect(&ii).ok();

        let _ = DeleteObject(HGDIOBJ(color_bmp.0));
        let _ = DeleteObject(HGDIOBJ(mask_bmp.0));
        let _ = DeleteDC(mem_dc);
        ReleaseDC(None, screen_dc);

        hicon.filter(|h| !h.is_invalid())
    }
}

// 메뉴 항목 ID.
//
// 1 부터 시작한다. 우클릭 메뉴를 TrackPopupMenuEx(TPM_RETURNCMD) 로 직접 띄울 때
// 반환값 0 은 "취소(선택 안 함)" 를 의미하므로, 항목 ID 가 0 이면 첫 항목 선택과
// 취소를 구분할 수 없다. InitMenu/OnMenuSelect 경로도 동일 상수를 쓰므로 양쪽 일관.
const MENU_ID_TOGGLE: u32 = 1;
const MENU_ID_SET_DEFAULT: u32 = 2;
const MENU_ID_SETTINGS: u32 = 3;
const MENU_ID_ABOUT: u32 = 4;

/// 정보(About) 다이얼로그를 띄운다.
///
/// settings_dialog 의 본문(커스텀 창)을 건드리지 않고, Win32 `MessageBoxW`
/// 로 UNIM 이름·버전·간단한 설명을 표시한다. TSF STA 스레드(OnMenuSelect)
/// 에서 호출되며, 모달이므로 추가 메시지 펌프가 필요 없다.
fn show_about_dialog(hwnd: HWND) {
    let caption: Vec<u16> = "UNIM 정보\0".encode_utf16().collect();
    let body = format!(
        "UNIM Korean IME\n버전 {}\n\n범용 차세대 입력기\nhttps://github.com/from104/unim",
        env!("CARGO_PKG_VERSION")
    );
    let text: Vec<u16> = body.encode_utf16().chain(std::iter::once(0)).collect();
    unsafe {
        let hwnd_opt = if hwnd.is_invalid() { None } else { Some(hwnd) };
        MessageBoxW(
            hwnd_opt,
            PCWSTR(text.as_ptr()),
            PCWSTR(caption.as_ptr()),
            MB_OK | MB_ICONINFORMATION,
        );
    }
}

// ── helper 창 WndProc ──────────────────────────────────────────────────────────

/// TrackPopupMenuEx owner 용 helper 창의 메시지 처리기.
///
/// 최소 구현: DefWindowProcW 에 위임. 이 창은 화면 밖에 있어 사용자에게 보이지 않는다.
unsafe extern "system" fn helper_wnd_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) }
}

// ── 공유 상태 ──────────────────────────────────────────────────────────────────

/// text_service 와 UnimLangBarButton 이 Arc 로 공유하는 랭귀지바 상태.
///
/// - `is_korean`: 엔진 모드의 캐시. text_service(OnKeyDown 후) 와
///   UnimLangBarButton(OnClick/OnMenuSelect) 양쪽에서 갱신.
/// - `sink`: Windows 가 AdviseSink 로 등록한 ITfLangBarItemSink.
///   갱신 시 OnUpdate() 를 발사해 랭귀지바에 변화를 알린다.
pub struct LangBarState {
    pub is_korean: AtomicBool,
    /// 현재 포커스 앱이 오버레이 폴백(composition 미지원 CUAS/IMM32 브리지)이라
    /// 자동교정(ATF)이 제한됨을 나타내는 플래그. true 면 툴팁에 "이 앱은 자동교정 제한"을
    /// 덧붙여 사용자에게 조용한 OFF 를 고지한다. OnSetFocus(정상 앱 복귀) 시 해제된다.
    pub atf_limited: AtomicBool,
    pub sink: Mutex<Option<ITfLangBarItemSink>>,
    /// ActivateEx 에서 주입되는 thread_mgr + client id.
    /// `update()` 가 OS 입력 표시기 compartment 를 동기화할 때 사용한다.
    /// 키 입력 경로(OnKeyDown)와 랭귀지바 클릭 경로(toggle_engine_mode) 양쪽이
    /// 모두 `update()` 를 거치므로, compartment 동기화를 여기 한 곳에 모은다.
    tsf: Mutex<Option<(ITfThreadMgr, u32)>>,
    /// TrackPopupMenuEx 의 owner HWND fallback 용 helper 창.
    ///
    /// GetForegroundWindow() 가 NULL(HWND(0))을 반환하는 경우(트레이 인디케이터
    /// 우클릭 시 전경 창이 없는 상황) TrackPopupMenuEx 에 NULL owner 를 전달하면
    /// 메뉴가 표시되지 않는다. 이를 막기 위해 화면 밖 1×1 WS_POPUP 창을 지연
    /// 생성해 보관한다. message-only(HWND_MESSAGE) 는 TrackPopupMenuEx dismiss 가
    /// 깨지므로 가시 창이어야 한다.
    helper_hwnd: Mutex<HWND>,
}

// SAFETY: ITfLangBarItemSink / ITfThreadMgr 는 TSF STA 스레드에서만 접근하며,
// Mutex 로 보호하므로 Send/Sync 를 수동 구현.
unsafe impl Send for LangBarState {}
unsafe impl Sync for LangBarState {}

impl LangBarState {
    pub fn new(is_korean: bool) -> Arc<Self> {
        Arc::new(Self {
            is_korean: AtomicBool::new(is_korean),
            atf_limited: AtomicBool::new(false),
            sink: Mutex::new(None),
            tsf: Mutex::new(None),
            helper_hwnd: Mutex::new(HWND::default()),
        })
    }

    /// ActivateEx 에서 thread_mgr + client id 를 주입한다.
    /// 이후 `update()` 호출 시 compartment 동기화가 활성화된다.
    pub fn set_tsf(&self, thread_mgr: ITfThreadMgr, tid: u32) {
        *self.tsf.lock().unwrap() = Some((thread_mgr, tid));
    }

    /// TrackPopupMenuEx owner 용 helper HWND 를 지연 생성·반환한다.
    ///
    /// GetForegroundWindow() 가 NULL(HWND(0))일 때 TrackPopupMenuEx 에 NULL owner 를
    /// 전달하면 메뉴가 표시되지 않는다. 이 함수는 화면 밖(-32000, -32000) 1×1
    /// WS_POPUP 창을 한 번만 만들어 재사용한다.
    ///
    /// - message-only 창(HWND_MESSAGE)은 TrackPopupMenuEx 의 dismiss(클릭 아웃 닫기)가
    ///   깨지므로 가시(visible) 창으로 만든다. 화면 밖 좌표라 사용자 눈에 보이지 않는다.
    /// - DLL 수명 동안 유지되며 DeactivateEx 에서 명시적으로 파괴할 수도 있다.
    ///
    /// 실패 시 HWND(0) 반환 → 호출자가 적절히 처리.
    pub fn get_or_create_helper_hwnd(&self) -> HWND {
        let mut guard = self.helper_hwnd.lock().unwrap();
        // 이미 유효한 HWND 가 있으면 재사용
        if guard.0 as isize != 0 {
            return *guard;
        }
        unsafe {
            // 창 클래스 이름 (null 종결 UTF-16)
            let class_name: Vec<u16> = "UNIM_HelperWnd\0".encode_utf16().collect();
            // 창 클래스 최초 등록. RegisterClassExW 는 같은 클래스명이 이미 등록돼
            // 있으면 ERROR_CLASS_ALREADY_EXISTS(1410) 을 반환하므로 결과 무시해도 안전.
            let wc = WNDCLASSEXW {
                cbSize: std::mem::size_of::<WNDCLASSEXW>() as u32,
                style: CS_NOCLOSE,
                lpfnWndProc: Some(helper_wnd_proc),
                cbClsExtra: 0,
                cbWndExtra: 0,
                hInstance: crate::dll_instance().into(),
                hIcon: HICON::default(),
                hCursor: HCURSOR::default(),
                hbrBackground: HBRUSH::default(),
                lpszMenuName: PCWSTR::null(),
                lpszClassName: PCWSTR(class_name.as_ptr()),
                hIconSm: HICON::default(),
            };
            let _ = RegisterClassExW(&wc);

            let title: Vec<u16> = "\0".encode_utf16().collect();
            // 화면 밖(-32000, -32000) 1×1 WS_POPUP 창. WS_VISIBLE 이어야
            // TrackPopupMenuEx 의 dismiss(click-outside) 가 올바로 동작한다.
            let hwnd = CreateWindowExW(
                WINDOW_EX_STYLE(0),
                PCWSTR(class_name.as_ptr()),
                PCWSTR(title.as_ptr()),
                WS_POPUP,
                -32000,
                -32000,
                1,
                1,
                None,  // parent = NULL (데스크탑)
                None,  // menu = NULL
                Some(crate::dll_instance().into()),
                None,
            );
            let hwnd = match hwnd {
                Ok(h) if h.0 as isize != 0 => h,
                _ => HWND::default(),
            };
            *guard = hwnd;
            hwnd
        }
    }

    /// helper HWND 를 파괴한다. DeactivateEx 또는 DLL_PROCESS_DETACH 에서 호출.
    pub fn destroy_helper_hwnd(&self) {
        let mut guard = self.helper_hwnd.lock().unwrap();
        if guard.0 as isize != 0 {
            unsafe {
                let _ = DestroyWindow(*guard);
            }
            *guard = HWND::default();
        }
    }

    /// is_korean 을 갱신하고, sink OnUpdate 발사 + OS compartment 동기화 +
    /// 스크린리더 능동 통지(NotifyWinEvent) + (옵션) 비프를 한다.
    ///
    /// OnKeyDown(엔진 토글)·toggle_engine_mode(랭귀지바 클릭) 양쪽이 이 한 곳을
    /// 거치므로, 모드 변경 시 랭귀지바와 OS 입력 표시기가 항상 함께 갱신된다.
    ///
    /// `announce_beep` = `config.engine.toggle_announce_beep`. true 면 한/영 차등
    /// 비프음을 울려 무시각 사용자가 현재 모드를 확인할 수 있다.
    pub fn update(&self, is_korean: bool, announce_beep: bool) {
        self.is_korean.store(is_korean, Ordering::SeqCst);
        if let Ok(guard) = self.sink.lock() {
            if let Some(ref sink) = *guard {
                unsafe {
                    let _ = sink.OnUpdate(TF_LBI_STATUS | TF_LBI_ICON | TF_LBI_TEXT);
                }
            }
        }
        // OS 입력 표시기 동기화 (compartment 2개). thread_mgr 미주입 시 skip.
        if let Ok(guard) = self.tsf.lock() {
            if let Some((ref tmgr, tid)) = *guard {
                crate::compartment::sync_keyboard_mode(tmgr, tid, is_korean);
            }
        }
        // I7: 한/영 전환 능동 통지 — 스크린리더(NVDA/Narrator)가 토글 즉시 낭독하도록
        // 이벤트를 발생시키고, 옵션이 켜져 있으면 차등 비프로 무시각 확인을 돕는다.
        self.announce_mode_change(is_korean, announce_beep);
    }

    /// 오버레이 폴백(자동교정 제한) 상태를 설정하고, 전이 시에만 랭바 툴팁 갱신
    /// (OnUpdate) + 스크린리더 능동 통지(NotifyWinEvent)를 발사한다.
    ///
    /// 매 폴백 키마다 호출돼도 값이 바뀔 때만 통지하므로(compare-and-swap) 과도
    /// 알림이 없다. 호출부(text_service)는 HWND 집합으로 로그를 앱당 1회로 더 조인다.
    pub fn set_atf_limited(&self, limited: bool) {
        let prev = self.atf_limited.swap(limited, Ordering::SeqCst);
        if prev == limited {
            return;
        }
        // 툴팁/설명 텍스트가 바뀌므로 랭바에 갱신을 알린다.
        if let Ok(guard) = self.sink.lock() {
            if let Some(ref sink) = *guard {
                unsafe {
                    let _ = sink.OnUpdate(TF_LBI_TEXT);
                }
            }
        }
        // 제한 진입 시 전경 창에 이름 변경 이벤트를 발생시켜 스크린리더가 즉시 낭독하게 한다.
        if limited {
            unsafe {
                let hwnd = GetForegroundWindow();
                if hwnd.0 as isize != 0 {
                    NotifyWinEvent(EVENT_OBJECT_NAMECHANGE, hwnd, OBJID_CLIENT.0, 0);
                }
            }
        }
    }

    /// I7: 한/영 모드 전환을 스크린리더/무시각 사용자에게 능동 통지한다.
    ///
    /// (a) `NotifyWinEvent(EVENT_OBJECT_STATECHANGE)` + `EVENT_OBJECT_NAMECHANGE` 를
    ///     전경 창의 클라이언트 영역에 발생시킨다. 랭바 인디케이터 항목의 접근성
    ///     객체는 OS(ctfmon)가 소유해 우리가 직접 HWND 를 못 잡으므로, 입력 포커스가
    ///     있는 전경 창을 대상으로 상태 변화를 알려 스크린리더가 즉시 갱신·낭독하게
    ///     한다. (수동 통지인 랭바 szDescription/툴팁과 달리 능동적.)
    /// (b) `announce_beep` 가 true 면 한/영 차등 비프음(한글=높은 음, 영문=낮은 음)
    ///     으로 무시각 확인을 제공한다. Beep 은 동기 블로킹이므로 입력 스레드 지연을
    ///     피하기 위해 별도 스레드에서 울린다.
    fn announce_mode_change(&self, is_korean: bool, announce_beep: bool) {
        unsafe {
            let hwnd = GetForegroundWindow();
            if hwnd.0 as isize != 0 {
                // idChild = CHILDID_SELF(0). OBJID_CLIENT 로 포커스 창의 클라이언트
                // 객체 상태/이름 변화를 통지.
                NotifyWinEvent(EVENT_OBJECT_STATECHANGE, hwnd, OBJID_CLIENT.0, 0);
                NotifyWinEvent(EVENT_OBJECT_NAMECHANGE, hwnd, OBJID_CLIENT.0, 0);
            }
        }
        if announce_beep {
            // 한글=높은 음(880Hz), 영문=낮은 음(440Hz), 각 90ms. 별도 스레드에서
            // 울려 TSF STA(키 입력) 스레드가 블로킹되지 않게 한다.
            let (freq, dur) = if is_korean { (880u32, 90u32) } else { (440u32, 90u32) };
            std::thread::spawn(move || unsafe {
                let _ = Beep(freq, dur);
            });
        }
    }

    /// AutoTypeFix 토글 단축키(전체/순방향/역방향)를 무시각 사용자에게 능동 통지한다.
    ///
    /// `announce_mode_change`(한/영)와 동일 패턴이나 비프 음역을 달리해(한/영 880/440Hz
    /// 와 구별) ATF 토글임을 청각적으로 구별하게 한다: 켜짐=고음(1318Hz)/꺼짐=저음
    /// (659Hz), 각 70ms. 세 종류(enabled/forward/reverse)는 on/off 차등만 두고 kind 별
    /// 구별은 하지 않는다(과도 신호 방지). `announce_beep` 는 `toggle_announce_beep`
    /// 설정을 존중한다(한/영 비프와 동일 게이트). Beep 은 별도 스레드에서 울려 TSF STA
    /// (키 입력) 스레드를 블로킹하지 않는다.
    pub fn announce_atf_toggle(&self, on: bool, announce_beep: bool) {
        unsafe {
            let hwnd = GetForegroundWindow();
            if hwnd.0 as isize != 0 {
                NotifyWinEvent(EVENT_OBJECT_STATECHANGE, hwnd, OBJID_CLIENT.0, 0);
                NotifyWinEvent(EVENT_OBJECT_NAMECHANGE, hwnd, OBJID_CLIENT.0, 0);
            }
        }
        if announce_beep {
            let (freq, dur) = if on { (1318u32, 70u32) } else { (659u32, 70u32) };
            std::thread::spawn(move || unsafe {
                let _ = Beep(freq, dur);
            });
        }
    }
}

// ── UnimLangBarButton ──────────────────────────────────────────────────────────

#[implement(ITfLangBarItemButton, ITfLangBarItem, ITfSource)]
pub struct UnimLangBarButton {
    /// text_service 와 공유하는 랭귀지바 상태 (is_korean 캐시 + sink).
    state: Arc<LangBarState>,
    /// 갭2: 랭귀지바 클릭 시 엔진을 직접 토글하기 위한 Arc 참조.
    engine: Arc<Mutex<InputEngine>>,
    /// 갭2: set_input_category 호출 시 필요한 Config.
    config: Arc<Mutex<Config>>,
    /// AdviseSink 쿠키 (UnadviseSink 용).
    sink_cookie: Mutex<u32>,
}

impl UnimLangBarButton {
    /// 엔진/설정 Arc 를 주입받아 생성. ActivateEx 에서 호출.
    pub fn new(
        state: Arc<LangBarState>,
        engine: Arc<Mutex<InputEngine>>,
        config: Arc<Mutex<Config>>,
    ) -> Self {
        Self {
            state,
            engine,
            config,
            sink_cookie: Mutex::new(0),
        }
    }
}

// ── ITfLangBarItem ──

impl ITfLangBarItem_Impl for UnimLangBarButton_Impl {
    fn GetInfo(&self, pinfo: *mut TF_LANGBARITEMINFO) -> Result<()> {
        unsafe {
            if !pinfo.is_null() {
                let mut desc = [0u16; 32];
                let text = if self.state.is_korean.load(Ordering::SeqCst) {
                    "한국어"
                } else {
                    "English"
                };
                for (i, c) in text.encode_utf16().enumerate() {
                    if i >= 31 {
                        break;
                    }
                    desc[i] = c;
                }

                *pinfo = TF_LANGBARITEMINFO {
                    clsidService: globals::UNIM_CLSID,
                    // 표준 입력 모드 인디케이터 GUID. 커스텀 GUID 를 쓰면 OS 가
                    // 본 항목을 "일반 버튼" 으로만 취급해 작업표시줄 시계 옆
                    // 한/영 표시기("가"/"A")를 그리지 않는다. GUID_LBI_INPUTMODE
                    // ({2C77A81E-41CC-4178-A3A7-5F8A987568E6}) 로 지정해야 OS·
                    // ctfmon 이 본 항목을 입력 모드 인디케이터로 인식해 트레이에
                    // 현재 모드를 그린다 (MS IME·SampleIME 와 동일).
                    guidItem: GUID_LBI_INPUTMODE,
                    // BTN_BUTTON: 좌클릭 → OnClick (한/영 토글).
                    // BTN_MENU: 우클릭 → OS 가 InitMenu 를 호출해 컨텍스트 메뉴를
                    //   띄운다. 이 플래그가 없으면 InitMenu 가 호출조차 안 돼
                    //   우클릭 메뉴가 안 뜬다.
                    // SHOWNINTRAY: Windows 10/11 트레이 IME 인디케이터에 노출.
                    dwStyle: TF_LBI_STYLE_BTN_BUTTON
                        | TF_LBI_STYLE_BTN_MENU
                        | TF_LBI_STYLE_SHOWNINTRAY,
                    ulSort: 0,
                    szDescription: desc,
                };
            }
        }
        Ok(())
    }

    fn GetStatus(&self) -> Result<u32> {
        Ok(0) // enabled
    }

    fn Show(&self, _fshow: BOOL) -> Result<()> {
        Ok(())
    }

    fn GetTooltipString(&self) -> Result<BSTR> {
        let text = if self.state.is_korean.load(Ordering::SeqCst) {
            "UNIM 한국어 입력"
        } else {
            "UNIM English Input"
        };
        // 오버레이 폴백 앱이면 자동교정이 조용히 꺼져 있음을 툴팁으로 고지한다.
        if self.state.atf_limited.load(Ordering::SeqCst) {
            return Ok(BSTR::from(format!("{text} — 이 앱은 자동교정 제한")));
        }
        Ok(BSTR::from(text))
    }
}

// ── ITfLangBarItemButton ──

impl ITfLangBarItemButton_Impl for UnimLangBarButton_Impl {
    fn OnClick(&self, click: TfLBIClick, pt: &POINT, _prcarea: *const RECT) -> Result<()> {
        // [B3 진단] Win11 모던 작업표시줄 트레이 IME 인디케이터가 우클릭을
        // OnClick(TF_LBI_CLK_RIGHT)로 실제 전달하는지 실증한다. 우클릭 시 본 줄이
        // 로그에 안 찍히면 = OS 셸이 OnClick 을 호출조차 안 함(가설 확정).
        // %TEMP%\unim-tsf.log 에서 "lang_bar OnClick" 항목으로 확인.
        crate::register::dbg_log(&format!(
            "lang_bar OnClick: click={} (LEFT={}, RIGHT={}) pt=({}, {})",
            click.0,
            TF_LBI_CLK_LEFT.0,
            TF_LBI_CLK_RIGHT.0,
            pt.x,
            pt.y,
        ));
        if click == TF_LBI_CLK_LEFT {
            // 갭2: 좌클릭 → 엔진을 직접 토글 + 랭귀지바 아이콘/텍스트 갱신.
            self.toggle_engine_mode();
        } else if click == TF_LBI_CLK_RIGHT {
            // 우클릭 → 컨텍스트 메뉴를 직접 띄운다.
            //
            // Win11 모던 작업표시줄의 트레이 IME 인디케이터는 우클릭을
            // OnClick(TF_LBI_CLK_RIGHT) 로만 전달하고 ITfLangBarItemButton::InitMenu
            // 를 호출하지 않는다. InitMenu 는 구형 플로팅 언어 바에서만 불린다.
            // 따라서 Weasel 과 동일하게 여기서 TrackPopupMenuEx 로 직접 메뉴를
            // 그려야 양쪽 환경(트레이/플로팅 바) 모두에서 우클릭 메뉴가 뜬다.
            self.show_context_menu(*pt);
        }
        Ok(())
    }

    fn InitMenu(&self, pmenu: Ref<'_, ITfMenu>) -> Result<()> {
        // [B3 진단] 레거시 플로팅 언어 바 경로. 이 줄이 로그에 찍히면 = OS 가
        // InitMenu 를 호출(우클릭 메뉴를 OS 가 그림). 트레이 환경에서 안 찍히고
        // OnClick(RIGHT)도 안 찍히면 = 우클릭이 OnClick/InitMenu 어느 쪽으로도
        // 안 옴(셸이 자체 메뉴를 가로챔 — 가설 확정).
        crate::register::dbg_log("lang_bar InitMenu: called (legacy floating langbar)");
        let Some(menu) = pmenu.as_ref() else { return Ok(()); };
        unsafe {
            // 메뉴 항목: 한/영 전환
            let toggle_text: Vec<u16> = "한/영 전환".encode_utf16().collect();
            let _ = menu.AddMenuItem(
                MENU_ID_TOGGLE,
                0,
                HBITMAP::default(),
                HBITMAP::default(),
                &toggle_text,
                std::ptr::null_mut(),
            );
            // 메뉴 항목: 기본 입력기로 설정
            let default_text: Vec<u16> = "기본 입력기로 설정".encode_utf16().collect();
            let _ = menu.AddMenuItem(
                MENU_ID_SET_DEFAULT,
                0,
                HBITMAP::default(),
                HBITMAP::default(),
                &default_text,
                std::ptr::null_mut(),
            );
            // 메뉴 항목: 설정 — 접근키 S ("설정(&S)" → Alt+S / 메뉴에서 S).
            let settings_text: Vec<u16> = "설정(&S)".encode_utf16().collect();
            let _ = menu.AddMenuItem(
                MENU_ID_SETTINGS,
                0,
                HBITMAP::default(),
                HBITMAP::default(),
                &settings_text,
                std::ptr::null_mut(),
            );
            // 메뉴 항목: 정보 — 접근키 I ("정보(&I)").
            let about_text: Vec<u16> = "정보(&I)".encode_utf16().collect();
            let _ = menu.AddMenuItem(
                MENU_ID_ABOUT,
                0,
                HBITMAP::default(),
                HBITMAP::default(),
                &about_text,
                std::ptr::null_mut(),
            );
        }
        Ok(())
    }

    fn OnMenuSelect(&self, wid: u32) -> Result<()> {
        // InitMenu(플로팅 언어 바) 경로. 트레이 우클릭은 show_context_menu 에서
        // 직접 처리하지만, 둘 다 동일한 handle_menu_command 로 분기한다.
        self.handle_menu_command(wid);
        Ok(())
    }

    fn GetIcon(&self) -> Result<HICON> {
        // 한/영 상태를 런타임에 그려 아이콘 반환.
        // SampleIME/Weasel 규약: 유효 HICON 이면 S_OK, NULL 이면 E_FAIL.
        // 과거엔 실패 시 NULL+Ok 를 반환했는데, TSF 가 "S_OK 인데 NULL"을 받아
        // 트레이에 빈/깨진 아이콘을 그릴 수 있어 규약대로 E_FAIL 로 바꾼다.
        let text = if self.state.is_korean.load(Ordering::SeqCst) {
            "한"
        } else {
            "A"
        };
        match create_status_icon(text) {
            Some(hicon) if !hicon.is_invalid() => Ok(hicon),
            _ => Err(E_FAIL.into()),
        }
    }

    fn GetText(&self) -> Result<BSTR> {
        let text = if self.state.is_korean.load(Ordering::SeqCst) {
            "가"
        } else {
            "A"
        };
        Ok(BSTR::from(text))
    }
}

impl UnimLangBarButton_Impl {
    /// 엔진 모드를 토글하고 랭귀지바를 갱신한다.
    ///
    /// - engine.lock() → set_input_category() (반대 카테고리)
    /// - state.update(new_is_korean) → OnUpdate 발사
    ///
    /// 호출 컨텍스트: TSF STA 스레드 (OnClick · OnMenuSelect).
    /// text_service 의 OnKeyDown 과 동일 스레드이므로 재진입 없음.
    fn toggle_engine_mode(&self) {
        let (new_is_korean, announce_beep) = {
            let mut eng = self.engine.lock().unwrap();
            let cfg = self.config.lock().unwrap();
            let current = eng.input_category();
            let next = if current == InputCategory::Korean {
                InputCategory::English
            } else {
                InputCategory::Korean
            };
            eng.set_input_category(next);
            let announce_beep = cfg.engine.toggle_announce_beep;
            drop(cfg); // config 참조를 명시적으로 해제
            (next == InputCategory::Korean, announce_beep)
        };
        self.state.update(new_is_korean, announce_beep);
    }

    /// 메뉴 항목 선택을 처리한다. InitMenu/OnMenuSelect(플로팅 바) 경로와
    /// show_context_menu(트레이 우클릭) 경로가 모두 이 한 곳으로 모인다.
    fn handle_menu_command(&self, wid: u32) {
        match wid {
            MENU_ID_TOGGLE => {
                // 갭2: 메뉴 한/영 전환 → 엔진 직접 토글.
                self.toggle_engine_mode();
            }
            MENU_ID_SET_DEFAULT => {
                let _ = crate::register::set_as_default();
            }
            MENU_ID_SETTINGS => {
                // 새 Slint 설정 GUI(unim-settings.exe)를 실행. 실패(exe 부재 등)
                // 시에만 옛 내장 Win32 다이얼로그로 폴백한다.
                if !crate::register::launch_settings_app() {
                    let hwnd_parent = unsafe { GetForegroundWindow() };
                    crate::settings_dialog::show_settings_dialog(hwnd_parent);
                }
            }
            MENU_ID_ABOUT => {
                let hwnd_parent = unsafe { GetForegroundWindow() };
                show_about_dialog(hwnd_parent);
            }
            _ => {}
        }
    }

    /// 우클릭 컨텍스트 메뉴를 TrackPopupMenuEx 로 직접 띄운다 (Win11 트레이용).
    ///
    /// Win11 트레이는 InitMenu 를 호출하지 않으므로 OnClick(우클릭)에서 직접
    /// HMENU 를 만들어 표시한다. TPM_RETURNCMD 로 선택 ID 를 동기 반환받아
    /// handle_menu_command 로 분기한다. 메뉴 표시 전 SetForegroundWindow 를
    /// 호출해야 클릭-아웃 시 메뉴가 정상적으로 닫힌다(Win32 규약).
    fn show_context_menu(&self, pt: POINT) {
        // [B3 진단] OnClick(RIGHT)에서 진입한 직접 메뉴 경로. 이 줄이 찍히는데
        // 메뉴가 안 뜨면 = OnClick 은 불리지만 TrackPopupMenuEx 표시/dismiss 가
        // 깨진 것(owner/foreground 문제). 아래 owner/cmd 로그로 추가 판별.
        crate::register::dbg_log("lang_bar show_context_menu: enter (OnClick RIGHT path)");
        unsafe {
            let Ok(hmenu) = CreatePopupMenu() else {
                crate::register::dbg_log("lang_bar show_context_menu: CreatePopupMenu FAILED");
                return;
            };

            let add = |id: u32, label: &str| {
                let w: Vec<u16> = label.encode_utf16().chain(std::iter::once(0)).collect();
                let _ = AppendMenuW(hmenu, MF_STRING, id as usize, PCWSTR(w.as_ptr()));
            };
            add(MENU_ID_TOGGLE, "한/영 전환");
            add(MENU_ID_SET_DEFAULT, "기본 입력기로 설정");
            add(MENU_ID_SETTINGS, "설정(&S)");
            add(MENU_ID_ABOUT, "정보(&I)");

            // 메뉴 소유 창. TrackPopupMenuEx 는 NULL owner 로는 메뉴를 표시하지
            // 않으므로 반드시 유효한 HWND 가 필요하다.
            //
            // GetForegroundWindow() 는 트레이 IME 인디케이터 우클릭 시점에 전경 창이
            // 없으면 HWND(0)(NULL)을 반환한다. windows-rs 의 HWND::is_invalid() 는
            // HWND(-1)(INVALID_HANDLE_VALUE)만 거르고 HWND(0)은 유효로 판단하므로,
            // 명시적으로 .0 포인터가 null 인지 검사해야 한다.
            let fg = GetForegroundWindow();
            let owner = if fg.0 as isize != 0 {
                // 유효한 전경 창이 있으면 그것을 owner 로 사용 + 전경으로 올림
                let _ = SetForegroundWindow(fg);
                fg
            } else {
                // 전경 창 없음(트레이 우클릭 시 흔한 상황) → helper 창을 owner 로 사용.
                // helper 창 자체를 전경으로 올려야 dismiss(클릭 아웃 닫기)가 동작한다.
                let h = self.state.get_or_create_helper_hwnd();
                if h.0 as isize != 0 {
                    let _ = SetForegroundWindow(h);
                }
                h
            };

            crate::register::dbg_log(&format!(
                "lang_bar show_context_menu: owner=0x{:X} (fg=0x{:X}) pt=({}, {})",
                owner.0 as isize, fg.0 as isize, pt.x, pt.y,
            ));

            // TPM_RETURNCMD: 선택된 항목 ID 를 i32 로 동기 반환. 0 이면 취소.
            let cmd = TrackPopupMenuEx(
                hmenu,
                (TPM_RETURNCMD | TPM_NONOTIFY | TPM_LEFTALIGN | TPM_TOPALIGN).0,
                pt.x,
                pt.y,
                owner,
                core::ptr::null(),
            );

            // TrackPopupMenuEx 반환 직후 WM_NULL 을 owner 에 PostMessage 해
            // dismiss 처리를 완료시킨다 (Win32 규약).
            if owner.0 as isize != 0 {
                let _ = PostMessageW(Some(owner), WM_NULL, WPARAM(0), LPARAM(0));
            }

            let _ = DestroyMenu(hmenu);

            crate::register::dbg_log(&format!(
                "lang_bar show_context_menu: TrackPopupMenuEx returned cmd={} (0=cancel/dismiss)",
                cmd,
            ));

            if cmd != 0 {
                self.handle_menu_command(cmd as u32);
            }
        }
    }
}

// ── ITfSource (싱크 관리) ──

impl ITfSource_Impl for UnimLangBarButton_Impl {
    fn AdviseSink(&self, riid: *const GUID, punk: Ref<'_, IUnknown>) -> Result<u32> {
        unsafe {
            if riid.is_null() || punk.is_null() {
                return Err(E_INVALIDARG.into());
            }
            if *riid != ITfLangBarItemSink::IID {
                return Err(windows::core::HRESULT(0x80040202_u32 as i32).into());
            }
            let sink: ITfLangBarItemSink = punk.unwrap().cast()?;
            *self.state.sink.lock().unwrap() = Some(sink);
            let cookie = 1u32;
            *self.sink_cookie.lock().unwrap() = cookie;
            Ok(cookie)
        }
    }

    fn UnadviseSink(&self, dwcookie: u32) -> Result<()> {
        if dwcookie == *self.sink_cookie.lock().unwrap() {
            *self.state.sink.lock().unwrap() = None;
            Ok(())
        } else {
            Err(windows::core::HRESULT(0x80040200_u32 as i32).into())
        }
    }
}
