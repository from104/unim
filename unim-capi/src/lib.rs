//! UNIM C API 바인딩
//!
//! GTK, Qt 등의 IM 모듈에서 사용할 수 있는 C FFI 레이어입니다.

#![allow(clippy::missing_safety_doc)]

use unim::config::{Config, InputCategory};
use unim::input_engine::{InputEngine, InputResult};
use unim::keycode::ModifierState;
use unim::popup::{PopupKey, PopupKeyResult, PopupKind, PopupState};

/// API 버전
pub const UNIM_API_VERSION: usize = 1;

/// UTF-8 문자열 래퍼
///
/// C에서 안전하게 Rust 문자열을 참조할 수 있도록 합니다.
#[repr(C)]
pub struct UnimStr {
    /// 문자열 데이터 포인터
    pub ptr: *const u8,
    /// 문자열 길이 (바이트)
    pub len: usize,
}

impl UnimStr {
    /// Rust 문자열 슬라이스로부터 UnimStr을 생성합니다.
    pub fn new(s: &str) -> Self {
        Self {
            ptr: s.as_ptr(),
            len: s.len(),
        }
    }

    /// 빈 문자열을 생성합니다.
    pub fn empty() -> Self {
        Self {
            ptr: std::ptr::null(),
            len: 0,
        }
    }
}

// ============================================
// API 버전
// ============================================

/// API 버전을 반환합니다.
#[no_mangle]
pub extern "C" fn unim_api_version() -> usize {
    UNIM_API_VERSION
}

// ============================================
// 설정 관리
// ============================================

/// 기본 경로에서 설정을 로드합니다.
///
/// 설정 파일이 없거나 파싱 실패 시 기본 설정을 생성하고 저장합니다.
///
/// # 반환값
///
/// Config 포인터. 사용 후 `unim_config_delete`로 해제해야 합니다.
#[no_mangle]
pub extern "C" fn unim_config_load() -> *mut Config {
    let config = Config::load_from_default_path();
    Box::into_raw(Box::new(config))
}

/// 설정 파일 존재 및 유효성을 보장합니다.
///
/// 파일이 없거나 유효하지 않으면 기본 설정으로 생성합니다.
///
/// # 반환값
///
/// 성공 시 true, 실패 시 false
#[no_mangle]
pub extern "C" fn unim_config_ensure_file() -> bool {
    Config::ensure_config_file().is_some()
}

/// 기본 설정을 생성합니다.
///
/// # 반환값
///
/// Config 포인터. 사용 후 `unim_config_delete`로 해제해야 합니다.
#[no_mangle]
pub extern "C" fn unim_config_default() -> *mut Config {
    Box::into_raw(Box::new(Config::default()))
}

/// Config를 해제합니다.
///
/// # Safety
///
/// `config`는 `unim_config_load` 또는 `unim_config_default`로 생성된 유효한 포인터여야 합니다.
#[no_mangle]
pub unsafe extern "C" fn unim_config_delete(config: *mut Config) {
    if !config.is_null() {
        drop(Box::from_raw(config));
    }
}

/// 설정 파일이 변경되어 다시 로드가 필요한지 확인합니다.
///
/// # Arguments
///
/// * `config` - Config 참조
///
/// # 반환값
///
/// 재로드가 필요하면 true
#[no_mangle]
pub extern "C" fn unim_config_needs_reload(config: &Config) -> bool {
    config.needs_reload()
}

/// 설정 파일이 변경되었으면 다시 로드합니다.
///
/// # Arguments
///
/// * `config` - Config 참조 (mutable)
///
/// # 반환값
///
/// 재로드 성공 시 true, 변경이 없거나 실패 시 false
#[no_mangle]
pub extern "C" fn unim_config_reload(config: &mut Config) -> bool {
    config.reload_if_changed()
}

// ============================================
// 엔진 생성/삭제
// ============================================

/// 새로운 InputEngine을 생성합니다.
///
/// # Arguments
///
/// * `config` - 설정 참조
///
/// # 반환값
///
/// InputEngine 포인터. 사용 후 `unim_engine_delete`로 해제해야 합니다.
#[no_mangle]
pub extern "C" fn unim_engine_new(config: &Config) -> *mut InputEngine {
    Box::into_raw(Box::new(InputEngine::new(config)))
}

/// InputEngine을 해제합니다.
///
/// # Safety
///
/// `engine`은 `unim_engine_new`로 생성된 유효한 포인터여야 합니다.
#[no_mangle]
pub unsafe extern "C" fn unim_engine_delete(engine: *mut InputEngine) {
    if !engine.is_null() {
        drop(Box::from_raw(engine));
    }
}

