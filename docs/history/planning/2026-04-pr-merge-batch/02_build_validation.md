# PR #1 빌드 검증 리포트

> head=`claude/korean-input-windows-gui-y9ZVW` (`e39aa3f`) · base=`develop` (`5d5b500`)
> 검증 시점: 2026-04-25 · 검증자: windows-build-validator

## 결과 요약

| 축 | 결과 | 비고 |
|---|---|---|
| LINUX_TEST  | **FAIL** | `unim-windows`/`unim-daemon` 12+ 컴파일 에러 (develop API 변경 미반영) |
| LINUX_BUILD | **FAIL** | `Cargo.lock` 중복 키 + warning 1건 (실패로 카운트 무의미) |
| WIN_BUILD   | **PASS** | 채택 방식: **3b. msvc** (`cargo check --target x86_64-pc-windows-msvc -p unim -p unim-capi -p unim-windows -p unim-tsf` → `Finished dev`) |
| CI_STATUS   | **no-workflow** | `gh pr checks 1` → "no checks reported", `gh run list` → `[]` |

> 머지 시뮬레이션은 Git 레벨에서 충돌 없이 자동 머지 성공(`Cargo.lock`/`Cargo.toml`/`src/keycode.rs` 자동 병합), 그러나 **API/락 시멘틱 머지 충돌**이 발생.

---

## 환경 정보

- **호스트**: Ubuntu 24.04 (`Linux gofu 6.17.0-22-generic`)
- **활성 cargo (PATH 우선순위)**: `/usr/bin/cargo` = cargo **1.75.0** (apt) — Cargo.lock v4 파싱 불가
- **rustup stable**: cargo **1.95.0 / rustc 1.95.0** (`~/.rustup/toolchains/stable-x86_64-unknown-linux-gnu/bin/cargo`) — 본 검증의 실제 빌드 도구
- **설치 타겟**: `x86_64-pc-windows-msvc`, `x86_64-unknown-linux-gnu`
- **mingw**: ✗ (`x86_64-w64-mingw32-gcc` not found) → 3a 불가, 3b 채택
- **rust-toolchain pin**: 없음 (저장소에 `rust-toolchain.toml` 미존재)

---

## Linux 회귀

### cargo test --workspace (rustup +stable)
- **결과**: FAIL · `error: could not compile unim-windows (bin "unim-windows" test) due to 12 previous errors`
- **로그**: `_workspace/02_test_log_linux.txt`
- **실패 카테고리** (모두 `unim-windows`, base develop의 신규 API 미반영):
  1. `unim::config::KoreanLayout::{Dubeolsik,Sebeolsik390,Sebeolsik391,SebeolsikNoShift}` — 5개 enum variant 미해결 (E0599 · `app.rs:238–241`)
     - 컴파일러 메시지: "associated item not found in `String`" → develop에서 `KoreanLayout`/`EnglishLayout`이 enum→`String` 또는 newtype-of-`String`으로 재설계된 것으로 추정
  2. `unim::config::EnglishLayout::{Qwerty,Dvorak,ColemakDh,Workman}` — 동일 (E0599 · `app.rs:256–260`)
  3. `engine.typefix_convert(...)` 반환 타입 변경 — `Option<(i32, u32, String)>`인데 PR 코드는 2-tuple 패턴 매치 (E0308 · `app.rs:177`, `app.rs:313`)
  4. `match action` non-exhaustive — `PopupAction::HanjaBookmarkChanged { .. }` 미커버 (E0004 · `unim-windows/src/ui/popup.rs:37`). develop의 한자 북마크 UI 작업으로 신규 추가된 variant
- **부수 영향**: `unim-daemon`도 동일 `PopupAction` 미커버 가능성(컴파일 미도달로 미관측)

### make build
- **결과**: FAIL · `make: *** [Makefile:99: build-rust] 오류 101`
- **로그**: `_workspace/02_build_log_linux.txt`
- **warning 카운트**: `^warning:` = **1건** (실패 빌드의 부수 출력, zero-warning 정책 검증은 무의미)
- **루트 원인**: Makefile의 `CARGO ?= $(shell which cargo)` 가 PATH 우선의 `/usr/bin/cargo` (cargo 1.75.0, Ubuntu apt) 를 잡으면 `Cargo.lock` v4 파싱 불가 → 후속 빌드 단계가 임시 상태에서 `include_str!` 경로 해석 실패로 이어짐 (`couldn't read src/keystroke/profile/../keymap/*.json`). 디스크의 `src/keystroke/keymap/*.json`은 정상 존재. 즉 사용자 환경에서 `make build`는 시스템 cargo로 실행되며 본 PR 머지 후 동일 에러 재현됨
- **2차 검증**: rustup +stable 로 직접 `cargo build --release -p unim` 실행 시 `Cargo.lock`이 `tikv-jemalloc-sys` 중복 entry로 거부됨 (`package "tikv-jemalloc-sys" is specified twice in the lockfile`). 이는 **머지 자체가 만든 Cargo.lock 손상** — develop과 PR 양쪽이 jemalloc 관련 의존을 서로 다른 형태로 추가한 결과
- **결론**: 머지 후 첫 작업으로 `cargo update --workspace` 또는 `Cargo.lock` 재생성 필요

