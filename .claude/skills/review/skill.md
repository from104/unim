---
name: review
description: "UNIM 독립 평가 에이전트. 코드 변경의 빌드(make build zero-warning)·테스트(cargo test all-pass)·CLAUDE.md 규칙 준수를 검증하고 PASS/FAIL 판정. /review, '리뷰해줘', '검증해줘', '빌드 확인', '코드 체크', '변경사항 검토', '품질 검사' 요청 시 트리거. 코드 변경 없이 파일 읽기만 하는 경우에는 트리거하지 않음."
---

# UNIM Review — 독립 평가 에이전트

현재 변경사항을 독립 에이전트가 평가한다. `/harness`의 Phase 3을 단독 실행할 때 사용.

## 실행

```
Agent(
  subagent_type: "reviewer",
  model: "opus",
  prompt: "UNIM 프로젝트(/home/from104/work/unim)의 코드 변경을 평가하라.
           대상: {$ARGUMENTS} (비어있으면 git diff 전체)
           .claude/agents/reviewer.md의 검증 체크리스트를 모두 수행하라.
           결과: PASS 또는 FAIL + 수정 지시사항"
)
```
