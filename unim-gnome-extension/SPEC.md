# UNIM GNOME Shell Extension 세부 기능 명세

> GNOME Shell 환경에서 "TypeFIX(오타 보정)" 기능과 "실시간 한글 입력(IME)" 기능을 동시에 제공하는 확장 프로그램입니다.
> `IBus`를 거치지 않고 `Clutter.InputMethod` 서브클래스를 Clutter Backend에 직접 등록하여 네이티브 성능을 확보했습니다.

> 팝업 UI(한자·특수문자)의 공통 명세는 [`../docs/specs/POPUP_SPEC.md`](../docs/specs/POPUP_SPEC.md)를 단일 원본으로 삼습니다.
> 본 문서는 GNOME Shell 고유 구현만을 다룹니다.

---

## 1. 아키텍처 개요

### 1.1 컴포넌트 구성

| 파일 | 역할 |
|------|------|
| `extension.js` | 확장의 수명주기 관리 (enable/disable), 컴포넌트 조율, TypeFIX 단축키 바인딩 |
| `dbus_ime.js` | `unim-daemon`과의 DBus 통신 (`Gio.DBusProxy` + 글로벌 signal 구독) |
| `unim_input_method.js` | `Clutter.InputMethod` 서브클래스 (vfunc 오버라이드로 Mutter 연동) |
| `key_handler.js` | 키 이벤트 필터링·분류·라우팅, call_sync 재진입용 **key queue** |
| `preedit_overlay.js` | 입력 중인 글자(Preedit)를 커서 위치에 표시하는 오버레이 |
| `hanja_popup.js` | 한자 후보 선택 팝업 UI (`St.BoxLayout` 기반) |
| `special_popup.js` | 특수문자 선택 팝업 UI (9×9 그리드) |
| `vkbd.js` | 가상 키보드 이벤트 생성 (AutoTypeFix backspace·TypeFIX paste용) |
| `indicator.js` | 상단 패널 입력 모드 표시기 + 설정/메뉴 항목 |
| `prefs.js` | 확장 환경설정 (GNOME Shell 전용 키만 노출, 일반 설정은 GTK GUI 리다이렉트) |
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
     ├→ self-sent BackSpace 우회 (AutoTypeFix vkbd self-feedback 차단)
     ├→ KeyHandler 위임 → DBus ProcessKeyEvent (call_sync)
     │   └→ call_sync 중 재진입 키는 _keyQueue에 적재 후 _drainKeyQueue()로 순차 처리
     └→ notify_key_event(event, consumed) 로 사후 통보 (IBus 패턴)
