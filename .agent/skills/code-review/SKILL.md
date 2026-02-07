---
name: code-review
description: UNIM 코드 리뷰 가이드 - 언어별 컨벤션, 안전성 체크, 변경 영향 분석
---

# UNIM 코드 리뷰 스킬

UNIM 프로젝트의 코드 변경사항을 리뷰할 때 사용합니다.

## 공통 체크리스트

- [ ] `cargo test --workspace` 통과
- [ ] 로깅: `unim_log!` / `unim_log_message()` / `unimLog()` 사용 (절대 `println!`, `console.log`, `log::*` 사용 금지)
- [ ] 설정 변경 시: `GEMINI.md`의 설정 연동 체크리스트 확인

## 언어별 컨벤션

### Rust (`src/`, `unim-*/`)

- [ ] `clippy` 경고 없음 (`cargo clippy --workspace`)
- [ ] `unsafe` 사용 최소화, 사용 시 주석으로 안전성 설명
- [ ] `Send + Sync` 트레이트 고려 (EngineWorker는 별도 스레드)
- [ ] 에러 처리: `?` 연산자 사용, 사용자 가시적 에러는 적절한 메시지

### C (`unim-frontends/gtk*/`, `unim-gtk-settings/`)

- [ ] 메모리 누수 없음 (`g_free`, `g_object_unref` 등 적절한 해제)
- [ ] NULL 포인터 체크
- [ ] GLib/GTK API 올바른 사용
- [ ] DBus 프록시 관리: 연결 실패 시 graceful fallback

### C++ (`unim-frontends/qt*/`, `unim-qt-settings/`)

- [ ] Qt 메모리 관리 패턴 (parent-child 소유권)
- [ ] `QString` ↔ `const char*` 변환 안전성
- [ ] 시그널-슬롯 연결 정확성

### JavaScript (`unim-gnome-extension/`)

- [ ] `unimLog` / `unimError` 사용 (로깅)
- [ ] GJS API 올바른 사용
- [ ] 확장 활성화/비활성화 시 리소스 정리
- [ ] GSettings 스키마 일관성

## 변경 영향 분석

### 핵심 파일 변경 시 주의

| 변경 파일 | 영향 범위 | 추가 확인 |
| --------- | --------- | --------- |
| `src/input_engine.rs` | 모든 프론트엔드 | 전체 테스트, 샌드박스 검증 |
| `src/config.rs` | 모든 설정 도구, DBus | 설정 연동 체크리스트 (`GEMINI.md`) |
| `src/hangul/*.rs` | 한글 조합 전체 | 2벌식, 3벌식 모두 테스트 |
| `unim-dbus/src/service.rs` | 모든 프론트엔드 | DBus 호환성, 시그널 확인 |
| `unim-capi/src/lib.rs` | GTK/Qt 설정, GNOME 확장 | FFI 안전성 |

### 프론트엔드 변경 시

- 하나의 프론트엔드 변경이 다른 프론트엔드에도 적용되어야 하는지 확인
- GTK3 ↔ GTK4, Qt5 ↔ Qt6 간 공통 로직은 `gtk-common/`, `qt-common/`에 위치
