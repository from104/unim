//! 오프라인 사용자 매뉴얼(HTML) 경로 해석 및 열기 — Linux 공용.
//!
//! 설정앱(`unim-settings`)과 트레이(`unim-indicator`)가 **같은 해석기**를 쓴다.
//! 각자 복사해 두면 후보 순서가 서로 어긋난 채 조용히 다른 파일을 열게 되므로
//! 한 곳에 모은다. GNOME 확장(JS)은 언어가 달라 공유가 불가능하며,
//! `unim-gnome-extension/indicator.js` 가 동일 순서를 주석으로 명시해 복제한다.
//!
//! 툴킷 무관(std 전용) — `xdg-open`/`notify-send` 프로세스 spawn 만 사용한다.

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

/// 매뉴얼 파일명 — Makefile·MSI·도움말 생성기와 공유하는 고정 계약.
pub const HELP_FILE_KO: &str = "unim-help-ko.html";
pub const HELP_FILE_EN: &str = "unim-help-en.html";

/// 매뉴얼 HTML 의 실제 경로. 후보를 순서대로 훑어 **파일이 존재하는 첫 항목**을 채택한다.
///
/// `caller_datadir` 는 호출 크레이트의 `option_env!("UNIM_DATADIR")` — 아래 참조.
pub fn find_help_file(korean: bool, caller_datadir: Option<&str>) -> Option<std::path::PathBuf> {
    let name = if korean { HELP_FILE_KO } else { HELP_FILE_EN };
    let exe = std::env::current_exe().ok();
    help_dir_candidates(exe.as_deref(), caller_datadir)
        .into_iter()
        .map(|d| d.join(name))
        .find(|p| p.is_file())
}

/// 후보 디렉터리 목록(우선순위 순). `exe` 는 `current_exe()`(테스트에서는 주입).
///
/// ① `caller_datadir` → ② 이 크레이트의 `UNIM_DATADIR` → ③ `/usr/share`
/// → ④ `/usr/local/share` → ⑤ 개발 폴백(실행 파일의 조상 디렉터리에서 `help/`
/// 탐색 — `target/debug` 든 `target/release` 든 저장소 루트에 닿는다).
///
/// 경로를 하드코딩하지 않는 이유: Makefile 의 `PREFIX ?= /usr/local` 때문에 설치
/// 위치가 갈린다(deb/rpm 은 `PREFIX=/usr`, 소스 빌드는 `/usr/local`). 주입값은
/// "정답을 후보 맨 앞에 세우는" 최적화이지 필수 조건이 아니다 — 미주입이어도
/// ③④⑤ 가 받아낸다.
///
/// ①②를 **둘 다** 보는 이유: `option_env!` 는 그 매크로가 전개된 크레이트의 컴파일
/// 환경만 읽는다. 해석기를 여기로 옮긴 뒤 ②만 남기면 자체 build.rs 로 주입하던
/// `unim-settings` 의 값이 사라지고, ①만 남기면 build.rs 가 없는 소비자
/// (`unim-indicator`)가 비표준 PREFIX 를 영영 못 찾는다. 값이 같으면 ②는 건너뛴다.
pub fn help_dir_candidates(
    exe: Option<&std::path::Path>,
    caller_datadir: Option<&str>,
) -> Vec<std::path::PathBuf> {
    use std::path::PathBuf;

    let own_datadir = option_env!("UNIM_DATADIR");
    let mut dirs: Vec<PathBuf> = Vec::new();
    // 첫 등장 위치만 남긴다 — 우선순위는 보존하면서 같은 경로를 두 번 stat 하지
    // 않는다. deb/rpm 은 `DATADIR=/usr/share` 라 주입값이 아래 하드코딩 후보와
    // 그대로 겹치고, 워크스페이스 일괄 빌드에서는 ①②가 같은 값을 받는다.
    let mut push = |dir: PathBuf| {
        if !dirs.contains(&dir) {
            dirs.push(dir);
        }
    };
    for datadir in [caller_datadir, own_datadir].into_iter().flatten() {
        push(PathBuf::from(datadir).join("unim").join("help"));
    }
    push(PathBuf::from("/usr/share/unim/help"));
    push(PathBuf::from("/usr/local/share/unim/help"));
    if let Some(exe) = exe {
        for ancestor in exe.ancestors() {
            push(ancestor.join("help"));
        }
    }
    dirs
}

/// 매뉴얼을 기본 브라우저(`xdg-open`)로 연다. 도움말 언어는 UI 언어 판정
/// (`ui_language_is_korean`)을 그대로 재사용해 자동 일치시킨다.
///
/// 실패(파일 부재·`xdg-open` 미설치)를 조용히 삼키지 않는다 — 버튼을 눌렀는데
/// 아무 일도 안 일어나는 것이 사용자에게 가장 나쁜 결과다.
pub fn open_help(caller_datadir: Option<&str>) {
    let korean = ui_language_is_korean();
    let Some(path) = find_help_file(korean, caller_datadir) else {
        notify_help_unavailable(korean);
        return;
    };
    if std::process::Command::new("xdg-open")
        .arg(&path)
        .spawn()
        .is_err()
    {
        notify_help_unavailable(korean);
    }
}

