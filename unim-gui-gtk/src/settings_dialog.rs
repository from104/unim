//! UNIM 설정 다이얼로그 (Phase F — 전면 재설계)
//!
//! libadwaita 0.7 기반, Adw.PreferencesWindow + PreferencesPage/Group/Row로 구성.
//! 3페이지: 일반 / 오타 교정 / GNOME Shell (GNOME 세션에서만).
//! 시스템 테마 자동 추종 (ColorScheme::Default), 최소주의.
//! 변경 시: 파일 저장(`Config::save_to_default_path`) + DBus `SetConfigYaml`
//! fire-and-forget 호출 + `Adw.Toast` 피드백.

use gtk4::gio;
use gtk4::glib;
use gtk4::prelude::*;
use libadwaita as adw;
use libadwaita::prelude::*;

use unim::config::{
    Config, EnglishLayout, InputCategory, ModeSharingMode, PopupMode,
    AUTO_TYPEFIX_ENG_MIN_LENGTH_MAX, AUTO_TYPEFIX_ENG_MIN_LENGTH_MIN,
    AUTO_TYPEFIX_KOR_THRESHOLD_MAX, AUTO_TYPEFIX_KOR_THRESHOLD_MIN,
    AUTO_TYPEFIX_OBSERVATION_TIMEOUT_MAX, AUTO_TYPEFIX_OBSERVATION_TIMEOUT_MIN,
    AUTO_TYPEFIX_TENTATIVE_EXPIRY_MAX, AUTO_TYPEFIX_TENTATIVE_EXPIRY_MIN,
    AUTO_TYPEFIX_TIME_WINDOW_MAX, AUTO_TYPEFIX_TIME_WINDOW_MIN,
};
use unim::keystroke::profile::{resolve_inherits, LayoutProfile, ProfileRegistry};
use unim::typefix_blacklist::{Blacklist, Direction, EntryStatus};
use unim::unim_log;

use std::cell::RefCell;
use std::rc::Rc;

// ─────────────────────────────────────────────────────────────
// 공용 상수
// ─────────────────────────────────────────────────────────────

const GSCHEMA_ID: &str = "org.gnome.shell.extensions.unim";
const TOAST_TIMEOUT_SECS: u32 = 2;
const WINDOW_MIN_WIDTH: i32 = 520;
const WINDOW_MIN_HEIGHT: i32 = 640;

// ─────────────────────────────────────────────────────────────
// 상태
// ─────────────────────────────────────────────────────────────

/// 다이얼로그 수명 동안 유지되는 뮤터블 상태.
///
/// `updating` 플래그는 초기값을 위젯에 주입할 때 콜백이 재발사되는
/// 것을 막는다. 모든 `connect_*_notify` 콜백은 반드시 이 플래그를
/// 먼저 확인해야 한다.
struct SettingsState {
    config: Config,
    updating: bool,
}

type State = Rc<RefCell<SettingsState>>;

// ─────────────────────────────────────────────────────────────
// 진입점
// ─────────────────────────────────────────────────────────────

/// 설정 다이얼로그를 생성하고 표시한다.
///
/// 공용 시그니처(`app: &adw::Application`)는 유지 — `gtk_ui.rs`/`main.rs` 호출부 변경 없음.
pub fn show_settings_dialog(app: &adw::Application) {
    // 시스템 테마 자동 추종 — 다이얼로그 단위에만 영향
    adw::StyleManager::default().set_color_scheme(adw::ColorScheme::Default);

    let config = Config::load_from_default_path();
    let state: State = Rc::new(RefCell::new(SettingsState {
        config,
        updating: true, // 초기 바인딩 동안은 콜백 억제
    }));

    let window = adw::PreferencesWindow::builder()
        .application(app)
        .title("UNIM 설정")
        .default_width(WINDOW_MIN_WIDTH)
        .default_height(WINDOW_MIN_HEIGHT)
        .search_enabled(false)
        .build();

    // ── Page 1: 일반 ──────────────────────────────────────────
    let page_general = adw::PreferencesPage::builder()
        .title("일반")
        .icon_name("preferences-system-symbolic")
        .build();
    let rule_sets_handle = build_rule_sets_group();
    page_general.add(&build_keymap_group(&state, &rule_sets_handle));
    page_general.add(&rule_sets_handle.group);
    page_general.add(&build_input_mode_group(&state));
    window.add(&page_general);

    // ── Page 2: 오타 교정 ─────────────────────────────────────
    let page_typefix = adw::PreferencesPage::builder()
        .title("오타 교정")
        .icon_name("edit-find-replace-symbolic")
        .build();

    let forward_group = build_forward_group(&state);
    let reverse_group = build_reverse_group(&state);
    let master_group = build_master_group(&state);

    // master_group(활성화 + 재트리거 감지/관찰 창/만료)은 공통 설정이므로 최상단에 배치
    page_typefix.add(&master_group);
    page_typefix.add(&forward_group);
    page_typefix.add(&reverse_group);
    window.add(&page_typefix);

    // ── Page 3: 교정 억제 단어 ────────────────────────────────
    let page_blacklist = build_blacklist_page();
    window.add(&page_blacklist);

    // ── Page 4: GNOME Shell (GNOME 세션 전용) ────────────────
    if is_gnome_session() {
        if let Some(page_gnome) = build_gnome_page(&window) {
            window.add(&page_gnome);
        }
    }

    // 전역 토스트 싱크 등록 — save_and_notify에서 사용
    attach_toast_sink(&window);

    // 초기 바인딩 완료 — 이제 콜백이 실제 저장/DBus 호출을 수행
    state.borrow_mut().updating = false;

    unim_log!("INDICATOR", "[Settings] PreferencesWindow presented");
    window.present();
}

// ─────────────────────────────────────────────────────────────
// Toast / Save 공용
// ─────────────────────────────────────────────────────────────

thread_local! {
    /// 현재 활성 다이얼로그 창 — toast 표시용
    static ACTIVE_WINDOW: RefCell<Option<adw::PreferencesWindow>> = const { RefCell::new(None) };
}

