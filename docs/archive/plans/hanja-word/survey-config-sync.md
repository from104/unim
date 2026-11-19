# 설정 동기화 지점 전수

조사 대상: UNIM(`/home/from104/work/unim`, branch `develop`, HEAD `ba64255`). 목표: 「한자 단어 입력」기능이 새로 추가할 설정(`hanja_output_format`, `hanja_word_conversion` 등)이 실제로 몇 곳을, 어떤 시그니처로 건드려야 하는지 file:line 근거로 확정.

## 0. 결론 먼저 — 문서(GEMINI.md) vs 실제 코드 불일치 (중요)

`docs/dev/architecture/GEMINI.md:40-57`은 "일반 설정 연동 대상 5지점"을 다음과 같이 적어 두었다:

1. `src/config.rs`
2. `unim-cli/src/main.rs` (`ConfigKey`)
3. `unim-cli/locales/*.yml`
4. `unim-dbus/src/service.rs` (레거시 key 디스패치)
5. `unim-gui-gtk/src/gtk_ui.rs` ← **이 파일은 저장소에 존재하지 않는다** (확인: `ls unim-gui-gtk` → 디렉터리 자체가 없음, `rg -l CommitUnit` 결과에도 없음)

실제로는:
- GUI는 **두 개**의 독립 바이너리다 — `unim-settings-gtk`(GTK4/libadwaita, Linux 전용, `unim-settings-gtk/src/settings_dialog.rs`)와 `unim-settings`(Slint, Windows/Linux 크로스플랫폼, `unim-settings/src/main.rs`). GEMINI.md는 이 개편(옛 `unim-gui-gtk`→`unim-settings-gtk` 개명 + Slint 앱 신설) 이전 상태를 기술한 채로 멈춰 있다.
- Windows에는 **세 번째** 설정 UI가 아직 살아 있다 — `unim-tsf/src/settings_dialog.rs`(순수 Win32 네이티브 모달, `unim-tsf/src/fn_configure.rs:45`·`unim-tsf/src/lang_bar.rs:799`에서 여전히 호출됨). `unim-settings/src/main.rs:3-6` 주석은 "Windows에서 DLL 내부 모달의 메시지펌프 충돌을 피하려고 별도 프로세스로 분리했다(구 unim-tsf-settings)"고 적지만, 구 `unim-tsf/src/settings_dialog.rs`는 삭제되지 않고 여전히 `mod` 선언(`unim-tsf/src/lib.rs:39`)돼 있고 `CommitUnit` 필드까지 갖고 있다(§2 참고). **레거시 코드가 죽지 않고 실사용 경로에 남아있을 위험**.
- `unim-cli/locales/*.yml`은 실재하고 정확히 그 역할을 한다(§4).
- `unim-dbus/src/service.rs`의 레거시 key 디스패치(`get_config`/`set_config`)는 실재하지만 **불완전** — `commit_unit`·`word_mode_apps`는 애초에 이 레거시 목록에 없다(§3.3). GUI들은 이제 `GetConfigYaml`/`SetConfigYaml`(전체 구조체 serde)을 1차 경로로 쓰므로 신규 필드는 이 레거시 디스패치를 건드리지 않아도 동작한다.
- **GEMINI.md가 언급조차 안 하는 6번째 실제 동기화 지점**이 있다: `unim-gui-common/src/settings_helpers.rs::merge_gtk_ui_owned`와 `unim-settings/src/main.rs::merge_ui_owned` — "이 세션에서 사용자가 UI로 실제로 건드린 필드만 disk 값을 덮어쓴다"는 **필드 화이트리스트 병합 로직**. 이 목록에 새 필드를 안 넣으면, 설정창이 열려 있는 동안 다른 프로세스(데몬/CLI)가 그 필드를 바꿔도 저장 시 씹히거나, 반대로 그 필드를 UI가 편집하는데 목록에 없으면 세션 시작 시점 값으로 롤백된다(§3.5). **이 항목이 실질적으로 가장 잘 빠뜨리기 쉬운 sync point다.**

→ 설계자는 GEMINI.md의 "5지점" 문구를 그대로 믿지 말고, 아래 §1의 실제 지점 목록(7개 + 조건부 2개)을 체크리스트로 쓸 것.

## 1. 핵심 파일과 역할

| # | 파일 | 역할 |
|---|------|------|
| 1 | `src/config.rs` | 설정 구조체·직렬화 정의(Source of Truth), enum 패턴, 파일 로드/저장/리로드, `clamp_ranges` |
| 2 | `unim-cli/src/main.rs` | `ConfigKey` enum(clap ValueEnum) + `config set/show/interactive` 서브커맨드 |
| 3 | `unim-cli/locales/{ko,en}.yml` | CLI 라벨·에러·enum 표시값·`--help` 텍스트(rust-i18n) |
| 4 | `unim-dbus/src/service.rs` | DBus `GetConfigYaml`/`SetConfigYaml`/`GetConfigJson`(전체 구조체, 자동 반영) + 레거시 `get_config`/`set_config`(key 단위, **수동 유지·일부 필드 누락**) |
| 5 | `unim-settings-gtk/src/settings_dialog.rs` | Linux GTK4/libadwaita 설정 다이얼로그(독립 프로세스). ComboRow/Scale 위젯 + `save_and_notify` |
| 6 | `unim-settings/src/main.rs` | Windows/Linux 공용 Slint 설정 앱 + 첫 실행 마법사. `.slint` UI 모델과 바인딩 |
| 7 | `unim-gui-common/src/settings_helpers.rs` | GTK 다이얼로그의 "UI-소유 필드 병합" 헬퍼(`merge_gtk_ui_owned`) + baseline 관리 |
| 8 | (조건부) `unim-tsf/src/settings_dialog.rs` | Windows 레거시 Win32 네이티브 모달. 아직 호출되므로 새 필드를 노출하려면 여기도 건드려야 함(또는 의도적으로 제외 명시) |
| 9 | (조건부, GNOME Shell 의존 키 한정) `unim-gnome-extension/prefs.js` + `schemas/org.gnome.shell.extensions.unim.gschema.xml` | 일반 설정은 여기 없음. `unim-settings`(Slint) 실행 바로가기만 제공 |
| — | `unim-capi/src/lib.rs` | C ABI. `Config` 불투명 포인터 + **극소수 필드 전용 setter**(korean/english layout 등)만 노출. 신규 필드가 C 프런트엔드에서 직접 편집될 필요가 없으면 손댈 필요 없음(§3.7) |
| — | `unim-daemon/src/migration.rs` | Phase 4 GSettings 폐기 1회성 이관 전용. **신규 필드 추가와 무관**(§3.8) — serde `#[serde(default)]`가 자동 처리 |

## 2. 핵심 타입·함수 (file:line · 시그니처 · 역할)

### 2.1 enum 설정 패턴 — `CommitUnit`을 표본으로

