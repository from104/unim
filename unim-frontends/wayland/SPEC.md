# UNIM Wayland 프론트엔드 세부 기능 명세

> Wayland 환경에서 한국어 입력을 제공하는 입력 방식(Input Method) 클라이언트의 상세 동작을 정의합니다.
> `input-method-unstable-v2` (`zwp_input_method_v2`) + `virtual-keyboard-unstable-v1` (`zwp_virtual_keyboard_v1`) 프로토콜을 사용합니다.

---

## 1. 아키텍처 개요

### 1.1 컴포넌트 구성

| 파일 | 역할 |
|------|------|
| `main.rs` | Wayland 연결, 글로벌 바인딩, mio::Poll 기반 이벤트 루프, `PopupEvent` 디스패치 |
| `state.rs` | AppState 구조체, Dispatch 구현 8종, 키 이벤트 처리, 팝업 상태 관리, AutoTypeFix 적용 |
| `keymap.rs` | xkbcommon 키맵 핸들러, evdev keycode → keysym 변환 |
| `dbus_client.rs` | unim-daemon과의 비동기 DBus 통신 (한자/특수문자/AutoTypeFix 시그널 포함) |
| `repeat.rs` | 키 반복 타이머 (RepeatInfo, PressState, RepeatTimer) |
| `popup_renderer.rs` | 팝업 렌더링 (tiny-skia + cosmic-text), 한자/특수문자 후보 표시 |
| `popup_surface.rs` | `zwp_input_popup_surface_v2` 기반 팝업 서피스 관리 |

### 1.2 통신 구조

```
┌──────────────┐  Wayland 프로토콜   ┌───────────────┐   DBus    ┌──────────────┐
│  컴포지터     │ ←──────────────→  │  unim-wayland │ ←──────→ │  unim-daemon │
│ (KDE/Sway)   │  input-method-v2   │  (IM 클라이언트)│  (async)  │  (입력 엔진) │
└──────────────┘                    └───────────────┘           └──────────────┘
```

### 1.3 주요 의존성

| 크레이트 | 버전 | 용도 |
|----------|------|------|
| `wayland-client` | 0.31 | Wayland 클라이언트 프로토콜 바인딩 |
| `wayland-protocols` | 0.32 | 표준/불안정 프로토콜 (input-method-v2) |
| `wayland-protocols-misc` | 0.3 | 기타 프로토콜 (virtual-keyboard-v1) |
| `xkbcommon` | 0.8 | 키맵 파싱, keycode→keysym 변환 |
| `tokio` | 1 | DBus 비동기 런타임 |
| `zbus` | 4 | DBus 통신 |
| `mio` | 1 | epoll 기반 이벤트 루프 (키 반복 타이머 통합) |
| `nix` | 0.29 | timerfd 시스템 호출, memfd 생성 |
| `tiny-skia` | 0.11 | 팝업 UI 2D 렌더링 (CPU 기반) |
| `cosmic-text` | 0.12 | 팝업 텍스트 레이아웃 및 폰트 렌더링 |
| `memmap2` | 0.9 | `wl_shm` 버퍼용 메모리 매핑 |

### 1.4 지원 컴포지터

| 컴포지터 | `input-method-v2` | `virtual-keyboard-v1` | 팝업 (`zwp_input_popup_surface_v2`) |
|----------|--------------------|-----------------------|---------------------------------------|
| KDE Plasma (KWin) | ✅ | ✅ | ✅ (KWin ≥ 5.27 기준) |
| Sway | ✅ | ✅ | ✅ |
| Weston | ✅ | ✅ | ✅ |
| Hyprland | ✅ | ✅ | ⚠ 일부 버전 불안정 |
| GNOME Shell (Mutter) | ❌ | ❌ | — (IM은 GNOME Extension 사용) |

> [!NOTE]
> GNOME Shell/Mutter는 `input-method-v2`와 `virtual-keyboard-v1` 모두 미지원이므로 `unim-wayland`는 기동되지 않고,
> GNOME 환경에서는 `unim-gnome-extension`이 대신 입력을 담당합니다. 따라서 `unim-wayland` 내부에서는
> GNOME 환경 감지/분기 로직이 존재하지 않습니다 (Mutter 자체가 프로토콜을 노출하지 않아 진입점이 없음).
>
> Standalone 팝업(GTK 창)은 GNOME + Wayland 조합에서 포커스 스틸링 문제 때문에 GUI 쪽에서 `GnomeWayland` 가드를 적용하지만,
> `unim-wayland`의 `zwp_input_popup_surface_v2` 기반 팝업은 입력 서피스에 앵커되므로 해당 이슈가 없습니다.
> 다만 각 컴포지터별 팝업 위치 보정 차이(커서 사각형 적용 시점 등)는 잔존 이슈입니다.

