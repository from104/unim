# UNIM Windows TSF — 한자/특수문자/이모지 후보 팝업 정상화 계획

> **작성 (2026-06-12, feat/windows-msi-redesign)**: 사용자 보고 "후보 팝업이 전반적으로 부실하거나 동작을 안 한다"에
> 대한 전수 조사 결과와 수정 로드맵. 코드 수정 0건 — 분석·계획 문서.
> 조사 트랙: Linux 기준 스펙(POPUP_SPEC) / 엔진 계약(popup_dispatch) / TSF 코드 감사 /
> MS 공식 문서(UIElement·GetTextExt) / 외부 사례(SampleIME·Mozc·Weasel·DIME·Chewing).
> 가설 17건 중 **16건 confirmed, 1건 refuted**(부록 참조).

핵심 결론 한 줄: `popup_window.rs` 649줄이 "완성돼 보이는데 부실"한 이유는 미세 버그 누적이 아니라
**①edit-session 규약 위반(위치) ②UIElement 프레임워크 부재(표시 차단) ③엔진-렌더 좌표계 계약 불일치(내용/선택)
④수명주기 상태 비대칭(reset이 팝업 모드를 안 푸는 것)** — 4개 축의 구조적 결함이다.

---

## 1. 증상 정리


| #   | 증상 분류           | 사용자 체감                                                                                                              | 원인 가설             |
| --- | --------------- | ------------------------------------------------------------------------------------------------------------------- | ----------------- |
| S1  | **위치 실패**       | 팝업이 캐럿이 아닌 마우스 커서 위치에 뜸. 마우스가 화면 구석/다른 모니터면 "안 뜨는 것"으로 보임. 화면 하단·우측에서 화면 밖으로 잘림                                     | H1, H11, H12, H13 |
| S2  | **표시 차단**       | UWP/스토어앱/Windows 검색창/풀스크린 게임에서 팝업이 아예 안 뜸                                                                           | H2                |
| S3  | **내용·선택 불일치**   | 화살표 이동이 엇나가 보이고, 숫자/Q~~O 선택 시 화면에 보이는 것과 **다른 문자가 커밋**됨. 선택 하이라이트가 엉뚱한 셀에 칠해지거나(1페이지) 아예 안 보임(2페이지~~)               | H3, H4            |
| S4  | **첫 표시 깨짐**     | 열자마자 격자가 3×3·페이지 1/1로 렌더됨(실제는 한자 compact 9행, 특수/이모지 9×9). 첫 화살표 키 이후에야 정상 격자로 변함                                    | H5                |
| S5  | **내용 부실**       | 한자 뜻풀이·헤더·footer 없음(글자+★+페이지 숫자뿐), 이모지 카테고리 탭/Recent 없음, 기존 즐겨찾기 ★ 첫 표시 안 됨                                         | H8, H9, H14       |
| S6  | **키 오동작**       | 포커스 전환 후 키가 보이지 않게 먹히거나 이전 문서의 글자가 새 문서에 커밋됨(스테일 커밋). 팝업 중 Z/X/C/V/B/N/M 입력 시 raw 문자가 문서에 들어가며 엔진은 팝업 모드 유지(desync) | H6, H7            |
| S7  | **마우스 무반응·잔존**  | 팝업 셀 클릭해도 무반응. 문서 다른 곳 클릭/다른 앱 클릭 후에도 TOPMOST 팝업이 잔존 가능                                                             | H15, H17          |
| S8  | **호스트 앱 종료 위험** | IME 전환/Deactivate 시 PostQuitMessage(0)가 호스트 앱 메시지 펌프에 WM_QUIT 주입 → 앱 통째 종료 가능                                       | H10               |
| S9  | **카테고리 전환 점프**  | 이모지 팝업에서 Tab으로 카테고리 변경 시 창이 마우스 좌표로 점프                                                                              | H8 (+H1)          |


---

## 2. Linux 기준 모델 요약 — TSF가 도달해야 할 목표 스펙

Linux는 daemon이 **완성된 view_model**(PopupRender)을 내려보내고 프런트엔드는 그리기만 한다.
TSF는 엔진의 raw `PopupAction`을 직접 소비하며 view_model 어셈블리를 자체 재구현해야 하는 구조라
아래 항목 전부가 "TSF 쪽 의무"가 된다.


| 항목      | Linux 기준 동작                                                                                        | 근거                                                  | TSF 현황                                              |
| ------- | -------------------------------------------------------------------------------------------------- | --------------------------------------------------- | --------------------------------------------------- |
| 셀 데이터   | `CellData{text, meaning(한자), is_selected, is_col_highlight, is_row_highlight, is_bookmarked(한자)}`  | view_model.rs:11-394                                | text+★만                                             |
| 셀 플래그   | 0x01 data / 0x02 selected / 0x04 col / 0x08 row / 0x10 bookmarked                                  | SPEC:131                                            | 부분(셀 selected 인덱스 자체가 깨짐, H4)                       |
| 헤더      | 한자 compact=target+라벨, 확장=target+선택 한자+뜻, special=target+라벨, emoji=카테고리+라벨                          | SPEC:131                                            | 없음                                                  |
| footer  | 한자=**항상** 표시, special/emoji=total_pages>1일 때만                                                      | view_model.rs:322,371                               | 페이지 숫자만                                             |
| 격자 인덱싱  | **column-major** `idx = col*rows + row` (special/이모지/한자 확장), compact 한자는 row 단독                    | popup_layout.rs:89-92,111-118                       | row-major 렌더(전치됨, H3)                               |
| 이모지 탭   | Recent+8 카테고리 = 9탭, `label_ko (key)`, active_tab_index                                             | gtk_ui.rs:129-135                                   | 없음(payload 폐기, H8)                                  |
| 북마크     | Space 토글: ON=page0 row0 승격·커서 추종, OFF=사전 위치 강등·도착 셀 140ms #f9e2af flash. 초기 ★는 비동기 fetch로 첫 렌더에 표시 | SPEC:240-247, popup_layout.rs:234-296, hanja.rs:271 | flash만 구현, 초기 ★ 없음(H14)                             |
| 위치      | 캐럿 기준 + 화면 경계에서 위로 플립 + 좌우 클램프 + 갭                                                                 | popup_position.rs:55-80                             | raw 좌표 그대로(H11), 캐럿 획득 자체 실패(H1)                    |
| dismiss | 타 창 포커스 상실=cancel, 같은 창 클릭=reset 자동 취소, ESC=cancel, 포커스 상실=CancelHanja                             | SPEC:238,797-799                                    | 문서 전환만(OnSetFocus), 스레드 포커스 상실·문서 내 클릭 미처리(H15,H17) |
| 마우스     | 셀 클릭 선택, 페이지 버튼, 확장 버튼                                                                             | SPEC + popup_change_page RPC                        | 전무(H15)                                             |
| 키 소비    | 팝업 중 비팝업 키 → `PopupKey::Other` → 닫고 키 재처리                                                          | popup_dispatch.rs:25-72,180-185                     | TSF 이중 게이트 구조상 도달 불가(H7)                            |


