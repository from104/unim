//! 4개 탭과 헤더가 공유하는 단일 진실 공급원.
//!
//! - `registry`  : `ProfileRegistry` 래퍼. 사용자 자판 디렉토리 자동 스캔.
//! - `editor`    : 현재 선택된 자판의 변경 버퍼 (EditorState).
//! - `is_builtin`: 저장 정책 분기 — true 면 'Save' disable, 'Save As' 만.
//! - `toast`     : 모든 탭/다이얼로그가 공유하는 알림 오버레이.
//! - `action_group` : `win.*` 액션 그룹. 헤더·본문 양쪽에 동일하게 install.
//! - `refresh_callbacks` : 자판 선택 변경 시 호출할 탭별 refresh 콜백.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use gtk4::gio;
use libadwaita as adw;

use unim_keymap_common::SharedRegistry;

use crate::state::editor_state::EditorState;

/// 자판 선택 변경 시 호출되는 탭별 refresh 콜백.
pub type RefreshCallback = Rc<dyn Fn()>;
/// 언어 변경 옵저버 — bool 은 is_korean.
pub type LanguageObserver = Rc<dyn Fn(bool)>;

pub struct AppState {
    pub registry: SharedRegistry,
    pub current_name: RefCell<Option<String>>,
    pub editor: RefCell<Option<EditorState>>,
    pub is_builtin: Cell<bool>,
    pub toast: adw::ToastOverlay,
    pub action_group: gio::SimpleActionGroup,
    pub refresh_callbacks: RefCell<Vec<RefreshCallback>>,
    /// 기본 탭에서 언어를 바꿨을 때 호출 — app.rs 가 "조합"·"확장" 탭 가시성 토글.
    pub language_observers: RefCell<Vec<LanguageObserver>>,
    /// editor 를 교체한 뒤 전체 UI(타이틀·드롭다운·탭 가시성·refresh)를 갱신하는
    /// 단일 진입점. app.rs 가 1회 등록. 다이얼로그(새 자판/복제/가져오기)가 호출.
    pub ui_refresh: RefCell<Option<RefreshCallback>>,
}

pub type SharedAppState = Rc<AppState>;

impl AppState {
    pub fn new(registry: SharedRegistry, toast: adw::ToastOverlay) -> SharedAppState {
        Rc::new(Self {
            registry,
            current_name: RefCell::new(None),
            editor: RefCell::new(None),
            is_builtin: Cell::new(false),
            toast,
            action_group: gio::SimpleActionGroup::new(),
            refresh_callbacks: RefCell::new(Vec::new()),
            language_observers: RefCell::new(Vec::new()),
            ui_refresh: RefCell::new(None),
        })
    }

    /// 전체 UI 갱신 진입점 등록 (app.rs 1회).
    pub fn set_ui_refresh(&self, cb: RefreshCallback) {
        *self.ui_refresh.borrow_mut() = Some(cb);
    }

    /// 전체 UI 갱신 호출 (editor 교체 후).
    pub fn run_ui_refresh(&self) {
        let cb = self.ui_refresh.borrow().clone();
        if let Some(cb) = cb {
            cb();
        }
    }

    /// 탭이 자기 refresh 콜백을 등록 — 자판 선택 변경 시 일괄 호출.
    pub fn register_refresh(&self, cb: RefreshCallback) {
        self.refresh_callbacks.borrow_mut().push(cb);
    }

    /// 모든 탭 refresh 콜백 호출.
    pub fn refresh_all(&self) {
        let callbacks: Vec<RefreshCallback> = self.refresh_callbacks.borrow().clone();
        for cb in callbacks {
            cb();
        }
    }

    /// 언어 변경 옵저버 등록 (app.rs 가 탭 가시성 토글용으로 1개 등록).
    pub fn register_language_observer(&self, cb: LanguageObserver) {
        self.language_observers.borrow_mut().push(cb);
    }

    /// 언어 변경 통지 — is_korean 전달.
    pub fn notify_language(&self, is_korean: bool) {
        let observers: Vec<LanguageObserver> = self.language_observers.borrow().clone();
        for cb in observers {
            cb(is_korean);
        }
    }

    /// 토스트 띄우기 헬퍼.
    pub fn toast(&self, text: &str) {
        self.toast.add_toast(adw::Toast::new(text));
    }
}
