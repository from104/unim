//! XIM 핸들러 모듈
//!
//! XIM 프로토콜 이벤트를 처리하고 DBus를 통해 UNIM 데몬과 연동합니다.

use std::ffi::CString;
use std::num::NonZeroU32;
use std::os::raw::c_int;

use ahash::AHashMap;
use tokio::sync::mpsc;
use x11rb::connection::Connection;
use x11rb::protocol::xproto::{ConfigureNotifyEvent, KeyPressEvent};

use unim::config::Config;
use unim::unim_log;

use xim::{
    x11rb::{HasConnection, X11rbServer},
    InputContext, InputStyle, Server, ServerError, ServerHandler, UserInputContext,
};

use crate::dbus_client::{DbusRequest, DbusResponse, PopupEvent};
use crate::pe_window::PeWindow;

/// KeyCode 이름에서 X11 keysym으로 변환
fn keycode_name_to_keysym(name: &str) -> Option<u32> {
    match name {
        "Hanja" => Some(0xff34),  // XK_Hangul_Hanja
        "Korean" => Some(0xff31), // XK_Hangul
        "F1" => Some(0xffbe),
        "F2" => Some(0xffbf),
        "F3" => Some(0xffc0),
        "F4" => Some(0xffc1),
        "F5" => Some(0xffc2),
        "F6" => Some(0xffc3),
        "F7" => Some(0xffc4),
        "F8" => Some(0xffc5),
        "F9" => Some(0xffc6),
        "F10" => Some(0xffc7),
        "F11" => Some(0xffc8),
        "F12" => Some(0xffc9),
        "RightAlt" => Some(0xffea),     // XK_Alt_R
        "LeftAlt" => Some(0xffe9),      // XK_Alt_L
        "RightControl" => Some(0xffe4), // XK_Control_R
        "LeftControl" => Some(0xffe3),  // XK_Control_L
        "RightShift" => Some(0xffe2),   // XK_Shift_R
        "LeftShift" => Some(0xffe1),    // XK_Shift_L
        "Space" => Some(0x0020),
        "Escape" => Some(0xff1b),
        "CapsLock" => Some(0xffe5),
        _ => None,
    }
}

/// 입력 컨텍스트별 상태
pub struct UnimInputContext {
    /// DBus 컨텍스트 경로
    context_path: String,
    /// preedit 윈도우 ID (Over-The-Spot 스타일일 때만 사용)
    pe_window: Option<NonZeroU32>,
    /// preedit 윈도우를 표시할지 여부
    show_preedit_window: bool,
    /// 현재 preedit 문자열 (캐시)
    preedit_cache: String,
}

impl UnimInputContext {
    fn new(context_path: String, input_style: InputStyle) -> Self {
        let show_preedit_window = !input_style.contains(InputStyle::PREEDIT_CALLBACKS)
            && !input_style.contains(InputStyle::PREEDIT_NOTHING);

        Self {
            context_path,
            pe_window: None,
            show_preedit_window,
            preedit_cache: String::new(),
        }
    }
}

/// X11 이벤트 마스크 (KeyPress)
const EVENT_MASK: u32 = 1;

/// UNIM XIM 핸들러
pub struct UnimHandler {
    #[allow(dead_code)]
    config: Config,
    /// preedit 윈도우들 (윈도우 ID -> PeWindow)
    preedit_windows: AHashMap<NonZeroU32, PeWindow>,
    /// Xlib Display (단일 연결, 핸들러가 소유)
    display: *mut x11::xlib::Display,
    /// 스크린 번호 (c_int)
    screen: c_int,
    /// DBus 클라이언트 (요청 전송 채널)
    dbus_tx: mpsc::Sender<DbusRequest>,
    /// 한자/특수문자 키 keysym 목록 (설정 기반)
    hanja_keysyms: Vec<u32>,
    /// 마지막 포커스된 IC 정보 (AutoTypeFix commit용)
    /// (client_win, input_method_id, input_context_id)
    last_focused_ic_info: Option<(u32, std::num::NonZeroU16, std::num::NonZeroU16)>,
    /// 마지막 포커스된 앱 윈도우 (AutoTypeFix BackSpace 전송용)
    last_focused_app_window: Option<u64>,
    /// 마지막 포커스된 IC의 DBus 컨텍스트 경로 (AutoTypeFix Reset용)
    last_focused_context_path: Option<String>,
    /// AutoTypeFix 자가 주입 BackSpace 카운터
    /// XTestFakeKeyEvent로 주입한 BackSpace가 XIM 서버로 재진입할 때
    /// 엔진 처리 없이 그대로 앱에 통과시키기 위한 카운터.
    /// 주입 직전에 delete_chars 만큼 증가, handle_forward_event에서 BackSpace
    /// 감지 시 감소시키고 Ok(false)로 반환하여 클라이언트에 전달.
    self_backspace_pending: u32,
    /// AutoTypeFix 지연 교정 (commit_text, preedit_text)
    /// N+1번째(마지막) BS의 handle_forward_event에서 pending==0 시
    /// 진짜 user_ic.ic로 commit+preedit 실행하고 Ok(true)로 소비.
    deferred_autofix: Option<(String, String)>,
    /// AutoTypeFix 시그널 수신 시점의 DBus 컨텍스트 경로
    autofix_context_path: Option<String>,
    /// AutoTypeFix commit/preedit 처리 중 재진입 방지 플래그.
    /// XIM crate가 server.commit()/preedit_draw() 시 keycode=0 가상 이벤트를
    /// handle_forward_event로 재진입시키므로, 이를 무시하기 위한 가드.
    autofix_commit_guard: bool,
    /// dedupe: `handle_reset_ic` 가 `ResetIcReply` 로 커밋 문자열을 동기 반환한
    /// 직후 데몬이 같은 값을 `CommitText` 시그널로 또 발행하므로, 그 값을
    /// 기억해 두었다가 1회만 skip 한다.
    ///
    /// 배경: `handle_reset_ic` 는 XIM 계약대로 preedit 을 동기 반환하고,
    /// 클라이언트는 그 문자열을 **조합이 시작된 자리**에 커밋한다. 그런데 같은
    /// 경로에서 보낸 `DbusRequest::Reset` 때문에 데몬이
    /// (`unim-dbus/src/service.rs` 의 `reset()`) 비운 조합을 `CommitText`
    /// 시그널로도 발행하고, 이게 팝업 커밋용 구독을 타고 들어와
    /// `server.commit()` 으로 한 번 더 들어간다. 시그널은 비동기라 앱이 이미
    /// 캐럿을 옮긴 뒤에 도착하므로 두 번째 글자가 클릭한 자리에 박힌다.
    ///
    /// GTK 모듈의 `pending_skip_commit`
    /// (`unim-frontends/gtk-common/src/unim_dbus_client.c`) 과 같은 방식이다.
    /// 만료 시각을 두지 않는 것도 의도적이다 — XIM 은 `dbus_tx` 채널 →
    /// `proxy.reset().await` → 데몬 → 시그널로 돌아오느라 왕복이 길어서
    /// (실측 로그에서 1초 가까이) 시한을 두면 늦은 메아리를 놓친다.
    pending_skip_commit: Option<String>,
}

