---
name: hanja-bookmark-rollout
description: UNIM 한자 북마크 UI를 GNOME extension 외의 나머지 4개 프런트엔드(GTK Standalone·GTK IM module·Qt IM module·XIM·Wayland)에 이식하는 파이프라인을 실행한다. PR #3 follow-up 작업으로, 파악·구현·검증 3개 전문 에이전트 팀을 구성해 순차 실행하고, 프런트엔드별 독립 커밋과 검증 리포트를 산출한다. "한자 북마크 프런트엔드 이식", "PR #3 follow-up", "hanja bookmark rollout" 같은 표현 시 이 스킬을 사용한다.
---

# hanja-bookmark-rollout — PR #3 follow-up 오케스트레이터

## 상황

PR #3 (`423191f Merge PR #3: Hanja bookmarks`) 에서 한자 북마크 기능이 엔진과
GNOME Shell extension에 이식 완료됐다. 나머지 프런트엔드(GTK Standalone·GTK/Qt
IM module·XIM·Wayland)는 **아직 UI 미적용** 상태로 남았다. 엔진·DBus는 준비되어
있으므로 각 프런트엔드에 별 렌더·Space 토글·signal 구독만 붙이면 된다.

## 실행 모드

**에이전트 팀 (파이프라인 패턴)** — 3명 순차. 각 단계 산출물은
`_workspace/` 하위 파일로 전달하며, 팀원 간 직접 `SendMessage`로 조율.

```
[리더(이 스킬)]
  │
  ├─ TeamCreate(hanja-bookmark-team,
  │              [hanja-bookmark-analyst,
  │               hanja-bookmark-implementer,
  │               hanja-bookmark-reviewer])
  │
  ├─ Task 1: 파악 (analyst)
  │   └→ _workspace/01_analyst_hanja_bookmark_plan.md
  │
  ├─ Task 2: 구현 (implementer, 의존: Task 1)
  │   └→ _workspace/02_implementer_commits.md + git commits
  │
  ├─ Task 3: 검증 (reviewer, 의존: Task 2)
  │   └→ _workspace/03_reviewer_report.md
  │
  └─ 종합 + push 또는 재작업
```

## Phase 1: 팀 구성

리더는 `TeamCreate`로 `hanja-bookmark-team` 을 생성하고 3명의 에이전트를 등록한다:

- `hanja-bookmark-analyst` (subagent_type: `researcher`)
- `hanja-bookmark-implementer` (subagent_type: `general-purpose`)
- `hanja-bookmark-reviewer` (subagent_type: `reviewer`)

모든 에이전트는 `model: "opus"` 지정.

## Phase 2: 작업 생성 및 실행

`TaskCreate`로 3개 작업을 등록하며, 의존성으로 순차 실행을 강제한다:

| Task | 담당 | 의존 | 산출물 |
|------|------|------|--------|
| 01-analyst | hanja-bookmark-analyst | (없음) | 01_analyst_hanja_bookmark_plan.md |
| 02-implementer | hanja-bookmark-implementer | 01-analyst | 02_implementer_commits.md + git commits |
| 03-reviewer | hanja-bookmark-reviewer | 02-implementer | 03_reviewer_report.md |

각 에이전트는 입력 파일을 읽고, 작업 후 지정된 산출물 파일을 작성한다.

## Phase 3: 결과 처리

### PASS 경로
검증 에이전트가 전체 PASS 리포트하면 리더는:
1. develop 브랜치에서 구현 커밋들 확인 (`git log --oneline develop..HEAD`)
2. 사용자에게 push 승인 요청 (push 전에 반드시 승인 받기)
3. 승인 시 `git push origin develop` 실행
4. `_workspace/` 파일들을 감사 추적용으로 보존

### FAIL 경로
검증 에이전트가 FAIL 리포트하면 리더는:
1. `_workspace/03_reviewer_report.md` 의 FAIL 항목을 사용자에게 제시
2. 사용자 결정 대기:
   - (a) 구현 에이전트 재호출하여 수정 (팀 유지, Task 02를 복원 후 재실행)
   - (b) FAIL 항목을 deferred로 두고 PASS 부분만 push
   - (c) 전체 취소하고 `git reset` 으로 커밋 되돌림

