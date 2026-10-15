# UNIM GNOME Shell Extension 세부 기능 명세

> GNOME Shell 환경에서 "TypeFIX(오타 보정)" 기능과 "실시간 한글 입력(IME)" 기능을 동시에 제공하는 확장 프로그램입니다.
> `IBus`를 거치지 않고 `Clutter.InputMethod` 서브클래스를 Clutter Backend에 직접 등록하여 네이티브 성능을 확보했습니다.

> 팝업 UI(한자·특수문자)의 공통 명세는 [`../docs/dev/specs/POPUP_SPEC.md`](../docs/dev/specs/POPUP_SPEC.md)를 단일 원본으로 삼습니다.
> 본 문서는 GNOME Shell 고유 구현만을 다룹니다.

---

## 1. 아키텍처 개요

### 1.1 컴포넌트 구성

| 파일 | 역할 |
|------|------|
| `extension.js` | 확장의 수명주기 관리 (enable/disable), 컴포넌트 조율, TypeFIX 단축키 바인딩 |
| `dbus_ime.js` | `unim-daemon`과의 DBus 통신 (`Gio.DBusProxy` + 글로벌 signal 구독) |
| `unim_input_method.js` | `Clutter.InputMethod` 서브클래스 (vfunc 오버라이드로 Mutter 연동) |
| `key_handler.js` | 키 이벤트 필터링·분류·라우팅, call_sync 재진입용 **key queue**, (v1.6) 바이패스 키·포인터·PRIMARY 소유자 변경을 `SurroundingHint` 로 보고(§2.8) — v1.2~v1.5 의 PRIMARY 선택 보강은 퇴역(§2.8.1) |
| `capabilities.js` | (v1.6) `SetClientCapabilities` 능력 비트 순수 계산(`computeClientCapabilities`) — 실측 스위치 `surrounding-word-replace` 값에 따라 0x4/0x5. gi 의존 없는 순수 모듈 — node 단위 시험(§2.8) |
| `pointer_hints.js` | (v1.6) 포인터 사건 분류(단일·Shift·500ms 더블클릭·4px 드래그·터치·보조/가운데 버튼 → `POINTER_PLAIN`/`POINTER_SELECT`)와 힌트 키 로그 이름. gi 의존 없는 순수 모듈 — node 단위 시험(§2.8). v1.2~v1.5 의 `primary_selection.js` 는 퇴역·삭제(§2.8.1) |
| `preedit_overlay.js` | 입력 중인 글자(Preedit)를 커서 위치에 표시하는 오버레이 |
| `hanja_popup.js` | 한자 후보 선택 팝업 UI (`St.BoxLayout` 기반) |
| `special_popup.js` | 특수문자 선택 팝업 UI (9×9 그리드) |
| `vkbd.js` | 가상 키보드 이벤트 생성 (AutoTypeFix backspace·TypeFIX paste용) |
| `indicator.js` | 상단 패널 입력 모드 표시기 + 설정/메뉴 항목 |
| `prefs.js` | 확장 환경설정 (GNOME Shell 전용 키만 노출, 일반 설정은 GTK GUI 리다이렉트) |
| `logging.js` | 통합 로깅 (`UNIM_DEVELOP=1` 시 활성) |
| `stylesheet.css` | 팝업/오버레이/인디케이터 스타일 |

### 1.2 통신 구조

```
┌──────────────┐  clutter_backend_   ┌──────────────────┐    DBus     ┌──────────────┐
│ GNOME Shell  │  set_input_method   │ GNOME Extension  │ ←────────→ │ unim-daemon  │
│ (Mutter)     │ ←─────────────────→ │ (JavaScript)     │  (Session)  │ (Rust)       │
└──────────────┘   C vtable 경유      └──────────────────┘             └──────────────┘
       ↑                                     │
       └────────────── St/Clutter ───────────┘
                  (Preedit/Popup UI)
```

### 1.3 키 이벤트 흐름

```
libinput → Mutter
  └→ vfunc_filter_key_event (UnimInputMethod, C vtable 경유)
     ├→ self-sent BackSpace 우회 (AutoTypeFix vkbd self-feedback 차단)
     ├→ KeyHandler 위임 → DBus ProcessKeyEvent (call_sync)
     │   └→ call_sync 중 재진입 키는 _keyQueue에 적재 후 _drainKeyQueue()로 순차 처리
     └→ notify_key_event(event, consumed) 로 사후 통보 (IBus 패턴)
```

- `GObject.registerClass()`로 등록된 JS 서브클래스의 vfunc은 C vtable에 올바르게 바인딩됨
- `Clutter.get_default_backend().set_input_method(im)` — `CLUTTER_EXPORT` API, GJS에 노출됨

---

## 2. 주요 기능

### 2.1 실시간 입력기 (Real-time IME)

- **연동 방식**: `Clutter.get_default_backend().set_input_method(this._inputMethod)`
- **키 처리**: `vfunc_filter_key_event` 오버라이드 → `KeyHandler` 위임 → `unim-daemon` DBus 호출
- **비활성화 시**: `backend.set_input_method(savedInputMethod)`로 원본 IM 복원

### 2.2 키 분류별 처리 (`key_handler.js`)

