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
| **CLI** | `unim-cli/` | Rust | 한↔영 변환 + `config` 서브커맨드 통합 명령줄 도구 |
| **GUI Common** | `unim-gui-common/` | Rust | DBus 통신, 트레이 공통 로직 |
| **GUI GTK** | `unim-gui-gtk/` | Rust | GTK 기반 시스템 트레이, 설정 UI |
| **Popup Service** | `unim-popup-service/` | Rust+GTK4 | 한자·특수문자·이모지 팝업 단일 렌더러 (D-Bus auto-activation) |
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
- **AutoTypeFix 억제 사전** (사용자 데이터): `~/.config/unim/typefix-blacklist.yaml` — 데몬이 mtime 감시로 자동 리로드
- **로그 파일**: `~/.unim-errors.log` (`UNIM_DEVELOP=1` 시 활성화)

### 팝업 아키텍처 (0.3.0 — 단일 SoT)

0.3.0부터 팝업은 **`unim-popup-service`** 사이드카가 단일 렌더러로 처리합니다.

```text
[unim-daemon] ──PopupRender payload──→ [unim-popup-service] ──GTK4 window──→ 화면
                                              또는
                                       [GNOME extension popup_view.js] ──St 위젯──→ 화면
```

| 환경 | 렌더러 | 조건 |
| ---- | ------ | ---- |
| GNOME Wayland | extension `popup_view.js` (St 위젯) | `Meta.is_wayland_compositor() == true` |
| GNOME X11 / KDE / Xfce / X11 WM | `unim-popup-service` GTK4 윈도우 | D-Bus auto-activation |
| Wayland WM (Sway/Hyprland) | `unim-popup-service` GTK4 (wayland-backend) | `libgtk4-layer-shell` 필요 |

**핵심 원칙**:

- daemon은 시그널 8종을 `org.atit.unim.Popup` 인터페이스로 forward만 한다
- `PopupRender` payload가 단일 view-model SoT — 셀·헤더·푸터·탭·하이라이트 모두 포함
- 렌더러는 `PopupRender`만 소비하고, 자체 상태를 관리하지 않는다
- 외부 좌클릭 dismiss: 클릭 이벤트는 아래 창에 pass-through
- 팝업 dismiss 단일 경로: `focus_out` / `reset` (cursor-jump 감지 보완)

엔진이 `PopupAction`으로 팝업 상태를 중앙 관리 (키 네비게이션, 선택, 취소).
FocusOut/Reset 시 엔진이 팝업을 취소하고 `HidePopup` 시그널 발행.

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

## 품질 규칙 (Zero Tolerance)

머지 기준은 다음 세 가지가 모두 충족될 때:

- `cargo build --workspace`는 **경고 0개**로 완료되어야 한다
- `cargo test --workspace`는 **모든 테스트 통과**해야 한다
- `make build` (C/C++ 프론트엔드 포함)도 경고 없이 완료되어야 한다
- 코드 변경 후 항상 빌드+테스트를 실행하고, 신규 경고는 즉시 제거한다

## 메모리 관리 규칙 (Zero Tolerance)

`unim-daemon`은 세션이 끝날 때까지 계속 실행되는 장수(long-lived) 프로세스다.
과거 RSS가 2GB까지 부푼 사건(glibc ptmalloc arena 폭발 + context 맵 누수) 이후
아래 항목은 **회귀 금지**다. Rust의 ownership이 `free()`를 호출해도 할당자가 OS에
메모리를 반환하지 않으면 RSS는 내려가지 않는다는 점을 항상 염두에 둘 것.

### 할당자

- [unim-daemon/src/main.rs](../../../unim-daemon/src/main.rs) 의 `#[global_allocator] tikv_jemallocator::Jemalloc` 지정은 **제거·교체 금지**
- [scripts/unim-daemon.service](../../../scripts/unim-daemon.service) 의 `Environment=MALLOC_ARENA_MAX=2` 도 유지 (C 라이브러리 경로 이중 차단)
- `main()` 안의 60초 주기 `libc::malloc_trim(0)` 태스크 유지

### per-context HashMap 수명

`engine_worker.rs`의 `DestroyContext` 핸들러는 context_id에 묶인 **모든** 맵을 함께 정리해야 한다. 한 개라도 빠뜨리면 IBus Portal 경로(context_id가 단조 증가)에서 무제한 누적된다. 현재 대상:

- `contexts`, `context_windows`, `keystroke_buffers`, `undo_states`, `recent_corrections`
- `last_focused_context_id == Some(id)` 이면 `None`으로 리셋

새로운 per-context 상태를 추가하면 **반드시** 이 핸들러에도 `remove(&id)` 라인을 추가한다. CI·리뷰에서 체크포인트로 간주.

