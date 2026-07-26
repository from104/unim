//! Wayland 입력기 상태 관리 및 Dispatch 구현
//!
//! input-method-v2 프로토콜의 모든 이벤트를 처리합니다.
//! kime 한국어 IME의 state.rs를 참조하여 UNIM DBus 모델에 맞게 구현했습니다.
//!
//! 핵심 프로토콜 흐름:
//!   1. compositor → activate → done: 입력 활성화, grab_active=true
//!   2. keyboard_grab → key: 키 이벤트 수신 → DBus로 처리
//!   3. 소비된 키: im.commit_string/set_preedit_string + im.commit(serial)
//!   4. 미소비 키: virtual_keyboard.key()로 포워딩
//!   5. 키 반복: timerfd 기반 delay → interval 타이머로 재처리

use std::os::fd::AsFd;
use std::sync::mpsc as std_mpsc;
use tokio::sync::mpsc;
use unim::keycode::{UNIM_KEY_REPEAT_MASK, UNIM_REPEAT_AWARE_MASK};
use unim::unim_log;
use wayland_client::{
    globals::GlobalListContents,
    protocol::{
        wl_keyboard::KeyState,
        wl_registry::WlRegistry,
        wl_seat::{self, WlSeat},
    },
    Connection, Dispatch, QueueHandle, WEnum,
};
use wayland_protocols::wp::text_input::zv3::client::zwp_text_input_v3::ContentPurpose as WlContentPurpose;
use wayland_protocols_misc::zwp_input_method_v2::client::{
    zwp_input_method_keyboard_grab_v2::{self, ZwpInputMethodKeyboardGrabV2},
    zwp_input_method_manager_v2::ZwpInputMethodManagerV2,
    zwp_input_method_v2::{self, ZwpInputMethodV2},
};
use wayland_protocols_misc::zwp_virtual_keyboard_v1::client::{
    zwp_virtual_keyboard_manager_v1::ZwpVirtualKeyboardManagerV1,
    zwp_virtual_keyboard_v1::ZwpVirtualKeyboardV1,
};

use crate::dbus_client::{DbusRequest, DbusResponse};
use crate::keymap::KeymapHandler;
use crate::repeat::{PressState, RepeatInfo, RepeatTimer};

/// Wayland 애플리케이션 전체 상태
pub struct AppState {
    // Wayland 글로벌 오브젝트
    pub seat: Option<WlSeat>,
    pub im_manager: Option<ZwpInputMethodManagerV2>,
    pub vk_manager: Option<ZwpVirtualKeyboardManagerV1>,

    // 프로토콜 오브젝트
    pub input_method: Option<ZwpInputMethodV2>,
    pub keyboard_grab: Option<ZwpInputMethodKeyboardGrabV2>,
    pub virtual_keyboard: Option<ZwpVirtualKeyboardV1>,

    // 입력 방식 상태 (더블버퍼)
    serial: u32,
    pending_activate: bool,
    pending_deactivate: bool,
    current_active: bool,
    grab_active: bool,
    keymap_init: bool,
    /// 이번 더블버퍼 사이클에 수신한 ContentType 목적(UNIM ContentPurpose u32).
    ///
    /// text-input-v3 는 content_type 을 필드 목적이 기본이 아닐 때만 보낸다. 즉시
    /// 송신하면 (a) FocusIn 보다 먼저 도달할 수 있고, (b) 비번 필드→목적 미송신 앱으로
    /// 이동 시 Password 가 잔존한다. 그래서 이벤트는 여기 보관하고 Done(활성화 커밋)에서
    /// FocusIn 뒤에 송신하되, 사이클에 ContentType 이 없었으면 Normal 을 명시 송신해
    /// 잔존을 차단한다(fail-safe). 각 Done 에서 `take` 로 소비된다.
    pending_content_purpose: Option<u32>,

    // 키 처리
    keymap_handler: KeymapHandler,
    last_preedit: String,

    // 키 반복
    pub repeat_timer: RepeatTimer,
    repeat_info: Option<RepeatInfo>,
    press_state: PressState,

