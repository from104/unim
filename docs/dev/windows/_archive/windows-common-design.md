# `unim-windows-common` — AUTHORITATIVE API contract & migration plan

작성: 2026-06-17 / 브랜치 `feat/windows-msi-redesign`
근거: `docs/dev/windows/windows-common-assessment.md` (결론 = **partial**)

이 문서는 **계약(contract)** 이다. extract 에이전트와 per-consumer(unim-tsf / unim-imm32)
migration 에이전트가 **병렬로** 작업해도 충돌하지 않도록, 공용 크레이트의 정확한 공개
시그니처와 각 consumer의 정확한 편집 범위를 못박는다. 시그니처는 임의로 바꾸지 말 것.

핵심 원칙: **동작을 정확히 보존한다.**
- TSF dbg_log: 파일만(`%TEMP%\unim-tsf.log`), 게이트 = `UNIM_DEBUG_LOG = true` (현행 그대로).
- IMM32 dbg_log: 파일(`%TEMP%\unim-imm32.log`) + `OutputDebugStringW`, 게이트 = `cfg!(debug_assertions)`.
- 게이트(언제 로깅하느냐)는 **consumer 쪽에 남긴다.** 공용 코어는 "쓰라면 쓴다"만 한다.
- `panic = "abort"` 프로파일은 워크스페이스 전역이라 새 멤버에도 자동 적용 — 별도 설정 불필요.

---

## 1. 크레이트 레이아웃

위치: `C:\Users\USER\Desktop\work\unim\unim-windows-common\`

```
unim-windows-common/
  Cargo.toml
  src/
    lib.rs
    registry.rs
    modifier.rs
    debug.rs
```

### `Cargo.toml` (extract 에이전트가 작성)

```toml
[package]
name = "unim-windows-common"
version.workspace = true
edition = "2021"
description = "UNIM Korean IME - shared Win32/COM glue for TSF and IMM32 DLLs"
license.workspace = true
repository.workspace = true
authors.workspace = true

[lib]
# 일반 rlib (기본). cdylib 아님 — unim-tsf / unim-imm32 가 의존하는 보통 라이브러리.
name = "unim_windows_common"

[target.'cfg(windows)'.dependencies]
windows-core = "0.62"
windows = { version = "0.62", features = [
    "Win32_Foundation",
    "Win32_System_LibraryLoader",
    "Win32_System_Registry",
    "Win32_UI_Input_KeyboardAndMouse",
] }
```

feature 집합 = 두 consumer가 **공통으로 쓰는 4종의 UNION** (assessment §7). registry는
`Win32_Foundation` + `Win32_System_Registry`, get_module_path는 `Win32_System_LibraryLoader`,
modifier live wrapper는 `Win32_UI_Input_KeyboardAndMouse`(GetKeyState/VK_*)가 필요하다.
windows-rs feature는 additive 이므로 consumer가 자기 고유 feature(TextServices / UI_Input_Ime)를
계속 켜도 충돌 없음. `[lib] crate-type` 미지정 → 기본 `rlib`.

### 루트 `Cargo.toml` (이 한 줄만 common-크레이트-생성 작업이 소유)

`[workspace] members` 배열에 한 줄 추가 (다른 멤버 줄과 alphabetical 인접 위치 무방):

```toml
    "unim-windows-common",
```

> per-consumer 에이전트는 루트 Cargo.toml을 **건드리지 않는다.** 멤버 등록은 extract 작업의 책임.

### `src/lib.rs`

```rust
//! UNIM Windows 공용 저수준 Win32/COM glue.
//!
//! consumer: `unim-tsf`(TSF cdylib), `unim-imm32`(IMM32 .ime cdylib).
//! Linux/macOS 빌드에는 들어가지 않는다 (모든 모듈 cfg(windows)).
//!
//! 비포함(assessment 근거): popup 와이어타입(serde, 별 크레이트), synth_input(TSF 특화 dead),
//! DllMain hinst 저장(자료구조 비대칭), windows feature 재노출.

#![cfg(windows)]

pub mod registry;
pub mod modifier;
pub mod debug;
```

> `#![cfg(windows)]` crate-level gate: non-windows 타깃에서는 빈 크레이트가 되어 `cargo build`
> (워크스페이스 전체)가 Linux CI에서도 깨지지 않는다. consumer들도 cdylib지만 windows 전용이다.

---

## 2. 모듈별 EXACT 공개 시그니처

### 2.1 `registry.rs`

assessment §1/§2/§3. 셋 다 `windows::core::Result` 반환, 동작 바이트 단위 보존.