/// 도움말을 열지 못했을 때의 사용자 안내. stderr(터미널 실행 시)와 데스크톱 알림
/// 양쪽에 남긴다. `notify-send` 미설치·실패는 무시(stderr 는 이미 남았다).
///
/// 안내 문구는 `rust_i18n` 이 아니라 `korean` 인자로 분기한다 — 이 함수는 로케일을
/// 초기화하지 않는 프로세스(`unim-settings` 는 Slint 번들 번역을 쓴다)에서도
/// 호출되므로, 전역 locale 상태에 기대면 영어로 새어 나간다.
fn notify_help_unavailable(korean: bool) {
    let (summary, body) = if korean {
        (
            "UNIM 도움말",
            "도움말 파일을 찾지 못했습니다. unim-common 패키지가 설치되어 있는지 확인해 주세요.",
        )
    } else {
        (
            "UNIM Help",
            "Could not find the help file. Check that the unim-common package is installed.",
        )
    };
    eprintln!("unim: {body}");
    let _ = std::process::Command::new("notify-send")
        .args(["--app-name=UNIM", "--icon=unim", summary, body])
        .spawn();
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 도움말 후보 경로의 **순서 계약**: 주입 DATADIR → 설치 경로(`/usr` →
    /// `/usr/local`) → 개발 폴백. 개발 폴백은 실행 파일에서 위로 올라가며 `help/` 를
    /// 찾는다(`target/debug/unim-settings` → 저장소 루트). PREFIX 하드코딩 회귀 방지.
    #[test]
    fn help_dir_candidates_prefer_install_then_dev_fallback() {
        let exe = std::path::Path::new("/repo/target/debug/unim-settings");
        let dirs = help_dir_candidates(Some(exe), None);

        let usr = dirs.iter().position(|d| d.as_os_str() == "/usr/share/unim/help");
        let usr_local = dirs
            .iter()
            .position(|d| d.as_os_str() == "/usr/local/share/unim/help");
        let repo_root = dirs.iter().position(|d| d.as_os_str() == "/repo/help");

        assert!(usr.is_some() && usr_local.is_some() && repo_root.is_some());
        assert!(usr < usr_local, "/usr 가 /usr/local 보다 먼저여야 한다");
        assert!(usr_local < repo_root, "설치 경로가 개발 폴백보다 먼저여야 한다");
    }

    /// 호출 크레이트가 주입한 `UNIM_DATADIR`(비표준 PREFIX)은 **맨 앞**에 선다.
    /// 이 순서가 뒤집히면 `/usr` 에 남은 구버전 매뉴얼이 먼저 잡힌다.
    #[test]
    fn caller_datadir_comes_first() {
        let dirs = help_dir_candidates(None, Some("/opt/unim/share"));
        assert_eq!(
            dirs.first().map(|d| d.as_os_str()),
            Some(std::ffi::OsStr::new("/opt/unim/share/unim/help")),
        );
    }

    /// 같은 경로는 후보에 한 번만 실린다. deb/rpm 의 `DATADIR=/usr/share` 는 주입값이
    /// 하드코딩 후보와 그대로 겹치고, 워크스페이스 일괄 빌드에서는 ①②가 같은 값을
    /// 받는다 — 어느 쪽이든 중복 stat 이 생기지 않아야 한다.
    #[test]
    fn duplicate_datadir_is_not_repeated() {
        let dirs = help_dir_candidates(None, Some("/usr/share"));
        let mut seen = dirs.clone();
        seen.sort();
        seen.dedup();
        assert_eq!(seen.len(), dirs.len(), "후보에 중복 경로가 있으면 안 된다");
        assert_eq!(
            dirs.first().map(|d| d.as_os_str()),
            Some(std::ffi::OsStr::new("/usr/share/unim/help")),
        );
    }

    /// 도움말 HTML 이 아직 없어도(생성 전 상태) 탐색은 패닉 없이 `None` 로 흐른다 —
    /// `open_help()` 는 그 `None` 을 받아 사용자 안내로 이어진다. `current_exe()` 를
    /// 못 얻는 상황(`None`)도 후보 생성이 견디는지 함께 확인한다.
    #[test]
    fn missing_help_file_resolves_to_none_without_panic() {
        let never_exists = "unim-help-does-not-exist.html";
        let hit = help_dir_candidates(None, None)
            .into_iter()
            .map(|d| d.join(never_exists))
            .find(|p| p.is_file());
        assert!(hit.is_none());
    }
}
