# 팝업 파이프라인(코어→DBus→렌더러)

조사 범위: 한자 단어 입력 기능 설계를 위한 팝업 파이프라인 현황 지도.
모든 주장은 `file:line` 근거를 달았다. 근거 없는 추측은 "없음(확인: rg 패턴)"으로 명시.

## 1. 핵심 파일과 역할

| 파일 | 역할 |
|---|---|
| `src/popup/popup_state.rs` (1406줄) | `PopupState` 단일 진실 소스 구조체 + 생성자(`new_hanja`/`new_special`/`new_emoji`) |
| `src/popup/popup_keys.rs` (683줄) | `PopupKey`/`PopupKind`/`PopupKeyResult` enum, 키 입력 → 상태 전이 로직 (`handle_hanja_key` 등) |
| `src/popup/popup_layout.rs` (422줄) | 페이지/행/열 계산, (row,col)→global index 변환, 접근자, `toggle_hanja_expanded` |
| `src/popup/view_model.rs` (603줄) | `PopupState` → `PopupViewModel`(렌더링용 평면 구조) 변환. `hanja_view_model`/`emoji_view_model`/`special_view_model` |
| `src/input_engine/popup_dispatch.rs` (426줄) | 엔진 레벨 팝업 디스패치: 키코드→`PopupKey` 매핑, `process_popup_key`, `popup_select`, `popup_cancel`, `popup_change_page` |
| `src/input_engine/candidates.rs` (270+줄) | 한자 변환 시작/선택/취소/즐겨찾기 토글의 실제 비즈니스 로직 (`start_hanja_conversion`, `select_hanja`, `toggle_hanja_bookmark`) |
| `src/input_engine/surrounding.rs` (325줄) | Surrounding text(커밋된 텍스트+커서+앵커) 보관, `smart_backspace`, `typefix_convert` — **AutoTypeFix 치환 프로토콜의 SoT** |
| `src/hanja/dict.rs` | `HanjaDictionary` — **음절 단위** 사전(`HashMap<String(1글자), Vec<HanjaEntry>>`). 단어 사전 없음 |
| `unim-popup-types/src/lib.rs` (111줄) | DBus payload 평면 타입의 단일 SoT — `PopupRenderPayload`, `HanjaCandidateResponse`, `popup_render_flags` |
| `unim-dbus/src/service.rs` (3446줄) | DBus 인터페이스: RPC 메서드 + 시그널 정의, `PopupAction` → 시그널 발행 매핑 |
| `unim-dbus/src/engine_worker.rs` (3211줄) | `EngineRequest` 처리 루프, `build_render_state`(view_model→payload 변환), popup-owner 라우팅 |
| `unim-popup-service/src/popup/hanja.rs` (644줄) | GTK4 한자 팝업 윈도우 (X11/일반 Wayland WM) |
| `unim-popup-win/src/render.rs` (811줄) / `protocol.rs` (347줄) | Windows Direct2D 렌더러 + wire 프로토콜 (named pipe JSON) |
| `unim-gnome-extension/popup_view.js` (574줄) | GNOME Shell(Wayland, mutter) St 위젯 렌더러 — GTK와 동일 CSS 토큰 공유 |
| `unim-gui-common/src/popup_state.rs` (223줄) | toolkit-free `PopupModel` — **현재 아무 크레이트도 소비하지 않음(Qt Phase 2용, 미사용 확인: `rg "PopupModel" -g '*.rs'` → 정의 파일만 매치)** |
| `unim-gui-common/src/popup_dbus.rs` (238줄) | GTK/GNOME 공용 DBus 클라이언트 헬퍼 (`select_hanja_via_dbus` 등) |
| `docs/dev/specs/POPUP_SPEC.md` (786줄) | 팝업 공통 규격 — **절대 규칙**(사용자 메모리 `feedback_popup_spec_absolute.md`: 예외 없이 준수, 변경 시 사용자 승인 필수) |

## 2. 핵심 타입·함수 (file:line · 시그니처 · 역할)

### 2.1 PopupState (엔진 SoT)

- `PopupKind` enum — `src/popup/popup_keys.rs:11-20`: `Hanja`(9개/페이지 단일 열), `SpecialChar`(9×9), `Emoji`(9×9+카테고리 탭).
- `PopupState::new_hanja_with_top_row(target: &str, candidates: Vec<(String,String)>, top_row: &str) -> Self` — `src/popup/popup_state.rs:93-122`. `target`은 **단일 문자열**(현재는 음절 1글자)이지만 필드 타입은 `String`이라 다글자 문자열도 그대로 들어간다 — 제약은 여기 없음, 제약은 §4.1 참조.
- 페이지 크기 상수 — `src/popup/popup_keys.rs:79-85`: `SPECIAL_PAGE_SIZE=81`, `EMOJI_PAGE_SIZE=81`, **`HANJA_PAGE_SIZE=9`**(compact). expanded 모드는 `toggle_hanja_expanded()`가 `page_size`를 `SPECIAL_PAGE_SIZE(81)`로 바꿔치기 — `src/popup/popup_layout.rs:56-60`.
- `update_page_layout(&mut self)` — `src/popup/popup_layout.rs:21-48`. Hanja compact: `rows=page_items, cols=1`. Hanja expanded/Special/Emoji: `rows=MAX_ROWS(9)` 고정, `cols=ceil(page_chars/9).clamp(1,9)`.
- `hanja_global_index_rc(row,col) -> Option<usize>` — `src/popup/popup_layout.rs:111-124`: compact은 `col` 무시, `page_start+row`; expanded은 **column-major** `col*rows+row`.
- `selected_global_index()` — `src/popup/popup_layout.rs:137-144`.
- `set_bookmark_flags`/`set_bookmark`/`replace_hanja_items`/`set_selected_global` — `src/popup/popup_layout.rs:211-293`. 즐겨찾기 토글 후 후보 재정렬+커서 재배치에 사용.
- `hanja_page_items() -> Vec<(&str,&str)>` — `src/popup/popup_layout.rs:409-421`: 현재 페이지의 (한자, 뜻) 슬라이스.

### 2.2 PopupKey 처리 (한자 전용)

