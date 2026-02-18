# UNIM GTK4 프론트엔드 세부 기능 명세

> GTK4 애플리케이션에서 한글 입력을 제공하는 IM(Input Method) 모듈의 상세 동작을 정의합니다.

---

## 1. 아키텍처 개요

### 1.1 컴포넌트 구성

| 파일 | 위치 | 역할 |
|------|------|------|
| `immodule.c` | `gtk4/src/` | GTK4 IM 모듈 메인 (GtkIMContext 구현) |
| `unim_dbus_client.c` | `gtk-common/src/` | GDBus 기반 unim-daemon 통신 (GTK3/4 공용) |
| `unim_dbus_client.h` | `gtk-common/include/` | DBus 클라이언트 API 헤더 |
| `unim_hanja_popup.c` | `gtk-common/src/` | GTK 기반 한자 후보 팝업 윈도우 (GTK3/4 공용) |
| `unim_hanja_popup.h` | `gtk-common/include/` | 한자 팝업 API 헤더 |
| `unim_special_popup.c` | `gtk-common/src/` | GTK 기반 특수문자 그리드 팝업 윈도우 (GTK3/4 공용) |
| `unim_special_popup.h` | `gtk-common/include/` | 특수문자 팝업 API 헤더 |

### 1.2 통신 구조

```
┌────────────────────┐   GtkIMContext API   ┌──────────────┐   GDBus    ┌──────────────┐
│  GTK4 애플리케이션 │ ←────────────────→ │  libim-unim  │ ←──────→ │  unim-daemon │
│ (gnome-text-editor)│ (filter_keypress 등)  │  (.so 모듈)  │  (동기)   │  (입력 엔진) │
└────────────────────┘                      └──────────────┘           └──────────────┘
```

### 1.3 주요 의존성

| 라이브러리 | 용도 |
|-----------|------|
| `gtk4` | GTK4 IM 프레임워크 (GtkIMContext) |
| `gio-2.0` | GDBus 통신 + GIO 모듈 등록 |
| `gtk4-x11` (선택) | X11 환경에서 한자 팝업 절대 좌표 계산 |
| `x11` (선택) | XTranslateCoordinates |

---

## 2. IM 모듈 수명주기

### 2.1 모듈 등록 (GIO Extension Point)

GTK4는 GTK3의 `im_module_*` 진입점 대신 **GIO Extension Point** 방식을 사용합니다.

| 엔트리 포인트 | 동작 |
|--------------|------|
| `g_io_module_load(GIOModule*)` | `UnimIMContext` GType 등록 + Extension Point 구현 |
| `g_io_module_unload(GIOModule*)` | (정리 작업 없음) |
| `g_io_module_query()` | `"gtk-im-module"` 반환 |

등록 코드:

```c
g_io_extension_point_implement(
    GTK_IM_MODULE_EXTENSION_POINT_NAME,  // "gtk-im-module"
    UNIM_TYPE_IM_CONTEXT,
    "unim",
    10  /* priority */
);
```

> [!NOTE]
> GTK3는 `im_module_init` / `im_module_list` / `im_module_create` 방식이었으나,
> GTK4는 GIO 모듈 시스템으로 변경되어 `g_io_module_*` 함수를 사용합니다.

### 2.2 타입 선언

GTK4는 `G_DECLARE_FINAL_TYPE` 매크로를 사용:

```c
G_DECLARE_FINAL_TYPE(UnimIMContext, unim_im_context, UNIM, IM_CONTEXT, GtkIMContext)
```

GTK3의 수동 `G_TYPE_CHECK_INSTANCE_CAST` 매크로 대신 자동 생성됩니다.

### 2.3 컨텍스트 초기화 (`unim_im_context_init`)

