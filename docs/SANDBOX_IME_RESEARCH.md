# Sandboxed App (Flatpak/Snap) 환경에서의 입력기 호환성 연구

## 1. 핵심 문제

Flatpak/Snap 앱은 자체 GTK/Qt 라이브러리를 번들링하며, 호스트 시스템의 IM 모듈(`im-unim.so` 등)을 볼 수 없다. Flatpak의 freedesktop-sdk 런타임은 **IBus IM 모듈만** 하드코딩으로 포함한다.

결과적으로:
- UNIM의 GTK3/4, Qt5/6 IM 모듈은 샌드박스 앱에서 로드 불가
- 앱은 IBus IM 모듈(`im-ibus.so`)을 통해서만 입력기와 통신
- Fcitx5도 같은 문제를 겪으며, 독자적 해결 방법을 사용

## 2. IBus가 사실상 표준인 이유

### GNOME 통합
- GNOME 3.6부터 IBus를 기본 입력기 프레임워크로 내장
- Mutter 컴포지터가 Wayland에서 `text-input-v3` 프로토콜을 받아 IBus DBus 프로토콜로 변환
- Ubuntu, Fedora 등 주요 배포판이 IBus를 기본값으로 설정

### Flatpak 런타임 구조
- `org.freedesktop.Sdk`/`org.gnome.Sdk` 런타임에 `im-ibus.so` 포함
- GTK_IM_MODULE=ibus가 기본값으로 설정됨
- IBus Portal (`org.freedesktop.portal.IBus`) 서비스로 샌드박스 앱이 안전하게 IBus 접근

### IBus Portal 메커니즘
- Flatpak 앱이 `IBUS_USE_PORTAL=1` 환경에서 실행되면:
  - 일반 IBus 소켓 대신 `org.freedesktop.portal.IBus` (세션 버스)에 연결
  - `org.freedesktop.IBus.Portal` 인터페이스 사용 (제한된 API)
  - `CreateInputContext` + InputContext 인터페이스만 노출
  - 각 클라이언트는 자신이 만든 InputContext만 접근 가능 (보안 격리)

### Snap 상황
- `desktop-legacy` 인터페이스로 IBus/Fcitx 접근 제공
- 하지만 DBus 통신 및 XDG_CACHE_HOME 접근 문제로 불안정
- IBus 소켓 파일 접근 권한 이슈 빈번

## 3. Fcitx5의 해결 방법

### IBus Frontend (핵심 전략)
Fcitx5는 `fcitx5-module-ibus` 패키지로 **IBus DBus 프로토콜을 에뮬레이션**한다:

1. **ibus-daemon 대체**: Fcitx5가 자동시작 시 기존 ibus-daemon을 대체
2. **IBus DBus 인터페이스 구현**: `org.freedesktop.IBus` 인터페이스를 직접 제공
3. **앱 투명 전환**: 앱 입장에서는 IBus와 통신하는 것처럼 보이지만 실제로는 Fcitx5가 처리

### GNOME에서의 동작 방식
- GNOME/Mutter는 Compositor → Input Method 통신에 IBus DBus 프로토콜 사용
- Fcitx5의 IBus Frontend가 이 프로토콜을 받아 처리
- 앱 → `im-ibus.so` (Flatpak 런타임 내장) → IBus Portal/Socket → Fcitx5 (IBus Frontend)
- **결과: Flatpak 앱에서도 작동** (앱은 IBus로 알고 있지만 실제로는 Fcitx5)

### Wayland에서의 추가 경로
- GTK_IM_MODULE을 설정하지 않으면 GTK3/4가 자동으로 Wayland `text-input-v3` 사용
- 컴포지터(Mutter/KWin)가 text-input-v3를 받아 input-method 프로토콜로 전달
- Fcitx5가 input-method-v2 Wayland 프로토콜로 직접 통신 가능 (KDE Plasma)
- GNOME은 IBus DBus 프로토콜만 지원 → Fcitx5의 IBus Frontend 필수

## 4. IBus 프로토콜 기술 상세

### 통신 구조
```
일반 모드:
  앱 → im-ibus.so → private IBus socket → ibus-daemon → IBusEngine

Portal 모드 (Flatpak):
  앱 → im-ibus.so → session bus → org.freedesktop.portal.IBus → ibus-daemon → IBusEngine
```

### IBus 주소 체계
- IBus는 **private session**을 사용 (일반 D-Bus 세션 버스가 아님)
- 주소 파일: `~/.config/ibus/bus/{machine-id}-{host}-{display}`
- 파일 내용: `IBUS_ADDRESS=unix:abstract=/.../.cache/ibus/dbus-xxxxx`

