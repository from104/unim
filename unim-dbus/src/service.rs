//! DBus 서비스 구현 (서버 측)
//!
//! `org.atit.unim.InputMethod` 및 `org.atit.unim.InputContext` 서비스를 구현합니다.
//!
//! # 아키텍처 노트
//!
//! `InputEngine`은 `Send + Sync`를 구현하지 않으므로 (HangulComposer trait object),
//! 엔진은 별도의 전용 스레드에서 실행하고 채널을 통해 통신합니다.

use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;

use tokio::sync::{mpsc, oneshot, RwLock};
use zbus::{interface, Connection, SignalContext};

use crate::interfaces::InputMode;
use unim::config::{Config, InputCategory, PopupMode};
use unim::input_engine::PopupAction;
use unim::unim_log;

// PopupAction은 unim::input_engine에서 정의됨 (re-export)

/// 엔진에 보내는 요청
#[derive(Debug)]
pub enum EngineRequest {
    /// 키 이벤트 처리
    ProcessKey {
        context_id: u32,
        keyval: u32,
        keycode: u32,
        state: u32,
        response: oneshot::Sender<EngineResponse>,
    },
    /// 컨텍스트 생성
    CreateContext {
        id: u32,
        window_id: String,
        response: oneshot::Sender<()>,
    },
    /// 컨텍스트 파괴
    DestroyContext { id: u32 },
    /// 포커스 인 (모드 조회용)
    FocusIn {
        context_id: u32,
        window_id: String,
        response: oneshot::Sender<bool>,
    },
    /// 포커스 아웃 (preedit 플러시)
    FocusOut {
        context_id: u32,
        response: oneshot::Sender<Option<String>>,
    },
    /// 리셋 (팝업 취소 + 조합 커밋)
    Reset {
        context_id: u32,
        response: oneshot::Sender<Option<String>>,
    },
    /// 전역 모드 설정 (모든 컨텍스트에 적용)
    SetGlobalMode { is_korean: bool },
    /// 한자 후보 조회
    GetHanjaCandidates {
        context_id: u32,
        response: oneshot::Sender<HanjaCandidateResponse>,
    },
    /// 한자 선택
    SelectHanja {
        context_id: u32,
        index: usize,
        response: oneshot::Sender<Option<String>>,
    },
    /// 한자 모드 취소 (남은 preedit을 반환)
    CancelHanja {
        context_id: u32,
        response: oneshot::Sender<Option<String>>,
    },
    /// 한자 즐겨찾기 상태 조회 (현재 후보 목록에 대응)
    GetHanjaBookmarkStates {
        context_id: u32,
        response: oneshot::Sender<Vec<bool>>,
    },
    /// 한자 즐겨찾기 토글
    ///
    /// 응답: `(new_index, bookmarked, popup_action, render_state)`. popup_action 은
    /// 보통 `HanjaCandidatesReordered` 이며 RPC 측에서 legacy 시그널로 발행한다.
    /// `render_state` 는 토글 후 view_model 스냅샷 — popup_render 시그널로도 발행.
    ToggleHanjaBookmark {
        context_id: u32,
        index: usize,
        response: oneshot::Sender<
            Option<(usize, bool, Option<PopupAction>, Option<PopupRenderPayload>)>,
        >,
    },
    /// 특수문자 후보 조회
    GetSpecialCharCandidates {
        context_id: u32,
        response: oneshot::Sender<SpecialCharResponse>,
    },
    /// 특수문자 선택
    SelectSpecialChar {
        context_id: u32,
        index: usize,
        response: oneshot::Sender<Option<String>>,
    },
    /// 특수문자 모드 취소 (남은 preedit을 반환)
    CancelSpecialChar {
        context_id: u32,
        response: oneshot::Sender<Option<String>>,
    },
    /// 커서 위치 보고 (프런트엔드 → 데몬)
    ReportCursorRect {
        context_id: u32,
        x: i32,
        y: i32,
        width: i32,
        height: i32,
    },
    /// 입력 필드 목적 설정 (비밀번호/PIN 등)
    SetContentType { context_id: u32, purpose: u32 },
    /// Surrounding text 설정
    SetSurroundingText {
        context_id: u32,
        text: String,
        cursor_pos: u32,
        anchor_pos: u32,
    },
    /// 글로벌 TypeFix 변환 요청 (마지막 포커스된 컨텍스트 대상)
    GlobalTypeFix {
        direction: u32,
        response: oneshot::Sender<Option<(i32, u32, String)>>,
    },
    /// Smart Backspace 요청
    SmartBackspace {
        context_id: u32,
        response: oneshot::Sender<Option<(u32, String)>>,
    },
    /// 역방향 사용자 사전: 단어 추가
    UserDictAdd {
        word: String,
        note: String,
        response: oneshot::Sender<bool>,
    },
    /// 역방향 사용자 사전: 단어 제거 (대소문자 무시)
    UserDictRemove {
        word: String,
        response: oneshot::Sender<bool>,
    },
    /// 역방향 사용자 사전: 전체 목록 조회
    UserDictList {
        response: oneshot::Sender<Vec<(String, String, u64)>>,
    },
    /// 역방향 사용자 사전: 인덱스 위치 엔트리 갱신
    UserDictUpdate {
        index: u32,
        word: String,
        note: String,
        response: oneshot::Sender<bool>,
    },
    /// 단축키 진입점: 마지막 포커스 컨텍스트의 선택 영역을 읽어 사용자 사전에 등록.
    /// 선택 텍스트가 한글이면 `kor_to_eng`로 영문 변환 후 등록,
    /// 이미 알파벳이면 그대로 등록.
    /// 반환값: 실제 등록된 영문 단어 (빈 문자열이면 실패/선택 없음).
    RegisterUserDictFromSelection { response: oneshot::Sender<String> },
    /// 이모지 팝업 카테고리 변경 (마우스 클릭 → 직접 카테고리 전환).
    ///
    /// 워커는 마지막 포커스 컨텍스트의 popup_state 가 emoji 인 경우에만 처리한다.
    /// 응답으로 갱신된 카테고리 메타·페이지 풀을 돌려주어 service.rs 가
    /// `ShowEmojiPopupV2` 시그널을 재발행할 수 있게 한다.
    /// 응답이 `None` 이면 emoji popup 이 활성화되지 않았거나 idx 가 범위 밖이라 무시됨.
    SetEmojiCategory {
        idx: u32,
        response: oneshot::Sender<Option<EmojiShowPayload>>,
    },
    /// 팝업 페이지 이동 (마우스 ◀/▶ 버튼 등 외부 RPC).
    ///
    /// `direction`: -1 또는 0 = 이전 페이지, 그 외(+1 등) = 다음 페이지. wrap-around.
    /// 응답: 페이지가 바뀌면 `Some(PopupRenderPayload)` (전체 view_model 스냅샷,
    /// service.rs 가 popup_navigate + popup_render 두 시그널로 변환). 단일
    /// 페이지·popup 비활성·context 부재 시 `None`.
    PopupChangePage {
        context_id: u32,
        direction: i32,
        response: oneshot::Sender<Option<PopupRenderPayload>>,
    },

    /// 한자 popup 확장 모드 토글 (마우스 ⊞/⊟ 아이콘 클릭).
    ///
    /// popup-owner context 로 라우팅 (GNOME extension·gui-gtk 처럼 popup 이 다른
    /// context 에 살 때 호출 context 기준이 아닌 popup_state 가 살아있는 context 를
    /// 찾아 toggle_hanja_expanded() 를 적용). 응답: 전체 `PopupRenderPayload` —
    /// service.rs 가 popup_navigate + popup_render 시그널로 변환.
    /// 활성 한자 popup 이 없으면 `None`.
    TogglePopupExpand {
        context_id: u32,
        response: oneshot::Sender<Option<PopupRenderPayload>>,
    },
}

/// 팝업 페이지 이동 응답 — service.rs 가 `PopupNavigate` 시그널 페이로드로 변환.
#[derive(Debug, Clone, Copy)]
pub struct PopupNavigatePayload {
    pub page: u32,
    pub total_pages: u32,
    pub selected: u32,
    pub rows: u32,
    pub cols: u32,
    pub sel_row: u32,
    pub sel_col: u32,
}

/// 이모지 카테고리 전환 응답 — `ShowEmojiPopupV2` 시그널 재발행용 payload.
#[derive(Debug, Clone)]
pub struct EmojiShowPayload {
    /// 새 활성 카테고리 id (예: "SmileysPeople").
    pub target_cat_id: String,
    /// 해당 카테고리 emoji 풀 전체 (페이지 슬라이싱은 GUI 측 책임).
    pub items: Vec<String>,
    /// 현재 활성 영문 키맵의 상단 행 9 문자.
    pub top_row: String,
    /// MRU Recent 캐시 (popup payload 호환용).
    pub recent: Vec<String>,
    /// 카테고리 메타 9 튜플 (id, ko, en, total).
    pub categories: Vec<(String, String, String, u32)>,
    /// 현재 활성 영문 키맵의 홈 행 9 문자 (이모지 카테고리 단축키 표시용).
    pub home_row: String,
}

/// `PopupRender` 시그널 페이로드 — engine `PopupViewModel` 의 DBus-friendly 평면 표현.
///
/// daemon = SoT. 두 frontend (GNOME extension / unim-gui-gtk) 가 본 페이로드를 그대로
/// 렌더링하여 헤더·푸터·셀·탭·확장 아이콘 등을 표시한다. 서식 문자열·visibility 조건이
/// 모두 daemon 산출이라 frontend 간 표시 일관성 자동 보장.
#[derive(Debug, Clone)]
pub struct PopupRenderPayload {
    /// 0=Hanja, 1=SpecialChar, 2=Emoji
    pub kind: u32,
    pub target: String,
    pub header_text: String,
    pub footer_text: String,
    pub show_footer: bool,
    pub rows: u32,
    pub cols: u32,
    pub sel_row: u32,
    pub sel_col: u32,
    pub current_page: u32,
    pub total_pages: u32,
    /// 셀 데이터 — column-major, 길이 = rows * cols. 각 (text, meaning, flags).
    /// flags 비트: 0x01=has_data, 0x02=selected, 0x04=col_highlight,
    ///            0x08=row_highlight, 0x10=bookmarked.
    /// has_data=0 이면 빈 셀 (text/meaning 무시).
    pub cells: Vec<(String, String, u32)>,
    /// 컬럼 헤더 (text, is_active) — 한자 compact 는 빈 벡터.
    pub col_headers: Vec<(String, bool)>,
    /// 행 헤더 (text, is_active).
    pub row_headers: Vec<(String, bool)>,
    /// 한자 expand 토글 아이콘 가시성 (한자 popup 만 true).
    pub expand_visible: bool,
    /// 한자 expand 토글 텍스트 ("⊞" / "⊟").
    pub expand_text: String,
    /// 이모지 좌측 9 카테고리 탭 라벨 (단축키 prefix 포함). 그 외 popup 은 빈 벡터.
    pub tab_labels: Vec<String>,
    /// 이모지 활성 탭 인덱스 (0..9).
    pub active_tab_index: u32,
}

/// PopupRenderPayload 의 cell flag 비트 정의.
pub mod popup_render_flags {
    pub const HAS_DATA: u32 = 0x01;
    pub const SELECTED: u32 = 0x02;
    pub const COL_HIGHLIGHT: u32 = 0x04;
    pub const ROW_HIGHLIGHT: u32 = 0x08;
    pub const BOOKMARKED: u32 = 0x10;
}

/// 한자 후보 응답
#[derive(Debug)]
pub struct HanjaCandidateResponse {
    /// 변환 대상 문자열
    pub target: String,
    /// 후보 목록 (한자, 뜻풀이)
    pub candidates: Vec<(String, String)>,
    /// 영문 키맵의 상단 행 레이블 (expanded 9x9 컬럼 헤더용; 특수문자와 동일 source).
    pub top_row: String,
    /// PopupRender 페이로드 — Standalone 모드에서 ShowHanjaPopup 직후 popup_render
    /// 시그널 발행에 사용. popup 비활성이면 None.
    pub render_state: Option<PopupRenderPayload>,
}

/// 특수문자 후보 응답
#[derive(Debug)]
pub struct SpecialCharResponse {
    /// 변환 대상 초성
    pub target: String,
    /// 특수문자 목록
    pub characters: Vec<String>,
    /// 영문 키맵의 상단 행 레이블 (예: "QWERTYUIO")
    pub top_row: String,
    /// PopupRender 페이로드 — Standalone 모드에서 ShowSpecialPopup 직후 popup_render
    /// 시그널 발행에 사용. popup 비활성이면 None.
    pub render_state: Option<PopupRenderPayload>,
}

/// 엔진 응답
#[derive(Debug)]
pub struct EngineResponse {
    /// 키가 소비되었는지
    pub consumed: bool,
    /// preedit 텍스트 (변경된 경우)
    pub preedit: Option<String>,
    /// 커밋 텍스트 (있는 경우)
    pub commit: Option<String>,
    /// 모드 변경됨 (Some(true) = 한국어, Some(false) = 영어, None = 변경 없음)
    pub mode_changed: Option<bool>,
    /// 팝업 동작 (한자/특수문자 팝업 제어)
    pub popup_action: Option<PopupAction>,
    /// AutoTypeFix 교정 결과 (삭제할 문자 수, commit 텍스트, preedit 텍스트)
    pub auto_typefix: Option<(u32, String, String)>,
    /// PopupRender 페이로드 — popup 활성 상태일 때 매 키 처리 후 산출되어 frontend
    /// 가 시각 갱신에 사용. popup 비활성이면 None.
    pub render_state: Option<PopupRenderPayload>,
}

