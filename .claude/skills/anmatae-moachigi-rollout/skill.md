---
name: anmatae-moachigi-rollout
description: UNIM에 안마태(Ahnmatae) 자판 표준안과 모아치기(moachigi) 입력 방식을 도입하는 6단계 오케스트레이터. 자판 프로필 v3 schema(layout_type=anmatae, moachigi 블록, jamo_symbol_map) 신설, HangulComposerAnmatae 코어 추가, 안마태 키맵 JSON 작성, rule_set 기반 모아치기 토글 통합, 5지점 config 동기화, GTK SwitchRow UI, 사용자 가이드까지 영구 6인 팀(pm·source·engine·ui·doc-promo·user-rep)에 분배 위임한다. 고어 자모(ㆍ 등) 자리는 한글 조판 기호로 대체하며 매핑 결정은 사용자 승인 게이트. 트리거: "안마태", "Ahnmatae", "모아치기", "moachigi", "한글 자판 표준안 추가", "세벌식 모아치기 옵션", "키맵 룰셋 토글 모아치기" 같은 요청.
---

# Anmatae & Moachigi Rollout — 안마태 자판 + 모아치기 도입 오케스트레이터

UNIM에 안마태 자판 표준안과 모아치기 동작을 도입하는 작업을 영구 6인 팀(pm·source-manager·engine-frontend-manager·ui-manager·doc-promo-manager·user-rep-reviewer)에 6단계로 분배 위임한다.

## 작업 범위

**도입 대상:**
- 안마태 자판 — libhangul XML 기반 4행 배열, 음절 경계 알고리즘(영역 채움), 종성 양방향 결합
- 모아치기 — 영역 간 순서 자유, 종성 영역 내부 순서 자유. **rule_set 토글로 켜고 끄기**. 세벌식 자판에도 부분 적용 가능
- 자판 프로필 v3 schema — `layout_type`, `moachigi` 블록, `jamo_symbol_map`, `rule_sets[].moachigi_overrides`

**도입 제외:**
- 한글 고어 자모 (ㆍ 등). 안마태 원본의 고어 자리는 한글 조판용 기호(가운뎃점·따옴표 등)로 대체. 매핑 결정은 사용자 승인 필수.

## 사전 조건 검증

작업 시작 전 다음을 확인 (없으면 사용자에게 보고 후 중단):

- [ ] `docs/references/research/안마태 자판 조사.md` 존재
- [ ] `docs/references/research/쿼티형 세벌식 초안.md` 존재
- [ ] `docs/dev/plans/LAYOUT_PROFILE_V1.md` 및 `V2.md` 존재 (v3는 이번 작업으로 신설)
- [ ] `src/hangul/composer.rs`, `composer_with_3bul.rs` 빌드 가능 상태
- [ ] develop 브랜치 깨끗 (uncommitted 0)

## 데이터 전달 디렉토리

작업 중간 산출물은 모두 `_workspace/anmatae/` 하위에 저장:

```
_workspace/anmatae/
├── 00_user_decisions.md          ← Phase 0 사용자 승인 사항
├── 01_schema_v3.md               ← Phase 1 v3 schema 명세
├── 02_composer_design.md         ← Phase 2 HangulComposerAnmatae 설계
├── 03_keymap_anmatae.json        ← Phase 3 안마태 자판 JSON 초안
├── 03_keymap_moachigi_3bul.json  ← Phase 3 세벌식+모아치기 JSON 초안
├── 04_symbol_replacement.md      ← Phase 3 고어→기호 매핑 (사용자 승인 게이트)
├── 05_integration_plan.md        ← Phase 4 5지점 동기화 + UI 계획
├── 06_test_matrix.md             ← Phase 5 테스트 매트릭스
└── 07_release_notes_draft.md     ← Phase 6 사용자 가이드 초안
```

## 6단계 위임 흐름

> **PM doctrine 준수**: 메인 세션이 PM 역할을 맡고, 매니저는 `Agent` 도구 동기 호출. 다단계 위임 금지. SendMessage 비동기 inbox 사용 금지.
> **모델 정책 준수**: 분석·검증 단계는 매니저 정의에 명시된 model 그대로(opus 또는 sonnet) 사용.

---

### Phase 0 — 사용자 결정 게이트 (메인 세션)

매니저 위임 전에 메인이 사용자에게 다음 결정을 받아 `_workspace/anmatae/00_user_decisions.md`에 기록:

