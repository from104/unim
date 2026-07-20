# UNIM 자판 프로필 v3 스키마 (모아치기 + 옛한글 거부)

> **v2 베이스**: [`docs/archive/plans/LAYOUT_PROFILE_V2.md`](../../archive/plans/LAYOUT_PROFILE_V2.md). 본 문서는 v2 위에 v3가 추가한 분만 다룬다. v1/v2의 `combinations`/`rule_sets`/`active_rule_sets`/`inherits`/`key_meta` 의미는 그 두 문서 그대로.

Date: 2026-05-05 (UNIM 0.3.0 릴리즈)
구현 범위: schema · loader · composer(3bul 통합) · input_engine(chord_buffer) · unim-dbus(ChordIdleFlush) · 사용자 config(`korean.bidirectional_combine` / `korean.chord_window_ms`) · GTK Moachigi group · ko_3bul_anmatae 빌트인
관련 브랜치: `feat/anmatae-moachigi` (commits `8b79a98 → aff0ff1`, 단일 PR로 develop 머지 예정)

---

## 0. 한 눈에 — v2 대비 무엇이 바뀌었나

| 항목 | v2 (0.2.0) | v3 (0.3.0+) |
|---|---|---|
| `schema_version` 값 | `1` 또는 `2` | `1`, `2`, **`3`** |
| 모아치기 (chord) 지원 | 비목표 (v2 §12) | **구현**: `chord_buffer` + `bidirectional_combine` + `chord_window_ms` |
| 모아치기 capability 마커 | — | **신설**: `supports_moachigi: bool` (top-level) |
| 모아치기 옵션 저장 위치 | — | **사용자 config 일원화** (`~/.config/unim/config.yaml`의 `korean.*`). 키맵 JSON에는 옵션 값 없음. |
| 옛한글(古韓글) 자모 | 별도 정책 없음 (사실상 무시) | **명시적 거부** — loader가 `LoadError::ArchaicJamoNotSupported`로 reject |
| 동시 입력(chord) 디폴트 | — | **OFF (opt-in)**. 사용자가 명시 활성화 필요. |
| 음절 경계 알고리즘 | 영역 외 jamo 시퀀스 의존 | **시간 기반 chord** + 영역 채움 (chord 활성 시) |
| 별도 `HangulComposer3BulMoachigi` | (미존재) | **존재했다 폐기** — `HangulComposer3Bul` + `chord_buffer` 통합 |
| 빌트인 한국어 자판 | 5종 (10 built-ins 중) | **6종** — `ko_3bul_anmatae` 추가 |

v2 §12 비목표(*chord/glide* · *time-based 조건*) 두 항목이 v3에서 데이터화되었다. 단, **chord 자체는 컴포저 외부**(`InputEngine::chord_buffer`)에서 처리한다 — composer trait 시그니처는 v2와 동일.

---

## 1. `schema_version: 3` 게이트

### 1.1 v2 → v3 판별

JSON에 `supports_moachigi: true` 마커가 존재하면 v3 의도. `schema_version` 필드는 호환을 위해 명시 권장이지만 **암묵 v3**도 허용 — `supports_moachigi`만 있고 `schema_version` 부재여도 파싱은 성공한다.

판별 로직 ([schema.rs:306, 315](../../../src/keystroke/profile/schema.rs)):

```rust
fn is_v3(&self) -> bool {
    self.schema_version == Some(3) || self.supports_moachigi
}
```

빌트인 ko_3bul_anmatae는 명시적으로:

```json
{
  "schema_version": 3,
  "language": "korean",
  "name": "ko_3bul_anmatae",
  "type": "3bul",
  "supports_moachigi": true,
  ...
}
```

### 1.2 v0/v1/v2 호환

- v0 (legacy): 0.2.0과 동일하게 `LoadError::UnsupportedSchema` 거부 (변경 없음).
- v1/v2: `supports_moachigi` 누락 = `false` 기본. v1/v2 자판은 v3 코드에서 그대로 작동, 모아치기 미지원으로 인식되어 GTK Moachigi 그룹이 표시되지 않는다.

