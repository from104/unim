# Changelog

All notable changes to the UNIM (Universal Next-generation Input Method) project are recorded in this file.

The format is based on [Keep a Changelog] and this project follows [Semantic Versioning].

## [Unreleased]

### Breaking changes

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

### Added

- **BUILTIN_NAMES × 4-axis integrity test** (`src/keystroke/mod.rs`):
  for each builtin, asserts (a) `get_keymap_json` does not fall back to
  `KO_2BULSTD`, (b) `get_builtin_json` returns the v1 JSON, (c) it parses
  with `schema_version == 1`, (d) Korean builtins have a non-empty
  `combinations` block. Closes the previous English-side coverage gap that
  could re-introduce silent fallback regressions like the early `en_workman`
  miss.

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
