# UNIM 테스트 앱 — 설계

> 2026-08-08 전면 재설계. 이 문서가 `tests/` 의 단일 기준(SoT)이다.

## 0. 왜 다시 만드나

2026-08-07 에 XIM ON-THE-SPOT preedit 누락 버그를 잡으면서 드러난 것:

- **화면의 진실을 보는 앱이 하나도 없었다.** 모든 테스트 앱의 "Preedit:" 라벨은
  DBus `PreeditChanged` 시그널을 그대로 옮겨 적은 것이다. 데몬이 옳게 보냈는데
  **툴킷이 화면에 안 그린** 경우 — 정확히 그 버그 — 를 전부 통과시킨다.
- **`--auto` 가 프런트엔드를 안 탄다.** `unim_test.c` 의 자동 모드는 데몬의
  `ProcessKeyEvent` 를 직접 호출한다. GTK IM 모듈도, XIM 서버도, Wayland
  text-input 도 거치지 않는다. 회귀를 못 잡는다.
- **UI 가 앱마다 다르다.** XIM 은 필드 4개(이름·주소·전화번호·메모), GTK3 는
  위젯 6종, GTK4 는 순서가 다르고, Qt5/Qt6 는 486 줄짜리 같은 파일이 두 벌,
  GNOME 은 진단 패널이 더 붙어 있다. 같은 키를 넣어도 앱마다 다른 것을 본다.
- **로그가 제각각이다.** 형식·타임스탬프 정밀도·출력 대상이 전부 달라 앱 간
  대조가 불가능하다.

## 1. 설계 원칙

| # | 원칙 | 귀결 |
|---|---|---|
| P1 | **화면이 최종 판정자다** | 모든 앱은 툴킷이 실제로 렌더링하는 문자열을 로그에 남긴다 |
| P2 | **자동시험은 실제 키 경로로** | XTEST(`xdotool`) 로 주입 → 툴킷 → IM 모듈 → 데몬 → 되돌아오는 전 구간 |
| P3 | **UI 는 한 곳에서 정의한다** | `unim_test_spec.h` 한 벌, 6개 앱이 같은 화면을 만든다 |
| P4 | **로그는 과할수록 좋다** | 관측 가능한 모든 사건을 JSONL 로. 침묵은 버그다 |
| P5 | **사람 손이 필요 없다** | 전 시나리오가 무인 실행되고 실패 시 스크린샷을 남긴다 |

## 2. 핵심 결정 — 코어 필드는 직접 그린다

툴킷 기본 위젯(`GtkEntry`, `QLineEdit`)은 내부 IM 컨텍스트를 숨긴다. 앱에서
preedit 을 볼 수 없으니 P1 을 만족할 수 없다.

그래서 **코어 필드는 각 툴킷의 캔버스 위젯 + IM 컨텍스트 직결**로 만든다.

| 툴킷 | 캔버스 | IM 결합 |
|---|---|---|
| GTK3 | `GtkDrawingArea` | `GtkIMMulticontext` — `preedit-start/changed/end`, `commit` 직접 수신 |
| GTK4 | `GtkDrawingArea` + `GtkIMContext` | 같음 (`GtkEventControllerKey` 로 위임) |
| Qt5/Qt6 | `QWidget` | `inputMethodEvent()` · `inputMethodQuery()` 오버라이드 |
| XIM | Xlib `Window` | 이미 직접 구현 (`PREEDIT_CALLBACKS`) |
| Wayland | wgpu/서피스 | `zwp_text_input_v3` 직접 |
| GNOME | GTK4 와 동일 | 확장 경로 진단만 추가 |

효과:
- preedit·commit·커서·attribute 를 **100% 관측**한다
- 6개 앱이 **픽셀 수준으로 같은 화면**을 그린다 (스펙 테이블 공유)
- 클릭 위치 → 캐럿 이동까지 우리가 통제하므로 클릭-커밋 회귀를 재현할 수 있다

