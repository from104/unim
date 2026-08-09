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

UNIM은 고성능과 확장을 위해 **3계층 구조 + popup-service 사이드카**를 사용합니다. 코드를 수정할 때는 다음 설계 철학을 준수해야 합니다.

1. **엔진의 완전한 고립 (src/)**
   - 한글의 조합, 분해 로직 및 엔진 핵심 데이터는 모두 `src/` (Rust 라이브러리) 내부에 위치합니다. UI 종속성, 플랫폼 전용 API 등은 절대로 여기에 위치할 수 없습니다.
2. **DBus 중앙 집중형 통신**
   - 애플리케이션 프론트엔드(GTK, Qt, Wayland, XIM)와 엔진은 직접적인 메모리 공유 없이 `unim-daemon` 프로세스를 거쳐 DBus(`org.atit.unim.InputMethod`)로 통신합니다.
3. **팝업 단일 SoT — popup-service (0.3.0+)**
   - 한자·특수문자·이모지 팝업의 view-model 생성은 daemon이, 렌더링은 `unim-popup-service`(GTK4) 또는 GNOME extension(`popup_view.js`)이 담당합니다.
   - 팝업 관련 시그널/메서드를 daemon에 직접 추가하지 마세요. `org.atit.unim.Popup` 인터페이스를 통해 forward합니다.
   - `PopupRender` payload가 단일 view-model SoT입니다. 렌더러는 이 payload만 소비합니다.
4. **안전한 C-API 래핑 (unim-capi/)**
   - Rust 코어를 C/C++ 모듈 등 외부에서 직접 가져와야 할 경우에는 FFI 기반의 `unim-capi` 래퍼 계층만을 이용해야 합니다.

## ⚙️ 설정 항목 추가/변경 가이드라인

단일 환경 설정(`src/config.rs`)은 다양한 시스템과 UI에 공유됩니다. 설정 값을 변경하거나 새로 추가할 때는 **모든 관련 컴포넌트를 동기화**해야 합니다.

| 컴포넌트 | 파일 위치 | 동기화 사항 |
| -------- | --------- | ----------- |
| **설정 코어** | `src/config.rs` | 설정 구조체 추가 및 직렬화 정의 (Source of Truth) |
| **CLI 설정 도구** | `unim-cli/src/main.rs` (`config` 서브커맨드) | `ConfigKey` enum, setter dispatch, `locales/*.yml` 반영 |
| **DBus 서비스** | `unim-dbus/src/service.rs` | `get_config`/`set_config` 등 메서드 업데이트 |
| **GUI 설정 창** | `unim-gui-gtk/src/settings_dialog.rs` | UI 슬라이더·스위치 위젯 바인딩 |
| **GNOME Extension** | `unim-gnome-extension/prefs.js` | GNOME Shell 전용 설정 항목에만 추가 |

새로운 설정 항목을 추가할 때는 위 체크리스트를 활용하세요. **5지점 중 하나라도 누락되면 설정이 동기화되지 않습니다.**

> GNOME Shell 의존 키(예: indicator 토글)만 `prefs.js` + `*.gschema.xml`도 함께 업데이트합니다. 그 외 일반 설정은 gschema에 추가하지 마세요.

## 🌿 브랜치 및 PR 워크플로

| 브랜치 | 역할 |
| ------ | ---- |
| `main` | **안정 릴리스 라인** — 태그된 릴리스(`v0.1.0` 등) 시점만 반영. 일반 기여는 직접 받지 않습니다. |
| `develop` | **활성 통합 라인** — 모든 기능·수정이 PR을 통해 합쳐지는 기본 작업 브랜치. |
| `feature/*`·`fix/*`·`claude/*` | 작업 브랜치. `develop`를 base로 분기하고, 완료 시 `develop`로 PR을 보냅니다. |

### 기여 흐름

