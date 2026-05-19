Name:           unim
Version:        0.3.0
Release:        1%{?dist}
Summary:        Universal Next-generation Input Method Engine (Korean IME)
Summary(ko):    범용 차세대 한글 입력기 엔진

License:        MIT
URL:            https://github.com/from104/unim
Source0:        https://github.com/from104/unim/archive/v%{version}.tar.gz#/%{name}-%{version}.tar.gz

# ─── BuildRequires ────────────────────────────────────────────────────────────
BuildRequires:  cargo >= 1.75
BuildRequires:  rust
BuildRequires:  cmake
BuildRequires:  pkg-config
BuildRequires:  python3
BuildRequires:  glib2-devel
BuildRequires:  dbus-devel
BuildRequires:  gtk3-devel
BuildRequires:  gtk4-devel
BuildRequires:  libadwaita-devel
BuildRequires:  qt5-qtbase-devel
BuildRequires:  qt5-qtbase-private-devel
BuildRequires:  qt6-qtbase-devel
BuildRequires:  qt6-qtbase-private-devel
BuildRequires:  libxkbcommon-devel
BuildRequires:  libX11-devel
BuildRequires:  libxcb-devel
BuildRequires:  wayland-devel
BuildRequires:  wayland-protocols-devel

# ─── Meta-package Requires (main `unim` rpm is the meta) ─────────────────────
Requires:       %{name}-common = %{version}-%{release}
Requires:       %{name}-im-gtk = %{version}-%{release}
Requires:       %{name}-im-qt = %{version}-%{release}
Requires:       %{name}-xim = %{version}-%{release}
Requires:       %{name}-wayland = %{version}-%{release}
Requires:       %{name}-indicator = %{version}-%{release}
Requires:       %{name}-settings = %{version}-%{release}
Requires:       %{name}-popup-service = %{version}-%{release}
Requires:       %{name}-gnome = %{version}-%{release}

# ─── Description ─────────────────────────────────────────────────────────────
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
Recommends:     im-config

%description common
Core components required by every UNIM installation:
  * libunim_capi shared library (stable C-API consumed by IM modules)
  * unim-daemon — central engine, D-Bus session-activated
  * unim-cli — unified command-line interface (incl. config subcommand)
  * D-Bus service file, icons, im-config integration, man pages

%description common -l ko
모든 UNIM 설치에 필요한 핵심 구성요소:
  * libunim_capi 공유 라이브러리
  * unim-daemon — 중앙 엔진 (D-Bus 세션 활성화)
  * unim-cli — 통합 커맨드라인 인터페이스
  * D-Bus 서비스 파일, 아이콘, im-config 통합, man 페이지

# ─────────────────────────────────────────────────────────────────────────────

%package im-gtk
Summary:        UNIM Korean IME — GTK3/GTK4 input method modules
Summary(ko):    UNIM 한글 입력기 — GTK3/GTK4 입력 메서드 모듈
Requires:       %{name}-common = %{version}-%{release}
Recommends:     %{name}-xim

%description im-gtk
GTK3 and GTK4 input method modules. Enables Korean typing in every GTK-based
application (GNOME, Xfce, MATE, Cinnamon, GIMP, Inkscape, etc.).

%description im-gtk -l ko
GTK3/GTK4 입력 메서드 모듈. GTK 기반 애플리케이션에서 한글 입력을 활성화합니다.

# ─────────────────────────────────────────────────────────────────────────────

%package im-qt
Summary:        UNIM Korean IME — Qt5/Qt6 platform input context plugins
Summary(ko):    UNIM 한글 입력기 — Qt5/Qt6 플랫폼 입력 컨텍스트 플러그인
Requires:       %{name}-common = %{version}-%{release}
Recommends:     %{name}-xim

%description im-qt
Qt5 and Qt6 platform input context plugins. Enables Korean typing in every
Qt-based application (KDE Plasma, Telegram Desktop, Qt Creator, etc.).

%description im-qt -l ko
Qt5/Qt6 플랫폼 입력 컨텍스트 플러그인. Qt 기반 애플리케이션에서 한글 입력을 활성화합니다.

# ─────────────────────────────────────────────────────────────────────────────

%package xim
Summary:        UNIM Korean IME — XIM protocol frontend (X11)
Summary(ko):    UNIM 한글 입력기 — XIM 프로토콜 프론트엔드 (X11)
Requires:       %{name}-common = %{version}-%{release}

%description xim
X11 XIM protocol server. Required in X11 sessions and for legacy applications
that only speak XIM (xterm, emacs -nw, Java/AWT apps, etc.).

%description xim -l ko
X11 XIM 프로토콜 서버. X11 세션 및 XIM만 지원하는 레거시 애플리케이션용.

