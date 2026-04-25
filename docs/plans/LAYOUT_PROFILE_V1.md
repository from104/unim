# UNIM 자판 프로필 v1 스키마 초안

Date: 2026-04-21
Status: **초안 (Draft)** — 구현 전 리뷰 대상.
Owner: Core / Keystroke
Scope: 현재 `src/keystroke/keymap/*.json` 포맷을 v0로 간주하고, v1 스키마를 정의한다. v0 파일은 **변경 없이 그대로 로드**돼야 한다.

---

## 1. 배경

현재 UNIM의 자판 정의는 두 곳에 분리돼 있다.

| 분류 | 위치 | 특성 |
|------|------|------|
| 글쇠 → 낱자 사상 | `src/keystroke/keymap/*.json` | 빌드 시 `include_str!`로 임베드. 사용자 수정 경로 없음. |
| 낱자 결합 규칙 | `src/hangul/composer_with_2bul.rs`, `composer_with_3bul.rs` | Rust `const` 배열(`JUNG_COMBINATIONS` / `JONG_COMBINATIONS` / `CHO_COMBINATIONS`). `type: "2bul"` 여부로만 선택. |

결과적으로:
- 사용자 자판 공유 불가(재컴파일 필요).
- 두벌식 변종(콜맥-한글 등 키 사상만 다른 경우)조차 소스를 건드려야 함.
- 세벌식 옛날아래·신세벌식 2015 같이 **결합 규칙이 다른 자판**은 새 `ComposerType` 변형을 만들지 않는 한 표현 불가.
- 날개셋의 초·종성 공유 규칙(`docs/research/NALGAESET_KEYBOARD_FORMAT.md` §5.1)을 이식하려 해도 Rust `const` 분리 구조 때문에 직접 매핑이 어렵다.

v1 스키마는 위 세 가지를 해결하는 **데이터 주도 자판 프로필**을 도입하되, **오토마타 상태 머신은 건드리지 않는다**(날개셋 고급 입력 스키마 이식은 별도 로드맵으로 분리, 연구 문서 §7.3 비목표 참조).

## 2. 설계 원칙

1. **하위 호환 절대 유지**: v0 JSON 9종(en × 5, ko × 4) 중 **단 한 줄도 수정하지 않아도** 기존과 동일하게 로드된다. v1 판별은 신규 필드(`schema_version`, `metadata`, `inherits`, `combinations`) 존재 여부로 결정.
2. **선택적 확장**: v1의 추가 필드는 optional. 누락 시 v0와 동일 동작.
3. **프로필 자기 완결성**: 각 v1 프로필은 자신이 쓰는 **초·중·종성 기본 조합을 `combinations` 필드에 모두 명시**한다. `combinations` 필드가 있으면 그 프로필은 자기 완결 — Rust 기본 테이블을 참조하지 않는다. 개발자는 프로필 파일 한 장으로 자판 동작을 전부 파악할 수 있고, 사용자는 파일 하나만 공유하면 환경 의존 없이 자판이 재현된다. `combinations` 필드가 없으면 v0 호환 경로로 Rust 기본 테이블(`JUNG/JONG/CHO_COMBINATIONS`)을 상속한다 — 내장 9종이 v1로 이관되기 전의 과도기 동작.
4. **옵션 규칙은 `rule_sets`**: 같은 자판 안에서 on/off 가능한 보조 규칙(순아래받침 같은 프리셋)은 `rule_sets`로 분리. 자판 정체성을 이루는 기본 조합과 선택 프리셋을 스키마 수준에서 구분.
5. **상속은 평면적**: `inherits: "<profile_name>"` 한 단계. 다단계 상속·다중 상속은 v1에서 명시적 비목표(순환 탐지만 구현).
6. **스키마 버전 명시**: `schema_version: 1`. 2 이상이 나오면 로더는 경고 후 v0 fallback.

## 3. JSON 스키마 (v1)

### 3.1 전체 형태

