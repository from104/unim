# Hanja Bookmark UI — 프런트엔드 이식 계획서

**브랜치**: develop (HEAD: 423191f Merge PR #3)
**기준선**: `unim-gnome-extension/hanja_popup.js` (PR #3 머지 완료)
**엔진/DBus 서피스**: 준비 완료 (`PopupKeyResult::ToggleBookmark`, `GetHanjaBookmarkStates`,
`ToggleHanjaBookmark`, `HanjaBookmarkChanged` signal, C-API `POPUP_RESULT_TOGGLE_BOOKMARK=5`)

---

## 1. 기준선 요약 — GNOME Extension (이미 구현됨)

### 1-A. 별 렌더 (`unim-gnome-extension/hanja_popup.js`)
- **Compact list**: 라인 **402-408**, `St.Label` 생성 + `item-bookmark` / `bookmarked` 클래스, 텍스트 `☆/★`. 행에 `add_child(star)` (라인 413).
- **Grid cell**: 라인 **462-468**, `bookmarked` 클래스로 강조 배경(별 아이콘 없음).
- 상태 반영 함수: `_applyBookmarkStyle()` 라인 **588-608**, `_refreshBookmarkAt()` 라인 **575-582**.
- 초기 상태 주입: `show(..., bookmarks, onToggleBookmark, onToggleExpand)` 라인 **161-195**.

### 1-B. Space 토글 경로 (UI → 엔진)
- **팝업 내부에 Space 처리 없음** (팝업은 "순수 UI"). 엔진이 `PopupKeyResult::ToggleBookmark(idx)`를 반환 → DBus `HanjaBookmarkChanged` signal 발행 → 팝업은 signal로만 갱신.
- 우클릭 경로(보조): `_toggleBookmarkAt()` 라인 **624-628** → callback → `extension.js:202` `this._dbusIME.toggleHanjaBookmark(globalIdx)`.
- **결론**: 프런트엔드는 Space를 따로 처리할 필요 없음. 엔진이 이미 처리. Space를 **팝업으로 라우팅**하기만 하면 됨 (이미 모든 프런트엔드가 그렇게 하고 있음).

### 1-C. DBus signal 구독
- `extension.js:247-250` — `onHanjaBookmarkChanged: (index, bookmarked) => this._hanjaPopup.setBookmark(index, bookmarked)`.
- `dbus_ime.js:303-305` — signal 디멀티플렉스.
- 초기 상태: `extension.js:176` — `getHanjaBookmarkStates()` 호출 후 `bookmarks` 배열로 `show()`에 전달.
- DBus RPC 래퍼: `dbus_ime.js:539 getHanjaCandidates`, `:596 getHanjaBookmarkStates`, `:622 toggleHanjaBookmark`.

---

## 2. 프런트엔드별 매트릭스

### 2-1. GTK Standalone — `unim-gui-gtk`
- **팝업 구현**: yes (`src/hanja_popup.rs`, 10 KB)
- **렌더 함수**: `HanjaPopup::update_list()` (구조체: 라인 14-28, `new()` 30-; `show()` 후 call). 행 구성은 `ListBox` 기반, `(String, String)` candidates. **별을 넣을 행 생성 지점**: update_list 내부 후보 루프. **bookmarks 필드 부재 — 추가 필요** (`candidates` 옆에 `bookmarks: Vec<bool>`).
- **Space 경로**: 엔진이 이미 처리하므로 해당 없음. 단, 팝업 자체에는 key handler가 없음(ListBox 클릭만). Space 처리는 IME가 엔진으로 전달. 신규 작업 없음.
- **DBus 경로**: 공유 `unim-gui-common/src/dbus_client.rs::watch_dbus_signals` (라인 18) — `ShowHanjaPopup` 파싱(라인 303-320)은 이미 있음. **추가 작업**: `handle_popup_signal()` 291행에 `HanjaBookmarkChanged` 분기 신설 → `GuiAction::HanjaBookmarkChanged { index, bookmarked }` 발행. `types.rs::GuiAction` (라인 36-)에 새 variant 추가. `gtk_ui.rs:60-71` `GuiAction::ShowHanjaPopup` 매칭에 이어 `HanjaBookmarkChanged` 매칭 추가 → `HanjaPopup::set_bookmark()` 호출.
- **초기 상태**: `ShowHanjaPopup`에서 target/candidates를 받을 때, 별도 DBus 호출 `GetHanjaBookmarkStates`로 `Vec<bool>` 취득. `dbus_client.rs` 또는 `hanja_popup.rs` show 내부에서 zbus proxy(이미 `SelectHanja` 호출 패턴 사용 중, hanja_popup.rs 내부) 동일 패턴.
- **난이도**: **M** — bookmarks 상태 저장소 추가 + 새 GuiAction + DBus 초기 fetch + set_bookmark API.

### 2-2. GTK IM module 내장 팝업 — `unim-frontends/gtk-common`
- **팝업 구현**: yes (`src/unim_hanja_popup.c`)
- **렌더 함수**: `update_listbox()` 라인 **120-200** (row_box 구성: num_label 라인 51, hanja_label ~60, meaning_label 62). 후보 루프 라인 43-. **별 추가 지점**: 64행 WIDGET_ADD_CSS_CLASS 이후, `gtk_box_pack_start` 다음에 `star_label` 생성·추가.
- **Space/key 핸들러**: `unim_hanja_popup_handle_key()` 라인 **689-720**. `unim_popup_handle_key()` 반환값 switch (라인 694). **추가 작업**: `case UNIM_POPUP_RESULT_TOGGLE_BOOKMARK` 분기 필요 (엔진이 Space를 소화해 반환 가능; C-API 라인 588에 상수 이미 존재).
- **DBus client**: `src/unim_dbus_client.c` — `get_hanja_candidates` 기존(~484), `AutoTypeFix` signal subscribe 패턴(~179) 사용. **추가 작업**: (a) `unim_dbus_get_hanja_bookmark_states(ctx, bool_array_out)`, (b) `unim_dbus_toggle_hanja_bookmark(ctx, index)`, (c) `HanjaBookmarkChanged` signal subscribe + callback 등록 (기존 `commit_text_signal_id` 패턴 미러링). 헤더 `unim_dbus_client.h`에 프로토타입 + callback typedef 추가.
- **UnimHanjaPopup 구조체**: `bookmarks` 필드 추가 (`bool*` 배열). `unim_hanja_popup_set_bookmark(popup, index, flag)` API + `update_listbox` refresh 트리거.
- **난이도**: **M**

### 2-3. Qt IM module 내장 팝업 — `unim-frontends/qt-common`
- **팝업 구현**: yes (`include/unim_hanja_popup.hpp`, `src/unim_hanja_popup.cpp`)
- **렌더 함수**: `UnimHanjaPopup::updateList()` 라인 **236-** (`m_labels[]` 배열로 QLabel 9개 선렌더, 페이지 갱신). `showPopup()` 라인 ~159. **별 추가**: 별도의 `QLabel m_bookmarkStars[MAX_VISIBLE_CANDIDATES]` 추가하거나 텍스트에 ★/☆ 프리픽스. CSS 섹션 라인 88-124.
- **Space/key 핸들러**: 라인 ~210-230 (`unim_popup_handle_key` 호출 후 `m_selectedIndex` 갱신). 동일하게 `TOGGLE_BOOKMARK` kind 분기 추가 필요.
- **DBus client**: `src/unim_dbus_client.cpp` — `getHanjaCandidates` 라인 **269**, `selectHanja` 라인 **317**, signal subscribe 패턴 라인 **501-537** (AutoTypeFix/CommitText `m_bus.connect`). **추가 작업**: (a) `bool getHanjaBookmarkStates(QList<bool> &out)`, (b) `bool toggleHanjaBookmark(quint32 index)`, (c) `HanjaBookmarkChanged` signal subscribe + Qt signal emit. 헤더 `include/unim_dbus_client.hpp` (라인 120 근처 메서드 선언 + signal `hanjaBookmarkChanged(quint32,bool)`).
- **난이도**: **M**

### 2-4. XIM — `unim-frontends/xim`
- **팝업 구현**: yes (`src/hanja_window.rs`, Xft로 직접 draw)
- **렌더 함수**: `HanjaWindow::redraw()` 라인 ~400-; 후보 그리기 `draw_string_with_fallback` 445-. **별 렌더 어려움**: 유니코드 ★/☆ 문자 지원 확인 필요(Xft fallback 경로 이미 있음, 라인 486-530). `set_candidates()` 라인 **315-370**에 `bookmarks` 파라미터 추가.
- **Space/key 핸들러**: key 처리는 엔진 위임(handler.rs가 `unim_popup_handle_key`를 호출하지 않고 엔진으로 바로 포워딩). XIM 프런트엔드는 popup key를 처리하지 않음 → 추가 작업 없음. 엔진이 ToggleBookmark 반환 시 `HanjaBookmarkChanged` signal이 나오므로 UI만 갱신.
- **DBus 경로**: `src/dbus_client.rs` `DbusRequest` enum(86-), `PopupEvent` enum(16-), `subscribe_popup_signals()` 라인 **576-656** — `receive_show_hanja_popup`, `receive_popup_navigate`, `receive_auto_typefix_apply`, `receive_commit_text` 패턴. **추가 작업**: (a) `DbusRequest::GetHanjaBookmarkStates`, `DbusRequest::ToggleHanjaBookmark` variant, (b) `PopupEvent::HanjaBookmarkChanged { index, bookmarked }`, (c) 새 stream `receive_hanja_bookmark_changed().await` 추가(subscribe_popup_signals 내부 select! 블록에 분기), (d) `handler.rs::handle_popup_event` 라인 500-에서 새 이벤트 → `hanja_window.set_bookmark(idx, flag)` 호출.
- **난이도**: **M–L** — Xft 별 그리기 검증 필요(폰트 fallback 가능성). signal stream 신설로 보일러플레이트 多.

### 2-5. Wayland — `unim-frontends/wayland`
- **팝업 구현**: yes, 하지만 **기능 제한적** (`popup_renderer.rs`, tiny-skia 픽스맵에 텍스트 직접 드로잉)
- **렌더 함수**: `render_hanja_page()` 라인 **62-116** — 후보 루프 90-108에 `draw_text`로 hanja + meaning 그림. **별 추가**: `draw_text(pixmap, "★", ...)` 로 추가 가능. bookmark 상태는 `PopupState` 기반으로 이미 조회 가능 (엔진이 관리).
- **Space/key 핸들러**: 엔진 위임 (`state.rs:167` 주석 "팝업 키 처리는 엔진이 담당"). 추가 작업 없음.
- **DBus 경로**: `src/dbus_client.rs` `subscribe_popup_signals()` 라인 **513**. XIM과 거의 동일한 패턴 — `DbusRequest`/`PopupEvent` enum 확장 + 새 signal stream 필요.
- **BLOCKER 주의**: Wayland 팝업은 `zwp_input_popup_surface_v2` 사용 — 별 렌더 자체는 가능하지만, 현재 `popup_renderer`는 `PopupState`를 읽어 전체 재렌더만 한다 (부분 갱신 없음). `HanjaBookmarkChanged` signal 수신 시 전체 재렌더 트리거만 추가하면 됨 (`popup_surface.show_hanja` 또는 `redraw`). **메모리**: `PopupState`의 `bookmarks` 필드가 엔진 쪽에 있고 Wayland는 엔진에서 상태를 DBus로 별도 fetch해야 함.
- **난이도**: **L** — 텍스트 draw + PopupState sync + 새 signal 추가. 프로젝트 전체에서 Wayland가 가장 동기화 난이도 높음.

---

## 3. 이식 권장 순서 (저위험 → 고위험)

1. **GTK Standalone** (M) — 공유 `unim-gui-common` 라우팅이 이미 있어 수정이 국소화됨.
2. **GTK IM module** (M) — C-API 상수 이미 준비(`POPUP_RESULT_TOGGLE_BOOKMARK=5`), signal subscribe 패턴 기존 2개(AutoTypeFix, CommitText) 답습.
3. **Qt IM module** (M) — 구조는 GTK와 대칭. 2번 직후 진행하면 패턴 재사용 효율 최대.
4. **XIM** (M–L) — Xft ★/☆ 렌더 가능성 확인 필요. 신규 signal stream 추가 보일러플레이트 존재.
5. **Wayland** (L) — 전체 재렌더 방식으로 구현 가능하지만, 현 상태에서 Wayland 팝업은 기능이 가장 덜 다듬어져 있음 (참고: `project_popup_architecture` 메모리 — "순수 Wayland 미해결"). **deferred 후보**.

---

## 4. 블로커 · Deferred 판정

| 프런트엔드 | 판정 | 사유 |
|---|---|---|
| GTK Standalone | **GO** | 구조 명확, 공유 dbus_client 라우팅 활용. |
| gtk-common | **GO** | signal subscribe 전례 2건(AutoTypeFix, CommitText), C-API 준비됨. |
| qt-common | **GO** | gtk-common과 대칭 구조, 리스크 낮음. |
| XIM | **GO with caveat** | Xft 별 문자(★U+2605, ☆U+2606) CJK 폰트 fallback 검증 필요. 실패 시 대체 마커(예: `[B]`) 권장. |
| Wayland | **DEFER 권장** | (a) 팝업이 `project_popup_architecture.md`에서 이미 "순수 Wayland 미해결"로 표시됨. (b) text-input-v3 popup surface의 위치·포커스 이슈 상존. (c) 별 렌더 자체는 쉬우나 전체 workflow 검증 비용 큼. 이 PR 범위 밖으로 미루고 별도 이슈 제안. |

---

## 5. 공통 주의사항 (구현 에이전트에게)

- **엔진은 Space 누르면 `PopupKeyResult::ToggleBookmark(idx)` 반환** — 이를 받는 프런트엔드(C/Qt)는 switch문에 해당 분기 추가만 하면 됨. 실제 토글은 엔진이 이미 수행하므로 프런트엔드는 "no-op 또는 consumed"로 처리하고, 상태 반영은 `HanjaBookmarkChanged` signal로만 수행(GNOME 패턴).
- **초기 bookmark 상태 fetch 타이밍**: `ShowHanjaPopup` signal 수신 직후 `GetHanjaBookmarkStates()`를 호출해 `Vec<bool>`을 얻고 첫 렌더에 반영 (GNOME `extension.js:176` 패턴).
- **Style**: GTK/Qt는 `bookmarked` CSS 클래스로 색상 강조, 별 문자는 UTF-8. Xft는 폰트 fallback 확인.
- **3지점 싱크(`feedback_config_3way_sync`)** 영향 없음 — 설정 항목 추가 없음.
