//! UNIM 설정 GUI (Slint, 크로스플랫폼 — Windows/Linux).
//!
//! 기원: Windows 에서 DLL 내부 Win32 모달 다이얼로그(`unim-tsf/src/settings_dialog.rs`)의
//! 호스트 메시지 펌프 충돌 문제를 피하기 위해 설정 UI 를 별도 프로세스로 분리했고
//! (구 unim-tsf-settings), platform/ 백엔드 분리로 Linux 에서도 동일 UI 를 쓴다.
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
    english_layout_display_name, normalize_korean_layout_name,
    AppRule, AutoTypeFixConfig, CommitUnit, Config, InputCategory, ModeSharingMode,
    ENGLISH_LAYOUT_BUILTINS, KOREAN_LAYOUT_SEBEOLSIK_NOSHIFT,
    AUTO_TYPEFIX_ENG_MIN_LENGTH_MAX, AUTO_TYPEFIX_ENG_MIN_LENGTH_MIN,
    AUTO_TYPEFIX_KOR_THRESHOLD_MAX, AUTO_TYPEFIX_KOR_THRESHOLD_MIN,
    AUTO_TYPEFIX_OBSERVATION_TIMEOUT_MAX, AUTO_TYPEFIX_OBSERVATION_TIMEOUT_MIN,
    AUTO_TYPEFIX_TENTATIVE_EXPIRY_MAX, AUTO_TYPEFIX_TENTATIVE_EXPIRY_MIN,
    AUTO_TYPEFIX_TIME_WINDOW_MAX, AUTO_TYPEFIX_TIME_WINDOW_MIN,
};
use unim::input_engine::InputEngine;
use unim::keystroke::profile::{resolve_inherits, LayoutProfile, ProfileRegistry};
use unim::typefix_blacklist::{Blacklist, BlacklistEntry, Direction, EntryStatus};
use unim::typefix_userdict::{ReverseWord, UserDictionary};

slint::include_modules!();

mod platform;
mod wizard;

/// "a, b, c" → ["a","b","c"] (공백 trim, 빈 항목 제거).
fn split_keys(s: &str) -> Vec<String> {
    s.split(',')
        .map(|k| k.trim())
        .filter(|k| !k.is_empty())
        .map(|k| k.to_string())
        .collect()
}

/// 키 성격 설정 필드 4군의 무효 표기 수집 결과.
///
/// 판정은 전부 엔진 파서(단일 진실 공급원)에 위임한다 — 설정앱은 파서 사본을 두지
/// 않는다. 필드마다 **엔진이 실제로 그 필드를 파싱하는 함수**와 1:1 로 맞춰야 한다.
/// 예컨대 toggle/hanja 에 `parse_atf_hotkey` 를 쓰면 수정자 base 배제 규칙 때문에
/// 출하 기본값 `RightAlt` 가 무효로 오탐된다.
///
/// 플랫폼 중립 — 이 판정에 `#[cfg]` 가 끼어들면 Linux/Windows 경고가 갈라진다.
#[derive(Debug, Default, PartialEq, Eq)]
struct InvalidKeys {
    toggle: Vec<String>,
    hanja: Vec<String>,
    auto_english: Vec<String>,
    /// ATF 토글 3필드(전체·정방향·역방향) 합산 — 종전 단일 경고 문구를 유지한다.
    atf: Vec<String>,
}

impl InvalidKeys {
    fn is_empty(&self) -> bool {
        self.toggle.is_empty()
            && self.hanja.is_empty()
            && self.auto_english.is_empty()
            && self.atf.is_empty()
    }

    /// 비어 있지 않은 그룹 수 — 상태줄 단일/복수 문구 분기에 쓴다.
    fn group_count(&self) -> usize {
        [&self.toggle, &self.hanja, &self.auto_english, &self.atf]
            .iter()
            .filter(|g| !g.is_empty())
            .count()
    }
}

/// Config 의 키 성격 필드를 엔진 파서로 검증한다. 저장 경로(auto_save·접근성 프리셋)가
/// 공유하는 단일 진입점 — 어느 경로로 저장하든 같은 판정이 나오게 한다.
///
/// 정책은 "저장은 하되 경고" — 여기서 무효를 찾아도 저장은 그대로 진행한다.
/// 무효 표기는 엔진이 조용히 버리므로(키가 죽음) 사용자에게 알리는 것이 목적이다.
pub(crate) fn collect_invalid_keys(cfg: &Config) -> InvalidKeys {
    let e = &cfg.engine;
    InvalidKeys {
        toggle: e
            .toggle_keys
            .iter()
            .filter(|k| InputEngine::parse_switch_key(k).is_none())
            .cloned()
            .collect(),
        hanja: e
            .hanja_keys
            .iter()
            .filter(|k| InputEngine::parse_switch_key(k).is_none())
            .cloned()
            .collect(),
        auto_english: e
            .auto_english
            .trigger_keys
            .iter()
            .filter(|k| !InputEngine::is_valid_auto_english_key(k))
            .cloned()
            .collect(),
        atf: e
            .auto_typefix
            .toggle_enabled_keys
            .iter()
            .chain(&e.auto_typefix.toggle_forward_keys)
            .chain(&e.auto_typefix.toggle_reverse_keys)
            .filter(|k| InputEngine::parse_atf_hotkey(k).is_none())
            .cloned()
            .collect(),
    }
}

/// 2그룹 이상 동시 무효일 때 상태줄에 넣을 `필드명: 키들 / 필드명: 키들` 조각.
///
/// `labels` 순서는 `InvalidKeys` 필드 순서(토글·한자·자동 영문·오타 교정)와 같다.
/// Slint 를 타지 않는 순수 함수 — 단위 테스트 대상.
fn invalid_keys_detail(inv: &InvalidKeys, labels: [&str; 4]) -> String {
    [
        (labels[0], &inv.toggle),
        (labels[1], &inv.hanja),
        (labels[2], &inv.auto_english),
        (labels[3], &inv.atf),
    ]
    .iter()
    .filter(|(_, keys)| !keys.is_empty())
    .map(|(label, keys)| format!("{label}: {}", keys.join(", ")))
    .collect::<Vec<_>>()
    .join(" / ")
}

/// 저장 직후 상태줄 문구. 전부 유효하면 `None`(호출부가 자기 기본 문구를 쓴다).
///
/// 상태줄은 1줄짜리 단일 슬롯이라 4군 문구를 그대로 이으면 잘린다. 그래서
/// 단일 그룹이면 그룹 전용 문구(문법 힌트 + 유효 예시 포함)를, 2그룹 이상이면
/// `필드명: 키들` 축약 문구를 쓴다 — 어느 칸이 문제인지가 최우선 정보다.
pub(crate) fn invalid_keys_status(ui: &SettingsWindow, inv: &InvalidKeys) -> Option<SharedString> {
    if inv.is_empty() {
        return None;
    }
    let tr = ui.global::<Tr>();
    if inv.group_count() == 1 {
        // 99% 케이스 — 기존 ATF 경고 스타일(예시 포함)을 그대로 유지한다.
        return Some(if !inv.toggle.is_empty() {
            tr.invoke_toggle_keys_invalid(inv.toggle.join(", ").into())
        } else if !inv.hanja.is_empty() {
            tr.invoke_hanja_keys_invalid(inv.hanja.join(", ").into())
        } else if !inv.auto_english.is_empty() {
            tr.invoke_auto_english_keys_invalid(inv.auto_english.join(", ").into())
        } else {
            tr.invoke_atf_hotkey_invalid(inv.atf.join(", ").into())
        });
    }
    let (lt, lh, la, lf) = (
        tr.get_key_field_toggle(),
        tr.get_key_field_hanja(),
        tr.get_key_field_auto_english(),
        tr.get_key_field_atf(),
    );
    let detail = invalid_keys_detail(inv, [lt.as_str(), lh.as_str(), la.as_str(), lf.as_str()]);
    Some(tr.invoke_keys_invalid_multi(detail.into()))
}

