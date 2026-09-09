//! Provides an implementation of XIM using [`x11rb`] as a transport.
//!
//! Wrap your `Connection` in an [`X11rbClient`] or [`X11rbServer`] and use it as a
//! client or server.
//!
//! [`x11rb`]: https://crates.io/crates/x11rb

use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;
use std::{convert::TryInto, eprintln, rc::Rc, sync::Arc, sync::Mutex, sync::OnceLock};
use x11rb::protocol::xproto::EventMask;

#[cfg(feature = "x11rb-client")]
use crate::client::{
    handle_request as client_handle_request, ClientCore, ClientError, ClientHandler,
};
#[cfg(feature = "x11rb-server")]
use crate::server::{ServerCore, ServerError, ServerHandler, XimConnection, XimConnections};
#[cfg(feature = "x11rb-client")]
use crate::AHashMap;
#[cfg(feature = "x11rb-client")]
use xim_parser::{Attr, AttributeName};

use crate::Atoms;

#[cfg(feature = "x11rb-xcb")]
use x11rb::xcb_ffi::XCBConnection;

#[allow(unused_imports)]
use x11rb::{
    connection::Connection,
    errors::{ConnectError, ConnectionError, ParseError, ReplyError, ReplyOrIdError},
    protocol::{
        xproto::{
            Atom, AtomEnum, ClientMessageEvent, ConnectionExt, KeyPressEvent, PropMode, Screen,
            SelectionNotifyEvent, SelectionRequestEvent, Window, WindowClass, CLIENT_MESSAGE_EVENT,
            SELECTION_NOTIFY_EVENT,
        },
        Event,
    },
    rust_connection::RustConnection,
    wrapper::ConnectionExt as _,
    COPY_DEPTH_FROM_PARENT, CURRENT_TIME,
};

use xim_parser::{Request, XimWrite};

macro_rules! convert_error {
    ($($ty:ty,)+) => {
        $(
            #[cfg(feature = "x11rb-client")]
            impl From<$ty> for ClientError {
                fn from(err: $ty) -> Self {
                    ClientError::Other(err.into())
                }
            }

            #[cfg(feature = "x11rb-server")]
            impl From<$ty> for ServerError {
                fn from(err: $ty) -> Self {
                    ServerError::Other(err.into())
                }
            }
        )+
    };
}

convert_error!(
    ConnectError,
    ConnectionError,
    ReplyError,
    ReplyOrIdError,
    ParseError,
);

pub trait HasConnection {
    type Connection: Connection + ConnectionExt;

    fn conn(&self) -> &Self::Connection;
}

#[cfg(feature = "x11rb-xcb")]
impl HasConnection for XCBConnection {
    type Connection = Self;

    #[inline(always)]
    fn conn(&self) -> &Self::Connection {
        self
    }
}

impl HasConnection for RustConnection {
    type Connection = Self;

    #[inline(always)]
    fn conn(&self) -> &Self::Connection {
        self
    }
}

#[cfg(feature = "x11rb-client")]
impl<C: HasConnection> HasConnection for X11rbClient<C> {
    type Connection = C::Connection;

    #[inline(always)]
    fn conn(&self) -> &Self::Connection {
        self.has_conn.conn()
    }
}

#[cfg(feature = "x11rb-server")]
impl<C: HasConnection> HasConnection for X11rbServer<C> {
    type Connection = C::Connection;

    #[inline(always)]
    fn conn(&self) -> &Self::Connection {
        self.has_conn.conn()
    }
}

impl<C: HasConnection> HasConnection for &C {
    type Connection = C::Connection;

    #[inline(always)]
    fn conn(&self) -> &Self::Connection {
        (**self).conn()
    }
}

impl<C: HasConnection> HasConnection for Rc<C> {
    type Connection = C::Connection;

    #[inline(always)]
    fn conn(&self) -> &Self::Connection {
        (**self).conn()
    }
}

impl<C: HasConnection> HasConnection for Arc<C> {
    type Connection = C::Connection;

    #[inline(always)]
    fn conn(&self) -> &Self::Connection {
        (**self).conn()
    }
}

#[cfg(feature = "x11rb-server")]
pub struct X11rbServer<C: HasConnection> {
    has_conn: C,
    locale_data: String,
    im_win: Window,
    atoms: Atoms<Atom>,
    buf: Vec<u8>,
    sequence: u16,
}

