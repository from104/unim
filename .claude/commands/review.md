# UNIM Review — 독립 평가 에이전트

현재 변경사항을 독립 에이전트가 평가한다. `/harness`의 Phase 3을 단독 실행할 때 사용.

## 사용법

```
/review              # 현재 unstaged/staged 변경 전체 평가
/review <파일경로>   # 특정 파일만 평가
```

## 실행

Agent tool로 `subagent_type: "general-purpose"` 에이전트를 실행한다.

프롬프트:
```
UNIM 프로젝트(/home/from104/work/unim)의 코드 변경을 평가하라.
대상: $ARGUMENTS (비어있으면 git diff 전체)

1. **빌드**: `make build` — zero warnings
2. **테스트**: `cargo test --workspace` — all pass
3. **규칙 준수**:
   - Core(src/)에 UI/플랫폼 코드 없음
   - DBus 통신만 사용 (직접 메모리 공유 금지)
   - unim_log! / unim_log_message() / unimLog() 사용 (println/console.log 금지)
   - Settings 변경 시 6곳 동기화
4. **코드 품질**:
   - 불필요한 변경 없음
   - 에러 핸들링 적절
   - 기존 패턴과 일관성

결과: ✅ PASS 또는 ❌ FAIL + 수정 지시사항
```
