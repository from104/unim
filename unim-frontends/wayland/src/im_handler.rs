//! 입력 방식 핸들러 모듈
//!
//! Wayland input-method-v2 이벤트를 처리하고 DBus를 통해 unim-daemon과 연동합니다.

use tokio::sync::mpsc;
use unim::unim_log;
use wayland_protocols_misc::zwp_input_method_v2::client::zwp_input_method_v2::ZwpInputMethodV2;

use crate::dbus_client::{DbusRequest, DbusResponse};

/// 입력 방식 핸들러
pub struct InputMethodHandler {
    active: bool,
    serial: u32,
    surrounding_text: String,
    surrounding_cursor: u32,
    surrounding_anchor: u32,
    pending_commit: Option<String>,
    pending_preedit: Option<String>,
    /// DBus 컨텍스트 경로
    context_path: String,
    /// DBus 요청 채널
    dbus_tx: mpsc::Sender<DbusRequest>,
}

impl InputMethodHandler {
    pub fn new(dbus_tx: mpsc::Sender<DbusRequest>) -> Self {
        // DBus를 통해 입력 컨텍스트 생성
        let (response_tx, response_rx) = std::sync::mpsc::channel();

        let context_path = if dbus_tx
            .blocking_send(DbusRequest::CreateContext {
                client_name: "unim-wayland".to_string(),
                response: Some(response_tx),
            })
            .is_ok()
        {
            match response_rx.recv_timeout(std::time::Duration::from_millis(500)) {
                Ok(DbusResponse::ContextCreated { path }) => path,
                _ => {
                    use std::time::{SystemTime, UNIX_EPOCH};
                    let id = SystemTime::now()
                        .duration_since(UNIX_EPOCH)
                        .map(|d| d.as_nanos() as u32)
                        .unwrap_or(0);
                    format!("/local/context_{}", id)
                }
            }
        } else {
            "/local/context_0".to_string()
        };

        Self {
            active: false,
            serial: 0,
            surrounding_text: String::new(),
            surrounding_cursor: 0,
            surrounding_anchor: 0,
            pending_commit: None,
            pending_preedit: None,
            context_path,
            dbus_tx,
        }
    }

    /*
    /// DBus 요청 전송 (동기적)
    fn send_dbus_request(&self, request: DbusRequest) -> Option<DbusResponse> {
        let (response_tx, response_rx) = std::sync::mpsc::channel();

        let request_with_response = match request {
            DbusRequest::ProcessKey { context_path, keyval, keycode, state, .. } => {
                DbusRequest::ProcessKey {
                    context_path,
                    keyval,
                    keycode,
                    state,
                    response: Some(response_tx),
                }
            }
            other => return {
                let _ = self.dbus_tx.blocking_send(other);
                None
            },
        };

        if self.dbus_tx.blocking_send(request_with_response).is_err() {
            return None;
        }

        response_rx.recv_timeout(std::time::Duration::from_millis(500)).ok()
    }
    */

    /// 입력 방식 활성화
    pub fn activate(&mut self) {
        self.active = true;
        let _ = self.dbus_tx.blocking_send(DbusRequest::FocusIn {
            context_path: self.context_path.clone(),
        });
        unim_log!("WAYLAND_HANDLER", "입력 방식 활성화됨");
    }

    /// 입력 방식 비활성화
    pub fn deactivate(&mut self, im: &ZwpInputMethodV2) {
        if self.active {
            // DBus FocusOut 호출
            let (response_tx, response_rx) = std::sync::mpsc::channel();
            let _ = self.dbus_tx.blocking_send(DbusRequest::FocusOut {
                context_path: self.context_path.clone(),
                response: Some(response_tx),
            });

            if let Ok(DbusResponse::CommitText { text }) =
                response_rx.recv_timeout(std::time::Duration::from_millis(500))
            {
                if !text.is_empty() {
                    im.commit_string(text);
                }
            }

            im.set_preedit_string(String::new(), 0, 0);
        }
        self.active = false;
        unim_log!("WAYLAND_HANDLER", "입력 방식 비활성화됨");
    }

    /// 주변 텍스트 설정
    pub fn set_surrounding_text(&mut self, text: &str, cursor: u32, anchor: u32) {
        self.surrounding_text = text.to_string();
        self.surrounding_cursor = cursor;
        self.surrounding_anchor = anchor;
    }

    /*
    /// 키 이벤트 처리
    pub fn process_key(&mut self, keyval: u32, keycode: u32, state: u32) -> bool {
        let response = self.send_dbus_request(DbusRequest::ProcessKey {
            context_path: self.context_path.clone(),
            keyval,
            keycode,
            state,
            response: None,
        });

        match response {
            Some(DbusResponse::KeyProcessed { consumed, preedit, commit }) => {
                if let Some(commit_text) = commit {
                    if !commit_text.is_empty() {
                        self.pending_commit = Some(commit_text);
                    }
                }
                if let Some(preedit_text) = preedit {
                    self.pending_preedit = Some(preedit_text);
                }
                consumed
            }
            _ => false,
        }
    }
    */

    /// Done 이벤트 처리 (상태 커밋)
    pub fn done(&mut self, im: &ZwpInputMethodV2) {
        self.serial = self.serial.wrapping_add(1);

        // 대기 중인 커밋이 있으면 전송
        if let Some(commit) = self.pending_commit.take() {
            im.commit_string(commit);
        }

        // 대기 중인 preedit이 있으면 전송
        if let Some(preedit) = self.pending_preedit.take() {
            let cursor = preedit.len() as i32;
            im.set_preedit_string(preedit, cursor, cursor);
        }

        im.commit(self.serial);
    }
}

impl Drop for InputMethodHandler {
    fn drop(&mut self) {
        let _ = self.dbus_tx.blocking_send(DbusRequest::DestroyContext {
            context_path: self.context_path.clone(),
        });
    }
}
