# Changelog

All notable changes to the UNIM (Universal Next-generation Input Method) project are recorded in this file.

The format is based on [Keep a Changelog] and this project follows [Semantic Versioning].

## [Unreleased]

---

## [0.4.0] 2026-07-19

This release brings one-line installation (`curl … | bash`), a first-run setup wizard, a Slint-based cross-platform settings app, Keymap Studio and Typing Practice tools, word-unit input, and a large (experimental) Windows port.

### ✨ Added

- **One-click install (`curl … | bash`)**: You can now install UNIM with a single command.

  ```bash
  curl -fsSL https://raw.githubusercontent.com/from104/unim/main/install.sh | bash
  ```

  The script verifies a Linux / amd64 / apt environment, downloads the 11 deb packages from the latest GitHub release, verifies their integrity against the `SHA256SUMS` manifest, and installs them with `apt-get`. It finishes with re-login and first-run wizard guidance. Environment variables (`UNIM_VERSION`, `UNIM_BASE_URL`) let you pin a specific version or point at a mirror.

- **First-run setup wizard**: The first time you log in after installation, the wizard runs automatically and sets UNIM as your default input method (via im-config, falling back to xinputrc). If setting the default fails, it shows an error card and blocks completion. Once completed, it does not appear again (recorded in XDG state).

- **Slint settings app (`unim-settings`)**: The settings app has been rewritten from GTK4 into a Slint-based cross-platform app, so Linux and Windows share the same settings screen. On Linux it saves to `config.yaml` and notifies the daemon over DBus exactly like the old GTK app, and it raises the existing window instead of launching a duplicate. It queries system fonts so Hangul does not render as tofu on distributions without a Korean font.

- **Korean/English switch beep**: A short beep signals when the mode changes (880 Hz for Korean, 440 Hz for English). It works with no external library or sound file and does not delay input. It sounds for the toggle key, AutoTypeFix auto-switching, and switches from the tray/extension. It can be turned off in settings.

- **Key auto-repeat suppression (accessibility)**: You can enable having the daemon ignore the OS auto-repeat (rapid re-fire) that happens when a key is held down. It applies to the Korean/English toggle key and to character keys in Korean mode; repeats of editing keys (Backspace, arrows) and direct English typing are left alone. It is meant for users with motor disabilities who tend to hold keys too long (e.g. tremor). **The default is off**, so nothing changes until you enable it. Turn it on with the "Suppress Composition Key Auto-repeat" switch in the settings app's accessibility section, or `unim-cli config set ignore-key-repeat true`. Wayland, Qt5/6, and the GNOME extension detect repeats precisely; the GTK3/4, XIM, and ibus-compatible paths approximate it with an 80 ms time window (the first repeat may slip through, and if your system key-repeat interval is set longer than 80 ms it may not be filtered — in either case it errs toward suppressing less, fail-safe).

- **Keymap Studio and Typing Practice (new GTK4 tools)**: Two standalone apps let you view, edit, and practice Korean layouts in one place.
  - **Keymap Studio (`unim-keymap-studio`)**: View the key placement and cho/jung/jong combinations of built-in and user layouts in a table, click a key to edit it, and save to a user layout (`~/.config/unim/layouts/*.json`).
  - **Typing Practice (`unim-typing-practice`)**: Pick a sample text by length and practice while live WPM, CPM, accuracy, and error rate are shown; a per-key error heatmap appears when you finish. It decomposes Hangul syllables into jamo to count keystrokes, and WPM is computed the Korean-standard way (CPM ÷ 5).

- **Auto-English switching — Ctrl/Alt/Super combination triggers**: Auto-English triggers now accept modifier combinations (e.g. `key:Ctrl+B`, `key:Ctrl+Shift+B`, `key:Alt+F1`, `key:Super+Space`). For workflows that need a combo key, such as the tmux/wmux prefix (`Ctrl+B`), pressing the combination in Korean mode switches to English and passes the key straight through to the application. It fires only when exactly the specified modifiers are held. Works on GTK, Qt, XIM, and GNOME; Windows support is planned.

- **AutoTypeFix toggle shortcuts (three)**: You can toggle automatic typo correction with a shortcut — separately for all / forward only (English → Korean) / reverse only (Korean → English). **The all-toggle defaults to `Shift+F9`**; the forward- and reverse-only shortcuts are empty (set them only if you want them). Shortcuts accept modifier combinations (`Shift+F9`, `Ctrl+F8`, …) and fire only on an exact modifier match, so combinations you did not configure (e.g. `Shift+F10` for the context menu) pass through to the application. Bare `F9` still opens the hanja/emoji popup as before. Holding the key (auto-repeat) toggles once without flicker. Clear the list to disable. On Windows, combination specs do not work yet — use a key without modifiers there.