```rust
use windows::core::*;
use windows::Win32::Foundation::*;
use windows::Win32::System::Registry::*;

/// REG_SZ 값을 `hkey` 아래에 기록한다. `name`이 `None`이면 기본값(unnamed).
/// (tsf register.rs:68-86 / imm32 register.rs:47-65 — 바이트 동일, as-is 이동)
pub fn set_reg_value(hkey: HKEY, name: Option<&HSTRING>, value: &str) -> Result<()>;

/// REG_DWORD 값을 `hkey` 아래에 기록한다.
/// (tsf register.rs:89-104 — as-is 이동. imm32는 향후 등록 확장 시 사용)
pub fn set_reg_dword(hkey: HKEY, name: &HSTRING, value: u32) -> Result<()>;

/// 주어진 모듈 핸들의 파일 시스템 경로를 반환한다 (GetModuleFileNameW, 260 buf).
/// HMODULE을 인자로 받아 호출자가 자기 hinst(tsf dll_instance / imm32 ime_state::hinst)를
/// 넘긴다. (tsf register.rs:21-32 / imm32 register.rs:30-43 — 핸들 소스만 차이)
pub fn get_module_path(hmodule: HMODULE) -> Result<String>;
```

구현 본문은 원본을 그대로 옮긴다. `get_module_path`는 `unsafe`를 함수 내부에 둔다(현행과
동일하게 호출자에게 unsafe를 요구하지 않음). 본문:

```rust
pub fn get_module_path(hmodule: HMODULE) -> Result<String> {
    let mut buf = [0u16; 260];
    let len = unsafe {
        windows::Win32::System::LibraryLoader::GetModuleFileNameW(Some(hmodule), &mut buf)
    };
    if len == 0 {
        return Err(E_FAIL.into());
    }
    Ok(String::from_utf16_lossy(&buf[..len as usize]))
}
```

### 2.2 `modifier.rs`

assessment §5. **하나의 코어 + 두 얇은 wrapper.** 코어는 down/toggled 두 클로저를 받는다
(`Fn(u16) -> bool`). bit 의미·VK 집합·조립은 두 consumer에서 글자 단위 동일하므로 코어 1개로
통일된다. `ModifierState`는 코어 crate(`unim::keycode`) 타입을 그대로 쓴다 → common이 `unim`에
의존하지 않도록, **타입을 common이 정의하지 않고** 호출자가 조립... 은 dedup을 깨므로,
common이 `unim`에 의존한다 (아래 Cargo 메모 참조).

```rust
use unim::keycode::ModifierState;

/// 코어: 수정자 상태를 두 프로브로부터 조립한다.
/// `down(vk)` = 해당 VK가 눌려 있는가(bit7), `toggled(vk)` = 토글 on 인가(bit0).
/// VK 집합/조립 규칙은 TSF·IMM32 공통. (assessment §5)
pub fn modifier_state_from(
    down: impl Fn(u16) -> bool,
    toggled: impl Fn(u16) -> bool,
) -> ModifierState {
    use windows::Win32::UI::Input::KeyboardAndMouse::*;
    ModifierState {
        shift: down(VK_SHIFT.0),
        control: down(VK_CONTROL.0),
        alt: down(VK_MENU.0),
        super_key: down(VK_LWIN.0) || down(VK_RWIN.0),
        caps_lock: toggled(VK_CAPITAL.0),
        num_lock: toggled(VK_NUMLOCK.0),
    }
}

/// TSF wrapper — live `GetKeyState`. (tsf key_handler.rs:20-38 대체)
pub fn modifier_state_live() -> ModifierState {
    use windows::Win32::UI::Input::KeyboardAndMouse::GetKeyState;
    modifier_state_from(
        |vk| unsafe { GetKeyState(vk as i32) } < 0,
        |vk| (unsafe { GetKeyState(vk as i32) } & 0x01) != 0,
    )
}

/// IMM32 wrapper — `lpbKeyState` 256바이트 배열. null → all-false.
/// (imm32 input.rs:45-61 대체)
///
/// # Safety
/// `key_state`는 읽을 수 있는 256바이트 배열을 가리켜야 한다(IMM32 콜백 계약).
pub unsafe fn modifier_state_from_key_array(key_state: *const u8) -> ModifierState {
    if key_state.is_null() {
        return ModifierState::default();
    }
    modifier_state_from(
        |vk| unsafe { (*key_state.add(vk as usize) & 0x80) != 0 },
        |vk| unsafe { (*key_state.add(vk as usize) & 0x01) != 0 },
    )
}
```

> 시그니처 차이 메모: IMM32 원본은 `pub fn get_modifier_state(*const u8)`(non-unsafe, null
> 가드 내장). 공용판은 `unsafe pub fn modifier_state_from_key_array`로 정한다 — raw 포인터
> 역참조를 노출하므로 unsafe 마킹이 정직하다. consumer는 호출부를 `unsafe { ... }`로 감싼다
> (입력경로는 이미 unsafe 블록 다수라 영향 미미). null 가드는 보존.