    // DBus 통신
    dbus_tx: mpsc::Sender<DbusRequest>,
    context_path: String,

    // 종료 플래그
    pub should_exit: bool,
}

impl AppState {
    pub fn new(dbus_tx: mpsc::Sender<DbusRequest>) -> Self {
        // DBus 컨텍스트 생성
        let context_path = create_dbus_context(&dbus_tx);

        Self {
            seat: None,
            im_manager: None,
            vk_manager: None,
            input_method: None,
            keyboard_grab: None,
            virtual_keyboard: None,
            serial: 0,
            pending_activate: false,
            pending_deactivate: false,
            current_active: false,
            grab_active: false,
            keymap_init: false,
            pending_content_purpose: None,
            keymap_handler: KeymapHandler::new(),
            last_preedit: String::new(),
            repeat_timer: RepeatTimer::new(),
            repeat_info: None,
            press_state: PressState::NotPressing,

            dbus_tx,
            context_path,
            should_exit: false,
        }
    }

    /// 입력 방식 셋업: input_method + keyboard_grab + virtual_keyboard 생성
    pub fn setup(&mut self, qh: &QueueHandle<Self>) -> bool {
        let (seat, im_manager) = match (&self.seat, &self.im_manager) {
            (Some(s), Some(m)) => (s, m),
            _ => {
                unim_log!("WAYLAND", "seat 또는 im_manager가 없습니다");
                return false;
            }
        };

        // input_method 생성
        let im = im_manager.get_input_method(seat, qh, ());
        unim_log!("WAYLAND", "zwp_input_method_v2 생성");

        // keyboard grab 시작
        let grab = im.grab_keyboard(qh, ());
        unim_log!("WAYLAND", "keyboard_grab 시작");

        // virtual keyboard 생성 (옵션)
        if let Some(ref vk_manager) = self.vk_manager {
            let vk = vk_manager.create_virtual_keyboard(seat, qh, ());
            unim_log!("WAYLAND", "virtual_keyboard 생성");
            self.virtual_keyboard = Some(vk);
        } else {
            unim_log!(
                "WAYLAND",
                "virtual_keyboard_manager 없음 - 키 바이패스 불가"
            );
        }

        self.input_method = Some(im);
        self.keyboard_grab = Some(grab);

        true
    }

    /// 키 이벤트를 DBus로 전송하고 결과 처리
    ///
    /// `is_repeat`: 이 press 가 자체 합성한 키 반복(timerfd)인지 여부. Wayland 는
    /// repeat 를 스스로 구분하므로 aware 프런트로서 `state` 상위 비트에
    /// `UNIM_REPEAT_AWARE_MASK`(항상) + `UNIM_KEY_REPEAT_MASK`(반복 시)를 태깅한다.
    /// 데몬은 `ignore_key_repeat` on 일 때만 이 비트로 억제를 판정한다(off 면 무손상).
    fn process_key_via_dbus(
        &mut self,
        evdev_keycode: u32,
        time: u32,
        key_state_raw: u32,
        is_repeat: bool,
    ) -> bool {
        let keysym = self.keymap_handler.get_keysym(evdev_keycode);
        let mod_state = self.keymap_handler.mod_state;
        // 데몬 ProcessKey 의 keycode 는 **raw evdev** 다 (엔진 from_evdev_keycode
        // 계약: 예 F9=67, A=30). wl_keyboard 는 이미 evdev 스캔코드를 주므로 그대로
        // 보낸다. 종전에 XKB 호환이라며 +8(X11 하드웨어 코드)을 더했으나, 그러면
        // from_evdev_keycode 가 엉뚱한 키로 해석해 **모든 키가 어긋났다**(GTK 는
        // hardware_keycode-8 로 이미 raw evdev 를 보낸다). keysym 조회에만 raw
        // evdev 가 쓰이고(위), +8 은 이 DBus 필드 외엔 미사용이라 안전한 정정.
        let keycode = evdev_keycode;

        /* 팝업 키 처리는 엔진이 담당 (process_popup_key) */

        /* 한자 키도 엔진이 ProcessKey로 처리 */

        // DBus 동기 호출
        let (response_tx, response_rx) = std_mpsc::channel();
        if self
            .dbus_tx
            .blocking_send(DbusRequest::ProcessKey {
                context_path: self.context_path.clone(),
                keyval: keysym,
                keycode,
                // aware 태깅: AWARE 비트는 항상, REPEAT 비트는 자체 합성 반복일 때만.
                // (모디파이어 파싱은 하위 비트만 소비하므로 상위 비트는 무손상)
                state: mod_state
                    | UNIM_REPEAT_AWARE_MASK
                    | if is_repeat { UNIM_KEY_REPEAT_MASK } else { 0 },
                response: Some(response_tx),
            })
            .is_err()
        {
            return false;
        }

        match response_rx.recv_timeout(std::time::Duration::from_millis(50)) {
            Ok(DbusResponse::KeyProcessed {
                consumed,
                preedit,
                commit,
            }) => {
                if consumed {
                    self.apply_input_result(&commit, &preedit);
                    true
                } else {
                    // 소비되지 않은 키 → virtual keyboard로 포워딩
                    self.forward_key(time, evdev_keycode, key_state_raw);
                    false
                }
            }
            _ => {
                // DBus 타임아웃 → 키 포워딩
                self.forward_key(time, evdev_keycode, key_state_raw);
                false
            }
        }
    }