- **Automatic password-field protection**: When you enter a password or other sensitive field, UNIM automatically switches to English mode and restores the previous state when you leave. Keys typed there are not retained anywhere — not in buffers, undo, recent corrections, the learning dictionary, or surrounding context. It works on Wayland, GTK, and Qt; XIM and IMM32 fallback environments are not detected, which is noted in the FAQ and troubleshooting docs.

- **Word-unit input (`commit_unit`)**: You can set the commit unit to word instead of syllable. In word mode the whole word stays in composition even after a correction. To avoid misbehavior, terminals, XIM/ibus-family frontends, and chord layouts are automatically downgraded to syllable units (WordGate). Selectable from the CLI (`unim-cli config`) and the settings app.

- **Distinct icons for four apps + reverse-DNS naming**: The indicator, settings, Keymap Studio, and Typing Practice apps each ship a distinct icon. App IDs now follow reverse-DNS naming consistently (app ID == `.desktop` file name == icon name, e.g. `io.github.from104.unim.Settings`), so the icon shows up correctly in the GNOME Wayland taskbar and Overview. The tray Korean/English status icon is unchanged.

### 🔄 Changed

- **Two settings apps — Slint primary, GTK legacy**: The new Slint settings app takes over the `unim-settings` name, and the previous GTK4 app is renamed `unim-settings-gtk` and shipped alongside for now (to be retired later). Only the Slint app appears in the desktop menu (the GTK one is hidden), and every component's "Open settings" (tray, GNOME extension, Typing Practice, etc.) now points at the new app.

- **Reorganized into 11 deb packages**: The packaging layout was tidied up. The settings app is split into `unim-settings` (Slint) and `unim-settings-gtk` (legacy), `unim-keymap-studio` and `unim-typing-practice` are added as new packages, and the indicator/settings/popup-service are bundled into `unim-desktop`.

- **Keymap Studio redesigned**: The old left-sidebar + two-tab layout is replaced by a three-step header dropdown (Language › Source › Layout) plus four tabs (Basics / Layout / Combinations / Extensions). Built-in layouts are protected (only "Save As" is allowed); your own layouts save in place. See `unim-keymap-studio/README.md`.

- **Layout list enumerated dynamically**: The settings app's layout list changed from a fixed set of four to an enumeration of actually registered profiles (`ProfileRegistry`), so Ahnmatae and user layouts appear with friendly names and the chord-related UI is enabled accordingly. The CLI layout validator uses the same source.

- **GNOME extension icons refreshed**: The tray/panel icons were replaced with a monochrome SVG set, and the input-method-disabled (`unim-disabled`) state is shown with its own icon.

### 🐛 Fixed

- **Right Alt (RightAlt) Korean/English toggle now works everywhere**: Two separate problems kept Right Alt from toggling. In the engine the modifier-key check ran before the toggle-key check (fixed, with an added regression test for jongseong combinations); in the frontends, GTK3/4, Qt5/6, and the GNOME extension filtered bare Right Alt themselves, so it never reached the daemon at all (XIM and pure Wayland already worked). The frontends' own skip has been removed and the toggle decision is now made solely by the daemon, so it behaves the same everywhere. AltGr (`ISO_Level3_Shift`) is still passed through, so AltGr layouts are unaffected, and GNOME only notifies without consuming the event, so accessibility features like Sticky Keys are unchanged. Note that at the moment of toggling the application may also receive the Alt press/release (e.g. menu-bar focus in some apps); if you don't want that, opt out by removing RightAlt from `toggle_keys`.

- **Super/Meta combination keys recognized (GTK/Qt immodules)**: The Super/Meta modifier mask bit was misaligned in the GTK3/4 and Qt5/6 input modules, so combination triggers like `key:Super+X` did not work through that path. Corrected to match the engine's X11 mask interpretation.

- **Word-mode syllable-downgrade guidance (WordGate)**: So the automatic downgrade to syllable units (in terminals, etc.) is not mistaken for "settings broke," the verdict is now logged (`[WordGate]`) and the always-syllable exception is spelled out in the settings app and CLI descriptions.

### 🪟 Windows support (experimental)

The Windows port advanced substantially this cycle. It is experimental support with on-device verification still in progress; the following is the scope of what was implemented.

- **Fully native TSF architecture**: All UI is consolidated into a single `unim_tsf.dll` and the separate helper executable (`unim-windows`) was removed. It includes the hanja/special-character/emoji popups (9×9 grid, bookmarks, paging), AutoTypeFix (forward, reverse, manual, suppression list, undo), settings, and the language bar. The shared core and Linux frontends were not touched.

- **Hangul composition in console/IMM32 apps (CUAS-compliant)**: Hangul composition is restored in console/IMM32 apps that use inline composition, such as WezTerm and Telegram.

