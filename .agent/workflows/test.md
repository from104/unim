---
description: UNIM 테스트 실행 (Rust 유닛 테스트 + 설치 상태 확인)
---

# 테스트 실행

// turbo-all

1. Rust 유닛 테스트 실행
```bash
cargo test --workspace
```

2. 설치 상태 확인 (시스템에 설치된 경우)
```bash
make test PREFIX=/usr
```

## 개별 테스트 앱 빌드

특정 툴킷 테스트 앱을 빌드하고 실행할 수 있습니다:

- **GTK3**: `make test-gtk3`
- **GTK4**: `make test-gtk4`
- **Qt5**: `make test-qt5`
- **Qt6**: `make test-qt6`
- **XIM**: `make test-xim`
- **DBus**: `make test-dbus`
