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
    Config, EnglishLayout, InputCategory, KoreanLayout, ModeSharingMode, PopupMode,
    AUTO_TYPEFIX_ENG_MIN_LENGTH_MAX, AUTO_TYPEFIX_ENG_MIN_LENGTH_MIN,
    AUTO_TYPEFIX_KOR_THRESHOLD_MAX, AUTO_TYPEFIX_KOR_THRESHOLD_MIN,
    AUTO_TYPEFIX_TIME_WINDOW_MAX, AUTO_TYPEFIX_TIME_WINDOW_MIN,
};
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
    page_general.add(&build_keymap_group(&state));
    page_general.add(&build_input_mode_group(&state));
    window.add(&page_general);

    // ── Page 2: 오타 교정 ─────────────────────────────────────
    let page_typefix = adw::PreferencesPage::builder()
        .title("오타 교정")
        .icon_name("edit-find-replace-symbolic")
        .build();

    // forward/reverse 트리거 윈도우 SpinRow는 동일한 `time_window_ms`를 공유한다.
    // 양방향 sync를 위해 Rc<RefCell<...>>로 핸들 공유.
    let time_sync: Rc<RefCell<(Option<adw::SpinRow>, Option<adw::SpinRow>)>> =
        Rc::new(RefCell::new((None, None)));

    let forward_group = build_forward_group(&state, &time_sync);
    let reverse_group = build_reverse_group(&state, &time_sync);
    let master_group = build_master_group(&state);

    page_typefix.add(&forward_group);
    page_typefix.add(&reverse_group);
    page_typefix.add(&master_group);
    window.add(&page_typefix);

    // ── Page 3: GNOME Shell (GNOME 세션 전용) ────────────────
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

