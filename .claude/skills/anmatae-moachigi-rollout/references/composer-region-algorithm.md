# HangulComposerAnmatae — 영역 기반 음절 경계 알고리즘

> 본 파일은 Phase 2 산출물(`_workspace/anmatae/02_composer_design.md`)의 알고리즘 명세 표준이다. 매니저는 본 의사 코드를 Rust로 직접 옮겨 구현한다. 임의 변형 금지.

## 출처

- `docs/references/research/안마태 자판 조사.md` §3 음절 경계 알고리즘
- `docs/references/research/쿼티형 세벌식 초안.md` §6 모아치기 고려

## 핵심 개념

안마태 자판은 **시간 기반 chord 미채택**(연구 §3.2). 대신 다음 두 메커니즘으로 음절 경계 결정:

1. **영역 채움**: cho/jung/jong 3영역. 한 영역이 채워진 상태에서 같은 영역 키가 다시 들어오면 force_compose → 새 음절 시작
2. **종성 영역 양방향 결합**: `(ㄹ, ㄱ)` 도 `(ㄱ, ㄹ)` 도 `ㄺ`으로 결합 (jong_unordered)

## 의사 코드 — add_jamo_with_region

```
fn add_jamo_with_region(jamo: JamoEnum, region: Region, spec: &MoachigiSpec):
    # region: Cho | Jung | Jong (안마태는 키별 region 고정. 자판 JSON에서 결정)

    # 1) 영역 채움 검사
    if spec.syllable_boundary == RegionFilled:
        if region == Cho and current.cho.is_some():
            force_compose()                    # 현재 음절 commit
            push_to_region(jamo, Cho)
            return
        if region == Jung and current.jung.is_some():
            # 복모음 결합 시도 (combinations.jung) 먼저
            if let Some(combined) = try_combine_jung(current.jung, jamo):
                current.jung = combined
                return
            else:
                force_compose()
                push_to_region(jamo, Jung)
                return
        if region == Jong:
            handle_jong_input(jamo, spec)
            return

    # 2) 영역 채움 OFF (strict 모드) — 기존 두벌식·세벌식과 동일 흐름
    delegate_to_legacy_composer(jamo)
```

## 의사 코드 — handle_jong_input (종성 양방향)

```
fn handle_jong_input(jamo: JamoEnum, spec: &MoachigiSpec):
    if current.jong.is_none():
        current.jong = jamo
        return

    # 종성 결합 시도
    let pair_a = (current.jong, jamo)
    if let Some(combined) = combinations.jong.get(&pair_a):
        current.jong = combined
        return

    if spec.jong_unordered:
        let pair_b = (jamo, current.jong)
        if let Some(combined) = combinations.jong.get(&pair_b):
            current.jong = combined
            return

    # 결합 실패 → 새 음절 시작 (현재 종성 commit, 새 종성 단독은 불가능 → 새 음절의 cho로 reinterpret? 안마태에서는 region이 Jong로 고정이므로 force_compose 후 jong 자리에 push)
    force_compose()
    current.jong = jamo
```

## 의사 코드 — force_compose

```
fn force_compose():
    if current is empty: return
    let syllable = compose_syllable(current.cho, current.jung, current.jong)
    output.push(syllable)
    current.clear()
```

`compose_syllable`은 기존 `BaseHangulComposer::compose_korean` 재사용. cho 또는 jung이 부재하면 부분 음절 emit (jamo 단독).

## 세벌식 부분 적용 (rule_set 기반)

세벌식 composer는 본래 자기 영역 알고리즘을 가지므로, `moachigi_overrides`가 적용되면 **종성 양방향 결합만 활성** (jong_unordered=true). 영역 간 순서 자유는 세벌식 본래 동작과 동등하므로 무영향.

```
fn HangulComposer3Bul::add_jamo_with_meta(jamo, meta):
    let effective_moachigi = profile.merged_moachigi();   # rule_set merge 결과
    if effective_moachigi.jong_unordered:
        # 종성 결합 시 양방향 시도
        ...
    delegate_to_existing_3bul_logic(jamo)
```

## 데이터 구조 — Region 추출

자판 JSON에서 키별 region 결정:
- `layout.lower["1st"][0]`이 자모 ㄱ이면 → key 'q'의 region = Cho
- jung jamo면 → Jung
- jong jamo면 → Jong
- combined jamo (ㅐ 등)면 → 첫 jamo의 region

`KeyToRegionMap: HashMap<KeyCode, Region>`을 LayoutProfile 빌드 시 사전 계산.

## 테스트 케이스 (각 영역별 5개 이상 권장)

### 영역 채움 (region_filled=true)
- 입력 `ㄱ ㅏ ㅂ ㅅ` → `갑ㅅ` (ㅂ 종성 채워진 상태에서 ㅅ 종성 입력 → force_compose 없이 결합 시도 → `(ㅂ, ㅅ)` 비결합이므로 force_compose → `갑` commit + `ㅅ` 새 음절 cho로 시작? 안마태 region 모드에서는 ㅅ이 jong region이면 `갑` commit + 새 음절 `_ _ ㅅ`. **사용자 결정 필요**)
- 입력 `ㅁ ㅏ ㄴ ㅁ` → `만` commit + 새 음절 `ㅁ_ _` (cho region 재진입)

### 종성 양방향 (jong_unordered=true)
- `ㄹ ㄱ` → `ㄺ`
- `ㄱ ㄹ` → `ㄺ` (양방향)
- `ㄴ ㅈ` → `ㄵ`
- `ㅈ ㄴ` → `ㄵ` (양방향)

### 모아치기 토글 OFF→ON 회귀
- 동일 입력 `ㄱ ㄹ`을 jong_unordered=false → `ㄱ` commit + 새 cho `ㄹ`. true → `ㄺ` 단일 종성. 두 모드 분기 정확성 검증.

## 사용자 결정 필요

- [ ] 영역 채움 모드에서 jong region 키가 비어있는 cho/jung 상태로 들어왔을 때의 동작 (단독 jong commit 허용? 아니면 빈 cho/jung으로 부분 음절?)
- [ ] 복모음 결합과 영역 채움의 우선순위 (현 의사 코드는 결합 우선)

## 안전 규칙

- `force_compose` 호출 후 큐 clear는 atomic. 중간 상태 노출 금지.
- 기존 두벌식·세벌식 composer 시그니처는 무변경. 새 trait method는 default impl로 위임만.
- `unim_log!()` 트레이스 권장 시점: force_compose 호출, 종성 양방향 매치, jamo_symbol_map emit.
