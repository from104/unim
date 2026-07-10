//! 설치 마법사(`--first-run` / `--whats-new`) — 플랫폼 중립 로직 + UI 배선.
//!
//! main.rs 에서 이동. 마법사 모드 파싱·페이지 구성·semver 게이트·UI 배선을 담는다.
//! 플랫폼 의존(기본 입력기/언어팩/seen 버전)은 [`crate::platform`] 으로 위임한다.

use std::cell::RefCell;
use std::rc::Rc;

use slint::ComponentHandle;
use unim::config::Config;

use crate::platform;
use crate::SettingsWindow;

/// 설치 마법사 실행 모드 — CLI 인자로 지정.
#[derive(Clone, Copy, PartialEq)]
enum WizardMode {
    /// 신규 설치(`--first-run`): 버전 무관 전체 안내.
    FirstRun,
    /// 업데이트(`--whats-new`): seen 버전보다 새로운 항목만 안내.
    WhatsNew,
}

/// CLI 인자에서 마법사 모드를 파싱. 인자 없으면 일반 설정 모드(None).
fn parse_wizard_mode() -> Option<WizardMode> {
    for a in std::env::args().skip(1) {
        match a.as_str() {
            "--first-run" => return Some(WizardMode::FirstRun),
            "--whats-new" => return Some(WizardMode::WhatsNew),
            _ => {}
        }
    }
    None
}

// 마법사 페이지 종류 (Slint `wiz-current-kind` 와 동일 값이어야 함).
const WIZ_WELCOME: i32 = 0;
const WIZ_LICENSE: i32 = 1;
const WIZ_LANGPACK: i32 = 2;
const WIZ_DEFAULT_IME: i32 = 3;
const WIZ_TYPEFIX: i32 = 4;
const WIZ_FINISH: i32 = 5;

/// 각 마법사 항목의 도입 버전. seen >= introduced 이면 `--whats-new` 에서 스킵한다.
/// (현재 전 항목 동일 — 이후 항목 추가 시 개별 상수로 분화 가능.)
const WIZ_INTRODUCED: &str = "0.3.61";

/// "a.b.c" → (a,b,c). 파싱 실패 성분은 0(느슨한 파서).
fn parse_semver(v: &str) -> (u32, u32, u32) {
    let mut it = v.split(['.', '-', '+']);
    let a = it.next().and_then(|s| s.parse().ok()).unwrap_or(0);
    let b = it.next().and_then(|s| s.parse().ok()).unwrap_or(0);
    let c = it.next().and_then(|s| s.parse().ok()).unwrap_or(0);
    (a, b, c)
}

/// introduced > seen 인지(= seen 이후 새로 도입된 항목). seen=None 이면 항상 새 항목.
fn is_new_since(introduced: &str, seen: Option<&str>) -> bool {
    match seen {
        None => true,
        Some(s) => parse_semver(introduced) > parse_semver(s),
    }
}

/// 표시할 마법사 페이지 종류 목록(표시 순서). 환영·완료는 항상 포함.
/// 라이선스/오타교정은 버전 게이트, 언어팩/기본입력기는 버전 + 상태(미설치/미지정) 게이트.
fn build_wizard_pages(mode: WizardMode, seen: Option<&str>) -> Vec<i32> {
    // FirstRun(신규 설치)은 버전 무관 전체 표시. WhatsNew 는 introduced>seen 인 항목만.
    let show_all = mode == WizardMode::FirstRun || seen.is_none();
    let is_new = |intro: &str| show_all || is_new_since(intro, seen);

    let mut pages = vec![WIZ_WELCOME];
    if is_new(WIZ_INTRODUCED) {
        pages.push(WIZ_LICENSE);
    }
    if is_new(WIZ_INTRODUCED) && !platform::wizard_is_korean_language_installed() {
        pages.push(WIZ_LANGPACK);
    }
    if is_new(WIZ_INTRODUCED) && !platform::wizard_is_default_ime() {
        pages.push(WIZ_DEFAULT_IME);
    }
    if is_new(WIZ_INTRODUCED) {
        pages.push(WIZ_TYPEFIX);
    }
    pages.push(WIZ_FINISH);
    pages
}

