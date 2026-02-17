---
description: 컴포넌트별 빠른 빌드+배포 (deb 빌드 불필요)
---

# UNIM 빠른 개발 워크플로우

> **전제조건**: 최초 1회 전체 빌드 및 설치가 완료되어 있어야 합니다.

```bash
make build && sudo make install PREFIX=/usr
```

## 컴포넌트별 빠른 배포

### GTK4 IM 모듈 수정 시 (가장 빈번)

// turbo

```bash
make dev-gtk4 PREFIX=/usr
```

그런 다음 GTK4 앱을 재시작하여 테스트합니다.

### GTK3 IM 모듈 수정 시

// turbo

```bash
make dev-gtk3 PREFIX=/usr
```

### Qt5/Qt6 플러그인 수정 시

// turbo

```bash
make dev-qt5 PREFIX=/usr   # 또는 make dev-qt6 PREFIX=/usr
```

### Rust 엔진(src/) 수정 시

// turbo

```bash
make dev-core PREFIX=/usr      # libunim_capi.so 빌드 + 배포
make dev-gtk4 PREFIX=/usr      # 프론트엔드도 다시 빌드 (C-API 변경 반영)
```

### unim-daemon 수정 시

// turbo

```bash
make dev-daemon PREFIX=/usr    # 빌드 + 배포 + 데몬 재시작
```

### XIM/Wayland 프론트엔드 수정 시

// turbo

```bash
make dev-xim PREFIX=/usr       # 또는 make dev-wayland PREFIX=/usr
```

### 데몬만 재시작 (코드 변경 없이)

// turbo

```bash
make dev-restart
```

## Sandbox 테스트 (시스템 건드리지 않음)

sandbox는 자동으로 `GTK_PATH` / `QT_PLUGIN_PATH`를 설정하여
로컬 빌드 결과물(.so)을 직접 로드합니다.

### GTK4 테스트

// turbo

```bash
make sandbox-gtk4
```

### 빌드 건너뛰고 sandbox만 시작

```bash
./scripts/sandbox.sh --no-build gtk4
```
