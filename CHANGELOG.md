# Changelog

All notable changes to the UNIM (Universal Next-generation Input Method) project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Changed
- **`KoreanLayout` enum removed (Phase 8)**: The Korean layout field is now a plain profile-name string (`KoreanLayout` is a public `String` type alias). `korean.layout` can hold any built-in (`ko_2bulstd`, `ko_3bul390`, `ko_3bul391`, `ko_3bul_noshift`, `ko_3bul_qwerty`) or a user profile name. Legacy `custom_layout: Option<String>` field is gone — merged into `layout`. Backward compat:
  - Existing `~/.config/unim/config.yaml` with `layout: Dubeolsik` (or `Sebeolsik390` / `2bul` / `390` etc.) is auto-normalized on load via a serde `from` compat layer that also absorbs the Phase 4b `custom_layout` override.
  - `~/.config/unim/typefix-blacklist.yaml` entries with `korean_layout: Dubeolsik` are similarly promoted via `deserialize_with`.
  - DBus key `korean_custom_layout` still works; it now sets `korean.layout` directly (empty value resets to `ko_2bulstd`).
  - C API (`unim-capi`): `unim_config_set_korean_layout` and `unim_engine_set_korean_layout` now take `const char *layout`; `unim_config_get_korean_layout` returns `UnimStr`; `unim_korean_layout_{name,display_name,at}` take a `size_t` index and enumerate all five built-ins (including the new `ko_3bul_qwerty`).
  - GUI ComboRow already used profile names since Phase 5 so user-visible behavior is unchanged.

### Added
- **Debian packaging — split into 9 binary packages** (`debian/control`): `unim-common` (core + daemon + CLI + libunim_capi), `unim-im-gtk` (GTK3/4 IM modules), `unim-im-qt` (Qt5/6 plugins), `unim-xim`, `unim-wayland`, `unim-gui-gtk`, `unim-gui-qt`, `unim-gnome` (Shell extension, depends on `unim-gui-gtk`), and the `unim` meta-package pulling in the full stack. Power users pick only what they need; `apt install unim` still gives everyone the previous one-shot install experience. Frontends coexist — `unim-gui-gtk` and `unim-gui-qt` do not conflict.
- **Layout Profile v1 (spec + engine + config + CLI + GUI)**: Built-in keyboard layouts are now self-contained v1 JSON profiles (`src/keystroke/keymap/*.json`), replacing the hybrid Rust-const + partial JSON path. New capabilities:
  - **User profiles**: Drop a v1 JSON into `~/.config/unim/layouts/*.json` and the daemon scans on startup with mtime-based hot reload.
  - **inherits chain resolution**: Child profiles declare `"inherits": "base_name"`; `ProfileRegistry` resolves the chain with cycle detection and layer-merged metadata/layout/rule_sets.
  - **Rule sets**: Each profile can declare named optional subrules (`rule_sets.<name>`) — e.g., `sun_arae_batchim` on `ko_3bul390` — toggled via GUI SwitchRow or CLI `set korean-active-rule-sets`.
  - **Config fields** (additive, zero impact when unset): `korean.custom_layout: Option<String>` (pins a specific profile name, takes precedence over `korean.layout` enum) and `korean.active_rule_sets: Vec<String>` (empty = use profile default). Wired through the 5-point sync (config.rs ↔ `unim-cli config` ConfigKey ↔ locales ↔ unim-dbus ↔ settings dialog).
  - **`unim-cli config layout` subcommand**: `list` (built-in + user profiles), `describe <name>` (metadata + combinations + rule sets), `validate <file.json>` (schema + jamo resolution + rule-set reference integrity). Exit codes 0=pass, 1=warnings, 2=errors.
  - **GUI — Adw.ComboRow + dynamic SwitchRows**: Settings dialog now lists all Korean profiles (10 built-in + user) and shows the selected profile's rule sets as live toggleable SwitchRows. Selection switches apply immediately via DBus `SetConfigYaml`.
  - **Built-in profile added — `ko_3bul_qwerty`** (쿼티형 세벌식): Shift-free, 26-seat alphabet saturation layout with 14 초성 / 15 중성 / 19 종성 combinations. Raises the built-in count from 9 to 10.
  - Spec: [`docs/plans/LAYOUT_PROFILE_V1.md`](docs/plans/LAYOUT_PROFILE_V1.md), implementation harness: [`docs/plans/LAYOUT_PROFILE_V1_IMPL.md`](docs/plans/LAYOUT_PROFILE_V1_IMPL.md).