1. `UNIM_DEVELOP=1` 여부 확인 → 디버그 모드 설정
2. `window_id` 생성: `"gtk4-ctx-0x..."` (컨텍스트 포인터 기반)
3. `unim_dbus_context_new("gtk4-unim", window_id)` → DBus 클라이언트 생성
4. `unim_hanja_popup_new()` → 한자 팝업 인스턴스 생성
5. `unim_special_popup_new()` → 특수문자 팝업 인스턴스 생성
6. 상태 필드 초기화 (focused, surrounding_text, cursor_area 등)

### 2.4 컨텍스트 소멸 (`unim_im_context_dispose`)

GTK4는 `finalize` 대신 **`dispose`** 를 사용합니다 (부모 클래스 호환성):

1. 한자 팝업 해제 (`unim_hanja_popup_free`)
2. 한자 후보 배열 해제 (`unim_hanja_candidates_free`)
3. DBus 클라이언트 해제 (`unim_dbus_context_free`)
4. `window_id`, `surrounding_text` 메모리 해제
5. 부모 클래스 `dispose` 호출

---

## 3. 컨텍스트 상태 (`UnimIMContext`)

```c
struct _UnimIMContext {
    GtkIMContext parent;
    UnimDbusContext *dbus_ctx;     /* DBus 클라이언트 */
    gboolean is_focused;
    gchar *window_id;              /* "gtk4-ctx-0x..." */
    GtkWidget *client_widget;      /* 입력 위젯 참조 (좌표 변환용) */

    /* 주변 텍스트 캐시 */
    gchar *surrounding_text;
    gint cursor_index;             /* 바이트 오프셋 */
    gint selection_index;          /* 바이트 오프셋 */

    /* 한자 변환 */
    UnimHanjaPopup *hanja_popup;
    UnimHanjaCandidate *hanja_candidates;
    gsize hanja_count;
    GdkRectangle cursor_area;      /* 커서 위치 (위젯 로컬 좌표) */

    /* 특수문자 입력 */
    UnimSpecialPopup *special_popup;

    /* 한자/특수문자 키 설정 캐시 */
    guint *hanja_keysyms;              /* 설정 기반 한자키 keysym 배열 */
    gsize n_hanja_keysyms;             /* 배열 크기 */
};
```

> [!NOTE]
> `hanja_keysyms`는 초기화 시 DBus `GetConfig("hanja_keys")` 호출로 설정을 로드하고,
> `unim_keycode_name_to_gdk_keyval()` 함수로 GDK keyval 배열로 변환하여 캐시합니다.
> 설정 로드 실패 시 기본값(`Hangul_Hanja`, `F9`)이 사용됩니다.

> [!NOTE]
> GTK3에서는 `GdkWindow *client_window`를 사용하지만,
> GTK4에서는 **`GtkWidget *client_widget`** 을 사용합니다 (GTK4에서 GdkWindow 폐지).

---

## 4. 키 입력 처리 (`filter_keypress`)

### 4.1 키 이벤트 인터페이스 차이

GTK4에서는 키 이벤트가 `GdkEventKey*` → **`GdkEvent*`** 로 변경되었습니다.
필드 직접 접근 대신 **접근자 함수**를 사용합니다:

```c
/* GTK3 */
event->keyval, event->hardware_keycode, event->state

/* GTK4 (4.4+) */
gdk_key_event_get_keyval(event)
gdk_key_event_get_keycode(event)
gdk_event_get_modifier_state(event)
```

이벤트 타입 확인:

```c
GdkEventType event_type = gdk_event_get_event_type(event);
```

### 4.2 전처리

```
이벤트 수신
  → 1. DBus 컨텍스트 확인 (없으면 return FALSE)
  → 2. 이벤트 타입 확인 (KEY_PRESS/KEY_RELEASE만 처리)
  → 3. KeyRelease 무시
  → 4. 한자 키 확인 (설정 기반: hanja_keysyms 배열 비교)
  → 5. 키 정보 추출 (GDK 4.4+ 접근자)
  → 6. 수정자 키 바이패스 → return FALSE (앱에 전달)
```

**바이패스 대상 수정자 키:**

