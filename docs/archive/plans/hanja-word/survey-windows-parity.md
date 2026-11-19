# Windows(TSF/IMM32) 한자·팝업·교체 동등성

조사 범위: `unim-tsf/`, `unim-imm32/`, `unim-popup-win/`, `unim-capi/`, 관련 코어(`src/`) 경계.
Read-only 조사. 모든 근거는 file:line.

## 1. 핵심 파일과 역할

| 파일 | 역할 |
|---|---|
| `src/input_engine/candidates.rs` | 한자/특수문자 변환 시작(`start_hanja_conversion`)·선택(`select_hanja`)·즐겨찾기. **플랫폼 공통** — Windows/Linux 둘 다 이 코드를 그대로 씀. |
| `src/hanja/dict.rs` | 한자 사전(`HanjaDictionary`). 키=한글 문자열(글자 수 무관), 값=`Vec<HanjaEntry>`. **이미 다음절 단어 키를 지원**(메모리 노트: 27.5만 다음절 항목 존재). |
| `src/popup/popup_state.rs`, `popup_keys.rs`, `popup_layout.rs` | 팝업 SoT. `PopupState::new_hanja`/`new_hanja_with_top_row`. compact(1열) 레이아웃, `HANJA_PAGE_SIZE = 9`(popup_keys.rs:85) — **페이지당 9개 요구사항이 이미 구현돼 있음**. |
| `unim-tsf/src/key_handler.rs` | TSF `OnTestKeyDown`(`test_key_down`)/`OnKeyDown`(`handle_key_down`) 본체. 한자키 소비 판정, `engine.press_key()` 호출, commit/preedit → TSF 문서 반영, ATF 오케스트레이션, 팝업 액션 drain, 마우스 역이벤트 적용(`apply_reverse_event`). |
| `unim-tsf/src/composition.rs` | `CompositionManager` — TSF composition 생성/갱신/종료, **`replace_surrounding`**(N+1 BS 폴백 포함 — target①의 "이미 커밋된 텍스트 교체"에 재사용할 1차 후보), `read_selection_text`(선택 영역 읽기 — target②의 1차 후보). |
| `unim-tsf/src/auto_typefix.rs` | ATF 상태기계. `process_after_key`가 `AutoFixApply{delete_chars, commit_text, replay_preedit, end_composition, replace_composition}`를 산출 → key_handler가 `replace_surrounding`/조합 SetText로 적용. |
| `unim-tsf/src/popup_ipc.rs` | TSF 프로세스 쪽 팝업 IPC 클라이언트(`PopupClient`) + wire 타입(`WireMsg`/`RenderState`/`WireCell`) + 역채널(`RevChannel`/`RevEvent`). Named pipe로 별도 프로세스(`unim-popup-win.exe`)와 통신. |
| `unim-popup-win/src/protocol.rs` | 렌더러 프로세스 쪽 wire 타입 — **`popup_ipc.rs`와 손으로 동기화하는 별도 사본**(공유 crate 아님). |
| `unim-popup-win/src/render.rs` | GDI 렌더링. compact(한자) 모드는 고정 폭 컬럼(한자 90px + 뜻 오프셋 96px, `render.rs:504-522`) — **단어 길이를 가정하지 않은 좁은 폭**. |
| `unim-tsf/src/ui_element.rs` | UIA/UILess 후보 노출(`ITfCandidateListUIElement`). `PopupKind`를 그대로 노출용 스냅샷으로 변환. |
| `unim-imm32/src/input.rs` | IMM32 `ImeProcessKey`/`ImeToAsciiEx` 판정. Hanja/F9 키를 소비하도록 판정은 하지만 팝업은 **미배선**(`input.rs:41` 주석 "popup IMM32 wires later"). |
| `unim-imm32/src/ui_window.rs` | 후보창 **스텁**. `IMN_OPENCANDIDATE`/`IMN_CLOSECANDIDATE`/`IMN_CHANGECANDIDATE`/`IMN_SETCANDIDATEPOS` 전부 `// TODO(unim-imm32)` (ui_window.rs:106-145). 실제 창 생성 없음. |
| `unim-capi/src/lib.rs` | C ABI 표면. `unim_engine_press_key`(180-187)가 `InputResult`(repr(C))를 값으로 반환 — **코어 InputResult 변경이 실제로 건드리는 유일한 FFI 지점**. Windows에서도 `make check-windows`가 이 crate를 빌드 대상에 포함(WIN_CRATES, Makefile:491)하지만, TSF/IMM32는 이 crate를 쓰지 않고 `unim` 코어crate를 직접 링크한다(`unim-tsf/Cargo.toml:15`). |

## 2. 핵심 타입·함수 (file:line · 시그니처 · 역할)

- `src/input_engine/candidates.rs:16` `pub fn start_hanja_conversion(&mut self) -> InputResult`
  — target = `preedit_cache`의 **마지막 1글자**(23-29행). preedit이 비어있으면 `target=None`이 되어 **커밋 버퍼는 절대 보지 않는다**(주석은 "마지막 커밋 음절"이라 쓰여 있으나 실제 구현은 `None` 고정 — 코드·주석 불일치, 미해결 질문 §7 참조).
  — `hanja_dict.search(&target_syllable)`로 조회, 없으면 `special_chars::search_by_choseong`로 폴백(74-104행).
  — 후보 확정 시 `self.popup_pending_action = Some(PopupAction::ShowHanja{target, candidates, top_row})`(66-70행), `PopupState::new_hanja_with_top_row`로 `self.popup_state` 세팅(58-64행).
