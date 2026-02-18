# UNIM XIM 프론트엔드 세부 기능 명세

> X11 환경에서 한국어 입력을 제공하는 XIM(X Input Method) 서버의 상세 동작을 정의합니다.

---

## 1. 아키텍처 개요

### 1.1 컴포넌트 구성

| 파일 | 역할 |
|------|------|
| `main.rs` | XIM 서버 초기화, x11rb 이벤트 루프, 종료 처리 |
| `handler.rs` | XIM 프로토콜 이벤트 처리, DBus 연동, 한자 팝업 관리 |
| `hanja_window.rs` | X11 Xlib/Xft 기반 한자 후보 팝업 윈도우 |
| `pe_window.rs` | Preedit(조합 중 텍스트) 표시 윈도우 |
| `dbus_client.rs` | unim-daemon과의 비동기 DBus 통신 |

### 1.2 통신 구조

```
┌─────────────┐   XIM 프로토콜    ┌──────────────┐   DBus    ┌──────────────┐
│  클라이언트  │ ←────────────→ │  unim-xim    │ ←──────→ │  unim-daemon │
│  (앱/에디터) │  (x11rb/xim)    │  (XIM 서버)  │  (async)  │  (입력 엔진) │
└─────────────┘                  └──────────────┘           └──────────────┘
```

### 1.3 주요 의존성

| 크레이트 | 버전 | 용도 |
|----------|------|------|
| `x11rb` | 0.13 | X11 프로토콜 (xcb/Rust 바인딩) |
| `xim` | 0.5 | XIM 프로토콜 서버 (x11rb-xcb 백엔드) |
| `x11` | — | Xlib/Xft API (Preedit, 한자 팝업 렌더링) |

---

## 2. XIM 서버 수명주기

### 2.1 초기화 (`main.rs`)

1. `UNIM_DEVELOP=1` 여부 확인 → 디버그 모드 설정
2. `Config::load_from_default_path()` → 설정 로드
3. `DbusClient::new()` → DBus 비동기 클라이언트 시작
4. `x11rb::RustConnection::connect()` → X11 연결
5. `X11rbServer::init(conn, screen, "unim", ALL_LOCALES)` → XIM 서버 등록
6. `Ctrl+C` 시그널 핸들러 설정
7. 이벤트 루프 진입

### 2.2 이벤트 루프

```
poll_for_event() → 10ms sleep (None) 또는 이벤트 처리:
├── filter_event() → true: XIM 프로토콜 이벤트 처리
└── filter_event() → false: 비-XIM 이벤트
    ├── Expose → preedit/popup 윈도우 다시 그리기
    ├── ConfigureNotify → 앱 윈도우 이동 시 preedit 위치 갱신
    ├── ButtonPress → 한자 팝업 외부 클릭 감지
    ├── DestroyNotify/UnmapNotify/MappingNotify → 무시
    └── 기타 → 로그 출력
```

### 2.3 종료

- `Ctrl+C` → `running` 플래그 해제 → 이벤트 루프 탈출
- `UnimHandler::drop()` → Xlib Display 연결 닫기

---

## 3. 입력 컨텍스트 (IC) 관리

### 3.1 IC 생성 (`handle_create_ic`)

1. 클라이언트 앱이 IC 생성 요청
2. `UnimInputContext` 생성 (context_path, input_style, pe_window)
3. DBus `CreateContext` → 데몬에 컨텍스트 등록
4. `server.set_event_mask(&ic, 1, 0)` → KeyPress 이벤트만 수신

### 3.2 IC 소멸 (`handle_destroy_ic`)

1. DBus `DestroyContext` → 데몬 컨텍스트 해제
2. 한자 팝업이 열려있으면 닫기 (ungrab 포함)
3. Preedit 윈도우 정리

### 3.3 IC 리셋 (`handle_reset_ic`)

1. DBus `ResetContext` → 데몬 상태 초기화
2. 커밋할 텍스트가 있으면 XIM 커밋
3. Preedit 윈도우 정리

### 3.4 포커스 관리

| 이벤트 | 동작 |
|--------|------|
| `handle_set_focus` | DBus `FocusIn` → 데몬에 포커스 알림 |
| `handle_unset_focus` | DBus `FocusOut` → 조합 중 텍스트 커밋 후 preedit 정리 |

---

## 4. 키 입력 처리 (`handle_forward_event`)

### 4.1 전처리

1. **KeyRelease 무시**: WezTerm 호환성을 위해 `response_type == 3` (KeyRelease) 이벤트는 소비 처리하고 반환
2. **evdev 코드 변환**: X keycode → evdev keycode (`keycode - 8`)
3. **keysym 조회**: Xlib `XkbKeycodeToKeysym(display, detail, 0, 0)` → X keysym

