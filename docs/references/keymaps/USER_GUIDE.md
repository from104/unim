# UNIM 자판 만들기 — 사용자 가이드

> 이 문서는 자판을 직접 만들거나 기존 자판의 옵션을 켜고 끄려는 사용자를 위한 안내입니다. 코드 내부 동작이 궁금하면 [LAYOUT_PROFILE_V2.md](../../dev/plans/LAYOUT_PROFILE_V2.md)를 참고하세요.

대상 UNIM 버전: 0.2.0 이후
지원 스키마: `schema_version: 1` (간단), `schema_version: 2` (키별 메타데이터 사용)

---

## 1. 5분 시작하기

### 1.1 가장 작은 자판 파일

```json
{
  "schema_version": 2,
  "language": "korean",
  "name": "my_first",
  "type": "2bul",
  "metadata": {
    "display_name": "내 첫 자판",
    "version": "1.0"
  },
  "layout": {
    "upper": {
      "1st": [], "2nd": [], "3nd": [], "4th": []
    },
    "lower": {
      "1st": [], "2nd": [], "3nd": [], "4th": []
    }
  },
  "combinations": {
    "cho": [],
    "jung": [],
    "jong": []
  }
}
```

이 파일을 `~/.config/unim/layouts/my_first.json` 으로 저장하면 UNIM이 다음 시작 때 인식합니다. 키 배열이 비어 있어 실제 입력은 안 되지만, "스키마는 통과"하는 최소 골격입니다.

### 1.2 자판 파일 위치

| 종류 | 위치 |
|---|---|
| UNIM 빌트인 | `/usr/share/unim/...` (수정 불가) |
| 사용자 자판 | `~/.config/unim/layouts/*.json` |

UNIM은 둘 다 같은 이름공간에 합쳐 인식합니다. 빌트인과 같은 이름으로 사용자 자판을 두면 사용자 자판이 우선합니다.

### 1.3 활성화

자판을 만들었으면 unim-cli 또는 GUI 설정에서 한국어 레이아웃 이름을 바꿉니다:

```bash
unim-cli config set engine.korean.layout my_first
```

UNIM 데몬이 새 자판을 즉시 로드하거나, 변경이 안 되면 데몬을 재시작:

```bash
systemctl --user restart unim
```

---

## 2. 자판 메타데이터

```json
"metadata": {
  "display_name": "세벌식 390",
  "author": "공병우 (1990)",
  "description": "공병우 박사가 1990년 발표한 세벌식 자판. ...",
  "version": "1.0",
  "tags": ["korean", "3bul", "standard"]
}
```

| 필드 | 의미 |
|---|---|
| `display_name` | 설정 화면에 보일 이름 (생략 시 `name` 그대로) |
| `author` | 만든 사람 / 출처 |
| `description` | 설정 도움말이나 README에 노출되는 설명 |
| `version` | 자판 자체의 버전 (자판 작성자 임의) |
| `tags` | 분류 태그. UI 검색·필터용 |

`display_name`과 `description`은 한국어/영어 동시 표기도 가능합니다:

```json
"display_name": {
  "ko": "세벌식 390",
  "en": "Sebeolsik 390"
}
```

UNIM이 시스템 언어에 맞춰 적절한 쪽을 골라 보여줍니다.

---

## 3. 키 배열 (layout)

`layout`은 자판의 14×4 격자 — 14열은 키 위치(영문 키맵 기준), 4행은 키 종류(숫자행/위/중간/아래):

