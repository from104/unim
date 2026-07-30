# Keyboard Shortcuts Guide

<!-- @platform:linux -->
**🐧 Linux**

UNIM shortcuts are captured by different actors depending on your desktop / compositor environment. This document explains how to enable them on each environment.
<!-- @endplatform -->
<!-- @platform:windows -->
**🪟 Windows**

On Windows, UNIM runs as a **TSF** (Text Services Framework — the standard Windows mechanism for plugging an input method into applications) input method. There is no step for registering a desktop-wide shortcut the way Linux compositors require. Instead UNIM intercepts keys **inside the application you are typing in**, or you drive it from the language bar in the taskbar. This document covers only the parts that **differ on Windows**.

Keys handled by the input engine itself — popup navigation, commit, cancel, backspace during composition — are **identical to Linux**. See [user manual §6 Key cheat sheet](../user-guide/README.md#6-key-cheat-sheet) for that list.
<!-- @endplatform -->

> Korean original: [`README-ko.md`](README-ko.md)

---

<!-- @platform:linux -->
## GNOME only — manual conversion shortcuts (on by default)

On GNOME Shell (`unim-gnome-extension`), the three shortcuts below are **on by default with no setup required.** No other desktop/compositor (KDE, Sway, Hyprland, etc.) has them — this is a GNOME-only feature, registered directly with the Shell via `Main.wm.addKeybinding` by the GNOME extension.

| Shortcut | Action | Example |
|--------|------|------|
| `Super+K` | Convert the focused word **English → Korean** and replace it | `gksrmf` → `한글` |
| `Shift+Super+K` | Convert the focused word **Korean → English** and replace it | `ㅗ디ㅣㅐ` → `hello` |
| `Super+E` | Read the selection and register it in the reverse (Korean→English) AutoTypeFix user dictionary | select `ㅎㅑㅅ` → registered as `git` |

These three are distinct from the automatic correction in [4.4 AutoTypeFix](../user-guide/README.md#44-autotypefix) — they are **manual conversions the user triggers directly**. If you use `Super` combos for something else, or hit a conflict, open the extension's preferences to rebind or disable them:

```bash
gnome-extensions prefs unim-gnome@from104.github.io
```

Rebind or clear `Super+K` / `Shift+Super+K` in the "Conversion shortcuts" group, and `Super+E` in the "Register user dictionary" group.

---

## Emoji popup shortcut (`Super+.`)

UNIM can pop up an emoji picker at the last active input location. The default shortcut is `Super+.` (Meta+`.`). However, **who captures the shortcut differs per environment**, so on some setups you must register it yourself.

### Per-environment behavior

| Environment | Auto? | How to register |
|-------------|-------|-----------------|
| X11 / XIM | Auto | The unim daemon receives the shortcut via X server redirect |
| Wayland + GNOME | Auto | UNIM GNOME extension registers via `Main.wm.addKeybinding` |
| Wayland + KDE Plasma | Manual | KCM Custom Shortcuts |
| Wayland + Hyprland | Manual | `hyprland.conf` |
| Wayland + Sway | Manual | `sway/config` |
| Wayland + Wayfire | Manual | `wayfire.ini` |
| Wayland + other compositors | Manual | Compositor's shortcut tool |

### Why manual registration is needed

Wayland compositors (KDE / Hyprland / Sway / Wayfire, etc.) intercept modifier-based combos like `Super+...` in their own shortcut subsystem before any application sees them. If the compositor consumes the keystroke, the input method (IME) never receives the event.

On GNOME, the UNIM GNOME extension hooks into Shell's shortcut slot, so it works automatically. Other compositors do not have such an extension, so you need to register **`unim-cli trigger emoji_popup`** as a shortcut command in the compositor's own shortcut system.

### Common command

```bash
unim-cli trigger emoji_popup
```

Internally this calls the daemon's DBus interface `org.atit.unim.InputMethod` via the `TriggerAction` RPC. When the daemon receives the signal, it shows the emoji popup at the most recently active input context (the text widget you used last).

Future actions (e.g. `hanja_popup`) will follow the same `unim-cli trigger <action>` pattern.

---

## Per-environment registration

### KDE Plasma 6 (Wayland)

1. Open **System Settings** → **Shortcuts** → **Custom Shortcuts**
2. Bottom-left **Edit** → **New** → **Global Shortcut** → **Command/URL**
3. Name: `Trigger UNIM emoji popup` (any name)
4. **Trigger** tab → press `Meta+.`
5. **Action** tab → **Command/URL**: `unim-cli trigger emoji_popup`
6. **Apply**

> Plasma 5 works the same way (paths: `System Settings → Shortcuts → Custom Shortcuts`).

### Hyprland

Add to `~/.config/hypr/hyprland.conf`:

```ini
bind = SUPER, period, exec, unim-cli trigger emoji_popup
```

The config auto-reloads; force it with `hyprctl reload`.

### Sway

Add to `~/.config/sway/config`:

```
bindsym Mod4+period exec unim-cli trigger emoji_popup
```

`Mod4` is the Super (Windows) key. Apply with `swaymsg reload`.

### Wayfire

Add the following to the `[command]` section of `~/.config/wayfire.ini`:

```ini
[command]
binding_emoji = <super> KEY_DOT
command_emoji = unim-cli trigger emoji_popup
```

The config auto-reloads on save.

### GNOME (fallback when not using the extension)

If you have the UNIM GNOME extension installed and enabled, no setup is needed. Otherwise, GNOME's built-in shortcut system can do the same:

1. **Settings** → **Keyboard** → **View and Customize Shortcuts**
2. **Custom Shortcuts** → **Add Shortcut**
3. **Name**: `UNIM Emoji Popup`
4. **Command**: `unim-cli trigger emoji_popup`
5. **Set Shortcut**: press `Super+.` → **Add**

> Some `Super` combos are reserved by GNOME. If there is a conflict, try a different combo (e.g. `Ctrl+Alt+E`).

### X11 + arbitrary WM (xbindkeys)

On X11/XIM, the daemon usually matches the shortcut automatically and **no manual registration is required**. However, in environments where the daemon does not receive the key (some game modes, certain screen-switching tools), you can supplement with `xbindkeys`.

Add to `~/.xbindkeysrc`:

```
"unim-cli trigger emoji_popup"
  Mod4 + period
```

Apply:

```bash
xbindkeys -p   # stop existing instance
xbindkeys      # start again
```

---

## Verifying it works

After registering the shortcut, watch the daemon log:

```bash
journalctl --user -f | grep unim
```

Or if you run it as a systemd service:

```bash
journalctl --user -u unim-daemon -f
```

A successful press should produce something like:

```
[DBus] TriggerAction(emoji_popup) received
```

If you don't see that message:

- Check the CLI path with `which unim-cli`
- Run `unim-cli trigger emoji_popup` directly in a terminal and verify there are no errors
- Confirm the daemon is running: `systemctl --user status unim-daemon`
- Check for shortcut conflicts in your compositor (another app may already own the combo)

---

## In-app shortcuts for the layout tools

The `Super+.` above is a desktop-global shortcut, but the two GTK4 layout tools that ship with
UNIM have their own shortcuts that work **only inside their windows** (no compositor registration
needed). For usage details see
[user manual §5.6](../user-guide/README.md#56-keyboard-layout-tools-keymap-studio--typing-practice).

### unim-keymap-studio (view / edit layouts)

| Key | Action |
| --- | ------ |
| F1 | Help |
| Ctrl + N | New layout |
| Ctrl + D | Duplicate current layout |
| Ctrl + S | Save (user layouts) |
| Ctrl + Shift + S | Save As |
| Ctrl + E | Export |
| Ctrl + I | Import |
| Ctrl + 1 / 2 / 3 / 4 | Switch tab (Basic / Keymap / Combos / Extended) |

### unim-typing-practice (typing practice)

| Key | Action |
| --- | ------ |
| F1 | Help |
| Ctrl + R | Restart |
| Ctrl + Shift + C | Copy results |
| Ctrl + 1 | Practice view |
| Ctrl + 2 | Results view |
| Ctrl + O | Import material from file |
| Ctrl + Shift + V | Import material from clipboard |

---

## Notes

- On X11/XIM, the X server redirects modifier-based combos to the IM, so it works automatically.
- On Wayland there is no standardized redirect mechanism, so the compositor must cooperate.
- Future actions follow the same `unim-cli trigger <action>` pattern and can be registered the same way.

Related docs:
- [`unim-cli/SPEC.md`](../../../unim-cli/SPEC.md) — CLI specification
- [`unim-daemon/SPEC.md`](../../../unim-daemon/SPEC.md) — daemon DBus interface
- [`unim-gnome-extension/SPEC.md`](../../../unim-gnome-extension/SPEC.md) — GNOME extension shortcut handling
<!-- @endplatform -->
<!-- @platform:windows -->
## Windows — shortcuts UNIM intercepts directly

The Windows build of UNIM runs without a background daemon: it is a single **TSF TIP** (Text Input Processor — an input method module built to the TSF spec). There is nothing to register with a desktop shortcut system; UNIM intercepts the key below **inside the application you are typing in**. No setup on your part.

| Shortcut | Action | Example |
| -------- | ------ | ------- |
| `Ctrl + Shift + Space` | Convert the word before the cursor (or the selection) to **the opposite of Korean/English** and replace it | `gksrmf` → `한글` |

If you have a selection, that selection is the target; otherwise the word before the cursor is. UNIM **decides the direction** (English→Korean or Korean→English) on its own, so there is no separate key per direction. Think of it as the two GNOME keys `Super+K` / `Shift+Super+K` merged into one.

This is separate from the **automatic** correction in [4.4 AutoTypeFix](../user-guide/README.md#44-autotypefix) — it is a **manual conversion you trigger yourself**. Use it to fix a word that automatic correction left alone.

> This key only works inside an app where UNIM is the active input method. If another input method is selected, the key goes to that one first.

---

## Windows — using the language bar (tray indicator)

The Korean/English indicator next to the taskbar clock (`한` / `A`) is UNIM's language bar. It is not a keyboard shortcut, but it is the always-available route to settings and help, so it belongs here.

| Action | Result |
| ------ | ------ |
| **Left click** | Toggle Korean/English |
| **Right click** | 5-item menu: `Switch Korean/English` · `Set as Default Input Method` · `Settings` · `Help` · `About` |

> ⚠️ Left click **toggles Korean/English — it does not open settings**. To open the settings window you must **right click → `Settings`**.

The menu's **`Help`** entry opens this very manual (`unim-help-ko.html` / `unim-help-en.html`) in your default browser. If you press it from a low-integrity app and nothing opens, the settings window appears instead; press its **[Help]** button once more to reach the manual.

---

## Windows — opening the settings window

| Method | Steps |
| ------ | ----- |
| **A. Language bar** (recommended) | Right click `한` / `A` next to the clock → **`Settings`** |
| **B. Start menu** | Start menu → **UNIM** folder → **`UNIM Settings`** |
| **C. Run directly** | `C:\Program Files\UNIM\unim-settings.exe` |

`unim-cli`, the command-line tool used to rebind keys on Linux, is **not included in the Windows build.** Every setting, shortcut changes included, is made in the settings window above.

---

## Windows — there is no global emoji popup shortcut

The Linux build has a desktop-wide shortcut (`Super+.`) that summons the emoji popup from anywhere. **The Windows build has no such route.** That feature is built from three pieces working together — the daemon, DBus (the Linux inter-process communication standard), and `unim-cli` — and the Windows installer ships none of them.

The keys that open the emoji, Hanja and special-character popups on Windows are **handled by the engine, so they are exactly the same as on Linux.** For those keys, and for navigating / committing / cancelling inside a popup, see [user manual §6 Key cheat sheet](../user-guide/README.md#6-key-cheat-sheet).

---

## Windows — shortcut conflicts

On Windows, both the OS itself and other input methods can claim a key before UNIM sees it.

| Situation | Symptom | What to do |
| --------- | ------- | ---------- |
| You press `Win` + `.` | The **built-in Windows emoji panel** appears, not UNIM's | This is expected. Use the popup keys from the cheat sheet above |
| You press `Alt` + `Shift` or `Win` + `Space` | The **OS switches input language** (UNIM ↔ another input method) | This is a different thing from toggling Korean/English inside UNIM. Don't confuse the two |
| Another Korean input method is also installed | The Korean/English and Hanja keys go to that input method instead of UNIM | Right click the language bar → **`Set as Default Input Method`** |

Apart from the keys it uses itself, UNIM passes `Ctrl` / `Alt` / `Win` combos straight through to the application. So it does not conflict with an app's own shortcuts (`Ctrl+C`, `Ctrl+S`, and so on).

---

## Windows — notes

- The Windows installer does **not** include the **layout tools** (`unim-keymap-studio` · `unim-typing-practice`). Those are Linux-only.
- 32-bit applications such as KakaoTalk and Hancom are served by the 32-bit module (`unim_tsf32.dll`) installed alongside; key behavior is the same as the 64-bit one.
- If no shortcut responds at all, UNIM may simply not be active in that application. Check the [troubleshooting guide](../troubleshooting/README.md) first.

Related docs:
- [User manual](../user-guide/README.md) — base key table and popup usage
- [Troubleshooting](../troubleshooting/README.md) — when keys don't respond
<!-- @endplatform -->