- 정의: `src/config.rs:58-73`
  ```rust
  #[derive(Clone, Copy, Debug, PartialEq, Eq, Default, Serialize, Deserialize)]
  #[repr(C)]
  pub enum CommitUnit { Syllable, Word, #[default] Smart }
  ```
  - **serde rename 없음** — variant 이름 그대로(`Syllable`/`Word`/`Smart`)가 YAML/JSON 태그. `#[serde(rename_all=...)]` 미사용, 대문자 카멜 그대로 직렬화됨. 새 enum도 이 관례를 따르면 됨(별도 rename 불필요, 단 `rename_all = "snake_case"` 같은 것도 이 프로젝트 관례엔 없다는 뜻).
  - `#[repr(C)]` — C ABI 노출 대비(`unim-capi`). 새 enum도 C에 안 노출할 계획이어도 기존 관례상 붙여두는 편이 일관적(강제는 아님, `InputCategory`/`ModeSharingMode`도 동일 패턴 — `src/config.rs:12,22`).
- `display_name()` / `all()`: `src/config.rs:76-90`
  ```rust
  impl CommitUnit {
      pub fn display_name(&self) -> &'static str { .. } // 한국어 하드코딩 리터럴(코어는 로케일 없음)
      pub fn all() -> &'static [CommitUnit] { &[..] }
  }
  ```
  - **주의**: `display_name()`이 반환하는 문자열은 **UI 라벨이 아니라 "값"** 인데도 한국어 리터럴이다. 코어는 로케일 개념이 없다는 설계 원칙(`unim-cli/src/main.rs:47-52` 주석) 때문. CLI/GUI는 각자 로케일 키로 다시 매핑해서 쓴다(§2.2, §2.4) — `display_name()`을 그대로 사용자에게 보여주면 영어 로케일에서도 한국어가 나온다. **새 enum도 `display_name()`은 참고용 fallback으로만 두고, 실제 노출은 각 프런트엔드의 i18n 키로 매핑해야 함.**
- `Default`는 파생이 아니라 `#[default]` 어트리뷰트로 지정 — variant 재배치는 절대 금지(`#[repr(C)]` 판별자 순서 불변, 주석: `src/config.rs:70-72`).

### 2.2 설정 파일 포맷·경로

- 경로: `Config::default_config_path()` — `src/config.rs:1027-1029`
  ```rust
  pub fn default_config_path() -> Option<PathBuf> {
      crate::paths::config_dir().map(|p| p.join("unim").join("config.yaml"))
  }
  ```
  Linux `~/.config/unim/config.yaml`, macOS `~/Library/Application Support/unim/config.yaml`, Windows `%APPDATA%\unim\config.yaml`.
- 로드: `Config::load_from_path` — `src/config.rs:1265-1277`. `serde_yaml::from_str` → `config.engine.auto_typefix.clamp_ranges()` 호출 → `last_modified`/`last_checked` 스냅샷.
- 저장: `Config::save_to_path` — `src/config.rs:1298-1303`. `serde_yaml::to_string` → `crate::atomic_io::atomic_write`(PID+나노초 tmp파일 + rename, 동시 저장 프로세스 간 경합 방지).
- 리로드: `needs_reload()`/`reload_if_changed()`(2초 throttle)/`reload_now()`(throttle 우회) — `src/config.rs:1349-1420` 부근. mtime 비교 기반.
- `sync_last_modified()` — `src/config.rs:1319-1327`: 저장 직후 자기 메아리(자기가 쓴 파일을 외부 변경으로 오인) 방지용 스냅샷 갱신. **DBus `SetConfigYaml`(`unim-dbus/src/service.rs:1241`)와 GTK `save_and_notify`(`unim-settings-gtk/src/settings_dialog.rs:179-203`) 양쪽 모두 저장 후 이 함수(또는 동등 로직)를 호출한다 — 새 저장 경로를 추가한다면 이 호출을 빠뜨리면 안 됨.**

### 2.3 섹션 구조

`Config`(`src/config.rs:1005-1013`)는 사실상 단일 섹션이다:
```rust
pub struct Config {
    pub engine: EngineConfig,          // 유일한 실 데이터 섹션
    #[serde(skip)] pub last_modified: Option<SystemTime>,
    #[serde(skip)] pub last_checked: Option<SystemTime>,
}
```
`EngineConfig`(`src/config.rs:942-982`) 필드: `default_category, mode_sharing, korean: KoreanConfig, english: EnglishConfig, toggle_keys, hanja_keys, app_rules, auto_typefix: AutoTypeFixConfig, auto_english: AutoEnglishConfig, toggle_announce_beep, ignore_key_repeat`.

**"popup" 섹션은 없음** (확인: `rg -n "popup" src/config.rs` → 0건). 한자 팝업 관련 상태(즐겨찾기 등)는 config.rs 밖의 **별도 사용자 데이터 파일**로 관리된다(§4.4). 새 `hanja_output_format`/`hanja_word_conversion`은 `KoreanConfig`(한자는 한글 음절/단어 변환이므로 자연스러운 위치) 또는 `EngineConfig` 최상위에 추가하는 두 선택지가 있음 — 기존 `hanja_keys`가 `EngineConfig` 최상위에 있는 것과의 일관성을 보면 최상위도 타당. **설계자 판단 필요(미해결 질문 참고).**

### 2.4 마이그레이션 패턴 — `unim-daemon/src/migration.rs`

- 목적: Phase 4 설정 개편에서 **GNOME gschema에서 삭제된 13개 키**를 `config.yaml`로 1회 이관(`unim-daemon/src/migration.rs:1-19`).
- 가드: `~/.config/unim/.migrated-v2`(`GUARD_FILENAME`, `migration.rs:29`). 존재하면 skip. `dconf` 미설치 시도 skip.
- 정책: "config.yaml이 아직 기본값인 필드만" dconf 값으로 덮어씀(사용자가 GTK/CLI에서 이미 수정한 값 보존) — `migration.rs:10-13`.
- 에러 정책: 실패는 비치명적, 가드 미생성으로 재시도 유도(`migration.rs:15-18`).
- **결론: 이 파일은 "GSettings→YAML 폐기 이관" 전용 1회성 로직이며, 새 필드를 config.rs에 추가하는 것과는 무관하다.** `#[serde(default)]`(구조체 레벨, `src/config.rs:1006`) 덕분에 새 필드가 없는 구 config.yaml도 자동으로 기본값을 채워 파싱된다 — `hanja_output_format`/`hanja_word_conversion` 추가에 migration.rs 수정 불필요. (레거시 필드→신필드 변환이 필요한 경우에만 `KoreanConfigCompat`류의 struct-level `#[serde(from = "...")]` 브리지를 쓴다 — §2.5.)

### 2.5 구조체 레벨 호환 브리지 패턴 (`KoreanConfigCompat`) — 참고용

`KoreanConfig`는 `#[serde(default, from = "KoreanConfigCompat")]`(`src/config.rs:604-605`)로 구 필드(`word_commit: bool`, `custom_layout` 등)를 흡수한다. `KoreanConfigCompat`(`src/config.rs:790-816`) → `impl From<KoreanConfigCompat> for KoreanConfig`(`src/config.rs:835-869`)에서 예: `commit_unit` 미지정 + `word_commit: true` → `CommitUnit::Word`로 승격(`src/config.rs:846-847`). **완전히 새로운 필드(레거시 이름 충돌 없음)라면 이 패턴은 불필요** — `#[serde(default)]` 필드 하나 추가로 충분.

