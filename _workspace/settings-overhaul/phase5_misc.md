# Phase 5 — Qt 리다이렉트 · CLI 확장 · locale 번역

## 요약

- **작업 A (Qt 리다이렉트)**: 트레이 "설정" → `unim-gui-gtk --settings` subprocess 기동
- **작업 B (CLI 확장)**: 신규 4 키 + 범위 갱신 (2~6, 3~8) + `clamp_ranges()` 방어
- **작업 C (locale 번역)**: ko/en 4개 라벨 추가. ja/zh는 이 프로젝트에 존재하지 않음

---

## 작업 A — Qt GUI 설정 진입점 리다이렉트

### 수정 파일

| 파일 | 라인 | 변경 |
|------|------|------|
| `unim-gui-qt/src/main.rs` | 28~49 | `SETTINGS_TX` 수신 스레드 신설, `GuiAction::OpenSettings` 수신 시 `Command::new("unim-gui-gtk").arg("--settings").spawn()`. 실패 시 `unim_log!(INDICATOR, ...)` |

### 리다이렉트 대상 진입점

- **원래 동작**: `unim-gui-common/src/tray.rs::open_settings()` (line 157~163) 가 `SETTINGS_TX` 로 `GuiAction::OpenSettings` 전송. Qt GUI의 기존 main.rs에서는 receiver를 `_settings_rx`로 폐기 중이어서 **클릭해도 아무 반응 없음(사실상 NOOP)**.
- **이제 동작**: Qt main 스레드에서 spawn한 수신 스레드가 `OpenSettings` 수신 시 GTK 설정 앱을 서브프로세스로 띄움.
- Qt 독자 설정 UI 위젯 코드: `unim-gui-qt/src/`에는 `main.rs`와 DBus 브릿지용 `bridge.rs`만 존재. **독자 설정 다이얼로그 코드는 애초에 없었음** → 삭제/비활성화 대상 부재.
- `qml/main.qml`도 설정 UI 없음(팝업용). 공용 시그니처 변경 없음.

### Fallback/메시지 박스

- PATH 미존재 시 `Command::spawn()` 은 `Err` 반환 → `unim_log!(INDICATOR, "unim-gui-gtk 실행 실패: {}", e)` 로 기록.
- cxx-qt MessageBox는 현재 브릿지 구조상 Qt 이벤트 루프 외 스레드에서 호출이 복잡. **로그만 기록**하기로 결정. 사용자 가시 에러가 필요하면 추후 Phase 7 QA에서 Qt 트레이 notify로 확장 고려.

---

## 작업 B — unim-config CLI ConfigKey 확장

### 수정 파일

| 파일 | 라인 | 변경 |
|------|------|------|
| `unim-config/src/main.rs` | 5~10 | `AUTO_TYPEFIX_*_MIN/MAX` 상수 import |
| `unim-config/src/main.rs` | 87~98 | `ConfigKey` enum에 4개 변형 추가: `AutoTypeFixSkipEnglishWord`, `AutoTypeFixSkipCompleteSyllable`, `ManualShortcutForward`, `ManualShortcutReverse` |
| `unim-config/src/main.rs` | 74, 77 | doc comment 범위 표기 2~5→**2~6**, 5~10→**3~8** |
| `unim-config/src/main.rs` | 378~412 | `AutoTypeFixKorThreshold` / `AutoTypeFixEngMinLength` / `AutoTypeFixTimeWindow` 매치 암의 하드코딩 범위를 `AUTO_TYPEFIX_*_MIN/MAX` 상수로 교체 |
| `unim-config/src/main.rs` | 428~485 | 신규 4개 매치 암 (bool 토글 2개 + 쉼표 구분 Vec<String> 2개) |
| `unim-config/src/main.rs` | 454~455 (신규) | 저장 직전 `config.engine.auto_typefix.clamp_ranges()` 방어 호출 |

### 신규 CLI 키

| kebab 이름 | config 필드 | 값 타입 | 비고 |
|------------|-------------|---------|------|
| `auto-typefix-skip-english-word` | `engine.auto_typefix.skip_on_english_word` | bool | 기본 true |
| `auto-typefix-skip-complete-syllable` | `engine.auto_typefix.skip_on_complete_syllable` | bool | 기본 true |
| `manual-shortcut-forward` | `engine.manual_shortcuts.forward` | `Vec<String>` | 쉼표 구분 |
| `manual-shortcut-reverse` | `engine.manual_shortcuts.reverse` | `Vec<String>` | 쉼표 구분 |

### 기존 키 사용자 호환성

- 기존 키(`auto-typefix-kor-threshold` 등)의 이름과 동작은 보존. **값 범위만 확장**: 2~5 → 2~6, 5~10 → 3~8. 확장 방향이라 기존 스크립트 비호환 없음.
- 기존 사용자의 `config.yaml`에 신규 필드가 없어도 `#[serde(default)]` 로 자동 채움(Phase 1에서 회귀 테스트 완료).

---

## 작업 C — locale 번역

### 수정 파일

| 파일 | 변경 |
|------|------|
| `unim-config/locales/ko.yml` | `auto_typefix_label` 다음 줄에 4개 키 추가 |
| `unim-config/locales/en.yml` | 동일 위치에 4개 키 추가 |

### 번역 표

