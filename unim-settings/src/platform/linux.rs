//! Linux 플랫폼 백엔드.
//!
//! B2(Linux 실동작) 단계에서 로케일 판정·flock 싱글턴·DBus 저장 통지를 실구현한다.
//! 저장 통지는 GTK unim-settings-gtk 의 `save_and_notify`(settings_dialog.rs) 와 **동일
//! 메커니즘**: 파일 선저장은 호출부(main.rs `persist_config`) 책임이고, 여기서는
//! DBus `SetConfigYaml` fire-and-forget 만 담당한다.
//! 마법사 seen 버전은 XDG state 파일(`~/.local/state/unim/wizard-seen-version`)에
//! 저장한다(B4). `dirs` 는 이 크레이트의 직접 의존이 아니라(코어 `unim` 의 전이
//! 의존) 여기서 `dirs::state_dir()`→`data_dir()` 의 XDG 규칙만 재현하며, 코어
//! `src/paths.rs` 는 건드리지 않는다.

use unim::config::Config;

/// OS UI 언어가 한국어인지 — 로케일 환경변수로 판정.
/// 트레이(`unim-indicator`)와 판정이 갈리면 같은 데스크톱에서 앱마다 다른 언어의
/// 매뉴얼이 열리므로, 구현은 `unim_gui_common::help` 한 곳에 둔다.
pub fn ui_language_is_korean() -> bool {
    unim_gui_common::help::ui_language_is_korean()
}

/// 단일 인스턴스 가드 (Linux). `$XDG_RUNTIME_DIR/unim-settings.lock` 에 대해
/// **비블로킹 배타 flock** 을 시도한다. 잠금 획득 = 첫 인스턴스(`true`), 실패 =
/// 이미 실행 중(`false`, stderr 안내 후 호출자가 즉시 종료). Windows 와 달리 타
/// 프로세스 창 전면화는 하지 않는다(Wayland 은 원천적으로 불가 — 한계 문서화).
///
/// 잠금은 파일 디스크립터에 걸리므로 fd 를 **프로세스 수명 동안 열어 둬야** 한다
/// (`mem::forget`). 프로세스 종료 시 커널이 fd 를 닫으며 잠금을 자동 해제한다.
pub fn acquire_singleton_or_foreground() -> bool {
    use rustix::fs::{flock, FlockOperation};
    use std::fs::OpenOptions;
    use std::path::PathBuf;

    // XDG_RUNTIME_DIR 이 없으면 임시 디렉터리로 폴백.
    let dir = std::env::var_os("XDG_RUNTIME_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(std::env::temp_dir);
    let path = dir.join("unim-settings.lock");

    let file = match OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(false)
        .open(&path)
    {
        Ok(f) => f,
        // 락 파일을 못 열면 싱글턴 보장을 포기하고 계속 진행한다(과잉 차단 방지).
        Err(_) => return true,
    };
    match flock(&file, FlockOperation::NonBlockingLockExclusive) {
        Ok(()) => {
            std::mem::forget(file); // fd 를 열어 둔 채 유지 → 잠금 지속(종료 시 OS 해제).
            true
        }
        Err(_) => {
            eprintln!("unim-settings: 이미 실행 중입니다.");
            // Wayland 에서 타 프로세스 창 전면화는 불가하나(문서화된 한계), 무피드백은 별개다.
            // 데스크톱 알림으로 "이미 실행 중" 을 안내해 재시도 클릭 비용을 줄인다.
            // notify-send 미설치·실패는 조용히 무시(기능 저하 없음). spawn 으로 논블로킹.
            let (summary, body) = if ui_language_is_korean() {
                ("UNIM 설정", "설정 창이 이미 실행 중입니다.")
            } else {
                ("UNIM Settings", "The settings window is already running.")
            };
            let _ = std::process::Command::new("notify-send")
                .args(["--app-name=UNIM", "--icon=unim", summary, body])
                .spawn();
            false
        }
    }
}

/// 저장 후 데몬 통지 — GTK `save_and_notify` 와 동일하게 DBus `SetConfigYaml` 을
/// fire-and-forget 로 보낸다(데몬이 저장 + `ConfigChangedJson` 브로드캐스트).
/// 파일 선저장은 호출부(`persist_config`)의 책임이며, 데몬이 없어도 파일은 남는다.
pub fn notify_config_saved(cfg: &Config, label: &str) {
    unim_gui_common::settings_dbus::save_config_via_dbus(cfg, label);
}

// ── 오프라인 사용자 매뉴얼(HTML) ──

/// 매뉴얼을 기본 브라우저(`xdg-open`)로 연다. 경로 해석·언어 판정·실패 안내는
/// 트레이와 공유하는 `unim_gui_common::help` 가 담당한다.
///
/// `option_env!("UNIM_DATADIR")` 을 **여기서** 전개해 넘기는 이유: 이 매크로는
/// 전개된 크레이트의 컴파일 환경만 읽으므로, 이 크레이트의 `build.rs` 가 주입한
/// 값은 이 크레이트 안에서만 보인다. gui-common 도 자체 build.rs 로 같은 값을
/// 받지만(트레이용), 둘 중 하나만 주입된 빌드에서도 비표준 PREFIX 를 놓치지
/// 않도록 양쪽을 모두 후보에 넣는다.
pub fn open_help() {
    unim_gui_common::help::open_help(option_env!("UNIM_DATADIR"));
}

// ── 설치 마법사 플랫폼 훅 (Linux) ──
// seen 버전은 XDG state 파일로(아래), 기본 입력기 판정·지정은 im-config 의 진실
// 원본인 xinputrc(`run_im <name>`)로 실구현한다. 언어팩 감지는 Linux 에서 무의미해
// true 로 두어 build_wizard_pages 가 LANGPACK 페이지를 자연 스킵한다.

/// im-config 의 xinputrc 에서 활성 입력기 이름을 뽑는 순수 파서.
/// 첫 **비주석·비공백** 라인이 `run_im <name>` 형태면 `<name>` 을 돌려준다. 그 외
/// (빈 파일·주석뿐·다른 지시문 선행·이름 누락)는 `None`. CRLF·후행 공백에 내성.
fn parse_run_im(content: &str) -> Option<String> {
    for line in content.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        // 첫 유효 라인만 판정 — run_im 이 아니면(다른 지시문 선행) 곧바로 실패.
        let mut it = line.split_whitespace();
        if it.next() == Some("run_im") {
            return it.next().map(String::from);
        }
        return None;
    }
    None
}