/// InputMethod 서비스 (팩토리 역할)
pub struct InputMethodService {
    /// 컨텍스트 카운터
    context_counter: AtomicU32,
    /// 설정
    config: Arc<RwLock<Config>>,
    /// 전역 입력 모드
    global_mode: Arc<RwLock<InputMode>>,
    /// 엔진 스레드로 요청을 보내는 채널
    engine_tx: mpsc::Sender<EngineRequest>,
    /// DBus Connection (동적 객체 등록용)
    connection: Connection,
    /// 전역 캐시: 가장 최근 보고된 커서 위치 (x, y, width, height)
    ///
    /// `InputContextHandler::report_cursor_rect`가 호출될 때마다 컨텍스트별 캐시와
    /// 함께 갱신된다. InputMethod-level `trigger_action`("emoji_popup") 처럼
    /// 어느 컨텍스트인지 알 수 없는 외부 단축키 우회 호출에서 fallback 위치로 사용한다.
    /// 한 번도 보고가 없으면 `(0, 0, 0, 0)` — GUI 측이 자체 fallback 위치를 결정.
    last_cursor_rect: Arc<std::sync::Mutex<(i32, i32, i32, i32)>>,
    /// 전역 캐시: 마지막으로 포커스를 받은 **실제 입력 컨텍스트**의 DBus path.
    ///
    /// GNOME extension의 자체 InputContext(`client_name = "gnome-extension"`)는
    /// 사용자 앱이 아닌 extension 자신의 컨텍스트이므로 본 캐시에서 **제외**된다.
    /// GTK3/4·Qt5/6·XIM·Wayland 등 사용자 앱 측 컨텍스트만 후보가 된다.
    ///
    /// 글로벌 `TriggerAction("emoji_popup")` 및 `CommitEmoji` 호출 시 데몬은 이
    /// path를 향해 시그널을 redirect한다. GTK4_IM 모듈이 캐시한 ACTIVE_CONTEXT_PATH와
    /// 일치시켜, 단축키 캡처 주체(GNOME extension)와 입력 주체(GTK4_IM)가 분리된
    /// 환경에서도 emoji가 사용자 앱에 정확히 들어가게 한다.
    last_active_input_context_path: Arc<std::sync::Mutex<Option<String>>>,
}

impl InputMethodService {
    /// 새 서비스 생성
    ///
    /// `engine_tx`는 엔진 스레드와 통신하기 위한 채널의 송신 측입니다.
    /// `connection`은 동적으로 InputContext 객체를 등록하기 위해 필요합니다.
    pub fn new(
        config: Config,
        engine_tx: mpsc::Sender<EngineRequest>,
        connection: Connection,
    ) -> Self {
        let global_mode = InputMode::from(config.engine.default_category);
        Self {
            context_counter: AtomicU32::new(0),
            config: Arc::new(RwLock::new(config)),
            global_mode: Arc::new(RwLock::new(global_mode)),
            engine_tx,
            connection,
            last_cursor_rect: Arc::new(std::sync::Mutex::new((0, 0, 0, 0))),
            last_active_input_context_path: Arc::new(std::sync::Mutex::new(None)),
        }
    }

    /// 전역 cursor_rect 캐시 핸들 (InputContextHandler에 공유 주입)
    pub fn last_cursor_rect(&self) -> Arc<std::sync::Mutex<(i32, i32, i32, i32)>> {
        Arc::clone(&self.last_cursor_rect)
    }

    /// 전역 last_active_input_context_path 캐시 핸들
    pub fn last_active_input_context_path(&self) -> Arc<std::sync::Mutex<Option<String>>> {
        Arc::clone(&self.last_active_input_context_path)
    }

    /// 설정 참조 반환
    pub fn config(&self) -> Arc<RwLock<Config>> {
        Arc::clone(&self.config)
    }

    /// 전역 모드 참조 반환
    pub fn global_mode(&self) -> Arc<RwLock<InputMode>> {
        Arc::clone(&self.global_mode)
    }

    /// 엔진 채널 복제
    pub fn engine_channel(&self) -> mpsc::Sender<EngineRequest> {
        self.engine_tx.clone()
    }
}

#[interface(name = "org.atit.unim.InputMethod")]
impl InputMethodService {
    /// 새 입력 컨텍스트 생성 (window_id: 창 식별자, 빈 문자열이면 client_name 사용)
    async fn create_input_context(
        &self,
        client_name: &str,
        window_id: &str,
    ) -> zbus::fdo::Result<String> {
        let id = self.context_counter.fetch_add(1, Ordering::SeqCst) + 1;
        let path = format!("{}{}", crate::INPUT_CONTEXT_PATH_PREFIX, id);

        // window_id가 비어있으면 client_name을 사용
        let effective_window_id = if window_id.is_empty() {
            client_name.to_string()
        } else {
            window_id.to_string()
        };

        // 엔진 스레드에 컨텍스트 생성 요청
        let (response_tx, response_rx) = oneshot::channel();
        self.engine_tx
            .send(EngineRequest::CreateContext {
                id,
                window_id: effective_window_id,
                response: response_tx,
            })
            .await
            .map_err(|e| zbus::fdo::Error::Failed(format!("Engine not available: {}", e)))?;

        response_rx
            .await
            .map_err(|_| zbus::fdo::Error::Failed("Engine response failed".to_string()))?;

        // InputContext 핸들러를 DBus에 등록
        let handler = InputContextHandler::new(
            id,
            client_name.to_string(),
            path.clone(),
            self.engine_tx.clone(),
            self.connection.clone(),
            Arc::clone(&self.last_cursor_rect),
            Arc::clone(&self.last_active_input_context_path),
        );
        let obj_path = zbus::zvariant::ObjectPath::try_from(path.as_str())
            .map_err(|e| zbus::fdo::Error::Failed(format!("Invalid path: {}", e)))?;
        self.connection
            .object_server()
            .at(obj_path, handler)
            .await
            .map_err(|e| {
                zbus::fdo::Error::Failed(format!("Failed to register InputContext: {}", e))
            })?;

        unim_log!(
            "DBUS",
            "[DBus] InputContext 생성 및 등록: {} (client: {}, window: {})",
            path,
            client_name,
            window_id
        );
        Ok(path)
    }

    /// 전역 입력 모드 설정
    async fn set_global_mode(
        &self,
        #[zbus(signal_context)] signal_ctx: SignalContext<'_>,
        is_korean: bool,
    ) -> zbus::fdo::Result<()> {
        let new_mode = if is_korean {
            InputMode::Korean
        } else {
            InputMode::English
        };

        {
            let mut mode = self.global_mode.write().await;
            *mode = new_mode;
        }

        // 설정에도 반영
        {
            let mut config = self.config.write().await;
            config.engine.default_category = InputCategory::from(new_mode);
        }

        // 시그널 전송
        Self::global_mode_changed(&signal_ctx, is_korean).await?;

        // 엔진 워커에 모드 변경 전달 (모든 컨텍스트에 적용)
        self.engine_tx
            .send(EngineRequest::SetGlobalMode { is_korean })
            .await
            .ok();

        unim_log!("DBUS", "[DBus] 전역 모드 변경: {:?}", new_mode);
        Ok(())
    }

    /// 전역 입력 모드 조회
    async fn get_global_mode(&self) -> zbus::fdo::Result<bool> {
        let mode = self.global_mode.read().await;
        Ok(*mode == InputMode::Korean)
    }

    /// 전역 모드 변경 시그널
    #[zbus(signal)]
    async fn global_mode_changed(
        signal_ctx: &SignalContext<'_>,
        is_korean: bool,
    ) -> zbus::Result<()>;

    /// 설정 변경 시그널 (레거시 단일 키)
    #[zbus(signal)]
    async fn config_changed(
        signal_ctx: &SignalContext<'_>,
        key: &str,
        value: &str,
    ) -> zbus::Result<()>;