### 4.2 한자 팝업 키 처리 (팝업 활성 시)

한자 팝업이 활성 상태(`hanja_window.is_some()`)일 때, **모든 키 입력은 먼저 팝업에 전달**됩니다.

#### 4.2.1 HanjaAction 분기

| Action | 트리거 키 | 동작 |
|--------|-----------|------|
| `Select(idx)` | `1`-`9`, `Enter` (후보 선택 시) | 한자 후보 선택 → DBus `SelectHanja` → XIM 커밋 |
| `Cancel` | `Escape` | 팝업 닫기 → DBus `ProcessKey(0,0,0)` + `CancelHanja` → 조합 복원 |
| `NextPage` | `→`, `Space` | 다음 페이지 이동 → redraw |
| `PrevPage` | `←`, `BackSpace` | 이전 페이지 이동 → redraw |
| `Consumed` | `↑`, `↓`, 모디파이어 키 | 내부 처리 (선택 이동/무시) → redraw |
| `None` | 기타 모든 키 | **조합 커밋 + 팝업 닫기 + 엔진에 키 전달** |

#### 4.2.2 `Select` 상세 흐름

```
숫자 키 또는 Enter → HanjaAction::Select(global_idx)
  → DBus SelectHanja(context_path, idx)
  → 응답: HanjaSelected { committed }
  → XIM commit(committed)
  → preedit 정리
  → 팝업 닫기 + ungrab_pointer
```

#### 4.2.3 `Cancel` 상세 흐름

```
Escape → HanjaAction::Cancel
  → DBus ProcessKey(0,0,0)  // 리셋용 더미키
  → DBus CancelHanja
  → 응답의 preedit/commit 복원
  → 팝업 닫기 + ungrab_pointer
```

#### 4.2.4 `None` (미지원 키) 상세 흐름 — fall-through 방식

```
문자 키 등 → HanjaAction::None
  → 1. FocusOut DBus → 조합 중 한글 커밋 (예: "한" 커밋)
  → 2. XIM commit + preedit 정리
  → 3. 한자 팝업 닫기
  → 4. CancelHanja DBus
  → 5. ungrab_pointer
  → 6. fall-through → 아래 ProcessKey 경로에서 엔진이 새 키 처리
         (한글 모드면 한글 조합, 영문 모드면 영문 입력)
```

> [!IMPORTANT]
> `return Ok(false)`가 아닌 **fall-through** 사용.
> `return Ok(false)`는 raw keysym을 앱에 직접 전달하여 엔진을 우회합니다.
> fall-through는 키를 정상적인 `ProcessKey` DBus 경로로 전달하여 언어 상태에 따른 올바른 입력을 보장합니다.

#### 4.2.5 `Consumed` — 모디파이어 키 무시

```
팝업에서 무시하는 키:
  - Shift, Ctrl, Alt, CapsLock, Super, Hyper, Meta: 0xffe1..=0xffee
  - Num_Lock: 0xff7f
  - Scroll_Lock: 0xff14
  → HanjaAction::Consumed → redraw + 키 소비 (팝업 유지)
```

#### 4.2.6 페이지 이동 및 선택 이동

```
Up/Down 화살표:
  → selected_index 변경 (0 ~ page_items.len()-1 순환)
  → redraw로 선택 바 갱신

Left/Right 화살표, Space, BackSpace:
  → current_page 변경 (0 ~ total_pages-1)
  → selected_index = 0 으로 리셋
  → redraw로 페이지 갱신
```

#### 4.2.7 redraw 대상

`NextPage`, `PrevPage`, `Consumed`, `None` 액션 후 `hw.redraw(display)` 호출.
`Select`, `Cancel`은 팝업을 닫으므로 redraw 불필요.

### 4.3 한자 키 처리 (설정 기반: `hanja_keys`)

한자 팝업이 **닫혀있을 때** 설정된 한자 키 입력 시:

> [!NOTE]
> 한자 키는 `hanja_keys` 설정에 의해 결정됩니다 (기본: `Hanja`, `F9`).
> `UnimHandler` 초기화 시 설정에서 `hanja_keys`를 읽어 X11 keysym 배열(`hanja_keysyms`)로 변환하여 캐시합니다.