/// 주어진 사용자/시스템 xinputrc 경로로 기본 입력기 판정(테스트 주입용 헬퍼).
/// 사용자 파일 우선, 읽기 실패 시 시스템 파일로 폴백해 `run_im unim` 이면 true.
/// 타 입력기·`default`·양쪽 부재·읽기 실패는 전부 false.
/// 세션 env(GTK_IM_MODULE 등)는 로그인 스냅샷이라 의도적으로 미참조 — 영속적
/// 진실은 xinputrc 다.
fn is_default_ime_at(user_rc: &std::path::Path, system_rc: &std::path::Path) -> bool {
    match std::fs::read_to_string(user_rc).or_else(|_| std::fs::read_to_string(system_rc)) {
        Ok(content) => parse_run_im(&content).as_deref() == Some("unim"),
        Err(_) => false,
    }
}

/// 기본 입력기가 unim 인지 — `~/.xinputrc` → (부재 시) `/etc/X11/xinit/xinputrc`
/// 의 `run_im` 지시문으로 판정. 부재·타 입력기·읽기 실패는 false 라, 마법사가
/// DEFAULT_IME 페이지를 노출해 지정을 유도한다. (테스트는 경로 주입형
/// `is_default_ime_at` 로 검증 — HOME env 오버라이드는 프로세스 전역이라 회피.)
pub fn wizard_is_default_ime() -> bool {
    use std::path::PathBuf;
    let system_rc = PathBuf::from("/etc/X11/xinit/xinputrc");
    match std::env::var_os("HOME").filter(|h| !h.is_empty()) {
        Some(home) => is_default_ime_at(&PathBuf::from(home).join(".xinputrc"), &system_rc),
        // HOME 부재: 사용자 파일이 없는 것과 동치 — 빈 경로는 읽기 실패라 시스템만 본다.
        None => is_default_ime_at(std::path::Path::new(""), &system_rc),
    }
}