// ============================================
// 입력 처리
// ============================================

/// 키를 처리합니다.
///
/// # Arguments
///
/// * `engine` - InputEngine 참조
/// * `config` - Config 참조
/// * `hardware_code` - 하드웨어 키코드 (evdev)
/// * `state` - 수정자 키 상태
///
/// # 반환값
///
/// 입력 처리 결과
#[no_mangle]
pub extern "C" fn unim_engine_press_key(
    engine: &mut InputEngine,
    config: &Config,
    hardware_code: u16,
    state: ModifierState,
) -> InputResult {
    engine.press_key_code(hardware_code, state, config)
}

/// commit 문자열을 반환합니다.
///
/// 반환된 문자열은 engine이 살아있는 동안만 유효합니다.
///
/// # Arguments
///
/// * `engine` - InputEngine 참조
#[no_mangle]
pub extern "C" fn unim_engine_commit_str(engine: &InputEngine) -> UnimStr {
    UnimStr::new(engine.commit_str())
}

/// preedit 문자열을 반환합니다.
///
/// 반환된 문자열은 engine이 살아있는 동안만 유효합니다.
///
/// # Arguments
///
/// * `engine` - InputEngine 참조
#[no_mangle]
pub extern "C" fn unim_engine_preedit_str(engine: &InputEngine) -> UnimStr {
    UnimStr::new(engine.preedit_str())
}

// ============================================
// 상태 관리
// ============================================

/// 입력 카테고리를 설정합니다.
#[no_mangle]
pub extern "C" fn unim_engine_set_input_category(
    engine: &mut InputEngine,
    category: InputCategory,
) {
    engine.set_input_category(category);
}

/// 한국어 레이아웃을 설정합니다.
///
/// Phase 8: `KoreanLayout` enum이 프로필 이름 문자열로 통합됨. `layout`에
/// C 문자열로 프로필 이름(`"ko_2bulstd"` / `"ko_3bul390"` / ... 또는 사용자
/// 프로필 이름)을 전달. 레거시 값(`"Dubeolsik"` 등)도 자동 승격.
///
/// # 반환값
/// 성공 시 `true`, NULL·UTF-8 오류·빈 문자열이면 `false` (변경 없음).
#[no_mangle]
pub unsafe extern "C" fn unim_config_set_korean_layout(
    config: &mut Config,
    layout: *const std::os::raw::c_char,
) -> bool {
    if layout.is_null() {
        return false;
    }
    let s = match std::ffi::CStr::from_ptr(layout).to_str() {
        Ok(s) if !s.trim().is_empty() => s.trim(),
        _ => return false,
    };
    config.engine.korean.layout = unim::config::normalize_korean_layout_name(s);
    true
}

/// 영어 레이아웃을 설정합니다.
///
/// Phase 9: EnglishLayout enum이 프로필 이름 문자열로 통합됨. `layout`에
/// C 문자열로 프로필 이름(`"qwerty"` 등)을 전달. 레거시 값(`"Qwerty"` 등)도 자동 승격.
#[no_mangle]
pub unsafe extern "C" fn unim_config_set_english_layout(
    config: &mut Config,
    layout: *const std::os::raw::c_char,
) -> bool {
    if layout.is_null() {
        return false;
    }
    let s = match std::ffi::CStr::from_ptr(layout).to_str() {
        Ok(s) if !s.trim().is_empty() => s.trim(),
        _ => return false,
    };
    config.engine.english.layout = unim::config::normalize_english_layout_name(s);
    true
}

/// 엔진의 한국어 레이아웃을 즉시 변경합니다.
///
/// Phase 8: `layout`는 C 문자열(프로필 이름). 레거시 값도 자동 승격.
///
/// # 반환값
/// 성공 시 `true`, NULL·UTF-8 오류·빈 문자열이면 `false`.
#[no_mangle]
pub unsafe extern "C" fn unim_engine_set_korean_layout(
    engine: &mut InputEngine,
    layout: *const std::os::raw::c_char,
) -> bool {
    if layout.is_null() {
        return false;
    }
    let s = match std::ffi::CStr::from_ptr(layout).to_str() {
        Ok(s) if !s.trim().is_empty() => s.trim(),
        _ => return false,
    };
    engine.set_korean_layout(unim::config::normalize_korean_layout_name(s));
    true
}

