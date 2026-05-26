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

> [!NOTE]
> 한자/특수문자/이모지 팝업은 IM 모듈이 **직접 그리지 않습니다**.
> 모든 팝업 렌더링은 독립 GTK4 프로세스인 **unim-popup-service**가 담당하며,
> IM 모듈은 데몬의 팝업 DBus 신호(`ShowEmojiPopupV2`/`HidePopup` 등)를 받아
> `popup_active` 플래그만 관리합니다 (§9 참고).
> 과거 `gtk-common`에 있던 `unim_hanja_popup`·`unim_special_popup`·`unim_emoji_popup`
> 임베디드 위젯은 제거되었고, 현재 `gtk-common`에 남은 소스는 `unim_dbus_client`뿐입니다.

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
| `gtk4-x11` (선택) | X11 환경에서 커서 절대 좌표 계산 (데몬에 보고) |
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
2. `window_id` 생성: `"{prgname}:gtk4-ctx-0x..."` (실행 파일명 + 컨텍스트 포인터 기반)
3. `unim_dbus_context_new("gtk4-unim", window_id)` → DBus 클라이언트 생성
4. `unim_dbus_set_auto_typefix_callback()` → `AutoTypefixApply` 시그널 구독
5. `unim_dbus_set_commit_text_callback()` → `CommitText` 시그널 구독 (Standalone 팝업 클릭 커밋용)
6. `unim_dbus_set_show_emoji_popup_callback()` → `ShowEmojiPopupV2` 시그널 구독 (`popup_active` 마킹)
7. `unim_dbus_set_hide_popup_callback()` → `HidePopup` 시그널 구독 (`popup_active` 해제)
8. `last_preedit = ""` 초기화 (preedit 전이 추적용)
9. 한자키 설정 로드 (`GetConfig("hanja_keys")` → `hanja_keysyms` 배열)
10. 상태 필드 초기화 (focused, surrounding_text, cursor_area, popup_active, autofix_* 등)

### 2.4 컨텍스트 소멸 (`unim_im_context_dispose`)

GTK4는 `finalize` 대신 **`dispose`** 를 사용합니다 (부모 클래스 호환성):

1. DBus 클라이언트 해제 (`unim_dbus_context_free`)
2. `window_id`, `surrounding_text`, `hanja_keysyms` 해제
3. `last_preedit`, `autofix_commit_text`, `autofix_preedit_text` 해제
4. 부모 클래스 `dispose` 호출

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

    /* 커서 위치 (위젯 로컬 좌표, 데몬에 보고 → popup-service 좌표 계산용) */
    GdkRectangle cursor_area;

    /* 한자/특수문자 키 설정 캐시 */
    guint *hanja_keysyms;              /* 설정 기반 한자키 keysym 배열 */
    gsize n_hanja_keysyms;             /* 배열 크기 */

    /* preedit 전이 추적 (preedit-start/end 자동 발사용) */
    gchar *last_preedit;               /* 마지막으로 emit한 preedit */

    /* 팝업 세션 상태 (Show*Popup 시그널 → TRUE, HidePopup → FALSE) */
    gboolean popup_active;             /* popup-service 팝업 가시 여부 */

    /* AutoTypeFix XTest 폴백용 (delete_surrounding 미지원 앱 대응) */
    guint  autofix_bs_pending;         /* 자가 주입 BackSpace 잔여 수 */
    gchar *autofix_commit_text;        /* 지연 commit 텍스트 */
    gchar *autofix_preedit_text;       /* 지연 preedit 텍스트 */
};
```

> [!NOTE]
> IM 모듈은 한자/특수문자 후보 배열이나 팝업 위젯 포인터를 보관하지 않습니다.
> 후보 데이터·렌더링·키 네비게이션은 전부 unim-daemon + unim-popup-service가 처리하며,
> IM 모듈은 팝업이 떠 있는 동안 `popup_active` 플래그만 유지합니다.

> [!IMPORTANT]
> `last_preedit`는 `unim_emit_preedit()` 헬퍼가 preedit 전이를 판정하여
> `preedit-start` / `preedit-changed` / `preedit-end` 시그널을 올바르게 발사하기 위한 캐시다.
> `preedit-end` 누락 시 ghostty 등 일부 GTK4 앱이 IM 활성 상태로 잠겨 non-text 키 전파가 차단되므로,
> 모든 preedit 변경은 반드시 이 헬퍼를 경유해야 한다.

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

### 4.3 한자 키 처리 (`F9` / `Hangul_Hanja`) — 팝업 트리거

한자키 입력 시 IM 모듈은 **팝업을 직접 그리지 않고** 데몬에 후보 존재 여부를 질의한 뒤
실제 표시는 unim-popup-service에 위임합니다.

```
F9 (0xffc6) 또는 Hangul_Hanja (0xff34) 입력
  → Wayland: ProcessKey 로 전달 → GNOME extension 이 팝업 처리, return TRUE
  → X11 (Standalone): cursor_area 절대 좌표를 데몬에 보고
    → DBus GetHanjaCandidates
      → 한자 후보 있으면: 후보 배열 즉시 해제, "Standalone popup 위임" 로그
         (popup-service 가 ShowHanja 경로로 렌더)
      → 한자 후보 없으면: DBus GetSpecialCharCandidates
        → 특수문자 후보 있으면: 후보 배열 즉시 해제, popup-service 위임
        → 둘 다 없으면(idle): ProcessKey 로 dual-purpose Hanja 분기 →
           엔진이 ShowEmojiPopupV2 시그널 발행 → 핸들러가 popup_active 마킹
  → return TRUE (키 소비)