- `src/input_engine/candidates.rs:140` `pub fn select_hanja(&mut self, index: usize) -> Option<String>`
  — `preedit_cache`만 비운다(151-156행). **문서(이미 커밋된 앱 텍스트)는 건드리지 않는다** — 코어는 앱 문서에 접근 권한이 없으므로 프론트(TSF)가 `commit_str()`을 읽어 삽입하는 구조.
  — 반환된 한자 문자열은 `select_hanja` 호출부(`popup_dispatch.rs`의 `process_popup_key`)가 `self.commit_buffer`에 넣는 것으로 추정(§7 미확인 — `process_popup_key` 본문 미독).
- `src/hanja/dict.rs:104` `pub fn search(&self, hangul: &str) -> Vec<HanjaEntry>` — 키 전체 문자열 일치. 다음절 단어 키 지원(코드상 글자 수 제약 없음, `HashMap<String, Vec<HanjaEntry>>`).
- `src/popup/popup_keys.rs:85` `pub(super) const HANJA_PAGE_SIZE: usize = 9;` — 요구사항 "페이지당 9단어"가 이미 코드 상수로 존재.
- `unim-tsf/src/key_handler.rs:389` `pub fn handle_key_down(engine, config, comp_mgr, popup, preedit_win, atf_state, context, tid, wparam, comp_sink, composition_unsupported, fallback_pending, known_cuas) -> KeyDownOutcome`
  — 557행 `engine.press_key(keycode, modifiers, config)` 호출이 유일한 코어 진입점. Hanja키 처리·팝업 네비게이션·선택 확정이 전부 `press_key` 내부(코어)에서 일어나고, TSF는 그 결과(`commit_changed`/`preedit_changed`/`popup_pending_action`)만 소비한다.
  — 587행 `drain_popup_actions(engine, popup, caret_rect)` — `PopupAction`을 소진해 렌더 전송.
- `unim-tsf/src/key_handler.rs:1057` `fn drain_popup_actions(engine, popup: &mut PopupClient, caret_rect)`
  — `PopupAction` **완전 매칭**(와일드카드 없음, 1074-1086행). 새 `PopupAction` variant 추가 시 이 match가 컴파일 실패한다(§4 위험 참조).
- `unim-tsf/src/key_handler.rs:1130` `pub fn apply_reverse_event(engine, config, comp_mgr, popup, context, tid, comp_sink, env: &RevEnvelope, last_owner, last_seq)`
  — `RevEvent::CellClick{row,col}` → `engine.popup_state_mut().handle_click(row,col)` → `PopupKeyResult::Select`면 `engine.press_key(KeyCode::Enter, ...)`로 **키보드 확정과 동일 경로 재사용**(1171-1174행). 즉 마우스 클릭도 결국 코어 `press_key`를 태운다.
  — 확정 후 `commit_str()`을 읽어 `comp_mgr.insert_text`(비조합 삽입, 1236행)로 문서에 넣는다 — **`replace_surrounding`이 아니라 단순 삽입**. 즉 현재 팝업 확정 경로는 "이미 커밋된 앱 텍스트를 지우는" 능력이 없다(target①에서 반드시 확장해야 하는 지점).
- `unim-tsf/src/composition.rs:663` `pub fn replace_surrounding(&mut self, context, tid, delete_chars: u32, commit_text: &str, preedit_text: &str, comp_sink) -> ReplaceOutcome`
  — target①(커밋된 "대한민" + preedit "국" → "대한민국" 교체)에 재사용할 **1차 후보 API**. `delete_chars`로 커서 앞 N글자 삭제 + `commit_text` 삽입 + (선택) `preedit_text`로 재조합. `ReplaceOutcome`(270-288행)이 4갈래: `Normal`/`PhaseSplit`/`SynthBatch`/`SynthHeadTail` — **이것이 사실상의 "ATF 4-Phase"**(요구사항 문서의 "N+1 BS" 노하우가 `SynthBatch`/`SynthHeadTail` 경로, `synth_input::send_replacement_batch`).
  — 호출 예시(수동 typefix): `key_handler.rs:424-457` (Ctrl+Shift+Space) — target②(선택 영역 → 변환) 구현의 **직접적인 템플릿**: `composition::read_selection_text` → `engine.set_surrounding_text` → 엔진 변환 호출 → `comp_mgr.replace_surrounding`.
- `unim-tsf/src/composition.rs:1637` `pub fn read_selection_text(context: &ITfContext, tid: u32) -> Option<SelectionReadResult>`
  — `SelectionReadResult{surrounding_text, cursor, anchor}`(1558-1565행). **주의**: `surrounding_text`는 선택된 텍스트 자체가 아니라 "커서(선택 끝) 앞 텍스트"다. 실제 선택 문자열은 호출부가 `surrounding_text[anchor..cursor]`(char 단위, 1621/1605행 `chars().count()` 기반)로 슬라이스해야 한다. 선택이 없으면(`cursor==anchor`) `None`에 준하는 처리 필요(현재 호출부는 `sel.cursor != sel.anchor`로 가드, key_handler.rs:428).
