# UNIM 0.4.0 릴리즈 노트 (한국어)

**릴리즈 날짜**: 2026-08-09
**브랜치**: develop → main

> 한 줄 요약: 원클릭 설치(`curl … | bash`), 첫 실행 설정 마법사, Slint 기반 크로스플랫폼 설정 앱, 자판 스튜디오·타자 연습 신규 도구, 단어 단위 입력, 그리고 대규모 Windows(TSF) 포팅.

> ⚠️ 2026-07-19에 한 번 태깅됐던 이전 v0.4.0 태그·릴리스는 회수되었습니다. 이 릴리즈 노트가 유효한 v0.4.0입니다.

---

## 패키지 구성 변경 안내

- **설정 앱 이원화**: 신규 **`unim-settings`**(Slint 기반, 리눅스·Windows 공용)가 정식 설정 앱 자리를 넘겨받았습니다. 기존 GTK4 설정 창은 `unim-settings-gtk` 바이너리로 이름이 바뀌어 `unim-desktop` 패키지 안에 당분간 함께 배포되지만, 데스크톱 메뉴에는 노출되지 않습니다(추후 퇴역 예정).
- **신규 패키지**: `unim-keymap-studio`(자판 스튜디오), `unim-typing-practice`(타자 연습).
- **`unim-desktop`**: 트레이 인디케이터·팝업 서비스·레거시 GTK 설정 창을 하나로 묶었습니다.
- 총 **11개 deb 패키지**로 재편되었습니다: `unim-common`, `unim-im-gtk`, `unim-im-qt`, `unim-xim`, `unim-wayland`, `unim-desktop`, `unim-settings`, `unim-keymap-studio`, `unim-typing-practice`, `unim-gnome`, `unim`(메타).
- `apt`/`dnf`로 기존 설치를 갱신하면 자동으로 전환됩니다. **설정 파일(`~/.config/unim/`)은 그대로 유지**됩니다.

---

## 신규 기능 (Added)

### 1. 원클릭 설치 (`curl … | bash`)

```bash
curl -fsSL https://raw.githubusercontent.com/from104/unim/main/install.sh | bash
```

리눅스·amd64·apt 환경을 확인한 뒤 최신 GitHub 릴리스에서 deb 11종을 내려받아 `SHA256SUMS`로 무결성을 검증하고 `apt-get`으로 설치합니다. 환경 변수(`UNIM_VERSION`, `UNIM_BASE_URL`)로 특정 버전 고정·미러 지정도 가능합니다.

### 2. 첫 실행 설정 마법사

설치 후 처음 로그인하면 마법사가 자동 실행되어 UNIM을 기본 입력기로 설정합니다(im-config, 실패 시 xinputrc 폴백). 완료 전까지는 다시 뜨며(재로그인마다), 한 번 완료하면 다시 나타나지 않습니다.

> GNOME Wayland 세션에서는 마법사 완료 후 확장을 한 번 더 활성화해야 합니다 — 재로그인 후 `gnome-extensions enable unim-gnome@from104.github.io` 를 한 번 실행하세요.

### 3. Slint 설정 앱 (`unim-settings`)

리눅스와 Windows가 동일한 설정 화면을 공유하는 크로스플랫폼 앱으로 새로 작성되었습니다. 저장 즉시 `config.yaml`에 반영되고 데몬에 DBus로 통지됩니다.

### 4. 한/영 전환 소리(비프)

모드가 바뀔 때 짧은 소리로 알려줍니다(한글 880Hz, 영문 440Hz). 기본은 꺼짐이며 설정에서 켤 수 있습니다.

### 5. 조합키 자동 반복 무시 (접근성)

키를 오래 눌러 생기는 자동 반복(연타)을 데몬이 무시하도록 켤 수 있습니다. 손 떨림 등으로 키를 오래 누르게 되는 지체장애 사용자를 위한 기능이며, **Windows·리눅스 양쪽에서 동일하게 집행**됩니다. 기본값은 꺼짐입니다.

### 6. 자판 스튜디오 · 타자 연습 (신규 GTK4 도구)

