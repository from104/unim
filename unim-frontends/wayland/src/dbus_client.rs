//! DBus 클라이언트 모듈
//!
//! unim-daemon과 비동기 통신을 위한 DBus 클라이언트입니다.
//! tokio 백그라운드 스레드에서 실행됩니다.

use std::sync::mpsc as std_mpsc;
use tokio::sync::mpsc;
use unim::unim_log;

use unim_dbus::client::{InputContextProxy, InputMethodProxy};
use zbus::zvariant::ObjectPath;
use zbus::Connection;

/// DBus 요청 타입
#[derive(Debug)]
pub enum DbusRequest {
    /// 새 입력 컨텍스트 생성
    CreateContext {
        client_name: String,
        window_id: String,
        response: Option<std_mpsc::Sender<DbusResponse>>,
    },
    /// 컨텍스트 파괴
    DestroyContext { context_path: String },
    /// 키 이벤트 처리
    ProcessKey {
        context_path: String,
        keyval: u32,
        keycode: u32,
        state: u32,
        response: Option<std_mpsc::Sender<DbusResponse>>,
    },
    /// 포커스 인
    FocusIn {
        context_path: String,
        window_id: String,
    },
    /// 포커스 아웃
    FocusOut {
        context_path: String,
        response: Option<std_mpsc::Sender<DbusResponse>>,
    },
    /// 리셋
    Reset { context_path: String },
    /// 한자 후보 조회
    GetHanjaCandidates {
        context_path: String,
        response: Option<std_mpsc::Sender<DbusResponse>>,
    },
    /// 한자 선택
    SelectHanja {
        context_path: String,
        index: u32,
        response: Option<std_mpsc::Sender<DbusResponse>>,
    },
    /// 한자 취소
    CancelHanja { context_path: String },
    /// 특수문자 후보 조회
    GetSpecialCharCandidates {
        context_path: String,
        response: Option<std_mpsc::Sender<DbusResponse>>,
    },
    /// 특수문자 선택
    SelectSpecialChar {
        context_path: String,
        index: u32,
        response: Option<std_mpsc::Sender<DbusResponse>>,
    },
    /// 특수문자 취소
    CancelSpecialChar { context_path: String },
}

/// DBus 응답 타입
#[derive(Debug)]
pub enum DbusResponse {
    /// 컨텍스트 생성 성공
    ContextCreated { path: String },
    /// 키 처리 결과
    KeyProcessed {
        consumed: bool,
        preedit: String,
        commit: String,
    },
    /// 커밋 텍스트 (focus_out 등에서)
    CommitText { text: String },
    /// 한자 후보 목록
    HanjaCandidates {
        target: String,
        candidates: Vec<(String, String)>,
    },
    /// 한자 선택 결과
    HanjaSelected { commit: String },
    /// 특수문자 후보 목록
    SpecialCharCandidates {
        target: String,
        characters: Vec<String>,
        top_row: String,
    },
    /// 특수문자 선택 결과
    SpecialCharSelected { commit: String },
}

/// DBus 클라이언트
pub struct DbusClient {
    _tx: mpsc::Sender<DbusRequest>,
}

