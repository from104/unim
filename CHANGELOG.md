# Changelog

All notable changes to the UNIM (Universal Next-generation Input Method) project are recorded in this file.

The format is based on [Keep a Changelog] and this project follows [Semantic Versioning].

## [Unreleased]

_No changes yet._

---

## [0.3.0] 2026-05-19

### Breaking changes

- **DBus signal `HanjaCandidatesReordered` is now a 10-tuple** (was 9). The new trailing field `was_bookmarked: bool` holds the pre-toggle bookmark state so frontends can render the cursor flash only on un-bookmark (`was_bookmarked && !bookmarked`). External subscribers of `org.atit.unim.InputContext` must upgrade their unpacking — the 9-tuple shape is rejected.
- **Layout profile v0 schema is no longer supported.** v0 (legacy) JSON files
  without any v1 marker (`schema_version`, `metadata`, `inherits`,
  `combinations`, `rule_sets`, `active_rule_sets`) are now rejected by the
  loader with `LoadError::UnsupportedSchema` and a console warning. Convert
  user profiles in `~/.config/unim/layouts/*.json` to v1 by adding
  `"schema_version": 1` and an explicit `combinations` block (see
  [`docs/dev/plans/LAYOUT_PROFILE_V1.md`](docs/dev/plans/LAYOUT_PROFILE_V1.md)).
  Built-in profiles were already migrated to v1 in 0.2.0.

### Removed

- **Rust const jamo combination tables** (`JUNG_COMBINATIONS`,
  `JONG_COMBINATIONS`, `CHO_COMBINATIONS`, `COMBINED_JAMO_2BUL`,
  `COMBINED_JAMO_3BUL` Lazy statics) deleted from
  `src/hangul/composer_with_2bul.rs` and `composer_with_3bul.rs`.
  `HangulComposer{2,3}Bul::new()` now delegates to
  `new_with_profile(load_builtin_profile("ko_2bulstd"|"ko_3bul390"))`. Single
  source of truth for jamo combinations is the v1 builtin profile JSON.
- **`SchemaKind` enum + `detect()`** removed from
  `src/keystroke/profile/schema.rs`. `RawProfile::has_v1_markers()` replaces
  the v0/v1 detection role. Builder's `fallback_for(layout_type)` v0
  compatibility path also deleted.
- **`HangulComposer3BulMoachigi` separate composer** removed. Moachigi logic is now handled entirely within `HangulComposer3Bul` via the `chord_buffer` layer in `InputEngine`. No user-visible behavior change.
- **`emoji_popup.enabled` config field** removed across all 5 sync points (`src/config.rs` `EmojiPopupConfig` struct, `src/input_engine/{engine,press_key}.rs` gate, `unim-cli config emoji-popup` subcommand, `unim-dbus` `GetConfig`/`SetConfig` branches, `unim-gui-gtk` SwitchRow + `row_emoji_popup*` locale keys, `emoji_popup_label` CLI label). The Hanja key idle trigger is now unconditionally always-on — single entry point for both Hanja conversion (during composition) and emoji popup (when idle). Existing `~/.config/unim/config.yaml` lines under `engine.emoji_popup` are silently ignored via `#[serde(default)]` and dropped on next save.

### Added

- **Mouse paginate buttons (◀/▶) on every popup, every frontend**: hanja, special-character, and emoji popups gain ◀ (previous) / ▶ (next) footer buttons across GNOME Shell extension, GTK Standalone (`unim-gui-gtk`), GTK3/4 IM modules, Qt5/6 IM modules, XIM, Wayland (`unim-frontends/wayland`), and Windows egui (`unim-windows/`). Behavior matches keyboard `←`/`→` and `Page Up`/`Page Down` with wrap-around; buttons are hidden when `total_pages == 1`. New unified DBus RPC `popup_change_page(direction: i32)` (0=Prev, 1=Next) is the shared entry point.
- **Hanja un-bookmark cursor flash** (Catppuccin yellow `#f9e2af`, 140 ms): when the user unstars (☆) a candidate, the popup reorders and the cursor jumps to the candidate's lexicographic home — possibly on a different page. The destination cell briefly flashes so the user perceives the auto page jump. Bookmarking (★ on) does not flash since the cursor lands predictably at page 1 row 1. Hanja popup only — special-character / emoji popups have no bookmark concept.
- **Wayland popup pointer input infrastructure** (`unim-frontends/wayland`): `WlPointer` event handling lets popup ◀/▶ receive clicks. Compositor support for `zwp_input_popup_surface_v2` pointer routing is required; keyboard `←`/`→` is the universal fallback (see troubleshooting §7-1).
- **i18n keys**: `popup_previous_page`, `popup_next_page` added to ko/en (`yml` and `po` files, 4 sync points).
- **BUILTIN_NAMES × 4-axis integrity test** (`src/keystroke/mod.rs`):
  for each builtin, asserts (a) `get_keymap_json` does not fall back to
  `KO_2BULSTD`, (b) `get_builtin_json` returns the v1 JSON, (c) it parses
  with `schema_version == 1`, (d) Korean builtins have a non-empty
  `combinations` block. Closes the previous English-side coverage gap that
  could re-introduce silent fallback regressions like the early `en_workman`
  miss.
