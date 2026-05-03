//! DBus 클라이언트 모듈
//!
//! XIM 프론트엔드에서 unim-daemon과 통신하기 위한 비동기 DBus 클라이언트입니다.

use std::sync::mpsc as std_mpsc;
use tokio::sync::mpsc;
use unim::unim_log;

use unim_dbus::client::{InputContextProxy, InputMethodProxy};
use zbus::zvariant::ObjectPath;
use zbus::Connection;

/// 팝업 이벤트 (DBus 시그널 기반)
#[derive(Debug)]
#[allow(dead_code)]
pub enum PopupEvent {
    /// 한자 팝업 표시
    ShowHanja {
        target: String,
        candidates: Vec<(String, String)>,
        /// 활성 영문 키맵 top_row (특수문자와 동일 source).
        top_row: String,
        cursor_x: i32,
        cursor_y: i32,
    },
    /// 특수문자 팝업 표시
    ShowSpecial {
        target: String,
        characters: Vec<String>,
        top_row: String,
        cursor_x: i32,
        cursor_y: i32,
    },
    /// 이모지 팝업 표시 (PR #5: ShowEmojiPopupV2 시그널).
    ///
    /// 한자/특수문자와 다르게 카테고리 메타·MRU·페이지 데이터를 한꺼번에 받는다.
    /// XIM 은 Embedded 모드에서만 자체 emoji_window 를 띄우고, Standalone 모드에서는
    /// 본 이벤트를 무시한다 (GTK standalone GUI / GNOME extension 이 표시 담당).
    ShowEmoji {
        target_cat_id: String,
        items: Vec<String>,
        top_row: String,
        recent: Vec<String>,
        categories: Vec<(String, String, String, u32)>,
        cursor_x: i32,
        cursor_y: i32,
    },
    /// 팝업 숨김
    Hide,
    /// 팝업 네비게이션 (페이지/선택 변경)
    Navigate {
        page: i32,
        total_pages: i32,
        selected: i32,
        rows: i32,
        cols: i32,
        sel_row: i32,
        sel_col: i32,
    },
    /// AutoTypeFix 교정 (백스페이스 N회 + 교정 텍스트 커밋)
    AutoTypeFix {
        delete_chars: u32,
        commit_text: String,
        preedit_text: String,
    },
    /// Standalone 팝업 마우스 클릭 시 커밋 텍스트
    CommitText { text: String },
    /// 한자 즐겨찾기 변경 (엔진 → 프런트엔드)
    HanjaBookmarkChanged { index: u32, bookmarked: bool },
    /// 한자 후보 재정렬 (즐겨찾기 토글 직후, 커서 점프 포함)
    HanjaCandidatesReordered {
        target: String,
        candidates: Vec<(String, String)>,
        bookmarks: Vec<bool>,
        new_cursor: u32,
        page: i32,
        sel_row: i32,
        sel_col: i32,
        bookmarked: bool,
        was_bookmarked: bool,
    },
}

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
    #[allow(dead_code)]
    SelectHanja {
        context_path: String,
        index: u32,
        response: Option<std_mpsc::Sender<DbusResponse>>,
    },
    /// 한자 취소 (트리거 문자 반환)
    CancelHanja {
        context_path: String,
        response: Option<std_mpsc::Sender<DbusResponse>>,
    },
    /// 특수문자 후보 조회
    GetSpecialCharCandidates {
        context_path: String,
        response: Option<std_mpsc::Sender<DbusResponse>>,
    },
    /// 특수문자 선택
    #[allow(dead_code)]
    SelectSpecialChar {
        context_path: String,
        index: u32,
        response: Option<std_mpsc::Sender<DbusResponse>>,
    },
    /// 특수문자 취소 (트리거 문자 반환)
    CancelSpecialChar {
        context_path: String,
        response: Option<std_mpsc::Sender<DbusResponse>>,
    },
    /// 커서 위치 보고 (팝업 포지셔닝용)
    ReportCursorRect {
        context_path: String,
        x: i32,
        y: i32,
        width: i32,
        height: i32,
    },
    /// 한자 즐겨찾기 상태 조회 (Vec<bool> 응답)
    GetHanjaBookmarkStates {
        context_path: String,
        response: Option<std_mpsc::Sender<DbusResponse>>,
    },
    /// 한자 즐겨찾기 토글 (엔진이 persist + HanjaBookmarkChanged 시그널 발행)
    ///
    /// XIM은 popup 키를 로컬에서 처리하지 않고 모든 키를 엔진으로 보내므로,
    /// 엔진이 Space를 직접 ToggleBookmark로 변환한다. 이 variant는 향후 마우스
    /// 기반 토글(우클릭 등) 대비용으로 남겨둔다.
    #[allow(dead_code)]
    ToggleHanjaBookmark { context_path: String, index: u32 },
    /// 팝업 페이지 이동 (마우스 ◀/▶ 좌클릭 또는 우클릭 다음 페이지).
    /// `direction`: 0 = 이전, 1 = 다음. Phase 6.
    PopupChangePage { context_path: String, direction: i32 },
}

