# Hanja Popup 9x9 Expanded Grid — Remaining Frontends

## 베이스 / 브랜치
- 베이스: `develop` HEAD `6c98dbe` (Windows TSF IME PR #1 머지 후)
- 작업 브랜치: `claude/hanja-grid-remaining` (worktree
  `/home/from104/work/unim/.claude/worktrees/agent-a312d99906b0399cd`)

## 커밋 (3개, 독립)

| # | hash | subject | files | +/- |
|---|------|---------|-------|-----|
| 1 | `4ea5fcb` | feat(gtk-im): hanja popup 9x9 expanded grid (Period toggle) | unim-frontends/gtk-common/src/unim_hanja_popup.c | +265 / -75 |
| 2 | `c420bfd` | feat(qt-im): hanja popup 9x9 expanded grid (Period toggle) | unim-frontends/qt-common/{include/unim_hanja_popup.hpp, src/unim_hanja_popup.cpp} | +245 / -70 |
| 3 | `ecaf048` | feat(xim): hanja popup 9x9 expanded grid (Period toggle) | unim-frontends/xim/src/hanja_window.rs | +241 / -16 |

## 빌드/테스트 결과

| toolkit | 결과 | 비고 |
|---------|------|------|
| GTK3 IM module (im-unim.so) | PASS | cmake build, 0 warning / 0 error |
| GTK4 IM module (libim-unim.so) | PASS | cmake build, 0 warning / 0 error |
| Qt5 IM module (libunim.so) | PASS | cmake build, 0 warning / 0 error |
| Qt6 IM module (libunim.so) | PASS | cmake build, 0 warning / 0 error |
| XIM (unim-xim) | PASS | cargo check 0 warning |
| 워크스페이스 cargo check --workspace --lib --tests --bins | PASS | 0 warning / 0 error |
| 엔진 popup_state 단위 테스트 | PASS | 67 passed, 0 failed (cargo test --lib -p unim popup_state) |

## 디자인 키포인트

### 공통 — 모든 프런트엔드
- `PopupState::is_hanja_expanded()` (또는 C-API `unim_popup_get_cols()`)로
  compact(1×9) ↔ expanded(9×9) 분기.
- col 우선 인덱싱: `idx = col * 9 + row`. 글로벌 인덱스 = `current_page * page_size + idx`.
  GNOME extension·GTK Standalone과 일관.
- 푸터 우측에 ⊞ (compact 상태) / ⊟ (expanded 상태) 토글 아이콘.
- Period(.) 키 토글은 엔진이 처리. 프런트엔드는 PopupNavigate에서 cols 변화를
  감지해 위젯 트리만 재구성.
- compact↔expanded 시 cursor 리셋(current_page=0, sel_row=0, sel_col=0)은 엔진이 처리.

### GTK common (commit 1)
- `unim_hanja_popup.c`에 새 body_container(GtkBox) + footer_box(page_label + expand_icon).
- compact 모드 = ListBox(1×9), expanded 모드 = GtkGrid(9×9 + 1 헤더 행).
- 클릭 hit-test: compact는 listbox row index → page-local 인덱스 → ageStart+i,
  expanded는 GtkButton의 g_object_data("unim-global-idx")로 글로벌 인덱스 직결.
  이전 페이지-로컬 인덱스 버그 동시 해결.
- CSS에 grid-cell, grid-row-number, popup-expand-icon 룰 추가 (GTK Standalone과 동일 클래스명).

### Qt common (commit 2)
- 고정 m_labels[9] 풀을 동적 m_cells (std::vector<QLabel*>)로 교체. 모드 전환마다
  body 컨테이너의 자식과 레이아웃을 정리하고 재생성.
- compact = QVBoxLayout, expanded = QGridLayout(9×9 + 1 헤더 행).
- 클릭 hit-test는 unim-global-idx Q_PROPERTY 우선 사용. compact는 i 기반.
- StyleSheet에 gridcell, gridheader 속성 룰 추가.
- `QString::fromUtf8(...)`로 ⊞/⊟ 텍스트 적용 (QStringLiteral 매크로 ternary 한계 회피).

### XIM (commit 3)
- `redraw()`를 dispatcher로 만들고 `redraw_compact` (기존 동작) + `redraw_expanded` 신설.
- expanded 윈도우 크기 = 420×(헤더 + 9 cells + footer + padding). compact는 340×N.
- col 헤더(1~9) row 위에 9×9 cell 격자. 셀 텍스트는 fallback 폰트 경로로 렌더.
- 모드 토글 시 `update_from_navigate`가 prev_expanded ≠ new_expanded를 감지해
  XResizeWindow 발행.

## 알려진 PENDING 항목 (수동 검증)

- [ ] GTK3/GTK4: 실제 IME 환경에서 한자 변환 → Period 키 → 9×9 격자 확인
- [ ] GTK3/GTK4: 격자 셀 클릭 → 해당 한자 입력 확인
- [ ] GTK3/GTK4: 9페이지 이상 후보 보유 시 expanded에서 페이지 네비게이션
- [ ] Qt5/Qt6: 동일 검증 (IME 변환 → Period → 격자 → 클릭/키보드)
- [ ] Qt5/Qt6: HanjaBookmarkChanged 시그널이 expanded grid의 ★ 셀 색에 반영되는지
- [ ] XIM: KDE/non-GNOME X11에서 Period 키 → 윈도우 크기 변경 + 격자 렌더 확인
- [ ] XIM: 셀 텍스트 정렬 (현재 cell_w 기반 left-padding의 시각 품질 확인)

## XIM 한계 (deferred sub-issue)

XIM expanded 모드 9×9 **좌클릭은 현재 keyboard-only로 한정**. 이유:
- XIM 프런트엔드는 합성 키만으로 엔진과 통신 (DBus 직결 없음).
- (col, row) → 글로벌 인덱스 → 합성 키 시퀀스가 자명하지 않다 (sel_col 동기화 위해
  Left/Right 합성 후 Number 합성 + Enter 합성 등 다단계 필요). 단일 클릭 → 단일 키
  매핑이 깨져 race-condition 위험.
- compact 좌클릭 (row → 숫자키 합성)·우클릭(다음 페이지)은 그대로 유지.
- 키보드(↑↓←→ + 1~9 + Enter)로는 expanded 모드도 완전 동작.

후속 PR에서 XIM 합성 키 시퀀스 또는 새로운 IPC 경로(예: 한자 직접 선택용 DBus
호출)로 해결 권장.

## 엔진 변경 — NONE

선행 implementer가 엔진(PopupState, PopupKey::Period, C-API getter,
DBus PopupNavigate payload)을 모두 정비해 두어 본 작업은 **프런트엔드만** 수정.

## 재발 방지 메모

- Qt: `QStringLiteral(MACRO)`은 컴파일 타임만 가능. 런타임 ternary는
  `QString::fromUtf8(...)` 사용.
- GTK common: 새 static 함수가 다른 static 함수에서 참조되면 forward declaration
  반드시 (compile-order 종속성).
- XIM expanded 윈도우 크기 변경은 `set_navigate_state` 호출만으로 안 되고 `XResizeWindow`
  추가 발행 필요. compact↔expanded 전환 감지는 `is_hanja_expanded()` before/after 비교.