```jsonc
{
  // ── v1 식별자 (optional — 없으면 v0로 해석) ──
  "schema_version": 1,

  // ── v0와 동일한 기본 필드 ──
  "language": "korean",
  "name": "3bul390",
  "type": "3bul",                // 기본 결합 규칙 선택 키 (2bul | 3bul)

  // ── v1 신규: 메타데이터 (optional) ──
  "metadata": {
    "display_name": "세벌식 390",
    "author": "공병우 · 한국표준과학연구원 (1990)",
    "version": "1.0.0",
    "license": "Public Domain",
    "description": "세벌식 최종 이전의 390 자판. 숫자열 대신 받침을 상단에 배치.",
    "source_url": "https://example.org/3bul390",
    "tags": ["3bul", "standard"]
  },

  // ── v1 신규: 상속 (optional) ──
  // 같은 디렉토리 또는 내장 프로필의 name을 참조.
  "inherits": "ko_3bul390",

  // ── v0와 동일: 키 매핑 (inherits 있을 때는 override 덮어쓰기) ──
  "layout": {
    "upper": { "1st": [...], "2nd": [...], "3nd": [...], "4th": [...] },
    "lower": { "1st": [...], "2nd": [...], "3nd": [...], "4th": [...] }
  },

  // ── v1 신규: 낱자 결합 규칙 (자판 자기 완결 명시) ──
  // 이 필드가 있으면 프로필이 쓰는 기본 조합을 **모두 나열**한다 — Rust const 상속 없음.
  // 필드가 아예 없으면 v0 호환 경로로 Rust 기본 테이블을 자동 상속(과도기 동작).
  // (예시는 3벌식 표준 규칙 전체; 실제 프로필은 자판별로 달라진다.)
  "combinations": {
    "jung": [
      { "first": "ㅗ", "second": "ㅏ", "result": "ㅘ" },
      { "first": "ㅗ", "second": "ㅐ", "result": "ㅙ" },
      { "first": "ㅗ", "second": "ㅣ", "result": "ㅚ" },
      { "first": "ㅜ", "second": "ㅓ", "result": "ㅝ" },
      { "first": "ㅜ", "second": "ㅔ", "result": "ㅞ" },
      { "first": "ㅜ", "second": "ㅣ", "result": "ㅟ" },
      { "first": "ㅡ", "second": "ㅣ", "result": "ㅢ" }
    ],
    "jong": [
      { "first": "ᆨ", "second": "ᆨ", "result": "ᆩ" },
      { "first": "ᆨ", "second": "ᆺ", "result": "ᆪ" },
      { "first": "ᆫ", "second": "ᆽ", "result": "ᆬ" },
      { "first": "ᆫ", "second": "ᇂ", "result": "ᆭ" },
      { "first": "ᆯ", "second": "ᆨ", "result": "ᆰ" },
      { "first": "ᆯ", "second": "ᆷ", "result": "ᆱ" },
      { "first": "ᆯ", "second": "ᆸ", "result": "ᆲ" },
      { "first": "ᆯ", "second": "ᆺ", "result": "ᆳ" },
      { "first": "ᆯ", "second": "ᇀ", "result": "ᆴ" },
      { "first": "ᆯ", "second": "ᇁ", "result": "ᆵ" },
      { "first": "ᆯ", "second": "ᇂ", "result": "ᆶ" },
      { "first": "ᆸ", "second": "ᆺ", "result": "ᆹ" },
      { "first": "ᆺ", "second": "ᆺ", "result": "ᆻ" }
    ],
    "cho": [
      { "first": "ㄱ", "second": "ㄱ", "result": "ㄲ" },
      { "first": "ㄷ", "second": "ㄷ", "result": "ㄸ" },
      { "first": "ㅂ", "second": "ㅂ", "result": "ㅃ" },
      { "first": "ㅅ", "second": "ㅅ", "result": "ㅆ" },
      { "first": "ㅈ", "second": "ㅈ", "result": "ㅉ" }
    ]
  },

  // ── v1 신규: 이름 있는 규칙 세트 (optional, 각각 on/off 토글 가능) ──
  // 대표 예: 순아래받침 (docs/research/순아래받침_규칙.md).
  // 규칙 세트는 기본 combinations 위에 추가로 얹히며, active=false면 불러오지 않음.
  // 모든 엔트리는 (first, second, result) pair combination 하나로 통일.
  // "재해석"도 first가 이미 조합된 낱자(예: ᆶ)인 pair로 자연 표현된다.
  // scope는 자모 코드포인트로 자동 판별 (jong은 U+11xx, cho/jung은 호환/조합형).
  "rule_sets": {
    "sun_arae_batchim": {
      "active": true,
      "description": "세벌식 390 Shift 없는 받침 입력",
      "combinations": [
        { "first": "ᆫ", "second": "ᆫ", "result": "ᆮ" },   // 가획
        { "first": "ᆫ", "second": "ᆺ", "result": "ᆽ" },
        { "first": "ᆨ", "second": "ᇂ", "result": "ᆿ" },   // 격음화
        { "first": "ᆸ", "second": "ᇂ", "result": "ᇁ" },
        { "first": "ᆮ", "second": "ᇂ", "result": "ᇀ" },
        { "first": "ᆽ", "second": "ᇂ", "result": "ᆾ" },
        { "first": "ᆭ", "second": "ᆺ", "result": "ᆬ" },   // ㅎ 경유 재해석 (pair로 자연 표현)
        { "first": "ᆶ", "second": "ᆫ", "result": "ᆴ" },
        { "first": "ᆶ", "second": "ᆸ", "result": "ᆵ" }
      ]
    }
  },

  // ── v1 신규: 프로필이 기본 활성화할 규칙 세트 이름 목록 (optional) ──
  // 비어 있으면 rule_sets.*.active 값(개별 선언)을 그대로 따른다.
  // 지정 시 이 목록에 있는 것만 on, 나머지는 강제 off. (사용자 override 지점)
  "active_rule_sets": ["sun_arae_batchim"]
}
```