툴킷 네이티브 위젯 섹션은 **없애지 않는다.** 실제 앱과 같은 경로를 지키는
관측점이라, 코어 필드 아래 별도 섹션으로 병행한다(관측은 얕지만 회귀는 잡는다).

## 3. 화면 스펙

창 `760 × 960` 고정. 위에서 아래로 4개 섹션.

```
┌─ ① 상태 패널 ────────────────────────────────┐
│ DBus       ✅ 연결됨 (org.atit.unim.InputMethod) │
│ 프런트엔드  gtk3   경로 GTK_IM_MODULE=unim        │
│ 엔진 모드   🇰🇷 한글    레이아웃 3bul390          │
│ 포커스     core.plain                          │
│ preedit    "ㄹ" (1자 / 3바이트 / 커서 1)         │
│ 최근 commit "ㄹㄹ"                              │
├─ ② 코어 필드 (직접 그리기 · IM 직결) ──────────┤
│ 일반      [                              ]   │
│ 일반 2    [                              ]   │
│ 숫자      [                              ]   │
│ 비밀번호   [                              ]   │
│ 검색      [                              ]   │
│ 여러 줄   [                              ]   │
├─ ③ 네이티브 위젯 (툴킷 기본) ─────────────────┤
│ Entry     [                              ]   │
│ Password  [                              ]   │
│ Multiline [                              ]   │
├─ ④ 로그 (실시간) ────────────────────────────┤
│ 12:34:56.789 +12 preedit.changed core.plain "ㄹ" │
└──────────────────────────────────────────────┘
```

코어 필드 6종은 ID 가 고정이다. 하네스 시나리오가 이 ID 로 필드를 지목한다.

| ID | 라벨 | 힌트 | 검증 목적 |
|---|---|---|---|
| `core.plain` | 일반 | 없음 | 기본 조합·확정 |
| `core.plain2` | 일반 2 | 없음 | 포커스 전환·필드 간 클릭 커밋 |
| `core.numeric` | 숫자 | `NUMBER` | 숫자 힌트에서의 IM 동작 |
| `core.password` | 비밀번호 | `PASSWORD` | AutoTypeFix·한자 팝업 억제 |
| `core.search` | 검색 | `SEARCH` | 검색 힌트 경로 |
| `core.multiline` | 여러 줄 | `MULTILINE` | 줄바꿈·멀티라인 캐럿 |

## 4. 로그 — `tests/common/unim_test_log.h`

한 줄 = 한 사건. JSON Lines.

```json
{"seq":42,"t":1754612345678,"dt":12,"app":"gtk3","ev":"preedit.changed",
 "field":"core.plain","text":"ㄹ","chars":1,"bytes":3,"cursor":1}
```

공통 키: `seq`(1부터) · `t`(epoch ms) · `dt`(직전 사건 대비 ms) · `app` · `ev`.

### 사건 목록 (전부 필수 — 침묵 금지)

| 분류 | `ev` | 추가 키 |
|---|---|---|
| 수명 | `app.start` `app.ready` `app.exit` | `argv` `pid` |
| 환경 | `env` | `GTK_IM_MODULE` `QT_IM_MODULE` `XMODIFIERS` `GDK_BACKEND` `XDG_SESSION_TYPE` `WAYLAND_DISPLAY` `DISPLAY` `toolkit_version` |
| DBus | `dbus.connect` `dbus.call` `dbus.signal` `dbus.error` | `iface` `member` `args` `elapsed_ms` |
| 포커스 | `focus.in` `focus.out` | `field` `prev` |
| 키 | `key.press` `key.release` | `keyval` `keysym` `hw_keycode` `state` `string` `filtered` |
| IM | `im.filter.enter` `im.filter.leave` | `field` `result` `elapsed_ms` |
| 조합 | `preedit.start` `preedit.changed` `preedit.end` | `text` `chars` `bytes` `cursor` `attrs` |
| 확정 | `commit` | `text` `chars` `bytes` |
| 주변문맥 | `surrounding.retrieve` `surrounding.delete` | `text` `cursor` `offset` `n_chars` |
| 리셋 | `reset` | `field` `reason` |
| 클릭 | `click` | `x` `y` `field` `caret_before` `caret_after` |
| **화면** | **`field.render`** | **`field` `committed` `preedit` `caret` `rendered`** |
| 진단 | `note` `warn` `error` | `msg` |

