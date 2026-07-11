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
use rust_i18n::t;

use unim::config::{
    CommitUnit, Config, InputCategory, ModeSharingMode, AUTO_TYPEFIX_ENG_MIN_LENGTH_MAX,
    AUTO_TYPEFIX_ENG_MIN_LENGTH_MIN, AUTO_TYPEFIX_KOR_THRESHOLD_MAX,
    AUTO_TYPEFIX_KOR_THRESHOLD_MIN, AUTO_TYPEFIX_OBSERVATION_TIMEOUT_MAX,
    AUTO_TYPEFIX_OBSERVATION_TIMEOUT_MIN, AUTO_TYPEFIX_TENTATIVE_EXPIRY_MAX,
    AUTO_TYPEFIX_TENTATIVE_EXPIRY_MIN, AUTO_TYPEFIX_TIME_WINDOW_MAX, AUTO_TYPEFIX_TIME_WINDOW_MIN,
};
use unim::keystroke::profile::LayoutProfile;
use unim::typefix_blacklist::{Blacklist, EntryStatus};
use unim::typefix_userdict::UserDictionary;
use unim::unim_log;

use unim_gui_common::settings_dbus::save_config_via_dbus;
use unim_gui_common::settings_helpers::{
    apply_korean_profile_choice, collect_korean_profile_choices, direction_label, is_gnome_session,
    is_wayland_session, load_and_resolve, ms_to_seconds, seconds_to_ms,
};

use std::cell::RefCell;
use std::rc::Rc;

// ─────────────────────────────────────────────────────────────
// 공용 상수
// ─────────────────────────────────────────────────────────────

const GSCHEMA_ID: &str = "org.gnome.shell.extensions.unim";
const TOAST_TIMEOUT_SECS: u32 = 2;
const WINDOW_MIN_WIDTH: i32 = 680;
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
        .title(t!("settings_window_title"))
        .default_width(WINDOW_MIN_WIDTH)
        .default_height(WINDOW_MIN_HEIGHT)
        .search_enabled(false)
        .build();

    // ── Page 1: 일반 ──────────────────────────────────────────
    let page_general = adw::PreferencesPage::builder()
        .title(t!("page_general_title"))
        .icon_name("preferences-system-symbolic")
        .build();
    let rule_sets_handle = build_rule_sets_group();
    let moachigi_handle = build_moachigi_group(&state);
    page_general.add(&build_keymap_group(&state, &rule_sets_handle, &moachigi_handle));
    page_general.add(&rule_sets_handle.group);
    page_general.add(&moachigi_handle.group);
    page_general.add(&build_input_mode_group(&state));
    page_general.add(&build_auto_english_group(&state));
    page_general.add(&build_accessibility_group(&state));
    window.add(&page_general);

    // ── Page 2: 오타 교정 ─────────────────────────────────────
    let page_typefix = adw::PreferencesPage::builder()
        .title(t!("page_typefix_title"))
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

    // ── Page 4: 사용자 사전 (역방향 whitelist) ───────────────
    let page_userdict = build_userdict_page(&window);
    window.add(&page_userdict);

    // ── Page 5: GNOME Shell (GNOME 세션 전용) ────────────────
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
        show_toast(&t!("settings_toast_save_failed", err = e.to_string()));
        return;
    }

    // 2. DBus 전파 (fire-and-forget)
    save_config_via_dbus(config, label);

    // 3. 토스트
    show_toast(&t!("settings_toast_saved"));
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


// ─────────────────────────────────────────────────────────────
// Page 1: 일반
// ─────────────────────────────────────────────────────────────


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
            self.group
                .set_description(Some(&t!("group_layout_options_empty")));
        } else {
            self.group
                .set_description(Some(&t!("group_layout_options_title")));
        }

        // 현재 GUI에 표시 중인 활성 집합을 미리 계산 — 첫 토글 시
        // `None → Some(vec)` 초기화의 시드로 쓴다. config가 None일 때 사용자가 한
        // 개를 끄면 *현재 표시 상태*에서 그것만 빠지는 것이 자연스러운 의미이므로,
        // profile_default 또는 rs.active로부터 도출한 활성 집합을 캡처한다.
        // `Rc`로 감싸 모든 행 콜백이 값-복제 비용 없이 공유한다.
        let initial_active_seed: Rc<Vec<String>> = {
            let mut acc = Vec::new();
            for (n, rs) in &profile.rule_sets {
                let on = match profile_default {
                    Some(list) => list.contains(n),
                    None => rs.active,
                };
                if on {
                    acc.push(n.clone());
                }
            }
            Rc::new(acc)
        };

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
            row.set_tooltip_text(Some(t!("row_rule_sets_label").as_ref()));

            // active 계산:
            //   Some(list) → 명시 override (빈 list면 모두 false)
            //   None       → 프로필의 active_rule_sets 또는 rule_set.active
            let is_active = match &config_active {
                Some(list) => list.contains(set_name),
                None => match profile_default {
                    Some(list) => list.contains(set_name),
                    None => rs.active,
                },
            };
            row.set_active(is_active);

            let state_c = state.clone();
            let name_c = set_name.clone();
            let seed = initial_active_seed.clone();
            row.connect_active_notify(move |r| {
                let mut s = state_c.borrow_mut();
                if s.updating {
                    return;
                }
                let on = r.is_active();
                // 첫 토글 시 None → Some(현재 표시 중 활성 집합)으로 "고정".
                // 이후 사용자의 모든 토글이 명시 override로 저장되어, 모두 OFF
                // 의도(`Some(vec![])`)도 손실 없이 보존된다.
                {
                    let list = s
                        .config
                        .engine
                        .korean
                        .active_rule_sets
                        .get_or_insert_with(|| (*seed).clone());
                    if on {
                        if !list.contains(&name_c) {
                            list.push(name_c.clone());
                        }
                    } else {
                        list.retain(|x| x != &name_c);
                    }
                }
                // 현재 자판의 캐시도 동기화 — 다음 자판 전환 시 본 상태가 보존된다.
                s.config.engine.korean.cache_active_rule_sets();
                save_and_notify(&s.config, "korean_active_rule_sets");
            });

            self.group.add(&row);
            self.rows.borrow_mut().push(row);
        }
    }
}

