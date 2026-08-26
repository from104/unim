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

/// IBusInputPurpose(값 체계는 GtkInputPurpose 와 완전 동일 — 로컬
/// gir1.2-ibus-1.0 1.5.29 실측: FREE_FORM 0, ALPHA 1, DIGITS 2, NUMBER 3,
/// PHONE 4, URL 5, EMAIL 6, NAME 7, PASSWORD 8, PIN 9, TERMINAL 10)
/// → UNIM `ContentPurpose` 원시값(u32) 변환.
///
/// 2026-07-26 감사 CONFIRMED: 개선 전에는 이 값이 무변환으로 넘어가
/// `ContentPurpose::from_u32`(0..=6 만 유효, 그 외는 Normal)에 들어가면서
/// PASSWORD(8)/PIN(9)이 Normal 로 소실(차단 완전 실패)되고 ALPHA(1)/DIGITS(2)가
/// 각각 Password/Pin 으로 오탐(평범한 필드에서 한글 차단)했다.
///
/// gtk3/src/immodule.c 의 `gtk3_input_purpose_to_unim`, gtk4/src/immodule.c 의
/// `gtk_input_purpose_to_unim` 과 반드시 1:1 동일해야 한다 — 같은 enum
/// (GtkInputPurpose == IBusInputPurpose)이므로 이 표가 갈리면 프런트별로 같은
/// 필드가 다르게 판정되는 회귀가 생긴다. DIGITS(2)·ALPHA(1)·PHONE(4)·NAME(7)이
/// Normal 로 떨어지는 것은 GTK 표와의 의도적 일치이지 누락이 아니다.
fn ibus_purpose_to_unim(purpose: u32) -> u32 {
    use unim::config::ContentPurpose as CP;
    match purpose {
        8 => CP::Password as u32,  // PASSWORD
        9 => CP::Pin as u32,       // PIN
        6 => CP::Email as u32,     // EMAIL
        3 => CP::Number as u32,    // NUMBER
        5 => CP::Url as u32,       // URL
        10 => CP::Terminal as u32, // TERMINAL
        _ => CP::Normal as u32,    // FREE_FORM(0)/ALPHA(1)/DIGITS(2)/PHONE(4)/NAME(7)/미래값
    }
}

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
    /// 클라이언트가 `ClientCommitPreedit` 속성으로 알려온 값. true 면
    /// GTK im-ibus(libibus >= 1.5.20) 계열로 판단해 UpdatePreeditTextWithMode
    /// 4-인자 시그널만 발신한다 — emit_update_preedit 참고.
    ///
    /// 이 구조체 인스턴스는 zbus ObjectServer 가 `Arc<RwLock<dyn Interface>>`
    /// 로 감싸 보관하고, `&self`/`&mut self` 메서드 호출이 전부 그 락을 거쳐
    /// 들어온다. 발신 지점(emit_update_preedit, emit_engine_response,
    /// FocusOut, Reset)이 전부 이 객체의 `&self` 메서드 안이라 락 밖의
    /// 스폰된 태스크가 따로 접근하지 않으므로, Arc<AtomicBool> 없이 평범한
    /// 필드로 충분하다.
    client_commit_preedit: bool,
    /// SetCapabilities 로 클라이언트가 보고한 기능 비트마스크. 위와 같은
    /// 이유로 평범한 필드 — 아직 동작 분기에는 쓰지 않고 저장만 한다.
    caps: u32,
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
            client_commit_preedit: false,
            caps: 0,
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
                self.emit_update_preedit(signal_ctx, text, cursor_pos, true)
                    .await;
                let _ = Self::show_preedit_text(signal_ctx).await;
            }
            _ => {
                // preedit 비어있으면 숨김
                if response.preedit.is_some() {
                    let text = ibus_types::serialize_ibus_text("");
                    self.emit_update_preedit(signal_ctx, text, 0, false).await;
                    let _ = Self::hide_preedit_text(signal_ctx).await;
                }
            }
        }

        // ForwardKeyEvent (consumed=false인 경우 클라이언트에서 처리)
        // IBus에서는 ProcessKeyEvent의 반환값으로 처리하므로 별도 시그널 불필요
    }

    /// UpdatePreeditText 발신 분기: `ClientCommitPreedit` 속성값에 따라
    /// 3-인자(UpdatePreeditText)/4-인자(UpdatePreeditTextWithMode) 중 하나만
    /// 보낸다 — 최신 GTK im-ibus(libibus >= 1.5.20)는 4-인자만 구독하고
    /// 3-인자는 무시하므로 둘 다 보내면 그쪽이 화면에 안 보인다(이 수정의
    /// 발단이 된 근본 원인).
    ///
    /// mode 는 항상 CLEAR(0) 로 고정한다. IBusEngine 의 COMMIT(1)은
    /// 클라이언트측 `ibus_im_context_clear_preedit_text` 가 포커스 아웃·클릭
    /// 시 데몬 RPC 응답보다 먼저 로컬로 실행되며, mode==COMMIT 이면 화면에
    /// 남아있던 preedit 문자열을 클라이언트가 자체적으로 다시 커밋해
    /// 버린다. 우리 데몬은 FocusOut/Reset 에서 이미 CommitText 로 같은
    /// 문자열을 커밋하므로 COMMIT 을 쓰면 이중 커밋이 된다. fcitx5 는
    /// COMMIT 을 쓰지만 그건 fcitx5 프레임워크의 커밋 순서 전제가 우리와
    /// 달라서다 — 여기서 베끼면 회귀다.
    async fn emit_update_preedit(
        &self,
        signal_ctx: &zbus::SignalContext<'_>,
        text: Value<'_>,
        cursor_pos: u32,
        visible: bool,
    ) {
        if self.client_commit_preedit {
            if let Err(e) = Self::update_preedit_text_with_mode(
                signal_ctx,
                text,
                cursor_pos,
                visible,
                ibus_types::preedit_mode::IBUS_ENGINE_PREEDIT_CLEAR,
            )
            .await
            {
                unim_log!(
                    "DAEMON",
                    "[IBus Compat] UpdatePreeditTextWithMode 시그널 실패: {}",
                    e
                );
            }
        } else {
            // 이 환경에 libibus < 1.5.20 클라이언트 실물이 없어 실제로는 타지
            // 않는 방어적 경로다 — ClientCommitPreedit 를 세팅하지 않는
            // 구버전 im-ibus 대비 3-인자 UpdatePreeditText 를 그대로
            // 유지한다.
            if let Err(e) = Self::update_preedit_text(signal_ctx, text, cursor_pos, visible).await
            {
                unim_log!(
                    "DAEMON",
                    "[IBus Compat] UpdatePreeditText 시그널 실패: {}",
                    e
                );
            }
        }
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

        // IBus 계약상 keycode 는 **이미 evdev 코드**다 — 클라이언트가 보내기 전에
        // 빼 준다(GTK `im-ibus` 의 `ibus_im_context_filter_keypress` 는
        // `event->hardware_keycode - 8` 을 넘긴다). 여기서 또 8 을 빼면 이중
        // 차감이 되어 다른 키로 바뀐다 — 2026-08-09 실측: `y`(X11 29 → evdev 21)
        // 가 13(`=`)으로 처리돼 한글 대신 "=" 이 찍혔다.
        let evdev_keycode = keycode;

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
                self.emit_update_preedit(&signal_ctx, empty, 0, false).await;
                let _ = Self::hide_preedit_text(&signal_ctx).await;
            }
        }

        // 이탈 시 Normal 복귀(fail-safe): IBus 클라이언트가 필드를 옮기며
        // SetContentType(0)을 별도로 보내지 않는 경우에도 Password/Pin 이 이
        // 컨텍스트에 잔존해 다음 필드의 한글 입력을 계속 차단하는 사고를 막는다.
        // engine.set_content_purpose 는 멱등(surrounding.rs:26-29)이라 이미
        // Normal 이면 로그도 안 남기고 no-op — 매 FocusOut 마다 보내도 무해하다.
        let _ = self
            .engine_tx
            .send(EngineRequest::SetContentType {
                context_id: self.context_id,
                purpose: unim::config::ContentPurpose::Normal as u32,
            })
            .await;

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
                self.emit_update_preedit(&signal_ctx, empty, 0, false).await;
                let _ = Self::hide_preedit_text(&signal_ctx).await;
            }
        }
        Ok(())
    }

    /// 클라이언트 기능 플래그 설정
    ///
    /// 아직 동작 분기(예: SURROUNDING_TEXT 유무에 따른 처리 변경)에는 쓰지
    /// 않고 저장만 한다.
    #[zbus(name = "SetCapabilities")]
    async fn set_capabilities(&mut self, caps: u32) -> zbus::fdo::Result<()> {
        self.caps = caps;
        unim_log!("DAEMON", "[IBus Compat] SetCapabilities: caps={:#x}", caps);
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
    ///
    /// `purpose` 는 IBus 번호 체계(IBusInputPurpose) → `ibus_purpose_to_unim`으로
    /// UNIM 번호 체계로 변환 후 엔진에 전달한다. `_hints`(IBusInputHints)는
    /// 무시한다 — 로컬 gir1.2-ibus-1.0 1.5.29 실측값(NONE 0, SPELLCHECK 1,
    /// NO_SPELLCHECK 2, WORD_COMPLETION 4, LOWERCASE 8, UPPERCASE_CHARS 16,
    /// UPPERCASE_WORDS 32, UPPERCASE_SENTENCES 64, INHIBIT_OSK 128,
    /// VERTICAL_WRITING 256, EMOJI 512, NO_EMOJI 1024, PRIVATE 2048)에는
    /// hidden-text 계열이 없고, PRIVATE 는 Qt 의 `ImhSensitiveData` 와 같은
    /// '보이는 민감필드' 계열이라 차단 근거로 부적합하다
    /// (선례: qt5/src/input_context.cpp:499-502).
    #[zbus(name = "SetContentType")]
    async fn set_content_type(&self, purpose: u32, _hints: u32) -> zbus::fdo::Result<()> {
        let unim_purpose = ibus_purpose_to_unim(purpose);
        // 로그 태그는 이 파일의 기존 관례("DAEMON" + "[IBus Compat]" 접두사)를 따른다.
        unim_log!(
            "DAEMON",
            "[IBus Compat] SetContentType: ibus_purpose={} → unim_purpose={}",
            purpose,
            unim_purpose
        );
        let _ = self
            .engine_tx
            .send(EngineRequest::SetContentType {
                context_id: self.context_id,
                purpose: unim_purpose,
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

    // ─── 속성 ───

    /// GTK im-ibus(libibus >= 1.5.20)가 세팅하는 클라이언트 기능 속성.
    ///
    /// 러스트 타입은 `bool` 이 아니라 `(bool,)` 튜플이다 — IBus 클라이언트가
    /// 실제로 기대하는 GVariant 와이어 타입이 `(b)`(단일원소 구조체)이지
    /// bool 그대로의 `b` 가 아니기 때문이다(ibus_types.rs 의
    /// test_client_commit_preedit_property_wire_signature 참고).
    #[zbus(property, name = "ClientCommitPreedit")]
    async fn client_commit_preedit(&self) -> (bool,) {
        (self.client_commit_preedit,)
    }

    #[zbus(property, name = "ClientCommitPreedit")]
    async fn set_client_commit_preedit(&mut self, value: (bool,)) -> zbus::fdo::Result<()> {
        self.client_commit_preedit = value.0;
        unim_log!("DAEMON", "[IBus Compat] ClientCommitPreedit 설정: {}", value.0);
        Ok(())
    }

    // ─── 시그널 ───

    /// 확정 텍스트 시그널
    #[zbus(signal, name = "CommitText")]
    async fn commit_text(ctx: &zbus::SignalContext<'_>, text: Value<'_>) -> zbus::Result<()>;

    /// Preedit 업데이트 시그널 (3-인자, 구버전 폴백)
    ///
    /// libibus < 1.5.20(ClientCommitPreedit 속성을 세팅하지 않는 클라이언트)
    /// 대비 방어적으로 유지하는 경로다 — 이 환경에는 그런 클라이언트 실물이
    /// 없다. 실제 발신 분기는 emit_update_preedit 참고.
    #[zbus(signal, name = "UpdatePreeditText")]
    async fn update_preedit_text(
        ctx: &zbus::SignalContext<'_>,
        text: Value<'_>,
        cursor_pos: u32,
        visible: bool,
    ) -> zbus::Result<()>;

    /// Preedit 업데이트 시그널 (4-인자, WithMode)
    ///
    /// GTK im-ibus(libibus >= 1.5.20)가 실제로 구독하는 경로 — `mode` 는
    /// 항상 CLEAR(0) 고정(emit_update_preedit 의 주석 참고).
    #[zbus(signal, name = "UpdatePreeditTextWithMode")]
    async fn update_preedit_text_with_mode(
        ctx: &zbus::SignalContext<'_>,
        text: Value<'_>,
        cursor_pos: u32,
        visible: bool,
        mode: u32,
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

#[cfg(test)]
mod tests {
    use super::*;
    use unim::config::ContentPurpose as CP;

    /// IBus/GTK 공유 InputPurpose enum 전 값(0..=10) + 미래값(999) 검증.
    /// gtk3_input_purpose_to_unim(gtk3/src/immodule.c) / gtk_input_purpose_to_unim
    /// (gtk4/src/immodule.c) 과 반드시 1:1 동일해야 한다 — 같은 enum 이므로 표가
    /// 갈리면 프런트별 동작 차이가 생긴다(회귀 방지 주석).
    #[test]
    fn test_ibus_purpose_to_unim_matches_gtk_table() {
        assert_eq!(ibus_purpose_to_unim(0), CP::Normal as u32); // FREE_FORM
        assert_eq!(ibus_purpose_to_unim(1), CP::Normal as u32); // ALPHA — GTK 표와 동일하게 Normal
        assert_eq!(ibus_purpose_to_unim(2), CP::Normal as u32); // DIGITS
        assert_eq!(ibus_purpose_to_unim(3), CP::Number as u32); // NUMBER
        assert_eq!(ibus_purpose_to_unim(4), CP::Normal as u32); // PHONE
        assert_eq!(ibus_purpose_to_unim(5), CP::Url as u32); // URL
        assert_eq!(ibus_purpose_to_unim(6), CP::Email as u32); // EMAIL
        assert_eq!(ibus_purpose_to_unim(7), CP::Normal as u32); // NAME
        assert_eq!(ibus_purpose_to_unim(8), CP::Password as u32); // PASSWORD — 핵심 회귀 방지
        assert_eq!(ibus_purpose_to_unim(9), CP::Pin as u32); // PIN — 핵심 회귀 방지
        assert_eq!(ibus_purpose_to_unim(10), CP::Terminal as u32); // TERMINAL
        assert_eq!(ibus_purpose_to_unim(999), CP::Normal as u32); // 미래/미지값 → Normal
    }

    /// 결함4(raw passthrough) 회귀 방지: 변환 없이 `ContentPurpose::from_u32(8)`을
    /// 그대로 넘기면 8은 정의역(0..=6) 밖이라 Normal 로 접혀 비번 차단이 완전히
    /// 실패했다. `ibus_purpose_to_unim`을 거치면 8 → Password, 9 → Pin 으로
    /// 정확히 떨어져야 한다.
    #[test]
    fn test_password_and_pin_no_longer_lost() {
        assert_eq!(CP::from_u32(ibus_purpose_to_unim(8)), CP::Password);
        assert_eq!(CP::from_u32(ibus_purpose_to_unim(9)), CP::Pin);
        // 변환 없이 raw 8/9를 그대로 from_u32에 넣으면(구 결함) Normal 로 소실됨을
        // 함께 남겨 이 테스트가 "무엇을 회귀 방지하는지" 대조 가능하게 한다.
        assert_eq!(CP::from_u32(8), CP::Normal);
        assert_eq!(CP::from_u32(9), CP::Normal);
    }
}