- `PopupKey` enum (툴킷 중립) — `src/popup/popup_keys.rs:27-56`: `Number(u8)`, `Letter(u8)`(expanded 열 점프), `CatLetter(u8)`(이모지 전용), `Up/Down/Left/Right/Enter/Escape/Tab/ShiftTab/PageUp/PageDown/Home/End/Space/Backspace/Period/Modifier/Other`.
- `PopupKeyResult` enum — `src/popup/popup_keys.rs:61-75`: `Select(usize)/ToggleBookmark(usize)/Cancel/Updated/Consumed/NotHandled`.
- `handle_hanja_key(&mut self, key: PopupKey) -> PopupKeyResult` — `src/popup/popup_keys.rs:306-`(383행대까지, Letter 처리는 460행대). 요지:
  - `Period` → `toggle_hanja_expanded()` (compact↔expanded).
  - `Enter` → `selected_global_index()` 로 `Select`.
  - `Space` → `selected_global_index()` 로 `ToggleBookmark`.
  - `Number(n)` → compact: `hanja_global_index(n-1)`; expanded: 해당 행(0-based n-1) 존재 시 `sel_row`설정 후 `Select`.
  - `Up/Down` — wrap-around, expanded은 빈 셀 skip.
  - `Left/Right` — compact: 페이지 wrap 전환; expanded: 열 wrap.
  - `Tab/PageDown`, `Backspace/ShiftTab/PageUp` — 페이지 wrap 전환 (`sel_row=sel_col=0`로 리셋).
  - `Home/End` — `jump_to_first`/`jump_to_last`(`src/popup/popup_keys.rs:100-` 부근, 3개 popup 공통 헬퍼).
  - expanded에서만 `Letter(col_idx)` → 열 점프 허용, compact은 `NotHandled`(회귀 방지 주석 — `src/popup/popup_keys.rs:458-461`).

### 2.3 뷰 모델

- `PopupViewModel` 구조체 — `src/popup/view_model.rs:33-74`. `cells: Vec<Vec<Option<CellData>>>`(row-major 2차원), `col_headers/row_headers`(항상 9개 고정폭), `header_text/footer_text`(daemon이 이미 포맷 완료한 문자열), `expand_visible/expand_text`, `tab_labels`(이모지 전용).
- `CellData` — `src/popup/view_model.rs:11-24`: `text: String`(글자 수 제한 없음), `meaning: Option<String>`, `is_selected/is_col_highlight/is_row_highlight/is_bookmarked`.
- `hanja_view_model(&self)` — `src/popup/view_model.rs:260-399`. expanded/compact 분기:
  - expanded(267-345행): 9×9 그리드, 헤더는 `「target」 → {선택된 한자} {뜻}` 동적 포맷(310-320행).
  - compact(346-398행): `hanja_page_items()` 순회, `row_headers=["1.","2.",...]`, 헤더는 고정 `「target」 → 한자`.
  - **주의**: `header_text`에 `target`을 그대로 삽입 — target이 여러 글자여도 문자열 연결이라 깨지지 않음.
- `emoji_view_model`/`special_view_model` — 유사 패턴, 이모지는 좌측 카테고리 탭 라벨 생성(129-150행).

### 2.4 InputEngine ↔ Popup 연결부

- `keycode_to_popup_key(keycode: KeyCode) -> PopupKey` — `src/input_engine/popup_dispatch.rs:25-72`. 물리 키 위치 고정(QWERTYUIO=Letter 0-8, ASDFGHJKL=CatLetter 0-8), 키맵 무관.
- `process_popup_key(&mut self, keycode, modifier, config) -> InputResult` — `src/input_engine/popup_dispatch.rs:75-187`. `PopupState::handle_key()` 호출 후 결과별 분기:
  - `Select` → `popup_select(abs_index)`.
  - `ToggleBookmark` → `toggle_hanja_bookmark(abs_index)`(재정렬+`HanjaCandidatesReordered` 액션 자체 emit).
  - `Cancel` → `popup_cancel()`.
  - `Updated` → 이모지면 카테고리 전환 감지 후 `ShowEmoji` 재발행 분기(113-160행), 그 외엔 `PopupNavigate` 액션 발행(161-172행).
  - `NotHandled` → 팝업 취소 후 `press_key()`로 키 재처리(180-185행).
- `popup_select(&mut self, abs_index) -> InputResult` — `src/input_engine/popup_dispatch.rs:190-222`. **한자 분기(191-197행)**: `select_hanja(abs_index)` 결과를 **`self.commit_buffer.push_str(&hanja)`** 로 단순 추가하고 `PopupAction::HidePopup` 설정. **이 경로는 "이미 커밋된 텍스트를 지우고 교체"하는 로직이 전혀 없다** — preedit을 지우고 새 문자열을 commit_buffer에 얹는 것뿐(§4.3 참조).
- `popup_cancel(&mut self)` — `src/input_engine/popup_dispatch.rs:225-241`. 한자 취소 시 `hanja_target`(원래 음절)을 그대로 commit_buffer에 복원.

### 2.5 한자 변환 시작 로직 (§4.1의 핵심 제약)

- `start_hanja_conversion(&mut self) -> InputResult` — `src/input_engine/candidates.rs:16-109`.
  - **대상 결정(22-29행)**: `preedit_cache`가 비어있지 않으면 그 **마지막 1글자**(`.chars().last()`)만 target. `preedit_cache`가 비어있으면 `target = None`(커밋 버퍼는 아예 보지 않음 — 주석은 "커밋 버퍼의 마지막 음절"이라 쓰여 있지만 실제 구현은 `None`으로 하드코딩, 코드가 주석과 불일치).
  - `hanja_dict.search(&target_syllable)` — 단일 음절 사전 조회(`src/hanja/dict.rs:104-106`, `entries: HashMap<String, Vec<HanjaEntry>>` 키가 1글자).
  - 후보 있으면 `PopupState::new_hanja_with_top_row` 생성 + `PopupAction::ShowHanja` 발행.
  - 후보 없고 초성이면 특수문자 검색으로 자동 전환(74-104행).
