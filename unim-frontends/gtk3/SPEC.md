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
| `unim_hanja_popup.c` | `gtk-common/src/` | GTK 기반 한자 후보 팝업 윈도우 (GTK3/4 공용) |
| `unim_hanja_popup.h` | `gtk-common/include/` | 한자 팝업 API 헤더 |
| `unim_special_popup.c` | `gtk-common/src/` | GTK 기반 특수문자 그리드 팝업 윈도우 (GTK3/4 공용) |
| `unim_special_popup.h` | `gtk-common/include/` | 특수문자 팝업 API 헤더 |

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
| `x11` (선택) | X11 환경에서 한자 팝업 위치 계산 |

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
6. `unim_hanja_popup_new()` → 한자 팝업 인스턴스 생성
7. `unim_special_popup_new()` → 특수문자 팝업 인스턴스 생성
8. `last_preedit = ""` 초기화 (preedit 전이 추적용)
9. 한자키 설정 로드 (`GetConfig("hanja_keys")` → `hanja_keysyms` 배열)
10. 상태 필드 초기화 (focused, surrounding_text, cursor_area, autofix_* 등)

### 2.3 컨텍스트 소멸 (`unim_im_context_finalize`)

1. 한자 팝업 해제 (`unim_hanja_popup_free`)
2. 한자 후보 배열 해제 (`unim_hanja_candidates_free`)
3. 특수문자 팝업 해제 (`unim_special_popup_free`)
4. 특수문자 후보 배열 해제 (`unim_special_chars_free`)
5. DBus 클라이언트 해제 (`unim_dbus_context_free`)
6. `window_id`, `surrounding_text`, `hanja_keysyms` 해제
7. `last_preedit`, `autofix_commit_text`, `autofix_preedit_text` 해제
8. 부모 클래스 `finalize` 호출

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

    /* 한자 변환 */
    UnimHanjaPopup *hanja_popup;
    UnimHanjaCandidate *hanja_candidates;
    gsize hanja_count;
    GdkRectangle cursor_area;      /* 커서 위치 (팝업 좌표 계산용) */

    /* 특수문자 입력 */
    UnimSpecialPopup *special_popup;   /* 특수문자 후보 팝업 */
    gchar **special_characters;        /* 현재 특수문자 목록 */
    gsize special_count;               /* 특수문자 개수 */

    /* 한자/특수문자 키 설정 캐시 */
    guint *hanja_keysyms;              /* 설정 기반 한자키 keysym 배열 */
    gsize n_hanja_keysyms;             /* 배열 크기 */

    /* preedit 전이 추적 (preedit-start/end 자동 발사용) */
    gchar *last_preedit;               /* 마지막으로 emit한 preedit */

    /* AutoTypeFix XTest 폴백용 (delete_surrounding 미지원 앱 대응) */
    guint  autofix_bs_pending;         /* 자가 주입 BackSpace 잔여 수 */
    gchar *autofix_commit_text;        /* 지연 commit 텍스트 */
    gchar *autofix_preedit_text;       /* 지연 preedit 텍스트 */
};
```

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

- Shift_L/R, Control_L/R, Alt_L/R
- Super_L/R, Meta_L/R, ISO_Level3_Shift

### 4.2 한자 팝업 키 처리 (팝업 활성 시)

한자 팝업이 활성 상태(`unim_hanja_popup_is_visible`)일 때, **모든 키 입력은 먼저 팝업에 전달**됩니다.

#### 4.2.1 Escape → 조합 복원 + 팝업 닫기

```
Escape 키 입력
  → 1. ProcessKey(0,0,0) — 엔진 리셋 (더미키)
       → 커밋 텍스트가 있으면 커밋
  → 2. CancelHanja — 한자 모드 해제
  → 3. preedit-changed 시그널 (preedit 복원)
  → 4. 팝업 닫기
  → return TRUE (키 소비)
```

#### 4.2.2 팝업 내부 처리 (`unim_hanja_popup_handle_key`)

| 동작 | 트리거 키 | 결과 |
|------|-----------|------|
| **숫자 선택** | `1`-`9` | 해당 인덱스 한자 선택 → 콜백 호출 |
| **Enter 선택** | `Return`, `KP_Enter` | 현재 선택된 한자 확정 → 콜백 호출 |
| **이전 페이지** | `←`, `PageUp`, `BackSpace` | 페이지 이동 + 리스트 갱신 |
| **다음 페이지** | `→`, `PageDown`, `Space` | 페이지 이동 + 리스트 갱신 |
| **선택 이동** | `↑`, `↓` | 선택 인덱스 변경 + 리스트 갱신 |
| **모디파이어** | Shift, Ctrl, Alt 등 | 소비 (팝업 유지) |

#### 4.2.3 한자 선택 콜백 (`on_hanja_selected`)

```
숫자/Enter 선택 → 콜백 호출
  → 1. 팝업 숨기기
  → 2. CancelHanja (preedit 클리어)
  → 3. preedit-changed 시그널
  → 4. commit 시그널 (선택된 한자)