---

## 2. Wayland 프로토콜 인터페이스

### 2.1 사용 프로토콜 4종

| 인터페이스 | 역할 | 필수 |
|------------|------|------|
| `zwp_input_method_v2` | 입력 방식 상태 관리, 텍스트 커밋/preedit | ✅ |
| `zwp_input_popup_surface_v2` | 한자/특수문자 후보 팝업 서피스 | ✅ |
| `zwp_input_method_keyboard_grab_v2` | 하드웨어 키보드 이벤트 수신 | ✅ |
| `zwp_virtual_keyboard_v1` | 미소비 키를 컴포지터에 재전달 | 옵션 |

### 2.2 더블버퍼링 (Double-Buffering)

`zwp_input_method_v2`는 더블버퍼 패턴을 사용합니다.

```
컴포지터 → Activate (pending)
컴포지터 → Done       ← 이 시점에 pending → current 전환
```

| 필드 | 설명 |
|------|------|
| `pending_activate` | Activate 이벤트 수신됨, Done 대기 중 |
| `pending_deactivate` | Deactivate 이벤트 수신됨, Done 대기 중 |
| `current_active` | 현재 활성 상태 (Done 이후의 실제 상태) |
| `grab_active` | 키 처리 활성 여부 (activate→done: true, deactivate→done: false) |

### 2.3 시리얼 관리

- `serial`은 `Done` 이벤트마다 1씩 증가
- `im.commit(serial)`에 사용하여 상태 변경을 원자적으로 적용
- `commit_string`, `set_preedit_string`은 `commit(serial)` 호출 전에 설정

---

## 3. 수명주기

### 3.1 초기화 (`main.rs`)

1. `DbusClient::new()` → tokio 백그라운드 스레드에서 DBus 클라이언트 시작
2. `Connection::connect_to_env()` → `WAYLAND_DISPLAY` 환경변수로 연결
3. `registry_queue_init()` → 이벤트 큐 + 레지스트리 초기화
4. 글로벌 바인딩:
   - `wl_seat` (v1~9) — **필수**
   - `zwp_input_method_manager_v2` (v1) — **필수**
   - `zwp_virtual_keyboard_manager_v1` (v1) — 옵션
5. `AppState::new(dbus_tx)` → DBus 컨텍스트 생성 (blocking, 1초 타임아웃)
6. `AppState::setup()` → 프로토콜 오브젝트 생성

### 3.2 셋업 (`state.rs::setup`)

```
im_manager.get_input_method(seat) → ZwpInputMethodV2
    ↓
input_method.grab_keyboard() → ZwpInputMethodKeyboardGrabV2
    ↓
vk_manager.create_virtual_keyboard(seat) → ZwpVirtualKeyboardV1  (옵션)
```

### 3.3 이벤트 루프

`mio::Poll` 기반 비동기 이벤트 루프로 Wayland fd와 timerfd를 동시 모니터링합니다.

```
mio::Poll (epoll)
  ├── TOKEN_WAYLAND (Wayland fd)
  │   ├── conn.prepare_read() + guard.read()
  │   └── event_queue.dispatch_pending()
  │       ├── ZwpInputMethodV2 이벤트
  │       │   ├── Activate → pending_activate = true
  │       │   ├── Deactivate → pending_deactivate = true
  │       │   ├── Done → 상태 전환 (활성화/비활성화 적용)
  │       │   └── Unavailable → should_exit = true
  │       └── ZwpInputMethodKeyboardGrabV2 이벤트
  │           ├── Keymap → xkbcommon 키맵 생성 + vk 포워딩
  │           ├── Key → 키 처리 (아래 §4 참조) + 반복 타이머 설정
  │           ├── Modifiers → 수정자 상태 업데이트 + vk 포워딩
  │           └── RepeatInfo → repeat_info 저장
  ├── TOKEN_TIMER (timerfd)
  │   └── handle_repeat_timer() → 키 반복 재처리
  └── should_exit 확인 → 루프 탈출
```

