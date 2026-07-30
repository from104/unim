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
    parse_wizard_mode_from(std::env::args().skip(1))
}

/// 인자 이터레이터를 주입받는 파서(테스트 용이성 — `parse_wizard_mode` 위임 대상).
///
/// `--first-run-if-needed` 는 autostart 게이트(seen 없음)를 통과한 첫 로그인에서
/// `--first-run` 과 동일하게 전체 마법사를 연다(seen 있으면 main() 최상단에서
/// [`autostart_should_exit_quietly`] 로 이미 조용히 종료됨).
fn parse_wizard_mode_from(args: impl Iterator<Item = String>) -> Option<WizardMode> {
    for a in args {
        match a.as_str() {
            "--first-run" => return Some(WizardMode::FirstRun),
            "--first-run-if-needed" => return Some(WizardMode::FirstRun),
            "--whats-new" => return Some(WizardMode::WhatsNew),
            _ => {}
        }
    }
    None
}

/// autostart(`--first-run-if-needed`) 전용 조용한 게이트.
///
/// 인자에 `--first-run-if-needed` 가 있고 마법사 완료 기록([`platform::wizard_seen_version`])이
/// 이미 있으면 `true`. main() 최상단(싱글턴 락 취득 이전)에서 검사해 창·flock·notify 없이
/// 즉시 종료하기 위한 것 — 매 로그인 마법사 재출현(나그)을 막는다. 비용 = 파일 read 1회.
pub fn autostart_should_exit_quietly() -> bool {
    should_exit_quietly_from(
        std::env::args().skip(1),
        platform::wizard_seen_version().as_deref(),
    )
}

