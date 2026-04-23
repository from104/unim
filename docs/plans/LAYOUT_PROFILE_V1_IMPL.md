# UNIM 자판 프로필 v1 — 구현 하네스 기획

Date: 2026-04-23
Status: **기획안 (Plan)** — 구현 착수 전 리뷰 대상.
Owner: Core / Keystroke / Settings
Scope: `docs/plans/LAYOUT_PROFILE_V1.md`의 스키마와 `docs/plans/new_keymaps/*.json` 드래프트를 실제로 UNIM 엔진에 통합하고, 자판별 `rule_sets` 토글을 CLI·GUI에서 제공하는 단계별 실행 계획.

---

## 0. 참조 문서

| 항목 | 위치 |
|---|---|
| 스키마 명세 | `docs/plans/LAYOUT_PROFILE_V1.md` |
| 키맵 드래프트 9종 | `docs/plans/new_keymaps/*.json` |
| 현 v0 키맵 | `src/keystroke/keymap/*.json` |
| 현 결합 규칙 | `src/hangul/composer_with_2bul.rs:19-44`, `composer_with_3bul.rs:19-53` |
| 현 로더 | `src/keystroke/mod.rs:7-30` |
| 설정 3지점 싱크 규약 | `GEMINI.md` (Settings Synchronization) |
| ROADMAP 6단계 | `ROADMAP.md` — 엔진 재설계 과제 (v1 비목표) |

---

## 1. 두 파트 분리 개요

```
┌───────────────────────────────────────────────────────────┐
│  PART A — 엔진 (keystroke/hangul)                          │
│  - LayoutProfile 스키마·로더                                │
│  - inherits 해석                                           │
│  - combinations + rule_sets 병합                          │
│  - HangulComposer 통합                                    │
│  - 내장 9종 + 사용자 디렉토리 스캔                            │
└───────────────────────────────────────────────────────────┘
                          ↕ (Config)
┌───────────────────────────────────────────────────────────┐
│  PART B — 설정 (config/cli/gui/dbus)                       │
│  - korean_custom_layout, korean_active_rule_sets 필드      │
│  - CLI: unim-cli config (+ rule-set 서브커맨드)              │
│  - GUI: unim-gui-gtk 자판 선택 + rule_set 토글              │
│  - DBus: 레거시 key 디스패치 + YAML/JSON 자동 반영            │
└───────────────────────────────────────────────────────────┘
```

---

## 2. Part A — 엔진

### 2.1 모듈 배치

신규 모듈 `src/keystroke/profile/`:

| 파일 | 역할 |
|---|---|
| `mod.rs` | Public API re-export, 단일 진입점 `LayoutProfileLoader::load(name)` |
| `schema.rs` | serde 구조체 정의 (v0/v1). `#[serde(untagged)]`로 판별 |
| `loader.rs` | 파일 로드, 내장/사용자 네임스페이스 병합, `schema_version` 판별 |
| `inherit.rs` | `inherits` 체인 해석 + 순환 탐지 |
| `builder.rs` | `combinations` + 활성 `rule_sets` → `CombinedJamoMap` |
| `builtin.rs` | 내장 9종 `include_str!` (v1 이관 후에도 유지) |
| `validate.rs` | `LAYOUT_PROFILE_V1.md` §7 검증 규칙 구현 |

### 2.2 핵심 타입

```rust
pub struct LayoutProfile {
    pub schema_version: u8,              // 0 또는 1
    pub language: Language,              // Korean / English
    pub name: String,
    pub layout_type: LayoutType,         // TwoBul / ThreeBul / English
    pub metadata: LayoutMetadata,
    pub layout: KeyLayout,               // 기존 v0 layout과 동형
    pub combinations: Option<CombinationSet>, // None → v0 호환 경로 (Rust const 상속)
    pub rule_sets: HashMap<String, RuleSet>,
    pub active_rule_sets: Option<Vec<String>>,
}

pub struct CombinationSet {
    pub cho: Vec<(Cho, Cho, Cho)>,
    pub jung: Vec<(Jung, Jung, Jung)>,
    pub jong: Vec<(Jong, Jong, Jong)>,
}

pub struct RuleSet {
    pub active: bool,
    pub description: Option<LocalizedText>,
    pub combinations: Vec<RawTriple>,    // scope는 첫 자모 코드포인트로 자동 판별
}

pub struct RawTriple {                   // 해석 전 문자열 → 후처리 시 Cho/Jung/Jong 분기
    pub first: char,
    pub second: char,
    pub result: char,
}

/// JSON에서 `"description": "..."`(단일 문자열) 또는
/// `"description": { "ko": "...", "en": "..." }` (다국어 객체) 둘 다 허용.
/// 단일 문자열은 내부적으로 `{ "default": "..." }`로 변환되어 저장.
#[derive(serde::Deserialize)]
#[serde(untagged)]
pub enum LocalizedText {
    Single(String),
    Map(HashMap<String, String>),    // key: "ko", "en", "ja" 등 ISO 639-1
}

impl LocalizedText {
    /// `locale`("ko"/"en")에 맞는 문자열을 반환. 없으면 "en" → "default" → 첫 값 순.
    pub fn resolve(&self, locale: &str) -> &str { /* ... */ }
}
```

