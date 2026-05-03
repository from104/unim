# UNIM 사용자 매뉴얼 (한국어)

> UNIM 0.2.0 — Universal Next-generation Input Method
> 한국어와 영어를 자유롭게 오가며 타이핑하기 위한 Rust 기반 입력기.
> 이 문서는 처음 쓰는 사람도 5분 안에 한글 한 글자를 칠 수 있게 만드는 것이 목표다.

---

## 1. UNIM이 무엇인가 (30초 설명)

- **IME** (Input Method Editor, 입력기): 키보드의 알파벳 키 두드림을 한글 음절로 바꿔서 앱 화면에 보여주는 시스템 컴포넌트. 다른 IME 예시: ibus-hangul, fcitx-hangul, kime, nimf.
- **UNIM의 차별점**: Rust로 작성된 단일 코어를 GTK·Qt·XIM·Wayland·GNOME Shell 다섯 환경 모두에 그대로 꽂는다. 즉 어떤 앱(터미널, 브라우저, 메모장, IDE)에서 한글을 쳐도 같은 조합 규칙·같은 한자 팝업·같은 자동 오타 교정이 동작한다.
- **30초 요약**: "한/영 키로 모드 전환 → 한글 입력 → 한자키로 한자 변환 → ㄱㄴㄷ로 특수문자 변환 → 영타가 한글로 잘못 나가도 자동 복구". 이게 UNIM이 사용자에게 제공하는 가치 전부다.

> 약어 풀이: **IM 모듈** (Input Method Module) — 앱이 키 입력을 IME에 위임할 수 있게 해 주는 툴킷별 어댑터(GTK용, Qt용 등). **DBus** (Desktop Bus) — 리눅스 데스크톱의 프로세스 간 통신 버스. UNIM은 DBus 위에서 데몬↔프론트엔드 통신을 한다. **XIM** (X Input Method) — X11 시절부터 쓰인 가장 오래된 IME 프로토콜. **Wayland** — 차세대 디스플레이 프로토콜로, X11과 IM 처리 방식이 다르다.

---

## 2. 빠른 시작 (5분)

### 2.1 설치

#### Debian/Ubuntu

```bash
# 패키지 설치 (UNIM 0.2.0 .deb 패키지가 이미 빌드돼 있다고 가정)
sudo apt install ./unim_0.2.0_amd64.deb \
                 ./unim-common_0.2.0_amd64.deb \
                 ./unim-im-gtk_0.2.0_amd64.deb \
                 ./unim-im-qt_0.2.0_amd64.deb \
                 ./unim-gui-gtk_0.2.0_amd64.deb

# IBus 제거 (GNOME 환경에서 충돌 방지)
sudo apt remove ibus

# 시스템 입력기로 UNIM 등록 (Debian/Ubuntu 표준 도구)
im-config -n unim
```

설치 직후 한 번 로그아웃하고 다시 로그인하면 환경변수가 새 셸에 적용된다.

#### 소스 빌드 (Arch/Fedora/그 외)

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

### 2.4 첫 한글 입력 (60초)

1. 텍스트 에디터(예: GNOME Text Editor, Kate)를 연다.
2. **한/영 키**(또는 `Shift+Space`, 키보드별로 다름)를 누른다 — 트레이 아이콘이 「한」으로 바뀐다.
3. `dkssud` 입력 → 화면에 `안녕`이 뜬다.
4. 한자 키(또는 F9)를 누르면 `安寧` 후보가 팝업으로 뜬다. 숫자 1~9로 선택.
5. 한 번 더 한/영 키 → 영문 모드.

여기까지 동작하면 설치는 끝났다. 동작이 안 되면 [트러블슈팅](../troubleshooting/README-ko.md)으로.

---

## 3. 환경별 설정 가이드