/// 기본 입력기를 unim 으로 지정 — im-config 를 in-process 동기 실행한다(ms 단위라
/// finish 핸들러에서 호출해도 UI 를 막지 않는다).
///
/// BLOCKER-1(GAP-first-run-lifecycle-01): im-config 가 **실행됐으나 실패**(exit≠0 —
/// im-config 는 설치돼 있으므로 그 xinputrc 인프라도 있다고 볼 수 있음)한 경우에만
/// `~/.xinputrc` 직접 기록으로 폴백한다. im-config 바이너리 자체가 **없는**(`Err`,
/// 예: Fedora 는 im-config 를 패키징하지 않음) 경우엔 폴백을 시도하지 않는다 — 그
/// 배포판엔 `/usr/share/im-config/xinputrc.common` + `Xsession.d/70im-config_launch`
/// 도 없어, 파일만 써 봐야 아무도 읽지 않는 "거짓 성공" 이 되기 때문이다(종전 결함:
/// 두 경우를 구분하지 않고 항상 폴백해 Fedora 에서 성공 처리 후 seen 을 찍었다).
///
/// 반환: [`DefaultImeOutcome`] — 호출부(wizard.rs)가 이 값으로 실패 안내(재시도 가능)와
/// 수동 설정 안내(폴백 자체가 무의미)를 구분해 seen 보류 여부를 정한다. 모든 결과는
/// eprintln 로도 남긴다.
pub fn wizard_set_as_default() -> super::DefaultImeOutcome {
    use super::DefaultImeOutcome;
    match std::process::Command::new("im-config")
        .args(["-n", "unim"])
        .status()
    {
        Ok(st) if st.success() => {
            eprintln!("unim-settings: im-config -n unim — 기본 입력기 지정 완료.");
            DefaultImeOutcome::Success
        }
        Ok(st) => {
            // im-config 는 존재하지만 이번 실행이 실패 — xinputrc 인프라 자체는 있다고
            // 볼 수 있으므로 직접 기록 폴백이 여전히 유의미하다.
            eprintln!(
                "unim-settings: im-config -n unim 실패 (exit {:?}) — xinputrc 폴백 시도.",
                st.code()
            );
            if write_xinputrc_fallback() {
                DefaultImeOutcome::Success
            } else {
                DefaultImeOutcome::Failed
            }
        }
        Err(e) => {
            // im-config 자체가 없음(NotFound 등) — 폴백을 시도해도 무효과이므로 시도하지
            // 않고 수동 설정이 필요함을 그대로 알린다(거짓 성공 금지).
            eprintln!(
                "unim-settings: im-config 실행 불가({e}) — 이 배포판엔 im-config 가 없어 \
                 xinputrc 폴백도 무효과입니다. 수동 설정 필요."
            );
            DefaultImeOutcome::ManualSetupRequired
        }
    }
}

/// im-config 실행 실패(설치는 돼 있음) 시 `~/.xinputrc` 에 `run_im unim` 을 직접 기록하는
/// 폴백. im-config 산출 포맷과 동일해 이후 im-config 재실행과도 호환된다.
///
/// M-04(GAP-first-03): 기존 파일이 있으면(사용자 수동 커스터마이징일 수 있음) 덮어쓰기
/// 전에 `.xinputrc.bak` 으로 최선노력 백업한다 — 백업 실패는 로그만 남기고 기록은
/// 계속 진행한다(마법사 완주를 막지 않기 위함, BLOCKER 수정이 백업 실패로 막히면 안 됨).
/// 반환: 기록 성공이면 `true`, HOME 부재·쓰기 실패면 `false`.
fn write_xinputrc_fallback() -> bool {
    let Some(home) = std::env::var_os("HOME").filter(|h| !h.is_empty()) else {
        eprintln!("unim-settings: HOME 미설정 — xinputrc 폴백 기록 불가.");
        return false;
    };
    let home = std::path::PathBuf::from(home);
    let path = home.join(".xinputrc");
    if path.exists() {
        let bak = home.join(".xinputrc.bak");
        match std::fs::copy(&path, &bak) {
            Ok(_) => eprintln!(
                "unim-settings: 기존 {} 를 {} 로 백업.",
                path.display(),
                bak.display()
            ),
            Err(e) => eprintln!(
                "unim-settings: 기존 {} 백업 실패({e}) — 그래도 기록은 계속 진행.",
                path.display()
            ),
        }
    }
    match std::fs::write(&path, "# generated by unim-settings wizard\nrun_im unim\n") {
        Ok(()) => {
            eprintln!(
                "unim-settings: {} 에 run_im unim 기록 (im-config 폴백).",
                path.display()
            );
            true
        }
        Err(e) => {
            eprintln!("unim-settings: xinputrc 폴백 기록 실패: {e}");
            false
        }
    }
}