/// 엔진의 영어 레이아웃을 즉시 변경합니다.
#[no_mangle]
pub unsafe extern "C" fn unim_engine_set_english_layout(
    engine: &mut InputEngine,
    layout: *const std::os::raw::c_char,
) -> bool {
    if layout.is_null() {
        return false;
    }
    let s = match std::ffi::CStr::from_ptr(layout).to_str() {
        Ok(s) if !s.trim().is_empty() => s.trim(),
        _ => return false,
    };
    engine.set_english_layout(unim::config::normalize_english_layout_name(s));
    true
}

/// 현재 입력 카테고리를 반환합니다.
#[no_mangle]
pub extern "C" fn unim_engine_get_input_category(engine: &InputEngine) -> InputCategory {
    engine.input_category()
}

/// 엔진 상태를 리셋합니다.
#[no_mangle]
pub extern "C" fn unim_engine_reset(engine: &mut InputEngine) {
    engine.reset();
}

/// commit 버퍼를 비웁니다.
#[no_mangle]
pub extern "C" fn unim_engine_clear_commit(engine: &mut InputEngine) {
    engine.clear_commit();
}

/// preedit을 비웁니다 (commit으로 플러시).
#[no_mangle]
pub extern "C" fn unim_engine_clear_preedit(engine: &mut InputEngine) {
    engine.clear_preedit();
}

/// preedit을 제거합니다 (commit 없이).
#[no_mangle]
pub extern "C" fn unim_engine_remove_preedit(engine: &mut InputEngine) {
    engine.remove_preedit();
}

/// 조합 중인지 확인합니다.
#[no_mangle]
pub extern "C" fn unim_engine_is_composing(engine: &InputEngine) -> bool {
    engine.is_composing()
}

/// ready 상태 확인 (프론트엔드 호환용)
#[no_mangle]
pub extern "C" fn unim_engine_check_ready(engine: &InputEngine) -> bool {
    engine.check_ready()
}

/// ready 상태 종료 (프론트엔드 호환용)
#[no_mangle]
pub extern "C" fn unim_engine_end_ready(engine: &mut InputEngine) -> InputResult {
    engine.end_ready()
}

// ============================================
// 설정 관리 (Settings UI용)
// ============================================

/// 설정을 기본 경로에 저장합니다.
///
/// # 반환값
///
/// 성공 시 true, 실패 시 false
#[no_mangle]
pub extern "C" fn unim_config_save(config: &Config) -> bool {
    config.save_to_default_path().is_ok()
}

/// 현재 한국어 레이아웃 프로필 이름을 반환합니다 (UnimStr, caller가 소유권 이전).
#[no_mangle]
pub extern "C" fn unim_config_get_korean_layout(config: &Config) -> UnimStr {
    UnimStr::new(config.engine.korean.layout.as_str())
}

/// 현재 영어 레이아웃을 반환합니다.
#[no_mangle]
pub extern "C" fn unim_config_get_english_layout(config: &Config) -> UnimStr {
    UnimStr::new(config.engine.english.layout.as_str())
}

/// 기본(초기) 입력 카테고리를 반환합니다.
///
/// # 반환값
///
/// 0 = English, 1 = Korean
#[no_mangle]
pub extern "C" fn unim_config_get_default_category(config: &Config) -> InputCategory {
    config.engine.default_category
}

/// 기본(초기) 입력 카테고리를 설정합니다.
///
/// # Arguments
///
/// * `category` - InputCategory 열거형 값 (Korean 또는 English)
#[no_mangle]
pub extern "C" fn unim_config_set_default_category(config: &mut Config, category: InputCategory) {
    config.engine.default_category = category;
}

/// 입력 모드 공유 방식을 반환합니다.
///
/// # 반환값
///
/// 0 = Global (전역 공유), 1 = PerApp (앱별 독립)
#[no_mangle]
pub extern "C" fn unim_config_get_mode_sharing(config: &Config) -> unim::config::ModeSharingMode {
    config.engine.mode_sharing
}

/// 입력 모드 공유 방식을 설정합니다.
///
/// # Arguments
///
/// * `mode` - ModeSharingMode 열거형 값
#[no_mangle]
pub extern "C" fn unim_config_set_mode_sharing(
    config: &mut Config,
    mode: unim::config::ModeSharingMode,
) {
    config.engine.mode_sharing = mode;
}

/// 모드 공유 방식 개수를 반환합니다.
#[no_mangle]
pub extern "C" fn unim_mode_sharing_count() -> usize {
    unim::config::ModeSharingMode::all().len()
}

