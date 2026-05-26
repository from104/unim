# UNIM C API (unim-capi) 세부 기능 명세

> `unim-capi`는 코어 엔진(`src/`)의 **C FFI 바인딩 레이어**입니다.
> **외부 애플리케이션이 UNIM 엔진을 임베드하기 위한 공개 C API**로 포지셔닝됩니다.
> C/C++ 코드는 `include/unim.h` 헤더(수작업 관리되는 단일 진실원천)를 통해 사용합니다.

> [!IMPORTANT]
> 현재 **in-tree 라이브 소비자는 0개**입니다.
> - unim-daemon, Windows(`unim-windows`/`unim-tsf`), `unim-popup-service`는 모두 `unim` 크레이트를 **직접** 사용합니다.
> - Linux 프론트엔드(GTK3/4, Qt5/6)는 IM 모듈 → 자체 C DBus 클라이언트(`unim_dbus_*`) → unim-daemon 경로로 동작하며 **capi를 링크하지 않습니다** (자세한 내용은 §11 참고).
>
> 따라서 capi의 in-tree 사용처는 `examples/capi-c`(외부 임베더용 데모)와 capi 자체 단위 테스트뿐입니다.
> capi는 외부 임베딩 시나리오를 위해 유지·관리되는 안정 API입니다.

---

## 1. 아키텍처

### 1.1 역할

```
┌────────────────────┐     ┌──────────────┐     ┌──────────────────┐
│  외부 임베더 앱     │────▶│              │     │                  │
│  (C/C++)           │     │  unim-capi   │────▶│  unim (코어 엔진) │
│  examples/capi-c   │────▶│  (C FFI)     │     │  (Rust)          │
│                    │     │  libunim_capi│     │  libhangul 대체   │
│                    │     │  .so / .a    │     │                  │
└────────────────────┘     └──────────────┘     └──────────────────┘
```

> Linux IM 프론트엔드는 이 경로를 사용하지 않습니다 (§11 참고).

### 1.2 파일 구조

```
unim-capi/
├── Cargo.toml          # crate-type: [cdylib, staticlib]
├── build.rs            # soname 링크 + C-API 드리프트 가드 (§7 참고)
├── cbindgen.toml       # C 헤더 생성 설정 (드리프트 가드용)
├── src/
│   └── lib.rs          # FFI 함수 구현
└── include/
    └── unim.h          # 수작업 관리되는 단일 진실원천(SoT) C 헤더
```

### 1.3 빌드 산출물

| 산출물 | 파일명 | 용도 |
|--------|--------|------|
| 동적 라이브러리 | `libunim_capi.so` | GTK/Qt IM 모듈 런타임 링크 |
| 정적 라이브러리 | `libunim_capi.a` | 정적 링크 빌드 |
| C 헤더 | `include/unim.h` | C/C++ 컴파일 시 포함 |

---

## 2. 타입 정의

### 2.1 불투명 타입 (Opaque)

```c
typedef struct Config     UnimConfig;
typedef struct InputEngine UnimEngine;
```

C 코드에서는 내부 구조를 알 수 없으며, **포인터로만** 접근합니다.
생성/해제는 반드시 `unim_*_new()` / `unim_*_delete()` 쌍으로 관리합니다.

### 2.2 값 타입

#### UnimInputResult

```c
typedef struct {
    bool consumed;                          // 키가 소비되었는지
    bool preedit_changed;                   // preedit 변경됨
    bool commit_changed;                    // commit 변경됨
    bool hanja_candidates_available;        // 한자 후보 사용 가능
    bool special_char_candidates_available; // 특수문자 후보 사용 가능
} UnimInputResult;
```

#### UnimModifierState

```c
typedef struct {
    bool shift;
    bool control;
    bool alt;
    bool super_key;
    bool caps_lock;
    bool num_lock;
} UnimModifierState;
```

#### UnimStr — 문자열 참조

```c
typedef struct {
    const uint8_t *ptr;  // UTF-8 데이터 포인터
    size_t len;          // 바이트 길이
} UnimStr;
```

> [!IMPORTANT]
> `UnimStr`은 **소유권을 이전하지 않습니다**.
> 포인터는 `UnimEngine`이 살아있고 **다음 키 입력 전까지만** 유효합니다.
> C 측에서 데이터를 보존하려면 반드시 복사(`memcpy` 등)해야 합니다.

