# Keyboard Shortcuts Guide

UNIM shortcuts are captured by different actors depending on your desktop / compositor environment. This document explains how to enable them on each environment.

> Korean original: [`README-ko.md`](README-ko.md)

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
