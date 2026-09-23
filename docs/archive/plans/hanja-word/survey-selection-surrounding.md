# 선택 영역·surrounding text 프런트엔드 매트릭스

조사 범위: 한자 단어 입력 기능(대상① 최근 커밋+preedit 결합, 대상② 앱 selection→한자) 설계를 위한
"프런트엔드가 selection/surrounding을 엔진에 얼마나·어떻게 전달하는가" 지도. 읽기 전용 조사, 코드 미수정.

## 1. 핵심 파일과 역할

| 파일 | 역할 |
|---|---|
| `src/input_engine/surrounding.rs` | 엔진의 surrounding_text/cursor/anchor 저장소 + content_purpose 게이트 + 이를 소비하는 `smart_backspace`/`typefix_convert` |
| `src/input_engine/engine.rs` | `InputEngine` 구조체 필드 정의(179-186행), 초기값, `reset()` 이 지우는 필드 목록 |
| `src/input_engine/press_key.rs` | GTK 계열이 타는 `process_key()` 진입점 — 한자키(hanja_keys) idle/조합 분기(POPUP_SPEC v3.2 정책) |
| `src/input_engine/candidates.rs` | `start_hanja_conversion()` — **모든 프런트엔드가 최종적으로 합류하는 단일 진입점**(현재 preedit 마지막 1음절만 대상) |
| `unim-dbus/src/service.rs` | DBus 표면 `org.atit.unim.InputContext` — `SetSurroundingText`, `GetHanjaCandidates`(Standalone pull), `SelectHanja` 등 |
| `unim-dbus/src/engine_worker.rs` | DBus 요청 → `EngineRequest` → 실제 엔진 메서드 호출 배선 |
| `unim-dbus/src/ibus_compat/ibus_context.rs` | `org.freedesktop.IBus.InputContext.SetSurroundingText` 호환 구현 (GNOME 확장 전용 경유로) |
| `unim-frontends/gtk3/src/immodule.c` | GTK3 IM 모듈 — `set_surrounding` vtable만 존재(anchor 없음) |
| `unim-frontends/gtk4/src/immodule.c` | GTK4 IM 모듈 — `set_surrounding_with_selection` vtable 존재(진짜 anchor) |
| `unim-frontends/qt5,qt6/src/input_context.cpp` | Qt IM — `QInputMethodQueryEvent`로 selection/surrounding 질의. 한자키는 `processKey()`를 안 타고 별도 분기 |
| `unim-frontends/wayland/src/state.rs` | `zwp_input_method_v2` 컴포지터측 구현 — `SurroundingText` 이벤트 수신하지만 **미사용** |
| `unim-frontends/xim/src/handler.rs`, `dbus_client.rs` | XIM — surrounding/selection 코드 전무. 한자키는 Qt와 동일한 "Standalone pull" 패턴 |
| `unim-gnome-extension/unim_input_method.js`, `dbus_ime.js` | GNOME Shell Clutter IMImplementation — `vfunc_set_surrounding(text,cursor,anchor)` 직접 지원, 하지만 selection 삭제 래퍼는 없음 |
| `unim-tsf/src/composition.rs` | Windows TSF — `read_selection_text()`(ReadOnly EditSession, GetSelection 기반, 진짜 selection 판별) + `replace_surrounding()`(RW EditSession, 삭제+커밋) |
| `unim-tsf/src/key_handler.rs` | TSF 키 처리 — ATF 수동 트리거에서 `read_selection_text → set_surrounding_text → typefix_convert → replace_surrounding` 전체 파이프라인의 실제 예시 |
| `unim-imm32/src/content_purpose.rs` | IMM32 — content_purpose만 최선노력 구현. surrounding/selection 코드 **없음**(확인: `rg -l -i 'surrounding|selection' unim-imm32/src/*.rs` → content_purpose.rs 1건만, 관련 없음) |
| `docs/dev/specs/POPUP_SPEC.md` | §"idle Hanja 키 dispatch 정책 (v3.2)" — 절대 규칙(변경 시 사용자 승인 필수, memory `feedback_popup_spec_absolute`) |

## 2. 핵심 타입·함수 (file:line · 시그니처 · 역할)

### 엔진 코어
- `src/input_engine/engine.rs:180-186` — `InputEngine` 필드: `content_purpose: ContentPurpose`, `surrounding_text: String`, `surrounding_cursor: u32`, `surrounding_anchor: u32`. 문자 단위(char) 오프셋, 바이트 아님.
- `src/input_engine/surrounding.rs:70` — `pub fn set_surrounding_text(&mut self, text: String, cursor_pos: u32, anchor_pos: u32)`. **유일한 쓰기 경로**. 71-76행: `content_purpose.should_block_hangul()`이면 인자를 버리고 강제로 빈 값 저장(fail-closed, 기존 잔류도 제거).
- `src/input_engine/surrounding.rs:83-89` — `pub fn surrounding_text(&self) -> (&str, u32, u32)` 읽기 전용 접근자. 반환 튜플 `(text, cursor, anchor)`.
- `src/input_engine/surrounding.rs:183` — `pub fn typefix_convert(&mut self, direction: u32) -> Option<(i32, u32, String)>`. 193행: `cursor == anchor`(선택 없음)이면 `None` — **"선택 여부 판별"의 기존 레퍼런스 구현**. 197-199행: `start=min(cursor,anchor)`, `end=max(...)`로 선택 문자열 추출.
- `src/input_engine/candidates.rs:16` — `pub fn start_hanja_conversion(&mut self) -> InputResult`. 23-29행: 대상 결정 로직이 `preedit_cache`의 **마지막 1글자만** 본다 — selection/surrounding_text를 전혀 참조하지 않음. 대상①②를 위해 반드시 손대야 할 지점.
- `src/input_engine/press_key.rs:212-217` — 한자키 분기 진입 전에 `content_purpose.should_block_hangul()`이면 강제 영문 전환(비밀번호 필드 안전장치, 한자키에도 적용됨).
- `src/input_engine/press_key.rs:226-238` — hanja_keys 매칭 시: `idle = preedit_cache.is_empty() && !korean_context.is_composing()` → idle이면 `start_emoji_popup()`, 아니면 `start_hanja_conversion()`. **idle 경로는 selection 유무를 전혀 확인하지 않는다.**

### DBus 표면
- `unim-dbus/src/service.rs:119-124` — `EngineRequest::SetSurroundingText { context_id: u32, text: String, cursor_pos: u32, anchor_pos: u32 }`.
- `unim-dbus/src/service.rs:2725` — `async fn set_surrounding_text(&self, text: &str, cursor_pos: u32, anchor_pos: u32) -> zbus::fdo::Result<()>` — `org.atit.unim.InputContext.SetSurroundingText`. 순수 패스스루, 게이트 없음(게이트는 엔진 내부에만 존재).
- `unim-dbus/src/engine_worker.rs:2270-2277` — `EngineRequest::SetSurroundingText` 처리: `engine.set_surrounding_text(text, cursor_pos, anchor_pos)` 호출.
- `unim-dbus/src/service.rs:2830-2882` — `async fn get_hanja_candidates(...) -> zbus::fdo::Result<(String, Vec<(String,String)>)>` — **Standalone pull**: 응답 조립 중 `ShowHanjaPopup` 시그널 발행(2859-2870행). Qt/XIM 프런트엔드가 이 RPC를 직접 호출.
- `unim-dbus/src/engine_worker.rs:1988-1990` — `GetHanjaCandidates` 핸들러가 `engine.start_hanja_conversion()`을 **직접 호출** — GTK의 `press_key.rs` 경유 호출과 동일한 함수로 합류.
- `unim-dbus/src/ibus_compat/ibus_context.rs:456-475` — `SetSurroundingText(text: Value, cursor_index: u32, anchor_index: u32)` — IBusText에서 문자열 추출 후 동일 `EngineRequest::SetSurroundingText`로 포워드. GNOME 확장이 이 인터페이스 대신 InputContext를 직접 쓰는지는 4장 참고.