- **자판 스튜디오(`unim-keymap-studio`)**: 빌트인·사용자 자판의 키 배치와 초·중·종성 조합을 보고 편집합니다.
- **타자 연습(`unim-typing-practice`)**: 예문 연습 중 실시간 WPM·CPM·정확도·오타율을 보여주고, 끝나면 키별 오타 히트맵을 표시합니다.

### 7. 자동 영문 전환 — Ctrl/Alt/Super 조합 트리거

`key:Ctrl+B`, `key:Super+Space` 같은 수정자 조합을 자동 영문 트리거로 쓸 수 있습니다(tmux/wmux 등). GTK·Qt·XIM·GNOME에서 동작하며, Windows는 후속 작업입니다.

### 8. AutoTypeFix 토글 단축키 3종

자동 오타 교정을 전체/순방향(영→한)/역방향(한→영) 별로 단축키를 지정해 켜고 끌 수 있습니다. **전체 토글 기본값은 `Shift+F8`**입니다. 리눅스·Windows 모두 동일하게 동작합니다.

### 9. 비밀번호 필드 자동 보호

비밀번호 입력란에서 자동으로 영문 모드로 전환하고, 입력된 키는 버퍼·학습 사전 등 어디에도 남기지 않습니다. Wayland·GTK·Qt·Windows(TSF)에서 동작하며, Windows IMM32는 최선노력, XIM은 미지원입니다.

### 10. 단어 단위 입력 (`commit_unit`)

조합 확정 단위를 음절 대신 단어로 둘 수 있습니다. 터미널·XIM/ibus 계열·모아치기 자판에서는 오작동 방지를 위해 자동으로 음절 단위로 강등됩니다.

### 11. 4개 앱 고유 아이콘 + 역DNS 명명

인디케이터·설정·자판 스튜디오·타자 연습 앱이 각각 고유 아이콘을 갖고, GNOME Wayland 작업표시줄·오버뷰에도 올바르게 표시됩니다.

---

## Windows 지원

이번 개발 주기에 Windows 포팅이 크게 진행되었습니다. 개발 환경에서 상시 사용하며 다듬어 왔으나, 리눅스만큼 다양한 머신·앱 조합을 거치지는 못했습니다.

- **TSF 완전 네이티브 아키텍처**: 조합·팝업(한자·특수문자·이모지)·AutoTypeFix·설정·언어바가 단일 `unim_tsf.dll`에 통합.
- **콘솔·IMM32 앱 한글 조합 복구**: WezTerm·텔레그램 등에서 CUAS 계약을 따라 정상 조합.
- **32비트 앱 지원**: `unim_tsf32.dll`로 카카오톡·한컴 등 32비트 앱 지원. Win11의 무의미한 IMM32 `.ime` 등록은 폐기.
- **접근성**: TSF UIA/UILess 노출, 조합키 자동반복 억제, 화면낭독기(NVDA/Narrator) 한/영 전환 통지.
- 자세한 사용법은 [UNIM (Windows) 사용 안내](../../UNIM-Windows-사용안내.md) 참고.

---

## 버그 수정 (Fixed)

**글자가 눈앞에서 어긋나던 것들** — 조합 중에 겪던 증상 위주로 먼저 적습니다.

- **조합 중 다른 곳을 클릭하면 글자가 클릭한 자리에 확정되던 문제**: Chrome·Obsidian 등에서 한글을 조합하다 같은 입력란의 다른 곳을 클릭하면, 조합 중이던 글자가 원래 자리가 아니라 클릭한 자리에 들어갔습니다. GNOME Wayland·XIM·Qt 경로를 모두 고쳤습니다.
- **XIM 앱에서 확정 직후 다음 글자가 늦게 보이던 문제**: Obsidian 등에서 한 음절이 확정되면 이어서 누른 자모가 바로 안 나타나고 자모를 하나 더 눌러야 보였습니다. 0.3.0부터 남아 있던 문제입니다.
- **XIM 앱에서 조합 중 Enter 를 누르면 줄바꿈이 글자보다 앞에 가던 문제**: 글자가 확정되고 줄이 바뀌는 대신, 줄이 먼저 바뀌고 글자가 그 아래에 놓였습니다. 다만 이때 다시 전달되는 Enter 는 수정자를 재현하지 않으므로 `Shift+Enter` 는 그냥 Enter 로 들어갑니다.
- **GNOME Wayland에서 비밀번호 필드 보호가 실제로는 아무 일도 하지 않던 문제**: GNOME 확장의 content-purpose 처리가 빈 껍데기여서, GTK3/4·Chrome이 모두 지나가는 그 경로에서 보호가 조용히 무력했습니다. 포커스를 유지한 채 필드 성격이 바뀌는 경우("비밀번호 보기" 토글 등)도 따라갑니다.