### zbus `object_server` 수명

`connection.object_server().at(path, handler).await` 로 등록한 모든 핸들러는 대응하는 `remove::<T, _>(path).await` 경로가 존재해야 한다. 현재 대상:

- [unim-dbus/src/service.rs](../../../unim-dbus/src/service.rs) `InputContextHandler::destroy`
- [unim-dbus/src/ibus_compat/ibus_context.rs](../../../unim-dbus/src/ibus_compat/ibus_context.rs) `IBusInputContextHandler::destroy`

핸들러가 남으면 zbus 내부 라우팅 테이블이 세션 수명 동안 계속 커진다.

### 진단 명령 (의심 시)

```bash
# RSS / Thread / anonymous mmap
grep -E 'VmRSS|VmData|Threads' /proc/$(pidof unim-daemon)/status
cat /proc/$(pidof unim-daemon)/smaps_rollup | grep -E 'Rss|Anonymous'
# 64MB 이상 익명 arena 개수 (정상은 0~2개, 10개 이상이면 할당자 회귀 의심)
python3 -c "import re; c=0
for L in open(f'/proc/{__import__(\"os\").popen(\"pidof unim-daemon\").read().strip()}/maps'):
    m=re.match(r'([0-9a-f]+)-([0-9a-f]+)\s+rw',L)
    if m and (int(m.group(2),16)-int(m.group(1),16))>=64*1024*1024: c+=1
print(f'big_anon_arenas={c}')"
```

## 개발 규약

- **`Makefile`**이 빌드/설치 프로세스의 소스 오브 트루스다
- Core 로직은 `src/`에 **엄격히 격리** — UI·플랫폼 의존성 금지
- 프론트엔드는 DBus(`unim-daemon` 경유)로만 엔진과 통신. 직접 메모리 공유 금지
- C/C++에서 Core 접근은 반드시 `unim-capi/` FFI 레이어 경유
- **문서·기획·walkthrough는 한국어**, **Git commit 메시지는 영어** (예: `feat: Add Wayland popup support`)
- 로깅은 `unim_log!` (Rust) / `unim_log_message()` (C/C++) / `unimLog()`·`unimError()` (JS) 사용. `println!`, `log::*`, `console.log` 금지. 자세한 규칙·포맷은 [GEMINI.md](GEMINI.md) 참조
- 설정 변경 시 **5지점 동기화** 규칙 준수 — [GEMINI.md의 Settings Synchronization](GEMINI.md) 참조

### CHANGELOG 작성 규칙

(출처: ~/work/spin 의 규칙을 2026-09-06 기현님 지시로 그대로 채택)

`CHANGELOG-ko.md`(한국어, 정본)·`CHANGELOG.md`(영어)는 릴리스 노트의 유일한 원본이다 — `scripts/release-body.sh` 가 해당 버전 절을 그대로 떼어 GitHub Release 본문으로 올린다(따로 릴리스 노트를 쓰지 않는다). 사용자가 직접 읽는 문서이므로 항목을 쓸 때마다 다음을 지킨다.

- **한 항목 = 한 줄.** 이어지는 설명 줄도, 하위 글머리(`  - `)도 두지 않는다.
- **명사형 어미로 끝맺는다.** "~습니다/합니다/됩니다" 로 끝나는 문장을 쓰지 않는다. "지원"·"추가"·"개선"·"수정"·"변경"·"제거"·"이동" 같은 명사로 끝낸다.
  예: "다크모드 지원", "로그인 오류 수정", "한자 입력 검색엔진 노출 지원".
- **철저히 사용자 관점.** 왜 그렇게 만들었는지, 내부 구현이 어땠는지, 누가 무슨 지시를 했는지, 어느 테스트를 고쳤는지, 어느 계획 문서 몇 절인지는 적지 않는다. **무엇이 달라졌는지** 만 남긴다. 사용자가 마주치는 화면·버튼 이름은 대괄호로 표기한다(`[한글]`, `[설정]` 처럼).
- **핵심만.** 세부 사례를 나열하지 않는다. 한 덩어리 변경(예: 팝업 UI를 전면 재작성)은 한 줄로 뭉친다 — 서로 다른 변경이라 뜻이 갈릴 때만 줄을 나눈다.
- **굵게(`**...**`)·강조 기호(★) 를 쓰지 않는다.** 문장이 이미 짧아 강조가 필요 없다.
- 형식은 그대로 유지한다: `## [x.y.z] YYYY-MM-DD`(미배포는 `## [Unreleased]`, 날짜 없음) → `### 소제목`(Keep a Changelog 범주 — 추가됨·변경됨·제거됨·고쳐짐 등) → `- 한 줄` 항목.
  `release-body.sh` 가 이 형식을 정규식으로 파싱하므로 헤딩 한 글자도 어긋나면 안 된다. 한 버전 안에서 같은 소제목을 두 번 쓰지 않는다 — 추가됨·변경됨·제거됨·고쳐짐이 여럿 필요하면 그때그때 하나로 합친다.