`metadata.description`·`metadata.display_name`도 같은 `LocalizedText` 사용.

### 2.3 로더·해석 파이프라인

```
LayoutProfileLoader::load(name) →
  1. resolve_path(name):
     - 사용자 디렉토리(~/.config/unim/layouts/<name>.json) 우선
     - 없으면 내장 9종 (builtin.rs)
  2. serde_json::from_str → ProfileRaw (v0/v1 구분 전)
  3. detect_schema_version():
     - schema_version 존재 → v1
     - metadata/inherits/combinations/rule_sets/active_rule_sets 중 하나라도 → v1
     - else → v0
  4. v0 경로: LayoutProfile::from_v0(raw)
     - combinations = None (로 마킹 → builder에서 Rust const 주입)
  5. v1 경로: LayoutProfile::from_v1(raw)
     - inherit::resolve(chain) → 병합 완료 프로필
     - validate::check_all() → 경고 누적
  6. resolve_active_rule_sets():
     - active_rule_sets 지정 시 그 이름만 active
     - 없으면 각 rule_sets.<name>.active 값 사용
     - 이 단계에서 비활성 rule_sets는 drop
  7. builder::build_combination_set(profile) → CombinationSet
     - combinations가 Some이면 그 값만
     - None이면 Rust const(JUNG/JONG/CHO_COMBINATIONS) 복제
     - 활성 rule_sets.combinations를 해당 scope 배열에 append (중복 시 덮어쓰기)
  8. LayoutProfile 반환 (fully resolved)
```

### 2.4 HangulComposer 통합

```rust
impl HangulComposer2Bul {
    pub fn new() -> Self { /* 기존 정적 테이블 */ }

    // 신규
    pub fn new_with_profile(profile: &LayoutProfile) -> Self {
        let map = profile.build_combined_jamo_map();
        let mut c = Self { base_composer: BaseHangulComposer::new() };
        *c.base_composer.combined_jamo() = map;
        c
    }
}
```

`HangulInputContext::new()` 호출부 (현 `src/keystroke/mod.rs:55`)에서 LayoutProfile을 받아 `new_with_profile`로 분기.

### 2.5 내장 9종 이관

Phase 6에서 `docs/plans/new_keymaps/*.json` 드래프트를 `src/keystroke/keymap/*.json`으로 **교체**. 그 전까지 공존:

```rust
fn builtin_profile_json(name: &str) -> &'static str {
    // Phase 1-5: 기존 v0 JSON
    // Phase 6 이후: v1 자기 완결 JSON
}
```

Phase 6 완료 시점에 `JUNG/JONG/CHO_COMBINATIONS` Rust const는 **v0 호환 fallback 경로**로만 남음. 삭제는 `LAYOUT_PROFILE_V1.md` §11 "코드 개편 시 삭제"에 따라 추후.

---

## 3. Part B — 설정

### 3.1 Config 필드 확장

`src/config.rs`:

```rust
pub struct Config {
    /// 자판 프로필 이름. 내장 9종 중 하나 또는 사용자 프로필(~/.config/unim/layouts/<name>.json).
    /// 기존 `KoreanLayout` enum을 **폐지하고 문자열로 통합**.
    /// 기본값: "2bulstd". 마이그레이션은 serde custom deserializer에서
    /// 구 enum 값("Dubeolsik" 등)을 새 이름으로 자동 변환.
    pub korean_layout: String,

    /// 활성 rule_set 이름 목록.
    /// - 빈 목록(`[]`): **프로필 기본값 사용** (각 rule_sets.<name>.active 값 그대로).
    /// - 비어 있지 않음: 나열된 이름만 active, 나머지는 강제 off.
    pub korean_active_rule_sets: Vec<String>,
}
```

