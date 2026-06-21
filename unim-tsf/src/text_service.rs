//! UnimTextService — TSF 메인 구조체

use std::collections::HashSet;
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Instant, SystemTime};

use windows::core::*;
use windows::Win32::Foundation::*;
use windows::Win32::UI::Input::KeyboardAndMouse::GetFocus;
use windows::Win32::UI::TextServices::*;

use unim::config::{Config, InputCategory};
use unim::input_engine::InputEngine;

use crate::auto_typefix::AutoTypeFixState;
use crate::composition::CompositionManager;
use crate::key_handler;
use crate::lang_bar::{LangBarState, UnimLangBarButton};
use crate::popup_ipc::{PopupClient, RevChannel, WM_UNIM_REV};
use crate::preedit_window::PreeditWindow;

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
    /// Arc 로 보관해 역채널 wndproc(RevWndContext)과 공유.
    pub(crate) composition_mgr: Arc<Mutex<CompositionManager>>,
    pub(crate) key_event_sink_installed: Mutex<bool>,
    pub(crate) thread_mgr_sink_cookie: Mutex<Option<u32>>,
    /// 마지막으로 로드한 config.yaml의 mtime. OnSetFocus 시 변경 감지에 사용.
    pub(crate) config_mtime: Mutex<Option<SystemTime>>,
    /// 한자/특수문자/이모지 팝업 IPC 클라이언트 (out-of-process 렌더러로 송신).
    /// 비차단 worker 스레드를 소유. 렌더러 부재 시에도 타이핑 무영향 (§6.2).
    /// Arc 로 보관해 역채널 wndproc(RevWndContext)과 공유 (§11.G 마우스 클릭 적용).
    pub(crate) popup_ipc: Arc<Mutex<PopupClient>>,
    /// 마지막 활성 `ITfContext` (OnKeyDown 진입 시 clone). 역채널 마우스 확정 시
    /// 이 컨텍스트로 비조합 문서 삽입한다 (§11.G). TSF STA 스레드 전용 접근.
    pub(crate) last_context: Arc<Mutex<Option<ITfContext>>>,
    /// 역채널 message-only 창 (HWND + 공유 컨텍스트). ActivateEx 에서 생성,
    /// Deactivate 에서 파괴. 생성 실패해도 IME 무영향 (역채널만 비활성).
    pub(crate) rev_window: Mutex<Option<RevWindow>>,
    /// client-side preedit 오버레이 창 (터미널·레거시 앱 폴백 전용, lazy 생성).
    /// composition 미지원 앱에서 조합 중 음절을 앱 버퍼 대신 이 창에 그린다.
    pub(crate) preedit_window: Mutex<Option<PreeditWindow>>,
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
    /// composition 미지원 앱(wezterm 등 CUAS/IMM32 브리지) 감지 플래그.
    ///
    /// `OnCompositionTerminated` 가 키 입력 직후 즉시 발생하면(=앱이 composition
    /// 을 유지 못 함) set 된다. 이후 키 입력은 composition 없이 backspace+reinsert
    /// 폴백 경로(key_handler)를 탄다.
    pub(crate) composition_unsupported: AtomicBool,
    /// 폴백 경로에서 문서에 떠있는 미확정(preedit) 글자 수.
    pub(crate) fallback_pending: AtomicUsize,
    /// 마지막 OnKeyDown 시각. OnCompositionTerminated 가 "키 직후 즉시 종료
    /// (wezterm)" 인지 "한참 뒤 종료(포커스 이탈)" 인지 구분하는 데 쓴다.
    pub(crate) last_key_instant: Mutex<Option<Instant>>,

    /// 마지막으로 config.yaml 변경을 확인한 시각. 키 입력 경로(OnKeyDown)에서
    /// 매 키마다 fs::metadata 를 때리지 않도록 스로틀링하는 데 쓴다.
    pub(crate) last_reload_check: Mutex<Option<Instant>>,

    /// 즉시-terminate(CUAS 브리지)로 판명된 포커스 창(HWND) 집합.
    ///
    /// `OnCompositionTerminated` 가 키 직후(<200ms) 발생하는 창을 같은 HWND 로 **2회**
    /// 관찰하면 학습해 저장한다. 이후 같은 창에서는 200ms 매직넘버에 의존하지 않고도
    /// "이 앱은 composition 유지 불가"로 확정 판정한다 — 느린 머신/원격 데스크톱에서
    /// 지연된 즉시-terminate(>200ms)를 놓쳐 조합이 다시 깨지는 것을 줄인다(지식베이스 P1).
    pub(crate) cuas_windows: Mutex<HashSet<isize>>,
}