/// GNOME Shell(Wayland) 세션인지 — `XDG_SESSION_TYPE=wayland` + GNOME 데스크톱 식별로
/// 판정한다(우분투 기본 세션이 정확히 이 조합, BLOCKER-2/GAP-first-run-lifecycle-02).
/// im-config/xinputrc 경로와 활성화 수단이 달라(`gnome-extensions enable`), 마법사가
/// 이 세션에서 별도 안내 카드를 띄우는 데 쓴다. 자동 실행은 하지 않는다(사용자 승인
/// 없이 임의 명령을 실행하지 않기 위함) — 안내만.
pub fn is_gnome_wayland_session() -> bool {
    let is_wayland = std::env::var("XDG_SESSION_TYPE")
        .map(|v| v.eq_ignore_ascii_case("wayland"))
        .unwrap_or(false);
    let is_gnome = std::env::var("XDG_CURRENT_DESKTOP")
        .map(|v| v.to_ascii_uppercase().contains("GNOME"))
        .unwrap_or(false)
        || std::env::var_os("GNOME_SHELL_SESSION_MODE").is_some();
    is_wayland && is_gnome
}

/// GAP-first-06: ibus 또는 fcitx(4/5) 데몬이 함께 떠 있는지 감지한다. 두 입력기
/// 프레임워크가 공존하면 어느 쪽이 실제로 조합을 가로채는지 앱마다 달라질 수 있어
/// 완료 페이지에서 경고만 한다 — 사용자의 기존 워크플로를 임의로 끊지 않도록 자동
/// 종료·비활성화는 하지 않는다. `pgrep -x` 로 정확한 comm 매치만 본다(부분 매치로 인한
/// 오탐 방지). `pgrep` 미설치 등 조회 실패는 "감지 안 됨" 으로 조용히 처리한다.
pub fn detect_conflicting_ime() -> bool {
    ["ibus-daemon", "fcitx", "fcitx5"].iter().any(|name| {
        std::process::Command::new("pgrep")
            .args(["-x", name])
            .stdout(std::process::Stdio::null())
            .status()
            .map(|s| s.success())
            .unwrap_or(false)
    })
}

/// im-config 지정은 영속적이라 로그인마다 재고정할 필요가 없다(Windows 의 startup
/// 재고정 개념 없음). 함수 표면 대칭을 위한 no-op.
pub fn wizard_set_default_on_startup(_v: bool) {}

/// Windows 전용 페이지 — Linux 는 한국어 폰트/입력을 unim-common 의
/// `Recommends: fonts-noto-cjk` 로 대체하므로 항상 설치된 것으로 본다(true 면
/// build_wizard_pages 가 LANGPACK 페이지를 스킵).
pub fn wizard_is_korean_language_installed() -> bool {
    true
}

pub fn wizard_open_language_settings() {}

// XDG: 환경변수 값이 절대경로면 그대로, 아니면 무시하고 `$HOME/<suffix>` 로 폴백.
// `wizard_seen_version_path` 와 `wizard_error_log_path` 가 공유하는 기반 로직이라
// 모듈 스코프로 뺐다(종전엔 전자 안에 중첩 fn 이었음).
fn xdg_base(env_key: &str, home_suffix: &str) -> Option<std::path::PathBuf> {
    use std::path::PathBuf;
    if let Some(v) = std::env::var_os(env_key) {
        let p = PathBuf::from(v);
        if p.is_absolute() {
            return Some(p);
        }
    }
    match std::env::var_os("HOME") {
        Some(h) if !h.is_empty() => Some(PathBuf::from(h).join(home_suffix)),
        _ => None,
    }
}

/// `~/.local/state/unim`(XDG_STATE_HOME 우선) 또는 `~/.local/share/unim` 디렉터리.
/// seen 버전 파일과 GAP-first-05 오류 로그가 공유하는 기반 경로.
fn unim_state_dir() -> Option<std::path::PathBuf> {
    let base = xdg_base("XDG_STATE_HOME", ".local/state")
        .or_else(|| xdg_base("XDG_DATA_HOME", ".local/share"))?;
    Some(base.join("unim"))
}

