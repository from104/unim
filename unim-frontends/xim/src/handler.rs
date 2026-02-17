//! XIM 핸들러 모듈
//!
//! XIM 프로토콜 이벤트를 처리하고 DBus를 통해 UNIM 데몬과 연동합니다.

use std::ffi::CString;
use std::num::NonZeroU32;
use std::os::raw::c_int;

use ahash::AHashMap;
use tokio::sync::mpsc;
use x11rb::connection::Connection;
use x11rb::protocol::xproto::ConnectionExt as XprotoConnectionExt;
use x11rb::protocol::xproto::{ConfigureNotifyEvent, EventMask, GrabMode, KeyPressEvent};

use unim::config::Config;
use unim::unim_log;

use xim::{
    x11rb::{HasConnection, X11rbServer},
    InputStyle, Server, ServerError, ServerHandler, UserInputContext,
};

use crate::dbus_client::{DbusRequest, DbusResponse};
use crate::hanja_window::{HanjaAction, HanjaWindow};
use crate::pe_window::PeWindow;

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
    /// 한자 후보 팝업 윈도우
    hanja_window: Option<HanjaWindow>,
    /// 한자 팝업이 활성일 때의 컨텍스트 경로
    hanja_context_path: Option<String>,
    /// 한자 팝업 활성 시 클라이언트 윈도우 ID (합성 Escape 전송용)
    hanja_client_window: Option<u64>,
}

impl UnimHandler {
    pub fn new(screen_num: usize, config: Config, dbus_tx: mpsc::Sender<DbusRequest>) -> Self {
        // 단일 Xlib Display 연결 열기
        let display = unsafe {
            let display_name = std::env::var("DISPLAY").unwrap_or_else(|_| ":0".to_string());
            let display_cstr = CString::new(display_name).unwrap();
            x11::xlib::XOpenDisplay(display_cstr.as_ptr())
        };

        if display.is_null() {
            panic!("XOpenDisplay failed");
        }

        Self {
            config,
            preedit_windows: AHashMap::new(),
            display,
            screen: screen_num as c_int,
            dbus_tx,
            hanja_window: None,
            hanja_context_path: None,
            hanja_client_window: None,
        }
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
        // 한자 팝업 Expose
        if let Some(ref mut hw) = self.hanja_window {
            if hw.window_id() == window {
                hw.expose(self.display);
                return Ok(());
            }
        }

        if let Some(w) = NonZeroU32::new(window) {
            if let Some(pe) = self.preedit_windows.get_mut(&w) {
                pe.expose(self.display);
            }
        }

        Ok(())
    }

    /// 한자 팝업 외부 클릭 감지 (grab_pointer에 의한 ButtonPress)
    pub fn handle_button_press<C: Connection + xim::x11rb::HasConnection>(
        &mut self,
        event_x: i16,
        event_y: i16,
        conn: &C,
    ) -> Result<bool, x11rb::errors::ConnectionError> {
        if let Some(ref hw) = self.hanja_window {
            let (w, h) = (hw.size().0 as i16, hw.size().1 as i16);
            // 클릭이 팝업 내부인지 확인
            if event_x >= 0 && event_y >= 0 && event_x < w && event_y < h {
                // 팝업 내부 클릭 → 무시 (키보드로만 선택)
                return Ok(false);
            }
            // 팝업 외부 클릭 → ungrab + 합성 Escape 전송
            // (클라이언트가 XFilterEvent로 XIM 서버에 전달 → HanjaAction::Cancel 경로에서 커밋)
            unim_log!(
                "XIM_HANDLER",
                "한자 팝업 외부 클릭 감지 -> 합성 Escape 전송"
            );

            // 1. ungrab pointer
            conn.ungrab_pointer(x11rb::CURRENT_TIME)?;
            conn.flush()?;

            // 2. 클라이언트 윈도우에 합성 Escape KeyPress 전송
            if let Some(client_win) = self.hanja_client_window {
                unsafe {
                    let root = x11::xlib::XRootWindow(self.display, self.screen);
                    let keycode = x11::xlib::XKeysymToKeycode(
                        self.display,
                        0xff1b, // XK_Escape
                    );
                    let mut event: x11::xlib::XKeyEvent = std::mem::zeroed();
                    event.type_ = x11::xlib::KeyPress;
                    event.display = self.display;
                    event.window = client_win;
                    event.root = root;
                    event.keycode = keycode as u32;
                    event.state = 0;
                    event.time = x11::xlib::CurrentTime as u64;
                    event.send_event = x11::xlib::True;
                    event.same_screen = x11::xlib::True;

                    x11::xlib::XSendEvent(
                        self.display,
                        client_win,
                        x11::xlib::False,
                        x11::xlib::KeyPressMask,
                        &mut event as *mut _ as *mut x11::xlib::XEvent,
                    );
                    x11::xlib::XFlush(self.display);
                }
                unim_log!(
                    "XIM_HANDLER",
                    "합성 Escape 전송 완료: client_win=0x{:x}",
                    client_win
                );
            }
            return Ok(true);
        }
        Ok(false)
    }

