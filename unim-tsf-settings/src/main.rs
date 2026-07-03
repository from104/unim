//! UNIM TSF 설정 GUI (Slint).
//!
//! DLL 내부 Win32 모달 다이얼로그(`unim-tsf/src/settings_dialog.rs`)의 호스트
//! 메시지 펌프 충돌 문제를 피하기 위해, 설정 UI 를 별도 프로세스로 분리한다.
//! 코어 `unim::config::Config` 와 blacklist/userdict yaml 을 그대로 공유한다.
//!
//! - 일반: 한글/영문 자판, 시작 모드, 모드 공유, 토글/한자 키, 자동 영문 전환
//! - 오타 교정: AutoTypeFix 방향·임계값·옵션
//! - 억제 단어: blacklist 조회/삭제 (변경 즉시 저장)
//! - 사용자 사전: userdict 추가/삭제 (변경 즉시 저장)
//
// GUI 전용 앱이므로 콘솔 창을 띄우지 않는다(무조건). debug 빌드에서도 빈
// 콘솔 창이 먼저 뜨지 않도록 cfg_attr 없이 항상 windows 서브시스템을 쓴다.
#![windows_subsystem = "windows"]

use std::cell::RefCell;
use std::rc::Rc;

use slint::{ModelRc, SharedString, StandardListViewItem, VecModel};

use unim::config::{
    english_layout_display_name, korean_layout_display_name, normalize_korean_layout_name,
    AutoTypeFixConfig, CommitUnit, Config, InputCategory, ModeSharingMode,
    ENGLISH_LAYOUT_BUILTINS, KOREAN_LAYOUT_BUILTINS, KOREAN_LAYOUT_SEBEOLSIK_NOSHIFT,
    AUTO_TYPEFIX_ENG_MIN_LENGTH_MAX, AUTO_TYPEFIX_ENG_MIN_LENGTH_MIN,
    AUTO_TYPEFIX_KOR_THRESHOLD_MAX, AUTO_TYPEFIX_KOR_THRESHOLD_MIN,
    AUTO_TYPEFIX_OBSERVATION_TIMEOUT_MAX, AUTO_TYPEFIX_OBSERVATION_TIMEOUT_MIN,
    AUTO_TYPEFIX_TENTATIVE_EXPIRY_MAX, AUTO_TYPEFIX_TENTATIVE_EXPIRY_MIN,
    AUTO_TYPEFIX_TIME_WINDOW_MAX, AUTO_TYPEFIX_TIME_WINDOW_MIN,
};
use unim::keystroke::profile::{resolve_inherits, LayoutProfile, ProfileRegistry};
use unim::typefix_blacklist::{Blacklist, BlacklistEntry, Direction, EntryStatus};
use unim::typefix_userdict::{ReverseWord, UserDictionary};

slint::include_modules!();

/// OS 기본 UI 언어가 한국어(LANG_KOREAN=0x12)인지 판정.
/// GetUserDefaultUILanguage 는 하위 10비트가 primary language id.
/// 비Windows 빌드(테스트/린트)에서는 항상 false → 영어 기본.
#[cfg(windows)]
fn ui_language_is_korean() -> bool {
    extern "system" {
        fn GetUserDefaultUILanguage() -> u16;
    }
    // SAFETY: 인자 없는 순수 조회 Win32 API.
    let langid = unsafe { GetUserDefaultUILanguage() };
    (langid & 0x3ff) == 0x12
}
#[cfg(not(windows))]
fn ui_language_is_korean() -> bool {
    false
}

/// 단일 인스턴스 가드 (Windows). 이미 설정 창이 떠 있으면 그 창을 전면화하고
/// `false` 를 돌려 호출자가 즉시 종료하도록 한다. 첫 인스턴스면 `true`.
///
/// 명명 뮤텍스(`Local\` = 세션 로컬)로 중복 실행을 감지하고, 기존 창은 제목으로
/// `FindWindowW` 해서 최소화 상태면 복원 후 `SetForegroundWindow` 로 끌어올린다.
/// 창 제목은 실행 중 인스턴스와 동일 규칙(OS UI 언어)으로 계산한다.
/// 첫 인스턴스가 만든 뮤텍스 핸들은 프로세스 종료 시 OS 가 정리하므로 닫지 않는다
/// (원시 포인터라 스코프 이탈만으로는 커널 핸들이 닫히지 않는다).
#[cfg(windows)]
fn acquire_singleton_or_foreground() -> bool {
    use std::ffi::c_void;
    extern "system" {
        fn CreateMutexW(attrs: *const c_void, owner: i32, name: *const u16) -> *mut c_void;
        fn GetLastError() -> u32;
        fn FindWindowW(class: *const u16, window: *const u16) -> *mut c_void;
        fn SetForegroundWindow(hwnd: *mut c_void) -> i32;
        fn ShowWindow(hwnd: *mut c_void, cmd: i32) -> i32;
        fn IsIconic(hwnd: *mut c_void) -> i32;
    }
    const ERROR_ALREADY_EXISTS: u32 = 183;
    const SW_RESTORE: i32 = 9;

    fn wide(s: &str) -> Vec<u16> {
        s.encode_utf16().chain(std::iter::once(0)).collect()
    }

    let name = wide("Local\\unim-tsf-settings-singleton");
    // SAFETY: 명명 커널 오브젝트 생성. 인자는 널종단 UTF-16 이름 하나뿐.
    let already_running = unsafe {
        let _h = CreateMutexW(std::ptr::null(), 0, name.as_ptr());
        GetLastError() == ERROR_ALREADY_EXISTS
    };
    if !already_running {
        return true;
    }

    // 이미 실행 중 → 기존 창 전면화. 제목은 실행 중 인스턴스와 동일 규칙으로 계산.
    let title = wide(if ui_language_is_korean() {
        "UNIM 설정"
    } else {
        "UNIM Settings"
    });
    // SAFETY: 조회/포커스 이동 Win32 호출. HWND 는 널 검사 후에만 사용.
    unsafe {
        let hwnd = FindWindowW(std::ptr::null(), title.as_ptr());
        if !hwnd.is_null() {
            if IsIconic(hwnd) != 0 {
                ShowWindow(hwnd, SW_RESTORE);
            }
            SetForegroundWindow(hwnd);
        }
    }
    false
}
#[cfg(not(windows))]
fn acquire_singleton_or_foreground() -> bool {
    true
}