```json
"layout": {
  "upper": {
    "1st": ["...", "...", ...],   // shift 누른 숫자행 (~!@#$...)
    "2nd": ["...", ...],          // shift 누른 윗줄  (Q W E ...)
    "3nd": ["...", ...],          // shift 누른 가운뎃줄 (A S D ...)
    "4th": ["...", ...]           // shift 누른 아랫줄 (Z X C ...)
  },
  "lower": {
    "1st": ["...", ...],          // 그냥 누른 숫자행 (`1234...)
    "2nd": [...],                 // 그냥 누른 윗줄 (q w e ...)
    "3nd": [...],                 // 가운뎃줄 (a s d ...)
    "4th": [...]                  // 아랫줄 (z x c ...)
  }
}
```

> **참고**: 행 이름이 `1st`, `2nd`, `3nd`(오타 아니라 역사적 명명), `4th`입니다.

각 셀은 한글 자모 한 글자 또는 다른 문자(특수기호 등):
- 초성: `"ㄱ"`, `"ㄴ"`, `"ㄷ"` ...
- 중성: `"ㅏ"`, `"ㅑ"`, `"ㅓ"` ...
- 종성: `"ᆨ"`, `"ᆫ"`, `"ᆮ"` ... (조합형 종성. 호환 자모 ㄱ과 다른 코드)
- 기타: `"~"`, `"@"`, `","` 같은 문자 (영문/특수)

빈 슬롯은 `""` 또는 생략. 한 행 길이는 자판마다 다릅니다 (보통 14칸 또는 12칸).

---

## 4. 자모 결합 (combinations)

`combinations`은 두 자모가 만났을 때 합쳐지는 규칙:

```json
"combinations": {
  "cho": [
    { "first": "ㄱ", "second": "ㄱ", "result": "ㄲ" }
  ],
  "jung": [
    { "first": "ㅗ", "second": "ㅏ", "result": "ㅘ" },
    { "first": "ㅗ", "second": "ㅐ", "result": "ㅙ" }
  ],
  "jong": [
    { "first": "ᆨ", "second": "ᆨ", "result": "ᆩ" },
    { "first": "ᆨ", "second": "ᆺ", "result": "ᆪ" }
  ]
}
```

- `cho`: 초성+초성 → 새 초성 (ㄱ+ㄱ = ㄲ)
- `jung`: 중성+중성 → 새 중성 (ㅗ+ㅏ = ㅘ)
- `jong`: 종성+종성 → 새 종성 (ᆨ+ᆨ = ᆩ, ᆨ+ᆺ = ᆪ)

종성 자모는 조합형(`ᆨ` U+11A8)을 씁니다. 호환 자모(`ㄱ` U+3131)와 코드가 달라요.

---

## 5. 옵션 그룹 (rule_sets) — 켜고 끄기

자판의 어떤 동작을 사용자가 켜고 끄게 만들고 싶다면 `rule_sets`에 묶습니다.

### 5.1 가장 단순한 형태 — 자모 결합 추가/제거

세벌식 390 자판의 "순아래받침" 옵션은 받침을 8개로 단순화하는 자판을 함께 쓸 때만 필요한 추가 결합 규칙입니다:

```json
"rule_sets": {
  "sun_arae_batchim": {
    "active": false,
    "description": "받침을 8개(ㄱㄴㄹㅁㅂㅅㅇㅎ)만 쓰는 단순화된 자판을 함께 쓸 때 켭니다. ㄴ+ㄴ→ㄷ, ㄱ+ㅎ→ㅋ처럼 두 키 조합으로 겹받침과 거센소리 받침을 만듭니다.",
    "combinations": [
      { "first": "ᆫ", "second": "ᆫ", "result": "ᆮ" },
      { "first": "ᆨ", "second": "ᇂ", "result": "ᆿ" }
    ]
  }
}
```

| 필드 | 의미 |
|---|---|
| `active` | 기본 활성 여부 (`true` 켜짐 / `false` 꺼짐) |
| `description` | 사용자가 켜고 끌 때 보일 설명. 효과를 명확히 적으세요. |
| `combinations` | 이 옵션이 켜졌을 때 추가될 결합 규칙들 |

`description`은 한국어/영어 동시 표기 가능:

```json
"description": {
  "ko": "받침을 8개만 쓸 때 켭니다.",
  "en": "Enable when using 8-jongseong simplified layout."
}
```

### 5.2 사용자가 옵션 끄는 방법

세 가지 방법 모두 가능합니다.

#### 5.2.1 GUI 설정 다이얼로그

UNIM 설정 다이얼로그(`unim-gui-gtk --settings`)의 "일반" 페이지에서 한국어 자판을 선택하면 그 자판의 rule_sets 그룹이 자동으로 나타납니다. 각 옵션마다 토글 스위치 + `description` 툴팁이 보이며, 켜고 끌 때 즉시 데몬에 반영됩니다.

#### 5.2.2 CLI 명령

```bash
# 현재 활성 목록 보기
unim-cli config get korean_active_rule_sets

