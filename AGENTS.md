# UNIM Project Agent Context

> 이 파일은 AI 코딩 어시스턴트를 위한 프로젝트 컨텍스트입니다.
> 프로젝트별 컨벤션은 [GEMINI.md](GEMINI.md)를 참조하세요.

## 프로젝트 개요

**UNIM** (Universal Next-generation Input Method)은 Rust로 작성된 한국어 입력기(IME)입니다.
3계층 아키텍처(Core → DBus → Frontend)로, 모든 주요 리눅스 데스크톱 툴킷을 지원합니다.

## 컴포넌트 맵

| 컴포넌트 | 경로 | 언어 | 역할 |
| -------- | ---- | ---- | ---- |
| **Core Engine** | `src/` | Rust | 한글 조합/분해 로직, 설정, 키맵 |
| **C-API** | `unim-capi/` | Rust (FFI) | Core를 C/C++에서 사용하기 위한 래퍼 |
| **DBus Daemon** | `unim-daemon/` | Rust | 중앙 엔진 서버 (unim-daemon) |
| **DBus Library** | `unim-dbus/` | Rust | DBus 서비스/클라이언트 구현 |
| **CLI** | `unim-cli/` | Rust | 명령줄 인터페이스 |
| **Config CLI** | `unim-config/` | Rust | 설정 관리 CLI 도구 |
| **GUI Common** | `unim-gui-common/` | Rust | DBus 통신, 트레이 공통 로직 |
| **GUI GTK** | `unim-gui-gtk/` | Rust | GTK 기반 시스템 트레이, 설정 UI |
| **GUI Qt** | `unim-gui-qt/` | Rust (cxx-qt) | Qt6 기반 시스템 트레이, 설정 UI |
| **GTK3 IM Module** | `unim-frontends/gtk3/` | C | GTK3 입력 모듈 |
| **GTK4 IM Module** | `unim-frontends/gtk4/` | C | GTK4 입력 모듈 |
| **GTK Common** | `unim-frontends/gtk-common/` | C | GTK3/4 공통 코드 (한자 팝업 등) |
| **Qt5 Plugin** | `unim-frontends/qt5/` | C++ | Qt5 입력 플러그인 |
| **Qt6 Plugin** | `unim-frontends/qt6/` | C++ | Qt6 입력 플러그인 |
| **Qt Common** | `unim-frontends/qt-common/` | C++ | Qt5/6 공통 코드 |
| **XIM Frontend** | `unim-frontends/xim/` | Rust | X11 XIM 프로토콜 프론트엔드 |
| **Wayland Frontend** | `unim-frontends/wayland/` | Rust | Wayland 프론트엔드 |
| **GNOME Extension** | `unim-gnome-extension/` | JavaScript | GNOME Shell IM (키 가로채기, 팝업, 인디케이터) |

## 아키텍처 흐름

```text
사용자 키 입력
    ↓
[IM Module / Frontend] ──DBus──→ [unim-daemon] ──→ [Core Engine (src/)]
    ↑                                                      ↓
    └──── DBus Signal (commit/preedit) ←──────────────────┘
```

- **DBus 서비스명**: `org.atit.unim.InputMethod`
- **설정 파일**: `~/.config/unim/config.yaml`
- **로그 파일**: `~/.unim-errors.log` (`UNIM_DEVELOP=1` 시 활성화)

### 팝업 아키텍처

한자/특수문자 팝업은 두 가지 모드로 동작합니다 (`popup_mode` 설정):

| 모드 | 팝업 주체 | 시그널 발행 | 환경 |
| ---- | --------- | ---------- | ---- |
| **Standalone** (기본) | unim-gui-gtk 또는 GNOME Extension | ShowHanja/ShowSpecial DBus 시그널 | 모든 환경 |
| **Embedded** | IM 모듈 자체 (GTK/Qt/XIM/Wayland) | 시그널 미발행 | X11 전용 |

- 엔진이 `PopupAction`으로 팝업 상태를 중앙 관리 (키 네비게이션, 선택, 취소)
- FocusOut/Reset 시 엔진이 팝업을 취소하고 `HidePopup` 시그널 발행
- GNOME+Wayland: GNOME Extension이 모든 프론트엔드의 팝업을 Push 방식으로 표시

### GNOME Extension 키 처리

GNOME Extension은 Clutter Backend에 커스텀 `InputMethod`를 등록하여 Wayland 텍스트 입력을 직접 가로챕니다:

```text
Wayland 키 이벤트 → vfunc_filter_key_event → KeyHandler → DBus ProcessKey
                                                  ↓
                                        키 큐 패턴 (call_sync 재진입 방지)
                                                  ↓
                                    notify_key_event(event, consumed)
```

