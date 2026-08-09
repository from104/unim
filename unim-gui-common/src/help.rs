//! 오프라인 사용자 매뉴얼(HTML) 경로 해석 및 열기 — Linux 공용.
//!
//! 설정앱(`unim-settings`)과 트레이(`unim-indicator`)가 **같은 해석기**를 쓴다.
//! 각자 복사해 두면 후보 순서가 서로 어긋난 채 조용히 다른 파일을 열게 되므로
//! 한 곳에 모은다. GNOME 확장(JS)은 언어가 달라 공유가 불가능하며,
//! `unim-gnome-extension/indicator.js` 가 동일 순서를 주석으로 명시해 복제한다.
//!
//! 툴킷 무관(std 전용) — `xdg-open`/`notify-send` 프로세스 spawn 만 사용한다.

/// 로케일 값이 C/POSIX(=번역 없음) 인지. `C`, `POSIX`, `C.UTF-8` 을 모두 잡는다.
fn is_c_locale(v: &str) -> bool {
    let base = v.split('.').next().unwrap_or(v);
    base.eq_ignore_ascii_case("C") || base.eq_ignore_ascii_case("POSIX")
}

/// 환경변수 조회를 주입받는 판정 본체(테스트용). 규칙은 [`ui_language_is_korean`] 참조.
fn ui_language_is_korean_from(get: impl Fn(&str) -> Option<String>) -> bool {
    // ① `LANGUAGE` 는 사용자가 명시한 우선순위 목록(콜론 구분, 예: `ko:en`)이라
    //    가장 강한 의사 표시다. C/POSIX 항목은 건너뛰고 첫 실질 항목을 본다.
    if let Some(s) = get("LANGUAGE") {
        if let Some(first) = s
            .split(':')
            .map(str::trim)
            .find(|e| !e.is_empty() && !is_c_locale(e))
        {
            return first.starts_with("ko");
        }
    }
    // ② 그다음 `LC_ALL` → `LC_MESSAGES` → `LANG`. **C/POSIX 는 건너뛴다** —
    //    개발 환경에서 출력 형식 고정용으로 `LC_ALL=C.UTF-8` 을 걸어 두는 일이
    //    흔한데, 그 값 하나 때문에 한국어 데스크톱에서 영문 매뉴얼이 열렸다
    //    (실기 확인 2026-07-28: LANGUAGE=ko:en · LANG=ko_KR.UTF-8 인데도 영문).
    for key in ["LC_ALL", "LC_MESSAGES", "LANG"] {
        if let Some(s) = get(key) {
            let s = s.trim();
            if s.is_empty() || is_c_locale(s) {
                continue;
            }
            return s.starts_with("ko");
        }
    }
    false
}

