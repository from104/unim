# UNIM Troubleshooting (English)

> UNIM 0.2.0 — organized as Symptom → first diagnosis → second-level command → fix.
> Covers 14 commonly seen symptoms, from "Korean never types" to "broken in one specific app".

Every diagnosis starts with two questions: **is the daemon alive?** and **what does the log say?**

```bash
# (1) Is the daemon alive?
systemctl --user status unim-daemon
# Or PID check
unim-daemon --check && echo "RUNNING" || echo "STOPPED"

# (2) Turn on debug logging and reproduce
UNIM_DEVELOP=1 systemctl --user restart unim-daemon
> ~/.unim-errors.log    # truncate
# … reproduce the bug …
tail -f ~/.unim-errors.log
```

> `UNIM_DEVELOP=1` aggregates Engine/DBus/Frontend/Extension logs into a single file (`~/.unim-errors.log`). The default is OFF so the log file does not grow unbounded.

---

## 1. "Korean never types" — fresh install

### First diagnosis

```bash
echo $GTK_IM_MODULE      # should be: unim
echo $QT_IM_MODULE       # should be: unim
echo $XMODIFIERS         # should be: @im=unim
unim-daemon --check && echo OK || echo MISSING
```

### Cause-by-cause fix

| Symptom | Cause | Fix |
|------|------|------|
| Env vars empty | im-config not configured | `im-config -n unim`, then log out/in |
| `unim-daemon --check` → MISSING | Systemd unit not enabled | `systemctl --user enable --now unim-daemon` |
| Visible in shell, not in GUI apps | Display manager did not load env | export in `~/.xprofile` or `/etc/environment` |
| GNOME+Wayland | Env-var path is dead | Use `gnome-extensions enable unim-gnome@from104.github.io` instead |

---

## 2. "Broken only in GTK apps (GNOME Text Editor, etc.)"

### Diagnosis

```bash
ls /usr/lib/x86_64-linux-gnu/gtk-3.0/3.0.0/immodules/im-unim.so 2>/dev/null
ls /usr/lib/x86_64-linux-gnu/gtk-4.0/4.0.0/immodules/libim-unim.so 2>/dev/null

sudo gtk-query-immodules-3.0 --update-cache
sudo gtk-query-immodules-4.0 --update-cache
```

### Fix

- File missing → reinstall `unim-im-gtk` or rerun `sudo make install PREFIX=/usr`.
- File present but inert → refresh the module cache, restart GTK apps.
- Note: GTK4's filename is `libim-unim.so`, GTK3's is `im-unim.so` (different `lib` prefix).

> Tip: `GTK_IM_MODULE_FILE=/usr/lib/.../immodules.cache GTK_IM_MODULE=unim gnome-text-editor` shows module-load errors on stderr.

---

## 3. "Broken only in Qt apps (Kate, Krita)"

### Diagnosis

```bash
ls /usr/lib/x86_64-linux-gnu/qt5/plugins/platforminputcontexts/libunimplatforminputcontextplugin.so
ls /usr/lib/x86_64-linux-gnu/qt6/plugins/platforminputcontexts/libunimplatforminputcontextplugin.so
QT_DEBUG_PLUGINS=1 kate 2>&1 | grep -i unim
```

### Fix

- Plugin missing → reinstall `unim-im-qt`.
- `QT_DEBUG_PLUGINS=1` shows `Cannot load library` → check dependencies with `ldd <plugin>.so`.
- Plasma 6 prefers Qt6, so `QT_IM_MODULE=unim` is enough.

---

## 4. "GNOME extension not in the menu"

### Diagnosis

```bash
gnome-extensions list | grep unim
gnome-extensions info unim-gnome@from104.github.io
journalctl --user -u gnome-shell -b | grep -i unim
```

### Fix

- Make sure `~/.local/share/gnome-shell/extensions/unim-gnome@from104.github.io/` exists.
- If not, `make dev-extension` (source) or install the `unim-gnome` package.
- Enable: `gnome-extensions enable unim-gnome@from104.github.io` → Alt+F2 → `r` (X11) or log out/in (Wayland).
- Verify Shell version compatibility in `metadata.json`'s `shell-version` array.

---

## 5. "Hanja popup never appears"