- **32-bit app support (KakaoTalk, Hancom, etc.)**: To fix 32-bit apps not finding the 64-bit-only TIP, a 32-bit TSF TIP (`unim_tsf32.dll`) is now registered alongside. The pointless IMM32 `.ime` registration on Win11 was dropped.

- **Accessibility**: The composition and candidate windows are exposed via TSF UIA/UILess, and an option to suppress combination-key auto-repeat (`ignore_key_repeat`, for users with motor disabilities) plus screen-reader notification of Korean/English switches (NotifyWinEvent, optional beep) were added.

- **MSI distribution**: Upgraded to windows-rs 0.62.2 and tidied up the WiX 3.x MSI build chain.

### 🧹 Internal

- **unim-capi header sync**: The public C header (`unim.h`) is kept in sync with the Rust surface (added two `UnimInputResult` fields and the `POPUP_KEY_PERIOD` constant), and a build-time guard (`build.rs` + cbindgen) now warns automatically if the header drifts out of sync.
- **Keymap tools share one keyboard widget**: Keymap Studio and Typing Practice now share a single 5-row keyboard widget (three duplicate copies merged into one), and dead code (the old `KeyboardWidget`, `ProfileSidebar`, etc.) was removed.
- **Frontend cleanup**: Unused embedded popup widgets were removed from the GTK/Qt IM modules — all popups are now drawn solely by the popup-service — and the frontends no longer link against unim-capi.
- **Line-ending normalization**: A `.gitattributes` (eol=lf) and `.editorconfig` were added so line endings stay consistent across mixed Linux/Windows development.

---

## [0.3.0] 2026-05-19

This release brings chord (simultaneous keystroke) input, a major popup overhaul with mouse navigation and bookmarks, a unified settings dialog, and the Ahnmatae keyboard as UNIM's first built-in chord layout.

### ✨ Added

- **Ahnmatae (안마태 2003) keyboard built-in**: The keyboard designed by Matthew Y. Ahn in 2003 is now available out of the box. It is a three-beol chord layout — you press multiple jamo simultaneously to form one syllable. Select it in Settings → Keyboard.

- **Chord (simultaneous keystroke) input engine v4**: When using a chord-capable layout such as Ahnmatae, you can now press several jamo at the same time and UNIM assembles the syllable once the chord window closes. The chord window is set to 60 ms by default; 100–150 ms is recommended for newcomers. Settings → Keyboard → Chord Window (ms) lets you adjust it with a slider. Setting it to 0 disables chord mode entirely.
  - **Bidirectional jamo combine** (off by default): when enabled, cho/jung/jong combinations are tried in both key-press orders, so the syllable assembles correctly regardless of which finger lands first. This option also applies to sequential (non-chord) input.
  - Pressing Backspace during or after a chord removes one jamo at a time and recomposes the remaining syllable, just as expected in sequential three-beol.
  - Punctuation and symbols typed within the chord window are always treated as separate characters — they do not interfere with jamo combinations.

- **Hanja popup — click outside to close**: In GNOME, clicking anywhere outside the hanja, special-character, or emoji popup closes it immediately. The click is passed through to the window underneath, so it works as a normal click on the target application. Previously ESC was the only way to dismiss.

- **Hanja popup — mouse page navigation (◀/▶)**: All popups (hanja, special character, emoji) now have ◀ and ▶ buttons in the footer. Click them to move between pages, or right-click anywhere in the popup to go to the next page. These buttons are hidden when there is only one page.

- **Hanja candidate bookmarks (★/☆)**: You can now star a frequently used hanja candidate. The next time you convert the same syllable, starred candidates appear at the top of the list. Toggle the star with Space (in GTK/Qt) or right-click (in GNOME). When you remove a star, the candidate moves back to its alphabetical position and the cell flashes briefly (yellow, 140 ms) so you can see where it landed.

- **Hanja popup 9×9 expanded grid mode**: The hanja popup normally shows 9 candidates at a time. Press the ⊞ icon in the bottom-right corner to switch to an 81-candidate grid view, letting you scan a full page at a glance. Press ⊟ to return to compact mode.

- **Emoji popup — category tabs with keyboard shortcuts**: The emoji popup now has a vertical tab bar on the left side with 9 categories (Smileys, Animals, Food, …). Press A / S / D / F / … to jump directly to each category without touching the mouse.

- **AutoTypeFix learning blacklist**: When AutoTypeFix (automatic Korean↔English typo correction) makes a wrong correction, you can right-click the corrected word and select "Do not auto-correct" to add it to a personal blacklist. UNIM will not correct that word automatically again. You can review and manage blacklist entries in Settings → Suppression Words.