impl UnimTextService {
    pub fn new() -> Self {
        // UWP(AppContainer) 경로 리다이렉트 우회: config 로드 전에 실제
        // %APPDATA% 경로를 환경변수로 공개한다 (프로세스 1회, idempotent).
        crate::sandbox_paths::init();
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
            composition_mgr: Arc::new(Mutex::new(CompositionManager::new())),
            key_event_sink_installed: Mutex::new(false),
            thread_mgr_sink_cookie: Mutex::new(None),
            config_mtime: Mutex::new(config_mtime),
            popup_ipc: Arc::new(Mutex::new(PopupClient::new())),
            last_context: Arc::new(Mutex::new(None)),
            rev_window: Mutex::new(None),
            preedit_window: Mutex::new(None),
            atf_state: Mutex::new(AutoTypeFixState::new()),
            langbar_item: Mutex::new(None),
            langbar_btn: Mutex::new(None),
            langbar_state: Mutex::new(Some(LangBarState::new(is_korean))),
            composition_unsupported: AtomicBool::new(false),
            fallback_pending: AtomicUsize::new(0),
            last_key_instant: Mutex::new(None),
            last_reload_check: Mutex::new(None),
            cuas_windows: Mutex::new(HashSet::new()),
        }
    }

    pub fn client_id(&self) -> u32 {
        self.client_id.load(Ordering::SeqCst)
    }

    /// 조합 중 패스쓰루 키(네비게이션/수정자 조합)를 만났을 때 현재 조합을
    /// 확정하고 정리한다. OnTestKeyDown 의 두 패스쓰루 블록(navkey·modifier-combo)
    /// 공통 로직 — 호출자가 이미 engine 락을 보유하므로 가드를 인자로 받는다.
    ///
    /// comp_mgr 가 활성이면 preedit 으로 확정 종료, 비활성이지만 preedit 이 남아
    /// 있으면(오버레이 폴백 모드) 문서에 직접 insert 한다. 이후 오버레이 창 숨김 +
    /// 엔진 preedit 제거.
    fn commit_for_passthrough(&self, engine: &mut InputEngine, context: &ITfContext) {
        let tid = self.client_id();
        let preedit = engine.preedit_str().to_string();
        {
            let mut comp_mgr = self.composition_mgr.lock().unwrap();
            if comp_mgr.is_active() {
                if preedit.is_empty() {
                    comp_mgr.end_composition(context, tid);
                } else {
                    comp_mgr.end_composition_with_text(context, tid, &preedit);
                }
            } else if !preedit.is_empty() {
                // 오버레이 폴백 모드: 조합 글자를 문서에 직접 확정.
                comp_mgr.insert_text(context, tid, &preedit);
            }
        }
        if let Some(w) = self.preedit_window.lock().unwrap().as_mut() {
            w.hide();
        }
        engine.remove_preedit();
    }

    /// config.yaml mtime 을 확인해 변경됐으면 엔진·설정을 조용히 reload 한다.
    /// 설정 GUI 가 저장하면 포커스 전환 없이도 다음 키 입력 시 즉시 반영된다.
    ///
    /// - `throttle=true`(키 입력 경로): 최근 검사 후 300ms 이내면 디스크 접근 없이 건너뜀.
    /// - 조합 중이거나 폴백(pending) 버퍼가 있으면 글자 깨짐 방지를 위해 reload 를 미룸
    ///   (mtime 을 갱신하지 않으므로 다음 기회에 재시도).
    ///
    /// 어떤 락도 보유하지 않은 상태에서 호출해야 한다(내부에서 engine/config/
    /// composition/popup/atf 락을 OnSetFocus·OnKeyDown 과 동일 순서로 취득).
    pub(crate) fn maybe_reload_config(&self, throttle: bool) {
        if throttle {
            let mut last = self.last_reload_check.lock().unwrap();
            if let Some(prev) = *last {
                if prev.elapsed() < std::time::Duration::from_millis(300) {
                    return;
                }
            }
            *last = Some(Instant::now());
        }

        let Some(path) = Config::default_config_path() else {
            return;
        };
        let Ok(meta) = std::fs::metadata(&path) else {
            return;
        };
        let Ok(new_mtime) = meta.modified() else {
            return;
        };

        // mtime 변화 여부만 먼저 싸게 확인.
        {
            let stored = self.config_mtime.lock().unwrap();
            let changed = match *stored {
                Some(prev) => new_mtime != prev,
                None => true,
            };
            if !changed {
                return;
            }
        }

        // 조합/폴백 진행 중이면 미룬다 (확정 후 다음 키에서 반영).
        if self.composition_mgr.lock().unwrap().is_active()
            || self.fallback_pending.load(Ordering::SeqCst) != 0
        {
            return;
        }

        if let Ok(new_config) = Config::load_from_path(&path) {
            let new_engine = InputEngine::new(&new_config);
            // engine → config → composition → popup → atf 순(다른 경로와 동일).
            let mut engine_guard = self.engine.lock().unwrap();
            let mut config_guard = self.config.lock().unwrap();
            let mut comp_guard = self.composition_mgr.lock().unwrap();
            comp_guard.clear();
            *engine_guard = new_engine;
            *config_guard = new_config;
            *self.config_mtime.lock().unwrap() = Some(new_mtime);
            self.popup_ipc.lock().unwrap().hide();
            if let Some(ref mut win) = *self.preedit_window.lock().unwrap() {
                win.hide();
            }
            let mut atf = self.atf_state.lock().unwrap();
            atf.reset_on_focus();
            atf.reload_external_data(&config_guard.engine.auto_typefix);
        }
    }
}

