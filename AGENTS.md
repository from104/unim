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
| **GUI** | `unim-gui/` | Rust | 시스템 트레이, 한자/특수문자 팝업, 설정 |
| **GTK3 IM Module** | `unim-frontends/gtk3/` | C | GTK3 입력 모듈 |
| **GTK4 IM Module** | `unim-frontends/gtk4/` | C | GTK4 입력 모듈 |
| **GTK Common** | `unim-frontends/gtk-common/` | C | GTK3/4 공통 코드 (한자 팝업 등) |
| **Qt5 Plugin** | `unim-frontends/qt5/` | C++ | Qt5 입력 플러그인 |
| **Qt6 Plugin** | `unim-frontends/qt6/` | C++ | Qt6 입력 플러그인 |
| **Qt Common** | `unim-frontends/qt-common/` | C++ | Qt5/6 공통 코드 |
| **XIM Frontend** | `unim-frontends/xim/` | Rust | X11 XIM 프로토콜 프론트엔드 |
| **Wayland Frontend** | `unim-frontends/wayland/` | Rust | Wayland 프론트엔드 |
| **GNOME Extension** | `unim-gnome-extension/` | JavaScript | GNOME Shell 확장 (인디케이터, 설정) |

## 아키텍처 흐름

```text
사용자 키 입력
    ↓
[IM Module / Frontend] ──DBus──→ [unim-daemon] ──→ [Core Engine (src/)]
    ↑                                                      ↓
    └──── DBus Signal (commit/preedit) ←──────────────────┘
```

- **DBus 서비스명**: `org.atit.unim.InputMethod`
- **설정 파일**: `~/.config/unim/config.json`
- **로그 파일**: `~/.unim-errors.log` (`UNIM_DEVELOP=1` 시 활성화)

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
| `cargo test` | Rust 유닛 테스트 |

## 핵심 파일

- **엔진 로직**: `src/input_engine.rs` - 한글/영어 키 처리, 모드 전환
- **한글 조합**: `src/hangul/` - 2벌식/3벌식 조합 로직
- **키맵**: `src/keystroke/` - 키보드 레이아웃 매핑
- **설정**: `src/config.rs` - 설정 구조체 (Source of Truth)
- **로깅**: `src/logging.rs` - 통합 로깅 매크로

## 워크플로우 및 스킬 (.agent/)

| 명령어/스킬 | 역할 |
| ----------- | ---- |
| `/build` | UNIM 전체 빌드 (Rust + 프론트엔드) |
| `/install` | UNIM 시스템 설치 및 제거 |
| `/test` | UNIM 테스트 실행 (Rust 유닛 테스트 + 설치 상태 확인) |
| `/sync` | **영문 요약 기반 Git 커밋 및 GitHub 동기화** (`git-sync` 스킬 사용) |
| `add-setting` | 새 설정 항목 추가 시 모든 컴포넌트 연동 가이드 |
| `git-sync` | 변경 사항 분석 및 영문 커밋 메시지 생성 스킬 |

## 디버깅

**버그 분석 시 `~/.unim-errors.log`를 반드시 먼저 확인하세요.**

- `UNIM_DEVELOP=1` 환경변수 설정 시 모든 컴포넌트(Engine, DBus, Frontend, Extension)가 이 파일에 로그를 기록합니다.
- 로그에는 타임스탬프, 모듈명, 상세 메시지가 포함되어 키 이벤트 흐름, DBus 통신, 조합 상태 변화를 추적할 수 있습니다.
- 재현 전 로그 파일을 비우고(`> ~/.unim-errors.log`), 재현 후 분석하면 효율적입니다.

## 참조 문서

- [GEMINI.md](GEMINI.md) - 개발 컨벤션, 설정 연동 가이드, 로깅 시스템
- [IME_BEHAVIOR.md](IME_BEHAVIOR.md) - 한글 입력기 동작 명세 (모든 프론트엔드 공통)
- [ROADMAP.md](ROADMAP.md) - 장기 개발 로드맵
- [README.md](README.md) - 프로젝트 소개 및 아키텍처 상세