    /// 엔진 결과를 Wayland 프로토콜로 적용
    fn apply_input_result(&mut self, commit: &str, preedit: &str) {
        if let Some(ref im) = self.input_method {
            // 커밋 문자열 전송
            if !commit.is_empty() {
                im.commit_string(commit.to_string());
            }

            // preedit 문자열 업데이트
            if !preedit.is_empty() {
                let len = preedit.len() as i32;
                im.set_preedit_string(preedit.to_string(), 0, len);
                self.last_preedit = preedit.to_string();
            } else if !self.last_preedit.is_empty() {
                // preedit 클리어
                im.set_preedit_string(String::new(), 0, 0);
                self.last_preedit.clear();
            }

            // 상태 커밋 (더블버퍼 적용)
            im.commit(self.serial);
        }
    }

    /// AutoTypeFix 교정 적용 (delete_surrounding_text + commit + preedit)
    pub fn apply_auto_typefix(&mut self, delete_chars: u32, commit_text: &str, preedit_text: &str) {
        if let Some(ref im) = self.input_method {
            if delete_chars > 0 {
                // 삭제할 텍스트의 바이트 수 계산
                // 순방향(영→한): commit이 한글 → 삭제 대상은 ASCII (1 byte/char)
                // 역방향(한→영): commit이 ASCII → 삭제 대상은 한글 (3 bytes/char)
                let is_forward = commit_text
                    .chars()
                    .next()
                    .map(|c| {
                        ('\u{AC00}'..='\u{D7A3}').contains(&c)
                            || ('\u{3131}'..='\u{318E}').contains(&c)
                    })
                    .unwrap_or(false);

                let before_bytes = if is_forward {
                    delete_chars // ASCII: 1 byte per char
                } else {
                    delete_chars * 3 // 한글: 3 bytes per char (UTF-8)
                };

                unim_log!(
                    "WAYLAND",
                    "AutoTypeFix delete_surrounding_text: chars={}, bytes={}, forward={}",
                    delete_chars,
                    before_bytes,
                    is_forward
                );

                im.delete_surrounding_text(before_bytes, 0);
            }

            if !commit_text.is_empty() {
                im.commit_string(commit_text.to_string());
            }

            // preedit 업데이트
            if !preedit_text.is_empty() {
                let len = preedit_text.len() as i32;
                im.set_preedit_string(preedit_text.to_string(), 0, len);
                self.last_preedit = preedit_text.to_string();
            } else if !self.last_preedit.is_empty() {
                im.set_preedit_string(String::new(), 0, 0);
                self.last_preedit.clear();
            }

            im.commit(self.serial);
        }
    }

    /// 미소비 키를 virtual keyboard로 포워딩
    fn forward_key(&self, time: u32, key: u32, state: u32) {
        if let Some(ref vk) = self.virtual_keyboard {
            vk.key(time, key, state);
        }
    }

