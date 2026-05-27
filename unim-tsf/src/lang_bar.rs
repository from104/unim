//! 언어 바 아이템 — 한/영 모드 표시 버튼 (ITfLangBarItemButton)
//!
//! 갭1(엔진→langbar) · 갭2(langbar→엔진) 를 모두 닫는 구현.
//! LangBarState (Arc 공유) 를 통해 text_service 와 UnimLangBarButton 이
//! 동일 is_korean 플래그와 ITfLangBarItemSink 를 참조한다.

use std::sync::{Arc, Mutex};
use std::sync::atomic::{AtomicBool, Ordering};

use windows::core::*;
use windows::Win32::Foundation::*;
use windows::Win32::Graphics::Gdi::HBITMAP;
use windows::Win32::UI::TextServices::*;
use windows::Win32::UI::WindowsAndMessaging::{GetForegroundWindow, HICON};

use unim::config::{Config, InputCategory};
use unim::input_engine::InputEngine;

use crate::globals;

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
}

// SAFETY: ITfLangBarItemSink 는 TSF STA 스레드에서만 접근하며,
// Mutex 로 보호하므로 Send/Sync 를 수동 구현.
unsafe impl Send for LangBarState {}
unsafe impl Sync for LangBarState {}

impl LangBarState {
    pub fn new(is_korean: bool) -> Arc<Self> {
        Arc::new(Self {
            is_korean: AtomicBool::new(is_korean),
            sink: Mutex::new(None),
        })
    }

    /// is_korean 을 갱신하고, 등록된 sink 가 있으면 OnUpdate 를 발사한다.
    pub fn update(&self, is_korean: bool) {
        self.is_korean.store(is_korean, Ordering::SeqCst);
        if let Ok(guard) = self.sink.lock() {
            if let Some(ref sink) = *guard {
                unsafe {
                    let _ = sink.OnUpdate(TF_LBI_STATUS | TF_LBI_ICON | TF_LBI_TEXT);
                }
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
                    guidItem: globals::UNIM_LANGBAR_ITEM_GUID,
                    dwStyle: 0x00020000, // TF_LBI_STYLE_BTN_BUTTON
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
        Ok(HICON::default())
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