# 특정 옵션만 활성으로 만들기 (다른 건 꺼짐)
unim-cli config set korean_active_rule_sets vowel_strict,slash_context_alt

# 모든 rule_set 끄기 (빈 배열)
unim-cli config set korean_active_rule_sets ""

# 자판 기본값으로 되돌리기 (각 rule_set의 active 그대로 사용)
unim-cli config unset korean_active_rule_sets
```

#### 5.2.3 자판 JSON 직접 편집

자판 JSON의 `"active": false`로 직접 바꾸거나, 자판 단에서 명시적으로 활성 목록을 지정:

```json
"active_rule_sets": ["vowel_strict"]
```

이러면 이름 목록에 없는 모든 rule_set은 강제로 꺼집니다. `"active_rule_sets": []` 는 모든 rule_set 비활성을 의미합니다.

#### 5.2.4 우선순위

세 방법 사이의 우선순위:

- 사용자 설정(GUI/CLI로 저장된 `config.engine.korean.active_rule_sets`)이 자판 JSON의 `active_rule_sets`보다 우선합니다.
- 사용자 설정이 미설정(unset)이면 자판 JSON 값을 따릅니다.
- 자판 JSON에도 `active_rule_sets`가 없으면 각 rule_set의 `active` 필드를 그대로 사용합니다.

---

## 6. 고급 기능 1 — 같은 자모, 다른 키별 결합 정책

### 6.1 무엇을 위한 기능인가

세벌식 390 자판에는 ㅗ가 두 군데 있습니다:
- `v` 키 (Z줄 4번째)
- `/` 키 (Z줄 마지막)

원본 공병우 규약에서는:
- `v` 키 ㅗ는 **단순 모음만** — 뒤에 ㅏ가 와도 ㅘ로 합치지 않고 새 음절 시작
- `/` 키 ㅗ는 **이중모음 첫 모음** 가능 — 뒤에 ㅏ가 오면 ㅘ로 합쳐짐

같은 ㅗ인데 어느 키에서 들어왔는지에 따라 동작이 달라야 합니다. 이걸 `vowel_combine_head` 한 가지 boolean으로 표현합니다.

### 6.2 사용 방법

```json
"key_meta": {
  "v": { "vowel_combine_head": false },
  "b": { "vowel_combine_head": false }
}
```

`v` 키와 `b` 키는 후속 모음과 합치지 말라는 표시. 다른 키들은 명시 없으면 합칠 수 있는 것이 기본값.

### 6.3 결과 동작

```
ㄱ + v(ㅗ) + ㅏ
  → "고" 확정 + 새 음절 "ㅏ"

ㄱ + /(ㅗ) + ㅏ
  → "과" (계속 조합 중)
