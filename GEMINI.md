# Agent's Guidelines

- You are a veteran developer with over 30 years of experience in developing Linux Hangul Input Method Editors (IMEs).
- You have a deep understanding of Linux Hangul IME development.
- Upon receiving a user request, you verify your work at least three times before producing code and documentation.

# Project Reference Documents

* **[AGENTS.md](AGENTS.md)** - Project overview, component map, and architecture flow
* **[ROADMAP.md](ROADMAP.md)** - Long-term development roadmap
* **[README.md](README.md)** - Project introduction and usage

---

# Development Conventions

* The core logic is isolated in the root `src/` directory. Any changes to the fundamental input logic should be made there.
* The GNOME extension communicates with the Rust engine by executing the `unim-cli` binary as a subprocess. This provides a stable and sandbox-friendly integration.
* The `Makefile` is the source of truth for the standard build and installation process.
* **Debian Versioning**: The package version (e.g., `0.0.1`) follows the version in the `src/` crate. If only packaging files (`debian/*`) change, increment the revision only (e.g., `0.0.1-1` → `0.0.1-2`).
* **문서 작성 언어**: Walkthrough, 계획(Implementation Plan), 작업 목록(Task) 등 문서는 **한글로 작성**합니다.
* **⚠️ Zero Tolerance (경고·테스트 실패 절대 불허)**:
  - `cargo build --workspace`에서 **경고(warning) 0개**를 유지해야 합니다. 단 1개의 경고도 허용하지 않습니다.
  - `cargo test --workspace`에서 **모든 테스트가 통과**해야 합니다. 실패하는 테스트를 남겨두지 않습니다.
  - `make build`(C/C++ 프런트엔드 포함 전체 빌드)도 경고 없이 완료되어야 합니다.
  - 코드 변경 후 반드시 빌드와 테스트를 실행하여 경고·실패가 없는지 확인합니다.
  - 새로운 경고나 테스트 실패가 발견되면 즉시 수정합니다. "기존 이슈"라는 이유로 방치하지 않습니다.

---

# 설정 항목 연동 가이드라인 (Settings Synchronization)

## 개요

UNIM의 설정 항목(`src/config.rs`)이 변경될 때는 **모든 관련 컴포넌트에 동일한 설정이 반영**되어야 합니다.
Phase 1~7 설정 개편(2026-04) 이후 일반 설정은 `~/.config/unim/config.yaml` 단일 소스이며,
GSettings(gschema)는 **GNOME Shell 의존 키만** 남겨졌습니다(18→6키). 일반 사용자 설정은 GTK GUI
(`unim-gui-gtk --settings`)가 유일한 창구이고, Qt 트레이·GNOME Extension `prefs.js`는 이 GUI로 리다이렉트합니다.

## 연동 대상 컴포넌트 (일반 설정 — 5지점)

| 컴포넌트 | 파일 위치 | 역할 |
| -------- | --------- | ---- |
| **설정 코어** | `src/config.rs` | 설정 구조체 및 직렬화 정의 (Source of Truth) |
| **unim-config (CLI)** | `unim-config/src/main.rs` | CLI `ConfigKey` enum + setter dispatch |
| **로케일** | `unim-config/locales/*.yml` | CLI 라벨/에러 메시지 (ko, en) |
| **unim-dbus** | `unim-dbus/src/service.rs` | key-기반 레거시 `get_config`/`set_config` 디스패치. YAML/JSON 엔드포인트는 serde로 전체 구조체를 자동 처리하므로 신규 필드는 자동 반영됨 |
| **unim-gui 설정** | `unim-gui-gtk/src/gtk_ui.rs` | GTK GUI 위젯 바인딩 (유일한 GUI 창구) |

GNOME Shell 의존 키(예: indicator 토글 등)만 `unim-gnome-extension/prefs.js` +
`*.gschema.xml`도 함께 업데이트합니다. 그 외 일반 설정은 gschema에 추가하지 마세요.

> **사용자 데이터 파일은 5지점 동기 대상이 아닙니다.**
> AutoTypeFix 억제 사전 `~/.config/unim/typefix-blacklist.yaml`은 설정이 아닌
> 사용자 데이터이며 `src/typefix_blacklist.rs`가 관리합니다. 데몬이 mtime
> 감시로 자동 리로드하므로 5지점 체크리스트에 포함되지 않습니다.

