# Changelog

All notable changes to the UNIM (Universal Next-generation Input Method) project are recorded in this file.

The format is based on [Keep a Changelog] and this project follows [Semantic Versioning].

## [Unreleased]

### Added

- **Each release now ships per-distribution builds.** Until now a single deb (built on Ubuntu 24.04) and a single rpm (built on Fedora 43) were published, so other distributions either failed to install due to system library differences or risked broken input in Qt apps. Every release is now built separately in six distribution containers (Ubuntu 24.04/26.04, Debian 13, Fedora 43/44, RHEL 10 family) with per-distro assets attached, and the install script automatically picks the build matching the detected distribution. The RHEL 10 family (Rocky, Alma) becomes a supported target with this change — the EPEL repository is required.
  - Derivative distributions (Mint, etc.) map to their upstream base; non-LTS Ubuntu (25.x) maps to the nearest lower LTS build.
- **Build-time detection of extension breakage on a new Shell.** Extension JS gets no compile check, so when GNOME Shell drops an API the build and the tests stay green and the failure surfaces only on a user's machine. `make check-compat` closes that gap with two overlapping checks: a static one that matches every GI symbol the extension uses against this Shell's introspection, and a dynamic one that loads the extension into a headless Shell and reads the log to confirm activation runs to completion. Neither requires a logout, and both skip themselves on machines without GNOME.
- **CI now builds on the next LTS ahead of time.** A `build-next-lts` job builds and tests inside the next Ubuntu LTS container. Using a container rather than a runner image means the check works before GitHub ships a runner for that release. The distro and desktop compatibility rules are written up in `docs/dev/linux/os-compatibility.md`.

### Fixed

- **Hangul input works on GNOME Shell 50 (Ubuntu 26.04).** Two things blocked it. The extension's `metadata.json` declared support only through Shell 49, so Shell 50 refused to load it as `OUT OF DATE`; and even once loaded, GNOME 50 removed the global `Meta.is_wayland_compositor()`, so enabling the extension ended in an exception. The second failure was the nastier one — the extension still showed as enabled while its IME registration had been rolled back entirely, so not a single character went through. Detection now tries `MetaContext.get_wayland_compositor()` first and falls back to the old function and then the session type, so one code path covers Shell 45 through 50.
- **In-progress Hangul is now visible in sandboxed apps (Flatpak, Snap).** Modern GTK IBus clients (libibus 1.5.20 and later) no longer listen to the old three-argument preedit signal at all — they subscribe only to the four-argument `UpdatePreeditTextWithMode`, while we emitted only the old one. That is why committing worked but the in-progress text never appeared. We now honor the `ClientCommitPreedit` property a client sets about itself and emit the new signal to it; clients that do not set the property keep receiving the old signal as before.
- **Composition no longer stalls in environments that fall back to GTK's `im-xim`.** The cause was not a transport defect but a hole in the protocol itself: XIM has no sequence numbers to pair requests with replies, so when the server's preedit callbacks meet GTK's nested waits, the replies race each other and the app freezes solid (first character visible, silence from the second key on). We verified by measurement that no server-side ordering can avoid this, and stopped advertising the ON-THE-SPOT style that triggers it. In these apps the in-progress text now appears in a small window next to the caret — instead of the inline underline, but it does not stall. Apps that explicitly request ON-THE-SPOT keep the old behavior.
- **Windows: composite keys such as `Ctrl`+`B` now fire while auto-English switching is on.** The key is only inspected for the switch trigger and passed to the application untouched. The check is idempotent, so apps like Word that probe the same key speculatively several times cannot toggle the mode twice.
- **Windows: when the tray menu is missing after an update, you can revive it on the spot.** The cause is that an already-running Explorer does not re-read the new input method, so the what's-new window now says a re-login is needed and offers a one-click optional Explorer restart — it never restarts anything by force. A startup banner (version, build time, DLL path) and a diagnostic logging guide were added so the cause can be told from logs alone.

### Known issues

- **The modifier combination that would not hold its latch mid-composition turned out to be a GNOME defect.** Upstream mutter reworked its Sticky Keys state handling, and the problem is **resolved from GNOME 50 (Ubuntu 26.04) on** — confirmed in real use. On earlier GNOME (Ubuntu 24.04 and the like) it remains: finish the syllable and the combination behaves normally.
- Windows: right after a fresh install, the tray menu may not appear until you log in again, because the already-running Explorer does not read the newly installed input method.

## [0.4.1] 2026-08-16

### Fixed

