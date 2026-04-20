//! TSF 조합(Composition) 관리

use windows::Win32::UI::TextServices::*;
use windows::core::*;

/// 조합 상태 관리자
pub struct CompositionManager {
    composition: Option<ITfComposition>,
}

impl CompositionManager {
    pub fn new() -> Self { Self { composition: None } }
    pub fn is_active(&self) -> bool { self.composition.is_some() }
    pub fn clear(&mut self) { self.composition = None; }

    pub fn start_composition(&mut self, context: &ITfContext, tid: u32, text: &str) {
        let session = StartCompositionEditSession { text: text.to_string(), tid };
        let session_intf: ITfEditSession = session.into();
        unsafe {
            let _ = context.RequestEditSession(tid, &session_intf, TF_ES_READWRITE | TF_ES_SYNC);
        }
    }

    pub fn update_composition(&self, context: &ITfContext, tid: u32, text: &str) {
        if let Some(ref composition) = self.composition {
            let session = UpdateCompositionEditSession {
                text: text.to_string(),
                composition: composition.clone(),
            };
            let session_intf: ITfEditSession = session.into();
            unsafe {
                let _ = context.RequestEditSession(tid, &session_intf, TF_ES_READWRITE | TF_ES_SYNC);
            }
        }
    }

    pub fn end_composition_with_text(&mut self, context: &ITfContext, tid: u32, text: &str) {
        if let Some(ref composition) = self.composition {
            let session = EndCompositionEditSession {
                text: Some(text.to_string()),
                composition: composition.clone(),
            };
            let session_intf: ITfEditSession = session.into();
            unsafe {
                let _ = context.RequestEditSession(tid, &session_intf, TF_ES_READWRITE | TF_ES_SYNC);
            }
        }
        self.composition = None;
    }

    pub fn end_composition(&mut self, context: &ITfContext, tid: u32) {
        if let Some(ref composition) = self.composition {
            let session = EndCompositionEditSession {
                text: None,
                composition: composition.clone(),
            };
            let session_intf: ITfEditSession = session.into();
            unsafe {
                let _ = context.RequestEditSession(tid, &session_intf, TF_ES_READWRITE | TF_ES_SYNC);
            }
        }
        self.composition = None;
    }

    pub fn insert_text(&self, context: &ITfContext, tid: u32, text: &str) {
        let session = InsertTextEditSession { text: text.to_string() };
        let session_intf: ITfEditSession = session.into();
        unsafe {
            let _ = context.RequestEditSession(tid, &session_intf, TF_ES_READWRITE | TF_ES_SYNC);
        }
    }
}

// ── EditSession 구현 ──

#[implement(ITfEditSession)]
struct StartCompositionEditSession {
    text: String,
    tid: u32,
}

impl ITfEditSession_Impl for StartCompositionEditSession_Impl {
    fn DoEditSession(&self, _ec: u32) -> Result<()> {
        let _ = (&self.text, self.tid);
        Ok(())
    }
}

#[implement(ITfEditSession)]
struct UpdateCompositionEditSession {
    text: String,
    composition: ITfComposition,
}

impl ITfEditSession_Impl for UpdateCompositionEditSession_Impl {
    fn DoEditSession(&self, ec: u32) -> Result<()> {
        unsafe {
            let range = self.composition.GetRange()?;
            let wide: Vec<u16> = self.text.encode_utf16().collect();
            range.SetText(ec, 0, &wide)?;
        }
        Ok(())
    }
}

#[implement(ITfEditSession)]
struct EndCompositionEditSession {
    text: Option<String>,
    composition: ITfComposition,
}

impl ITfEditSession_Impl for EndCompositionEditSession_Impl {
    fn DoEditSession(&self, ec: u32) -> Result<()> {
        unsafe {
            if let Some(ref text) = self.text {
                let range = self.composition.GetRange()?;
                let wide: Vec<u16> = text.encode_utf16().collect();
                range.SetText(ec, 0, &wide)?;
            }
            self.composition.EndComposition(ec)?;
        }
        Ok(())
    }
}

#[implement(ITfEditSession)]
struct InsertTextEditSession {
    text: String,
}

impl ITfEditSession_Impl for InsertTextEditSession_Impl {
    fn DoEditSession(&self, _ec: u32) -> Result<()> {
        let _ = &self.text;
        Ok(())
    }
}