    /// 포커스 아웃 시 조합 중인 텍스트 커밋
    fn handle_deactivate(&mut self) {
        // 한자/특수문자 팝업 모드 취소 + 트리거 문자 커밋
        {
            let (tx, rx) = std_mpsc::channel();
            let _ = self.dbus_tx.blocking_send(DbusRequest::CancelHanja {
                context_path: self.context_path.clone(),
                response: Some(tx),
            });
            if let Ok(DbusResponse::CommitText { text }) =
                rx.recv_timeout(std::time::Duration::from_millis(500))
            {
                if !text.is_empty() {
                    if let Some(ref im) = self.input_method {
                        im.commit_string(text);
                    }
                }
            }
        }
        {
            let (tx, rx) = std_mpsc::channel();
            let _ = self.dbus_tx.blocking_send(DbusRequest::CancelSpecialChar {
                context_path: self.context_path.clone(),
                response: Some(tx),
            });
            if let Ok(DbusResponse::CommitText { text }) =
                rx.recv_timeout(std::time::Duration::from_millis(500))
            {
                if !text.is_empty() {
                    if let Some(ref im) = self.input_method {
                        im.commit_string(text);
                    }
                }
            }
        }

        // DBus FocusOut으로 조합 중 텍스트 가져오기
        let (response_tx, response_rx) = std_mpsc::channel();
        let _ = self.dbus_tx.blocking_send(DbusRequest::FocusOut {
            context_path: self.context_path.clone(),
            response: Some(response_tx),
        });

        if let Ok(DbusResponse::CommitText { text }) =
            response_rx.recv_timeout(std::time::Duration::from_millis(500))
        {
            if !text.is_empty() {
                if let Some(ref im) = self.input_method {
                    im.commit_string(text);
                }
            }
        }

        // preedit 클리어
        if let Some(ref im) = self.input_method {
            if !self.last_preedit.is_empty() {
                im.set_preedit_string(String::new(), 0, 0);
                self.last_preedit.clear();
            }
            im.commit(self.serial);
        }

        // 키 반복 취소
        self.repeat_timer.cancel();
        self.press_state = PressState::NotPressing;
    }

    /// 한자 팝업 표시 직후 즐겨찾기 상태 fetch (XIM handler.rs:1306 패턴).
    ///
    /// 엔진은 [`HanjaBookmarkChanged`]/[`HanjaCandidatesReordered`] 시그널로 변화만
    /// 알리므로, 첫 표시 시 ☆/★ 페인트를 위해서는 한 번 동기 조회가 필요하다.
    #[allow(dead_code)]
    pub fn fetch_initial_bookmark_states(&mut self) -> Vec<bool> {
        let (tx, rx) = std_mpsc::channel();
        if self
            .dbus_tx
            .blocking_send(DbusRequest::GetHanjaBookmarkStates {
                context_path: self.context_path.clone(),
                response: Some(tx),
            })
            .is_err()
        {
            return Vec::new();
        }
        match rx.recv_timeout(std::time::Duration::from_millis(200)) {
            Ok(DbusResponse::HanjaBookmarkStates { states }) => states,
            _ => Vec::new(),
        }
    }

    /// 키 반복 타이머 만료 처리
    pub fn handle_repeat_timer(&mut self) {
        let count = self.repeat_timer.read().unwrap_or(0);
        if count == 0 {
            return;
        }

        if let PressState::Pressing {
            key,
            is_repeating,
            wayland_time,
        } = &mut self.press_state
        {
            if !*is_repeating {
                // delay → interval 전환
                if let Some(ref info) = self.repeat_info {
                    unim_log!("WAYLAND", "키 반복 시작: key={}, rate={}", key, info.rate);
                    self.repeat_timer.set_interval(info.rate);
                }
                *is_repeating = true;
            }

            let key = *key;
            let _time = *wayland_time;

            // 활성 상태에서만 키 반복 처리 (자체 합성 반복 → is_repeat=true 태깅)
            if self.grab_active {
                self.process_key_via_dbus(key, 0, KeyState::Pressed as u32, true);
            }
        }
    }
}

