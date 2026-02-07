---
description: UNIM 전체 빌드 (Rust + 프론트엔드 + 설정도구)
---

# 전체 빌드

프로젝트 전체를 빌드합니다 (Rust workspace + GTK3/4/Qt5/6 IM 모듈 + 설정 도구).

// turbo-all

1. 전체 빌드 실행
```bash
make build
```

## 개별 빌드 옵션

필요에 따라 개별 타겟을 빌드할 수 있습니다:

- **Rust만**: `make build-rust`
- **프론트엔드만**: `make build-frontends`
- **설정도구만**: `make build-settings`
