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
use unim::config::{Config, InputCategory};
use unim::unim_log;

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
    /// 리셋
    Reset { context_id: u32 },
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
    /// 한자 모드 취소
    CancelHanja { context_id: u32 },
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
    /// 특수문자 모드 취소
    CancelSpecialChar { context_id: u32 },
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
        let handler = InputContextHandler::new(id, self.engine_tx.clone(), self.connection.clone());
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
    /// 엔진 스레드로 요청을 보내는 채널
    engine_tx: mpsc::Sender<EngineRequest>,
    /// DBus 연결 (시그널 발송용)
    connection: Connection,
}

impl InputContextHandler {
    /// 새 핸들러 생성
    pub fn new(id: u32, engine_tx: mpsc::Sender<EngineRequest>, connection: Connection) -> Self {
        Self {
            id,
            engine_tx,
            connection,
        }
    }
}

#[interface(name = "org.atit.unim.InputContext")]
impl InputContextHandler {
    /// 키 이벤트 처리
    /// 반환값: (consumed, preedit, commit)
    async fn process_key_event(
        &self,
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
            // InputMethod 경로에서 GlobalModeChanged 시그널 발송
            let signal_ctx = zbus::SignalContext::new(&self.connection, crate::INPUT_METHOD_PATH)
                .map_err(|e| {
                zbus::fdo::Error::Failed(format!("Signal context error: {}", e))
            })?;
            InputMethodService::global_mode_changed(&signal_ctx, is_korean)
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

        if let Ok(is_korean) = response_rx.await {
            // InputMethod 경로에서 GlobalModeChanged 시그널 발송 (UI 동기화)
            unim_log!(
                "DBUS",
                "[DBus] FocusIn: context_id={}, window_id={}, mode={}",
                self.id,
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
            unim_log!("DBUS", "[DBus] FocusIn: context_id={}", self.id);
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

    /// 입력 상태 초기화
    async fn reset(
        &self,
        #[zbus(signal_context)] signal_ctx: SignalContext<'_>,
    ) -> zbus::fdo::Result<()> {
        self.engine_tx
            .send(EngineRequest::Reset {
                context_id: self.id,
            })
            .await
            .ok();
        Self::update_preedit_text(&signal_ctx, "", 0, false)
            .await
            .ok();
        unim_log!("DBUS", "[DBus] Reset: context_id={}", self.id);
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
    // 한자 변환 관련 메서드
    // =========================================

    /// 한자 후보 목록 조회
    /// 반환값: (변환 대상, [(한자, 뜻풀이), ...])
    async fn get_hanja_candidates(&self) -> zbus::fdo::Result<(String, Vec<(String, String)>)> {
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

        Ok((response.target, response.candidates))
    }

    /// 한자 선택
    /// 반환값: 선택된 한자 (실패 시 빈 문자열)
    async fn select_hanja(&self, index: u32) -> zbus::fdo::Result<String> {
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

        unim_log!(
            "DBUS",
            "[DBus] SelectHanja: index={}, result='{}'",
            index,
            hanja
        );

        Ok(hanja)
    }

    /// 한자 모드 취소
    async fn cancel_hanja(&self) -> zbus::fdo::Result<()> {
        self.engine_tx
            .send(EngineRequest::CancelHanja {
                context_id: self.id,
            })
            .await
            .ok();

        unim_log!("DBUS", "[DBus] CancelHanja: context_id={}", self.id);
        Ok(())
    }

    // =========================================
    // 특수문자 변환 관련 메서드
    // =========================================

    /// 특수문자 후보 목록 조회
    /// 반환값: (변환 대상 초성, [특수문자, ...], 상단 행 레이블)
    async fn get_special_char_candidates(
        &self,
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

        Ok((response.target, response.characters, response.top_row))
    }

    /// 특수문자 선택
    /// 반환값: 선택된 특수문자 (실패 시 빈 문자열)
    async fn select_special_char(&self, index: u32) -> zbus::fdo::Result<String> {
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

        unim_log!(
            "DBUS",
            "[DBus] SelectSpecialChar: index={}, result='{}'",
            index,
            ch
        );

        Ok(ch)
    }

    /// 특수문자 모드 취소
    async fn cancel_special_char(&self) -> zbus::fdo::Result<()> {
        self.engine_tx
            .send(EngineRequest::CancelSpecialChar {
                context_id: self.id,
            })
            .await
            .ok();

        unim_log!("DBUS", "[DBus] CancelSpecialChar: context_id={}", self.id);
        Ok(())
    }
}
