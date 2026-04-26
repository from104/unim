---
name: hanja-grid-rollout
description: UNIM 한자 popup의 9x9 확장 격자 모드(GNOME extension PR #3 두 번째 기능)를 GTK Standalone·GTK IM module·Qt IM module·XIM 프런트엔드에 이식하는 파이프라인을 실행한다. 파악·구현·검증 3개 전문 에이전트 팀을 순차 실행하고, 프런트엔드별 독립 커밋과 PENDING 수동 검증 리포트를 산출한다. "한자 9x9 grid 이식", "한자 팝업 확장 모드 follow-up", "expanded grid rollout" 같은 표현 시 이 스킬을 사용한다.
---

# hanja-grid-rollout — 9x9 확장 격자 follow-up 오케스트레이터

## 상황

PR #3에서 GNOME Shell extension(`unim-gnome-extension/hanja_popup.js`)에는
9x9(81칸) 확장 격자 모드가 도입됐다(Period 키 토글 + ⊞/⊟ 아이콘 + compact 9 ↔
expanded 81 페이지). 이 기능은 **JS 단독 client-side feature** 라 엔진/DBus는
모르는 상태. 다른 프런트엔드(GTK Standalone, gtk-common, qt-common, XIM)는
자체 페이징/렌더 시스템을 갖고 있어 toolkit별로 별도 구현이 필요.

## 실행 모드

**에이전트 팀 (파이프라인 패턴)** — 3명 순차. `_workspace/` 파일 기반 데이터 전달.

```
[리더]
  ├─ Task 1: 파악 (hanja-grid-analyst) → 04_grid_analyst_plan.md
  ├─ Task 2: 구현 (hanja-grid-implementer, 의존: 1) → 05 + git commits
  └─ Task 3: 검증 (hanja-grid-reviewer, 의존: 2) → 06_grid_reviewer_report.md
```

## Phase 1: 팀 구성

`hanja-grid-team` 생성:
- `hanja-grid-analyst` (subagent_type: `researcher`)
- `hanja-grid-implementer` (subagent_type: `general-purpose`)
- `hanja-grid-reviewer` (subagent_type: `reviewer`)

모두 `model: "opus"`.

## Phase 2: 작업 실행

| Task | 담당 | 의존 | 산출물 |
|------|------|------|--------|
| 04-grid-analyst | hanja-grid-analyst | (없음) | 04_grid_analyst_plan.md |
| 05-grid-implementer | hanja-grid-implementer | 04 | 05_grid_implementer_commits.md + commits |
| 06-grid-reviewer | hanja-grid-reviewer | 05 | 06_grid_reviewer_report.md |

## Phase 3: 결과 처리

### PASS 경로
1. 구현된 커밋 목록 표시 (`git log --oneline develop..HEAD`)
2. 사용자에게 push 승인 요청 (필수)
3. 승인 시 `git merge --no-ff` + `git push origin develop`

### FAIL 경로
1. FAIL 항목 사용자에게 제시
2. 사용자 결정: 재구현 / 부분 PASS push / 전체 취소

### PENDING (수동 시각 검증)
9x9 grid는 시각적 정렬·페이지 변환 정확도가 핵심. PASS여도 PENDING 시나리오를
사용자에게 제시하고, 깨지면 후속 커밋으로 보정.

## 데이터 전달

- 파일 기반 (주): `_workspace/0[4-6]_*.md`
- SendMessage (보조): 진행 상황 + 재요청

## 에러 핸들링

| 문제 | 대응 |
|------|------|
| 자체 렌더(XIM) 81칸 좌표 계산 복잡 | 해당 프런트엔드 deferred, 나머지 진행 |
| Period 키가 다른 기능과 충돌 | 다른 키(예: `Insert`, `F2`)로 변경 후 재구현 |
| 엔진 PopupKeyResult variant 추가 필요 | 선행 엔진 커밋 1개만 추가, 변경 최소화 |
| Wayland 팝업 미구현 | 무조건 deferred (북마크 작업과 동일 결정) |
| 시각 정렬 깨짐 | reviewer가 PENDING으로 표시, 사용자 실기 검증 |

**Push 규칙**: 모든 push는 사용자 명시적 승인 후. 검증 PASS ≠ 자동 push.

## 성공 기준

- 4개 프런트엔드 중 **최소 1개 이상**에 9x9 격자 통합 + 빌드/테스트 통과
- deferred는 사유 명시
- 전체 판정 PASS

## 테스트 시나리오

### 정상 흐름
1. analyst가 GTK Standalone(M)·gtk-common(M)·qt-common(M)·XIM(L) 매핑 완료
2. implementer가 GTK Standalone + gtk-common 구현 → 빌드 PASS
3. qt-common 구현 시 QGridLayout vs QListView 결정, 구현 PASS
4. XIM은 좌표 재계산이 너무 복잡하다고 판단 → deferred
5. reviewer가 3개 프런트엔드 PASS, XIM deferred 명시
6. 리더가 사용자에게 요약 → push 승인 → merge

### 에러 흐름
1. implementer가 gtk-common GtkListBox 구조에서 GtkGrid 전환 시 행 번호 표시 셀
   누락
2. reviewer가 "compact 모드에서 행 번호 누락" FAIL 판정
3. 리더가 implementer에 SendMessage로 수정 요청
4. implementer가 행 번호 셀 추가 후 재커밋
5. reviewer 재검증 → PASS

## 참고 파일

- 기준선: `unim-gnome-extension/hanja_popup.js` (9x9 grid JS 구현)
- 이전 follow-up 산출물: `_workspace/01_analyst_hanja_bookmark_plan.md`
  (각 프런트엔드 후보 렌더 함수 위치가 이미 정리됨 — 재활용)
- 팝업 명세: `docs/dev/specs/POPUP_SPEC.md`