#[cfg(feature = "x11rb-server")]
impl<C: HasConnection> X11rbServer<C> {
    pub fn init(
        has_conn: C,
        screen_num: usize,
        im_name: &str,
        locales: &str,
    ) -> Result<Self, ServerError> {
        let im_name = format!("@server={}", im_name);
        let conn = has_conn.conn();
        let screen = &conn.setup().roots[screen_num];
        let im_win = conn.generate_id()?;
        conn.create_window(
            COPY_DEPTH_FROM_PARENT,
            im_win,
            screen.root,
            0,
            0,
            1,
            1,
            0,
            WindowClass::INPUT_ONLY,
            screen.root_visual,
            &Default::default(),
        )?;
        let atoms = Atoms::new::<ServerError, _>(|name| {
            Ok(conn.intern_atom(false, name.as_bytes())?.reply()?.atom)
        })?;

        let reply = conn
            .get_property(
                false,
                screen.root,
                atoms.XIM_SERVERS,
                AtomEnum::ATOM,
                0,
                u32::MAX,
            )?
            .reply()?;

        if reply.type_ != x11rb::NONE && (reply.type_ != u32::from(AtomEnum::ATOM)) {
            return Err(ServerError::InvalidReply);
        }

        let server_name = conn.intern_atom(false, im_name.as_bytes())?.reply()?.atom;

        let mut found = false;

        if reply.type_ != x11rb::NONE {
            for prop in reply.value32().ok_or(ServerError::InvalidReply)? {
                if prop == server_name {
                    log::info!("Found previous XIM_SERVER it will overrided");
                    found = true;
                }
            }
        }

        // override owner
        conn.set_selection_owner(im_win, server_name, x11rb::CURRENT_TIME)?;

        if !found {
            conn.change_property32(
                PropMode::PREPEND,
                screen.root,
                atoms.XIM_SERVERS,
                AtomEnum::ATOM,
                &[server_name],
            )?;
        }

        conn.flush()?;

        log::info!("Start server win: {}", im_win);

        Ok(Self {
            has_conn,
            locale_data: format!("@locale={}", locales),
            im_win,
            atoms,
            buf: Vec::with_capacity(1024),
            sequence: 0,
        })
    }

    pub fn filter_event<T>(
        &mut self,
        e: &Event,
        connections: &mut XimConnections<T>,
        handler: &mut impl ServerHandler<Self, InputContextData = T>,
    ) -> Result<bool, ServerError> {
        match e {
            Event::SelectionRequest(req) if req.owner == self.im_win => {
                if req.property == self.atoms.LOCALES {
                    log::trace!("Selection notify locale");
                    self.send_selection_notify(req, &self.locale_data)?;
                } else if req.property == self.atoms.TRANSPORT {
                    log::trace!("Selection notify transport");
                    self.send_selection_notify(req, "@transport=X/")?;
                }
                Ok(true)
            }
            Event::ClientMessage(msg) => {
                if msg.type_ == self.atoms.XIM_XCONNECT {
                    let com_win = self.conn().generate_id()?;
                    self.conn().create_window(
                        COPY_DEPTH_FROM_PARENT,
                        com_win,
                        self.im_win,
                        0,
                        0,
                        1,
                        1,
                        0,
                        WindowClass::INPUT_ONLY,
                        0,
                        &Default::default(),
                    )?;
                    let client_win = msg.data.as_data32()[0];
                    log::info!("XConnected with {}", client_win);
                    self.conn().send_event(
                        false,
                        client_win,
                        EventMask::NO_EVENT,
                        ClientMessageEvent {
                            format: 32,
                            type_: self.atoms.XIM_XCONNECT,
                            data: [com_win, 0, 0, 0, 0].into(),
                            response_type: CLIENT_MESSAGE_EVENT,
                            sequence: 0,
                            window: client_win,
                        },
                    )?;
                    self.conn().flush()?;
                    connections.new_connection(com_win, client_win);
                } else if msg.type_ == self.atoms.XIM_PROTOCOL {
                    if let Some(connection) = connections.get_connection(msg.window) {
                        self.handle_xim_protocol(msg, connection, handler)?;
                        if connection.disconnected {
                            connections.remove_connection(msg.window);
                        }
                    } else {
                        log::warn!("Unknown connection");
                    }
                }

                Ok(true)
            }
            _ => Ok(false),
        }
    }

