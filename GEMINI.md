# 프로젝트 참조 문서

* **[AGENTS.md](AGENTS.md)** - 프로젝트 개요, 컴포넌트 맵, 아키텍처 흐름
* **[ROADMAP.md](ROADMAP.md)** - 장기 개발 로드맵
* **[README.md](README.md)** - 프로젝트 소개 및 사용법

---

# Development Conventions

* The core logic is isolated in the root `src/` directory. Any changes to the fundamental input logic should be made there.
* The GNOME extension communicates with the Rust engine by executing the `unim-cli` binary as a subprocess. This provides a stable and sandbox-friendly integration.
* The `Makefile` is the source of truth for the standard build and installation process.
* **Debian Versioning**: The package version (e.g., `0.0.1`) follows the version in the `src/` crate. If only packaging files (`debian/*`) change, increment the revision only (e.g., `0.0.1-1` → `0.0.1-2`).
* **문서 작성 언어**: Walkthrough, 계획(Implementation Plan), 작업 목록(Task) 등 문서는 **한글로 작성**합니다.

---

# 설정 항목 연동 가이드라인 (Settings Synchronization)

## 개요

UNIM의 설정 항목(`src/config.rs`)이 변경될 때는 **모든 관련 컴포넌트에 동일한 설정이 반영**되어야 합니다.
설정 변경 시 반드시 아래 체크리스트를 확인하세요.

## 연동 대상 컴포넌트

| 컴포넌트 | 파일 위치 | 역할 |
| -------- | --------- | ---- |
| **설정 코어** | `src/config.rs` | 설정 구조체 및 직렬화 정의 (Source of Truth) |
| **unim-config (CLI)** | `unim-config/src/main.rs` | CLI 설정 관리 도구 |
| **unim-dbus** | `unim-dbus/src/service.rs` | `get_config`/`set_config` DBus 메서드 |
| **GTK 설정 도구** | `unim-gtk-settings/src/settings_dialog.c` | GTK 기반 GUI 설정 |
| **Qt 설정 도구** | `unim-qt-settings/src/SettingsDialog.cpp` | Qt 기반 GUI 설정 |
| **GNOME Extension 설정** | `unim-gnome-extension/prefs.js` | GNOME Extension Preferences |
| **C-API** | `unim-capi/src/lib.rs` | FFI 바인딩 (필요 시) |

## 설정 항목 추가/변경 시 체크리스트

1. [ ] `src/config.rs` - 설정 구조체에 새 필드 추가
2. [ ] `unim-config/src/main.rs` - `ConfigKey` enum 및 관련 함수 업데이트
3. [ ] `unim-config/locales/*.yml` - 번역 문자열 추가
4. [ ] `unim-dbus/src/service.rs` - `get_config`/`set_config` 매칭 업데이트
5. [ ] `unim-gtk-settings` - UI 위젯 및 DBus 연동 추가
6. [ ] `unim-qt-settings` - UI 위젯 및 DBus 연동 추가
7. [ ] `unim-gnome-extension/prefs.js` - GSettings 스키마 및 UI 추가
8. [ ] `unim-gnome-extension/*.gschema.xml` - GSchema 정의 업데이트
9. [ ] `unim-capi/src/lib.rs` - FFI 함수 추가 (필요 시)

## 예시: `mode_sharing` 설정 추가 시

```text
src/config.rs           → ModeSharingMode enum 정의
unim-config/main.rs     → ConfigKey::ModeSharing 추가
unim-dbus/service.rs    → get_config("mode_sharing"), set_config("mode_sharing", ...) 처리
unim-gtk-settings       → ComboBox 추가, DBus 연동
unim-qt-settings        → QComboBox 추가, DBus 연동
prefs.js + gschema.xml  → 'mode-sharing' 키 추가
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
| `INDICATOR` | `unim-indicator` |

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
