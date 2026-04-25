# Hanja 9x9 Grid Toggle — 프런트엔드 이식 구현 보고서

**작성일**: 2026-04-25
**작업 브랜치**: `claude/hanja-grid-frontends` (베이스: `develop` HEAD `5d5b500`)
**경로 선택**: B (엔진 + 프런트엔드 풀)

## 1. 검증 결과 — 엔진 측 P0 작업 이미 완료

작업 시작 시 분석 계획서(`04_grid_analyst_plan.md`)는 엔진 측 Period/ToggleExpand가
미존재한다고 가정했으나, **실제로는 이미 모두 구현되어 있음**을 확인.

| 항목 | 상태 | 위치 |
|---|---|---|
| `PopupKey::Period` enum variant | ✅ | `src/popup/popup_state.rs:36-37` |
| `PopupState::hanja_expanded: bool` 필드 | ✅ | `src/popup/popup_state.rs:98` |
| `toggle_hanja_expanded()` | ✅ | `src/popup/popup_state.rs:214-235` |
| 한자 모드 Period 키 처리 → `PopupKeyResult::Updated` | ✅ | `src/popup/popup_state.rs:478-482` |
| 동적 cols/rows (compact 1×9 ↔ expanded 9×9) | ✅ | `update_page_layout()` |
| C-API `POPUP_KEY_PERIOD` 상수 + GDK/Qt 매핑 (0x2e) | ✅ | `unim-capi/src/lib.rs:620, 653, 686` |
| C-API getter `unim_popup_get_cols/rows/sel_row/sel_col` | ✅ | `unim-capi/src/lib.rs:848-870` |
| DBus `PopupNavigate` signal payload `rows: i32, cols: i32` | ✅ | `unim-dbus/src/service.rs:1213-1224` |
| 단위 테스트 (12개+) `hanja_period_*` `hanja_expanded_*` | ✅ | `src/popup/popup_state.rs` 테스트 모듈 |

**`cargo test --workspace --lib popup` 결과**: 73 passed, 0 failed (popup 관련 테스트 전부 통과).

→ **엔진 선결 커밋 불필요.** 이미 develop에 머지된 상태.

**미구현 항목**: C 헤더(`unim-capi/include/unim.h`)에 `UNIM_POPUP_KEY_PERIOD`
매크로가 누락 (Rust 측 상수는 존재). 프런트엔드 C 코드에서 직접 참조하지는 않으므로
런타임 영향은 없으나, 일관성을 위해 별도 PR로 추가 권장.

## 2. 프런트엔드 이식 — 1/4 성공, 3/4 미착수 (환경 사고)

### 2-1. GTK Standalone — ✅ **커밋 완료**

| 항목 | 값 |
|---|---|
| 커밋 해시 | `4731764` |
| 메시지 | `feat(gtk-standalone): hanja popup 9x9 expanded grid (Period toggle)` |
| 변경 라인 | +211 / -60 (`unim-gui-gtk/src/hanja_popup.rs`) |
| 빌드 결과 | `cargo check --workspace --lib --tests --bins` 0 warning / 0 error |
| 테스트 | 워크스페이스 전체 73 popup 테스트 통과 유지 |

**구현 내용**:
- `COMPACT_PAGE_SIZE=9, EXPANDED_PAGE_SIZE=81, EXPANDED_COLS=9, EXPANDED_ROWS=9`,
  `ICON_EXPAND='⊞', ICON_COMPACT='⊟'` 상수 추가
- `HanjaPopup` 구조체: `list_box` 필드 → `body_container: gtk4::Box`로 교체
  (compact는 ListBox, expanded는 Grid를 동적 차일드로 보유)
- `expand_icon: gtk4::Label` 추가 (footer에 ⊞/⊟ 표시; 클릭 콜백은 GNOME extension과
  동일하게 미배선 — Period 키만으로 토글)
- `cols: usize`, `sel_row`, `sel_col` 필드 추가
- `update_page()` → `render_list()` (compact) / `render_grid()` (expanded) 분기
- `navigate(page, _total, _selected, _rows, cols, sel_row, sel_col)` 시그니처 변경
  → `cols` 변화 감지 시 위젯 트리 재구성 (GNOME JS `updateFromNavigate` 패턴)
- 9×9 그리드: col 우선 인덱싱 (`offset = col * EXPANDED_ROWS + row`) — 엔진의
  `hanja_global_index_rc`와 일치
- 클릭 핸들러: 페이지 로컬 인덱스 → **글로벌 인덱스**로 변경 (기존 코드의
  잠재적 버그 동시 수정)
- CSS 추가: `popup-footer-box`, `popup-expand-icon`, `hanja-grid`, `grid-row-number`,
  `grid-cell`, `grid-cell-selected`, `grid-cell.bookmarked`

**수동 검증 필요 항목**:
1. `unim-gui-gtk` 실행 후 Period 키로 compact ↔ expanded 전환 확인
2. expanded 모드에서 9×9 그리드 셀 마우스 클릭 → 글로벌 인덱스로 SelectHanja 호출
3. Number(1-9) 키로 expanded 모드의 현재 열 내 행 선택 (엔진이 자동 처리)
4. 화살표 키로 expanded 모드의 (row, col) 이동
5. 페이지 전환 시 cursor 위치 복원 (엔진 `current_page=0, sel_row=0, sel_col=0` 리셋)

### 2-2. GTK IM module (`unim-frontends/gtk-common`) — ⚠️ **DEFERRED (환경 사고)**

작업 도중 작업 환경의 git 브랜치가 `_pr_1_validate` (Windows TSF IME 작업 브랜치)로
외부 요인에 의해 강제 전환되었으며, develop 베이스라인이 다른 상태로 변경됨. 이 시점부터
원래 계획(develop + bookmark merge)에 기반한 추가 작업은 안전성 보장 불가.