1. `develop`에서 작업 브랜치 분기.
2. 변경 완료 후 **`develop`** 대상으로 Pull Request 생성. (GitHub 기본 base는 `main`이지만, 일반 기여 PR은 base를 `develop`로 변경해 주세요.)
3. 리뷰·CI 통과 후 머지.
4. 릴리스 시점에만 메인테이너가 `develop` → `main` 머지 + 태깅을 수행합니다.

## 📝 문서화

1. **기능 명세 (`SPEC.md`) 갱신**
   - 모듈의 아키텍처나 주요 작동 방식이 바뀌면, 해당 모듈 디렉토리(예: `unim-frontends/gtk4/`)의 `SPEC.md`를 즉시 반영해야 합니다.
2. **언어**
   - 개발 계획(Implementation Plan), 작업 목록, Walkthrough 등의 기여 문서는 **한글** 작성을 기본으로 합니다.
   - 단, `git commit` 메시지는 `git-sync` 관례에 따라 **영문**으로 작성하는 것을 권장합니다 (예: `feat: Add Wayland popup support`).

## 🤖 6매니저 하네스 (Claude Code 에이전트)

이 프로젝트는 Claude Code 에이전트 6인 팀으로 작업을 분담합니다. `.claude/agents/` 디렉토리에 각 에이전트 정의가 있습니다.

| 에이전트 | 역할 | 파일 |
|---------|------|------|
| **pm** | 작업 라우팅·계획·종합 | `pm.md` |
| **source-manager** | 파일 구조·git·패키징·CHANGELOG | `source-manager.md` |
| **engine-frontend-manager** | Rust 엔진·프론트엔드 구현 | `engine-frontend-manager.md` |
| **ui-manager** | GTK/Qt UI·설정 다이얼로그 | `ui-manager.md` |
| **doc-promo-manager** | 문서·릴리즈 노트·홍보 | `doc-promo-manager.md` |
| **user-rep-reviewer** | 사용자 관점 리뷰·UX 검증 | `user-rep-reviewer.md` |

**6지점 sync 체크리스트** (신기능·팝업 관련 변경 시):

- [ ] `src/config.rs` — 설정 구조체
- [ ] `unim-cli/src/main.rs` — ConfigKey enum
- [ ] `unim-cli/locales/{ko,en}.yml` — CLI 라벨
- [ ] `unim-dbus/src/service.rs` — DBus 디스패치
- [ ] `unim-gui-gtk/src/settings_dialog.rs` — GUI 위젯
- [ ] `docs/dev/specs/POPUP_SPEC.md` — 팝업 명세 (팝업 변경 시)
- [ ] `unim-gnome-extension/popup_view.js` — GNOME 렌더러 (GNOME 팝업 변경 시)
- [ ] `unim-popup-service/src/` — popup-service 렌더러 (팝업 변경 시)

## 🛠️ 개발 환경 설정 및 빌드

소스 코드를 클론한 후 다음 명령어로 프로젝트를 빌드하고 테스트할 수 있습니다.

```bash
# 전체 빌드 (Rust 엔진 + C/C++ 프론트엔드)
make build

# Rust 유닛 테스트
cargo test --workspace

# Debian 패키지 빌드
make deb

# RPM 패키지 빌드
make rpm

# 포그라운드 데몬 테스트 실행
UNIM_DEVELOP=1 target/debug/unim-daemon -n

# popup-service 디버그 실행
UNIM_DEVELOP=1 target/debug/unim-popup-service
```

> **버그 디버깅 팁:**
> 환경변수 `UNIM_DEVELOP=1`을 설정하면, 시스템 내 모든 모듈(프론트엔드, 데몬, CLI, popup-service)에서 발생하는 상세 에러 로그가 `~/.unim-errors.log`에 통합 기록됩니다.
>
> popup-service DBus 연결 확인:
>
> ```bash
> busctl --user introspect org.atit.unim.PopupService /org/atit/unim/Popup
> ```

저희의 목표와 철학에 공감해주셔서 감사합니다. 여러분의 멋진 PR(Pull Request)을 기다립니다!