| 키 분류 | 동작 |
|---------|------|
| 문자 키 | DBus `ProcessKeyEvent` → commit/preedit 처리 |
| 수정자 키 (Shift, Ctrl 등) | `vfunc`에서 `false` 반환 → Mutter가 직접 처리 (고정키 접근성 유지) |
| 오른쪽 Alt (bare Alt_R) | 절대 비소비(`EVENT_PROPAGATE` 유지 → Sticky Keys 불변) + `processKey` fire-and-forget 통지 → 데몬 `toggle_keys` 판정 |
| Ctrl/Alt/Super 조합 | 조합 flush → 바이패스 (시스템 단축키) |
| 네비게이션 (←→↑↓, Home, End, PgUp/Dn) | 조합 flush → 바이패스 |
| Enter / KP_Enter, Escape, Tab | 조합 flush → 바이패스 |
| 한자키 (F9, Hangul_Hanja) | 엔진이 팝업 시그널 발사 → GNOME 팝업 표시. (v1.6) idle 이면 엔진이 대상② → 대상③(커서가 걸친 단어) → 이모지 순으로 판정 — 확장은 PRIMARY 를 읽지 않는다(§2.8) |
| BackSpace | DBus `ProcessKeyEvent` → 자모 삭제 (self-sent BackSpace는 선제 우회) |
| KEY_RELEASE | IM 미처리, `notify_key_event(false)`만 호출 (키 반복 유지). (v1.5 의 "PRIMARY 읽기로 미뤄진 눌림 뒤 큐잉" 은 v1.6 에서 읽기와 함께 퇴역, §2.8.1) |

- **`_flushCompose()`**: DBus `FocusOut` 호출 → 조합 중 텍스트 커밋 + preedit 클리어
- **Key Queue 패턴**: `ProcessKeyEvent`가 `call_sync()`이므로 GLib 메인 루프 재진입이 발생할 수 있음. `_processingKey` 플래그로 재진입을 감지하여 후속 키를 `_keyQueue`에 저장하고, 현재 호출 완료 후 `_drainKeyQueue()`가 FIFO 순서로 처리하여 키 누락을 방지.
- **이중 처리 방지**: Backend IM 등록 시 `captured-event` 핸들러에서 `EVENT_PROPAGATE` 반환
- **repeat 태깅(접근성)**: Clutter `REPEATED` 플래그를 `state` 상위 비트의 `UNIM_KEY_REPEAT_MASK`(1<<29)로, 그리고 항상 `UNIM_REPEAT_AWARE_MASK`(1<<31)로 실어 보낸다(vfunc·드레인·X11 divert 경로 공통, `>>>0` unsigned 정규화). 데몬은 `ignore_key_repeat` on 일 때 이 비트로 반복을 정확 판정한다. 확장 비활성화 시 통지 콜백(`setToggleKeyNotifier`)은 해제되며, GNOME 확장 변경분은 재로그인 후 적용된다.

### 2.3 포커스 처리

- **Focus In**: DBus `FocusIn(windowId)` 호출
- **Focus Out**: 팝업 정리 → DBus `FocusOut` → 반환된 조합 중 텍스트를 `commitText`로 커밋 → preedit 클리어
- `vfunc_focus_out` → `_focusOutHandler()` 콜백 → extension.js에서 등록
- 영문 모드 Space는 데몬이 일반 commit 경로로 처리 (2026-04 수정)

### 2.4 한자/특수문자 팝업

> UI/입력 규칙은 [`docs/dev/specs/POPUP_SPEC.md`](../docs/dev/specs/POPUP_SPEC.md) 단일 원본 참조.

GNOME extension 고유 사항:
- **표시 주체**: Wayland 세션일 때 extension이 **글로벌 signal 구독**으로 자기 context 외 프론트엔드(GTK3/4, Qt, XIM)의 팝업 시그널도 수신하여 표시 (Wayland 공통 팝업 서버 역할). X11에서는 `unim-gui-gtk`가 담당하므로 extension은 자기 context 시그널만 처리.
- **좌표 변환**: `_adjustCursorRect()` — 외부 프론트엔드 좌표계 차이 보정
  - 네이티브 Wayland 앱(GTK3/4): 윈도우 상대좌표 + `focus_window.get_buffer_rect()` 오프셋
  - XWayland 앱(XIM/Qt): X11 절대좌표 그대로 사용
- **화면 경계 처리**: 오른쪽/아래 넘침 시 좌/상 조정, 실패 시 화면 중앙 폴백

### 2.5 TypeFIX (오타 보정 단축키)

사용자 단축키(`<Super>k`, `<Shift><Super>k`)로 최근 포커스된 컨텍스트의 선택/주변 텍스트를 변환.

- **엔진 API**: `GlobalTypeFix` (InputMethod iface, 43fbb43에서 도입) — 클립보드 미사용
- **흐름**:
  1. `Main.wm.addKeybinding()` 단축키 감지
  2. `request_surrounding()` 후 50ms 대기 (gedit/gnome-text-editor 호환)
  3. `TypeFix(direction)` DBus 호출 → `(deleteOffset, deleteCount, replacement)` 수신
  4. `delete_surrounding(offset, count)` + `commitText(replacement)` 로 치환
  5. `show-notification` 활성 시 `Main.notify()` 로 알림
- **direction**: 0=자동, 1=영→한, 2=한→영 (`shortcut-normal-reverse` 사용 시 2)

### 2.6 AutoTypeFix (조합 중 자동 교정)

엔진이 롤백 기반 자동 교정을 감지하면 `AutoTypefixApply(deleteChars, commitText, preeditText)` 시그널을 발사.

- **수신 경로**: `dbus_ime.js` 글로벌 signal 구독 (자기 context 한정) → `_handleContextSignal`
- **적용 로직** (`extension.js` `onAutoTypeFix`):
  1. preedit 비어 있지 않으면 `clearPreedit()` 선행 (한자 단어 교체 잔상 방지)
  2. `expectSelfBackspaces(deleteChars)` — `UnimInputMethod`의 self-backspace 카운터 등록
  3. `vkbd.backspaceMultiple(deleteChars)` — 가상 키보드로 BackSpace 연타
  4. 50ms 후 `commitText(commitText)`, 다시 10ms 후 `updatePreedit(preeditText)`
