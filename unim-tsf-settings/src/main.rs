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
    english_layout_display_name, korean_layout_display_name, CommitUnit, Config, InputCategory,
    ModeSharingMode, ENGLISH_LAYOUT_BUILTINS, KOREAN_LAYOUT_BUILTINS,
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

fn fmt_blacklist(e: &BlacklistEntry) -> String {
    let dir = match e.direction {
        Direction::Forward => "정방향",
        Direction::Reverse => "역방향",
    };
    let st = match e.status {
        EntryStatus::Tentative => "임시",
        EntryStatus::Confirmed => "확정",
        EntryStatus::Inactive => "비활성",
    };
    format!("{}    [{dir} · {st}]    감지 {}회", e.ascii, e.hit_count)
}

fn fmt_userdict(w: &ReverseWord) -> String {
    match &w.note {
        Some(n) if !n.is_empty() => format!("{} — {n}", w.word),
        _ => w.word.clone(),
    }
}

fn refresh_blacklist(ui: &SettingsWindow, bl: &Blacklist) {
    let items: Vec<StandardListViewItem> = bl
        .entries
        .iter()
        .map(|e| StandardListViewItem::from(SharedString::from(fmt_blacklist(e))))
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
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
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
        ui.set_category_options(string_model(vec![
            "영문으로 시작".into(),
            "한글로 시작".into(),
        ]));
        ui.set_category_index(match e.default_category {
            InputCategory::English => 0,
            InputCategory::Korean => 1,
        });

        // 모드 공유: 0=전역, 1=앱별.
        ui.set_mode_sharing_options(string_model(vec![
            "전역 공유".into(),
            "앱별 분리".into(),
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

        let a = &e.auto_typefix;
        ui.set_atf_enabled(a.enabled);
        ui.set_atf_forward(a.forward);
        ui.set_atf_reverse(a.reverse);
        ui.set_atf_kor_threshold(a.kor_syllable_threshold as i32);
        ui.set_atf_eng_min_length(a.eng_word_min_length as i32);
        ui.set_atf_forward_window(a.forward_time_window_ms as i32);
        ui.set_atf_reverse_window(a.reverse_time_window_ms as i32);
        ui.set_atf_tentative_expiry(a.tentative_expiry_hours as i32);
        ui.set_atf_observation_timeout(a.observation_timeout_secs as i32);
        ui.set_atf_skip_english_word(a.skip_on_english_word);
        ui.set_atf_skip_complete_syllable(a.skip_on_complete_syllable);
        ui.set_atf_rollback_detection(a.rollback_detection);
        ui.set_atf_user_dict_enabled(a.user_dict_enabled);
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

            let a = &mut e.auto_typefix;
            a.enabled = ui.get_atf_enabled();
            a.forward = ui.get_atf_forward();
            a.reverse = ui.get_atf_reverse();
            a.kor_syllable_threshold = (ui.get_atf_kor_threshold() as u8)
                .clamp(AUTO_TYPEFIX_KOR_THRESHOLD_MIN, AUTO_TYPEFIX_KOR_THRESHOLD_MAX);
            a.eng_word_min_length = (ui.get_atf_eng_min_length() as u8)
                .clamp(AUTO_TYPEFIX_ENG_MIN_LENGTH_MIN, AUTO_TYPEFIX_ENG_MIN_LENGTH_MAX);
            a.forward_time_window_ms = (ui.get_atf_forward_window() as u32)
                .clamp(AUTO_TYPEFIX_TIME_WINDOW_MIN, AUTO_TYPEFIX_TIME_WINDOW_MAX);
            a.reverse_time_window_ms = (ui.get_atf_reverse_window() as u32)
                .clamp(AUTO_TYPEFIX_TIME_WINDOW_MIN, AUTO_TYPEFIX_TIME_WINDOW_MAX);
            a.tentative_expiry_hours = (ui.get_atf_tentative_expiry() as u16)
                .clamp(AUTO_TYPEFIX_TENTATIVE_EXPIRY_MIN, AUTO_TYPEFIX_TENTATIVE_EXPIRY_MAX);
            a.observation_timeout_secs = (ui.get_atf_observation_timeout() as u8).clamp(
                AUTO_TYPEFIX_OBSERVATION_TIMEOUT_MIN,
                AUTO_TYPEFIX_OBSERVATION_TIMEOUT_MAX,
            );
            a.skip_on_english_word = ui.get_atf_skip_english_word();
            a.skip_on_complete_syllable = ui.get_atf_skip_complete_syllable();
            a.rollback_detection = ui.get_atf_rollback_detection();
            a.user_dict_enabled = ui.get_atf_user_dict_enabled();

            // 자판이 바뀌었으면 규칙 세트 그룹을 새 프로필로 재구성.
            if let Some(profile) = new_profile.as_ref() {
                rule_model.set_vec(rule_set_items_for(&cfg, profile));
            }

            match cfg.save_to_default_path() {
                Ok(()) => ui.set_status_text("변경 사항이 적용되었습니다.".into()),
                Err(err) => ui.set_status_text(format!("저장 실패: {err}").into()),
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
                Ok(()) => ui.set_status_text("변경 사항이 적용되었습니다.".into()),
                Err(err) => ui.set_status_text(format!("저장 실패: {err}").into()),
            }
            item.active = on;
            rule_model.set_row_data(idx as usize, item);
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
            let mut bl = blacklist.borrow_mut();
            if idx >= 0 && (idx as usize) < bl.entries.len() {
                // I6: 삭제 직전 전체 목록을 스냅샷(복원 시 그대로 되돌림).
                *delete_snapshot.borrow_mut() = Some(DeleteSnapshot::Blacklist(bl.clone()));
                bl.remove(idx as usize);
                let _ = bl.save_to_default_path();
                refresh_blacklist(&ui, &bl);
                ui.set_status_text("억제 단어를 삭제했습니다.".into());
                ui.set_toast_message("억제 단어를 삭제했습니다.".into());
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
            let mut bl = blacklist.borrow_mut();
            if bl.entries.is_empty() {
                return;
            }
            // I6: 삭제 직전 전체 목록을 스냅샷.
            *delete_snapshot.borrow_mut() = Some(DeleteSnapshot::Blacklist(bl.clone()));
            bl.entries.clear();
            let _ = bl.save_to_default_path();
            refresh_blacklist(&ui, &bl);
            ui.set_status_text("억제 단어를 모두 삭제했습니다.".into());
            ui.set_toast_message("억제 단어를 모두 삭제했습니다.".into());
            ui.set_toast_visible(true);
        });
    }

    // ── 사용자 사전 추가 ──
    {
        let ui_weak = ui.as_weak();
        let userdict = userdict.clone();
        ui.on_userdict_add(move || {
            let ui = ui_weak.unwrap();
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
                ui.set_status_text("사용자 사전에 추가했습니다.".into());
            } else {
                ui.set_status_text("이미 등록된 단어입니다.".into());
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
            let mut ud = userdict.borrow_mut();
            if idx < 0 || (idx as usize) >= ud.reverse_words.len() {
                return;
            }
            // I6: 삭제 직전 전체 목록을 스냅샷(제거 성공이 보장되는 시점).
            *delete_snapshot.borrow_mut() = Some(DeleteSnapshot::Userdict(ud.clone()));
            if ud.remove_at(idx as usize) {
                let _ = ud.save_to_default_path();
                refresh_userdict(&ui, &ud);
                ui.set_status_text("사용자 사전에서 삭제했습니다.".into());
                ui.set_toast_message("사용자 사전에서 삭제했습니다.".into());
                ui.set_toast_visible(true);
            }
        });
    }

    // ── I6: 되돌리기(undo) — 마지막 삭제 스냅샷 복원 ──
    {
        let ui_weak = ui.as_weak();
        let blacklist = blacklist.clone();
        let userdict = userdict.clone();
        let delete_snapshot = delete_snapshot.clone();
        ui.on_undo_restore(move || {
            let ui = ui_weak.unwrap();
            match delete_snapshot.borrow_mut().take() {
                Some(DeleteSnapshot::Blacklist(snap)) => {
                    let mut bl = blacklist.borrow_mut();
                    *bl = snap;
                    let _ = bl.save_to_default_path();
                    refresh_blacklist(&ui, &bl);
                    ui.set_status_text("삭제를 되돌렸습니다.".into());
                }
                Some(DeleteSnapshot::Userdict(snap)) => {
                    let mut ud = userdict.borrow_mut();
                    *ud = snap;
                    let _ = ud.save_to_default_path();
                    refresh_userdict(&ui, &ud);
                    ui.set_status_text("삭제를 되돌렸습니다.".into());
                }
                None => {}
            }
            ui.set_toast_visible(false);
        });
    }

    ui.run()?;
    Ok(())
}