### 3.2 필드 상세

| 필드 | 타입 | 필수 | 기본값 | 비고 |
|------|------|------|--------|------|
| `schema_version` | `u8` | ❌ | `0` (필드 없음) | v1 기능 사용 시 `1` 필수. |
| `language` | `string` | ✅ | — | v0와 동일. `"korean"` / `"english"`. |
| `name` | `string` | ✅ | — | 프로필 식별자. 파일명과 일치 권장. |
| `type` | `string` | ✅ | — | `"2bul"` / `"3bul"`. 기본 결합 규칙 선택 + 컴포저 선택. |
| `metadata` | `object` | ❌ | `{}` | 로딩 동작에 영향 없음. GUI 표시에 사용. |
| `inherits` | `string` | ❌ | `null` | 다른 프로필 `name` 참조. 해석 실패 시 경고 + 무시. |
| `layout` | `object` | ❌* | — | `inherits`가 있으면 optional(부분 override). 없으면 필수. |
| `combinations` | `object` | ❌ | `null` | 있으면 자기 완결 — 아래 세 배열이 자판의 조합 규칙 전부. 없으면 v0 호환 경로로 Rust 기본 테이블 상속. |
| `combinations.jung` | `array` | ❌ | 빈 배열 | 자판이 사용하는 중성 조합 전체. |
| `combinations.jong` | `array` | ❌ | 빈 배열 | 종성 조합 전체. |
| `combinations.cho` | `array` | ❌ | 빈 배열 | 초성 조합 전체(경음 등). |
| `rule_sets` | `object` | ❌ | `{}` | 이름 있는 규칙 묶음. §3.5 참조. |
| `active_rule_sets` | `array<string>` | ❌ | `null` | 지정 시 이 이름만 on, 나머지는 off. 미지정 시 `rule_sets.*.active` 값 그대로. |

### 3.3 결합 규칙 entry 포맷

각 항목은 **객체** 또는 **3-튜플 배열**을 허용(작성 편의):

```jsonc
// 객체 형태 — 권장
{ "first": "ㄹ", "second": "ㄱ", "result": "ㄺ" }

// 배열 형태 — 짧게 쓰고 싶을 때
["ㄹ", "ㄱ", "ㄺ"]
```