/// 마법사 seen 버전을 저장할 XDG state 파일 경로. XDG 규격대로 환경변수 값은
/// **절대경로일 때만** 유효하고, 아니면 `$HOME` 폴백을 쓴다. `$HOME` 조차 없으면
/// `None`(저장·조회 모두 조용히 무시).
fn wizard_seen_version_path() -> Option<std::path::PathBuf> {
    Some(unim_state_dir()?.join("wizard-seen-version"))
}

/// GAP-first-05: 마법사 창 생성 실패 로그 경로(`~/.local/state/unim/wizard-error.log`).
/// autostart(`--first-run-if-needed`)가 렌더링 실패로 매 로그인 조용히 재시도만 하면
/// 사용자가 원인을 알 방법이 없다 — 실패 시 여기 남기고 데스크톱 알림으로 위치를 알린다.
fn wizard_error_log_path() -> Option<std::path::PathBuf> {
    Some(unim_state_dir()?.join("wizard-error.log"))
}

/// 마법사(설정 창) 생성 실패를 로그 파일에 append 하고, 가능하면 데스크톱 알림으로
/// 로그 위치를 안내한다(GAP-first-05). 실패해도(경로 없음·쓰기 실패) 조용히 무시 —
/// 이 함수 자체가 새로운 실패 경로가 되어서는 안 된다.
pub fn log_wizard_render_failure(msg: &str) {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    if let Some(path) = wizard_error_log_path() {
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        use std::io::Write;
        if let Ok(mut f) = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&path)
        {
            let _ = writeln!(f, "[unix {now}] {msg}");
        }
        let (summary, body) = if ui_language_is_korean() {
            (
                "UNIM 설정 창 생성 실패".to_string(),
                format!("자세한 내용은 {} 를 확인하세요.", path.display()),
            )
        } else {
            (
                "UNIM settings window failed to start".to_string(),
                format!("See {} for details.", path.display()),
            )
        };
        let _ = std::process::Command::new("notify-send")
            .args(["--app-name=UNIM", "--icon=unim", &summary, &body])
            .spawn();
    }
    eprintln!("unim-settings: wizard render failure: {msg}");
}

/// 마지막으로 마법사를 완주한 버전(있으면). 파일 부재·읽기 실패·공백은 `None` —
/// 그러면 `--whats-new` 도 전체 항목을 표시한다(seen 없음과 동일 취급).
pub fn wizard_seen_version() -> Option<String> {
    let path = wizard_seen_version_path()?;
    let content = std::fs::read_to_string(path).ok()?;
    let trimmed = content.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}

/// 마법사 완주 버전을 XDG state 파일에 한 줄로 기록. 디렉터리는 필요 시 생성하고,
/// 실패(경로 없음·권한 등)는 조용히 무시한다 — 마법사 UX 를 막지 않기 위함.
pub fn set_wizard_seen_version(v: &str) {
    let Some(path) = wizard_seen_version_path() else {
        return;
    };
    if let Some(parent) = path.parent() {
        if std::fs::create_dir_all(parent).is_err() {
            return;
        }
    }
    let _ = std::fs::write(path, format!("{v}\n"));
}

#[cfg(test)]
mod tests {
    use super::*;

    // 도움말 경로 후보의 순서 계약 테스트는 해석기와 함께
    // `unim-gui-common/src/help.rs` 로 이동했다 (트레이·설정앱 공용).