| 환경 | 설치 방법 | IM 모듈 | 팝업 주체 | 주의점 |
|------|----------|---------|----------|--------|
| **X11 + GTK 앱** | `GTK_IM_MODULE=unim` | gtk3/gtk4 IM 모듈 | IM 모듈 자체(Embedded) 또는 unim-gui-gtk(Standalone) | `popup_mode` 설정으로 선택 |
| **X11 + Qt 앱** | `QT_IM_MODULE=unim` | qt5/qt6 IM 플러그인 | 동상 | Plasma는 Qt 모드로 통일 권장 |
| **X11 + 레거시 (Emacs, xterm)** | `XMODIFIERS=@im=unim` | xim 프론트엔드 | XIM 자체 Xft 팝업 | over-the-spot 모드 |
| **GNOME + Wayland** | GNOME Extension 활성화 | (앱은 텍스트-인풋-v3 직접) | GNOME Extension | IBus 제거 필수 |
| **KDE + Wayland** | `QT_IM_MODULE=unim` + Wayland 프론트엔드 | wayland | unim-gui-gtk Standalone | input-method-v2 사용 |
| **Sway/Hyprland (Wayland)** | 환경변수 + Wayland 프론트엔드 | wayland | unim-gui-gtk Standalone | 컴포지터의 input-method-v2 지원 필요 |

> 환경 판별: `echo $XDG_SESSION_TYPE`(x11/wayland), `echo $XDG_CURRENT_DESKTOP`(GNOME/KDE/sway).

### 3.1 Flatpak/Snap 앱에서 한글이 안 나올 때

GNOME+Wayland에서 Telegram, VS Code 같은 Flatpak/Snap 앱은 샌드박스 안에 UNIM IM 모듈이 없다. 호스트의 `GTK_IM_MODULE=unim`이 오히려 입력을 막는다.

**자동 처리**: `unim-daemon`이 GNOME+Wayland를 감지하면 시작 시 자동으로 Flatpak 전역 override를 설정해 IM 환경변수를 비운다. Flatpak 앱은 Wayland text-input-v3 → GNOME Extension 경로로 동작한다.

**수동 설정**(자동 설정이 동작하지 않을 때):

```bash
flatpak override --user --env=QT_IM_MODULE= --env=GTK_IM_MODULE=
```

Snap 앱에는 전역 override 메커니즘이 없으니 `~/.profile`에 조건부로 환경변수를 비우는 스크립트를 추가한다 — [README §1.7](../../../README.md) 참고.

---

## 4. 일상 사용

### 4.1 한/영 모드 전환

| 키 | 동작 | 비고 |
|----|------|------|
| 한/영 키 (Hangul) | 모드 토글 | 키보드에 따라 키 코드가 다름 |
| `Shift+Space` | 모드 토글 (대체) | 모든 키보드에서 동작 |
| 트레이 아이콘 클릭 | 모드 토글 (마우스) | unim-gui-gtk가 트레이에 |

> **모드 공유 방식**(`mode_share_mode` 설정): 「창별 독립」/「전역 공유」 중 선택. 창별 독립이 기본 — 터미널은 영어, 텍스트 에디터는 한글로 따로 유지된다. 한 모드를 모든 창에 동기화하고 싶으면 「전역 공유」로 바꾼다.

### 4.2 한자 변환 (Hanja)

1. 한자로 바꿀 한글을 입력 (예: `한국`).
2. **한자 키**(또는 F9)를 누른다.
3. 9칸 그리드 팝업이 뜬다 — `韓國`, `漢國` 등.
4. 숫자 1~9로 직접 선택, 화살표로 이동, Enter로 확정, ESC로 취소.
5. 후보가 9개 이상이면 **마침표(.)** 키로 9×9=81칸 확장 그리드로 토글. 화면 우상단에 ⊞/⊟ 아이콘으로 현재 모드 표시.

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

#### 즐겨찾기 ☆/★

자주 쓰는 한자를 즐겨찾기에 등록할 수 있다. 후보에 포커스를 둔 상태에서 **Space** 키 → ☆(미등록) ↔ ★(등록) 토글. 등록된 한자는 `HanjaCandidatesReordered` DBus 시그널로 모든 열린 팝업이 즉시 갱신된다.

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

**저장 위치**: `~/.config/unim/typefix-blacklist.yaml`. 데몬은 mtime 감시로 자동 리로드한다.