**문자 표현 규약 (영역은 자모 코드포인트로 자동 판별)**:
- **초성**: 호환 자모 영역(U+3131–U+314E 중 자음) 또는 초성 조합형(U+1100–U+1112).
- **중성**: 호환 자모 영역(U+314F–U+3163) 또는 중성 조합형(U+1161–U+1175).
- **종성**: **종성 조합형(U+11A8–U+11C2) 전용** — 호환 자모 불허. `combinations.jong`의 `first`·`second`·`result`, `rule_sets.*.combinations`의 종성 엔트리 모두 U+11xx로만 표기.
- 엔트리의 `first` 코드포인트를 보고 로더가 cho/jung/jong 중 어느 결합 테이블에 넣을지 자동 결정. 프로필에 별도 `scope` 필드 불필요.
- 변환 실패 entry는 **경고 로그 + 무시**(전체 프로필 로드는 계속).
- 한 `rule_set` 내에서는 하나의 영역(all cho 또는 all jung 또는 all jong)으로 통일되어야 한다 — 영역이 섞이면 해당 엔트리 skip + 경고.

### 3.4 v0 ↔ v1 판별 규칙

로더는 아래 순서로 판별한다:

1. `schema_version` 필드 존재 → v1.
2. `metadata` / `inherits` / `combinations` / `rule_sets` / `active_rule_sets` 중 하나라도 존재 → v1.
3. 그 외 → v0. 현재 v0 deserializer 그대로 사용.

v1로 판별된 파일은 v1 deserializer가 추가 필드를 처리한 뒤 내부 표현(`LayoutProfile`)으로 흡수. v0 파일은 `LayoutProfile::from_v0(KeyboardMapJson)` 경로로 자동 승격된다.

### 3.5 규칙 세트 (Rule Sets)

`combinations`가 "기본 테이블"이라면, `rule_sets`는 **프로필 안에서 그룹 단위로 on/off 가능한 보조 규칙 묶음**이다. 대표 예가 순아래받침 규칙(`docs/research/순아래받침_규칙.md`)으로, 이를 규칙 세트 하나로 캡슐화하면 세벌식 390을 기반으로 "받침 영역만 Shift 없이" 쓰고 싶은 사용자가 한 플래그로 전체를 켜고 끌 수 있다.

#### 3.5.1 규칙 세트 정의

```jsonc
"rule_sets": {
  "<set_name>": {
    "active": true,            // 기본 활성 여부
    "description": "...",       // GUI 설명용 (optional)

    // pair combinations 목록. 각 엔트리의 first 자모 코드포인트로
    // cho/jung/jong 영역이 자동 판별된다(§3.3). 한 세트 내 엔트리는
    // 하나의 영역으로 통일되어야 한다.
    "combinations": [
      { "first": "ᆫ", "second": "ᆫ", "result": "ᆮ" },
      { "first": "ᆶ", "second": "ᆫ", "result": "ᆴ" }
    ]
  }
}
```

#### 3.5.2 pair combinations로 모든 규칙 표현

v1은 규칙 유형을 **pair combinations 하나로 단일화**한다. `(first, second) → result` 형태로:

- **기본 겹받침**: `(ᆯ, ᆨ) → ᆰ` — 첫 자모가 기본 낱자.
- **가획·격음화**: `(ᆫ, ᆫ) → ᆮ`, `(ᆨ, ᇂ) → ᆿ` — 훈민정음식 결합.
- **"재해석"**: `(ᆶ, ᆫ) → ᆴ` — 첫 자모가 **이미 조합된 겹받침**. 컴포저가 현재 상태의 낱자(`ᆶ`)와 다음 입력(`ᆫ`)을 pair로 look up하면 자연스럽게 적용된다.

이전 초안에서 별도 타입으로 다뤘던 `reinterpret`은 실상 "first가 composed jamo인 pair combination"일 뿐이므로 `combinations`로 흡수된다. 구현 측에서도 `HashMap<(Jamo, Jamo), Jamo>` 하나만 유지하면 된다.

#### 3.5.3 체인 3타도 pair combinations로 자연 동작

`ㄴ + ㄴ + ㅎ → ㅌ` 같은 3타 체인은 별도 규칙 없이 **pair 2회 적용**으로 풀린다.

1. 첫 ㄴ → 상태 `ᆫ`
2. 둘째 ㄴ → `(ᆫ, ᆫ) → ᆮ` 적용 → 상태 `ᆮ`
3. ㅎ → `(ᆮ, ᇂ) → ᇀ` 적용 → 상태 `ᇀ`

3타·4타 체인 모두 매 입력마다 `(현재 상태, 새 입력) → 다음 상태` 한 번씩 look up하면 되므로 상태 머신이 단순하다.

