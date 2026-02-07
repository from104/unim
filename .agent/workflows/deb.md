---
description: 데비안 패키지(.deb) 빌드
---

# 데비안 패키지 빌드

// turbo-all

1. 전체 빌드 및 데비안 패키지 생성
```bash
make deb
```

빌드된 .deb 파일은 `./debs/` 디렉토리에 저장됩니다.

2. 빌드 아티팩트 확인
```bash
ls -la debs/
```

## 패키지 구성

- `unim` - 코어 엔진, CLI, 데몬, C-API
- `unim-gtk` - GTK3/4 IM 모듈, GTK 설정 도구
- `unim-qt` - Qt5/6 IM 플러그인, Qt 설정 도구
- `unim-indicator` - 시스템 트레이 인디케이터
- `gnome-shell-extension-unim` - GNOME Shell 확장

## 버전 관리

- 패키지 버전은 `src/` 크레이트의 `Cargo.toml` 버전을 따릅니다.
- 패키징 파일만 변경 시: 리비전만 증가 (예: `0.0.1-1` → `0.0.1-2`)
- `debian/changelog`은 `dch` 명령으로 업데이트합니다.

## 정리
```bash
make clean-deb
```