엔진 계약 요점(양 플랫폼 공통, 공유 crate `src/input_engine/`):

- `press_key()` 후 `take_popup_action()` 1회 — **단일 Option 슬롯**이라 Show*와 PopupNavigate가 동시 도착 불가 (popup_dispatch.rs:20-23).
- 팝업 활성 게이트는 엔진이 소유: `hanja_mode || special_char_mode || is_emoji_popup_active()` → `process_popup_key()` (press_key.rs 최우선 분기).
- `PopupNavigate.selected = state.sel_row()` — **flat 인덱스가 아니라 행 번호**다 (popup_dispatch.rs:166). 프런트엔드는 sel_row/sel_col만 신뢰해야 함.
- 트리거는 hanja_keys(기본 Hanja/F9) dual-purpose 단일 진입점: 조합 중=한자→(후보 없으면 초성 특수문자 fallback), idle=이모지.

---

## 3. 근본 원인 — confirmed 가설 상세

### 3.1 위치 축 (S1)

**[H1·blocker] 캐럿 좌표 획득 전면 실패 — edit cookie 없이 GetSelection/GetTextExt 호출**

- `unim-tsf/src/key_handler.rs:209-230` `get_composition_screen_pos()`가 edit session **밖**(OnKeyDown 스택, text_service.rs:394→441→key_handler.rs:323→491)에서
`context.GetSelection(u32::MAX, 1, …)` (`ec` 자리에 u32::MAX, `ulindex` 자리에 1 — 인자 슬롯까지 어긋남, key_handler.rs:217)과
`view.GetTextExt(tid, …)` (`ec` 자리에 TfClientId, key_handler.rs:225)를 호출.
- TSF 규격상 두 API의 `ec`는 `DoEditSession` 콜백의 cookie여야 하며 그 외 값은 TF_E_NOLOCK → `.ok()?`로 None →
`popup_window.rs:568-574` **GetCursorPos(마우스 좌표) 폴백**. 캐럿 추적 0%.
- 같은 파일 주석(key_handler.rs:213)이 "ulindex: TF_DEFAULT_SELECTION(u32::MAX)"라고 적어 의도-구현 불일치 확정.
- 올바른 패턴은 같은 코드베이스에 이미 존재: `composition.rs:809-816` DoEditSession 내 `GetSelection(ec, TF_DEFAULT_SELECTION, …)`,
`composition.rs:873-892` RequestEditSession(TF_ES_READ|TF_ES_SYNC) 경유.
- preedit 오버레이(key_handler.rs:367)도 동일 함수를 사용 → 같이 깨짐.

**[H11·major] 모니터 클램프/플립 부재**

- `popup_window.rs:559-589` show()는 받은 (x,y)를 그대로 SetWindowPos. MonitorFromPoint/GetMonitorInfo 사용 0건.
- Linux `unim-gui-common/src/popup_position.rs:55-80`의 위로 플립(71-75)+X 클램프(78)에 해당하는 로직 없음.
- update() 리사이즈(popup_window.rs:614-627)도 재클램프 없이 커짐(9×9 확장 시 화면 밖 이탈).

**[H12·minor] TS_E_NOLAYOUT 재시도·ITfTextLayoutSink 부재**

- unim-tsf 전체에 ITfTextLayoutSink/OnLayoutChange/TF_LC_CHANGE grep 0건. `key_handler.rs:225` `.ok()?` 즉시 탈출.
- H1을 고쳐도 비동기 레이아웃 앱(Chrome/Electron/UWP)에서 TS_E_NOLAYOUT 시 해당 키의 위치 획득이 영구 실패.

**[H13·minor] DPI 미대응**

- `popup_window.rs:29-39` CELL_W/H=52/28 등 물리 픽셀 고정, `:313-328` 폰트 높이 16 고정.
- GetDpiForWindow/WM_DPICHANGED/LogicalToPhysical 등 grep 0건. 150~200% 모니터에서 비례 축소, 혼합 DPI에서 좌표 어긋남.

### 3.2 표시 차단 축 (S2)

**[H2·blocker] UIElement/UILess 프레임워크 전무 — 선언-이행 불일치**

- unim-tsf/src 전체에 ITfUIElementMgr/BeginUIElement/EndUIElement/UpdateUIElement/ITfCandidateListUIElement/ITfUIElementSink **grep 0건**.
- 반면 `installer/wix/unim.wxs:130-136`은 GUID_TFCAT_TIPCAP_UIELEMENTENABLED를, `:162-168`은 TIPCAP_IMMERSIVESUPPORT를 등록 — "지원한다"고 선언만 한 상태.
- `text_service.rs:200` ActivateEx가 `_dwflags`를 무시 → TF_TMAE_UIELEMENTENABLEDONLY(UILess 강제) 감지 분기 자체가 없음.
- 규격: TIP는 HWND ShowWindow 전 반드시 BeginUIElement 호출, pbShow=FALSE면 HWND 숨기고 ITfCandidateListUIElement 데이터만 제공.
UNIM은 이 분기가 없어 UILess 호스트에서 팝업이 차단/무시됨.

### 3.3 내용·선택 축 (S3, S4, S5)

**[H3·blocker] 그리드 transpose — 엔진 column-major vs 렌더 row-major**

- 엔진 선택: `src/popup/popup_layout.rs:91` `idx = col*self.rows + row` (special), `:114` (한자 확장), `:116` row 단독(compact).
커밋 경로: `popup_layout.rs:140,142` selected_global_index → `popup_keys.rs:147,160,199,207,316,324,335`.
- TSF 렌더: `popup_window.rs:392-393` `row = i/cols; col = i%cols` — **row-major**.
- unim-tsf는 special_global_index/hanja_global_index_rc/selected_global_index를 일절 호출 안 함(grep 0건).
- 결과: rows>1 & cols>1 격자(특수/이모지/한자 확장)에서 화면 셀과 엔진 선택·커밋 후보가 전치. **보이는 것과 다른 문자가 커밋**됨. compact 한자만 우연히 일치.

**[H4·blocker] 선택 셀 하이라이트 이중 오류 — selected=sel_row를 글로벌 flat 인덱스와 비교**

- 엔진: `src/input_engine/popup_dispatch.rs:166` `selected: state.sel_row()` — 페이지-로컬 **행 번호**(0..8).
- TSF: `popup_window.rs:187` 무변환 저장 → `:394` `abs_idx = page*per_page + i`, `:397` `abs_idx == state.selected` 비교.
- 의미(행번호 vs flat)와 기준(페이지-로컬 vs 글로벌) 둘 다 불일치: 1페이지에선 상단행 sel_row번째 셀이 엉뚱하게 칠해지고, 2페이지부터는 하이라이트 완전 소실.
- 행/열 **레이블** 하이라이트(popup_window.rs:342,371)만 sel_row/sel_col로 옳음 → "레이블은 움직이는데 셀은 딴 데 칠해진다" 체감의 직접 원인.
- 수정 시 주의: selected에 flat 인덱스를 넣더라도 H3의 major-order까지 함께 맞춰야 정확한 셀이 칠해짐.