**그 밖에**

- 도움말이 브라우저 대신 다른 앱(VS Code 계열 IDE 등)으로 열리던 문제 수정.
- **오른쪽 Alt(RightAlt) 한/영 토글**이 모든 프런트엔드에서 동작하도록 수정(GTK3/4·Qt5/6·GNOME 확장이 자체적으로 걸러내던 문제 해소).
- GTK/Qt immodule의 Super/Meta 조합 키 마스크 정합.
- 잘못된 단축키 표기(`ScrollLock`, `Hangul` 등 인식 불가 예시)를 실제 동작하는 표기(`F10`, `Korean`, `Hanja` 등)로 교체하고, "수정자 조합은 지원하지 않는다"는 낡은 설명을 바로잡았습니다.
- 순수 Wayland(비-GNOME) 컴포지터의 키코드 off-by-8 오독 수정.
- 접근성 프리셋(한 손 사용/넉넉한 타이밍)의 조합키 자동반복 억제가 리눅스에서도 실제로 집행되도록 수정.
- 단어 단위 입력이 터미널 등에서 음절 단위로 자동 강등될 때, 설정이 고장 난 것으로 오해하지 않도록 로그(`[WordGate]`)와 설정 설명에 명시.

---

## 알려진 문제

- **Windows 판은 검증된 앱의 폭이 리눅스보다 좁습니다.** 드문 앱에서 문제를 겪으면 [트러블슈팅](../../troubleshooting/README-ko.md) 또는 [GitHub Issues](https://github.com/from104/unim/issues)에 제보해 주세요.
- **GNOME Shell 49 세션은 코드상으로만 지원 범위(45–49)에 추가**되었고, 아직 실기기(Fedora 43 등)로 세션 스모크 테스트를 하지 못했습니다.
- **데몬이 수동으로 재시작되거나 크래시하면** GTK/Qt/XIM/Wayland 프런트엔드가 자동으로 재연결하지 않아, 열려 있던 앱을 재시작해야 한글 입력이 복구될 수 있습니다. 일반적인 `apt`/`dnf` 업그레이드 경로는 이번 릴리스에서 데몬을 더 이상 중단시키지 않도록 완화했습니다.
- **Plasma 6(Qt6) Konsole에서 자동 오타 교정 시 텍스트가 중복될 수 있습니다.** 실기 검증 전이므로 문제가 보이면 자동 오타 교정을 끄고 사용하세요.
- **순수 Wayland(비-GNOME, Sway/Hyprland 등) 컴포지터**에서는 한자·특수문자·이모지 팝업이 실험적 지원(`wayland-backend` + `libgtk4-layer-shell`)이며 충분히 검증되지 않았습니다. KDE Plasma 5.x Wayland는 아예 미지원입니다(X11 세션 또는 GNOME으로 우회).
- **Ubuntu 22.04·Debian 12는 미지원**입니다(시스템 라이브러리가 오래되어 배포 패키지 사용 불가). 소스 빌드를 사용하세요.
- **Windows MSI는 리눅스 패키지보다 릴리스에 늦게 첨부될 수 있습니다**(별도 CI 워크플로, 최대 45분 소요). 그 사이 Windows 한 줄 설치(`irm | iex`)를 시도하면 `SHA256SUMS-msi` 미수신으로 실패할 수 있으니 잠시 후 다시 시도하세요.

---

## 더 읽을 거리

- [사용자 매뉴얼](../../user-guide/README-ko.md)
- [트러블슈팅](../../troubleshooting/README-ko.md)
- [FAQ](../../faq/README-ko.md)
- [UNIM (Windows) 사용 안내](../../UNIM-Windows-사용안내.md)
- [CHANGELOG](../../../../CHANGELOG-ko.md)
