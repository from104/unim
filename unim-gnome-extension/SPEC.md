# UNIM GNOME Shell Extension 세부 기능 명세

> GNOME Shell 환경에서 "TypeFIX(오타 보정)" 기능과 "실시간 한글 입력(IME)" 기능을 동시에 제공하는 확장 프로그램입니다.
> `IBus`를 거치지 않고 `Clutter.InputMethod`와 `unim-daemon`을 직접 연결하여 네이티브 성능을 확보했습니다.

---

## 1. 아키텍처 개요

### 1.1 컴포넌트 구성

| 파일 | 역할 |
|------|------|
| `extension.js` | 확장의 수명주기 관리 (enable/disable), 컴포넌트 조율 |
| `dbus_ime.js` | `unim-daemon`과의 DBus 통신 (Gio.DBusProxy) |
| `unim_input_method.js` | `Clutter.InputMethod` 서브클래스 (Mutter와 연동) |
| `key_handler.js` | 키 이벤트 필터링 및 라우팅 로직 |
| `preedit_overlay.js` | 입력 중인 글자(Preedit)를 커서 위치에 표시하는 오버레이 |
| `hanja_popup.js` | 한자 후보 선택 팝업 UI (`St.BoxLayout` 기반) |
| `special_popup.js` | 특수문자 선택 팝업 UI |
| `vkbd.js` | 가상 키보드 이벤트 생성 (TypeFIX 기능용) |
| `indicator.js` | 상단 패널 입력 모드 표시기 |

### 1.2 통신 구조

```
┌──────────────┐    Clutter API     ┌──────────────────┐    DBus     ┌──────────────┐
│ GNOME Shell  │ ←───────────────→ │ GNOME Extension  │ ←────────→ │ unim-daemon  │
│ (Mutter)     │  InputMethod/Seat  │ (JavaScript)     │  (Session)  │ (Rust)       │
└──────────────┘                    └──────────────────┘             └──────────────┘
       ↑                                     │
       └────────────── St/Clutter ───────────┘
                  (Preedit/Popup UI)
```

---

## 2. 주요 기능

### 2.1 실시간 입력기 (Real-time IME)

`IBus`와 독립적으로 동작하며, GNOME Shell의 내장 입력기 인터페이스를 사용합니다.

- **연동 방식**: `Clutter.get_default_backend().get_default_seat().set_input_method(this._inputMethod)`
- **키 처리**: `vfunc_filter_key_event`를 오버라이드하여 키 이벤트를 가로채고 `unim-daemon`으로 전달
- **Preedit**: `Clutter.Text`나 `St.Label`이 아닌, `Main.layoutManager`에 직접 그리는 `PreeditOverlay` 사용

### 2.2 한자/특수문자 팝업

Wayland 프론트엔드와 달리, GNOME Shell의 네이티브 UI 툴킷(`St`, `Clutter`)을 사용하여 이질감 없는 디자인을 제공합니다.

- **구현**: `St.BoxLayout` (컨테이너) + `St.Label` (항목)
- **위치**: 현재 모니터 중앙 상단 (포커스 위치 추적의 기술적 한계로 고정 위치 사용 권장)
- **네비게이션**: 방향키, PageUp/Down, 숫자키 지원 (Wayland/GTK와 동일한 UX)

### 2.3 TypeFIX (오타 보정)

사용자가 실수로 영문 키보드에서 한글을 입력했거나 그 반대일 때, 텍스트를 선택하고 단축키를 눌러 변환합니다.

- **동작 원리**:
  1. `St.Clipboard`를 통해 선택된 텍스트 획득
  2. `unim-cli` 유틸리티를 호출하여 문자열 변환
  3. `VirtualKeyboard` 모듈로 Backspace 연타 + 변환된 텍스트 붙여넣기

---

## 3. DBus 통신 (`dbus_ime.js`)

`Gio.DBusProxy`를 사용하여 동기(Sync) 호출 위주로 구현되었습니다. 자바스크립트의 비동기 특성상 입력 지연을 막기 위해 타임아웃을 짧게(500ms) 설정합니다.

| 메서드 | 역할 |
|--------|------|
| `connect(windowId)` | `CreateInputContext` 호출 및 시그널 연결 |
| `processKey(keyval, ...)` | `ProcessKeyEvent` 호출 (키 소비 여부 반환) |
| `focusIn(windowId)` | 포커스 획득 알림 |
| `focusOut()` | 포커스 상실 알림 (조합 중 텍스트 커밋) |
| `getHanjaCandidates()` | 한자 후보 목록 조회 |
| `selectHanja(index)` | 후보 선택 |

---

## 4. 알려진 이슈 및 제한사항

### 4.1 Wayland 호환성

이 확장은 GNOME Shell 내부에서 동작하므로 X11과 Wayland 세션 모두에서 동작하지만, **Wayland에서는 클라이언트의 커서 위치(`cursor-location`)를 정확히 알 수 없습니다.** 이로 인해 Preedit과 팝업이 커서를 따라다니지 못하고 고정된 위치(화면 중앙 등)에 표시될 수 있습니다.

### 4.2 IBus와의 충돌

GNOME 설정에서 IBus 입력기가 활성화되어 있으면 키 이벤트 경합이 발생할 수 있습니다. UNIM 확장을 사용할 때는 IBus 입력 소스를 '영어(No input method)'로 설정하거나 IBus를 비활성화하는 것이 좋습니다.

---

## 5. 설치 및 빌드

```bash
# 확장 빌드 및 설치
make install-extension  # 또는 make install-gnome-extension

# 로그 확인
journalctl -f -o cat /usr/bin/gnome-shell | grep UNIM
```
