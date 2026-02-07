---
description: Xephyr 샌드박스 환경에서 UNIM 테스트
---

# 샌드박스 테스트

Xephyr 기반 격리 환경에서 시스템 IM 설정에 영향 없이 UNIM을 테스트합니다.

## 사전 조건

- `Xephyr` 설치 필요: `sudo apt install xserver-xephyr`
- 인디케이터 테스트 시: `sudo apt install stalonetray`

// turbo-all

1. 기본 샌드박스 실행 (기본 터미널)
```bash
make sandbox
```

## 특정 툴킷 테스트

- **GTK3**: `make sandbox-gtk3`
- **GTK4**: `make sandbox-gtk4`
- **Qt5**: `make sandbox-qt5`
- **Qt6**: `make sandbox-qt6`
- **XIM**: `make sandbox-xim`
- **인디케이터 포함**: `make sandbox-indicator`
