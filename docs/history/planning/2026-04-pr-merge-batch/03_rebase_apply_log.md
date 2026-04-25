# Phase C+D 적용 로그

> worktree: `/home/from104/work/unim-emoji-rebase` (브랜치 `feat/emoji-popup-rebased`)
> develop tip: `0f68d7a`
> PR #7 split patches: `/tmp/pr7_split/`

---

## Step 1 결과 (자동 적용)

- 자동 적용 성공: 11/11 (3건은 라인 드리프트로 부분 수동 보완)
- 부분 수동 보완:
  - `unim-gui-common/src/types.rs` — `HanjaBookmarkChanged` / `PopupNavigate` 가 끼어들어 컨텍스트 어긋남. `ShowEmojiPopup` variant를 `HidePopup` 직전에 직접 삽입.
  - `unim-gui-gtk/src/gtk_ui.rs` — `HanjaBookmarkChanged` arm 추가로 컨텍스트 드리프트. `EmojiPopup` 모듈 use, `emoji_popup` 변수, `ShowEmojiPopup` arm, `HidePopup` arm 의 `emoji_clone.hide()`, `load_css()` format!() 4번째 인수까지 4 hunk 모두 직접 수동 적용.
  - `unim-dbus/src/service.rs` — `auto_english`/`auto_english_keys` 분기가 끼어들어 `app_rules` 직전 라인이 어긋남. 또 develop의 `PopupAction::ShowHanja/ShowSpecial`이 `if is_standalone` guard를 사용하므로 `PopupAction::ShowEmoji`도 동일 guard 적용.
- 깨끗하게 적용된 파일: src/hangul/emoji.rs, unim-gui-common/src/dbus_client.rs, unim-gui-gtk/src/main.rs, unim-gui-gtk/src/emoji_popup.rs (신규), unim-gnome-extension/extension.js, unim-gnome-extension/stylesheet.css, unim-gnome-extension/emoji_popup.js (신규).
- `unim-gnome-extension/dbus_ime.js`도 컨텍스트 드리프트로 수동 보완 (콜백 필드, setPopupCallbacks, signal handler, 이모지 메서드 블록, cleanup) — 5 hunk 모두 패턴 그대로 이식.
- 실패 후 중단된 파일: 없음.

## Step 2 결과 (Group 4: 의미 충돌)

- `src/config.rs`: ✅
  - `EngineConfig`에 `emoji_popup: EmojiPopupConfig` 추가 (auto_english 다음)
  - `EmojiPopupConfig` struct + `Default` impl 추가
  - `Default for EngineConfig` initializer에 `emoji_popup: EmojiPopupConfig::default()` 추가
- `src/input_engine.rs`: ✅ (press_key 우선순위 확인)
  - `PopupAction::ShowEmoji` variant 추가 (HidePopup 직전)
  - `InputEngine` struct에 `emoji_triggers`/`emoji_popup_enabled` 필드 추가 (auto_english_triggers 다음)
  - `new()` initializer 동일 위치에 추가
  - `parse_emoji_trigger`/`matches_emoji_trigger` 헬퍼 추가 (`new()` 직후)
  - `press_key()` 의 emoji 트리거 분기를 **popup 모드 인터셉트 다음·Control/Alt early-return 직전** 위치에 정확히 삽입 (매핑서 명시 우선순위)
  - 단위 테스트 8개 (`test_emoji_trigger_*`) 추가 — `mod tests` 끝부분.

## Step 3 결과 (Group 2: unim-cli 이전)

- `unim-cli/src/main.rs`:
  - `ConfigKey` enum: ✅ — `EmojiPopup`, `EmojiPopupKeys` 2개 variant 추가 (`AppRules` 직후, line 267-272)
  - `config_show()`: ✅ — `app_rules_label` 출력 직후, `println!()` 빈 줄 직전에 emoji 출력 추가
  - `config_set()`: ✅ — `ConfigKey::AppRules` arm 다음에 `EmojiPopup` / `EmojiPopupKeys` 분기 추가
- `unim-cli/locales/en.yml`: ✅ — `app_rules_label` 직전에 2개 키 추가
- `unim-cli/locales/ko.yml`: ✅ — 동일 위치에 한국어 번역 2개 키 추가

## Step 4 결과 (Group 3: settings_dialog 토글)

- `unim-gui-gtk/src/settings_dialog.rs`:
  - emoji-popup `SwitchRow`: ✅ (hanja_keys row 직후, `group` 반환 직전에 삽입; line ~492)
  - emoji-popup-keys `build_string_list_row`: ✅ (SwitchRow 다음)
  - 시그널 핸들러: ✅ (`save_and_notify(&s.config, "emoji_popup")` — 매핑서 명시 키)

## Step 5 결과 (unim-config/ 폐기 검증)

- `unim-config/` 디렉토리 잔재: 없음 ✅ (per-file 적용으로 신규 생성 차단)

## 최종 변경 파일 목록 (`git status --short`)

```
 M src/config.rs
 M src/hangul/emoji.rs
 M src/input_engine.rs
 M unim-cli/locales/en.yml
 M unim-cli/locales/ko.yml
 M unim-cli/src/main.rs
 M unim-dbus/src/service.rs
 M unim-gnome-extension/dbus_ime.js
 M unim-gnome-extension/extension.js
 M unim-gnome-extension/stylesheet.css
 M unim-gui-common/src/dbus_client.rs
 M unim-gui-common/src/types.rs
 M unim-gui-gtk/src/gtk_ui.rs
 M unim-gui-gtk/src/main.rs
 M unim-gui-gtk/src/settings_dialog.rs
 M unim-gui-qt/src/bridge.rs            # ← develop 전용 보완 (PopupAction 누락 매치 추가)
 M unim-windows/src/ui/popup.rs         # ← develop 전용 보완 (PopupAction 누락 매치 추가)
?? unim-gnome-extension/emoji_popup.js  # 신규
?? unim-gui-gtk/src/emoji_popup.rs      # 신규
```

PR이 건드리지 않았던 `unim-gui-qt/src/bridge.rs`와 `unim-windows/src/ui/popup.rs`에서
`PopupAction::ShowEmoji` 및 `GuiAction::ShowEmojiPopup` non-exhaustive 매치 컴파일 에러가
발생하여, 두 곳에 빈 처리 (Qt: ignore arm, Windows: standalone egui 미지원 ignore) 를 추가.
**기능적 의미 변경 없음** — develop tip의 다른 PopupAction variant들(HanjaBookmarkChanged 등)도 같은 패턴으로 처리됨.

## cargo check 결과

- compile: **PASS** ✅ (`cargo check --workspace`)
- 추가 경고: **0개** (전체 워크스페이스, all-targets) ✅
- toolchain: cargo 1.95.0 (`~/.cargo/bin/cargo`)

## 검증 안 된 항목 (다음 단계)

- 단위 테스트 실행 (`cargo test --workspace -p unim`)
- `cargo build --workspace --release`
- GTK GUI 실행 시 emoji-popup 토글/트리거 키 행 표시
- GNOME Extension reload 후 Super+Period 동작
- CLI 시나리오: `unim-cli config set emoji-popup-keys "Control+Shift+E"`
