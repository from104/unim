# Phase 3 — GTK Designer 산출물

## 1. 수정/신규 파일

| 파일 | 변경 요약 |
|------|-----------|
| `unim-gui-gtk/src/settings_dialog.rs` | **전면 재작성** — `Adw.PreferencesWindow` + 3 Page + 8 Group 구성, SwitchRow/SpinRow/ComboRow/EntryRow 사용, 시스템 테마 추종, 변경 즉시 파일 저장 + DBus `SetConfigYaml` fire-and-forget + `Adw.Toast` 피드백 |
| `unim-gui-gtk/src/gtk_ui.rs:157` | `run_settings_only()`에서 `ColorScheme::ForceDark` → `ColorScheme::Default`로 변경. 다이얼로그에만 한정. `run_gtk_app`의 ForceDark(line 32)는 모드 팝업 일관성을 위해 **유지**. |
| `unim-gui-gtk/Cargo.toml:24` | `libadwaita = "0.7"` → `libadwaita = { version = "0.7", features = ["v1_4"] }` — `SwitchRow`/`SpinRow`(1.4+) 활성화 |
| `unim-gui-gtk/Cargo.toml:33-36` | `serde_yaml = "0.9"` 및 `unim-dbus = { path = "../unim-dbus" }` 의존성 추가 (DBus `SetConfigYaml` 호출용) |

## 2. 위젯 트리 (실제 구현)

```
Adw.PreferencesWindow (520x640, title="UNIM 설정", search_enabled=false)
├─ Page "일반" (icon=preferences-system-symbolic)
│  ├─ Group "자판 및 키맵"
│  │  ├─ ComboRow    "한국어 자판"
│  │  ├─ ComboRow    "영어 자판"
│  │  ├─ EntryRow    "한/영 전환 키"          (tooltip: 쉼표 구분)
│  │  ├─ EntryRow    "한자 키"                (tooltip: 쉼표 구분)
│  │  ├─ EntryRow    "수동 순방향 단축키"      (tooltip: GNOME Shell 전용)
│  │  └─ EntryRow    "수동 역방향 단축키"      (tooltip: GNOME Shell 전용)
│  ├─ Group "입력 모드"
│  │  ├─ ComboRow    "초기 입력 모드"         (영문/한글)
│  │  ├─ ComboRow    "모드 공유 방식"         (전역/앱별)
│  │  └─ ComboRow    "팝업 모드"              (독립/내장)
│  └─ Group "자동 전환"
│     ├─ SwitchRow   "사용"
│     └─ SpinRow     "감지 임계값"            (0.0~1.0, step 0.05, digits=2, 연동 sensitive)
├─ Page "오타 교정" (icon=edit-find-replace-symbolic)
│  ├─ Group "자동 순방향 교정 (영→한)"
│  │  ├─ SwitchRow   "사용"                   → auto_typefix.forward
│  │  ├─ SpinRow     "임계 음절 수"           (2~6, step 1)
│  │  ├─ SpinRow     "트리거 윈도우 (초)"     (0.5~5.0, step 0.5, digits=1, ↔ reverse sync)
│  │  └─ SwitchRow   "영단어 매칭 시 억제"    → skip_on_english_word
│  ├─ Group "자동 역방향 교정 (한→영)"
│  │  ├─ SwitchRow   "사용"                   → auto_typefix.reverse
│  │  ├─ SpinRow     "임계 글자 수"           (3~8, step 1)
│  │  ├─ SpinRow     "트리거 윈도우 (초)"     (동일 필드 공유)
│  │  └─ SwitchRow   "온전한 음절 매칭 시 억제" → skip_on_complete_syllable
│  └─ Group "전체 기능"
│     └─ SwitchRow   "자동 오타 교정 사용"    → auto_typefix.enabled (마스터)
└─ Page "GNOME Shell" (is_gnome_session() && gschema 발견 시에만 add)
   ├─ Group "표시"
   │  ├─ SwitchRow   "상단 패널 인디케이터"   (GSettings: show-panel-indicator)
   │  └─ SwitchRow   "변환 알림 표시"          (GSettings: show-notification)
   └─ Group "실시간 입력기"
      └─ SwitchRow   "IME 모드 활성화"        (GSettings: enable-ime, Wayland에서만 sensitive)
```