1. **안마태 변종 선택** — 2003 표준안(김진형) / 신세벌식M / 기타. 기본 권장: 2003 표준안.
2. **고어 자모 처리 정책** — 본 작업은 고어 제외 확정. 자리에 들어갈 한글 조판 기호 후보 제시 시점은 Phase 3.
3. **모아치기 디폴트 룰셋** — 신규 자판 설치 시 기본 활성화할 룰셋. 후보:
   - `moachigi_jong_unordered` (종성 순서 자유)
   - `moachigi_region_free` (영역 간 순서 자유)
   - 둘 다 / 모두 OFF
4. **세벌식 모아치기 옵션 적용 범위** — `ko_3bul390` / `ko_3bul391` / `ko_3bul_qwerty` / 신규 `ko_3bul_moachigi`만
5. **빌트인 포함 여부** — v0.3.0 빌트인 자판으로 추가할지, 사용자 자판(`~/.config/unim/layouts/`) 예시로만 둘지

사용자 응답을 받기 전에는 다음 Phase 진행 금지.

---

### Phase 1 — v3 schema 설계 (engine-frontend-manager)

**위임 대상**: `engine-frontend-manager` (책임 영역: 설정 코어 src/config.rs + 입력 엔진. v3 schema는 LayoutProfile 내부 표현 확장 + loader 변경이므로 본 매니저 영역)

**Agent 동기 호출 prompt 골자**:
```
[작업 ID] anmatae-{YYYYMMDD-NN}-phase1
[목적] 자판 프로필 v3 schema 설계. v1/v2 호환 유지, layout_type/moachigi/jamo_symbol_map/rule_sets.moachigi_overrides 신설
[입력]
  - _workspace/anmatae/00_user_decisions.md
  - docs/references/research/안마태 자판 조사.md (§6 JSON 구조 초안, §9 영향 범위)
  - docs/references/research/쿼티형 세벌식 초안.md (§6 모아치기 고려)
  - docs/dev/plans/LAYOUT_PROFILE_V1.md, V2.md
[제약]
  - 코드 변경 금지 (이번 Phase는 설계 문서만)
  - v1/v2 자판 10종 후방 호환 절대 보장
  - 모아치기 활성화 토글은 별도 bool 필드 추가 금지. active_rule_sets 포함 여부로만 결정 (사용자 명시 요구)
  - 고어 자모는 합성 큐 진입 금지 (jamo_symbol_map은 즉시 commit)
[출력]
  - _workspace/anmatae/01_schema_v3.md (references/v3-schema-template.md 구조 따름)
  - 다음 Phase에서 즉시 사용할 Rust struct 시그니처(LayoutType, MoachigiSpec, SymbolEmit) 포함
[보고] 단일 응답으로 종합 보고
```

검수: 메인이 산출물을 읽고 후방 호환 항목 OK 확인 후 Phase 2 진행.

---

### Phase 2 — HangulComposerAnmatae 코어 + composer trait 확장 (engine-frontend-manager)

**위임 대상**: `engine-frontend-manager`

**핵심 산출물**:
- `src/hangul/composer_with_anmatae.rs` 신설
- `src/hangul/composer.rs::HangulComposer` trait에 `add_jamo_with_region`(또는 동등) 확장. default impl은 기존 add_jamo_with_meta 위임으로 후방 호환
- 영역별 버퍼링 큐 (cho_region, jung_region, jong_region)
- 음절 경계 알고리즘 — `region_filled` 모드: 영역이 채워졌고 같은 영역 키가 다시 들어오면 force_compose
- 종성 양방향 결합 (jong_unordered): `(ㄹ, ㄱ) → ㄺ` 양방향 자동 등록
- 세벌식 composer가 `moachigi_overrides` rule_set 활성화 시 부분 동작 적용 (예: `ko_3bul_moachigi` 룰셋 활성 시 종성만 unordered)

