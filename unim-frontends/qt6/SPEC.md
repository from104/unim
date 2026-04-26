# UNIM Qt6 프론트엔드 세부 기능 명세

> Qt6 애플리케이션에서 한글 입력을 제공하는 QPlatformInputContext 플러그인의 상세 동작을 정의합니다.

---

## 1. 아키텍처 개요

### 1.1 컴포넌트 구성

| 파일 | 위치 | 역할 |
|------|------|------|
| `plugin.cpp` | `qt6/src/` | Qt6 플러그인 엔트리 포인트 (`QPlatformInputContextPlugin`) |
| `input_context.cpp` | `qt6/src/` | 입력 컨텍스트 구현 (`QPlatformInputContext`) |
| `input_context.hpp` | `qt6/src/` | 입력 컨텍스트 헤더 |
| `unim_dbus_client.cpp` | `qt-common/src/` | QtDBus 기반 unim-daemon 통신 (Qt5/6 공용) |
| `unim_dbus_client.hpp` | `qt-common/include/` | DBus 클라이언트 API 헤더 |
| `unim_hanja_popup.cpp` | `qt-common/src/` | Qt 기반 한자 후보 팝업 윈도우 (Qt5/6 공용) |
| `unim_hanja_popup.hpp` | `qt-common/include/` | 한자 팝업 API 헤더 |
| `unim_special_popup.cpp` | `qt-common/src/` | 특수문자 그리드 팝업 윈도우 (Qt5/6 공용) |
| `unim_special_popup.hpp` | `qt-common/include/` | 특수문자 팝업 API 헤더 |

> 팝업 UI/조작 명세는 [`docs/dev/specs/POPUP_SPEC.md`](../../docs/dev/specs/POPUP_SPEC.md) 참조 (GTK/Qt 공통 규격).

### 1.2 통신 구조

```
┌────────────────────┐  QPlatformInputContext  ┌──────────────┐   QtDBus   ┌──────────────┐
│  Qt6 애플리케이션  │ ←──────────────────→  │   libunim    │ ←───────→ │  unim-daemon │
│  (KDE6, 최신 앱)   │   (filterEvent 등)     │ (.so 플러그인)│  (동기)    │  (입력 엔진) │
└────────────────────┘                        └──────────────┘           └──────────────┘
```

### 1.3 주요 의존성

| Qt 모듈 | 용도 |
|---------|------|
| `Qt6::Core` | 기본 타입, 이벤트 시스템 |
| `Qt6::Gui` | 입력 메서드 프레임워크 |
| `Qt6::GuiPrivate` | `QPlatformInputContext` Private API |
| `Qt6::DBus` | QtDBus 통신 |
| `Qt6::Widgets` | 한자 팝업 UI |

---

## 2. 플러그인 수명주기

### 2.1 플러그인 등록

Qt6에서는 **`qt_add_plugin()`** CMake 매크로를 사용하여 플러그인을 빌드합니다.

```cmake
qt_add_plugin(unim
    CLASS_NAME UnimPlatformInputContextPlugin
    PLUGIN_TYPE platforminputcontexts
)
```

플러그인 코드는 Qt5와 동일한 패턴:

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
> Qt5에서는 `add_library(unim MODULE ...)` + 수동 Private 헤더 설정이 필요하지만,
> Qt6에서는 `qt_add_plugin()`이 플러그인 타입/클래스명/GuiPrivate를 자동 처리합니다.

### 2.2 컨텍스트 초기화 (`UnimInputContext()`)

1. `QPlatformInputContext()` 명시적 부모 생성자 호출
2. `UNIM_DEVELOP=1` 여부 확인 → 디버그 모드 설정
3. `m_windowId` 생성: `"qt6-ctx-0x..."` (컨텍스트 포인터 기반)
4. `UnimDbusClient("qt6-unim", m_windowId)` → DBus 클라이언트 생성
5. `UnimHanjaPopup()` → 한자 팝업 인스턴스 생성
6. 상태 필드 초기화 (focusObject, composing, cursorRect)
7. `setAutoTypeFixCallback()` — `AutoTypefixApply` 시그널 구독.
   람다에서 `ev.setCommitString(commitText, -deleteChars, deleteChars)`로 기존 텍스트 삭제 + 교정 텍스트 커밋,
   이어서 `preeditText`가 있으면 `SingleUnderline` 속성의 `QInputMethodEvent`를 focusObject에 전달 (AutoTypeFix 순방향 재조합 표시)
8. `setCommitTextCallback()` — `CommitText` 시그널 구독 (standalone 한자/특수문자 팝업의 마우스 클릭 커밋 수신용)

> [!NOTE]
> Qt5에서는 부모 생성자 호출을 생략하지만,
> Qt6에서는 `QPlatformInputContext()` 를 **명시적으로** 호출합니다.