# ─────────────────────────────────────────────────────────────────────────────

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

# ─────────────────────────────────────────────────────────────────────────────

%package indicator
Summary:        UNIM Korean IME — GTK4 system-tray indicator
Summary(ko):    UNIM 한글 입력기 — GTK4 시스템 트레이 인디케이터
Requires:       %{name}-common = %{version}-%{release}
Recommends:     %{name}-settings
Recommends:     %{name}-popup-service
Recommends:     %{name}-im-gtk

%description indicator
System-tray indicator (StatusNotifierItem) built with GTK4. Provides the global
mode-toggle indicator and a tray menu that launches the settings dialog.

%description indicator -l ko
GTK4로 빌드된 시스템 트레이 인디케이터. 전역 모드 전환 및 설정 대화상자 실행 메뉴 제공.

# ─────────────────────────────────────────────────────────────────────────────

%package settings
Summary:        UNIM Korean IME — GTK4 settings dialog
Summary(ko):    UNIM 한글 입력기 — GTK4 설정 대화상자
Requires:       %{name}-common = %{version}-%{release}

%description settings
Settings dialog built with GTK4 and libadwaita. Provides hotkey, layout,
TypeFix, blacklist, and popup-behaviour configuration.

%description settings -l ko
GTK4 + libadwaita로 빌드된 설정 대화상자. 단축키, 배열, TypeFix, 블랙리스트, 팝업 설정 제공.

# ─────────────────────────────────────────────────────────────────────────────

%package popup-service
Summary:        UNIM Korean IME — popup service (GTK4, X11/Wayland)
Summary(ko):    UNIM 한글 입력기 — 팝업 서비스 (GTK4, X11/Wayland)
Requires:       %{name}-common = %{version}-%{release}
Recommends:     %{name}-indicator
Recommends:     %{name}-settings
Obsoletes:      unim-gui-qt < 0.3.0~

%description popup-service
Standalone GTK4 popup service for hanja/special-character/emoji popups across
non-GNOME desktops (KDE Plasma, Xfce, Cinnamon). Detects X11 vs Wayland at
runtime. Replaces the deprecated unim-gui-qt Qt6/QML GUI.

%description popup-service -l ko
비-GNOME 데스크톱용 GTK4 팝업 서비스. X11/Wayland를 런타임에 자동 감지.
기존 unim-gui-qt Qt6/QML GUI를 대체합니다.

# ─────────────────────────────────────────────────────────────────────────────

%package gnome
Summary:        UNIM Korean IME — GNOME Shell extension
Summary(ko):    UNIM 한글 입력기 — GNOME Shell 확장
BuildArch:      noarch
Requires:       %{name}-common = %{version}-%{release}
Requires:       %{name}-settings = %{version}-%{release}
Requires:       %{name}-popup-service = %{version}-%{release}
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
%make_build PREFIX=%{_prefix} \
            LIBDIR=%{_libdir} \
            LIBEXECDIR=%{_libexecdir} \
            BINDIR=%{_bindir} \
            DATADIR=%{_datadir} \
            SYSCONFDIR=%{_sysconfdir} \
            INCLUDEDIR=%{_includedir}

%install
%make_install PREFIX=%{_prefix} \
              LIBDIR=%{_libdir} \
              LIBEXECDIR=%{_libexecdir} \
              BINDIR=%{_bindir} \
              DATADIR=%{_datadir} \
              SYSCONFDIR=%{_sysconfdir} \
              INCLUDEDIR=%{_includedir}

# ─── %files ──────────────────────────────────────────────────────────────────

%files common
%license LICENSE
%doc README.md
%{_libdir}/libunim_capi.so*
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

%files indicator
%{_bindir}/unim-indicator
%{_sysconfdir}/xdg/autostart/unim-indicator.desktop

%files settings
%{_bindir}/unim-settings
%{_datadir}/applications/unim-settings.desktop

%files popup-service
%{_bindir}/unim-popup-service
%{_datadir}/dbus-1/services/org.atit.unim.PopupService.service

%files gnome
%{_datadir}/gnome-shell/extensions/unim-gnome@from104.github.io/

# ─── Meta-package files (main `unim` rpm is the meta — no own files) ────────
%files
# meta-package — Requires declared at top of spec, no files of its own

# ─── Changelog ───────────────────────────────────────────────────────────────

%changelog
* Tue May 19 2026 from104 <from104@gmail.com> - 0.3.0-1
- Initial RPM packaging for UNIM 0.3.0
- Moachigi v4 Atomic Window Principle chord engine
- Popup-service replaces unim-gui-qt (GTK4 unified implementation)
- XIM commit_then_preedit best-effort fix
- Ahnmatae (안마태) and Qwerty Sebeolsik v2 layouts added then cleaned up