/// 모드 공유 방식 표시 이름을 반환합니다 (UI용).
#[no_mangle]
pub extern "C" fn unim_mode_sharing_display_name(mode: unim::config::ModeSharingMode) -> UnimStr {
    UnimStr::new(mode.display_name())
}

/// 인덱스로 모드 공유 방식을 가져옵니다.
#[no_mangle]
pub extern "C" fn unim_mode_sharing_at(index: usize) -> unim::config::ModeSharingMode {
    unim::config::ModeSharingMode::all()
        .get(index)
        .copied()
        .unwrap_or(unim::config::ModeSharingMode::Global)
}

// ============================================
// 레이아웃 열거형 헬퍼 (UI 표시용)
// ============================================

/// 내장 한국어 자판의 개수를 반환합니다.
///
/// 현재 4종 (ko_2bulstd, ko_3bul390, ko_3bul391, ko_3bul_noshift). 값은
/// `KOREAN_LAYOUT_BUILTINS`에서 동적으로 산출하므로 빌트인이 늘면 자동 반영된다.
/// 사용자 정의 프로필은 `~/.config/unim/layouts/` 스캔 경로를 쓰므로 여기서 세지 않는다.
#[no_mangle]
pub extern "C" fn unim_korean_layout_count() -> usize {
    unim::config::KOREAN_LAYOUT_BUILTINS.len()
}

/// 인덱스로 내장 한국어 자판의 정식 이름을 반환합니다. 범위 밖이면 빈 문자열.
#[no_mangle]
pub extern "C" fn unim_korean_layout_name(index: usize) -> UnimStr {
    let name = unim::config::KOREAN_LAYOUT_BUILTINS
        .get(index)
        .copied()
        .unwrap_or("");
    UnimStr::new(name)
}

/// 인덱스로 내장 한국어 자판의 표시 이름을 반환합니다 (UI용).
#[no_mangle]
pub extern "C" fn unim_korean_layout_display_name(index: usize) -> UnimStr {
    let name = unim::config::KOREAN_LAYOUT_BUILTINS
        .get(index)
        .copied()
        .unwrap_or("");
    UnimStr::new(unim::config::korean_layout_display_name(name))
}

/// 지원하는 영어 레이아웃 개수를 반환합니다.
#[no_mangle]
pub extern "C" fn unim_english_layout_count() -> usize {
    unim::config::ENGLISH_LAYOUT_BUILTINS.len()
}

/// 영어 레이아웃 이름을 반환합니다.
#[no_mangle]
pub extern "C" fn unim_english_layout_name(index: usize) -> UnimStr {
    let name = unim::config::ENGLISH_LAYOUT_BUILTINS
        .get(index)
        .copied()
        .unwrap_or("");
    UnimStr::new(name)
}

/// 영어 레이아웃 표시 이름을 반환합니다 (UI용).
#[no_mangle]
pub extern "C" fn unim_english_layout_display_name(index: usize) -> UnimStr {
    let name = unim::config::ENGLISH_LAYOUT_BUILTINS
        .get(index)
        .copied()
        .unwrap_or("");
    UnimStr::new(unim::config::english_layout_display_name(name))
}

/// 인덱스로 내장 한국어 자판의 정식 프로필 이름을 반환합니다 (String alias).
#[no_mangle]
pub extern "C" fn unim_korean_layout_at(index: usize) -> UnimStr {
    unim_korean_layout_name(index)
}

/// 인덱스로 내장 영어 자판의 정식 프로필 이름을 반환합니다 (레거시 alias).
#[no_mangle]
pub extern "C" fn unim_english_layout_at(index: usize) -> UnimStr {
    unim_english_layout_name(index)
}

// ============================================
// 상태 파일 관리
// ============================================

/// 상태 파일에서 현재 입력 모드를 읽습니다.
///
/// # 반환값
///
/// 0 = English, 1 = Korean, -1 = 오류
#[no_mangle]
pub extern "C" fn unim_status_get() -> i32 {
    match unim::status::get_status() {
        Ok(unim::status::InputCategory::English) => 0,
        Ok(unim::status::InputCategory::Korean) => 1,
        Err(_) => -1,
    }
}

/// 상태 파일에 입력 모드를 씁니다.
///
/// # Arguments
///
/// * `category` - 0 = English, 1 = Korean
///
/// # 반환값
///
/// 성공 시 true, 실패 시 false
#[no_mangle]
pub extern "C" fn unim_status_set(category: i32) -> bool {
    let cat = match category {
        0 => unim::status::InputCategory::English,
        1 => unim::status::InputCategory::Korean,
        _ => return false,
    };
    unim::status::set_status(cat).is_ok()
}