```

#### 4.2.4 미지원 키 → fall-through 방식

```
문자 키 등 → handle_key() returns FALSE
  → 1. FocusOut → 조합 중 한글 커밋 (예: "한" 커밋)
  → 2. preedit-changed (preedit 클리어)
  → 3. CancelHanja + 팝업 닫기
  → 4. FocusIn (컨텍스트 복원)
  → 5. fall-through → 아래 ProcessKey 경로에서 엔진이 새 키 처리
         (한글 모드면 한글 조합, 영문 모드면 영문 입력)
```

> [!IMPORTANT]
> `return FALSE`가 아닌 **fall-through** 사용.
> `return FALSE`는 raw keysym을 앱에 직접 전달하여 엔진을 우회합니다.
> fall-through는 키를 정상적인 `ProcessKey` DBus 경로로 전달하여 언어 상태에 따른 올바른 입력을 보장합니다.

### 4.3 한자 키 처리 (`F9` / `Hangul_Hanja`)

한자/특수문자 팝업이 모두 **닫혀있을 때** F9 키 입력 시:

```
F9 (0xffc6) 또는 Hangul_Hanja (0xff34) 입력
  → 화면 좌표 계산 (cursor_area + X11 절대 좌표 변환)
  → DBus GetHanjaCandidates
  → 한자 후보가 있으면:
    1. 이전 후보 배열 정리
    2. unim_hanja_popup_show(popup, target, candidates, count, x, y, h, callback, unim)
  → 한자 후보가 없으면:
    **특수문자 폴백 →** DBus GetSpecialCharCandidates
    → 특수문자 후보가 있으면:
      1. 이전 특수문자 배열 정리
      2. 특수문자/개수 저장
      3. unim_special_popup_show(popup, target, chars, count, top_row, x, y, h, callback, unim)
    → 특수문자 후보도 없으면:
      로그 출력, 아무 동작 없음
  → return TRUE (키 소비)
```

### 4.4 특수문자 팝업 표시 중 키 처리

특수문자 팝업이 **보이는 동안** 모든 키는 먼저 팝업에서 처리됩니다:

```
특수문자 팝업 표시 중 키 입력
  → Escape 키:
    1. ProcessKey(0,0,0) → 조합 중 자모 커밋 (commit 시그널)
    2. CancelSpecialChar (DBus)
    3. preedit-changed 시그널
    4. 팝업 숨김
    5. return TRUE
  → 팝업 내부 키 (열 점프/숫자/방향키/Enter/Tab/클릭):
    unim_special_popup_handle_key() → return TRUE
  → 미지원 키 (F1~F12, Home, End 등):
    1. CancelSpecialChar (DBus)
    2. 팝업 숨김
    3. preedit-changed 시그널
    4. fall-through → ProcessKey 경로로 전달
```

> [!NOTE]
> 미지원 키 처리 시 `return FALSE`가 아닌 **fall-through**를 사용합니다.
> 특수문자 모드를 취소한 후, 해당 키를 엔진의 `ProcessKey` 경로로 정상 전달합니다.

### 4.5 특수문자 선택 콜백 (`on_special_char_selected`)

팝업에서 문자 선택 (숫자 키, Enter, 클릭) 시 호출:

```
특수문자 선택 (e.g. '☃')
  → 1. unim_special_popup_hide() → 팝업 닫기
  → 2. unim_dbus_cancel_special_char() → 엔진 특수문자 모드 취소 (preedit 클리어)
  → 3. preedit-changed 시그널 (빈 preedit)
  → 4. commit 시그널 (선택된 특수문자)