```
설정된 한자 키 입력 (예: F9 또는 Hangul_Hanja)
  → hanja_keysyms 배열과 현재 keysym 비교
  → 1. DBus GetHanjaCandidates(context_path)
       (엔진의 start_hanja_conversion() 트리거)
  → 한자 후보가 있으면:
     1. app_win 절대 좌표 계산 (XTranslateCoordinates)
     2. preedit spot 기반 팝업 위치 결정 (커서 아래 +20px)
     3. HanjaWindow::new() → 팝업 생성
     4. set_candidates() → 후보 표시
     5. grab_pointer(popup_wid, BUTTON_PRESS) → 외부 클릭 감지 활성화
     6. hanja_client_window 저장 (합성 Escape 전송용)
  → 한자 후보가 없으면:
     → 2. DBus GetSpecialCharCandidates(context_path)
          (엔진이 이미 special_char_mode를 설정한 상태)
     → 특수문자 후보가 있으면:
        1. app_win 절대 좌표 계산 (XTranslateCoordinates)
        2. preedit spot 기반 팝업 위치 결정 (커서 아래 +20px)
        3. SpecialWindow::new() → 팝업 생성
        4. set_characters() → 후보 표시
        5. grab_pointer(popup_wid, BUTTON_PRESS) → 외부 클릭 감지 활성화
     → 특수문자 후보도 없으면:
        로그 출력, 아무 동작 없음
```

> [!IMPORTANT]
> **호출 순서가 중요합니다.** `GetHanjaCandidates`를 반드시 먼저 호출해야 합니다.
> 이 호출이 엔진의 `start_hanja_conversion()`을 트리거하여 한자/특수문자 모드를 설정합니다.
> `GetSpecialCharCandidates`는 이미 설정된 모드 상태만 읽으므로, 순서가 바뀌면 첫 번째 키 입력에서 후보가 표시되지 않습니다.
> 이 순서는 GTK3/4 구현과 동일합니다.

### 4.4 일반 키 처리 (ProcessKey)

한자 팝업이 닫혀있고 한자 키가 아닌 경우:

```
키 입력 → DBus ProcessKey(context_path, keysym, evdev_code, state)
  → 응답: KeyProcessed { result, preedit, commit, mode }
  → result == true: 키가 엔진에 의해 소비됨
      → commit 텍스트 있으면 XIM commit
      → preedit 텍스트 있으면 preedit 표시, 없으면 정리
  → result == false: 키가 엔진에 의해 처리되지 않음
      → return Ok(false) → 앱에 키 바이패스
```

---

## 5. 한자 팝업 외부 클릭 감지

### 5.1 메커니즘: `grab_pointer` + 합성 Escape

XIM 프로토콜의 제약 상 `handle_button_press`에서는 `user_ic` (입력 컨텍스트)에 접근할 수 없습니다. 이를 해결하기 위해 **합성 Escape 키 주입** 방식을 사용합니다.

### 5.2 grab_pointer 설정

팝업 생성 시:
```rust
server.conn().grab_pointer(
    false,                     // owner_events
    popup_wid,                 // grab_window
    EventMask::BUTTON_PRESS,   // event_mask
    GrabMode::ASYNC,           // pointer_mode
    GrabMode::ASYNC,           // keyboard_mode
    NONE,                      // confine_to
    NONE,                      // cursor
    CURRENT_TIME,              // time
);
```

→ 모든 마우스 클릭이 팝업 윈도우로 리다이렉트됩니다.

### 5.3 외부 클릭 처리 흐름

```
ButtonPress 이벤트 수신 (main.rs)
  → has_hanja_popup() 확인
  → handle_button_press(event_x, event_y, conn) 호출
  → 클릭 좌표가 팝업 내부?
    ├── YES → Ok(false) (무시, 키보드로만 선택)
    └── NO  → 외부 클릭:
        1. ungrab_pointer() → 마우스 제어 해제
        2. Xlib XSendEvent(Escape) → 클라이언트 윈도우에 합성 Escape 전송
        3. Ok(true)

합성 Escape 전달 경로:
  XSendEvent → 클라이언트 앱 → XFilterEvent → XIM_FORWARD_EVENT → XIM 서버
  → handle_forward_event (user_ic 있음!) → HanjaAction::Cancel
  → 조합 복원 + 팝업 닫기 + ungrab
```

### 5.4 합성 Escape 키 구성

```rust
XKeyEvent {
    type_: KeyPress,
    window: client_win,          // 저장된 클라이언트 윈도우 ID
    root: XRootWindow(display, screen),
    keycode: XKeysymToKeycode(display, 0xff1b),  // Escape keysym
    state: 0,
    time: CurrentTime,
    send_event: True,
    same_screen: True,
}
```

> [!NOTE]
> 이 방식은 클라이언트가 합성 이벤트(send_event=True)를 `XFilterEvent`로 전달하는 경우에만 동작합니다.
> 대부분의 X11 앱(xterm, leafpad, WezTerm 등)은 합성 이벤트도 정상 필터링합니다.