- **Ahnmatae (안마태 2003) keyboard built-in** (`ko_3bul_anmatae`): First moachigi (chord-based) Korean layout in UNIM. Three-beol layout with fixed cho/jung/jong regions. Includes 9 cho, 15 jung, and 20 jong combination rules. Archaic jamo (옛한글) positions (W/T/G/J/B/N upper) are remapped to Korean typography symbols (`"` `"` `'` `'` `·` `…`); archaic codepoints anywhere in the profile trigger `LoadError::ArchaicJamoNotSupported`.
- **Qwerty Sebeolsik v2 keyboard built-in** (`ko_3bul_qwerty`, v2): Reintroduced under the v3 moachigi schema after being dropped from built-ins in 0.2.0. Alphabet 26-seat saturation (10 cho / 6 jung / 10 jong) on lower — full Korean input without Shift. Upper 26 seats carry differentiated content: 5 doubled jamo (KK→ㄲ, UU→ㄸ, PP→ㅃ, OO→ㅆ, LL→ㅉ), 6 combined medials (T→ㅖ, F→ㅒ, D→ㅢ, G→ㅙ, N→ㅘ, B→ㅞ), 4 aspirated finals (X→ᆿ, C→ᇀ, R→ᇁ, V→ᆾ), 5 combined/doubled finals (Q→ᆻ, W→ᆰ, S→ᆬ, Z→ᆱ, E→ᆭ), and 6 Korean typography symbols (Y→「, I→」, H→※, J→·, A→", M→").
- **Layout profile v3 schema** (`schema_version: 3`): Adds a single top-level capability marker for moachigi layouts:
  - `supports_moachigi: bool` — signals that this layout is chord-capable. The GTK settings dialog reveals the Moachigi group only when this flag is true. Behavior options live in the user config, not the keymap (see below).
- **Moachigi user config** (`~/.config/unim/config.yaml` under `korean.*`): two new opt-in settings, applied only when the active layout has `supports_moachigi=true`:
  - `korean.bidirectional_combine: Option<bool>` — when `true`, cho/jung/jong combinations are attempted in both `(a,b)` and `(b,a)` order. Default unset → **OFF** (opt-in).
  - `korean.chord_window_ms: Option<u16>` — duration of the single chord window in milliseconds. `0` or unset = chord disabled. GUI exposes 10–200 ms via slider. Default unset → **OFF** (opt-in).