### 2.3 열거형

#### UnimInputCategory

| 값 | C 상수 | 의미 |
|----|--------|------|
| 0 | `UNIM_INPUT_CATEGORY_KOREAN` | 한국어 모드 |
| 1 | `UNIM_INPUT_CATEGORY_ENGLISH` | 영어 모드 |

#### UnimKoreanLayout

| 값 | C 상수 | 의미 |
|----|--------|------|
| 0 | `UNIM_KOREAN_LAYOUT_DUBEOLSIK` | 두벌식 표준 |
| 1 | `UNIM_KOREAN_LAYOUT_SEBEOLSIK_390` | 세벌식 390 |
| 2 | `UNIM_KOREAN_LAYOUT_SEBEOLSIK_391` | 세벌식 최종 |
| 3 | `UNIM_KOREAN_LAYOUT_SEBEOLSIK_NOSHIFT` | 세벌식 순아래 |

#### UnimEnglishLayout

| 값 | C 상수 | 의미 |
|----|--------|------|
| 0 | `UNIM_ENGLISH_LAYOUT_QWERTY` | QWERTY |
| 1 | `UNIM_ENGLISH_LAYOUT_DVORAK` | Dvorak |
| 2 | `UNIM_ENGLISH_LAYOUT_COLEMAK` | Colemak |
| 3 | `UNIM_ENGLISH_LAYOUT_COLEMAK_DH` | Colemak-DH |
| 4 | `UNIM_ENGLISH_LAYOUT_WORKMAN` | Workman |

#### UnimModeSharingMode

| 값 | C 상수 | 의미 |
|----|--------|------|
| 0 | `UNIM_MODE_SHARING_GLOBAL` | 전역 모드 공유 |
| 1 | `UNIM_MODE_SHARING_PER_APP` | 앱별 독립 모드 |

---

## 3. API 그룹별 함수 목록

### 3.1 API 버전

| 함수 | 원형 | 설명 |
|------|------|------|
| `unim_api_version` | `size_t unim_api_version(void)` | 현재 API 버전 반환 (1) |

### 3.2 설정 생명주기

| 함수 | 원형 | 설명 |
|------|------|------|
| `unim_config_load` | `UnimConfig* unim_config_load(void)` | 기본 경로에서 설정 로드 |
| `unim_config_default` | `UnimConfig* unim_config_default(void)` | 기본값으로 설정 생성 |
| `unim_config_delete` | `void unim_config_delete(UnimConfig*)` | 설정 객체 해제 |
| `unim_config_ensure_file` | `bool unim_config_ensure_file(void)` | 설정 파일 존재 보장 |
| `unim_config_needs_reload` | `bool unim_config_needs_reload(const UnimConfig*)` | 파일 변경 확인 |
| `unim_config_reload` | `bool unim_config_reload(UnimConfig*)` | 변경 시 재로드 |
| `unim_config_save` | `bool unim_config_save(const UnimConfig*)` | 설정 저장 |

### 3.3 설정 Getter/Setter

| 함수 | 타입 | 대상 필드 |
|------|------|-----------|
| `unim_config_get_korean_layout` / `set` | `UnimKoreanLayout` | `engine.korean.layout` |
| `unim_config_get_english_layout` / `set` | `UnimEnglishLayout` | `engine.english.layout` |
| `unim_config_get_default_category` / `set` | `UnimInputCategory` | `engine.default_category` |
| `unim_config_get_mode_sharing` / `set` | `UnimModeSharingMode` | `engine.mode_sharing` |

### 3.4 엔진 생명주기

| 함수 | 원형 | 설명 |
|------|------|------|
| `unim_engine_new` | `UnimEngine* unim_engine_new(const UnimConfig*)` | 엔진 생성 |
| `unim_engine_delete` | `void unim_engine_delete(UnimEngine*)` | 엔진 해제 |

### 3.5 입력 처리

| 함수 | 원형 | 설명 |
|------|------|------|
| `unim_engine_press_key` | `UnimInputResult unim_engine_press_key(UnimEngine*, const UnimConfig*, uint16_t, UnimModifierState)` | 키 입력 처리 |
| `unim_engine_commit_str` | `UnimStr unim_engine_commit_str(const UnimEngine*)` | 확정 문자열 조회 |
| `unim_engine_preedit_str` | `UnimStr unim_engine_preedit_str(const UnimEngine*)` | 조합 문자열 조회 |

