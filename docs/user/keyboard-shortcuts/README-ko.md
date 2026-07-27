# 키보드 단축키 가이드

UNIM의 단축키는 환경(데스크톱/컴포지터)에 따라 캡처 주체가 달라집니다. 이 문서는 사용자가 환경별로 어떻게 단축키를 활성화해야 하는지 설명합니다.

> 영문 버전: [`README.md`](README.md)

---

## GNOME 전용 — 수동 변환 단축키 (기본 활성)

GNOME Shell(`unim-gnome-extension`)에서는 아래 세 단축키가 **별도 설정 없이 기본으로 켜져 있다.** 다른 데스크톱/컴포지터(KDE, Sway, Hyprland 등)에는 이 단축키가 없다 — GNOME 확장이 `Main.wm.addKeybinding`으로 Shell 자체에 등록하는 GNOME 전용 기능이기 때문이다.

| 단축키 | 동작 | 예시 |
|--------|------|------|
| `Super+K` | 포커스된 단어를 **영어 → 한글**로 변환 후 교체 | `gksrmf` → `한글` |
| `Shift+Super+K` | 포커스된 단어를 **한글 → 영어**로 변환 후 교체 | `ㅗ디ㅣㅐ` → `hello` |
| `Super+E` | 선택 영역을 읽어 역방향(한→영) AutoTypeFix 사용자 사전에 등록 | `ㅎㅑㅅ` 선택 → `git` 으로 등록 |

