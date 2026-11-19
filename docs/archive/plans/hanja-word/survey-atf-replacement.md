# AutoTypeFix 교체 메커니즘·최근 커밋 버퍼

## 1. 핵심 파일과 역할

| 파일 | 역할 |
|---|---|
| `src/auto_typefix/buffer.rs` | `KeystrokeBuffer` — **키스트로크(keycode+modifier) 시퀀스**를 보관. 완성된 한글 텍스트 자체는 저장하지 않는다. `committed_chars`(카운트만), `has_preedit`, `word_mode` 플래그 동반. |
| `src/auto_typefix/forward.rs` | 영어모드에서 타이핑된 keycode를 한글로 조합 시뮬레이션 → 완성 음절 수 임계치 이상이면 오타 교정 트리거 (순방향: 영타→한글 오타 자동수정). |
| `src/auto_typefix/reverse.rs` | 한글모드에서 타이핑된 keycode를 영문으로 복원 → 사전 매칭되면 트리거 (역방향: 한타→영어 오타 자동수정). |
| `src/auto_typefix/dictionary.rs` | `count_korean_syllables`(완성형 음절만 카운트), `dictionary_contains`(내장 영어사전 조회). |
| `src/auto_typefix/mod.rs` | `AutoTypeFixResult` 공개 타입 정의 + 모듈 재노출. |
| `src/input_engine/surrounding.rs` | ATF 버퍼와는 **별도 경로**. `content_purpose`(비밀번호 게이트) + `surrounding_text`/`cursor`/`anchor`(앱이 push 하는 문서 컨텍스트) + `smart_backspace`(미배선) + `typefix_convert`(**선택 영역** 기반 수동 한/영 변환 — Ctrl+Shift+Space). |
| `src/input_engine/candidates.rs` | 기존 "한자 변환"(음절 단위) — `start_hanja_conversion`/`select_hanja`/`cancel_hanja`/북마크 3종. **preedit 마지막 음절만** 대상으로 삼는다(커밋된 텍스트 대상 경로는 주석상 "None"으로 미구현). |
| `src/input_engine/engine.rs` | `InputEngine` 필드 정의 — `hanja_*`, `content_purpose`, `surrounding_text`, `saved_category`, `commit_buffer`(1키 단위 임시 flush 버퍼, 최근-커밋 히스토리 아님). |
| `unim-dbus/src/engine_worker.rs` | Linux 데몬의 컨텍스트별 오케스트레이션. `keystroke_buffers: HashMap<context_id, KeystrokeBuffer>` 소유, 매 키 이벤트마다 `check_forward`/`check_reverse` 호출 + 결과 적용(BackSpace/커밋/preedit) + 버퍼 리셋 조건 전부 여기 있음. |
| `unim-tsf/src/auto_typefix.rs` | Windows TSF 포팅. `AutoTypeFixState`(단일 인스턴스, HashMap 없음) — buf/undo/recent_corrections/blacklist/user_dict 소유. `process_after_key`가 engine_worker의 per-context 오케스트레이션과 동형. |
| `unim-tsf/src/composition.rs` | TSF `replace_surrounding`(확정문 삭제+교체) + `end_composition_with_text`/`SetText` 계열(라이브 조합 SetText 치환, D1 예외). |
| `unim-frontends/xim/src/handler.rs` | X11 XIM 서버. **N+1 self-injected BackSpace** 트릭으로 delete_surrounding 프로토콜 부재를 우회. |
| `unim-frontends/wayland/src/state.rs` | text-input-v3 네이티브 `delete_surrounding_text` 사용 — 트릭 불필요. |
| `unim-frontends/gtk3,4/src/immodule.c` | `gtk_im_context_delete_surrounding` + 실패시 XTest BackSpace 폴백(Electron 등). `surrounding-text-set`/`retrieve-surrounding` 시그널에서 `cursor_index`+`selection_index`(=anchor) 를 DBus로 전달 — **대상② 선택 텍스트 API가 이미 존재**. |
| `unim-dbus/src/ibus_compat/ibus_context.rs` | IBus 호환 D-Bus 프로토콜 어댑터(GTK 앱과 통신하는 실제 저수준 계층). `set_surrounding_text`(line 457), `forward_key_event`(545). |

## 2. 핵심 타입·함수 (file:line · 시그니처 · 역할)

### `AutoTypeFixResult` — `src/auto_typefix/mod.rs:41-64`
```rust
pub struct AutoTypeFixResult {
    pub delete_chars: u32,          // 삭제할 화면 글자 수
    pub commit_text: String,        // commit할 텍스트(마지막 음절 제외)
    pub corrected: String,          // 전체 교정 텍스트(undo용)
    pub original: String,           // 원래 텍스트(undo용)
    pub clear_preedit: bool,
    pub replay_keys: Vec<(KeyCode, ModifierState)>,  // 순방향: 마지막 음절 재조합용 키 재생
    pub replace_composition: bool,  // word_mode(라이브 조합)면 true → SetText 치환 경로
}
```
- **삭제/교체는 3가지 필드 조합으로 표현된다**: `delete_chars`(몇 글자 지울지) + `commit_text`(그 자리에 넣을 확정 텍스트) + `replay_keys`(순방향만, 마지막 음절을 preedit으로 재생성). "backspace 개수"라는 별도 필드는 없다 — 프런트가 `delete_chars`를 backspace 횟수 또는 `delete_surrounding(-(N),N)` 인자로 변환한다.
- `replace_composition`: `word_mode`(어절 단위 조합 모드)일 때만 true. true면 프런트는 "확정문 삭제" 대신 "**진행 중 조합 자체를 SetText로 치환**"한다(TSF 전용 분기, Linux는 이 필드를 아직 소비하지 않음 — 근거는 §7).

### `check_forward` — `src/auto_typefix/forward.rs:16`
```rust
pub fn check_forward(buffer: &KeystrokeBuffer, config: &AutoTypeFixConfig,
    korean_layout: &str, english_layout: &str, blacklist: &dyn BlacklistGate)
    -> Option<AutoTypeFixResult>
```
- 조건: `config.forward && buffer.len() >= 2`, blacklist 미억제, (옵션)영어사전 미매치, `count_korean_syllables(converted) >= threshold`, 마지막 글자 제외 전부 완성 음절.
- 마지막 음절 분리 알고리즘(forward.rs:70-121): `converted`의 앞 n-1글자(`target_prefix`)와 정확히 일치하는 가장 큰 partial-ascii 접두사 인덱스 `i`를 뒤에서부터 탐색 → `commit_text`=앞부분, `replay_keys`=그 이후 keycode들(엔진에 재입력해 마지막 음절을 preedit으로 재생).

