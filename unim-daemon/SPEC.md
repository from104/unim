# UNIM 데몬 세부 기능 명세

> `unim-daemon`은 UNIM 입력기의 핵심 프로세스로, DBus 서비스를 호스팅하고 프론트엔드 모듈(XIM, Wayland)을 관리합니다.

---

## 1. 아키텍처 개요

### 1.1 역할

```
┌───────────────────────────────────────────────────────────────┐
│                      unim-daemon 프로세스                     │
│                                                               │
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────────────┐│
│  │ DBus 서비스  │  │ Engine Worker│  │ 프론트엔드 프로세스   ││
│  │ (tokio 런타임)│  │ (전용 스레드)│  │ 매니저               ││
│  │              │  │              │  │  ├─ unim-xim (Child) ││
│  │ InputMethod  │  │ InputEngine  │  │  └─ unim-wayland     ││
│  │ InputContext │  │  (HashMap)   │  │                      ││
│  └──────────────┘  └──────────────┘  └──────────────────────┘│
│                                                               │
│  ┌──────────────┐  ┌──────────────┐                          │
│  │ PID 관리     │  │ 시그널 처리  │                          │
│  │ (파일 기반)  │  │ (SIGINT)     │                          │
│  └──────────────┘  └──────────────┘                          │
└───────────────────────────────────────────────────────────────┘
```

### 1.2 의존성

| 크레이트 | 버전 | 용도 |
|----------|------|------|
| `unim` | (로컬) | 코어 엔진, 설정, 로깅 |
| `unim-dbus` | (로컬) | DBus 서비스/클라이언트 |
| `tokio` | 1.x | 비동기 런타임 (rt-multi-thread, signal) |
| `zbus` | 4.x | DBus 통신 |
| `clap` | 4.4 | CLI 인수 파싱 |
| `daemonize` | 0.5 | 프로세스 데몬화 |
| `dirs` | 5.0 | XDG 런타임 디렉토리 |
| `which` | 6.0 | PATH 탐색 |
| `env_logger` | 0.11 | 로그 초기화 |

---

## 2. CLI 인터페이스

```
unim-daemon [옵션]
```

| 옵션 | 설명 |
|------|------|
| `-n`, `--no-daemon` | 데몬화 없이 포그라운드 실행 |
| `-r`, `--replace` | 기존 데몬 강제 종료 후 교체 실행 |
| `--check` | 실행 여부만 확인 (실행 중: exit 0, 아니면: exit 1) |

---

## 3. 시작 순서 (`main`)

```
1. env_logger 초기화
2. CLI 인수 파싱 (clap)
3. PID 파일 경로 결정 ($XDG_RUNTIME_DIR/unim-daemon.pid)

4. --check → 실행 여부 확인 → exit 0/1

5. 기존 데몬 확인:
   → --replace → kill_existing_daemon() + 500ms 대기
   → 없으면 → exit 1

6. 설정 로드 (Config::load_from_default_path)

7. 데몬화:
   → --no-daemon 아니면 → daemonize (PID 파일 자동 생성)
   → --no-daemon이면 → 수동 PID 파일 작성

8. 엔진 워커 시작 (spawn_engine_worker)
9. DBus 서비스 시작 (start_dbus_service)
10. 프론트엔드 모듈 감지 + 시작

11. tokio::select! {
      종료 시그널 (Ctrl+C) 대기
      프로세스 모니터링 루프 (1초 간격)
    }

12. 정리:
    → 자식 프로세스 kill
    → PID 파일 삭제
```

---

## 4. 프론트엔드 모듈 관리

### 4.1 모듈 종류

| 모듈 | 바이너리 | 감지 조건 | 설명 |
|------|----------|-----------|------|
| `Xim` | `unim-xim` | `$DISPLAY` 존재 | X11 XIM 서버 |
| `Wayland` | `unim-wayland` | `$WAYLAND_DISPLAY` 존재 | Wayland IM |

> [!NOTE]
> GTK3/4, Qt5/6 프론트엔드는 IM 모듈(`.so`)로 애플리케이션 내에서 로드되므로
> 데몬이 관리하지 않습니다. 데몬은 **헤드리스 프론트엔드**만 자식 프로세스로 관리합니다.

### 4.2 바이너리 탐색 순서 (`resolve_path`)

```
1. 환경 변수: $UNIM_UNIM_XIM_PATH 또는 $UNIM_UNIM_WAYLAND_PATH
2. 실행 파일과 같은 디렉토리 (libexec)
3. 빌드 디렉토리 (UNIM_DEVELOP=1 시):
   → target/release/{name}
   → target/debug/{name}
   → ../target/release/{name}
   → ../target/debug/{name}
4. 시스템 경로:
   → /usr/local/libexec/{name}
   → /usr/libexec/{name}
   → /usr/local/bin/{name}
   → /usr/bin/{name}
5. PATH (which::which)
```

### 4.3 프로세스 시작 (`start_module`)

```rust
Command::new(&path)
    .stdin(Stdio::null())
    .stdout(Stdio::inherit())   // 데몬의 stdout 상속
    .stderr(Stdio::inherit())   // 데몬의 stderr 상속
    .spawn()
```

### 4.4 프로세스 모니터링

```
1초 간격 루프:
  → processes.retain_mut(|(name, process)| {
      process.try_wait() → 종료됨 → 로그 출력 + 제거
                         → 실행 중 → 유지
    })
```