## 3. 현재 동작 흐름 (단계별, 호출 순서) — `CommitUnit`을 예시로

### 3.1 `src/config.rs` 정의
`KoreanConfig.commit_unit: CommitUnit`(`src/config.rs:664`, `#[serde(default)]`), `Default` 구현에서 `CommitUnit::default()`(`src/config.rs:685`).

### 3.2 CLI (`unim-cli/src/main.rs`)
1. `ConfigKey` enum에 variant 추가 — `unim-cli/src/main.rs:616-617`:
   ```rust
   #[value(name = "commit-unit", help = h("help_ck_commit_unit"))]
   CommitUnit,
   ```
2. 표시용 로케일 어댑터 함수(코어 `display_name()`을 감싸 i18n 키로 재매핑) — `unim-cli/src/main.rs:74-79`:
   ```rust
   fn commit_unit_display_name_localized(unit: CommitUnit) -> String {
       match unit {
           CommitUnit::Syllable => t!("commit_unit_syllable").to_string(),
           ...
       }
   }
   ```
3. `config set` 매치 암 — `unim-cli/src/main.rs:1701-1723`: 영문 키워드 + 한국어 별칭 이중 수용(`"syllable" | "음절"` 패턴), 잘못된 값은 `t!("error_invalid_commit_unit", value=.., allowed=..)`로 에러.
4. `config show` 출력 — `unim-cli/src/main.rs:886-890`: `t!("commit_unit_label")` + `commit_unit_display_name_localized(..)` + `t!("commit_unit_note")`.
5. **`config interactive`(대화형 메뉴)에는 `commit_unit`이 없다** — `unim-cli/src/main.rs:1806-1814`의 `options` 목록(한국어 레이아웃/영어 레이아웃/기본 카테고리/모드 공유/토글 키/한자 키/리셋/저장/취소)에 `commit_unit`도 `word_mode_apps`도 빠져 있다. **실제 관례는 "`config set`엔 다 넣지만 대화형 메뉴는 선택적으로만 넣는다"임을 확인** — 새 설정도 대화형 메뉴 추가는 필수가 아니라 설계자 재량.
6. `tools/gen-help`는 **CLI `--help` 텍스트 생성기가 아니다** — 이것은 `docs/user/**/*.md`(사용자 매뉴얼 4종) → `help/unim-help-{ko,en}.html`(오프라인 HTML)을 병합하는 별도 도구다(`tools/gen-help/src/main.rs:1-20`). `help_ck_*` i18n 키는 clap derive `#[value(help = h(..))]`가 런타임에 직접 읽으며 코드 생성 단계가 없다. **task 지시문의 "help i18n 키·tools/gen-help" 연결은 실제로는 두 개의 독립된 것** — 새 설정을 문서화하려면 (a) `help_ck_*` i18n 키 추가(코드), (b) `docs/user/**/*.md`에 설명 추가(문서, 있으면 gen-help가 다음 빌드 때 자동으로 HTML에 반영, 코드 수정 불필요)를 별개로 챙길 것.

### 3.3 DBus (`unim-dbus/src/service.rs`)
- **1차 경로(자동)**: `get_config_yaml`(`service.rs:1161-1166`)/`set_config_yaml`(`service.rs:1187-1265`)/`get_config_json`(`service.rs:1172-1177`)이 `Config` 전체를 serde로 직렬화/역직렬화한다. 새 필드는 **코드 수정 없이 자동으로** 왕복된다. `set_config_yaml`은 파싱 → `clamp_ranges()` → 필수 키 목록 검증(`toggle_keys`/`hanja_keys`/`auto_english_keys`, `service.rs:1201-1224`) → `save_to_default_path()` → 공유 `Arc<RwLock<Config>>` 갱신 → `ConfigChangedJson` signal(payload=JSON, `service.rs:1257`) 방출.
- **2차 경로(레거시, key 단위)**: `get_config`(`service.rs:719-799`)/`set_config`(`service.rs:812-`)는 `match key { "korean_layout" => .., ... }` 식 수동 화이트리스트. **`commit_unit`·`word_mode_apps`는 이 목록에 없음**(확인: `rg -n '"commit_unit"' unim-dbus/src/service.rs` → 0건) — `korean_bidirectional_combine`/`korean_chord_window_ms`는 있는데(`service.rs:783-789`) `commit_unit`은 빠졌다. 즉 최근 필드일수록 레거시 디스패치에 안 실리는 경향 → **새 필드도 레거시 key 디스패치에 넣을 필요 없음**(GetConfigYaml/JSON이 사실상의 SSoT 경로). 넣고 싶다면 `get_config`/`set_config`에 각각 한 개의 `match` 암만 추가하면 됨.
- 클라이언트 구독: GNOME extension 등은 `ConfigChangedJson`(전체 payload)만 구독하고 수신 시 캐시를 통째로 교체한다(`service.rs:287-288` 주석) — 새 필드를 프런트엔드가 실시간 반영하려면 그 프런트엔드의 JSON 파싱 스키마에도 필드를 추가해야 함(JS는 동적 타입이라 보통 자동으로 통과하지만, 명시적으로 구조체/인터페이스를 두는 프런트엔드는 별도 확인 필요).

### 3.4 GTK 설정 (`unim-settings-gtk/src/settings_dialog.rs`)
enum → `adw::ComboRow` 패턴(수치 아닌 설정, "초기모드·모드공유와 동일 관례" 주석 — `settings_dialog.rs:691`):
```rust
let commit_row = adw::ComboRow::builder().title(t!("row_commit_unit")).subtitle(t!("row_commit_unit_subtitle")).build();   // :692-695
commit_row.set_tooltip_text(Some(t!("row_commit_unit_tooltip").as_ref()));                                                  // :696
let commit_list = gtk4::StringList::new(&[t!("commit_unit_syllable").as_ref(), ..]);                                        // :697-701
commit_row.set_model(Some(&commit_list));                                                                                   // :702
// 초기값 반영: s.config.engine.korean.commit_unit → index                                                                  // :704-709
// 변경 콜백: connect_selected_notify → index → enum → save_and_notify(&s.config, "commit_unit")                            // :711-724
```
- 저장 흐름 `save_and_notify`(`settings_dialog.rs:179-203`): ① `Config::load_from_default_path()`로 최신 disk 재로드 ② `merge_gtk_ui_owned(&mut disk, config)`(§3.5) ③ `disk.save_to_default_path()` ④ `set_gtk_config_baseline(&disk)`(세션 baseline 갱신, "껐다 다시 켜서 세션 시작값으로 복귀" 오판 방지) ⑤ `save_config_via_dbus(&disk, label)`(fire-and-forget `SetConfigYaml`) ⑥ 토스트("저장됨 ✓", 2초 자동 소멸).
- **슬라이더 정책**(메모리 `feedback_slider_for_numeric.md`와 일치 확인): 수치 입력은 `SpinRow` 금지, `build_int_scale_row`(`settings_dialog.rs:843-865`) 공용 헬퍼 사용 — `AdwActionRow` + `gtk4::Scale`(`draw_value=true`, tick 마크, step=1 정수). 시그니처:
  ```rust
  fn build_int_scale_row(state: &State, title: &str, subtitle: &str, tooltip: &str,
                          min: i32, max: i32, init: i32, label: &'static str,
                          set: impl Fn(&mut Config, i32) + 'static) -> adw::ActionRow
  ```
  `hanja_output_format`은 enum(3택1)이므로 ComboRow, `hanja_word_conversion`이 bool이면 `adw::SwitchRow`(다른 bool 설정, 예 `toggle_announce_beep`의 실제 위젯 코드는 미확인 — 별도 `rg -n "SwitchRow" unim-settings-gtk/src/settings_dialog.rs`로 대조 권장, 이번 조사 범위 밖).

