//! UNIM GUI 공통 모듈
//!
//! GTK나 Qt 같은 툴킷에 의존하지 않는 순수 Rust 구현체 모음입니다.
//! DBus 통신, 시스템 트레이(ksni), 공통 타입, 상태 관리 등을 제공합니다.

// 트레이 메뉴 등 공통 GUI 문자열에 대한 i18n.
// `locales/{ko,en}.yml` 파일이 컴파일 시점에 임베드된다.
rust_i18n::i18n!("locales", fallback = "en");

pub mod dbus_client;
pub mod tray;
pub mod types;

/// LANG/LC_ALL 환경 변수 기반으로 로케일을 결정해 `rust_i18n::set_locale`을 호출한다.
///
/// - `ko*` 로 시작하면 `"ko"`, 그 외에는 `"en"` (fallback과 동일).
/// - 호출 측은 GUI 진입점에서 한 번만 호출하면 된다.
pub fn init_locale() {
    let lang = std::env::var("LANG")
        .or_else(|_| std::env::var("LC_ALL"))
        .unwrap_or_default();
    let locale = if lang.to_ascii_lowercase().starts_with("ko") {
        "ko"
    } else {
        "en"
    };
    rust_i18n::set_locale(locale);
}
