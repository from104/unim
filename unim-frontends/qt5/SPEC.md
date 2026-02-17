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
| `unim_hanja_popup.cpp` | `qt-common/src/` | Qt 기반 한자 후보 팝업 윈도우 (Qt5/6 공용) |
| `unim_hanja_popup.hpp` | `qt-common/include/` | 한자 팝업 API 헤더 |

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
| `Qt5::Widgets` | 한자 팝업 UI |

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
4. `UnimHanjaPopup()` → 한자 팝업 인스턴스 생성
5. 상태 필드 초기화 (focusObject, composing, cursorRect)

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
    QString m_windowId;              /* "qt5-ctx-0x..." */
    bool m_composing;                /* 조합 중 여부 (로컬 캐시) */
    QRect m_cursorRect;              /* 커서 위치 (글로벌 좌표) */
};
```

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
  → DBus getHanjaCandidates(target, candidates)
  → 후보가 있으면:
    1. 커서 위치 계산 (m_cursorRect 사용, 이미 글로벌 좌표)
    2. showPopup(target, candidates, x, y, height, 선택 콜백)
  → 후보가 없으면:
    로그 출력, 아무 동작 없음
  → return true (키 소비)
```

> [!NOTE]
> GTK3/4에서는 위젯 로컬 → 글로벌 좌표 변환이 필요하지만,
> Qt5에서는 `update()` 시 `mapToGlobal()`로 미리 변환하여
> `m_cursorRect`에 **글로벌 좌표**를 저장합니다.

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
| `focusOut()` | `FocusOut` | `QString` | 포커스 상실 → 조합 커밋 |
| `processKey(keyval, keycode, state)` | `ProcessKey` | `UnimDbusKeyResult` | 키 입력 처리 |
| `reset()` | `ResetContext` | `QString` | 상태 초기화 → 조합 커밋 |
| `getPreedit()` | (캐시 조회) | `QString` | 현재 preedit 문자열 |
| `isComposing()` | (캐시 조회) | `bool` | 조합 중 여부 |
| `getHanjaCandidates(...)` | `GetHanjaCandidates` | `target, candidates[]` | 한자 후보 조회 |
| `selectHanja(index, ...)` | `SelectHanja` | `selectedHanja` | 한자 후보 선택 |
| `cancelHanja()` | `CancelHanja` | — | 한자 모드 취소 |

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
| **팝업 UI** | GtkWindow + GtkListBox | QWidget + QLabel + QVBoxLayout |

---

## 11. QPlatformInputContext 인터페이스

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

## 12. 빌드 및 배포

### 12.1 빌드

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

### 12.2 개발 배포 (`make dev-qt5`)

```bash
make dev-qt5 PREFIX=/usr
```

### 12.3 설치 경로

```
$(QT5_PLUGINS_DIR)/platforminputcontexts/libunim.so
```

일반적으로: `/usr/lib/x86_64-linux-gnu/qt5/plugins/platforminputcontexts/libunim.so`

### 12.4 로컬 테스트

빌드 후 자동으로 `build/platforminputcontexts/libunim.so`에 복사됩니다:

```bash
QT_PLUGIN_PATH=$PWD/build QT_IM_MODULE=unim your-qt5-app
```

---

## 13. 로깅

`UNIM_DEVELOP=1` 환경변수 설정 시 활성화.

| 모듈명 | 컴포넌트 |
|--------|---------|
| `QT5_IM` | `input_context.cpp` (키 처리, 포커스, preedit) |
| `QT5_DBUS` | `unim_dbus_client.cpp` (DBus 통신) |
| `HANJA_POPUP` | `unim_hanja_popup.cpp` (한자 팝업) |

로그 포맷:
```
[YYYY/MM/DD HH:MM:SS] - [QT5_IM] - 메시지
```

출력 대상:
- 콘솔 (`qDebug`)
- 파일 (`~/.unim-errors.log`)