fn attach_toast_sink(window: &adw::PreferencesWindow) {
    ACTIVE_WINDOW.with(|w| {
        *w.borrow_mut() = Some(window.clone());
    });
    window.connect_close_request(move |_| {
        ACTIVE_WINDOW.with(|w| {
            *w.borrow_mut() = None;
        });
        glib::Propagation::Proceed
    });
}

/// 저장 + DBus 전파 + 토스트 알림.
///
/// - 파일: `Config::save_to_default_path()`
/// - DBus: `SetConfigYaml` (fire-and-forget, 실패해도 UI는 막히지 않음)
/// - 토스트: "저장됨 ✓" (2초 자동 소멸)
fn save_and_notify(config: &Config, label: &str) {
    // 1. 파일 저장
    if let Err(e) = config.save_to_default_path() {
        unim_log!(
            "INDICATOR",
            "[Settings] config 저장 실패 ({}): {}",
            label,
            e
        );
        show_toast(&format!("저장 실패: {}", e));
        return;
    }

    // 2. DBus 전파 (fire-and-forget)
    match serde_yaml::to_string(config) {
        Ok(yaml) => spawn_set_config_yaml(yaml, label.to_string()),
        Err(e) => unim_log!("INDICATOR", "[Settings] YAML 직렬화 실패: {}", e),
    }

    // 3. 토스트
    show_toast("저장됨 ✓");
}

fn show_toast(text: &str) {
    ACTIVE_WINDOW.with(|w| {
        if let Some(window) = w.borrow().as_ref() {
            let toast = adw::Toast::builder()
                .title(text)
                .timeout(TOAST_TIMEOUT_SECS)
                .build();
            window.add_toast(toast);
        }
    });
}

/// DBus SetConfigYaml fire-and-forget.
///
/// 새로운 tokio current-thread 런타임을 임시로 생성하여 호출.
/// 메인 GTK 스레드를 차단하지 않기 위해 별도 OS 스레드에 위임.
fn spawn_set_config_yaml(yaml: String, label: String) {
    std::thread::spawn(move || {
        let rt = match tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
        {
            Ok(rt) => rt,
            Err(e) => {
                unim_log!("INDICATOR", "[Settings] tokio 런타임 생성 실패: {}", e);
                return;
            }
        };
        rt.block_on(async move {
            match send_set_config_yaml(&yaml).await {
                Ok(()) => unim_log!(
                    "INDICATOR",
                    "[Settings] DBus SetConfigYaml 성공 ({})",
                    label
                ),
                Err(e) => unim_log!(
                    "INDICATOR",
                    "[Settings] DBus SetConfigYaml 실패 ({}): {}",
                    label,
                    e
                ),
            }
        });
    });
}

async fn send_set_config_yaml(yaml: &str) -> zbus::Result<()> {
    use unim_dbus::client::InputMethodProxy;
    let conn = zbus::Connection::session().await?;
    let proxy = InputMethodProxy::new(&conn).await?;
    proxy.set_config_yaml(yaml).await
}

// ─────────────────────────────────────────────────────────────
// Page 1: 일반
// ─────────────────────────────────────────────────────────────