### 2.3 컨텍스트 소멸 (`~UnimInputContext()`)

1. `delete m_hanjaPopup`
2. `delete m_dbus`

---

## 3. 컨텍스트 상태 (`UnimInputContext`)

```cpp
class UnimInputContext : public QPlatformInputContext {
private:
    UnimDbusClient *m_dbus;          /* QtDBus 클라이언트 */
    UnimHanjaPopup *m_hanjaPopup;    /* 한자 후보 팝업 */
    QObject *m_focusObject;          /* 현재 포커스된 위젯 */
    QString m_windowId;              /* "qt6-ctx-0x..." */
    bool m_composing;                /* 조합 중 여부 (로컬 캐시) */
    QRect m_cursorRect;              /* 커서 위치 (글로벌 좌표) */
};
```

> [!NOTE]
> **단일 `QPlatformInputContext` 인스턴스**가 전체 앱에서 공유됩니다.
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

### 4.2 한자 팝업 키 처리 (팝업 활성 시)

한자 팝업이 활성 상태(`m_hanjaPopup->isVisible()`)일 때, **모든 키 입력은 먼저 팝업에 전달**됩니다.

#### 4.2.1 Escape → 조합 복원 + 팝업 닫기

```
Escape 키 입력
  → 1. processKey(0,0,0) — 엔진 리셋 (더미키)
       → 커밋 텍스트가 있으면 commitString()
  → 2. cancelHanja() — 한자 모드 해제
  → 3. m_composing = isComposing() 갱신
  → 4. updatePreedit() (preedit 복원)
  → 5. hidePopup()
  → return true (키 소비)
```

#### 4.2.2 팝업 내부 처리 (`handleKey`)

| 동작 | 트리거 키 | 결과 |
|------|-----------|------|
| **숫자 선택** | `1`-`9` | 해당 인덱스 한자 선택 → 콜백 호출 |
| **Enter 선택** | `Return`, `Enter` | 현재 선택된 한자 확정 → 콜백 호출 |
| **이전 페이지** | `←`, `PageUp`, `Backspace` | 페이지 이동 + 리스트 갱신 |
| **다음 페이지** | `→`, `PageDown`, `Space` | 페이지 이동 + 리스트 갱신 |
| **선택 이동** | `↑`, `↓` | 선택 인덱스 변경 + 리스트 갱신 |
| **모디파이어** | Shift, Ctrl, Alt 등 | 소비 (팝업 유지) |

#### 4.2.3 한자 선택 콜백 (람다)

```
숫자/Enter 선택 → 콜백 호출
  → 1. cancelHanja() (preedit 클리어)
  → 2. m_composing = false
  → 3. updatePreedit() (preedit 비움)
  → 4. commitString(hanja) (선택된 한자 커밋)
```

#### 4.2.4 미지원 키 → fall-through 방식

```
문자 키 등 → handleKey() returns false
  → 1. focusOut() → 조합 중 한글 커밋 (예: "한" 커밋)
  → 2. m_composing = false + updatePreedit() (preedit 클리어)
  → 3. cancelHanja() + hidePopup()
  → 4. focusIn(m_windowId) (컨텍스트 복원)
  → 5. fall-through → 아래 processKey 경로에서 엔진이 새 키 처리
```

> [!IMPORTANT]
> `return false`가 아닌 **fall-through** 사용.
> `return false`는 raw keysym을 앱에 직접 전달하여 엔진을 우회합니다.
> fall-through는 키를 정상적인 `processKey` DBus 경로로 전달하여 언어 상태에 따른 올바른 입력을 보장합니다.

### 4.3 한자 키 처리 (`F9` / `Hangul_Hanja`)

한자 팝업이 **닫혀있을 때** F9 키 입력 시:

```
F9 또는 Hangul_Hanja 입력
  → 1. DBus getHanjaCandidates(target, candidates)
       (엔진의 start_hanja_conversion() 트리거)
  → 한자 후보가 있으면:
     1. 커서 위치 계산 (m_cursorRect 사용, 이미 글로벌 좌표)
     2. showPopup(target, candidates, x, y, height, 선택 콜백)
  → 한자 후보가 없으면:
     → 2. DBus getSpecialCharCandidates(target, chars, topRow)
          (엔진이 이미 special_char_mode를 설정한 상태)
     → 특수문자 후보가 있으면:
        1. 커서 위치 계산
        2. showPopup(target, chars, topRow, x, y, height, 선택 콜백)
     → 특수문자 후보도 없으면:
        로그 출력, 아무 동작 없음
  → return true (키 소비)
```

