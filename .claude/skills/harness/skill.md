---
name: harness
description: "UNIM 하네스 — 기획→구현→평가 루프 오케스트레이터. Rust IME 코드 변경이 필요한 모든 작업에 사용. /harness, '하네스 실행', '기획부터 해줘', '계획 세우고 구현해줘', '이거 고쳐줘' (코드 수정 동반), '기능 추가해줘' 등의 요청에 트리거. 단순 코드 읽기, 설명 요청, 로그 분석에는 트리거하지 않음."
---

# UNIM Harness — 기획→구현→평가 루프

## 실행 모드: 서브 에이전트

Plan→Code→Review 직렬 파이프라인. 에이전트 간 통신 불필요 — 메인이 결과를 중계한다.

```
[메인] → Agent(planner) → 계획 산출물
[메인] → 직접 구현 (계획 기반)
[메인] → Agent(reviewer) → PASS/FAIL 판정
```

## 에이전트 구성

| 에이전트 | 타입 | 모델 | 역할 |
|---------|------|------|------|
| planner | Plan | opus | 코드 탐색 + 구현 계획 수립 |
| (main)  | —    | —    | 계획에 따라 직접 코드 수정 |
| reviewer| general-purpose | opus | 빌드·테스트·규칙 검증 |

## 워크플로우

### Phase 1: 기획

```
Agent(
  subagent_type: "planner",
  model: "opus",
  prompt: "UNIM 프로젝트에서 다음 작업을 기획하라: {$ARGUMENTS}
           .claude/agents/planner.md의 출력 형식을 따를 것."
)
```

- 결과를 사용자에게 보여주고 승인을 받는다
- 사용자가 수정 요청 시 계획을 조정한다
- 승인 후 Phase 2로 진행

### Phase 2: 구현 (메인 에이전트 직접 수행)

- 계획의 구현 순서를 따라 직접 코드를 수정한다
- 계획에 없는 파일은 수정하지 않는다
- 각 단계 완료 시 TaskUpdate로 진행 상황을 추적한다
- docs/dev/architecture/AGENTS.md 규칙 준수: `unim_log!` 매크로, Core 격리, DBus 통신 등

### Phase 3: 평가

```
Agent(
  subagent_type: "reviewer",
  model: "opus",
  prompt: "UNIM 프로젝트의 코드 변경을 평가하라.
           .claude/agents/reviewer.md의 검증 체크리스트를 모두 수행할 것."
)
```

## 루프 판정

| 결과 | 다음 행동 |
|------|----------|
| PASS | 사용자에게 결과 보고 → 커밋 여부 확인 |
| FAIL | 수정 지시사항 기반으로 Phase 2 재실행 → Phase 3 재평가 |
| 3회 연속 FAIL | 사용자에게 상황 보고 후 판단 요청 |

## 에러 핸들링

| 에러 유형 | 대응 |
|----------|------|
| planner가 파일을 못 찾음 | Glob/Grep 재탐색 지시 후 1회 재시도 |
| 빌드 실패 (warning) | reviewer 수정 지시에 따라 코드 수정 후 재빌드 |
| 테스트 실패 | 실패 테스트 분석 → 코드 수정 → 재테스트 |
| reviewer 타임아웃 | 수동으로 `make build && cargo test --workspace` 실행 |
| 계획 범위 과도 | 단계 분리 제안 후 사용자 승인 |

## 핵심 원칙

- 기획 없이 코딩하지 않는다
- 평가 없이 완료하지 않는다
- 사용자 승인 없이 계획을 변경하지 않는다
- `make build` zero-warning + `cargo test` all-pass가 최소 기준이다

## 테스트 시나리오

### 정상 흐름
1. 입력: "/harness src/config.rs에 새 설정 항목 추가"
2. Phase 1: planner가 config.rs + 6곳 동기화 계획 수립
3. Phase 2: 메인이 계획대로 구현
4. Phase 3: reviewer가 빌드·테스트·동기화 검증 → PASS
5. 사용자에게 커밋 여부 확인

### 에러 흐름 (FAIL → 재시도)
1. 입력: "/harness GTK4 프론트엔드에 새 시그널 핸들러 추가"
2. Phase 1: planner가 계획 수립
3. Phase 2: 메인이 구현 (warning 발생)
4. Phase 3: reviewer가 FAIL 판정 + "unused variable" 수정 지시
5. Phase 2 재실행: warning 수정
6. Phase 3 재실행: PASS