/// "a, b, c" → ["a","b","c"] (공백 trim, 빈 항목 제거).
fn split_keys(s: &str) -> Vec<String> {
    s.split(',')
        .map(|k| k.trim())
        .filter(|k| !k.is_empty())
        .map(|k| k.to_string())
        .collect()
}

/// 내장 자판 목록 + 현재값 → (정규 이름 Vec, 표시용 Vec, 선택 인덱스).
/// 현재값이 내장이 아니면(사용자 프로필) 목록 맨 앞에 보존한다.
fn build_layout_lists(
    current: &str,
    builtins: &[&str],
    disp: fn(&str) -> &'static str,
) -> (Vec<String>, Vec<SharedString>, i32) {
    let mut canon: Vec<String> = builtins.iter().map(|s| s.to_string()).collect();
    if !current.is_empty() && !canon.iter().any(|c| c == current) {
        canon.insert(0, current.to_string());
    }
    let display: Vec<SharedString> = canon
        .iter()
        .map(|c| {
            let d = disp(c);
            if d.is_empty() {
                SharedString::from(c.as_str())
            } else {
                SharedString::from(format!("{d} ({c})"))
            }
        })
        .collect();
    let index = canon.iter().position(|c| c == current).unwrap_or(0) as i32;
    (canon, display, index)
}

fn string_model(items: Vec<SharedString>) -> ModelRc<SharedString> {
    ModelRc::new(VecModel::from(items))
}

/// 레지스트리에서 프로필을 찾아 inherits까지 해석한다. 실패 시 `None`.
fn load_profile(name: &str) -> Option<LayoutProfile> {
    let reg = ProfileRegistry::new();
    let raw = reg.find_raw(name)?;
    resolve_inherits(&raw, &reg).ok()
}

/// 선택된 자판 프로필의 규칙 세트 → UI 항목 목록.
///
/// active 판정 (GTK settings_dialog 와 동일):
///   config.active_rule_sets Some(list) → 명시 override (빈 list = 모두 OFF)
///   None → 프로필의 active_rule_sets 또는 각 rule_set.active
fn rule_set_items_for(cfg: &Config, profile: &LayoutProfile) -> Vec<RuleSetItem> {
    let config_active = &cfg.engine.korean.active_rule_sets;
    let profile_default = profile.active_rule_sets.as_ref();
    profile
        .rule_sets
        .iter()
        .map(|(name, rs)| {
            let label = rs
                .description
                .as_ref()
                .map(|d| d.resolve("ko").to_string())
                .unwrap_or_default();
            let active = match config_active {
                Some(list) => list.contains(name),
                None => match profile_default {
                    Some(list) => list.contains(name),
                    None => rs.active,
                },
            };
            RuleSetItem {
                name: name.as_str().into(),
                label: label.into(),
                active,
            }
        })
        .collect()
}

/// 유효 활성 chord_window_ms 판정 — `Some(10..=200)` 만 "모아치기 켜짐".
/// `None`(미설정)·`Some(0)`(명시 OFF)·`Some(1..=9)`(무효)은 모두 꺼짐으로 본다.
fn moachigi_is_enabled(chord_window_ms: Option<u16>) -> bool {
    matches!(chord_window_ms, Some(n) if (10..=200).contains(&n))
}

/// 현재 자판 프로필의 moachigi capability + config chord_window 상태를 UI 에 반영.
/// - supported: 프로필이 `moachigi` capability 를 선언한 자판인가(안마태 등).
/// - enabled: chord_window_ms 가 유효 활성값인가.
/// - window: 슬라이더 표시값(비활성이면 권장 기본 60ms).
fn push_moachigi_to_ui(ui: &SettingsWindow, cfg: &Config, profile: Option<&LayoutProfile>) {
    let supported = profile.map(|p| p.moachigi.is_some()).unwrap_or(false);
    let cw = cfg.engine.korean.chord_window_ms;
    let enabled = moachigi_is_enabled(cw);
    ui.set_moachigi_supported(supported);
    ui.set_moachigi_enabled(enabled);
    ui.set_moachigi_window(if enabled { cw.unwrap() as f32 } else { 60.0 });
}

/// 선택된 한글 자판이 '세벌식 순아래'인지 — 접근성 추천 배지 노출 판정.
fn is_noshift_layout(cfg: &Config) -> bool {
    cfg.engine.korean.effective_layout_name() == KOREAN_LAYOUT_SEBEOLSIK_NOSHIFT
}