| 키 | ko | en | 사용자 리뷰 |
|---|---|---|:-:|
| `auto_typefix_skip_english_word_label` | 영단어 매칭 시 억제 | Skip on English word match | - |
| `auto_typefix_skip_complete_syllable_label` | 온전한 음절 매칭 시 억제 | Skip on complete syllable match | - |
| `manual_shortcut_forward_label` | 수동 순방향 단축키 | Manual forward shortcut | - |
| `manual_shortcut_reverse_label` | 수동 역방향 단축키 | Manual reverse shortcut | - |

### 로캘 커버리지 메모

- 프로젝트 현재 `unim-config/locales/`에는 **ko.yml, en.yml 두 파일만** 존재. ja/zh 등 추가 로캘 파일 없음 → 해당 항목 **N/A**.
- `rust-i18n` 매크로(`i18n!("locales")`)가 파일 목록을 그대로 로드하므로 신규 키가 ko/en 양쪽에 존재하면 누락 없음.

---

## 검증 결과

### 빌드·테스트

| 검증 레벨 | 명령 | 결과 |
|-----------|------|:-:|
| L2 | `cargo build -p unim-config --release` | ✓ zero warning (18s) |
| L2 | `cargo build --workspace --release` | ✓ zero warning (17s) |
| L2 | `cargo test --workspace` | ✓ all pass (Phase 1 신규 테스트 포함 전체 통과, 0 failed / 2 ignored) |
| L3 | `make build` | ✓ zero warning / zero error (grep 결과 무음) |

### CLI 동작 (임시 `$XDG_CONFIG_HOME` 환경)

```
$ unim-config set auto-typefix-skip-english-word false
영단어 매칭 시 억제: OFF
설정이 저장되었습니다.                                   ✓

$ unim-config set auto-typefix-skip-complete-syllable true
온전한 음절 매칭 시 억제: ON                            ✓

$ unim-config set auto-typefix-kor-threshold 6
한글 음절 임계값: 6                                      ✓ (신규 상한)

$ unim-config set auto-typefix-kor-threshold 7
Range 2~6, got 7                                         ✓ (범위 에러 반환)

$ unim-config set auto-typefix-eng-min-length 3
영문 단어 최소 길이: 3                                   ✓ (신규 하한)

$ unim-config set auto-typefix-eng-min-length 9
Range 3~8, got 9                                         ✓

$ unim-config set manual-shortcut-forward "<Ctrl>k"
수동 순방향 단축키: <Ctrl>k                              ✓

$ unim-config set manual-shortcut-reverse "<Shift><Ctrl>k,<Super>j"
수동 역방향 단축키: <Shift><Ctrl>k, <Super>j             ✓ (쉼표 구분 정상)
```

### 로캘 동작

- clap `--help` 텍스트는 매크로 정적 doc comment라 runtime locale 반영되지 않음(기존 구조 유지).
- 신규 라벨은 `t!()` 동적 조회라 `LANG=en_US.UTF-8`에서 `config_set` 실행 시 "Skip on English word match: OFF" 형태로 출력되는 구조. set 출력 경로에서 직접 확인하려면 `LANG=en_US.UTF-8 unim-config set auto-typefix-skip-english-word false` 수동 점검 권장 (자동 검증 스크립트에서는 config 쓰기 부작용 때문에 제외).

---

## Phase 6 · daemon-migrator 인수인계

- **CLI에서 신규 키 세팅 시 자동 `clamp_ranges()` 호출**이 이미 들어감 → 마이그레이션 루틴에서도 반드시 `clamp_ranges()` 호출 후 저장 권장. 특히 gschema `auto-typefix-kor-threshold` 기존 값(2~5 범위)은 새 범위(2~6)와 호환되나, 외부 주입 값이 범위 밖일 수 있음.
- gschema → config.yaml 이관 시 필드 매핑:
  - `shortcut-normal` → `engine.manual_shortcuts.forward`
  - `shortcut-normal-reverse` → `engine.manual_shortcuts.reverse`
  - 기존 `auto-typefix-*` gschema 키 → `engine.auto_typefix.*` (이름 그대로)
- 신규 필드 2개(`skip_on_english_word`, `skip_on_complete_syllable`)는 gschema에 존재한 적 없음 → 마이그레이션 불필요, default 유지.

## Phase 7 · reviewer 인수인계

- `cargo build --workspace --release`, `cargo test --workspace`, `make build` 모두 zero warning / all pass 확인됨.
- 수동 E2E 체크리스트 항목:
  1. Qt 트레이 "설정" 클릭 → `unim-gui-gtk` 기동 확인 (`UNIM_DEVELOP=1` 로그에 `[INDICATOR] unim-gui-gtk --settings 기동`).
  2. PATH에 `unim-gui-gtk` 없는 환경에서 트레이 "설정" 클릭 → 로그에 `unim-gui-gtk 실행 실패: ...` 기록 확인.
  3. `unim-config set auto-typefix-kor-threshold 6` 후 GTK GUI에서 SpinRow 상한이 6까지 도달하는지 확인.
  4. `LANG=en_US.UTF-8 unim-config set auto-typefix-skip-english-word false` → "Skip on English word match: OFF" 영문 출력 확인.

## 잔존 과제 / 누락 플래그

- Qt MessageBox fallback 미구현 (로그만). 필요 시 Phase 7 또는 후속 작업에서 `Notify` crate 또는 KNotifications로 확장 가능.
- 기존 `t!("error_label", error=...)` 템플릿이 `실행 오류: %{error}` 에 대해 `rust_i18n` 변수 치환이 실패하는 선행 버그 관찰(`실행 오류: %{error}: Range ...`). 본 Phase 범위 아님. 별도 이슈로 기록 권장.