fn build_rule_sets_group() -> RuleSetsHandle {
    let group = adw::PreferencesGroup::builder()
        .title(t!("row_rule_sets_label"))
        .description(t!("row_layout_pending"))
        .build();
    RuleSetsHandle {
        group,
        rows: Rc::new(RefCell::new(Vec::new())),
    }
}

/// 모아치기 설정 그룹 핸들 — `supports_moachigi=true` 자판에서만 표시.
#[derive(Clone)]
struct MoachigiHandle {
    group: adw::PreferencesGroup,
    bidir_row: adw::SwitchRow,
    /// ActionRow는 Scale의 부모로만 사용 — 직접 접근 없이 lifetime 보존용.
    #[allow(dead_code)]
    chord_row: adw::ActionRow,
    chord_scale: gtk4::Scale,
}

impl MoachigiHandle {
    /// `profile.moachigi`에 따라 그룹 표시 여부와 위젯 초기값을 갱신.
    ///
    /// Phase 7 OPT-IN: 키맵 spec 기본값 미사용. 사용자 config 명시값만 반영.
    /// None = OFF (사용자가 명시적으로 활성화해야 모아치기 동작).
    fn refresh(&self, profile: &LayoutProfile, state: &State) {
        if profile.moachigi.is_none() {
            self.group.set_visible(false);
            return;
        }
        self.group.set_visible(true);

        // OPT-IN: 사용자 config 명시값만. None → OFF(false/0).
        let bidir_val = state
            .borrow()
            .config
            .engine
            .korean
            .bidirectional_combine
            .unwrap_or(false);
        let chord_val = state
            .borrow()
            .config
            .engine
            .korean
            .chord_window_ms
            .unwrap_or(0) as f64;

        // updating 플래그로 콜백 폭주 방지
        {
            let mut s = state.borrow_mut();
            s.updating = true;
        }
        self.bidir_row.set_active(bidir_val);
        self.chord_scale.set_value(chord_val);
        {
            let mut s = state.borrow_mut();
            s.updating = false;
        }
    }
}

fn build_moachigi_group(state: &State) -> MoachigiHandle {
    let group = adw::PreferencesGroup::builder()
        .title(t!("group_moachigi_title"))
        .build();
    // 처음엔 숨김 — refresh()에서 supports_moachigi 확인 후 표시
    group.set_visible(false);

    // SwitchRow: bidirectional_combine
    let bidir_row = adw::SwitchRow::builder()
        .title(t!("row_moachigi_bidirectional_label"))
        .subtitle(t!("row_moachigi_bidirectional_subtitle"))
        .build();
    bidir_row.set_tooltip_text(Some(t!("row_moachigi_bidirectional_tooltip").as_ref()));
    {
        let state_c = state.clone();
        bidir_row.connect_active_notify(move |r| {
            let mut s = state_c.borrow_mut();
            if s.updating {
                return;
            }
            s.config.engine.korean.bidirectional_combine = Some(r.is_active());
            save_and_notify(&s.config, "korean_bidirectional_combine");
        });
    }
    group.add(&bidir_row);

    // Scale: chord_window_ms (0=OFF 또는 10–200 ms, step 50, tick 50 단위)
    // 0/50/100/150/200 만 슬라이더로 도달. 미세값(예: 60, 80)은 CLI 로 별도 설정.
    let chord_scale = gtk4::Scale::with_range(gtk4::Orientation::Horizontal, 0.0, 200.0, 50.0);
    chord_scale.set_width_request(120);
    chord_scale.set_hexpand(false);
    chord_scale.set_valign(gtk4::Align::Center);
    chord_scale.set_draw_value(true);
    chord_scale.set_value_pos(gtk4::PositionType::Top);
    chord_scale.set_digits(0);
    chord_scale.set_round_digits(0);
    // 마우스 휠 1 tick = 10ms 변화 (드래그/클릭은 임의 값)
    chord_scale.adjustment().set_step_increment(10.0);
    // 눈금 마크 50ms 단위 (라벨 표시): 0(OFF)/50/100/150/200
    for mark in [0.0_f64, 50.0, 100.0, 150.0, 200.0] {
        let label = if mark == 0.0 {
            "OFF".to_string()
        } else {
            format!("{}", mark as i32)
        };
        chord_scale.add_mark(mark, gtk4::PositionType::Bottom, Some(&label));
    }
    {
        let state_c = state.clone();
        chord_scale.connect_value_changed(move |s_widget| {
            let mut s = state_c.borrow_mut();
            if s.updating {
                return;
            }
            s.config.engine.korean.chord_window_ms = Some(s_widget.value() as u16);
            save_and_notify(&s.config, "korean_chord_window_ms");
        });
    }
    let chord_row = adw::ActionRow::builder()
        .title(t!("row_moachigi_chord_label"))
        .subtitle(t!("row_moachigi_chord_subtitle"))
        .build();
    chord_row.set_tooltip_text(Some(t!("row_moachigi_chord_tooltip").as_ref()));
    chord_row.add_suffix(&chord_scale);
    group.add(&chord_row);

    MoachigiHandle {
        group,
        bidir_row,
        chord_row: chord_row.clone(),
        chord_scale,
    }
}