> [!IMPORTANT]
> **호출 순서가 중요합니다.** `getHanjaCandidates()`를 반드시 먼저 호출해야 합니다.
> 이 호출이 엔진의 `start_hanja_conversion()`을 트리거하여 한자/특수문자 모드를 설정합니다.
> `getSpecialCharCandidates()`는 이미 설정된 모드 상태만 읽으므로, 순서가 바뀌면 첫 번째 키 입력에서 후보가 표시되지 않습니다.
> 이 순서는 GTK3/4 구현과 동일합니다.

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

```
setFocusObject(newObject) 호출
  → 1. 한자 팝업 열려있으면 닫기 + cancelHanja()
  → 2. 이전 포커스에서 조합 중이었으면:
       → focusOut() → 조합 커밋
       → m_composing = false + updatePreedit()
  → 3. m_focusObject = newObject
  → 4. newObject가 있으면 focusIn(m_windowId)
```

### 5.1 리셋 (`reset`)

```
reset() 호출
  → 1. DBus reset() → 조합 중 텍스트 커밋
  → 2. m_composing = false + updatePreedit()
  → 3. 한자 팝업 열려있으면 닫기 + cancelHanja()
```

### 5.2 커밋 (`commit`)

```
commit() 호출 (조합 중일 때만)
  → 1. DBus reset() → 조합 중 텍스트 커밋
  → 2. m_composing = false + updatePreedit()
  → 3. 한자 팝업 열려있으면 닫기 + cancelHanja()
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
| `CommitText(s)` | `setCommitTextCallback()` | standalone 한자/특수문자 팝업의 마우스 클릭 커밋 |

> [!NOTE]
> `FocusOut`은 (552b5bd 이후) `CommitText` 시그널을 동반하지 않고 **RPC 반환값만** 커밋 채널로 사용합니다.
> 과거 시그널은 브로드캐스트라 컨텍스트 비한정이라 중복 커밋(예: `늘` 두 번)을 일으켰습니다.
> Config 조회/변경은 `GetConfigYaml`/`SetConfigYaml`/`ConfigChangedJson` (modern) 및 legacy `GetConfig`/`SetConfig`/`ConfigChanged`가 병존합니다 — `unim-dbus/SPEC.md` 참조. Qt 플러그인은 설정 변경을 실시간 구독하지 않습니다 (재시작 또는 설정 GUI 경유).

---

## 9. 한자 팝업 윈도우 (`UnimHanjaPopup`)

### 9.1 윈도우 속성

- `Qt::ToolTip | Qt::FramelessWindowHint | Qt::WindowStaysOnTopHint`
- 다크 테마 스타일시트 (배경 `#2d2d2d`, 선택 `#4a90d9`)
- QVBoxLayout + QLabel 기반 후보 목록

### 9.2 레이아웃

```
┌─────────────────────────────┐
│ 1. 韓  [한]                 │  ← QLabel (selected=true 시 파란 배경)
│ 2. 漢  [한]                 │
│ 3. 限  [한]                 │
│ ...                         │
│ 9. 翰  [한]                 │
│ ← 1/3 →                    │  ← 페이지 QLabel
└─────────────────────────────┘
```

### 9.3 페이지네이션

- 페이지당 최대 9개 후보 (`MAX_VISIBLE_CANDIDATES = 9`)
- `→`/`Space`/`PageDown`: 다음 페이지
- `←`/`BackSpace`/`PageUp`: 이전 페이지
- `↑`/`↓`: 현재 페이지 내 선택 이동

---

## 10. 특수문자 팝업 윈도우 (`UnimSpecialPopup`)

### 10.1 개요

한자 후보가 없을 때, 조합 중인 자모에 매핑된 특수문자를 **9×9 그리드 팝업**으로 표시합니다.
한자 키(F9)로 트리거되며, 한자 후보가 없으면 자동으로 특수문자 모드로 전환됩니다.

> [!NOTE]
> 구현은 `qt-common/src/unim_special_popup.cpp`에 위치하며, Qt5/Qt6 공통 코드입니다.

### 10.2 윈도우 속성

- `Qt::ToolTip | Qt::FramelessWindowHint | Qt::WindowStaysOnTopHint | Qt::WindowDoesNotAcceptFocus`
- 다크 테마 스타일시트 (배경 `#2d2d2d`)
- QGridLayout 기반 문자 배치 (최대 9열 × 9행)
- 선택/하이라이트 색상 Qt 네이티브 팔레트 사용

### 10.3 레이아웃

```text
┌──────────────────────────────────────┐
│      Q    W    E    R    T    ...    │  ← top_row 열 헤더
│ 1    $    %    ₩    °F   ‰    ...    │  ← 행 1
│ 2    ...                             │  ← 행 2
│ ...                                  │
│ 9    ...                             │  ← 행 9
│               ← 1/3 →               │  ← 페이지 라벨 (2페이지 이상 시만 표시)
└──────────────────────────────────────┘
```