- **Moachigi chord engine** (`src/input_engine/chord_buffer.rs`): Single-window chord accumulator. First jamo starts an N-ms tokio timer; jamo arriving within the window are buffered. On expiry: 1 jamo → normal sequential processing; 2+ jamo → region-classified chord compose with bidirectional combine. Flush triggers: idle timeout (tokio timer), Space/Enter/Tab/Backspace/Hanja/etc., mode switch, FocusOut, Escape (discard), MAX 8 jamo.
- **Moachigi v4 — Atomic Window Principle**: The chord window now makes all branching decisions at expiry time, not on each keystroke. 1 jamo in buffer → normal sequential processing; 2+ jamo → region-sorted chord compose with permutation search. This eliminates the previous mid-window commit artifacts.
- **`chord_compose` module** (`src/input_engine/chord_compose.rs`): Region-classified permutation search for chord composition. cho ≤ 2 keys (2 permutations), jung/jong ≤ 3 keys (6 permutations), with fallback to compatibility jamo on no match.
- **Non-jamo keys stay outside the chord window**: punctuation and symbols (e.g., `-`, `,`) no longer join the chord buffer. On window expiry, if a syllable can be formed it is committed first, then the non-jamo character is emitted. If no combination succeeds, compatibility jamo + the non-jamo character are committed in sequence.
- **`bidirectional_combine` semantics clarified**: The option is now independent of `chord_window_ms`. Sequential (time-separated) jamo can also combine bidirectionally — e.g., ㅎ typed before ㄱ produces ㅋ even without chord timing.
- **`chord_window_ms` defaults and range updated**: default 50 ms → **60 ms**, range 10–100 ms → **10–200 ms**.
- **`KoreanConfig::validate_chord_window_ms`**: New validation function. Accepts 0 (chord disabled) or 10–200 ms; rejects all other values with a descriptive error.
- **Backspace restores chord preedit**: pressing Backspace during or after a chord removes jamo in `input_order` reverse sequence and recomposes the remaining syllable, matching the behavior users expect from sequential three-beol.
- **GTK settings dialog — Moachigi group**: New `AdwPreferencesGroup` with a toggle row ("동시 입력 자모 역순 결합" / "Bidirectional Jamo Combine") and a slider row ("동시 입력 시간 (ms)" / "Chord Window (ms)", 10–200 ms, default 60 ms, tick marks at 10 / 50 / 100 / 150 / 200). Group is shown only when the selected layout has `supports_moachigi=true`; hidden automatically when switching to other layouts. Tooltips note independence of the two moachigi options and recommend 100–150 ms for beginners.
- **User guide** for Ahnmatae keyboard: `docs/user/keymaps/anmatae.md` (Korean) and `docs/user/keymaps/anmatae.en.md` (English).
- **User guide — keyboard compatibility section**: `docs/user/keymaps/anmatae.md` and `anmatae.en.md` gain a new "Keyboard Compatibility (NKRO Recommended)" section covering KRO limits, USB polling rate, and a ghosting self-diagnosis guide (`xev`, online key tester, window-expansion test).
- **Troubleshooting — moachigi section** (§15): `docs/user/troubleshooting/README-ko.md` and `README.md` gain "모아치기(chord)가 제대로 인식 안 됨" / "Moachigi not recognized correctly" covering the five most common root causes: window too short, NKRO not supported, low USB polling rate, bidirectional_combine off, and layout not moachigi-capable.

### Changed

- **Settings dialog live help enrichment** (`unim-gui-gtk/src/settings_dialog.rs` + `locales/{ko,en}.yml`): Every settings row in the dialog gained richer subtitles and tooltips in both Korean and English — 26 tooltips reworked, 15 subtitles reworked, 5 new i18n keys added (`row_moachigi_bidirectional_subtitle`, `row_moachigi_chord_subtitle`, `userdict_group_desc_count`, plus enriched `mode_share_*` labels). Tooltips now follow a consistent four-element template (what / when / why / recommended-value-or-side-effect) with concrete numeric guidance (e.g., AutoTypeFix forward `2`, reverse `3`, observe window `5–15s`, tentative expiry `1–12h`) and explicit X11 / GNOME Wayland behavior differences where applicable. Domain terminology unified across locales (`AutoTypeFix`, `순방향`/`역방향`, `preedit`/`조합`, `IME`/`실시간 입력기`).
- `chord_window_ms` slider in the GTK settings dialog: previous range 10–100 ms is now 10–200 ms; previous default 50 ms is now 60 ms.
- `bidirectional_combine` tooltip text updated to emphasize that it operates independently of the chord window and applies to sequential input as well.

### Fixed (best-effort)

- **XIM `commit_then_preedit` now forces `clear_preedit()` before `commit()`** (`unim-frontends/xim/src/handler.rs:378-`): xim-0.5.0/src/server.rs:236-248 `commit()` does not toggle `preedit_started`, so a subsequent `preedit_draw()` skips the PreeditStart re-emission (server.rs:205-214). Forcing `clear_preedit()` first makes the crate emit `PreeditDraw(empty) + PreeditDone` and resets `preedit_started=false`, so the new `preedit_draw()` re-fires PreeditStart cleanly. This restores the post-commit preedit on the typical OVER-THE-SPOT path (XTerm, WezTerm) but **does not fully resolve the regression on every ON-THE-SPOT (PREEDIT_CALLBACKS) client** — see Known Issues.