#### 3.5.4 `active_rule_sets`와 사용자 override

프로필 최상위의 `active_rule_sets` 배열은 "이 프로필이 기본으로 켜두는" 세트 이름의 집합.

| 상태 | 의미 |
|------|------|
| 필드 없음 (`null`) | 각 `rule_sets.<name>.active` 값을 그대로 사용. |
| 빈 배열 `[]` | 모든 세트 강제 off. |
| 이름 나열 | 나열된 세트만 on, 그 외는 off. |

사용자 설정(`~/.config/unim/config.yaml`)에서 `korean_active_rule_sets: ["sun_arae_batchim"]` 같은 필드를 두고 프로필 값을 override할 수 있도록 길을 열어둔다(실제 config 필드 추가는 구현 PR에서).

#### 3.5.5 상속과의 상호작용

- `inherits`로 base 프로필의 `rule_sets`를 모두 가져온다.
- child 프로필의 `rule_sets.<name>`은 같은 이름이면 **항목 단위 덮어쓰기**(`active`/`combinations` 개별 교체).
- child의 `active_rule_sets`는 base의 같은 필드를 **완전 교체**(추가가 아님). base 값에 추가하고 싶으면 `null` 두고 개별 세트의 `active`만 바꾼다.

#### 3.5.6 검증 규칙

1. `combinations` entry의 `first`/`second`/`result` 자모가 해석 불가(§3.3의 코드포인트 범위 밖) → 해당 entry skip + 경고.
2. 한 rule_set 내에서 엔트리들의 영역이 섞여 있으면(예: 일부는 cho jamo, 일부는 jong jamo) → 영역 불일치 엔트리 skip + 경고.
3. `active_rule_sets`에 정의되지 않은 이름이 있으면 → 경고 + 무시.
4. 이름 충돌(같은 `<set_name>`이 inherits 체인과 자기 자신에 모두 있으면 덮어쓰기로 처리, 경고 없음).

### 3.6 v0 ↔ v1 판별 규칙 (재언급)

§3.4 참조. `rule_sets` / `active_rule_sets`도 v1 신호 필드다.

## 4. 상속(inherits) 해석 알고리즘

```
resolve(profile):
    if profile.inherits is None:
        return profile
    base = find_profile(profile.inherits)  # 내장 4종 또는 ~/.config/unim/layouts/*.json
    if base is None:
        warn("inherits target not found: {}", profile.inherits)
        return profile (sans inherits)
    if cycle_detected(profile, base):
        warn("inheritance cycle at {} → {}", profile.name, base.name)
        return profile (sans inherits)
    resolved_base = resolve(base)                # 재귀 해석
    return merge(resolved_base, profile)
```

### 4.1 병합 규칙 (`merge(base, child)`)

| 영역 | 동작 |
|------|------|
| `language` | child 우선, 다르면 경고 후 child 채택. |
| `type` | child 우선, 다르면 경고(결합 규칙 충돌 주의). |
| `metadata` | 얕은 병합. child 키가 base를 덮어씀. |
| `layout.upper.*`, `layout.lower.*` | **행 단위 덮어쓰기**. child가 지정한 행만 교체. 지정 안 한 행은 base 유지. |
| `combinations` | child에 `combinations` 필드가 있으면 **child 배열이 전체 조합 규칙**(자기 완결). base의 `combinations`는 `inherits`로 상속되지 않는다. child에 필드가 없으면 base의 `combinations`를 그대로 상속하고, base에도 없으면 Rust 기본 테이블 상속(v0 호환). |

### 4.2 순환 탐지

`resolve` 진입 시 현재 해석 체인을 `Vec<&str>`으로 추적. 중복 이름 등장 시 경고 + 해당 지점에서 체인 종료(더 이상 inherits 따라가지 않음).

### 4.3 해석 범위

- 내장 프로필 9종(`ko_*`, `en_*`)과 `~/.config/unim/layouts/*.json` 사용자 디렉토리의 프로필을 **하나의 네임스페이스**로 간주.
- 같은 `name`이 겹치면 **사용자 디렉토리 우선**(override 가능).
- `inherits`는 같은 네임스페이스에서만 찾는다.

## 5. 로딩 파이프라인 변경

### 5.1 현재 (v0 전용)