```

### 6.4 가능한 활용 사례

- 사용자가 잘못 누른 키를 합치지 않게 막고 싶을 때
- 한 자판에 같은 자모가 여러 키에 있을 때 키별 행동 분리
- 옛한글이나 변형 자판에서 "이 키만 단순 모음" 같은 규칙

### 6.5 두벌식과의 호환

두벌식(`ko_2bulstd`)은 `key_meta`가 아예 없습니다. 명시 없으면 모든 자모가 합쳐질 수 있는 기본값이라 기존 동작이 그대로 유지됩니다.

---

## 7. 고급 기능 2 — preedit 상태별 키 분기

### 7.1 무엇을 위한 기능인가

세벌식 390 자판의 `/` 키는:
- preedit이 **초성만** 있을 때(`고`라는 음절을 시작 중) → ㅗ로 동작 → ㄱ+ㅗ = "고"
- 그 외 (빈 preedit, 또는 음절이 더 진행됐을 때) → 그냥 `/` 글자 출력

같은 키가 컨텍스트에 따라 다른 의미가 되는 것. `context_alt` 필드로 표현합니다.

### 7.2 사용 방법

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

| 필드 | 의미 |
|---|---|
| `when` | 분기 조건 (아래 표 참고) |
| `to` | 조건이 맞을 때 동작할 자모 (자판 layout에 매핑된 자모와 일치해야 함) |
| `fallback` | 조건이 안 맞을 때 그대로 출력할 문자열 |

### 7.3 사용 가능한 9가지 조건

#### 음절 모양 기반 (6개)

| `when` 값 | 언제 참인가 |
|---|---|
| `"empty"` | preedit 비어 있음 (한글 조합 안 하고 있을 때) |
| `"composing"` | 한글 조합 중 (어떤 자모라도 들어 있음) |
| `"choseong_only"` | 초성 1개만 있음 (예: `ㄱ`) |
| `"jungseong_only"` | 중성만 있음 (예: `ㅏ` — 초성·종성 없이) |
| `"cho_jung_filled"` | 초성+중성 있고 종성 없음 (예: `가`) |
| `"jongseong_filled"` | 종성 있음 (예: `각`, `갉`) |

#### 직전 키 종류 기반 (3개)

| `when` 값 | 언제 참인가 |
|---|---|
| `"last_is_cho"` | 마지막에 누른 키가 초성 |
| `"last_is_jung"` | 마지막에 누른 키가 중성 |
| `"last_is_jong"` | 마지막에 누른 키가 종성 |

### 7.4 활용 예시

#### 종성이 있을 때만 다른 자모로

```json
",": {
  "context_alt": {
    "when": "jongseong_filled",
    "to": "ㅢ",
    "fallback": ","
  }
}
```

받침 있는 음절 뒤에 `,`를 누르면 ㅢ로, 그 외에는 그대로 `,`.

#### 빈 상태에서만 특수문자

```json
"=": {
  "context_alt": {
    "when": "empty",
    "to": "ㅛ",
    "fallback": "="
  }
}
```

조합 중일 때는 `=`을 그대로 출력하고, 빈 상태에서만 ㅛ로 동작.

### 7.5 룰 A·B 함께 쓰기

`vowel_combine_head`와 `context_alt`을 같은 키에 둘 다 둘 수 있습니다. 두 룰이 협력해서 세벌식 390의 `/` 키 동작을 만듭니다 (자세한 사례는 §10 빌트인).

```json
"/": {
  "vowel_combine_head": true,
  "context_alt": {
    "when": "choseong_only",
    "to": "ㅗ",
    "fallback": "/"
  }
}
```

> **주의**: `vowel_combine_head: true`는 명시 안 해도 같은 결과(기본값 결합 가능). 명시는 의도 표현용.

---

## 8. 옵션과 키 메타 한꺼번에 — rule_sets에 key_meta

§5의 `rule_sets`에서는 자모 결합만 다뤘지만, **키 메타도 옵션으로 묶을 수 있습니다**. 이러면 `vowel_combine_head`나 `context_alt`을 사용자가 끌 수 있게 됩니다.

```json
"rule_sets": {
  "vowel_strict": {
    "active": true,
    "description": "ㅗ·ㅜ가 위치한 키마다 이중모음 결합 가부를 따로 둡니다. /·9 키의 ㅗ·ㅜ는 후속 ㅏ/ㅐ/ㅣ/ㅓ/ㅔ와 합쳐 ㅘ/ㅙ/ㅚ/ㅝ/ㅞ/ㅟ로 결합되지만, v·b 키의 ㅗ·ㅜ는 단순 모음만 — 후속 모음이 와도 새 음절로 분리됩니다. 끄면 모든 ㅗ·ㅜ가 합쳐질 수 있어 v·b 키에서도 합용 가능.",
    "key_meta": {
      "v": { "vowel_combine_head": false },
      "b": { "vowel_combine_head": false }
    }
  },
  "slash_context_alt": {
    "active": true,
    "description": "/ 키를 컨텍스트에 따라 분기합니다. preedit이 초성 한 개만 들어 있을 때는 ㅗ로 동작해 ㅘ/ㅙ/ㅚ 합용을 가능하게 하고, 그 외(빈 preedit·중성·종성 채워짐·영어 모드)에서는 리터럴 / 로 출력됩니다. 끄면 / 키가 어떤 상황에서도 항상 ㅗ로 동작합니다.",
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

이렇게 만들면:
- 두 옵션 모두 기본 켜짐 (`active: true`).
- 사용자가 `vowel_strict`만 끄면 v/b 키도 모음 결합 가능, `/` 분기는 그대로.
- `slash_context_alt`만 끄면 v/b는 그대로 단순 모음, `/`는 항상 ㅗ.

자판 본질이 아니라 사용자가 선택할 수 있는 동작은 이렇게 rule_set으로 분리하는 것을 권장합니다.

### 8.1 결합 우선순위 (참고)

같은 키에 여러 곳에서 정의가 있을 때 우선순위:

```
기본값(결합 가능) < 자판 최상위 key_meta < 활성 rule_set의 key_meta
```

같은 키의 같은 필드는 활성 rule_set이 덮어쓰고, 다른 필드면 합쳐집니다. 예:
- 자판 최상위에 `"/"` 키 `vowel_combine_head: true`
- rule_set이 `"/"` 키 `context_alt: {...}` 추가

→ 결과: `/` 키는 두 속성 모두 적용 (`vowel_combine_head: true` + `context_alt: {...}`).

---

## 9. 빌트인 자판 사례

### 9.1 ko_3bul390 / ko_3bul391

세벌식 390/391은 §6~§8에서 설명한 모든 기능을 사용합니다:

```
schema_version: 2
rule_sets:
  sun_arae_batchim    (active=false, 옛 자판 호환용 받침 단순화)
  sun_arae_moeum      (active=false, 옛 자판 호환용 모음 결합)  ← 390만
  vowel_strict        (active=true,  v/b 키 ㅗ·ㅜ 단순 모음)
  slash_context_alt   (active=true,  / 키 컨텍스트 분기)
```

기본 켜진 `vowel_strict` + `slash_context_alt` 조합으로 공병우 원본 규약에 가까운 동작을 재현합니다.

### 9.2 ko_2bulstd

두벌식은 `key_meta` 없이 단순합니다. v2 기능을 안 쓰지만 v2 코드에서도 그대로 작동합니다.

### 9.3 ko_3bul_noshift / ko_3bul_qwerty

세벌식의 다른 변종. `key_meta`를 안 써서 모든 ㅗ가 결합 가능한 느슨한 모드로 동작합니다.

---

## 10. FAQ

**Q. `schema_version: 2`를 꼭 적어야 하나요?**

아닙니다. `key_meta` 없이 v1 형태로 만들면 자동으로 v1로 인식됩니다. v2 기능을 쓸 때만 `schema_version: 2` 명시를 권장합니다 (가독성).

**Q. 기존 v1 자판이 v2에서도 작동하나요?**

네. 변경 없이 그대로 작동합니다. v2는 v1 위에 옵션 필드만 더한 것입니다.

**Q. 같은 키에 두 룰을 다 쓰면 어떻게 되나요?**

`vowel_combine_head`와 `context_alt`은 별개 의미라 함께 쓸 수 있습니다. 룰 B(`context_alt`)가 먼저 평가되고, 그 결과 자모 흐름이 계속되면 룰 A(`vowel_combine_head`)가 그 자모에 적용됩니다.

**Q. 두벌식에 `key_meta`를 추가해도 되나요?**

기술적으로는 가능하지만 두벌식은 같은 자모가 한 키에만 있어 룰 A의 의미가 약합니다. 룰 B(컨텍스트 분기)는 두벌식에서도 활용 가능합니다.

**Q. `to` 필드의 자모는 layout에도 있어야 하나요?**

네. `context_alt`이 조건 충족 시 `to`로 동작한다는 건, 정상 자모 흐름으로 진입한다는 뜻입니다. 그 자모가 자판 layout 어딘가에 매핑되어 있어야 합성됩니다.

**Q. `fallback`은 한글이어도 되나요?**

가능합니다. 다만 `fallback`은 그대로 commit되는 문자열이라 자모 합성 흐름을 거치지 않습니다. 한글을 적으면 그 한 글자가 그냥 출력됩니다.

**Q. 사용자가 자판 옵션을 GUI로 켜고 끌 수 있나요?**

네. UNIM 설정 다이얼로그(`unim-gui-gtk --settings`)의 "일반" 페이지에서 자판을 선택하면 그 자판이 정의한 rule_sets가 토글 스위치로 자동 노출됩니다. CLI(`unim-cli config set korean_active_rule_sets ...`)로도 가능합니다. 자세한 내용은 §5.2 참고.

---

## 11. 잘 안 될 때

### 11.1 자판이 인식 안 됨

```bash
# 자판 파일이 인식되는지 확인
unim-cli layouts list
```

`my_first` 같은 이름이 안 보이면:
- 파일 위치가 `~/.config/unim/layouts/` 인지 확인
- 파일 확장자가 `.json` 인지 확인
- JSON 문법 오류가 있는지 검사 (예: 마지막 콤마, 따옴표 누락)

### 11.2 자판은 인식되는데 입력이 안 됨

- `language` 필드가 `"korean"` 인지 확인
- `type` 필드가 `"2bul"` 또는 `"3bul"` 인지 확인 (영문 자판은 `"qwerty"` 등)
- `layout`의 `lower.3nd` 같은 행 이름이 정확한지 확인 (`3nd`는 오타 아님)

### 11.3 룰 A를 켰는데 합쳐짐 / 안 켰는데 안 합쳐짐

- `key_meta`의 키가 **영어 자판 char**인지 확인 (`"v"`, `"b"`, `"/"` 같은 글자)
- 그 키가 layout에서 어느 자모에 매핑되는지 확인
- `vowel_combine_head: false`만 결합 거부, 누락 또는 `true`는 결합 가능

### 11.4 룰 B의 `when` 조건이 안 먹힘

- `when` 값이 `"choseong_only"` 같은 snake_case인지 확인 (`"ChoseongOnly"` 아님)
- 9개 조건 표(§7.3) 중 정확한 이름인지 확인
- 영문 모드(한/영 전환 후)에서는 `context_alt`이 적용 안 됨

### 11.5 rule_set이 켜져 있는데 동작 안 함

- 자판의 최상위에 `active_rule_sets`가 있으면 그 목록에 이름이 들어 있는지 확인
- `active: true` + `active_rule_sets`에 이름 둘 다 있어야 활성

### 11.6 데몬이 자판을 로드 안 할 때

```bash
# 데몬 로그 확인
journalctl --user -u unim -n 50

# 또는 데몬 재시작
systemctl --user restart unim
```

JSON 파싱 에러나 v0 스키마 거부 메시지가 보이면 메시지 내용대로 수정.

---

## 12. 더 알고 싶다면

- 개발자 문서 (코드 진입점, 큐 메타 추적 흐름): [LAYOUT_PROFILE_V2.md](../../dev/plans/LAYOUT_PROFILE_V2.md)
- v1 베이스 (자판 JSON 구조 전반): [LAYOUT_PROFILE_V1.md](../../dev/plans/LAYOUT_PROFILE_V1.md)
- 빌트인 자판 JSON 샘플:
  - 두벌식: `keymaps/ko_2bulstd.json`
  - 세벌식 390: `keymaps/ko_3bul390.json` (룰 A·B 활용 사례)
  - 세벌식 391: `keymaps/ko_3bul391.json`
  - 영문: `keymaps/en_qwerty.json`, `en_dvorak.json` 등