**common → unim 의존**: `modifier.rs`가 `unim::keycode::ModifierState`를 반환하려면 common이
`unim`에 의존해야 한다. 이는 단방향(common → unim)이라 사이클 없음. Cargo.toml에 추가:

```toml
[dependencies]
unim = { path = ".." }
```

(이 줄은 §1 Cargo.toml 스니펫의 `[dependencies]` 섹션에 포함시킬 것 — extract 에이전트 책임.)

### 2.3 `debug.rs`

assessment §4. **게이트는 호출자에 남기고**, 코어는 파일 append + 옵션 OutputDebugStringW만.
컴포넌트 라벨·파일명·debug-string 토글을 전부 파라미터로 받는다 → `unim-tsf.log` 하드코딩 제거.

```rust
/// 진단 로그 한 줄을 `%TEMP%\{file}` 에 append 한다. `also_output_debug_string`이면
/// `OutputDebugStringW`에도 쓴다. 태그 라인 형식: `[{component} {PID}] {msg}`.
///
/// **게이트 없음**: 이 함수는 호출되면 무조건 쓴다. ON/OFF 판단(UNIM_DEBUG_LOG /
/// cfg!(debug_assertions))은 consumer의 얇은 래퍼에서 한다 (동작 보존).
/// 실패해도 무시(크래시 없음).
///
/// - `component`: 로그 태그 라벨 (예: "unim-tsf", "unim-imm32").
/// - `file`:      `%TEMP%` 하위 파일명 (예: "unim-tsf.log").
/// - `msg`:       로그 본문.
/// - `also_output_debug_string`: true면 OutputDebugStringW 동시 출력(IMM32 동작).
pub fn dbg_log(component: &str, file: &str, msg: &str, also_output_debug_string: bool) {
    if also_output_debug_string {
        #[link(name = "kernel32")]
        extern "system" {
            fn OutputDebugStringW(lpoutputstring: windows::core::PCWSTR);
        }
        let tagged = format!("[{component} {}] {msg}\0", std::process::id());
        let wide: Vec<u16> = tagged.encode_utf16().collect();
        unsafe { OutputDebugStringW(windows::core::PCWSTR(wide.as_ptr())); }
    }

    use std::io::Write;
    let path = std::env::temp_dir().join(file);
    if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(path) {
        let _ = writeln!(f, "[{component} {}] {msg}", std::process::id());
    }
}
```

> 형식 100% 보존: 태그 `[unim-tsf {pid}] {msg}` / `[unim-imm32 {pid}] {msg}`, PID = `process_tag()`
> = `std::process::id()`. OutputDebugStringW의 수동 FFI 선언(Diagnostics_Debug feature 없이)도
> 그대로 유지. IMM32는 OutputDebugString을 먼저, 파일을 나중에 쓰던 순서 보존.

---

## 3. per-consumer 마이그레이션 (각자 자기 크레이트 + 자기 Cargo.toml만 편집)

두 작업은 서로 독립이며 루트 Cargo.toml을 건드리지 않는다. 호출부 변경을 최소화하기 위해
**각 consumer의 `register.rs`에 동일 시그니처의 얇은 로컬 `dbg_log(&str)` 래퍼를 남긴다**
→ 132/34개의 기존 `dbg_log(...)`/`register::dbg_log(...)` 호출부를 **하나도 안 고친다.**

### 3.1 unim-tsf

**Cargo.toml** — `[target.'cfg(windows)'.dependencies]` 에 추가:
```toml
unim-windows-common = { path = "../unim-windows-common" }
```

**`unim-tsf/src/register.rs`** 편집:
- 삭제: `fn get_dll_path()`(21-32) 본문 → common 호출로 교체:
  ```rust
  fn get_dll_path() -> Result<String> {
      unim_windows_common::registry::get_module_path(
          windows::Win32::Foundation::HMODULE(crate::dll_instance().0)
      )
  }
  ```
  (현행 `crate::dll_instance()`가 이미 HMODULE이면 그대로 전달. 타입 확인 후 캐스팅 정리.)
- 삭제: `fn set_reg_value`(68-86), `fn set_reg_dword`(89-104). 같은 파일 내 호출부
  (`register_com_server`, `register_server`)를 `unim_windows_common::registry::set_reg_value(...)`
  / `::set_reg_dword(...)` 로 치환. (use 별칭 `use unim_windows_common::registry::{set_reg_value, set_reg_dword};` 추가하면 호출부 텍스트 무변경.)
