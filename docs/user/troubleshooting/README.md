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
| Pure Wayland (Sway, etc.) | `Standalone` | Open issue — see [popup spec §8.4](../../dev/specs/POPUP_SPEC.md) |

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

## 7-1. "Wayland popup shows ◀/▶ buttons but mouse clicks do nothing"

### Symptom

On `unim-frontends/wayland` popups (compositors: GNOME mutter, KWin, Sway, etc.) the ◀/▶ buttons render correctly but a mouse left-click on them produces no reaction.

### Cause

Wayland popups are drawn on `zwp_input_popup_surface_v2`. For pointer events to reach this surface, the compositor must route them into the IM popup. Some compositors (notably some GNOME mutter versions) treat the IM popup as pass-through, so the click falls through to the application below.

### Fix

- **Immediate workaround**: keyboard `←` / `→` (or `Page Up` / `Page Down`). 100 % equivalent to the mouse buttons.
- **GNOME users**: the GNOME Shell extension popup takes over and renders outside mutter, so ◀/▶ clicks work there. If they still fail on GNOME, the extension is probably disabled or missing — check with `gnome-extensions list --enabled | grep unim`.
- **Long-term**: depends on compositor support for IM popup pointer routing. File reports at [GitHub Issues](https://github.com/from104/unim/issues), include compositor name and version.

> Keyboard ←/→ is guaranteed across every compositor. Mouse ◀/▶ is best-effort, gated by compositor policy.

---

## 7-2. "XIM emoji popup shows ◀/▶ alongside category tabs"

### Symptom

In XIM (`unim-frontends/xim`) the emoji popup shows category tabs (smileys, animals, food, …) along the top *and* ◀/▶ paginate buttons at the bottom. Both react to left-click, which can be confusing.

### Cause / status

**Working as intended.** The XIM emoji popup has two distinct controls: (1) category tabs along the top (left-click switches category), (2) ◀/▶ in the footer (left-click moves one page within the active category). Both use left-click but they live in different regions.

### Fix

- Memorize the layout: category tabs on top, page nav on the bottom.
- Or use the keyboard:
  - **Switch category**: `Tab` (next) / `Shift+Tab` (previous).
  - **Page nav**: `←` / `→` (or `Page Up`/`Page Down`).

> The behavior is intended but visually under-separated. Tracked for a footer color tweak in a future release.

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
- No word boundary yet → the correction only fires after a space/punctuation. Type one more char then space — corrects immediately.
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

[`AGENTS.md` §Memory rules](../../dev/architecture/AGENTS.md) lists the regression-banned items and diagnostic commands.

---

## 15. "Moachigi (chord) not recognized correctly"

Chord input failures have several possible causes. Work through the items below in order.

### 15-1. The active layout does not support moachigi

Chord input is only available for layouts that carry `supports_moachigi: true`. Currently only the **Ahnmatae layout (ko_3bul_anmatae)** and **Qwerty Sebeolsik v2 (ko_3bul_qwerty)** qualify.

```bash
# Check the active layout
unim-cli config show | grep -E 'layout|keymap'
```

If the output shows a different layout, switch to one of the moachigi-capable layouts in the GTK settings dialog. The **Moachigi** option group appears automatically once a compatible layout is selected.

### 15-2. chord_window_ms is too short

The recommended default for `chord_window_ms` is **60 ms**. If you are new to moachigi or type at a moderate pace, start at **80–100 ms** and lower the value as you become comfortable.

```bash
# Check current setting
unim-cli config get korean.chord_window_ms

# Set to 80 ms
unim-cli config set korean.chord_window_ms 80
```

Alternatively, use the GTK settings dialog → Keyboard → **Chord Window (ms)** slider.

### 15-3. bidirectional_combine is off

If reverse-order jamo combinations do not work (e.g., ᆯ+ᆨ → ᆰ, or ㅎ+ㄱ → ㅋ), the **Bidirectional Jamo Combine** option is disabled.

```bash
# Check current state
unim-cli config get korean.bidirectional_combine

# Enable
unim-cli config set korean.bidirectional_combine true
```

Or use the GTK settings dialog → Keyboard → **Bidirectional Jamo Combine** toggle → ON.

### 15-4. Keyboard does not support NKRO (ghosting)

Standard membrane keyboards are limited to 2–3 KRO (Key Rollover). When more keys are pressed simultaneously than the keyboard can report, the extras are silently dropped — this is called **ghosting**. Symptoms include chords that are consistently incomplete or produce the wrong jamo.

Self-diagnosis:

```sh
# Check simultaneous key events on X11
xev -event keyboard
```

On Wayland, use `wev` instead of `xev` (`apt install wev` or equivalent).

Focus the window that appears, then press all keys in your chord at once. The terminal must print one `KeyPress event` per key. You can also use an online key tester such as [keyboardchecker.com](https://keyboardchecker.com).

Fix: use a gaming keyboard or a mechanical keyboard in NKRO mode. See [Ahnmatae keyboard guide — Keyboard Compatibility](../keymaps/anmatae.en.md#keyboard-compatibility-nkro-recommended) for details.

### 15-5. Low USB polling rate (125 Hz = 8 ms resolution)

The default USB polling rate is 125 Hz (one report every 8 ms). If `chord_window_ms` is set to 10–30 ms, the polling interval itself occupies most of the window, causing some chord keys to be missed.

Fix:

- Raise `chord_window_ms` to **60 ms or higher** to accommodate polling latency.
- Switch to a 1000 Hz gaming keyboard, or try a different USB port.

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
- [`IME_BEHAVIOR.md`](../../dev/architecture/IME_BEHAVIOR.md) — behavior spec (developer-oriented)
- [`AGENTS.md`](../../dev/architecture/AGENTS.md) — architecture and memory rules

---

## 0.2.0 release-specific notes

> Auxiliary diagnostics drafted by manual-test-planner just before the 0.2.0 release. The user-facing sections above (§1–§14) take precedence; this section keeps only supplementary diagnostic tools and regression-watch items.

### A. Diagnostic helpers

| Command | Purpose |
| --- | --- |
| `journalctl --user -u unim -b --no-pager` | Daemon systemd logs (this boot) |
| `: > ~/.unim-errors.log; UNIM_DEVELOP=1 /usr/libexec/unim-daemon -n --replace &` | Reset log + restart in dev mode |
| `pgrep -a unim-` | All unim-* processes |
| `busctl --user introspect org.atit.unim.InputMethod /org/atit/unim/InputMethod` | DBus API surface |

### B. 0.2.0 regression-watch cases

- gedit double-commit (`늘늘`): focus-out CommitText broadcast — fixed in 0.2.0.
- English-mode space drop (gedit): fixed via `consumed=true commit=" "` path.
- AutoTypeFix residual BS (XIM): fixed via N+1 BS model. Chrome preedit edge case is a known SKIP.
- `tentative_expiry_hours` unit changed days → hours (1..=12) since 0.2.0; existing config auto-migrates.

### C. Multiple daemon instances

- `pkill -9 -x unim-daemon; sleep 1; systemctl --user start unim`
- Caused by DBus auto-activation overlapping with manual launch. When launching manually, use `--replace`.

### D. Hanja popup coordinates

- caret_rect missing → `cursor_y = 0` fallback (see POPUP_SPEC §6.3).
- 9-cell ↔ 81-cell toggle blocked → period (`.`) key intercepted elsewhere; check keymap.
- Bookmark (★) sync: `HanjaBookmarkChanged` signal not reaching listeners → `busctl --user monitor org.atit.unim.InputMethod`.

### E. Environment matrix as of 0.2.0

| Environment | Known issue |
| --- | --- |
| Wayland + GNOME | OK (Push mode) |
| Wayland + KDE | Hanja popup not shown (Push mode not implemented) |
| X11 + GNOME | XIM fallback recommended |
| X11 + KDE | OK |
| Pure Wayland (Weston/sway) | Hanja popup unresolved — SKIP |
