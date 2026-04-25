# UNIM 예제

이 디렉토리는 UNIM 핵심 크레이트(`unim`)의 실행 가능한 예제를 담습니다.
각 파일은 `cargo run --example` 으로 독립 실행됩니다. IME 통합 없이
**Core 엔진 라이브러리만** 사용하는 예제이므로 DBus 데몬·프론트엔드가 필요 없습니다.

## 실행 방법

워크스페이스 루트에서:

```bash
cargo run --example <예제이름>
```

예:

```bash
cargo run --example input_simulation_2bul
cargo run --example mk_korean
```

## 현재 예제 (Core 엔진)

| 파일 | 목적 | 핵심 API |
|------|------|---------|
| [`input_simulation_2bul.rs`](input_simulation_2bul.rs) | 두벌식 조합 시뮬레이션 — 도깨비불 현상 포함 | `HangulComposer2Bul`, `JamoEnum::{Cho, Jung}` |
| [`input_simulation_3bul.rs`](input_simulation_3bul.rs) | 세벌식 조합 시뮬레이션 — 초/중/종 명시 입력 | `HangulComposer3Bul`, `JamoEnum::{Cho, Jung, Jong}` |
| [`jamo_pattern_search.rs`](jamo_pattern_search.rs) | 자모 단위 퍼지 검색(자동완성·필터링 응용) | `HangulChar`, `JamoEnum`, `Cho::to_char()` |
| [`mk_korean.rs`](mk_korean.rs) | 유니코드 한글 11,172음절 전수 생성 | `HangulChar::from_jamo_sequences`, `CHOSEONG_NUMBER` 등 |
| [`string_processing.rs`](string_processing.rs) | 문자열의 각 음절을 초/중/종 자모로 분해 | `HangulChar::from_char`, `to_jamo_tuple` |

> 모든 Core 예제는 2026-02-24 작성. API 변경 시 동반 검증이 필요하므로
> 코어 공개 API 변경 PR에서는 `cargo run --example` 수동 실행으로 컴파일 가능성을 확인한다.

## 하위 디렉토리 예제 (3계층 확장)

Core 전용 예제 외에 계층별 하위 디렉토리를 두어 C-API·DBus·설정까지 커버:

| 경로 | 내용 | 상태 |
|------|------|------|
| [`capi-c/`](capi-c/) | C-API 최소 샘플 (C에서 UNIM 엔진 직접 호출, 43줄) | ✅ 존재 — `minimal_session.c` |
| [`dbus-client/`](dbus-client/) | DBus 클라이언트 스니펫 (Python/Rust로 데몬과 대화) | 🏗️ 골격만 — `README.md`에 `busctl` 수동 예제, 코드 예제는 계획 |
| [`config/`](config/) | 주석 포함 `config.yaml` 템플릿 | ✅ 존재 — `example.yaml` (전체 필드 + 범위 주석) |
| `frontend-minimal/` | GTK4/Qt6 최소 로더 앱 (IM 모듈 로드 검증용) | 계획 |

신규 예제 추가 시:
1. 이 README의 표에 한 줄 추가
2. 예제 파일 최상단에 `//!` doc comment로 "목적 / 실행 방법 / 기대 출력" 기술
3. 가능하면 `cargo test --examples` 또는 CI smoke에 컴파일 검증 편입

## 관련 문서

- [`../README.md`](../README.md) — 프로젝트 전반
- [`../src/SPEC.md`](../src/SPEC.md) — Core 엔진 API 상세
- [`../unim-capi/SPEC.md`](../unim-capi/SPEC.md) — C-API FFI 명세
- [`../unim-dbus/SPEC.md`](../unim-dbus/SPEC.md) — DBus 인터페이스 (클라이언트 예제 작성 시 참조)
