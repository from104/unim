# Changelog

All notable changes to the UNIM (Universal Next-generation Input Method) project are recorded in this file.

The format is based on [Keep a Changelog] and this project follows [Semantic Versioning].

## [0.4.3] 2026-09-10

### Fixed

- Windows: the installer no longer launches the popup renderer during silent installs (`/qn`, winget), as required by winget automated validation; the install script still starts it itself

## [0.4.2] 2026-09-09

### Added

- Per-distribution release builds for six distributions (Ubuntu 24.04/26.04, Debian 13, Fedora 43/44, RHEL 10 family) with automatic detection and install in the install script (RHEL 10 family newly supported, requires the EPEL repository)
- Static and dynamic build-time checks that the GNOME extension still works on a new Shell version (`make check-compat`)
- Post-install verification that IM modules actually load and accept keystrokes on every per-distribution build (GTK3/4, Qt5/6, XIM, Windows MSI)
- Removed the causes of antivirus false positives on the Windows build — embedded file version info, a CI pre-scan with Defender, and a code-signing pipeline
- CI pre-builds and tests on the next Ubuntu LTS container ahead of its release

### Changed

- Moved `unim-cli config user-dict` and `config blacklist` to the top-level `unim-cli dict user-dict` and `dict blacklist` (old paths still work for now, with a notice)
- `unim-cli dict` add/remove now accept several words at once, with a confirmation prompt when removing more than one (`-y` to skip)

### Fixed

- Fixed Hangul input not working on GNOME Shell 50 (Ubuntu 26.04)
- Fixed in-progress Hangul being invisible in sandboxed apps (Flatpak, Snap)
- Fixed composition stalling in environments that fall back to GTK's `im-xim`, in-progress text now shown in a small window next to the caret instead
- Fixed the composed syllable being lost or the composition breaking the instant settings are saved
- Fixed the XIM server dying when an app closes, which used to block XIM input in every app afterward
- Windows: fixed composite keys such as `Ctrl`+`B` not working while auto-English switching is active
- Windows: added a one-click Explorer-restart fix for a missing tray menu after an update
- Windows: fixed character keys not reaching browser shortcuts in Chrome and others when no editable field has focus

### Known issues

- The modifier combination that would not hold its latch mid-composition — resolved from GNOME 50 (Ubuntu 26.04) on, still present on earlier versions
- Windows: the tray menu may not appear until you log in again right after a fresh install

## [0.4.1] 2026-08-16

### Fixed

- Republished the rpm packages — fixed a `MAKEFLAGS` handling bug in the jemalloc build script and a missing explicit `GuiPrivate` request on Qt 6.8+, verified with a build on Fedora 43
- Fixed Sticky Keys combinations (`Ctrl`→`A`, etc.) failing intermittently on GNOME Wayland, along with selection commands like `Shift`+`Home` (fix lives in the GNOME extension, log out and back in to apply)

### Known issues

- A modifier combination pressed mid-composition does not hold its latch, the composed text is committed first
- In-progress Hangul is invisible in sandboxed apps (Flatpak, Snap)
- Composition stalls in environments that go through GTK's `im-xim`
- Windows: composite keys such as `Ctrl`+`B` do not fire while auto-English switching is active
- Windows: the tray right-click menu does not appear after a fresh install or an update

## [0.4.0] 2026-08-10

Windows support on the same core as Linux, one-line install, and a shared settings window on both platforms

v0.4.0 tag published on 2026-07-19 withdrawn over an MSI release-gate failure (`guids.wxi` version mismatch, fixed in `65c66f8`); this entry is the valid v0.4.0

### Added

- One-line install (`curl … | bash` on Linux, `irm … | iex` on Windows) with automatic distribution detection, SHA256 verification, and version pinning via `UNIM_VERSION`
- First-run wizard that walks through setting the default input method on first login after install
- Settings app rewritten from GTK4 to Slint, giving Linux and Windows the same window
- Word-unit composition commit, selectable instead of syllable-unit (terminals, XIM, and chord layouts fall back to syllable units automatically)
- Independent toggle shortcuts for AutoTypeFix all/forward/reverse (all-toggle defaults to `Shift+F8`)
- Automatic password-field protection that switches to English mode and keeps keystrokes out of buffers, undo, and the learning dictionary
- Modifier-combination triggers such as `Ctrl+B` for auto-English switching, with the trigger key still passed through to the app
- Mode-switch beep on Korean/English toggle, can be turned off in settings
- Auto-repeat suppression for the Korean/English toggle key and Korean-mode character keys, off by default
- Keymap Studio and Typing Practice tools added (Linux)
- Distinct icons and cleaned-up app IDs for the indicator, settings, Keymap Studio, and Typing Practice, plus an icon embedded in the Windows executable

### Changed