    /// 설정 전체 변경 시그널 (YAML 통짜 수신 → JSON 브로드캐스트)
    ///
    /// payload는 전체 Config의 JSON 직렬화. GNOME extension / GTK GUI /
    /// Qt GUI 등 모든 클라이언트가 이 한 번의 시그널로 전 설정을 갱신한다.
    #[zbus(signal)]
    async fn config_changed_json(signal_ctx: &SignalContext<'_>, json: &str) -> zbus::Result<()>;

    /// 글로벌 TypeFix 변환 (마지막 포커스된 컨텍스트 대상)
    /// direction: 0=자동, 1=영→한, 2=한→영
    /// 반환값: (커서로부터의 오프셋, 삭제할 문자 수, 대체 텍스트) 또는 (0, 0, "")
    /// - offset_from_cursor: 음수=커서 앞, 0=현재/뒤에서 삭제 시작
    async fn type_fix(&self, direction: u32) -> zbus::fdo::Result<(i32, u32, String)> {
        let (response_tx, response_rx) = oneshot::channel();

        self.engine_tx
            .send(EngineRequest::GlobalTypeFix {
                direction,
                response: response_tx,
            })
            .await
            .map_err(|_| zbus::fdo::Error::Failed("Engine not available".to_string()))?;

        let result = response_rx
            .await
            .map_err(|_| zbus::fdo::Error::Failed("Engine response failed".to_string()))?;

        if let Some((offset_from_cursor, delete_chars, replacement)) = result {
            unim_log!(
                "DBUS",
                "[DBus] GlobalTypeFix: offset={}, delete={}, replacement='{}'",
                offset_from_cursor,
                delete_chars,
                replacement
            );
            Ok((offset_from_cursor, delete_chars, replacement))
        } else {
            Ok((0_i32, 0_u32, String::new()))
        }
    }

    /// 역방향 사용자 사전: 단어 추가
    /// word는 영문 알파벳만 허용. note는 설명(빈 문자열이면 미부여).
    /// 성공 시 true, 중복/빈값/비영문이면 false.
    async fn add_reverse_user_dict_word(&self, word: &str, note: &str) -> zbus::fdo::Result<bool> {
        let (tx, rx) = oneshot::channel();
        self.engine_tx
            .send(EngineRequest::UserDictAdd {
                word: word.to_string(),
                note: note.to_string(),
                response: tx,
            })
            .await
            .map_err(|_| zbus::fdo::Error::Failed("Engine not available".to_string()))?;
        let ok = rx
            .await
            .map_err(|_| zbus::fdo::Error::Failed("Engine response failed".to_string()))?;
        unim_log!(
            "DBUS",
            "[DBus] AddReverseUserDictWord: word='{}', ok={}",
            word,
            ok
        );
        Ok(ok)
    }

    /// 역방향 사용자 사전: 단어 제거 (대소문자 무시). 성공 시 true.
    async fn remove_reverse_user_dict_word(&self, word: &str) -> zbus::fdo::Result<bool> {
        let (tx, rx) = oneshot::channel();
        self.engine_tx
            .send(EngineRequest::UserDictRemove {
                word: word.to_string(),
                response: tx,
            })
            .await
            .map_err(|_| zbus::fdo::Error::Failed("Engine not available".to_string()))?;
        let ok = rx
            .await
            .map_err(|_| zbus::fdo::Error::Failed("Engine response failed".to_string()))?;
        unim_log!(
            "DBUS",
            "[DBus] RemoveReverseUserDictWord: word='{}', ok={}",
            word,
            ok
        );
        Ok(ok)
    }

    /// 역방향 사용자 사전: 전체 목록 조회 (word, note, added_at).
    async fn list_reverse_user_dict_words(&self) -> zbus::fdo::Result<Vec<(String, String, u64)>> {
        let (tx, rx) = oneshot::channel();
        self.engine_tx
            .send(EngineRequest::UserDictList { response: tx })
            .await
            .map_err(|_| zbus::fdo::Error::Failed("Engine not available".to_string()))?;
        let list = rx
            .await
            .map_err(|_| zbus::fdo::Error::Failed("Engine response failed".to_string()))?;
        Ok(list)
    }

    /// 역방향 사용자 사전: idx 위치 엔트리를 새 word/note로 갱신.
    async fn update_reverse_user_dict_word(
        &self,
        index: u32,
        word: &str,
        note: &str,
    ) -> zbus::fdo::Result<bool> {
        let (tx, rx) = oneshot::channel();
        self.engine_tx
            .send(EngineRequest::UserDictUpdate {
                index,
                word: word.to_string(),
                note: note.to_string(),
                response: tx,
            })
            .await
            .map_err(|_| zbus::fdo::Error::Failed("Engine not available".to_string()))?;
        let ok = rx
            .await
            .map_err(|_| zbus::fdo::Error::Failed("Engine response failed".to_string()))?;
        Ok(ok)
    }

    /// 단축키 진입점: 마지막 포커스 컨텍스트의 선택 영역을 읽어 사용자 사전에 등록.
    /// 반환값: 등록된 영문 단어 (선택 없음/이미 등록됨/비유효면 빈 문자열).
    async fn register_user_dict_from_selection(&self) -> zbus::fdo::Result<String> {
        let (tx, rx) = oneshot::channel();
        self.engine_tx
            .send(EngineRequest::RegisterUserDictFromSelection { response: tx })
            .await
            .map_err(|_| zbus::fdo::Error::Failed("Engine not available".to_string()))?;
        let word = rx
            .await
            .map_err(|_| zbus::fdo::Error::Failed("Engine response failed".to_string()))?;
        unim_log!(
            "DBUS",
            "[DBus] RegisterUserDictFromSelection: word='{}'",
            word
        );
        Ok(word)
    }

    /// 이모지 최근 사용(MRU) 조회 (PR #1 신규 RPC).
    ///
    /// `~/.config/unim/emoji-recent.yaml` 에서 사용자별 MRU (최대 81개) 를 읽어
    /// 반환한다. 일반적으로는 `ShowEmojiPopupV2` 시그널의 `recent` payload 가
    /// frontend 에 푸시되므로 이 RPC 를 호출할 필요가 없지만, GUI 가 명시적
    /// refresh 가 필요할 때 사용한다.
    async fn get_emoji_recent(&self) -> zbus::fdo::Result<Vec<String>> {
        Ok(unim::emoji::load_recent())
    }

    /// 설정값 조회
    async fn get_config(&self, key: &str) -> zbus::fdo::Result<String> {
        let config = self.config.read().await;
        let value = match key {
            "korean_layout" => config.engine.korean.layout.clone(),
            "english_layout" => config.engine.english.layout.clone(),
            "default_category" => match config.engine.default_category {
                InputCategory::Korean => "Korean".to_string(),
                InputCategory::English => "English".to_string(),
            },
            "mode_sharing" => match config.engine.mode_sharing {
                unim::config::ModeSharingMode::Global => "Global".to_string(),
                unim::config::ModeSharingMode::PerApp => "PerApp".to_string(),
            },
            "toggle_keys" => config.engine.toggle_keys.join(","),
            "hanja_keys" => config.engine.hanja_keys.join(","),
            "korean_active_rule_sets" => match &config.engine.korean.active_rule_sets {
                // None → "default" sentinel (프로필 기본값 사용)
                None => "default".to_string(),
                // Some(vec![]) → "" (사용자 명시 All OFF)
                // Some(list)   → "a,b,c"
                Some(list) => list.join(","),
            },
            // Phase 8: korean_custom_layout 필드 폐지. korean_layout이 이제 프로필 이름
            // 문자열을 직접 담는다. 호환성 — 구 클라이언트가 이 키로 조회 시 동일값 반환.
            "korean_custom_layout" => config.engine.korean.layout.clone(),
            "popup_mode" => config.engine.popup_mode.name().to_string(),
            "auto_typefix" => config.engine.auto_typefix.enabled.to_string(),
            "auto_typefix_forward" => config.engine.auto_typefix.forward.to_string(),
            "auto_typefix_reverse" => config.engine.auto_typefix.reverse.to_string(),
            "auto_typefix_kor_threshold" => {
                config.engine.auto_typefix.kor_syllable_threshold.to_string()
            }
            "auto_typefix_eng_min_length" => {
                config.engine.auto_typefix.eng_word_min_length.to_string()
            }
            "auto_typefix_forward_time_window_ms" => {
                config.engine.auto_typefix.forward_time_window_ms.to_string()
            }
            "auto_typefix_reverse_time_window_ms" => {
                config.engine.auto_typefix.reverse_time_window_ms.to_string()
            }
            "auto_typefix_skip_on_english_word" => {
                config.engine.auto_typefix.skip_on_english_word.to_string()
            }
            "auto_typefix_skip_on_complete_syllable" => {
                config.engine.auto_typefix.skip_on_complete_syllable.to_string()
            }
            "auto_typefix_rollback_detection" => {
                config.engine.auto_typefix.rollback_detection.to_string()
            }
            "auto_typefix_tentative_expiry_hours" => {
                config.engine.auto_typefix.tentative_expiry_hours.to_string()
            }
            "auto_typefix_observation_timeout_secs" => {
                config.engine.auto_typefix.observation_timeout_secs.to_string()
            }
            "auto_typefix_user_dict_enabled" => {
                config.engine.auto_typefix.user_dict_enabled.to_string()
            }
            "auto_english" => config.engine.auto_english.enabled.to_string(),
            "auto_english_keys" => config.engine.auto_english.trigger_keys.join(","),
            "emoji_popup" => config.engine.emoji_popup.enabled.to_string(),
            "app_rules" => serde_json::to_string(&config.engine.app_rules).unwrap_or_default(),
            _ => {
                return Err(zbus::fdo::Error::InvalidArgs(format!(
                    "Unknown key: {}",
                    key
                )))
            }
        };
        Ok(value)
    }

    /// 설정값 변경 및 시그널 브로드캐스트
    async fn set_config(
        &self,
        #[zbus(signal_context)] signal_ctx: SignalContext<'_>,
        key: &str,
        value: &str,
    ) -> zbus::fdo::Result<()> {
        {
            let mut config = self.config.write().await;
            match key {
                "korean_layout" => {
                    // Phase 8: 프로필 이름 문자열을 직접 수용. 레거시 enum 이름·별칭은
                    // normalize_korean_layout_name()이 정식 이름으로 승격.
                    let trimmed = value.trim();
                    if trimmed.is_empty() {
                        return Err(zbus::fdo::Error::InvalidArgs(
                            "korean_layout cannot be empty".to_string(),
                        ));
                    }
                    // GTK/CLI와 동일한 캐시 동작 — 이전 자판의 active_rule_sets 보존,
                    // 새 자판의 캐시된 값(있으면) 복원. switch_layout이 자판 정규화도 수행.
                    config.engine.korean.switch_layout(trimmed, None);
                }
                "english_layout" => {
                    // Phase 9: 프로필 이름 문자열 직접 수용. 레거시 enum 이름·소문자 별칭
                    // 모두 normalize_english_layout_name이 정식 이름으로 승격.
                    let trimmed = value.trim();
                    if trimmed.is_empty() {
                        return Err(zbus::fdo::Error::InvalidArgs(
                            "english_layout cannot be empty".to_string(),
                        ));
                    }
                    config.engine.english.layout =
                        unim::config::normalize_english_layout_name(trimmed);
                }
                "default_category" => {
                    config.engine.default_category = match value {
                        "Korean" => InputCategory::Korean,
                        "English" => InputCategory::English,
                        _ => {
                            return Err(zbus::fdo::Error::InvalidArgs(format!(
                                "Invalid value: {}",
                                value
                            )))
                        }
                    };
                }
                "mode_sharing" => {
                    config.engine.mode_sharing = match value {
                        "Global" => unim::config::ModeSharingMode::Global,
                        "PerApp" => unim::config::ModeSharingMode::PerApp,
                        _ => {
                            return Err(zbus::fdo::Error::InvalidArgs(format!(
                                "Invalid value: {}",
                                value
                            )))
                        }
                    };
                }
                "toggle_keys" => {
                    let keys: Vec<String> = value
                        .split(',')
                        .map(|s| s.trim().to_string())
                        .filter(|s| !s.is_empty())
                        .collect();
                    if keys.is_empty() {
                        return Err(zbus::fdo::Error::InvalidArgs(
                            "At least one key required".to_string(),
                        ));
                    }
                    config.engine.toggle_keys = keys;
                }
                "hanja_keys" => {
                    let keys: Vec<String> = value
                        .split(',')
                        .map(|s| s.trim().to_string())
                        .filter(|s| !s.is_empty())
                        .collect();
                    if keys.is_empty() {
                        return Err(zbus::fdo::Error::InvalidArgs(
                            "At least one key required".to_string(),
                        ));
                    }
                    config.engine.hanja_keys = keys;
                }
                "korean_active_rule_sets" => {
                    // value 의미 (CLI ConfigKey::KoreanActiveRuleSets와 동치):
                    //   "default" → None (프로필 기본값 사용)
                    //   ""        → Some(vec![]) (사용자 명시 All OFF)
                    //   "a,b,c"   → Some(vec!["a","b","c"])
                    let trimmed = value.trim();
                    if trimmed.eq_ignore_ascii_case("default") {
                        config.engine.korean.active_rule_sets = None;
                    } else {
                        let names: Vec<String> = trimmed
                            .split(',')
                            .map(|s| s.trim().to_string())
                            .filter(|s| !s.is_empty())
                            .collect();
                        config.engine.korean.active_rule_sets = Some(names);
                    }
                    // 현재 자판의 캐시 동기화 — 다음 자판 전환 시 본 상태가 보존된다.
                    config.engine.korean.cache_active_rule_sets();
                }
                "korean_custom_layout" => {
                    // Phase 8 호환: custom_layout 필드는 폐지되고 korean_layout이 문자열
                    // 이므로 이 키도 korean_layout을 직접 업데이트한다. 빈 문자열은
                    // 기본(ko_2bulstd)으로 복귀.
                    let trimmed = value.trim();
                    let target = if trimmed.is_empty() {
                        unim::config::KOREAN_LAYOUT_DUBEOLSIK.to_string()
                    } else {
                        trimmed.to_string()
                    };
                    // korean_layout과 동일한 캐시 동작.
                    config.engine.korean.switch_layout(&target, None);
                }
                "popup_mode" => {
                    config.engine.popup_mode = match value {
                        "Standalone" => unim::config::PopupMode::Standalone,
                        "Embedded" => unim::config::PopupMode::Embedded,
                        _ => {
                            return Err(zbus::fdo::Error::InvalidArgs(format!(
                                "Invalid value: {}",
                                value
                            )))
                        }
                    };
                }
                "auto_typefix" => {
                    config.engine.auto_typefix.enabled = value
                        .parse()
                        .map_err(|_| zbus::fdo::Error::InvalidArgs("Invalid bool".to_string()))?;
                }
                "auto_typefix_forward" => {
                    config.engine.auto_typefix.forward = value
                        .parse()
                        .map_err(|_| zbus::fdo::Error::InvalidArgs("Invalid bool".to_string()))?;
                }
                "auto_typefix_reverse" => {
                    config.engine.auto_typefix.reverse = value
                        .parse()
                        .map_err(|_| zbus::fdo::Error::InvalidArgs("Invalid bool".to_string()))?;
                }
                "auto_typefix_kor_threshold" => {
                    let v: u8 = value.parse().map_err(|_| {
                        zbus::fdo::Error::InvalidArgs(format!("Invalid number: {}", value))
                    })?;
                    config.engine.auto_typefix.kor_syllable_threshold = v;
                    config.engine.auto_typefix.clamp_ranges();
                }
                "auto_typefix_eng_min_length" => {
                    let v: u8 = value.parse().map_err(|_| {
                        zbus::fdo::Error::InvalidArgs(format!("Invalid number: {}", value))
                    })?;
                    config.engine.auto_typefix.eng_word_min_length = v;
                    config.engine.auto_typefix.clamp_ranges();
                }
                "auto_typefix_forward_time_window_ms" => {
                    let v: u32 = value.parse().map_err(|_| {
                        zbus::fdo::Error::InvalidArgs(format!("Invalid number: {}", value))
                    })?;
                    config.engine.auto_typefix.forward_time_window_ms = v;
                    config.engine.auto_typefix.clamp_ranges();
                }
                "auto_typefix_reverse_time_window_ms" => {
                    let v: u32 = value.parse().map_err(|_| {
                        zbus::fdo::Error::InvalidArgs(format!("Invalid number: {}", value))
                    })?;
                    config.engine.auto_typefix.reverse_time_window_ms = v;
                    config.engine.auto_typefix.clamp_ranges();
                }
                "auto_typefix_skip_on_english_word" => {
                    config.engine.auto_typefix.skip_on_english_word = value
                        .parse()
                        .map_err(|_| zbus::fdo::Error::InvalidArgs("Invalid bool".to_string()))?;
                }
                "auto_typefix_skip_on_complete_syllable" => {
                    config.engine.auto_typefix.skip_on_complete_syllable = value
                        .parse()
                        .map_err(|_| zbus::fdo::Error::InvalidArgs("Invalid bool".to_string()))?;
                }
                "auto_typefix_rollback_detection" => {
                    config.engine.auto_typefix.rollback_detection = value
                        .parse()
                        .map_err(|_| zbus::fdo::Error::InvalidArgs("Invalid bool".to_string()))?;
                }
                "auto_typefix_tentative_expiry_hours" => {
                    let v: u16 = value.parse().map_err(|_| {
                        zbus::fdo::Error::InvalidArgs(format!("Invalid number: {}", value))
                    })?;
                    config.engine.auto_typefix.tentative_expiry_hours = v;
                    config.engine.auto_typefix.clamp_ranges();
                }
                "auto_typefix_observation_timeout_secs" => {
                    let v: u8 = value.parse().map_err(|_| {
                        zbus::fdo::Error::InvalidArgs(format!("Invalid number: {}", value))
                    })?;
                    config.engine.auto_typefix.observation_timeout_secs = v;
                    config.engine.auto_typefix.clamp_ranges();
                }
                "auto_typefix_user_dict_enabled" => {
                    config.engine.auto_typefix.user_dict_enabled = value
                        .parse()
                        .map_err(|_| zbus::fdo::Error::InvalidArgs("Invalid bool".to_string()))?;
                }
                "auto_english" => {
                    config.engine.auto_english.enabled = value
                        .parse()
                        .map_err(|_| zbus::fdo::Error::InvalidArgs("Invalid bool".to_string()))?;
                }
                "auto_english_keys" => {
                    let keys: Vec<String> = value
                        .split(',')
                        .map(|s| s.trim().to_string())
                        .filter(|s| !s.is_empty())
                        .collect();
                    if keys.is_empty() {
                        return Err(zbus::fdo::Error::InvalidArgs(
                            "At least one key required".to_string(),
                        ));
                    }
                    config.engine.auto_english.trigger_keys = keys;
                }
                "emoji_popup" => {
                    config.engine.emoji_popup.enabled = value
                        .parse()
                        .map_err(|_| zbus::fdo::Error::InvalidArgs("Invalid bool".to_string()))?;
                }
                "app_rules" => {
                    let rules: Vec<unim::config::AppRule> =
                        serde_json::from_str(value).map_err(|e| {
                            zbus::fdo::Error::InvalidArgs(format!("Invalid JSON: {}", e))
                        })?;
                    config.engine.app_rules = rules;
                }
                _ => {
                    return Err(zbus::fdo::Error::InvalidArgs(format!(
                        "Unknown key: {}",
                        key
                    )))
                }
            }
            // 파일에도 저장
            if let Err(e) = config.save_to_default_path() {
                unim_log!("DBUS", "[DBus] Config save failed: {}", e);
            }
        }

        // 시그널 브로드캐스트
        Self::config_changed(&signal_ctx, key, value).await?;
        unim_log!("DBUS", "[DBus] Config changed: {} = {}", key, value);
        Ok(())
    }

    /// 전체 설정을 YAML 문자열로 조회 (`~/.config/unim/config.yaml`와 동일 포맷)
    ///
    /// 클라이언트가 단일 메서드 호출로 전체 설정을 읽을 수 있다.
    async fn get_config_yaml(&self) -> zbus::fdo::Result<String> {
        let config = self.config.read().await;
        let yaml = serde_yaml::to_string(&*config)
            .map_err(|e| zbus::fdo::Error::Failed(format!("Config YAML 직렬화 실패: {}", e)))?;
        Ok(yaml)
    }

    /// 전체 설정을 JSON 문자열로 조회
    ///
    /// `ConfigChangedJson` signal payload와 동일한 직렬화. GNOME extension 등
    /// JS 클라이언트가 YAML 파서 없이 시작 시점의 전체 Config를 읽기 위한 메서드.
    async fn get_config_json(&self) -> zbus::fdo::Result<String> {
        let config = self.config.read().await;
        let json = serde_json::to_string(&*config)
            .map_err(|e| zbus::fdo::Error::Failed(format!("Config JSON 직렬화 실패: {}", e)))?;
        Ok(json)
    }

    /// 전체 설정을 YAML 문자열로 수신하여 저장·반영·브로드캐스트
    ///
    /// 동작 순서:
    /// 1. YAML 파싱 — 실패 시 `InvalidArgs`
    /// 2. `AutoTypeFixConfig::clamp_ranges()` 방어 호출
    /// 3. `save_to_default_path()` — 실패 시 `Failed`
    /// 4. 공유 Config(Arc<RwLock>) 갱신 (lock 범위 최소화)
    /// 5. lock 해제 후 `ConfigChangedJson` signal 방출 (payload = JSON)
    async fn set_config_yaml(
        &self,
        #[zbus(signal_context)] signal_ctx: SignalContext<'_>,
        yaml: String,
    ) -> zbus::fdo::Result<()> {
        // 1. 파싱
        let mut new_config: Config = serde_yaml::from_str(&yaml)
            .map_err(|e| zbus::fdo::Error::InvalidArgs(format!("YAML 파싱 실패: {}", e)))?;

        // 2. 범위 방어
        new_config.engine.auto_typefix.clamp_ranges();

        // 3. 파일 저장
        if let Err(e) = new_config.save_to_default_path() {
            unim_log!("DBUS", "[DBus] SetConfigYaml save 실패: {}", e);
            return Err(zbus::fdo::Error::Failed(format!(
                "Config 파일 저장 실패: {}",
                e
            )));
        }

        // 4. 공유 Config 갱신 — lock 스코프 최소화
        let json = {
            let mut cfg = self.config.write().await;
            *cfg = new_config;
            // JSON 직렬화도 lock 안에서 (Config 복제 비용 회피)
            serde_json::to_string(&*cfg)
                .map_err(|e| zbus::fdo::Error::Failed(format!("Config JSON 직렬화 실패: {}", e)))?
        };
        // lock drop 완료

        // 5. signal 방출 — payload는 JSON (GNOME extension JS 호환성)
        Self::config_changed_json(&signal_ctx, &json).await?;

        unim_log!(
            "DBUS",
            "[DBus] SetConfigYaml: {} bytes, ConfigChangedJson emitted ({} bytes)",
            yaml.len(),
            json.len()
        );
        Ok(())
    }

    // =========================================
    // 외부 트리거 액션 — 글로벌(InputContext 비보유) 진입점
    // =========================================

    /// 외부 단축키 도구(KDE System Settings, Hyprland config, Sway, AHK 등)가 호출하는
    /// **InputContext 비보유** 트리거. CLI wrapper(`unim-cli trigger <action>`)도 동일 경로 사용.
    ///
    /// `InputContextHandler::trigger_action`은 GNOME extension처럼 자체 InputContext를 가진
    /// 클라이언트를 위한 것이고, 본 메서드는 **어느 컨텍스트인지 알 수 없는** 환경에서
    /// 동일한 동작을 가능하게 한다. cursor 좌표는 마지막으로 보고된 전역 캐시 사용.
    ///
    /// 지원 액션:
    /// - `"emoji_popup"`: Standalone 모드에서 `ShowEmojiPopupV2` 시그널 발행. Embedded 모드는 no-op.
    ///
    /// 알 수 없는 액션은 경고 로그만 남기고 `Ok(())` — 향후 액션 추가 시 호환성 보장.
    ///
    /// **시그널 발행 경로**: 기존 GUI 구독(`path_namespace=/org/atit/unim`,
    /// `interface=org.atit.unim.InputContext`)을 그대로 활용하기 위해 `ShowEmojiPopupV2`를
    /// `org.atit.unim.InputContext` 인터페이스로 InputMethod path에서 수동 발행한다.
    /// (별도 InputMethod-level signal을 추가하면 zbus `#[proxy]` 매크로 충돌 발생.)
    async fn trigger_action(&self, action: &str) -> zbus::fdo::Result<()> {
        unim_log!("DBUS", "[DBus] TriggerAction(global): action='{}'", action);
        match action {
            "emoji_popup" => {
                // Standalone 모드일 때만 시그널 발행 (Embedded는 IM 모듈이 자체 처리)
                let (is_standalone, english_layout) = {
                    let cfg = self.config.read().await;
                    (
                        cfg.engine.popup_mode == PopupMode::Standalone,
                        cfg.engine.english.layout.clone(),
                    )
                };
                if !is_standalone {
                    unim_log!(
                        "DBUS",
                        "[DBus] TriggerAction(global,emoji_popup) skipped: Embedded 모드"
                    );
                    return Ok(());
                }
                let (x, y, w, h) = *self.last_cursor_rect.lock().unwrap();
                // 마지막 실제 입력 컨텍스트 path가 있으면 그 path에서 시그널 발행
                // (없으면 InputMethod path fallback — 1번 작업의 기존 동작 유지)
                let target_path_str = self
                    .last_active_input_context_path
                    .lock()
                    .unwrap()
                    .clone()
                    .unwrap_or_else(|| crate::INPUT_METHOD_PATH.to_string());
                let path = zbus::zvariant::ObjectPath::try_from(target_path_str.as_str())
                    .map_err(|e| zbus::fdo::Error::Failed(format!("Invalid path: {}", e)))?;
                let (target_cat_id, items, top_row, recent, categories, home_row) =
                    build_emoji_show_payload(&english_layout);
                let result = self
                    .connection
                    .emit_signal(
                        None::<&str>,
                        &path,
                        "org.atit.unim.InputContext",
                        "ShowEmojiPopupV2",
                        &(
                            target_cat_id.as_str(),
                            items.clone(),
                            top_row.as_str(),
                            recent.clone(),
                            categories.clone(),
                            x,
                            y,
                            w,
                            h,
                            home_row.as_str(),
                        ),
                    )
                    .await;
                if let Err(e) = result {
                    unim_log!("DBUS", "[DBus] ShowEmojiPopupV2(global) 발행 실패: {}", e);
                } else {
                    unim_log!(
                        "DBUS",
                        "[DBus] ShowEmojiPopupV2(global) 시그널 발행: path={}, cat='{}', items={}, recent={}, cats={}, pos=({},{},{},{}), home='{}'",
                        target_path_str,
                        target_cat_id,
                        items.len(),
                        recent.len(),
                        categories.len(),
                        x,
                        y,
                        w,
                        h,
                        home_row
                    );
                }
            }
            other => {
                unim_log!(
                    "DBUS",
                    "[DBus] TriggerAction(global): 알 수 없는 action='{}' — 무시 (호환성 유지)",
                    other
                );
            }
        }
        Ok(())
    }

    /// 이모지 팝업 카테고리 변경 — GNOME extension 등 GUI 가 마우스 클릭으로
    /// 좌측 탭을 직접 전환할 때 호출하는 RPC.
    ///
    /// 동작:
    /// 1. 워커에 `SetEmojiCategory { idx }` 보내 마지막 포커스 컨텍스트의 popup_state
    ///    (PopupKind::Emoji) 의 cat_index/items 를 갱신.
    /// 2. 워커가 갱신된 payload (target_cat_id, items, top_row, recent, categories)
    ///    를 응답으로 돌려주면 `ShowEmojiPopupV2` 시그널을 `last_active_input_context_path`
    ///    경로로 재발행하여 모든 V2 구독자(GTK4 GUI, GNOME extension, XIM, Wayland)가
    ///    동일하게 카테고리 전환에 반응하도록 한다.
    ///
    /// emoji popup 이 활성화되지 않았거나 idx 가 범위 밖이면 워커가 `None` 을 반환하여
    /// 시그널 발행을 생략한다 (호환성: RPC 자체는 실패하지 않는다).
    async fn set_emoji_category(&self, idx: u32) -> zbus::fdo::Result<()> {
        let (tx, rx) = oneshot::channel();
        if self
            .engine_tx
            .send(EngineRequest::SetEmojiCategory { idx, response: tx })
            .await
            .is_err()
        {
            unim_log!(
                "DBUS",
                "[DBus] SetEmojiCategory(idx={}): 엔진 워커 송신 실패",
                idx
            );
            return Ok(());
        }
        let payload = match rx.await {
            Ok(Some(p)) => p,
            Ok(None) => {
                unim_log!(
                    "DBUS",
                    "[DBus] SetEmojiCategory(idx={}): emoji popup 비활성 또는 idx 범위 밖 — 시그널 재발행 skip",
                    idx
                );
                return Ok(());
            }
            Err(_) => {
                unim_log!(
                    "DBUS",
                    "[DBus] SetEmojiCategory(idx={}): 응답 채널 수신 실패",
                    idx
                );
                return Ok(());
            }
        };

        // Standalone 모드만 시그널 발행 — Embedded 는 IM 모듈이 자체 처리.
        let is_standalone = {
            let cfg = self.config.read().await;
            cfg.engine.popup_mode == PopupMode::Standalone
        };
        if !is_standalone {
            unim_log!(
                "DBUS",
                "[DBus] SetEmojiCategory(idx={}) skipped: Embedded 모드",
                idx
            );
            return Ok(());
        }

        let (x, y, w, h) = *self.last_cursor_rect.lock().unwrap();
        let target_path_str = self
            .last_active_input_context_path
            .lock()
            .unwrap()
            .clone()
            .unwrap_or_else(|| crate::INPUT_METHOD_PATH.to_string());
        let path = match zbus::zvariant::ObjectPath::try_from(target_path_str.as_str()) {
            Ok(p) => p,
            Err(e) => {
                unim_log!(
                    "DBUS",
                    "[DBus] SetEmojiCategory: 잘못된 path '{}': {} — skip",
                    target_path_str,
                    e
                );
                return Ok(());
            }
        };
        let result = self
            .connection
            .emit_signal(
                None::<&str>,
                &path,
                "org.atit.unim.InputContext",
                "ShowEmojiPopupV2",
                &(
                    payload.target_cat_id.as_str(),
                    payload.items.clone(),
                    payload.top_row.as_str(),
                    payload.recent.clone(),
                    payload.categories.clone(),
                    x,
                    y,
                    w,
                    h,
                    payload.home_row.as_str(),
                ),
            )
            .await;
        if let Err(e) = result {
            unim_log!(
                "DBUS",
                "[DBus] SetEmojiCategory ShowEmojiPopupV2 발행 실패: {}",
                e
            );
        } else {
            unim_log!(
                "DBUS",
                "[DBus] SetEmojiCategory(idx={}) ShowEmojiPopupV2 재발행: path={}, cat='{}', items={}, recent={}, cats={}",
                idx,
                target_path_str,
                payload.target_cat_id,
                payload.items.len(),
                payload.recent.len(),
                payload.categories.len()
            );
        }
        Ok(())
    }

    /// 글로벌 이모지 커밋 — InputContext 비보유 클라이언트(GNOME extension의 emoji 팝업)용.
    ///
    /// GNOME extension은 자체 InputContext를 가지나, GTK4_IM_MODULE=unim 환경에서는
    /// extension의 context로 commit하면 GTK4_IM 모듈을 우회하여 사용자 앱에 도달하지 못한다.
    /// 본 메서드는 마지막으로 포커스를 받은 **실제 입력 컨텍스트 path**(`last_active_input_context_path`)로
    /// `CommitText` / `HidePopup` 시그널을 redirect하여 GTK4_IM·Qt·XIM 모듈이 받아 사용자 앱에
    /// 그대로 commit되게 한다. emoji MRU 즐겨찾기 갱신도 함께 수행.
    ///
    /// `last_active_input_context_path`가 비어있거나 path가 더이상 유효하지 않으면 경고 로그만
    /// 남기고 `Ok(())` 반환 — 호환성을 위해 실패하지 않는다.
    async fn commit_emoji(&self, emoji: &str) -> zbus::fdo::Result<()> {
        if emoji.is_empty() {
            return Ok(());
        }
        // MRU 최근 사용 갱신 (~/.config/unim/emoji-recent.yaml).
        unim::emoji::touch_recent(emoji);

        let target_path_str = match self.last_active_input_context_path.lock().unwrap().clone() {
            Some(p) => p,
            None => {
                unim_log!(
                    "DBUS",
                    "[DBus] CommitEmoji(global): last_active_input_context_path 없음 — skip"
                );
                return Ok(());
            }
        };

        let path = match zbus::zvariant::ObjectPath::try_from(target_path_str.as_str()) {
            Ok(p) => p,
            Err(e) => {
                unim_log!(
                    "DBUS",
                    "[DBus] CommitEmoji(global): 잘못된 path '{}': {} — skip",
                    target_path_str,
                    e
                );
                return Ok(());
            }
        };

        // CommitText 발행 (emoji가 비어있지 않을 때만)
        if !emoji.is_empty() {
            let r = self
                .connection
                .emit_signal(
                    None::<&str>,
                    &path,
                    "org.atit.unim.InputContext",
                    "CommitText",
                    &(emoji.to_string(),),
                )
                .await;
            if let Err(e) = r {
                unim_log!(
                    "DBUS",
                    "[DBus] CommitEmoji(global) CommitText 발행 실패: {} (path={})",
                    e,
                    target_path_str
                );
            }
        }

        // HidePopup 발행 — InputContext 측 commit_emoji와 동일한 후처리
        let r = self
            .connection
            .emit_signal(
                None::<&str>,
                &path,
                "org.atit.unim.InputContext",
                "HidePopup",
                &(),
            )
            .await;
        if let Err(e) = r {
            unim_log!(
                "DBUS",
                "[DBus] CommitEmoji(global) HidePopup 발행 실패: {} (path={})",
                e,
                target_path_str
            );
        }

        unim_log!(
            "DBUS",
            "[DBus] CommitEmoji(global): emoji='{}', path={}",
            emoji,
            target_path_str
        );
        Ok(())
    }
}

/// InputContext 인터페이스 구현을 위한 핸들러
///
/// 이 핸들러는 엔진을 직접 소유하지 않고, 채널을 통해 엔진 스레드와 통신합니다.
pub struct InputContextHandler {
    /// 컨텍스트 ID
    id: u32,
    /// 프론트엔드 클라이언트 이름 (프론트엔드 종류 식별용)
    client_name: String,
    /// 엔진 스레드로 요청을 보내는 채널
    engine_tx: mpsc::Sender<EngineRequest>,
    /// DBus 연결 (시그널 발송용)
    connection: Connection,
    /// 캐싱된 커서 위치 (x, y, width, height) — 본 컨텍스트 전용
    cursor_rect: std::sync::Mutex<(i32, i32, i32, i32)>,
    /// 본 컨텍스트의 DBus 객체 path (예: `/org/atit/unim/InputContext_15`).
    /// `focus_in`/`destroy` 시 글로벌 `last_active_input_context_path` 캐시를
    /// 갱신/무효화할 때 사용한다.
    path: String,
    /// 전역 cursor_rect 캐시 (`InputMethodService::last_cursor_rect`와 동일 Arc).
    /// `report_cursor_rect`가 호출되면 본 컨텍스트뿐 아니라 전역 캐시도 갱신해
    /// InputMethod-level `trigger_action`이 fallback 좌표로 사용할 수 있게 한다.
    last_cursor_rect: Arc<std::sync::Mutex<(i32, i32, i32, i32)>>,
    /// 전역 last_active_input_context_path 캐시 (`InputMethodService`와 동일 Arc).
    /// `focus_in` 시 frontend가 GNOME(extension 자체 컨텍스트)이 아니면 본 path를 저장.
    /// `destroy` 시 본 path가 캐시 값과 같으면 None으로 비워준다.
    last_active_input_context_path: Arc<std::sync::Mutex<Option<String>>>,
}

/// client_name으로부터 프론트엔드 종류를 식별
fn detect_frontend_type(client_name: &str) -> &'static str {
    match client_name {
        "gtk3-unim" => "GTK3",
        "gtk4-unim" => "GTK4",
        "qt5-unim" => "Qt5",
        "qt6-unim" => "Qt6",
        "unim-xim" => "XIM",
        "unim-wayland" => "Wayland",
        "gnome-extension" => "GNOME",
        _ => "Unknown",
    }
}