---

## 2. `supports_moachigi` — capability 마커

### 2.1 의미

자판이 모아치기(chord 입력)를 **수용 가능**한지를 표시하는 boolean. 옵션 값이 아닌 **자판 정체성** 일부 (자판 디자이너가 결정).

- `true` → GTK 설정 다이얼로그가 Moachigi 그룹을 노출. 사용자 config의 `korean.bidirectional_combine` / `korean.chord_window_ms` 값이 적용된다.
- `false` (또는 누락) → Moachigi 그룹 숨김. chord 관련 사용자 설정은 무시된다 (silently ignored).

### 2.2 MoachigiSpec — capability 마커 객체

`MoachigiSpec`은 v3에서 `Option<MoachigiSpec>` 형태로만 의미를 갖는다 — `Some(_)` = capability 보유, `None` = 미보유. 내부 필드는 사용하지 않는다.

```rust
// schema.rs:39
/// `MoachigiSpec`은 "이 자판이 모아치기를 지원함"의 capability 마커 역할만 보유.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct MoachigiSpec {
    // 과거 키맵 필드 잔재 — Phase 7 이후 항상 false/0. 사용자 config가 권위.
    pub bidirectional_combine: bool,   // unused
    pub chord_window_ms: u16,           // unused
}
```

코드: [schema.rs:39-50](../../../src/keystroke/profile/schema.rs#L39-L50), [schema.rs:395-405](../../../src/keystroke/profile/schema.rs#L395-L405) (loader가 capability에만 따라 `Some/None` 결정).

### 2.3 키맵 JSON에 옵션 값 두지 않는 이유

Phase 7에서 결정. 이유:

1. **단일 권위 (Single Source of Truth)**: 옵션은 사용자 선호. 키맵에 있으면 두 위치(키맵 + 사용자 config)가 충돌 가능.
2. **자판 호환 안정성**: 사용자가 안마태 ↔ 향후 다른 모아치기 자판으로 전환해도 동일 사용자 옵션이 그대로 적용된다.
3. **기본 OFF (opt-in)**: 키맵에 디폴트를 박아두면 "안마태=항상 chord"가 강제된다. 입력 환경(터미널·게임 등)에 따라 OFF가 더 안전.
4. **의도 명시**: 사용자가 GTK 설정에서 한 번 켜야 동작 — 우연한 ghost 입력 방지.

---

## 3. 사용자 config 키 (`korean.*`)

모아치기 옵션은 `~/.config/unim/config.yaml`의 `korean` 섹션에 산다. CLI/DBus/GUI에서 모두 동일 키.

### 3.1 `korean.bidirectional_combine: Option<bool>`

- 의미: 초성·중성·종성 영역 내에서 자모 입력 순서에 관계없이 결합을 시도.
- `None` (디폴트) → **OFF**. combinations 정방향 순서만 인식.
- `Some(true)` → ON. composer가 정방향 결합 실패 시 역방향 (b, a) 도 시도. `(ᆯ, ᆨ) → ᆰ` 가능.
- `Some(false)` → 명시적 OFF (None과 동일 효과).

### 3.2 `korean.chord_window_ms: Option<u16>`

- 의미: 첫 자모 타건 후 단일 윈도우 길이 (밀리초).
- `None` 또는 `Some(0)` (디폴트) → **chord OFF**. 자모를 즉시 처리.
- `Some(N)` (10–100 권장) → 첫 자모 후 N ms 동안 추가 자모를 buffer에 누적, 만료 시 일괄 처리.
- 종속 규칙: `bidirectional_combine != Some(true)`이면 chord_window_ms도 무시. 옵션 2는 옵션 1에 종속.

### 3.3 적용 게이트

런타임에 chord가 동작하려면 모두 충족해야 한다:

1. 활성 자판이 `supports_moachigi=true` (capability)
2. `korean.bidirectional_combine == Some(true)` (옵션 1)
3. `korean.chord_window_ms` > 0 (옵션 2)

하나라도 false면 일반 세벌식 동작.

### 3.4 5지점 동기화

config 키 추가 표준 5지점 모두 반영:

| 지점 | 파일·위치 | 비고 |
|---|---|---|
| 엔진 코어 | [`src/config.rs:516-540`](../../../src/config.rs#L516-L540) | `KoreanConfig.bidirectional_combine` / `.chord_window_ms` |
| CLI | [`unim-cli/src/main.rs:443-450`](../../../unim-cli/src/main.rs#L443-L450), [1190-1212](../../../unim-cli/src/main.rs#L1190-L1212) | `ConfigKey::KoreanBidirectionalCombine` / `KoreanChordWindowMs` show/set |
| DBus | [`unim-dbus/src/service.rs:755-763`](../../../unim-dbus/src/service.rs#L755-L763), [999-1010](../../../unim-dbus/src/service.rs#L999-L1010) | get/set 라우팅 |
| GTK GUI | [`unim-gui-gtk/src/settings_dialog.rs:406-518`](../../../unim-gui-gtk/src/settings_dialog.rs#L406-L518) | `MoachigiHandle` 그룹 |
| Locales (ko/en) | `unim-gui-gtk/locales/{ko,en}.yml` | `row_moachigi_*` 키 |

GNOME extension은 이 옵션을 노출하지 않는다 (extension은 Push Mode 전용 — chord는 unim-daemon에서 처리되므로 별도 UI 불필요).

---

## 4. 옛한글(古韓글) 명시 거부

### 4.1 정책

v3는 **현대 한글만** 다룬다. 옛한글 코드포인트가 자판 JSON 어디에든 (layout / combinations / jamo_symbol_map / 등) 진입하면 loader가 `LoadError::ArchaicJamoNotSupported`를 즉시 반환.

### 4.2 거부 코드포인트 범위

| 범위 | 의미 |
|---|---|
| U+1140 ~ U+115F | Hangul Jamo 옛한글 (초성 영역) |
| U+11A8 이전 / U+11C3 이후 | 종성 영역의 옛한글 |
| U+302E ~ U+302F | 방점 |
| U+3165 ~ U+318E | Hangul Compatibility Jamo 옛한글 (ㆍ U+318D 포함) |
| U+A960 ~ U+A97F | Hangul Jamo Extended-A |
| U+D7B0 ~ U+D7FF | Hangul Jamo Extended-B |

### 4.3 코드 진입점

```rust
// loader.rs:38, 51, 67, 107
pub enum LoadError {
    ...
    ArchaicJamoNotSupported { codepoint: u32, location: String },
}
```

진입 위치 (loader.rs):
- L38: enum variant 정의
- L51, L67: Display / Error impl
- L107: 검사 진입점 (각 layout 셀 / combinations / jamo_symbol_map 검사 시 호출)

### 4.4 안마태 원본 옛한글 자리 처리

안마태 2003 자판 원본은 W·T·G·J·B·N의 upper 위치에 옛한글 자모를 배치했다. UNIM 빌트인은 그 6칸을 **한글 조판 기호**로 대체:

| 키 | Shift+키 | 문자 | 코드포인트 | 용도 |
|---|---|---|---|---|
| W | Shift+W | `'` | U+2019 | 닫는 작은따옴표 |
| T | Shift+T | `…` | U+2026 | 줄임표 |
| G | Shift+G | `"` | U+201D | 닫는 큰따옴표 |
| J | Shift+J | `·` | U+00B7 | 가운뎃점 |
| B | Shift+B | `"` | U+201C | 여는 큰따옴표 |
| N | Shift+N | `'` | U+2018 | 여는 작은따옴표 |

이 6키 대체는 사용자 승인 게이트(Phase 0 결정 5번) 산출물.

### 4.5 향후 옛한글 지원

v4 또는 별도 단계에서 검토. v3에서는 100% 배제 — composer/loader/UI 모든 단계가 옛한글 코드포인트를 받지 않는다고 가정해 단순화.

---

## 5. Chord 엔진 — `src/input_engine/chord_buffer.rs`

### 5.1 핵심 원리 (단일 윈도우)

```
첫 자모 도착
    │
    ├─ epoch 증가 + start_instant = Instant::now()
    │
    ├─ 윈도우 안 (now - start_instant < window_ms) 추가 자모 도착
    │     → buffer.push_back(jamo)
    │
    └─ 윈도우 만료 감지
         ├─ 다음 키 도착 시점에 lazy 검사 → flush
         └─ tokio::spawn 비동기 타이머가 ChordIdleFlush 송신 → flush
              ├─ buffer.len() == 1 → 일반 push (composer.add_jamo)
              └─ buffer.len() ≥ 2 → 영역별 분류 → 양방향 결합 → 음절 commit
```

매 키마다 타이머를 reset하지 **않는다** — 첫 자모 기준 단일 윈도우. 이것이 안마태 사양("첫 타건 후 N ms")이며, 음절 경계가 시간으로 결정 가능하게 한다.

### 5.2 영역 분류

buffer flush 시 누적 자모를 region별로 분류:

```rust
match jamo {
    JamoEnum::Cho(_)  => Region::Cho,
    JamoEnum::Jung(_) => Region::Jung,
    JamoEnum::Jong(_) => Region::Jong,
}
```

각 영역에 자모 1개면 단순 set, 2개면 양방향 결합 시도, 3개 이상이면 첫 두 개를 결합 후 나머지로 새 음절 시작.

### 5.3 Flush 트리거 (전수)

| 트리거 | 동작 |
|---|---|
| Idle timeout (tokio::spawn) | 정상 flush — chord 결합 commit |
| Space / Enter / Tab | 정상 flush + 트리거 키 처리 |
| Backspace | 정상 flush + Backspace |
| Hanja key | 정상 flush + 한자 모드 진입 |
| Mode switch (한↔영) | 정상 flush + 모드 전환 |
| FocusOut | 정상 flush (다음 컨텍스트로 유출 방지) |
| Escape | **discard** (buffer 폐기 — 미확정 자모 버림) |
| MAX 8 jamo | 즉시 flush (overflow 방지) |

### 5.4 Stale timer 방어 — epoch counter

`tokio::spawn`은 비동기 타이머로 N ms 후 `ChordIdleFlush` 메시지를 engine_worker로 보낸다. 그동안 다른 키가 들어와 chord가 이미 flush되었으면 (epoch 증가), stale 메시지가 도착해도 무시한다:

```rust
// engine_worker.rs:1639-1665
EngineRequest::ChordIdleFlush { context_id, epoch } => {
    if engine.chord_epoch() == epoch {
        engine.chord_idle_flush_commit()
    } else {
        // stale — 무시
    }
}
```

코드: [chord_buffer.rs](../../../src/input_engine/chord_buffer.rs), [engine_worker.rs:283, 1639-1670](../../../unim-dbus/src/engine_worker.rs), [service.rs:203, 334, 1881-1920](../../../unim-dbus/src/service.rs).

---

## 6. Composer 통합 — `HangulComposer3Bul` 한 군데로

### 6.1 폐기된 별도 composer

초기 설계에는 `HangulComposer3BulMoachigi` (774 lines) 별도 composer가 있었다. Phase 4 이후 제거 — 안마태는 "세벌식 + 모아치기"의 한 변종일 뿐, composer를 분리할 이유가 없다는 판단.

### 6.2 양방향 결합 통합

`HangulComposer3Bul::add_jamo_with_meta`가 사용자 config의 `bidirectional_combine`를 받아 cho/jung/jong 모든 영역에서 정방향 실패 시 역방향 결합 시도. 안마태 키맵의 `combinations` 블록은 정방향 페어만 정의하면 충분 — 역방향은 composer가 자동 시도.

### 6.3 trait 시그니처 무변경

v2 trait `HangulComposer::add_jamo_with_meta(&mut self, jamo, meta)` 그대로. region 인자 추가 안 함 — composer가 받은 `JamoEnum::Cho|Jung|Jong` variant로 자체 분류.

코드: [composer_with_3bul.rs](../../../src/hangul/composer_with_3bul.rs).

---

## 7. 빌트인 자판 — `ko_3bul_anmatae`

### 7.1 파일

[`src/keystroke/keymap/ko_3bul_anmatae.json`](../../../src/keystroke/keymap/ko_3bul_anmatae.json)

### 7.2 핵심 메타

```json
{
  "schema_version": 3,
  "language": "korean",
  "name": "ko_3bul_anmatae",
  "type": "3bul",
  "supports_moachigi": true,
  "metadata": {
    "display_name": { "ko": "안마태 자판 (2003)", "en": "Ahnmatae Keyboard (2003)" },
    "author": "안마태·김진형 (2003)",
    ...
  },
  "layout": { "upper": {...}, "lower": {...} },
  "combinations": { "cho": [...9...], "jung": [...15...], "jong": [...20...] }
}
```

- **type "3bul"**: 3벌식 composer 사용 (별도 moachigi composer 없음).
- **supports_moachigi: true**: 모아치기 capability 마커.
- **`bidirectional_combine` / `chord_window_ms` 필드는 없다** — 사용자 config 소관.
- **layout은 4행** (1st 숫자행 / 2nd-3rd 초성·중성 / 4th 종성). v1/v2 동일 직관 구조.

### 7.3 사용자 가이드

- 한국어: [`docs/user/keymaps/anmatae.md`](../../user/keymaps/anmatae.md)
- 영어: [`docs/user/keymaps/anmatae.en.md`](../../user/keymaps/anmatae.en.md)

---

## 8. 마이그레이션 노트

### 8.1 자판 작성자 입장

- v1/v2 자판은 변경 없이 v3 코드에서 그대로 작동. `supports_moachigi` 누락 = `false` 기본 = 모아치기 OFF.
- 새 모아치기 자판을 만들고 싶으면:
  1. `"schema_version": 3` 명시 (선택, 가독성).
  2. `"supports_moachigi": true` 추가.
  3. **옵션 값(`bidirectional_combine` / `chord_window_ms`)은 키맵에 두지 않는다** — 사용자 config로 위임.
  4. `combinations.{cho,jung,jong}`은 정방향 페어만 적어도 충분 — bidirectional이 켜지면 composer가 역방향 자동 시도.
- 옛한글 자모 코드포인트를 layout/combinations에 두면 즉시 거부 — 한글 조판 기호 또는 일반 ASCII로 대체.

### 8.2 사용자 입장

- 안마태 자판을 처음 선택해도 chord가 자동 켜지지 않는다 (디폴트 OFF).
- chord를 쓰려면 GTK 설정 → 자판 선택 → "안마태 자판 (2003)" → 아래 Moachigi 그룹에서:
  1. **양방향 자모 결합** 토글 ON
  2. **동시 입력 시간 (ms)** 슬라이더를 0에서 50ms 등으로 끌어올림
- 옵션 값은 `~/.config/unim/config.yaml`에 저장되어 자판 전환 후에도 보존.

### 8.3 코드 변경 영향 (라이브러리 사용자)

- `HangulComposer` trait 시그니처 무변경.
- `MoachigiSpec` 구조체 노출됨 — capability 마커. 외부에서 직접 참조할 일은 없음.
- `LoadError`에 `ArchaicJamoNotSupported` variant 추가 — 외부에서 `LoadError`를 패턴 매칭하면 신규 arm 처리 필요.

---

## 9. 코드 진입점 매핑 (0.3.0)

| 책임 | 파일:line |
|---|---|
| `MoachigiSpec` 정의 | [schema.rs:39-50](../../../src/keystroke/profile/schema.rs#L39-L50) |
| `RawProfile.supports_moachigi` 필드 | [schema.rs:90, 306, 315](../../../src/keystroke/profile/schema.rs) |
| `LayoutProfile.moachigi: Option<MoachigiSpec>` | [schema.rs:355, 421](../../../src/keystroke/profile/schema.rs) |
| v3 capability 결정 (`Some` ↔ `None`) | [schema.rs:395-405](../../../src/keystroke/profile/schema.rs#L395-L405) |
| `LoadError::ArchaicJamoNotSupported` | [loader.rs:38, 107](../../../src/keystroke/profile/loader.rs) |
| `KoreanConfig.bidirectional_combine` / `.chord_window_ms` | [config.rs:516-540](../../../src/config.rs#L516-L540) |
| `chord_buffer` 모듈 | [src/input_engine/chord_buffer.rs](../../../src/input_engine/chord_buffer.rs) |
| chord 통합 (press_key) | [src/input_engine/press_key.rs](../../../src/input_engine/press_key.rs) |
| `chord_idle_flush_commit` | [engine.rs](../../../src/input_engine/engine.rs), [engine_worker.rs:283](../../../unim-dbus/src/engine_worker.rs#L283) |
| `EngineRequest::ChordIdleFlush` | [unim-dbus/src/service.rs:203, 334](../../../unim-dbus/src/service.rs), [engine_worker.rs:1639](../../../unim-dbus/src/engine_worker.rs#L1639) |
| tokio::spawn idle 타이머 | [unim-dbus/src/service.rs:1881-1920](../../../unim-dbus/src/service.rs#L1881-L1920) |
| GTK Moachigi group | [unim-gui-gtk/src/settings_dialog.rs:406-518](../../../unim-gui-gtk/src/settings_dialog.rs#L406-L518) |
| CLI ConfigKey (set/show) | [unim-cli/src/main.rs:443, 1190-1212](../../../unim-cli/src/main.rs) |
| DBus get/set 라우팅 | [unim-dbus/src/service.rs:755, 999](../../../unim-dbus/src/service.rs) |
| 빌트인 안마태 자판 | [src/keystroke/keymap/ko_3bul_anmatae.json](../../../src/keystroke/keymap/ko_3bul_anmatae.json) |

---

## 10. 테스트 매트릭스

### 10.1 단위 (schema · loader · composer)

| 테스트 | 위치 | 검증 |
|---|---|---|
| `sv3_supports_moachigi_parses_correctly` | schema.rs:775 | `supports_moachigi=true` → `MoachigiSpec` capability `Some(_)` |
| `sv3_keymap_options_are_ignored` | schema.rs | 키맵의 `bidirectional_combine`/`chord_window_ms`는 무시 (Phase 7) |
| `loader_rejects_archaic_in_layout` | loader.rs:433+ | U+318D layout 진입 거부 |
| `loader_rejects_archaic_in_jong` | loader.rs:475+ | U+11C3 이후 종성 거부 |
| `loader_rejects_archaic_in_combinations` | loader.rs:501+ | combinations 옛한글 거부 |
| `bidirectional_combine_jong_reverse_pair` | composer_with_3bul.rs | (b,a) 역방향 시도 |
| `bidirectional_combine_cho_jung_reverse` | composer_with_3bul.rs | cho/jung 양방향 |

### 10.2 통합 (chord_buffer · input_engine)

| 테스트 | 시나리오 |
|---|---|
| `chord_window_basic_chord_compose` | 50ms 안에 ㄱ+ㅎ+ㅡ+ㅣ → "킈" |
| `chord_window_expired_separates_syllables` | 50ms 만료 후 ㅎ+ㅣ → 별개 음절 |
| `chord_max_size_flush` | 8자모 도달 시 즉시 flush |
| `chord_idle_flush_via_tokio_timer` | ChordIdleFlush 비동기 메시지 처리 |
| `chord_escape_discards_buffer` | Escape → buffer 폐기 |
| `chord_focus_out_flushes` | FocusOut → 정상 flush |
| `chord_hanja_key_flushes` | 한자 키 진입 전 flush |
| `chord_stale_epoch_ignored` | 오래된 ChordIdleFlush 무시 |

### 10.3 옵트인/옵트아웃 (opt_*)

| 테스트 | 시나리오 |
|---|---|
| `opt_chord_off_default_behavior` | 디폴트 (둘 다 None) → 일반 세벌식 동작 |
| `opt_chord_supports_moachigi_false_ignored` | 사용자 옵션 ON이지만 자판이 capability 없음 → 무시 |
| `opt_chord_bidirectional_off_disables_chord` | bidirectional=false → chord_window_ms도 무시 |
| `opt_chord_window_zero_disables` | `chord_window_ms=Some(0)` → chord OFF |
| `opt_chord_active_when_all_three_satisfied` | capability + bidirectional + window>0 → 활성 |

### 10.4 카운트

0.3.0 시점 lib 테스트 581 PASS / 0 FAIL. v3 신규 ≈ 30개 (chord 통합 14개 + idle flush 7개 + opt 5개 + schema/loader/composer 4개).

---

## 11. v3 비목표 (v4 또는 별도)

본 v3가 데이터로 표현하지 않은 것:

- **옛한글(고어) 입력**: 100% 거부. v4 또는 별도 트랙에서 별도 설계.
- **preedit 진행 중 chord 시각화**: chord 만료 전까지 화면 무반응. 50ms 디폴트라 실사용 영향 미미하지만, 향후 partial preedit 표시 검토 가능.
- **chord 동안의 키 hold 처리**: 현재는 KeyDown만 본다. KeyDown→KeyUp 페어 기반 chord (실제 동시성 보장)는 v3에서 미구현.
- **자판 단위 chord 디폴트값 매크로**: `supports_moachigi=true` 자판마다 권장 디폴트(예: 50ms)가 다를 수 있으나, 현재는 사용자가 직접 설정 — 자판 권장값 메타 미지원.
- **두벌식 모아치기**: `type: "2bul"` 자판은 chord 미지원 (composer 통합 안 함). 두벌식 + chord 조합은 v4 검토.
- **GNOME extension UI 노출**: extension은 chord 옵션을 직접 노출하지 않는다. 모든 chord 처리는 unim-daemon에서 발생하므로 GTK 설정만으로 충분.

---

## 12. 변경 이력

- **2026-05-04**: Phase 0 사용자 결정 게이트 (자판 이름, 옛한글 정책, 한글 조판 기호 6개 승인).
- **2026-05-04**: Phase 1 schema v3 + loader 옛한글 거부.
- **2026-05-05**: Phase 2~3 별도 `HangulComposer3BulMoachigi` 작성 후 폐기 — `HangulComposer3Bul` 통합으로 회귀.
- **2026-05-05**: Phase 4 `chord_buffer` + ChordIdleFlush 비동기 타이머 (tokio).
- **2026-05-05**: Phase 5 QA 회귀 발견 ("구하다" 손상) → Phase 4-bis idle flush 보강.
- **2026-05-05**: Phase 6 사용자 가이드(한/영) + CHANGELOG.
- **2026-05-05**: Phase 7 모아치기 옵션을 키맵 → 사용자 config로 일원화. 디폴트 OFF (opt-in)로 변경. `MoachigiSpec`을 capability 마커로 환원.

---

## 13. 참고

- v1 베이스: [`docs/archive/plans/LAYOUT_PROFILE_V1.md`](../../archive/plans/LAYOUT_PROFILE_V1.md)
- v2 베이스: [`docs/archive/plans/LAYOUT_PROFILE_V2.md`](../../archive/plans/LAYOUT_PROFILE_V2.md)
- 사용자 가이드: [`docs/user/keymaps/anmatae.md`](../../user/keymaps/anmatae.md) · [`anmatae.en.md`](../../user/keymaps/anmatae.en.md)
- 자판 JSON: [`src/keystroke/keymap/ko_3bul_anmatae.json`](../../../src/keystroke/keymap/ko_3bul_anmatae.json)
- 사전 조사: `docs/references/research/안마태 자판 조사.md`, `쿼티형 세벌식 초안.md` (옵시디언 vault)
- 안마태 신부, 김진형 (2003), "모아치기 한글 입력 자판 설계"
