//! `org.freedesktop.IBus.InputContext` 구현
//!
//! IBus 클라이언트의 키 이벤트, 포커스, 커서 위치를 처리하고
//! CommitText, UpdatePreeditText 시그널을 발행한다.

use tokio::sync::mpsc;
use zbus::interface;
use zbus::zvariant::{ObjectPath, Value};

use unim::unim_log;

use super::ibus_types;
use crate::service::{EngineRequest, EngineResponse};

/// IBus InputContext 핸들러
pub struct IBusInputContextHandler {
    /// UNIM 엔진 컨텍스트 ID
    context_id: u32,
    /// 엔진 워커 채널
    engine_tx: mpsc::Sender<EngineRequest>,
    /// zbus 연결 (destroy 시 object 제거용)
    connection: zbus::Connection,
    /// 이 핸들러의 object path
    object_path: String,
    /// 커서 위치
    cursor_x: i32,
    cursor_y: i32,
    /// 윈도우 ID (FocusIn용)
    window_id: String,
}

impl IBusInputContextHandler {
    pub fn new(
        context_id: u32,
        object_path: &str,
        engine_tx: mpsc::Sender<EngineRequest>,
        connection: zbus::Connection,
    ) -> Self {
        Self {
            context_id,
            engine_tx,
            connection,
            object_path: object_path.to_string(),
            cursor_x: 0,
            cursor_y: 0,
            window_id: format!("ibus-ctx-{}", context_id),
        }
    }

    /// 엔진 응답에서 IBus 시그널 발행
    async fn emit_engine_response(
        &self,
        response: &EngineResponse,
        signal_ctx: &zbus::SignalContext<'_>,
    ) {
        // CommitText
        if let Some(ref commit) = response.commit {
            if !commit.is_empty() {
                let text = ibus_types::serialize_ibus_text(commit);
                if let Err(e) = Self::commit_text(signal_ctx, text).await {
                    unim_log!("DAEMON", "[IBus Compat] CommitText 시그널 실패: {}", e);
                }
            }
        }

        // UpdatePreeditText
        match &response.preedit {
            Some(preedit) if !preedit.is_empty() => {
                let text = ibus_types::serialize_preedit_text(preedit);
                let cursor_pos = preedit.chars().count() as u32;
                let _ = Self::update_preedit_text(signal_ctx, text, cursor_pos, true).await;
                let _ = Self::show_preedit_text(signal_ctx).await;
            }
            _ => {
                // preedit 비어있으면 숨김
                if response.preedit.is_some() {
                    let text = ibus_types::serialize_ibus_text("");
                    let _ = Self::update_preedit_text(signal_ctx, text, 0, false).await;
                    let _ = Self::hide_preedit_text(signal_ctx).await;
                }
            }
        }

        // ForwardKeyEvent (consumed=false인 경우 클라이언트에서 처리)
        // IBus에서는 ProcessKeyEvent의 반환값으로 처리하므로 별도 시그널 불필요
    }
}

#[interface(name = "org.freedesktop.IBus.InputContext")]
impl IBusInputContextHandler {
    /// 키 이벤트 처리
    ///
    /// IBus 프로토콜: keyval, keycode, state → consumed (bool)
    /// state bit 30 (1<<30) = IBus RELEASE flag
    #[zbus(name = "ProcessKeyEvent")]
    async fn process_key_event(
        &self,
        #[zbus(signal_context)] signal_ctx: zbus::SignalContext<'_>,
        keyval: u32,
        keycode: u32,
        state: u32,
    ) -> zbus::fdo::Result<bool> {
        // IBus key release flag (bit 30) → 릴리스 이벤트는 무시
        if state & (1 << 30) != 0 {
            return Ok(false);
        }

        // IBus keycode = X11 keycode = evdev + 8 → evdev로 변환
        let evdev_keycode = keycode.saturating_sub(8);

        let (response_tx, response_rx) = tokio::sync::oneshot::channel();
        self.engine_tx
            .send(EngineRequest::ProcessKey {
                context_id: self.context_id,
                keyval,
                keycode: evdev_keycode,
                state,
                response: response_tx,
            })
            .await
            .map_err(|e| zbus::fdo::Error::Failed(format!("Engine error: {}", e)))?;

        let response: EngineResponse = response_rx
            .await
            .map_err(|e| zbus::fdo::Error::Failed(format!("Response error: {}", e)))?;

        self.emit_engine_response(&response, &signal_ctx).await;

        Ok(response.consumed)
    }

