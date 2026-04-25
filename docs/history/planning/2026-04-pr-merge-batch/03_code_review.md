# PR #1 코드 리뷰 리포트

- **대상 head**: `acd40f2` (claude/korean-input-windows-gui-y9ZVW)
- **base**: develop (이미 PR 브랜치에 머지됨, d30d9d0)
- **변경 범위**: 25 파일, +5635 / -183 (PR 메타) — `src/build.rs`, `src/keycode.rs`, `Cargo.toml`, `unim-tsf/*` (12 파일), `unim-windows/*` (8 파일), `docs/TSF_IME_PLAN.md`
- **acd40f2 어댑터 패치**: `Cargo.lock` + `unim-windows/src/app.rs` (+18/-12) + `unim-windows/src/ui/popup.rs` (+2/-0)

## 판정: PASS

이전 단계(`02_full_validation.log`)에서 cargo build/test workspace, Windows MSVC cross-check, `make build` 모두 zero-warning PASS. 본 단계는 정성 규약 준수만 검증.

---

## 규약별 검증

### 일반 UNIM 규약
- **unim_log 매크로 사용 (println/eprintln 금지)**: 부분 준수
  - `grep -rn 'println!|eprintln!' unim-windows unim-tsf` → **0건** (위반 없음)
  - 단, `unim_log!` **호출도 0건** — 두 신규 크레이트 모두 로깅 자체가 부재.
    Windows GUI/COM scaffolding 단계에서는 허용 가능하나, 후속 PR에서 추가 권고.
- **Core 분리 원칙**: PASS
  - Core(`src/`)는 플랫폼 의존성을 토글 형태로만 보유:
    - `Cargo.toml:14-16` → `[target.'cfg(unix)'.dependencies] x11=…, libc=…`
    - `src/build.rs:2` → `#[cfg(target_os = "linux")] println!("cargo:rustc-link-lib=X11");`
  - Win32 매핑 함수는 `src/keycode.rs`에 추가됐지만 cfg 가드 없이 항상 컴파일되며,
    이는 의도된 설계(VK 코드 정수 매핑은 플랫폼 OS 콜이 아니라 순수 변환 함수).
  - Linux IM 영역(`unim-frontends/*`, `unim-gui-*`, `unim-daemon`, `unim-dbus`) 0 변경 — 비영향.
- **에러 핸들링 (unwrap 지양)**: 조건부 PASS
  - `unim-windows`: `unwrap()` **0건** (정상).
  - `unim-tsf`: `unwrap()` 다수 — 전부 `Mutex::lock().unwrap()` 또는 일회성 COM 캐스트
    (`composition.rs`, `display_attr.rs`, `lang_bar.rs`, `lib.rs`).
    `Mutex::lock` unwrap은 Rust idiom (poisoning만 가능) → 허용. 비즈니스 로직 unwrap 아님.
- **Memory 규칙 (mimalloc 금지)**: PASS — `grep mimalloc` 0건.
- **per-context HashMap 수명**: 본 PR 영향 없음 (Linux IM frontend 미변경).

### Windows 특수 규약
- **(a) Linux 전용 코드 cfg 가드**: PASS
  - `src/build.rs`의 X11 링크 — `cfg(target_os="linux")` 가드 추가됨.
  - `Cargo.toml`의 x11/libc — `cfg(unix)` 가드 (기존 → 조건부 의존성으로 전환).
- **(b) Windows 전용 코드 cfg 가드**: PASS
  - `unim-tsf/src/lib.rs`: 22+ 곳 `#[cfg(windows)]` 적용, `globals.rs` 6곳 적용.
  - `unim-windows/Cargo.toml:18`: `[target.'cfg(windows)'.dependencies] tray-icon = "0.19"`.
  - `eframe`/`egui`/`image`/`arboard`는 cross-platform 라이브러리 → 무가드 정상.
- **(c) Win32 KeyCode/ModifierState 매핑 단위 테스트**: PASS
  - `src/keycode.rs:1005-1059+` `#[test] fn test_keycode_from_win32_vk` — 알파벳/숫자/제어/특수기호/펑션키/네비/IME(VK_HANGUL=0x15, VK_HANJA=0x19)/Shift LR 모두 검증.
  - ModifierState `from_win32_modifiers`도 src/keycode.rs:879에 존재 (테스트 모듈 내 검증 함수 동일 위치).