### 3.6 엔진 상태

| 함수 | 설명 |
|------|------|
| `unim_engine_set_input_category` | 입력 모드 변경 |
| `unim_engine_get_input_category` | 현재 입력 모드 조회 |
| `unim_engine_set_korean_layout` | 한국어 레이아웃 즉시 변경 |
| `unim_engine_set_english_layout` | 영어 레이아웃 즉시 변경 |
| `unim_engine_reset` | 전체 상태 리셋 |
| `unim_engine_clear_commit` | commit 버퍼 비우기 |
| `unim_engine_clear_preedit` | preedit 플러시 (→ commit) |
| `unim_engine_remove_preedit` | preedit 제거 (commit 없이) |
| `unim_engine_is_composing` | 조합 중 여부 |
| `unim_engine_check_ready` | ready 상태 확인 |
| `unim_engine_end_ready` | ready 상태 종료 |

### 3.7 레이아웃 열거 헬퍼 (UI용)

| 함수 | 설명 |
|------|------|
| `unim_korean_layout_count` | 한국어 레이아웃 수 (4) |
| `unim_korean_layout_at(index)` | 인덱스로 레이아웃 조회 |
| `unim_korean_layout_name(layout)` | 내부 이름 (예: `"2bul"`) |
| `unim_korean_layout_display_name(layout)` | 표시 이름 (예: `"두벌식 표준"`) |
| `unim_english_layout_count` | 영어 레이아웃 수 (5) |
| `unim_english_layout_at(index)` | 인덱스로 레이아웃 조회 |
| `unim_english_layout_name(layout)` | 내부 이름 |
| `unim_english_layout_display_name(layout)` | 표시 이름 |
| `unim_mode_sharing_count` | 모드 공유 방식 수 |
| `unim_mode_sharing_at(index)` | 인덱스로 조회 |
| `unim_mode_sharing_display_name(mode)` | 표시 이름 |

### 3.8 상태 파일

| 함수 | 원형 | 설명 |
|------|------|------|
| `unim_status_get` | `int32_t unim_status_get(void)` | 상태 읽기 (0=EN, 1=KO, -1=에러) |
| `unim_status_set` | `bool unim_status_set(int32_t)` | 상태 쓰기 |

---

## 4. 사용 패턴

> 아래 예시는 **외부 임베더 애플리케이션**이 UNIM 엔진을 직접 사용하는 패턴입니다.
> (in-tree Linux 프론트엔드는 capi 대신 DBus 경로를 사용합니다 — §11 참고.)

### 4.1 엔진 임베딩의 일반적인 사용

```c
#include "unim.h"

// 초기화
UnimConfig *config = unim_config_load();
UnimEngine *engine = unim_engine_new(config);

// 키 이벤트 처리
UnimModifierState state = { .shift = false, .caps_lock = false, ... };
UnimInputResult result = unim_engine_press_key(engine, config, keycode, state);

if (result.commit_changed) {
    UnimStr commit = unim_engine_commit_str(engine);
    // commit.ptr ~ commit.len 범위의 UTF-8 문자열을 앱에 전달
    // 주의: ptr은 다음 press_key 호출 전까지만 유효
    unim_engine_clear_commit(engine);
}

if (result.preedit_changed) {
    UnimStr preedit = unim_engine_preedit_str(engine);
    // preedit 표시 업데이트
}

// 종료
unim_engine_delete(engine);
unim_config_delete(config);
```

### 4.2 설정 도구에서의 사용

```c
// 설정 로드
UnimConfig *config = unim_config_load();

// 현재 값 읽기 (UI 초기화용)
UnimKoreanLayout ko = unim_config_get_korean_layout(config);
UnimEnglishLayout en = unim_config_get_english_layout(config);

// 레이아웃 콤보박스 채우기
for (size_t i = 0; i < unim_korean_layout_count(); i++) {
    UnimKoreanLayout layout = unim_korean_layout_at(i);
    UnimStr name = unim_korean_layout_display_name(layout);
    // UI 항목 추가: name.ptr, name.len
}

// 값 변경
unim_config_set_korean_layout(config, UNIM_KOREAN_LAYOUT_SEBEOLSIK_390);

// 저장
unim_config_save(config);

// 해제
unim_config_delete(config);
```