- `select_hanja(&mut self, index) -> Option<String>` — `src/input_engine/candidates.rs:140-160`. `preedit_cache`/`korean_context`를 `clear()`하고 한자 문자열만 반환 — **호출자가 commit_buffer에 push**(§2.4).
- `toggle_hanja_bookmark(&mut self, index) -> Option<(usize,bool,bool)>` — `src/input_engine/candidates.rs:186-263`. 토글 후 `hanja_dict.search`로 재조회+재정렬, `popup_state.replace_hanja_items`+`set_selected_global` 호출, `PopupAction::HanjaCandidatesReordered` 자체 emit.

### 2.6 Surrounding text / 선택 영역 (대상② 후보 인프라 — **이미 존재**)

- `surrounding_text: String`, `surrounding_cursor: u32`, `surrounding_anchor: u32` 필드 — `src/input_engine/surrounding.rs:70-89`.
- `set_surrounding_text(text, cursor_pos, anchor_pos)` — `src/input_engine/surrounding.rs:70-80`. **`cursor_pos != anchor_pos`면 앱에서 텍스트가 선택된 상태** — 대상②(앱에서 선택된 한글 단어) 트리거를 만들 때 그대로 재사용 가능한 필드.
- `typefix_convert(&mut self, direction: u32) -> Option<(i32, u32, String)>` — `src/input_engine/surrounding.rs:183-272`. **선택 영역(cursor≠anchor)에서만 동작**(192-195행), 반환값 `(offset_from_cursor, delete_chars, replacement)` — **이 3-튜플이 "이미 커밋된 텍스트를 지우고 다른 텍스트로 교체"하는 프로토콜의 SoT**. AutoTypeFix가 이 값을 소비하는 곳:
  - Windows: `unim-tsf/src/key_handler.rs:422-434` — `engine.typefix_convert(0)` 호출 후 `delete_count`만큼 지우고 `replacement` 삽입.
  - Linux 글로벌: DBus RPC `TypeFix(direction)` — `unim-dbus/src/service.rs:570-599`(`EngineRequest::GlobalTypeFix` 경유 `unim-dbus/src/engine_worker.rs:2281-2305`). 응답 `(offset, delete, replacement)`를 받은 **클라이언트**(GNOME extension 등)가 실제 N+1 BS/삭제 키 입력을 수행 — 그 소비 코드는 이번 조사 범위 밖(다른 서브시스템), `rg "type_fix\(" unim-gnome-extension/*.js`로 위치 확인 가능.
- `smart_backspace(&self) -> Option<(u32, String)>` — `src/input_engine/surrounding.rs:105-161`. 커밋된 한글을 자모 단위로 지우는 유사 패턴이지만 **미배선(주석 명시, 104행: "현재 호출자는 테스트뿐")**.

### 2.7 DBus payload 타입 (단일 SoT)

- `PopupRenderPayload` — `unim-popup-types/src/lib.rs:44-74`. `cells: Vec<(String,String,u32)>` column-major, flags 비트 `unim-popup-types/src/lib.rs:77-83`(`HAS_DATA=0x01, SELECTED=0x02, COL_HIGHLIGHT=0x04, ROW_HIGHLIGHT=0x08, BOOKMARKED=0x10`). **cell text 필드는 `String`이라 다글자 허용** — 프로토콜 레벨 제약 없음.
- `HanjaCandidateResponse` — `unim-popup-types/src/lib.rs:86-97`: `target, candidates: Vec<(String,String)>, top_row, render_state: Option<PopupRenderPayload>`.
- `build_render_state(engine) -> Option<PopupRenderPayload>` — `unim-dbus/src/engine_worker.rs:437-517`. `PopupState::view_model()` 결과를 column-major로 평탄화. **로직만 봐서는 다글자 `text`도 그대로 통과**(문자열 그대로 clone, 448행대에서 길이 가정 없음).

### 2.8 DBus 시그널/RPC 시그니처

- `show_hanja_popup(target:&str, candidates:Vec<(String,String)>, top_row:&str, cursor_x/y/w/h)` — `unim-dbus/src/service.rs:2469-2478`. **internal-only**: popup-service의 forward 전용, 외부 frontend는 구독 안 함(주석 2452-2460행).
- `popup_render(kind:u32, texts:(target,header,footer,expand), layout:(rows,cols,sel_row,sel_col,page,total_pages), flags:(show_footer,expand_visible), cells:Vec<(text,meaning,flags_bits)>, col_headers, row_headers, tab_labels, active_tab_index)` — `unim-dbus/src/service.rs:2552-2565`. **이것이 실제 렌더링 SoT 시그널**(모든 프런트엔드가 구독).
- `hanja_bookmark_changed(index:u32, bookmarked:bool)` — `unim-dbus/src/service.rs:2567-2573`.
- `hanja_candidates_reordered(target, hanjas, meanings, bookmarks, new_cursor, page, sel_row, sel_col, bookmarked, was_bookmarked)` — `unim-dbus/src/service.rs:2582-2596`.
- RPC 메서드: `get_hanja_candidates()->(target,candidates)` (`:2830-2882`), `select_hanja(index)->hanja:String`(`:2886-2920`), `get_hanja_bookmark_states()->Vec<bool>`(`:2926-2947`), `toggle_hanja_bookmark(index)->(new_index,bookmarked)`(`:2957-`), `popup_change_page(direction)`(`:3038-`), `toggle_popup_expand()`(`:3099-`), `cancel_hanja()`(`:3147-`).
- `PopupAction` enum(엔진→DBus 매개 타입) — `src/input_engine/types.rs:124-217`: `ShowHanja/ShowSpecial/ShowEmoji/HidePopup/PopupNavigate/HanjaBookmarkChanged/HanjaCandidatesReordered/PageJump`.
- `EngineRequest` 처리 매핑(engine_worker.rs) — `GetHanjaCandidates`(1979-2011), `SelectHanja`(2013-2028, **popup-owner로 라우팅**), `CancelHanja`(2030-2045), `GetHanjaBookmarkStates`(2047-2057), `ToggleHanjaBookmark`(2059-2080), `PopupChangePage`(2082-2110), `TogglePopupExpand`(2112-2130대).
- `resolve_popup_owner(contexts, caller) -> u32` — `unim-dbus/src/engine_worker.rs:529-542`. 호출 context에 popup이 없으면 popup이 활성인 다른 context로 라우팅(예: GNOME extension 자체 context vs 실제 GTK4_IM context).

