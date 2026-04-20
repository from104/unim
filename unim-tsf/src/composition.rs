//! TSF 조합(Composition) 관리

use std::sync::{Arc, Mutex};

use windows::Win32::UI::TextServices::*;
use windows::core::*;

/// 조합 상태 관리자
pub struct CompositionManager {
    composition: Option<ITfComposition>,
    composition_slot: Arc<Mutex<Option<ITfComposition>>>,
}

impl CompositionManager {
    pub fn new() -> Self {
        Self {
            composition: None,
            composition_slot: Arc::new(Mutex::new(None)),
        }
    }

    pub fn is_active(&self) -> bool { self.composition.is_some() }
    pub fn clear(&mut self) { self.composition = None; }

    pub fn start_composition(
        &mut self,
        context: &ITfContext,
        tid: u32,
        text: &str,
        comp_sink: &ITfCompositionSink,
    ) {
        let slot = self.composition_slot.clone();
        *slot.lock().unwrap() = None;

        let session = StartCompositionEditSession {
            context: context.clone(),
            text: text.to_string(),
            comp_sink: comp_sink.clone(),
            composition_slot: slot.clone(),
        };
        let session_intf: ITfEditSession = session.into();
        unsafe {
            let _ = context.RequestEditSession(tid, &session_intf, TF_ES_READWRITE | TF_ES_SYNC);
        }

        let result = self.composition_slot.lock().unwrap().take();
        if let Some(comp) = result {
            self.composition = Some(comp);
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
        let session = InsertTextEditSession {
            context: context.clone(),
            text: text.to_string(),
        };
        let session_intf: ITfEditSession = session.into();
        unsafe {
            let _ = context.RequestEditSession(tid, &session_intf, TF_ES_READWRITE | TF_ES_SYNC);
        }
    }
}

// ── EditSession: 조합 시작 ──

#[implement(ITfEditSession)]
struct StartCompositionEditSession {
    context: ITfContext,
    text: String,
    comp_sink: ITfCompositionSink,
    composition_slot: Arc<Mutex<Option<ITfComposition>>>,
}

impl ITfEditSession_Impl for StartCompositionEditSession_Impl {
    fn DoEditSession(&self, ec: u32) -> Result<()> {
        unsafe {
            let wide: Vec<u16> = self.text.encode_utf16().collect();
            let insert: ITfInsertAtSelection = self.context.cast()?;
            let range = insert.InsertTextAtSelection(ec, TF_IAS_QUERYONLY, &[])?;
            range.SetText(ec, 0, &wide)?;

            let ctx_comp: ITfContextComposition = self.context.cast()?;
            let composition = ctx_comp.StartComposition(ec, &range, &self.comp_sink)?;
            *self.composition_slot.lock().unwrap() = Some(composition);
        }
        Ok(())
    }
}

// ── EditSession: 조합 갱신 ──

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

// ── EditSession: 조합 종료 ──

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

// ── EditSession: 텍스트 삽입 ──

#[implement(ITfEditSession)]
struct InsertTextEditSession {
    context: ITfContext,
    text: String,
}

impl ITfEditSession_Impl for InsertTextEditSession_Impl {
    fn DoEditSession(&self, ec: u32) -> Result<()> {
        unsafe {
            let wide: Vec<u16> = self.text.encode_utf16().collect();
            let insert: ITfInsertAtSelection = self.context.cast()?;
            let range = insert.InsertTextAtSelection(ec, TF_IAS_QUERYONLY, &[])?;
            range.SetText(ec, 0, &wide)?;
        }
        Ok(())
    }
}