### Diagnosis

```bash
unim-cli config get popup_mode

# Are DBus signals being emitted?
busctl --user monitor org.atit.unim.InputMethod
# Then type Korean and press Hanja → ShowHanjaPopup signal should appear
```

### Fix

| Environment | Recommended `popup_mode` | Note |
|------|----------------|------|
| GNOME+Wayland | `Standalone` | Extension paints |
| KDE+Wayland | `Standalone` | unim-gui-gtk paints |
| X11 (any DE) | `Embedded` or `Standalone` | Embedded means the IM module renders directly |
| Pure Wayland (Sway, etc.) | `Standalone` | Open issue — see [popup spec §8.4](../specs/POPUP_SPEC.md) |

```bash
unim-cli config set popup_mode Standalone
systemctl --user restart unim-daemon
```

> **DBus dead?**: `busctl --user list | grep atit` — if empty, the daemon failed to register. Check `journalctl --user -u unim-daemon -n 100`.

---

## 6. "Special-character popup never appears"

Same code path as Hanja popup, so the cause is similar.

```bash
unim-cli config get current_mode    # must be Korean
```

Type one consonant (ㄱ–ㅎ), then Hanja. Works cleanly on Dubeolsik; on Sebeolsik the consonant entry is different.

---

## 7. "Hanja popup shows only 9 cells, period toggle does nothing"

### Diagnosis

```bash
UNIM_DEVELOP=1 systemctl --user restart unim-daemon
# Open the Hanja popup, hit `.`, then:
grep -i 'ToggleExpanded\|9x9\|expanded' ~/.unim-errors.log
```

### Fix

- Old IM modules from 0.1.x may still be installed → `make build && sudo make install`.
- Check that your keymap actually emits `.` for the period key.

---

## 8. "AutoTypeFix not firing"

### Diagnosis

```bash
unim-cli config get auto_typefix.enabled
unim-cli config get auto_typefix.forward.enabled
unim-cli config get auto_typefix.reverse.enabled
cat ~/.config/unim/typefix-blacklist.yaml | head -50
```

### Fix

- Master toggle off → flip ON in the GUI.
- Particular word always blacklisted → "Suppression Words" page, mark as Inactive or remove.
- No word boundary yet → prefix-avoidance is holding the correction. Type one more char then space — corrects immediately.
- "Skip in English mode" is ON by intent (default).

---

## 9. "AutoTypeFix corrects too aggressively / wrongly"

### Fix

| Case | Fix |
|--------|-----|
| One specific word | BS + Hangul to roll back → next time the same word triggers, it auto-registers as Tentative |
| Frequent regrets | Settings → "Type Correction" → bump tentative-expiry hours |
| Reverse fires in English mode | Turn ON `auto_typefix.reverse.skip_incomplete_syllable` |
| Hand-edit | Open `~/.config/unim/typefix-blacklist.yaml` — daemon hot-reloads on mtime change |

---

## 10. "Settings won't save / changes don't take effect"

### Diagnosis

```bash
ls -la ~/.config/unim/
test -w ~/.config/unim/config.yaml && echo writable || echo BLOCKED
unim-cli config list 2>&1 | head -5
journalctl --user -u unim-daemon -n 50
```

### Fix

- Permission issue → `chmod 644 ~/.config/unim/*.yaml`, `chmod 755 ~/.config/unim`.
- Root-owned files → `sudo chown -R $USER:$USER ~/.config/unim`.
- Changed in GUI but daemon doesn't pick it up → `systemctl --user restart unim-daemon`.

---

## 11. "Keys are locked (ghostty/terminal)"

Symptom: after one keystroke, the terminal freezes IME-wise.

### Cause

Missing `preedit-end` signal in GTK3/4 IM (a 0.1.x leftover; resolved in 0.2.0 via the `unim_emit_preedit` helper).

### Fix

```bash
unim-cli --version       # 0.2.0+ contains the fix
```

If 0.1.x, rebuild and reinstall: `make build && sudo make install`.

---

## 12. "Korean broken in Flatpak apps (Telegram, VS Code)"

### Diagnosis

```bash
flatpak list --columns=application,environment | grep -E 'GTK_IM|QT_IM'
```

