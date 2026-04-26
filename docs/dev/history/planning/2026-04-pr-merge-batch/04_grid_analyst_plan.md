# Hanja 9x9 Grid Toggle — 프런트엔드 이식 계획서

**브랜치**: develop (HEAD: `5d5b500` Merge hanja bookmark frontends rollout)
**기준선**: `unim-gnome-extension/hanja_popup.js` (PR #3 9x9 그리드 머지 완료)
**선결 의존**: 엔진 측의 `cols`/`rows` 인자는 이미 DBus `PopupNavigate` 시그널 시그니처에
존재(`unim-dbus/src/service.rs:957-958, 1218-1219`, `client.rs:174-175`). 다만 실제로
expanded(`cols>1`) 모드를 활성화하는 **엔진 토글 (Period 키 → ToggleExpand)** 가 아직
없을 가능성이 큼 — `unim-engine/src/popup/`에 `Period`/`ToggleExpand` 매칭 grep 무수확.
**검증 필요 항목 #1**으로 분리.

---

## 1. 기준선 요약 — GNOME Extension JS

`unim-gnome-extension/hanja_popup.js`:

| 요소 | 위치 |
|---|---|
| 상수 `MAX_ROWS=9, MAX_COLS=9, EXPANDED_PAGE_SIZE=81, COMPACT_PAGE_SIZE=9` | 24-27 |
| 아이콘 텍스트 `ICON_EXPAND='⊞', ICON_COMPACT='⊟'` | 30-31 |
| 상태 변수 `_cols`(1=compact, >1=expanded), `_rows`, `_selRow`, `_selCol` | 67-76 |
| `_expandIcon` 위젯 + 클릭 핸들러 → `_onToggleExpand()` | 122-139 |
| `_pageSize() = _cols>1 ? 81 : 9` | 326-328 |
| `_globalIndex(row, col)` — compact/expanded 인덱싱 분기 | 333-340 |
| `_renderBody()` → `_renderGrid()` / `_renderList()` 분기 + 아이콘 텍스트 갱신 | 347-363 |
| `_renderGrid()` 9×9 셀 (`grid-row`, `grid-row-number`, `grid-cell` CSS) | 451-470 |
| `updateFromNavigate(page, sel_row, sel_col, rows, cols, ...)` cols 변경 감지 | 255-265 |

**핵심 모델**: 팝업은 순수 UI. 엔진이 `PopupNavigate`로 `(rows, cols)`를 송신 → JS가
1×9 ↔ 9×9 자동 전환. Period 키 처리는 엔진이 담당 (또는 ⊞/⊟ 클릭 → 콜백 → 엔진 RPC).

**검증 필요 항목 #2**: `_onToggleExpand()` 콜백이 어떤 DBus RPC를 부르는지
(`extension.js`/`dbus_ime.js` 측 추가 RPC) 확인 필요. 기존 GNOME 클라이언트가 이미
ToggleExpand RPC를 호출하지 않으면 엔진/DBus 표면이 미완성이라는 뜻.

---

## 2. 프런트엔드별 매트릭스

### 2-1. GTK Standalone — `unim-gui-gtk/src/hanja_popup.rs` (441 lines)

| 항목 | 현재 상태 |
|---|---|
| 페이지 크기 | `const PAGE_SIZE: usize = 9;` 라인 11 — **하드코딩, compact 전용** |
| 레이아웃 모델 | `gtk4::ListBox` (`list_box`, 라인 16/57) — **순수 linear list** |
| `navigate()` 시그너처 | 라인 210 — `_rows`, `_cols`, `_sel_row`, `_sel_col` 모두 **`_` prefix로 무시** |
| `update_page()` | 라인 148 — 1열 행 생성만 (`gtk4::ListBoxRow::new()`, 라인 160) |
| ⊞/⊟ 토글 추가 위치 | 신규: `HanjaPopup` 구조체에 `expand_icon: gtk4::Label` 필드 + `new()`에 푸터 박스 추가 + GestureClick 연결 → DBus RPC 호출 |
| Period 키 매핑 | **프런트엔드 처리 없음** — 엔진이 한자 팝업 키를 모두 소화 (GNOME 패턴) → 추가 작업 없음 |
| 이식 난이도 | **L** |
| 사유 | linear ListBox → 2D 그리드(`gtk4::Grid` 또는 `Box[Box[…]]`) 모드 분기 필요. `update_page()`를 `update_compact()`/`update_grid()`로 분리. ListBoxRow 기반 selection 모델이 그리드에서는 작동 안 함 (호버/마우스 클릭 직접 처리 필요). bookmark 코드(라인 239-)도 모드별 별도 갱신 경로 필요 |

### 2-2. GTK IM module — `unim-frontends/gtk-common/src/unim_hanja_popup.c` (818 lines)

| 항목 | 현재 상태 |
|---|---|
| 페이지 크기 | `#define MAX_VISIBLE_CANDIDATES 9` 라인 22 — **하드코딩, compact 전용** |
| 레이아웃 모델 | row_box를 `gtk_box_append`로 수직 누적 (라인 183/189) — **linear list** |
| `update_listbox()` | 라인 125-200 — 후보 루프(라인 147)에서 `MAX_VISIBLE_CANDIDATES` 사용 |
| 페이지 인덱싱 | `popup->current_page * MAX_VISIBLE_CANDIDATES + index` (라인 237) |
| `unim_hanja_popup_handle_key()` | 라인 689-720 — `unim_popup_handle_key()` switch (라인 694) |
| ⊞/⊟ 토글 추가 위치 | `popup->expand_icon` 필드 + 푸터 박스(현재 page_label 라인 510). GestureClick으로 → 엔진에 RPC 또는 popup_state에 toggle |
| Period 키 매핑 | `unim_popup_key_from_gdk()` (라인 729) — Period→`UNIM_POPUP_KEY_PERIOD` 매핑 추가 필요 (C-API + 엔진 enum 동시 추가 — **블로커**) |
| `PopupNavigate` cols/rows 수신 | grep 결과 0건 — DBus client에서 cols/rows를 받지 **않음**. `unim_dbus_client.c` 시그널 핸들러도 미연결 |
| 이식 난이도 | **L** |
| 사유 | (a) `MAX_VISIBLE_CANDIDATES`를 동적 page_size로 대체, (b) `update_listbox()`를 모드별 분기, (c) DBus PopupNavigate 시그널에서 cols/rows 파싱 + popup_state 전파, (d) C-API의 popup_handle_key 결과에 `UNIM_POPUP_RESULT_TOGGLE_EXPAND` 추가 |

### 2-3. Qt IM module — `unim-frontends/qt-common/src/unim_hanja_popup.cpp`

| 항목 | 현재 상태 |
|---|---|
| 페이지 크기 | `MAX_VISIBLE_CANDIDATES` (헤더 정의) — 라인 123/276/285/290 — **9 고정** |
| 레이아웃 모델 | `m_labels[MAX_VISIBLE_CANDIDATES]` QLabel 풀 사전 할당 (라인 123) — **선렌더 1D 배열, 그리드 아님** |
| `updateList()` | 라인 283-310 — 9개 라벨 순회 |
| `handleKey()` | 라인 209-243 — `unim_popup_handle_key` 결과 switch. `TOGGLE_BOOKMARK` 분기 이미 있음 (라인 234) |
| ⊞/⊟ 토글 추가 위치 | 푸터 QLabel `m_expandIcon` 신설 + `mousePressEvent` 또는 `QPushButton` |
| `PopupNavigate` cols/rows | grep 결과 0건 — **수신 안 함** |
| 이식 난이도 | **L** |
| 사유 | 1D `m_labels[9]` 배열 → 2D `m_labels[81]` 또는 `QGridLayout` 기반 동적 풀로 재구조화 필요. `MAX_VISIBLE_CANDIDATES`가 곳곳 하드코딩. PopupNavigate cols/rows 수신 신설 |

### 2-4. XIM — `unim-frontends/xim/src/hanja_window.rs`

| 항목 | 현재 상태 |
|---|---|
| 페이지 크기 | `ps.rows()` (PopupState 기반) — 라인 329, 453 — 이미 **동적**. 단 cols 사용 안 함 |
| 레이아웃 모델 | Xft `draw_string_with_fallback` (라인 470) 자체 좌표 렌더 — **자체 렌더** |
| `update_from_navigate()` | 라인 419-431 — `(page, sel_row, sel_col)` 세 인자만 (rows/cols 없음). 호출부 `handler.rs:1185-`에서 PopupNavigate 처리 |
| `redraw()` | 라인 556 — 페이지의 후보를 line_height 단위로 수직 그림 |
| ⊞/⊟ 토글 위치 | Xft 자체 렌더 → 새 영역(footer)에 ⊞ 문자 그리고 클릭 영역(button_press_event 라인 433)에 좌표 비교 분기 추가 |
| Period 키 | XIM은 키를 엔진으로 그대로 포워딩 (`handler.rs:1184` 주석) → 엔진에 Period 추가하면 자동 적용 |
| `PopupState` cols 지원 | rows()는 있고(라인 329), cols 사용 흔적 없음. PopupState 자체에 cols 필드 존재 여부 미확인 (**검증 필요 항목 #3**) |
| 이식 난이도 | **L** |
| 사유 | 자체 렌더 좌표 시스템 (열 단위 cell 위치 계산) 신설. 9×9 = 81 셀 표시 → 윈도우 크기 동적 계산. 마우스 hit-test 좌표 변환 (현재 row 기반 → row+col 기반). Xft hanja 가독성을 위한 폰트 크기 조정. update_from_navigate에 rows/cols 파라미터 추가 |

### 2-5. Wayland — `unim-frontends/wayland/src/popup_renderer.rs`

| 항목 | 현재 상태 |
|---|---|
| 페이지 크기 | `state.hanja_page_items()` 동적, 단 항상 1열로 그림 |
| 레이아웃 모델 | tiny-skia pixmap 자체 드로잉 — **자체 렌더** |
| `render_hanja_page()` | 라인 62-116 — 1열 후보 루프 (`for (i, (hanja, meaning))`, 라인 90) |
| 특수문자 팝업과 비교 | `render_special_from_state()` (라인 117-)는 이미 `state.cols()` 사용 — **그리드 렌더 패턴 재활용 가능** |
| ⊞/⊟ 토글 위치 | tiny-skia로 그리는 footer ("← → 페이지 \| 1~9 선택 \| ESC 취소", 라인 109) 옆에 그릴 수 있으나, **클릭 hit-test 인프라 부재** (text-input-v3 popup surface는 마우스 이벤트 라우팅 미구현) |
| 이식 난이도 | **L+ (블로커)** |
| 사유 | 렌더 자체는 `render_special_from_state()` 그리드 코드 복제로 가능하나, (a) 마우스 클릭/호버를 popup surface에서 받지 못함 → ⊞ 클릭 토글 불가, (b) 토글은 Period 키에만 의존, (c) 기존 Hanja 팝업이 `project_popup_architecture.md`상 "순수 Wayland 미해결" |

---

## 3. 이식 권장 순서 (저위험 → 고위험)

> **P0 — 엔진 선결 작업 (모든 프런트엔드 공통)**
> 1. `unim-engine/src/popup/`의 `PopupKey` enum에 `Period` 추가
> 2. `PopupKeyResult::ToggleExpand` (또는 동등) variant 추가
> 3. Hanja `PopupState`에 `expanded: bool` 필드 + Period 키 처리 → cols/rows 갱신
> 4. C-API `UNIM_POPUP_KEY_PERIOD` + `UNIM_POPUP_RESULT_TOGGLE_EXPAND` 상수 노출
> 5. `unim_popup_key_from_gdk()` / `from_qt()` / X11 keysym 변환에 Period 매핑
> 6. (선택) 별도 RPC `ToggleHanjaExpand()` — 클릭 아이콘용
>
> **P0 검증 필요**: GNOME extension(`extension.js`/`dbus_ime.js`)이 이미 어떤 RPC/키를
> 부르는지 확인. 이미 있다면 그대로 답습, 없다면 신설.

1. **Qt IM module** (L) — `m_labels` pool 재구조화는 크지만 keyhandler 패턴이
   가장 단순하고 PopupNavigate 시그널 추가만 하면 ⊞ 클릭은 `mousePressEvent`로 쉬움.
   **권장 시작점.**
2. **GTK IM module** (L) — Qt와 대칭 구조. C-API 상수가 같이 가야 하므로 Qt와
   같이 작업하면 효율 좋음.
3. **GTK Standalone** (L) — ListBox→Grid 전환 보일러플레이트 큼. 단 호스트
   프로세스가 분리되어 있어 회귀 영향 격리가 가장 쉬움.
4. **XIM** (L) — Xft 자체 렌더 좌표 재계산. ⊞ hit-test 직접 구현. 폰트 fallback
   리스크. 다만 키 처리는 엔진 위임이라 Period 매핑만 끝나면 자동 동작.
5. **Wayland** — **DEFER**.

---

## 4. 블로커 / Deferred 판정

| 프런트엔드 | 판정 | 사유 |
|---|---|---|
| GTK Standalone | **GO (P0 후)** | linear→grid 전환 코스트 크지만 명확한 패턴 |
| gtk-common | **GO (P0 후)** | C-API 상수 동시 추가 필요. PopupNavigate cols 수신 신설 |
| qt-common | **GO (P0 후)** | 가장 좋은 시작점. handleKey switch에 `TOGGLE_EXPAND` 추가 명확 |
| XIM | **GO with caveat** | Xft로 9×9 셀 렌더 시 폰트 크기·여백 튜닝 필요. ⊞ 아이콘 hit-test 좌표 직접 구현 |
| Wayland | **DEFER** | (a) text-input-v3 popup surface가 마우스 입력 못 받음, (b) Hanja 팝업 자체가 미해결 상태(`project_popup_architecture` 메모리), (c) Period 키만 지원하면 동작은 가능하나 ⊞ 클릭 미지원으로 GNOME 패리티 미달성. **별도 이슈로 분리 권장** |

---

## 5. 검증 필요 항목 (구현 에이전트 첫 작업)

1. **엔진 PopupKey/PopupKeyResult enum 현 상태**: `Period`/`ToggleExpand` 부재 여부.
   부재 시 P0 1-5번 모두 신설.
2. **GNOME `_onToggleExpand()` 콜백 경로**: `extension.js` → `dbus_ime.js`에서
   어떤 RPC를 호출하는가? 또는 단순 Period 키를 엔진에 보내고 끝인가?
3. **`PopupState`의 `cols`/`expanded` 필드**: `unim-engine/src/popup/state.rs`에
   이미 있는지. `state.cols()`는 special_popup이 사용 중이므로 메서드 자체는 존재.
   `Hanja` 모드에서 동적으로 cols를 변경하는 공개 API가 있는지 확인.
4. **DBus `PopupNavigate` 페이로드의 cols 실제 값**: 한자에서도 cols≥1 송신 중인지
   (`service.rs:953-981`의 실제 데이터플로우).

---

## 6. 공통 주의사항 (구현 에이전트에게)

- **엔진 위임 모델 유지**: 모든 프런트엔드는 키 처리를 엔진에 위임. ⊞ 클릭 콜백도
  엔진에 RPC로 전달하고, 상태 변화는 `PopupNavigate` 시그널로만 수신.
- **PopupNavigate cols 변화 감지**: GNOME JS 패턴(`updateFromNavigate`에서
  `_cols !== cols` 변화 시 `_renderBody()` 강제) 답습. cols 1↔81 전환 시 위젯
  계층 재구성 필수.
- **CSS 클래스명 통일**: `grid-row`, `grid-row-number`, `grid-cell`, `popup-footer-box`,
  `popup-expand-icon` — GNOME JS 클래스를 GTK/Qt CSS에서도 동일 명칭 사용해
  스타일 시트 공유 가능성 검토.
- **3지점 싱크**(`feedback_config_3way_sync`) **영향 없음** — 새 설정 항목 없음.
- **모바일 운영(기현 환경)**: 9x9 셀은 마우스 클릭 영역이 작아질 수 있음 →
  expanded 모드에서 셀 최소 크기 보장(GNOME JS 라인 462-468 참조).
