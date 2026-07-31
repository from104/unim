# UNIM

Rust로 만든 한국어 입력기. 리눅스와 Windows에서 같은 엔진을 쓴다.

MIT · Rust 1.78+ · Linux (deb/rpm) · Windows 10/11 (MSI) · 현재 0.4.0

---

## 왜

한/영 전환을 깜빡하는 건 누구나 한다. 대부분은 지우고 다시 친다.
키 하나를 누르는 비용이 큰 사람에게는 그 '다시 치기'가 문제다.

UNIM은 거기서 출발했다. 잘못 친 걸 알아서 되돌리고, 환경이 바뀌어도 같은 키가
같게 동작하고, 오래 눌린 키가 멋대로 반복되지 않는 입력기.

그래서 이 프로젝트의 기능 목록은 대체로 "입력 횟수를 줄이는 것"과
"실수를 사용자가 아니라 프로그램이 수습하는 것"으로 수렴한다.
자동 오타 교정, 토글 단축키, 자동 반복 무시, 전환 소리 알림이 전부 같은 이유에서 나왔다.

## 무엇을 하나

| 기능 | 내용 |
|------|------|
| **한/영 전환** | 한/영 키, `Shift+Space`, 오른쪽 Alt(설정 시). 프론트엔드가 달라도 같은 키가 같게 동작한다 |
| **자동 오타 교정 (AutoTypeFix)** | 모드를 잘못 두고 친 글자를 되돌린다. `dkssud` → `안녕`, `ㅈㅐㅍㅁ` → `wave`. [아래 절 참고](#자동-오타-교정) |
| **자동 영문 전환** | 지정한 키·문자에서 영문 모드로 넘어간다. `Ctrl+B` 같은 수정자 조합도 지정할 수 있고, 그 키는 앱에 그대로 전달된다 |
| **비밀번호 필드 보호** | 비밀번호 칸에서는 영문 모드로 강제 전환하고, 그동안의 키는 어떤 기록에도 남기지 않는다 |
| **한자·특수문자·이모지** | `F9`로 9칸/81칸 격자 팝업. 즐겨찾기, 마우스 페이지 이동. 조합 없이 누르면 이모지 |
| **모아치기** | 여러 자모를 동시에 눌러 한 음절을 만드는 자판(안마태 등). 동시 입력 판정 시간 조절 가능 |
| **단어 단위 입력** | 조합 확정을 음절 대신 단어 단위로. 터미널처럼 위험한 곳에서는 자동으로 음절 단위로 돌아간다. **한글 조합에만 적용된다** — 영문은 조합할 것이 없어 밑줄 없이 바로 확정된다 |
| **자동 반복 무시** | 키를 오래 눌러 생기는 auto-repeat을 무시한다. 손을 빨리 떼기 어려운 사람용 |
| **전환 소리 알림** | 모드가 바뀔 때 짧은 비프(한글 880Hz·영문 440Hz). 화면을 안 봐도 현재 모드를 안다 |
| **자판** | 2벌식, 3벌식 390/391/순아래, QWERTY/Dvorak/Colemak/Workman. 자판 편집기와 타자 연습 도구 포함(리눅스) |

## 설치

### 리눅스

바이너리 패키지는 amd64/x86_64만 제공한다.

| 배포판 | 최소 버전 | 형식 |
|--------|----------|------|
| Ubuntu | 24.04 (noble) | `.deb` |
| Debian | 13 (trixie) | `.deb` |
| Fedora | 43 | `.rpm` |

```bash
curl -fsSL https://raw.githubusercontent.com/from104/unim/main/install.sh | bash
```

배포판을 감지해 apt 또는 dnf로 설치한다. 불변 시스템(Silverblue·Kinoite·Bazzite)은
`rpm-ostree` 레이어링으로 처리한다. 모든 패키지는 SHA256으로 검증하고, 검증에 실패하면
아무것도 설치하지 않고 멈춘다.

```bash
# 업데이트 (이미 최신이면 내려받지 않는다)
curl -fsSL .../install.sh | bash -s -- --update

# 확인만 (아무것도 바꾸지 않는다)
curl -fsSL .../install.sh | bash -s -- --check

# 버전 고정
UNIM_VERSION=v0.4.0 curl -fsSL .../install.sh | bash
```

`curl | bash`가 내키지 않으면 받아서 읽고 실행하면 된다. 전체 옵션은 `bash install.sh --help`.

```bash
curl -fsSL https://raw.githubusercontent.com/from104/unim/main/install.sh -o install.sh
less install.sh && bash install.sh
```

Debian 12(bookworm)는 시스템 라이브러리가 오래돼 패키지를 쓸 수 없다.
그 밖의 배포판(openSUSE·Arch·RHEL 계열 등)은 소스 빌드를 쓴다 —
[사용자 매뉴얼](docs/user/user-guide/README-ko.md)의 소스 빌드 절 참고.

### Windows

Windows 10/11 64비트. PowerShell에서 실행하고, 설치할 때 UAC 승인이 한 번 필요하다.

```powershell
irm https://raw.githubusercontent.com/from104/unim/main/install.ps1 | iex
```

Releases에서 최신 MSI를 받아 `msiexec`으로 설치한다. 기본 경로는 버전 비교 없이 항상
최신을 내려받아 재설치한다. 설치된 버전과 비교해 필요할 때만 갱신하려면 `-Update`를 쓴다.

MSI는 SHA256(`SHA256SUMS-msi`)으로 검증하고, 관리자로 승격된 프로세스가 설치 직전에
해시를 한 번 더 확인한다. 코드 서명은 아직 없어서 SmartScreen 경고가 뜬다 —
"추가 정보 → 실행"으로 넘어간다.

```powershell
# 업데이트
& ([scriptblock]::Create((irm .../install.ps1))) -Update

# 확인만
& ([scriptblock]::Create((irm .../install.ps1))) -Check

# 버전 고정 (해당 릴리스에 SHA256SUMS-msi가 있어야 한다)
$env:UNIM_VERSION='v0.4.0'; irm .../install.ps1 | iex
```

받아서 읽고 실행하는 쪽:

```powershell
irm https://raw.githubusercontent.com/from104/unim/main/install.ps1 -OutFile install.ps1
notepad install.ps1
powershell -ExecutionPolicy Bypass -File install.ps1
```

조합·팝업·자동 오타 교정·설정·언어바가 단일 TSF DLL(32/64비트)에 들어 있고, 리눅스와 같은
코어를 쓴다. 자판 편집기와 타자 연습 도구는 MSI에 없다.

### 수동 설치

[Releases](https://github.com/from104/unim/releases)에서 패키지와 체크섬을 받아 검증 후 설치한다.

```bash
# Ubuntu / Debian
sha256sum -c SHA256SUMS && sudo apt install ./unim*.deb

# Fedora
sha256sum -c SHA256SUMS-rpm && sudo dnf install ./unim*.rpm
```

```powershell
Get-Content SHA256SUMS-msi                         # 기대 해시
Get-FileHash unim-*-x64.msi -Algorithm SHA256      # 실제 해시
msiexec /i unim-<버전>-x64.msi
```

### 설치 후

로그아웃 후 다시 로그인하면 첫 세션에서 설정 마법사가 떠서 기본 입력기 지정까지 안내한다.

GNOME에서 UNIM 확장을 쓸 거라면 IBus를 먼저 비활성화하거나 지워야 한다. 둘이 같이 있으면
키 이벤트가 유실된다. 그 밖의 환경변수 설정(`im-config -n unim`, `GTK_IM_MODULE` 등),
Flatpak/Snap 앱 대응은 [사용자 매뉴얼](docs/user/user-guide/README-ko.md)과
[트러블슈팅 §12–13](docs/user/troubleshooting/README-ko.md)에 있다.

## 자동 오타 교정

UNIM에서 손이 제일 많이 가는 기능이라 따로 적는다.

### 어떻게 동작하나

두 방향이 있고, 각각 독립적으로 켜고 끌 수 있다.

- **순방향 (영→한)**: 영문 모드에서 한글 자판대로 친 걸 한글로 되돌린다. `dkssud` → `안녕`
- **역방향 (한→영)**: 한글 모드에서 영어를 치려다 나온 자모를 영어로 되돌린다. `ㅈㅐㅍㅁ` → `wave`

교정은 단어 경계(스페이스·구두점)에서 판정한다. 판정에 쓰는 값은 전부 설정으로 조절할 수 있다 —
변환 결과의 한글 음절 수 임계값, 영어 단어 최소 길이, 각 방향의 시간 창(ms),
이미 완성된 음절은 건너뛰기, 사전에 있는 영어 단어는 건드리지 않기.

비밀번호 필드에서는 판정 자체를 하지 않는다.

### 켜고 끄기

전체 토글의 기본 단축키는 **`Shift+F8`**이다. 순방향·역방향 전용 토글은 비어 있어서
필요한 사람만 지정한다.

```bash
# 리눅스 — CLI
unim-cli config set auto-typefix-toggle-keys "Shift+F8"
unim-cli config set auto-typefix-forward-toggle-keys F10
unim-cli config set auto-typefix-reverse-toggle-keys F11

# 여러 키 (그중 아무거나)
unim-cli config set auto-typefix-toggle-keys "Shift+F8,Ctrl+Left"

# 해제 — 빈 값이면 어떤 키도 가로채지 않는다
unim-cli config set auto-typefix-toggle-keys ""
```

Windows에서는 설정 창 → 「오타 교정」 → 「토글 단축키」 그룹의 세 칸에 적는다.
여러 키는 쉼표로 구분하고, 안 쓸 칸은 비운다. 리눅스 설정 GUI도 같은 자리에 있다.

### 오교정이 났을 때

자동 교정은 추측이라 틀린다. 틀렸을 때 손이 덜 가는 순서로 적는다.

**1. 그냥 되돌린다 — 그게 학습이다**

교정 결과가 마음에 안 들면 `BackSpace`로 지우고 모드를 전환한다. 평소에 하던 그대로다.
UNIM은 이 패턴을 롤백으로 관측해 그 단어를 「의심 단어」로 표시해 둔다.
같은 단어가 다시 교정 대상이 되면 **그 시도를 억제하면서** 승인 대기(Tentative)로 등록한다.
즉, 두 번째부터는 안 건드린다.

Tentative는 기본 **4시간** 뒤 만료되고(설정으로 1~12시간), 만료되면 기록은 남되 억제는 풀린다.

**2. 영구 억제로 확정한다**

설정 창의 「억제 단어」 페이지에서 단어를 고르고 [선택 영구 활성]을 누르면 Confirmed가 되어
계속 억제된다. 목록은 `~/.config/unim/typefix-blacklist.yaml`(Windows는 `%APPDATA%\unim\`)에
있고, 파일을 직접 고치면 데몬이 알아서 다시 읽는다.

억제 키는 `(ascii, 방향, 한글자판, 영문자판)` 조합이다. 같은 키 시퀀스라도 자판이 다르면
다른 단어로 해석되기 때문에 자판별로 따로 저장한다.

**3. 반대로, 특정 단어는 꼭 교정되게 한다**

역방향 교정은 내장 영어 사전을 기준으로 판정하기 때문에, 사전에 없거나 최소 길이보다 짧은
단어는 그냥 지나친다. `git`, `ls`, `cargo`, `kubectl` 같은 명령어가 대표적이다.
설정 창의 「사용자 사전」 페이지에서 그런 단어를 등록하면 역방향 판정에서 우선 매칭된다.
저장은 `~/.config/unim/typefix-userdict.yaml`.

**4. 판정 기준을 조인다**

특정 방향이 통째로 과하게 튀면 임계값을 올린다.

```bash
# 순방향 — 변환 결과가 N음절 미만이면 교정하지 않는다 (2~6, 기본 2)
unim-cli config set auto-typefix-kor-threshold 3

# 역방향 — 이 길이보다 짧은 영단어는 교정하지 않는다 (3~8)
unim-cli config set auto-typefix-eng-min-length 5

# 내장 사전에 있는 영어 단어는 건드리지 않는다
unim-cli config set auto-typefix-skip-english-word true
```

**5. 상황별로 끈다**

코드를 칠 때만 거슬린다면 `Shift+F8`로 그 순간만 끄는 편이 낫다.
한 방향만 문제면 그 방향 전용 토글을 지정해 두면 된다.

전체 목록과 설정 창 화면 설명은 [사용자 매뉴얼 §4.4·§5.3·§5.4](docs/user/user-guide/README-ko.md)에 있다.

## 지원 환경

**실사용으로 검증한 환경**: GNOME Shell 45–49 (X11 / Wayland), X11 데스크톱 전반(KDE Plasma 5.x,
XFCE, MATE, Cinnamon, LXDE), Windows 10/11 (TSF).

| 환경 | 자동 시작 | 팝업 렌더러 | 설정 |
|------|-----------|-------------|------|
| GNOME Wayland | GNOME 확장 | 확장 내장 (St 위젯) | unim-settings |
| GNOME X11 | GNOME 확장 | unim-popup-service (GTK4) | unim-settings |
| KDE (X11) · Xfce · MATE · Cinnamon · LXDE | unim-indicator | unim-popup-service (GTK4) | unim-settings |
| KDE 6 Wayland · Sway · Hyprland — 실측 안 됨 | unim-indicator | unim-popup-service (wayland-backend) | unim-settings |
| Windows 10/11 | unim-popup-win.exe (로그인 시) | unim-popup-win.exe | unim-settings |

**KDE Plasma 5.x Wayland는 안 된다.** Wayland에서 팝업 위치를 잡는 데 `gtk4-layer-shell`이
필요한데 Ubuntu 24.04 저장소에 그 패키지가 없다. X11 세션을 쓰거나 GNOME으로 우회해야 한다.

**KDE 6 Wayland·Sway·Hyprland 등 단독 컴포지터**는 코드는 있지만 실기기에서 확인하지 못했다.
`libgtk4-layer-shell`이 설치된 상태에서 `wayland-backend` feature를 켜고 빌드하면 동작해야
하지만, 팝업 위치·IME 포커스 전환·좌표 변환은 컴포지터마다 다르게 굴러서 그대로 믿을 게 못 된다.
지금 이 프로젝트에서 "된다고 말하기 어려운" 구간은 여기다.
써 보고 결과를 [Issues](https://github.com/from104/unim/issues)로 알려주면 도움이 된다.

**비밀번호 필드 감지도 제보를 받는다.** 비밀번호 칸에서 자동 오타 교정을 끄는 건 앱이
"이 칸은 비밀번호"라고 알려 줄 때만 동작한다. 어떤 앱이 제대로 알려 주고 어떤 앱이 안 알려
주는지는 실사용 사례가 쌓여야 알 수 있는데, 리눅스·Windows 양쪽 다 사례가 충분치 않아
앱별 대응이 다 들어가 있지 못하다. 비밀번호 칸에서 교정이 발동하는 앱을 만나면 앱 이름과
버전을 [Issues](https://github.com/from104/unim/issues)로 알려주기 바란다.

## 로드맵

전체는 [ROADMAP.md](ROADMAP.md)에 있다. 요약하면:

**완료** — Rust 코어(2벌식·3벌식 계열), 3계층 아키텍처(Core → D-Bus → Frontend),
전 프론트엔드(GTK3/4, Qt5/6, XIM, Wayland, GNOME Shell), 배포 채널(deb·rpm·MSI·설치 스크립트),
자판 프로필 v1, 자동 오타 교정 + 억제 사전.

**진행 중** — Windows 쪽 완성도 다듬기, 단독 Wayland 컴포지터 실측,
Wayland surrounding-text / content-type 활용, 문서 정비.

**다음** — 문맥을 보고 한/영을 알아서 전환하는 것. 이게 프로젝트의 원래 목표다.
그리고 엔진 재설계: 낱자 provenance 태깅, 문맥 의존 키 해석, 모아치기 stroke replay,
복벌식, 옛한글.

**구상 단계** — 한자 단어 단위 변환, macOS(InputMethodKit), 모바일, 음성 입력,
경량 LLM 기반 단어·문장 예측, 한손 자판(천지인·나랏글).

## 문서

- [사용자 매뉴얼](docs/user/user-guide/README-ko.md) · [User Manual](docs/user/user-guide/README.md)
- [트러블슈팅](docs/user/troubleshooting/README-ko.md) · [Troubleshooting](docs/user/troubleshooting/README.md)
- [FAQ](docs/user/faq/README-ko.md) · [FAQ (EN)](docs/user/faq/README.md)
- [단축키 정리](docs/user/keyboard-shortcuts/README-ko.md) · [Shortcuts](docs/user/keyboard-shortcuts/README.md)
- [0.4.0 릴리스 노트](docs/user/release-notes/0.4.0/README.md) · [Release Notes](docs/user/release-notes/0.4.0/README.en.md)
- [변경 이력](CHANGELOG-ko.md) · [Changelog](CHANGELOG.md)

같은 내용이 오프라인 도움말로도 설치된다. 리눅스·Windows 판이 각각 따로 생성되므로,
자기 플랫폼에 해당하는 내용만 보인다.

## 개발

구조는 3계층이다. 코어(`src/`)는 순수 Rust 라이브러리고, `unim-daemon`이 D-Bus 세션 버스에
`org.atit.unim.InputMethod`를 등록해 상태를 관리하며, 각 프론트엔드가 클라이언트로 붙는다.
창마다 독립된 입력 컨텍스트를 받아 서로 간섭하지 않는다. Windows는 데몬 없이 TSF DLL이
코어를 in-process로 링크한다.

컴포넌트별 명세는 코드 옆 `SPEC.md`에 있다.

| 계층 | 명세 |
|------|------|
| Core | [`src/`](src/SPEC.md) · [`unim-capi/`](unim-capi/SPEC.md) · [`unim-cli/`](unim-cli/SPEC.md) |
| D-Bus | [`unim-daemon/`](unim-daemon/SPEC.md) · [`unim-dbus/`](unim-dbus/SPEC.md) |
| Frontend | [gtk3](unim-frontends/gtk3/SPEC.md) · [gtk4](unim-frontends/gtk4/SPEC.md) · [qt5](unim-frontends/qt5/SPEC.md) · [qt6](unim-frontends/qt6/SPEC.md) · [xim](unim-frontends/xim/SPEC.md) · [wayland](unim-frontends/wayland/SPEC.md) · [gnome](unim-gnome-extension/SPEC.md) |
| 공용 | [팝업](docs/dev/specs/POPUP_SPEC.md) · [IME 동작](docs/dev/architecture/IME_BEHAVIOR.md) |

- 문서 색인: [`docs/README.md`](docs/README.md)
- 개발 규약·로깅·설정 동기화: [`GEMINI.md`](docs/dev/architecture/GEMINI.md)
- 기여: [`CONTRIBUTING.md`](CONTRIBUTING.md) · [`AGENTS.md`](docs/dev/architecture/AGENTS.md)
- 예제: [`examples/README.md`](examples/README.md) — `cargo run --example string_processing`

## 라이선스

MIT. 전문은 [LICENSE](LICENSE).

함께 배포하는 외부 데이터는 각 출처의 라이선스를 따른다. 자세한 건 [NOTICE](NOTICE)와
[`LICENSES/`](LICENSES/)에 있다.

- 한자 사전 (`src/data/hanja.txt`) — [libhangul](https://github.com/libhangul/libhangul), BSD 3-Clause
- 이모지 데이터 (`src/emoji/data.rs`) — [Unicode CLDR](https://cldr.unicode.org/) `emoji-test.txt` (15.0), Unicode License v3
- 자판 표준 (`docs/references/keymaps/*.json`) — KS X 5002, 세벌식 390/391 등 공개 표준
- Rust 의존성 — 전부 MIT / Apache-2.0 / BSD / Unicode, MIT와 호환
- 시스템 라이브러리 — GTK3/4, Qt5/6, libwayland, libX11, libxkbcommon, glib 등은 동적 링크