**`KoreanLayout` enum 폐지 마이그레이션**:

- `src/config.rs`의 `KoreanLayout` enum 제거.
- 구 enum 값(`Dubeolsik`·`Sebeolsik390`·`Sebeolsik391`·`Sebeolsik3BulNoshift`) → 새 문자열(`"2bulstd"`·`"3bul390"`·`"3bul391"`·`"3bul_noshift"`).
- serde `deserialize_with`로 구 YAML(`korean_layout: Dubeolsik`)도 자동 수용해 새 문자열로 변환.
- `src/config.rs:50-105`의 `KoreanLayout` 사용 전부 `&str` 또는 `String`으로 치환.

**해석 우선순위** (`LayoutProfile` 로드 시):

- 단일 경로: `LayoutProfileLoader::load(&config.korean_layout)`. fallback 없이 실패 시 경고 + 기본 `"2bulstd"`.

**Rule set 적용 우선순위**:

- `korean_active_rule_sets`가 비어 있으면 → 프로필의 `active_rule_sets` 또는 각 `rule_sets.*.active` 값 그대로 사용.
- 비어 있지 않으면 → 이 목록으로 **프로필 `active_rule_sets` override**.

### 3.2 CLI — `unim-cli config`

**ConfigKey enum 확장** (`unim-cli/src/main.rs`):

```rust
enum ConfigKey {
    // 기존 ...
    KoreanLayout,             // set/get string (enum 폐지로 string 타입으로 전환)
    KoreanActiveRuleSets,     // set/get comma-separated list
}
```

**명령 예**:

```bash
# 자판 선택 (enum 이름 대신 프로필 이름 문자열)
unim-cli config set korean_layout 3bul_qwerty
unim-cli config get korean_layout

# Rule set 일괄 설정
unim-cli config set korean_active_rule_sets "qwerty_sebul_jong_reinterpret,qwerty_sebul_jong_extended"
unim-cli config set korean_active_rule_sets ""   # 빈 목록 = 프로필 기본값 사용

# Rule set 편의 서브커맨드 (신규)
unim-cli config rule-set list            # 현재 프로필의 사용 가능 rule_set + 활성 상태
unim-cli config rule-set enable <name>   # 활성 목록에 추가
unim-cli config rule-set disable <name>  # 제거
unim-cli config rule-set reset           # korean_active_rule_sets 비움 → 프로필 기본값 회복

# 자판 프로필 관리 (신규 `layout` 서브커맨드)
unim-cli layout list                     # 내장 9종 + ~/.config/unim/layouts/*.json
unim-cli layout describe <name>          # metadata + 포함 rule_sets 표시
unim-cli layout validate <file.json>     # 스키마·자모 해석·rule_set 영역 일관성 검사
                                         # (스펙 §7 검증 규칙 + §3.5.6 rule_set 규칙)
                                         # exit code 0=통과, 1=경고만, 2=오류
```

`layout validate`는 사용자가 자기 프로필을 작성하면서 활용할 CLI. 내장 호출은 `validate::check_all(&profile)` 결과를 사람이 읽기 쉽게 출력.

**로케일 추가** (`unim-cli/locales/{ko,en}.yml`):

```yaml
config:
  korean_custom_layout:
    label: "사용자 자판 프로필"
    description: "~/.config/unim/layouts/ 또는 내장 프로필 이름"
  korean_active_rule_sets:
    label: "활성 규칙 세트"
    description: "쉼표 구분 이름 목록. 비우면 프로필 기본값 사용."
  rule_set:
    list_header: "현재 프로필의 규칙 세트"
    active_mark: "✓"
    inactive_mark: "·"
```

### 3.3 GUI — `unim-gui-gtk`

**설정 대화상자 (`settings_dialog.rs`)**:

신규 `Adw.PreferencesGroup` "자판" 추가:

```
┌─ 자판 ─────────────────────────────────────┐
│ 자판 선택          [2벌식 표준       ▼]    │  ← Adw.ComboRow
│   (내장 9종 + 사용자 프로필 동적 로드)       │
│                                           │
│ ▼ 규칙 세트                                │  ← 펼침 그룹
│   □ 순아래받침 규칙                         │  ← Adw.SwitchRow
│     Shift 없이 종성 조합...                 │     (description 부제)
│   □ 쿼티 세벌 확장 격음                     │
│     ㅈ+ㅎ→ㅊ, ㄷ+ㅎ→ㅌ 활성화               │
│                                           │
│   [기본값으로 재설정]                       │  ← 버튼
└───────────────────────────────────────────┘
```