**Agent 동기 호출 prompt 골자**:
```
[작업 ID] anmatae-{YYYYMMDD-NN}-phase2
[목적] HangulComposerAnmatae 신규 + HangulComposer trait region 확장 + 세벌식 부분 적용
[입력]
  - _workspace/anmatae/01_schema_v3.md (Phase 1 산출물)
  - src/hangul/composer.rs, composer_with_3bul.rs (현재 구조 참고)
[제약]
  - 빌드 zero-warning, cargo test --workspace 통과 필수
  - 두벌식·세벌식 기존 테스트 회귀 0
  - 새 trait method는 default impl로 외부 구현체 영향 0
  - unim_log!() 사용, println!/eprintln! 금지
[출력]
  - 신규/수정 파일 목록 + diff 요약을 _workspace/anmatae/02_composer_design.md
  - 단위 테스트: 영역 채움/종성 양방향/모아치기 토글 OFF→ON 회귀 각 5개 이상
[보고] L1·L2 검증 결과(cargo test -p unim, --workspace) 포함
```

검수: 메인이 빌드/테스트 PASS 확인 후 Phase 3 진행.

---

### Phase 3 — 안마태 자판 JSON + 고어→기호 매핑 (engine-frontend-manager + 사용자 승인 게이트)

**위임 대상**: `engine-frontend-manager`

**핵심 산출물**:
- `_workspace/anmatae/03_keymap_anmatae.json` — libhangul XML 기준 4행 안마태 v3 JSON (cho/jung/jong 영역, combinations 거센·된·복모음·겹받침 전체)
- `_workspace/anmatae/03_keymap_moachigi_3bul.json` — 기존 `ko_3bul390` 또는 `ko_3bul_qwerty` 베이스 + `inherits` + `moachigi_overrides` rule_set
- `_workspace/anmatae/04_symbol_replacement.md` — 안마태 원본 고어 자모 위치 N개의 기호 후보 매트릭스 (각 위치당 후보 3종 이상, references/symbol-candidates.md 참고)

**사용자 승인 게이트 (메인 세션)**:
- 매니저가 `04_symbol_replacement.md`를 산출하면 메인은 사용자에게 후보 매트릭스를 제시하고 단일 시안 결정 받음
- 결정된 매핑을 `03_keymap_anmatae.json`의 `jamo_symbol_map`에 반영하도록 매니저 재호출

검수: 사용자 승인 매핑이 JSON에 정확히 반영됐는지 메인이 직접 grep 확인.

---

### Phase 4 — 5지점 config 동기화 + GTK UI (engine-frontend-manager + ui-manager 직렬)

**4-A. config 5지점 동기화 (engine-frontend-manager)**

새 필드:
- `korean.active_rule_sets`는 이미 존재 — 모아치기 룰셋도 같은 메커니즘 사용. **신규 config 필드 추가 없음** (사용자 요구: "키맵 룰셋으로 토글" 명시)
- `korean.custom_layout`도 이미 존재 — 안마태는 `"anmatae"` 또는 사용자 자판 경로 식별자
- 즉, **5지점 동기화는 기존 메커니즘으로 자동 흡수**. 매니저는 회귀 테스트만 추가

**4-B. GTK 설정 UI (ui-manager)**

```
[작업 ID] anmatae-{YYYYMMDD-NN}-phase4ui
[목적] settings_dialog 한국어 자판 ComboRow에 "안마태"/"세벌식+모아치기" 선택지 추가, 모아치기 룰셋 SwitchRow 동적 재구성
[입력]
  - _workspace/anmatae/01_schema_v3.md (rule_sets 메타)
  - 현 settings_dialog의 한국어 자판 ComboRow 구현
[제약]
  - 슬라이더/스피너 정책 무영향(본 작업은 ComboRow + SwitchRow만)
  - rust-i18n + locale yml ko/en 동시 갱신
  - 기존 v1/v2 자판 표시 회귀 0
[출력]
  - 변경 파일 + 스크린샷 텍스트 묘사 _workspace/anmatae/05_integration_plan.md
[보고] 단일 응답
```

---

### Phase 5 — 테스트 매트릭스 + 회귀 검증 (engine-frontend-manager + user-rep-reviewer)

**5-A. 테스트 매트릭스 (engine-frontend-manager)**

`_workspace/anmatae/06_test_matrix.md`:
- 단위(schema 라운드트립 v1/v2/v3 × moachigi 4종 = 16종)
- 단위(HangulComposerAnmatae 영역 채움/양방향/force_compose 각 10종)
- 통합(input_engine 키 입력 → preedit/commit, 안마태/모아치기 3bul 각 20 시나리오)
- 회귀(두벌식·세벌식 기존 테스트 전부 PASS)

**5-B. 사용자 시점 검증 (user-rep-reviewer)**

