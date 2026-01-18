//! XIM 핸들러 모듈
//!
//! XIM 프로토콜 이벤트를 처리하고 UNIM 엔진과 연동합니다.

use log::{debug, trace};
use x11rb::connection::Connection;
use x11rb::protocol::xproto::{ConfigureNotifyEvent, KeyPressEvent};

use unim::config::Config;
use unim::input_engine::InputEngine;
use unim::keycode::{KeyCode, ModifierState};

use xim::x11rb::X11rbServer;
use xim::{InputStyle, Server, ServerError, ServerHandler, UserInputContext};

/// 입력 컨텍스트별 상태
pub struct UnimInputContext {
    engine: InputEngine,
}

impl UnimInputContext {
    fn new(config: &Config) -> Self {
        Self {
            engine: InputEngine::new(config),
        }
    }
}

/// X11 이벤트 마스크 (KeyPress)
const EVENT_MASK: u32 = 1; // KeyPressMask

/// UNIM XIM 핸들러
pub struct UnimHandler {
    config: Config,
}

impl UnimHandler {
    pub fn new(_screen_num: usize, config: Config) -> Self {
        Self { config }
    }

    /// Expose 이벤트 처리 (preedit 윈도우 다시 그리기)
    pub fn expose<C: Connection>(
        &mut self,
        _window: u32,
        _conn: &C,
    ) -> Result<(), x11rb::errors::ConnectionError> {
        // TODO: Preedit 윈도우 다시 그리기
        Ok(())
    }

    /// ConfigureNotify 이벤트 처리
    pub fn configure_notify(&mut self, _event: &ConfigureNotifyEvent) {
        // TODO: 윈도우 크기/위치 변경 처리
    }
}

impl<C: Connection + xim::x11rb::HasConnection> ServerHandler<X11rbServer<C>> for UnimHandler {
    type InputStyleArray = [InputStyle; 3];
    type InputContextData = UnimInputContext;

    fn new_ic_data(
        &mut self,
        _server: &mut X11rbServer<C>,
        _input_style: InputStyle,
    ) -> Result<Self::InputContextData, ServerError> {
        debug!("새 IC 데이터 생성");
        Ok(UnimInputContext::new(&self.config))
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
        _server: &mut X11rbServer<C>,
        user_ic: &mut UserInputContext<Self::InputContextData>,
    ) -> Result<(), ServerError> {
        debug!("IC 생성: id={:?}", user_ic.ic.input_context_id());
        Ok(())
    }

    fn handle_destroy_ic(
        &mut self,
        _server: &mut X11rbServer<C>,
        user_ic: UserInputContext<Self::InputContextData>,
    ) -> Result<(), ServerError> {
        debug!("IC 삭제: id={:?}", user_ic.ic.input_context_id());
        Ok(())
    }

    fn handle_reset_ic(
        &mut self,
        server: &mut X11rbServer<C>,
        user_ic: &mut UserInputContext<Self::InputContextData>,
    ) -> Result<String, ServerError> {
        debug!("IC 리셋");
        user_ic.user_data.engine.clear_preedit();
        let commit = user_ic.user_data.engine.commit_str().to_string();
        user_ic.user_data.engine.clear_commit();

        // preedit 지우기
        server.preedit_draw(&mut user_ic.ic, "")?;

        Ok(commit)
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
        debug!("포커스 아웃");

        // 조합 중이면 커밋
        if user_ic.user_data.engine.is_composing() {
            user_ic.user_data.engine.clear_preedit();
            let commit = user_ic.user_data.engine.commit_str();
            if !commit.is_empty() {
                server.commit(&user_ic.ic, commit)?;
                user_ic.user_data.engine.clear_commit();
            }
            // preedit 지우기
            server.preedit_draw(&mut user_ic.ic, "")?;
        }
        Ok(())
    }

    fn handle_set_ic_values(
        &mut self,
        _server: &mut X11rbServer<C>,
        _user_ic: &mut UserInputContext<Self::InputContextData>,
    ) -> Result<(), ServerError> {
        debug!("IC 값 설정");
        Ok(())
    }

    fn handle_forward_event(
        &mut self,
        server: &mut X11rbServer<C>,
        user_ic: &mut UserInputContext<Self::InputContextData>,
        xev: &KeyPressEvent,
    ) -> Result<bool, ServerError> {
        trace!(
            "키 이벤트: keycode={}, state={:?}",
            xev.detail,
            xev.state
        );

        // 설정 파일 변경 체크 (mtime 기반, 매우 가벼움)
        if self.config.reload_if_changed() {
            debug!("설정 파일 변경 감지, 리로드 완료");
            // 엔진에 새 레이아웃 적용
            user_ic.user_data.engine.set_hangul_layout(self.config.engine.hangul.layout);
            user_ic.user_data.engine.set_latin_layout(self.config.engine.latin.layout);
        }

        // X11 키코드를 UNIM KeyCode로 변환 (X11 keycode = evdev + 8)
        let key = KeyCode::from_x11_keycode(xev.detail as u16);

        // 수정자 상태 변환
        let modifier = ModifierState::from_x11_mask(u16::from(xev.state) as u32);

        // 입력 처리
        let result = user_ic.user_data.engine.press_key(key, modifier, &self.config);

        if result.consumed {
            // 커밋 문자열이 있으면 전송
            if result.commit_changed {
                let commit = user_ic.user_data.engine.commit_str();
                if !commit.is_empty() {
                    server.commit(&user_ic.ic, commit)?;
                    user_ic.user_data.engine.clear_commit();
                }
            }

            // Preedit 변경이 있으면 업데이트
            if result.preedit_changed {
                let preedit = user_ic.user_data.engine.preedit_str();
                server.preedit_draw(&mut user_ic.ic, preedit)?;
            }

            Ok(true) // consumed
        } else {
            Ok(false) // forward to client
        }
    }
}