    // UNIM trace: Phase 0 계측 (이슈 C). 이 함수는 UNIM 이 XIM 서버로서 GTK
    // 클라이언트(im-xim)의 요청을 실제로 수신·역직렬화하는, 워크스페이스에서
    // 실행되는 유일한 handle_xim_protocol 이다(X11rbClient 쪽은 unim-xim 크레이트가
    // `x11rb-client` 피처를 켜지 않아 빌드되지 않는다). ②③ 항목은 여기서 찍는다.
    fn handle_xim_protocol<T>(
        &mut self,
        msg: &ClientMessageEvent,
        connection: &mut XimConnection<T>,
        handler: &mut impl ServerHandler<Self, InputContextData = T>,
    ) -> Result<(), ServerError> {
        let trace_on = xim_trace_enabled();
        // UNIM trace: ④ 클라이언트가 뭔가를 보내왔다 — 정지·타임아웃 뒤의 첫 수신
        // 시점이 직전 property 소비 여부를 묻기에 가장 뜻있는 순간이다.
        xim_trace_probe_prev_prop(&*self, "recv-from-client");
        if msg.format == 32 {
            let [length, atom, ..] = msg.data.as_data32();
            let data = self
                .conn()
                .get_property(true, msg.window, atom, AtomEnum::ANY, 0, length)?
                .reply()?
                .value;
            if trace_on {
                let opcode = data.first().copied().unwrap_or(0);
                eprintln!(
                    "[unim-xim-trace] recv role=server<-client route=property(>20B) window=0x{:x} atom={atom} requested_len={length} data.len={} opcode={}",
                    msg.window,
                    data.len(),
                    xim_opcode_name(opcode),
                );
            }
            let req: xim_parser::Request = xim_parser::read(&data)?;
            if trace_on {
                // UNIM trace: ③ 여기까지 왔으면 GetProperty + xim_parser::read 가
                // 모두 성공한 것이므로, PREEDIT_DRAW 뿐 아니라 FORWARD_EVENT·COMMIT
                // 도 이 지점 도달 여부로 "property 경로 완주" 를 opcode 별로 구분해
                // 확인할 수 있다.
                eprintln!(
                    "[unim-xim-trace] recv route=property(>20B) COMPLETE req={} window=0x{:x}",
                    req.name(),
                    msg.window,
                );
                if let xim_parser::Request::Error { .. } = &req {
                    // UNIM trace: ② XIM_ERROR 수신 — 전문 로깅.
                    eprintln!(
                        "[unim-xim-trace] recv XIM_ERROR route=property(>20B) window=0x{:x} full={:?}",
                        msg.window, req,
                    );
                }
            }
            connection.handle_request(self, req, handler)
        } else {
            let raw = msg.data.as_data8();
            if trace_on {
                let opcode = raw.first().copied().unwrap_or(0);
                eprintln!(
                    "[unim-xim-trace] recv role=server<-client route=direct-clientmessage(<=20B) window=0x{:x} data.len={} opcode={}",
                    msg.window,
                    raw.len(),
                    xim_opcode_name(opcode),
                );
            }
            let req: xim_parser::Request = xim_parser::read(&raw)?;
            if trace_on {
                eprintln!(
                    "[unim-xim-trace] recv route=direct-clientmessage(<=20B) COMPLETE req={} window=0x{:x}",
                    req.name(),
                    msg.window,
                );
                if let xim_parser::Request::Error { .. } = &req {
                    // UNIM trace: ② XIM_ERROR 수신 — 전문 로깅.
                    eprintln!(
                        "[unim-xim-trace] recv XIM_ERROR route=direct-clientmessage(<=20B) window=0x{:x} full={:?}",
                        msg.window, req,
                    );
                }
            }
            connection.handle_request(self, req, handler)
        }
    }

    fn send_selection_notify(
        &self,
        req: &SelectionRequestEvent,
        data: &str,
    ) -> Result<(), ServerError> {
        let e = SelectionNotifyEvent {
            response_type: SELECTION_NOTIFY_EVENT,
            property: req.property,
            time: req.time,
            target: req.target,
            selection: req.selection,
            requestor: req.requestor,
            sequence: 0,
        };

        self.conn().change_property8(
            PropMode::REPLACE,
            req.requestor,
            req.property,
            req.target,
            data.as_bytes(),
        )?;
        self.conn()
            .send_event(false, req.requestor, EventMask::NO_EVENT, e)?;
        self.conn().flush()?;

        Ok(())
    }
}

#[cfg(feature = "x11rb-server")]
impl<C: HasConnection> ServerCore for X11rbServer<C> {
    type XEvent = KeyPressEvent;