**현 상태**: `claude/hanja-grid-frontends` 브랜치에 GTK Standalone 커밋
(`4731764`) 만 존재. 후속 작업 미착수.

**남은 작업 (다음 에이전트가 수행)**:
- `unim_hanja_popup.c`의 `update_listbox()` → compact 분기 + 신규 `update_grid()` 추가
- `MAX_VISIBLE_CANDIDATES=9` → `COMPACT_PAGE_SIZE=9, EXPANDED_PAGE_SIZE=81`
- `popup_state` C-API의 `unim_popup_get_cols()` 반환값으로 분기
- 푸터에 ⊞/⊟ 라벨 추가 (GtkLabel + 정적 텍스트)
- `unim_hanja_popup_handle_key()`는 이미 `UNIM_POPUP_RESULT_UPDATED`를 처리하므로
  Period 키 입력 자체는 자동 동작 (`unim_popup_key_from_gdk(0x2e)` → 엔진)

### 2-3. Qt IM module (`unim-frontends/qt-common`) — ⚠️ **DEFERRED (환경 사고)**

위와 동일 사유. **남은 작업**:
- `unim_hanja_popup.cpp`의 `m_labels` `QLabel[9]` pool → `QGridLayout` 9×9로 마이그레이션
- compact 모드는 column=0의 첫 9개 셀만 사용
- ⊞/⊟ 토글 라벨 추가
- Period 키는 `unim_popup_key_from_qt(0x2e)`로 이미 매핑됨 → keyhandler 수정 불필요

### 2-4. XIM (`unim-frontends/xim/src/hanja_window.rs`) — ⚠️ **DEFERRED (환경 사고)**

위와 동일 사유. 추가로, **분석 계획서에서 가장 고위험으로 평가됨** — Xft 좌표 재계산
및 폰트 fallback 리스크. 별도 분석 단계 권장.

### 2-5. Wayland — **계획대로 DEFER**

`palette analyst` 판정에 따라 본 작업 범위 외.

## 3. 환경 사고 분석

### 사고 시점
GTK Standalone 커밋 `4731764` 직후, GTK common (C) 작업 진입 시 파일 디스크 상태가
"베이스라인(bookmark/grid 미적용)"으로 변경된 system-reminder를 수신. 동시에 `git
branch --show-current` 결과가 `_pr_1_validate`로 변경됨.

### 추정 원인
사용자/외부 도구에 의한 브랜치 강제 전환 (예: `git checkout _pr_1_validate`). 본
에이전트는 브랜치 변경 권한 행사하지 않았으며, 본인이 만든 `claude/hanja-grid-frontends`
브랜치에서 작업 중이었음.

### 영향
- `claude/hanja-grid-frontends` 브랜치의 `4731764` 커밋은 **그대로 보존됨** (git log
  상 확인됨, `git branch -a` 출력에도 존재).
- `_pr_1_validate` 브랜치는 한자 북마크/그리드 작업 미반영된 별개 라인 (TSF IME 중심).
- 두 브랜치 간 working-tree 차이가 커서 단순 `git checkout`으로 복귀 시 충돌 발생.

### 후속 권장 사항
1. 사용자가 의도적으로 브랜치 전환했다면 본 작업 부재 사유 확인.
2. 작업 재개 시 `git checkout claude/hanja-grid-frontends` (필요 시 stash) 후
   가지 2-2 ~ 2-4를 순차 진행.
3. 또는 본 보고서를 참고하여 `_pr_1_validate` 베이스에 동일 패치를 다시 적용할 수도
   있음 — 단, 한자 popup 코드가 `_pr_1_validate`에 존재하는지 별도 확인 필요.

## 4. 페이지 인덱스 변환 / 모드 전환 정책 (구현된 GTK Standalone 기준)

엔진의 `toggle_hanja_expanded()`는 **`current_page=0, sel_row=0, sel_col=0` 으로
리셋**한다. 즉:

- compact의 현재 후보 cursor는 expanded 전환 시 보존되지 않음
- expanded → compact 복귀 시도 cursor 0,0으로 리셋

이는 엔진 차원의 결정이며, 프런트엔드는 PopupNavigate에서 받은 (page, sel_row,
sel_col)을 그대로 따른다. GTK Standalone 구현은 `navigate()`에서 `sel_row`,
`sel_col`을 단순 반영.

대안(절대 인덱스 보존)을 원할 경우 `popup_state.rs:214-235`의
`toggle_hanja_expanded()`를 수정해야 함. 본 PR 범위 외.

## 5. 빌드 환경 사용한 명령

```fish
export PATH="$HOME/.cargo/bin:$PATH"
cargo check --workspace --lib --tests --bins   # 클린 통과
cargo test --workspace --lib popup             # 73 passed
cargo check -p unim-gui-gtk                    # 클린 통과 (GTK Standalone 커밋 후)
```

## 6. 결론

| 항목 | 결과 |
|---|---|
| 엔진 선결 커밋 | 불필요 (이미 머지됨) |
| GTK Standalone | ✅ 1 커밋 (`4731764`) |
| GTK common | ⚠️ 미착수 (환경 사고) |
| Qt common | ⚠️ 미착수 (환경 사고) |
| XIM | ⚠️ 미착수 (환경 사고) |
| Wayland | ─ 계획대로 deferred |

**부분 성공**. 엔진 표면이 이미 완비되어 있다는 검증을 끝낸 것 자체가 후속 에이전트에게
큰 가치를 가진다 — 분석 계획서가 가정한 P0 작업이 모두 불필요하다는 점.