### Known Issues

- **XIM ON-THE-SPOT (PREEDIT_CALLBACKS) preedit drop after commit (UNRESOLVED)**: After committing a Hangul syllable, the next jamo's preedit is invisible for one frame and only renders once an additional jamo arrives. Reproduces in custom XIM clients (e.g. `unim-test-xim`) and some ON-THE-SPOT XIM apps. **Unaffected**: XTerm, WezTerm, other OVER-THE-SPOT clients, GTK3/4, Qt5/6, Wayland, GNOME extension. Best-effort mitigation above shipped; root cause is xim-0.5.0's `commit()` not driving the `preedit_started` state machine — needs an upstream xim fix or a redesigned protocol sequence on the UNIM side. Tracked in `docs/user/troubleshooting/README.md` §B.

### Removed (continued)

- **Qwerty Sebeolsik (`ko_3bul_qwerty`) dropped from built-ins**: removed from `BUILTIN_NAMES` (10 → reduced) and from the `get_builtin_json` / `get_keymap_json` match arms in `src/keystroke/profile/builtin.rs` and `src/keystroke/mod.rs`. The full v3 moachigi schema JSON is preserved as a research reference at `docs/references/keymaps/ko_3bul_qwerty_v2.json` — copy it to `~/.config/unim/layouts/ko_3bul_qwerty.json` to keep using it as a user profile. CLI `--korean` help, `unim-cli config set korean-layout` enumeration, GTK settings dialog alias map, FAQ §Q7, and troubleshooting §15-1 all updated accordingly.

## [0.2.0] 2026-04-26

### Added

- **Layout Profile v1 (spec + engine + config + CLI + GUI)**: Built-in keyboard layouts are now self-contained v1 JSON profiles (`src/keystroke/keymap/*.json`), replacing the hybrid Rust-const + partial-JSON path.
  - **User profiles**: Drop a v1 JSON into `~/.config/unim/layouts/*.json` and the daemon scans on startup with mtime-based hot reload.
  - **inherits chain resolution**: Child profiles declare `"inherits": "base_name"`; `ProfileRegistry` resolves the chain with cycle detection and layer-merged metadata/layout/rule_sets.
  - **Rule sets**: Each profile may declare named optional subrules (`rule_sets.<name>`) — e.g., `sun_arae_batchim` on `ko_3bul390` — toggled via GUI SwitchRow or CLI `set korean-active-rule-sets`.
  - **Config fields** (additive, zero impact when unset): `korean.custom_layout: Option<String>` and `korean.active_rule_sets: Vec<String>`. Wired through the 5-point sync (config.rs ↔ `unim-cli config` ConfigKey ↔ locales ↔ unim-dbus ↔ settings dialog).
  - **`unim-cli config layout` subcommand**: `list` / `describe <name>` / `validate <file.json>` (exit codes 0=pass, 1=warnings, 2=errors).
  - **GUI — Adw.ComboRow + dynamic SwitchRows**: Settings dialog lists all Korean profiles (10 built-in + user) and shows the selected profile's rule sets as live toggleable SwitchRows.
  - **New built-in profile — `ko_3bul_qwerty`** (쿼티형 세벌식): Shift-free 26-seat alphabet saturation layout (14 초성 / 15 중성 / 19 종성). Built-in count 9 → 10.
  - Spec: [`docs/dev/plans/LAYOUT_PROFILE_V1.md`](docs/dev/plans/LAYOUT_PROFILE_V1.md).
