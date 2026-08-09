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

// ─────────────────────────────────────────────────────────────
// M-08 / GAP-config-03: 저장 직전 재로드 + UI-소유 필드만 병합
// ─────────────────────────────────────────────────────────────

thread_local! {
    /// M-08/GAP-config-03(검증 보완): 다이얼로그 시작 시 로드된 config 의 세션
    /// baseline(세션 동안 불변, `set_gtk_config_baseline` 로 1회 고정). GTK 다이얼로그가
    /// 단일 프로세스·단일 GTK 메인루프에서만 쓰이므로 `Mutex` 대신 `thread_local`
    /// (`unim-settings`(Slint)의 `CONFIG_BASELINE` 과 동일 패턴).
    static GTK_CONFIG_BASELINE: std::cell::RefCell<Option<Config>> = const { std::cell::RefCell::new(None) };
}

/// GTK 레거시 설정창(`unim-settings-gtk`)이 다이얼로그를 열 때 1회 호출해 세션
/// baseline 을 고정한다. 이후 `merge_gtk_ui_owned` 가 "이 세션에서 사용자가 실제로
/// 이 필드를 건드렸는가" 를 이 baseline 대비로 판별한다. 저장 성공 직후에도 다시
/// 호출해 baseline 을 "마지막으로 저장한 값" 으로 갱신해야 한다(그러지 않으면 같은
/// 값으로 되돌아간 뒤 재저장할 때 `merge_field` 가 "미변경" 으로 오판해 저장이
/// 무시되는 M-07 재발 패턴을 그대로 반복한다 — `unim-settings/src/main.rs` 참고).
pub fn set_gtk_config_baseline(cfg: &Config) {
    GTK_CONFIG_BASELINE.with(|b| *b.borrow_mut() = Some(cfg.clone()));
}

/// `Debug` 포맷 비교로 값 동치를 판정한다(`AutoTypeFixConfig` 등 다수 필드 타입이
/// `PartialEq` 를 구현하지 않아 — 코어 `src/config.rs` 소유라 이 크레이트에서 손댈 수
/// 없음 — 근사 동치로 충분한 "세션 중 안 바뀜" 판정에만 쓴다).
fn debug_eq<T: std::fmt::Debug>(a: &T, b: &T) -> bool {
    format!("{a:?}") == format!("{b:?}")
}

/// baseline 대비 세션 중 미변경이면 disk(외부 변경, 예: 데몬/CLI/Slint) 값을 보존하고,
/// 변경됐으면 ui(이 세션에서 사용자가 실제로 바꾼 값)를 확정한다. baseline 미확보
/// (`None` — `set_gtk_config_baseline` 미호출, 예: 테스트)면 종전과 동일하게 무조건
/// ui 를 채택한다(회귀 0).
fn merge_field<T: Clone + std::fmt::Debug>(dst: &mut T, baseline: Option<&T>, ui: &T) {
    match baseline {
        Some(b) if debug_eq(b, ui) => {} // 세션 중 미변경 — disk(외부 변경) 보존.
        _ => *dst = ui.clone(),
    }
}

