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
    CreateBitmap, CreateCompatibleBitmap, CreateCompatibleDC, DeleteDC, DeleteObject, GetDC,
    GetStockObject, PatBlt, ReleaseDC, SelectObject, SetBkMode, SetTextColor, TextOutW, BLACKNESS,
    DEFAULT_GUI_FONT, HBITMAP, HGDIOBJ, TRANSPARENT, WHITENESS,
};
use windows::Win32::UI::TextServices::*;
use windows::Win32::UI::WindowsAndMessaging::{
    CreateIconIndirect, GetForegroundWindow, GetSystemMetrics, HICON, ICONINFO, SM_CXSMICON,
    SM_CYSMICON,
};

use unim::config::{Config, InputCategory};
use unim::input_engine::InputEngine;

use crate::globals;

/// 한/영 상태 텍스트("한"/"A")를 GDI 로 그려 작은 트레이용 HICON 을 만든다.
///
/// .ico 리소스를 DLL 에 임베드하지 않고 런타임에 그려, 현재 입력 상태를 반영한다.
/// 어떤 단계든 실패하면 `None` 을 돌려주고 호출자(GetIcon)는 NULL HICON 으로
/// 폴백한다 — 절대 패닉/크래시하지 않는다. 반환 HICON 의 소유권은 OS 에 있다
/// (ITfLangBarItemButton::GetIcon 계약 — OS 가 DestroyIcon 호출).
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
        // 1bpp AND 마스크. 전부 0 으로 채우면 아이콘 전체가 불투명.
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

        // ── 마스크를 전부 0(불투명) 으로 초기화 ──
        let mask_dc = CreateCompatibleDC(Some(screen_dc));
        if !mask_dc.is_invalid() {
            let old = SelectObject(mask_dc, HGDIOBJ(mask_bmp.0));
            let _ = PatBlt(mask_dc, 0, 0, cx, cy, BLACKNESS);
            SelectObject(mask_dc, old);
            let _ = DeleteDC(mask_dc);
        }

        // ── 컬러 비트맵에 배경(흰색) + 텍스트(검정) ──
        let old_bmp = SelectObject(mem_dc, HGDIOBJ(color_bmp.0));
        let _ = PatBlt(mem_dc, 0, 0, cx, cy, WHITENESS);
        let old_font = SelectObject(mem_dc, GetStockObject(DEFAULT_GUI_FONT));
        SetBkMode(mem_dc, TRANSPARENT);
        SetTextColor(mem_dc, COLORREF(0x0000_0000));
        let wide: Vec<u16> = text.encode_utf16().collect();
        // 작은 아이콘이라 정밀 측정 없이 대략 가운데로 오프셋.
        let x = (cx / 2 - cx / 4).max(0);
        let y = (cy / 2 - cy / 3).max(0);
        let _ = TextOutW(mem_dc, x, y, &wide);
        SelectObject(mem_dc, old_font);
        SelectObject(mem_dc, old_bmp);

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

// 메뉴 항목 ID
const MENU_ID_TOGGLE: u32 = 0;
const MENU_ID_SET_DEFAULT: u32 = 1;
const MENU_ID_SETTINGS: u32 = 2;

// ── 공유 상태 ──────────────────────────────────────────────────────────────────

/// text_service 와 UnimLangBarButton 이 Arc 로 공유하는 랭귀지바 상태.
///
/// - `is_korean`: 엔진 모드의 캐시. text_service(OnKeyDown 후) 와
///   UnimLangBarButton(OnClick/OnMenuSelect) 양쪽에서 갱신.
/// - `sink`: Windows 가 AdviseSink 로 등록한 ITfLangBarItemSink.
///   갱신 시 OnUpdate() 를 발사해 랭귀지바에 변화를 알린다.
pub struct LangBarState {
    pub is_korean: AtomicBool,
    pub sink: Mutex<Option<ITfLangBarItemSink>>,
    /// ActivateEx 에서 주입되는 thread_mgr + client id.
    /// `update()` 가 OS 입력 표시기 compartment 를 동기화할 때 사용한다.
    /// 키 입력 경로(OnKeyDown)와 랭귀지바 클릭 경로(toggle_engine_mode) 양쪽이
    /// 모두 `update()` 를 거치므로, compartment 동기화를 여기 한 곳에 모은다.
    tsf: Mutex<Option<(ITfThreadMgr, u32)>>,
}

