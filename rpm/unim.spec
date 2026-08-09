Name:           unim
Version:        0.4.0
Release:        1%{?dist}
Summary:        Universal Next-generation Input Method Engine (Korean IME)
Summary(ko):    범용 차세대 한글 입력기 엔진

License:        MIT
URL:            https://github.com/from104/unim
Source0:        https://github.com/from104/unim/archive/v%{version}.tar.gz#/%{name}-%{version}.tar.gz

# deb 릴리스가 ddeb(디버그 심볼)를 게시하지 않는 정책을 미러 — debuginfo/
# debugsource 서브패키지를 생성하지 않는다 (rpm 11개 카운트 게이트 유지).
%global debug_package %{nil}

# ─── BuildRequires ────────────────────────────────────────────────────────────
# debian/control:5-23 Build-Depends 의 Fedora 미러.
# qt6-declarative-dev 는 저장소 내 사용처가 없는 잔존 의존(unim-gui-qt 제거
# 이후)이라 도입하지 않는다. libqt6dbus6 은 qt6-qtbase 본체가 번들.
BuildRequires:  gcc
BuildRequires:  gcc-c++
BuildRequires:  make
BuildRequires:  cmake
BuildRequires:  pkgconf-pkg-config
BuildRequires:  chrpath
BuildRequires:  python3
# msgfmt — make gnome-extension 의 .po → .mo 컴파일. 부재 시 Makefile 가드가
# 무음 스킵하여 unim-gnome 이 번역 없이 포장되므로 반드시 명시한다.
BuildRequires:  gettext
# Cargo.lock v4 → Rust 1.78+ (Cargo.toml rust-version). Fedora 43 stable 충족.
BuildRequires:  rust >= 1.78
BuildRequires:  cargo >= 1.78
BuildRequires:  glib2-devel
BuildRequires:  dbus-devel
BuildRequires:  gtk3-devel
BuildRequires:  gtk4-devel
BuildRequires:  libadwaita-devel
BuildRequires:  qt5-qtbase-devel
BuildRequires:  qt5-qtbase-private-devel
BuildRequires:  qt6-qtbase-devel
BuildRequires:  qt6-qtbase-private-devel
BuildRequires:  wayland-devel
BuildRequires:  libxkbcommon-devel
BuildRequires:  libX11-devel
BuildRequires:  libxcb-devel

# ─── Meta-package Requires (main `unim` rpm is the meta) ─────────────────────
# debian/control:210-219 미러 (indicator/popup-service → desktop 통합,
# settings/keymap-studio/typing-practice 추가).
Requires:       %{name}-common = %{version}-%{release}
Requires:       %{name}-im-gtk = %{version}-%{release}
Requires:       %{name}-im-qt = %{version}-%{release}
Requires:       %{name}-xim = %{version}-%{release}
Requires:       %{name}-wayland = %{version}-%{release}
Requires:       %{name}-desktop = %{version}-%{release}
Requires:       %{name}-settings = %{version}-%{release}
Requires:       %{name}-keymap-studio = %{version}-%{release}
Requires:       %{name}-typing-practice = %{version}-%{release}
Requires:       %{name}-gnome = %{version}-%{release}

%description
UNIM (Universal Next-generation Input Method) is a modular Korean Input Method
Engine written in Rust. It supports the Dubeolsik (2-bul) and Sebeolsik (3-bul
390/391/no-shift/Ahnmatae) keyboard layouts, moachigi (chord-based) input,
automatic Korean/English typo correction (AutoTypeFix), Hanja conversion with
bookmarks, and special-character/emoji popups.

Frontends: GTK3/4 IM modules, Qt5/6 platform input context plugins, XIM (X11),
Wayland input-method-v2 + virtual-keyboard-v1, and a GNOME Shell extension.

%description -l ko
UNIM은 Rust로 작성된 모듈형 한글 입력기 엔진입니다. 두벌식 및 세벌식
(390/391/NoShift/안마태) 배열, 모아치기(코드 기반) 입력, 자동 한영 오타 교정
(AutoTypeFix), 한자 변환(즐겨찾기 포함), 특수문자/이모지 팝업을 지원합니다.

