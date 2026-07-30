# UNIM 사용자 매뉴얼 (한국어)

> UNIM 0.4.0 — Universal Next-generation Input Method
> 한국어와 영어를 자유롭게 오가며 타이핑하기 위한 Rust 기반 입력기.
> 이 문서는 처음 쓰는 사람도 5분 안에 한글 한 글자를 칠 수 있게 만드는 것이 목표다.

---

## 1. UNIM이 무엇인가 (30초 설명)

<!-- @platform:linux -->
- **IME** (Input Method Editor, 입력기): 키보드의 알파벳 키 두드림을 한글 음절로 바꿔서 앱 화면에 보여주는 시스템 컴포넌트. 다른 IME 예시: ibus-hangul, fcitx-hangul, kime, nimf.
<!-- @endplatform -->
<!-- @platform:windows -->
- **IME** (Input Method Editor, 입력기): 키보드의 알파벳 키 두드림을 한글 음절로 바꿔서 앱 화면에 보여주는 시스템 컴포넌트. Windows 에 기본 내장된 「Microsoft 입력기」가 대표적인 예이고, UNIM 은 그것을 대신할 수 있는 또 하나의 선택지다.
<!-- @endplatform -->
<!-- @platform:linux -->
- **🐧 리눅스에서의 차별점**: Rust로 작성된 단일 코어를 GTK·Qt·XIM·Wayland·GNOME Shell 다섯 환경 모두에 그대로 꽂는다. 즉 어떤 앱(터미널, 브라우저, 메모장, IDE)에서 한글을 쳐도 같은 조합 규칙·같은 한자 팝업·같은 자동 오타 교정이 동작한다.
<!-- @endplatform -->
<!-- @platform:windows -->
- **🪟 Windows에서의 차별점**: 리눅스판과 **똑같은 Rust 코어**가 Windows에서는 **TSF** 방식으로 꽂힌다. 즉 메모장·크롬·워드·터미널 어디서 쳐도 같은 조합 규칙·같은 한자 팝업·같은 자동 오타 교정이 동작하고, 같은 `config.yaml` 형식을 쓴다. 64비트 앱용과 32비트 앱용 두 벌이 함께 설치되어 카카오톡·한컴 같은 32비트 전용 앱에서도 동작한다.
<!-- @endplatform -->
- **30초 요약**: "한/영 키로 모드 전환 → 한글 입력 → 한자키로 한자 변환 → ㄱㄴㄷ로 특수문자 변환 → 영타가 한글로 잘못 나가도 자동 복구". 이게 UNIM이 사용자에게 제공하는 가치 전부다.

<!-- @platform:linux -->
> 약어 풀이: **IM 모듈** (Input Method Module) — 앱이 키 입력을 IME에 위임할 수 있게 해 주는 툴킷별 어댑터(GTK용, Qt용 등). **DBus** (Desktop Bus) — 리눅스 데스크톱의 프로세스 간 통신 버스. UNIM은 DBus 위에서 데몬↔프론트엔드 통신을 한다. **XIM** (X Input Method) — X11 시절부터 쓰인 가장 오래된 IME 프로토콜. **Wayland** — 차세대 디스플레이 프로토콜로, X11과 IM 처리 방식이 다르다.
<!-- @endplatform -->
<!-- @platform:windows -->
> 약어 풀이: **TSF** (Text Services Framework) — Windows XP 이후의 표준 입력기 프레임워크. 입력기를 **TIP**(Text Input Processor)라는 COM 부품으로 만들어 OS에 등록하는 방식이며, UNIM의 TIP은 `unim_tsf.dll`(64비트 앱용)과 `unim_tsf32.dll`(32비트 앱용) 두 벌이다. **IMM32** (Input Method Manager 32) — TSF 이전의 옛 입력기 API. 지금도 콘솔 계열 앱(예: WezTerm)이나 자체 입력 처리를 하는 앱이 이 경로를 쓴다. **CUAS** (Cicero Unaware Application Support) — 그런 옛 앱의 IMM32 호출을 Windows가 TSF로 중계해 주는 호환 계층. UNIM은 이 계약을 지켜서 WezTerm·텔레그램 같은 앱에서도 한글 조합이 깨지지 않는다.
<!-- @endplatform -->

---

## 2. 빠른 시작 (5분)

### 2.1 설치

<!-- @platform:linux -->
> **🐧 리눅스** 지원 환경: **Ubuntu 24.04 (noble) 이상 / 동급 Debian, amd64.** 릴리스 `.deb` 는 noble 기준으로 빌드된다.

#### 방법 1 — 자동 설치 스크립트 (권장)

한 줄이면 GitHub Releases 의 `.deb` 전체를 내려받아 `apt` 로 설치한다. 모든 `.deb` 를 **SHA256 체크섬으로 검증**하고, `mktemp` 임시 디렉토리에 격리하며, 외부 런타임 의존성을 자동 해결한다. 검증 실패 시 아무것도 설치하지 않고 중단한다(부분 설치 없음).

```bash
curl -fsSL https://raw.githubusercontent.com/from104/unim/main/install.sh | bash
```

특정 버전을 고정하려면:

```bash
UNIM_VERSION=v0.4.0 curl -fsSL https://raw.githubusercontent.com/from104/unim/main/install.sh | bash
```

`curl | bash` 를 신뢰하지 않는다면 스크립트를 먼저 받아 읽고 실행할 수 있다:

```bash
curl -fsSL https://raw.githubusercontent.com/from104/unim/main/install.sh -o install.sh
less install.sh && bash install.sh
```

#### 방법 2 — Releases 수동 다운로드