/// `ShowEmojiPopupV2` 발행 시 들어가는 6-튜플 payload 를 만든다.
///
/// `trigger_action("emoji_popup")` 처럼 엔진 파이프라인을 거치지 않는 외부 우회
/// 경로에서 사용. 시작 카테고리는 항상 `Recent`(MRU) 이며 `items` 도 동일한 MRU
/// 리스트를 그대로 담는다 (popup_state cat_index=0 의 동작과 일치).
///
/// 반환: `(target_cat_id, items, top_row, recent, categories, home_row)`.
fn build_emoji_show_payload(
    english_layout: &str,
) -> (
    String,
    Vec<String>,
    String,
    Vec<String>,
    Vec<(String, String, String, u32)>,
    String,
) {
    let recent = unim::emoji::load_recent();
    let mut categories: Vec<(String, String, String, u32)> = Vec::with_capacity(9);
    categories.push((
        "Recent".to_string(),
        "최근 사용".to_string(),
        "Recent".to_string(),
        recent.len() as u32,
    ));
    categories.extend(unim::emoji::list_categories());
    let top_row = unim::config::english_layout_top_row_labels(english_layout).to_string();
    let home_row = unim::config::english_layout_home_row_labels(english_layout).to_string();
    (
        "Recent".to_string(),
        recent.clone(),
        top_row,
        recent,
        categories,
        home_row,
    )
}