fn build_keymap_group(
    state: &State,
    rule_sets: &RuleSetsHandle,
    moachigi: &MoachigiHandle,
) -> adw::PreferencesGroup {
    let group = adw::PreferencesGroup::builder()
        .title(t!("group_layouts_keymap"))
        .build();

    // 한국어 자판 — 내장 + 사용자 프로필 통합
    let kor_row = adw::ComboRow::builder()
        .title(t!("row_korean_layout"))
        .subtitle(t!("row_korean_layout_subtitle"))
        .build();
    kor_row.set_tooltip_text(Some(t!("row_korean_layout_tooltip").as_ref()));
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
                )
            })
        })
        .unwrap_or(0);
    kor_row.set_selected(current_idx as u32);

    // 초기 rule_sets + moachigi 채우기
    if let Some((init_name, _)) = choices.get(current_idx) {
        if let Some(profile) = load_and_resolve(init_name) {
            rule_sets.refresh(&profile, state);
            moachigi.refresh(&profile, state);
        }
    }

    // 선택 변경 콜백
    {
        let state_c = state.clone();
        let rule_sets_c = rule_sets.clone();
        let moachigi_c = moachigi.clone();
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
            moachigi_c.refresh(&profile, &state_c);
            state_c.borrow_mut().updating = false;
        });
    }
    group.add(&kor_row);

    // 영어 자판 — Phase 9에서 enum 폐지 후 내장 5종 + 사용자 프로필 가능 (현재는 내장만 표시).
    let eng_row = adw::ComboRow::builder()
        .title(t!("row_english_layout"))
        .subtitle(t!("row_english_layout_subtitle"))
        .build();
    eng_row.set_tooltip_text(Some(t!("row_english_layout_tooltip").as_ref()));
    let eng_builtins: Vec<&str> = unim::config::ENGLISH_LAYOUT_BUILTINS.to_vec();
    let eng_items: Vec<&str> = eng_builtins
        .iter()
        .map(|l| unim::config::english_layout_display_name(l))
        .collect();
    let eng_list = gtk4::StringList::new(&eng_items);
    eng_row.set_model(Some(&eng_list));
    {
        let s = state.borrow();
        if let Some(idx) = eng_builtins
            .iter()
            .position(|l| *l == s.config.engine.english.layout)
        {
            eng_row.set_selected(idx as u32);
        }
    }
    {
        let state_c = state.clone();
        let builtins = eng_builtins.clone();
        eng_row.connect_selected_notify(move |row| {
            let mut s = state_c.borrow_mut();
            if s.updating {
                return;
            }
            if let Some(layout) = builtins.get(row.selected() as usize) {
                s.config.engine.english.layout = (*layout).to_string();
                save_and_notify(&s.config, "english_layout");
            }
        });
    }
    group.add(&eng_row);

    // 한/영 전환 키
    group.add(&build_string_list_row(
        state,
        &t!("row_toggle_keys"),
        Some(t!("row_toggle_keys_subtitle").as_ref()),
        Some(t!("row_toggle_keys_tooltip").as_ref()),
        |cfg| cfg.engine.toggle_keys.join(", "),
        |cfg, v| cfg.engine.toggle_keys = v,
        "toggle_keys",
    ));

    // 한자 키
    group.add(&build_string_list_row(
        state,
        &t!("row_hanja_keys"),
        Some(t!("row_hanja_keys_subtitle").as_ref()),
        Some(t!("row_hanja_keys_tooltip").as_ref()),
        |cfg| cfg.engine.hanja_keys.join(", "),
        |cfg, v| cfg.engine.hanja_keys = v,
        "hanja_keys",
    ));

    // 단축키 설정 row 는 제거됨 — 한자 키 idle 상태가 단일 진입점.
    // 이모지 팝업은 항상 활성 — 별도 토글 없음.

    group
}

fn build_input_mode_group(state: &State) -> adw::PreferencesGroup {
    let group = adw::PreferencesGroup::builder()
        .title(t!("group_input_mode"))
        .build();

    // 초기 입력 모드
    let init_row = adw::ComboRow::builder()
        .title(t!("row_initial_mode"))
        .subtitle(t!("row_initial_mode_subtitle"))
        .build();
    init_row.set_tooltip_text(Some(t!("row_initial_mode_tooltip").as_ref()));
    let init_list =
        gtk4::StringList::new(&[t!("mode_english").as_ref(), t!("mode_korean").as_ref()]);
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
    let share_row = adw::ComboRow::builder()
        .title(t!("row_mode_sharing"))
        .subtitle(t!("row_mode_sharing_subtitle"))
        .build();
    share_row.set_tooltip_text(Some(t!("row_mode_sharing_tooltip").as_ref()));
    let share_list = gtk4::StringList::new(&[
        t!("mode_share_global").as_ref(),
        t!("mode_share_perapp").as_ref(),
    ]);
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

    // 조합 확정 단위 (음절/단어/스마트) — 수치 아님이라 ComboRow (초기모드·모드공유와 동일 관례)
    let commit_row = adw::ComboRow::builder()
        .title(t!("row_commit_unit"))
        .subtitle(t!("row_commit_unit_subtitle"))
        .build();
    commit_row.set_tooltip_text(Some(t!("row_commit_unit_tooltip").as_ref()));
    let commit_list = gtk4::StringList::new(&[
        t!("commit_unit_syllable").as_ref(),
        t!("commit_unit_word").as_ref(),
        t!("commit_unit_smart").as_ref(),
    ]);
    commit_row.set_model(Some(&commit_list));
    {
        let s = state.borrow();
        commit_row.set_selected(match s.config.engine.korean.commit_unit {
            CommitUnit::Syllable => 0,
            CommitUnit::Word => 1,
            CommitUnit::Smart => 2,
        });
    }
    {
        let state_c = state.clone();
        commit_row.connect_selected_notify(move |row| {
            let mut s = state_c.borrow_mut();
            if s.updating {
                return;
            }
            s.config.engine.korean.commit_unit = match row.selected() {
                0 => CommitUnit::Syllable,
                1 => CommitUnit::Word,
                _ => CommitUnit::Smart,
            };
            save_and_notify(&s.config, "commit_unit");
        });
    }
    group.add(&commit_row);

    group
}