```

- `GObject.registerClass()`로 등록된 JS 서브클래스의 vfunc은 C vtable에 올바르게 바인딩됨
- `Clutter.get_default_backend().set_input_method(im)` — `CLUTTER_EXPORT` API, GJS에 노출됨

---

## 2. 주요 기능

### 2.1 실시간 입력기 (Real-time IME)

- **연동 방식**: `Clutter.get_default_backend().set_input_method(this._inputMethod)`
- **키 처리**: `vfunc_filter_key_event` 오버라이드 → `KeyHandler` 위임 → `unim-daemon` DBus 호출
- **비활성화 시**: `backend.set_input_method(savedInputMethod)`로 원본 IM 복원

### 2.2 키 분류별 처리 (`key_handler.js`)

| 키 분류 | 동작 |
|---------|------|
| 문자 키 | DBus `ProcessKeyEvent` → commit/preedit 처리 |
| 수정자 키 (Shift, Ctrl 등) | `vfunc`에서 `false` 반환 → Mutter가 직접 처리 (고정키 접근성 유지) |
| Ctrl/Alt/Super 조합 | 조합 flush → 바이패스 (시스템 단축키) |
| 네비게이션 (←→↑↓, Home, End, PgUp/Dn) | 조합 flush → 바이패스 |
| Enter / KP_Enter, Escape, Tab | 조합 flush → 바이패스 |
| 한자키 (F9, Hangul_Hanja) | 엔진이 팝업 시그널 발사 → GNOME 팝업 표시 |
| BackSpace | DBus `ProcessKeyEvent` → 자모 삭제 (self-sent BackSpace는 선제 우회) |
| KEY_RELEASE | IM 미처리, `notify_key_event(false)`만 호출 (키 반복 유지) |

- **`_flushCompose()`**: DBus `FocusOut` 호출 → 조합 중 텍스트 커밋 + preedit 클리어
- **Key Queue 패턴**: `ProcessKeyEvent`가 `call_sync()`이므로 GLib 메인 루프 재진입이 발생할 수 있음. `_processingKey` 플래그로 재진입을 감지하여 후속 키를 `_keyQueue`에 저장하고, 현재 호출 완료 후 `_drainKeyQueue()`가 FIFO 순서로 처리하여 키 누락을 방지.
- **이중 처리 방지**: Backend IM 등록 시 `captured-event` 핸들러에서 `EVENT_PROPAGATE` 반환

### 2.3 포커스 처리

- **Focus In**: DBus `FocusIn(windowId)` 호출
- **Focus Out**: 팝업 정리 → DBus `FocusOut` → 반환된 조합 중 텍스트를 `commitText`로 커밋 → preedit 클리어
- `vfunc_focus_out` → `_focusOutHandler()` 콜백 → extension.js에서 등록
- 영문 모드 Space는 데몬이 일반 commit 경로로 처리 (2026-04 수정)

### 2.4 한자/특수문자 팝업

> UI/입력 규칙은 [`docs/specs/POPUP_SPEC.md`](../docs/specs/POPUP_SPEC.md) 단일 원본 참조.

GNOME extension 고유 사항:
- **표시 주체**: Wayland 세션일 때 extension이 **글로벌 signal 구독**으로 자기 context 외 프론트엔드(GTK3/4, Qt, XIM)의 팝업 시그널도 수신하여 표시 (Wayland 공통 팝업 서버 역할). X11에서는 `unim-gui-gtk`가 담당하므로 extension은 자기 context 시그널만 처리.
- **좌표 변환**: `_adjustCursorRect()` — 외부 프론트엔드 좌표계 차이 보정
  - 네이티브 Wayland 앱(GTK3/4): 윈도우 상대좌표 + `focus_window.get_buffer_rect()` 오프셋
  - XWayland 앱(XIM/Qt): X11 절대좌표 그대로 사용
- **화면 경계 처리**: 오른쪽/아래 넘침 시 좌/상 조정, 실패 시 화면 중앙 폴백

### 2.5 TypeFIX (오타 보정 단축키)

사용자 단축키(`<Super>k`, `<Shift><Super>k`)로 최근 포커스된 컨텍스트의 선택/주변 텍스트를 변환.

- **엔진 API**: `GlobalTypeFix` (InputMethod iface, 43fbb43에서 도입) — 클립보드 미사용
- **흐름**:
  1. `Main.wm.addKeybinding()` 단축키 감지
  2. `request_surrounding()` 후 50ms 대기 (gedit/gnome-text-editor 호환)
  3. `TypeFix(direction)` DBus 호출 → `(deleteOffset, deleteCount, replacement)` 수신
  4. `delete_surrounding(offset, count)` + `commitText(replacement)` 로 치환
  5. `show-notification` 활성 시 `Main.notify()` 로 알림
- **direction**: 0=자동, 1=영→한, 2=한→영 (`shortcut-normal-reverse` 사용 시 2)

### 2.6 AutoTypeFix (조합 중 자동 교정)

엔진이 롤백 기반 자동 교정을 감지하면 `AutoTypefixApply(deleteChars, commitText, preeditText)` 시그널을 발사.

- **수신 경로**: `dbus_ime.js` 글로벌 signal 구독 (자기 context 한정) → `_handleContextSignal`
- **적용 로직** (`extension.js` `onAutoTypeFix`):
  1. `expectSelfBackspaces(deleteChars)` — `UnimInputMethod`의 self-backspace 카운터 등록
  2. `vkbd.backspaceMultiple(deleteChars)` — 가상 키보드로 BackSpace 연타
  3. 50ms 후 `commitText(commitText)`, 다시 10ms 후 `updatePreedit(preeditText)`
- **Self-feedback 차단 (af8b563)**: vkbd가 보낸 BackSpace는 Mutter를 거쳐 `vfunc_filter_key_event`에 재진입한다. 한글 엔진이 이를 실제 backspace로 오인해 복원된 preedit 음절을 다시 깎는 self-feedback을 막기 위해 `_selfBackspaceCount`를 PRESS+RELEASE = 2×N으로 등록하고, 매칭되는 BackSpace 이벤트는 IM 처리 없이 `false` 반환으로 mutter에 통과시킨다.

---

## 3. DBus 통신 (`dbus_ime.js`)

`Gio.DBusProxy`를 사용하여 동기(Sync) 호출 위주로 구현. 타임아웃 500ms.

### 3.1 Config 캐시

- 시작 시 **`GetConfigJson`** 1회 호출로 `_configCache`에 전체 설정 스냅샷 로드
- **`ConfigChangedJson`** 시그널(InputMethod iface)로 실시간 갱신
- `getCachedConfig()` / `setOnConfigChanged(cb)` API로 소비자(모드 스위치·AutoTypeFix 옵션 등)가 매 키 입력 DBus 호출 없이 최신 값을 참조
- 단일 진실 공급원은 `~/.config/unim/config.yaml`

### 3.2 주요 DBus 메서드 (Context/Method)

| 메서드 | iface | 역할 |
|--------|-------|------|
| `CreateInputContext(client, windowId)` | InputMethod | 컨텍스트 생성, path 반환 |
| `GetConfigJson` | InputMethod | 전체 설정 JSON 문자열 반환 |
| `TypeFix(direction)` | InputMethod | 글로벌 TypeFIX 실행 (offset, count, replacement) |
| `GetConfig(key)` | InputMethod | 단일 키 조회 (레거시) |
| `ProcessKeyEvent(keyval, keycode, state)` | InputContext | `(consumed, preedit, commit)` 반환 |
| `FocusIn(windowId)` / `FocusOut` | InputContext | 포커스 알림. FocusOut은 조합 중 텍스트 반환 |
| `Reset` | InputContext | 입력 상태 초기화 |
| `ReportCursorRect(x,y,w,h)` | InputContext | 커서 좌표 보고 |
| `SetSurroundingText(text, cursor, anchor)` | InputContext | 주변 텍스트 전달 |
| `GetHanjaCandidates` / `SelectHanja(idx)` / `CancelHanja` | InputContext | 한자 변환 |
| `GetSpecialCharCandidates` / `SelectSpecialChar(idx)` / `CancelSpecialChar` | InputContext | 특수문자 변환 |
| `Destroy` | InputContext | 컨텍스트 파괴 |

### 3.3 수신 시그널 — `_handleContextSignal` dispatch

| 시그널 | iface | 처리 경로 |
|--------|-------|-----------|
| `GlobalModeChanged(isKorean)` | InputMethod | 인디케이터 아이콘 갱신 |
| `ConfigChangedJson(jsonStr)` | InputMethod | `_configCache` 갱신 + 콜백 |
| `ShowHanjaPopup(target, candidates, rect)` | InputContext | → `_onShowHanja` |
| `ShowSpecialPopup(target, characters, topRow, rect)` | InputContext | → `_onShowSpecial` |
| `HidePopup` | InputContext | → `_onHidePopup` |
| `PopupNavigate(page, totalPages, selected, rows, cols, selRow, selCol)` | InputContext | → `_onPopupNavigate` |
| `AutoTypefixApply(deleteChars, commit, preedit)` | InputContext | 자기 context에서만 `_onAutoTypeFix` 호출 |

- InputContext 시그널은 `_icProxy` g-signal과 **세션 버스 글로벌 구독(`signal_subscribe`)** 두 경로를 병용. 자기 context는 proxy 경로, 외부 context(Wayland 전용)는 글로벌 경로로 처리하여 중복 방지.
- `AutoTypefixApply`는 proxy introspection 미등록 가능성 때문에 자기 context도 글로벌 경로에서 dispatch.

---

## 4. Wayland 통합 상세

### 4.1 UnimInputMethod (Clutter.InputMethod 서브클래스)

`GObject.registerClass()`로 등록되어 vfunc이 C vtable에 바인딩됩니다.

| vfunc | 역할 |
|-------|------|
| `vfunc_filter_key_event(event)` | 키 이벤트 가로채기 → self-BS 우회 → KeyHandler 위임 (IBus 패턴, 항상 true) |
| `vfunc_focus_in(focus)` | 포커스 획득 |
| `vfunc_focus_out()` | 포커스 상실 → 팝업 정리 + 조합 커밋 |
| `vfunc_reset()` | 입력 상태 리셋 |
| `vfunc_set_cursor_location(rect)` | 커서 위치 저장 (팝업 배치용) |
| `vfunc_set_surrounding(text, cursor, anchor)` | 주변 텍스트 수신 (TypeFIX용) |
| `vfunc_update_content_hints/purpose` | 힌트 저장 |

**공개 API:**
- `commitText(text)`, `updatePreedit(text)`, `clearPreedit()`
- `setActive(active)`, `cursorRect` (getter)
- `setKeyHandler(handler)`, `setFocusOutHandler(handler)`, `setResetHandler(handler)`
- **`expectSelfBackspaces(n)`** — AutoTypeFix self-feedback 차단용 카운터 (`n*2`)

### 4.2 플랫폼별 IM 모듈과의 관계

| 대상 | 처리 주체 | 비고 |
|------|-----------|------|
| Wayland 네이티브 앱 | GNOME IM (Backend level) | vfunc이 먼저 키를 소비 |
| X11/XWayland 앱 | GTK/Qt IM 모듈 또는 XIM | GNOME IM은 `captured-event` 폴백 |
| GNOME Shell UI | GNOME IM | 유일한 처리자 |

- `GTK_IM_MODULE=unim` 환경에서 GTK 앱은 자체 IM 모듈이 로드되지만, GNOME IM이 먼저 키를 소비하므로 충돌 없음
- Backend IM 등록 시 `captured-event`에서 자동으로 스킵하여 이중 처리 방지

---

## 5. 설정 (Settings)

### 5.1 GSettings 스키마 (`schemas/org.gnome.shell.extensions.unim.gschema.xml`)

**Phase 8 Settings Cleanup (2026-04)** 이후, 이 스키마는 GNOME Shell API에 직접 의존하는 키만 남긴다. 일반 설정(자판·입력 모드·한자키·AutoTypeFix 등)은 모두 `~/.config/unim/config.yaml`(SSoT)에서 관리하며 `GetConfigJson`/`ConfigChangedJson`으로 동기화된다.

| 키 | 타입 | 설명 |
|----|------|------|
| `enable-extension` | bool | 확장 전체 활성 |
| `show-notification` | bool | TypeFIX 변환 시 `Main.notify()` 알림 |
| `show-panel-indicator` | bool | 상단 패널 한/영 아이콘 표시 |
| `panel-click-action` | enum(`toggle-mode`\|`menu`) | 인디케이터 좌클릭 동작 |
| `enable-ime` | bool | Clutter.InputMethod 기반 실시간 IME (Wayland) |
| `shortcut-normal` | strv | 영→한 변환 단축키 (기본 `<Super>k`) |
| `shortcut-normal-reverse` | strv | 한→영 변환 단축키 (기본 `<Shift><Super>k`) |

총 **7개 키** (enable + 표시 4 + IME 1 + 단축키 2). 이전 버전의 18개 키에서 대폭 축소되었다.

### 5.2 `prefs.js` UX

- 첫 페이지 상단의 **"UNIM 설정 앱 열기"** 행이 `Gio.Subprocess`로 `unim-gui-gtk --settings`를 띄운다. 자판·AutoTypeFix·한자키 등 일반 설정은 모두 GTK GUI에서 편집.
- 이어서 표시(indicator/notification/click-action), IME 활성, TypeFIX 단축키만 직접 편집.
- Wayland 세션이 아니면 `enable-ime` 행은 비활성화되고 부제가 안내문으로 바뀐다.
- `unim-gui-gtk` 실행 실패 시 `Adw.Toast`로 폴백 안내.

### 5.3 인디케이터 메뉴

- 헤더: 데몬 연결 상태 / 입력 대기 / 현재 모드 표시
- "한국어 모드", "영어 모드" 선택 (체크 오너먼트)
- "UNIM 설정 (Settings)..." → GTK GUI 실행
- "GNOME 확장 설정 (Extension)..." → `prefs.js`

### 5.4 구 GSettings → config.yaml 마이그레이션

레거시 GSettings 트리에서 `config.yaml`로의 일회성 마이그레이션은 `unim-daemon/src/migration.rs`에서 수행하며, `~/.config/unim/.migrated-v2` 가드 파일로 재실행을 방지한다. extension 자체는 더 이상 일반 설정을 GSettings에 기록하지 않는다.

---

## 6. 알려진 이슈 및 제한사항

### 6.1 IBus와의 충돌

GNOME 설정에서 IBus 입력기가 활성화되어 있으면 키 이벤트 경합이 발생할 수 있다. UNIM 확장을 사용할 때는 IBus를 비활성화하는 것이 좋다.

### 6.2 커서 위치 정확도

`vfunc_set_cursor_location`으로 받는 커서 rect는 앱이 `text-input-v3` 프로토콜로 보고한 값이다. 일부 앱에서는 정확한 위치를 보고하지 않으며, 이 경우 팝업이 화면 중앙에 폴백 배치된다.

---

## 7. 설치 및 빌드

```bash
# 확장 빌드 및 로컬 배포
make dev-extension

# 시스템 설치
make install-extension

# 로그 확인
journalctl -f -o cat /usr/bin/gnome-shell | grep unim
```