> **사용자 사전 (역방향 화이트리스트)**: 0.2.0 신규. 텍스트 선택 후 단축키로 `RegisterUserDictFromSelection` DBus 메서드 호출 → 영어 사전 항목으로 등록. 설정 GUI 「사용자 사전」 페이지에서 추가/제거/수정.

### 4.5 자동 영문 모드 전환 (Auto-English-Mode)

vim 명령 모드(`Esc`), CLI 슬래시 명령(`/`) 같은 비한글 컨텍스트에 들어갈 때 자동으로 영문으로 전환되게 하는 **opt-in** 기능. 기본 비활성.

- 활성화: 설정 GUI → 「일반」 → 「자동 영문 전환」 그룹 → `자동 영문 전환 사용` ON.
- 트리거 키: 기본은 `Escape`, `Slash`. 필요 시 `ShiftSemicolon`(:), `ShiftSlash`(?) 같은 가상 이름을 추가.
- 동작: 한글 모드에서 트리거 키를 누르면 (1) 조합 commit → (2) 영문 모드로 영구 전환 → (3) 트리거 키 자체는 그대로 앱에 전달.

> 한/영 토글 키와 트리거 키가 겹치면 토글 분기가 우선이라 자동 전환이 발동하지 않는다. 비밀번호 필드는 이미 강제 영문이라 영향 없음.

---

## 5. 설정 GUI 투어

`unim-gui-gtk`(GTK4 + libadwaita 다이얼로그) 또는 `unim-gui-qt`(Qt6 cxx-qt 다이얼로그) 중 하나를 띄워 설정을 만진다. 기본은 GTK 버전.

```bash
unim-gtk-settings &     # GTK4/libadwaita
unim-qt-settings &      # Qt6 (대안)
```

페이지 구성 (GTK 기준 5장):

### 5.1 페이지 1 — 일반

<!-- screenshot: settings-general -->

| 그룹 | 위젯 | 권장 값 |
|------|------|---------|
| **자판 및 키맵** | 한국어 자판 (ComboRow) | `ko_2bulstd`(두벌식 표준) — 가장 익숙 |
|  | 영어 자판 (ComboRow) | `qwerty` |
|  | 이모지 입력 사용 (Switch) | ON — 「:smile:」 같은 단축어 입력 |
| **한국어 자판 옵션** | 동적 SwitchRow (자판마다 다름) | 자판 변경 시 옵션도 재구성. 예: `ko_3bul390` 선택 시 `sun_arae_batchim`(순아래받침) 토글 표시 |
| **입력 모드** | 초기 입력 모드 (ComboRow) | `한글`/`영문` — 데몬 시작 시 어느 쪽? |
|  | 모드 공유 방식 (ComboRow) | `창별 독립`(권장) / `전역 공유` |
|  | 팝업 모드 (ComboRow) | `Standalone`(기본, 모든 환경) / `Embedded`(X11 한정, IM 모듈이 직접 그림) |
| **자동 영문 전환** | 자동 영문 전환 사용 (Switch) | OFF(기본). vim 사용자라면 ON 추천 |

### 5.2 페이지 2 — 오타 교정

<!-- screenshot: settings-typefix -->

| 그룹 | 위젯 | 의미 |
|------|------|------|
| **공통** | 활성화 (Switch) | 마스터 토글. OFF면 forward/reverse 둘 다 정지 |
|  | 롤백 감지 (Switch) | BS+모드전환 관측 자동 학습. 기본 ON |
|  | 관찰 시간(초) (Slider) | 기본 10초, 5~15. 길수록 학습 민감 |
|  | 임시 만료(시간) (Slider) | 기본 1시간, 1~12. Tentative → Inactive 전환 시간 |
| **순방향(영→한)** | 사용 (Switch) | `gksrmf`→`한글` |
|  | 영문 모드 시 무시 (Switch) | ON 권장. 영문 모드일 땐 교정 안 함 |
| **역방향(한→영)** | 사용 (Switch) | `ㅈㅐㅍㅁ`→`wave` |
|  | 음절 미완 시 무시 (Switch) | ON 권장. 조합 중인 글자는 건드리지 않음 |
|  | 사용자 사전만 사용 (Switch) | OFF면 자동 매핑까지 사용. ON이면 등록한 단어만 |