### Fix

On GNOME+Wayland, the host's IM env vars leak into the Flatpak sandbox and block input. Auto-handling should clear them.

```bash
# Verify auto-handling worked
journalctl --user -u unim-daemon | grep -i flatpak
# These two lines mean OK:
#   [Flatpak] GNOME+Wayland detected — applying Flatpak IM override
#   [Flatpak] IM environment override done

# Manual fallback
flatpak override --user --env=QT_IM_MODULE= --env=GTK_IM_MODULE=
flatpak kill org.telegram.desktop
```

On X11 or non-GNOME you actually need to keep the env vars — auto-handling fires only on GNOME+Wayland.

---

## 13. "Korean broken in Snap apps"

Snap inherits host env vars but offers no global-override mechanism.

### Fix

Conditional export in `~/.profile`:

```bash
if [ "$XDG_SESSION_TYPE" = "wayland" ] && echo "$XDG_CURRENT_DESKTOP" | grep -q "GNOME"; then
    export GTK_IM_MODULE=
    export QT_IM_MODULE=
else
    export GTK_IM_MODULE=unim
    export QT_IM_MODULE=unim
fi
export XMODIFIERS="@im=unim"
```

Or per-launch:

```bash
QT_IM_MODULE= GTK_IM_MODULE= snap run telegram-desktop
```

---

## 14. "Daemon eats too much memory (RSS 500 MB+)"

### Diagnosis

```bash
grep -E 'VmRSS|VmData|Threads' /proc/$(pidof unim-daemon)/status
cat /proc/$(pidof unim-daemon)/smaps_rollup | grep -E 'Rss|Anonymous'
```

### Fix

UNIM 0.2.0 ships with `tikv_jemallocator` + `MALLOC_ARENA_MAX=2` + a 60-second `malloc_trim(0)` task, which keeps RSS in the low MB. If you still cross 500 MB:

```bash
# Quick recovery
systemctl --user restart unim-daemon

# Diagnostics for an issue report
ps -o pid,rss,vsz,cmd $(pidof unim-daemon)
journalctl --user -u unim-daemon -n 500 > unim-mem.log
```

[`AGENTS.md` §Memory rules](../../AGENTS.md) lists the regression-banned items and diagnostic commands.

---

## Build failure

```bash
make clean
make build 2>&1 | tee /tmp/unim-build.log
```

| Error | Cause | Fix |
|------|------|------|
| `lock file version 4 requires '-Znext-lockfile-bump'` | cargo 1.75 (old) | `rustup update stable` to get cargo 1.95+ |
| `gtk4/libadwaita not found` | Dev headers missing | `sudo apt install libgtk-4-dev libadwaita-1-dev` |
| `Qt6Core not found` | Qt6 dev missing | `sudo apt install qt6-base-dev` |
| `cxx-qt build error` | Qt header path mismatch | Inspect `pkg-config --cflags Qt6Core` |
| Any warnings | UNIM enforces zero-warning | File the warning verbatim as an issue |

> The canonical build command is `make build`. `cargo build --workspace` alone misses the C/C++ frontends.

---

## Diagnostic bundle (for issue reports)

```bash
{
  echo "=== version ==="
  unim-cli --version
  echo "=== env ==="
  echo "session=$XDG_SESSION_TYPE"
  echo "desktop=$XDG_CURRENT_DESKTOP"
  env | grep -E 'GTK_IM|QT_IM|XMOD' | sort
  echo "=== daemon ==="
  systemctl --user status unim-daemon --no-pager
  echo "=== config ==="
  unim-cli config list
  echo "=== logs (last 200) ==="
  tail -n 200 ~/.unim-errors.log 2>/dev/null
} > unim-report.txt
```

Attach `unim-report.txt` to your issue. Skim it once first — passwords/tokens are unlikely to be in the log, but a quick check is wise.

---

## Read more

- [FAQ](../faq/README.md) — comparison with other IMEs, coexistence, backup
- [User manual](../user-guide/README.md) — settings GUI page-by-page
- [`IME_BEHAVIOR.md`](../../IME_BEHAVIOR.md) — behavior spec (developer-oriented)
- [`AGENTS.md`](../../AGENTS.md) — architecture and memory rules
