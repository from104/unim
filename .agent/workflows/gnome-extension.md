---
description: GNOME Shell 확장 빌드, 설치, 패키징
---

# GNOME Shell 확장

## 빌드 및 설치

// turbo-all

1. GNOME 확장 빌드 및 설치
```bash
make install-gnome-extension PREFIX=/usr
```

2. 확장 활성화
```bash
gnome-extensions enable unim-indicator@from104.github.io
```

## 배포 패키지 생성

```bash
make pack
```

`unim-indicator@from104.github.io-<version>.zip` 파일이 생성됩니다.

## 제거

```bash
make uninstall-gnome-extension PREFIX=/usr
```

## 로그 확인

```bash
journalctl -f -o cat /usr/bin/gnome-shell
```

## 주요 파일

- `unim-gnome-extension/extension.js` - 메인 확장 로직
- `unim-gnome-extension/indicator.js` - 트레이 인디케이터
- `unim-gnome-extension/prefs.js` - 설정 UI
- `unim-gnome-extension/logging.js` - 로깅 모듈
- `unim-gnome-extension/schemas/*.gschema.xml` - GSettings 스키마
