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
/// `LC_ALL` → `LC_MESSAGES` → `LANG` 순서로 **첫 비어있지 않은 값**을 채택하고,
/// 그 값이 `ko` 프리픽스(ko / ko_KR / ko_KR.UTF-8 …)면 한국어로 본다.
pub fn ui_language_is_korean() -> bool {
    for key in ["LC_ALL", "LC_MESSAGES", "LANG"] {
        if let Some(val) = std::env::var_os(key) {
            let s = val.to_string_lossy();
            if !s.is_empty() {
                return s.starts_with("ko");
            }
        }
    }
    false
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

    let file = match OpenOptions::new().create(true).write(true).open(&path) {
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

// ── 설치 마법사 플랫폼 훅 (Linux) ──
// seen 버전만 XDG state 파일로 실구현한다(아래). 언어팩·기본입력기 감지는 Linux
// 에서 무의미하므로 true/no-op 로 두면 build_wizard_pages 가 두 페이지를 자연 스킵한다.
pub fn wizard_is_default_ime() -> bool {
    true
}

pub fn wizard_set_as_default() {}

pub fn wizard_set_default_on_startup(_v: bool) {}

pub fn wizard_is_korean_language_installed() -> bool {
    true
}

pub fn wizard_open_language_settings() {}

/// 마법사 seen 버전을 저장할 XDG state 파일 경로.
/// `dirs::state_dir()`(=`$XDG_STATE_HOME` 또는 `~/.local/state`)를 우선하고, 없으면
/// `dirs::data_dir()`(=`$XDG_DATA_HOME` 또는 `~/.local/share`)로 폴백해 `unim/` 하위에
/// 둔다. XDG 규격대로 환경변수 값은 **절대경로일 때만** 유효하고, 아니면 `$HOME`
/// 폴백을 쓴다. `$HOME` 조차 없으면 `None`(저장·조회 모두 조용히 무시).
fn wizard_seen_version_path() -> Option<std::path::PathBuf> {
    use std::path::PathBuf;

    // XDG: 환경변수 값이 절대경로면 그대로, 아니면 무시하고 `$HOME/<suffix>` 로 폴백.
    fn xdg_base(env_key: &str, home_suffix: &str) -> Option<PathBuf> {
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

    let base = xdg_base("XDG_STATE_HOME", ".local/state")
        .or_else(|| xdg_base("XDG_DATA_HOME", ".local/share"))?;
    Some(base.join("unim").join("wizard-seen-version"))
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
}
