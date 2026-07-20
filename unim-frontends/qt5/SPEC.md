# UNIM Qt5 프론트엔드 세부 기능 명세

> Qt5 애플리케이션에서 한글 입력을 제공하는 QPlatformInputContext 플러그인의 상세 동작을 정의합니다.

---

## 1. 아키텍처 개요

### 1.1 컴포넌트 구성

| 파일 | 위치 | 역할 |
|------|------|------|
| `plugin.cpp` | `qt5/src/` | Qt5 플러그인 엔트리 포인트 (`QPlatformInputContextPlugin`) |
| `input_context.cpp` | `qt5/src/` | 입력 컨텍스트 구현 (`QPlatformInputContext`) |
| `input_context.hpp` | `qt5/src/` | 입력 컨텍스트 헤더 |
| `unim_dbus_client.cpp` | `qt-common/src/` | QtDBus 기반 unim-daemon 통신 (Qt5/6 공용) |
| `unim_dbus_client.hpp` | `qt-common/include/` | DBus 클라이언트 API 헤더 |

> [!NOTE]
> 한자/특수문자/이모지 팝업은 플러그인이 **직접 그리지 않습니다**.
> 모든 팝업 렌더링은 독립 GTK4 프로세스인 **unim-popup-service**가 담당하며,
> 플러그인은 데몬의 팝업 DBus 신호(`ShowEmojiPopupV2`/`HidePopup` 등)를 받아
> `m_popupActive` 플래그만 관리합니다 (§9 참고).
> 과거 `qt-common`에 있던 `unim_hanja_popup`·`unim_special_popup` 임베디드 위젯은
> 제거되었고, 현재 `qt-common`에 남은 소스는 `unim_dbus_client`뿐입니다.

> 팝업 UI/조작 명세는 [`docs/dev/specs/POPUP_SPEC.md`](../../docs/dev/specs/POPUP_SPEC.md) 참조 (GTK/Qt 공통 규격, popup-service가 구현).

### 1.2 통신 구조

```
┌────────────────────┐  QPlatformInputContext  ┌──────────────┐   QtDBus   ┌──────────────┐
│  Qt5 애플리케이션  │ ←──────────────────→  │   libunim    │ ←───────→ │  unim-daemon │
│   (KDE, Telegram)  │   (filterEvent 등)     │ (.so 플러그인)│  (동기)    │  (입력 엔진) │
└────────────────────┘                        └──────────────┘           └──────────────┘
```

### 1.3 주요 의존성

| Qt 모듈 | 용도 |
|---------|------|
| `Qt5::Core` | 기본 타입, 이벤트 시스템 |
| `Qt5::Gui` | 입력 메서드 프레임워크 |
| `Qt5::GuiPrivate` | `QPlatformInputContext` Private API |
| `Qt5::DBus` | QtDBus 통신 |

---

## 2. 플러그인 수명주기

### 2.1 플러그인 등록

Qt5는 **QPlatformInputContextPlugin** 서브클래스 + JSON 메타데이터로 등록합니다.

```cpp
class UnimPlatformInputContextPlugin : public QPlatformInputContextPlugin {
    Q_OBJECT
    Q_PLUGIN_METADATA(IID QPlatformInputContextFactoryInterface_iid FILE "unim.json")
public:
    QPlatformInputContext *create(const QString &key, const QStringList &) override {
        if (key.compare(QLatin1String("unim"), Qt::CaseInsensitive) == 0)
            return new UnimInputContext();
        return nullptr;
    }
};
```

활성화: `QT_IM_MODULE=unim` 환경변수

> [!NOTE]
> GTK3/4는 `GtkIMContext` / GIO Extension Point 방식이지만,
> Qt5는 `QPlatformInputContext` 플러그인 방식으로 등록됩니다.

### 2.2 컨텍스트 초기화 (`UnimInputContext()`)