- Shift_L/R, Control_L/R, Alt_L/R
- Super_L/R, Meta_L/R, ISO_Level3_Shift

### 4.3 한자 팝업 키 처리 (팝업 활성 시)

한자 팝업이 활성 상태(`unim_hanja_popup_is_visible`)일 때, **모든 키 입력은 먼저 팝업에 전달**됩니다.

#### 4.3.1 Escape → 조합 복원 + 팝업 닫기

```
Escape 키 입력
  → 1. ProcessKey(0,0,0) — 엔진 리셋 (더미키)
       → 커밋 텍스트가 있으면 커밋
  → 2. CancelHanja — 한자 모드 해제
  → 3. preedit-changed 시그널 (preedit 복원)
  → 4. 팝업 닫기
  → return TRUE (키 소비)
```

#### 4.3.2 팝업 내부 처리 (`unim_hanja_popup_handle_key`)

| 동작 | 트리거 키 | 결과 |
|------|-----------|------|
| **숫자 선택** | `1`-`9` | 해당 인덱스 한자 선택 → 콜백 호출 |
| **Enter 선택** | `Return`, `KP_Enter` | 현재 선택된 한자 확정 → 콜백 호출 |
| **이전 페이지** | `←`, `PageUp`, `BackSpace` | 페이지 이동 + 리스트 갱신 |
| **다음 페이지** | `→`, `PageDown`, `Space` | 페이지 이동 + 리스트 갱신 |
| **선택 이동** | `↑`, `↓` | 선택 인덱스 변경 + 리스트 갱신 |
| **모디파이어** | Shift, Ctrl, Alt 등 | 소비 (팝업 유지) |

#### 4.3.3 한자 선택 콜백 (`on_hanja_selected`)

```
숫자/Enter 선택 → 콜백 호출
  → 1. 팝업 숨기기
  → 2. CancelHanja (preedit 클리어)
  → 3. preedit-changed 시그널
  → 4. commit 시그널 (선택된 한자)
```

#### 4.3.4 미지원 키 → fall-through 방식

```
문자 키 등 → handle_key() returns FALSE
  → 1. FocusOut → 조합 중 한글 커밋 (예: "한" 커밋)
  → 2. preedit-changed (preedit 클리어)
  → 3. CancelHanja + 팝업 닫기
  → 4. FocusIn(window_id) (컨텍스트 복원)
  → 5. fall-through → 아래 ProcessKey 경로에서 엔진이 새 키 처리
```

> [!IMPORTANT]
> `return FALSE`가 아닌 **fall-through** 사용.
> `return FALSE`는 raw keysym을 앱에 직접 전달하여 엔진을 우회합니다.
> fall-through는 키를 정상적인 `ProcessKey` DBus 경로로 전달하여 언어 상태에 따른 올바른 입력을 보장합니다.

### 4.4 한자 키 처리 (`F9` / `Hangul_Hanja`)

한자 팝업이 **닫혀있을 때** F9 키 입력 시:

```
F9 (0xffc6) 또는 Hangul_Hanja (0xff34) 입력
  → DBus GetHanjaCandidates
  → 후보가 있으면:
    1. 이전 후보 배열 정리
    2. 좌표 변환 (위젯 로컬 → 루트 위젯 → X11 절대)
    3. unim_hanja_popup_show(popup, target, candidates, count, x, y, h, callback, unim)
  → 후보가 없으면:
    로그 출력, 아무 동작 없음
  → return TRUE (키 소비)
```

#### 4.4.1 좌표 변환 (GTK4 고유)

GTK4에서는 GTK3의 `gdk_window_get_origin` 대신 **2단계 좌표 변환**을 수행합니다:

```
[1단계] 위젯 로컬 → 루트 위젯 (graphene_point)
    GtkWidget *root = gtk_widget_get_root(client_widget);
    gtk_widget_compute_point(client_widget, root, &p_in, &p_out);

[2단계] X11: GdkSurface → 화면 절대 좌표
    GtkNative *native = gtk_widget_get_native(client_widget);
    GdkSurface *surface = gtk_native_get_surface(native);
    XTranslateCoordinates(xdisplay, xwindow,
        DefaultRootWindow(xdisplay), 0, 0, &abs_x, &abs_y, &child_return);
    popup_x += abs_x;
    popup_y += abs_y;
```

