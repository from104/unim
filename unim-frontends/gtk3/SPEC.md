# UNIM GTK3 프론트엔드 세부 기능 명세

> GTK3 애플리케이션에서 한글 입력을 제공하는 IM(Input Method) 모듈의 상세 동작을 정의합니다.

---

## 1. 아키텍처 개요

### 1.1 컴포넌트 구성

| 파일 | 위치 | 역할 |
|------|------|------|
| `immodule.c` | `gtk3/src/` | GTK3 IM 모듈 메인 (GtkIMContext 구현) |
| `unim_dbus_client.c` | `gtk-common/src/` | GDBus 기반 unim-daemon 통신 (GTK3/4 공용) |
| `unim_dbus_client.h` | `gtk-common/include/` | DBus 클라이언트 API 헤더 |

> [!NOTE]
> 한자/특수문자/이모지 팝업은 IM 모듈이 **직접 그리지 않습니다**.
> 모든 팝업 렌더링은 독립 GTK4 프로세스인 **unim-popup-service**가 담당합니다.
> IM 모듈은 데몬의 팝업 DBus 신호(`ShowEmojiPopupV2`/`HidePopup` 등)를 받아
> `popup_active` 플래그만 관리합니다 (§9 참고).
> 과거 `gtk-common`에 있던 `unim_hanja_popup`·`unim_special_popup`·`unim_emoji_popup`
> 임베디드 위젯은 제거되었고, 현재 `gtk-common`에 남은 소스는 `unim_dbus_client`뿐입니다.

### 1.2 통신 구조

```
┌────────────────────┐   GtkIMContext API   ┌──────────────┐   GDBus    ┌──────────────┐
│  GTK3 애플리케이션 │ ←────────────────→ │  libim-unim  │ ←──────→ │  unim-daemon │
│ (gedit, leafpad 등)│ (filter_keypress 등)  │  (.so 모듈)  │  (동기)   │  (입력 엔진) │
└────────────────────┘                      └──────────────┘           └──────────────┘
```

### 1.3 주요 의존성

| 라이브러리 | 용도 |
|-----------|------|
| `gtk+-3.0` | GTK3 IM 프레임워크 (GtkIMContext) |
| `gio-2.0` | GDBus 비동기 통신 |
| `x11` (선택) | X11 환경에서 커서 절대 좌표 계산 (데몬에 보고) |

---

## 2. IM 모듈 수명주기

### 2.1 모듈 등록

GTK3는 공유 라이브러리 형태의 IM 모듈을 동적으로 로드합니다.

| 엔트리 포인트 | 동작 |
|--------------|------|
| `im_module_init(GTypeModule*)` | `UnimIMContext` GType 등록 |
| `im_module_exit()` | (정리 작업 없음) |
| `im_module_list(**contexts, *n_contexts)` | `"unim"` 컨텍스트 정보 반환 |
| `im_module_create(context_id)` | `UnimIMContext` 인스턴스 생성 |

모듈 정보:

```c
context_id     = "unim"
context_name   = "UNIM 한글 입력기"
default_locales = "ko:*"
```

### 2.2 컨텍스트 초기화 (`unim_im_context_init`)

1. `UNIM_DEVELOP=1` 여부 확인 → 디버그 모드 설정
2. `window_id` 생성: `"{prgname}:gtk3-ctx-0x..."` (실행 파일명 + 컨텍스트 포인터 기반)
3. `unim_dbus_context_new("gtk3-unim", window_id)` → DBus 클라이언트 생성
4. `unim_dbus_set_auto_typefix_callback()` → `AutoTypefixApply` 시그널 구독
5. `unim_dbus_set_commit_text_callback()` → `CommitText` 시그널 구독 (Standalone 팝업 클릭 커밋용)
6. `unim_dbus_set_show_emoji_popup_callback()` → `ShowEmojiPopupV2` 시그널 구독 (`popup_active` 마킹)
7. `unim_dbus_set_hide_popup_callback()` → `HidePopup` 시그널 구독 (`popup_active` 해제)
8. `last_preedit = ""` 초기화 (preedit 전이 추적용)
9. 한자키 설정 로드 (`GetConfig("hanja_keys")` → `hanja_keysyms` 배열)
10. 상태 필드 초기화 (focused, surrounding_text, cursor_area, popup_active, autofix_* 등)

