# UNIM 자판 프로필 v2 스키마 (key_meta 확장)

> **v1 베이스**: [LAYOUT_PROFILE_V1.md](LAYOUT_PROFILE_V1.md). 본 문서는 v1 위에 v2가 추가한 분만 자세히 다룬다. v1의 `combinations`/`rule_sets`/`active_rule_sets`/`inherits` 의미는 v1 문서 그대로.

Date: 2026-05-02 (Phase: 0.2.0 Rule A·B 완료 시점)
구현 범위: schema·builder·composer·input_engine·press_key·내장 자판 ko_3bul390/391
관련 PR: PR-A(schema dangling, develop 머지) → PR-B1(룰 B, develop 머지) → PR-B2/v2 확장(본 문서, develop 머지)

---

## 0. 한 눈에 — v1 대비 무엇이 바뀌었나

| 항목 | v1 (0.1.x ~ 초기 0.2.0) | v2 (0.2.0+) |
|---|---|---|
| `schema_version` 값 | `1` (또는 부재 시 1로 정규화) | `1` 또는 `2` |
| 키 단위 메타데이터 | 없음 | `key_meta` (top-level + `rule_sets[name].key_meta`) |
| 낱자 provenance 태깅 | 비목표(§9) | **구현**: `vowel_combine_head` |
| 문맥 의존 키 해석 | 비목표(§9) | **구현**: `context_alt` (9개 `ContextCondition`) |
| `RuleSet` 필드 | `active`/`description`/`combinations`/`reinterpret`/`scope` | + `key_meta` (rule_set이 토글하는 키 메타) |
| 세벌식 390/391 원본 충실 재현 | 보류 | **달성**: v/b 키 ㅗ·ㅜ 비결합 + `/` 키 초성-only 분기 |
| 두벌식 호환 | 동일 | 동일 (`vowel_combine_head` 누락 시 `true` 기본 → 모든 ㅗ/ㅜ 결합 가능) |

v1 문서 §9 (비목표)의 첫 두 항목 — *낱자 provenance 태깅*과 *문맥 의존 키 해석* — 이 v2에서 모두 데이터화됐다. 엔진 재설계 없이 schema 확장 + composer 큐 평행 메타로 해결.

---

## 1. `schema_version: 2` 게이트

### 1.1 v1 → v2 판별

JSON에 `key_meta`가 (top-level 또는 `rule_sets[name].key_meta` 중 하나라도) 존재하면 v2 의도. 단 `schema_version` 필드는 호환을 위해 **암묵 1**도 허용 — 즉 `key_meta`만 있고 `schema_version` 부재여도 파싱은 성공한다 (필요 시 자판 작성자가 `2` 명시).

빌트인 ko_3bul390/391은 명시적으로:
```json
{
  "schema_version": 2,
  "language": "korean",
  "name": "3bul390",
  ...
}
```

### 1.2 RawProfile 마커

`has_v1_markers()`가 `key_meta.is_some()`도 v1 마커로 인정하므로, v0 → v1/v2 게이트(`LoadError::UnsupportedSchema`)는 v2에도 그대로 적용된다.

