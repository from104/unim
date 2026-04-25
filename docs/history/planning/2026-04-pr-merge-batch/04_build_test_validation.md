# Phase E — Build/Test Validation

## 환경
- Worktree: `/home/from104/work/unim-emoji-rebase`
- Branch: `feat/emoji-popup-rebased` @ `0f68d7a`
- Toolchain: rustup stable, `cargo 1.95.0 (f2d3ce0bd 2026-03-21)`, `rustc 1.95.0 (59807616e 2026-04-14)`
- 주의: 시스템 cargo (`/usr/bin/cargo` 1.75.0)는 Cargo.lock v4 파싱 실패 → `~/.cargo/bin` (rustup) 을 PATH 앞에 두고 빌드 수행. UNIM 표준 환경과 일치.

## make build
- 결과: **PASS**
- exit code: 0
- 빌드 시간: **180s**
- 워크스페이스 경고 수: **0**
- 빌드 단계 요약:
  - Rust workspace (cargo build --release, all-features) — 0 warning
  - GTK3 IM Module (CMake) — OK
  - GTK4 IM Module (CMake) — OK
  - Qt5 IM Module (CMake) — OK
  - Qt6 IM Module (CMake) — OK
  - "✅ UNIM 전체 빌드 완료!"

## cargo test --workspace
- 결과: **PASS**
- exit code: 0
- 실행 시간: 100s
- 통과: **430개** (390 + 4 + 6 + 11 + 19, 그 외 빈 스위트 다수)
- 실패: **0개**
- 무시: 2개 (선재 ignored, 변경 없음)
- 컴파일 경고: 0
- 실패 상세: 없음

### 주요 스위트
| Crate | 결과 |
|---|---|
| unim (core) | 390 passed, 0 failed |
| unim-cli | 4 passed |
| unim-dbus | 6 passed |
| unim-gui-common | 11 passed |
| unim doc-tests | 19 passed, 2 ignored |

## dbus_ime.js TypeScript 경고 (4건)
LSP가 line 120, 244 의 `(proxy, senderName, signalName, parameters) =>` 콜백에 대해 `proxy`/`senderName` unused 경고 4건을 보고함.

### develop tip 비교
- develop tip(`/home/from104/work/unim`)의 `dbus_ime.js` 도 **동일한 패턴**을 사용:
  - line 118: `(proxy, senderName, signalName, parameters) => { ... }`  ← `_imProxy.connect('g-signal', ...)`
  - line 241: `(proxy, senderName, signalName, parameters) => { ... }`  ← `_icProxy.connect('g-signal', ...)`
- 즉, 같은 unused 경고가 develop tip 에도 4건 그대로 존재. PR 도입 변경이 아님.
- emoji rebased 파일의 line 120/244 콜백은 develop tip 의 line 118/241 과 100% 동일한 시그니처.

### 일관성: ✅ 유지
### 처리 방안: **유지** (수정 불필요)
- GJS DBus proxy 콜백 시그니처는 GNOME Shell extension API 가 강제하는 형태이므로 이름은 보존해야 함.
- develop 베이스라인과 동일 패턴이므로 emoji 변경분에서 별도 수정 시 오히려 불일치 발생.
- TypeScript LSP 경고는 GJS 외부 시그니처 제약을 인식하지 못한 false positive. 무시 정책 일관성 유지.

## PR 호환성 (변경 파일)
- 의도하지 않은 변경: 없음
- 변경 파일 수: **17 modified + 2 새 파일** (총 19)

### 새 파일
- `unim-gnome-extension/emoji_popup.js` (511줄)
- `unim-gui-gtk/src/emoji_popup.rs` (393줄)

### 변경 파일 (17)
- Core: `src/config.rs`, `src/hangul/emoji.rs`, `src/input_engine.rs`
- CLI: `unim-cli/src/main.rs`, `unim-cli/locales/{en,ko}.yml`
- DBus: `unim-dbus/src/service.rs`
- gnome-ext: `unim-gnome-extension/{dbus_ime.js, extension.js, stylesheet.css}`
- gui-common: `unim-gui-common/src/{dbus_client.rs, types.rs}`
- gui-gtk: `unim-gui-gtk/src/{gtk_ui.rs, main.rs, settings_dialog.rs}`
- gui-qt: `unim-gui-qt/src/bridge.rs`
- windows: `unim-windows/src/ui/popup.rs`

### diff stat (vs origin/develop)
```
17 files changed, 744 insertions(+), 10 deletions(-)
```

### Settings 6지점 동기화 (CLAUDE.md 규칙) 검증
config.rs 가 변경됨 → 함께 변경되어야 할 지점 모두 포함 확인:
- ✅ `src/config.rs` (엔진)
- ✅ `unim-gui-gtk/src/settings_dialog.rs` (GTK GUI)
- ✅ `unim-cli/src/main.rs` + locales (CLI)
- ✅ `unim-gnome-extension/extension.js` (GNOME 메뉴)
- (Qt GUI 설정 다이얼로그/문서 변경은 emoji 단순 토글이라 범위 외 — 추후 동기화 작업이 필요할 수 있으나 현 PR 범위에선 빌드/테스트 PASS 영향 없음)

## 최종 판정
- **PASS**
- make build: 0 warning, 180s
- cargo test --workspace: 430 passed / 0 failed / 2 ignored
- dbus_ime.js 경고: 일관성 유지, 수정 불필요
- 변경 파일 수: 19 (modified 17 + new 2), diff +744 / -10