fn fmt_blacklist(tr: &Tr, e: &BlacklistEntry) -> String {
    let dir = match e.direction {
        Direction::Forward => tr.get_dir_forward(),
        Direction::Reverse => tr.get_dir_reverse(),
    };
    let st = match e.status {
        EntryStatus::Tentative => tr.get_st_tentative(),
        EntryStatus::Confirmed => tr.get_st_confirmed(),
        EntryStatus::Inactive => tr.get_st_inactive(),
    };
    tr.invoke_blacklist_entry(e.ascii.as_str().into(), dir, st, e.hit_count as i32)
        .to_string()
}

fn fmt_userdict(w: &ReverseWord) -> String {
    match &w.note {
        Some(n) if !n.is_empty() => format!("{} — {n}", w.word),
        _ => w.word.clone(),
    }
}

/// AutoTypeFix 설정 → UI 프로퍼티 일괄 반영. 초기 주입·프리셋 적용·기본값
/// 복원·되돌리기 네 경로에서 공통 사용(중복 제거, 값 동기화 누락 방지).
fn push_atf_to_ui(ui: &SettingsWindow, a: &AutoTypeFixConfig) {
    ui.set_atf_enabled(a.enabled);
    ui.set_atf_forward(a.forward);
    ui.set_atf_reverse(a.reverse);
    // 수치 항목은 Slider(값 라벨 병기) 와 양방향 바인딩하므로 float 로 노출한다.
    // Rust 는 저장 시 반올림해 정수 config 필드로 되돌린다(모아치기 슬라이더와 동일 패턴).
    ui.set_atf_kor_threshold(a.kor_syllable_threshold as f32);
    ui.set_atf_eng_min_length(a.eng_word_min_length as f32);
    ui.set_atf_forward_window(a.forward_time_window_ms as f32);
    ui.set_atf_reverse_window(a.reverse_time_window_ms as f32);
    ui.set_atf_tentative_expiry(a.tentative_expiry_hours as f32);
    ui.set_atf_observation_timeout(a.observation_timeout_secs as f32);
    ui.set_atf_skip_english_word(a.skip_on_english_word);
    ui.set_atf_skip_complete_syllable(a.skip_on_complete_syllable);
    ui.set_atf_rollback_detection(a.rollback_detection);
    ui.set_atf_user_dict_enabled(a.user_dict_enabled);
}

/// 오타 교정 강도 프리셋 (0=보수적, 1=표준, 2=적극적).
/// 인지부하가 큰 임계값·단어 길이·감지창만 일괄 세팅하고, 만료/관찰 타임아웃과
/// 불리언 옵션(사전·롤백 등)은 사용자가 고급에서 정한 값을 존중해 건드리지 않는다.
/// 세 프리셋은 단조 증가(보수적→적극적)로 교정 적극성이 커진다. 모든 값은
/// config clamp 허용 범위(kor 2~6, eng 3~8, window 500~5000) 안이다.
fn apply_atf_preset(a: &mut AutoTypeFixConfig, preset: i32) {
    let (kor, eng, window): (u8, u8, u32) = match preset {
        0 => (4, 7, 2000), // 보수적: 더 많은 근거를 모은 뒤 교정 → 오탐 최소
        2 => (2, 3, 5000), // 적극적: 짧은 입력·긴 감지창으로 더 자주 교정
        _ => (3, 5, 3500), // 표준(1): 균형
    };
    a.kor_syllable_threshold = kor;
    a.eng_word_min_length = eng;
    a.forward_time_window_ms = window;
    a.reverse_time_window_ms = window;
}

fn refresh_blacklist(ui: &SettingsWindow, bl: &Blacklist) {
    let tr = ui.global::<Tr>();
    let items: Vec<StandardListViewItem> = bl
        .entries
        .iter()
        .map(|e| StandardListViewItem::from(SharedString::from(fmt_blacklist(&tr, e))))
        .collect();
    ui.set_blacklist_model(ModelRc::new(VecModel::from(items)));
}

fn refresh_userdict(ui: &SettingsWindow, ud: &UserDictionary) {
    let items: Vec<StandardListViewItem> = ud
        .reverse_words
        .iter()
        .map(|w| StandardListViewItem::from(SharedString::from(fmt_userdict(w))))
        .collect();
    ui.set_userdict_model(ModelRc::new(VecModel::from(items)));
}

/// I6: 파괴적 삭제 직전 상태 스냅샷 1개. '되돌리기' 토스트에서 복원한다.
/// 최근 삭제 하나만 되돌릴 수 있도록 항상 최신 스냅샷으로 덮어쓴다.
enum DeleteSnapshot {
    Blacklist(Blacklist),
    Userdict(UserDictionary),
    /// 오타 교정 '기본값으로 복원' 직전의 AutoTypeFix 설정. 되돌리기 시 그대로
    /// 되돌려 사용자가 실수로 복원해도 이전 튜닝을 잃지 않게 한다(WCAG 3.3.4).
    AtfDefaults(AutoTypeFixConfig),
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 단일 인스턴스 가드: 이미 설정 창이 떠 있으면 그 창을 전면화하고 즉시 종료한다.
    // (중복 창이 뜨면 각자 저장해 마지막 창이 이전 변경을 덮어쓰는 문제를 차단.)
    if !acquire_singleton_or_foreground() {
        return Ok(());
    }