코드: [schema.rs:217-225](../../../src/keystroke/profile/schema.rs#L217-L225)

---

## 2. `key_meta` 최상위 (top-level)

### 2.1 JSON 형태

```json
{
  "schema_version": 2,
  ...
  "key_meta": {
    "<key>": {
      "vowel_combine_head": <bool>,
      "context_alt": { ... }
    },
    ...
  }
}
```

- `<key>`: layout 셀과 동일 컨벤션의 **단일 문자**. 영어 자판 char(`"v"`, `"/"`, `"9"`) 또는 한글 자모(`"ᆮ"`). 다중 문자 키는 빌더가 무시.
- 값은 [`KeyMeta`](#22-keymeta-구조체) 객체.

### 2.2 KeyMeta 구조체

```rust
pub struct KeyMeta {
    pub vowel_combine_head: Option<bool>,   // 룰 A
    pub context_alt: Option<ContextAlt>,    // 룰 B
}
```

두 필드 모두 optional. 누락 시:
- `vowel_combine_head: None` → 결합 가능(`true`)으로 해석. 두벌식 호환.
- `context_alt: None` → 해당 키에 분기 없음 (literal 자모 매핑 그대로).

코드: [schema.rs:70-77](../../../src/keystroke/profile/schema.rs#L70-L77)

### 2.3 토글 불가 (rule_set과 차이)

top-level `key_meta`는 자판이 **항상 적용해야 하는** 키 본질 메타. 사용자가 켜고 끌 일이 없는 자판 정체성 일부. 끌 수 있게 하려면 rule_set으로 옮길 것 (§4).

---

## 3. 룰 A — `vowel_combine_head` (이중모음 결합 키 제한)

### 3.1 개념

같은 ㅗ가 자판의 여러 키에 매핑될 때, "이 키에서 들어온 ㅗ는 후속 ㅏ/ㅐ/ㅣ와 합쳐 ㅘ/ㅙ/ㅚ가 되는가?"를 키 단위로 판정하는 boolean.

세벌식 390 lower row4 슬롯 매핑:
- 슬롯 4 (`v` 키) → ㅗ
- 슬롯 5 (`b` 키) → ㅜ
- 슬롯 10 (`/` 키) → ㅗ (룰 B와 협력, §5.1)

원본 공병우 규약은 v/b의 ㅗ·ㅜ를 **단순 모음 전용**으로 두고, 이중모음 결합은 lower 1st 슬롯 10 (`9` 키 ㅜ)과 lower 4th 슬롯 10 (`/` 키 ㅗ)에서만 허용한다. 이 차이를 v1 시점엔 표현하지 못해 §9 비목표로 두었으나, v2에서 `vowel_combine_head` 한 boolean으로 데이터화.

### 3.2 큐 메타 평행 추적

자모가 composer 큐에 push될 때 출처 키의 `vowel_combine_head`도 함께 보관. composer는 큐의 jung을 합용 시도할 때 첫 jung의 meta를 검사:

```
입력 시퀀스:  ㄱ_default → ㅗ_v(head=false) → ㅏ_default(head=true)
큐:           [(Cho ㄱ, ⋯), (Jung ㅗ, head=false), (Jung ㅏ, head=true)]
compose_jung: jung 두 개 발견 → 첫 jung의 meta.vowel_combine_head=false
              → 결합 거부 → false 반환
                ↓
        add_jamo_with_meta가 분리 path 진입:
        마지막 자모(Jung ㅏ) pop → "고" complete + 새 음절 ㅏ 시작
```

데이터 흐름:
1. press_key 수준에서 char → key_meta_map 조회 → JamoMeta(vowel_combine_head 포함) 추출.
2. process_jamo_with_meta(jamo, meta) → composer.add_jamo_with_meta.
3. BaseHangulComposer.add_jamo_with(jamo, meta, compose_fn) → push_back_synced(jamo, meta).
4. compose_jung()이 jung_with_meta 평행 수집, 첫 jung head 검사.

코드 진입점:
- [press_key.rs:284-294](../../../src/input_engine/press_key.rs#L284-L294) — meta 추출
- [input_context.rs:107-145](../../../src/hangul/input_context.rs#L107-L145) — `process_jamo_with_meta`
- [composer.rs:235-320](../../../src/hangul/composer.rs#L235-L320) — sync helper들
- [composer.rs:861-905](../../../src/hangul/composer.rs#L861-L905) — `compose_jung` 룰 A 검사

### 3.3 두벌식 호환

두벌식 자판(`ko_2bulstd`)에는 `key_meta` 자체가 없다 → `key_meta_map`이 빈 HashMap → press_key 매번 `unwrap_or_default()` → `JamoMeta::default()` (`vowel_combine_head: true`). 모든 ㅗ가 head로 큐에 들어가 기존대로 ㅘ/ㅙ/ㅚ 결합.

회귀 테스트: `test_rule_a_two_bul_default_combines_o_a` ([mod.rs](../../../src/input_engine/mod.rs)).

### 3.4 평가 시점

큐에 jung이 **두 개 이상** 있을 때만 검사. jung 한 개만 있으면 `vowel_combine_head` 무관하게 정상 jung 채워짐 (단순 모음 입력은 항상 OK).

코드 한 줄 요약:
```rust
if jung_with_meta.len() > 1 && !first_meta.vowel_combine_head {
    return false;  // 결합 거부 → 분리
}
```

### 3.5 큐 sync 무결성

`jamo_queue`와 `meta_queue`는 항상 같은 길이여야 한다. 직접 mutate 진입점은 다음 4개 sync helper로 강제:

| Helper | 의미 |
|---|---|
| `push_back_synced(jamo, meta)` | 한 쌍으로 push |
| `pop_back_synced() -> Option<(JamoEnum, JamoMeta)>` | 한 쌍으로 pop |
| `clear_queues_synced()` | 두 큐 동시 비움 |
| `backup_to_last_synced()` | 현재 → last_*에 백업 + 두 큐 비움 |

외부 진입점 1곳(`composer_with_2bul.rs` 도깨비불 처리의 직접 `pop_back`)만 sync helper로 옮기면 평행 큐 무결성 보존. 이 외 모든 큐 mutate는 `BaseHangulComposer` 내부에 캡슐화.

---

## 4. 룰 B — `context_alt` (preedit 상태별 키 분기)

### 4.1 JSON 형태

```json
"key_meta": {
  "/": {
    "context_alt": {
      "when": "choseong_only",
      "to": "ㅗ",
      "fallback": "/"
    }
  }
}
```

### 4.2 ContextAlt 구조체

```rust
pub struct ContextAlt {
    pub when: ContextCondition,
    pub to: String,        // 조건 충족 시 출력 (실제로는 자모 → keyboard_map 통한 정상 자모 흐름)
    pub fallback: String,  // 조건 불충족 시 출력 (literal commit)
}
```

평가 동작:
- `when` 조건이 `true` → 정상 jamo 흐름으로 진입 (current code path가 자판 매핑된 자모를 process_jamo_with_meta로 전달).
- `false` → preedit flush + `fallback` 문자열을 commit_buffer에 push, return `InputResult::committed()`.

코드: [press_key.rs:259-302](../../../src/input_engine/press_key.rs#L259-L302)

### 4.3 ContextCondition 9개 변종

JSON에서는 모두 snake_case. 두 축으로 분류:

#### 4.3.1 상태 축 (current `HangulChar` 채워짐 패턴, 6개)

| `when` 값 | Rust 변이체 | helper | 의미 |
|---|---|---|---|
| `"empty"` | `Empty` | `!is_composing()` | preedit 비어 있음 |
| `"composing"` | `Composing` | `is_composing()` | 조합 중 (cho/jung/jong 아무거나) |
| `"choseong_only"` | `ChoseongOnly` | `is_only_cho_filled()` | 초성만 (jung·jong 없음) |
| `"jungseong_only"` | `JungseongOnly` | `is_only_jung_filled()` | 중성만 (cho·jong 없음) |
| `"cho_jung_filled"` | `ChoJungFilled` | `is_cho_jung_filled()` | cho+jung, jong 없음 |
| `"jongseong_filled"` | `JongseongFilled` | `is_jong_filled()` | 종성 들어 있음 |

#### 4.3.2 마지막 자모 축 (큐 `back()`, 3개)

| `when` 값 | Rust 변이체 | helper | 의미 |
|---|---|---|---|
| `"last_is_cho"` | `LastIsCho` | `last_jamo_is_cho()` | 큐 마지막 자모가 초성 |
| `"last_is_jung"` | `LastIsJung` | `last_jamo_is_jung()` | 큐 마지막 자모가 중성 |
| `"last_is_jong"` | `LastIsJong` | `last_jamo_is_jong()` | 큐 마지막 자모가 종성 |

상태 축은 "현재 음절이 어떤 모양인가"를, 마지막 자모 축은 "직전 키가 어느 종류였나"를 본다. 도깨비불·자동 종성·시퀀스 분기 등 시간 의존 로직에 후자가 유용.

코드:
- enum 정의: [schema.rs:90-118](../../../src/keystroke/profile/schema.rs#L90-L118)
- helper 6개: [input_context.rs:198-251](../../../src/hangul/input_context.rs#L198-L251)
- press_key match 9-arm: [press_key.rs:266-280](../../../src/input_engine/press_key.rs#L266-L280)

### 4.4 평가 위치

press_key 한국어 분기에서, **keyboard_map 조회 직전** + **다음 조건 모두 만족 시**:
- `input_category == Korean`
- `popup_state.is_none()` (한자/특수문자/이모지 팝업 비활성)
- `!hanja_mode && !special_char_mode`

영어 모드, 팝업 활성, 한자/특수문자 모드에서는 `context_alt` 비적용. 자판 디자이너는 한글 모드 전용 분기로 가정해야 한다.

### 4.5 룰 B와 룰 A 협력 — `/` 키 사례

`/` 키 정의:
```json
"/": {
  "context_alt": {
    "when": "choseong_only",
    "to": "ㅗ",
    "fallback": "/"
  }
  // vowel_combine_head는 base에 명시 안 함 → default true
}
```

흐름:
1. ㄱ 입력 (preedit "ㄱ", choseong-only).
2. `/` 입력 → `context_alt.when=choseong_only` 충족 → 정상 jamo 흐름 진입.
3. keyboard_map[`/`] = ㅗ. press_key가 ㅗ + JamoMeta(vowel_combine_head=true) 로 process.
4. 큐 `[Cho ㄱ, Jung ㅗ_head=true]`. preedit "고".
5. ㅏ 입력 (`f` 키, key_meta 없음 → default head=true). 큐 `[..., Jung ㅏ_head=true]`.
6. `compose_jung`: 첫 jung head=true → 결합 시도 → ㅗ+ㅏ→ㅘ → preedit "과".

반대로 cho+jung 채워진 상태에서 `/` 입력:
1. `/` 입력 → `context_alt.when=choseong_only` 불충족 (jung 차 있음) → fallback `"/"` literal commit.
2. preedit flush ("가" commit) + commit_buffer에 `/` push.

이 두 룰의 조합으로 사용자 결정 6번(타입픽스 라운드트립 미사용)을 데이터로 표현.

---

## 5. `rule_sets[name].key_meta` — 토글 가능한 키 메타

### 5.1 동기

자판 본질이 아니라 사용자 선호 의존인 키 메타는 rule_set으로 분리해 `active` 토글 가능하게. 예:
- 룰 A 자체를 끄고 모든 ㅗ/ㅜ를 결합 가능하게 하고 싶다.
- `/` 키의 컨텍스트 분기를 끄고 항상 ㅗ로 동작하게 하고 싶다.

ko_3bul390/391 빌트인은 두 룰 모두 별개 rule_set으로 분리, `active: true` 기본:

```json
"rule_sets": {
  "vowel_strict": {
    "active": true,
    "description": "ㅗ·ㅜ가 위치한 키마다 이중모음 결합 가부를 따로 둡니다. ...",
    "key_meta": {
      "v": { "vowel_combine_head": false },
      "b": { "vowel_combine_head": false }
    }
  },
  "slash_context_alt": {
    "active": true,
    "description": "/ 키를 컨텍스트에 따라 분기합니다. ...",
    "key_meta": {
      "/": {
        "context_alt": {
          "when": "choseong_only",
          "to": "ㅗ",
          "fallback": "/"
        }
      }
    }
  }
}
```

### 5.2 RuleSet schema 확장

기존 `RuleSet`에 `key_meta` 옵션 필드 추가:

```rust
pub struct RuleSet {
    pub active: bool,
    pub description: Option<LocalizedText>,
    pub combinations: Vec<RawTriple>,
    pub reinterpret: Vec<ReinterpretTriple>,  // legacy
    pub scope: Option<String>,                // legacy
    pub key_meta: Option<HashMap<String, KeyMeta>>,   // v2 신설
}
```

기존 `combinations`(자모 조합) 토글과 `key_meta`(키 메타) 토글이 같은 RuleSet 안에 공존 가능.

코드: [schema.rs:189-210](../../../src/keystroke/profile/schema.rs#L189-L210)

### 5.3 병합 규칙

`build_key_meta_char_map(profile)`이 char→KeyMeta 맵을 생성할 때:

1. **base 시작점**: `profile.key_meta`가 있으면 그것을 출발점으로 String 키 단위 HashMap에 적재.
2. **active rule_sets 순회**: `resolve_active_rule_set_names(profile)` 결과(`active_rule_sets` override 또는 각 rule_set의 `active`)를 BTreeMap 이름순으로 순회.
3. **필드 단위 덮어쓰기**: 각 active rule_set의 `key_meta` 항목을 base에 적용:
   - 같은 키의 `vowel_combine_head`가 `Some(_)`이면 덮어씀.
   - 같은 키의 `context_alt`가 `Some(_)`이면 덮어씀.
   - **다른 필드**는 보존 — base가 `vowel_combine_head=true`, rule_set이 `context_alt={...}`만 추가하면 둘 다 살아남음.
4. **String → char 좁히기**: 단일 문자 키만 char map에 수록.

우선순위 요약:
```
default(true) < base.key_meta < active rule_set.key_meta < (이름 순 후행 rule_set)
```

inactive rule_set (또는 `active_rule_sets: []`로 강제 비활성)의 `key_meta`는 적용되지 않는다 → 사용자가 `vowel_strict`를 끄면 v/b 키도 ㅘ/ㅝ 합용 가능 (느슨한 모드).

코드: [builder.rs:240-296](../../../src/keystroke/profile/builder.rs#L240-L296)

### 5.4 두 rule_set이 같은 키를 다룰 때

같은 키에 두 rule_set이 같은 필드를 정의하면 BTreeMap 이름순으로 후행 우선. 이 시나리오는 자판 디자이너가 의식적으로 만들 때만 발생. 권장: rule_set 하나당 하나의 의미로 분리해 충돌 회피.

테스트: `two_rule_sets_merge_per_field_for_same_key` ([builder.rs](../../../src/keystroke/profile/builder.rs)).

---

## 6. JamoMeta — composer 큐 평행 메타

### 6.1 정의

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct JamoMeta {
    pub vowel_combine_head: bool,
}

impl Default for JamoMeta {
    fn default() -> Self {
        Self { vowel_combine_head: true }   // 두벌식 호환
    }
}
```

`KeyMeta`(스키마, optional) → `JamoMeta`(런타임, concrete) 변환:
```rust
impl KeyMeta {
    pub fn to_jamo_meta(&self) -> JamoMeta {
        JamoMeta {
            vowel_combine_head: self.vowel_combine_head.unwrap_or(true),
        }
    }
}
```

코드: [schema.rs:79-89](../../../src/keystroke/profile/schema.rs#L79-L89), [composer.rs:16-33](../../../src/hangul/composer.rs#L16-L33)

### 6.2 trait 확장

`HangulComposer` trait에 새 method:
```rust
fn add_jamo_with_meta(&mut self, jamo: JamoEnum, _meta: JamoMeta) -> Option<char> {
    self.add_jamo(jamo)   // default: meta 무시 (호환)
}
```

`BaseHangulComposer`/`HangulComposer2Bul`/`HangulComposer3Bul`이 override해서 받은 meta를 큐에 push.

호출 흐름:
```
press_key → process_jamo_with_meta(jamo, meta)
         → composer.add_jamo_with_meta(jamo, meta)
         → base.add_jamo_with(jamo, meta, compose_fn)   [inherent helper]
         → push_back_synced(jamo, meta)
         → compose_fn (composer 종류별 규칙 검사 + base.compose_korean)
         → compose_jung() (룰 A 검사)
```

### 6.3 inherent compose_cho/jung/jong 제거

v1 코드에 `BaseHangulComposer`의 inherent `fn compose_cho/jung/jong`이 trait impl과 중복으로 존재했고, Rust method resolution이 inherent를 우선하므로 trait impl은 dead로 묻혀 있었다. v2에서 룰 A를 trait impl `compose_jung`에 추가하면서 inherent를 제거. 이제 모든 `self.compose_jung()` 호출이 trait method로 dispatch되어 룰 A가 일관 적용된다.

이 변경은 v2 부수 효과 — 잠재 버그(컴파일러는 ambiguity 안 잡지만 의도와 다르게 동작) 동시 해소.

---

## 7. 빌트인 자판 적용 현황

### 7.1 ko_3bul390 / ko_3bul391

```json
{
  "schema_version": 2,
  "rule_sets": {
    "sun_arae_batchim":   { "active": false, "combinations": [...] },   // v1 그대로
    "sun_arae_moeum":     { "active": false, "combinations": [...] },   // v1 그대로 (390만)
    "vowel_strict":       { "active": true,  "key_meta": { "v": ..., "b": ... } },
    "slash_context_alt":  { "active": true,  "key_meta": { "/": { "context_alt": ... } } }
  }
}
```

base `key_meta`는 비어 있음 — 항상 적용해야 하는 키 본질 메타가 현재 없으므로. 모든 v2 동작은 rule_set으로 토글 가능하다.

### 7.2 ko_3bul_noshift / ko_3bul_qwerty

`key_meta` 미적용. v1과 동일 (default head=true → 모든 ㅗ/ㅜ 결합 가능).

### 7.3 ko_2bulstd

`key_meta`/`schema_version` 모두 미명시 → schema_version=1 정규화. 두벌식은 룰 A·B 무관.

### 7.4 영문 자판 (en_qwerty/dvorak/colemak/colemak_dh/workman)

`key_meta` 미적용. press_key 한글 분기 자체가 안 진입 (input_category=English).

---

## 8. 사용자 자판 작성 가이드

### 8.1 최소 v2 예시

```json
{
  "schema_version": 2,
  "language": "korean",
  "name": "my_3bul_strict",
  "type": "3bul",
  "metadata": { "display_name": "내 세벌식 (엄격)", "version": "1.0" },
  "layout": { "upper": {...}, "lower": {...} },
  "combinations": { "cho": [...], "jung": [...], "jong": [...] },
  "key_meta": {
    "v": { "vowel_combine_head": false }
  }
}
```

이렇게 하면 `v` 키 ㅗ가 단순 모음만, 나머지는 default(true)로 결합 가능.

### 8.2 토글 가능한 자판 만들기

```json
{
  "schema_version": 2,
  "rule_sets": {
    "my_strict_mode": {
      "active": true,
      "description": "내 자판 엄격 모드 — v 키 ㅗ 비결합. 끄면 모든 ㅗ가 합용 가능.",
      "key_meta": {
        "v": { "vowel_combine_head": false }
      }
    }
  }
}
```

사용자가 `active: false`로 자판 JSON 직접 편집하거나, `active_rule_sets: ["다른_세트"]`로 override해 끄면 룰이 풀린다. (UI 토글은 0.2.0 시점 후속.)

### 8.3 컨텍스트 분기 활용 패턴

`,` 키를 종성이 들어 있을 때만 다른 자모로 분기:
```json
"key_meta": {
  ",": {
    "context_alt": {
      "when": "jongseong_filled",
      "to": "ㅢ",
      "fallback": ","
    }
  }
}
```

종성 채워진 음절 뒤에서만 ㅢ, 그 외엔 literal `,`. (실제로는 `to`가 자판 layout에 매핑된 한글 자모여야 정상 처리됨 — keyboard_map 통한 jamo 흐름 진입이라.)

### 8.4 `to` 필드의 의미

`context_alt.to`는 현 구현에서 **참고 표기**. 실제 분기 시 정상 jamo 흐름으로 들어가면 `keyboard_map.get(&c)`가 자판 layout에서 자모를 찾는다. 따라서 `to`에 적은 자모와 layout 셀에 적힌 자모가 일치해야 의미대로 동작.

`fallback`은 literal commit이라 자판 layout과 무관 — 자판 디자이너 임의 문자열.

---

## 9. 코드 진입점 매핑 (0.2.0+)

| 책임 | 파일:line |
|---|---|
| `KeyMeta`/`ContextAlt`/`ContextCondition` 정의 | [schema.rs:70-118](../../../src/keystroke/profile/schema.rs#L70-L118) |
| `RuleSet.key_meta` 필드 | [schema.rs:189-210](../../../src/keystroke/profile/schema.rs#L189-L210) |
| `KeyMeta::to_jamo_meta()` | [schema.rs:79-89](../../../src/keystroke/profile/schema.rs#L79-L89) |
| `LayoutProfile.key_meta` (정규화) | [schema.rs:262-280](../../../src/keystroke/profile/schema.rs#L262-L280) |
| `build_key_meta_char_map()` | [builder.rs:240-296](../../../src/keystroke/profile/builder.rs#L240-L296) |
| `JamoMeta` 정의 + Default | [composer.rs:16-33](../../../src/hangul/composer.rs#L16-L33) |
| sync helper 4개 | [composer.rs:235-320](../../../src/hangul/composer.rs#L235-L320) |
| `add_jamo_with_meta` trait method | [composer.rs:38-50](../../../src/hangul/composer.rs#L38-L50) |
| `add_jamo_with` inherent (callback + meta) | [composer.rs:556-630](../../../src/hangul/composer.rs#L556-L630) |
| `compose_jung` 룰 A 검사 | [composer.rs:861-905](../../../src/hangul/composer.rs#L861-L905) |
| 2bul `add_jamo_with_meta` override | [composer_with_2bul.rs:467-490](../../../src/hangul/composer_with_2bul.rs#L467-L490) |
| 3bul `add_jamo_with_meta` override | [composer_with_3bul.rs:294-318](../../../src/hangul/composer_with_3bul.rs#L294-L318) |
| `process_jamo_with_meta` | [input_context.rs:107-145](../../../src/hangul/input_context.rs#L107-L145) |
| 9개 ContextCondition helper | [input_context.rs:198-251](../../../src/hangul/input_context.rs#L198-L251) |
| `key_meta_map` 필드 + `create_key_meta_map` | [engine.rs:33,231-240,343,379](../../../src/input_engine/engine.rs) |
| press_key 룰 B 분기 + 룰 A meta 추출 | [press_key.rs:259-302](../../../src/input_engine/press_key.rs#L259-L302) |

---

## 10. 테스트 매트릭스

### 10.1 단위 (schema·builder·composer·input_context)

| 테스트 | 위치 | 검증 |
|---|---|---|
| `schema_v2_key_meta_parses_successfully` | schema.rs | base key_meta v2 파싱 |
| `key_meta_round_trip_serde` | schema.rs | KeyMeta JSON round-trip |
| `key_meta_rejects_unknown_when_value` | schema.rs | unknown ContextCondition 거부 |
| `context_condition_all_variants_round_trip` | schema.rs | 9개 변종 직렬/역직렬 |
| `rule_set_key_meta_active_merges_into_base` | builder.rs | active rule_set 병합 |
| `rule_set_key_meta_inactive_is_ignored` | builder.rs | inactive 무시 |
| `active_rule_sets_override_disables_key_meta` | builder.rs | override 강제 비활성 |
| `two_rule_sets_merge_per_field_for_same_key` | builder.rs | 두 rule_set 필드 단위 병합 |
| `base_key_meta_combines_with_rule_set_key_meta` | builder.rs | base + rule_set 병합 |
| `rule_set_key_meta_overrides_base_for_same_field` | builder.rs | rule_set 우선 |
| `ko_3bul390_builtin_rule_sets_apply_key_meta` | builder.rs | 빌트인 적용 검증 |
| `context_helpers_*` (5개) | input_context.rs | 9개 helper 단위 동작 |

### 10.2 통합 (input_engine 키 입력 → preedit/commit)

| 테스트 | 시나리오 |
|---|---|
| `test_rule_b_empty_preedit_commits_slash` | / 키 빈 preedit → literal `/` |
| `test_rule_b_choseong_only_keeps_jamo` | ㄱ + / → preedit "고" |
| `test_rule_b_cho_jung_filled_commits_slash` | "가" + / → "가" commit + `/` |
| `test_rule_b_english_mode_unaffected` | 영문 모드 / → 분기 미적용 |
| `test_rule_b_two_bul_no_key_meta_branch` | 두벌식 key_meta_map 비어 있음 |
| `test_rule_a_v_key_o_does_not_combine_with_a` | ㄱ + v + ㅏ → "고" + "ㅏ" |
| `test_rule_a_slash_key_o_combines_with_a_via_rule_b` | ㄱ + / + ㅏ → preedit "과" |
| `test_rule_a_b_key_u_does_not_combine_with_eo` | ㄱ + b + ㅓ → "구" + "ㅓ" |
| `test_rule_a_nine_key_u_combines_with_eo` | ㄱ + 9 + ㅓ → preedit "궈" |
| `test_rule_a_two_bul_default_combines_o_a` | 두벌식 ㅗ + ㅏ → "과" 정상 |
| `test_rule_a_v_key_3bul391_does_not_combine` | 391도 v 키 동일 |

총: 0.2.0 시점 lib 테스트 483 PASS (v2 신규 ≈ 24개 포함).

---

## 11. v1 → v2 마이그레이션

### 11.1 자판 작성자 입장

v1 자판은 변경 없이 v2 코드에서 그대로 작동. `key_meta` 누락 = 기존 동작.

새 v2 기능을 쓰고 싶으면:
1. `schema_version: 2` 명시 (선택, 가독성).
2. `key_meta` 또는 `rule_sets[name].key_meta` 추가.
3. 두벌식 호환성 깨지 않도록 default(true) 의미 인지 — `vowel_combine_head: false`는 명시적 거부.

### 11.2 코드 변경 영향 (라이브러리 사용자)

`HangulComposer` trait에 `add_jamo_with_meta` 추가됐지만 default impl이 `add_jamo`로 위임 — 외부 구현체는 변경 불필요. trait의 `jamo_queue()` 시그니처 무변경. 외부에서 직접 jamo_queue를 mutate하는 코드는 없으므로 영향 0.

`HangulInputContext::process_jamo`도 시그니처 무변경 (`process_jamo_with_meta`로 default meta 위임하는 thin wrapper).

### 11.3 v1 문서 §9 비목표 갱신

v1 §9의 처음 두 항목은 v2에서 구현됨. v1 문서에 cross-ref 주석 추가됨. 나머지 비목표(모아치기·복벌식·옛한글·날개셋 .ist 파싱·한자/상용구 번들·임의 깊이 자동 추론)는 여전히 v3 또는 별도 단계.

---

## 12. v2 비목표 (v3로 미룸)

`key_meta` 데이터 표현이 데이터로 흡수하지 않은 것:

- **여러 키가 함께 눌렸을 때의 시퀀스 분기** (chord/glide). 현재 `context_alt`는 한 키 누를 때 단발 평가만.
- **time-based 조건** (예: "200ms 이내 두 번 누르면" 같은 doubletap). 시각 기반 분기 없음.
- **자모 조합 시도 자체의 동적 차단** (예: "특정 음절 패턴 조합 거부"). 룰 A는 *jung 두 개 결합* 단계만 검사.
- **`to` 필드의 다중 자모 시퀀스** — 현재 단일 자모만 의미 있음. 시퀀스 commit은 미지원.
- **외부 상태 의존 분기** (Caps Lock 외 application focus, IM 활성 시간 등). press_key가 보유한 정보만 사용.
- **사용자 GUI 토글** — 자판 JSON 편집 또는 `active_rule_sets` config로만 가능. 0.2.0 후속에서 GUI 추가 예정.

---

## 13. 변경 이력

- **2026-05-01**: PR-A schema dangling 머지 (KeyMeta/ContextAlt/ContextCondition::ChoseongOnly).
- **2026-05-02**: PR-B1 룰 B 활성 (press_key 분기 + ko_3bul390/391 base key_meta).
- **2026-05-02**: PR-B2 룰 A 본구현 (composer 큐 평행 meta + compose_jung 검사).
- **2026-05-02**: rule_sets[name].key_meta 토글 가능성 추가 (vowel_strict / slash_context_alt 분리).
- **2026-05-02**: ContextCondition 1 → 9 확장 (Empty/Composing + 6개 상태/마지막 자모 축).

## 14. 참고

- v1 베이스: [LAYOUT_PROFILE_V1.md](LAYOUT_PROFILE_V1.md) (§3.5 rule_sets 의미, §4 inherits)
- v1 IMPL 하네스: [LAYOUT_PROFILE_V1_IMPL.md](LAYOUT_PROFILE_V1_IMPL.md)
- 사전 조사 / 구현 plan: 옵시디언 `2 Projects/ATIT/unim/archive/_workspace/3bul_strict_vowel_{research,plan}.md`
- ContextCondition 단위 테스트: `src/keystroke/profile/schema.rs::tests::context_condition_all_variants_round_trip`
- 룰 A·B 통합 테스트: `src/input_engine/mod.rs::tests::test_rule_*`