`field.render` 가 **P1 의 관측점**이다. 필드가 다시 그려질 때마다 화면에 실제로
나타나는 최종 문자열(`rendered` = 확정 텍스트에 preedit 을 캐럿 위치에 끼운 것)
을 남긴다. 하네스는 이 값으로 판정한다.

### 출력

| 환경변수 | 기본 | 뜻 |
|---|---|---|
| `UNIM_TEST_LOG` | (없음) | JSONL 파일 경로. 미지정 시 파일 출력 없음 |
| `UNIM_TEST_LOG_FORMAT` | `both` | `json` \| `human` \| `both` — stdout 형식 |
| `UNIM_TEST_LOG_LEVEL` | `all` | `all` \| `no-key` — 키 사건 제외 |

stdout 은 항상 **줄 버퍼링**(`setvbuf(_IOLBF)`) 이다. 파이프로 받아도 즉시 흐른다.
`stdbuf -oL` 로 감쌀 필요가 없어야 한다.

## 5. 자동시험 하네스 — `tests/harness/`

Python 3 표준 라이브러리만 쓴다.

```
tests/harness/
  run.py              CLI·결과 출력
  harness.py          앱 실행 · 키 주입 · 판정 (APPS 표가 앱별 환경변수를 쥔다)
  scenarios/          시나리오 — 한 파일에 하나 또는 배열
    3bul390.json      세벌식 390
    2bulstd.json      두벌식
    common.json       레이아웃 무관
```

### 실행

```sh
make test-apps                                   # XTEST 가능한 전 앱 × 전 시나리오
make test-app APP=gtk3
tests/harness/run.py --app xim --scenario commit-then-preedit
tests/harness/run.py --all --allow-layout-change # 필요하면 레이아웃을 바꾸고 되돌린다
tests/harness/run.py --list
```

시나리오의 `layout` 이 현재 설정과 다르면 **기본은 건너뛴다** — 사용자 설정을
말없이 바꾸지 않는다. `--allow-layout-change` 를 주면 바꾸고 끝나면 되돌린다.

### 동작

1. 앱을 그 앱 전용 환경변수로 띄운다 (`apps.py`)
2. `app.ready` 사건이 JSONL 에 뜰 때까지 대기 (타임아웃 10 s)
3. 시나리오의 필드로 포커스 이동 — `xdotool` 클릭 또는 Tab
4. 각 스텝의 키를 `xdotool key` 로 주입 (XTEST → 툴킷 → IM 모듈 → 데몬)
5. 스텝마다 기대값과 JSONL 을 대조 (기본 대조 대상은 `field.render.rendered`)
6. 불일치 시 `import -window` 스크린샷 + 해당 구간 JSONL 을 결과에 첨부
7. 앱 종료, 판정 출력

### 시나리오 형식

```json
{
  "name": "commit-then-preedit",
  "desc": "확정 직후 다음 글자가 같은 키에서 즉시 보이는가 (2026-08-07 XIM 회귀)",
  "layout": "3bul390",
  "field": "core.plain",
  "steps": [
    { "key": "y", "expect": { "preedit": "ㄹ", "committed": "" } },
    { "key": "y", "expect": { "preedit": "ㄹ", "committed": "ㄹ" } },
    { "key": "y", "expect": { "preedit": "ㄹ", "committed": "ㄹㄹ" } },
    { "key": "space", "expect": { "preedit": "", "committed": "ㄹㄹㄹ " } }
  ]
}
```