// ============================================
// 팝업 상태 관리 (C-API)
// ============================================

/// PopupKey C 표현 상수
pub mod popup_key_constants {
    pub const POPUP_KEY_NUMBER_1: u32 = 1;
    pub const POPUP_KEY_NUMBER_9: u32 = 9;
    pub const POPUP_KEY_LETTER_0: u32 = 10; // Q
    pub const POPUP_KEY_LETTER_8: u32 = 18; // O
    pub const POPUP_KEY_UP: u32 = 20;
    pub const POPUP_KEY_DOWN: u32 = 21;
    pub const POPUP_KEY_LEFT: u32 = 22;
    pub const POPUP_KEY_RIGHT: u32 = 23;
    pub const POPUP_KEY_ENTER: u32 = 24;
    pub const POPUP_KEY_ESCAPE: u32 = 25;
    pub const POPUP_KEY_TAB: u32 = 26;
    pub const POPUP_KEY_SHIFT_TAB: u32 = 27;
    pub const POPUP_KEY_PAGE_UP: u32 = 28;
    pub const POPUP_KEY_PAGE_DOWN: u32 = 29;
    pub const POPUP_KEY_SPACE: u32 = 30;
    pub const POPUP_KEY_BACKSPACE: u32 = 31;
    pub const POPUP_KEY_PERIOD: u32 = 34;
    pub const POPUP_KEY_MODIFIER: u32 = 32;
    pub const POPUP_KEY_OTHER: u32 = 33;
}

/// PopupKeyResult C 표현 상수
pub mod popup_result_constants {
    pub const POPUP_RESULT_SELECT: i32 = 0;
    pub const POPUP_RESULT_CANCEL: i32 = 1;
    pub const POPUP_RESULT_UPDATED: i32 = 2;
    pub const POPUP_RESULT_CONSUMED: i32 = 3;
    pub const POPUP_RESULT_NOT_HANDLED: i32 = 4;
    pub const POPUP_RESULT_TOGGLE_BOOKMARK: i32 = 5;
}

/// C용 PopupKeyResult 구조체
#[repr(C)]
pub struct CPopupKeyResult {
    /// 결과 종류 (popup_result_constants 참조)
    pub kind: i32,
    /// Select인 경우 선택된 전체 인덱스, 그 외 -1
    pub selected_index: i32,
}

/// u32 → PopupKey 변환
fn u32_to_popup_key(key: u32) -> PopupKey {
    use popup_key_constants::*;
    match key {
        POPUP_KEY_NUMBER_1..=POPUP_KEY_NUMBER_9 => PopupKey::Number(key as u8),
        POPUP_KEY_LETTER_0..=POPUP_KEY_LETTER_8 => {
            PopupKey::Letter((key - POPUP_KEY_LETTER_0) as u8)
        }
        POPUP_KEY_UP => PopupKey::Up,
        POPUP_KEY_DOWN => PopupKey::Down,
        POPUP_KEY_LEFT => PopupKey::Left,
        POPUP_KEY_RIGHT => PopupKey::Right,
        POPUP_KEY_ENTER => PopupKey::Enter,
        POPUP_KEY_ESCAPE => PopupKey::Escape,
        POPUP_KEY_TAB => PopupKey::Tab,
        POPUP_KEY_SHIFT_TAB => PopupKey::ShiftTab,
        POPUP_KEY_PAGE_UP => PopupKey::PageUp,
        POPUP_KEY_PAGE_DOWN => PopupKey::PageDown,
        POPUP_KEY_SPACE => PopupKey::Space,
        POPUP_KEY_BACKSPACE => PopupKey::Backspace,
        POPUP_KEY_PERIOD => PopupKey::Period,
        POPUP_KEY_MODIFIER => PopupKey::Modifier,
        _ => PopupKey::Other,
    }
}