### `check_reverse` — `src/auto_typefix/reverse.rs:18`
```rust
pub fn check_reverse(buffer: &KeystrokeBuffer, config: &AutoTypeFixConfig,
    korean_layout: &str, english_layout: &str, blacklist: &dyn BlacklistGate,
    user_dict: &dyn UserDictGate) -> Option<AutoTypeFixResult>
```
- `screen_chars = buffer.committed_chars + (has_preedit ? 1 : 0)` 를 `delete_chars`로 사용 — **음절 수 카운트일 뿐 실제 텍스트가 아니다**(§6 신뢰도 문제와 직결).
- `replay_keys`는 항상 빈 벡터(영어로 확정하므로 preedit 불필요).

### `KeystrokeBuffer` — `src/auto_typefix/buffer.rs:20-30`
```rust
pub struct KeystrokeBuffer {
    entries: VecDeque<KeystrokeEntry>,   // keycode+modifier+timestamp, 실제 문자 아님
    pub committed_chars: usize,          // 카운트만, 텍스트 없음
    pub has_preedit: bool,
    pub word_mode: bool,
}
```
주요 메서드:
- `push(keycode, modifier) -> bool` (buffer.rs:51) — 비문자 키(Enter/BS 등)와 `Space`는 `false` 반환하며 **버퍼에 안 들어감**(공백=단어 구분자로 취급하되 clear는 하지 않음).
- `clear()` (buffer.rs:69) — entries/committed_chars/has_preedit/word_mode 전부 초기화.
- `pop_last()` (buffer.rs:84) — TSF 보유영문 Backspace 동기 축소 전용.
- `expire(time_window_ms)` (buffer.rs:103) — 시간 윈도우 밖 오래된 entry FIFO 제거. **이것이 사실상의 "자동 리셋" 메커니즘**(명시적 clear가 없어도 오래되면 사라짐).
- `to_ascii_string(english_layout)` (buffer.rs:116) — keycode → 지정 레이아웃 기준 ASCII 변환.
- `update_on_commit(&str)` / `update_on_preedit(&str)` (buffer.rs:130,135) — **문자열 내용은 버리고** 길이/유무만 상태에 반영.

### `InputEngine::start_hanja_conversion` — `src/input_engine/candidates.rs:16-108`
- target 결정: `!preedit_cache.is_empty()` 이면 preedit 마지막 문자 1개, **아니면 `None`**(candidates.rs:24-28 — "커밋 버퍼의 마지막 음절 (이미 입력된 경우)"라고 주석은 달려 있으나 실제로 `None` 하드코딩, 즉 **이미 커밋된 텍스트 기반 한자 변환은 구현되어 있지 않다**).
- 한자 후보 없으면 초성 기준 특수문자 검색 폴백(choseong).
- 결과: `hanja_target`/`hanja_candidates`/`hanja_mode`/`popup_state`(9칸 그리드) 세팅, `PopupAction::ShowHanja` 발행.

### `InputEngine::select_hanja` — `candidates.rs:140-164`
```rust
pub fn select_hanja(&mut self, index: usize) -> Option<String>
```
- preedit 마지막 음절 제거(`korean_context.clear()`, `preedit_cache.clear()`) 후 `cancel_hanja()`. **commit_buffer에 넣지 않는다** — 호출자(D-Bus 응답)가 직접 커밋 문자열로 반환·사용. 이는 프런트가 자체적으로 delete+commit을 수행해야 함을 뜻한다(대체 입력 로직이 프런트 쪽에 있음 — 아래 §3).

### `InputEngine::toggle_hanja_bookmark` — `candidates.rs:186-266`
```rust
pub fn toggle_hanja_bookmark(&mut self, index: usize) -> Option<(usize, bool, bool)>
```
- `HanjaBookmarkStore` 토글 → 후보 재정렬(북마크 우선 stable sort) → `PopupAction::HanjaCandidatesReordered` 발행(대상, 후보, 북마크, 새 커서, 페이지/행/열, 새 상태, 이전 상태). **한자 단어 팝업의 즐겨찾기 요구사항이 그대로 재사용 가능한 기존 패턴**.

### `InputEngine::set_surrounding_text` — `src/input_engine/surrounding.rs:70-80`
```rust
pub fn set_surrounding_text(&mut self, text: String, cursor_pos: u32, anchor_pos: u32)
```
- `content_purpose.should_block_hangul()`(Password/Pin)이면 **저장 자체를 거부하고 기존 값도 비운다**(fail-closed).
- **Pull 방식**: 앱이 GTK `retrieve_surrounding`/IBus `SetSurroundingText`/XIM 프로토콜/TSF `ITfContextView`로 밀어줄 때만 갱신됨. 매 키스트로크마다 자동 갱신되지 않는다(§6).

### `InputEngine::typefix_convert` — `surrounding.rs:183-272`
```rust
pub fn typefix_convert(&mut self, direction: u32) -> Option<(i32, u32, String)>
```
- **선택 영역(cursor != anchor)에만 동작**, 반환 `(offset_from_cursor, delete_chars, replacement)`. 이는 **대상② "선택된 한글 단어" 처리의 기존 레퍼런스 구현**이다(한/영 변환 대신 한자 변환으로 바꾸면 거의 동일 인터페이스 재사용 가능).

## 3. 현재 동작 흐름 (단계별, 호출 순서)

