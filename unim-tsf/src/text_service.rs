//! UnimTextService — TSF 메인 구조체

use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Mutex};
use std::time::SystemTime;

use windows::core::*;
use windows::Win32::Foundation::*;
use windows::Win32::UI::TextServices::*;

use unim::config::{Config, InputCategory};
use unim::input_engine::InputEngine;

use crate::auto_typefix::AutoTypeFixState;
use crate::composition::CompositionManager;
use crate::key_handler;
use crate::lang_bar::{LangBarState, UnimLangBarButton};
use crate::popup_window::PopupWindow;

#[implement(
    ITfTextInputProcessorEx,
    ITfKeyEventSink,
    ITfCompositionSink,
    ITfThreadMgrEventSink,
    ITfTextEditSink,
    ITfDisplayAttributeProvider,
    ITfFunctionProvider
)]
pub struct UnimTextService {
    pub(crate) thread_mgr: Mutex<Option<ITfThreadMgr>>,
    pub(crate) client_id: AtomicU32,
    /// Arc 로 보관해 UnimLangBarButton 과 공유 (갭2: langbar→엔진 토글).
    pub(crate) engine: Arc<Mutex<InputEngine>>,
    /// Arc 로 보관해 UnimLangBarButton 과 공유 (set_input_category 시 Config 필요).
    pub(crate) config: Arc<Mutex<Config>>,
    pub(crate) composition_mgr: Mutex<CompositionManager>,
    pub(crate) key_event_sink_installed: Mutex<bool>,
    pub(crate) thread_mgr_sink_cookie: Mutex<Option<u32>>,
    /// 마지막으로 로드한 config.yaml의 mtime. OnSetFocus 시 변경 감지에 사용.
    pub(crate) config_mtime: Mutex<Option<SystemTime>>,
    /// 한자/특수문자/이모지 팝업 윈도우 (TSF STA 스레드 전용).
    /// 팝업 비활성 시 None, 최초 Show* 액션 시 lazy 생성.
    pub(crate) popup_window: Mutex<Option<PopupWindow>>,
    /// AutoTypeFix 오케스트레이션 상태 (키스트로크 버퍼·undo·blacklist).
    pub(crate) atf_state: Mutex<AutoTypeFixState>,
    /// 랭귀지바 버튼 (ActivateEx 에서 AddItem, Deactivate 에서 RemoveItem).
    /// ITfLangBarItem 으로 보관해 RemoveItem 시 재사용.
    pub(crate) langbar_item: Mutex<Option<ITfLangBarItem>>,
    /// 랭귀지바 버튼 COM 참조 (Deactivate 시 None 처리용, 현재 미사용 직접 호출).
    pub(crate) langbar_btn: Mutex<Option<ITfLangBarItemButton>>,
    /// 갭1: 엔진→langbar 동기화용 공유 상태 (is_korean 캐시 + sink).
    /// OnKeyDown 후 엔진 모드 변화 시 update() 를 호출해 랭귀지바를 갱신한다.
    pub(crate) langbar_state: Mutex<Option<Arc<LangBarState>>>,
}

impl UnimTextService {
    pub fn new() -> Self {
        let config = Config::load_from_default_path();
        // 현재 config 파일의 mtime 초기화
        let config_mtime = Config::default_config_path()
            .and_then(|p| std::fs::metadata(p).ok())
            .and_then(|m| m.modified().ok());
        let engine = InputEngine::new(&config);
        // 초기 한/영 모드 (기본 카테고리 기준)
        let is_korean = engine.input_category() == InputCategory::Korean;
        Self {
            thread_mgr: Mutex::new(None),
            client_id: AtomicU32::new(0),
            engine: Arc::new(Mutex::new(engine)),
            config: Arc::new(Mutex::new(config)),
            composition_mgr: Mutex::new(CompositionManager::new()),
            key_event_sink_installed: Mutex::new(false),
            thread_mgr_sink_cookie: Mutex::new(None),
            config_mtime: Mutex::new(config_mtime),
            popup_window: Mutex::new(None),
            atf_state: Mutex::new(AutoTypeFixState::new()),
            langbar_item: Mutex::new(None),
            langbar_btn: Mutex::new(None),
            langbar_state: Mutex::new(Some(LangBarState::new(is_korean))),
        }
    }

    pub fn client_id(&self) -> u32 {
        self.client_id.load(Ordering::SeqCst)
    }
}

// ── ITfTextInputProcessorEx ──

