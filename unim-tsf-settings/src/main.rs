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
    english_layout_display_name, korean_layout_display_name, Config, InputCategory,
    ModeSharingMode, ENGLISH_LAYOUT_BUILTINS, KOREAN_LAYOUT_BUILTINS,
    AUTO_TYPEFIX_ENG_MIN_LENGTH_MAX, AUTO_TYPEFIX_ENG_MIN_LENGTH_MIN,
    AUTO_TYPEFIX_KOR_THRESHOLD_MAX, AUTO_TYPEFIX_KOR_THRESHOLD_MIN,
    AUTO_TYPEFIX_OBSERVATION_TIMEOUT_MAX, AUTO_TYPEFIX_OBSERVATION_TIMEOUT_MIN,
    AUTO_TYPEFIX_TENTATIVE_EXPIRY_MAX, AUTO_TYPEFIX_TENTATIVE_EXPIRY_MIN,
    AUTO_TYPEFIX_TIME_WINDOW_MAX, AUTO_TYPEFIX_TIME_WINDOW_MIN,
};
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

    // ── 초기값 주입 ──
    {
        let cfg = config.borrow();
        let e = &cfg.engine;

        ui.set_korean_layouts(string_model(kor_disp));
        ui.set_korean_layout_index(kor_idx);
        ui.set_english_layouts(string_model(eng_disp));
        ui.set_english_layout_index(eng_idx);

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
        ui.set_auto_english_enabled(e.auto_english.enabled);
        ui.set_auto_english_keys(e.auto_english.trigger_keys.join(", ").into());

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
        ui.on_auto_save(move || {
            let ui = ui_weak.unwrap();
            let mut cfg = config.borrow_mut();
            let e = &mut cfg.engine;

            let ki = (ui.get_korean_layout_index().max(0) as usize).min(kor_canon.len() - 1);
            e.korean.layout = kor_canon[ki].clone();
            let ei = (ui.get_english_layout_index().max(0) as usize).min(eng_canon.len() - 1);
            e.english.layout = eng_canon[ei].clone();

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
            e.auto_english.enabled = ui.get_auto_english_enabled();
            e.auto_english.trigger_keys = split_keys(&ui.get_auto_english_keys());

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

            match cfg.save_to_default_path() {
                Ok(()) => ui.set_status_text("변경 사항이 적용되었습니다.".into()),
                Err(err) => ui.set_status_text(format!("저장 실패: {err}").into()),
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

    // ── 억제 단어 삭제 ──
    {
        let ui_weak = ui.as_weak();
        let blacklist = blacklist.clone();
        ui.on_blacklist_remove(move |idx| {
            let ui = ui_weak.unwrap();
            let mut bl = blacklist.borrow_mut();
            if idx >= 0 && (idx as usize) < bl.entries.len() {
                bl.remove(idx as usize);
                let _ = bl.save_to_default_path();
                refresh_blacklist(&ui, &bl);
                ui.set_status_text("억제 단어를 삭제했습니다.".into());
            }
        });
    }
    {
        let ui_weak = ui.as_weak();
        let blacklist = blacklist.clone();
        ui.on_blacklist_clear(move || {
            let ui = ui_weak.unwrap();
            let mut bl = blacklist.borrow_mut();
            bl.entries.clear();
            let _ = bl.save_to_default_path();
            refresh_blacklist(&ui, &bl);
            ui.set_status_text("억제 단어를 모두 삭제했습니다.".into());
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

    // ── 사용자 사전 삭제 ──
    {
        let ui_weak = ui.as_weak();
        let userdict = userdict.clone();
        ui.on_userdict_remove(move |idx| {
            let ui = ui_weak.unwrap();
            let mut ud = userdict.borrow_mut();
            if idx >= 0 && ud.remove_at(idx as usize) {
                let _ = ud.save_to_default_path();
                refresh_userdict(&ui, &ud);
                ui.set_status_text("사용자 사전에서 삭제했습니다.".into());
            }
        });
    }

    ui.run()?;
    Ok(())
}