- **RPM package support**: `.rpm` packages for Fedora, openSUSE, and RHEL-based distributions are now provided. Note: the spec file is newly written and has not been fully validated on all target distributions — please report any packaging issues via [GitHub Issues](https://github.com/from104/unim/issues).

### 🔄 Changed

- **Single unified settings dialog (`unim-settings`)**: The separate GTK and Qt settings windows have been merged into one GTK4 + libadwaita dialog (`unim-settings`). All settings are in the same place regardless of your desktop environment.

- **Tray indicator is now a separate process (`unim-indicator`)**: The tray icon runs independently from the settings window. Closing the settings window no longer kills the tray icon, and vice versa.

- **Popup rendering is now a separate background service (`unim-popup-service`)**: The hanja/special-character/emoji popup is handled by a dedicated background process. It starts automatically the first time you trigger a hanja conversion. You do not need to start it manually.

- **Settings sliders instead of number spinners**: Numeric options in the settings dialog (such as Chord Window) are now adjusted with a slider with tick marks, making single-click adjustments easier with a mouse.

- **Settings dialog help text improved**: Every option in the settings dialog now has a clearer subtitle and tooltip explaining what the option does, when to turn it on, and what value to start with. Concrete examples and recommended values are included throughout.

- **Emoji popup is always available**: The dedicated "emoji popup enabled" toggle has been removed. The emoji popup is now always accessible — press the Hanja key while not composing Hangul to open it. Existing config files that contain `engine.emoji_popup` entries will have those entries silently ignored and removed on next save.

- **Chord window range and default updated**: The chord window now goes up to 200 ms (previously 100 ms), and the default is 60 ms (previously 50 ms). The extended range accommodates users who need more time between simultaneous keystrokes.

### 🐛 Fixed

- **XIM: next jamo was invisible for one frame after committing a syllable** (XTerm, WezTerm and other OVER-THE-SPOT XIM terminals): After you finish a Hangul syllable and start the next jamo, the preedit indicator now appears immediately. Previously it was missing for one keystroke. Note: a small number of rare ON-THE-SPOT XIM applications may still show this behavior — see Known Issues.

- **Compiler warnings eliminated**: Zero build warnings in this release. This does not directly affect user experience, but it reduces the likelihood of latent bugs in future releases.

### 🗑️ Removed

- **Qwerty Sebeolsik (`ko_3bul_qwerty`) removed from built-ins**: This layout has been dropped from the default keyboard list. If you were using it, your layout selection will not fall back automatically — go to Settings → Keyboard and choose a different layout. If you want to continue using it, copy the reference JSON from `docs/references/keymaps/ko_3bul_qwerty_v2.json` to `~/.config/unim/layouts/ko_3bul_qwerty.json` and it will appear as a user layout.

- **Qt settings dialog (`unim-gui-qt`) removed**: The Qt6-based alternative settings window has been removed. The GTK4 `unim-settings` dialog is now the single settings interface for all environments including KDE Plasma.

### ⚠️ Migration notes

- **Upgrading from 0.2.0**: Your settings file (`~/.config/unim/config.yaml`) and custom keyboard layouts (`~/.config/unim/layouts/*.json`) are preserved as-is. No manual migration is needed for these files.
- **`unim-gui-qt` users**: Run `apt remove unim-gui-qt && apt install unim-settings unim-popup-service` to switch to the new unified packages.
- **`ko_3bul_qwerty` users**: Your layout selection will not migrate automatically. After upgrading, open Settings → Keyboard and reselect your layout, or follow the workaround above to keep using the layout as a user profile.
- **Custom layout profiles from 0.1.x (v0 format)**: If you have hand-written layout JSON files in `~/.config/unim/layouts/` that were created for UNIM 0.1.x, they must be updated to the v1 format (add `"schema_version": 1` and a `combinations` block). See the migration guide in `docs/archive/plans/LAYOUT_PROFILE_V1.md`.

### 🚧 Known issues

- **KDE Plasma 5.x Wayland**: Hanja, special-character, and emoji popups do not appear. The required system library (`gtk4-layer-shell`) is not available in Ubuntu 24.04 standard repositories. Use an X11 session or switch to GNOME as a workaround.
- **KDE Plasma 6 Wayland / Sway / Hyprland / river and other standalone Wayland compositors**: This release has not been fully tested on these environments. Popup placement, IME focus handover, or input focus switching may have minor regressions. Please report issues via [GitHub Issues](https://github.com/from104/unim/issues).
- **Some rare ON-THE-SPOT XIM applications**: After committing a Hangul syllable, the preedit for the next jamo may be missing for one frame. XTerm, WezTerm, GTK, Qt, Wayland, and GNOME are not affected.

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
  - Spec: [`docs/archive/plans/LAYOUT_PROFILE_V1.md`](docs/archive/plans/LAYOUT_PROFILE_V1.md).
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