1. `UNIM_DEVELOP=1` 여부 확인 → 디버그 모드 설정
2. `m_windowId` 생성: `"qt5-ctx-0x..."` (컨텍스트 포인터 기반)
3. `UnimDbusClient("qt5-unim", m_windowId)` → DBus 클라이언트 생성
4. 상태 필드 초기화 (focusObject, composing, popupActive, cursorRect)
5. `setAutoTypeFixCallback()` — `AutoTypefixApply` 시그널 구독.
   람다에서 `ev.setCommitString(commitText, -deleteChars, deleteChars)`로 기존 텍스트 삭제 + 교정 텍스트 커밋,
   이어서 `preeditText`가 있으면 `SingleUnderline` 속성의 `QInputMethodEvent`를 focusObject에 전달 (AutoTypeFix 순방향 재조합 표시)
6. `setCommitTextCallback()` — `CommitText` 시그널 구독 (standalone 팝업의 마우스 클릭 커밋 수신용)
7. `setShowEmojiPopupCallback()` — `ShowEmojiPopupV2` 시그널 구독 (`m_popupActive = true` 마킹)
8. `setHidePopupCallback()` — `HidePopup` 시그널 구독 (`m_popupActive = false`)

### 2.3 컨텍스트 소멸 (`~UnimInputContext()`)

1. `delete m_dbus`

---

## 3. 컨텍스트 상태 (`UnimInputContext`)

```cpp
class UnimInputContext : public QPlatformInputContext {
private:
    UnimDbusClient *m_dbus;          /* QtDBus 클라이언트 */
    QObject *m_focusObject;          /* 현재 포커스된 위젯 */
    QString m_windowId;              /* "qt5-ctx-0x..." */
    bool m_composing;                /* 조합 중 여부 (로컬 캐시) */
    bool m_popupActive;              /* popup-service 팝업 활성 여부 (nav 키 우회 차단용) */
    QRect m_cursorRect;              /* 커서 위치 (글로벌 좌표) */
};
```

> [!NOTE]
> 플러그인은 한자/특수문자 후보나 팝업 위젯 포인터를 보관하지 않습니다.
> 후보 데이터·렌더링·키 네비게이션은 unim-daemon + unim-popup-service가 처리하며,
> 플러그인은 팝업이 떠 있는 동안 `m_popupActive` 플래그만 유지합니다.

> [!NOTE]
> GTK3/4는 `GtkIMContext` 인스턴스가 위젯별로 생성되지만,
> Qt5에서는 **단일 `QPlatformInputContext` 인스턴스**가 전체 앱에서 공유됩니다.
> 포커스 전환은 `setFocusObject()` 콜백으로 관리합니다.

---

## 4. 키 입력 처리 (`filterEvent`)

### 4.1 전처리

```
이벤트 수신 (const QEvent*)
  → 1. DBus/포커스 유효성 확인 (없으면 return false)
  → 2. KeyPress 이벤트만 처리 (KeyRelease 무시)
  → 3. QKeyEvent로 캐스팅
  → 4. 수정자 키 바이패스 → return false (앱에 전달)
```

**바이패스 대상 수정자 키:**
- Shift, Control, Alt, Meta
- Super_L, Super_R, AltGr

### 4.2 한자 키 처리 (`F9` / `Hangul_Hanja`) — 팝업 트리거

한자키 입력 시 플러그인은 **팝업을 직접 그리지 않고** 데몬에 후보 존재 여부를 질의한 뒤
실제 표시는 unim-popup-service에 위임합니다.

```
F9 또는 Hangul_Hanja 입력
  → 1. DBus getHanjaCandidates(target, candidates)
       (엔진의 start_hanja_conversion() 트리거)
  → 한자 후보가 있으면: 후보 데이터를 별도 표시하지 않고 popup-service에 위임 ("Standalone popup 위임" 로그)
  → 한자 후보가 없으면:
     → 2. DBus getSpecialCharCandidates(target, chars, topRow)
          (엔진이 이미 special_char_mode를 설정한 상태)
     → 특수문자 후보가 있으면: popup-service에 위임
     → 둘 다 없으면(idle): processKey 로 dual-purpose Hanja 분기 →
        엔진이 ShowEmojiPopupV2 시그널 발행 → 콜백이 m_popupActive 마킹
  → return true (키 소비)
```