### PENDING (수동 확인)
GUI 시맨틱은 자동 테스트로 완전히 커버하기 어려우므로, 검증 에이전트가 `PENDING`
항목을 남기면 사용자에게 실기 테스트 요청:
- `unim-gui-gtk` Standalone 팝업 실행 → 한자 변환 → Space로 별 토글 확인
- 다른 프런트엔드에서 동일 단어 한자 변환 → 별 상태 실시간 동기화 확인

## 데이터 전달 프로토콜

**파일 기반 (주)** + **SendMessage (진행 상황 공유)**

모든 중간 산출물은 `_workspace/` 에:
```
_workspace/
├── 01_analyst_hanja_bookmark_plan.md
├── 02_implementer_commits.md
└── 03_reviewer_report.md
```

`_workspace/` 는 최종 감사 추적용으로 **유지** (삭제 금지). `.gitignore` 에
추가되어 있지 않으면 리더가 `echo "_workspace/" >> .gitignore` 체크.

## 에러 핸들링

| 문제 | 대응 |
|------|------|
| 파악 에이전트가 deferred 프런트엔드를 너무 많이 식별 | 리더가 사용자에게 확인, 범위 축소 제안 |
| 구현 에이전트가 프런트엔드 1개에서 실패 | 해당 프런트엔드 reset, 나머지 계속 (부분 PR) |
| 구현 중 엔진 API 누락 발견 | 중단, 엔진 수정은 별도 태스크 — 이 스킬 범위 밖 |
| 검증 에이전트 FAIL | 구현 에이전트 재호출 (최대 2회), 2회 실패 시 사용자 판단 |
| cargo 빌드 환경 문제 | 검증 보류, 환경 점검 후 재시도 |

**Push 규칙** (중요): 모든 push는 **사용자의 명시적 승인** 후에만. 검증 PASS가
자동 push를 의미하지 않는다.

## 성공 기준

- 4개 프런트엔드 중 최소 1개 이상에 북마크 UI 통합 + 빌드 · 테스트 통과
- deferred 프런트엔드는 사유와 함께 리포트에 명시
- 검증 리포트에 FAIL 항목 0개
- `origin/develop` 에 push된 후 PR 없음 (직접 commit 방식)

## 테스트 시나리오

### 정상 흐름
1. 리더가 팀 구성 → 3개 Task 등록
2. analyst가 GTK Standalone + gtk-common은 구현 가능, Qt IM은 "현재 한자 popup 미구현"
   으로 판정
3. implementer가 GTK Standalone + gtk-common 2개 프런트엔드 커밋 생성
4. reviewer가 양쪽 PASS 판정, Qt/XIM/Wayland는 "이 PR 범위 밖" 으로 명시
5. 리더가 사용자에게 커밋 요약 제시 → push 승인 받음 → 완료

### 에러 흐름 (커밋 빌드 실패)
1. implementer가 gtk-common 구현 중 C 파일에서 `unim_dbus_client.c` 의
   `GetHanjaBookmarkStates` wrapper 누락 발견
2. 리더에게 SendMessage로 "DBus client wrapper 필요" 보고
3. 리더가 분석 — 엔진 수정 아니라 gtk-common 내부 C 코드에 wrapper 추가만 필요
4. implementer가 wrapper 추가 후 재시도 → 성공
5. 통상 흐름으로 복귀

## 참고 파일 (로드 필요 시)

- PR #3 본체 구현: `unim-gnome-extension/hanja_popup.js`, `src/hangul/hanja_bookmark.rs`
- DBus surface: `unim-dbus/src/service.rs` (`GetHanjaBookmarkStates`,
  `ToggleHanjaBookmark`, `HanjaBookmarkChanged`)
- 팝업 명세: `docs/specs/POPUP_SPEC.md`
- 프런트엔드 목록: `unim-frontends/` (gtk3/gtk4/gtk-common/qt5/qt6/qt-common/xim/wayland)