`layout` 이 현재 설정과 다르면 하네스가 실행 전에 `unim-cli` 로 바꾸고 끝나면
되돌린다.

### 알려진 실패 표시

시나리오에 `known_fail` 을 두면 그 앱의 실패를 `✗ KNOWN` 으로 구분해 세고,
**종료 코드를 더럽히지 않는다**(신규 회귀만 빨갛게 만든다).

```json
"known_fail": { "xim": "왜 실패하는지 한 줄" }
```

고쳐지면 이 항목을 지운다 — 그때부터 다시 `FAIL` 로 잡힌다. 통과로 위장하지
않는 것이 요점이다.

### 필수 시나리오 (회귀 자산)

| 이름 | 유래 |
|---|---|
| `basic-compose` | 기본 조합·확정 |
| `commit-then-preedit` | 2026-08-07 XIM ON-THE-SPOT preedit 누락 |
| `click-commit` | 2026-08-06 조합 중 클릭 시 클릭 자리 커밋 |
| `focus-switch` | Tab 전환 시 조합 플러시 |
| `password-suppress` | 비밀번호 필드 AutoTypeFix·팝업 억제 |
| `backspace-decompose` | 조합 중 백스페이스 분해 |
| `english-passthrough` | 영문 모드 무간섭 |
| `mode-toggle` | 한/영 전환 중 조합 플러시 |

### CI 기능 타이핑 (`scripts/ci/functional-test.sh`)

이 하네스는 실세션(로그인한 데스크톱, 실 D-Bus 세션 버스, 실 unim 데몬)을
전제한다 — CI 러너·배포판 컨테이너에는 그런 세션이 없다. `scripts/ci/
functional-test.sh <tag>` 가 그 간극을 메운다: `Xvfb`(헤드리스 X 서버) +
`dbus-run-session`(격리된 세션 버스) 안에서 데몬(과 필요하면 `unim-xim`)을
새로 기동하고, `tests/harness/run.py` 를 그대로 돌린다. `HOME`/`XDG_*`/
`UNIM_{CONFIG,DATA,CACHE}_DIR` 을 전부 스크래치 디렉터리로 격리하므로 실행
중인 실세션 설정·데몬에는 닿지 않는다.

```sh
scripts/ci/functional-test.sh ubuntu24.04                       # 기본 5앱(gtk3/gtk4/qt5/qt6/xim) × 전 시나리오
scripts/ci/functional-test.sh ubuntu24.04 --apps gtk3,xim
scripts/ci/functional-test.sh ubuntu24.04 --scenarios basic-compose,click-commit
```

전제는 두 가지 중 하나다: **설치된 패키지**(기본 — 데몬은 `/usr/libexec/
unim-daemon`, IM 모듈은 시스템 GTK_PATH/QT_PLUGIN_PATH 가 이미 안다 —
`scripts/ci/build-{deb,rpm}.sh --smoke` 가 `verify-installed.sh` 직후, 제거
검증 전에 이 스크립트를 부른다), 또는 **로컬 `target/release` 빌드본**
(`UNIM_DAEMON_BIN`/`UNIM_XIM_BIN` 환경변수로 오버라이드 — `make
check-runtime-x11` 이 문서화하는 경로. 이 타깃은 호스트 실세션 보호를 위해
자동 실행하지 않는다). 테스트 앱(`tests/unim-test-*`)은 `tests/common`(C) +
GTK/Qt/X11 헤더만으로 매번 그 자리에서 직접 컴파일한다 — `make build-tests`
가 끌어오는 전체 cargo 워크스페이스 빌드는 필요 없다.

CI 배선: push/PR 마다 `linux-ci.yml` 의 `functional-x11` 잡이 우분투 24.04
러너 네이티브에서(`make install` 로, 패키지 빌드 없이) 상시 검증하고,
릴리스 매트릭스(`linux-deb.yml`/`linux-rpm.yml`)는 배포판별 실제 설치 패키지
로 검증한다. 실패하면 앱별 JSONL 로그·실패 스텝 스크린샷이 두 워크플로 모두
CI 아티팩트로 올라온다.