```

### 4.6 비조합 시 특수키 바이패스

조합 상태가 아닐 때(`!unim_dbus_is_composing`), 다음 키들은 엔진을 거치지 않고 앱에 직접 전달:

| 키 그룹 | 키 범위 | 비고 |
|---------|---------|------|
| 기능키 | F1~F12 | F9 제외 (한자키로 위에서 처리) |
| 방향키 | Left, Up, Right, Down | |
| 네비게이션 | Home, End, PageUp, PageDown, Insert, Delete | |
| Escape | 조합 중이 아니면 앱으로 | |

> [!NOTE]
> 조합 **중**일 때는 이 키들도 엔진(`ProcessKey`)으로 전달됩니다.
> 예: 조합 중 방향키 → 엔진이 조합 확정 후 키 바이패스.

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
  → 1. 한자 팝업 열려있으면 닫기 + CancelHanja (트리거 문자 있으면 커밋)
  → 2. 특수문자 팝업 열려있으면 닫기 + CancelSpecialChar (트리거 문자 있으면 커밋)
  → 3. DBus FocusOut → 조합 중 텍스트 커밋 (RPC 반환값 사용)
       → commit 시그널 (커밋할 텍스트가 있으면)
       → unim_emit_preedit(unim, "") — preedit-end까지 발사
  → is_focused = FALSE
```

> [!IMPORTANT]
> 엔진의 `CommitText` 시그널은 Standalone 팝업 마우스 클릭 경로 전용이며,
> FocusOut 커밋은 **RPC 반환값만** 사용한다 (IME_BEHAVIOR.md §2.2, 552b5bd).
> 데몬이 FocusOut에서 CommitText 시그널을 추가로 발송하지 않으므로 이중 커밋은 발생하지 않는다.

### 5.3 리셋 (`reset`)

```
GTK reset 호출
  → 1. DBus ResetContext → 조합 중 텍스트 커밋
       → commit 시그널 (커밋할 텍스트가 있으면)
       → unim_emit_preedit(unim, "") — preedit-end까지 발사
  → 2. 한자 팝업 열려있으면 닫기 + CancelHanja (트리거 문자 있으면 커밋)
  → 3. 특수문자 팝업 열려있으면 닫기 + CancelSpecialChar (트리거 문자 있으면 커밋)
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

> [!NOTE]
> GTK3에서는 클라이언트 앱이 preedit을 직접 렌더링합니다 (inline preedit).
> 앱의 텍스트 위젯이 preedit-changed 시그널을 받으면 `get_preedit_string`을 호출하여 표시합니다.

### 6.2 커서 위치 업데이트 (`set_cursor_location`)

```
GTK set_cursor_location(GdkRectangle *area) 호출
  → cursor_area에 저장 (한자 팝업 위치 계산에 사용)
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
| `AutoTypefixApply` | `org.atit.unim.InputContext` | `on_auto_typefix_signal` | 자동 한영 교정 적용 — `{delete_chars, commit_text, preedit_text}` |
| `CommitText` | `org.atit.unim.InputContext` | `on_commit_text_signal` | Standalone 팝업 마우스 클릭 커밋 |

> [!NOTE]
> GTK3/4 IM 모듈은 **legacy `GetConfig`만** 사용한다 (`hanja_keys` 로드용).
> 전체 설정 YAML/JSON 엔드포인트(`GetConfigYaml`/`SetConfigYaml`/`GetConfigJson`
> + `ConfigChangedJson` 시그널)는 GUI(`unim-gui-gtk`, `unim-gui-qt`)와
> `unim-cli config` 서브커맨드가 사용한다 (unim-dbus/SPEC.md §5.1, §5.2 참고).

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

### 9.4 한자 팝업 위치 계산

```
popup_x = cursor_area.x
popup_y = cursor_area.y + cursor_area.height  (커서 아래)

X11 환경:
  + gdk_window_get_origin(client_window) → 절대 좌표 변환
```

---

## 10. 특수문자 팝업 윈도우 (`unim_special_popup`)

### 10.1 개요

한자 후보가 없을 때, 조합 중인 자모에 매핑된 특수문자를 **9×9 그리드 팝업**으로 표시합니다.
한자 키(F9)로 트리거되며, 한자 후보가 없으면 자동으로 특수문자 모드로 전환됩니다.

> [!NOTE]
> 구현은 `gtk-common/src/unim_special_popup.c`에 위치하며, GTK3/GTK4 공통 코드입니다.
> `#if GTK_CHECK_VERSION(4, 0, 0)` 전처리기로 GTK 버전별 차이를 처리합니다.

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
| 열 점프 | `Q`~`O` (물리 키) | 해당 열로 이동 |
| 숫자 선택 (행) | `1`-`9` | 선택된 열의 해당 행 문자 커밋 |
| 방향키 이동 | `↑`/`↓`/`←`/`→` | 셀 선택 이동 (경계에서 순환) |
| Enter 확정 | `Return`/`KP_Enter` | 현재 선택 셀의 문자 커밋 |
| 다음 페이지 | `Page_Down`/`Space` | 다음 페이지 |
| 이전 페이지 | `Page_Up` | 이전 페이지 |
| Escape | `Escape` | 조합 중 자모 커밋 + 특수문자 모드 취소 + 팝업 닫기 |
| 마우스 클릭 | 좌클릭 | 클릭한 셀의 문자 커밋 |