```

> [!IMPORTANT]
> IM 모듈은 후보 데이터를 받더라도 **즉시 해제**합니다. 후보 렌더링·페이지·선택은
> unim-popup-service가 전담하며, 모듈은 후보 존재 여부만 확인해 트리거 역할을 합니다.

#### 4.3.1 커서 좌표 보고 (GTK4 고유)

팝업 위치 계산은 popup-service가 수행하지만, IM 모듈은 X11에서 커서의 화면 절대 좌표를
**2단계 변환**으로 구해 데몬에 보고합니다:

```
[1단계] 위젯 로컬 → 루트 위젯 (graphene_point)
    GtkWidget *root = gtk_widget_get_root(client_widget);
    gtk_widget_compute_point(client_widget, root, &p_in, &p_out);

[2단계] X11: GdkSurface → 화면 절대 좌표
    GtkNative *native = gtk_widget_get_native(client_widget);
    GdkSurface *surface = gtk_native_get_surface(native);
    XTranslateCoordinates(xdisplay, xwindow,
        DefaultRootWindow(xdisplay), 0, 0, &abs_x, &abs_y, &child_return);
```

> [!NOTE]
> GTK3에서는 `gdk_window_get_origin()` 한 번으로 절대 좌표를 얻지만,
> GTK4에서는 위젯→루트 변환과 Surface→X11 변환을 분리하여 수행합니다.

### 4.4 팝업 활성 중 키 처리 (`popup_active`)

데몬이 `Show*Popup` 시그널을 보내면 `popup_active = TRUE`가 됩니다.
이 동안 들어오는 키는 IM 모듈이 직접 해석하지 않고 **그대로 `ProcessKey`로 전달**되며,
데몬이 그 키를 받아 popup-service의 선택/페이지 이동·확정·취소를 구동합니다.
선택 확정 커밋은 `ProcessKey` 응답 또는 `CommitText` 시그널로 도달하고,
팝업 종료 시 `HidePopup` 시그널로 `popup_active`가 해제됩니다.

### 4.5 비조합 시 특수키 바이패스

조합 상태가 아니고(`!unim_dbus_is_composing`) **팝업도 비활성(`!popup_active`)** 일 때,
다음 키들은 엔진을 거치지 않고 앱에 직접 전달:

| 키 그룹 | 키 범위 | 비고 |
|---------|---------|------|
| 기능키 | F1~F12 | F9 제외 (한자키로 위에서 처리) |
| 방향키 | Left, Up, Right, Down | |
| 네비게이션 | Home, End, PageUp, PageDown, Insert, Delete | |
| Escape | 조합 중이 아니면 앱으로 | |

> [!NOTE]
> 조합 **중**이거나 **팝업 활성 중**일 때는 이 키들도 엔진(`ProcessKey`)으로 전달됩니다.
> (팝업 활성 중 방향키/Enter/Esc 등이 popup-service 네비게이션으로 가도록 우회를 차단합니다.)

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
  → DBus FocusOut → 조합 중 텍스트 커밋
       → commit 시그널 (커밋할 텍스트가 있으면)
       → preedit-changed 시그널
  → is_focused = FALSE
```