- **AutoTypeFix rollback-learned blacklist suppression** (`src/typefix_blacklist.rs`, `~/.config/unim/typefix-blacklist.yaml`): Observes the rollback pattern (backspace + input-mode switch on top of the last correction). On a second AutoTypeFix attempt with the same ASCII (retrigger), registers a tentative suppression entry and suppresses that very attempt in one step. Manual GUI "Confirm" promotes Tentative → Confirmed; tentatives flip to Inactive after `tentative_expiry_hours` (default 1, range 1..=12). Daemon auto-reloads on mtime change.
- **AutoTypeFix settings**: three new keys under `auto_typefix.*` — `rollback_detection` (bool, default true), `tentative_expiry_hours` (u16, default 1, range 1..=12), `observation_timeout_secs` (u8, default 10, range 5..=15). All three wired through the 3-point sync.
- **Settings GUI "Suppression Words" page** (`unim-gui-gtk`): New `Adw.PreferencesPage` with three groups (Tentative / Confirmed / Inactive) and Confirm / Deactivate / Remove / Reactivate row actions.
- **Hanja popup 9×9 expanded grid mode**: Period key toggles compact (9) ↔ expanded (81) modes across GTK Standalone, GTK IM, Qt IM, and XIM frontends, matching the GNOME extension. ⊞/⊟ icon indicates current mode.
- **Hanja bookmark UI** (☆/★): Space toggles bookmark on the focused candidate; live `HanjaBookmarkChanged` DBus signal refreshes all open popups across GTK/Qt/XIM/Wayland/GNOME.
- **Reverse AutoTypeFix user dictionary**: Register selected text as an English-side dictionary entry via shortcut (`RegisterUserDictFromSelection` DBus method); GUI page for add/remove/update entries.
- **Auto-English mode switching on trigger keys**: Configurable trigger key list (e.g., `:`, `/`) auto-switches Korean → English mode at boundary characters; default trigger set is empty for backward compatibility.
- **Emoji popup (Super+.)** with category tabs, search, and MRU favorites: GTK Standalone (`unim-gui-gtk/src/emoji_popup.rs`) + GNOME Shell extension (`unim-gnome-extension/emoji_popup.js`) implementations.

### Changed

- **`KoreanLayout` enum removed (Phase 8)**: The Korean layout field is now a plain profile-name string (`KoreanLayout` is a public `String` type alias). `korean.layout` accepts any built-in (`ko_2bulstd`, `ko_3bul390`, `ko_3bul391`, `ko_3bul_noshift`, `ko_3bul_qwerty`) or a user profile name. Legacy `custom_layout: Option<String>` field merged into `layout`. Existing `config.yaml` with `layout: Dubeolsik` and `typefix-blacklist.yaml` entries auto-normalize via serde compat layers. C API setters/getters now take/return C strings.
- **`EnglishLayout` enum removed (Phase 9)**: Symmetric to the Korean change. `english.layout` is now a String (built-ins: `qwerty` / `dvorak` / `colemak` / `colemak_dh` / `workman`). Legacy YAML values auto-normalized via serde `from = "EnglishConfigCompat"`. C API: `UnimEnglishLayout` enum deleted; setters/getters take/return C strings.
- **AutoTypeFix reverse-direction rollback gate relaxed from BS-AND-switch to BS-OR-switch**. Reverse corrections use `clear_preedit=true`, so IM modules consume the rollback Backspace locally and never forward it to `engine_worker` — the AND gate was structurally unreachable. Mode-switch alone is now sufficient for reverse. Forward keeps BS-AND-switch.
- **AutoTypeFix reverse-direction suppression key fixed**: `RecentCorrection.ascii` now stores `fix.corrected` for reverse and `fix.original` for forward. Previously every reverse entry was blacklisted as `""`, never matching subsequent queries.
- **AutoTypeFix blacklist registration moved from rollback-moment to retrigger-moment**. The earlier "register-on-rollback" model produced false positives; now BS/mode-switch only flag the pending correction, and the tentative entry is added at the retrigger.
- **`unim-config` orphaned crate removed**: Legacy CLI subcrate folded into `unim-cli config` subcommand (single source of truth for config CLI).
- Refactored `unim-gui` tray icons and popups to synchronize immediately upon receiving the `GlobalModeChanged` signal from `unim-daemon`.

### Fixed

- **IME — Space in English mode is now committed via the direct-commit path** (`consumed=true`, `commit=" "`), matching the Korean-mode path. Previously English-mode Space returned `not_consumed`, causing GTK IM modules to intermittently drop spaces (observed in gedit).
- **IME — Focus-out no longer emits a duplicate `CommitText` DBus signal** on top of the RPC return value. The signal is not context-scoped, so broadcasting it caused characters like `늘` to be committed twice in gedit.
- **AutoTypeFix — `tentative_expiry_days` (1..=90) renamed to `tentative_expiry_hours` (1..=12)**. The days unit was too coarse for practical blacklist curation.
- **TypeFix surrounding-text support for gedit/gnome-text-editor**: GTK IM modules now use `request_surrounding()` to fetch context, enabling reverse correction in apps that previously didn't expose committed text.
- **GTK preedit-end keylock bug**: GTK3/4 IM modules now emit `preedit-end` via the `unim_emit_preedit` helper, fixing ghostty/terminal key-lock that occurred when preedit ended without an explicit signal.
- **XIM AutoTypeFix re-implementation**: Switched to the N+1 BS protocol model so XIM frontends correctly handle multi-character corrections (Chrome preedit edge case still pending).