### GTK3 (`unim-frontends/gtk3/src/immodule.c`)
- `115행` vtable 선언, `313행` 등록: `im_class->set_surrounding = unim_im_context_set_surrounding;` — **`set_surrounding_with_selection` 없음** (GTK3 `GtkIMContextClass`엔 그 vtable 슬롯 자체가 없다 — GTK 4.2+ 전용 API).
- `1207-1234행` — `unim_im_context_set_surrounding(context, text, len, cursor_index)`. **1220-1222행**: `/* GTK3에서는 anchor 정보가 별도로 없으므로 cursor_index와 동일하게 설정 */ unim->selection_index = cursor_index;` — 매 호출마다 selection_index를 cursor_index로 덮어씀.
- `917-943행` — 매 키 처리 후: `retrieve-surrounding` 재요청(921) → `if (handled && unim->surrounding_text && unim->cursor_index != unim->selection_index)`(923) 이면 선택영역 계산 후 `delete-surrounding` 시그널 발행. **이 조건은 GTK3에서 구조적으로 항상 거짓** — `selection_index`가 `set_surrounding`에서 매번 `cursor_index`로 재설정되므로 두 값이 달라질 방법이 없다(死 코드, GTK4와 코드를 공유하다 남은 흔적으로 추정).

### GTK4 (`unim-frontends/gtk4/src/immodule.c`)
- `104-105행` 선언, `380-381행` 등록: `set_surrounding = ...; set_surrounding_with_selection = ...;` — GTK4는 둘 다 구현.
- `1241-1244행` — 구형 `set_surrounding`은 `unim_im_context_set_surrounding_with_selection(context, text, len, cursor_index, cursor_index)`로 위임(anchor=cursor, 즉 "선택 없음"으로 정규화).
- `1248-1275행` — `unim_im_context_set_surrounding_with_selection(context, text, len, cursor_index, selection_index)`. 1260행: `unim->selection_index = selection_index;`(진짜 anchor). 1268-1274행: 문자 오프셋으로 변환 후 `unim_dbus_set_surrounding_text(unim->dbus_ctx, text, cursor_char, anchor_char)` 호출.
- `1064-1084행` — 위 GTK3와 동일한 "선택 삭제 후 커밋" 래퍼지만, GTK4에서는 `selection_index`가 실제 앵커값을 담을 수 있으므로 **이 경로가 실제로 동작한다**.

### Qt5/Qt6 (`unim-frontends/qt5/src/input_context.cpp`, qt6 동일 라인대)
- `446-463행` — 매 키 처리 후(consumed일 때만): `QInputMethodQueryEvent query(Qt::ImAnchorPosition | Qt::ImCursorPosition)` → `sendEvent(m_focusObject, &query)` → `anchorPos != cursorPos`면 `QInputMethodEvent deleteEvent; deleteEvent.setCommitString("", start-cursorPos, end-start); sendEvent(m_focusObject, &deleteEvent)` — GTK4와 동일한 "선택 삭제 후 커밋" 패턴, Qt는 표준 API로 항상 가능(모든 QTextInterface 지원 위젯).
- `554-564행` — `setFocusObject()` **1회에서만** `QInputMethodQueryEvent(ImSurroundingText|ImCursorPosition|ImAnchorPosition)` 질의 후 `m_dbus->setSurroundingText(text, cursorPos, anchorPos)` 호출. **`update(Qt::InputMethodQueries)`(290-330행 확인)에는 surrounding 갱신 코드가 없다** — 포커스 유지 중 타이핑/커서 이동으로 엔진 쪽 surrounding_text가 갱신되지 않음(아래 4장 리스크).
- `560행` — `if (!surroundingText.isEmpty())`일 때만 DBus 전송 — **새 포커스 필드가 빈 문자열이면 SetSurroundingText 자체를 호출하지 않는다**(이전 필드 값이 엔진에 잔류할 수 있음, 6장 리스크).
- `380-413행` — 한자키(F9/Hangul_Hanja) 처리는 `processKey()`(즉 `press_key.rs`)를 타지 않고, `m_dbus->getHanjaCandidates(target, candidates)`를 **직접** 호출(385행). 후보가 없으면(388-410행) special-char 확인 후 최종 폴백으로만 `m_dbus->processKey(...)`를 호출해 엔진의 idle-Hanja(이모지) 분기를 태운다. **이 F9 분기 안에는 selection 재조회 코드가 없다** — GetHanjaCandidates 호출 시점에 엔진에 캐시된 surrounding_text/anchor는 마지막 포커스 전환 시점 것.

### Wayland (`unim-frontends/wayland/src/state.rs`)
- `626-628행` — `zwp_input_method_v2::Event::SurroundingText { .. } => { // surrounding text 정보 (현재 미사용) }` — **수신은 하되 아무 처리도 안 함**. 엔진으로 전달하는 코드 없음(확인: `rg 'set_surrounding_text|SetSurroundingText' unim-frontends/wayland/src/*.rs` → 매치 0건).
- `248-278행` — `delete_surrounding_text` 사용은 AutoTypeFix 전용, `im.delete_surrounding_text(before_bytes, 0)`(278행) — 커서 **앞**만 삭제(after=0 고정), selection 개념 없음.
- `unim-frontends/wayland/src/dbus_client.rs:139` 주석 "AutoTypeFix 교정 (delete_surrounding_text + 교정 텍스트 커밋)" — 동일하게 ATF 전용.

### XIM (`unim-frontends/xim/src/*.rs`)
- surrounding/selection 관련 코드 **없음**(확인: `find unim-frontends/xim -name '*.rs' -o -name '*.c' | xargs rg -i 'surrounding|selection'` → 무매치, exit 123). X11 core XIM 프로토콜 자체에 표준 surrounding-text 확장이 없고 unim도 비표준 확장을 구현하지 않음.
- `unim-frontends/xim/src/handler.rs:1162-1226` — 한자키 처리는 Qt와 동일한 **Standalone pull** 패턴: `DbusRequest::GetHanjaCandidates` RPC(1171행) 호출 → 후보 있으면 `ShowHanjaPopup`은 데몬이 이미 발행(1184행 주석) → 후보 없으면(1213-1215행) `ProcessKey` 위임으로 idle-Hanja(이모지) 폴백.

### IBus 호환 / GNOME 확장
- `unim-dbus/src/ibus_compat/ibus_context.rs:456-475` — `SetSurroundingText(IBusText, cursor_index, anchor_index)` (freedesktop.IBus.InputContext 인터페이스명 그대로). 이 경로는 **정통 IBus 클라이언트**(im-ibus.so를 쓰는 앱, GNOME Wayland 네이티브 텍스트 위젯이 IBus 프로토콜로 말할 때)를 위한 것으로 추정 — GNOME 확장 자체 경로와는 별개.
- `unim-gnome-extension/unim_input_method.js:562-566` — `vfunc_set_surrounding(text, cursor, anchor)`(Clutter `InputMethod` vfunc, Mutter가 호출) → `this._dbusIME.setSurroundingText(text||'', cursor||'0', anchor||0)`.
- `unim-gnome-extension/dbus_ime.js:646-652` — `setSurroundingText(text, cursor, anchor)` → `GLib.Variant('(suu)', [text, cursor, anchor])`로 `SetSurroundingText` 메서드 호출(대상 인터페이스는 `org.atit.unim.InputContext`로 추정 — `(suu)` 시그니처가 `set_surrounding_text(text:&str, cursor_pos:u32, anchor_pos:u32)`와 정확히 일치, IBus 쪽 `(s,u,u)`이지만 첫 인자가 IBusText 구조체가 아닌 plain string이라 IBus 호환 인터페이스가 아님).
- `unim-gnome-extension/unim_input_method.js:715-721` — `deleteSurrounding(charCount)` → `this.delete_surrounding(-(charCount), charCount)` — **커서 앞만** 삭제(before=charCount, after=0), ATF 전용. **selection을 지우는 코드가 없다** — GTK3/4·Qt에 있는 "선택 있으면 delete 후 commit" 제네릭 래퍼가 GNOME 확장에는 존재하지 않는다(확인: `rg -i 'selection' unim_input_method.js` → 캐시 필드/주석뿐, 삭제 로직 없음).