fn build_keymap_group(state: &State) -> adw::PreferencesGroup {
    let group = adw::PreferencesGroup::builder().title("자판 및 키맵").build();

    // 한국어 자판
    let kor_row = adw::ComboRow::builder().title("한국어 자판").build();
    let kor_items: Vec<&str> = KoreanLayout::all()
        .iter()
        .map(|l| l.display_name())
        .collect();
    let kor_list = gtk4::StringList::new(&kor_items);
    kor_row.set_model(Some(&kor_list));
    {
        let s = state.borrow();
        if let Some(idx) = KoreanLayout::all()
            .iter()
            .position(|l| *l == s.config.engine.korean.layout)
        {
            kor_row.set_selected(idx as u32);
        }
    }
    {
        let state_c = state.clone();
        kor_row.connect_selected_notify(move |row| {
            let mut s = state_c.borrow_mut();
            if s.updating {
                return;
            }
            if let Some(layout) = KoreanLayout::all().get(row.selected() as usize) {
                s.config.engine.korean.layout = *layout;
                save_and_notify(&s.config, "korean_layout");
            }
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
// Page 2: 오타 교정
// ─────────────────────────────────────────────────────────────

type TimeSyncSlot = Rc<RefCell<(Option<adw::SpinRow>, Option<adw::SpinRow>)>>;

fn build_forward_group(state: &State, time_sync: &TimeSyncSlot) -> adw::PreferencesGroup {
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
    let kor_adj = gtk4::Adjustment::new(
        2.0,
        AUTO_TYPEFIX_KOR_THRESHOLD_MIN as f64,
        AUTO_TYPEFIX_KOR_THRESHOLD_MAX as f64,
        1.0,
        1.0,
        0.0,
    );
    let kor_row = adw::SpinRow::builder()
        .title("임계 음절 수")
        .subtitle("이 개수 이상의 완성 한글이 감지되면 교정 검사")
        .adjustment(&kor_adj)
        .digits(0)
        .build();
    {
        let s = state.borrow();
        kor_adj.set_value(s.config.engine.auto_typefix.kor_syllable_threshold as f64);
    }
    {
        let state_c = state.clone();
        kor_row.connect_value_notify(move |row| {
            let mut s = state_c.borrow_mut();
            if s.updating {
                return;
            }
            s.config.engine.auto_typefix.kor_syllable_threshold = row.value() as u8;
            save_and_notify(&s.config, "auto_typefix_kor_syllable_threshold");
        });
    }
    group.add(&kor_row);

    // 트리거 윈도우 (초) — forward/reverse가 time_window_ms 공유
    let fwd_time_row = build_time_window_row(state, time_sync, true);
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

    // time_sync 슬롯에 forward 등록
    time_sync.borrow_mut().0 = Some(fwd_time_row);

    group
}

fn build_reverse_group(state: &State, time_sync: &TimeSyncSlot) -> adw::PreferencesGroup {
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
    let eng_adj = gtk4::Adjustment::new(
        5.0,
        AUTO_TYPEFIX_ENG_MIN_LENGTH_MIN as f64,
        AUTO_TYPEFIX_ENG_MIN_LENGTH_MAX as f64,
        1.0,
        1.0,
        0.0,
    );
    let eng_row = adw::SpinRow::builder()
        .title("임계 글자 수")
        .subtitle("이 개수 이상의 한글이 감지되면 교정 검사")
        .adjustment(&eng_adj)
        .digits(0)
        .build();
    {
        let s = state.borrow();
        eng_adj.set_value(s.config.engine.auto_typefix.eng_word_min_length as f64);
    }
    {
        let state_c = state.clone();
        eng_row.connect_value_notify(move |row| {
            let mut s = state_c.borrow_mut();
            if s.updating {
                return;
            }
            s.config.engine.auto_typefix.eng_word_min_length = row.value() as u8;
            save_and_notify(&s.config, "auto_typefix_eng_word_min_length");
        });
    }
    group.add(&eng_row);

    // 트리거 윈도우 (초) — forward와 값 공유
    let rev_time_row = build_time_window_row(state, time_sync, false);
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

    // time_sync 슬롯에 reverse 등록
    time_sync.borrow_mut().1 = Some(rev_time_row);

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
    group
}

/// 트리거 윈도우(초) SpinRow 생성.
///
/// - UI: 0.5 ~ 5.0초, step 0.5, digits=1
/// - 저장: `f64 초 × 1000 → u32 ms`
/// - forward/reverse 두 SpinRow가 동일한 `time_window_ms`를 공유하므로
///   변경 시 다른 쪽 SpinRow도 `set_value`로 sync (updating 플래그로 재진입 방지).
fn build_time_window_row(
    state: &State,
    time_sync: &TimeSyncSlot,
    is_forward: bool,
) -> adw::SpinRow {
    let min_s = AUTO_TYPEFIX_TIME_WINDOW_MIN as f64 / 1000.0;
    let max_s = AUTO_TYPEFIX_TIME_WINDOW_MAX as f64 / 1000.0;
    let adj = gtk4::Adjustment::new(max_s, min_s, max_s, 0.5, 0.5, 0.0);
    let row = adw::SpinRow::builder()
        .title("트리거 윈도우 (초)")
        .subtitle("최근 입력을 유효한 것으로 간주할 시간")
        .adjustment(&adj)
        .digits(1)
        .build();

    {
        let s = state.borrow();
        adj.set_value(ms_to_seconds(s.config.engine.auto_typefix.time_window_ms));
    }

    let time_sync_c = time_sync.clone();
    let state_c = state.clone();
    row.connect_value_notify(move |row| {
        let mut s = state_c.borrow_mut();
        if s.updating {
            return;
        }
        let ms = seconds_to_ms(row.value());
        s.config.engine.auto_typefix.time_window_ms = ms;

        // 반대쪽 SpinRow sync
        let slot = time_sync_c.borrow();
        let other = if is_forward { slot.1.clone() } else { slot.0.clone() };
        drop(slot);
        if let Some(other) = other {
            let new_val = ms_to_seconds(ms);
            if (other.value() - new_val).abs() > f64::EPSILON {
                s.updating = true;
                other.set_value(new_val);
                s.updating = false;
            }
        }

        save_and_notify(&s.config, "auto_typefix_time_window_ms");
    });

    row
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