### 2.3 컨텍스트 소멸 (`unim_im_context_finalize`)

1. DBus 클라이언트 해제 (`unim_dbus_context_free`)
2. `window_id`, `surrounding_text`, `hanja_keysyms` 해제
3. `last_preedit`, `autofix_commit_text`, `autofix_preedit_text` 해제
4. 부모 클래스 `finalize` 호출

---

## 3. 컨텍스트 상태 (`UnimIMContext`)

```c
struct _UnimIMContext {
    GtkIMContext parent;
    UnimDbusContext *dbus_ctx;     /* DBus 클라이언트 */
    gboolean is_focused;
    GdkWindow *client_window;     /* GTK3 윈도우 레퍼런스 */
    gchar *window_id;              /* "gtk3-ctx-0x..." */

    /* 주변 텍스트 캐시 */
    gchar *surrounding_text;
    gint cursor_index;             /* 바이트 오프셋 */
    gint selection_index;          /* 바이트 오프셋 */

    /* 커서 위치 (데몬에 보고 → popup-service 좌표 계산용) */
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
> `preedit-end` 누락 시 ghostty 등 일부 앱이 IM 활성 상태로 잠겨 non-text 키 전파가 차단되므로,
> 모든 preedit 변경은 반드시 이 헬퍼를 경유해야 한다.

> [!NOTE]
> `hanja_keysyms`는 초기화 시 DBus `GetConfig("hanja_keys")` 호출로 설정을 로드하고,
> `unim_keycode_name_to_gdk_keyval()` 함수로 GDK keyval 배열로 변환하여 캐시합니다.
> 설정 로드 실패 시 기본값(`Hangul_Hanja`, `F9`)이 사용됩니다.

---

## 4. 키 입력 처리 (`filter_keypress`)

### 4.1 전처리

```
이벤트 수신
  → 1. DBus 컨텍스트 확인 (없으면 return FALSE)
  → 2. KeyRelease 무시 (GDK_KEY_PRESS만 처리)
  → 3. 한자 키 확인 (설정 기반: hanja_keysyms 배열 비교)
  → 4. 수정자 키 바이패스 → return FALSE (앱에 전달)
```

**바이패스 대상 수정자 키:**

- Shift_L/R, Control_L/R, Alt_L
- Super_L/R, Meta_L/R, ISO_Level3_Shift

> **Alt_R(오른쪽 Alt)은 바이패스하지 않습니다** — bare Alt_R 은 데몬으로 전달해 토글 여부를 `toggle_keys` 가 판정한다(T3, 프런트 자체 스킵 제거·토글 판정 데몬 일원화). AltGr(`ISO_Level3_Shift`)은 계속 바이패스하므로 AltGr 레이아웃에는 영향이 없다.

### 4.2 한자 키 처리 (`F9` / `Hangul_Hanja`) — 팝업 트리거

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

### 4.3 팝업 활성 중 키 처리 (`popup_active`)

데몬이 `Show*Popup` 시그널을 보내면 `popup_active = TRUE`가 됩니다.
이 동안 들어오는 키는 IM 모듈이 직접 해석하지 않고 **그대로 `ProcessKey`로 전달**되며,
데몬이 그 키를 받아 popup-service의 선택/페이지 이동·확정·취소를 구동합니다.

```
popup_active == TRUE 인 동안 키 입력
  → §4.6의 비조합 특수키 바이패스 가드가 popup_active 일 때 우회를 차단
     (그렇지 않으면 방향키/Esc/Home/End/PageUp/PageDown 이 popup 대신 앱으로 샘)
  → 키는 ProcessKey DBus 경로(§4.7)로 전달
  → 데몬이 popup-service 네비게이션/선택/커밋을 수행하고
     필요한 commit/preedit/HidePopup 결과를 모듈에 전달