### 3.1 Linux 데몬(`unim-dbus/src/engine_worker.rs`) — 매 키 이벤트
1. `ProcessKeyEvent` 수신 → `keystroke_buffers.get_or_insert(context_id)`로 해당 컨텍스트 `KeystrokeBuffer` 획득 (engine_worker.rs:1345 부근).
2. `buf.push(key, modifier)`가 `true`면(문자 키 & Space 아님) → `buf.expire(window_ms)`(방향별 forward/reverse 시간창 별도) (engine_worker.rs:1350-1360).
3. 엔진의 `commit`/`preedit` 결과를 `buf.update_on_commit`/`update_on_preedit`로 반영(engine_worker.rs:1378-1391) — **문자열이 아니라 길이/유무만 갱신**.
4. `buf.word_mode = engine.is_word_mode()` 매 키 직전 동기화(engine_worker.rs:1391).
5. 현재 입력 카테고리에 따라 `check_forward`(English) 또는 `check_reverse`(Korean) 호출(engine_worker.rs:1394-1417).
6. 재트리거(rollback) 감지 게이트 통과 시에만 `fix`를 확정, 아니면 `buf.clear()` 하고 blacklist에 tentative 등록(engine_worker.rs:1420-1461).
7. `fix`가 있으면 프런트별 적용 이벤트 발행(BackSpace 개수·commit·preedit) 후 `buf.clear()`(engine_worker.rs:1543-1689 부근, 정확 라인은 방향별로 분기).
8. `FocusOut`/`CreateContext`(신규)/`FocusIn` 시 **`keystroke_buffers.remove(&context_id)`**로 컨텍스트째 폐기(engine_worker.rs:593, 642, 1900).
9. `SetContentType`이 비밀번호/PIN 진입을 판정하면 `keystroke_buffers.remove` + `undo_states.remove` + `recent_corrections.remove`(engine_worker.rs:2248-2257).

### 3.2 Windows TSF(`unim-tsf/src/auto_typefix.rs` + `key_handler.rs`)
1. `key_handler.rs:552` `observe_backspace` — 사용자가 직접 Backspace를 치면 undo 후보 관찰.
2. `key_handler.rs:572` `observe_mode_switch` — 한/영 전환 시 `state.buf.clear()`(auto_typefix.rs:221).
3. `key_handler.rs:771` `process_after_key` 호출은 **`engine.reset()` 이전**에 이루어진다(주석 명시) — 코어 `check_forward/check_reverse`가 리셋 전 상태를 봐야 하므로 순서가 고정.
4. `try_undo`(auto_typefix.rs:174) — Ctrl+Z 계열, `state.buf.clear()`(187) 동반.
5. TSF 프런트 적용은 `composition.rs`의 `replace_surrounding`(확정문 삭제, 663) 또는 `replace_composition=true`면 `end_composition_with_text`/`SetText` 계열로 분기.

### 3.3 프런트별 "삭제+교체" 실행 상세

**GTK3/4 IM 모듈** (`unim-frontends/gtk{3,4}/src/immodule.c`, `on_auto_typefix`):
- `gtk_im_context_delete_surrounding(context, -(delete_chars), delete_chars)` 우선 시도(gtk4/immodule.c:483).
- 실패(Electron 등 미지원 앱)하면 X11이면 `XTestFakeKeyEvent`로 실제 BackSpace 키를 delete_chars회 주입하고 지연 commit 상태(`autofix_bs_pending`/`autofix_commit_text`/`autofix_preedit_text`)를 저장 → 다음 `filter_keypress`에서 처리(gtk4/immodule.c:488-519). Non-X11이면 `\b` 문자 자체를 commit 시그널로 흘려보내는 최후 폴백(517).
- 성공/폴백 후 `commit_text` 커밋 → `preedit_text` 설정(529-533).
- `surrounding-text-set` 시그널(`unim_im_context_set_surrounding_with_selection`, gtk4/immodule.c:1247-1276)이 `cursor_index`뿐 아니라 **`selection_index`(=anchor)**도 함께 D-Bus로 전달 — 대상② 선택-단어 API가 GTK 계층에는 이미 있다.

**XIM** (`unim-frontends/xim/src/handler.rs`) — **N+1 self-injected BackSpace**:
- XIM은 `delete_surrounding` 표준 프로토콜이 사실상 무용(비협조 클라이언트 다수)이라, `XTestFakeKeyEvent`로 **진짜 하드웨어 BackSpace 이벤트**를 `delete_chars + 1`회 주입한다(handler.rs:547,556-573).
- **왜 +1인가**: 주입한 BS가 X 서버를 거쳐 XIM 서버 자신에게도 `ForwardEvent`로 재진입한다. 앞의 `delete_chars`개는 `Ok(false)`로 그대로 앱에 통과시켜 "실제로 글자 삭제"를 앱이 수행하게 만들고, **마지막 N+1번째**는 `Ok(true)`로 소비해 "앱이 삭제를 다 끝냈다"는 신호로 삼아 그 시점에 진짜 `commit`+`preedit`을 보낸다(handler.rs:1052-1116). 즉 카운터(`self_backspace_pending`)가 0이 되는 순간이 "delete 완료 확인" 트리거다.
- **preedit 잔존 이슈**: commit 전송 후 곧바로 preedit을 보내면 Chrome 등 일부 XIM 클라이언트가 commit 처리 중 preedit을 초기화해버려 유실된다 → commit flush 후 **10ms sleep** 뒤 preedit 전송(handler.rs:1086-1099). BS 주입 사이에도 앱이 각 BS를 순차 처리할 시간을 벌기 위해 매 BS마다 flush+10ms sleep(handler.rs:569-571) — 이 전체가 타이밍에 의존하는 휴리스틱이며 완전한 프로토콜 보장이 아니다.
- `autofix_commit_guard` 재진입 가드(handler.rs:1040) — XIM crate가 `server.commit()`/`preedit_draw()` 내부에서 keycode=0 가상 이벤트를 다시 `handle_forward_event`로 넣기 때문에 이를 걸러냄.

**Wayland** (`unim-frontends/wayland/src/state.rs:248-294`, `apply_auto_typefix`):
- text-input-v3 네이티브 `im.delete_surrounding_text(before_bytes, 0)` — 프로토콜 표준 지원, 트릭 불필요.
- 단 바이트 수 계산이 휴리스틱: `commit_text`의 첫 글자가 한글 완성형/자모 범위면 "순방향"으로 간주해 `delete_chars`(ASCII, 1byte/char)를, 아니면 "역방향"으로 간주해 `delete_chars*3`(한글 UTF-8 3byte 고정 가정)을 삭제 바이트 수로 사용(state.rs:255-268). **한글이 항상 3바이트라는 가정**이며 조합형/특수 유니코드 확장 완성자모 등 예외 미고려.

**IBus 호환(D-Bus)** (`unim-dbus/src/ibus_compat/ibus_context.rs`) — GTK/Qt(ibus 백엔드) 앱과의 저수준 D-Bus 프로토콜 어댑터. `set_surrounding_text`(457)/`forward_key_event`(545)로 앱 쪽 IBus 클라이언트와 통신.