**UI 동작 흐름**:

1. 기동 시 `LayoutProfileLoader::list_available()` 호출 → ComboRow 채움
2. ComboRow 선택 변경 → DBus `set_config("korean_custom_layout", name)`
3. 선택 변경 시 해당 프로필 다시 로드 → rule_sets 목록 **동적 재구성**
4. 각 SwitchRow 토글 → `korean_active_rule_sets` 리스트 업데이트 → DBus set
5. "기본값으로 재설정" → `korean_active_rule_sets` 비움 (프로필 기본값 회복)

**SwitchRow 자동 생성**:

```rust
for (name, rule_set) in profile.rule_sets {
    let row = adw::SwitchRow::builder()
        .title(name.as_str())
        .subtitle(rule_set.description.as_deref().unwrap_or(""))
        .active(active_rule_sets.contains(&name))
        .build();
    row.connect_active_notify(|_| /* emit DBus */);
    group.add(&row);
}
```

### 3.4 DBus 서비스

`unim-dbus/src/service.rs`:

- 레거시 key 디스패치에 `korean_custom_layout` / `korean_active_rule_sets` 추가
- YAML/JSON 엔드포인트 (`get_config_yaml` / `set_config_yaml`)는 serde로 자동 반영
- `ConfigChanged` signal 브로드캐스트: GNOME Extension·unim-gui가 구독

### 3.5 Rule set 토글 UX 원칙

- **자판 전환 시**: `korean_active_rule_sets` **초기화하지 않음**. 단, 새 프로필에 없는 이름은 로드 시 silently drop + 경고 로그.
- **description 다국어**: `LocalizedText`(§2.2) 사용. GUI·CLI는 시스템 locale(`$LANG` 또는 Config) 기준으로 언어 선택. fallback 순서는 `resolve()` 정의대로.
- **CLI 표시**: `rule-set list` 출력에 `active: true/false` + localized description 표시.
- **빈 `korean_active_rule_sets` 의미**: 프로필 기본값 사용(§3.1). "모든 rule_set 강제 off" 시나리오는 프로필 파일 자체 수정 또는 inherits로 해결.

### 3.6 사용자 디렉토리 핫리로드

`~/.config/unim/layouts/*.json`는 `typefix-blacklist.yaml` 방식과 동일하게 **mtime 감시 자동 재로드**.

- 데몬 내 `profile_watcher.rs` 신설 — `src/typefix_blacklist.rs`의 mtime 핫리로드 패턴 재사용.
- 디렉토리 전체 또는 현재 사용 중인 프로필 파일만 감시(후자가 성능 유리).
- 변경 감지 시: `LayoutProfileLoader::reload(name)` → 새 `CombinedJamoMap` 생성 → `HangulInputContext` 재구성 → DBus `ConfigChanged` signal 방출(GUI 동기화).
- GUI "재로드" 명시 버튼은 **불필요**. 파일 저장 즉시 반영.
- CLI 수정(`unim-cli layout` 편집 시나리오는 없음)은 config.yaml을 거치므로 별도 감시 불필요.

---

## 4. Config 3지점 싱크 체크리스트

`GEMINI.md` Settings Synchronization 원칙 준수 — 신규 필드 2개 각각 다음 5곳 동시 반영:

| # | 파일 | `korean_layout` (String, enum 폐지) | `korean_active_rule_sets` |
|---|---|---|---|
| 1 | `src/config.rs` | 필드 타입 enum→String, default + 구 enum 수용 deserializer | 필드 + default(빈 Vec) |
| 2 | `unim-cli/src/main.rs` (`ConfigKey`) | `KoreanLayout` arm (타입 변경) | `KoreanActiveRuleSets` arm |
| 3 | `unim-cli/locales/{ko,en}.yml` | label + description | label + description |
| 4 | `unim-dbus/src/service.rs` | 레거시 key dispatch (값 타입 string) | 레거시 key dispatch |
| 5 | `unim-gui-gtk/src/settings_dialog.rs` (또는 `gtk_ui.rs`) | ComboRow bind (프로필 이름 나열) | SwitchRow 동적 생성 |

GNOME Shell 전용 키가 아니므로 `unim-gnome-extension/prefs.js` 수정 **불필요**.

---

## 5. 구현 단계 (Phases)