- Settings app split in two — the new Slint app takes over the `unim-settings` name, the previous GTK4 app is renamed `unim-settings-gtk` and ships alongside it
- Reorganized into 11 deb packages — `unim-settings`, Keymap Studio, and Typing Practice split into new packages, the indicator, popup service, and legacy GTK dialog bundled into `unim-desktop`
- Keymap Studio UI redesigned — the sidebar + two-tab layout became a header dropdown (Language › Source › Layout) plus four tabs
- Layout list now enumerates registered profiles, so Ahnmatae and user layouts appear by name and the chord UI activates with them
- GNOME extension tray and panel icons replaced with a monochrome SVG set, with a dedicated disabled-state icon
- Invalid-shortcut save warnings extended from the AutoTypeFix toggle to the Korean/English toggle, hanja, and auto-English trigger keys, with save rejected when every entry is invalid

### Fixed

- Fixed Help opening in an IDE instead of the default browser (bypassing the text/html default handler)
- Fixed Right Alt Korean/English toggle not working, decision now made solely by the daemon
- Fixed `Super` combinations (e.g. `Super+X`) not being recognized in the GTK/Qt input modules
- Fixed every key being misread on pure Wayland (Sway, standalone Hyprland, etc.)
- Fixed password-field auto-protection not working on GNOME Wayland
- Fixed the next character showing up one keystroke late after a commit in XIM apps
- Fixed clicking elsewhere in the field mid-composition committing the text at the click position (GNOME Wayland, XIM, Qt)
- Fixed Enter mid-composition breaking the line before the character in XIM apps
- Fixed shortcut fields suggesting keys that do not exist (`ScrollLock`, `Hangul`), replaced with working specs (`F10`, `Korean`, `Hanja`)
- Logged and documented the reason word-mode falls back to syllable units in terminals

### Windows

- Consolidated the language bar, composition/candidate popups, and AutoTypeFix UI into a single `unim_tsf.dll`, removing the separate helper executable
- Restored Hangul composition in inline-composition apps such as WezTerm and Telegram (CUAS-compliant)
- Registered a 32-bit TSF (`unim_tsf32.dll`) for 32-bit apps such as KakaoTalk and Hancom, dropped the unnecessary IMM32 `.ime` registration
- Exposed composition and candidate windows via TSF UIA/UILess, added auto-repeat suppression and screen-reader mode-switch notification
- Tidied up the WiX 3.x MSI build chain (may reach the release a few minutes after the deb and rpm packages)

---

### Known issues