/// 설치 마법사(`--first-run` / `--whats-new`) 배선.
///
/// CLI 인자가 없으면 통째로 건너뛰어 기존 일반 설정 모드로 동작(무회귀).
/// 진행 중에는 아무것도 저장하지 않고, [마침] 시에만 config 를 일괄 저장한다.
pub fn wire(ui: &SettingsWindow, config: &Rc<RefCell<Config>>) {
    if let Some(mode) = parse_wizard_mode() {
        let seen = platform::wizard_seen_version();
        let pages = Rc::new(build_wizard_pages(mode, seen.as_deref()));
        let idx = Rc::new(RefCell::new(0usize));

        ui.set_wizard_mode(true);
        ui.set_wiz_whats_new(mode == WizardMode::WhatsNew);

        // 현재 인덱스 → UI(페이지 종류·첫/마지막 여부) 반영.
        let apply = {
            let pages = pages.clone();
            let idx = idx.clone();
            let ui_weak = ui.as_weak();
            move || {
                if let Some(ui) = ui_weak.upgrade() {
                    let i = *idx.borrow();
                    ui.set_wiz_current_kind(pages[i]);
                    ui.set_wiz_is_first(i == 0);
                    ui.set_wiz_is_last(i + 1 >= pages.len());
                }
            }
        };
        apply();

        // 다음
        {
            let pages = pages.clone();
            let idx = idx.clone();
            let apply = apply.clone();
            ui.on_wizard_next(move || {
                {
                    let mut i = idx.borrow_mut();
                    if *i + 1 < pages.len() {
                        *i += 1;
                    }
                }
                apply();
            });
        }
        // 이전
        {
            let idx = idx.clone();
            let apply = apply.clone();
            ui.on_wizard_back(move || {
                {
                    let mut i = idx.borrow_mut();
                    if *i > 0 {
                        *i -= 1;
                    }
                }
                apply();
            });
        }
        // 한국어 언어팩: [언어 설정 열기] → ms-settings:regionlanguage.
        ui.on_wizard_open_langpack(|| platform::wizard_open_language_settings());

        // 마침: config load(공유본)→체크박스값 반영→save. 그 후 기본입력기 지정(체크 시)
        // + seen 버전 기록. '설정 열기' 체크면 그 자리에서 설정 모드로 전환, 아니면 닫기.
        {
            let ui_weak = ui.as_weak();
            let config = config.clone();
            // 기본 입력기 페이지가 실제로 표시된 흐름에서만 기본 지정을 적용한다.
            // (--whats-new 요약 모드처럼 페이지가 스킵되면 체크박스 기본값 true 로
            //  뜻하지 않게 기본 입력기를 바꾸는 것을 방지.)
            let showed_default_ime = pages.contains(&WIZ_DEFAULT_IME);
            ui.on_wizard_finish(move || {
                let ui = ui_weak.unwrap();
                {
                    // 시작 시 로드한 공유 config 에 마법사 체크박스값만 반영(다른 설정 유실 없음).
                    let mut cfg = config.borrow_mut();
                    cfg.engine.auto_typefix.forward = ui.get_atf_forward();
                    cfg.engine.auto_typefix.reverse = ui.get_atf_reverse();
                    // persist_config 로 수렴 — Linux 에서 데몬에 즉시 DBus 통지(저장 사이트
                    // 통지 누락 방지 불변식). save 직접 호출은 통지를 우회했다.
                    let _ = crate::persist_config(&cfg, "wizard_finish");
                }
                if showed_default_ime && ui.get_wiz_default_ime_checked() {
                    platform::wizard_set_as_default();
                    platform::wizard_set_default_on_startup(true);
                }
                platform::set_wizard_seen_version(env!("CARGO_PKG_VERSION"));
                if ui.get_wiz_open_settings_checked() {
                    // 재실행(뮤텍스 충돌) 대신 같은 창을 일반 설정 모드로 전환.
                    ui.set_wizard_mode(false);
                } else {
                    let _ = ui.window().hide();
                }
            });
        }
    }
}
