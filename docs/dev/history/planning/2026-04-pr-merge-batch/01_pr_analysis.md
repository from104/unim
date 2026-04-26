# PR #1 윈도우 분석 리포트

## 메타 정보
- **title**: feat: Add Windows frontend GUI with egui and cross-platform build support
- **base**: `develop`  ←  **head**: `claude/korean-input-windows-gui-y9ZVW` (`e39aa3f`)
- **mergeable**: `MERGEABLE`
- **mergeStateStatus**: `CLEAN`
- **changedFiles**: 25 · **+5635 / -183**
- **CI**: 체크 없음 (no checks reported by `gh pr checks 1`)
- **URL**: https://github.com/from104/unim/pull/1

## 변경 파일 분류 (Windows 카테고리)

| 카테고리 | 파일 수 | +라인 | -라인 |
|---|---:|---:|---:|
| `src/` (Core 엔진 — 크로스플랫폼화) | 2 | 238 | 0 |
| `unim-tsf/` (Windows TSF IME 신규) | 11 | 1398 | 0 |
| `unim-windows/` (egui GUI 신규) | 9 | 891 | 0 |
| `Cargo.toml` / `Cargo.lock` (workspace) | 2 | 2701 | 183 |
| `docs/` (TSF_IME_PLAN.md) | 1 | 407 | 0 |
| 기타 (Linux IM 등) | 0 | 0 | 0 |

세부 내역:
- `src/build.rs` (+1) · `src/keycode.rs` (+237)
- `unim-tsf/`: `Cargo.toml`(+27), `candidate_ui.rs`(+114), `class_factory.rs`(+48), `composition.rs`(+190), `display_attr.rs`(+165), `globals.rs`(+22), `key_handler.rs`(+142), `lang_bar.rs`(+150), `lib.rs`(+116), `register.rs`(+147), `text_service.rs`(+277)
- `unim-windows/`: `Cargo.toml`(+19), `app.rs`(+318), `input_handler.rs`(+145), `main.rs`(+25), `tray.rs`(+27), `ui/main_window.rs`(+192), `ui/mod.rs`(+3), `ui/popup.rs`(+125), `ui/status_bar.rs`(+37)
- `Cargo.toml`(+4/-0), `Cargo.lock`(+2697/-183)
- `docs/dev/plans/TSF_IME_PLAN.md`(+407) — at PR time: `docs/TSF_IME_PLAN.md`

## cfg gate 정합성

> 본 저장소는 루트 `Cargo.toml`이 `[package] name="unim"`(Core 엔진)과 `[workspace]`를 동시에 보유하는 합본 manifest다. 따라서 `src/Cargo.toml`은 존재하지 않으며 모든 검증은 루트 `Cargo.toml` 기준으로 수행했다.

- [OK] **Linux 전용 deps `cfg(unix)` gating** — 루트 `Cargo.toml`에 `[target.'cfg(unix)'.dependencies]` 섹션이 신설되어 `x11 = "2.18.2"`, `libc = "0.2"`가 그 아래로 이동.
- [OK] **`build.rs`의 X11 link `cfg(target_os = "linux")` 가드** — `println!("cargo:rustc-link-lib=X11")` 직전에 `#[cfg(target_os = "linux")]` attribute 추가. 표준 어트리뷰트-on-statement 패턴으로 정합.
- [OK] **`unim-tsf` Windows 전용 deps gating** — `windows-core = "0.58"`, `windows = "0.58"` (TextServices/Foundation/Com/Ole/LibraryLoader/Registry/Variant/WindowsAndMessaging/Gdi/Input_KeyboardAndMouse 피처)가 모두 `[target.'cfg(windows)'.dependencies]` 아래.
- [OK] **`unim-windows` 트레이 deps gating** — `tray-icon = "0.19"` 가 `[target.'cfg(windows)'.dependencies]` 아래. `eframe`/`egui`/`image`/`arboard`는 cross-plat 라이브러리이므로 일반 `[dependencies]` 위치 적정.
- [OK] **Win32 KeyCode/ModifierState 단위 테스트 동봉** — `src/keycode.rs`의 `mod tests`에 `#[test] fn test_keycode_from_win32_vk` 추가 (알파벳/숫자/특수키/OEM 기호/펑션/모디파이어 전반 커버). `from_win32_modifiers` 변환 로직도 동일 모듈에 동봉. PR 설명에 "all 246 core tests pass" 명시.