### 2.9 렌더러별 셀 폭 가정

| 렌더러 | 파일:line | 셀 폭/글자수 가정 |
|---|---|---|
| GTK4 compact(list) | `unim-popup-service/src/popup/hanja.rs:314-385` | `hanja_label`/`meaning_label`에 고정폭 없음(자동 크기) — **다글자 안전** |
| GTK4 expanded(grid) | `unim-popup-service/src/popup/hanja.rs:390-462` | `cell.set_label(hanja)`, CSS `.grid-cell{min-width:30px}`(`popup_styles.generated.css:176-181`) — **min만 지정, 다글자도 렌더는 되나 열 정렬이 셀마다 달라짐(시각적 들쭉날쭉)** |
| Windows Direct2D compact | `unim-popup-win/src/render.rs:307-329` | `max_w` 를 각 행의 `hanja+meaning` 실측 폭으로 동적 계산 — **다글자 안전** |
| Windows Direct2D expanded | `unim-popup-win/src/render.rs:197,339,588-636` | `CELL_W: i32 = 44`(고정 상수) — **다글자 단어가 4~5자면 44px 셀에서 잘리거나 겹칠 위험** |
| GNOME extension compact | `unim-gnome-extension/popup_view.js:351-382` | `St.Label` 자동 크기, `x_expand` — **다글자 안전** |
| GNOME extension expanded | `unim-gnome-extension/popup_view.js:383-386`(특수/이모지) vs 351-382(한자, isHanjaCompact 분기 없이 항상 inner BoxLayout 사용) | CSS `.grid-cell{min-width:30px}` 공유 — GTK와 동일 위험 |

## 3. 현재 동작 흐름 (단계별, 호출 순서)

### 3.1 한자키 → 팝업 표시 (Embedded, 즉 IM 모듈이 직접 키를 받는 경우 — push 방식)
1. 사용자가 한국어 모드에서 한자키(F9/Hanja) 입력.
2. `InputEngine::start_hanja_conversion()` (`src/input_engine/candidates.rs:16`) 호출.
3. `preedit_cache`의 마지막 1글자를 target으로 결정(§2.5).
4. `hanja_dict.search(target)` → 후보 `Vec<HanjaEntry>`.
5. 즐겨찾기 우선 stable sort.
6. `PopupState::new_hanja_with_top_row(target, pairs, top_row)` 생성 → `self.popup_state = Some(..)`.
7. `self.popup_pending_action = Some(PopupAction::ShowHanja{target, candidates, top_row})`.
8. `InputResult::hanja_candidates()` 반환 → 호출자(각 IM 모듈의 key handler)가 이 결과를 보고 `take_popup_action()`으로 pending action을 꺼냄.
9. DBus 계층(`unim-dbus/src/service.rs:1955-1982`)이 `PopupAction::ShowHanja`를 받아 `show_hanja_popup` 시그널(internal) 발행 + (Standalone 한정) `emit_popup_render`.
10. popup-service(GTK4)의 `forward_daemon_popup_signals`가 internal 시그널을 구독해 자신의 `org.atit.unim.Popup` 인터페이스로 재발행(주석 `unim-dbus/src/service.rs:2452-2460`).
11. 각 프런트엔드(GTK4 popup-service / GNOME extension `popup_view.js`)가 `popup_render` 시그널을 구독해 그리기.

### 3.2 Pull 방식 (Standalone, GetHanjaCandidates RPC)
1. 클라이언트가 `GetHanjaCandidates()` RPC 호출 → `unim-dbus/src/service.rs:2830`.
2. `EngineRequest::GetHanjaCandidates` 전송 → `engine_worker.rs:1979`에서 `engine.start_hanja_conversion()` 직접 호출(3.1의 2~7단계와 동일 내부 로직).
3. `engine.take_popup_action()`으로 pending action 소비·폐기(중복 방지, 1993행).
4. `build_render_state(engine)`로 `PopupRenderPayload` 생성.
5. `HanjaCandidateResponse{target, candidates, top_row, render_state}` 반환.
6. 서비스가 `show_hanja_popup` + `emit_popup_render` 발행(2857-2874행).

### 3.3 키 네비게이션 (팝업 열린 상태)
1. `InputEngine::process_popup_key(keycode, modifier, config)` (`src/input_engine/popup_dispatch.rs:75`).
2. `keycode_to_popup_key` 변환 → `popup_state.handle_key(popup_key)` → `PopupKeyResult`.
3. 결과 분기(§2.4)로 `PopupAction::{PopupNavigate, HanjaCandidatesReordered, HidePopup}` 등 발행.
4. DBus가 해당 시그널로 변환 발행 → 프런트엔드가 `navigate()`/`update_from_render()` 호출해 갱신.

### 3.4 선택 → 커밋
1. `Enter`/숫자키/마우스 클릭 → `PopupKeyResult::Select(abs_index)` 또는 RPC `SelectHanja(index)`.
2. `popup_select(abs_index)` (push 경로) 또는 `select_hanja(index)` 직접 호출(RPC 경로, `unim-dbus/src/service.rs:2886-2920`).
3. `select_hanja`가 `preedit_cache`/`korean_context`를 비우고 한자 문자열 반환.
4. push 경로: `commit_buffer.push_str(&hanja)` 후 `InputResult::committed()`.
   RPC 경로: `redirect_commit_and_hide(&hanja)` 호출(`unim-dbus/src/service.rs:2910`) — commit + hide를 popup-owner의 실제 입력 대상 path로 리다이렉트.
5. `PopupAction::HidePopup` → 모든 프런트엔드 hide.

### 3.5 즐겨찾기 토글 (Space / 우클릭)
1. `PopupKeyResult::ToggleBookmark(idx)` 또는 RPC `ToggleHanjaBookmark(idx)`.
2. `toggle_hanja_bookmark(idx)`(`src/input_engine/candidates.rs:186`): 북마크 플립 → `hanja_dict.search` 재조회 → 북마크 우선 재정렬 → 새 인덱스 탐색 → `popup_state.replace_hanja_items`+`set_selected_global`.
3. `PopupAction::HanjaCandidatesReordered` 발행 → 모든 열린 팝업이 후보+북마크+커서를 일괄 교체, `was_bookmarked&&!bookmarked`면 140ms yellow flash(`unim-popup-service/src/popup/hanja.rs:27-29`, POPUP_SPEC.md §3.7-9).