### Windows TSF (`unim-tsf/src/composition.rs`, `key_handler.rs`)
- `composition.rs:1556-1565` — `pub struct SelectionReadResult { surrounding_text: String, cursor: u32, anchor: u32 }`.
- `composition.rs:1567-1631` — `ReadSelectionEditSession::DoEditSession`: `context.GetSelection(ec, TF_DEFAULT_SELECTION, ...)`(1573) → `sel_range.IsEmpty(ec)`이 참이면(1584) **조용히 반환(None)** — 즉 이 함수는 "선택이 실제로 있을 때만" `Some`을 반환하도록 이미 설계돼 있음. anchor/cursor는 `TF_ANCHOR_START`/`TF_ANCHOR_END`로 각각 collapse한 뒤 `ShiftStart(-4096)` + `GetText`로 "그 지점 앞 글자 수"를 세어 문자 단위 오프셋을 계산(1588-1621행).
- `composition.rs:1637-1656` — `pub fn read_selection_text(context: &ITfContext, tid: u32) -> Option<SelectionReadResult>` — `TF_ES_READ | TF_ES_SYNC` 동기 EditSession으로 즉시 결과 회수. **대상②(선택된 한글 단어 → 한자)에 그대로 재사용 가능한 기존 API.**
- `composition.rs:663-693` — `pub fn replace_surrounding(&mut self, context: &ITfContext, tid: u32, delete_chars: u32, commit_text: &str, preedit_text: &str, comp_sink: &ITfCompositionSink) -> ReplaceOutcome` — `TF_ES_READWRITE | TF_ES_SYNC`. delete_chars는 **커서(선택 끝) 기준 뒤쪽으로**만 삭제(N+1 BS 계열, AutoTypeFix와 동일 메커니즘). `ReplaceOutcome::{Normal, PhaseSplit, SynthBatch, SynthHeadTail}` — 네이티브 조합 vs synth(SendInput BS+UNICODE) 폴백 분기.
- `key_handler.rs:424-456` — **완전한 참조 구현 예시**(수동 AutoTypeFix, Ctrl+Shift+Space): `read_selection_text(context, tid)` → `sel.cursor != sel.anchor`면 `engine.set_surrounding_text(...)` → 무조건 `engine.typefix_convert(0)` 호출 → 있으면 `comp_mgr.replace_surrounding(context, tid, delete_count, &replacement, "", comp_sink)`. 대상②의 Windows 구현이 그대로 베낄 수 있는 패턴.
- `key_handler.rs:417` — `let atf_active = !engine.content_purpose().should_block_hangul();` — **TSF 프런트엔드 자체에서도** 엔진 게이트와 별개로 로컬 게이트를 한 번 더 검사(중복 방어, 엔진 게이트만으로 원칙적으론 충분하지만 TSF는 selection 읽기 자체를 아예 스킵해 COM 호출 비용도 아낌).

### IMM32
- `unim-imm32/src/content_purpose.rs:1-40` — content_purpose 최선노력 감지만 존재. `ES_PASSWORD` 스타일 비트 검사(표준 Edit/RichEdit 계열 한정). surrounding/selection 관련 코드 **없음**(확인: 위 rg 결과).

## 3. 현재 동작 흐름 (단계별, 호출 순서)

### 3-A. Surrounding text 갱신 흐름 (일반, ATF 용도)
1. **GTK3/4**: 매 키 입력 직전(`immodule.c:713`/`862` 부근) `retrieve-surrounding` 시그널 발행 → 위젯이 `set_surrounding[_with_selection]` vtable을 콜백 → `unim_dbus_set_surrounding_text()` → DBus `SetSurroundingText` → `EngineRequest::SetSurroundingText` → `engine.set_surrounding_text()`. **매 키 입력마다 최신값 유지**.
2. **Qt5/6**: `setFocusObject()`(포커스 전환) 시점 1회만 질의·전송. 이후 같은 위젯 안에서 타이핑/커서 이동이 일어나도 재전송 없음(`update()`엔 surrounding 갱신 로직 없음). → 엔진의 surrounding_text는 포커스 전환 순간의 스냅샷으로 **정체(stale)**될 수 있음.
3. **Wayland(zwp_input_method_v2)**: `SurroundingText` 이벤트는 오지만 드롭. 엔진 surrounding_text는 항상 초기값(빈 문자열)으로 남음.
4. **XIM**: 애초에 이벤트/전송 경로 없음.
5. **GNOME 확장**: Clutter `vfunc_set_surrounding`이 호출될 때마다(어떤 빈도로 호출되는지는 Mutter/GTK 내부 구현에 의존 — 코드 확인 불가, 7장 미해결) `setSurroundingText()`로 즉시 전송.
6. **TSF**: 상시 자동 전송 경로 없음 — `read_selection_text()`를 **호출자가 필요할 때마다**(현재는 Ctrl+Shift+Space 수동 ATF 시점) 능동적으로 읽어 `set_surrounding_text()`에 채워 넣는 pull 방식.
7. **IMM32**: 경로 없음.

### 3-B. 한자키 눌림 → 팝업 표시 흐름 (현재, 단일 음절 전용)
- **GTK(inline 아키텍처)**: 키 → `unim_dbus_process_key()`(DBus `ProcessKey`) → `press_key.rs::process_key()` → `hanja_keys.contains(keycode)`(226행) → `idle = preedit_cache.is_empty() && !is_composing()`(232행) → idle이면 `start_emoji_popup()`, 아니면 `start_hanja_conversion()`(237행) → 결과가 `popup_pending_action`에 실려 나가고 unim-popup-service(GTK 레이어셸/X11 오버레이)가 인라인 렌더.
- **Qt/XIM("Standalone" 아키텍처)**: 키를 프런트엔드가 가로채 `processKey()`를 아예 호출하지 않고 `GetHanjaCandidates` RPC(engine_worker.rs:1979)를 직접 호출 → 핸들러가 `engine.start_hanja_conversion()`을 호출(1990행, GTK와 동일 함수) → 후보 있으면 데몬이 `ShowHanjaPopup` 시그널 발행 → **unim-gui-gtk**(별도 프로세스)가 팝업을 그림 → 후보 없으면 프런트엔드가 `ProcessKey`로 폴백해 idle-Hanja(이모지) 분기를 태움.
- 두 아키텍처 모두 **동일한 `start_hanja_conversion()`으로 수렴** — 이 함수를 확장하면 GTK/Qt/XIM 모두에 자동 적용됨(단, GTK의 idle-게이트(`press_key.rs:231-238`)는 별도로 손대야 idle 상태에서도 도달 가능).