    fn send_req(&mut self, client_win: u32, req: Request) -> Result<(), ServerError> {
        send_req_impl(
            &self.has_conn,
            &self.atoms,
            client_win,
            &mut self.buf,
            &mut self.sequence,
            20,
            &req,
            "server->client", // UNIM trace
        )
    }

    #[inline]
    fn deserialize_event(&self, ev: &xim_parser::XEvent) -> Self::XEvent {
        deserialize_event_impl(ev)
    }
}

#[cfg(feature = "x11rb-client")]
pub struct X11rbClient<C: HasConnection> {
    has_conn: C,
    server_owner_window: Window,
    im_window: Window,
    server_atom: Atom,
    atoms: Atoms<Atom>,
    transport_max: usize,
    client_window: u32,
    im_attributes: AHashMap<AttributeName, u16>,
    ic_attributes: AHashMap<AttributeName, u16>,
    sequence: u16,
    buf: Vec<u8>,
}

#[cfg(feature = "x11rb-client")]
impl<C: HasConnection> X11rbClient<C> {
    pub fn init(
        has_conn: C,
        screen_num: usize,
        im_name: Option<&str>,
    ) -> Result<Self, ClientError> {
        let conn = has_conn.conn();
        let screen = &conn.setup().roots[screen_num];
        let client_window = conn.generate_id()?;

        conn.create_window(
            COPY_DEPTH_FROM_PARENT,
            client_window,
            screen.root,
            0,
            0,
            1,
            1,
            0,
            WindowClass::INPUT_ONLY,
            screen.root_visual,
            &Default::default(),
        )?;

        let var = std::env::var("XMODIFIERS").ok();
        let var = var.as_ref().and_then(|n| n.strip_prefix("@im="));
        let im_name = im_name.or(var).ok_or(ClientError::NoXimServer)?;

        log::info!("Try connect {}", im_name);

        let atoms = Atoms::new::<ClientError, _>(|name| {
            Ok(conn.intern_atom(false, name.as_bytes())?.reply()?.atom)
        })?;
        let server_reply = conn
            .get_property(
                false,
                screen.root,
                atoms.XIM_SERVERS,
                AtomEnum::ATOM,
                0,
                u32::MAX,
            )?
            .reply()?;

        if server_reply.type_ != u32::from(AtomEnum::ATOM) || server_reply.format != 32 {
            Err(ClientError::InvalidReply)
        } else {
            for server_atom in server_reply.value32().ok_or(ClientError::InvalidReply)? {
                let server_owner = conn.get_selection_owner(server_atom)?.reply()?.owner;
                let name = conn.get_atom_name(server_atom)?.reply()?.name;

                let name = match String::from_utf8(name) {
                    Ok(name) => name,
                    _ => continue,
                };

                if let Some(name) = name.strip_prefix("@server=") {
                    if name == im_name {
                        conn.convert_selection(
                            client_window,
                            server_atom,
                            atoms.TRANSPORT,
                            atoms.TRANSPORT,
                            CURRENT_TIME,
                        )?;

                        conn.flush()?;

                        return Ok(Self {
                            has_conn,
                            atoms,
                            server_atom,
                            server_owner_window: server_owner,
                            im_attributes: AHashMap::with_hasher(Default::default()),
                            ic_attributes: AHashMap::with_hasher(Default::default()),
                            im_window: x11rb::NONE,
                            transport_max: 20,
                            client_window,
                            sequence: 0,
                            buf: Vec::with_capacity(1024),
                        });
                    }
                }
            }

            Err(ClientError::NoXimServer)
        }
    }