- **Self-feedback 차단 (af8b563)**: vkbd가 보낸 BackSpace는 Mutter를 거쳐 `vfunc_filter_key_event`에 재진입한다. 한글 엔진이 이를 실제 backspace로 오인해 복원된 preedit 음절을 다시 깎는 self-feedback을 막기 위해 `_selfBackspaceCount`를 PRESS+RELEASE = 2×N으로 등록하고, 매칭되는 BackSpace 이벤트는 IM 처리 없이 `false` 반환으로 mutter에 통과시킨다.

### 2.7 콘텐츠 목적(Content Purpose) 억제 — 비밀번호/PIN 필드 자동 영문 전환

비밀번호·PIN 등 입력 필드에 포커스가 있으면 ATF(자동 오타 교정)·한영 자동전환을
데몬에 억제시켜야 한다(억제 체인: extension → `SetContentType(u)` DBus →
`unim::config::ContentPurpose` → `should_block_hangul()`).

**Clutter → UNIM 매핑표** (`unim_input_method.js` `CLUTTER_PURPOSE_TO_UNIM`,
로컬 `/usr/lib/x86_64-linux-gnu/mutter-14/Clutter-14.gir` 실측):

| Clutter InputContentPurpose | 값 | UNIM ContentPurpose |
|---|---|---|
| normal | 0 | Normal(0) |
| alpha | 1 | Normal(0) — GTK 표와 의도적 일치 |
| digits | 2 | Normal(0) |
| number | 3 | Number(4) |
| phone | 4 | Normal(0) |
| url | 5 | Url(5) |
| email | 6 | Email(3) |
| name | 7 | Normal(0) |
| password | 8 | **Password(1)** |
| date | 9 | Normal(0) — GTK/IBus 의 9=PIN 과 무관, passthrough 금지 |
| time | 10 | Normal(0) — GTK/IBus 의 10=TERMINAL 과 무관 |
| datetime | 11 | Normal(0) |
| terminal | 12 | Terminal(6) |

GTK(`GtkInputPurpose`)·IBus(`IBusInputPurpose`) enum 과 9·10 자리의 의미가
다르므로 그 표를 재사용하는 passthrough 는 절대 금지 — 위 Map 만 사용한다.

**PIN 구조적 미구분**: Clutter enum 에는 PIN 값이 없다. 이를 보완하기 위해
`Clutter.InputContentHintFlags.HIDDEN_TEXT`(64) 힌트가 서 있으면 purpose 가
Normal 로 떨어졌어도 Password(1)로 승격한다(`_effectivePurpose()`). `SENSITIVE_DATA`
(128)는 의도적으로 미사용 — Qt 선례(`unim-frontends/qt5/src/input_context.cpp:499-509`)와
동일하게 "화면에 보이는 민감 필드"(카드번호 등)까지 차단하면 오차단이 된다.

**리셋 지점은 `vfunc_focus_out`** — Normal(0) 복귀는 focus_out 에서만 송신한다.
`vfunc_focus_in`에서 무조건 Normal 을 보내지 않는 이유: Mutter 가
`vfunc_update_content_purpose` 를 `vfunc_focus_in` 보다 먼저 호출하는 순서일 경우
(호출 순서 미확정 — Clutter-14.gir 의 InputMethod vtable 선언 순서는 호출 순서를
보장하지 않음), focus_in 무조건 리셋이 방금 도착한 Password 를 즉시 덮어써 차단을
영구 무력화한다. 대신 포커스 없는 동안 도착한 purpose 는 `_pendingPurpose` 에
버퍼링해 두었다가 다음 `vfunc_focus_in` 이 flush 한다(포커스 중 도착분은 그 자리에서
즉시 송신). `vfunc_reset` 은 purpose 를 건드리지 않는다 — reset 은 필드 **내부**
조합 취소(Escape 등) 이벤트라, 여기서 Normal 로 되돌리면 비밀번호 필드 안에서
Escape 한 번에 차단이 풀리는 회귀가 된다.

**호출 순서 실측 상태(2026-07-26)**: `vfunc_update_content_purpose`/
`vfunc_update_content_hints` 에 계측 로그(`unimLog('IME', 'update_content_purpose:
clutter=...')`)를 추가했으나, 확장 재로그인 없이는 실제 gnome-shell 세션에서
Mutter 가 이 vfunc 을 호출하는지·focus_in 과의 순서가 무엇인지 관측할 수 없어
**실측은 아직 미완료**(정적 구현 + 코드 스타일 검증만 완료). 다음 실제 재로그인
QA 세션에서 `~/.unim-log` 의 `[GNOME_EXT] ... update_content_purpose` 라인 유무와
`vfunc_focus_in` 대비 순서를 확인하고 본 절을 갱신할 것. `_pendingPurpose` 더블버퍼는
이 불확실성에 대한 안전망이므로 실측 후에도 제거하지 않는다(Mutter 버전 차이 방어).

**알려진 단방향 한계(실측 TODO)**: 현재 안전망은 "포커스 없는 동안 도착한 purpose"
만 버퍼링한다. 만약 Mutter 가 **다음 필드의 purpose 를 직전 필드의 focus_out 보다
먼저**(포커스가 아직 살아있는 동안) 전달하는 순서라면, 그 값은 즉시 송신된 뒤
focus_out 의 Normal 복귀·상태 초기화(`_sentPurpose`/`_contentPurposeRaw`)에 지워져
다음 focus_in 에 flush 할 pending 이 없다 — 비밀번호 필드가 Normal 로 동작하는
under-block. 반대로 이를 막으려고 pending 을 무조건 보존하면 purpose 를 안 보내는
앱으로 이동 시 직전 Password 가 재적용되는 over-block 이 된다. 어느 쪽 순서인지는
위 계측 로그 실측으로만 확정 가능하므로, 실측에서 "focus_out 이전 도착" 순서가
관측되면 그때 pending 보존 + focus_out 시점 구분 방식으로 전환한다(오늘자 로그
실측상 focus_in/out 은 IN 221/OUT 221 엄격 교대 — 현 설계가 우선).