```

> [!NOTE]
> 선택 확정 시 커밋 문자열은 `CommitText` 시그널(마우스 클릭 경로) 또는 `ProcessKey`
> 응답(키보드 경로)을 통해 도달하며, 팝업 종료 시 `HidePopup` 시그널로 `popup_active`가 해제됩니다.

### 4.6 비조합 시 특수키 바이패스

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
> 예: 조합 중 방향키 → 엔진이 조합 확정 후 키 바이패스 / 팝업 활성 중 방향키 → 팝업 네비게이션.

### 4.7 일반 키 처리 (ProcessKey)

```
키 입력 → 수정자 상태 변환 (GDK → 비트필드)
       → evdev 코드 변환 (hardware_keycode - 8)
       → DBus ProcessKey(keyval, evdev_code, mod_state)
       → 응답: UnimDbusKeyResult { consumed, preedit, commit }
```

#### 4.7.1 수정자 상태 비트필드 변환

| GDK 마스크 | 비트 | 의미 |
|-----------|------|------|
| `GDK_SHIFT_MASK` | bit 0 | Shift |
| `GDK_LOCK_MASK` | bit 1 | CapsLock |
| `GDK_CONTROL_MASK` | bit 2 | Ctrl |
| `GDK_MOD1_MASK` | bit 3 | Alt |
| `GDK_SUPER_MASK` | bit 26 | Super |

#### 4.7.2 결과 처리

```
result.consumed == TRUE:
  → 1. 선택 영역 삭제 (retrieve-surrounding → delete-surrounding)
  → 2. commit 텍스트 커밋 (commit 시그널)
  → 3. preedit 전이는 `unim_emit_preedit()` 헬퍼로 emit
       (preedit-start / preedit-changed / preedit-end 자동 판정)
  → return TRUE

result.consumed == FALSE:
  → return FALSE (앱에 키 바이패스)
```

> [!NOTE]
> Space 키 영문 모드에서도 엔진이 `consumed=TRUE, commit=" "`로 응답하므로
> 위 경로를 그대로 따른다. `not_consumed`로 바이패스하면 조합 중 영문 전환 시
> Space가 FocusOut 커밋과 중복될 수 있어 금지됨 (552b5bd 참조).

### 4.8 선택 영역 자동 삭제

키가 엔진에 의해 소비된 경우, 선택 영역이 있으면 자동 삭제:

```
retrieve-surrounding 시그널 → 최신 주변 텍스트 획득
  → cursor_index != selection_index (선택 영역 존재)
    → 바이트 오프셋 → 문자 오프셋 변환
    → delete-surrounding(offset, length)
    → 캐시 무효화
```

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
  → 1. DBus FocusOut → 조합 중 텍스트 커밋 (RPC 반환값 사용)
       → commit 시그널 (커밋할 텍스트가 있으면)
       → unim_emit_preedit(unim, "") — preedit-end까지 발사
  → is_focused = FALSE
```

> [!NOTE]
> 팝업 정리는 IM 모듈이 직접 하지 않습니다. 팝업 종료는 데몬이 결정하고
> `HidePopup` 시그널로 통지하면 `popup_active`가 해제됩니다.

> [!IMPORTANT]
> 엔진의 `CommitText` 시그널은 Standalone 팝업 마우스 클릭 경로 전용이며,
> FocusOut 커밋은 **RPC 반환값만** 사용한다 (IME_BEHAVIOR.md §2.2, 552b5bd).
> 데몬이 FocusOut에서 CommitText 시그널을 추가로 발송하지 않으므로 이중 커밋은 발생하지 않는다.

### 5.3 리셋 (`reset`)

```
GTK reset 호출
  → DBus ResetContext → 조합 중 텍스트 커밋
       → commit 시그널 (커밋할 텍스트가 있으면)
       → unim_emit_preedit(unim, "") — preedit-end까지 발사
```

> [!NOTE]
> 팝업 정리는 데몬/popup-service 측에서 처리됩니다 (§5.2 참고).

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

