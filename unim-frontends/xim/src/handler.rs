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

use unim::config::{Config, PopupMode};
use unim::unim_log;

use xim::{
    x11rb::{HasConnection, X11rbServer},
    InputStyle, Server, ServerError, ServerHandler, UserInputContext,
};

use crate::dbus_client::{DbusRequest, DbusResponse, PopupEvent};
use crate::hanja_window::{HanjaClickResult, HanjaWindow};
use crate::pe_window::PeWindow;
use crate::special_window::{SpecialClickResult, SpecialWindow};

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
    /// 한자 후보 팝업 윈도우
    hanja_window: Option<HanjaWindow>,
    /// 한자 팝업이 활성일 때의 컨텍스트 경로
    hanja_context_path: Option<String>,
    /// 한자 팝업 활성 시 클라이언트 윈도우 ID (합성 Escape 전송용)
    hanja_client_window: Option<u64>,
    /// 특수문자 팝업 윈도우
    special_window: Option<SpecialWindow>,
    /// 특수문자 팝업이 활성일 때의 컨텍스트 경로
    special_context_path: Option<String>,
    /// 한자/특수문자 키 keysym 목록 (설정 기반)
    hanja_keysyms: Vec<u32>,
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
            hanja_window: None,
            hanja_context_path: None,
            hanja_client_window: None,
            special_window: None,
            special_context_path: None,
            hanja_keysyms,
        })
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
            DbusRequest::CancelSpecialChar { context_path, .. } => {
                DbusRequest::CancelSpecialChar {
                    context_path,
                    response: Some(response_tx),
                }
            }
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

        // 특수문자 팝업 Expose
        if let Some(ref mut sw) = self.special_window {
            if sw.window_id() == window {
                sw.expose(self.display);
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

    /// 합성 키 전송 (마우스 클릭 → 키 이벤트 변환용)
    fn send_synthetic_key(&self, keysym: u64) {
        if let Some(client_win) = self.hanja_client_window {
            unsafe {
                let root = x11::xlib::XRootWindow(self.display, self.screen);
                let keycode = x11::xlib::XKeysymToKeycode(self.display, keysym);
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
        }
    }

    /// 한자/특수문자 팝업 마우스 클릭 처리
    /// 클릭을 합성 키로 변환하여 엔진에 위임
    pub fn handle_button_press<C: Connection + xim::x11rb::HasConnection>(
        &self,
        button: u8,
        event_x: i16,
        event_y: i16,
        conn: &C,
    ) -> Result<bool, x11rb::errors::ConnectionError> {
        // 한자 팝업
        if let Some(ref hw) = self.hanja_window {
            let (w, h) = (hw.size().0 as i16, hw.size().1 as i16);
            if event_x >= 0 && event_y >= 0 && event_x < w && event_y < h {
                let result = hw.handle_button_press(button as u32, event_y as c_int, self.display);
                match result {
                    HanjaClickResult::Select(page_idx) => {
                        let keysym = 0x31 + page_idx as u64; // '1'~'9'
                        self.send_synthetic_key(keysym);
                        unim_log!("XIM_HANDLER", "좌클릭 → 합성 숫자키 '{}' 전송", page_idx + 1);
                    }
                    HanjaClickResult::NextPage => {
                        self.send_synthetic_key(0xff53); // Right
                    }
                    HanjaClickResult::Consumed => {}
                }
                return Ok(true);
            }
            // 외부 클릭 → Escape
            unim_log!("XIM_HANDLER", "한자 팝업 외부 클릭 → 합성 Escape 전송");
            conn.ungrab_pointer(x11rb::CURRENT_TIME)?;
            conn.flush()?;
            self.send_synthetic_key(0xff1b); // Escape
            return Ok(true);
        }

        // 특수문자 팝업
        if let Some(ref sw) = self.special_window {
            let (w, h) = sw.size();
            if event_x >= 0 && event_y >= 0 && event_x < w as i16 && event_y < h as i16 {
                let result = sw.handle_button_press(button as u32, event_x as c_int, event_y as c_int);
                match result {
                    SpecialClickResult::Select(_row, _col) => {
                        // Enter 키로 현재 선택 확정 (엔진이 선택 처리)
                        self.send_synthetic_key(0xff0d); // Enter
                        unim_log!("XIM_HANDLER", "특수문자 좌클릭 → 합성 Enter 전송");
                    }
                    SpecialClickResult::NextPage => {
                        self.send_synthetic_key(0xff09); // Tab
                    }
                    SpecialClickResult::Consumed => {}
                }
                return Ok(true);
            }
            // 외부 클릭 → Escape
            unim_log!("XIM_HANDLER", "특수문자 팝업 외부 클릭 → 합성 Escape 전송");
            conn.ungrab_pointer(x11rb::CURRENT_TIME)?;
            conn.flush()?;
            self.send_synthetic_key(0xff1b); // Escape
            return Ok(true);
        }

        Ok(false)
    }

    /// 한자/특수문자 팝업이 활성 상태인지 확인
    pub fn has_hanja_popup(&self) -> bool {
        self.hanja_window.is_some() || self.special_window.is_some()
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

    /// DBus 팝업 시그널 처리 (main.rs 이벤트 루프에서 호출)
    pub fn handle_popup_event<C: Connection + xim::x11rb::HasConnection>(
        &mut self,
        event: PopupEvent,
        server: &mut X11rbServer<C>,
    ) -> Result<(), ServerError> {
        match event {
            PopupEvent::Navigate {
                page,
                sel_row,
                sel_col,
                ..
            } => {
                // Embedded 모드: 팝업 UI 갱신
                if let Some(ref mut hw) = self.hanja_window {
                    hw.update_from_navigate(
                        page as usize,
                        sel_row as usize,
                        sel_col as usize,
                        self.display,
                    );
                }
                if let Some(ref mut sw) = self.special_window {
                    sw.update_from_navigate(
                        page as usize,
                        sel_row as usize,
                        sel_col as usize,
                        self.display,
                    );
                }
            }
            PopupEvent::Hide => {
                // 팝업 닫기 (엔진이 결정)
                if let Some(hw) = self.hanja_window.take() {
                    hw.clean(self.display, self.screen);
                    unim_log!("XIM_HANDLER", "HidePopup 시그널: 한자 팝업 닫기");
                }
                self.hanja_context_path = None;
                if let Some(sw) = self.special_window.take() {
                    sw.clean(self.display, self.screen);
                    unim_log!("XIM_HANDLER", "HidePopup 시그널: 특수문자 팝업 닫기");
                }
                self.special_context_path = None;
                self.hanja_client_window = None;
                let _ = server.conn().ungrab_pointer(x11rb::CURRENT_TIME);
                server.conn().flush().ok();
            }
            PopupEvent::ShowHanja { .. } | PopupEvent::ShowSpecial { .. } => {
                // Embedded 모드: GetHanjaCandidates에서 직접 처리
                // Standalone 모드: GNOME extension이 처리
                // → 여기서는 무시
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

        // 한자 팝업이 열려있으면 닫기 + 트리거 커밋
        if self.hanja_window.is_some() {
            unim_log!("XIM_HANDLER", "reset: 한자 팝업 닫기");
            if let Some(hw) = self.hanja_window.take() {
                hw.clean(self.display, self.screen);
            }
            if let Some(ctx) = self.hanja_context_path.take() {
                if let Some(DbusResponse::CommitText { text }) =
                    self.send_dbus_request(DbusRequest::CancelHanja {
                        context_path: ctx,
                        response: None,
                    })
                {
                    if !text.is_empty() {
                        unim_log!("XIM_HANDLER", "reset: 한자 트리거 커밋 '{}'", text);
                        let _ = server.commit(&user_ic.ic, &text);
                    }
                }
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

        // 한자 팝업이 열려있으면 닫기 + 트리거 커밋
        if self.hanja_window.is_some() {
            unim_log!("XIM_HANDLER", "focus_out: 한자 팝업 닫기");
            if let Some(hw) = self.hanja_window.take() {
                hw.clean(self.display, self.screen);
            }
            if let Some(ctx) = self.hanja_context_path.take() {
                if let Some(DbusResponse::CommitText { text }) =
                    self.send_dbus_request(DbusRequest::CancelHanja {
                        context_path: ctx,
                        response: None,
                    })
                {
                    if !text.is_empty() {
                        unim_log!("XIM_HANDLER", "focus_out: 한자 트리거 커밋 '{}'", text);
                        let _ = server.commit(&user_ic.ic, &text);
                    }
                }
            }
            let _ = server.conn().ungrab_pointer(x11rb::CURRENT_TIME);
            server.conn().flush().ok();
        }

        // 특수문자 팝업이 열려있으면 닫기 + 트리거 커밋
        if self.special_window.is_some() {
            unim_log!("XIM_HANDLER", "focus_out: 특수문자 팝업 닫기");
            if let Some(sw) = self.special_window.take() {
                sw.clean(self.display, self.screen);
            }
            if let Some(ctx) = self.special_context_path.take() {
                if let Some(DbusResponse::CommitText { text }) =
                    self.send_dbus_request(DbusRequest::CancelSpecialChar {
                        context_path: ctx,
                        response: None,
                    })
                {
                    if !text.is_empty() {
                        unim_log!("XIM_HANDLER", "focus_out: 특수문자 트리거 커밋 '{}'", text);
                        let _ = server.commit(&user_ic.ic, &text);
                    }
                }
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
        // 팝업 활성 상태: 모든 키를 엔진(ProcessKeyEvent)에 위임
        // ======================================================================
        // GNOME extension 기준 모델: 팝업은 순수 UI, 키 처리는 엔진이 전담.
        // 엔진이 PopupNavigate/HidePopup 시그널로 팝업 상태를 갱신한다.
        // keysym은 한자키 체크에도 사용
        let keysym = unsafe { x11::xlib::XKeycodeToKeysym(self.display, xev.detail, 0) as u32 };

        // ======================================================================
        // 한자/특수문자 키 처리 (설정 기반)
        // ======================================================================

        if self.hanja_keysyms.contains(&keysym) {
            let ctx_path = user_ic.user_data.context_path.clone();

            // 1. 한자 후보 우선 시도 (start_hanja_conversion 트리거)
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
                    let popup_x = abs_x + spot.x as c_int;
                    let popup_y = abs_y + spot.y as c_int + 20;

                    // Standalone 모드에서는 프론트엔드 팝업을 표시하지 않음
                    if Config::load_from_default_path().engine.popup_mode != PopupMode::Standalone {
                        match HanjaWindow::new(self.display, self.screen, popup_x, popup_y) {
                            Ok(mut hw) => {
                                hw.set_candidates(self.display, self.screen, &target, candidates);
                                let popup_wid = hw.window_id();
                                self.hanja_window = Some(hw);
                                self.hanja_context_path = Some(ctx_path);
                                self.hanja_client_window = app_win.map(|w| w.get() as u64);
                                let _ = server.conn().grab_pointer(
                                    false,
                                    popup_wid,
                                    EventMask::BUTTON_PRESS,
                                    GrabMode::ASYNC,
                                    GrabMode::ASYNC,
                                    x11rb::NONE,
                                    x11rb::NONE,
                                    x11rb::CURRENT_TIME,
                                );
                                server.conn().flush().ok();
                            }
                            Err(e) => {
                                unim_log!("XIM_HANDLER", "한자 팝업 생성 실패: {}", e);
                            }
                        }
                    }
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
                top_row,
            }) = special_response
            {
                if !characters.is_empty() {
                    unim_log!(
                        "XIM_HANDLER",
                        "특수문자 후보 표시: target='{}', count={}, top_row='{}'",
                        target,
                        characters.len(),
                        top_row
                    );

                    // 팝업 위치 계산
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
                    let popup_x = abs_x + spot.x as c_int;
                    let popup_y = abs_y + spot.y as c_int + 20;

                    // Standalone 모드에서는 프론트엔드 팝업을 표시하지 않음
                    if Config::load_from_default_path().engine.popup_mode != PopupMode::Standalone {
                        match SpecialWindow::new(self.display, self.screen, popup_x, popup_y) {
                            Ok(mut sw) => {
                                sw.set_characters(
                                    self.display,
                                    self.screen,
                                    &target,
                                    characters,
                                    &top_row,
                                );
                                let popup_wid = sw.window_id();
                                self.special_window = Some(sw);
                                self.special_context_path = Some(ctx_path);
                                self.hanja_client_window = app_win.map(|w| w.get() as u64);
                                let _ = server.conn().grab_pointer(
                                    false,
                                    popup_wid,
                                    EventMask::BUTTON_PRESS,
                                    GrabMode::ASYNC,
                                    GrabMode::ASYNC,
                                    x11rb::NONE,
                                    x11rb::NONE,
                                    x11rb::CURRENT_TIME,
                                );
                                server.conn().flush().ok();
                            }
                            Err(e) => {
                                unim_log!("XIM_HANDLER", "특수문자 팝업 생성 실패: {}", e);
                            }
                        }
                    }
                    return Ok(true);
                }
            }
            unim_log!("XIM_HANDLER", "한자/특수문자 후보 없음");
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
        // 중요: commit 전에 clear_preedit()을 호출하지 않음.
        // clear_preedit()은 XIM PreeditDone을 전송하여 preedit 세션을 종료하는데,
        // 이후 새 preedit 설정 시 PreeditStart→PreeditDraw 시퀀스가 발생하며
        // 일부 XIM 클라이언트(GTK 등)가 이 빠른 전환을 올바르게 처리하지 못함.
        // commit 후 preedit을 in-place로 업데이트하면 PreeditDraw만으로 교체됨.
        if let Some(commit_text) = commit {
            if !commit_text.is_empty() {
                unim_log!("XIM_HANDLER", "커밋: \"{}\"", commit_text);
                server.commit(&user_ic.ic, &commit_text)?;
                server.conn().flush().ok();
            }
        }

        // Preedit 처리
        let preedit_text = preedit.unwrap_or_default();
        if preedit_text.is_empty() {
            self.clear_preedit(server, user_ic)?;
        } else {
            unim_log!("XIM_HANDLER", "Preedit: \"{}\"", preedit_text);
            self.preedit(server, user_ic, &preedit_text)?;
        }
        server.conn().flush().ok();

        Ok(consumed)
    }
}