/// GDK keyval → PopupKey u32 변환
#[no_mangle]
pub extern "C" fn unim_popup_key_from_gdk(gdk_keyval: u32) -> u32 {
    use popup_key_constants::*;
    match gdk_keyval {
        0x31..=0x39 => POPUP_KEY_NUMBER_1 + (gdk_keyval - 0x31), // '1'-'9'
        0x71 => POPUP_KEY_LETTER_0,                              // q
        0x77 => POPUP_KEY_LETTER_0 + 1,                          // w
        0x65 => POPUP_KEY_LETTER_0 + 2,                          // e
        0x72 => POPUP_KEY_LETTER_0 + 3,                          // r
        0x74 => POPUP_KEY_LETTER_0 + 4,                          // t
        0x79 => POPUP_KEY_LETTER_0 + 5,                          // y
        0x75 => POPUP_KEY_LETTER_0 + 6,                          // u
        0x69 => POPUP_KEY_LETTER_0 + 7,                          // i
        0x6f => POPUP_KEY_LETTER_0 + 8,                          // o
        0xff52 => POPUP_KEY_UP,
        0xff54 => POPUP_KEY_DOWN,
        0xff51 => POPUP_KEY_LEFT,
        0xff53 => POPUP_KEY_RIGHT,
        0xff0d => POPUP_KEY_ENTER,
        0xff1b => POPUP_KEY_ESCAPE,
        0xff09 => POPUP_KEY_TAB,
        0xfe20 => POPUP_KEY_SHIFT_TAB,
        0xff55 => POPUP_KEY_PAGE_UP,
        0xff56 => POPUP_KEY_PAGE_DOWN,
        0x20 => POPUP_KEY_SPACE,
        0xff08 => POPUP_KEY_BACKSPACE,
        0x2e => POPUP_KEY_PERIOD,
        0xffe1..=0xffee | 0xff7f | 0xff14 => POPUP_KEY_MODIFIER,
        _ => POPUP_KEY_OTHER,
    }
}

/// Qt key → PopupKey u32 변환
#[no_mangle]
pub extern "C" fn unim_popup_key_from_qt(qt_key: i32) -> u32 {
    use popup_key_constants::*;
    match qt_key {
        0x31..=0x39 => POPUP_KEY_NUMBER_1 + (qt_key as u32 - 0x31),
        0x51 => POPUP_KEY_LETTER_0,     // Qt::Key_Q
        0x57 => POPUP_KEY_LETTER_0 + 1, // Qt::Key_W
        0x45 => POPUP_KEY_LETTER_0 + 2, // Qt::Key_E
        0x52 => POPUP_KEY_LETTER_0 + 3, // Qt::Key_R
        0x54 => POPUP_KEY_LETTER_0 + 4, // Qt::Key_T
        0x59 => POPUP_KEY_LETTER_0 + 5, // Qt::Key_Y
        0x55 => POPUP_KEY_LETTER_0 + 6, // Qt::Key_U
        0x49 => POPUP_KEY_LETTER_0 + 7, // Qt::Key_I
        0x4f => POPUP_KEY_LETTER_0 + 8, // Qt::Key_O
        0x01000013 => POPUP_KEY_UP,
        0x01000015 => POPUP_KEY_DOWN,
        0x01000012 => POPUP_KEY_LEFT,
        0x01000014 => POPUP_KEY_RIGHT,
        0x01000004 | 0x01000005 => POPUP_KEY_ENTER,
        0x01000000 => POPUP_KEY_ESCAPE,
        0x01000001 => POPUP_KEY_TAB,
        0x01000002 => POPUP_KEY_SHIFT_TAB, // Qt::Key_Backtab
        0x01000016 => POPUP_KEY_PAGE_UP,
        0x01000017 => POPUP_KEY_PAGE_DOWN,
        0x20 => POPUP_KEY_SPACE,
        0x01000003 => POPUP_KEY_BACKSPACE,
        0x2e => POPUP_KEY_PERIOD,
        0x01000020..=0x01000023 | 0x01001103 => POPUP_KEY_MODIFIER,
        _ => POPUP_KEY_OTHER,
    }
}

/// 한자 팝업 상태 생성
///
/// # Safety
///
/// `target`은 유효한 UTF-8 문자열 포인터, `candidates`는 (한자, 뜻) 쌍의 배열이어야 합니다.
#[no_mangle]
pub unsafe extern "C" fn unim_popup_new_hanja(
    target_ptr: *const u8,
    target_len: usize,
    hanja_ptrs: *const *const u8,
    hanja_lens: *const usize,
    meaning_ptrs: *const *const u8,
    meaning_lens: *const usize,
    count: usize,
) -> *mut PopupState {
    let target = std::str::from_utf8_unchecked(std::slice::from_raw_parts(target_ptr, target_len));
    let mut candidates = Vec::with_capacity(count);
    for i in 0..count {
        let h = std::str::from_utf8_unchecked(std::slice::from_raw_parts(
            *hanja_ptrs.add(i),
            *hanja_lens.add(i),
        ))
        .to_string();
        let m = std::str::from_utf8_unchecked(std::slice::from_raw_parts(
            *meaning_ptrs.add(i),
            *meaning_lens.add(i),
        ))
        .to_string();
        candidates.push((h, m));
    }
    Box::into_raw(Box::new(PopupState::new_hanja(target, candidates)))
}

