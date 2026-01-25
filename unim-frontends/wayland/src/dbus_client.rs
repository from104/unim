//! DBus 클라이언트 모듈
//!
//! XIM 프론트엔드에서 unim-daemon과 통신하기 위한 비동기 DBus 클라이언트입니다.

use log::{debug, error, info};
use std::sync::mpsc as std_mpsc;
use tokio::sync::mpsc;

use unim_dbus::client::{InputContextProxy, InputMethodProxy};
use zbus::zvariant::ObjectPath;
use zbus::Connection;

/// DBus 요청 타입
#[derive(Debug)]
pub enum DbusRequest {
    /// 새 입력 컨텍스트 생성
    CreateContext {
        client_name: String,
        response: Option<std_mpsc::Sender<DbusResponse>>,
    },
    /// 컨텍스트 파괴
    DestroyContext { context_path: String },
    /// 키 이벤트 처리
    #[allow(dead_code)]
    ProcessKey {
        context_path: String,
        keyval: u32,
        keycode: u32,
        state: u32,
        response: Option<std_mpsc::Sender<DbusResponse>>,
    },
    /// 포커스 인
    FocusIn { context_path: String },
    /// 포커스 아웃
    FocusOut {
        context_path: String,
        response: Option<std_mpsc::Sender<DbusResponse>>,
    },
    /// 리셋
    #[allow(dead_code)]
    Reset { context_path: String },
}

/// DBus 응답 타입
#[derive(Debug)]
pub enum DbusResponse {
    /// 컨텍스트 생성 성공
    ContextCreated { path: String },
    /// 키 처리 결과
    #[allow(dead_code)]
    KeyProcessed {
        consumed: bool,
        preedit: Option<String>,
        commit: Option<String>,
    },
    /// 커밋 텍스트 (focus_out 등에서)
    CommitText { text: String },
}

/// DBus 클라이언트
pub struct DbusClient {
    _tx: mpsc::Sender<DbusRequest>,
}

impl DbusClient {
    /// 새 DBus 클라이언트 생성 및 백그라운드 태스크 시작
    pub fn new() -> (Self, mpsc::Sender<DbusRequest>) {
        let (tx, rx) = mpsc::channel::<DbusRequest>(256);

        // 백그라운드 스레드에서 tokio 런타임 실행
        let tx_clone = tx.clone();
        std::thread::spawn(move || {
            let rt = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .expect("tokio 런타임 생성 실패");

            rt.block_on(async {
                if let Err(e) = run_dbus_client(rx).await {
                    error!("DBus 클라이언트 오류: {}", e);
                }
            });
        });

        (
            Self {
                _tx: tx_clone.clone(),
            },
            tx_clone,
        )
    }
}

/// DBus 클라이언트 실행 (비동기)
async fn run_dbus_client(mut rx: mpsc::Receiver<DbusRequest>) -> zbus::Result<()> {
    // DBus 세션 버스에 연결
    let connection = Connection::session().await?;
    info!("[XIM-DBus] 세션 버스 연결 성공");

    // InputMethod 프록시 생성
    let im_proxy = InputMethodProxy::new(&connection).await?;
    info!("[XIM-DBus] InputMethod 프록시 생성 완료");

    // 요청 처리 루프
    while let Some(request) = rx.recv().await {
        match request {
            DbusRequest::CreateContext {
                client_name,
                response,
            } => match im_proxy.create_input_context(&client_name).await {
                Ok(path) => {
                    debug!("[XIM-DBus] 컨텍스트 생성: {}", path);
                    if let Some(tx) = response {
                        let _ = tx.send(DbusResponse::ContextCreated { path });
                    }
                }
                Err(e) => {
                    error!("[XIM-DBus] 컨텍스트 생성 실패: {}", e);
                }
            },

            DbusRequest::DestroyContext { context_path } => {
                if let Ok(obj_path) = ObjectPath::try_from(context_path.as_str()) {
                    if let Ok(proxy) = InputContextProxy::builder(&connection)
                        .path(obj_path)
                        .expect("path error")
                        .build()
                        .await
                    {
                        let _ = proxy.destroy().await;
                        debug!("[XIM-DBus] 컨텍스트 파괴: {}", context_path);
                    }
                }
            }

            DbusRequest::ProcessKey {
                context_path,
                keyval,
                keycode,
                state,
                response,
            } => {
                let result =
                    process_key_event(&connection, &context_path, keyval, keycode, state).await;

                if let Some(tx) = response {
                    let _ = tx.send(result);
                }
            }

            DbusRequest::FocusIn { context_path } => {
                if let Ok(obj_path) = ObjectPath::try_from(context_path.as_str()) {
                    if let Ok(proxy) = InputContextProxy::builder(&connection)
                        .path(obj_path)
                        .expect("path error")
                        .build()
                        .await
                    {
                        let _ = proxy.focus_in().await;
                        debug!("[XIM-DBus] FocusIn: {}", context_path);
                    }
                }
            }

            DbusRequest::FocusOut {
                context_path,
                response,
            } => {
                if let Ok(obj_path) = ObjectPath::try_from(context_path.as_str()) {
                    if let Ok(proxy) = InputContextProxy::builder(&connection)
                        .path(obj_path)
                        .expect("path error")
                        .build()
                        .await
                    {
                        let _ = proxy.focus_out().await;
                        debug!("[XIM-DBus] FocusOut: {}", context_path);

                        if let Some(tx) = response {
                            let _ = tx.send(DbusResponse::CommitText {
                                text: String::new(),
                            });
                        }
                    }
                }
            }

            DbusRequest::Reset { context_path } => {
                if let Ok(obj_path) = ObjectPath::try_from(context_path.as_str()) {
                    if let Ok(proxy) = InputContextProxy::builder(&connection)
                        .path(obj_path)
                        .expect("path error")
                        .build()
                        .await
                    {
                        let _ = proxy.reset().await;
                        debug!("[XIM-DBus] Reset: {}", context_path);
                    }
                }
            }
        }
    }

    Ok(())
}

/// 키 이벤트 처리 (별도 함수로 분리)
async fn process_key_event(
    connection: &Connection,
    context_path: &str,
    keyval: u32,
    keycode: u32,
    state: u32,
) -> DbusResponse {
    let obj_path = match ObjectPath::try_from(context_path) {
        Ok(p) => p,
        Err(_) => {
            return DbusResponse::KeyProcessed {
                consumed: false,
                preedit: None,
                commit: None,
            };
        }
    };

    let ctx_proxy = match InputContextProxy::builder(connection)
        .path(obj_path)
        .expect("path error")
        .build()
        .await
    {
        Ok(proxy) => proxy,
        Err(e) => {
            error!("[XIM-DBus] 프록시 생성 실패: {}", e);
            return DbusResponse::KeyProcessed {
                consumed: false,
                preedit: None,
                commit: None,
            };
        }
    };

    // 키 이벤트 처리 - 반환값: (consumed, preedit, commit)
    let (consumed, preedit, commit) =
        match ctx_proxy.process_key_event(keyval, keycode, state).await {
            Ok(result) => result,
            Err(e) => {
                error!("[XIM-DBus] 키 처리 실패: {}", e);
                return DbusResponse::KeyProcessed {
                    consumed: false,
                    preedit: None,
                    commit: None,
                };
            }
        };

    // TODO: preedit/commit 시그널 수신 구현
    DbusResponse::KeyProcessed {
        consumed,
        preedit: if preedit.is_empty() {
            None
        } else {
            Some(preedit)
        },
        commit: if commit.is_empty() {
            None
        } else {
            Some(commit)
        },
    }
}