> [!note] el10(AlmaLinux/RHEL 10 계열)은 이 시험을 건너뛴다(SKIP, 빌드는
> 안 막는다) — 2026-09 실측 기준 EPEL10 이 아직 `xorg-x11-server-Xvfb`·
> `xdotool`·`xwininfo` 를 패키징하지 않는다(Xwayland 만 있다). 코드 회귀가
> 아니라 배포판 패키지 생태계가 못 따라온 것이라, `functional-test.sh` 가
> 이 도구들의 부재를 감지하면 스킵하고 빠져나온다 — `verify-installed.sh`
> (L1+L2)는 이미 통과한 뒤다. EPEL10 이 채워지면 자동으로 다시 돈다.

## 6. 디렉토리

```
tests/
  common/            C/C++ 공용 — 스펙·로그·필드 엔진·DBus 러너
    unim_test_spec.h     ← UI 스펙 단일 정의
    unim_test_log.{h,c}  ← JSONL 로거
    unim_test_field.{h,c}← 코어 필드 상태기계(툴킷 무관)
    unim_test.{h,c}      ← 기존 DBus 스모크 러너 (유지)
  harness/           자동시험 하네스
  unim-test-gtk3/    GTK3
  unim-test-gtk4/    GTK4
  unim-test-qt/      Qt5·Qt6 공용 소스 1벌 → 바이너리 2개
  unim-test-xim/     XIM
  unim-test-wayland/ Wayland
  unim-test-gnome/   GNOME 확장 경로
  unim-test-dbus/    헤드리스 DBus 스모크
```

### Rust 앱은 미러가 아니라 같은 코드를 쓴다

`tests/common-rs/` 는 스펙·로거·필드 엔진을 Rust 로 **다시 구현하지 않는다.**
`build.rs` 가 `tests/common` 의 C 파일 세 개를 그대로 컴파일해 링크한다.

```
tests/common-rs/
  build.rs           cc 로 unim_test_{spec,log,field}.c 컴파일
  src/lib.rs         extern 선언 + 안전한 껍데기
  examples/smoke.rs  C 판과 결과가 같은지 확인 (cargo run --example smoke)
```

미러를 두면 언젠가 어긋나지만 같은 오브젝트를 링크하면 어긋날 수가 없다.
매크로와 static 배열은 FFI 로 안 보이므로 `unim_test_spec.c` 가 접근자를
노출한다(`unim_spec_metrics()` 등). 가변인자 로그 함수도 FFI 로 부르기
까다로워 `unim_log_note_str()` 같은 비-가변인자 판을 함께 둔다.

`unim_test_dbus.c` 는 gio 의존이라 **`dbus` 기능으로만** 붙인다. 켜면 데몬
연결과 상태 패널 6줄 문구까지 C 앱과 같은 함수에서 나온다 — 상태 문구를
Rust 로 옮겨 적으면 6개 앱 화면이 어긋나므로, 쓰는 앱은 반드시 켠다
(`unim-test-wayland` 가 그렇다). `unim_test.c`(DBus 스모크 러너)는 안 쓴다.

GDBus 는 GMainContext 위에서 돈다. calloop 은 그걸 돌리지 않으므로 앱이
루프마다 `daemon::pump()` 를 불러야 데몬 신호가 도착한다 — Qt 앱이 `QTimer`
로 `g_main_context_iteration` 을 도는 것과 같은 이유다.

**삭제**: `unim-test-qt5/`, `unim-test-qt6/` — `unim-test-qt/` 가 두 바이너리를
모두 만든다(이미 CMakeLists 에 구현되어 있음). Makefile 만 옛 경로를 보고 있었다.

### Wayland 앱 (2026-08-09 재작성)