### 3-C. 선택 영역 → 삭제 → 커밋 흐름 (기존, ATF/일반 텍스트 대체 목적)
- **GTK4/Qt**: 매 "consumed" 키 결과 처리 시 (a) 최신 surrounding/selection 재조회 → (b) `cursor != anchor`면 즉시 `delete-surrounding`(GTK)/`setCommitString("", offset, len)`(Qt)로 선택 영역 삭제 → (c) `result.commit` 문자열을 commit 신호로 전송. **이 순서(삭제 먼저, 커밋 나중)가 이미 범용 인프라로 존재** — 신규 커밋 텍스트가 무엇이든(한자 단어 포함) 그대로 재사용 가능.
- **GTK3**: 코드는 존재하지만 `selection_index`가 항상 `cursor_index`와 같게 강제되어 (b) 조건이 결코 참이 되지 않음(死 경로).
- **GNOME 확장**: 이 3단계 래퍼 자체가 없음 — `deleteSurrounding(charCount)`는 ATF 코드가 명시적으로 호출할 때만 동작하고, 커밋 자동 선택-교체를 보장하는 범용 로직이 없다(Clutter 텍스트 위젯이 commit 시 자체적으로 selection을 지우는지는 위젯 구현에 달림 — 7장 미해결).
- **Wayland**: 없음.
- **TSF**: 위 3-A 항목의 `key_handler.rs:424-456` 패턴 — `read_selection_text` → (선택 있으면) `set_surrounding_text` → 변환 함수 호출 → `replace_surrounding(delete_chars, commit_text, "")`가 삭제+커밋을 **단일 EditSession**으로 원자 처리.

## 4. 이 기능을 위한 확장 지점 (어디를 어떻게, 위험도)

| # | 확장 지점 | 방법 | 위험도 |
|---|---|---|---|
| 1 | `src/input_engine/candidates.rs:16` `start_hanja_conversion()` | preedit 마지막 1글자 외에 (a) `surrounding_cursor != surrounding_anchor`면 선택 텍스트 우선 검색(대상②), (b) 없으면 `surrounding_text`의 커서 앞 부분 + `preedit_cache`를 이어붙여 단어 후보 검색(대상①). **모든 프런트엔드가 이 함수로 합류하므로 단일 지점 수정으로 GTK/Qt/XIM 3개 아키텍처 전부 커버** | 중 — 기존 단일 음절 동작과 100% 호환되게 "먼저 단어 후보 시도, 없으면 기존 음절 로직 폴백" 순서로 짜야 회귀 없음 |
| 2 | `src/input_engine/press_key.rs:231-238` idle 게이트 | 현재 idle(preedit 없음)이면 무조건 emoji. 대상①(방금 커밋한 "대한민"+조합중 "국")과 대상②(선택 상태, preedit 없어도 selection만 있는 경우)는 **idle 상태에서** 트리거돼야 하므로, idle 판정에 `surrounding_cursor != surrounding_anchor` 및 "직전에 뭔가 커밋했는가"를 추가해야 함 | **높음** — `docs/dev/specs/POPUP_SPEC.md:606-609` "idle Hanja 키 dispatch 정책 (v3.2)"는 절대 규칙(memory `feedback_popup_spec_absolute`: 변경 시 사용자 승인 필수). 이 지점을 건드리는 순간 스펙 문서 갱신 + 명시적 승인 필요 |
| 3 | `InputEngine` 신규 필드: "최근 커밋 이력" | 대상①은 앱이 보고하는 surrounding text에 의존하지 않고 **엔진이 스스로 커밋한 텍스트를 기억**하는 편이 Wayland/XIM/GTK3(selection 불가)에서도 동작해 안전. `commit_buffer`(engine.rs:100)는 매 결과 pull 시 drain되는 휘발성 버퍼라 재사용 불가(확인: engine.rs:855-858 `std::mem::take`) — 신규 `recent_commit_syllables: String`류 필드 필요, `reset()`(engine.rs:766 부근, hanja_target 클리어와 동일 위치)에서 클리어 | 낮음(신규 상태 추가, 기존 로직 무변경) |
| 4 | GTK3 `set_surrounding_with_selection` 부재 | GTK3는 `GtkIMContextClass`에 해당 vtable이 없어(GTK 4.2+ 전용) 코드 추가로 해결 불가 — **대상②는 GTK3 앱에서 구조적으로 불가능**, 대상①(엔진 자체 커밋 이력 방식)로 대체해야 함 | 확인된 프로토콜 한계, 우회 불가 |
| 5 | Qt5/6 `input_context.cpp:380-413` 한자키 분기 | selection 기반 대상②를 태우려면 이 분기 안에서 `QInputMethodQueryEvent(ImAnchorPosition|ImCursorPosition|ImSurroundingText)`로 **즉시 재질의** 후 `setSurroundingText()`로 갱신 → 그 다음에 `getHanjaCandidates()` 호출하도록 순서 추가 필요(현재는 포커스-전환 시점 캐시에만 의존) | 중 — Qt 코드 변경 필요, 회귀 위험은 낮음(새 질의 삽입만) |
| 6 | Wayland `state.rs:626-628` | `zwp_input_method_v2::Event::SurroundingText { text, cursor, anchor }`을 실제로 파싱해 `DbusRequest::SetSurroundingText` 상당 요청으로 엔진에 전달하는 코드를 신규 작성해야 함(현재 아무 배선도 없음) | 높음 — 신규 배선, 문자/바이트 오프셋 변환 주의(프로토콜은 UTF-8 바이트 오프셋) |
| 7 | GNOME 확장 `unim_input_method.js` | 선택 삭제+커밋 제네릭 래퍼(GTK4/Qt에 있는 것)를 신설하거나, 최소한 한자 후보 선택 커밋 시점에 `_lastSurrounding`의 anchor/cursor를 보고 `delete_surrounding` + `commit`을 순서대로 호출하는 전용 경로 추가 | 중 |
| 8 | TSF 한자키 경로(현재 리포트에 미확인 — 별도 서브시스템 담당 추정) | `key_handler.rs:424-456` 패턴을 한자키 브랜치에도 이식: `read_selection_text` → 없으면 기존 로직, 있으면 word 검색 → `SelectHanja` 커밋 시 `replace_surrounding` | 낮음 — 기존 검증된 패턴 재사용 |

## 5. 지켜야 할 규칙 (AGENTS.md·POPUP_SPEC·주석에서 인용, file:line)

- `docs/dev/specs/POPUP_SPEC.md:606-609` — **idle Hanja 키 dispatch 정책(v3.2)**, 원문: "Hanja 키는 `input_category` 와 무관하게 `press_key()` 의 언어 분기 직전에 처리. preedit/조합 idle 이면 emoji popup 트리거, 조합 중이면 한자 변환." → 4장 #2 확장 지점을 건드리려면 이 문서를 갱신하고 **사용자 승인**을 받아야 한다(전역 메모리 `feedback_popup_spec_absolute`).
- `src/input_engine/surrounding.rs:65-69` (주석) — 비밀번호/PIN 필드에서 "빈 값으로 덮어 **기존 잔류까지 제거**" — fail-closed 설계 의도. 신규 코드가 surrounding_text를 다른 경로로 캐싱하면(예: 새 "최근 커밋 이력" 필드) 이 gate를 우회하지 않도록 반드시 같은 규율을 적용해야 한다.
- `unim-dbus/src/service.rs:2816-2826` (주석) — "외부 frontend 는 본 interface 의 popup method 를 직접 호출하지 않는다. popup-service 의 `org.atit.unim.Popup` interface 가 단일 외부 표면" — 단, 현재 코드 실측으로는 Qt/XIM이 `GetHanjaCandidates`를 **직접** 호출하고 있어(qt: `input_context.cpp:385`, xim: `handler.rs:1171`) 이 주석과 실제 동작이 불일치한다(7장 미해결 질문 참고 — 문서/주석이 최신이 아닐 가능성).
- `src/input_engine/engine.rs:759` 부근 주석 — "`content_purpose`/`surrounding_*`/`saved_category` — 앱·포커스 컨텍스트" → `reset()`이 이들을 지우는 조건/타이밍을 그대로 따라야 함(새 "최근 커밋 이력" 필드도 동일 reset 지점에 추가).
- 전역 메모리 `feedback_config_3way_sync` — 새 설정(출력 형식 漢字/한자(漢字)/漢字(한자), 즐겨찾기 등)을 추가할 경우 엔진(`src/config.rs`)·GUI·CLI 3곳 동시 반영 원칙 적용 대상.