fn build_auto_english_group(state: &State) -> adw::PreferencesGroup {
    let group = adw::PreferencesGroup::builder()
        .title(t!("group_auto_english"))
        .description(
            "지정된 키(기본: key:Escape, char:/)를 한글 모드에서 입력하면 \
             조합을 커밋하고 영문 모드로 전환합니다 (vi/CLI 명령 호환). \
             char:<문자> 는 한국어 레이아웃과 무관하게 산출 문자를 비교합니다. \
             key:Ctrl+B 처럼 Ctrl/Alt/Super 조합도 지원하며, 이때 키는 앱으로 \
             그대로 전달됩니다 (tmux prefix 등).",
        )
        .build();

    // 활성화 스위치
    let sw = adw::SwitchRow::builder()
        .title(t!("row_auto_english_enable"))
        .subtitle(t!("row_auto_english_enable_subtitle"))
        .build();
    sw.set_tooltip_text(Some(t!("row_auto_english_enable_tooltip").as_ref()));
    {
        let s = state.borrow();
        sw.set_active(s.config.engine.auto_english.enabled);
    }
    {
        let state_c = state.clone();
        sw.connect_active_notify(move |sw| {
            let mut s = state_c.borrow_mut();
            if s.updating {
                return;
            }
            s.config.engine.auto_english.enabled = sw.is_active();
            save_and_notify(&s.config, "auto_english_enabled");
        });
    }
    group.add(&sw);

    // 트리거 키 목록 (쉼표 구분)
    group.add(&build_string_list_row(
        state,
        &t!("row_auto_english_triggers"),
        Some(t!("row_auto_english_triggers_subtitle").as_ref()),
        Some(t!("row_auto_english_triggers_tooltip").as_ref()),
        |cfg| cfg.engine.auto_english.trigger_keys.join(", "),
        |cfg, v| cfg.engine.auto_english.trigger_keys = v,
        "auto_english_keys",
    ));

    group
}