**TSF** (`unim-tsf/src/composition.rs`) — `replace_surrounding`(663)이 확정 텍스트 삭제+교체의 메인 경로. `replace_composition=true`(word 모드)일 때는 `end_composition_with_text`(528)/`SetText`(range.SetText, 970/1007/1041) 로 **삭제 없이 조합 텍스트 자체를 치환**한다 — Word 등 confident-text 삭제를 차단하는 앱에서도 동작(§5 D1 인용).
- `unim-imm32`(레거시 IMM32 — TSF 아닌 구식 API)에는 ATF/surrounding 연동이 **없음**(확인: `rg AutoTypeFix|auto_typefix|delete_surrounding|SetText unim-imm32` 무매치) — Windows의 실제 ATF 교체는 TSF 전용.

## 4. 이 기능을 위한 확장 지점 (어디를 어떻게, 위험도)

### 4.1 대상① "최근 커밋 + 현재 preedit" 결합 — **신규 버퍼 필요, 위험도: 중**
- `KeystrokeBuffer`/`commit_buffer`는 재사용 불가: 전자는 keycode만, 후자는 매 키 이벤트 후 drain되는 1회성 임시 버퍼(engine.rs:827-828,855-858)라 "최근 커밋 음절 히스토리"가 아니다.
- **새 상태 필요**: 예) `InputEngine`에 `recent_commit_text: String`(또는 음절 `VecDeque<char>`) 필드 추가, `commit_buffer`를 실제로 flush하는 모든 지점(engine.rs 755-770 `commit()`류, press_key.rs의 각 `commit_buffer.push*` 직후)에서 병행 append. 캡이 필요(예: 최근 N음절, 어절 경계까지).
- **리셋 조건은 ATF 버퍼와 다르게(더 좁게) 설계해야 한다** — ATF는 "오타locality"만 보므로 시간창 expire로 충분하지만, 한자 어절 결합은 "같은 어절"이어야 하므로 **공백/문장부호/Enter/모드전환/포커스전환/커서이동(Reset)/비밀번호진입 전부에서 즉시 클리어**해야 의미적으로 옳다. 현재 ATF 버퍼는 이 중 다수를 클리어하지 않는다(§6).
- 한자키 처리부(engine_worker.rs GetHanjaCandidates 분기, candidates.rs:16-108)에 "preedit 비어있고 recent_commit_text 비어있지 않으면 recent_commit_text + 이번 키로 완성된 마지막 preedit 음절을 합쳐 사전 검색" 분기를 추가.

### 4.2 대상② "선택된 한글 단어" — **기존 인프라 재사용 가능, 위험도: 낮음**
- `typefix_convert`(surrounding.rs:183)와 거의 동일한 패턴으로 `hanja_word_convert(&self) -> Option<(String /*word*/, Vec<HanjaEntry>)>` 류 신설 — `surrounding_text[start..end]`를 사전에 조회.
- GTK immodule은 이미 `selection_index`를 D-Bus로 보낸다(gtk4/immodule.c:1247). XIM/Wayland/TSF도 선택 텍스트 획득 API가 있는지는 프런트별 SPEC.md 확인 필요(미조사 — §7).
- 팝업 오픈 트리거(한자키)가 눌렸을 때 "preedit 있음 → 기존 음절 경로", "preedit 없고 selection 있음 → 신규 선택 단어 경로", "preedit 없고 selection 없고 recent_commit_text 있음 → 신규 어절결합 경로" 3분기로 `start_hanja_conversion`을 확장.

### 4.3 대체 입력 실행 — **거의 그대로 재사용 가능, 위험도: 낮음**
- `AutoTypeFixResult`의 `delete_chars`+`commit_text` 패턴, 프런트별 적용 경로(GTK `delete_surrounding`+XTest 폴백, XIM N+1 BS, Wayland `delete_surrounding_text`, TSF `replace_surrounding`/SetText)를 **그대로** "한자 단어 교체"에 재사용 가능 — 새 `PopupAction`/D-Bus 시그널(예: `ReplaceHanjaWord { delete_chars, commit_text }`)만 추가하고 프런트의 기존 `on_auto_typefix`류 핸들러를 호출하거나 그 핸들러를 공용 함수로 리팩터링.
- **word 모드 조합 SetText 치환(`replace_composition`)** 은 TSF·Linux(engine_worker.rs) 양쪽에 이미 있다(§6). 대상①에서 "조합 중인 국"이 있는 상태로 팝업이 뜨는 경우, 이 기존 Phase A2 패턴(all_keys 재생 → 전체 단어를 라이브 조합 preedit으로 재구성)을 참고해 확장하면 신규 구현량을 줄일 수 있음 — 단 word 모드 전제 조건(§7-2)을 먼저 확인할 것.

### 4.4 비밀번호/민감 필드 게이트 — **재사용 필수, 위험도: 미대응 시 높음(정보 유실 리스크)**
- `content_purpose.should_block_hangul()` 게이트가 `set_surrounding_text`(surrounding.rs:71)와 TSF `clear_sensitive`(auto_typefix.rs:112)에 이미 있음. **신규 `recent_commit_text` 버퍼도 이 게이트를 반드시 통과**시켜야 한다 — 비밀번호 필드 진입 시 즉시 clear, 진입 전 잔류도 제거(surrounding.rs의 fail-closed 패턴 그대로 복제).

## 5. 지켜야 할 규칙 (AGENTS.md·POPUP_SPEC·주석에서 인용, file:line)

