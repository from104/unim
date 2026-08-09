//! UNIM 데몬의 현재 `korean.layout` 을 읽어 LayoutProfile 로 해석.
//!
//! 우선순위:
//! 1. 데몬 DBus `GetConfig("korean.layout")` 메서드 (blocking, 일반 Proxy::call).
//! 2. 폴백: `~/.config/unim/config.yaml` 의 `korean.layout` 직접 파싱
//!    (`unim::config::Config::load_from_default_path`).
//! 3. 최종 폴백: `ko_2bulstd` (두벌식 표준).
//!
//! 그 다음 `ProfileRegistry::find_raw` 로 LayoutProfile 인스턴스를 얻는다 —
//! 사용자 정의 layout 도 그대로 지원.

use unim::config::Config;
use unim::keystroke::profile::{LayoutProfile, ProfileRegistry};
use unim_dbus::{BUS_NAME, INPUT_METHOD_PATH};

const FALLBACK_LAYOUT: &str = "ko_2bulstd";
const FALLBACK_EN_LAYOUT: &str = "en_qwerty";

/// 데몬 DBus 로 임의 config 키 값을 읽는다 (blocking). 실패하면 None.
fn read_config_via_dbus(key: &str) -> Option<String> {
    let conn = zbus::blocking::Connection::session().ok()?;
    let proxy = zbus::blocking::Proxy::new(
        &conn,
        BUS_NAME,
        INPUT_METHOD_PATH,
        "org.atit.unim.InputMethod",
    )
    .ok()?;
    let val: String = proxy.call("GetConfig", &(key,)).ok()?;
    let trimmed = val.trim().trim_matches('"').to_string();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed)
    }
}

fn read_korean_layout_via_dbus() -> Option<String> {
    read_config_via_dbus("korean.layout")
}

fn read_english_layout_via_dbus() -> Option<String> {
    read_config_via_dbus("english.layout")
}

/// 현재 활성 영문 자판 코드 (예: "en_qwerty"). DBus → config → fallback 순.
pub fn read_english_layout_name() -> String {
    if let Some(name) = read_english_layout_via_dbus() {
        return name;
    }
    let cfg = Config::load_from_default_path();
    let name = cfg.engine.english.layout.trim().to_string();
    if !name.is_empty() {
        return name;
    }
    FALLBACK_EN_LAYOUT.to_string()
}

/// 활성 영문 자판 코드 + LayoutProfile. 빌트인에 없으면 (name, None) — 호출자가 폴백.
pub fn load_active_english_profile() -> (String, Option<LayoutProfile>) {
    let mut registry = ProfileRegistry::new();
    let _ = registry.scan();
    let name = read_english_layout_name();
    let profile = registry
        .find_raw(&name)
        .or_else(|| registry.find_raw("en_qwerty"));
    (name, profile)
}

/// 영문 자판 코드를 사용자에게 보일 짧은 라벨로 변환 (en_qwerty → "QWERTY").
pub fn english_layout_label(code: &str) -> String {
    match code {
        "en_qwerty" => "QWERTY".to_string(),
        "en_dvorak" => "Dvorak".to_string(),
        "en_workman" => "Workman".to_string(),
        "en_colemak" => "Colemak".to_string(),
        other => other.to_string(),
    }
}

/// 데몬의 현재 korean.layout 이름을 얻는다 (정규화 전 raw).
pub fn read_korean_layout_name() -> String {
    if let Some(name) = read_korean_layout_via_dbus() {
        return name;
    }
    let cfg = Config::load_from_default_path();
    let name = cfg.engine.korean.layout.trim().to_string();
    if !name.is_empty() {
        return name;
    }
    FALLBACK_LAYOUT.to_string()
}

/// 데몬의 활성 자판을 LayoutProfile 로 해석.
///
/// 반환: (layout_name, profile).
/// 어떤 이유로든 빌트인/사용자 어디에서도 못 찾으면 `ko_2bulstd` 로 폴백.
pub fn load_active_profile() -> (String, LayoutProfile) {
    let mut registry = ProfileRegistry::new();
    let _ = registry.scan();

    let name = read_korean_layout_name();
    if let Some(p) = registry.find_raw(&name) {
        return (name, p);
    }
    let normalized = unim::config::normalize_korean_layout_name(&name);
    if normalized != name {
        if let Some(p) = registry.find_raw(&normalized) {
            return (normalized, p);
        }
    }
    let p = registry
        .find_raw(FALLBACK_LAYOUT)
        .expect("ko_2bulstd builtin profile must exist");
    (FALLBACK_LAYOUT.to_string(), p)
}