`unim-test-wayland` 는 `common-rs` 위에 다시 썼다. 화면·필드·로그가 다른 5개
앱과 같아졌고, `wl_shm` 버퍼에 바이트를 직접 쓴다(`Canvas`).

**이전 판이 먹통이던 이유 셋** — 새로 쓸 때 같은 함정을 다시 밟지 않도록:

1. **IME 가 아예 붙지 않았다.** `zwp_text_input_manager_v3` 를
   `Dispatch<WlRegistry, GlobalList>` 로 받으려 했는데, `registry_queue_init`
   이 만든 레지스트리의 user-data 는 `GlobalListContents` 라 그 impl 이 **한
   번도 불리지 않았다.** 매니저가 영영 `None` → text-input 객체 없음 → 조합
   없음. 지금은 시작할 때 `GlobalList::bind` 로 직접 잡는다.
2. **키 처리가 Escape 하나뿐이었다.** 영문·편집키가 전부 무시됐다.
3. **색 바이트 순서가 뒤집혀 있었다.** `wl_shm::Format::Argb8888` 은 32비트
   값 `0xAARRGGBB` 를 리틀엔디언으로 담으므로 메모리 배열은 **B,G,R,A** 다.
   tiny-skia 의 `PixmapMut` 은 R,G,B,A 로 읽는다. 라이브러리를 끼우면 이
   순서를 착각하기 쉬워서 지금은 `Canvas` 가 바이트를 명시적으로 쓴다.

**text-input-v3 를 다룰 때의 함정 둘**:

- **`done` 에 조건 없이 `commit` 으로 답하면 무한 왕복이 된다.** IM 은 클라
  이언트 `commit` 마다 `done` 을 보낸다. 첫 실행에서 6초에 6000번을 돌았다.
  → 앱 상태가 **실제로 바뀐 경우에만** 되쏜다.
- **preedit·commit·delete 는 `done` 에서 원자적으로 적용한다.** 이벤트마다
  바로 적용하면 화면이 중간 상태를 보인다. 순서는 프로토콜이 정한 대로
  삭제 → 확정 → 새 preedit.

**HiDPI** — `Canvas` 는 좌표를 **논리 픽셀**로 받고 안에서 배율을 곱한다.
버퍼는 장치 픽셀로 만들고 `set_buffer_scale` 로 컴포지터에 알려 주므로 배율 2
화면에서도 확대 흐림이 없다(스펙 수치는 앱 코드 어디에도 배율이 섞이지
않는다). 폰트도 `px × scale` 로 그려 또렷하다.

**`--dump-frame PATH`** — 합성한 화면을 PPM 으로 저장한다(매 프레임 덮어써서
파일에는 늘 마지막 화면이 남는다). GNOME 은 포털 밖 스크린샷을 거부하므로
(`org.gnome.Shell.Screenshot` → `AccessDenied`) Wayland 앱의 "화면의 진실"을
눈으로 확인하는 유일한 길이다. `convert frame.ppm frame.png` 로 본다.

**`--auto` 는 없앴다.** 데몬을 직접 부르는 시험은 프런트엔드 경로를 타지
않아 회귀를 놓친다(§1). 창을 띄워 눈으로 본다.

## 7. 툴킷 무관 필드 엔진 — `unim_test_field.{h,c}`

코어 필드의 상태·편집·렌더 문자열 계산을 툴킷과 무관한 순수 C 로 둔다.
GTK3/GTK4/Qt/XIM 이 이걸 공유하므로 **동작이 어긋날 수 없다.**