> [!NOTE]
> 팝업 정리는 IM 모듈이 직접 하지 않습니다. 데몬이 팝업 종료를 결정하고
> `HidePopup` 시그널로 통지하면 `popup_active`가 해제됩니다.

### 5.3 리셋 (`reset`)

```
GTK reset 호출
  → DBus ResetContext → 조합 중 텍스트 커밋
       → commit 시그널 (커밋할 텍스트가 있으면)
       → preedit-changed 시그널
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

### 8.3 구독 시그널

| 시그널 | 핸들러 | 용도 |
|--------|--------|------|
| `AutoTypefixApply` | `on_auto_typefix` | 자동 한영 교정 적용 — `{delete_chars, commit_text, preedit_text}` |
| `CommitText` | `on_commit_text` | Standalone 팝업 마우스 클릭 커밋 |
| `ShowEmojiPopupV2` | `on_show_emoji_popup` | popup-service 팝업 표시 통지 → `popup_active = TRUE` |
| `HidePopup` | `on_hide_popup` | 팝업 종료 통지 → `popup_active = FALSE` |

---

## 9. 팝업 렌더링 (unim-popup-service 위임)

한자·특수문자·이모지 팝업은 **IM 모듈이 직접 그리지 않습니다**. 모든 팝업 UI는 독립
GTK4 프로세스인 **unim-popup-service**(코어 `unim` 크레이트의 popup 모듈 사용)가 렌더링하며,
IM 모듈은 트리거와 `popup_active` 플래그 관리만 담당합니다.

### 9.1 역할 분담

| 책임 | 담당 |
|------|------|
| 한자키 입력 감지 → 후보 존재 질의 | IM 모듈 (`filter_keypress`, §4.3) |
| 커서 절대 좌표 보고 | IM 모듈 (`ReportCursorRect` / `cursor_area`) |
| 후보 데이터·페이지·선택 상태 | unim-daemon |
| 팝업 윈도우 생성·그리드 렌더·하이라이트·위치 보정 | unim-popup-service (GTK4) |
| 선택 결과 커밋 전달 | `ProcessKey` 응답(키보드) 또는 `CommitText` 시그널(마우스 클릭) |
| 팝업 종료 통지 | `HidePopup` 시그널 → `popup_active = FALSE` |

> [!NOTE]
> 팝업 UI/조작 규격(그리드 레이아웃, 열 점프, 페이지네이션, 시각적 피드백 등)은
> `docs/dev/specs/POPUP_SPEC.md`(GTK/Qt 공통)에 정의되어 있으며 popup-service가 구현합니다.
> 과거 이 SPEC에 있던 `unim_hanja_popup`/`unim_special_popup` GTK 위젯 구현 절은
> 해당 코드가 제거됨에 따라 삭제되었습니다.

---

## 10. GTK4 vs GTK3 차이점 요약

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

## 11. 빌드 및 배포

### 12.1 빌드

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

### 12.2 개발 배포 (`make dev-gtk4`)

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

### 12.3 설치 경로

```
$(GTK4_LIBDIR)/gtk-4.0/4.0.0/immodules/libim-unim.so
```

일반적으로: `/usr/lib/x86_64-linux-gnu/gtk-4.0/4.0.0/immodules/libim-unim.so`

---

## 12. 로깅

`UNIM_DEVELOP=1` 환경변수 설정 시 활성화.

| 모듈명 | 컴포넌트 |
|--------|---------|
| `GTK4_IM` | `immodule.c` (키 처리, 포커스, preedit, popup_active) |
| `GTK4_DBUS` | `unim_dbus_client.c` (DBus 통신) |

> [!NOTE]
> 팝업 렌더링 로그는 unim-popup-service 측에 있습니다 (IM 모듈은 팝업을 직접 그리지 않음).

로그 포맷:

```
[YYYY/MM/DD HH:MM:SS] - [GTK4_IM] - 메시지
```

출력 대상:

- 콘솔 (`g_print`)
- 파일 (`~/.unim-errors.log`)
