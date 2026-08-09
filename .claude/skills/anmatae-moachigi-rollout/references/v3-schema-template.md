# Layout Profile v3 — Schema 명세 템플릿

> 본 파일은 Phase 1 산출물(`_workspace/anmatae/01_schema_v3.md`)의 표준 구조다. 매니저는 이 9개 절을 모두 채워야 한다.

## 1. v3 마커 식별 알고리즘

RawProfile에서 다음 중 하나라도 있으면 v3로 처리:

```rust
fn is_v3(raw: &RawProfile) -> bool {
    raw.schema_version == Some(3)
        || raw.layout_type.is_some()
        || raw.moachigi.is_some()
        || raw.jamo_symbol_map.is_some()
}
```

식별 우선순위: schema_version → layout_type → moachigi → jamo_symbol_map. v1·v2 게이트는 불변(LAYOUT_PROFILE_V2.md §1.1·1.2 그대로).

## 2. JSON 스키마 — 안마태 예시

```jsonc
{
  "schema_version": 3,
  "language": "korean",
  "name": "anmatae_2003",
  "layout_type": "anmatae",
  "metadata": {
    "display_name": {
      "ko": "안마태 자판 (2003)",
      "en": "Ahnmatae Keyboard (2003)"
    },
    "author": "안마태·김진형 (2003)",
    "description": "...",
    "version": "1.0",
    "tags": ["korean", "anmatae", "moachigi", "phonetic"]
  },
  "layout": {
    "lower": { "1st": [...], "2nd": [...], "3rd": [...], "4th": [...] },
    "upper": { ... }
  },
  "combinations": {
    "cho": [
      {"first":"ㄱ","second":"ㅎ","result":"ㅋ"},
      {"first":"ㄱ","second":"ㄱ","result":"ㄲ"}
      // ... 거센·된소리 전체
    ],
    "jung": [
      {"first":"ㅏ","second":"ㅣ","result":"ㅐ"},
      {"first":"ㅗ","second":"ㅏ","result":"ㅘ"}
    ],
    "jong": [
      // 겹받침 11개 — moachigi.jong_unordered=true면 양방향 자동 등록
    ]
  },
  "moachigi": {
    "syllable_boundary": "region_filled",
    "jong_unordered": true,
    "jung_unordered": false,
    "region_filled": true
  },
  "jamo_symbol_map": {
    "p": { "emit_char": "·", "comment": "고어 ㆍ 위치 → 가운뎃점(U+00B7)" },
    "P": { "emit_char": "°", "comment": "shift+p" }
  },
  "rule_sets": {
    "moachigi_strict": {
      "display_name": {"ko": "엄격 순서 (모아치기 OFF)"},
      "moachigi_overrides": { "syllable_boundary": "strict", "jong_unordered": false }
    }
  },
  "active_rule_sets": []
}
```

## 3. JSON 스키마 — 세벌식 + 모아치기 옵션 예시

```jsonc
{
  "schema_version": 3,
  "language": "korean",
  "name": "ko_3bul_moachigi",
  "layout_type": "moachigi_3bul",
  "inherits": ["ko_3bul390"],
  "metadata": {
    "display_name": {"ko": "세벌식 390 + 모아치기"}
  },
  "moachigi": {
    "syllable_boundary": "strict",
    "jong_unordered": false,
    "jung_unordered": false,
    "region_filled": false
  },
  "rule_sets": {
    "moachigi_jong_unordered": {
      "display_name": {"ko": "종성 순서 자유"},
      "moachigi_overrides": { "jong_unordered": true }
    },
    "moachigi_region_free": {
      "display_name": {"ko": "영역 간 순서 자유"},
      "moachigi_overrides": { "syllable_boundary": "region_filled", "region_filled": true }
    }
  },
  "active_rule_sets": []
}
```

## 4. Rust 내부 표현 확장

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum LayoutType {
    #[default]
    Jamo,
    Anmatae,
    Moachigi3Bul,
}