- **팝업 페이지 크기 9 고정** — `docs/dev/specs/POPUP_SPEC.md:166` "페이지 크기 | **9** | 한 페이지에 표시할 후보 수 (숫자키 1~9 대응)", `:315` "v3.2 rows=9 고정 정책". 요구사항 "한 페이지 9단어"와 정확히 일치 — 새 규격을 만들 필요 없이 기존 hanja 팝업 그리드를 그대로 확장.
- **즐겨찾기 토글 시그널 계약** — `docs/dev/specs/POPUP_SPEC.md:118` `HanjaCandidatesReordered (s target, as hanjas, as meanings, ab bookmarks, u new_cursor, u page, u sel_row, u sel_col, b bookmarked, b was_bookmarked)`, `:247-248` 해제 시 140ms `#f9e2af` flash. 단어 단위로 확장 시 `target`이 음절이 아니라 어절 문자열이 되는 점만 다르고 시그널 구조는 그대로.
- **한자 팝업 즐겨찾기 우선정렬은 stable sort** — `src/input_engine/candidates.rs:33-34,186-266` 주석 "dict 에서 빈도순을 매번 다시 받아 정렬 기반을 리셋" — 토글마다 원본 순서 재조회 후 재정렬(캐시된 정렬 결과를 그대로 토글하면 안 됨).
- **비밀번호 필드 fail-closed** — `src/input_engine/surrounding.rs:65-69` "필드를 벗어나면(비-비밀번호 목적)... 이탈 신호가 늦어도 비번 평문은 잔류하지 않는다" — 최근-커밋 버퍼도 동일 원칙 적용 필수(사용자 CLAUDE.md 3번째 최우선 지침 "정보 유실 항상 유념"과 직결).
- **replace_composition 무회귀 불변식** — `src/auto_typefix/mod.rs:58-62` "`false`(음절 모드/committed 섞임)면 기존 삭제 경로와 바이트 동일 — 무회귀 불변식." 신규 기능이 이 필드의 의미를 변경하거나 재활용할 경우 기존 ATF 경로에 회귀가 없는지 반드시 확인.
- **AGENTS.md 경로 리다이렉트** — 루트 `AGENTS.md`(6줄)는 `docs/dev/architecture/AGENTS.md`로 리다이렉트된 stub. 실제 규칙은 후자에 있음(`docs/dev/architecture/AGENTS.md:230` "모델 사용 지침", `:205` AutoTypeFix 억제 사전 설명, `:275` POPUP_SPEC.md 링크).
- **설정 3지점 동기 원칙**(세션 메모리 `feedback_config_3way_sync`) — 한자 단어 기능에 신규 설정(출력 형식 漢字/한자(漢字)/漢字(한자), 활성화 여부 등)을 추가하면 `src/config.rs` + `unim-gui-gtk`/`unim-settings-gtk` + CLI `ConfigKey` 3곳을 항상 함께 갱신해야 함 — 이번 서브시스템 조사 범위 밖이지만 설계자에게 필수 주지사항.

## 6. Windows 동등성 메모

- **TSF만 ATF 대체 입력을 지원**한다. `unim-imm32`(레거시 IMM32 API 경로)에는 AutoTypeFix/surrounding-text 연동이 전혀 없음(확인: `rg "AutoTypeFix|auto_typefix|delete_surrounding|SetText" unim-imm32` → 무매치). 한자 단어 기능을 Windows에 얹으려면 TSF 경로(`unim-tsf/src/auto_typefix.rs`, `composition.rs`)에만 배선하면 되고, IMM32 레거시 경로는 대상 밖으로 봐야 한다(설계자 확인 필요 — §7).
- TSF는 `AutoTypeFixState`가 **단일 인스턴스**(HashMap 없음, `unim-tsf/src/auto_typefix.rs:67-76`)라 Linux의 `HashMap<context_id, KeystrokeBuffer>`와 구조가 다르다 — 대상①의 "최근 커밋 버퍼"를 InputEngine 필드로 넣으면(§4.1 제안대로) 이 차이는 자연히 흡수된다(엔진 자체가 컨텍스트 단위이므로).
- TSF는 `replace_composition=true`(word 모드) 분기를 이미 활용 중(SetText로 조합 자체 치환, D1 예외 — Word 등 확정문 삭제 차단 앱 대응). **정정**: Linux(`unim-dbus/src/engine_worker.rs`)도 이 필드를 소비한다 — `effective_reverse_delete`(engine_worker.rs:801-807, "word 라이브 조합은 문서 확정 0자이므로 delete_surrounding 을 0으로 강제하지 않으면 무관한 문서 텍스트를 비가역 삭제")와, 순방향 word 모드에서 `all_keys`(버퍼 전체 키스트로크, 마지막 음절만 담는 `fix.replay_keys`와 별도)를 재생해 전체 단어를 하나의 라이브 조합(preedit)으로 재구성하는 로직(engine_worker.rs:1580-1600, "Phase A2")이 있다. 즉 대상①의 "조합 중(국)+최근커밋(대한민)" 결합 시나리오와 매우 유사한 기존 패턴이 이미 존재 — 재사용 가능성 높음(위험도 낮음으로 하향).
- TSF의 `process_after_key`는 **`engine.reset()` 이전**에 호출된다(`unim-tsf/src/key_handler.rs:767` 주석) — 한자 단어 변환 훅을 TSF에 추가할 때도 이 순서(reset 전 코어 조회)를 지켜야 한다.

## 7. 미해결 질문

1. **XIM/Qt 프런트가 selection(anchor) 정보를 D-Bus로 전달하는지 확인 안 됨** — GTK3/4는 `selection_index`를 확인했으나(§3.3), `unim-frontends/xim`·`unim-frontends/qt5,qt6`·TSF의 `ITfContextView::GetSelection` 연동 여부는 이번 조사 범위에서 미확인. 대상②(선택 단어) 구현 전 프런트별 SPEC.md 재확인 필요.
2. ~~`replace_composition` 필드를 Linux 프런트가 읽는지~~ — **해결됨**: `unim-dbus/src/engine_worker.rs`가 소비한다(`effective_reverse_delete` 801-807, Phase A2 word 라이브 조합 재구성 1580-1600). 단 이 로직을 발동시키는 `engine.is_word_mode()`(어절 단위 조합 모드)가 대상①의 "음절 확정 모드"(기현님 원문 예시: "대한민"이 이미 커밋되고 "국"이 조합 중)와 같은 세팅인지, 즉 **음절 모드에서도 이 결합이 자연히 되는지 아니면 word 모드 전환이 선행되어야 하는지**는 미확인 — 설계 시 `is_word_mode()`/`commit_unit` 설정(`config.engine.korean` 계열)과의 관계를 별도 확인 필요.
3. **`EngineRequest::Reset`(마우스 클릭·GTK IM reset 유발)이 `keystroke_buffers`를 클리어하지 않는 것으로 보임** — `unim-dbus/src/engine_worker.rs:1923-1936` `Reset` 처리 블록에 `keystroke_buffers` 언급이 없다. "커서 이동·클릭" 리셋 조건이 ATF 버퍼 차원에서는 실제로 걸려있지 않을 가능성 — 신규 recent-commit 버퍼는 이 gap을 그대로 물려받지 않도록 별도로 Reset 훅에 clear를 추가해야 한다(설계자 확인 필요).
4. **Enter/Space가 ATF 버퍼를 clear하지 않는다는 점의 실제 사용자 체감 영향** — `KeystrokeBuffer::push`가 Space/비문자키를 그냥 무시(false 반환)할 뿐이라, 그 시점에 `check_forward/check_reverse` 자체가 호출 안 되어 버퍼가 갱신되지 않고 남는다. 시간창(`expire`) 밖으로 벗어나야 자연 소멸. 한자 어절 결합 버퍼는 이 동작을 그대로 쓰면 "3분 전 입력한 단어"가 여전히 최근-커밋으로 잡힐 위험이 있어, §4.1에서 제안한 대로 **의미론적 경계(공백/구두점/모드전환/Enter)마다 즉시 clear**하는 별도 로직이 필요하다는 결론이지만, 최종 UX(예: 공백 후에도 결합 허용할지)는 기획 확정 필요.
5. **config reload가 keystroke_buffers를 건드리지 않음**(engine_worker.rs:968-1046 근방 — `config.reload_if_changed()` 이후 `apply_korean_rebuild`만 호출, `keystroke_buffers` 미언급) — 요구사항의 "리로드" 리셋 조건이 ATF 버퍼 차원에서 실제로 구현돼 있는지 재확인 필요.
6. **한자→어절 사전 자체의 다음절 항목 커버리지** — 이번 조사는 ATF/버퍼 서브시스템 한정이라 `src/hanja`/사전 데이터가 "대한민국" 같은 4음절 단어를 이미 갖고 있는지는 확인하지 않았다(세션 메모리 `project_future_backlog`에 "한자 단어변환이 최저비용(사전에 다음절 27.5만 이미 존재)"이라는 기존 조사 결과가 있어 참고할 것 — 단 본 문서 범위 밖).