    /// XDG_STATE_HOME 을 유니크 임시 디렉터리로 지정하면(절대경로) 경로 해석이
    /// 그 하위 `unim/wizard-seen-version` 로 고정된다 — HOME 폴백을 타지 않아 헐메틱.
    /// seen 버전 저장→조회 왕복과 한 줄(개행 포함) 기록·트림 계약을 검증한다.
    #[test]
    fn seen_version_roundtrip_under_xdg_state_home() {
        let uniq = format!(
            "unim-settings-b4-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        );
        let tmp = std::env::temp_dir().join(uniq);
        std::fs::create_dir_all(&tmp).unwrap();
        assert!(tmp.is_absolute());

        let prev = std::env::var_os("XDG_STATE_HOME");
        std::env::set_var("XDG_STATE_HOME", &tmp);

        // 최초에는 파일이 없어 None.
        assert_eq!(wizard_seen_version(), None);

        // 경로가 XDG_STATE_HOME 하위로 잡히는지 확인.
        let expected = tmp.join("unim").join("wizard-seen-version");
        assert_eq!(wizard_seen_version_path().as_deref(), Some(expected.as_path()));

        // 저장 → 한 줄(개행 포함)로 기록 → 조회 시 트림되어 되돌아온다.
        set_wizard_seen_version("0.3.63");
        assert_eq!(std::fs::read_to_string(&expected).unwrap(), "0.3.63\n");
        assert_eq!(wizard_seen_version(), Some("0.3.63".to_string()));

        // 주변 공백이 있어도 트림 후 반환.
        std::fs::write(&expected, "  1.2.3  \n").unwrap();
        assert_eq!(wizard_seen_version(), Some("1.2.3".to_string()));

        // 빈 파일은 None(= seen 없음과 동일 취급).
        std::fs::write(&expected, "\n").unwrap();
        assert_eq!(wizard_seen_version(), None);

        // 환경 원복 + 정리.
        match prev {
            Some(v) => std::env::set_var("XDG_STATE_HOME", v),
            None => std::env::remove_var("XDG_STATE_HOME"),
        }
        let _ = std::fs::remove_dir_all(&tmp);
    }

    /// `parse_run_im` 순수 파서: 첫 유효 라인만 판정, 주석/공백/CRLF/이름누락 계약.
    #[test]
    fn parse_run_im_variants() {
        // 단순 케이스.
        assert_eq!(parse_run_im("run_im unim\n").as_deref(), Some("unim"));
        // 선행 주석·빈 줄 스킵 후 첫 유효 라인.
        assert_eq!(
            parse_run_im("# im-config(8) generated\n\n  run_im ibus\n").as_deref(),
            Some("ibus")
        );
        // CRLF 내성(str::lines + trim 이 \r 제거).
        assert_eq!(
            parse_run_im("# c\r\nrun_im fcitx5\r\n").as_deref(),
            Some("fcitx5")
        );
        // `run_im default` → "default"(unim 아님 → is_default_ime 는 false).
        assert_eq!(parse_run_im("run_im default\n").as_deref(), Some("default"));
        // 탭 구분·후행 공백 내성.
        assert_eq!(parse_run_im("run_im\tunim  \n").as_deref(), Some("unim"));
        // 빈 파일 / 주석만 → None.
        assert_eq!(parse_run_im(""), None);
        assert_eq!(parse_run_im("# only\n#more\n"), None);
        // 첫 유효 라인이 run_im 이 아니면 뒤에 run_im 이 있어도 None.
        assert_eq!(parse_run_im("export FOO=1\nrun_im unim\n"), None);
        // 이름 누락 → None.
        assert_eq!(parse_run_im("run_im\n"), None);
    }

    /// `is_default_ime_at` 경로 주입 헬퍼: 사용자 우선·시스템 폴백·부재/타입력기 false.
    /// HOME env 를 건드리지 않고 임시 파일 경로만 주입해 헐메틱하게 검증한다.
    #[test]
    fn is_default_ime_at_precedence_and_values() {
        use std::path::Path;

        let uniq = format!(
            "unim-settings-s4-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        );
        let dir = std::env::temp_dir().join(uniq);
        std::fs::create_dir_all(&dir).unwrap();
        let user = dir.join("user-xinputrc");
        let system = dir.join("system-xinputrc");
        let missing = dir.join("does-not-exist");
        let also_missing = dir.join("also-missing");

        // 사용자 파일이 unim → true(시스템 미참조).
        std::fs::write(&user, "run_im unim\n").unwrap();
        assert!(is_default_ime_at(&user, &missing));

        // 사용자 파일이 타 입력기 → false(존재하므로 시스템 unim 을 보지 않음).
        std::fs::write(&user, "run_im ibus\n").unwrap();
        std::fs::write(&system, "run_im unim\n").unwrap();
        assert!(!is_default_ime_at(&user, &system));

        // 사용자 부재 → 시스템(unim) 폴백 → true.
        assert!(is_default_ime_at(&missing, &system));

        // 사용자 부재 + 시스템이 default → false.
        std::fs::write(&system, "run_im default\n").unwrap();
        assert!(!is_default_ime_at(&missing, &system));

        // 둘 다 부재 → false. 빈 경로(HOME 부재 분기 대리)도 읽기 실패 → 시스템 폴백.
        assert!(!is_default_ime_at(&missing, &also_missing));
        assert!(!is_default_ime_at(Path::new(""), &also_missing));

        let _ = std::fs::remove_dir_all(&dir);
    }

    /// BLOCKER-2(GAP-first-run-lifecycle-02): GNOME Shell(Wayland) 세션 감지 — 두
    /// 조건(Wayland 세션 + GNOME 데스크톱)이 모두 있어야 true. env 를 건드리므로
    /// 저장/복원한다(이 파일의 다른 env 테스트와 동일 관례).
    #[test]
    fn gnome_wayland_session_requires_both_conditions() {
        let prev_session = std::env::var_os("XDG_SESSION_TYPE");
        let prev_desktop = std::env::var_os("XDG_CURRENT_DESKTOP");
        let prev_mode = std::env::var_os("GNOME_SHELL_SESSION_MODE");

        std::env::set_var("XDG_SESSION_TYPE", "wayland");
        std::env::set_var("XDG_CURRENT_DESKTOP", "ubuntu:GNOME");
        std::env::remove_var("GNOME_SHELL_SESSION_MODE");
        assert!(is_gnome_wayland_session(), "Wayland + GNOME 데스크톱 → true");

        std::env::set_var("XDG_SESSION_TYPE", "x11");
        assert!(!is_gnome_wayland_session(), "X11 세션은 대상 아님(false)");

        std::env::set_var("XDG_SESSION_TYPE", "wayland");
        std::env::set_var("XDG_CURRENT_DESKTOP", "KDE");
        assert!(!is_gnome_wayland_session(), "비-GNOME 데스크톱은 false");

        std::env::remove_var("XDG_CURRENT_DESKTOP");
        std::env::set_var("GNOME_SHELL_SESSION_MODE", "user");
        assert!(
            is_gnome_wayland_session(),
            "XDG_CURRENT_DESKTOP 부재라도 GNOME_SHELL_SESSION_MODE 로 판정"
        );

        match prev_session {
            Some(v) => std::env::set_var("XDG_SESSION_TYPE", v),
            None => std::env::remove_var("XDG_SESSION_TYPE"),
        }
        match prev_desktop {
            Some(v) => std::env::set_var("XDG_CURRENT_DESKTOP", v),
            None => std::env::remove_var("XDG_CURRENT_DESKTOP"),
        }
        match prev_mode {
            Some(v) => std::env::set_var("GNOME_SHELL_SESSION_MODE", v),
            None => std::env::remove_var("GNOME_SHELL_SESSION_MODE"),
        }
    }

    /// M-04(GAP-first-03): xinputrc 폴백이 기존 파일을 백업 없이 덮어쓰지 않는다.
    /// HOME 을 유니크 임시 디렉터리로 돌려 헐메틱하게 검증(다른 테스트와 병렬해도 안전).
    #[test]
    fn xinputrc_fallback_backs_up_existing_file() {
        let uniq = format!(
            "unim-settings-m04-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        );
        let home = std::env::temp_dir().join(uniq);
        std::fs::create_dir_all(&home).unwrap();

        let prev_home = std::env::var_os("HOME");
        std::env::set_var("HOME", &home);

        let xinputrc = home.join(".xinputrc");
        std::fs::write(&xinputrc, "run_im my-custom-ime\n").unwrap();

        assert!(write_xinputrc_fallback());

        // 기존 커스터마이징이 백업으로 보존됨.
        let bak = home.join(".xinputrc.bak");
        assert_eq!(
            std::fs::read_to_string(&bak).unwrap(),
            "run_im my-custom-ime\n"
        );
        // 새 파일은 폴백 포맷으로 갱신됨.
        assert_eq!(
            std::fs::read_to_string(&xinputrc).unwrap(),
            "# generated by unim-settings wizard\nrun_im unim\n"
        );

        match prev_home {
            Some(v) => std::env::set_var("HOME", v),
            None => std::env::remove_var("HOME"),
        }
        let _ = std::fs::remove_dir_all(&home);
    }
}
