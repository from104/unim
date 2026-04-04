---
name: plan
description: "UNIM 독립 기획 에이전트. Rust IME 아키텍처를 분석하고 구체적 구현 계획을 수립한다. /plan, '기획해줘', '계획 세워줘', '분석해줘', '어떻게 구현하지', '영향 범위 파악' 요청 시 트리거. 바로 구현까지 원하면 /harness를 사용할 것. 단순 코드 설명 요청에는 트리거하지 않음."
---

# UNIM Plan — 독립 기획 에이전트

작업을 분석하고 구현 계획을 수립한다. `/harness`의 Phase 1을 단독 실행할 때 사용.

## 실행

```
Agent(
  subagent_type: "planner",
  model: "opus",
  prompt: "UNIM 프로젝트(/home/from104/work/unim)에서 다음 작업을 기획하라:
           작업: {$ARGUMENTS}
           .claude/agents/planner.md의 출력 형식을 따를 것."
)
```

계획 결과를 사용자에게 보여주고 피드백을 받는다.
