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

**아직 옮기지 않은 것**: `unim-test-wayland` 는 Rust 라 공용 C 코드를 그대로
못 쓴다. `tests/common-rs/`(spec·log·field 미러)를 만들어야 하고, 미러가
어긋나지 않게 `make check-test-spec` 로 대조할 계획이다. 그때까지 이 앱만
옛 구조로 남는다.

**삭제**: `unim-test-qt5/`, `unim-test-qt6/` — `unim-test-qt/` 가 두 바이너리를
모두 만든다(이미 CMakeLists 에 구현되어 있음). Makefile 만 옛 경로를 보고 있었다.

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
  건너뛴다). 이 둘은 (a) 앱 자체의 재생 모드로 내부에서 키 이벤트를 합성하거나
  (b) 수동 검증 체크리스트로 남긴다. XWayland 로 띄우는 GTK3/GTK4/Qt5/Qt6/XIM
  5종은 완전 무인이다.
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

| # | 증상 | 판정 |
|---|---|---|
| ① | Qt5·Qt6 만 `click-commit`·`focus-switch` 실패. 로그에 `commit core.plain "한"` 직후 `commit core.plain2 "한"` — 같은 글자가 클릭한 필드에 또 박힌다 | **설치 문제.** 코드는 맞는데 이 기기의 `/usr/lib/x86_64-linux-gnu/qt{5,6}/plugins/platforminputcontexts/libunim.so` 가 07-19 빌드본이다. dedupe 를 넣은 08-07 판이 설치되지 않았다 |
| ② | XIM 만 `multiline-compose` 실패. 조합 중 `Return` → `"한\n"` 이 아니라 `"\n한"` | **동작 결함.** XIM 이 forward(`\n`)를 확정 문자보다 먼저 보낸다. GTK·Qt 경로는 정상. `known_fail` 로 표시해 뒀다 |

②는 2026-08-07 에 고친 "preedit 을 commit 보다 먼저" 와 같은 계열의 순서
문제다. 실제 앱에서 한글 조합 중 Enter 를 누르면 줄바꿈이 글자보다 앞에 간다.
