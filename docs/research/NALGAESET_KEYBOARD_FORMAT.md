# 날개셋 한글 입력기 자판 정의 파일 포맷 조사

Date: 2026-04-21 (최초 작성) · 2026-04-21 (§3 실측 개정) · 2026-04-21 (§3.5·§4.1·§4.2·§5.1·§5.2·§8 매뉴얼 기반 확장)
Scope: UNIM에서 사용자 정의 자판을 지원하거나 날개셋 자판을 불러오는 기능을 고려하기 위한 사전 조사.
Status:
- §3 (포맷) — 실제 `.ist` 파일 2종의 헥스 덤프 + 오픈소스 변환기 코드 분석에 근거.
- §3.5 (`.key` vs `.ist` 범위), §4.1 (날개셋문자 64-bit), §4.2 (글쇠 수식), §5.1 (초·종성 공유 결합), §5.2 (복벌식), §8 (제어판/편집기 운용) — mearie.org 날개셋 9.0 매뉴얼 사본의 검색 엔진 스니펫으로 간접 확인된 원문 기반. WebFetch는 403.
- **확정되지 않은 영역**: XML `.ist`의 `<KeyTable>` 외 태그(모디파이어·오토마타·수식), 바이너리 `.ist` 각 필드의 의미. 실제 구현 전 실 샘플 추가 확보 필수.

---

## 1. 조사 배경

UNIM은 현재 두벌식 표준과 세벌식 390/최종/순아래 4종을 **컴파일 시점에 하드코딩된 JSON**으로만 지원한다 (`src/keystroke/keymap/*.json`, `src/config.rs:50-105`). 사용자가 자판 배열을 직접 추가하거나 수정하려면 소스 수정·재컴파일이 필요하다.

반면 Windows용 날개셋 한글 입력기(김용묵 제작)는 사용자가 자판·오토마타·낱자 처리 규칙을 **설정 파일로 공유**할 수 있는 것이 핵심 강점이다. UNIM의 사용자 정의 자판 지원을 위해 날개셋이 어떤 파일 포맷과 데이터 모델을 채택했는지를 정리해 둔다.

- UNIM 기존 자판 로딩 경로: `src/keystroke/mod.rs:17,64` (`get_keymap_json` → `KeyboardMap::create_keyboard_map_from_str`)
- 자판 열거형: `src/config.rs:50-105` (`KoreanLayout`)
- 조합 규칙: `src/hangul/composer_with_2bul.rs:19-44`, `src/hangul/composer_with_3bul.rs:19-74`

---

## 2. 날개셋 설정 파일 확장자와 용도

날개셋의 설정은 크게 아래 3계층으로 분리되고, 각각 서로 다른 파일 단위로 저장·공유된다.

| 확장자 | 층위 | 담는 내용 |
|-------|------|----------|
| `.key` | 글쇠배열 (Keyboard Layout) | 47개 글쇠 × shift/비shift 조합의 **글쇠값 테이블**만 담음. 예: QWERTY `q` 위치에 `ㅂ`. |
| `.ist` | 입력기 유형 (Input-Scheme + Type) | 글쇠배열에 더해 **글쇠 인식 옵션, 입력 일반, 낱자 처리, 오토마타**까지 한 묶음. 자판 하나를 온전히 공유할 수 있는 단위. |
| `.hst` | 종합 설정 (Huge Setting) | 여러 입력기 유형 + 제어판 전역 설정 + 한자·상용구 사전까지 포함하는 최상위 묶음. |
| `.set` | (구) 단일 설정 | 4.x 이전의 통합 설정 파일. 5.0 이후로는 위 3개로 분해됨. |

**권장 공유 단위**: 자판만 공유할 땐 `.ist`, 사용 환경 전체를 복제할 땐 `.hst`.

---

## 3. 파일 포맷 — 이중 포맷 (바이너리 기본 + XML 수동 내보내기)

**실제 `.ist` 파일을 내려받아 분석한 결과 (2026-04-21)** 기존 자료의 "5.0부터 XML" 기술은 부정확하다. 정확한 상황은 아래와 같다.

### 3.1 기본 포맷 — MFC 스타일 바이너리

