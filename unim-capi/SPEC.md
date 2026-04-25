# UNIM C API (unim-capi) 세부 기능 명세

> `unim-capi`는 코어 엔진(`src/`)의 **C FFI 바인딩 레이어**입니다.
> GTK/Qt IM 모듈, 설정 도구 등 C/C++ 프론트엔드가 Rust 코어와 통신하는 유일한 경로입니다.
> `cbindgen`으로 자동 생성되는 `unim.h` 헤더를 통해 C/C++ 코드에서 사용됩니다.

---

## 1. 아키텍처

### 1.1 역할

```
┌──────────────┐     ┌──────────────┐     ┌──────────────────┐
│  GTK IM 모듈  │────▶│              │     │                  │
│  (C)         │     │  unim-capi   │────▶│  unim (코어 엔진) │
│  Qt IM 모듈   │────▶│  (C FFI)     │     │  (Rust)          │
│  (C++)       │     │              │     │                  │
│  설정 도구    │────▶│  libunim_capi│     │  libhangul 대체   │
│  (C/C++)     │     │  .so / .a    │     │                  │
└──────────────┘     └──────────────┘     └──────────────────┘
```

### 1.2 파일 구조

```
unim-capi/
├── Cargo.toml          # crate-type: [cdylib, staticlib]
├── cbindgen.toml       # C 헤더 자동 생성 설정
├── src/
│   └── lib.rs          # FFI 함수 구현 (579행, 40+ 함수)
└── include/
    └── unim.h          # cbindgen으로 생성된 C 헤더 (448행)
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
    bool consumed;         // 키가 소비되었는지
    bool preedit_changed;  // preedit 변경됨
    bool commit_changed;   // commit 변경됨
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

### 4.1 GTK IM 모듈에서의 일반적인 사용

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
이는 GTK/Qt IM 모듈이 항상 메인 스레드에서 실행되므로 실질적인 제약이 아닙니다.

---

## 6. cbindgen 설정

[cbindgen.toml](cbindgen.toml)에 의해 C 헤더가 자동 생성됩니다.

```toml
language = "C"
include_guard = "UNIM_CAPI_H"
cpp_compat = true           # C++ extern "C" 래핑

[export]
include = ["InputResult", "InputCategory", "ModifierState", "UnimStr"]

[export.rename]
"InputResult"    = "UnimInputResult"
"InputCategory"  = "UnimInputCategory"
"ModifierState"  = "UnimModifierState"

[fn]
prefix = "UNIM_API"
```

| 설정 | 효과 |
|------|------|
| `cpp_compat = true` | `#ifdef __cplusplus extern "C"` 래핑 |
| `export.rename` | Rust 타입명 → `Unim` 접두사 C 타입명 |
| `fn.prefix = "UNIM_API"` | 함수에 `UNIM_API` 매크로 접두 (DLL export 등) |
| `parse_deps = false` | 의존 크레이트 내부는 파싱하지 않음 |
| `style = "Both"` | typedef + struct 이름 모두 생성 |

---

## 7. 수동 관리 헤더

현재 `include/unim.h`는 `cbindgen` 자동 생성이 아닌 **수동으로 작성된 헤더**입니다.

> [!NOTE]
> `cbindgen.toml`에 `parse_deps = false`로 설정되어 있어,
> 코어(`unim`) 크레이트의 타입(`Config`, `InputEngine`, `InputResult` 등)이
> 자동 생성에 포함되지 않습니다.
> 따라서 수동 헤더에서 Opaque 타입(`typedef struct Config UnimConfig`)과
> 값 타입(`UnimInputResult`, `UnimModifierState`)을 직접 정의합니다.

`lib.rs`에서 새 FFI 함수를 추가하면 **반드시 `include/unim.h`에도 수동으로 추가**해야 합니다.

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

| 컴포넌트 | 언어 | 링크 방식 | 사용 API 그룹 |
|----------|------|-----------|--------------|
| GTK3 IM 모듈 | C | 동적 (`.so`) | 엔진 + 입력 처리 |
| GTK4 IM 모듈 | C | 동적 (`.so`) | 엔진 + 입력 처리 |
| Qt5 IM 모듈 | C++ | 동적 (`.so`) | 엔진 + 입력 처리 |
| Qt6 IM 모듈 | C++ | 동적 (`.so`) | 엔진 + 입력 처리 |
| GTK 설정 도구 | C | 동적 (`.so`) | 설정 Getter/Setter + 레이아웃 헬퍼 |
| Qt 설정 도구 | C++ | 동적 (`.so`) | 설정 Getter/Setter + 레이아웃 헬퍼 |