"방향별 사용" 그룹 → **"전체 기능" 단일 마스터 스위치로 통합**. 각 방향별 사용 스위치는 이미 순방향/역방향 그룹 내에 존재(forward, reverse). 중복 방지.

## 3. 위젯 ↔ Config 필드 바인딩 표

| 위젯 (Page/Group/Row) | 타입 | Config 필드 |
|------------------------|------|-------------|
| 일반/자판및키맵/한국어 자판 | ComboRow | `engine.korean.layout` (KoreanLayout) |
| 일반/자판및키맵/영어 자판 | ComboRow | `engine.english.layout` (EnglishLayout) |
| 일반/자판및키맵/한/영 전환 키 | EntryRow | `engine.toggle_keys: Vec<String>` (쉼표 구분) |
| 일반/자판및키맵/한자 키 | EntryRow | `engine.hanja_keys: Vec<String>` |
| 일반/자판및키맵/수동 순방향 단축키 | EntryRow | `engine.manual_shortcuts.forward: Vec<String>` |
| 일반/자판및키맵/수동 역방향 단축키 | EntryRow | `engine.manual_shortcuts.reverse: Vec<String>` |
| 일반/입력모드/초기 입력 모드 | ComboRow | `engine.default_category` (InputCategory) |
| 일반/입력모드/모드 공유 방식 | ComboRow | `engine.mode_sharing` (ModeSharingMode) |
| 일반/입력모드/팝업 모드 | ComboRow | `engine.popup_mode` (PopupMode) |
| 일반/자동전환/사용 | SwitchRow | `engine.auto_switch.enabled` |
| 일반/자동전환/감지 임계값 | SpinRow | `engine.auto_switch.threshold: f32` |
| 오타교정/순방향/사용 | SwitchRow | `engine.auto_typefix.forward` |
| 오타교정/순방향/임계 음절 수 | SpinRow | `engine.auto_typefix.kor_syllable_threshold: u8` (2~6) |
| 오타교정/순방향/트리거 윈도우 | SpinRow | `engine.auto_typefix.time_window_ms: u32` (초 ↔ ms) |
| 오타교정/순방향/영단어 매칭 시 억제 | SwitchRow | `engine.auto_typefix.skip_on_english_word` |
| 오타교정/역방향/사용 | SwitchRow | `engine.auto_typefix.reverse` |
| 오타교정/역방향/임계 글자 수 | SpinRow | `engine.auto_typefix.eng_word_min_length: u8` (3~8) |
| 오타교정/역방향/트리거 윈도우 | SpinRow | `engine.auto_typefix.time_window_ms` (동일, 양방향 sync) |
| 오타교정/역방향/온전한 음절 매칭 시 억제 | SwitchRow | `engine.auto_typefix.skip_on_complete_syllable` |
| 오타교정/전체/자동 오타 교정 사용 | SwitchRow | `engine.auto_typefix.enabled` |
| GNOME/표시/상단 패널 인디케이터 | SwitchRow | GSettings `show-panel-indicator` |
| GNOME/표시/변환 알림 표시 | SwitchRow | GSettings `show-notification` |
| GNOME/실시간/IME 모드 활성화 | SwitchRow | GSettings `enable-ime` |

범위 상수는 Phase 1의 `AUTO_TYPEFIX_KOR_THRESHOLD_MIN/MAX (2~6)`, `AUTO_TYPEFIX_ENG_MIN_LENGTH_MIN/MAX (3~8)`, `AUTO_TYPEFIX_TIME_WINDOW_MIN/MAX (500~5000 ms)`을 그대로 참조.