> [!NOTE]
> 현재는 프로세스 종료 후 자동 재시작하지 않습니다.
> `retain_mut`로 종료된 프로세스를 리스트에서 제거합니다.

---

## 5. DBus 서비스 초기화 (`start_dbus_service`)

```
1. Config::load_from_default_path()
2. Connection::session().await  (세션 버스 연결)
3. RequestName("org.atit.unim.InputMethod",
               ReplaceExisting | AllowReplacement)
   → Exists: kill_existing_daemon() + 500ms 대기 + 재시도
   → PrimaryOwner: 성공
   → AlreadyOwner: 이미 소유자
   → InQueue: 대기열
   → Exists (재시도 후): Error 반환
4. InputMethodService 생성 + DBus 등록
   → at("/org/atit/unim/InputMethod", service)
5. Connection 반환
```

### 5.1 DBus 이름 충돌 처리

| 상황 | 동작 |
|------|------|
| 처음 실행 | `PrimaryOwner` → 정상 시작 |
| 기존 데몬 존재 | `Exists` → PID 파일 기반 kill → 재시도 |
| 재시도 실패 | `Exists` → `Error("Name already taken")` → exit 1 |
| `--replace` 사용 | CLI 레벨에서 먼저 기존 프로세스 종료 |

---

## 6. PID 파일 관리

### 6.1 경로

```
$XDG_RUNTIME_DIR/unim-daemon.pid
```

폴백: `/tmp/unim-daemon.pid`

### 6.2 생명주기

| 단계 | 동작 |
|------|------|
| 데몬화 시 (`daemonize`) | PID 파일 자동 생성 |
| 포그라운드 (`-n`) | `write_pid_file()` 수동 생성 |
| 종료 시 | `remove_pid_file()` 삭제 |
| Stale 감지 | `/proc/{pid}` 확인 → 없으면 삭제 |

### 6.3 기존 데몬 종료 (`kill_existing_daemon`)

```
1. PID 파일에서 PID 읽기 → kill {pid} (SIGTERM)
   → 200ms 대기

2. pgrep -f "unim-daemon" → 자기 자신 제외 모두 kill
```

---

## 7. 데몬화 (`daemonize`)

`--no-daemon` 미지정 시:

```rust
daemonize::Daemonize::new()
    .pid_file(&pid_file)            // PID 파일 자동 생성
    .working_directory("/tmp")      // 작업 디렉토리 변경
    .start()
```

- `fork()` → 부모 종료 → 자식이 새 세션 리더
- stdout/stderr 분리
- PID 파일에 자식 PID 기록

---

## 8. 종료 처리

### 8.1 시그널

```rust
tokio::signal::ctrl_c().await  // SIGINT (Ctrl+C) 대기
→ running.store(false, SeqCst)
```

### 8.2 정리 순서

```
1. tokio::select! 종료
2. 모든 자식 프로세스 kill (SIGKILL)
3. PID 파일 삭제
4. 로그: "UNIM 데몬 종료"
```

---

## 9. systemd 서비스

### 9.1 유닛 파일 (`unim-daemon.service`)

```ini
[Unit]
Description=UNIM Input Method Daemon
After=graphical-session.target
PartOf=graphical-session.target

[Service]
Type=simple
ExecStart=@LIBEXECDIR@/unim-daemon -n
Restart=on-failure
RestartSec=3

[Install]
WantedBy=graphical-session.target
```

| 항목 | 값 | 설명 |
|------|-----|------|
| Type | `simple` | `-n` (포그라운드) 모드 |
| After | `graphical-session.target` | 그래픽 세션 준비 후 시작 |
| Restart | `on-failure` | 비정상 종료 시 자동 재시작 |
| RestartSec | 3초 | 재시작 간격 |

> [!IMPORTANT]
> systemd로 실행 시 반드시 `-n` (no-daemon) 플래그를 사용합니다.
> 이중 fork는 systemd의 프로세스 추적을 방해합니다.

### 9.2 설치 경로

```
~/.config/systemd/user/unim-daemon.service   (사용자 서비스)
```

관리 명령:
```bash
systemctl --user enable unim-daemon
systemctl --user start unim-daemon
systemctl --user status unim-daemon
```

---

## 10. 빌드 및 설치

### 10.1 빌드

```bash
cargo build -p unim-daemon --release
```

또는:

```bash
make build-rust
```

### 10.2 설치 경로

| 파일 | 경로 |
|------|------|
| 바이너리 | `/usr/libexec/unim-daemon` |
| systemd 유닛 | `~/.config/systemd/user/unim-daemon.service` |

---

## 11. 환경 변수

| 변수 | 용도 |
|------|------|
| `DISPLAY` | X11 환경 감지 → XIM 모듈 시작 |
| `WAYLAND_DISPLAY` | Wayland 환경 감지 → Wayland 모듈 시작 |
| `UNIM_DEVELOP` | 디버그 로깅 + 개발용 빌드 디렉토리 탐색 |
| `UNIM_UNIM_XIM_PATH` | unim-xim 바이너리 경로 오버라이드 |
| `UNIM_UNIM_WAYLAND_PATH` | unim-wayland 바이너리 경로 오버라이드 |

---

## 12. 로깅

모듈명: `DAEMON`

```rust
unim_log!("DAEMON", "UNIM 데몬 시작...");
unim_log!("DAEMON", "[DBus] 서비스 등록: {}", INPUT_METHOD_PATH);
```

활성화: `UNIM_DEVELOP=1`

출력:
- 콘솔 (포그라운드 모드)
- `~/.unim-errors.log`