```c
typedef struct {
    const char *id;             /* "core.plain" */
    char  committed[4096];      /* 확정 텍스트 */
    int   caret;                /* 확정 텍스트 내 바이트 오프셋 */
    char  preedit[512];         /* 조합 중 문자열 */
    int   preedit_caret;        /* preedit 내 바이트 오프셋 */
    UnimFieldHint hint;         /* NONE/NUMBER/PASSWORD/SEARCH/MULTILINE */
    /* … */
} UnimTestField;

void unim_field_commit(UnimTestField *f, const char *text);
void unim_field_set_preedit(UnimTestField *f, const char *text, int caret);
void unim_field_backspace(UnimTestField *f);
void unim_field_move_caret(UnimTestField *f, int dir);
int  unim_field_caret_from_x(const UnimTestField *f, int x, UnimTextMeasure m);
/** 화면에 실제로 나타나는 문자열 (확정 + preedit 삽입) */
const char *unim_field_rendered(const UnimTestField *f, char *out, size_t n);
```

렌더는 툴킷이 하되 **문자열과 캐럿은 이 엔진이 정한다.** 툴킷 코드는 폰트로
그리기만 한다.

## 8. Makefile

| 타겟 | 뜻 |
|---|---|
| `make build-tests` | 6개 앱 + 하네스 준비 |
| `make test-apps` | `harness/run.py --all` |
| `make test-apps-xephyr` | 격리 디스플레이에서 전체 |
| `make test-app APP=gtk3` | 한 앱 |
| `make smoke-test` | 기존 DBus 스모크 (유지) |

## 9. 남은 제약

- **Wayland 네이티브 앱에는 XTEST 가 안 먹는다.** `unim-test-wayland` 와 GNOME
  경로는 하네스가 키를 주입할 수 없다(`APPS` 표의 `xtest: False` — 자동으로
  건너뛴다). 이 둘은 수동 검증으로 남기되, Wayland 앱은 `--dump-frame` 으로
  화면을 파일에 뱉으므로 눈으로 대조할 근거는 남는다. XWayland 로 띄우는
  GTK3/GTK4/Qt5/Qt6/XIM 5종은 완전 무인이다.
- **창 원점은 `xwininfo` 로 구한다.** `xdotool getwindowgeometry` 의 X/Y 는 CSD
  창에서 콘텐츠 원점과 어긋난다(GTK3 실측 (114,115) vs 실제 (100,66)).
  `xwininfo` 의 `Absolute upper-left` 가 정확하다. 앱이 화면 절대 좌표를
  낼 수 있으면(`screen_cx` ≥ 0) 그쪽을 먼저 쓴다.
- `xdotool key` 는 keysym 이름을 그대로 받는다. 시나리오에 적는 `y`·`space`·
  `BackSpace`·`Hangul` 은 X11 keysym 이름이다.
- **한/영 전환은 한/영 키로 시험한다.** DBus `SetGlobalMode` 는 조합 중인
  preedit 을 플러시하지 않는다 — 트레이·설정에서 부르는 경로라 사용자가 키로
  전환하는 길과 다르다. 시나리오는 실제 사용자 경로만 검증한다.
- **XIM 앱 창 제목은 `_NET_WM_NAME`(UTF8_STRING) 으로도 설정해야 한다.**
  `XStoreName` 은 Latin-1 이라 한글 제목이 깨지고 `xdotool search --name` 이
  창을 못 찾는다.

## 10. 하네스가 처음 돌자마자 찾아낸 것 (2026-08-08)

인프라가 제 몫을 했다는 기록이자, 각각의 후속 작업 목록이다.

| # | 증상 | 판정 | 상태 |
|---|---|---|---|
| ① | Qt5·Qt6 `click-commit` 실패. `commit core.plain "한"` 직후 `commit core.plain2 "한"` — 같은 글자가 클릭한 필드에 또 박힌다 | **설치 문제.** 코드는 맞는데 설치된 플러그인이 07-19 빌드본이었다 | ✅ `make install-frontends PREFIX=/usr` 로 해결 |
| ② | Qt5·Qt6 `focus-switch` 실패 | **테스트 앱 결함.** Qt 는 Tab 을 위젯 포커스 이동으로 먼저 처리해 `keyPressEvent` 에 오지 않는다. GTK 판은 가로채고 있었다 | ✅ `event()` 에서 Tab 가로채기 |
| ③ | XIM `multiline-compose` 실패. 조합 중 `Return` → `"한\n"` 이 아니라 `"\n한"` | **동작 결함** | ✅ 해결 (아래) |