*[H5·blocker] 초기 Show 액션에 그리드 파라미터 부재 + 단일 액션 슬롯 → 첫 표시가 항상 3×3/1페이지**

- ShowHanja/ShowSpecial/ShowEmoji payload에 rows/cols/total_pages 없음. `popup_window.rs:117-167` handle_action이 이 필드를 안 건드리고,
`PopupState::default()`(popup_window.rs:100,102-103)는 total_pages=1, rows=3, cols=3.
- 엔진 popup_pending_action은 단일 Option 슬롯(`src/input_engine/popup_dispatch.rs:20-22`)이라 트리거 키 이벤트에서는 Show*만 발행
(candidates.rs:66-70, 98-102 — PopupNavigate 추가 발행 없음). 첫 화살표 키가 와야 실제 격자 파라미터 도착.
- 창 크기(popup_window.rs:256-262 window_size)·렌더(:297-298,390,456) 모두 Default 3×3을 사용 → 첫 프레임은 후보 수와 무관하게 3×3·1/1.
- 참고: Linux daemon도 엔진 payload는 동일하나 daemon↔프런트엔드 사이 별도 어셈블리(DBus/extension)에서 격자를 완성하므로 증상이 없음.

**[H8·major] 이모지 콘텐츠 소실 — recent/categories/home_row 폐기 + 카테고리 전환 시 위치 점프**

- `src/input_engine/types.rs:70-78` ShowEmoji의 recent/categories/home_row를 `popup_window.rs:153-167`이 `..`으로 폐기. PopupWindowState(popup_window.rs:69-90)에 보관 필드 자체가 없어 탭 UI 렌더 불가.
- 카테고리 변경 시 엔진이 ShowEmoji 재발행(popup_dispatch.rs:124-155) → `key_handler.rs:487-495` Show* 분기가 win.show() 재호출 → 위치 재계산 → H1 폴백으로 **마우스 좌표 점프**(Tab마다).

**[H9·major] 한자 뜻풀이·헤더·footer 미렌더**

- `popup_window.rs:124,209` descriptions 저장만 하고 render()(:292-468) 전체에서 참조 0회.
- 렌더 출력: top_row 열 레이블(:333), 행 번호(:379), 글자+★(:429-433), "N/M"(:456)뿐. Period 확장 토글(엔진은 처리)이 무의미.

**[H14·minor] 초기 북마크 ★ 미표시**

- ShowHanja payload(types.rs:45-50)에 bookmarks 없음 → `popup_window.rs:125` `vec![false; n]` 고정.
- Linux는 표시 직후 비동기 fetch(unim-popup-service/src/popup/hanja.rs:271 → gtk_ui.rs:152-157)로 첫 렌더에 ★ 표시. Windows는 Space 토글 전까지 기존 즐겨찾기 표식 없음(승격 정렬은 돼 있어 순서만 바뀌고 이유가 안 보임).

### 3.4 수명주기·키 라우팅 축 (S6, S7, S8)

**[H6·major] engine.reset()이 팝업 상태를 안 지움 — 보이지 않는 팝업 모드 감금**

- `text_service.rs:579-582` OnSetFocus = engine.reset() + popup_window.hide(). 그러나 `src/input_engine/engine.rs:312-317` reset()은
korean_context/commit_buffer/preedit_cache/chord_buffer만 비우고 **hanja_mode(:48)/special_char_mode(:52)/popup_state(:74)/popup_pending_action(:76) 미정리**.
- 이후: 창 기준 popup_active=false(text_service.rs:341-347)라 test_key_down은 일반 게이트 → 키가 OnKeyDown으로 통과 →
press_key 최우선 분기(press_key.rs:63-65)가 process_popup_key로 라우팅 → 키가 보이지 않는 팝업 내비게이션으로 먹히거나,
Other 키로 popup_cancel(popup_dispatch.rs:225-241)이 **이전 문서의 hanja_target을 새 컨텍스트에 스테일 커밋**.
- cancel_hanja/cancel_special_char(candidates.rs:266-275,324-333)는 올바르게 정리하지만 reset()에서 호출되지 않음.

**[H7·major] 팝업 중 비팝업 문자키 통과 — 'NotHandled→닫고 재처리' 경로 부재**

- `key_handler.rs:62-70` 팝업 게이트는 is_popup_key(:154-205) 집합만 소비. 하단 행 Z/X/C/V/B/N/M은 집합에 없어 FALSE 반환.
- TSF 규약: OnTestKeyDown=FALSE면 OnKeyDown 미호출(text_service.rs:327-391,394-441) → 엔진이 그 키를 영영 못 봄 →
Linux의 `PopupKey::Other → popup_cancel → 재처리`(popup_dispatch.rs:70,180-185) 도달 불가.
- 결과: raw 문자가 문서에 들어가고 엔진 팝업 상태 유지(desync). 반대로 한자/특수 팝업에서도 A~L(이모지 전용 CatLetter)을 무조건 먹는 과소비 공존.

**[H10·major] WM_DESTROY에서 PostQuitMessage(0) — 호스트 앱에 WM_QUIT 주입**

- `popup_window.rs:503-505` WM_DESTROY 분기 무조건 PostQuitMessage(0). `:640-647` Drop이 DestroyWindow 호출 → 체인 성립.
- in-proc 창은 호스트 UI 스레드 소속이므로 IME 전환/Deactivate 시 호스트 앱이 통째로 종료될 수 있는 금기 패턴(SampleIME CBaseWindow에는 없음).

**[H15·minor] 마우스 입력 전무**

- popup_wnd_proc(popup_window.rs:472-509)는 WM_PAINT/WM_TIMER/WM_ERASEBKGND/WM_DESTROY만 처리. WM_LBUTTONDOWN 등 0건.
- engine.popup_select/popup_change_page 호출 grep 0건. ITfMouseSink/문서 내 클릭 dismiss 경로도 없음.
- WS_EX_NOACTIVATE(:526-527)라 팝업 클릭이 포커스도 안 뺏어 focus-loss 기반 dismiss조차 불가.

**[H17·minor] ITfThreadFocusSink 부재**

- OnSetThreadFocus/OnKillThreadFocus grep 0건. 유일한 포커스 핸들러는 ITfThreadMgrEventSink::OnSetFocus(text_service.rs:562-588) — 문서 전환만 감지.
- 같은 문서 유지 + 타 프로세스 포그라운드 전환 시 TOPMOST 팝업 잔존 가능. SampleIME는 OnKillThreadFocus에서 Show(FALSE).

### 3.5 런타임 확인 필요 (코드상 결함은 확정, 증상 확정에 실측 필요)