### 3.5 GTK-소유 필드 병합 (`unim-gui-common/src/settings_helpers.rs`) — **놓치기 쉬운 6번째 sync point**
`merge_gtk_ui_owned(disk: &mut Config, ui: &Config)`(`settings_helpers.rs:170-207`)는 "GTK-소유 필드" 화이트리스트를 하드코딩한다:
```
engine.{default_category, mode_sharing, toggle_keys, hanja_keys,
        toggle_announce_beep, ignore_key_repeat, auto_typefix, auto_english}
engine.korean.{layout, active_rule_sets, layout_rule_sets,
               bidirectional_combine, chord_window_ms, commit_unit}
engine.english.layout
```
각 필드는 `merge_field(dst, baseline, ui)`(정의는 `unim-settings/src/main.rs:511-516`과 동일 패턴이 `unim-gui-common`에도 있음 — **정확한 정의 위치는 `unim-gui-common/src/settings_helpers.rs` 내부, 이번 조사에서 라인 미확정, `rg -n "fn merge_field" unim-gui-common/src/settings_helpers.rs`로 확정 필요**)로 "이 세션에서 UI가 실제로 그 필드를 건드렸는가"를 baseline과 비교해 판정한다. **주석 명시(`settings_helpers.rs:154-156`): GTK 레거시 다이얼로그는 `app_rules`/`korean.word_mode_apps`를 편집하지 않으므로 이 목록에 없다** — 이는 "GTK 다이얼로그가 그 필드의 UI를 아예 갖고 있지 않다"는 뜻이지 "필드가 존재 안 한다"는 뜻이 아님에 주의.
→ **새 필드를 GTK 다이얼로그에 노출하면 반드시 이 목록에 `merge_field` 호출을 추가**해야 한다. 안 그러면: 설정창이 열려 있는 동안 데몬/CLI가 그 필드를 바꿔도 GTK 창에서 아무거나 저장하는 순간 세션 시작 시점 값(=합쳐지지 않은 옛 값)으로 덮여씀 — 정확히는 반대로, 목록에 없으면 그 필드는 `disk`(외부값) 그대로 유지되어 GTK가 그 필드를 편집해도 저장이 반영 안 됨.

### 3.6 Slint 설정 (`unim-settings/src/main.rs`) — Windows/Linux 공용
동일 패턴, 완전히 별도의 구현체(코드 공유 없음, `unim-gui-common` 미의존 — Cargo.toml 확인: `unim-settings/Cargo.toml`는 `unim = {path=".."}`만 core 의존, `unim-gui-common` 없음):
- import: `unim-settings/src/main.rs:24` `use unim::config::{.., CommitUnit, ..}`
- 콤보 옵션 채우기: `ui.set_commit_unit_options(string_model(CommitUnit::all()....))`(`main.rs:842-848`), 선택 인덱스: `ui.set_commit_unit_index(CommitUnit::all().iter().position(...))`(`main.rs:849-853`)
- 저장 시 역변환: `let cu_idx = ...; e.korean.commit_unit = cu_all[cu_idx];`(`main.rs:935-938`)
- `merge_ui_owned(disk, ui)`(`main.rs:527-597`, 시그니처 주석은 `main.rs:520-529`) — GTK판보다 **화이트리스트가 더 넓다**: `app_rules`와 `korean.word_mode_apps`가 **포함됨**(`main.rs:544, 580-584`) + `ignore_key_repeat`은 "양 플랫폼 UI-소유"라 무조건 UI 값 채택(주석 `main.rs:529-530`, 코드 위치는 merge_ui_owned 마지막 줄 근방, 이번 조사에서 정확한 라인 미확정).
- `.slint` UI 파일 자체는 이번 조사에서 미열람 — `set_commit_unit_options`/`set_commit_unit_index`가 codegen 프로퍼티이므로 **`.slint` 소스에 해당 프로퍼티 선언이 반드시 있어야** 컴파일된다(파일 위치는 `unim-settings/ui/*.slint` 추정, 확인 필요 — 미해결 질문).
- Slint UI도 `merge_field`/`debug_eq` 자체 구현을 갖고 있음(`main.rs:504-516`) — **GTK와 Slint가 병합 로직을 각각 독립적으로 복붙 구현**하고 있다(공유 크레이트 미사용). 새 필드를 두 GUI 모두에 노출하면 이 중복된 두 화이트리스트(§3.5, §3.6)에 **각각** 추가해야 하며 하나를 빠뜨리는 실수가 나기 쉬운 구조.

### 3.7 Windows 레거시 네이티브 모달 (`unim-tsf/src/settings_dialog.rs`)
- 아직 `unim-tsf/src/fn_configure.rs:45`(`crate::settings_dialog::show_settings_dialog(hwndparent)`), `unim-tsf/src/lang_bar.rs:799`에서 호출된다.
- `CommitUnit::all()`로 콤보 아이템 구성(`settings_dialog.rs:689-691`), 저장 시 인덱스→enum 역변환(`settings_dialog.rs:1247-1252`).
- 순수 Win32(`SysTabControl32`/`msctls_trackbar32`/`CBS_DROPDOWNLIST`), i18n 방식은 CLI/GTK의 `t!()`(rust-i18n)와 다를 가능성 높음(미확인 — 이번 조사 범위 밖, `rg -n "t!\(" unim-tsf/src/settings_dialog.rs`로 확인 필요).
- **이 파일이 "죽은 코드/의도적으로 유지 중인 폴백"인지, "실사용 중이라 새 설정도 반드시 반영해야 하는 활성 UI"인지가 불명확** — 미해결 질문으로 이관.