> [!NOTE]
> GTK3에서는 클라이언트 앱이 preedit을 직접 렌더링합니다 (inline preedit).
> 앱의 텍스트 위젯이 preedit-changed 시그널을 받으면 `get_preedit_string`을 호출하여 표시합니다.

### 6.2 커서 위치 업데이트 (`set_cursor_location`)

```
GTK set_cursor_location(GdkRectangle *area) 호출
  → cursor_area에 저장 (데몬에 보고 → popup-service 팝업 위치 계산에 사용)
```

---

## 7. 주변 텍스트 (`set_surrounding`)

```
GTK set_surrounding(text, len, cursor_index) 호출
  → surrounding_text 저장 (len < 0이면 전체 문자열, 아니면 len 바이트)
  → cursor_index 저장 (바이트 오프셋)
  → selection_index = cursor_index (GTK3는 anchor 정보 미제공)
```

> [!NOTE]
> GTK3에서는 `set_surrounding`이 커서 위치만 제공합니다.
> 선택 영역 정보는 `retrieve-surrounding` 시그널 후 위젯이 업데이트합니다.

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
| `unim_dbus_cancel_hanja` | `CancelHanja` | `commit_trigger` | 한자 모드 취소 (트리거 문자 반환) |
| `unim_dbus_get_special_char_candidates` | `GetSpecialCharCandidates` | `{target, chars[], count, top_row}` | 특수문자 후보 조회 |
| `unim_dbus_select_special_char` | `SelectSpecialChar` | `selected_char` | 특수문자 선택 |
| `unim_dbus_cancel_special_char` | `CancelSpecialChar` | `commit_trigger` | 특수문자 모드 취소 |
| `unim_dbus_set_surrounding_text` | `SetSurroundingText` | — | 주변 텍스트 전달 |
| `unim_dbus_set_content_type` | `SetContentType` | — | 입력 필드 목적 전달 (Password/Email 등) |
| `unim_dbus_report_cursor_rect` | `ReportCursorRect` | — | 커서 위치 보고 (fire-and-forget) |
| `unim_dbus_get_config` | `GetConfig` | `value` | 설정 값 조회 (legacy 키 단위) |

### 8.3 구독 시그널

| 시그널 | 인터페이스 | 핸들러 | 용도 |
|--------|-----------|--------|------|
| `AutoTypefixApply` | `org.atit.unim.InputContext` | `on_auto_typefix` | 자동 한영 교정 적용 — `{delete_chars, commit_text, preedit_text}` |
| `CommitText` | `org.atit.unim.InputContext` | `on_commit_text` | Standalone 팝업 마우스 클릭 커밋 |
| `ShowEmojiPopupV2` | `org.atit.unim.InputContext` | `on_show_emoji_popup` | popup-service 팝업 표시 통지 → `popup_active = TRUE` |
| `HidePopup` | `org.atit.unim.InputContext` | `on_hide_popup` | 팝업 종료 통지 → `popup_active = FALSE` |

> [!NOTE]
> GTK3/4 IM 모듈은 **legacy `GetConfig`만** 사용한다 (`hanja_keys` 로드용).
> 전체 설정 YAML/JSON 엔드포인트(`GetConfigYaml`/`SetConfigYaml`/`GetConfigJson`
> + `ConfigChangedJson` 시그널)는 GUI(`unim-gui-gtk`, `unim-gui-qt`)와
> `unim-cli config` 서브커맨드가 사용한다 (unim-dbus/SPEC.md §5.1, §5.2 참고).

---

## 9. 팝업 렌더링 (unim-popup-service 위임)

한자·특수문자·이모지 팝업은 **IM 모듈이 직접 그리지 않습니다**. 모든 팝업 UI는 독립
GTK4 프로세스인 **unim-popup-service**(코어 `unim` 크레이트의 popup 모듈 사용)가 렌더링하며,
IM 모듈은 트리거와 `popup_active` 플래그 관리만 담당합니다.

### 9.1 역할 분담

| 책임 | 담당 |
|------|------|
| 한자키 입력 감지 → 후보 존재 질의 | IM 모듈 (`filter_keypress`, §4.2) |
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