/// I7: 접근성 그룹 — 한/영 전환 소리 알림(비프) 토글.
///
/// 시각장애 사용자가 화면 없이도 한/영 토글 후 현재 모드를 소리 높낮이(한글=높은
/// 음, 영문=낮은 음)로 확인할 수 있게 한다. Windows TSF 전용 기능이지만 설정은
/// 단일 config.yaml 을 공유하므로 모든 프런트엔드에서 편집 가능하게 노출한다.
fn build_accessibility_group(state: &State) -> adw::PreferencesGroup {
    let group = adw::PreferencesGroup::builder()
        .title(t!("group_accessibility"))
        .build();

    let sw = adw::SwitchRow::builder()
        .title(t!("row_toggle_announce_beep"))
        .subtitle(t!("row_toggle_announce_beep_subtitle"))
        .build();
    sw.set_tooltip_text(Some(t!("row_toggle_announce_beep_tooltip").as_ref()));
    {
        let s = state.borrow();
        sw.set_active(s.config.engine.toggle_announce_beep);
    }
    {
        let state_c = state.clone();
        sw.connect_active_notify(move |sw| {
            let mut s = state_c.borrow_mut();
            if s.updating {
                return;
            }
            s.config.engine.toggle_announce_beep = sw.is_active();
            save_and_notify(&s.config, "toggle_announce_beep");
        });
    }
    group.add(&sw);

    // 조합키 자동반복 억제 (지체장애 접근성) — 키 홀드 시 연타·토글 진동 방지.
    // Windows TSF 전용 동작이지만 config.yaml 을 공유하므로 노출.
    let sw_repeat = adw::SwitchRow::builder()
        .title(t!("row_ignore_key_repeat"))
        .subtitle(t!("row_ignore_key_repeat_subtitle"))
        .build();
    sw_repeat.set_tooltip_text(Some(t!("row_ignore_key_repeat_tooltip").as_ref()));
    {
        let s = state.borrow();
        sw_repeat.set_active(s.config.engine.ignore_key_repeat);
    }
    {
        let state_c = state.clone();
        sw_repeat.connect_active_notify(move |sw_repeat| {
            let mut s = state_c.borrow_mut();
            if s.updating {
                return;
            }
            s.config.engine.ignore_key_repeat = sw_repeat.is_active();
            save_and_notify(&s.config, "ignore_key_repeat");
        });
    }
    group.add(&sw_repeat);

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
#[allow(clippy::too_many_arguments)] // title/subtitle/tooltip + min/max/init + label/set 묶음
fn build_int_scale_row(
    state: &State,
    title: &str,
    subtitle: &str,
    tooltip: &str,
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
    row.set_tooltip_text(Some(tooltip));

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

    // 마우스 휠 1 tick = +/-1 (정수 슬라이더)
    {
        let scale_c = scale.clone();
        let min_v = min as f64;
        let max_v = max as f64;
        let scroll_ctrl =
            gtk4::EventControllerScroll::new(gtk4::EventControllerScrollFlags::VERTICAL);
        scroll_ctrl.connect_scroll(move |_, _dx, dy| {
            let delta = if dy > 0.0 { 1.0 } else { -1.0 };
            let new_val = (scale_c.value() + delta).clamp(min_v, max_v);
            scale_c.set_value(new_val);
            gtk4::glib::Propagation::Stop
        });
        scale.add_controller(scroll_ctrl);
    }

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
    tooltip: &str,
    get_ms: impl Fn(&Config) -> u32 + 'static,
    set_ms: impl Fn(&mut Config, u32) + 'static,
    label: &'static str,
) -> adw::ActionRow {
    let row = adw::ActionRow::builder()
        .title(title)
        .subtitle(subtitle)
        .build();
    row.set_tooltip_text(Some(tooltip));

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

    // 마우스 휠 1 tick = +/-0.5초
    {
        let scale_c = scale.clone();
        let min_v = min_s;
        let max_v = max_s;
        let scroll_ctrl =
            gtk4::EventControllerScroll::new(gtk4::EventControllerScrollFlags::VERTICAL);
        scroll_ctrl.connect_scroll(move |_, _dx, dy| {
            let delta = if dy > 0.0 { 0.5 } else { -0.5 };
            let new_val = (scale_c.value() + delta).clamp(min_v, max_v);
            scale_c.set_value(new_val);
            gtk4::glib::Propagation::Stop
        });
        scale.add_controller(scroll_ctrl);
    }

    row.add_suffix(&scale);
    row
}

fn build_forward_group(state: &State) -> adw::PreferencesGroup {
    let group = adw::PreferencesGroup::builder()
        .title(t!("group_typefix_forward"))
        .build();

    // 사용 (forward)
    let fwd_sw = adw::SwitchRow::builder()
        .title(t!("row_typefix_enable"))
        .subtitle(t!("row_typefix_forward_enable_subtitle"))
        .build();
    fwd_sw.set_tooltip_text(Some(t!("row_typefix_forward_enable_tooltip").as_ref()));
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

    // 순방향 토글 단축키 (옵트인, 비우면 사용 안 함)
    group.add(&build_string_list_row(
        state,
        &t!("row_atf_forward_toggle_keys"),
        Some(t!("row_atf_forward_toggle_keys_subtitle").as_ref()),
        Some(t!("row_atf_forward_toggle_keys_tooltip").as_ref()),
        |cfg| cfg.engine.auto_typefix.toggle_forward_keys.join(", "),
        |cfg, v| cfg.engine.auto_typefix.toggle_forward_keys = v,
        "auto_typefix_toggle_forward_keys",
    ));

    // 임계 음절 수
    let init_kor = state
        .borrow()
        .config
        .engine
        .auto_typefix
        .kor_syllable_threshold as i32;
    let kor_row = build_int_scale_row(
        state,
        &t!("row_typefix_kor_threshold"),
        &t!("row_typefix_kor_threshold_subtitle"),
        &t!("row_typefix_kor_threshold_tooltip"),
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
        &t!("row_typefix_forward_window"),
        &t!("row_typefix_forward_window_subtitle"),
        &t!("row_typefix_forward_window_tooltip"),
        |cfg| cfg.engine.auto_typefix.forward_time_window_ms,
        |cfg, ms| cfg.engine.auto_typefix.forward_time_window_ms = ms,
        "auto_typefix_forward_time_window_ms",
    );
    group.add(&fwd_time_row);

    // 영단어 매칭 시 억제
    let skip_eng_sw = adw::SwitchRow::builder()
        .title(t!("row_typefix_skip_engword").as_ref())
        .subtitle(t!("row_typefix_skip_engword_subtitle"))
        .build();
    skip_eng_sw.set_tooltip_text(Some(t!("row_typefix_skip_engword_tooltip").as_ref()));
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
        .title(t!("group_typefix_reverse"))
        .build();

    // 사용 (reverse)
    let rev_sw = adw::SwitchRow::builder()
        .title(t!("row_typefix_enable"))
        .subtitle(t!("row_typefix_reverse_enable_subtitle"))
        .build();
    rev_sw.set_tooltip_text(Some(t!("row_typefix_reverse_enable_tooltip").as_ref()));
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

    // 역방향 토글 단축키 (옵트인, 비우면 사용 안 함)
    group.add(&build_string_list_row(
        state,
        &t!("row_atf_reverse_toggle_keys"),
        Some(t!("row_atf_reverse_toggle_keys_subtitle").as_ref()),
        Some(t!("row_atf_reverse_toggle_keys_tooltip").as_ref()),
        |cfg| cfg.engine.auto_typefix.toggle_reverse_keys.join(", "),
        |cfg, v| cfg.engine.auto_typefix.toggle_reverse_keys = v,
        "auto_typefix_toggle_reverse_keys",
    ));

    // 임계 글자 수
    let init_eng = state
        .borrow()
        .config
        .engine
        .auto_typefix
        .eng_word_min_length as i32;
    let eng_row = build_int_scale_row(
        state,
        &t!("row_typefix_eng_threshold"),
        &t!("row_typefix_eng_threshold_subtitle"),
        &t!("row_typefix_eng_threshold_tooltip"),
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
        &t!("row_typefix_forward_window"),
        &t!("row_typefix_reverse_window_subtitle"),
        &t!("row_typefix_reverse_window_tooltip"),
        |cfg| cfg.engine.auto_typefix.reverse_time_window_ms,
        |cfg, ms| cfg.engine.auto_typefix.reverse_time_window_ms = ms,
        "auto_typefix_reverse_time_window_ms",
    );
    group.add(&rev_time_row);

    // 온전한 음절 매칭 시 억제
    let skip_syl_sw = adw::SwitchRow::builder()
        .title(t!("row_typefix_skip_complete"))
        .subtitle(t!("row_typefix_skip_complete_subtitle"))
        .build();
    skip_syl_sw.set_tooltip_text(Some(t!("row_typefix_skip_complete_tooltip").as_ref()));
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

    // 사용자 사전 활성화 (PR #6)
    let user_dict_sw = adw::SwitchRow::builder()
        .title(t!("row_typefix_userdict"))
        .subtitle(t!("row_typefix_userdict_subtitle"))
        .build();
    user_dict_sw.set_tooltip_text(Some(t!("row_typefix_userdict_tooltip").as_ref()));
    {
        let s = state.borrow();
        user_dict_sw.set_active(s.config.engine.auto_typefix.user_dict_enabled);
    }
    {
        let state_c = state.clone();
        user_dict_sw.connect_active_notify(move |sw| {
            let mut s = state_c.borrow_mut();
            if s.updating {
                return;
            }
            s.config.engine.auto_typefix.user_dict_enabled = sw.is_active();
            save_and_notify(&s.config, "auto_typefix_user_dict_enabled");
        });
    }
    group.add(&user_dict_sw);

    group
}

fn build_master_group(state: &State) -> adw::PreferencesGroup {
    let group = adw::PreferencesGroup::builder()
        .title(t!("group_typefix_master"))
        .description(t!("row_typefix_master_subtitle"))
        .build();

    let master = adw::SwitchRow::builder()
        .title(t!("row_typefix_master"))
        .subtitle(t!("row_typefix_master_row_subtitle"))
        .build();
    master.set_tooltip_text(Some(t!("row_typefix_master_tooltip").as_ref()));
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

    // 전체 토글 단축키 (옵트인, 비우면 사용 안 함) — 오타 교정이 꺼져 있어도 켜는 용도라 여기 배치.
    group.add(&build_string_list_row(
        state,
        &t!("row_atf_toggle_keys"),
        Some(t!("row_atf_toggle_keys_subtitle").as_ref()),
        Some(t!("row_atf_toggle_keys_tooltip").as_ref()),
        |cfg| cfg.engine.auto_typefix.toggle_enabled_keys.join(", "),
        |cfg, v| cfg.engine.auto_typefix.toggle_enabled_keys = v,
        "auto_typefix_toggle_enabled_keys",
    ));

    // 재트리거 자동 감지
    let rollback_sw = adw::SwitchRow::builder()
        .title(t!("group_typefix_retrigger"))
        .subtitle(t!("row_typefix_retrigger_subtitle"))
        .build();
    rollback_sw.set_tooltip_text(Some(t!("row_typefix_retrigger_tooltip").as_ref()));
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
    let init_obs = state
        .borrow()
        .config
        .engine
        .auto_typefix
        .observation_timeout_secs as i32;
    let obs_row = build_int_scale_row(
        state,
        &t!("row_typefix_observe_window"),
        &t!("row_typefix_observe_window_subtitle"),
        &t!("row_typefix_observe_window_tooltip"),
        AUTO_TYPEFIX_OBSERVATION_TIMEOUT_MIN as i32,
        AUTO_TYPEFIX_OBSERVATION_TIMEOUT_MAX as i32,
        init_obs,
        "auto_typefix_observation_timeout_secs",
        |cfg, v| cfg.engine.auto_typefix.observation_timeout_secs = v as u8,
    );
    group.add(&obs_row);

    // 임시 억제 만료 (시간)
    let init_exp = state
        .borrow()
        .config
        .engine
        .auto_typefix
        .tentative_expiry_hours as i32;
    let exp_row = build_int_scale_row(
        state,
        &t!("row_typefix_tentative_expiry"),
        &t!("row_typefix_tentative_expiry_subtitle"),
        &t!("row_typefix_tentative_expiry_tooltip"),
        AUTO_TYPEFIX_TENTATIVE_EXPIRY_MIN as i32,
        AUTO_TYPEFIX_TENTATIVE_EXPIRY_MAX as i32,
        init_exp,
        "auto_typefix_tentative_expiry_hours",
        |cfg, v| cfg.engine.auto_typefix.tentative_expiry_hours = v as u16,
    );
    group.add(&exp_row);

    group
}

// ─────────────────────────────────────────────────────────────
// Page 3: GNOME Shell
// ─────────────────────────────────────────────────────────────

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
    let disp = adw::PreferencesGroup::builder()
        .title(t!("group_display"))
        .build();

    let panel_row = adw::SwitchRow::builder()
        .title(t!("row_panel_indicator"))
        .subtitle(t!("row_panel_indicator_subtitle"))
        .build();
    panel_row.set_tooltip_text(Some(t!("row_panel_indicator_tooltip").as_ref()));
    panel_row.set_active(gsettings.boolean("show-panel-indicator"));
    {
        let gs = gsettings.clone();
        panel_row.connect_active_notify(move |sw| {
            let _ = gs.set_boolean("show-panel-indicator", sw.is_active());
            show_toast(&t!("settings_toast_saved"));
        });
    }
    disp.add(&panel_row);

    let notif_row = adw::SwitchRow::builder()
        .title(t!("row_show_notification"))
        .subtitle(t!("row_show_notification_subtitle"))
        .build();
    notif_row.set_tooltip_text(Some(t!("row_show_notification_tooltip").as_ref()));
    notif_row.set_active(gsettings.boolean("show-notification"));
    {
        let gs = gsettings.clone();
        notif_row.connect_active_notify(move |sw| {
            let _ = gs.set_boolean("show-notification", sw.is_active());
            show_toast(&t!("settings_toast_saved"));
        });
    }
    disp.add(&notif_row);

    page.add(&disp);

    // 실시간 입력기
    let ime = adw::PreferencesGroup::builder()
        .title(t!("group_ime"))
        .description(t!("group_ime_subtitle"))
        .build();

    let ime_row = adw::SwitchRow::builder()
        .title(t!("row_ime_enable"))
        .subtitle(t!("row_ime_enable_subtitle"))
        .build();
    ime_row.set_tooltip_text(Some(t!("row_ime_enable_tooltip").as_ref()));
    ime_row.set_active(gsettings.boolean("enable-ime"));
    ime_row.set_sensitive(is_wayland_session());
    {
        let gs = gsettings.clone();
        ime_row.connect_active_notify(move |sw| {
            let _ = gs.set_boolean("enable-ime", sw.is_active());
            show_toast(&t!("settings_toast_saved"));
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
    tooltip: Option<&str>,
    get: G,
    set: S,
    label: &'static str,
) -> adw::EntryRow
where
    G: Fn(&Config) -> String + 'static,
    S: Fn(&mut Config, Vec<String>) + 'static,
{
    let row = adw::EntryRow::builder().title(title).build();
    // EntryRow는 visible subtitle을 직접 지원하지 않으므로 tooltip으로 노출.
    // tooltip이 명시적으로 주어지면 우선 사용하고, 그렇지 않으면 subtitle을 tooltip으로 폴백.
    if let Some(tip) = tooltip.or(subtitle) {
        row.set_tooltip_text(Some(tip));
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
        .title(t!("page_blacklist_title"))
        .icon_name("edit-clear-all-symbolic")
        .build();

    let tentative_group = adw::PreferencesGroup::builder()
        .title(t!("blacklist_group_tentative"))
        .build();
    let confirmed_group = adw::PreferencesGroup::builder()
        .title(t!("blacklist_group_confirmed"))
        .build();
    let inactive_group = adw::PreferencesGroup::builder()
        .title(t!("blacklist_group_inactive"))
        .build();
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
        let row = empty_placeholder_row(&t!("blacklist_empty"));
        refs.tentative_group.add(&row);
        refs.tentative_rows.borrow_mut().push(row.upcast());
    }
    if c_count == 0 {
        let row = empty_placeholder_row(&t!("blacklist_empty"));
        refs.confirmed_group.add(&row);
        refs.confirmed_rows.borrow_mut().push(row.upcast());
    }
    if i_count == 0 {
        let row = empty_placeholder_row(&t!("blacklist_empty"));
        refs.inactive_group.add(&row);
        refs.inactive_rows.borrow_mut().push(row.upcast());
    }

    refs.tentative_group.set_description(Some(
        t!("blacklist_tentative_desc", count = t_count.to_string()).as_ref(),
    ));
    refs.confirmed_group.set_description(Some(
        t!("blacklist_confirmed_desc", count = c_count.to_string()).as_ref(),
    ));
    refs.inactive_group.set_description(Some(
        t!("blacklist_inactive_desc", count = i_count.to_string()).as_ref(),
    ));
}

fn empty_placeholder_row(text: &str) -> adw::ActionRow {
    adw::ActionRow::builder()
        .title(text)
        .sensitive(false)
        .build()
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
    let hangul_form =
        unim::typefix::eng_to_kor(&entry.ascii, &entry.korean_layout, &entry.english_layout);

    // 방향별 주/부 표시 규칙:
    // - 역방향(한→영): 주 = 한글(사용자가 친 실제 모습), 부 = 영어(저장된 ASCII)
    // - 순방향(영→한): 주 = 영어(사용자가 친 ASCII), 부 = 한글(교정 결과)
    let (title_text, counterpart) = match entry.direction {
        unim::typefix_blacklist::Direction::Reverse => (hangul_form.clone(), entry.ascii.clone()),
        unim::typefix_blacklist::Direction::Forward => (entry.ascii.clone(), hangul_form.clone()),
    };

    let subtitle = t!(
        "blacklist_row_summary",
        kind = counterpart,
        ts = direction_label(entry.direction),
        ko = unim::config::korean_layout_display_name(&entry.korean_layout),
        en = unim::config::english_layout_display_name(&entry.english_layout),
        hits = entry.hit_count.to_string(),
    )
    .into_owned();

    let row = adw::ActionRow::builder()
        .title(&title_text)
        .subtitle(&subtitle)
        .build();

    match kind {
        BlacklistRowKind::Tentative => {
            row.add_suffix(&make_action_button(
                "emblem-ok-symbolic",
                &t!("blacklist_btn_confirm"),
                idx,
                refs,
                |bl, i| bl.promote_to_confirmed(i),
            ));
            row.add_suffix(&make_action_button(
                "window-close-symbolic",
                &t!("blacklist_btn_disable"),
                idx,
                refs,
                |bl, i| bl.deactivate(i),
            ));
            row.add_suffix(&make_action_button(
                "user-trash-symbolic",
                &t!("blacklist_btn_delete"),
                idx,
                refs,
                |bl, i| bl.remove(i),
            ));
        }
        BlacklistRowKind::Confirmed => {
            row.add_suffix(&make_action_button(
                "window-close-symbolic",
                &t!("blacklist_btn_disable"),
                idx,
                refs,
                |bl, i| bl.deactivate(i),
            ));
            row.add_suffix(&make_action_button(
                "user-trash-symbolic",
                &t!("blacklist_btn_delete"),
                idx,
                refs,
                |bl, i| bl.remove(i),
            ));
        }
        BlacklistRowKind::Inactive => {
            row.add_suffix(&make_action_button(
                "emblem-ok-symbolic",
                &t!("blacklist_btn_reactivate"),
                idx,
                refs,
                |bl, i| bl.reactivate_as_confirmed(i),
            ));
            row.add_suffix(&make_action_button(
                "user-trash-symbolic",
                &t!("blacklist_btn_delete"),
                idx,
                refs,
                |bl, i| bl.remove(i),
            ));
        }
    }

    row
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
                show_toast(&t!("settings_toast_saved"));
                refill_blacklist_groups(&refs_c);
            }
            Err(e) => {
                unim_log!("INDICATOR", "[Settings] blacklist 저장 실패: {}", e);
                show_toast(&t!("settings_toast_save_failed", err = e.to_string()));
            }
        }
    });
    btn
}

