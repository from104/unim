mod connection;

use alloc::string::String;
use alloc::vec;
use alloc::vec::Vec;
use core::fmt;
use core::num::NonZeroU16;

use xim_parser::{
    CommitData, ErrorCode, ErrorFlag, Feedback, InputStyle, PreeditDrawStatus, Request,
};

pub use self::connection::{
    InputContext, InputMethod, UserInputContext, XimConnection, XimConnections,
};

#[derive(Debug)]
#[non_exhaustive]
pub enum ServerError {
    ClientNotExists,
    ReadProtocol(xim_parser::ReadError),
    XimError(xim_parser::ErrorCode, String),
    InvalidReply,
    Internal(String),
    #[cfg(feature = "std")]
    Other(alloc::boxed::Box<dyn std::error::Error + Send + Sync>),
}

impl From<xim_parser::ReadError> for ServerError {
    fn from(e: xim_parser::ReadError) -> Self {
        ServerError::ReadProtocol(e)
    }
}

impl fmt::Display for ServerError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ServerError::ClientNotExists => write!(f, "Client doesn't exists"),
            ServerError::ReadProtocol(e) => write!(f, "Can't read xim message: {}", e),
            ServerError::XimError(e, d) => {
                write!(f, "Client send error code: {:?}, detail: {}", e, d)
            }
            ServerError::InvalidReply => write!(f, "Invalid reply from client"),
            ServerError::Internal(e) => write!(f, "Internal error: {}", e),
            #[cfg(feature = "std")]
            ServerError::Other(e) => write!(f, "Other error: {}", e),
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for ServerError {}

pub trait ServerHandler<S: Server> {
    type InputStyleArray: AsRef<[InputStyle]>;
    type InputContextData;

    fn new_ic_data(
        &mut self,
        server: &mut S,
        input_style: InputStyle,
    ) -> Result<Self::InputContextData, ServerError>;

    fn input_styles(&self) -> Self::InputStyleArray;
    fn filter_events(&self) -> u32;

    fn handle_connect(&mut self, server: &mut S) -> Result<(), ServerError>;

    fn handle_create_ic(
        &mut self,
        server: &mut S,
        user_ic: &mut UserInputContext<Self::InputContextData>,
    ) -> Result<(), ServerError>;

    fn handle_destroy_ic(
        &mut self,
        server: &mut S,
        user_ic: UserInputContext<Self::InputContextData>,
    ) -> Result<(), ServerError>;
    fn handle_reset_ic(
        &mut self,
        server: &mut S,
        user_ic: &mut UserInputContext<Self::InputContextData>,
    ) -> Result<String, ServerError>;

    fn handle_set_focus(
        &mut self,
        server: &mut S,
        user_ic: &mut UserInputContext<Self::InputContextData>,
    ) -> Result<(), ServerError>;

    fn handle_unset_focus(
        &mut self,
        server: &mut S,
        user_ic: &mut UserInputContext<Self::InputContextData>,
    ) -> Result<(), ServerError>;

    fn handle_set_ic_values(
        &mut self,
        server: &mut S,
        user_ic: &mut UserInputContext<Self::InputContextData>,
    ) -> Result<(), ServerError>;

    /// return `false` when event back to client
    /// if return `true` it consumed and don't back to client
    fn handle_forward_event(
        &mut self,
        server: &mut S,
        user_ic: &mut UserInputContext<Self::InputContextData>,
        xev: &S::XEvent,
    ) -> Result<bool, ServerError>;
}

pub trait Server {
    type XEvent;

    fn error(
        &mut self,
        client_win: u32,
        code: ErrorCode,
        detail: String,
        input_method_id: Option<NonZeroU16>,
        user_ic_id: Option<NonZeroU16>,
    ) -> Result<(), ServerError>;

    fn preedit_draw(&mut self, ic: &mut InputContext, s: &str) -> Result<(), ServerError>;
    /// UNIM patch: preedit 내용만 비우고 `PreeditDone` 은 보내지 않는다
    /// (= PreeditStart/Done 사이클을 유지한다).
    ///
    /// ON-THE-SPOT 클라이언트는 한 키를 처리하다 `Commit` 을 만나면 그 뒤에 온
    /// 메시지를 버린다. 그래서 조합 종료 시의 "비움" 도 `Commit` **앞**에
    /// 보내야 화면에 반영된다. 그런데 이때 `PreeditDone` 까지 앞세우면 일부
    /// 클라이언트가 세션을 닫아 다음 조합이 깨진다. 내용만 지우고 사이클은
    /// 남겨 두는 경로가 따로 필요한 이유다.
    fn preedit_clear_keep_session(&mut self, ic: &mut InputContext) -> Result<(), ServerError>;
    fn commit(&mut self, ic: &InputContext, s: &str) -> Result<(), ServerError>;

    fn set_event_mask(
        &mut self,
        ic: &InputContext,
        forward_event_mask: u32,
        synchronous_event_mask: u32,
    ) -> Result<(), ServerError>;
}

impl<S: ServerCore> Server for S {
    type XEvent = S::XEvent;