### 3.8 C ABI (`unim-capi/src/lib.rs`)
`Config`를 불투명 포인터로 노출(`unim_config_load`/`unim_config_default`/`unim_config_delete`, `lib.rs:66-101`), 리로드 체크(`unim_config_needs_reload`/`unim_config_reload`, `lib.rs:115-130`)까지는 전체 구조체 단위라 신규 필드가 자동 왕복된다. 다만 **개별 필드 setter는 화이트리스트**다 — `unim_config_set_korean_layout`(`lib.rs:235-`), `unim_config_set_english_layout`(`lib.rs:255-`) 등 극소수만 존재. `commit_unit` 전용 C setter는 없음(확인: `rg -n "commit_unit" unim-capi/src/lib.rs` → 0건 매치, 위 rg 결과 참고) — C 프런트엔드는 popup/설정을 대부분 DBus(`unim-gui-common`의 dbus 클라이언트) 경유로 다루고, capi의 Config 포인터는 엔진 임베딩(`unim_engine_new` 등)에만 쓰이는 것으로 보인다. **새 설정에 C setter가 필요한지는 "그 필드를 엔진 keypress 처리 hot path에서 capi 경유로 읽는 C 프런트엔드가 있는가"에 달림 — 한자 팝업은 이미 popup-service(Rust, DBus)가 담당하므로 원칙적으로 불필요할 가능성 높음(미해결 질문으로 재확인 권장).**

## 4. 이 기능을 위한 확장 지점 (어디를 어떻게, 위험도)

### 4.1 `src/config.rs` — 신규 필드 추가
```rust
// 위치 후보 A: EngineConfig 최상위 (hanja_keys와 동급, src/config.rs:942-982 부근)
// 위치 후보 B: KoreanConfig 내부 (commit_unit과 동급, src/config.rs:606-665 부근)
pub hanja_output_format: HanjaOutputFormat,   // enum: Hanja | HanjaParenKorean | KoreanParenHanja (漢字/한자(漢字)/漢字(한자))
pub hanja_word_conversion: bool,              // on/off — #[serde(default)] 로 true 권장(무회귀 기본 유지 원칙, 단 신기능이므로 false 기본도 검토)
```
위험도: **낮음**. `#[serde(default)]` 필드 추가는 구 config.yaml 무회귀(필드 없으면 기본값 채움) — `Config` 구조체 자체가 `#[serde(default)]`(`src/config.rs:1006`)라 구조체 레벨에서도 이중으로 안전.
- enum은 `CommitUnit`과 동일 패턴(§2.1)으로: `#[derive(Clone, Copy, Debug, PartialEq, Eq, Default, Serialize, Deserialize)] #[repr(C)]` + `display_name()`/`all()`.
- `hanja_word_conversion`이 bool이면 별도 enum 불필요, `AutoEnglishConfig.enabled`류 패턴(단순 bool 필드) 참고.

### 4.2 `unim-cli/src/main.rs` — 위험도 낮음, 기계적
1. `ConfigKey` enum에 `HanjaOutputFormat`, `HanjaWordConversion` variant 추가(§3.2-1 패턴 그대로).
2. `commit_unit_display_name_localized`류 로케일 어댑터 함수 추가(§3.2-2).
3. `config set` match arm 추가(§3.2-3, 영문 키워드 + 한국어 별칭 이중 수용 관례 유지: 예 `"hanja" | "한자"`, `"hanja-paren-korean" | "한자(한글)"` 등 — 값 문자열 설계는 설계자 재량).
4. `config show` 출력 라인 추가(§3.2-4).
5. 대화형 메뉴는 **선택**(기존 `commit_unit`도 안 들어가 있음, §3.2-5) — 다만 이 기능은 사용자 대면 신기능이라 넣는 쪽을 권장(설계자 판단).

### 4.3 `unim-cli/locales/{ko,en}.yml` — 위험도 낮음, 누락되면 즉시 컴파일/런타임 티가 남
naming 관례(§확인됨, `ko.yml`/`en.yml` 완전 동일 줄번호로 미러링돼 있음 — `commit_unit_*` 키가 양쪽 44/45/88/105-106/119-122/378행에 정확히 대응):
```
hanja_output_format_label / _note?
error_invalid_hanja_output_format
hanja_output_format_changed
hanja_output_format_hanja / _hanja_paren_korean / _korean_paren_hanja   (표시값, enum variant별)
help_ck_hanja_output_format
(hanja_word_conversion 도 동일 패턴, bool이면 enabled/disabled 문구는 기존 "enabled"/"disabled" 공용 키 재사용 가능 — config.rs:900 부근 auto_typefix_status 패턴 참고)
```
**두 파일을 항상 같은 줄 수·같은 순서로 유지하는 관례**를 지킬 것(리뷰 편의, 강제 검증 도구는 미확인).

### 4.4 팝업 UI(GTK 위젯) — 위험도 중간, 이미 유사 인프라 존재
- **한자 즐겨찾기 기능은 이미 존재한다**: `src/hanja/bookmark.rs`(`HanjaBookmarkStore`, JSON 저장 `~/.local/share/unim/hanja-bookmarks.json`, 스키마 `{ "한글음절": ["漢","韓",..] }` — `src/hanja/bookmark.rs:1-8, 27-31, 49-51`). DBus surface: `GetHanjaBookmarkStates`/`ToggleHanjaBookmark`/`HanjaBookmarkChanged`(확인: `rg -l HanjaBookmark` → `unim-dbus/src/service.rs`, `unim-dbus/src/engine_worker.rs`, `unim-dbus/src/client.rs` 등 다수 히트). 전 프런트엔드(GNOME ext, GTK, Qt, XIM, Wayland)에 이미 통합 완료(`git log --oneline | grep hanja` 다수 커밋: `f947a25 feat(popup): hanja bookmark UX...(전 7개 프런트엔드)`, `238a4ad fix(hanja-popup): right-click bookmark toggle` 등).
- **9칸/9x9 확장 그리드 팝업도 이미 존재한다**: `eb7f9e3 fix(hanja-popup): always render 9×9 grid`, 각 프런트엔드별 row-number 컬럼 커밋 다수. 팝업 명세: `docs/dev/specs/POPUP_SPEC.md`(메모리 규칙: "예외 없이 준수, 변경 시 사용자 승인 필수" — §5 참고).
- **중요**: 즐겨찾기 저장소(`hanja-bookmarks.json`)는 **키가 한글 음절 단위**(`entries: BTreeMap<String, BTreeSet<String>>`, key=한글, value=한자 집합). "한자 단어 입력"의 즐겨찾기는 **어절/단어 단위**가 필요("대한민국" 같은 다음절 단어)일 수 있으므로, 기존 스토어를 그대로 재사용할지(키를 다음절 문자열로 확장 — 스키마 변경 없이 가능해 보임, `String` 키라 제약 없음) 별도 스토어를 둘지 결정 필요(§7 미해결 질문).
- 이 즐겨찾기 파일은 **config.yaml이 아니라 사용자 데이터 파일**이라 §0/§1의 "5(6)지점 config sync" 체크리스트 대상이 **아니다** — AutoTypeFix blacklist와 동일한 예외 범주(`docs/dev/architecture/GEMINI.md:52-55`의 "사용자 데이터 파일은 5지점 동기 대상이 아닙니다" 원칙과 정확히 같은 패턴). 다만 **표시 형식(`hanja_output_format`)은 config.yaml 설정이므로 5(6)지점 전부 대상.**