// ─────────────────────────────────────────────────────────────
// 사용자 사전 페이지 (역방향 AutoTypeFix whitelist)
// ─────────────────────────────────────────────────────────────

struct UserDictPageRefs {
    group: adw::PreferencesGroup,
    rows: RefCell<Vec<gtk4::Widget>>,
    last_mtime: RefCell<Option<std::time::SystemTime>>,
    window: adw::PreferencesWindow,
}

/// 사용자 사전 페이지 생성.
///
/// 파일(`~/.config/unim/typefix-userdict.yaml`)을 직접 R/W하고, daemon은 mtime
/// 감지로 자동 reload한다 (blacklist와 동일 패턴).
fn build_userdict_page(window: &adw::PreferencesWindow) -> adw::PreferencesPage {
    let page = adw::PreferencesPage::builder()
        .title(t!("page_userdict_title"))
        .icon_name("accessories-dictionary-symbolic")
        .build();

    let group = adw::PreferencesGroup::builder()
        .title(t!("userdict_group_title"))
        .description(t!("userdict_group_desc").as_ref())
        .build();

    // 상단 "추가" 버튼 (헤더 suffix)
    let add_btn = gtk4::Button::builder()
        .icon_name("list-add-symbolic")
        .tooltip_text(t!("userdict_btn_add_tooltip"))
        .valign(gtk4::Align::Center)
        .css_classes(vec!["flat"])
        .build();
    group.set_header_suffix(Some(&add_btn));
    page.add(&group);

    let refs = Rc::new(UserDictPageRefs {
        group,
        rows: RefCell::new(Vec::new()),
        last_mtime: RefCell::new(None),
        window: window.clone(),
    });

    // 클릭 → 추가 다이얼로그
    {
        let refs_c = refs.clone();
        add_btn.connect_clicked(move |_| {
            show_userdict_edit_dialog(&refs_c, None);
        });
    }

    refill_userdict_group(&refs);

    // mtime 폴링으로 외부 변경 반영 (CLI/단축키)
    let weak = Rc::downgrade(&refs);
    glib::timeout_add_seconds_local(2, move || match weak.upgrade() {
        Some(refs) => {
            if userdict_file_changed(&refs) {
                refill_userdict_group(&refs);
            }
            glib::ControlFlow::Continue
        }
        None => glib::ControlFlow::Break,
    });

    page
}