- **UI Frontend Separation Preparation (Phase 3.5 Planning)**: Initiated plans to decouple the engine (daemon) and UI (popup/indicator/settings) based on DBus signals, enabling toolkit-specific native GUI support.
- **XIM and Wayland Protocol Reference Documentation**: Added comprehensive documentation referencing `input-method-v2` and `virtual-keyboard-v1` protocol specifications and architectural details.
- **Wayland Frontend Specifics**: Implemented foundational support for KDE Plasma environments utilizing the Wayland protocol.
- **XIM Compatibility Verification**: Completed protocol conformity verification against the X11R7.6 XIM specification (11 conformance items).
- **AutoTypeFix rollback-learned blacklist suppression** (`src/typefix_blacklist.rs`, `~/.config/unim/typefix-blacklist.yaml`): observes the natural rollback pattern (backspace erasing the corrected result + input-mode switch) on top of the last correction, and upon a **second AutoTypeFix attempt with the same ASCII** (retrigger), registers a tentative suppression entry and suppresses that very attempt in one step. Manual promotion via the GUI "Confirm" button turns Tentative into Confirmed; tentatives flip to Inactive after `tentative_expiry_hours` (default 1, range 1..=12). Records are preserved for audit — only the suppression effect is removed. The daemon auto-reloads `typefix-blacklist.yaml` on mtime change, so external YAML edits and GUI changes both take effect immediately.
- **AutoTypeFix settings**: three new keys under `auto_typefix.*` — `rollback_detection` (bool, default true), `tentative_expiry_hours` (u16, default 1, range 1..=12), `observation_timeout_secs` (u8, default 10, range 5..=15). All three are wired through the 3-point sync (config.rs ↔ `unim-cli config` ↔ GTK settings dialog); YAML/JSON DBus endpoints pick them up automatically via serde.
- **Settings GUI "억제 단어" page** (`unim-gui-gtk`): new `Adw.PreferencesPage` with three `Adw.PreferencesGroup`s (Tentative / Confirmed / Inactive) and Confirm / Deactivate / Remove / Reactivate row actions. Reverse-direction rows show the Korean jamos the user actually saw as the title with the committed English ASCII as the subtitle; forward-direction rows show the ASCII as the title with the Korean conversion result as the subtitle.

### Changed
- Refactored `unim-gui` tray icons and popups to synchronize immediately upon receiving the `GlobalModeChanged` signal from `unim-daemon`.
- **AutoTypeFix reverse-direction rollback gate relaxed from BS-AND-switch to BS-OR-switch**. Reverse corrections use `clear_preedit=true`, so IM modules consume the rollback Backspace locally and never forward it to `engine_worker` — the AND gate was structurally unreachable. Mode-switch observation alone is now sufficient evidence of a rollback for the reverse direction. Forward direction keeps the BS-AND-switch gate.
- **AutoTypeFix reverse-direction suppression key fixed**: `RecentCorrection.ascii` now stores `fix.corrected` (the committed English word) for reverse and `fix.original` (the ASCII run) for forward. Previously every reverse entry was blacklisted as `""`, which never matched any subsequent query.
- **AutoTypeFix blacklist registration moved from rollback-moment to retrigger-moment**. The earlier "register-on-rollback" model produced false positives from isolated mode switches and did not match forward-direction intuition. Now BS/mode-switch observations only flag the pending correction; the tentative entry is added and the duplicate attempt suppressed in one step at the retrigger.

### Fixed
- **IME — Space in English mode is now committed via the direct-commit path** (`consumed=true`, `commit=" "`), matching the Korean-mode path. Previously English-mode Space returned `not_consumed`, causing GTK IM modules to intermittently drop spaces (observed in gedit).
- **IME — Focus-out no longer emits a duplicate `CommitText` DBus signal** on top of the RPC return value. The signal is not context-scoped, so broadcasting it alongside the return value caused characters like `늘` to be committed twice in gedit. The `FocusOut()` RPC return value is now the sole commit channel on focus-out.
- **AutoTypeFix — `tentative_expiry_days` (1..=90) renamed to `tentative_expiry_hours` (1..=12)**. The days unit was too coarse for practical blacklist curation. Users with existing YAML should remove the old key; the new default (1 hour) applies automatically.

## [0.0.1] - 2024-03-XX (Initial Foundation)

### Added
- **Core Engine (`src/`)**: Pure Rust core library capable of handling all Hangul composition and decomposition logic (2-bul, 3-bul 390, 391 standards) without external dependencies.
- **DBus Daemon Architecture**: Centralized engine server (`unim-daemon`) and DBus library (`unim-dbus`) to manage system-wide input states.
- **C-API Wrapper (`unim-capi`)**: FFI binding to expose the Rust core engine to C/C++ frontends.
- **Unified CLI (`unim-cli`)**: Portable command-line tool combining the core Hangul↔English converter with a `config` subcommand (show / set / path / reset / interactive) for managing engine settings.
- **GUI Settings and Popup (`unim-gui`)**: Centralized module for the system tray, Hanja/Special character popups, and settings.
- **GTK Frontends**: C-based IM modules for GTK3 (`unim-frontends/gtk3/`) and GTK4 (`unim-frontends/gtk4/`) with shared components (`unim-frontends/gtk-common/`).
- **Qt Frontends**: C++-based QPlatformInputContext plugins for Qt5 (`unim-frontends/qt5/`) and Qt6 (`unim-frontends/qt6/`) with shared components (`unim-frontends/qt-common/`).
- **XIM Frontend (`unim-frontends/xim/`)**: Native Rust-based X11 XIM protocol frontend with Over-The-Spot Preedit support.
- **Wayland Frontend (`unim-frontends/wayland/`)**: Native Rust-based Wayland frontend supporting `input-method-v2` and `virtual-keyboard-v1` protocols.
- **GNOME Shell Extension (`unim-gnome-extension/`)**: JavaScript extension for native integration, including layout conversion shortcuts (e.g., 'gksrmf' ↔ '한국어') and terminal-aware paste modes.
- **Comprehensive Documentation**: Included 12 component-specific `SPEC.md` files, `IME_BEHAVIOR.md` for consistent frontend behavior, and `POPUP_SPEC.md` for unified popup design.