### 3.4 종료

- `Unavailable` 이벤트 수신 시 `should_exit = true`
- 디스패치 오류 발생 시 루프 탈출
- `AppState::drop()` → DBus `DestroyContext` 호출

---

## 4. 키 이벤트 처리

### 4.1 키맵 초기화 (`Keymap` 이벤트)

컴포지터가 `keymap` 이벤트로 XKB 키맵 fd를 전송합니다.

```
keyboard_grab → Keymap { format, fd, size }
  1. virtual_keyboard.keymap(format, fd.as_fd(), size)   ← fd borrow로 포워딩
  2. keymap_handler.update_keymap(fd, size)               ← fd 소유권 이전
     → xkbcommon Keymap::new_from_fd() + State::new()
```

> [!IMPORTANT]
> fd 소유권 문제: `update_keymap()`이 `OwnedFd`를 소비하므로, virtual keyboard 포워딩을 **먼저** 수행 (borrow)한 후 xkbcommon에 소유권을 이전합니다.

### 4.2 수정자 처리 (`Modifiers` 이벤트)

```
keyboard_grab → Modifiers { mods_depressed, mods_latched, mods_locked, group }
  1. keymap_handler.update_modifiers()
     → xkbcommon state.update_mask()
     → XKB modifier → GDK 호환 비트마스크 변환
  2. virtual_keyboard.modifiers() 포워딩
```

**XKB → GDK 비트마스크 변환:**

| XKB 비트 | GDK 상수 | 수정자 |
|----------|----------|--------|
| `0x01` | `GDK_SHIFT_MASK` (1 << 0) | Shift |
| `0x04` | `GDK_CONTROL_MASK` (1 << 2) | Control |
| `0x08` | `GDK_MOD1_MASK` (1 << 3) | Alt |
| `0x40` | `GDK_SUPER_MASK` (1 << 26) | Super |

### 4.3 키 처리 흐름 (`Key` 이벤트)

```
keyboard_grab → Key { time, key(evdev), state }
  ├── Pressed + grab_active:
  │   → process_key_via_dbus(evdev_keycode, time, state)
  │     1. keymap_handler.get_keysym(evdev_keycode) → keysym
  │     2. keycode = evdev_keycode + 8  (XKB 호환)
  │     3. DBus ProcessKey(keysym, keycode, mod_state)  [동기, 500ms 타임아웃]
  │     4. 응답 분기:
  │        ├── consumed=true:
  │        │   → apply_input_result(commit, preedit)
  │        │     → im.commit_string(commit)
  │        │     → im.set_preedit_string(preedit, 0, len)
  │        │     → im.commit(serial)
  │        └── consumed=false:
  │            → virtual_keyboard.key(time, key, state)  [바이패스]
  ├── Released:
  │   → virtual_keyboard.key(time, key, Released)  [항상 포워딩]
  └── Pressed + !grab_active:
      → virtual_keyboard.key(time, key, state)  [바이패스]
```

### 4.4 evdev keycode → keysym 변환 (`keymap.rs`)

```
evdev keycode (Wayland 원시값)
  → +8 오프셋 → XKB keycode
  → xkbcommon state.key_get_one_sym(keycode)
  → XKB keysym (u32)
```

> [!NOTE]
> Wayland은 evdev keycode를 사용하고, XKB/X11은 evdev+8 오프셋을 사용합니다.
> DBus API(`process_key_event`)에는 keysym과 keycode(evdev+8)를 전달합니다.

---

## 5. 텍스트 커밋 및 Preedit

### 5.1 apply_input_result

엔진이 키를 소비한 경우:

```rust
// 1. 커밋 문자열 전송
if !commit.is_empty() {
    im.commit_string(commit);
}
// 2. preedit 업데이트
if !preedit.is_empty() {
    im.set_preedit_string(preedit, cursor_begin=0, cursor_end=len);
} else if last_preedit.is_not_empty() {
    im.set_preedit_string("", 0, 0);  // 클리어
}
// 3. 더블버퍼 적용
im.commit(serial);
```

### 5.2 비활성화 시 조합 커밋 (Focus-out)

```
Deactivate + Done
  → DBus FocusOut → RPC 응답: CommitText { text }
  → im.commit_string(text)       (조합 중이던 텍스트, 비어 있지 않을 때만)
  → im.set_preedit_string("", 0, 0)  (preedit 클리어)
  → im.commit(serial)
  → grab_active = false
```