## 10. AutoTypeFix 통합

엔진이 한영 오타(예: `dkssud` → `안녕`)를 감지하면 `AutoTypefixApply` 시그널을
해당 컨텍스트 경로로 발송한다. IM 모듈은 이를 구독해 기존 타이핑을 교정 문자열로 치환한다.

### 11.1 시그널 페이로드

```
AutoTypefixApply(delete_chars: u, commit_text: s, preedit_text: s)
```

### 11.2 교정 흐름 (`on_auto_typefix`)

```
1. delete_chars 만큼 GtkIMContext delete_surrounding 시도
2. 성공 → commit_text 커밋 → preedit_text를 unim_emit_preedit으로 설정
3. 실패 (Electron 등 delete_surrounding 미지원):
   X11이면 → XTest로 BackSpace 키 N번 합성 주입
     + filter_keypress에서 자가 주입 BackSpace를 패스스루 카운트 다운
     + 마지막 BackSpace 소비 후 g_idle_add로 지연 commit/preedit 적용
   X11 아니면 → commit에 "\b" × N 폴백 후 정상 commit/preedit
```

### 11.3 자가 주입 BackSpace 패스스루

- `autofix_bs_pending`가 0보다 크면 `filter_keypress`는 BackSpace를
  `return FALSE`로 앱에 전달하고 카운터만 감소시킨다.
- 카운터가 0이 되는 시점에 `autofix_deferred_commit_cb`를 idle로 예약해
  실제 `commit` + `unim_emit_preedit`을 처리한다.
- 이 패턴은 Chrome/Electron 앱에서도 순방향/역방향 AutoTypeFix가 동작하게 한다.

---

## 11. GTK3 vs XIM 비교

| 관점 | GTK3 | XIM |
|------|------|-----|
| **통신** | GDBus 동기 호출 | tokio mpsc + DBus 비동기 |
| **preedit** | GTK inline (앱이 렌더링) | 별도 PeWindow / Callbacks |
| **한자 팝업** | unim-popup-service 위임 (모듈은 트리거만) | Xlib/Xft 직접 렌더링 (인프로세스) |
| **외부 클릭** | popup-service 처리 | grab_pointer + 합성 Escape |
| **키 릴리스** | GDK_KEY_PRESS만 처리 | response_type == 3 필터링 |
| **모듈 형태** | .so 공유 라이브러리 | 독립 실행 파일 |
| **언어** | C | Rust |

---

## 12. 빌드 및 배포

### 13.1 빌드

```bash
mkdir -p unim-frontends/gtk3/build
cd unim-frontends/gtk3/build
cmake ..
make
```

또는 프로젝트 루트에서:

```bash
make build-frontends
```

### 13.2 개발 배포 (`make dev-gtk3`)

```bash
make dev-gtk3 PREFIX=/usr
```

동작:

1. `cmake` + `make` (gtk3/build)
2. `sudo cp libim-unim.so $(GTK3_IM_MODULEDIR)/`

### 13.3 설치 경로

```
$(GTK3_LIBDIR)/gtk-3.0/3.0.0/immodules/libim-unim.so
```

일반적으로: `/usr/lib/x86_64-linux-gnu/gtk-3.0/3.0.0/immodules/libim-unim.so`

---

## 13. 로깅

`UNIM_DEVELOP=1` 환경변수 설정 시 활성화.

| 모듈명 | 컴포넌트 |
|--------|---------|
| `GTK3_IM` | `immodule.c` (키 처리, 포커스, preedit, popup_active) |
| `GTK_DBUS` | `unim_dbus_client.c` (DBus 통신, GTK3/4 공용) |

> [!NOTE]
> 팝업 렌더링 로그는 unim-popup-service 측에 있습니다 (IM 모듈은 팝업을 직접 그리지 않음).

로그 포맷:

```
[YYYY/MM/DD HH:MM:SS] - [GTK3_IM] - 메시지
```

출력 대상:

- 콘솔 (`g_print`)
- 파일 (`~/.unim-errors.log`)