- `unim-tsf/src/auto_typefix.rs:49` `pub struct AutoFixApply{delete_chars: u32, commit_text: String, replay_preedit: String, end_composition: bool, replace_composition: bool}` — target①의 "위임(N+1 BS) 결과" 구조를 그대로 본뜰 수 있는 기존 패턴.
- `unim-tsf/src/popup_ipc.rs:239` `pub fn to_render_state(vm: &unim::popup::PopupViewModel) -> RenderState` — `PopupKind` **완전 매칭**(280-284행, 와일드카드 없음): `Hanja=>0, SpecialChar=>1, Emoji=>2`. 새 `PopupKind` variant 추가 시 컴파일 실패.
- `unim-tsf/src/ui_element.rs:88` 동일하게 `PopupKind` 완전 매칭(간접 확인 — grep 결과, 상세 미독).
- `unim-capi/src/lib.rs:180` `pub extern "C" fn unim_engine_press_key(engine, config, hardware_code: u16, state: ModifierState) -> InputResult` — `InputResult`(repr(C), `src/input_engine/types.rs:224`)를 값으로 반환하는 유일한 FFI. TSF/IMM32는 이 함수를 쓰지 않지만(코어crate 직접 링크), **`make check-windows`가 unim-capi를 Windows 타깃으로 컴파일**하므로 `InputResult`에 필드를 추가하면 `src/input_engine/types.rs`의 생성자들(`not_consumed`/`consumed`/`preedit_updated`/`committed`/`committed_passthrough`/`hanja_candidates`/`special_char_candidates`, 241-315행)을 **전부** 갱신해야 한다(이 리터럴들은 `..Default::default()`를 안 쓰고 필드를 전부 명시 — 하나라도 빠지면 코어 자체가 모든 타깃에서 컴파일 실패, Windows 한정 이슈 아님).

## 3. 현재 동작 흐름 (단계별, 호출 순서)

### 3.1 키보드로 한자 변환·선택 (현 구현, 음절 단위)
1. `OnTestKeyDown` → `key_handler::test_key_down`(66) — Hanja/F9면 소비(184행 `keycode == KeyCode::Hanja || keycode == KeyCode::F9` → `return true`).
2. `OnKeyDown` → `handle_key_down`(389) → 557행 `engine.press_key(Hanja, ...)`.
3. 코어 `press_key.rs:226-237` — `hanja_keys.contains(&keycode)` → `start_hanja_conversion()`(candidates.rs:16) 호출. **preedit 마지막 1글자**만 검색 대상.
4. `candidates.rs:66-71` — 후보 있으면 `popup_pending_action=ShowHanja{...}`, `popup_state=Some(...)`, `InputResult::hanja_candidates()`(consumed=true, preedit_changed=true, commit_changed=false) 반환.
5. `handle_key_down:587` `drain_popup_actions` → `PopupAction::ShowHanja` 매치 → `first=true` → `popup.send_render(rs, first=true, flash=false)`(popup_ipc.rs:388) → named pipe로 `WireMsg{cmd:"render", render:Some(RenderState), first:Some(true), ...}` 전송 → `unim-popup-win.exe`가 렌더.
6. 숫자키/화살표/Enter 입력 시 다시 `press_key` → 코어 `process_popup_key`(popup_dispatch.rs:75, 본문 미독) → 확정 시 `select_hanja` 경유(추정) → `commit_buffer`에 한자 채움 → `InputResult.commit_changed=true`.
7. `handle_key_down:673-750` 정상 경로 — `commit = engine.commit_str()`, **preedit는 이미 비어있으므로** `comp_mgr.end_composition_with_text(context, tid, &commit)`(723행, 조합 중이었다면) 또는 `insert_text`(734행)로 **현재 조합 중이던 자리에만** 삽입. 이미 문서에 커밋된 앞부분("대한민")은 전혀 건드리지 않는다.

### 3.2 마우스로 한자 선택 (팝업 클릭)
1. `unim-popup-win.exe`가 셀 클릭 시 `WireMsg{cmd:"evt", evt:"cell_click", row, col}`을 파이프에 씀.
2. TSF 쪽 reader 서브스레드(`popup_ipc.rs:836` `reader_loop`)가 파싱 → `RevEvent::CellClick`을 `RevChannel`에 push + `PostMessageW(WM_UNIM_REV)`(popup_ipc.rs:181,211).
3. TSF wndproc이 `WM_UNIM_REV` 수신 → `RevChannel::drain()` → 각 이벤트에 `apply_reverse_event`(key_handler.rs:1130) 호출.
4. `CellClick` → `engine.popup_state_mut().handle_click(row,col)` → `Select`면 `engine.press_key(KeyCode::Enter, ...)`(1173행)로 **키보드 확정과 동일 코어 경로** 재사용.
5. `drain_popup_actions` 재호출(1218행) → `commit_str()` 있으면 `comp_mgr.insert_text`(1236행, 비조합 단순 삽입) — **`replace_surrounding` 미사용**.