### 4.5 `unim-settings-gtk/src/settings_dialog.rs` — 위험도 낮음~중간
- enum(`hanja_output_format`) → `adw::ComboRow` 3-way(§3.4 코드 그대로 복제, `commit_row`를 템플릿으로).
- bool(`hanja_word_conversion`) → 기존 bool 설정 위젯 패턴 확인 필요(`toggle_announce_beep`/`ignore_key_repeat`가 어떤 위젯인지 이번 조사에서 미열람 — `rg -n "toggle_announce_beep" unim-settings-gtk/src/settings_dialog.rs`로 위젯 종류 확인 권장, 슬라이더 정책 문서상 SwitchRow가 유력).
- **`merge_gtk_ui_owned`(§3.5) 화이트리스트에 두 필드 추가 필수** — 빠뜨리면 GTK 창에서 설정을 바꿔도 저장 안 됨(반대로 disk 값이 이김).

### 4.6 `unim-settings/src/main.rs` (Slint) — 위험도 중간
- `.slint` UI 소스에 프로퍼티 선언 추가(파일 미확인 — §7).
- `set_hanja_output_format_options`/`_index` 류 codegen 바인딩 추가(§3.6 패턴).
- **`merge_ui_owned`(§3.6) 화이트리스트에도 별도로 추가 필수** — GTK판과 완전히 분리된 코드이므로 **양쪽에 각각** 추가해야 함(공유 안 됨, §3.6 마지막 문단).
- GNOME 사용자는 `prefs.js`가 이 앱으로 리다이렉트하므로(§1의 표 9번), **이 앱에 새 설정이 없으면 GNOME 사용자는 설정 자체에 접근 불가** — GTK판에만 추가하고 Slint판을 빠뜨리면 플랫폼별로 기능이 갈리는 회귀.

### 4.7 `unim-dbus/src/service.rs` — 위험도 낮음(대부분 자동)
- `GetConfigYaml`/`SetConfigYaml`/`GetConfigJson`: **코드 수정 불필요**(전체 구조체 serde).
- 레거시 `get_config`/`set_config` key 디스패치: **선택**(§3.3에서 확인했듯 `commit_unit`도 최근 필드라 레거시 목록에서 빠져 있음 — 전례를 따라 생략 가능). 다만 어떤 소비자가 레거시 key API에 의존하는지(구버전 프런트엔드?) 확인 후 결정 권장.

### 4.8 Windows `unim-tsf/src/settings_dialog.rs` — 위험도 미확정(§3.7 참고)
이 파일이 활성 UI인지 폐기 예정 폴백인지에 따라 대응이 갈림. **활성이라면** §4.5와 동일한 콤보 위젯 추가(Win32 API라 코드는 다르지만 패턴은 동일: `CommitUnit::all()` 순회 → 콤보 아이템, 인덱스 역변환).

### 4.9 C ABI(`unim-capi`)/GNOME gschema — 위험도 낮음, 대부분 불필요
- capi: 새 필드를 C 프런트엔드가 hot path에서 직접 읽을 필요가 없으면 스킵(§3.8).
- gschema: 일반 설정이므로 **추가 금지**(GEMINI.md 원칙, GNOME Shell 의존 키만 예외).

## 5. 지켜야 할 규칙 (AGENTS.md·POPUP_SPEC·주석에서 인용, file:line)

- **AGENTS.md는 리다이렉트 stub**: `/home/from104/work/unim/AGENTS.md:1-6` — 실제 본문은 `docs/dev/architecture/AGENTS.md`(277줄). 단, 그 본문에도 "설정 추가 체크리스트"라는 절은 없다(확인: `rg -n "설정.*체크리스트|config.*sync"` 0건). **"설정 항목 추가/변경 시 체크리스트"는 AGENTS.md가 아니라 `docs/dev/architecture/GEMINI.md:58-66`에 있다** (task 지시문의 "AGENTS.md에서... 인용" 전제가 부정확 — 실제 소재지는 GEMINI.md).
- GEMINI.md 체크리스트 원문 인용(`docs/dev/architecture/GEMINI.md:58-66`):
  ```
  1. [ ] src/config.rs — 설정 구조체에 새 필드 추가 (+ clamp_ranges() 방어 시 범위 확인)
  2. [ ] unim-cli/src/main.rs — ConfigKey enum 및 관련 config_set 매치 암 업데이트 (config 서브커맨드)
  3. [ ] unim-cli/locales/{ko,en}.yml — 번역 문자열 추가
  4. [ ] unim-dbus/src/service.rs — 레거시 key 디스패치가 필요한 경우 매칭 업데이트 (YAML/JSON은 자동)
  5. [ ] unim-gui-gtk/src/gtk_ui.rs — GTK GUI 위젯 및 바인딩 추가   ← 실제 파일명은 unim-settings-gtk/src/settings_dialog.rs (경로 stale, §0 참고)
  6. [ ] (GNOME Shell 전용 키일 때만) unim-gnome-extension/prefs.js + *.gschema.xml
  7. [ ] (AutoTypeFix 관련 설정일 때) Blacklist 파일 핫리로드 로직이 새 설정과 독립적으로 동작하는지 확인
  ```
  **이 문서가 언급하지 않는 항목**(실제로는 필요): `unim-settings/src/main.rs`(Slint, §4.6), `merge_gtk_ui_owned`/`merge_ui_owned` 화이트리스트(§3.5-3.6), Windows `unim-tsf/src/settings_dialog.rs`(§3.7-4.8 존재 여부 확인).
- 5지점 원칙 원문(`docs/dev/architecture/GEMINI.md:34-38`): "Phase 1~7 설정 개편(2026-04) 이후 일반 설정은 `~/.config/unim/config.yaml` 단일 소스이며, GSettings(gschema)는 GNOME Shell 의존 키만 남겨졌다(18→6키). 일반 사용자 설정은 GTK GUI가 유일한 창구이고, Qt 트레이·GNOME Extension prefs.js는 이 GUI로 리다이렉트한다." — **"유일한 창구"라는 서술 자체가 Slint 앱 신설 이후 stale**(§0).
- 사용자 데이터 파일 예외 원칙(`docs/dev/architecture/GEMINI.md:52-55`): "AutoTypeFix 억제 사전은 설정이 아닌 사용자 데이터이며 5지점 동기 대상이 아니다" — 한자 즐겨찾기(`hanja-bookmarks.json`)도 동일 논리로 예외(§4.4).
- Zero Tolerance 빌드 규칙(`docs/dev/architecture/GEMINI.md:20-24`): `cargo build --workspace` 경고 0개, `cargo test --workspace` 전체 통과, `make build` 경고 0개 — 새 필드/enum 추가 후 반드시 확인.
- POPUP_SPEC.md 절대 준수: 메모리 `feedback_popup_spec_absolute.md` — "POPUP_SPEC.md 규칙은 예외 없이 준수, 변경 시 사용자 승인 필수". 위치: `docs/dev/specs/POPUP_SPEC.md`(본 조사에서 내용 미열람 — 팝업 페이지네이션/즐겨찾기 UI를 재사용/확장할 다른 서브시스템 조사자가 반드시 이 파일 본문을 확인해야 함, 내 담당(config sync) 범위 밖).
- Config 3지점 동기 절대 원칙(메모리 `feedback_config_3way_sync.md`): "엔진(src/config.rs)·GUI(unim-gui-gtk)·CLI(unim-cli config ConfigKey) 설정은 항상 함께 싱크. 한 곳만 추가/삭제 금지" — 메모리의 "unim-gui-gtk" 표기도 stale(§0과 동일 사유), 개념(3점 이상 항상 동시 수정)은 유효.
- 슬라이더 정책(메모리 `feedback_slider_for_numeric.md`): "+/- SpinRow 금지, gtk::Scale + tick 마크 사용" — 코드로 확인됨(§3.4 `build_int_scale_row`).