    // 렌더러 명시 선택: 기본 FemtoVG 는 힌팅이 없어 작은 한글(14px) 획 굵기가
    // 불균일하다. Skia 렌더러(Cargo.toml 의 renderer-skia feature 로 컴파일됨)는
    // 네이티브 글리프 래스터화로 힌팅이 적용돼 균일한 획을 낸다.
    // SLINT_BACKEND 형식은 "<backend>-<renderer>" → winit + skia.
    // 사용자/개발자가 이미 지정했다면(예: 디버깅용 winit-software) 존중한다.
    // SAFETY: SettingsWindow::new() 이전, 단일 스레드 진입부에서만 호출한다.
    if std::env::var("SLINT_BACKEND").is_err() {
        std::env::set_var("SLINT_BACKEND", "winit-skia");
    }

    let ui = SettingsWindow::new()?;

    // OS UI 언어 추종: 한국어면 원문(index 0), 그 외에는 영어 번들 번역 선택.
    // select_bundled_translation 은 첫 컴포넌트 생성 이후에 호출해야 한다.
    // 빈 문자열("")은 index 0(=소스 한국어)로 특수 처리된다.
    let _ = slint::select_bundled_translation(if ui_language_is_korean() { "" } else { "en" });

    // 설정 항목 검색: Slint 문자열엔 substring 매칭 내장이 없어(1.12: to-lowercase 만)
    // 매칭을 Rust 순수 콜백으로 위임한다. 각 설정 행이 제목·설명을 넘겨 호출한다.
    ui.global::<Search>().on_matches(|query, title, description| {
        let q = query.as_str().trim().to_lowercase();
        if q.is_empty() {
            return true;
        }
        title.as_str().to_lowercase().contains(&q)
            || description.as_str().to_lowercase().contains(&q)
    });

    let config = Rc::new(RefCell::new(Config::load_from_default_path()));
    let blacklist = Rc::new(RefCell::new(Blacklist::load_from_default_path()));
    let userdict = Rc::new(RefCell::new(UserDictionary::load_from_default_path()));
    // I6: 되돌리기용 스냅샷 1개(마지막 삭제만 복원 가능).
    let delete_snapshot: Rc<RefCell<Option<DeleteSnapshot>>> = Rc::new(RefCell::new(None));

    // 자판 목록 구성 (정규 이름은 저장 시 인덱스→이름 변환에 사용).
    let (kor_canon, kor_disp, kor_idx);
    let (eng_canon, eng_disp, eng_idx);
    {
        let cfg = config.borrow();
        (kor_canon, kor_disp, kor_idx) = build_layout_lists(
            &cfg.engine.korean.layout,
            KOREAN_LAYOUT_BUILTINS,
            korean_layout_display_name,
        );
        (eng_canon, eng_disp, eng_idx) = build_layout_lists(
            &cfg.engine.english.layout,
            ENGLISH_LAYOUT_BUILTINS,
            english_layout_display_name,
        );
    }
    let kor_canon = Rc::new(kor_canon);
    let eng_canon = Rc::new(eng_canon);

    // 키맵별 규칙 세트(자판 옵션) 모델 — 자판 변경 시 재구성.
    let rule_model: Rc<VecModel<RuleSetItem>> = Rc::new(VecModel::default());
    ui.set_rule_set_items(ModelRc::from(rule_model.clone()));
    {
        let cfg = config.borrow();
        if let Some(profile) = load_profile(&cfg.engine.korean.effective_layout_name()) {
            rule_model.set_vec(rule_set_items_for(&cfg, &profile));
        }
    }

    // ── 초기값 주입 ──
    {
        let cfg = config.borrow();
        let e = &cfg.engine;

        ui.set_korean_layouts(string_model(kor_disp));
        ui.set_korean_layout_index(kor_idx);
        ui.set_english_layouts(string_model(eng_disp));
        ui.set_english_layout_index(eng_idx);

        // 한글 확정 단위 (음절/단어/스마트) — CommitUnit::all() 순서 = 콤보 인덱스.
        ui.set_commit_unit_options(string_model(
            CommitUnit::all()
                .iter()
                .map(|u| SharedString::from(u.display_name()))
                .collect(),
        ));
        ui.set_commit_unit_index(
            CommitUnit::all()
                .iter()
                .position(|u| *u == e.korean.commit_unit)
                .unwrap_or(0) as i32,
        );

        // 시작 입력 모드: 0=영문, 1=한글 (DLL 다이얼로그와 동일 순서).
        let tr = ui.global::<Tr>();
        ui.set_category_options(string_model(vec![
            tr.get_cat_english(),
            tr.get_cat_korean(),
        ]));
        ui.set_category_index(match e.default_category {
            InputCategory::English => 0,
            InputCategory::Korean => 1,
        });

        // 모드 공유: 0=전역, 1=앱별.
        ui.set_mode_sharing_options(string_model(vec![
            tr.get_share_global(),
            tr.get_share_perapp(),
        ]));
        ui.set_mode_sharing_index(match e.mode_sharing {
            ModeSharingMode::Global => 0,
            ModeSharingMode::PerApp => 1,
        });

        ui.set_toggle_keys(e.toggle_keys.join(", ").into());
        ui.set_hanja_keys(e.hanja_keys.join(", ").into());
        // 단어 모드 앱 화이트리스트 (Smart 게이트 정확일치 대상, Windows 전용).
        ui.set_word_mode_apps(e.korean.word_mode_apps.join(", ").into());
        ui.set_auto_english_enabled(e.auto_english.enabled);
        ui.set_auto_english_keys(e.auto_english.trigger_keys.join(", ").into());
        // I7: 한/영 전환 비프 통지 (접근성).
        ui.set_toggle_announce_beep(e.toggle_announce_beep);
        // 조합키 자동반복 억제 (접근성, 지체장애).
        ui.set_ignore_key_repeat(e.ignore_key_repeat);

        push_atf_to_ui(&ui, &e.auto_typefix);

        // 접근성 추천 배지 + 모아치기 카드 초기 상태.
        ui.set_korean_noshift_selected(is_noshift_layout(&cfg));
        push_moachigi_to_ui(
            &ui,
            &cfg,
            load_profile(&cfg.engine.korean.effective_layout_name()).as_ref(),
        );
    }
    refresh_blacklist(&ui, &blacklist.borrow());
    refresh_userdict(&ui, &userdict.borrow());