> [!IMPORTANT]
> Focus-out 커밋은 `FocusOut` **RPC 반환값(`CommitText`)만**을 사용합니다.
> 엔진이 별도로 `CommitText` DBus **시그널**을 발송하지 않으므로(`552b5bd` 이후) 이중 커밋이 발생하지 않습니다.
> 이 규칙은 모든 프론트엔드에 공통이며, Wayland에서도 시그널 브로드캐스트에 기댄 경로는 존재하지 않습니다.

### 5.3 Space 키 (영문 모드)

Wayland 프론트엔드는 Space를 특별 취급하지 않고 그대로 엔진(DBus `ProcessKey`)에 전달합니다.
엔진이 영문 모드에서도 Space에 대해 `consumed=true, commit=" "`를 반환하도록 통일되었으므로
(`fix(ime): commit space in English mode` / `552b5bd`), Wayland 경로에서는 자동으로
`apply_input_result`를 통한 direct commit으로 수렴합니다. 별도 바이패스 경로가 필요하지 않습니다.

### 5.4 키 바이패스

엔진이 키를 소비하지 않거나 DBus 타임아웃 시:

```
virtual_keyboard.key(time, evdev_key, state)
```

> [!WARNING]
> `virtual_keyboard_manager`가 없으면 미소비 키를 포워딩할 수 없습니다.
> 이 경우 영문 키, 화살표, 기능 키 등이 앱에 전달되지 않습니다.

### 5.5 AutoTypeFix 적용

한영 자동 오타 교정은 엔진이 `AutoTypefixApply` DBus 시그널(`delete_chars`, `commit_text`, `preedit_text`)을
브로드캐스트하고, `dbus_client.rs`가 이를 수신해 `PopupEvent::AutoTypeFix`로 메인 루프에 전달합니다.
메인 루프는 `AppState::apply_auto_typefix(delete_chars, commit_text, preedit_text)`를 호출합니다.

```
fn apply_auto_typefix:
  1. is_forward = commit_text 의 첫 글자가 ASCII(영→한) 인지 한글(한→영) 인지 판정
  2. before_bytes = is_forward ? delete_chars (ASCII 1B/char)
                               : delete_chars * 3 (한글 UTF-8 3B/char)
  3. im.delete_surrounding_text(before_bytes, 0)   ← 핵심: 프로토콜 단일 원자 삭제
  4. im.commit_string(commit_text)
  5. im.set_preedit_string(preedit_text, 0, len)   (비어 있으면 클리어)
  6. im.commit(serial)
```

> [!IMPORTANT]
> **self-feedback이 발생하지 않습니다.** XIM/GNOME Extension 계통은 `XTestFakeKeyEvent` 또는 Clutter
> 가상 키보드로 BackSpace를 합성 주입하므로 주입한 BS가 IM으로 재진입하는 문제가 있고, 이를 막기 위해
> `self_backspace_pending` 카운터 / `expectSelfBackspaces` API 같은 우회가 필요합니다
> (참고: `af8b563 gnome-extension: bypass self-sent BackSpace from AutoTypeFix vkbd`).
> 반면 `unim-wayland`는 `input-method-v2`의 `delete_surrounding_text`를 직접 사용하여 컴포지터 측에서
> 원자적으로 주변 텍스트를 삭제하므로 BackSpace 키 주입 자체가 없고, 따라서 self-feedback 우회 로직도
> 필요하지 않습니다. `virtual_keyboard.key()`는 오직 **미소비 키 바이패스**(§5.4)에만 사용됩니다.
>
> 방향 판정은 `commit_text` 첫 문자의 ASCII 여부로만 결정됩니다. 혼합 문자열은 현재 지원하지 않습니다.

---

## 6. DBus 통신 (`dbus_client.rs`)

### 6.1 요청-응답 패턴

```
AppState → tokio::mpsc::Sender<DbusRequest> → DbusClient (tokio 백그라운드 스레드)
                                                  ↕ DBus 비동기 호출
AppState ← std::sync::mpsc::channel       ← DbusClient
```

**동기-비동기 브릿지**: Wayland 이벤트 루프(동기)와 DBus(비동기)를 연결합니다.

- `tokio::sync::mpsc` → 요청 전송 (blocking_send)
- `std::sync::mpsc` → 응답 수신 (recv_timeout, 500ms)