// ── ITfTextInputProcessorEx ──

impl ITfTextInputProcessorEx_Impl for UnimTextService_Impl {
    fn ActivateEx(&self, ptim: Ref<'_, ITfThreadMgr>, tid: u32, _dwflags: u32) -> Result<()> {
        let thread_mgr = ptim.as_ref().ok_or(E_INVALIDARG)?;
        self.client_id.store(tid, Ordering::SeqCst);
        *self.thread_mgr.lock().unwrap() = Some(thread_mgr.clone());

        unsafe {
            let keystroke_mgr: ITfKeystrokeMgr = thread_mgr.cast()?;
            let this_sink: ITfKeyEventSink = self.to_interface();
            keystroke_mgr.AdviseKeyEventSink(tid, &this_sink, true)?;
            *self.key_event_sink_installed.lock().unwrap() = true;
        }

        unsafe {
            let source: ITfSource = thread_mgr.cast()?;
            let this_sink: ITfThreadMgrEventSink = self.to_interface();
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
                // compartment 동기화용 thread_mgr + tid 주입. 이후 state.update()
                // (OnKeyDown · 랭귀지바 클릭 양쪽)가 OS 입력 표시기를 갱신한다.
                state_arc.set_tsf(thread_mgr.clone(), tid);
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

        // ── compartment 초기 동기화 ──────────────────────────────────────
        // 현재 엔진의 한/영 상태를 thread-manager compartment 2개에 1회 set 해
        // OS 입력 표시기를 UNIM 상태와 일치시킨다 (이전 IME 표시기 잔상 제거).
        // OPENCLOSE 는 thread-mgr 공유 상태이지만 SampleIME 도 활성화 시
        // 초기값을 set 하므로 1회 set 은 안전. 실패해도 입력 기능엔 무관.
        {
            let is_korean =
                self.engine.lock().unwrap().input_category() == InputCategory::Korean;
            crate::compartment::sync_keyboard_mode(thread_mgr, tid, is_korean);
        }

        // ── §11.G 역채널 message-only 창 생성 (마우스 클릭 적용 경로) ──────────
        // 실패해도 IME 무영향 — 역채널(마우스)만 비활성, 키보드 입력은 정상.
        {
            let rev = self.popup_ipc.lock().unwrap().rev_channel();
            let comp_sink: ITfCompositionSink = self.to_interface();
            let ctx = Box::new(RevWndContext {
                engine: Arc::clone(&self.engine),
                config: Arc::clone(&self.config),
                composition_mgr: Arc::clone(&self.composition_mgr),
                popup_ipc: Arc::clone(&self.popup_ipc),
                last_context: Arc::clone(&self.last_context),
                rev,
                comp_sink,
                tid,
            });
            match RevWindow::create(ctx) {
                Some(w) => {
                    *self.rev_window.lock().unwrap() = Some(w);
                    crate::register::dbg_log("popup_rev: reverse channel armed (message-only HWND)");
                }
                None => {
                    crate::register::dbg_log(
                        "popup_rev: reverse channel NOT armed (window create failed) — mouse disabled",
                    );
                }
            }
        }

        crate::dll_add_ref();
        Ok(())
    }
}

impl ITfTextInputProcessor_Impl for UnimTextService_Impl {
    fn Activate(&self, ptim: Ref<'_, ITfThreadMgr>, tid: u32) -> Result<()> {
        self.ActivateEx(ptim, tid, 0)
    }