impl InputContextHandler {
    /// 새 핸들러 생성
    ///
    /// `last_cursor_rect`는 `InputMethodService`의 전역 cursor 캐시 — 글로벌
    /// `trigger_action`이 cursor 좌표 없이 호출될 때 사용한다.
    /// `last_active_input_context_path`는 동일하게 글로벌 last-active path 캐시 —
    /// 글로벌 `CommitEmoji`/`TriggerAction`이 시그널을 redirect할 때 사용한다.
    pub fn new(
        id: u32,
        client_name: String,
        path: String,
        engine_tx: mpsc::Sender<EngineRequest>,
        connection: Connection,
        last_cursor_rect: Arc<std::sync::Mutex<(i32, i32, i32, i32)>>,
        last_active_input_context_path: Arc<std::sync::Mutex<Option<String>>>,
    ) -> Self {
        Self {
            id,
            client_name,
            engine_tx,
            connection,
            cursor_rect: std::sync::Mutex::new((0, 0, 0, 0)),
            path,
            last_cursor_rect,
            last_active_input_context_path,
        }
    }

    /// `CommitText` 와 `HidePopup` 시그널을 popup-owner path 로 redirect 발행.
    ///
    /// GNOME extension 자체 InputContext 에서 RPC (SelectHanja/CancelHanja/
    /// SelectSpecialChar/CancelSpecialChar) 가 들어와도, 실제 입력 대상은 다른
    /// frontend (GTK4_IM 등) 의 path 다. last_active path 가 popup-owner 와
    /// 일치한다는 가정하에 그 path 로 시그널을 발행해 사용자 앱에 commit 도달하게 한다.
    /// last_active 가 비어있으면 self.path 로 fallback (호출자 = popup-owner 인 단순
    /// 케이스 — 기존 동작 유지).
    ///
    /// `text` 가 비어있으면 `CommitText` 는 발행하지 않고 `HidePopup` 만 발행한다.
    async fn redirect_commit_and_hide(&self, text: &str) {
        let target_path_str = self
            .last_active_input_context_path
            .lock()
            .unwrap()
            .clone()
            .unwrap_or_else(|| self.path.clone());
        let path = match zbus::zvariant::ObjectPath::try_from(target_path_str.as_str()) {
            Ok(p) => p,
            Err(e) => {
                unim_log!(
                    "DBUS",
                    "[DBus] redirect_commit_and_hide: 잘못된 path '{}': {} — skip",
                    target_path_str,
                    e
                );
                return;
            }
        };
        if !text.is_empty() {
            let r = self
                .connection
                .emit_signal(
                    None::<&str>,
                    &path,
                    "org.atit.unim.InputContext",
                    "CommitText",
                    &(text.to_string(),),
                )
                .await;
            if let Err(e) = r {
                unim_log!(
                    "DBUS",
                    "[DBus] redirect_commit_and_hide CommitText 발행 실패: {} (path={})",
                    e,
                    target_path_str
                );
            }
        }
        let r = self
            .connection
            .emit_signal(
                None::<&str>,
                &path,
                "org.atit.unim.InputContext",
                "HidePopup",
                &(),
            )
            .await;
        if let Err(e) = r {
            unim_log!(
                "DBUS",
                "[DBus] redirect_commit_and_hide HidePopup 발행 실패: {} (path={})",
                e,
                target_path_str
            );
        }
    }

    /// PopupRender 시그널 발행 헬퍼 — engine view_model 페이로드를 unpack 하여 emit.
    /// frontend (GNOME / gui-gtk) 가 본 시그널로 헤더·푸터·셀·탭·확장 아이콘 모두 즉시 렌더.
    async fn emit_popup_render(signal_ctx: &SignalContext<'_>, rs: &PopupRenderPayload) {
        Self::popup_render(
            signal_ctx,
            rs.kind,
            (
                rs.target.clone(),
                rs.header_text.clone(),
                rs.footer_text.clone(),
                rs.expand_text.clone(),
            ),
            (
                rs.rows,
                rs.cols,
                rs.sel_row,
                rs.sel_col,
                rs.current_page,
                rs.total_pages,
            ),
            (rs.show_footer, rs.expand_visible),
            rs.cells.clone(),
            rs.col_headers.clone(),
            rs.row_headers.clone(),
            rs.tab_labels.clone(),
            rs.active_tab_index,
        )
        .await
        .ok();
        unim_log!(
            "DBUS",
            "[DBus] PopupRender: kind={}, target='{}', cells={}, page={}/{}",
            rs.kind,
            rs.target,
            rs.cells.len(),
            rs.current_page + 1,
            rs.total_pages
        );
    }
}

#[interface(name = "org.atit.unim.InputContext")]
impl InputContextHandler {
    /// 키 이벤트 처리
    /// 반환값: (consumed, preedit, commit)
    async fn process_key_event(
        &self,
        #[zbus(signal_context)] signal_ctx: SignalContext<'_>,
        keyval: u32,
        keycode: u32,
        state: u32,
    ) -> zbus::fdo::Result<(bool, String, String)> {
        let (response_tx, response_rx) = oneshot::channel();

        self.engine_tx
            .send(EngineRequest::ProcessKey {
                context_id: self.id,
                keyval,
                keycode,
                state,
                response: response_tx,
            })
            .await
            .map_err(|_| zbus::fdo::Error::Failed("Engine not available".to_string()))?;

        let response = response_rx
            .await
            .map_err(|_| zbus::fdo::Error::Failed("Engine response failed".to_string()))?;

        let preedit = response.preedit.unwrap_or_default();
        let commit = response.commit.unwrap_or_default();

        // 모드 변경 시그널 발송
        if let Some(is_korean) = response.mode_changed {
            unim_log!("DBUS", "[DBus] 모드 변경 감지: is_korean={}", is_korean);
            let im_signal_ctx =
                zbus::SignalContext::new(&self.connection, crate::INPUT_METHOD_PATH).map_err(
                    |e| zbus::fdo::Error::Failed(format!("Signal context error: {}", e)),
                )?;
            InputMethodService::global_mode_changed(&im_signal_ctx, is_korean)
                .await
                .ok();
        }

        // 팝업 시그널 자동 발행 (Push 방식: 인디케이터가 팝업 표시)
        // Standalone 모드일 때만 Show 시그널 발행 (Embedded 모드에서는 IM 모듈이 자체 처리)
        let is_standalone =
            Config::load_from_default_path().engine.popup_mode == PopupMode::Standalone;
        if let Some(popup) = &response.popup_action {
            match popup {
                PopupAction::ShowHanja {
                    target,
                    candidates,
                    top_row,
                } if is_standalone => {
                    let (x, y, w, h) = *self.cursor_rect.lock().unwrap();
                    Self::show_hanja_popup(
                        &signal_ctx,
                        target,
                        candidates.clone(),
                        top_row,
                        x,
                        y,
                        w,
                        h,
                    )
                    .await
                    .ok();
                    unim_log!(
                        "DBUS",
                        "[DBus] ShowHanjaPopup 시그널 발행: target='{}', count={}, top_row='{}'",
                        target,
                        candidates.len(),
                        top_row
                    );
                }
                PopupAction::ShowSpecial {
                    target,
                    characters,
                    top_row,
                } if is_standalone => {
                    let (x, y, w, h) = *self.cursor_rect.lock().unwrap();
                    Self::show_special_popup(
                        &signal_ctx,
                        target,
                        characters.clone(),
                        top_row,
                        x,
                        y,
                        w,
                        h,
                    )
                    .await
                    .ok();
                    unim_log!(
                        "DBUS",
                        "[DBus] ShowSpecialPopup 시그널 발행: target='{}', count={}",
                        target,
                        characters.len()
                    );
                }
                PopupAction::ShowEmoji {
                    target_cat_id,
                    items,
                    top_row,
                    recent,
                    categories,
                    home_row,
                } if is_standalone => {
                    let (x, y, w, h) = *self.cursor_rect.lock().unwrap();
                    // PR #5: v1 4-인자 시그널 제거. ShowEmojiPopupV2 single-emit.
                    Self::show_emoji_popup_v2(
                        &signal_ctx,
                        target_cat_id,
                        items.clone(),
                        top_row,
                        recent.clone(),
                        categories.clone(),
                        x,
                        y,
                        w,
                        h,
                        home_row,
                    )
                    .await
                    .ok();
                    unim_log!(
                        "DBUS",
                        "[DBus] ShowEmojiPopupV2 시그널 발행: target='{}', items={}, recent={}, cats={}, pos=({},{},{},{}), home='{}'",
                        target_cat_id,
                        items.len(),
                        recent.len(),
                        categories.len(),
                        x,
                        y,
                        w,
                        h,
                        home_row
                    );
                }
                PopupAction::HidePopup => {
                    Self::hide_popup(&signal_ctx).await.ok();
                    unim_log!("DBUS", "[DBus] HidePopup 시그널 발행");
                }
                PopupAction::PopupNavigate {
                    page,
                    total_pages,
                    selected,
                    rows,
                    cols,
                    sel_row,
                    sel_col,
                } => {
                    Self::popup_navigate(
                        &signal_ctx,
                        *page as i32,
                        *total_pages as i32,
                        *selected as i32,
                        *rows as i32,
                        *cols as i32,
                        *sel_row as i32,
                        *sel_col as i32,
                    )
                    .await
                    .ok();
                    unim_log!(
                        "DBUS",
                        "[DBus] PopupNavigate: page={}/{}, selected={}, rows={}, cols={}, sel=({},{})",
                        page,
                        total_pages,
                        selected,
                        rows,
                        cols,
                        sel_row,
                        sel_col
                    );
                }
                PopupAction::HanjaBookmarkChanged { index, bookmarked } => {
                    Self::hanja_bookmark_changed(&signal_ctx, *index as u32, *bookmarked)
                        .await
                        .ok();
                    unim_log!(
                        "DBUS",
                        "[DBus] HanjaBookmarkChanged: index={}, bookmarked={}",
                        index,
                        bookmarked
                    );
                }
                PopupAction::HanjaCandidatesReordered {
                    target,
                    candidates,
                    bookmarks,
                    new_cursor,
                    page,
                    sel_row,
                    sel_col,
                    bookmarked,
                    was_bookmarked,
                } => {
                    // 후보·즐겨찾기·커서를 한 시그널로 전달해 frontend가 일괄 갱신.
                    let hanjas: Vec<String> = candidates.iter().map(|(h, _)| h.clone()).collect();
                    let meanings: Vec<String> = candidates.iter().map(|(_, m)| m.clone()).collect();
                    Self::hanja_candidates_reordered(
                        &signal_ctx,
                        target,
                        hanjas,
                        meanings,
                        bookmarks.clone(),
                        *new_cursor as u32,
                        *page as i32,
                        *sel_row as i32,
                        *sel_col as i32,
                        *bookmarked,
                        *was_bookmarked,
                    )
                    .await
                    .ok();
                    unim_log!(
                        "DBUS",
                        "[DBus] HanjaCandidatesReordered: target='{}', count={}, new_cursor={}, page={}, sel=({},{}), bookmarked={} (was={})",
                        target,
                        candidates.len(),
                        new_cursor,
                        page,
                        sel_row,
                        sel_col,
                        bookmarked,
                        was_bookmarked
                    );
                }
                PopupAction::PageJump { page_index } => {
                    // 마우스 ◀/▶ 등 외부 RPC 응답 — process_key 경로에서는 발생 안 함이지만
                    // 안전하게 PopupNavigate 시그널로 변환. (현 진입점은 popup_change_page RPC.)
                    unim_log!(
                        "DBUS",
                        "[DBus] PageJump: page_index={} (process_key 경로 — no-op)",
                        page_index
                    );
                }
                // Embedded 모드에서 ShowHanja/ShowSpecial은 IM 모듈이 자체 처리
                _ => {}
            }
        }

        // PopupRender 시그널 발행 (Phase B 통합 SoT — frontend 가 이걸로 헤더·푸터·셀·탭
        // 전부 즉시 렌더). popup 활성 시에만 산출.
        if let Some(rs) = &response.render_state {
            Self::emit_popup_render(&signal_ctx, rs).await;
        }

        // AutoTypeFix: 교정 결과가 있으면 비동기 시그널로 발행
        if let Some((delete_chars, commit_text, preedit_text)) = &response.auto_typefix {
            unim_log!(
                "DBUS",
                "[DBus] AutoTypeFix 시그널: delete={}, commit='{}', preedit='{}'",
                delete_chars,
                commit_text,
                preedit_text
            );
            Self::auto_typefix_apply(&signal_ctx, *delete_chars, commit_text, preedit_text)
                .await
                .ok();
        }

        unim_log!(
            "DBUS",
            "[DBus] ProcessKeyEvent: keyval={}, consumed={}, preedit='{}', commit='{}'",
            keyval,
            response.consumed,
            preedit,
            commit
        );

        Ok((response.consumed, preedit, commit))
    }