현재 코드는 pending 보존 쪽(over-block 분기)을 택하되 **TTL 2초**
(`PENDING_PURPOSE_TTL_US`, 단조 시각 기준)로 창을 봉쇄한다: 정상 순서(purpose 도착
직후 focus_in)는 ms 단위라 영향이 없고, 대상 필드가 포커스를 끝내 못 받고 사라진
stale pending 은 flush 시점에 폐기된다. 잔여 위험은 "pending 도착 후 2초 안에
무관한 필드가 포커스를 얻는" 좁은 창뿐이며 실측 후 재평가한다.

extension.js `_onFocusWindowChanged` 의 창 전환 fail-safe(Clutter vfunc 를 우회하는
별도 DBus focusOut/focusIn 경로에서 `!hasFocus()` 게이트로 `SetContentType(0)` 강제)는
위 실측이 끝난 뒤 2단계로 미룬다 — `hasFocus()` 접근자만 선행 추가해 두었다.


### 2.8 대상③ 거부 신호 — 선택 가능성·낡음 보고 (HANJA_WORD_SPEC v1.6 §2.4b·§4.6, 구현 — 대상③ 확정은 §6.5 (10) 실측 전까지 비활성, caps 0x4)

> **v1.6 (2026-10-12, 기현님 확정 설계)**: v1.2~v1.5 의 "idle 한자키 PRIMARY 선택 보강" 은 **퇴역**한다(아래 "퇴역" 절). 확장은 더 이상 선택을 추정하지 않는다. 대신 idle 한자키가 **커서가 걸친 단어**(대상③)를 바꾸게 되면서, 확장은 엔진이 직접 보지 못하는 사건만 **보고**하고 판정은 엔진이 한다(SoT 1곳).

text-input-v3 → mutter 경로에서 GTK3·LibreOffice(gtk3/gtk4 VCL)·Firefox 앱은 선택이 있어도 anchor=cursor 로 보고하고(gtk-3-24 imwayland.c:893-901), GTK4·Qt6·Chromium 은 anchor 를 보내지만 **불규칙하고 늦다**(2026-10-04 실측: Shift+Right 보고, Shift+Home 미보고, 마우스 선택은 다음 클릭 때 한 번 늦게, 더블클릭 선택은 미보고). 앱이 IM 에 알리지 않은 선택이 있는 채 대상③ 을 확정하면 커밋이 그 선택까지 치환해 글자가 사라지고, 낡은 surrounding 으로 대상③ 을 잡으면 엉뚱한 글자가 지워진다. 확장은 이 둘을 막는 사건을 보고한다.

- **능력 선언**: FocusIn 직후마다(멱등) `SetClientCapabilities(caps)` — 비트 계산은 gi 의존 없는 순수 함수 `capabilities.js` 의 `computeClientCapabilities(replaceEnabled)`(node 단위 시험). GSettings 키 `surrounding-word-replace`(**기본 false**, prefs 미노출, 실측 전용 — §5.1)가 false 인 동안 `0x4 POINTER_HINTS` 만, 켜면 `0x5`(`REPLACE_SURROUNDING | POINTER_HINTS`). 확장은 키를 실시간으로 읽고(`extension.js` `_readSurroundingWordReplace`, 키 없는 옛 스키마는 `has_key` 로 거르고 false), 값이 바뀌면 `dbus_ime.js` `setSurroundingWordReplace` 가 현재 컨텍스트에 능력을 다시 선언한다(시그널은 `_settingsChangedIds` 로 `disable()` 에서 해제). §6.5 (10)(`delete_surrounding` 의 바이트/문자 삭제 수) 실측 절차는 HANJA_WORD_SPEC §6.5 (10), 판정은 데몬 `[REPLACE_VERIFY]` 로그(확장 로그는 기본으로 안 남는다). `REPLACE_SURROUNDING` 이 없는 동안은 엔진이 대상③ 을 끄지만(코드는 들어 있되 도달하지 않는다), 대상② 의 신선도 게이트와 아래 거부 신호는 바로 켜진다. `SELECTION_RELIABLE` 은 선언하지 않는다(App-push 경로).
- **보고**(`SurroundingHint(kind, keyval, state)`, 응답 없이·InputContext 직렬 처리로 이후 한자키보다 먼저 반영):

| kind | 언제 보내나 | 엔진 처리(HANJA_WORD_SPEC §2.4b.6·§2.4b.7) |
|---|---|---|
| `1 KEY` | 확장이 엔진을 거치지 않고 앱으로 넘긴 키(§2.2 의 탐색·Enter/Tab/Escape·Ctrl/Alt/Super 조합 바이패스) — 조합 flush(`_flushCompose`) **뒤에** 보낸다. keyval·state 그대로, 확장은 분류하지 않는다 | 낡음 설정. Shift+탐색·Ctrl+A → 거부 플래그 설정, 평이동·편집 키 → 해제 |
| `2 POINTER_PLAIN` | stage `captured-event` 의 Shift 없는 단일 버튼 눌림(직전 눌림에서 500ms 초과) | 낡음 설정, 거부 플래그 해제 |
| `3 POINTER_SELECT` | Shift+클릭, 더블/세 번 클릭(직전 눌림 500ms 이내), 드래그(눌린 채 4px 초과 이동 — 1회만) | 낡음 설정, 거부 플래그 설정 |
| `4 PRIMARY_CHANGED` | `global.display.get_selection()` 의 `owner-changed`(`Meta.SelectionType.SELECTION_PRIMARY`). **내용은 읽지 않는다** — `St.Clipboard`·`get_text`·mime 조회를 하지 않는다 | 거부 플래그 설정(낡음 아님) |

  - 보조 버튼은 무시, 가운데 버튼(붙여넣기)은 `POINTER_PLAIN` 과 같다(붙여넣기 커밋은 앱이 하므로 낡음이 함께 선다). 터치는 `POINTER_SELECT`(길게 눌러 선택을 구분할 수 없다 — 이모지로 가는 쪽).
  - 포인터 사건은 IM 포커스 창 밖 클릭도 보낸다(포커스가 바뀌면 FocusOut/FocusIn 이 뒤따르고, 엔진은 FocusIn 에서 상태를 초기화한다).
  - 비밀번호/PIN 목적(`UnimInputMethod.isSensitivePurpose()`)이어도 보고는 보낸다 — 내용이 없고 엔진이 비번 필드에서 대상③ 을 이미 막는다.
