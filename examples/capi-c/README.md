# C-API 예제

`unim-capi/`가 제공하는 FFI로 C 프로그램이 UNIM Core 엔진을 직접 사용하는 예제.

## 파일

| 파일 | 설명 |
|------|------|
| [`minimal_session.c`](minimal_session.c) | 엔진 생성 → 두벌식/세벌식 390 레이아웃 전환 → keycode 3개 입력 → preedit/commit 상태 출력 (43줄) |

## 빌드

C-API 라이브러리는 Rust 빌드 산출물로 생성됩니다. 먼저 워크스페이스를 빌드:

```bash
cd /path/to/unim
cargo build -p unim-capi --release
# 산출물: target/release/libunim.{so,a}, unim-capi/include/unim.h
```

예제 컴파일:

```bash
cd examples/capi-c
gcc -I../../unim-capi/include \
    -L../../target/release \
    -o minimal_session minimal_session.c \
    -lunim -ldl -lpthread
LD_LIBRARY_PATH=../../target/release ./minimal_session
```

예상 출력:

```
UNIM C-API Dynamic Layout Test Start

--- Testing 2-bul (Should be '한') ---
Preedit: '한', Commit: ''

--- Testing 3-bul 390 (Should be '한') ---
Pressing 'M'(50), 'F'(33), 'S'(31)...
Preedit: '한', Commit: ''

UNIM C-API Test Finished
```

## 다음 단계

- 조합 후 Commit 유발(space, enter 등)까지 추가 → `committed_session.c`
- `unim_engine_set_input_category()` 로 한/영 전환 데모
- `request_hanja`, `select_hanja` 로 한자 변환 데모

## 관련 문서

- [`unim-capi/SPEC.md`](../../unim-capi/SPEC.md) — 전체 C-API 명세
- [`unim-capi/include/unim.h`](../../unim-capi/include/unim.h) — 헤더 파일 (타입·함수 시그니처)
