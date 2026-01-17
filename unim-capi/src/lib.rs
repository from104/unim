//! UNIM C API 바인딩
//!
//! GTK, Qt 등의 IM 모듈에서 사용할 수 있는 C FFI 레이어입니다.

#![allow(clippy::missing_safety_doc)]

use unim::config::{Config, InputCategory};
use unim::input_engine::{InputEngine, InputResult};
use unim::keycode::ModifierState;

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
/// 설정 파일이 없거나 파싱 실패 시 기본값을 반환합니다.
///
/// # 반환값
///
/// Config 포인터. 사용 후 `unim_config_delete`로 해제해야 합니다.
#[no_mangle]
pub extern "C" fn unim_config_load() -> *mut Config {
    let config = Config::load_from_default_path();
    Box::into_raw(Box::new(config))
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
pub extern "C" fn unim_engine_set_input_category(engine: &mut InputEngine, category: InputCategory) {
    engine.set_input_category(category);
}

/// 한글 레이아웃을 설정합니다.
#[no_mangle]
pub extern "C" fn unim_config_set_hangul_layout(config: &mut Config, layout: unim::config::HangulLayout) {
    config.engine.hangul.layout = layout;
}

/// 영문 레이아웃을 설정합니다.
#[no_mangle]
pub extern "C" fn unim_config_set_latin_layout(config: &mut Config, layout: unim::config::LatinLayout) {
    config.engine.latin.layout = layout;
}

/// 엔진의 한글 레이아웃을 즉시 변경합니다.
#[no_mangle]
pub extern "C" fn unim_engine_set_hangul_layout(engine: &mut InputEngine, layout: unim::config::HangulLayout) {
    engine.set_hangul_layout(layout);
}

/// 엔진의 영문 레이아웃을 즉시 변경합니다.
#[no_mangle]
pub extern "C" fn unim_engine_set_latin_layout(engine: &mut InputEngine, layout: unim::config::LatinLayout) {
    engine.set_latin_layout(layout);
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

/// 현재 한글 레이아웃을 반환합니다.
#[no_mangle]
pub extern "C" fn unim_config_get_hangul_layout(config: &Config) -> unim::config::HangulLayout {
    config.engine.hangul.layout
}

/// 현재 영문 레이아웃을 반환합니다.
#[no_mangle]
pub extern "C" fn unim_config_get_latin_layout(config: &Config) -> unim::config::LatinLayout {
    config.engine.latin.layout
}

/// 자동 전환 활성화 여부를 반환합니다.
#[no_mangle]
pub extern "C" fn unim_config_get_auto_switch_enabled(config: &Config) -> bool {
    config.engine.auto_switch.enabled
}

/// 자동 전환 활성화 여부를 설정합니다.
#[no_mangle]
pub extern "C" fn unim_config_set_auto_switch_enabled(config: &mut Config, enabled: bool) {
    config.engine.auto_switch.enabled = enabled;
}

/// 자동 전환 임계값을 반환합니다.
#[no_mangle]
pub extern "C" fn unim_config_get_auto_switch_threshold(config: &Config) -> f32 {
    config.engine.auto_switch.threshold
}

/// 자동 전환 임계값을 설정합니다.
#[no_mangle]
pub extern "C" fn unim_config_set_auto_switch_threshold(config: &mut Config, threshold: f32) {
    config.engine.auto_switch.threshold = threshold.clamp(0.0, 1.0);
}

/// 자동 전환 알림 표시 여부를 반환합니다.
#[no_mangle]
pub extern "C" fn unim_config_get_auto_switch_notification(config: &Config) -> bool {
    config.engine.auto_switch.show_notification
}

/// 자동 전환 알림 표시 여부를 설정합니다.
#[no_mangle]
pub extern "C" fn unim_config_set_auto_switch_notification(config: &mut Config, show: bool) {
    config.engine.auto_switch.show_notification = show;
}

// ============================================
// 레이아웃 열거형 헬퍼 (UI 표시용)
// ============================================

/// 지원하는 한글 레이아웃 개수를 반환합니다.
#[no_mangle]
pub extern "C" fn unim_hangul_layout_count() -> usize {
    3 // Dubeolsik, Sebeolsik390, Sebeolsik391
}

/// 한글 레이아웃 이름을 반환합니다.
///
/// # Arguments
///
/// * `layout` - 레이아웃 열거형 값
///
/// # 반환값
///
/// 레이아웃 이름 문자열 (lifetime: static)
#[no_mangle]
pub extern "C" fn unim_hangul_layout_name(layout: unim::config::HangulLayout) -> UnimStr {
    UnimStr::new(layout.name())
}

/// 한글 레이아웃 표시 이름을 반환합니다 (UI용).
#[no_mangle]
pub extern "C" fn unim_hangul_layout_display_name(layout: unim::config::HangulLayout) -> UnimStr {
    let name = match layout {
        unim::config::HangulLayout::Dubeolsik => "두벌식 표준",
        unim::config::HangulLayout::Sebeolsik390 => "세벌식 390",
        unim::config::HangulLayout::Sebeolsik391 => "세벌식 최종",
    };
    UnimStr::new(name)
}

/// 지원하는 영문 레이아웃 개수를 반환합니다.
#[no_mangle]
pub extern "C" fn unim_latin_layout_count() -> usize {
    2 // Qwerty, Dvorak
}

/// 영문 레이아웃 이름을 반환합니다.
#[no_mangle]
pub extern "C" fn unim_latin_layout_name(layout: unim::config::LatinLayout) -> UnimStr {
    UnimStr::new(layout.name())
}

/// 영문 레이아웃 표시 이름을 반환합니다 (UI용).
#[no_mangle]
pub extern "C" fn unim_latin_layout_display_name(layout: unim::config::LatinLayout) -> UnimStr {
    let name = match layout {
        unim::config::LatinLayout::Qwerty => "QWERTY",
        unim::config::LatinLayout::Dvorak => "Dvorak",
    };
    UnimStr::new(name)
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