    fn Deactivate(&self) -> Result<()> {
        // 종료/IME 전환 경로 — 화면 중앙 팝업 잔존 방지 (설계서 §6.6).
        // ITfThreadMgrEventSink::OnSetFocus 는 TIP 전환 시 보장되지 않으므로
        // Deactivate 진입부에서 명시적으로 Hide 를 송신한다.
        self.popup_ipc.lock().unwrap().hide();

        // §11.G 역채널 창 파괴 (notify 해제 + DestroyWindow + 컨텍스트 회수).
        // 반드시 해제 — 훅/창 누수 시 다음 활성화에서 stale wndproc 위험.
        *self.rev_window.lock().unwrap() = None;
        *self.last_context.lock().unwrap() = None;

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
        // 포커스 전환(입력 비활성 시점)에 config 변경을 즉시 확인해 reload.
        self.maybe_reload_config(false);
        // 포커스 이동 시마다 AutoTypeFix 버퍼 초기화 (engine_worker handle_focus_in 대응).
        // reload 시 maybe_reload_config 가 이미 reset 했더라도 idempotent 하므로 안전.
        self.atf_state.lock().unwrap().reset_on_focus();
        Ok(())
    }

    fn OnTestKeyDown(
        &self,
        pic: Ref<'_, ITfContext>,
        wparam: WPARAM,
        _lparam: LPARAM,
    ) -> Result<BOOL> {
        // ATF SendInput 폴백(synth_input)이 주입한 합성 키는 엔진을 거치지 않고
        // 식별: BS/유니코드는 통과(앱이 직접 처리), 센티널은 소비(OnKeyDown 에서
        // replay preedit composition 시작). 어떤 락 취득보다 먼저 검사해야 한다.
        if let Some(eaten) = crate::synth_input::observe_test_key_down(wparam.0 as u16) {
            return Ok(BOOL::from(eaten));
        }
        let mut engine = self.engine.lock().unwrap();
        let config = self.config.lock().unwrap();
        let popup_active = self.popup_ipc.lock().unwrap().is_active();

        let kc = unim::keycode::KeyCode::from_win32_vk(wparam.0 as u16);

        // ── 버그③: 조합 중 수정자 단축키(Ctrl+J, Shift+Del 등) 패스쓰루 ──
        //
        // is_commit_passthrough_key 목록(Enter/Tab/Esc/방향/Home/End/PgUp/PgDn)에
        // 없는 수정자 조합은 OnTestKeyDown 커밋 블록을 못 타 조합이 잔류했다.
        // modifier 우선으로 먼저 검사한다(아래 navkey 블록보다 앞).
        //
        // 조건(get_modifier_state 인라인 호출):
        //   - Ctrl/Alt/Super + 비수정자  → 커밋+패스쓰루.
        //   - Shift + !is_character_key()(Del/Insert/F-keys/방향 등) → 커밋+패스쓰루.
        //   - Shift + is_character_key()(Shift+A 대문자, Shift+1=! 등) → 제외
        //     (엔진이 Korean 모드에서 이미 소비·처리하는 조합/기호 입력).
        //   - is_modifier() 단독(Ctrl/Shift 누름) → 제외.
        let m = key_handler::get_modifier_state();
        let is_combo = !kc.is_modifier()
            && (m.control || m.alt || m.super_key || (m.shift && !kc.is_character_key()));
        if !popup_active && engine.is_composing() && is_combo {
            if let Some(context) = pic.as_ref() {
                self.commit_for_passthrough(&mut engine, context);
                crate::register::dbg_log(&format!(
                    "OnTestKeyDown: commit+passthrough modifier-combo vk=0x{:02X}",
                    wparam.0 as u16
                ));
                return Ok(FALSE);
            }
        }


        // NavilIME 패턴: 조합 중 Enter/화살표/Tab/Esc 등 네비게이션 키는 여기서
        // 현재 조합을 확정하고 pIsEaten=FALSE 로 통과시킨다. 그래야 OnKeyDown 이
        // 호출되지 않아(키가 IME 에 안 먹힘) 앱이 개행·커서이동 등을 직접 처리한다.
        // (commit 을 OnKeyDown 에서 한 뒤 false 반환하면 CUAS 가 이미 claim 된 키를
        // 앱으로 안 흘려보낸다 — Enter/화살표가 동작하지 않던 원인.)
        // 팝업 활성 시에는 이 키들이 후보 내비게이션이므로 제외한다.
        if !popup_active && engine.is_composing() && key_handler::is_commit_passthrough_key(kc) {
            if let Some(context) = pic.as_ref() {
                self.commit_for_passthrough(&mut engine, context);
                crate::register::dbg_log(&format!(
                    "OnTestKeyDown: commit+passthrough navkey vk=0x{:02X}",
                    wparam.0 as u16
                ));
                return Ok(FALSE);
            }
        }

        let eaten =
            key_handler::test_key_down(&engine, &config, wparam, pic.as_ref(), popup_active);
        Ok(BOOL::from(eaten))
    }