## 4. 트리거 윈도우 ms ↔ 초 변환

UI는 **초(step 0.5, digits=1)**로 표시하고 저장은 **ms**(`u32`).

```rust
fn ms_to_seconds(ms: u32) -> f64 { ms as f64 / 1000.0 }
fn seconds_to_ms(secs: f64) -> u32 { (secs * 1000.0).round() as u32 }
```

- SpinRow의 Adjustment: `min = 500/1000 = 0.5`, `max = 5000/1000 = 5.0`, step `0.5`.
- 저장 순간 `(row.value() * 1000).round() as u32` → `time_window_ms`.
- **forward/reverse SpinRow 양방향 sync**: `TimeSyncSlot = Rc<RefCell<(Option<SpinRow>, Option<SpinRow>)>>`. 한쪽 변경 시 반대쪽을 `updating=true` 플래그 아래에서 `set_value()` — 재진입 방지.

## 5. libadwaita 버전 확인

- Cargo 크레이트: `libadwaita 0.7.2`
- 시스템: `pkg-config --modversion libadwaita-1` → **1.5.0**
- `v1_4` feature 활성화 필요 (SwitchRow, SpinRow는 1.4에서 추가, EntryRow는 1.2).
- Cargo.toml에 `features = ["v1_4"]` 추가. Fallback(ActionRow 조합)은 **불필요** — 정상 위젯 사용.

## 6. 저장 흐름

1. 위젯 콜백에서 `state.borrow_mut().config.*` 업데이트
2. `save_and_notify(&config, label)` 호출:
   - `Config::save_to_default_path()` — YAML 파일 저장
   - `serde_yaml::to_string(&config)` → 별도 OS 스레드에서 tokio runtime 생성 → `InputMethodProxy::set_config_yaml(yaml)` fire-and-forget
   - `adw::Toast` "저장됨 ✓" (timeout=2s)
3. GSettings 항목은 DBus 경유 없이 `gio::Settings.set_boolean` 직접.

토스트 표시는 `thread_local!` `ACTIVE_WINDOW` 슬롯을 통해 `PreferencesWindow.add_toast()` 호출 (libadwaita 0.7의 `PreferencesWindow`는 내장 ToastOverlay 제공).

## 7. 재진입 방지

- `SettingsState.updating: bool` 플래그로 초기 바인딩 중 콜백 무시.
- time window SpinRow sync 시 일시적으로 `updating=true` 세팅 → `set_value()` → `updating=false`.

## 8. GNOME 세션 감지

- `XDG_CURRENT_DESKTOP`에 "GNOME" 포함 여부.
- 추가로 `gio::SettingsSchemaSource::default().lookup(GSCHEMA_ID, true)`로 스키마 설치 확인 — 스키마 없으면 페이지 add 생략 (Extension 미설치 개발 환경 대응).
- Wayland 감지는 `XDG_SESSION_TYPE=wayland`로 IME 모드 SwitchRow `sensitive`만 제어.

## 9. 빌드 검증 결과

### `cargo build -p unim-gui-gtk --release`
```
   Compiling unim-gui-gtk v0.0.1 (/home/from104/work/unim/unim-gui-gtk)
    Finished `release` profile [optimized] target(s) in 5.79s
```
**warning 0, error 0.**

### `cargo build --workspace --release`
```
   Compiling libadwaita v0.7.2
   Compiling unim-gui-gtk v0.0.1 (/home/from104/work/unim/unim-gui-gtk)
    Finished `release` profile [optimized] target(s) in 9.98s
```
warning 0.

### `cargo test --workspace`
총 254 passed + 19 passed + 6 passed + 4 passed ... 전체 **0 failed**, 2 ignored(기존).

### `make build` (C/C++ 프론트엔드 포함)
```
✅ UNIM 전체 빌드 완료!
```
warning/error grep 결과 **0건**.

## 10. 수동 테스트 체크리스트