> [!IMPORTANT]
> **호출 순서가 중요합니다.** `getHanjaCandidates()`를 반드시 먼저 호출해야 합니다.
> 이 호출이 엔진의 `start_hanja_conversion()`을 트리거하여 한자/특수문자 모드를 설정합니다.
> `getSpecialCharCandidates()`는 이미 설정된 모드 상태만 읽으므로, 순서가 바뀌면 첫 번째 키 입력에서 후보가 표시되지 않습니다.
> 이 순서는 GTK3/4 구현과 동일합니다.

> [!NOTE]
> 플러그인은 후보 데이터를 받더라도 직접 렌더링하지 않습니다. 후보 표시·페이지·선택은
> unim-popup-service가 전담하며, 플러그인은 후보 존재 여부만 확인해 트리거 역할을 합니다.
> 커서 위치는 `m_cursorRect`(글로벌 좌표)를 통해 데몬에 보고됩니다.

### 4.3 팝업 활성 중 키 처리 (`m_popupActive`)

데몬이 `Show*Popup` 시그널을 보내면 `m_popupActive = true`가 됩니다.
이 동안 들어오는 키는 플러그인이 직접 해석하지 않고 **그대로 `processKey`로 전달**되며,
데몬이 그 키를 받아 popup-service의 선택/페이지 이동·확정·취소를 구동합니다.
선택 확정 커밋은 `processKey` 응답 또는 `CommitText` 시그널(마우스 클릭)로 도달하고,
팝업 종료 시 `HidePopup` 시그널로 `m_popupActive`가 해제됩니다.

### 4.4 일반 키 처리 (processKey)

```
키 입력 → 수정자 상태 변환 (Qt::KeyboardModifiers → 비트필드)
       → evdev 코드 변환 (nativeScanCode - 8)
       → DBus processKey(key, evdev_code, mod_state)
       → 응답: UnimDbusKeyResult { consumed, preedit, commit }
```

#### 4.4.1 수정자 상태 비트필드 변환

| Qt 마스크 | 비트 | 의미 |
|----------|------|------|
| `Qt::ShiftModifier` | bit 0 | Shift |
| `Qt::ControlModifier` | bit 2 | Ctrl |
| `Qt::AltModifier` | bit 3 | Alt |
| `Qt::MetaModifier` | bit 26 | Super/Meta |

> **자동 반복·Alt_R 태깅(접근성)**: `state` 상위 비트에 반복 정보를 함께 실어 보낸다 — `QKeyEvent::isAutoRepeat()` 이 참이면 `UNIM_KEY_REPEAT_MASK`(1<<29), 그리고 항상 `UNIM_REPEAT_AWARE_MASK`(1<<31)를 세운다(메인·이모지 두 경로). 데몬은 `ignore_key_repeat` on 일 때 이 비트로 반복을 정확 판정한다. 또한 bare **Alt_R** 은 스킵하지 않고 데몬에 전달해 `toggle_keys` 로 토글 여부를 판정한다 — `Qt::Key_Alt` 는 좌우를 구분하지 못하므로 `nativeScanCode`(evdev 100) 기준으로 오른쪽 Alt만 골라낸다. `Key_AltGr`·`ISO_Level3_Shift` 는 계속 스킵하므로 AltGr 레이아웃에는 영향이 없다.

#### 4.4.2 결과 처리

```
result.consumed == true:
  → 1. 선택 영역 삭제 (ImAnchorPosition ≠ ImCursorPosition)
  → 2. commit 텍스트 커밋
  → 3. m_composing 갱신 + updatePreedit()
  → return true

result.consumed == false:
  → commit이 있으면 커밋 (Enter, Tab 등)
  → 조합 중이었다면 commit() 호출 (preedit 강제 커밋)
  → return false (앱에 키 바이패스)
```

> [!NOTE]
> 영문 모드의 Space는 엔진이 직접 커밋 경로(`consumed=true`, `commit=" "`)로 처리합니다 (552b5bd).
> 한글 모드 직접 커밋과 동일하게 Qt 플러그인은 `result.consumed == true` 분기의 `commitString(" ")`만 수행합니다.

### 4.5 선택 영역 자동 삭제

키가 엔진에 의해 소비된 경우, 선택 영역이 있으면 자동 삭제:

```
QInputMethodQueryEvent(ImAnchorPosition | ImCursorPosition)
  → anchorPos ≠ cursorPos (선택 영역 존재)
    → QInputMethodEvent::setCommitString("", offset, length)로 삭제
```