- 삭제: `const UNIM_DEBUG_LOG`(191) **는 남긴다**(게이트). `fn dbg_log`(196-205) 본문 교체:
  ```rust
  pub(crate) const UNIM_DEBUG_LOG: bool = true;   // 게이트 유지(현행 그대로)
  pub(crate) fn dbg_log(msg: &str) {
      if !UNIM_DEBUG_LOG { return; }
      unim_windows_common::debug::dbg_log("unim-tsf", "unim-tsf.log", msg, false);
  }
  ```
- 삭제: `fn process_tag`(208-210) — common이 PID를 내부 처리하므로 불필요.
- 손대지 않음: `launch_settings_app`, `register_com_server`, `register_server`,
  `set_as_default`, `get_default_on_startup`, `set_default_on_startup`,
  `unregister_*` (registry 헬퍼 호출만 경로 치환).

**`unim-tsf/src/key_handler.rs`** 편집:
- 삭제: `fn get_modifier_state()`(20-38) 본문 교체(시그니처/이름 유지 → 호출부 무변경):
  ```rust
  pub fn get_modifier_state() -> ModifierState {
      unim_windows_common::modifier::modifier_state_live()
  }
  ```
- 호출부(`test_key_down` 등) 무변경. `ModifierState` import 그대로.

> 기타 7개 파일(text_service/popup_ipc/auto_typefix/composition/compartment/synth_input)의
> `dbg_log`/`get_modifier_state` 호출은 로컬 래퍼를 거치므로 **편집 불필요.**

### 3.2 unim-imm32

**Cargo.toml** — `[target.'cfg(windows)'.dependencies]` 에 추가:
```toml
unim-windows-common = { path = "../unim-windows-common" }
```

**`unim-imm32/src/register.rs`** 편집:
- 삭제: `fn get_dll_path()`(30-43) 본문 교체:
  ```rust
  fn get_dll_path() -> Result<String> {
      let raw = crate::ime_state::hinst();
      let hmodule = windows::Win32::Foundation::HMODULE(raw as *mut core::ffi::c_void);
      unim_windows_common::registry::get_module_path(hmodule)
  }
  ```
- 삭제: `fn set_reg_value`(47-65). 호출부(`register_ime`)는
  `use unim_windows_common::registry::set_reg_value;` 추가로 텍스트 무변경.
  (set_reg_dword는 imm32에 현재 없음 — 추가 불필요.)
- 삭제: `const UNIM_DEBUG_LOG`(240) **유지**(게이트). `fn dbg_log`(245-268) 본문 교체:
  ```rust
  pub(crate) const UNIM_DEBUG_LOG: bool = cfg!(debug_assertions);  // 게이트 유지
  pub fn dbg_log(msg: &str) {
      if !UNIM_DEBUG_LOG { return; }
      unim_windows_common::debug::dbg_log("unim-imm32", "unim-imm32.log", msg, true);
  }
  ```
  (`also_output_debug_string = true` → OutputDebugStringW 동작 보존. `pub` 가시성 유지.)
- 삭제: `fn process_tag`(271-273).

**`unim-imm32/src/input.rs`** 편집:
- 삭제: 로컬 `const VK_*`(20-26), `const KS_DOWN/KS_TOGGLED`(29-31) — common이 소유.
- `fn get_modifier_state(key_state: *const u8)`(45-61) 본문 교체(시그니처/이름 유지):
  ```rust
  pub fn get_modifier_state(key_state: *const u8) -> ModifierState {
      unsafe { unim_windows_common::modifier::modifier_state_from_key_array(key_state) }
  }
  ```
  (공용판이 `unsafe`이므로 래퍼에서 `unsafe {}`로 감싼다. null 가드는 공용판 내부 보존 →
  외부 동작 동일.) 호출부(`should_consume`) 무변경.

---

## 4. 검증 (각 에이전트 self-check)

- `cargo build -p unim-windows-common` (windows) — 경고 0.
- `cargo build -p unim-tsf` / `cargo build -p unim-imm32` — cdylib 링크 OK, 경고 0.
- 동작 보존 grep: `unim-tsf.log` 문자열은 tsf register.rs 래퍼 1곳, `unim-imm32.log`는 imm32
  register.rs 래퍼 1곳에만 남아야 한다(common엔 없음 — 파라미터로 받음).
- dbg_log 게이트: tsf=`true`, imm32=`cfg!(debug_assertions)` 가 각 register.rs에 유지.

## 5. 비포함 (assessment 확정 — 이번 작업에서 손대지 말 것)

- popup 와이어타입(RenderState/WireCell/WireMsg/RevEvent) → 후속 `unim-popup-wire`(serde-only) 별 PR.
- `synth_input.rs`(TSF 특화 dead) / DllMain hinst 저장(자료구조 비대칭) / windows feature 재노출.
- `unim-tsf-settings`, `unim-popup-win` 은 consumer 아님 (raw Win32 glue 미공유).