프론트엔드: GTK3/4 IM 모듈, Qt5/6 플랫폼 입력 컨텍스트 플러그인, XIM(X11),
Wayland input-method-v2 + virtual-keyboard-v1, GNOME Shell 확장.

# ─── Subpackages ─────────────────────────────────────────────────────────────

%package common
Summary:        UNIM Korean IME — core engine, daemon, CLI, shared library
Summary(ko):    UNIM 한글 입력기 — 핵심 엔진, 데몬, CLI, 공유 라이브러리
Requires:       dbus
# debian/control:36-37 미러. im-config 는 Debian 전용이라 Fedora 미선언
# (imsettings 런타임 통합은 미구현 — 알려진 갭). 비프 도구는 rich dep.
# [요검증 → linux-rpm.yml 리허설의 repoquery 게이트가 확정]
Recommends:     google-noto-sans-cjk-fonts
Recommends:     (pulseaudio-utils or alsa-utils)

%description common
Core components required by every UNIM installation:
  * libunim_capi shared library (stable C-API consumed by IM modules)
  * unim-daemon — central engine, D-Bus session-activated
  * unim-cli — unified command-line interface (incl. config subcommand)
  * D-Bus service file, icons, im-config integration data, man pages

%description common -l ko
모든 UNIM 설치에 필요한 핵심 구성요소:
  * libunim_capi 공유 라이브러리
  * unim-daemon — 중앙 엔진 (D-Bus 세션 활성화)
  * unim-cli — 통합 커맨드라인 인터페이스
  * D-Bus 서비스 파일, 아이콘, im-config 데이터, man 페이지

%package im-gtk
Summary:        UNIM Korean IME — GTK3/GTK4 input method modules
Summary(ko):    UNIM 한글 입력기 — GTK3/GTK4 입력 메서드 모듈
Requires:       %{name}-common = %{version}-%{release}
# debian/control:60 'unim-xim | unim-wayland' 미러 (rpm rich dependency).
Recommends:     (%{name}-xim or %{name}-wayland)

%description im-gtk
GTK3 and GTK4 input method modules. Enables Korean typing in every GTK-based
application (GNOME, Xfce, MATE, Cinnamon, GIMP, Inkscape, etc.).

%description im-gtk -l ko
GTK3/GTK4 입력 메서드 모듈. GTK 기반 애플리케이션에서 한글 입력을 활성화합니다.

%package im-qt
Summary:        UNIM Korean IME — Qt5/Qt6 platform input context plugins
Summary(ko):    UNIM 한글 입력기 — Qt5/Qt6 플랫폼 입력 컨텍스트 플러그인
Requires:       %{name}-common = %{version}-%{release}
Recommends:     (%{name}-xim or %{name}-wayland)

%description im-qt
Qt5 and Qt6 platform input context plugins. Enables Korean typing in every
Qt-based application (KDE Plasma, Telegram Desktop, Qt Creator, etc.).

%description im-qt -l ko
Qt5/Qt6 플랫폼 입력 컨텍스트 플러그인. Qt 기반 애플리케이션에서 한글 입력을 활성화합니다.

%package xim
Summary:        UNIM Korean IME — XIM protocol frontend (X11)
Summary(ko):    UNIM 한글 입력기 — XIM 프로토콜 프론트엔드 (X11)
Requires:       %{name}-common = %{version}-%{release}

%description xim
X11 XIM protocol server. Required in X11 sessions and for legacy applications
that only speak XIM (xterm, emacs -nw, Java/AWT apps, etc.).

%description xim -l ko
X11 XIM 프로토콜 서버. X11 세션 및 XIM만 지원하는 레거시 애플리케이션용.

%package wayland
Summary:        UNIM Korean IME — Wayland input-method frontend
Summary(ko):    UNIM 한글 입력기 — Wayland 입력 메서드 프론트엔드
Requires:       %{name}-common = %{version}-%{release}