### 3.6 취소
- `Escape`/포커스 상실/알 수 없는 키 → `popup_cancel()`(push) 또는 `CancelHanja()` RPC(`unim-dbus/src/service.rs:3147-`) → `cancel_hanja()`가 `hanja_mode/hanja_candidates/hanja_target/popup_state`를 모두 클리어, 원래 한글(`hanja_target`)을 commit_buffer에 복원.

## 4. 이 기능을 위한 확장 지점 (어디를 어떻게, 위험도)

### 4.1 대상①(방금 입력한 내용: 커�밋+preedit 결합) — **핵심 변경 필요 지점**

- **문제**: `start_hanja_conversion()`(`src/input_engine/candidates.rs:22-29`)이 target을 `preedit_cache`의 **마지막 1글자만** 본다. 커밋된 텍스트("대한민")는 이미 `commit_buffer`를 거쳐 앱으로 전송된 뒤 엔진 메모리에서 사라진다(`commit_buffer`는 매 사이클 flush되는 출력 버퍼, 히스토리 아님 — 필드 선언 `src/input_engine/engine.rs:100`). "최근 커밋 음절을 기억"하는 저장소가 **엔진에 없다**.
- **확장 방법 A (엔진 내부 히스토리)**: `InputEngine`에 `recent_committed: String`(또는 `VecDeque<char>`, 어절 경계까지) 필드를 추가하고, commit_buffer가 flush될 때마다 append. 한자키 트리거 시 `format!("{}{}", recent_committed_tail, preedit_last_syllable)`로 target을 구성해 `hanja_dict.search`를 어절 단위로 확장 검색(사전 자체가 음절 단위이므로 **단어 사전이 별도로 필요** — §4.5 참조). 리셋 조건: 어절 경계(공백/구두점 커밋), 모드 전환, `reset()`(`src/input_engine/engine.rs:761-`) 시 클리어해야 함 — 안 하면 문맥 오염(오래된 커밋이 엉뚱하게 결합) 위험.
- **확장 방법 B (surrounding_text 재사용, 권장)**: `surrounding_text`/`surrounding_cursor`(§2.6, `src/input_engine/surrounding.rs:70-89`)가 이미 "커서 앞 텍스트"를 보관한다. IM 모듈들이 `set_surrounding_text`를 호출하는 빈도·타이밍이 프런트엔드마다 다를 수 있어(예: XIM은 spot location만 있고 surrounding text 미지원인 경우 있음 — 확인 필요, §7) 신뢰성 검증이 선행돼야 하지만, 새 상태 필드를 안 늘리고 기존 인프라를 재사용할 수 있는 유일한 경로.
- **위험도: 높음.** `start_hanja_conversion`은 특수문자 자동 폴백(§2.5, 74-104행)과 로직이 얽혀 있어, target을 "음절"에서 "어절"로 바꾸면 `search_by_choseong`(초성 검색) 분기 조건도 재검토해야 한다. 또한 POPUP_SPEC.md §3.7 규칙 2번("대상: preedit의 마지막 음절")을 정면으로 변경하는 것이므로 **문서 갱신은 사용자 승인 필수**(메모리 `feedback_popup_spec_absolute.md`).

### 4.2 대상②(앱에서 선택된 한글 단어)

- **트리거 인프라는 이미 존재**: `surrounding_cursor != surrounding_anchor`(`src/input_engine/surrounding.rs:70-89`)로 선택 여부 판정 가능 — `typefix_convert`가 정확히 이 패턴(192-195행)을 쓴다.
- **신규 진입점 필요**: 한자키가 눌렸을 때 "선택 영역이 있으면 그 선택 텍스트로, 없으면 기존 음절 로직으로" 분기하는 코드가 `start_hanja_conversion` 안에 **없음**(확인: 함수 전체를 읽었으며 `surrounding_anchor`/`surrounding_cursor` 참조 없음). 새 분기를 추가해 선택 텍스트를 `hanja_dict`(또는 신규 단어 사전)에 조회해야 한다.
- **위험도: 중간.** 순수 추가(새 if 분기)라 기존 음절 경로를 건드리지 않고 넣을 수 있음. 다만 "선택 영역이 한글 음절 경계와 일치하는지" 검증(부분 음절 선택 시 깨진 문자 조합) 로직이 필요.

### 4.3 선택 시 치환 커밋 (기존 커밋 텍스트를 지우고 한자로 교체)

- **현재 `popup_select`(`src/input_engine/popup_dispatch.rs:190-222`)는 "preedit을 지우고 commit_buffer에 새로 push"만 지원** — 이미 앱에 전송된 텍스트를 지우는 기능이 전혀 없다.
- **재사용 가능한 기존 프로토콜**: `typefix_convert`의 반환 튜플 `(offset_from_cursor, delete_chars, replacement)`(`src/input_engine/surrounding.rs:183`) — 정확히 "N+1 BS/삭제 후 교체"가 필요로 하는 형태. 이 튜플을 소비하는 기존 코드:
  - Windows: `unim-tsf/src/key_handler.rs:422-434`.
  - Linux 글로벌 RPC: `unim-dbus/src/service.rs:570-599`(`type_fix` RPC) → 프런트엔드가 delete_chars만큼 backspace(또는 surrounding-text delete API)를 실행 후 replacement 삽입.