### 3.3 수동 AutoTypeFix (Ctrl+Shift+Space) — target②의 기존 유사 패턴
1. `key_handler.rs:424-431` `composition::read_selection_text(context, tid)` → 선택 있으면(`sel.cursor != sel.anchor`) `engine.set_surrounding_text(surrounding_text, cursor, anchor)`.
2. `engine.typefix_convert(0)`(434행, 코어) → `(offset, delete_count, replacement)` 반환.
3. `comp_mgr.replace_surrounding(context, tid, delete_count, &replacement, "", comp_sink)`(435-442행) → `ReplaceOutcome`에 따라 `Normal`/`PhaseSplit`(→ `schedule_flush=true`)/`SynthBatch`(→ `engine.remove_preedit()`) 분기(445-454행).

### 3.4 ATF 4-Phase(교체 노하우) 실제 코드 위치
- `try_undo`(auto_typefix.rs:174) — Ctrl+Z, `key_handler.rs:463-491`에서 `replace_surrounding` 호출.
- `process_after_key`(auto_typefix.rs:233) — 순방향/역방향 자동 교정. 반환 `AutoFixApply`를 `key_handler.rs:771-909`가 소비하며 5갈래로 분기:
  1. `word_live_reverse`(792-809행) — Word 라이브 조합 SetText 치환(삭제 0).
  2. Word 순방향 보유영문 치환(810-849행) — `comp_mgr.update_composition`으로 라이브 조합 치환.
  3. 일반 역방향(850-909행) — `comp_mgr.end_composition_keep_text` + `replace_surrounding`.
  4. `ReplaceOutcome::PhaseSplit` → `schedule_flush=true` → 타이머(`WM_UNIM_FLUSH2`) → `flush_restart_phase_b`(key_handler.rs:968).
  5. `ReplaceOutcome::SynthHeadTail` → `flush_pending_tail`(key_handler.rs:1005).
- "N+1 BS"의 실제 SendInput 지점은 `composition.rs:711,728` `crate::synth_input::send_replacement_batch(d, &c)`(BS×d + UNICODE 삽입, edit session 밖에서 호출 — 699-734행 참조). **target①의 이미-커밋된-텍스트 삭제도 이 두 경로(TSF 정상 ShiftStart 역확장 / synth SendInput 폴백) 중 하나를 반드시 타게 된다** — CUAS 앱(카톡 등)에서는 synth 폴백이 유일한 삭제 수단.

### 3.5 렌더 데이터 흐름 (엔진 → 화면)
`PopupState`(코어 SoT) → `PopupViewModel` → `to_render_state`(popup_ipc.rs:239, column-major 평탄화) → `WireMsg{cmd:"render", render:RenderState}` JSON 직렬화 → named pipe(`\\.\pipe\unim-popup-win.<session_id>`, popup_ipc.rs:35) → `unim-popup-win.exe` 파싱(자체 `protocol.rs` 사본으로 역직렬화) → `render.rs::paint_compact`(461행, 한자/특수문자 compact 1열 레이아웃) 또는 그리드 레이아웃(이모지 등, 미조사).

## 4. 이 기능을 위한 확장 지점 (어디를 어떻게, 위험도)

1. **변환 대상 계산 확장(target①: 커밋+preedit 결합)** — `src/input_engine/candidates.rs:22-29`.
   - 현재: preedit 마지막 1글자만. 확장: "최근 커밋 음절 버퍼"(신규 상태, 코어 `InputEngine`에 필드 추가 필요 — 미존재, `rg "recent_commit\|last_committed"` 무결과 확인 필요/미실행)와 현재 preedit을 결합해 최장 일치 한글 단어를 만들고 `hanja_dict.search(word)`로 조회.
   - 위험: **낮음(코어 전용)** — Windows 특화 위험 아님. 단, 이 신규 상태를 "언제 리셋하는가"(공백/구두점/포커스이동/모드전환)가 Windows·Linux 공통으로 정의돼야 한다. `engine.reset()` 호출부들(key_handler.rs:844 등 다수)이 이 버퍼도 함께 비우는지 감사 필요(§7).
2. **문서 상의 이미-커밋된 텍스트 교체(target①ᴮ, ②)** — `unim-tsf/src/key_handler.rs:1220-1249`(`apply_reverse_event` 커밋 삽입부)와 `handle_key_down:673-750`(키보드 확정 경로) 둘 다 현재 `insert_text`만 쓴다.
   - 확장: 코어가 "삭제해야 할 글자 수"(committed 쪽)를 `PopupAction`/`InputResult`에 실어 보내면, TSF는 `comp_mgr.replace_surrounding(context, tid, delete_chars, &hanja, "", comp_sink)`(composition.rs:663)를 호출하도록 두 지점 모두 바꿔야 한다.
   - 위험: **중간** — `replace_surrounding`은 이미 CUAS/synth 폴백까지 구현된 검증된 API라 재사용 자체는 안전하지만, **호출부 두 곳(키보드 확정 vs 마우스 클릭 확정)을 반드시 함께 고쳐야** 하며 하나만 고치면 "키보드로는 되는데 마우스 클릭만 안 되는" 회귀가 난다.