    fn OnKeyDown(&self, pic: Ref<'_, ITfContext>, wparam: WPARAM, _lparam: LPARAM) -> Result<BOOL> {
        // [imm32 진입 진단] OnKeyDown 무조건 진입 로그. OnTestKeyDown 은 찍히는데
        // OnKeyDown 이 안 찍히면(=test 만 FALSE 반환) 키 라우팅 단절 지점 식별 가능.
        crate::register::dbg_log(&format!("OnKeyDown ENTER vk=0x{:02X}", wparam.0 as u16));
        let context = pic.as_ref().ok_or(E_INVALIDARG)?;
        // §11.G 역채널 마우스 확정용으로 최신 컨텍스트 보관 (clone, TSF STA 스레드).
        *self.last_context.lock().unwrap() = Some(context.clone());
        // ATF SendInput 폴백의 합성 키 처리. 센티널 도착 = 큐 순서상 앞의
        // 백스페이스·유니코드 삽입이 모두 앱에 전달된 뒤이므로, 이 시점에
        // replay preedit composition 을 시작한다.
        match crate::synth_input::observe_key_down(wparam.0 as u16) {
            Some(crate::synth_input::SynthKeyAction::PassThrough) => return Ok(FALSE),
            Some(crate::synth_input::SynthKeyAction::StartPreedit(preedit)) => {
                if !preedit.is_empty() {
                    let tid = self.client_id();
                    let comp_sink: ITfCompositionSink = self.to_interface();
                    let mut comp_mgr = self.composition_mgr.lock().unwrap();
                    comp_mgr.start_composition(context, tid, &preedit, &comp_sink);
                }
                return Ok(TRUE);
            }
            None => {}
        }
        // OnCompositionTerminated 의 "즉시 종료" 판별용 타임스탬프.
        *self.last_key_instant.lock().unwrap() = Some(Instant::now());
        // 설정 GUI 가 config.yaml 을 저장하면 포커스 전환 없이도 다음 키 입력 시
        // 즉시 반영. 스로틀(300ms) + 조합 중 미룸으로 글자 깨짐·디스크 부담 방지.
        // 반드시 아래 락 취득 전에 호출(내부에서 동일 락을 잡으므로 재진입 데드락 방지).
        self.maybe_reload_config(true);
        let mut engine = self.engine.lock().unwrap();
        let config = self.config.lock().unwrap();
        let mut comp_mgr = self.composition_mgr.lock().unwrap();
        let mut popup_ipc = self.popup_ipc.lock().unwrap();
        let mut preedit_win = self.preedit_window.lock().unwrap();
        let mut atf_state = self.atf_state.lock().unwrap();
        let tid = self.client_id();
        let comp_sink: ITfCompositionSink = self.to_interface();

        // 연속 조합 복구(non-sticky 폴백): 새 조합이 시작되는 시점(엔진 미조합 &
        // 활성 composition 없음 = 단어 경계)마다 폴백 플래그를 리셋해 composition 을
        // 다시 시도한다. fInterimChar=TRUE 적용 후 wezterm 에서 composition 이 대부분
        // 성공하므로, CUAS 가 간헐적으로 한 단어의 조합을 종료시켜도 그 단어만
        // 오버레이로 처리되고 다음 단어부터 inline 조합이 복구된다(과거엔 1회 종료
        // 시 composition_unsupported 가 영구 true 로 고착돼 이후 전부 오버레이였음).
        if !engine.is_composing() && !comp_mgr.is_active() {
            self.composition_unsupported.store(false, Ordering::SeqCst);
            self.fallback_pending.store(0, Ordering::SeqCst);
        }

        // 갭1: 키 처리 전후 모드 비교를 위해 이전 카테고리 저장.
        let prev_category = engine.input_category();

        let eaten = key_handler::handle_key_down(
            &mut engine,
            &config,
            &mut comp_mgr,
            &mut popup_ipc,
            &mut preedit_win,
            &mut atf_state,
            context,
            tid,
            wparam,
            &comp_sink,
            self.composition_unsupported.load(Ordering::SeqCst),
            &self.fallback_pending,
        );

        // 갭1: 엔진→랭귀지바 동기화.
        // 키 처리로 모드가 바뀌었으면 langbar_state 를 갱신 → OnUpdate 발사.
        let current_category = engine.input_category();
        if prev_category != current_category {
            let is_korean = current_category == InputCategory::Korean;
            // state.update() 가 랭귀지바 OnUpdate + OS compartment 동기화를 함께 처리.
            if let Some(ref state) = *self.langbar_state.lock().unwrap() {
                state.update(is_korean);
            }
        }

        Ok(BOOL::from(eaten))
    }

    fn OnTestKeyUp(
        &self,
        _pic: Ref<'_, ITfContext>,
        _wparam: WPARAM,
        _lparam: LPARAM,
    ) -> Result<BOOL> {
        Ok(FALSE)
    }

