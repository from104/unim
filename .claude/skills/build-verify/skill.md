---
name: build-verify
description: UNIM 코드 변경 후 빌드·테스트 zero-warning/all-pass 검증 반복 패턴. Rust 워크스페이스, C/C++ 프론트엔드, GNOME extension, 설치까지 검증 범위를 단계별로 확장한다. "빌드 검증", "cargo test", "make build", "warning 확인", "배포 전 검증", "PASS/FAIL 판정" 맥락에서 반드시 트리거.
---

# Build Verify — UNIM 빌드·테스트 반복 패턴

CLAUDE.md의 "Strict Quality Rules"에 따라 **warning 0, 테스트 all-pass**가 머지 기준이다. 이 스킬은 각 Phase 완료 시 실행할 고정 검증 절차를 제공한다.

## 검증 레벨

변경 범위에 따라 레벨을 선택하라. **상위 레벨은 하위 레벨을 포함한다.**

### L1 — 빠른 피드백 (핵심 크레이트만)

변경 직후 실행. 초단위.

```bash
cargo build -p unim            # Core만
cargo test -p unim             # Core 테스트만
```

### L2 — 워크스페이스 Rust

단일 Phase 완료 시. 수 분.

```bash
cargo build --workspace --release    # zero warning
cargo test --workspace               # all pass
```

### L3 — 전체 프로젝트 (C/C++ 포함)

Phase 완료 후 reviewer 검증 직전. 5~10분.

```bash
make build                           # Rust + GTK + Qt + XIM + Wayland 전부
make build-tests                     # 테스트 애플리케이션
```

### L4 — 설치 + 샌드박스 UI QA

GTK/Qt/extension 가시적 변경 후.

```bash
sudo make install PREFIX=/usr        # 위험: 로컬 설치, 사용자 승인 후만
make sandbox-gtk4                    # Xephyr에서 격리 테스트
```

설치는 사용자 명시 승인 없이 실행 금지. Phase 3·4 완료 후 사용자에게 "수동 설치·테스트 요청" 보고.

## warning 판정 기준

- `cargo build` 출력에 `warning:`이 하나라도 있으면 FAIL
- C/C++ 빌드는 `make build` 로그에 `warning:` 포함 여부 확인
- **의도적 허용 불가** — `#[allow(...)]`로 숨기는 것은 사용자 승인 필요

## 테스트 실패 디버깅 순서

1. 실패한 테스트 이름 정확히 기록
2. `cargo test -p {crate} {test_name} -- --nocapture`로 로그 관찰
3. 기대값 vs 실제값 비교
4. **구현 버그**: 수정 후 재실행
5. **테스트 오기**: 기대값 수정 이유를 보고서에 명시
6. 플래키 테스트: 동일 명령 3회 반복 후 결정

## DBus 통합 검증 (Phase 2 이후)

```bash
UNIM_DEVELOP=1 target/debug/unim-daemon -n &
# 다른 터미널에서
busctl --user call org.atit.unim.InputMethod \
    /org/atit/unim/InputMethod \
    org.atit.unim.InputMethod GetConfig
busctl --user monitor org.atit.unim.InputMethod   # signal 관찰
```

## extension 재로드 (Phase 4 이후)

```bash
# 스키마 재컴파일
glib-compile-schemas unim-gnome-extension/schemas/

# extension 재설치 (Makefile 사용)
make dev-extension

# Wayland: 재로그인
# X11: Alt+F2 → r → Enter
```

## 판정 보고 형식

각 Phase 보고서 끝에 아래 표 첨부:

```
| 검증 레벨 | 명령 | 결과 |
|-----------|------|------|
| L2 cargo build --workspace | zero warning | ✓ |
| L2 cargo test --workspace | 437 passed | ✓ |
| L3 make build | zero warning | ✓ |
| DBus GetConfig | YAML 반환 | ✓ |
```

FAIL 항목이 하나라도 있으면 다음 Phase로 진행 금지.
