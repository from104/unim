# Changelog

All notable changes to the UNIM (Universal Next-generation Input Method) project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added
- **UI Frontend Separation Preparation (Phase 3.5 Planning)**: Initiated plans to decouple the engine (daemon) and UI (popup/indicator/settings) based on DBus signals, enabling toolkit-specific native GUI support.
- **XIM and Wayland Protocol Reference Documentation**: Added comprehensive documentation referencing `input-method-v2` and `virtual-keyboard-v1` protocol specifications and architectural details.
- **Wayland Frontend Specifics**: Implemented foundational support for KDE Plasma environments utilizing the Wayland protocol.
- **XIM Compatibility Verification**: Completed protocol conformity verification against the X11R7.6 XIM specification (11 conformance items).
- **AutoTypeFix rollback-learned blacklist suppression** (`src/typefix_blacklist.rs`, `~/.config/unim/typefix-blacklist.yaml`): observes the natural rollback pattern (backspace erasing the corrected result + input-mode switch) on top of the last correction, and upon a **second AutoTypeFix attempt with the same ASCII** (retrigger), registers a tentative suppression entry and suppresses that very attempt in one step. Manual promotion via the GUI "Confirm" button turns Tentative into Confirmed; tentatives flip to Inactive after `tentative_expiry_hours` (default 1, range 1..=12). Records are preserved for audit — only the suppression effect is removed. The daemon auto-reloads `typefix-blacklist.yaml` on mtime change, so external YAML edits and GUI changes both take effect immediately.
- **AutoTypeFix prefix-avoidance for reverse direction**: when the current ASCII sequence is itself in the dictionary but also a *strict prefix* of a longer dictionary word (e.g. `wood` → `woody`, `woodpecker`), firing is deferred so the user can keep typing. Gated by `auto_typefix.skip_on_prefix_collision` (default true). Independent of the blacklist.
- **AutoTypeFix settings**: four new keys under `auto_typefix.*` — `skip_on_prefix_collision` (bool, default true), `rollback_detection` (bool, default true), `tentative_expiry_hours` (u16, default 1, range 1..=12), `observation_timeout_secs` (u8, default 10, range 5..=15). All four are wired through the 5-point sync (config.rs ↔ unim-config CLI ↔ locales ↔ unim-dbus legacy key dispatch ↔ GTK settings dialog); YAML/JSON DBus endpoints pick them up automatically via serde.
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
- **Standalone CLI (`unim-cli`)**: Portable command-line interface for the core logic, usable as an independent converter.
- **Configuration Tool (`unim-config`)**: CLI utility for managing engine settings.
- **GUI Settings and Popup (`unim-gui`)**: Centralized module for the system tray, Hanja/Special character popups, and settings.
- **GTK Frontends**: C-based IM modules for GTK3 (`unim-frontends/gtk3/`) and GTK4 (`unim-frontends/gtk4/`) with shared components (`unim-frontends/gtk-common/`).
- **Qt Frontends**: C++-based QPlatformInputContext plugins for Qt5 (`unim-frontends/qt5/`) and Qt6 (`unim-frontends/qt6/`) with shared components (`unim-frontends/qt-common/`).
- **XIM Frontend (`unim-frontends/xim/`)**: Native Rust-based X11 XIM protocol frontend with Over-The-Spot Preedit support.
- **Wayland Frontend (`unim-frontends/wayland/`)**: Native Rust-based Wayland frontend supporting `input-method-v2` and `virtual-keyboard-v1` protocols.
- **GNOME Shell Extension (`unim-gnome-extension/`)**: JavaScript extension for native integration, including layout conversion shortcuts (e.g., 'gksrmf' ↔ '한국어') and terminal-aware paste modes.
- **Comprehensive Documentation**: Included 12 component-specific `SPEC.md` files, `IME_BEHAVIOR.md` for consistent frontend behavior, and `POPUP_SPEC.md` for unified popup design.
