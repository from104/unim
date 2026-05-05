# 3bul_moachigi 컴포저 — 영역 기반 음절 경계 알고리즘

> 본 파일은 Phase 2 산출물(`_workspace/anmatae/02_composer_design.md`)의 알고리즘 명세 표준이다. 매니저는 본 의사 코드를 Rust로 직접 옮겨 구현한다. 임의 변형 금지.

## 출처

- `docs/references/research/안마태 자판 조사.md` §3 음절 경계 알고리즘
- `docs/references/research/쿼티형 세벌식 초안.md` §6 모아치기 고려

## 핵심 원칙

### 1. 한글 고어 완전 배제 (절대 규칙)

본 작업은 **현대 한글만** 다룬다. 한글 고어(옛한글) 자모는 **모든 단계에서 완전 배제**:

- 자판 JSON `layout` 필드에 옛한글 코드포인트 진입 금지:
  - U+1140~U+115F (Hangul Jamo 옛한글 영역)
  - U+11A8 이전 / U+11C3 이후 종성 옛한글
  - U+302E~U+302F (방점)
  - U+3165~U+318E (Hangul Compatibility Jamo 옛한글 영역, ㆍ U+318D 포함)
  - U+A960~U+A97F (Hangul Jamo Extended-A)
  - U+D7B0~U+D7FF (Hangul Jamo Extended-B)
- 옛한글 코드포인트 발견 시 loader가 `LoadError::ArchaicJamoNotSupported` 즉시 거부
- 안마태 원본의 옛한글 자리에는 `jamo_symbol_map`으로 **한글 조판 기호** 매핑 (사용자 승인 게이트)
- composer는 옛한글 자모를 처리할 책임 없음. 받지 않는다는 전제로 단순화

향후 v4 등에서 옛한글 별도 설계 예정. 본 v3 작업에서는 100% 배제.

### 2. 영역(Cho/Jung/Jong) 자동 분류

영역은 **JamoEnum 자체로 자명하다**. 별도 KeyToRegionMap이나 외부 region 주입 불필요:

```rust
match jamo {
    JamoEnum::Cho(_)  => Region::Cho,
    JamoEnum::Jung(_) => Region::Jung,
    JamoEnum::Jong(_) => Region::Jong,
}
```

키 → 자모 매핑은 `layout.lower["1st"]/["2nd"]/["3rd"]/["4th"]` (v1/v2와 동일 직관 구조)에서 결정. 컴포저는 자모만 받아 자기 책임으로 영역 분류.

### 3. 음절 경계 (시간 기반 chord 미채택)

- **영역 채움**: 한 영역이 채워진 상태에서 같은 영역 자모가 다시 들어오면 force_compose → 새 음절 시작
- **종성 영역 양방향 결합**: `(ㄹ, ㄱ)` 도 `(ㄱ, ㄹ)` 도 `ㄺ`으로 결합 (jong_unordered)

## 의사 코드 — add_jamo_with_meta (기존 시그니처 유지)

```rust
impl HangulComposer for HangulComposer3BulMoachigi {
    fn add_jamo_with_meta(&mut self, jamo: JamoEnum, meta: JamoMeta) -> Option<char> {
        let region = match jamo {
            JamoEnum::Cho(_)  => Region::Cho,
            JamoEnum::Jung(_) => Region::Jung,
            JamoEnum::Jong(_) => Region::Jong,
        };
        let spec = &self.moachigi_spec;

        // 1) 영역 채움 검사 (region_filled 모드)
        if spec.syllable_boundary == SyllableBoundary::RegionFilled {
            match region {
                Region::Cho if self.current.has_cho() => {
                    self.force_compose();
                    self.push(jamo);
                    return self.flush_one();
                }
                Region::Jung if self.current.has_jung() => {
                    // 복모음 결합 우선
                    if let Some(combined) = self.try_combine_jung(jamo) {
                        self.current.set_jung(combined);
                        return None;
                    }
                    self.force_compose();
                    self.push(jamo);
                    return self.flush_one();
                }
                Region::Jong => {
                    return self.handle_jong_input(jamo, spec);
                }
                _ => {}
            }
        }

        // 2) Strict 모드 또는 빈 영역 — 일반 push
        self.push(jamo);
        self.flush_one()
    }
}
```

**핵심**: 외부에서 region을 주입하지 않는다. composer가 받은 jamo의 enum variant로 자체 판단.

## 의사 코드 — handle_jong_input (종성 양방향)

```rust
fn handle_jong_input(&mut self, jamo: JamoEnum, spec: &MoachigiSpec) -> Option<char> {
    let JamoEnum::Jong(new_jong) = jamo else { unreachable!() };

    let Some(cur_jong) = self.current.jong() else {
        self.current.set_jong(new_jong);
        return None;
    };

    // 1) 정방향 결합 시도
    if let Some(combined) = self.combinations.jong.get(&(cur_jong, new_jong)) {
        self.current.set_jong(*combined);
        return None;
    }

    // 2) 양방향 결합 시도 (jong_unordered=true)
    if spec.jong_unordered {
        if let Some(combined) = self.combinations.jong.get(&(new_jong, cur_jong)) {
            self.current.set_jong(*combined);
            return None;
        }
    }

    // 3) 결합 실패 → force_compose 후 새 음절 jong 자리
    self.force_compose();
    self.current.set_jong(new_jong);
    self.flush_one()
}
```

## 의사 코드 — force_compose

```rust
fn force_compose(&mut self) {
    if self.current.is_empty() { return; }
    let syllable = self.current.compose_korean();   // BaseHangulComposer 재사용
    self.output_buffer.push(syllable);
    self.current.clear();
}
```

