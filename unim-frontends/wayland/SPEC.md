# UNIM Wayland 프론트엔드 세부 기능 명세

> Wayland 환경에서 한국어 입력을 제공하는 입력 방식(Input Method) 클라이언트의 상세 동작을 정의합니다.
> `input-method-unstable-v2` 프로토콜을 사용합니다.

---

## 1. 아키텍처 개요

### 1.1 컴포넌트 구성

| 파일 | 줄 수 | 역할 |
|------|-------|------|
| `main.rs` | ~125 | Wayland 연결, 글로벌 바인딩, 프로토콜 셋업, 이벤트 루프 |
| `state.rs` | ~509 | AppState 구조체, Dispatch 구현 7종, 키 이벤트 처리 |
| `keymap.rs` | ~121 | xkbcommon 키맵 핸들러, evdev keycode → keysym 변환 |
| `dbus_client.rs` | ~225 | unim-daemon과의 비동기 DBus 통신 |

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

### 1.4 지원 컴포지터

| 컴포지터 | `input-method-v2` | `virtual-keyboard-v1` |
|----------|--------------------|-----------------------|
| KDE (KWin) | ✅ | ✅ |
| Sway | ✅ | ✅ |
| Weston | ✅ | ✅ |
| Hyprland | ✅ | ✅ |
| GNOME (Mutter) | ❌ | ❌ |

> [!NOTE]
> GNOME은 `input-method-v2`를 지원하지 않으므로 별도의 GNOME Extension 기반 입력 방식을 사용합니다.

---

## 2. Wayland 프로토콜 인터페이스

### 2.1 사용 프로토콜 3종

| 인터페이스 | 역할 | 필수 |
|------------|------|------|
| `zwp_input_method_v2` | 입력 방식 상태 관리, 텍스트 커밋/preedit | ✅ |
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

```
blocking_dispatch() → Wayland 이벤트 대기 및 Dispatch 호출
    ├── ZwpInputMethodV2 이벤트
    │   ├── Activate → pending_activate = true
    │   ├── Deactivate → pending_deactivate = true
    │   ├── Done → 상태 전환 (활성화/비활성화 적용)
    │   └── Unavailable → should_exit = true
    ├── ZwpInputMethodKeyboardGrabV2 이벤트
    │   ├── Keymap → xkbcommon 키맵 생성 + vk 포워딩
    │   ├── Key → 키 처리 (아래 §4 참조)
    │   ├── Modifiers → 수정자 상태 업데이트 + vk 포워딩
    │   └── RepeatInfo → (Phase 2 예정)
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

### 5.2 비활성화 시 조합 커밋

```
Deactivate + Done
  → DBus FocusOut → 응답: CommitText { text }
  → im.commit_string(text)       (조합 중이던 텍스트)
  → im.set_preedit_string("", 0, 0)  (preedit 클리어)
  → im.commit(serial)
  → grab_active = false
```

### 5.3 키 바이패스

엔진이 키를 소비하지 않거나 DBus 타임아웃 시:

```
virtual_keyboard.key(time, evdev_key, state)
```

> [!WARNING]
> `virtual_keyboard_manager`가 없으면 미소비 키를 포워딩할 수 없습니다.
> 이 경우 영문 키, 화살표, 기능 키 등이 앱에 전달되지 않습니다.

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

### 6.2 주요 DBus 메서드

| 요청 | 응답 | 용도 |
|------|------|------|
| `CreateContext` | `ContextCreated { path }` | 초기화 시 DBus 컨텍스트 등록 |
| `DestroyContext` | — | 종료 시 (Drop) 컨텍스트 해제 |
| `FocusIn` | — | Activate→Done 시 포커스 알림 |
| `FocusOut` | `CommitText { text }` | Deactivate→Done 시 조합 텍스트 커밋 |
| `ProcessKey` | `KeyProcessed { consumed, preedit, commit }` | 키 입력 처리 |
| `Reset` | — | 상태 초기화 |

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

    // DBus
    dbus_tx: mpsc::Sender<DbusRequest>,
    context_path: String,

    // 제어
    should_exit: bool,
}
```

### 7.2 Dispatch 구현 7종

| Dispatch 대상 | 구현 내용 |
|---------------|-----------|
| `WlRegistry` (GlobalListContents) | no-op (`registry_queue_init`이 처리) |
| `WlSeat` | no-op |
| `ZwpInputMethodManagerV2` | no-op (이벤트 없음) |
| `ZwpVirtualKeyboardManagerV1` | no-op (이벤트 없음) |
| `ZwpVirtualKeyboardV1` | no-op (이벤트 없음) |
| `ZwpInputMethodV2` | **핵심**: Activate/Deactivate/Done/Unavailable |
| `ZwpInputMethodKeyboardGrabV2` | **핵심**: Keymap/Key/Modifiers/RepeatInfo |

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

### 10.1 현재 제한사항

| 항목 | 상태 | 설명 |
|------|------|------|
| 키 반복 (Key Repeat) | ❌ 미구현 | `RepeatInfo` 수신만 하고 처리하지 않음 |
| 한자 팝업 | ❌ 미구현 | XIM과 달리 Wayland에서는 별도 팝업 전략 필요 |
| 특수문자 팝업 | ❌ 미구현 | 한자 팝업과 동일 |
| Surrounding Text | ❌ 미사용 | 프로토콜 이벤트 수신하나 무시 |
| Content Type | ❌ 미사용 | 프로토콜 이벤트 수신하나 무시 |
| GNOME 지원 | ❌ 불가 | Mutter가 프로토콜 미지원 |

### 10.2 향후 계획

| 단계 | 내용 |
|------|------|
| Phase 2 | 키 반복 구현 (`mio` + `timerfd` 기반 타이머) |
| Phase 3 | 한자/특수문자 팝업 (Wayland layer-shell 또는 popup surface) |
| Phase 4 | Surrounding Text / Content Type 활용 |

---

## 11. 참조

### 11.1 프로토콜 사양

- [input-method-unstable-v2](https://wayland.app/protocols/input-method-unstable-v2)
- [virtual-keyboard-unstable-v1](https://wayland.app/protocols/virtual-keyboard-unstable-v1)

### 11.2 참조 구현

- [kime (Rust 한국어 IME)](https://github.com/Riey/kime/tree/develop/src/frontends/wayland/src) — `state.rs` 패턴 참조