| Phase | 범위 | 산출물 | 동작 변화 |
|---|---|---|---|
| **1** | 스키마 + 로더 기본 | `src/keystroke/profile/*.rs`, `LocalizedText`, v0→v1 자동 승격 | 없음 (내부만) |
| **2** | combinations 병합 + Composer 통합 | `new_with_profile` + 기존 경로와 동일 결과 regression | 없음 |
| **3** | 사용자 디렉토리 스캔 + 핫리로드 | `~/.config/unim/layouts/*.json` 로드, `profile_watcher.rs` mtime 감시 | 사용자가 수동 배치·편집 시 자동 인식 |
| **4** | Config 필드 + CLI + validate | `korean_layout`(String) + `korean_active_rule_sets` + `unim-cli config` + `unim-cli layout {list,describe,validate}` + `KoreanLayout` enum 폐지 마이그레이션 | CLI로 자판·rule_set 전환 + 프로필 검증 |
| **5** | GUI | `settings_dialog.rs` Adw 그룹 + 동적 SwitchRow + locale 적용 | GUI 토글 가능 |
| **6** | 내장 9종 v1 이관 (**Phase 5 직후 즉시**) | `docs/plans/new_keymaps/*.json` → `src/keystroke/keymap/*.json` 교체, metadata 공개 | 기본 동작 동일하나 내부 자기 완결 |
| **7** | 문서·마이그레이션 공지 | `CHANGELOG.md`, `IME_BEHAVIOR.md`, `README.md` 갱신, enum 폐지 공지 | 사용자 대상 공지 |

각 phase는 독립 PR. Phase 1-3는 behavior-preserving (회귀 위험 최소), Phase 4-5가 사용자 대면 변화. **Phase 6은 Phase 5 직후 바로 진행** — 사용자 피드백 사이클 없이 즉시 이관해 Rust const와 v1 자기 완결 형태의 공존 기간을 최소화.

---

## 6. 테스트 전략

### 6.1 단위 테스트 (`src/keystroke/profile/`)

- `schema::detect_schema_version`: v0/v1 판별 정확성
- `schema::from_v0`: 9종 v0 JSON 로드 후 LayoutProfile 구조 검증
- `inherit::resolve`: 2단계 체인, 순환 탐지, 미존재 참조
- `builder::build_combination_set`:
  - combinations None → Rust const 복제
  - combinations Some → 자기 완결
  - active rule_sets 병합 + 중복 덮어쓰기
- `validate::check_all`: §7 6개 규칙 각각

### 6.2 통합 테스트 (`tests/`)

- **회귀**: v0 9종 → `new_with_profile` 경로로 로드 시 기존 정적 경로와 **동일한 `CombinedJamoMap`** 생성 (HashMap eq).
- **순아래받침 토글**: `ko_3bul390` + `sun_arae_batchim` on/off 시 `"곧"` 입력 시 종성 결과 차이 검증.
- **쿼티 세벌식 자기 완결**: `ko_3bul_qwerty` 로드 시 Rust const 참조 없이 `"까"`·`"꽃"` 정상 조합.
- **사용자 디렉토리**: temp dir에 커스텀 프로필 배치 후 로드·override 검증.

### 6.3 설정 싱크 테스트

- CLI `set` → config.yaml 반영 → DBus `get_config` 결과 일치
- GUI 토글 → DBus `ConfigChanged` signal 수신 → 엔진 재구성
- YAML 직접 편집 → 데몬 mtime 핫리로드 (typefix-blacklist 방식 재사용 검토)

### 6.4 수동 smoke (GUI)

- 자판 전환 ↔ 입력 결과 즉시 반영
- rule_set SwitchRow 토글 후 live preview 단어 조합
- `~/.config/unim/layouts/` 빈 상태·프로필 1개·충돌 각각

---

## 7. 마이그레이션 영향

### 7.1 기존 사용자

- Phase 1-3: 변화 없음. 기존 config.yaml 그대로 작동.
- Phase 4: `korean_custom_layout` 미설정(default `None`) → 기존 `korean_layout` enum 경로. **영향 zero**.
- Phase 6: 내장 9종이 v1로 바뀌지만 `CombinedJamoMap`이 동일하므로 입력 결과 불변.

### 7.2 신규 사용자

- Phase 4 이후: `~/.config/unim/layouts/3bul_qwerty.json` 배치 → `unim-cli config set korean_custom_layout 3bul_qwerty` 한 줄로 활성.
- Phase 5 이후: GUI 자판 ComboRow에 자동 노출.

### 7.3 기여자 관점

- 신규 자판 기여: `docs/plans/new_keymaps/*.json` 형식으로 PR, Phase 1 로더가 그대로 수용.
- Rust 코드 수정 불필요 (자판 정체성이 프로필에 전부 있으므로).