    pub fn filter_event(
        &mut self,
        e: &Event,
        handler: &mut impl ClientHandler<Self>,
    ) -> Result<bool, ClientError> {
        match e {
            Event::SelectionNotify(e) if e.requestor == self.client_window => {
                if e.property == self.atoms.LOCALES {
                    // TODO: set locale
                    let _locale = self
                        .conn()
                        .get_property(
                            true,
                            self.client_window,
                            self.atoms.LOCALES,
                            self.atoms.LOCALES,
                            0,
                            u32::MAX,
                        )?
                        .reply()?;

                    self.xconnect()?;

                    Ok(true)
                } else if e.property == self.atoms.TRANSPORT {
                    let transport = self
                        .conn()
                        .get_property(
                            true,
                            self.client_window,
                            self.atoms.TRANSPORT,
                            self.atoms.TRANSPORT,
                            0,
                            u32::MAX,
                        )?
                        .reply()?;

                    if !transport.value.starts_with(b"@transport=X/") {
                        return Err(ClientError::UnsupportedTransport);
                    }

                    self.conn().convert_selection(
                        self.client_window,
                        self.server_atom,
                        self.atoms.LOCALES,
                        self.atoms.LOCALES,
                        CURRENT_TIME,
                    )?;

                    self.conn().flush()?;

                    Ok(true)
                } else {
                    Ok(false)
                }
            }
            Event::ClientMessage(msg) if msg.window == self.client_window => {
                if msg.type_ == self.atoms.XIM_XCONNECT {
                    let [im_window, major, minor, max, _] = msg.data.as_data32();
                    log::info!(
                        "XConnected server on {}, transport version: {}.{}, TRANSPORT_MAX: {}",
                        im_window,
                        major,
                        minor,
                        max
                    );
                    self.im_window = im_window;
                    self.transport_max = max as usize;
                    self.send_req(Request::Connect {
                        client_major_protocol_version: 1,
                        client_minor_protocol_version: 0,
                        endian: xim_parser::Endian::Native,
                        client_auth_protocol_names: Vec::new(),
                    })?;
                    Ok(true)
                } else if msg.type_ == self.atoms.XIM_PROTOCOL {
                    self.handle_xim_protocol(msg, handler)?;
                    Ok(true)
                } else {
                    Ok(false)
                }
            }
            _ => Ok(false),
        }
    }

    // UNIM trace: Phase 0 계측 (이슈 C). 참고 — unim-xim 크레이트는 `x11rb-client`
    // 피처를 켜지 않으므로(이 크레이트의 `examples/x11rb_client.rs` 전용) 이 함수는
    // 실제 UNIM 데몬 빌드에는 포함되지 않는다. GTK 는 자체 C 구현 xim immodule 을
    // 쓰므로 이 크레이트의 클라이언트 경로를 타지 않는다 — 실서비스 계측은
    // X11rbServer::handle_xim_protocol 쪽(위)이 담당한다. 여기는 지시된 라인 범위에
    // 맞춰 동일 패턴으로만 남겨 둔다(다른 x11rb-client 소비자가 생기면 바로 쓸 수 있게).
    fn handle_xim_protocol(
        &mut self,
        msg: &ClientMessageEvent,
        handler: &mut impl ClientHandler<Self>,
    ) -> Result<(), ClientError> {
        let trace_on = xim_trace_enabled();
        if msg.format == 32 {
            let [length, atom, ..] = msg.data.as_data32();
            let reply = self
                .conn()
                .get_property(true, msg.window, atom, AtomEnum::ANY, 0, length)?
                .reply()?;
            // handle fcitx4 occasionally sending empty reply
            if reply.value_len == 0 {
                if trace_on {
                    eprintln!(
                        "[unim-xim-trace] recv role=client<-server route=property(>20B) FAILED(empty-reply) window=0x{:x} atom={atom} requested_len={length}",
                        msg.window,
                    );
                }
                return Err(ClientError::InvalidReply);
            }
            let data = reply.value;
            if trace_on {
                let opcode = data.first().copied().unwrap_or(0);
                eprintln!(
                    "[unim-xim-trace] recv role=client<-server route=property(>20B) window=0x{:x} atom={atom} requested_len={length} data.len={} opcode={}",
                    msg.window,
                    data.len(),
                    xim_opcode_name(opcode),
                );
            }
            let req: xim_parser::Request = xim_parser::read(&data)?;
            if trace_on {
                // UNIM trace: ③ PREEDIT_DRAW 뿐 아니라 FORWARD_EVENT·COMMIT 도 여기
                // 도달 여부로 property 경로 완주를 opcode 별로 구분해 확인한다.
                eprintln!(
                    "[unim-xim-trace] recv route=property(>20B) COMPLETE req={} window=0x{:x}",
                    req.name(),
                    msg.window,
                );
                if let xim_parser::Request::Error { .. } = &req {
                    // UNIM trace: ② XIM_ERROR 수신 — 전문 로깅.
                    eprintln!(
                        "[unim-xim-trace] recv XIM_ERROR route=property(>20B) window=0x{:x} full={:?}",
                        msg.window, req,
                    );
                }
            }
            client_handle_request(self, handler, req)?;
        } else if msg.format == 8 {
            let data = msg.data.as_data8();
            if trace_on {
                let opcode = data.first().copied().unwrap_or(0);
                eprintln!(
                    "[unim-xim-trace] recv role=client<-server route=direct-clientmessage(<=20B) window=0x{:x} data.len={} opcode={}",
                    msg.window,
                    data.len(),
                    xim_opcode_name(opcode),
                );
            }
            let req: xim_parser::Request = xim_parser::read(&data)?;
            if trace_on {
                eprintln!(
                    "[unim-xim-trace] recv route=direct-clientmessage(<=20B) COMPLETE req={} window=0x{:x}",
                    req.name(),
                    msg.window,
                );
                if let xim_parser::Request::Error { .. } = &req {
                    // UNIM trace: ② XIM_ERROR 수신 — 전문 로깅.
                    eprintln!(
                        "[unim-xim-trace] recv XIM_ERROR route=direct-clientmessage(<=20B) window=0x{:x} full={:?}",
                        msg.window, req,
                    );
                }
            }
            client_handle_request(self, handler, req)?;
        }

        Ok(())
    }