    fn OnKeyUp(&self, _pic: Ref<'_, ITfContext>, _wparam: WPARAM, _lparam: LPARAM) -> Result<BOOL> {
        Ok(FALSE)
    }

    fn OnPreservedKey(&self, _pic: Ref<'_, ITfContext>, _rguid: *const GUID) -> Result<BOOL> {
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
        _pcomposition: Ref<'_, ITfComposition>,
    ) -> Result<()> {
        // OnCompositionTerminated 는 "우리 외의 주체(앱/CUAS)" 가 composition 을
        // 강제 종료할 때만 호출된다. 두 경우를 시간으로 구분한다:
        //
        //  (1) 키 입력 직후(<200ms) 즉시 종료 → wezterm 등 CUAS/IMM32 브리지 앱.
        //      composition 을 유지하지 못하므로 폴백(backspace+reinsert) 모드로
        //      전환하고 엔진 preedit 버퍼는 보존한다(문서에 남은 글자 = pending).
        //  (2) 한참 뒤 종료 → 포커스 이탈/Esc 등 정상 종료. 엔진을 reset 한다.
        // 포커스 창(HWND) — 즉시-terminate 학습/조회 키.
        let focus_hwnd = unsafe { GetFocus() }.0 as isize;

        // 시간 기반 즉시-terminate 판정(<200ms). 이 200ms 는 느린 머신/원격 데스크톱
        // 에서 흔들리므로, 한 번이라도 즉시-terminate 로 학습된 창은 타이밍과 무관하게
        // 즉시로 확정한다(지연된 즉시-terminate 놓침 방지).
        let by_time = self
            .last_key_instant
            .lock()
            .unwrap()
            .map(|t| t.elapsed() < std::time::Duration::from_millis(200))
            .unwrap_or(false);
        let known_cuas = focus_hwnd != 0 && self.cuas_windows.lock().unwrap().contains(&focus_hwnd);
        let immediate = by_time || known_cuas;

        // 학습(1-hit): by_time 즉시-종료가 관찰된 창을 즉시 CUAS 로 학습한다.
        //
        // 과거 2-hit 방식은 폴백 진입 후 composition 을 더는 만들지 않아 둘째 hit 가
        // 영영 오지 않았다(실측 known_cuas=0). 1-hit 로 학습해 두면, 같은 창을 다시
        // 포커스할 때 ITfThreadMgrEventSink::OnSetFocus 가 선제적으로 폴백을 켜서
        // 첫 키부터 composition 을 만들지 않는다 → 문서에 stray 글자 0.
        // 오탐 비용: 정상 앱이 잘못 학습돼도 client-side preedit(오버레이)로 동작하며
        // 확정 텍스트는 정상 삽입되므로 파괴적이지 않다.
        if by_time && focus_hwnd != 0 {
            self.cuas_windows.lock().unwrap().insert(focus_hwnd);
        }

        self.composition_mgr.lock().unwrap().clear();

        // immediate(<200ms) 분기만 폴백 진입 처리. 엔진 preedit 버퍼는 보존해
        // 다음 키에서 누적 텍스트로 자연 재진입(start_composition)한다.
        // 정상 종료(포커스 이탈) 정리(engine.reset()/popup hide/atf reset)는
        // ITfThreadMgrEventSink::OnSetFocus(아래)로 이전했다 — CUAS의 정당한
        // 단발 종료를 "사용자 조합 취소"로 오인해 preedit 를 자폭시키지 않기 위함.
        if immediate {
            self.composition_unsupported.store(true, Ordering::SeqCst);
            // 문서에 이미 떠있는 미확정 글자 수 = 현재 엔진 preedit 길이.
            let pending = self.engine.lock().unwrap().preedit_str().chars().count();
            self.fallback_pending.store(pending, Ordering::SeqCst);
            crate::register::dbg_log(&format!(
                "OnCompositionTerminated: IMMEDIATE -> fallback (pending={}, hwnd={:#x}, by_time={}, known_cuas={})",
                pending, focus_hwnd, by_time, known_cuas
            ));
        }
        Ok(())
    }
}

// ── ITfThreadMgrEventSink ──

impl ITfThreadMgrEventSink_Impl for UnimTextService_Impl {
    fn OnInitDocumentMgr(&self, _pdim: Ref<'_, ITfDocumentMgr>) -> Result<()> {
        Ok(())
    }
    fn OnUninitDocumentMgr(&self, _pdim: Ref<'_, ITfDocumentMgr>) -> Result<()> {
        Ok(())
    }
    fn OnSetFocus(
        &self,
        _pdimfocus: Ref<'_, ITfDocumentMgr>,
        _pdimprevfocus: Ref<'_, ITfDocumentMgr>,
    ) -> Result<()> {
        // (Phase 0) 포커스가 새 문서 컨텍스트로 이동 → 폴백 상태 리셋.
        //   단, 이전에 즉시-terminate 로 학습된(cuas_windows) 창으로 포커스가
        //   가는 경우엔 선제적으로 폴백을 켠다 → 첫 키부터 composition 을 만들지
        //   않아 문서에 stray 글자가 남지 않는다(client-side preedit 오버레이 사용).
        //   학습 안 된 창이면 false 로 두고, 즉시-terminate 발생 시 자동 재감지한다.
        let focus_hwnd = unsafe { GetFocus() }.0 as isize;
        let known_cuas =
            focus_hwnd != 0 && self.cuas_windows.lock().unwrap().contains(&focus_hwnd);
        self.composition_unsupported.store(known_cuas, Ordering::SeqCst);
        self.fallback_pending.store(0, Ordering::SeqCst);
        // (Phase 1) OnCompositionTerminated 의 "정상 종료" 정리를 이리로 이전.
        //   포커스 이탈 = 사용자가 조합을 떠남 → 엔진 preedit 를 비우고 팝업/ATF 정리.
        self.engine.lock().unwrap().reset();
        // 포커스 전환 → 팝업 Hide 송신 (렌더러 owner 규칙으로 stale hide 무시 처리).
        self.popup_ipc.lock().unwrap().hide();
        // 포커스 이동 → 보관 컨텍스트 무효화 (역채널 마우스 확정이 옛 창에 안 가도록).
        *self.last_context.lock().unwrap() = None;
        if let Some(ref mut win) = *self.preedit_window.lock().unwrap() {
            win.hide();
        }
        self.atf_state.lock().unwrap().reset_on_focus();
        Ok(())
    }
    fn OnPushContext(&self, _pic: Ref<'_, ITfContext>) -> Result<()> {
        Ok(())
    }
    fn OnPopContext(&self, _pic: Ref<'_, ITfContext>) -> Result<()> {
        Ok(())
    }
}

// ── ITfTextEditSink ──

impl ITfTextEditSink_Impl for UnimTextService_Impl {
    fn OnEndEdit(
        &self,
        _pic: Ref<'_, ITfContext>,
        _ecreadonly: u32,
        _peditrecord: Ref<'_, ITfEditRecord>,
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

// ════════════════════════════════════════════════════════════════════════════
// §11.G 역채널 message-only 창 — 렌더러 클릭 이벤트를 TSF(STA) 스레드로 마샬링.
//
// reader 서브스레드(popup_ipc)가 RevChannel 에 push + PostMessageW(WM_UNIM_REV).
// 본 창의 wndproc 은 TSF 스레드 메시지 펌프가 호출하므로, 엔진 락을 짧게 잡고
// edit session(insert_text)을 안전하게 수행할 수 있다. 어떤 실패도 panic 금지.
// ════════════════════════════════════════════════════════════════════════════

use windows::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, DestroyWindow, GetWindowLongPtrW, RegisterClassExW,
    SetWindowLongPtrW, GWLP_USERDATA, HWND_MESSAGE, WINDOW_EX_STYLE, WINDOW_STYLE, WNDCLASSEXW,
};

const REV_WND_CLASS_NAME: PCWSTR = w!("UNIM_PopupRevWnd");
static REV_WND_CLASS_ATOM: std::sync::OnceLock<u16> = std::sync::OnceLock::new();

/// 역채널 wndproc 이 보는 공유 상태 (Arc 클론 묶음). HWND userdata 로 포인터 보관.
struct RevWndContext {
    engine: Arc<Mutex<InputEngine>>,
    config: Arc<Mutex<Config>>,
    composition_mgr: Arc<Mutex<CompositionManager>>,
    popup_ipc: Arc<Mutex<PopupClient>>,
    last_context: Arc<Mutex<Option<ITfContext>>>,
    rev: Arc<RevChannel>,
    /// composition sink — insert_text 경로에서 comp_sink 서명 일관성용.
    comp_sink: ITfCompositionSink,
    tid: u32,
}

/// 역채널 message-only 창 핸들 래퍼. Drop 시 창 파괴 + Box 회수.
pub(crate) struct RevWindow {
    hwnd: HWND,
    /// userdata 가 가리키는 Box 의 소유권 (Drop 시 회수).
    _ctx: Box<RevWndContext>,
    /// 큐 notify 해제용.
    rev: Arc<RevChannel>,
}

impl RevWindow {
    /// message-only 창을 생성하고 RevChannel 에 notify HWND 를 등록한다.
    /// 실패 시 None (역채널만 비활성, IME 무영향).
    fn create(ctx: Box<RevWndContext>) -> Option<Self> {
        unsafe {
            REV_WND_CLASS_ATOM.get_or_init(|| {
                let wc = WNDCLASSEXW {
                    cbSize: std::mem::size_of::<WNDCLASSEXW>() as u32,
                    lpfnWndProc: Some(rev_wnd_proc),
                    hInstance: crate::dll_instance().into(),
                    lpszClassName: REV_WND_CLASS_NAME,
                    ..Default::default()
                };
                RegisterClassExW(&wc)
            });

            let rev = Arc::clone(&ctx.rev);
            // Box → raw 포인터를 userdata 에 넣어 wndproc 에서 복구.
            let ctx_ptr: *mut RevWndContext = Box::into_raw(ctx);

            let hwnd = match CreateWindowExW(
                WINDOW_EX_STYLE(0),
                REV_WND_CLASS_NAME,
                w!("UNIM_PopupRev"),
                WINDOW_STYLE(0),
                0,
                0,
                0,
                0,
                Some(HWND_MESSAGE),
                None,
                Some(crate::dll_instance().into()),
                None,
            ) {
                Ok(h) => h,
                Err(e) => {
                    crate::register::dbg_log(&format!(
                        "popup_rev: CreateWindowExW(message-only) failed: {e:?}"
                    ));
                    // Box 회수 (창 생성 실패).
                    drop(Box::from_raw(ctx_ptr));
                    return None;
                }
            };

            SetWindowLongPtrW(hwnd, GWLP_USERDATA, ctx_ptr as _);
            rev.set_notify_hwnd(hwnd.0 as isize);
            crate::register::dbg_log(&format!(
                "popup_rev: message-only HWND created {:#x}",
                hwnd.0 as isize
            ));

            // Box 소유권 복구 (Drop 시 함께 해제). raw 포인터와 동일 메모리.
            let owned = Box::from_raw(ctx_ptr);
            Some(Self {
                hwnd,
                _ctx: owned,
                rev,
            })
        }
    }
}

impl Drop for RevWindow {
    fn drop(&mut self) {
        unsafe {
            // notify 차단 먼저 (이후 도착 push 는 PostMessage 안 함).
            self.rev.set_notify_hwnd(0);
            // userdata 초기화 후 창 파괴 (wndproc 재진입 방지).
            SetWindowLongPtrW(self.hwnd, GWLP_USERDATA, 0);
            let _ = DestroyWindow(self.hwnd);
            crate::register::dbg_log("popup_rev: message-only HWND destroyed");
        }
        // _ctx Box 는 자동 Drop.
    }
}

/// 역채널 message-only 창 프로시저 — `WM_UNIM_REV` 에서 큐를 drain 해 엔진 적용.
extern "system" fn rev_wnd_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    if msg == WM_UNIM_REV {
        // panic 격리 — wndproc 에서 panic 이 COM/STA 경계를 넘지 않도록.
        let _ = std::panic::catch_unwind(|| unsafe {
            let ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *const RevWndContext;
            if ptr.is_null() {
                return;
            }
            let ctx = &*ptr;
            rev_drain_and_apply(ctx);
        });
        return LRESULT(0);
    }
    unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) }
}