```
get_keymap_json(name) → &'static str
  → KeyboardMap::create_keyboard_map_from_str(en_json, ko_json, is_three_bul)
  → HashMap<char, JamoEnum>
```

### 5.2 제안 (v0+v1 겸용)

```
LayoutProfileLoader::load(name)
  → LayoutProfile (v0 또는 v1, 내부 표현은 동일 구조체)
     └ inherits 해석 완료
     └ combinations 병합 완료 (Rust const + JSON entry)
  → LayoutProfile::build_keyboard_map() → HashMap<char, JamoEnum>
  → LayoutProfile::build_jamo_combination_map() → CombinedJamoMap
```

`HangulComposer{2,3}Bul::new()` 서명은 그대로 두되, 내부 `COMBINED_JAMO_{2,3}BUL` static을 `LayoutProfile`이 제공하는 맵으로 **주입 가능**하게 확장한다. `new()`는 기본 static을 계속 쓰고, `new_with_profile(&LayoutProfile)` 신설.

### 5.3 `LayoutProfile` 내부 표현 (Rust 측 초안)

```rust
pub struct LayoutProfile {
    pub schema_version: u8,           // 0 또는 1
    pub language: Language,
    pub name: String,
    pub layout_type: LayoutType,      // TwoBul / ThreeBul / EnglishVariant
    pub metadata: LayoutMetadata,     // 비어 있을 수 있음
    pub layout: KeyLayout,            // 기존 v0 layout
    pub combinations: CombinationSet, // 병합 완료 상태
    pub rule_sets: Vec<RuleSet>,      // active=true인 것만 들어옴 (§3.5)
}

pub struct CombinationSet {
    pub jung: Vec<(Jung, Jung, Jung)>,
    pub jong: Vec<(Jong, Jong, Jong)>,
    pub cho:  Vec<(Cho, Cho, Cho)>,
    pub share_cho_jong: bool,
}

pub struct RuleSet {
    pub name: String,
    pub combinations: Vec<JamoTriple>,// (first, second, result) — 영역은 자모 enum 종류로 자동 판별
}
```

`rule_sets`는 이미 active 여부가 해소된(= 활성 집합만 남은) 상태로 컴포저에 전달된다. 컴포저는 매 입력마다 `(현재 상태 마지막 자모, 새 입력) → 새 상태` 형태로 기본 `combinations` + 활성 rule_sets의 `combinations`를 **동일한 pair look-up 경로**로 조회한다. 별도의 reinterpret 훅은 필요 없다.

## 6. 기존 Rust const와의 관계

`JUNG_COMBINATIONS` / `JONG_COMBINATIONS` / `CHO_COMBINATIONS`는 **유지하되 역할이 좁아진다**.

- **`combinations` 필드 없는 프로필**(v0 키맵 및 v0에서 자동 승격된 프로필): Rust 기본 테이블 그대로 적용 — **현재 동작 불변**. 과도기 호환 경로.
- **`combinations` 필드 있는 프로필**(v1 기본 형태): Rust const는 참조하지 않는다. 프로필 파일 하나만으로 자판 동작이 결정되어 공유·리뷰·디버깅이 단순해진다.

**자판 자기 완결성의 결과**:
- 개발자 시점: 새 자판 기여자는 Rust 소스를 읽지 않고도 프로필 JSON만 보고 전체 동작을 이해할 수 있다.
- 사용자 시점: 자판 파일 한 장만 주고받으면 UNIM 버전이나 내부 const 변화와 무관하게 동일하게 동작한다.
- 유지보수 시점: Rust const 변경이 기본 프로필의 의미를 바꾸지 않는다(프로필이 자기 값을 명시하기 때문).

**내장 9종 이관 방침**: 내장 프로필이 v1로 이관될 때 `combinations`를 **모두 명시**한 자기 완결 형태로 변환한다(§8.1 참조). 그 시점 이후에도 Rust const는 v0 호환과 테스트 픽스처용으로 남는다 — 장기적으로는 §11의 "코드 개편 시 삭제" 항목에 따라 제거.

## 7. 검증(validation) 규칙

로더는 아래를 검사하고 **발견된 문제는 경고 후 해당 항목만 제외**한다(프로필 전체 거부는 스키마 자체가 JSON 파싱 실패한 경우뿐).