---

## 5. 포커스 관리 (`setFocusObject`)

Qt5에서는 GTK의 `focus_in`/`focus_out` 대신 **`setFocusObject(QObject*)`** 하나로 관리합니다.

```
setFocusObject(newObject) 호출
  → 1. 이전 포커스에서 조합 중이었으면:
       → focusOut() → 조합 커밋
       → m_composing = false + updatePreedit()
  → 2. m_focusObject = newObject
  → 3. newObject가 있으면 focusIn(m_windowId)
```

> [!NOTE]
> 팝업 정리는 플러그인이 직접 하지 않습니다. 데몬이 팝업 종료를 결정하고
> `HidePopup` 시그널로 통지하면 `m_popupActive`가 해제됩니다 (§4.3 참고).

### 5.1 리셋 (`reset`)

```
reset() 호출
  → 1. DBus reset() → 조합 중 텍스트 커밋
  → 2. m_composing = false + updatePreedit()
```

### 5.2 커밋 (`commit`)

```
commit() 호출 (조합 중일 때만)
  → 1. DBus reset() → 조합 중 텍스트 커밋
  → 2. m_composing = false + updatePreedit()
```

---

## 6. Preedit (조합 문자) 표시

### 6.1 preedit 업데이트 (`updatePreedit`)

```
updatePreedit() 호출
  → focusObject 없으면 return
  → DBus getPreedit() → 현재 조합 문자열
  → QInputMethodEvent 생성:
     → 텍스트가 있으면 TextFormat(SingleUnderline) 속성 추가
  → QCoreApplication::sendEvent(focusObject, &imEvent)
```

> [!NOTE]
> GTK에서는 `g_signal_emit_by_name(context, "preedit-changed")`로 시그널을 발생시키고
> 앱이 `get_preedit_string()`을 호출하는 **풀(pull)** 방식이지만,
> Qt에서는 `QInputMethodEvent`를 직접 전달하는 **푸시(push)** 방식입니다.

### 6.2 커밋 문자열 전달 (`commitString`)

```
commitString(str) 호출
  → focusObject 없거나 str 비어있으면 return
  → QInputMethodEvent + setCommitString(str)
  → QCoreApplication::sendEvent(focusObject, &imEvent)
```

---

## 7. 커서 위치 업데이트 (`update`)

```
update(Qt::ImCursorRectangle) 호출
  → QInputMethodQueryEvent(ImCursorRectangle) → 위젯 로컬 좌표
  → QWidget 계층 탐색 → mapToGlobal() 변환
  → m_cursorRect에 글로벌 좌표 저장
```

> [!NOTE]
> GTK에서는 `set_cursor_location()`이 로컬 좌표를 받아 팝업 표시 시 변환하지만,
> Qt에서는 `update()` 시 **미리 글로벌 좌표로 변환**하여 저장합니다.

---

## 8. DBus 통신 (`UnimDbusClient`)

### 8.1 연결 정보

| 항목 | 값 |
|------|-----|
| 서비스 | `org.atit.unim.InputMethod` |
| 경로 | `/org/atit/unim/InputMethod` |
| 인터페이스 | `org.atit.unim.InputMethod` / `org.atit.unim.InputContext` |
| 타임아웃 | 500ms |
| 통신 방식 | QtDBus 동기 호출 |

### 8.2 주요 DBus 메서드