/// 인자·seen 을 주입받는 순수 판정 (테스트 용이성).
fn should_exit_quietly_from(mut args: impl Iterator<Item = String>, seen: Option<&str>) -> bool {
    let has_flag = args.any(|a| a == "--first-run-if-needed");
    has_flag && seen.is_some()
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
        // FirstRun(Linux): '설정 열기'를 기본 해제 → Enter 연타 완주가 설정 창 없이 곧장
        // 끝나(추가 닫기 클릭 제거, 기현 접근성) 재로그인 후 바로 한글 입력으로 이어진다.
        // WhatsNew 는 새 기능 탐색을 위해 기본 체크 유지. Windows 는 Slint 기본값(true)을
        // 유지해 회귀 0 (cfg! 상수 false → set 미실행).
        if cfg!(target_os = "linux") {
            ui.set_wiz_open_settings_checked(mode == WizardMode::WhatsNew);
        }
        // Slint 문구·카드 분기용 플랫폼 플래그. cfg! 라 Windows 바이너리에선 상수 false →
        // ternary/적용시점 카드가 전부 Windows 경로로 렌더(렌더 트리 불변).
        ui.set_wiz_platform_linux(cfg!(target_os = "linux"));
        // BLOCKER-2(GAP-first-run-lifecycle-02): GNOME Shell(Wayland) 세션(우분투 기본
        // 세션)이면 완료 페이지에 확장 활성화 안내 카드를 띄운다. Windows/fallback 은
        // 항상 false 라 렌더 트리 불변(회귀 0).
        //
        // 단 **이미 확장을 켠 사용자에게는 띄우지 않는다** — 세션 종류만 보고 띄우면
        // 할 필요 없는 명령을 실행하라고 지시하게 된다(실기 확인 2026-07-28).
        ui.set_wiz_gnome_wayland(
            platform::is_gnome_wayland_session() && platform::gnome_extension_needs_enable(),
        );

        // FINISH 페이지 최초 진입 시 데몬 감지를 딱 1회 트리거하기 위한 가드(Linux 전용).
        #[cfg(target_os = "linux")]
        let daemon_probed = Rc::new(std::cell::Cell::new(false));

        // 현재 인덱스 → UI(페이지 종류·첫/마지막 여부) 반영.
        let apply = {
            let pages = pages.clone();
            let idx = idx.clone();
            let ui_weak = ui.as_weak();
            #[cfg(target_os = "linux")]
            let daemon_probed = daemon_probed.clone();
            move || {
                if let Some(ui) = ui_weak.upgrade() {
                    let i = *idx.borrow();
                    ui.set_wiz_current_kind(pages[i]);
                    ui.set_wiz_is_first(i == 0);
                    ui.set_wiz_is_last(i + 1 >= pages.len());
                    // 완료 페이지 진입 시 UNIM 데몬 상태를 백그라운드로 감지(1회).
                    // GAP-first-06: 같은 1회 진입에 ibus/fcitx 공존도 동기 감지한다
                    // (pgrep 조회 1~2회라 데몬 감지처럼 스레드로 뺄 만큼 무겁지 않음).
                    #[cfg(target_os = "linux")]
                    if pages[i] == WIZ_FINISH && !daemon_probed.replace(true) {
                        ui.set_wiz_conflicting_ime(platform::detect_conflicting_ime());
                        spawn_daemon_probe(ui.as_weak());
                    }
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
        ui.on_wizard_open_langpack(platform::wizard_open_language_settings);

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
                    let outcome = platform::wizard_set_as_default();
                    platform::wizard_set_default_on_startup(true);
                    // Linux: 재시도 가능한 실행 실패(Failed)와 자동 지정 수단 자체가 없는
                    // 경우(ManualSetupRequired, BLOCKER-1/Fedora 등)를 분리해 각각 다른
                    // FINISH 카드를 띄우고 seen 기록을 보류(다음 로그인 재안내)한 뒤 흐름을
                    // 중단한다. Windows 는 cfg! 상수 false 라 이 분기를 타지 않아 종전 동작
                    // (실패 무시·완료)을 그대로 유지한다(회귀 0).
                    if cfg!(target_os = "linux") {
                        match outcome {
                            platform::DefaultImeOutcome::Success => {}
                            platform::DefaultImeOutcome::Failed => {
                                ui.set_wiz_default_ime_failed(true);
                                return;
                            }
                            platform::DefaultImeOutcome::ManualSetupRequired => {
                                // im-config 자체가 없는 배포판(Fedora 등)은 재시도가 원리상
                                // 무의미한 조건이다 — 그 배포판에 im-config 가 생기지 않는 한
                                // 몇 번을 다시 눌러도 같은 결과다. seen 을 보류하면 "재시도
                                // 불가"인데 "매 로그인 영구 재출현"이라는 별개의 UX 회귀가
                                // 생긴다(검증 지적). 그래서 카드는 그대로 띄우되(거짓 성공
                                // 금지·수동 지정 안내는 유지) seen 은 이 자리에서 즉시
                                // 기록해 이 안내가 평생 한 번만 뜨게 한다. 체크박스도 꺼서
                                // 다음 [마침] 클릭이 이 분기를 다시 타지 않고 그대로 창을
                                // 닫게 한다 — 발 마우스 사용자가 반복 조작 없이 한 번의
                                // 추가 클릭으로 마법사를 끝낼 수 있도록.
                                ui.set_wiz_default_ime_checked(false);
                                ui.set_wiz_default_ime_manual_setup(true);
                                platform::set_wizard_seen_version(env!("CARGO_PKG_VERSION"));
                                return;
                            }
                        }
                    }
                }
                platform::set_wizard_seen_version(env!("CARGO_PKG_VERSION"));
                if ui.get_wiz_open_settings_checked() {
                    // 마법사도 config 저장 사이트다 — 설정 모드로 넘어가는 순간 상태줄이
                    // 생기므로, 죽어 있는 키가 있으면 그 첫 화면에서 알린다(B1 단일
                    // 규칙의 마법사 판). 창을 닫는 분기(마침 후 종료)는 상태줄이 없으므로
                    // 대상이 아니다.
                    if let Some(warn) =
                        crate::invalid_keys_status(&ui, &crate::collect_invalid_keys(&config.borrow()))
                    {
                        ui.set_status_text(warn);
                    }
                    // 재실행(뮤텍스 충돌) 대신 같은 창을 일반 설정 모드로 전환.
                    ui.set_wizard_mode(false);
                } else {
                    let _ = ui.window().hide();
                }
            });
        }

        // 창 X 닫기(마침 없이 종료) seen 기록 정책:
        // - Windows / Linux WhatsNew: 기록한다. Windows 는 `--first-run` 수동 기동이라 X
        //   닫기 = 의도적 종료(재진입은 수동), Linux WhatsNew 는 업데이트 노트 '읽음' 처리.
        // - Linux FirstRun: 미완주 X 닫기는 기록하지 **않는다** → 다음 로그인에 마법사가
        //   다시 뜬다(autostart 게이트는 seen 이 없으면 통과). 오직 [마침]만 seen 을 남겨,
        //   기본 입력기 미지정 상태로 마법사가 한 번의 오클릭으로 영구 소실되는 것을 막는다
        //   (§Q-B1 재검토). 이 '다음 로그인 재출현' 자체가 터미널 없는 GUI 재진입 경로다.
        // HideWindow 반환으로 기존 종료 경로 유지(마지막 창 hide → run() 반환).
        let record_seen_on_close = if cfg!(target_os = "linux") {
            mode != WizardMode::FirstRun
        } else {
            true
        };
        ui.window().on_close_requested(move || {
            if record_seen_on_close {
                platform::set_wizard_seen_version(env!("CARGO_PKG_VERSION"));
            }
            slint::CloseRequestResponse::HideWindow
        });
    }
}

/// 완료 페이지 진입 시 UNIM 데몬 상태를 감지해 `wiz-daemon-state` 를 갱신한다(Linux).
///
/// detach 스레드 + tokio current-thread 런타임(`settings_dbus::spawn_set_config_yaml` 패턴)
/// 으로 세션 버스를 조회한다 — 이벤트 루프를 차단하지 않는다. 창이 먼저 닫히면
/// `invoke_from_event_loop` 가 실패하고 no-op(weak upgrade 실패)로 안전 강하한다.
#[cfg(target_os = "linux")]
fn spawn_daemon_probe(ui: slint::Weak<SettingsWindow>) {
    std::thread::spawn(move || {
        // 1=실행 중, 2=자동 시작 대기(미기동/조회 실패 — 재로그인 시 시작).
        let state = probe_daemon_state();
        let _ = slint::invoke_from_event_loop(move || {
            if let Some(ui) = ui.upgrade() {
                ui.set_wiz_daemon_state(state);
            }
        });
    });
}

