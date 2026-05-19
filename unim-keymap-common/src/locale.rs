//! LANG/LC_ALL 기반 로케일 감지 — 두 바이너리에서 한 줄로 호출.
//!
//! `unim-gui-common::init_locale()`과 동일한 규칙이지만, 본 crate의 `i18n!`
//! 매크로가 만든 별도 backend에도 같은 로케일을 적용해야 한다. 두 매크로는
//! 각각 자기 static을 쓰므로 한쪽만 설정하면 다른 쪽이 영어로 남는다.

/// `LANG` → `LC_ALL` 순으로 검사, `ko*`이면 `"ko"`, 그 외 `"en"`.
pub fn detect_locale() -> &'static str {
    let lang = std::env::var("LANG")
        .or_else(|_| std::env::var("LC_ALL"))
        .unwrap_or_default();
    if lang.to_ascii_lowercase().starts_with("ko") {
        "ko"
    } else {
        "en"
    }
}

/// common crate의 `i18n!` backend에 로케일을 적용.
///
/// 호출 측 바이너리는 자기 `i18n!` backend에도 동일하게 `set_locale`을
/// 호출해야 한다 — 매크로마다 static이 분리되어 있기 때문.
pub fn init_locale() {
    rust_i18n::set_locale(detect_locale());
}