3. **선택 영역 기반 변환(target②)** — 신규 진입점 필요. `unim-tsf/src/key_handler.rs:389` 어딘가(Hanja 키 처리 전, `composition::read_selection_text`가 유의미한 선택을 반환하면 우선)에서 `read_selection_text` → 슬라이스(`surrounding_text[anchor..cursor]`, char 인덱스 주의) → 코어 신규 API(예: `engine.start_hanja_conversion_for_text(word)`, 미존재)를 호출하는 분기 추가.
   - 위험: **중간** — `read_selection_text`는 `TF_ES_READ|TF_ES_SYNC`라 대부분 앱에서 동작하지만, 일부 CUAS 앱은 `GetSelection` 자체를 지원 안 하거나 부정확한 값을 줄 수 있다(코드 내 방어적 clamp 존재, composition.rs:1601-1603, 1618-1619 — 이미 "오작동 앱" 가정하에 방어 코드가 있다는 것 자체가 실제 사고 이력 시사).
4. **팝업 렌더 폭(단어 길이)** — `unim-popup-win/src/render.rs:505,514,520` 고정 90px/96px 컬럼.
   - 확장: `text_width(hdc, &main, font_hanja)`(이미 513행에 유사 계산 존재)로 동적 폭 계산, `mean_left`를 `hanja_left + max(96, tw+8)`처럼 가변화.
   - 위험: **낮음(렌더러 로컬)** — 단, `unim-tsf/src/popup_ipc.rs`와 `unim-popup-win/src/protocol.rs`는 **완전히 별개 파일의 손 동기화 사본**(주석 popup_ipc.rs:81 "필드 추가 시 반드시 unim-popup-win/src/protocol.rs 사본과 동일하게 유지")이라, 폭 힌트를 wire에 새 필드로 추가하면 두 파일을 함께, 순서까지 동일하게 고쳐야 한다(직렬화 키 순서 동결 주석, popup_ipc.rs:103-104).
5. **PopupKind 재사용 vs 신규 variant** — 신규 "한자 단어" 팝업을 별도 `PopupKind`로 만들면 `popup_ipc.rs:280-284`·`ui_element.rs`(PopupKind 완전매치 지점)·`render.rs`(kind→색상 매핑, `sel_hanja` 등)까지 전부 손대야 한다.
   - **권장**: 기존 `PopupKind::Hanja`/`ShowHanja` 그대로 재사용하고 `candidates: Vec<(String,String)>`에 다중 글자 문자열만 넣는다(타입이 이미 `String`이라 글자 수 제약 없음, dict.rs:104 확인). 이러면 §4-4의 렌더 폭 이슈만 남고 exhaustive-match 파손 위험이 0.
   - 위험: 재사용 시 **낮음**, 신규 variant 시 **높음**(다중 파일 동시 수정, 컴파일 강제이므로 빠뜨리면 즉시 실패하지만 수정량이 큼).
6. **출력 형식 설정(漢字/한자(漢字)/漢字(한자))** — 신규 `Config` 필드. Windows UI는 `unim-tsf/src/settings_dialog.rs:708`(`build_page_general_rest`) 안의 `ModeSharingMode` 콤보박스 패턴(759-760, 1282-1284행)을 그대로 본뜨면 된다.
   - 위험: **낮음** — 순수 UI 추가. 단, 실제 서식 적용은 `commit_text` 조립 지점(코어 `select_hanja` 또는 TSF 삽입 직전)에서 해야 하며, 한 곳에만 넣으면 마우스 클릭 경로(§4-2)에서 서식이 빠지는 회귀가 날 수 있다 — **커밋 텍스트 조립은 코어 한 곳(예: select_hanja 내부)에서 끝내고 프론트는 그대로 삽입만 하도록** 설계해야 키보드/마우스 두 경로가 자동으로 동기화된다.
7. **IMM32 후보창** — `unim-imm32/src/ui_window.rs:106-145` 전부 스텁. 단어 변환을 IMM32에서도 지원하려면 이 창을 처음부터 구현해야 하며 기존 재사용 자원이 없다.
   - 위험: **높음(신규 구현)** — 그러나 "회귀 0" 요구는 자동 충족: 현재 스텁 상태가 이미 "동작 안 함"이므로, Linux/TSF만 먼저 구현하고 IMM32를 손대지 않으면 **새로 깨질 기존 동작 자체가 없다**(스텁은 스텁인 채로 컴파일만 유지하면 됨).

## 5. 지켜야 할 규칙 (근거 인용)

