---
name: auto-typefix
description: "자동 실시간 한영 오타 수정 기능의 기술 조사, 가능성 탐색, 설계, 구현을 오케스트레이션한다. '자동 오타', '실시간 한영 수정', 'auto typefix', '오타 감지' 언급 시 트리거. 반드시 사용자 중간 승인을 거친 후 구현에 착수한다."
---

# Auto TypeFix — 자동 실시간 한영 오타 수정 오케스트레이터

## 목표

사용자가 한영 전환을 깜빡하고 타이핑했을 때, IME가 **실시간으로 감지하여 자동 교정**하는 기능을 구현한다.

예시:
- 한글 모드에서 "ㅗ디ㅣㅐ" 입력 → "hello" 의도를 감지 → 자동 교정
- 영문 모드에서 "gksrmf" 입력 → "한글" 의도를 감지 → 자동 교정

## 제약 조건

- GNOME+Wayland 전용 (GNOME Shell Extension + DBus + Rust Core)
- IME 키 처리 레이턴시 < 10ms 유지
- 클립보드 사용 금지 (메모리 참조: feedback_no_clipboard_typefix.md)
- 기존 수동 TypeFix(Super+K)와 공존

## 워크플로우 (3 Phase)

### Phase 1: 기술 조사 (팬아웃)

researcher와 analyst를 병렬로 실행하여 기반 기술을 조사한다.

```
[오케스트레이터]
    ├── TeamCreate("auto-typefix-research", [researcher, analyst])
    ├── TaskCreate:
    │   ├── researcher: "IME 자동 오타 수정 기술 조사"
    │   └── analyst: "UNIM 코드베이스 실현 가능성 분석"
    ├── 팀원들이 SendMessage로 교차 피드백
    └── 결과 수집: _workspace/01_*.md, _workspace/02_*.md
```

**researcher 작업:**
- 웹 검색으로 기존 IME 자동 교정 사례 조사
- 한글 특성을 고려한 오타 감지 알고리즘 탐색
- surrounding text 활용 방안 조사
- 성능 제약 조건 정리

**analyst 작업:**
- UNIM 키 처리 파이프라인(press_key → process_korean_key) 분석
- surrounding text 인프라 현황 파악
- typefix.rs 기존 변환 로직 재활용 가능성
- 감지 삽입 지점(integration point) 후보 도출

**산출물:** `_workspace/01_researcher_findings.md`, `_workspace/02_analyst_assessment.md`

### Phase 2: 종합 리포트 + 사용자 승인

Phase 1 결과를 합쳐 종합 리포트를 작성하고 사용자에게 제시한다.

```
[오케스트레이터]
    ├── _workspace/01 + 02 읽기
    ├── 종합 리포트 작성: _workspace/03_feasibility_report.md
    │   ├── 가능한 접근법 비교표
    │   ├── 권장 방안 + MVP 정의
    │   └── 구현 로드맵
    └── AskUserQuestion으로 승인 요청
```

**승인 포인트:**
- 접근법 선택 (규칙 기반 vs 통계 기반 vs 하이브리드)
- 감지 시점 (매 키 vs 단어 완성 시)
- 교정 방식 (자동 교정 vs 제안 vs 알림)
- MVP 범위

### Phase 3: 구현 (승인 후)

승인된 설계를 기반으로 구현한다. 기존 planner + reviewer 에이전트를 활용.

```
[오케스트레이터]
    ├── planner: 구현 계획 수립
    ├── 코드 구현 (직접 또는 빌더 에이전트)
    ├── reviewer: 빌드/테스트 검증
    └── 사용자 최종 확인
```

## 데이터 전달

| Phase | 산출물 | 경로 |
|-------|--------|------|
| 1 | 기술 조사 결과 | `_workspace/01_researcher_findings.md` |
| 1 | 코드 분석 결과 | `_workspace/02_analyst_assessment.md` |
| 2 | 종합 가능성 리포트 | `_workspace/03_feasibility_report.md` |
| 3 | 구현 계획 | `_workspace/04_implementation_plan.md` |

## 에러 핸들링

- 웹 검색 실패 시: 코드베이스 내 기존 typefix.rs 분석으로 대체
- 기술적 불가능 판정 시: Phase 2에서 대안 제시 후 사용자 판단에 위임
- 빌드 실패 시: 1회 재시도, 실패 시 에러 보고 후 사용자 확인

## 테스트 시나리오

### 정상 흐름
1. Phase 1 실행 → 2개 산출물 생성
2. Phase 2 → 종합 리포트 + 사용자 승인
3. Phase 3 → 구현 + 빌드 성공 + 테스트 통과

### 에러 흐름
1. Phase 1에서 "실시간 자동 감지는 기술적으로 불가능" 결론
2. Phase 2에서 대안 제시 (예: "단어 완성 후 감지"로 범위 축소)
3. 사용자가 대안 승인 → Phase 3 진행