/// 역채널 큐를 drain 해 각 이벤트를 엔진에 적용 (TSF 스레드, 엔진 락 짧게).
fn rev_drain_and_apply(ctx: &RevWndContext) {
    let events = ctx.rev.drain();
    if events.is_empty() {
        return;
    }
    // stale 판정용 식별자 (마지막 send_render).
    let (last_owner, last_seq) = {
        let p = ctx.popup_ipc.lock().unwrap();
        p.last_render_ident()
    };

    for env in events {
        // 락 순서: engine → config → composition → popup_ipc (다른 경로와 동일).
        let mut engine = ctx.engine.lock().unwrap();
        let config = ctx.config.lock().unwrap();
        let mut comp_mgr = ctx.composition_mgr.lock().unwrap();
        let mut popup = ctx.popup_ipc.lock().unwrap();
        // last_context 는 별도 락(컨텍스트만) — 위 락 이후 취득.
        let ctx_guard = ctx.last_context.lock().unwrap();
        let context_ref = ctx_guard.as_ref();

        key_handler::apply_reverse_event(
            &mut engine,
            &config,
            &mut comp_mgr,
            &mut popup,
            context_ref,
            ctx.tid,
            &ctx.comp_sink,
            &env,
            last_owner,
            last_seq,
        );
        // 락은 루프 끝에서 자동 해제 (다음 이벤트 위해 재취득 — 엔진 락 짧게 유지).
    }
}