1. `type`이 `"2bul"` / `"3bul"` 외 → 경고 + `"2bul"`로 간주.
2. `combinations.*` entry의 자모 문자가 변환 불가 → entry 단위 skip + 경고.
3. `inherits` 순환 → 4.2에 따라 끊음.
4. `inherits` 대상 부재 → 경고 + inherits 무시.
5. `layout` 행에 `null` 또는 길이 불일치 → v0 로더와 동일하게 처리(기존 동작 유지).
6. `rule_sets` 개별 항목 검증은 §3.5.6 참조. `active_rule_sets`에 없는 이름 → 경고 + 무시.

## 8. 마이그레이션

### 8.1 내장 9종 자판

- 현 단계: 수정 없음(v0). 모두 그대로 임베드된 상태로 유지.
- 이관 단계: 각 파일에 `schema_version: 1` + `metadata` 추가. 동시에 **자판 고유의 기본 조합을 `combinations`에 모두 명시**한 자기 완결 프로필로 변환 (§2 원칙 3, §6).
  - 한국어 4종: 현재 Rust `JUNG_COMBINATIONS` / `JONG_COMBINATIONS` / `CHO_COMBINATIONS`에 있는 항목 중 해당 자판이 실제로 쓰는 것을 선별해 프로필 `combinations.jung/jong/cho`에 선언. (2벌식은 CHO 규칙 없음, 3벌식은 CHO 경음 5종 포함 등 자판별로 다름.)
  - 영어 5종: 결합 규칙 자체가 없으므로 `combinations`는 빈 객체 또는 생략. 메타데이터만 추가.
- 완료 후: 내장 프로필은 전부 `combinations`를 명시한 자기 완결 상태. Rust const는 v0 호환과 테스트 픽스처 용도로만 남는다.
- 드래프트 파일은 `docs/plans/new_keymaps/*.json`에 준비되어 있으며, 본 원칙(자기 완결성)에 맞춰 순차적으로 정비된다.

### 8.2 사용자 디렉토리

- `~/.config/unim/layouts/*.json` 스캔 경로 신설.
- 파일명 규약: `<name>.json`. 내부 `name` 필드와 불일치 시 경고.
- 네임스페이스 충돌 시 사용자 파일이 내장을 덮어쓴다(§4.3).

### 8.3 설정 연동

- `src/config.rs`의 `KoreanLayout` 열거형은 유지(내장 4종 보호).
- 사용자 프로필 선택은 **별도 문자열 필드** `korean_custom_layout: Option<String>`로 표현. 설정되면 `KoreanLayout::Custom(name)` 유사 경로로 처리.
- GUI/CLI 노출은 본 문서 범위 밖(설정 3지점 싱크는 구현 PR에서).

## 9. 비목표 (명시적 제외)

v1이 커버하지 **않는** 것. 아래 항목들은 **엔진 재설계 과제**로, UNIM 로드맵의 별도 단계에서 다룬다(ROADMAP 6단계).

- **낱자 provenance 태깅** — 같은 ㅗ/ㅜ라도 "어느 키에서 왔는지" 구별해야 하는 설계. 세벌식 390의 `9`·`/` 이중모음 첫 모음 전용 역할(연구 문서 §4.1 날개셋문자 64-bit 토큰 개념)이 여기 필요하다. `src/hangul/jamo.rs`의 `Cho`/`Jung`/`Jong` enum 전면 개편 필요.
- **문맥 의존 키 해석** — `/`가 초성 뒤에서만 ㅗ로 동작하는 적응형 글쇠 (연구 문서 §4.2 글쇠 수식). static `KeyboardMap`을 넘어 컴포저 상태를 읽는 predicate 엔진이 필요.
- **모아치기 (안마태 자판 등)** — 낱자 입력 순서가 바뀌어도 재배열해 조합하는 stroke-replay.
- **복벌식** — 좌손·우손 기반 두벌식/세벌식 자동 전환 (연구 문서 §5.2).
- **옛한글** — 방점·합용병서·U+1100 확장 블록 낱자 처리.
- **고급 입력 스키마** — 글쇠 수식, 상태별 분기, 오토마타 데이터화 전체.
- **날개셋 `.ist`(바이너리/XML) 파싱** — 연구 문서 §7.3에 따라 별도 수입기(`unim-import-nalgaeset`)의 몫.
- **한자·상용구 번들** (`.hst` 대응).
- `rule_sets`의 **임의 깊이 자동 추론**(예: 여러 단계 reinterpret 연쇄, 조건부 규칙, scope 간 교차). v1은 scope 내부의 pair + single-step reinterpret까지만 허용한다.