분석 대상: [youknowone/3finalnoshift](https://github.com/youknowone/3finalnoshift) 저장소의 2008년 `.ist` 두 종 (`세벌식 최종 순아래 정석.ist` 887 B, `세벌식 최종 순아래 호환.ist` 870 B).

**헥스 덤프 (앞부분)**:

```
000000  4e 67 73 49 4d 45 20 73 65 74 20 76 35 2e 30 1a   NgsIME set v5.0.
000010  11 43 42 61 73 69 63 49 6e 70 75 74 53 63 68 65   .CBasicInputSche
000020  6d 65 01 01 01 02 8a 04 0d 38 c1 8c bc dd c2 20   me.......8.....
...
000230  0f 00 00 07 43 4e 67 73 49 6d 65 80 40 01 af 02   ....CNgsIme.@...
```

구조:

| 구간 | 바이트 | 내용 |
|------|--------|------|
| 0x00 – 0x0E | `NgsIME set v5.0` | ASCII 매직 헤더 (15 B, 버전 문자열 포함) |
| 0x0F | `0x1A` | DOS EOF (SUB). `type` 명령에 비이진 검사로 쓰이는 고전적 트릭 |
| 0x10 | `0x11` (=17) | 다음 Pascal 문자열 길이 |
| 0x11 – 0x21 | `CBasicInputScheme` | 클래스 태그 (MFC `CRuntimeClass` 이름) |
| 0x22 – 0x232 | 바이너리 | **입력 스키마** 직렬화 데이터 (글쇠표, 낱자 결합 등) |
| 0x233 | `0x07` (=7) | 다음 Pascal 문자열 길이 |
| 0x234 – 0x23A | `CNgsIme` | 클래스 태그 — 문자 생성기(한글 오토마타) |
| 0x23B – EOF | 바이너리 | **오토마타** 상태 전이표 |

즉 `.ist`는 두 개의 C++ 객체를 연이어 **MFC `CArchive` 포맷으로 serialize**한 파일이다. 각 객체 앞에 길이 접두 Pascal 문자열(`BYTE len` + ASCII)로 클래스 이름을 두는 것은 MFC의 `CArchive::ReadClass` / `WriteClass` 고유 관례다. 이후 내용은 각 클래스의 `Serialize(CArchive&)` 구현에 종속적이므로, **공개 스펙 없이는 날개셋 소스(비공개) 참조가 필수**다.

**내부 낱자 인코딩**: 데이터 블록을 파이썬 스크립트로 UTF-16LE 스캔하면 Jamo 영역(U+1100–U+11FF)의 문자들이 등장 — 예: offset `0x5D`에 `ᄀ`(U+1100), `0x1BB`에 `ᄒ`(U+1112). 즉 낱자 테이블은 **조합형 자모(U+1100 블록)** 기반이다. UNIM의 `src/hangul/jamo.rs`와 동일한 Unicode 범위.

### 3.2 부가 포맷 — XML (수동 내보내기 전용)

[xnuk/ngconverter](https://github.com/xnuk/ngconverter/blob/master/index.js#L241) 소스 분석 결과, 날개셋 **편집기는 XML로 "다른 이름으로 저장"하는 옵션을 별도로 제공**한다. ngconverter의 README도 명시:

> "일단 ist 파일을 날개셋에 불러온 다음 XML 파일로 저장합니다."

즉 기본 배포되는 `.ist`는 바이너리이고, XML은 **사람이 편집하고 싶은 사용자가 수동으로 내보낼 때만 생성**된다. 이 때문에 공개된 XML 샘플 수가 매우 적다.

### 3.3 XML 스키마 (ngconverter가 읽는 XPath 기준)

ngconverter는 XML 파일에서 오직 한 XPath만 재작성한다:

```xpath
/InputEntry/InputSchemeSetting/KeyTable/Key/@at
```

역산한 최소 XML 골격:

```xml
<InputEntry>
  <InputSchemeSetting>
    <KeyTable>
      <Key at="0x61" .../>   <!-- 'a' (0x61) 위치의 매핑 -->
      <Key at="0x62" .../>
      ...
    </KeyTable>
  </InputSchemeSetting>
</InputEntry>
```

- 루트: `<InputEntry>` — 입력기 유형 전체 (바이너리의 `CBasicInputScheme` + `CNgsIme` 묶음)
- `<InputSchemeSetting>` — 입력 스키마 계층
- `<KeyTable>/<Key>` — 글쇠별 매핑. `@at` 속성은 **가상 키코드의 16진 문자열**(예: `0x61` = `a` 키)
- 나머지 속성(매핑 대상, shift 변형, 낱자 결합 규칙 등)은 ngconverter가 건드리지 않아 추가 샘플 없이는 확정 불가

**공개된 공식 DTD/XSD는 없다**. 태그 이름은 편집기 내부 상수에서 파생되며, 4.81의 상수 명칭 호환성이 5.x 이후로 유지된다고 자료에 언급된 정도다. 자동 파서를 쓰려면 실 샘플 다수 확보 후 역설계가 필요하다.

### 3.4 요약: UNIM이 읽어야 할 포맷

| 포맷 | 실용성 | 난이도 | 권장 |
|------|-------|-------|------|
| 바이너리 `.ist` (MFC) | 공개 .ist 대다수가 이 형식 | 높음 (비공개 스펙, 리버스 엔지니어링 필요) | ❌ |
| XML `.ist` (수동 저장) | 사용자가 편집기에서 내보낸 경우만 | 중간 (스키마 역설계) | ⚠️ 부분 지원 |
| `.key` (글쇠배열만) | 글쇠 매핑에 한정, 파일 크기 < 1 KB | 낮을 것으로 추정 | ✅ 우선 대상 |

현실적 결론: **UNIM이 날개셋 파일을 "직접 파싱"하는 것은 비용이 매우 높다**. 대신 (a) 사용자가 XML로 내보낸 파일을 받도록 안내하고, (b) `KeyTable/Key` 수준만 부분 지원하는 수입기를 제공하는 것이 현실적이다.

---

### 3.5 `.key` vs `.ist` 포함 범위 확정

mearie.org `ngsdoc` 본문(검색 스니펫으로 확인)에 따르면 확장자 간 포함 범위는 **엄격한 포함 관계**다.

| 파일 | 글쇠배열 (47 키) | 글쇠 인식 | 입력 일반 | 낱자 처리 | 오토마타 | 한자/상용구 |
|------|:---------------:|:--------:|:--------:|:--------:|:-------:|:---------:|
| `.key` | ✅ | ❌ | ❌ | ❌ | ❌ | ❌ |
| `.ist` | ✅ | ✅ | ✅ | ✅ | ✅ | ❌ |
| `.hst` | ✅ (다중) | ✅ | ✅ | ✅ | ✅ | ✅ |

따라서 **`.key` 파일만 해석하면 UNIM 현재 JSON과 거의 동형**이다. 낱자 처리·오토마타를 건드리지 않고 키 사상만 바꾸는 "단순 배열 변형"(콜맥 한글, 드보락 한글 등)은 `.key`만으로 충분히 표현된다.

원문(mearie): *"낱자 처리나 오토마타 등은 key 파일에 들어가 있지 않으므로, 기본값과 다르게 쓰고 싶을 때에는 따로 지정해주어야 합니다."*

---

## 4. 입력 스키마 — 2×3 계층 모델

날개셋의 가장 독창적인 설계는 자판을 "글쇠 배열"로 단순화하지 않고, **입력 스키마**와 **문자 생성기**의 **2계층**으로 나누고 각 계층에 **빈 / 기본 / 고급**의 **3등급**을 부여한 것이다.

```
    [입력 스키마]            [문자 생성기]
    (키→날개셋문자)         (날개셋문자→조합/문자열)
     빈 Empty                 빈 Empty
     기본 Basic               기본 Basic (한글 오토마타)
     고급 Advanced            고급 Advanced (스크립트)
```

- **입력 스키마**: 물리 글쇠 입력을 "날개셋문자"라는 내부 토큰으로 사상한다.
  - *빈*: 아무 가공 없이 통과. (영문용)
  - *기본*: 고정된 날개셋문자를 반환. 대부분의 자판은 이 등급.
  - *고급*: 상황(앞 상태, 모디파이어)에 따라 서로 다른 날개셋문자를 되돌리는 **수식**으로 표현.
- **문자 생성기**: 날개셋문자 시퀀스를 받아 최종 조합 문자열로 만든다.
  - *빈*: 날개셋문자를 그대로 UTF-16으로 내보냄.
  - *기본*: **한글 오토마타**를 돌려 초·중·종성을 조합.
  - *고급*: 옛한글·한자·사용자 스크립트를 허용.

이 2×3=최대 9조합 중 실제 자판은 대체로 *(기본/기본)* 또는 *(고급/기본)* 정도에 머문다. **UNIM의 JSON 포맷은 이 중 "입력 스키마 기본 × 문자 생성기 기본"의 정적 테이블 형태에만 해당**한다.

### 4.1 "날개셋문자"(Ngs Char) 내부 표현 — 64 bit 토큰

mearie `biu.htm` 본문에 따르면 날개셋문자는 **64-bit 단일 토큰**이다. 레이아웃:

```
 63           48 47                                       0
┌──────────────┬─────────────────────────────────────────┐
│  type tag    │     payload (데이터 종류별)               │
│  (상위 16 bit) │                                          │
└──────────────┴─────────────────────────────────────────┘
```

**Payload 해석 (문서 원문 요약)**:
- **일반 문자**: UTF-32 한 코드포인트.
- **한글 낱자**: 초·중·종성 각 **16 bit**씩(총 48 bit). 즉 `(cho, jung, jong)` 트리플을 한 토큰에 욱여넣을 수 있다. 한 토큰이 **"초+중"이나 "종+종"** 같은 복합 낱자도 표현 가능(두세 낱자를 하나의 날개셋문자로 포장).
- **"같은 낱자라도 어떤 글쇠를 눌러 입력했느냐에 따라 내부적으로 여러 다른 값을 배당하는 것이 가능"** — 같은 `ㄱ`이라도 A키에서 온 것과 Q키에서 온 것을 구별 가능. 이로써 오토마타의 분기 폭이 Unicode 코드포인트보다 훨씬 넓다.

**UNIM 대비**: UNIM의 Jamo 표현은 `Cho`/`Jung`/`Jong` 3개의 `#[repr(u8)]` 열거형(`src/hangul/jamo.rs`)으로 각각 독립 저장. 즉 **키 출처(provenance)를 구별할 수 없는 설계**. 복벌식 자동 감지처럼 "어떤 키였는지"를 오토마타에서 참조해야 하는 기능은 현재 구조에서 불가능하다.

### 4.2 글쇠 수식 — 고급 입력 스키마의 심장

고급 입력 스키마는 **C 언어 스타일 수식**으로 키 반응을 정의한다 (위키백과·나무위키 공통 기술).

- 수식은 글쇠를 누를 때 평가되어 **다음 중 하나**를 반환:
  1. 날개셋문자 (한글 낱자 or 일반 유니코드 문자)
  2. **오토마타 상태 전이** 지시자
  3. **"비문자" 제어 지시자**(커밋/롤백/시프트 상태 변경 등)
- 변수 `T`: 현재 오토마타 상태(`T == 0`은 "조합 중 아님", `T != 0`은 조합 진행 중). 수식 내부에서 `T`를 읽어 분기.
- **적응형 글쇠 설계 사례**(mearie 문서):
  - 영문 키보드의 `CapsLock` 영향
  - 한글 키보드에서 초성/종성이 조합 상태에 따라 다른 글쇠에 배정
  - 두벌식 `/` 키: 직전에 중성이 없으면 `ㅗ` 출력, 있으면 `/` 출력
  - 세벌식의 NumLock 키 처리
  - **왼손으로 시작하면 두벌식, 오른손이면 세벌식**(복벌식의 기반 로직)

이는 단순 테이블이 아닌 **키 → (상태, 문맥) → 결과의 함수**를 `.ist`가 직접 담아야 한다는 뜻이다. UNIM JSON 표현력으로는 도달 불가.

---

## 5. 오토마타 4단계 파이프라인

`.ist`가 담는 설정은 아래 순서로 입력을 처리한다.

1. **글쇠 인식 (Key Recognition)**
   스캔코드·가상키·모디파이어를 읽어 날개셋문자로 정규화. CapsLock·한/영키·좌우 Shift 구분·Dead key까지 여기서 처리.
2. **입력 일반 (General Input)**
   BackSpace 단위(낱자/음절), 일정 시간 무입력 시 상태 리셋·문자 자동 커밋, 마우스 클릭·포커스 이동 시 조합 처리 같은 메타 규칙.
3. **낱자 처리 (Jamo / Character Combination)**
   한 타로 바로 입력 못 하는 복잡한 낱자를 **낱자 결합 규칙**으로 합성. 예: `ㄹ+ㄱ → ㄺ`, `ㅗ+ㅏ → ㅘ`, 옛한글 방점·이중 모음 등. UNIM의 `JUNG_COMBINATIONS`/`JONG_COMBINATIONS`/`CHO_COMBINATIONS`가 바로 이 단계에 해당한다.
4. **오토마타 (Automaton)**
   상태 머신. 상태 집합 × (현재 낱자 → 다음 상태, 조합 커밋/롤백 동작)을 사용자 정의 가능. 두벌식은 〈비어있음 / 초 / 초중 / 초중종〉 4상태, 세벌식은 더 단순한 2~3상태, 옛한글은 10+ 상태까지 확장된다.

UNIM은 2·4단계가 `HangulComposer` 트레이트 내부에 하드코딩되어 있고 (`src/hangul/composer.rs`), 3단계만 레이아웃별 `const` 배열로 노출된다. 즉 **데이터 주도(data-driven) 오토마타가 아니라 코드 주도**다.

### 5.1 초·종성 공유 낱자 결합 규칙 (Cho/Jong shared combination)

mearie `arc_mixshrunit.htm` 본문:

> "초성과 종성은 동일한 낱자 결합 규칙을 공유하며, 더 정확히 말하면 종성도 초성의 낱자 결합 규칙을 그대로 사용한다. 쌍자음 받침에서 각 받침을 조합하는 데 초성에서 정의해 놓은 결합 규칙을 중첩 적용할 수 있다. 초성과 종성에서 중복되는 규칙을 매번 정의하지 않아도 되므로 입력 방식을 효율적으로 구성할 수 있다."

예:
- 초성 규칙: `ㄹ + 가획 → ㄹㅋ`, `ㄹ + 쌍자음 → ㄹㄲ`
- 종성 규칙(상속): `ㄹ + ㄱ → ㄺ`, 그리고 위 초성 규칙을 중첩 적용해 `ㄺ + 가획 → ㄹㅋ`까지 자동 도출.

**UNIM 대비**: `src/hangul/composer_with_3bul.rs`의 `CHO_COMBINATIONS`와 `JONG_COMBINATIONS`는 **완전히 별개의 테이블**이다. 공유 계층이 없어 같은 개념(가획/쌍자음)을 두 곳에 중복 정의해야 한다. 날개셋 `.ist` → UNIM 변환 시 규칙이 **물리적으로 펼쳐져 양쪽에 복사**되는 형태로 손실 없이 이식 가능하다(단, 중복 증가).

### 5.2 복벌식(複벌式) 자동 감지의 실제 원리

mearie `qsetup_bok.htm`·나무위키의 진술을 종합:

- 두벌식 초성: **왼손**(Q W E R T, A S D F G)에 집중
- 세벌식 초성: **오른손**(Y U I O P, H J K L)에 집중
- 어절(word) 첫 타자의 **손 위치**로 그 어절에 적용할 배열을 결정.
- 어절 경계(스페이스/문장부호/Enter) 마다 리셋.
- 장점: 한 키보드에서 한영 전환 없이 두 배열 공유. 두 사용자가 같은 PC 공유 시 유용.

구현에 필요한 것: (a) 어절 단위 버퍼, (b) 첫 낱자의 키 출처 기록(→ 날개셋문자의 "키 출처 구분 가능" 특성 활용), (c) 중간 커밋/롤백 로직. UNIM으로 이식하려면 `InputEngine`에 어절 스코프의 컨텍스트와 "키 출처가 태깅된 낱자" 표현이 추가되어야 한다.

---

## 6. 특수 기능들과 파일 포맷의 연관

| 기능 | `.ist`에 담기는 규칙 | UNIM 대응 |
|------|---------------------|----------|
| **모아주기 (Stroke Replay)** | 오토마타 단계에서 "타자 순서가 틀리면 재배열" 규칙을 상태 전이로 기술. 세벌식에서 `중성→초성` 순으로 쳐도 음절이 깨지지 않게 정렬. | 미구현. `docs/history/planning/01_researcher_findings.md:133-139`에 참고 항목으로 기록됨. |
| **복벌식 (Dual-Mode 자동 인식)** | 하나의 `.ist`에 두벌식·세벌식 규칙을 병렬로 두고, 입력 시퀀스 패턴으로 동적 선택. | 미구현. UNIM은 `KoreanLayout` 단일 선택. |
| **옛한글** | 낱자 처리 단계에서 U+1100 확장 블록과 방점·합용병서 규칙 추가. 문자 생성기는 고급. | 미구현. Jamo 테이블 자체가 현대 한글만 커버 (`src/hangul/jamo.rs`). |
| **한자·상용구** | 별도 사전 파일 + `.hst`의 인덱스. | 부분 구현. `src/data/hanja.txt`의 한자 사전은 존재하나 `.hst` 같은 번들 개념 없음. |

---

## 7. UNIM 도입 시 고려 사항

### 7.1 포맷 선택지

1. **날개셋 바이너리 `.ist`를 직접 파싱** — ❌ 비권장
   - 장점: 인터넷에 공개된 `.ist` 자산(3-2015, 3-2015P, 순아래 변형들) 즉시 활용 가능성.
   - 단점: **MFC `CArchive` 바이너리**. 공개 스펙 없음. `CBasicInputScheme`·`CNgsIme`의 `Serialize(CArchive&)` 구현을 리버스 엔지니어링해야 하는데, 날개셋은 폐쇄 소스 Windows 바이너리.
   - 위험: 버전(4.x/5.x/6~10.x) 간 바이너리 레이아웃이 조용히 바뀌어 왔을 가능성이 매우 크며(매직이 `v5.0` 그대로인 8~10.x 샘플 확보 불가), 테스트 표본 자체를 모으기 어려움.

2. **날개셋 XML `.ist`만 지원** — ⚠️ 제한적 권장
   - 장점: 텍스트 기반. `quick-xml` 등 기존 러스트 크레이트로 파싱 가능.
   - 단점: 사용자가 **편집기에서 "XML 저장"을 수동 실행**해야 얻을 수 있는 포맷. 커뮤니티 배포 파일 대부분은 바이너리라 UX 가이드가 필수.
   - 스키마: `/InputEntry/InputSchemeSetting/KeyTable/Key[@at]`까지는 ngconverter로 확인됨. 그 외 자판/오토마타 관련 태그는 샘플 없이 확정 불가.

3. **UNIM 자체 포맷(YAML/TOML) 설계 + XML `.ist` 부분 수입기** — ✅ 권장
   - 장점: 이미 쓰는 `config.yaml`과 일관. `serde` 재사용.
   - 수입기 범위 제한: "입력 스키마 기본 × 문자 생성기 기본" 부분집합만. `<Key @at>` 테이블 → UNIM `{upper/lower × 1st~4th}` 사상.
   - 사용자 UX: XML 저장 경로를 안내하는 문서 + 변환 실패 시 부분 결과 + 경고.

4. **런타임 JSON 로드** — ✅ 최단 경로
   - 현재 `include_str!`로 임베드된 JSON을 `std::fs::read_to_string` 경로로 돌리면 됨.
   - `~/.config/unim/layouts/*.json` 스캔만 추가. 구조 변경 거의 없음.
   - 한계: 자판 표현력을 현재 JSON 수준(shift/비shift × 3~4행 × 글쇠 문자)으로 고정. **날개셋 호환은 목표로 하지 않음**.

### 7.2 권장 단계

1. 기존 4종 내장 자판은 유지하고, `~/.config/unim/layouts/*.json` 사용자 디렉터리만 추가 스캔. (최소 변경, 위 4번 안)
2. 낱자 결합 규칙(`JUNG/JONG/CHO_COMBINATIONS`)을 JSON의 선택 필드로 이관 → **3벌식 변종 자판** 지원 가능.
3. (선택) `.ist`(XML) 부분 수입기를 별도 바이너리(`unim-import-nalgaeset`)로 제공. 다음 규칙:
   - 입력은 `.ist`의 XML 저장본만 허용. 바이너리 `.ist`를 받으면 "편집기에서 XML로 다시 저장해 주세요" 안내만 출력.
   - `/InputEntry/InputSchemeSetting/KeyTable/Key` 요소만 해석. 그 외 태그(수식·스크립트·사용자 정의 조합)는 만나면 경고 + 무시.
   - 결과는 UNIM JSON 자판 파일로 덤프, 바로 `~/.config/unim/layouts/`에 떨어뜨림.
4. 오토마타 상태 머신을 데이터 주도로 전환하는 작업은 **별도 로드맵 항목**으로 분리. 현재 `HangulComposer` 트레이트 전면 재설계가 필요.

### 7.3 비목표

- 고급 입력 스키마(수식), 고급 문자 생성기(스크립트), 옛한글 풀스펙은 이번 범위에서 **명시적 비목표**로 둔다. 날개셋은 Windows + C++로 30년 누적된 엔진이며, 해당 기능 이식은 엔진 재작성 수준의 작업이다.

---

## 8. 날개셋 편집기 / 제어판 운용 (매뉴얼 기반)

mearie.org 9.0 문서 (검색 스니펫으로 간접 확인)와 pat.im 블로그(`https://pat.im/949`)를 종합한 실무 흐름. **XML 내보내기·`.ist` 불러오기 경로**가 UNIM 수입기의 UX 기준점이 된다.

### 8.1 제어판 구조

```
날개셋 제어판
└ 입력기 계층 (input hierarchy)       ← 최상위 (최대 N개의 "입력 항목")
  ├ 0번 입력 항목 (기본)
  │   ├ 빠른설정 ▼ (기본 글자판 / 복벌식 / 신세벌식 …)
  │   ├ 자판 배열 이름
  │   │   ├ 글쇠배열   ← .key 불러오기 지점
  │   │   ├ 글쇠 인식 옵션
  │   │   ├ 입력 일반
  │   │   ├ 낱자 처리
  │   │   └ 오토마타
  │   └ 문자 생성기 (빈/기본/고급)
  ├ 1번 입력 항목
  ├ 2번 입력 항목 (예: 구결자 상용구)
  └ …
```

- 사용자는 **핫키**(기본 `우측 Shift`)로 입력 항목 0↔1 순환.
- `.ist` 불러오기는 "자판 배열 이름"의 **루트** 우측 "외부 파일 불러오기"로 수행 (한 `.ist`가 하나의 입력 항목 전체를 덮어씀).
- `.key`는 하위 "글쇠배열" 노드에서만 불러오기 가능. 나머지 항목(낱자 처리·오토마타)은 기본값 유지.

### 8.2 XML 내보내기 흐름 (ngconverter 매뉴얼 기반)

1. 날개셋 편집기를 열어 대상 `.ist`(바이너리) 불러오기.
2. `파일 → 다른 이름으로 저장`에서 **파일 형식**을 `XML`로 선택.
3. `.xml` 또는 `.ist`(XML 모드) 확장자로 저장.
4. (UNIM 수입기 입력) 이 XML을 `unim-import-nalgaeset <file.xml>`에 전달.
5. `~/.config/unim/layouts/<이름>.json` 생성.

> 사용자 입장에서 이 흐름은 "Wine에서 날개셋 편집기 구동 → XML 저장 → Linux에서 수입"이 된다. **Wine 의존은 UX 큰 걸림돌**이지만, MFC 바이너리 리버싱을 피하는 유일한 안전 경로.

### 8.3 텍스트 필터 (부가 기능)

날개셋 편집기는 `편집 → 텍스트 필터 (F4)`로 **이미 입력된 텍스트에 IME 알고리즘을 재적용**할 수 있다. 이는 (a) 잘못 조합된 한글 재조합, (b) 자판 변환(두벌 → 세벌), (c) 한자 일괄 변환 등에 쓰인다.

UNIM에 대응할 기능이 없으며, 본 범위에서는 비목표. 다만 **AutoTypeFix의 후방 재조합(delete_surrounding_text + commit)은 개념적으로 이 필터와 유사**하다는 점이 추후 통합 시 재사용 가능하다.

---

## 9. 참고 자료

### 9.1 실측 자료 (이 문서의 1차 근거)

- [youknowone/3finalnoshift](https://github.com/youknowone/3finalnoshift) — 2008년 배포 `.ist` 2종.
  - `세벌식 최종 순아래 정석.ist` (887 B), `세벌식 최종 순아래 호환.ist` (870 B)
  - 매직 `NgsIME set v5.0`, MFC Pascal-string 클래스 태그 `CBasicInputScheme`(0x11) → `CNgsIme`(0x234) 순.
  - 낱자 데이터는 UTF-16LE Unicode Jamo(U+1100–U+11FF).
- [xnuk/ngconverter/index.js:241](https://github.com/xnuk/ngconverter/blob/master/index.js#L241) — XML `.ist`의 XPath 레이아웃 확인.
  - `/InputEntry/InputSchemeSetting/KeyTable/Key/@at`
- [xnuk/ngconverter/README.md](https://github.com/xnuk/ngconverter/blob/master/README.md) — "ist 파일을 날개셋에 불러온 다음 XML 파일로 저장합니다." — XML은 수동 저장 경로임을 명시.

### 9.2 2차 자료 (배경 설명)

mearie.org `/f/ngsdoc/`는 **날개셋 9.0 매뉴얼 사본**(cosmic.mearie.org는 이 사이트 관리자가 CHM을 HTML로 풀어서 호스팅한 아카이브). 이 문서의 §3.5/§4/§5 대부분은 본 아카이브의 검색 엔진 스니펫에서 역추출한 원문 인용에 근거한다(WebFetch는 403).

- [날개셋 한글 입력기 9.0 문서 — 인덱스](https://cosmic.mearie.org/f/ngsdoc/) / [목차 Index.hhk](https://cosmic.mearie.org/f/ngsdoc/Index.hhk.html) / [트리 Ngs.hhc](https://cosmic.mearie.org/f/ngsdoc/Ngs.hhc.html)
- [입력 스키마 — `arc_ischeme.htm`](https://cosmic.mearie.org/f/ngsdoc/arc_ischeme.htm) — 2계층 × 3등급, 47 글쇠/94 위치, 적응형 글쇠 예(두벌식 `/` 키 등).
- [날개셋문자 — `biu.htm`](https://cosmic.mearie.org/f/ngsdoc/biu.htm) — 64-bit 내부 토큰 구조(상위 16 bit = type, 한글 초/중/종 16 bit씩).
- [초·종성 공유 낱자 결합 규칙 — `arc_mixshrunit.htm`](https://cosmic.mearie.org/f/ngsdoc/arc_mixshrunit.htm) — 초성 규칙을 종성이 중첩 적용.
- [기본 글자판 설정 — `basic_qsetup.htm`](https://cosmic.mearie.org/f/ngsdoc/basic_qsetup.htm) — 빠른설정 도우미 및 `.key` 포함 범위 명세.
- [복벌식 빠른설정 — `qsetup_bok.htm`](https://cosmic.mearie.org/f/ngsdoc/qsetup_bok.htm) — 좌·우손 기반 자판 자동 선택.
- [신세벌식 빠른설정 — `qsetup_neo3.htm`](https://cosmic.mearie.org/f/ngsdoc/qsetup_neo3.htm).
- [날개셋 편집기 — `ngeindex.htm`](https://cosmic.mearie.org/f/ngsdoc/ngeindex.htm) / [기본적인 사용법 — `nge_basic.htm`](https://cosmic.mearie.org/f/ngsdoc/nge_basic.htm) — 편집기 UI·텍스트 필터(F4).
- [외부 모듈 TSF/IMM 인덱스 — `tsfindex.htm`](https://cosmic.mearie.org/f/ngsdoc/tsfindex.htm).
- [변환기 인덱스 — `ncvindex.htm`](https://cosmic.mearie.org/f/ngsdoc/ncvindex.htm).
- [날개셋 한글 입력기 — 나무위키](https://namu.wiki/w/%EB%82%A0%EA%B0%9C%EC%85%8B%20%ED%95%9C%EA%B8%80%20%EC%9E%85%EB%A0%A5%EA%B8%B0) — 버전별 변경점·복벌식·모아주기·옛한글·텍스트 필터 배경.
- [날개셋 한글 입력기 — 위키백과](https://ko.wikipedia.org/wiki/%EB%82%A0%EA%B0%9C%EC%85%8B_%ED%95%9C%EA%B8%80_%EC%9E%85%EB%A0%A5%EA%B8%B0) — 2계층 모델 원문.
- [.ist / .key 설정 파일 불러오기 — pat.im 블로그](https://pat.im/949) — 불러오기 메뉴 경로.
- [김용묵 — 날개셋 전반적인 특징](http://moogi.new21.org/ngs_menu1.htm) / [10.7 다운로드](http://moogi.new21.org/prg4.html) — 제작자 공식 페이지(sandbox에서 직접 fetch 불가, 브라우저 권장).
- [세벌식 팁모음 — 날개셋 자판 적용하기](https://m.cafe.daum.net/3bulsik/JOK2/119) — `.ist` 공유 커뮤니티 실사용 예.
- [MFC `CArchive` — Microsoft Learn](https://learn.microsoft.com/en-us/cpp/mfc/reference/carchive-class) — 바이너리 `.ist`의 `Serialize` 직렬화 관례.

### 9.3 UNIM 내부 참조

- `src/config.rs:50-105` — `KoreanLayout` 열거형.
- `src/keystroke/mod.rs:17,64` — 자판 JSON 로더.
- `src/keystroke/keymap/*.json` — 현재 4종 자판.
- `src/hangul/composer_with_2bul.rs:19-44`, `src/hangul/composer_with_3bul.rs:19-74` — 낱자 결합 규칙.
- `src/hangul/jamo.rs` — Jamo 열거형 (U+1100/U+3130 이중 매핑).
- `docs/history/planning/01_researcher_findings.md:133-139` — 모아주기·복벌식 기존 조사 기록.