/// 필수 키 필드 중 "비어 있지 않은데 전 항목 무효"인 첫 필드의 인덱스.
///
/// 0=한/영 토글, 1=한자, 2=자동 영문 트리거. CLI(`all_keys_invalid`)·DBus(SetConfig/
/// SetConfigYaml)와 동일 정책 — 전 항목 무효는 엔진 파서가 전부 걸러 빈 목록 저장과
/// 결과가 같아지므로(한/영 전환 불능 등) 저장 자체를 반려한다. 부분 무효는 경고 후
/// 저장(warn-not-block) 유지. ATF 토글 3종은 빈 목록이 정상 옵트아웃이라 제외.
pub(crate) fn fully_invalid_required_field(cfg: &Config) -> Option<usize> {
    let e = &cfg.engine;
    let all_bad = |keys: &[String], valid: &dyn Fn(&str) -> bool| {
        !keys.is_empty() && keys.iter().all(|k| !valid(k))
    };
    if all_bad(&e.toggle_keys, &|k| InputEngine::parse_switch_key(k).is_some()) {
        return Some(0);
    }
    if all_bad(&e.hanja_keys, &|k| InputEngine::parse_switch_key(k).is_some()) {
        return Some(1);
    }
    if all_bad(&e.auto_english.trigger_keys, &|k| {
        InputEngine::is_valid_auto_english_key(k)
    }) {
        return Some(2);
    }
    None
}