    /// 포커스 획득 - 현재 컨텍스트의 모드를 시그널로 발송 (window_id: 창 식별자)
    async fn focus_in(&self, window_id: &str) -> zbus::fdo::Result<()> {
        // 현재 컨텍스트의 모드 조회
        let (response_tx, response_rx) = oneshot::channel();

        self.engine_tx
            .send(EngineRequest::FocusIn {
                context_id: self.id,
                window_id: window_id.to_string(),
                response: response_tx,
            })
            .await
            .ok();

        let frontend = detect_frontend_type(&self.client_name);

        // 모든 frontend의 InputContext를 last_active 후보로 등록한다.
        // GNOME extension의 자체 context도 포함하는 이유:
        //   - cursorai/Chrome/Electron 등 GTK_IM_MODULE을 받지 않는 앱은 Wayland
        //     text-input v3 경로로 GNOME Shell InputMethod가 IM을 처리하며 데몬엔
        //     `frontend=GNOME`으로만 보고된다.
        //   - 이 경우 CommitText는 GNOME Shell이 현재 포커스 클라이언트로 자동
        //     forward하므로 사용자가 보고 있는 창에 정확히 입력된다.
        //   - GTK4_IM_MODULE=unim 사용 가능한 앱은 frontend=GTK4로 자체 context를
        //     만들고, 그쪽이 더 최근 활동이면 last_active를 덮어쓴다.
        *self.last_active_input_context_path.lock().unwrap() = Some(self.path.clone());

        if let Ok(is_korean) = response_rx.await {
            // InputMethod 경로에서 GlobalModeChanged 시그널 발송 (UI 동기화)
            unim_log!(
                "DBUS",
                "[DBus] FocusIn: context_id={}, frontend={}, window_id={}, mode={}",
                self.id,
                frontend,
                window_id,
                if is_korean { "Korean" } else { "English" }
            );
            let signal_ctx = zbus::SignalContext::new(&self.connection, crate::INPUT_METHOD_PATH)
                .map_err(|e| {
                zbus::fdo::Error::Failed(format!("Signal context error: {}", e))
            })?;
            InputMethodService::global_mode_changed(&signal_ctx, is_korean)
                .await
                .ok();
        } else {
            unim_log!(
                "DBUS",
                "[DBus] FocusIn: context_id={}, frontend={}",
                self.id,
                frontend
            );
        }

        Ok(())
    }

    /// 포커스 상실
    /// 반환값: 커밋된 텍스트 (조합 중이던 문자열)
    async fn focus_out(
        &self,
        #[zbus(signal_context)] signal_ctx: SignalContext<'_>,
    ) -> zbus::fdo::Result<String> {
        let (response_tx, response_rx) = oneshot::channel();

        self.engine_tx
            .send(EngineRequest::FocusOut {
                context_id: self.id,
                response: response_tx,
            })
            .await
            .ok();

        let commit = response_rx.await.ok().flatten().unwrap_or_default();

        // 커밋 텍스트는 RPC 반환값으로만 전달한다.
        // 과거 레거시 경로에서 CommitText 시그널도 함께 발행했으나, IM 모듈이
        // 시그널과 RPC 반환값을 모두 처리하여 이중 커밋(예: "늘" 두 번)이 발생.
        // 포커스 아웃 커밋은 해당 컨텍스트에만 전달해야 하므로 브로드캐스트 시그널이
        // 의미상으로도 부적절하다.
        Self::hide_popup(&signal_ctx).await.ok();

        unim_log!(
            "DBUS",
            "[DBus] FocusOut: context_id={}, commit='{}'",
            self.id,
            commit
        );
        Ok(commit)
    }

    /// 입력 상태 초기화 (팝업 취소 + 조합 커밋)
    async fn reset(
        &self,
        #[zbus(signal_context)] signal_ctx: SignalContext<'_>,
    ) -> zbus::fdo::Result<()> {
        let (response_tx, response_rx) = oneshot::channel();

        self.engine_tx
            .send(EngineRequest::Reset {
                context_id: self.id,
                response: response_tx,
            })
            .await
            .ok();

        let commit = response_rx.await.ok().flatten().unwrap_or_default();

        // 팝업이 열려있었을 수 있으므로 HidePopup 시그널 발행
        Self::hide_popup(&signal_ctx).await.ok();

        if !commit.is_empty() {
            Self::commit_text(&signal_ctx, &commit).await.ok();
        }
        Self::update_preedit_text(&signal_ctx, "", 0, false)
            .await
            .ok();

        unim_log!(
            "DBUS",
            "[DBus] Reset: context_id={}, commit='{}'",
            self.id,
            commit
        );
        Ok(())
    }

    /// 컨텍스트 파괴
    async fn destroy(&self) -> zbus::fdo::Result<()> {
        // last_active 캐시가 본 컨텍스트를 가리키고 있다면 비워준다 — stale path
        // 으로 글로벌 CommitEmoji/TriggerAction이 죽은 path에 시그널을 보내는 것 방지.
        {
            let mut last = self.last_active_input_context_path.lock().unwrap();
            if last.as_deref() == Some(self.path.as_str()) {
                *last = None;
            }
        }

        self.engine_tx
            .send(EngineRequest::DestroyContext { id: self.id })
            .await
            .ok();

        // object_server에서 이 핸들러 자신도 제거 — Destroy 호출 후에도 핸들러가
        // 남아있으면 zbus 내부 라우팅 테이블·시그널 구독 상태가 context_id마다 누적돼
        // 장시간 실행 시 RSS가 꾸준히 증가한다. 경로는 create_input_context와 동일 포맷.
        let path_str = format!("{}{}", crate::INPUT_CONTEXT_PATH_PREFIX, self.id);
        if let Ok(path) = zbus::zvariant::ObjectPath::try_from(path_str.as_str()) {
            let _ = self
                .connection
                .object_server()
                .remove::<InputContextHandler, _>(path)
                .await;
        }

        unim_log!("DBUS", "[DBus] Context 파괴: id={}", self.id);
        Ok(())
    }

    /// Preedit 텍스트 업데이트 시그널
    #[zbus(signal)]
    async fn update_preedit_text(
        signal_ctx: &SignalContext<'_>,
        text: &str,
        cursor_pos: u32,
        visible: bool,
    ) -> zbus::Result<()>;