> [!IMPORTANT]
> **열 점프는 물리 키 위치(QWERTY) 기준으로 매칭합니다.**
> OS keyval은 항상 QWERTY 기반이고, UNIM 영문 키맵 변환은 엔진 내부에서 일어납니다.
> `top_row` 문자열은 **표시 전용** (드보락: `',.PYFGCR`, 콜맥: `QWFPGJLUY`)이고,
> 키 매칭은 항상 `"qwertyuio"` 물리 키로 수행합니다.

### 10.5 팝업 위치 계산 (모니터별 경계 보정)

```text
1. 기본 위치: popup_x = cursor_area.x, popup_y = cursor_area.y + cursor_area.height
2. X11: gdk_window_get_origin(client_window) → 절대 좌표 변환
3. show_all → 정확한 크기 측정 (GTK3에서는 show_all 후에만 크기 정확)
4. 커서가 위치한 모니터 기준 경계 보정:
   - gdk_display_get_monitor_at_point(display, x, y)
   - gdk_monitor_get_geometry(monitor, &mon_geom)
   - 오른쪽 넘침: popup_x = mon_geom.x + mon_geom.width - width
   - 아래쪽 넘침: popup_y = y - cursor_height - height (커서 위로)
```

> [!IMPORTANT]
> `gdk_screen_width()` / `gdk_screen_height()` 대신 **모니터별 geometry**를 사용합니다.
> 다중 모니터 환경에서 전체 가상 화면이 아닌 실제 모니터 영역 기준으로 보정합니다.

### 10.6 포커스 보존 패턴

X11에서 팝업이 부모 앱의 포커스를 빼앗지 않도록 하는 핵심 순서:

```text
1. gtk_window_new(GTK_WINDOW_POPUP) — 포커스 불가 윈도우
2. gtk_widget_set_can_focus(window, FALSE)
3. show_all 후 위치 재조정 (경계 보정)
```

> [!IMPORTANT]
> GTK3에서는 `GTK_WINDOW_POPUP` 타입이 자동으로 `override_redirect`를 설정하므로,
> GTK4처럼 별도의 X11 `XChangeWindowAttributes` 호출이 불필요합니다.

### 10.7 시각적 피드백

| CSS 클래스 | 용도 |
|---|---|
| `cell-selected` | 현재 선택된 셀 하이라이트 |
| `cell-col-highlight` | 선택된 열의 모든 셀 배경 |
| `cell-row-highlight` | 선택된 행의 모든 셀 배경 |
| `header-active` | 선택된 열/행의 헤더 강조 |
| `row-header` | 행 번호 헤더 (1-9) |
| `cell-flash` | 문자 선택 시 120ms 플래시 효과 |

---

## 11. AutoTypeFix 통합

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

## 12. GTK3 vs XIM 비교

| 관점 | GTK3 | XIM |
|------|------|-----|
| **통신** | GDBus 동기 호출 | tokio mpsc + DBus 비동기 |
| **preedit** | GTK inline (앱이 렌더링) | 별도 PeWindow / Callbacks |
| **한자 팝업** | GTK ListBox 기반 | Xlib/Xft 직접 렌더링 |
| **외부 클릭** | (GTK 윈도우 시스템 처리) | grab_pointer + 합성 Escape |
| **키 릴리스** | GDK_KEY_PRESS만 처리 | response_type == 3 필터링 |
| **모듈 형태** | .so 공유 라이브러리 | 독립 실행 파일 |
| **언어** | C | Rust |

---

## 13. 빌드 및 배포

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

## 14. 로깅

`UNIM_DEVELOP=1` 환경변수 설정 시 활성화.

| 모듈명 | 컴포넌트 |
|--------|---------|
| `GTK3_IM` | `immodule.c` (키 처리, 포커스, preedit) |
| `GTK_DBUS` | `unim_dbus_client.c` (DBus 통신, GTK3/4 공용) |
| `HANJA_POPUP` | `unim_hanja_popup.c` (한자 팝업) |
| `SPECIAL_POPUP` | `unim_special_popup.c` (특수문자 팝업) |

로그 포맷:

```
[YYYY/MM/DD HH:MM:SS] - [GTK3_IM] - 메시지
```

출력 대상:

- 콘솔 (`g_print`)
- 파일 (`~/.unim-errors.log`)