    /// 포커스 인
    #[zbus(name = "FocusIn")]
    async fn focus_in(&mut self) -> zbus::fdo::Result<()> {
        let (response_tx, response_rx) = tokio::sync::oneshot::channel();
        let _ = self
            .engine_tx
            .send(EngineRequest::FocusIn {
                context_id: self.context_id,
                window_id: self.window_id.clone(),
                response: response_tx,
            })
            .await;
        let _ = response_rx.await;
        Ok(())
    }

    /// 포커스 아웃
    #[zbus(name = "FocusOut")]
    async fn focus_out(
        &self,
        #[zbus(signal_context)] signal_ctx: zbus::SignalContext<'_>,
    ) -> zbus::fdo::Result<()> {
        let (response_tx, response_rx) = tokio::sync::oneshot::channel();
        let _ = self
            .engine_tx
            .send(EngineRequest::FocusOut {
                context_id: self.context_id,
                response: response_tx,
            })
            .await;

        // 잔여 preedit 커밋
        if let Ok(Some(commit)) = response_rx.await {
            if !commit.is_empty() {
                let text = ibus_types::serialize_ibus_text(&commit);
                let _ = Self::commit_text(&signal_ctx, text).await;
                let empty = ibus_types::serialize_ibus_text("");
                let _ = Self::update_preedit_text(&signal_ctx, empty, 0, false).await;
                let _ = Self::hide_preedit_text(&signal_ctx).await;
            }
        }
        Ok(())
    }

    /// 리셋 (팝업 취소 + 조합 커밋)
    #[zbus(name = "Reset")]
    async fn reset(
        &self,
        #[zbus(signal_context)] signal_ctx: zbus::SignalContext<'_>,
    ) -> zbus::fdo::Result<()> {
        let (response_tx, response_rx) = tokio::sync::oneshot::channel();
        let _ = self
            .engine_tx
            .send(EngineRequest::Reset {
                context_id: self.context_id,
                response: response_tx,
            })
            .await;

        if let Ok(Some(commit)) = response_rx.await {
            if !commit.is_empty() {
                let text = ibus_types::serialize_ibus_text(&commit);
                let _ = Self::commit_text(&signal_ctx, text).await;
                let empty = ibus_types::serialize_ibus_text("");
                let _ = Self::update_preedit_text(&signal_ctx, empty, 0, false).await;
                let _ = Self::hide_preedit_text(&signal_ctx).await;
            }
        }
        Ok(())
    }

    /// 클라이언트 기능 플래그 설정
    #[zbus(name = "SetCapabilities")]
    async fn set_capabilities(&self, _caps: u32) -> zbus::fdo::Result<()> {
        Ok(())
    }

    /// 커서 위치 설정
    #[zbus(name = "SetCursorLocation")]
    async fn set_cursor_location(
        &mut self,
        x: i32,
        y: i32,
        w: i32,
        h: i32,
    ) -> zbus::fdo::Result<()> {
        self.cursor_x = x;
        self.cursor_y = y;
        let _ = self
            .engine_tx
            .send(EngineRequest::ReportCursorRect {
                context_id: self.context_id,
                x,
                y,
                width: w,
                height: h,
            })
            .await;
        Ok(())
    }

    /// 상대 커서 위치 설정
    #[zbus(name = "SetCursorLocationRelative")]
    async fn set_cursor_location_relative(
        &mut self,
        x: i32,
        y: i32,
        w: i32,
        h: i32,
    ) -> zbus::fdo::Result<()> {
        // 상대 좌표 → 절대 좌표로 변환 (간단한 구현)
        self.set_cursor_location(x, y, w, h).await
    }

    /// 컨텍스트 파괴
    #[zbus(name = "Destroy")]
    async fn destroy(&self) -> zbus::fdo::Result<()> {
        let _ = self
            .engine_tx
            .send(EngineRequest::DestroyContext {
                id: self.context_id,
            })
            .await;

        // object server에서 핸들러 제거
        if let Ok(path) = ObjectPath::try_from(self.object_path.as_str()) {
            let _ = self
                .connection
                .object_server()
                .remove::<IBusInputContextHandler, _>(path)
                .await;
        }

        unim_log!(
            "DAEMON",
            "[IBus Compat] InputContext 파괴: id={}",
            self.context_id
        );
        Ok(())
    }

    /// 엔진 활성 여부
    #[zbus(name = "IsEnabled")]
    async fn is_enabled(&self) -> bool {
        true
    }