| C++ 메서드 | DBus 메서드 | 반환 | 용도 |
|-----------|------------|------|------|
| `UnimDbusClient()` | `CreateContext` | `context_path` | 컨텍스트 등록 |
| `~UnimDbusClient()` | `DestroyContext` | — | 컨텍스트 해제 |
| `focusIn(windowId)` | `FocusIn` | — | 포커스 획득 알림 |
| `focusOut()` | `FocusOut` | `QString` | 포커스 상실 → 조합 커밋 문자열을 **RPC 반환값으로** 수신 (단일 채널) |
| `processKey(keyval, keycode, state)` | `ProcessKey` | `UnimDbusKeyResult` | 키 입력 처리 |
| `reset()` | `ResetContext` | `QString` | 상태 초기화 → 조합 커밋 |
| `getPreedit()` | (캐시 조회) | `QString` | 현재 preedit 문자열 |
| `isComposing()` | (캐시 조회) | `bool` | 조합 중 여부 |
| `getHanjaCandidates(...)` | `GetHanjaCandidates` | `target, candidates[]` | 한자 후보 조회 |
| `selectHanja(index, ...)` | `SelectHanja` | `selectedHanja` | 한자 후보 선택 |
| `cancelHanja()` | `CancelHanja` | — | 한자 모드 취소 |
| `getSpecialCharCandidates(...)` | `GetSpecialCharCandidates` | `target, chars[], topRow` | 특수문자 후보 조회 |
| `selectSpecialChar(index, ...)` | `SelectSpecialChar` | `selectedChar` | 특수문자 후보 선택 |
| `cancelSpecialChar()` | `CancelSpecialChar` | — | 특수문자 모드 취소 |
| `reportCursorRect(x,y,w,h)` | `ReportCursorRect` | — | 커서 위치 보고 (Wayland 팝업 좌표용) |
| `setContentType(purpose)` / `setSurroundingText(...)` | `SetContentType` / `SetSurroundingText` | — | 입력 힌트/주변 텍스트 전달 |

### 8.3 구독 시그널

| DBus 시그널 | 콜백 | 용도 |
|-------------|------|------|
| `AutoTypefixApply(u s s)` | `setAutoTypeFixCallback()` | `(deleteChars, commitText, preeditText)` — 한영 오타 자동 교정 적용 |
| `CommitText(s)` | `setCommitTextCallback()` | standalone 팝업의 마우스 클릭 커밋 |
| `ShowEmojiPopupV2` | `setShowEmojiPopupCallback()` | popup-service 팝업 표시 통지 → `m_popupActive = true` |
| `HidePopup` | `setHidePopupCallback()` | 팝업 종료 통지 → `m_popupActive = false` |

> [!NOTE]
> `FocusOut`은 (552b5bd 이후) `CommitText` 시그널을 동반하지 않고 **RPC 반환값만** 커밋 채널로 사용합니다.
> 과거 시그널은 브로드캐스트라 컨텍스트 비한정이라 중복 커밋(예: `늘` 두 번)을 일으켰습니다.
> Config 조회/변경은 `GetConfigYaml`/`SetConfigYaml`/`ConfigChangedJson` (modern) 및 legacy `GetConfig`/`SetConfig`/`ConfigChanged`가 병존합니다 — `unim-dbus/SPEC.md` 참조. Qt 플러그인은 설정 변경을 실시간 구독하지 않습니다 (재시작 또는 설정 GUI 경유).

---

## 9. 팝업 렌더링 (unim-popup-service 위임)

한자·특수문자·이모지 팝업은 **플러그인이 직접 그리지 않습니다**. 모든 팝업 UI는 독립
GTK4 프로세스인 **unim-popup-service**(코어 `unim` 크레이트의 popup 모듈 사용)가 렌더링하며,
플러그인은 트리거와 `m_popupActive` 플래그 관리만 담당합니다.

### 9.1 역할 분담

| 책임 | 담당 |
|------|------|
| 한자키 입력 감지 → 후보 존재 질의 | 플러그인 (`filterEvent`, §4.2) |
| 커서 글로벌 좌표 보고 | 플러그인 (`update()` → `m_cursorRect`) |
| 후보 데이터·페이지·선택 상태 | unim-daemon |
| 팝업 윈도우 생성·그리드 렌더·하이라이트·위치 보정 | unim-popup-service (GTK4) |
| 선택 결과 커밋 전달 | `processKey` 응답(키보드) 또는 `CommitText` 시그널(마우스 클릭) |
| 팝업 종료 통지 | `HidePopup` 시그널 → `m_popupActive = false` |

> [!NOTE]
> 팝업 UI/조작 규격(그리드 레이아웃, 열 점프, 페이지네이션, 시각적 피드백 등)은
> `docs/dev/specs/POPUP_SPEC.md`(GTK/Qt 공통)에 정의되어 있으며 popup-service가 구현합니다.
> 과거 이 SPEC에 있던 `UnimHanjaPopup`/`UnimSpecialPopup` Qt 위젯 구현 절은
> 해당 코드가 제거됨에 따라 삭제되었습니다.