impl UnimHandler {
    pub fn new(
        screen_num: usize,
        config: Config,
        dbus_tx: mpsc::Sender<DbusRequest>,
    ) -> Result<Self, String> {
        // 단일 Xlib Display 연결 열기
        let display = unsafe {
            let display_name = std::env::var("DISPLAY").unwrap_or_else(|_| ":0".to_string());
            let display_cstr = CString::new(display_name).unwrap();
            x11::xlib::XOpenDisplay(display_cstr.as_ptr())
        };

        if display.is_null() {
            return Err("XOpenDisplay failed: X11 서버에 연결할 수 없습니다".to_string());
        }

        let hanja_keysyms = config
            .engine
            .hanja_keys
            .iter()
            .filter_map(|name| keycode_name_to_keysym(name))
            .collect();

        Ok(Self {
            config,
            preedit_windows: AHashMap::new(),
            display,
            screen: screen_num as c_int,
            dbus_tx,
            hanja_keysyms,
            last_focused_ic_info: None,
            last_focused_app_window: None,
            last_focused_context_path: None,
            self_backspace_pending: 0,
            deferred_autofix: None,
            autofix_context_path: None,
            autofix_commit_guard: false,
            pending_skip_commit: None,
        })
    }

    /// `CommitText` 시그널이 방금 동기 반환한 커밋의 메아리인지 판정하고 소비한다.
    fn take_pending_skip_commit(&mut self, text: &str) -> bool {
        if self.pending_skip_commit.as_deref() == Some(text) {
            self.pending_skip_commit = None; // 1회용
            return true;
        }
        false
    }

    /// DBus 요청 전송 (동기적 - 블로킹)
    fn send_dbus_request(&self, request: DbusRequest) -> Option<DbusResponse> {
        let (response_tx, response_rx) = std::sync::mpsc::channel();

        // 요청과 함께 응답 채널 전송
        let request_with_response = match request {
            DbusRequest::ProcessKey {
                context_path,
                keyval,
                keycode,
                state,
                ..
            } => DbusRequest::ProcessKey {
                context_path,
                keyval,
                keycode,
                state,
                response: Some(response_tx),
            },
            DbusRequest::CreateContext {
                client_name,
                window_id,
                ..
            } => DbusRequest::CreateContext {
                client_name,
                window_id,
                response: Some(response_tx),
            },
            DbusRequest::FocusOut { context_path, .. } => DbusRequest::FocusOut {
                context_path,
                response: Some(response_tx),
            },
            DbusRequest::GetHanjaCandidates { context_path, .. } => {
                DbusRequest::GetHanjaCandidates {
                    context_path,
                    response: Some(response_tx),
                }
            }
            DbusRequest::SelectHanja {
                context_path,
                index,
                ..
            } => DbusRequest::SelectHanja {
                context_path,
                index,
                response: Some(response_tx),
            },
            DbusRequest::GetSpecialCharCandidates { context_path, .. } => {
                DbusRequest::GetSpecialCharCandidates {
                    context_path,
                    response: Some(response_tx),
                }
            }
            DbusRequest::SelectSpecialChar {
                context_path,
                index,
                ..
            } => DbusRequest::SelectSpecialChar {
                context_path,
                index,
                response: Some(response_tx),
            },
            DbusRequest::CancelHanja { context_path, .. } => DbusRequest::CancelHanja {
                context_path,
                response: Some(response_tx),
            },
            DbusRequest::CancelSpecialChar { context_path, .. } => DbusRequest::CancelSpecialChar {
                context_path,
                response: Some(response_tx),
            },
            other => other,
        };

        // tokio 채널에 전송 (blocking)
        if self.dbus_tx.blocking_send(request_with_response).is_err() {
            return None;
        }

        // 응답 대기 (타임아웃 500ms)
        response_rx
            .recv_timeout(std::time::Duration::from_millis(500))
            .ok()
    }

    /// Expose 이벤트 처리
    pub fn expose<C: Connection + xim::x11rb::HasConnection>(
        &mut self,
        window: u32,
        _conn: &C,
    ) -> Result<(), x11rb::errors::ConnectionError> {
        if let Some(w) = NonZeroU32::new(window) {
            if let Some(pe) = self.preedit_windows.get_mut(&w) {
                pe.expose(self.display);
            }
        }

        Ok(())
    }