    // ── 자동 저장 (config.yaml) — 컨트롤 변경 즉시 호출 ──
    {
        let ui_weak = ui.as_weak();
        let config = config.clone();
        let kor_canon = kor_canon.clone();
        let eng_canon = eng_canon.clone();
        let rule_model = rule_model.clone();
        ui.on_auto_save(move || {
            let ui = ui_weak.unwrap();
            let tr = ui.global::<Tr>();
            let mut cfg = config.borrow_mut();
            let e = &mut cfg.engine;

            let ki = (ui.get_korean_layout_index().max(0) as usize).min(kor_canon.len() - 1);
            // 자판 전환은 switch_layout 경유 — 이전 자판의 active_rule_sets 를
            // layout_rule_sets 캐시에 보존하고 새 자판의 캐시를 복원한다.
            let new_kor = &kor_canon[ki];
            let kor_changed = e.korean.effective_layout_name()
                != unim::config::normalize_korean_layout_name(new_kor);
            let new_profile = if kor_changed { load_profile(new_kor) } else { None };
            if kor_changed {
                let valid: Option<Vec<String>> = new_profile
                    .as_ref()
                    .map(|p| p.rule_sets.keys().cloned().collect());
                e.korean.switch_layout(new_kor, valid.as_deref());
            }
            let ei = (ui.get_english_layout_index().max(0) as usize).min(eng_canon.len() - 1);
            e.english.layout = eng_canon[ei].clone();

            // 한글 확정 단위 (음절/단어/스마트) — 콤보 인덱스 = CommitUnit::all() 순서.
            let cu_all = CommitUnit::all();
            let cu_idx = (ui.get_commit_unit_index().max(0) as usize).min(cu_all.len() - 1);
            e.korean.commit_unit = cu_all[cu_idx];

            e.default_category = if ui.get_category_index() == 1 {
                InputCategory::Korean
            } else {
                InputCategory::English
            };
            e.mode_sharing = if ui.get_mode_sharing_index() == 1 {
                ModeSharingMode::PerApp
            } else {
                ModeSharingMode::Global
            };

            e.toggle_keys = split_keys(&ui.get_toggle_keys());
            e.hanja_keys = split_keys(&ui.get_hanja_keys());
            // 단어 모드 앱 목록 (빈 목록도 유효 = Smart 게이트가 어떤 앱도 단어 모드 안 켬).
            e.korean.word_mode_apps = split_keys(&ui.get_word_mode_apps());
            e.auto_english.enabled = ui.get_auto_english_enabled();
            e.auto_english.trigger_keys = split_keys(&ui.get_auto_english_keys());
            // I7: 한/영 전환 비프 통지 (접근성).
            e.toggle_announce_beep = ui.get_toggle_announce_beep();
            // 조합키 자동반복 억제 (접근성, 지체장애).
            e.ignore_key_repeat = ui.get_ignore_key_repeat();

            let a = &mut e.auto_typefix;
            a.enabled = ui.get_atf_enabled();
            a.forward = ui.get_atf_forward();
            a.reverse = ui.get_atf_reverse();
            // Slider 는 float 값을 주므로 반올림 후 정수 범위로 clamp 한다.
            a.kor_syllable_threshold = (ui.get_atf_kor_threshold().round() as u8)
                .clamp(AUTO_TYPEFIX_KOR_THRESHOLD_MIN, AUTO_TYPEFIX_KOR_THRESHOLD_MAX);
            a.eng_word_min_length = (ui.get_atf_eng_min_length().round() as u8)
                .clamp(AUTO_TYPEFIX_ENG_MIN_LENGTH_MIN, AUTO_TYPEFIX_ENG_MIN_LENGTH_MAX);
            a.forward_time_window_ms = (ui.get_atf_forward_window().round() as u32)
                .clamp(AUTO_TYPEFIX_TIME_WINDOW_MIN, AUTO_TYPEFIX_TIME_WINDOW_MAX);
            a.reverse_time_window_ms = (ui.get_atf_reverse_window().round() as u32)
                .clamp(AUTO_TYPEFIX_TIME_WINDOW_MIN, AUTO_TYPEFIX_TIME_WINDOW_MAX);
            a.tentative_expiry_hours = (ui.get_atf_tentative_expiry().round() as u16)
                .clamp(AUTO_TYPEFIX_TENTATIVE_EXPIRY_MIN, AUTO_TYPEFIX_TENTATIVE_EXPIRY_MAX);
            a.observation_timeout_secs = (ui.get_atf_observation_timeout().round() as u8).clamp(
                AUTO_TYPEFIX_OBSERVATION_TIMEOUT_MIN,
                AUTO_TYPEFIX_OBSERVATION_TIMEOUT_MAX,
            );
            a.skip_on_english_word = ui.get_atf_skip_english_word();
            a.skip_on_complete_syllable = ui.get_atf_skip_complete_syllable();
            a.rollback_detection = ui.get_atf_rollback_detection();
            a.user_dict_enabled = ui.get_atf_user_dict_enabled();

            // 자판이 바뀌었으면 규칙 세트 그룹 + 접근성 배지 + 모아치기 카드를
            // 새 프로필 기준으로 재구성 (moachigi-supported 는 자판마다 다름).
            if kor_changed {
                if let Some(profile) = new_profile.as_ref() {
                    rule_model.set_vec(rule_set_items_for(&cfg, profile));
                }
                ui.set_korean_noshift_selected(is_noshift_layout(&cfg));
                push_moachigi_to_ui(&ui, &cfg, new_profile.as_ref());
            }

            match cfg.save_to_default_path() {
                Ok(()) => ui.set_status_text(tr.get_applied()),
                Err(err) => ui.set_status_text(tr.invoke_save_failed(err.to_string().into())),
            }
        });
    }