- `call_sync()` 중 GLib 메인 루프 재진입으로 도착한 키를 `event.copy()`로 큐에 저장, FIFO 순차 처리

## 빌드 시스템

`Makefile`이 소스 오브 트루스입니다.

| 명령 | 설명 |
| ---- | ---- |
| `make build` | 전체 빌드 (Rust + 프론트엔드) |
| `make build-rust` | Rust workspace만 빌드 |
| `make build-frontends` | GTK3/4/Qt5/6 IM 모듈 빌드 |
| `sudo make install PREFIX=/usr` | 시스템 설치 |
| `make deb` | 데비안 패키지 빌드 |
| `make sandbox-gtk4` | Xephyr 샌드박스에서 GTK4 테스트 |
| `cargo test --workspace` | Rust 유닛 테스트 (전체) |
| `make dev-gtk4` | GTK4 모듈 빌드 + 배포 |
| `make dev-daemon` | 데몬 빌드 + 배포 |
| `make dev-extension` | GNOME Extension 배포 (~/.local/share/) |

## 핵심 파일

- **엔진 로직**: `src/input_engine.rs` - 한글/영어 키 처리, 모드 전환, 팝업 키 네비게이션
- **한글 조합**: `src/hangul/` - 2벌식/3벌식 조합 로직
- **키맵**: `src/keystroke/` - 키보드 레이아웃 매핑
- **설정**: `src/config.rs` - 설정 구조체 (Source of Truth)
- **로깅**: `src/logging.rs` - 통합 로깅 매크로
- **엔진 워커**: `unim-dbus/src/engine_worker.rs` - FocusIn/Out, Reset, ProcessKey 요청 처리
- **DBus 서비스**: `unim-dbus/src/service.rs` - DBus 메서드/시그널, 팝업 시그널 발행
- **GNOME 키 핸들러**: `unim-gnome-extension/key_handler.js` - 키 큐 패턴, 재진입 방지
- **GNOME IM**: `unim-gnome-extension/unim_input_method.js` - Clutter InputMethod 서브클래스

## 에이전트 및 스킬 (.claude/)

### 에이전트 정의 (`.claude/agents/`)

| 에이전트 | 타입 | 역할 |
| -------- | ---- | ---- |
| `planner` | Plan (opus) | 코드 탐색 + CLAUDE.md 규칙 기반 구현 계획 수립 |
| `reviewer` | general-purpose (opus) | 빌드(zero-warning) + 테스트(all-pass) + 규칙 준수 검증 |

### 스킬 (`.claude/skills/`)

| 스킬 | 트리거 | 역할 |
| ---- | ------ | ---- |
| `/harness` | 코드 변경 작업 | **기획→구현→평가 루프** 오케스트레이터 (서브 에이전트 모드) |
| `/plan` | 분석/기획 요청 | 독립 기획 (planner 에이전트 단독 실행) |
| `/review` | 검증/리뷰 요청 | 독립 평가 (reviewer 에이전트 단독 실행) |
| `/unim-log` | 로그 분석 | `~/.unim-errors.log` 분석 및 진단 |

### 하네스 워크플로우

```text
/harness <작업 설명>
    ↓
Phase 1: planner 에이전트 → 구현 계획 → 사용자 승인
    ↓
Phase 2: 메인 에이전트 → 직접 코드 수정
    ↓
Phase 3: reviewer 에이전트 → PASS/FAIL 판정
    ↓
FAIL → Phase 2 재실행 (최대 3회) / PASS → 커밋
```

## 디버깅

**버그 분석 시 `~/.unim-errors.log`를 반드시 먼저 확인하세요.**

- `UNIM_DEVELOP=1` 환경변수 설정 시 모든 컴포넌트(Engine, DBus, Frontend, Extension)가 이 파일에 로그를 기록합니다.
- 로그에는 타임스탬프, 모듈명, 상세 메시지가 포함되어 키 이벤트 흐름, DBus 통신, 조합 상태 변화를 추적할 수 있습니다.
- 재현 전 로그 파일을 비우고(`> ~/.unim-errors.log`), 재현 후 분석하면 효율적입니다.

## 참조 문서

- [GEMINI.md](GEMINI.md) - 개발 컨벤션, 설정 연동 가이드, 로깅 시스템
- [IME_BEHAVIOR.md](IME_BEHAVIOR.md) - 한글 입력기 동작 명세 (모든 프론트엔드 공통)
- [docs/POPUP_SPEC.md](docs/POPUP_SPEC.md) - 한자/특수문자 팝업 통합 설계서 (색상, 폰트, 키 바인딩, 프런트엔드별 전략)
- [ROADMAP.md](ROADMAP.md) - 장기 개발 로드맵
- [README.md](README.md) - 프로젝트 소개 및 아키텍처 상세