### 4.3 설정 핫리로드 패턴

```c
// 주기적 또는 키 입력 시 호출
if (unim_config_needs_reload(config)) {
    unim_config_reload(config);
    // 엔진 레이아웃도 동기화
    unim_engine_set_korean_layout(engine, unim_config_get_korean_layout(config));
    unim_engine_set_english_layout(engine, unim_config_get_english_layout(config));
}
```

---

## 5. 메모리 관리 규칙

### 5.1 소유권 계약

| 함수 유형 | 소유권 | 해제 의무 |
|-----------|--------|-----------|
| `*_new()` / `*_load()` / `*_default()` | **호출자에게 이전** | 반드시 `*_delete()` 호출 |
| `*_str()` (UnimStr 반환) | **빌려줌 (borrow)** | 해제 금지, 다음 키 입력 전까지만 유효 |
| `*_name()` / `*_display_name()` | **정적 수명** | 해제 금지, 프로세스 종료까지 유효 |

### 5.2 NULL 안전성

`unim_config_delete`와 `unim_engine_delete`는 **NULL 포인터를 안전하게 무시**합니다.

```rust
pub unsafe extern "C" fn unim_config_delete(config: *mut Config) {
    if !config.is_null() {
        drop(Box::from_raw(config));
    }
}
```

### 5.3 스레드 안전성

`unim-capi` 함수들은 **스레드 안전하지 않습니다**.
동일한 `UnimEngine` 또는 `UnimConfig` 인스턴스를 여러 스레드에서 동시에 사용하면 안 됩니다.
임베더 앱은 단일 스레드(보통 UI/메인 스레드)에서 인스턴스를 사용해야 합니다.

---

## 6. cbindgen 설정 (드리프트 가드용)

[cbindgen.toml](cbindgen.toml)은 `build.rs`가 드리프트 가드(§7)를 수행할 때
사용하는 생성 설정입니다. **배포 헤더는 cbindgen 출력이 아니라 수작업 관리되는
`include/unim.h`** 입니다 (§7 참고).

```toml
language = "C"
include_guard = "UNIM_CAPI_H"
cpp_compat = true           # C++ extern "C" 래핑
style = "type"
documentation = true
documentation_style = "doxy"

[export]
include = [
    "InputResult", "InputCategory", "ModifierState",
    "ModeSharingMode", "UnimStr", "CPopupKeyResult",
]

[export.rename]
"InputResult"    = "UnimInputResult"
"InputCategory"  = "UnimInputCategory"
"ModifierState"  = "UnimModifierState"
"ModeSharingMode" = "UnimModeSharingMode"
"CPopupKeyResult" = "UnimPopupKeyResult"
# Opaque/값 타입 rename — 기존 소비자가 의존하는 Unim* 이름 유지
"Config"      = "UnimConfig"
"InputEngine" = "UnimEngine"
"PopupState"  = "UnimPopupState"

[parse]
parse_deps = true
include = ["unim"]
```

| 설정 | 효과 |
|------|------|
| `cpp_compat = true` | `#ifdef __cplusplus extern "C"` 래핑 |
| `export.rename` | Rust 타입명 → `Unim` 접두사 C 타입명 (소비자 호환 유지) |
| `parse_deps = true` | 코어(`unim`) 크레이트 내부 타입까지 파싱 (완전 자동 생성 전환 대비) |
| `style = "type"` | typedef 스타일 타입명 생성 |

---

## 7. 헤더 관리 — 단일 진실원천 + 드리프트 가드

배포되는 `include/unim.h`는 **수작업으로 관리되는 단일 진실원천(source of truth)** 입니다.
`lib.rs`에서 새 FFI 함수를 추가하면 **반드시 `include/unim.h`에도 직접 추가**해야 합니다.

이 수작업 헤더가 Rust `#[no_mangle]` export 집합과 조용히 어긋나는 것을 막기 위해,
`build.rs`가 **드리프트 가드**를 수행합니다:

```
build.rs (빌드 시)
  1. cbindgen으로 OUT_DIR에 헤더를 생성
  2. 생성 헤더의 export 함수 집합 ↔ 커밋된 include/unim.h 함수 집합 대조
  3. 불일치 시 cargo:warning 발행 (MISSING / 미exported)
     → 빌드 실패는 아님 (read-only 트리 패키징 영향 없음)
```