## 보충 #1: AutoTypefixApply 시그널을 한자 단어 교체 채널로 그대로 재사용 가능한가

### (1) 각 프런트엔드가 `preedit_text=""` + 임의 `commit_text`(한자/괄호 혼합)를 순수 "delete→commit"으로만 처리하는가

| 프런트 | 근거(file:line) | 결론 |
|---|---|---|
| GTK4 | `unim-frontends/gtk4/src/immodule.c:463-533` `on_auto_typefix` | `delete_surrounding`→XTest BS→`\b` 3단 폴백 후 `g_signal_emit_by_name(context,"commit",commit_text)`(text 내용에 대한 가정 없음, 임의 문자열 그대로 커밋 가능) → `preedit_text`가 비면 `unim_emit_preedit(unim,"")`으로 **명시적 clear**(529-533). 부작용 없음 — **그대로 재사용 가능**. |
| GTK3 | `unim-frontends/gtk3/src/immodule.c:396-462` | GTK4와 동형(코드 구조 동일). 재사용 가능. |
| Qt5 | `unim-frontends/qt5/src/input_context.cpp:177-220` | 터미널(className에 "TerminalDisplay") 이면 `\b`×N prefix + commitText를 하나의 commit string으로(콘솔 우회, 192-198), 아니면 `ev.setCommitString(commitText, -(deleteChars), deleteChars)`(Qt 네이티브 원자적 교체, 210) — 둘 다 `commitText` 내용 무관. `preeditText.isEmpty()`이면 `m_composing=false`만 세팅(로컬 UI 플래그, 219) — 엔진 호출 없음. **재사용 가능**. |
| XIM | `unim-frontends/xim/src/handler.rs:1071-1107` | N+1 self-BS 완료 후 `server.commit(commit_text)`(1074) 하는 것까지는 동일하나, **`!has_preedit`(=preedit_text=="") 분기에서 `DbusRequest::Reset{context_path}`를 엔진에 발행한다(1104-1107, "역방향: 엔진 Reset" 주석과 일치)**. 이는 다른 3개 프런트엔드에는 없는 **추가 부작용**이다. 한자 단어 교체는 항상 `preedit_text=""`이므로 이 채널을 그대로 쓰면 **매 교체마다 엔진 Reset RPC가 함께 발행**된다. Reset이 무엇을 지우는지(§7-3 미해결 질문의 `keystroke_buffers` 비언급과 별개로, 신규 §4.1 `recent_commit_text` 버퍼를 만들 경우 그 버퍼까지 Reset이 지우는지)는 engine_worker.rs의 `Reset` 처리 블록(`:1923-1936` 부근, 미조사)에서 별도 확인 필요 — **재사용 가능하나 조건부**(Reset 부작용을 설계에 반영하거나 회피해야 함). |
| Wayland | `unim-frontends/wayland/src/state.rs:248-294` `apply_auto_typefix` | `is_forward` 판정(255-262)이 `commit_text`의 **첫 글자가 한글 완성형/자모 범위인지**로 삭제 바이트 수(1byte vs 3byte)를 정한다. 한자 단어 교체의 `commit_text`는 한자(예: "大韓民國")로 시작 → 한글 범위 아님 → `is_forward=false` → `delete_chars*3`바이트 삭제. 삭제 대상이 실제로는 커밋된 **한글** 텍스트("대한민국", 3byte/char)이므로 **결과적으로 바이트 수는 우연히 맞는다** — 그러나 이는 "commit이 ASCII면 역방향"이라는 원래 전제가 깨진 상태에서 나온 우연의 일치이며, 향후 이 휴리스미틱을 수정하면 조용히 깨질 수 있는 **취약한 재사용**이다(명시적 byte-length 파라미터 추가를 권고). preedit 처리(281-288)는 내용 무관, `set_preedit_string`/no-op만 — 엔진 호출 없음. |

결론: GTK3/4·Qt5·Wayland 4개는 **수정 없이 안전하게 재사용 가능**. XIM만 "reverse-ATF용 Reset" 부작용이 딸려 있어 **그대로 쓰면 안 되고**, (a) Reset이 신규 recent-commit 버퍼/한자 상태를 건드리지 않음을 확인하거나 (b) `preedit_text`를 항상 빈 문자열로 두지 않는 시그널 페이로드 설계(예: 별도 플래그 추가)로 우회해야 한다. 이는 "7개 지도 전부가 신규 시그널을 전제"한 위험도 평가와 달리 **XIM 한 곳만 실제 차이가 있다**는 뜻 — 작업량 축소 방향은 맞으나 "위험 없음"은 아니다.

### (2) `redirect_commit_and_hide` 패턴을 `AutoTypefixApply`에도 그대로 적용 가능한가, GNOME의 `isOwnContext` 필터가 owner-redirect를 버리는가