%description wayland
Wayland input-method-v2 + virtual-keyboard-v1 protocol server with
hanja/special-character popup via zwp_input_popup_surface_v2.
Required in pure Wayland sessions (KDE Plasma Wayland, Sway, Hyprland, etc.).

%description wayland -l ko
Wayland input-method-v2 + virtual-keyboard-v1 프로토콜 서버.
순수 Wayland 세션(KDE Plasma Wayland, Sway, Hyprland 등)에 필요합니다.

%package desktop
Summary:        UNIM Korean IME — desktop UI (tray indicator, settings, popups)
Summary(ko):    UNIM 한글 입력기 — 데스크톱 UI (트레이, 설정, 팝업)
Requires:       %{name}-common = %{version}-%{release}
Recommends:     %{name}-im-gtk
# 구 rpm 3분할(0.3.0 spec 의 indicator / settings(GTK) / popup-service) 승계.
# debian/control:119-126 Replaces/Breaks 의 rpm 등가. 구 GTK 'settings' 는
# 동명의 신규 Slint 패키지가 정상 업그레이드로 승계하므로 여기 미기재.
Obsoletes:      %{name}-indicator < 0.4.0
Obsoletes:      %{name}-popup-service < 0.4.0
Obsoletes:      unim-gui-qt < 0.3.0

%description desktop
GTK4 desktop user-interface bundle for non-GNOME desktops (KDE Plasma, Xfce,
Cinnamon, MATE). Bundles the tray indicator (StatusNotifierItem), the
unim-settings-gtk settings dialog (Adw.PreferencesWindow) and the D-Bus
activated popup service for hanja/special-character/emoji popups.

%description desktop -l ko
비-GNOME 데스크톱용 GTK4 UI 번들: 트레이 인디케이터, unim-settings-gtk 설정
대화상자, D-Bus 활성화 팝업 서비스(한자/특수문자/이모지).

%package settings
Summary:        UNIM Korean IME — settings app & first-run wizard (Slint)
Summary(ko):    UNIM 한글 입력기 — 설정 앱·첫 실행 마법사 (Slint)
Requires:       %{name}-common = %{version}-%{release}
Recommends:     fontconfig

%description settings
Cross-platform settings application (Slint, winit + Skia) sharing one codebase
with the Windows build. Includes the first-run wizard (--first-run,
--whats-new) and its per-login autostart gate (--first-run-if-needed).

%description settings -l ko
Windows 빌드와 코드베이스를 공유하는 크로스플랫폼 설정 앱(Slint, winit+Skia).
첫 실행 마법사(--first-run/--whats-new)와 로그인당 autostart 게이트 포함.

%package keymap-studio
Summary:        UNIM Keymap Studio — view and edit Hangul keymaps
Summary(ko):    UNIM 키맵 스튜디오 — 한글 키맵 열람·편집
Requires:       %{name}-common = %{version}-%{release}

%description keymap-studio
GTK4/libadwaita tool that opens any v1/v2/v3 Hangul keymap profile, shows the
4xN key layout with per-key metadata, and edits keys, jamo combinations, rule
sets and the moachigi marker. Saves to ~/.config/unim/layouts/.

%description keymap-studio -l ko
GTK4/libadwaita 한글 키맵 열람·편집 도구. ~/.config/unim/layouts/ 에 저장.

%package typing-practice
Summary:        UNIM Typing Practice — WPM/accuracy/heatmap on any Hangul keymap
Summary(ko):    UNIM 타자 연습 — WPM/정확도/히트맵
Requires:       %{name}-common = %{version}-%{release}

%description typing-practice
GTK4/libadwaita typing-practice tool for the currently selected UNIM Hangul
keymap: live WPM, CPM, accuracy, error rate and a per-key error heatmap.

%description typing-practice -l ko
현재 선택된 UNIM 한글 키맵으로 타자 연습: WPM/CPM/정확도/오타율/히트맵.