    fn xconnect(&mut self) -> Result<(), ClientError> {
        self.conn().send_event(
            false,
            self.server_owner_window,
            EventMask::NO_EVENT,
            ClientMessageEvent {
                data: [self.client_window, 0, 0, 0, 0].into(),
                format: 32,
                response_type: CLIENT_MESSAGE_EVENT,
                sequence: 0,
                type_: self.atoms.XIM_XCONNECT,
                window: self.server_owner_window,
            },
        )?;

        self.conn().flush()?;

        Ok(())
    }
}

#[cfg(feature = "x11rb-client")]
impl<C: HasConnection> ClientCore for X11rbClient<C> {
    type XEvent = KeyPressEvent;
    fn set_attrs(&mut self, im_attrs: Vec<Attr>, ic_attrs: Vec<Attr>) {
        for im_attr in im_attrs {
            self.im_attributes.insert(im_attr.name, im_attr.id);
        }

        for ic_attr in ic_attrs {
            self.ic_attributes.insert(ic_attr.name, ic_attr.id);
        }
    }

    #[inline]
    fn ic_attributes(&self) -> &AHashMap<AttributeName, u16> {
        &self.ic_attributes
    }

    #[inline]
    fn im_attributes(&self) -> &AHashMap<AttributeName, u16> {
        &self.im_attributes
    }

    #[inline]
    fn serialize_event(&self, xev: &Self::XEvent) -> xim_parser::XEvent {
        xim_parser::XEvent {
            response_type: xev.response_type,
            detail: xev.detail,
            sequence: xev.sequence,
            time: xev.time,
            root: xev.root,
            event: xev.event,
            child: xev.child,
            root_x: xev.root_x,
            root_y: xev.root_y,
            event_x: xev.event_x,
            event_y: xev.event_y,
            state: xev.state.into(),
            same_screen: xev.same_screen,
        }
    }

    #[inline]
    fn deserialize_event(&self, xev: &xim_parser::XEvent) -> Self::XEvent {
        deserialize_event_impl(xev)
    }

    #[inline]
    fn send_req(&mut self, req: Request) -> Result<(), ClientError> {
        send_req_impl(
            &self.has_conn,
            &self.atoms,
            self.im_window,
            &mut self.buf,
            &mut self.sequence,
            self.transport_max,
            &req,
            "client->server", // UNIM trace
        )
    }
}

// UNIM trace: Phase 0 계측 (이슈 C — GTK im-xim 조합 정지 확진). 자세한 내용은
// third_party/xim/UNIM-FORK.md 참고. `UNIM_XIM_TRACE` 환경변수가 설정된 경우에만
// eprintln! 로 출력하고, 기본은 완전 무음이다. 로깅 전용 — 동작은 절대 바꾸지 않는다.
#[inline]
fn xim_trace_enabled() -> bool {
    static ENABLED: OnceLock<bool> = OnceLock::new();
    *ENABLED.get_or_init(|| std::env::var_os("UNIM_XIM_TRACE").is_some())
}

// UNIM trace: ④ 직전 property 송신의 소비 프로브. 클라이언트(im-xim)는 property 를
// delete=True 로 읽어가므로, 이후 시점에 같은 atom 을 delete=False 로 되물었을 때
// 데이터가 남아 있으면 "클라이언트가 GetProperty 자체를 안 했다"(정지가 읽기 이전),
// 비어 있으면 "읽고도 멎었다"(정지가 파싱/처리 이후)로 갈린다 — xtrace 없이 서버
// 안에서 얻는 이슈 C 의 1차 판별자. 계측 전용, 프로토콜 동작 무변경.
static XIM_TRACE_LAST_PROP: Mutex<Option<(u32, u32)>> = Mutex::new(None);

