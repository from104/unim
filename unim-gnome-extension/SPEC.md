# UNIM GNOME Shell Extension 세부 기능 명세

> GNOME Shell 환경에서 "TypeFIX(오타 보정)" 기능과 "실시간 한글 입력(IME)" 기능을 동시에 제공하는 확장 프로그램입니다.
> `IBus`를 거치지 않고 `Clutter.InputMethod` 서브클래스를 Clutter Backend에 직접 등록하여 네이티브 성능을 확보했습니다.

---

## 1. 아키텍처 개요

### 1.1 컴포넌트 구성

| 파일 | 역할 |
|------|------|
| `extension.js` | 확장의 수명주기 관리 (enable/disable), 컴포넌트 조율 |
| `dbus_ime.js` | `unim-daemon`과의 DBus 통신 (Gio.DBusProxy) |
| `unim_input_method.js` | `Clutter.InputMethod` 서브클래스 (vfunc 오버라이드로 Mutter 연동) |
| `key_handler.js` | 키 이벤트 필터링, 분류, 라우팅 로직 |
| `preedit_overlay.js` | 입력 중인 글자(Preedit)를 커서 위치에 표시하는 오버레이 |
| `hanja_popup.js` | 한자 후보 선택 팝업 UI (`St.BoxLayout` 기반) |
| `special_popup.js` | 특수문자 선택 팝업 UI (9×9 그리드) |
| `vkbd.js` | 가상 키보드 이벤트 생성 (TypeFIX 기능용) |
| `indicator.js` | 상단 패널 입력 모드 표시기 |
| `logging.js` | 통합 로깅 (`UNIM_DEVELOP=1` 시 활성) |
| `stylesheet.css` | 팝업/오버레이/인디케이터 스타일 |

### 1.2 통신 구조

```
┌──────────────┐  clutter_backend_   ┌──────────────────┐    DBus     ┌──────────────┐
│ GNOME Shell  │  set_input_method   │ GNOME Extension  │ ←────────→ │ unim-daemon  │
│ (Mutter)     │ ←─────────────────→ │ (JavaScript)     │  (Session)  │ (Rust)       │
└──────────────┘   C vtable 경유      └──────────────────┘             └──────────────┘
       ↑                                     │
       └────────────── St/Clutter ───────────┘
                  (Preedit/Popup UI)
```

### 1.3 키 이벤트 흐름

```
libinput → Mutter
  └→ vfunc_filter_key_event (UnimInputMethod, C vtable 경유)
     ├→ consumed=true  → commit()/set_preedit_text() → text-input-v3 → 앱
     └→ consumed=false → wl_keyboard.key() → 앱에 직접 전달
```

- `GObject.registerClass()`로 등록된 JS 서브클래스의 vfunc은 C vtable에 올바르게 바인딩됨
- `Clutter.get_default_backend().set_input_method(im)` — `CLUTTER_EXPORT` API, GJS에 노출됨

---

## 2. 주요 기능

### 2.1 실시간 입력기 (Real-time IME)

`IBus`와 독립적으로 동작하며, Clutter Backend의 InputMethod API를 직접 사용합니다.

- **연동 방식**: `Clutter.get_default_backend().set_input_method(this._inputMethod)`
- **키 처리**: `vfunc_filter_key_event`를 오버라이드하여 키 이벤트를 가로채고 `unim-daemon`으로 전달
- **비활성화 시**: `backend.set_input_method(savedInputMethod)`로 원본 IM 복원

### 2.2 키 분류별 처리 (`key_handler.js`)

| 키 분류 | 동작 |
|---------|------|
| 문자 키 | DBus `processKey` → commit/preedit 처리 |
| 수정자 키 (Shift, Ctrl 등) | 무시 (바이패스) |
| Ctrl/Alt/Super 조합 | 조합 flush → 바이패스 (시스템 단축키) |
| 네비게이션 (←→↑↓, Home, End, PgUp/Dn) | 조합 flush → 바이패스 |
| Enter / KP_Enter | 조합 flush → 바이패스 (이중 커밋 방지) |
| Escape | 조합 flush → 바이패스 |
| Tab / Shift+Tab | 조합 flush → 바이패스 |
| 한자키 (F9 등) | 한자/특수문자 팝업 요청 |
| BackSpace | DBus `processKey` → 자모 삭제 |

- **`_flushCompose()`**: DBus `FocusOut` 호출 → 조합 중 텍스트 커밋 + preedit 클리어
- **이중 처리 방지**: Backend IM 등록 시 `captured-event` 핸들러에서 `EVENT_PROPAGATE` 반환

### 2.3 포커스 처리

- **Focus In**: DBus `FocusIn(windowId)` 호출
- **Focus Out**: 조합 중이면 커밋, 팝업 열려있으면 닫기 + 모드 취소, preedit 클리어
- `vfunc_focus_out` → `_focusOutHandler()` 콜백 → extension.js에서 등록

### 2.4 한자/특수문자 팝업

GNOME Shell의 네이티브 UI 툴킷(`St`, `Clutter`)을 사용하여 이질감 없는 디자인 제공.