    /// ConfigureNotify 이벤트 처리
    pub fn configure_notify(&mut self, event: &ConfigureNotifyEvent) {
        if let Some(win) = NonZeroU32::new(event.window) {
            if let Some(pe) = self.preedit_windows.get_mut(&win) {
                unim_log!(
                    "XIM_HANDLER",
                    "ConfigureNotify: window={}, width={}, height={}",
                    event.window,
                    event.width,
                    event.height
                );
                pe.configure_notify(*event, self.display);
            }
        }
    }

    /// Preedit 표시
    fn preedit<C: Connection + xim::x11rb::HasConnection>(
        &mut self,
        server: &mut X11rbServer<C>,
        user_ic: &mut UserInputContext<UnimInputContext>,
        preedit_str: &str,
    ) -> Result<(), ServerError> {
        // preedit 캐시 업데이트
        user_ic.user_data.preedit_cache = preedit_str.to_string();

        // ibus 호환: 입력 스타일과 무관하게 항상 preedit_draw 호출
        server.preedit_draw(&mut user_ic.ic, preedit_str)?;

        // PREEDIT_CALLBACKS가 아니면 Over-The-Spot 렌더링도 수행
        if !user_ic
            .ic
            .input_style()
            .contains(InputStyle::PREEDIT_CALLBACKS)
        {
            if !user_ic.user_data.show_preedit_window {
                return Ok(());
            }

            if preedit_str.is_empty() {
                // PeWindow 정리
                if let Some(pe_id) = user_ic.user_data.pe_window.take() {
                    if let Some(pe) = self.preedit_windows.remove(&pe_id) {
                        unim_log!("XIM_HANDLER", "PeWindow 삭제: id={}", pe_id);
                        pe.clean(self.display, self.screen);
                    }
                }
                return Ok(());
            }

            // PeWindow로 렌더링
            if let Some(pe_id) = user_ic.user_data.pe_window {
                if let Some(pe) = self.preedit_windows.get_mut(&pe_id) {
                    pe.set_preedit(preedit_str);
                    pe.refresh(self.display);
                }
            } else {
                // 새 PeWindow 생성
                let mut pe = PeWindow::new(
                    self.display,
                    self.screen,
                    user_ic.ic.app_win(),
                    user_ic.ic.preedit_spot(),
                )?;

                let pe_id = pe.window();
                user_ic.user_data.pe_window = Some(pe_id);

                pe.set_preedit(preedit_str);
                pe.refresh(self.display);

                self.preedit_windows.insert(pe_id, pe);
                unim_log!("XIM_HANDLER", "PeWindow 생성: id={}", pe_id);
            }
        }

        Ok(())
    }

    fn clear_preedit<C: Connection + xim::x11rb::HasConnection>(
        &mut self,
        server: &mut X11rbServer<C>,
        user_ic: &mut UserInputContext<UnimInputContext>,
    ) -> Result<(), ServerError> {
        user_ic.user_data.preedit_cache.clear();

        // ibus 호환: 입력 스타일과 무관하게 항상 preedit_draw("") 호출
        server.preedit_draw(&mut user_ic.ic, "")?;

        // PeWindow도 정리
        if let Some(pe_id) = user_ic.user_data.pe_window.take() {
            if let Some(pe) = self.preedit_windows.remove(&pe_id) {
                unim_log!("XIM_HANDLER", "PeWindow 삭제: id={}", pe_id);
                pe.clean(self.display, self.screen);
            }
        }

        Ok(())
    }

    /// commit + preedit 송출 SSOT 헬퍼.
    ///
    /// 동일 frame에 commit과 preedit_draw를 함께 발사하면 일부 XIM 클라이언트
    /// (Chrome, ibus 호환 GTK 등)가 commit 처리 도중 preedit을 초기화하면서
    /// 새 preedit을 놓치는 race가 발생한다. (예: 두벌식 ㄹㄹㄹ 5연타 시
    /// 두 번째 ㄹ 입력에서 daemon이 commit='ㄹ', preedit='ㄹ' 둘 다 반환 →
    /// 클라이언트가 commit 처리하며 preedit을 비워버림.)
    ///
    /// 회피책: commit 직전에 `clear_preedit()` 을 강제 호출해 현재 preedit
    /// 사이클을 종료(PreeditDraw(empty)+PreeditDone)시켜 xim crate 내부의
    /// `ic.preedit_started=false` 로 reset한 다음 commit → 새 preedit_draw.
    /// 그러면 새 preedit_draw 호출 시 xim crate 가 자동으로 PreeditStart 를
    /// 재발사(server.rs:205-214)해 PREEDIT_CALLBACKS(ON-THE-SPOT) 모드
    /// 클라이언트가 PreeditStart 없이 도착한 PreeditDraw 를 무시하던
    /// 누락 버그를 차단한다.
    ///
    /// 배경: xim-0.5.0/src/server.rs:236-248 의 `commit()` 은 단순히 Commit
    /// 메시지만 보내고 `preedit_started` 를 그대로 둔다. 그러나 일부 ON-THE-SPOT
    /// 클라이언트(unim-test-xim 등)는 commit 후 PreeditDone 을 자체 가정 →
    /// 다음 PreeditDraw 가 PreeditStart 없이 도착하면 무시. XTerm/WezTerm 은
    /// PreeditPosition(OVER-THE-SPOT) 모드라 이 사이클 영향이 없어 무관.
    ///
    /// AutoTypeFix N+1 BS 분기(handle_xevent 내 deferred_autofix 처리)는
    /// XTest 가짜 이벤트 주입 + per-key sleep 컨텍스트라 별개 동작 — 본
    /// 함수 변경의 영향 범위 밖.
    fn commit_then_preedit<C: Connection + xim::x11rb::HasConnection>(
        &mut self,
        server: &mut X11rbServer<C>,
        user_ic: &mut UserInputContext<UnimInputContext>,
        commit_text: &str,
        preedit_text: &str,
    ) -> Result<(), ServerError> {
        let has_commit = !commit_text.is_empty();
        let has_preedit = !preedit_text.is_empty();

        if has_commit {
            // [1단계] 현재 preedit 사이클 종료 — xim crate 가 PreeditDraw(empty)
            // + PreeditDone 을 발사하고 preedit_started=false 로 reset.
            // 이 호출이 noop 인 경우(이미 preedit 비어있음)도 무해.
            self.clear_preedit(server, user_ic)?;

            // [2단계] commit 전송
            server.commit(&user_ic.ic, commit_text)?;
            server.conn().flush().ok();
        }

        // [3단계] 새 preedit 전송 — preedit_started=false 인 상태에서
        // preedit_draw 가 호출되면 xim crate 가 PreeditStart 를 자동 재발사.
        if has_preedit {
            self.preedit(server, user_ic, preedit_text)?;
            server.conn().flush().ok();
        } else if !has_commit {
            // commit 도 없고 preedit 도 비어있는 케이스만 별도 clear.
            // commit 분기는 이미 [1단계] 에서 clear 처리됨.
            self.clear_preedit(server, user_ic)?;
            server.conn().flush().ok();
        }

        Ok(())
    }