// SAFETY: ITfLangBarItemSink / ITfThreadMgr 는 TSF STA 스레드에서만 접근하며,
// Mutex 로 보호하므로 Send/Sync 를 수동 구현.
unsafe impl Send for LangBarState {}
unsafe impl Sync for LangBarState {}

impl LangBarState {
    pub fn new(is_korean: bool) -> Arc<Self> {
        Arc::new(Self {
            is_korean: AtomicBool::new(is_korean),
            sink: Mutex::new(None),
            tsf: Mutex::new(None),
        })
    }

    /// ActivateEx 에서 thread_mgr + client id 를 주입한다.
    /// 이후 `update()` 호출 시 compartment 동기화가 활성화된다.
    pub fn set_tsf(&self, thread_mgr: ITfThreadMgr, tid: u32) {
        *self.tsf.lock().unwrap() = Some((thread_mgr, tid));
    }

    /// is_korean 을 갱신하고, sink OnUpdate 발사 + OS compartment 동기화를 한다.
    ///
    /// OnKeyDown(엔진 토글)·toggle_engine_mode(랭귀지바 클릭) 양쪽이 이 한 곳을
    /// 거치므로, 모드 변경 시 랭귀지바와 OS 입력 표시기가 항상 함께 갱신된다.
    pub fn update(&self, is_korean: bool) {
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
                    // SampleIME 표준: BTN_BUTTON | SHOWNINTRAY.
                    // SHOWNINTRAY 플래그는 Windows 10/11 트레이 IME 인디케이터에
                    // 본 항목을 노출시킨다.
                    dwStyle: TF_LBI_STYLE_BTN_BUTTON | TF_LBI_STYLE_SHOWNINTRAY,
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
        Ok(BSTR::from(text))
    }
}

// ── ITfLangBarItemButton ──

impl ITfLangBarItemButton_Impl for UnimLangBarButton_Impl {
    fn OnClick(&self, _click: TfLBIClick, _pt: &POINT, _prcarea: *const RECT) -> Result<()> {
        // 갭2: 좌클릭 → 엔진을 직접 토글 + 랭귀지바 아이콘/텍스트 갱신.
        self.toggle_engine_mode();
        Ok(())
    }

    fn InitMenu(&self, pmenu: Ref<'_, ITfMenu>) -> Result<()> {
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
            // 메뉴 항목: 설정 열기
            let settings_text: Vec<u16> = "설정 열기".encode_utf16().collect();
            let _ = menu.AddMenuItem(
                MENU_ID_SETTINGS,
                0,
                HBITMAP::default(),
                HBITMAP::default(),
                &settings_text,
                std::ptr::null_mut(),
            );
        }
        Ok(())
    }

    fn OnMenuSelect(&self, wid: u32) -> Result<()> {
        match wid {
            MENU_ID_TOGGLE => {
                // 갭2: 메뉴 한/영 전환 → 엔진 직접 토글.
                self.toggle_engine_mode();
            }
            MENU_ID_SET_DEFAULT => {
                let _ = crate::register::set_as_default();
            }
            MENU_ID_SETTINGS => {
                let hwnd_parent = unsafe { GetForegroundWindow() };
                crate::settings_dialog::show_settings_dialog(hwnd_parent);
            }
            _ => {}
        }
        Ok(())
    }

    fn GetIcon(&self) -> Result<HICON> {
        // 한/영 상태를 런타임에 그려 아이콘 반환. 실패 시 NULL 폴백.
        let text = if self.state.is_korean.load(Ordering::SeqCst) {
            "한"
        } else {
            "A"
        };
        Ok(create_status_icon(text).unwrap_or_default())
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
        let new_is_korean = {
            let mut eng = self.engine.lock().unwrap();
            let cfg = self.config.lock().unwrap();
            let current = eng.input_category();
            let next = if current == InputCategory::Korean {
                InputCategory::English
            } else {
                InputCategory::Korean
            };
            eng.set_input_category(next);
            drop(cfg); // config 참조를 명시적으로 해제
            next == InputCategory::Korean
        };
        self.state.update(new_is_korean);
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