/// DBus 컨텍스트 생성 (초기화 시)
fn create_dbus_context(dbus_tx: &mpsc::Sender<DbusRequest>) -> String {
    let (response_tx, response_rx) = std_mpsc::channel();

    if dbus_tx
        .blocking_send(DbusRequest::CreateContext {
            client_name: "unim-wayland".to_string(),
            window_id: "unim-wayland".to_string(),
            response: Some(response_tx),
        })
        .is_ok()
    {
        match response_rx.recv_timeout(std::time::Duration::from_millis(1000)) {
            Ok(DbusResponse::ContextCreated { path }) => {
                unim_log!("WAYLAND", "DBus 컨텍스트 생성: {}", path);
                return path;
            }
            _ => {
                unim_log!("WAYLAND", "DBus 컨텍스트 생성 타임아웃 - 로컬 경로 사용");
            }
        }
    }

    // 폴백
    let id = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos() as u32)
        .unwrap_or(0);
    format!("/local/context_{}", id)
}

impl Drop for AppState {
    fn drop(&mut self) {
        let _ = self.dbus_tx.blocking_send(DbusRequest::DestroyContext {
            context_path: self.context_path.clone(),
        });
    }
}

// ============================================================
// Dispatch 구현들
// ============================================================

// --- WlRegistry ---
impl Dispatch<WlRegistry, GlobalListContents> for AppState {
    fn event(
        _state: &mut Self,
        _proxy: &WlRegistry,
        _event: wayland_client::protocol::wl_registry::Event,
        _data: &GlobalListContents,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
    ) {
        // registry_queue_init이 처리함
    }
}

// --- WlSeat ---
impl Dispatch<WlSeat, ()> for AppState {
    fn event(
        _state: &mut Self,
        _seat: &WlSeat,
        _event: wl_seat::Event,
        _data: &(),
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
    ) {
        // capabilities 이벤트 수신 (현재 별도 처리 없음)
    }
}

// --- ZwpInputMethodManagerV2 ---
impl Dispatch<ZwpInputMethodManagerV2, ()> for AppState {
    fn event(
        _state: &mut Self,
        _proxy: &ZwpInputMethodManagerV2,
        _event: wayland_protocols_misc::zwp_input_method_v2::client::zwp_input_method_manager_v2::Event,
        _data: &(),
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
    ) {
        // Manager는 이벤트 없음
    }
}

// --- ZwpVirtualKeyboardManagerV1 ---
impl Dispatch<ZwpVirtualKeyboardManagerV1, ()> for AppState {
    fn event(
        _state: &mut Self,
        _proxy: &ZwpVirtualKeyboardManagerV1,
        _event: wayland_protocols_misc::zwp_virtual_keyboard_v1::client::zwp_virtual_keyboard_manager_v1::Event,
        _data: &(),
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
    ) {
        // Manager 이벤트 없음
    }
}

// --- ZwpVirtualKeyboardV1 ---
impl Dispatch<ZwpVirtualKeyboardV1, ()> for AppState {
    fn event(
        _state: &mut Self,
        _proxy: &ZwpVirtualKeyboardV1,
        _event: wayland_protocols_misc::zwp_virtual_keyboard_v1::client::zwp_virtual_keyboard_v1::Event,
        _data: &(),
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
    ) {
        // Virtual keyboard 이벤트 없음
    }
}