- **D-Bus 시그널 정의**: `auto_typefix_apply(delete_chars: u32, commit_text: &str, preedit_text: &str)`는 `org.atit.unim.InputContext` **per-context 인터페이스**의 시그널(`unim-dbus/src/service.rs:2801-2809`, 필드 원천은 `EngineResponse.auto_typefix`(267) → `service.rs:2155-2166`에서 `&signal_ctx`(자기 path)로 발행). D-Bus wire 상 멤버명은 `AutoTypefixApply`(zbus snake→PascalCase 변환, 실사용측 확인: `dbus_ime.js:267,278,382`, `gtk-common/unim_dbus_client.c:1202`, `qt-common/unim_dbus_client.cpp:572`).
- **redirect_commit_and_hide** (`service.rs:1807-1862`)는 `signal_ctx`(자기 context 고정) 대신 `self.connection.emit_signal(None, &target_path, "org.atit.unim.InputContext", "CommitText", &(text,))`(1826-1836)와 `"HidePopup"`(1846-1853)을 **임의의 `target_path`**(=`last_active_input_context_path`, popup-owner의 실제 object path)로 직접 발행한다 — `zbus::Connection::emit_signal`은 신호 대상 path를 자유롭게 지정할 수 있으므로, 동일한 방식으로 `redirect_auto_typefix_apply(&self, delete_chars, commit_text, preedit_text)`를 추가해 `self.connection.emit_signal(None, &owner_path, "org.atit.unim.InputContext", "AutoTypefixApply", &(delete_chars, commit_text, preedit_text))`를 발행하는 것은 **프로토콜적으로 동일 패턴이며 막힘 없음** — 신규 시그널 불필요, 신규 헬퍼 함수 하나만 추가.
- **각 프런트의 구독은 전부 path-scoped**: XIM/Wayland의 zbus `InputContextProxy`는 `.path(obj_path)`로 특정 object path에 바인딩되어 생성된다(`unim-frontends/wayland/src/dbus_client.rs:613-614`, `unim-frontends/xim/src/dbus_client.rs:309-310` 등 동일 패턴 반복) — 표준 D-Bus 매치 규칙상 emit한 path와 proxy의 path가 일치해야 콜백이 온다. `owner_path`가 곧 그 앱의 실제 context path이므로 정상 수신된다.
- **GNOME `dbus_ime.js`의 이중 처리**: (a) 자기 context용 `_icProxy`의 `g-signal` 구독은 `AutoTypefixApply`를 명시적으로 스킵(`:267` "글로벌 구독에서만 처리, 중복 방지"). (b) 별도로 **path 필터 없는** 버스-레벨 `bus.signal_subscribe(UNIM_BUS_NAME, UNIM_IC_INTERFACE, 'AutoTypefixApply', null, null, ...)`(`:272-278`)를 걸어두고, 콜백에서 `const isOwn = (path === this._contextPath)`(`:285`)로 **수신된 시그널의 실제 object path와 자기 자신의 context path를 단순 문자열 비교**한다. 이 필터는 "이 시그널이 내 RPC 호출에서 비롯됐는가"가 아니라 "이 시그널의 대상 path가 나인가"만 본다. 따라서:
  - popup-owner가 **GNOME 자신의 shadow context가 아닌 다른 프런트**(GTK4_IM 등)일 때 → `owner_path ≠ this._contextPath` → GNOME은 정확히 무시(의도된 동작, 그 프런트가 대신 처리).
  - popup-owner가 **GNOME 자신**(순수 Wayland 네이티브 앱을 GNOME 확장이 직접 입력 처리하는 케이스)일 때 → `owner_path === this._contextPath` → `isOwn=true` → `_handleContextSignal('AutoTypefixApply', params, true)` → `this._onAutoTypeFix(...)` 정상 호출(`dbus_ime.js:382-385`).
  - 즉 **owner-redirect 발행을 버리지 않는다** — `redirect_commit_and_hide`가 이미 `CommitText`/`HidePopup`에 대해 이 경로로 정상 동작 중이므로(같은 인터페이스, 같은 필터), `AutoTypefixApply`도 동일하게 통과한다.

결론: (2)는 **그대로 확장 가능** — 신규 D-Bus 시그널·신규 PopupAction 없이 `redirect_commit_and_hide`와 나란히 `redirect_auto_typefix_apply` 헬퍼 하나만 `service.rs`에 추가하면 마우스 클릭(SelectHanja RPC) 경로의 "커밋 텍스트 교체"를 owner path로 정확히 전달할 수 있다. 단 (1)에서 확인한 XIM의 Reset 부작용은 이 redirect 경로를 타도 그대로 적용되므로 별개로 처리해야 한다.

## 보충 #6: Wayland 삭제 바이트 수 — '한글 3바이트 가정'이 출력 형식 설정과 충돌하는가

**질문**: `unim-frontends/wayland/src/state.rs:252-268`(정확 위치는 아래 참조)의 `is_forward` 휴리스틱이 '한자(漢字)' 출력 형식(한글이 먼저 옴)에서 오분류를 일으키는가, 그리고 이 프런트엔드가 정확한 삭제 바이트 수를 알 방법이 있는가. GNOME `deleteSurrounding`/GTK `delete_surrounding`이 문자 단위라 무관한지도 확인.

