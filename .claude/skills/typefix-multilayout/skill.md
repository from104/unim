---
name: typefix-multilayout
description: "AutoTypeFix 다중 영문 키맵(Dvorak/Colemak/ColemakDh/Workman) 지원 파이프라인. 조사→계획→구현+테스트→검증 4단계를 자동화한다. '다중 키맵', '영문 레이아웃', 'Dvorak AutoTypeFix', 'QWERTY 하드코딩', '키맵 지원' 언급 시 트리거."
---

# TypeFix Multi-Layout — 다중 영문 키맵 지원 파이프라인

## 목표

AutoTypeFix의 QWERTY 하드코딩을 제거하고, 5개 영문 키맵(Qwerty/Dvorak/Colemak/ColemakDh/Workman)
모두에서 순방향(영→한) 및 역방향(한→영) 자동 오타 교정이 동작하도록 한다.

## 핵심 문제

`KeystrokeBuffer::to_ascii_string()` (auto_typefix.rs)이 `KeyCode::to_char()`를 호출하는데,
`to_char()`는 QWERTY 고정이다. 결과적으로:
- **순방향**: check_forward()는 english_layout을 받지만, 입력 ASCII가 이미 QWERTY 기준
- **역방향**: check_reverse()는 english_layout 파라미터 자체가 없음

## 제약 조건

- `cargo build --workspace` zero warning 필수
- `cargo test --workspace` all pass 필수
- Core(src/)에 UI/플랫폼 의존성 금지
- 기존 QWERTY 동작에 대한 회귀 없음
- IME 키 처리 레이턴시 < 10ms 유지

## 워크플로우 (4 Phase 서브 에이전트 파이프라인)

### Phase 1: 정밀 조사 (analyst)

기존 조사 결과가 `references/keymap-analysis.md`에 있다. 이를 기반으로 추가 정밀 분석을 수행한다.

```
[오케스트레이터]
    └── Agent(analyst, model: opus)
        ├── keycode.rs의 to_char()/to_shifted_char() 전체 매핑 테이블 추출
        ├── 5개 영문 JSON 키맵의 물리키 위치 ↔ 문자 매핑 교차 비교
        ├── auto_typefix.rs의 to_ascii_string() 호출 체인 전체 추적
        └── 산출물: _workspace/10_multilayout_analysis.md
```

**analyst 프롬프트 핵심:**
- `references/keymap-analysis.md`를 먼저 읽고 기존 분석 확인
- KeyCode enum의 모든 알파벳/숫자/특수문자 변형을 열거
- 각 KeyCode가 5개 레이아웃에서 어떤 문자에 매핑되는지 교차표 작성
- to_char_for_layout() 구현에 필요한 정확한 매핑 데이터 도출

### Phase 2: 구현 계획 (planner)

Phase 1 결과를 읽고 구체적 구현 계획을 수립한다.

```
[오케스트레이터]
    └── Agent(planner, model: opus)
        ├── _workspace/10_multilayout_analysis.md 읽기
        ├── 수정 파일별 변경사항 (file:line 형식)
        ├── 구현 순서와 의존 관계
        ├── 테스트 전략 수립
        └── 산출물: _workspace/11_multilayout_plan.md
```

**planner 프롬프트 핵심:**
- to_char_for_layout() 구현 방식 결정:
  - 방안 A: KeyCode마다 match 분기로 5개 레이아웃 문자 반환 (정적, 빠름)
  - 방안 B: JSON 키맵 파싱하여 HashMap<(KeyCode, EnglishLayout), char> 빌드 (동적, 유연)
  - 방안 C: lazy_static으로 레이아웃별 역매핑 테이블 캐싱 (절충)
- 성능 영향 분석 (매 키스트로크마다 호출되므로 중요)
- 테스트 매트릭스 정의 (레이아웃 5개 x 기능 3개 x 케이스 N개)

### Phase 3: 구현 + 테스트 (순차)

구현 후 촘촘한 테스트를 작성한다.

```
[오케스트레이터]
    ├── _workspace/11_multilayout_plan.md 읽기
    ├── 직접 구현 (4개 파일 수정)
    │   ├── keycode.rs: to_char_for_layout() + to_shifted_char_for_layout()
    │   ├── auto_typefix.rs: to_ascii_string() 시그니처 변경
    │   ├── auto_typefix.rs: check_reverse() english_layout 추가
    │   └── engine_worker.rs: check_reverse() 호출 수정
    ├── cargo build --workspace (중간 검증)
    └── Agent(test-writer, model: opus)
        ├── 구현된 코드 읽기
        ├── 촘촘한 테스트 코드 작성
        └── 산출물: _workspace/07_test_report.md
```