```
[작업 ID] anmatae-{YYYYMMDD-NN}-phase5qa
[목적] 안마태/모아치기 신규 자판의 사용성·접근성·기현 키 시퀀스 부담 검증
[입력] _workspace/anmatae/00..06 전체
[제약] 빌드/테스트 검증이 아닌 사용자 시점만
[출력] PASS/WARN/FAIL + 개선 권고 _workspace/anmatae/qa_user_rep.md
[보고] 단일 응답
```

---

### Phase 6 — 문서·홍보·릴리스 준비 (doc-promo-manager + source-manager)

**6-A. 사용자 가이드 (doc-promo-manager)**
- `docs/user/keymaps/anmatae.md` (한/영)
- `docs/user/keymaps/moachigi-on-3bul.md` (한/영)
- 키 배열 ASCII 다이어그램 + 모아치기 룰셋 토글 설명
- 릴리스 노트 초안 `_workspace/anmatae/07_release_notes_draft.md`

**6-B. 브랜치 정리 (source-manager)**
- feature 브랜치 `feat/anmatae-moachigi` 정합성 점검
- CHANGELOG 업데이트 초안
- **머지/태그/push는 사용자 명시 승인 후만** (memory: feedback_main_release_approval.md)

---

## 위험 게이트 (메인 세션이 사용자 승인 받기)

다음은 매니저 자체 결정 금지, 메인이 사용자 승인 받아 진행:

| 단계 | 항목 | 근거 |
|------|------|------|
| Phase 0 | 안마태 변종 선택, 디폴트 룰셋 | 사용자 선호 |
| Phase 3 | 고어→기호 매핑 단일 시안 | 사용자 명시 요구 |
| Phase 6 | git push, develop 머지, 릴리스 태그 | feedback_main_release_approval.md |
| Phase 6 | 빌트인 포함 여부 (Phase 0 결정 재확인) | 사용자 정책 |

## 에러 핸들링

| 에러 | 1차 대응 | 재실패 시 |
|------|---------|---------|
| Phase 1 schema 후방 호환 의문 | 매니저에 "v1/v2 게이트 식별 알고리즘 보강 + 회귀 테스트 추가" 재호출 | 메인이 사용자에게 보고, Phase 2 진행 보류 |
| Phase 2 cargo test 실패 | 매니저에 "실패 테스트의 expected/actual 첨부 + 구현 vs 테스트 오기 판별" 재호출 | 메인이 사용자에게 진단 보고, 수정 방향 합의 |
| Phase 3 자판 JSON 내 자모 누락 | 매니저에 "안마태 자판 조사.md §2.1~2.3 기준 누락 자모 보강" 재호출 | 사용자에게 보고 후 보류 |
| Phase 4 UI ComboRow 자판 비표시 | ProfileRegistry 통합 검증 + locale yml 누락 키 점검 재호출 | 사용자에게 환경 매트릭스 동봉 보고 |
| Phase 5 회귀 테스트 실패 | 회귀 테스트 단일 케이스부터 격리·분석 | 즉시 직전 commit revert 후 사용자 보고 |

상충 데이터(예: 매니저 두 곳이 다른 영역 알고리즘 제시)는 삭제하지 않고 출처 병기, 메인이 합의 유도.

## 참조 파일

- `references/v3-schema-template.md` — Phase 1 산출물 표준 구조
- `references/symbol-candidates.md` — 고어 자모 위치별 한글 조판 기호 후보
- `references/composer-region-algorithm.md` — 영역 기반 음절 경계 알고리즘 의사 코드
- `references/test-matrix-template.md` — Phase 5 테스트 매트릭스 표준 형식
- `references/decision-log-template.md` — Phase 0 사용자 결정 기록 형식

## 테스트 시나리오

**정상 흐름**: 사용자가 "안마태 자판 추가" 요청 → 메인이 본 스킬 발동 → Phase 0 결정 5종 사용자 응답 → Phase 1~6 순차 위임 → 메인이 사용자에게 단일 응답 종합 보고 (변경 파일·테스트 결과·릴리스 노트 초안 경로)

**에러 흐름**: Phase 2에서 회귀 테스트 1개 실패 → 매니저 1차 재시도 → 재실패 → 메인이 사용자에게 실패 테스트 + 진단 보고 → 사용자 결정으로 Phase 2 수정 방향 합의 → 재진행