// --- ZwpInputMethodV2 (핵심: 활성화/비활성화/Done) ---
impl Dispatch<ZwpInputMethodV2, ()> for AppState {
    fn event(
        state: &mut Self,
        _proxy: &ZwpInputMethodV2,
        event: zwp_input_method_v2::Event,
        _data: &(),
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
    ) {
        match event {
            zwp_input_method_v2::Event::Activate => {
                unim_log!("WAYLAND", "Activate (pending)");
                state.pending_activate = true;
            }

            zwp_input_method_v2::Event::Deactivate => {
                unim_log!("WAYLAND", "Deactivate (pending)");
                state.pending_deactivate = true;
            }

            zwp_input_method_v2::Event::Done => {
                state.serial += 1;

                let should_activate = !state.current_active && state.pending_activate;
                let should_deactivate = state.current_active && state.pending_deactivate;

                if should_activate {
                    unim_log!("WAYLAND", "Done → 활성화 (serial={})", state.serial);
                    state.grab_active = true;
                    state.current_active = true;

                    // DBus FocusIn — SetContentType 보다 먼저 보내 순서를 고정한다.
                    let _ = state.dbus_tx.blocking_send(DbusRequest::FocusIn {
                        context_path: state.context_path.clone(),
                        window_id: "unim-wayland".to_string(),
                    });

                    // 활성화 커밋: 이번 사이클에 ContentType 을 받았으면 그 목적을,
                    // 못 받았으면 Normal 을 명시 송신한다. 후자가 비번 필드 → 목적
                    // 미송신 앱 이동 시 Password 잔존을 차단하는 fail-safe.
                    let purpose = state
                        .pending_content_purpose
                        .take()
                        .unwrap_or(unim::config::ContentPurpose::Normal as u32);
                    unim_log!("WAYLAND", "활성화 → SetContentType(purpose={})", purpose);
                    let _ = state.dbus_tx.blocking_send(DbusRequest::SetContentType {
                        context_path: state.context_path.clone(),
                        purpose,
                    });
                } else if should_deactivate {
                    unim_log!("WAYLAND", "Done → 비활성화 (serial={})", state.serial);
                    state.handle_deactivate();
                    state.grab_active = false;
                    state.current_active = false;
                    // 비활성 사이클의 잔여 목적은 폐기(다음 활성화에서 새로 판정).
                    state.pending_content_purpose = None;
                    // Normal 명시 복귀 — 다른 5개 프런트엔드의 "필드 이탈 시
                    // SetContentType(Normal)" 규약(SPEC §13.4)과 동일. 다음 활성화의
                    // fail-safe 에만 의존하면 그 사이 엔진에 Password 가 잔존한다.
                    let _ = state.dbus_tx.blocking_send(DbusRequest::SetContentType {
                        context_path: state.context_path.clone(),
                        purpose: unim::config::ContentPurpose::Normal as u32,
                    });
                } else if state.current_active {
                    // 포커스 유지 중 목적 변경(mid-focus content_type 갱신) 반영.
                    if let Some(purpose) = state.pending_content_purpose.take() {
                        unim_log!(
                            "WAYLAND",
                            "목적 변경 → SetContentType(purpose={})",
                            purpose
                        );
                        let _ = state.dbus_tx.blocking_send(DbusRequest::SetContentType {
                            context_path: state.context_path.clone(),
                            purpose,
                        });
                    }
                }

                // pending 상태 리셋
                state.pending_activate = false;
                state.pending_deactivate = false;
            }

            zwp_input_method_v2::Event::SurroundingText { .. } => {
                // surrounding text 정보 (현재 미사용)
            }

            zwp_input_method_v2::Event::TextChangeCause { .. } => {
                // text change cause (현재 미사용)
            }

            zwp_input_method_v2::Event::ContentType { purpose, .. } => {
                // 프로토콜 생성 enum(password/pin)만 매칭 → UNIM ContentPurpose 로 변환.
                // 그 외 목적은 Normal 로 매핑한다. 즉시 송신하지 않고 pending 에 보관해
                // Done(활성화 커밋)에서 FocusIn 뒤에 송신한다(순서 보장 + 미수신 시 잔존 차단).
                let unim_purpose = match purpose {
                    WEnum::Value(WlContentPurpose::Password) => {
                        unim::config::ContentPurpose::Password as u32
                    }
                    WEnum::Value(WlContentPurpose::Pin) => {
                        unim::config::ContentPurpose::Pin as u32
                    }
                    _ => unim::config::ContentPurpose::Normal as u32,
                };
                unim_log!(
                    "WAYLAND",
                    "ContentType 수신 → pending(purpose={})",
                    unim_purpose
                );
                state.pending_content_purpose = Some(unim_purpose);
            }

            zwp_input_method_v2::Event::Unavailable => {
                unim_log!(
                    "WAYLAND",
                    "Unavailable - 다른 입력기가 이미 실행 중이거나 seat가 제거됨"
                );
                state.should_exit = true;
            }

            _ => {}
        }
    }
}