/// DBus 응답 타입
#[derive(Debug)]
pub enum DbusResponse {
    /// 컨텍스트 생성 성공
    ContextCreated { path: String },
    /// 컨텍스트 생성 실패
    ContextCreationFailed,
    /// 키 처리 결과
    KeyProcessed {
        consumed: bool,
        preedit: Option<String>,
        commit: Option<String>,
    },
    /// 커밋 텍스트 (focus_out 등에서)
    CommitText { text: String },
    /// 한자 후보 목록
    HanjaCandidates {
        target: String,
        candidates: Vec<(String, String)>,
    },
    /// 한자 선택 결과
    HanjaSelected {
        #[allow(dead_code)]
        commit: String,
    },
    /// 특수문자 후보 목록
    SpecialCharCandidates {
        target: String,
        characters: Vec<String>,
        top_row: String,
    },
    /// 특수문자 선택 결과
    SpecialCharSelected {
        #[allow(dead_code)]
        commit: String,
    },
    /// 한자 즐겨찾기 상태 조회 결과
    HanjaBookmarkStates { states: Vec<bool> },
}

/// DBus 클라이언트
pub struct DbusClient {
    _tx: mpsc::Sender<DbusRequest>,
}

impl DbusClient {
    /// 새 DBus 클라이언트 생성 및 백그라운드 태스크 시작
    pub fn new(popup_tx: std_mpsc::Sender<PopupEvent>) -> (Self, mpsc::Sender<DbusRequest>) {
        let (tx, rx) = mpsc::channel::<DbusRequest>(256);

        // 백그라운드 스레드에서 tokio 런타임 실행
        let tx_clone = tx.clone();
        std::thread::spawn(move || {
            let rt = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .expect("tokio 런타임 생성 실패");

            rt.block_on(async {
                if let Err(e) = run_dbus_client(rx, popup_tx).await {
                    unim_log!("XIM_DBUS", "DBus 클라이언트 오류: {}", e);
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
async fn run_dbus_client(
    mut rx: mpsc::Receiver<DbusRequest>,
    popup_tx: std_mpsc::Sender<PopupEvent>,
) -> zbus::Result<()> {
    // DBus 세션 버스에 연결
    let connection = Connection::session().await?;
    unim_log!("XIM_DBUS", "[XIM-DBus] 세션 버스 연결 성공");

    // InputMethod 프록시 생성
    let im_proxy = InputMethodProxy::new(&connection).await?;
    unim_log!("XIM_DBUS", "[XIM-DBus] InputMethod 프록시 생성 완료");

    // 요청 처리 루프
    while let Some(request) = rx.recv().await {
        match request {
            DbusRequest::CreateContext {
                client_name,
                window_id,
                response,
            } => match im_proxy
                .create_input_context(&client_name, &window_id)
                .await
            {
                Ok(path) => {
                    unim_log!("XIM_DBUS", "[XIM-DBus] 컨텍스트 생성: {}", path);

                    // 팝업 시그널 구독 시작
                    let popup_tx_clone = popup_tx.clone();
                    let conn_clone = connection.clone();
                    let path_clone = path.clone();
                    tokio::spawn(async move {
                        subscribe_popup_signals(&conn_clone, &path_clone, popup_tx_clone).await;
                    });

                    if let Some(tx) = response {
                        let _ = tx.send(DbusResponse::ContextCreated { path });
                    }
                }
                Err(e) => {
                    unim_log!("XIM_DBUS", "[XIM-DBus] 컨텍스트 생성 실패: {}", e);
                    // 실패 응답 전송 - 핸들러가 타임아웃까지 대기하지 않도록
                    if let Some(tx) = response {
                        let _ = tx.send(DbusResponse::ContextCreationFailed);
                    }
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
                        unim_log!("XIM_DBUS", "[XIM-DBus] 컨텍스트 파괴: {}", context_path);
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

            DbusRequest::FocusIn {
                context_path,
                window_id,
            } => {
                if let Ok(obj_path) = ObjectPath::try_from(context_path.as_str()) {
                    if let Ok(proxy) = InputContextProxy::builder(&connection)
                        .path(obj_path)
                        .expect("path error")
                        .build()
                        .await
                    {
                        let _ = proxy.focus_in(&window_id).await;
                        unim_log!("XIM_DBUS", "[XIM-DBus] FocusIn: {}", context_path);
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
                        // focus_out()이 커밋 텍스트를 반환
                        let commit_text = proxy.focus_out().await.unwrap_or_default();
                        unim_log!(
                            "XIM_DBUS",
                            "[XIM-DBus] FocusOut: {} (commit: '{}')",
                            context_path,
                            commit_text
                        );

                        if let Some(tx) = response {
                            let _ = tx.send(DbusResponse::CommitText { text: commit_text });
                        }
                    } else if let Some(tx) = response {
                        let _ = tx.send(DbusResponse::CommitText {
                            text: String::new(),
                        });
                    }
                } else if let Some(tx) = response {
                    let _ = tx.send(DbusResponse::CommitText {
                        text: String::new(),
                    });
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
                        unim_log!("XIM_DBUS", "[XIM-DBus] Reset: {}", context_path);
                    }
                }
            }

            DbusRequest::GetHanjaCandidates {
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
                        match proxy.get_hanja_candidates().await {
                            Ok((target, candidates)) => {
                                unim_log!(
                                    "XIM_DBUS",
                                    "[XIM-DBus] 한자 후보: target='{}', count={}",
                                    target,
                                    candidates.len()
                                );
                                if let Some(tx) = response {
                                    let _ = tx
                                        .send(DbusResponse::HanjaCandidates { target, candidates });
                                }
                            }
                            Err(e) => {
                                unim_log!("XIM_DBUS", "[XIM-DBus] 한자 후보 조회 실패: {}", e);
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
            }

            DbusRequest::SelectHanja {
                context_path,
                index,
                response,
            } => {
                if let Ok(obj_path) = ObjectPath::try_from(context_path.as_str()) {
                    if let Ok(proxy) = InputContextProxy::builder(&connection)
                        .path(obj_path)
                        .expect("path error")
                        .build()
                        .await
                    {
                        match proxy.select_hanja(index).await {
                            Ok(commit) => {
                                unim_log!(
                                    "XIM_DBUS",
                                    "[XIM-DBus] 한자 선택: index={}, commit='{}'",
                                    index,
                                    commit
                                );
                                if let Some(tx) = response {
                                    let _ = tx.send(DbusResponse::HanjaSelected { commit });
                                }
                            }
                            Err(e) => {
                                unim_log!("XIM_DBUS", "[XIM-DBus] 한자 선택 실패: {}", e);
                                if let Some(tx) = response {
                                    let _ = tx.send(DbusResponse::HanjaSelected {
                                        commit: String::new(),
                                    });
                                }
                            }
                        }
                    }
                }
            }

            DbusRequest::CancelHanja {
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
                        match proxy.cancel_hanja().await {
                            Ok(text) => {
                                unim_log!("XIM_DBUS", "[XIM-DBus] 한자 취소: commit='{}'", text);
                                if let Some(tx) = response {
                                    let _ = tx.send(DbusResponse::CommitText { text });
                                }
                            }
                            Err(e) => {
                                unim_log!("XIM_DBUS", "[XIM-DBus] 한자 취소 실패: {}", e);
                                if let Some(tx) = response {
                                    let _ = tx.send(DbusResponse::CommitText {
                                        text: String::new(),
                                    });
                                }
                            }
                        }
                    }
                }
            }

            DbusRequest::GetSpecialCharCandidates {
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
                        match proxy.get_special_char_candidates().await {
                            Ok((target, characters, top_row)) => {
                                unim_log!(
                                    "XIM_DBUS",
                                    "[XIM-DBus] 특수문자 후보: target='{}', count={}, top_row='{}'",
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
                                unim_log!("XIM_DBUS", "[XIM-DBus] 특수문자 후보 조회 실패: {}", e);
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
            }

            DbusRequest::SelectSpecialChar {
                context_path,
                index,
                response,
            } => {
                if let Ok(obj_path) = ObjectPath::try_from(context_path.as_str()) {
                    if let Ok(proxy) = InputContextProxy::builder(&connection)
                        .path(obj_path)
                        .expect("path error")
                        .build()
                        .await
                    {
                        match proxy.select_special_char(index).await {
                            Ok(commit) => {
                                unim_log!(
                                    "XIM_DBUS",
                                    "[XIM-DBus] 특수문자 선택: index={}, commit='{}'",
                                    index,
                                    commit
                                );
                                if let Some(tx) = response {
                                    let _ = tx.send(DbusResponse::SpecialCharSelected { commit });
                                }
                            }
                            Err(e) => {
                                unim_log!("XIM_DBUS", "[XIM-DBus] 특수문자 선택 실패: {}", e);
                                if let Some(tx) = response {
                                    let _ = tx.send(DbusResponse::SpecialCharSelected {
                                        commit: String::new(),
                                    });
                                }
                            }
                        }
                    }
                }
            }

            DbusRequest::CancelSpecialChar {
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
                        match proxy.cancel_special_char().await {
                            Ok(text) => {
                                unim_log!(
                                    "XIM_DBUS",
                                    "[XIM-DBus] 특수문자 취소: commit='{}'",
                                    text
                                );
                                if let Some(tx) = response {
                                    let _ = tx.send(DbusResponse::CommitText { text });
                                }
                            }
                            Err(e) => {
                                unim_log!("XIM_DBUS", "[XIM-DBus] 특수문자 취소 실패: {}", e);
                                if let Some(tx) = response {
                                    let _ = tx.send(DbusResponse::CommitText {
                                        text: String::new(),
                                    });
                                }
                            }
                        }
                    }
                }
            }

            DbusRequest::ReportCursorRect {
                context_path,
                x,
                y,
                width,
                height,
            } => {
                if let Ok(obj_path) = ObjectPath::try_from(context_path.as_str()) {
                    if let Ok(proxy) = InputContextProxy::builder(&connection)
                        .path(obj_path)
                        .expect("path error")
                        .build()
                        .await
                    {
                        let _ = proxy.report_cursor_rect(x, y, width, height).await;
                    }
                }
            }

            DbusRequest::GetHanjaBookmarkStates {
                context_path,
                response,
            } => {
                let mut states: Vec<bool> = Vec::new();
                if let Ok(obj_path) = ObjectPath::try_from(context_path.as_str()) {
                    if let Ok(proxy) = InputContextProxy::builder(&connection)
                        .path(obj_path)
                        .expect("path error")
                        .build()
                        .await
                    {
                        match proxy.get_hanja_bookmark_states().await {
                            Ok(s) => states = s,
                            Err(e) => {
                                unim_log!(
                                    "XIM_DBUS",
                                    "[XIM-DBus] GetHanjaBookmarkStates 실패: {}",
                                    e
                                );
                            }
                        }
                    }
                }
                if let Some(tx) = response {
                    let _ = tx.send(DbusResponse::HanjaBookmarkStates { states });
                }
            }

            DbusRequest::ToggleHanjaBookmark {
                context_path,
                index,
            } => {
                if let Ok(obj_path) = ObjectPath::try_from(context_path.as_str()) {
                    if let Ok(proxy) = InputContextProxy::builder(&connection)
                        .path(obj_path)
                        .expect("path error")
                        .build()
                        .await
                    {
                        // 엔진이 persist + HanjaBookmarkChanged 시그널 발행.
                        // 반환값은 사용하지 않는다 (시그널로 UI 갱신).
                        let _ = proxy.toggle_hanja_bookmark(index).await;
                    }
                }
            }

            DbusRequest::PopupChangePage {
                context_path,
                direction,
            } => {
                if let Ok(obj_path) = ObjectPath::try_from(context_path.as_str()) {
                    if let Ok(proxy) = InputContextProxy::builder(&connection)
                        .path(obj_path)
                        .expect("path error")
                        .build()
                        .await
                    {
                        // 엔진이 PopupNavigate 시그널 발행 → cursor 보존된 페이지 점프.
                        let _ = proxy.popup_change_page(direction).await;
                    }
                }
            }
        }
    }

    Ok(())
}

/// InputContext 팝업 시그널 구독 (ShowHanjaPopup, ShowSpecialPopup, HidePopup, PopupNavigate)
async fn subscribe_popup_signals(
    connection: &Connection,
    context_path: &str,
    popup_tx: std_mpsc::Sender<PopupEvent>,
) {
    use zbus::export::futures_util::StreamExt;

    let obj_path = match ObjectPath::try_from(context_path) {
        Ok(p) => p,
        Err(e) => {
            unim_log!("XIM_DBUS", "[XIM-DBus] 팝업 시그널 경로 변환 실패: {}", e);
            return;
        }
    };

    let proxy = match InputContextProxy::builder(connection)
        .path(obj_path)
        .expect("path error")
        .build()
        .await
    {
        Ok(p) => p,
        Err(e) => {
            unim_log!("XIM_DBUS", "[XIM-DBus] 팝업 시그널 구독 실패: {}", e);
            return;
        }
    };

    // 5개 시그널 스트림 동시 구독
    let mut hanja_stream = match proxy.receive_show_hanja_popup().await {
        Ok(s) => s,
        Err(e) => {
            unim_log!("XIM_DBUS", "[XIM-DBus] ShowHanjaPopup 구독 실패: {}", e);
            return;
        }
    };
    let mut special_stream = match proxy.receive_show_special_popup().await {
        Ok(s) => s,
        Err(e) => {
            unim_log!("XIM_DBUS", "[XIM-DBus] ShowSpecialPopup 구독 실패: {}", e);
            return;
        }
    };
    let mut emoji_stream = match proxy.receive_show_emoji_popup_v2().await {
        Ok(s) => s,
        Err(e) => {
            unim_log!("XIM_DBUS", "[XIM-DBus] ShowEmojiPopupV2 구독 실패: {}", e);
            return;
        }
    };
    let mut hide_stream = match proxy.receive_hide_popup().await {
        Ok(s) => s,
        Err(e) => {
            unim_log!("XIM_DBUS", "[XIM-DBus] HidePopup 구독 실패: {}", e);
            return;
        }
    };
    let mut navigate_stream = match proxy.receive_popup_navigate().await {
        Ok(s) => s,
        Err(e) => {
            unim_log!("XIM_DBUS", "[XIM-DBus] PopupNavigate 구독 실패: {}", e);
            return;
        }
    };
    let mut autofix_stream = match proxy.receive_auto_typefix_apply().await {
        Ok(s) => s,
        Err(e) => {
            unim_log!("XIM_DBUS", "[XIM-DBus] AutoTypefixApply 구독 실패: {}", e);
            return;
        }
    };
    let mut commit_stream = match proxy.receive_commit_text().await {
        Ok(s) => s,
        Err(e) => {
            unim_log!("XIM_DBUS", "[XIM-DBus] CommitText 구독 실패: {}", e);
            return;
        }
    };
    let mut bookmark_stream = match proxy.receive_hanja_bookmark_changed().await {
        Ok(s) => s,
        Err(e) => {
            unim_log!(
                "XIM_DBUS",
                "[XIM-DBus] HanjaBookmarkChanged 구독 실패: {}",
                e
            );
            return;
        }
    };
    let mut reordered_stream = match proxy.receive_hanja_candidates_reordered().await {
        Ok(s) => s,
        Err(e) => {
            unim_log!(
                "XIM_DBUS",
                "[XIM-DBus] HanjaCandidatesReordered 구독 실패: {}",
                e
            );
            return;
        }
    };

    unim_log!(
        "XIM_DBUS",
        "[XIM-DBus] 팝업 시그널 구독 시작: {}",
        context_path
    );

    loop {
        tokio::select! {
            Some(signal) = hanja_stream.next() => {
                if let Ok(args) = signal.args() {
                    let _ = popup_tx.send(PopupEvent::ShowHanja {
                        target: args.target,
                        candidates: args.candidates,
                        top_row: args.top_row,
                        cursor_x: args.cursor_x,
                        cursor_y: args.cursor_y,
                    });
                }
            }
            Some(signal) = special_stream.next() => {
                if let Ok(args) = signal.args() {
                    let _ = popup_tx.send(PopupEvent::ShowSpecial {
                        target: args.target,
                        characters: args.characters,
                        top_row: args.top_row,
                        cursor_x: args.cursor_x,
                        cursor_y: args.cursor_y,
                    });
                }
            }
            Some(signal) = emoji_stream.next() => {
                if let Ok(args) = signal.args() {
                    let _ = popup_tx.send(PopupEvent::ShowEmoji {
                        target_cat_id: args.target_cat_id,
                        items: args.items,
                        top_row: args.top_row,
                        recent: args.recent,
                        categories: args.categories,
                        cursor_x: args.cursor_x,
                        cursor_y: args.cursor_y,
                    });
                }
            }
            Some(_signal) = hide_stream.next() => {
                let _ = popup_tx.send(PopupEvent::Hide);
            }
            Some(signal) = navigate_stream.next() => {
                if let Ok(args) = signal.args() {
                    let _ = popup_tx.send(PopupEvent::Navigate {
                        page: args.page,
                        total_pages: args.total_pages,
                        selected: args.selected,
                        rows: args.rows,
                        cols: args.cols,
                        sel_row: args.sel_row,
                        sel_col: args.sel_col,
                    });
                }
            }
            Some(signal) = autofix_stream.next() => {
                if let Ok(args) = signal.args() {
                    let _ = popup_tx.send(PopupEvent::AutoTypeFix {
                        delete_chars: args.delete_chars,
                        commit_text: args.commit_text.to_string(),
                        preedit_text: args.preedit_text.to_string(),
                    });
                }
            }
            Some(signal) = commit_stream.next() => {
                if let Ok(args) = signal.args() {
                    let text = args.text.to_string();
                    if !text.is_empty() {
                        let _ = popup_tx.send(PopupEvent::CommitText { text });
                    }
                }
            }
            Some(signal) = bookmark_stream.next() => {
                if let Ok(args) = signal.args() {
                    let _ = popup_tx.send(PopupEvent::HanjaBookmarkChanged {
                        index: args.index,
                        bookmarked: args.bookmarked,
                    });
                }
            }
            Some(signal) = reordered_stream.next() => {
                if let Ok(args) = signal.args() {
                    let candidates: Vec<(String, String)> = args
                        .hanjas
                        .iter()
                        .enumerate()
                        .map(|(i, h)| {
                            let m = args.meanings.get(i).cloned().unwrap_or_default();
                            (h.clone(), m)
                        })
                        .collect();
                    let _ = popup_tx.send(PopupEvent::HanjaCandidatesReordered {
                        target: args.target.to_string(),
                        candidates,
                        bookmarks: args.bookmarks.clone(),
                        new_cursor: args.new_cursor,
                        page: args.page,
                        sel_row: args.sel_row,
                        sel_col: args.sel_col,
                        bookmarked: args.bookmarked,
                        was_bookmarked: args.was_bookmarked,
                    });
                }
            }
            else => break,
        }
    }

    unim_log!(
        "XIM_DBUS",
        "[XIM-DBus] 팝업 시그널 구독 종료: {}",
        context_path
    );
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
            unim_log!("XIM_DBUS", "[XIM-DBus] 프록시 생성 실패: {}", e);
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
                unim_log!("XIM_DBUS", "[XIM-DBus] 키 처리 실패: {}", e);
                return DbusResponse::KeyProcessed {
                    consumed: false,
                    preedit: None,
                    commit: None,
                };
            }
        };

    DbusResponse::KeyProcessed {
        consumed,
        preedit: Some(preedit),
        commit: if commit.is_empty() {
            None
        } else {
            Some(commit)
        },
    }
}