fn userdict_file_changed(refs: &Rc<UserDictPageRefs>) -> bool {
    let Some(path) = UserDictionary::default_path() else {
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

fn refill_userdict_group(refs: &Rc<UserDictPageRefs>) {
    for row in refs.rows.borrow().iter() {
        refs.group.remove(row);
    }
    refs.rows.borrow_mut().clear();

    let ud = UserDictionary::load_from_default_path();
    if ud.is_empty() {
        let row = adw::ActionRow::builder()
            .title(t!("userdict_empty"))
            .subtitle(t!("userdict_empty_subtitle"))
            .sensitive(false)
            .build();
        refs.group.add(&row);
        refs.rows.borrow_mut().push(row.upcast());
    } else {
        for (idx, entry) in ud.reverse_words.iter().enumerate() {
            let subtitle = entry.note.clone().unwrap_or_default();
            let row = adw::ActionRow::builder()
                .title(&entry.word)
                .subtitle(&subtitle)
                .build();

            // 편집 버튼
            let edit_btn = gtk4::Button::builder()
                .icon_name("document-edit-symbolic")
                .tooltip_text(t!("userdict_btn_edit"))
                .valign(gtk4::Align::Center)
                .css_classes(vec!["flat"])
                .build();
            {
                let refs_c = refs.clone();
                edit_btn.connect_clicked(move |_| {
                    show_userdict_edit_dialog(&refs_c, Some(idx));
                });
            }
            row.add_suffix(&edit_btn);

            // 삭제 버튼
            let del_btn = gtk4::Button::builder()
                .icon_name("user-trash-symbolic")
                .tooltip_text(t!("blacklist_btn_delete"))
                .valign(gtk4::Align::Center)
                .css_classes(vec!["flat"])
                .build();
            {
                let refs_c = refs.clone();
                del_btn.connect_clicked(move |_| {
                    let mut ud = UserDictionary::load_from_default_path();
                    if ud.remove_at(idx) {
                        match ud.save_to_default_path() {
                            Ok(_) => {
                                show_toast(&t!("settings_toast_deleted"));
                                refill_userdict_group(&refs_c);
                            }
                            Err(e) => {
                                unim_log!("INDICATOR", "[Settings] userdict 저장 실패: {}", e);
                                show_toast(&t!("settings_toast_save_failed", err = e.to_string()));
                            }
                        }
                    }
                });
            }
            row.add_suffix(&del_btn);

            refs.group.add(&row);
            refs.rows.borrow_mut().push(row.upcast());
        }
    }

    let count = ud.len();
    refs.group.set_description(Some(
        t!("userdict_group_desc_count", count = count.to_string()).as_ref(),
    ));
}

/// 추가/편집 다이얼로그. `edit_idx=None`이면 신규 추가.
fn show_userdict_edit_dialog(refs: &Rc<UserDictPageRefs>, edit_idx: Option<usize>) {
    let is_edit = edit_idx.is_some();
    let dialog = adw::Window::builder()
        .transient_for(&refs.window)
        .modal(true)
        .title({
            let s = if is_edit {
                t!("userdict_dialog_edit_title")
            } else {
                t!("userdict_btn_add_tooltip")
            };
            s.into_owned()
        })
        .default_width(420)
        .default_height(280)
        .build();

    let toolbar = adw::ToolbarView::new();
    let header = adw::HeaderBar::builder().build();
    toolbar.add_top_bar(&header);

    let content = gtk4::Box::new(gtk4::Orientation::Vertical, 12);
    content.set_margin_top(18);
    content.set_margin_bottom(18);
    content.set_margin_start(18);
    content.set_margin_end(18);

    let group = adw::PreferencesGroup::new();
    let word_row = adw::EntryRow::builder()
        .title(t!("userdict_dialog_field_word"))
        .build();
    let note_row = adw::EntryRow::builder()
        .title(t!("userdict_dialog_field_desc"))
        .build();

    // 기존 엔트리 로드
    if let Some(idx) = edit_idx {
        let ud = UserDictionary::load_from_default_path();
        if let Some(e) = ud.reverse_words.get(idx) {
            word_row.set_text(&e.word);
            if let Some(n) = &e.note {
                note_row.set_text(n);
            }
        }
    }

    group.add(&word_row);
    group.add(&note_row);
    content.append(&group);

    let btn_box = gtk4::Box::new(gtk4::Orientation::Horizontal, 8);
    btn_box.set_halign(gtk4::Align::End);
    let cancel_btn = gtk4::Button::with_label(&t!("common_cancel"));
    let save_label = if is_edit {
        t!("common_save")
    } else {
        t!("common_add")
    };
    let save_btn = gtk4::Button::with_label(save_label.as_ref());
    save_btn.add_css_class("suggested-action");
    btn_box.append(&cancel_btn);
    btn_box.append(&save_btn);
    content.append(&btn_box);

    toolbar.set_content(Some(&content));
    dialog.set_content(Some(&toolbar));

    {
        let dialog_c = dialog.clone();
        cancel_btn.connect_clicked(move |_| dialog_c.close());
    }

    {
        let refs_c = refs.clone();
        let dialog_c = dialog.clone();
        let word_row_c = word_row.clone();
        let note_row_c = note_row.clone();
        save_btn.connect_clicked(move |_| {
            let word = word_row_c.text().to_string();
            let note_raw = note_row_c.text().to_string();
            let note = if note_raw.trim().is_empty() {
                None
            } else {
                Some(note_raw)
            };

            let mut ud = UserDictionary::load_from_default_path();
            let ok = match edit_idx {
                Some(idx) => ud.update_at(idx, &word, note),
                None => ud.add(&word, note),
            };

            if !ok {
                show_toast(&t!("userdict_add_failed"));
                return;
            }
            match ud.save_to_default_path() {
                Ok(_) => {
                    let toast_msg = if is_edit {
                        t!("settings_toast_saved")
                    } else {
                        t!("settings_toast_added")
                    };
                    show_toast(toast_msg.as_ref());
                    refill_userdict_group(&refs_c);
                    dialog_c.close();
                }
                Err(e) => {
                    unim_log!("INDICATOR", "[Settings] userdict 저장 실패: {}", e);
                    show_toast(&t!("settings_toast_save_failed", err = e.to_string()));
                }
            }
        });
    }

    dialog.present();
}