**결과**: 세벌식 390 원본 규약의 일부는 v1에서 재현되지 않는다. 구체적으로 `9`-ㅜ/`/`-ㅗ가 이중모음 첫 모음이 아닐 때도 커밋되고, `/`는 초성 없이 눌러도 ㅗ로 나간다. 원본 충실 재현은 엔진 재설계 이후로 연기.

## 10. 구현 순서 (참고용)

1. `LayoutProfile` 구조체 + v0 → v1 자동 승격.
2. v1 deserializer(serde) + `schema_version` 판별.
3. `LayoutProfileLoader` + 내장 9종을 `HashMap<&'static str, &'static str>`로 재구성.
4. `inherits` 해석(순환 탐지 포함).
5. `combinations` 병합 + `HangulComposer{2,3}Bul::new_with_profile`.
6. 사용자 디렉토리 스캔(`~/.config/unim/layouts/`).
7. 통합 테스트: v0 9종이 변경 없이 로드되고, `make test` 100% 통과.
8. 내장 9종에 `schema_version`/`metadata` 부여(별도 PR, 선택).

## 11. 열린 질문

- `combinations.jung` entry의 자모를 **호환 자모로 쓸지 조합형으로 쓸지 한 쪽으로 강제**할지, 현재 제안처럼 양쪽 허용할지. 허용 시 비교·중복 판정 기준을 명확히 해야 한다.
- `inherits`가 다른 `type`을 가질 때 허용할지(예: `ko_2bulstd`를 base로 두는 세벌식 변종). 현재 제안은 "경고만 하고 child 채택" — 실용성 검증 필요.
- 내장 9종도 함께 파일 기반 로딩으로 옮길지, 아니면 `include_str!`는 유지하고 사용자 디렉토리만 추가 스캔할지. 후자가 부팅 속도·바이너리 자립성 면에서 유리.
- Rust const의 **장기 거취** — 결정: **코드 개편 시 삭제**(아직은 아님). 내장 9종이 v1 자기 완결 프로필로 모두 이관되고 v0 경로 제거가 공지된 시점에 `JUNG/JONG/CHO_COMBINATIONS` const를 제거. 이관 완료 전까지는 v0 호환과 `extend_default: true` 옵트인 경로의 base 테이블로 계속 남는다.
- `rule_sets.<name>.combinations`가 기본 `combinations.<scope>`와 동일 엔트리를 중복 선언할 때. 제안: **rule set이 덮어쓴다**(같은 키면 rule set의 result가 우선). 이 경우 rule set을 껐을 때 기본 테이블 값으로 자연스레 복원되어 기대 동작과 일치.
- `active_rule_sets`를 사용자 설정으로 override할 때의 UI. 규칙 세트마다 GUI 토글을 제공할지(세트가 많아지면 번잡), 프로필 단에서 하나의 프리셋으로 관리할지. 당장은 config 필드만 두고 GUI는 후속.
- pair combinations의 `first`를 **composed 낱자 한 자**(예: `ᆶ`)로만 제한할지, **낱자 시퀀스**(예: `ᆯ+ᇂ`)도 허용할지. 현재는 전자 — composed jamo는 이미 enum 한 값(`Jong::RieulHieuh`)으로 표현되므로 pair look-up이 자연스럽다.

## 12. 참고

- 연구 문서: `docs/research/NALGAESET_KEYBOARD_FORMAT.md`, `docs/research/순아래받침_규칙.md`
- 현재 v0 포맷: `src/keystroke/keymap/*.json`
- 결합 규칙 원본: `src/hangul/composer_with_2bul.rs:19-44`, `src/hangul/composer_with_3bul.rs:19-53`
- 로더: `src/keystroke/mod.rs:7-30`
- Jamo 표현: `src/hangul/jamo.rs`