---

## Windows cross-compile (3b: msvc)

- **명령**: `cargo +stable check --target x86_64-pc-windows-msvc -p unim -p unim-capi -p unim-windows -p unim-tsf`
- **결과**: ✅ `Finished dev profile [unoptimized + debuginfo] target(s) in 15.65s`
- **로그**: `_workspace/02_build_log_windows.txt`
- **검사된 크레이트**: `unim`, `unim-capi`, `unim-tsf`, `unim-windows` 모두 통과
- **주요 의존 컴파일/체크**: `windows-core 0.58`, `windows 0.58`, `windows-implement/-interface 0.58`, `tray-icon 0.19`, `eframe 0.31`, `egui 0.31`, `egui-winit`, `accesskit_windows 0.24`, `glutin 0.32`, `glutin-winit 0.5`, `clipboard-win 5.4`, `winapi 0.3.9`. 의존 충돌 없음
- **특이사항**: `cargo check`는 .rmeta 만 생산(linker 미실행) 이므로 msvc linker 부재가 문제되지 않음. 컴파일 단계 통과 자체가 의미 있는 신호

> ⚠️ msvc `cargo check`는 **PR head에서 발견되는 develop API 부정합을 노출하지 않는다** — `unim-windows`가 의존하는 `unim::config::KoreanLayout` enum 등이 PR head 시점의 시그니처를 사용하므로 통과. 실제 머지 후 사용자 환경에서는 위 LINUX_TEST 섹션의 12개 에러가 그대로 windows 타겟에서도 발생할 가능성이 높음(타겟 무관한 src/ Rust 코드 모순). 즉 본 PASS는 "PR 단독 빌드 표면이 windows target에서 cross-compile 가능"을 의미할 뿐, "develop+PR 머지본이 windows에서 빌드된다"는 보장이 아님

---

## CI 비교

- **`gh pr checks 1`**: `no checks reported on the 'claude/korean-input-windows-gui-y9ZVW' branch`
- **`gh run list --branch claude/korean-input-windows-gui-y9ZVW --limit 3`**: `[]`
- **상태**: GitHub Actions 워크플로우 미등록 → CI 갈음 검증 불가 → 로컬 검증이 단일 진실 공급원

---

## 권고 (비차단/차단)

### 차단 (머지 전 반드시 해소)

1. **`unim-windows` API 정합성 패치**: `KoreanLayout`/`EnglishLayout` 신규 시그니처(`String` newtype 또는 stringly-typed) 채택, `typefix_convert` 3-tuple 반환 적응, `PopupAction::HanjaBookmarkChanged` 매치 암(arm) 추가. 동일 패턴이 `unim-daemon`에서도 필요할 수 있음
2. **`Cargo.lock` 재생성**: 머지 후 `cargo update --workspace` 또는 `rm Cargo.lock && cargo generate-lockfile` 로 `tikv-jemalloc-sys` 중복 제거

### 비차단 (별도 작업)

3. **CI 워크플로우 신설** — develop/main에 cargo test + windows cross-compile 잡 추가. 본 PR이 두 번째 OS 타겟을 도입하는 첫 PR이므로 적기
4. **Makefile `CARGO` 우선순위** — `$(shell which cargo)` 가 apt cargo를 잡지 않도록 `~/.cargo/bin/cargo` 우선 또는 `rust-toolchain.toml` 추가 검토

---

## 산출물

- `_workspace/02_build_validation.md` (본 문서)
- `_workspace/02_test_log_linux.txt` — `cargo +stable test --workspace` 전체 로그
- `_workspace/02_build_log_linux.txt` — `make build` 전체 로그
- `_workspace/02_build_log_windows.txt` — `cargo +stable check --target x86_64-pc-windows-msvc ...` 전체 로그

## 머지 진행 가능 여부

**BLOCK** — Linux 회귀(LINUX_TEST FAIL, LINUX_BUILD FAIL) 가 단일 PR 책임 범위(API 진화 미추종) 내 결함이므로, base develop과의 정합 작업 없이 머지하면 develop 자체가 빌드 불가 상태로 진입. PR 작성자에게 develop merge-in 후 컴파일 에러 수정을 요청해야 한다. WIN_BUILD PASS는 PR 단독 표면 검증으로 한정.
