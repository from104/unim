# UNIM 버그 종합 진단 리포트

**작성일**: 2026-04-04  
**대상 브랜치**: develop  
**분석 범위**: Core Engine, DBus Daemon, XIM/GTK/Qt/Wayland/GNOME Extension 프론트엔드 전체

---

## 버그 1: XIM 호환 앱에서 연속 한글 입력 시 2번째 초성 preedit 미표시

### 증상

- **발생 환경**: 일부 XIM 호환 앱 (GTK2 기반 앱, 특정 터미널 등)
- **재현 시나리오**: 연속 한글 입력 시 받침이 불가능한 상황에서 새 초성 입력
  - 예: "간" + ㄴ → commit "간" + preedit "ㄴ" (ㄴ이 안 보임)
  - 예: "앙" + ㄱ → commit "앙" + preedit "ㄱ" (ㄱ이 안 보임)
  - 모음을 입력하면 그때서야 preedit이 보임 ("나", "가" 등)

### 근본 원인 분석

#### 1차 원인: X11 커넥션 중간 flush로 인한 commit/preedit 분리

**파일**: `unim-frontends/xim/src/handler.rs` (라인 1109-1131)

```rust
// Commit 처리 (라인 1115-1121)
if let Some(commit_text) = commit {
    if !commit_text.is_empty() {
        server.commit(&user_ic.ic, &commit_text)?;
        server.conn().flush().ok();    // ← 여기서 commit만 먼저 전송!
    }
}

// Preedit 처리 (라인 1123-1131)
let preedit_text = preedit.unwrap_or_default();
if preedit_text.is_empty() {
    self.clear_preedit(server, user_ic)?;
} else {
    self.preedit(server, user_ic, &preedit_text)?;
}
server.conn().flush().ok();            // ← preedit은 여기서 전송
```

**문제**: `server.conn().flush().ok()` (라인 1119)이 commit과 preedit 사이에 있어서:
1. commit 메시지가 X11 클라이언트에 **먼저** 도달
2. 클라이언트가 commit을 처리하며 내부 상태를 업데이트
3. 이후 PreeditDraw가 도달하지만, 일부 클라이언트가 이를 무시하거나 제대로 렌더링하지 않음

GTK4/Qt 등은 GLib/Qt 이벤트 루프에서 preedit-changed 시그널을 직접 emit하므로 이 문제가 없음.

#### 2차 원인: XIM 프로토콜 상태 머신의 빠른 전환 문제

**파일**: xim 크레이트 `server.rs` (라인 176-230)

xim 크레이트의 `preedit_draw()` 구현:
- 빈 문자열 → `PreeditDraw("")` + `PreeditDone` (세션 종료, `preedit_started = false`)
- 비어있지 않은 문자열 → `preedit_started`가 false면 `PreeditStart` 먼저 → `PreeditDraw`

만약 어떤 이유로 preedit이 한번 비워졌다가 다시 설정되면:
```
PreeditDraw("") → PreeditDone → PreeditStart → PreeditDraw("ㄴ")
```
이 빠른 전환을 일부 XIM 클라이언트가 올바르게 처리하지 못함.
(handler.rs 라인 1110-1114 주석에 이미 이 문제가 기록되어 있음)

### 영향받는 코드 경로

| 파일 | 라인 | 내용 |
|------|------|------|
| `unim-frontends/xim/src/handler.rs` | 1115-1121 | commit 처리 + 중간 flush |
| `unim-frontends/xim/src/handler.rs` | 1123-1131 | preedit 처리 |
| `unim-frontends/xim/src/handler.rs` | 383-443 | preedit() 메서드 |
| `unim-frontends/xim/src/handler.rs` | 445-464 | clear_preedit() 메서드 |
| xim 크레이트 `server.rs` | 176-230 | preedit_draw XIM 프로토콜 구현 |
| `src/input_engine.rs` | 499-534 | 자모 처리 (commit+preedit 동시 발생) |
| `unim-dbus/src/engine_worker.rs` | 167-171 | preedit 추출 로직 |

### 수정 방안

**수정 A (핵심)**: 중간 flush 제거 — commit과 preedit을 하나의 atomic batch로 전송

```rust
// 수정 전
server.commit(&user_ic.ic, &commit_text)?;
server.conn().flush().ok();    // 제거

// 수정 후
server.commit(&user_ic.ic, &commit_text)?;
// flush는 preedit 처리 후 한 번만 수행
```

이렇게 하면 commit + PreeditDraw가 하나의 X11 메시지 배치로 전달되어 클라이언트가 원자적으로 처리 가능.

---

## 버그 2: 포커스 아웃/리셋 시 커밋 지연

### 증상

- **발생 환경**: 한글 조합 중 포커스 아웃 또는 리셋 시
- **증상**: 커밋이 순간적으로 지연된 후 반영됨 (체감 10-50ms)
- **원인 커밋**: `945956a` (2026-03-31) "fix: use server response for focus-out commit instead of local cache"

### 근본 원인 분석

#### 동기 DBus 라운드트립 아키텍처