**1. 휴리스틱 재확인 (`state.rs:248-297` `apply_auto_typefix`)**
- 판정 로직은 249-263행: `commit_text`의 첫 글자가 한글 완성형(`AC00..=D7A3`) 또는 자모(`3131..=318E`)면 `is_forward=true` → `before_bytes = delete_chars`(1B/글자, ASCII 가정), 아니면 `is_forward=false` → `before_bytes = delete_chars * 3`(3B/글자, 한글 가정).
- **출력 형식 '한자(漢字)'로 확정 요청하신 대로 확인**: 이 형식의 `commit_text`는 "대한민국(大韓民國)"처럼 **한글로 시작**한다 → `is_forward=true`로 오판정 → `before_bytes = delete_chars`(1B/글자)로 계산됨. 그러나 실제로 지워야 할 대상은 **이미 커밋된 한글**("대한민국", 3B/글자, UTF-8)이므로 필요한 삭제 바이트 수는 `delete_chars * 3`이다. **1B로 계산된 값을 `delete_surrounding_text`에 넘기면 실제 필요량의 1/3만 삭제되어 문서에 지워지지 않은 한글 잔여 바이트가 섞인 채 새 커밋이 이어붙는다** — 질문이 제기한 대로 데이터 파괴(문서 깨짐)가 실제로 재현되는 경로다. (반대로 '漢字' 단독 형식은 commit이 한자로 시작 → `is_forward=false`로 오판정되지만 삭제 대상도 한글 3B/글자이므로 §7-3 원문이 지적한 "우연히 맞는" 케이스, '漢字(한자)'도 commit 시작이 한자이므로 동일하게 우연히 맞음. **'한자(漢字)' 형식만 유일하게 실제로 깨진다.**)
- 즉 이 휴리스틱은 "commit_text가 무엇으로 시작하는가"만 보고 "삭제 대상이 무엇인가"를 추론하는데, 한자 단어 교체는 AutoTypeFix의 순방향/역방향 2분법(교정 언어가 곧 삭제 대상 언어의 반대)을 벗어난 **제3의 케이스**(삭제 대상=한글 고정, commit=설정에 따라 한글/한자/혼합 임의)라서 근본적으로 안 맞는다.

**2. 정확한 바이트 수를 알 방법 — 있다. `SurroundingText` 이벤트를 드롭하지 않으면 된다**
- 프로토콜 정의(`~/.cargo/registry/src/index.crates.io-*/wayland-protocols-misc-0.3.10/protocols/input-method-unstable-v2.xml:111-146`, `surrounding_text` 이벤트): 인자는 `text: string`, `cursor: uint`, `anchor: uint`이며 명시적으로 **"cursor is the byte offset of the cursor within the text buffer"**(라인 124), **"anchor is the byte offset ... within the text buffer"**(126-127)라고 규정한다. 즉 컴포지터가 매번 커서 주변 텍스트 전체와 그 안에서의 **바이트 오프셋**을 보내준다.
- `delete_surrounding_text` 요청도 바이트 단위로 문서화되어 있다(같은 xml `:262-263`, "before_length and after_length are **the number of bytes** ... to delete").
- 그런데 현재 코드는 이 이벤트를 **완전히 버리고 있다**: `unim-frontends/wayland/src/state.rs:626-628`
  ```rust
  zwp_input_method_v2::Event::SurroundingText { .. } => {
      // surrounding text 정보 (현재 미사용)
  }
  ```
  `{ .. }`로 `text`/`cursor`/`anchor` 필드를 전부 무시하고 있어, State에 surrounding text 상태가 전혀 없다.
- **해결 경로**: `State`에 `surrounding_text: String`, `surrounding_cursor: u32`(둘 다 이 이벤트에서 갱신, `TextChangeCause`/`Done`의 더블버퍼 규약에 맞춰 `done` 처리 시점에 적용)를 추가해 매 이벤트마다 최신값을 저장하면, `apply_auto_typefix`/한자 교체 시 `delete_chars`(문자 수)를 `surrounding_text[..surrounding_cursor]`의 **뒤에서부터 문자 단위로 `delete_chars`개를 슬라이스**해 그 슬라이스의 `.len()`(바이트)을 그대로 `before_bytes`로 쓰면 **추측이 아니라 실측**이 된다 — 형식이 한글이든 한자든 혼합이든 무관하게 정확하다.
  - 단, 프로토콜 스펙(xml `:130-134`, "If this event does not arrive before the first done event, the input method may assume that the text input does not support this functionality")에 따라 **일부 앱은 surrounding_text를 아예 지원하지 않을 수 있다** — 이 경우 폴백으로 현재의 char→byte 추정 휴리스틱(단, '한자(漢字)' 형식 전용 분기: "삭제 대상은 항상 한글" 이라는 사실을 이미 알고 있으므로 `commit_text` 내용을 보지 말고 **호출부(엔진)가 "삭제 대상이 한글 N글자"라는 사실 자체를 명시적 파라미터로 넘기는 것**이 근본 수정이다 — `is_forward`로 "추론"하지 말고 delete 대상 언어를 시그널 자체에 실어야 한다는 뜻)을 유지해야 한다.
- **결론(핵심)**: (a) `zwp_input_method_v2::SurroundingText`는 이미 정확한 바이트 오프셋을 실어 보내주고 있으므로 이걸 살려 쓰면 추측이 필요 없다. (b) 그와 별개로, 더 근본적인 수정은 D-Bus `AutoTypefixApply`/신규 한자 교체 시그널 자체에 `delete_chars`(문자 수)뿐 아니라 **"삭제 대상 텍스트가 어떤 스크립트인지"를 나타내는 명시적 플래그(또는 바로 바이트 수 자체)**를 실어, 프런트엔드가 `commit_text`의 첫 글자를 보고 추론하는 현재 방식 자체를 없애는 것 — 이는 atf-replacement 원문이 이미 "명시적 byte-length 파라미터 추가를 권고"라고 남긴 취약점 항목과 정확히 같은 결론이며, 이번 조사로 그 취약점이 '한자(漢字)' 형식에서 **이론이 아니라 실제로 터진다**는 것이 확인됐다.

**3. GNOME `deleteSurrounding`/GTK `delete_surrounding`은 무관한가 — 그렇다, 문자 단위라 이 버그와 별개**
- GNOME 확장(`unim-gnome-extension/unim_input_method.js:711-723`): `deleteSurrounding(charCount)`는 `this.delete_surrounding(-(charCount), charCount)`(718행)를 호출한다. 이는 Clutter `InputMethod`/`Gio`류 API로, 시그니처 자체가 `charCount`(글자 수)를 그대로 offset/length로 사용 — **문자 단위**다.
- GTK3/4 (`unim-frontends/gtk3/src/immodule.c:415`, `unim-frontends/gtk4/src/immodule.c:483`): `gtk_im_context_delete_surrounding(context, offset, n_chars)` — GTK API 자체가 offset·length를 **문자(character) 단위**로 정의한다(GTK 공식 문서 규약).
- 따라서 이 둘은 UTF-8 바이트 수를 몰라도 되고, 애초에 이번 문제의 "1B vs 3B" 오분류가 존재할 여지가 없다 — **질문대로 이 버그와 무관**하다. 바이트 단위 추정이 필요한 프런트엔드는 Wayland(`delete_surrounding_text`, 바이트)와 XIM(자체 N+1 BS 방식, 별도 조사 필요하나 이번 질문 범위 밖) 뿐이다.