- **확장 방법**: 대상②(선택 커밋) 경로에서 한자 선택 시 `PopupAction`에 새 variant(예: `ReplaceSelection{delete_chars, replacement}` 혹은 기존 `(offset,delete,replacement)` 튜플 형태 재사용)를 추가하고, `unim-dbus/src/service.rs`에 대응 시그널/RPC를 추가한 뒤, 각 프런트엔드가 `type_fix`용으로 이미 구현한 backspace-and-insert 로직을 그대로 호출하도록 배선. **이 소비자 코드 자체(GNOME extension의 backspace 시퀀스 등)는 이번 조사 범위 밖**이므로 실제 구현 시 별도 조사 필요(`rg "type_fix" unim-gnome-extension/*.js`로 시작점 확인).
- **위험도: 높음.** 신규 `PopupAction` variant 추가는 `unim-dbus/src/service.rs`의 매치 문(1956행부터) + `unim-popup-win/src/protocol.rs`(WireMsg, 하위호환 요구사항 `#[serde(default)]` 필수, 1-3행 주석) + GNOME extension 세 곳을 동시에 건드리는 5지점 동기화급 작업(하네스 발동 조건 메모리 참고).

### 4.4 팝업 페이지네이션(9단어/페이지)·즐겨찾기

- **그대로 재사용 가능**: `HANJA_PAGE_SIZE=9`(`src/popup/popup_keys.rs:85`)와 즐겨찾기 배선(`bookmarks: Vec<bool>`, `toggle_hanja_bookmark`, `HanjaCandidatesReordered`)은 candidate가 "한자 1글자"든 "한자 단어(여러 글자)"든 구조적으로 무관하다 — `PopupState`/`PopupViewModel`/`PopupRenderPayload`의 `items`/`cells` 필드가 모두 `String`이라 다글자 후보를 그대로 통과시킬 수 있다(§2.1, §2.3, §2.7).
- `HanjaBookmarks::is_bookmarked(target, hanja)`(`src/input_engine/candidates.rs:36,52,171` 등에서 호출) — 북마크 키가 `(target, hanja)` 쌍이므로 target이 "한" 대신 "대한민국"이어도 그대로 동작. 단, 북마크 저장소 구현(`HanjaBookmarks` 구조체, 이번 조사 범위 밖 파일)이 문자열 길이에 대한 가정을 갖고 있는지는 미확인 — `rg "struct HanjaBookmarks"`로 별도 확인 필요(§7).
- **위험도: 낮음.** 구조적으로 이미 다글자 대응.

### 4.5 출력 형식 선택(漢字 / 한자(漢字) / 漢字(한자))

- **현재 코드에 해당 설정 없음**(확인: `rg "output_format|hanja_format|漢字|OutputFormat" src/config.rs src/input_engine/candidates.rs` → 매치 없음).
- 삽입 지점 후보: `select_hanja()`(`src/input_engine/candidates.rs:140-160`)가 반환하는 문자열을 조립하기 직전 — 여기서 `config.hanja.output_format`(신규 설정) 에 따라 `"{hanja}"`/`"{hangul}({hanja})"`/`"{hanja}({hangul})"` 포맷팅.
- **주의**: 사용자 메모리 `feedback_config_3way_sync.md` — 신규 설정은 엔진(`src/config.rs`)·GUI·CLI 3곳 동시 배선 필수. 이번 조사 범위(팝업 파이프라인) 밖이지만 설계 시 반드시 별도 서브시스템 작업으로 계상.

### 4.6 렌더러 셀 폭 (다글자 단어 표시)

- compact(1열 리스트) 모드는 3개 렌더러 모두 자동 폭 — **변경 불필요**.
- expanded(9×9) 모드는 GTK(`min-width:30px`)/GNOME(동일 CSS 공유)/Windows(`CELL_W=44px` 고정, `unim-popup-win/src/render.rs:197`)에서 다글자 단어가 셀을 벗어나거나 열 정렬이 깨질 위험 — **한자 단어 후보는 expanded 그리드 모드에서 노출하지 않거나(컴팩트 강제), Windows는 `CELL_W`를 동적 계산하도록 바꿔야 함**. 두 가지 중 전자가 훨씬 저비용.

## 5. 지켜야 할 규칙 (AGENTS.md·POPUP_SPEC·주석에서 인용, file:line)

- `docs/dev/specs/POPUP_SPEC.md:237` — "**대상**: preedit의 마지막 음절 (예: "대한민국" → "국")" — 대상①을 구현하려면 **이 규칙 자체를 개정**해야 한다. 개정은 사용자 승인 필수(아래 항목).
- 사용자 메모리 `feedback_popup_spec_absolute.md`(요약, MEMORY.md 인덱스) — "POPUP_SPEC.md 규칙은 예외 없이 준수, 변경 시 사용자 승인 필수."
- `docs/dev/architecture/AGENTS.md:48-73` — "팝업 아키텍처(0.3.0 — 단일 SoT)": `PopupRender` payload가 유일한 view-model SoT, 셀·헤더·푸터·탭·하이라이트 모두 daemon이 산출. 신규 필드/포맷은 **daemon 쪽에서 결정**하고 프런트엔드는 그대로 그리기만 해야 한다 — 프런트엔드에 표시 로직(예: 출력 형식 문자열 조립)을 넣지 말 것.
- `unim-popup-win/src/protocol.rs:1-3` — "필드 추가는 반드시 `#[serde(default)]`(하위호환) + 설계서 갱신 + v 유지." Windows wire 프로토콜에 필드 추가 시 반드시 지킬 것 (§4.3의 신규 액션이 여기 해당).
- `src/popup/popup_keys.rs:458-461`(주석) — "expanded(9x9)에서만 special과 동일한 열 점프 동작. compact(1열)는 NotHandled로 남겨 회귀 방지." — 키 처리 확장 시 compact/expanded 분기를 흩트리지 말 것.
- `src/popup/popup_layout.rs:13-20`(주석) — rows를 9로 고정하는 이유(과거 회귀 이력: column-major 인덱싱과 시각 인덱싱 불일치) — 그리드 레이아웃 상수를 함부로 가변으로 바꾸지 말 것.
- 사용자 메모리 `feedback_config_3way_sync.md` — 신규 설정(§4.5 출력 형식)은 엔진/GUI/CLI 3곳 동시 동기화.
- 사용자 메모리 `feedback_force_unim_harness.md` — 5지점 동기화·신기능·멀티 컴포넌트급 작업(§4.3, §4.1)은 PM 하네스 라우팅 대상.

## 6. Windows 동등성 메모

