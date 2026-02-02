# Development Conventions

* The core logic is isolated in the root `src/` directory. Any changes to the fundamental input logic should be made there.
* The GNOME extension communicates with the Rust engine by executing the `unim-cli` binary as a subprocess. This provides a stable and sandbox-friendly integration.
* The `Makefile` is the source of truth for the standard build and installation process.
* **Debian Versioning**: The package version (e.g., `0.0.1`) follows the version in the `src/` crate. If only packaging files (`debian/*`) change, increment the revision only (e.g., `0.0.1-1` → `0.0.1-2`).
* **문서 작성 언어**: Walkthrough, 계획(Implementation Plan), 작업 목록(Task) 등 문서는 **한글로 작성**합니다.

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