---

## 8. 결정 사항 (Resolved — 2026-04-23)

이전 열린 질문 6개에 대한 확정 답변. 이후 충돌 시 이 절이 최종 기준.

1. **내장 9종 Phase 6 타이밍** → **즉시**. Phase 5 직후 바로 이관. Rust const와 v1 자기 완결 프로필의 공존 기간을 최소화해 복잡도 감소.
2. **사용자 디렉토리 핫리로드** → **mtime 자동 재로드**. `profile_watcher.rs` 신설, `typefix-blacklist.yaml` 패턴 재사용. GUI "재로드" 버튼 불필요.
3. **`korean_layout` enum 폐지**. `KoreanLayout` enum 제거, `korean_layout: String`으로 통합. 구 enum YAML 값(`Dubeolsik` 등)은 serde custom deserializer로 자동 문자열 변환(`"2bulstd"` 등). Phase 4에서 수행, Phase 7에서 사용자 공지.
4. **description 다국어 지원**. `LocalizedText` 신규 타입(§2.2) — `"description": "..."` 단일 문자열 또는 `"description": { "ko": "...", "en": "..." }` 객체 둘 다 허용. `metadata.description`·`metadata.display_name`·`rule_sets.*.description` 모두 적용. locale fallback: 요청 locale → `en` → `default` → 첫 값.
5. **`unim-cli layout validate <file.json>` CLI 추가**. Phase 4에서 구현. 스키마·자모 해석·rule_set 영역 일관성 검사. exit code `0`=통과, `1`=경고만, `2`=오류.
6. **빈 `korean_active_rule_sets` = 프로필 기본값 사용**. `Option<Vec<String>>` 복잡도 없이 단순 `Vec<String>` + 빈 목록을 "기본값" 센티넬로 사용. "모든 rule_set 강제 off" 시나리오는 프로필 파일 수정·`inherits`로 해결(use case 빈도 낮음).

---

## 9. 완료 기준

Phase 7까지 완료 (2026-04-24):

- [x] `docs/plans/new_keymaps/*.json` 9종 + `ko_3bul_qwerty` 신규 = 10종이 `src/keystroke/keymap/`에 이관됨 (Phase 6).
- [x] `cargo test --workspace` 331 lib tests + 바이너리 테스트 전부 통과.
- [x] `unim-config layout list / describe / validate` 작동 (Phase 4c). 계획상 `rule-set list` 서브커맨드는 `describe`가 rule_sets 블록을 포함하는 형태로 통합됨.
- [x] GUI 자판 ComboRow + rule_set SwitchRow 작동 (Phase 5). 빌드 성공, 수동 GTK 스모크는 사용자 검증.
- [x] `~/.config/unim/layouts/` 사용자 프로필 로드·override 검증 (Phase 3 registry 테스트 `user_profile_overrides_builtin_by_name` 등).
- [x] CHANGELOG 갱신 (Unreleased 블록에 Layout Profile v1 항목 추가). IME_BEHAVIOR는 조합 동작이 아닌 preedit/commit 프로토콜 문서이므로 본 변경 범위 밖. README는 향후 자판 작성 가이드를 별도 지면으로 분리 예정.
- [x] ROADMAP 3.7단계 신규 섹션에 Phase 1-7 체크리스트 완료 표시.

### 이후 별도 연기된 작업

- **`korean_layout` enum 폐지** (IMPL §3.1·§8.3 결정): 32개 파일 블라스트 반경을 이유로 Phase 4에서 **additive `custom_layout: Option<String>`으로 대체**. 내장 9종이 v1로 이관된 지금(Phase 6 이후) enum 참조점이 감소했으므로 별도 커밋으로 정리 가능. `typefix_blacklist.rs`의 `HashMap` 키가 enum이라는 점이 가장 큰 마이그레이션 부담 (blacklist YAML 포맷 변경 + 기존 엔트리 마이그레이션 경로 필요).

---

## 10. 참고

- v1 스키마: `docs/plans/LAYOUT_PROFILE_V1.md`
- 드래프트 9종: `docs/plans/new_keymaps/*.json`
- Config 3지점 싱크: `GEMINI.md`, `CLAUDE.md` feedback memory
- AutoTypeFix 설정 확장 선례: `src/typefix_blacklist.rs` + GUI 패턴
- 엔진 재설계 v2 과제: `ROADMAP.md` 6단계 (본 기획 범위 밖)