- Windows는 별도 크레이트(`unim-popup-win`)가 **named pipe JSON wire 프로토콜**(`WireMsg`/`RenderState`, `unim-popup-win/src/protocol.rs:38-98`)로 daemon 역할의 `unim-tsf`와 통신 — DBus가 아니라 자체 IPC. `RenderState`는 `PopupRenderPayload`와 필드가 1:1 대응(column-major cells, flags 비트 동일값 `unim-popup-win/src/protocol.rs:12-18` vs `unim-popup-types/src/lib.rs:77-83`)이라 **논리적으로는 같은 SoT를 복제**하고 있음 — 신규 필드 추가 시 두 정의(`unim-tsf/src/popup_ipc.rs`와 `unim-popup-win/src/protocol.rs`, 주석 2행에 "양쪽 크레이트 동일 정의" 명시)를 **동시에** 고쳐야 하며 누락하면 컴파일은 되지만 런타임에 조용히 필드가 빠짐(`#[serde(default)]`라 에러 안 남).
- expanded 그리드 셀 폭이 Windows만 고정 상수(`CELL_W=44`, `unim-popup-win/src/render.rs:197`)이고 GTK/GNOME은 CSS `min-width`(가변) — 플랫폼 간 이미 비대칭. 한자 단어 기능에서 다글자 후보를 다루려면 이 비대칭이 가장 먼저 터질 지점(Windows expanded 모드에서 4~5글자 단어가 44px에 안 들어감).
- `typefix_convert`(§2.6)의 Windows 소비자는 `unim-tsf/src/key_handler.rs:422-434`에 이미 있음 — 대상②의 "선택 텍스트 치환" 커밋을 만들 때 Windows 쪽은 **이미 존재하는 소비 패턴을 그대로 재사용 가능**(대상②를 위해 새로 만들 `PopupAction`도 이 패턴을 흉내내면 3플랫폼 동등성이 자연히 맞춰짐).
- Windows 렌더러 쪽에는 `unim-gui-common`의 `PopupModel`(§1의 "미사용" 항목) 같은 대응 구조가 아예 없고, `RenderState`를 직접 파싱해서 그린다(`unim-popup-win/src/render.rs`) — GTK/GNOME과 코드 공유가 안 되고 각자 구현이라, 한자 단어 확장 시 **3곳(GTK, GNOME JS, Windows Direct2D)을 각각 손대야 함**.

## 7. 미해결 질문

1. `HanjaBookmarks` 구조체(북마크 저장소, 파일 위치 미확인 — `rg "struct HanjaBookmarks"` 필요)가 `(target, hanja)` 키의 `target` 문자열 길이나 형식에 가정을 갖고 있는가? 다글자 단어를 target으로 쓸 때 저장 포맷(JSON/TOML 등)이 깨지지 않는지 확인 필요.
2. `hanja_dict`(음절 단위)와 별도로 **단어 단위 한자 사전**을 신설해야 하는데, 이 사전의 데이터 소스/빌드 방식(현재 `include_str!("../data/hanja.txt")`, `src/hanja/dict.rs` 근방)을 어떻게 확장할지는 이번 조사 범위 밖 — 별도 서브시스템(사전/데이터) 조사 필요.
3. 각 IM 모듈(XIM/GTK3/GTK4/Qt5/Qt6/Wayland/TSF)이 실제로 `set_surrounding_text`를 얼마나 자주·정확하게 호출하는지(특히 XIM처럼 surrounding text 프로토콜 지원이 제한적인 환경) 확인되지 않음 — 대상①/②의 신뢰성이 여기 달려있음.
4. `commit_buffer`가 매 사이클 언제 "flush"되어 앱으로 전송되는지(정확한 타이밍/호출 지점)는 이번 조사에서 깊이 보지 않음 — 대상① "최근 커밋 음절 기억" 설계 시 정확한 커밋 이벤트 훅 지점이 필요.
5. 대상②(선택 커밋)에서 선택 영역이 여러 어절에 걸치거나 한글이 아닌 문자가 섞인 경우의 처리 정책(무시? 부분 변환?)이 기획 단계에서 정의됐는지 미확인.
6. `unim-frontends/gtk-common`(C, GTK3/4 공통 코드, `docs/dev/architecture/AGENTS.md:25`)이 이번 조사에서 다루지 않은 4번째 렌더 경로인지, 아니면 `unim-popup-service`로 완전히 대체됐는지 — AGENTS.md는 "GTK 팝업 등"이라고만 언급, 실제 소스 확인 필요.

## 보충 #7: 다음절 항목(275,020건) meaning="" 렌더링 + 사전 순서 전제 + 커밋 버퍼 상한

### (1) compact 렌더러 3종의 빈 meaning 처리

- **GTK4** `unim-popup-service/src/popup/hanja.rs:343-347`: `gtk4::Label::new(Some(meaning))` — `meaning`이 `""`이어도 `Some("")`로 라벨을 항상 생성한다(빈 문자열 자체는 유효한 `Some` 값). `hexpand(true)` + `set_ellipsize(EllipsizeMode::End)`(348)까지 걸려 있지만 빈 텍스트라 자연폭 0 — 예외 처리 없이 "그냥 안 보임"으로 우아하게 축소된다.
- **GNOME 확장** `unim-gnome-extension/popup_view.js:365-375`: 유일하게 **명시적 주석 있는 처리**. `text: meaning || ''`로 빈 문자열을 hexpand spacer로 명시 사용, "★가 항상 우측 끝에 고정되게" 의도적으로 설계됨(361-363 주석). `clutter_text.set_ellipsize(Pango.EllipsizeMode.END)`(373)도 적용.
- **Windows** `unim-popup-win/src/render.rs:519-520`: `draw_text(hdc, &cell.m, &mean_rect, ...)` — `cell.m`이 빈 문자열이면 GDI가 그냥 아무것도 안 그린다. `mean_left = hanja_left + s(96, scale)`(518, 질문의 "96px 오프셋") 컬럼 위치는 **고정**이라 meaning 유무와 무관하게 항상 예약된다.
- **결론**: 3종 모두 크래시·레이아웃 붕괴 없이 "한자만 보이는 행"으로 우아하게 축소된다. 지도의 "compact 는 변경 불필요" 판단은 **이 축(빈 meaning)에 한해서는 유효**하다.