    /// DBus 팝업 시그널 처리 (main.rs 이벤트 루프에서 호출)
    pub fn handle_popup_event<C: Connection + xim::x11rb::HasConnection>(
        &mut self,
        event: PopupEvent,
        server: &mut X11rbServer<C>,
    ) -> Result<(), ServerError> {
        match event {
            PopupEvent::Navigate { .. } => {
                // Standalone 모드: unim-gui-gtk가 Navigate 시그널을 직접 수신해서 처리
            }
            PopupEvent::Hide => {
                // Standalone 모드: unim-gui-gtk가 HidePopup 시그널을 직접 수신해서 처리
            }
            PopupEvent::ShowHanja { .. } | PopupEvent::ShowSpecial { .. } => {
                // Standalone 모드: unim-gui-gtk가 Show* 시그널을 직접 수신해서 처리
            }
            PopupEvent::ShowEmoji { .. } => {
                // Standalone 모드: unim-gui-gtk가 ShowEmojiPopupV2 시그널을 직접 수신해서 처리
            }
            PopupEvent::AutoTypeFix {
                delete_chars,
                commit_text,
                preedit_text,
            } => {
                // XIM: 백스페이스 N회 전송 후 교정 텍스트 커밋
                // 순방향(preedit 있음): commit + preedit 분리, 엔진 replay 유지
                // 역방향(preedit 없음): commit만, 엔진 Reset
                if let Some(_app_win) = self.last_focused_app_window {
                    unim_log!(
                        "XIM_HANDLER",
                        "AutoTypeFix: delete={}, commit='{}', preedit='{}'",
                        delete_chars,
                        commit_text,
                        preedit_text
                    );

                    // 이전 AutoTypeFix가 미완료 상태면 폐기
                    if self.deferred_autofix.is_some() {
                        unim_log!(
                            "XIM_HANDLER",
                            "AutoTypeFix: 이전 교정 미완료, 폐기 (pending_bs={})",
                            self.self_backspace_pending
                        );
                        self.deferred_autofix = None;
                        self.self_backspace_pending = 0;
                        self.autofix_context_path = None;
                    }

                    self.autofix_context_path = self.last_focused_context_path.clone();

                    // 자가 주입 BackSpace 카운터: N+1 (마지막 1개는 commit 트리거로 소비)
                    self.self_backspace_pending = delete_chars + 1;
                    // commit/preedit 분리 저장 (순방향: preedit 있음, 역방향: 없음)
                    self.deferred_autofix = Some((commit_text.clone(), preedit_text.clone()));

                    // BackSpace를 XTEST 확장으로 일괄 주입 (GTK3 패턴)
                    // XSendEvent는 send_event=True 때문에 modern app이 무시하므로
                    // XTestFakeKeyEvent 사용 (실제 하드웨어 이벤트로 인식)
                    unsafe {
                        let bs_keycode = x11::xlib::XKeysymToKeycode(self.display, 0xff08);
                        for _ in 0..delete_chars + 1 {
                            x11::xtest::XTestFakeKeyEvent(
                                self.display,
                                bs_keycode as u32,
                                1, // KeyPress
                                0,
                            );
                            x11::xtest::XTestFakeKeyEvent(
                                self.display,
                                bs_keycode as u32,
                                0, // KeyRelease
                                0,
                            );
                            // per-BS flush + 10ms 간격: 앱이 각 BS를 순차 처리할 시간 확보
                            x11::xlib::XFlush(self.display);
                            std::thread::sleep(std::time::Duration::from_millis(10));
                        }
                    }
                } else {
                    unim_log!("XIM_HANDLER", "AutoTypeFix: 활성 IC 없음, 무시");
                }
            }
            PopupEvent::CommitText { text } => {
                // 우리가 보낸 Reset 이 되쏘는 메아리는 여기서 걸러낸다. 이미
                // ResetIcReply 로 같은 글자를 제자리에 커밋했으므로, 이걸 통과시키면
                // 캐럿이 옮겨간 자리에 한 번 더 박힌다. (pending_skip_commit 주석 참조)
                if self.take_pending_skip_commit(&text) {
                    unim_log!("XIM_HANDLER", "CommitText 시그널 dedupe skip: '{}'", text);
                    return Ok(());
                }
                // Standalone popup 마우스 클릭 시 커밋. last_focused_ic_info에 캐시된
                // (client_win, im_id, ic_id)로 InputContext를 재구성해서 server.commit
                // 호출. server.commit 내부는 client_win + im_id + ic_id만 사용하므로
                // locale은 빈 문자열로 충분.
                if let Some((cw, im_id, ic_id)) = self.last_focused_ic_info {
                    let ic = InputContext::new(cw, im_id, ic_id, String::new());
                    match server.commit(&ic, &text) {
                        Ok(()) => unim_log!(
                            "XIM_HANDLER",
                            "CommitText (Standalone): '{}' → server.commit OK",
                            text
                        ),
                        Err(e) => unim_log!(
                            "XIM_HANDLER",
                            "CommitText (Standalone): '{}' → server.commit 실패: {:?}",
                            text,
                            e
                        ),
                    }
                } else {
                    unim_log!(
                        "XIM_HANDLER",
                        "CommitText (Standalone): '{}' → 활성 IC 없음, 무시",
                        text
                    );
                }
            }
            PopupEvent::HanjaBookmarkChanged { .. } => {
                // Standalone 모드: unim-gui-gtk가 처리
            }
            PopupEvent::HanjaCandidatesReordered { .. } => {
                // Standalone 모드: unim-gui-gtk가 처리
            }
        }
        Ok(())
    }