### ①에서 배운 것 — 설치 경로

`make install-frontends` 를 그냥 돌리면 `PREFIX ?= /usr/local` 이라
`/usr/local/lib/qt6/plugins/…` 로 간다. **Qt·GTK 는 그 경로를 보지 않는다.**
deb 가 쓰는 자리는 multiarch 경로이므로 `PREFIX=/usr` 를 반드시 준다.

```sh
sudo make install-frontends PREFIX=/usr
# → /usr/lib/x86_64-linux-gnu/{qt5,qt6}/plugins/platforminputcontexts/libunim.so
```

파일명뿐 아니라 **경로도** 기존 설치본과 대조할 것.

### ③ XIM Enter 순서 — 시도한 것과 실패 이유

서버는 순서를 지키는데 클라이언트가 뒤집는다. 실측:

```
서버     : 키 입력(Return) → 커밋 "한" → forward      (정상)
클라이언트: preedit 비움 → forward(\n) → commit "한"   (뒤집힘)
```

**Xlib XIM 이 forward 받은 이벤트를 자기 이벤트 큐 앞으로 되돌리기** 때문으로
보인다. 두 가지를 시도했고 둘 다 막혔다.

| 방식 | 결과 |
|---|---|
| (a) 키를 `consumed` 로 삼키고 commit 뒤 XTest 로 재주입 | 순서는 고쳐졌다(`"\n한"` → `"한"`). 다만 재주입한 Return 이 서버로 되돌아오지 않아 줄바꿈이 빠졌다 |
| (b) commit 뒤 forward 앞에 20 ms 지연 | 효과 없음. **도착 순서가 아니라 처리 순서 문제**라 지연으로는 못 이긴다 — 클라이언트가 두 메시지를 모두 받아 둔 채 큐를 재배치한다 |

**(a) 를 채택했다.** 막힌 곳은 딱 한 지점이었다.

진단은 테스트 앱이 `XFilterEvent` **앞에서** 원본 키를 먼저 로그하게 만들자
바로 풀렸다. "앱이 받기는 했는가" 와 "IM 이 삼켰는가" 를 구분하지 못하면
이런 문제는 추적할 수 없다.

```
X 수신: type=2 keycode=36     ← 원본 Return
XFilterEvent 삼킴: keycode=36  ← 서버로 (여기서 삼켜 확정)
… commit "한" …
X 수신: type=3 keycode=36     ← 재주입 KeyRelease만 도착!
```

**재주입한 KeyPress 가 사라지고 KeyRelease 만 도착했다.** 원본 키가 아직
물리적으로 눌려 있어서 X 서버가 중복 `KeyPress` 를 버린 것이다. 그래서
재주입 순서를 `release → press → release` 로 바꿨다 — 먼저 눌림 상태를
정리한 뒤 새로 누른다.

```rust
XTestFakeKeyEvent(dpy, keycode, 0, 0);  // 눌림 상태 정리
XTestFakeKeyEvent(dpy, keycode, 1, 0);
XTestFakeKeyEvent(dpy, keycode, 0, 0);
```

이걸로 `multiline-compose` 가 통과하고 XIM 이 **10/10** 이 됐다.

중간에 잘못 짚은 것도 남겨 둔다 — 서버 로그만 보고 "XTest 가 배달되지
않는다" 고 결론지었는데, 실제로는 재주입된 키가 Xlib XIM 을 그대로 통과해
**서버를 거치지 않고** 앱이 직접 처리하기 때문에 서버 로그에 안 보였던
것이다. 한쪽 로그만으로 배달 여부를 판정하면 안 된다.

한계: modifier 는 재현하지 않는다. Shift+Enter 처럼 수식키가 붙은 채
확정하는 조합은 수식키 없이 전달된다.