- **surrounding 전달**: Mutter `vfunc_set_surrounding` 값을 **그대로** `SetSurroundingText` 로 보낸다(가짜 anchor·가짜 텍스트 금지). 엔진이 수신을 낡음 해제로 쓴다. **포커스 이탈(`vfunc_focus_out`) 시 surrounding 캐시를 새 객체로 초기화**한다(이전 입력란 본문이 surrounding 을 안 보내는 클라이언트(VTE 등)에서 재사용되지 않게, focus_in 에서는 지우지 않음).
- **대상③ 확정**: 시그널 `ReplaceSurroundingText(a, b, text, expect)`(`uuss`; GNOME 은 expect 를 대조하지 않는다) → `UnimInputMethod.replaceSurrounding(a, b, text)` = `delete_surrounding(-a, a+b)` → `commit_text`. 시그널은 `AutoTypefixApply` 처럼 세션 버스 글로벌 구독(자기 context path 만)으로 받는다. idle 이라 preedit 선클리어가 필요 없다. `onAutoTypeFix` 와 같은 `_hasFocus` 게이트(포커스 없으면 드롭 — 문서 무변경). Clutter 쪽은 삭제 실패를 알 수 없다 — LibreOffice 선택 상태의 삭제 거부는 엔진의 `c == 0` 방어와 거부 플래그로 막는다(HANJA_WORD_SPEC §2.4b.8).
- **오프셋 단위**: Mutter `vfunc_set_surrounding` 의 cursor/anchor 와 데몬 엔진(Rust `chars()`)은 모두 **코드포인트** 단위다(2026-10-04 실측: 커밋 뒤 cursor 57→58 문자, len 228→231 바이트). `delete_surrounding` 의 a·b 도 코드포인트(구간은 전부 한글 완성 음절이라 BMP).
- **선택 삭제 금지(회귀 방지, 유지)**: 팝업(한자·특수·이모지) 표시 경로는 null 이 아닌 preedit(빈·투명·ZWSP 포함)을 세우지 않는다 — GTK4 GtkTextView 가 preedit-start 에서 선택을 지운다(ce32aed). 시험이 `onShowEmoji`·`onShowHanja`·`onShowSpecial` 본문에 `updatePreedit` 호출이 없고 `updatePreedit(ZWSP)` 호출이 없음을 정적으로 검사한다.
- **로그**: 한자키마다 엔진이 결정 이유 코드를 남긴다(HANJA_WORD_SPEC §2.8). 확장은 보낸 힌트의 kind·keyval(탐색·편집 키만 이름, 문자 키는 `char`)만 남긴다. surrounding·PRIMARY 내용은 남기지 않는다(PRIMARY 는 읽지도 않는다).
- **금지**: 클립보드 읽기·쓰기·백업·복원(메모리 feedback_no_clipboard_typefix 취지 — v1.6 부터 읽기도 금지).
- **시험**: 포인터 분류(단일·Shift·500ms 더블클릭·4px 드래그·보조/가운데 버튼)를 gi 의존 없는 순수 모듈로 떼어 node 단위 시험(`make test-gnome-logic` 자리 재사용, `make check-compat` 포함). 정적 시험에 "`St.Clipboard`·PRIMARY `get_text` 호출 없음" 을 더한다.

#### 2.8.1 퇴역 — idle 한자키 PRIMARY 선택 보강 (v1.2~v1.5, 2026-10-04~06)

아래는 v1.6 에서 걷어 낸 것(2026-10-12 삭제 — 백업 `~/backup/unim-wip-20261012-primary/`). 근거는 HANJA_WORD_SPEC §2.10.1(LibreOffice 가 PRIMARY 를 첫 읽기 시점에 굳힘·editeng 키보드 선택 미반영·mozc 도 PRIMARY 감시 제거).

| 퇴역 항목 | 위치 |
|---|---|
| 발동 게이트·이유 코드(`nokey`·`mod`·`nofocus`·`preedit`·`popup`·`sensitive`·`nosur`·`selknown`·`ok`)와 `PRIMARY 게이트:` 로그 | `KeyHandler._primaryGateReason` |
| PRIMARY 비동기 읽기(150ms)·읽는 동안 키 큐(`_processingKey`/`_keyQueue`) 재정렬·KEY_RELEASE 지연 큐잉(`setKeyReleaseHandler`) | `key_handler.js` `_probePrimaryThenProcess` 일대 |
| 읽기 실패 이유 코드(`noowner`·`nomime`·`xfer`·`empty`·`timeout`)·mime 조회 | `classifyPrimaryRead` |
| 판정 — 인접(커서 바로 앞/뒤 정확 일치)·모호 거부·유일 일치(2자 이상) | `primary_selection.js` `decidePrimarySelection`·`decidePrimaryUnique`·`decidePrimary` |
| 키보드 선택 길이 추적·대조(`kbdmismatch`, 상태 `collapsed`/`known`/`unknown`) | `kbdSelStep`·`kbdSelOnPointer`·`kbdSelOnFocus`·`decideKbdMismatch`, `KeyHandler._trackKbdKey`·`_trackPointer`·`_watchImFocus`(포인터 분류 일부는 위 `POINTER_*` 보고로 재사용 가능) |
| 가짜 anchor `SetSurroundingText` 송신 후 Mutter 값으로 복원 | `key_handler.js` 적용 경로 |
| 시험 | `scripts/test-gnome-primary-selection.mjs` |

