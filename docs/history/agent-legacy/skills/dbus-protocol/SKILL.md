---
name: dbus-protocol
description: UNIM DBus 프로토콜 참조 - 메서드, 시그널, 입력 컨텍스트 관리 패턴
---

# UNIM DBus 프로토콜 스킬

UNIM의 DBus 통신 패턴을 이해하고, 새 메서드/시그널을 추가하거나 기존 프로토콜을 수정할 때 사용합니다.

## 서비스 정보

- **버스**: Session Bus
- **서비스명**: `org.atit.unim.InputMethod`
- **기본 경로**: `/org/atit/unim/InputMethod`
- **인터페이스**: `org.atit.unim.InputMethod`

## 아키텍처

```text
┌─────────────┐     ┌──────────────┐     ┌──────────────┐
│ IM Module    │────▶│  DBus Layer  │────▶│ EngineWorker │
│ (Client)     │◀────│  (Service)   │◀────│   (Thread)   │
└─────────────┘     └──────────────┘     └──────────────┘
    GTK/Qt/XIM       unim-dbus/           src/input_engine
```

- **요청**: 클라이언트 → `unim-dbus/src/service.rs` → `EngineWorker` (채널)
- **응답**: `EngineWorker` → DBus 시그널 → 클라이언트

## 핵심 메서드

### 입력 컨텍스트 관리

| 메서드 | 인자 | 반환 | 설명 |
| ------ | ---- | ---- | ---- |
| `CreateInputContext` | `app_name: s` | `context_path: o` | 새 입력 컨텍스트 생성 |
| `DestroyInputContext` | | | 입력 컨텍스트 해제 |
| `FocusIn` | `window_id: s` | | 포커스 진입 |
| `FocusOut` | | | 포커스 이탈 |
| `Reset` | | | preedit 초기화 |

### 키 처리

| 메서드 | 인자 | 반환 | 설명 |
| ------ | ---- | ---- | ---- |
| `ProcessKeyEvent` | `keyval: u, keycode: u, modifiers: u` | `consumed: b` | 키 이벤트 처리 |

### 모드/설정

| 메서드 | 인자 | 반환 | 설명 |
| ------ | ---- | ---- | ---- |
| `GetMode` | | `mode: s` | 현재 입력 모드 조회 |
| `SetMode` | `mode: s` | | 입력 모드 설정 |
| `GetConfig` | `key: s` | `value: s` | 설정값 조회 |
| `SetConfig` | `key: s, value: s` | | 설정값 변경 |

### 한자 (Hanja)

| 메서드 | 인자 | 반환 | 설명 |
| ------ | ---- | ---- | ---- |
| `GetHanjaCandidates` | | `candidates: as` | 한자 후보 목록 조회 |
| `SelectHanjaCandidate` | `index: u` | | 한자 후보 선택 |

## 핵심 시그널

| 시그널 | 인자 | 설명 |
| ------ | ---- | ---- |
| `CommitText` | `text: s` | 텍스트 확정 (앱에 입력) |
| `UpdatePreedit` | `text: s, cursor_pos: i` | preedit 텍스트 업데이트 |
| `GlobalModeChanged` | `mode: s` | 전역 한/영 모드 변경 |
| `ConfigChanged` | `key: s, value: s` | 설정 변경 알림 |

## 새 메서드 추가 절차

### 1. 서비스 측 (`unim-dbus/src/service.rs`)

```rust
// DBus 인터페이스에 메서드 추가
#[dbus_interface(name = "org.atit.unim.InputMethod")]
impl InputMethodService {
    async fn new_method(&self, arg: String) -> Result<String, fdo::Error> {
        // EngineWorker로 요청 전달
        let result = self.send_request(EngineRequest::NewMethod(arg)).await?;
        Ok(result)
    }
}
```

### 2. 클라이언트 측 (`unim-dbus/src/client.rs`)

```rust
pub async fn new_method(&self, arg: &str) -> Result<String> {
    let proxy = self.proxy().await?;
    let result: String = proxy.call("NewMethod", &(arg,)).await?;
    Ok(result)
}
```

### 3. 프론트엔드 연동

각 프론트엔드(GTK/Qt/XIM)의 DBus 클라이언트에서 새 메서드를 호출하도록 업데이트합니다.

## 디버깅 도구

```bash
# 서비스 확인
busctl --user list | grep unim

# 인터페이스 조사
busctl --user introspect org.atit.unim.InputMethod /org/atit/unim/InputMethod

# 시그널 모니터링
dbus-monitor --session "interface='org.atit.unim.InputMethod'"

# 수동 메서드 호출
busctl --user call org.atit.unim.InputMethod \
  /org/atit/unim/InputMethod \
  org.atit.unim.InputMethod \
  GetMode
```