/// 한국어 프로필 선택지 — (name, display_string).
fn collect_korean_profile_choices() -> Vec<(String, String)> {
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
/// Phase 8: `korean.layout`이 enum → String으로 통합되어 별칭 분기 불필요.
/// 레거시 값은 `normalize_korean_layout_name`이 정식 이름으로 승격.
///
/// 새 프로필에 정의되지 않은 `active_rule_sets` 이름은 silently drop (§3.5).
fn apply_korean_profile_choice(config: &mut Config, name: &str, new_profile: &LayoutProfile) {
    config.engine.korean.layout = unim::config::normalize_korean_layout_name(name);
    // 새 프로필에 없는 rule_set 이름 드롭.
    config
        .engine
        .korean
        .active_rule_sets
        .retain(|n| new_profile.rule_sets.contains_key(n));
}

/// 레지스트리에서 프로필을 찾아 inherits까지 해석한다. 실패 시 `None`.
fn load_and_resolve(name: &str) -> Option<LayoutProfile> {
    let reg = ProfileRegistry::new();
    let raw = reg.find_raw(name)?;
    resolve_inherits(&raw, &reg).ok()
}

/// 동적 규칙 세트 그룹 핸들 — 자판 변경 시 SwitchRow 재구성.
#[derive(Clone)]
struct RuleSetsHandle {
    group: adw::PreferencesGroup,
    rows: Rc<RefCell<Vec<adw::SwitchRow>>>,
}

impl RuleSetsHandle {
    fn refresh(&self, profile: &LayoutProfile, state: &State) {
        // 기존 행 제거.
        for row in self.rows.borrow().iter() {
            self.group.remove(row);
        }
        self.rows.borrow_mut().clear();

        let config_active = state.borrow().config.engine.korean.active_rule_sets.clone();
        // 프로필이 선언한 active_rule_sets (있으면 그 집합이 기본 활성).
        let profile_default: Option<&Vec<String>> = profile.active_rule_sets.as_ref();

        if profile.rule_sets.is_empty() {
            self.group.set_description(Some("이 자판은 옵션 규칙이 없습니다."));
        } else {
            self.group.set_description(Some("선택된 자판의 옵션 규칙"));
        }

        for (set_name, rs) in &profile.rule_sets {
            let title = rs
                .description
                .as_ref()
                .map(|d| d.resolve("ko").to_string())
                .unwrap_or_else(|| set_name.clone());
            let row = adw::SwitchRow::builder()
                .title(set_name)
                .subtitle(&title)
                .build();

            // active 계산: config override 비어 있지 않으면 config 값, 아니면
            // 프로필의 active_rule_sets 또는 rule_set.active.
            let is_active = if !config_active.is_empty() {
                config_active.contains(set_name)
            } else if let Some(list) = profile_default {
                list.contains(set_name)
            } else {
                rs.active
            };
            row.set_active(is_active);

            let state_c = state.clone();
            let name_c = set_name.clone();
            row.connect_active_notify(move |r| {
                let mut s = state_c.borrow_mut();
                if s.updating {
                    return;
                }
                let on = r.is_active();
                let list = &mut s.config.engine.korean.active_rule_sets;
                if on {
                    if !list.contains(&name_c) {
                        list.push(name_c.clone());
                    }
                } else {
                    list.retain(|x| x != &name_c);
                }
                save_and_notify(&s.config, "korean_active_rule_sets");
            });

            self.group.add(&row);
            self.rows.borrow_mut().push(row);
        }
    }
}

fn build_rule_sets_group() -> RuleSetsHandle {
    let group = adw::PreferencesGroup::builder()
        .title("규칙 세트")
        .description("자판 프로필 로드 후 표시")
        .build();
    RuleSetsHandle {
        group,
        rows: Rc::new(RefCell::new(Vec::new())),
    }
}

fn build_keymap_group(state: &State, rule_sets: &RuleSetsHandle) -> adw::PreferencesGroup {
    let group = adw::PreferencesGroup::builder().title("자판 및 키맵").build();

    // 한국어 자판 — 내장 + 사용자 프로필 통합
    let kor_row = adw::ComboRow::builder()
        .title("한국어 자판")
        .subtitle("내장 + ~/.config/unim/layouts/*.json")
        .build();
    let choices = collect_korean_profile_choices();
    let display_items: Vec<&str> = choices.iter().map(|(_, d)| d.as_str()).collect();
    let kor_list = gtk4::StringList::new(&display_items);
    kor_row.set_model(Some(&kor_list));

    // 현재 선택 위치 결정 — effective_layout_name() 기반
    let current_name = state.borrow().config.engine.korean.effective_layout_name();
    let current_idx = choices
        .iter()
        .position(|(n, _)| *n == current_name)
        .or_else(|| {
            // 별칭 검색: "2bul" → "ko_2bulstd" 등
            choices.iter().position(|(n, _)| {
                matches!(
                    (current_name.as_str(), n.as_str()),
                    ("2bul" | "2bulstd", "ko_2bulstd")
                        | ("3bul390" | "390", "ko_3bul390")
                        | ("3bul391" | "391", "ko_3bul391")
                        | ("3bul_noshift" | "noshift", "ko_3bul_noshift")
                        | ("3bul_qwerty", "ko_3bul_qwerty")
                )
            })
        })
        .unwrap_or(0);
    kor_row.set_selected(current_idx as u32);

    // 초기 rule_sets 채우기
    if let Some((init_name, _)) = choices.get(current_idx) {
        if let Some(profile) = load_and_resolve(init_name) {
            rule_sets.refresh(&profile, state);
        }
    }

    // 선택 변경 콜백
    {
        let state_c = state.clone();
        let rule_sets_c = rule_sets.clone();
        let choices_c = choices.clone();
        kor_row.connect_selected_notify(move |row| {
            let idx = row.selected() as usize;
            let Some((name, _)) = choices_c.get(idx) else {
                return;
            };
            let Some(profile) = load_and_resolve(name) else {
                unim_log!("INDICATOR", "[Settings] 프로필 로드 실패: {name}");
                return;
            };
            {
                let mut s = state_c.borrow_mut();
                if s.updating {
                    return;
                }
                apply_korean_profile_choice(&mut s.config, name, &profile);
                save_and_notify(&s.config, "korean_layout");
            }
            // rule_sets 그룹 재구성 — updating 플래그 내부 flip으로 콜백 폭주 방지
            state_c.borrow_mut().updating = true;
            rule_sets_c.refresh(&profile, &state_c);
            state_c.borrow_mut().updating = false;
        });
    }
    group.add(&kor_row);

    // 영어 자판
    let eng_row = adw::ComboRow::builder().title("영어 자판").build();
    let eng_items: Vec<&str> = EnglishLayout::all()
        .iter()
        .map(|l| l.display_name())
        .collect();
    let eng_list = gtk4::StringList::new(&eng_items);
    eng_row.set_model(Some(&eng_list));
    {
        let s = state.borrow();
        if let Some(idx) = EnglishLayout::all()
            .iter()
            .position(|l| *l == s.config.engine.english.layout)
        {
            eng_row.set_selected(idx as u32);
        }
    }
    {
        let state_c = state.clone();
        eng_row.connect_selected_notify(move |row| {
            let mut s = state_c.borrow_mut();
            if s.updating {
                return;
            }
            if let Some(layout) = EnglishLayout::all().get(row.selected() as usize) {
                s.config.engine.english.layout = *layout;
                save_and_notify(&s.config, "english_layout");
            }
        });
    }
    group.add(&eng_row);

    // 한/영 전환 키
    group.add(&build_string_list_row(
        state,
        "한/영 전환 키",
        Some("쉼표로 구분 (예: Korean, RightAlt)"),
        |cfg| cfg.engine.toggle_keys.join(", "),
        |cfg, v| cfg.engine.toggle_keys = v,
        "toggle_keys",
    ));

    // 한자 키
    group.add(&build_string_list_row(
        state,
        "한자 키",
        Some("쉼표로 구분 (예: Hanja, F9)"),
        |cfg| cfg.engine.hanja_keys.join(", "),
        |cfg, v| cfg.engine.hanja_keys = v,
        "hanja_keys",
    ));

    group
}

fn build_input_mode_group(state: &State) -> adw::PreferencesGroup {
    let group = adw::PreferencesGroup::builder().title("입력 모드").build();

    // 초기 입력 모드
    let init_row = adw::ComboRow::builder().title("초기 입력 모드").build();
    let init_list = gtk4::StringList::new(&["영문", "한글"]);
    init_row.set_model(Some(&init_list));
    {
        let s = state.borrow();
        init_row.set_selected(match s.config.engine.default_category {
            InputCategory::English => 0,
            InputCategory::Korean => 1,
        });
    }
    {
        let state_c = state.clone();
        init_row.connect_selected_notify(move |row| {
            let mut s = state_c.borrow_mut();
            if s.updating {
                return;
            }
            s.config.engine.default_category = if row.selected() == 1 {
                InputCategory::Korean
            } else {
                InputCategory::English
            };
            save_and_notify(&s.config, "default_category");
        });
    }
    group.add(&init_row);

    // 모드 공유 방식
    let share_row = adw::ComboRow::builder().title("모드 공유 방식").build();
    let share_list = gtk4::StringList::new(&["전역 (Global)", "앱별 (PerApp)"]);
    share_row.set_model(Some(&share_list));
    {
        let s = state.borrow();
        share_row.set_selected(match s.config.engine.mode_sharing {
            ModeSharingMode::Global => 0,
            ModeSharingMode::PerApp => 1,
        });
    }
    {
        let state_c = state.clone();
        share_row.connect_selected_notify(move |row| {
            let mut s = state_c.borrow_mut();
            if s.updating {
                return;
            }
            s.config.engine.mode_sharing = if row.selected() == 1 {
                ModeSharingMode::PerApp
            } else {
                ModeSharingMode::Global
            };
            save_and_notify(&s.config, "mode_sharing");
        });
    }
    group.add(&share_row);

    // 팝업 모드
    let popup_row = adw::ComboRow::builder().title("팝업 모드").build();
    let popup_list = gtk4::StringList::new(&["독립 (Standalone)", "내장 (Embedded)"]);
    popup_row.set_model(Some(&popup_list));
    {
        let s = state.borrow();
        popup_row.set_selected(match s.config.engine.popup_mode {
            PopupMode::Standalone => 0,
            PopupMode::Embedded => 1,
        });
    }
    {
        let state_c = state.clone();
        popup_row.connect_selected_notify(move |row| {
            let mut s = state_c.borrow_mut();
            if s.updating {
                return;
            }
            s.config.engine.popup_mode = if row.selected() == 1 {
                PopupMode::Embedded
            } else {
                PopupMode::Standalone
            };
            save_and_notify(&s.config, "popup_mode");
        });
    }
    group.add(&popup_row);

    group
}

// ─────────────────────────────────────────────────────────────
// Page 2: 오타 교정 (Scale/슬라이더 기반)
// ─────────────────────────────────────────────────────────────

/// 정수 슬라이더 Row. `AdwActionRow` + suffix `gtk::Scale`.
///
/// - 현재 값은 핸들 위에 표시(`draw_value=true`)
/// - 최솟값/최댓값 위치에 tick 마크 + 라벨
/// - step=1, 정수만 허용
fn build_int_scale_row(
    state: &State,
    title: &str,
    subtitle: &str,
    min: i32,
    max: i32,
    init: i32,
    label: &'static str,
    set: impl Fn(&mut Config, i32) + 'static,
) -> adw::ActionRow {
    let row = adw::ActionRow::builder()
        .title(title)
        .subtitle(subtitle)
        .build();

    let adj = gtk4::Adjustment::new(init as f64, min as f64, max as f64, 1.0, 1.0, 0.0);
    let scale = gtk4::Scale::new(gtk4::Orientation::Horizontal, Some(&adj));
    scale.set_width_request(154);
    scale.set_hexpand(false);
    scale.set_valign(gtk4::Align::Center);
    scale.set_draw_value(true);
    scale.set_value_pos(gtk4::PositionType::Top);
    scale.set_digits(0);
    scale.set_round_digits(0);
    scale.add_mark(
        min as f64,
        gtk4::PositionType::Bottom,
        Some(&min.to_string()),
    );
    scale.add_mark(
        max as f64,
        gtk4::PositionType::Bottom,
        Some(&max.to_string()),
    );

    let state_c = state.clone();
    scale.connect_value_changed(move |s| {
        let mut st = state_c.borrow_mut();
        if st.updating {
            return;
        }
        let v = s.value().round() as i32;
        set(&mut st.config, v);
        save_and_notify(&st.config, label);
    });

    row.add_suffix(&scale);
    row
}

/// 초 단위 슬라이더 Row (내부 저장은 u32 ms).
///
/// - 범위는 `AUTO_TYPEFIX_TIME_WINDOW_MIN/MAX`(ms)를 초로 환산
/// - step=0.5초, digits=1
fn build_time_window_row(
    state: &State,
    title: &str,
    subtitle: &str,
    get_ms: impl Fn(&Config) -> u32 + 'static,
    set_ms: impl Fn(&mut Config, u32) + 'static,
    label: &'static str,
) -> adw::ActionRow {
    let row = adw::ActionRow::builder()
        .title(title)
        .subtitle(subtitle)
        .build();

    let min_s = AUTO_TYPEFIX_TIME_WINDOW_MIN as f64 / 1000.0;
    let max_s = AUTO_TYPEFIX_TIME_WINDOW_MAX as f64 / 1000.0;
    let init_s = {
        let s = state.borrow();
        ms_to_seconds(get_ms(&s.config))
    };

    let adj = gtk4::Adjustment::new(init_s, min_s, max_s, 0.5, 0.5, 0.0);
    let scale = gtk4::Scale::new(gtk4::Orientation::Horizontal, Some(&adj));
    scale.set_width_request(154);
    scale.set_hexpand(false);
    scale.set_valign(gtk4::Align::Center);
    scale.set_draw_value(true);
    scale.set_value_pos(gtk4::PositionType::Top);
    scale.set_digits(1);
    scale.set_round_digits(1);
    scale.add_mark(
        min_s,
        gtk4::PositionType::Bottom,
        Some(&format!("{:.1}", min_s)),
    );
    scale.add_mark(
        max_s,
        gtk4::PositionType::Bottom,
        Some(&format!("{:.1}", max_s)),
    );

    // 드래그 중에도 0.5 단위로 스냅
    scale.connect_change_value(|s, _scroll, val| {
        let snapped = (val * 2.0).round() / 2.0;
        s.set_value(snapped);
        gtk4::glib::Propagation::Stop
    });

    let state_c = state.clone();
    scale.connect_value_changed(move |s| {
        let mut st = state_c.borrow_mut();
        if st.updating {
            return;
        }
        // step에 스냅
        let snapped = (s.value() * 2.0).round() / 2.0;
        let ms = seconds_to_ms(snapped);
        set_ms(&mut st.config, ms);
        save_and_notify(&st.config, label);
    });

    row.add_suffix(&scale);
    row
}

fn build_forward_group(state: &State) -> adw::PreferencesGroup {
    let group = adw::PreferencesGroup::builder()
        .title("자동 순방향 교정 (영→한)")
        .build();

    // 사용 (forward)
    let fwd_sw = adw::SwitchRow::builder().title("사용").build();
    {
        let s = state.borrow();
        fwd_sw.set_active(s.config.engine.auto_typefix.forward);
    }
    {
        let state_c = state.clone();
        fwd_sw.connect_active_notify(move |sw| {
            let mut s = state_c.borrow_mut();
            if s.updating {
                return;
            }
            s.config.engine.auto_typefix.forward = sw.is_active();
            save_and_notify(&s.config, "auto_typefix_forward");
        });
    }
    group.add(&fwd_sw);

    // 임계 음절 수
    let init_kor = state.borrow().config.engine.auto_typefix.kor_syllable_threshold as i32;
    let kor_row = build_int_scale_row(
        state,
        "임계 음절 수",
        "이 개수 이상의 완성 한글이 감지되면 교정 검사",
        AUTO_TYPEFIX_KOR_THRESHOLD_MIN as i32,
        AUTO_TYPEFIX_KOR_THRESHOLD_MAX as i32,
        init_kor,
        "auto_typefix_kor_syllable_threshold",
        |cfg, v| cfg.engine.auto_typefix.kor_syllable_threshold = v as u8,
    );
    group.add(&kor_row);

    // 트리거 윈도우 (초) — 순방향 전용
    let fwd_time_row = build_time_window_row(
        state,
        "트리거 윈도우 (초)",
        "순방향 교정이 유효로 간주하는 최근 입력 시간",
        |cfg| cfg.engine.auto_typefix.forward_time_window_ms,
        |cfg, ms| cfg.engine.auto_typefix.forward_time_window_ms = ms,
        "auto_typefix_forward_time_window_ms",
    );
    group.add(&fwd_time_row);

    // 영단어 매칭 시 억제
    let skip_eng_sw = adw::SwitchRow::builder()
        .title("영단어 매칭 시 억제")
        .subtitle("사전에 일치하는 영단어이면 교정하지 않음")
        .build();
    {
        let s = state.borrow();
        skip_eng_sw.set_active(s.config.engine.auto_typefix.skip_on_english_word);
    }
    {
        let state_c = state.clone();
        skip_eng_sw.connect_active_notify(move |sw| {
            let mut s = state_c.borrow_mut();
            if s.updating {
                return;
            }
            s.config.engine.auto_typefix.skip_on_english_word = sw.is_active();
            save_and_notify(&s.config, "auto_typefix_skip_on_english_word");
        });
    }
    group.add(&skip_eng_sw);

    group
}

fn build_reverse_group(state: &State) -> adw::PreferencesGroup {
    let group = adw::PreferencesGroup::builder()
        .title("자동 역방향 교정 (한→영)")
        .build();

    // 사용 (reverse)
    let rev_sw = adw::SwitchRow::builder().title("사용").build();
    {
        let s = state.borrow();
        rev_sw.set_active(s.config.engine.auto_typefix.reverse);
    }
    {
        let state_c = state.clone();
        rev_sw.connect_active_notify(move |sw| {
            let mut s = state_c.borrow_mut();
            if s.updating {
                return;
            }
            s.config.engine.auto_typefix.reverse = sw.is_active();
            save_and_notify(&s.config, "auto_typefix_reverse");
        });
    }
    group.add(&rev_sw);

    // 임계 글자 수
    let init_eng = state.borrow().config.engine.auto_typefix.eng_word_min_length as i32;
    let eng_row = build_int_scale_row(
        state,
        "임계 글자 수",
        "이 개수 이상의 한글이 감지되면 교정 검사",
        AUTO_TYPEFIX_ENG_MIN_LENGTH_MIN as i32,
        AUTO_TYPEFIX_ENG_MIN_LENGTH_MAX as i32,
        init_eng,
        "auto_typefix_eng_word_min_length",
        |cfg, v| cfg.engine.auto_typefix.eng_word_min_length = v as u8,
    );
    group.add(&eng_row);

    // 트리거 윈도우 (초) — 역방향 전용
    let rev_time_row = build_time_window_row(
        state,
        "트리거 윈도우 (초)",
        "역방향 교정이 유효로 간주하는 최근 입력 시간",
        |cfg| cfg.engine.auto_typefix.reverse_time_window_ms,
        |cfg, ms| cfg.engine.auto_typefix.reverse_time_window_ms = ms,
        "auto_typefix_reverse_time_window_ms",
    );
    group.add(&rev_time_row);

    // 온전한 음절 매칭 시 억제
    let skip_syl_sw = adw::SwitchRow::builder()
        .title("온전한 음절 매칭 시 억제")
        .subtitle("버퍼의 한글이 모두 완성 음절이면 교정하지 않음")
        .build();
    {
        let s = state.borrow();
        skip_syl_sw.set_active(s.config.engine.auto_typefix.skip_on_complete_syllable);
    }
    {
        let state_c = state.clone();
        skip_syl_sw.connect_active_notify(move |sw| {
            let mut s = state_c.borrow_mut();
            if s.updating {
                return;
            }
            s.config.engine.auto_typefix.skip_on_complete_syllable = sw.is_active();
            save_and_notify(&s.config, "auto_typefix_skip_on_complete_syllable");
        });
    }
    group.add(&skip_syl_sw);

    group
}

fn build_master_group(state: &State) -> adw::PreferencesGroup {
    let group = adw::PreferencesGroup::builder()
        .title("전체 기능")
        .description("순방향·역방향을 일괄 활성화/비활성화하는 마스터 스위치")
        .build();

    let master = adw::SwitchRow::builder()
        .title("자동 오타 교정 사용")
        .build();
    {
        let s = state.borrow();
        master.set_active(s.config.engine.auto_typefix.enabled);
    }
    {
        let state_c = state.clone();
        master.connect_active_notify(move |sw| {
            let mut s = state_c.borrow_mut();
            if s.updating {
                return;
            }
            s.config.engine.auto_typefix.enabled = sw.is_active();
            save_and_notify(&s.config, "auto_typefix_enabled");
        });
    }
    group.add(&master);

    // 재트리거 자동 감지
    let rollback_sw = adw::SwitchRow::builder()
        .title("재트리거 자동 감지")
        .subtitle("같은 입력이 관찰 창 내에 다시 교정되면 오탐 후보로 기록하고 억제")
        .build();
    {
        let s = state.borrow();
        rollback_sw.set_active(s.config.engine.auto_typefix.rollback_detection);
    }
    {
        let state_c = state.clone();
        rollback_sw.connect_active_notify(move |sw| {
            let mut s = state_c.borrow_mut();
            if s.updating {
                return;
            }
            s.config.engine.auto_typefix.rollback_detection = sw.is_active();
            save_and_notify(&s.config, "auto_typefix_rollback_detection");
        });
    }
    group.add(&rollback_sw);

    // 관찰 창 (초)
    let init_obs = state.borrow().config.engine.auto_typefix.observation_timeout_secs as i32;
    let obs_row = build_int_scale_row(
        state,
        "관찰 창 (초)",
        "첫 교정 후 이 시간 내 동일 입력이 재트리거되면 오탐으로 판정",
        AUTO_TYPEFIX_OBSERVATION_TIMEOUT_MIN as i32,
        AUTO_TYPEFIX_OBSERVATION_TIMEOUT_MAX as i32,
        init_obs,
        "auto_typefix_observation_timeout_secs",
        |cfg, v| cfg.engine.auto_typefix.observation_timeout_secs = v as u8,
    );
    group.add(&obs_row);

    // 임시 억제 만료 (시간)
    let init_exp = state.borrow().config.engine.auto_typefix.tentative_expiry_hours as i32;
    let exp_row = build_int_scale_row(
        state,
        "임시 억제 만료 (시간)",
        "이 기간 내 수동 확정하지 않으면 비활성화",
        AUTO_TYPEFIX_TENTATIVE_EXPIRY_MIN as i32,
        AUTO_TYPEFIX_TENTATIVE_EXPIRY_MAX as i32,
        init_exp,
        "auto_typefix_tentative_expiry_hours",
        |cfg, v| cfg.engine.auto_typefix.tentative_expiry_hours = v as u16,
    );
    group.add(&exp_row);

    group
}

fn ms_to_seconds(ms: u32) -> f64 {
    ms as f64 / 1000.0
}

fn seconds_to_ms(secs: f64) -> u32 {
    (secs * 1000.0).round() as u32
}

// ─────────────────────────────────────────────────────────────
// Page 3: GNOME Shell
// ─────────────────────────────────────────────────────────────

fn is_gnome_session() -> bool {
    std::env::var("XDG_CURRENT_DESKTOP")
        .map(|s| s.to_uppercase().contains("GNOME"))
        .unwrap_or(false)
}

fn is_wayland_session() -> bool {
    std::env::var("XDG_SESSION_TYPE")
        .map(|s| s.to_lowercase() == "wayland")
        .unwrap_or(false)
}

fn build_gnome_page(_window: &adw::PreferencesWindow) -> Option<adw::PreferencesPage> {
    // GSettings 스키마가 설치되어 있지 않으면 페이지를 생략
    let source = gio::SettingsSchemaSource::default()?;
    let _schema = source.lookup(GSCHEMA_ID, true)?;
    let gsettings = gio::Settings::new(GSCHEMA_ID);

    let page = adw::PreferencesPage::builder()
        .title("GNOME Shell")
        .icon_name("preferences-desktop-symbolic")
        .build();

    // 표시
    let disp = adw::PreferencesGroup::builder().title("표시").build();

    let panel_row = adw::SwitchRow::builder().title("상단 패널 인디케이터").build();
    panel_row.set_active(gsettings.boolean("show-panel-indicator"));
    {
        let gs = gsettings.clone();
        panel_row.connect_active_notify(move |sw| {
            let _ = gs.set_boolean("show-panel-indicator", sw.is_active());
            show_toast("저장됨 ✓");
        });
    }
    disp.add(&panel_row);

    let notif_row = adw::SwitchRow::builder().title("변환 알림 표시").build();
    notif_row.set_active(gsettings.boolean("show-notification"));
    {
        let gs = gsettings.clone();
        notif_row.connect_active_notify(move |sw| {
            let _ = gs.set_boolean("show-notification", sw.is_active());
            show_toast("저장됨 ✓");
        });
    }
    disp.add(&notif_row);

    page.add(&disp);

    // 실시간 입력기
    let ime = adw::PreferencesGroup::builder()
        .title("실시간 입력기")
        .description("Wayland 세션에서만 활성화됩니다")
        .build();

    let ime_row = adw::SwitchRow::builder().title("IME 모드 활성화").build();
    ime_row.set_active(gsettings.boolean("enable-ime"));
    ime_row.set_sensitive(is_wayland_session());
    {
        let gs = gsettings.clone();
        ime_row.connect_active_notify(move |sw| {
            let _ = gs.set_boolean("enable-ime", sw.is_active());
            show_toast("저장됨 ✓");
        });
    }
    ime.add(&ime_row);

    page.add(&ime);

    Some(page)
}

// ─────────────────────────────────────────────────────────────
// 공용 헬퍼: 쉼표 구분 문자열 리스트 EntryRow
// ─────────────────────────────────────────────────────────────

fn build_string_list_row<G, S>(
    state: &State,
    title: &str,
    subtitle: Option<&str>,
    get: G,
    set: S,
    label: &'static str,
) -> adw::EntryRow
where
    G: Fn(&Config) -> String + 'static,
    S: Fn(&mut Config, Vec<String>) + 'static,
{
    let row = adw::EntryRow::builder().title(title).build();
    // EntryRow는 subtitle을 직접 지원하지 않으므로 tooltip으로 대체
    if let Some(sub) = subtitle {
        row.set_tooltip_text(Some(sub));
    }

    {
        let s = state.borrow();
        row.set_text(&get(&s.config));
    }

    let state_c = state.clone();
    row.connect_changed(move |r| {
        let mut s = state_c.borrow_mut();
        if s.updating {
            return;
        }
        let text = r.text().to_string();
        let keys: Vec<String> = text
            .split(',')
            .map(|k| k.trim().to_string())
            .filter(|k| !k.is_empty())
            .collect();
        set(&mut s.config, keys);
        save_and_notify(&s.config, label);
    });

    row
}

// ─────────────────────────────────────────────────────────────
// Page 3: 교정 억제 단어 관리 (학습형 Blacklist)
// ─────────────────────────────────────────────────────────────

/// 3개 PreferencesGroup의 핸들 + 각 그룹에 추가된 Row들 추적용 Vec.
///
/// Adw.PreferencesGroup에는 "자식 전부 제거" API가 없으므로, 추가한 Row들을
/// 우리가 직접 추적해서 재구성 시 명시적으로 `remove()`로 떼어낸다.
struct BlacklistPageRefs {
    tentative_group: adw::PreferencesGroup,
    confirmed_group: adw::PreferencesGroup,
    inactive_group: adw::PreferencesGroup,
    tentative_rows: RefCell<Vec<gtk4::Widget>>,
    confirmed_rows: RefCell<Vec<gtk4::Widget>>,
    inactive_rows: RefCell<Vec<gtk4::Widget>>,
    last_mtime: RefCell<Option<std::time::SystemTime>>,
}

/// 교정 억제 단어 페이지 생성.
///
/// 파일(`~/.config/unim/typefix-blacklist.yaml`)을 직접 R/W하고, daemon은 mtime
/// 감지로 자동 reload한다 (별도 DBus 라운드트립 없음).
///
/// 3개 섹션:
/// - 승인 대기(Tentative): 자동 감지된 의심 단어. [확정]/[비활성화]/[삭제]
/// - 확정(Confirmed): 영구 억제. [비활성화]/[삭제]
/// - 비활성(Inactive): 만료/수동 비활성. [재활성화]/[삭제]
///
/// daemon이 파일을 갱신(BS+모드전환 감지 등)하는 경우를 위해, 2초 주기로 mtime을
/// 폴링하여 변경 시 UI를 실시간으로 다시 채운다.
fn build_blacklist_page() -> adw::PreferencesPage {
    let page = adw::PreferencesPage::builder()
        .title("교정 억제 단어")
        .icon_name("edit-clear-all-symbolic")
        .build();

    let tentative_group = adw::PreferencesGroup::builder().title("승인 대기").build();
    let confirmed_group = adw::PreferencesGroup::builder().title("확정").build();
    let inactive_group = adw::PreferencesGroup::builder().title("비활성").build();
    page.add(&tentative_group);
    page.add(&confirmed_group);
    page.add(&inactive_group);

    let refs = Rc::new(BlacklistPageRefs {
        tentative_group,
        confirmed_group,
        inactive_group,
        tentative_rows: RefCell::new(Vec::new()),
        confirmed_rows: RefCell::new(Vec::new()),
        inactive_rows: RefCell::new(Vec::new()),
        last_mtime: RefCell::new(None),
    });

    refill_blacklist_groups(&refs);

    // daemon이 파일을 갱신하는 경우를 실시간으로 반영하기 위한 2초 주기 mtime 폴링.
    // 페이지/창 수명이 끝나면 Rc가 drop되어 자동으로 타이머도 제거되도록 Weak 보유.
    let weak = Rc::downgrade(&refs);
    glib::timeout_add_seconds_local(2, move || match weak.upgrade() {
        Some(refs) => {
            if blacklist_file_changed(&refs) {
                refill_blacklist_groups(&refs);
            }
            glib::ControlFlow::Continue
        }
        None => glib::ControlFlow::Break,
    });

    page
}

/// 파일 mtime 변경 여부를 확인하고, 변경됐으면 `last_mtime`을 갱신한다.
fn blacklist_file_changed(refs: &Rc<BlacklistPageRefs>) -> bool {
    let Some(path) = Blacklist::default_path() else {
        return false;
    };
    let Ok(meta) = std::fs::metadata(&path) else {
        return false;
    };
    let Ok(mtime) = meta.modified() else {
        return false;
    };
    let mut last = refs.last_mtime.borrow_mut();
    match *last {
        Some(prev) if prev == mtime => false,
        _ => {
            *last = Some(mtime);
            true
        }
    }
}

/// 3개 그룹의 모든 row를 제거한 뒤 현재 파일 상태로 다시 채운다.
fn refill_blacklist_groups(refs: &Rc<BlacklistPageRefs>) {
    // 기존 row 모두 제거 — 추적해 둔 Vec를 명시적으로 순회.
    for row in refs.tentative_rows.borrow().iter() {
        refs.tentative_group.remove(row);
    }
    for row in refs.confirmed_rows.borrow().iter() {
        refs.confirmed_group.remove(row);
    }
    for row in refs.inactive_rows.borrow().iter() {
        refs.inactive_group.remove(row);
    }
    refs.tentative_rows.borrow_mut().clear();
    refs.confirmed_rows.borrow_mut().clear();
    refs.inactive_rows.borrow_mut().clear();

    let bl = Blacklist::load_from_default_path();

    let (mut t_count, mut c_count, mut i_count) = (0usize, 0usize, 0usize);
    for (idx, entry) in bl.entries.iter().enumerate() {
        match entry.status {
            EntryStatus::Tentative => {
                let row = build_blacklist_row(entry, idx, refs, BlacklistRowKind::Tentative);
                refs.tentative_group.add(&row);
                refs.tentative_rows.borrow_mut().push(row.upcast());
                t_count += 1;
            }
            EntryStatus::Confirmed => {
                let row = build_blacklist_row(entry, idx, refs, BlacklistRowKind::Confirmed);
                refs.confirmed_group.add(&row);
                refs.confirmed_rows.borrow_mut().push(row.upcast());
                c_count += 1;
            }
            EntryStatus::Inactive => {
                let row = build_blacklist_row(entry, idx, refs, BlacklistRowKind::Inactive);
                refs.inactive_group.add(&row);
                refs.inactive_rows.borrow_mut().push(row.upcast());
                i_count += 1;
            }
        }
    }

    // 비어 있는 섹션엔 placeholder row 하나 추가.
    if t_count == 0 {
        let row = empty_placeholder_row("기록 없음");
        refs.tentative_group.add(&row);
        refs.tentative_rows.borrow_mut().push(row.upcast());
    }
    if c_count == 0 {
        let row = empty_placeholder_row("기록 없음");
        refs.confirmed_group.add(&row);
        refs.confirmed_rows.borrow_mut().push(row.upcast());
    }
    if i_count == 0 {
        let row = empty_placeholder_row("기록 없음");
        refs.inactive_group.add(&row);
        refs.inactive_rows.borrow_mut().push(row.upcast());
    }

    refs.tentative_group.set_description(Some(&format!(
        "BS + 모드 전환으로 자동 감지된 의심 단어 — 확정하면 영구 억제 ({}개)",
        t_count
    )));
    refs.confirmed_group
        .set_description(Some(&format!("영구 억제 대상 ({}개)", c_count)));
    refs.inactive_group
        .set_description(Some(&format!("만료되어 억제 효과 없음 ({}개)", i_count)));
}

fn empty_placeholder_row(text: &str) -> adw::ActionRow {
    adw::ActionRow::builder().title(text).sensitive(false).build()
}

#[derive(Clone, Copy)]
enum BlacklistRowKind {
    Tentative,
    Confirmed,
    Inactive,
}

fn build_blacklist_row(
    entry: &unim::typefix_blacklist::BlacklistEntry,
    idx: usize,
    refs: &Rc<BlacklistPageRefs>,
    kind: BlacklistRowKind,
) -> adw::ActionRow {
    // ASCII → 한글 변환 (사용자가 그 레이아웃으로 타이핑했을 때 화면에 보였을 형태).
    // 역방향: ASCII가 영단어(예: "wood") → eng_to_kor 하면 사용자가 Korean 모드에서 본
    //         자모 조합(예: "ㅊㅊ"/세벌식390)이 나온다.
    // 순방향: ASCII가 영어 타이핑(예: "gksrmf") → eng_to_kor 하면 의도한 한글(예: "한글").
    let hangul_form = unim::typefix::eng_to_kor(
        &entry.ascii,
        &entry.korean_layout,
        entry.english_layout,
    );

    // 방향별 주/부 표시 규칙:
    // - 역방향(한→영): 주 = 한글(사용자가 친 실제 모습), 부 = 영어(저장된 ASCII)
    // - 순방향(영→한): 주 = 영어(사용자가 친 ASCII), 부 = 한글(교정 결과)
    let (title_text, counterpart) = match entry.direction {
        unim::typefix_blacklist::Direction::Reverse => (hangul_form.clone(), entry.ascii.clone()),
        unim::typefix_blacklist::Direction::Forward => (entry.ascii.clone(), hangul_form.clone()),
    };

    let subtitle = format!(
        "{} · {} · 한글:{} · 영문:{} · 히트:{}",
        counterpart,
        direction_label(entry.direction),
        unim::config::korean_layout_display_name(&entry.korean_layout),
        entry.english_layout.display_name(),
        entry.hit_count,
    );

    let row = adw::ActionRow::builder()
        .title(&title_text)
        .subtitle(&subtitle)
        .build();

    match kind {
        BlacklistRowKind::Tentative => {
            row.add_suffix(&make_action_button(
                "emblem-ok-symbolic",
                "확정",
                idx,
                refs,
                |bl, i| bl.promote_to_confirmed(i),
            ));
            row.add_suffix(&make_action_button(
                "window-close-symbolic",
                "비활성화",
                idx,
                refs,
                |bl, i| bl.deactivate(i),
            ));
            row.add_suffix(&make_action_button(
                "user-trash-symbolic",
                "삭제",
                idx,
                refs,
                |bl, i| bl.remove(i),
            ));
        }
        BlacklistRowKind::Confirmed => {
            row.add_suffix(&make_action_button(
                "window-close-symbolic",
                "비활성화",
                idx,
                refs,
                |bl, i| bl.deactivate(i),
            ));
            row.add_suffix(&make_action_button(
                "user-trash-symbolic",
                "삭제",
                idx,
                refs,
                |bl, i| bl.remove(i),
            ));
        }
        BlacklistRowKind::Inactive => {
            row.add_suffix(&make_action_button(
                "emblem-ok-symbolic",
                "재활성화(확정)",
                idx,
                refs,
                |bl, i| bl.reactivate_as_confirmed(i),
            ));
            row.add_suffix(&make_action_button(
                "user-trash-symbolic",
                "삭제",
                idx,
                refs,
                |bl, i| bl.remove(i),
            ));
        }
    }

    row
}

fn direction_label(dir: Direction) -> &'static str {
    match dir {
        Direction::Forward => "순방향(영→한)",
        Direction::Reverse => "역방향(한→영)",
    }
}

/// 버튼 하나를 만들어 `action`을 실행하고 페이지를 rebuild한다.
///
/// action은 `(&mut Blacklist, idx)`를 받아 상태 변경. 실행 후 파일 저장 + UI 새로고침.
fn make_action_button<F>(
    icon_name: &str,
    tooltip: &str,
    idx: usize,
    refs: &Rc<BlacklistPageRefs>,
    action: F,
) -> gtk4::Button
where
    F: Fn(&mut Blacklist, usize) + 'static,
{
    let btn = gtk4::Button::builder()
        .icon_name(icon_name)
        .tooltip_text(tooltip)
        .valign(gtk4::Align::Center)
        .css_classes(vec!["flat"])
        .build();

    let refs_c = refs.clone();
    btn.connect_clicked(move |_| {
        let mut bl = Blacklist::load_from_default_path();
        action(&mut bl, idx);
        match bl.save_to_default_path() {
            Ok(_) => {
                show_toast("저장됨 ✓");
                refill_blacklist_groups(&refs_c);
            }
            Err(e) => {
                unim_log!("INDICATOR", "[Settings] blacklist 저장 실패: {}", e);
                show_toast(&format!("저장 실패: {}", e));
            }
        }
    });
    btn
}