### 10.4 키 처리

`input_context.cpp`에서 팝업이 보이는 동안 모든 키를 먼저 팝업에게 전달합니다.

| 동작 | 트리거 키 | 결과 |
|------|-----------|------|
| 열 점프 | `Q`~`O` (물리 키) | 해당 열로 이동 |
| 숫자 선택 (행) | `1`-`9` | 선택된 열의 해당 행 문자 커밋 |
| 방향키 이동 | `↑`/`↓`/`←`/`→` | 셀 선택 이동 (경계에서 순환) |
| Enter 확정 | `Return`/`KP_Enter` | 현재 선택 셀의 문자 커밋 |
| 다음 페이지 | `Page_Down`/`Space` | 다음 페이지 |
| 이전 페이지 | `Page_Up` | 이전 페이지 |
| Escape | `Escape` | 조합 중 자모 커밋 + 특수문자 모드 취소 + 팝업 닫기 |
| 마우스 클릭 | 좌클릭 | 클릭한 셀의 문자 커밋 |

> [!IMPORTANT]
> **열 점프는 물리 키 위치(QWERTY) 기준**으로 매칭합니다.
> `top_row` 문자열은 **표시 전용**이고, 키 매칭은 항상 `"qwertyuio"` 물리 키로 수행합니다.

### 10.5 시각적 피드백

| 스타일 | 용도 |
|---|---|
| 선택 셀 배경 (`#4a90d9`) | 현재 선택된 셀 하이라이트 |
| 열 하이라이트 | 선택된 열의 모든 셀 배경 |
| 행 하이라이트 | 선택된 행의 모든 셀 배경 |
| 헤더 강조 | 선택된 열/행의 헤더 |
| 플래시 효과 | 문자 선택 시 120ms 깜빡임 |

---

## 11. Qt6 vs Qt5 차이점 요약

| 관점 | Qt5 | Qt6 |
|------|-----|-----|
| **CMake 빌드** | `add_library(unim MODULE ...)` | `qt_add_plugin(unim ...)` |
| **Private 헤더** | `find_package(Qt5Gui ... Private)` + 수동 경로 | `Qt6::GuiPrivate` 자동 처리 |
| **생성자** | 부모 호출 생략 | `QPlatformInputContext()` 명시적 호출 |
| **클라이언트 ID** | `"qt5-unim"` / `"qt5-ctx-0x..."` | `"qt6-unim"` / `"qt6-ctx-0x..."` |
| **로그 모듈명** | `QT5_IM` | `QT6_IM` |
| **`qsizetype`** | `int` | `long long` (format `%d` → `%lld` 경고) |

> [!NOTE]
> 동작 로직 (키 처리, 포커스 관리, Preedit, 한자 팝업)은 **Qt5와 완전히 동일**합니다.
> 차이는 빌드 시스템과 Qt6 API 호환성에 한정됩니다.

---

## 12. QPlatformInputContext 인터페이스

| 메서드 | 구현 |
|--------|------|
| `isValid()` | DBus 연결 유효성 반환 |
| `reset()` | 조합 커밋 + 팝업 닫기 |
| `commit()` | 조합 중이면 커밋 + 팝업 닫기 |
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

## 13. 빌드 및 배포

### 13.1 빌드

```bash
mkdir -p unim-frontends/qt6/build
cd unim-frontends/qt6/build
cmake ..
make
```

또는 프로젝트 루트에서:

```bash
make build-frontends
```

### 13.2 개발 배포 (`make dev-qt6`)

```bash
make dev-qt6 PREFIX=/usr
```

### 13.3 설치 경로

```
$(QT6_PLUGINS_DIR)/platforminputcontexts/libunim.so
```

일반적으로: `/usr/lib/x86_64-linux-gnu/qt6/plugins/platforminputcontexts/libunim.so`

### 13.4 로컬 테스트

빌드 후 자동으로 `build/platforminputcontexts/libunim.so`에 복사됩니다:

```bash
QT_PLUGIN_PATH=$PWD/build QT_IM_MODULE=unim your-qt6-app
```

---

## 14. 로깅

`UNIM_DEVELOP=1` 환경변수 설정 시 활성화.

| 모듈명 | 컴포넌트 |
|--------|---------|
| `QT6_IM` | `input_context.cpp` (키 처리, 포커스, preedit) |
| `QT6_DBUS` | `unim_dbus_client.cpp` (DBus 통신) |
| `HANJA_POPUP` | `unim_hanja_popup.cpp` (한자 팝업) |

로그 포맷:
```
[YYYY/MM/DD HH:MM:SS] - [QT6_IM] - 메시지
```

출력 대상:
- 콘솔 (`qDebug`)
- 파일 (`~/.unim-errors.log`)
