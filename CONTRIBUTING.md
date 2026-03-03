# 기여자 가이드 (Contributing to UNIM)

UNIM(Universal Next-generation Input Method) 프로젝트에 기여해 주셔서 감사합니다! 이 프로젝트는 오픈 소스 기반의 강력하고 이식 가능한 한국어 입력기 엔진을 구축하는 것을 목표로 합니다.

여러분의 코드가 원활하게 병합될 수 있도록 아래 가이드라인을 준수해 주시길 부탁드립니다.

## 🚨 무관용 원칙 (Zero Tolerance)

UNIM은 커널 수준의 안정성을 지향하는 핵심 시스템 소프트웨어입니다. 따라서 다음 품질 기준을 엄격하게 적용합니다.

1. **경고(Warning) 0개 유지**
   - `cargo build --workspace` 실행 시 단 1개의 경고도 허용하지 않습니다.
   - `make build` 실행(C/C++ 프론트엔드 포함) 역시 경고 없이 완료되어야 합니다.
2. **모든 테스트 통과**
   - `cargo test --workspace` 실행 시 모든 유닛/통합 테스트가 통과해야 합니다.
   - 어떠한 이유로든 테스트 실패를 방치해서는 안 됩니다.
3. **기존 이슈 방치 금지**
   - 코드 수정 시 기존 코드에서 발생한 새로운 경고나 오류가 발견되면 "내가 한 것이 아니니까" 넘기지 말고 즉시 수정해야 합니다.

## 🏗️ 아키텍처 규칙

UNIM은 고성능과 확장을 위해 **3계층 구조**를 사용합니다. 코드를 수정할 때는 다음 설계 철학을 준수해야 합니다.

1. **엔진의 완전한 고립 (src/)**
   - 한글의 조합, 분해 로직 및 엔진 핵심 데이터는 모두 `src/` (Rust 라이브러리) 내부에 위치합니다. UI 종속성, 플랫폼 전용 API 등은 절대로 여기에 위치할 수 없습니다.
2. **DBus 중앙 집중형 통신**
   - 애플리케이션 프론트엔드(GTK, Qt, Wayland, XIM)와 엔진은 직접적인 메모리 공유 없이 `unim-daemon` 프로세스를 거쳐 DBus(`org.atit.unim.InputMethod`)로 통신합니다.
3. **안전한 C-API 래핑 (unim-capi/)**
   - Rust 코어를 C/C++ 모듈 등 외부에서 직접 가져와야 할 경우에는 FFI 기반의 `unim-capi` 래퍼 계층만을 이용해야 합니다.

## ⚙️ 설정 항목 추가/변경 가이드라인

단일 환경 설정(`src/config.rs`)은 다양한 시스템과 UI에 공유됩니다. 설정 값을 변경하거나 새로 추가할 때는 **모든 관련 컴포넌트를 동기화**해야 합니다.

| 컴포넌트 | 파일 위치 | 동기화 사항 |
| -------- | --------- | ----------- |
| **설정 코어** | `src/config.rs` | 설정 구조체 추가 및 직렬화 정의 (Source of Truth) |
| **CLI 설정 도구** | `unim-config/src/main.rs` | 명령줄 인자, 도움말 문자열 반영 |
| **DBus 서비스** | `unim-dbus/src/service.rs` | `get_config`/`set_config` 등 메서드 업데이트 |
| **GUI 설정 창** | `unim-gui/src/settings_dialog.rs` | UI 체크박스, 스피너 등 위젯 바인딩 |
| **GNOME Extension** | `unim-gnome-extension/prefs.js` | GNOME 기본 Preferences 페이지에 반영 |

새로운 설정 항목을 추가할 때는 `.agent/skills/add-setting/SKILL.md`를 참고하거나 위 목록을 체크리스트로 활용하세요.

## 📝 문서화

1. **기능 명세 (`SPEC.md`) 갱신**
   - 모듈의 아키텍처나 주요 작동 방식이 바뀌면, 해당 모듈 디렉토리(예: `unim-frontends/gtk4/`)의 `SPEC.md`를 즉시 반영해야 합니다.
2. **언어**
   - 개발 계획(Implementation Plan), 작업 목록, Walkthrough 등의 기여 문서는 **한글** 작성을 기본으로 합니다.
   - 단, `git commit` 메시지는 `git-sync` 관례에 따라 **영문**으로 작성하는 것을 권장합니다 (예: `feat: Add Wayland popup support`).

## 🛠️ 개발 환경 설정 및 빌드

소스 코드를 클론한 후 다음 명령어로 프로젝트를 빌드하고 테스트할 수 있습니다.

```bash
# 전체 빌드 (Rust 엔진 + C/C++ 프론트엔드)
make build

# Rust 유닛 테스트
cargo test --workspace

# 포그라운드 데몬 테스트 실행
UNIM_DEVELOP=1 target/debug/unim-daemon -n
```

> **버그 디버깅 팁:**
> 환경변수 `UNIM_DEVELOP=1`을 설정하면, 시스템 내 모든 모듈(프론트엔드, 데몬, CLI)에서 발생하는 상세 에러 로그가 `~/.unim-errors.log`에 통합 기록되어 쉽게 원인을 추적할 수 있습니다.

저희의 목표와 철학에 공감해주셔서 감사합니다. 여러분의 멋진 PR(Pull Request)을 기다립니다!
