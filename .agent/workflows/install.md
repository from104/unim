---
description: UNIM 시스템 설치 및 제거
---

# 시스템 설치

## 설치

// turbo-all

1. 전체 빌드
```bash
make build
```

2. 시스템 설치 (sudo 필요)
```bash
sudo make install PREFIX=/usr
```

3. 설치 확인
```bash
make test PREFIX=/usr
```

## 설치 후 설정

설치 후 다음 환경변수를 설정하거나 로그아웃/재로그인합니다:

```bash
export GTK_IM_MODULE=unim
export QT_IM_MODULE=unim
export XMODIFIERS=@im=unim
```

또는 `im-config`에서 `unim`을 선택합니다.

## Systemd 서비스 (선택)

```bash
sudo make install-systemd PREFIX=/usr
systemctl --user daemon-reload
systemctl --user enable --now unim-daemon.service
```

## 제거

```bash
sudo make uninstall PREFIX=/usr
```
