---
name: pr-merge-windows
description: UNIM Windows 프론트엔드(unim-windows egui / unim-tsf TSF) PR 전용 안전 머지 오케스트레이터. 4단계 파이프라인 — windows-pr-analyzer → windows-build-validator → reviewer → (사용자 승인) → merge-executor. 일반 pr-merge는 Linux IM 5지점 동기화 검증과 make build 위주이므로 Windows 프론트엔드 PR에서는 반드시 본 스킬을 사용한다. 트리거: "윈도우 PR #N 머지", "Windows PR 병합", "윈도우 프론트엔드 PR 머지 하네스".
---

# PR Merge (Windows) — UNIM 윈도우 프론트엔드 PR 머지 오케스트레이터

## 워크플로우 (파이프라인 모드)

```
[windows-pr-analyzer] → [windows-build-validator] → [reviewer] → (사용자 승인) → [merge-executor]
            ↓                       ↓                     ↓
       _workspace/01_*       _workspace/02_*       _workspace/03_*
```

각 단계는 이전 단계의 산출물을 `_workspace/` 에서 읽고, 자신의 결과를 다음 번호 파일로 쓴다. 모든 Agent 호출에 `model: "opus"` 명시.

## Phase 1 — Windows PR 분석 (windows-pr-analyzer)

목적: cfg gate 정합성, Workspace 멤버 정합성, Linux IM 비영향성, 충돌 검증.

호출:
```
Agent({
  description: "Windows PR #<N> 사전 진단",
  subagent_type: "windows-pr-analyzer",
  model: "opus",
  prompt: "PR #<N>(https://github.com/from104/unim/pull/<N>)을 Windows 카테고리로 분석하고 _workspace/01_pr_analysis.md 작성. cfg gate / Cargo workspace 멤버 / Linux IM 비영향 검증 필수."
})
```

산출: `_workspace/01_pr_analysis.md`

게이트: 머지 가능 여부가 `BLOCKED` 이면 즉시 종료.

## Phase 2 — 빌드/테스트 검증 (windows-build-validator)

목적: Linux 회귀 + Windows cross-compile (mingw → msvc → CI fallback).

호출:
```
Agent({
  description: "Windows PR #<N> 빌드 검증",
  subagent_type: "windows-build-validator",
  model: "opus",
  prompt: "PR #<N> 브랜치를 base에 머지 시뮬레이션 후 (1) cargo test --workspace (2) make build (3) cargo check --target x86_64-pc-windows-* 또는 GitHub CI 상태 조회를 실행하고 _workspace/02_build_validation.md 에 LINUX_TEST/LINUX_BUILD/WIN_BUILD/CI_STATUS 4축 결과 기록."
})
```

산출: `_workspace/02_build_validation.md`

게이트:
- LINUX_TEST 또는 LINUX_BUILD = FAIL → 종료
- WIN_BUILD = UNVERIFIED → Phase 4 사용자 승인에서 명시적 확인 필요 표시

## Phase 3 — 코드 리뷰 (reviewer)

목적: UNIM 일반 규약 + Windows 프론트엔드 특수 규약 준수 검증.

호출 (기존 reviewer 재사용):
```
Agent({
  description: "Windows PR #<N> 코드 리뷰",
  subagent_type: "reviewer",
  model: "opus",
  prompt: "PR #<N> 의 변경사항을 _workspace/01_pr_analysis.md 와 함께 검토. 일반 UNIM 규약(unim_log 사용, Core 분리, 에러 핸들링) 외에 다음 Windows 특수 규약을 추가 검증: (a) Linux 전용 코드는 cfg(target_os=\"linux\") 또는 cfg(unix) 가드 (b) Windows 전용 코드는 cfg(windows) 가드 (c) Win32 KeyCode/ModifierState 매핑은 단위 테스트 동봉 (d) unim-windows 는 DBus 의존 없이 in-process Core 사용 (e) unim-tsf 는 com::interfaces / windows-rs 매크로 정합성. _workspace/03_code_review.md 에 PASS/FAIL 판정 기록."
})
```

산출: `_workspace/03_code_review.md`

게이트: FAIL 시 종료, 사용자에게 수정 항목 보고.

