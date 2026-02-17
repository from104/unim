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
2. `window_id` 생성: `"gtk3-ctx-0x..."` (컨텍스트 포인터 기반)
3. `unim_dbus_context_new("gtk3-unim", window_id)` → DBus 클라이언트 생성
4. `unim_hanja_popup_new()` → 한자 팝업 인스턴스 생성
5. 상태 필드 초기화 (focused, surrounding_text, cursor_area 등)

### 2.3 컨텍스트 소멸 (`unim_im_context_finalize`)

1. 한자 팝업 해제 (`unim_hanja_popup_free`)
2. 한자 후보 배열 해제 (`unim_hanja_candidates_free`)
3. DBus 클라이언트 해제 (`unim_dbus_context_free`)
4. `window_id`, `surrounding_text` 메모리 해제
5. 부모 클래스 `finalize` 호출

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
};
```

---

## 4. 키 입력 처리 (`filter_keypress`)

### 4.1 전처리

```
이벤트 수신
  → 1. DBus 컨텍스트 확인 (없으면 return FALSE)
  → 2. KeyRelease 무시 (GDK_KEY_PRESS만 처리)
  → 3. 수정자 키 바이패스 → return FALSE (앱에 전달)
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

한자 팝업이 **닫혀있을 때** F9 키 입력 시:

```
F9 (0xffc6) 또는 Hangul_Hanja (0xff34) 입력
  → DBus GetHanjaCandidates
  → 후보가 있으면:
    1. 이전 후보 배열 정리
    2. cursor_area 기반 팝업 위치 계산
    3. X11: gdk_window_get_origin → 절대 좌표 변환
    4. unim_hanja_popup_show(popup, target, candidates, count, x, y, h, callback, unim)
  → 후보가 없으면:
    로그 출력, 아무 동작 없음
  → return TRUE (키 소비)
```

### 4.4 비조합 시 특수키 바이패스

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

### 4.5 일반 키 처리 (ProcessKey)

```
키 입력 → 수정자 상태 변환 (GDK → 비트필드)
       → evdev 코드 변환 (hardware_keycode - 8)
       → DBus ProcessKey(keyval, evdev_code, mod_state)
       → 응답: UnimDbusKeyResult { consumed, preedit, commit }
```

#### 4.5.1 수정자 상태 비트필드 변환

| GDK 마스크 | 비트 | 의미 |
|-----------|------|------|
| `GDK_SHIFT_MASK` | bit 0 | Shift |
| `GDK_LOCK_MASK` | bit 1 | CapsLock |
| `GDK_CONTROL_MASK` | bit 2 | Ctrl |
| `GDK_MOD1_MASK` | bit 3 | Alt |
| `GDK_SUPER_MASK` | bit 26 | Super |

#### 4.5.2 결과 처리

```
result.consumed == TRUE:
  → 1. 선택 영역 삭제 (retrieve-surrounding → delete-surrounding)
  → 2. commit 텍스트 커밋  (commit 시그널)
  → 3. preedit-changed 시그널
  → return TRUE

result.consumed == FALSE:
  → return FALSE (앱에 키 바이패스)
```

### 4.6 선택 영역 자동 삭제

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
  → 1. DBus FocusOut → 조합 중 텍스트 커밋
       → commit 시그널 (커밋할 텍스트가 있으면)
       → preedit-changed 시그널
  → 2. 한자 팝업 열려있으면 닫기 + CancelHanja
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
| `unim_dbus_cancel_hanja` | `CancelHanja` | — | 한자 모드 취소 |

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

## 10. GTK3 vs XIM 비교

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

## 11. 빌드 및 배포

### 11.1 빌드

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

### 11.2 개발 배포 (`make dev-gtk3`)

```bash
make dev-gtk3 PREFIX=/usr
```

동작:
1. `cmake` + `make` (gtk3/build)
2. `sudo cp libim-unim.so $(GTK3_IM_MODULEDIR)/`

### 11.3 설치 경로

```
$(GTK3_LIBDIR)/gtk-3.0/3.0.0/immodules/libim-unim.so
```

일반적으로: `/usr/lib/x86_64-linux-gnu/gtk-3.0/3.0.0/immodules/libim-unim.so`

---

## 12. 로깅

`UNIM_DEVELOP=1` 환경변수 설정 시 활성화.

| 모듈명 | 컴포넌트 |
|--------|---------|
| `GTK3_IM` | `immodule.c` (키 처리, 포커스, preedit) |
| `GTK3_DBUS` | `unim_dbus_client.c` (DBus 통신) |
| `HANJA_POPUP` | `unim_hanja_popup.c` (한자 팝업) |

로그 포맷:
```
[YYYY/MM/DD HH:MM:SS] - [GTK3_IM] - 메시지
```

출력 대상:
- 콘솔 (`g_print`)
- 파일 (`~/.unim-errors.log`)