### 5.5 hanja_client_window 관리

| 시점 | 동작 |
|------|------|
| 팝업 생성 | `app_win.get() as u64` 저장 |
| Select/Cancel/외부 클릭/destroy_ic | `None`으로 초기화 |

---

## 6. Preedit 윈도우 (`pe_window.rs`)

### 6.1 지원 입력 스타일

| 스타일 | 동작 |
|--------|------|
| `PREEDIT_POSITION` | 자체 PeWindow (override_redirect X11 윈도우) 사용 |
| `PREEDIT_CALLBACKS` | 클라이언트 앱이 preedit 표시 (server.preedit_draw) |
| `PREEDIT_NOTHING` | 자체 PeWindow 사용 (Position과 동일) |

### 6.2 PeWindow 위치

- `preedit_spot` (XIM IC의 `XNSpotLocation` 속성) 기반
- `ConfigureNotify` 이벤트로 앱 윈도우 이동/크기 변경 추적
- `XTranslateCoordinates`로 앱 윈도우 → 루트 윈도우 상대 좌표 변환

---

## 7. DBus 통신 (`dbus_client.rs`)

### 7.1 요청-응답 패턴

```
UnimHandler → mpsc::Sender<DbusRequest> → DbusClient (tokio 태스크)
                                            ↕ DBus 비동기 호출
UnimHandler ← std::sync::mpsc::channel ← DbusClient
```

### 7.2 주요 DBus 메서드

| 요청 | 응답 | 용도 |
|------|------|------|
| `CreateContext` | `ContextCreated { context_path }` | IC 생성 시 데몬 컨텍스트 등록 |
| `DestroyContext` | — | IC 소멸 시 데몬 컨텍스트 해제 |
| `FocusIn` | — | 포커스 획득 알림 |
| `FocusOut` | `CommitText { text }` | 포커스 상실 → 조합 텍스트 커밋 |
| `ProcessKey` | `KeyProcessed { result, preedit, commit, mode }` | 키 입력 처리 |
| `ResetContext` | `CommitText { text }` | IC 리셋 → 조합 텍스트 커밋 |
| `GetHanjaCandidates` | `HanjaCandidates { target, candidates }` | 한자 후보 조회 |
| `SelectHanja` | `HanjaSelected { committed }` | 한자 후보 선택 |
| `CancelHanja` | — | 한자 모드 취소 |

---

## 8. WezTerm 호환성

### 8.1 KeyRelease 필터링

WezTerm(xcb-imdkit 기반)은 `KeyPress`와 `KeyRelease`를 모두 `ForwardEvent`로 전송하여 이중 입력 문제가 발생합니다.

**해결**: `handle_forward_event` 진입 시 `response_type == 3` (KeyRelease)인 이벤트를 즉시 소비 처리.

```rust
const KEY_RELEASE: u8 = 3;
if xev.response_type == KEY_RELEASE {
    return Ok(true); // 소비
}
```

---

## 9. 한자 팝업 윈도우 (`hanja_window.rs`)

### 9.1 윈도우 속성

- `override_redirect = True` (윈도우 매니저 무시)
- Xlib/Xft 기반 렌더링 (안티앨리어싱 폰트)
- 배경색: `#2D2D2D` (다크 테마)
- 선택 바: `#4A90D9` (파란색 강조)
- 텍스트: 흰색

### 9.2 레이아웃

```
┌─────────────────────────────┐
│ 1. 韓  [한]                 │  ← 후보 항목 (번호. 한자  [원래 한글])
│ 2. 漢  [한]                 │
│ 3. 限  [한]                 │
│ ...                         │
│ 9. 翰  [한]                 │
│ ← 1/3 →      [Esc: 닫기]   │  ← 상태 바 (페이지/탐색 안내)
└─────────────────────────────┘
```

### 9.3 페이지네이션

- 페이지당 최대 9개 후보 (`PAGE_SIZE = 9`)
- `→`/`Space`: 다음 페이지, `←`/`BackSpace`: 이전 페이지
- `↑`/`↓`: 현재 페이지 내 선택 이동 (순환)

---

## 10. 빌드 및 배포

### 10.1 빌드

```bash
cargo build --release -p unim-xim
```

### 10.2 개발 배포 (`make dev-xim`)

```bash
make dev-xim PREFIX=/usr
```

동작:
1. `cargo build --release -p unim-xim`
2. `pkill -x unim-xim` (기존 프로세스 종료, **복사 전**)
3. `sleep 0.5` (프로세스 종료 대기)
4. `sudo cp target/release/unim-xim $(LIBEXECDIR)/`