%package gnome
Summary:        UNIM Korean IME — GNOME Shell extension
Summary(ko):    UNIM 한글 입력기 — GNOME Shell 확장
BuildArch:      noarch
# debian/control:193-196 미러: common + desktop + gnome-shell.
Requires:       %{name}-common = %{version}-%{release}
Requires:       %{name}-desktop = %{version}-%{release}
Requires:       gnome-shell

%description gnome
Native GNOME Shell extension. Adds a top-panel indicator, hotkey management,
per-app input-mode rules, and virtual keyboard / preedit overlay on Wayland.

%description gnome -l ko
네이티브 GNOME Shell 확장. 상단 패널 인디케이터, 단축키 관리, 앱별 입력 모드 규칙 제공.

# ─── Build & Install ─────────────────────────────────────────────────────────

%prep
%autosetup -n %{name}-%{version}

%build
# debian/rules:62-63 미러: make build + make gnome-extension.
# MULTIARCH= 강제: Fedora 에는 dpkg-architecture 가 없고 gcc -print-multiarch 도
# 빈 값이지만(Makefile:16-25 → REAL_LIBDIR=LIBDIR), 환경 편차에 흔들리지 않게
# 명시적으로 비워 REAL_LIBDIR == %%{_libdir}(/usr/lib64) 를 결정론화한다.
%make_build MULTIARCH= \
            PREFIX=%{_prefix} \
            LIBDIR=%{_libdir} \
            LIBEXECDIR=%{_libexecdir} \
            BINDIR=%{_bindir} \
            DATADIR=%{_datadir} \
            SYSCONFDIR=%{_sysconfdir} \
            INCLUDEDIR=%{_includedir} \
            build gnome-extension

%install
%make_install MULTIARCH= \
              PREFIX=%{_prefix} \
              LIBDIR=%{_libdir} \
              LIBEXECDIR=%{_libexecdir} \
              BINDIR=%{_bindir} \
              DATADIR=%{_datadir} \
              SYSCONFDIR=%{_sysconfdir} \
              INCLUDEDIR=%{_includedir}
# debian/rules:71-76 미러 — 잔존 RUNPATH 제거 (check-rpaths 방어).
find %{buildroot} -type f \( -name '*.so' -o -name '*.so.*' \) \
     -exec chrpath -d {} + 2>/dev/null || :

# ─── Scriptlets ──────────────────────────────────────────────────────────────
# GTK3 immodule 캐시: Fedora gtk3 는 %%{_libdir}/gtk-3.0/3.0.0/immodules 파일
# 트리거로 gtk-query-immodules-3.0-64 를 자동 실행하므로 deb 의
# unim-im-gtk.postinst/postrm 대응 %%post/%%postun 이 불필요하다.
# [요검증 → 리허설: rpm -q --filetriggers gtk3] 아이콘/desktop 파일도 Fedora
# 파일트리거가 처리. 아래는 deb *.prerm 의 pkill 미러 — rpm %%preun 은
# erase($1=0)와 upgrade 시 구패키지 측($1=1) 모두 실행된다. **M-30**: upgrade
# 시($1=1)에도 죽이면 재연결 수단이 없는 프런트엔드가 그대로 남아 dnf upgrade
# 마다 입력이 정지한다 — deb 쪽도 remove 전용으로 축소했으므로(M-30) 여기서도
# erase($1=0) 로만 한정한다. pkill(procps-ng) 부재는 || : 로 무해.

# M-30: %%preun 이 erase($1=0) 전용으로 축소돼 dnf upgrade 중에는 구 버전
# unim-daemon 을 죽이지 않으므로, 새 버전 IM 모듈(.so)이 새로 뜨는 앱에 로드되는
# 동안 구 데몬이 재로그인 전까지 계속 떠 있는 혼합 버전 상태가 된다. 지금은 D-Bus
# 계약이 0.3.x↔0.4.0 호환이라 즉시 문제는 아니지만, 안내 없이 넘어가면 다음
# 릴리스에서 메서드가 하나라도 추가될 때 조용한 오작동이 된다. $1 >= 2 는 업그레이드
# (신규 설치 후 구버전이 이미 하나 이상 존재)를 뜻한다.
%post common
if [ "$1" -ge 2 ]; then
    echo "unim-common: UNIM 이(가) 업데이트되었습니다. 새 버전을 적용하려면 재로그인하거나 'unim-daemon --replace' 를 실행하세요." >&2
    echo "unim-common: UNIM has been updated. Log out and back in, or run 'unim-daemon --replace', to switch to the new version." >&2