    // ── 규칙 세트(자판 옵션) 토글 ──
    {
        let ui_weak = ui.as_weak();
        let config = config.clone();
        let rule_model = rule_model.clone();
        ui.on_rule_set_toggled(move |idx, on| {
            use slint::Model;
            let ui = ui_weak.unwrap();
            let tr = ui.global::<Tr>();
            let Some(mut item) = rule_model.row_data(idx as usize) else {
                return;
            };
            let mut cfg = config.borrow_mut();
            {
                // 첫 토글 시 None → Some(현재 표시 중 활성 집합)으로 고정 —
                // 이후 모든 토글이 명시 override 로 저장된다 (GTK 와 동일 의미).
                let seed: Vec<String> = rule_model
                    .iter()
                    .filter(|it| it.active)
                    .map(|it| it.name.to_string())
                    .collect();
                let list = cfg
                    .engine
                    .korean
                    .active_rule_sets
                    .get_or_insert_with(|| seed);
                let name = item.name.to_string();
                if on {
                    if !list.contains(&name) {
                        list.push(name);
                    }
                } else {
                    list.retain(|x| x != &name);
                }
            }
            // 현재 자판의 캐시도 동기화 — 자판 전환 시 본 상태가 보존된다.
            cfg.engine.korean.cache_active_rule_sets();
            match cfg.save_to_default_path() {
                Ok(()) => ui.set_status_text(tr.get_applied()),
                Err(err) => ui.set_status_text(tr.invoke_save_failed(err.to_string().into())),
            }
            item.active = on;
            rule_model.set_row_data(idx as usize, item);
        });
    }

    // ── 오타 교정 강도 프리셋 (보수적/표준/적극적) 일괄 적용 ──
    {
        let ui_weak = ui.as_weak();
        let config = config.clone();
        ui.on_atf_apply_preset(move |preset| {
            let ui = ui_weak.unwrap();
            let tr = ui.global::<Tr>();
            let mut cfg = config.borrow_mut();
            apply_atf_preset(&mut cfg.engine.auto_typefix, preset);
            push_atf_to_ui(&ui, &cfg.engine.auto_typefix);
            let name = match preset {
                0 => tr.get_strength_conservative(),
                2 => tr.get_strength_aggressive(),
                _ => tr.get_strength_standard(),
            };
            match cfg.save_to_default_path() {
                Ok(()) => ui.set_status_text(tr.invoke_strength_applied(name)),
                Err(err) => ui.set_status_text(tr.invoke_save_failed(err.to_string().into())),
            }
        });
    }

    // ── 오타 교정 기본값으로 복원 (+ 5초 되돌리기 토스트, WCAG 3.3.4) ──
    {
        let ui_weak = ui.as_weak();
        let config = config.clone();
        let delete_snapshot = delete_snapshot.clone();
        ui.on_atf_restore_defaults(move || {
            let ui = ui_weak.unwrap();
            let tr = ui.global::<Tr>();
            let mut cfg = config.borrow_mut();
            // 복원 직전 값을 스냅샷 → 되돌리기로 원상 복구 가능.
            *delete_snapshot.borrow_mut() =
                Some(DeleteSnapshot::AtfDefaults(cfg.engine.auto_typefix.clone()));
            cfg.engine.auto_typefix = AutoTypeFixConfig::default();
            push_atf_to_ui(&ui, &cfg.engine.auto_typefix);
            match cfg.save_to_default_path() {
                Ok(()) => ui.set_status_text(tr.get_atf_restored()),
                Err(err) => ui.set_status_text(tr.invoke_save_failed(err.to_string().into())),
            }
            ui.set_toast_message(tr.get_atf_restored());
            ui.set_toast_visible(true);
        });
    }

    // ── 모아치기 사용 스위치 → chord_window_ms Some(60)/None 전환 ──
    {
        let ui_weak = ui.as_weak();
        let config = config.clone();
        ui.on_moachigi_toggled(move |on| {
            let ui = ui_weak.unwrap();
            let tr = ui.global::<Tr>();
            let mut cfg = config.borrow_mut();
            if on {
                // 슬라이더 표시값(또는 권장 60ms)을 반올림·유효 범위로 clamp 해 활성화.
                let w = (ui.get_moachigi_window().round() as u16).clamp(10, 200);
                cfg.engine.korean.chord_window_ms = Some(w);
            } else {
                cfg.engine.korean.chord_window_ms = None;
            }
            let prof = load_profile(&cfg.engine.korean.effective_layout_name());
            push_moachigi_to_ui(&ui, &cfg, prof.as_ref());
            let msg = if on { tr.get_moachigi_on() } else { tr.get_moachigi_off() };
            match cfg.save_to_default_path() {
                Ok(()) => ui.set_status_text(msg),
                Err(err) => ui.set_status_text(tr.invoke_save_failed(err.to_string().into())),
            }
        });
    }