---

## 10. Qt5 vs GTK3 비교

| 관점 | GTK3 | Qt5 |
|------|------|-----|
| **프레임워크** | GtkIMContext | QPlatformInputContext |
| **모듈 등록** | `im_module_init/list/create` | QPlatformInputContextPlugin + JSON |
| **인스턴스** | 위젯별 GtkIMContext | **앱 전체 단일 인스턴스** |
| **포커스** | `focus_in()` / `focus_out()` 분리 | `setFocusObject(QObject*)` 통합 |
| **Preedit 방식** | 풀(pull): 시그널 → get_preedit_string | 푸시(push): QInputMethodEvent 직접 전달 |
| **좌표 계산** | `set_cursor_location()` 로컬 → 팝업 시 변환 | `update()` 시 `mapToGlobal()` 미리 변환 |
| **선택 삭제** | `g_signal_emit_by_name("delete-surrounding")` | `QInputMethodEvent::setCommitString("", offset, len)` |
| **키 이벤트** | `GdkEventKey*` 구조체 | `QKeyEvent*` |
| **수정자 마스크** | `GDK_MOD1_MASK` (Alt) | `Qt::AltModifier` |
| **키코드 추출** | `event->hardware_keycode` | `keyEvent->nativeScanCode()` |
| **통신 라이브러리** | GDBus (GIO) | QtDBus |
| **팝업 UI** | unim-popup-service 위임 (모듈은 트리거만) | unim-popup-service 위임 (플러그인은 트리거만) |

---

## 11. QPlatformInputContext 인터페이스

| 메서드 | 구현 |
|--------|------|
| `isValid()` | DBus 연결 유효성 반환 |
| `reset()` | 조합 커밋 (팝업 정리는 데몬/popup-service) |
| `commit()` | 조합 중이면 커밋 (팝업 정리는 데몬/popup-service) |
| `update(queries)` | `ImCursorRectangle` → 글로벌 좌표 갱신 |
| `invokeAction(action, pos)` | 미사용 |
| `filterEvent(event)` | 메인 키 입력 처리 |
| `keyboardRect()` | 빈 QRectF (가상 키보드 없음) |
| `isAnimating()` | false |
| `showInputPanel()` | 미구현 |
| `hideInputPanel()` | 미구현 |
| `isInputPanelVisible()` | false |
| `locale()` | `QLocale::Korean` |
| `inputDirection()` | `Qt::LeftToRight` |
| `setFocusObject(obj)` | 포커스 전환 관리 |

---

## 12. 빌드 및 배포

### 13.1 빌드

```bash
mkdir -p unim-frontends/qt5/build
cd unim-frontends/qt5/build
cmake ..
make
```

또는 프로젝트 루트에서:

```bash
make build-frontends
```

### 13.2 개발 배포 (`make dev-qt5`)

```bash
make dev-qt5 PREFIX=/usr
```

### 13.3 설치 경로

```
$(QT5_PLUGINS_DIR)/platforminputcontexts/libunim.so
```

일반적으로: `/usr/lib/x86_64-linux-gnu/qt5/plugins/platforminputcontexts/libunim.so`

### 13.4 로컬 테스트

빌드 후 자동으로 `build/platforminputcontexts/libunim.so`에 복사됩니다:

```bash
QT_PLUGIN_PATH=$PWD/build QT_IM_MODULE=unim your-qt5-app
```

---

## 13. 로깅

`UNIM_DEVELOP=1` 환경변수 설정 시 활성화.

| 모듈명 | 컴포넌트 |
|--------|---------|
| `QT5_IM` | `input_context.cpp` (키 처리, 포커스, preedit, m_popupActive) |
| `QT5_DBUS` | `unim_dbus_client.cpp` (DBus 통신) |

> [!NOTE]
> 팝업 렌더링 로그는 unim-popup-service 측에 있습니다 (플러그인은 팝업을 직접 그리지 않음).

로그 포맷:
```
[YYYY/MM/DD HH:MM:SS] - [QT5_IM] - 메시지
```

출력 대상:
- 콘솔 (`qDebug`)
- 파일 (`~/.unim-errors.log`)