impl ITfTextInputProcessorEx_Impl for UnimTextService_Impl {
    fn ActivateEx(&self, ptim: Option<&ITfThreadMgr>, tid: u32, _dwflags: u32) -> Result<()> {
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

        // ── 랭귀지바 버튼 등록 ──────────────────────────────────────────────
        unsafe {
            if let Ok(lbmgr) = thread_mgr.cast::<ITfLangBarItemMgr>() {
                // 공유 상태 Arc (이미 new() 에서 생성됨)
                let state_arc = {
                    self.langbar_state.lock().unwrap()
                        .clone()
                        .expect("langbar_state must be initialized in new()")
                };
                // 갭2: engine Arc · config Arc 를 버튼에 주입
                let btn = UnimLangBarButton::new(
                    state_arc,
                    Arc::clone(&self.engine),
                    Arc::clone(&self.config),
                );
                // ITfLangBarItemButton → ITfLangBarItem (계층 관계) 으로 캐스팅
                let btn_button: ITfLangBarItemButton = btn.into();
                let btn_item: ITfLangBarItem = btn_button.cast()?;
                let _ = lbmgr.AddItem(&btn_item);
                // COM 참조 보관 (Deactivate 시 RemoveItem 용)
                let btn_button2: ITfLangBarItemButton = btn_item.cast()?;
                *self.langbar_btn.lock().unwrap() = Some(btn_button2);
                *self.langbar_item.lock().unwrap() = Some(btn_item);
            }
        }

        // 한/영(VK_HANGUL)·한자(VK_HANJA) 키는 PreserveKey 로 등록하지 않는다.
        // 등록하면 입력이 OnPreservedKey 로 라우팅되는데, 그 경로는 별도 핸들링이
        // 필요하고 rguid 로 키를 구별해야 한다. 대신 일반 OnTestKeyDown/OnKeyDown
        // 경로에서 KeyCode::Korean / KeyCode::Hanja 로 받아 공유 InputEngine 이
        // 처리하도록 둔다. (key_handler::test_key_down 이 해당 키를 소비 처리)

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

            // ── 랭귀지바 버튼 제거 ──────────────────────────────────────────
            if let Some(ref item) = self.langbar_item.lock().unwrap().take() {
                unsafe {
                    if let Ok(lbmgr) = thread_mgr.cast::<ITfLangBarItemMgr>() {
                        let _ = lbmgr.RemoveItem(item);
                    }
                }
            }
            *self.langbar_btn.lock().unwrap() = None;
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
        // config.yaml mtime을 비교해 변경됐으면 엔진·설정을 조용히 reload.
        // 입력 비활성 시점(포커스 전환)이므로 race 없음.
        let Some(path) = Config::default_config_path() else {
            return Ok(());
        };
        let Ok(meta) = std::fs::metadata(&path) else {
            return Ok(());
        };
        let Ok(new_mtime) = meta.modified() else {
            return Ok(());
        };

        let mut stored = self.config_mtime.lock().unwrap();
        let reload = match *stored {
            Some(prev) => new_mtime != prev,
            None => true,
        };

        if reload {
            if let Ok(new_config) = Config::load_from_path(&path) {
                let new_engine = InputEngine::new(&new_config);
                // 진행 중인 composition 안전 종료: engine → config → composition 순
                let mut engine_guard = self.engine.lock().unwrap();
                let mut config_guard = self.config.lock().unwrap();
                let mut comp_guard = self.composition_mgr.lock().unwrap();
                comp_guard.clear();
                *engine_guard = new_engine;
                *config_guard = new_config;
                *stored = Some(new_mtime);
                // 팝업도 닫기 (재설정 시 남아있으면 안 됨)
                if let Some(ref mut win) = *self.popup_window.lock().unwrap() {
                    win.hide();
                }
                // AutoTypeFix 상태 초기화 + 외부 데이터 리로드
                let mut atf = self.atf_state.lock().unwrap();
                atf.reset_on_focus();
                atf.reload_external_data(&config_guard.engine.auto_typefix);
            }
        } else {
            // 포커스 이동 시마다 AutoTypeFix 버퍼 초기화 (engine_worker handle_focus_in 대응)
            self.atf_state.lock().unwrap().reset_on_focus();
        }

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
        let popup_active = self
            .popup_window
            .lock()
            .unwrap()
            .as_ref()
            .map(|w| w.is_active())
            .unwrap_or(false);
        let eaten = key_handler::test_key_down(&engine, &config, wparam, pic, popup_active);
        Ok(BOOL::from(eaten))
    }