- `unim-tsf/src/popup_ipc.rs:81-82` "필드 추가 시 반드시 unim-popup-win/src/protocol.rs 사본과 동일하게 유지" — 와이어 프로토콜은 공유crate가 아니라 **수동 동기화 사본**.
- `unim-tsf/src/popup_ipc.rs:103-104` "필드 순서(직렬화 키 순서)는 동결: v,cmd,pid,seq,first,flash,owner_hwnd,render,evt,row,col,dir,index" — JSON 골든 라인 테스트가 바이트 단위로 이 순서를 검증(`popup_ipc.rs` 하단 `golden_json_line_stable`류 테스트, 1263행 부근).
- `unim-tsf/src/composition.rs:270-288`(`ReplaceOutcome` 주석) — 호출부가 각 variant별로 정확히 다른 후속 처리(엔진 preedit 보존 여부, schedule_flush 여부)를 해야 한다는 계약이 주석에 명문화돼 있음. 새 호출부(target①)를 추가할 때 이 4갈래 분기를 빠짐없이 복제해야 한다.
- `unim-tsf/src/key_handler.rs:1046-1049`(drain_popup_actions 문서 주석, 원문 근처) "first = Show*(ShowHanja/ShowSpecial/ShowEmoji) 수신 (새 팝업)" / "flash = HanjaCandidatesReordered 에서 was_bookmarked && !bookmarked" — `PopupAction` 신규 variant를 만들면 이 3-플래그(first/hide/flash) 축소 모델에 어느 축으로 편입시킬지 결정해야 함.
- `CLAUDE.md`(프로젝트, `/home/from104/work/unim/CLAUDE.md`) — Bash 출력 20줄 제한·ctx_execute 우선 등 조사 방법론(본 조사에서 준수: Read는 전부 offset/limit 지정).
- 메모리 `feedback_config_3way_sync.md` — "엔진(src/config.rs)·GUI·CLI 설정은 항상 함께 싱크. 한 곳만 추가/삭제 금지" — 출력 형식 설정 추가 시 Windows `settings_dialog.rs`뿐 아니라 GTK(`unim-settings-gtk`)·CLI(`unim-cli`)도 함께 추가해야 함(본 조사 범위 밖이나 §6에 명시).
- 메모리 `feedback_prefer_lsp.md` — 심볼 탐색은 LSP 우선(본 조사는 `.rs` LSP 서버 부재/미시도, rg+Read offset/limit로 대체 — 사용자 CLAUDE.md의 "서버 없으면 Read로" 예외 조항에 해당).

## 6. Windows 동등성 메모

- **핵심 구조적 사실**: TSF/IMM32는 `unim` 코어crate를 **직접** 링크한다(FFI 아님). 따라서 target①/②의 핵심 로직(변환 대상 계산, 사전 조회, 팝업 상태 관리, 서식 적용)을 코어(`src/input_engine/`, `src/hanja/`, `src/popup/`)에 구현하면 **Windows 쪽은 코드 추가 없이 자동으로 같은 로직을 공유**한다. Windows 쪽에서 반드시 추가로 손대야 하는 곳은 "문서에 반영하는 방법"(레이어 3: `replace_surrounding` 호출 지점 2곳, §4-2)과 "렌더 폭"(§4-4)뿐이다.
- **unim-capi는 Windows TSF/IMM32와 무관**하지만 `make check-windows`의 컴파일 대상(Makefile:491)이라 **코어 타입 변경 시 반드시 같이 빌드되어 에러를 낸다** — 이것이 오히려 안전망 역할(cfg gate로 숨겨지는 게 아니라 컴파일 에러로 드러남). `cfg(windows)`로 조건부 컴파일되는 코드는 unim-capi/src/lib.rs에 **없음**(grep 결과 0건) — 즉 이 crate는 완전히 플랫폼 중립이며, "cfg gate가 깨지는 지점"은 사실상 없고 대신 "타입 시그니처 불일치로 즉시 컴파일 실패"가 안전장치다.
- **cfg(windows) 실제 분기는 각 crate의 `Cargo.toml`의 `[target.'cfg(windows)'.dependencies]`** (예: `unim-tsf/Cargo.toml:24`)에 있고, `.rs` 코드 내부에는 `cfg(windows)` 어트리뷰트가 거의 없다(플랫폼별로 crate 자체가 분리돼 있어 코드 내부 분기가 적음, 확인: `rg "cfg(windows)" unim-capi/src` 0건 — 다른 crate는 미조사, 필요시 추가 확인).
- **IMM32는 사실상 미구현**이므로 "Windows 동등성"을 이번 기능에서 확보하려면 최소 TSF만 목표로 잡고 IMM32는 "기존과 동일하게 미지원(스텁 유지)"으로 명시적으로 범위 제외하는 것이 안전 — 스텁을 건드리지 않으면 회귀 위험 0.
- **`caret_rect`가 `Option`인 이유**(popup_ipc.rs:77-83, `skip_serializing_if`)처럼, 이 코드베이스는 신규 옵셔널 필드를 `#[serde(default, skip_serializing_if="Option::is_none")]`로 추가해 **기존 골든 직렬화 바이트를 불변으로 유지**하는 패턴을 이미 확립해 두었다 — target①/②용 신규 wire 필드(예: 단어 모드 플래그, 폭 힌트)도 이 패턴을 따라야 골든 테스트(`popup_ipc.rs` 하단 `golden_*` 테스트군)가 깨지지 않는다.
- **설정 3지점 동기화**는 이 조사 범위(Windows 서브시스템) 밖의 GTK/CLI까지 걸치므로, 다른 담당 서브시스템(engine-frontend/ui)과 반드시 좌표해야 한다.

## 7. 미해결 질문

