---
name: pr-merge
description: UNIM 프로젝트의 GitHub PR을 안전하게 병합하는 4단계 오케스트레이터. PR 분석 → 빌드/테스트 검증 → 코드 리뷰(reviewer 에이전트) → 머지 실행 순서로 파이프라인을 돌린다. PR 번호와 함께 "PR #N 병합", "PR #N 머지해", "PR 머지 하네스 실행" 등의 트리거에서 반드시 사용할 것. 5지점 동기화 검증, 충돌 분석, 사후 빌드 검증, 충돌·실패 시 사용자 승인 게이트까지 포함한다.
---

# PR Merge — UNIM PR 머지 오케스트레이터

## 워크플로우 (파이프라인 모드, 서브에이전트)

```
[pr-analyzer] → [build-validator] → [reviewer] → (사용자 승인) → [merge-executor]
                       ↓                    ↓
                  파일 산출물        파일 산출물
              _workspace/02_*    _workspace/03_*
```

각 단계는 이전 단계의 산출물을 `_workspace/` 에서 읽고, 자신의 결과를 다음 번호 파일로 쓴다. 모든 Agent 호출에 `model: "opus"` 명시.

## Phase 1 — PR 분석 (pr-analyzer)

목적: PR의 base/head, 충돌 상태, 5지점 동기화 누락 여부, 변경 영향 범위를 진단.

호출:
```
Agent({
  description: "PR #<N> 사전 진단",
  subagent_type: "pr-analyzer",
  model: "opus",
  prompt: "PR #<N>(https://github.com/from104/unim/pull/<N>)을 분석하고 _workspace/01_pr_analysis.md 작성. 5지점 동기화 검증과 충돌 분석 필수."
})
```

산출: `_workspace/01_pr_analysis.md`

게이트: 머지 가능 여부 필드가 `BLOCKED` 이면 즉시 종료하고 사용자에게 보고.

## Phase 2 — 빌드/테스트 검증 (build-validator)

목적: PR 브랜치를 base에 머지 시뮬레이션 후 zero-warning 빌드와 cargo test --workspace 통과 여부 확인.

호출:
```
Agent({
  description: "PR #<N> 빌드 검증",
  subagent_type: "build-validator",
  model: "opus",
  prompt: "PR #<N> 브랜치를 체크아웃하고 base에 머지 시뮬레이션 후 make build, cargo test --workspace 실행. 결과를 _workspace/02_build_validation.md 에 기록."
})
```

산출: `_workspace/02_build_validation.md`

게이트: BUILD/TEST 중 하나라도 FAIL 시 종료, 사용자에게 수정 항목 보고.

## Phase 3 — 코드 리뷰 (reviewer)

목적: UNIM 규약 준수 검증 (docs/dev/architecture/AGENTS.md/AGENTS.md/docs/dev/architecture/GEMINI.md, 5지점 동기화 정합성, unim_log 사용, Core 분리).

호출 (기존 reviewer 에이전트 재사용):
```
Agent({
  description: "PR #<N> 코드 리뷰",
  subagent_type: "reviewer",
  model: "opus",
  prompt: "PR #<N> 의 변경사항을 _workspace/01_pr_analysis.md 와 함께 검토. UNIM 규약 준수, 5지점 동기화, unim_log 사용, Core 분리, 에러 핸들링을 검증하고 _workspace/03_code_review.md 에 PASS/FAIL 판정 기록."
})
```

산출: `_workspace/03_code_review.md`

게이트: FAIL 시 종료, 사용자에게 수정 항목 보고.

## Phase 4 — 사용자 승인 게이트

오케스트레이터(나)는 Phase 1~3 결과를 종합하여 사용자에게 다음을 제시:
- 5지점 동기화 ✅/❌
- 빌드/테스트 결과
- 코드 리뷰 결과
- 머지 모드 권고 (--squash / --merge / --rebase)
- 충돌 존재 시 충돌 파일 목록

사용자가 명시적으로 "머지해", "진행해", "merge" 등을 응답할 때만 Phase 5 진행.

## Phase 5 — 머지 실행 (merge-executor)

호출:
```
Agent({
  description: "PR #<N> 머지 실행",
  subagent_type: "merge-executor",
  model: "opus",
  prompt: "PR #<N>을 <base> 브랜치에 --<mode> 모드로 머지. 사후 make build, cargo test --workspace 재실행 후 _workspace/05_merge_result.md 작성. 실패 시 즉시 git revert."
})
```

산출: `_workspace/05_merge_result.md`

## 데이터 전달 프로토콜
- 파일 기반 (`_workspace/` 보존)
- 파일명 컨벤션: `{phase:02d}_{role}_{artifact}.md`
- 중간 빌드 로그는 `_workspace/02_build_log.txt` 같이 별도 파일로 분리

## 에러 핸들링
- 각 Phase 1회 재시도, 재실패 시 사용자에게 결과 그대로 보고하고 종료
- Phase 1에서 `mergeable=CONFLICTING` 발견 시: 충돌 파일 목록만 보고하고 사용자 결정 대기 (자동 해결 금지)
- Phase 5 머지 후 빌드 실패: `git revert -m 1 <sha>` 자동 롤백, 사용자에게 보고
- working tree dirty 상태에서 절대 시작 금지

## 트리거 예시
- "PR #7 머지해"
- "PR 7번 안전하게 병합"
- "PR 머지 하네스 실행"
- "이 PR을 main에 머지하기 위한 검증 돌려"

## near-miss (트리거하지 않음)
- "PR 만들어" → gh pr create
- "PR 리뷰만 해줘" → reviewer 단독 호출
- "이 브랜치 빌드 잘 되는지 봐줘" → build-validator 단독

## 테스트 시나리오

### 정상 흐름
PR #N (clean, 5지점 모두 동기화)
→ Phase1 READY → Phase2 PASS → Phase3 PASS → 사용자 승인 → Phase5 머지 → 사후 빌드 PASS

### 충돌 흐름 (Phase 1에서 차단)
PR #N (CONFLICTING)
→ Phase1: 충돌 파일 목록 출력, 머지 가능 여부 = NEEDS_RESOLUTION
→ 종료, 사용자에게 충돌 해결 요청

### 동기화 누락 흐름 (Phase 3에서 차단)
PR #N (config.rs 추가하나 unim-cli 누락)
→ Phase1 ❌ 표시 → Phase3 FAIL → 종료, 사용자에게 누락 항목 보고