## Workspace 멤버

- **unim-windows**: OK 추가됨 (`Cargo.toml` `[workspace] members`에 `"unim-windows"` 신규 라인)
- **unim-tsf**:     OK 추가됨 (`Cargo.toml` `[workspace] members`에 `"unim-tsf"` 신규 라인)
- 결과: `cargo check --workspace` 가 두 신규 크레이트를 모두 커버한다.
- Cargo.lock 변경 +2697/-183 — egui/eframe/windows-rs/tray-icon 신규 의존 트리 도입에 따른 정상적 폭. 현행 워크스페이스 규모 대비 비정상적이지 않음.

## Linux IM 비영향

- `unim-gui-gtk/`, `unim-gui-qt/`, `unim-frontends/xim/`, `unim-frontends/wayland/`, `unim-dbus/`, `unim-daemon/`, `unim-gnome-extension/`, `Makefile` — **변경 없음**.
- Linux IM 직접 변경 없음 (Core cfg gate 검증으로 갈음).
- Core 변경분(`src/build.rs`, `src/keycode.rs`)은 모두 _addition only_ (-0)로 기존 Linux 경로를 깨뜨리지 않음. `cfg(target_os="linux")` 게이트가 X11 링크를 보호하고, `cfg(unix)` 게이트가 x11/libc 크레이트를 보호 — Linux 빌드 표면은 동등.

## 충돌 분석

- mergeStateStatus = `CLEAN`, mergeable = `MERGEABLE` → GitHub 측 자동 머지 가능 상태.
- 가벼운 base 정합성 점검:
  - PR base = `develop` 현 HEAD: `5d5b500` (hanja bookmark UI rollout 머지)
  - PR head SHA: `e39aa3f`
  - merge-base: `94efe8d` (develop이 그 이후 44 커밋 진행됨)
- develop이 44커밋 앞서 있지만 변경 영역(Linux IM 프론트엔드)이 본 PR의 변경 영역(Core cfg gate + Windows 신규 크레이트)과 **파일 수준에서 분리**되어 있어 GitHub의 CLEAN 판정과 일치한다. 충돌 없음.

## 머지 진행 가능 여부

**READY**

사유:
1. mergeStateStatus = CLEAN, mergeable = MERGEABLE — GitHub 충돌 없음 확인.
2. cfg gate 정합성 5건 모두 OK — Linux 빌드 표면 보존, Windows 빌드 표면 격리.
3. Workspace 멤버에 `unim-windows`/`unim-tsf` 모두 등재됨.
4. Linux IM(GTK/Qt/XIM/Wayland/DBus/daemon/gnome-extension/Makefile) **직접 변경 0건** — 회귀 위험 없음.
5. Win32 KeyCode/ModifierState 단위 테스트 동봉 (PR 설명 기준 246 core tests 통과).
6. CI 체크는 등록되지 않았으나(저장소에 워크플로우 미설정으로 보임), 정적 분석 책임 범위 내 차단 사유 없음.

### 비차단 권고 (warning)
- CI 워크플로우 부재 — 별도로 build-validator 단계에서 `cargo check --target x86_64-pc-windows-gnu`(PR 본문에 PR 작성자가 수동 검증했다고 기재) 및 `cargo test --workspace` 재현 권장.
- `Cargo.lock` +2697 라인은 egui+windows-rs 도입에 따른 정상 폭이지만, 머지 직전에 develop 최신 HEAD 기준 rebase 또는 merge-from-develop 한 번 권장(현재는 CLEAN이라 강제 아님).