### 핵심 DBus 인터페이스
```
org.freedesktop.IBus (메인 인터페이스):
  - CreateInputContext(client_name) → object_path
  - CurrentInputContext() → object_path
  - RegisterComponent(component)
  - ListEngines() → engines
  - SetGlobalEngine(engine_name)

org.freedesktop.IBus.InputContext (입력 컨텍스트):
  Methods:
    - ProcessKeyEvent(keyval, keycode, state) → handled
    - SetCapabilities(caps)
    - FocusIn() / FocusOut()
    - Reset()
    - SetCursorLocation(x, y, w, h)
  Signals:
    - CommitText(text)
    - UpdatePreeditText(text, cursor_pos, visible)
    - ShowPreeditText() / HidePreeditText()
    - UpdateAuxiliaryText(text, visible)
    - UpdateLookupTable(table, visible)

org.freedesktop.IBus.Engine (엔진 인터페이스):
  - process_key_event(keyval, keycode, state) → handled
  - commit_text(text)
  - update_preedit_text(text, cursor_pos, visible)
  - focus_in() / focus_out()
  - reset()
  - set_cursor_location(x, y, w, h)

org.freedesktop.IBus.Portal (Flatpak용):
  - CreateInputContext(client_name) → object_path
  (InputContext 인터페이스 + Service.Destroy만 노출)
```

### IBusEngine 구현 (엔진 측)
IBusEngine은 `IBusService`의 서브클래스로 다음 시그널/콜백을 구현:
- `process-key-event`: 키 입력 처리, True(처리됨)/False(패스스루) 반환
- `commit_text()`: 완성된 텍스트를 앱에 전달
- `update_preedit_text()`: 조합 중인 텍스트 표시
- `candidate-clicked`: 후보 선택 처리
- `focus-in`/`focus-out`: 포커스 변경 시 상태 관리

## 5. Wayland text-input-v3 경로

### 프로토콜 체인
```
앱 (GTK/Qt) → text-input-v3 → 컴포지터(Mutter/KWin) → input-method-v2 → 입력기
```

### Flatpak에서의 의미
- Flatpak 앱이 Wayland 네이티브로 실행되면 `text-input-v3`를 사용 가능
- **IM 모듈이 전혀 필요 없음** — 프로토콜이 컴포지터 레벨에서 작동
- GTK_IM_MODULE을 설정하지 않으면 GTK3/4가 자동으로 `text-input-v3` 사용
- Qt 6.7+에서 `text-input-v3` 지원 추가 (6.8.2+에서 안정)

### 한계
- GNOME/Mutter는 `input-method-v2`를 지원하지 않음 (IBus DBus만 사용)
- KDE Plasma(KWin)는 `input-method-v2` 지원
- Sway도 `input-method-v2` 지원
- 결국 **GNOME에서는 이 경로로 직접 입력기를 연결할 수 없음**

## 6. UNIM을 위한 가능한 솔루션

### Option A: IBus Engine 래퍼 (가장 현실적, Fcitx5와 동일한 접근)

**개요**: UNIM 코어를 IBus 엔진으로 등록하여 ibus-daemon을 통해 서비스

```
앱(Flatpak 포함) → im-ibus.so → ibus-daemon → unim-ibus-engine → UNIM 코어
```

**구현 방법**:
1. `ibus-hangul` 같은 IBus 엔진 컴포넌트 작성 (Rust로 가능)
2. IBusEngine 인터페이스 구현 (process_key_event, commit_text, update_preedit_text)
3. IBus에 컴포넌트 등록 (`.xml` 파일 + RegisterComponent)
4. UNIM 코어 엔진(`src/`)을 직접 호출하여 한글 조합 처리

**장점**:
- 모든 Flatpak/Snap 앱에서 즉시 작동
- IBus Portal을 통한 샌드박스 호환성 자동 확보
- 기존 UNIM 코어 엔진 재사용 가능
- `ibus` Rust crate 사용 가능 (docs.rs/ibus)

**단점**:
- ibus-daemon 의존성 추가
- UNIM만의 독자적 DBus 프로토콜과 병행 운용 필요
- 한자 팝업 등 커스텀 UI 기능이 IBus 프레임워크 내로 제한

**참고 crate**: `ibus` (0.2.0) — IBus 클라이언트 구현, `ibus-rs` (GitHub)

### Option B: IBus 프로토콜 에뮬레이션 (Fcitx5 방식)

**개요**: unim-daemon이 직접 IBus DBus 인터페이스를 구현하여 ibus-daemon을 대체

```
앱(Flatpak 포함) → im-ibus.so → unim-daemon (IBus 에뮬레이션) → UNIM 코어
```

**구현 방법**:
1. `org.freedesktop.IBus` DBus 인터페이스를 unim-daemon에 구현
2. `CreateInputContext`, `ProcessKeyEvent` 등 핵심 메서드 구현
3. IBus Portal (`org.freedesktop.portal.IBus`) 인터페이스도 구현
4. unim-daemon 시작 시 ibus-daemon을 대체
5. IBus private session bus 프로토콜까지 에뮬레이션