### 6.2 주요 DBus 메서드/시그널

| 요청 (RPC) | 응답 | 용도 |
|------|------|------|
| `CreateContext` | `ContextCreated { path }` | 초기화 시 DBus 컨텍스트 등록 |
| `DestroyContext` | — | 종료 시 (Drop) 컨텍스트 해제 |
| `FocusIn` | — | Activate→Done 시 포커스 알림 |
| `FocusOut` | `CommitText { text }` | Deactivate→Done 시 조합 텍스트 커밋 (RPC 반환값 단일 경로) |
| `ProcessKey` | `KeyProcessed { consumed, preedit, commit }` | 키 입력 처리 |
| `Reset` | — | 상태 초기화 |

| 시그널 (수신) | 페이로드 | 용도 |
|------|------|------|
| `ShowHanja` | `target, candidates` | 한자 후보 팝업 표시 |
| `ShowSpecialChar` | `target, characters` | 특수문자 후보 팝업 표시 |
| `HidePopup` | — | 팝업 닫기 |
| `PopupNavigate` | `direction` | 팝업 내 이동 |
| `AutoTypefixApply` | `delete_chars, commit_text, preedit_text` | AutoTypeFix 교정 적용 (§5.5) |

### 6.3 컨텍스트 관리

- **단일 컨텍스트**: XIM과 달리 Wayland에서는 하나의 `window_id="unim-wayland"` 사용
- **초기화 타임아웃**: 1초 (컨텍스트 생성)
- **키 처리 타임아웃**: 500ms
- **폴백**: DBus 연결 실패 시 로컬 경로 (`/local/context_{timestamp}`) 생성

---

## 7. AppState 구조체

### 7.1 필드 구조

```rust
pub struct AppState {
    // 글로벌 오브젝트 (main.rs에서 설정)
    seat: Option<WlSeat>,
    im_manager: Option<ZwpInputMethodManagerV2>,
    vk_manager: Option<ZwpVirtualKeyboardManagerV1>,

    // 프로토콜 오브젝트 (setup()에서 생성)
    input_method: Option<ZwpInputMethodV2>,
    keyboard_grab: Option<ZwpInputMethodKeyboardGrabV2>,
    virtual_keyboard: Option<ZwpVirtualKeyboardV1>,

    // 더블버퍼 상태
    serial: u32,
    pending_activate / pending_deactivate: bool,
    current_active / grab_active: bool,
    keymap_init: bool,

    // 키 처리
    keymap_handler: KeymapHandler,
    last_preedit: String,
    
    // 키 반복
    repeat_timer: RepeatTimer,
    repeat_info: Option<RepeatInfo>,
    press_state: PressState,
    
    // 팝업
    popup_surface: Option<PopupSurface>,
    popup_state: PopupState,
    hanja_popup: HanjaPopup,
    special_popup: SpecialPopup,

    // DBus
    dbus_tx: mpsc::Sender<DbusRequest>,
    context_path: String,

    // 제어
    should_exit: bool,
    
    // 이벤트 큐 (팝업 렌더링용)
    qh: Option<QueueHandle<Self>>,
}
```

### 7.2 Dispatch 구현 8종

| Dispatch 대상 | 구현 내용 |
|---------------|-----------|
| `WlRegistry` (GlobalListContents) | no-op (`registry_queue_init`이 처리) |
| `WlSeat` | no-op |
| `ZwpInputMethodManagerV2` | no-op (이벤트 없음) |
| `ZwpVirtualKeyboardManagerV1` | no-op (이벤트 없음) |
| `ZwpVirtualKeyboardV1` | no-op (이벤트 없음) |
| `ZwpInputMethodV2` | **핵심**: Activate/Deactivate/Done/Unavailable |
| `ZwpInputMethodKeyboardGrabV2` | **핵심**: Keymap/Key/Modifiers/RepeatInfo |
| `ZwpInputPopupSurfaceV2` | **팝업**: TextInputRectangle (커서 위치 수신) |

---

## 8. 빌드 및 배포

### 8.1 빌드

```bash
cargo build --release -p unim-wayland
```

### 8.2 개발 배포 (`make dev-wayland`)

```bash
make dev-wayland PREFIX=/usr
```

동작:

1. `cargo build --release -p unim-wayland`
2. `pkill -f unim-wayland` (기존 프로세스 종료)
3. `sudo cp target/release/unim-wayland $(LIBEXECDIR)/`

---

## 9. 로깅

`UNIM_DEVELOP=1` 환경변수 설정 시 활성화.

| 모듈명 | 컴포넌트 |
|--------|---------|
| `WAYLAND` | `main.rs`, `state.rs` (수명주기, 키 처리) |
| `WAYLAND_DBUS` | `dbus_client.rs` (DBus 통신) |

로그 예시:

```
[2026/02/18 09:00:00] - [WAYLAND] - Keymap 수신 (format=XkbV1, size=42536)
[2026/02/18 09:00:00] - [WAYLAND] - xkbcommon 키맵 초기화 성공
[2026/02/18 09:00:00] - [WAYLAND] - Done → 활성화 (serial=1)
[2026/02/18 09:00:00] - [WAYLAND_DBUS] - FocusIn: /org/atit/unim/InputContext/1
```

---

## 10. 제한사항 및 향후 계획

### 10.1 구현 완료 기능

| 항목 | 상태 | 설명 |
|------|------|------|
| 키 반복 (Key Repeat) | ✅ 구현 | `mio::Poll` + `nix::sys::timerfd` 기반 (PressState 상태 머신) |
| 한자/특수문자 DBus | ✅ 통합 | DBus 요청/응답 타입 추가 |
| 한자/특수문자 팝업 | ✅ 구현 | `zwp_input_popup_surface_v2` + `tiny-skia` + `cosmic-text` 기반 렌더링 |
| AutoTypeFix | ✅ 구현 | `delete_surrounding_text` 기반 원자 교정 (self-feedback 없음, §5.5) |
| Focus-out 이중 커밋 방지 | ✅ 해결 | `FocusOut` RPC 반환값만 사용 (`552b5bd`) |
| Space 영문 모드 커밋 | ✅ 해결 | 엔진이 `consumed=true, commit=" "` 반환 (`552b5bd`) |

### 10.2 현재 제한사항 / 잔존 이슈

| 항목 | 상태 | 설명 |
|------|------|------|
| Surrounding Text | ❌ 미사용 | 프로토콜 이벤트 수신하나 무시 (AutoTypeFix는 엔진이 결정한 `delete_chars`에 의존) |
| Content Type | ❌ 미사용 | 프로토콜 이벤트 수신하나 무시 |
| GNOME 지원 | ❌ 불가 | Mutter가 프로토콜 미지원 → GNOME Extension 경로 사용 |
| 순수 Wayland 팝업 (일부 컴포지터) | ⚠ 부분 | Hyprland 등 일부 버전에서 `zwp_input_popup_surface_v2` 동작이 불안정 |
| 혼합 ASCII/한글 AutoTypeFix | ❌ 미지원 | `commit_text` 첫 글자로 바이트 계산 방식이 고정되어 있음 |

### 10.3 컴포지터별 주의사항

- **KDE Plasma (KWin)**: `input-method-v2` + 팝업 서피스 모두 정상. 기준 구현.
- **Sway**: 정상 동작. Standalone 팝업(GTK)은 포커스 스틸링 유발 가능 → 기본 팝업 모드 권장.
- **Hyprland**: 프로토콜은 지원하나 버전에 따라 팝업 좌표 보정(`text_input_rectangle`) 타이밍 차이가 있음.
- **Weston**: 레퍼런스 컴포지터. 프로토콜 스펙 검증용으로 사용 가능.
- **GNOME Shell (Mutter)**: `unim-wayland`가 바인딩에 실패하므로 기동되지 않음. `unim-gnome-extension`으로 자동 분기.

### 10.4 향후 계획

| 단계 | 내용 |
|------|------|
| Phase 4 | Surrounding Text / Content Type 활용 |
| — | 혼합 ASCII/한글 AutoTypeFix 바이트 계산 정교화 |

---

## 11. 참조

### 11.1 프로토콜 사양

- [input-method-unstable-v2](https://wayland.app/protocols/input-method-unstable-v2)
- [virtual-keyboard-unstable-v1](https://wayland.app/protocols/virtual-keyboard-unstable-v1)

### 11.2 참조 구현

- [kime (Rust 한국어 IME)](https://github.com/Riey/kime/tree/develop/src/frontends/wayland/src) — `state.rs` 패턴 참조