`_processingKey`/`_keyQueue` 의 **본래 용도**(동기 `ProcessKeyEvent` 재진입 방지, §2.2 Key Queue 패턴)는 유지한다 — 걷는 것은 PRIMARY 읽기 대기에 덧붙인 부분뿐이다.

---

## 3. DBus 통신 (`dbus_ime.js`)

`Gio.DBusProxy`를 사용하여 동기(Sync) 호출 위주로 구현. 입력 경로 호출은 타임아웃
500ms(`DBUS_TIMEOUT_MS`), 연결 수립용 `CreateInputContext` 만 2000ms
(`DBUS_SETUP_TIMEOUT_MS`) — 갓 기동한 데몬은 엔진을 새로 만드느라 500ms 를 넘긴다.

### 3.1 Config 캐시

- 시작 시 **`GetConfigJson`** 1회 호출로 `_configCache`에 전체 설정 스냅샷 로드
- **`ConfigChangedJson`** 시그널(InputMethod iface)로 실시간 갱신
- `getCachedConfig()` / `setOnConfigChanged(cb)` API로 소비자(모드 스위치·AutoTypeFix 옵션 등)가 매 키 입력 DBus 호출 없이 최신 값을 참조
- 단일 진실 공급원은 `~/.config/unim/config.yaml`

### 3.2 주요 DBus 메서드 (Context/Method)

| 메서드 | iface | 역할 |
|--------|-------|------|
| `CreateInputContext(client, windowId)` | InputMethod | 컨텍스트 생성, path 반환 |
| `GetConfigJson` | InputMethod | 전체 설정 JSON 문자열 반환 |
| `TypeFix(direction)` | InputMethod | 글로벌 TypeFIX 실행 (offset, count, replacement) |
| `GetConfig(key)` | InputMethod | 단일 키 조회 (레거시) |
| `ProcessKeyEvent(keyval, keycode, state)` | InputContext | `(consumed, preedit, commit)` 반환 |
| `FocusIn(windowId)` / `FocusOut` | InputContext | 포커스 알림. FocusOut은 조합 중 텍스트 반환 |
| `Reset` | InputContext | 입력 상태 초기화 |
| `SetContentType(purpose)` | InputContext | UNIM ContentPurpose 원시값 통지 → ATF·한영전환 억제(§2.7). `vfunc_focus_out` 에서만 Normal(0) 복귀 |
| `ReportCursorRect(x,y,w,h)` | InputContext | 커서 좌표 보고 |
| `SetSurroundingText(text, cursor, anchor)` | InputContext | 주변 텍스트 전달 |
| `SetClientCapabilities(caps)` | InputContext | (v1.6) 프런트 능력 비트 통지(fire-and-forget, `focusIn()` 끝에서 매번). `caps` = `0x1 REPLACE_SURROUNDING`·`0x2 SELECTION_RELIABLE`·`0x4 POINTER_HINTS`(§2.8) |
| `SurroundingHint(kind, keyval, state)` | InputContext | (v1.6) 대상③ 거부 신호 보고(fire-and-forget): 1 KEY · 2 POINTER_PLAIN · 3 POINTER_SELECT · 4 PRIMARY_CHANGED · 5 SELECTION_UNKNOWN (§2.8) |
| `GetHanjaCandidates` / `SelectHanja(idx)` / `CancelHanja` | InputContext | 한자 변환 |
| `GetSpecialCharCandidates` / `SelectSpecialChar(idx)` / `CancelSpecialChar` | InputContext | 특수문자 변환 |
| `Destroy` | InputContext | 컨텍스트 파괴 |

### 3.3 수신 시그널 — `_handleContextSignal` dispatch

| 시그널 | iface | 처리 경로 |
|--------|-------|-----------|
| `GlobalModeChanged(isKorean)` | InputMethod | 인디케이터 아이콘 갱신 |
| `ConfigChangedJson(jsonStr)` | InputMethod | `_configCache` 갱신 + 콜백 |
| `ShowHanjaPopup(target, candidates, rect)` | InputContext | → `_onShowHanja` (활성화 + 커서) |
| `ShowSpecialPopup(target, characters, topRow, rect)` | InputContext | → `_onShowSpecial` |
| `ShowEmojiPopupV2(catId, items, top_row, recent, cats, rect, home_row)` | InputContext | → `_onShowEmoji` |
| `HidePopup` | InputContext | → `_onHidePopup` |
| `PopupNavigate(page, totalPages, selected, rows, cols, selRow, selCol)` | InputContext | (legacy v3.2 부터 PopupRender 와 dual-emit) → `_onPopupNavigate` (셀/그리드 갱신용) |
| `HanjaBookmarkChanged(index, bookmarked)` | InputContext | → `_onHanjaBookmarkChanged` |
| `HanjaCandidatesReordered(target, hanjas, meanings, bookmarks, newCursor, page, selR, selC, bookmarked, wasBookmarked)` | InputContext | → `_onHanjaCandidatesReordered` (cursor flash 분기) |
| `PopupRender(...)` (v3.2) | InputContext | → `_onPopupRender` — 통합 view_model. 헤더/푸터/탭 라벨/확장 아이콘 등 daemon 산출 문자열 일괄 적용. 자세한 페이로드는 [`docs/dev/specs/POPUP_SPEC.md`](../docs/dev/specs/POPUP_SPEC.md) §10 참조. |
| `AutoTypefixApply(deleteChars, commit, preedit)` | InputContext | 자기 context에서만 `_onAutoTypeFix` 호출. 한자 단어 교체에도 사용(preedit_text="") |
| `ReplaceSurroundingText(deleteBefore, deleteAfter, commit, expect)` (v1.6, `uuss`) | InputContext | 자기 context에서만 `_onReplaceSurrounding` 호출 → `extension.js` 가 `_hasFocus` 게이트 뒤 `replaceSurrounding`. 대상③ 확정(능력 `0x1` 선언 전에는 도달하지 않음, §2.8) |

