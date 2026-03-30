# UNIM Harness — 기획→구현→평가 루프

기획(Plan), 구현(Code), 평가(Review) 3단계를 분리된 에이전트가 순환 수행한다.
사용자의 요청을 받아 품질 기준을 통과할 때까지 루프를 반복한다.

## 사용법

```
/harness <작업 설명>
```

## 실행 프로토콜

### Phase 1: 기획 (Plan Agent)

Agent tool로 `subagent_type: "Plan"` 에이전트를 실행한다.

프롬프트:
```
UNIM 프로젝트(/home/from104/work/unim)에서 다음 작업을 기획하라:

작업: $ARGUMENTS

다음을 수행하라:
1. 관련 파일을 탐색하고 현재 코드 상태를 파악
2. CLAUDE.md의 아키텍처 규칙과 컨벤션을 준수하는 구현 계획 수립
3. 변경 대상 파일 목록과 각 파일의 변경 내용을 구체적으로 명시
4. 리스크 또는 주의사항 식별

출력 형식:
- **목표**: 1줄 요약
- **변경 파일**: 파일별 변경 내용 (file:line 형식)
- **구현 순서**: 번호 매긴 단계별 순서
- **검증 방법**: 어떻게 성공을 확인할지
- **리스크**: 주의사항
```

Plan Agent의 결과를 사용자에게 보여주고, 진행 여부를 확인한다.
사용자가 수정을 원하면 계획을 조정한다. 승인하면 Phase 2로 진행.

### Phase 2: 구현 (Main Agent — 직접 수행)

Plan Agent가 작성한 계획을 따라 직접 코드를 수정한다.

규칙:
- CLAUDE.md의 로깅 규칙 (`unim_log!` 매크로) 준수
- 계획에 없는 파일은 수정하지 않음
- 각 단계 완료 시 Task로 진행 상황 추적

### Phase 3: 평가 (Review Agent)

구현 완료 후, Agent tool로 `subagent_type: "general-purpose"` 에이전트를 실행한다.

프롬프트:
```
UNIM 프로젝트(/home/from104/work/unim)의 코드 변경을 평가하라.

다음 검증을 모두 수행하라:

1. **빌드 검증**:
   - `make build` 실행 — warning 0개 필수
   - 실패 시 에러 전문 보고

2. **테스트 검증**:
   - `cargo test --workspace` 실행 — 전부 통과 필수
   - 실패 시 실패한 테스트명과 에러 보고

3. **코드 품질 검증**:
   - `git diff` 로 변경사항 검토
   - CLAUDE.md 규칙 위반 여부 확인:
     * Core(src/)에 UI/플랫폼 의존성 없는지
     * 프론트엔드가 DBus 통해서만 통신하는지
     * println!/console.log 대신 unim_log! 사용하는지
     * Settings 동기화 규칙 (config.rs 변경 시 6곳 동기화)
   - 불필요한 변경, 누락된 변경 확인

4. **결과 보고**:
   - ✅ PASS / ❌ FAIL 판정
   - FAIL 시: 구체적 수정 지시사항 목록
   - PASS 시: 변경 요약 1-3줄
```

### 루프 판정

- **Review PASS** → 사용자에게 결과 보고, 커밋 여부 확인
- **Review FAIL** → 수정 지시사항을 바탕으로 Phase 2 재실행, 다시 Phase 3
- **최대 3회 루프** — 3회 실패 시 사용자에게 상황 보고 후 판단 요청

## 핵심 원칙

- 기획 없이 코딩하지 않는다
- 평가 없이 완료하지 않는다
- 사용자 승인 없이 계획을 변경하지 않는다
- `make build` zero-warning + `cargo test` all-pass가 최소 기준이다