    /// 텍스트 커밋 시그널
    #[zbus(signal)]
    async fn commit_text(signal_ctx: &SignalContext<'_>, text: &str) -> zbus::Result<()>;

    // =========================================
    // 팝업 관련 시그널 (unim-gui-gtk/unim-gui-qt가 구독)
    // =========================================

    /// 한자 팝업 표시 시그널
    ///
    /// `top_row`는 활성 영문 키맵의 가장 위 9문자 (예: "QWERTYUIO", "',.PYFGCR").
    /// expanded(9x9) 컬럼 헤더와 Letter 키 점프에 사용한다. 특수문자 팝업과 동일 source.
    #[zbus(signal)]
    async fn show_hanja_popup(
        signal_ctx: &SignalContext<'_>,
        target: &str,
        candidates: Vec<(String, String)>,
        top_row: &str,
        cursor_x: i32,
        cursor_y: i32,
        cursor_width: i32,
        cursor_height: i32,
    ) -> zbus::Result<()>;

    /// 특수문자 팝업 표시 시그널
    #[zbus(signal)]
    async fn show_special_popup(
        signal_ctx: &SignalContext<'_>,
        target: &str,
        characters: Vec<String>,
        top_row: &str,
        cursor_x: i32,
        cursor_y: i32,
        cursor_width: i32,
        cursor_height: i32,
    ) -> zbus::Result<()>;

    /// 이모지 팝업 표시 시그널 v2 (PR #1 emoji overhaul, OPTION X engine-driven).
    ///
    /// 한자/특수문자 시그널과 동일하게 카테고리·페이지·MRU 데이터를 push 한다 —
    /// GUI 가 별도 sync RPC 를 호출할 필요가 없다. (PR #5 에서 v1 4-인자 시그널은
    /// 완전히 제거됨.)
    ///
    /// payload:
    /// - `target_cat_id`: 시작 카테고리 id ("Recent" / "SmileysPeople" / ... / "Flags").
    /// - `items`: 시작 카테고리의 emoji 풀 (전체).
    /// - `top_row`: 활성 영문 키맵 상단 9 문자.
    /// - `recent`: 'Recent' 탭 캐시 (MRU 81개).
    /// - `categories`: 좌측 9 탭 메타 — `(id, ko, en, count)` 튜플 9개 (Recent + 8).
    #[zbus(signal)]
    #[allow(clippy::too_many_arguments)]
    async fn show_emoji_popup_v2(
        signal_ctx: &SignalContext<'_>,
        target_cat_id: &str,
        items: Vec<String>,
        top_row: &str,
        recent: Vec<String>,
        categories: Vec<(String, String, String, u32)>,
        cursor_x: i32,
        cursor_y: i32,
        cursor_width: i32,
        cursor_height: i32,
        home_row: &str,
    ) -> zbus::Result<()>;

    /// 팝업 숨김 시그널
    #[zbus(signal)]
    async fn hide_popup(signal_ctx: &SignalContext<'_>) -> zbus::Result<()>;

    /// 팝업 네비게이션 시그널 (페이지/선택 변경)
    #[zbus(signal)]
    async fn popup_navigate(
        signal_ctx: &SignalContext<'_>,
        page: i32,
        total_pages: i32,
        selected: i32,
        rows: i32,
        cols: i32,
        sel_row: i32,
        sel_col: i32,
    ) -> zbus::Result<()>;

    /// 팝업 렌더 시그널 — engine `PopupViewModel` 의 DBus 표현 (Phase B 통합 SoT).
    ///
    /// popup 활성 시 매 상태 변화마다 발행. frontend (GNOME / gui-gtk) 는 본 시그널만
    /// 구독하면 헤더·푸터·셀·탭·확장 아이콘 모두 즉시 렌더 가능 — 서식 문자열·visibility
    /// 조건이 daemon 측 SoT.
    ///
    /// 인자 묶음 (zbus 의 단일 시그널 인자 수 한계 회피용 그룹화):
    /// - `texts`: `(target, header_text, footer_text, expand_text)`.
    /// - `layout`: `(rows, cols, sel_row, sel_col, current_page, total_pages)` (u32 6개).
    /// - `flags`: `(show_footer, expand_visible)`.
    /// - `cells`: column-major 길이 `rows*cols`, 각 `(text, meaning, flags_bits)`.
    ///   `flags_bits` 비트는 [`popup_render_flags`] 참조 — has_data/selected/col_hl/row_hl/bookmarked.
    /// - `col_headers`/`row_headers`: `(text, is_active)`.
    /// - `tab_labels`: 이모지 좌측 탭 라벨 (단축키 prefix 포함).
    #[zbus(signal)]
    #[allow(clippy::too_many_arguments)]
    async fn popup_render(
        signal_ctx: &SignalContext<'_>,
        kind: u32,
        texts: (String, String, String, String),
        layout: (u32, u32, u32, u32, u32, u32),
        flags: (bool, bool),
        cells: Vec<(String, String, u32)>,
        col_headers: Vec<(String, bool)>,
        row_headers: Vec<(String, bool)>,
        tab_labels: Vec<String>,
        active_tab_index: u32,
    ) -> zbus::Result<()>;

    /// 한자 즐겨찾기 상태 변경 시그널 (Space 토글 등으로 상태가 바뀐 경우)
    #[zbus(signal)]
    async fn hanja_bookmark_changed(
        signal_ctx: &SignalContext<'_>,
        index: u32,
        bookmarked: bool,
    ) -> zbus::Result<()>;

    /// 한자 후보 재정렬 시그널 (즐겨찾기 토글 시 즐겨찾기 우선 정렬 후 발행).
    ///
    /// frontend는 candidates+meanings+bookmarks를 그대로 받아 후보 리스트를
    /// 통째로 교체하고 (page, sel_row, sel_col)로 커서를 점프시킨다.
    /// new_cursor는 토글된 한자의 새 전체 인덱스(편의용).
    /// `was_bookmarked` 는 토글 직전 상태 — frontend 가 OFF→ON 인지 ON→OFF 인지
    /// 분기해 시각 신호(예: 별 해제 시 원위치 점프 flash)를 띄우는 용도.
    #[allow(clippy::too_many_arguments)]
    #[zbus(signal)]
    async fn hanja_candidates_reordered(
        signal_ctx: &SignalContext<'_>,
        target: &str,
        hanjas: Vec<String>,
        meanings: Vec<String>,
        bookmarks: Vec<bool>,
        new_cursor: u32,
        page: i32,
        sel_row: i32,
        sel_col: i32,
        bookmarked: bool,
        was_bookmarked: bool,
    ) -> zbus::Result<()>;

    // =========================================
    // 외부 트리거 액션 (단축키 우회)
    // =========================================

    /// 외부 컴포넌트(예: GNOME Wayland extension)가 단축키 우회를 위해 발행하는
    /// 일반화된 액션 트리거.
    ///
    /// GNOME Wayland 컴포지터가 `Super+.` 같은 시스템 조합 키를 가로채 IM 모듈에
    /// 키가 도달하지 못하는 환경을 위해, extension이 `grab_accelerator()`로 직접
    /// 단축키를 캡처한 뒤 본 RPC를 호출해 데몬에 동작을 요청한다.
    /// X11/XIM 등 IM 모듈이 키를 직접 수신하는 환경에서는 호출되지 않는다.
    ///
    /// 지원 액션:
    /// - `"emoji_popup"`: 이모지 팝업 시그널 발행. Standalone 모드에서만 발행하며
    ///   Embedded 모드에서는 IM 모듈이 자체 처리한다 (no-op).
    ///
    /// 알 수 없는 액션은 경고 로그만 남기고 `Ok(())` 반환 — 향후 액션 추가 시
    /// 구버전 데몬과 신버전 extension의 호환성을 보장한다.
    async fn trigger_action(
        &self,
        #[zbus(signal_context)] signal_ctx: SignalContext<'_>,
        action: &str,
    ) -> zbus::fdo::Result<()> {
        unim_log!(
            "DBUS",
            "[DBus] TriggerAction: context_id={}, action='{}'",
            self.id,
            action
        );
        match action {
            "emoji_popup" => {
                // Standalone 모드일 때만 시그널 발행 (Embedded는 IM 모듈이 자체 처리)
                let cfg_snapshot = Config::load_from_default_path();
                let is_standalone = cfg_snapshot.engine.popup_mode == PopupMode::Standalone;
                if !is_standalone {
                    unim_log!(
                        "DBUS",
                        "[DBus] TriggerAction(emoji_popup) skipped: Embedded 모드"
                    );
                    return Ok(());
                }
                let (x, y, w, h) = *self.cursor_rect.lock().unwrap();
                let (target_cat_id, items, top_row, recent, categories, home_row) =
                    build_emoji_show_payload(&cfg_snapshot.engine.english.layout);
                Self::show_emoji_popup_v2(
                    &signal_ctx,
                    &target_cat_id,
                    items.clone(),
                    &top_row,
                    recent.clone(),
                    categories.clone(),
                    x,
                    y,
                    w,
                    h,
                    &home_row,
                )
                .await
                .ok();
                unim_log!(
                    "DBUS",
                    "[DBus] ShowEmojiPopupV2 시그널 발행 (trigger_action): cat='{}', items={}, recent={}, cats={}, pos=({},{},{},{}), home='{}'",
                    target_cat_id,
                    items.len(),
                    recent.len(),
                    categories.len(),
                    x,
                    y,
                    w,
                    h,
                    home_row
                );
            }
            // 향후 확장 지점 (예: "hanja_popup", "special_popup")
            other => {
                unim_log!(
                    "DBUS",
                    "[DBus] TriggerAction: 알 수 없는 action='{}' — 무시 (호환성 유지)",
                    other
                );
            }
        }
        Ok(())
    }

    // =========================================
    // 커서 위치 보고
    // =========================================

    /// 프런트엔드가 커서 위치를 보고 (팝업 포지셔닝용)
    ///
    /// 컨텍스트 전용 캐시(`cursor_rect`)와 전역 캐시(`last_cursor_rect`)를 동시 갱신.
    /// 전역 캐시는 InputMethod-level `trigger_action`이 fallback 좌표로 사용한다.
    async fn report_cursor_rect(
        &self,
        x: i32,
        y: i32,
        width: i32,
        height: i32,
    ) -> zbus::fdo::Result<()> {
        *self.cursor_rect.lock().unwrap() = (x, y, width, height);
        *self.last_cursor_rect.lock().unwrap() = (x, y, width, height);
        // last_active도 함께 갱신 — cursor를 보고하는 context가 사용자가 입력 중인
        // context. focus_in 보다 더 빈번/최신 신호이므로 창 전환 race를 보완한다.
        *self.last_active_input_context_path.lock().unwrap() = Some(self.path.clone());
        unim_log!(
            "DBUS",
            "[DBus] CursorRect: context_id={}, x={}, y={}, w={}, h={}",
            self.id,
            x,
            y,
            width,
            height
        );
        Ok(())
    }

    /// 입력 필드 목적 설정 (비밀번호/PIN 필드 감지용)
    async fn set_content_type(&self, purpose: u32) -> zbus::fdo::Result<()> {
        self.engine_tx
            .send(EngineRequest::SetContentType {
                context_id: self.id,
                purpose,
            })
            .await
            .ok();
        unim_log!(
            "DBUS",
            "[DBus] SetContentType: context_id={}, purpose={}",
            self.id,
            purpose
        );
        Ok(())
    }

    /// Surrounding text 설정 (커서 주변 텍스트 전달)
    async fn set_surrounding_text(
        &self,
        text: &str,
        cursor_pos: u32,
        anchor_pos: u32,
    ) -> zbus::fdo::Result<()> {
        self.engine_tx
            .send(EngineRequest::SetSurroundingText {
                context_id: self.id,
                text: text.to_string(),
                cursor_pos,
                anchor_pos,
            })
            .await
            .ok();
        unim_log!(
            "DBUS",
            "[DBus] SetSurroundingText: context_id={}, cursor={}, anchor={}, len={}",
            self.id,
            cursor_pos,
            anchor_pos,
            text.len()
        );
        Ok(())
    }

    /// Smart Backspace (자모 단위 삭제)
    /// 반환값: (삭제할 문자 수, 대체 텍스트) 또는 (0, "")
    async fn smart_backspace(
        &self,
        #[zbus(signal_context)] signal_ctx: SignalContext<'_>,
    ) -> zbus::fdo::Result<(u32, String)> {
        let (response_tx, response_rx) = oneshot::channel();

        self.engine_tx
            .send(EngineRequest::SmartBackspace {
                context_id: self.id,
                response: response_tx,
            })
            .await
            .map_err(|_| zbus::fdo::Error::Failed("Engine not available".to_string()))?;

        let result = response_rx
            .await
            .map_err(|_| zbus::fdo::Error::Failed("Engine response failed".to_string()))?;

        if let Some((delete_chars, replacement)) = result {
            unim_log!(
                "DBUS",
                "[DBus] SmartBackspace: delete={}, replacement='{}'",
                delete_chars,
                replacement
            );
            // 기존 글자 삭제
            Self::delete_surrounding_text(&signal_ctx, -(delete_chars as i32), delete_chars)
                .await
                .ok();
            // 대체 텍스트 커밋 (있는 경우)
            if !replacement.is_empty() {
                Self::commit_text(&signal_ctx, &replacement).await.ok();
            }
            Ok((delete_chars, replacement))
        } else {
            Ok((0, String::new()))
        }
    }

    /// delete_surrounding_text 시그널 (엔진 → 프론트엔드)
    #[zbus(signal)]
    async fn delete_surrounding_text(
        signal_ctx: &SignalContext<'_>,
        offset: i32,
        n_chars: u32,
    ) -> zbus::Result<()>;

    /// AutoTypeFix 교정 시그널 (엔진 → 프론트엔드)
    /// delete_chars 글자를 삭제 → commit_text 커밋 → preedit_text를 preedit으로 설정
    #[zbus(signal)]
    async fn auto_typefix_apply(
        signal_ctx: &SignalContext<'_>,
        delete_chars: u32,
        commit_text: &str,
        preedit_text: &str,
    ) -> zbus::Result<()>;

    // =========================================
    // 한자 변환 관련 메서드
    // =========================================

    /// 한자 후보 목록 조회
    /// 반환값: (변환 대상, [(한자, 뜻풀이), ...])
    async fn get_hanja_candidates(
        &self,
        #[zbus(signal_context)] signal_ctx: SignalContext<'_>,
    ) -> zbus::fdo::Result<(String, Vec<(String, String)>)> {
        let (response_tx, response_rx) = oneshot::channel();

        self.engine_tx
            .send(EngineRequest::GetHanjaCandidates {
                context_id: self.id,
                response: response_tx,
            })
            .await
            .map_err(|_| zbus::fdo::Error::Failed("Engine not available".to_string()))?;

        let response = response_rx
            .await
            .map_err(|_| zbus::fdo::Error::Failed("Engine response failed".to_string()))?;

        unim_log!(
            "DBUS",
            "[DBus] GetHanjaCandidates: target='{}', count={}, top_row='{}'",
            response.target,
            response.candidates.len(),
            response.top_row
        );

        // Standalone 모드: unim-gui-gtk가 팝업을 표시하도록 시그널 발행
        if !response.candidates.is_empty()
            && Config::load_from_default_path().engine.popup_mode == PopupMode::Standalone
        {
            let (x, y, w, h) = *self.cursor_rect.lock().unwrap();
            Self::show_hanja_popup(
                &signal_ctx,
                &response.target,
                response.candidates.clone(),
                &response.top_row,
                x,
                y,
                w,
                h,
            )
            .await
            .ok();
            // 통합 popup_render — 헤더/푸터/셀/expand 아이콘 등 즉시 렌더.
            if let Some(rs) = &response.render_state {
                Self::emit_popup_render(&signal_ctx, rs).await;
            }
            unim_log!(
                "DBUS",
                "[DBus] GetHanjaCandidates -> ShowHanjaPopup 시그널 발행 (Standalone)"
            );
        }

        Ok((response.target, response.candidates))
    }

    /// 한자 선택
    /// 반환값: 선택된 한자 (실패 시 빈 문자열)
    async fn select_hanja(
        &self,
        #[zbus(signal_context)] _signal_ctx: SignalContext<'_>,
        index: u32,
    ) -> zbus::fdo::Result<String> {
        let (response_tx, response_rx) = oneshot::channel();

        self.engine_tx
            .send(EngineRequest::SelectHanja {
                context_id: self.id,
                index: index as usize,
                response: response_tx,
            })
            .await
            .map_err(|_| zbus::fdo::Error::Failed("Engine not available".to_string()))?;

        let hanja = response_rx
            .await
            .map_err(|_| zbus::fdo::Error::Failed("Engine response failed".to_string()))?
            .unwrap_or_default();

        // CommitText / HidePopup 을 popup-owner path 로 redirect 발행 — GNOME extension 자체
        // context 에서 RPC 가 들어와도 commit 은 GTK4_IM 등 실제 입력 대상 path 로 보낸다.
        // last_active path 가 없으면 self.path 로 fallback (기존 동일 path 동작 보존).
        self.redirect_commit_and_hide(&hanja).await;

        unim_log!(
            "DBUS",
            "[DBus] SelectHanja: index={}, result='{}'",
            index,
            hanja
        );

        Ok(hanja)
    }

    /// 현재 한자 후보 목록의 즐겨찾기 상태 조회
    ///
    /// 반환값은 `GetHanjaCandidates` 응답의 `candidates` 와 동일한 순서이며
    /// 각 후보의 즐겨찾기 여부(true=등록됨)를 나타낸다.
    async fn get_hanja_bookmark_states(&self) -> zbus::fdo::Result<Vec<bool>> {
        let (response_tx, response_rx) = oneshot::channel();

        self.engine_tx
            .send(EngineRequest::GetHanjaBookmarkStates {
                context_id: self.id,
                response: response_tx,
            })
            .await
            .map_err(|_| zbus::fdo::Error::Failed("Engine not available".to_string()))?;

        let states = response_rx
            .await
            .map_err(|_| zbus::fdo::Error::Failed("Engine response failed".to_string()))?;

        unim_log!(
            "DBUS",
            "[DBus] GetHanjaBookmarkStates: count={}",
            states.len()
        );
        Ok(states)
    }

    /// 한자 후보 즐겨찾기 토글
    ///
    /// 반환값: `(new_index, bookmarked)` — 토글 후 즐겨찾기 우선 정렬이
    /// 자동 적용되므로 인덱스가 바뀔 수 있다. 한자 모드가 아니거나 인덱스가
    /// 범위를 벗어나면 실패(Error).
    ///
    /// 토글 직후 후보 재정렬·커서 점프 정보를 담은
    /// `HanjaCandidatesReordered` 시그널을 발행한다.
    async fn toggle_hanja_bookmark(
        &self,
        #[zbus(signal_context)] signal_ctx: SignalContext<'_>,
        index: u32,
    ) -> zbus::fdo::Result<(u32, bool)> {
        let (response_tx, response_rx) = oneshot::channel();

        self.engine_tx
            .send(EngineRequest::ToggleHanjaBookmark {
                context_id: self.id,
                index: index as usize,
                response: response_tx,
            })
            .await
            .map_err(|_| zbus::fdo::Error::Failed("Engine not available".to_string()))?;

        let (idx, bookmarked, popup_action, render_state) = response_rx
            .await
            .map_err(|_| zbus::fdo::Error::Failed("Engine response failed".to_string()))?
            .ok_or_else(|| zbus::fdo::Error::Failed("한자 모드 아님 또는 범위 밖".to_string()))?;

        // 호환성을 위해 단일 인덱스 시그널도 함께 발행 (구버전 frontend용)
        Self::hanja_bookmark_changed(&signal_ctx, idx as u32, bookmarked)
            .await
            .ok();

        // 정렬·커서 시그널 발행 — 신버전 frontend는 이걸 받아 일괄 갱신
        if let Some(PopupAction::HanjaCandidatesReordered {
            target,
            candidates,
            bookmarks,
            new_cursor,
            page,
            sel_row,
            sel_col,
            bookmarked: bm,
            was_bookmarked: was_bm,
        }) = popup_action
        {
            let hanjas: Vec<String> = candidates.iter().map(|(h, _)| h.clone()).collect();
            let meanings: Vec<String> = candidates.iter().map(|(_, m)| m.clone()).collect();
            Self::hanja_candidates_reordered(
                &signal_ctx,
                &target,
                hanjas,
                meanings,
                bookmarks,
                new_cursor as u32,
                page as i32,
                sel_row as i32,
                sel_col as i32,
                bm,
                was_bm,
            )
            .await
            .ok();
        }

        // 통합 popup_render — frontend 가 이걸로 즐겨찾기 토글 + 재정렬 일괄 적용.
        if let Some(rs) = render_state {
            Self::emit_popup_render(&signal_ctx, &rs).await;
        }

        unim_log!(
            "DBUS",
            "[DBus] ToggleHanjaBookmark: index={}, bookmarked={}",
            idx,
            bookmarked
        );
        Ok((idx as u32, bookmarked))
    }

    /// 팝업 페이지 이동 (마우스 ◀/▶ 버튼 클릭용).
    ///
    /// `direction`: -1 또는 0 = 이전 페이지, +1 (그 외) = 다음 페이지.
    /// 첫/마지막 페이지에서 wrap-around 한다. cursor (sel_row, sel_col) 은
    /// 가능한 한 보존된다 (셀이 없으면 마지막 유효 위치로 보정).
    ///
    /// 한자/특수문자/이모지 모든 popup 종류에서 동작. popup 이 활성 아니거나
    /// total_pages <= 1 이면 no-op (시그널 미발행). 페이지가 바뀌면 기존
    /// `PopupNavigate` 시그널을 발행해 frontend 가 일괄 갱신하도록 한다.
    async fn popup_change_page(
        &self,
        #[zbus(signal_context)] signal_ctx: SignalContext<'_>,
        direction: i32,
    ) -> zbus::fdo::Result<()> {
        let (response_tx, response_rx) = oneshot::channel();

        self.engine_tx
            .send(EngineRequest::PopupChangePage {
                context_id: self.id,
                direction,
                response: response_tx,
            })
            .await
            .map_err(|_| zbus::fdo::Error::Failed("Engine not available".to_string()))?;

        let payload = response_rx
            .await
            .map_err(|_| zbus::fdo::Error::Failed("Engine response failed".to_string()))?;

        if let Some(rs) = payload {
            // Legacy popup_navigate (frontend 점진 마이그레이션 동안 dual-emit).
            Self::popup_navigate(
                &signal_ctx,
                rs.current_page as i32,
                rs.total_pages as i32,
                rs.sel_row as i32,
                rs.rows as i32,
                rs.cols as i32,
                rs.sel_row as i32,
                rs.sel_col as i32,
            )
            .await
            .ok();
            // 통합 popup_render — frontend 가 본 시그널만 구독해도 헤더/푸터/셀/탭 모두 갱신.
            Self::emit_popup_render(&signal_ctx, &rs).await;
            unim_log!(
                "DBUS",
                "[DBus] PopupChangePage: dir={}, page={}/{}, sel=({},{})",
                direction,
                rs.current_page,
                rs.total_pages,
                rs.sel_row,
                rs.sel_col
            );
        } else {
            unim_log!(
                "DBUS",
                "[DBus] PopupChangePage: dir={}, no-op (single page or no popup)",
                direction
            );
        }
        Ok(())
    }

    /// 한자 popup 확장 모드 토글 (마우스 ⊞/⊟ 아이콘 클릭용).
    ///
    /// 호출 context 가 popup-owner 와 다를 수 있으므로 engine_worker 가
    /// `resolve_popup_owner` 로 보정. 토글 후 변경된 layout (cols=1↔9) 을
    /// `PopupNavigate` 시그널로 발행해 frontend 가 그리드 재구성.
    /// 활성 한자 popup 이 없으면 no-op.
    async fn toggle_popup_expand(
        &self,
        #[zbus(signal_context)] signal_ctx: SignalContext<'_>,
    ) -> zbus::fdo::Result<()> {
        let (response_tx, response_rx) = oneshot::channel();

        self.engine_tx
            .send(EngineRequest::TogglePopupExpand {
                context_id: self.id,
                response: response_tx,
            })
            .await
            .map_err(|_| zbus::fdo::Error::Failed("Engine not available".to_string()))?;

        let payload = response_rx
            .await
            .map_err(|_| zbus::fdo::Error::Failed("Engine response failed".to_string()))?;

        if let Some(rs) = payload {
            // Legacy popup_navigate (dual-emit).
            Self::popup_navigate(
                &signal_ctx,
                rs.current_page as i32,
                rs.total_pages as i32,
                rs.sel_row as i32,
                rs.rows as i32,
                rs.cols as i32,
                rs.sel_row as i32,
                rs.sel_col as i32,
            )
            .await
            .ok();
            // 통합 popup_render.
            Self::emit_popup_render(&signal_ctx, &rs).await;
            unim_log!(
                "DBUS",
                "[DBus] TogglePopupExpand: page={}/{}, cols={} (1=compact, 9=expanded)",
                rs.current_page,
                rs.total_pages,
                rs.cols
            );
        } else {
            unim_log!("DBUS", "[DBus] TogglePopupExpand: no-op (no active popup)");
        }
        Ok(())
    }

    /// 한자 모드 취소 (남은 preedit을 커밋하고 반환)
    async fn cancel_hanja(
        &self,
        #[zbus(signal_context)] _signal_ctx: SignalContext<'_>,
    ) -> zbus::fdo::Result<String> {
        let (response_tx, response_rx) = oneshot::channel();

        self.engine_tx
            .send(EngineRequest::CancelHanja {
                context_id: self.id,
                response: response_tx,
            })
            .await
            .ok();

        let commit_text = if let Ok(Some(preedit)) = response_rx.await {
            // CommitText 를 popup-owner path 로 redirect (select_hanja 와 동일 사유).
            // HidePopup 도 함께 발행 — popup 외부 마우스 클릭 cancel 후 GTK4_IM 모듈이
            // popup 을 닫게 한다.
            if !preedit.is_empty() {
                self.redirect_commit_and_hide(&preedit).await;
            }
            preedit
        } else {
            String::new()
        };

        unim_log!(
            "DBUS",
            "[DBus] CancelHanja: context_id={}, commit='{}'",
            self.id,
            commit_text
        );
        Ok(commit_text)
    }

    // =========================================
    // 특수문자 변환 관련 메서드
    // =========================================

    /// 특수문자 후보 목록 조회
    /// 반환값: (변환 대상 초성, [특수문자, ...], 상단 행 레이블)
    async fn get_special_char_candidates(
        &self,
        #[zbus(signal_context)] signal_ctx: SignalContext<'_>,
    ) -> zbus::fdo::Result<(String, Vec<String>, String)> {
        let (response_tx, response_rx) = oneshot::channel();

        self.engine_tx
            .send(EngineRequest::GetSpecialCharCandidates {
                context_id: self.id,
                response: response_tx,
            })
            .await
            .map_err(|_| zbus::fdo::Error::Failed("Engine not available".to_string()))?;

        let response = response_rx
            .await
            .map_err(|_| zbus::fdo::Error::Failed("Engine response failed".to_string()))?;

        unim_log!(
            "DBUS",
            "[DBus] GetSpecialCharCandidates: target='{}', count={}, top_row='{}'",
            response.target,
            response.characters.len(),
            response.top_row
        );

        // Standalone 모드: unim-gui-gtk가 팝업을 표시하도록 시그널 발행
        if !response.characters.is_empty()
            && Config::load_from_default_path().engine.popup_mode == PopupMode::Standalone
        {
            let (x, y, w, h) = *self.cursor_rect.lock().unwrap();
            Self::show_special_popup(
                &signal_ctx,
                &response.target,
                response.characters.clone(),
                &response.top_row,
                x,
                y,
                w,
                h,
            )
            .await
            .ok();
            // 통합 popup_render.
            if let Some(rs) = &response.render_state {
                Self::emit_popup_render(&signal_ctx, rs).await;
            }
            unim_log!(
                "DBUS",
                "[DBus] GetSpecialCharCandidates -> ShowSpecialPopup 시그널 발행 (Standalone)"
            );
        }

        Ok((response.target, response.characters, response.top_row))
    }

    /// 특수문자 선택
    /// 반환값: 선택된 특수문자 (실패 시 빈 문자열)
    async fn select_special_char(
        &self,
        #[zbus(signal_context)] _signal_ctx: SignalContext<'_>,
        index: u32,
    ) -> zbus::fdo::Result<String> {
        let (response_tx, response_rx) = oneshot::channel();

        self.engine_tx
            .send(EngineRequest::SelectSpecialChar {
                context_id: self.id,
                index: index as usize,
                response: response_tx,
            })
            .await
            .map_err(|_| zbus::fdo::Error::Failed("Engine not available".to_string()))?;

        let ch = response_rx
            .await
            .map_err(|_| zbus::fdo::Error::Failed("Engine response failed".to_string()))?
            .unwrap_or_default();

        // CommitText / HidePopup 을 popup-owner path 로 redirect (select_hanja 와 동일).
        self.redirect_commit_and_hide(&ch).await;

        unim_log!(
            "DBUS",
            "[DBus] SelectSpecialChar: index={}, result='{}'",
            index,
            ch
        );

        Ok(ch)
    }

    /// 특수문자 모드 취소 (남은 preedit을 커밋하고 반환)
    async fn cancel_special_char(
        &self,
        #[zbus(signal_context)] _signal_ctx: SignalContext<'_>,
    ) -> zbus::fdo::Result<String> {
        let (response_tx, response_rx) = oneshot::channel();

        self.engine_tx
            .send(EngineRequest::CancelSpecialChar {
                context_id: self.id,
                response: response_tx,
            })
            .await
            .ok();

        let commit_text = if let Ok(Some(preedit)) = response_rx.await {
            if !preedit.is_empty() {
                self.redirect_commit_and_hide(&preedit).await;
            }
            preedit
        } else {
            String::new()
        };

        unim_log!(
            "DBUS",
            "[DBus] CancelSpecialChar: context_id={}, commit='{}'",
            self.id,
            commit_text
        );
        Ok(commit_text)
    }

    /// 이모지 커밋 (GUI 팝업에서 선택한 이모지를 현재 컨텍스트로 커밋)
    ///
    /// `HidePopup` 시그널도 함께 발행하여 다른 구독자의 팝업 상태를 정리합니다.
    async fn commit_emoji(
        &self,
        #[zbus(signal_context)] signal_ctx: SignalContext<'_>,
        emoji: &str,
    ) -> zbus::fdo::Result<()> {
        if !emoji.is_empty() {
            Self::commit_text(&signal_ctx, emoji).await.ok();
            unim::emoji::touch_recent(emoji);
        }
        Self::hide_popup(&signal_ctx).await.ok();
        unim_log!(
            "DBUS",
            "[DBus] CommitEmoji: context_id={}, emoji='{}'",
            self.id,
            emoji
        );
        Ok(())
    }
}