- **The rpm packages are back.** 0.4.0 shipped only .deb and the Windows MSI, so one-line installation was simply unavailable on Fedora and RHEL-family systems. The build was broken in two places — the `--` separator that `rpmbuild` puts into `MAKEFLAGS` was swallowed by the jemalloc build script, and `Qt6::GuiPrivate` must be requested explicitly on Qt 6.8 and later, which we did not do. The .deb side uses Qt 6.4, where it comes along for free, so this never surfaced there. Verified with a real build on Fedora 43.
- **Sticky Keys combinations no longer fail intermittently (GNOME Wayland).** Inside text fields, combinations such as `Ctrl`→`A` or `Alt`→`F` were sometimes delivered without the modifier, so a plain character was typed instead. When the input method intercepted a key and handed it back, GNOME skipped the point where it re-applies the Sticky Keys latch, and the returned key arrived a beat later — by which time the latch had already been released. Keys the input method will not consume are now passed straight through, so the latch survives. Selection commands like `Shift`+`Home` and `Shift`+arrow had the same defect and are fixed as well. For people who can only press one key at a time, this is the difference between being able to use combination keys and not.
  - The fix lives in the GNOME Shell extension, and GNOME does not reload extension code during a session. Log out and back in after upgrading.

### Known issues

- **A modifier combination pressed in the middle of composing does not hold the latch.** Pressing `Shift`→`Home` while a Hangul syllable is still being composed is the case in point. The composed text is committed to the application first, because characters arriving out of order is the worse failure. Finish the syllable and the combination behaves normally.
- Hangul is invisible while being composed in sandboxed apps (Flatpak, Snap). Committing and continuous typing were restored in 0.4.0 over the IBus path, but the in-progress text still does not appear.
- Composition stalls in environments that go through GTK's `im-xim`.
- Windows: composite keys such as `Ctrl`+`B` do not fire while auto-English switching is active, and the tray context menu does not appear after a fresh install or an update.

## [0.4.0] 2026-08-10

The release where an input method that only ran on Linux started running on Windows off the same core, installation became a single line, and both platforms got the same settings window.

> The v0.4.0 tag published on 2026-07-19 was withdrawn — the MSI release gate was broken by a `guids.wxi` version mismatch (fixed in `65c66f8`). This entry is the valid v0.4.0.

### Added

- **One-line install**: `curl … | bash` on Linux, `irm … | iex` on Windows. The script detects your distribution, downloads the deb or rpm packages, verifies them against SHA256, and installs — and if verification fails it installs nothing and stops. `UNIM_VERSION` pins a specific version.

- **First-run wizard**: Runs automatically the first time you log in after installing and walks you through setting UNIM as the default input method. It does not come back once finished.

- **Settings app rewritten (`unim-settings`)**: Moved from GTK4 to Slint, so Linux and Windows share one settings window.

- **Word-unit input**: The commit unit can be a word instead of a syllable. Terminals, XIM, and chord layouts fall back to syllable units automatically to avoid misbehavior.

- **AutoTypeFix toggle shortcuts**: Toggle all / forward / reverse independently. The all-toggle defaults to `Shift+F8`; the per-direction toggles are left empty for those who want them. Modifier combinations such as `Shift+F8` and `Ctrl+Left` work, and combinations you did not configure pass straight through to the application.

- **Automatic password-field protection**: Entering a password field switches to English mode and keeps the keys you type out of buffers, undo, and the learning dictionary. Leaving the field restores the previous state. It relies on the app reporting "this field is a password", so apps that do not report it are not detected (see user manual 4.4).

- **Modifier combinations for auto-English switching**: Triggers like `key:Ctrl+B` are now accepted. For environments that need a combo key, such as the tmux prefix, the key is passed through to the application as the mode switches.

- **Mode-switch beep**: A short beep when the Korean/English mode changes (880 Hz Korean, 440 Hz English). No external library, no added input latency. Can be turned off in settings.

- **Key auto-repeat suppression**: Ignores the auto-repeat from holding a key down. It covers the Korean/English toggle key and character keys in Korean mode, leaving Backspace and arrow keys alone. Meant for users who cannot release keys quickly; **off by default**.

- **Keymap Studio and Typing Practice**: A tool for viewing and editing layouts, and one for measuring speed and accuracy per layout (Linux).

- **Distinct app icons**: The indicator, settings, Keymap Studio, and Typing Practice each have their own icon, and app IDs follow reverse-DNS naming so they display correctly in the GNOME taskbar and Overview. The settings app is Slint-based and has no equivalent of GTK's `application_id`, so its window carried no app_id; it is now set explicitly on the winit backend. On Windows the executable embeds an icon resource, so the taskbar, Explorer, and Start menu show the real icon too.

### Changed

- **The settings app is now two apps**: The new Slint app takes over the `unim-settings` name and the previous GTK4 app is renamed `unim-settings-gtk`, shipped alongside for now (to be retired later). Only the Slint app appears in the desktop menu, and every component's "Open settings" points at it.

- **Reorganized into 11 deb packages**: The Slint settings app becomes its own `unim-settings` package, and Keymap Studio and Typing Practice are added as new packages. The indicator, popup service, and legacy GTK dialog are bundled into `unim-desktop`.

- **Keymap Studio redesigned**: The left sidebar + two tabs became a three-step header dropdown (Language › Source › Layout) plus four tabs. Built-in layouts allow only "Save As"; your own layouts save in place.

- **Layout list enumerated from registered profiles**: Instead of a fixed set of four, the actually registered profiles are enumerated — so Ahnmatae and user layouts appear by name and the chord-related UI turns on with them.