- InputContext 시그널은 `_icProxy` g-signal과 **세션 버스 글로벌 구독(`signal_subscribe`)** 두 경로를 병용. 자기 context는 proxy 경로, 외부 context(Wayland 전용)는 글로벌 경로로 처리하여 중복 방지.
- `AutoTypefixApply`·`ReplaceSurroundingText`는 proxy introspection 미등록 가능성 때문에 자기 context도 글로벌 경로에서 dispatch(proxy 경로에서는 return, 데몬 세션 재구성 때 두 구독 모두 해제).

### 3.x 데몬 교체·재시작 추종 (NameOwnerChanged)

`unim-daemon --replace` 는 버스 이름 `org.atit.unim.InputMethod` 의 **소유자만**
바꾼다. 한 번 만든 `Gio.DBusProxy` 는 옛 소유자의 고유 이름(`:1.2` 등)에 묶여
있어, 교체 뒤에는 살아 있는 객체처럼 보이면서 호출만 조용히 실패한다. 예전에는
이 상태에서 벗어나는 길이 확장을 껐다 켜는 것뿐이었다.

`dbus_ime.js` 가 `Gio.DBus.watch_name()` 으로 이름을 감시해 스스로 따라간다.

| 사건 | 동작 |
|---|---|
| vanished | 데몬 쪽 프록시·시그널 구독 해제, `isConnected=false`, `onHidePopup` 1회 발행 후 `onLost` |
| appeared | 헐고 다시 짓는다 — IM 프록시 → Config 로드 → `CreateInputContext` → 시그널 재구독, 성공하면 `onReady` |

- **항상 헐고 짓는다**: `--replace` 는 vanished 없이 소유자만 바뀌어 오기도 해서
  이벤트 순서에 기대지 않는다.
- **재시도 백오프** 300·800·2000·4000ms: 이름을 잡은 직후의 데몬은 아직 요청을
  받을 준비가 안 됐을 수 있는데, `NameOwnerChanged` 는 한 번뿐이라 첫 시도가
  실패하면 다시 부를 계기가 없다. 4회 실패하면 다음 소유자 변경까지 기다린다.
- **팝업 정리**: 데몬이 죽으면 `HidePopup` 을 보낼 주체가 없어 팝업이 화면에
  남는다. 렌더러만 접으며 확장은 여전히 팝업 상태를 보유하지 않는다.
- **호출자 몫**: `InputContext` 가 새로 만들어지므로 데몬 쪽에만 있던 상태는
  `setDaemonLifecycleCallbacks({onReady})` 에서 다시 심는다 —
  `extension.js` 가 `RegisterFrontend` 와 (IME 사용 중이면) `FocusIn` 을 재전송한다.
- popup-service 구독은 **별도 서비스**라 데몬 교체 때 건드리지 않는다
  (재구축 시 중복 구독 방지 가드 있음).

---

## 4. Wayland 통합 상세

### 4.1 UnimInputMethod (Clutter.InputMethod 서브클래스)

`GObject.registerClass()`로 등록되어 vfunc이 C vtable에 바인딩됩니다.

| vfunc | 역할 |
|-------|------|
| `vfunc_filter_key_event(event)` | 키 이벤트 가로채기 → self-BS 우회 → KeyHandler 위임 (IBus 패턴, 항상 true) |
| `vfunc_focus_in(focus)` | 포커스 획득 |
| `vfunc_focus_out()` | 포커스 상실 → 팝업 정리 + 조합 커밋 |
| `vfunc_reset()` | 입력 상태 리셋 |
| `vfunc_set_cursor_location(rect)` | 커서 위치 저장 (팝업 배치용) |
| `vfunc_set_surrounding(text, cursor, anchor)` | 주변 텍스트 수신 (TypeFIX용). 한자 선택 변환(대상②)·접두 정합성 검증 용도 |
| `vfunc_update_content_hints(hints)` | 힌트 저장 → `_applyContentPurpose()` 재판정(§2.7) |
| `vfunc_update_content_purpose(purpose)` | Clutter 원시값 저장 → `CLUTTER_PURPOSE_TO_UNIM` 매핑 → `_applyContentPurpose()`(§2.7) |

**공개 API:**
- `commitText(text)`, `updatePreedit(text)`, `clearPreedit()`
- `setActive(active)`, `cursorRect` (getter), `hasFocus()`
- `setKeyHandler(handler)`, `setFocusOutHandler(handler)`, `setResetHandler(handler)`
- **`setContentTypeHandler(handler)`** — content purpose 변경 시 `(unimPurpose) => void` 호출(§2.7)
- **`expectSelfBackspaces(n)`** — AutoTypeFix self-feedback 차단용 카운터 (`n*2`)

### 4.2 플랫폼별 IM 모듈과의 관계

| 대상 | 처리 주체 | 비고 |
|------|-----------|------|
| Wayland 네이티브 앱 | GNOME IM (Backend level) | vfunc이 먼저 키를 소비 |
| X11/XWayland 앱 | GTK/Qt IM 모듈 또는 XIM | GNOME IM은 `captured-event` 폴백 |
| GNOME Shell UI | GNOME IM | 유일한 처리자 |

- `GTK_IM_MODULE=unim` 환경에서 GTK 앱은 자체 IM 모듈이 로드되지만, GNOME IM이 먼저 키를 소비하므로 충돌 없음
- Backend IM 등록 시 `captured-event`에서 자동으로 스킵하여 이중 처리 방지

