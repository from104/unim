//! UnimTextService — TSF 메인 구조체

use std::collections::HashSet;
use std::sync::atomic::{AtomicBool, AtomicIsize, AtomicU32, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Instant, SystemTime};

use windows::core::*;
use windows::Win32::Foundation::*;
use windows::Win32::UI::Input::KeyboardAndMouse::GetFocus;
use windows::Win32::UI::TextServices::*;

use unim::config::{CommitUnit, Config, InputCategory};
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
    /// D3 — 활성 context 에 건 `ITfTextEditSink` advise cookie 와 그 context.
    /// (cookie, context) 를 보관해 context 변경/Deactivate 시 같은 context 의
    /// `ITfSource::UnadviseSink(cookie)` 로 정확히 해제한다. context 가 바뀌면
    /// 먼저 옛 것을 Unadvise 후 새 것을 Advise(누수·이중 advise 방지).
    pub(crate) text_edit_sink: Mutex<Option<(u32, ITfContext)>>,
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

    /// b1 Phase2 — 신규 조합 즉시-terminate 학습 억제 플래그.
    ///
    /// b1 Phase2 가 시작한 신규 TSF 조합("기")을 Blink 가 즉시 terminate 할 수 있다.
    /// 그 단발 terminate 를 OnCompositionTerminated 의 by_time(<200ms) 학습이 cuas_windows
    /// 에 등록하면, 이후 같은 Blink 창에서 정상 한글 입력까지 영구 폴백(오버레이)으로
    /// 오염된다(회귀). insert_pending 직전에 set, 직후 clear 해 그 사이의 terminate 만
    /// 학습 대상에서 제외한다(by_time 자체는 폴백 진입에 그대로 사용). Arc 로 보관해
    /// 타이머 wndproc(RevWndContext)과 공유한다.
    pub(crate) suppress_cuas_learn: Arc<AtomicBool>,

    /// 단어별 preedit Phase 3a — 매-키 Word 게이트 메모이즈 캐시(포그라운드 HWND).
    ///
    /// `OnKeyDown` 의 매-키 게이트 재적용(`reapply_word_gate`)에서, 직전 키와 같은
    /// 포그라운드 창이면 OpenProcess 를 생략하고 캐시된 word 판정을 재사용한다(매 키
    /// 시스템콜 비용 회피). HWND 가 바뀌면 프로세스명을 재취득해 갱신한다. 초기값 0.
    pub(crate) last_fg_hwnd: AtomicIsize,
    /// 위 `last_fg_hwnd` 에 대응하는 캐시된 word 모드 판정(=프로세스가 winword.exe).
    pub(crate) last_fg_word: AtomicBool,

    /// 직전 키 입력 시점의 포그라운드 프로세스 basename(소문자). Excel 셀 첫타 ATF
    /// 유실 게이트(`atf_reset_should_skip`)의 `same_proc` 판정에 쓴다.
    ///
    /// `reapply_word_gate` 의 HWND 메모이즈와 동기로(=HWND 변경 시에만) 갱신한다. 키
    /// 경로에서만 기록되고 OnSetFocus 두 곳은 읽기만 하므로, 두 OnSetFocus 가 서로의
    /// 값을 덮어써 정상 앱 전환을 same_proc=true 로 오판하는 회귀가 원천 차단된다.
    pub(crate) last_key_proc: Mutex<Option<String>>,
}

/// b1 Phase2 — SetTimer one-shot 식별자.
const FLUSH_TIMER_ID: usize = 0xB1;
/// b1 Phase2 — BS×N 이 Blink 문서에 적용될 시간을 보수적으로 기다린다(ms).
///
/// BS 는 sink 로 echo 되지 않고 Blink 문서로 직행하므로 "적용 완료"를 직접 관찰할
/// 수단이 없다(센티널 sink 왕복은 적용 전 도착 — v0.3.29 실패). 따라서 시간 하한으로
/// 기다린다.
///
/// D3: 주 경로가 OnEndEdit + read-back 검증(이벤트 기반)으로 바뀌었으므로 이 타이머는
/// **안전망**(OnEndEdit 미advise/미발사·read-back stale 시 폴백)으로 역할이 바뀐다.
/// 주 경로가 즉시 발화하므로 값을 키워도 체감 지연이 없어, 느린 머신/RDP 안전을 위해
/// 40 → 60ms 로 키운다(안전망 폴백 시점만 늦춰지고 정상 경로는 OnEndEdit 가 선행).
const FLUSH_DELAY_MS: u32 = 60;

/// D3 — OnEndEdit read-back 검증 통과 시 wndproc 에 보내는 "즉시 flush" 메시지.
/// OnEndEdit ec 는 read-only 이므로 삽입(RW)을 직접 못 한다 → PostMessage 로 wndproc
/// (별도 RW edit session)에 위임한다. WM_APP(0x8000)+0x22 — WM_UNIM_REV(+0x21) 와 구분.
const WM_UNIM_FLUSH: u32 = 0x8000 + 0x22;

/// b1[D] Phase2b — 방식 D 펌프-분할 2단계 메시지. WM_UNIM_FLUSH(Phase2a)가 세션A 조합을
/// 만든 뒤, 이 메시지를 PostMessage 해 **메시지 펌프를 한 번 돌린 뒤** 세션B
/// (commit_and_restart)를 별도 RW edit session 으로 실행한다. 펌프가 세션A 조합의 문서
/// 등록 경계를 만들어 세션B commit 이 크롬에서 정착하게 한다. WM_APP(0x8000)+0x23.
const WM_UNIM_FLUSH2: u32 = 0x8000 + 0x23;

