# Phase 1 — Config Editor 산출물

## 1. 수정 파일

| 파일 | 변경 요약 |
|------|-----------|
| `src/config.rs` | `AutoTypeFixConfig` 필드 2개 추가 + serde default, 범위 상수/clamp 함수, `ManualShortcutConfig` 신설, `EngineConfig.manual_shortcuts` 추가, 기본값/역호환성 테스트 4개 추가 |
| `src/auto_typefix.rs` | `check_forward` DICTIONARY 체크를 `skip_on_english_word` 토글로 감쌈, `check_reverse`에 "온전한 음절" 검증(`skip_on_complete_syllable`) 추가, 토글 동작 검증 테스트 5개 추가, 기존 struct-literal 테스트를 `..Default::default()` 패턴으로 보정 |

### 구체 위치 (파일:라인)

- `src/config.rs`
  - `AUTO_TYPEFIX_*_MIN/MAX` 상수 (≈244–250)
  - `default_auto_typefix_*` + `default_manual_shortcut_*` 함수들 (≈252–282)
  - `AutoTypeFixConfig` 필드 + serde 어노테이션 (≈284–311)
  - `AutoTypeFixConfig::clamp_ranges()` (≈328–343)
  - `ManualShortcutConfig` (≈353–376)
  - `EngineConfig.manual_shortcuts` 필드 및 `Default` (≈465, 483)
  - 신규 테스트 `test_auto_typefix_defaults`, `test_auto_typefix_clamp`, `test_manual_shortcut_defaults`, `test_legacy_yaml_backcompat_autotypefix_and_manual_shortcuts`, `test_empty_yaml_full_defaults`

- `src/auto_typefix.rs`
  - `check_forward`의 `skip_on_english_word` 토글 가드 (≈167–173)
  - `check_reverse`의 완성 음절 skip 가드 (≈284–295)
  - 신규 테스트 `test_forward_skip_on_english_word_toggle_off`, `test_forward_skip_on_english_word_off_triggers_for_word`, `test_reverse_skip_on_complete_syllable_on_suppresses`, `test_reverse_skip_on_complete_syllable_off_triggers`, `test_reverse_with_preedit_always_triggers_regardless_of_toggle` (파일 말미)

## 2. 추가/변경된 필드·함수 요약

### AutoTypeFixConfig (필드 2개 추가)
- `skip_on_english_word: bool` — 기본 `true`. `check_forward`에서 사전 hit 억제 여부.
- `skip_on_complete_syllable: bool` — 기본 `true`. `check_reverse`에서 버퍼가 모두 완성 음절(=preedit 없음)일 때 트리거 억제.
- 모든 기존 필드도 `#[serde(default = "…")]` 명시하여 누락 시 기본값으로 복원.
- 범위 상수 노출: `AUTO_TYPEFIX_KOR_THRESHOLD_MIN=2 / MAX=6`, `AUTO_TYPEFIX_ENG_MIN_LENGTH_MIN=3 / MAX=8`, `AUTO_TYPEFIX_TIME_WINDOW_MIN=500 / MAX=5000`.
- `AutoTypeFixConfig::clamp_ranges()` — config 레벨 범위 보정 (CLI/GUI 중복 검증 제거 목적).

### ManualShortcutConfig (신설)
- `forward: Vec<String>` — 기본 `["<Super>k"]`
- `reverse: Vec<String>` — 기본 `["<Shift><Super>k"]`
- `EngineConfig.manual_shortcuts: ManualShortcutConfig` (`#[serde(default)]`)

### auto_typefix.rs 로직 변경
- `check_forward`: `config.skip_on_english_word && ascii.chars().all(is_ascii_alphabetic)` 조건일 때만 사전 hit 억제. 토글 false면 사전 경로 건너뛰고 음절 임계값 판단으로 진행.
- `check_reverse`: 사전 매칭 전에 `config.skip_on_complete_syllable && !buffer.has_preedit && buffer.committed_chars > 0` 이면 `None` 반환. 해석: 음절 단위 commit + preedit 없음 = 정상 한글 입력으로 판단하여 억제.

## 3. 추가 테스트 목록

| 크레이트 | 테스트 | 결과 |
|----------|--------|------|
| unim | `config::tests::test_auto_typefix_defaults` | PASS |
| unim | `config::tests::test_auto_typefix_clamp` | PASS |
| unim | `config::tests::test_manual_shortcut_defaults` | PASS |
| unim | `config::tests::test_legacy_yaml_backcompat_autotypefix_and_manual_shortcuts` | PASS |
| unim | `config::tests::test_empty_yaml_full_defaults` | PASS |
| unim | `auto_typefix::tests::test_forward_skip_on_english_word_toggle_off` | PASS |
| unim | `auto_typefix::tests::test_forward_skip_on_english_word_off_triggers_for_word` | PASS |
| unim | `auto_typefix::tests::test_reverse_skip_on_complete_syllable_on_suppresses` | PASS |
| unim | `auto_typefix::tests::test_reverse_skip_on_complete_syllable_off_triggers` | PASS |
| unim | `auto_typefix::tests::test_reverse_with_preedit_always_triggers_regardless_of_toggle` | PASS |