/// GTK 레거시 설정창(`unim-settings-gtk`)이 저장 직전 디스크를 재로드해
/// UI-소유 필드만 in-memory 값으로 덮어쓰는 헬퍼.
///
/// 다이얼로그가 열려 있는 동안 데몬/CLI/Slint 설정앱이 `config.yaml`을
/// 바꿨을 수 있다. GTK 다이얼로그는 창을 연 시점의 스냅샷을 계속 들고
/// 있다가 슬라이더 하나만 바꿔도 그 스냅샷 전체를 그대로 저장해 왔는데,
/// 이 함수는 저장 직전 디스크를 다시 읽어 GTK가 실제로 편집하는 필드만
/// in-memory(`ui`) 값으로 덮어쓰고 나머지는 디스크 값을 보존한다.
///
/// `unim-settings`(Slint)의 `merge_ui_owned`와 동일한 의도이나, GTK
/// 레거시 다이얼로그는 `app_rules`/`korean.word_mode_apps`를 편집하지
/// 않으므로 그 두 필드는 건드리지 않는다(건드리면 다이얼로그가 열려
/// 있는 동안의 외부 변경을 되돌리는 정반대 회귀가 된다).
///
/// GTK-소유 필드: `engine.{default_category, mode_sharing, toggle_keys,
/// hanja_keys, toggle_announce_beep, ignore_key_repeat, auto_typefix,
/// auto_english}`, `engine.korean.{layout, active_rule_sets,
/// layout_rule_sets, bidirectional_combine, chord_window_ms, commit_unit}`,
/// `engine.english.layout`.
///
/// M-08/GAP-config-03(검증 보완): 위 목록은 종전엔 "GTK-소유 필드는 disk 값을
/// 무조건 덮어쓴다" 는 뜻이었다 — 그래서 TSF/Slint 가 이미 막은 시나리오(다이얼로그가
/// 열려 있는 동안 ATF 토글 핫키가 `auto_typefix.enabled` 를 바꾸면, 창에서 무관한
/// 항목 하나만 저장해도 그 값이 되돌아감)가 이 GTK 경로에서만 그대로 남아 있었다.
/// 이제 각 필드는 세션 baseline(`set_gtk_config_baseline`) 대비 "이 세션에서
/// 사용자가 실제로 건드렸는가" 로 한 번 더 걸러, 안 건드린 필드는 disk(외부 변경)
/// 값을 보존한다. baseline 부재(테스트 등)는 종전 동작(ui 우선)과 바이트 동일하다.
///
/// 새 함수 추가로만 확장 — 기존 pub API는 변경하지 않는다(`unim-indicator`,
/// `unim-popup-service` 등 다른 소비 크레이트에 영향 없음).
pub fn merge_gtk_ui_owned(disk: &mut Config, ui: &Config) {
    let baseline = GTK_CONFIG_BASELINE.with(|b| b.borrow().clone());
    let be = baseline.as_ref().map(|c| &c.engine);
    let bk = be.map(|e| &e.korean);

    let d = &mut disk.engine;
    let u = &ui.engine;
    merge_field(&mut d.default_category, be.map(|e| &e.default_category), &u.default_category);
    merge_field(&mut d.mode_sharing, be.map(|e| &e.mode_sharing), &u.mode_sharing);
    merge_field(&mut d.toggle_keys, be.map(|e| &e.toggle_keys), &u.toggle_keys);
    merge_field(&mut d.hanja_keys, be.map(|e| &e.hanja_keys), &u.hanja_keys);
    merge_field(
        &mut d.toggle_announce_beep,
        be.map(|e| &e.toggle_announce_beep),
        &u.toggle_announce_beep,
    );
    merge_field(&mut d.ignore_key_repeat, be.map(|e| &e.ignore_key_repeat), &u.ignore_key_repeat);
    merge_field(&mut d.auto_typefix, be.map(|e| &e.auto_typefix), &u.auto_typefix);
    merge_field(&mut d.auto_english, be.map(|e| &e.auto_english), &u.auto_english);
    merge_field(&mut d.korean.layout, bk.map(|k| &k.layout), &u.korean.layout);
    merge_field(
        &mut d.korean.active_rule_sets,
        bk.map(|k| &k.active_rule_sets),
        &u.korean.active_rule_sets,
    );
    merge_field(
        &mut d.korean.layout_rule_sets,
        bk.map(|k| &k.layout_rule_sets),
        &u.korean.layout_rule_sets,
    );
    merge_field(
        &mut d.korean.bidirectional_combine,
        bk.map(|k| &k.bidirectional_combine),
        &u.korean.bidirectional_combine,
    );
    merge_field(&mut d.korean.chord_window_ms, bk.map(|k| &k.chord_window_ms), &u.korean.chord_window_ms);
    merge_field(&mut d.korean.commit_unit, bk.map(|k| &k.commit_unit), &u.korean.commit_unit);
    merge_field(&mut d.english.layout, be.map(|e| &e.english.layout), &u.english.layout);
}