/// 세션 버스에서 UNIM 데몬 소유 여부를 확인하고, 없으면 자동활성(Peer.Ping)을 유도한다.
/// 반환: 실행 중이면 1, 그 외(미기동·활성 실패·버스 없음)면 2.
#[cfg(target_os = "linux")]
fn probe_daemon_state() -> i32 {
    let rt = match tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
    {
        Ok(rt) => rt,
        Err(_) => return 2,
    };
    rt.block_on(async {
        match daemon_running().await {
            Ok(true) => 1,
            _ => 2,
        }
    })
}

/// `NameHasOwner`(부작용 없는 조회) → false 면 `Peer.Ping` 으로 자동활성 서비스를
/// 즉석 기동한 뒤 재조회. 활성화에 성공하면 "지금 바로" 항목이 실제로 참이 된다.
#[cfg(target_os = "linux")]
async fn daemon_running() -> zbus::Result<bool> {
    use unim_gui_common::types::UNIM_BUS_NAME;

    let conn = zbus::Connection::session().await?;
    let dbus = zbus::fdo::DBusProxy::new(&conn).await?;
    let name: zbus::names::BusName = UNIM_BUS_NAME.try_into()?;

    if dbus.name_has_owner(name.clone()).await? {
        return Ok(true);
    }
    // 자동활성 .service(unim-common) 를 Peer.Ping 으로 기동 시도 후 재확인.
    // 활성화 실패(파일 부재 등)는 무시하고 최종 소유 여부만 신뢰한다.
    let _ = try_activate_daemon(&conn).await;
    Ok(dbus.name_has_owner(name).await?)
}

/// 데몬 well-known 이름으로 `org.freedesktop.DBus.Peer.Ping` 을 보내 D-Bus 자동활성을 유도.
/// (Ping 은 대상 객체 경로와 무관하므로 데몬의 기본 경로를 사용한다.)
#[cfg(target_os = "linux")]
async fn try_activate_daemon(conn: &zbus::Connection) -> zbus::Result<()> {
    use unim_gui_common::types::UNIM_BUS_NAME;

    let peer = zbus::fdo::PeerProxy::builder(conn)
        .destination(UNIM_BUS_NAME)?
        .path("/org/atit/unim/InputMethod")?
        .build()
        .await?;
    // Peer.Ping 은 fdo::Error 를 반환 — `?` 로 zbus::Error 로 변환해 반환 타입을 맞춘다.
    peer.ping().await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// &[&str] → String 이터레이터 (인자 주입 헬퍼).
    fn args(v: &[&str]) -> impl Iterator<Item = String> {
        v.iter().map(|s| s.to_string()).collect::<Vec<_>>().into_iter()
    }

    #[test]
    fn parse_mode_first_run_variants() {
        // --first-run 과 --first-run-if-needed 모두 FirstRun(전체 마법사)로 파싱.
        assert!(matches!(
            parse_wizard_mode_from(args(&["--first-run"])),
            Some(WizardMode::FirstRun)
        ));
        assert!(matches!(
            parse_wizard_mode_from(args(&["--first-run-if-needed"])),
            Some(WizardMode::FirstRun)
        ));
        assert!(matches!(
            parse_wizard_mode_from(args(&["--whats-new"])),
            Some(WizardMode::WhatsNew)
        ));
        // 다른 앞선 인자가 있어도 뒤의 플래그를 찾는다.
        assert!(matches!(
            parse_wizard_mode_from(args(&["--other", "--first-run-if-needed"])),
            Some(WizardMode::FirstRun)
        ));
        assert!(parse_wizard_mode_from(args(&["--other"])).is_none());
        assert!(parse_wizard_mode_from(args(&[])).is_none());
    }

    #[test]
    fn exit_quietly_needs_flag_and_seen() {
        // 플래그 있음 + seen 있음 → 조용히 종료(true).
        assert!(should_exit_quietly_from(
            args(&["--first-run-if-needed"]),
            Some("0.3.63")
        ));
        // 플래그 있음 + seen 없음(첫 로그인) → 마법사 표시(false).
        assert!(!should_exit_quietly_from(args(&["--first-run-if-needed"]), None));
        // 플래그 없음 → seen 유무와 무관하게 게이트 미적용(false).
        assert!(!should_exit_quietly_from(args(&["--first-run"]), Some("0.3.63")));
        assert!(!should_exit_quietly_from(args(&["--whats-new"]), None));
        assert!(!should_exit_quietly_from(args(&[]), Some("0.3.63")));
    }
}