| 항목             | 확인 방법                                                                                                                         |
| -------------- | ----------------------------------------------------------------------------------------------------------------------------- |
| H1의 실제 HRESULT | key_handler.rs:218/226 결과를 `.ok()?` 대신 임시 로깅 → 0x80040201(TF_E_NOLOCK) 확인. 부가: ec가 유효했더라도 ulindex=1은 잘못된 인덱스라 정상 동작 불가        |
| H2의 호스트별 차단 양상 | Windows 검색창·Microsoft Store 앱·풀스크린 게임에서 한자키 입력 → HWND 팝업 표시 여부. Win32 데스크톱(메모장)에서는 HWND_TOPMOST가 정상 표시될 수 있어 **호스트별로 증상이 갈림** |
| H5 첫 프레임       | 트리거 직후(화살표 입력 전) 창 크기·페이지 표기 1/1 육안 확인 — 코드 경로만으로도 결정적이나 재현 영상 확보 권장                                                          |
| 로그 공백          | unim-tsf.log 19,910줄에 popup/hanja/emoji 키워드 0건 — 팝업 경로에 로깅 자체가 없음. P0 작업 시 진단 로깅 동반 추가                                        |


---

## 4. 외부 사례 비교 — 설계 선택지


| 프로젝트                  | 렌더 방식                                                                   | UIElement 3-phase                                                                | 위치 추적                                                                                                                          | 시사점                                                                  |
| --------------------- | ----------------------------------------------------------------------- | -------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------ | -------------------------------------------------------------------- |
| **SampleIME** (MS 공식) | in-proc HWND (CCandidateWindow, CS_IME, TOPMOST+TOOLWINDOW)             | ✅ BeginUIElement→_isShowMode 분기→Update→End. ITfCandidateListUIElementBehavior 구현 | ITfTextLayoutSink 등록, OnLayoutChange(TF_LC_CHANGE)→edit session에서 GetTextExt(ec)→_Move(rc.left, rc.bottom)+MonitorFromPoint 보정 | in-proc HWND라도 **3-phase·LayoutSink는 필수** 준수. ThreadFocusSink로 숨김/복원 |
| **Mozc**              | **별도 프로세스** mozc_renderer.exe (named pipe + protobuf, SenderThread 비동기) | ✅ tip_ui_element_manager.cc BeginUI                                              | GetTextExt→ApplicationInfo로 pack, 실패 시 no_layout 플래그                                                                           | AppContainer IL·UILess·크래시 격리 때문에 프로세스 분리. 렌더러 재기동 가능                |
| **Weasel**            | **별도 프로세스** WeaselServer.exe + WeaselUI(D2D/DWrite)                     | ✅ CCandidateList = ITfCandidateListUIElementBehavior + Integratable              | GetTextExt→UpdateInputPosition→IPC                                                                                             | TIP는 데이터·위치만 IPC, 그리기는 서버. UILess 시 데이터 접근자 제공                       |
| **DIME**              | in-proc HWND (3레이어: shadow/main/scrollbar)                              | ✅ dual-mode (custom UI + ITfCandidateListUIElement)                              | 캐럿 추적 + Z-order 이슈는 SetTimer re-stamp로 해결(#135)                                                                                | in-proc 유지파의 현실적 상한선. Z-order/positioning 버그와 장기 싸움                  |
| **Chewing(Win TSF)**  | **HWND 없음** — ITfCandidateListUIElement만(UI-less), 시스템/앱 UI 위임          | ✅ (그게 전부)                                                                        | 해당 없음                                                                                                                          | 호환성 최대·커스터마이징 최소 노선                                                  |
| **UNIM 현재**           | in-proc HWND 단독                                                         | ❌ 0건 (wxs 선언만)                                                                   | ❌ edit session 규약 위반 → 마우스 폴백 100%                                                                                             | 외부 사례 4종 모두와 어긋나는 유일한 조합                                             |


요약: 자체 HWND를 쓰는 진영(SampleIME/DIME)조차 **UIElement 3-phase + LayoutSink 기반 위치 재시도**는 지키고,
프로세스 분리 진영(Mozc/Weasel)은 in-proc HWND의 구조적 한계(AppContainer IL, UILess 억제, UIPI, 게임 오버레이, 크래시 격리)
때문에 아예 렌더러를 밖으로 뺐다. UNIM은 어느 쪽 규약도 따르지 않는 상태다.

---

## 5. 수정 방향 — 단계별 로드맵

### P0 — 즉시 수정 (현 구조 유지, 데스크톱 Win32 앱에서 "정상 동작"까지)


| 항목                                | 구현 방향                                                                                                                                                                                                                                                                                    | 변경 파일                                                                       | 리스크                                                                     | 난이도 |
| --------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | --------------------------------------------------------------------------- | ----------------------------------------------------------------------- | --- |
| **P0-1 위치: edit session 경유** (H1) | get_composition_screen_pos를 ITfEditSession 구현체로 교체: RequestEditSession(tid, TF_ES_READ|TF_ES_SYNC) → DoEditSession(ec)에서 GetSelection(ec, TF_DEFAULT_SELECTION,…)+GetActiveView+GetTextExt(ec,…) → rect.left/bottom 반환. composition.rs:809-816/:873-892 기존 패턴 재사용. 결과를 임시 로깅(HRESULT 포함) | key_handler.rs (209-230 교체), composition.rs(헬퍼 추출 가능)                       | 키스트로크 컨텍스트 밖 호출 지점이 생기면 TF_ES_SYNC 부적법 — drain 호출 지점이 OnKeyDown 내부임을 유지 | 중   |
| **P0-2 격자 전치 해소** (H3)            | 렌더 루프를 엔진과 동일한 column-major로 전환: 셀 (row,col)의 후보 = candidates[col*rows+row] (special/이모지/한자 확장), compact 한자는 row 단독. 또는 page_candidates를 전치 배열로 변환                                                                                                                                       | popup_window.rs (390-399, 240-253)                                          | compact/확장 모드 분기 누락 시 한자만 다시 깨짐 — 모드별 단위 케이스 필요                         | 중   |
| **P0-3 하이라이트 인덱스** (H4)           | state.selected 의존 제거. 셀 강조를 sel_row/sel_col 직접 비교로 변경(레이블과 동일 기준). 엔진 selected(=sel_row)는 무시                                                                                                                                                                                             | popup_window.rs (187, 394-397)                                              | 낮음 — P0-2와 동시 적용 필수(기준 좌표계 통일)                                          | 하   |
| **P0-4 첫 표시 격자** (H5)             | Show* 수신 시 rows/cols/total_pages를 TSF 쪽에서 직접 산출(한자 compact=후보수→9행 1열, 특수/이모지=9×9, total_pages=ceil(n/per_page)). 장기적으로는 엔진 Show* payload에 rows/cols 추가가 정도(공유 crate 변경 — Linux 회귀 0 검증 동반)                                                                                               | popup_window.rs (117-167), (선택) src/input_engine/types.rs+popup_dispatch.rs | 엔진 페이지 계산(rows9 고정 30/40 규칙)과 어긋나면 N/M 표기 불일치                           | 중   |
| **P0-5 reset 팝업 정리** (H6)         | engine.reset()에서 popup 정리 호출 추가(cancel류 재사용하되 **스테일 커밋 없이** 상태만 클리어하는 내부 메서드 신설: popup_state/hanja_mode/special_char_mode/popup_pending_action=None). 공유 crate 변경이므로 Linux 회귀 테스트 동반                                                                                                     | src/input_engine/engine.rs (312-317), candidates.rs                         | popup_cancel 재사용 시 commit_buffer에 target이 들어가는 부작용 주의 — 무커밋 클리어 경로 필수   | 중   |
| **P0-6 키 통과 desync** (H7)         | test_key_down 팝업 게이트에서 비팝업 키도 TRUE(eat) 반환 → OnKeyDown에서 press_key로 전달 → 엔진 NotHandled 경로(닫고 재처리)가 자연 작동. 동시에 is_popup_key의 모드별 분기(한자/특수에서 A~L 과소비 제거)는 엔진 팝업 종류를 조회해 처리                                                                                                                 | key_handler.rs (41-70, 154-205)                                             | "모든 키 eat" 전환 시 modifier 조합(Ctrl+C 등)은 통과 예외 필요 — 예외 목록 설계              | 중   |
| **P0-7 PostQuitMessage 제거** (H10) | WM_DESTROY 분기에서 PostQuitMessage(0) 삭제(0 반환만)                                                                                                                                                                                                                                             | popup_window.rs (503-505)                                                   | 없음 (자체 메시지 루프가 없으므로 제거가 곧 정답)                                           | 하   |
| **P0-8 모니터 클램프/플립** (H11)         | show()/update()에 MonitorFromPoint(MONITOR_DEFAULTTONEAREST)+GetMonitorInfo(rcWork) → Linux compute_popup_xy 로직 이식(아래 공간 부족 시 위로 플립, X 클램프, 갭)                                                                                                                                            | popup_window.rs (559-589, 614-627)                                          | 멀티모니터 음수 좌표 처리                                                          | 하   |
| **P0-9 카테고리 전환 점프 억제** (H8 일부)    | 팝업 이미 visible이면 Show* 재수신 시 위치 재계산 생략(내용만 갱신)                                                                                                                                                                                                                                            | key_handler.rs (487-495), popup_window.rs                                   | 낮음                                                                      | 하   |


P0 완료 기준: Win32 데스크톱 앱(메모장/wezterm)에서 ①캐럿 옆에 뜨고 ②보이는 문자가 그대로 커밋되고 ③첫 표시부터 올바른 격자이고 ④포커스 전환 후 키 오염이 없다.

### P1 — 구조 보강 (TSF 규약 완비 + 콘텐츠 동등화)


| 항목                                | 구현 방향                                                                                                                                                                                                                                                                                                                                                          | 변경 파일                                                                                 | 리스크                                                                                                          | 난이도 |
| --------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------ | --- |
| **P1-1 UIElement 3-phase** (H2)   | ITfCandidateListUIElement(+Behavior) 구현체 신설(GetCount/GetString/GetSelection/GetCurrentPage 등 — PopupWindowState 데이터 노출). 표시 직전 ITfUIElementMgr::BeginUIElement(&pbShow,&dwId) → pbShow=FALSE면 HWND 미표시·데이터 전용, TRUE면 HWND 병행. 변경 시 UpdateUIElement(TF_CLUIE_SELECTION/CURRENTPAGE), 닫을 때 EndUIElement. ActivateEx dwflags에서 TF_TMAE_UIELEMENTENABLEDONLY 감지 저장 | 신규 candidate_ui_element.rs, text_service.rs(199-268), key_handler.rs, popup_window.rs | COM 구현 분량 큼. UILess 호스트에서 한자 9×9 격자가 시스템 선형 UI로 강제 단순화됨(수용 필요). SampleIME CandidateListUIPresenter가 직접 참조 모델 | 상   |
| **P1-2 LayoutSink 재시도** (H12)     | ITfTextLayoutSink 구현+AdviseSink. GetTextExt가 TS_E_NOLAYOUT이면 pending 플래그 → OnLayoutChange(TF_LC_CHANGE)에서 재시도 후 창 이동                                                                                                                                                                                                                                           | 신규/`composition.rs`, key_handler.rs                                                   | sink 수명·UnadviseSink 누수 주의                                                                                   | 중   |
| **P1-3 한자 뜻풀이/헤더/footer 렌더** (H9) | render()에 meaning 컬럼(확장 모드), header_text(compact=target+라벨, 확장=target+선택 한자+뜻), 한자 상시 footer 추가 — Linux view_model.rs 포맷 규칙 이식. 창 폭 동적 계산                                                                                                                                                                                                                      | popup_window.rs (render 전반, window_size)                                              | GDI 텍스트 측정(GetTextExtentPoint32) 기반 폭 계산 필요                                                                  | 중   |
| **P1-4 이모지 탭 UI** (H8)            | PopupWindowState에 categories/recent/active_tab_index 보관, ShowEmoji `..` 폐기 중단. 탭 바(9탭, label_ko+key) 렌더 + active 강조                                                                                                                                                                                                                                            | popup_window.rs (69-90, 153-167, render)                                              | 창 높이 증가 — 클램프(P0-8)와 상호작용                                                                                    | 중   |
| **P1-5 초기 북마크 ★** (H14)           | (a) 엔진 ShowHanja payload에 bookmarks 추가(공유 crate, Linux 회귀 검증) 또는 (b) TSF가 표시 직후 엔진 북마크 상태 조회 API 호출. (a) 권장 — Linux도 비동기 fetch 제거 가능                                                                                                                                                                                                                           | src/input_engine/types.rs+candidates.rs, popup_window.rs:125                          | 공유 crate 변경 — 5지점 동기화 아님(엔진 내부)이나 Linux popup-service 소비부 확인 필요                                              | 하   |
| **P1-6 마우스 입력** (H15)             | WM_LBUTTONDOWN→셀 히트테스트→engine.popup_select(엔진 column-major 인덱스 주의)→take_popup_action drain. 페이지 버튼→popup_change_page. WM_MOUSEACTIVATE는 MA_NOACTIVATE 반환                                                                                                                                                                                                       | popup_window.rs (wnd_proc), key_handler.rs(drain 재사용 경로)                              | wnd_proc에서 엔진 뮤텍스 접근 — 데드락/재진입 설계 필요                                                                         | 중   |
| **P1-7 ThreadFocusSink** (H17)    | ITfThreadFocusSink 구현: OnKillThreadFocus→popup hide(+엔진 cancel), OnSetThreadFocus→필요 시 복원. SampleIME 패턴                                                                                                                                                                                                                                                        | text_service.rs                                                                       | sink cookie 관리                                                                                               | 하   |
| **P1-8 DPI 대응** (H13)             | GetDpiForWindow 기반 스케일 팩터로 CELL/LABEL/폰트 환산, WM_DPICHANGED 처리. GetTextExt 좌표는 호스트 DPI 컨텍스트 확인 후 변환                                                                                                                                                                                                                                                             | popup_window.rs (29-39, 313-328, show/update)                                         | 호스트별 awareness 혼재 — 실측 매트릭스 필요                                                                               | 중   |


### P2 — 장기 구조 결정: in-proc HWND 유지 vs 렌더러 프로세스 분리


| 선택지                                 | 내용                                                                                          | 장점                                                                                     | 단점                                                                                     |
| ----------------------------------- | ------------------------------------------------------------------------------------------- | -------------------------------------------------------------------------------------- | -------------------------------------------------------------------------------------- |
| **A. in-proc 유지 + P1 완비** (DIME 노선) | 현 popup_window.rs 강화로 종결                                                                    | 추가 프로세스 없음, MSI 단순, 현 코드 자산 보존                                                         | AppContainer IL·UIPI·게임 오버레이 한계 잔존(UILess 데이터 경로로 부분 보완), Z-order 분쟁(DIME #135류) 장기 부담 |
| **B. 렌더러 프로세스 분리** (Mozc/Weasel 노선) | unim-renderer.exe(또는 향후 상주 프로세스 겸용) + named pipe IPC. TIP는 PopupAction+위치만 전송, 렌더러가 HWND 소유 | UWP/게임/검색창에서 정상 IL로 표시 가능, 렌더러 크래시 격리, H10류 호스트 오염 원천 차단, Linux popup-service와 아키텍처 대칭 | 프로세스 수명 관리(기동/재기동/세션), MSI 변경, IPC 프로토콜 설계 — 공수 큼                                      |


권고: **P0+P1을 A 노선으로 먼저 완료**해 데스크톱 정상화를 확보하고, UWP/게임 실측(6장 매트릭스)에서 차단 빈도가 실제 사용에 유의미하면 B로 이행한다.
B 이행 시에도 P1-1(UIElement 데이터 경로)과 P0의 엔진 계약 수정은 전부 재사용된다 — 선행 투자 손실 없음.

권장 착수 순서: `P0-7(1줄, 앱 종료 위험 즉제거) → P0-1(위치) → P0-2+P0-3(전치+하이라이트 동시) → P0-4 → P0-5+P0-6(수명주기·키) → P0-8/9 → P1`.
공유 crate(src/input_engine) 변경이 포함되는 P0-4(b)/P0-5/P1-5는 Linux 회귀 0 검증(make build zero-warning + cargo test all-pass)을 동반한다.

---

## 6. 검증 계획 — 수동 테스트 매트릭스

전제: VM 실타이핑(SMOKE_TEST.md 절차 준용). 각 셀은 "한자(compact/9×9 확장)/특수문자/이모지" 3종 × 아래 시나리오.

### 6.1 앱 × 팝업 종류


| 앱 (호스트 유형)                              | 한자 compact | 한자 9×9 확장(Period) | 특수문자(초성) | 이모지(idle 한자키) |
| --------------------------------------- | ---------- | ----------------- | -------- | ------------- |
| 메모장 (Win32 클래식)                         | □          | □                 | □        | □             |
| WordPad/워드 (리치 에디트)                     | □          | □                 | □        | □             |
| Chrome 주소창+textarea (CUAS/Electron 계열)  | □          | □                 | □        | □             |
| wezterm (터미널, client-side preedit 경로)   | □          | □                 | □        | □             |
| Telegram (레거시 폴백 경로)                    | □          | □                 | □        | □             |
| Windows 검색창 (UILess)                    | □          | □                 | □        | □             |
| Microsoft Store 앱/설정 (UWP·AppContainer) | □          | □                 | □        | □             |
| 풀스크린 게임 1종 (D3D 독점)                     | □          | □                 | □        | □             |


### 6.2 시나리오 체크리스트 (각 앱·팝업 조합마다)

1. **위치**: 캐럿 바로 아래 표시(마우스를 화면 반대편에 두고 검증 — H1 회귀 감지). 화면 하단/우측 끝에서 플립·클램프. 멀티모니터 경계+DPI 혼합(100%+150%).
2. **첫 표시**: 트리거 직후(키 추가 입력 전) 격자·페이지 표기가 올바른가(3×3 회귀 감지).
3. **선택 일치**: 화살표로 임의 셀 이동 → 셀 하이라이트=레이블 하이라이트 일치 → Enter 커밋 문자=화면 문자. Num1-9, Q~O 열 점프 동일. **2페이지 이상**에서 반복(H4 소실 회귀).
4. **내용**: 한자 뜻풀이/헤더/footer(상시), 이모지 9탭+active 강조+Recent, 초기 즐겨찾기 ★ 첫 렌더 표시.
5. **북마크**: Space ON(선두 승격·커서 추종) / OFF(강등·140ms flash) / 페이지 넘어 점프.
6. **키 라우팅**: 팝업 중 Z/B/M 입력 → 팝업 닫히고 해당 키 정상 재처리(raw 삽입+팝업 잔존 desync 회귀). 한자/특수 팝업에서 A~L이 글자 입력을 부당하게 먹지 않는가. Shift+Tab 이모지 역방향 카테고리.
7. **수명주기**: 팝업 연 채 ①같은 앱 내 다른 문서로 전환 ②다른 앱 클릭(타 프로세스) ③ESC — 모두 팝업 소멸+엔진 상태 클린(직후 한글 타이핑 정상, 스테일 커밋 무). 팝업 연 채 IME 전환(한영키/Win+Space) → **호스트 앱 생존**(H10).
8. **마우스**: 셀 클릭 커밋, 페이지 버튼, 문서 내 다른 위치 클릭 시 dismiss, 팝업 클릭이 포커스를 뺏지 않음.
9. **UILess**(검색창/UWP/게임): pbShow=FALSE 경로에서 시스템 후보 UI로 후보·선택·페이지가 노출되는가(P1-1 후).
10. **로그**: 신설 팝업 진단 로깅에서 GetTextExt HRESULT, BeginUIElement pbShow, NOLAYOUT 재시도 횟수 수집.

### 6.3 자동화 가능 부분

- 전치/하이라이트/페이지 계산은 popup_window.rs의 인덱스 산출을 순수 함수로 추출해 cargo test(엔진 popup_layout과 교차 검증: 모든 (row,col,page)에 대해 TSF 셀 후보 == engine.selected_global_index 후보).
- engine.reset() 팝업 클리어는 엔진 단위 테스트로 고정(reset 후 hanja_mode/popup_state/pending 전부 false/None).

---

## 부록 A — 기각된 가설 (재조사 방지)

- **[H16·refuted] drain 이전 조기 return으로 pending PopupAction 1키 지연**: key_handler.rs:313-315의 조기 return은 구조적으로 drain(:323)보다 앞서지만, popup_dispatch.rs:96-186의 모든 PopupAction 발행 분기가 최소 1개 InputResult 플래그(committed/preedit_updated)를 보장하므로 현 엔진 로직에서는 발동 불가. 향후 엔진이 "플래그 전부 false + 액션 세팅" 경로를 추가하면 재점화되는 잠재 위험으로만 기록.

## 부록 B — 가설↔증상↔수정 매핑 요약


| 가설                     | 심각도     | 증상     | 수정 단계       |
| ---------------------- | ------- | ------ | ----------- |
| H1 ec 오용 위치 실패         | blocker | S1     | P0-1        |
| H2 UIElement 전무        | blocker | S2     | P1-1        |
| H3 격자 전치               | blocker | S3     | P0-2        |
| H4 하이라이트 이중 오류         | blocker | S3     | P0-3        |
| H5 첫 표시 3×3            | blocker | S4     | P0-4        |
| H6 reset 미정리           | major   | S6     | P0-5        |
| H7 키 통과 desync         | major   | S6     | P0-6        |
| H8 이모지 탭 소실/점프         | major   | S5, S9 | P1-4 / P0-9 |
| H9 뜻풀이 미렌더             | major   | S5     | P1-3        |
| H10 PostQuitMessage    | major   | S8     | P0-7        |
| H11 클램프/플립 부재          | major   | S1     | P0-8        |
| H12 NOLAYOUT 재시도 부재    | minor   | S1     | P1-2        |
| H13 DPI 미대응            | minor   | S1     | P1-8        |
| H14 초기 ★ 미표시           | minor   | S5     | P1-5        |
| H15 마우스 전무             | minor   | S7     | P1-6        |
| H17 ThreadFocusSink 부재 | minor   | S7     | P1-7        |

---

# IMM32·ATF·조합 패스쓰루 수정 (2026-06-15 통합 진단)

대상: `unim-tsf` (TSF TIP). 코어 `src/`·Linux 크레이트 무수정. `preedit_window.rs` 불가침.
빌드: `cargo build -p unim-tsf --target x86_64-pc-windows-msvc` zero-warning.

## 0. 코드 재확인으로 확정한 근본원인

### 버그① — IMM32/CUAS 앱 완전 무동작 (회귀 확정: 10743c6)
`git show 10743c6` 로 두 변경 확인됨:
1. `key_handler.rs:99-105` — 영문모드+ATF순방향 ON 이면 문자 키를 `test_key_down`에서 소비(return true). 이전엔 끝에서 false → CUAS 가 raw 영문키를 앱에 전달했음.
2. `key_handler.rs:408` — 비조합 commit 을 `insert_text`(구) → `replace_surrounding(...,0,...)`(reconversion, 신) 로 교체.

두 경로 모두 `TF_ES_READWRITE|TF_ES_SYNC` 세션에 의존. CUAS 가상문서가 sync 세션을 거부하면(`RequestEditSession`→음수 HRESULT `TS_E_SYNCHRONOUS(0x80040249)` 또는 `DoEditSession` 미실행) commit 문자가 문서에 안 들어감. 그런데 키는 이미 OnTestKeyDown/OnKeyDown 이 TRUE 로 소비 → raw 입력도 차단 → 완전 무동작. 한글모드도 `start_composition` sync 세션 거부로 조합 미생성. (composition.rs:288 만 hr 로깅, 402·435·316·331·347·390 은 `let _ =` 로 결과 무시.)

핵심: `composition_unsupported` 게이트는 이미 있으나 **OnCompositionTerminated(=composition 생성 후)** 로만 학습된다. 영문 commit 은 composition 을 안 만들어 영영 학습 안 됨 → 무한 유실. **편집세션 거부 자체를 학습 신호로 만들어야 한다.**

### 버그② — Chrome(Blink) 순방향 첫 글자('서') 누락 (회귀: 10743c6)
`composition.rs:726` `range.Collapse(ec, TF_ANCHOR_END)` 가 StartComposition 이 AddRef 로 보유 중인 동일 range 를 0폭으로 만든 뒤 `EndComposition`(728) 호출 → Blink 가 0폭 composition 을 "빈 확정"으로 해석, before-composition 재계산 중 첫 글자 소실. `move_caret_to_end`(727)는 Clone 한 별도 range 를 쓰므로 원본 불간섭 → 726 한 줄 제거가 정답. delete=0(비조합 영문 commit) 경로도 이 블록을 타므로 함께 고쳐짐.

### 버그④ + CRITICAL — Word/Chrome SendInput 폴백 오발동 (회귀: 미커밋, synth_input ??)
`composition.rs:678` `let mut shifted = 0;` 는 `if self.delete_chars>0` **밖**에서 선언되나, fallback 체크(699)는 `delete_chars>0 && -shifted < delete_chars`. ShiftStart 성공(shifted=-4, delete=4)이면 `4<4=false` → 폴백 안 함(=CRITICAL "항상 TRUE" 주장은 부분 오류). **진짜 문제: Word/Chrome 같은 정상 TSF 에서 ShiftStart 가 확정텍스트를 부분(shifted=-1)만 노출 → 폴백 발동 → SendInput 비동기 큐 ↔ 동기 composition 순서 미보장으로 Word 문서모델 손상.** 폴백은 진짜 CUAS(IMM32)에서만 써야 한다.

→ **버그②와 버그④는 별개 원인(Collapse vs 폴백게이트)이나 동일 함수(`ReplaceSurroundingEditSession::DoEditSession`)를 공유**하므로 한 함수 안에서 순차 수정한다. 공통점은 "정상 TSF 에서 폴백/파괴적 편집을 하지 말 것".

### 버그③ — 조합 중 수정자 단축키(Ctrl+J, Shift+Del) 통과 시 조합 잔류 (회귀: unknown)
`key_handler.rs:73` Ctrl/Alt/Super 조기 false 가 `is_composing()` 검사 없음. `is_commit_passthrough_key`(134-149) 목록엔 Enter/Tab/Esc/방향/Home/End/PgUp/PgDn 만 있어 Ctrl+J·Shift+Del 미포함 → OnTestKeyDown 커밋 블록 미진입 → 조합 잔류. test_key_down 은 comp_mgr 접근 불가 → 커밋은 **OnTestKeyDown(text_service.rs)** 에서 해야 함.
`is_character_key()`=`to_char().is_some()` 가 A-Z·**Num0-9·Space·기호키 전부 포함**(conversion.rs:158-208 확인). 따라서:
- Ctrl/Alt/Super + any(비수정자) → 커밋+패스쓰루.
- Shift + **!is_character_key()** (Del/Insert/F-keys/방향 등) → 커밋+패스쓰루.
- Shift + is_character_key()(Shift+A 대문자, Shift+1=! 등) → **제외**(엔진이 Korean 모드에서 이미 소비·처리하는 조합/기호 입력).
- is_modifier() 단독(Ctrl/Shift 누름) → 제외.

## 1. 단일 구현자 변경 순서 (의존성순)

순서 근거: 공유 함수 충돌 회피. (a) composition.rs 의 게이트/시그널 인프라를 먼저 만들고 → (b) ReplaceSurrounding 내부 수정 → (c) key_handler 분기 → (d) text_service 패스쓰루. (a)→(b) 는 같은 파일, (c) 는 (a) 의 시그니처 의존, (d) 는 독립.

### STEP 1 (composition.rs) — 편집세션 거부 감지 + CUAS 폴백 신호 인프라
1-1. 모듈 정적 신호 추가(composition.rs 상단): `pub static LAST_EDIT_REFUSED: AtomicBool` + `pub fn take_edit_refused() -> bool { swap(false) }`. text_service 가 read+clear 해 학습.
1-2. `RequestEditSession` 결과 일관 처리 헬퍼 `request_sync(context,tid,session)->bool`: 반환 `i32`(세션 결과 HRESULT)가 `>=0` 이면 성공(true); 음수(`TS_E_SYNCHRONOUS=0x80040249` 음수임)거나 Err 이면 `LAST_EDIT_REFUSED=true` + dbg_log 후 false. 모든 호출부의 `let _ = context.RequestEditSession(...)` 를 이 헬퍼로 교체.
1-3. `insert_text`·`replace_surrounding`·`start_composition` 는 **bool 성공여부 반환**으로 시그니처 변경(현재 () 반환). key_handler 가 실패 시 raw/synth 폴백·학습에 사용. `commit_and_restart`/`update_composition`/`end_composition*` 도 내부 `request_sync` 사용(반환은 () 유지 가능).

### STEP 2 (composition.rs `ReplaceSurroundingEditSession::DoEditSession`) — 버그②+④
2-1. **버그② 수정**: 726 의 `let _ = range.Collapse(ec, TF_ANCHOR_END);` **삭제**. 결과 순서: `SetText` → `move_caret_to_end`(Clone 기반, 원본 불간섭) → `EndComposition` → `set_result?`. (raw 폴백 736 의 Collapse 는 composition 미보유라 무해, 유지.)
2-2. **버그④ 폴백 게이트**: `ReplaceSurroundingEditSession` 에 `pub is_cuas: bool` 필드 신설, `replace_surrounding` 시그니처에 `is_cuas: bool` 인자 추가. 699 조건을 `self.delete_chars>0 && -shifted < self.delete_chars && self.is_cuas` 로 변경. 정상 TSF(is_cuas=false)는 폴백 미발동 → 3단계 reconversion 으로 range 편집 성립.
2-3. (보수적) is_cuas=false 인데 shifted 부족이면 폴백 대신 dbg_log 남기고 range 편집 진행(런타임 관찰).

### STEP 3 (key_handler.rs) — 버그① 영문 경로 + CUAS 게이트 전파
3-1. `replace_surrounding` 호출 4곳(268 수동ATF, 282 undo, 408 비조합commit, 449 ATF)에 `is_cuas` 인자 추가 = `composition_unsupported` 전달.
3-2. **버그① 비조합 commit 분기(401-409)**: `composition_unsupported` 면 reconversion 대신 `insert_text`, 그래도 실패하면 `synth_input::send_replacement(0, &commit, "")`. 아니면 기존 `replace_surrounding(...,0,...,false,...)`.
3-3. **버그① test_key_down 영문 소비 게이트(99-105)**: `test_key_down` 시그니처에 `composition_unsupported: bool` 추가(text_service:430 에서 `self.composition_unsupported.load` 전달). 조건 끝에 `&& !composition_unsupported` 추가 → CUAS 면 영문 소비 금지(raw 통과). 학습 전 첫 1회 유실은 STEP4 거부신호 학습으로 다음 키부터 복구(보수적 절충).

### STEP 4 (text_service.rs) — 거부신호 학습 + 버그③ 패스쓰루
4-1. **버그① 학습 일원화**: `OnKeyDown` 의 `handle_key_down` 호출 직후 `if composition::take_edit_refused()` → `GetFocus` HWND 를 `cuas_windows` 에 insert + `composition_unsupported=true`. OnCompositionTerminated(200ms) 와 동일 cuas_windows/composition_unsupported 로 수렴(판정원 일원화).
4-2. **버그③ 패스쓰루 블록**: OnTestKeyDown 의 `is_commit_passthrough_key` 블록(395-427) **바로 앞**(modifier 우선)에 삽입: `kc=from_win32_vk(vk)`, `m=get_modifier_state()`(key_handler 의 함수를 pub 화 또는 동등 인라인), `is_combo = !kc.is_modifier() && (m.control||m.alt||m.super_key || (m.shift && !kc.is_character_key()))`. `!popup_active && engine.is_composing() && is_combo` 이면 기존 커밋 로직(comp_mgr end/insert + win.hide + remove_preedit) 수행 후 `return Ok(FALSE)`. 커밋 로직은 두 블록 공통이므로 헬퍼 추출 권장(중복 제거).

## 2. 충돌 회피 매트릭스
- 버그②↔④: 동일 함수, 726 삭제(②)와 699 게이트(④)는 다른 라인 — 순차 무충돌.
- 버그①↔④: 둘 다 synth_input 재사용. STEP3-2 의 `send_replacement(0,commit,"")` 는 preedit="" → 센티널 미발생 → ARMED 미설정 → ATF 폴백(preedit 有)과 카운터 경합 없음. PENDING 은 매 send 가 store(덮어쓰기)라 누적 안전.
- 버그①↔③: STEP4-1(거부학습, OnKeyDown) ↔ STEP4-2(패스쓰루, OnTestKeyDown) 다른 콜백 — 무충돌. 패스쓰루로 FALSE 시 OnKeyDown 미호출이나 조합 커밋은 패스쓰루 블록이 직접 수행하므로 문제 없음.
- 미커밋 rev_window/팝업분리: STEP4 는 OnTestKeyDown/OnKeyDown 내부 추가라 ActivateEx/rev_window/last_context(437) 라인과 분리 — 머지 충돌 최소.

## 3. 보수적 처리 / 로그 (confidence 낮은 항목)
- CRITICAL "shifted 성공해도 폴백" 주장은 코드상 미성립 → 큰 구조 변경 대신 **is_cuas 게이트만** 추가(STEP2-2). shifted 거동은 로그로 관찰(STEP2-3).
- 버그③ regression unknown → 큰 리팩터 금지, 기존 커밋 블록 재사용 + 조건만 확장.
- STEP3-3 의 "첫 키 1회 유실" 은 의도적 절충 — 큰 동기/비동기 재설계 회피. 잦으면 ActivateEx/OnSetFocus 선제 학습(known_cuas)로 0회화 검토.

## 4. 회귀 가드 체크리스트
- 메모장 순방향 ATF 'ntkd'→'서기' 첫 글자 정상 + 역방향 'ㅈ디'→'woo' 정상.
- 메모장 일반 한글/영문·비조합 commit 무회귀(insert/reconversion 정상).
- Chrome 순방향 첫 글자 정상(726 삭제), is_cuas=false 폴백 미발동.
- Word 순/역방향 텍스트 손상 없음(폴백 미발동, range 편집).
- 카카오톡/한글 윈도우입력기(CUAS): 영문/한글/한영키 동작 + 거부학습 후 raw/insert 폴백.
- wezterm 오버레이(composition_unsupported) 한글 조합·확정 무회귀.
- 팝업(한자/이모지) 표시·내비·마우스 역채널 무회귀.
- 조합 중 Ctrl+J/Shift+Del 커밋+패스쓰루, Shift+A·Shift+1(!) 은 기존대로 조합/입력.
- AutoTypeFix Ctrl+Z undo 정상(replace_surrounding is_cuas 인자 추가 후).
- `cargo build -p unim-tsf --target x86_64-pc-windows-msvc` zero-warning.

