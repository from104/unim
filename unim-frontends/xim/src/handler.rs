//! XIM 핸들러 모듈
//!
//! XIM 프로토콜 이벤트를 처리하고 DBus를 통해 UNIM 데몬과 연동합니다.

use std::ffi::CString;
use std::num::NonZeroU32;
use std::os::raw::c_int;
use std::sync::atomic::Ordering;

use ahash::AHashMap;
use tokio::sync::mpsc;
use x11rb::connection::Connection;
use x11rb::protocol::xproto::{ConfigureNotifyEvent, KeyPressEvent};

use unim::config::Config;
use unim::unim_log;

use xim::{
    x11rb::{HasConnection, X11rbServer},
    InputStyle, Server, ServerError, ServerHandler, UserInputContext,
};

use crate::dbus_client::{DbusRequest, DbusResponse};
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
            DbusRequest::CreateContext { client_name, .. } => DbusRequest::CreateContext {
                client_name,
                response: Some(response_tx),
            },
            DbusRequest::FocusOut { context_path, .. } => DbusRequest::FocusOut {
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
    pub fn expose<C: Connection>(
        &mut self,
        window: u32,
        _conn: &C,
    ) -> Result<(), x11rb::errors::ConnectionError> {
        if let Some(win) = NonZeroU32::new(window) {
            if let Some(pe) = self.preedit_windows.get_mut(&win) {
                unim_log!("XIM_HANDLER", "Expose 이벤트: window={}", window);
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
        // 모든 PeWindow 정리
        for (_, pe) in self.preedit_windows.drain() {
            pe.clean(self.display, self.screen);
        }
        // Display 닫기
        unsafe {
            x11::xlib::XCloseDisplay(self.display);
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

        // DBus를 통해 InputContext 생성
        let context_path = match self.send_dbus_request(DbusRequest::CreateContext {
            client_name: "unim-xim".to_string(),
            response: None,
        }) {
            Some(DbusResponse::ContextCreated { path }) => path,
            _ => {
                // DBus 연결 실패 시 로컬 ID 생성
                use std::time::{SystemTime, UNIX_EPOCH};
                let id = SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .map(|d| d.as_nanos() as u32)
                    .unwrap_or(0);
                format!("/local/context_{}", id)
            }
        };

        Ok(UnimInputContext::new(context_path, input_style))
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

        let _ = self.dbus_tx.blocking_send(DbusRequest::FocusIn {
            context_path: user_ic.user_data.context_path.clone(),
        });

        Ok(())
    }

    fn handle_unset_focus(
        &mut self,
        server: &mut X11rbServer<C>,
        user_ic: &mut UserInputContext<Self::InputContextData>,
    ) -> Result<(), ServerError> {
        unim_log!("XIM_HANDLER", "focus_out 호출");

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
        let evdev_code = if xev.detail > 8 { xev.detail - 8 } else { 0 };

        unim_log!(
            "XIM_HANDLER",
            "키 입력: keycode={}, evdev={}, state={:?}",
            xev.detail,
            evdev_code,
            xev.state
        );

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
