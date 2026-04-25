# PR #7 → develop 이식 매핑서

> 대상 worktree: `/home/from104/work/unim-emoji-rebase` (브랜치 `feat/emoji-popup-rebased`)
> develop tip: `0f68d7a` — `unim-config/` 크레이트 부재, CLI는 `unim-cli/`로 이전됨
> 패치 출처: PR #7 (`claude/emoji-popup-input-nW43A`) — `/tmp/pr7.patch` (2103 lines)
> 분리된 per-file 패치: `/tmp/pr7_split/<safe-name>.patch`

---

## git apply --3way 결과 요약

`git apply --3way --whitespace=fix /tmp/pr7.patch` 1차 시도 결과:

- **자동 적용 성공 (10개 파일)**:
  - `src/hangul/emoji.rs`
  - `unim-gui-common/src/dbus_client.rs`
  - `unim-gui-common/src/types.rs`
  - `unim-gui-gtk/src/gtk_ui.rs`
  - `unim-gui-gtk/src/main.rs`
  - `unim-gui-gtk/src/emoji_popup.rs` (신규)
  - `unim-dbus/src/service.rs` (한 시도는 충돌, 폴백 직접 적용 성공 — 실제로는 깨끗하게 들어감)
  - `unim-gnome-extension/dbus_ime.js`
  - `unim-gnome-extension/extension.js`
  - `unim-gnome-extension/stylesheet.css`
  - `unim-gnome-extension/emoji_popup.js` (신규)
- **3-way 충돌(부분 적용 + .rej 가능)**:
  - `src/config.rs` — 컨텍스트 라인이 develop tip에서 어긋남(주변에 `user_dictionary_path`·`auto_english`가 추가됨)
  - `src/input_engine.rs` — Layout Profile v1 / AutoTypeFix 코드가 `press_key()` 윗부분에 들어와 컨텍스트가 밀림
- **인덱스에 없음 (3개 파일, 전부 폐기 대상)**:
  - `unim-config/locales/en.yml`
  - `unim-config/locales/ko.yml`
  - `unim-config/src/main.rs`

이후 `git checkout -- . && git clean -fd` 으로 워킹트리는 깨끗한 상태로 되돌렸습니다. 본 문서는 깨끗한 상태에서 재적용할 때의 매핑입니다.

---

## Group 1 — 자동 적용 (검증만)

| 파일 | 적용 방식 | 검증 포인트 |
|---|---|---|
| `src/hangul/emoji.rs` | `git apply` 또는 패치 직접 | `popular_emojis()` / `categories()` / `load_favorites()` / `save_favorites()` API가 develop의 emoji.rs와 충돌 없는지 (PR이 `+73 -5` 로 함수 추가 위주). |
| `unim-gui-common/src/dbus_client.rs` | 자동 | `ShowEmojiPopup` 시그널 구독 코드가 기존 시그널 등록 패턴과 동일한 형태인지. |
| `unim-gui-common/src/types.rs` | 자동 | `GuiAction::ShowEmojiPopup` (또는 그에 준하는) 추가 — IndicatorState 의 다른 variant 와 정합성. |
| `unim-gui-gtk/src/gtk_ui.rs` | 자동 | `mod emoji_popup;` use, `let emoji_popup = EmojiPopup::new(...)` 와 popup 라우팅 (`GuiAction::ShowEmoji` → `emoji_popup.show(...)`). 한자/특수문자 팝업과 평행 패턴. |
| `unim-gui-gtk/src/main.rs` | 자동 (1줄: `mod emoji_popup;`) | 모듈 선언만 — 컴파일러가 잡아줌. |
| `unim-gui-gtk/src/emoji_popup.rs` | 신규 파일 | 신규 — 충돌 없음. 타입 시그니처(`HanjaPopup` 패턴 차용)만 검증. |
| `unim-dbus/src/service.rs` | 자동 (3-way 1회 충돌은 폴백으로 흡수) | `PopupAction::ShowEmoji` 처리 분기·`show_emoji_popup` 시그널 메소드·`get_config / set_config` 의 `"emoji_popup"`·`"emoji_popup_keys"` 키 분기·`list_emoji_categories`·`get_emoji_favorites` D-Bus 메소드. |
| `unim-gnome-extension/{dbus_ime.js, extension.js, stylesheet.css}` | 자동 | `ShowEmojiPopup` 시그널 콜백·`CommitEmoji`/`SearchEmoji`/`ListEmojiCategories`/`GetEmojiFavorites` D-Bus call_sync 시그니처. dbus_ime.js 의 call_sync 비표준 인자 검증 필요(메모리: cancelHanja/cancelSpecialChar 패턴). |
| `unim-gnome-extension/emoji_popup.js` | 신규 파일 | St-based modal grab popup. 충돌 없음. |