### (2) expanded 헤더 `「target」 → {한자} {뜻}` — 빈 뜻은 처리됨, 4자+ target 은 미처리(신규 위험)

- `src/popup/view_model.rs:305-314`: 이미 `meaning.is_empty()` 분기가 있어 `"「{target}」 → {hanja}"` (뜻 생략)로 정확히 처리된다 — 빈 뜻 축은 문제 없음.
- **문제는 target 길이다.** `header_text`는 `self.target()` 원문 문자열을 그대로 보간하며, 3개 렌더러 **어디에도 이 헤더 라벨에 ellipsize/wrap 설정이 없다**(meaning 라벨에는 3곳 다 있었던 것과 대조):
  - GTK4: `target_label`(hanja.rs:102-107)에 `set_ellipsize` 호출 없음(rg 결과 348행 `meaning_label` 만 존재). CSS `.unim-hanja-popup`(popup_styles.generated.css:14) 은 `max-width: 420px` 로 팝업 자체 폭을 제한 — 헤더 텍스트가 길면 ellipsis 없이 GTK 레이아웃 클리핑(잘림)만 발생.
  - GNOME JS: `this._header`(popup_view.js:98) 생성 시 `ellipsize` 미설정(rg: `clutter_text.set_ellipsize` 는 373행 `meaningLbl` 에만 존재).
  - Windows: `draw_text(hdc, &rs.header_text, &hr, ..., DT_VCENTER | DT_SINGLELINE | DT_LEFT)`(render.rs:375) — `DT_END_ELLIPSIS` 플래그 없음. `hr` 폭은 팝업 고정 폭(`w`) 기준이라 긴 문자열은 그냥 픽셀 단위로 잘려 보인다.
- **다음절 사전은 4자 이상 target 이 드물지 않다**(9~18자 항목 2,124건, `hanja.txt` 다음절 275,020건 중 0.77%; 대부분은 2~7자). expanded 헤더는 `target`(한글)과 `hanja`(한자, 보통 target 과 동일 글자 수)를 **동시에** 이어붙이므로 8자 target 이면 헤더 문자열이 20자 안팎(「」→ 기호 포함)이 되어 420px 팝업 폭을 넘긴다.
- **결론**: 지도의 "팝업은 변경 불필요" 판단은 **이 축(긴 target 헤더)에서는 성립하지 않는다** — 빈 meaning 은 이미 처리돼 있지만, 4자 이상 target 의 헤더 오버플로우는 3플랫폼 공통 미검증 신규 위험이며 한자 단어 기능이 오면 즉시 노출된다.

### (3) 사전 순서=빈도순 전제(candidates.rs:33-34, dict.rs:104-108, POPUP_SPEC.md:238)

- `src/hanja/dict.rs:74-88`: 파싱은 `entries.entry(hangul).or_default().push(entry)` — 동일 키의 값 순서는 **`hanja.txt` 파일 내 등장 순서(라인 순)** 그대로 보존된다(HashMap 은 키 순서만 임의이고 값 Vec 순서는 insertion 순서 유지).
- `dict.rs:104-108` 주석 "결과는 빈도순(사전 내 순서)으로 정렬되어 있습니다"과 `docs/dev/specs/POPUP_SPEC.md:238` "사전 저장 순서(빈도순)"는 **코드 주석상의 주장일 뿐, 이를 뒷받침하는 메타데이터·빈도 필드는 저장소 어디에도 없다**(`hanja.txt` 헤더는 2005/2006 Choe Hwanjin 저작권 고지만 있고 정렬 근거 문서 없음, `head -30 src/data/hanja.txt` 확인).
- 실측: 다음절 중복 키 예시 `국가:國家:` / `국가:國歌:`(hanja.txt:28636-28637, 질문 원문의 "동음 다중 항목") — 國家(나라)가 國歌(애국가)보다 앞. 반면 `가가` 키의 5개 항목(假家/可呵/可嘉/家家/呵呵)은 한자 코드포인트 오름차순도, 명백한 다른 규칙도 따르지 않음(呵 U+5478 이 家 U+5BB6 뒤에 옴) — **단순 유니코드 정렬 가설은 기각**.
- **결론**: "사전 순서=빈도순" 전제는 **다음절 항목에서 검증 불가**(반증도 못 하지만 입증할 근거도 없음) — 원본 libhangul BSD 사전의 컴파일 순서를 그대로 물려받은 것으로 보이며, 빈도 기반이라는 주장은 코드 주석의 미검증 가정이다. 한자 단어 기능이 이 순서에 후보 랭킹을 의존하면 **품질이 원 저작자의 임의 순서에 좌우**된다는 리스크를 설계 문서에 명시해야 한다.

### (4) 최근 커밋 버퍼 상한 권고

- 실측 최장 다음절 키 = **18자**(`청룡기쟁탈전국고등학교야구선수권대회`, hanja.txt:253444, python `len()` 기준 문자 수 — awk 바이트 기준은 78로 오판되므로 주의).
- 길이 분포(다음절 275,020건 중): 8~18자 = 2,124건(0.77%), 2~7자 = 272,896건(99.2%). 즉 절대다수는 짧다.
- 대상①(최근 커밋+현재 preedit 결합) 설계상 조회 키는 "최근 커밋 N자 + 현재 preedit 마지막 1자" 형태가 될 것이므로, **사전 최장 키(18자)와 정확히 매치하려면 커밋 버퍼는 최소 17자**를 보관해야 한다(18 - preedit 1자).
- **권고**: 버퍼 상한을 **17자**로 잡는다 — 메모리 비용은 무시할 수준(문자 17개, UTF-8 최대 4바이트×17 ≈ 68바이트)이므로 굳이 8~10자로 줄여 0.77%의 항목(18자짜리 기관명·대회명 등 실사용 빈도 극히 낮은 항목)을 놓칠 이유가 없다. 다만 이는 "상한" 문제일 뿐이고, 실제 정확도는 버퍼를 언제 리셋(공백/구두점/포커스 전환/비한글 입력)하는지에 더 좌우된다 — 이 리셋 정책은 미해결 질문 #4(§7)로 남아 있다.