## 4. 빌드·테스트 결과

| 검증 레벨 | 명령 | 결과 |
|-----------|------|------|
| L2 | `cargo build --workspace --release` | ✓ zero warning (25s, 모든 workspace crate 컴파일 성공) |
| L2 | `cargo test --workspace` | ✓ 전체 PASS (unim lib 254 passed, unim-dbus 4, unim-gui-common 6, doc-tests 19 등 전부 0 failed / 2 ignored) |
| L1 | `cargo build -p unim` | ✓ zero warning |

## 5. 6지점 커버리지

| 지점 | 상태 | 비고 |
|------|------|------|
| 1. `src/config.rs` | ✅ 완료 | 필드·default·clamp·테스트 모두 추가 |
| 2. `unim-config/src/main.rs` (CLI ConfigKey) | ⏳ Phase 5 이관 | `auto-typefix-skip-on-english-word`, `auto-typefix-skip-on-complete-syllable`, `manual-shortcut-forward`, `manual-shortcut-reverse` 추가 + 범위 변경(2~6, 3~8) clap 반영 필요 |
| 3. `unim-config/locales/*.yml` | ⏳ Phase 5 이관 | ko/en 신규 키 번역 |
| 4. `unim-dbus/src/service.rs` | ⏳ Phase 2 이관 | YAML 통짜 교환이면 자동 커버. 개별 키 인터페이스가 있다면 매치 암 확장 필요. `ConfigChanged` signal 설계 예정 |
| 5. `unim-gui-gtk/src/settings_dialog.rs` | ⏳ Phase 3 이관 | 새 필드 2개를 SwitchRow로, manual_shortcuts 2개를 EntryRow/ShortcutRow로. 범위 SpinRow는 2~6 / 3~8 로 조정 |
| 6. gschema/prefs.js | ⏳ Phase 4 이관 | 원칙상 gschema 잔존 5개만. manual_shortcuts는 GNOME Shell 바인딩 대상이므로 gschema 유지 여부 Phase 4에서 결정 |

⚠️ **누락 방지 플래그**: Phase 5 CLI 반영 시 `kor_syllable_threshold` 유효 범위가 2~5→2~6, `eng_word_min_length`가 5~10→3~8로 바뀌었음. CLI의 clap value_parser(range)도 반드시 업데이트.

## 6. 인수인계

### Phase 2 · dbus-implementer
- `AutoTypeFixConfig`에 2개 필드(`skip_on_english_word`, `skip_on_complete_syllable`), `EngineConfig`에 `manual_shortcuts: ManualShortcutConfig` 추가됨.
- YAML 통짜 직렬화(`GetConfig/SetConfig`) 방식이면 추가 작업 불필요. 개별 키 매치 암이 있으면 확장.
- `ConfigChanged` signal 신설 시 새 필드도 포함되어야 함(자동 — 전체 YAML 교환이면 OK).
- **계약**: `AutoTypeFixConfig::clamp_ranges()` 를 `SetConfig` 수신 시점에 반드시 호출해서 범위 벗어난 값이 엔진에 전달되지 않도록 방어.

### Phase 3 · gtk-designer
- 범위 상수를 config.rs에서 pub const로 노출했으므로 SpinRow 값 설정에 재사용 가능:
  - `AUTO_TYPEFIX_KOR_THRESHOLD_MIN/MAX` (2~6)
  - `AUTO_TYPEFIX_ENG_MIN_LENGTH_MIN/MAX` (3~8)
  - `AUTO_TYPEFIX_TIME_WINDOW_MIN/MAX` (500~5000, step 500은 UI 결정)
- 새 SwitchRow 2개: "영단어 매칭 시 억제"(`skip_on_english_word`), "온전한 음절 매칭 시 억제"(`skip_on_complete_syllable`). 기본 ON.
- `manual_shortcuts.forward/reverse`: Vec<String> — EntryRow 쉼표 구분 또는 ShortcutRow 사용. GNOME 세션에서만 의미 있음을 안내.

### Phase 5 · config-editor (자기 자신)
- unim-config CLI:
  - 신규 kebab 키: `auto-typefix-skip-on-english-word`, `auto-typefix-skip-on-complete-syllable`, `manual-shortcut-forward`, `manual-shortcut-reverse`.
  - 범위 clap 파서 갱신: `kor_syllable_threshold` 2~6, `eng_word_min_length` 3~8.
  - 리스트형(`manual_shortcut_*`)은 쉼표 분리 또는 반복 flag 중 CLI 정책 결정.
- locales/ko.yml, en.yml (및 존재하는 ja/zh) 신규 키 번역.
- `set_value` 호출 후 `clamp_ranges()`로 방어 적용하여 CLI가 범위 밖 값을 YAML에 쓰지 않도록 보장.

### 호환성 메모
- 기존 사용자의 `~/.config/unim/config.yaml`은 신규 필드가 없어도 모두 default로 채워져 로드됨(회귀 테스트 `test_legacy_yaml_backcompat_*` 로 검증).
- 기존 동작(영단어 억제, 역방향 완성-음절 억제)은 기본값 ON으로 그대로 유지됨.