검증 명령(빌드 직전):
```bash
cd /home/from104/work/unim-emoji-rebase
git apply /tmp/pr7_split/src__hangul__emoji.rs.patch
git apply /tmp/pr7_split/unim-gui-common__src__dbus_client.rs.patch
git apply /tmp/pr7_split/unim-gui-common__src__types.rs.patch
git apply /tmp/pr7_split/unim-gui-gtk__src__gtk_ui.rs.patch
git apply /tmp/pr7_split/unim-gui-gtk__src__main.rs.patch
git apply /tmp/pr7_split/unim-gui-gtk__src__emoji_popup.rs.patch
git apply /tmp/pr7_split/unim-dbus__src__service.rs.patch
git apply /tmp/pr7_split/unim-gnome-extension__dbus_ime.js.patch
git apply /tmp/pr7_split/unim-gnome-extension__extension.js.patch
git apply /tmp/pr7_split/unim-gnome-extension__stylesheet.css.patch
git apply /tmp/pr7_split/unim-gnome-extension__emoji_popup.js.patch
```

---

## Group 2 — 위치 이전 (`unim-config/` → `unim-cli/`)

PR #7 은 사라진 `unim-config/` 크레이트를 수정합니다. 동일 변경분을 develop tip의 `unim-cli/`로 옮겨야 합니다.

### 2-A. `unim-cli/src/main.rs` — `ConfigKey` enum

**현재 develop tip 상태 (확인됨):**
- `enum ConfigKey { ... }` 정의: `unim-cli/src/main.rs:194` (선언 라인)
- 마지막 variant `AppRules`: `unim-cli/src/main.rs:265-266`
  ```rust
      #[value(name = "app-rules")]
      AppRules,
  }
  ```
- `config_show()` 본체에서 `app_rules_label` 출력: `unim-cli/src/main.rs:571-575` 부근
- `config_set()` 의 `ConfigKey::AppRules` 분기: `unim-cli/src/main.rs:938-947`
- `match` 의 닫는 `}` 직전이 `AppRules`