**장점**:
- ibus-daemon 없이 완전 독립 운영
- Flatpak 앱이 IBus로 인식하여 자동 호환
- UNIM의 모든 기능(한자 팝업, 특수문자 등)을 완벽 제어
- Fcitx5가 이미 이 방식으로 검증됨

**단점**:
- 구현 복잡도 매우 높음 (IBus의 private session bus 프로토콜 전체 에뮬레이션)
- IBus API 호환성 유지 부담
- 디버깅 어려움

### Option C: Wayland input-method-v2 프로토콜 (제한적)

**개요**: Wayland 컴포지터의 input-method-v2 프로토콜을 통해 직접 연결

```
앱 → text-input-v3 → 컴포지터 → input-method-v2 → UNIM
```

**현재 상태**: UNIM은 이미 `unim-frontends/wayland/`에 이 구현이 있음

**장점**:
- IM 모듈 불필요 — 샌드박스 문제 자체가 사라짐
- KDE Plasma, Sway에서 작동
- 가장 "올바른" Wayland 네이티브 접근

**단점**:
- **GNOME/Mutter가 input-method-v2를 지원하지 않음** (치명적)
- GNOME에서는 여전히 IBus DBus 경로 필수
- Ubuntu 기본 환경(GNOME)에서 작동 불가

### Option D: Flatpak Extension

**개요**: UNIM IM 모듈을 Flatpak 확장으로 배포

**장점**:
- 직접적인 IM 모듈 사용 가능

**단점**:
- 사용자가 별도 확장 설치 필요
- 런타임별(GNOME, KDE 등) 별도 빌드 필요
- Fcitx5 개발자(wengxt)도 이 방식은 비현실적이라 판단 (Issue #43, #108)
- Snap에서는 불가능

## 7. 권장 전략

### 단기 (Phase 1): IBus Engine 래퍼 — Option A

가장 빠르고 현실적인 접근:

1. `unim-ibus-engine` 크레이트 생성
2. IBusEngine 인터페이스를 Rust로 구현 (dbus-rs 또는 zbus 사용)
3. UNIM 코어(`src/`)를 직접 호출하여 process_key_event 처리
4. `.xml` 컴포넌트 파일로 IBus에 등록
5. 기존 UNIM daemon/frontends와 병행 운영

이렇게 하면:
- 모든 Flatpak/Snap 앱 지원
- 기존 네이티브 IM 모듈도 계속 사용 (더 나은 성능)
- IBus가 설치된 모든 환경에서 작동

### 중기 (Phase 2): IBus 프로토콜 에뮬레이션 검토 — Option B

Fcitx5 소스 코드 참고하여 IBus 프로토콜 에뮬레이션 가능성 평가:
- `fcitx5/src/modules/dbus/dbusmodule.cpp` 참조
- IBus Portal 구현 복잡도 평가
- ibus-daemon 대체가 가치 있는지 판단

### 장기: Wayland input-method-v2 관련

GNOME이 `input-method-v2`를 지원할 때까지 대기. 그때까지는 GNOME에서 IBus 경로가 유일한 옵션.

## 8. 참고 자료

### 핵심 문서
- Fcitx5 Wayland 가이드: https://fcitx-im.org/wiki/Using_Fcitx_5_on_Wayland
- Fcitx5 Setup: https://fcitx-im.org/wiki/Setup_Fcitx_5
- IBus 프로토콜 분석 (한국어): https://seoyoungjin.github.io/ibus/text%20input/IBus/
- Linux 입력기 동작 원리: https://nerufic.com/en/posts/how-input-methods-work-in-linux/
- Wayland 입력기 프로토콜: https://dorotac.eu/posts/input_method/

### IBus 개발
- IBus Engine API: https://ibus.github.io/docs/ibus-1.5/IBusEngine.html
- IBus Rust crate: https://docs.rs/ibus/latest/ibus/ (클라이언트 전용, 엔진 측 미지원)
- ibus-rs: https://github.com/ArturKovacs/ibus-rs
- IBus 커스텀 엔진 만들기: https://studymongolian.net/technical/how-to-create-linux-input-method-editor/

### Flatpak/Snap 이슈
- Flatpak IBus 이슈: https://github.com/flatpak/flatpak/issues/675
- IBus Portal 패치: https://github.com/flatpak/freedesktop-sdk-images/blob/1.6/ibus-portal.patch
- Flatpak IM 모듈 이슈: https://github.com/flatpak/freedesktop-sdk-images/issues/43
- Fcitx5 Flatpak 논의: https://github.com/fcitx/fcitx5/issues/108
- Snap 입력기 이슈: https://forum.snapcraft.io/t/cant-use-input-method-in-snap-apps/4712

### Wayland 프로토콜
- text-input-v3: https://wayland.app/protocols/text-input-unstable-v3
- Fcitx5 Wayland: https://fcitx-im.org/wiki/Using_Fcitx_5_on_Wayland