## Phase 4 — 사용자 승인 게이트

오케스트레이터는 Phase 1~3 결과를 종합하여 사용자에게 다음을 제시:
- cfg gate 정합성 ✅/❌
- Workspace 멤버 (unim-windows, unim-tsf) 추가 여부
- Linux IM 비영향 ✅/⚠️
- LINUX_TEST / LINUX_BUILD 결과
- WIN_BUILD 검증 방식 (mingw / msvc / CI / UNVERIFIED) 및 결과
- 코드 리뷰 결과
- 머지 모드 권고 (--squash / --merge / --rebase)

WIN_BUILD = UNVERIFIED 인 경우 다음 문구 강제 포함:
> ⚠️ 로컬에서 Windows cross-compile을 검증하지 못했고 CI 결과도 확정되지 않았습니다. 머지를 진행하면 Windows 빌드 회귀 가능성이 있습니다. 그래도 진행하시겠습니까?

사용자가 명시적으로 "머지해", "진행해", "merge" 등을 응답할 때만 Phase 5 진행.

## Phase 5 — 머지 실행 (merge-executor)

호출:
```
Agent({
  description: "Windows PR #<N> 머지 실행",
  subagent_type: "merge-executor",
  model: "opus",
  prompt: "PR #<N>을 <base>(통상 develop) 브랜치에 --<mode> 모드로 머지. 사후 검증으로 cargo test --workspace, make build 재실행 후 _workspace/05_merge_result.md 작성. Windows cross-compile 사후 검증은 windows-build-validator 와 동일 절차로 한 번 더 수행. 실패 시 즉시 git revert -m 1 <merge-commit>."
})
```

산출: `_workspace/05_merge_result.md`

## 데이터 전달 프로토콜
- 파일 기반 (`_workspace/` 보존)
- 파일명 컨벤션: `{phase:02d}_{role}_{artifact}.md`
- 빌드 로그는 `_workspace/02_build_log_linux.txt`, `_workspace/02_build_log_windows.txt`, `_workspace/02_test_log_linux.txt` 로 분리

## 에러 핸들링
- 각 Phase 1회 재시도, 재실패 시 사용자에게 결과 그대로 보고하고 종료
- Phase 1에서 `mergeable=CONFLICTING` 발견 시: 충돌 파일 목록만 보고하고 사용자 결정 대기 (자동 해결 금지). 단 `Cargo.lock` 단독 충돌은 자동 재생성 후보로 제안 가능
- Phase 2에서 LINUX_BUILD/LINUX_TEST FAIL 시: Windows 검증 단계 진입 금지
- Phase 5 머지 후 사후 검증 실패: `git revert -m 1 <sha>` 자동 롤백, 사용자에게 보고
- working tree dirty 상태에서 절대 시작 금지

## 트리거 예시
- "윈도우 PR #1 머지해"
- "Windows 프론트엔드 PR 병합해"
- "PR 1번을 윈도우 머지 하네스로 진행"
- "윈도우 프론트엔드 PR 머지 하네스 실행"

## near-miss (트리거하지 않음)
- 일반 Linux PR (`unim-gui-gtk` 등) → `pr-merge` 스킬 사용
- "윈도우 PR 만들어" → `gh pr create`
- "윈도우 빌드만 검증해줘" → `windows-build-validator` 단독 호출

## 테스트 시나리오

### 정상 흐름
PR #N (cfg gate ✅, Workspace 멤버 추가 ✅, Linux IM 영향 없음, MERGEABLE)
→ Phase1 READY → Phase2 LINUX_PASS + WIN_PASS → Phase3 PASS → 사용자 승인 → Phase5 머지 → 사후 PASS

### Windows 검증 불가 흐름
mingw·msvc 둘 다 없고 CI 미완 → Phase2 WIN_BUILD=UNVERIFIED
→ Phase4 에서 ⚠️ 경고 문구 표시, 사용자 명시 승인 필요

### Linux 회귀 흐름
PR #N 의 src/ 변경이 cfg(target_os="linux") 가드를 누락
→ Phase2 LINUX_BUILD=FAIL → 종료, 가드 누락 위치 보고