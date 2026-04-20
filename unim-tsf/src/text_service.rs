//! UnimTextService — TSF 메인 구조체

use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Mutex;

use windows::Win32::Foundation::*;
use windows::Win32::UI::TextServices::*;
use windows::core::*;

use unim::config::Config;
use unim::input_engine::InputEngine;

use crate::composition::CompositionManager;
use crate::key_handler;

#[implement(
    ITfTextInputProcessorEx,
    ITfKeyEventSink,
    ITfCompositionSink,
    ITfThreadMgrEventSink,
    ITfTextEditSink,
    ITfDisplayAttributeProvider,
)]
pub struct UnimTextService {
    pub(crate) thread_mgr: Mutex<Option<ITfThreadMgr>>,
    pub(crate) client_id: AtomicU32,
    pub(crate) engine: Mutex<InputEngine>,
    pub(crate) config: Mutex<Config>,
    pub(crate) composition_mgr: Mutex<CompositionManager>,
    pub(crate) key_event_sink_installed: Mutex<bool>,
    pub(crate) thread_mgr_sink_cookie: Mutex<Option<u32>>,
}

impl UnimTextService {
    pub fn new() -> Self {
        let config = Config::load_from_default_path();
        let engine = InputEngine::new(&config);
        Self {
            thread_mgr: Mutex::new(None),
            client_id: AtomicU32::new(0),
            engine: Mutex::new(engine),
            config: Mutex::new(config),
            composition_mgr: Mutex::new(CompositionManager::new()),
            key_event_sink_installed: Mutex::new(false),
            thread_mgr_sink_cookie: Mutex::new(None),
        }
    }

    pub fn client_id(&self) -> u32 {
        self.client_id.load(Ordering::SeqCst)
    }
}

// ── ITfTextInputProcessorEx ──

impl ITfTextInputProcessorEx_Impl for UnimTextService_Impl {
    fn ActivateEx(
        &self,
        ptim: Option<&ITfThreadMgr>,
        tid: u32,
        _dwflags: u32,
    ) -> Result<()> {
        let thread_mgr = ptim.ok_or(E_INVALIDARG)?;
        self.client_id.store(tid, Ordering::SeqCst);
        *self.thread_mgr.lock().unwrap() = Some(thread_mgr.clone());

        unsafe {
            let keystroke_mgr: ITfKeystrokeMgr = thread_mgr.cast()?;
            let this_sink: ITfKeyEventSink = self.cast()?;
            keystroke_mgr.AdviseKeyEventSink(tid, &this_sink, TRUE)?;
            *self.key_event_sink_installed.lock().unwrap() = true;
        }

        unsafe {
            let source: ITfSource = thread_mgr.cast()?;
            let this_sink: ITfThreadMgrEventSink = self.cast()?;
            let cookie = source.AdviseSink(&ITfThreadMgrEventSink::IID, &this_sink)?;
            *self.thread_mgr_sink_cookie.lock().unwrap() = Some(cookie);
        }

        // PreservedKey 등록 (한/영 전환, 한자)
        unsafe {
            let keystroke_mgr: ITfKeystrokeMgr = thread_mgr.cast()?;
            // Hangul key
            let hangul_key = TF_PRESERVEDKEY {
                uVKey: 0x15, // VK_HANGUL
                uModifiers: 0,
            };
            let desc_hangul: Vec<u16> = "Toggle Korean/English"
                .encode_utf16()
                .collect();
            let _ = keystroke_mgr.PreserveKey(
                tid,
                &crate::globals::UNIM_CLSID,
                &hangul_key,
                &desc_hangul,
            );
            // Hanja key
            let hanja_key = TF_PRESERVEDKEY {
                uVKey: 0x19, // VK_HANJA
                uModifiers: 0,
            };
            let desc_hanja: Vec<u16> = "Hanja conversion"
                .encode_utf16()
                .collect();
            let _ = keystroke_mgr.PreserveKey(
                tid,
                &crate::globals::UNIM_CLSID,
                &hanja_key,
                &desc_hanja,
            );
        }

        crate::dll_add_ref();
        Ok(())
    }
}

impl ITfTextInputProcessor_Impl for UnimTextService_Impl {
    fn Activate(&self, ptim: Option<&ITfThreadMgr>, tid: u32) -> Result<()> {
        self.ActivateEx(ptim, tid, 0)
    }

    fn Deactivate(&self) -> Result<()> {
        let thread_mgr_guard = self.thread_mgr.lock().unwrap();
        if let Some(ref thread_mgr) = *thread_mgr_guard {
            let tid = self.client_id();

            if *self.key_event_sink_installed.lock().unwrap() {
                unsafe {
                    if let Ok(keystroke_mgr) = thread_mgr.cast::<ITfKeystrokeMgr>() {
                        let _ = keystroke_mgr.UnadviseKeyEventSink(tid);
                    }
                }
                *self.key_event_sink_installed.lock().unwrap() = false;
            }

            if let Some(cookie) = self.thread_mgr_sink_cookie.lock().unwrap().take() {
                unsafe {
                    if let Ok(source) = thread_mgr.cast::<ITfSource>() {
                        let _ = source.UnadviseSink(cookie);
                    }
                }
            }
        }
        drop(thread_mgr_guard);
        *self.thread_mgr.lock().unwrap() = None;
        crate::dll_release();
        Ok(())
    }
}