1. `select_hanja`(candidates.rs:140)가 실제로 `self.commit_buffer`에 쓰는 코드 경로를 이 조사에서 직접 읽지 못했다(`popup_dispatch.rs::process_popup_key` 본문 미독, 75행 시그니처만 확인). target①/②에서 "코어가 커밋 텍스트를 어떻게 조립하는지"의 정확한 위치 확인 필요.
2. `candidates.rs:22` 주석("마지막 커밋 음절")과 실제 구현(28행 `None` 고정)이 불일치한다 — 이미 "최근 커밋 음절을 본다"는 코드가 죽은 채 남아있는 것인지, 단순 주석 노후화인지 코어 담당자 확인 필요.
3. "최근 커밋 음절 버퍼"(target①)를 코어에 신설할 경우 리셋 조건(공백/문장부호/포커스이동/`engine.reset()`/모드전환/시간 초과) — Linux 데스크톱(IBus/fcitx)과 Windows(TSF/IMM32) 양쪽에서 "포커스 이동"을 감지하는 시점이 다를 수 있어(TSF는 `OnSetFocus`, 코어는 프론트가 명시적으로 `reset()` 호출) 버퍼 리셋이 플랫폼별로 어긋날 위험 — 코어 SoT로 강제해야 함.
4. 마우스 클릭 확정 경로(`apply_reverse_event`)에 `replace_surrounding`을 추가할 때, `comp_sink`/`context`가 `Option<&ITfContext>`(1135행, 키보드 경로는 `&ITfContext` 비-옵션)라 **컨텍스트가 None인 경우**(1242-1248행 "commit deferred (dropped)" 로그 존재) target① 교체가 아예 스킵될 수 있다 — 이 None 케이스가 실제로 얼마나 자주 발생하는지 미확인.
5. `unim-popup-win`의 그리드(비-compact, 특수문자/이모지) 레이아웃 코드는 이번 조사에서 안 읽었다 — 한자 단어 팝업을 compact 유지로 결정했다면 무관하지만, 혹시 향후 "다중 열" 요구가 생기면 별도 조사 필요.
6. Windows 쪽 `Config` 리로드(`unim_config_reload`, capi:129 / TSF 자체 파일감시)가 신규 출력형식 설정을 핫리로드하는지, 재시작이 필요한지 확인 안 됨.

## 보충 #8: TSF out-of-band 결과 전달 — 신규 접근자 위치, ReplaceOutcome 4갈래 복제 지점, 배타 조건

### 8.1 왜 `commit_str()`만으로는 부족한가
`engine.commit_str()`(엔진 공용 accessor)는 **삽입할 텍스트**만 돌려준다. target①/②의 "이미 커밋된 한글을 지우고 한자를 넣는" 대체 입력은 **몇 글자를 지울지(delete_chars)**가 반드시 함께 필요하다 — 이는 `InputResult`(repr(C) ABI, `src/input_engine/types.rs:224` 부근)에 없는 정보이며, ABI를 동결한 채로는 리턴값에 필드를 못 늘린다. 이미 이 문제를 풀어둔 선례가 두 개 있다:
- `take_atf_toggle()` (`src/input_engine/engine.rs:604`) — ATF 토글 결과를 `InputResult` 밖에서 별도 drain.
- `popup_pending_action`/`take_popup_action()`류 (`src/input_engine/candidates.rs:66,98,240`, `src/input_engine/popup_dispatch.rs:21`) — 팝업 렌더 지시를 `InputResult` 밖에서 별도 drain.
- `AutoFixApply{delete_chars, commit_text, replay_preedit, end_composition, replace_composition}` (`unim-tsf/src/auto_typefix.rs:49`) — **바로 이 모양**이 target①에 필요한 페이로드와 사실상 동일 구조.

### 8.2 신규 접근자 설계
`popup_pending_action` 선례와 동일한 drain 패턴으로, 코어에 예:
```
pub fn take_pending_hanja_replacement(&mut self) -> Option<(u32 /*delete_chars*/, String /*commit_text*/)>
```
를 신설한다(`InputResult` 필드는 건드리지 않음 — engine.rs:144/types.rs:61 문서 주석이 명시한 "out-of-band 드레인 채널" 계약 그대로 재사용). `select_hanja`(candidates.rs:140) 확정 시점에, 대상이 "이미 커밋된 문자열 + preedit 결합"(target①)이거나 "선택 영역"(target②)이었다면 지울 글자 수를 여기 실어 둔다. 확정이 순수 preedit 치환(현재 동작, §3.1)이면 `None`을 유지해 기존 경로와 바이트 동일.

### 8.3 호출부 두 곳 — 정확한 삽입 위치와 ReplaceOutcome 4갈래 복제
두 곳 다 **기존 `insert_text`/`end_composition_with_text` 호출 직후**, `take_pending_hanja_replacement()`가 `Some`이면 그 결과를 우선해 `comp_mgr.replace_surrounding`으로 분기해야 한다.