- **(d) unim-windows DBus 의존 없음, in-process Core 사용**: PASS
  - `unim-windows/Cargo.toml` deps: `unim` (path), `eframe`, `egui`, `image`, `arboard`, `tray-icon`. **DBus/zbus/zvariant 없음**.
  - app.rs:178/316: `self.engine.typefix_convert(...)` — 직접 `InputEngine` 메서드 호출 (in-process).
- **(e) unim-tsf의 windows-rs/`#[implement]` 매크로 정합성**: PASS (cargo check 통과로 입증)
  - 6개 TSF 인터페이스(ITfTextInputProcessorEx/KeyEventSink/CompositionSink/ThreadMgrEventSink/TextEditSink/DisplayAttributeProvider) 구현.
  - `cargo check --target x86_64-pc-windows-msvc -p unim-tsf` PASS.
- **(f) acd40f2 어댑터 패치의 Core API 시그니처 부합**: PASS
  - `typefix_convert` 시그니처 = `src/input_engine.rs:1426 pub fn typefix_convert(&mut self, direction: u32) -> Option<(i32, u32, String)>` — app.rs의 3-tuple 패턴 매치 정확.
  - `KOREAN_LAYOUT_*` / `ENGLISH_LAYOUT_*`: `src/config.rs:62-66, 128-132`에 `pub const &str`로 정의 → app.rs에서 `to_string()` 변환 후 `String` 필드에 대입 — 타입 정확.
  - `PopupAction::HanjaBookmarkChanged { index, bookmarked }`: `src/input_engine.rs:41`에서 variant 정의 확인 → ui/popup.rs:76 `{ .. } => {}` ignore arm으로 비망라(E0004) 해소. 의미적으로도 standalone egui 팝업은 즐겨찾기 시각 동기화 대상 외(즐겨찾기 변경은 in-engine 상태로만 반영) → 정합.

---

## 발견 사항

- **[INFO]** `unim-windows`/`unim-tsf` 두 크레이트 모두 `unim_log!` 호출 0건.
  현재는 silent. 후속 PR에서 최소한 panic/COM HRESULT 경로에 logging 추가 권고.
  (현 PR은 zero-warning 통과이며 위반은 아님.)
- **[INFO]** `unim-tsf/src/lang_bar.rs:134` `punk.unwrap().cast()` — COM `Option<IUnknown>` unwrap.
  Sink가 항상 nonnull임을 호출 규약상 보장하지만, 방어적으로 `if let Some` 또는 `ok_or` 변환 권고 (후속 PR).
- **[INFO]** acd40f2의 ui/popup.rs:76 `PopupAction::HanjaBookmarkChanged { .. } => {}` 무시 분기는
  주석 한 줄로 의도(standalone egui 팝업은 즐겨찾기 시각 갱신 대상 아님)를 명시하면
  미래 유지보수 시 누락된 처리로 오인되지 않음 — 후속 PR 권고.
- **위반 항목 없음.**

---

## 권고

1. **머지 진행 가능.** 빌드/테스트/규약 모두 통과. acd40f2 어댑터 패치는 Core API 변화에 정확히 추종.
2. (후속 PR) `unim-windows`/`unim-tsf`에 `unim_log!` 도입 — 최소 ERROR 레벨로 COM HRESULT 실패 / panic 경로.
3. (후속 PR) ui/popup.rs:76의 `HanjaBookmarkChanged { .. } => {}` 위에 의도 주석 추가.
4. (후속 PR) `unim-tsf/src/lang_bar.rs:134`의 `punk.unwrap().cast()` 방어적 처리.

## 검증 증거 인덱스
- 빌드/테스트 결과: `_workspace/02_full_validation.log` (이미 작성됨)
- 정적 분석: `_workspace/01_pr_analysis.md`
- Core API 시그니처:
  - `src/input_engine.rs:1426` (typefix_convert)
  - `src/input_engine.rs:16, 41` (PopupAction enum + HanjaBookmarkChanged variant)
  - `src/config.rs:59-74, 125-140` (KoreanLayout/EnglishLayout 타입 + 상수)
- Win32 매핑/테스트: `src/keycode.rs:471, 879, 1005-1059+`
- cfg 가드: `src/build.rs:2`, `Cargo.toml:14`, `unim-windows/Cargo.toml:18`, `unim-tsf/src/lib.rs` 다수
