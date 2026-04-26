---
name: release-orchestrator
description: UNIM 0.2.0 릴리즈 파이프라인 오케스트레이터. release-cleanup → (i18n-applier ∥ doc-writer ∥ manual-test-planner) → release-qa 순서로 서브 에이전트를 호출하고 산출물을 종합. "릴리즈 준비", "0.2.0 릴리즈 점검 시작", "릴리즈 파이프라인 실행", "릴리즈 작업 진행" 요청 시 반드시 트리거.
---

# Release Orchestrator — 0.2.0 릴리즈 파이프라인

## 실행 모드
**서브 에이전트 모드**. 메인이 단계별로 Agent 도구를 호출, 산출물은 `_workspace/release/`에 파일 기반으로 전달.

## 파이프라인

```
Phase 0: release-cleanup        (sequential, 단일)
   ↓ _workspace/release/00_cleanup_report.md
Phase 1: 병렬 fan-out
   ├── i18n-applier            → 02_i18n_report.md
   ├── doc-writer              → 03_doc_report.md
   └── manual-test-planner     → 01_test_plan_report.md (+ docs/release/0.2.0/)
   ↓ (모두 완료 시)
Phase 2: release-qa             (sequential, 단일)
   ↓ _workspace/release/04_qa_report.md
최종: 사용자에게 종합 보고
```

## Phase 0 호출

```
Agent(
  subagent_type: "general-purpose",
  model: "opus",
  description: "Release cleanup",
  prompt: "/home/from104/work/unim/.claude/agents/release-cleanup.md를 따라 0.2.0 릴리즈 정리를 수행하라.
           정리 후 `_workspace/release/00_cleanup_report.md`로 결과 출력.
           위험 작업(force push, hard reset, rm -rf 임의 디렉토리) 절대 금지.
           git status로 의도된 변경만 staged 됐는지 확인하고,
           make build warning 0 유지 검증."
)
```

## Phase 1 병렬 호출 (한 메시지에서 3개 동시 spawn)

```
Agent(subagent_type: "general-purpose", model: "opus",
      description: "i18n applier",
      prompt: "/home/from104/work/unim/.claude/agents/i18n-applier.md 따라 i18n 적용...
               결과 _workspace/release/02_i18n_report.md")
Agent(subagent_type: "general-purpose", model: "opus",
      description: "doc writer",
      prompt: "/home/from104/work/unim/.claude/agents/doc-writer.md 따라 문서 작성 + 라이브 도움말...
               결과 _workspace/release/03_doc_report.md")
Agent(subagent_type: "general-purpose", model: "opus",
      description: "manual test planner",
      prompt: "/home/from104/work/unim/.claude/agents/manual-test-planner.md 따라 시나리오 설계...
               결과 _workspace/release/01_test_plan_report.md")
```

## Phase 2 호출

```
Agent(subagent_type: "general-purpose", model: "opus",
      description: "Release QA",
      prompt: "/home/from104/work/unim/.claude/agents/release-qa.md 따라 8개 항목 검증.
               결과 _workspace/release/04_qa_report.md")
```

## 데이터 전달
- 파일 기반: `_workspace/release/<NN>_<agent>_*.md`
- 모든 산출물 보존(감사용)
- 최종 종합은 메인이 04_qa_report.md를 읽어 사용자에게 요약

## 에러 핸들링
- 각 Phase 실패 시 1회 재시도
- 재실패 시: 해당 단계 결과 SKIP/FAIL로 표시하고 다음 Phase 진행
- BUILD/TEST FAIL은 critical, 다른 Phase 결과와 무관하게 사용자에게 즉시 보고

## 안전 규칙 (모든 Phase 공통)
- force push, hard reset, rm -rf 임의 디렉토리 금지
- target/ 외 디렉토리 통째 삭제 금지
- git commit은 메인이 사용자 승인 후에만 실행 (에이전트는 stage만)
- 큰 출력은 _workspace/ 파일로, 컨텍스트 보호

## 종합 보고서 양식 (메인이 작성)

`_workspace/release/REPORT.md`:
```markdown
# UNIM 0.2.0 Release Pipeline Report

## Phase별 요약
- Phase 0 cleanup: <한 줄 요약>
- Phase 1 i18n: <한 줄 요약>
- Phase 1 docs: <한 줄 요약>
- Phase 1 tests: <한 줄 요약>
- Phase 2 QA: PASS/FAIL/WARN

## 머지 권고
가능 / 수정 필요

## 사용자 판단 필요 항목
- ...

## 후속 작업 제안
- ...
```

## 테스트 시나리오

### 정상 흐름
1. Phase 0 — 잡파일 N개 정리, 보고서 출력
2. Phase 1 병렬 — 3개 에이전트 동시 실행, 각각 산출물 생성
3. Phase 2 — QA 모든 PASS, 머지 권고 가능

### 에러 흐름
- Phase 0 도중 빌드 실패 → 그 자리에서 정지, 사용자에게 알림
- Phase 1 i18n에서 빌드 깨짐 → docs/tests는 계속 진행, QA에서 종합
- Phase 2 QA FAIL → 보고서 출력 후 사용자에게 수정 영역 제안
