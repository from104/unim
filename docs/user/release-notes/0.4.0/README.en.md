# UNIM 0.4.0 Release Notes (English)

**Release date**: 2026-08-10
**Branch**: develop → main

> One-line summary: one-click install (`curl … | bash`), a first-run setup wizard, a Slint-based cross-platform settings app, new Keymap Studio and Typing Practice tools, word-unit input, and a large Windows (TSF) port.

> ⚠️ The previous v0.4.0 tag/release published on 2026-07-19 was retracted. This release notes document describes the valid v0.4.0.

---

## Package layout changes

- **Two settings apps**: the new **`unim-settings`** (Slint-based, shared between Linux and Windows) takes over as the primary settings app. The previous GTK4 settings window is renamed to the `unim-settings-gtk` binary and shipped inside the `unim-desktop` package for now, but it is hidden from the desktop menu (to be retired later).
- **New packages**: `unim-keymap-studio` (Keymap Studio), `unim-typing-practice` (Typing Practice).
- **`unim-desktop`**: bundles the tray indicator, popup service, and legacy GTK settings window together.
- Reorganized into **11 deb packages**: `unim-common`, `unim-im-gtk`, `unim-im-qt`, `unim-xim`, `unim-wayland`, `unim-desktop`, `unim-settings`, `unim-keymap-studio`, `unim-typing-practice`, `unim-gnome`, `unim` (meta).
- Upgrading via `apt`/`dnf` switches you over automatically. **Your settings (`~/.config/unim/`) are preserved.**

---

## Added

### 1. One-click install (`curl … | bash`)

```bash
curl -fsSL https://raw.githubusercontent.com/from104/unim/main/install.sh | bash
```

Verifies a Linux / amd64 / apt environment, downloads the 11 deb packages from the latest GitHub release, verifies them against `SHA256SUMS`, and installs with `apt-get`. Environment variables (`UNIM_VERSION`, `UNIM_BASE_URL`) let you pin a version or point at a mirror.

### 2. First-run setup wizard

The first time you log in after installation, the wizard runs automatically and sets UNIM as your default input method (via im-config, falling back to xinputrc). It re-appears at each login until completed; once completed it does not appear again.

> On a GNOME Wayland session, you still need to enable the extension once after re-login: run `gnome-extensions enable unim-gnome@from104.github.io`.

### 3. Slint settings app (`unim-settings`)

Rewritten as a cross-platform app that Linux and Windows share. Changes are saved to `config.yaml` immediately and the daemon is notified over DBus.

### 4. Korean/English switch beep

A short beep signals the mode change (880 Hz for Korean, 440 Hz for English). Off by default; can be turned on in settings.

### 5. Key auto-repeat suppression (accessibility)

The daemon can ignore OS auto-repeat (rapid re-fire) that happens when a key is held down — useful for users with motor disabilities (e.g. tremor). **Enforced identically on Windows and Linux.** Off by default.

### 6. Keymap Studio and Typing Practice (new GTK4 tools)

- **Keymap Studio (`unim-keymap-studio`)**: view and edit built-in/user layout key placement and cho/jung/jong combinations.
- **Typing Practice (`unim-typing-practice`)**: practice sample text with live WPM/CPM/accuracy/error-rate, plus a per-key error heatmap when done.

### 7. Auto-English switching — Ctrl/Alt/Super combination triggers

Modifier combinations such as `key:Ctrl+B` or `key:Super+Space` can now be used as auto-English triggers (e.g. for tmux/wmux). Works on GTK, Qt, XIM, and GNOME; Windows support is planned.

### 8. AutoTypeFix toggle shortcuts (three)

Toggle automatic typo correction — for all / forward-only (English → Korean) / reverse-only (Korean → English) — with a shortcut. **The all-toggle default is `Shift+F8`.** Behaves identically on Linux and Windows.

### 9. Automatic password-field protection

UNIM automatically switches to English mode in password fields, and keys typed there are not retained anywhere. Works on Wayland, GTK, Qt, and Windows (TSF); Windows IMM32 is best-effort, XIM is not supported.

### 10. Word-unit input (`commit_unit`)

You can set the commit unit to word instead of syllable. Terminals, XIM/ibus-family frontends, and chord layouts are automatically downgraded to syllable units to avoid misbehavior.

### 11. Distinct icons for four apps + reverse-DNS naming

The indicator, settings, Keymap Studio, and Typing Practice apps each ship a distinct icon and show up correctly in the GNOME Wayland taskbar and Overview.

---

## Windows support

The Windows port advanced substantially this cycle. It has been refined through continuous day-to-day use on the maintainer's own machine, though it has not been through the same breadth of machines and applications as Linux.