- **GNOME extension icons refreshed**: Tray and panel icons replaced with a monochrome SVG set, with a separate icon for the disabled state.

- **Invalid-shortcut warnings extended to every key field**: The save-time validation that only covered AutoTypeFix toggles now also covers the Korean/English toggle key, the Hanja key, and auto-English triggers. Specs the engine cannot parse are reported in the settings app, the CLI, and the daemon log. Partially invalid lists save with a warning; **a list where every entry is invalid is rejected** — otherwise the engine would drop them all and leave no way to switch languages. Verdicts come from one place in the engine, so Linux and Windows apply the same rules.

### Fixed

- **Help opened in an IDE instead of the browser**: The Help entries followed the text/html default handler, so on systems where a VS Code-family editor had claimed text/html the manual opened there. It now prefers your default web browser.

- **Right Alt Korean/English toggle did not work**: In the engine the modifier check ran before the toggle check, and GTK, Qt, and the GNOME extension filtered Right Alt themselves so it never reached the daemon. The toggle decision is now made solely by the daemon and behaves the same everywhere. AltGr layouts are unaffected. Note that the application may also receive the Alt press at the moment of toggling; remove it from `toggle_keys` if you don't want that.

- **Super/Meta combinations unrecognized in the GTK/Qt input modules**: A misaligned modifier mask bit left triggers like `key:Super+X` dead on that path.

- **Every key misread on pure Wayland**: On Sway, standalone Hyprland, and similar, the frontend sent X11-style keycodes (raw evdev + 8) while the daemon expected raw evdev, so every key lookup was off by 8. GNOME sessions were unaffected, as they use the extension path.

- **Password-field suppression did nothing on GNOME Wayland**: The GNOME extension's content-purpose handling was an empty stub, so suppression was silently inert on GNOME Wayland — where GTK3/4 and Chrome all funnel through that path. It is now wired up and tracks a field's purpose changing while it stays focused (a "show password" toggle, for instance).

- **In XIM apps, the character after a commit showed up one keystroke late**: In apps that go through XIM (Obsidian and others), once a syllable was committed the next jamo you typed did not appear until you typed another one. Open since 0.3.0; it now appears as you type. (Terminals and other OVER-THE-SPOT clients were already correct and stay that way.)

- **Clicking elsewhere while composing committed the text at the click position**: In Chrome, Obsidian, and other apps, clicking elsewhere in the same input field mid-composition placed the in-progress syllable at the click position instead of where it was being typed. Fixed on the GNOME Wayland, XIM, and Qt paths.

- **In XIM apps, pressing Enter while composing put the line break before the character**: In Obsidian and other apps that go through XIM, composing a syllable and pressing Enter broke the line first and left the character below it, instead of committing the character and then breaking. The GTK and Qt paths were already correct. Note that the Enter delivered afterwards does not carry modifiers, so `Shift+Enter` arrives as a plain Enter.

- **Shortcut fields suggested keys that do not exist**: Following the hints and entering `ScrollLock` or `Hangul` left the shortcut silently dead. The examples now use specs that actually work (`F10`, `Korean`, `Hanja`), and the stale "modifier combinations are not supported" note was corrected.

- **Word-mode syllable-downgrade guidance**: So the automatic fallback to syllable units in terminals is not mistaken for broken settings, it is now logged (`[WordGate]`) and spelled out in the settings descriptions.

### Windows

- **A single TSF DLL**: The language bar, composition and candidate popups (hanja, special characters, emoji), and the AutoTypeFix UI are consolidated into `unim_tsf.dll`, replacing the previous separate helper executable. The shared core and the Linux frontends were not touched.

- **Hangul composition in console/IMM32 apps**: Restored for apps that use inline composition, such as WezTerm and Telegram (CUAS-compliant).

- **32-bit apps**: A 32-bit TSF TIP (`unim_tsf32.dll`) is registered alongside, so KakaoTalk, Hancom, and similar work. The pointless IMM32 `.ime` registration on Win11 was dropped.

- **Accessibility**: Composition and candidate windows are exposed via TSF UIA/UILess, and auto-repeat suppression plus screen-reader notification of mode switches were added.

- **MSI distribution**: The WiX 3.x build chain was tidied up. The MSI is built by a separate CI workflow, so it may reach the release a few minutes after the deb and rpm packages — if the installer reports a missing checksum, wait a moment and retry.

### Internal

- The public C header (`unim.h`) is kept in sync with the Rust surface, with a build-time guard that warns on drift.
- Keymap Studio and Typing Practice now share a single keyboard widget (three duplicate copies merged into one).
- Unused embedded popup widgets were removed from the GTK/Qt IM modules; popups are drawn solely by the popup service.
- `.gitattributes` (eol=lf) and `.editorconfig` normalize line endings.

---

### Known issues

- **This release carries no rpm packages.** The build was broken, so only the 11 .deb packages and the Windows MSI were published, and one-line installation is unavailable on Fedora and RHEL-family systems. Fixed in [0.4.1](https://github.com/from104/unim/releases/tag/v0.4.1).

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
