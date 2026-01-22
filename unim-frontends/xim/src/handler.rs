//! XIM 핸들러 모듈
//!
//! XIM 프로토콜 이벤트를 처리하고 UNIM 엔진과 연동합니다.

use std::ffi::CString;
use std::num::NonZeroU32;
use std::os::raw::c_int;
use std::sync::atomic::Ordering;

use ahash::AHashMap;
use log::debug;
use x11rb::connection::Connection;
use x11rb::protocol::xproto::{ConfigureNotifyEvent, KeyPressEvent};

use unim::config::Config;
use unim::input_engine::InputEngine;
use unim::keycode::{KeyCode, ModifierState};

use xim::x11rb::X11rbServer;
use xim::{InputStyle, Server, ServerError, ServerHandler, UserInputContext};

use crate::pe_window::PeWindow;

/// 디버그 로깅 매크로 (UNIM_DEVELOP 환경변수 활성화 시)
macro_rules! unim_debug {
    ($($arg:tt)*) => {
        if crate::DEBUG_ENABLED.load(Ordering::Relaxed) {
            println!("[UNIM-XIM] {}", format!($($arg)*));
        }
    };
}

/// 입력 컨텍스트별 상태
pub struct UnimInputContext {
    engine: InputEngine,
    /// preedit 윈도우 ID (Over-The-Spot 스타일일 때만 사용)
    pe_window: Option<NonZeroU32>,
    /// preedit 윈도우를 표시할지 여부
    show_preedit_window: bool,
}

impl UnimInputContext {
    fn new(config: &Config, input_style: InputStyle) -> Self {
        let show_preedit_window = !input_style.contains(InputStyle::PREEDIT_CALLBACKS)
            && !input_style.contains(InputStyle::PREEDIT_NOTHING);

        Self {
            engine: InputEngine::new(config),
            pe_window: None,
            show_preedit_window,
        }
    }
}

/// X11 이벤트 마스크 (KeyPress)
const EVENT_MASK: u32 = 1;

/// UNIM XIM 핸들러
pub struct UnimHandler {
    config: Config,
    /// preedit 윈도우들 (윈도우 ID -> PeWindow)
    preedit_windows: AHashMap<NonZeroU32, PeWindow>,
    /// 스크린 번호
    screen_num: usize,
    /// Xlib Display (단일 연결, 핸들러가 소유)
    display: *mut x11::xlib::Display,
    /// 스크린 번호 (c_int)
    screen: c_int,
}

