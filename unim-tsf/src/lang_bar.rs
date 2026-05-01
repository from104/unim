//! 언어 바 아이템 — 한/영 모드 표시 버튼 (ITfLangBarItemButton)

use std::sync::Mutex;

use windows::core::*;
use windows::Win32::Foundation::*;
use windows::Win32::UI::TextServices::*;
use windows::Win32::UI::WindowsAndMessaging::HICON;

use crate::globals;

#[implement(ITfLangBarItemButton, ITfLangBarItem, ITfSource)]
pub struct UnimLangBarButton {
    is_korean: Mutex<bool>,
    sink: Mutex<Option<ITfLangBarItemSink>>,
    sink_cookie: Mutex<u32>,
}

impl UnimLangBarButton {
    pub fn new() -> Self {
        Self {
            is_korean: Mutex::new(true),
            sink: Mutex::new(None),
            sink_cookie: Mutex::new(0),
        }
    }

    pub fn set_mode(&self, is_korean: bool) {
        *self.is_korean.lock().unwrap() = is_korean;
        if let Some(ref sink) = *self.sink.lock().unwrap() {
            unsafe {
                let _ = sink.OnUpdate(TF_LBI_STATUS | TF_LBI_ICON | TF_LBI_TEXT);
            }
        }
    }
}

// ── ITfLangBarItem ──

impl ITfLangBarItem_Impl for UnimLangBarButton_Impl {
    fn GetInfo(&self, pinfo: *mut TF_LANGBARITEMINFO) -> Result<()> {
        unsafe {
            if !pinfo.is_null() {
                let mut desc = [0u16; 32];
                let text = if *self.is_korean.lock().unwrap() {
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
        let text = if *self.is_korean.lock().unwrap() {
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
        let mut is_korean = self.is_korean.lock().unwrap();
        *is_korean = !*is_korean;
        drop(is_korean);

        if let Some(ref sink) = *self.sink.lock().unwrap() {
            unsafe {
                let _ = sink.OnUpdate(TF_LBI_STATUS | TF_LBI_ICON | TF_LBI_TEXT);
            }
        }
        Ok(())
    }

    fn InitMenu(&self, _pmenu: Option<&ITfMenu>) -> Result<()> {
        Ok(())
    }

    fn OnMenuSelect(&self, _wid: u32) -> Result<()> {
        Ok(())
    }

    fn GetIcon(&self) -> Result<HICON> {
        Ok(HICON::default())
    }

    fn GetText(&self) -> Result<BSTR> {
        let text = if *self.is_korean.lock().unwrap() {
            "가"
        } else {
            "A"
        };
        Ok(BSTR::from(text))
    }
}

// ── ITfSource (싱크 관리) ──

impl ITfSource_Impl for UnimLangBarButton_Impl {
    fn AdviseSink(&self, riid: *const GUID, punk: Option<&IUnknown>) -> Result<u32> {
        unsafe {
            if riid.is_null() || punk.is_none() {
                return Err(E_INVALIDARG.into());
            }
            if *riid != ITfLangBarItemSink::IID {
                return Err(windows::core::HRESULT(0x80040202_u32 as i32).into());
            }
            let sink: ITfLangBarItemSink = punk.unwrap().cast()?;
            *self.sink.lock().unwrap() = Some(sink);
            let cookie = 1u32;
            *self.sink_cookie.lock().unwrap() = cookie;
            Ok(cookie)
        }
    }

    fn UnadviseSink(&self, dwcookie: u32) -> Result<()> {
        if dwcookie == *self.sink_cookie.lock().unwrap() {
            *self.sink.lock().unwrap() = None;
            Ok(())
        } else {
            Err(windows::core::HRESULT(0x80040200_u32 as i32).into())
        }
    }
}