- 파일 맨 끝의 참조 링크 각주(`[Keep a Changelog (korean)]: …` 류)는 항상 모든 버전 절 **바깥**에 둔다. 새 각주를 추가할 이유가 생기면 이 각주 블록 하나에 더한다.

새 버전을 기록할 때마다 이 규칙을 따른다. 옛 항목의 문체를 참고해 옮겨 적지 않는다 — 옛 것이 이 규칙을 어기고 있으면 그건 아직 못 고친 것이지 본받을 스타일이 아니다.

#### 한국어와 영어를 함께 간다

`CHANGELOG-ko.md`(한국어, 정본) 옆의 `CHANGELOG.md`(영어)는 같은 구조(버전 헤딩·소제목·항목 수)를 유지하는 **번역본**이지 별개 문서가 아니다 — `release-body.sh` 가 둘을 나란히 릴리스 본문에 싣는다.

- **새 버전을 기록할 때 모든 언어 파일을 같은 커밋에서 함께 채운다.** 한국어만 쓰고 영어를 나중으로 미루지 않는다 — 미루는 순간 다른 언어 사용자 눈에는 "달라진 게 없다"로 보인다.
- 번역은 그 언어의 **명사형 어미 관례**를 따른다 — 한국어를 그대로 옮기지 않는다.
  - 영어: Keep a Changelog 의 표준 소제목(Added/Changed/Removed/Fixed 등)을 쓰고, 항목은 "Added dark mode", "Fixed login error" 처럼 짧은 명사구/과거분사로 끝낸다.
- 화면·버튼 이름(대괄호 표기)은 **그 언어의 실제 UI 문자열**을 쓴다 — 한국어 이름을 그대로 옮기지 않는다.
- 모든 언어 파일의 버전 헤딩(`## [x.y.z] YYYY-MM-DD`)과 각 절의 항목 **개수**는 서로 같아야 한다 — 한쪽에만 있는 사실은 없다. 개수가 다르면 번역이 빠졌거나 한쪽에 정보가 새로 끼어든 것이다.

## 핵심 파일

- **엔진 로직**: `src/input_engine.rs` - 한글/영어 키 처리, 모드 전환, 팝업 키 네비게이션
- **한글 조합**: `src/hangul/` - 2벌식/3벌식 조합 로직
- **키맵**: `src/keystroke/` - 키보드 레이아웃 매핑
- **설정**: `src/config.rs` - 설정 구조체 (Source of Truth)
- **자동 오타 교정**: `src/auto_typefix.rs` - forward/reverse 교정기
- **AutoTypeFix 억제 사전**: `src/typefix_blacklist.rs` - Tentative/Confirmed/Inactive 3상태 블랙리스트, 재시도 기반 자동 학습, mtime 핫리로드
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

### 모델 사용 지침 (기현님 지시, 2026-09-06)

에이전트·워크플로에 모델을 배정할 때 아래 표를 따른다. 토큰과 시간은 예산이다 —
작업 난이도에 맞는 가장 싼 모델을 고르고, 판단·검증의 최종 책임만 위로 올린다.

| 모델 | 맡기는 일 |
|---|---|
| **fable** | 계획 수립, 난이도 판단(어느 모델에 맡길지 결정), 어려운 검증 |
| **opus** | 쉬운 검증, 어려운 구현 |
| **sonnet** | 쉬운 구현, 어려운 검색·조사 |
| **haiku** | 쉬운 검색·조사 |

- "어렵다"의 기준: 설계 판단이 갈리거나, 여러 컴포넌트에 걸치거나, 실측 없이는 결과를
  못 믿는 작업. 그 외는 "쉽다".
- 메인 세션(fable)은 계획과 난이도 판단을 하고 실행은 위임한다. 위임한 검색을 직접
  반복하지 않는다.
- 워크플로 스크립트의 `agent(..., {model})` 과 `.claude/agents/*.md` 의 `model:` 은 이 표에
  맞춘다. 2026-06-02 의 "전 단계 opus" 지시는 이 표로 대체됐다.

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
- [POPUP_SPEC.md](../specs/POPUP_SPEC.md) - 한자/특수문자 팝업 통합 설계서 (색상, 폰트, 키 바인딩, 프런트엔드별 전략) — 단일 원본
- [ROADMAP.md](../../../ROADMAP.md) - 장기 개발 로드맵
- [README.md](../../../README.md) - 프로젝트 소개 및 아키텍처 상세