// ── ITfKeyEventSink ──
// v0.58: 출력 파라미터(pfEaten)가 반환값으로 변경되지 않고 raw pointer로 유지

impl ITfKeyEventSink_Impl for UnimTextService_Impl {
    fn OnSetFocus(&self, _fforeground: BOOL) -> Result<()> {
        Ok(())
    }

    fn OnTestKeyDown(
        &self,
        pic: Option<&ITfContext>,
        wparam: WPARAM,
        _lparam: LPARAM,
    ) -> Result<BOOL> {
        let engine = self.engine.lock().unwrap();
        let config = self.config.lock().unwrap();
        let eaten = key_handler::test_key_down(&engine, &config, wparam, pic);
        Ok(BOOL::from(eaten))
    }

    fn OnKeyDown(
        &self,
        pic: Option<&ITfContext>,
        wparam: WPARAM,
        _lparam: LPARAM,
    ) -> Result<BOOL> {
        let context = pic.ok_or(E_INVALIDARG)?;
        let mut engine = self.engine.lock().unwrap();
        let config = self.config.lock().unwrap();
        let mut comp_mgr = self.composition_mgr.lock().unwrap();
        let tid = self.client_id();
        let comp_sink: ITfCompositionSink = unsafe { self.cast()? };

        let eaten = key_handler::handle_key_down(
            &mut engine, &config, &mut comp_mgr, context, tid, wparam, &comp_sink,
        );
        Ok(BOOL::from(eaten))
    }

    fn OnTestKeyUp(
        &self,
        _pic: Option<&ITfContext>,
        _wparam: WPARAM,
        _lparam: LPARAM,
    ) -> Result<BOOL> {
        Ok(FALSE)
    }

    fn OnKeyUp(
        &self,
        _pic: Option<&ITfContext>,
        _wparam: WPARAM,
        _lparam: LPARAM,
    ) -> Result<BOOL> {
        Ok(FALSE)
    }

    fn OnPreservedKey(
        &self,
        _pic: Option<&ITfContext>,
        _rguid: *const GUID,
    ) -> Result<BOOL> {
        Ok(FALSE)
    }
}

// ── ITfCompositionSink ──

impl ITfCompositionSink_Impl for UnimTextService_Impl {
    fn OnCompositionTerminated(
        &self,
        _ecwrite: u32,
        _pcomposition: Option<&ITfComposition>,
    ) -> Result<()> {
        self.composition_mgr.lock().unwrap().clear();
        self.engine.lock().unwrap().reset();
        Ok(())
    }
}

// ── ITfThreadMgrEventSink ──

impl ITfThreadMgrEventSink_Impl for UnimTextService_Impl {
    fn OnInitDocumentMgr(&self, _pdim: Option<&ITfDocumentMgr>) -> Result<()> { Ok(()) }
    fn OnUninitDocumentMgr(&self, _pdim: Option<&ITfDocumentMgr>) -> Result<()> { Ok(()) }
    fn OnSetFocus(&self, _pdimfocus: Option<&ITfDocumentMgr>, _pdimprevfocus: Option<&ITfDocumentMgr>) -> Result<()> { Ok(()) }
    fn OnPushContext(&self, _pic: Option<&ITfContext>) -> Result<()> { Ok(()) }
    fn OnPopContext(&self, _pic: Option<&ITfContext>) -> Result<()> { Ok(()) }
}

// ── ITfTextEditSink ──

impl ITfTextEditSink_Impl for UnimTextService_Impl {
    fn OnEndEdit(
        &self,
        _pic: Option<&ITfContext>,
        _ecreadonly: u32,
        _peditrecord: Option<&ITfEditRecord>,
    ) -> Result<()> {
        Ok(())
    }
}

// ── ITfDisplayAttributeProvider ──

impl ITfDisplayAttributeProvider_Impl for UnimTextService_Impl {
    fn EnumDisplayAttributeInfo(&self) -> Result<IEnumTfDisplayAttributeInfo> {
        let enumerator = crate::display_attr::DisplayAttributeEnum::new();
        Ok(enumerator.into())
    }

    fn GetDisplayAttributeInfo(&self, guid: *const GUID) -> Result<ITfDisplayAttributeInfo> {
        unsafe {
            if guid.is_null() { return Err(E_INVALIDARG.into()); }
            let guid = &*guid;
            if *guid == crate::globals::UNIM_DISPLAY_ATTR_INPUT {
                Ok(crate::display_attr::InputDisplayAttribute::new().into())
            } else if *guid == crate::globals::UNIM_DISPLAY_ATTR_CONVERTED {
                Ok(crate::display_attr::ConvertedDisplayAttribute::new().into())
            } else {
                Err(E_INVALIDARG.into())
            }
        }
    }
}