## 6. Windows 동등성 메모

- Windows에는 **설정 UI가 최소 2개** 존재할 가능성: `unim-settings.exe`(Slint, 신규·권장 경로, `unim-settings/Cargo.toml` description: "UNIM settings GUI (Slint) for Windows/Linux") + `unim-tsf` 내부 레거시 Win32 모달(`unim-tsf/src/settings_dialog.rs`, 아직 `fn_configure.rs`/`lang_bar.rs`에서 호출됨). 새 한자 설정을 추가할 때 **Windows 경로에서 어느 쪽이 실제로 사용자에게 보이는지**(레거시가 완전히 대체됐는지, 아니면 조건부 폴백인지) 먼저 확인해야 이중 작업/누락을 막을 수 있음.
- `KoreanConfig` compat 브리지(§2.5)에 이미 Windows 특화 분기가 있음(`src/config.rs:863-869`, `#[cfg(not(target_os = "windows"))]`/`#[cfg(target_os = "windows")]`로 `word_mode_apps` 레거시 시드값 처리 상이) — 새 한자 설정이 플랫폼별로 기본값이 달라야 한다면 이 지점의 `#[cfg]` 분기 패턴을 참고.
- `unim-capi`는 Windows TSF(`unim-tsf`)와 Linux 프런트엔드가 공유하는 C ABI 크레이트로 보이나, 이번 조사에서 `unim-tsf`가 `unim-capi`를 실제로 링크하는지는 미확인(대부분 `unim-tsf`는 `unim`(core) crate를 Rust에서 직접 의존하는 것으로 보임 — `unim-tsf/src/settings_dialog.rs:22`가 `use unim::config::{...}`를 직접 쓰고 있어 capi 경유가 아님). 즉 **Windows 설정 UI는 capi를 안 거치고 core crate를 직접 링크** — capi 확장 여부와 Windows 지원은 무관.
- `AUTO_TYPEFIX_*_MIN/MAX` 상수들이 `unim-tsf/src/settings_dialog.rs:26-31`, `unim-settings-gtk`, `unim-settings/src/main.rs` 세 곳에 동일하게 import되는 패턴 확인(`src/config.rs`에 정의된 공용 상수) — 새 설정에 범위 제약이 있다면 이 상수 관례(코어에 `pub const XXX_MIN/MAX`)를 따를 것.

## 7. 미해결 질문

1. `unim-tsf/src/settings_dialog.rs`(레거시 Win32 모달)는 폐기 예정 죽은 코드인가, 여전히 유지보수 대상인 활성 UI인가? (`fn_configure.rs`/`lang_bar.rs`가 호출은 하지만 실행 경로 조건 — 예: Slint 앱 실행 실패 시 폴백인지 — 은 미확인.) 이 질문이 §4.8의 위험도를 완전히 좌우한다.
2. `hanja_output_format`/`hanja_word_conversion`을 `EngineConfig` 최상위에 둘지 `KoreanConfig` 내부에 둘지 — 기존 `hanja_keys`는 최상위(§2.3), `commit_unit`은 `KoreanConfig` 내부. 일관성 기준이 모호(단축키류=최상위, 조합/변환 동작류=KoreanConfig 내부로 보임 → 후자 쪽이 더 근접한 전례일 수 있음).
3. 한자 즐겨찾기 스토어(`src/hanja/bookmark.rs`)를 어절/단어 단위로 확장 재사용할지, 별도 스토어(`hanja-word-bookmarks.json`?)를 새로 만들지 — 재사용 시 기존 단일 음절 데이터와 스키마 충돌 여부(문자열 키라 형식 제약은 없어 보이나 UI가 "음절"과 "단어"를 구분해서 보여줘야 하는지는 UX 팀 판단 필요).
4. `unim-gui-common/src/settings_helpers.rs`의 `merge_field` 정확한 정의 라인 미확정(존재는 확실, `unim-settings-gtk`가 이를 import해서 씀 — `settings_helpers.rs` 상단 확인 필요, 이번 조사에서 라인 특정 안 함).
5. `unim-settings`(Slint)의 `.slint` UI 소스 파일 경로·`set_commit_unit_options` 등 codegen 프로퍼티 선언부 미열람 — 새 콤보를 추가하려면 이 `.slint` 파일 구조를 별도로 조사해야 함(이번 조사는 Rust 쪽 바인딩까지만 확인).
6. GNOME extension의 `hanja_popup.js`가 `ConfigChangedJson` payload에서 `hanja_output_format`을 어떻게 소비할지 — JS는 동적 타입이라 필드 추가 자체는 자동 통과하겠지만, **표시 로직(漢字/한자(漢字)/漢字(한자) 렌더링)은 각 프런트엔드(GNOME ext JS, GTK, Qt, XIM 등)에 별도 구현 필요** — 이는 "설정 동기화"가 아니라 "팝업 렌더링" 서브시스템 담당 범위이므로 이 문서에서는 존재만 표시.
7. 레거시 `get_config`/`set_config`(DBus key 디스패치)에 새 필드를 추가할지 여부를 결정할 "어떤 소비자가 아직 레거시 key API를 쓰는지" 목록 미조사(gtk-common C 클라이언트 wrapper 등 — `unim-gui-common/src/dbus_client.rs` 확인 필요, 범위 밖).

## 보충 #12: 출력 형식 설정의 적용 지점·핫리로드 / HanjaEntry 필드 / GTK enum 위젯 관례·merge_field 라인

**(1) 캐시 동기화 지점 — commit_unit 패턴과 동일**

