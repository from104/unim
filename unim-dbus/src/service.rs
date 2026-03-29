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
    SetContentType {
        context_id: u32,
        purpose: u32,
    },
    /// Surrounding text 설정
    SetSurroundingText {
        context_id: u32,
        text: String,
        cursor_pos: u32,
        anchor_pos: u32,
    },
    /// TypeFix 변환 요청
    TypeFix {
        context_id: u32,
        direction: u32,
        response: oneshot::Sender<Option<(u32, String)>>,
    },
    /// Smart Backspace 요청
    SmartBackspace {
        context_id: u32,
        response: oneshot::Sender<Option<(u32, String)>>,
    },
    /// 이모지 검색
    SearchEmoji {
        keyword: String,
        response: oneshot::Sender<Vec<String>>,
    },
}

/// 한자 후보 응답
#[derive(Debug)]
pub struct HanjaCandidateResponse {
    /// 변환 대상 문자열
    pub target: String,
    /// 후보 목록 (한자, 뜻풀이)
    pub candidates: Vec<(String, String)>,
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
    /// TypeFix 더블탭 결과 (Some((삭제 문자 수, 대체 텍스트)))
    pub typefix_result: Option<(u32, String)>,
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
        }
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
            self.engine_tx.clone(),
            self.connection.clone(),
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

    /// 설정 변경 시그널
    #[zbus(signal)]
    async fn config_changed(
        signal_ctx: &SignalContext<'_>,
        key: &str,
        value: &str,
    ) -> zbus::Result<()>;

    /// 이모지 검색
    /// keyword가 빈 문자열이면 인기 이모지를 반환합니다.
    async fn search_emoji(&self, keyword: &str) -> zbus::fdo::Result<Vec<String>> {
        let results = unim::hangul::emoji::search_emoji(keyword);
        let emoji_strings: Vec<String> = results.iter().map(|c| c.to_string()).collect();
        unim_log!(
            "DBUS",
            "[DBus] SearchEmoji: keyword='{}', count={}",
            keyword,
            emoji_strings.len()
        );
        Ok(emoji_strings)
    }

    /// 설정값 조회
    async fn get_config(&self, key: &str) -> zbus::fdo::Result<String> {
        let config = self.config.read().await;
        let value = match key {
            "korean_layout" => config.engine.korean.layout.name().to_string(),
            "english_layout" => config.engine.english.layout.name().to_string(),
            "default_category" => match config.engine.default_category {
                InputCategory::Korean => "Korean".to_string(),
                InputCategory::English => "English".to_string(),
            },
            "mode_sharing" => match config.engine.mode_sharing {
                unim::config::ModeSharingMode::Global => "Global".to_string(),
                unim::config::ModeSharingMode::PerApp => "PerApp".to_string(),
                unim::config::ModeSharingMode::PerWindow => "PerWindow".to_string(),
            },
            "auto_switch_enabled" => config.engine.auto_switch.enabled.to_string(),
            "auto_switch_threshold" => config.engine.auto_switch.threshold.to_string(),
            "toggle_keys" => config.engine.toggle_keys.join(","),
            "hanja_keys" => config.engine.hanja_keys.join(","),
            "popup_mode" => config.engine.popup_mode.name().to_string(),
            "app_rules" => serde_json::to_string(&config.engine.app_rules)
                .unwrap_or_default(),
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
                    config.engine.korean.layout = match value {
                        "Dubeolsik" => unim::config::KoreanLayout::Dubeolsik,
                        "Sebeolsik390" => unim::config::KoreanLayout::Sebeolsik390,
                        "Sebeolsik391" => unim::config::KoreanLayout::Sebeolsik391,
                        "SebeolsikNoShift" => unim::config::KoreanLayout::SebeolsikNoShift,
                        _ => {
                            return Err(zbus::fdo::Error::InvalidArgs(format!(
                                "Invalid value: {}",
                                value
                            )))
                        }
                    };
                }
                "english_layout" => {
                    config.engine.english.layout = match value {
                        "Qwerty" => unim::config::EnglishLayout::Qwerty,
                        "Dvorak" => unim::config::EnglishLayout::Dvorak,
                        "Colemak" => unim::config::EnglishLayout::Colemak,
                        "ColemakDh" => unim::config::EnglishLayout::ColemakDh,
                        "Workman" => unim::config::EnglishLayout::Workman,
                        _ => {
                            return Err(zbus::fdo::Error::InvalidArgs(format!(
                                "Invalid value: {}",
                                value
                            )))
                        }
                    };
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
                        "PerWindow" => unim::config::ModeSharingMode::PerWindow,
                        _ => {
                            return Err(zbus::fdo::Error::InvalidArgs(format!(
                                "Invalid value: {}",
                                value
                            )))
                        }
                    };
                }
                "auto_switch_enabled" => {
                    config.engine.auto_switch.enabled = value
                        .parse()
                        .map_err(|_| zbus::fdo::Error::InvalidArgs("Invalid bool".to_string()))?;
                }
                "auto_switch_threshold" => {
                    config.engine.auto_switch.threshold = value
                        .parse()
                        .map_err(|_| zbus::fdo::Error::InvalidArgs("Invalid float".to_string()))?;
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
    /// 캐싱된 커서 위치 (x, y, width, height)
    cursor_rect: std::sync::Mutex<(i32, i32, i32, i32)>,
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

impl InputContextHandler {
    /// 새 핸들러 생성
    pub fn new(
        id: u32,
        client_name: String,
        engine_tx: mpsc::Sender<EngineRequest>,
        connection: Connection,
    ) -> Self {
        Self {
            id,
            client_name,
            engine_tx,
            connection,
            cursor_rect: std::sync::Mutex::new((0, 0, 0, 0)),
        }
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
        if let Some(popup) = &response.popup_action {
            match popup {
                PopupAction::ShowHanja { target, candidates } => {
                    let (x, y, w, h) = *self.cursor_rect.lock().unwrap();
                    Self::show_hanja_popup(&signal_ctx, target, candidates.clone(), x, y, w, h)
                        .await
                        .ok();
                    unim_log!(
                        "DBUS",
                        "[DBus] ShowHanjaPopup 시그널 발행: target='{}', count={}",
                        target,
                        candidates.len()
                    );
                }
                PopupAction::ShowSpecial {
                    target,
                    characters,
                    top_row,
                } => {
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
            }
        }

        // TypeFix 더블탭 결과 처리 (delete_surrounding + commit)
        if let Some((delete_count, replacement)) = &response.typefix_result {
            unim_log!(
                "DBUS",
                "[DBus] TypeFix 더블탭 시그널: delete={}, replacement='{}'",
                delete_count,
                replacement
            );
            Self::delete_surrounding_text(
                &signal_ctx,
                -(*delete_count as i32),
                *delete_count,
            )
            .await
            .ok();
            if !replacement.is_empty() {
                Self::commit_text(&signal_ctx, replacement).await.ok();
            }
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

        // 시그널도 발송 (호환성 유지)
        if !commit.is_empty() {
            Self::commit_text(&signal_ctx, &commit).await.ok();
        }

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

        if !commit.is_empty() {
            Self::commit_text(&signal_ctx, &commit).await.ok();
        }
        Self::update_preedit_text(&signal_ctx, "", 0, false)
            .await
            .ok();

        unim_log!("DBUS", "[DBus] Reset: context_id={}, commit='{}'", self.id, commit);
        Ok(())
    }

    /// 컨텍스트 파괴
    async fn destroy(&self) -> zbus::fdo::Result<()> {
        self.engine_tx
            .send(EngineRequest::DestroyContext { id: self.id })
            .await
            .ok();
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
    #[zbus(signal)]
    async fn show_hanja_popup(
        signal_ctx: &SignalContext<'_>,
        target: &str,
        candidates: Vec<(String, String)>,
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

    // =========================================
    // 커서 위치 보고
    // =========================================

    /// 프런트엔드가 커서 위치를 보고 (팝업 포지셔닝용)
    async fn report_cursor_rect(
        &self,
        x: i32,
        y: i32,
        width: i32,
        height: i32,
    ) -> zbus::fdo::Result<()> {
        *self.cursor_rect.lock().unwrap() = (x, y, width, height);
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

    /// TypeFix 변환 (한/영 오타 변환)
    /// direction: 0=자동, 1=영→한, 2=한→영
    /// 반환값: (삭제할 문자 수, 대체 텍스트) 또는 빈 문자열
    async fn type_fix(
        &self,
        #[zbus(signal_context)] signal_ctx: SignalContext<'_>,
        direction: u32,
    ) -> zbus::fdo::Result<(u32, String)> {
        let (response_tx, response_rx) = oneshot::channel();

        self.engine_tx
            .send(EngineRequest::TypeFix {
                context_id: self.id,
                direction,
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
                "[DBus] TypeFix: delete={}, replacement='{}'",
                delete_chars,
                replacement
            );
            // delete_surrounding_text 시그널 발송하여 프론트엔드가 기존 텍스트를 삭제
            Self::delete_surrounding_text(&signal_ctx, -(delete_chars as i32), delete_chars)
                .await
                .ok();
            // 변환된 텍스트를 커밋
            Self::commit_text(&signal_ctx, &replacement).await.ok();
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
            "[DBus] GetHanjaCandidates: target='{}', count={}",
            response.target,
            response.candidates.len()
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
                x,
                y,
                w,
                h,
            )
            .await
            .ok();
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
        #[zbus(signal_context)] signal_ctx: SignalContext<'_>,
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

        // 선택된 한자를 CommitText 시그널로 프론트엔드에 전달
        if !hanja.is_empty() {
            Self::commit_text(&signal_ctx, &hanja).await.ok();
        }

        // 마우스 클릭 선택은 ProcessKeyEvent를 거치지 않으므로
        // HidePopup 시그널을 여기서 명시적으로 발행해야 함
        Self::hide_popup(&signal_ctx).await.ok();

        unim_log!(
            "DBUS",
            "[DBus] SelectHanja: index={}, result='{}'",
            index,
            hanja
        );

        Ok(hanja)
    }

    /// 한자 모드 취소 (남은 preedit을 커밋하고 반환)
    async fn cancel_hanja(
        &self,
        #[zbus(signal_context)] signal_ctx: SignalContext<'_>,
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
            if !preedit.is_empty() {
                Self::commit_text(&signal_ctx, &preedit).await.ok();
            }
            preedit
        } else {
            String::new()
        };

        unim_log!("DBUS", "[DBus] CancelHanja: context_id={}, commit='{}'", self.id, commit_text);
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
        #[zbus(signal_context)] signal_ctx: SignalContext<'_>,
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

        // 선택된 특수문자를 CommitText 시그널로 프론트엔드에 전달
        if !ch.is_empty() {
            Self::commit_text(&signal_ctx, &ch).await.ok();
        }

        // 마우스 클릭 선택은 ProcessKeyEvent를 거치지 않으므로
        // HidePopup 시그널을 여기서 명시적으로 발행해야 함
        Self::hide_popup(&signal_ctx).await.ok();

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
        #[zbus(signal_context)] signal_ctx: SignalContext<'_>,
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
                Self::commit_text(&signal_ctx, &preedit).await.ok();
            }
            preedit
        } else {
            String::new()
        };

        unim_log!("DBUS", "[DBus] CancelSpecialChar: context_id={}, commit='{}'", self.id, commit_text);
        Ok(commit_text)
    }
}
