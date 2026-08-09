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
| `key_handler.js` | 키 이벤트 필터링·분류·라우팅, call_sync 재진입용 **key queue** |
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
| 한자키 (F9, Hangul_Hanja) | 엔진이 팝업 시그널 발사 → GNOME 팝업 표시 |
| BackSpace | DBus `ProcessKeyEvent` → 자모 삭제 (self-sent BackSpace는 선제 우회) |
| KEY_RELEASE | IM 미처리, `notify_key_event(false)`만 호출 (키 반복 유지) |

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
  1. `expectSelfBackspaces(deleteChars)` — `UnimInputMethod`의 self-backspace 카운터 등록
  2. `vkbd.backspaceMultiple(deleteChars)` — 가상 키보드로 BackSpace 연타
  3. 50ms 후 `commitText(commitText)`, 다시 10ms 후 `updatePreedit(preeditText)`
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

---

## 3. DBus 통신 (`dbus_ime.js`)

`Gio.DBusProxy`를 사용하여 동기(Sync) 호출 위주로 구현. 타임아웃 500ms.

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
| `AutoTypefixApply(deleteChars, commit, preedit)` | InputContext | 자기 context에서만 `_onAutoTypeFix` 호출 |

- InputContext 시그널은 `_icProxy` g-signal과 **세션 버스 글로벌 구독(`signal_subscribe`)** 두 경로를 병용. 자기 context는 proxy 경로, 외부 context(Wayland 전용)는 글로벌 경로로 처리하여 중복 방지.
- `AutoTypefixApply`는 proxy introspection 미등록 가능성 때문에 자기 context도 글로벌 경로에서 dispatch.

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
| `vfunc_set_surrounding(text, cursor, anchor)` | 주변 텍스트 수신 (TypeFIX용) |
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

총 **7개 키** (enable + 표시 4 + IME 1 + 단축키 2). 이전 버전의 18개 키에서 대폭 축소되었다.

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
# 확장 빌드 및 로컬 배포
make dev-extension

# 시스템 설치
make install-extension

# 로그 확인
journalctl -f -o cat /usr/bin/gnome-shell | grep unim
```