#[derive(Debug, Clone, Default)]
pub struct MoachigiSpec {
    pub syllable_boundary: SyllableBoundary,
    pub jong_unordered: bool,
    pub jung_unordered: bool,
    pub region_filled: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SyllableBoundary {
    #[default]
    Strict,
    RegionFilled,
}

#[derive(Debug, Clone)]
pub struct SymbolEmit {
    pub emit_char: char,
    pub comment: Option<String>,
}

pub struct LayoutProfile {
    // ... 기존 필드
    pub layout_type: LayoutType,
    pub moachigi: Option<MoachigiSpec>,
    pub jamo_symbol_map: HashMap<KeyCode, SymbolEmit>,
}
```

## 5. 로더 파이프라인 변경

`parse_v3` 분기 신설. v1·v2 경로는 무변경. `MoachigiSpec` 검증 규칙:
- `layout_type=Anmatae`인데 `moachigi` 없으면 `LoadError::MissingMoachigiBlock`
- `layout_type=Jamo`인데 `moachigi` 있으면 경고 후 무시
- `jamo_symbol_map` 키와 `layout` 키가 충돌하면 `jamo_symbol_map` 우선 + 경고

## 6. rule_set merge 규칙

`active_rule_sets`에 포함된 룰셋이 순서대로 base `moachigi` 위에 layer-merge:

```rust
fn merge_moachigi(base: MoachigiSpec, overrides: &[MoachigiOverride]) -> MoachigiSpec {
    let mut effective = base;
    for ov in overrides {
        if let Some(v) = ov.syllable_boundary { effective.syllable_boundary = v; }
        if let Some(v) = ov.jong_unordered    { effective.jong_unordered = v; }
        if let Some(v) = ov.jung_unordered    { effective.jung_unordered = v; }
        if let Some(v) = ov.region_filled     { effective.region_filled = v; }
    }
    effective
}
```

같은 키를 두 룰셋이 다루면 **나중에 활성화된 룰셋 우선** (LAYOUT_PROFILE_V2.md §5.4 동등).

## 7. 5지점 동기화 영향

| 지점 | 영향 |
|------|------|
| src/config.rs | **무영향** — `korean.custom_layout`, `korean.active_rule_sets` 기존 필드 재사용 |
| unim-cli config 서브커맨드 | **무영향** — 기존 ConfigKey 재사용 |
| unim-cli locales/*.yml | **무영향** |
| unim-dbus service.rs | **무영향** |
| unim-gui-gtk settings_dialog.rs | ComboRow 항목 추가 + SwitchRow 동적 재구성(v2 로직 재사용) |
| unim-gnome-extension | **무영향** (한국어 자판은 GTK GUI에서 관리) |

→ 5지점 동기화 부담 없음. 사용자 요구 "키맵 룰셋으로 토글" 설계의 부수 효과.

## 8. 테스트 매트릭스 (schema 라운드트립)

| ID | 입력 | 기대 |
|----|------|------|
| sv1 | v1 자판 10종 그대로 | 모두 v1 path 통과, 회귀 0 |
| sv2 | v2 자판 (key_meta 포함) | v2 path, 회귀 0 |
| sv3-jamo | v3 layout_type=jamo | v2와 동일 composer |
| sv3-anmatae | v3 layout_type=anmatae | HangulComposerAnmatae 선택 |
| sv3-moachigi-3bul | v3 layout_type=moachigi_3bul + inherits=ko_3bul390 | 3bul composer + moachigi rule_set 적용 |
| sv3-missing-moachigi | layout_type=anmatae, moachigi 없음 | LoadError::MissingMoachigiBlock |
| sv3-symbol-collision | jamo_symbol_map 키와 layout 키 충돌 | symbol 우선 + 경고 로그 |
| sv3-rule-merge | 두 moachigi rule_set 동시 활성 | 나중 활성 룰셋 우선 |

## 9. v2 → v3 마이그레이션

자동 승격 없음. v2 자판은 그대로 v2로 동작. 사용자가 모아치기/안마태를 원하면 명시적으로 v3 JSON 작성. 마이그레이션 도구는 v0.3.0 비목표.

## 사용자 결정 필요 (Phase 0 입력 필수)

- [ ] 안마태 변종 (2003 표준 / 신세벌식M / 기타)
- [ ] 디폴트 활성 모아치기 룰셋
- [ ] 빌트인 포함 여부 (v0.3.0)