**테스트 매트릭스 (최소 요구):**

| 테스트 카테고리 | 대상 | 레이아웃 | 예상 수량 |
|---------------|------|---------|----------|
| to_char_for_layout 단위 | keycode.rs | 5개 전체 | 25+ |
| to_shifted_char_for_layout 단위 | keycode.rs | 5개 전체 | 25+ |
| to_ascii_string 레이아웃별 | auto_typefix.rs | 5개 전체 | 10+ |
| check_forward 레이아웃별 | auto_typefix.rs | 5개 전체 | 10+ |
| check_reverse 레이아웃별 | auto_typefix.rs | 5개 전체 | 10+ |
| QWERTY 회귀 | 전체 | Qwerty만 | 기존 전부 |
| 경계값 (빈 버퍼, 특수키) | auto_typefix.rs | 대표 2개 | 5+ |

**촘촘한 테스트 작성 원칙:**
1. 각 레이아웃에서 동일 물리키가 다른 문자를 반환하는지 검증
2. Shift 상태의 레이아웃별 차이 검증 (Dvorak: Shift+2 = '@' vs QWERTY: Shift+2 = '@' — 같은 것도 확인)
3. 순방향: 각 레이아웃에서 "한글"에 해당하는 물리키 시퀀스가 다름을 검증
4. 역방향: 각 레이아웃에서 "hello"에 해당하는 물리키 시퀀스가 다름을 검증
5. 대칭성: eng_to_kor(to_ascii_string(keys, layout), layout) == 한글 결과 일관성

### Phase 4: 최종 검증 (reviewer)

```
[오케스트레이터]
    └── Agent(reviewer, model: opus)
        ├── scripts/verify-multilayout.sh 실행
        ├── make build (C/C++ 프론트엔드 포함)
        ├── cargo test --workspace
        ├── git diff 코드 리뷰
        ├── docs/dev/architecture/AGENTS.md 규칙 준수 확인
        └── PASS/FAIL 판정
```

## 데이터 전달

| Phase | 입력 | 산출물 |
|-------|------|--------|
| 1 조사 | references/keymap-analysis.md | `_workspace/10_multilayout_analysis.md` |
| 2 계획 | _workspace/10 | `_workspace/11_multilayout_plan.md` |
| 3 구현 | _workspace/11 | 소스 코드 변경 + `_workspace/07_test_report.md` |
| 4 검증 | 소스 코드 변경 | PASS/FAIL 판정 |

## 에러 핸들링

- **Phase 1 분석 불충분**: 오케스트레이터가 직접 보완 조사
- **Phase 2 방안 갈등**: 성능 > 유연성 우선순위로 결정, 사용자 확인
- **Phase 3 빌드 실패**: 에러 분석 후 1회 수정 재시도, 실패 시 사용자 보고
- **Phase 3 테스트 실패**: 구현 버그 vs 테스트 기대값 오류 판별 후 수정
- **Phase 4 FAIL**: 구체적 수정 지시사항 받아 Phase 3 재실행

## 테스트 시나리오

### 정상 흐름
1. Phase 1 → 5개 레이아웃 교차표 + 매핑 데이터 생성
2. Phase 2 → 방안 선택 + file:line 수준 구현 계획
3. Phase 3 → 4개 파일 수정 + 80개+ 테스트 작성 + 전부 통과
4. Phase 4 → zero warning + all pass + PASS 판정

### 에러 흐름
1. Phase 3에서 Dvorak 역방향 테스트 실패
2. check_reverse()에서 to_ascii_string()에 레이아웃 전달 누락 발견
3. 수정 후 재테스트 → 통과
4. Phase 4 → PASS

## 실행 방법

이 스킬이 트리거되면 오케스트레이터는 Phase 1부터 순차 실행한다.
각 Phase 완료 후 산출물을 확인하고 다음 Phase로 진행한다.
Phase 2 완료 후 사용자에게 구현 계획을 제시하고 승인을 받은 뒤 Phase 3에 착수한다.