실행 환경 (에이전트 실행 컨텍스트)은 **헤드리스**(`DISPLAY` 없음, `WAYLAND_DISPLAY` 없음)이므로 다음 항목은 **사용자 수동 테스트 필요**:

- [ ] `target/release/unim-gui-gtk --settings` 실행 → PreferencesWindow 표시
- [ ] 3개 Page 탭 확인 (GNOME 세션이면 "GNOME Shell" 탭까지 4개)
- [ ] 시스템 다크모드 테마 자동 추종 확인 (라이트/다크 테마 전환)
- [ ] 순방향 "임계 음절 수" SpinRow 2 → 4 변경 → 토스트 "저장됨 ✓" 표시 → `~/.config/unim/config.yaml` `kor_syllable_threshold: 4` 확인
- [ ] 순방향 "트리거 윈도우" 5.0 → 2.5 변경 → 역방향 SpinRow도 동일하게 2.5로 표시 (sync) 확인, `time_window_ms: 2500` 저장
- [ ] "한/영 전환 키"를 `Korean, RightAlt`로 편집 → toggle_keys 2개 저장
- [ ] daemon 실행 중이면 DBus `ConfigChangedJson` 방출 로그 확인 (`UNIM_DEVELOP=1`)
- [ ] 창 닫은 뒤 다시 열어 값 persist 확인

자동 검증 가능 부분은 모두 통과.

## 11. Phase 4 (gnome-migrator) 인수인계

- `unim-gui-gtk --settings` 진입점 **확정**. prefs.js에서 `Gio.Subprocess.new(['unim-gui-gtk', '--settings'], ...)` 호출하면 설정 다이얼로그만 뜬다.
- 설정 다이얼로그는 **파일 저장 + DBus SetConfigYaml**을 모두 수행하므로, extension은 `ConfigChangedJson` signal을 구독해서 즉시 반영만 하면 됨.
- 다이얼로그는 시스템 테마 자동 추종(ColorScheme::Default) — prefs.js 리다이렉트 버튼 UX는 GNOME 기본 Adwaita와 일치.
- GNOME Shell 페이지의 3개 GSettings 키(`show-panel-indicator`, `show-notification`, `enable-ime`)는 Phase 4에서도 gschema에 **유지**되어야 한다 (Phase 4 플랜의 "유지할 5개"에 포함).
- 삭제 예정 13개 키(`korean-layout` 등)는 이 다이얼로그에서 **참조하지 않음** — 안전하게 gschema에서 제거 가능.

## 12. Phase 5 (config-editor CLI) 인수인계

- GUI와 CLI 모두 동일한 `Config::save_to_default_path()` + `set_config_yaml` DBus를 거친다. CLI도 동일 흐름을 따르면 양쪽 변경이 일관되게 전파됨.
- SpinRow 범위는 이미 Phase 1의 `clamp_ranges()`로 방어되므로 CLI도 같은 함수 호출 권장.

## 13. 결정 및 이슈

- **ForceDark 전역 제거는 하지 않음**: `run_gtk_app`은 모드 팝업/한자 팝업의 디자인 통일을 위해 기존 ForceDark 유지. 설정 다이얼로그(`run_settings_only`)만 Default로 변경. plan의 "ForceDark 제거" 의도는 다이얼로그 범위로 해석.
- **"방향별 사용" 그룹 삭제**: forward/reverse 스위치가 이미 각 섹션에 있으므로 중복. 대신 마스터 `auto_typefix.enabled` 한 개를 "전체 기능" 그룹에 배치.
- **libadwaita v1_4 feature 활성화**: SwitchRow/SpinRow 사용에 필수. 시스템 libadwaita 1.5.0 확인됨.
- **DBus 호출 방식**: `set_config_yaml`(Phase 2 산출물)을 그대로 사용. 별도 OS 스레드 + 임시 tokio runtime으로 fire-and-forget — GTK 메인 스레드 비차단.