    fn OnKeyDown(&self, pic: Option<&ITfContext>, wparam: WPARAM, _lparam: LPARAM) -> Result<BOOL> {
        let context = pic.ok_or(E_INVALIDARG)?;
        let mut engine = self.engine.lock().unwrap();
        let config = self.config.lock().unwrap();
        let mut comp_mgr = self.composition_mgr.lock().unwrap();
        let mut popup_win = self.popup_window.lock().unwrap();
        let mut atf_state = self.atf_state.lock().unwrap();
        let tid = self.client_id();
        let comp_sink: ITfCompositionSink = unsafe { self.cast()? };

        // 갭1: 키 처리 전후 모드 비교를 위해 이전 카테고리 저장.
        let prev_category = engine.input_category();

        let eaten = key_handler::handle_key_down(
            &mut engine,
            &config,
            &mut comp_mgr,
            &mut popup_win,
            &mut atf_state,
            context,
            tid,
            wparam,
            &comp_sink,
        );

        // 갭1: 엔진→랭귀지바 동기화.
        // 키 처리로 모드가 바뀌었으면 langbar_state 를 갱신 → OnUpdate 발사.
        let current_category = engine.input_category();
        if prev_category != current_category {
            let is_korean = current_category == InputCategory::Korean;
            if let Some(ref state) = *self.langbar_state.lock().unwrap() {
                state.update(is_korean);
            }
        }

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

    fn OnKeyUp(&self, _pic: Option<&ITfContext>, _wparam: WPARAM, _lparam: LPARAM) -> Result<BOOL> {
        Ok(FALSE)
    }

    fn OnPreservedKey(&self, _pic: Option<&ITfContext>, _rguid: *const GUID) -> Result<BOOL> {
        // 현재 PreserveKey 로 등록하는 키가 없으므로 호출되지 않는다.
        // 한/영·한자는 OnKeyDown 경로에서 처리한다.
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
        // 팝업도 닫기 (포커스 이탈 등으로 composition 강제 종료 시)
        if let Some(ref mut win) = *self.popup_window.lock().unwrap() {
            win.hide();
        }
        // AutoTypeFix 버퍼 초기화
        self.atf_state.lock().unwrap().reset_on_focus();
        Ok(())
    }
}

// ── ITfThreadMgrEventSink ──

impl ITfThreadMgrEventSink_Impl for UnimTextService_Impl {
    fn OnInitDocumentMgr(&self, _pdim: Option<&ITfDocumentMgr>) -> Result<()> {
        Ok(())
    }
    fn OnUninitDocumentMgr(&self, _pdim: Option<&ITfDocumentMgr>) -> Result<()> {
        Ok(())
    }
    fn OnSetFocus(
        &self,
        _pdimfocus: Option<&ITfDocumentMgr>,
        _pdimprevfocus: Option<&ITfDocumentMgr>,
    ) -> Result<()> {
        Ok(())
    }
    fn OnPushContext(&self, _pic: Option<&ITfContext>) -> Result<()> {
        Ok(())
    }
    fn OnPopContext(&self, _pic: Option<&ITfContext>) -> Result<()> {
        Ok(())
    }
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

// ── ITfFunctionProvider ──
//
// TSF 가 TIP CLSID 의 ITfFunctionProvider 를 요청할 때 UnimTextService 자체가 응답.
// GetFunction(*, IID_ITfFnConfigure) → fn_configure::UnimFnConfigure 반환.

impl ITfFunctionProvider_Impl for UnimTextService_Impl {
    fn GetType(&self) -> Result<GUID> {
        Ok(crate::globals::UNIM_CLSID)
    }

    fn GetDescription(&self) -> Result<BSTR> {
        Ok(BSTR::from("UNIM Korean IME Function Provider"))
    }

    fn GetFunction(
        &self,
        _rguid: *const GUID,
        riid: *const GUID,
    ) -> Result<IUnknown> {
        unsafe {
            if riid.is_null() {
                return Err(E_INVALIDARG.into());
            }
            if *riid == ITfFnConfigure::IID {
                let obj = crate::fn_configure::UnimFnConfigure::new();
                let fn_cfg: ITfFnConfigure = obj.into();
                let unk: IUnknown = fn_cfg.cast()?;
                Ok(unk)
            } else {
                Err(windows_core::HRESULT(0x80004002_u32 as i32).into())
            }
        }
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
            if guid.is_null() {
                return Err(E_INVALIDARG.into());
            }
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