**추가할 variant** (PR #7 의 `unim-config/src/main.rs:104-110` 해치):
```rust
    /// 이모지 팝업 활성화 (true, false)
    #[value(name = "emoji-popup")]
    EmojiPopup,
    /// 이모지 팝업 트리거 키 (예: Super+Period)
    #[value(name = "emoji-popup-keys")]
    EmojiPopupKeys,
```
**삽입 위치**: `unim-cli/src/main.rs:266` 의 `AppRules,` 다음 줄, enum 닫는 `}` 직전.

**`config_show()` 출력 추가** (PR `unim-config/src/main.rs:188-198`):
```rust
    let emoji_status = if config.engine.emoji_popup.enabled {
        t!("enabled")
    } else {
        t!("disabled")
    };
    println!("{}: {}", t!("emoji_popup_label"), emoji_status);
    println!(
        "{}: {}",
        t!("emoji_popup_keys_label"),
        config.engine.emoji_popup.trigger_keys.join(", ")
    );
```
**삽입 위치**: `unim-cli/src/main.rs:575` 부근 — `app_rules_label` 출력 블록 바로 다음, `println!();` 빈 줄 출력 직전. (`config_show()` 의 `t!("config_file_label")` 출력 이전)

**`config_set()` match arm 추가** (PR `unim-config/src/main.rs:508-530`):
```rust
        ConfigKey::EmojiPopup => {
            let enabled: bool = value.parse()
                .map_err(|_| "Invalid value, use true/false".to_string())?;
            config.engine.emoji_popup.enabled = enabled;
            let status = if enabled { t!("enabled") } else { t!("disabled") };
            println!("{}: {}", t!("emoji_popup_label"), status);
        }
        ConfigKey::EmojiPopupKeys => {
            let keys: Vec<String> = value
                .split(',')
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .collect();
            if keys.is_empty() {
                return Err("At least one trigger required".to_string());
            }
            config.engine.emoji_popup.trigger_keys = keys;
            println!(
                "{}: {}",
                t!("emoji_popup_keys_label"),
                config.engine.emoji_popup.trigger_keys.join(", ")
            );
        }
```
**삽입 위치**: `unim-cli/src/main.rs:946` (`ConfigKey::AppRules` arm 닫는 `}`) 다음 줄, `match` 닫는 `}` 직전 (`unim-cli/src/main.rs:947` 추정 — `config.engine.auto_typefix.clamp_ranges();` 직전).

### 2-B. `unim-cli/locales/en.yml`

**기존 키 위치**: `app_rules_label: "App Mode Rules"` (Group "Config related" 블록의 끝부분).
PR `unim-config/locales/en.yml` 패치(`+2 -0`)는 `auto_typefix_observation_timeout_secs_label` 그룹 직후, `unit_secs` 다음에 두 줄을 넣습니다 — develop의 `unim-cli/locales/en.yml` 도 동일 키 그룹이 있으므로 같은 위치에 추가:
```yaml
emoji_popup_label: "Emoji Popup"
emoji_popup_keys_label: "Emoji Popup Trigger Keys"
```
**삽입 위치**: `unim-cli/locales/en.yml` 의 `app_rules_label: "App Mode Rules"` 줄 **바로 위** (PR 의 정확한 위치 = `unit_secs:` 직후·`app_rules_label` 직전과 동일하게 유지).

### 2-C. `unim-cli/locales/ko.yml`

PR 의 한국어 번역 (`unim-config/locales/ko.yml +51,7`):
```yaml
emoji_popup_label: "이모지 팝업"
emoji_popup_keys_label: "이모지 팝업 트리거 키"
```
**삽입 위치**: `unim-cli/locales/ko.yml` 의 `app_rules_label: "앱별 모드 규칙"` 줄 **바로 위**.

### 2-D. `unim-config/{Cargo.toml, SPEC.md}` — 폐기

PR 패치에는 별도의 `unim-config/Cargo.toml`·`SPEC.md` 변경이 없습니다 (위에서 확인한 16개 변경 파일 목록 기준). 따라서 별도 처리 불필요. 단 `git apply` 가 `unim-config/locales/*` 와 `unim-config/src/main.rs` 를 시도하여 새 폴더를 만들 가능성이 있으므로 **per-file 적용 시 이 3개는 건너뛴다**. 만약 실수로 폴더가 생성되면:
```bash
git rm -rf unim-config/
```

---

## Group 3 — 누락 보완 (`unim-gui-gtk/src/settings_dialog.rs`)

PR #7 에는 GUI 설정 다이얼로그 토글이 빠져 있습니다. `Config 3지점 싱크 절대 원칙`(엔진·GUI·CLI)에 따라 GTK GUI 설정 다이얼로그에도 emoji-popup 항목을 추가해야 합니다.

### 3-A. 참고할 기존 토글 패턴 (확인됨)

- **switch row 패턴 (마스터 enable/disable)**: `unim-gui-gtk/src/settings_dialog.rs:929-944` 의 `auto_typefix.enabled` 마스터 SwitchRow.
  ```rust
  let master = adw::SwitchRow::builder().title("…").build();
  // set_active 초기화
  // connect_active_notify { config.engine.X.enabled = sw.is_active(); save_and_notify(..., "X_enabled"); }
  ```
- **trigger_keys 입력 패턴 (쉼표 구분 EntryRow)**: `unim-gui-gtk/src/settings_dialog.rs:483-490` 의 `hanja_keys` 빌드 호출, 그리고 `auto_english` 의 `trigger_keys` 빌드 호출 (`unim-gui-gtk/src/settings_dialog.rs:614-620` 부근):
  ```rust
  group.add(&build_string_list_row(
      state,
      "한자 키",
      Some("쉼표로 구분 (예: Hanja, F9)"),
      |cfg| cfg.engine.hanja_keys.join(", "),
      |cfg, v| cfg.engine.hanja_keys = v,
      "hanja_keys",
  ));
  ```
- **헬퍼 시그니처**: `build_string_list_row(state, title, subtitle, get, set, label)` — `unim-gui-gtk/src/settings_dialog.rs:1091` 부근에 정의.

### 3-B. emoji-popup 토글 추가 명세

페이지 선택: 한자 키와 같은 페이지(키 입력/단축키 그룹)에 두는 것이 가장 자연스럽습니다 — `hanja_keys` 와 같은 group 직후.

**삽입 위치**: `unim-gui-gtk/src/settings_dialog.rs:489` 의 `"hanja_keys",` 가 닫히는 `));` 다음 줄. 즉 `group` 변수가 returned 되기 직전.

**추가할 코드 (스켈레톤)**:
```rust
    // 이모지 팝업 enable/disable
    let emoji_sw = adw::SwitchRow::builder()
        .title("이모지 팝업")
        .subtitle("Super+. 등 트리거 키로 이모지 팝업 표시")
        .build();
    {
        let s = state.borrow();
        emoji_sw.set_active(s.config.engine.emoji_popup.enabled);
    }
    {
        let state_c = state.clone();
        emoji_sw.connect_active_notify(move |sw| {
            let mut s = state_c.borrow_mut();
            if s.updating { return; }
            s.config.engine.emoji_popup.enabled = sw.is_active();
            save_and_notify(&s.config, "emoji_popup");
        });
    }
    group.add(&emoji_sw);

    // 이모지 팝업 트리거 키
    group.add(&build_string_list_row(
        state,
        "이모지 팝업 트리거 키",
        Some("쉼표로 구분 (예: Super+Period, Control+Shift+E)"),
        |cfg| cfg.engine.emoji_popup.trigger_keys.join(", "),
        |cfg, v| cfg.engine.emoji_popup.trigger_keys = v,
        "emoji_popup_keys",
    ));
```

**save_and_notify 키 이름**: `"emoji_popup"`, `"emoji_popup_keys"` — 이 이름이 `unim-dbus/src/service.rs:407-408` 의 `get_config` 와 `set_config` 분기 키와 정확히 일치해야 합니다 (PR 패치에서 추가된 키 이름과 동일).

**(선택) 다국어**: 위 `"이모지 팝업"` / `"이모지 팝업 트리거 키"` 는 다른 토글들이 한국어 리터럴을 직접 사용하는 패턴(예: `"한자 키"`, `"자동 영문 전환 사용"`)과 일관됩니다. `t!()` 사용 여부는 페이지 전체 일관성에 맞춰 결정 — 현재 settings_dialog 는 한국어 리터럴을 직접 사용하므로 그대로 두는 게 맞음.

---

## Group 4 — 의미 충돌 검증 (`src/config.rs`, `src/input_engine.rs`)

### 4-A. `src/config.rs`

**충돌 사유**: PR 의 컨텍스트 행 (`@@ -455,6 +455,29 @@`) 가 `popup_mode` / `auto_typefix` 다음을 가정하는데, develop tip 에서는 그 사이에 `user_dictionary_path` (PR #6, AutoTypeFix reverse user dict) 와 `auto_english` (PR #5) 가 끼어들어 라인 번호가 어긋남.

**develop tip 의 실제 EngineConfig 끝부분** (확인됨, `src/config.rs:614-645` 부근):
```rust
pub struct EngineConfig {
    ...
    pub toggle_keys: Vec<String>,
    pub hanja_keys: Vec<String>,
    pub app_rules: Vec<AppRule>,
    pub popup_mode: PopupMode,
    pub auto_typefix: AutoTypeFixConfig,
    pub auto_english: AutoEnglishConfig,
}
impl Default for EngineConfig {
    fn default() -> Self {
        Self {
            ...
            popup_mode: PopupMode::default(),
            auto_typefix: AutoTypeFixConfig::default(),
            auto_english: AutoEnglishConfig::default(),
        }
    }
}
```

**적용 매핑 (의미 충돌 없음, 단순 위치 이동)**:
1. `EngineConfig` 구조체에 `pub emoji_popup: EmojiPopupConfig,` 를 `auto_english: AutoEnglishConfig,` **다음 줄**에 추가 (PR 은 `auto_typefix` 다음에 두지만, develop 에서는 마지막 필드 뒤로 통일 — `auto_english` 다음).
2. `Default for EngineConfig` 의 struct literal 끝에 `emoji_popup: EmojiPopupConfig::default(),` 를 `auto_english: AutoEnglishConfig::default(),` **다음 줄**에 추가.
3. `EmojiPopupConfig` 구조체와 `Default` impl 은 `EngineConfig` 정의 **앞** 또는 **뒤** 어디든 가능 — PR 패치는 `EngineConfig` 정의 직후·`Default for EngineConfig` 직전에 둠. develop 에서도 같은 위치에 두면 됨 (대략 `src/config.rs:613` 즈음, `EngineConfig` 닫는 `}` 다음).

**적용 권장 절차**:
- `git apply` 의 3-way 충돌을 받아들이고 `.orig`/`<<<<` 마커를 손으로 정리하기보다는, **수동 패치** 가 더 빠릅니다 (필드 3곳만 추가).

### 4-B. `src/input_engine.rs`

**충돌 사유**: PR 의 컨텍스트가 `@@ -25,6 +25,10 @@ pub enum PopupAction { ... ShowSpecial { ... top_row: String, } }` (PopupAction 의 `ShowSpecial` 다음) 과 `@@ -174,6 +178,10 @@ pub struct InputEngine` 등 라인 번호 단정. develop tip 은 `HanjaBookmarkChanged` (`src/input_engine.rs:41`) 와 `PopupNavigate` 가 추가되어 PopupAction enum 길이가 늘어났고, struct 내부에는 Layout Profile v1 / AutoTypeFix retrigger 필드가 끼어들어 라인이 밀렸음.

**의미 충돌 — 분석 결과: 충돌 없음, 단순 라인 드리프트만 존재**.

**삽입 매핑**:

1. **`PopupAction` enum (`src/input_engine.rs:16-44` 부근)**
   - 추가: `ShowEmoji,` variant
   - **삽입 위치**: `HidePopup` (develop `src/input_engine.rs:29`) **직전**. (PR 도 동일 위치)
   - 의미 충돌: 없음. 새 variant 추가는 기존 패턴 매치(`PopupAction::ShowHanja`, `PopupAction::ShowSpecial`, `PopupAction::HidePopup`, `PopupAction::PopupNavigate`, `PopupAction::HanjaBookmarkChanged`)에 영향 없음 (소비처는 `unim-dbus/src/service.rs:912-988` 의 match가 wildcard 없음 — Rust 컴파일러가 `ShowEmoji` 처리 누락을 잡아주는데 PR 의 service.rs 패치가 정확히 이 분기를 추가함 → 정합).

2. **`InputEngine` struct fields**
   - 추가: `emoji_triggers: Vec<(ModifierState, KeyCode)>`, `emoji_popup_enabled: bool`
   - PR 컨텍스트는 `toggle_keys: Vec<KeyCode>,` 다음. develop tip 의 `toggle_keys` 위치가 같은 부근(`src/input_engine.rs:178` 근처)이므로 **`toggle_keys` 다음 줄**에 그대로 추가.
   - 다른 PR(#5, #6)이 이미 새 필드들을 추가했더라도 같은 struct 내부 필드 추가는 서로 독립이라 충돌 없음.

3. **`InputEngine::new()` initializer 의 struct literal**
   - 추가: `emoji_triggers: ...filter_map(parse_emoji_trigger)...`, `emoji_popup_enabled: ...`
   - **삽입 위치**: `toggle_keys: ...` 초기화 다음. (PR 컨텍스트도 `toggle_keys` 다음·`popup_state` 직전)

4. **`parse_emoji_trigger` 정적 함수, `matches_emoji_trigger` 메소드**
   - 신규 함수, 기존 코드와 충돌 없음. **삽입 위치**: `InputEngine::new()` 직후·`create_english_keymap` 또는 `press_key` 직전 영역(대략 `src/input_engine.rs:267` 부근, `impl InputEngine` 블록 내).

5. **`press_key()` 의 emoji 트리거 분기 — 가장 중요한 우선순위 결정**
   - PR 은 `// Hanja 키 처리` 직전·`// Control/Alt가 눌린 경우 (단축키) 무시` 직전에 emoji trigger 체크를 둠.
   - develop 의 `press_key()` 는 다음 순서:
     1. modifier-only 무시
     2. **`if self.hanja_mode || self.special_char_mode { ... process_popup_key }`** (popup 모드)
     3. (PR 이 추가하려는 위치) `matches_emoji_trigger` 체크 — popup 모드보다 **뒤**, Control/Alt 단축키 early-return 보다 **앞**
     4. Control/Alt early-return
     5. Backspace/Enter/Tab/Escape
     6. Hanja 키 처리 (`if keycode == KeyCode::Hanja`)
     7. **`is_auto_english_trigger`** (auto-english category switch)

   - **권고 적용 순서 (의미 충돌 없는 안전한 순서)**:
     ```
     popup 모드 인터셉트
       ↓
     >>> matches_emoji_trigger (NEW) <<<     ← 여기에 삽입
       ↓
     Control/Alt early-return
       ↓
     Backspace/Enter/Tab/Escape
       ↓
     Hanja 키 처리
       ↓
     is_auto_english_trigger
     ```
   - **사유**:
     - `Super+Period` 는 Control/Alt 단축키 분기를 통과하면 modifier=super 로 인해 early-return 으로 흘러가 그대로 무시됨 → emoji 분기는 반드시 그 **앞**에 있어야 함.
     - popup 모드(한자/특수문자) 가 활성일 때는 popup_key 처리가 우선이어야 하므로 **그 다음** 에 둠.
     - `is_auto_english_trigger` 도 modifier 조합을 보지만 **문자 키** 위주(Slash 등)이며, Super 조합과 겹치지 않음 → 충돌 없음.
     - PR 의 테스트 (`test_emoji_trigger_only_super_period_matches`, `test_emoji_custom_trigger_from_config`, 조합 중 Super+Period) 가 모두 이 순서를 가정.
   - **삽입 위치 (라인 단위)**: `src/input_engine.rs` 의 `if self.hanja_mode || self.special_char_mode { return self.process_popup_key(...); }` 블록 닫는 `}` **직후**·`if modifier.control || modifier.alt || modifier.super_key {` **직전**. develop tip 에서는 대략 `src/input_engine.rs:355-365` 사이.

6. **테스트 추가 분량** (`src/input_engine.rs` 패치의 끝부분, +200줄 중 ~80줄)
   - 신규 `#[test]` 함수 4-5개 (`test_emoji_trigger_only_super_period_matches`, `test_emoji_custom_trigger_from_config`, 조합 중 Super+Period 트리거 등)
   - **삽입 위치**: `mod tests { ... }` 내부, 마지막 `#[test]` 직전 또는 직후. develop tip 에서는 라인 1700-1800 부근.

**결론 (Group 4)**: **의미 충돌 없음 — 라인 드리프트만 존재**. 수동/3-way 어느 방식이든 `src/config.rs` 와 `src/input_engine.rs` 두 파일은 **6-개 후크 포인트**(config: 3개, input_engine: 6개) 를 사람이 한 번씩만 손보면 충돌 없이 들어감.

---

## `unim-config/` 크레이트 처리

- **결정**: 패치의 `unim-config/locales/en.yml`, `unim-config/locales/ko.yml`, `unim-config/src/main.rs` 변경은 **모두 폐기**한다.
- **사유**: develop `6a71f78` 에서 `unim-config/` 크레이트 자체가 제거되고 모든 CLI 책임은 `unim-cli/` 로 이전됨. PR 변경분의 의미는 Group 2 에서 `unim-cli/` 로 옮겨 흡수.
- **운영 절차**:
  1. `git apply /tmp/pr7.patch` (전체) 는 사용하지 않는다. 반드시 per-file 적용으로.
  2. 만에 하나 `unim-config/` 폴더가 생성되면 `git rm -rf unim-config/` 로 즉시 제거.
- **PR 에 별도 `unim-config/Cargo.toml`·`SPEC.md` 변경이 없음**(per-file split 결과 확인) → 추가 폐기 대상 없음.

---

## 다음 단계 작업 순서 권고

1. **(Group 1) 깨끗하게 자동 적용되는 11개 파일 → per-file `git apply` 로 일괄 패치** — 빌드 검증 전에 컴파일 에러는 `EmojiPopupConfig` 미정의로 인해 발생할 것이므로 다음 단계까지 묶어서 진행.
2. **(Group 4-A) `src/config.rs`** — `EmojiPopupConfig` struct + `Default` + `EngineConfig` 의 필드 1개 + Default initializer 1개 추가. **수동 편집 권장** (3개 후크 지점만 손대면 됨).
3. **(Group 4-B) `src/input_engine.rs`** — PopupAction variant + InputEngine 필드 2개 + `new()` initializer 2개 + 헬퍼 함수 2개 + `press_key()` 의 emoji trigger 분기 1개 + 테스트. **수동 편집 권장**.
4. **(Group 2) `unim-cli/` 적용 — `config-editor` 에이전트 위임**
   - `unim-cli/src/main.rs` 에 ConfigKey 2개·config_show 출력·config_set arm 2개 추가
   - `unim-cli/locales/{en,ko}.yml` 에 라벨 2개씩 추가
5. **(Group 3) `unim-gui-gtk/src/settings_dialog.rs` 적용 — `gtk-designer` 에이전트 위임**
   - SwitchRow + EntryRow 2개 추가, save_and_notify 키 `"emoji_popup"` / `"emoji_popup_keys"` 사용.
6. **빌드 검증** (`build-validator` 에이전트):
   - `cargo build --workspace`
   - `cargo test --workspace -p unim --test ...` (emoji 관련 단위 테스트)
   - GTK GUI 띄워 토글 표시 확인
   - GNOME extension reload 후 `Super+Period` 동작 확인
7. **테스트 시나리오**:
   - 한글 조합 중 `Super+.` → 조합 commit 후 emoji 팝업
   - emoji popup 키보드 네비게이션 (방향키/Tab/Enter/Esc)
   - Standalone(GTK GUI) vs GNOME Wayland (extension) 분기 동작
   - 사용자 정의 트리거 (`Control+Shift+E`) 동작 확인 (CLI 로 설정 후)

---

## 참고: per-file 패치 파일 인덱스

```
/tmp/pr7_split/
  src__config.rs.patch                          # 수동 적용 권장 (4-A)
  src__hangul__emoji.rs.patch                   # 자동 (Group 1)
  src__input_engine.rs.patch                    # 수동 적용 권장 (4-B)
  unim-config__locales__en.yml.patch            # 폐기 — 내용은 unim-cli/locales/en.yml 로 (Group 2-B)
  unim-config__locales__ko.yml.patch            # 폐기 — 내용은 unim-cli/locales/ko.yml 로 (Group 2-C)
  unim-config__src__main.rs.patch               # 폐기 — 내용은 unim-cli/src/main.rs 로 (Group 2-A)
  unim-dbus__src__service.rs.patch              # 자동 (Group 1)
  unim-gnome-extension__dbus_ime.js.patch       # 자동 (Group 1)
  unim-gnome-extension__emoji_popup.js.patch    # 자동 신규 파일 (Group 1)
  unim-gnome-extension__extension.js.patch      # 자동 (Group 1)
  unim-gnome-extension__stylesheet.css.patch    # 자동 (Group 1)
  unim-gui-common__src__dbus_client.rs.patch    # 자동 (Group 1)
  unim-gui-common__src__types.rs.patch          # 자동 (Group 1)
  unim-gui-gtk__src__emoji_popup.rs.patch       # 자동 신규 파일 (Group 1)
  unim-gui-gtk__src__gtk_ui.rs.patch            # 자동 (Group 1)
  unim-gui-gtk__src__main.rs.patch              # 자동 (Group 1)
```

원본 단일 패치: `/tmp/pr7.patch` (2103 lines)