fi

%preun common
if [ "$1" -eq 0 ]; then
    _user="$(logname 2>/dev/null || echo root)"
    pkill -u "$_user" -x unim-daemon 2>/dev/null || :
fi

%preun desktop
if [ "$1" -eq 0 ]; then
    _user="$(logname 2>/dev/null || echo root)"
    pkill -u "$_user" -x unim-indicator 2>/dev/null || :
    pkill -u "$_user" -f '(^|/)unim-settings-gtk( |$)' 2>/dev/null || :
fi

%preun settings
if [ "$1" -eq 0 ]; then
    _user="$(logname 2>/dev/null || echo root)"
    # comm "unim-settings"(13자)는 procps 15자 절단에 안전 → -x 정확 매치.
    pkill -u "$_user" -x unim-settings 2>/dev/null || :
fi

%preun wayland
if [ "$1" -eq 0 ]; then
    _user="$(logname 2>/dev/null || echo root)"
    pkill -u "$_user" -x unim-wayland 2>/dev/null || :
fi

%preun xim
if [ "$1" -eq 0 ]; then
    _user="$(logname 2>/dev/null || echo root)"
    pkill -u "$_user" -x unim-xim 2>/dev/null || :
fi

# M-05: 제거 시 im-config/xinputrc 롤백 안내(경고만) — deb 의
# debian/unim-common.postrm 과 동일 근거. %%postun 은 root 권한으로 로그인
# 세션 밖에서 실행되어 사용자 세션 상태를 알 수 없고 다른 IME 설정을 잘못
# 건드릴 위험이 있어 `im-config -n auto` 를 여기서 자동 실행하지 않는다.
%postun common
if [ "$1" -eq 0 ]; then
    if command -v im-config >/dev/null 2>&1; then
        echo "unim-common: UNIM 제거됨 — 기본 입력기가 여전히 unim 으로 지정돼 있다면" >&2
        echo "  다음 로그인 세션에서 'im-config -n auto' 를 실행해 되돌리세요." >&2
    else
        echo "unim-common: UNIM 제거됨 — ~/.xinputrc 에 'run_im unim' 이 남아있다면" >&2
        echo "  직접 편집하거나 삭제해 다른 입력기가 다시 뜨도록 하세요." >&2
    fi
fi

# ─── %files ──────────────────────────────────────────────────────────────────