impl DbusClient {
    /// 새 DBus 클라이언트 생성 및 백그라운드 태스크 시작
    pub fn new() -> (Self, mpsc::Sender<DbusRequest>) {
        let (tx, rx) = mpsc::channel::<DbusRequest>(256);

        let tx_clone = tx.clone();
        std::thread::spawn(move || {
            let rt = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .expect("tokio 런타임 생성 실패");

            rt.block_on(async {
                if let Err(e) = run_dbus_client(rx).await {
                    unim_log!("WAYLAND_DBUS", "DBus 클라이언트 오류: {}", e);
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

/// DBus 클라이언트 비동기 실행
async fn run_dbus_client(mut rx: mpsc::Receiver<DbusRequest>) -> zbus::Result<()> {
    let connection = Connection::session().await?;
    unim_log!("WAYLAND_DBUS", "세션 버스 연결 성공");

    let im_proxy = InputMethodProxy::new(&connection).await?;
    unim_log!("WAYLAND_DBUS", "InputMethod 프록시 생성 완료");

    while let Some(request) = rx.recv().await {
        match request {
            DbusRequest::CreateContext {
                client_name,
                window_id,
                response,
            } => {
                match im_proxy
                    .create_input_context(&client_name, &window_id)
                    .await
                {
                    Ok(path) => {
                        unim_log!("WAYLAND_DBUS", "컨텍스트 생성: {}", path);
                        if let Some(tx) = response {
                            let _ = tx.send(DbusResponse::ContextCreated { path });
                        }
                    }
                    Err(e) => {
                        unim_log!("WAYLAND_DBUS", "컨텍스트 생성 실패: {}", e);
                    }
                }
            }

            DbusRequest::DestroyContext { context_path } => {
                if let Ok(proxy) = build_ctx_proxy(&connection, &context_path).await {
                    let _ = proxy.destroy().await;
                    unim_log!("WAYLAND_DBUS", "컨텍스트 파괴: {}", context_path);
                }
            }

            DbusRequest::ProcessKey {
                context_path,
                keyval,
                keycode,
                state,
                response,
            } => {
                let result = if let Ok(proxy) = build_ctx_proxy(&connection, &context_path).await {
                    match proxy.process_key_event(keyval, keycode, state).await {
                        Ok((consumed, preedit, commit)) => DbusResponse::KeyProcessed {
                            consumed,
                            preedit,
                            commit,
                        },
                        Err(e) => {
                            unim_log!("WAYLAND_DBUS", "키 처리 실패: {}", e);
                            DbusResponse::KeyProcessed {
                                consumed: false,
                                preedit: String::new(),
                                commit: String::new(),
                            }
                        }
                    }
                } else {
                    DbusResponse::KeyProcessed {
                        consumed: false,
                        preedit: String::new(),
                        commit: String::new(),
                    }
                };

                if let Some(tx) = response {
                    let _ = tx.send(result);
                }
            }

            DbusRequest::FocusIn {
                context_path,
                window_id,
            } => {
                if let Ok(proxy) = build_ctx_proxy(&connection, &context_path).await {
                    let _ = proxy.focus_in(&window_id).await;
                    unim_log!("WAYLAND_DBUS", "FocusIn: {}", context_path);
                }
            }

            DbusRequest::FocusOut {
                context_path,
                response,
            } => {
                if let Ok(proxy) = build_ctx_proxy(&connection, &context_path).await {
                    match proxy.focus_out().await {
                        Ok(commit_text) => {
                            unim_log!("WAYLAND_DBUS", "FocusOut: {}", context_path);
                            if let Some(tx) = response {
                                let _ = tx.send(DbusResponse::CommitText { text: commit_text });
                            }
                        }
                        Err(_) => {
                            if let Some(tx) = response {
                                let _ = tx.send(DbusResponse::CommitText {
                                    text: String::new(),
                                });
                            }
                        }
                    }
                }
            }

            DbusRequest::Reset { context_path } => {
                if let Ok(proxy) = build_ctx_proxy(&connection, &context_path).await {
                    let _ = proxy.reset().await;
                    unim_log!("WAYLAND_DBUS", "Reset: {}", context_path);
                }
            }

            DbusRequest::GetHanjaCandidates {
                context_path,
                response,
            } => {
                if let Ok(proxy) = build_ctx_proxy(&connection, &context_path).await {
                    match proxy.get_hanja_candidates().await {
                        Ok((target, candidates)) => {
                            unim_log!(
                                "WAYLAND_DBUS",
                                "한자 후보: target='{}', count={}",
                                target,
                                candidates.len()
                            );
                            if let Some(tx) = response {
                                let _ =
                                    tx.send(DbusResponse::HanjaCandidates { target, candidates });
                            }
                        }
                        Err(e) => {
                            unim_log!("WAYLAND_DBUS", "한자 후보 조회 실패: {}", e);
                            if let Some(tx) = response {
                                let _ = tx.send(DbusResponse::HanjaCandidates {
                                    target: String::new(),
                                    candidates: Vec::new(),
                                });
                            }
                        }
                    }
                }
            }

            DbusRequest::SelectHanja {
                context_path,
                index,
                response,
            } => {
                if let Ok(proxy) = build_ctx_proxy(&connection, &context_path).await {
                    match proxy.select_hanja(index).await {
                        Ok(commit) => {
                            unim_log!(
                                "WAYLAND_DBUS",
                                "한자 선택: index={}, commit='{}'",
                                index,
                                commit
                            );
                            if let Some(tx) = response {
                                let _ = tx.send(DbusResponse::HanjaSelected { commit });
                            }
                        }
                        Err(e) => {
                            unim_log!("WAYLAND_DBUS", "한자 선택 실패: {}", e);
                            if let Some(tx) = response {
                                let _ = tx.send(DbusResponse::HanjaSelected {
                                    commit: String::new(),
                                });
                            }
                        }
                    }
                }
            }

            DbusRequest::CancelHanja { context_path } => {
                if let Ok(proxy) = build_ctx_proxy(&connection, &context_path).await {
                    let _ = proxy.cancel_hanja().await;
                    unim_log!("WAYLAND_DBUS", "한자 취소: {}", context_path);
                }
            }

            DbusRequest::GetSpecialCharCandidates {
                context_path,
                response,
            } => {
                if let Ok(proxy) = build_ctx_proxy(&connection, &context_path).await {
                    match proxy.get_special_char_candidates().await {
                        Ok((target, characters, top_row)) => {
                            unim_log!(
                                "WAYLAND_DBUS",
                                "특수문자 후보: target='{}', count={}, top_row='{}'",
                                target,
                                characters.len(),
                                top_row
                            );
                            if let Some(tx) = response {
                                let _ = tx.send(DbusResponse::SpecialCharCandidates {
                                    target,
                                    characters,
                                    top_row,
                                });
                            }
                        }
                        Err(e) => {
                            unim_log!("WAYLAND_DBUS", "특수문자 후보 조회 실패: {}", e);
                            if let Some(tx) = response {
                                let _ = tx.send(DbusResponse::SpecialCharCandidates {
                                    target: String::new(),
                                    characters: Vec::new(),
                                    top_row: String::new(),
                                });
                            }
                        }
                    }
                }
            }

            DbusRequest::SelectSpecialChar {
                context_path,
                index,
                response,
            } => {
                if let Ok(proxy) = build_ctx_proxy(&connection, &context_path).await {
                    match proxy.select_special_char(index).await {
                        Ok(commit) => {
                            unim_log!(
                                "WAYLAND_DBUS",
                                "특수문자 선택: index={}, commit='{}'",
                                index,
                                commit
                            );
                            if let Some(tx) = response {
                                let _ = tx.send(DbusResponse::SpecialCharSelected { commit });
                            }
                        }
                        Err(e) => {
                            unim_log!("WAYLAND_DBUS", "특수문자 선택 실패: {}", e);
                            if let Some(tx) = response {
                                let _ = tx.send(DbusResponse::SpecialCharSelected {
                                    commit: String::new(),
                                });
                            }
                        }
                    }
                }
            }

            DbusRequest::CancelSpecialChar { context_path } => {
                if let Ok(proxy) = build_ctx_proxy(&connection, &context_path).await {
                    let _ = proxy.cancel_special_char().await;
                    unim_log!("WAYLAND_DBUS", "특수문자 취소: {}", context_path);
                }
            }
        }
    }

    Ok(())
}

/// InputContext 프록시 빌드 헬퍼
async fn build_ctx_proxy<'a>(
    connection: &'a Connection,
    context_path: &'a str,
) -> zbus::Result<InputContextProxy<'a>> {
    let obj_path = ObjectPath::try_from(context_path)?;
    InputContextProxy::builder(connection)
        .path(obj_path)?
        .build()
        .await
}