impl UnimHandler {
    pub fn new(screen_num: usize, config: Config) -> Self {
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
            screen_num,
            display,
            screen: screen_num as c_int,
        }
    }

    /// Expose 이벤트 처리   
    pub fn expose<C: Connection>(
        &mut self,
        window: u32,
        _conn: &C,
    ) -> Result<(), x11rb::errors::ConnectionError> {
        if let Some(win) = NonZeroU32::new(window) {
            if let Some(pe) = self.preedit_windows.get_mut(&win) {
                unim_debug!("Expose 이벤트: window={}", window);
                pe.expose(self.display);
            }
        }
        Ok(())
    }

    /// ConfigureNotify 이벤트 처리
    pub fn configure_notify(&mut self, event: &ConfigureNotifyEvent) {
        if let Some(win) = NonZeroU32::new(event.window) {
            if let Some(pe) = self.preedit_windows.get_mut(&win) {
                unim_debug!(
                    "ConfigureNotify: window={}, width={}, height={}",
                    event.window, event.width, event.height
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
    ) -> Result<(), ServerError> {
        let preedit_str = user_ic.user_data.engine.preedit_str();

        // PREEDIT_CALLBACKS 스타일이면 xim crate의 콜백 사용
        if user_ic.ic.input_style().contains(InputStyle::PREEDIT_CALLBACKS) {
            server.preedit_draw(&mut user_ic.ic, preedit_str)?;
            return Ok(());
        }

        if !user_ic.user_data.show_preedit_window {
            return Ok(());
        }

        if preedit_str.is_empty() {
            return self.clear_preedit(server, user_ic);
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
            unim_debug!("PeWindow 생성: id={}", pe_id);
        }

        Ok(())
    }

    /// Preedit 윈도우 삭제
    fn clear_preedit<C: Connection + xim::x11rb::HasConnection>(
        &mut self,
        server: &mut X11rbServer<C>,
        user_ic: &mut UserInputContext<UnimInputContext>,
    ) -> Result<(), ServerError> {
        if user_ic.ic.input_style().contains(InputStyle::PREEDIT_CALLBACKS) {
            server.preedit_draw(&mut user_ic.ic, "")?;
            return Ok(());
        }

        if let Some(pe_id) = user_ic.user_data.pe_window.take() {
            if let Some(pe) = self.preedit_windows.remove(&pe_id) {
                unim_debug!("PeWindow 삭제: id={}", pe_id);
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
        debug!("새 IC 데이터 생성 (style: {:?})", input_style);
        Ok(UnimInputContext::new(&self.config, input_style))
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
        debug!("XIM 클라이언트 연결됨");
        Ok(())
    }

    fn handle_create_ic(
        &mut self,
        server: &mut X11rbServer<C>,
        user_ic: &mut UserInputContext<Self::InputContextData>,
    ) -> Result<(), ServerError> {
        debug!("IC 생성: id={:?}, style={:?}", user_ic.ic.input_context_id(), user_ic.ic.input_style());
        server.set_event_mask(&user_ic.ic, 1, 0)
    }

    fn handle_destroy_ic(
        &mut self,
        _server: &mut X11rbServer<C>,
        user_ic: UserInputContext<Self::InputContextData>,
    ) -> Result<(), ServerError> {
        debug!("IC 삭제: id={:?}", user_ic.ic.input_context_id());

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
        unim_debug!("reset 호출");

        if !user_ic.user_data.engine.is_composing() {
            return Ok(String::new());
        }

        let preedit = user_ic.user_data.engine.preedit_str().to_string();
        unim_debug!("commit_and_clear: \"{}\"", preedit);

        user_ic.user_data.engine.reset();
        self.clear_preedit(server, user_ic)?;

        Ok(preedit)
    }

    fn handle_set_focus(
        &mut self,
        _server: &mut X11rbServer<C>,
        user_ic: &mut UserInputContext<Self::InputContextData>,
    ) -> Result<(), ServerError> {
        debug!("포커스 인: id={:?}", user_ic.ic.input_context_id());
        Ok(())
    }

    fn handle_unset_focus(
        &mut self,
        server: &mut X11rbServer<C>,
        user_ic: &mut UserInputContext<Self::InputContextData>,
    ) -> Result<(), ServerError> {
        unim_debug!("focus_out 호출");

        if !user_ic.user_data.engine.is_composing() {
            return Ok(());
        }

        let preedit = user_ic.user_data.engine.preedit_str();
        if !preedit.is_empty() {
            unim_debug!("commit_and_clear: \"{}\"", preedit);
            server.commit(&user_ic.ic, preedit)?;
        }

        user_ic.user_data.engine.reset();
        self.clear_preedit(server, user_ic)?;

        Ok(())
    }

    fn handle_set_ic_values(
        &mut self,
        server: &mut X11rbServer<C>,
        user_ic: &mut UserInputContext<Self::InputContextData>,
    ) -> Result<(), ServerError> {
        debug!("IC 값 설정 (spot: {:?})", user_ic.ic.preedit_spot());

        // spot_location 변경 시 preedit 윈도우 재생성
        if user_ic.user_data.pe_window.is_some() && user_ic.user_data.engine.is_composing() {
            self.clear_preedit(server, user_ic)?;
            self.preedit(server, user_ic)?;
        }

        Ok(())
    }

    fn handle_forward_event(
        &mut self,
        server: &mut X11rbServer<C>,
        user_ic: &mut UserInputContext<Self::InputContextData>,
        xev: &KeyPressEvent,
    ) -> Result<bool, ServerError> {
        if self.config.reload_if_changed() {
            unim_debug!("설정 파일 변경 감지, 리로드 완료");
            user_ic.user_data.engine.set_hangul_layout(self.config.engine.hangul.layout);
            user_ic.user_data.engine.set_latin_layout(self.config.engine.latin.layout);
        }

        let key = KeyCode::from_x11_keycode(xev.detail as u16);
        let evdev_code = if xev.detail > 8 { xev.detail - 8 } else { 0 };
        let modifier = ModifierState::from_x11_mask(u16::from(xev.state) as u32);

        unim_debug!(
            "키 입력: keyval={}, keycode={}, evdev={}, shift={}, ctrl={}, alt={}",
            xev.detail, xev.detail, evdev_code,
            modifier.shift, modifier.control, modifier.alt
        );

        let result = user_ic.user_data.engine.press_key(key, modifier, &self.config);

        unim_debug!(
            "엔진 결과: consumed={}, preedit_changed={}, commit_changed={}",
            result.consumed, result.preedit_changed, result.commit_changed
        );

        if result.consumed {
            // 중요: commit이 발생하면 이전 preedit을 먼저 종료 (PreeditDone)
            // 그래야 새 조합 시작 시 PreeditStart가 제대로 전송됨
            if result.commit_changed {
                // 1. 먼저 이전 preedit 정리 (PreeditDone 전송)
                unim_debug!("preedit done (commit 전)");
                self.clear_preedit(server, user_ic)?;

                // 2. 그 다음 commit
                let commit = user_ic.user_data.engine.commit_str();
                if !commit.is_empty() {
                    unim_debug!("커밋: \"{}\"", commit);
                    server.commit(&user_ic.ic, commit)?;
                    user_ic.user_data.engine.clear_commit();
                }
            }

            // 3. 새 preedit이 있으면 표시 (PreeditStart + PreeditDraw)
            if result.preedit_changed {
                let preedit = user_ic.user_data.engine.preedit_str();
                if !preedit.is_empty() {
                    unim_debug!("preedit: \"{}\"", preedit);
                    self.preedit(server, user_ic)?;
                } else if !result.commit_changed {
                    // commit 없이 preedit만 비워지는 경우 (예: Escape)
                    unim_debug!("preedit: (empty)");
                    self.clear_preedit(server, user_ic)?;
                }
            }

            Ok(true)
        } else {
            // consumed=false: 키를 앱으로 전달해야 함
            // ibus-hangul 패턴: clear_preedit → commit 순서
            
            if result.commit_changed {
                // 1. 먼저 이전 preedit 정리 (PreeditDone 전송)
                self.clear_preedit(server, user_ic)?;

                // 2. 그 다음 commit
                let commit = user_ic.user_data.engine.commit_str();
                if !commit.is_empty() {
                    unim_debug!("committed-passthrough: \"{}\"", commit);
                    server.commit(&user_ic.ic, commit)?;
                    user_ic.user_data.engine.clear_commit();
                }
            } else if result.preedit_changed {
                // commit 없이 preedit만 변경되는 경우
                self.clear_preedit(server, user_ic)?;
            }
            
            Ok(false)
        }
    }
}
