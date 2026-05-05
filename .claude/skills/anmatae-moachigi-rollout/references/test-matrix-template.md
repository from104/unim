# 테스트 매트릭스 — 안마태/모아치기

> 본 파일은 Phase 5 산출물(`_workspace/anmatae/06_test_matrix.md`)의 표준 형식이다. 매니저는 본 매트릭스 전체를 채워 회귀 0과 신기능 정합성을 동시 검증한다.

## 검증 사다리

- L1: `cargo test -p unim`
- L2: `cargo test --workspace`
- L3: `make build` (zero-warning)
- L4: 샌드박스 실측 (`make sandbox-{gtk3,gtk4,qt5,qt6}` + 안마태/모아치기 키 입력)

## 단위 테스트 — schema 라운드트립

| ID | 입력 자판 | 기대 |
|----|---------|------|
| sv1-* | v1 자판 10종 | 모두 v1 path, 회귀 0 |
| sv2-* | v2 key_meta 자판 | v2 path, 회귀 0 |
| sv3-jamo | v3 layout_type=jamo | v2와 동일 composer |
| sv3-anmatae | layout_type=anmatae + moachigi 블록 | HangulComposerAnmatae |
| sv3-3bul-moachigi | layout_type=moachigi_3bul + inherits=ko_3bul390 | 3bul + moachigi_overrides |
| sv3-missing | anmatae인데 moachigi 누락 | LoadError::MissingMoachigiBlock |
| sv3-symbol | jamo_symbol_map 키와 layout 키 충돌 | symbol 우선 + 경고 |
| sv3-merge | 두 moachigi rule_set 동시 활성 | 나중 활성 우선 |

## 단위 테스트 — HangulComposerAnmatae

### A. 영역 채움 (region_filled=true)
- A1 `ㄱ ㅏ` → `가` (cho+jung 채움)
- A2 `ㄱ ㅏ ㅁ` → `감` (jong 추가)
- A3 `ㄱ ㅏ ㅂ ㅅ` → `갑` + `(jong 자리 ㅅ 단독 새 음절)` ※ 사용자 결정에 따라 변동
- A4 `ㅁ ㅏ ㄴ ㅁ` → `만` commit + `ㅁ_ _` 새 음절 (cho region 재진입)
- A5 `ㅏ ㅏ` → `아` commit + `_ㅏ_` (jung region 재진입)

### B. 종성 양방향 (jong_unordered=true)
- B1 `ㄹ ㄱ` → `ㄺ`
- B2 `ㄱ ㄹ` → `ㄺ` (양방향 핵심)
- B3 `ㄴ ㅈ` → `ㄵ`
- B4 `ㅈ ㄴ` → `ㄵ`
- B5 `ㄹ ㅁ` → `ㄻ`
- B6 `ㅁ ㄹ` → `ㄻ`
- B7 `ㅂ ㅅ` → `ㅄ`
- B8 `ㅅ ㅂ` → `ㅄ`

### C. 모아치기 토글 OFF→ON 회귀
- C1 jong_unordered=false 상태 `ㄱ ㄹ` → `ㄱ` commit + 새 cho `ㄹ`
- C2 jong_unordered=true 상태 동일 입력 → `ㄺ` 단일 종성
- C3 syllable_boundary=Strict + region_filled=true 충돌 → 명시적 우선순위 (Strict 우선)
- C4 rule_set 비활성 ↔ 활성 토글 시 composer 즉시 재구성 (런타임 동적 전환)

### D. jamo_symbol_map (즉시 commit)
- D1 'p' 키 입력 → `·` 즉시 output (composer 큐 무영향)
- D2 'p' 입력 직후 'ㄱ' → output `·ㄱ_` (큐 격리 검증)
- D3 jamo_symbol_map 미정의 키 → 일반 jamo path
- D4 shift+'p' → 다른 emit_char (upper 매핑)

## 단위 테스트 — 세벌식 + 모아치기

- 3M1 `ko_3bul_moachigi` 로딩 + active_rule_sets=[moachigi_jong_unordered] → 종성만 양방향
- 3M2 영역 간 순서는 세벌식 본래 동작과 동등 (회귀 0)
- 3M3 rule_set 비활성 → 기존 `ko_3bul390` 동작과 100% 동일

## 통합 테스트 — input_engine 키 입력 → preedit/commit

| ID | 자판 | 키 시퀀스 | 기대 preedit/commit |
|----|------|---------|----------------------|
| I-AM1 | anmatae | "ㄱ ㅏ ㅂ" | preedit `갑` |
| I-AM2 | anmatae | "ㄱ ㅏ ㅂ ㅅ" | commit `갑`, preedit `(사용자 결정)` |
| I-AM3 | anmatae | "ㄹ ㄱ" (단독) | preedit `ㄺ` (jong region) |
| I-AM4 | anmatae | "p" (jamo_symbol_map) | commit `·`, preedit 무 |
| I-AM5 | anmatae | "ㄱ ㅏ" + ESC reset | preedit clear |
| I-3M1 | moachigi_3bul | "ㄱ ㅏ ㄹ ㄱ" with jong_unordered | preedit `갉` |
| I-3M2 | moachigi_3bul | 동일 입력 with rule_set 비활성 | preedit `갈` + 새 cho `ㄱ` |

## 회귀 테스트

- R1 두벌식 `ko_2bulstd` 기존 테스트 전부 PASS
- R2 세벌식 390/391/noshift/qwerty 기존 테스트 전부 PASS
- R3 영문 자판 5종 무영향
- R4 한자/이모지 popup 무영향
- R5 AutoTypeFix 무영향

## 환경 매트릭스 (L4 샌드박스)

| 자판 | GTK3 | GTK4 | Qt5 | Qt6 | XIM | Wayland | GNOME |
|------|------|------|-----|-----|-----|---------|-------|
| anmatae | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ |
| moachigi_3bul | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ |

각 셀에서 최소 5음절 입력으로 preedit/commit 정상 확인. 실패 시 환경별 분기 검증 (project_popup_architecture.md 참고).

## 보고 형식

```markdown
## Test Matrix Report — anmatae-{ID}

### L1/L2 결과
- cargo test -p unim: PASS (N tests, 0 fail)
- cargo test --workspace: PASS

### L3 결과
- make build: PASS (warning 0)

### L4 환경 매트릭스
- (위 표 채움)

### 회귀
- R1~R5: PASS

### 신규 케이스
- 영역 채움 5/5, 종성 양방향 8/8, 토글 회귀 4/4, symbol 4/4

### 미해결
- (있다면 사용자 결정 필요 항목 추출)
```