- `InputEngine.commit_unit`/`word_mode_apps` 캐시는 `src/input_engine/engine.rs:191,198`(필드 선언), config 로부터의 동기화는 **오직 두 곳**: 생성자 `new()`(`engine.rs:245-246`, `config.engine.korean.commit_unit`/`.word_mode_apps` 그대로 대입)와 `rebuild_korean_context()`(`engine.rs:948,950`, 동일 대입 + 주석 "hot-reload 경로에서 config 에서 재동기화"). **`set_korean_layout()`(`engine.rs:895-922`)은 캐시를 재동기화하지 않는다** — `Config::default()` 스냅샷에 layout 만 얹어 `build_korean_context`를 돌리므로(909-912) commit_unit/word_mode_apps 는 건드리지 않고 기존 캐시값을 그대로 둔다. 즉 새 `hanja_output_format` 캐시를 추가한다면 **`new()`+`rebuild_korean_context()` 두 곳에만** 넣으면 된다(set_korean_layout 은 대상 아님).
- 호출 경로: Linux 데몬은 `unim-dbus/src/engine_worker.rs:404-421` `apply_korean_rebuild()`가 `engine.rebuild_korean_context(config)`를 호출하고, 이는 리로드 루프(`engine_worker.rs:968` 이하)에서 `korean_context_fingerprint()`(`engine_worker.rs:361-397`, 필드 목록 380-392: layout/active_rule_sets/bidirectional_combine/chord_window_ms/**commit_unit**/**word_mode_apps**/english.layout/user_dir_fingerprint)가 **바뀐 경우에만** 조합 중이 아닐 때(`is_mid_composition` 가드, `:998-1006`) 또는 조합 종료 직후(`pending_korean_rebuild`, `:1013-1030`) 실행된다.
- ⚠️ **핵심 설계 분기점**: `hanja_output_format`을 이 fingerprint 튜플에 넣으면 값이 바뀔 때마다 `flush_preedit()`+컨텍스트 전체 재구성(조합 끊김)이 따라온다 — 그런데 이 설정은 자모 조합/키맵과 무관한 **순수 표시 포맷**이라 그럴 필요가 없다. 같은 파일 373-374 주석이 이미 이 구분을 명시: ATF 핫키·전환키는 "비파괴 재적용"(`set_atf_hotkeys`/`set_switch_keys`, `:982,988`, 매 루프 무조건 호출, fingerprint 밖)으로 처리되고 fingerprint 에서 **일부러 제외**돼 있다. `hanja_output_format`은 이 ATF/전환키 쪽 전례를 따라야 한다 — fingerprint 에 넣지 말고 `set_atf_hotkeys` 와 동일한 자리에 `engine.set_hanja_output_format(&config)`류 비파괴 setter 를 추가해 매 루프 무조건 재동기화하는 편이 옳다(조합 중단 없음, `select_hanja` 는 다음 호출부터 새 값 사용).
- Windows TSF 는 이 증분 갱신 경로가 아예 없다: `unim-tsf/src/text_service.rs:424` `maybe_reload_config()`는 mtime 변경 시(`:461`) `InputEngine::new(&new_config)`로 **엔진 전체를 새로 만들어 교체**한다(`:462-463`). 즉 TSF 쪽은 `new()` 캐시 동기화 한 줄로 자동으로 새 포맷을 반영하며, engine_worker 식 fingerprint/비파괴 재적용 이원화가 필요 없다(대신 조합/word 모드가 리셋되는 비용은 이미 감수 중인 기존 동작).

**(2) HanjaEntry — `hanja_target`은 엔진 필드이지 HanjaEntry 필드가 아님**

- `HanjaEntry`(`src/hanja/dict.rs:13-20`)는 `hangul`/`hanja`/`meaning` 3필드뿐 — `hanja_target` 필드는 **존재하지 않는다**. `hanja_target`은 `InputEngine`의 별도 필드(`engine.rs:123`)로, 현재 변환 대상 음절 문자열을 담아 즐겨찾기 키로 쓰인다(`candidates.rs:171,193,194,201,225` — `is_bookmarked(&self.hanja_target, &e.hanja)`).
- 사전은 이미 다음절 표제어를 지원한다: `src/data/hanja.txt`에 `국가:國家:`/`국가:國歌:` (동음이의, `hangul="국가"` 동일) 처럼 `hangul` 필드가 **완성된 어절 전체**를 담을 수 있음을 실측 확인(`가부장제국가:家父長制國家:` 등 6음절 표제어도 존재).
- 결론: 서식 `한자(漢字)`의 한글 부분은 **`HanjaEntry.hangul`**을 써야 한다 — 이는 이미 "국가"처럼 어절 전체 키와 일치하고, 동음 항목(國家/國歌) 간에도 동일하므로 표시·즐겨찾기 키 일관성이 보장된다. 대상①(커밋+preedit 결합) 기능을 구현하려면 `InputEngine.hanja_target`을 "현재 변환 대상 어절 전체 문자열"로 확장해 사전 조회 키로 쓰면 되고(음절 하나뿐이던 기존 좁은 의미에서 확장), `HanjaEntry` 구조체 자체는 변경 불필요.

**(3) GTK enum 위젯 관례 + merge_field 정의 라인**

- 출력 형식은 bool 이 아니라 **3지 선택**이므로 `SwitchRow` 대상이 아니다. 정확한 전례는 `commit_unit`(음절/단어/스마트 3지)이며 `adw::ComboRow`를 쓴다 — `unim-settings-gtk/src/settings_dialog.rs:691-723`, 특히 691행 주석이 관례를 명문화: "조합 확정 단위(음절/단어/스마트) — 수치 아님이라 ComboRow (초기모드·모드공유와 동일 관례)". `SwitchRow`는 `bidirectional_combine` 등 순수 bool 전용(`settings_dialog.rs:349,405` 등)이고 슬라이더(`gtk::Scale`)는 수치 전용 — 3지 열거형은 ComboRow 가 유일한 기존 관례.
- `merge_field` 제네릭 정의: **`unim-gui-common/src/settings_helpers.rs:135-140`**(`fn merge_field<T: Clone + std::fmt::Debug>(dst: &mut T, baseline: Option<&T>, ui: &T)`, `debug_eq` 비교로 세션 중 미변경 필드는 disk 값 보존) — 이전 미해결 질문 #4 해소. `unim-settings/src/main.rs` 쪽은 동일 로직이 **별도 정의**로 중복 존재: `debug_eq`(:505-507)+`merge_field`(:512-514). 즉 이 두 크레이트는 헬퍼를 공유하지 않고 각자 복붙 유지 중 — 새 `hanja_output_format` 필드를 UI-소유로 노출하면 **양쪽 `merge_field` 호출 목록에 각각** 한 줄씩 추가해야 한다: `merge_gtk_ui_owned`(`settings_helpers.rs:170-207`, `commit_unit` 옆 204행 부근에 추가) + `merge_ui_owned`(`unim-settings/src/main.rs:527-597`, `commit_unit` 옆 573행 부근에 추가). 이는 기존 문서의 "6지점"에 **이 두 merge 화이트리스트가 이미 §3.5-3.6 으로 포함**돼 있었는지 재확인 필요 — 포함 안 돼 있었다면 실제 동기 지점은 6이 아니라 **7~8지점**(코어 config 필드 + GTK ComboRow + Slint UI + CLI + 두 merge_field 화이트리스트 + engine 캐시 setter)이다.

**남은 미해결(신규)**
- `hanja_output_format`을 `EngineConfig` 최상위 vs `KoreanConfig` 내부 중 어디 둘지는 기존 §7-Q2 미해결 그대로(조합/변환 동작류 = KoreanConfig 내부 전례가 더 근접).
- `set_hanja_output_format` 류 비파괴 setter를 실제로 신설할지, 아니면 그냥 `select_hanja` 호출 시점에 `&Config`를 인자로 받아 매번 config 에서 직접 읽어 캐시 자체를 없애는 대안(캐시 불일치 리스크 원천 제거)이 더 나은지는 구현 설계 판단 필요 — 이번 조사 범위 밖.