// --- ZwpInputMethodKeyboardGrabV2 (핵심: 키 이벤트 수신) ---
impl Dispatch<ZwpInputMethodKeyboardGrabV2, ()> for AppState {
    fn event(
        state: &mut Self,
        _grab: &ZwpInputMethodKeyboardGrabV2,
        event: zwp_input_method_keyboard_grab_v2::Event,
        _data: &(),
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
    ) {
        match event {
            zwp_input_method_keyboard_grab_v2::Event::Keymap { format, fd, size } => {
                unim_log!(
                    "WAYLAND",
                    "Keymap 수신 (format={:?}, size={})",
                    format,
                    size
                );

                // virtual keyboard에 먼저 포워딩 (borrow)
                if !state.keymap_init {
                    if let Some(ref vk) = state.virtual_keyboard {
                        vk.keymap(format.into(), fd.as_fd(), size);
                        unim_log!("WAYLAND", "Keymap → virtual_keyboard 포워딩");
                    }
                    state.keymap_init = true;
                }

                // xkbcommon state 생성 (fd 소유권 이전)
                if state.keymap_handler.update_keymap(fd, size) {
                    unim_log!("WAYLAND", "xkbcommon 키맵 초기화 성공");
                } else {
                    unim_log!("WAYLAND", "xkbcommon 키맵 초기화 실패");
                }
            }

            zwp_input_method_keyboard_grab_v2::Event::Key {
                time,
                key,
                state: key_state,
                ..
            } => {
                let is_pressed = matches!(key_state, WEnum::Value(KeyState::Pressed));

                if is_pressed && state.grab_active {
                    // 활성 상태에서 키 눌림 → DBus 엔진에 전달 (실제 press → is_repeat=false)
                    let consumed =
                        state.process_key_via_dbus(key, time, KeyState::Pressed as u32, false);

                    if consumed {
                        // 소비된 키 → 키 반복 시작
                        if let Some(ref info) = state.repeat_info {
                            if !state.press_state.is_pressing(key) {
                                state.repeat_timer.set_delay(info.delay);
                                state.press_state = PressState::Pressing {
                                    key,
                                    wayland_time: time,
                                    is_repeating: false,
                                };
                            }
                        }
                    } else {
                        // 미소비 키 → 반복 취소 (포워딩은 process_key_via_dbus에서 처리)
                        state.repeat_timer.cancel();
                        state.press_state = PressState::NotPressing;
                    }
                } else if matches!(key_state, WEnum::Value(KeyState::Released)) {
                    // 키 릴리스 → 반복 취소 + 포워딩
                    if state.press_state.is_pressing(key) {
                        state.repeat_timer.cancel();
                        state.press_state = PressState::NotPressing;
                    }
                    state.forward_key(time, key, KeyState::Released as u32);
                } else {
                    // grab_active=false (비활성): 키 바이패스
                    if let WEnum::Value(ks) = key_state {
                        state.forward_key(time, key, ks as u32);
                    }
                }
            }

            zwp_input_method_keyboard_grab_v2::Event::Modifiers {
                mods_depressed,
                mods_latched,
                mods_locked,
                group,
                ..
            } => {
                // xkbcommon 수정자 상태 업데이트
                state.keymap_handler.update_modifiers(
                    mods_depressed,
                    mods_latched,
                    mods_locked,
                    group,
                );

                // virtual keyboard에도 포워딩
                if let Some(ref vk) = state.virtual_keyboard {
                    vk.modifiers(mods_depressed, mods_latched, mods_locked, group);
                }
            }

            zwp_input_method_keyboard_grab_v2::Event::RepeatInfo { rate, delay } => {
                unim_log!("WAYLAND", "RepeatInfo: rate={}, delay={}", rate, delay);
                if rate > 0 {
                    state.repeat_info = Some(RepeatInfo { rate, delay });
                } else {
                    state.repeat_info = None;
                }
            }

            _ => {}
        }
    }
}