### 5.3 페이지 3 — 교정 억제 단어

<!-- screenshot: settings-blacklist -->

3섹션: **승인 대기 (Tentative)** / **확정 (Confirmed)** / **비활성 (Inactive)**. 각 행에 [확정]/[비활성화]/[삭제]/[재활성화] 버튼.

> 데몬이 파일을 갱신해도 GUI가 2초 주기 mtime 폴링으로 즉시 반영한다. 따로 리로드 안 해도 된다.

### 5.4 페이지 4 — 사용자 사전 (역방향 화이트리스트)

<!-- screenshot: settings-userdict -->

영어 단어 ↔ 한글 시퀀스 매핑을 직접 등록. 예: `wave` ↔ `ㅈㅐㅍㅁ`. reverse 교정에서 우선 매칭.

### 5.5 페이지 5 — GNOME Shell

<!-- screenshot: settings-gnome -->

GNOME 세션에서만 표시되는 페이지. 확장의 인디케이터·키 가로채기 옵션.

---

## 6. 키 매핑 치트시트

| 상황 | 키 | 결과 |
|------|----|------|
| 어디서나 | 한/영 (또는 Shift+Space) | 모드 토글 |
| 한글 모드, 글자 입력 후 | 한자 (F9) | 한자 팝업 |
| 한자 팝업 | 숫자 1~9 | 직접 선택 |
| 한자 팝업 | 화살표 | 후보 이동 |
| 한자 팝업 | `←`/`→` 또는 PageUp/PageDown | 페이지 이동 (wrap-around) |
| 한자 팝업 | 마우스 ◀ / ▶ | 페이지 이동 (wrap-around, 단일 페이지면 숨김) |
| 한자 팝업 | Enter | 포커스 항목 확정 |
| 한자 팝업 | ESC | 취소 |
| 한자 팝업 | `.` (마침표) | 9칸 ↔ 81칸 토글 |
| 한자 팝업 | Space | 즐겨찾기 ☆/★ 토글 (해제 시 cursor 셀 140ms flash) |
| 한글 모드, 자음만 입력 | 한자 (F9) | 특수문자 팝업 |
| 한글 입력 중 | BackSpace | 마지막 자모 1개 삭제 |
| forward 교정 후 후회 | BS + 한/영 | Tentative 학습 트리거 |
| 자동 영문 모드 활성 시 | `Esc` 또는 `/` | 영문 강제 전환 + 키 전달 |

---

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
# 모든 설정 키 보기
unim-cli config list

# 특정 키 값 보기
unim-cli config get auto_typefix.enabled

# 값 설정
unim-cli config set auto_typefix.tentative_expiry_hours 6
unim-cli config set engine.auto_english.enabled true

# 자판 프로필 관리
unim-cli config layout list                    # 내장 + 사용자 프로필 목록
unim-cli config layout describe ko_3bul390     # 프로필 상세
unim-cli config layout validate my.json        # 사용자 정의 자판 검증
```

> 설정 변경은 데몬에 즉시 반영된다. config.yaml ↔ unim-cli ↔ GTK GUI 3지점이 항상 싱크되도록 설계됐다.

---

## 8. 설정 파일 위치 / 백업

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

---

## 9. 다음 단계

- 동작이 이상하다 → [트러블슈팅](../troubleshooting/README-ko.md)
- 다른 IME와 비교 / 안정성 / 마이그레이션 → [FAQ](../faq/README-ko.md)
- 0.2.0의 변경 내역과 마이그레이션 → [릴리즈 노트](../release-notes/0.2.0/RELEASE_NOTES-ko.md)
- 기여하고 싶다 → [`CONTRIBUTING.md`](../../../CONTRIBUTING.md)
- 핵심 동작 명세 → [`IME_BEHAVIOR.md`](../../dev/architecture/IME_BEHAVIOR.md), [`docs/dev/specs/POPUP_SPEC.md`](../../dev/specs/POPUP_SPEC.md)

---

문서 버전: 0.2.0 / 작성일: 2026-04-26 / 라이선스: 본문 라이선스는 프로젝트와 동일.