- This release carries no rpm packages, one-line install unavailable on Fedora and RHEL-family systems (fixed in [0.4.1](https://github.com/from104/unim/releases/tag/v0.4.1))

## [0.3.0] 2026-05-19

Chord (simultaneous keystroke) input, a mouse-driven popup overhaul with bookmarks, a unified settings dialog, and Ahnmatae as the first built-in chord layout

### Added

- Built-in one-hand chord three-beol keyboard Ahnmatae (Ahnmatae 2003)
- Chord (simultaneous keystroke) input engine v4, chord window (ms) adjustable via slider in Settings → Keyboard (default 60 ms, 0 disables it)
- Bidirectional jamo combine option added (off by default) — combines regardless of key-press order
- Click-outside-to-close for hanja/special-character/emoji popups on GNOME
- Mouse page navigation (◀/▶) buttons added to hanja/special-character/emoji popups
- Hanja candidate bookmarks (★/☆), starred candidates surface at the top of the list on conversion
- 9×9 expanded grid mode added to the hanja popup (toggled via ⊞/⊟ icon)
- Vertical category tabs and shortcuts (A/S/D/F, etc.) added to the emoji popup
- AutoTypeFix mis-correction learning blacklist added, register via right-click and manage in Settings → Suppression Words
- rpm package build support added for Fedora, openSUSE, and RHEL-based distributions

### Changed

- Merged the separate GTK and Qt settings windows into a single GTK4 + libadwaita settings window (`unim-settings`)
- Split the tray icon into its own process (`unim-indicator`), independent from the settings window
- Split hanja/special-character/emoji popup rendering into a separate background service (`unim-popup-service`), auto-starts on first use
- Changed numeric settings inputs to sliders with tick marks
- Added clearer descriptions and recommended values to every settings item
- Removed the emoji-popup enable/disable setting, now always available via the hanja key while not composing Hangul
- Changed the chord window's upper bound to 200 ms and its default to 60 ms

### Fixed

- Fixed the next jamo's preedit showing up one frame late right after committing a syllable in XIM environments (XTerm, WezTerm, and other OVER-THE-SPOT apps)

### Removed

- Removed the Qwerty Sebeolsik (`ko_3bul_qwerty`) built-in layout, a reference JSON can still be registered as a user layout if needed
- Removed the Qt settings window (`unim-gui-qt`), GTK4 `unim-settings` is now the single settings window

### Upgrade notes

- Upgrading from 0.2.0 keeps your settings file and user layouts as-is, no manual migration needed
- `unim-gui-qt` users need to switch to `unim-settings` and `unim-popup-service`
- `ko_3bul_qwerty` users need to reselect a layout in Settings → Keyboard (no automatic migration)
- User layout JSON created in 0.1.x (v0 format) needs converting to the v1 format

### Known issues

- Hanja/special-character/emoji popups do not appear on KDE Plasma 5.x Wayland (missing `gtk4-layer-shell` library, use an X11 session or GNOME instead)
- Insufficient verification on KDE Plasma 6 Wayland, Sway, Hyprland, river, and other standalone Wayland compositors, popup placement and focus handover may regress
- Some rare ON-THE-SPOT XIM apps may miss one frame of the next jamo's preedit right after committing a syllable

## [0.2.0] 2026-04-26

### Added

- Custom keyboard layout profiles (JSON) added, with live hot-reload
- Layout profile inheritance (`inherits`) and rule-set options added, rule sets individually toggleable in the settings screen
- `unim-cli config layout` subcommand added (`list`/`describe`/`validate`)
- Added the Qwerty Sebeolsik (`ko_3bul_qwerty`) built-in layout
- Automatic detection and learning suppression (blacklist) for AutoTypeFix mis-corrections, confirm/deactivate/manage in Settings → Suppression Words
- 9×9 expanded grid mode added to the hanja popup (toggled with the Period key, ⊞/⊟ icon)
- Hanja candidate bookmarks (☆/★) added, synced live across open popups
- Reverse AutoTypeFix user dictionary registration added (register selected text via shortcut, manage in the GUI)
- Auto Korean → English switching on configurable trigger characters (e.g. `:`, `/`) added
- Emoji popup (`Super`+`.`) added — category tabs, search, and recent-use favorites

### Changed

- Korean layout setting changed from a fixed enum to a free-form string, existing `config.yaml` values auto-normalized
- English layout setting changed the same way, existing values auto-normalized
- Relaxed the reverse-AutoTypeFix mis-correction suppression detection condition for more accurate results
- Fixed how the reverse-AutoTypeFix suppression list stores its key, so intended words are suppressed correctly
- Moved AutoTypeFix blacklist registration from rollback-detection time to retrigger time, reducing false positives
- Unified the config CLI under `unim-cli config`

### Fixed

- Fixed Space being dropped in English mode (e.g. in gedit)
- Fixed characters being committed twice on input-field focus-out (e.g. "늘" typed twice)
- Changed the AutoTypeFix suppression expiry unit from days to hours (1–12 hours)
- Added reverse-AutoTypeFix support for gedit and gnome-text-editor
- Fixed keystrokes locking up in GTK3/4 apps (including terminals) when composition ended
- Reimplemented XIM AutoTypeFix so multi-character corrections work correctly (a Chrome mid-composition edge case remains)

## [0.1.0] 2026-04-21 — Initial Release

First official release of UNIM (Universal Next-generation Input Method) — a Korean IME engine redesigned from scratch in Rust; components listed below

### Added — Engine Core

- Pure Rust Hangul engine added (2-bul / 3-bul 390 / 3-bul 391 composition and decomposition, zero UI/platform dependencies)
- DBus daemon architecture added (`unim-daemon` + `unim-dbus`, service name `org.atit.unim.InputMethod`)
- C API wrapper (`unim-capi` / `libunim_capi`) added for C/C++ frontends
- Unified CLI (`unim-cli`) added (Korean↔English converter plus a `config` subcommand)

### Added — Frontends

- GTK3/GTK4 input method modules added
- Qt5/Qt6 platform input context plugins added
- XIM frontend added (Over-The-Spot Preedit support, verified against the X11R7.6 XIM specification)
- Wayland frontend added (`input-method-v2` + `virtual-keyboard-v1`, foundational KDE Plasma support, hanja/special-character popup integration)
- GNOME Shell extension added (layout conversion shortcuts, terminal-aware paste mode, etc.)

### Added — GUI

- GTK4/libadwaita settings dialog (`unim-gui-gtk`) added (tray icon, hanja/special-character popups included)
- Qt6 alternative settings GUI (`unim-gui-qt`) added
- System im-config tool integration added

### Added — Features

- Korean layouts: 2-bul and 3-bul (390/391/no-shift) built-ins
- AutoTypeFix (automatic Korean↔English typo correction), forward and reverse, supported on XIM/GTK/Qt/GNOME
- Hanja conversion popup added (search, pagination, index-key navigation)
- Special-character/emoji search popup added
- Per-application input-mode auto-switching rules added

### Added — Packaging & Documentation

- Debian packaging split into 9 binary packages (`unim-common`, `unim-im-gtk`, `unim-im-qt`, `unim-xim`, `unim-wayland`, `unim-gui-gtk`, `unim-gui-qt`, `unim-gnome`, and the full-stack meta-package `unim`)
- Comprehensive documentation provided: 12 component `SPEC.md` files plus `IME_BEHAVIOR.md` and `POPUP_SPEC.md`

[Keep a Changelog]: https://keepachangelog.com/en/1.0.0/
[Semantic Versioning]: https://semver.org/spec/v2.0.0.html