fn xim_trace_probe_prev_prop<C: HasConnection>(c: &C, at: &str) {
    if !xim_trace_enabled() {
        return;
    }
    let Some((win, atom)) = XIM_TRACE_LAST_PROP.lock().unwrap().take() else {
        return;
    };
    let reply = c
        .conn()
        .get_property(false, win, atom, AtomEnum::ANY, 0, 4)
        .map_err(|e| format!("{e:?}"))
        .and_then(|cookie| cookie.reply().map_err(|e| format!("{e:?}")));
    match reply {
        Ok(r) => eprintln!(
            "[unim-xim-trace] probe at={at} window=0x{win:x} atom={atom} consumed={} value.len={} bytes_after={}",
            r.value.is_empty() && r.bytes_after == 0,
            r.value.len(),
            r.bytes_after,
        ),
        Err(e) => eprintln!(
            "[unim-xim-trace] probe at={at} window=0x{win:x} atom={atom} FAILED: {e}"
        ),
    }
}

// UNIM trace: 와이어 버퍼 선두 바이트(메이저 opcode)를 XIM 프로토콜 이름으로
// 변환한다. 계측 전용 — xim-parser 0.2.2 `Request::read` 의 opcode 표와 동일하게
// 맞춰 두었다(상류 표가 바뀌면 여기도 갱신 필요).
fn xim_opcode_name(opcode: u8) -> String {
    match opcode {
        1 => "XIM_CONNECT",
        2 => "XIM_CONNECT_REPLY",
        3 => "XIM_DISCONNECT",
        4 => "XIM_DISCONNECT_REPLY",
        10 => "XIM_AUTH_REQUIRED",
        11 => "XIM_AUTH_REPLY",
        12 => "XIM_AUTH_NEXT",
        13 => "XIM_AUTH_SETUP",
        14 => "XIM_AUTH_NG",
        20 => "XIM_ERROR",
        30 => "XIM_OPEN",
        31 => "XIM_OPEN_REPLY",
        32 => "XIM_CLOSE",
        33 => "XIM_CLOSE_REPLY",
        34 => "XIM_REGISTER_TRIGGERKEYS",
        35 => "XIM_TRIGGER_NOTIFY",
        36 => "XIM_TRIGGER_NOTIFY_REPLY",
        37 => "XIM_SET_EVENT_MASK",
        38 => "XIM_ENCODING_NEGOTIATION",
        39 => "XIM_ENCODING_NEGOTIATION_REPLY",
        40 => "XIM_QUERY_EXTENSION",
        41 => "XIM_QUERY_EXTENSION_REPLY",
        42 => "XIM_SET_IM_VALUES",
        43 => "XIM_SET_IM_VALUES_REPLY",
        44 => "XIM_GET_IM_VALUES",
        45 => "XIM_GET_IM_VALUES_REPLY",
        50 => "XIM_CREATE_IC",
        51 => "XIM_CREATE_IC_REPLY",
        52 => "XIM_DESTROY_IC",
        53 => "XIM_DESTROY_IC_REPLY",
        54 => "XIM_SET_IC_VALUES",
        55 => "XIM_SET_IC_VALUES_REPLY",
        56 => "XIM_GET_IC_VALUES",
        57 => "XIM_GET_IC_VALUES_REPLY",
        58 => "XIM_SET_IC_FOCUS",
        59 => "XIM_UNSET_IC_FOCUS",
        60 => "XIM_FORWARD_EVENT",
        61 => "XIM_SYNC",
        62 => "XIM_SYNC_REPLY",
        63 => "XIM_COMMIT",
        64 => "XIM_RESET_IC",
        65 => "XIM_RESET_IC_REPLY",
        70 => "XIM_GEOMETRY",
        71 => "XIM_STR_CONVERSION",
        72 => "XIM_STR_CONVERSION_REPLY",
        73 => "XIM_PREEDIT_START",
        74 => "XIM_PREEDIT_START_REPLY",
        75 => "XIM_PREEDIT_DRAW",
        76 => "XIM_PREEDIT_CARET",
        77 => "XIM_PREEDIT_CARET_REPLY",
        78 => "XIM_PREEDIT_DONE",
        79 => "XIM_STATUS_START",
        80 => "XIM_STATUS_DRAW",
        81 => "XIM_STATUS_DONE",
        82 => "XIM_PREEDIT_STATE",
        other => return format!("UNKNOWN({other})"),
    }
    .into()
}