커밋 `945956a` 이전에는 로컬 preedit_cache를 즉시 사용하여 커밋했으나, 데몬과의 상태 불일치 문제가 있어 서버 응답을 사용하도록 변경됨. 하지만 이로 인해 모든 프론트엔드에서 동기 블로킹 호출이 발생:

| 프론트엔드 | 호출 방식 | 타임아웃 | 블로킹 |
|-----------|----------|---------|-------|
| GTK3/GTK4 | `g_dbus_connection_call_sync()` | 500ms | YES |
| Qt5/Qt6 | `QDBusConnection::call(Block)` | 500ms | YES |
| XIM | `recv_timeout(500ms)` | 500ms | YES |
| Wayland | `recv_timeout(500ms)` | 500ms | YES |
| GNOME Ext | `call_sync()` | 500ms | YES |

#### Focus-Out 처리 흐름 (현재)

```
프론트엔드: focus-out 이벤트 발생
    ↓
프론트엔드: DBus FocusOut 동기 호출 (블로킹!)
    ↓ ← 네트워크 라운드트립 지연 ←
데몬: EngineRequest::FocusOut → engine_worker 처리
    ↓
데몬: commit 텍스트 반환
    ↓
프론트엔드: 반환된 commit 텍스트로 커밋
```

#### 지연의 구성 요소

1. DBus 메시지 전송 (프론트엔드 → 데몬): ~1-5ms
2. engine_worker 비동기 채널 처리: ~1-5ms  
3. 엔진 상태 처리 (팝업 확인, preedit flush, 상태 리셋): ~1ms
4. DBus 응답 전송 (데몬 → 프론트엔드): ~1-5ms
5. **총 지연**: 약 5-20ms (정상), 최대 500ms (부하 시)

### 영향받는 코드 경로

| 파일 | 라인 | 내용 |
|------|------|------|
| `unim-frontends/gtk-common/src/unim_dbus_client.c` | 282-326 | GTK focus_out DBus 동기 호출 |
| `unim-frontends/gtk4/src/immodule.c` | 853-866 | GTK4 focus_out 처리 |
| `unim-frontends/xim/src/handler.rs` | 745-814 | XIM focus_out 처리 |
| `unim-dbus/src/service.rs` | 814-830 | 데몬 FocusOut 핸들러 |
| `unim-dbus/src/engine_worker.rs` | FocusOut 핸들러 | engine_worker 처리 |

### 수정 방안

**수정 B**: XIM 프론트엔드에서 로컬 캐시 우선 커밋 + 비동기 데몬 동기화

XIM은 자체 preedit_cache를 유지하고 있으므로 (handler.rs 라인 390), 이를 즉시 커밋에 사용하고 데몬에는 비동기로 알림:

```rust
// 1. 로컬 캐시로 즉시 커밋
let cached = user_ic.user_data.preedit_cache.clone();
if !cached.is_empty() {
    server.commit(&user_ic.ic, &cached)?;
}

// 2. 데몬에 비동기 알림 (응답 불필요)
self.send_dbus_request_fire_and_forget(DbusRequest::FocusOut { ... });

// 3. preedit 정리
self.clear_preedit(server, user_ic)?;
```

**주의**: GTK/Qt 프론트엔드는 이미 로컬 캐시 폴백이 있음 (gtk-common 라인 320-325). 하지만 서버 응답을 1순위로 사용하는 현재 방식이 안정적이므로, XIM에서만 로컬 우선 전략을 적용하는 것이 안전.

**대안**: 현재 방식을 유지하되, 데몬 측에서 FocusOut 처리를 최적화하여 라운드트립 시간을 단축.

---

## 검증 계획

### 버그 1 테스트

1. **XIM 앱에서 연속 입력 테스트**
   - xterm, urxvt, 또는 GTK2 앱에서 테스트
   - "간ㄴ" 입력 시 ㄴ의 preedit이 즉시 표시되는지 확인
   - "앙ㄱ" 입력 시 ㄱ의 preedit이 즉시 표시되는지 확인

2. **UNIM_DEVELOP=1 로그 확인**
   - `~/.unim-errors.log`에서 commit과 preedit의 타이밍 확인
   - PreeditDraw 호출 순서 확인

### 버그 2 테스트

1. **포커스 전환 테스트**
   - 한글 조합 중 다른 창으로 Alt+Tab
   - 조합 중이던 글자가 즉시 커밋되는지 확인
   - 지연 없이 반영되는지 체감 확인

2. **리셋 테스트**
   - 조합 중 Escape 또는 마우스 클릭으로 리셋
   - 커밋 타이밍 확인

---

## 수정 우선순위

| 순서 | 수정 | 위험도 | 영향 범위 |
|------|------|--------|----------|
| 1 | **수정 A**: XIM 중간 flush 제거 | 낮음 | XIM 프론트엔드만 |
| 2 | **수정 B**: XIM focus-out 로컬 캐시 우선 | 중간 | XIM 프론트엔드만 |

수정 A는 단순하고 안전하며, 수정 B는 기존 동작 변경이므로 신중하게 적용.