    /// AutoTypeFix 지연 교정 처리 (메인 루프에서 호출)
    ///
    /// GTK3의 g_idle_add 패턴 적용:
    /// handle_forward_event에서 마지막 BackSpace의 ForwardEvent가
    /// 전송·flush된 후, 메인 루프로 돌아와서 교정 텍스트를 commit.
    /// 이렇게 해야 앱이 BS 처리 → commit 수신 순서를 보장받음.
    pub fn process_deferred_autofix<C: Connection + HasConnection>(
        &mut self,
        _server: &mut X11rbServer<C>,
    ) {
        // BS가 아직 남아있으면 대기
        if self.self_backspace_pending > 0 || self.deferred_autofix.is_none() {
            return;
        }

        // N+1 방식: commit+preedit은 handle_forward_event의 pending==0에서 직접 처리
        // process_deferred_autofix는 더 이상 사용하지 않음 (폴백 정리만)
        if self.deferred_autofix.is_some() {
            unim_log!(
                "XIM_HANDLER",
                "AutoTypeFix: deferred_autofix 폴백 정리 (BS 미도달)"
            );
            self.deferred_autofix = None;
            self.autofix_context_path = None;
        }
    }
}

impl Drop for UnimHandler {
    fn drop(&mut self) {
        // 모든 PeWindow 정리
        for (_, pe) in self.preedit_windows.drain() {
            pe.clean(self.display, self.screen);
        }
        // Display 닫기
        if !self.display.is_null() {
            unsafe { x11::xlib::XCloseDisplay(self.display) };
        }
    }
}

impl<C: Connection + xim::x11rb::HasConnection> ServerHandler<X11rbServer<C>> for UnimHandler {
    type InputStyleArray = [InputStyle; 3];
    type InputContextData = UnimInputContext;

    fn new_ic_data(
        &mut self,
        _server: &mut X11rbServer<C>,
        input_style: InputStyle,
    ) -> Result<Self::InputContextData, ServerError> {
        unim_log!(
            "XIM_HANDLER",
            "새 IC 데이터 생성 (style: {:?})",
            input_style
        );

        // DBus를 통해 InputContext 생성 (재시도 포함)
        let mut context_path = None;
        for attempt in 1..=3 {
            match self.send_dbus_request(DbusRequest::CreateContext {
                client_name: "unim-xim".to_string(),
                window_id: "unim-xim".to_string(),
                response: None,
            }) {
                Some(DbusResponse::ContextCreated { path }) => {
                    context_path = Some(path);
                    break;
                }
                Some(DbusResponse::ContextCreationFailed) => {
                    unim_log!(
                        "XIM_HANDLER",
                        "DBus 컨텍스트 생성 실패 (시도 {}/3)",
                        attempt
                    );
                }
                _ => {
                    unim_log!(
                        "XIM_HANDLER",
                        "DBus 응답 없음 (시도 {}/3) - daemon 실행 여부 확인 필요",
                        attempt
                    );
                }
            }
            // 마지막 시도가 아니면 잠시 대기 후 재시도
            if attempt < 3 {
                std::thread::sleep(std::time::Duration::from_millis(100));
            }
        }

        match context_path {
            Some(path) => Ok(UnimInputContext::new(path, input_style)),
            None => {
                unim_log!(
                    "XIM_HANDLER",
                    "DBus 컨텍스트 생성 최종 실패 - unim-daemon 실행 필요"
                );
                Err(ServerError::Other(Box::new(std::io::Error::new(
                    std::io::ErrorKind::NotConnected,
                    "unim-daemon not running",
                ))))
            }
        }
    }

    fn input_styles(&self) -> Self::InputStyleArray {
        [
            InputStyle::PREEDIT_CALLBACKS | InputStyle::STATUS_NOTHING,
            InputStyle::PREEDIT_NOTHING | InputStyle::STATUS_NOTHING,
            InputStyle::PREEDIT_POSITION | InputStyle::STATUS_NOTHING,
        ]
    }

    fn filter_events(&self) -> u32 {
        EVENT_MASK
    }

    fn handle_connect(&mut self, _server: &mut X11rbServer<C>) -> Result<(), ServerError> {
        unim_log!("XIM_HANDLER", "XIM 클라이언트 연결됨");
        Ok(())
    }

    fn handle_create_ic(
        &mut self,
        server: &mut X11rbServer<C>,
        user_ic: &mut UserInputContext<Self::InputContextData>,
    ) -> Result<(), ServerError> {
        unim_log!(
            "XIM_HANDLER",
            "IC 생성: id={:?}, style={:?}",
            user_ic.ic.input_context_id(),
            user_ic.ic.input_style()
        );
        server.set_event_mask(&user_ic.ic, 1, 0)
    }