[Releases](https://github.com/from104/unim/releases) 에서 `unim*_<버전>-1_{amd64,all}.deb` 전부와 `SHA256SUMS` 를 같은 디렉토리에 받아 검증 후 설치한다.

```bash
# 체크섬 검증 (예: 0.4.0-1 — 11개 패키지)
sha256sum -c SHA256SUMS

# 패키지 설치 (apt 가 의존성을 자동 해결)
sudo apt install ./unim*.deb

# IBus 제거 (GNOME 환경에서 충돌 방지)
sudo apt remove ibus

# 시스템 입력기로 UNIM 등록 (Debian/Ubuntu 표준 도구)
im-config -n unim
```

설치 직후 한 번 **로그아웃하고 다시 로그인**하면 환경변수가 새 셸에 적용된다. 재로그인한 첫 세션에서 **첫 실행 마법사(unim-settings)가 자동으로 뜨며**, 기본 입력기 지정까지 GUI로 안내한다. 마법사를 도중에 닫으면 다음 로그인에 다시 나타나고, 수동으로 다시 열려면 `unim-settings --first-run` 을 실행한다.

#### 방법 3 — 소스 빌드 (Arch/Fedora/그 외)

```bash
git clone https://github.com/from104/unim.git
cd unim
make build           # Rust workspace + GTK3/4·Qt5/6 IM 모듈 한 번에 빌드
sudo make install PREFIX=/usr
sudo make install-systemd PREFIX=/usr
systemctl --user daemon-reload
systemctl --user enable --now unim-daemon.service
```

소스 빌드는 `cargo` 1.95 이상, GTK4/libadwaita 헤더, Qt5/Qt6 개발 패키지가 필요하다. 패키지명은 배포판마다 다르니 빌드 실패 시 [`docs/user/troubleshooting/README-ko.md`](../troubleshooting/README-ko.md#빌드-실패) 참고.
<!-- @endplatform -->

<!-- @platform:windows -->
> **🪟 Windows** 지원 환경: **Windows 10 / 11 (64비트).** 설치 시 관리자 권한(UAC) 승인이 한 번 필요하다.
>
> ⚠️ **실험적 지위**: Windows 지원은 0.4.0에서 TSF 기반으로 크게 확장됐지만 아직 실기기 검증(QA)이 진행 중인 **실험적 기능**이다. 문제를 겪으면 [GitHub Issues](https://github.com/from104/unim/issues)로 제보해 주기 바란다.

#### 방법 A — 설치 스크립트 (권장)

PowerShell(또는 Windows Terminal)에서 한 줄이면 끝난다.

```powershell
irm https://raw.githubusercontent.com/from104/unim/main/install.ps1 | iex
```

GitHub Releases 의 최신 MSI(`unim-<버전>-x64.msi`)를 내려받아 **`SHA256SUMS-msi` 로 SHA256 검증**(승격된 프로세스 안에서 한 번 더 재검증해 검증↔설치 사이 변조 차단)하고 `msiexec` 로 설치한다. 체크섬이 어긋나면 아무것도 설치하지 않고 중단한다.

```powershell
# 업데이트 (이미 최신이면 내려받지도 않는다)
& ([scriptblock]::Create((irm https://raw.githubusercontent.com/from104/unim/main/install.ps1))) -Update

# 설치 여부·최신 버전만 확인 (아무것도 바꾸지 않는다)
& ([scriptblock]::Create((irm https://raw.githubusercontent.com/from104/unim/main/install.ps1))) -Check

# 특정 버전 고정 (해당 버전 릴리스에 SHA256SUMS-msi 가 첨부돼 있어야 한다)
$env:UNIM_VERSION='v0.4.0'; irm https://raw.githubusercontent.com/from104/unim/main/install.ps1 | iex
```

#### 방법 B — MSI 직접 실행

[Releases](https://github.com/from104/unim/releases) 에서 `unim-<버전>-x64.msi` 를 내려받아 더블클릭한다. 체크섬을 손으로 확인하려면 같은 릴리스의 `SHA256SUMS-msi` 와 대조한다.

```powershell
# 내려받은 MSI 의 해시 확인 (SHA256SUMS-msi 의 값과 같아야 한다)
Get-FileHash .\unim-0.4.0-x64.msi -Algorithm SHA256
```

> **SmartScreen 경고**: MSI 는 아직 코드 서명이 없어 "Windows에서 PC를 보호했습니다" 경고가 뜰 수 있다. **추가 정보 → 실행**으로 진행한다.

#### 설치되는 것 / 설치 후 해야 할 일

설치 위치는 `C:\Program Files\UNIM\` 이고, 다음이 들어간다.

| 파일 | 역할 |
|------|------|
| `unim_tsf.dll` | 64비트 앱용 입력기 본체 (TSF TIP) |
| `unim_tsf32.dll` | 32비트 앱용 입력기 본체 — 카카오톡·한컴 등 |
| `unim-settings.exe` | 설정 창 (§5 참고) |
| `unim-popup-win.exe` | 한자·특수문자·이모지 팝업 렌더러 (로그인 시 자동 시작) |
| `help\unim-help-ko.html`, `help\unim-help-en.html` | 지금 읽고 있는 이 오프라인 매뉴얼 |
| `register-tsf.bat`, `unregister-tsf.bat` | 입력기 등록/해제를 손으로 다시 하기 위한 보조 스크립트 |
| `LICENSE.txt`, `NOTICE.txt`, `LICENSES\` | 라이선스 고지 |

설치가 끝나면 **재부팅(또는 로그아웃 후 재로그인)** 한다. 그래야 실행 중이던 앱들이 새 입력기를 인식한다. 재로그인 후 **첫 실행 마법사(`unim-settings`)가 자동으로 뜨며** 기본 입력기 지정까지 안내한다. 마법사를 도중에 닫으면 다음 로그인에 다시 나타나고, 수동으로 다시 열려면 시작 메뉴의 **UNIM → UNIM Settings** 를 실행한 뒤 안내에 따르거나 `"C:\Program Files\UNIM\unim-settings.exe" --first-run` 을 실행한다.

시작 메뉴에는 **UNIM** 폴더가 생기고 그 안에 두 항목이 놓인다.

- **UNIM Settings** — 설정 창
- **Uninstall UNIM** — 제거

#### 업데이트

위 `-Update` 한 줄이 가장 간단하다. MSI 를 직접 받아 실행해도 되며, 같은 제품이므로 **이전 버전을 먼저 지울 필요는 없다**(업그레이드 설치).

#### 제거

세 가지 중 아무거나 쓴다. 어느 쪽이든 파일·시작 메뉴 항목·입력기 등록이 함께 지워진다.

- **설정 → 앱 → 설치된 앱** 목록에서 UNIM 제거
- 시작 메뉴 → **UNIM → Uninstall UNIM**
- 관리자 PowerShell 에서 `msiexec /x` (제품 코드는 위 두 방법이 알아서 처리하므로 보통 필요 없다)

> `unregister-tsf.bat` 은 **제거 도구가 아니다.** 입력기 COM 등록만 해제하는 보조 스크립트라 파일과 설정은 그대로 남는다. 완전 제거는 위 세 방법을 쓴다.
<!-- @endplatform -->

<!-- @platform:linux -->
### 2.2 환경 변수 (GNOME 확장을 안 쓰는 모든 데스크톱)

KDE Plasma·XFCE·Sway·Hyprland 등에서는 `~/.xprofile` 혹은 `/etc/environment`에 다음 세 줄을 직접 추가한다.

```bash
export GTK_IM_MODULE=unim
export QT_IM_MODULE=unim
export XMODIFIERS="@im=unim"
```

추가 후 로그아웃 → 다시 로그인. `im-config -n unim` 한 줄로도 같은 효과를 본다 (Debian 계열 한정).

### 2.3 GNOME + Wayland 사용자

GNOME Shell 위에서는 `unim-gnome-extension`이 키 가로채기·팝업을 책임진다. 환경변수가 아니라 **GNOME Extension**을 활성화한다.

```bash
gnome-extensions enable unim-gnome@from104.github.io
```

그리고 IBus와 충돌하지 않게 IBus는 비활성화 또는 제거한다.

```bash
sudo apt remove ibus
```
<!-- @endplatform -->

<!-- @platform:windows -->
### 2.2 Windows — 입력기 목록에서 UNIM 고르기

리눅스와 달리 Windows 에는 환경 변수 설정이 필요 없다. MSI 가 입력기를 OS 에 등록해 두므로, 재부팅 뒤에는 **한국어 입력기 목록에 UNIM 이 나타난다.**

1. 작업 표시줄 오른쪽 아래 언어 표시(`한`/`A` 또는 `KOR`)를 클릭하거나 **`Windows` + `Space`** 를 누른다.
2. 목록에서 **UNIM** 을 고른다.
3. 목록에 UNIM 이 없으면 **설정 → 시간 및 언어 → 언어 및 지역** 에서 **한국어**가 설치돼 있는지 확인한다. 한국어가 없으면 먼저 추가한 뒤 재로그인한다.

> **UNIM 을 기본 입력기로**: 매번 고르는 게 번거로우면 설정 창(§5)의 첫 실행 마법사에서 **기본 입력기로 설정**을 누르거나, 언어바 우클릭 메뉴의 **기본 입력기로 설정**을 쓴다.

### 2.3 Windows — 언어바(트레이 인디케이터)

작업 표시줄 시계 옆에 현재 모드가 글자로 표시된다 — 한글 모드면 **`한`**, 영문 모드면 **`A`**.

- **왼쪽 클릭** → 한/영 전환
- **오른쪽 클릭** → 메뉴: **한/영 전환** · **기본 입력기로 설정** · **설정** · **도움말** · **정보**

> ⚠️ 헷갈리기 쉬운 지점: 설정 창은 **오른쪽 클릭 → 설정**이다. 왼쪽 클릭은 한/영 전환이라 설정 창이 열리지 않는다.

> Windows 11 에서는 언어바가 트레이 아이콘 형태로 나오고, 트레이 아이콘이 넘침 영역(`^` 버튼)에 숨어 있을 수 있다. 항상 보이게 하려면 **설정 → 개인 설정 → 작업 표시줄 → 기타 시스템 트레이 아이콘**에서 켜 둔다.
<!-- @endplatform -->

### 2.4 첫 한글 입력 (60초)

<!-- @platform:linux -->
1. 텍스트 에디터(예: GNOME Text Editor, Kate)를 연다.
2. **한/영 키**(또는 `Shift+Space`, 키보드별로 다름)를 누른다 — 트레이 아이콘이 「한」으로 바뀐다.
3. `dkssud` 입력 → 화면에 `안녕`이 뜬다.
4. 한자 키(또는 F9)를 누르면 `安寧` 후보가 팝업으로 뜬다. 숫자 1~9로 선택.
5. 한 번 더 한/영 키 → 영문 모드.
<!-- @endplatform -->
<!-- @platform:windows -->
1. 메모장을 연다.
2. **한/영 키**(또는 **오른쪽 Alt**)를 누른다 — 언어바 표시가 `A` 에서 「한」으로 바뀐다.
3. `dkssud` 입력 → 화면에 `안녕`이 뜬다.
4. 한자 키(또는 F9)를 누르면 `安寧` 후보가 팝업으로 뜬다. 숫자 1~9로 선택.
5. 한 번 더 한/영 키 → 영문 모드.
<!-- @endplatform -->

여기까지 동작하면 설치는 끝났다. 동작이 안 되면 [트러블슈팅](../troubleshooting/README-ko.md)으로.

---

## 2.5 팝업 동작 개요

<!-- @platform:linux -->
**🐧 리눅스** — UNIM 0.3.0부터 한자·특수문자·이모지 팝업은 **`unim-popup-service`** 가 단일 렌더러로 처리한다.

| 환경 | 팝업 렌더 주체 | 비고 |
|------|--------------|------|
| GNOME Wayland | GNOME Extension `popup_view.js` (St 위젯) | Mutter가 wlr-layer-shell 미지원이므로 extension 자체 렌더 |
| GNOME X11 / KDE / Xfce / WM (X11) | `unim-popup-service` GTK4 윈도우 | D-Bus auto-activation으로 자동 기동 |
| Wayland (KDE Plasma 6 / Sway / Hyprland) | `unim-popup-service` GTK4 윈도우 (wayland-backend) | `libgtk4-layer-shell` 필요 |

**공통 SoT**: 환경과 무관하게 daemon의 `PopupRender` payload(셀·헤더·푸터·탭·하이라이트)가 단일 view-model로 모든 렌더러에 전달된다. 렌더 구현만 다를 뿐 동작은 동일하다.

**외부 클릭 dismiss**: 팝업 바깥 좌클릭 시 팝업이 닫히며, 클릭 이벤트는 아래 창에 그대로 전달된다. 팝업이 예상치 않게 닫혔다면 의도된 동작이다 — [트러블슈팅 §popup-dismiss](../troubleshooting/README-ko.md) 참고.

**KDE Plasma 5.x Wayland 미지원**: `gtk4-layer-shell`이 Ubuntu 24.04 표준 저장소에 없어 팝업이 표시되지 않는다. X11 세션 또는 GNOME으로 우회.

**KDE Plasma 6 Wayland / Sway / Hyprland / river — 실험적, 검증 미흡**: `wayland-backend` cargo feature + `libgtk4-layer-shell` 조합으로 빌드 시 이론상 동작하나, 0.3.0 QA 사이클 이후 v0.4.0까지 이 실험적 지위는 변하지 않았다(추가 검증 없음). popup 위치, IME 포커스 전환, layer-shell 좌표 변환에서 미세 회귀가 있을 수 있다. 자세한 환경별 지원 현황은 [트러블슈팅 §G 환경 매트릭스](../troubleshooting/README-ko.md#g-환경-매트릭스-040-재확인--최초-작성은-030) 참고.
<!-- @endplatform -->

<!-- @platform:windows -->
**🪟 Windows** — 한자·특수문자·이모지 팝업은 입력기 본체(`unim_tsf.dll`)가 직접 그리지 않고, **별도 프로세스인 `unim-popup-win.exe`** 가 그린다. 입력기는 "무엇을 보여줄지"(후보 목록·헤더·푸터·탭·하이라이트)만 넘기고 그리기는 이 프로세스가 맡는 구조라, 어떤 앱에서 팝업을 띄우든 모양과 동작이 같다.

- 이 렌더러는 **로그인할 때 자동으로 함께 시작**된다(설치 시 등록되는 자동 실행 항목 `UnimPopupRenderer`). 작업 관리자에 `unim-popup-win.exe` 가 상주하는 것이 정상이다.
- 어떤 이유로 꺼져 있어도 팝업이 처음 필요해지는 순간 입력기가 다시 띄운다. 그래서 이 프로세스를 강제 종료해도 다음 한자 키 입력에서 복구된다.
- 이 프로세스를 끄면 팝업만 사라질 뿐 **한글 조합 자체는 계속 동작한다.**

> **작업 관리자에서 이걸 꺼도 되나?** 시작 프로그램에서 빼도 입력에는 지장이 없지만, 한자 키를 처음 누를 때 팝업이 뜨기까지 잠깐 지연이 생긴다. 그대로 두는 쪽을 권한다.
<!-- @endplatform -->

---

## 3. 환경별 설정 가이드

<!-- @platform:linux -->
**🐧 리눅스** — 데스크톱 환경과 앱 툴킷 조합에 따라 진입 경로가 갈린다.

| 환경 | 설치 방법 | IM 모듈 | 팝업 주체 | 주의점 |
|------|----------|---------|----------|--------|
| **X11 + GTK 앱** | `GTK_IM_MODULE=unim` | gtk3/gtk4 IM 모듈 | `unim-popup-service`(GTK4, D-Bus auto-activation) | — |
| **X11 + Qt 앱** | `QT_IM_MODULE=unim` | qt5/qt6 IM 플러그인 | 동상 | Plasma는 Qt 모드로 통일 권장 |
| **X11 + 레거시 (Emacs, xterm)** | `XMODIFIERS=@im=unim` | xim 프론트엔드 | XIM 자체 Xft 팝업 | over-the-spot 모드 |
| **GNOME + Wayland** | GNOME Extension 활성화 | (앱은 텍스트-인풋-v3 직접) | GNOME Extension | IBus 제거 필수 |
| **KDE + Wayland** | `QT_IM_MODULE=unim` + Wayland 프론트엔드 | wayland | `unim-popup-service`(wayland-backend) | input-method-v2 사용 |
| **Sway/Hyprland (Wayland)** | 환경변수 + Wayland 프론트엔드 | wayland | `unim-popup-service`(wayland-backend) | 컴포지터의 input-method-v2 지원 필요 |

> 환경 판별: `echo $XDG_SESSION_TYPE`(x11/wayland), `echo $XDG_CURRENT_DESKTOP`(GNOME/KDE/sway).

### 3.1 Flatpak/Snap 앱에서 한글이 안 나올 때

GNOME+Wayland에서 Telegram, VS Code 같은 Flatpak/Snap 앱은 샌드박스 안에 UNIM IM 모듈이 없다. 호스트의 `GTK_IM_MODULE=unim`이 오히려 입력을 막는다.

**자동 처리**: `unim-daemon`이 GNOME+Wayland를 감지하면 시작 시 자동으로 Flatpak 전역 override를 설정해 IM 환경변수를 비운다. Flatpak 앱은 Wayland text-input-v3 → GNOME Extension 경로로 동작한다.

**수동 설정**(자동 설정이 동작하지 않을 때):

```bash
flatpak override --user --env=QT_IM_MODULE= --env=GTK_IM_MODULE=
```

Snap 앱에는 전역 override 메커니즘이 없으니 `~/.profile`에 조건부로 환경변수를 비우는 스크립트를 추가한다 — [README §1.7](../../../README.md) 참고.
<!-- @endplatform -->

<!-- @platform:windows -->
**🪟 Windows** — 환경 변수나 데스크톱 환경별 설정이 없다. 대신 **앱이 어떤 입력 방식을 쓰는가**에 따라 UNIM 안에서 담당하는 부품이 갈린다. 세 경우 모두 자동이라 사용자가 고를 일은 없다.

| 앱 종류 | 예 | 담당 부품 |
|---------|-----|----------|
| **64비트 앱** (대부분) | 메모장, 크롬, 워드, Windows Terminal | `unim_tsf.dll` (TSF) |
| **32비트 앱** | 카카오톡, 한컴오피스 | `unim_tsf32.dll` (32비트 TSF) |
| **콘솔·IMM32 계열 앱** | WezTerm, 텔레그램 | TSF + CUAS 호환 계층 |

> **왜 두 벌인가?** Windows 는 64비트 앱과 32비트 앱이 서로 다른 입력기 부품을 로드한다. 그래서 32비트 전용 앱(카카오톡·한컴 등)에서도 한글이 되게 하려면 32비트용 입력기를 따로 등록해야 한다. UNIM 은 설치할 때 두 벌을 모두 등록하므로 사용자가 신경 쓸 것은 없다.

> **IMM32 `.ime` 방식은 안 쓴다**: 아주 오래된 방식의 입력기 등록(`.ime` 파일)은 Windows 11 에서 의미가 없어졌기 때문에 0.4.0 에서 폐기했다. 옛 IMM32 API 를 쓰는 앱은 위 표의 세 번째 줄, 즉 Windows 가 제공하는 호환 계층(CUAS)을 통해 TSF 로 연결된다.

### 3.1 특정 앱에서만 한글이 안 될 때

앱마다 입력 처리 방식이 달라, 일부 앱에서만 증상이 나타날 수 있다. 아래 순서로 좁힌다.

1. **메모장에서는 되는가?** 메모장에서 정상이면 UNIM 자체는 살아 있는 것이고, 문제는 그 앱과의 궁합이다.
2. **그 앱이 32비트인가?** 작업 관리자 → 세부 정보 탭에서 프로세스 이름 끝에 `(32비트)` 표시가 있는지 본다. 32비트인데 안 된다면 32비트 입력기 등록이 풀린 경우일 수 있다 — 관리자 권한 명령 프롬프트에서 `C:\Program Files\UNIM\register-tsf.bat` 을 한 번 실행한 뒤 재로그인한다.
3. **그래도 안 되면** 앱 이름·버전과 함께 [트러블슈팅](../troubleshooting/README-ko.md) 을 확인하고, 해결되지 않으면 [GitHub Issues](https://github.com/from104/unim/issues) 로 제보한다. Windows 지원은 실험적 단계라 앱별 제보가 특히 도움이 된다.
<!-- @endplatform -->

---

## 4. 일상 사용

### 4.1 한/영 모드 전환

<!-- @platform:linux -->
| 키 | 동작 | 비고 |
|----|------|------|
| 한/영 키 (Hangul) | 모드 토글 | 키보드에 따라 키 코드가 다름 |
| `Shift+Space` | 모드 토글 (대체) | 모든 키보드에서 동작 |
| 오른쪽 Alt (RightAlt) | 모드 토글 (`toggle_keys`에 추가한 경우) | 이제 GTK·Qt·GNOME 포함 모든 환경에서 동작 |
| 트레이 아이콘 클릭 | 모드 토글 (마우스) | `unim-indicator`가 트레이에 |
<!-- @endplatform -->
<!-- @platform:windows -->
| 키 | 동작 | 비고 |
|----|------|------|
| 한/영 키 (Hangul) | 모드 토글 | 키보드에 따라 키 코드가 다름 |
| 오른쪽 Alt (RightAlt) | 모드 토글 | 기본값. 한/영 키가 없는 키보드에서 유용 |
| 언어바 왼쪽 클릭 | 모드 토글 (마우스) | 작업 표시줄 시계 옆 `한`/`A` 표시 |
<!-- @endplatform -->

> **모드 공유 방식**(`mode_sharing` 설정, CLI 키 `mode-sharing`): 「전역 공유」/「앱별 독립」 중 선택. **「전역 공유」가 기본값**이다 — 한 모드를 바꾸면 모든 창에 즉시 동기화된다. 터미널은 영어, 텍스트 에디터는 한글처럼 창마다 모드를 따로 유지하고 싶으면 「앱별 독립」으로 바꾼다.

<!-- @platform:linux -->
> **접근성 — 비시각 사용 시 한/영 전환 비프를 켜라**: 리눅스에는 화면 낭독기로 모드 전환을 능동 통지하는 기능이 없고, 전환 비프도 **기본은 꺼짐**이다(오탐지 방지를 위한 의도된 기본값). 화면을 보지 않고 입력한다면 다음 명령으로 켜 두는 것을 권장한다.
> ```bash
> unim-cli config set toggle-announce-beep true
> ```
> 켜짐/꺼짐을 서로 다른 음높이로 구분해 알려 준다.
>
> **단, 모드 공유 방식에 따라 이 비프가 안 들릴 수 있다**: 트레이 아이콘·GNOME 확장에서 모드를 바꿀 때 비프가 울리는 경로는 「전역 공유」에서만 동작한다(`unim-dbus/src/engine_worker.rs:1766-1772`). 「앱별 독립」으로 바꾸면 트레이 토글로는 현재 포커스 창의 모드가 실제로 안 바뀌므로(의도된 동작 — 잘못된 신호 방지) 이 경로의 비프가 무음 처리된다. 청각 신호에 의존해 한/영 전환을 확인한다면, 「앱별 독립」으로 바꾸는 순간 트레이 조작으로 나는 비프 신호를 잃는다는 점을 감안한다.
<!-- @endplatform -->
<!-- @platform:windows -->
> **접근성 — 화면 낭독기와 전환음**: Windows 에서는 한/영 전환이 **화면 낭독기(NVDA·내레이터)에 통지**되고, 조합 중인 글자와 후보 창도 낭독기가 읽을 수 있게 노출된다.
>
> 여기에 더해 **한/영 전환 소리 알림**을 켤 수 있다 — 한글은 높은음, 영문은 낮은음으로 구분된다. **기본값은 꺼짐**이며(오탐지 방지를 위한 의도된 기본값), 설정 창(§5)의 「일반」 → 「접근성」에서 켠다. 화면을 보지 않고 입력한다면 켜 두는 쪽을 권한다.
<!-- @endplatform -->

<!-- @platform:linux -->
> **오른쪽 Alt로 토글**: `toggle_keys` 설정에 `RightAlt`를 넣으면 오른쪽 Alt 키로 한/영을 전환할 수 있다. 예전에는 GTK·Qt·GNOME 확장이 오른쪽 Alt를 자체적으로 걸러내 이 환경들에서는 동작하지 않았지만(XIM·순수 Wayland는 원래 동작), 이제 토글 판정이 데몬으로 일원화되어 어디서나 똑같이 동작한다. AltGr(오른쪽 Alt를 AltGr로 쓰는 레이아웃)에는 영향이 없다. 단, 토글하는 순간 앱도 Alt 입력을 함께 받을 수 있어(예: 일부 앱의 메뉴바 포커스) 이 부작용이 싫으면 `toggle_keys`에서 `RightAlt`를 빼면 된다.
<!-- @endplatform -->
<!-- @platform:windows -->
> **오른쪽 Alt로 토글**: 한/영 키가 없는 키보드(영문 배열 노트북 등)에서는 **오른쪽 Alt** 로 한/영을 전환할 수 있다. AltGr(오른쪽 Alt를 AltGr로 쓰는 레이아웃)에는 영향이 없다. 단, 토글하는 순간 앱도 Alt 입력을 함께 받을 수 있어(예: 일부 앱의 메뉴바 포커스) 이 부작용이 싫으면 설정의 `toggle_keys` 목록에서 `RightAlt` 를 빼면 된다.
<!-- @endplatform -->

> **키 이름 표기**: `toggle_keys` 에는 UNIM 이 아는 키 이름을 **단독으로** 적는다(기본값 `Korean`, `RightAlt`). 오타 교정 단축키(4.4)와 달리 한/영 전환키는 수정자 조합 표기(`Ctrl+X`)를 받지 않는다. 잘못 적으면 CLI 와 설정 앱이 저장 시 경고를 띄우고, 데몬 로그에도 파싱 실패가 기록된다 — 값 자체는 저장되지만 그 키는 조용히 죽지 않고 눈에 보인다.

### 4.2 한자 변환 (Hanja)

1. 한자로 바꿀 한글을 입력 (예: `한국`).
2. **한자 키**(또는 F9)를 누른다.
3. 9칸 그리드 팝업이 뜬다 — `韓國`, `漢國` 등.
4. 숫자 1~9로 직접 선택, 화살표로 이동, Enter로 확정, ESC로 취소.
5. 후보가 9개 이상이면 **마침표(.)** 키로 9×9=81칸 확장 그리드로 토글. 화면 우상단에 ⊞/⊟ 아이콘으로 현재 모드 표시.

> **한자 키 지정**: `hanja_keys` 기본값은 `Hanja`, `F9` 다. 한/영 전환키(4.1)와 마찬가지로 단일 키 이름만 받고 수정자 조합 표기는 쓸 수 없다. 잘못 적으면 CLI 와 설정 앱이 저장 시 경고를 띄우고 데몬 로그에도 파싱 실패가 기록된다.

#### 페이지 이동 (마우스/키보드)

후보가 한 페이지(9칸 또는 81칸)를 넘으면 푸터에 **◀ / ▶** 버튼이 나타난다.

```text
[◀]  page 2 / 5  [▶]  ⊞
```

- **마우스 좌클릭** ◀ : 이전 페이지, ▶ : 다음 페이지. 마지막에서 ▶을 누르면 첫 페이지로 wrap, 첫에서 ◀ 누르면 마지막으로 wrap.
- **키보드** `←` / `Page Up` : 이전 페이지, `→` / `Page Down` : 다음 페이지. 동일하게 wrap-around.
- 후보가 한 페이지 안에 다 들어가면 ◀/▶ 버튼은 숨겨진다 (한 페이지짜리에 페이지 컨트롤이 보이는 혼란 방지).
- 페이지를 옮겨도 cursor의 행/열 위치는 유지된다 — 예: 81칸 grid에서 (3, 4) 셀을 보다가 ▶ 누르면 다음 페이지의 (3, 4) 셀이 선택된 상태로 표시.

> **약어 풀이**: cursor란 현재 키보드 포커스가 가 있는 셀을 가리키는 시각 표시 (배경 하이라이트로 보인다).

##### 우클릭 동작 — 환경별 차이

<!-- @platform:linux -->
페이지 이동 ◀/▶ 버튼은 모든 프런트엔드에서 동일하게 동작하지만, **그리드 영역 위에서의 우클릭(마우스 오른쪽 버튼)**은 프런트엔드마다 의미가 다르다. 시각적 단축으로 활용하려면 자기 환경의 매핑을 알아 둬야 한다.
<!-- @endplatform -->
<!-- @platform:windows -->
마우스로 할 수 있는 조작은 다음이 전부다.
<!-- @endplatform -->

<!-- @platform:linux -->
- **GNOME Shell 확장** (Wayland/X11 모두): **★ 즐겨찾기 토글** — 키보드 Space와 동일.
- **GTK IM 모듈** (gtk3 / gtk4): **다음 페이지** (wrap-around) — `→` / Page Down과 동일.
- **Qt IM 모듈** (qt5 / qt6): **다음 페이지** (wrap-around).
- **XIM** (한자 / 특수문자 / 이모지 팝업 공통): **다음 페이지** (wrap-around).
- **그 외** (`unim-popup-service` Standalone, 순수 Wayland): 동작 없음 (정의되지 않음).

> **왜 GNOME만 다른가?** GNOME Shell 확장은 후보 셀이 곧 `Clutter.Actor`라 셀 단위 우클릭 hit-test가 자연스럽다. 그래서 우클릭에 즐겨찾기를 매핑한 게 손이 덜 가는 단축이 된다. 반면 GTK/Qt IM 모듈과 XIM은 X11/Wayland override-redirect 윈도우라 셀 hit-test가 제한적이고, 우클릭으로 페이지를 넘기는 기존 관습(고전 IME 패턴)을 따른다.
>
> **정리**: 어느 환경이든 ◀/▶ 마우스 버튼·키보드 `←`/`→`·Space는 의미가 같다. 환경에 따라 달라지는 건 오직 **그리드 본문에서의 우클릭**뿐이다. 헷갈리면 키보드 단축키만 써도 모든 동작을 할 수 있다.
<!-- @endplatform -->
<!-- @platform:windows -->
Windows 에서는 팝업을 그리는 프로세스가 하나뿐이라(§2.5) 앱마다 달라지는 일이 없다.

- **좌클릭**: 셀을 누르면 그 후보가 확정된다. 푸터의 ◀ / ▶ 를 누르면 페이지가 넘어간다.
- **우클릭**: **아무 동작도 하지 않는다** — 의도된 동작이며, 실수로 눌러도 팝업이 닫히거나 후보가 확정되지 않는다.

> 리눅스판 UNIM 은 데스크톱 환경에 따라 그리드 위 우클릭이 「즐겨찾기 토글」 또는 「다음 페이지」로 달라진다. Windows 에는 그 분기가 없다. 즐겨찾기는 **Space**, 페이지 이동은 **`←`/`→`** 또는 **◀/▶ 버튼**을 쓴다.
<!-- @endplatform -->

#### 즐겨찾기 ☆/★

자주 쓰는 한자를 즐겨찾기에 등록할 수 있다. 후보에 포커스를 둔 상태에서 **Space** 키 → ☆(미등록) ↔ ★(등록) 토글.

<!-- @platform:linux -->
등록된 한자는 `HanjaCandidatesReordered` DBus 시그널로 모든 열린 팝업이 즉시 갱신된다.
<!-- @endplatform -->
<!-- @platform:windows -->
등록하면 열려 있는 모든 팝업이 즉시 갱신된다.
<!-- @endplatform -->

토글하면 한자가 자동으로 재정렬되며 cursor가 그 한자를 따라간다.

- **★ 등록 시**: 그 한자가 1페이지 상단(★ 그룹)으로 promote되고 cursor가 그 위치(page 1, row 1)로 이동.
- **☆ 해제 시**: 그 한자가 사전순 원위치로 demote되며 cursor가 그 자리(다른 페이지일 수 있음)로 점프. 점프해 도착한 셀이 **140ms 동안 노란색(#f9e2af)으로 짧게 깜박인다(flash)** — "별을 끄니 한자가 여기로 돌아갔구나"를 즉시 인지하게 만드는 시각 단서.

> **왜 flash가 등록(★ ON) 때는 없고 해제(★ OFF) 때만 있나?** 등록은 cursor가 자연스럽게 1페이지 상단으로 따라가므로 시각 단서가 충분. 반면 해제는 다른 페이지로 점프할 때 사용자가 화면 변화를 놓치기 쉬워 flash로 위치를 찍어 준다.

### 4.3 특수문자 입력

한글 모드에서 자음(초성) 키 → 한자 키. 초성에 따라 카테고리가 달라진다.

| 초성 | 카테고리 | 예시 |
|------|---------|------|
| ㄱ | 특수기호 | `!`, `@`, `÷`, `≠`, `∞` |
| ㄴ | 괄호류 | `「」`, `『』`, `≪≫` |
| ㄷ | 수학기호 | `∂`, `∇`, `√`, `∫` |
| ㄹ | 단위기호 | `＄`, `％`, `℃`, `Å` |
| ㅁ | 도형기호 | `■`, `□`, `●`, `○` |
| ㅂ | 선문자 | `─`, `│`, `┌`, `┐` |
| ㅅ | 한글 자모 | `ㄱ`, `ㄴ`, `ㅏ`, `ㅑ` |
| ㅇ | 원문자 | `①`, `②`, `ⓐ` |
| ㅈ | 괄호한글 | `㈀`, `㈁` |
| ㅊ/ㅋ | 괄호숫자 | `⑴`, `⑵` |
| ㅌ | 괄호영문 | `⒜`, `⒝` |
| ㅍ | 그리스문자 | `Α`, `β`, `γ` |
| ㅎ | 기타기호 | `●`, `♨`, `☏` |

예: `ㅁ` 입력 → 한자 키 → 도형기호 그리드 → `2` 선택 → `□` 커밋.

### 4.4 자동 오타 교정 (AutoTypeFix)

타자가 모드를 잘못 둔 채 친 글을 자동으로 되살리는 기능. 두 방향이 있다.

- **Forward (영→한)**: 한글 모드인 줄 알았는데 영문 모드라 `gksrmf`이 그대로 나가버린 경우 → 단어 경계(스페이스/구두점) 시점에 자동으로 `한글`로 교체.
- **Reverse (한→영)**: 반대로 `ㅈㅐㅍㅁ`처럼 한글로 영타를 친 경우 → `wave`로 교체.

#### 억제 사전 (Blacklist) — 사용자 학습

특정 단어가 매번 잘못 교정되면 UNIM이 알아서 학습한다.

1. 교정 결과가 마음에 안 들어 `BackSpace`로 지우고 모드를 전환한다 → UNIM이 「의심 단어」로 표시(Pending).
2. 같은 단어가 또 교정 시도되면 → **그 시도를 억제하고 동시에** 「승인 대기(Tentative)」로 등록.
3. 설정 GUI 「교정 억제 단어」 페이지에서 [확정] 누르면 **Confirmed**(영구 억제), 1시간 동안 재트리거가 없으면 **Inactive**(만료).

<!-- @platform:linux -->
**저장 위치**: `~/.config/unim/typefix-blacklist.yaml`. 데몬은 mtime 감시로 자동 리로드한다.
<!-- @endplatform -->
<!-- @platform:windows -->
**저장 위치**: `%APPDATA%\unim\typefix-blacklist.yaml`. 파일이 바뀌면 잠시 뒤 자동으로 다시 읽어들인다.
<!-- @endplatform -->

> **사용자 사전 (역방향 화이트리스트)**: 0.2.0 신규. 텍스트 선택 후 단축키로 `RegisterUserDictFromSelection` DBus 메서드 호출 → 영어 사전 항목으로 등록. 설정 GUI 「사용자 사전」 페이지에서 추가/제거/수정.

#### 토글 단축키 — 지정한 키로 즉시 켜고 끄기

키 하나로 자동 오타 교정을 바로 켜고 끌 수 있다. **전체 토글의 기본값은 `Shift+F8`** 이고, 순방향·역방향 전용 단축키는 비어 있다(필요한 사람만 지정). 세 가지를 각각 따로 둔다.

- **전체 토글**: 자동 오타 교정 전체(마스터 스위치)를 켜고 끈다.
- **순방향(=정방향) 토글**: 순방향(영→한) 교정만 켜고 끈다. (일부 GUI 라벨은 "정방향"으로 표기하지만 같은 뜻이다.)
- **역방향 토글**: 역방향(한→영) 교정만 켜고 끈다.

<!-- @platform:linux -->
CLI로 지정한다.

```bash
# 기본값 (전체 토글 = Shift+F8)
unim-cli config set auto-typefix-toggle-keys "Shift+F8"

# 순방향/역방향을 각각 F10, F11 로
unim-cli config set auto-typefix-forward-toggle-keys F10
unim-cli config set auto-typefix-reverse-toggle-keys F11

# 여러 키를 쉼표로 (그중 아무거나 누르면 토글)
unim-cli config set auto-typefix-toggle-keys "Shift+F8,Ctrl+Left"

# 해제 — 빈 값을 주면 어떤 키도 가로채지 않는다
unim-cli config set auto-typefix-toggle-keys ""
```
<!-- @endplatform -->
<!-- @platform:windows -->
설정 창에서 지정한다 — **설정(§5) → 오타 교정 → 「토글 단축키」** 그룹의 세 칸에 원하는 키를 적는다. 여러 키를 쓰려면 쉼표로 구분하고(`Shift+F8,Ctrl+Left`), 쓰지 않으려면 칸을 비운다.
<!-- @endplatform -->

**설정 GUI(`unim-settings`, 5.2)** 에서는 세 칸이 **오타 교정** 페이지의 **「토글 단축키」** 그룹에 나란히 모여 있다.

<!-- @platform:linux -->
> 레거시 GTK 다이얼로그(`unim-settings-gtk`, 앱 메뉴에는 숨겨져 있음 — [§5](#5-설정-gui-투어) 참고)에서는 세 칸이 한 그룹에 모여 있지 않고 각 기능 그룹에 나뉘어 배치돼 있다 — **전체 토글**은 「오타 교정」 마스터 그룹(전체 켜기·끄기 스위치 옆), **순방향(정방향) 토글**은 「순방향」 그룹, **역방향 토글**은 「역방향」 그룹에 각각 놓인다.
<!-- @endplatform -->

> - **수정자 조합을 쓸 수 있다** — `Shift+F8`, `Ctrl+Left`, `Ctrl+Shift+F7` 처럼 적는다(`Ctrl`/`Control`, `Alt`, `Super`/`Win`/`Meta`, `Shift`. 대소문자·순서 무관). 수정자 없이 `F10` 처럼 적으면 종전대로 그 키 단독으로만 발동한다. 구분자는 `+` 가 정식이고, `+` 가 하나도 없는 표기에서는 `-` 도 허용한다(`Ctrl-F8` = `Ctrl+F8`). 단, `Ctrl+Shift-F8` 처럼 혼용하면 무효다.
> - **표기한 수정자가 정확히 눌렸을 때만** 발동한다. 그래서 지정하지 않은 조합(예: `Shift+F10` 컨텍스트 메뉴)은 UNIM 이 가로채지 않고 앱으로 그대로 간다.
> - 기본값은 `Shift+F8` 이다 — F9~F11 은 미디어 키나 리매핑(잘라내기/복사/붙여넣기 등)으로 OS 에 도달하지 못하는 키보드가 있어 피했다. 한자/이모지 키(맨 `F9`)와는 키가 달라 충돌이 없고, `Shift+F9` 처럼 F9 조합을 지정해도 맨 `F9` 팝업은 그대로다.
> - 키 이름은 `F1`~`F12` 처럼 UNIM 이 아는 이름이어야 한다(`ScrollLock`·`Pause`·`PrintScreen`·`Menu` 는 인식하지 않는다). 잘못 적으면 CLI 와 설정 앱이 저장 시 경고를 띄우고, 데몬 로그에도 파싱 실패가 기록된다 — 조용히 무시되지 않는다.
> - 쓰지 않으려면 목록을 비운다(`""`). 그러면 그 단축키는 아무 키도 소비하지 않는다.
> - GNOME(Wayland)에서는 `Ctrl+Left` 같은 Ctrl/Alt/Super 조합도 동작한다 — UNIM 확장이 토글 조합을 엔진으로 전달한다(확장 업데이트 후 재로그인 필요).
> - Windows(TSF)에서도 조합 표기가 리눅스와 동일하게 동작한다 — 기본값 `Shift+F8` 그대로 쓰면 된다.
> - 전체가 꺼진 상태에서 순방향만 토글로 켜도 실제 교정은 전체를 다시 켜야 발동한다(전체가 마스터 스위치). 순방향/역방향 토글은 각 방향의 플래그만 바꾼다.
> - 접근성 참고: `toggle-announce-beep` 설정을 켜면 이 토글도 한/영 전환과 같은 차등 비프(켜짐=상승음/꺼짐=하강음)로 상태를 알려 준다 — **Windows·Linux 공통**이다(리눅스는 daemon 이 `paplay`/`pw-cat`/`aplay` 중 PATH 에서 찾은 재생기로 소리를 낸다. 셋 다 없으면 무음 no-op). 기본값은 꺼짐 — [4.1 접근성 안내](#41-한영-모드-전환) 참고.

#### 비밀번호 필드 자동 보호

비밀번호·PIN 입력 칸에서는 자동 오타 교정이 **자동으로 꺼진다.** 앱이 "이 칸은 비밀번호"라고 알려 주면(`content_purpose`) UNIM 은 그 칸에 머무는 동안 순방향·역방향 교정을 모두 멈추고, 이미 쌓인 키 관측 버퍼·되돌리기 기록도 지운다. 덕분에 `dkssud`처럼 친 비밀번호가 한글로 자동 교정돼 값이 깨지는 일이 없다.

- 칸을 벗어나면 즉시 원래대로 돌아온다. 수동으로 토글해 둔 켜짐/꺼짐 상태는 그대로 유지된다 — 비밀번호 보호는 그 위에 잠깐 덮이는 안전장치일 뿐이다.
<!-- @platform:linux -->
- 이 보호는 앱이 비밀번호 칸임을 알려 줄 때 동작한다. 알려 주지 않는 일부 환경(XIM 레거시 앱, content-purpose 를 보내지 않는 일부 Wayland 컴포지터·웹폼)에서는 자동 감지가 안 될 수 있다 → [FAQ](../faq/README-ko.md) Q9 참고.
<!-- @endplatform -->
<!-- @platform:windows -->
- 이 보호는 앱이 비밀번호 칸임을 알려 줄 때 동작한다. 알려 주지 않는 앱(자체 입력 처리를 하는 일부 프로그램, 비밀번호임을 표시하지 않는 일부 웹 폼)에서는 자동 감지가 안 될 수 있다 → [FAQ](../faq/README-ko.md) Q9 참고.
<!-- @endplatform -->

### 4.5 자동 영문 모드 전환 (Auto-English-Mode)

vim 명령 모드(`Esc`), CLI 슬래시 명령(`/`) 같은 비한글 컨텍스트에 들어갈 때 자동으로 영문으로 전환되게 하는 **opt-in** 기능. 기본 비활성.

- 활성화: 설정 GUI → 「일반」 → 「자동 영문 전환」 그룹 → `자동 영문 전환 사용` ON.
- 트리거 키: 기본은 `Escape`, `Slash`. 필요 시 `ShiftSemicolon`(:), `ShiftSlash`(?) 같은 가상 이름을 추가.
- 동작: 한글 모드에서 트리거 키를 누르면 (1) 조합 commit → (2) 영문 모드로 영구 전환 → (3) 트리거 키 자체는 그대로 앱에 전달.

> 한/영 토글 키와 트리거 키가 겹치면 토글 분기가 우선이라 자동 전환이 발동하지 않는다. 비밀번호 필드는 이미 강제 영문이라 영향 없음.

> **트리거 표기**: `key:<키이름>` / `char:<문자>` 형식으로 적는다(예: `key:Escape`, `char:/`). 접두사 없는 옛 표기(`Escape`)도 그대로 인식하고, `key:Ctrl+B` 같은 수정자 조합도 쓸 수 있다. 잘못 적으면 CLI 와 설정 앱이 저장 시 경고를 띄우고, 데몬 로그에도 파싱 실패가 기록된다 — 조용히 무시되지 않는다.

### 4.6 단어 단위 입력 (조합 확정 단위)

기본은 **음절 단위** 확정 — 한 글자가 완성되면 바로 확정된다. **단어 단위**로 두면 스페이스·구두점 같은 단어 경계까지 조합을 밑줄(preedit) 로 누적했다가 한 번에 확정한다. 조합 도중 `BackSpace` 는 자모 단위로 되돌아가고, 자동 오타 교정(4.4)의 역방향 교정과도 더 자연스럽게 맞물린다.

세 가지 값이 있다.

- **음절 (syllable)**: 항상 음절 단위 확정. 가장 예측 가능.
- **단어 (word)**: 모든 대상 앱에서 단어 단위 누적.
- **스마트 (smart, 기본)**: `word-mode-apps` 목록에 등록된 앱에서만 단어 단위, 그 외에는 음절 단위. 기본 목록은 `winword.exe`(Windows) 뿐이라 리눅스에서는 **아무것도 등록하지 않으면 사실상 음절 단위**로 동작한다(무회귀).

**켜는 법**

<!-- @platform:linux -->
```bash
# 전역 단어 단위
unim-cli config set commit-unit word

# 스마트 + 특정 앱만 단어 단위 (예: LibreOffice)
unim-cli config set commit-unit smart
unim-cli config set word-mode-apps "winword.exe,soffice"
```

설정 GUI 에서는 「일반」 → 「입력 모드」 그룹의 **조합 확정 단위** 콤보에서 고른다. `word-mode-apps` 는 CLI/`config.yaml` 로 편집한다(정확일치, 대소문자 무시). 리눅스 앱 ID 예시: LibreOffice `soffice`, 앱 ID 는 창별 독립 모드에서 로그로 확인할 수 있다.
<!-- @endplatform -->
<!-- @platform:windows -->
설정 창(§5)의 「일반」 → 「입력 모드」 그룹에서 **조합 확정 단위** 콤보로 고른다.

**스마트**(기본)로 두면 **MS Word(`winword.exe`)에서만** 단어 단위가 되고 나머지 앱은 음절 단위다. 다른 앱도 단어 단위로 쓰려면 그 앱의 **실행 파일 이름**을 「단어 모드 앱」 목록에 추가한다(정확일치, 대소문자 무시). 실행 파일 이름은 작업 관리자 → **세부 정보** 탭에서 확인할 수 있다.

```yaml
# %APPDATA%\unim\config.yaml — 직접 편집해도 되고, 잠시 뒤 자동 반영된다
korean:
  commit_unit: Smart          # Syllable / Word / Smart — 대문자로 시작
  word_mode_apps:
    - winword.exe
```
<!-- @endplatform -->

**적용되지 않는 경우 (안전장치)**

<!-- @platform:linux -->
- **터미널** (ghostty·kitty·wezterm·alacritty·foot·gnome-terminal·konsole·xterm 등): preedit 이 취약해 항상 음절 단위.
- **XIM** (xterm 계열 레거시 앱): 구조적으로 단어 단위 불가 — 항상 음절 단위.
- **순수 Wayland·Flatpak/Snap(ibus)**: 앱 식별이 안 돼 현재는 제외(음절 단위).
- **모아치기(동시치기)** 를 켠 상태: 단어 단위와 양립하지 않아 항상 음절 단위로 동작한다.
<!-- @endplatform -->
<!-- @platform:windows -->
- **「단어 모드 앱」 목록에 없는 앱**(스마트 모드일 때): 음절 단위로 동작한다. 즉 기본 상태에서는 MS Word 외에는 영향이 없다.
- **모아치기(동시치기)** 를 켠 상태: 단어 단위와 양립하지 않아 항상 음절 단위로 동작한다.
<!-- @endplatform -->

> 단어 단위에서 자동 오타 교정 역방향(한→영)은 조합만 교체하고 앞선 확정 텍스트는 건드리지 않는다. 위 비대상 경우에는 자동으로 음절 단위로 내려가므로 데이터 손실 걱정 없이 켜 두어도 된다.

---

## 5. 설정 GUI 투어

**`unim-settings`**(Slint, 리눅스·Windows 공통 단일 코드베이스)가 유일한 설정 진입점이다. 즉 아래 4개 페이지의 구성과 항목은 **양쪽 운영체제가 동일**하다 — 다른 것은 창을 여는 방법뿐이다.

<!-- @platform:linux -->
**🐧 리눅스에서 여는 법** — 앱 메뉴의 「UNIM 설정」 항목, 첫 실행 마법사, 트레이 메뉴가 모두 이 실행 파일을 띄운다. 터미널에서 직접 띄워도 된다.

```bash
unim-settings &
```

> **레거시 GTK 다이얼로그(`unim-settings-gtk`)에 관해**: GTK4+libadwaita로 짠 예전 설정창은 계속 패키지에 포함되지만 **앱 메뉴에는 더 이상 노출되지 않는다**(`.desktop` 파일에 `NoDisplay=true`). 직접 `unim-settings-gtk &`로 띄울 수는 있으나 신규 설정 항목이 이쪽에는 반영되지 않을 수 있다 — 위 `unim-settings`를 기준으로 삼는다. Qt 다이얼로그(`unim-gui-qt`)는 이미 폐기됐고, 트레이 아이콘은 `unim-indicator`, 한자·특수문자·이모지 팝업은 `unim-popup-service`가 각각 별도 프로세스로 담당한다(§2.5 참고).
<!-- @endplatform -->

<!-- @platform:windows -->
**🪟 Windows에서 여는 법** — 세 가지 중 편한 것을 쓴다.

| 방법 | 순서 |
|------|------|
| **A. 언어바** (추천) | 작업 표시줄 시계 옆의 `한`/`A` 표시를 **마우스 오른쪽 버튼**으로 클릭 → **설정** |
| **B. 시작 메뉴** | 시작 메뉴 → **UNIM** 폴더 → **UNIM Settings** |
| **C. Windows 언어 설정** | 설정 → 시간 및 언어 → 언어 및 지역 → 한국어 → 키보드에서 UNIM 옵션 진입 |

> ⚠️ 언어바를 **왼쪽 클릭하면 한/영 전환**이다. 설정은 **오른쪽 클릭 → 설정**.
>
> 직접 실행하려면 `C:\Program Files\UNIM\unim-settings.exe` 다.

> **저장 버튼이 없다**: 바꾸는 즉시 적용되고 파일에도 저장된다. 창을 그냥 닫아도 된다.
>
> 같은 창의 사이드바 아래쪽 **[도움말]** 버튼을 누르면 지금 읽고 있는 이 매뉴얼이 기본 브라우저로 열린다.
<!-- @endplatform -->

페이지 구성 (좌측 내비게이션 4개):

### 5.1 페이지 1 — 일반

<!-- screenshot: settings-general -->

| 그룹 | 항목 | 비고 |
|------|------|------|
| **자판 옵션** | 한글 자판 / 영문 자판 | `ko_2bulstd`(두벌식 표준) 등 — §7.1 참고. 자판별 동적 옵션(예: 세벌식 390의 순아래받침)도 이 그룹에 나타난다 |
|  | 한글 확정 단위 | `음절`/`단어`/`스마트` — [4.6](#46-단어-단위-입력-조합-확정-단위) 참고 |
| **입력 모드** | 시작 입력 모드 | 데몬 시작 시 한글/영문 중 어느 쪽으로 켤지 |
|  | 모드 공유 | `전역 공유`(기본) / `앱별 독립` — [4.1](#41-한영-모드-전환) 참고 |
|  | 앱별 규칙 | 특정 앱(창 식별자·클라이언트 이름 부분 일치)에서 항상 지정 모드로 시작하도록 규칙 추가. 「모드 공유=앱별 독립」일 때 효과가 크다 |
| **자동 영문 전환** | 사용 스위치 + 트리거 키 | 기본 OFF — [4.5](#45-자동-영문-모드-전환-auto-english-mode) 참고 |
| **접근성** | 원클릭 프리셋 (「한 손 사용」/「넉넉한 타이밍」) + 개별 스위치 | 아래 설명 참고 |

> **접근성 프리셋**: 「한 손 사용」은 세벌식 순아래 자판 + 비수정자 토글키 + 모아치기 끔 + 자동반복 억제를 한 번에 적용한다. 「넉넉한 타이밍」은 자동반복 억제 + 오타 교정 판정 시간 확대 + (지원 자판이면) 모아치기 조합창을 넉넉하게 잡는다. 프리셋 적용 후에도 개별 스위치로 세부 조정할 수 있다.

<!-- @platform:linux -->
> **조합키 자동반복 억제(접근성)**: 키를 계속 누르고 있으면 운영체제가 같은 키를 빠르게 반복 입력하는데(자동 반복), 이 옵션을 켜면 데몬이 그 반복을 무시한다. 손 떨림 등으로 키를 오래 누르게 되는 지체장애 사용자를 위한 기능이며, **Windows·Linux 공통으로 데몬이 집행**한다(과거 리눅스에서는 미집행이었으나 v0.4.0에서 정정). 억제 대상은 **한/영 토글 키와 한글 모드의 문자 키**이며, 백스페이스·방향키 같은 편집키와 영문 직접 입력의 반복은 그대로 둔다. Wayland·Qt5/6·GNOME 확장에서는 반복 여부를 정확히 가려내고, GTK3/4·XIM·ibus 호환 경로에서는 80ms 시간창으로 근사 판정하므로 첫 반복 1회는 통과될 수 있고 시스템 키 반복 간격을 80ms보다 길게 잡았다면 걸러지지 않을 수 있다(어느 경우든 "덜 막는" 쪽으로 안전하게 동작). 기본값은 꺼짐이며, `unim-cli config set ignore-key-repeat true` 로도 켤 수 있다. GNOME 확장 사용자는 재로그인 후 적용된다.
>
> **이모지 입력**은 별도 스위치가 없다 — 한자 팝업과 같은 경로로 항상 켜져 있으며, [키보드 단축키 가이드](../keyboard-shortcuts/README-ko.md#이모지-팝업-단축키-super) 의 `Super+.`(또는 등록한 단축키)로 호출한다.
>
> **GNOME 확장 전용 설정**(패널 인디케이터 표시 여부, 수동 변환 단축키 등)은 이 앱이 아니라 `gnome-extensions prefs unim-gnome@from104.github.io` 로 별도로 연다 — [키보드 단축키 가이드 GNOME 절](../keyboard-shortcuts/README-ko.md) 참고.
<!-- @endplatform -->
<!-- @platform:windows -->
> **조합키 자동반복 억제(접근성)**: 키를 계속 누르고 있으면 운영체제가 같은 키를 빠르게 반복 입력하는데(자동 반복), 이 옵션을 켜면 입력기가 그 반복을 무시한다. 손 떨림 등으로 키를 오래 누르게 되는 지체장애 사용자를 위한 기능이며, 리눅스판과 **같은 방식으로 동작**한다. 억제 대상은 **한/영 토글 키와 한글 모드의 문자 키**이며, 백스페이스·방향키 같은 편집키와 영문 직접 입력의 반복은 그대로 둔다. 기본값은 꺼짐이다.
>
> **이모지 입력**은 별도 스위치가 없다 — 한자 팝업과 같은 경로로 항상 켜져 있다. 조합 중이 아닌 상태에서 **한자 키(또는 F9)** 를 누르면 이모지 팝업이 뜬다. 즉 같은 키가 상황에 따라 **한자 · 특수문자 · 이모지** 세 용도로 자동 전환된다.
<!-- @endplatform -->

### 5.2 페이지 2 — 오타 교정

<!-- screenshot: settings-typefix -->

| 그룹 | 항목 | 비고 |
|------|------|------|
| **활성화** | 자동 오타 교정 사용 | 마스터 스위치. OFF면 순방향·역방향 둘 다 정지 |
| **교정 강도** | 보수적 / 표준 / 적극적 프리셋 | 임계값·단어 길이·감지창을 한 번에 맞춘다. 세부값은 아래 「고급 설정」에서 직접 조정 가능 |
| **교정 방향** | 정방향 사용 / 역방향 사용 | 각각 독립적으로 켜고 끌 수 있다 — 4.4 참고 |
| **토글 단축키** | 전체 켜기·끄기 / 정방향 켜기·끄기 / 역방향 켜기·끄기 (세 칸 한 그룹) | 지정한 키로 즉시 토글. 비우면 사용 안 함. `Shift+F9`처럼 수정자 조합 가능 — 4.4 참고 |
| **고급 설정**(펼침) | 한글 음절 임계값 / 영문 단어 최소 길이 / 정방향·역방향 감지창(ms) / 임시 항목 만료(시간) / 관찰 타임아웃(초) — 전부 슬라이더 | 대부분은 위 「교정 강도」 프리셋으로 충분하다 |
| **옵션**(고급 설정 안) | 영어 단어는 교정 건너뛰기 / 완성된 음절은 교정 건너뛰기 / 되돌리기(롤백) 감지 / 사용자 사전 사용 | — |
| — | 기본값으로 복원 | 오타 교정 설정을 초기값으로 되돌린다(5초 안에 취소 가능) |

### 5.3 페이지 3 — 교정 억제 단어

<!-- screenshot: settings-blacklist -->

한 개의 목록에 **임시(Tentative)**·**확정(Confirmed)**·**비활성(Inactive)** 세 상태의 억제 단어가 함께 표시된다(상태별 표시로 구분, 예전 GTK 다이얼로그의 3개 섹션 구성과는 다르다). 선택한 항목을 「선택 영구 활성」(확정으로 승격) 또는 「선택 삭제」할 수 있고, 「모두 삭제」로 전체를 비울 수도 있다(5초 안에 되돌리기 가능).

> 데몬이 파일을 갱신해도 GUI가 즉시 반영한다. 따로 리로드 안 해도 된다.

### 5.4 페이지 4 — 사용자 사전 (역방향 화이트리스트)

<!-- screenshot: settings-userdict -->

「단어」+「메모(선택)」를 입력해 [추가]하면 영어 단어 ↔ 한글 시퀀스 매핑이 등록된다. 예: `wave` ↔ `ㅈㅐㅍㅁ`. 아래 목록에서 선택 후 [선택 삭제]로 제거한다. reverse 교정에서 우선 매칭.

---

<!-- @platform:windows -->
## 5.6 자판 도구 (Keymap Studio / Typing Practice)

**🪟 Windows 에는 아직 포함되지 않는다.** 자판을 눈으로 보며 편집하는 도구(Keymap Studio)와 타자 연습 도구(Typing Practice)는 현재 리눅스 패키지에만 들어 있다. Windows 설치본(MSI)에는 입력기 본체·설정 창·팝업 렌더러만 담긴다.

자판 **선택**은 Windows 에서도 설정 창의 「일반」 → 「자판 옵션」에서 그대로 할 수 있다. 없는 것은 자판을 새로 만들거나 타자 연습을 하는 도구뿐이다.

<!-- @endplatform -->

<!-- @platform:linux -->
## 5.6 자판 도구 (Keymap Studio / Typing Practice)

설정 GUI와 별개로, 자판을 눈으로 보고·편집하고·연습하는 두 GTK4 도구가 함께 설치된다. 각자
고유 아이콘으로 앱 목록에 등록된다(`io.github.from104.unim.KeymapStudio`,
`io.github.from104.unim.TypingPractice`).

```bash
unim-keymap-studio &      # 자판 보기·편집
unim-typing-practice &    # 타자 연습
```

### 5.6.1 unim-keymap-studio — 자판 보기·편집

<!-- screenshot: keymap-studio -->

한국어/영문 자판이 키마다 어떤 자모·문자를 내는지 시각적으로 확인하고, 사용자 자판을 만든다.

- **헤더 3단 드롭다운**: 「언어 > 출처 > 자판」 순서로 좁혀 선택. 예: 한국어 > 사용자 정의 >
  `my_3bul_variant`.
- **4개 탭**: 「기본」(메타 정보) · 「자판」(키별 매핑 그리드) · 「조합」(자모 결합 규칙) ·
  「확장」(rule_set 토글). **「조합」·「확장」 탭은 한글 자판을 선택했을 때만 나타난다** (영문
  자판에는 자모 조합 개념이 없으므로).
- **헤더 우측 버튼**: [도움말] (F1) · [설정] · [메뉴].
- **저장 정책**: 빌트인 자판은 읽기 전용이라 「다른 이름으로 저장」만 가능. 사용자 자판은
  「저장」 + 「다른 이름으로 저장」 둘 다 된다. 새로 만든 사용자 자판은
  `~/.config/unim/layouts/` 에 JSON으로 떨어지고, 데몬이 자동 스캔해 설정 GUI 자판 목록에 나타난다.

#### 단축키

| 키 | 동작 |
|----|------|
| F1 | 도움말 |
| Ctrl + N | 새 자판 |
| Ctrl + D | 현재 자판 복제 |
| Ctrl + S | 저장 (사용자 자판) |
| Ctrl + Shift + S | 다른 이름으로 저장 |
| Ctrl + E | 내보내기 (export) |
| Ctrl + I | 가져오기 (import) |
| Ctrl + 1 / 2 / 3 / 4 | 탭 전환 (기본 / 자판 / 조합 / 확장) |

### 5.6.2 unim-typing-practice — 타자 연습

<!-- screenshot: typing-practice -->

현재 **활성 자판**(데몬이 쓰는 자판)으로 타자 연습을 한다. 자판을 바꾸면 자동으로 다시 로드된다.

- **측정 지표**: WPM(분당 단어), CPM(분당 글자), 정확도, 그리고 **오타 히트맵** — 어떤 키에서
  실수가 잦은지 키보드 위에 색으로 표시.
- **연습 글감**: 내장 단어/문장 외에 파일(Ctrl+O)이나 클립보드(Ctrl+Shift+V)에서 가져올 수 있다.
- keymap-studio와 **동일한 5행 키보드 위젯**을 공유하므로 보이는 자판이 일관된다.

#### 단축키

| 키 | 동작 |
|----|------|
| F1 | 도움말 |
| Ctrl + R | 다시 시작 (재시작) |
| Ctrl + Shift + C | 결과 복사 |
| Ctrl + 1 | 연습 화면 |
| Ctrl + 2 | 결과 화면 |
| Ctrl + O | 파일에서 글감 가져오기 |
| Ctrl + Shift + V | 클립보드에서 글감 가져오기 |
<!-- @endplatform -->

---

## 6. 키 매핑 치트시트

<!-- @platform:linux -->
**🐧 리눅스**

| 상황 | 키 | 결과 |
|------|----|------|
| 어디서나 | 한/영 (또는 Shift+Space) | 모드 토글 |
| 한글 모드, 글자 입력 후 | 한자 (F9) | 한자 팝업 |
| 한자 팝업 | 숫자 1~9 | 직접 선택 |
| 한자 팝업 | 화살표 | 후보 이동 |
| 한자 팝업 | `←`/`→` 또는 PageUp/PageDown | 페이지 이동 (wrap-around) |
| 한자 팝업 | 마우스 ◀ / ▶ | 페이지 이동 (wrap-around, 단일 페이지면 숨김) |
| 한자 팝업 | 마우스 우클릭 | **환경별 차이**: GNOME=★ 즐겨찾기 토글 / GTK·Qt IM·XIM=다음 페이지 / 기타=동작 없음 (§4.2) |
| 한자 팝업 | Enter | 포커스 항목 확정 |
| 한자 팝업 | ESC | 취소 |
| 한자 팝업 | `.` (마침표) | 9칸 ↔ 81칸 토글 |
| 한자 팝업 | Space | 즐겨찾기 ☆/★ 토글 (해제 시 cursor 셀 140ms flash) |
| 한글 모드, 자음만 입력 | 한자 (F9) | 특수문자 팝업 |
| 한글 입력 중 | BackSpace | 마지막 자모 1개 삭제 |
| forward 교정 후 후회 | BS + 한/영 | Tentative 학습 트리거 |
| 자동 영문 모드 활성 시 | `Esc` 또는 `/` | 영문 강제 전환 + 키 전달 |
<!-- @endplatform -->
<!-- @platform:windows -->
**🪟 Windows**

| 상황 | 키 | 결과 |
|------|----|------|
| 어디서나 | 한/영 (또는 오른쪽 Alt) | 모드 토글 |
| 한글 모드, 글자 입력 후 | 한자 (F9) | 한자 팝업 |
| 한글 모드, 자음만 입력 | 한자 (F9) | 특수문자 팝업 |
| 조합 중이 아닐 때 | 한자 (F9) | 이모지 팝업 |
| 한자 팝업 | 숫자 1~9 | 직접 선택 |
| 한자 팝업 | 화살표 | 후보 이동 |
| 한자 팝업 | `←`/`→` 또는 PageUp/PageDown | 페이지 이동 (wrap-around) |
| 한자 팝업 | 마우스 ◀ / ▶ | 페이지 이동 (wrap-around, 단일 페이지면 숨김) |
| 한자 팝업 | 마우스 좌클릭 (후보 셀) | 그 후보 선택 |
| 한자 팝업 | 마우스 좌클릭 (팝업 바깥) | 팝업 닫기 — 클릭은 아래 창에 그대로 전달 |
| 한자 팝업 | **마우스 우클릭** | **동작 없음** (의도된 무동작 — 팝업이 닫히지도, 후보가 확정되지도 않는다) |
| 한자 팝업 | Enter | 포커스 항목 확정 |
| 한자 팝업 | ESC | 취소 |
| 한자 팝업 | `.` (마침표) | 9칸 ↔ 81칸 토글 |
| 한자 팝업 | Space | 즐겨찾기 ☆/★ 토글 (해제 시 cursor 셀 140ms flash) |
| 한글 입력 중 | BackSpace | 마지막 자모 1개 삭제 |
| forward 교정 후 후회 | BS + 한/영 | Tentative 학습 트리거 |
| 자동 영문 모드 활성 시 | `Esc` 또는 `/` | 영문 강제 전환 + 키 전달 |
<!-- @endplatform -->

---

<!-- @platform:windows -->
## 7. 설정을 손으로 바꾸기

Windows 설치본에는 명령줄 도구(`unim-cli`)가 들어 있지 않다. 설정은 **설정 창(§5)** 에서 바꾸는 것이 정식 경로이고, 창에 노출되지 않은 항목만 설정 파일을 직접 편집한다.

### 7.1 지원 자판

설정 창의 「일반」 → 「자판 옵션」에서 고른다.

- 한국어: 두벌식 표준(`ko_2bulstd`), 세벌식 390, 세벌식 391, 세벌식 순아래
- 영어: QWERTY, Dvorak, Colemak, Colemak-DH, Workman

### 7.2 설정 파일 직접 편집

`%APPDATA%\unim\config.yaml` 을 메모장 등으로 연다. 주소창에 `%APPDATA%\unim` 을 붙여 넣으면 바로 그 폴더가 열린다.

```powershell
# 탐색기로 설정 폴더 열기
explorer "$env:APPDATA\unim"

# 메모장으로 바로 열기
notepad "$env:APPDATA\unim\config.yaml"
```

저장하면 **잠시 뒤 자동으로 반영**된다 — 재부팅이나 입력기 재시작이 필요 없다.

> ⚠️ YAML 은 들여쓰기가 문법이다. 탭 대신 **스페이스**를 쓰고, 편집 전에 파일을 한 벌 복사해 두는 것이 안전하다. 파일이 깨지면 UNIM 은 기본 설정으로 되돌아간다.
<!-- @endplatform -->

<!-- @platform:linux -->
## 7. CLI 사용법 (`unim-cli`)

CLI는 두 용도. (1) 한↔영 변환 필터, (2) 설정 관리.

### 7.1 변환 필터

```bash
# 영타를 한글로 (compose, 기본 모드)
echo "dkssudgktpdy" | unim-cli
# → 안녕하세요

# 한글을 영타로 (decompose)
echo "안녕하세요" | unim-cli -d
# → dkssudgktpdy

# 자판 지정
echo "ekswn" | unim-cli -k 2bul       # 두벌식 (기본)
echo "j;ax" | unim-cli -k 390         # 세벌식 390

# 파일 입출력
unim-cli -o out.txt input.txt
```

지원 자판:
- 한국어: `2bul`, `390`, `391`, `noshift`
- 영어: `qwerty`, `dvorak`, `colemak`, `colemak_dh`, `workman`

### 7.2 설정 관리

```bash
# 현재 설정 전체 보기 (grep으로 특정 키만 확인해도 된다)
unim-cli config show

# 값 설정 — 키 이름은 케밥 표기(하이픈)만 받는다
unim-cli config set auto-typefix true
unim-cli config set auto-typefix-tentative-expiry-hours 6
unim-cli config set auto-english true

# 조합 확정 단위 (음절/단어/스마트) — 4.6 참고
unim-cli config set commit-unit word

# 자동 오타 교정 토글 단축키 (쉼표 구분, 수정자 조합 가능) — 4.4 참고
unim-cli config set auto-typefix-toggle-keys "Shift+F8"
unim-cli config set auto-typefix-forward-toggle-keys F10
unim-cli config set auto-typefix-reverse-toggle-keys F11

# 자판 프로필 관리
unim-cli config layout list                    # 내장 + 사용자 프로필 목록
unim-cli config layout describe ko_3bul390     # 프로필 상세
unim-cli config layout validate my.json        # 사용자 정의 자판 검증
```

> 설정 변경은 데몬에 즉시 반영된다. config.yaml ↔ unim-cli ↔ 설정 GUI(`unim-settings`) 3지점이 항상 싱크되도록 설계됐다.
<!-- @endplatform -->

---

## 8. 설정 파일 위치 / 백업

<!-- @platform:linux -->
| 파일 | 용도 | 백업 권장 |
|------|------|----------|
| `~/.config/unim/config.yaml` | 일반 설정 (자판·모드·자동 영문 등) | YES |
| `~/.config/unim/typefix-blacklist.yaml` | 학습된 교정 억제 사전 | YES |
| `~/.config/unim/userdict.yaml` | reverse 사용자 사전 | YES |
| `~/.config/unim/layouts/*.json` | 사용자 정의 자판 v1 프로필 | YES |
| `~/.unim-errors.log` | 디버그 로그 (`UNIM_DEVELOP=1` 시) | NO (휘발) |

```bash
# 통째 백업
tar -czf unim-backup-$(date +%F).tar.gz -C ~/.config unim
# 복원
tar -xzf unim-backup-2026-04-26.tar.gz -C ~/.config
systemctl --user restart unim-daemon
```
<!-- @endplatform -->

<!-- @platform:windows -->
설정은 전부 **`%APPDATA%\unim\`** 아래에 있다. 보통 `C:\Users\<사용자이름>\AppData\Roaming\unim\` 이며, `AppData` 는 숨김 폴더라 탐색기 주소창에 `%APPDATA%\unim` 을 붙여 넣는 편이 빠르다.

| 파일 | 용도 | 백업 권장 |
|------|------|----------|
| `config.yaml` | 일반 설정 (자판·모드·오타 교정 등) | YES |
| `typefix-blacklist.yaml` | 학습된 교정 억제 사전 | YES |
| `typefix-userdict.yaml` | 역방향 사용자 사전 | YES |

```powershell
# 통째 백업 — 바탕화면에 날짜가 붙은 zip 을 만든다
Compress-Archive -Path "$env:APPDATA\unim" `
  -DestinationPath "$env:USERPROFILE\Desktop\unim-backup-$(Get-Date -Format yyyy-MM-dd).zip"

# 복원 — 기존 파일을 덮어쓴다
Expand-Archive -Path "$env:USERPROFILE\Desktop\unim-backup-2026-07-27.zip" `
  -DestinationPath $env:APPDATA -Force
```

> 복원 뒤에는 **재로그인**하면 확실히 반영된다. 설정 파일 변경은 보통 잠시 뒤 자동 반영되지만, 여러 파일을 한꺼번에 되돌린 경우에는 재로그인이 안전하다.

> **UNIM 을 지워도 이 폴더는 남는다.** 설정을 완전히 없애려면 제거 후 `%APPDATA%\unim` 폴더를 직접 지운다. 반대로 재설치 전에 이 폴더를 그대로 두면 설정이 그대로 이어진다.
<!-- @endplatform -->

---

## 9. 다음 단계

- 동작이 이상하다 → [트러블슈팅](../troubleshooting/README-ko.md)
- 다른 IME와 비교 / 안정성 / 마이그레이션 → [FAQ](../faq/README-ko.md)
- 0.4.0 변경 내역 → [릴리즈 노트 0.4.0](../release-notes/0.4.0/README.md)
- 과거 버전 변경 내역 → [0.3.0](../release-notes/0.3.0/README.md) / [0.2.0](../release-notes/0.2.0/RELEASE_NOTES-ko.md)
- 기여하고 싶다 → [`CONTRIBUTING.md`](../../../CONTRIBUTING.md)
- 핵심 동작 명세 → [`IME_BEHAVIOR.md`](../../dev/architecture/IME_BEHAVIOR.md), [`docs/dev/specs/POPUP_SPEC.md`](../../dev/specs/POPUP_SPEC.md)

---

문서 버전: 0.4.0 / 작성일: 2026-07-27 / 라이선스: 본문 라이선스는 프로젝트와 동일.