- **Fully native TSF architecture**: composition, popups (hanja/special-character/emoji), AutoTypeFix, settings, and the language bar are consolidated into a single `unim_tsf.dll`.
- **Console/IMM32 app Hangul composition restored**: works correctly in WezTerm, Telegram, etc. under the CUAS contract.
- **32-bit app support**: `unim_tsf32.dll` supports 32-bit apps such as KakaoTalk and Hancom. The pointless IMM32 `.ime` registration on Win11 was dropped.
- **Accessibility**: TSF UIA/UILess exposure, combination-key auto-repeat suppression, and screen-reader (NVDA/Narrator) notification of Korean/English switches.
- See the [UNIM (Windows) User Guide](../../UNIM-Windows-사용안내.md) (Korean) for details.

---

## Fixed

**Text that went wrong in front of you** — the symptoms you hit while composing come first.

- **Clicking elsewhere while composing committed the text at the click position**: In Chrome, Obsidian, and other apps, clicking elsewhere in the same input field mid-composition placed the in-progress syllable at the click position instead of where it was being typed. Fixed on the GNOME Wayland, XIM, and Qt paths.
- **In XIM apps, the character after a commit showed up one keystroke late**: In Obsidian and similar apps, once a syllable was committed the next jamo did not appear until you typed another one. Open since 0.3.0.
- **In XIM apps, pressing Enter while composing put the line break before the character**: The line broke first and the character landed below it, instead of committing the character and then breaking. Note that the Enter delivered afterwards does not carry modifiers, so `Shift+Enter` arrives as a plain Enter.
- **Password-field protection did nothing on GNOME Wayland**: The GNOME extension's content-purpose handling was an empty stub, so suppression was silently inert on the path GTK3/4 and Chrome all funnel through. It now also tracks a field's purpose changing while it stays focused (a "show password" toggle, for instance).

**Also**

- Help no longer opens in the wrong app (e.g. a VS Code-family IDE) instead of the browser.
- **Right Alt (RightAlt) Korean/English toggle now works everywhere** (previously filtered out by GTK3/4, Qt5/6, and the GNOME extension before reaching the daemon).
- Corrected the Super/Meta modifier mask in the GTK/Qt input modules.
- Replaced unrecognized shortcut examples (`ScrollLock`, `Hangul`, …) with ones that actually work (`F10`, `Korean`, `Hanja`, …), and corrected the stale note claiming modifier combinations were unsupported.
- Fixed an off-by-8 keycode misread on pure Wayland (non-GNOME) compositors.
- Accessibility presets (one-handed use / relaxed timing) now actually enforce key auto-repeat suppression on Linux too.
- When word-unit input falls back to syllable units in terminals and similar, it is now logged (`[WordGate]`) and spelled out in the settings descriptions, so it is not mistaken for broken settings.

---

## Known issues

- **The Windows edition has been exercised against a narrower range of applications than the Linux one.** If you run into problems with an uncommon app, please report them via [Troubleshooting](../../troubleshooting/README.md) or [GitHub Issues](https://github.com/from104/unim/issues).
- **GNOME Shell 49 has only been added to the supported range (45–49) in code**; a session smoke test on real hardware (e.g. Fedora 43) has not yet been performed.
- **If the daemon is restarted manually or crashes**, GTK/Qt/XIM/Wayland frontends do not automatically reconnect, so open apps may need to be restarted to recover Korean input. Routine `apt`/`dnf` upgrades no longer stop the daemon in this release.
- **On Plasma 6 (Qt6) Konsole, AutoTypeFix corrections may duplicate text.** This has not been verified on real hardware — if you see it, turn off AutoTypeFix for now.
- **On pure Wayland (non-GNOME) compositors** such as Sway/Hyprland, the hanja/special-character/emoji popup is experimental (`wayland-backend` + `libgtk4-layer-shell`) and not thoroughly tested. KDE Plasma 5.x Wayland is not supported at all (use an X11 session or GNOME instead).
- **Ubuntu 22.04 and Debian 12 are not supported** (system libraries are too old for the distributed packages). Use a source build instead.
- **The Windows MSI may be attached to the release later than the Linux packages** (a separate CI workflow, up to 45 minutes). If you try the Windows one-line install (`irm | iex`) in that window, it may fail because `SHA256SUMS-msi` isn't there yet — just retry shortly after.

---

## Further reading

- [User Guide](../../user-guide/README.md)
- [Troubleshooting](../../troubleshooting/README.md)
- [FAQ](../../faq/README.md)
- [UNIM (Windows) User Guide](../../UNIM-Windows-사용안내.md) (Korean only)
- [CHANGELOG](../../../../CHANGELOG.md)