    // ── 모아치기 조합창 슬라이더(released) → chord_window_ms 갱신 ──
    {
        let ui_weak = ui.as_weak();
        let config = config.clone();
        ui.on_moachigi_window_released(move |v| {
            let ui = ui_weak.unwrap();
            let tr = ui.global::<Tr>();
            let mut cfg = config.borrow_mut();
            // 슬라이더는 활성 상태에서만 노출 — 켜져 있을 때만 값을 반영한다.
            if moachigi_is_enabled(cfg.engine.korean.chord_window_ms) {
                let w = (v.max(0.0).round() as u16).clamp(10, 200);
                cfg.engine.korean.chord_window_ms = Some(w);
                ui.set_moachigi_window(w as f32);
                match cfg.save_to_default_path() {
                    Ok(()) => {
                        ui.set_status_text(tr.invoke_moachigi_window_set(w as i32))
                    }
                    Err(err) => ui.set_status_text(tr.invoke_save_failed(err.to_string().into())),
                }
            }
        });
    }

    // ── 접근성 프리셋(0='한 손 사용', 1='넉넉한 타이밍') 일괄 적용 ──
    {
        let ui_weak = ui.as_weak();
        let config = config.clone();
        let kor_canon = kor_canon.clone();
        let rule_model = rule_model.clone();
        ui.on_apply_accessibility_preset(move |preset| {
            let ui = ui_weak.unwrap();
            let tr = ui.global::<Tr>();
            let mut cfg = config.borrow_mut();
            let status: SharedString;
            if preset == 0 {
                // 한 손 사용: 순아래 자판 + 비수정자 토글 + 모아치기 OFF + 자동반복 억제.
                let new_kor = KOREAN_LAYOUT_SEBEOLSIK_NOSHIFT;
                let changed = cfg.engine.korean.effective_layout_name() != new_kor;
                let new_profile = load_profile(new_kor);
                if changed {
                    let valid: Option<Vec<String>> = new_profile
                        .as_ref()
                        .map(|p| p.rule_sets.keys().cloned().collect());
                    cfg.engine.korean.switch_layout(new_kor, valid.as_deref());
                }
                // 비수정자 토글(한글키·오른쪽 Alt) — 한 손으로 누를 수 있는 기본값.
                cfg.engine.toggle_keys = vec!["Korean".to_string(), "RightAlt".to_string()];
                cfg.engine.korean.chord_window_ms = None; // 모아치기 OFF
                cfg.engine.ignore_key_repeat = true;

                // UI 재동기화.
                if let Some(pos) = kor_canon
                    .iter()
                    .position(|c| normalize_korean_layout_name(c) == new_kor)
                {
                    ui.set_korean_layout_index(pos as i32);
                }
                if let Some(profile) = new_profile.as_ref() {
                    rule_model.set_vec(rule_set_items_for(&cfg, profile));
                }
                ui.set_toggle_keys(cfg.engine.toggle_keys.join(", ").into());
                ui.set_ignore_key_repeat(true);
                ui.set_korean_noshift_selected(is_noshift_layout(&cfg));
                push_moachigi_to_ui(&ui, &cfg, new_profile.as_ref());
                status = tr.get_preset_onehand();
            } else {
                // 넉넉한 타이밍: 자동반복 억제 + 오타 교정 판정 시간 확대 +
                // (지원 자판) 모아치기 조합창을 넉넉하게.
                cfg.engine.ignore_key_repeat = true;
                let a = &mut cfg.engine.auto_typefix;
                a.forward_time_window_ms = AUTO_TYPEFIX_TIME_WINDOW_MAX;
                a.reverse_time_window_ms = AUTO_TYPEFIX_TIME_WINDOW_MAX;
                a.observation_timeout_secs = AUTO_TYPEFIX_OBSERVATION_TIMEOUT_MAX;

                let prof = load_profile(&cfg.engine.korean.effective_layout_name());
                if prof.as_ref().map(|p| p.moachigi.is_some()).unwrap_or(false) {
                    // 지원 자판이면 조합창을 넉넉히(150ms) — 천천히 눌러도 모아짐.
                    cfg.engine.korean.chord_window_ms = Some(150);
                }

                ui.set_ignore_key_repeat(true);
                push_atf_to_ui(&ui, &cfg.engine.auto_typefix);
                push_moachigi_to_ui(&ui, &cfg, prof.as_ref());
                status = tr.get_preset_relaxed();
            }
            match cfg.save_to_default_path() {
                Ok(()) => ui.set_status_text(status),
                Err(err) => ui.set_status_text(tr.invoke_save_failed(err.to_string().into())),
            }
        });
    }

    // ── 닫기 ──
    {
        let ui_weak = ui.as_weak();
        ui.on_close_clicked(move || {
            let _ = ui_weak.unwrap().window().hide();
        });
    }