> [!NOTE]
> `pkill -x` (exact match)를 사용하여 `make` 프로세스 자체가 kill되는 것을 방지합니다.
> 복사 **전에** kill하여 "실행 파일 사용 중" 오류를 방지합니다.

---

## 11. 로깅

`UNIM_DEVELOP=1` 환경변수 설정 시 활성화.

| 모듈명 | 컴포넌트 |
|--------|---------|
| `XIM` | `main.rs` (서버 수명주기) |
| `XIM_HANDLER` | `handler.rs` (키 처리, 한자 팝업) |

로그 예시:
```
[2026/02/17 14:00:00] - [XIM_HANDLER] - 한자 팝업 외부 클릭 감지 -> 합성 Escape 전송
[2026/02/17 14:00:00] - [XIM_HANDLER] - 합성 Escape 전송 완료: client_win=0x1400001
```

---

## 12. 프로토콜 적합성 검증

> [XIM 프로토콜 사양](https://www.x.org/releases/X11R7.6/doc/libX11/specs/XIM/xim.html)과의 적합성을 3회 교차 검증한 결과입니다.

### 12.1 적합 항목

| # | 프로토콜 항목 | 구현 위치 | 비고 |
|---|-------------|----------|------|
| 1 | **Input Method Styles** | `input_styles()` | on-the-spot, off-the-spot, over-the-spot 3종 |
| 2 | **BackEnd Event Handling Model** | `xim` crate ServerHandler | 기본 BackEnd 모델 사용 |
| 3 | **Static Event Flow Control** | `filter_events()=1` | trigger key 미등록 (Static 방식) |
| 4 | **XIM_CREATE_IC** | `new_ic_data()` + `handle_create_ic()` | DBus 컨텍스트 생성, event mask 설정 |
| 5 | **XIM_DESTROY_IC** | `handle_destroy_ic()` | 컨텍스트 파괴, 팝업 정리, preedit 정리 |
| 6 | **XIM_RESET_IC** | `handle_reset_ic()` | preedit string 반환 (사양 준수) |
| 7 | **XIM_SET_IC_FOCUS / UNSET_IC_FOCUS** | `handle_set_focus()`, `handle_unset_focus()` | DBus FocusIn/FocusOut 연동 |
| 8 | **XIM_SET_IC_VALUES** | `handle_set_ic_values()` | spot location 변경 시 PeWindow 갱신 |
| 9 | **XIM_FORWARD_EVENT** | `handle_forward_event()` | KeyPress → ProcessKey → commit/preedit/forward |
| 10 | **XIM_COMMIT** | `server.commit()` | committed string 전송 |
| 11 | **Preedit Callbacks** | `server.preedit_draw()` + PeWindow | XIM_PREEDIT_DRAW, dual-path (callback + OTS) |

### 12.2 참고 사항

| # | 항목 | 설명 |
|---|------|------|
| 1 | **KeyRelease 소비** | WezTerm(xcb-imdkit) 호환을 위해 `Ok(true)` 반환. `filter_events()=1`(KeyPress 전용)이므로 KeyRelease 수신은 비표준 클라이언트에 한정. 프로토콜 위반 아님 |
| 2 | **root-window 스타일 미지원** | 4가지 스타일 중 1가지 미지원. 현대 IME에서 거의 사용되지 않아 실무 영향 없음 |
| 3 | **XIM_GET_IC_VALUES 미구현** | `xim` crate 내부에서 기본 처리. 커스텀 IC 속성이 없으므로 문제 없음 |

---

## 13. 참조

### 13.1 프로토콜 사양

- [The Input Method Protocol (X.Org)](https://www.x.org/releases/X11R7.6/doc/libX11/specs/XIM/xim.html) — XIM 프로토콜 공식 사양
- [Xlib - C Language X Interface: Input Methods](https://www.x.org/releases/current/doc/libX11/libX11/libX11.html#Input_Methods) — Xlib XIM API 레퍼런스
- [X Input Method (Wikipedia)](https://en.wikipedia.org/wiki/X_Input_Method) — XIM 개요 및 역사

### 13.2 참조 구현 및 라이브러리

- [xim-rs (xim crate)](https://github.com/pum-purum-pum-pum/xim-rs) — Rust XIM 프로토콜 구현 (서버/클라이언트)
- [x11rb](https://github.com/psychon/x11rb) — Rust X11 프로토콜 바인딩 (xcb 기반)
- [kime XIM 프론트엔드](https://github.com/Riey/kime/tree/develop/src/frontends/x11) — Rust 한국어 IME의 XIM 구현 참조