> [!NOTE]
> 드리프트 가드는 함수 시그니처 단위가 아닌 **exported 함수 이름 집합**을 비교합니다.
> 일치 시 `C-API drift guard OK: N exported functions in sync` 로그를 남깁니다.
> `cbindgen.toml`의 rename/`parse_deps=true` 설정은 향후 완전 자동 생성으로
> 전환하더라도 기존 소비자(opaque/값 타입 이름)와 호환되도록 맞춰져 있습니다.

### 7.1 최근 헤더 동기화

| 추가 항목 | 위치 |
|-----------|------|
| `UnimInputResult.hanja_candidates_available` | 값 타입 필드 |
| `UnimInputResult.special_char_candidates_available` | 값 타입 필드 |
| `#define UNIM_POPUP_KEY_PERIOD 34` | popup 키 상수 |

---

## 8. 의존성

| 크레이트 | 버전 | 역할 |
|----------|------|------|
| `unim` | `path = ".."` | 코어 엔진 (유일한 런타임 의존) |
| `cbindgen` | 0.28 (build-dep) | C 헤더 생성 (빌드 시) |

---

## 9. 빌드

```bash
# 빌드 (cdylib + staticlib)
cargo build -p unim-capi

# 릴리스 빌드
cargo build -p unim-capi --release

# C 헤더 재생성 (필요 시)
cbindgen --config unim-capi/cbindgen.toml --crate unim-capi --output unim-capi/include/unim.h
```

### 설치 경로 (Makefile 기준)

| 파일 | 경로 |
|------|------|
| `libunim_capi.so` | `/usr/lib/unim/` |
| `unim.h` | `/usr/include/unim/` |

---

## 10. 테스트

```bash
cargo test -p unim-capi
```

| 테스트 | 검증 내용 |
|--------|-----------|
| `test_api_version` | API 버전 상수 일치 |
| `test_config_lifecycle` | Config 생성→해제 (NULL 아님) |
| `test_engine_lifecycle` | Config→Engine 생성→해제 |
| `test_unim_str` | UnimStr 생성, 길이, NULL 포인터 |

---

## 11. 소비자 목록

### 11.1 현재 in-tree 소비자

| 컴포넌트 | 언어 | 링크 방식 | 용도 |
|----------|------|-----------|------|
| `examples/capi-c` | C | 동적 (`.so`) | 외부 임베더용 데모 (`minimal_session.c`) |
| capi 단위 테스트 | Rust | (크레이트 내부) | FFI 함수 동작 검증 (§10) |

> [!NOTE]
> **라이브(런타임) in-tree 소비자는 없습니다.** capi는 외부 애플리케이션이 UNIM 엔진을
> 임베드할 때를 위한 공개 C API입니다.

### 11.2 capi를 사용하지 **않는** 컴포넌트 (참고)

혼동을 막기 위해, 과거 capi 소비자로 오해되기 쉬운 컴포넌트의 실제 경로를 명시합니다.

| 컴포넌트 | 실제 엔진 접근 경로 |
|----------|--------------------|
| GTK3/GTK4 IM 모듈 | IM 모듈 → 자체 C DBus 클라이언트(`unim_dbus_*`) → unim-daemon (capi 미링크) |
| Qt5/Qt6 IM 모듈 | 플러그인 → 자체 C++ DBus 클라이언트 → unim-daemon (capi 미링크) |
| unim-daemon | `unim` 크레이트 직접 사용 |
| `unim-popup-service` | `unim` 크레이트의 popup 모듈 직접 사용 |
| Windows (`unim-windows`/`unim-tsf`) | `unim` 크레이트 직접 사용 |

> [!NOTE]
> 과거 프론트엔드가 capi에서 쓰던 유일한 심볼은 `unim_popup_*`였으나, 해당 임베디드 팝업
> 위젯이 프론트엔드에서 제거되면서 capi 의존이 통째로 사라졌습니다. 모든 팝업은 이제
> `unim-popup-service`(독립 GTK4 프로세스)가 렌더링합니다.
> capi 헤더에는 `unim_popup_*` API가 외부 임베더를 위해 그대로 남아 있습니다.