    fn handle_destroy_ic(
        &mut self,
        _server: &mut X11rbServer<C>,
        user_ic: UserInputContext<Self::InputContextData>,
    ) -> Result<(), ServerError> {
        unim_log!(
            "XIM_HANDLER",
            "IC 삭제: id={:?}",
            user_ic.ic.input_context_id()
        );

        // DBus 컨텍스트 파괴
        let _ = self.dbus_tx.blocking_send(DbusRequest::DestroyContext {
            context_path: user_ic.user_data.context_path.clone(),
        });

        if let Some(pe_id) = user_ic.user_data.pe_window {
            if let Some(pe) = self.preedit_windows.remove(&pe_id) {
                pe.clean(self.display, self.screen);
            }
        }

        Ok(())
    }

    fn handle_reset_ic(
        &mut self,
        server: &mut X11rbServer<C>,
        user_ic: &mut UserInputContext<Self::InputContextData>,
    ) -> Result<String, ServerError> {
        unim_log!("XIM_HANDLER", "reset 호출");

        let preedit = user_ic.user_data.preedit_cache.clone();

        // 이 문자열은 아래에서 ResetIcReply 로 동기 반환되고, 클라이언트가 조합이
        // 시작된 자리에 커밋한다. 곧이어 보내는 Reset 때문에 데몬이 같은 글자를
        // CommitText 시그널로 되쏘므로, 그 메아리를 1회 skip 하도록 표시해 둔다.
        if !preedit.is_empty() {
            self.pending_skip_commit = Some(preedit.clone());
        }

        // DBus Reset 호출
        let _ = self.dbus_tx.blocking_send(DbusRequest::Reset {
            context_path: user_ic.user_data.context_path.clone(),
        });

        self.clear_preedit(server, user_ic)?;

        Ok(preedit)
    }

    fn handle_set_focus(
        &mut self,
        _server: &mut X11rbServer<C>,
        user_ic: &mut UserInputContext<Self::InputContextData>,
    ) -> Result<(), ServerError> {
        unim_log!(
            "XIM_HANDLER",
            "포커스 인: id={:?}",
            user_ic.ic.input_context_id()
        );

        // AutoTypeFix: 포커스된 IC 정보 저장
        self.last_focused_ic_info = Some((
            user_ic.ic.client_win(),
            user_ic.ic.input_method_id(),
            user_ic.ic.input_context_id(),
        ));
        self.last_focused_app_window = user_ic.ic.app_win().map(|w| w.get() as u64);
        self.last_focused_context_path = Some(user_ic.user_data.context_path.clone());

        let window_id = format!(
            "xim-win-0x{:x}",
            user_ic.ic.app_win().map(u32::from).unwrap_or(0)
        );
        let _ = self.dbus_tx.blocking_send(DbusRequest::FocusIn {
            context_path: user_ic.user_data.context_path.clone(),
            window_id,
        });

        Ok(())
    }

    fn handle_unset_focus(
        &mut self,
        server: &mut X11rbServer<C>,
        user_ic: &mut UserInputContext<Self::InputContextData>,
    ) -> Result<(), ServerError> {
        unim_log!("XIM_HANDLER", "focus_out 호출");

        // AutoTypeFix 진행 중이면 폐기 (포커스 변경 시 BS가 잘못된 창에 갈 수 있음)
        if self.deferred_autofix.is_some() || self.self_backspace_pending > 0 {
            unim_log!(
                "XIM_HANDLER",
                "focus_out: AutoTypeFix 진행 중 폐기 (pending_bs={}, deferred={:?})",
                self.self_backspace_pending,
                self.deferred_autofix.is_some()
            );
            self.self_backspace_pending = 0;
            self.deferred_autofix = None;
            self.autofix_context_path = None;
        }

        // 로컬 preedit_cache로 즉시 커밋 (DBus 라운드트립 없이)
        let cached_preedit = user_ic.user_data.preedit_cache.clone();
        if !cached_preedit.is_empty() {
            unim_log!(
                "XIM_HANDLER",
                "focus_out: 로컬 캐시 커밋 \"{}\"",
                cached_preedit
            );
            server.commit(&user_ic.ic, &cached_preedit)?;
        }

        // 데몬에 FocusOut 전송 (응답 대기 없이 fire-and-forget)
        // 데몬은 엔진 상태를 리셋하고 입력 모드를 유지함
        let _ = self.dbus_tx.blocking_send(DbusRequest::FocusOut {
            context_path: user_ic.user_data.context_path.clone(),
            response: None,
        });

        self.clear_preedit(server, user_ic)?;

        Ok(())
    }

    fn handle_set_ic_values(
        &mut self,
        _server: &mut X11rbServer<C>,
        user_ic: &mut UserInputContext<Self::InputContextData>,
    ) -> Result<(), ServerError> {
        unim_log!(
            "XIM_HANDLER",
            "IC 값 설정 (spot: {:?})",
            user_ic.ic.preedit_spot()
        );

        // spot_location 변경 시 커서 위치를 데몬에 보고
        let spot = user_ic.ic.preedit_spot();
        let app_win = user_ic.ic.app_win();
        let (abs_x, abs_y) = if let Some(win) = app_win {
            unsafe {
                let mut child_return: x11::xlib::Window = 0;
                let mut rx = 0i32;
                let mut ry = 0i32;
                x11::xlib::XTranslateCoordinates(
                    self.display,
                    win.get() as x11::xlib::Window,
                    x11::xlib::XRootWindow(self.display, self.screen),
                    0,
                    0,
                    &mut rx,
                    &mut ry,
                    &mut child_return,
                );
                (rx, ry)
            }
        } else {
            (0, 0)
        };
        let cursor_x = abs_x + spot.x as i32;
        let cursor_y = abs_y + spot.y as i32;
        let _ = self.dbus_tx.blocking_send(DbusRequest::ReportCursorRect {
            context_path: user_ic.user_data.context_path.clone(),
            x: cursor_x,
            y: cursor_y,
            width: 0,
            height: 20,
        });

        // spot_location 변경 시 preedit 윈도우 재생성
        // 단, preedit이 활성 상태일 때만 (preedit_cache가 비어있지 않을 때)
        // 포커스 해제 후에는 preedit 작업을 하지 않음
        if user_ic.user_data.pe_window.is_some() && !user_ic.user_data.preedit_cache.is_empty() {
            let preedit = user_ic.user_data.preedit_cache.clone();
            // PeWindow만 갱신 (XIM preedit 상태는 건드리지 않음)
            if let Some(pe_id) = user_ic.user_data.pe_window {
                if let Some(pe) = self.preedit_windows.get_mut(&pe_id) {
                    pe.set_preedit(&preedit);
                    pe.refresh(self.display);
                }
            }
        }

        Ok(())
    }