    fn error(
        &mut self,
        client_win: u32,
        code: ErrorCode,
        detail: String,
        input_method_id: Option<NonZeroU16>,
        user_ic_id: Option<NonZeroU16>,
    ) -> Result<(), ServerError> {
        let mut flag = ErrorFlag::empty();

        let input_method_id = if let Some(id) = input_method_id {
            flag |= ErrorFlag::INPUT_METHOD_ID_VALID;
            id.get()
        } else {
            0
        };

        let input_context_id = if let Some(id) = user_ic_id {
            flag |= ErrorFlag::INPUT_CONTEXT_ID_VALID;
            id.get()
        } else {
            0
        };

        self.send_req(
            client_win,
            Request::Error {
                input_method_id,
                input_context_id,
                code,
                detail,
                flag,
            },
        )
    }

    fn preedit_draw(&mut self, ic: &mut InputContext, s: &str) -> Result<(), ServerError> {
        // UNIM patch: PreeditStart 응답 대기 중이면 아무것도 보내지 않고 보류한다.
        // 마지막 값만 남기면 되므로 큐가 아니라 덮어쓰기다.
        if ic.preedit_start_pending {
            ic.pending_preedit = Some(s.into());
            return Ok(());
        }

        let preedit_length = s.chars().count();

        if preedit_length == 0 {
            if ic.preedit_started {
                self.send_req(
                    ic.client_win(),
                    Request::PreeditDraw {
                        input_method_id: ic.input_method_id().get(),
                        input_context_id: ic.input_context_id().get(),
                        chg_first: 0,
                        chg_length: ic.prev_preedit_length as _,
                        caret: preedit_length as _,
                        preedit_string: Vec::new(),
                        feedbacks: Vec::new(),
                        status: PreeditDrawStatus::NO_FEEDBACK | PreeditDrawStatus::NO_STRING,
                    },
                )?;
                self.send_req(
                    ic.client_win(),
                    Request::PreeditDone {
                        input_method_id: ic.input_method_id().get(),
                        input_context_id: ic.input_context_id().get(),
                    },
                )?;
                ic.preedit_started = false;
                ic.prev_preedit_length = 0;
                // UNIM patch: 사이클이 끝났으므로 보류분도 버린다.
                ic.pending_preedit = None;
            }
        } else {
            if !ic.preedit_started {
                self.send_req(
                    ic.client_win(),
                    Request::PreeditStart {
                        input_method_id: ic.input_method_id().get(),
                        input_context_id: ic.input_context_id().get(),
                    },
                )?;
                ic.preedit_started = true;

                // UNIM patch: PreeditStart 는 동기 요청이다. PreeditStartReply 가
                // 올 때까지 PreeditDraw 를 보내지 않고 보류한다. 응답 처리는
                // server/connection.rs 의 `Request::PreeditStartReply` 분기.
                ic.preedit_start_pending = true;
                ic.pending_preedit = Some(s.into());
                return Ok(());
            }

            self.send_req(
                ic.client_win(),
                Request::PreeditDraw {
                    input_method_id: ic.input_method_id().get(),
                    input_context_id: ic.input_context_id().get(),
                    chg_first: 0,
                    chg_length: ic.prev_preedit_length as _,
                    caret: preedit_length as _,
                    preedit_string: xim_ctext::utf8_to_compound_text(s),
                    feedbacks: vec![Feedback::UNDERLINE; preedit_length],
                    status: PreeditDrawStatus::empty(),
                },
            )?;

            ic.prev_preedit_length = preedit_length;
        }

        Ok(())
    }

    // UNIM patch: 선언부 주석 참조.
    fn preedit_clear_keep_session(&mut self, ic: &mut InputContext) -> Result<(), ServerError> {
        if ic.preedit_start_pending {
            // 시작 응답 대기 중이면 보류분만 비운다.
            ic.pending_preedit = Some(alloc::string::String::new());
            return Ok(());
        }
        if !ic.preedit_started {
            return Ok(());
        }

        self.send_req(
            ic.client_win(),
            Request::PreeditDraw {
                input_method_id: ic.input_method_id().get(),
                input_context_id: ic.input_context_id().get(),
                chg_first: 0,
                chg_length: ic.prev_preedit_length as _,
                caret: 0,
                preedit_string: Vec::new(),
                feedbacks: Vec::new(),
                status: PreeditDrawStatus::NO_FEEDBACK | PreeditDrawStatus::NO_STRING,
            },
        )?;
        ic.prev_preedit_length = 0;
        Ok(())
    }

    fn commit(&mut self, ic: &InputContext, s: &str) -> Result<(), ServerError> {
        self.send_req(
            ic.client_win(),
            Request::Commit {
                input_method_id: ic.input_method_id().get(),
                input_context_id: ic.input_context_id().get(),
                data: CommitData::Chars {
                    commited: xim_ctext::utf8_to_compound_text(s),
                    syncronous: false,
                },
            },
        )
    }

    fn set_event_mask(
        &mut self,
        ic: &InputContext,
        forward_event_mask: u32,
        synchronous_event_mask: u32,
    ) -> Result<(), ServerError> {
        self.send_req(
            ic.client_win(),
            Request::SetEventMask {
                input_method_id: ic.input_method_id().get(),
                input_context_id: ic.input_context_id().get(),
                forward_event_mask,
                synchronous_event_mask,
            },
        )
    }
}

pub trait ServerCore {
    type XEvent;

    fn deserialize_event(&self, ev: &xim_parser::XEvent) -> Self::XEvent;
    fn send_req(&mut self, client_win: u32, req: Request) -> Result<(), ServerError>;
}
