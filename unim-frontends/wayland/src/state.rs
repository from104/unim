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
use unim::unim_log;
use wayland_client::{
    globals::GlobalListContents,
    protocol::{
        wl_compositor::WlCompositor, wl_keyboard::KeyState, wl_registry::WlRegistry,
        wl_seat::WlSeat, wl_shm::WlShm,
    },
    Connection, Dispatch, QueueHandle, WEnum,
};
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
use crate::popup_surface::PopupSurface;
use crate::repeat::{PressState, RepeatInfo, RepeatTimer};

/// Wayland 애플리케이션 전체 상태
pub struct AppState {
    // Wayland 글로벌 오브젝트
    pub seat: Option<WlSeat>,
    pub im_manager: Option<ZwpInputMethodManagerV2>,
    pub vk_manager: Option<ZwpVirtualKeyboardManagerV1>,
    pub compositor: Option<WlCompositor>,
    pub shm: Option<WlShm>,

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

    // 이벤트 큐 핸들 (팝업 렌더링용)
    pub qh: Option<QueueHandle<Self>>,

    // 팝업 서피스
    pub popup_surface: PopupSurface,
}

impl AppState {
    pub fn new(dbus_tx: mpsc::Sender<DbusRequest>) -> Self {
        // DBus 컨텍스트 생성
        let context_path = create_dbus_context(&dbus_tx);

        Self {
            seat: None,
            im_manager: None,
            vk_manager: None,
            compositor: None,
            shm: None,
            input_method: None,
            keyboard_grab: None,
            virtual_keyboard: None,
            serial: 0,
            pending_activate: false,
            pending_deactivate: false,
            current_active: false,
            grab_active: false,
            keymap_init: false,
            keymap_handler: KeymapHandler::new(),
            last_preedit: String::new(),
            repeat_timer: RepeatTimer::new(),
            repeat_info: None,
            press_state: PressState::NotPressing,

            dbus_tx,
            context_path,
            should_exit: false,
            qh: None,
            popup_surface: PopupSurface::new(),
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

        // 팝업 서피스 초기화
        if let (Some(ref compositor), Some(ref im_ref)) = (&self.compositor, &self.input_method) {
            self.popup_surface.init(compositor, im_ref, qh);
        }

        true
    }

    /// 키 이벤트를 DBus로 전송하고 결과 처리
    fn process_key_via_dbus(&mut self, evdev_keycode: u32, time: u32, key_state_raw: u32) -> bool {
        let keysym = self.keymap_handler.get_keysym(evdev_keycode);
        let mod_state = self.keymap_handler.mod_state;
        // evdev keycode → hardware code (XKB 호환: +8)
        let keycode = evdev_keycode + 8;

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
                state: mod_state,
                response: Some(response_tx),
            })
            .is_err()
        {
            return false;
        }

        match response_rx.recv_timeout(std::time::Duration::from_millis(500)) {
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

    /// 미소비 키를 virtual keyboard로 포워딩
    fn forward_key(&self, time: u32, key: u32, state: u32) {
        if let Some(ref vk) = self.virtual_keyboard {
            vk.key(time, key, state);
        }
    }

    /// 포커스 아웃 시 조합 중인 텍스트 커밋
    fn handle_deactivate(&mut self) {
        // 한자/특수문자 팝업 모드 취소 (활성 아니면 no-op)
        let _ = self.dbus_tx.blocking_send(DbusRequest::CancelHanja {
            context_path: self.context_path.clone(),
        });
        let _ = self.dbus_tx.blocking_send(DbusRequest::CancelSpecialChar {
            context_path: self.context_path.clone(),
        });

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

        // 팝업 숨기기
        self.popup_surface.hide();
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

            // 활성 상태에서만 키 반복 처리
            if self.grab_active {
                self.process_key_via_dbus(key, 0, KeyState::Pressed as u32);
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
        _proxy: &WlSeat,
        _event: wayland_client::protocol::wl_seat::Event,
        _data: &(),
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
    ) {
        // Seat 이벤트 처리 불필요
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

                    // DBus FocusIn
                    let _ = state.dbus_tx.blocking_send(DbusRequest::FocusIn {
                        context_path: state.context_path.clone(),
                        window_id: "unim-wayland".to_string(),
                    });
                } else if should_deactivate {
                    unim_log!("WAYLAND", "Done → 비활성화 (serial={})", state.serial);
                    state.handle_deactivate();
                    state.grab_active = false;
                    state.current_active = false;
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

            zwp_input_method_v2::Event::ContentType { .. } => {
                // content type hint (현재 미사용)
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
                    // 활성 상태에서 키 눌림 → DBus 엔진에 전달
                    let consumed = state.process_key_via_dbus(key, time, KeyState::Pressed as u32);

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