    /// 엔진 활성화 (stub)
    #[zbus(name = "Enable")]
    async fn enable(&self) -> zbus::fdo::Result<()> {
        Ok(())
    }

    /// 엔진 비활성화 (stub)
    #[zbus(name = "Disable")]
    async fn disable(&self) -> zbus::fdo::Result<()> {
        Ok(())
    }

    /// 엔진 설정 (stub)
    #[zbus(name = "SetEngine")]
    async fn set_engine(&self, _engine_name: &str) -> zbus::fdo::Result<()> {
        Ok(())
    }

    /// 현재 엔진 조회 (stub)
    #[zbus(name = "GetEngine")]
    async fn get_engine(&self) -> Value<'_> {
        ibus_types::serialize_engine_desc()
    }

    /// 콘텐츠 타입 설정
    #[zbus(name = "SetContentType")]
    async fn set_content_type(&self, purpose: u32, _hints: u32) -> zbus::fdo::Result<()> {
        let _ = self
            .engine_tx
            .send(EngineRequest::SetContentType {
                context_id: self.context_id,
                purpose,
            })
            .await;
        Ok(())
    }

    /// Surrounding text 설정
    #[zbus(name = "SetSurroundingText")]
    async fn set_surrounding_text(
        &self,
        text: Value<'_>,
        cursor_index: u32,
        anchor_index: u32,
    ) -> zbus::fdo::Result<()> {
        // IBusText에서 문자열 추출
        let text_str = extract_ibus_text_string(&text).unwrap_or_default();
        let _ = self
            .engine_tx
            .send(EngineRequest::SetSurroundingText {
                context_id: self.context_id,
                text: text_str,
                cursor_pos: cursor_index,
                anchor_pos: anchor_index,
            })
            .await;
        Ok(())
    }

    /// 속성 활성화 (stub)
    #[zbus(name = "PropertyActivate")]
    async fn property_activate(&self, _prop_name: &str, _state: i32) -> zbus::fdo::Result<()> {
        Ok(())
    }

    // ─── 시그널 ───

    /// 확정 텍스트 시그널
    #[zbus(signal, name = "CommitText")]
    async fn commit_text(ctx: &zbus::SignalContext<'_>, text: Value<'_>) -> zbus::Result<()>;

    /// Preedit 업데이트 시그널
    #[zbus(signal, name = "UpdatePreeditText")]
    async fn update_preedit_text(
        ctx: &zbus::SignalContext<'_>,
        text: Value<'_>,
        cursor_pos: u32,
        visible: bool,
    ) -> zbus::Result<()>;

    /// Preedit 표시 시그널
    #[zbus(signal, name = "ShowPreeditText")]
    async fn show_preedit_text(ctx: &zbus::SignalContext<'_>) -> zbus::Result<()>;

    /// Preedit 숨김 시그널
    #[zbus(signal, name = "HidePreeditText")]
    async fn hide_preedit_text(ctx: &zbus::SignalContext<'_>) -> zbus::Result<()>;

    /// 키 이벤트 전달 시그널
    #[zbus(signal, name = "ForwardKeyEvent")]
    async fn forward_key_event(
        ctx: &zbus::SignalContext<'_>,
        keyval: u32,
        keycode: u32,
        state: u32,
    ) -> zbus::Result<()>;

    /// 엔진 활성화 시그널
    #[zbus(signal, name = "Enabled")]
    async fn enabled_signal(ctx: &zbus::SignalContext<'_>) -> zbus::Result<()>;

    /// 엔진 비활성화 시그널
    #[zbus(signal, name = "Disabled")]
    async fn disabled_signal(ctx: &zbus::SignalContext<'_>) -> zbus::Result<()>;
}

/// IBusText GVariant에서 문자열 추출
fn extract_ibus_text_string(value: &Value<'_>) -> Option<String> {
    // IBusText = (sa{sv}sv) → 3번째 필드가 문자열
    match value {
        Value::Structure(s) => {
            let fields = s.fields();
            if fields.len() >= 3 {
                match &fields[2] {
                    Value::Str(s) => Some(s.to_string()),
                    Value::Value(inner) => {
                        if let Value::Str(s) = inner.as_ref() {
                            Some(s.to_string())
                        } else {
                            None
                        }
                    }
                    _ => None,
                }
            } else {
                None
            }
        }
        Value::Value(inner) => extract_ibus_text_string(inner),
        _ => None,
    }
}