## [0.1.0] 2026-04-21 — Initial Release

The first official release of UNIM (Universal Next-generation Input Method). A Korean input method engine redesigned from scratch in Rust, composed of the following components.

### Added — Engine Core

- **Pure Rust Hangul engine (`src/`)**: 2-bul / 3-bul 390 / 3-bul 391 Hangul composition and decomposition logic. Zero UI/platform dependencies.
- **DBus daemon architecture (`unim-daemon` + `unim-dbus`)**: System-wide input state management based on D-Bus session activation. Service name `org.atit.unim.InputMethod`.
- **C-API wrapper (`unim-capi` / `libunim_capi`)**: Exposes the Rust core for use from C/C++ frontends.
- **Unified CLI (`unim-cli`)**: Hangul↔English converter + `config` subcommand (show / set / path / reset / interactive).

### Added — Frontends

- **GTK input method modules**: GTK3 (`unim-frontends/gtk3/`) and GTK4 (`unim-frontends/gtk4/`) modules with shared component `unim-frontends/gtk-common/`.
- **Qt platform input context plugins**: Qt5 (`unim-frontends/qt5/`) and Qt6 (`unim-frontends/qt6/`) `QPlatformInputContext` implementations with shared `unim-frontends/qt-common/`.
- **XIM frontend (`unim-frontends/xim/`)**: Native Rust X11 XIM protocol implementation, Over-The-Spot Preedit support, verified against 11 conformance items of the X11R7.6 XIM specification.
- **Wayland frontend (`unim-frontends/wayland/`)**: Supports `input-method-v2` + `virtual-keyboard-v1` protocols, foundational KDE Plasma support, and `zwp_input_popup_surface_v2` integration for hanja/special-character popups.
- **GNOME Shell extension (`unim-gnome-extension/`)**: Native integration JS extension with layout conversion shortcuts (`gksrmf` ↔ `한국어`), terminal-aware paste mode, etc.

### Added — GUI

- **GTK4/libadwaita settings dialog (`unim-gui-gtk`)**: Tray icon, hanja/special-character popups, settings dialog.
- **Qt6/cxx-qt alternative GUI (`unim-gui-qt`)**: GTK alternative. Coexists with `unim-gui-gtk` without conflict.
- **im-config integration**: Automatic linkage with the system IM selection tool.

### Added — Features

- **Korean layouts**: 2-bul (Dubeolsik standard) + 3-bul (Sebeolsik 390 / 391 / no-shift) built-ins.
- **AutoTypeFix (TypeFix)**: Automatic Korean↔English typo correction (forward: English typed → Korean, reverse: Korean typed → English). Supported on XIM / GTK / Qt / GNOME.
- **Hanja conversion**: Hanja conversion popup with search, pagination, and index key navigation.
- **Special-character / emoji search**: Search popup for special characters and emoji.
- **Per-application input mode rules**: Application-specific input mode auto-switching rules.

### Added — Packaging & Documentation

- **Debian packaging — 9 binary packages** (`debian/control`):
  - `unim-common` (core + daemon + CLI + libunim_capi)
  - `unim-im-gtk` (GTK3/4 IM modules)
  - `unim-im-qt` (Qt5/6 plugins)
  - `unim-xim` (X11 XIM frontend)
  - `unim-wayland` (Wayland input-method frontend)
  - `unim-gui-gtk` (GTK4/libadwaita settings GUI + tray)
  - `unim-gui-qt` (Qt6/cxx-qt settings GUI + tray, alternative)
  - `unim-gnome` (GNOME Shell extension, depends on `unim-gui-gtk`)
  - `unim` (meta-package — full stack)
- **Comprehensive documentation**: 12 component-specific `SPEC.md` files, `IME_BEHAVIOR.md` (frontend behavior consistency), `POPUP_SPEC.md` (unified popup design).

[Keep a Changelog]: https://keepachangelog.com/en/1.0.0/
[Semantic Versioning]: https://semver.org/spec/v2.0.0.html