## 설정 항목 추가/변경 시 체크리스트

1. [ ] `src/config.rs` — 설정 구조체에 새 필드 추가 (+ `clamp_ranges()` 방어 시 범위 확인)
2. [ ] `unim-config/src/main.rs` — `ConfigKey` enum 및 관련 함수 업데이트
3. [ ] `unim-config/locales/{ko,en}.yml` — 번역 문자열 추가
4. [ ] `unim-dbus/src/service.rs` — 레거시 key 디스패치가 필요한 경우 매칭 업데이트 (YAML/JSON은 자동)
5. [ ] `unim-gui-gtk/src/gtk_ui.rs` — GTK GUI 위젯 및 바인딩 추가
6. [ ] (GNOME Shell 전용 키일 때만) `unim-gnome-extension/prefs.js` + `*.gschema.xml`
7. [ ] (AutoTypeFix 관련 설정일 때) Blacklist 파일 핫리로드 로직이 새 설정과 독립적으로 동작하는지 확인

## DBus API (Phase 2)

| API | 시그니처 | 용도 |
| --- | -------- | ---- |
| `GetConfigYaml()` | → `s` | 전체 Config를 YAML로 반환 (파일 포맷과 동일) |
| `GetConfigJson()` | → `s` | 전체 Config를 JSON으로 반환 (JS 친화) |
| `SetConfigYaml(yaml)` | `s` → | YAML 파싱 → clamp → 저장 → `ConfigChangedJson` 방출 |
| `ConfigChangedJson` signal | `s` | 변경 후 전체 Config JSON payload |
| `GetConfig`/`SetConfig`/`ConfigChanged` (legacy) | key/value | gtk3/4, qt5/6, gnome-ext, tests 호환용으로 **병존 유지** |

## 마이그레이션 (Phase 6)

`unim-daemon` 기동 시 1회성 루틴(`unim-daemon/src/migration.rs`)이 legacy GSettings
값을 `config.yaml`로 이관합니다. 가드 파일 `~/.config/unim/.migrated-v2`가 존재하거나
`dconf` 미설치 환경이면 스킵됩니다. 이관 성공 후에만 가드를 touch하여 재실행을 방지합니다.

## 예시: `mode_sharing` 설정 추가 시

```text
src/config.rs               → ModeSharingMode enum 정의
unim-config/main.rs         → ConfigKey::ModeSharing 추가
unim-config/locales/*.yml   → 라벨/에러 번역
unim-dbus/service.rs        → get_config("mode_sharing"), set_config(...) 처리 (레거시만)
unim-gui-gtk/gtk_ui.rs      → ComboRow 추가
```

---

# Logging System (로깅 시스템)

## 개요

UNIM은 `UNIM_DEVELOP=1` 환경변수가 설정된 경우에만 활성화되는 통합 로깅 시스템을 사용합니다.
로그는 콘솔과 `~/.unim-errors.log` 파일에 동시에 출력됩니다.

## 로그 포맷

```text
[YYYY/MM/DD HH:MM:SS] - [모듈명] - 메시지
```

## 언어별 사용법

### Rust

```rust
use unim::unim_log;

unim_log!("ENGINE", "엔진 초기화 완료");
unim_log!("DAEMON", "연결 수: {}", count);
```

**모듈명 예시:**

| 모듈명 | 컴포넌트 |
| ------ | -------- |
| `ENGINE` | `src/input_engine.rs` |
| `HANGUL` | `src/hangul/*.rs` |
| `DAEMON` | `unim-daemon` |
| `DBUS` | `unim-dbus` |
| `XIM` | `unim-frontends/xim` |
| `WAYLAND` | `unim-frontends/wayland` |
| `CLI` | `unim-cli` |
| `INDICATOR` | `unim-gui` |

### C (GTK)

```c
// immodule.c 또는 unim_dbus_client.c 상단에 정의
static void unim_log_message(const char *module, const char *format, ...) {
    // UNIM_DEVELOP 환경변수 확인 후 콘솔+파일 출력
}

unim_log_message("GTK_IM", "포커스 변경: %s", app_name);
```

**모듈명:** `GTK_IM`, `GTK_DBUS`, `GTK3_IM`, `GTK4_IM`