/// R6b — PENDING==0 게이트 → 라이브 꼬리 조합. OnTestKeyDown(edit session 불가)이 PostMessage 로
/// wndproc 에 위임한다(WM_UNIM_FLUSH 동일 패턴). WM_APP(0x8000)+0x24
/// (REV=+0x21, FLUSH=+0x22, FLUSH2=+0x23).
const WM_UNIM_TAIL: u32 = 0x8000 + 0x24;

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
            text_edit_sink: Mutex::new(None),
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
            suppress_cuas_learn: Arc::new(AtomicBool::new(false)),
            last_fg_hwnd: AtomicIsize::new(0),
            last_fg_word: AtomicBool::new(false),
            last_key_proc: Mutex::new(None),
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
            if comp_mgr.english_hold_active() {
                // Phase 4 — 보유 영문 라이브 조합(영문은 engine preedit 없음)을 영문 그대로 확정.
                // end_composition(None)으로 종료하면 range 텍스트가 비워져 보유 영문이 사라지므로,
                // 반드시 보유 문자열로 확정한다.
                comp_mgr.confirm_english_hold(context, tid);
            } else if comp_mgr.is_active() {
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

    /// b1 Phase2 — 보류 삽입(PendingInsert)을 TSF 로 삽입한다.
    ///
    /// 타이머 발화(WM_TIMER)·race-flush(다음 키)·degrade(타이머 호스트 부재) 공용
    /// 진입점. 어떤 락도 보유하지 않은 상태에서 호출해야 한다(내부에서 engine→
    /// composition 락을 OnKeyDown 과 동일 순서로 짧게 취득). SendInput 미사용이라
    /// edit session 안전. 보류 삽입이 없으면 즉시 false(중복 호출 무해, 1회성).
    ///
    /// Phase2 가 시작하는 신규 조합의 즉시-terminate 가 cuas_windows 를 오염시키지
    /// 않도록 insert 동안 suppress_cuas_learn 을 set 한다.
    ///
    /// comp_sink 는 호출자(_Impl 블록)가 `to_interface()` 로 만들어 전달한다
    /// (to_interface 는 `_Impl` 타입에만 존재).
    pub(crate) fn flush_pending_insert(&self, comp_sink: &ITfCompositionSink) -> bool {
        if !crate::synth_input::has_pending_insert() {
            return false;
        }
        let ctx_guard = self.last_context.lock().unwrap();
        let Some(context) = ctx_guard.as_ref() else {
            // 컨텍스트 부재(포커스 이탈 등) → stale 삽입 방지: 폐기.
            let _ = crate::synth_input::discard_pending_insert();
            crate::register::dbg_log("b1: flush no context → pending 폐기");
            return false;
        };
        let context = context.clone();
        drop(ctx_guard);

        let tid = self.client_id();
        // 락 순서: engine → composition (다른 경로와 동일 접두).
        let mut engine = self.engine.lock().unwrap();
        let mut comp_mgr = self.composition_mgr.lock().unwrap();
        // Phase2 신규 조합의 단발 terminate 를 학습에서 제외.
        self.suppress_cuas_learn.store(true, Ordering::SeqCst);
        let did = key_handler::flush_pending_insert(
            &mut engine,
            &mut comp_mgr,
            &context,
            tid,
            comp_sink,
        );
        // b1[D] Phase2b: race-flush(다음 키 선점)·degrade(rev_window 부재) 경로는 메시지
        // 펌프를 끼울 수 없으므로 세션B 를 동기로(같은 틱) 실행한다 — 펌프-분할 효능은 못
        // 받지만 현행과 동등(회귀 아님). 주(主) 경로(WM_UNIM_FLUSH→WM_UNIM_FLUSH2)는
        // rev_wnd_proc 가 PostMessage 로 펌프를 끼워 정상 분할한다.
        if crate::synth_input::has_pending_restart() {
            key_handler::flush_restart_phase_b(&mut engine, &mut comp_mgr, &context, tid, comp_sink);
        }
        self.suppress_cuas_learn.store(false, Ordering::SeqCst);
        did
    }

    /// b1[D] Phase2b — 보류 재시작(PENDING_RESTART)을 동기로 확정한다.
    ///
    /// race-flush 가 Phase2b 펌프(WM_UNIM_FLUSH2) 대기 중에 다음 키를 받았을 때, 새 키
    /// 처리 **전에** 세션B 를 선행해 stale 세션A 조합 오염·PENDING_RESTART 덮어쓰기를
    /// 막는다. 보류 재시작이 없으면 즉시 false(1회성, 중복 무해). 어떤 락도 보유하지
    /// 않은 상태에서 호출(내부에서 engine→composition 락 짧게). SendInput 미사용.
    pub(crate) fn flush_restart_phase_b(&self, comp_sink: &ITfCompositionSink) -> bool {
        if !crate::synth_input::has_pending_restart() {
            return false;
        }
        let ctx_guard = self.last_context.lock().unwrap();
        let Some(context) = ctx_guard.as_ref() else {
            let _ = crate::synth_input::discard_pending_restart();
            crate::register::dbg_log("b1[D]: Phase2b flush no context → restart 폐기");
            return false;
        };
        let context = context.clone();
        drop(ctx_guard);

        let tid = self.client_id();
        // 락 순서: engine → composition (다른 경로와 동일 접두).
        let mut engine = self.engine.lock().unwrap();
        let mut comp_mgr = self.composition_mgr.lock().unwrap();
        self.suppress_cuas_learn.store(true, Ordering::SeqCst);
        let did = key_handler::flush_restart_phase_b(
            &mut engine,
            &mut comp_mgr,
            &context,
            tid,
            comp_sink,
        );
        self.suppress_cuas_learn.store(false, Ordering::SeqCst);
        did
    }

    /// R6b — 보류 꼬리를 동기 처리한다(race-flush·rev_window 부재 degrade 공용). 어떤 락도
    /// 보유하지 않은 상태에서 호출(내부에서 engine→composition 락 짧게). SendInput(degrade)은
    /// edit session 밖. 반환: degrade(확정)했으면 true, 라이브 유지면 false.
    pub(crate) fn flush_pending_tail(&self, comp_sink: &ITfCompositionSink) -> bool {
        if !crate::synth_input::has_pending_tail() {
            return false;
        }
        let ctx_guard = self.last_context.lock().unwrap();
        let Some(context) = ctx_guard.as_ref() else {
            // 컨텍스트 부재(포커스 이탈) = 허용된 유일 손실 케이스 → 꼬리 폐기.
            let _ = crate::synth_input::discard_pending_tail();
            crate::register::dbg_log("synth tail: flush no context → 꼬리 폐기(포커스 이탈)");
            return false;
        };
        let context = context.clone();
        drop(ctx_guard);
        let tid = self.client_id();
        // 락 순서: engine → composition (다른 경로와 동일 접두).
        let mut engine = self.engine.lock().unwrap();
        let mut comp_mgr = self.composition_mgr.lock().unwrap();
        // 라이브 꼬리 단발 terminate 를 학습에서 제외.
        self.suppress_cuas_learn.store(true, Ordering::SeqCst);
        let did =
            key_handler::flush_pending_tail(&mut engine, &mut comp_mgr, &context, tid, comp_sink);
        self.suppress_cuas_learn.store(false, Ordering::SeqCst);
        did
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

    /// 단어별 preedit Phase 3a — 매-키 Word 전용 게이트 재적용.
    ///
    /// `OnKeyDown` 에서 `maybe_reload_config` 직후·엔진 키처리 직전에 호출한다.
    /// ThreadMgr `OnSetFocus` 게이트가 "이미 포커스된 채 TIP 활성 → OnSetFocus 미발화"
    /// 또는 config hot-reload 의 엔진 재생성(word 모드 리셋)으로 무력화돼도, 매 키 직전
    /// word 모드를 재보장한다.
    ///
    /// 비용 회피: 포그라운드 HWND 로 메모이즈한다. 직전 키와 같은 HWND 면 OpenProcess 를
    /// 생략하고 캐시된 word 판정을 재사용한다(단어 중엔 같은 앱이므로 재호출 0). HWND 가
    /// 바뀐 키(앱 전환)에서만 `process_basename_for_hwnd` 로 프로세스명을 재취득한다.
    ///
    /// 단어모드 desired 는 **config `commit_unit`** 이 결정한다(설정 존중):
    ///   - `Syllable` → 항상 false(단어모드 기능 비활성, 모든 앱 음절).
    ///   - `Word` → true. 단 composition 미지원(CUAS/즉시-terminate 학습) 앱은 false
    ///     강제(영문보유 B2·조합 미지원 회귀 차단) — 비협조앱은 Word 값이어도 syllable.
    ///   - `Smart` → 보수적 앱 화이트리스트만 true(winword.exe), 그 외 false.
    ///
    /// 무회귀: 종전 winword 하드코딩과 동치인 `Smart` 가 기본값이라 출고 동작 보존.
    /// `set_word_mode` 는 desired 가 현재와 다를 때만 호출(불필요 토글·flush 회피).
    ///
    /// 동시성: Win32 호출(GetForegroundWindow/OpenProcess 등)과 CUAS 판정(cuas_windows
    /// 락)은 **엔진 락 밖**에서 먼저 수행하고, 짧게 엔진 락을 잡아 `commit_unit` 을 읽고
    /// `set_word_mode` 만 호출한다(OnKeyDown 의 엔진 락 취득 전에 위치).
    fn reapply_word_gate(&self) {
        use windows::Win32::UI::WindowsAndMessaging::GetForegroundWindow;

        // 1) Win32: 포그라운드 HWND 취득(엔진 락 밖).
        let hwnd = unsafe { GetForegroundWindow() };
        let hwnd_isize = hwnd.0 as isize;
        let prev_hwnd = self.last_fg_hwnd.load(Ordering::Relaxed);

        // 2) winword 판정: HWND 동일 → 캐시 재사용(OpenProcess 생략), 변경 → 재취득+로그.
        let is_winword = if hwnd_isize == prev_hwnd {
            self.last_fg_word.load(Ordering::Relaxed)
        } else {
            let name = process_basename_for_hwnd(hwnd);
            let w = name
                .as_deref()
                .map(|n| n == "winword.exe")
                .unwrap_or(false);
            self.last_fg_hwnd.store(hwnd_isize, Ordering::Relaxed);
            self.last_fg_word.store(w, Ordering::Relaxed);
            crate::register::dbg_log(&format!(
                "word-gate: proc='{}' hwnd=0x{:x} → winword={} (HWND 변경 → 캐시 갱신)",
                name.as_deref().unwrap_or("<취득실패>"),
                hwnd_isize,
                w
            ));
            // Excel 셀 첫타 ATF 게이트용: 직전 키의 포그라운드 프로세스명을 HWND 메모이즈와
            // 동기로 기록한다(취득 실패 시 None → same_proc=false 로 보수적 폴백, 무회귀).
            *self.last_key_proc.lock().unwrap() = name;
            w
        };

        // 3) composition 미지원(CUAS/즉시-terminate 학습) 판정 — 엔진 락 밖에서 먼저.
        //    Word 설정이어도 비협조앱이면 syllable 로 강제한다. 현재 컨텍스트 폴백
        //    플래그가 true 면 즉시 단정(cuas 락 생략), 아니면 학습 집합을 조회한다.
        let comp_unsupported = self.composition_unsupported.load(Ordering::SeqCst)
            || (hwnd_isize != 0 && self.cuas_windows.lock().unwrap().contains(&hwnd_isize));

        // 4) 엔진 락(짧게): commit_unit 설정에 따라 desired 판정 → 다를 때만 토글.
        let mut engine = self.engine.lock().unwrap();
        let commit_unit = engine.commit_unit();
        let desired_word = match commit_unit {
            CommitUnit::Syllable => false,
            CommitUnit::Word => !comp_unsupported,
            CommitUnit::Smart => is_winword,
        };
        let was = engine.is_word_mode();
        if was != desired_word {
            crate::register::dbg_log(&format!(
                "word-gate: set_word_mode({}) (commit_unit={:?} winword={} comp_unsupported={} was={})",
                desired_word, commit_unit, is_winword, comp_unsupported, was
            ));
            engine.set_word_mode(desired_word);
        }
    }

    /// Excel 셀 첫타 ATF 유실 대응 — 스퓨리어스 OnSetFocus 의 ATF 버퍼 reset 을 스킵할지
    /// 판정한다(+ 진단 로그). **확신 낮음(가설)** — 로그로 실측 확정하기 위함.
    ///
    /// 가설: Excel 은 셀 Ready→Enter(편집 진입) 전이 때 직전 키 직후 같은 프로세스로
    /// 스퓨리어스 TSF OnSetFocus 를 발화한다. 그 reset_on_focus 가 ATF 키스트로크
    /// 버퍼를 비우면 `forward.rs` 의 `len() < 2` 게이트가 미충족돼 첫 타 순방향(영→한)
    /// 교정이 유실되거나 첫 글자가 빠진 오교정이 된다.
    ///
    /// 3중 AND 게이트(셋 다 충족 시에만 skip=true → ATF 버퍼 clear 만 보류):
    ///   (a) `same_proc`: 현재 포커스 프로세스 == 직전 키의 포그라운드 프로세스
    ///       (앱 전환이 아님). 기준값은 키 경로에서만 기록되므로 두 OnSetFocus 가
    ///       서로를 덮어쓰는 회귀가 없다.
    ///   (b) `dt_since_key_ms < 250`: 직전 키 직후 발생한 스퓨리어스 포커스.
    ///   (c) `buf_len > 0`: 보존할 키스트로크가 실제로 있음.
    ///
    /// 셋 중 하나라도 어긋나면(다른 프로세스·오래된 키·빈 버퍼) false → 호출부가
    /// 기존대로 reset 한다(정상 앱 전환·포커스 이탈 무회귀). 반환과 무관하게 항상
    /// dbg_log 로 세 값과 판정을 남긴다(가설 검증용).
    ///
    /// 주의: skip 해도 `engine.reset()` 등 다른 정리는 호출부에서 그대로 수행된다.
    /// 순방향(영문 모드)은 키가 조합 없이 즉시 commit 되어 엔진에 보존할 조합 상태가
    /// 없으므로(문서에 이미 N 자 존재) ATF 버퍼만 살리면 정합한다.
    fn atf_reset_should_skip(&self, cur_proc: Option<&str>) -> bool {
        let same_proc = {
            let last = self.last_key_proc.lock().unwrap();
            match (last.as_deref(), cur_proc) {
                (Some(prev), Some(cur)) if !prev.is_empty() => prev == cur,
                _ => false,
            }
        };
        let dt_since_key_ms = self
            .last_key_instant
            .lock()
            .unwrap()
            .map(|t| t.elapsed().as_millis())
            .unwrap_or(u128::MAX);
        let buf_len = self.atf_state.lock().unwrap().buf_len();
        let skip = same_proc && dt_since_key_ms < 250 && buf_len > 0;
        crate::register::dbg_log(&format!(
            "atf reset_on_focus: same_proc={} dt_since_key_ms={} buf_len={} → skip={}",
            same_proc, dt_since_key_ms, buf_len, skip
        ));
        skip
    }
}

// ── D3: ITfTextEditSink advise 헬퍼 (to_interface 가 _Impl 타입에만 존재) ──

impl UnimTextService_Impl {
    /// D3 — 주어진 context 에 `ITfTextEditSink` 를 advise 한다(이미 같은 context 면 무시).
    ///
    /// context 가 바뀌었으면 옛 것을 먼저 Unadvise 한 뒤 새 것을 Advise 해 이중 advise·
    /// 누수를 막는다. 실패해도 IME 무영향(D3 게이트만 비활성 = 타이머 단독 폴백).
    /// OnKeyDown 진입(매 키)에서 호출 — context 동일 시 atomic 비교 후 즉시 반환하므로 싸다.
    fn advise_text_edit_sink(&self, context: &ITfContext) {
        {
            // 이미 같은 context 에 advise 돼 있으면 재advise 불필요.
            let cur = self.text_edit_sink.lock().unwrap();
            if let Some((_, ref c)) = *cur {
                if c == context {
                    return;
                }
            }
        }
        // 옛 advise 해제(context 변경).
        self.unadvise_text_edit_sink();
        unsafe {
            let Ok(source) = context.cast::<ITfSource>() else {
                return;
            };
            let sink: ITfTextEditSink = self.to_interface();
            match source.AdviseSink(&ITfTextEditSink::IID, &sink) {
                Ok(cookie) => {
                    *self.text_edit_sink.lock().unwrap() = Some((cookie, context.clone()));
                    crate::register::dbg_log("D3: ITfTextEditSink advised on active context");
                }
                Err(e) => {
                    crate::register::dbg_log(&format!(
                        "D3: AdviseSink(ITfTextEditSink) failed: {e:?}"
                    ));
                }
            }
        }
    }

    /// D3 — advise 된 `ITfTextEditSink` 를 (저장된 context 의 source 로) 해제한다.
    fn unadvise_text_edit_sink(&self) {
        if let Some((cookie, context)) = self.text_edit_sink.lock().unwrap().take() {
            unsafe {
                if let Ok(source) = context.cast::<ITfSource>() {
                    let _ = source.UnadviseSink(cookie);
                }
            }
            crate::register::dbg_log("D3: ITfTextEditSink unadvised");
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
                suppress_cuas_learn: Arc::clone(&self.suppress_cuas_learn),
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
        // D3: ITfTextEditSink advise 해제(누수·stale sink 방지).
        self.unadvise_text_edit_sink();
        crate::synth_input::disarm_readback_gate();

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
        //
        // Excel 셀 첫타 ATF 유실 대응(가설·확신낮음): 직전 키 직후 같은 프로세스로 발생한
        // 스퓨리어스 포커스(3중 게이트)면 ATF 버퍼 reset 을 스킵해 첫 타를 보존한다.
        // 게이트 밖(정상 앱 전환·포커스 이탈)은 기존대로 reset(무회귀).
        let cur_proc = foreground_process_basename();
        if !self.atf_reset_should_skip(cur_proc.as_deref()) {
            self.atf_state.lock().unwrap().reset_on_focus();
        }
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
            // R6b 게이트: 머리 배치 echo 전량 복귀(PENDING==0) + 보류 꼬리 존재 → 라이브 꼬리
            // 트리거. OnTestKeyDown 은 edit session 불가 → wndproc(WM_UNIM_TAIL)에 위임
            // (1회성 take 로 중복 무해).
            if crate::synth_input::tail_gate_ready() {
                if let Some(w) = self.rev_window.lock().unwrap().as_ref() {
                    w.post_tail();
                }
            }
            return Ok(BOOL::from(eaten));
        }
        // R6b: 합성 echo 가 아닌 실제 사용자 키 관찰 시점 → fragile 라이브 꼬리 가드
        // 해제(M-3). 연장(update_composition)·passthrough-commit(navkey)·무관키 전부 공통 통과.
        crate::synth_input::clear_tail_live();
        let mut engine = self.engine.lock().unwrap();
        let config = self.config.lock().unwrap();
        let popup_active = self.popup_ipc.lock().unwrap().is_active();
        // Phase 4 — 보유 영문 라이브 조합이 떠 있으면(영문은 engine.is_composing()=false 이므로)
        // 경계/네비게이션/조합단축 패스쓰루 게이트가 이를 조합처럼 취급해 먼저 영문 확정하게 한다.
        // (lock 순서 engine→composition 준수 — OnKeyDown 과 동일 접두.)
        let has_english_hold = self.composition_mgr.lock().unwrap().english_hold_active();

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
        //   - is_modifier() 단독(Ctrl/Shift/Alt/Win 누름 자체) → 제외.
        //
        // ⚠ 핵심 불변식: **수정자 키 자체의 키다운(Shift/Ctrl/Alt/Win down)에서는
        // 절대 커밋하지 않는다.** Windows 는 키다운에서 좌/우 구분 없는 제네릭
        // VK_SHIFT(0x10)/VK_CONTROL(0x11)/VK_MENU(0x12) 를 보내는데, 이를 KeyCode
        // 로 매핑하지 않으면 Unknown 이 되어 is_modifier()=false → 아래 is_combo 가
        // (m.shift && !Unknown.is_character_key()) 로 true 가 되어 조합 중 한글이
        // 커밋됐다(세벌식 시프트-자모 분리: '가'+Shift+ㅌ → '가ㅌ'). from_win32_vk
        // 에서 0x10/0x11/0x12 를 수정자로 매핑(conversion.rs)해 kc.is_modifier()=
        // true 가 되므로 is_combo=false 가 되어 조합이 보존된다. 그래도 안전망으로
        // 제네릭 VK 도 명시 차단한다(매핑 회귀 방어).
        let m = key_handler::get_modifier_state();
        let raw_vk = wparam.0 as u16;
        // 한/영 토글키(우Alt 등 — 그 자체가 수정자인 키도 toggle_keys 에 지정 가능)는
        // 수정자 단독 패스쓰루보다 먼저 판정해, 토글 동작이 죽지 않도록 한다.
        // (선례: '오른쪽 Alt 한영 토글' 결함 — toggle 검사를 가드 앞으로.)
        let is_toggle = engine.is_toggle_key(kc);
        // 수정자 키 자체의 키다운(제네릭 0x10/0x11/0x12 포함)은 조합 경계가 아니다.
        let is_bare_modifier =
            (kc.is_modifier() || matches!(raw_vk, 0x10 | 0x11 | 0x12)) && !is_toggle;
        let is_combo = !is_bare_modifier
            && !is_toggle
            && (m.control || m.alt || m.super_key || (m.shift && !kc.is_character_key()));
        if !popup_active && (engine.is_composing() || has_english_hold) && is_bare_modifier {
            // 조합 중 수정자 단독 키다운: 조합을 깨지 않고 투명 통과(시프트-자모가
            // 같은 음절에 결합되도록 보존). 보유 영문(Phase 4)도 동일 — Shift 단독 키다운
            // (대문자 입력 등)에서 보유를 깨지 않는다. 토글키(우Alt 등)는 is_toggle 로 제외돼
            // 아래 test_key_down 의 is_toggle 분기가 소비/토글을 처리한다.
            crate::register::dbg_log(&format!(
                "OnTestKeyDown: modifier keydown 조합 보존(투과) vk=0x{:02X}",
                raw_vk
            ));
            return Ok(FALSE);
        }
        if !popup_active && (engine.is_composing() || has_english_hold) && is_combo {
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
        if !popup_active
            && (engine.is_composing() || has_english_hold)
            && key_handler::is_commit_passthrough_key(kc)
        {
            if let Some(context) = pic.as_ref() {
                self.commit_for_passthrough(&mut engine, context);
                crate::register::dbg_log(&format!(
                    "OnTestKeyDown: commit+passthrough navkey vk=0x{:02X}",
                    wparam.0 as u16
                ));
                return Ok(FALSE);
            }
        }

        // ── 버그①: 조합 중 넘패드(NumLock ON) 숫자/기호 패스쓰루 ──
        //
        // VK_NUMPAD0..9·MULTIPLY·ADD·SEPARATOR·SUBTRACT·DECIMAL·DIVIDE(0x60-0x6F)는
        // from_win32_vk 가 미매핑 → KeyCode::Unknown → is_character_key()=false 라,
        // 위 문자/네비/커밋 패스쓰루 게이트를 전부 통과하지 못한다. 그러면 OnKeyDown 이
        // 호출되지 않아 조합이 확정되지 않은 채 넘패드 문자가 앱에 먼저 들어가 순서가
        // 역전된다(한글 잔류 + 숫자/기호 선입력). navkey 와 동일하게 현재 조합을 먼저
        // 확정하고 pIsEaten=FALSE 로 통과시켜 "한글 확정 → 넘패드 문자" 순서를 맞춘다.
        // (Ctrl/Alt/Shift+넘패드 등 수정자 조합은 위 is_combo 분기가 이미 동일하게
        //  커밋+패스쓰루하므로 여기엔 수정자 없는 순수 넘패드만 도달한다. NumLock OFF
        //  면 VK 가 0x60-0x6F 가 아니라 VK_HOME/VK_LEFT 등이라 위 navkey 가 처리한다.)
        if !popup_active
            && (engine.is_composing() || has_english_hold)
            && key_handler::is_numpad_vk(raw_vk)
        {
            if let Some(context) = pic.as_ref() {
                self.commit_for_passthrough(&mut engine, context);
                crate::register::dbg_log(&format!(
                    "OnTestKeyDown: commit+passthrough numpad vk=0x{:02X}",
                    raw_vk
                ));
                return Ok(FALSE);
            }
        }

        // ── Phase 4(S1): 보유 영문 라이브 조합 중 Backspace 소비 ──
        //
        // 보유 영문(committed=0 라이브 조합) 편집용 Backspace 는 위 패스쓰루 게이트(수정자/
        // 조합/commit-passthrough)에 모두 해당하지 않아 아래 test_key_down 으로 떨어지는데,
        // 영문은 engine.is_composing()=false 라 거기서도 미소비(eaten=FALSE)→앱 직행→OnKeyDown
        // 미호출이 된다. 그러면 english_hold 가 불변이라 백스페이스가 무효화되고 desync 가 난다.
        // 여기서 소비(eaten=TRUE)해 OnKeyDown(handle_key_down)이 호출되도록 보장한다(거기서
        // english_hold pop + ATF 버퍼 동기 축소). Shift+Backspace 등 수정자 조합은 위 is_combo
        // 분기가 이미 처리(영문 확정 + 패스쓰루)하므로 여기엔 순수 Backspace 만 도달한다.
        if !popup_active && has_english_hold && kc == unim::keycode::KeyCode::Backspace {
            crate::register::dbg_log(
                "OnTestKeyDown: 보유 영문 Backspace 소비(eaten=TRUE → OnKeyDown 보장)",
            );
            return Ok(TRUE);
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
        // D3: 활성 context 에 ITfTextEditSink 를 advise(OnEndEdit read-back 게이트용).
        // context 동일 시 즉시 반환하므로 매 키 호출 비용은 미미하다.
        self.advise_text_edit_sink(context);
        // ATF SendInput 폴백의 합성 BS 키 처리(b1 Phase1 BS×N 복귀분).
        match crate::synth_input::observe_key_down(wparam.0 as u16) {
            Some(crate::synth_input::SynthKeyAction::PassThrough) => return Ok(FALSE),
            None => {}
        }

        // ── b1 race-flush 가드 ──────────────────────────────────────────────
        // Phase2 타이머(FLUSH_DELAY_MS) 발화 전에 사용자가 다음 키를 누르면, 보류
        // 삽입을 **먼저** 완료하고 타이머를 끈다 → 다음 글자가 "기" 앞에 삽입되는
        // 순서 역전을 막는다. 1회성(take)이라 중복 무해. 이 키는 이 합성키가 아닌
        // 사용자 키이므로(observe_key_down 통과) 정상 처리를 이어간다.
        //
        // ⚠ 수정자 단독 키다운(Shift/Ctrl/Alt/Win down)에서는 flush 하지 않는다
        //   (v0.3.31 조합보존 불변식 보호 — 아래 is_bare_modifier 판정과 일관).
        //   따라서 이 가드는 maybe_reload_config·락 취득보다는 뒤, 그러나 실제
        //   엔진 처리(handle_key_down) 앞에 둔다. 여기서는 우선 수정자 판정만 하고
        //   비수정자일 때 flush 한다.
        // has_pending_insert(Phase2a 미완) 또는 has_pending_restart(Phase2b 펌프 대기)
        // 어느 쪽이든, 다음 키 처리 전에 보류분을 완료한다. restart 만 남은 상태
        // (Phase2a 는 끝났고 WM_UNIM_FLUSH2 대기 중)에서 새 키가 stale 세션A 조합 위로
        // 떨어지거나 PENDING_RESTART 가 덮어써지는 fast-typing 레이스를 막는다.
        if crate::synth_input::has_pending_insert()
            || crate::synth_input::has_pending_restart()
            || crate::synth_input::has_pending_tail()
        {
            let raw_vk = wparam.0 as u16;
            let kc = unim::keycode::KeyCode::from_win32_vk(raw_vk);
            let is_bare_modifier = kc.is_modifier() || matches!(raw_vk, 0x10 | 0x11 | 0x12);
            if !is_bare_modifier {
                // 타이머 선점: 먼저 끄고(없으면 무해) 보류 삽입·재시작·꼬리를 완료한다.
                if let Some(w) = self.rev_window.lock().unwrap().as_ref() {
                    w.kill_flush_timer();
                }
                let comp_sink: ITfCompositionSink = self.to_interface();
                // Phase2a 보류분(있으면 A+동기 B) → 이어서 Phase2b 보류분(펌프 대기 중이던
                // 세션B). 둘 다 1회성(take)이라 중복·이중 호출 무해.
                self.flush_pending_insert(&comp_sink);
                self.flush_restart_phase_b(&comp_sink);
                // R6b: 보류 꼬리 선점 — PENDING==0 이면 라이브 조합 성립(이어지는
                // handle_key_down 이 같은 키로 "다"→"단" 연장), PENDING>0 이면 FIFO degrade.
                self.flush_pending_tail(&comp_sink);
                crate::register::dbg_log(&format!(
                    "b1: race-flush (다음 키 선점) vk=0x{:02X}",
                    raw_vk
                ));
            }
        }
        // OnCompositionTerminated 의 "즉시 종료" 판별용 타임스탬프.
        *self.last_key_instant.lock().unwrap() = Some(Instant::now());
        // 설정 GUI 가 config.yaml 을 저장하면 포커스 전환 없이도 다음 키 입력 시
        // 즉시 반영. 스로틀(300ms) + 조합 중 미룸으로 글자 깨짐·디스크 부담 방지.
        // 반드시 아래 락 취득 전에 호출(내부에서 동일 락을 잡으므로 재진입 데드락 방지).
        self.maybe_reload_config(true);
        // 단어별 preedit Phase 3a — 매-키 Word 게이트 재적용. maybe_reload_config(엔진
        // 재생성으로 word 모드 리셋 가능) 직후·엔진 키처리 직전에 word 모드를 재보장한다.
        // 내부에서 엔진 락을 짧게 잡으므로 아래 락 취득보다 반드시 앞에 둔다(재진입 방지).
        self.reapply_word_gate();
        // Phase 4 — 포커스 창이 학습된 CUAS(즉시-terminate 브리지)인지 판정(영문 라이브 보유
        // 게이트). cuas_windows 는 GetFocus().0 로 키잉되므로 동일 기준 사용. 시스템콜이라
        // 엔진 락 밖에서 먼저 구한다(락 보유 중 시스템콜 금지). Word 는 미등록 불변식 → false.
        let known_cuas = {
            let focus_hwnd = unsafe { GetFocus() }.0 as isize;
            focus_hwnd != 0 && self.cuas_windows.lock().unwrap().contains(&focus_hwnd)
        };
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

        let outcome = key_handler::handle_key_down(
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
            known_cuas,
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

        // ── b1 Phase2 예약 ──────────────────────────────────────────────────
        // synth 폴백이 Phase1(BS×N)만 했고 삽입은 보류 중이다. 락을 모두 해제한 뒤
        // SetTimer 로 Phase2(삽입)를 예약한다. SendInput·BS 가 Blink 문서에 적용될
        // 시간(FLUSH_DELAY_MS)을 벌어 삭제↔삽입이 같은 배치에 안 섞이게 한다.
        if outcome.schedule_flush {
            // engine/comp/popup/preedit/atf 락 해제(SetTimer·degrade 시 flush 가
            // 동일 락을 재취득하므로 데드락 방지).
            drop(atf_state);
            drop(preedit_win);
            drop(popup_ipc);
            drop(comp_mgr);
            drop(config);
            drop(engine);

            let armed = {
                let rev = self.rev_window.lock().unwrap();
                match rev.as_ref() {
                    Some(w) => {
                        w.arm_flush_timer(FLUSH_TIMER_ID, FLUSH_DELAY_MS);
                        true
                    }
                    None => false,
                }
            };
            if armed {
                crate::register::dbg_log(&format!(
                    "b1: SetTimer armed ({}ms) for Phase2 insert",
                    FLUSH_DELAY_MS
                ));
            } else {
                // 타이머 호스트(message-only HWND) 부재 → degrade: 동기 즉시 삽입.
                // 분리-플러시의 시간 지연은 못 벌지만 삽입 누락 0 을 보장한다. synth 보류분
                // (flush_pending_insert)과 네이티브 full-조합 보류분(flush_restart_phase_b)을
                // 모두 동기 처리한다(펌프 불가라 commit 정착은 보장 못 함 — degrade 한정).
                crate::register::dbg_log(
                    "b1: rev_window 부재 → degrade 동기 flush (no delay)",
                );
                self.flush_pending_insert(&comp_sink);
                self.flush_restart_phase_b(&comp_sink);
                // R6b: 머리 echo 미복귀(PENDING>0) → FIFO degrade(append). 라이브 게이트가
                // 못 뜨므로 꼬리도 동기 확정한다(유실 0).
                self.flush_pending_tail(&comp_sink);
            }
        }

        Ok(BOOL::from(outcome.eaten))
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

        // ── R6b 라이브 꼬리 terminate 가드 (M1 유사) ──
        // CUAS 가 synth 순방향 라이브 꼬리("다") 조합을 강제 종료하면 그 조합문을
        // result-string 으로 이미 확정(문서에 "다" 존재)했으므로 재삽입=중복이다. 엔진
        // preedit 만 비워 중복을 차단하고, 보류 꼬리·플래그를 폐기한 뒤 폴백 플립/CUAS
        // 학습을 건너뛰고 즉시 반환한다(머리는 synth 로 정상 확정됐으므로 컨텍스트 전체를
        // 오버레이로 떨구지 않는다 — 회귀 방지).
        if crate::synth_input::tail_live() {
            self.composition_mgr.lock().unwrap().clear();
            self.engine.lock().unwrap().remove_preedit();
            let _ = crate::synth_input::discard_pending_tail(); // 슬롯 폐기 + tail_live 해제
            crate::register::dbg_log(
                "OnCompositionTerminated: live-tail terminate → preedit 제거, 폴백/학습 skip",
            );
            return Ok(());
        }

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
        //
        // b1 회귀 차단: Phase2 가 시작한 신규 조합("기")의 즉시 terminate 는 학습에서
        // 제외한다. 안 하면 Blink 창이 cuas_windows 에 오염돼 이후 정상 한글 입력까지
        // 영구 오버레이 폴백으로 떨어진다(adversarial 검증 requiredRevision 2).
        let suppress_learn = self.suppress_cuas_learn.load(Ordering::SeqCst);
        if by_time && focus_hwnd != 0 && !suppress_learn {
            self.cuas_windows.lock().unwrap().insert(focus_hwnd);
        }

        self.composition_mgr.lock().unwrap().clear();

        // b1[D] M1 가드: full 조합(Phase2a/native)이 Phase2b(commit_and_restart) 전에
        // terminate 되면 composition=None 이 되는데 PENDING_RESTART 가 남아있으면
        // commit_and_restart 의 else 분기(insert_text+start)가 **이미 문서에 있는** 조합
        // 텍스트 위에 다시 삽입해 이중삽입("우간다우간다")이 된다. terminate 시 보류
        // 재시작도 함께 폐기해 Phase2b 를 no-op 화한다(terminate 된 텍스트는 문서에 보존).
        let _ = crate::synth_input::discard_pending_restart();

        // immediate(<200ms) 분기만 폴백 진입 처리. 엔진 preedit 버퍼는 보존해
        // 다음 키에서 누적 텍스트로 자연 재진입(start_composition)한다.
        // 정상 종료(포커스 이탈) 정리(engine.reset()/popup hide/atf reset)는
        // ITfThreadMgrEventSink::OnSetFocus(아래)로 이전했다 — CUAS의 정당한
        // 단발 종료를 "사용자 조합 취소"로 오인해 preedit 를 자폭시키지 않기 위함.
        //
        // b1: Phase2 신규 조합("기")의 즉시 terminate 면 폴백 플립도 억제한다. 안 하면
        // composition_unsupported=true 로 컨텍스트 전체가 오버레이로 떨어진다(회귀).
        // 글자 손실은 insert_pending 의 확정 폴백이 막으므로 폴백 플립이 불필요하다.
        if immediate && !suppress_learn {
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

// ── 단어별 preedit Phase 3a: Word 전용 게이트 헬퍼 ──

/// 포그라운드 창의 프로세스 실행파일 basename(소문자)을 반환한다.
///
/// `GetForegroundWindow → GetWindowThreadProcessId → OpenProcess(QUERY_LIMITED)
/// → QueryFullProcessImageNameW` 경로. 어느 단계든 실패하면 `None` 을 반환하고
/// 호출부는 syllable 모드로 안전 폴백한다(무회귀). 핸들은 read-only 조회 직후
/// CloseHandle 로 즉시 반납한다.
fn foreground_process_basename() -> Option<String> {
    use windows::Win32::UI::WindowsAndMessaging::GetForegroundWindow;
    let hwnd = unsafe { GetForegroundWindow() };
    process_basename_for_hwnd(hwnd)
}

/// 주어진 창(HWND)의 프로세스 실행파일 basename(소문자)을 반환한다.
///
/// `foreground_process_basename` 의 본체. 매-키 Word 게이트가 미리 얻어둔 포그라운드
/// HWND 를 그대로 넘겨 `GetForegroundWindow` 재호출을 피한다(메모이즈 일관성). HWND 가
/// 0 이거나 어느 단계든 실패하면 `None` 을 반환해 호출부가 syllable 로 안전 폴백한다
/// (무회귀). 핸들은 read-only 조회 직후 CloseHandle 로 즉시 반납한다.
fn process_basename_for_hwnd(hwnd: HWND) -> Option<String> {
    use windows::Win32::System::Threading::{
        OpenProcess, QueryFullProcessImageNameW, PROCESS_NAME_WIN32,
        PROCESS_QUERY_LIMITED_INFORMATION,
    };
    use windows::Win32::UI::WindowsAndMessaging::GetWindowThreadProcessId;

    unsafe {
        if hwnd.0 as isize == 0 {
            return None;
        }
        let mut pid: u32 = 0;
        GetWindowThreadProcessId(hwnd, Some(&mut pid));
        if pid == 0 {
            return None;
        }
        let handle = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid).ok()?;
        let mut buf = [0u16; 512];
        let mut len = buf.len() as u32;
        let res =
            QueryFullProcessImageNameW(handle, PROCESS_NAME_WIN32, PWSTR(buf.as_mut_ptr()), &mut len);
        // read-only 조회 끝 — 핸들 즉시 반납(이후 분기와 무관하게).
        let _ = CloseHandle(handle);
        res.ok()?;
        let path = String::from_utf16_lossy(&buf[..len as usize]);
        let base = path
            .rsplit(|c| c == '\\' || c == '/')
            .next()
            .unwrap_or(path.as_str())
            .to_ascii_lowercase();
        if base.is_empty() {
            None
        } else {
            Some(base)
        }
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
        pdimfocus: Ref<'_, ITfDocumentMgr>,
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
        // b1: 포커스 전환 → 보류 삽입 폐기(이전 문서에 stale "서기" 삽입 방지) + 타이머 끄기.
        if let Some(w) = self.rev_window.lock().unwrap().as_ref() {
            w.kill_flush_timer();
        }
        if crate::synth_input::discard_pending_insert() {
            crate::register::dbg_log("b1: OnSetFocus → 보류 삽입 폐기(포커스 전환)");
        }
        // R6b: 허용된 유일 손실(머리↔게이트 사이 포커스 전환) → 보류 꼬리 폐기.
        if crate::synth_input::discard_pending_tail() {
            crate::register::dbg_log("synth tail: OnSetFocus → 보류 꼬리 폐기(포커스 전환)");
        }
        // 방어적(라이브 중 포커스 전환; terminate 가드 미발화 대비 — discard 가 이미
        // false 로 만들었으면 idempotent).
        crate::synth_input::clear_tail_live();
        // (Phase 1) OnCompositionTerminated 의 "정상 종료" 정리를 이리로 이전.
        //   포커스 이탈 = 사용자가 조합을 떠남 → 엔진 preedit 를 비우고 팝업/ATF 정리.
        //
        // (단어별 preedit Phase 5) CommitUnit 존중 게이트.
        //   desired 는 config commit_unit 이 결정한다(reapply_word_gate 와 동일 정책):
        //   Syllable→off / Word→on(단 known_cuas 비협조앱은 off 강제) / Smart→winword 만.
        //   프로세스명 취득 실패 시 winword=false(안전 폴백). 매 포커스 재호출되므로
        //   config hot-reload(maybe_reload_config)가 엔진을 재생성해도 다음 포커스에서
        //   복구된다. known_cuas 는 위에서 이미 산출됨(focus_hwnd 기준).
        //   주의: 이 게이트는 set_word_mode 만 호출 — ATF/펌프-분할/synth 미접촉.
        //   Win32 호출은 엔진 락 밖에서 먼저 수행(락 보유 중 시스템콜 금지).
        let fg_name = foreground_process_basename();
        let is_winword = fg_name
            .as_deref()
            .map(|name| name == "winword.exe")
            .unwrap_or(false);
        // 비밀번호 필드 임시 영문(버그2): 포커스 문서의 InputScope 를 조회해
        // IS_PASSWORD(또는 PIN 계열)면 content_purpose=Password, 아니면 Normal 로 둔다.
        // 코어 set_content_purpose 상태머신이 진입 시 직전 한/영 저장+영문 강제,
        // 벗어남 시 직전 상태 복구를 수행한다. COM 조회(edit session)는 시스템콜이므로
        // 엔진 락을 잡기 전에 먼저 수행한다(락 보유 중 시스템콜 금지). 실패 시 normal.
        let is_password =
            crate::input_scope::focus_is_password(pdimfocus.as_ref(), self.client_id());
        let content_purpose = if is_password {
            unim::config::ContentPurpose::Password
        } else {
            unim::config::ContentPurpose::Normal
        };
        {
            let mut engine = self.engine.lock().unwrap();
            // commit_unit 설정 존중: Syllable→off / Word→on(known_cuas 비협조앱은 off) / Smart→winword.
            let commit_unit = engine.commit_unit();
            let word_mode = match commit_unit {
                CommitUnit::Syllable => false,
                CommitUnit::Word => !known_cuas,
                CommitUnit::Smart => is_winword,
            };
            // 진단: ThreadMgr OnSetFocus 게이트 발화 여부 + 판정 확인용(매-키 게이트와 동일 포맷).
            crate::register::dbg_log(&format!(
                "word-gate: ThreadMgr::OnSetFocus proc='{}' focus_hwnd=0x{:x} commit_unit={:?} winword={} known_cuas={} → word_mode={} (was={}) is_password={}",
                fg_name.as_deref().unwrap_or("<취득실패>"),
                focus_hwnd,
                commit_unit,
                is_winword,
                known_cuas,
                word_mode,
                engine.is_word_mode(),
                is_password
            ));
            engine.reset();
            engine.set_word_mode(word_mode);
            // 비밀번호 진입→임시 영문 / 벗어남→직전 한/영 복구 (코어 상태머신, 멱등).
            engine.set_content_purpose(content_purpose);
        }
        // Phase 4 — 포커스 전환 시 보유 영문 누적 폐기(이전 문서 보유분이 새 컨텍스트로 새지
        // 않게). 조합 객체는 보통 OnCompositionTerminated 가 이미 정리하므로 누적만 비운다.
        self.composition_mgr.lock().unwrap().clear_english_hold();
        // 포커스 전환 → 팝업 Hide 송신 (렌더러 owner 규칙으로 stale hide 무시 처리).
        self.popup_ipc.lock().unwrap().hide();
        // 포커스 이동 → 보관 컨텍스트 무효화 (역채널 마우스 확정이 옛 창에 안 가도록).
        *self.last_context.lock().unwrap() = None;
        if let Some(ref mut win) = *self.preedit_window.lock().unwrap() {
            win.hide();
        }
        // Excel 셀 첫타 ATF 유실 대응(가설·확신낮음): 직전 키 직후 같은 프로세스로 발생한
        // 스퓨리어스 포커스(3중 게이트)면 ATF 버퍼 reset 을 스킵(첫 타 보존). 위에서 이미
        // 구한 fg_name 을 재사용한다. 게이트 밖(정상 앱 전환·포커스 이탈)은 기존대로
        // reset(무회귀). 주의: engine.reset()/word·content_purpose/comp clear 등 다른 정리는
        // 위에서 이미 수행됐고, 순방향(영문)은 보존할 엔진 조합 상태가 없어 정합.
        if !self.atf_reset_should_skip(fg_name.as_deref()) {
            self.atf_state.lock().unwrap().reset_on_focus();
        }
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
        pic: Ref<'_, ITfContext>,
        ecreadonly: u32,
        _peditrecord: Ref<'_, ITfEditRecord>,
    ) -> Result<()> {
        // D3 — BS×N 삭제가 Blink 문서에 실제 적용됐는지 read-back 으로 검증해, 적용
        // 확인 즉시 Phase2 삽입을 트리거한다(40~60ms 고정 대기 대신 이벤트 기반).
        //
        // panic=abort 보호: COM 콜백에서 panic 이 STA/COM 경계를 넘지 않도록
        // catch_unwind 로 감싼다(rev_wnd_proc 패턴 모방). 어떤 실패도 graceful degrade
        // (게이트 안 열림 → 60ms 안전망 타이머가 read-back 없이 폴백 = 현행 b1).
        let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            // 이중 가드(설계 §회귀): 무관한 편집·BS 미주입 상태는 atomic load 2회로 즉시 무시.
            if !crate::synth_input::has_pending_insert() {
                return;
            }
            if !crate::synth_input::readback_gate_armed() {
                return;
            }
            let Some(context) = pic.as_ref() else {
                return;
            };
            // OnEndEdit 의 ec 는 read-only — read-back 만 수행(삽입 RW 금지). 보유한 ec
            // 를 그대로 써 중첩 edit session 을 열지 않는다.
            let Some(cur_len) =
                crate::composition::read_text_before_cursor_len(context, ecreadonly)
            else {
                // read-back 실패 → 게이트 안 열림, 타이머 안전망에 위임(악화 없음).
                return;
            };
            // 설계 약점 1 방어: 신호만 믿지 않고 cur_len<=expected 를 반드시 검증.
            // 부분 삭제(BS 적용 전 조기 OnEndEdit)면 통과 못 하고 다음 OnEndEdit/타이머 대기.
            if !crate::synth_input::readback_delete_applied(cur_len) {
                return;
            }
            // 적용 확인.
            if let Some(w) = self.rev_window.lock().unwrap().as_ref() {
                // 게이트 해제 + 안전망 타이머 kill + flush 를 wndproc 에 위임(async post).
                crate::synth_input::disarm_readback_gate();
                w.kill_flush_timer();
                w.post_flush();
                crate::register::dbg_log(&format!(
                    "D3: OnEndEdit read-back OK (cur_len={}) → WM_UNIM_FLUSH",
                    cur_len
                ));
            } else {
                // rev_window 부재(비활성/창생성 실패). OnEndEdit 의 ec 는 read-only 이고 이
                // 콜백은 문서 lock 보유 중이라, 여기서 flush_pending_insert(동기 RW
                // RequestEditSession)를 호출하면 TF_E_NOLOCK 으로 거부될 수 있고 그 경우
                // 이미 take 된 보류삽입이 유실된다(shouldFix #1). 따라서 OnEndEdit 안에서는
                // 절대 새 세션을 열지 않는다 — 게이트/슬롯을 그대로 보존해, schedule 시점의
                // degrade 동기 flush(rev_window 부재 경로) 또는 다음 키 race-flush 에 위임한다
                // (race-flush 는 게이트가 아니라 has_pending_insert 로 동작하므로 자가복구됨).
                crate::register::dbg_log(
                    "D3: OnEndEdit read-back OK but no rev_window → race-flush 위임(세션 미개설)",
                );
            }
        }));
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
    CreateWindowExW, DefWindowProcW, DestroyWindow, GetWindowLongPtrW, KillTimer, PostMessageW,
    RegisterClassExW, SetTimer, SetWindowLongPtrW, GWLP_USERDATA, HWND_MESSAGE, WINDOW_EX_STYLE,
    WINDOW_STYLE, WM_TIMER, WNDCLASSEXW,
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
    /// b1 Phase2 — 신규 조합 즉시-terminate 학습 억제 플래그(text_service 와 공유).
    suppress_cuas_learn: Arc<AtomicBool>,
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

    /// b1 Phase2 — one-shot SetTimer 를 건다(이 HWND 의 메시지 펌프 = TSF STA 스레드).
    /// 같은 timer_id 로 재호출하면 기존 타이머를 재설정한다(중복 발화 없음).
    fn arm_flush_timer(&self, timer_id: usize, delay_ms: u32) {
        unsafe {
            // SetTimer 는 0 반환 시 실패. lpTimerFunc=None → WM_TIMER 가 wndproc 로 옴.
            let r = SetTimer(Some(self.hwnd), timer_id, delay_ms, None);
            if r == 0 {
                crate::register::dbg_log("b1: SetTimer FAILED (r=0)");
            }
        }
    }

    /// b1 Phase2 — 보류 중인 flush 타이머를 끈다(race-flush 선점·Drop·포커스 전환).
    fn kill_flush_timer(&self) {
        unsafe {
            let _ = KillTimer(Some(self.hwnd), FLUSH_TIMER_ID);
        }
    }

    /// D3 — OnEndEdit read-back 검증 통과 시 wndproc 에 즉시 flush 를 위임한다.
    /// OnEndEdit ec(read-only)에서 RW 삽입을 못 하므로 PostMessage 로 STA 펌프에 던진다.
    fn post_flush(&self) {
        unsafe {
            let _ = PostMessageW(Some(self.hwnd), WM_UNIM_FLUSH, WPARAM(0), LPARAM(0));
        }
    }

    /// R6b — PENDING==0 게이트 통과 시 wndproc 에 라이브 꼬리 조합을 위임한다
    /// (OnTestKeyDown 은 RW edit session 불가).
    fn post_tail(&self) {
        unsafe {
            let _ = PostMessageW(Some(self.hwnd), WM_UNIM_TAIL, WPARAM(0), LPARAM(0));
        }
    }
}

impl Drop for RevWindow {
    fn drop(&mut self) {
        unsafe {
            // b1: 보류 중인 flush 타이머를 먼저 끈다(파괴 후 WM_TIMER 도착 방지).
            let _ = KillTimer(Some(self.hwnd), FLUSH_TIMER_ID);
            // notify 차단 먼저 (이후 도착 push 는 PostMessage 안 함).
            self.rev.set_notify_hwnd(0);
            // userdata 초기화 후 창 파괴 (wndproc 재진입 방지).
            SetWindowLongPtrW(self.hwnd, GWLP_USERDATA, 0);
            let _ = DestroyWindow(self.hwnd);
            crate::register::dbg_log("popup_rev: message-only HWND destroyed");
        }
        // _ctx Box 는 자동 Drop. b1 보류 삽입도 폐기(stale 방지).
        let _ = crate::synth_input::discard_pending_insert();
        // R6b 보류 꼬리도 폐기(tail_live 도 false).
        let _ = crate::synth_input::discard_pending_tail();
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
    if msg == WM_UNIM_FLUSH {
        // D3 — OnEndEdit read-back 검증 통과 → 즉시 Phase2 삽입(주 경로). WM_TIMER 분기와
        // 동일 처리(catch_unwind + KillTimer + timer_flush_pending_insert). take 1회성이라
        // 타이머/race-flush 와 다중 발화해도 1회만 삽입(중복 무해).
        let _ = std::panic::catch_unwind(|| unsafe {
            let _ = KillTimer(Some(hwnd), FLUSH_TIMER_ID);
            let ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *const RevWndContext;
            if ptr.is_null() {
                let _ = crate::synth_input::discard_pending_insert();
                return;
            }
            let ctx = &*ptr;
            crate::register::dbg_log("D3: WM_UNIM_FLUSH → Phase2 insert (read-back path)");
            timer_flush_pending_insert(ctx);
            // b1[D]: Phase2a 가 세션A 조합을 만들고 PENDING_RESTART 를 적재했으면, 펌프를
            // 한 번 돌린 뒤 세션B(commit_and_restart)가 돌도록 WM_UNIM_FLUSH2 를 던진다.
            if crate::synth_input::has_pending_restart() {
                let _ = PostMessageW(Some(hwnd), WM_UNIM_FLUSH2, WPARAM(0), LPARAM(0));
            }
        });
        return LRESULT(0);
    }
    if msg == WM_UNIM_FLUSH2 {
        // b1[D] Phase2b — 펌프 경과 후 세션B(commit_and_restart) 실행(별도 RW edit session).
        let _ = std::panic::catch_unwind(|| unsafe {
            let ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *const RevWndContext;
            if ptr.is_null() {
                let _ = crate::synth_input::discard_pending_restart();
                return;
            }
            let ctx = &*ptr;
            crate::register::dbg_log("b1[D]: WM_UNIM_FLUSH2 → Phase2b (commit_and_restart)");
            timer_flush_restart_phase_b(ctx);
        });
        return LRESULT(0);
    }
    if msg == WM_UNIM_TAIL {
        // R6b — PENDING==0 게이트 통과 → 라이브 꼬리 조합(또는 PENDING>0 시 FIFO degrade).
        let _ = std::panic::catch_unwind(|| unsafe {
            let ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *const RevWndContext;
            if ptr.is_null() {
                let _ = crate::synth_input::discard_pending_tail();
                return;
            }
            let ctx = &*ptr;
            crate::register::dbg_log("synth: WM_UNIM_TAIL → 라이브 꼬리 조합");
            tail_flush_pending(ctx);
        });
        return LRESULT(0);
    }
    if msg == WM_TIMER && wparam.0 == FLUSH_TIMER_ID {
        // b1 Phase2 — BS×N 이 Blink 문서에 적용될 시간(FLUSH_DELAY_MS)이 지났다.
        // 보류 삽입을 TSF 로 삽입한다. panic=abort 환경 보호: catch_unwind 로 감싼다.
        let _ = std::panic::catch_unwind(|| unsafe {
            // 1회성 보장: 즉시 KillTimer (재진입·중복 WM_TIMER 무해).
            let _ = KillTimer(Some(hwnd), FLUSH_TIMER_ID);
            let ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *const RevWndContext;
            if ptr.is_null() {
                // 창 파괴 중(userdata=0) — 보류 삽입 폐기(stale 방지).
                let _ = crate::synth_input::discard_pending_insert();
                return;
            }
            let ctx = &*ptr;
            crate::register::dbg_log("b1: WM_TIMER fired → Phase2 insert");
            timer_flush_pending_insert(ctx);
            // b1[D]: Phase2a 가 PENDING_RESTART 를 적재했으면 펌프 후 세션B 실행.
            if crate::synth_input::has_pending_restart() {
                let _ = PostMessageW(Some(hwnd), WM_UNIM_FLUSH2, WPARAM(0), LPARAM(0));
            }
            // R6b: synth 라이브 꼬리 안전망 — 게이트(WM_UNIM_TAIL)가 60ms 내 못 떴거나
            // echo 미복귀일 때. 정상 케이스는 게이트가 선행 take 해 has_pending_tail=false
            // → no-op. 내부 PENDING 판정: ==0 라이브, >0 FIFO degrade.
            if crate::synth_input::has_pending_tail() {
                tail_flush_pending(ctx);
            }
        });
        return LRESULT(0);
    }
    unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) }
}

/// b1 Phase2 — WM_TIMER 발화 시 보류 삽입을 TSF 로 삽입 (TSF STA 스레드, 락 짧게).
///
/// SendInput 미사용이라 edit session 안전. 어떤 실패도 패닉 금지. PendingInsert 가
/// 이미 race-flush 로 소비됐으면 take 가 None → no-op.
fn timer_flush_pending_insert(ctx: &RevWndContext) {
    if !crate::synth_input::has_pending_insert() {
        return;
    }
    // 컨텍스트(문서) 부재면 stale 삽입 방지: 폐기.
    let context = {
        let g = ctx.last_context.lock().unwrap();
        match g.as_ref() {
            Some(c) => c.clone(),
            None => {
                let _ = crate::synth_input::discard_pending_insert();
                crate::register::dbg_log("b1: WM_TIMER no context → pending 폐기");
                return;
            }
        }
    };
    // 락 순서: engine → composition (다른 경로와 동일 접두).
    let mut engine = ctx.engine.lock().unwrap();
    let mut comp_mgr = ctx.composition_mgr.lock().unwrap();
    // Phase2 신규 조합의 단발 terminate 를 학습에서 제외.
    ctx.suppress_cuas_learn.store(true, Ordering::SeqCst);
    key_handler::flush_pending_insert(
        &mut engine,
        &mut comp_mgr,
        &context,
        ctx.tid,
        &ctx.comp_sink,
    );
    ctx.suppress_cuas_learn.store(false, Ordering::SeqCst);
}

/// b1[D] Phase2b — WM_UNIM_FLUSH2 발화 시 보류 재시작을 commit_and_restart 로 확정한다
/// (TSF STA 스레드, 락 짧게). 펌프 경과 후라 세션A 조합이 문서에 등록돼 있어 세션B
/// commit 이 정착한다. SendInput 미사용 → edit session 안전. 패닉 금지. 보류 재시작이
/// 이미 소비/폐기됐으면 take 가 None → no-op.
fn timer_flush_restart_phase_b(ctx: &RevWndContext) {
    if !crate::synth_input::has_pending_restart() {
        return;
    }
    let context = {
        let g = ctx.last_context.lock().unwrap();
        match g.as_ref() {
            Some(c) => c.clone(),
            None => {
                let _ = crate::synth_input::discard_pending_restart();
                crate::register::dbg_log("b1[D]: Phase2b no context → restart 폐기");
                return;
            }
        }
    };
    // 락 순서: engine → composition (다른 경로와 동일 접두).
    let mut engine = ctx.engine.lock().unwrap();
    let mut comp_mgr = ctx.composition_mgr.lock().unwrap();
    // 세션B 신규 tail 조합의 단발 terminate 를 학습에서 제외.
    ctx.suppress_cuas_learn.store(true, Ordering::SeqCst);
    key_handler::flush_restart_phase_b(
        &mut engine,
        &mut comp_mgr,
        &context,
        ctx.tid,
        &ctx.comp_sink,
    );
    ctx.suppress_cuas_learn.store(false, Ordering::SeqCst);
}

/// R6b — WM_UNIM_TAIL/WM_TIMER 발화 시 보류 꼬리를 라이브/degrade 처리한다(TSF STA, 락
/// 짧게). 라이브 조합의 단발 terminate 가 cuas_windows 를 오염시키지 않도록
/// suppress_cuas_learn 으로 감싼다. SendInput(degrade)은 edit session 밖. 패닉 금지.
fn tail_flush_pending(ctx: &RevWndContext) {
    if !crate::synth_input::has_pending_tail() {
        return;
    }
    let context = {
        let g = ctx.last_context.lock().unwrap();
        match g.as_ref() {
            Some(c) => c.clone(),
            None => {
                let _ = crate::synth_input::discard_pending_tail();
                crate::register::dbg_log("synth tail: wndproc no context → 폐기");
                return;
            }
        }
    };
    // 락 순서: engine → composition (다른 경로와 동일 접두).
    let mut engine = ctx.engine.lock().unwrap();
    let mut comp_mgr = ctx.composition_mgr.lock().unwrap();
    ctx.suppress_cuas_learn.store(true, Ordering::SeqCst);
    key_handler::flush_pending_tail(
        &mut engine,
        &mut comp_mgr,
        &context,
        ctx.tid,
        &ctx.comp_sink,
    );
    ctx.suppress_cuas_learn.store(false, Ordering::SeqCst);
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