> [!NOTE]
> GTK3에서는 `gdk_window_get_origin()` 한 번으로 절대 좌표를 얻지만,
> GTK4에서는 위젯→루트 변환과 Surface→X11 변환을 분리하여 수행합니다.

### 4.5 비조합 시 특수키 바이패스

조합 상태가 아닐 때(`!unim_dbus_is_composing`), 다음 키들은 엔진을 거치지 않고 앱에 직접 전달:

| 키 그룹 | 키 범위 | 비고 |
|---------|---------|------|
| 기능키 | F1~F12 | F9 제외 (한자키로 위에서 처리) |
| 방향키 | Left, Up, Right, Down | |
| 네비게이션 | Home, End, PageUp, PageDown, Insert, Delete | |
| Escape | 조합 중이 아니면 앱으로 | |

> [!NOTE]
> 조합 **중**일 때는 이 키들도 엔진(`ProcessKey`)으로 전달됩니다.

### 4.6 일반 키 처리 (ProcessKey)

```
키 입력 → 수정자 상태 변환 (GdkModifierType → 비트필드)
       → evdev 코드 변환 (keycode - 8)
       → DBus ProcessKey(keyval, evdev_code, mod_state)
       → 응답: UnimDbusKeyResult { consumed, preedit, commit }
```

#### 4.6.1 수정자 상태 비트필드 변환

| GDK 마스크 | 비트 | 의미 |
|-----------|------|------|
| `GDK_SHIFT_MASK` | bit 0 | Shift |
| `GDK_LOCK_MASK` | bit 1 | CapsLock |
| `GDK_CONTROL_MASK` | bit 2 | Ctrl |
| `GDK_ALT_MASK` | bit 3 | Alt |
| `GDK_SUPER_MASK` | bit 26 | Super |

> [!NOTE]
> GTK3에서는 `GDK_MOD1_MASK` (Alt), GTK4에서는 **`GDK_ALT_MASK`** 를 사용합니다.

#### 4.6.2 결과 처리

```
result.consumed == TRUE:
  → 1. 선택 영역 삭제 (retrieve-surrounding → delete_surrounding)
  → 2. commit 텍스트 커밋  (commit 시그널)
  → 3. preedit-changed 시그널
  → return TRUE

result.consumed == FALSE:
  → return FALSE (앱에 키 바이패스)
```

### 4.7 선택 영역 자동 삭제

키가 엔진에 의해 소비된 경우, 선택 영역이 있으면 자동 삭제:

```
retrieve-surrounding 시그널 → 최신 주변 텍스트 획득
  → cursor_index != selection_index (선택 영역 존재)
    → 바이트 오프셋 → 문자 오프셋 변환
    → gtk_im_context_delete_surrounding(context, offset, length)
    → 캐시 무효화
```

> [!NOTE]
> GTK3에서는 `g_signal_emit_by_name(context, "delete-surrounding", ...)` 사용,
> GTK4에서는 **`gtk_im_context_delete_surrounding()`** API 직접 호출합니다.

---

## 5. 포커스 관리

### 5.1 포커스 획득 (`focus_in`)

```
GTK focus_in 호출
  → DBus FocusIn(window_id)
  → is_focused = TRUE
```

### 5.2 포커스 상실 (`focus_out`)

```
GTK focus_out 호출
  → 1. DBus FocusOut → 조합 중 텍스트 커밋
       → commit 시그널 (커밋할 텍스트가 있으면)
       → preedit-changed 시그널
  → 2. 한자 팝업 열려있으면 닫기 + CancelHanja
  → 3. 특수문자 팝업 열려있으면 닫기 + CancelSpecialChar
  → is_focused = FALSE
```