### C++ (Qt)

```cpp
// input_context.cpp 또는 unim_dbus_client.cpp 상단에 정의
static void unim_log_message(const char *module, const QString &message) {
    // UNIM_DEVELOP 환경변수 확인 후 콘솔+파일 출력
}

unim_log_message("QT_IM", QString::asprintf("키 입력: %d", keyval));
```

**모듈명:** `QT_IM`, `QT_DBUS`, `QT5_IM`, `QT6_IM`

### JavaScript (GNOME Extension)

```javascript
import { unimLog, unimError } from './logging.js';

unimLog('EXTENSION', `단축키 바인딩: ${key}`);
unimError('INDICATOR', `DBus 연결 실패: ${e.message}`);
```

**모듈명:** `EXTENSION`, `INDICATOR`, `VKBD`, `PREFS`

## 규칙

1. **새 컴포넌트 추가 시**: 해당 언어의 로깅 패턴을 따라 `unim_log` 매크로/함수를 사용합니다.
2. **기존 `log::*` 크레이트 사용 금지**: Rust에서는 `log::info!`, `log::debug!` 등 대신 `unim_log!`를 사용합니다.
3. **기존 `console.log` 사용 금지**: GNOME Extension에서는 `unimLog/unimError`를 사용합니다.
4. **모듈명 일관성**: 각 컴포넌트에 맞는 모듈명을 사용하여 로그 추적을 용이하게 합니다.
5. **환경변수 의존**: 프로덕션 환경에서는 `UNIM_DEVELOP`이 설정되지 않으므로 로그가 출력되지 않습니다.

## 로그 파일 위치

* **파일 경로**: `~/.unim-errors.log`
* **활성화 조건**: `UNIM_DEVELOP=1` 환경변수 설정

## 핵심 파일

| 언어 | 로깅 구현 파일 |
| ---- | -------------- |
| Rust | `src/logging.rs` |
| JavaScript | `unim-gnome-extension/logging.js` |
| C/C++ | 각 컴포넌트의 소스 파일 내 `unim_log_message()` 함수 |

---

# CLAUDE.md

Behavioral guidelines to reduce common LLM coding mistakes. Merge with project-specific instructions as needed.

**Tradeoff:** These guidelines bias toward caution over speed. For trivial tasks, use judgment.

## 1. Think Before Coding

**Don't assume. Don't hide confusion. Surface tradeoffs.**

Before implementing:

* State your assumptions explicitly. If uncertain, ask.
* If multiple interpretations exist, present them - don't pick silently.
* If a simpler approach exists, say so. Push back when warranted.
* If something is unclear, stop. Name what's confusing. Ask.

## 2. Simplicity First

**Minimum code that solves the problem. Nothing speculative.**

* No features beyond what was asked.
* No abstractions for single-use code.
* No "flexibility" or "configurability" that wasn't requested.
* No error handling for impossible scenarios.
* If you write 200 lines and it could be 50, rewrite it.

Ask yourself: "Would a senior engineer say this is overcomplicated?" If yes, simplify.

## 3. Surgical Changes

**Touch only what you must. Clean up only your own mess.**

When editing existing code:

* Don't "improve" adjacent code, comments, or formatting.
* Don't refactor things that aren't broken.
* Match existing style, even if you'd do it differently.
* If you notice unrelated dead code, mention it - don't delete it.

When your changes create orphans:

* Remove imports/variables/functions that YOUR changes made unused.
* Don't remove pre-existing dead code unless asked.

The test: Every changed line should trace directly to the user's request.

## 4. Goal-Driven Execution

**Define success criteria. Loop until verified.**

Transform tasks into verifiable goals:

* "Add validation" → "Write tests for invalid inputs, then make them pass"
* "Fix the bug" → "Write a test that reproduces it, then make it pass"
* "Refactor X" → "Ensure tests pass before and after"

For multi-step tasks, state a brief plan:

```
1. [Step] → verify: [check]
2. [Step] → verify: [check]
3. [Step] → verify: [check]
```

Strong success criteria let you loop independently. Weak criteria ("make it work") require constant clarification.

---

**These guidelines are working if:** fewer unnecessary changes in diffs, fewer rewrites due to overcomplication, and clarifying questions come before implementation rather than after mistakes.