1. **키보드 확정** — `unim-tsf/src/key_handler.rs:673-750`(정상 경로 commit 렌더링 블록). 현재 723행 `comp_mgr.end_composition_with_text(context, tid, &commit)` / 734행 `comp_mgr.insert_text(context, tid, &commit)`가 그 자리다. `take_pending_hanja_replacement()`가 `Some((delete_chars, hanja))`이면 이 두 호출 대신
   ```
   let outcome = comp_mgr.replace_surrounding(context, tid, delete_chars, &hanja, "", comp_sink);
   ```
   을 호출하고, 그 직후 **`key_handler.rs:446-454`(수동 typefix)와 바이트 동일한 4갈래**를 복제한다:
   - `ReplaceOutcome::Normal => {}`
   - `ReplaceOutcome::PhaseSplit => schedule_flush = true;` (엔진 preedit 보존 — `remove_preedit()` 호출 금지, composition.rs:277-282 계약)
   - `ReplaceOutcome::SynthBatch => engine.remove_preedit();`
   - `ReplaceOutcome::SynthHeadTail => { schedule_flush = true; engine.remove_preedit(); }` (`key_handler.rs:897-905` 패턴과 동일 — R6b 머리/꼬리 분리)
   `schedule_flush`는 이미 `handle_key_down` 로컬 변수로 존재하고(753행 선언) `KeyDownOutcome{ eaten, schedule_flush, .. }`(454행 리턴 패턴)로 흘러가므로 배선 변경 불필요.

2. **마우스/원격 팝업 확정** — `unim-tsf/src/key_handler.rs:1130`(`apply_reverse_event`) 내부, 현재 1210-1234행이 `commit_str()` → `comp_mgr.insert_text(ctx, tid, &commit)`(1234행)만 호출하는 지점. 여기가 target①ᴮ의 실제 구멍이다: 마우스로 한자 단어를 클릭 확정해도 지금은 순수 삽입뿐이라 원본 한글이 지워지지 않는다. 동일하게 `take_pending_hanja_replacement()` 분기 + `replace_surrounding` 호출 + 위 4갈래를 여기에도 복제해야 하는데, **`apply_reverse_event`는 현재 반환형이 `()`이고 `schedule_flush`를 호출부(WM_UNIM_REV 핸들러, wndproc)에 돌려줄 통로가 없다** — `PhaseSplit`/`SynthHeadTail` 분기를 살리려면 `apply_reverse_event`의 시그니처를 `-> KeyDownOutcome`(또는 최소 `bool` schedule_flush)으로 넓혀야 하고, 그 반환값을 wndproc의 `WM_UNIM_REV` 처리부(popup_ipc.rs `reader_loop`가 아니라 TSF 쪽 `PostMessageW(WM_UNIM_REV)` 수신부, key_handler.rs:1130 밖)까지 배선해야 한다. 이것이 windows-parity가 "두 확정 경로를 함께 고쳐야" 하는 이유의 구체적 정체 — 단순 로직 복제가 아니라 **호출부 시그니처 변경**이 동반된다.

3. `context: Option<&ITfContext>`(apply_reverse_event 시그니처, 1135행)가 `None`인 경우(1242-1248행 "commit deferred (dropped)" 로그 경로) `replace_surrounding` 자체를 호출할 대상이 없다 — 이 경로에서는 target① 교체를 스킵하고 기존처럼 drop 로그만 남기는 것이 안전 degrade(§7-4의 미확인 빈도 문제와 동일 리스크, 신규 코드가 새로 만드는 문제 아님).

### 8.4 이중 삽입 없이 배타적으로 갈리는 조건
ATF 자동교정 오케스트레이션 블록은 `if !popup_active && atf_active`(`key_handler.rs:753` 부근, "AutoTypeFix 오케스트레이션" 주석 직후)로 게이트돼 있어, **팝업이 떠 있는 동안(popup_active=true)에는 ATF의 `replace_surrounding` 호출이 구조적으로 발생하지 않는다**. 신규 한자 단어 대체 경로는 정의상 "팝업에서 항목을 확정한 순간"에만 `take_pending_hanja_replacement()`가 `Some`을 반환하므로(§8.2 — `select_hanja` 확정 시점에만 세팅), 두 경로가 동시에 같은 키 입력에서 값을 갖는 일이 없다:
- 일반 타이핑(popup 없음) → ATF 경로만 `Some` 가능, 신규 경로는 항상 `None`.
- 팝업 확정(Enter/클릭) → ATF 블록 자체가 `!popup_active` 가드에 안 걸려 스킵되고(단, 확정 키 입력 자체는 `press_key`가 팝업을 이미 닫힌 상태로 처리하므로 `popup_active`가 확정 **직후** 시점엔 false로 바뀔 수 있어 엄밀히는 "확정 처리 그 프레임"의 순서 보장이 필요 — `drain_popup_actions`가 `HidePopup`을 먼저 소비한 뒤 `take_pending_hanja_replacement()`를 읽는 순서면 안전), 신규 경로만 `Some`.
결론: 배타성은 **게이트 조건(popup_active)이 아니라 "어느 쪽이 언제 `Some`을 세팅하는가"(생산자 측 상호배제)**로 보장해야 하며, 이는 `select_hanja`/`process_after_key`가 같은 키 처리(`press_key` 1회 호출) 안에서 동시에 두 개의 pending 값을 세팅하지 않는다는 코어 쪽 불변식(§7 미확인 4번과 연결 — `process_popup_key` 본문 미독)으로 확인해야 확정된다.