### 5.3 리셋 (`reset`)

```
GTK reset 호출
  → 1. DBus ResetContext → 조합 중 텍스트 커밋
       → commit 시그널 (커밋할 텍스트가 있으면)
       → preedit-changed 시그널
  → 2. 한자 팝업 열려있으면 닫기 + CancelHanja
```

---

## 6. Preedit (조합 문자) 표시

### 6.1 preedit 문자열 조회 (`get_preedit_string`)

```
GTK get_preedit_string 호출
  → DBus get_preedit() → 현재 조합 문자열
  → PangoAttrList 생성:
     → 텍스트가 있으면 PANGO_UNDERLINE_SINGLE 속성 추가
  → cursor_pos = 문자 수 (UTF-8 문자 단위)
```

### 6.2 커서 위치 업데이트 (`set_cursor_location`)

```
GTK set_cursor_location(GdkRectangle *area) 호출
  → cursor_area에 저장 (위젯 로컬 좌표, 팝업 위치 계산에 사용)
```

---

## 7. 위젯 및 주변 텍스트

### 7.1 클라이언트 위젯 설정 (`set_client_widget`)

GTK4 고유 API. GTK3의 `set_client_window(GdkWindow*)` 대신 사용:

```
GTK set_client_widget(GtkWidget *widget) 호출
  → client_widget에 저장 (좌표 변환에 사용)
```

### 7.2 주변 텍스트 — 선택 영역 포함 (`set_surrounding_with_selection`)

GTK4에서는 `set_surrounding_with_selection`이 추가되어 **선택 영역 정보를 직접 제공**합니다:

```c
/* GTK4 — anchor(selection) 정보 포함 */
set_surrounding_with_selection(text, len, cursor_index, selection_index)
  → surrounding_text 저장
  → cursor_index 저장
  → selection_index 저장
```

`set_surrounding`은 내부적으로 `set_surrounding_with_selection`을 `selection_index = cursor_index`로 호출합니다.

> [!NOTE]
> GTK3에서는 `set_surrounding`만 존재하고, anchor 정보가 없어
> `selection_index = cursor_index`로 동일하게 설정합니다.
> GTK4에서는 정확한 선택 범위를 알 수 있어 선택 영역 삭제가 더 정확합니다.

---

## 8. DBus 통신 (`unim_dbus_client`)

### 8.1 연결 정보

| 항목 | 값 |
|------|-----|
| 서비스 | `org.atit.unim.InputMethod` |
| 경로 | `/org/atit/unim/InputMethod` |
| 인터페이스 | `org.atit.unim.InputMethod` / `org.atit.unim.InputContext` |
| 타임아웃 | 500ms |
| 통신 방식 | GDBus 동기 호출 |

### 8.2 주요 DBus 메서드

| 함수 | DBus 메서드 | 반환 | 용도 |
|------|------------|------|------|
| `unim_dbus_context_new` | `CreateContext` | `context_path` | 컨텍스트 등록 |
| `unim_dbus_context_free` | `DestroyContext` | — | 컨텍스트 해제 |
| `unim_dbus_focus_in` | `FocusIn` | — | 포커스 획득 알림 |
| `unim_dbus_focus_out` | `FocusOut` | `commit` | 포커스 상실 → 조합 커밋 |
| `unim_dbus_process_key` | `ProcessKey` | `{consumed, preedit, commit}` | 키 입력 처리 |
| `unim_dbus_reset` | `ResetContext` | `commit` | 상태 초기화 → 조합 커밋 |
| `unim_dbus_get_preedit` | (캐시 조회) | `preedit` | 현재 preedit 문자열 |
| `unim_dbus_is_composing` | (캐시 조회) | `bool` | 조합 중 여부 |
| `unim_dbus_get_hanja_candidates` | `GetHanjaCandidates` | `{target, candidates[], count}` | 한자 후보 조회 |
| `unim_dbus_select_hanja` | `SelectHanja` | `selected_hanja` | 한자 후보 선택 |
| `unim_dbus_cancel_hanja` | `CancelHanja` | — | 한자 모드 취소 |
| `unim_dbus_get_special_char_candidates` | `GetSpecialCharCandidates` | `{target, chars[], count, top_row}` | 특수문자 후보 조회 |
| `unim_dbus_select_special_char` | `SelectSpecialChar` | — | 특수문자 선택 → 커밋 |
| `unim_dbus_cancel_special_char` | `CancelSpecialChar` | — | 특수문자 모드 취소 |