    fn handle_forward_event(
        &mut self,
        server: &mut X11rbServer<C>,
        user_ic: &mut UserInputContext<Self::InputContextData>,
        xev: &KeyPressEvent,
    ) -> Result<bool, ServerError> {
        // ======================================================================
        // [중요] WezTerm 호환성: KeyRelease 이벤트 무시
        // ======================================================================
        // WezTerm(xcb-imdkit 기반)은 KeyPress와 KeyRelease를 모두 ForwardEvent로
        // 전송하여 이중 입력 문제가 발생함. KeyRelease는 문자 입력에 사용되지
        // 않으므로 무시해도 안전함.
        // response_type: 2=KeyPress, 3=KeyRelease
        // ======================================================================
        const KEY_RELEASE: u8 = 3;

        // ======================================================================
        // AutoTypeFix commit/preedit 재진입 가드
        // ======================================================================
        // XIM crate는 server.commit()/preedit_draw() 호출 시 내부적으로
        // keycode=0 가상 KeyPress 이벤트를 handle_forward_event로 재진입시킨다.
        // 이를 그대로 엔진에 넘기면 조합 상태가 오염되므로 소비 처리.
        if self.autofix_commit_guard {
            return Ok(true);
        }

        // ======================================================================
        // AutoTypeFix 자가 주입 BackSpace 패스스루
        // ======================================================================
        // XTestFakeKeyEvent로 쏜 BackSpace는 X 서버가 진짜 키로 간주해 XIM 서버에
        // 재진입한다. 이를 엔진에 넘기면 한글 조합 상태가 오염되므로 여기서
        // 가로채 Ok(false)로 반환하여 클라이언트(앱)가 직접 처리하도록 한다.
        // KeyPress/KeyRelease 쌍 모두 통과시키되, 카운터는 KeyPress에서만 감소.
        let bs_keysym = unsafe { x11::xlib::XKeycodeToKeysym(self.display, xev.detail, 0) as u32 };
        if bs_keysym == 0xff08 && self.self_backspace_pending > 0 {
            if xev.response_type != KEY_RELEASE {
                // KeyPress: 카운터 감소, 앱에 통과
                self.self_backspace_pending -= 1;
                unim_log!(
                    "XIM_HANDLER",
                    "AutoTypeFix self-BackSpace 패스스루 (남은={})",
                    self.self_backspace_pending
                );

                // 마지막 BackSpace KeyPress — 앱에 통과(Ok(false)) 후
                // XIM 응답이 먼저 전송되고 commit 패킷이 뒤따르므로
                // 앱이 BS 처리 → commit 수신 순서가 보장됨.
                // (XIM 클라이언트는 KeyRelease를 forward하지 않아
                //  KeyRelease 대기 방식은 동작하지 않음)
                if self.self_backspace_pending == 0 {
                    // N+1번째(마지막) BS: 앱에 전달하지 않고 소비(Ok(true))
                    // 여기서 진짜 user_ic.ic로 commit+preedit 실행
                    // 앞선 N개 BS는 이미 Ok(false)로 전달+flush 완료 → 순서 보장
                    if let Some((commit_text, preedit_text)) = self.deferred_autofix.take() {
                        let has_preedit = !preedit_text.is_empty();
                        self.autofix_commit_guard = true;

                        // [1단계] commit 전송 + flush
                        if !commit_text.is_empty() {
                            let _ = server.commit(&user_ic.ic, &commit_text);
                            server.conn().flush().ok();
                            unim_log!(
                                "XIM_HANDLER",
                                "AutoTypeFix commit '{}' (진짜 IC)",
                                commit_text
                            );
                        }

                        // [2단계] preedit 전송 (commit과 분리 — 10ms 간격)
                        // Chrome 등 XIM 클라이언트가 commit 처리 시 preedit을 초기화하므로
                        // commit flush 후 간격을 두어 클라이언트가 commit을 완전히 처리한 뒤
                        // 새 preedit을 수신하도록 한다.
                        if has_preedit {
                            std::thread::sleep(std::time::Duration::from_millis(10));
                            self.preedit(server, user_ic, &preedit_text)?;
                            server.conn().flush().ok();
                            unim_log!(
                                "XIM_HANDLER",
                                "AutoTypeFix preedit '{}' (진짜 IC)",
                                preedit_text
                            );
                        }

                        self.autofix_commit_guard = false;

                        if !has_preedit {
                            if let Some(path) = self.autofix_context_path.take() {
                                let _ = self
                                    .dbus_tx
                                    .blocking_send(DbusRequest::Reset { context_path: path });
                            }
                        }
                    }
                    return Ok(true); // N+1번째 BS 소비 (앱에 안 감)
                }
            }
            // BS 1..N: 앱이 실제로 글자를 지우도록 XIM이 소비하지 않음
            return Ok(false);
        }

        if xev.response_type == KEY_RELEASE {
            unim_log!(
                "XIM_HANDLER",
                "KeyRelease 이벤트 무시: type={}, keycode={}",
                xev.response_type,
                xev.detail
            );
            return Ok(true); // 소비된 것으로 처리
        }

        let evdev_code = xev.detail.saturating_sub(8);

        unim_log!(
            "XIM_HANDLER",
            "키 입력: type={}, keycode={}, evdev={}, state={:?}",
            xev.response_type,
            xev.detail,
            evdev_code,
            xev.state
        );

        // ======================================================================
        // 팝업 활성 상태: 모든 키를 엔진(ProcessKeyEvent)에 위임
        // ======================================================================
        // GNOME extension 기준 모델: 팝업은 순수 UI, 키 처리는 엔진이 전담.
        // 엔진이 PopupNavigate/HidePopup 시그널로 팝업 상태를 갱신한다.
        // keysym은 한자키 체크에도 사용.
        //
        // P0-3: XkbKeycodeToKeysym(group, level) — Shift/CapsLock 반영.
        // state 비트에서 level을 산출: ShiftMask(1) 또는 LockMask(2) 적용 시 level=1.
        // group은 일반적으로 0 (다중 키보드 그룹 레이아웃 미사용 시 무해).
        let xstate = u16::from(xev.state) as u32;
        let level = if (xstate & (x11::xlib::ShiftMask | x11::xlib::LockMask)) != 0 {
            1i32
        } else {
            0i32
        };
        let keysym = unsafe {
            x11::xlib::XkbKeycodeToKeysym(self.display, xev.detail, 0, level) as u32
        };

        // ======================================================================
        // 한자/특수문자 키 처리 (설정 기반) — Standalone 경로
        // ======================================================================
        // GetHanjaCandidates / GetSpecialCharCandidates RPC 호출로 데몬이
        // ShowHanjaPopup / ShowSpecialPopup 시그널을 발행하고 unim-gui-gtk가 팝업 표시.
        // 특수문자 후보 있음 → 키 consumed (ProcessKey 재위임 금지 — emoji 중첩 방지).
        // 후보 없음 → ProcessKey 위임 → 데몬의 dual-purpose Hanja가 emoji 트리거.

        if self.hanja_keysyms.contains(&keysym) {
            let ctx_path = user_ic.user_data.context_path.clone();

            // 1. 한자 후보 우선 시도
            let response = self.send_dbus_request(DbusRequest::GetHanjaCandidates {
                context_path: ctx_path.clone(),
                response: None,
            });

            if let Some(DbusResponse::HanjaCandidates { target, candidates }) = response {
                if !candidates.is_empty() {
                    unim_log!(
                        "XIM_HANDLER",
                        "한자 후보 있음 (Standalone) → 키 consumed: target='{}', count={}",
                        target,
                        candidates.len()
                    );
                    // ShowHanjaPopup 시그널은 GetHanjaCandidates RPC 응답에서 데몬이 이미 발행.
                    return Ok(true);
                }
            }

            // 2. 한자 후보 없으면 특수문자 후보로 폴백
            let special_response = self.send_dbus_request(DbusRequest::GetSpecialCharCandidates {
                context_path: ctx_path.clone(),
                response: None,
            });

            if let Some(DbusResponse::SpecialCharCandidates {
                target,
                characters,
                top_row: _,
            }) = special_response
            {
                if !characters.is_empty() {
                    unim_log!(
                        "XIM_HANDLER",
                        "특수문자 후보 있음 (Standalone) → 키 consumed: target='{}'",
                        target
                    );
                    // ShowSpecialPopup 시그널은 GetSpecialCharCandidates RPC 응답에서 데몬이 이미 발행.
                    return Ok(true);
                }
            }
            unim_log!(
                "XIM_HANDLER",
                "한자/특수문자 후보 없음 → idle Hanja: ProcessKey 위임"
            );
            // fall-through: 엔진의 dual-purpose Hanja 분기가 emoji popup 트리거 →
            // ShowEmojiPopupV2 signal 발행 → unim-gui-gtk가 popup 표시.
        }

        // ======================================================================
        // 일반 키 처리
        // ======================================================================

        // DBus를 통해 키 이벤트 처리.
        // keyval 은 GTK/Qt 와 동일한 X keysym (예: 한자키 0xff34, F9 0xffc6) 을
        // 보내야 한다 — `xev.detail` 은 X11 hardware keycode (예: 131) 이라
        // 데몬이 dual-purpose Hanja 분기를 못 잡아 idle Hanja → emoji popup
        // 트리거가 동작하지 않는다. line 1414 에서 이미 산출한 keysym 사용.
        let response = self.send_dbus_request(DbusRequest::ProcessKey {
            context_path: user_ic.user_data.context_path.clone(),
            keyval: keysym,
            keycode: evdev_code as u32,
            state: u16::from(xev.state) as u32,
            response: None,
        });

        let (consumed, preedit, commit) = match response {
            Some(DbusResponse::KeyProcessed {
                consumed,
                preedit,
                commit,
            }) => (consumed, preedit, commit),
            _ => {
                // DBus 실패 시 키 통과
                return Ok(false);
            }
        };

        // Commit + Preedit 처리
        //
        // 과거에는 commit과 preedit을 같은 frame에서 발사한 뒤 하나의 flush로
        // atomic batch 전송했으나, 일부 XIM 클라이언트(Chrome 등)가 commit 처리
        // 도중 preedit을 초기화하면서 새 preedit을 놓치는 race가 있었다
        // (예: 두벌식 ㄹㄹㄹ 5연타 시 두 번째 ㄹ 시각화 누락).
        //
        // 해결: commit_then_preedit() 헬퍼로 [commit→flush→10ms→preedit→flush]
        // 분리 송출. AutoTypeFix N+1 BS 분기와 동일한 패턴.
        let commit_text = commit.unwrap_or_default();
        let preedit_text = preedit.unwrap_or_default();

        if !commit_text.is_empty() {
            unim_log!("XIM_HANDLER", "커밋: \"{}\"", commit_text);
        }
        if !preedit_text.is_empty() {
            unim_log!("XIM_HANDLER", "Preedit: \"{}\"", preedit_text);
        }

        self.commit_then_preedit(server, user_ic, &commit_text, &preedit_text)?;

        Ok(consumed)
    }
}