`compose_korean`은 기존 `BaseHangulComposer` 그대로. cho 또는 jung 부재 시 부분 음절 emit (jamo 단독).

## 세벌식 부분 적용 (rule_set 기반)

세벌식 composer는 본래 자기 영역 알고리즘 보유. `moachigi_overrides`가 적용되면 **종성 양방향 결합만 활성** (jong_unordered=true). 영역 간 순서 자유는 세벌식 본래 동작과 동등하므로 무영향.

```rust
impl HangulComposer for HangulComposer3Bul {
    fn add_jamo_with_meta(&mut self, jamo: JamoEnum, meta: JamoMeta) -> Option<char> {
        // jong_unordered가 활성이고 jamo가 종성이면 양방향 시도
        if self.effective_moachigi.jong_unordered {
            if let JamoEnum::Jong(new_jong) = jamo {
                if let Some(cur_jong) = self.current.jong() {
                    if let Some(combined) = self.combinations.jong
                        .get(&(new_jong, cur_jong))   // 역방향 우선 시도
                    {
                        self.current.set_jong(*combined);
                        return None;
                    }
                }
            }
        }
        // 기존 3bul 로직 그대로
        self.legacy_add_jamo(jamo)
    }
}
```

## 자판 JSON 구조 — v1/v2 직관 형식 유지

자판 JSON의 `layout` 필드는 v1/v2와 **동일한 4행 구조**로 표기:

```jsonc
{
  "schema_version": 3,
  "layout_type": "moachigi_3bul",
  "name": "ko_3bul_anmatae",
  "layout": {
    "lower": {
      "1st": ["q", "w", ..., "p"],     // 숫자행 (1행)
      "2nd": ["a", "s", ..., ";"],     // 윗글쇠행 (2행)
      "3rd": ["z", "x", ..., "/"],     // 아랫글쇠행 (3행)
      "4th": [...]                     // (안마태 4행 사용 시)
    },
    "upper": { /* shift = lower 동일 또는 별도 */ }
  },
  "jamo_symbol_map": {
    "B": { "emit_char": "“" }   // 큰따옴표 등 (사용자 승인된 6키만)
  }
}
```

키 → 자모 매핑은 `layout`에서 직접 읽는다. **`_region_map` 같은 별도 메타 필드 금지** — 자모 자체가 region을 갖는다.

## jamo_symbol_map 처리 경로 (composer 우회)

키 입력이 jamo_symbol_map에 정의되어 있으면:
1. composer 진입 안 함
2. `emit_char`를 즉시 commit
3. composer 큐는 영향 없음 (큐 격리)

이 분기는 **input_engine 진입 직후**에서 처리 — composer는 이 키를 보지 못함.

## 테스트 케이스 (각 영역별 5개 이상 권장)

### 영역 채움 (region_filled=true)
- A1 `ㄱ ㅏ` → preedit `가`
- A2 `ㄱ ㅏ ㅁ` → preedit `감`
- A3 `ㅁ ㅏ ㄴ ㅁ` → commit `만` + preedit `ㅁ_ _`
- A4 `ㅏ ㅏ` → commit `아` + preedit `_ㅏ_`
- A5 `ㄱ ㅏ ㅂ ㅅ` → `(ㅂ,ㅅ)` 비결합이면 commit `갑` + preedit `_ _ ㅅ`

### 종성 양방향 (jong_unordered=true)
- B1 `ㄹ ㄱ` → `ㄺ`
- B2 `ㄱ ㄹ` → `ㄺ` (양방향 핵심)
- B3 `ㄴ ㅈ` → `ㄵ`
- B4 `ㅈ ㄴ` → `ㄵ` (양방향)
- B5 `ㄹ ㅁ` → `ㄻ`
- B6 `ㅁ ㄹ` → `ㄻ` (양방향)
- B7 `ㅂ ㅅ` → `ㅄ`
- B8 `ㅅ ㅂ` → `ㅄ` (양방향)

### 모아치기 토글 OFF→ON 회귀
- C1 jong_unordered=false 상태 `ㄱ ㄹ` → `ㄱ` commit + 새 cho `ㄹ`
- C2 jong_unordered=true 상태 동일 입력 → `ㄺ` 단일 종성
- C3 syllable_boundary=Strict + region_filled=true 충돌 → 명시적 우선순위 (Strict 우선)
- C4 rule_set 비활성 ↔ 활성 토글 시 composer 즉시 재구성

### jamo_symbol_map (즉시 commit)
- D1 `B`(shift+b) 입력 → 즉시 commit `"`
- D2 `B` 직후 `ㄱ` → output `"ㄱ_` (큐 격리)
- D3 jamo_symbol_map 미정의 키 → 일반 jamo path
- D4 lower/upper 분리 매핑 정상

### 옛한글 거부 (LoadError)
- E1 자판 JSON에 U+318D(ㆍ) 진입 → `LoadError::ArchaicJamoNotSupported`
- E2 jong에 U+11C3 진입 → 거부
- E3 jamo_symbol_map의 emit_char가 옛한글이면 거부

## 안전 규칙

- `force_compose` 호출 후 큐 clear는 atomic. 중간 상태 노출 금지.
- 기존 두벌식·세벌식 composer trait 시그니처는 **무변경** (`add_jamo_with_meta` 그대로 사용). region 인자 추가 금지.
- 외부(input_engine·layout 빌더)는 region을 결정하지 않는다. composer 책임.
- `unim_log!()` 트레이스 권장 시점: force_compose 호출, 종성 양방향 매치, jamo_symbol_map 우회 commit.
- 옛한글 코드포인트는 loader 진입점에서 거부. composer는 받지 않는다 가정.