이 세 단축키는 [4.4 자동 오타 교정](../user-guide/README-ko.md#44-자동-오타-교정-autotypefix)의 자동 교정과는 별개로, **사용자가 직접 트리거하는 수동 변환**이다. `Super` 조합을 다른 용도로 쓰고 있거나 충돌이 난다면, 다음 명령으로 확장 설정을 열어 바꾸거나 끌 수 있다.

```bash
gnome-extensions prefs unim-gnome@from104.github.io
```

「변환 단축키」 그룹에서 `Super+K`/`Shift+Super+K`, 「사용자 사전 등록」 그룹에서 `Super+E` 를 각각 재지정하거나 빈 값으로 비워 끌 수 있다.

---

## 이모지 팝업 단축키 (`Super+.`)

UNIM은 마지막 활성 입력 위치에 이모지 팝업을 띄우는 기능을 제공합니다. 기본 단축키는 `Super+.`(Meta+`.`)입니다. 다만 **단축키를 누가 캡처하느냐**가 환경별로 다르기 때문에, 일부 환경에서는 사용자가 직접 등록해야 합니다.

### 환경별 동작 매트릭스

| 환경 | 자동 동작? | 등록 방법 |
|------|-----------|-----------|
| X11 / XIM | 자동 | unim 데몬이 X server로부터 redirect 받아 직접 매칭 |
| Wayland + GNOME | 자동 | UNIM GNOME extension이 `Main.wm.addKeybinding`으로 처리 |
| Wayland + KDE Plasma | 직접 등록 필요 | KCM Custom Shortcuts |
| Wayland + Hyprland | 직접 등록 필요 | `hyprland.conf` |
| Wayland + Sway | 직접 등록 필요 | `sway/config` |
| Wayland + Wayfire | 직접 등록 필요 | `wayfire.ini` |
| Wayland + 기타 컴포지터 | 직접 등록 필요 | 컴포지터별 단축키 도구 |

### 왜 직접 등록이 필요한가?

Wayland 컴포지터(KDE/Hyprland/Sway/Wayfire 등)는 `Super` 같은 modifier 조합 키를 자체 단축키 시스템에서 우선적으로 가로챕니다. 이 단계에서 컴포지터가 단축키를 소비해 버리면 입력기(IME)에는 키 이벤트 자체가 도달하지 않습니다.

GNOME에서는 UNIM의 GNOME 확장이 Shell 측 단축키 슬롯에 등록되어 자동 동작합니다. 그러나 다른 컴포지터에는 그런 확장이 없으므로, 컴포지터의 단축키 시스템에 **`unim-cli trigger emoji_popup` 명령을 단축키로 직접 등록**해야 합니다.

### 공통 명령

```bash
unim-cli trigger emoji_popup
```

내부적으로 이 명령은 데몬의 DBus 인터페이스 `org.atit.unim.InputMethod`의 `TriggerAction` RPC를 호출합니다. 데몬이 이 신호를 받으면, 마지막으로 활성화되어 있던 입력 컨텍스트(가장 최근에 사용한 텍스트 위젯) 위에 이모지 팝업을 띄웁니다.

향후 다른 액션(`hanja_popup` 등)도 같은 패턴으로 추가될 예정입니다.

---

## 환경별 등록 방법

### KDE Plasma 6 (Wayland)

1. **System Settings** → **Shortcuts** → **Custom Shortcuts** 열기
2. 좌하단 **Edit** → **New** → **Global Shortcut** → **Command/URL**
3. 이름: `Trigger UNIM emoji popup` (자유롭게 지정)
4. **Trigger** 탭 → 단축키 입력란에 `Meta+.` 입력
5. **Action** 탭 → **Command/URL**: `unim-cli trigger emoji_popup`
6. **Apply**

> Plasma 5 계열에서도 메뉴 경로만 다를 뿐 동일하게 동작합니다. (`System Settings → Shortcuts → Custom Shortcuts`)

### Hyprland

`~/.config/hypr/hyprland.conf` 에 다음 줄을 추가하세요.

```ini
bind = SUPER, period, exec, unim-cli trigger emoji_popup
```

설정 파일은 자동 reload되며, 적용을 강제하려면 `hyprctl reload`를 실행하세요.

### Sway

`~/.config/sway/config` 에 다음 줄을 추가하세요.

```
bindsym Mod4+period exec unim-cli trigger emoji_popup
```

`Mod4`는 Super(Windows) 키입니다. 적용은 `swaymsg reload`.

### Wayfire

`~/.config/wayfire.ini` 의 `[command]` 섹션에 두 줄을 추가하세요.

```ini
[command]
binding_emoji = <super> KEY_DOT
command_emoji = unim-cli trigger emoji_popup
```

설정 변경 시 자동 reload됩니다.

### GNOME (확장 없이 사용할 때 fallback)

UNIM GNOME 확장을 설치/활성화한 경우 별도 설정이 필요 없지만, 확장을 쓰지 않는다면 GNOME 자체 단축키 시스템으로 같은 효과를 낼 수 있습니다.

1. **Settings** → **Keyboard** → **View and Customize Shortcuts**
2. **Custom Shortcuts** → **Add Shortcut**
3. **Name**: `UNIM Emoji Popup`
4. **Command**: `unim-cli trigger emoji_popup`
5. **Set Shortcut**: `Super+.` 누르기 → **Add**

> GNOME에서는 일부 `Super` 조합이 시스템 예약입니다. 충돌이 나면 다른 키 조합(예: `Ctrl+Alt+E`)을 시도하세요.

### X11 + 임의 WM (xbindkeys)

X11/XIM 환경에서는 데몬이 자동 매칭하므로 **보통은 별도 등록이 필요 없습니다**. 하지만 데몬이 단축키를 받지 못하는 환경(특정 게임 모드, 일부 화면 전환 도구 등)에서는 `xbindkeys`로 보강할 수 있습니다.

`~/.xbindkeysrc` 에 다음을 추가하세요.

```
"unim-cli trigger emoji_popup"
  Mod4 + period
```

적용:

```bash
xbindkeys -p   # 기존 인스턴스 종료
xbindkeys      # 다시 시작
```

---

## 동작 확인

단축키를 설정한 뒤 정상 동작하는지 확인하려면 데몬 로그를 보세요.

```bash
journalctl --user -f | grep unim
```

또는 systemd 서비스로 실행 중이라면:

```bash
journalctl --user -u unim-daemon -f
```

단축키를 누른 직후 다음과 비슷한 메시지가 보이면 성공입니다.

```
[DBus] TriggerAction(emoji_popup) 수신
```

만약 메시지가 보이지 않는다면:

- `which unim-cli` 로 CLI 경로 확인
- `unim-cli trigger emoji_popup` 을 터미널에서 직접 실행했을 때 에러가 없는지 확인
- `systemctl --user status unim-daemon` 으로 데몬이 실행 중인지 확인
- 컴포지터의 단축키 충돌 확인 (다른 앱이 같은 키를 점유했을 수 있음)

---

## 자판 도구 앱 내부 단축키

위 `Super+.` 는 데스크톱 전역 단축키지만, 함께 설치되는 두 GTK4 자판 도구는 **앱 창 안에서만**
동작하는 자체 단축키를 가집니다(컴포지터 등록 불필요). 자세한 사용법은
[사용자 매뉴얼 §5.6](../user-guide/README-ko.md#56-자판-도구-keymap-studio--typing-practice).

### unim-keymap-studio (자판 보기·편집)

| 키 | 동작 |
| -- | ---- |
| F1 | 도움말 |
| Ctrl + N | 새 자판 |
| Ctrl + D | 현재 자판 복제 |
| Ctrl + S | 저장 (사용자 자판) |
| Ctrl + Shift + S | 다른 이름으로 저장 |
| Ctrl + E | 내보내기 |
| Ctrl + I | 가져오기 |
| Ctrl + 1 / 2 / 3 / 4 | 탭 전환 (기본 / 자판 / 조합 / 확장) |

### unim-typing-practice (타자 연습)

| 키 | 동작 |
| -- | ---- |
| F1 | 도움말 |
| Ctrl + R | 다시 시작 |
| Ctrl + Shift + C | 결과 복사 |
| Ctrl + 1 | 연습 화면 |
| Ctrl + 2 | 결과 화면 |
| Ctrl + O | 파일에서 글감 가져오기 |
| Ctrl + Shift + V | 클립보드에서 글감 가져오기 |

---

## 참고

- X11/XIM에서는 X server가 modifier 키 조합을 IM에 redirect 해주기 때문에 자동 동작합니다.
- Wayland에서는 그런 redirect 메커니즘이 표준화되어 있지 않아 컴포지터 협조가 필요합니다.
- 향후 추가될 액션은 `unim-cli trigger <action>` 형태로 동일하게 등록할 수 있습니다.

관련 문서:
- [`unim-cli/SPEC.md`](../../../unim-cli/SPEC.md) — CLI 명세
- [`unim-daemon/SPEC.md`](../../../unim-daemon/SPEC.md) — 데몬 DBus 인터페이스
- [`unim-gnome-extension/SPEC.md`](../../../unim-gnome-extension/SPEC.md) — GNOME 확장 단축키 처리