---

## 9. 한자 팝업 윈도우 (`unim_hanja_popup`)

### 9.1 윈도우 속성

- GTK Window (`GTK_WINDOW_POPUP` 타입)
- `override_redirect = True` (X11에서 WM 데코레이션 제거)
- GtkListBox 기반 후보 목록
- 페이지 네비게이션 라벨

### 9.2 레이아웃

```
┌─────────────────────────────┐
│ 1. 韓  [한]                 │  ← 후보 항목 (번호. 한자  [원래 한글])
│ 2. 漢  [한]                 │
│ 3. 限  [한]                 │
│ ...                         │
│ 9. 翰  [한]                 │
│ ← 1/3 →                    │  ← 페이지 라벨
└─────────────────────────────┘
```

### 9.3 페이지네이션

- 페이지당 최대 9개 후보 (`MAX_VISIBLE_CANDIDATES = 9`)
- `→`/`Space`/`PageDown`: 다음 페이지
- `←`/`BackSpace`/`PageUp`: 이전 페이지
- `↑`/`↓`: 현재 페이지 내 선택 이동

---

## 10. 특수문자 팝업 윈도우 (`unim_special_popup`)

### 10.1 개요

한자 후보가 없을 때, 조합 중인 자모에 매핑된 특수문자를 **9×9 그리드 팝업**으로 표시합니다.
한자 키(F9)로 트리거되며, 한자 후보가 없으면 자동으로 특수문자 모드로 전환됩니다.

### 10.2 윈도우 속성

- GTK Window (`GTK_WINDOW_POPUP` 타입)
- `override_redirect = True` (X11에서 WM 데코레이션 제거)
- `can_focus = FALSE` (부모 앱의 포커스 유지)
- GtkGrid 기반 문자 배치 (최대 9열 × 9행)
- CSS 커스텀 스타일링 (`unim-special-popup` 클래스)

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

immodule.c에서 팝업이 보이는 동안 모든 키를 먼저 팝업에게 전달합니다.

| 동작 | 트리거 키 | 결과 |
|------|-----------|------|
| 숫자 선택 (행) | `1`-`9` | 선택된 열의 해당 행 문자 커밋 |
| 방향키 이동 | `↑`/`↓`/`←`/`→` | 셀 선택 이동 (경계에서 순환) |
| Enter 확정 | `Return`/`KP_Enter` | 현재 선택 셀의 문자 커밋 |
| 다음 페이지 | `Tab` | 다음 페이지 (순환) |
| 이전 페이지 | `Shift+Tab` | 이전 페이지 (순환) |
| Escape | `Escape` | 조합 중 자모 커밋 + 특수문자 모드 취소 + 팝업 닫기 |
| 마우스 클릭 | 좌클릭 | 클릭한 셀의 문자 커밋 |

> [!IMPORTANT]
> **열 점프는 물리 키 위치(QWERTY) 기준으로 매칭합니다.**
> OS keyval은 항상 QWERTY 기반이고, UNIM 영문 키맵 변환은 엔진 내부에서 일어납니다.
> `top_row` 문자열은 **표시 전용** (드보락: `',.PYFGCR`, 콜맥: `QWFPGJLUY`)이고,
> 키 매칭은 항상 `"qwertyuio"` 물리 키로 수행합니다.

### 10.5 포커스 보존 패턴

X11에서 팝업이 부모 앱의 포커스를 빼앗지 않도록 하는 핵심 순서:

```text
1. gtk_widget_realize() — X11 윈도우 생성 (미표시)
2. XSetWindowAttributes.override_redirect = True — WM이 이 창 무시
3. XMoveWindow() — 정확한 위치 설정
4. gtk_widget_set_visible(TRUE) — 마지막에 표시
```

> [!IMPORTANT]
> 이 순서가 반대이면 (set_visible → override_redirect) WM이 일반 창으로 맵핑하여
> 부모 앱에 `focus_out`이 발생 → 팝업이 즉시 자동 닫힘되는 버그가 발생합니다.

### 10.6 시각적 피드백

| CSS 클래스 | 용도 |
|-----------|------|
| `cell-selected` | 현재 선택된 셀 하이라이트 |
| `cell-col-highlight` | 선택된 열의 모든 셀 배경 |
| `header-active` | 선택된 열의 헤더 강조 |
| `cell-flash` | 문자 선택 시 200ms 플래시 효과 |

---

## 11. GTK4 vs GTK3 차이점 요약

| 관점 | GTK3 | GTK4 |
|------|------|------|
| **모듈 등록** | `im_module_init/list/create` | `g_io_module_load/unload/query` |
| **타입 선언** | 수동 `_CAST` 매크로 | `G_DECLARE_FINAL_TYPE` |
| **소멸자** | `finalize` | `dispose` |
| **윈도우 참조** | `GdkWindow *client_window` | `GtkWidget *client_widget` |
| **키 이벤트** | `GdkEventKey *` (필드 직접 접근) | `GdkEvent *` (접근자 함수) |
| **Alt 마스크** | `GDK_MOD1_MASK` | `GDK_ALT_MASK` |
| **주변 텍스트** | `set_surrounding` only | `set_surrounding_with_selection` 추가 |
| **선택 삭제** | `g_signal_emit_by_name("delete-surrounding")` | `gtk_im_context_delete_surrounding()` |
| **좌표 변환** | `gdk_window_get_origin()` | `gtk_widget_compute_point()` + `XTranslateCoordinates` |
| **좌표 타입** | `gint` | `graphene_point_t` |

---

## 12. 빌드 및 배포

### 11.1 빌드

```bash
mkdir -p unim-frontends/gtk4/build
cd unim-frontends/gtk4/build
cmake ..
make
```

또는 프로젝트 루트에서:

```bash
make build-frontends
```

### 11.2 개발 배포 (`make dev-gtk4`)

```bash
make dev-gtk4 PREFIX=/usr
```

동작:

1. `cmake` + `make` (gtk4/build)
2. `sudo cp libim-unim.so $(GTK4_IM_MODULEDIR)/`
3. `sudo gio-querymodules $(GTK4_IM_MODULEDIR)/`

> [!IMPORTANT]
> GTK4에서는 모듈 설치 후 반드시 `gio-querymodules`를 실행해야 합니다.
> GTK3의 `gtk-query-immodules-3.0`과 유사하지만 GIO 기반입니다.

### 11.3 설치 경로

```
$(GTK4_LIBDIR)/gtk-4.0/4.0.0/immodules/libim-unim.so
```

일반적으로: `/usr/lib/x86_64-linux-gnu/gtk-4.0/4.0.0/immodules/libim-unim.so`

---

## 13. 로깅

`UNIM_DEVELOP=1` 환경변수 설정 시 활성화.

| 모듈명 | 컴포넌트 |
|--------|---------|
| `GTK4_IM` | `immodule.c` (키 처리, 포커스, preedit) |
| `GTK4_DBUS` | `unim_dbus_client.c` (DBus 통신) |
| `HANJA_POPUP` | `unim_hanja_popup.c` (한자 팝업) |

로그 포맷:

```
[YYYY/MM/DD HH:MM:SS] - [GTK4_IM] - 메시지
```

출력 대상:

- 콘솔 (`g_print`)
- 파일 (`~/.unim-errors.log`)