/// OS UI 언어가 한국어인지 — 로케일 환경변수로 판정.
///
/// 우선순위: `LANGUAGE`(콜론 목록의 첫 실질 항목) → `LC_ALL` → `LC_MESSAGES` → `LANG`.
/// 값이 `ko` 프리픽스(ko / ko_KR / ko_KR.UTF-8 …)면 한국어로 본다.
/// **C/POSIX 값은 판정에서 제외**한다 — "번역 없음" 은 "영어를 원한다" 와 다르고,
/// 실제로 앱 UI(Slint 번들 번역)는 한국어인데 도움말만 영어로 갈리는 불일치를 낳았다.
pub fn ui_language_is_korean() -> bool {
    ui_language_is_korean_from(|k| std::env::var(k).ok())
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

/// 매뉴얼을 사용자의 **기본 웹 브라우저**로 연다. 도움말 언어는 UI 언어 판정
/// (`ui_language_is_korean`)을 그대로 재사용해 자동 일치시킨다.
///
/// `xdg-open` 은 file 경로를 **text/html MIME 기본 핸들러**로 라우팅하는데,
/// VS Code 계열 IDE(antigravity 등)가 text/html 을 자기 앱으로 등록해 도움말이
/// 브라우저 대신 IDE 로 열리는 사례가 실재한다(실사용 보고). 도움말의 의도는
/// "브라우저에서 보는 문서"이므로 기본 브라우저를 명시적으로 우선하고,
/// 실패 시에만 종전 `xdg-open`(MIME 핸들러)으로 폴백한다.
///
/// 실패(파일 부재·양쪽 런처 부재)를 조용히 삼키지 않는다 — 버튼을 눌렀는데
/// 아무 일도 안 일어나는 것이 사용자에게 가장 나쁜 결과다.
pub fn open_help(caller_datadir: Option<&str>) {
    let korean = ui_language_is_korean();
    let Some(path) = find_help_file(korean, caller_datadir) else {
        notify_help_unavailable(korean);
        return;
    };
    if open_in_default_browser(&path) {
        return;
    }
    if std::process::Command::new("xdg-open")
        .arg(&path)
        .spawn()
        .is_err()
    {
        notify_help_unavailable(korean);
    }
}

/// `--new-window` 를 확실히 지원하는 브라우저 계열(실행 파일 basename 부분 일치).
///
/// 화이트리스트로 가는 이유: 브라우저는 모르는 인자를 **URL 로 오해**해 빈 탭이나
/// 검색 결과를 띄우는 경우가 있어, 무조건 붙였다가 실패하면 사용자가 도움말 대신
/// 엉뚱한 페이지를 본다. 목록에 없으면 새 창을 포기하고 종전 경로로 내려간다 —
/// "새 창" 보다 "도움말이 열리는 것" 이 우선이다.
const NEW_WINDOW_BROWSERS: &[&str] = &[
    "firefox",
    "librewolf",
    "waterfox",
    "floorp",
    "zen",
    "chrome",
    "chromium",
    "brave",
    "edge",
    "vivaldi",
    "opera",
    "epiphany",
];

fn supports_new_window(prog: &str) -> bool {
    let base = prog.rsplit('/').next().unwrap_or(prog).to_ascii_lowercase();
    NEW_WINDOW_BROWSERS.iter().any(|b| base.contains(b))
}

/// `.desktop` 의 `Exec=` 에서 실행 파일과 **고정 인자**를 뽑는다.
///
/// 필드 코드(`%u` `%U` `%f` …)와 flatpak 래퍼 표식(`@@u` `@@`)은 버린다. 고정 인자를
/// 살리는 이유는 flatpak/snap 브라우저가 `flatpak run --branch=stable org.mozilla.firefox`
/// 처럼 실제 실행 파일 앞에 인자를 두기 때문이다 — 이걸 버리면 실행 자체가 깨진다.
fn parse_desktop_exec(contents: &str) -> Option<(String, Vec<String>)> {
    let mut in_entry = false;
    for line in contents.lines() {
        let line = line.trim();
        if line.starts_with('[') {
            in_entry = line == "[Desktop Entry]";
            continue;
        }
        if !in_entry {
            continue;
        }
        let Some(rest) = line.strip_prefix("Exec=") else {
            continue;
        };
        let mut toks = rest
            .split_whitespace()
            .filter(|t| !t.starts_with('%') && !t.starts_with("@@"))
            .map(|t| t.trim_matches('"').to_string());
        let prog = toks.next()?;
        if prog.is_empty() {
            return None;
        }
        return Some((prog, toks.collect()));
    }
    None
}

/// 데스크톱 ID 로 `.desktop` 파일 경로를 찾는다(XDG 데이터 디렉터리 순회).
fn find_desktop_file(desktop_id: &str) -> Option<std::path::PathBuf> {
    use std::path::PathBuf;
    let mut roots: Vec<PathBuf> = Vec::new();
    if let Some(home) = std::env::var_os("XDG_DATA_HOME") {
        roots.push(PathBuf::from(home));
    } else if let Some(home) = std::env::var_os("HOME") {
        roots.push(PathBuf::from(home).join(".local/share"));
    }
    let dirs = std::env::var("XDG_DATA_DIRS")
        .unwrap_or_else(|_| "/usr/local/share:/usr/share".to_string());
    roots.extend(dirs.split(':').filter(|s| !s.is_empty()).map(PathBuf::from));
    roots
        .into_iter()
        .map(|r| r.join("applications").join(desktop_id))
        .find(|p| p.is_file())
}

/// 기본 웹 브라우저(`x-scheme-handler/http` 핸들러)로 파일을 연다. 성공 시 true.
///
/// 매뉴얼은 **항상 새 창**으로 연다 — 작업 중인 탭 사이에 끼어들면 사용자가 도움말을
/// 찾으려 탭을 뒤져야 하고, 조작 비용이 큰 사용자에게 그 탐색이 부담이 된다.
///
/// 순서: ① `.desktop` 의 `Exec` 을 직접 실행하며 `--new-window` 부여(화이트리스트
/// 계열만) → ② `gtk-launch`(새 창 보장 없음) → 호출부의 ③ `xdg-open`.
/// 모듈 계약(std 전용, GIO 의존 없음)은 그대로 지킨다.
fn open_in_default_browser(path: &std::path::Path) -> bool {
    let Ok(out) = std::process::Command::new("xdg-settings")
        .args(["get", "default-web-browser"])
        .output()
    else {
        return false;
    };
    if !out.status.success() {
        return false;
    }
    let desktop_id = String::from_utf8_lossy(&out.stdout).trim().to_string();
    if desktop_id.is_empty() {
        return false;
    }

    // ① 새 창 강제.
    if let Some((prog, args)) = find_desktop_file(&desktop_id)
        .and_then(|p| std::fs::read_to_string(p).ok())
        .and_then(|c| parse_desktop_exec(&c))
    {
        if supports_new_window(&prog)
            && std::process::Command::new(&prog)
                .args(&args)
                .arg("--new-window")
                .arg(path)
                .spawn()
                .is_ok()
        {
            return true;
        }
    }

    // ② gtk-launch 폴백. 앱을 띄우고 즉시 종료하므로 `status()` 대기는 짧고,
    //    종료 코드가 실제 실행 성공을 알려 준다(spawn 성공만으로는 미지 데스크톱
    //    ID 실패를 놓친다).
    let app = desktop_id.strip_suffix(".desktop").unwrap_or(&desktop_id);
    std::process::Command::new("gtk-launch")
        .arg(app)
        .arg(path)
        .status()
        .is_ok_and(|s| s.success())
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

    /// 로케일 판정 계약. 실기 회귀(2026-07-28): `LC_ALL=C.UTF-8` 하나 때문에
    /// `LANGUAGE=ko:en` · `LANG=ko_KR.UTF-8` 인 한국어 데스크톱에서 영문 매뉴얼이
    /// 열렸다. C/POSIX 는 "번역 없음" 이지 "영어 선호" 가 아니다.
    #[test]
    fn korean_detection_skips_c_locale_and_honors_language() {
        let env = |pairs: &[(&str, &str)]| {
            let owned: Vec<(String, String)> = pairs
                .iter()
                .map(|(k, v)| (k.to_string(), v.to_string()))
                .collect();
            move |k: &str| {
                owned
                    .iter()
                    .find(|(key, _)| key == k)
                    .map(|(_, v)| v.clone())
            }
        };

        // 실제로 터진 조합.
        assert!(ui_language_is_korean_from(env(&[
            ("LANGUAGE", "ko:en"),
            ("LANG", "ko_KR.UTF-8"),
            ("LC_ALL", "C.UTF-8"),
        ])));
        // LANGUAGE 가 없어도 C 계열은 건너뛰고 LANG 을 본다.
        assert!(ui_language_is_korean_from(env(&[
            ("LC_ALL", "C.UTF-8"),
            ("LANG", "ko_KR.UTF-8"),
        ])));
        // 영어 데스크톱은 그대로 영어.
        assert!(!ui_language_is_korean_from(env(&[
            ("LANGUAGE", "en_US:en"),
            ("LANG", "en_US.UTF-8"),
        ])));
        // LANGUAGE 가 명시적으로 영어면 LANG 이 한국어라도 영어(gettext 우선순위).
        assert!(!ui_language_is_korean_from(env(&[
            ("LANGUAGE", "en"),
            ("LANG", "ko_KR.UTF-8"),
        ])));
        // 아무것도 없으면 영어.
        assert!(!ui_language_is_korean_from(env(&[])));
        // C 만 있으면 영어(번역 자산이 없다는 뜻이므로 기본값 유지).
        assert!(!ui_language_is_korean_from(env(&[("LANG", "C")])));
    }

    /// `.desktop` `Exec` 파싱: 필드 코드는 버리고 flatpak 래퍼의 고정 인자는 살린다.
    /// 고정 인자를 버리면 flatpak/snap 브라우저 실행 자체가 깨진다.
    #[test]
    fn desktop_exec_parsing_keeps_wrapper_args_drops_field_codes() {
        let (prog, args) =
            parse_desktop_exec("[Desktop Entry]\nExec=/usr/bin/firefox %u\nName=Firefox\n").unwrap();
        assert_eq!(prog, "/usr/bin/firefox");
        assert!(args.is_empty());

        let (prog, args) = parse_desktop_exec(
            "[Desktop Entry]\nExec=/usr/bin/flatpak run --branch=stable org.mozilla.firefox @@u %u @@\n",
        )
        .unwrap();
        assert_eq!(prog, "/usr/bin/flatpak");
        assert_eq!(args, ["run", "--branch=stable", "org.mozilla.firefox"]);

        // [Desktop Action] 등 다른 섹션의 Exec 은 채택하지 않는다.
        assert!(parse_desktop_exec("[Desktop Action new]\nExec=/usr/bin/nope\n").is_none());
    }

    /// 새 창 화이트리스트는 실행 파일 basename 으로 판정한다 — 경로·접미사 변형
    /// (`google-chrome-stable`, `/usr/bin/firefox-esr`)을 모두 받아야 한다.
    #[test]
    fn new_window_whitelist_matches_real_browser_names() {
        assert!(supports_new_window("/usr/bin/firefox-esr"));
        assert!(supports_new_window("google-chrome-stable"));
        assert!(supports_new_window("/snap/bin/chromium"));
        assert!(supports_new_window("/usr/bin/microsoft-edge-stable"));
        // 미지 브라우저는 새 창을 포기하고 폴백(엉뚱한 페이지를 여느니 낫다).
        assert!(!supports_new_window("/usr/bin/some-unknown-browser"));
        assert!(!supports_new_window("/usr/bin/code"));
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