    /// 한자 팝업이 활성 상태인지 확인
    pub fn has_hanja_popup(&self) -> bool {
        self.hanja_window.is_some()
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
                pe.configure_notify(event.clone(), self.display);
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
}

impl Drop for UnimHandler {
    fn drop(&mut self) {
        // 한자 팝업 정리
        if let Some(hw) = self.hanja_window.take() {
            hw.clean(self.display, self.screen);
        }
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
        server: &mut X11rbServer<C>,
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

        // 한자 팝업이 열려있으면 닫기
        if self.hanja_window.is_some() {
            if let Some(hw) = self.hanja_window.take() {
                hw.clean(self.display, self.screen);
            }
            self.hanja_context_path = None;
            self.hanja_client_window = None;
            let _ = server.conn().ungrab_pointer(x11rb::CURRENT_TIME);
            server.conn().flush().ok();
        }

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

        // 한자 팝업이 열려있으면 닫기
        if self.hanja_window.is_some() {
            unim_log!("XIM_HANDLER", "reset: 한자 팝업 닫기");
            if let Some(hw) = self.hanja_window.take() {
                hw.clean(self.display, self.screen);
            }
            if let Some(ctx) = self.hanja_context_path.take() {
                let _ = self
                    .dbus_tx
                    .blocking_send(DbusRequest::CancelHanja { context_path: ctx });
            }
            let _ = server.conn().ungrab_pointer(x11rb::CURRENT_TIME);
            server.conn().flush().ok();
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

        // 한자 팝업이 열려있으면 닫기
        if self.hanja_window.is_some() {
            unim_log!("XIM_HANDLER", "focus_out: 한자 팝업 닫기");
            if let Some(hw) = self.hanja_window.take() {
                hw.clean(self.display, self.screen);
            }
            if let Some(ctx) = self.hanja_context_path.take() {
                let _ = self
                    .dbus_tx
                    .blocking_send(DbusRequest::CancelHanja { context_path: ctx });
            }
            let _ = server.conn().ungrab_pointer(x11rb::CURRENT_TIME);
            server.conn().flush().ok();
        }

        // DBus FocusOut 호출 - 커밋 텍스트 반환
        if let Some(DbusResponse::CommitText { text }) =
            self.send_dbus_request(DbusRequest::FocusOut {
                context_path: user_ic.user_data.context_path.clone(),
                response: None,
            })
        {
            if !text.is_empty() {
                unim_log!("XIM_HANDLER", "commit_and_clear: \"{}\"", text);
                server.commit(&user_ic.ic, &text)?;
            }
        }

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
        if xev.response_type == KEY_RELEASE {
            unim_log!(
                "XIM_HANDLER",
                "KeyRelease 이벤트 무시: type={}, keycode={}",
                xev.response_type,
                xev.detail
            );
            return Ok(true); // 소비된 것으로 처리
        }

        let evdev_code = if xev.detail > 8 { xev.detail - 8 } else { 0 };

        unim_log!(
            "XIM_HANDLER",
            "키 입력: type={}, keycode={}, evdev={}, state={:?}",
            xev.response_type,
            xev.detail,
            evdev_code,
            xev.state
        );

        // ======================================================================
        // 한자 팝업이 활성 상태일 때: 키를 팝업에 전달
        // ======================================================================
        // keysym을 x11rb의 keycode로부터 변환
        let keysym = unsafe { x11::xlib::XKeycodeToKeysym(self.display, xev.detail, 0) as u32 };

        if self.hanja_window.is_some() {
            let action = {
                let hw = self.hanja_window.as_mut().unwrap();
                let action = hw.handle_key(keysym);
                // 페이지 변경/선택 이동 시 즉시 다시 그리기
                match &action {
                    HanjaAction::NextPage
                    | HanjaAction::PrevPage
                    | HanjaAction::Consumed
                    | HanjaAction::None => {
                        hw.redraw(self.display);
                    }
                    _ => {}
                }
                action
            };

            match action {
                HanjaAction::Select(index) => {
                    // 한자 선택 → DBus SelectHanja
                    let ctx_path = self.hanja_context_path.clone().unwrap_or_default();
                    let response = self.send_dbus_request(DbusRequest::SelectHanja {
                        context_path: ctx_path,
                        index,
                        response: None,
                    });
                    if let Some(DbusResponse::HanjaSelected { commit }) = response {
                        if !commit.is_empty() {
                            self.clear_preedit(server, user_ic)?;
                            server.conn().flush().ok();
                            unim_log!("XIM_HANDLER", "한자 커밋: \"{}\"", commit);
                            server.commit(&user_ic.ic, &commit)?;
                            server.conn().flush().ok();
                            // preedit도 정리
                            server.preedit_draw(&mut user_ic.ic, "")?;
                            server.conn().flush().ok();
                        }
                    }
                    // 팝업 닫기
                    if let Some(hw) = self.hanja_window.take() {
                        hw.clean(self.display, self.screen);
                    }
                    self.hanja_context_path = None;
                    self.hanja_client_window = None;
                    // ungrab pointer
                    let _ = server.conn().ungrab_pointer(x11rb::CURRENT_TIME);
                    server.conn().flush().ok();
                    return Ok(true);
                }
                HanjaAction::Cancel => {
                    // 한자 취소 → DBus CancelHanja
                    let ctx_path = self.hanja_context_path.clone().unwrap_or_default();
                    // 조합 중 preedit 커밋 후 닫기
                    let response = self.send_dbus_request(DbusRequest::ProcessKey {
                        context_path: ctx_path.clone(),
                        keyval: 0, // reset용 더미
                        keycode: 0,
                        state: 0,
                        response: None,
                    });
                    // reset을 보냄
                    self.send_dbus_request(DbusRequest::CancelHanja {
                        context_path: ctx_path,
                    });
                    // preedit 복원
                    if let Some(DbusResponse::KeyProcessed {
                        preedit, commit, ..
                    }) = response
                    {
                        if let Some(ct) = commit {
                            if !ct.is_empty() {
                                self.clear_preedit(server, user_ic)?;
                                server.commit(&user_ic.ic, &ct)?;
                                server.conn().flush().ok();
                            }
                        }
                        if let Some(pt) = preedit {
                            if !pt.is_empty() {
                                self.preedit(server, user_ic, &pt)?;
                            } else {
                                self.clear_preedit(server, user_ic)?;
                            }
                            server.conn().flush().ok();
                        }
                    }
                    // 팝업 닫기
                    if let Some(hw) = self.hanja_window.take() {
                        hw.clean(self.display, self.screen);
                    }
                    self.hanja_context_path = None;
                    self.hanja_client_window = None;
                    // ungrab pointer
                    let _ = server.conn().ungrab_pointer(x11rb::CURRENT_TIME);
                    server.conn().flush().ok();
                    return Ok(true);
                }
                HanjaAction::NextPage | HanjaAction::PrevPage => {
                    return Ok(true); // 이미 redraw 완료
                }
                HanjaAction::Consumed => {
                    // 팝업이 내부 처리한 키 (↑↓ 등) → redraw 후 소비
                    return Ok(true);
                }
                HanjaAction::None => {
                    // 팝업이 처리하지 않은 키 → 조합 커밋 + 팝업 닫고 엔진에 키 전달
                    unim_log!(
                        "XIM_HANDLER",
                        "한자 팝업 미지원 키 -> 조합 커밋 + 팝업 닫고 엔진에 키 전달"
                    );

                    // 1. 조합 중인 한글 커밋 (FocusOut → commit)
                    let ctx_path = self
                        .hanja_context_path
                        .clone()
                        .unwrap_or_else(|| user_ic.user_data.context_path.clone());
                    if let Some(DbusResponse::CommitText { text }) =
                        self.send_dbus_request(DbusRequest::FocusOut {
                            context_path: ctx_path.clone(),
                            response: None,
                        })
                    {
                        if !text.is_empty() {
                            unim_log!("XIM_HANDLER", "조합 커밋: \"{}\"", text);
                            self.clear_preedit(server, user_ic)?;
                            server.commit(&user_ic.ic, &text)?;
                            server.conn().flush().ok();
                        }
                    }
                    self.clear_preedit(server, user_ic)?;

                    // 2. 한자 팝업 닫기
                    if let Some(hw) = self.hanja_window.take() {
                        hw.clean(self.display, self.screen);
                    }

                    // 3. CancelHanja
                    if let Some(ctx) = self.hanja_context_path.take() {
                        let _ = self
                            .dbus_tx
                            .blocking_send(DbusRequest::CancelHanja { context_path: ctx });
                    }
                    self.hanja_client_window = None;

                    // 4. ungrab pointer
                    let _ = server.conn().ungrab_pointer(x11rb::CURRENT_TIME);
                    server.conn().flush().ok();

                    // fall-through: 아래 ProcessKey 경로에서 엔진이 새 키를 처리
                }
            }
        }

        // ======================================================================
        // 한자 키 (F9 / Hangul_Hanja) 처리
        // ======================================================================
        const XK_HANGUL_HANJA: u32 = 0xff34;
        const XK_F9: u32 = 0xffc6;

        if keysym == XK_HANGUL_HANJA || keysym == XK_F9 {
            let ctx_path = user_ic.user_data.context_path.clone();
            let response = self.send_dbus_request(DbusRequest::GetHanjaCandidates {
                context_path: ctx_path.clone(),
                response: None,
            });

            if let Some(DbusResponse::HanjaCandidates { target, candidates }) = response {
                if !candidates.is_empty() {
                    unim_log!(
                        "XIM_HANDLER",
                        "한자 후보 표시: target='{}', count={}",
                        target,
                        candidates.len()
                    );

                    // 팝업 위치: preedit spot 기반
                    let spot = user_ic.ic.preedit_spot();
                    let app_win = user_ic.ic.app_win();

                    // 앱 윈도우의 절대 좌표 계산
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

                    let popup_x = abs_x + spot.x as c_int;
                    let popup_y = abs_y + spot.y as c_int + 20; // 커서 아래

                    // 한자 팝업 생성
                    match HanjaWindow::new(self.display, self.screen, popup_x, popup_y) {
                        Ok(mut hw) => {
                            hw.set_candidates(self.display, self.screen, &target, candidates);
                            let popup_wid = hw.window_id();
                            self.hanja_window = Some(hw);
                            self.hanja_context_path = Some(ctx_path);
                            self.hanja_client_window = app_win.map(|w| w.get() as u64);

                            // grab_pointer: 팝업 외부 클릭 감지용
                            let _ = server.conn().grab_pointer(
                                false, // owner_events
                                popup_wid,
                                EventMask::BUTTON_PRESS,
                                GrabMode::ASYNC,
                                GrabMode::ASYNC,
                                x11rb::NONE, // confine_to
                                x11rb::NONE, // cursor
                                x11rb::CURRENT_TIME,
                            );
                            server.conn().flush().ok();
                        }
                        Err(e) => {
                            unim_log!("XIM_HANDLER", "한자 팝업 생성 실패: {}", e);
                        }
                    }

                    return Ok(true);
                }
            }
            unim_log!("XIM_HANDLER", "한자 후보 없음");
            return Ok(true);
        }

        // ======================================================================
        // 일반 키 처리
        // ======================================================================

        // DBus를 통해 키 이벤트 처리
        let response = self.send_dbus_request(DbusRequest::ProcessKey {
            context_path: user_ic.user_data.context_path.clone(),
            keyval: xev.detail as u32,
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

        // Commit 처리
        if let Some(commit_text) = commit {
            if !commit_text.is_empty() {
                self.clear_preedit(server, user_ic)?;
                server.conn().flush().ok();

                unim_log!("XIM_HANDLER", "커밋: \"{}\"", commit_text);
                server.commit(&user_ic.ic, &commit_text)?;
                server.conn().flush().ok();
            }
        }

        // Preedit 처리
        if let Some(preedit_text) = preedit {
            if preedit_text.is_empty() {
                self.clear_preedit(server, user_ic)?;
            } else {
                unim_log!("XIM_HANDLER", "Preedit: \"{}\"", preedit_text);
                self.preedit(server, user_ic, &preedit_text)?;
            }
            server.conn().flush().ok();
        } else {
            // preedit이 None이지만 캐시에 preedit이 남아있으면 정리
            // (백스페이스로 마지막 글자를 지운 경우 등)
            if !user_ic.user_data.preedit_cache.is_empty() {
                unim_log!("XIM_HANDLER", "Preedit 캐시 정리 (preedit=None)");
                self.clear_preedit(server, user_ic)?;
                server.conn().flush().ok();
            }
        }

        Ok(consumed)
    }
}