## 6. Windows 동등성 메모

- **surrounding/selection 획득**: TSF가 리눅스 GTK4/Qt보다 오히려 더 정확하고 원자적이다 — `GetSelection`이 `IsEmpty()`로 "진짜 선택 있음"을 명시적으로 판별하며(`composition.rs:1584`), pull 방식이라 stale 문제(Qt의 §3-A #2)가 구조적으로 없다(필요한 순간에 항상 재조회).
- **삭제+커밋 원자성**: `replace_surrounding`이 `TF_ES_READWRITE|TF_ES_SYNC` 단일 EditSession으로 삭제·커밋을 묶어, GTK4/Qt처럼 "삭제 시그널 → 커밋 시그널" 2단계로 쪼개지 않는다(중간 상태 노출 없음, 더 안전).
- **격차**: 리눅스는 GTK 계열에서 "매 키 입력 전 자동 갱신"(push에 가까움)인 반면 Windows는 필요 시점에만 능동 조회(pull) — 대상①(최근 커밋+조합 결합)을 Windows에 이식하려면 한자키 브랜치에서 `read_selection_text` 호출을 새로 넣어야 하며, 이는 4장 #8과 동일한 작업.
- **IMM32**: surrounding/selection 관련 인프라가 전무 — 대상①②모두 IMM32 레거시 앱(카톡, 한컴 등 커스텀 컨트롤)에서는 처음부터 지원 불가. `unim-imm32/src/content_purpose.rs` 주석이 이미 이 API 자체의 구조적 한계를 문서화해뒀다(1-33행). Windows 쪽에서 한자 단어 기능은 **TSF 앱에서만** 대상②가 되고, IMM32 앱은 대상①(엔진 자체 커밋 이력, surrounding 불요)만 가능.
- 비밀번호 게이트: TSF는 엔진 게이트(`surrounding.rs:71-76`) 외에 프런트엔드 자체 게이트(`key_handler.rs:417` `atf_active`)를 이중으로 두는 패턴 — 대상①②의 한자키 경로에도 동일한 이중 게이트를 넣는 것이 TSF 관례에 맞다.

## 7. 미해결 질문

1. **GNOME 확장에서 commit이 selection을 자동으로 덮어쓰는가?** `unim_input_method.js`에 범용 삭제 래퍼가 없으므로(4장 #7), Clutter `IBusImplementation`/`InputMethod.commit()`이 위젯 레벨에서 선택 영역을 자동 치환하는지, 아니면 선택 옆에 텍스트가 삽입되는지 코드로 확인 불가 — 런타임(GNOME Wayland, gedit 등) 실측 필요.
2. **`vfunc_set_surrounding`의 호출 빈도**: 매 키 입력마다인지, 위젯 구현에 따라 다른지 — Mutter/Clutter 소스가 이 저장소 밖이라 확인 불가. Qt의 stale 문제(§3-A #2)와 유사한 문제가 GNOME 확장에도 있을 수 있음.
3. **`unim-dbus/src/service.rs:2816-2826` 주석과 Qt/XIM의 `GetHanjaCandidates` 직접 호출 간 불일치**: 주석이 "외부 frontend 는 popup method 직접 호출 안 함"이라 하는데 실측 코드는 Qt/XIM이 직접 호출 중 — 문서가 최신이 아니거나(popup-service가 forward만 하고 최종 호출자는 Qt/XIM 자신이라는 의미로 재해석 필요) 의도된 예외인지 원 작성자 확인 필요.
4. **GTK3용 대상② 완전 배제가 맞는지**: GTK3는 `set_surrounding_with_selection`이 없다는 프로토콜 레벨 한계를 확인했지만, GTK3 앱이 `GtkClipboard`/`PRIMARY` selection(X11 프라이머리 선택)을 통해 우회 조회 가능한지는 이번 조사 범위 밖(별도 X11 API, IM 프로토콜과 무관) — 필요하면 별도 조사 필요.
5. **문자 vs 바이트 오프셋 일관성**: GTK(`g_utf8_pointer_to_offset`로 문자 단위 변환 후 전송), TSF(`chars().count()`로 문자 단위), Qt(`QString` 인덱스 — UTF-16 code unit 단위, 서로게이트 페어 있는 이모지 등에서 문자 단위와 어긋날 수 있음), Wayland 프로토콜 자체는 UTF-8 바이트 오프셋 — 엔진은 전부 "문자(char) 단위"로 가정(`surrounding.rs:111-116` `chars().take(...)`)하는데 Qt가 실제로 문자 단위로 정확히 변환해 보내는지는 이번 조사에서 값 검증까지는 못함(코드상 `toInt()`만 확인, 단위 변환 로직 부재로 보임 — 서로게이트 쌍 포함 텍스트에서 오프셋 오차 가능성, 리스크로 별도 플래그).

## 보충 #4: Reset/FocusOut 송신 빈도 매트릭스 — "대한민" 버퍼가 정상 타이핑 중 계속 비워지는가

조사 목적: 대상①(직전 커밋 음절 + 현재 preedit 결합)을 위해 신설할 "최근 커밋 이력" 버퍼가, 같은 필드 안에서 연속 타이핑하는 동안 Reset 때문에 항상 비어 있게 되지는 않는지 — 전 프런트엔드의 Reset 송신 트리거를 전수 확인.

### 프런트엔드별 Reset 트리거 실측 (file:line 근거)

| 프런트엔드 | Reset RPC를 보내는 지점 | 정상 연속 타이핑(같은 필드, 매 음절 커밋) 중 발동하는가 |
|---|---|---|
| GTK3 | `immodule.c:309` `im_class->reset = unim_im_context_reset` — GTK 코어가 `gtk_im_context_reset()`을 호출할 때만 실행됨(리포 안엔 호출부 없음, GTK 자체가 호출자) | 아니오(로컬 근거) — 정상 커밋 경로는 `press_key` 결과를 `g_signal_emit_by_name(context,"commit",...)`로 직접 발행하며 reset 을 경유하지 않음. reset 은 GTK 위젯이 별도로 호출해야만 실행 |
| GTK4 | `immodule.c:376` 동일 구조(`unim_im_context_reset`, 1161행) | 동일(아니오) |
| Qt5/6 | `input_context.cpp:264-287` — `reset()`/`commit()` 오버라이드 둘 다 `m_dbus->reset()` 호출 | 아니오 — 정상 키 처리 경로(468행 부근)는 `commitString(result.commit)`을 직접 호출, `reset()`/`commit()` 오버라이드를 거치지 않음. 이 두 오버라이드는 Qt 위젯이 포커스 이탈·명시적 커밋 요청 시에만 호출 |
| XIM | `handler.rs:806` `handle_reset_ic` — 클라이언트의 `XResetIC` 프로토콜 요청에만 응답 | 아니오 — 정상 타이핑은 별도 커밋 경로, `ResetIC` 미경유. `handler.rs:1107`의 Reset은 ATF 자기교정(N+1 BS) **완료 시 1회** 내부 발송, 사용자 타이핑 자체와 무관 |
| Wayland | 없음 — `state.rs` 전체에 `DbusRequest::Reset` 송신 코드가 존재하지 않음(rg 전수 확인, `dbus_client.rs:381`은 핸들러만 있고 호출부가 `state.rs` 어디에도 없음) | 구조적으로 불가능 — Wayland 프런트엔드는 Reset RPC 자체를 보내지 않는다. 필드 전환 시 `handle_deactivate`(state.rs:307)→`DbusRequest::FocusOut`(344행)만 1회 |
| GNOME 확장 | `unim_input_method.js:406` `vfunc_reset` — Mutter의 `clutter_input_focus_reset()`이 호출할 때만 실행(js:421-424 주석) | 확인 불가(Mutter/Clutter 소스는 리포 밖) — 단, 유일하게 확인된 실제 트리거는 **커서 점프/클릭**(`vfunc_set_cursor_location`의 cursor-jump 감지, js:510) 경로이지 "매 음절 커밋" 경로가 아님 |
| TSF | `text_service.rs:1868` — 리포 전체에서 실제 `engine.reset()` 호출은 **이 1곳뿐**(`ThreadMgr::OnSetFocus` 핸들러, 1690행, 안) | 아니오, 그리고 real 포커스 전환이어도 조건부 스킵됨 — same-process && dt<250ms(스퓨리어스 전이 포커스) 이면서 `engine.is_composing()`이면 reset 자체를 건너뜀(1843-1849행, Excel 셀 전환 오탐 방지). `OnEndEdit`(1913행)은 edit context 가 read-only 라 `engine.reset()` 호출 불가(1938-1965행 주석) |

### 질문 전제에 대한 두 가지 정정

1. **"engine_worker.rs:723 → engine.rs:764-784 도달"은 부정확.** `reset_engine_and_capture_commit()`(engine_worker.rs:723)은 `engine.reset()`을 호출하지 않는다 — `*engine = InputEngine::new(config)`(776행 부근)로 **엔진을 통째로 재생성**한다. `pub fn reset(&mut self)`(engine.rs:764)이 실제로 호출되는 곳은 `EngineRequest::Reset`/`FocusOut` RPC 경로가 아니라 AutoTypeFix 내부(engine_worker.rs:600 되돌리기, 1562·1629 순방향/역방향 교정)와 각 OS별 자체 리셋 지점(TSF `text_service.rs:1868`·`key_handler.rs:844`·`auto_typefix.rs:455,520`, IMM32 `ime_state.rs:170,193,234`)이다. "대한민" 버퍼는 `InputEngine::new()`(재생성) 시점에는 무조건 사라지고, `engine.reset()`(부분 초기화) 시점에는 그 함수가 지우는 필드 목록에 새 버퍼를 넣을지 뺄지 설계자가 직접 고른다 — 두 초기화 경로가 다른 함수임을 반드시 구분해서 설계해야 한다(4장 #3의 "reset()에서 클리어" 제안은 RPC 재생성 경로엔 적용 안 됨, 별도로 `InputEngine::new()` 기본값도 맞춰야 함).

2. **"웹뷰/Electron 계열이 매 음절 커밋마다 reset을 보낸다"는 이 리포에서 확인되지 않는다.** 메모리에 있는 "웹뷰 클릭 커밋 버그"(project_webview_click_commit_bug)의 실제 코드 근거(`unim_input_method.js:406-440`)를 다시 보면, 트리거는 **클릭에 의한 `vfunc_set_cursor_location` 커서 점프 감지**이지 "정상 타이핑 중 커밋" 이벤트가 아니다(주석 421-424: "clutter_input_focus_reset() 은 COMMIT 모드면 먼저 커밋한 뒤에야 이 vfunc 을 부른다" — 즉 이 vfunc 은 필드 이탈/취소성 조합 종료에 묶인 이벤트지, 연속 타이핑 중의 정상 커밋에 묶인 이벤트가 아니다). GTK3/4·Qt5/6·XIM 세 곳 모두 정상 커밋 경로가 reset RPC를 전혀 거치지 않는다는 로컬 근거(위 표)까지 더하면, 6개 프런트엔드 전수 조사에서 "정상 연속 타이핑 중 매 음절 커밋 직후 Reset이 온다"는 경로는 **하나도 발견되지 않았다**.

### 결론

Reset RPC는 예외 없이 (a) 필드/포커스 이탈(FocusOut, Wayland deactivate, TSF OnSetFocus), (b) 사용자의 명시적 취소(Escape/`XResetIC`), (c) 클릭·커서 점프로 인한 필드 내부 조합 취소(GNOME 확장 vfunc_reset), (d) ATF 자기교정 내부 완료 신호(XIM handler.rs:1107) 중 하나에만 묶여 있다 — "같은 필드 안에서 계속 타이핑"이라는 조건만으로 Reset이 발동하는 프런트엔드는 이번 조사에서 없었다. 따라서 대상①의 "최근 커밋 이력" 버퍼를 `engine.reset()`/`InputEngine::new()` 재생성 양쪽에 걸어 비우는 설계는, 정상 연속 타이핑에서 버퍼가 못 살아남을 것이라는 우려(질문의 전제)를 뒷받침하는 근거가 없다 — 오히려 그 지점들(필드 이탈·클릭·Escape)에서 버퍼가 비는 것은 "대한민국" 같은 어절 조립이 의미상으로도 끊겨야 하는 경우와 일치한다.

### 남은 미해결 (회귀 범위 확정에 직결)

- GTK 코어가 `gtk_im_context_reset()`을 실제로 호출하는 전체 조건 목록(포커스 이동·클릭·Escape 외에 WebKitGTK/특정 툴킷이 더 얹어 부르는 경우가 있는지)은 GTK 소스가 리포 밖이라 이번 조사로 확정 불가 — 실측(Epiphany/WebKitGTK, gedit, 순수 GTK4 텍스트필드에서 연속 타이핑 중 `UNIM_DEBUG("reset 호출")` 로그가 찍히는지) 필요.
- Mutter가 `vfunc_reset`을 호출하는 전체 조건(커서 점프 외에 다른 경로가 있는지)도 동일하게 리포 밖 — 기존 7장 미해결#2와 동일한 실측 필요 항목.
- Qt의 `reset()`/`commit()` 오버라이드가 실제로 어떤 위젯 동작에서 호출되는지(포커스 이탈 외 Enter/프로그래밍적 커밋 등)도 Qt 프레임워크 내부라 리포 안 근거로는 확정 불가.

## 보충 #5: Standalone pull 경로(GetHanjaCandidates)에 press_key 게이트가 미적용 — 프런트엔드 자체 차단 여부·신규 버퍼 노출 위험·명세 개정 필요성

### (1) pull 경로 호출 시점에 프런트엔드 자체 비밀번호 차단이 있는가 — **없음, 실측 확인**

- **Qt5/6** (`unim-frontends/qt5/src/input_context.cpp:380-413`, qt6 동일): `filterEvent()`의 F9/Hangul_Hanja 분기는 `m_contentPurpose`를 전혀 검사하지 않고 곧바로 `m_dbus->getHanjaCandidates()`를 호출한다(385행). `m_contentPurpose`는 `Qt::ImHints` 변화 감지 시(323-336행) `m_dbus->setContentType(purpose)`로 **데몬에 전송만** 되고, Qt 프런트엔드 로컬에서 이 값으로 한자키 분기를 막는 코드는 파일 전체에 없음(확인: `rg -n "contentPurpose" input_context.cpp` 결과 중 385행 앞뒤 어디에도 조건 분기 없음 — 위 4개 결과는 전부 로그 마스킹(`unim_mask`, 430행)과 update()의 mid-focus 캐시 갱신용).
- **XIM** (`unim-frontends/xim/src/handler.rs`): `content_purpose`/`ContentPurpose`/`SetContentType` 어떤 것도 파일에 등장하지 않는다(확인: `rg -n "content_purpose|ContentPurpose|SetContentType" unim-frontends/xim/src/` → 무매치). XIM은 X11 core 프로토콜 자체에 필드 목적(purpose) 통지 수단이 없어 **구조적으로 이 개념이 없다** — 즉 XIM 컨텍스트의 데몬 측 `content_purpose`는 항상 기본값(비-비밀번호)에 머물고, 뒤에서 설명할 daemon 게이트조차 XIM에는 사실상 무의미(트리거될 조건이 애초에 발생하지 않음)하다.
- 결론: **프런트엔드 자체 차단은 Qt·XIM 둘 다 없다.** 유일한 방어선은 daemon(엔진) 쪽인데, 그것도 아래처럼 이 경로엔 없다.

### (2) daemon 쪽 게이트도 pull 경로엔 없음 — 신규 '최근 커밋 버퍼'를 얹으면 무방비 노출

- `unim-dbus/src/engine_worker.rs:1979-2011` `EngineRequest::GetHanjaCandidates` 핸들러: `engine.start_hanja_conversion()`(1990행)을 호출할 뿐 그 앞뒤 어디에도 `content_purpose`/`should_block_hangul()` 검사가 없다(실측: 1960-2011행 전체 재확인, 조건문은 `contexts.get_mut(&context_id)`의 Option 매칭뿐).
- `src/input_engine/candidates.rs:16-60` `start_hanja_conversion()` 자체도 `content_purpose`를 전혀 참조하지 않는다 — 현재는 `self.preedit_cache`(23-29행)만 보므로 비밀번호 필드에서 안전한 이유는 순전히 **간접 효과**: `press_key.rs:212-216`의 `should_block_hangul()` 강제 영문 전환이 애초에 Korean composing 자체를 막아 `preedit_cache`가 비어있게 되는 것뿐이다. 그런데 이 강제 전환도 pull 경로(Qt/XIM `GetHanjaCandidates` 직접 호출)엔 적용되지 않는다 — `press_key()`를 아예 거치지 않기 때문. 즉 현재 안전한 것은 "게이트가 있어서"가 아니라 "preedit_cache가 우연히 비어 있어서"에 가깝다.
- 기존 프로젝트 관례는 **소비 지점이 아니라 저장(setter) 지점에서 fail-closed로 게이트**한다: `surrounding.rs:65-76` `set_surrounding_text()`는 `should_block_hangul()`이면 새 값을 저장하는 대신 **기존 잔류까지 지운다**(71-76행, 71행 `if self.content_purpose.should_block_hangul() { self.surrounding_text.clear(); ... return; }`). 신규 '최근 커밋 버퍼'도 이 패턴을 그대로 따라야 한다 — 버퍼에 append 하는 지점에서 `should_block_hangul()`이면 append 대신 clear.
- 이게 왜 중요한가: 동일 위젯이 비-비밀번호→비밀번호로 **런타임 전환**되는 실제 케이스가 이미 문서화돼 있다 — Qt `input_context.cpp:317-323` 주석 "mid-focus 힌트 변경 반영(예: `QLineEdit::setEchoMode` 런타임 전환)". 이 시나리오에서는 `context_id`(엔진 인스턴스)가 **동일하게 유지**된 채 `content_purpose`만 바뀐다. `surrounding_text`는 이 전환 시점에 즉시 지워지도록(fail-closed) 이미 구현돼 있지만, 신규 '최근 커밋 버퍼'가 같은 지점에서 클리어되도록 짜지 않으면 — 비밀번호 전환 **직전에** 커밋된 평문 음절이 버퍼에 남아있는 채로, 전환 **직후** pull 경로(`GetHanjaCandidates`)가 게이트 없이 `start_hanja_conversion()`을 호출해 그 잔류 버퍼로 단어 후보를 만들어 팝업에 노출할 위험이 있다. `set_content_purpose()`(`surrounding.rs:27-53`)가 `saved_category`를 다루는 지점(38-44행, `should_block_hangul()` 분기)이 이 신규 버퍼를 클리어할 자연스러운 위치.
- XIM은 위 (1)에서 확인했듯 `content_purpose`가 애초에 갱신되지 않으므로, 이 게이트를 아무리 정교하게 만들어도 **XIM 컨텍스트에서는 절대 발동하지 않는다** — XIM에서 최근 커밋 버퍼 기반 대상①을 노출하는 것 자체가 (비밀번호 필드 여부와 무관하게 상시) 프로토콜 레벨 정보 노출 경로가 된다는 뜻. 이는 신규 기능이 만드는 위험이 아니라 XIM의 기존 구조적 공백(이미 `project_atf_password_audit` 계열 감사에서 다뤄진 문제와 동종)이 새 버퍼로 인해 처음으로 "표면화"되는 것.

### (3) 대상②(selection, preedit 없음)를 pull 경로에서 명세 개정 없이 지원 가능한가 — **가능, POPUP_SPEC v3.2 idle 게이트는 이 경로에 애초에 적용된 적이 없다**

- `docs/dev/specs/POPUP_SPEC.md:606-609`의 v3.2 idle 게이트 정책 원문은 스코프를 명시한다: *"Hanja 키는 ... `press_key()` 의 언어 분기 직전에 처리."* 그런데 pull 경로(Qt `input_context.cpp:385` `getHanjaCandidates()`, XIM `handler.rs:1171` `GetHanjaCandidates` RPC)는 **`press_key()`를 호출하지 않는다** — `engine_worker.rs:1990`에서 `engine.start_hanja_conversion()`을 직접 부른다. `press_key.rs:220-238`의 idle 판정(`preedit_cache.is_empty() && !is_composing()` → emoji 트리거)은 pull 경로의 호출 스택에 코드 경로상 존재하지 않는다(실측: `rg -n "fn start_hanja_conversion|process_key\(" unim-dbus/src/engine_worker.rs` 결과 `GetHanjaCandidates` 핸들러 블록 안에 `process_key` 호출 없음).
- POPUP_SPEC.md 자체도 이 두 아키텍처를 별개 절로 이미 구분해 문서화하고 있다 — 697행: *"GetHanjaCandidates / GetSpecialCharCandidates (Standalone 모드): show 시그널 직후 popup_render 발행"*, 232행: 마우스 페이지 이동 기능을 "GTK Standalone / GTK IM modules / Qt IM modules / XIM ..." 등으로 프런트엔드별로 나열 — v3.2 idle 게이트 절은 이 중 GTK inline(`press_key` 경유) 전용 서술이지 Standalone/pull 전체에 대한 서술이 아니다.
- 따라서 **`start_hanja_conversion()`을 확장해 preedit 없이도(즉 idle 상태에서도) selection 텍스트만으로 후보를 찾게 만드는 것은, pull 경로에 한해서는 기존 v3.2 idle-게이트 조항의 문언을 고치거나 위반하지 않고 구현 가능**하다 — 그 조항이 규율하는 대상(`press_key()`) 자체를 건드리지 않기 때문.
- 단, 주의: (a) 이는 **PM이 이미 4장 #1에서 지목한 것과 같은 지점**(`candidates.rs:16` `start_hanja_conversion()`)을 건드리는 것이므로 GTK inline 경로도 **같은 함수를 공유**해 자동으로 영향을 받는다 — GTK inline에서 idle 상태로 pull 경로를 타는 유일한 방법은 여전히 `press_key.rs:231-238`의 idle 게이트를 통과하는 것뿐이므로, GTK 쪽에서 대상②를 idle 상태에 노출하려면 결국 4장 #2(v3.2 게이트 자체 수정, 승인 필요)를 피할 수 없다. Qt/XIM만 우회 가능하고 GTK는 여전히 막혀 있다는 뜻. (b) "명세 개정 불필요"는 기존 v3.2 조항과의 **충돌 회피**를 말하는 것이지, 신규 기능이므로 POPUP_SPEC에 pull 경로의 새 동작을 **추가 서술**하는 일반적 문서화 의무(신규 기능 설계 시 SPEC 갱신 관례)까지 면제되는 것은 아니다 — 이건 "승인 필요한 기존 조항 변경"이 아니라 "새 절 추가"이므로 문턱이 다르다는 차이.

### 결론 요약
1. Qt·XIM 모두 pull 경로 호출 자체에 프런트엔드 로컬 비밀번호 차단 없음(Qt는 `m_contentPurpose` 미검사, XIM은 개념 자체 부재).
2. daemon 쪽도 `GetHanjaCandidates` 핸들러·`start_hanja_conversion()` 둘 다 `content_purpose` 미검사 — 안전은 현재 "preedit_cache가 우연히 비어있음"에 의존. 신규 최근-커밋 버퍼는 `surrounding_text`와 같은 setter-측 fail-closed 클리어(`set_content_purpose()`의 `should_block_hangul()` 분기, `surrounding.rs:38-44` 근처)를 반드시 복제해야 하며, XIM은 이 게이트가 원천적으로 발동하지 않으므로 별도 위험으로 취급해야 함.
3. 대상②의 pull-경로 우선 지원은 POPUP_SPEC v3.2 idle-게이트 조항을 고치지 않고도 가능(그 조항이 규율하는 `press_key()` 자체를 안 건드리므로) — 단, GTK inline까지 idle 노출하려면 결국 그 조항 개정(사용자 승인 필수)이 필요하고, 신규 동작이므로 SPEC에 새 절 추가는 별개로 권장.

## 보충 #9: selection-surrounding — 팝업 마우스 클릭 확정이 앱에 도달하는 경로, 선택-삭제 래퍼가 신호 경로에서도 도는가

**결론: GTK4/Qt/XIM 3개 아키텍처 모두, 팝업 클릭 확정(CommitText DBus 시그널 경로)은 선택-삭제 래퍼를 타지 않는다.** 래퍼는 오직 "키 눌림 → `ProcessKey` 동기 반환값 `result.consumed`" 경로에만 걸려 있고, 클릭 확정은 그 경로를 전혀 지나지 않는 별도의 비동기 시그널 콜백이다. 즉 대상②(선택 영역 존재 상태에서 팝업 후보 클릭)는 **현재 코드상 선택 영역이 자동 치환되지 않고, 커밋 텍스트가 커서 위치(대개 선택 옆/앞)에 그냥 삽입된다** — 질문에서 제기한 우려가 사실로 확인됨.

### 경로 추적 (file:line 근거)

- **GNOME 클릭 → RPC**: `unim-gnome-extension/popup_view.js:394` `btn.connect('clicked', ...)` → `this._rpc.selectHanja(idx)` → `unim-gnome-extension/dbus_ime.js:701-703` `popupSelectHanja(index)` → `_callPopupService('SelectHanja', ...)`.
- **데몬 처리**: `unim-dbus/src/service.rs:2886` `select_hanja()` → 엔진에 `EngineRequest::SelectHanja` 전달 → 응답 문자열(`hanja`)을 받아 `unim-dbus/src/service.rs:2908` `self.redirect_commit_and_hide(&hanja).await` 호출.
- **redirect_commit_and_hide**: `unim-dbus/src/service.rs:1807-1845`. `last_active_input_context_path`(popup-owner) 로 `"org.atit.unim.InputContext"` 인터페이스의 **`CommitText` 시그널**을 발행(1826-1845행) — payload는 **텍스트 문자열 하나뿐**, 삭제 길이·오프셋 등 선택-관련 정보는 전혀 실리지 않는다.
- **Qt5/6 수신**: `unim-frontends/qt6/src/input_context.cpp:222-229` `m_dbus->setCommitTextCallback([this](const QString &text) {...})` → `QInputMethodEvent ev; ev.setCommitString(text); QCoreApplication::sendEvent(focusObj, &ev);` — **`QInputMethodQueryEvent`로 anchor/cursor 재질의도, delete 이벤트도 없음.** 선택-삭제 래퍼는 별도 위치(370-465행 부근, 정확히는 `if (result.consumed) { ... QInputMethodQueryEvent query(...); if (anchorPos != cursorPos) { ...setCommitString("", start-cursorPos, end-start)...} }`)에 있고, 이 블록은 `filterEvent()`의 `m_dbus->processKey(...)` 반환값(`result.consumed`) 안에서만 실행된다 — **키 이벤트 경로 전용**, CommitText 시그널 콜백과는 완전히 분리된 코드 경로.
- **GTK4 수신**: `unim-frontends/gtk4/src/immodule.c:451-459` `on_commit_text(const gchar *text, gpointer user_data)` → `g_signal_emit_by_name(context, "commit", text)` 뿐. 선택-삭제 래퍼(`retrieve-surrounding` 발행 + `cursor_index != selection_index` 검사 + `gtk_im_context_delete_surrounding`)는 1058행 부근 `if (result.consumed) { ... }` 블록 안에 있으며, `result`는 `unim_dbus_process_key()`(동기 `ProcessKey` DBus 호출, 키 눌림 시 필터 함수 안에서만 호출됨)의 반환값 — **`on_commit_text` 콜백에서는 이 블록에 도달할 방법이 없다** (별도 함수, 별도 호출 스택).
- **XIM 수신**: `unim-frontends/xim/src/dbus_client.rs:783` `proxy.receive_commit_text().await`로 구독한 `commit_stream`을 831-838행에서 처리 — `PopupEvent::CommitText { text }`를 채널로 그대로 전달할 뿐, 선택/surrounding 조회나 삭제 호출이 아예 없다. (XIM은애초에 3-A/3-C에서 이미 확인했듯 surrounding-text 인프라 자체가 없으므로, "래퍼가 있는데 이 경로에서만 안 탄다"가 아니라 **애초에 래퍼가 존재하지 않는다** — GTK4/Qt와는 성격이 다른 원천적 결여.)
- **GNOME 자체 커밋도 동일 패턴**: `unim-gnome-extension/extension.js:260-275` `onCommitText` 콜백 → Reset 메아리 필터링 후 `this._inputMethod.commitText(text)` → `unim-gnome-extension/unim_input_method.js:637-651` `commitText(text)`는 preedit 클리어 후 `this.commit(text)`(Clutter.InputMethod 베이스 vfunc) 호출뿐 — GNOME 확장 자체에는 (기존 7장 #7에서 이미 확인했듯) 선택-삭제 범용 래퍼가 애초에 없어 GTK4/Qt와 같은 "경로 분리로 인한 우회"조차 아니고, 구조적으로 처음부터 없다.

### 결론이 대상②(3)에 갖는 의미

- **GTK4·Qt**: 코드 구조상 CommitText 시그널 콜백은 선택-삭제 래퍼를 절대 타지 않는다 → 대상② 구현 시, 선택 영역 자동 치환을 원하면 `on_commit_text`(GTK4)/`setCommitTextCallback` 람다(Qt) **안에** 동일한 선택 조회(`retrieve-surrounding`/`QInputMethodQueryEvent`) + 삭제(`gtk_im_context_delete_surrounding`/`setCommitString("", offset, len)`) 로직을 새로 넣어야 한다(기존 키-경로 래퍼와 나란히 별도 구현 필요, 코드 재사용은 가능하나 자동으로 적용되지 않음).
- **XIM**: 선택 개념 자체가 인프라에 없으므로 대상②는 XIM에서 구조적으로 지원 불가 — 이는 이번 질문과 무관하게 기존에도 참(추가 확인일 뿐, 새로운 제약 아님).
- **GNOME**: 여전히 미해결(기존 7장 open question #1과 동일) — `commit()` 신호를 받은 Mutter/Clutter 위젯이 자체적으로 선택을 치환하는지는 이 저장소 밖(Mutter 소스) 문제라 코드로 확정 불가. 단, unim 확장 자체는 어떤 삭제도 하지 않는다는 점은 이번 조사로 재확인됨.

### 미해결/리스크
- GNOME의 실제 런타임 동작(Mutter가 commit 시 선택을 자동 치환하는지)은 여전히 실측 필요 — 기존 미해결 질문 #1과 동일 이슈, 이번 조사로 답이 나오지 않음.
- Qt/GTK4에 새 선택-삭제 로직을 CommitText 콜백에 추가할 경우, 키-경로 래퍼와 값 계산 로직이 중복되므로 공용 헬퍼로 뽑아내는 리팩터링이 필요할 수 있음(현재는 각 경로에 인라인).