/// 특수문자 팝업 상태 생성
///
/// # Safety
///
/// 모든 포인터는 유효한 UTF-8 문자열이어야 합니다.
#[no_mangle]
pub unsafe extern "C" fn unim_popup_new_special(
    target_ptr: *const u8,
    target_len: usize,
    char_ptrs: *const *const u8,
    char_lens: *const usize,
    count: usize,
    top_row_ptr: *const u8,
    top_row_len: usize,
) -> *mut PopupState {
    let target = std::str::from_utf8_unchecked(std::slice::from_raw_parts(target_ptr, target_len));
    let top_row =
        std::str::from_utf8_unchecked(std::slice::from_raw_parts(top_row_ptr, top_row_len));
    let mut characters = Vec::with_capacity(count);
    for i in 0..count {
        let ch = std::str::from_utf8_unchecked(std::slice::from_raw_parts(
            *char_ptrs.add(i),
            *char_lens.add(i),
        ))
        .to_string();
        characters.push(ch);
    }
    Box::into_raw(Box::new(PopupState::new_special(
        target, characters, top_row,
    )))
}

/// 팝업 상태 해제
///
/// # Safety
///
/// `state`는 `unim_popup_new_*`로 생성된 유효한 포인터여야 합니다.
#[no_mangle]
pub unsafe extern "C" fn unim_popup_free(state: *mut PopupState) {
    if !state.is_null() {
        drop(Box::from_raw(state));
    }
}

/// 팝업 키 처리
#[no_mangle]
pub extern "C" fn unim_popup_handle_key(state: &mut PopupState, popup_key: u32) -> CPopupKeyResult {
    let key = u32_to_popup_key(popup_key);
    match state.handle_key(key) {
        PopupKeyResult::Select(idx) => CPopupKeyResult {
            kind: popup_result_constants::POPUP_RESULT_SELECT,
            selected_index: idx as i32,
        },
        PopupKeyResult::ToggleBookmark(idx) => CPopupKeyResult {
            kind: popup_result_constants::POPUP_RESULT_TOGGLE_BOOKMARK,
            selected_index: idx as i32,
        },
        PopupKeyResult::Cancel => CPopupKeyResult {
            kind: popup_result_constants::POPUP_RESULT_CANCEL,
            selected_index: -1,
        },
        PopupKeyResult::Updated => CPopupKeyResult {
            kind: popup_result_constants::POPUP_RESULT_UPDATED,
            selected_index: -1,
        },
        PopupKeyResult::Consumed => CPopupKeyResult {
            kind: popup_result_constants::POPUP_RESULT_CONSUMED,
            selected_index: -1,
        },
        PopupKeyResult::NotHandled => CPopupKeyResult {
            kind: popup_result_constants::POPUP_RESULT_NOT_HANDLED,
            selected_index: -1,
        },
    }
}

/// 마우스 클릭 처리
#[no_mangle]
pub extern "C" fn unim_popup_handle_click(
    state: &mut PopupState,
    row: i32,
    col: i32,
) -> CPopupKeyResult {
    if row < 0 || col < 0 {
        return CPopupKeyResult {
            kind: popup_result_constants::POPUP_RESULT_CONSUMED,
            selected_index: -1,
        };
    }
    match state.handle_click(row as usize, col as usize) {
        PopupKeyResult::Select(idx) => CPopupKeyResult {
            kind: popup_result_constants::POPUP_RESULT_SELECT,
            selected_index: idx as i32,
        },
        other => CPopupKeyResult {
            kind: match other {
                PopupKeyResult::Cancel => popup_result_constants::POPUP_RESULT_CANCEL,
                PopupKeyResult::Updated => popup_result_constants::POPUP_RESULT_UPDATED,
                PopupKeyResult::Consumed => popup_result_constants::POPUP_RESULT_CONSUMED,
                PopupKeyResult::NotHandled => popup_result_constants::POPUP_RESULT_NOT_HANDLED,
                PopupKeyResult::ToggleBookmark(_) => {
                    popup_result_constants::POPUP_RESULT_TOGGLE_BOOKMARK
                }
                PopupKeyResult::Select(_) => unreachable!(),
            },
            selected_index: -1,
        },
    }
}

// --- 팝업 ViewModel 쿼리 ---

