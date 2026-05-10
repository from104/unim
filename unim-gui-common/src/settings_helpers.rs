//! toolkit-free 설정 헬퍼
//!
//! 자판 프로필 열거, Config 적용, 환경 감지 등 GTK 무관 순수 함수들.
//! GTK settings_dialog 와 미래 Qt settings 다이얼로그 양쪽에서 재사용.

use unim::config::Config;
use unim::keystroke::profile::{resolve_inherits, LayoutProfile, ProfileRegistry};
use unim::typefix_blacklist::Direction;
use rust_i18n::t;

// ─────────────────────────────────────────────────────────────
// 자판 프로필
// ─────────────────────────────────────────────────────────────

/// 한국어 프로필 선택지 — `(name, display_string)` 목록.
///
/// 레지스트리에서 `language == "korean"` 인 프로필만 수집한다.
pub fn collect_korean_profile_choices() -> Vec<(String, String)> {
    let reg = ProfileRegistry::new();
    let mut out: Vec<(String, String)> = Vec::new();
    for name in reg.list_names() {
        if let Some(p) = reg.find_raw(&name) {
            if p.language != "korean" {
                continue;
            }
            let disp = p
                .metadata
                .display_name
                .as_ref()
                .map(|d| d.resolve("ko").to_string())
                .unwrap_or_else(|| name.clone());
            out.push((name, disp));
        }
    }
    out
}

/// 선택된 프로필 이름을 Config에 반영한다.
///
/// `KoreanConfig::switch_layout`이 이전 자판의 `active_rule_sets`(Some이면)를
/// `layout_rule_sets`에 보존하고, 새 자판의 캐시된 값(있으면)을 복원한다.
/// 새 프로필에 정의되지 않은 stale 이름은 valid 슬라이스로 자동 정리.
pub fn apply_korean_profile_choice(config: &mut Config, name: &str, new_profile: &LayoutProfile) {
    let valid_names: Vec<String> = new_profile.rule_sets.keys().cloned().collect();
    config
        .engine
        .korean
        .switch_layout(name, Some(&valid_names));
}

/// 레지스트리에서 프로필을 찾아 inherits까지 해석한다. 실패 시 `None`.
pub fn load_and_resolve(name: &str) -> Option<LayoutProfile> {
    let reg = ProfileRegistry::new();
    let raw = reg.find_raw(name)?;
    resolve_inherits(&raw, &reg).ok()
}

// ─────────────────────────────────────────────────────────────
// 시간 단위 변환
// ─────────────────────────────────────────────────────────────

/// 밀리초 → 초 (슬라이더 표시용)
pub fn ms_to_seconds(ms: u32) -> f64 {
    ms as f64 / 1000.0
}

/// 초 → 밀리초 (Config 저장용)
pub fn seconds_to_ms(secs: f64) -> u32 {
    (secs * 1000.0).round() as u32
}

// ─────────────────────────────────────────────────────────────
// 환경 감지
// ─────────────────────────────────────────────────────────────

/// GNOME 세션 여부 (XDG_CURRENT_DESKTOP).
pub fn is_gnome_session() -> bool {
    std::env::var("XDG_CURRENT_DESKTOP")
        .map(|s| s.to_uppercase().contains("GNOME"))
        .unwrap_or(false)
}

/// Wayland 세션 여부 (XDG_SESSION_TYPE).
pub fn is_wayland_session() -> bool {
    std::env::var("XDG_SESSION_TYPE")
        .map(|s| s.to_lowercase() == "wayland")
        .unwrap_or(false)
}

// ─────────────────────────────────────────────────────────────
// 오타 교정 방향 레이블
// ─────────────────────────────────────────────────────────────

/// `Direction` → 사람이 읽기 좋은 레이블 (i18n).
pub fn direction_label(dir: Direction) -> String {
    match dir {
        Direction::Forward => t!("blacklist_kind_forward").into_owned(),
        Direction::Reverse => t!("blacklist_kind_reverse").into_owned(),
    }
}