- **위치**: `vfunc_set_cursor_location`에서 받은 커서 rect 바로 아래
- **화면 경계 처리**: 오른쪽/아래 넘침 시 왼쪽/위로 자동 조정
- **한자 팝업**: 세로 리스트, 숫자(1-9) 직접 선택, ↑↓ 네비게이션, 페이지 전환
- **특수문자 팝업**: 9×9 그리드, top_row 키(q~o)로 열 점프, 숫자로 행 선택
- **포커스 이동 시 자동 닫기**

### 2.5 TypeFIX (오타 보정)

사용자가 실수로 영문 키보드에서 한글을 입력했거나 그 반대일 때, 텍스트를 선택하고 단축키를 눌러 변환합니다.

- **동작 원리**:
  1. `St.Clipboard`를 통해 선택된 텍스트 획득
  2. `unim-cli` 유틸리티를 호출하여 문자열 변환
  3. `VirtualKeyboard` 모듈로 Backspace 연타 + 변환된 텍스트 붙여넣기

---

## 3. DBus 통신 (`dbus_ime.js`)

`Gio.DBusProxy`를 사용하여 동기(Sync) 호출 위주로 구현. 타임아웃 500ms.

| 메서드 | 역할 |
|--------|------|
| `connect(windowId)` | `CreateInputContext` 호출 및 시그널 연결 |
| `processKey(keyval, ...)` | `ProcessKeyEvent` 호출 (consumed, preedit, commit 반환) |
| `focusIn(windowId)` | 포커스 획득 알림 |
| `focusOut()` | 포커스 상실 알림 (조합 중 텍스트 반환) |
| `getHanjaCandidates()` | 한자 후보 목록 조회 |
| `selectHanja(index)` | 한자 선택 |
| `cancelHanja()` | 한자 모드 취소 |
| `getSpecialCharCandidates()` | 특수문자 후보 목록 조회 |
| `selectSpecialChar(index)` | 특수문자 선택 |
| `cancelSpecialChar()` | 특수문자 모드 취소 |
| `getConfig(key)` | 설정 값 조회 |
| `reset()` | 입력 상태 초기화 |

---

## 4. Wayland 통합 상세

### 4.1 UnimInputMethod (Clutter.InputMethod 서브클래스)

`GObject.registerClass()`로 등록되어 vfunc이 C vtable에 바인딩됩니다.

| vfunc | 역할 |
|-------|------|
| `vfunc_filter_key_event(event)` | 키 이벤트 가로채기 → KeyHandler 위임 |
| `vfunc_focus_in(focus)` | 포커스 획득 |
| `vfunc_focus_out()` | 포커스 상실 → 조합 커밋 + 팝업 닫기 |
| `vfunc_reset()` | 입력 상태 리셋 |
| `vfunc_set_cursor_location(rect)` | 커서 위치 저장 (팝업 배치용) |

**공개 API:**
- `commitText(text)` — `this.commit(text)` 래핑
- `updatePreedit(text)` — `this.set_preedit_text()` 래핑
- `clearPreedit()` — preedit 클리어
- `setActive(active)` — IME 활성/비활성
- `cursorRect` — 현재 커서 위치 (getter)
- `setKeyHandler(handler)` — 키 처리 콜백 등록
- `setFocusOutHandler(handler)` — 포커스 상실 콜백 등록

### 4.2 플랫폼별 IM 모듈과의 관계

| 대상 | 처리 주체 | 비고 |
|------|-----------|------|
| Wayland 네이티브 앱 | GNOME IM (Backend level) | vfunc이 먼저 키를 소비 |
| X11/XWayland 앱 | GTK/Qt IM 모듈 또는 XIM | GNOME IM은 `captured-event` 폴백 |
| GNOME Shell UI | GNOME IM | 유일한 처리자 |

- `GTK_IM_MODULE=unim` 설정 시 GTK 앱은 자체 IM 모듈이 로드되지만, GNOME IM이 먼저 키를 소비하므로 충돌 없음
- Backend IM 등록 시 `captured-event`에서 자동으로 스킵하여 이중 처리 방지

---

## 5. 알려진 이슈 및 제한사항

### 5.1 IBus와의 충돌

GNOME 설정에서 IBus 입력기가 활성화되어 있으면 키 이벤트 경합이 발생할 수 있습니다. UNIM 확장을 사용할 때는 IBus를 비활성화하는 것이 좋습니다.

### 5.2 커서 위치 정확도

`vfunc_set_cursor_location`으로 받는 커서 rect는 앱이 `text-input-v3` 프로토콜로 보고한 값입니다. 일부 앱에서는 정확한 위치를 보고하지 않을 수 있으며, 이 경우 팝업이 화면 중앙에 폴백 배치됩니다.

---

## 6. 설치 및 빌드

```bash
# 확장 빌드 및 로컬 배포
make dev-extension

# 시스템 설치
make install-extension

# 로그 확인
journalctl -f -o cat /usr/bin/gnome-shell | grep unim
```