%files common
# M-28: deb 는 unim-common.docs 로 NOTICE·LICENSES/*(BSD-3 hanja 사전·Unicode
# CLDR 서드파티 고지)를 이미 동봉한다 — rpm 도 동일하게 맞춘다.
%license LICENSE NOTICE LICENSES/*
%doc README.md
%{_libdir}/libunim_capi.so
%{_libdir}/libunim_capi.so.0
%{_libdir}/libunim_capi.so.0.1.0
%{_includedir}/unim.h
%{_bindir}/unim-cli
%{_libexecdir}/unim-daemon
%{_datadir}/dbus-1/services/org.atit.unim.InputMethod.service
%{_datadir}/im-config/data/25_unim.conf
%{_datadir}/im-config/data/25_unim.rc
%{_sysconfdir}/xdg/autostart/unim-daemon.desktop
%{_mandir}/man1/unim.1*
%{_mandir}/man1/unim-cli.1*
%{_datadir}/icons/hicolor/scalable/apps/unim-korean.svg
%{_datadir}/icons/hicolor/scalable/apps/unim-english.svg
# 오프라인 도움말 HTML — %%install 의 %%make_install 이 install-core 에서 함께 설치하므로
# 별도 install 지시가 불필요하다. 디렉터리도 이 패키지 소유(다른 서브패키지가
# %%{_datadir}/unim 을 쓰지 않는다).
%dir %{_datadir}/unim
%dir %{_datadir}/unim/help
%{_datadir}/unim/help/unim-help-ko.html
%{_datadir}/unim/help/unim-help-en.html

%files im-gtk
%{_libdir}/gtk-3.0/3.0.0/immodules/im-unim.so
%{_libdir}/gtk-4.0/4.0.0/immodules/libim-unim.so

%files im-qt
%{_libdir}/qt5/plugins/platforminputcontexts/libunim.so
%{_libdir}/qt6/plugins/platforminputcontexts/libunim.so

%files xim
%{_libexecdir}/unim-xim

%files wayland
%{_libexecdir}/unim-wayland

%files desktop
%{_bindir}/unim-indicator
%{_sysconfdir}/xdg/autostart/io.github.from104.unim.Indicator.desktop
%{_datadir}/icons/hicolor/scalable/apps/io.github.from104.unim.Indicator.svg
%{_mandir}/man1/unim-indicator.1*
%{_bindir}/unim-settings-gtk
%{_datadir}/applications/io.github.from104.unim.Settings.desktop
%{_datadir}/icons/hicolor/scalable/apps/io.github.from104.unim.Settings.svg
%{_mandir}/man1/unim-settings-gtk.1*
%{_bindir}/unim-popup-service
%{_datadir}/dbus-1/services/org.atit.unim.PopupService.service
%{_mandir}/man1/unim-popup-service.1*

%files settings
%{_bindir}/unim-settings
%{_datadir}/applications/io.github.from104.unim.SettingsSlint.desktop
%{_sysconfdir}/xdg/autostart/io.github.from104.unim.FirstRun.desktop
%{_datadir}/icons/hicolor/scalable/apps/io.github.from104.unim.SettingsSlint.svg
%{_mandir}/man1/unim-settings.1*

%files keymap-studio
%{_bindir}/unim-keymap-studio
%{_datadir}/applications/io.github.from104.unim.KeymapStudio.desktop
%{_datadir}/icons/hicolor/scalable/apps/io.github.from104.unim.KeymapStudio.svg
%{_mandir}/man1/unim-keymap-studio.1*

%files typing-practice
%{_bindir}/unim-typing-practice
%{_datadir}/applications/io.github.from104.unim.TypingPractice.desktop
%{_datadir}/icons/hicolor/scalable/apps/io.github.from104.unim.TypingPractice.svg
%{_mandir}/man1/unim-typing-practice.1*

%files gnome
%{_datadir}/gnome-shell/extensions/unim-gnome@from104.github.io/

# ─── Meta-package files (main `unim` rpm is the meta — no own files) ────────
%files
# meta-package — Requires declared at top of spec, no files of its own

# ─── Changelog ───────────────────────────────────────────────────────────────

%changelog
* Sun Aug 09 2026 from104 <from104@gmail.com> - 0.4.0-1
- Restructure to mirror the Debian 11-package layout: new unim-desktop bundle
  (replaces the indicator/settings-gtk/popup-service split), new Slint
  unim-settings (settings app + first-run wizard), new unim-keymap-studio and
  unim-typing-practice packages
- Add gettext BuildRequires so GNOME extension translations (.mo) are built
- Disable debuginfo subpackages to mirror the deb release asset policy
- Force MULTIARCH= so REAL_LIBDIR resolves to %%{_libdir} deterministically

* Tue May 19 2026 from104 <from104@gmail.com> - 0.3.0-1
- Initial RPM packaging for UNIM 0.3.0
- Moachigi v4 Atomic Window Principle chord engine
- Popup-service replaces unim-gui-qt (GTK4 unified implementation)
- XIM commit_then_preedit best-effort fix
- Ahnmatae (안마태) and Qwerty Sebeolsik v2 layouts added then cleaned up