    // ── 억제 단어 삭제 (개별) — 삭제 직전 스냅샷 후 되돌리기 토스트 노출 ──
    {
        let ui_weak = ui.as_weak();
        let blacklist = blacklist.clone();
        let delete_snapshot = delete_snapshot.clone();
        ui.on_blacklist_remove(move |idx| {
            let ui = ui_weak.unwrap();
            let tr = ui.global::<Tr>();
            let mut bl = blacklist.borrow_mut();
            if idx >= 0 && (idx as usize) < bl.entries.len() {
                // I6: 삭제 직전 전체 목록을 스냅샷(복원 시 그대로 되돌림).
                *delete_snapshot.borrow_mut() = Some(DeleteSnapshot::Blacklist(bl.clone()));
                bl.remove(idx as usize);
                let _ = bl.save_to_default_path();
                refresh_blacklist(&ui, &bl);
                ui.set_status_text(tr.get_bl_removed());
                ui.set_toast_message(tr.get_bl_removed());
                ui.set_toast_visible(true);
            }
        });
    }
    // ── 억제 단어 모두 삭제 (확인 다이얼로그 통과 후 호출) ──
    {
        let ui_weak = ui.as_weak();
        let blacklist = blacklist.clone();
        let delete_snapshot = delete_snapshot.clone();
        ui.on_blacklist_clear(move || {
            let ui = ui_weak.unwrap();
            let tr = ui.global::<Tr>();
            let mut bl = blacklist.borrow_mut();
            if bl.entries.is_empty() {
                return;
            }
            // I6: 삭제 직전 전체 목록을 스냅샷.
            *delete_snapshot.borrow_mut() = Some(DeleteSnapshot::Blacklist(bl.clone()));
            bl.entries.clear();
            let _ = bl.save_to_default_path();
            refresh_blacklist(&ui, &bl);
            ui.set_status_text(tr.get_bl_cleared());
            ui.set_toast_message(tr.get_bl_cleared());
            ui.set_toast_visible(true);
        });
    }

    // ── 사용자 사전 추가 ──
    {
        let ui_weak = ui.as_weak();
        let userdict = userdict.clone();
        ui.on_userdict_add(move || {
            let ui = ui_weak.unwrap();
            let tr = ui.global::<Tr>();
            let word = ui.get_userdict_new_word().to_string();
            let word = word.trim();
            if word.is_empty() {
                return;
            }
            let note = ui.get_userdict_new_note().to_string();
            let note = note.trim();
            let note = if note.is_empty() {
                None
            } else {
                Some(note.to_string())
            };
            let mut ud = userdict.borrow_mut();
            if ud.add(word, note) {
                let _ = ud.save_to_default_path();
                refresh_userdict(&ui, &ud);
                ui.set_userdict_new_word("".into());
                ui.set_userdict_new_note("".into());
                ui.set_status_text(tr.get_ud_added());
            } else {
                ui.set_status_text(tr.get_ud_dup());
            }
        });
    }

    // ── 사용자 사전 삭제 (개별) — 삭제 직전 스냅샷 후 되돌리기 토스트 노출 ──
    {
        let ui_weak = ui.as_weak();
        let userdict = userdict.clone();
        let delete_snapshot = delete_snapshot.clone();
        ui.on_userdict_remove(move |idx| {
            let ui = ui_weak.unwrap();
            let tr = ui.global::<Tr>();
            let mut ud = userdict.borrow_mut();
            if idx < 0 || (idx as usize) >= ud.reverse_words.len() {
                return;
            }
            // I6: 삭제 직전 전체 목록을 스냅샷(제거 성공이 보장되는 시점).
            *delete_snapshot.borrow_mut() = Some(DeleteSnapshot::Userdict(ud.clone()));
            if ud.remove_at(idx as usize) {
                let _ = ud.save_to_default_path();
                refresh_userdict(&ui, &ud);
                ui.set_status_text(tr.get_ud_removed());
                ui.set_toast_message(tr.get_ud_removed());
                ui.set_toast_visible(true);
            }
        });
    }

    // ── I6: 되돌리기(undo) — 마지막 삭제 스냅샷 복원 ──
    {
        let ui_weak = ui.as_weak();
        let blacklist = blacklist.clone();
        let userdict = userdict.clone();
        let config = config.clone();
        let delete_snapshot = delete_snapshot.clone();
        ui.on_undo_restore(move || {
            let ui = ui_weak.unwrap();
            let tr = ui.global::<Tr>();
            match delete_snapshot.borrow_mut().take() {
                Some(DeleteSnapshot::AtfDefaults(snap)) => {
                    let mut cfg = config.borrow_mut();
                    cfg.engine.auto_typefix = snap;
                    push_atf_to_ui(&ui, &cfg.engine.auto_typefix);
                    let _ = cfg.save_to_default_path();
                    ui.set_status_text(tr.get_atf_undone());
                }
                Some(DeleteSnapshot::Blacklist(snap)) => {
                    let mut bl = blacklist.borrow_mut();
                    *bl = snap;
                    let _ = bl.save_to_default_path();
                    refresh_blacklist(&ui, &bl);
                    ui.set_status_text(tr.get_delete_undone());
                }
                Some(DeleteSnapshot::Userdict(snap)) => {
                    let mut ud = userdict.borrow_mut();
                    *ud = snap;
                    let _ = ud.save_to_default_path();
                    refresh_userdict(&ui, &ud);
                    ui.set_status_text(tr.get_delete_undone());
                }
                None => {}
            }
            ui.set_toast_visible(false);
        });
    }

    ui.run()?;
    Ok(())
}