---

## 5. 설정 (Settings)

### 5.1 GSettings 스키마 (`schemas/org.gnome.shell.extensions.unim.gschema.xml`)

**Phase 8 Settings Cleanup (2026-04)** 이후, 이 스키마는 GNOME Shell API에 직접 의존하는 키만 남긴다. 일반 설정(자판·입력 모드·한자키·AutoTypeFix 등)은 모두 `~/.config/unim/config.yaml`(SSoT)에서 관리하며 `GetConfigJson`/`ConfigChangedJson`으로 동기화된다.

| 키 | 타입 | 설명 |
|----|------|------|
| `enable-extension` | bool | 확장 전체 활성 |
| `show-notification` | bool | TypeFIX 변환 시 `Main.notify()` 알림 |
| `show-panel-indicator` | bool | 상단 패널 한/영 아이콘 표시 |
| `panel-click-action` | enum(`toggle-mode`\|`menu`) | 인디케이터 좌클릭 동작 |
| `enable-ime` | bool | Clutter.InputMethod 기반 실시간 IME (Wayland) |
| `shortcut-normal` | strv | 영→한 변환 단축키 (기본 `<Super>k`) |
| `shortcut-normal-reverse` | strv | 한→영 변환 단축키 (기본 `<Shift><Super>k`) |

총 **7개 키** (enable + 표시 4 + IME 1 + 단축키 2) + 실측 전용 1개. 이전 버전의 18개 키에서 대폭 축소되었다.

**GNOME 전용 GSettings 키 예외 — `surrounding-word-replace`** (bool, 기본 `false`): HANJA_WORD_SPEC §6.5 (10) 실측용 스위치. 켜면 `SetClientCapabilities` 가 `0x5`, 끄면 `0x4`(§2.8). 이 키는 **prefs UI 에 노출하지 않으며**(`gsettings --schemadir <확장 schemas 경로> set …` 로만 바꾼다), **"설정은 엔진·GUI·CLI 3지점 싱크" 원칙의 대상이 아닌 GNOME 전용 GSettings 키 예외**다 — 일반 설정이 아니라 mutter `delete_surrounding` 동작을 실측하는 동안만 쓰는 임시 스위치이고, 실측 결과가 나오면 키를 없애고 기본 동작으로 굳히거나(승인 후) 앱 거름 규칙으로 바꾼다. 설명 문구: "HANJA_WORD_SPEC §6.5 (10) 실측용 — 실측 전에는 켜지 말 것".

### 5.2 `prefs.js` UX

- 첫 페이지 상단의 **"UNIM 설정 앱 열기"** 행이 `Gio.Subprocess`로 `unim-gui-gtk --settings`를 띄운다. 자판·AutoTypeFix·한자키 등 일반 설정은 모두 GTK GUI에서 편집.
- 이어서 표시(indicator/notification/click-action), IME 활성, TypeFIX 단축키만 직접 편집.
- Wayland 세션이 아니면 `enable-ime` 행은 비활성화되고 부제가 안내문으로 바뀐다.
- `unim-gui-gtk` 실행 실패 시 `Adw.Toast`로 폴백 안내.

### 5.3 인디케이터 메뉴

- 헤더: 데몬 연결 상태 / 입력 대기 / 현재 모드 표시
- "한국어 모드", "영어 모드" 선택 (체크 오너먼트)
- "UNIM 설정 (Settings)..." → GTK GUI 실행
- "GNOME 확장 설정 (Extension)..." → `prefs.js`

### 5.4 구 GSettings → config.yaml 마이그레이션

레거시 GSettings 트리에서 `config.yaml`로의 일회성 마이그레이션은 `unim-daemon/src/migration.rs`에서 수행하며, `~/.config/unim/.migrated-v2` 가드 파일로 재실행을 방지한다. extension 자체는 더 이상 일반 설정을 GSettings에 기록하지 않는다.

---

## 6. 알려진 이슈 및 제한사항

### 6.1 IBus와의 충돌

GNOME 설정에서 IBus 입력기가 활성화되어 있으면 키 이벤트 경합이 발생할 수 있다. UNIM 확장을 사용할 때는 IBus를 비활성화하는 것이 좋다.

### 6.2 커서 위치 정확도

`vfunc_set_cursor_location`으로 받는 커서 rect는 앱이 `text-input-v3` 프로토콜로 보고한 값이다. 일부 앱에서는 정확한 위치를 보고하지 않으며, 이 경우 팝업이 화면 중앙에 폴백 배치된다.

---

## 7. 설치 및 빌드

```bash
# 확장 빌드 및 로컬 배포 (개발 전용 — 아래 경고 참고)
make dev-extension

# 시스템 설치
make install-extension

# 로그 확인
journalctl -f -o cat /usr/bin/gnome-shell | grep unim
```

> [!warning] 사용자 사본이 시스템 설치본을 가린다
> `make dev-extension` 은 `~/.local/share/gnome-shell/extensions/<UUID>/` 에 사본을 만든다. GNOME Shell 은 **사용자 디렉터리를 먼저 로드**하므로, 같은 UUID 의 시스템 설치본(deb·`make install`)을 가린다 — 새 버전을 설치해도 옛 코드가 계속 돌아 "증상이 그대로"처럼 보인다(2026-10-04: 0.4.1 사본이 0.4.4 를 가림). 확인: `gnome-extensions info unim-gnome@from104.github.io` 의 경로, 그리고 셸 로그의 `Extension 활성화 시작... version=… path=…` 줄(0.4.4 이상에서 남김). 개발이 끝나면 사본을 지우고(`rm -r ~/.local/share/gnome-shell/extensions/unim-gnome@from104.github.io`) 재로그인한다. `make install-extension` 과 deb `unim-gnome` postinst 는 사본이 있으면 경고한다(삭제는 하지 않음).