fn send_req_impl<C: HasConnection, E: From<ConnectionError> + From<ReplyError>>(
    c: &C,
    atoms: &Atoms<Atom>,
    target: Window,
    buf: &mut Vec<u8>,
    sequence: &mut u16,
    transport_max: usize,
    req: &Request,
    // UNIM trace: 호출부(서버/클라이언트) 구분용 라벨. 계측 로그 전용, 프로토콜에는 무영향.
    role: &'static str,
) -> Result<(), E> {
    if log::log_enabled!(log::Level::Trace) {
        log::trace!("->: {:?}", req);
    } else {
        log::debug!("->: {}", req.name());
    }
    buf.resize(req.size(), 0);
    xim_parser::write(req, buf);

    // UNIM trace: ① 분기 판정 전 와이어 opcode를 buf 선두 바이트에서 직접 뽑는다
    // (req.name() 이 아니라 실제 시리얼라이즈드 바이트 기준 — 파서 라벨과 와이어가
    // 어긋나는지도 같이 검증하기 위함).
    let trace_on = xim_trace_enabled();
    let trace_opcode = buf.first().copied().unwrap_or(0);
    // UNIM trace: ④ 새 송신에 앞서 직전 property 송신의 소비 여부를 되묻는다.
    xim_trace_probe_prev_prop(c, "before-send");

    if buf.len() < transport_max {
        if buf.len() > 20 {
            todo!("multi-CM");
        }
        if trace_on {
            eprintln!(
                "[unim-xim-trace] send role={role} route=direct-clientmessage(<=20B) opcode={} req={} target=0x{target:x} buf.len={} transport_max={transport_max} atom=none PropMode=none",
                xim_opcode_name(trace_opcode),
                req.name(),
                buf.len(),
            );
        }
        buf.resize(20, 0);
        let buf: [u8; 20] = buf.as_slice().try_into().unwrap();
        c.conn().send_event(
            false,
            target,
            EventMask::NO_EVENT,
            ClientMessageEvent {
                response_type: CLIENT_MESSAGE_EVENT,
                data: buf.into(),
                format: 8,
                sequence: 0,
                type_: atoms.XIM_PROTOCOL,
                window: target,
            },
        )?;
    } else {
        let prop = c
            .conn()
            .intern_atom(false, format!("_XIM_DATA_{}", sequence).as_bytes())?
            .reply()?
            .atom;
        if trace_on {
            eprintln!(
                "[unim-xim-trace] send role={role} route=property(>20B) opcode={} req={} target=0x{target:x} buf.len={} transport_max={transport_max} atom={prop} PropMode=APPEND",
                xim_opcode_name(trace_opcode),
                req.name(),
                buf.len(),
            );
        }
        *sequence = sequence.wrapping_add(1);
        c.conn().change_property(
            PropMode::APPEND,
            target,
            prop,
            AtomEnum::STRING,
            8,
            buf.len() as u32,
            buf,
        )?;
        c.conn().send_event(
            false,
            target,
            EventMask::NO_EVENT,
            ClientMessageEvent {
                data: [buf.len() as u32, prop, 0, 0, 0].into(),
                format: 32,
                sequence: 0,
                response_type: CLIENT_MESSAGE_EVENT,
                type_: atoms.XIM_PROTOCOL,
                window: target,
            },
        )?;
        if trace_on {
            // UNIM trace: ④ 이 property 를 클라이언트가 읽어갔는지 다음 송수신 시점에 되묻는다.
            *XIM_TRACE_LAST_PROP.lock().unwrap() = Some((target, prop));
        }
    }
    buf.clear();
    c.conn().flush()?;
    Ok(())
}

#[inline]
fn deserialize_event_impl(xev: &xim_parser::XEvent) -> KeyPressEvent {
    KeyPressEvent {
        response_type: xev.response_type,
        detail: xev.detail,
        sequence: xev.sequence,
        time: xev.time,
        root: xev.root,
        event: xev.event,
        child: xev.child,
        root_x: xev.root_x,
        root_y: xev.root_y,
        event_x: xev.event_x,
        event_y: xev.event_y,
        state: xev.state.into(),
        same_screen: xev.same_screen,
    }
}