/// config 저장 + 저장 후 상태줄 문구 결정을 한 곳으로 수렴하는 헬퍼.
///
/// 상태줄은 "이 저장의 결과" 를 말하는 1줄 슬롯이다. 무효 표기가 남아 있으면 그 키는
/// 엔진에서 조용히 죽으므로, **어떤 config 저장 경로(`persist_config` 경유)든** 그
/// 사실이 자기 문구(적용됨/강도 적용/모아치기 켬 …)보다 우선한다 — 경로마다 자기
/// 문구로 덮으면 경고가 점멸한다(B1). blacklist/userdict 처럼 자체 파일로 저장하는
/// 비-config 경로는 이 헬퍼를 타지 않아 자기 문구가 그대로 나온다(현행 유지).
///
/// * 필수 필드 전 항목 무효  → `keys_all_invalid` 반려 — 저장하지 않는다(최우선)
/// * 저장 성공 + 무효 키 있음 → `invalid_keys_status` 경고
/// * 저장 성공 + 전부 유효   → 호출부의 `own`
/// * 저장 실패               → `save_failed` (경고보다 우선 — 저장 자체가 안 됐다)
///
/// 병기(자기 문구 + 경고)는 채택하지 않는다: 상태줄이 1줄이라 4군 문구가 잘리고,
/// 새 Slint 조합 문자열(+ .po 항목)이 필요해진다. 자기 문구가 정보성인 경로
/// (강도 적용·모아치기 창 ms·기본값 복원)는 슬라이더/스위치/토스트가 이미 시각
/// 피드백을 주므로 문구를 양보해도 정보 손실이 없다.
fn persist_and_report(ui: &SettingsWindow, cfg: &Config, label: &str, own: SharedString) {
    let tr = ui.global::<Tr>();
    // 전 항목 무효인 필수 키 필드가 있으면 저장 자체를 반려 — config 는 통째로
    // 저장되므로 무관한 항목의 저장이라도 그 파일 쓰기가 죽은 키 목록을 함께
    // 영속화한다. UI 값은 남겨 두어 사용자가 바로 고칠 수 있게 한다.
    if let Some(idx) = fully_invalid_required_field(cfg) {
        let field = match idx {
            0 => tr.get_key_field_toggle(),
            1 => tr.get_key_field_hanja(),
            _ => tr.get_key_field_auto_english(),
        };
        ui.set_status_text(tr.invoke_keys_all_invalid(field));
        return;
    }
    match persist_config(cfg, label) {
        Ok(()) => {
            let invalid = collect_invalid_keys(cfg);
            ui.set_status_text(invalid_keys_status(ui, &invalid).unwrap_or(own));
        }
        Err(err) => ui.set_status_text(tr.invoke_save_failed(err.to_string().into())),
    }
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

/// 한국어 자판 프로필 선택지 — `(정규 이름, 친화명)` 목록.
///
/// 코어 `ProfileRegistry` 를 열거해 `language == "korean"` 프로필만 수집한다
/// (내장 + 사용자 프로필, 친화명 포함). GTK 설정앱의
/// `unim-gui-common::settings_helpers::collect_korean_profile_choices` 와 동일
/// 의미론이지만, 그 크레이트는 이 크레이트에 **Linux 전용** 의존(Cargo.toml)이라
/// 직접 호출하면 Windows 빌드가 깨진다. 크로스플랫폼(Windows/Linux 동일 목록)을
/// 위해 코어(`unim::keystroke::profile`)만 사용해 여기에 인라인 복제한다.
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

/// `(정규 이름, 친화명)` 쌍 목록 + 현재값 → (정규 이름 Vec, 표시용 Vec, 선택 인덱스).
///
/// `build_layout_lists` 의 (name, display) 쌍 변형. 표시는 친화명이 비었거나 이름과
/// 같으면 이름만, 아니면 `친화명 (정규이름)`. 현재값이 목록에 없으면(정규화 실패·
/// 스테일 이름 등) 맨 앞에 보존한다(기존 동작 유지).
fn build_layout_lists_from_pairs(
    current: &str,
    pairs: &[(String, String)],
) -> (Vec<String>, Vec<SharedString>, i32) {
    let mut canon: Vec<String> = Vec::with_capacity(pairs.len() + 1);
    let mut display: Vec<SharedString> = Vec::with_capacity(pairs.len() + 1);
    if !current.is_empty() && !pairs.iter().any(|(n, _)| n == current) {
        canon.push(current.to_string());
        display.push(SharedString::from(current));
    }
    for (name, disp) in pairs {
        canon.push(name.clone());
        display.push(if disp.is_empty() || disp == name {
            SharedString::from(name.as_str())
        } else {
            SharedString::from(format!("{disp} ({name})"))
        });
    }
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
    // bidirectional_combine 은 chord 활성과 독립 — GTK 표시 규칙(OPT-IN, None→OFF)과 동일.
    ui.set_moachigi_bidirectional(cfg.engine.korean.bidirectional_combine.unwrap_or(false));
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
    // 리눅스 GUI 미러링: 교정 결과(한글형)를 계산해 "원본 → 변환" 병기(단어 저장 안 함).
    // Forward(영→한): 입력 영문 ascii → 교정 한글 / Reverse(한→영): 입력 한글 → 교정 영문.
    let korean = unim::typefix::eng_to_kor(&e.ascii, &e.korean_layout, &e.english_layout);
    let pair = match e.direction {
        Direction::Forward => format!("{} → {}", e.ascii, korean),
        Direction::Reverse => format!("{} → {}", korean, e.ascii),
    };
    tr.invoke_blacklist_entry(pair.as_str().into(), dir, st, e.hit_count as i32)
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
    // 토글 단축키 3종 (옵트인) — 빈 목록이면 빈 문자열로 노출.
    ui.set_atf_toggle_enabled_keys(a.toggle_enabled_keys.join(", ").into());
    ui.set_atf_toggle_forward_keys(a.toggle_forward_keys.join(", ").into());
    ui.set_atf_toggle_reverse_keys(a.toggle_reverse_keys.join(", ").into());
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

/// config.yaml 저장 + (Linux) 데몬 통지를 한 곳으로 수렴하는 헬퍼.
///
/// GTK unim-settings-gtk 의 `save_and_notify`(settings_dialog.rs) 와 동일 계약:
///   ① `Config::save_to_default_path()` 로 파일 선저장
///   ② 성공 시에만 `platform::notify_config_saved` 로 데몬에 통지
///      (Linux = DBus `SetConfigYaml` fire-and-forget → 데몬이 저장 +
///       `ConfigChangedJson` 브로드캐스트, Windows = no-op — TSF DLL 이 mtime 폴링).
/// 저장 실패 시 통지하지 않고 `Err` 를 그대로 돌려 호출부가 상태 문구를 정하게 한다.
/// 모든 config 저장 사이트를 이 헬퍼로 수렴해 통지 누락을 구조적으로 막는다
/// (blacklist/userdict 저장은 데몬 mtime 감지로 충분하므로 이 헬퍼를 쓰지 않는다).
pub(crate) fn persist_config(cfg: &Config, label: &str) -> Result<(), unim::config::ConfigError> {
    // F7: 저장 직전 디스크 상태를 재로드해 UI 가 소유하지 않은 필드(데몬/CLI 가
    // 설정 창이 뜬 사이 라이브로 바꿨을 수 있는 값)를 보존한다. UI-소유 필드만
    // in-memory 값으로 덮어써 저장하고, 실제 저장된 config 로 데몬에 통지한다.
    let mut disk = Config::load_from_default_path();
    merge_ui_owned(&mut disk, cfg);
    disk.save_to_default_path()?;
    // 검증자 지적(BLOCKER, M-07 재발): baseline 을 "세션 시작값" 으로 고정해 두면
    // 세션 중 어떤 필드를 껐다가 다시 켜서(=세션 시작값으로 복귀) 저장하면
    // `merge_field` 가 "미변경" 으로 오판해 disk(이전 저장값)를 그대로 보존해버려
    // 방금 한 저장이 무시된다. baseline 을 "마지막으로 저장한 값" 으로 갱신해
    // `unim-tsf/src/settings_dialog.rs` 의 저장 후 baseline 갱신 계약과 맞춘다.
    set_config_baseline(&disk);
    platform::notify_config_saved(&disk, label);
    Ok(())
}

thread_local! {
    /// M-07(GAP-config-02) 세션 baseline — 설정 창 시작 시 로드된 config 의 스냅샷(세션
    /// 동안 불변, `set_config_baseline` 로 1회 고정). `merge_ui_owned` 가 "이 세션에서
    /// 사용자가 실제로 이 필드를 건드렸는가" 를 판별하는 데만 쓴다. Slint 이벤트 루프는
    /// 메인 스레드 단일 실행이라 스레드 간 공유가 필요 없어 `Mutex` 대신 `thread_local`.
    static CONFIG_BASELINE: RefCell<Option<Config>> = const { RefCell::new(None) };
}

/// main() 시작 시 1회 호출 — 이후 세션 동안 `merge_ui_owned` 의 lost-update 판별
/// 기준선을 고정한다. 테스트 등 호출하지 않는 경로는 baseline 이 `None` 으로 남아
/// 종전과 동일하게 "UI 값 무조건 우선" 으로 동작한다(회귀 0).
fn set_config_baseline(cfg: &Config) {
    CONFIG_BASELINE.with(|b| *b.borrow_mut() = Some(cfg.clone()));
}

/// `Debug` 포맷 비교로 값 동치를 판정한다. `merge_field` 가 다루는 타입 다수
/// (`AutoTypeFixConfig` 등)가 `PartialEq` 를 구현하지 않아(코어 `src/config.rs` 소유,
/// 이 크레이트에서 손댈 수 없음) 이미 파생돼 있는 `Debug` 로 근사 동치를 본다 —
/// "세션 중 안 바뀜" 판정에만 쓰이므로 정확한 구조적 동치가 아니어도 충분하다.
fn debug_eq<T: std::fmt::Debug>(a: &T, b: &T) -> bool {
    format!("{a:?}") == format!("{b:?}")
}

/// M-07(GAP-config-02): baseline 대비 세션 중 미변경이면 disk(외부 변경, 예: CLI) 값을
/// 보존하고, 변경됐으면 ui(이 세션에서 사용자가 실제로 바꾼 값)를 확정한다. baseline
/// 미확보(`None`)면 종전과 동일하게 무조건 ui 를 채택한다.
fn merge_field<T: Clone + std::fmt::Debug>(dst: &mut T, baseline: Option<&T>, ui: &T) {
    match baseline {
        Some(b) if debug_eq(b, ui) => {} // 세션 중 미변경 — disk(외부 변경) 보존.
        _ => *dst = ui.clone(),
    }
}

/// F7: 디스크에서 재로드한 `disk` 에 UI 가 소유·편집하는 필드만 in-memory(`ui`)
/// 값으로 덮어쓴다. UI 가 노출하지 않는 필드는 `disk` 값을 그대로 보존해, 설정
/// 창이 열려 있는 동안 외부(데몬/CLI)에서 바뀐 값을 저장 시 되돌리지 않게 한다.
///
/// UI-소유 필드(이 설정앱이 실제 편집하는 것):
///   engine.{default_category, mode_sharing, toggle_keys, hanja_keys, app_rules,
///           toggle_announce_beep, auto_typefix, auto_english},
///   engine.korean.{layout, active_rule_sets, layout_rule_sets, bidirectional_combine,
///                  chord_window_ms, commit_unit, word_mode_apps},
///   engine.english.layout.
/// `ignore_key_repeat` 는 양 플랫폼 UI-소유(Windows·Linux 모두 설정에 노출)라 UI 값으로
///   덮어쓴다 — Linux 는 데몬 repeat 게이트가 이 값을 집행한다.
///
/// M-07(GAP-config-02): 위 목록은 "UI-소유 필드는 disk 값을 무조건 덮어쓴다" 는 뜻이라,
/// 설정 창이 떠 있는 동안 CLI/DBus 가 UI-소유 필드를 바꿔도 저장 시 그 변경이 사라졌다
/// (창 시작 시점 in-memory 값이 항상 이김). 이제 각 필드는 세션 baseline 대비 "이 세션
/// 에서 사용자가 실제로 건드렸는가" 로 한 번 더 걸러, 안 건드린 필드는 disk(외부 변경)
/// 값을 보존한다. baseline 부재(테스트 등)는 종전 동작(ui 우선)과 바이트 동일하다.
fn merge_ui_owned(disk: &mut Config, ui: &Config) {
    let baseline = CONFIG_BASELINE.with(|b| b.borrow().clone());
    let be = baseline.as_ref().map(|c| &c.engine);
    let bk = be.map(|e| &e.korean);

    let d = &mut disk.engine;
    let u = &ui.engine;

    merge_field(&mut d.default_category, be.map(|e| &e.default_category), &u.default_category);
    merge_field(&mut d.mode_sharing, be.map(|e| &e.mode_sharing), &u.mode_sharing);
    merge_field(&mut d.toggle_keys, be.map(|e| &e.toggle_keys), &u.toggle_keys);
    merge_field(&mut d.hanja_keys, be.map(|e| &e.hanja_keys), &u.hanja_keys);
    merge_field(&mut d.app_rules, be.map(|e| &e.app_rules), &u.app_rules);
    merge_field(
        &mut d.toggle_announce_beep,
        be.map(|e| &e.toggle_announce_beep),
        &u.toggle_announce_beep,
    );
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
    merge_field(
        &mut d.korean.chord_window_ms,
        bk.map(|k| &k.chord_window_ms),
        &u.korean.chord_window_ms,
    );
    merge_field(&mut d.korean.commit_unit, bk.map(|k| &k.commit_unit), &u.korean.commit_unit);
    merge_field(
        &mut d.korean.word_mode_apps,
        bk.map(|k| &k.word_mode_apps),
        &u.korean.word_mode_apps,
    );
    merge_field(&mut d.english.layout, be.map(|e| &e.english.layout), &u.english.layout);
    // ignore_key_repeat: 양 플랫폼 UI-소유 (Linux 데몬 집행 + UI 노출과 정합).
    merge_field(&mut d.ignore_key_repeat, be.map(|e| &e.ignore_key_repeat), &u.ignore_key_repeat);
}

/// config `AppRule` → Slint `AppRuleItem` (category-index: 0=영문, 1=한글).
fn app_rule_to_item(r: &AppRule) -> AppRuleItem {
    AppRuleItem {
        pattern: r.app_pattern.as_str().into(),
        category_index: match r.default_category {
            InputCategory::Korean => 1,
            InputCategory::English => 0,
        },
    }
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

/// fontconfig(`fc-match`)로 한국어 지원 폰트의 실제 패밀리명을 질의한다.
///
/// `%{family}` 는 콤마로 여러 값을 반환할 수 있어 첫 패밀리만 취한다. fc-match 미설치·
/// 실패·빈 결과는 `None` → 호출부가 Noto 로 폴백한다. 시작 시 1회 동기 호출.
#[cfg(target_os = "linux")]
fn linux_korean_font_family() -> Option<String> {
    let out = std::process::Command::new("fc-match")
        .args(["-f", "%{family}", ":lang=ko"])
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    let stdout = String::from_utf8_lossy(&out.stdout);
    let first = stdout.split(',').next().unwrap_or("").trim();
    if first.is_empty() {
        None
    } else {
        Some(first.to_string())
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // autostart(--first-run-if-needed): 마법사 완료 기록(seen)이 있으면 창·싱글턴 락·
    // 알림 전부 없이 즉시 종료한다. acquire_singleton_or_foreground() 이전에 검사해
    // flock·notify 비용까지 없앤다 (매 로그인 마법사 재출현 방지). 비용 = 파일 read 1회.
    if wizard::autostart_should_exit_quietly() {
        return Ok(());
    }

    // 단일 인스턴스 가드: 이미 설정 창이 떠 있으면 그 창을 전면화하고 즉시 종료한다.
    // (중복 창이 뜨면 각자 저장해 마지막 창이 이전 변경을 덮어쓰는 문제를 차단.)
    if !platform::acquire_singleton_or_foreground() {
        return Ok(());
    }

    // Linux 주의: AccessKit(AT-SPI)의 zbus 5 는 반드시 async-io 모드여야 한다.
    // 어느 워크스페이스 멤버라도 zbus 5 의 tokio feature 를 켜면(feature 통일)
    // AccessKit 이 자기 내부 스레드에서 앰비언트 tokio 런타임을 요구해 창 생성 시
    // "there is no reactor running" 패닉이 난다 — 이 프로세스에서 런타임을 만들어
    // enter 해도 남의 스레드라 소용없다. 실제 가드는 unim-gui-common 의 ksni
    // default-features = false (async-io 백엔드) 선언이다.

    // 백엔드는 양 플랫폼 공통 winit-skia 고정이다. Qt 백엔드는 Cargo.toml 에서
    // 컴파일 자체를 제외했다 — Slint 의 Qt 의존은 하드 링크라 포함하는 순간 GTK
    // 데스크톱 사용자에게 libQt6 런타임을 강제하게 돼 UNIM 컨셉과 상충한다.
    // Skia 렌더러 명시 이유: 기본 FemtoVG 는 힌팅이 없어 작은 한글(14px) 획 굵기가
    // 불균일하고, Skia 는 네이티브(FreeType/DirectWrite) 글리프 힌팅을 탄다.
    // 설정앱 내 한글 입력은 X11=XIM / Wayland=text-input-v3 경로를 탄다.
    // SLINT_BACKEND 형식은 "<backend>-<renderer>". 사용자/개발자가 이미
    // 지정했다면(예: 디버깅용 winit-software) 존중한다.
    // SAFETY: SettingsWindow::new() 이전, 단일 스레드 진입부에서만 호출한다.
    if std::env::var("SLINT_BACKEND").is_err() {
        std::env::set_var("SLINT_BACKEND", "winit-skia");
    }

    // GAP-first-05: 창 생성 실패(예: 렌더러/디스플레이 초기화 불가)를 그냥 `?` 로
    // 전파하면 autostart(`--first-run-if-needed`) 경로에서 "매 로그인마다 조용히 재시도
    // 실패" 가 되어 사용자가 원인을 알 방법이 없다. 실패 시 로그 위치를 남기고(가능하면
    // 데스크톱 알림) 계속 실패를 전파한다 — 성공 처리로 seen 을 찍지는 않는다(원인이
    // 고쳐질 수도 있으므로 재시도 자체는 막지 않음).
    let ui = match SettingsWindow::new() {
        Ok(ui) => ui,
        Err(e) => {
            platform::log_wizard_render_failure(&e.to_string());
            return Err(e.into());
        }
    };

    // Linux: 기본 폰트 property(맑은 고딕 = Windows 기본값)를 시스템에 실제 설치된
    // 한글 폰트로 교체한다. skia fontmgr 의 부재 패밀리 폴백이 fontconfig 치환과 다를 수
    // 있어 명시 지정이 안전한데, 특정 패밀리 하드코딩("Noto Sans CJK KR")은 미설치
    // 배포판에서 tofu(□) 위험이 있다. fontconfig(fc-match :lang=ko)로 실 설치 패밀리를
    // 질의하고, 실패 시에만 Noto 로 폴백한다. Windows 는 이 호출이 컴파일되지 않아 불변.
    #[cfg(target_os = "linux")]
    ui.set_ui_font_family(
        linux_korean_font_family()
            .unwrap_or_else(|| "Noto Sans CJK KR".to_string())
            .into(),
    );

    // OS UI 언어 추종: 한국어면 원문(index 0), 그 외에는 영어 번들 번역 선택.
    // select_bundled_translation 은 첫 컴포넌트 생성 이후에 호출해야 한다.
    // 빈 문자열("")은 index 0(=소스 한국어)로 특수 처리된다.
    let _ = slint::select_bundled_translation(if platform::ui_language_is_korean() { "" } else { "en" });

    // UX-SHARED-V02: 조합 트리거(`key:Ctrl+B` 등)는 Windows TSF 가 다중 키 조합을
    // 인식하지 못해 미지원(단일 키만 유효)이다. 안 되는 예시를 광고하지 않도록 Windows
    // 만 false — Linux 는 기본값(true)을 그대로 둔다(회귀 0).
    ui.global::<Tr>().set_combo_trigger_supported(!cfg!(target_os = "windows"));

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
    // M-07(GAP-config-02): 세션 시작 시점 스냅샷을 고정 — persist_config 의
    // merge_ui_owned 가 "이 세션에서 사용자가 실제로 건드린 UI-소유 필드" 만 확정
    // 저장하고, 안 건드린 필드는 외부(CLI/DBus) 변경을 되돌리지 않게 한다.
    set_config_baseline(&config.borrow());
    let blacklist = Rc::new(RefCell::new(Blacklist::load_from_default_path()));
    let userdict = Rc::new(RefCell::new(UserDictionary::load_from_default_path()));
    // I6: 되돌리기용 스냅샷 1개(마지막 삭제만 복원 가능).
    let delete_snapshot: Rc<RefCell<Option<DeleteSnapshot>>> = Rc::new(RefCell::new(None));

    // 자판 목록 구성 (정규 이름은 저장 시 인덱스→이름 변환에 사용).
    let (kor_canon, kor_disp, kor_idx);
    let (eng_canon, eng_disp, eng_idx);
    {
        let cfg = config.borrow();
        // 한글 자판 목록 — 정적 4종 대신 ProfileRegistry 열거(내장 5종+사용자 프로필,
        // 안마태 등 모아치기 자판 포함). GTK 설정앱과 동일 소스. 현재값은 정규화된
        // effective_layout_name 으로 매칭한다(별칭 처리 포함).
        let kor_pairs = collect_korean_profile_choices();
        (kor_canon, kor_disp, kor_idx) =
            build_layout_lists_from_pairs(&cfg.engine.korean.effective_layout_name(), &kor_pairs);
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

    // 앱별 강제 모드 규칙 모델 — config.engine.app_rules 와 양방향(추가/삭제/편집).
    let app_rule_model: Rc<VecModel<AppRuleItem>> = Rc::new(VecModel::default());
    ui.set_app_rule_items(ModelRc::from(app_rule_model.clone()));
    {
        let cfg = config.borrow();
        app_rule_model.set_vec(
            cfg.engine
                .app_rules
                .iter()
                .map(app_rule_to_item)
                .collect::<Vec<_>>(),
        );
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
        // 조합키 자동반복 억제 (접근성, 지체장애) — Windows·Linux 양 플랫폼 노출.
        ui.set_ignore_key_repeat(e.ignore_key_repeat);
        // 앱별 강제 모드 콤보 옵션(0=영문, 1=한글).
        ui.set_app_rule_category_options(string_model(vec![
            tr.get_mode_english(),
            tr.get_mode_korean(),
        ]));

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
            // 토글 단축키 3종 — 빈 목록 허용(옵트인, 비우면 사용 안 함).
            a.toggle_enabled_keys = split_keys(&ui.get_atf_toggle_enabled_keys());
            a.toggle_forward_keys = split_keys(&ui.get_atf_toggle_forward_keys());
            a.toggle_reverse_keys = split_keys(&ui.get_atf_toggle_reverse_keys());

            // 자판이 바뀌었으면 규칙 세트 그룹 + 접근성 배지 + 모아치기 카드를
            // 새 프로필 기준으로 재구성 (moachigi-supported 는 자판마다 다름).
            if kor_changed {
                if let Some(profile) = new_profile.as_ref() {
                    rule_model.set_vec(rule_set_items_for(&cfg, profile));
                }
                ui.set_korean_noshift_selected(is_noshift_layout(&cfg));
                push_moachigi_to_ui(&ui, &cfg, new_profile.as_ref());
            }

            // 무효 표기 검증 — 엔진 파서(단일 진실 공급원)가 버리는 표기는 데몬에서
            // 조용히 사라진다(키가 죽음). CLI·D-Bus 와 같은 "저장은 하되 경고" 정책으로
            // 저장은 그대로 하고 상태줄로 알린다. `a` 대여가 끝난 뒤라 `&cfg` 재대여 성립.
            persist_and_report(&ui, &cfg, "auto_save", tr.get_applied());
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
                let list = cfg.engine.korean.active_rule_sets.get_or_insert(seed);
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
            persist_and_report(&ui, &cfg, "rule_set_toggled", tr.get_applied());
            item.active = on;
            rule_model.set_row_data(idx as usize, item);
        });
    }

    // ── 앱별 강제 모드 규칙: 추가 ──
    {
        let ui_weak = ui.as_weak();
        let config = config.clone();
        let app_rule_model = app_rule_model.clone();
        ui.on_app_rule_add(move || {
            let ui = ui_weak.unwrap();
            let tr = ui.global::<Tr>();
            let mut cfg = config.borrow_mut();
            cfg.engine.app_rules.push(AppRule {
                app_pattern: String::new(),
                default_category: InputCategory::English,
            });
            // 모델을 config 기준으로 재구성(config 가 항상 진실원). 편집 중 패턴은
            // config 에 이미 반영돼 있으므로 재구성해도 표시가 어긋나지 않는다.
            app_rule_model.set_vec(
                cfg.engine.app_rules.iter().map(app_rule_to_item).collect::<Vec<_>>(),
            );
            persist_and_report(&ui, &cfg, "app_rule_add", tr.get_applied());
        });
    }
    // ── 앱별 강제 모드 규칙: 삭제 (스크롤 위치는 ScrollView 가 보존) ──
    {
        let ui_weak = ui.as_weak();
        let config = config.clone();
        let app_rule_model = app_rule_model.clone();
        ui.on_app_rule_remove(move |idx| {
            let ui = ui_weak.unwrap();
            let tr = ui.global::<Tr>();
            if idx < 0 {
                return;
            }
            let i = idx as usize;
            let mut cfg = config.borrow_mut();
            if i >= cfg.engine.app_rules.len() {
                return;
            }
            cfg.engine.app_rules.remove(i);
            // config 기준 재구성 — 삭제로 인덱스가 밀려도 각 행이 올바른 패턴/모드를
            // 표시한다. 스크롤 위치는 ScrollView 가 보존(viewport-y 미변경).
            app_rule_model.set_vec(
                cfg.engine.app_rules.iter().map(app_rule_to_item).collect::<Vec<_>>(),
            );
            persist_and_report(&ui, &cfg, "app_rule_remove", tr.get_applied());
        });
    }
    // ── 앱별 강제 모드 규칙: 패턴 편집 (저장만 — 모델 재기록 없이 커서 점프 방지) ──
    {
        let ui_weak = ui.as_weak();
        let config = config.clone();
        ui.on_app_rule_pattern_edited(move |idx, text| {
            let ui = ui_weak.unwrap();
            let tr = ui.global::<Tr>();
            if idx < 0 {
                return;
            }
            let i = idx as usize;
            let mut cfg = config.borrow_mut();
            if i >= cfg.engine.app_rules.len() {
                return;
            }
            cfg.engine.app_rules[i].app_pattern = text.trim().to_string();
            persist_and_report(&ui, &cfg, "app_rule_pattern", tr.get_applied());
        });
    }
    // ── 앱별 강제 모드 규칙: 모드(영문/한글) 변경 ──
    {
        let ui_weak = ui.as_weak();
        let config = config.clone();
        let app_rule_model = app_rule_model.clone();
        ui.on_app_rule_category_changed(move |idx, cat_idx| {
            use slint::Model;
            let ui = ui_weak.unwrap();
            let tr = ui.global::<Tr>();
            if idx < 0 {
                return;
            }
            let i = idx as usize;
            let mut cfg = config.borrow_mut();
            if i >= cfg.engine.app_rules.len() {
                return;
            }
            cfg.engine.app_rules[i].default_category = if cat_idx == 1 {
                InputCategory::Korean
            } else {
                InputCategory::English
            };
            // 해당 행만 config 기준으로 갱신(패턴도 config 값 — 편집 중 패턴 되돌림 방지).
            app_rule_model.set_row_data(i, app_rule_to_item(&cfg.engine.app_rules[i]));
            persist_and_report(&ui, &cfg, "app_rule_category", tr.get_applied());
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
            persist_and_report(&ui, &cfg, "atf_preset", tr.invoke_strength_applied(name));
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
            persist_and_report(&ui, &cfg, "atf_restore_defaults", tr.get_atf_restored());
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
            persist_and_report(&ui, &cfg, "moachigi_toggled", msg);
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
                persist_and_report(
                    &ui,
                    &cfg,
                    "moachigi_window",
                    tr.invoke_moachigi_window_set(w as i32),
                );
            }
        });
    }

    // ── 양방향 자모 결합 스위치 → bidirectional_combine Some(v) (chord 와 독립) ──
    {
        let ui_weak = ui.as_weak();
        let config = config.clone();
        ui.on_moachigi_bidirectional_toggled(move |v| {
            let ui = ui_weak.unwrap();
            let tr = ui.global::<Tr>();
            let mut cfg = config.borrow_mut();
            // OPT-IN 명시 활성화. None 복귀는 CLI 전용 의미론 유지(GTK settings_dialog.rs:399 와 동일).
            cfg.engine.korean.bidirectional_combine = Some(v);
            persist_and_report(&ui, &cfg, "korean_bidirectional_combine", tr.get_applied());
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
                // M-12(A11Y-02/UX-SHARED-01): 자동반복 억제는 Windows·Linux 양 플랫폼
                // UI-소유 기능이다 — Linux 는 데몬 게이트(`unim-dbus/src/engine_worker.rs`
                // 의 `should_suppress_repeat`)가 이미 집행하므로 여기서 cfg 로 걸러선 안 된다.
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
                // 넉넉한 타이밍: 자동반복 억제(양 플랫폼) + 오타 교정 판정 시간 확대 +
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
            // 프리셋도 config 를 통째로 저장하므로 같은 검증을 태운다 — 죽은 키가
            // 남아 있다는 정보가 프리셋 적용 안내보다 우선한다(상태줄은 "이 저장의 결과").
            persist_and_report(&ui, &cfg, "accessibility_preset", status);
        });
    }

    // ── 닫기 ──
    {
        let ui_weak = ui.as_weak();
        ui.on_close_clicked(move || {
            let _ = ui_weak.unwrap().window().hide();
        });
    }

    // ── 도움말 — 동봉된 오프라인 매뉴얼을 기본 브라우저로 연다 ──
    // 경로 해석(플랫폼별 설치 위치)·언어 선택(UI 언어 추종)·실패 안내는 백엔드 몫이라
    // 여기는 위임만 한다. UI 는 플랫폼을 모른다.
    ui.on_open_help(platform::open_help);

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
    // ── 억제 단어 영구 활성 (선택) — Tentative→확정 / Inactive→재활성(확정) ──
    // 리눅스 GUI 의 Tentative "확정"·Inactive "재활성화(확정)" 버튼을 단일 버튼으로 통합.
    // StandardListView 는 행내 버튼 불가라 선택 인덱스 기반. 인덱스는 entries 순서와 1:1.
    {
        let ui_weak = ui.as_weak();
        let blacklist = blacklist.clone();
        ui.on_blacklist_confirm(move |idx| {
            let ui = ui_weak.unwrap();
            let tr = ui.global::<Tr>();
            let mut bl = blacklist.borrow_mut();
            if idx < 0 || (idx as usize) >= bl.entries.len() {
                return;
            }
            let i = idx as usize;
            match bl.entries[i].status {
                // 억제를 영구 확정한다. 이미 Confirmed(영구 활성)면 no-op.
                EntryStatus::Tentative => bl.promote_to_confirmed(i),
                EntryStatus::Inactive => bl.reactivate_as_confirmed(i),
                EntryStatus::Confirmed => return,
            }
            let _ = bl.save_to_default_path();
            refresh_blacklist(&ui, &bl);
            // 확정(영구 활성)은 되돌리기(undo) 대상이 아니므로 status_text 만 갱신한다.
            // undo 토스트(set_toast_visible)를 재사용하면 confirm 이 delete_snapshot 을
            // 만들지 않아, 직전 삭제 스냅샷이 남아있을 때 "되돌리기" 클릭 시 삭제가 부활하고
            // confirm 이 소실된다(D1). undo 토스트는 삭제/모두삭제 전용.
            ui.set_status_text(tr.get_bl_confirmed());
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
                    persist_and_report(&ui, &cfg, "undo_atf_defaults", tr.get_atf_undone());
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

    // ── 설치 마법사(--first-run / --whats-new) 배선 ──
    // 인자가 없으면 통째로 건너뛰고 기존 일반 설정 모드로 동작(무회귀).
    wizard::wire(&ui, &config);

    ui.run()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// S3-1/S3-3: 한글 자판 목록에 안마태(모아치기) 자판이 포함되고 친화명이 비어있지 않다.
    #[test]
    fn korean_choices_include_anmatae_with_display() {
        let choices = collect_korean_profile_choices();
        let anmatae = choices
            .iter()
            .find(|(n, _)| n.as_str() == "ko_3bul_anmatae")
            .expect("ko_3bul_anmatae 가 한글 자판 목록에 있어야 함");
        assert!(
            !anmatae.1.trim().is_empty(),
            "안마태 자판 친화명이 비어있지 않아야 함"
        );
        // 영문 프로필은 한글 목록에 섞이지 않는다.
        assert!(
            choices.iter().all(|(n, _)| !n.starts_with("en_")),
            "한글 목록에 영문 프로필이 섞이면 안 됨"
        );
    }

    /// 최우선 가드: 신규 설치(출하 기본값)는 어떤 키 경고도 내지 않아야 한다.
    #[test]
    fn collect_invalid_keys_empty_for_defaults() {
        let inv = collect_invalid_keys(&Config::default());
        assert_eq!(inv, InvalidKeys::default(), "기본 설정에 경고가 있으면 안 됨");
        assert!(inv.is_empty());
        assert_eq!(inv.group_count(), 0);
    }

    /// D3/R7 회귀 가드: 전환키는 수정자 키 자체가 유효하다. 여기에 ATF 파서를
    /// 쓰면 출하 기본값 RightAlt 가 무효로 오탐된다.
    #[test]
    fn collect_invalid_keys_accepts_modifier_toggle_key() {
        let mut cfg = Config::default();
        cfg.engine.toggle_keys = vec!["Korean".to_string(), "RightAlt".to_string()];
        cfg.engine.hanja_keys = vec!["Hanja".to_string(), "F9".to_string()];
        let inv = collect_invalid_keys(&cfg);
        assert!(inv.toggle.is_empty(), "RightAlt 는 유효한 전환키");
        assert!(inv.hanja.is_empty(), "F9 는 유효한 한자키");
    }

    /// 전 항목 무효 반려 — CLI/DBus `all_keys_invalid` 와 동일 정책 3지점 싱크.
    #[test]
    fn fully_invalid_required_field_matches_cli_policy() {
        // 기본값은 반려 없음.
        assert_eq!(fully_invalid_required_field(&Config::default()), None);
        // 부분 무효는 warn-not-block 유지(반려 아님).
        let mut cfg = Config::default();
        cfg.engine.toggle_keys = vec!["NoSuchKey".to_string(), "Korean".to_string()];
        assert_eq!(fully_invalid_required_field(&cfg), None);
        // 전 항목 무효 → 필드 인덱스 반환.
        cfg.engine.toggle_keys = vec!["NoSuchKey".to_string(), "Shift+Space".to_string()];
        assert_eq!(fully_invalid_required_field(&cfg), Some(0));
        let mut cfg = Config::default();
        cfg.engine.hanja_keys = vec!["Hangul".to_string()];
        assert_eq!(fully_invalid_required_field(&cfg), Some(1));
        let mut cfg = Config::default();
        cfg.engine.auto_english.trigger_keys = vec!["key:Ctrl+".to_string()];
        assert_eq!(fully_invalid_required_field(&cfg), Some(2));
        // ATF 3종 전 항목 무효는 반려 대상이 아니다(옵트아웃 필드).
        let mut cfg = Config::default();
        cfg.engine.auto_typefix.toggle_enabled_keys = vec!["ScrollLock".to_string()];
        assert_eq!(fully_invalid_required_field(&cfg), None);
    }

    /// ATF 는 전환키와 달리 수정자 조합을 지원한다(`+`·`-` 구분자 모두).
    #[test]
    fn collect_invalid_keys_atf_accepts_combo() {
        let mut cfg = Config::default();
        let a = &mut cfg.engine.auto_typefix;
        a.toggle_enabled_keys = vec!["Shift+F8".to_string()];
        a.toggle_forward_keys = vec!["Ctrl-Left".to_string()];
        a.toggle_reverse_keys = vec!["F10".to_string()];
        assert!(collect_invalid_keys(&cfg).atf.is_empty());
    }

    /// I7: ATF 3필드는 빈 목록이 정상(옵트인) — 경고 대상이 아니다.
    #[test]
    fn collect_invalid_keys_atf_allows_empty_lists() {
        let mut cfg = Config::default();
        let a = &mut cfg.engine.auto_typefix;
        a.toggle_enabled_keys.clear();
        a.toggle_forward_keys.clear();
        a.toggle_reverse_keys.clear();
        assert!(collect_invalid_keys(&cfg).is_empty());
    }

    /// 4군이 각각 자기 파서로 판정되고, 무효값이 자기 그룹에만 담긴다.
    #[test]
    fn collect_invalid_keys_flags_each_group() {
        let mut cfg = Config::default();
        cfg.engine.toggle_keys = vec!["Korean".to_string(), "Shift+Space".to_string()];
        cfg.engine.hanja_keys = vec!["Hanja".to_string(), "Control+Enter".to_string()];
        cfg.engine.auto_english.trigger_keys =
            vec!["key:Escape".to_string(), "key:Ctrl+".to_string()];
        cfg.engine.auto_typefix.toggle_enabled_keys =
            vec!["F10".to_string(), "ScrollLock".to_string()];
        cfg.engine.auto_typefix.toggle_forward_keys.clear();
        cfg.engine.auto_typefix.toggle_reverse_keys.clear();

        let inv = collect_invalid_keys(&cfg);
        assert_eq!(inv.toggle, vec!["Shift+Space".to_string()]);
        assert_eq!(inv.hanja, vec!["Control+Enter".to_string()]);
        assert_eq!(inv.auto_english, vec!["key:Ctrl+".to_string()]);
        // ScrollLock 은 KeyCode 에 없는 이름이다(옛 placeholder 가 권장하던 값).
        assert_eq!(inv.atf, vec!["ScrollLock".to_string()]);
        assert_eq!(inv.group_count(), 4);
    }

    /// ATF 3필드는 하나의 그룹으로 합산되며 순서는 전체→정방향→역방향.
    #[test]
    fn collect_invalid_keys_atf_merges_three_fields() {
        let mut cfg = Config::default();
        let a = &mut cfg.engine.auto_typefix;
        a.toggle_enabled_keys = vec!["Bogus1".to_string()];
        a.toggle_forward_keys = vec!["Bogus2".to_string()];
        a.toggle_reverse_keys = vec!["Bogus3".to_string()];
        let inv = collect_invalid_keys(&cfg);
        assert_eq!(
            inv.atf,
            vec![
                "Bogus1".to_string(),
                "Bogus2".to_string(),
                "Bogus3".to_string()
            ]
        );
        assert_eq!(inv.group_count(), 1, "ATF 3필드는 단일 그룹으로 센다");
    }

    /// 복수 그룹 축약 문구 조각 — 무효한 그룹만 라벨과 함께 이어 붙인다.
    #[test]
    fn invalid_keys_detail_joins_groups_with_labels() {
        let inv = InvalidKeys {
            toggle: vec!["X".to_string()],
            hanja: vec!["Y".to_string(), "Z".to_string()],
            ..InvalidKeys::default()
        };
        let detail = invalid_keys_detail(&inv, ["한/영 토글", "한자 변환", "자동 영문", "오타 교정"]);
        assert_eq!(detail, "한/영 토글: X / 한자 변환: Y, Z");
    }

    /// 무효가 없으면 조각도 비어야 한다(호출부가 기본 문구를 쓴다).
    #[test]
    fn invalid_keys_detail_empty_when_valid() {
        let detail = invalid_keys_detail(&InvalidKeys::default(), ["a", "b", "c", "d"]);
        assert_eq!(detail, "");
    }

    /// S3-4 회귀 가드: 안마태 프로필이 moachigi capability 를 유지한다.
    #[test]
    fn anmatae_profile_has_moachigi() {
        let p = load_profile("ko_3bul_anmatae").expect("안마태 프로필 로드");
        assert!(
            p.moachigi.is_some(),
            "안마태는 moachigi capability 를 가져야 함"
        );
    }

    /// F7: 저장 병합이 UI-소유 필드는 in-memory 값으로 덮어쓰고, 비-UI 필드는
    /// disk 값을 보존한다(ignore_key_repeat 는 양 플랫폼 UI-소유).
    #[test]
    fn merge_ui_owned_overwrites_ui_and_preserves_non_ui() {
        let mut disk = Config::default();
        // UI-소유 필드 — disk 값은 in-memory 값으로 덮어써져야 함.
        disk.engine.toggle_keys = vec!["OldToggle".to_string()];
        disk.engine.app_rules = vec![AppRule {
            app_pattern: "old".to_string(),
            default_category: InputCategory::English,
        }];
        // ignore_key_repeat 도 UI-소유(양 플랫폼) — disk 값(true)이 ui 값(false)으로 덮어써져야 함.
        disk.engine.ignore_key_repeat = true;
        // 비-UI 필드(merge_ui_owned 가 복사하지 않음) — disk 값이 보존돼야 함.
        disk.engine.english.preferred_direct = true;

        let mut ui = Config::default();
        ui.engine.toggle_keys = vec!["NewToggle".to_string()];
        ui.engine.app_rules = vec![AppRule {
            app_pattern: "new".to_string(),
            default_category: InputCategory::Korean,
        }];
        ui.engine.ignore_key_repeat = false;
        ui.engine.english.preferred_direct = false;

        merge_ui_owned(&mut disk, &ui);

        // UI-소유 필드는 덮어써짐.
        assert_eq!(disk.engine.toggle_keys, vec!["NewToggle".to_string()]);
        assert_eq!(disk.engine.app_rules.len(), 1);
        assert_eq!(disk.engine.app_rules[0].app_pattern, "new");
        assert_eq!(
            disk.engine.app_rules[0].default_category,
            InputCategory::Korean
        );

        // ignore_key_repeat 는 양 플랫폼 UI-소유이므로 ui 값(false)이 반영됨.
        assert!(
            !disk.engine.ignore_key_repeat,
            "ignore_key_repeat 는 UI-소유이므로 ui 값(false)이 반영돼야 함"
        );

        // 비-UI 필드(preferred_direct)는 merge 대상이 아니므로 disk 값(true)이 보존됨.
        assert!(
            disk.engine.english.preferred_direct,
            "preferred_direct(비-UI)는 disk 값이 보존돼야 함"
        );
    }

    /// M-07(GAP-config-02) 회귀 가드: baseline 대비 세션 중 안 건드린 UI-소유 필드는
    /// disk(외부 변경, 예: CLI) 값을 보존하고, 실제로 건드린 필드는 ui 값을 확정한다.
    /// 테스트 종료 시 baseline 을 반드시 리셋해 스레드 재사용 시 다른 테스트로
    /// 누수되지 않게 한다(cargo test 는 테스트마다 스레드를 새로 만들지 않는다).
    #[test]
    fn merge_ui_owned_preserves_untouched_field_but_keeps_user_edit() {
        let baseline = Config::default(); // 세션 시작 시 로드된 값(양쪽 필드 모두 기본).
        set_config_baseline(&baseline);

        let mut disk = Config::default();
        // 외부(CLI)가 세션 도중 toggle_keys 를 바꿔 놓음 — UI 는 이 필드를 안 건드림.
        disk.engine.toggle_keys = vec!["ExternalCli".to_string()];
        // 외부가 hanja_keys 도 바꿔 놓음 — 이번엔 UI 도 이 필드를 건드릴 것(경합).
        disk.engine.hanja_keys = vec!["ExternalHanja".to_string()];

        let mut ui = baseline.clone();
        // UI 는 hanja_keys 만 이 세션에서 실제로 편집(baseline 과 다름).
        ui.engine.hanja_keys = vec!["UserEdited".to_string()];
        // toggle_keys 는 UI 상 baseline 그대로(세션 중 안 건드림).

        merge_ui_owned(&mut disk, &ui);

        // 안 건드린 필드(toggle_keys) — 외부(CLI) 변경이 보존됨.
        assert_eq!(
            disk.engine.toggle_keys,
            vec!["ExternalCli".to_string()],
            "세션 중 UI 가 안 건드린 필드는 외부 변경을 잃으면 안 됨(M-07)"
        );
        // 건드린 필드(hanja_keys) — 사용자의 이번 세션 편집이 확정됨(외부 변경보다 우선).
        assert_eq!(
            disk.engine.hanja_keys,
            vec!["UserEdited".to_string()],
            "세션 중 UI 가 실제로 편집한 필드는 그 값이 확정돼야 함"
        );

        CONFIG_BASELINE.with(|b| *b.borrow_mut() = None); // 다른 테스트로 누수 방지.
    }

    /// BLOCKER(M-07 재발) 회귀 가드: baseline 을 "세션 시작값"으로 고정한 채 갱신하지
    /// 않으면, 한 세션 안에서 어떤 필드를 껐다가(1차 저장) 다시 켜서(=세션 시작값으로
    /// 복귀, 2차 저장) 저장할 때 `merge_field` 가 "미변경"으로 오판해 2차 저장이
    /// 무시된다. `persist_config` 가 저장 성공 직후 `set_config_baseline` 을 호출해
    /// baseline 을 "마지막으로 저장한 값"으로 갱신하는 계약을 이 테스트로 고정한다
    /// (`unim-tsf/src/settings_dialog.rs` 와 동일 계약).
    #[test]
    fn toggle_off_then_on_in_same_session_is_saved() {
        let mut session_start = Config::default();
        session_start.engine.toggle_announce_beep = true;
        set_config_baseline(&session_start);

        // 1차 저장: 사용자가 끔.
        let mut disk = session_start.clone(); // 디스크는 세션 시작 시점과 동일하다고 가정.
        let mut ui = session_start.clone();
        ui.engine.toggle_announce_beep = false;
        merge_ui_owned(&mut disk, &ui);
        assert!(
            !disk.engine.toggle_announce_beep,
            "1차 저장(끔)은 항상 반영돼야 함"
        );
        // persist_config 계약: 저장 성공 직후 baseline 을 방금 저장한 값으로 갱신.
        set_config_baseline(&disk);

        // 2차 저장: 사용자가 다시 켬(=세션 시작값과 동일하게 복귀).
        let mut disk2 = disk.clone(); // 디스크 재로드 시뮬레이션(1차 저장 결과 그대로).
        let mut ui2 = disk.clone();
        ui2.engine.toggle_announce_beep = true;
        merge_ui_owned(&mut disk2, &ui2);

        assert!(
            disk2.engine.toggle_announce_beep,
            "off→on 왕복 후 2차 저장이 무시되면 안 됨 (BLOCKER M-07 재발)"
        );

        CONFIG_BASELINE.with(|b| *b.borrow_mut() = None); // 다른 테스트로 누수 방지.
    }

    /// B1 가드: config 저장 사이트는 전부 `persist_and_report` 를 거친다.
    /// 자기 문구가 무효 키 경고를 덮어 경고가 점멸하던 회귀를 구조로 막는다.
    /// (wizard.rs 는 상태줄이 없어 예외 — 설정 모드 전환 지점에서만 경고를 싣는다.)
    ///
    /// `include_str!` 로 이 파일 자신을 읽으므로, 찾는 패턴을 리터럴 그대로 적으면
    /// 이 assert 문 자신의 문자열 리터럴이 검색 대상에 다시 걸려 카운트가 어긋난다
    /// (자기 매치). 조각을 나눠 런타임에 이어붙여 그 문제를 피한다.
    #[test]
    fn no_direct_persist_config_call_site_in_settings_main() {
        let src = include_str!("main.rs");
        let direct_call = ["persist_config(&cf", "g"].concat();
        assert_eq!(
            src.matches(direct_call.as_str()).count(),
            0,
            "persist_config 직접 호출 금지 — persist_and_report 로 수렴할 것"
        );
        let helper_call = ["persist_config(cfg", ", label)"].concat();
        assert_eq!(
            src.matches(helper_call.as_str()).count(),
            1,
            "persist_config 호출은 persist_and_report 안 1곳뿐이어야 함"
        );
    }

    /// M-12(A11Y-02/UX-SHARED-01) 회귀 가드: 접근성 프리셋의 자동반복 억제 적용이
    /// 다시 Windows 전용 cfg 게이트로 회귀하지 않도록 감시한다. 집행은
    /// `unim-dbus/src/engine_worker.rs` 데몬 게이트가 이미 플랫폼 무관으로 수행하므로,
    /// `on_apply_accessibility_preset` 클로저 본문은 어떤 target_os 게이트도 없이
    /// 무조건 켜야 한다.
    ///
    /// 검사 범위는 해당 클로저 본문으로 한정한다 — 파일 전체를 대상으로 하면
    /// 이 프리셋과 무관한 정당한 `cfg(target_os = "windows")` 분기(예: 대칭되는
    /// `cfg(target_os = "linux")` 분기의 반대편)를 추가하는 순간 이 테스트가
    /// 원인 불명으로 깨진다.
    ///
    /// `include_str!` 자기 매치 회피를 위해 찾는 리터럴을 조각내 이어붙인다(위 가드와 동일 기법).
    #[test]
    fn no_windows_only_cfg_gate_in_settings_main() {
        let src = include_str!("main.rs");
        let start_marker = ["on_apply_accessibility_preset(move |pre", "set| {"].concat();
        let end_marker = ["persist_and_report(&ui, &cfg, \"accessibility_pre", "set\", status);"].concat();

        let start = src
            .find(start_marker.as_str())
            .expect("on_apply_accessibility_preset 클로저를 찾을 수 없음 — 가드 대상 이동/개명 여부 확인 필요");
        let end = src[start..]
            .find(end_marker.as_str())
            .map(|rel| start + rel + end_marker.len())
            .expect("접근성 프리셋 클로저 종료 지점(persist_and_report 호출)을 찾을 수 없음");
        let body = &src[start..end];

        let needle = ["cfg(target_os = \"win", "dows\")"].concat();
        assert_eq!(
            body.matches(needle.as_str()).count(),
            0,
            "접근성 프리셋 클로저에 Windows 전용 cfg 게이트가 있으면 안 됨(M-12 회귀 — 리눅스 접근성 프리셋 무력화)"
        );
    }
}