/// 팝업 종류 반환 (0=Hanja, 1=SpecialChar, 2=Emoji)
#[no_mangle]
pub extern "C" fn unim_popup_get_kind(state: &PopupState) -> i32 {
    match state.kind() {
        PopupKind::Hanja => 0,
        PopupKind::SpecialChar => 1,
        PopupKind::Emoji => 2,
    }
}

/// 현재 행 수
#[no_mangle]
pub extern "C" fn unim_popup_get_rows(state: &PopupState) -> i32 {
    state.rows() as i32
}

/// 현재 열 수
#[no_mangle]
pub extern "C" fn unim_popup_get_cols(state: &PopupState) -> i32 {
    state.cols() as i32
}

/// 선택 행
#[no_mangle]
pub extern "C" fn unim_popup_get_sel_row(state: &PopupState) -> i32 {
    state.sel_row() as i32
}

/// 선택 열
#[no_mangle]
pub extern "C" fn unim_popup_get_sel_col(state: &PopupState) -> i32 {
    state.sel_col() as i32
}

/// 현재 페이지 (0-based)
#[no_mangle]
pub extern "C" fn unim_popup_get_current_page(state: &PopupState) -> i32 {
    state.current_page() as i32
}

/// 전체 페이지 수
#[no_mangle]
pub extern "C" fn unim_popup_get_total_pages(state: &PopupState) -> i32 {
    state.total_pages() as i32
}

/// 전체 항목 수
#[no_mangle]
pub extern "C" fn unim_popup_get_total_items(state: &PopupState) -> i32 {
    state.total_items() as i32
}

/// 셀에 문자가 있는지 확인
#[no_mangle]
pub extern "C" fn unim_popup_cell_exists(state: &PopupState, row: i32, col: i32) -> bool {
    if row < 0 || col < 0 {
        return false;
    }
    state.cell_text(row as usize, col as usize).is_some()
}

/// 셀 텍스트 반환 (UnimStr — state가 살아있는 동안 유효)
#[no_mangle]
pub extern "C" fn unim_popup_get_cell_text(state: &PopupState, row: i32, col: i32) -> UnimStr {
    if row < 0 || col < 0 {
        return UnimStr::empty();
    }
    match state.cell_text(row as usize, col as usize) {
        Some(text) => UnimStr::new(text),
        None => UnimStr::empty(),
    }
}

/// 전체 인덱스로 항목 텍스트 반환
#[no_mangle]
pub extern "C" fn unim_popup_get_item(state: &PopupState, index: i32) -> UnimStr {
    if index < 0 {
        return UnimStr::empty();
    }
    match state.get_item(index as usize) {
        Some(text) => UnimStr::new(text),
        None => UnimStr::empty(),
    }
}

/// 전체 인덱스로 뜻 반환 (한자 전용)
#[no_mangle]
pub extern "C" fn unim_popup_get_meaning(state: &PopupState, index: i32) -> UnimStr {
    if index < 0 {
        return UnimStr::empty();
    }
    match state.get_meaning(index as usize) {
        Some(text) => UnimStr::new(text),
        None => UnimStr::empty(),
    }
}

/// 현재 선택된 항목의 전체 인덱스 (-1이면 선택 없음)
#[no_mangle]
pub extern "C" fn unim_popup_selected_index(state: &PopupState) -> i32 {
    match state.selected_global_index() {
        Some(idx) => idx as i32,
        None => -1,
    }
}

/// 대상 문자열 반환
#[no_mangle]
pub extern "C" fn unim_popup_get_target(state: &PopupState) -> UnimStr {
    UnimStr::new(state.target())
}

/// top_row 레이블 반환
#[no_mangle]
pub extern "C" fn unim_popup_get_top_row(state: &PopupState) -> UnimStr {
    UnimStr::new(state.top_row())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_api_version() {
        assert_eq!(unim_api_version(), UNIM_API_VERSION);
    }

    #[test]
    fn test_config_lifecycle() {
        unsafe {
            let config = unim_config_default();
            assert!(!config.is_null());
            unim_config_delete(config);
        }
    }

    #[test]
    fn test_engine_lifecycle() {
        unsafe {
            let config = unim_config_default();
            let engine = unim_engine_new(&*config);
            assert!(!engine.is_null());
            unim_engine_delete(engine);
            unim_config_delete(config);
        }
    }

    #[test]
    fn test_unim_str() {
        let s = "테스트";
        let unim_str = UnimStr::new(s);
        assert_eq!(unim_str.len, s.len());
        assert!(!unim_str.ptr.is_null());

        let empty = UnimStr::empty();
        assert!(empty.ptr.is_null());
        assert_eq!(empty.len, 0);
    }
}
