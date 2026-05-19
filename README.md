# UNIM: 차세대 범용 입력기 (Universal Next-generation Input Method)

![release](https://img.shields.io/badge/release-0.3.0-blue)
![status](https://img.shields.io/badge/status-popup--unify%20%2B%20moachigi-green)
![rust](https://img.shields.io/badge/rust-1.95%2B-orange)
![license](https://img.shields.io/badge/license-see%20LICENSE-lightgrey)

**UNIM**은 Rust로 작성된 오픈 소스 한국어 입력기 엔진(IME)입니다. 모든 주요 플랫폼에서 한국어와 영어 사용자에게 원활하고 고성능이며 확장이 가능한 타이핑 경험을 제공하는 것을 목표로 합니다.

> 📘 **사용자 문서 진입점**: 처음 쓰는 분이라면 [사용자 매뉴얼](docs/user/user-guide/README-ko.md)부터 보시기 바랍니다.
>
> - [사용자 매뉴얼 (한국어)](docs/user/user-guide/README-ko.md) · [User Manual (English)](docs/user/user-guide/README.md)
> - [트러블슈팅 (한국어)](docs/user/troubleshooting/README-ko.md) · [Troubleshooting](docs/user/troubleshooting/README.md)
> - [FAQ (한국어)](docs/user/faq/README-ko.md) · [FAQ (English)](docs/user/faq/README.md)
> - [0.3.0 릴리즈 노트 (한국어)](docs/user/release-notes/0.3.0/README.md) · [Release Notes (English)](docs/user/release-notes/0.3.0/README.en.md)
>
> 🚀 **5분 빠른 시작**: 아래 [§5. 환경 변수 설정](#5-환경-변수-설정-범용-데스크톱-환경) 또는 [사용자 매뉴얼 §2](docs/user/user-guide/README-ko.md#2-빠른-시작-5분).

## v0.3.0 주요 신기능

| 기능 | 요약 |
|------|------|
| **Popup 단일 SoT 아키텍처** | daemon → `unim-popup-service` 이관. 8개 시그널 forward, `org.atit.unim.Popup` 인터페이스. D-Bus auto-activation |
| **GNOME extension popup_view 통합** | Wayland에서 St 위젯(`popup_view.js`)으로 자체 렌더. 팝업 외부 좌클릭 dismiss(클릭 이벤트는 아래 창에 전달) |
| **안마태 + Moachigi v4 Atomic Window** | chord_window_ms 슬라이더 10–200ms (기본 60ms). 윈도우 만료 시 단일 결정, 중간 commit 아티팩트 없음 |
| **AutoTypeFix 학습 blacklist** | retrigger 시점에 tentative 억제 항목 등록 + 즉시 억제. GUI "억제 단어" 페이지 |
| **Hanja 마우스 페이지네이션 + 9×9** | ◀/▶ 버튼 전 프런트엔드 통일. Period 키로 compact ↔ expanded 81칸 전환 |

## 🚀 최종 비전

UNIM의 최종 목표는 다음과 같은 기능을 갖춘 한국어/영어 텍스트 처리 및 입력을 위한 **완벽한 크로스 플랫폼 솔루션**이 되는 것입니다.

1. **자동 상태 전환**: 문맥에 따라 한국어와 영어 모드를 지능적으로 감지하고 전환합니다.
2. **범용 변환**: 잘못 입력된 텍스트(영타를 한글로, 또는 그 반대)를 단축키를 통해 손쉽게 변환합니다.

## 🛠️ 현재 상태

현재 프로젝트는 **3계층 아키텍처(Core → DBus → Frontend)** 기반으로 다음 컴포넌트가 구현 완료되었습니다.

### 핵심 엔진

| 컴포넌트 | 경로 | 설명 |
|----------|------|------|
| **Core Engine** | `src/` | Rust 한글 조합/분해 로직 (2벌식, 3벌식 390/391/순아래) |
| **AutoTypeFix** | `src/auto_typefix.rs` | 한↔영 자동 오타 교정 (forward/reverse) |
| **억제 사전** | `src/typefix_blacklist.rs` | 롤백·재시도 관측으로 오타 교정 제외 단어 자동 학습 (`~/.config/unim/typefix-blacklist.yaml`). GTK 설정창의 "억제 단어" 페이지에서 Tentative/Confirmed/Inactive 관리 |
| **C-API** | `unim-capi/` | Core를 C/C++에서 사용하기 위한 FFI 래퍼 |
| **CLI** | `unim-cli/` | 한↔영 변환 + `config` 서브커맨드로 설정 관리까지 통합한 독립형 명령줄 도구 |

### 3계층 서비스

| 컴포넌트 | 경로 | 설명 |
|----------|------|------|
| **DBus Daemon** | `unim-daemon/` | 중앙 엔진 서버 (세션 버스 서비스) |
| **DBus Library** | `unim-dbus/` | DBus 서비스/클라이언트 구현 |

### 입력 프론트엔드 (IM 모듈)

| 컴포넌트 | 경로 | 설명 |
|----------|------|------|
| **GTK3 IM Module** | `unim-frontends/gtk3/` | C 기반 GTK3 입력 모듈 |
| **GTK4 IM Module** | `unim-frontends/gtk4/` | C 기반 GTK4 입력 모듈 |
| **Qt5 Plugin** | `unim-frontends/qt5/` | C++ 기반 Qt5 입력 플러그인 |
| **Qt6 Plugin** | `unim-frontends/qt6/` | C++ 기반 Qt6 입력 플러그인 |
| **XIM Frontend** | `unim-frontends/xim/` | Rust `xim` crate 기반 X11 XIM 서버 |
| **Wayland Frontend** | `unim-frontends/wayland/` | `input-method-v2` 프로토콜 기반 (KDE/Sway) |

### UI 및 GNOME 확장

| 컴포넌트 | 경로 | 설명 |
|----------|------|------|
| **GUI Common** | `unim-gui-common/` | DBus 통신·트레이·popup 모델·설정 헬퍼 등 toolkit 무관 공통 로직 |
| **GUI GTK** | `unim-gui-gtk/` | GTK4/libadwaita 트레이·설정 UI (GNOME·Xfce·Cinnamon 등 GTK 데스크톱) |
| **Popup Service** | `unim-popup-service/` | 한자·특수문자·이모지 팝업 단일 렌더러 (GTK4, D-Bus auto-activation) |
| **GNOME Extension** | `unim-gnome-extension/` | GNOME Shell 확장 (인디케이터, 오타 변환, popup_view, 설정) |

데스크톱별 GUI 매트릭스:

| 환경 | autostart 패키지 | 한자/특수/이모지 popup | 설정 다이얼로그 |
|------|------------------|------------------------|-----------------|
| GNOME Wayland | unim-gnome (extension) | GNOME Shell extension popup_view.js (St 위젯) | unim-gui-gtk (Adwaita) |
| GNOME X11 | unim-gnome (extension) | unim-popup-service (GTK4) | unim-gui-gtk (Adwaita) |
| KDE Plasma / Xfce / MATE | unim-gui-gtk | unim-popup-service (GTK4) | unim-gui-gtk (GTK4) |
| Sway/Hyprland 등 WM | unim-gui-gtk | unim-popup-service (GTK4, wayland-backend) | unim-gui-gtk (GTK4) |

> ⚠️ **환경 제약 — KDE Plasma 5.x Wayland 미지원**
>
> 한자/특수문자/이모지 popup 은 Wayland 환경에서 `gtk4-layer-shell` 라이브러리로 위치를 지정합니다. Ubuntu 24.04 (noble) 표준 저장소에는 해당 패키지가 없어, **KDE Plasma 5.x Wayland 세션에서는 popup 이 표시되지 않습니다.** 해당 환경에서는 X11 세션을 사용하거나 GNOME 으로 우회해 주세요.
>
> Plasma 6, Sway, Hyprland 등 다른 Wayland 환경은 시스템에 `libgtk4-layer-shell` 가 설치된 상태에서 `wayland-backend` cargo feature 를 켜고 빌드하면 동작합니다.

## 📖 컴포넌트별 명세(SPEC) 인덱스

컴포넌트마다 세부 명세는 코드 옆 `SPEC.md`에 두었다. 아래가 전체 조감도:

| 계층 | 컴포넌트 | 명세 |
|------|---------|------|
| Core | Rust 엔진 | [`src/SPEC.md`](src/SPEC.md) |
| Core | C-API FFI | [`unim-capi/SPEC.md`](unim-capi/SPEC.md) |
| Core | CLI 변환기 + 설정 관리 | [`unim-cli/SPEC.md`](unim-cli/SPEC.md) |
| DBus | 데몬 | [`unim-daemon/SPEC.md`](unim-daemon/SPEC.md) |
| DBus | IPC 라이브러리 | [`unim-dbus/SPEC.md`](unim-dbus/SPEC.md) |
| Frontend | GTK3 IM | [`unim-frontends/gtk3/SPEC.md`](unim-frontends/gtk3/SPEC.md) |
| Frontend | GTK4 IM | [`unim-frontends/gtk4/SPEC.md`](unim-frontends/gtk4/SPEC.md) |
| Frontend | Qt5 IM | [`unim-frontends/qt5/SPEC.md`](unim-frontends/qt5/SPEC.md) |
| Frontend | Qt6 IM | [`unim-frontends/qt6/SPEC.md`](unim-frontends/qt6/SPEC.md) |
| Frontend | XIM | [`unim-frontends/xim/SPEC.md`](unim-frontends/xim/SPEC.md) |
| Frontend | Wayland | [`unim-frontends/wayland/SPEC.md`](unim-frontends/wayland/SPEC.md) |
| Frontend | GNOME Shell | [`unim-gnome-extension/SPEC.md`](unim-gnome-extension/SPEC.md) |
| 공용 | 한자/특수문자 팝업 | [`docs/dev/specs/POPUP_SPEC.md`](docs/dev/specs/POPUP_SPEC.md) |
| 공용 | IME 동작(프론트엔드 공통) | [`docs/dev/architecture/IME_BEHAVIOR.md`](docs/dev/architecture/IME_BEHAVIOR.md) |

관련 리소스:
- 개발 규약 / 로깅 / 설정 동기화: [`docs/dev/architecture/GEMINI.md`](docs/dev/architecture/GEMINI.md)
- 에이전트·기여자 진입점: [`AGENTS.md`](docs/dev/architecture/AGENTS.md), [`CONTRIBUTING.md`](CONTRIBUTING.md)
- 아키텍처 리서치: [`docs/references/research/`](docs/references/research/)
- 실행 가능 예제: [`examples/README.md`](examples/README.md)
- 사용자 가이드: [`docs/user/keyboard-shortcuts.md`](docs/user/keyboard-shortcuts.md) (한국어) · [`docs/user/en/keyboard-shortcuts.md`](docs/user/en/keyboard-shortcuts.md) (English) — 환경별 단축키 등록 (이모지 팝업 등)

## 🏗️ 시스템 아키텍처 및 동작 원리

UNIM은 고성능과 확장성을 위해 **3계층 구조(3-Layered Architecture)**를 채택하고 있습니다. 특히 DBus를 통해 모든 입력 프론트엔드와 코어 엔진이 유기적으로 통신합니다.

### 1. 전체 구조도

- **Core Engine (Rust)**: 한글 조합/분해 로직이 담긴 순수 Rust 라이브러리 (`src/`).
- **DBus Layer (unim-daemon)**: 시스템 전반의 입력 상태를 관리하고 프론트엔드의 요청을 처리하는 중앙 서버.
- **Frontend / IM Modules**: 각 애플리케이션(GTK, Qt, XIM, Wayland)에서 동작하는 클라이언트 모듈.

### 2. DBus 통신 매커니즘

DBus는 UNIM 시스템의 **중추신경계** 역할을 하며 다음과 같이 동작합니다:

1. **중앙 집중식 관리 (`unim-daemon`)**:
    - `unim-daemon`이 실행되면 시스템 세션 버스에 `org.atit.unim.InputMethod` 서비스를 등록합니다.
    - 엔진 코어는 스레드 안전성(`Send+Sync`) 문제로 인해 별도의 **Worker Thread**에서 고립되어 동작하며, DBus 요청은 비동기 채널을 통해 이 스레드로 전달됩니다.
2. **가상 입력 컨텍스트 (Input Context)**:
    - 각 애플리케이션(창)이 포커스를 받으면 DBus를 통해 자신만의 `입력 컨텍스트`를 할당받습니다.
    - 이를 통해 여러 창에서 서로 간섭 없이 독립적인 한글 조합 상태(preedit)를 유지할 수 있습니다.
3. **이벤트 흐름 (Event Flow)**:
    - **입력**: `사용자 키 입력` → `IM 모듈 (클라이언트)` → `DBus` → `unim-daemon (서버)` → `코어 엔진`.
    - **응답**: `결과 생성 (Commit/Preedit)` → `DBus 시그널` → `IM 모듈` → `애플리케이션 화면 출력`.
4. **전역 상태 동기화 (Global Sync)**:
    - 한 창에서 한/영 모드를 바꾸면 `unim-daemon`이 `GlobalModeChanged` 시그널을 방송합니다.
    - `unim-gui`(트레이 아이콘, 팝업)와 다른 모든 입력 모듈들이 이 시그널을 수신하여 즉시 UI와 내부 상태를 동기화합니다.

### 3. C-API 및 라이브러리 연동

- **`unim-capi`**: Rust 코어를 C 언어에서 사용할 수 있도록 래핑한 계층입니다.
- 설정 도구(`unim-cli config`)나 일부 성능이 중요한 툴킷 모듈은 DBus 대신 이 C-API를 통해 엔진 데이터에 직접 접근하거나 설정을 관리합니다.

### 4. 데몬 관리 및 Systemd 통합

`unim-daemon`은 PID 파일 기반 싱글톤 관리와 systemd 사용자 서비스 통합을 지원합니다.

#### 명령줄 옵션

```bash
unim-daemon [OPTIONS]
  -n, --no-daemon  포그라운드 실행 (데몬화 없이)
  -r, --replace    기존 데몬 강제 종료 후 교체
      --check      실행 여부 확인 (exit 0=실행중, 1=미실행)
```

#### Systemd 사용자 서비스

```bash
# 서비스 파일 설치
sudo make install-systemd PREFIX=/usr

# 서비스 활성화 및 시작
systemctl --user daemon-reload
systemctl --user enable --now unim-daemon.service

# 상태 확인
systemctl --user status unim-daemon
```

---
### 5. 환경 변수 설정 (범용 데스크톱 환경)

GNOME 확장을 사용하지 않는 일반적인 데스크톱 환경(KDE Plasma, XFCE, Sway 등)이나 개별 윈도우 매니저(WM)를 사용하는 경우, 시스템의 기본 입력기를 UNIM으로 설정하기 위해 다음 환경 변수를 추가해야 합니다.

가장 권장되는 방법은 `im-config` 도구를 사용하는 것입니다. (Debian/Ubuntu 계열 기준)

```bash
im-config -n unim
```

또는 `~/.xprofile`, `~/.bash_profile`, `~/.pam_environment` 또는 `/etc/environment` 파일에 직접 다음 내용을 추가하고 세션을 다시 시작하세요.

```bash
export GTK_IM_MODULE=unim
export QT_IM_MODULE=unim
export XMODIFIERS="@im=unim"
```

Wayland 네이티브 환경(예: Sway, Hyprland)의 경우, 환경에 맞는 방식으로 위 변수들을 세션 시작 시 내보내도록 설정하세요.

### 6. GNOME 환경 사용 주의사항

GNOME 환경에서 UNIM 확장을 사용할 때는 **기존 IBus 입력기를 시스템에서 완전히 비활성화하거나 삭제**해야 합니다. GNOME은 기본적으로 IBus와 강력하게 결합되어 있어, 두 입력기가 충돌할 경우 키 이벤트가 유실되거나 정상적으로 동작하지 않을 수 있습니다.

```bash
# Debian/Ubuntu 기반 시스템의 경우
sudo apt remove ibus
```

### 7. Flatpak/Snap 앱 한글 입력 (GNOME+Wayland)

GNOME+Wayland 환경에서 Flatpak/Snap 앱(예: Telegram, VS Code 등)은 **샌드박스 내부에 UNIM IM 모듈이 없으므로** 호스트의 `QT_IM_MODULE=unim`/`GTK_IM_MODULE=unim` 설정이 오히려 입력을 방해합니다.

**자동 처리**: `unim-daemon`이 GNOME+Wayland 환경을 감지하면, 시작 시 자동으로 Flatpak 전역 override를 설정하여 IM 환경변수를 비웁니다. 이를 통해 Flatpak 앱들이 Wayland text-input-v3 → GNOME extension 경로로 정상 동작합니다.

로그에서 다음 메시지를 확인할 수 있습니다:
```
[Flatpak] GNOME+Wayland 감지 — Flatpak IM 환경변수 설정 시작
[Flatpak] IM 환경변수 override 완료 (QT_IM_MODULE=, GTK_IM_MODULE= → text-input-v3 경로 사용)
```

**수동 설정** (자동 설정이 동작하지 않는 경우):
```bash
# Flatpak 전역 override
flatpak override --user --env=QT_IM_MODULE= --env=GTK_IM_MODULE=

# 특정 앱만 override
flatpak override --user --env=QT_IM_MODULE= org.telegram.desktop
```

**Snap 앱**: Snap은 호스트 환경변수를 직접 상속하며 Flatpak과 같은 전역 override 메커니즘이 없습니다. Snap 앱에서 한글 입력이 안 되는 경우, 호스트 환경변수를 조건부로 설정하세요:
```bash
# ~/.profile 또는 /etc/profile.d/unim.sh
if [ "$XDG_SESSION_TYPE" = "wayland" ] && echo "$XDG_CURRENT_DESKTOP" | grep -q "GNOME"; then
    export GTK_IM_MODULE=       # 비움 → text-input-v3 기본 경로
    export QT_IM_MODULE=        # 비움 → text-input-v3 기본 경로
else
    export GTK_IM_MODULE=unim
    export QT_IM_MODULE=unim
fi
export XMODIFIERS="@im=unim"
```

**im-config와의 충돌 주의**: `im-config -n unim`으로 입력기를 설정한 경우, im-config가 세션 시작 시 `GTK_IM_MODULE=unim`, `QT_IM_MODULE=unim`을 자동 설정합니다. 이 값은 Snap 앱에도 그대로 전파되어 위와 같은 문제를 일으킬 수 있습니다. GNOME+Wayland 환경에서는 다음 중 하나를 선택하세요:

1. **im-config 비활성화 + 수동 환경변수 설정** (권장):
   ```bash
   im-config -n none                    # im-config 비활성화
   # 위의 조건부 스크립트를 ~/.profile에 추가
   ```

2. **im-config 유지 + Snap 개별 대응**:
   ```bash
   # Snap 앱은 호스트 환경변수를 상속하므로,
   # 개별 snap 앱 실행 시 환경변수를 비워서 실행
   QT_IM_MODULE= GTK_IM_MODULE= snap run telegram-desktop
   ```

---

## 🗺️ 장기 로드맵

1. **1~2단계 (완료)**: Rust 코어 안정화, GNOME Shell 확장, 3계층 아키텍처 + 전체 프론트엔드 (GTK3/4, Qt5/6, XIM, Wayland).
2. **3단계 (진행 중)**: 문서화 및 안정화, Debian 패키지 개선.
3. **4단계 (예정)**: 문맥 인식 기반의 **자동 한/영 전환 알고리즘** 구현.

## 📚 예제

`examples/` 디렉토리에는 UNIM 라이브러리를 시작하는 데 도움이 되는 몇 가지 예제가 포함되어 있습니다.

- **[입력 시뮬레이션 (2벌식)](examples/input_simulation_2bul.rs)**: 2벌식 표준이 실시간 조합 및 "도깨비불" 현상을 어떻게 처리하는지 확인하세요.
- **[입력 시뮬레이션 (3벌식)](examples/input_simulation_3bul.rs)**: 3벌식 레이아웃 처리 이면의 로직을 탐구합니다.
- **[자모 패턴 검색](examples/jamo_pattern_search.rs)**: 텍스트를 자모 단위로 분해하여 퍼지 검색을 수행하는 고급 예제입니다.
- **[문자열 처리](examples/string_processing.rs)**: 한글 음절을 초성, 중성, 종성으로 분해하는 기본 기능을 보여줍니다.
- **[음절 매트릭스](examples/mk_korean.rs)**: 한글 음절 전체 범위를 프로그래밍 방식으로 생성합니다.

예제 실행 방법:

```bash
cargo run --example string_processing
```

---

GNOME 확장의 자세한 설치 및 사용 방법은 [unim-gnome-extension/SPEC.md](unim-gnome-extension/SPEC.md)를 참조하세요.

장기 개발 계획은 [ROADMAP.md](ROADMAP.md)를 참조하세요.

---

## 📜 라이선스 / License

본 프로젝트는 **MIT License** 로 배포됩니다. 전문은 [LICENSE](LICENSE) 파일을 참조하세요.

## 🙏 Credits / 출처

UNIM은 다음과 같은 외부 데이터·표준을 함께 배포하며, 각 출처의 라이선스를 준수합니다. 자세한 내용은 [NOTICE](NOTICE)와 [`LICENSES/`](LICENSES/) 디렉터리를 참조하세요.

- **한자 사전** (`src/data/hanja.txt`) — [libhangul](https://github.com/libhangul/libhangul) 프로젝트 (Copyright © 2005, 2006 Choe Hwanjin), **BSD 3-Clause License**.
- **이모지 데이터** (`src/emoji/data.rs`) — [Unicode CLDR](https://cldr.unicode.org/) `emoji-test.txt` (Unicode 15.0)에서 자동 생성, **Unicode License v3**.
- **자판 표준** (`docs/references/keymaps/*.json`) — KS X 5002, 세벌식 390/391, QWERTY/Dvorak/Colemak/Workman 등 공개 표준. 각 JSON의 `metadata.author` 필드 참조.
- **Rust crate 의존성** — Cargo.lock에 기록된 모든 transitive 의존성은 MIT / Apache-2.0 / BSD / Unicode 라이선스로, MIT와 호환됩니다.
- **시스템 라이브러리** — GTK3/4, Qt5/6, libwayland, libX11, libxkbcommon, glib 등은 동적 링크되며 각 upstream의 LGPL/MIT/X11 라이선스를 따릅니다.
